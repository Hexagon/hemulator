//! Frame counter that drives envelope, length counter, and sweep units.
//!
//! The frame counter operates at approximately 240Hz (NTSC) or 200Hz (PAL)
//! and provides timing signals for various APU components.

use super::timing::TimingMode;

/// Frame counter component.
///
/// Provides timing signals for:
/// - Envelope generators (quarter frames)
/// - Length counters (half frames)
/// - Sweep units (half frames)
///
/// Supports two modes:
/// - 4-step mode: f f f f (4 quarter frames, IRQ on 4th)
/// - 5-step mode: f f f f - (5 quarter frames, no IRQ)
#[derive(Debug, Clone)]
pub struct FrameCounter {
    /// Current mode: false = 4-step, true = 5-step
    mode_5_step: bool,
    /// IRQ inhibit flag
    irq_inhibit: bool,
    /// IRQ pending flag
    irq_pending: bool,
    /// Current step in the sequence (0-3 for 4-step, 0-4 for 5-step)
    step: u8,
    /// Cycle counter for timing
    cycle_count: u32,
    /// Cycles per quarter frame
    cycles_per_quarter_frame: u32,
}

impl FrameCounter {
    /// Create a new frame counter for the specified timing mode
    pub fn new(timing: TimingMode) -> Self {
        let cpu_hz = timing.cpu_clock_hz();
        let frame_counter_hz = timing.frame_counter_hz();
        let cycles_per_quarter_frame = (cpu_hz / frame_counter_hz) as u32;

        Self {
            mode_5_step: false,
            irq_inhibit: true,
            irq_pending: false,
            step: 0,
            cycle_count: 0,
            cycles_per_quarter_frame,
        }
    }

    /// Clock the frame counter for one CPU cycle.
    /// Returns (quarter_frame, half_frame) signals.
    pub fn clock(&mut self) -> (bool, bool) {
        self.cycle_count += 1;

        if self.cycle_count >= self.cycles_per_quarter_frame {
            self.cycle_count = 0;
            self.step += 1;

            let max_step = if self.mode_5_step { 5 } else { 4 };

            if self.step >= max_step {
                self.step = 0;
                // Generate IRQ on step 4 in 4-step mode if not inhibited
                if !self.mode_5_step && !self.irq_inhibit {
                    self.irq_pending = true;
                }
            }

            // Quarter frame signal on every step
            let quarter_frame = true;

            // Half frame signal on steps 1 and 3 (4-step) or 1 and 4 (5-step)
            let half_frame = if self.mode_5_step {
                self.step == 1 || self.step == 4
            } else {
                self.step == 1 || self.step == 3
            };

            (quarter_frame, half_frame)
        } else {
            (false, false)
        }
    }

    /// Write to the frame counter control register ($4017)
    pub fn write_control(&mut self, value: u8) {
        self.mode_5_step = (value & 0x80) != 0;
        self.irq_inhibit = (value & 0x40) != 0;

        if self.irq_inhibit {
            self.irq_pending = false;
        }

        // Reset the sequence
        self.step = 0;
        self.cycle_count = 0;

        // In 5-step mode, immediately clock all units
        // (This would need to be handled by the caller)
    }

    /// Check if IRQ is pending
    pub fn is_irq_pending(&self) -> bool {
        self.irq_pending
    }

    /// Clear the IRQ flag
    pub fn clear_irq(&mut self) {
        self.irq_pending = false;
    }

    /// Update timing mode (for switching between NTSC/PAL)
    pub fn set_timing(&mut self, timing: TimingMode) {
        let cpu_hz = timing.cpu_clock_hz();
        let frame_counter_hz = timing.frame_counter_hz();
        self.cycles_per_quarter_frame = (cpu_hz / frame_counter_hz) as u32;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_counter_4_step_mode() {
        let mut fc = FrameCounter::new(TimingMode::Ntsc);
        fc.write_control(0x00); // 4-step mode, IRQ enabled

        assert!(!fc.mode_5_step);
    }

    #[test]
    fn frame_counter_5_step_mode() {
        let mut fc = FrameCounter::new(TimingMode::Ntsc);
        fc.write_control(0x80); // 5-step mode

        assert!(fc.mode_5_step);
    }

    #[test]
    fn frame_counter_irq_inhibit() {
        let mut fc = FrameCounter::new(TimingMode::Ntsc);
        fc.write_control(0x40); // IRQ inhibit

        assert!(fc.irq_inhibit);
        assert!(!fc.is_irq_pending());
    }

    #[test]
    fn frame_counter_generates_quarter_frames() {
        let mut fc = FrameCounter::new(TimingMode::Ntsc);
        fc.write_control(0x00); // 4-step mode

        let mut quarter_count = 0;
        // Clock for a full cycle
        for _ in 0..(fc.cycles_per_quarter_frame * 5) {
            let (quarter, _half) = fc.clock();
            if quarter {
                quarter_count += 1;
            }
        }

        // Should have at least a few quarter frames
        assert!(quarter_count >= 4);
    }

    #[test]
    fn frame_counter_pal_timing() {
        let fc_ntsc = FrameCounter::new(TimingMode::Ntsc);
        let fc_pal = FrameCounter::new(TimingMode::Pal);

        // PAL should have different (slower) timing
        assert_ne!(
            fc_ntsc.cycles_per_quarter_frame,
            fc_pal.cycles_per_quarter_frame
        );
    }
}
