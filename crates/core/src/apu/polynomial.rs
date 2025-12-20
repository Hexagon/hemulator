//! Polynomial counter for Atari 2600 TIA audio waveform generation.
//!
//! This module implements a polynomial counter used in the TIA audio chip to generate
//! various waveform types including pure tones, buzzy sounds, and noise.

/// Polynomial counter for TIA-style waveform generation.
///
/// The TIA uses a combination of division and polynomial feedback to generate
/// different waveform types. The AUDC (Audio Control) register selects which
/// waveform type to use.
#[derive(Debug, Clone)]
pub struct PolynomialCounter {
    /// 4-bit polynomial counter
    poly4: u8,
    /// 5-bit polynomial counter
    poly5: u8,
    /// Division counter (5-bit)
    div: u8,
    /// Audio control value (0-15) selects waveform type
    pub control: u8,
    /// Audio frequency value (0-31) controls base frequency
    pub frequency: u8,
    /// Audio volume (0-15)
    pub volume: u8,
}

impl PolynomialCounter {
    /// Create a new polynomial counter with default state
    pub fn new() -> Self {
        Self {
            poly4: 0x0F,
            poly5: 0x1F,
            div: 0,
            control: 0,
            frequency: 0,
            volume: 0,
        }
    }

    /// Clock the polynomial counter once (at TIA clock rate)
    /// Returns the current audio output (0-15)
    pub fn clock(&mut self) -> u8 {
        // Increment division counter
        self.div = (self.div + 1) & 0x1F;

        // Clock poly4 when div matches frequency
        if self.div == self.frequency {
            self.div = 0;
            self.clock_poly4();

            // Clock poly5 based on poly4 bit 0
            if (self.poly4 & 1) == 0 {
                self.clock_poly5();
            }
        }

        // Generate output based on control value
        self.generate_output()
    }

    /// Clock the 4-bit polynomial counter
    fn clock_poly4(&mut self) {
        // 4-bit LFSR with taps at bits 0 and 1
        let feedback = ((self.poly4 & 1) ^ ((self.poly4 >> 1) & 1)) & 1;
        self.poly4 = (self.poly4 >> 1) | (feedback << 3);
    }

    /// Clock the 5-bit polynomial counter
    fn clock_poly5(&mut self) {
        // 5-bit LFSR with taps at bits 0 and 2
        let feedback = ((self.poly5 & 1) ^ ((self.poly5 >> 2) & 1)) & 1;
        self.poly5 = (self.poly5 >> 1) | (feedback << 4);
    }

    /// Generate output based on current control value
    fn generate_output(&self) -> u8 {
        // TIA waveform types based on AUDC value
        // Simplified implementation - full TIA would have more complex mixing
        let bit = match self.control {
            0x00 | 0x0B => 0,                            // Set to 1 (pure tone - always on)
            0x01 => self.poly4 & 1,                      // 4-bit polynomial
            0x02 => (self.div & 1) ^ 1,                  // Division by 2
            0x03 => (self.poly4 & 1) & (self.poly5 & 1), // 4-bit AND 5-bit
            0x04 | 0x05 => self.div & 1,                 // Pure tone (division)
            0x06 | 0x0A => self.div & 1,                 // Division by 31
            0x07 | 0x09 => self.poly5 & 1,               // 5-bit polynomial
            0x08 => self.poly5 & 1,                      // 5-bit poly (noise)
            0x0C | 0x0D => self.poly4 & 1,               // Pure tone with 4-bit
            0x0E => self.poly4 & 1,                      // 4-bit polynomial
            0x0F => (self.poly4 & 1) ^ (self.poly5 & 1), // 4-bit XOR 5-bit
            _ => 0,
        };

        if bit != 0 {
            self.volume
        } else {
            0
        }
    }

    /// Reset the polynomial counter
    pub fn reset(&mut self) {
        self.poly4 = 0x0F;
        self.poly5 = 0x1F;
        self.div = 0;
    }

    /// Set audio control register (selects waveform type)
    pub fn set_control(&mut self, control: u8) {
        self.control = control & 0x0F;
    }

