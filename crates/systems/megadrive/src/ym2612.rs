//! Yamaha YM2612 FM Synthesizer
//!
//! 6 FM channels with 4 operators each, using 8 algorithms.
//! Channel 6 can optionally be used as a DAC for PCM playback.
//!
//! Mapped to $A04000-$A04003 on the 68K bus:
//!   $A04000 / $A04002: Address register (part 1 / part 2)
//!   $A04001 / $A04003: Data register (part 1 / part 2)

/// Sample rate for output
const SAMPLE_RATE: u32 = 44100;
/// FM clock divider: master clock / 7 / 6 / 24 ≈ 53267 Hz internal rate
const FM_RATE: u32 = 53267;

/// Sine table size
const SINE_TABLE_SIZE: usize = 1024;

/// Detune table: [detune_level (0-3)][key_code (0-31)] → frequency offset
/// Values from YM2612 die analysis. DT values 4-7 negate levels 0-3.
const DETUNE_TABLE: [[i32; 32]; 4] = [
    [0; 32],
    [
        0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 3, 3, 3, 4, 4, 4, 5, 5, 6, 6, 7, 8, 8,
        8, 8,
    ],
    [
        1, 1, 1, 1, 2, 2, 2, 2, 2, 3, 3, 3, 4, 4, 4, 5, 5, 6, 6, 7, 8, 8, 9, 10, 11, 12, 13, 14,
        16, 16, 16, 16,
    ],
    [
        2, 2, 2, 2, 2, 3, 3, 3, 4, 4, 4, 5, 5, 6, 6, 7, 8, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19,
        20, 22, 22, 22, 22,
    ],
];

/// Envelope increment table: [rate_group (0-15)][sub_counter (0-7)] → increment
const EG_INC_TABLE: [[u8; 8]; 16] = [
    [0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0],
    [0, 1, 0, 1, 0, 1, 0, 1],
    [0, 1, 0, 1, 0, 1, 0, 1],
    [0, 1, 0, 1, 0, 1, 0, 1],
    [0, 1, 0, 1, 0, 1, 0, 1],
    [0, 1, 1, 1, 0, 1, 1, 1],
    [0, 1, 1, 1, 0, 1, 1, 1],
    [0, 1, 1, 1, 0, 1, 1, 1],
    [0, 1, 1, 1, 0, 1, 1, 1],
    [1, 1, 1, 1, 1, 1, 1, 1],
    [1, 1, 1, 2, 1, 1, 1, 2],
    [1, 2, 1, 2, 1, 2, 1, 2],
    [1, 2, 2, 2, 1, 2, 2, 2],
    [2, 2, 2, 4, 2, 2, 2, 4],
    [2, 4, 4, 8, 2, 4, 4, 8],
];

/// Envelope counter shift — determines how many counter bits to skip per rate group
const EG_RATE_SHIFT: [u8; 16] = [12, 12, 10, 8, 6, 4, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0];

/// LFO phase increment (16.16 fixed point) per FM tick
/// Produces frequencies: 3.98, 5.56, 6.02, 6.37, 6.88, 9.63, 48.1, 72.2 Hz
const LFO_INC: [u32; 8] = [1254, 1752, 1896, 2007, 2167, 3034, 15150, 22741];

/// Operator (slot)
#[derive(Clone, Copy)]
struct Operator {
    /// Total level (attenuation, 0-127)
    total_level: u8,
    /// Key scale / attack rate
    key_scale: u8,
    attack_rate: u8,
    /// Decay rate
    decay_rate: u8,
    /// Sustain rate (secondary decay)
    sustain_rate: u8,
    /// Release rate
    release_rate: u8,
    /// Sustain level (0-15)
    sustain_level: u8,
    /// Detune
    detune: u8,
    /// Multiple
    multiple: u8,
    /// SSG-EG mode
    ssg_eg: u8,
    /// SSG-EG inversion flag
    ssg_eg_inv: bool,

