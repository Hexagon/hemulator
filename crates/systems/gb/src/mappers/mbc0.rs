//! MBC0 (No Mapper) - Basic ROM with no banking
//!
//! This is the simplest "mapper" - just a plain ROM with no banking capability.
//! Used by early Game Boy games that fit in 32KB or less.

/// MBC0 mapper - no banking, direct ROM access
#[derive(Debug)]
pub struct Mbc0 {
    rom: Vec<u8>,
    ram: Vec<u8>,
}

impl Mbc0 {
    pub fn new(rom: Vec<u8>, ram: Vec<u8>) -> Self {
        Self { rom, ram }
    }

    pub fn read_rom(&self, addr: u16) -> u8 {
        let offset = addr as usize;
        if offset < self.rom.len() {
            self.rom[offset]
        } else {
            0xFF
        }
    }

    pub fn write_rom(&mut self, _addr: u16, _val: u8) {
        // No banking commands for MBC0
    }

    pub fn read_ram(&self, addr: u16) -> u8 {
        let offset = (addr - 0xA000) as usize;
        if offset < self.ram.len() {
            self.ram[offset]
        } else {
            0xFF
        }
    }

    pub fn write_ram(&mut self, addr: u16, val: u8) {
        let offset = (addr - 0xA000) as usize;
        if offset < self.ram.len() {
            self.ram[offset] = val;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mbc0_rom_read() {
        let mut rom = vec![0; 0x8000]; // 32KB ROM
        rom[0x0000] = 0x12;
        rom[0x4000] = 0x34;

        let mbc = Mbc0::new(rom, vec![]);

        assert_eq!(mbc.read_rom(0x0000), 0x12);
        assert_eq!(mbc.read_rom(0x4000), 0x34);
    }

    #[test]
    fn test_mbc0_ram_read_write() {
        let mbc = Mbc0::new(vec![0; 0x8000], vec![0; 0x2000]); // 8KB RAM

        // Read uninitialized RAM
        assert_eq!(mbc.read_ram(0xA000), 0x00);

        let mut mbc = mbc;
        // Write and read back
        mbc.write_ram(0xA000, 0x42);
        assert_eq!(mbc.read_ram(0xA000), 0x42);
    }

    #[test]
    fn test_mbc0_no_ram() {
        let mbc = Mbc0::new(vec![0; 0x8000], vec![]);

        // Reading from non-existent RAM should return 0xFF
        assert_eq!(mbc.read_ram(0xA000), 0xFF);

        let mut mbc = mbc;
        // Writing should be ignored
        mbc.write_ram(0xA000, 0x42);
        assert_eq!(mbc.read_ram(0xA000), 0xFF);
    }

    #[test]
    fn test_mbc0_write_rom_ignored() {
        let mut rom = vec![0; 0x8000];
        rom[0] = 0xAA;
        let mut mbc = Mbc0::new(rom, vec![]);

        // Writing to ROM should be ignored
        mbc.write_rom(0x0000, 0xFF);
        assert_eq!(mbc.read_rom(0x0000), 0xAA);
    }
}
