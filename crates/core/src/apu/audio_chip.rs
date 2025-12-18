//! Audio chip trait for pluggable audio implementations.
//!
//! This module defines a common interface for various retro gaming audio chips,
//! allowing them to be plugged into different emulated systems.

use super::TimingMode;

/// A trait for audio chips/APUs from various retro gaming systems.
///
/// Implementations exist for:
/// - RP2A03 (NES NTSC)
/// - RP2A07 (NES PAL)
/// - SID (Commodore 64) - future
/// - TIA (Atari 2600) - future
/// - SN76489 (ColecoVision, Sega Master System) - future
/// - POKEY (Atari 8-bit computers) - future
pub trait AudioChip {
    /// Write to a register on the audio chip
    fn write_register(&mut self, addr: u16, val: u8);

    /// Read from a register on the audio chip (if supported)
    fn read_register(&self, addr: u16) -> u8 {
        let _ = addr;
        0 // Default: no readable registers
    }

    /// Clock the chip for one CPU cycle, returning an audio sample
    fn clock(&mut self) -> i16;

    /// Get the timing mode of this chip (NTSC/PAL)
    fn timing(&self) -> TimingMode;

    /// Generate multiple samples efficiently
    fn generate_samples(&mut self, count: usize) -> Vec<i16> {
        let mut samples = Vec::with_capacity(count);
        for _ in 0..count {
            samples.push(self.clock());
        }
        samples
    }

    /// Reset the chip to power-on state
    fn reset(&mut self);

    /// Get the native sample rate of this chip (in Hz)
    fn sample_rate(&self) -> f64 {
        self.timing().cpu_clock_hz()
    }
}