    /// Phase accumulator
    phase: u32,
    /// Envelope state
    env_state: EnvState,
    /// Current envelope level (attenuation, 0-1023, 10-bit)
    env_level: u16,
    /// Key on
    key_on: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum EnvState {
    Attack,
    Decay,
    Sustain,
    Release,
    Off,
}

impl Default for Operator {
    fn default() -> Self {
        Self {
            total_level: 127,
            key_scale: 0,
            attack_rate: 0,
            decay_rate: 0,
            sustain_rate: 0,
            release_rate: 0,
            sustain_level: 0,
            detune: 0,
            multiple: 0,
            ssg_eg: 0,
            ssg_eg_inv: false,
            phase: 0,
            env_state: EnvState::Off,
            env_level: 1023,
            key_on: false,
        }
    }
}

/// FM Channel
#[derive(Clone)]
struct Channel {
    /// 4 operators
    ops: [Operator; 4],
    /// Frequency number (11-bit)
    fnum: u16,
    /// Block/octave (3-bit)
    block: u8,
    /// Algorithm (0-7)
    algorithm: u8,
    /// Feedback (0-7)
    feedback: u8,
    /// Left output enable
    left: bool,
    /// Right output enable
    right: bool,
    /// AMS (amplitude modulation sensitivity)
    ams: u8,
    /// FMS (frequency modulation sensitivity)
    fms: u8,
    /// Feedback output history
    fb_out: [i16; 2],
}

impl Default for Channel {
    fn default() -> Self {
        Self {
            ops: [Operator::default(); 4],
            fnum: 0,
            block: 0,
            algorithm: 0,
            feedback: 0,
            left: true,
            right: true,
            ams: 0,
            fms: 0,
            fb_out: [0; 2],
        }
    }
}

/// YM2612 FM Synthesizer
pub struct Ym2612 {
    /// 6 channels
    channels: [Channel; 6],
    /// Currently addressed register (part 1)
    address_1: u8,
    /// Currently addressed register (part 2)
    address_2: u8,
    /// DAC enable
    dac_enabled: bool,
    /// DAC data
    dac_data: u8,
    /// Global registers
    /// Timer A value
    timer_a: u16,
    /// Timer B value
    timer_b: u8,
    /// Timer control
    timer_control: u8,
    /// LFO enable
    lfo_enable: bool,
    /// LFO frequency
    lfo_freq: u8,
    /// Sample accumulator for resampling
    sample_counter: f64,
    /// Audio output buffer
    output_buffer: Vec<i16>,
    /// Sine lookup table
    sine_table: Vec<i16>,
    /// Envelope generator global counter
    eg_counter: u32,
    /// LFO phase accumulator (16.16 fixed point)
    lfo_counter: u32,
    /// Current LFO phase (0-255)
    lfo_phase: u8,
    /// Timer A counter
    timer_a_counter: u16,
    /// Timer B counter
    timer_b_counter: u8,
    /// Timer B prescaler (ticks every 16 FM ticks)
    timer_b_prescaler: u8,
    /// Timer A overflow flag
    timer_a_overflow: bool,
    /// Timer B overflow flag
    timer_b_overflow: bool,
    /// Timer A enable
    timer_a_enable: bool,
    /// Timer B enable
    timer_b_enable: bool,
    /// Timer A load
    timer_a_load: bool,
    /// Timer B load
    timer_b_load: bool,
}

impl Ym2612 {
    pub fn new() -> Self {
        // Build sine table
        let sine_table: Vec<i16> = (0..SINE_TABLE_SIZE)
            .map(|i| {
                let phase = (i as f64 / SINE_TABLE_SIZE as f64) * 2.0 * std::f64::consts::PI;
                (phase.sin() * 4095.0) as i16
            })
            .collect();

        Self {
            channels: std::array::from_fn(|_| Channel::default()),
            address_1: 0,
            address_2: 0,
            dac_enabled: false,
            dac_data: 0x80,
            timer_a: 0,
            timer_b: 0,
            timer_control: 0,
            lfo_enable: false,
            lfo_freq: 0,
            sample_counter: 0.0,
            output_buffer: Vec::new(),
            sine_table,
            eg_counter: 0,
            lfo_counter: 0,
            lfo_phase: 0,
            timer_a_counter: 0,
            timer_b_counter: 0,
            timer_b_prescaler: 0,
            timer_a_overflow: false,
            timer_b_overflow: false,
            timer_a_enable: false,
            timer_b_enable: false,
            timer_a_load: false,
            timer_b_load: false,
        }
    }

