//! MBC2 (Memory Bank Controller 2)
//!
//! A unique Game Boy mapper with built-in RAM.
//! Used by approximately 1% of Game Boy games.
//!
//! Supports up to 256KB ROM (16 banks) with 512×4 bits of built-in RAM.
//!
//! # Key Features
//!
//! - Built-in 512×4 bits RAM (only lower 4 bits of each byte are usable)
//! - No external RAM support
//! - Address bit 8 determines register function
//! - ROM banking from 1-15 (bank 0 not selectable at 0x4000)
//!
//! # Memory Map
//!
//! - 0x0000-0x3FFF: ROM Bank 0 (fixed)
//! - 0x4000-0x7FFF: ROM Bank 1-15 (switchable)
//! - 0xA000-0xA1FF: Built-in RAM (512 bytes, only lower 4 bits used per byte)
//!   - Mirrored throughout 0xA000-0xBFFF
//!
//! # Register Map (0x0000-0x3FFF)
//!
//! The function depends on bit 8 of the address:
//! - If bit 8 = 0 (e.g., 0x0000, 0x2000): RAM Enable
//!   - Write 0x0A to enable, anything else to disable
//! - If bit 8 = 1 (e.g., 0x0100, 0x2100): ROM Bank Select
//!   - Lower 4 bits select bank 0-15
//!   - Bank 0 cannot be selected (automatically maps to bank 1)

/// MBC2 mapper with built-in RAM
#[derive(Debug)]
pub struct Mbc2 {
    rom: Vec<u8>,
    ram: Vec<u8>,      // 512 bytes of built-in RAM
    ram_enabled: bool,
    rom_bank: u8,      // 4-bit register (1-15, bank 0 maps to 1)
}

impl Mbc2 {
    pub fn new(rom: Vec<u8>, _ram: Vec<u8>) -> Self {
        // MBC2 always has 512 bytes of built-in RAM, ignore external RAM
        Self {
            rom,
            ram: vec![0; 512], // Built-in 512×4 bits RAM
            ram_enabled: false,
            rom_bank: 1, // Default to bank 1
        }
    }

    fn rom_bank_count(&self) -> usize {
        self.rom.len().div_ceil(0x4000)
    }

    pub fn read_rom(&self, addr: u16) -> u8 {
        let bank = if addr < 0x4000 {
            0 // Bank 0 is fixed at 0x0000-0x3FFF
        } else {
            // Bank 1-15 at 0x4000-0x7FFF
            let bank = if self.rom_bank == 0 { 1 } else { self.rom_bank };
            (bank as usize) % self.rom_bank_count().max(1)
        };

        let offset = (bank * 0x4000) + ((addr & 0x3FFF) as usize);
        if offset < self.rom.len() {
            self.rom[offset]
        } else {
            0xFF
        }
    }

    pub fn write_rom(&mut self, addr: u16, val: u8) {
        // Address bit 8 determines the function
        if addr & 0x0100 == 0 {
            // Bit 8 = 0: RAM Enable (addresses like 0x0000, 0x2000, etc.)
            self.ram_enabled = (val & 0x0F) == 0x0A;
        } else {
            // Bit 8 = 1: ROM Bank Select (addresses like 0x0100, 0x2100, etc.)
            self.rom_bank = val & 0x0F; // Only lower 4 bits
        }
    }

    pub fn read_ram(&self, addr: u16) -> u8 {
        if !self.ram_enabled {
            return 0xFF;
        }

        // RAM is 512 bytes (0xA000-0xA1FF), mirrored throughout 0xA000-0xBFFF
        let offset = ((addr - 0xA000) & 0x01FF) as usize;
        
        // Only lower 4 bits are usable, upper 4 bits read as 1
        self.ram[offset] | 0xF0
    }

