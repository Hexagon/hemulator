//! SMS PSG (Programmable Sound Generator) implementation.
//!
//! This module provides the SMS-specific PSG interface while using
//! the reusable SN76489 adapter from the core module.
//!
//! ## Current Implementation
//!
//! The PSG implements all 4 channels:
//!
//! - **3 Tone Channels**: Square wave generators with 10-bit frequency control
//! - **1 Noise Channel**: Pseudo-random noise with 16-bit LFSR (Sega variant)
//! - **Volume Control**: 4-bit volume control per channel (0=max, 15=mute)
//!
//! ## Register Interface
//!
//! The PSG is accessed via I/O ports 0x7E and 0x7F:
//!
//! - Latch/Data byte format: 1cctdddd (c=channel, t=type, d=data)
//! - Data byte format: 0ddddddd (d=data)
//!
//! ## Audio Output
//!
//! The PSG generates 44.1 kHz audio by clocking the SN76489 at SMS CPU speed
//! (3.579545 MHz NTSC or 3.546894 MHz PAL) and downsampling via cycle
//! accumulation.

use emu_core::apu::{sn76489::Sn76489Adapter, TimingMode};
use emu_core::logging::{log, LogCategory, LogLevel};

/// SMS NTSC CPU clock rate (Hz).
const CPU_HZ_NTSC: f64 = 3_579_545.0;

/// SMS PAL CPU clock rate (Hz).
const CPU_HZ_PAL: f64 = 3_546_894.0;

/// SMS PSG with proper CPU-cycle-based audio generation.
pub struct SmsPsg(Sn76489Adapter);

impl SmsPsg {
    /// Create a new SMS PSG with NTSC timing.
    pub fn new() -> Self {
        Self::new_with_timing(TimingMode::Ntsc)
    }

    /// Create a new SMS PSG with the specified timing mode.
    pub fn new_with_timing(timing: TimingMode) -> Self {
        Self(Sn76489Adapter::new(timing, CPU_HZ_NTSC, CPU_HZ_PAL))
    }

    /// Set timing mode (NTSC/PAL).
    pub fn set_timing(&mut self, timing: TimingMode) {
        self.0.set_timing(timing);
    }

    /// Write a byte to the PSG.
    pub fn write(&mut self, data: u8) {
        log(LogCategory::APU, LogLevel::Debug, || {
            format!("SMS PSG: Write 0x{:02X}", data)
        });
        self.0.write(data);
    }

    /// Reset the PSG to its initial state.
    pub fn reset(&mut self) {
        log(LogCategory::APU, LogLevel::Info, || {
            "SMS PSG: Reset".to_string()
        });
        self.0.reset();
    }

    /// Generate `sample_count` audio samples at 44.1 kHz.
    pub fn generate_samples(&mut self, sample_count: usize) -> Vec<i16> {
        self.0.generate_samples(sample_count)
    }

    /// Serialise PSG state for save states.
    pub fn get_state(&self) -> serde_json::Value {
        self.0.get_state()
    }

    /// Restore PSG state from a save state.
    pub fn set_state(&mut self, state: &serde_json::Value) -> Result<(), serde_json::Error> {
        self.0.set_state(state)
    }
}

impl Default for SmsPsg {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_psg_creation() {
        let mut psg = SmsPsg::new();
        // NTSC default: all channels muted, so samples should be silent
        let samples = psg.generate_samples(10);
        assert_eq!(samples.len(), 10);
        assert!(
            samples.iter().all(|&s| s == 0),
            "Expected silent output from a freshly created PSG"
        );
    }

    #[test]
    fn test_psg_reset() {
        let mut psg = SmsPsg::new();

        // Write some data
        psg.write(0x80); // Latch tone 0, data 0

        // Reset should restore initial state; output must be silent again
        psg.reset();
        let samples = psg.generate_samples(10);
        assert_eq!(samples.len(), 10);
        assert!(
            samples.iter().all(|&s| s == 0),
            "Expected silent output after PSG reset (all channels muted)"
        );
    }

    #[test]
    fn test_psg_volume_write() {
        let mut psg = SmsPsg::new();

        // Set channel 0 to max volume (0)
        psg.write(0x90); // Latch tone 0, volume, value 0 (max)

        // Generate samples - should produce non-zero output if frequency is set
        let samples = psg.generate_samples(100);
        assert_eq!(samples.len(), 100);
    }

