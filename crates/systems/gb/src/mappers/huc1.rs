//! HuC1 (Hudson Soft Mapper)
//!
//! A mapper used by Hudson Soft for a few Game Boy games.
//! Used by approximately <1% of Game Boy games.
//!
//! Supports up to 1MB ROM (64 banks) and 32KB RAM (4 banks).
//!
//! # Key Features
//!
//! - ROM banking from 1-63 (bank 0 not selectable, maps to bank 1)
//! - RAM banking (4 banks of 8KB)
//! - IR (infrared) mode support (stubbed - not commonly used)
//!
//! # Memory Map
//!
//! - 0x0000-0x3FFF: ROM Bank 0 (fixed)
//! - 0x4000-0x7FFF: ROM Bank 1-63 (switchable)
//! - 0xA000-0xBFFF: RAM Bank 0-3 (switchable, if enabled)
//!
//! # Register Map (0x0000-0x7FFF)
//!
//! - 0x0000-0x1FFF: RAM Enable
//!   - Write 0x0A to enable RAM, anything else to disable
//! - 0x2000-0x3FFF: ROM Bank Select
//!   - 6-bit value (0-63)
//!   - Bank 0 cannot be selected (automatically maps to bank 1)
//! - 0x4000-0x5FFF: RAM Bank Select / IR Mode
//!   - In ROM mode: 2-bit RAM bank number (0-3)
//!   - In IR mode: IR register (stubbed)
//! - 0x6000-0x7FFF: Mode Select
//!   - 0 = ROM banking mode
//!   - 1 = IR mode (infrared sensor, rarely used)

/// HuC1 mapper
#[derive(Debug)]
pub struct Huc1 {
    rom: Vec<u8>,
    ram: Vec<u8>,
    ram_enabled: bool,
    rom_bank: u8,  // 6-bit register (1-63, bank 0 maps to 1)
    ram_bank: u8,  // 2-bit register (0-3)
    ir_mode: bool, // false = ROM mode, true = IR mode
}

impl Huc1 {
    pub fn new(rom: Vec<u8>, ram: Vec<u8>) -> Self {
        Self {
            rom,
            ram,
            ram_enabled: false,
            rom_bank: 1, // Default to bank 1
            ram_bank: 0,
            ir_mode: false,
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
            // Use current ROM bank, wrapping to available banks
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
            0x2000..=0x3FFF => {
                // ROM Bank Select (6 bits)
                let bank = val & 0x3F;
                // Bank 0 is not selectable, map to bank 1
                self.rom_bank = if bank == 0 { 1 } else { bank };
            }
            0x4000..=0x5FFF => {
                if self.ir_mode {
                    // IR mode register - stubbed (not commonly used)
                    // Games that use this are very rare
                } else {
                    // RAM Bank Select (2 bits)
                    self.ram_bank = val & 0x03;
                }
            }
            0x6000..=0x7FFF => {
                // Mode Select: 0 = ROM mode, 1 = IR mode
                self.ir_mode = (val & 0x01) != 0;
            }
            _ => {}
        }
    }