    pub fn write_ram(&mut self, addr: u16, val: u8) {
        if !self.ram_enabled {
            return;
        }

        // RAM is 512 bytes (0xA000-0xA1FF), mirrored throughout 0xA000-0xBFFF
        let offset = ((addr - 0xA000) & 0x01FF) as usize;
        
        // Only lower 4 bits are writable, upper 4 bits are ignored
        self.ram[offset] = val & 0x0F;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mbc2_creation() {
        let rom = vec![0; 0x8000]; // 32KB ROM
        let mbc = Mbc2::new(rom, vec![]); // External RAM is ignored
        
        assert_eq!(mbc.ram.len(), 512); // Built-in RAM is always 512 bytes
        assert!(!mbc.ram_enabled);
        assert_eq!(mbc.rom_bank, 1);
    }

    #[test]
    fn test_mbc2_default_banks() {
        let rom = vec![0xFF; 0x40000]; // 256KB (16 banks)
        let mbc = Mbc2::new(rom, vec![]);

        // Bank 0 at 0x0000-0x3FFF
        assert_eq!(mbc.read_rom(0x0000), 0xFF);
        // Bank 1 at 0x4000-0x7FFF (default)
        assert_eq!(mbc.read_rom(0x4000), 0xFF);
    }

    #[test]
    fn test_mbc2_rom_banking() {
        let mut rom = vec![0; 0x40000]; // 256KB (16 banks)
        // Mark each bank with its number
        for bank in 0..16 {
            rom[bank * 0x4000] = bank as u8;
        }

        let mut mbc = Mbc2::new(rom, vec![]);

        // Default: bank 0 at lower, bank 1 at upper
        assert_eq!(mbc.read_rom(0x0000), 0);
        assert_eq!(mbc.read_rom(0x4000), 1);

        // Switch to bank 5 (bit 8 of address must be 1)
        mbc.write_rom(0x0100, 5); // 0x0100 has bit 8 set
        assert_eq!(mbc.read_rom(0x4000), 5);

        // Switch to bank 15
        mbc.write_rom(0x2100, 15); // 0x2100 has bit 8 set
        assert_eq!(mbc.read_rom(0x4000), 15);

        // Bank 0 is not selectable, should map to bank 1
        mbc.write_rom(0x0100, 0);
        assert_eq!(mbc.read_rom(0x4000), 1);
    }

    #[test]
    fn test_mbc2_address_bit8_rom_banking() {
        let mut rom = vec![0; 0x40000]; // 256KB (16 banks)
        for bank in 0..16 {
            rom[bank * 0x4000] = bank as u8;
        }

        let mut mbc = Mbc2::new(rom, vec![]);

        // Bit 8 = 1: Should change ROM bank
        mbc.write_rom(0x0100, 7);
        assert_eq!(mbc.read_rom(0x4000), 7);

        mbc.write_rom(0x2100, 3);
        assert_eq!(mbc.read_rom(0x4000), 3);

        mbc.write_rom(0x3F00, 9);
        assert_eq!(mbc.read_rom(0x4000), 9);

        // Bit 8 = 0: Should NOT change ROM bank (RAM enable instead)
        mbc.write_rom(0x0000, 12); // Should not change bank
        assert_eq!(mbc.read_rom(0x4000), 9); // Still bank 9

        mbc.write_rom(0x2000, 6); // Should not change bank
        assert_eq!(mbc.read_rom(0x4000), 9); // Still bank 9
    }

    #[test]
    fn test_mbc2_ram_enable() {
        let mbc = Mbc2::new(vec![0; 0x8000], vec![]);
        assert!(!mbc.ram_enabled);

        let mut mbc = mbc;
        
        // Enable RAM (bit 8 = 0)
        mbc.write_rom(0x0000, 0x0A);
        assert!(mbc.ram_enabled);

        // Disable RAM (bit 8 = 0)
        mbc.write_rom(0x2000, 0x00);
        assert!(!mbc.ram_enabled);

        // Bit 8 = 1 should NOT affect RAM enable
        mbc.write_rom(0x0100, 0x0A);
        assert!(!mbc.ram_enabled); // Still disabled
    }

    #[test]
    fn test_mbc2_ram_4bit() {
        let mut mbc = Mbc2::new(vec![0; 0x8000], vec![]);
        
        // Enable RAM
        mbc.write_rom(0x0000, 0x0A);

        // Write full byte (0xAB)
        mbc.write_ram(0xA000, 0xAB);
        
        // Only lower 4 bits stored, upper 4 bits read as 1
        assert_eq!(mbc.read_ram(0xA000), 0xFB); // 0xF0 | 0x0B

        // Write another value
        mbc.write_ram(0xA001, 0x37);
        assert_eq!(mbc.read_ram(0xA001), 0xF7); // 0xF0 | 0x07
    }

    #[test]
    fn test_mbc2_ram_mirroring() {
        let mut mbc = Mbc2::new(vec![0; 0x8000], vec![]);
        mbc.write_rom(0x0000, 0x0A); // Enable RAM

        // Write to 0xA000
        mbc.write_ram(0xA000, 0x05);

        // Read from mirrored addresses (512-byte mirror)
        assert_eq!(mbc.read_ram(0xA000), 0xF5);
        assert_eq!(mbc.read_ram(0xA200), 0xF5); // +0x200
        assert_eq!(mbc.read_ram(0xA400), 0xF5); // +0x400
        assert_eq!(mbc.read_ram(0xA600), 0xF5); // +0x600
        assert_eq!(mbc.read_ram(0xA800), 0xF5); // +0x800
        assert_eq!(mbc.read_ram(0xAA00), 0xF5); // +0xA00
        assert_eq!(mbc.read_ram(0xAC00), 0xF5); // +0xC00
        assert_eq!(mbc.read_ram(0xAE00), 0xF5); // +0xE00
        assert_eq!(mbc.read_ram(0xB000), 0xF5); // 0xB000
        
        // 0xBFFF wraps to offset 0x1FF (last byte of 512-byte range)
        // which should be uninitialized (0xF0)
        assert_eq!(mbc.read_ram(0xBFFF), 0xF0);

        // Write to different offset within first 512 bytes
        mbc.write_ram(0xA100, 0x0C);
        assert_eq!(mbc.read_ram(0xA100), 0xFC);
        assert_eq!(mbc.read_ram(0xA300), 0xFC); // Mirrored
        assert_eq!(mbc.read_ram(0xB100), 0xFC); // Mirrored
    }

    #[test]
    fn test_mbc2_ram_disabled() {
        let mut mbc = Mbc2::new(vec![0; 0x8000], vec![]);
        
        // RAM disabled by default
        assert_eq!(mbc.read_ram(0xA000), 0xFF);

        // Writes are ignored when disabled
        mbc.write_ram(0xA000, 0x05);
        assert_eq!(mbc.read_ram(0xA000), 0xFF);

        // Enable RAM
        mbc.write_rom(0x0000, 0x0A);
        mbc.write_ram(0xA000, 0x05);
        assert_eq!(mbc.read_ram(0xA000), 0xF5);

        // Disable RAM
        mbc.write_rom(0x2000, 0x00);
        assert_eq!(mbc.read_ram(0xA000), 0xFF); // Returns 0xFF when disabled
    }

    #[test]
    fn test_mbc2_rom_bank_mask() {
        let mut rom = vec![0; 0x40000]; // 256KB (16 banks)
        for bank in 0..16 {
            rom[bank * 0x4000] = bank as u8;
        }

        let mut mbc = Mbc2::new(rom, vec![]);

        // Write value with upper bits set (should be masked to 4 bits)
        mbc.write_rom(0x0100, 0xF5); // 0xF5 & 0x0F = 0x05
        assert_eq!(mbc.read_rom(0x4000), 5);

        mbc.write_rom(0x0100, 0x8E); // 0x8E & 0x0F = 0x0E
        assert_eq!(mbc.read_rom(0x4000), 14);
    }

    #[test]
    fn test_mbc2_bank_wrapping() {
        let rom = vec![0xFF; 0x20000]; // 128KB (8 banks)
        let mbc = Mbc2::new(rom, vec![]);

        // ROM has only 8 banks
        assert!(mbc.rom_bank_count() == 8);
    }

    #[test]
    fn test_mbc2_ram_boundary() {
        let mut mbc = Mbc2::new(vec![0; 0x8000], vec![]);
        mbc.write_rom(0x0000, 0x0A); // Enable RAM

        // Write to last byte of RAM
        mbc.write_ram(0xA1FF, 0x0A);
        assert_eq!(mbc.read_ram(0xA1FF), 0xFA);

        // Verify it mirrors
        assert_eq!(mbc.read_ram(0xA3FF), 0xFA);
        assert_eq!(mbc.read_ram(0xBFFF), 0xFA);
    }
}