    #[test]
    fn test_psg_tone_frequency() {
        let mut psg = SmsPsg::new();

        // Set channel 0 to max volume
        psg.write(0x90); // Channel 0, volume 0 (max)

        // Set a frequency (440 Hz A note)
        // SMS PSG frequency = clock / (32 * register)
        // For 440 Hz: register = 3579545 / (32 * 440) ≈ 254
        psg.write(0x80 | 0x0E); // Latch tone 0, low 4 bits = 0xE
        psg.write(0x0F); // High 6 bits = 0xF, total = 0xFE (254)

        // Generate samples
        let samples = psg.generate_samples(1000);
        assert_eq!(samples.len(), 1000);

        // Check that we have varying output (not all zeros)
        let non_zero_count = samples.iter().filter(|&&s| s != 0).count();
        assert!(non_zero_count > 0, "Expected non-zero audio output");
    }

    #[test]
    fn test_psg_noise_channel() {
        let mut psg = SmsPsg::new();

        // Set noise channel (channel 3) to max volume
        psg.write(0xF0); // Channel 3, volume 0 (max)

        // Configure noise (white noise, rate 0)
        psg.write(0xE4); // Latch noise, white noise (bit 2), rate 0

        // Generate samples
        let samples = psg.generate_samples(1000);
        assert_eq!(samples.len(), 1000);

        // Noise should produce varying output
        let non_zero_count = samples.iter().filter(|&&s| s != 0).count();
        assert!(non_zero_count > 0, "Expected non-zero noise output");
    }

    #[test]
    fn test_psg_muted_output() {
        let mut psg = SmsPsg::new();

        // Set all channels to muted (volume 0xF)
        psg.write(0x9F); // Channel 0, volume F (muted)
        psg.write(0xBF); // Channel 1, volume F (muted)
        psg.write(0xDF); // Channel 2, volume F (muted)
        psg.write(0xFF); // Channel 3, volume F (muted)

        // Generate samples - should be silent
        let samples = psg.generate_samples(100);

        // All samples should be zero (or very close to zero)
        let max_sample = samples.iter().map(|&s| s.abs()).max().unwrap_or(0);
        assert!(
            max_sample <= 100,
            "Expected near-silent output when muted, got max sample: {}",
            max_sample
        );
    }

    #[test]
    fn test_psg_cycle_accumulation() {
        let mut psg = SmsPsg::new();

        // Generate a few samples
        let samples1 = psg.generate_samples(10);
        assert_eq!(samples1.len(), 10);

        // Generate more samples - should continue from previous accumulator state
        let samples2 = psg.generate_samples(10);
        assert_eq!(samples2.len(), 10);
    }

    #[test]
    fn test_psg_timing_modes() {
        let mut psg_ntsc = SmsPsg::new_with_timing(TimingMode::Ntsc);
        let mut psg_pal = SmsPsg::new_with_timing(TimingMode::Pal);

        // Set same frequency and volume for both
        psg_ntsc.write(0x90); // Max volume
        psg_ntsc.write(0x80 | 0x04); // Frequency low
        psg_ntsc.write(0x01); // Frequency high

        psg_pal.write(0x90); // Max volume
        psg_pal.write(0x80 | 0x04); // Frequency low
        psg_pal.write(0x01); // Frequency high

        // Both should generate samples
        let samples_ntsc = psg_ntsc.generate_samples(100);
        let samples_pal = psg_pal.generate_samples(100);

        assert_eq!(samples_ntsc.len(), 100);
        assert_eq!(samples_pal.len(), 100);

        // The samples might differ slightly due to timing differences
        // but both should produce audio
        let non_zero_ntsc = samples_ntsc.iter().filter(|&&s| s != 0).count();
        let non_zero_pal = samples_pal.iter().filter(|&&s| s != 0).count();

        assert!(non_zero_ntsc > 0, "NTSC PSG should produce audio");
        assert!(non_zero_pal > 0, "PAL PSG should produce audio");
    }

    #[test]
    fn test_psg_multiple_tone_channels() {
        let mut psg = SmsPsg::new();

        // Enable multiple tone channels with different frequencies
        // Channel 0: 440 Hz (A)
        psg.write(0x90); // Volume 0
        psg.write(0x80 | 0x0E); // Low bits
        psg.write(0x0F); // High bits

        // Channel 1: 554 Hz (C#)
        psg.write(0xB0); // Volume 0
        psg.write(0xA0 | 0x08); // Low bits
        psg.write(0x0C); // High bits

        // Channel 2: 659 Hz (E)
        psg.write(0xD0); // Volume 0
        psg.write(0xC0 | 0x0A); // Low bits
        psg.write(0x0A); // High bits

        // Generate samples
        let samples = psg.generate_samples(1000);
        assert_eq!(samples.len(), 1000);

        // Should have significant non-zero output from multiple channels
        let non_zero_count = samples.iter().filter(|&&s| s.abs() > 100).count();
        assert!(
            non_zero_count > 100,
            "Expected strong audio output from multiple channels"
        );
    }
}
