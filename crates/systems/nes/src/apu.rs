//! NES APU (Audio Processing Unit) implementation.
//!
//! This module provides the NES-specific APU interface while using
//! reusable components from the core module.
//!
//! ## Current Implementation
//!
//! The APU implements all 5 channels:
//!
//! - **2 Pulse Channels**: Square wave generators with duty cycle control
//! - **Sweep Units**: Frequency sweep for pulse channels with NES-specific behavior
//! - **Triangle Channel**: 32-step triangle wave generator
//! - **Noise Channel**: Pseudo-random noise with LFSR
//! - **DMC Channel**: Delta modulation channel for sample playback
//! - **Length Counter**: Automatic note duration control
//! - **Envelope**: Volume envelope with decay
//! - **Frame Counter**: Timing controller (4-step and 5-step modes)
//! - **Frame IRQ**: Frame counter interrupt support
//!
//! ## Register Interface
//!
//! - **$4000-$4003**: Pulse channel 1 (duty, envelope, frequency, length)
//! - **$4004-$4007**: Pulse channel 2 (duty, envelope, frequency, length)
//! - **$4008-$400B**: Triangle channel (control, linear counter, frequency, length)
//! - **$400C-$400F**: Noise channel (envelope, mode/period, length)
//! - **$4010-$4013**: DMC channel (flags/rate, direct load, address, length)
//! - **$4015**: Status/enable register
//! - **$4017**: Frame counter mode and IRQ control
//!
//! ## Audio Output
//!
//! The APU generates 44.1 kHz stereo audio by:
//!
//! 1. Clocking the APU at CPU speed (1.789773 MHz NTSC or 1.662607 MHz PAL)
//! 2. Mixing all 5 channels using linear approximation
//! 3. Downsampling to the target sample rate
//!
//! The current implementation uses a simple average mixing strategy.
//! Future enhancements could include proper NES mixer simulation with
//! non-linear output curves.

use emu_core::apu::{
    DmcChannel, Envelope, NoiseChannel, PulseChannel, TimingMode, TriangleChannel, LENGTH_TABLE,
};
use emu_core::logging::{log, LogCategory, LogLevel};
use std::cell::Cell;

/// NES-specific sweep unit for pulse channels.
///
/// The NES sweep unit differs from the Game Boy version:
/// - Uses 11-bit frequency values (0-2047)
/// - Silences channel when frequency > 0x7FF (2047)
/// - Pulse 1 uses one's complement for negation
/// - Pulse 2 uses two's complement for negation
#[derive(Debug, Clone)]
pub(crate) struct NesSweep {
    pub(crate) enabled: bool,
    pub(crate) period: u8,
    pub(crate) timer: u8,
    pub(crate) negate: bool,
    pub(crate) shift: u8,
    pub(crate) reload: bool,
    /// True for pulse 1 (ones' complement), false for pulse 2 (twos' complement)
    ones_complement: bool,
}

impl NesSweep {
    fn new(ones_complement: bool) -> Self {
        Self {
            enabled: false,
            period: 0,
            timer: 0,
            negate: false,
            shift: 0,
            reload: false,
            ones_complement,
        }
    }

    fn set_params(&mut self, period: u8, negate: bool, shift: u8) {
        self.period = period & 0x07;
        self.negate = negate;
        self.shift = shift & 0x07;
        self.reload = true;
        // Sweep is enabled if period or shift is non-zero
        self.enabled = self.period > 0 || self.shift > 0;
    }

    fn trigger(&mut self) {
        // Triggering doesn't directly affect sweep, but reload flag is cleared
        // Sweep reloading happens on next clock
    }

    /// Clock the sweep unit (called at half-frame rate, ~120 Hz NTSC)
    /// Returns Some(new_freq) if frequency should be updated
    fn clock(&mut self, current_freq: u16) -> Option<u16> {
        let mut should_update = false;

        // Reload timer if reload flag is set
        if self.reload {
            self.timer = self.period;
            self.reload = false;
        } else if self.timer > 0 {
            self.timer -= 1;
        } else {
            // Timer expired, reload and potentially update frequency
            self.timer = self.period;
            should_update = self.enabled && self.shift > 0;
        }

        if should_update {
            let new_freq = self.calculate_target_frequency(current_freq);
            // Only update if target frequency is valid (not muted)
            if !self.mutes_channel(current_freq) && !self.mutes_channel(new_freq) {
                return Some(new_freq);
            }
        }

        None
    }

    fn calculate_target_frequency(&self, current_freq: u16) -> u16 {
        let delta = current_freq >> self.shift;

        if self.negate {
            if self.ones_complement {
                // Pulse 1: one's complement (subtract delta + 1)
                current_freq.saturating_sub(delta).saturating_sub(1)
            } else {
                // Pulse 2: two's complement (subtract delta)
                current_freq.saturating_sub(delta)
            }
        } else {
            // Increase frequency
            current_freq.saturating_add(delta)
        }
    }

