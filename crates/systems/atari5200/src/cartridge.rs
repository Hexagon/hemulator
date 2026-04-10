//! Atari 5200 cartridge handling
//!
//! The Atari 5200 uses raw binary ROM cartridges with no standard header.
//! Cartridge sizes are typically 8KB, 16KB, or 32KB.
//!
//! # Banking
//!
//! - **8KB**: No banking, mapped at $8000-$9FFF (mirrored to $A000-$BFFF)
//! - **16KB**: No banking, mapped at $8000-$BFFF
//! - **32KB**: Two 16KB banks, bank switching via address access
//!   - Bank 0: $8000-$BFFF (default)
//!   - Switch by writing to specific addresses

use std::cell::Cell;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CartridgeError {
    #[error("Invalid ROM size: {0} bytes (expected 8192, 16384, or 32768)")]
    InvalidSize(usize),
}

/// Cartridge banking scheme
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BankingScheme {
    /// 8KB ROM, no banking
    Rom8K,
    /// 16KB ROM, no banking
    Rom16K,
    /// 32KB ROM, 2 banks of 16KB
    Rom32K,
}

/// Atari 5200 cartridge
#[derive(Debug)]
pub struct Cartridge {
    rom: Vec<u8>,
    scheme: BankingScheme,
    current_bank: Cell<usize>,
}

impl Cartridge {
    /// Create a new cartridge from ROM data
    pub fn new(rom: Vec<u8>) -> Result<Self, CartridgeError> {
        let scheme = match rom.len() {
            8192 => BankingScheme::Rom8K,
            16384 => BankingScheme::Rom16K,
            32768 => BankingScheme::Rom32K,
            other => return Err(CartridgeError::InvalidSize(other)),
        };

        Ok(Self {
            rom,
            scheme,
            current_bank: Cell::new(0),
        })
    }

    /// Read a byte from the cartridge ROM space ($8000-$BFFF)
    pub fn read(&self, addr: u16) -> u8 {
        let offset = (addr as usize) & 0x3FFF; // Offset within 16KB window

        match self.scheme {
            BankingScheme::Rom8K => {
                // 8KB ROM mirrored in the 16KB space
                let rom_offset = offset & 0x1FFF;
                self.rom.get(rom_offset).copied().unwrap_or(0xFF)
            }
            BankingScheme::Rom16K => self.rom.get(offset).copied().unwrap_or(0xFF),
            BankingScheme::Rom32K => {
                let bank = self.current_bank.get();
                let rom_offset = bank * 0x4000 + offset;
                self.rom.get(rom_offset).copied().unwrap_or(0xFF)
            }
        }
    }

    /// Write to cartridge space (for bank switching)
    pub fn write(&self, addr: u16, _val: u8) {
        if self.scheme == BankingScheme::Rom32K {
            // 32KB uses address-based bank switching
            // $BFE0-$BFE7: select bank 0, $BFE8-$BFEF: select bank 1
            match addr {
                0xBFE0..=0xBFE7 => self.current_bank.set(0),
                0xBFE8..=0xBFEF => self.current_bank.set(1),
                _ => {}
            }
        }
    }

    /// Get ROM size
    pub fn size(&self) -> usize {
        self.rom.len()
    }

    /// Get banking scheme
    pub fn scheme(&self) -> BankingScheme {
        self.scheme
    }

    /// Get current bank
    pub fn current_bank(&self) -> usize {
        self.current_bank.get()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_8k_cartridge() {
        let mut rom = vec![0u8; 8192];
        rom[0] = 0x42;
        rom[0x1FFC] = 0x00; // Reset vector low
        rom[0x1FFD] = 0x80; // Reset vector high
        let cart = Cartridge::new(rom).unwrap();
        assert_eq!(cart.scheme(), BankingScheme::Rom8K);
        assert_eq!(cart.read(0x8000), 0x42);
        // Mirrored
        assert_eq!(cart.read(0xA000), 0x42);
    }

    #[test]
    fn test_16k_cartridge() {
        let mut rom = vec![0u8; 16384];
        rom[0] = 0x42;
        rom[0x2000] = 0x84;
        let cart = Cartridge::new(rom).unwrap();
        assert_eq!(cart.scheme(), BankingScheme::Rom16K);
        assert_eq!(cart.read(0x8000), 0x42);
        assert_eq!(cart.read(0xA000), 0x84);
    }

    #[test]
    fn test_32k_cartridge() {
        let mut rom = vec![0u8; 32768];
        rom[0] = 0x42; // Bank 0, offset 0
        rom[0x4000] = 0x84; // Bank 1, offset 0
        let cart = Cartridge::new(rom).unwrap();
        assert_eq!(cart.scheme(), BankingScheme::Rom32K);

        // Default bank 0
        assert_eq!(cart.read(0x8000), 0x42);

        // Switch to bank 1
        cart.write(0xBFE8, 0);
        assert_eq!(cart.read(0x8000), 0x84);

        // Switch back to bank 0
        cart.write(0xBFE0, 0);
        assert_eq!(cart.read(0x8000), 0x42);
    }

    #[test]
    fn test_invalid_size() {
        let rom = vec![0u8; 1024];
        assert!(Cartridge::new(rom).is_err());
    }
}
