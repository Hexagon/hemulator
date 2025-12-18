//! Noise channel with Linear Feedback Shift Register (LFSR).
//!
//! The noise channel generates pseudo-random noise for percussion and sound effects.

/// Noise channel that generates pseudo-random noise.
///
/// The noise channel uses a 15-bit LFSR to generate pseudo-random bit sequences.
/// It supports:
/// - Two noise modes (normal and periodic/metallic)
/// - 16 preset period values
/// - Length counter
/// - Envelope generator for volume control
#[derive(Debug, Clone)]
pub struct NoiseChannel {
    /// Whether the channel is enabled
    pub enabled: bool,
    /// Mode flag: false = normal, true = periodic (short LFSR period)
    pub mode: bool,
    /// Period index (0-15) into period lookup table
    pub period_index: u8,
    /// Timer counter
    timer: u16,
    /// 15-bit Linear Feedback Shift Register
    shift_register: u16,
    /// Length counter
    pub length_counter: u8,
    /// Envelope volume (4-bit, 0-15)
    pub envelope: u8,
    /// Whether to use constant volume (true) or envelope (false)
    pub constant_volume: bool,
    /// Length counter halt / envelope loop flag
    pub length_counter_halt: bool,
}

/// NTSC noise period lookup table
const NOISE_PERIOD_TABLE_NTSC: [u16; 16] = [
    4, 8, 16, 32, 64, 96, 128, 160, 202, 254, 380, 508, 762, 1016, 2034, 4068,
];

/// PAL noise period lookup table
const NOISE_PERIOD_TABLE_PAL: [u16; 16] = [
    4, 8, 14, 30, 60, 88, 118, 148, 188, 236, 354, 472, 708, 944, 1890, 3778,
];

impl NoiseChannel {
    pub fn new() -> Self {
        Self {
            enabled: false,
            mode: false,
            period_index: 0,
            timer: 0,
            shift_register: 1, // Initialize with non-zero value
            length_counter: 0,
            envelope: 15,
            constant_volume: false,
            length_counter_halt: false,
        }
    }

    /// Clock the noise channel for one CPU cycle (NTSC timing)
    pub fn clock(&mut self) -> i16 {
        self.clock_with_table(&NOISE_PERIOD_TABLE_NTSC)
    }

    /// Clock the noise channel with a specific period table (for PAL support)
    pub fn clock_with_table(&mut self, period_table: &[u16; 16]) -> i16 {
        // Output based on bit 0 of the shift register
        let sample = if self.enabled && self.length_counter > 0 && (self.shift_register & 1) == 0 {
            (self.envelope as i16) << 10
        } else {
            0
        };

        // Timer countdown
        if self.timer > 0 {
            self.timer -= 1;
        } else {
            // Reload timer
            self.timer = period_table[self.period_index as usize & 0x0F];

            // Clock the LFSR
            let feedback = if self.mode {
                // Mode 1: feedback from bits 0 and 6 (short period, metallic sound)
                ((self.shift_register & 1) ^ ((self.shift_register >> 6) & 1)) & 1
            } else {
                // Mode 0: feedback from bits 0 and 1 (long period, white noise)
                ((self.shift_register & 1) ^ ((self.shift_register >> 1) & 1)) & 1
            };

            self.shift_register >>= 1;
            self.shift_register |= feedback << 14;
        }

        sample
    }

    /// Set the period index (0-15)
    pub fn set_period(&mut self, index: u8) {
        self.period_index = index & 0x0F;
    }

    /// Reset the shift register
    pub fn reset_shift_register(&mut self) {
        self.shift_register = 1;
    }
}

impl Default for NoiseChannel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_lfsr_generates_sequence() {
        let mut noise = NoiseChannel::new();
        noise.enabled = true;
        noise.length_counter = 10;
        noise.envelope = 15;
        noise.set_period(0); // Shortest period
        noise.shift_register = 1;

        // Clock multiple times and verify shift register changes
        let initial_sr = noise.shift_register;
        for _ in 0..10 {
            noise.clock();
        }
        // Shift register should have changed
        assert_ne!(noise.shift_register, initial_sr);
    }

    #[test]
    fn noise_mode_affects_feedback() {
        let mut noise1 = NoiseChannel::new();
        noise1.enabled = true;
        noise1.length_counter = 10;
        noise1.mode = false; // Normal mode
        noise1.set_period(0);
        noise1.shift_register = 0b101010101010101; // Pattern with mixed bits

        let mut noise2 = NoiseChannel::new();
        noise2.enabled = true;
        noise2.length_counter = 10;
        noise2.mode = true; // Periodic mode
        noise2.set_period(0);
        noise2.shift_register = 0b101010101010101; // Same initial pattern

        // Clock both and check they produce different sequences
        for _ in 0..50 {
            noise1.clock();
            noise2.clock();
        }

        // After clocking, the shift registers should differ due to different feedback taps
        assert_ne!(noise1.shift_register, noise2.shift_register);
    }

    #[test]
    fn noise_length_counter_mutes() {
        let mut noise = NoiseChannel::new();
        noise.enabled = true;
        noise.length_counter = 0;
        noise.envelope = 15;

        let sample = noise.clock();
        assert_eq!(sample, 0); // Should be muted
    }

    #[test]
    fn noise_disabled_mutes() {
        let mut noise = NoiseChannel::new();
        noise.enabled = false;
        noise.length_counter = 10;
        noise.envelope = 15;

        let sample = noise.clock();
        assert_eq!(sample, 0); // Should be muted
    }
}