    pub fn reset(&mut self) {
        self.channels = std::array::from_fn(|_| Channel::default());
        self.address_1 = 0;
        self.address_2 = 0;
        self.dac_enabled = false;
        self.dac_data = 0x80;
        self.timer_a = 0;
        self.timer_b = 0;
        self.timer_control = 0;
        self.lfo_enable = false;
        self.lfo_freq = 0;
        self.sample_counter = 0.0;
        self.output_buffer.clear();
        self.eg_counter = 0;
        self.lfo_counter = 0;
        self.lfo_phase = 0;
        self.timer_a_counter = 0;
        self.timer_b_counter = 0;
        self.timer_b_prescaler = 0;
        self.timer_a_overflow = false;
        self.timer_b_overflow = false;
        self.timer_a_enable = false;
        self.timer_b_enable = false;
        self.timer_a_load = false;
        self.timer_b_load = false;
    }

    /// Write to address register (part 1: channels 1-3, part 2: channels 4-6)
    pub fn write_address(&mut self, part: u8, val: u8) {
        if part == 0 {
            self.address_1 = val;
        } else {
            self.address_2 = val;
        }
    }

    /// Write to data register
    pub fn write_data(&mut self, part: u8, val: u8) {
        let addr = if part == 0 {
            self.address_1
        } else {
            self.address_2
        };

        // Global registers (part 1 only, $20-$2F)
        if part == 0 && addr < 0x30 {
            match addr {
                0x22 => {
                    // LFO
                    self.lfo_enable = val & 0x08 != 0;
                    self.lfo_freq = val & 0x07;
                }
                0x24 => {
                    // Timer A MSB
                    self.timer_a = (self.timer_a & 0x03) | ((val as u16) << 2);
                }
                0x25 => {
                    // Timer A LSB
                    self.timer_a = (self.timer_a & 0x3FC) | (val as u16 & 0x03);
                }
                0x26 => {
                    // Timer B
                    self.timer_b = val;
                }
                0x27 => {
                    // Timer control / Channel 3 mode
                    self.timer_control = val;
                    self.timer_a_enable = val & 0x04 != 0;
                    self.timer_b_enable = val & 0x08 != 0;
                    self.timer_a_load = val & 0x01 != 0;
                    self.timer_b_load = val & 0x02 != 0;
                    if val & 0x10 != 0 {
                        self.timer_a_overflow = false;
                    }
                    if val & 0x20 != 0 {
                        self.timer_b_overflow = false;
                    }
                }
                0x28 => {
                    // Key on/off
                    let ch = match val & 0x07 {
                        0 => 0,
                        1 => 1,
                        2 => 2,
                        4 => 3,
                        5 => 4,
                        6 => 5,
                        _ => return,
                    };
                    for op in 0..4 {
                        let key = val & (0x10 << op) != 0;
                        if key && !self.channels[ch].ops[op].key_on {
                            // Key on
                            self.channels[ch].ops[op].key_on = true;
                            self.channels[ch].ops[op].env_state = EnvState::Attack;
                            self.channels[ch].ops[op].env_level = 1023;
                            self.channels[ch].ops[op].phase = 0;
                            self.channels[ch].ops[op].ssg_eg_inv = false;
                        } else if !key && self.channels[ch].ops[op].key_on {
                            // Key off
                            self.channels[ch].ops[op].key_on = false;
                            self.channels[ch].ops[op].env_state = EnvState::Release;
                        }
                    }
                }
                0x2A => {
                    // DAC data
                    self.dac_data = val;
                }
                0x2B => {
                    // DAC enable
                    self.dac_enabled = val & 0x80 != 0;
                }
                _ => {}
            }
            return;
        }

        // Per-channel registers
        let ch_offset = if part == 0 { 0 } else { 3 };
        let ch_idx = (addr & 0x03) as usize;
        if ch_idx == 3 {
            return; // Invalid
        }
        let ch = ch_idx + ch_offset;
        if ch >= 6 {
            return;
        }

        match addr & 0xF0 {
            0x30 => {
                // Detune / Multiple
                let op = ((addr >> 2) & 0x03) as usize;
                if op < 4 {
                    self.channels[ch].ops[op].detune = (val >> 4) & 0x07;
                    self.channels[ch].ops[op].multiple = val & 0x0F;
                }
            }
            0x40 => {
                // Total Level
                let op = ((addr >> 2) & 0x03) as usize;
                if op < 4 {
                    self.channels[ch].ops[op].total_level = val & 0x7F;
                }
            }
            0x50 => {
                // Key Scale / Attack Rate
                let op = ((addr >> 2) & 0x03) as usize;
                if op < 4 {
                    self.channels[ch].ops[op].key_scale = (val >> 6) & 0x03;
                    self.channels[ch].ops[op].attack_rate = val & 0x1F;
                }
            }
            0x60 => {
                // Decay Rate
                let op = ((addr >> 2) & 0x03) as usize;
                if op < 4 {
                    self.channels[ch].ops[op].decay_rate = val & 0x1F;
                }
            }
            0x70 => {
                // Sustain Rate
                let op = ((addr >> 2) & 0x03) as usize;
                if op < 4 {
                    self.channels[ch].ops[op].sustain_rate = val & 0x1F;
                }
            }
            0x80 => {
                // Sustain Level / Release Rate
                let op = ((addr >> 2) & 0x03) as usize;
                if op < 4 {
                    self.channels[ch].ops[op].sustain_level = (val >> 4) & 0x0F;
                    self.channels[ch].ops[op].release_rate = val & 0x0F;
                }
            }
            0x90 => {
                // SSG-EG
                let op = ((addr >> 2) & 0x03) as usize;
                if op < 4 {
                    self.channels[ch].ops[op].ssg_eg = val & 0x0F;
                }
            }
            0xA0 => {
                if (addr & 0x0C) == 0 {
                    // Frequency LSB
                    self.channels[ch].fnum = (self.channels[ch].fnum & 0x0700) | val as u16;
                } else if (addr & 0x0C) == 0x04 {
                    // Block + Frequency MSB
                    self.channels[ch].block = (val >> 3) & 0x07;
                    self.channels[ch].fnum =
                        (self.channels[ch].fnum & 0x00FF) | (((val & 0x07) as u16) << 8);
                }
            }
            0xB0 => {
                if (addr & 0x0C) == 0 {
                    // Algorithm + Feedback
                    self.channels[ch].algorithm = val & 0x07;
                    self.channels[ch].feedback = (val >> 3) & 0x07;
                } else if (addr & 0x0C) == 0x04 {
                    // LR / AMS / FMS
                    self.channels[ch].left = val & 0x80 != 0;
                    self.channels[ch].right = val & 0x40 != 0;
                    self.channels[ch].ams = (val >> 4) & 0x03;
                    self.channels[ch].fms = val & 0x07;
                }
            }
            _ => {}
        }
    }

