//! GBA Flash save storage emulation.
//!
//! Flash memory on the GBA is accessed at address 0x0E000000 through a
//! command-based protocol. Games identify the Flash chip by reading
//! manufacturer/device IDs and use sector erase + byte program operations.
//!
//! ## Supported Chip Types
//!
//! | Chip            | Manufacturer | Device | Size  | Sectors |
//! |-----------------|-------------|--------|-------|---------|
//! | Sanyo LE26FV10N | 0x62        | 0x13   | 128KB | 2×16    |
//! | Macronix MX29L010 | 0xC2     | 0x09   | 128KB | 2×16    |
//! | SST 39VF512     | 0xBF        | 0xD4   | 64KB  | 16      |
//! | Panasonic MN63F805 | 0x32    | 0x1B   | 64KB  | 16      |
//! | Atmel AT29LV512 | 0x1F        | 0x3D   | 64KB  | 1024    |
//!
//! ## Command Protocol
//!
//! Commands follow the pattern: write 0xAA to 0x5555, 0x55 to 0x2AAA, then
//! the command byte to 0x5555 (or other address for some commands).
//!
//! | Command       | Byte 1        | Byte 2        | Byte 3           |
//! |---------------|---------------|---------------|------------------|
//! | Enter ID mode | 5555h=AAh     | 2AAAh=55h     | 5555h=90h        |
//! | Exit ID mode  | 5555h=AAh     | 2AAAh=55h     | 5555h=F0h        |
//! | Prepare erase | 5555h=AAh     | 2AAAh=55h     | 5555h=80h        |
//! | Erase all     | 5555h=AAh     | 2AAAh=55h     | 5555h=10h        |
//! | Erase sector  | 5555h=AAh     | 2AAAh=55h     | sectorAddr=30h   |
//! | Write byte    | 5555h=AAh     | 2AAAh=55h     | 5555h=A0h        |
//! | Bank switch   | 5555h=AAh     | 2AAAh=55h     | 5555h=B0h        |

use emu_core::logging::{log, LogCategory, LogLevel};

/// Flash sector size: 4KB
const SECTOR_SIZE: usize = 4096;

/// Single bank size: 64KB
const BANK_SIZE: usize = 65536;

/// Flash chip state machine
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FlashState {
    /// Ready for commands
    Ready,
    /// Received first command byte (0xAA to 0x5555)
    Cmd1,
    /// Received second command byte (0x55 to 0x2AAA)
    Cmd2,
    /// In ID mode (reads return manufacturer/device ID)
    IdMode,
    /// Preparing for erase (after 0x80 command)
    PrepareErase,
    /// Received first erase confirmation (0xAA to 0x5555)
    EraseCmd1,
    /// Received second erase confirmation (0x55 to 0x2AAA)
    EraseCmd2,
    /// Single byte write mode (after 0xA0 command)
    WriteByte,
    /// Bank switch mode (after 0xB0 command) — 128KB Flash only
    BankSwitch,
}

/// GBA Flash save storage
#[derive(Debug, Clone)]
pub struct Flash {
    /// Flash data (64KB or 128KB, initialized to 0xFF = erased)
    data: Vec<u8>,
    /// Current state machine state
    state: FlashState,
    /// Whether the chip is in ID mode
    id_mode: bool,
    /// Currently selected bank (0 or 1, for 128KB Flash)
    bank: usize,
    /// Total size in bytes (65536 or 131072)
    size: usize,
    /// Manufacturer ID
    manufacturer_id: u8,
    /// Device ID
    device_id: u8,
}

impl Flash {
    /// Create a new Flash chip with the given size.
    ///
    /// `is_128k` — true for 128KB (FLASH1M_V), false for 64KB (FLASH_V)
    pub fn new(is_128k: bool) -> Self {
        let size = if is_128k { BANK_SIZE * 2 } else { BANK_SIZE };

        // Use Sanyo LE26FV10N1TS IDs for 128KB, Panasonic MN63F805MNP for 64KB
        let (manufacturer_id, device_id) = if is_128k {
            (0x62, 0x13) // Sanyo
        } else {
            (0x32, 0x1B) // Panasonic
        };

        Self {
            data: vec![0xFF; size], // Flash is erased to 0xFF
            state: FlashState::Ready,
            id_mode: false,
            bank: 0,
            size,
            manufacturer_id,
            device_id,
        }
    }

