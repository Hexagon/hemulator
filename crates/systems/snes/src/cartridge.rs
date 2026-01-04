//! SNES cartridge implementation

use crate::SnesError;
use emu_core::logging::{log, LogCategory, LogLevel};

/// ROM mapping mode
#[derive(Debug, Clone, Copy, PartialEq)]
enum MappingMode {
    LoROM,
    HiROM,
}

/// SNES cartridge
pub struct Cartridge {
    /// ROM data
    rom: Vec<u8>,
    /// RAM (if present)
    ram: Vec<u8>,
    /// Header offset (512 bytes if SMC header present)
    header_offset: usize,
    /// Mapping mode (LoROM or HiROM)
    mapping_mode: MappingMode,
}

impl Cartridge {
    pub fn load(data: &[u8]) -> Result<Self, SnesError> {
        if data.len() < 0x8000 {
            log(LogCategory::Bus, LogLevel::Error, || {
                format!(
                    "SNES Cartridge: ROM too small ({} bytes, minimum 32KB)",
                    data.len()
                )
            });
            return Err(SnesError::InvalidRom(
                "ROM too small (minimum 32KB)".to_string(),
            ));
        }

        // Check for SMC header (512 bytes)
        let header_offset = if data.len() % 1024 == 512 { 512 } else { 0 };

        let rom_data = &data[header_offset..];

        // Validate minimum ROM size
        if rom_data.len() < 0x8000 {
            log(LogCategory::Bus, LogLevel::Error, || {
                format!(
                    "SNES Cartridge: ROM data too small after header ({} bytes)",
                    rom_data.len()
                )
            });
            return Err(SnesError::InvalidRom(
                "ROM data too small after header".to_string(),
            ));
        }

        // Detect mapping mode from header
        // SNES ROM header is at $7FC0 (LoROM) or $FFC0 (HiROM)
        let mapping_mode = Self::detect_mapping_mode(rom_data);

        log(LogCategory::Bus, LogLevel::Info, || {
            format!(
                "SNES Cartridge: Loaded ROM - Size: {} KB, SMC Header: {}, Mapping: {:?}",
                rom_data.len() / 1024,
                if header_offset > 0 { "Yes" } else { "No" },
                mapping_mode
            )
        });

        Ok(Self {
            rom: rom_data.to_vec(),
            ram: vec![0; 0x8000], // 32KB SRAM (standard size)
            header_offset,
            mapping_mode,
        })
    }

    /// Detect mapping mode by checking ROM headers
    fn detect_mapping_mode(rom: &[u8]) -> MappingMode {
        // Try LoROM header at $7FC0 (offset in ROM)
        let lorom_header_offset = 0x7FC0;
        // Try HiROM header at $FFC0 (offset in ROM)
        let hirom_header_offset = 0xFFC0;

        let lorom_score = if lorom_header_offset < rom.len() {
            Self::score_header(rom, lorom_header_offset)
        } else {
            0
        };

        let hirom_score = if hirom_header_offset < rom.len() {
            Self::score_header(rom, hirom_header_offset)
        } else {
            0
        };

        // If HiROM score is higher, use HiROM, otherwise default to LoROM
        if hirom_score > lorom_score {
            MappingMode::HiROM
        } else {
            MappingMode::LoROM
        }
    }

