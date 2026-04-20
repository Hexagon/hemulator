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
//!
//! Both filters are generic over the float type (`f32` or `f64`) so each
//! consumer can pick the precision that matches its signal path.  The default
//! type parameter is `f32`, so existing call-sites written as
//! `DcBlockFilter::new(0.995)` continue to work unchanged.

use std::ops::{Add, Mul, Sub};

// ---------------------------------------------------------------------------
// FilterFloat – sealed trait marking the two supported float primitives
// ---------------------------------------------------------------------------

mod private {
    pub trait Sealed {}
    impl Sealed for f32 {}
    impl Sealed for f64 {}
}

/// Marker trait for float types supported by the audio filters.
///
/// Currently implemented for `f32` and `f64` only.
pub trait FilterFloat:
    private::Sealed
    + Copy
    + Default
    + PartialOrd
    + Add<Output = Self>
    + Sub<Output = Self>
    + Mul<Output = Self>
    + From<f32>
{
}
impl FilterFloat for f32 {}
impl FilterFloat for f64 {}

// ---------------------------------------------------------------------------
// DcBlockFilter
// ---------------------------------------------------------------------------

/// DC-blocking high-pass filter.
///
/// Removes DC offset from an audio signal using a first-order IIR difference
/// equation:
///
/// ```text
/// y[n] = x[n] - x[n-1] + α * y[n-1]
/// ```
///
/// The generic parameter `F` selects the internal float precision; it defaults
/// to `f32`.  Use `DcBlockFilter::<f64>` when the surrounding signal path
/// requires `f64` precision (e.g. the NES APU).
///
/// ## Valid `alpha` range
///
/// `alpha` must be in **[0.0, 1.0]**.  Values outside this range produce
/// unstable or nonsensical output.  Practical audio cut-off frequencies at
/// 44.1 kHz:
///
/// - `0.999` → ~7 Hz cutoff (NES)
/// - `0.995` → ~35 Hz cutoff (GB / GBA)
///
/// A `debug_assert!` guards the range in debug builds; release builds skip
/// the check for performance.
///
/// # Example
///
/// ```
/// use emu_core::apu::DcBlockFilter;
///
/// let mut filter = DcBlockFilter::new(0.995_f32);
/// let out = filter.process(1024.0_f32);
/// assert!(out.abs() <= 1024.0 + f32::EPSILON);
///
/// // f64 variant
/// let mut filter64 = DcBlockFilter::<f64>::new(0.999_f64);
/// let out64 = filter64.process(1024.0_f64);
/// assert!(out64.abs() <= 1024.0 + f64::EPSILON);
/// ```
#[derive(Debug, Clone)]
pub struct DcBlockFilter<F: FilterFloat = f32> {
    alpha: F,
    prev_in: F,
    prev_out: F,
}

impl<F: FilterFloat> DcBlockFilter<F> {
    /// Create a new DC-blocking filter with the given `alpha` coefficient.
    ///
    /// `alpha` must be in **[0.0, 1.0]**.  Values close to 1.0 give a very
    /// low cut-off (removes only very slow drift); lower values remove more
    /// low-frequency content.
    ///
    /// # Panics (debug builds only)
    ///
    /// Panics if `alpha` is outside [0.0, 1.0].
    pub fn new(alpha: F) -> Self {
        debug_assert!(
            alpha >= F::from(0.0_f32) && alpha <= F::from(1.0_f32),
            "DcBlockFilter alpha must be in [0.0, 1.0]"
        );
        Self {
            alpha,
            prev_in: F::default(),
            prev_out: F::default(),
        }
    }

    /// Process one sample, returning the high-pass filtered output.
    #[inline]
    pub fn process(&mut self, input: F) -> F {
        let y = input - self.prev_in + self.alpha * self.prev_out;
        self.prev_in = input;
        self.prev_out = y;
        y
    }

    /// Reset filter state to zero (e.g. on system reset / power cycle).
    pub fn reset(&mut self) {
        self.prev_in = F::default();
        self.prev_out = F::default();
    }
}

