//! Game Boy Memory Bank Controllers (MBCs)
//!
//! This module contains implementations of various Game Boy cartridge mappers
//! that handle ROM/RAM banking and other cartridge hardware features.

mod huc1;
mod mbc0;
mod mbc1;
mod mbc2;
mod mbc3;
mod mbc5;

pub use huc1::Huc1;
pub use mbc0::Mbc0;
pub use mbc1::Mbc1;
pub use mbc2::Mbc2;
pub use mbc3::Mbc3;
pub use mbc5::Mbc5;

/// Unified mapper enum that dispatches to specific implementations
#[derive(Debug)]
pub enum Mapper {
    Huc1(Huc1),
    Mbc0(Mbc0),
    Mbc1(Mbc1),
    Mbc2(Mbc2),
    Mbc3(Mbc3),
    Mbc5(Mbc5),
}

impl Mapper {
    /// Create a mapper from ROM data and cartridge type
    pub fn from_cart(rom: Vec<u8>, ram: Vec<u8>, cart_type: u8) -> Self {
        match cart_type {
            0x00 => Mapper::Mbc0(Mbc0::new(rom, ram)), // ROM ONLY
            0x01 => Mapper::Mbc1(Mbc1::new(rom, ram)), // MBC1
            0x02 => Mapper::Mbc1(Mbc1::new(rom, ram)), // MBC1+RAM
            0x03 => Mapper::Mbc1(Mbc1::new(rom, ram)), // MBC1+RAM+BATTERY
            0x05 => Mapper::Mbc2(Mbc2::new(rom, ram)), // MBC2
            0x06 => Mapper::Mbc2(Mbc2::new(rom, ram)), // MBC2+BATTERY
            0x0F => Mapper::Mbc3(Mbc3::new(rom, ram)), // MBC3+TIMER+BATTERY
            0x10 => Mapper::Mbc3(Mbc3::new(rom, ram)), // MBC3+TIMER+RAM+BATTERY
            0x11 => Mapper::Mbc3(Mbc3::new(rom, ram)), // MBC3
            0x12 => Mapper::Mbc3(Mbc3::new(rom, ram)), // MBC3+RAM
            0x13 => Mapper::Mbc3(Mbc3::new(rom, ram)), // MBC3+RAM+BATTERY
            0x19 => Mapper::Mbc5(Mbc5::new(rom, ram)), // MBC5
            0x1A => Mapper::Mbc5(Mbc5::new(rom, ram)), // MBC5+RAM
            0x1B => Mapper::Mbc5(Mbc5::new(rom, ram)), // MBC5+RAM+BATTERY
            0x1C => Mapper::Mbc5(Mbc5::new(rom, ram)), // MBC5+RUMBLE
            0x1D => Mapper::Mbc5(Mbc5::new(rom, ram)), // MBC5+RUMBLE+RAM
            0x1E => Mapper::Mbc5(Mbc5::new(rom, ram)), // MBC5+RUMBLE+RAM+BATTERY
            0xFF => Mapper::Huc1(Huc1::new(rom, ram)), // HuC1
            _ => Mapper::Mbc0(Mbc0::new(rom, ram)),    // Default to MBC0
        }
    }

    /// Read from ROM address space
    pub fn read_rom(&self, addr: u16) -> u8 {
        match self {
            Mapper::Huc1(m) => m.read_rom(addr),
            Mapper::Mbc0(m) => m.read_rom(addr),
            Mapper::Mbc1(m) => m.read_rom(addr),
            Mapper::Mbc2(m) => m.read_rom(addr),
            Mapper::Mbc3(m) => m.read_rom(addr),
            Mapper::Mbc5(m) => m.read_rom(addr),
        }
    }

    /// Write to ROM address space (for mapper registers)
    pub fn write_rom(&mut self, addr: u16, val: u8) {
        match self {
            Mapper::Huc1(m) => m.write_rom(addr, val),
            Mapper::Mbc0(m) => m.write_rom(addr, val),
            Mapper::Mbc1(m) => m.write_rom(addr, val),
            Mapper::Mbc2(m) => m.write_rom(addr, val),
            Mapper::Mbc3(m) => m.write_rom(addr, val),
            Mapper::Mbc5(m) => m.write_rom(addr, val),
        }
    }