    /// Read a byte from Flash at the given offset (0x0000-0xFFFF within current bank)
    pub fn read(&self, addr: u16) -> u8 {
        if self.id_mode {
            // In ID mode, address 0x0000 returns manufacturer ID,
            // address 0x0001 returns device ID
            match addr {
                0x0000 => {
                    log(LogCategory::Bus, LogLevel::Debug, || {
                        format!("Flash: Read manufacturer ID: {:02X}", self.manufacturer_id)
                    });
                    self.manufacturer_id
                }
                0x0001 => {
                    log(LogCategory::Bus, LogLevel::Debug, || {
                        format!("Flash: Read device ID: {:02X}", self.device_id)
                    });
                    self.device_id
                }
                _ => 0, // Other addresses return 0 in ID mode
            }
        } else {
            let offset = self.bank * BANK_SIZE + addr as usize;
            if offset < self.data.len() {
                self.data[offset]
            } else {
                0xFF
            }
        }
    }

    /// Write a byte to Flash. This handles the command protocol state machine.
    pub fn write(&mut self, addr: u16, val: u8) {
        log(LogCategory::Bus, LogLevel::Trace, || {
            format!(
                "Flash: Write {:02X} to {:04X} (state={:?}, bank={})",
                val, addr, self.state, self.bank
            )
        });

        match self.state {
            FlashState::Ready => {
                if addr == 0x5555 && val == 0xAA {
                    self.state = FlashState::Cmd1;
                }
            }

            FlashState::Cmd1 => {
                if addr == 0x2AAA && val == 0x55 {
                    self.state = FlashState::Cmd2;
                } else {
                    // Invalid sequence, reset
                    self.state = FlashState::Ready;
                }
            }

            FlashState::Cmd2 => {
                if addr == 0x5555 {
                    match val {
                        0x90 => {
                            // Enter ID mode
                            self.id_mode = true;
                            self.state = FlashState::IdMode;
                            log(LogCategory::Bus, LogLevel::Debug, || {
                                "Flash: Enter ID mode".to_string()
                            });
                        }
                        0xF0 => {
                            // Exit ID mode / software reset
                            self.id_mode = false;
                            self.state = FlashState::Ready;
                            log(LogCategory::Bus, LogLevel::Debug, || {
                                "Flash: Exit ID mode".to_string()
                            });
                        }
                        0x80 => {
                            // Prepare erase
                            self.state = FlashState::PrepareErase;
                        }
                        0xA0 => {
                            // Enter single byte write mode
                            self.state = FlashState::WriteByte;
                        }
                        0xB0 => {
                            // Bank switch (128KB only)
                            if self.size > BANK_SIZE {
                                self.state = FlashState::BankSwitch;
                            } else {
                                self.state = FlashState::Ready;
                            }
                        }
                        _ => {
                            log(LogCategory::Bus, LogLevel::Warn, || {
                                format!("Flash: Unknown command {:02X}", val)
                            });
                            self.state = FlashState::Ready;
                        }
                    }
                } else {
                    self.state = FlashState::Ready;
                }
            }

            FlashState::IdMode => {
                // In ID mode, accept exit command sequence
                if addr == 0x5555 && val == 0xAA {
                    self.state = FlashState::Cmd1;
                } else if addr == 0x5555 && val == 0xF0 {
                    // Direct exit from ID mode
                    self.id_mode = false;
                    self.state = FlashState::Ready;
                }
            }

            FlashState::PrepareErase => {
                if addr == 0x5555 && val == 0xAA {
                    self.state = FlashState::EraseCmd1;
                } else {
                    self.state = FlashState::Ready;
                }
            }

            FlashState::EraseCmd1 => {
                if addr == 0x2AAA && val == 0x55 {
                    self.state = FlashState::EraseCmd2;
                } else {
                    self.state = FlashState::Ready;
                }
            }

            FlashState::EraseCmd2 => {
                if addr == 0x5555 && val == 0x10 {
                    // Erase entire chip
                    log(LogCategory::Bus, LogLevel::Debug, || {
                        "Flash: Chip erase".to_string()
                    });
                    self.data.fill(0xFF);
                    self.state = FlashState::Ready;
                } else if val == 0x30 {
                    // Sector erase: addr determines which 4KB sector
                    let sector = (addr as usize >> 12) & 0xF;
                    let base = self.bank * BANK_SIZE + sector * SECTOR_SIZE;
                    let end = base + SECTOR_SIZE;
                    if end <= self.data.len() {
                        log(LogCategory::Bus, LogLevel::Debug, || {
                            format!(
                                "Flash: Sector erase bank={} sector={} (${:05X}-${:05X})",
                                self.bank,
                                sector,
                                base,
                                end - 1
                            )
                        });
                        self.data[base..end].fill(0xFF);
                    }
                    self.state = FlashState::Ready;
                } else {
                    self.state = FlashState::Ready;
                }
            }

            FlashState::WriteByte => {
                // Write a single byte (Flash can only clear bits, i.e., AND with existing data)
                let offset = self.bank * BANK_SIZE + addr as usize;
                if offset < self.data.len() {
                    // Flash write: can only change 1→0 (AND behavior)
                    self.data[offset] &= val;
                    log(LogCategory::Bus, LogLevel::Trace, || {
                        format!(
                            "Flash: Write byte ${:02X} at bank={} addr=${:04X} (offset=${:05X})",
                            val, self.bank, addr, offset
                        )
                    });
                }
                self.state = FlashState::Ready;
            }

            FlashState::BankSwitch => {
                // Bank switch: write bank number to 0x0000
                if addr == 0x0000 {
                    self.bank = (val & 1) as usize;
                    log(LogCategory::Bus, LogLevel::Debug, || {
                        format!("Flash: Bank switch to {}", self.bank)
                    });
                }
                self.state = FlashState::Ready;
            }
        }
    }

