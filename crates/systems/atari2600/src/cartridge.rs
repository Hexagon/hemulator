//! Atari 2600 cartridge handling and banking
//!
//! Most Atari 2600 cartridges use simple ROM banking schemes.
//! Common formats:
//! - 2K: No banking, ROM at $F800-$FFFF
//! - 4K: No banking, ROM at $F000-$FFFF
//! - 8K and larger: Various banking schemes (F8, F6, F4, etc.)

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CartridgeError {
    #[error("Invalid ROM size: {0} bytes")]
    InvalidSize(usize),
    #[error("Unsupported banking scheme")]
    UnsupportedBanking,
}

/// Banking scheme types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BankingScheme {
    /// 2K ROM (no banking)
    Rom2K,
    /// 4K ROM (no banking)
    Rom4K,
    /// 8K F8 banking (2x 4K banks)
    F8,
    /// 12K FA banking (3x 4K banks)
    FA,
    /// 16K F6 banking (4x 4K banks)
    F6,
    /// 32K F4 banking (8x 4K banks)
    F4,
}

/// Atari 2600 cartridge
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cartridge {
    /// ROM data
    rom: Vec<u8>,
    /// Current bank number
    current_bank: usize,
    /// Banking scheme
    scheme: BankingScheme,
}

impl Cartridge {
    /// Create a new cartridge from ROM data
    pub fn new(rom: Vec<u8>) -> Result<Self, CartridgeError> {
        let scheme = Self::detect_banking(&rom)?;

        Ok(Self {
            rom,
            current_bank: 0,
            scheme,
        })
    }

    /// Detect banking scheme from ROM size
    fn detect_banking(rom: &[u8]) -> Result<BankingScheme, CartridgeError> {
        match rom.len() {
            2048 => Ok(BankingScheme::Rom2K),
            4096 => Ok(BankingScheme::Rom4K),
            8192 => Ok(BankingScheme::F8),
            12288 => Ok(BankingScheme::FA),
            16384 => Ok(BankingScheme::F6),
            32768 => Ok(BankingScheme::F4),
            _ => Err(CartridgeError::InvalidSize(rom.len())),
        }
    }

    /// Read from cartridge address space
    pub fn read(&self, addr: u16) -> u8 {
        match self.scheme {
            BankingScheme::Rom2K => {
                // 2K ROM mapped to $F800-$FFFF (mirrored)
                let offset = (addr & 0x07FF) as usize;
                self.rom[offset]
            }
            BankingScheme::Rom4K => {
                // 4K ROM mapped to $F000-$FFFF
                let offset = (addr & 0x0FFF) as usize;
                self.rom[offset]
            }
            BankingScheme::F8 => {
                // 8K F8: Two 4K banks
                // Bank switching at $1FF8 (bank 0) and $1FF9 (bank 1)
                let offset = (addr & 0x0FFF) as usize;
                let bank_offset = self.current_bank * 4096;
                self.rom[bank_offset + offset]
            }
            BankingScheme::FA => {
                // 12K FA: Three 4K banks
                let offset = (addr & 0x0FFF) as usize;
                let bank_offset = self.current_bank * 4096;
                self.rom[bank_offset + offset]
            }
            BankingScheme::F6 => {
                // 16K F6: Four 4K banks
                let offset = (addr & 0x0FFF) as usize;
                let bank_offset = self.current_bank * 4096;
                self.rom[bank_offset + offset]
            }
            BankingScheme::F4 => {
                // 32K F4: Eight 4K banks
                let offset = (addr & 0x0FFF) as usize;
                let bank_offset = self.current_bank * 4096;
                self.rom[bank_offset + offset]
            }
        }
    }