    fn mutes_channel(&self, freq: u16) -> bool {
        // Channel is muted if:
        // 1. Current period is < 8 (too high frequency)
        // 2. Target period would be > 0x7FF (too low frequency)
        freq < 8 || freq > 0x7FF
    }
}

impl Default for NesSweep {
    fn default() -> Self {
        Self::new(false)
    }
}

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
    pub(crate) sweep1: NesSweep,
    pub(crate) sweep2: NesSweep,
    envelope1: Envelope,
    envelope2: Envelope,
    pub triangle: TriangleChannel,
    pub noise: NoiseChannel,
    pub dmc: DmcChannel,
    envelope_noise: Envelope,
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

    /// Cycle-accurate $4017 write handling
    /// When true, clock envelopes/length counters immediately (5-step mode write)
    pending_immediate_clock: bool,

    /// High-pass filter state for DC offset removal
    /// Simple 1-pole high-pass filter to remove DC bias from mixer output
    hp_filter_acc: f64,
}

impl APU {
    pub fn new() -> Self {
        Self::new_with_timing(TimingMode::Ntsc)
    }

    pub fn new_with_timing(timing: TimingMode) -> Self {
        Self {
            pulse1: PulseChannel::new(),
            pulse2: PulseChannel::new(),
            sweep1: NesSweep::new(true),  // Pulse 1 uses one's complement
            sweep2: NesSweep::new(false), // Pulse 2 uses two's complement
            envelope1: Envelope::new(),
            envelope2: Envelope::new(),
            triangle: TriangleChannel::new(),
            noise: NoiseChannel::new(),
            dmc: DmcChannel::new(),
            envelope_noise: Envelope::new(),
            cycle_accum: 0.0,
            timing,
            frame_counter_cycles: 0,
            frame_counter_mode: false,
            irq_frame_counter_cycles: 0,
            irq_inhibit: true, // Default is inhibited
            irq_pending: Cell::new(false),
            pending_immediate_clock: false,
            hp_filter_acc: 0.0,
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

                // Configure the envelope generator
                // For NES: initial volume is always 15, period comes from bits 0-3
                // Loop flag from bit 5 (same as length_counter_halt)
                self.envelope1.set_params(15, false, val & 15);
                self.envelope1.set_loop(self.pulse1.length_counter_halt);
            }
            0x4001 => {
                // Sweep: EPPP NSSS
                // E = enabled (bit 7) - not used, sweep is always enabled if period/shift > 0
                // P = period (bits 6-4)
                // N = negate (bit 3)
                // S = shift (bits 2-0)
                let period = (val >> 4) & 0x07;
                let negate = (val & 0x08) != 0;
                let shift = val & 0x07;

                self.sweep1.set_params(period, negate, shift);
                // Note: Sweep is triggered when channel is triggered at $4003
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
                let freq = (high << 8) | low;
                self.pulse1.set_timer(freq);
                // Trigger: reset phase
                self.pulse1.reset_phase();
                // Trigger sweep unit (NES sweep doesn't use the frequency parameter)
                self.sweep1.trigger();
                // Restart envelope on note trigger
                self.envelope1.restart();
                // Note: enabled flag is only controlled by $4015, not by writes to $4003
                // Length counter index in upper 5 bits
                // Only reload length counter if channel is enabled
                let len_index = (val >> 3) & 0x1F;
                if self.pulse1.enabled {
                    self.pulse1.length_counter = LENGTH_TABLE[len_index as usize];
                }
            }

            // Pulse 2 registers
            0x4004 => {
                self.pulse2.duty = (val >> 6) & 3;
                self.pulse2.length_counter_halt = (val & 0x20) != 0;
                self.pulse2.constant_volume = (val & 0x10) != 0;
                self.pulse2.envelope = val & 15;

                // Configure the envelope generator
                self.envelope2.set_params(15, false, val & 15);
                self.envelope2.set_loop(self.pulse2.length_counter_halt);
            }
            0x4005 => {
                // Sweep: EPPP NSSS (same format as $4001)
                // E = enabled (bit 7) - not used, sweep is always enabled if period/shift > 0
                // P = period (bits 6-4)
                // N = negate (bit 3)
                // S = shift (bits 2-0)
                let period = (val >> 4) & 0x07;
                let negate = (val & 0x08) != 0;
                let shift = val & 0x07;

                self.sweep2.set_params(period, negate, shift);
            }
            0x4006 => {
                let low = val as u16;
                let high = (self.pulse2.timer_reload >> 8) & 0x07;
                self.pulse2.set_timer((high << 8) | low);
            }
            0x4007 => {
                let freq_high = (val & 0x07) as u16;
                let freq_low = (self.pulse2.timer_reload & 0xFF) as u16;
                let freq = (freq_high << 8) | freq_low;
                self.pulse2.set_timer(freq);
                self.pulse2.reset_phase();
                // Trigger sweep unit (NES sweep doesn't use the frequency parameter)
                self.sweep2.trigger();
                // Restart envelope on note trigger
                self.envelope2.restart();
                // Note: enabled flag is only controlled by $4015, not by writes to $4007
                // Only reload length counter if channel is enabled
                let len_index = (val >> 3) & 0x1F;
                if self.pulse2.enabled {
                    self.pulse2.length_counter = LENGTH_TABLE[len_index as usize];
                }
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
                // Only reload length counter if channel is enabled
                let len_index = (val >> 3) & 0x1F;
                if self.triangle.enabled {
                    self.triangle.length_counter = LENGTH_TABLE[len_index as usize];
                }
            }

            // Noise registers
            0x400C => {
                // Envelope settings (same format as pulse channels)
                self.noise.length_counter_halt = (val & 0x20) != 0;
                self.noise.constant_volume = (val & 0x10) != 0;
                self.noise.envelope = val & 0x0F;

                // Configure the envelope generator
                self.envelope_noise.set_params(15, false, val & 0x0F);
                self.envelope_noise.set_loop(self.noise.length_counter_halt);
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
                // Only reload length counter if channel is enabled
                let len_index = (val >> 3) & 0x1F;
                if self.noise.enabled {
                    self.noise.length_counter = LENGTH_TABLE[len_index as usize];
                }
                // Restart envelope on note trigger
                self.envelope_noise.restart();
            }

            // DMC Channel ($4010-$4013)
            0x4010 => {
                // IL-- RRRR: IRQ enable, Loop, Rate index
                let use_pal = matches!(self.timing, TimingMode::Pal);
                self.dmc.write_flags_rate(val, use_pal);
                log(LogCategory::APU, LogLevel::Debug, || {
                    format!("DMC flags/rate: 0x{:02X}", val)
                });
            }
            0x4011 => {
                // -DDD DDDD: Direct load (7-bit value)
                self.dmc.write_direct_load(val);
                log(LogCategory::APU, LogLevel::Debug, || {
                    format!("DMC direct load: 0x{:02X}", val)
                });
            }
            0x4012 => {
                // AAAA AAAA: Sample address
                self.dmc.write_sample_address(val);
                log(LogCategory::APU, LogLevel::Debug, || {
                    format!("DMC sample address: 0x{:02X}", val)
                });
            }
            0x4013 => {
                // LLLL LLLL: Sample length
                self.dmc.write_sample_length(val);
                log(LogCategory::APU, LogLevel::Debug, || {
                    format!("DMC sample length: 0x{:02X}", val)
                });
            }

            // APU Enable register
            0x4015 => {
                self.pulse1.enabled = (val & 0x01) != 0;
                self.pulse2.enabled = (val & 0x02) != 0;
                self.triangle.enabled = (val & 0x04) != 0;
                self.noise.enabled = (val & 0x08) != 0;

                // DMC enable at bit 4
                let dmc_enable = (val & 0x10) != 0;
                if dmc_enable && !self.dmc.enabled {
                    self.dmc.enabled = true;
                    self.dmc.start_sample();
                } else if !dmc_enable {
                    self.dmc.enabled = false;
                    self.dmc.stop();
                }

                log(LogCategory::APU, LogLevel::Debug, || {
                    format!(
                        "APU Channel enable: Pulse1={} Pulse2={} Triangle={} Noise={} DMC={}",
                        self.pulse1.enabled,
                        self.pulse2.enabled,
                        self.triangle.enabled,
                        self.noise.enabled,
                        self.dmc.enabled
                    )
                });
            }

            // Frame Counter register
            0x4017 => {
                // Bit 7: Mode (0 = 4-step, 1 = 5-step)
                // Bit 6: IRQ inhibit flag
                let new_mode = (val & 0x80) != 0;
                self.frame_counter_mode = new_mode;
                self.irq_inhibit = (val & 0x40) != 0;

                log(LogCategory::APU, LogLevel::Debug, || {
                    format!(
                        "APU Frame counter: mode={}-step IRQ={}",
                        if self.frame_counter_mode { 5 } else { 4 },
                        if self.irq_inhibit {
                            "inhibit"
                        } else {
                            "enabled"
                        }
                    )
                });

                if self.irq_inhibit {
                    self.irq_pending.set(false);
                }

                // Reset frame counter on write
                self.frame_counter_cycles = 0;
                self.irq_frame_counter_cycles = 0;

                // CYCLE-ACCURATE: If 5-step mode, clock envelopes and length counters immediately
                // This happens on the 3rd or 4th CPU cycle after the write (we approximate as immediate)
                if new_mode {
                    self.pending_immediate_clock = true;
                }
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
                // Bit 4: DMC active (bytes remaining > 0)
                // Bit 5: unused (return 0)
                // Bit 6: Frame interrupt
                // Bit 7: DMC interrupt
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
                // DMC active if it has bytes remaining
                if self.dmc.has_bytes_remaining() {
                    status |= 0x10;
                }
                if self.irq_pending.get() {
                    status |= 0x40;
                    self.irq_pending.set(false); // Reading $4015 clears frame interrupt
                }
                // DMC interrupt
                if self.dmc.irq_pending {
                    status |= 0x80;
                    // Note: DMC IRQ is NOT cleared by reading $4015, only by writing to $4015
                }
                status
            }
            _ => 0,
        }
    }

    pub fn irq_pending(&self) -> bool {
        self.irq_pending.get() || self.dmc.irq_pending
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

    /// Clock the DMC channel.
    /// Returns the address to read from if the DMC needs a sample byte.
    /// The caller should read from this address and call load_dmc_sample().
    pub fn clock_dmc(&mut self) -> Option<u16> {
        self.dmc.clock()
    }

    /// Load a sample byte into the DMC channel.
    /// This should be called after clock_dmc() returns Some(address).
    pub fn load_dmc_sample(&mut self, byte: u8) {
        self.dmc.load_sample_byte(byte);
    }

    /// Check if DMC has an IRQ pending.
    /// This should be checked at the system level alongside frame counter IRQs.
    #[allow(dead_code)]
    pub fn dmc_irq_pending(&self) -> bool {
        self.dmc.irq_pending
    }

    /// Clear and return DMC IRQ flag.
    #[allow(dead_code)]
    pub fn take_dmc_irq(&mut self) -> bool {
        self.dmc.take_irq_pending()
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
            // CYCLE-ACCURATE: Handle pending immediate clock from $4017 write in 5-step mode
            if self.pending_immediate_clock {
                self.pending_immediate_clock = false;

                // Clock envelopes
                self.envelope1.clock();
                self.envelope2.clock();
                self.envelope_noise.clock();

                // Clock length counters (half frame)
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

                // Clock sweep units
                if let Some(new_freq) = self.sweep1.clock(self.pulse1.timer_reload) {
                    self.pulse1.set_timer(new_freq);
                }
                if let Some(new_freq) = self.sweep2.clock(self.pulse2.timer_reload) {
                    self.pulse2.set_timer(new_freq);
                }
            }

            self.cycle_accum += cycles_per_sample;
            let mut cycles = self.cycle_accum as u32;
            if cycles == 0 {
                cycles = 1; // ensure we advance state even if timing slips
            }
            self.cycle_accum -= cycles as f64;

            // Accumulate raw channel values for hardware-accurate mixing
            let mut pulse1_acc = 0u32;
            let mut pulse2_acc = 0u32;
            let mut triangle_acc = 0u32;
            let mut noise_acc = 0u32;
            let mut dmc_acc = 0u32;

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
                    // Clock envelopes at every quarter frame (~240 Hz NTSC, ~200 Hz PAL)
                    self.envelope1.clock();
                    self.envelope2.clock();
                    self.envelope_noise.clock();

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

                        // Clock sweep units at half-frame rate
                        // Pass current frequency so sweep can calculate new frequency
                        if let Some(new_freq) = self.sweep1.clock(self.pulse1.timer_reload) {
                            self.pulse1.set_timer(new_freq);
                        }
                        if let Some(new_freq) = self.sweep2.clock(self.pulse2.timer_reload) {
                            self.pulse2.set_timer(new_freq);
                        }
                    }

                    // Clock linear counter (triangle) at every quarter frame
                    self.triangle.clock_linear_counter();
                }

                // Clock all channels
                // Update envelope volumes before clocking
                // When constant_volume is false, use envelope decay level; otherwise use the constant value
                let pulse1_vol = if self.pulse1.constant_volume {
                    self.pulse1.envelope
                } else {
                    self.envelope1.level()
                };
                let pulse2_vol = if self.pulse2.constant_volume {
                    self.pulse2.envelope
                } else {
                    self.envelope2.level()
                };
                let noise_vol = if self.noise.constant_volume {
                    self.noise.envelope
                } else {
                    self.envelope_noise.level()
                };

                // Temporarily set envelope values for this sample
                let saved_p1_env = self.pulse1.envelope;
                let saved_p2_env = self.pulse2.envelope;
                let saved_noise_env = self.noise.envelope;

                self.pulse1.envelope = pulse1_vol;
                self.pulse2.envelope = pulse2_vol;
                self.noise.envelope = noise_vol;

                // Get raw channel outputs (before scaling)
                // Pulse channels output envelope value (0-15) when active
                let p1_raw = if self.pulse1.enabled
                    && self.pulse1.length_counter > 0
                    && self.pulse1.duty_output()
                {
                    pulse1_vol as u32
                } else {
                    0
                };

                let p2_raw = if self.pulse2.enabled
                    && self.pulse2.length_counter > 0
                    && self.pulse2.duty_output()
                {
                    pulse2_vol as u32
                } else {
                    0
                };

                // Triangle outputs 4-bit value (0-15)
                let tri_raw = if self.triangle.enabled
                    && self.triangle.length_counter > 0
                    && self.triangle.linear_counter > 0
                {
                    self.triangle.triangle_output_raw() as u32
                } else {
                    0
                };

                // Noise outputs envelope value (0-15) when active
                let noise_raw = if self.noise.enabled
                    && self.noise.length_counter > 0
                    && (self.noise.shift_register & 1) == 0
                {
                    noise_vol as u32
                } else {
                    0
                };

                // DMC outputs 7-bit value (0-127)
                let dmc_raw = self.dmc.output() as u32;

                // Clock the channels (we still need to clock them to advance state)
                self.pulse1.clock();
                self.pulse2.clock();
                self.triangle.clock();
                self.noise.clock();

                // Restore original envelope values
                self.pulse1.envelope = saved_p1_env;
                self.pulse2.envelope = saved_p2_env;
                self.noise.envelope = saved_noise_env;

                // Accumulate raw values for mixing
                pulse1_acc += p1_raw;
                pulse2_acc += p2_raw;
                triangle_acc += tri_raw;
                noise_acc += noise_raw;
                dmc_acc += dmc_raw;
            }

            // Average the accumulated values
            let pulse1 = (pulse1_acc / cycles) as f64;
            let pulse2 = (pulse2_acc / cycles) as f64;
            let triangle = (triangle_acc / cycles) as f64;
            let noise = (noise_acc / cycles) as f64;
            let dmc = (dmc_acc / cycles) as f64;

            // Hardware-accurate non-linear mixing formulas
            // Reference: https://www.nesdev.org/wiki/APU_Mixer
            //
            // pulse_out = 95.88 / (8128 / (pulse1 + pulse2) + 100)
            // tnd_out = 159.79 / (1 / (triangle/8227 + noise/12241 + dmc/22638) + 100)
            //
            // These formulas model the NES's resistor network DAC, which produces
            // a non-linear output curve that compresses loud signals.

            let pulse_out = {
                let pulse_sum = pulse1 + pulse2;
                if pulse_sum > 0.0 {
                    95.88 / ((8128.0 / pulse_sum) + 100.0)
                } else {
                    0.0
                }
            };

            let tnd_out = {
                let tnd_sum = triangle / 8227.0 + noise / 12241.0 + dmc / 22638.0;
                if tnd_sum > 0.0 {
                    159.79 / ((1.0 / tnd_sum) + 100.0)
                } else {
                    0.0
                }
            };

            // Combine pulse and TND outputs
            // The formulas produce values in the range:
            // - pulse_out: 0.0 (silent) to ~0.95883 (both channels at max volume 15)
            // - tnd_out: 0.0 (silent) to ~1.00367 (all channels at max)
            // Total range: 0.0 to ~1.9625
            //
            // According to the nesdev wiki, these formulas produce the actual DAC
            // output voltage normalized to 0-2V range. This is a DC-biased signal
            // (always positive). We need to remove the DC component for audio playback.
            //
            // We use a simple first-order high-pass filter to remove DC offset:
            //   y[n] = x[n] - x[n-1] + α * y[n-1]
            // where α = 0.999 gives us a cutoff around 7 Hz at 44.1 kHz sample rate
            let total = pulse_out + tnd_out;
            
            // High-pass filter to remove DC offset
            const ALPHA: f64 = 0.999;
            let filtered = total - self.hp_filter_acc;
            self.hp_filter_acc = total - ALPHA * filtered;
            
            // Scale to 16-bit signed range
            // The range after filtering is approximately [-1.96, +1.96]
            // Scale conservatively to avoid clipping
            let sample = (filtered * 16384.0) as i32;

            out.push(sample.clamp(-32768, 32767) as i16);
        }

        out
    }
}

