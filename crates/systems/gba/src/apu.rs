//! GBA APU (Audio Processing Unit) implementation.
//!
//! The GBA audio system consists of:
//!
//! ## PSG Channels (inherited from Game Boy)
//!
//! 1. **Pulse 1**: Square wave with sweep (SOUND1CNT_L/H/X)
//! 2. **Pulse 2**: Square wave without sweep (SOUND2CNT_L/H)
//! 3. **Wave**: Programmable waveform with wave RAM (SOUND3CNT_L/H/X)
//! 4. **Noise**: LFSR-based noise (SOUND4CNT_L/H)
//!
//! ## DMA Sound Channels (GBA-specific)
//!
//! - **FIFO A**: 32-byte FIFO fed by DMA, driven by Timer 0 or 1
//! - **FIFO B**: 32-byte FIFO fed by DMA, driven by Timer 0 or 1
//!
//! ## Register Map (offsets from 0x04000000)
//!
//! | Offset | Name        | Description                          |
//! |--------|-------------|--------------------------------------|
//! | 0x060  | SOUND1CNT_L | Channel 1 Sweep                      |
//! | 0x062  | SOUND1CNT_H | Channel 1 Duty/Length/Envelope        |
//! | 0x064  | SOUND1CNT_X | Channel 1 Frequency/Control          |
//! | 0x068  | SOUND2CNT_L | Channel 2 Duty/Length/Envelope        |
//! | 0x06C  | SOUND2CNT_H | Channel 2 Frequency/Control          |
//! | 0x070  | SOUND3CNT_L | Channel 3 Stop/Wave RAM Select       |
//! | 0x072  | SOUND3CNT_H | Channel 3 Length/Volume               |
//! | 0x074  | SOUND3CNT_X | Channel 3 Frequency/Control          |
//! | 0x078  | SOUND4CNT_L | Channel 4 Length/Envelope             |
//! | 0x07C  | SOUND4CNT_H | Channel 4 Frequency/Control          |
//! | 0x080  | SOUNDCNT_L  | PSG Master Volume/Panning            |
//! | 0x082  | SOUNDCNT_H  | DMA Sound Control/Mixing             |
//! | 0x084  | SOUNDCNT_X  | Sound On/Off (Master Enable)         |
//! | 0x088  | SOUNDBIAS   | Sound PWM Control                    |
//! | 0x090  | WAVE_RAM    | Channel 3 Wave RAM (16 bytes)        |
//! | 0x0A0  | FIFO_A      | DMA Sound A FIFO (write only)        |
//! | 0x0A4  | FIFO_B      | DMA Sound B FIFO (write only)        |
//!
//! ## Timing
//!
//! - CPU clock: 16,777,216 Hz (2^24)
//! - PSG channels clock at CPU/4 = 4,194,304 Hz (same as GB)
//! - Frame sequencer: 512 Hz (every 32,768 CPU cycles)
//! - DMA sound: 1 sample per timer overflow (typically 16-65 kHz)

use emu_core::apu::{Envelope, LengthCounter, NoiseChannel, PulseChannel, SweepUnit, WaveChannel};

/// GBA CPU clock frequency
const CPU_CLOCK: f64 = 16_777_216.0;

/// Target audio sample rate in Hz
const SAMPLE_RATE: f64 = 44_100.0;

/// CPU cycles per output sample
const CYCLES_PER_SAMPLE: f64 = CPU_CLOCK / SAMPLE_RATE;

/// CPU cycles per frame sequencer step (512 Hz)
const CYCLES_PER_FRAME_STEP: u32 = 32_768;

/// PSG prescaler: PSG channels run at CPU/4
const PSG_PRESCALER: u32 = 4;

/// FIFO buffer capacity in bytes
const FIFO_CAPACITY: usize = 32;

// =============================================================================
// DMA Sound FIFO
// =============================================================================

/// A 32-byte circular FIFO buffer for DMA sound channels.
#[derive(Debug, Clone)]
pub struct SoundFifo {
    buffer: [i8; FIFO_CAPACITY],
    read_pos: usize,
    write_pos: usize,
    count: usize,
    /// Current output sample (held between pops)
    current_sample: i8,
}

impl SoundFifo {
    fn new() -> Self {
        Self {
            buffer: [0; FIFO_CAPACITY],
            read_pos: 0,
            write_pos: 0,
            count: 0,
            current_sample: 0,
        }
    }

    /// Push 4 bytes (one word) into the FIFO.
    fn push_word(&mut self, data: u32) {
        for i in 0..4 {
            if self.count < FIFO_CAPACITY {
                self.buffer[self.write_pos] = ((data >> (i * 8)) & 0xFF) as i8;
                self.write_pos = (self.write_pos + 1) % FIFO_CAPACITY;
                self.count += 1;
            }
        }
    }

    /// Pop one sample from the FIFO. Returns the sample and updates held value.
    fn pop(&mut self) -> i8 {
        if self.count > 0 {
            self.current_sample = self.buffer[self.read_pos];
            self.read_pos = (self.read_pos + 1) % FIFO_CAPACITY;
            self.count -= 1;
        }
        // If empty, holds last sample
        self.current_sample
    }

    /// Reset the FIFO to empty state.
    fn reset(&mut self) {
        self.read_pos = 0;
        self.write_pos = 0;
        self.count = 0;
        self.current_sample = 0;
        self.buffer = [0; FIFO_CAPACITY];
    }

    /// Check if FIFO needs refilling (has room for at least 4 words = 16 bytes).
    fn needs_refill(&self) -> bool {
        self.count <= 16
    }
}

// =============================================================================
// GBA APU
// =============================================================================

