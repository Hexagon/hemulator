//! Texas Instruments SN76489 Programmable Sound Generator
//!
//! The SN76489 is a sound chip used in the Sega Master System, Game Gear,
//! and many other systems.
//!
//! # Architecture
//! - 3 square wave tone channels
//! - 1 noise channel
//! - 4-bit volume control per channel
//! - 10-bit frequency control for tone channels
//!
//! # Sega Variant (SN76496)
//! The Sega variant uses a 16-bit LFSR for noise (instead of 15-bit)

use crate::apu::{AudioChip, TimingMode};

/// SN76489 PSG state
pub struct Sn76489Psg {
    // Tone generators
    tone_freq: [u16; 3],    // 10-bit frequency values
    tone_counter: [u16; 3], // Current counter values
    tone_output: [bool; 3], // Current output state

    // Noise generator
    noise_control: u8,  // Noise control register
    noise_lfsr: u16,    // Linear feedback shift register (16-bit for Sega)
    noise_counter: u16, // Noise counter
    noise_output: bool, // Current noise output

    // Volume control (4-bit, 0=max, 15=min/mute)
    volume: [u8; 4],

    // Latched register
    latched_reg: u8,

    // Clock rate and timing
    timing_mode: TimingMode,
}

impl Sn76489Psg {
    /// Create a new SN76489 PSG
    ///
    /// # Arguments
    /// * `timing_mode` - NTSC or PAL timing mode
    pub fn new(timing_mode: TimingMode) -> Self {
        Self {
            tone_freq: [0; 3],
            tone_counter: [0; 3],
            tone_output: [false; 3],
            noise_control: 0,
            noise_lfsr: 0x8000, // Initial LFSR state
            noise_counter: 0,
            noise_output: false,
            volume: [0x0F; 4], // All channels muted initially
            latched_reg: 0,
            timing_mode,
        }
    }

    /// Write a byte to the PSG
    pub fn write(&mut self, data: u8) {
        if data & 0x80 != 0 {
            // Latch/data byte
            let channel = (data >> 5) & 0x03;
            let is_volume = (data >> 4) & 0x01;

            self.latched_reg = channel;

            if is_volume != 0 {
                // Volume write
                self.volume[channel as usize] = data & 0x0F;
            } else if channel == 3 {
                // Noise control
                self.noise_control = data & 0x07;
                self.noise_lfsr = 0x8000; // Reset LFSR
            } else {
                // Tone frequency (low 4 bits)
                let ch = channel as usize;
                self.tone_freq[ch] = (self.tone_freq[ch] & 0x3F0) | ((data & 0x0F) as u16);
            }
        } else {
            // Data byte (continuation of previous latch)
            let channel = self.latched_reg;
            if channel < 3 {
                // Tone frequency (high 6 bits)
                let ch = channel as usize;
                self.tone_freq[ch] = (self.tone_freq[ch] & 0x00F) | (((data & 0x3F) as u16) << 4);
            }
        }
    }

    /// Clock the PSG and generate samples
    fn clock_once(&mut self) {
        // Clock tone generators
        for i in 0..3 {
            if self.tone_counter[i] > 0 {
                self.tone_counter[i] -= 1;
            } else {
                // Reload counter
                self.tone_counter[i] = self.tone_freq[i];
                if self.tone_freq[i] > 0 {
                    self.tone_output[i] = !self.tone_output[i];
                }
            }
        }

        // Clock noise generator
        if self.noise_counter > 0 {
            self.noise_counter -= 1;
        } else {
            // Reload noise counter based on control register
            let noise_rate = self.noise_control & 0x03;
            self.noise_counter = match noise_rate {
                0 => 0x10,
                1 => 0x20,
                2 => 0x40,
                3 => self.tone_freq[2], // Use tone 2 frequency
                _ => unreachable!(),
            };

            // Clock LFSR
            let feedback = if (self.noise_control & 0x04) != 0 {
                // White noise (tapped)
                ((self.noise_lfsr & 1) ^ ((self.noise_lfsr >> 1) & 1)) != 0
            } else {
                // Periodic noise
                (self.noise_lfsr & 1) != 0
            };

            self.noise_lfsr >>= 1;
            if feedback {
                self.noise_lfsr |= 0x8000;
            }

            self.noise_output = (self.noise_lfsr & 1) != 0;
        }
    }

