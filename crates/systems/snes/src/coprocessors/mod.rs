//! SNES Enhancement Chip (Coprocessor) implementations
//!
//! This module provides implementations of various SNES enhancement chips that were
//! used in cartridges to extend the capabilities of the base SNES hardware.
//!
//! ## Implemented Coprocessors
//!
//! - **DSP-1**: Math coprocessor for 3D calculations (multiply, divide, sin, cos, etc.)
//!
//! ## References
//!
//! - SNESdev Wiki - Enhancement Chips: https://snes.nesdev.org/wiki/Enhancement_chips
//! - SNESdev Wiki - DSP-1: https://snes.nesdev.org/wiki/DSP-1
//! - SNESLab - DSP1: https://sneslab.net/wiki/DSP1

pub mod dsp1;

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
        // $x0-$x2: Regular cartridge (no chip)
        // $x3: DSP-1
        // $x4: DSP-1 (alternate)
        // $x5: DSP-2
        // $x6: DSP-3
        // $xB: DSP-4
        // $13-$15: SuperFX (various versions)
        // $33-$35: SA-1
        // $43-$45: S-DD1
        // $E3-$E5: SPC7110
        // $F3: CX4
        // $F5: ST010
        // $F6: ST011
        // $F9: ST018
        // And more...

        match rom_type {
            0x03 | 0x04 => ChipType::Dsp1,
            0x05 => ChipType::Dsp2,
            0x06 => ChipType::Dsp3,
            0x0B => ChipType::Dsp4,
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
        matches!(self, ChipType::Dsp1)
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
}
