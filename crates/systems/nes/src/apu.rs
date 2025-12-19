//! NES APU implementation using core APU components.
//!
//! This module provides the NES-specific APU interface while using
//! reusable components from the core module.

use emu_core::apu::{PulseChannel, TimingMode, LENGTH_TABLE};

/// Minimal NES APU with 2 pulse channels.
///
/// Uses core PulseChannel components for the actual synthesis.
#[derive(Debug)]
pub struct APU {
    pub pulse1: PulseChannel,
    pub pulse2: PulseChannel,
    cycle_accum: f64,
    timing: TimingMode,
    /// Frame counter for clocking length counters and envelopes
    /// Counts CPU cycles and triggers quarter/half frame events
    frame_counter_cycles: u32,
    frame_counter_mode: bool, // false = 4-step, true = 5-step
}

impl APU {
    pub fn new() -> Self {
        Self::new_with_timing(TimingMode::Ntsc)
    }

    pub fn new_with_timing(timing: TimingMode) -> Self {
        Self {
            pulse1: PulseChannel::new(),
            pulse2: PulseChannel::new(),
            cycle_accum: 0.0,
            timing,
            frame_counter_cycles: 0,
            frame_counter_mode: false,
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

            // APU Enable register
            0x4015 => {
                self.pulse1.enabled = (val & 1) != 0;
                self.pulse2.enabled = (val & 2) != 0;
            }

            // Frame Counter register
            0x4017 => {
                // Bit 7: Mode (0 = 4-step, 1 = 5-step)
                // Bit 6: IRQ inhibit flag
                self.frame_counter_mode = (val & 0x80) != 0;
                // Reset frame counter on write
                self.frame_counter_cycles = 0;
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
                // Bits 2-3: Triangle and Noise (not implemented, return 0)
                // Bit 4: DMC active (not implemented, return 0)
                // Bits 5: unused (return 0)
                // Bit 6: Frame interrupt (not implemented, return 0)
                // Bit 7: DMC interrupt (not implemented, return 0)
                let mut status = 0u8;
                if self.pulse1.length_counter > 0 {
                    status |= 0x01;
                }
                if self.pulse2.length_counter > 0 {
                    status |= 0x02;
                }
                status
            }
            _ => 0,
        }
    }

    /// Generate audio samples for a given count, stepping APU in CPU-cycle time
    /// using the configured timing mode and sample rate of 44.1 kHz.
    pub fn generate_samples(&mut self, sample_count: usize) -> Vec<i16> {
        const SAMPLE_HZ: f64 = 44_100.0;
        let cpu_hz = self.timing.cpu_clock_hz();
        let cycles_per_sample = cpu_hz / SAMPLE_HZ;

        // Frame counter clocking intervals (4-step mode)
        // Quarter frame: clocks envelope at ~240 Hz (NTSC) or ~200 Hz (PAL)
        // Half frame: clocks length counter at ~120 Hz (NTSC) or ~100 Hz (PAL)
        let frame_counter_hz = self.timing.frame_counter_hz();
        let quarter_frame_cycles = (cpu_hz / frame_counter_hz) as u32;
        let half_frame_cycles = quarter_frame_cycles * 2;

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
                // Clock frame counter (wraps at a full frame cycle count)
                let prev_half = self.frame_counter_cycles / half_frame_cycles;
                
                self.frame_counter_cycles = self.frame_counter_cycles.wrapping_add(1);
                
                // Check for half frame (length counter clocking)
                let curr_half = self.frame_counter_cycles / half_frame_cycles;
                if curr_half != prev_half {
                    // Clock length counters at half-frame rate (~120 Hz NTSC, ~100 Hz PAL)
                    if self.pulse1.length_counter > 0 && !self.pulse1.length_counter_halt {
                        self.pulse1.length_counter -= 1;
                    }
                    if self.pulse2.length_counter > 0 && !self.pulse2.length_counter_halt {
                        self.pulse2.length_counter -= 1;
                    }
                }
                
                // Clock pulse channels
                let s1 = self.pulse1.clock() as i32;
                let s2 = self.pulse2.clock() as i32;
                acc += s1 + s2;
            }

            let avg = acc / cycles as i32;
            let mixed = avg / 2; // simple pulse mix average
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
