//! NES APU (Audio Processing Unit) implementation.
//!
//! This module provides the NES-specific APU interface while using
//! reusable components from the core module.
//!
//! ## Current Implementation
//!
//! The APU currently implements:
//!
//! - **2 Pulse Channels**: Square wave generators with duty cycle control
//! - **Triangle Channel**: 32-step triangle wave generator
//! - **Noise Channel**: Pseudo-random noise with LFSR
//! - **Length Counter**: Automatic note duration control
//! - **Envelope**: Volume envelope with decay
//! - **Frame Counter**: Timing controller (4-step and 5-step modes)
//! - **Frame IRQ**: Frame counter interrupt support
//!
//! ## Not Yet Implemented
//!
//! - **DMC Channel**: Delta modulation channel for sample playback
//! - **Sweep Units**: Pitch bending for pulse channels
//!
//! ## Register Interface
//!
//! - **$4000-$4003**: Pulse channel 1 (duty, envelope, frequency, length)
//! - **$4004-$4007**: Pulse channel 2 (duty, envelope, frequency, length)
//! - **$4008-$400B**: Triangle channel (control, linear counter, frequency, length)
//! - **$400C-$400F**: Noise channel (envelope, mode/period, length)
//! - **$4010-$4013**: DMC channel (not implemented)
//! - **$4015**: Status/enable register
//! - **$4017**: Frame counter mode and IRQ control
//!
//! ## Audio Output
//!
//! The APU generates 44.1 kHz stereo audio by:
//!
//! 1. Clocking the APU at CPU speed (1.789773 MHz NTSC or 1.662607 MHz PAL)
//! 2. Mixing the active channels using linear approximation
//! 3. Downsampling to the target sample rate
//!
//! The current implementation uses a simple average mixing strategy.
//! Future enhancements could include proper NES mixer simulation with
//! non-linear output curves.

use emu_core::apu::{NoiseChannel, PulseChannel, TimingMode, TriangleChannel, LENGTH_TABLE};
use std::cell::Cell;

/// NES APU with pulse, triangle, and noise channels.
///
/// Uses core APU components for audio synthesis.
///
/// # Registers
///
/// The APU responds to writes at $4000-$4017:
///
/// - $4000-$4003: Pulse 1 (DDLC VVVV, sweep, timer low, length/timer high)
/// - $4004-$4007: Pulse 2 (same as pulse 1)
/// - $4008-$400B: Triangle (control, unused, timer low, timer high/length)
/// - $400C-$400F: Noise (envelope, unused, mode/period, length)
/// - $4015: Enable register (bits 0-3 enable pulse 1-2, triangle, noise)
/// - $4017: Frame counter mode (bit 7 = 5-step, bit 6 = IRQ inhibit)
///
/// # Timing
///
/// The APU runs at CPU clock speed:
///
/// - NTSC: 1.789773 MHz
/// - PAL: 1.662607 MHz
///
/// Frame counter events occur at:
///
/// - NTSC: ~240 Hz (quarter frame)
/// - PAL: ~200 Hz (quarter frame)
///
/// # IRQ Generation
///
/// In 4-step mode, the frame counter generates an IRQ at the end of step 4
/// unless the IRQ inhibit flag is set. Reading $4015 clears the pending IRQ.
#[derive(Debug)]
pub struct APU {
    pub pulse1: PulseChannel,
    pub pulse2: PulseChannel,
    pub triangle: TriangleChannel,
    pub noise: NoiseChannel,
    cycle_accum: f64,
    timing: TimingMode,
    /// Frame counter for clocking length counters and envelopes
    /// Counts CPU cycles and triggers quarter/half frame events
    frame_counter_cycles: u32,
    frame_counter_mode: bool, // false = 4-step, true = 5-step

    // IRQ specific state (duplicated to avoid rewriting audio generation for now)
    irq_frame_counter_cycles: u32,
    irq_inhibit: bool,
    irq_pending: Cell<bool>,
}

