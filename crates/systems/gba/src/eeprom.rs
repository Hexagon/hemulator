//! GBA EEPROM save storage emulation.
//!
//! EEPROM on the GBA uses a serial bit-banging protocol accessed through
//! specific addresses in the cartridge ROM space (typically 0x0D000000+
//! for ROMs > 16MB, or the upper portion of the ROM address space).
//!
//! ## Protocol
//!
//! All communication is done 1 bit at a time via DMA3 transfers.
//! The game writes 16-bit values where only bit 0 matters (the serial data bit).
//!
//! ### Read Request (game → EEPROM → game)
//!
//! 1. Write `11` (2 bits: read command)
//! 2. Write address (6 or 14 bits depending on EEPROM size)
//! 3. Write `0` (end bit)
//! 4. Read 4 dummy bits (all 1s)
//! 5. Read 64 data bits (8 bytes, MSB first)
//!
//! ### Write Request (game → EEPROM)
//!
//! 1. Write `10` (2 bits: write command)
//! 2. Write address (6 or 14 bits depending on EEPROM size)
//! 3. Write 64 data bits (8 bytes, MSB first)
//! 4. Write `0` (end bit)
//! 5. Read status: 0 = busy, 1 = ready
//!
//! ## Sizes
//!
//! - **512 bytes** (64 blocks × 8 bytes): 6-bit addressing
//! - **8 KB** (1024 blocks × 8 bytes): 14-bit addressing

use emu_core::logging::{log, LogCategory, LogLevel};

/// EEPROM size: 512 bytes (64 blocks × 8 bytes each)
const EEPROM_SIZE_SMALL: usize = 512;
/// EEPROM size: 8KB (1024 blocks × 8 bytes each)
const EEPROM_SIZE_LARGE: usize = 8192;

/// Address bits for small EEPROM (6 bits → 64 blocks)
const ADDR_BITS_SMALL: u32 = 6;
/// Address bits for large EEPROM (14 bits → 1024 blocks)
const ADDR_BITS_LARGE: u32 = 14;

/// Number of data bits per block (8 bytes = 64 bits)
const BLOCK_BITS: u32 = 64;

/// EEPROM protocol state machine
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EepromState {
    /// Waiting for command bits
    Idle,
    /// Receiving the command type (2 bits: 10=write, 11=read)
    ReceivingCommand,
    /// Receiving address bits
    ReceivingAddress,
    /// Receiving data bits (write command)
    ReceivingData,
    /// Waiting for end bit after write data
    ReceivingEndBit,
    /// Transmitting dummy bits before read data (4 bits)
    TransmittingDummy,
    /// Transmitting data bits (read command, 64 bits)
    TransmittingData,
    /// Write completed, returning ready status (bit 1)
    WriteFinished,
}

/// GBA EEPROM controller.
///
/// Emulates serial EEPROM with 512B or 8KB capacity.
#[derive(Debug, Clone)]
pub struct Eeprom {
    /// EEPROM data storage
    data: Vec<u8>,
    /// Number of address bits (6 for 512B, 14 for 8KB)
    addr_bits: u32,
    /// Current protocol state
    state: EepromState,
    /// Bit counter within current phase
    bit_count: u32,
    /// Accumulated command bits
    command: u8,
    /// Accumulated address
    address: u32,
    /// Data buffer for read/write (64 bits = 8 bytes)
    buffer: u64,
    /// Whether size has been auto-detected
    size_detected: bool,
}

impl Eeprom {
    /// Create a new EEPROM with unknown size (will be auto-detected).
    pub fn new() -> Self {
        // Default to large (8KB) until we can detect size from access patterns
        Self {
            data: vec![0xFF; EEPROM_SIZE_LARGE],
            addr_bits: ADDR_BITS_LARGE,
            state: EepromState::Idle,
            bit_count: 0,
            command: 0,
            address: 0,
            buffer: 0,
            size_detected: false,
        }
    }

    /// Reset the EEPROM state machine (not the data)
    pub fn reset_state(&mut self) {
        self.state = EepromState::Idle;
        self.bit_count = 0;
        self.command = 0;
        self.address = 0;
        self.buffer = 0;
    }

    /// Get reference to EEPROM data for save state serialization
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Restore EEPROM data from save state
    pub fn set_data(&mut self, data: &[u8]) {
        let len = data.len().min(self.data.len());
        self.data[..len].copy_from_slice(&data[..len]);
    }

