//! SNES Enhancement Chip (Coprocessor) implementations
//!
//! This module provides implementations of various SNES enhancement chips that were
//! used in cartridges to extend the capabilities of the base SNES hardware.
//!
//! ## Implemented Coprocessors
//!
//! - **DSP-1**: Math coprocessor for 3D calculations (multiply, divide, sin, cos, etc.)
//! - **SuperFX/SuperFX2**: Graphics coprocessor (GSU-1/GSU-2) for 3D rendering and effects
//!
//! ## References
//!
//! - SNESdev Wiki - Enhancement Chips: https://snes.nesdev.org/wiki/Enhancement_chips
//! - SNESdev Wiki - DSP-1: https://snes.nesdev.org/wiki/DSP-1
//! - SNESLab - DSP1: https://sneslab.net/wiki/DSP1
//! - SNESdev Wiki - SuperFX: https://snes.nesdev.org/wiki/Super_FX
//! - SnesLab - SuperFX: https://sneslab.net/wiki/Super_FX

pub mod dsp1;
pub mod sa1;
pub mod superfx;

use serde::{Deserialize, Serialize};

/// Type of enhancement chip detected from ROM header
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChipType {
    /// No enhancement chip
    None,
    /// DSP-1, DSP-1A, or DSP-1B math coprocessor
    Dsp1,
    /// DSP-2 math coprocessor
    Dsp2,
    /// DSP-3 math coprocessor
    Dsp3,
    /// DSP-4 math coprocessor
    Dsp4,
    /// SuperFX graphics coprocessor (GSU-1)
    SuperFx,
    /// SuperFX 2 graphics coprocessor (GSU-2)
    SuperFx2,
    /// SA-1 CPU coprocessor
    Sa1,
    /// S-DD1 decompression chip
    Sdd1,
    /// OBC-1 (used in Metal Combat)
    Obc1,
    /// SPC7110 data decompression chip
    Spc7110,
    /// ST010 coprocessor
    St010,
    /// ST011 coprocessor
    St011,
    /// ST018 coprocessor
    St018,
    /// CX4 coprocessor (used in Mega Man X2/X3)
    Cx4,
}

impl ChipType {
    /// Detect enhancement chip type from ROM header data
    ///
    /// # Arguments
    ///
    /// * `rom_type` - ROM type byte from header offset +$16
    /// * `map_mode` - Map mode byte from header offset +$15
    ///
    /// # Returns
    ///
    /// The detected chip type
    pub fn detect(rom_type: u8, _map_mode: u8) -> Self {
        // ROM type byte encoding (offset +$16 from header):
        //
        // The low nibble encodes ROM/RAM/Battery configuration:
        //   $x3 = ROM + Coprocessor
        //   $x4 = ROM + Coprocessor + RAM
        //   $x5 = ROM + Coprocessor + RAM + Battery
        //   $x6 = ROM + Coprocessor + Battery
        //
        // The high nibble encodes the coprocessor family:
        //   $0x = DSP (DSP-1 for most games; DSP-2/3/4 need title-based detection)
        //   $1x = SuperFX (GSU)
        //   $2x = SuperFX2 / OBC-1
        //   $3x = SA-1
        //   $4x = S-DD1
        //   $Ex = SPC7110 / other
        //   $Fx = ST / CX4
        //
        // Note: DSP-1 vs DSP-2/3/4 cannot be distinguished by ROM type alone.
        // DSP-2 (Dungeon Master), DSP-3 (SD Gundam GX), DSP-4 (Top Gear 3000)
        // all use $03 as their ROM type. We default to DSP-1 since it covers
        // the vast majority of DSP games (Super Mario Kart, Pilotwings, etc.).

        match rom_type {
            0x03..=0x06 => ChipType::Dsp1,
            0x13..=0x15 | 0x1A => ChipType::SuperFx,
            0x23..=0x25 => ChipType::SuperFx2,
            0x33..=0x35 => ChipType::Sa1,
            0x43..=0x45 => ChipType::Sdd1,
            0xD3 => ChipType::Obc1,
            0xE3..=0xE5 => ChipType::Spc7110,
            0xF3 => ChipType::Cx4,
            0xF5 => ChipType::St010,
            0xF6 => ChipType::St011,
            0xF9 => ChipType::St018,
            _ => ChipType::None,
        }
    }

