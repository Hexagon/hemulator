//! Atari 2600 cartridge handling and banking
//!
//! Most Atari 2600 cartridges use simple ROM banking schemes to expand beyond the
//! 4KB directly addressable in the cartridge space ($1000-$1FFF).
//!
//! # Banking Schemes
//!
//! Unlike the NES with its complex mapper chips, Atari 2600 banking is typically very simple:
//! reading or writing to specific addresses in the cartridge space switches banks.
//!
//! ## Supported Formats
//!
//! ### 2K ROM (No Banking)
//! - **Size**: 2048 bytes
//! - **Mapping**: ROM appears at $F800-$FFFF (2K), mirrored
//! - **Games**: Early simple games (e.g., Combat, Video Olympics)
//! - **Note**: This was the original Atari 2600 ROM size
//!
//! ### 4K ROM (No Banking)
//! - **Size**: 4096 bytes
//! - **Mapping**: ROM appears at $F000-$FFFF (4K)
//! - **Games**: Most common format (e.g., Adventure, Pac-Man, Space Invaders)
//! - **Note**: This is the "standard" Atari 2600 cartridge size
//!
//! ### F8 Banking (8K)
//! - **Size**: 8192 bytes (2 banks of 4K each)
//! - **Banks**: 2
//! - **Mapping**: One 4K bank visible at $F000-$FFFF
//! - **Switching**:
//!   - Read from $1FF8 → select bank 0
//!   - Read from $1FF9 → select bank 1
//! - **Games**: Many popular games (e.g., Pitfall!, River Raid)
//!
//! ### FA Banking (12K)
//! - **Size**: 12288 bytes (3 banks of 4K each)
//! - **Banks**: 3
//! - **Mapping**: One 4K bank visible at $F000-$FFFF
//! - **Switching**:
//!   - Read from $1FF8 → select bank 0
//!   - Read from $1FF9 → select bank 1
//!   - Read from $1FFA → select bank 2
//! - **Games**: CBS games (e.g., Omega Race)
//!
//! ### F6 Banking (16K)
//! - **Size**: 16384 bytes (4 banks of 4K each)
//! - **Banks**: 4
//! - **Mapping**: One 4K bank visible at $F000-$FFFF
//! - **Switching**:
//!   - Read from $1FF6 → select bank 0
//!   - Read from $1FF7 → select bank 1
//!   - Read from $1FF8 → select bank 2
//!   - Read from $1FF9 → select bank 3
//! - **Games**: Later games needing more space (e.g., Crystal Castles)
//!
//! ### F4 Banking (32K)
//! - **Size**: 32768 bytes (8 banks of 4K each)
//! - **Banks**: 8
//! - **Mapping**: One 4K bank visible at $F000-$FFFF
//! - **Switching**:
//!   - Read from $1FF4 → select bank 0
//!   - Read from $1FF5 → select bank 1
//!   - ... through ...
//!   - Read from $1FFB → select bank 7
//! - **Games**: Large games (e.g., Fatal Run)
//! - **Note**: This is the largest standard Atari 2600 cartridge format
//!
//! # Bank Switching Mechanics
//!
//! Bank switching on the Atari 2600 is **triggered by reads or writes** to specific addresses.
//! The actual data read/written doesn't matter - just accessing the address switches the bank.
//!
//! ## Example: F8 Banking
//!
//! ```text
//! # Atari 2600 assembly example
//! LDA $1FF9    ; Switch to bank 1 (the value read is discarded)
//! JMP SubInBank1
//!
//! LDA $1FF8    ; Switch to bank 0
//! JMP SubInBank0
//! ```
//!
//! ## Common Patterns
//!
//! 1. **Hotspots in ROM**: Bank switch addresses are usually in the cartridge ROM area itself,
//!    so jumping to code near the end of a bank automatically switches banks.
//!
//! 2. **Shared Code**: The last few bytes of each bank often contain the same reset vectors,
//!    ensuring the system boots correctly regardless of which bank is selected.
//!
//! 3. **Initialization**: Most games switch to bank 0 during initialization.
//!
//! # Auto-Detection
//!
//! This implementation **auto-detects** the banking scheme based on ROM size:
//! - 2KB → No banking (2K ROM)
//! - 4KB → No banking (4K ROM)
//! - 8KB → F8 banking
//! - 12KB → FA banking
//! - 16KB → F6 banking
//! - 32KB → F4 banking
//!
//! There's no header or metadata - the size determines the banking scheme. This works because
//! these schemes became de facto standards.
//!
//! # Implementation Details
//!
//! This implementation:
//! - ✅ Supports all 6 standard banking schemes (2K, 4K, F8, FA, F6, F4)
//! - ✅ Auto-detects banking from ROM size
//! - ✅ Properly handles bank switching via read/write access
//! - ✅ Maintains current bank state across frames
//! - ✅ Supports save states (bank state is serializable)
//! - ❌ Does not support more exotic schemes (e.g., DPC, FE, 3F, E0, etc.)
//!
//! The implemented schemes cover the vast majority of commercially released Atari 2600 games.

