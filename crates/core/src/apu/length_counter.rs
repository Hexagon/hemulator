//! Length counter used by pulse, triangle, and noise channels.
//!
//! The length counter provides automatic note duration control.

/// NES length counter lookup table.
/// 
/// This table is indexed by a 5-bit value (0-31) and returns the length counter value.
/// The counter is clocked at half the frame counter rate (~120Hz NTSC, ~100Hz PAL).
pub const LENGTH_TABLE: [u8; 32] = [
    10, 254, 20, 2, 40, 4, 80, 6, 160, 8, 60, 10, 14, 12, 26, 14, 
    12, 16, 24, 18, 48, 20, 96, 22, 192, 24, 72, 26, 16, 28, 32, 30,
];

/// Length counter component.
/// 
/// Automatically decrements and can be halted. When it reaches zero,
/// the associated channel is silenced.
#[derive(Debug, Clone)]
pub struct LengthCounter {
    /// Current counter value
    value: u8,
    /// Halt flag (when true, counter doesn't decrement)
    halt: bool,
    /// Enabled flag (when false, counter is set to 0)
    enabled: bool,
}

impl LengthCounter {
    pub fn new() -> Self {
        Self {
            value: 0,
            halt: false,
            enabled: false,
        }
    }

    /// Clock the length counter (decrement if not halted)
    pub fn clock(&mut self) {
        if !self.halt && self.enabled && self.value > 0 {
            self.value -= 1;
        }
    }

    /// Load a new value from the length table
    pub fn load(&mut self, index: u8) {
        if self.enabled {
            self.value = LENGTH_TABLE[(index & 0x1F) as usize];
        }
    }

    /// Get the current counter value
    pub fn value(&self) -> u8 {
        self.value
    }

    /// Set the halt flag
    pub fn set_halt(&mut self, halt: bool) {
        self.halt = halt;
    }

    /// Set the enabled flag
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.value = 0;
        }
    }

    /// Check if the counter is non-zero (channel should be active)
    pub fn is_active(&self) -> bool {
        self.value > 0
    }
}

impl Default for LengthCounter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn length_counter_decrements() {
        let mut lc = LengthCounter::new();
        lc.set_enabled(true);
        lc.load(0); // Load value 10 (from LENGTH_TABLE[0])
        
        assert_eq!(lc.value(), 10);
        lc.clock();
        assert_eq!(lc.value(), 9);
        lc.clock();
        assert_eq!(lc.value(), 8);
    }

    #[test]
    fn length_counter_halt_prevents_decrement() {
        let mut lc = LengthCounter::new();
        lc.set_enabled(true);
        lc.load(0); // Load value 10
        lc.set_halt(true);

        let initial = lc.value();
        lc.clock();
        assert_eq!(lc.value(), initial); // Should not decrement
    }

    #[test]
    fn length_counter_disabled_zeros_value() {
        let mut lc = LengthCounter::new();
        lc.set_enabled(true);
        lc.load(0); // Load value 10
        
        assert_eq!(lc.value(), 10);
        lc.set_enabled(false);
        assert_eq!(lc.value(), 0);
    }

    #[test]
    fn length_counter_load_when_disabled() {
        let mut lc = LengthCounter::new();
        lc.set_enabled(false);
        lc.load(1); // Try to load value 254

        assert_eq!(lc.value(), 0); // Should remain 0 when disabled
    }

    #[test]
    fn length_counter_table_values() {
        // Verify some key values in the length table
        assert_eq!(LENGTH_TABLE[0], 10);
        assert_eq!(LENGTH_TABLE[1], 254);
        assert_eq!(LENGTH_TABLE[31], 30);
    }
}