    /// Score a potential header location (higher = more likely valid)
    fn score_header(rom: &[u8], offset: usize) -> u32 {
        if offset + 0x40 > rom.len() {
            return 0;
        }

        let mut score = 0u32;

        // Check mapper type byte at +$15 (should be reasonable value)
        let mapper_type = rom[offset + 0x15];
        if mapper_type < 0x08 {
            score += 2; // Valid mapper type
        }

        // Check ROM size byte at +$17 (should be 0x07-0x0D typically)
        let rom_size = rom[offset + 0x17];
        if (0x07..=0x0D).contains(&rom_size) {
            score += 2; // Reasonable ROM size
        }

        // Check checksum complement at +$1C-$1D and checksum at +$1E-$1F
        let checksum_comp = u16::from_le_bytes([rom[offset + 0x1C], rom[offset + 0x1D]]);
        let checksum = u16::from_le_bytes([rom[offset + 0x1E], rom[offset + 0x1F]]);
        if checksum_comp == !checksum {
            score += 4; // Valid checksum pair
        }

        // Check reset vector at +$3C-$3D (should be reasonable address)
        let reset_vector = u16::from_le_bytes([rom[offset + 0x3C], rom[offset + 0x3D]]);
        if reset_vector >= 0x8000 {
            score += 2; // Valid reset vector (in ROM area)
        }

        score
    }

    pub fn read(&self, addr: u32) -> u8 {
        match self.mapping_mode {
            MappingMode::LoROM => self.read_lorom(addr),
            MappingMode::HiROM => self.read_hirom(addr),
        }
    }

    fn read_lorom(&self, addr: u32) -> u8 {
        let bank = (addr >> 16) as u8;
        let offset = (addr & 0xFFFF) as u16;

        // LoROM mapping: $8000-$FFFF in each bank maps to 32KB chunks
        match bank {
            0x00..=0x7D => {
                if offset >= 0x8000 {
                    let rom_offset =
                        ((bank as usize) << 15) | ((offset as usize - 0x8000) & 0x7FFF);
                    *self.rom.get(rom_offset).unwrap_or(&0)
                } else if matches!(bank, 0x70..=0x7D) && offset < 0x8000 {
                    // SRAM in banks $70-$7D at $0000-$7FFF
                    *self.ram.get(offset as usize).unwrap_or(&0)
                } else {
                    0
                }
            }
            0x80..=0xFF => {
                if offset >= 0x8000 {
                    let rom_offset =
                        (((bank as usize) - 0x80) << 15) | ((offset as usize - 0x8000) & 0x7FFF);
                    *self.rom.get(rom_offset).unwrap_or(&0)
                } else if matches!(bank, 0xF0..=0xFF) && offset < 0x8000 {
                    // SRAM in banks $F0-$FF at $0000-$7FFF (mirror)
                    *self.ram.get(offset as usize).unwrap_or(&0)
                } else {
                    0
                }
            }
            _ => 0,
        }
    }

    fn read_hirom(&self, addr: u32) -> u8 {
        let bank = (addr >> 16) as u8;
        let offset = (addr & 0xFFFF) as u16;

        // HiROM mapping: Full 64KB per bank
        match bank {
            // Banks $00-$3F: SRAM at $6000-$7FFF, ROM at $8000-$FFFF
            0x00..=0x3F => {
                if (0x6000..0x8000).contains(&offset) {
                    // SRAM
                    let sram_offset = (offset - 0x6000) as usize;
                    *self.ram.get(sram_offset).unwrap_or(&0)
                } else if offset >= 0x8000 {
                    // ROM mirror
                    let rom_offset = ((bank as usize) << 16) | (offset as usize);
                    *self.rom.get(rom_offset).unwrap_or(&0)
                } else {
                    0
                }
            }
            // Banks $40-$7D: Full ROM access
            0x40..=0x7D => {
                let rom_offset = ((bank as usize) << 16) | (offset as usize);
                *self.rom.get(rom_offset).unwrap_or(&0)
            }
            // Banks $80-$BF: Mirror of $00-$3F
            0x80..=0xBF => {
                if (0x6000..0x8000).contains(&offset) {
                    // SRAM (mirror)
                    let sram_offset = (offset - 0x6000) as usize;
                    *self.ram.get(sram_offset).unwrap_or(&0)
                } else if offset >= 0x8000 {
                    // ROM mirror
                    let rom_offset = (((bank - 0x80) as usize) << 16) | (offset as usize);
                    *self.rom.get(rom_offset).unwrap_or(&0)
                } else {
                    0
                }
            }
            // Banks $C0-$FF: Full ROM access (primary area)
            0xC0..=0xFF => {
                let rom_offset = (((bank - 0xC0) as usize) << 16) | (offset as usize);
                *self.rom.get(rom_offset).unwrap_or(&0)
            }
            _ => 0,
        }
    }