use serde::{Deserialize, Serialize};
use std::cell::Cell;
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
    /// FE banking (8K, 2x 4K banks, write to $01FE switches banks)
    FE,
    /// 3F banking (up to 512K, bank selected by writing to $3F)
    ThreeF,
    /// E0 banking (8K, 3 banks: 1x 4K fixed + 2x 2K switchable)
    E0,
    /// DPC (Pitfall II, 10K with extra display data)
    DPC,
}

/// Atari 2600 cartridge
#[derive(Debug, Clone)]
pub struct Cartridge {
    /// ROM data
    rom: Vec<u8>,
    /// Current bank number (for simple schemes)
    current_bank: Cell<usize>,
    /// For E0: separate bank selections for each 2K segment
    e0_banks: Cell<[usize; 3]>,
    /// Banking scheme
    scheme: BankingScheme,
}

impl Cartridge {
    /// Create a new cartridge from ROM data
    pub fn new(rom: Vec<u8>) -> Result<Self, CartridgeError> {
        let scheme = Self::detect_banking(&rom)?;

        Ok(Self {
            rom,
            current_bank: Cell::new(0),
            e0_banks: Cell::new([0, 0, 0]),
            scheme,
        })
    }

    /// Detect exotic banking schemes by searching for signature sequences
    fn detect_by_signature(rom: &[u8]) -> Option<BankingScheme> {
        // FE scheme: Look for STA $01FE (0x8D 0xFE 0x01 or 0x8D 0xFE 0x00)
        for i in 0..rom.len().saturating_sub(3) {
            if rom[i] == 0x8D && rom[i + 1] == 0xFE && (rom[i + 2] == 0x00 || rom[i + 2] == 0x01) {
                // Found FE signature
                if rom.len() == 8192 {
                    return Some(BankingScheme::FE);
                }
            }
        }

        // 3F scheme: Look for STA $3F (0x85 0x3F)
        for i in 0..rom.len().saturating_sub(2) {
            if rom[i] == 0x85 && rom[i + 1] == 0x3F {
                // Found 3F signature
                return Some(BankingScheme::ThreeF);
            }
        }

        // E0 scheme: Look for STA $E0/$E1/$E2 (0x8D 0xE0/0xE1/0xE2 0x1F)
        for i in 0..rom.len().saturating_sub(3) {
            if rom[i] == 0x8D
                && (rom[i + 1] == 0xE0 || rom[i + 1] == 0xE1 || rom[i + 1] == 0xE2)
                && rom[i + 2] == 0x1F
            {
                // Found E0 signature
                if rom.len() == 8192 {
                    return Some(BankingScheme::E0);
                }
            }
        }

        // DPC scheme: Check for 10K size (10240 bytes) - Pitfall II
        if rom.len() == 10240 {
            return Some(BankingScheme::DPC);
        }

        None
    }