    /// Read status register
    pub fn read_status(&self) -> u8 {
        let mut status = 0u8;
        if self.timer_a_overflow {
            status |= 0x01;
        }
        if self.timer_b_overflow {
            status |= 0x02;
        }
        status
    }

    /// Generate audio samples
    pub fn generate_samples(&mut self, count: usize) -> Vec<i16> {
        let mut samples = Vec::with_capacity(count * 2); // Stereo
        let step = FM_RATE as f64 / SAMPLE_RATE as f64;

        for _ in 0..count {
            self.sample_counter += step;
            while self.sample_counter >= 1.0 {
                self.sample_counter -= 1.0;
                self.clock_fm();
                self.clock_timers();
                self.clock_lfo();
            }

            let (left, right) = self.mix_output();
            samples.push(left);
            samples.push(right);
        }

        samples
    }

    /// Clock all FM channels once
    fn clock_fm(&mut self) {
        self.eg_counter = self.eg_counter.wrapping_add(1);

        for ch in 0..6 {
            let fnum = self.channels[ch].fnum as u32;
            let block = self.channels[ch].block;
            let key_code = ((block as u32) << 2) | ((fnum >> 9) & 3);

            // LFO FM modulation
            let lfo_fm = if self.lfo_enable && self.channels[ch].fms > 0 {
                let lfo_am = if self.lfo_phase < 128 {
                    self.lfo_phase as i32
                } else {
                    255 - self.lfo_phase as i32
                };
                let lfo_signed = lfo_am - 64;
                let fms = self.channels[ch].fms;
                let depth = match fms {
                    1 => fnum as i32 >> 5,
                    2 => fnum as i32 >> 4,
                    3 => fnum as i32 >> 3,
                    4 => fnum as i32 >> 2,
                    5 => fnum as i32 >> 1,
                    6 => fnum as i32,
                    7 => (fnum as i32) << 1,
                    _ => 0,
                };
                (depth * lfo_signed) >> 6
            } else {
                0
            };

            // Update phase for each operator
            for op in 0..4 {
                let mult = self.channels[ch].ops[op].multiple as u32;
                let mult = if mult == 0 { 1 } else { mult * 2 };

                // Apply detune
                let dt = self.channels[ch].ops[op].detune;
                let dt_fine = (dt & 3) as usize;
                let kc = key_code.min(31) as usize;
                let dt_val = DETUNE_TABLE[dt_fine][kc];
                let dt_sign = if dt & 4 != 0 { -1i32 } else { 1 };

                let base_freq = (fnum as i32 + lfo_fm) << block;
                let freq = ((base_freq * mult as i32) + dt_val * dt_sign) as u32;

                self.channels[ch].ops[op].phase =
                    self.channels[ch].ops[op].phase.wrapping_add(freq);
            }

            // Update envelopes
            let kc = key_code as u8;
            for op in 0..4 {
                self.update_envelope(ch, op, kc);
            }
        }
    }