    /// Write a bit to the EEPROM (called when game writes to EEPROM address).
    ///
    /// `val` is the 16-bit value written; only bit 0 is the serial data bit.
    pub fn write_bit(&mut self, val: u16) {
        let bit = (val & 1) != 0;

        match self.state {
            EepromState::Idle => {
                // First bit of a new command
                self.command = bit as u8;
                self.bit_count = 1;
                self.state = EepromState::ReceivingCommand;
            }

            EepromState::ReceivingCommand => {
                // Second bit of command
                self.command = (self.command << 1) | bit as u8;
                self.bit_count = 0;
                self.address = 0;

                match self.command {
                    0b11 => {
                        // Read command
                        self.state = EepromState::ReceivingAddress;
                    }
                    0b10 => {
                        // Write command
                        self.state = EepromState::ReceivingAddress;
                    }
                    _ => {
                        // Invalid command, reset
                        log(LogCategory::Bus, LogLevel::Debug, || {
                            format!("EEPROM: Invalid command 0b{:02b}, resetting", self.command)
                        });
                        self.reset_state();
                    }
                }
            }

            EepromState::ReceivingAddress => {
                self.address = (self.address << 1) | bit as u32;
                self.bit_count += 1;

                if self.bit_count >= self.addr_bits {
                    // Auto-detect size: if we get a 6-bit address access and
                    // haven't detected size yet, check the DMA transfer size
                    if !self.size_detected && self.bit_count == ADDR_BITS_LARGE {
                        // We consumed 14 bits of address. If the game actually
                        // meant 6-bit addressing, we'd have gotten the data
                        // bits mixed in. We'll detect this from the total
                        // transfer length instead (handled externally).
                        self.size_detected = true;
                    }

                    self.bit_count = 0;
                    self.buffer = 0;

                    if self.command == 0b11 {
                        // Read: load data from storage into buffer
                        let block = self.address as usize;
                        let byte_addr = block * 8;
                        self.buffer = 0;
                        for i in 0..8 {
                            let byte = if byte_addr + i < self.data.len() {
                                self.data[byte_addr + i]
                            } else {
                                0xFF
                            };
                            self.buffer = (self.buffer << 8) | byte as u64;
                        }
                        self.state = EepromState::TransmittingDummy;
                    } else {
                        // Write: receive 64 data bits
                        self.state = EepromState::ReceivingData;
                    }
                }
            }

            EepromState::ReceivingData => {
                self.buffer = (self.buffer << 1) | bit as u64;
                self.bit_count += 1;

                if self.bit_count >= BLOCK_BITS {
                    self.state = EepromState::ReceivingEndBit;
                }
            }

            EepromState::ReceivingEndBit => {
                // End bit received, commit write
                let block = self.address as usize;
                let byte_addr = block * 8;

                for i in 0..8 {
                    let byte = ((self.buffer >> (56 - i * 8)) & 0xFF) as u8;
                    if byte_addr + i < self.data.len() {
                        self.data[byte_addr + i] = byte;
                    }
                }

                log(LogCategory::Bus, LogLevel::Debug, || {
                    format!("EEPROM: Write block {} (addr=0x{:04X})", block, byte_addr)
                });

                self.state = EepromState::WriteFinished;
                self.bit_count = 0;
            }

            EepromState::TransmittingDummy | EepromState::TransmittingData => {
                // Writes during read phase are ignored (game is reading)
            }

            EepromState::WriteFinished => {
                // Writes during status phase are ignored
            }
        }
    }

    /// Read a bit from the EEPROM (called when game reads from EEPROM address).
    ///
    /// Returns a 16-bit value with the serial data bit in bit 0.
    pub fn read_bit(&mut self) -> u16 {
        match self.state {
            EepromState::TransmittingDummy => {
                self.bit_count += 1;
                if self.bit_count >= 4 {
                    self.bit_count = 0;
                    self.state = EepromState::TransmittingData;
                }
                // Dummy bits are all 0 (some docs say 1, but 0 works)
                0
            }

            EepromState::TransmittingData => {
                // MSB first
                let bit_idx = BLOCK_BITS - 1 - self.bit_count;
                let bit = ((self.buffer >> bit_idx) & 1) as u16;
                self.bit_count += 1;

                if self.bit_count >= BLOCK_BITS {
                    // Transfer complete, return to idle
                    self.reset_state();
                }

                bit
            }

            EepromState::WriteFinished => {
                // Return ready status (1 = ready)
                self.reset_state();
                1
            }

            _ => {
                // Not in a read phase, return 1 (ready/high)
                1
            }
        }
    }

    /// Detect EEPROM size from the DMA word count used for the first transfer.
    ///
    /// - 9 words (read) or 73 words (write) → 512B EEPROM (6-bit address)
    /// - 17 words (read) or 81 words (write) → 8KB EEPROM (14-bit address)
    pub fn detect_size_from_dma(&mut self, word_count: u32) {
        if self.size_detected {
            return;
        }

        let is_small = matches!(word_count, 9 | 73);
        let is_large = matches!(word_count, 17 | 81);

        if is_small {
            log(LogCategory::Bus, LogLevel::Info, || {
                "EEPROM: Detected 512B (6-bit addressing) from DMA transfer size".to_string()
            });
            self.addr_bits = ADDR_BITS_SMALL;
            self.data.resize(EEPROM_SIZE_SMALL, 0xFF);
            self.size_detected = true;
        } else if is_large {
            log(LogCategory::Bus, LogLevel::Info, || {
                "EEPROM: Detected 8KB (14-bit addressing) from DMA transfer size".to_string()
            });
            self.addr_bits = ADDR_BITS_LARGE;
            // Already 8KB by default
            self.size_detected = true;
        }
    }

