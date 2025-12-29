//! Triangle wave generator for RP2A03/RP2A07 APU.
//!
//! The triangle channel produces a quantized triangle wave with 32 steps.

/// Triangle channel that generates triangle wave samples.
///
/// The triangle channel has:
/// - 32-step triangle wave (no volume control, fixed output)
/// - Length counter for note duration
/// - Linear counter for additional duration control
/// - No envelope generator (unlike pulse/noise)
#[derive(Debug, Clone)]
pub struct TriangleChannel {
    /// 11-bit timer reload value
    pub timer_reload: u16,
    /// Timer counter
    timer: u16,
    /// Current step in the triangle sequence (0-31)
    sequence_pos: u8,
    /// Length counter
    pub length_counter: u8,
    /// Linear counter (7-bit)
    pub linear_counter: u8,
    /// Linear counter reload value
    pub linear_counter_reload: u8,
    /// Linear counter reload flag
    pub linear_counter_reload_flag: bool,
    /// Control flag (halt length counter and linear counter)
    pub control_flag: bool,
    /// Whether the channel is enabled
    pub enabled: bool,
}

impl TriangleChannel {
    /// Create a new triangle channel with default state
    pub fn new() -> Self {
        Self {
            timer_reload: 0,
            timer: 0,
            sequence_pos: 0,
            length_counter: 0,
            linear_counter: 0,
            linear_counter_reload: 0,
            linear_counter_reload_flag: false,
            control_flag: false,
            enabled: false,
        }
    }

    /// Clock the triangle channel for one CPU cycle
    pub fn clock(&mut self) -> i16 {
        // Triangle sequence: 32 steps forming a triangle wave
        // Output only if both length counter and linear counter are non-zero
        let sample = if self.enabled && self.length_counter > 0 && self.linear_counter > 0 {
            self.triangle_output()
        } else {
            // When silenced, output 0 (like pulse and noise channels)
            0
        };

        // Only advance the sequencer if linear counter and length counter are both non-zero
        if self.linear_counter > 0 && self.length_counter > 0 {
            if self.timer > 0 {
                self.timer -= 1;
            } else {
                self.timer = self.timer_reload;
                self.sequence_pos = (self.sequence_pos + 1) & 31;
            }
        }

        sample
    }

    /// Get the current triangle wave output value
    fn triangle_output(&self) -> i16 {
        // NES triangle wave: 32 steps, 4-bit output
        // Sequence: 15, 14, 13, ..., 0, 0, 1, 2, ..., 15
        const TRIANGLE_TABLE: [u8; 32] = [
            15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10,
            11, 12, 13, 14, 15,
        ];
        let value = TRIANGLE_TABLE[self.sequence_pos as usize];
        // Convert 4-bit value to signed 16-bit centered around 0
        ((value as i16) - 7) << 10
    }

    /// Set timer reload value
    pub fn set_timer(&mut self, t: u16) {
        self.timer_reload = t & 0x07FF;
    }

    /// Clock the linear counter (called by frame counter at ~240Hz NTSC)
    pub fn clock_linear_counter(&mut self) {
        if self.linear_counter_reload_flag {
            self.linear_counter = self.linear_counter_reload;
        } else if self.linear_counter > 0 {
            self.linear_counter -= 1;
        }

        if !self.control_flag {
            self.linear_counter_reload_flag = false;
        }
    }
}

impl Default for TriangleChannel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn triangle_sequence_advances() {
        let mut tri = TriangleChannel::new();
        tri.enabled = true;
        tri.length_counter = 10;
        tri.linear_counter = 10;
        tri.set_timer(0); // Fastest timer

        let initial_pos = tri.sequence_pos;
        tri.clock();
        // Position should advance when timer expires
        assert_eq!(tri.sequence_pos, (initial_pos + 1) & 31);
    }

    #[test]
    fn triangle_silenced_when_counters_zero() {
        let mut tri = TriangleChannel::new();
        tri.enabled = true;
        tri.length_counter = 0;
        tri.linear_counter = 0;
        tri.set_timer(0);

        let initial_pos = tri.sequence_pos;
        tri.clock();
        // Sequence should not advance when counters are zero
        assert_eq!(tri.sequence_pos, initial_pos);
    }

    #[test]
    fn triangle_linear_counter_reload() {
        let mut tri = TriangleChannel::new();
        tri.linear_counter_reload = 10;
        tri.linear_counter_reload_flag = true;
        tri.control_flag = false;

        tri.clock_linear_counter();
        assert_eq!(tri.linear_counter, 10);
        // Flag should be cleared if control_flag is false
        assert!(!tri.linear_counter_reload_flag);
    }

    #[test]
    fn triangle_linear_counter_halt() {
        let mut tri = TriangleChannel::new();
        tri.linear_counter = 5;
        tri.linear_counter_reload_flag = false;
        tri.control_flag = false;

        tri.clock_linear_counter();
        assert_eq!(tri.linear_counter, 4); // Should decrement

        tri.clock_linear_counter();
        assert_eq!(tri.linear_counter, 3);
    }

    #[test]
    fn triangle_outputs_zero_when_disabled() {
        let mut tri = TriangleChannel::new();
        tri.enabled = false;
        tri.length_counter = 10;
        tri.linear_counter = 10;
        tri.set_timer(0);

        let sample = tri.clock();
        assert_eq!(sample, 0, "Triangle channel should output 0 when disabled");
    }

    #[test]
    fn triangle_outputs_zero_when_length_counter_zero() {
        let mut tri = TriangleChannel::new();
        tri.enabled = true;
        tri.length_counter = 0;
        tri.linear_counter = 10;
        tri.set_timer(0);

        let sample = tri.clock();
        assert_eq!(
            sample, 0,
            "Triangle channel should output 0 when length counter is 0"
        );
    }

    #[test]
    fn triangle_outputs_zero_when_linear_counter_zero() {
        let mut tri = TriangleChannel::new();
        tri.enabled = true;
        tri.length_counter = 10;
        tri.linear_counter = 0;
        tri.set_timer(0);

        let sample = tri.clock();
        assert_eq!(
            sample, 0,
            "Triangle channel should output 0 when linear counter is 0"
        );
    }
}