    /// Get a reference to the raw Flash data for save state serialization
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Load Flash data from a save state
    pub fn load_data(&mut self, data: &[u8]) {
        let len = data.len().min(self.data.len());
        self.data[..len].copy_from_slice(&data[..len]);
    }

    /// Get the current bank index
    pub fn bank(&self) -> usize {
        self.bank
    }

    /// Whether this is a 128KB Flash chip
    pub fn is_128k(&self) -> bool {
        self.size > BANK_SIZE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flash_initial_state() {
        let flash = Flash::new(false);
        assert_eq!(flash.size, BANK_SIZE);
        assert!(flash.data.iter().all(|&b| b == 0xFF));
    }

    #[test]
    fn test_flash_128k_initial_state() {
        let flash = Flash::new(true);
        assert_eq!(flash.size, BANK_SIZE * 2);
        assert!(flash.data.iter().all(|&b| b == 0xFF));
    }

    #[test]
    fn test_flash_id_mode() {
        let mut flash = Flash::new(true); // Sanyo 128KB

        // Enter ID mode: 0xAA→0x5555, 0x55→0x2AAA, 0x90→0x5555
        flash.write(0x5555, 0xAA);
        flash.write(0x2AAA, 0x55);
        flash.write(0x5555, 0x90);

        assert_eq!(flash.read(0x0000), 0x62); // Sanyo manufacturer
        assert_eq!(flash.read(0x0001), 0x13); // Device ID

        // Exit ID mode
        flash.write(0x5555, 0xAA);
        flash.write(0x2AAA, 0x55);
        flash.write(0x5555, 0xF0);

        // Normal read returns 0xFF (erased)
        assert_eq!(flash.read(0x0000), 0xFF);
    }

    #[test]
    fn test_flash_write_byte() {
        let mut flash = Flash::new(false);

        // Write 0x42 to address 0x1000
        flash.write(0x5555, 0xAA);
        flash.write(0x2AAA, 0x55);
        flash.write(0x5555, 0xA0);
        flash.write(0x1000, 0x42);

        assert_eq!(flash.read(0x1000), 0x42);
        assert_eq!(flash.read(0x1001), 0xFF); // Adjacent byte still erased
    }

    #[test]
    fn test_flash_sector_erase() {
        let mut flash = Flash::new(false);

        // Write some data first
        flash.write(0x5555, 0xAA);
        flash.write(0x2AAA, 0x55);
        flash.write(0x5555, 0xA0);
        flash.write(0x1000, 0x42);
        assert_eq!(flash.read(0x1000), 0x42);

        // Erase sector 1 (0x1000-0x1FFF)
        flash.write(0x5555, 0xAA);
        flash.write(0x2AAA, 0x55);
        flash.write(0x5555, 0x80);
        flash.write(0x5555, 0xAA);
        flash.write(0x2AAA, 0x55);
        flash.write(0x1000, 0x30);

        assert_eq!(flash.read(0x1000), 0xFF); // Erased
    }

    #[test]
    fn test_flash_chip_erase() {
        let mut flash = Flash::new(false);

        // Write data
        flash.write(0x5555, 0xAA);
        flash.write(0x2AAA, 0x55);
        flash.write(0x5555, 0xA0);
        flash.write(0x0000, 0x42);

        flash.write(0x5555, 0xAA);
        flash.write(0x2AAA, 0x55);
        flash.write(0x5555, 0xA0);
        flash.write(0x1000, 0x55);

        // Chip erase
        flash.write(0x5555, 0xAA);
        flash.write(0x2AAA, 0x55);
        flash.write(0x5555, 0x80);
        flash.write(0x5555, 0xAA);
        flash.write(0x2AAA, 0x55);
        flash.write(0x5555, 0x10);

        assert_eq!(flash.read(0x0000), 0xFF);
        assert_eq!(flash.read(0x1000), 0xFF);
    }

    #[test]
    fn test_flash_bank_switch() {
        let mut flash = Flash::new(true); // 128KB

        // Write to bank 0
        flash.write(0x5555, 0xAA);
        flash.write(0x2AAA, 0x55);
        flash.write(0x5555, 0xA0);
        flash.write(0x0000, 0x11);

        // Switch to bank 1
        flash.write(0x5555, 0xAA);
        flash.write(0x2AAA, 0x55);
        flash.write(0x5555, 0xB0);
        flash.write(0x0000, 0x01);

        // Write to bank 1
        flash.write(0x5555, 0xAA);
        flash.write(0x2AAA, 0x55);
        flash.write(0x5555, 0xA0);
        flash.write(0x0000, 0x22);

        // Read bank 1
        assert_eq!(flash.read(0x0000), 0x22);

        // Switch back to bank 0
        flash.write(0x5555, 0xAA);
        flash.write(0x2AAA, 0x55);
        flash.write(0x5555, 0xB0);
        flash.write(0x0000, 0x00);

        // Read bank 0
        assert_eq!(flash.read(0x0000), 0x11);
    }

    #[test]
    fn test_flash_write_and_behavior() {
        let mut flash = Flash::new(false);

        // Flash write is AND: can only clear bits (1→0), not set them
        // First write 0xF0
        flash.write(0x5555, 0xAA);
        flash.write(0x2AAA, 0x55);
        flash.write(0x5555, 0xA0);
        flash.write(0x0000, 0xF0);
        assert_eq!(flash.read(0x0000), 0xF0);

        // Second write 0x0F — AND with 0xF0 = 0x00
        flash.write(0x5555, 0xAA);
        flash.write(0x2AAA, 0x55);
        flash.write(0x5555, 0xA0);
        flash.write(0x0000, 0x0F);
        assert_eq!(flash.read(0x0000), 0x00);
    }
}