/// GBA Audio Processing Unit.
///
/// Manages 4 PSG channels (inherited from Game Boy) plus 2 DMA sound FIFOs.
#[derive(Debug, Clone)]
pub struct GbaApu {
    // ---- PSG Channel 1: Pulse with Sweep ----
    pulse1: PulseChannel,
    pulse1_sweep: SweepUnit,
    pulse1_envelope: Envelope,
    pulse1_length: LengthCounter,
    pulse1_frequency: u16,

    // ---- PSG Channel 2: Pulse ----
    pulse2: PulseChannel,
    pulse2_envelope: Envelope,
    pulse2_length: LengthCounter,
    pulse2_frequency: u16,

    // ---- PSG Channel 3: Wave ----
    wave: WaveChannel,
    wave_length: LengthCounter,
    wave_frequency: u16,
    wave_dac_enabled: bool,
    /// GBA wave RAM bank select (bit 6 of SOUND3CNT_L)
    wave_bank_select: u8,
    /// GBA wave RAM dimension (bit 5: 0=one bank, 1=two banks)
    wave_bank_mode: bool,

    // ---- PSG Channel 4: Noise ----
    noise: NoiseChannel,
    noise_envelope: Envelope,
    noise_length: LengthCounter,

    // ---- DMA Sound FIFOs ----
    pub fifo_a: SoundFifo,
    pub fifo_b: SoundFifo,

    // ---- DMA Sound Control (SOUNDCNT_H bits) ----
    /// PSG volume ratio: 0=25%, 1=50%, 2=100%
    psg_volume: u8,
    /// FIFO A volume: false=50%, true=100%
    fifo_a_full_volume: bool,
    /// FIFO B volume: false=50%, true=100%
    fifo_b_full_volume: bool,
    /// FIFO A output to right speaker
    fifo_a_right: bool,
    /// FIFO A output to left speaker
    fifo_a_left: bool,
    /// FIFO A timer select: false=Timer 0, true=Timer 1
    fifo_a_timer: bool,
    /// FIFO B output to right speaker
    fifo_b_right: bool,
    /// FIFO B output to left speaker
    fifo_b_left: bool,
    /// FIFO B timer select: false=Timer 0, true=Timer 1
    fifo_b_timer: bool,

    // ---- Master Controls (SOUNDCNT_L) ----
    /// PSG master volume left (0-7)
    psg_left_volume: u8,
    /// PSG master volume right (0-7)
    psg_right_volume: u8,
    /// PSG channel panning (bits 0-3: right ch1-4, bits 4-7: left ch1-4)
    psg_panning: u8,

    // ---- Frame Sequencer ----
    frame_sequencer_cycles: u32,
    frame_sequencer_step: u8,

    // ---- PSG Prescaler ----
    psg_prescaler: u32,

    // ---- Master Enable ----
    power_on: bool,

    // ---- Sample Generation State ----
    cycle_accum: f64,
    last_pulse1: i16,
    last_pulse2: i16,
    last_wave: i16,

    // ---- Buffered Output Samples ----
    /// Stereo samples (interleaved L, R) generated during step_frame.
    sample_buffer: Vec<i16>,

    // ---- Audio Filtering ----
    dc_prev_in_l: f32,
    dc_prev_out_l: f32,
    dc_prev_in_r: f32,
    dc_prev_out_r: f32,
}

impl Default for GbaApu {
    fn default() -> Self {
        Self::new()
    }
}

impl GbaApu {
    /// Create a new GBA APU in its initial state.
    pub fn new() -> Self {
        Self {
            pulse1: PulseChannel::new(),
            pulse1_sweep: SweepUnit::new(),
            pulse1_envelope: Envelope::new(),
            pulse1_length: LengthCounter::new(),
            pulse1_frequency: 0,

            pulse2: PulseChannel::new(),
            pulse2_envelope: Envelope::new(),
            pulse2_length: LengthCounter::new(),
            pulse2_frequency: 0,

            wave: WaveChannel::new(),
            wave_length: LengthCounter::new(),
            wave_frequency: 0,
            wave_dac_enabled: false,
            wave_bank_select: 0,
            wave_bank_mode: false,

            noise: NoiseChannel::new(),
            noise_envelope: Envelope::new(),
            noise_length: LengthCounter::new(),

            fifo_a: SoundFifo::new(),
            fifo_b: SoundFifo::new(),

            psg_volume: 2, // 100% by default
            fifo_a_full_volume: true,
            fifo_b_full_volume: true,
            fifo_a_right: false,
            fifo_a_left: false,
            fifo_a_timer: false,
            fifo_b_right: false,
            fifo_b_left: false,
            fifo_b_timer: false,

            psg_left_volume: 7,
            psg_right_volume: 7,
            psg_panning: 0xFF,

            frame_sequencer_cycles: 0,
            frame_sequencer_step: 0,

            psg_prescaler: 0,

            power_on: true,

            cycle_accum: 0.0,
            last_pulse1: 0,
            last_pulse2: 0,
            last_wave: 0,

            sample_buffer: Vec::with_capacity(1600),

            dc_prev_in_l: 0.0,
            dc_prev_out_l: 0.0,
            dc_prev_in_r: 0.0,
            dc_prev_out_r: 0.0,
        }
    }

    /// Reset the APU to initial state.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    // =========================================================================
    // Clocking
    // =========================================================================