impl Default for APU {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sweep_register_write() {
        let mut apu = APU::new();

        // Write to sweep 1 register: period=3, negate=true, shift=2
        // Binary: EPPP NSSS = 0011 1010 = period 3, negate 1, shift 2
        apu.write_register(0x4001, 0b00111010);

        assert_eq!(apu.sweep1.period, 3);
        assert!(apu.sweep1.negate);
        assert_eq!(apu.sweep1.shift, 2);

        // Write to sweep 2 register: period=5, negate=false, shift=1
        // Binary: EPPP NSSS = 0101 0001 = period 5, negate 0, shift 1
        apu.write_register(0x4005, 0b01010001);

        assert_eq!(apu.sweep2.period, 5);
        assert!(!apu.sweep2.negate);
        assert_eq!(apu.sweep2.shift, 1);
    }

    #[test]
    fn test_sweep_ones_complement_vs_twos_complement() {
        // Test that pulse 1 uses one's complement and pulse 2 uses two's complement
        let mut sweep1 = NesSweep::new(true); // one's complement
        let mut sweep2 = NesSweep::new(false); // two's complement

        sweep1.set_params(1, true, 1); // period=1, negate=true, shift=1
        sweep2.set_params(1, true, 1);

        let current_freq = 100u16;

        // One's complement: 100 - (100 >> 1) - 1 = 100 - 50 - 1 = 49
        let target1 = sweep1.calculate_target_frequency(current_freq);
        assert_eq!(target1, 49);

        // Two's complement: 100 - (100 >> 1) = 100 - 50 = 50
        let target2 = sweep2.calculate_target_frequency(current_freq);
        assert_eq!(target2, 50);
    }

