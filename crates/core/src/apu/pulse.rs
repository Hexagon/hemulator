//! Pulse wave generator for RP2A03/RP2A07 APU.
//!
//! This module implements the pulse channel used in the NES APU and potentially
//! reusable in other systems with similar square wave synthesis chips.

/// Pulse channel that generates square wave samples.
///
/// The pulse channel produces a variable-width pulse signal with support for:
/// - 4 duty cycle modes (12.5%, 25%, 50%, 75%)
/// - 11-bit timer for frequency control
/// - Length counter for note duration
/// - Envelope generator for volume control
#[derive(Debug, Clone)]
pub struct PulseChannel {
    /// Duty cycle (0-3): 12.5%, 25%, 50%, 75%
    pub duty: u8,
    /// 11-bit timer reload value from registers
    pub timer_reload: u16,
    /// Timer counter (counts down to 0, then resets)
    timer: u16,
    /// Current phase of the duty cycle (0-7)
    phase: u8,
    /// Length counter (decrements each frame, mutes when 0)
    pub length_counter: u8,
    /// Envelope volume (4-bit, 0-15)
    pub envelope: u8,
    /// Whether the channel is enabled
    pub enabled: bool,
    /// Whether to use constant volume (true) or envelope (false)
    pub constant_volume: bool,
    /// Length counter halt / envelope loop flag
    pub length_counter_halt: bool,
}

impl PulseChannel {
    /// Create a new pulse channel with default state
    pub fn new() -> Self {
        Self {
            duty: 0,
            timer_reload: 0,
            timer: 0,
            phase: 0,
            length_counter: 0,
            envelope: 15,
            enabled: false,
            constant_volume: false,
            length_counter_halt: false,
        }
    }

    /// Clock the pulse channel for one CPU cycle, returning a single sample (-32768..32767)
    pub fn clock(&mut self) -> i16 {
        // Generate current sample based on duty and phase
        let sample = if self.enabled && self.length_counter > 0 {
            let output = self.duty_output();
            if output {
                (self.envelope as i16) << 10
            } else {
                -((self.envelope as i16) << 10)
            }
        } else {
            0
        };

        // Decrement timer
        if self.timer > 0 {
            self.timer -= 1;
        } else {
            // Reset timer and advance phase
            self.timer = self.timer_reload.wrapping_add(1).saturating_mul(2);
            self.phase = (self.phase + 1) & 7;
        }

        sample
    }

    /// Determine if the current phase should output 1 based on duty cycle
    pub fn duty_output(&self) -> bool {
        // NES/RP2A03 duty patterns indexed by (duty, phase)
        // 0: 0 1 0 0 0 0 0 0 (12.5%)
        // 1: 0 1 1 0 0 0 0 0 (25%)
        // 2: 0 1 1 1 1 0 0 0 (50%)
        // 3: 1 0 0 1 1 1 1 1 (75%)
        const TABLE: [[bool; 8]; 4] = [
            [false, true, false, false, false, false, false, false],
            [false, true, true, false, false, false, false, false],
            [false, true, true, true, true, false, false, false],
            [true, false, false, true, true, true, true, true],
        ];
        TABLE[(self.duty & 3) as usize][(self.phase & 7) as usize]
    }

    /// Set timer reload value
    pub fn set_timer(&mut self, t: u16) {
        self.timer_reload = t & 0x07FF;
        self.timer = self.timer_reload.wrapping_add(1).saturating_mul(2);
    }

    /// Reset the phase (used when triggering a new note)
    pub fn reset_phase(&mut self) {
        self.phase = 0;
    }
}

impl Default for PulseChannel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pulse_duty_patterns() {
        let mut pulse = PulseChannel::new();

        // Test 12.5% duty cycle
        pulse.duty = 0;
        let pattern: Vec<bool> = (0..8)
            .map(|i| {
                pulse.phase = i;
                pulse.duty_output()
            })
            .collect();
        assert_eq!(
            pattern,
            vec![false, true, false, false, false, false, false, false]
        );

        // Test 25% duty cycle
        pulse.duty = 1;
        let pattern: Vec<bool> = (0..8)
            .map(|i| {
                pulse.phase = i;
                pulse.duty_output()
            })
            .collect();
        assert_eq!(
            pattern,
            vec![false, true, true, false, false, false, false, false]
        );

        // Test 50% duty cycle
        pulse.duty = 2;
        let pattern: Vec<bool> = (0..8)
            .map(|i| {
                pulse.phase = i;
                pulse.duty_output()
            })
            .collect();
        assert_eq!(
            pattern,
            vec![false, true, true, true, true, false, false, false]
        );

        // Test 75% duty cycle
        pulse.duty = 3;
        let pattern: Vec<bool> = (0..8)
            .map(|i| {
                pulse.phase = i;
                pulse.duty_output()
            })
            .collect();
        assert_eq!(
            pattern,
            vec![true, false, false, true, true, true, true, true]
        );
    }

    #[test]
    fn pulse_timer_countdown() {
        let mut pulse = PulseChannel::new();
        pulse.enabled = true;
        pulse.length_counter = 10;
        pulse.set_timer(1); // Timer reload = 1, actual period = (1+1)*2 = 4

        // Initial timer value is 4, counts down: 4, 3, 2, 1, 0
        // On the 5th clock (when timer=0), phase advances and timer reloads to 4
        let initial_phase = pulse.phase;

        // Clock 4 times - timer goes from 4->3->2->1
        for _ in 0..4 {
            pulse.clock();
        }
        // Phase should still be the same
        assert_eq!(pulse.phase, initial_phase);

        // Clock once more - timer goes 0, phase advances, timer reloads
        pulse.clock();
        assert_eq!(pulse.phase, (initial_phase + 1) & 7);
    }

    #[test]
    fn pulse_length_counter_mutes() {
        let mut pulse = PulseChannel::new();
        pulse.enabled = true;
        pulse.envelope = 15;
        pulse.length_counter = 0; // Length counter at 0 should mute

        let sample = pulse.clock();
        assert_eq!(sample, 0); // Should be muted
    }

    #[test]
    fn pulse_disabled_mutes() {
        let mut pulse = PulseChannel::new();
        pulse.enabled = false;
        pulse.envelope = 15;
        pulse.length_counter = 10;

        let sample = pulse.clock();
        assert_eq!(sample, 0); // Should be muted when disabled
    }
}
