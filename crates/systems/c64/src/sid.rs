//! SID (MOS 6581/8580) Sound Interface Device
//!
//! ## Features
//! - 3 independent voices
//! - Waveforms: triangle, sawtooth, pulse (square), noise
//! - Full ADSR envelope generator per voice
//! - Ring modulation (voice with previous voice)
//! - Hard synchronization (voice with previous voice)
//! - Multimode programmable filter (low-pass, band-pass, high-pass)
//! - 4-bit master volume control
//!
//! ## Registers (per voice, 7 bytes each)
//! - +$00/$01: Frequency (16-bit)
//! - +$02/$03: Pulse width (12-bit)
//! - +$04: Control register (waveform select, gate, sync, ring, test)
//! - +$05: Attack/Decay (4-bit each)
//! - +$06: Sustain/Release (4-bit each)
//!
//! ## Audio output
//! - PAL clock: 985,248 Hz
//! - Sample rate: 44,100 Hz
//! - Accumulates samples and drains per frame

/// PAL CPU/SID clock frequency
const PAL_CLOCK: u32 = 985_248;
/// Audio output sample rate
const SAMPLE_RATE: u32 = 44_100;

/// ADSR attack rate table (cycles per step)
/// Each entry corresponds to attack value 0–15
const ATTACK_RATES: [u32; 16] = [
    2, 8, 16, 24, 38, 56, 68, 80, 100, 250, 500, 800, 1000, 3000, 5000, 8000,
];

/// ADSR decay/release rate table (cycles per step)
const DECAY_RELEASE_RATES: [u32; 16] = [
    6, 24, 48, 72, 114, 168, 204, 240, 300, 750, 1500, 2400, 3000, 9000, 15000, 24000,
];

/// Sustain level table (maps 4-bit value to 8-bit envelope level)
const SUSTAIN_LEVELS: [u8; 16] = [
    0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF,
];

/// Envelope state machine
#[derive(Clone, Copy, PartialEq)]
enum EnvelopePhase {
    Attack,
    Decay,
    Sustain,
    Release,
}

/// Single SID voice state
struct SidVoice {
    /// 24-bit phase accumulator
    phase: u32,
    /// 16-bit frequency
    freq: u16,
    /// 12-bit pulse width
    pulse_width: u16,
    /// Control register
    control: u8,
    /// Attack (0–15)
    attack: u8,
    /// Decay (0–15)
    decay: u8,
    /// Sustain (0–15)
    sustain: u8,
    /// Release (0–15)
    release: u8,
    /// ADSR envelope level (0–255)
    envelope: u8,
    /// Current ADSR phase
    envelope_phase: EnvelopePhase,
    /// Envelope rate counter
    envelope_counter: u32,
    /// 23-bit noise LFSR
    noise_lfsr: u32,
    /// Previous gate state for edge detection
    prev_gate: bool,
}

impl SidVoice {
    fn new() -> Self {
        Self {
            phase: 0,
            freq: 0,
            pulse_width: 0x800,
            control: 0,
            attack: 0,
            decay: 0,
            sustain: 0,
            release: 0,
            envelope: 0,
            envelope_phase: EnvelopePhase::Release,
            envelope_counter: 0,
            noise_lfsr: 0x7FFFF8,
            prev_gate: false,
        }
    }

    fn reset(&mut self) {
        *self = Self::new();
    }