    /// Write to cartridge (for bank switching)
    pub fn write(&mut self, addr: u16) {
        match self.scheme {
            BankingScheme::Rom2K | BankingScheme::Rom4K => {
                // No banking
            }
            BankingScheme::F8 => {
                // F8 banking: $1FF8 = bank 0, $1FF9 = bank 1
                match addr {
                    0x1FF8 => self.current_bank = 0,
                    0x1FF9 => self.current_bank = 1,
                    _ => {}
                }
            }
            BankingScheme::FA => {
                // FA banking: $1FF8, $1FF9, $1FFA
                match addr {
                    0x1FF8 => self.current_bank = 0,
                    0x1FF9 => self.current_bank = 1,
                    0x1FFA => self.current_bank = 2,
                    _ => {}
                }
            }
            BankingScheme::F6 => {
                // F6 banking: $1FF6-$1FF9
                match addr {
                    0x1FF6 => self.current_bank = 0,
                    0x1FF7 => self.current_bank = 1,
                    0x1FF8 => self.current_bank = 2,
                    0x1FF9 => self.current_bank = 3,
                    _ => {}
                }
            }
            BankingScheme::F4 => {
                // F4 banking: $1FF4-$1FFB
                match addr {
                    0x1FF4 => self.current_bank = 0,
                    0x1FF5 => self.current_bank = 1,
                    0x1FF6 => self.current_bank = 2,
                    0x1FF7 => self.current_bank = 3,
                    0x1FF8 => self.current_bank = 4,
                    0x1FF9 => self.current_bank = 5,
                    0x1FFA => self.current_bank = 6,
                    0x1FFB => self.current_bank = 7,
                    _ => {}
                }
            }
        }
    }

    /// Get the current banking scheme
    pub fn scheme(&self) -> BankingScheme {
        self.scheme
    }

    /// Get the current bank number
    pub fn current_bank(&self) -> usize {
        self.current_bank
    }

    /// Get ROM size
    pub fn size(&self) -> usize {
        self.rom.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_2k_cartridge() {
        let rom = vec![0x42; 2048];
        let cart = Cartridge::new(rom).unwrap();

        assert_eq!(cart.scheme(), BankingScheme::Rom2K);
        assert_eq!(cart.read(0xF800), 0x42);
        assert_eq!(cart.read(0xFFFF), 0x42);
    }

    #[test]
    fn test_4k_cartridge() {
        let mut rom = vec![0x00; 4096];
        rom[0] = 0x12;
        rom[4095] = 0x34;

        let cart = Cartridge::new(rom).unwrap();

        assert_eq!(cart.scheme(), BankingScheme::Rom4K);
        assert_eq!(cart.read(0xF000), 0x12);
        assert_eq!(cart.read(0xFFFF), 0x34);
    }

    #[test]
    fn test_8k_f8_banking() {
        let mut rom = vec![0x00; 8192];
        // Bank 0 data
        rom[0] = 0x11;
        // Bank 1 data
        rom[4096] = 0x22;

        let mut cart = Cartridge::new(rom).unwrap();

        assert_eq!(cart.scheme(), BankingScheme::F8);

        // Initially in bank 0
        assert_eq!(cart.current_bank(), 0);
        assert_eq!(cart.read(0xF000), 0x11);

        // Switch to bank 1
        cart.write(0x1FF9);
        assert_eq!(cart.current_bank(), 1);
        assert_eq!(cart.read(0xF000), 0x22);

        // Switch back to bank 0
        cart.write(0x1FF8);
        assert_eq!(cart.current_bank(), 0);
        assert_eq!(cart.read(0xF000), 0x11);
    }

    #[test]
    fn test_16k_f6_banking() {
        let mut rom = vec![0x00; 16384];
        for i in 0..4 {
            rom[i * 4096] = (0x10 + i) as u8;
        }

        let mut cart = Cartridge::new(rom).unwrap();

        assert_eq!(cart.scheme(), BankingScheme::F6);

        // Test all 4 banks
        for bank in 0..4 {
            cart.write(0x1FF6 + bank as u16);
            assert_eq!(cart.current_bank(), bank);
            assert_eq!(cart.read(0xF000), (0x10 + bank) as u8);
        }
    }

    #[test]
    fn test_32k_f4_banking() {
        let rom = vec![0x00; 32768];
        let mut cart = Cartridge::new(rom).unwrap();

        assert_eq!(cart.scheme(), BankingScheme::F4);

        // Test all 8 banks
        for bank in 0..8 {
            cart.write(0x1FF4 + bank as u16);
            assert_eq!(cart.current_bank(), bank);
        }
    }

    #[test]
    fn test_invalid_rom_size() {
        let rom = vec![0x00; 1000];
        assert!(Cartridge::new(rom).is_err());
    }
}