    pub fn write(&mut self, addr: u32, val: u8) {
        match self.mapping_mode {
            MappingMode::LoROM => self.write_lorom(addr, val),
            MappingMode::HiROM => self.write_hirom(addr, val),
        }
    }

    fn write_lorom(&mut self, addr: u32, val: u8) {
        let bank = (addr >> 16) as u8;
        let offset = (addr & 0xFFFF) as u16;

        // SRAM mapping (banks $70-$7D, $F0-$FF at $0000-$7FFF)
        if matches!(bank, 0x70..=0x7D | 0xF0..=0xFF) && offset < 0x8000 {
            let ram_offset = offset as usize;
            if ram_offset < self.ram.len() {
                log(LogCategory::Bus, LogLevel::Trace, || {
                    format!(
                        "SNES Cartridge: LoROM SRAM Write ${:06X} = ${:02X}",
                        addr, val
                    )
                });
                self.ram[ram_offset] = val;
            }
        }
    }

    fn write_hirom(&mut self, addr: u32, val: u8) {
        let bank = (addr >> 16) as u8;
        let offset = (addr & 0xFFFF) as u16;

        // SRAM mapping (banks $20-$3F, $A0-$BF at $6000-$7FFF)
        if matches!(bank, 0x20..=0x3F | 0xA0..=0xBF) && (0x6000..0x8000).contains(&offset) {
            let ram_offset = (offset - 0x6000) as usize;
            if ram_offset < self.ram.len() {
                log(LogCategory::Bus, LogLevel::Trace, || {
                    format!(
                        "SNES Cartridge: HiROM SRAM Write ${:06X} = ${:02X}",
                        addr, val
                    )
                });
                self.ram[ram_offset] = val;
            }
        }
    }

    pub fn rom_size(&self) -> usize {
        self.rom.len()
    }