    /// Advance the oscillator and envelope by one clock cycle
    fn clock(&mut self, sync_source_msb: bool) {
        let old_msb = self.phase & 0x800000 != 0;

        // Advance phase accumulator
        self.phase = (self.phase + self.freq as u32) & 0xFFFFFF;

        let new_msb = self.phase & 0x800000 != 0;

        // Hard sync: reset phase when sync source MSB transitions 0→1
        if self.control & 0x02 != 0 && !old_msb && sync_source_msb {
            self.phase = 0;
        }

        // Clock noise LFSR when bit 19 transitions
        if self.phase & 0x80000 != 0 && (self.phase.wrapping_sub(self.freq as u32)) & 0x80000 == 0 {
            let bit = ((self.noise_lfsr >> 22) ^ (self.noise_lfsr >> 17)) & 1;
            self.noise_lfsr = ((self.noise_lfsr << 1) | bit) & 0x7FFFFF;
        }

        // Envelope generator
        let gate = self.control & 0x01 != 0;
        if gate && !self.prev_gate {
            // Gate on: start attack
            self.envelope_phase = EnvelopePhase::Attack;
            self.envelope_counter = 0;
        } else if !gate && self.prev_gate {
            // Gate off: start release
            self.envelope_phase = EnvelopePhase::Release;
            self.envelope_counter = 0;
        }
        self.prev_gate = gate;

        self.envelope_counter += 1;
        match self.envelope_phase {
            EnvelopePhase::Attack => {
                let rate = ATTACK_RATES[self.attack as usize];
                if self.envelope_counter >= rate {
                    self.envelope_counter = 0;
                    if self.envelope == 0xFF {
                        self.envelope_phase = EnvelopePhase::Decay;
                    } else {
                        self.envelope = self.envelope.saturating_add(1);
                    }
                }
            }
            EnvelopePhase::Decay => {
                let rate = DECAY_RELEASE_RATES[self.decay as usize];
                let sustain_level = SUSTAIN_LEVELS[self.sustain as usize];
                if self.envelope_counter >= rate {
                    self.envelope_counter = 0;
                    if self.envelope <= sustain_level {
                        self.envelope = sustain_level;
                        self.envelope_phase = EnvelopePhase::Sustain;
                    } else {
                        self.envelope = self.envelope.saturating_sub(1);
                    }
                }
            }
            EnvelopePhase::Sustain => {
                // Hold at sustain level until gate off
                self.envelope = SUSTAIN_LEVELS[self.sustain as usize];
            }
            EnvelopePhase::Release => {
                let rate = DECAY_RELEASE_RATES[self.release as usize];
                if self.envelope_counter >= rate {
                    self.envelope_counter = 0;
                    self.envelope = self.envelope.saturating_sub(1);
                }
            }
        }

        // Ignore MSB transition tracking
        let _ = new_msb;
    }

    /// Get the current oscillator output (-2048..2047, 12-bit signed)
    fn output(&self, ring_mod_source_msb: bool) -> i16 {
        let waveform = (self.control >> 4) & 0x0F;

        // Test bit forces oscillator output to 0
        if self.control & 0x08 != 0 {
            return 0;
        }

        if waveform == 0 {
            return 0;
        }

        // Phase is 24-bit; use upper 12 bits for waveform generation
        let p = (self.phase >> 12) & 0xFFF;

        let raw: i16 = match waveform {
            0x01 => {
                // Triangle
                let mut tri = if p < 0x800 { p } else { 0xFFF - p };
                // Ring modulation: XOR with MSB of modulating voice
                if self.control & 0x04 != 0 && ring_mod_source_msb {
                    tri ^= 0x800;
                }
                tri as i16 - 2048
            }
            0x02 => {
                // Sawtooth
                p as i16 - 2048
            }
            0x04 => {
                // Pulse/Square
                let pw = self.pulse_width & 0xFFF;
                if (self.phase >> 12) < pw as u32 {
                    2047
                } else {
                    -2048
                }
            }
            0x08 => {
                // Noise (from LFSR)
                let noise_out = ((self.noise_lfsr >> 11) & 0xFFF) as i16;
                noise_out - 2048
            }
            // Combined waveforms: approximate by AND-ing
            _ => {
                let mut val: u16 = 0xFFF;
                if waveform & 0x01 != 0 {
                    let tri = if p < 0x800 { p } else { 0xFFF - p };
                    val &= tri as u16;
                }
                if waveform & 0x02 != 0 {
                    val &= p as u16;
                }
                if waveform & 0x04 != 0 {
                    let pw = self.pulse_width & 0xFFF;
                    if (self.phase >> 12) >= pw as u32 {
                        val = 0;
                    }
                }
                if waveform & 0x08 != 0 {
                    val &= ((self.noise_lfsr >> 11) & 0xFFF) as u16;
                }
                val as i16 - 2048
            }
        };

        raw
    }