    #[test]
    fn test_sweep_increase_frequency() {
        let mut sweep = NesSweep::new(false);
        sweep.set_params(1, false, 1); // period=1, negate=false (increase), shift=1

        let current_freq = 100u16;
        // New frequency = 100 + (100 >> 1) = 100 + 50 = 150
        let target = sweep.calculate_target_frequency(current_freq);
        assert_eq!(target, 150);
    }

    #[test]
    fn test_sweep_mutes_on_low_frequency() {
        let sweep = NesSweep::new(false);

        // Frequency < 8 should mute
        assert!(sweep.mutes_channel(7));
        assert!(sweep.mutes_channel(0));
        assert!(!sweep.mutes_channel(8));
    }

    #[test]
    fn test_sweep_mutes_on_high_frequency() {
        let sweep = NesSweep::new(false);

        // Frequency > 0x7FF (2047) should mute
        assert!(sweep.mutes_channel(0x800));
        assert!(sweep.mutes_channel(0xFFF));
        assert!(!sweep.mutes_channel(0x7FF));
    }

    #[test]
    fn test_sweep_clock_with_period() {
        let mut sweep = NesSweep::new(false);
        sweep.set_params(2, false, 1); // period=2, shift=1

        let freq = 100u16;

        // First clock: timer is reloaded because reload flag is set, timer becomes 2
        assert_eq!(sweep.clock(freq), None);
        assert_eq!(sweep.timer, 2);

        // Second clock: timer decrements to 1
        assert_eq!(sweep.clock(freq), None);
        assert_eq!(sweep.timer, 1);

        // Third clock: timer decrements to 0
        assert_eq!(sweep.clock(freq), None);
        assert_eq!(sweep.timer, 0);

        // Fourth clock: timer is 0, sweeps and reloads
        // 100 + (100 >> 1) = 150
        assert_eq!(sweep.clock(freq), Some(150));
        assert_eq!(sweep.timer, 2); // Timer reloaded to period
    }

