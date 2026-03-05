//! Reusable audio DSP filter utilities.
//!
//! Provides common signal-processing filters used across multiple emulated
//! audio processing units (APUs).  Centralising them here avoids duplicating
//! the same filter logic in each system crate.
//!
//! ## Filters
//!
//! - [`DcBlockFilter`] – first-order high-pass IIR filter that removes DC offset.
//! - [`LowPassFilter`] – one-pole low-pass IIR filter for smoothing harsh edges.

/// DC-blocking high-pass filter.
///
/// Removes DC offset from an audio signal using a first-order IIR difference
/// equation:
///
/// ```text
/// y[n] = x[n] - x[n-1] + α * y[n-1]
/// ```
///
/// Typical `alpha` values (at 44.1 kHz sample rate):
/// - `0.999` → ~7 Hz cutoff (used by NES)
/// - `0.995` → ~35 Hz cutoff (used by GB / GBA)
///
/// # Example
///
/// ```
/// use emu_core::apu::DcBlockFilter;
///
/// let mut filter = DcBlockFilter::new(0.995);
/// let out = filter.process(1024.0);
/// assert!(out.abs() <= 1024.0 + f32::EPSILON);
/// ```
#[derive(Debug, Clone)]
pub struct DcBlockFilter {
    alpha: f32,
    prev_in: f32,
    prev_out: f32,
}

impl DcBlockFilter {
    /// Create a new DC-blocking filter with the given `alpha` coefficient.
    ///
    /// `alpha` controls the cut-off frequency.  Values close to 1.0 give a
    /// very low cut-off (removes only very slow drift); lower values remove
    /// more of the low-frequency content as well.
    pub fn new(alpha: f32) -> Self {
        Self {
            alpha,
            prev_in: 0.0,
            prev_out: 0.0,
        }
    }

    /// Process one sample, returning the high-pass filtered output.
    #[inline]
    pub fn process(&mut self, input: f32) -> f32 {
        let y = input - self.prev_in + self.alpha * self.prev_out;
        self.prev_in = input;
        self.prev_out = y;
        y
    }

    /// Reset filter state to zero (e.g. on system reset / power cycle).
    pub fn reset(&mut self) {
        self.prev_in = 0.0;
        self.prev_out = 0.0;
    }
}

impl Default for DcBlockFilter {
    /// Returns a DC-blocking filter with `alpha = 0.995` (~35 Hz at 44.1 kHz).
    fn default() -> Self {
        Self::new(0.995)
    }
}

/// One-pole low-pass filter.
///
/// Smooths an audio signal using a first-order IIR low-pass equation:
///
/// ```text
/// y[n] = y[n-1] + c * (x[n] - y[n-1])
/// ```
///
/// where `c` is the smoothing coefficient in the range (0, 1).  Smaller
/// values give stronger smoothing (lower cut-off).
///
/// # Example
///
/// ```
/// use emu_core::apu::LowPassFilter;
///
/// let mut filter = LowPassFilter::new(0.08);
/// let out = filter.process(32767.0);
/// assert!(out >= 0.0 && out <= 32767.0);
/// ```
#[derive(Debug, Clone)]
pub struct LowPassFilter {
    coefficient: f32,
    prev: f32,
}

impl LowPassFilter {
    /// Create a new low-pass filter with the given smoothing `coefficient`.
    pub fn new(coefficient: f32) -> Self {
        Self {
            coefficient,
            prev: 0.0,
        }
    }

    /// Process one sample, returning the low-pass filtered output.
    #[inline]
    pub fn process(&mut self, input: f32) -> f32 {
        let y = self.prev + self.coefficient * (input - self.prev);
        self.prev = y;
        y
    }

    /// Reset filter state to zero.
    pub fn reset(&mut self) {
        self.prev = 0.0;
    }
}

impl Default for LowPassFilter {
    /// Returns a low-pass filter with `coefficient = 0.08`.
    fn default() -> Self {
        Self::new(0.08)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dc_block_removes_dc_offset() {
        let mut filter = DcBlockFilter::new(0.995);

        // Feed a constant non-zero value; after many samples the output
        // should converge towards zero.
        for _ in 0..2000 {
            filter.process(1000.0);
        }
        let out = filter.process(1000.0);
        assert!(
            out.abs() < 50.0,
            "DC block should converge near zero for constant input, got {out}"
        );
    }

    #[test]
    fn dc_block_passes_ac_signal() {
        let mut filter = DcBlockFilter::new(0.995);

        // An alternating +/- signal should pass through largely unchanged after
        // the filter has settled.
        let mut sum = 0.0f32;
        for i in 0..100 {
            let input = if i % 2 == 0 { 1000.0 } else { -1000.0 };
            sum += filter.process(input).abs();
        }
        // Average magnitude should be close to the input magnitude.
        let avg = sum / 100.0;
        assert!(
            avg > 500.0,
            "DC block should pass AC signal, avg magnitude was {avg}"
        );
    }

    #[test]
    fn dc_block_reset_clears_state() {
        let mut filter = DcBlockFilter::new(0.995);
        // Run for a while so internal state is non-zero.
        for _ in 0..100 {
            filter.process(500.0);
        }
        filter.reset();
        // Immediately after reset, a zero input should yield a zero output.
        let out = filter.process(0.0);
        assert_eq!(out, 0.0, "After reset, zero input should give zero output");
    }

    #[test]
    fn low_pass_smooths_step() {
        let mut filter = LowPassFilter::new(0.08);

        // Step input: value jumps from 0 to 1000.
        // Output must approach 1000 but never exceed it.
        let mut last = 0.0f32;
        for _ in 0..500 {
            last = filter.process(1000.0);
        }
        assert!(
            (last - 1000.0).abs() < 1.0,
            "LP filter should settle near 1000.0 after many steps, got {last}"
        );
    }

    #[test]
    fn low_pass_reset_clears_state() {
        let mut filter = LowPassFilter::new(0.08);
        for _ in 0..100 {
            filter.process(1000.0);
        }
        filter.reset();
        let out = filter.process(0.0);
        assert_eq!(out, 0.0, "After reset, zero input should give zero output");
    }
}
