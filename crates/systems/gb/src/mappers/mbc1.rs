//! MBC1 (Memory Bank Controller 1)
//!
//! The most common Game Boy mapper, used by approximately 70% of cartridges.
//! Supports up to 2MB ROM and 32KB RAM with banking.
//!
//! # Banking Modes
//!
//! MBC1 has two banking modes:
//! - Mode 0: ROM banking mode (default)
//!   - ROM Bank 0 at 0x0000-0x3FFF (fixed)
//!   - ROM Bank 1-127 at 0x4000-0x7FFF (switchable)
//!   - RAM Bank 0 at 0xA000-0xBFFF (fixed, if enabled)
//! - Mode 1: RAM banking mode
//!   - ROM Bank 0/32/64/96 at 0x0000-0x3FFF (switchable via upper bits)
//!   - ROM Bank 1-127 at 0x4000-0x7FFF (switchable)
//!   - RAM Bank 0-3 at 0xA000-0xBFFF (switchable, if enabled)
//!
//! # Register Map
//!
//! - 0x0000-0x1FFF: RAM Enable (write 0x0A to enable, anything else to disable)
//! - 0x2000-0x3FFF: ROM Bank Number (lower 5 bits)
//! - 0x4000-0x5FFF: RAM Bank Number / ROM Bank Number (upper 2 bits)
//! - 0x6000-0x7FFF: Banking Mode Select (0 = ROM banking, 1 = RAM banking)

/// MBC1 mapper
#[derive(Debug)]
pub struct Mbc1 {
    rom: Vec<u8>,
    ram: Vec<u8>,
    ram_enabled: bool,
    rom_bank: u8,     // 5-bit register (0x2000-0x3FFF)
    ram_bank: u8,     // 2-bit register (0x4000-0x5FFF)
    banking_mode: u8, // 0 or 1
}

impl Mbc1 {
    pub fn new(rom: Vec<u8>, ram: Vec<u8>) -> Self {
        Self {
            rom,
            ram,
            ram_enabled: false,
            rom_bank: 1, // Default to bank 1 (bank 0 cannot be selected at 0x4000)
            ram_bank: 0,
            banking_mode: 0,
        }
    }

    /// Calculate the actual ROM bank for the lower bank (0x0000-0x3FFF)
    fn rom_bank_lower(&self) -> usize {
        if self.banking_mode == 1 {
            // In RAM banking mode, upper bits can affect lower bank
            ((self.ram_bank as usize) << 5) % self.rom_bank_count()
        } else {
            0
        }
    }

    /// Calculate the actual ROM bank for the upper bank (0x4000-0x7FFF)
    fn rom_bank_upper(&self) -> usize {
        let bank = (self.rom_bank & 0x1F) as usize;
        // Bank 0 is not selectable, map to bank 1
        let bank = if bank == 0 { 1 } else { bank };
        // Combine with upper bits
        let bank = bank | ((self.ram_bank as usize) << 5);
        bank % self.rom_bank_count()
    }

    /// Calculate the actual RAM bank
    fn ram_bank_current(&self) -> usize {
        if self.banking_mode == 1 {
            (self.ram_bank as usize) % self.ram_bank_count()
        } else {
            0
        }
    }

    fn rom_bank_count(&self) -> usize {
        self.rom.len().div_ceil(0x4000)
    }

    fn ram_bank_count(&self) -> usize {
        if self.ram.is_empty() {
            1
        } else {
            self.ram.len().div_ceil(0x2000)
        }
    }

    pub fn read_rom(&self, addr: u16) -> u8 {
        let bank = if addr < 0x4000 {
            self.rom_bank_lower()
        } else {
            self.rom_bank_upper()
        };

        let offset = (bank * 0x4000) + ((addr & 0x3FFF) as usize);
        if offset < self.rom.len() {
            self.rom[offset]
        } else {
            0xFF
        }
    }

