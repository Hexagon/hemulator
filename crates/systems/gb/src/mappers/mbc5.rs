//! MBC5 (Memory Bank Controller 5)
//!
//! The most advanced standard MBC, used for large ROMs and newer games.
//! Supports up to 8MB ROM and 128KB RAM.
//!
//! # Register Map
//!
//! - 0x0000-0x1FFF: RAM Enable (write 0x0A to enable)
//! - 0x2000-0x2FFF: ROM Bank Number (lower 8 bits)
//! - 0x3000-0x3FFF: ROM Bank Number (upper 1 bit, bit 8)
//! - 0x4000-0x5FFF: RAM Bank Number (4 bits, 0-15)

/// MBC5 mapper
#[derive(Debug)]
pub struct Mbc5 {
    rom: Vec<u8>,
    ram: Vec<u8>,
    ram_enabled: bool,
    rom_bank: u16, // 9-bit register (0-511)
    ram_bank: u8,  // 4-bit register (0-15)
}

impl Mbc5 {
    pub fn new(rom: Vec<u8>, ram: Vec<u8>) -> Self {
        Self {
            rom,
            ram,
            ram_enabled: false,
            rom_bank: 1,
            ram_bank: 0,
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
            0
        } else {
            (self.rom_bank as usize) % self.rom_bank_count()
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
            0x2000..=0x2FFF => {
                // ROM Bank Number (lower 8 bits)
                self.rom_bank = (self.rom_bank & 0x100) | (val as u16);
            }
            0x3000..=0x3FFF => {
                // ROM Bank Number (upper 1 bit)
                self.rom_bank = (self.rom_bank & 0x0FF) | (((val & 0x01) as u16) << 8);
            }
            0x4000..=0x5FFF => {
                // RAM Bank Number (4 bits)
                self.ram_bank = val & 0x0F;
            }
            _ => {}
        }
    }