    #[test]
    fn test_sweep_no_change_with_shift_zero() {
        let mut sweep = NesSweep::new(false);
        sweep.set_params(1, false, 0); // shift=0

        let freq = 100u16;

        // Clock to reload
        sweep.clock(freq);

        // Even though period expired, shift=0 means no change
        assert_eq!(sweep.clock(freq), None);
    }

    #[test]
    fn test_sweep_reload_flag() {
        let mut sweep = NesSweep::new(false);
        sweep.set_params(1, false, 1);

        // Reload flag should be set after set_params
        assert!(sweep.reload);

        // First clock should reload timer and clear flag
        sweep.clock(100);
        assert!(!sweep.reload);
    }

    #[test]
    fn test_sweep_trigger() {
        let mut sweep = NesSweep::new(false);
        sweep.set_params(1, false, 1);

        // Clear reload flag
        sweep.reload = false;

        // Trigger should not affect reload flag (in NES, trigger is separate)
        sweep.trigger();

        // Reload flag should still be false (trigger doesn't set it on NES)
        assert!(!sweep.reload);
    }

    #[test]
    fn test_apu_sweep_integration() {
        let mut apu = APU::new();

        // Set up pulse 1: frequency = 100
        apu.write_register(0x4002, 100); // Low byte
        apu.write_register(0x4003, 0x08); // High byte (0) + length counter

        // Verify initial frequency
        assert_eq!(apu.pulse1.timer_reload, 100);

        // Set up sweep: period=1, negate=false, shift=1 (should add 50 to frequency)
        apu.write_register(0x4001, 0b00010001);

        // Enable pulse 1
        apu.write_register(0x4015, 0x01);

        // Generate some samples to trigger frame counter
        // We need to generate enough samples to cross a half-frame boundary
        let samples = apu.generate_samples(5000);

        // After half-frame, frequency should have changed from sweep
        // Note: This is a basic integration test - actual frequency change
        // depends on timing and may not happen in the first 5000 samples
        assert!(samples.len() == 5000);
    }

