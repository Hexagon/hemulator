//! SNES cartridge implementation

use crate::SnesError;

/// SNES cartridge
pub struct Cartridge {
    /// ROM data
    rom: Vec<u8>,
    /// RAM (if present)
    ram: Vec<u8>,
    /// Header offset (512 bytes if SMC header present)
    header_offset: usize,
}

impl Cartridge {
    pub fn load(data: &[u8]) -> Result<Self, SnesError> {
        if data.len() < 0x8000 {
            return Err(SnesError::InvalidRom(
                "ROM too small (minimum 32KB)".to_string(),
            ));
        }

        // Check for SMC header (512 bytes)
        let header_offset = if data.len() % 1024 == 512 {
            512
        } else {
            0
        };

        let rom_data = &data[header_offset..];
        
        // Validate minimum ROM size
        if rom_data.len() < 0x8000 {
            return Err(SnesError::InvalidRom(
                "ROM data too small after header".to_string(),
            ));
        }

        Ok(Self {
            rom: rom_data.to_vec(),
            ram: vec![0; 0x8000], // 32KB SRAM (standard size)
            header_offset,
        })
    }

    pub fn read(&self, addr: u32) -> u8 {
        let bank = (addr >> 16) as u8;
        let offset = (addr & 0xFFFF) as u16;

        // Map address to ROM offset
        // Simple LoROM mapping for now
        match bank {
            0x00..=0x7D => {
                if offset >= 0x8000 {
                    let rom_offset = ((bank as usize) << 15) | ((offset as usize - 0x8000) & 0x7FFF);
                    *self.rom.get(rom_offset).unwrap_or(&0)
                } else {
                    0
                }
            }
            0x80..=0xFF => {
                if offset >= 0x8000 {
                    let rom_offset = (((bank as usize) - 0x80) << 15) | ((offset as usize - 0x8000) & 0x7FFF);
                    *self.rom.get(rom_offset).unwrap_or(&0)
                } else {
                    0
                }
            }
            _ => 0,
        }
    }

    pub fn write(&mut self, addr: u32, val: u8) {
        let bank = (addr >> 16) as u8;
        let offset = (addr & 0xFFFF) as u16;

        // SRAM mapping (banks $70-$7D, $F0-$FF at $0000-$7FFF)
        if matches!(bank, 0x70..=0x7D | 0xF0..=0xFF) && offset < 0x8000 {
            let ram_offset = offset as usize;
            if ram_offset < self.ram.len() {
                self.ram[ram_offset] = val;
            }
        }
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
        for i in 0..512 {
            data[i] = 0xFF;
        }
        
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
    fn test_read_rom() {
        let mut data = vec![0; 0x8000];
        data[0] = 0x42; // First byte
        
        let cart = Cartridge::load(&data).unwrap();
        
        // Bank 0, offset $8000 should read first ROM byte
        assert_eq!(cart.read(0x008000), 0x42);
        // Bank 0x80, offset $8000 should also read first ROM byte (mirror)
        assert_eq!(cart.read(0x808000), 0x42);
    }

    #[test]
    fn test_write_read_ram() {
        let data = vec![0; 0x8000];
        let mut cart = Cartridge::load(&data).unwrap();
        
        // Write to SRAM (bank $70, offset $0000)
        cart.write(0x700000, 0x55);
        
        // Read back (not through normal read since that's ROM-only in our simple implementation)
        assert_eq!(cart.ram[0], 0x55);
    }
}