impl Default for DcBlockFilter<f32> {
    /// Returns a DC-blocking filter with `alpha = 0.995` (~35 Hz at 44.1 kHz).
    fn default() -> Self {
        Self::new(0.995_f32)
    }
}

impl Default for DcBlockFilter<f64> {
    /// Returns a DC-blocking filter with `alpha = 0.995` (~35 Hz at 44.1 kHz).
    fn default() -> Self {
        Self::new(0.995_f64)
    }
}

// ---------------------------------------------------------------------------
// LowPassFilter
// ---------------------------------------------------------------------------

/// One-pole low-pass filter.
///
/// Smooths an audio signal using a first-order IIR low-pass equation:
///
/// ```text
/// y[n] = y[n-1] + c * (x[n] - y[n-1])
/// ```
///
/// The generic parameter `F` selects the internal float precision; it defaults
/// to `f32`.
///
/// ## Valid `coefficient` range
///
/// `coefficient` must be in **(0.0, 1.0]**.  A coefficient of `0.0` would
/// freeze the output at its initial value (always zero after creation),
/// while values above `1.0` are unstable and can amplify the signal.  A
/// `debug_assert!` guards the range in debug builds.
///
/// Typical value: `0.08` (strong smoothing, used by the GB APU).
///
/// # Example
///
/// ```
/// use emu_core::apu::LowPassFilter;
///
/// let mut filter = LowPassFilter::new(0.08_f32);
/// let out = filter.process(32767.0_f32);
/// assert!(out >= 0.0 && out <= 32767.0);
/// ```
#[derive(Debug, Clone)]
pub struct LowPassFilter<F: FilterFloat = f32> {
    coefficient: F,
    prev: F,
}

impl<F: FilterFloat> LowPassFilter<F> {
    /// Create a new low-pass filter with the given smoothing `coefficient`.
    ///
    /// `coefficient` must be in **(0.0, 1.0]**.  Smaller values give stronger
    /// smoothing (lower cut-off).
    ///
    /// # Panics (debug builds only)
    ///
    /// Panics if `coefficient` is outside (0.0, 1.0].
    pub fn new(coefficient: F) -> Self {
        debug_assert!(
            coefficient > F::default() && coefficient <= F::from(1.0_f32),
            "LowPassFilter coefficient must be in (0.0, 1.0]"
        );
        Self {
            coefficient,
            prev: F::default(),
        }
    }

    /// Process one sample, returning the low-pass filtered output.
    #[inline]
    pub fn process(&mut self, input: F) -> F {
        let y = self.prev + self.coefficient * (input - self.prev);
        self.prev = y;
        y
    }

    /// Reset filter state to zero.
    pub fn reset(&mut self) {
        self.prev = F::default();
    }
}

impl Default for LowPassFilter<f32> {
    /// Returns a low-pass filter with `coefficient = 0.08`.
    fn default() -> Self {
        Self::new(0.08_f32)
    }
}

impl Default for LowPassFilter<f64> {
    /// Returns a low-pass filter with `coefficient = 0.08`.
    fn default() -> Self {
        Self::new(0.08_f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dc_block_removes_dc_offset() {
        let mut filter = DcBlockFilter::new(0.995_f32);

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
    fn dc_block_removes_dc_offset_f64() {
        let mut filter = DcBlockFilter::<f64>::new(0.999);

        for _ in 0..5000 {
            filter.process(1.0);
        }
        let out = filter.process(1.0);
        assert!(
            out.abs() < 0.01,
            "f64 DC block should converge near zero for constant input, got {out}"
        );
    }

    #[test]
    fn dc_block_passes_ac_signal() {
        let mut filter = DcBlockFilter::new(0.995_f32);

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
        let mut filter = DcBlockFilter::new(0.995_f32);
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
        let mut filter = LowPassFilter::new(0.08_f32);

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
        let mut filter = LowPassFilter::new(0.08_f32);
        for _ in 0..100 {
            filter.process(1000.0);
        }
        filter.reset();
        let out = filter.process(0.0);
        assert_eq!(out, 0.0, "After reset, zero input should give zero output");
    }
}