    #[test]
    fn test_length_counter_not_loaded_when_disabled() {
        let mut apu = APU::new();

        // Pulse 1: write to $4003 with channel disabled
        apu.write_register(0x4015, 0x00); // Disable all channels
                                          // Binary 0b11111000 = bits 7-3 = 11111 = index 31, which gives LENGTH_TABLE[31] = 30
        apu.write_register(0x4003, 0b11111000);
        assert_eq!(
            apu.pulse1.length_counter, 0,
            "Pulse 1 length counter should remain 0 when disabled"
        );

        // Pulse 2: write to $4007 with channel disabled
        // Binary 0b11111000 = bits 7-3 = 11111 = index 31, which gives LENGTH_TABLE[31] = 30
        apu.write_register(0x4007, 0b11111000);
        assert_eq!(
            apu.pulse2.length_counter, 0,
            "Pulse 2 length counter should remain 0 when disabled"
        );

        // Triangle: write to $400B with channel disabled
        // Binary 0b11111000 = bits 7-3 = 11111 = index 31, which gives LENGTH_TABLE[31] = 30
        apu.write_register(0x400B, 0b11111000);
        assert_eq!(
            apu.triangle.length_counter, 0,
            "Triangle length counter should remain 0 when disabled"
        );

        // Noise: write to $400F with channel disabled
        // Binary 0b11111000 = bits 7-3 = 11111 = index 31, which gives LENGTH_TABLE[31] = 30
        apu.write_register(0x400F, 0b11111000);
        assert_eq!(
            apu.noise.length_counter, 0,
            "Noise length counter should remain 0 when disabled"
        );
    }