impl APU {
    pub fn new() -> Self {
        Self::new_with_timing(TimingMode::Ntsc)
    }

    pub fn new_with_timing(timing: TimingMode) -> Self {
        Self {
            pulse1: PulseChannel::new(),
            pulse2: PulseChannel::new(),
            triangle: TriangleChannel::new(),
            noise: NoiseChannel::new(),
            cycle_accum: 0.0,
            timing,
            frame_counter_cycles: 0,
            frame_counter_mode: false,
            irq_frame_counter_cycles: 0,
            irq_inhibit: true, // Default is inhibited
            irq_pending: Cell::new(false),
        }
    }

    /// Set timing mode (NTSC/PAL)
    pub fn set_timing(&mut self, timing: TimingMode) {
        self.timing = timing;
    }

    /// Process APU register writes
    pub fn write_register(&mut self, addr: u16, val: u8) {
        match addr {
            // Pulse 1 registers
            0x4000 => {
                // DDLC VVVV: duty, loop/length counter halt, constant volume, volume/envelope
                self.pulse1.duty = (val >> 6) & 3;
                self.pulse1.length_counter_halt = (val & 0x20) != 0;
                self.pulse1.constant_volume = (val & 0x10) != 0;
                self.pulse1.envelope = val & 15;
            }
            0x4001 => {
                // Sweep (ignore for now)
            }
            0x4002 => {
                // Frequency low byte
                let low = val as u16;
                let high = (self.pulse1.timer_reload >> 8) & 0x07;
                self.pulse1.set_timer((high << 8) | low);
            }
            0x4003 => {
                // Frequency high byte + length counter index
                let high = (val & 0x07) as u16;
                let low = self.pulse1.timer_reload & 0xFF;
                self.pulse1.set_timer((high << 8) | low);
                // Trigger: reset phase
                self.pulse1.reset_phase();
                // Note: enabled flag is only controlled by $4015, not by writes to $4003
                // Length counter index in upper 5 bits
                let len_index = (val >> 3) & 0x1F;
                self.pulse1.length_counter = LENGTH_TABLE[len_index as usize];
            }

            // Pulse 2 registers
            0x4004 => {
                self.pulse2.duty = (val >> 6) & 3;
                self.pulse2.length_counter_halt = (val & 0x20) != 0;
                self.pulse2.constant_volume = (val & 0x10) != 0;
                self.pulse2.envelope = val & 15;
            }
            0x4005 => {
                // Sweep (ignore for now)
            }
            0x4006 => {
                let low = val as u16;
                let high = (self.pulse2.timer_reload >> 8) & 0x07;
                self.pulse2.set_timer((high << 8) | low);
            }
            0x4007 => {
                let freq_high = (val & 0x07) as u16;
                let freq_low = (self.pulse2.timer_reload & 0xFF) as u16;
                self.pulse2.set_timer((freq_high << 8) | freq_low);
                self.pulse2.reset_phase();
                // Note: enabled flag is only controlled by $4015, not by writes to $4007
                let len_index = (val >> 3) & 0x1F;
                self.pulse2.length_counter = LENGTH_TABLE[len_index as usize];
            }

            // Triangle registers
            0x4008 => {
                // Control flag (bit 7) and linear counter reload value (bits 6-0)
                self.triangle.control_flag = (val & 0x80) != 0;
                self.triangle.linear_counter_reload = val & 0x7F;
            }
            0x4009 => {
                // Unused
            }
            0x400A => {
                // Frequency low byte
                let low = val as u16;
                let high = (self.triangle.timer_reload >> 8) & 0x07;
                self.triangle.set_timer((high << 8) | low);
            }
            0x400B => {
                // Frequency high byte + length counter index
                let high = (val & 0x07) as u16;
                let low = self.triangle.timer_reload & 0xFF;
                self.triangle.set_timer((high << 8) | low);
                // Set reload flag to reload linear counter
                self.triangle.linear_counter_reload_flag = true;
                // Length counter index in upper 5 bits
                let len_index = (val >> 3) & 0x1F;
                self.triangle.length_counter = LENGTH_TABLE[len_index as usize];
            }

            // Noise registers
            0x400C => {
                // Envelope settings (same format as pulse channels)
                self.noise.length_counter_halt = (val & 0x20) != 0;
                self.noise.constant_volume = (val & 0x10) != 0;
                self.noise.envelope = val & 0x0F;
            }
            0x400D => {
                // Unused
            }
            0x400E => {
                // Mode flag (bit 7) and period index (bits 3-0)
                self.noise.mode = (val & 0x80) != 0;
                self.noise.set_period(val & 0x0F);
            }
            0x400F => {
                // Length counter index
                let len_index = (val >> 3) & 0x1F;
                self.noise.length_counter = LENGTH_TABLE[len_index as usize];
            }

            // APU Enable register
            0x4015 => {
                self.pulse1.enabled = (val & 0x01) != 0;
                self.pulse2.enabled = (val & 0x02) != 0;
                self.triangle.enabled = (val & 0x04) != 0;
                self.noise.enabled = (val & 0x08) != 0;
                // DMC enable at bit 4 (not yet implemented)
            }

            // Frame Counter register
            0x4017 => {
                // Bit 7: Mode (0 = 4-step, 1 = 5-step)
                // Bit 6: IRQ inhibit flag
                self.frame_counter_mode = (val & 0x80) != 0;
                self.irq_inhibit = (val & 0x40) != 0;

                if self.irq_inhibit {
                    self.irq_pending.set(false);
                }

                // Reset frame counter on write
                self.frame_counter_cycles = 0;
                self.irq_frame_counter_cycles = 0;

                // If 5-step mode, clock immediately (not implemented here for audio, but noted)
            }

            _ => {}
        }
    }