    /// Update envelope for an operator with key scaling
    fn update_envelope(&mut self, ch: usize, op: usize, key_code: u8) {
        let eg_counter = self.eg_counter;
        let operator = &mut self.channels[ch].ops[op];

        // Calculate key scale rate
        let ks = operator.key_scale;
        let ksr = key_code >> (3 - ks);

        match operator.env_state {
            EnvState::Attack => {
                if operator.attack_rate == 0 {
                    return;
                }
                let eff_rate = ((operator.attack_rate as u16) * 2 + ksr as u16).min(63) as u8;

                if eff_rate >= 62 {
                    operator.env_level = 0;
                    operator.env_state = EnvState::Decay;
                    return;
                }

                let inc = eg_increment(eg_counter, eff_rate);
                if inc > 0 {
                    // Exponential attack: faster as level approaches 0
                    let delta = ((!operator.env_level as u32 & 0x3FF) * inc as u32) >> 4;
                    operator.env_level = operator.env_level.saturating_sub(delta.max(1) as u16);
                }

                if operator.env_level == 0 {
                    operator.env_state = EnvState::Decay;
                }
            }
            EnvState::Decay => {
                let sl = if operator.sustain_level == 15 {
                    1023
                } else {
                    (operator.sustain_level as u16) << 5
                };

                if operator.decay_rate == 0 {
                    if operator.env_level >= sl {
                        operator.env_state = EnvState::Sustain;
                    }
                    return;
                }

                let eff_rate = ((operator.decay_rate as u16) * 2 + ksr as u16).min(63) as u8;
                let inc = eg_increment(eg_counter, eff_rate);

                if inc > 0 {
                    if operator.ssg_eg & 0x08 != 0 && operator.ssg_eg_inv {
                        operator.env_level = operator.env_level.saturating_sub(inc);
                    } else {
                        operator.env_level = (operator.env_level + inc).min(1023);
                    }
                }

                if operator.env_level >= sl {
                    operator.env_state = EnvState::Sustain;
                }
            }
            EnvState::Sustain => {
                if operator.sustain_rate == 0 {
                    return;
                }

                let eff_rate = ((operator.sustain_rate as u16) * 2 + ksr as u16).min(63) as u8;
                let inc = eg_increment(eg_counter, eff_rate);

                if inc > 0 {
                    operator.env_level = (operator.env_level + inc).min(1023);
                }

                // SSG-EG: when reaching max attenuation, possibly loop
                if operator.ssg_eg & 0x08 != 0 && operator.env_level >= 1023 {
                    if operator.ssg_eg & 0x01 != 0 {
                        // Hold
                        operator.env_level = if operator.ssg_eg & 0x02 != 0 { 0 } else { 1023 };
                    } else {
                        // Repeat
                        operator.env_level = 0;
                        operator.env_state = EnvState::Attack;
                        if operator.ssg_eg & 0x02 != 0 {
                            operator.ssg_eg_inv = !operator.ssg_eg_inv;
                        }
                    }
                }
            }
            EnvState::Release => {
                // Release rate: 4-bit, scaled by 4 then +2
                let rr = operator.release_rate as u16;
                let eff_rate = (rr * 4 + 2 + ksr as u16).min(63) as u8;
                let inc = eg_increment(eg_counter, eff_rate);

                if inc > 0 {
                    operator.env_level = (operator.env_level + inc).min(1023);
                }

                if operator.env_level >= 1023 {
                    operator.env_level = 1023;
                    operator.env_state = EnvState::Off;
                }
            }
            EnvState::Off => {
                operator.env_level = 1023;
            }
        }
    }