    /// Check if this chip type is implemented
    pub fn is_implemented(self) -> bool {
        matches!(
            self,
            ChipType::Dsp1 | ChipType::SuperFx | ChipType::SuperFx2 | ChipType::Sa1
        )
    }

    /// Get a human-readable name for the chip
    pub fn name(self) -> &'static str {
        match self {
            ChipType::None => "None",
            ChipType::Dsp1 => "DSP-1",
            ChipType::Dsp2 => "DSP-2",
            ChipType::Dsp3 => "DSP-3",
            ChipType::Dsp4 => "DSP-4",
            ChipType::SuperFx => "SuperFX",
            ChipType::SuperFx2 => "SuperFX 2",
            ChipType::Sa1 => "SA-1",
            ChipType::Sdd1 => "S-DD1",
            ChipType::Obc1 => "OBC-1",
            ChipType::Spc7110 => "SPC7110",
            ChipType::St010 => "ST010",
            ChipType::St011 => "ST011",
            ChipType::St018 => "ST018",
            ChipType::Cx4 => "CX4",
        }
    }
}

/// Trait for all enhancement chips
pub trait EnhancementChip {
    /// Read from the chip's memory space
    fn read(&mut self, addr: u32) -> u8;

    /// Write to the chip's memory space
    fn write(&mut self, addr: u32, value: u8);

    /// Reset the chip to its initial state
    fn reset(&mut self);

    /// Get the chip type
    fn chip_type(&self) -> ChipType;

    /// Serialize chip state for save states
    /// Returns a JSON string representation of the chip's state
    fn save_state(&self) -> Result<String, String>;

    /// Deserialize chip state from save states
    /// Restores the chip's state from a JSON string
    fn load_state(&mut self, state: &str) -> Result<(), String>;

    /// Tick the chip for the given number of cycles (for coprocessors that run asynchronously)
    /// Default implementation does nothing (for chips that don't need continuous execution)
    fn tick(&mut self, _cycles: u64) {
        // Default: do nothing
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chip_detect_dsp1_all_variants() {
        // ROM types $03-$06 (high nibble $0) should ALL detect as DSP-1
        // $03 = ROM + Coprocessor
        // $04 = ROM + Coprocessor + RAM
        // $05 = ROM + Coprocessor + RAM + Battery (Super Mario Kart!)
        // $06 = ROM + Coprocessor + Battery
        assert_eq!(ChipType::detect(0x03, 0x20), ChipType::Dsp1);
        assert_eq!(ChipType::detect(0x04, 0x20), ChipType::Dsp1);
        assert_eq!(ChipType::detect(0x05, 0x20), ChipType::Dsp1);
        assert_eq!(ChipType::detect(0x06, 0x20), ChipType::Dsp1);
    }

    #[test]
    fn test_chip_detect_superfx() {
        assert_eq!(ChipType::detect(0x13, 0x20), ChipType::SuperFx);
        assert_eq!(ChipType::detect(0x14, 0x20), ChipType::SuperFx);
        assert_eq!(ChipType::detect(0x15, 0x20), ChipType::SuperFx);
        assert_eq!(ChipType::detect(0x1A, 0x20), ChipType::SuperFx);
    }

    #[test]
    fn test_chip_detect_sa1() {
        assert_eq!(ChipType::detect(0x33, 0x21), ChipType::Sa1);
        assert_eq!(ChipType::detect(0x34, 0x21), ChipType::Sa1);
        assert_eq!(ChipType::detect(0x35, 0x21), ChipType::Sa1);
    }

    #[test]
    fn test_chip_detect_none() {
        assert_eq!(ChipType::detect(0x00, 0x20), ChipType::None);
        assert_eq!(ChipType::detect(0x01, 0x20), ChipType::None);
        assert_eq!(ChipType::detect(0x02, 0x20), ChipType::None);
    }

    #[test]
    fn test_dsp1_is_implemented() {
        assert!(ChipType::Dsp1.is_implemented());
        assert!(!ChipType::Dsp2.is_implemented());
        assert!(!ChipType::Dsp3.is_implemented());
        assert!(!ChipType::Dsp4.is_implemented());
    }
}