    /// Check if this EEPROM address should be intercepted.
    ///
    /// For ROMs > 16MB, EEPROM is mapped at 0x0D000000-0x0DFFFFFF.
    /// For ROMs <= 16MB, EEPROM is at the address just past the ROM end
    /// (typically 0x0DFFFF00-0x0DFFFFFF for any ROM).
    ///
    /// In practice, most games access EEPROM at the very end of each
    /// wait state region, so we check the top of the 0x0D range.
    pub fn is_eeprom_address(addr: u32, rom_size: usize) -> bool {
        // EEPROM is accessed in the upper portion of the ROM address space.
        // For ROMs > 16MB, it occupies wait state 2 (0x0D000000).
        // For smaller ROMs, it's accessible at the end of any wait state region.
        //
        // Most games use DMA3 to access EEPROM at addresses like 0x0DFFFF00.
        // We intercept any access in 0x0D000000-0x0DFFFFFF that falls outside
        // the actual ROM data.
        if (0x0D000000..=0x0DFFFFFF).contains(&addr) {
            let offset = (addr & 0x01FFFFFF) as usize;
            // If the address is beyond the ROM, it's EEPROM
            return offset >= rom_size;
        }
        false
    }
}

impl Default for Eeprom {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eeprom_write_read_cycle() {
        let mut eeprom = Eeprom::new();
        eeprom.addr_bits = ADDR_BITS_SMALL;
        eeprom.size_detected = true;
        eeprom.data = vec![0xFF; EEPROM_SIZE_SMALL];

        // Write command: 10 + 6-bit address (0) + 64 data bits + end bit
        // Command: write (10)
        eeprom.write_bit(1); // bit 1 of command
        eeprom.write_bit(0); // bit 0 of command -> 0b10 = write

        // Address: 0 (6 bits)
        for _ in 0..6 {
            eeprom.write_bit(0);
        }

        // Data: 64 bits = 0x0123456789ABCDEF
        let test_data: u64 = 0x0123456789ABCDEF;
        for i in 0..64 {
            let bit = ((test_data >> (63 - i)) & 1) as u16;
            eeprom.write_bit(bit);
        }

        // End bit
        eeprom.write_bit(0);

        // Read status (should be ready = 1)
        let status = eeprom.read_bit();
        assert_eq!(status, 1);

        // Now read it back
        // Command: read (11)
        eeprom.write_bit(1); // bit 1 of command
        eeprom.write_bit(1); // bit 0 of command -> 0b11 = read

        // Address: 0 (6 bits)
        for _ in 0..6 {
            eeprom.write_bit(0);
        }

        // End bit (the 0 after address for read)
        eeprom.write_bit(0);

        // Read 4 dummy bits
        for _ in 0..4 {
            eeprom.read_bit();
        }

        // Read 64 data bits
        let mut result: u64 = 0;
        for _ in 0..64 {
            let bit = eeprom.read_bit() as u64;
            result = (result << 1) | bit;
        }

        assert_eq!(result, test_data);
    }

    #[test]
    fn test_eeprom_default_ff() {
        let mut eeprom = Eeprom::new();
        eeprom.addr_bits = ADDR_BITS_SMALL;
        eeprom.size_detected = true;

        // Read block 0 without writing first - should be 0xFF
        eeprom.write_bit(1); // read command
        eeprom.write_bit(1);
        for _ in 0..6 {
            eeprom.write_bit(0);
        }
        eeprom.write_bit(0); // end

        // Dummy bits
        for _ in 0..4 {
            eeprom.read_bit();
        }

        // Read data - should be all 1s (0xFF bytes)
        let mut result: u64 = 0;
        for _ in 0..64 {
            let bit = eeprom.read_bit() as u64;
            result = (result << 1) | bit;
        }

        assert_eq!(result, 0xFFFFFFFFFFFFFFFF);
    }

    #[test]
    fn test_size_detection() {
        let mut eeprom = Eeprom::new();
        assert!(!eeprom.size_detected);

        eeprom.detect_size_from_dma(9);
        assert!(eeprom.size_detected);
        assert_eq!(eeprom.addr_bits, ADDR_BITS_SMALL);
        assert_eq!(eeprom.data.len(), EEPROM_SIZE_SMALL);
    }

    #[test]
    fn test_eeprom_address_detection() {
        // ROM size 16MB - EEPROM at 0x0D000000+
        assert!(Eeprom::is_eeprom_address(0x0DFFFF00, 16 * 1024 * 1024));
        assert!(Eeprom::is_eeprom_address(0x0D000000, 16 * 1024 * 1024));

        // ROM size 8MB - EEPROM at end of 0x0D region (past ROM)
        assert!(Eeprom::is_eeprom_address(0x0DFFFF00, 8 * 1024 * 1024));
        assert!(Eeprom::is_eeprom_address(0x0D800000, 8 * 1024 * 1024));

        // In-ROM address - not EEPROM
        assert!(!Eeprom::is_eeprom_address(0x08000000, 16 * 1024 * 1024));
    }
}