    /// Calculate operator output with LFO AM modulation
    fn calc_operator(&self, ch: usize, op: usize, modulation: i16) -> i16 {
        let operator = &self.channels[ch].ops[op];

        if operator.env_state == EnvState::Off {
            return 0;
        }

        // Phase to sine table index (10-bit phase → table index)
        let phase = (operator.phase >> 10) as usize;
        let mod_phase = (phase as i32 + modulation as i32) as usize;
        let sine_idx = mod_phase % SINE_TABLE_SIZE;
        let sine_val = self.sine_table[sine_idx] as i32;

        // Apply envelope (attenuation)
        let tl = (operator.total_level as i32) << 3; // Scale TL to 10-bit range
        let mut env = operator.env_level as i32;

        // SSG-EG inversion
        if operator.ssg_eg & 0x08 != 0 && operator.ssg_eg_inv {
            env = 1023 - env;
        }

        // LFO AM modulation
        if self.lfo_enable {
            let ams = self.channels[ch].ams;
            if ams > 0 {
                let lfo_am = if self.lfo_phase < 128 {
                    self.lfo_phase as i32
                } else {
                    255 - self.lfo_phase as i32
                };
                let am_depth = match ams {
                    1 => lfo_am >> 4,
                    2 => lfo_am >> 1,
                    3 => lfo_am,
                    _ => 0,
                };
                env = (env + am_depth).min(1023);
            }
        }

        let total_atten = (tl + env).min(1023);

        // Attenuation to linear scale (simplified)
        let scale = 1023 - total_atten;
        let output = (sine_val * scale) / 1023;

        output as i16
    }

    /// Mix all channels to stereo output
    fn mix_output(&mut self) -> (i16, i16) {
        let mut left: i32 = 0;
        let mut right: i32 = 0;

        for ch_idx in 0..6 {
            // DAC channel
            if ch_idx == 5 && self.dac_enabled {
                let dac_sample = ((self.dac_data as i16) - 128) << 6;
                if self.channels[ch_idx].left {
                    left += dac_sample as i32;
                }
                if self.channels[ch_idx].right {
                    right += dac_sample as i32;
                }
                continue;
            }

            let output = self.calc_channel(ch_idx);

            if self.channels[ch_idx].left {
                left += output as i32;
            }
            if self.channels[ch_idx].right {
                right += output as i32;
            }
        }

        // Clamp to i16 range
        let left = left.clamp(-32768, 32767) as i16;
        let right = right.clamp(-32768, 32767) as i16;
        (left, right)
    }