    /// Generate a single audio sample
    fn generate_sample(&self) -> i16 {
        let mut output = 0.0;

        // Mix tone channels
        for i in 0..3 {
            let amplitude = self.volume_to_amplitude(self.volume[i]);
            output += if self.tone_output[i] {
                amplitude
            } else {
                -amplitude
            };
        }

        // Mix noise channel
        let noise_amplitude = self.volume_to_amplitude(self.volume[3]);
        output += if self.noise_output {
            noise_amplitude
        } else {
            -noise_amplitude
        };

        // Average, normalize, and convert to i16
        let normalized = output / 4.0;
        (normalized * 32767.0) as i16
    }

    /// Convert 4-bit volume to amplitude (0=max, 15=min)
    fn volume_to_amplitude(&self, volume: u8) -> f32 {
        if volume == 0x0F {
            0.0 // Muted
        } else {
            // Exponential volume curve (approximately -2dB per step)
            let attenuation = volume as f32 * 2.0;
            10_f32.powf(-attenuation / 20.0)
        }
    }

    /// Reset the PSG to initial state
    pub fn reset_state(&mut self) {
        self.tone_freq.fill(0);
        self.tone_counter.fill(0);
        self.tone_output.fill(false);
        self.noise_control = 0;
        self.noise_lfsr = 0x8000;
        self.noise_counter = 0;
        self.noise_output = false;
        self.volume.fill(0x0F);
        self.latched_reg = 0;
    }
}

impl AudioChip for Sn76489Psg {
    fn write_register(&mut self, _addr: u16, val: u8) {
        // SMS writes to PSG via I/O port, not memory-mapped
        self.write(val);
    }

    fn clock(&mut self) -> i16 {
        self.clock_once();
        self.generate_sample()
    }

    fn timing(&self) -> TimingMode {
        self.timing_mode
    }

    fn reset(&mut self) {
        self.reset_state();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_psg_creation() {
        let psg = Sn76489Psg::new(TimingMode::Ntsc);
        assert_eq!(psg.volume, [0x0F; 4]); // All muted
    }

    #[test]
    fn test_psg_volume_write() {
        let mut psg = Sn76489Psg::new(TimingMode::Ntsc);

        // Latch tone 0, volume
        psg.write(0x90); // Channel 0, volume, value 0 (max)
        assert_eq!(psg.volume[0], 0x00);

        // Latch tone 1, volume 5
        psg.write(0xB5); // Channel 1, volume, value 5
        assert_eq!(psg.volume[1], 0x05);

        // Latch tone 2, volume F (mute)
        psg.write(0xDF); // Channel 2, volume, value F
        assert_eq!(psg.volume[2], 0x0F);
    }

    #[test]
    fn test_psg_tone_frequency() {
        let mut psg = Sn76489Psg::new(TimingMode::Ntsc);

        // Set channel 0 frequency to 0x1A4
        psg.write(0x84); // Latch tone 0, data, low 4 bits = 0x4
        psg.write(0x1A); // High 6 bits = 0x1A

        assert_eq!(psg.tone_freq[0], 0x1A4);
    }

    #[test]
    fn test_psg_noise_control() {
        let mut psg = Sn76489Psg::new(TimingMode::Ntsc);

        // Set noise to white noise, rate 3 (uses tone 2)
        psg.write(0xE7); // Latch noise, control = 0x7

        assert_eq!(psg.noise_control, 0x07);
    }

    #[test]
    fn test_volume_to_amplitude() {
        let psg = Sn76489Psg::new(TimingMode::Ntsc);

        // Volume 0 should be maximum amplitude
        let max_amp = psg.volume_to_amplitude(0);
        assert!(max_amp > 0.9);

        // Volume 15 should be muted
        assert_eq!(psg.volume_to_amplitude(15), 0.0);

        // Volume 8 should be approximately -16dB
        let vol_8 = psg.volume_to_amplitude(8);
        assert!(vol_8 > 0.15 && vol_8 < 0.17);
    }
}