    /// Read from RAM address space
    pub fn read_ram(&self, addr: u16) -> u8 {
        match self {
            Mapper::Huc1(m) => m.read_ram(addr),
            Mapper::Mbc0(m) => m.read_ram(addr),
            Mapper::Mbc1(m) => m.read_ram(addr),
            Mapper::Mbc2(m) => m.read_ram(addr),
            Mapper::Mbc3(m) => m.read_ram(addr),
            Mapper::Mbc5(m) => m.read_ram(addr),
        }
    }

    /// Write to RAM address space
    pub fn write_ram(&mut self, addr: u16, val: u8) {
        match self {
            Mapper::Huc1(m) => m.write_ram(addr, val),
            Mapper::Mbc0(m) => m.write_ram(addr, val),
            Mapper::Mbc1(m) => m.write_ram(addr, val),
            Mapper::Mbc2(m) => m.write_ram(addr, val),
            Mapper::Mbc3(m) => m.write_ram(addr, val),
            Mapper::Mbc5(m) => m.write_ram(addr, val),
        }
    }

    /// Get the cartridge type name
    #[cfg(test)]
    pub fn name(&self) -> &str {
        match self {
            Mapper::Huc1(_) => "HuC1",
            Mapper::Mbc0(_) => "MBC0",
            Mapper::Mbc1(_) => "MBC1",
            Mapper::Mbc2(_) => "MBC2",
            Mapper::Mbc3(_) => "MBC3",
            Mapper::Mbc5(_) => "MBC5",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mapper_from_cart_type() {
        // MBC0
        let mapper = Mapper::from_cart(vec![0; 0x8000], vec![], 0x00);
        assert_eq!(mapper.name(), "MBC0");

        // MBC1
        let mapper = Mapper::from_cart(vec![0; 0x8000], vec![], 0x01);
        assert_eq!(mapper.name(), "MBC1");

        let mapper = Mapper::from_cart(vec![0; 0x8000], vec![], 0x02);
        assert_eq!(mapper.name(), "MBC1");

        let mapper = Mapper::from_cart(vec![0; 0x8000], vec![], 0x03);
        assert_eq!(mapper.name(), "MBC1");

        // MBC2
        let mapper = Mapper::from_cart(vec![0; 0x8000], vec![], 0x05);
        assert_eq!(mapper.name(), "MBC2");

        let mapper = Mapper::from_cart(vec![0; 0x8000], vec![], 0x06);
        assert_eq!(mapper.name(), "MBC2");

        // MBC3
        let mapper = Mapper::from_cart(vec![0; 0x8000], vec![], 0x0F);
        assert_eq!(mapper.name(), "MBC3");

        let mapper = Mapper::from_cart(vec![0; 0x8000], vec![], 0x11);
        assert_eq!(mapper.name(), "MBC3");

        let mapper = Mapper::from_cart(vec![0; 0x8000], vec![], 0x13);
        assert_eq!(mapper.name(), "MBC3");

        // MBC5
        let mapper = Mapper::from_cart(vec![0; 0x8000], vec![], 0x19);
        assert_eq!(mapper.name(), "MBC5");

        let mapper = Mapper::from_cart(vec![0; 0x8000], vec![], 0x1B);
        assert_eq!(mapper.name(), "MBC5");

        // HuC1
        let mapper = Mapper::from_cart(vec![0; 0x8000], vec![], 0xFF);
        assert_eq!(mapper.name(), "HuC1");

        // Unknown type defaults to MBC0
        let mapper = Mapper::from_cart(vec![0; 0x8000], vec![], 0xAA);
        assert_eq!(mapper.name(), "MBC0");
    }

    #[test]
    fn test_mapper_delegation() {
        let mut rom = vec![0; 0x8000];
        rom[0] = 0xAA;
        rom[0x4000] = 0xBB;

        let mapper = Mapper::from_cart(rom, vec![], 0x00);

        // Test ROM reads
        assert_eq!(mapper.read_rom(0x0000), 0xAA);
        assert_eq!(mapper.read_rom(0x4000), 0xBB);
    }
}