    pub fn read_ram(&self, addr: u16) -> u8 {
        if !self.ram_enabled || self.ram.is_empty() {
            return 0xFF;
        }

        let bank = (self.ram_bank as usize) % self.ram_bank_count();
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

        let bank = (self.ram_bank as usize) % self.ram_bank_count();
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
    fn test_mbc5_default_banks() {
        let rom = vec![0xFF; 0x80000]; // 512KB
        let mbc = Mbc5::new(rom, vec![]);

        assert_eq!(mbc.rom_bank, 1);
        assert_eq!(mbc.ram_bank, 0);
    }

    #[test]
    fn test_mbc5_rom_banking() {
        let mut rom = vec![0; 0x200000]; // 2MB (128 banks)
        for bank in 0..128 {
            rom[bank * 0x4000] = bank as u8;
        }

        let mut mbc = Mbc5::new(rom, vec![]);

        // Bank 0 at lower, bank 1 at upper
        assert_eq!(mbc.read_rom(0x0000), 0);
        assert_eq!(mbc.read_rom(0x4000), 1);

        // Switch to bank 5
        mbc.write_rom(0x2000, 5);
        assert_eq!(mbc.read_rom(0x4000), 5);

        // Switch to bank 127
        mbc.write_rom(0x2000, 127);
        assert_eq!(mbc.read_rom(0x4000), 127);

        // Bank 0 can be selected at 0x4000 (unlike MBC1)
        mbc.write_rom(0x2000, 0);
        assert_eq!(mbc.read_rom(0x4000), 0);
    }

    #[test]
    fn test_mbc5_9bit_rom_banking() {
        let mut rom = vec![0; 0x800000]; // 8MB (512 banks)
        for bank in 0..512 {
            rom[bank * 0x4000] = (bank & 0xFF) as u8;
        }

        let mut mbc = Mbc5::new(rom, vec![]);

        // Test lower 8 bits
        mbc.write_rom(0x2000, 0xFF);
        assert_eq!(mbc.rom_bank, 0xFF);
        assert_eq!(mbc.read_rom(0x4000), 0xFF);

        // Test bit 8
        mbc.write_rom(0x3000, 0x01);
        assert_eq!(mbc.rom_bank, 0x1FF); // Bank 511
        assert_eq!(mbc.read_rom(0x4000), 0xFF); // (511 & 0xFF) = 0xFF

        // Set to bank 256 (bit 8 = 1, lower bits = 0)
        mbc.write_rom(0x2000, 0x00);
        mbc.write_rom(0x3000, 0x01);
        assert_eq!(mbc.rom_bank, 0x100); // Bank 256
        assert_eq!(mbc.read_rom(0x4000), 0x00); // (256 & 0xFF) = 0

        // Set to bank 300
        mbc.write_rom(0x2000, 0x2C); // 44
        mbc.write_rom(0x3000, 0x01);
        assert_eq!(mbc.rom_bank, 0x12C); // Bank 300
        assert_eq!(mbc.read_rom(0x4000), 0x2C); // (300 & 0xFF) = 44
    }

    #[test]
    fn test_mbc5_ram_enable() {
        let mbc = Mbc5::new(vec![0; 0x8000], vec![0; 0x2000]);
        assert!(!mbc.ram_enabled);

        let mut mbc = mbc;
        mbc.write_rom(0x0000, 0x0A);
        assert!(mbc.ram_enabled);

        mbc.write_rom(0x0000, 0x00);
        assert!(!mbc.ram_enabled);
    }

    #[test]
    fn test_mbc5_ram_read_write() {
        let mut mbc = Mbc5::new(vec![0; 0x8000], vec![0; 0x2000]);

        // RAM disabled
        assert_eq!(mbc.read_ram(0xA000), 0xFF);

        // Enable RAM
        mbc.write_rom(0x0000, 0x0A);

        // Write and read
        mbc.write_ram(0xA000, 0x42);
        assert_eq!(mbc.read_ram(0xA000), 0x42);
    }

    #[test]
    fn test_mbc5_ram_banking() {
        let mut ram = vec![0; 0x20000]; // 128KB (16 banks)
        for bank in 0..16 {
            ram[bank * 0x2000] = bank as u8;
        }

        let mut mbc = Mbc5::new(vec![0; 0x8000], ram);
        mbc.write_rom(0x0000, 0x0A); // Enable RAM

        // Test all 16 banks
        for bank in 0u8..16 {
            mbc.write_rom(0x4000, bank);
            assert_eq!(mbc.read_ram(0xA000), bank);
        }
    }

    #[test]
    fn test_mbc5_ram_banking_wrapping() {
        let mut ram = vec![0; 0x2000]; // 8KB (1 bank)
        ram[0] = 0xAA;

        let mut mbc = Mbc5::new(vec![0; 0x8000], ram);
        mbc.write_rom(0x0000, 0x0A); // Enable RAM

        // Bank 0
        mbc.write_rom(0x4000, 0);
        assert_eq!(mbc.read_ram(0xA000), 0xAA);

        // Bank 1 should wrap to bank 0
        mbc.write_rom(0x4000, 1);
        assert_eq!(mbc.read_ram(0xA000), 0xAA);
    }

    #[test]
    fn test_mbc5_no_ram() {
        let mut mbc = Mbc5::new(vec![0; 0x8000], vec![]);

        mbc.write_rom(0x0000, 0x0A); // Enable RAM

        assert_eq!(mbc.read_ram(0xA000), 0xFF);
        mbc.write_ram(0xA000, 0x42);
        assert_eq!(mbc.read_ram(0xA000), 0xFF);
    }

    #[test]
    fn test_mbc5_large_rom() {
        let mut rom = vec![0; 0x800000]; // 8MB (512 banks, maximum for MBC5)
                                         // Mark first byte of each bank
        for bank in 0..512 {
            rom[bank * 0x4000] = (bank & 0xFF) as u8;
        }

        let mut mbc = Mbc5::new(rom, vec![]);

        // Test bank 0
        mbc.write_rom(0x2000, 0);
        mbc.write_rom(0x3000, 0);
        assert_eq!(mbc.read_rom(0x4000), 0);

        // Test bank 255
        mbc.write_rom(0x2000, 255);
        mbc.write_rom(0x3000, 0);
        assert_eq!(mbc.read_rom(0x4000), 255);

        // Test bank 256
        mbc.write_rom(0x2000, 0);
        mbc.write_rom(0x3000, 1);
        assert_eq!(mbc.read_rom(0x4000), 0);

        // Test bank 511
        mbc.write_rom(0x2000, 255);
        mbc.write_rom(0x3000, 1);
        assert_eq!(mbc.read_rom(0x4000), 255);
    }
}