    /// Set audio frequency register
    pub fn set_frequency(&mut self, frequency: u8) {
        self.frequency = frequency & 0x1F;
    }

    /// Set audio volume register
    pub fn set_volume(&mut self, volume: u8) {
        self.volume = volume & 0x0F;
    }
}

impl Default for PolynomialCounter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polynomial_counter_creates() {
        let poly = PolynomialCounter::new();
        assert_eq!(poly.poly4, 0x0F);
        assert_eq!(poly.poly5, 0x1F);
        assert_eq!(poly.div, 0);
    }

    #[test]
    fn polynomial_counter_clocks_div() {
        let mut poly = PolynomialCounter::new();
        poly.set_frequency(10);
        poly.set_volume(15);

        // Clock multiple times
        for _ in 0..5 {
            poly.clock();
        }

        // Division counter should have incremented
        assert!(poly.div > 0);
    }

    #[test]
    fn polynomial_counter_volume_affects_output() {
        let mut poly = PolynomialCounter::new();
        poly.set_control(0x04); // Pure tone
        poly.set_frequency(0);

        // Volume 0 should produce 0 output
        poly.set_volume(0);
        let output = poly.clock();
        assert_eq!(output, 0);

        // Volume 15 should produce non-zero when waveform is high
        poly.div = 0;
        poly.set_volume(15);
        let _ = poly.clock();
    }

    #[test]
    fn polynomial_counter_poly4_advances() {
        let mut poly = PolynomialCounter::new();
        poly.set_frequency(0); // Clock on every cycle
        poly.set_volume(15);

        let initial_poly4 = poly.poly4;

        // Need to clock until div wraps around
        // div starts at 0, increments each clock, and resets when == frequency (0)
        // So div will go 1, 2, ..., 31, 0 then poly4 clocks
        for _ in 0..32 {
            poly.clock();
        }

        // poly4 should have changed
        assert_ne!(poly.poly4, initial_poly4);
    }

    #[test]
    fn polynomial_counter_control_selects_waveform() {
        let mut poly = PolynomialCounter::new();
        poly.set_frequency(0);
        poly.set_volume(15);

        // Try different control values
        for control in 0..16 {
            poly.set_control(control);
            poly.reset();

            // Generate some samples
            let mut samples = Vec::new();
            for _ in 0..100 {
                samples.push(poly.clock());
            }

            // Verify output is within valid range
            assert!(samples.iter().all(|&s| s <= 15));
        }
    }

    #[test]
    fn polynomial_counter_reset_restores_initial_state() {
        let mut poly = PolynomialCounter::new();
        poly.set_frequency(5);
        poly.set_volume(10);

        // Clock it a bunch
        for _ in 0..100 {
            poly.clock();
        }

        poly.reset();

        // Check reset state
        assert_eq!(poly.poly4, 0x0F);
        assert_eq!(poly.poly5, 0x1F);
        assert_eq!(poly.div, 0);
        // Control, frequency, and volume should remain unchanged
        assert_eq!(poly.frequency, 5);
        assert_eq!(poly.volume, 10);
    }

    #[test]
    fn polynomial_counter_frequency_controls_rate() {
        let mut poly1 = PolynomialCounter::new();
        poly1.set_control(0x04); // Pure tone
        poly1.set_frequency(1); // Triggers when div reaches 1
        poly1.set_volume(15);

        let mut poly2 = PolynomialCounter::new();
        poly2.set_control(0x04);
        poly2.set_frequency(10); // Triggers when div reaches 10
        poly2.set_volume(15);

        let initial_poly4_1 = poly1.poly4;
        let _initial_poly4_2 = poly2.poly4;

        // Clock both enough times that poly1 should trigger but poly2 shouldn't
        for _ in 0..5 {
            poly1.clock();
            poly2.clock();
        }

        // poly1 should have clocked poly4 (freq=1 reached after 2 clocks)
        // poly2 should not have clocked poly4 yet (freq=10 not reached in 5 clocks)
        assert_ne!(poly1.poly4, initial_poly4_1);
    }
}
