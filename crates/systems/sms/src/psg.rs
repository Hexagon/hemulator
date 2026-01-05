//! SMS PSG (Programmable Sound Generator) implementation.
//!
//! This module provides the SMS-specific PSG interface while using
//! the reusable SN76489 component from the core module.
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
//! Channel types:
//! - 0: Tone frequency
//! - 1: Volume
//!
//! ## Audio Output
//!
//! The PSG generates 44.1 kHz stereo audio by:
//!
//! 1. Clocking the PSG at CPU speed (~3.58 MHz NTSC or ~3.55 MHz PAL)
//! 2. Mixing all 4 channels using exponential volume curve
//! 3. Downsampling to the target sample rate (44.1 kHz)
//!
//! The implementation uses cycle accumulation for precise timing,
//! similar to the NES APU approach.

use emu_core::apu::{AudioChip, Sn76489Psg, TimingMode};
use emu_core::logging::{log, LogCategory, LogLevel};

/// SMS PSG with proper CPU-cycle-based audio generation
///
/// This wrapper provides cycle-accurate PSG emulation by clocking
/// the underlying SN76489 chip at CPU speed and downsampling to
/// the target audio sample rate.
///
/// # Timing
///
/// The PSG runs at CPU clock speed:
///
/// - NTSC: 3.579545 MHz
/// - PAL: 3.546894 MHz
///
/// Audio is downsampled to 44.1 kHz by accumulating CPU cycles
/// and clocking the PSG multiple times per audio sample.
pub struct SmsPsg {
    /// Core SN76489 PSG implementation
    psg: Sn76489Psg,

    /// Cycle accumulator for downsampling
    /// Tracks fractional CPU cycles to generate precise audio timing
    cycle_accum: f64,

    /// Current timing mode (NTSC/PAL)
    timing: TimingMode,
}

impl SmsPsg {
    /// Create a new SMS PSG
    pub fn new() -> Self {
        Self::new_with_timing(TimingMode::Ntsc)
    }

    /// Create a new SMS PSG with specific timing mode
    pub fn new_with_timing(timing: TimingMode) -> Self {
        Self {
            psg: Sn76489Psg::new(timing),
            cycle_accum: 0.0,
            timing,
        }
    }

    /// Set timing mode (NTSC/PAL)
    pub fn set_timing(&mut self, timing: TimingMode) {
        self.timing = timing;
        self.psg = Sn76489Psg::new(timing);
    }

    /// Write a byte to the PSG
    ///
    /// This handles both latch/data and continuation data bytes
    pub fn write(&mut self, data: u8) {
        log(LogCategory::APU, LogLevel::Debug, || {
            format!("SMS PSG: Write 0x{:02X}", data)
        });

        self.psg.write(data);
    }

    /// Reset the PSG to initial state
    pub fn reset(&mut self) {
        log(LogCategory::APU, LogLevel::Info, || {
            "SMS PSG: Reset".to_string()
        });

        self.psg.reset();
        self.cycle_accum = 0.0;
    }

    /// Generate audio samples for a given count, stepping PSG in CPU-cycle time
    /// using the configured timing mode and sample rate of 44.1 kHz.
    ///
    /// This method follows the same pattern as the NES APU:
    /// 1. Calculate how many CPU cycles correspond to each audio sample
    /// 2. Clock the PSG that many times per sample
    /// 3. Average the output over those cycles
    pub fn generate_samples(&mut self, sample_count: usize) -> Vec<i16> {
        const SAMPLE_HZ: f64 = 44_100.0;
        let cpu_hz = self.timing.cpu_clock_hz();
        let cycles_per_sample = cpu_hz / SAMPLE_HZ;

        let mut out = Vec::with_capacity(sample_count);
        for _ in 0..sample_count {
            self.cycle_accum += cycles_per_sample;
            let mut cycles = self.cycle_accum as u32;
            if cycles == 0 {
                cycles = 1; // Ensure we advance state even if timing slips
            }
            self.cycle_accum -= cycles as f64;

            // Clock PSG for all cycles and accumulate output
            let mut acc = 0i32;
            for _ in 0..cycles {
                let sample = self.psg.clock() as i32;
                acc += sample;
            }

            // Average over all cycles
            let avg = acc / cycles as i32;
            out.push(avg.clamp(-32768, 32767) as i16);
        }

        out
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
        let psg = SmsPsg::new();
        assert_eq!(psg.timing, TimingMode::Ntsc);
    }

    #[test]
    fn test_psg_reset() {
        let mut psg = SmsPsg::new();

        // Write some data
        psg.write(0x80); // Latch tone 0, data 0

        // Reset should clear everything
        psg.reset();

        // Cycle accumulator should be reset
        assert_eq!(psg.cycle_accum, 0.0);
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

        // Cycle accumulator should have some fractional value
        // (unless it happens to be exactly 0, which is unlikely)

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