    /// Clock the APU for one CPU cycle.
    ///
    /// The PSG channels run at CPU/4 (same rate as Game Boy).
    /// The frame sequencer runs at 512 Hz.
    pub fn clock(&mut self) {
        if !self.power_on {
            return;
        }

        // Frame sequencer at 512 Hz (every 32768 CPU cycles)
        self.frame_sequencer_cycles += 1;
        if self.frame_sequencer_cycles >= CYCLES_PER_FRAME_STEP {
            self.frame_sequencer_cycles = 0;
            self.clock_frame_sequencer();
        }

        // PSG prescaler: tick channels at CPU/4
        self.psg_prescaler += 1;
        if self.psg_prescaler >= PSG_PRESCALER {
            self.psg_prescaler = 0;
            self.last_pulse1 = self.pulse1.clock();
            self.last_pulse2 = self.pulse2.clock();
            self.last_wave = self.wave.clock();
            let _ = self.noise.clock();
        }
    }

    /// Clock the 512 Hz frame sequencer.
    /// Controls length counters, sweep, and envelopes.
    fn clock_frame_sequencer(&mut self) {
        match self.frame_sequencer_step {
            // Length counters clock at 256 Hz (every other step)
            0 | 2 | 4 | 6 => {
                self.pulse1_length.clock();
                self.pulse2_length.clock();
                self.wave_length.clock();
                self.noise_length.clock();

                if !self.pulse1_length.is_active() {
                    self.pulse1.enabled = false;
                }
                if !self.pulse2_length.is_active() {
                    self.pulse2.enabled = false;
                }
                if !self.wave_length.is_active() {
                    self.wave.enabled = false;
                }
                if !self.noise_length.is_active() {
                    self.noise.enabled = false;
                }

                // Sweep clocks at 128 Hz (steps 2 and 6)
                if self.frame_sequencer_step == 2 || self.frame_sequencer_step == 6 {
                    if let Some(new_freq) = self.pulse1_sweep.clock() {
                        self.pulse1_frequency = new_freq;
                        self.pulse1.set_timer(gba_square_timer(new_freq));
                    }
                }
            }
            // Envelope clocks at 64 Hz (step 7)
            7 => {
                self.pulse1_envelope.clock();
                self.pulse2_envelope.clock();
                self.noise_envelope.clock();

                self.pulse1.envelope = self.pulse1_envelope.volume();
                self.pulse2.envelope = self.pulse2_envelope.volume();
                self.noise.envelope = self.noise_envelope.volume();
            }
            _ => {}
        }
        self.frame_sequencer_step = (self.frame_sequencer_step + 1) & 7;
    }

    // =========================================================================
    // Timer Overflow (for DMA FIFO feeding)
    // =========================================================================

    /// Notify the APU that a timer has overflowed.
    ///
    /// DMA sound channels are driven by Timer 0 or Timer 1 overflows.
    /// Each overflow pops one sample from the corresponding FIFO.
    ///
    /// Returns a bitmask: bit 0 = FIFO A needs refill, bit 1 = FIFO B needs refill.
    pub fn on_timer_overflow(&mut self, timer_index: u8) -> u8 {
        let mut refill_request = 0u8;

        // FIFO A
        if (timer_index == 0 && !self.fifo_a_timer) || (timer_index == 1 && self.fifo_a_timer) {
            self.fifo_a.pop();
            if self.fifo_a.needs_refill() {
                refill_request |= 1;
            }
        }

        // FIFO B
        if (timer_index == 0 && !self.fifo_b_timer) || (timer_index == 1 && self.fifo_b_timer) {
            self.fifo_b.pop();
            if self.fifo_b.needs_refill() {
                refill_request |= 2;
            }
        }

        refill_request
    }

    // =========================================================================
    // Real-time Sample Generation
    // =========================================================================

    /// Advance the APU by `cpu_cycles` CPU cycles, generating output samples
    /// into the internal buffer at 44,100 Hz.
    ///
    /// Call this during step_frame AFTER timer ticks and FIFO pops so that
    /// mix_channels reads the current FIFO sample at the right time.
    pub fn tick(&mut self, cpu_cycles: u32) {
        for _ in 0..cpu_cycles {
            self.clock();
            self.cycle_accum += 1.0;

            if self.cycle_accum >= CYCLES_PER_SAMPLE {
                self.cycle_accum -= CYCLES_PER_SAMPLE;
                let (left, right) = self.mix_channels();
                self.sample_buffer.push(left);
                self.sample_buffer.push(right);
            }
        }
    }

    /// Drain buffered stereo samples, returning them and clearing the buffer.
    ///
    /// Returns interleaved L, R, L, R, ... i16 samples at 44,100 Hz.
    /// If `target_stereo_count` is specified, pads with silence or truncates.
    pub fn drain_samples(&mut self, target_stereo_count: usize) -> Vec<i16> {
        let mut samples = std::mem::take(&mut self.sample_buffer);
        self.sample_buffer = Vec::with_capacity(1600);
        samples.resize(target_stereo_count, 0);
        samples
    }