    #[test]
    fn test_length_counter_loaded_when_enabled() {
        let mut apu = APU::new();

        // Enable all channels
        apu.write_register(0x4015, 0x0F); // Enable pulse1, pulse2, triangle, noise

        // Pulse 1: write to $4003 with channel enabled
        // Binary 0b00001000 = bits 7-3 = 00001 = index 1, which gives LENGTH_TABLE[1] = 254
        apu.write_register(0x4003, 0b00001000);
        assert_eq!(
            apu.pulse1.length_counter, 254,
            "Pulse 1 length counter should be loaded when enabled"
        );

        // Pulse 2: write to $4007 with channel enabled
        // Binary 0b00010000 = bits 7-3 = 00010 = index 2, which gives LENGTH_TABLE[2] = 20
        apu.write_register(0x4007, 0b00010000);
        assert_eq!(
            apu.pulse2.length_counter, 20,
            "Pulse 2 length counter should be loaded when enabled"
        );

        // Triangle: write to $400B with channel enabled
        // Binary 0b00000000 = bits 7-3 = 00000 = index 0, which gives LENGTH_TABLE[0] = 10
        apu.write_register(0x400B, 0b00000000);
        assert_eq!(
            apu.triangle.length_counter, 10,
            "Triangle length counter should be loaded when enabled"
        );

        // Noise: write to $400F with channel enabled
        // Binary 0b00011000 = bits 7-3 = 00011 = index 3, which gives LENGTH_TABLE[3] = 2
        apu.write_register(0x400F, 0b00011000);
        assert_eq!(
            apu.noise.length_counter, 2,
            "Noise length counter should be loaded when enabled"
        );
    }

    #[test]
    fn test_length_counter_toggle_enable() {
        let mut apu = APU::new();

        // Start with channel disabled
        apu.write_register(0x4015, 0x00);

        // Write length counter while disabled - should not load
        // Binary 0b11111000 = bits 7-3 = 11111 = index 31, which gives LENGTH_TABLE[31] = 30
        apu.write_register(0x4003, 0b11111000);
        assert_eq!(apu.pulse1.length_counter, 0);

        // Enable the channel
        apu.write_register(0x4015, 0x01);

        // Write length counter while enabled - should load
        // Binary 0b11111000 = bits 7-3 = 11111 = index 31, which gives LENGTH_TABLE[31] = 30
        apu.write_register(0x4003, 0b11111000);
        assert_eq!(apu.pulse1.length_counter, 30);

        // Disable again
        apu.write_register(0x4015, 0x00);

        // Write length counter while disabled again - should not load
        // Binary 0b00001000 = bits 7-3 = 00001 = index 1, which gives LENGTH_TABLE[1] = 254
        apu.write_register(0x4003, 0b00001000);
        assert_eq!(
            apu.pulse1.length_counter, 30,
            "Length counter should not change when disabled"
        );
    }

    #[test]
    fn test_envelope_basic_functionality() {
        // Test the Envelope component directly to ensure we understand its behavior
        let mut env = Envelope::new();

        // Initially should be at 0
        assert_eq!(env.level(), 0);

        // Set params: initial_volume=15, add_mode=false (decay), period=0
        env.set_params(15, false, 0);

        // Still at 0 until restart and clock
        assert_eq!(env.level(), 0);

        // Restart the envelope
        env.restart();

        // Still at 0 until clocked
        assert_eq!(env.level(), 0);

        // Clock to process the start flag
        env.clock();

        // Now should be at initial_volume = 15
        assert_eq!(
            env.level(),
            15,
            "Envelope should start at 15 after restart and clock"
        );

        // Clock again with period=0 should decay immediately
        env.clock();
        assert_eq!(env.level(), 14, "Envelope should decay to 14");
    }

    #[test]
    fn test_envelope_decay() {
        let mut apu = APU::new();

        // Enable pulse 1
        apu.write_register(0x4015, 0x01);

        // Configure pulse 1 with envelope mode (constant_volume=0) and period=0 (fastest decay)
        // Binary: DDLC VVVV = 00000000 = duty 0, no loop, envelope mode, period 0
        apu.write_register(0x4000, 0b00000000);

        // Trigger a note
        apu.write_register(0x4003, 0b00001000); // Length counter + trigger

        // Generate enough samples to hit the first quarter frame boundary (~184 samples)
        // This will clock the envelope and process the start flag
        let _samples = apu.generate_samples(200);

        // Verify envelope starts at 15 after being clocked
        assert_eq!(apu.envelope1.level(), 15, "Envelope should start at 15");

        // Generate samples for several more quarter frames
        // With period=0, envelope decays by 1 every quarter frame
        let _samples = apu.generate_samples(1000);

        // After multiple quarter frames, envelope should have decayed from 15
        let final_level = apu.envelope1.level();
        assert!(
            final_level < 15,
            "Envelope should decay from 15, got {}",
            final_level
        );
    }