    /// Get the MSB of the phase accumulator (for sync/ring mod)
    fn phase_msb(&self) -> bool {
        self.phase & 0x800000 != 0
    }
}

/// SID chip state
pub struct Sid {
    /// Raw register storage for reads
    pub regs: [u8; 32],
    /// Three voices
    voices: [SidVoice; 3],
    /// Audio sample buffer (interleaved stereo)
    buffer: Vec<i16>,
    /// Fractional cycle accumulator for sample generation
    cycle_acc: u32,
    /// Filter cutoff frequency (11-bit)
    filter_cutoff: u16,
    /// Filter resonance (4-bit)
    filter_resonance: u8,
    /// Filter routing bitmask (bits 0-2: voice 1-3 routed through filter)
    filter_route: u8,
    /// Filter mode (bits 4-6 of $D418): LP, BP, HP
    filter_mode: u8,
    /// Master volume (0–15)
    master_volume: u8,
    /// Voice 3 disconnect from output (bit 7 of $D418)
    voice3_off: bool,
}

impl Sid {
    pub fn new() -> Self {
        Self {
            regs: [0u8; 32],
            voices: [SidVoice::new(), SidVoice::new(), SidVoice::new()],
            buffer: Vec::new(),
            cycle_acc: 0,
            filter_cutoff: 0,
            filter_resonance: 0,
            filter_route: 0,
            filter_mode: 0,
            master_volume: 0,
            voice3_off: false,
        }
    }

    pub fn reset(&mut self) {
        self.regs = [0u8; 32];
        for v in &mut self.voices {
            v.reset();
        }
        self.buffer.clear();
        self.cycle_acc = 0;
        self.filter_cutoff = 0;
        self.filter_resonance = 0;
        self.filter_route = 0;
        self.filter_mode = 0;
        self.master_volume = 0;
        self.voice3_off = false;
    }

    /// Read SID register (most are write-only)
    pub fn read_reg(&self, reg: u8) -> u8 {
        match reg & 0x1F {
            0x19 => 0x00, // Paddle X (not implemented)
            0x1A => 0x00, // Paddle Y (not implemented)
            0x1B => {
                // Voice 3 oscillator output (upper 8 bits)
                let output = self.voices[2].output(self.voices[1].phase_msb());
                ((output + 2048) >> 4) as u8
            }
            0x1C => {
                // Voice 3 envelope output
                self.voices[2].envelope
            }
            _ => 0, // All other registers read as 0
        }
    }