    /// Mix all channels into a stereo sample pair.
    fn mix_channels(&mut self) -> (i16, i16) {
        if !self.power_on {
            return (0, 0);
        }

        // --- PSG mixing ---
        // Hardware sums all enabled PSG channels (no averaging).
        let mut psg_left = 0i32;
        let mut psg_right = 0i32;

        let pan = self.psg_panning;

        // Channel 1: Pulse with sweep
        // When length counter is disabled, channel plays indefinitely (real hardware behavior).
        if self.pulse1.enabled
            && (!self.pulse1_length.is_enabled() || self.pulse1_length.is_active())
        {
            let sample = self.last_pulse1 as i32;
            if pan & (1 << 4) != 0 {
                psg_left += sample;
            }
            if pan & (1 << 0) != 0 {
                psg_right += sample;
            }
        }

        // Channel 2: Pulse
        if self.pulse2.enabled
            && (!self.pulse2_length.is_enabled() || self.pulse2_length.is_active())
        {
            let sample = self.last_pulse2 as i32;
            if pan & (1 << 5) != 0 {
                psg_left += sample;
            }
            if pan & (1 << 1) != 0 {
                psg_right += sample;
            }
        }

        // Channel 3: Wave
        // Wave output is unipolar (0 to max). Center it for proper mixing.
        if self.wave.enabled
            && (!self.wave_length.is_enabled() || self.wave_length.is_active())
            && self.wave_dac_enabled
        {
            // last_wave ranges from 0 to (sample << 10). Center around 0.
            let sample = (self.last_wave as i32) * 2 - 15360;
            if pan & (1 << 6) != 0 {
                psg_left += sample;
            }
            if pan & (1 << 2) != 0 {
                psg_right += sample;
            }
        }

        // Channel 4: Noise
        // Noise output is unipolar (0 or envelope). Make bipolar.
        if self.noise.enabled && (!self.noise_length.is_enabled() || self.noise_length.is_active())
        {
            let sample = if self.noise.is_silenced() {
                -((self.noise.envelope as i32) << 10)
            } else {
                (self.noise.envelope as i32) << 10
            };
            if pan & (1 << 7) != 0 {
                psg_left += sample;
            }
            if pan & (1 << 3) != 0 {
                psg_right += sample;
            }
        }

        // Apply PSG master volume (0-7). Scale: volume/8 maps 0-7 to 0.0-0.875.
        // Divide by 4 to keep PSG at a reasonable level relative to FIFO channels.
        psg_left = psg_left * (1 + self.psg_left_volume as i32) / 32;
        psg_right = psg_right * (1 + self.psg_right_volume as i32) / 32;

        // Apply PSG volume ratio from SOUNDCNT_H (bits 0-1)
        let psg_shift = match self.psg_volume {
            0 => 2, // 25%
            1 => 1, // 50%
            _ => 0, // 100%
        };
        psg_left >>= psg_shift;
        psg_right >>= psg_shift;

        // --- DMA FIFO mixing ---
        // FIFO samples are signed 8-bit, scaled to ~i16 range.
        // Use << 7 (not << 8) to leave headroom for mixing both FIFOs + PSG.
        let fifo_a_sample = (self.fifo_a.current_sample as i32) << 7;
        let fifo_b_sample = (self.fifo_b.current_sample as i32) << 7;

        // Apply FIFO volume (50% or 100%)
        let fifo_a_scaled = if self.fifo_a_full_volume {
            fifo_a_sample
        } else {
            fifo_a_sample >> 1
        };
        let fifo_b_scaled = if self.fifo_b_full_volume {
            fifo_b_sample
        } else {
            fifo_b_sample >> 1
        };

        // Mix FIFO into L/R based on panning
        let mut left = psg_left;
        let mut right = psg_right;

        if self.fifo_a_left {
            left += fifo_a_scaled;
        }
        if self.fifo_a_right {
            right += fifo_a_scaled;
        }
        if self.fifo_b_left {
            left += fifo_b_scaled;
        }
        if self.fifo_b_right {
            right += fifo_b_scaled;
        }

        // DC-blocking filter only (removes DC offset, preserves signal).
        let left_out = dc_block(left as f32, &mut self.dc_prev_in_l, &mut self.dc_prev_out_l);
        let right_out = dc_block(
            right as f32,
            &mut self.dc_prev_in_r,
            &mut self.dc_prev_out_r,
        );

        (left_out, right_out)
    }

    // =========================================================================
    // Register Access
    // =========================================================================