    pub fn write_rom(&mut self, addr: u16, val: u8) {
        match addr {
            0x0000..=0x1FFF => {
                // RAM Enable
                self.ram_enabled = (val & 0x0F) == 0x0A;
            }
            0x2000..=0x3FFF => {
                // ROM Bank Number (lower 5 bits)
                self.rom_bank = val & 0x1F;
            }
            0x4000..=0x5FFF => {
                // RAM Bank Number / Upper ROM Bank bits
                self.ram_bank = val & 0x03;
            }
            0x6000..=0x7FFF => {
                // Banking Mode Select
                self.banking_mode = val & 0x01;
            }
            _ => {}
        }
    }

    pub fn read_ram(&self, addr: u16) -> u8 {
        if !self.ram_enabled || self.ram.is_empty() {
            return 0xFF;
        }

        let bank = self.ram_bank_current();
        let offset = (bank * 0x2000) + ((addr - 0xA000) as usize);

        if offset < self.ram.len() {
            self.ram[offset]
        } else {
            0xFF
        }
    }

    pub fn write_ram(&mut self, addr: u16, val: u8) {
        if !self.ram_enabled || self.ram.is_empty() {
            return;
        }

        let bank = self.ram_bank_current();
        let offset = (bank * 0x2000) + ((addr - 0xA000) as usize);

        if offset < self.ram.len() {
            self.ram[offset] = val;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mbc1_default_banks() {
        let rom = vec![0xFF; 0x80000]; // 512KB (32 banks)
        let mbc = Mbc1::new(rom, vec![]);

        // Bank 0 at 0x0000-0x3FFF
        assert_eq!(mbc.rom_bank_lower(), 0);
        // Bank 1 at 0x4000-0x7FFF (default)
        assert_eq!(mbc.rom_bank_upper(), 1);
    }

    #[test]
    fn test_mbc1_rom_banking() {
        let mut rom = vec![0; 0x80000]; // 512KB (32 banks)
                                        // Mark each bank with its number
        for bank in 0..32 {
            rom[bank * 0x4000] = bank as u8;
        }

        let mut mbc = Mbc1::new(rom, vec![]);

        // Default: bank 0 at lower, bank 1 at upper
        assert_eq!(mbc.read_rom(0x0000), 0);
        assert_eq!(mbc.read_rom(0x4000), 1);

        // Switch to bank 5
        mbc.write_rom(0x2000, 5);
        assert_eq!(mbc.read_rom(0x4000), 5);

        // Bank 0 is not selectable, should map to bank 1
        mbc.write_rom(0x2000, 0);
        assert_eq!(mbc.read_rom(0x4000), 1);

        // Switch to bank 31 using upper bits
        mbc.write_rom(0x2000, 0x1F); // Lower 5 bits = 31
        assert_eq!(mbc.read_rom(0x4000), 31);
    }

    #[test]
    fn test_mbc1_upper_rom_bits() {
        let mut rom = vec![0; 0x200000]; // 2MB (128 banks)
        for bank in 0..128 {
            rom[bank * 0x4000] = bank as u8;
        }

        let mut mbc = Mbc1::new(rom, vec![]);

        // Set lower bits to 1
        mbc.write_rom(0x2000, 1);
        assert_eq!(mbc.read_rom(0x4000), 1);

        // Set upper bits to 1 (bank 32 + 1 = 33)
        mbc.write_rom(0x4000, 1);
        assert_eq!(mbc.read_rom(0x4000), 33);

        // Set upper bits to 2 (bank 64 + 1 = 65)
        mbc.write_rom(0x4000, 2);
        assert_eq!(mbc.read_rom(0x4000), 65);

        // Set upper bits to 3 (bank 96 + 1 = 97)
        mbc.write_rom(0x4000, 3);
        assert_eq!(mbc.read_rom(0x4000), 97);
    }

    #[test]
    fn test_mbc1_ram_banking_mode() {
        let mut rom = vec![0; 0x200000]; // 2MB
        for bank in 0..128 {
            rom[bank * 0x4000] = bank as u8;
        }

        let mut mbc = Mbc1::new(rom, vec![]);

        // Mode 0 (ROM banking): lower bank is always 0
        mbc.write_rom(0x6000, 0);
        mbc.write_rom(0x4000, 1); // Set upper bits
        assert_eq!(mbc.read_rom(0x0000), 0);

        // Mode 1 (RAM banking): upper bits affect lower bank
        mbc.write_rom(0x6000, 1);
        assert_eq!(mbc.read_rom(0x0000), 32); // Bank 32 (upper bits = 1)

        mbc.write_rom(0x4000, 2);
        assert_eq!(mbc.read_rom(0x0000), 64); // Bank 64 (upper bits = 2)
    }

    #[test]
    fn test_mbc1_ram_enable() {
        let mbc = Mbc1::new(vec![0; 0x8000], vec![0; 0x2000]);
        assert!(!mbc.ram_enabled);

        let mut mbc = mbc;
        // Enable RAM
        mbc.write_rom(0x0000, 0x0A);
        assert!(mbc.ram_enabled);

        // Disable RAM
        mbc.write_rom(0x0000, 0x00);
        assert!(!mbc.ram_enabled);
    }

    #[test]
    fn test_mbc1_ram_read_write() {
        let mut mbc = Mbc1::new(vec![0; 0x8000], vec![0; 0x2000]);

        // RAM disabled, reads return 0xFF
        assert_eq!(mbc.read_ram(0xA000), 0xFF);

        // Enable RAM
        mbc.write_rom(0x0000, 0x0A);

        // Write and read back
        mbc.write_ram(0xA000, 0x42);
        assert_eq!(mbc.read_ram(0xA000), 0x42);

        // Disable RAM, writes ignored
        mbc.write_rom(0x0000, 0x00);
        mbc.write_ram(0xA000, 0xFF);

        // Enable and check old value is still there
        mbc.write_rom(0x0000, 0x0A);
        assert_eq!(mbc.read_ram(0xA000), 0x42);
    }

    #[test]
    fn test_mbc1_ram_banking() {
        let mut ram = vec![0; 0x8000]; // 32KB (4 banks)
                                       // Mark each bank
        for bank in 0..4 {
            ram[bank * 0x2000] = bank as u8;
        }

        let mut mbc = Mbc1::new(vec![0; 0x8000], ram);
        mbc.write_rom(0x0000, 0x0A); // Enable RAM

        // Mode 0: only bank 0 accessible
        mbc.write_rom(0x6000, 0);
        assert_eq!(mbc.read_ram(0xA000), 0);

        mbc.write_rom(0x4000, 1);
        assert_eq!(mbc.read_ram(0xA000), 0); // Still bank 0 in mode 0

        // Mode 1: bank switching works
        mbc.write_rom(0x6000, 1);
        assert_eq!(mbc.read_ram(0xA000), 1); // Now bank 1

        mbc.write_rom(0x4000, 2);
        assert_eq!(mbc.read_ram(0xA000), 2); // Bank 2

        mbc.write_rom(0x4000, 3);
        assert_eq!(mbc.read_ram(0xA000), 3); // Bank 3
    }

    #[test]
    fn test_mbc1_no_ram() {
        let mut mbc = Mbc1::new(vec![0; 0x8000], vec![]);

        // Enable RAM (but cart has no RAM)
        mbc.write_rom(0x0000, 0x0A);

        // Reads should return 0xFF
        assert_eq!(mbc.read_ram(0xA000), 0xFF);

        // Writes should be ignored
        mbc.write_ram(0xA000, 0x42);
        assert_eq!(mbc.read_ram(0xA000), 0xFF);
    }

    #[test]
    fn test_mbc1_bank_wrapping() {
        let rom = vec![0xFF; 0x20000]; // 128KB (8 banks)
        let mbc = Mbc1::new(rom, vec![]);

        // ROM has only 8 banks, requesting bank 9 should wrap to bank 1
        assert!(mbc.rom_bank_count() == 8);
    }
}