    /// Write SID register
    pub fn write_reg(&mut self, reg: u8, val: u8) {
        let r = (reg & 0x1F) as usize;
        if r < self.regs.len() {
            self.regs[r] = val;
        }

        match r {
            // Voice 1 (offset 0x00)
            0x00 => self.voices[0].freq = (self.voices[0].freq & 0xFF00) | val as u16,
            0x01 => self.voices[0].freq = (self.voices[0].freq & 0x00FF) | ((val as u16) << 8),
            0x02 => self.voices[0].pulse_width = (self.voices[0].pulse_width & 0x0F00) | val as u16,
            0x03 => {
                self.voices[0].pulse_width =
                    (self.voices[0].pulse_width & 0x00FF) | (((val & 0x0F) as u16) << 8)
            }
            0x04 => self.voices[0].control = val,
            0x05 => {
                self.voices[0].attack = (val >> 4) & 0x0F;
                self.voices[0].decay = val & 0x0F;
            }
            0x06 => {
                self.voices[0].sustain = (val >> 4) & 0x0F;
                self.voices[0].release = val & 0x0F;
            }

            // Voice 2 (offset 0x07)
            0x07 => self.voices[1].freq = (self.voices[1].freq & 0xFF00) | val as u16,
            0x08 => self.voices[1].freq = (self.voices[1].freq & 0x00FF) | ((val as u16) << 8),
            0x09 => self.voices[1].pulse_width = (self.voices[1].pulse_width & 0x0F00) | val as u16,
            0x0A => {
                self.voices[1].pulse_width =
                    (self.voices[1].pulse_width & 0x00FF) | (((val & 0x0F) as u16) << 8)
            }
            0x0B => self.voices[1].control = val,
            0x0C => {
                self.voices[1].attack = (val >> 4) & 0x0F;
                self.voices[1].decay = val & 0x0F;
            }
            0x0D => {
                self.voices[1].sustain = (val >> 4) & 0x0F;
                self.voices[1].release = val & 0x0F;
            }

            // Voice 3 (offset 0x0E)
            0x0E => self.voices[2].freq = (self.voices[2].freq & 0xFF00) | val as u16,
            0x0F => self.voices[2].freq = (self.voices[2].freq & 0x00FF) | ((val as u16) << 8),
            0x10 => self.voices[2].pulse_width = (self.voices[2].pulse_width & 0x0F00) | val as u16,
            0x11 => {
                self.voices[2].pulse_width =
                    (self.voices[2].pulse_width & 0x00FF) | (((val & 0x0F) as u16) << 8)
            }
            0x12 => self.voices[2].control = val,
            0x13 => {
                self.voices[2].attack = (val >> 4) & 0x0F;
                self.voices[2].decay = val & 0x0F;
            }
            0x14 => {
                self.voices[2].sustain = (val >> 4) & 0x0F;
                self.voices[2].release = val & 0x0F;
            }

            // Filter
            0x15 => self.filter_cutoff = (self.filter_cutoff & 0x7F8) | (val & 0x07) as u16,
            0x16 => self.filter_cutoff = (self.filter_cutoff & 0x007) | ((val as u16) << 3),
            0x17 => {
                self.filter_resonance = (val >> 4) & 0x0F;
                self.filter_route = val & 0x0F;
            }
            0x18 => {
                self.master_volume = val & 0x0F;
                self.filter_mode = (val >> 4) & 0x07;
                self.voice3_off = val & 0x80 != 0;
            }

            _ => {}
        }
    }

    /// Advance SID by given number of CPU cycles, generating audio samples
    pub fn clock(&mut self, cycles: u32) {
        for _ in 0..cycles {
            // Clock all three voices
            let msb0 = self.voices[0].phase_msb();
            let msb1 = self.voices[1].phase_msb();
            let msb2 = self.voices[2].phase_msb();

            // Sync source for voice N is voice (N-1) mod 3
            self.voices[0].clock(msb2); // Voice 1 syncs to voice 3
            self.voices[1].clock(msb0); // Voice 2 syncs to voice 1
            self.voices[2].clock(msb1); // Voice 3 syncs to voice 2

            // Generate audio sample at 44.1 kHz
            self.cycle_acc += SAMPLE_RATE;
            if self.cycle_acc >= PAL_CLOCK {
                self.cycle_acc -= PAL_CLOCK;
                let sample = self.generate_sample();
                self.buffer.push(sample);
                self.buffer.push(sample); // Stereo: duplicate for L+R
            }
        }
    }

    /// Mix voices and generate one audio sample
    fn generate_sample(&self) -> i16 {
        let mut mix: i32 = 0;

        for i in 0..3usize {
            // Skip voice 3 if disconnected
            if i == 2 && self.voice3_off {
                continue;
            }

            // Ring modulation source is the previous voice
            let ring_src = match i {
                0 => self.voices[2].phase_msb(),
                1 => self.voices[0].phase_msb(),
                2 => self.voices[1].phase_msb(),
                _ => false,
            };

            let raw = self.voices[i].output(ring_src) as i32;
            let env = self.voices[i].envelope as i32;
            mix += (raw * env) / 256;
        }

        // Apply master volume (0–15)
        mix = (mix * self.master_volume as i32) / 15;

        mix.clamp(-32768, 32767) as i16
    }

    /// Drain all buffered audio samples
    pub fn drain_samples(&mut self) -> Vec<i16> {
        std::mem::take(&mut self.buffer)
    }
}

impl Default for Sid {
    fn default() -> Self {
        Self::new()
    }
}