    /// Write to a GBA sound register.
    ///
    /// `addr` is the full I/O address (e.g. 0x04000060).
    pub fn write_register(&mut self, addr: u32, val: u8) {
        // Master enable gate: when power is off, only SOUNDCNT_X (0x084) is writable
        if !self.power_on && addr != 0x04000084 && addr != 0x04000085 {
            return;
        }

        match addr {
            // ---- Channel 1: Sweep ----
            // SOUND1CNT_L (0x060): Sweep register
            0x04000060 => {
                self.pulse1_sweep.shift = val & 0x07;
                self.pulse1_sweep.negate = (val & 0x08) != 0;
                self.pulse1_sweep.period = (val >> 4) & 0x07;
            }
            0x04000061 => {} // High byte of SOUND1CNT_L (unused)

            // SOUND1CNT_H (0x062): Duty/Length/Envelope
            0x04000062 => {
                let length_load = val & 0x3F;
                self.pulse1_length.load_gb(length_load, 64);
                self.pulse1.duty = (val >> 6) & 0x03;
            }
            0x04000063 => {
                let initial_volume = (val >> 4) & 0x0F;
                let add_mode = (val & 0x08) != 0;
                let period = val & 0x07;
                self.pulse1_envelope
                    .set_params(initial_volume, add_mode, period);
                if (val & 0xF8) == 0 {
                    self.pulse1.enabled = false;
                }
            }

            // SOUND1CNT_X (0x064): Frequency/Control
            0x04000064 => {
                self.pulse1_frequency = (self.pulse1_frequency & 0x0700) | (val as u16);
            }
            0x04000065 => {
                self.pulse1_frequency =
                    (self.pulse1_frequency & 0x00FF) | (((val & 0x07) as u16) << 8);
                let length_enable = (val & 0x40) != 0;
                let trigger = (val & 0x80) != 0;
                self.pulse1_length.set_enabled(length_enable);
                if trigger {
                    self.pulse1.enabled = true;
                    self.pulse1.length_counter = 64; // Keep internal counter non-zero; GBA uses external LengthCounter
                    self.pulse1
                        .set_timer(gba_square_timer(self.pulse1_frequency));
                    self.pulse1_envelope.trigger();
                    self.pulse1_sweep.trigger(self.pulse1_frequency);
                    if self.pulse1_length.value() == 0 {
                        self.pulse1_length.load_gb(0, 64);
                    }
                }
            }

            // ---- Channel 2: Pulse ----
            // SOUND2CNT_L (0x068): Duty/Length/Envelope
            0x04000068 => {
                let length_load = val & 0x3F;
                self.pulse2_length.load_gb(length_load, 64);
                self.pulse2.duty = (val >> 6) & 0x03;
            }
            0x04000069 => {
                let initial_volume = (val >> 4) & 0x0F;
                let add_mode = (val & 0x08) != 0;
                let period = val & 0x07;
                self.pulse2_envelope
                    .set_params(initial_volume, add_mode, period);
                if (val & 0xF8) == 0 {
                    self.pulse2.enabled = false;
                }
            }

            // SOUND2CNT_H (0x06C): Frequency/Control
            0x0400006C => {
                self.pulse2_frequency = (self.pulse2_frequency & 0x0700) | (val as u16);
            }
            0x0400006D => {
                self.pulse2_frequency =
                    (self.pulse2_frequency & 0x00FF) | (((val & 0x07) as u16) << 8);
                let length_enable = (val & 0x40) != 0;
                let trigger = (val & 0x80) != 0;
                self.pulse2_length.set_enabled(length_enable);
                if trigger {
                    self.pulse2.enabled = true;
                    self.pulse2.length_counter = 64; // Keep internal counter non-zero; GBA uses external LengthCounter
                    self.pulse2
                        .set_timer(gba_square_timer(self.pulse2_frequency));
                    self.pulse2_envelope.trigger();
                    if self.pulse2_length.value() == 0 {
                        self.pulse2_length.load_gb(0, 64);
                    }
                }
            }

            // ---- Channel 3: Wave ----
            // SOUND3CNT_L (0x070): Stop/Wave RAM bank
            0x04000070 => {
                self.wave_bank_mode = (val & 0x20) != 0;
                self.wave_bank_select = (val >> 6) & 1;
                self.wave_dac_enabled = (val & 0x80) != 0;
                if !self.wave_dac_enabled {
                    self.wave.enabled = false;
                }
            }
            0x04000071 => {} // High byte unused

            // SOUND3CNT_H (0x072): Length/Volume
            0x04000072 => {
                self.wave_length.load_gb(val, 256);
            }
            0x04000073 => {
                // Volume: bits 5-6 (GBA also has bit 7 for force 75%)
                let vol_code = (val >> 5) & 0x03;
                self.wave.volume_shift = vol_code;
            }

            // SOUND3CNT_X (0x074): Frequency/Control
            0x04000074 => {
                self.wave_frequency = (self.wave_frequency & 0x0700) | (val as u16);
            }
            0x04000075 => {
                self.wave_frequency = (self.wave_frequency & 0x00FF) | (((val & 0x07) as u16) << 8);
                let length_enable = (val & 0x40) != 0;
                let trigger = (val & 0x80) != 0;
                self.wave_length.set_enabled(length_enable);
                if trigger && self.wave_dac_enabled {
                    self.wave.enabled = true;
                    self.wave.set_timer(gba_wave_timer(self.wave_frequency));
                    self.wave.reset_position();
                    if self.wave_length.value() == 0 {
                        self.wave_length.load_gb(0, 256);
                    }
                }
            }

            // ---- Channel 4: Noise ----
            // SOUND4CNT_L (0x078): Length/Envelope
            0x04000078 => {
                let length_load = val & 0x3F;
                self.noise_length.load_gb(length_load, 64);
            }
            0x04000079 => {
                let initial_volume = (val >> 4) & 0x0F;
                let add_mode = (val & 0x08) != 0;
                let period = val & 0x07;
                self.noise_envelope
                    .set_params(initial_volume, add_mode, period);
                if (val & 0xF8) == 0 {
                    self.noise.enabled = false;
                }
            }

            // SOUND4CNT_H (0x07C): Frequency/Control
            0x0400007C => {
                let width = (val & 0x08) != 0;
                self.noise.mode = width;
                self.noise.set_period(gba_noise_period_index(val));
            }
            0x0400007D => {
                let length_enable = (val & 0x40) != 0;
                let trigger = (val & 0x80) != 0;
                self.noise_length.set_enabled(length_enable);
                if trigger {
                    self.noise.enabled = true;
                    self.noise.length_counter = 64; // Keep internal counter non-zero; GBA uses external LengthCounter
                    self.noise_envelope.trigger();
                    self.noise.reset_shift_register();
                    if self.noise_length.value() == 0 {
                        self.noise_length.load_gb(0, 64);
                    }
                }
            }

            // ---- SOUNDCNT_L (0x080): PSG Master Volume/Panning ----
            0x04000080 => {
                self.psg_right_volume = val & 0x07;
                self.psg_left_volume = (val >> 4) & 0x07;
            }
            0x04000081 => {
                self.psg_panning = val;
            }

            // ---- SOUNDCNT_H (0x082): DMA Sound Control ----
            0x04000082 => {
                self.psg_volume = val & 0x03;
                self.fifo_a_full_volume = (val & 0x04) != 0;
                self.fifo_b_full_volume = (val & 0x08) != 0;
            }
            0x04000083 => {
                self.fifo_a_right = (val & 0x01) != 0;
                self.fifo_a_left = (val & 0x02) != 0;
                self.fifo_a_timer = (val & 0x04) != 0;
                if val & 0x08 != 0 {
                    self.fifo_a.reset();
                }
                self.fifo_b_right = (val & 0x10) != 0;
                self.fifo_b_left = (val & 0x20) != 0;
                self.fifo_b_timer = (val & 0x40) != 0;
                if val & 0x80 != 0 {
                    self.fifo_b.reset();
                }
            }

            // ---- SOUNDCNT_X (0x084): Master Enable ----
            0x04000084 => {
                let new_power = (val & 0x80) != 0;
                if !new_power && self.power_on {
                    self.power_off();
                }
                self.power_on = new_power;
            }
            0x04000085 => {} // High byte unused

            // ---- SOUNDBIAS (0x088-0x089) ----
            // Just stored in I/O, no special handling needed
            0x04000088 | 0x04000089 => {}

            // ---- Wave RAM (0x090-0x09F) ----
            0x04000090..=0x0400009F => {
                let offset = (addr - 0x04000090) as usize;
                // On GBA, wave RAM access goes to the bank NOT currently being played
                let bank_offset = if self.wave_bank_select == 0 { 0 } else { 16 };
                // Write to the opposite bank from the one being played
                let write_bank_offset = if bank_offset == 0 { 16 } else { 0 };
                let _ = write_bank_offset;
                // For simplicity, write directly (like GB behavior)
                if offset < 16 {
                    self.wave.write_wave_ram_byte(offset, val);
                }
            }

            // ---- FIFO A (0x0A0-0x0A3) ----
            0x040000A0..=0x040000A3 => {
                // FIFOs are written as words; accumulate bytes
                // In practice, games write 32-bit words via DMA
                // Individual byte writes: just push one byte as a partial word
                if self.fifo_a.count < FIFO_CAPACITY {
                    self.fifo_a.buffer[self.fifo_a.write_pos] = val as i8;
                    self.fifo_a.write_pos = (self.fifo_a.write_pos + 1) % FIFO_CAPACITY;
                    self.fifo_a.count += 1;
                }
            }

            // ---- FIFO B (0x0A4-0x0A7) ----
            0x040000A4..=0x040000A7 => {
                if self.fifo_b.count < FIFO_CAPACITY {
                    self.fifo_b.buffer[self.fifo_b.write_pos] = val as i8;
                    self.fifo_b.write_pos = (self.fifo_b.write_pos + 1) % FIFO_CAPACITY;
                    self.fifo_b.count += 1;
                }
            }

            _ => {}
        }
    }