    /// Calculate channel output using algorithm
    fn calc_channel(&mut self, ch: usize) -> i16 {
        let algorithm = self.channels[ch].algorithm;
        let feedback = self.channels[ch].feedback;

        // Calculate feedback modulation for operator 1
        let fb_mod = if feedback > 0 {
            let fb = &self.channels[ch].fb_out;
            ((fb[0] as i32 + fb[1] as i32) >> (9 - feedback as i32)) as i16
        } else {
            0
        };

        // Calculate all operators
        let op1 = self.calc_operator(ch, 0, fb_mod);
        let op2: i16;
        let op3: i16;
        let op4: i16;
        let output: i16;

        match algorithm {
            0 => {
                // 1→2→3→4
                op2 = self.calc_operator(ch, 1, op1);
                op3 = self.calc_operator(ch, 2, op2);
                op4 = self.calc_operator(ch, 3, op3);
                output = op4;
            }
            1 => {
                // (1+2)→3→4
                op2 = self.calc_operator(ch, 1, 0);
                let sum = ((op1 as i32 + op2 as i32) / 2) as i16;
                op3 = self.calc_operator(ch, 2, sum);
                op4 = self.calc_operator(ch, 3, op3);
                output = op4;
            }
            2 => {
                // (1+(2→3))→4
                op2 = self.calc_operator(ch, 1, 0);
                op3 = self.calc_operator(ch, 2, op2);
                let sum = ((op1 as i32 + op3 as i32) / 2) as i16;
                op4 = self.calc_operator(ch, 3, sum);
                output = op4;
            }
            3 => {
                // ((1→2)+3)→4
                op2 = self.calc_operator(ch, 1, op1);
                op3 = self.calc_operator(ch, 2, 0);
                let sum = ((op2 as i32 + op3 as i32) / 2) as i16;
                op4 = self.calc_operator(ch, 3, sum);
                output = op4;
            }
            4 => {
                // (1→2)+(3→4)
                op2 = self.calc_operator(ch, 1, op1);
                op3 = self.calc_operator(ch, 2, 0);
                op4 = self.calc_operator(ch, 3, op3);
                output = ((op2 as i32 + op4 as i32) / 2) as i16;
            }
            5 => {
                // 1→(2+3+4)
                op2 = self.calc_operator(ch, 1, op1);
                op3 = self.calc_operator(ch, 2, op1);
                op4 = self.calc_operator(ch, 3, op1);
                output = ((op2 as i32 + op3 as i32 + op4 as i32) / 3) as i16;
            }
            6 => {
                // (1→2)+3+4
                op2 = self.calc_operator(ch, 1, op1);
                op3 = self.calc_operator(ch, 2, 0);
                op4 = self.calc_operator(ch, 3, 0);
                output = ((op2 as i32 + op3 as i32 + op4 as i32) / 3) as i16;
            }
            7 => {
                // 1+2+3+4
                op2 = self.calc_operator(ch, 1, 0);
                op3 = self.calc_operator(ch, 2, 0);
                op4 = self.calc_operator(ch, 3, 0);
                output = ((op1 as i32 + op2 as i32 + op3 as i32 + op4 as i32) / 4) as i16;
            }
            _ => {
                output = 0;
            }
        }

        // Update feedback history
        self.channels[ch].fb_out[1] = self.channels[ch].fb_out[0];
        self.channels[ch].fb_out[0] = op1;

        output
    }

    /// Clock the LFO (Low Frequency Oscillator)
    fn clock_lfo(&mut self) {
        if !self.lfo_enable {
            return;
        }
        self.lfo_counter = self
            .lfo_counter
            .wrapping_add(LFO_INC[self.lfo_freq as usize]);
        self.lfo_phase = (self.lfo_counter >> 16) as u8;
    }

    /// Clock Timer A and Timer B
    fn clock_timers(&mut self) {
        // Timer A: increments every FM tick
        if self.timer_a_load {
            self.timer_a_counter = self.timer_a_counter.wrapping_add(1);
            if self.timer_a_counter >= 1024 {
                self.timer_a_counter = self.timer_a;
                if self.timer_a_enable {
                    self.timer_a_overflow = true;
                }
            }
        }

        // Timer B: increments every 16 FM ticks
        self.timer_b_prescaler = self.timer_b_prescaler.wrapping_add(1);
        if self.timer_b_prescaler >= 16 {
            self.timer_b_prescaler = 0;
            if self.timer_b_load {
                let next = self.timer_b_counter as u16 + 1;
                if next >= 256 {
                    self.timer_b_counter = self.timer_b;
                    if self.timer_b_enable {
                        self.timer_b_overflow = true;
                    }
                } else {
                    self.timer_b_counter = next as u8;
                }
            }
        }
    }

    // ── Save State ──────────────────────────────────────────────

    pub fn get_state(&self) -> serde_json::Value {
        serde_json::json!({
            "dac_enabled": self.dac_enabled,
            "dac_data": self.dac_data,
            "timer_a": self.timer_a,
            "timer_b": self.timer_b,
        })
    }

    pub fn set_state(&mut self, _state: &serde_json::Value) -> Result<(), serde_json::Error> {
        // TODO: Full YM2612 state restore
        Ok(())
    }
}

/// Get envelope increment based on effective rate and global counter
fn eg_increment(eg_counter: u32, rate: u8) -> u16 {
    if rate < 2 {
        return 0;
    }
    let rate = rate.min(63);
    let rate_group = (rate >> 2) as usize;
    let shift = EG_RATE_SHIFT[rate_group] as u32;

    // Check if this tick should update
    if shift > 0 && (eg_counter & ((1 << shift) - 1)) != 0 {
        return 0;
    }

    // Select sub-counter position
    let sub = if shift > 0 {
        ((eg_counter >> shift) & 7) as usize
    } else {
        (eg_counter & 7) as usize
    };

    EG_INC_TABLE[rate_group][sub] as u16
}
