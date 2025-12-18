//! Envelope generator for volume control.
//!
//! The envelope generator provides automatic volume fade-out for pulse and noise channels.

/// Envelope generator component.
/// 
/// Provides automatic volume control with decay from 15 to 0.
/// Can also be used for constant volume mode.
#[derive(Debug, Clone)]
pub struct Envelope {
    /// Start flag (set when a note is triggered)
    start_flag: bool,
    /// Decay level counter (0-15)
    decay_level: u8,
    /// Divider counter
    divider: u8,
    /// Divider period (reload value from register)
    period: u8,
    /// Loop flag (restart envelope when it reaches 0)
    loop_flag: bool,
}

impl Envelope {
    pub fn new() -> Self {
        Self {
            start_flag: false,
            decay_level: 0,
            divider: 0,
            period: 0,
            loop_flag: false,
        }
    }

    /// Clock the envelope (called by frame counter at ~240Hz NTSC)
    pub fn clock(&mut self) {
        if self.start_flag {
            self.start_flag = false;
            self.decay_level = 15;
            self.divider = self.period;
        } else if self.divider > 0 {
            self.divider -= 1;
        } else {
            self.divider = self.period;
            if self.decay_level > 0 {
                self.decay_level -= 1;
            } else if self.loop_flag {
                self.decay_level = 15;
            }
        }
    }

    /// Get the current decay level (0-15)
    pub fn level(&self) -> u8 {
        self.decay_level
    }

    /// Restart the envelope
    pub fn restart(&mut self) {
        self.start_flag = true;
    }

    /// Set the period (divider reload value)
    pub fn set_period(&mut self, period: u8) {
        self.period = period & 0x0F;
    }

    /// Set the loop flag
    pub fn set_loop(&mut self, loop_flag: bool) {
        self.loop_flag = loop_flag;
    }
}

impl Default for Envelope {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_restart_sets_level_to_15() {
        let mut env = Envelope::new();
        env.set_period(1);
        env.restart();
        env.clock(); // Process the start flag
        assert_eq!(env.level(), 15);
    }

    #[test]
    fn envelope_decays_to_zero() {
        let mut env = Envelope::new();
        env.set_period(0); // Fastest decay
        env.restart();
        env.clock(); // Process start flag, level = 15

        // Clock until decay reaches 0
        for _ in 0..16 {
            env.clock();
        }
        assert_eq!(env.level(), 0);
    }

    #[test]
    fn envelope_loops_when_flag_set() {
        let mut env = Envelope::new();
        env.set_period(0); // Fastest decay
        env.set_loop(true);
        env.restart();
        
        // First clock processes start flag
        env.clock(); 
        assert_eq!(env.level(), 15);
        
        // Decay down to 0 (15 more clocks: 14, 13, ..., 1, 0)
        for expected in (0..15).rev() {
            env.clock();
            assert_eq!(env.level(), expected);
        }
        
        // Next clock should loop back to 15
        env.clock();
        assert_eq!(env.level(), 15);
    }

    #[test]
    fn envelope_period_controls_decay_rate() {
        let mut env = Envelope::new();
        env.set_period(2); // Slower decay
        env.restart();
        env.clock(); // Process start flag, level = 15, divider = 2

        env.clock(); // divider = 1
        assert_eq!(env.level(), 15); // Should not decay yet

        env.clock(); // divider = 0
        assert_eq!(env.level(), 15); // Still not decayed

        env.clock(); // divider reloads, decay happens
        assert_eq!(env.level(), 14); // Now decayed
    }
}