    /// Write a 32-bit word to a FIFO address.
    ///
    /// Called by the DMA path for efficient multi-byte transfers.
    pub fn write_fifo_word(&mut self, addr: u32, val: u32) {
        match addr {
            0x040000A0 => self.fifo_a.push_word(val),
            0x040000A4 => self.fifo_b.push_word(val),
            _ => {}
        }
    }

    /// Read a GBA sound register.
    ///
    /// `addr` is the full I/O address (e.g. 0x04000060).
    pub fn read_register(&self, addr: u32) -> u8 {
        match addr {
            // SOUND1CNT_L
            0x04000060 => {
                let period = self.pulse1_sweep.period & 0x07;
                let negate = if self.pulse1_sweep.negate { 0x08 } else { 0x00 };
                let shift = self.pulse1_sweep.shift & 0x07;
                (period << 4) | negate | shift
            }
            0x04000061 => 0,

            // SOUND1CNT_H
            0x04000062 => (self.pulse1.duty << 6) | 0x3F,
            0x04000063 => {
                let volume = self.pulse1_envelope.initial_volume() & 0x0F;
                let add_mode = if self.pulse1_envelope.add_mode() {
                    0x08
                } else {
                    0x00
                };
                let period = self.pulse1_envelope.period() & 0x07;
                (volume << 4) | add_mode | period
            }

            // SOUND1CNT_X
            0x04000064 => 0, // Write-only frequency low
            0x04000065 => {
                let length_enable = if self.pulse1_length.is_enabled() {
                    0x40
                } else {
                    0x00
                };
                0x3F | length_enable // Only bit 6 readable
            }

            // SOUND2CNT_L
            0x04000068 => (self.pulse2.duty << 6) | 0x3F,
            0x04000069 => {
                let volume = self.pulse2_envelope.initial_volume() & 0x0F;
                let add_mode = if self.pulse2_envelope.add_mode() {
                    0x08
                } else {
                    0x00
                };
                let period = self.pulse2_envelope.period() & 0x07;
                (volume << 4) | add_mode | period
            }

            // SOUND2CNT_H
            0x0400006C => 0,
            0x0400006D => {
                let length_enable = if self.pulse2_length.is_enabled() {
                    0x40
                } else {
                    0x00
                };
                0x3F | length_enable
            }

            // SOUND3CNT_L
            0x04000070 => {
                let bank_mode = if self.wave_bank_mode { 0x20 } else { 0x00 };
                let bank_sel = self.wave_bank_select << 6;
                let dac = if self.wave_dac_enabled { 0x80 } else { 0x00 };
                bank_mode | bank_sel | dac
            }
            0x04000071 => 0,

            // SOUND3CNT_H
            0x04000072 => 0, // Length write-only
            0x04000073 => ((self.wave.volume_shift & 0x03) << 5) | 0x1F,

            // SOUND3CNT_X
            0x04000074 => 0,
            0x04000075 => {
                let length_enable = if self.wave_length.is_enabled() {
                    0x40
                } else {
                    0x00
                };
                0x3F | length_enable
            }

            // SOUND4CNT_L
            0x04000078 => 0,
            0x04000079 => {
                let volume = self.noise_envelope.initial_volume() & 0x0F;
                let add_mode = if self.noise_envelope.add_mode() {
                    0x08
                } else {
                    0x00
                };
                let period = self.noise_envelope.period() & 0x07;
                (volume << 4) | add_mode | period
            }

            // SOUND4CNT_H
            0x0400007C => {
                let shift = (self.noise.period_index >> 4) & 0x0F;
                let width = if self.noise.mode { 0x08 } else { 0x00 };
                let divisor = self.noise.period_index & 0x07;
                (shift << 4) | width | divisor
            }
            0x0400007D => {
                let length_enable = if self.noise_length.is_enabled() {
                    0x40
                } else {
                    0x00
                };
                0x3F | length_enable
            }

            // SOUNDCNT_L
            0x04000080 => (self.psg_left_volume << 4) | self.psg_right_volume,
            0x04000081 => self.psg_panning,

            // SOUNDCNT_H
            0x04000082 => {
                self.psg_volume
                    | if self.fifo_a_full_volume { 0x04 } else { 0 }
                    | if self.fifo_b_full_volume { 0x08 } else { 0 }
            }
            0x04000083 => {
                let mut val = 0u8;
                if self.fifo_a_right {
                    val |= 0x01;
                }
                if self.fifo_a_left {
                    val |= 0x02;
                }
                if self.fifo_a_timer {
                    val |= 0x04;
                }
                if self.fifo_b_right {
                    val |= 0x10;
                }
                if self.fifo_b_left {
                    val |= 0x20;
                }
                if self.fifo_b_timer {
                    val |= 0x40;
                }
                val
            }

            // SOUNDCNT_X
            0x04000084 => {
                let power = if self.power_on { 0x80 } else { 0x00 };
                let ch1 = if self.pulse1.enabled { 0x01 } else { 0x00 };
                let ch2 = if self.pulse2.enabled { 0x02 } else { 0x00 };
                let ch3 = if self.wave.enabled { 0x04 } else { 0x00 };
                let ch4 = if self.noise.enabled { 0x08 } else { 0x00 };
                power | ch1 | ch2 | ch3 | ch4 | 0x70
            }
            0x04000085 => 0,

            // SOUNDBIAS
            0x04000088 | 0x04000089 => 0, // Will read from I/O array

            // Wave RAM
            0x04000090..=0x0400009F => {
                let offset = (addr - 0x04000090) as usize;
                if offset < 16 {
                    self.wave.read_wave_ram_byte(offset)
                } else {
                    0
                }
            }

            // FIFOs are write-only
            0x040000A0..=0x040000A7 => 0,

            _ => 0,
        }
    }