    pub fn has_smc_header(&self) -> bool {
        self.header_offset == 512
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn is_hirom(&self) -> bool {
        self.mapping_mode == MappingMode::HiROM
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_too_small() {
        let data = vec![0; 1024];
        assert!(Cartridge::load(&data).is_err());
    }

    #[test]
    fn test_load_with_smc_header() {
        let mut data = vec![0; 512 + 0x8000]; // 512-byte header + 32KB ROM
                                              // SMC header
        data.iter_mut().take(512).for_each(|x| *x = 0xFF);

        let cart = Cartridge::load(&data).unwrap();
        assert_eq!(cart.header_offset, 512);
        assert_eq!(cart.rom.len(), 0x8000);
    }

    #[test]
    fn test_load_without_header() {
        let data = vec![0; 0x8000]; // 32KB ROM, no header

        let cart = Cartridge::load(&data).unwrap();
        assert_eq!(cart.header_offset, 0);
        assert_eq!(cart.rom.len(), 0x8000);
    }

    #[test]
    fn test_read_rom_lorom() {
        let mut data = vec![0; 0x8000];
        data[0] = 0x42; // First byte

        let cart = Cartridge::load(&data).unwrap();

        // Bank 0, offset $8000 should read first ROM byte (LoROM)
        assert_eq!(cart.read(0x008000), 0x42);
        // Bank 0x80, offset $8000 should also read first ROM byte (mirror)
        assert_eq!(cart.read(0x808000), 0x42);
    }

    #[test]
    fn test_read_rom_hirom() {
        // Create a ROM large enough for HiROM with valid header
        let mut data = vec![0; 0x10000];

        // Set up HiROM header at $FFC0
        let header_offset = 0xFFC0;
        data[header_offset + 0x15] = 0x01; // Mapper type
        data[header_offset + 0x17] = 0x09; // ROM size
        data[header_offset + 0x1C] = 0x00; // Checksum complement low
        data[header_offset + 0x1D] = 0x00; // Checksum complement high
        data[header_offset + 0x1E] = 0xFF; // Checksum low
        data[header_offset + 0x1F] = 0xFF; // Checksum high
        data[header_offset + 0x3C] = 0x00; // Reset vector low
        data[header_offset + 0x3D] = 0x80; // Reset vector high

        // Put test data at ROM start
        data[0] = 0x42;

        let cart = Cartridge::load(&data).unwrap();
        assert!(cart.is_hirom());

        // In HiROM, bank $C0 maps directly to ROM start
        assert_eq!(cart.read(0xC00000), 0x42);
    }

    #[test]
    fn test_write_read_ram_lorom() {
        let data = vec![0; 0x8000];
        let mut cart = Cartridge::load(&data).unwrap();
        assert!(!cart.is_hirom()); // Should be LoROM

        // Write to SRAM (bank $70, offset $0000)
        cart.write(0x700000, 0x55);

        // Read back
        assert_eq!(cart.ram[0], 0x55);
        assert_eq!(cart.read(0x700000), 0x55);
    }

    #[test]
    fn test_write_read_ram_hirom() {
        // Create HiROM ROM with valid header
        let mut data = vec![0; 0x10000];

        // Set up HiROM header at $FFC0
        let header_offset = 0xFFC0;
        data[header_offset + 0x15] = 0x01; // Mapper type
        data[header_offset + 0x17] = 0x09; // ROM size
        data[header_offset + 0x1C] = 0x00; // Checksum complement low
        data[header_offset + 0x1D] = 0x00; // Checksum complement high
        data[header_offset + 0x1E] = 0xFF; // Checksum low
        data[header_offset + 0x1F] = 0xFF; // Checksum high
        data[header_offset + 0x3C] = 0x00; // Reset vector low
        data[header_offset + 0x3D] = 0x80; // Reset vector high

        let mut cart = Cartridge::load(&data).unwrap();
        assert!(cart.is_hirom());

        // Write to SRAM (bank $20, offset $6000-$7FFF)
        cart.write(0x206000, 0xAA);

        // Read back
        assert_eq!(cart.ram[0], 0xAA);
        assert_eq!(cart.read(0x206000), 0xAA);
    }

    #[test]
    fn test_mapping_mode_detection() {
        // LoROM: header at $7FC0 should score higher
        let mut lorom_data = vec![0; 0x8000];
        lorom_data[0x7FC0 + 0x15] = 0x01; // Valid mapper
        lorom_data[0x7FC0 + 0x17] = 0x09; // Valid size
        lorom_data[0x7FC0 + 0x3C] = 0x00; // Reset vector
        lorom_data[0x7FC0 + 0x3D] = 0x80;

        let lorom_cart = Cartridge::load(&lorom_data).unwrap();
        assert!(!lorom_cart.is_hirom());

        // HiROM: header at $FFC0 should score higher
        let mut hirom_data = vec![0; 0x10000];
        hirom_data[0xFFC0 + 0x15] = 0x01; // Valid mapper
        hirom_data[0xFFC0 + 0x17] = 0x09; // Valid size
        hirom_data[0xFFC0 + 0x1C] = 0x00; // Checksum complement
        hirom_data[0xFFC0 + 0x1D] = 0x00;
        hirom_data[0xFFC0 + 0x1E] = 0xFF; // Checksum
        hirom_data[0xFFC0 + 0x1F] = 0xFF;
        hirom_data[0xFFC0 + 0x3C] = 0x00; // Reset vector
        hirom_data[0xFFC0 + 0x3D] = 0x80;

        let hirom_cart = Cartridge::load(&hirom_data).unwrap();
        assert!(hirom_cart.is_hirom());
    }
}
