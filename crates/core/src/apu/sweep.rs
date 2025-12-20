//! Frequency sweep unit for pulse channels.
//!
//! This module implements a frequency sweep unit that can automatically adjust
//! the frequency of a sound channel over time. Used in the Game Boy APU pulse 1 channel
//! and potentially reusable in other systems with frequency modulation.

/// Sweep unit that automatically adjusts frequency over time.
///
/// The sweep unit supports:
/// - Increase or decrease frequency
/// - Configurable shift amount (0-7)
/// - Configurable sweep period (0-7)
/// - Overflow detection (frequency too high)
#[derive(Debug, Clone)]
pub struct SweepUnit {
    /// Whether sweep is enabled
    pub enabled: bool,
    /// Sweep period (0-7) - how often to apply sweep
    pub period: u8,
    /// Period timer - counts down from period
    timer: u8,
    /// Negate flag: false = increase frequency, true = decrease frequency
    pub negate: bool,
    /// Shift amount (0-7) - how much to change frequency
    pub shift: u8,
    /// Current frequency value being swept
    frequency: u16,
    /// Shadow frequency register for overflow checking
    shadow_frequency: u16,
    /// Whether sweep has been triggered
    triggered: bool,
}

impl SweepUnit {
    /// Create a new sweep unit with default state
    pub fn new() -> Self {
        Self {
            enabled: false,
            period: 0,
            timer: 0,
            negate: false,
            shift: 0,
            frequency: 0,
            shadow_frequency: 0,
            triggered: false,
        }
    }

    /// Clock the sweep unit (called at sweep rate, e.g., 128 Hz on Game Boy)
    pub fn clock(&mut self) -> Option<u16> {
        if self.timer > 0 {
            self.timer -= 1;
        }

        if self.timer == 0 {
            self.timer = if self.period > 0 { self.period } else { 8 };

            if self.enabled && self.period > 0 {
                let new_freq = self.calculate_frequency();
                
                // Check for overflow (frequency >= 2048 on Game Boy)
                if new_freq < 2048 && self.shift > 0 {
                    self.frequency = new_freq;
                    self.shadow_frequency = new_freq;
                    
                    // Perform overflow check again
                    let _ = self.calculate_frequency();
                    
                    return Some(new_freq);
                }
            }
        }

        None
    }

    /// Calculate the new frequency based on current sweep settings
    fn calculate_frequency(&self) -> u16 {
        let delta = self.shadow_frequency >> self.shift;
        
        if self.negate {
            // Decrease frequency
            self.shadow_frequency.saturating_sub(delta)
        } else {
            // Increase frequency
            self.shadow_frequency.saturating_add(delta)
        }
    }

    /// Trigger the sweep unit (when channel is triggered)
    pub fn trigger(&mut self, frequency: u16) {
        self.shadow_frequency = frequency;
        self.frequency = frequency;
        self.timer = if self.period > 0 { self.period } else { 8 };
        self.enabled = self.period > 0 || self.shift > 0;
        self.triggered = true;

        // Perform initial overflow check
        if self.shift > 0 {
            let _ = self.calculate_frequency();
        }
    }

    /// Get the current frequency
    pub fn get_frequency(&self) -> u16 {
        self.frequency
    }

    /// Set sweep parameters
    pub fn set_params(&mut self, period: u8, negate: bool, shift: u8) {
        self.period = period & 0x07;
        self.negate = negate;
        self.shift = shift & 0x07;
    }

    /// Check if frequency would overflow (>= 2048)
    pub fn will_overflow(&self) -> bool {
        self.calculate_frequency() >= 2048
    }

    /// Reset the sweep unit
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

impl Default for SweepUnit {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sweep_increases_frequency() {
        let mut sweep = SweepUnit::new();
        sweep.set_params(1, false, 1); // Period 1, increase, shift 1
        sweep.trigger(100);
        
        // Initial frequency
        assert_eq!(sweep.get_frequency(), 100);
        
        // Clock once to decrement timer
        sweep.clock();
        
        // Timer should be 0, sweep should apply
        // New frequency = 100 + (100 >> 1) = 100 + 50 = 150
        assert_eq!(sweep.get_frequency(), 150);
    }

    #[test]
    fn sweep_decreases_frequency() {
        let mut sweep = SweepUnit::new();
        sweep.set_params(1, true, 1); // Period 1, decrease, shift 1
        sweep.trigger(100);
        
        assert_eq!(sweep.get_frequency(), 100);
        
        sweep.clock();
        
        // New frequency = 100 - (100 >> 1) = 100 - 50 = 50
        assert_eq!(sweep.get_frequency(), 50);
    }

    #[test]
    fn sweep_respects_period() {
        let mut sweep = SweepUnit::new();
        sweep.set_params(3, false, 1); // Period 3
        sweep.trigger(100);
        
        // Clock twice - frequency should not change yet
        sweep.clock();
        sweep.clock();
        assert_eq!(sweep.get_frequency(), 100);
        
        // Clock third time - frequency should change
        sweep.clock();
        assert_eq!(sweep.get_frequency(), 150);
    }

    #[test]
    fn sweep_shift_zero_does_nothing() {
        let mut sweep = SweepUnit::new();
        sweep.set_params(1, false, 0); // Shift 0
        sweep.trigger(100);
        
        sweep.clock();
        
        // Frequency should not change with shift = 0
        assert_eq!(sweep.get_frequency(), 100);
    }

    #[test]
    fn sweep_overflow_detection() {
        let mut sweep = SweepUnit::new();
        sweep.set_params(1, false, 1); // Increase by half each time
        sweep.trigger(1500);
        
        // 1500 + (1500 >> 1) = 1500 + 750 = 2250, which is >= 2048 (overflow)
        assert!(sweep.will_overflow());
        
        // After sweep: frequency should not change due to overflow
        sweep.clock();
        
        // Frequency should not have changed due to overflow
        assert_eq!(sweep.get_frequency(), 1500);
    }

    #[test]
    fn sweep_period_zero_uses_default() {
        let mut sweep = SweepUnit::new();
        sweep.set_params(0, false, 1); // Period 0
        sweep.trigger(100);
        
        // With period 0, timer should use 8
        assert_eq!(sweep.timer, 8);
    }

    #[test]
    fn sweep_disabled_when_period_and_shift_zero() {
        let mut sweep = SweepUnit::new();
        sweep.set_params(0, false, 0);
        sweep.trigger(100);
        
        assert!(!sweep.enabled);
    }

    #[test]
    fn sweep_enabled_when_period_or_shift_nonzero() {
        let mut sweep = SweepUnit::new();
        sweep.set_params(1, false, 0);
        sweep.trigger(100);
        
        // Should be enabled with period > 0
        assert!(sweep.enabled);
        
        sweep.reset();
        sweep.set_params(0, false, 1);
        sweep.trigger(100);
        
        // Should be enabled with shift > 0
        assert!(sweep.enabled);
    }
}