    /// Turn off all sound (called when SOUNDCNT_X bit 7 is cleared).
    fn power_off(&mut self) {
        self.pulse1 = PulseChannel::new();
        self.pulse1_sweep = SweepUnit::new();
        self.pulse1_envelope = Envelope::new();
        self.pulse1_length = LengthCounter::new();
        self.pulse2 = PulseChannel::new();
        self.pulse2_envelope = Envelope::new();
        self.pulse2_length = LengthCounter::new();
        self.wave = WaveChannel::new();
        self.wave_length = LengthCounter::new();
        self.noise = NoiseChannel::new();
        self.noise_envelope = Envelope::new();
        self.noise_length = LengthCounter::new();
        self.frame_sequencer_cycles = 0;
        self.frame_sequencer_step = 0;
        self.psg_left_volume = 0;
        self.psg_right_volume = 0;
        self.psg_panning = 0;
        self.pulse1_frequency = 0;
        self.pulse2_frequency = 0;
        self.wave_frequency = 0;
        self.wave_dac_enabled = false;
        // Note: FIFOs and DMA sound settings are NOT cleared by power off
    }
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Calculate pulse channel timer reload from GBA frequency register value.
///
/// Period = (2048 - freq) * 16 CPU cycles → at CPU/4, that's (2048 - freq) * 4.
/// Timer reload = period / 2 (half-period for duty cycle stepping).
fn gba_square_timer(freq: u16) -> u16 {
    let period = (2048u32.saturating_sub(freq as u32)) * 4;
    let reload = period / 2;
    reload.saturating_sub(1) as u16
}

/// Calculate wave channel timer reload from GBA frequency register value.
///
/// Period = (2048 - freq) * 8 CPU cycles → at CPU/4, that's (2048 - freq) * 2.
fn gba_wave_timer(freq: u16) -> u16 {
    let period = (2048u32.saturating_sub(freq as u32)) * 2;
    period.saturating_sub(1) as u16
}

/// GBA noise channel - GB-compatible divisor/shift to period index mapping.
const GBA_NOISE_DIVISOR_TABLE: [u16; 8] = [8, 16, 32, 48, 64, 80, 96, 112];
const GBA_NOISE_PERIOD_TABLE: [u16; 16] = [
    4, 8, 16, 32, 64, 96, 128, 160, 202, 254, 380, 508, 762, 1016, 2034, 4068,
];

/// Map GBA noise register value to a period table index.
fn gba_noise_period_index(val: u8) -> u8 {
    let shift = (val >> 4) & 0x0F;
    let divisor_code = val & 0x07;
    let divisor = GBA_NOISE_DIVISOR_TABLE[divisor_code as usize] as u32;
    let target_cycles = divisor << shift;

    let mut best_idx = 0u8;
    let mut best_diff = u32::MAX;
    for (idx, &period) in GBA_NOISE_PERIOD_TABLE.iter().enumerate() {
        let diff = period.abs_diff(target_cycles as u16) as u32;
        if diff < best_diff {
            best_diff = diff;
            best_idx = idx as u8;
        }
    }
    best_idx
}

/// DC-blocking high-pass filter.
///
/// Removes DC offset while preserving the audio signal.
/// Transfer function: y[n] = x[n] - x[n-1] + α * y[n-1]
/// α = 0.995 gives a ~35 Hz cutoff at 44.1 kHz.
fn dc_block(input: f32, prev_in: &mut f32, prev_out: &mut f32) -> i16 {
    let y = input - *prev_in + 0.995 * *prev_out;
    *prev_in = input;
    *prev_out = y;

    // Hard clamp to i16 range (no soft clipping)
    y.clamp(i16::MIN as f32, i16::MAX as f32) as i16
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apu_creation() {
        let apu = GbaApu::new();
        assert!(apu.power_on);
        assert_eq!(apu.psg_left_volume, 7);
        assert_eq!(apu.psg_right_volume, 7);
        assert_eq!(apu.psg_panning, 0xFF);
    }

    #[test]
    fn test_apu_reset() {
        let mut apu = GbaApu::new();
        apu.pulse1.enabled = true;
        apu.fifo_a.push_word(0x12345678);
        apu.reset();
        assert!(!apu.pulse1.enabled);
        assert_eq!(apu.fifo_a.count, 0);
    }

    #[test]
    fn test_fifo_push_pop() {
        let mut fifo = SoundFifo::new();
        fifo.push_word(0x01020304);
        assert_eq!(fifo.count, 4);

        assert_eq!(fifo.pop(), 0x04);
        assert_eq!(fifo.pop(), 0x03);
        assert_eq!(fifo.pop(), 0x02);
        assert_eq!(fifo.pop(), 0x01);
        assert_eq!(fifo.count, 0);
    }

    #[test]
    fn test_fifo_holds_last_sample() {
        let mut fifo = SoundFifo::new();
        fifo.push_word(0x00000042);
        // Word splits into bytes: [0x42, 0x00, 0x00, 0x00]
        fifo.pop(); // 0x42
        assert_eq!(fifo.current_sample, 0x42);
        fifo.pop(); // 0x00
        fifo.pop(); // 0x00
        fifo.pop(); // 0x00 -- now empty
        assert_eq!(fifo.count, 0);
        // Pop from empty FIFO should hold last value (0x00)
        fifo.pop();
        assert_eq!(fifo.current_sample, 0x00);
    }

    #[test]
    fn test_fifo_reset() {
        let mut fifo = SoundFifo::new();
        fifo.push_word(0xDEADBEEF);
        fifo.reset();
        assert_eq!(fifo.count, 0);
        assert_eq!(fifo.current_sample, 0);
    }

    #[test]
    fn test_fifo_overflow() {
        let mut fifo = SoundFifo::new();
        // Fill to capacity (32 bytes = 8 words)
        for i in 0..8 {
            fifo.push_word(i);
        }
        assert_eq!(fifo.count, FIFO_CAPACITY);
        // Additional writes should be dropped
        fifo.push_word(0xFF);
        assert_eq!(fifo.count, FIFO_CAPACITY);
    }

    #[test]
    fn test_timer_overflow_fifo_a_timer0() {
        let mut apu = GbaApu::new();
        apu.fifo_a_timer = false; // Timer 0
        apu.fifo_a.push_word(0x01020304);
        let refill = apu.on_timer_overflow(0);
        assert_eq!(apu.fifo_a.count, 3);
        // FIFO has 3 bytes left, needs refill (<=16)
        assert_ne!(refill & 1, 0);
    }

    #[test]
    fn test_timer_overflow_fifo_b_timer1() {
        let mut apu = GbaApu::new();
        apu.fifo_b_timer = true; // Timer 1
        apu.fifo_b.push_word(0x05060708);
        let refill = apu.on_timer_overflow(1);
        assert_eq!(apu.fifo_b.count, 3);
        assert_ne!(refill & 2, 0);
    }

    #[test]
    fn test_master_enable() {
        let mut apu = GbaApu::new();
        assert!(apu.power_on);
        // Write 0 to SOUNDCNT_X to disable
        apu.write_register(0x04000084, 0x00);
        assert!(!apu.power_on);
        // Writes to other registers should be ignored
        apu.write_register(0x04000062, 0xFF);
        assert_eq!(apu.pulse1.duty, 0);
        // Re-enable
        apu.write_register(0x04000084, 0x80);
        assert!(apu.power_on);
    }

    #[test]
    fn test_soundcnt_h_write() {
        let mut apu = GbaApu::new();
        // SOUNDCNT_H low byte: PSG volume + FIFO volume
        apu.write_register(0x04000082, 0x0D); // psg=01 (50%), fifo_a=100%, fifo_b=100%
        assert_eq!(apu.psg_volume, 1);
        assert!(apu.fifo_a_full_volume);
        assert!(apu.fifo_b_full_volume);

        // SOUNDCNT_H high byte: FIFO panning/timer/reset
        apu.fifo_a.push_word(0x11223344);
        apu.write_register(0x04000083, 0x0B); // FIFO A: R+L+reset, timer 0
        assert!(apu.fifo_a_right);
        assert!(apu.fifo_a_left);
        assert!(!apu.fifo_a_timer);
        assert_eq!(apu.fifo_a.count, 0); // Reset clears FIFO
    }

    #[test]
    fn test_generate_silence_when_off() {
        let mut apu = GbaApu::new();
        apu.power_on = false;
        apu.tick(1000);
        let samples = apu.drain_samples(2000);
        // All samples should be 0 (silence)
        assert!(samples.iter().all(|&s| s == 0));
    }

    #[test]
    fn test_square_timer_calc() {
        // Frequency 0: period = (2048-0)*4 = 8192, reload = 4096-1 = 4095
        assert_eq!(gba_square_timer(0), 4095);
        // Frequency 2048: period = 0, reload = 0
        assert_eq!(gba_square_timer(2048), 0);
    }

    #[test]
    fn test_wave_timer_calc() {
        assert_eq!(gba_wave_timer(0), 4095);
        assert_eq!(gba_wave_timer(2048), 0);
    }
}