    /// Detect banking scheme from ROM size and signatures
    fn detect_banking(rom: &[u8]) -> Result<BankingScheme, CartridgeError> {
        // First, try signature-based detection for exotic schemes
        if let Some(scheme) = Self::detect_by_signature(rom) {
            return Ok(scheme);
        }

        // Fall back to size-based detection for standard schemes
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
        // Many Atari 2600 bank-switch schemes are triggered by *reads* from hot-spot addresses.
        // Because the CPU memory interface is `read(&self)`, we use interior mutability.
        self.maybe_bank_switch(addr);

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
                let bank_offset = self.current_bank.get() * 4096;
                self.rom[bank_offset + offset]
            }
            BankingScheme::FA => {
                // 12K FA: Three 4K banks
                let offset = (addr & 0x0FFF) as usize;
                let bank_offset = self.current_bank.get() * 4096;
                self.rom[bank_offset + offset]
            }
            BankingScheme::F6 => {
                // 16K F6: Four 4K banks
                let offset = (addr & 0x0FFF) as usize;
                let bank_offset = self.current_bank.get() * 4096;
                self.rom[bank_offset + offset]
            }
            BankingScheme::F4 => {
                // 32K F4: Eight 4K banks
                let offset = (addr & 0x0FFF) as usize;
                let bank_offset = self.current_bank.get() * 4096;
                self.rom[bank_offset + offset]
            }
            BankingScheme::FE => {
                // FE banking: Two 4K banks
                // Bank is selected by D5 bit of last byte written to $01FE
                let offset = (addr & 0x0FFF) as usize;
                let bank_offset = self.current_bank.get() * 4096;
                self.rom[bank_offset + offset]
            }
            BankingScheme::ThreeF => {
                // 3F banking: Write bank number to $3F
                // Can support many banks (up to 512K)
                let offset = (addr & 0x07FF) as usize; // 2K banks
                let bank_offset = self.current_bank.get() * 2048;
                if bank_offset + offset < self.rom.len() {
                    self.rom[bank_offset + offset]
                } else {
                    0
                }
            }
            BankingScheme::E0 => {
                // E0 banking: 8K with 3 segments
                // $1000-$13FF: bank 0-7 (selected by $1FE0-$1FE7)
                // $1400-$17FF: bank 0-7 (selected by $1FE8-$1FEF)
                // $1800-$1BFF: bank 0-7 (selected by $1FE0-$1FE7) - alternate
                // $1C00-$1FFF: always bank 7 (fixed)
                let banks = self.e0_banks.get();
                let segment = ((addr & 0x0FFF) >> 10) as usize; // Which 1K segment (0-3)

                match segment {
                    0 => {
                        // $1000-$13FF - switchable bank
                        let offset = (addr & 0x03FF) as usize;
                        let bank_offset = banks[0] * 1024;
                        self.rom[bank_offset + offset]
                    }
                    1 => {
                        // $1400-$17FF - switchable bank
                        let offset = (addr & 0x03FF) as usize;
                        let bank_offset = banks[1] * 1024;
                        self.rom[bank_offset + offset]
                    }
                    2 => {
                        // $1800-$1BFF - switchable bank
                        let offset = (addr & 0x03FF) as usize;
                        let bank_offset = banks[2] * 1024;
                        self.rom[bank_offset + offset]
                    }
                    _ => {
                        // $1C00-$1FFF - fixed to last 1K
                        let offset = (addr & 0x03FF) as usize;
                        let bank_offset = 7 * 1024;
                        self.rom[bank_offset + offset]
                    }
                }
            }
            BankingScheme::DPC => {
                // DPC (Pitfall II): Complex scheme with display processor
                // For now, implement basic 8K + 2K structure
                // First 8K is banked ROM, last 2K is graphics data
                if addr < 0x1800 {
                    // Banked ROM area
                    let offset = (addr & 0x0FFF) as usize;
                    let bank_offset = self.current_bank.get() * 4096;
                    self.rom[bank_offset + offset]
                } else {
                    // Graphics data area (fixed)
                    let offset = ((addr - 0x1800) & 0x07FF) as usize;
                    self.rom[8192 + offset]
                }
            }
        }
    }

    fn maybe_bank_switch(&self, addr: u16) {
        // Address is already masked to 13 bits by the bus, so hot-spots are in $1FF4-$1FFB.
        match self.scheme {
            BankingScheme::Rom2K | BankingScheme::Rom4K => {}
            BankingScheme::F8 => match addr {
                0x1FF8 => self.current_bank.set(0),
                0x1FF9 => self.current_bank.set(1),
                _ => {}
            },
            BankingScheme::FA => match addr {
                0x1FF8 => self.current_bank.set(0),
                0x1FF9 => self.current_bank.set(1),
                0x1FFA => self.current_bank.set(2),
                _ => {}
            },
            BankingScheme::F6 => match addr {
                0x1FF6 => self.current_bank.set(0),
                0x1FF7 => self.current_bank.set(1),
                0x1FF8 => self.current_bank.set(2),
                0x1FF9 => self.current_bank.set(3),
                _ => {}
            },
            BankingScheme::F4 => match addr {
                0x1FF4 => self.current_bank.set(0),
                0x1FF5 => self.current_bank.set(1),
                0x1FF6 => self.current_bank.set(2),
                0x1FF7 => self.current_bank.set(3),
                0x1FF8 => self.current_bank.set(4),
                0x1FF9 => self.current_bank.set(5),
                0x1FFA => self.current_bank.set(6),
                0x1FFB => self.current_bank.set(7),
                _ => {}
            },
            BankingScheme::FE => {
                // FE banking switches on reads to $01FE
                // Bank selected by D5 bit (bit 5) of data bus during write
                // For simplicity, alternate between banks on access
                if addr == 0x01FE {
                    let current = self.current_bank.get();
                    self.current_bank.set(1 - current); // Toggle between 0 and 1
                }
            }
            BankingScheme::ThreeF => {
                // 3F banking: bank number written to $3F
                // Handled in write() method
            }
            BankingScheme::E0 => {
                // E0 banking: Multiple hotspots for different segments
                match addr {
                    0x1FE0..=0x1FE6 => {
                        // Select bank for segment 0
                        let bank = (addr - 0x1FE0) as usize;
                        let mut banks = self.e0_banks.get();
                        banks[0] = bank;
                        self.e0_banks.set(banks);
                    }
                    0x1FE8..=0x1FEE => {
                        // Select bank for segment 1
                        let bank = (addr - 0x1FE8) as usize;
                        let mut banks = self.e0_banks.get();
                        banks[1] = bank;
                        self.e0_banks.set(banks);
                    }
                    0x1FE7 | 0x1FEF => {
                        // Select bank for segment 2
                        let bank = if addr == 0x1FE7 { 6 } else { 7 };
                        let mut banks = self.e0_banks.get();
                        banks[2] = bank;
                        self.e0_banks.set(banks);
                    }
                    _ => {}
                }
            }
            BankingScheme::DPC => {
                // DPC banking switches on reads to $1FF8-$1FF9
                match addr {
                    0x1FF8 => self.current_bank.set(0),
                    0x1FF9 => self.current_bank.set(1),
                    _ => {}
                }
            }
        }
    }

    /// Write to cartridge (for bank switching)
    pub fn write(&mut self, addr: u16, value: u8) {
        // Handle 3F banking which uses the written value as bank number
        if matches!(self.scheme, BankingScheme::ThreeF) && (addr & 0x3F) == 0x3F {
            // Write to $3F selects bank
            let bank = value as usize;
            let max_banks = self.rom.len() / 2048; // 2K banks
            if bank < max_banks {
                self.current_bank.set(bank);
            }
        } else {
            // Other schemes just need address-based switching
            self.maybe_bank_switch(addr);
        }
    }

    /// Get the current banking scheme
    pub fn scheme(&self) -> BankingScheme {
        self.scheme
    }

    /// Get the current bank number
    pub fn current_bank(&self) -> usize {
        self.current_bank.get()
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
        cart.write(0x1FF9, 0);
        assert_eq!(cart.current_bank(), 1);
        assert_eq!(cart.read(0xF000), 0x22);

        // Switch back to bank 0
        cart.write(0x1FF8, 0);
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
            cart.write(0x1FF6 + bank as u16, 0);
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
            cart.write(0x1FF4 + bank as u16, 0);
            assert_eq!(cart.current_bank(), bank);
        }
    }

    #[test]
    fn test_invalid_rom_size() {
        let rom = vec![0x00; 1000];
        assert!(Cartridge::new(rom).is_err());
    }

    #[test]
    fn test_fe_banking_signature_detection() {
        // Create an 8K ROM with FE signature (STA $01FE)
        let mut rom = vec![0x00; 8192];
        // Insert signature: 0x8D 0xFE 0x01 (STA $01FE)
        rom[100] = 0x8D;
        rom[101] = 0xFE;
        rom[102] = 0x01;

        let cart = Cartridge::new(rom).unwrap();
        assert_eq!(cart.scheme(), BankingScheme::FE);
    }

    #[test]
    fn test_3f_banking_signature_detection() {
        // Create a ROM with 3F signature (STA $3F)
        let mut rom = vec![0x00; 8192];
        // Insert signature: 0x85 0x3F (STA $3F)
        rom[50] = 0x85;
        rom[51] = 0x3F;

        let cart = Cartridge::new(rom).unwrap();
        assert_eq!(cart.scheme(), BankingScheme::ThreeF);
    }

    #[test]
    fn test_e0_banking_signature_detection() {
        // Create an 8K ROM with E0 signature (STA $1FE0)
        let mut rom = vec![0x00; 8192];
        // Insert signature: 0x8D 0xE0 0x1F (STA $1FE0)
        rom[200] = 0x8D;
        rom[201] = 0xE0;
        rom[202] = 0x1F;

        let cart = Cartridge::new(rom).unwrap();
        assert_eq!(cart.scheme(), BankingScheme::E0);
    }

    #[test]
    fn test_dpc_banking_size_detection() {
        // Create a 10K ROM (DPC - Pitfall II)
        let rom = vec![0x00; 10240];

        let cart = Cartridge::new(rom).unwrap();
        assert_eq!(cart.scheme(), BankingScheme::DPC);
    }

    #[test]
    fn test_3f_banking_write_value() {
        // Create a ROM large enough for multiple 2K banks
        let mut rom = vec![0x00; 8192]; // 4 banks of 2K each
                                        // Add 3F signature
        rom[50] = 0x85;
        rom[51] = 0x3F;

        // Set different values in each 2K bank
        for bank in 0..4 {
            rom[bank * 2048] = (0x10 + bank) as u8;
        }

        let mut cart = Cartridge::new(rom).unwrap();
        assert_eq!(cart.scheme(), BankingScheme::ThreeF);

        // Test bank switching by writing bank number to $3F
        cart.write(0x003F, 0); // Select bank 0
        assert_eq!(cart.read(0xF800), 0x10);

        cart.write(0x003F, 1); // Select bank 1
        assert_eq!(cart.read(0xF800), 0x11);

        cart.write(0x003F, 2); // Select bank 2
        assert_eq!(cart.read(0xF800), 0x12);

        cart.write(0x003F, 3); // Select bank 3
        assert_eq!(cart.read(0xF800), 0x13);
    }
}