    /// Read APU status register ($4015)
    pub fn read_register(&self, addr: u16) -> u8 {
        match addr {
            0x4015 => {
                // Bit 0: Pulse 1 length counter > 0
                // Bit 1: Pulse 2 length counter > 0
                // Bit 2: Triangle length counter > 0
                // Bit 3: Noise length counter > 0
                // Bit 4: DMC active (not implemented, return 0)
                // Bit 5: unused (return 0)
                // Bit 6: Frame interrupt
                // Bit 7: DMC interrupt (not implemented, return 0)
                let mut status = 0u8;
                if self.pulse1.length_counter > 0 {
                    status |= 0x01;
                }
                if self.pulse2.length_counter > 0 {
                    status |= 0x02;
                }
                if self.triangle.length_counter > 0 {
                    status |= 0x04;
                }
                if self.noise.length_counter > 0 {
                    status |= 0x08;
                }
                if self.irq_pending.get() {
                    status |= 0x40;
                    self.irq_pending.set(false); // Reading $4015 clears frame interrupt
                }
                status
            }
            _ => 0,
        }
    }

    pub fn irq_pending(&self) -> bool {
        self.irq_pending.get()
    }

    pub fn clock_irq(&mut self, cycles: u32) {
        if self.frame_counter_mode {
            return; // 5-step mode: no IRQ
        }

        let cpu_hz = self.timing.cpu_clock_hz();
        let frame_counter_hz = self.timing.frame_counter_hz();
        let quarter_frame_cycles = (cpu_hz / frame_counter_hz) as u32;
        // 4-step mode: IRQ at end of step 4 (approx 29828 cycles)
        let irq_time = quarter_frame_cycles * 4;

        self.irq_frame_counter_cycles += cycles;

        if !self.irq_inhibit && self.irq_frame_counter_cycles >= irq_time {
            self.irq_pending.set(true);
        }

        // Wrap around (simplified)
        // In reality, it wraps slightly differently, but this ensures periodic IRQs
        if self.irq_frame_counter_cycles >= irq_time + 20 {
            // small buffer
            self.irq_frame_counter_cycles %= irq_time;
        }
    }