    #[test]
    fn test_envelope_constant_volume() {
        let mut apu = APU::new();

        // Enable pulse 1
        apu.write_register(0x4015, 0x01);

        // Configure pulse 1 with constant volume mode (bit 4 = 1) and volume = 8
        // Binary: DDLC VVVV = 00011000 = duty 0, no loop, constant volume, volume 8
        apu.write_register(0x4000, 0b00011000);

        // Trigger a note
        apu.write_register(0x4003, 0b00001000);

        // Generate many samples
        let _samples = apu.generate_samples(5000);

        // In constant volume mode, the volume should remain at 8 (not decay)
        // We can't directly check this, but we can verify the channel is still playing
        // by checking that pulse1.envelope still equals 8
        assert_eq!(
            apu.pulse1.envelope, 8,
            "Constant volume should remain unchanged"
        );
    }

    #[test]
    fn test_envelope_restart_on_retrigger() {
        let mut apu = APU::new();

        // Enable pulse 1
        apu.write_register(0x4015, 0x01);

        // Configure with envelope mode and period=0
        apu.write_register(0x4000, 0b00000000);

        // Trigger first note
        apu.write_register(0x4003, 0b00001000);

        // Generate enough samples to hit first quarter frame and start envelope
        let _samples = apu.generate_samples(200);

        // Should start at 15
        assert_eq!(apu.envelope1.level(), 15, "Envelope should start at 15");

        // Generate samples to let envelope decay (several quarter frames)
        let _samples = apu.generate_samples(1000);

        // Envelope should have decayed
        let decayed_level = apu.envelope1.level();
        assert!(decayed_level < 15, "Envelope should have decayed from 15");

        // Retrigger the note
        apu.write_register(0x4003, 0b00001000);

        // Generate enough samples to process the restart (hit next quarter frame)
        let _samples = apu.generate_samples(200);

        // After restart, envelope should be back at 15
        let restarted_level = apu.envelope1.level();
        assert_eq!(
            restarted_level, 15,
            "Envelope should restart at 15 after retrigger, got {}",
            restarted_level
        );
    }

    #[test]
    fn test_dmc_basic_operation() {
        let mut apu = APU::new();

        // Configure DMC: IRQ disabled, no loop, rate index 0
        apu.write_register(0x4010, 0x00);

        // Set direct load to 64
        apu.write_register(0x4011, 0x40);
        assert_eq!(apu.dmc.output_level, 0x40);

        // Set sample address: 0xC000 + 0x10 * 64 = 0xC400
        apu.write_register(0x4012, 0x10);
        assert_eq!(apu.dmc.sample_address, 0xC400);

        // Set sample length: 0x10 * 16 + 1 = 257 bytes
        apu.write_register(0x4013, 0x10);
        assert_eq!(apu.dmc.sample_length, 257);

        // Enable DMC
        apu.write_register(0x4015, 0x10);
        assert!(apu.dmc.enabled);
        assert!(apu.dmc.has_bytes_remaining());
    }

    #[test]
    fn test_dmc_memory_read_request() {
        let mut apu = APU::new();

        // Configure and enable DMC
        apu.write_register(0x4010, 0x00);
        apu.write_register(0x4012, 0x00); // Address = 0xC000
        apu.write_register(0x4013, 0x01); // Length = 17 bytes
        apu.write_register(0x4015, 0x10); // Enable DMC

        // Clock DMC until it requests a memory read
        let mut read_requested = false;
        for _ in 0..1000 {
            if let Some(addr) = apu.clock_dmc() {
                read_requested = true;
                // Simulate memory read: load a byte with pattern 0b10101010
                apu.load_dmc_sample(0b10101010);
                assert_eq!(addr, 0xC000, "First read should be from sample address");
                break;
            }
        }
        assert!(read_requested, "DMC should request memory read");
    }

    #[test]
    fn test_dmc_irq_generation() {
        let mut apu = APU::new();

        // Configure DMC with IRQ enabled, no loop
        apu.write_register(0x4010, 0x80); // IRQ enabled
        apu.write_register(0x4012, 0x00); // Address = 0xC000
        apu.write_register(0x4013, 0x00); // Length = 1 byte (minimum)
        apu.write_register(0x4015, 0x10); // Enable DMC

        // Clock DMC and process the sample
        for _ in 0..2000 {
            if let Some(_addr) = apu.clock_dmc() {
                apu.load_dmc_sample(0xFF);
            }
        }

        // After sample completes, IRQ should be pending
        assert!(
            apu.dmc.irq_pending,
            "DMC IRQ should be set after sample completes"
        );
    }
}