    pub fn read_ram(&self, addr: u16) -> u8 {
        if !self.ram_enabled || self.ram.is_empty() {
            return 0xFF;
        }

        if self.ir_mode {
            // IR mode - return 0xC0 (no IR signal detected)
            // This is a stub for the infrared sensor feature
            return 0xC0;
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

        if self.ir_mode {
            // IR mode - writes are ignored
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
    fn test_huc1_creation() {
        let rom = vec![0; 0x8000]; // 32KB ROM (2 banks)
        let ram = vec![0; 0x2000]; // 8KB RAM (1 bank)
        let mapper = Huc1::new(rom, ram);

        assert_eq!(mapper.rom_bank, 1);
        assert_eq!(mapper.ram_bank, 0);
        assert!(!mapper.ram_enabled);
        assert!(!mapper.ir_mode);
    }

    #[test]
    fn test_huc1_rom_banking() {
        let mut rom = vec![0; 0x10000]; // 64KB ROM (4 banks)
                                        // Mark each bank with a unique pattern
        rom[0x0000] = 0x00; // Bank 0
        rom[0x4000] = 0x01; // Bank 1
        rom[0x8000] = 0x02; // Bank 2
        rom[0xC000] = 0x03; // Bank 3

        let mut mapper = Huc1::new(rom, vec![]);

        // Bank 0 is always at 0x0000-0x3FFF
        assert_eq!(mapper.read_rom(0x0000), 0x00);

        // Default bank 1 at 0x4000-0x7FFF
        assert_eq!(mapper.read_rom(0x4000), 0x01);

        // Switch to bank 2
        mapper.write_rom(0x2000, 0x02);
        assert_eq!(mapper.read_rom(0x4000), 0x02);

        // Switch to bank 3
        mapper.write_rom(0x2000, 0x03);
        assert_eq!(mapper.read_rom(0x4000), 0x03);
    }

    #[test]
    fn test_huc1_bank_zero_maps_to_one() {
        let mut rom = vec![0; 0x8000]; // 32KB ROM
        rom[0x4000] = 0xAA; // Bank 1

        let mut mapper = Huc1::new(rom, vec![]);

        // Try to select bank 0 - should map to bank 1
        mapper.write_rom(0x2000, 0x00);
        assert_eq!(mapper.rom_bank, 1);
        assert_eq!(mapper.read_rom(0x4000), 0xAA);
    }

    #[test]
    fn test_huc1_ram_enable() {
        let rom = vec![0; 0x8000];
        let ram = vec![0x00; 0x2000];
        let mut mapper = Huc1::new(rom, ram);

        // RAM disabled by default
        assert!(!mapper.ram_enabled);
        assert_eq!(mapper.read_ram(0xA000), 0xFF);

        // Enable RAM
        mapper.write_rom(0x0000, 0x0A);
        assert!(mapper.ram_enabled);

        // Disable RAM
        mapper.write_rom(0x0000, 0x00);
        assert!(!mapper.ram_enabled);
    }

    #[test]
    fn test_huc1_ram_banking() {
        let rom = vec![0; 0x8000];
        let mut ram = vec![0; 0x8000]; // 32KB RAM (4 banks)

        // Mark each bank with a unique pattern
        ram[0x0000] = 0x00; // Bank 0
        ram[0x2000] = 0x01; // Bank 1
        ram[0x4000] = 0x02; // Bank 2
        ram[0x6000] = 0x03; // Bank 3

        let mut mapper = Huc1::new(rom, ram);
        mapper.write_rom(0x0000, 0x0A); // Enable RAM

        // Default bank 0
        assert_eq!(mapper.read_ram(0xA000), 0x00);

        // Switch to bank 1
        mapper.write_rom(0x4000, 0x01);
        assert_eq!(mapper.read_ram(0xA000), 0x01);

        // Switch to bank 2
        mapper.write_rom(0x4000, 0x02);
        assert_eq!(mapper.read_ram(0xA000), 0x02);

        // Switch to bank 3
        mapper.write_rom(0x4000, 0x03);
        assert_eq!(mapper.read_ram(0xA000), 0x03);
    }

    #[test]
    fn test_huc1_ram_read_write() {
        let rom = vec![0; 0x8000];
        let ram = vec![0; 0x2000];
        let mut mapper = Huc1::new(rom, ram);

        // Enable RAM
        mapper.write_rom(0x0000, 0x0A);

        // Write and read
        mapper.write_ram(0xA000, 0x42);
        assert_eq!(mapper.read_ram(0xA000), 0x42);

        mapper.write_ram(0xBFFF, 0x99);
        assert_eq!(mapper.read_ram(0xBFFF), 0x99);
    }

    #[test]
    fn test_huc1_ir_mode() {
        let rom = vec![0; 0x8000];
        let mut ram = vec![0; 0x2000];
        ram[0] = 0x42; // Put data in RAM bank 0

        let mut mapper = Huc1::new(rom, ram);
        mapper.write_rom(0x0000, 0x0A); // Enable RAM

        // ROM mode (default) - can read/write RAM
        assert!(!mapper.ir_mode);
        assert_eq!(mapper.read_ram(0xA000), 0x42);
        mapper.write_ram(0xA001, 0x99);
        assert_eq!(mapper.read_ram(0xA001), 0x99);

        // Switch to IR mode
        mapper.write_rom(0x6000, 0x01);
        assert!(mapper.ir_mode);

        // In IR mode, reads return IR sensor value (0xC0 = no signal)
        assert_eq!(mapper.read_ram(0xA000), 0xC0);

        // Writes in IR mode are ignored
        mapper.write_ram(0xA000, 0xFF);
        // Switch back to ROM mode to verify write was ignored
        mapper.write_rom(0x6000, 0x00);
        assert_eq!(mapper.read_ram(0xA000), 0x42); // Original value unchanged
    }

    #[test]
    fn test_huc1_no_ram() {
        let rom = vec![0; 0x8000];
        let mut mapper = Huc1::new(rom, vec![]); // No RAM

        // Enable RAM (but there is none)
        mapper.write_rom(0x0000, 0x0A);

        // Reads return 0xFF
        assert_eq!(mapper.read_ram(0xA000), 0xFF);

        // Writes are ignored (no crash)
        mapper.write_ram(0xA000, 0x42);
    }

    #[test]
    fn test_huc1_rom_bank_wrapping() {
        let rom = vec![0; 0x4000]; // 16KB ROM (1 bank)
        let mut mapper = Huc1::new(rom, vec![]);

        // Try to select bank 63 (should wrap to available banks)
        mapper.write_rom(0x2000, 0x3F);
        // Should not crash, wraps using modulo
        let _ = mapper.read_rom(0x4000);
    }

    #[test]
    fn test_huc1_ram_bank_wrapping() {
        let rom = vec![0; 0x8000];
        let ram = vec![0; 0x2000]; // 8KB RAM (1 bank)
        let mut mapper = Huc1::new(rom, ram);
        mapper.write_rom(0x0000, 0x0A); // Enable RAM

        // Try to select bank 3 (should wrap to available banks)
        mapper.write_rom(0x4000, 0x03);
        // Should not crash, wraps using modulo
        let _ = mapper.read_ram(0xA000);
    }
}