    /// Generate audio samples for a given count, stepping APU in CPU-cycle time
    /// using the configured timing mode and sample rate of 44.1 kHz.
    pub fn generate_samples(&mut self, sample_count: usize) -> Vec<i16> {
        const SAMPLE_HZ: f64 = 44_100.0;
        let cpu_hz = self.timing.cpu_clock_hz();
        let cycles_per_sample = cpu_hz / SAMPLE_HZ;

        // Frame counter clocking intervals
        // Quarter frame: clocks envelope at ~240 Hz (NTSC) or ~200 Hz (PAL)
        // Half frame: clocks length counter at quarters 2 and 4
        let frame_counter_hz = self.timing.frame_counter_hz();
        let quarter_frame_cycles = (cpu_hz / frame_counter_hz) as u32;

        // Full frame cycle count for resetting counter (prevents overflow)
        // 4-step mode: 4 quarter frames (~29829 cycles NTSC)
        // 5-step mode: 5 quarter frames (~37281 cycles NTSC)
        let full_frame_cycles = if self.frame_counter_mode {
            quarter_frame_cycles * 5 // 5-step mode
        } else {
            quarter_frame_cycles * 4 // 4-step mode
        };

        let mut out = Vec::with_capacity(sample_count);
        for _ in 0..sample_count {
            self.cycle_accum += cycles_per_sample;
            let mut cycles = self.cycle_accum as u32;
            if cycles == 0 {
                cycles = 1; // ensure we advance state even if timing slips
            }
            self.cycle_accum -= cycles as f64;

            let mut acc = 0i32;
            for _ in 0..cycles {
                // Clock frame counter
                let prev_quarter = self.frame_counter_cycles / quarter_frame_cycles;

                self.frame_counter_cycles = self.frame_counter_cycles.wrapping_add(1);

                // Reset frame counter at end of full frame to prevent overflow issues
                if self.frame_counter_cycles >= full_frame_cycles {
                    self.frame_counter_cycles = 0;
                }

                // Check for quarter frame boundaries
                let curr_quarter = self.frame_counter_cycles / quarter_frame_cycles;
                if curr_quarter != prev_quarter {
                    // Length counters clock at quarters 2 and 4 (half frames)
                    // In 4-step mode: quarters 0, 1, 2, 3 -> clock at 1 and 3 (0-indexed)
                    // In 5-step mode: quarters 0, 1, 2, 3, 4 -> clock at 1 and 3 only (NOT at 4)
                    let quarter_index = curr_quarter % 5; // Will be 0-4 in 5-step, 0-3 in 4-step
                    if quarter_index == 1 || quarter_index == 3 {
                        // Clock length counters at half-frame rate (~120 Hz NTSC, ~100 Hz PAL)
                        if self.pulse1.length_counter > 0 && !self.pulse1.length_counter_halt {
                            self.pulse1.length_counter -= 1;
                        }
                        if self.pulse2.length_counter > 0 && !self.pulse2.length_counter_halt {
                            self.pulse2.length_counter -= 1;
                        }
                        if self.triangle.length_counter > 0 && !self.triangle.control_flag {
                            self.triangle.length_counter -= 1;
                        }
                        if self.noise.length_counter > 0 && !self.noise.length_counter_halt {
                            self.noise.length_counter -= 1;
                        }
                    }

                    // Clock linear counter (triangle) at every quarter frame
                    self.triangle.clock_linear_counter();
                }

                // Clock all channels
                let s1 = self.pulse1.clock() as i32;
                let s2 = self.pulse2.clock() as i32;
                let s3 = self.triangle.clock() as i32;
                let s4 = self.noise.clock() as i32;
                acc += s1 + s2 + s3 + s4;
            }

            let avg = acc / cycles as i32;
            const CHANNEL_COUNT: i32 = 4;
            let mixed = avg / CHANNEL_COUNT; // Average for 4 channels
            out.push(mixed.clamp(-32768, 32767) as i16);
        }

        out
    }
}

impl Default for APU {
    fn default() -> Self {
        Self::new()
    }
}
