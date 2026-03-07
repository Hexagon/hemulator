//! PlayStation 1 SPU — Sound Processing Unit
//!
//! Hardware features:
//! - 24 ADPCM voices
//! - 512 KB sound RAM
//! - Reverb processing
//! - CD audio mixing
//! - Noise generator
//! - Pitch modulation
//!
//! ## References
//! - nocash PSX-SPX SPU documentation: https://problemkaputt.de/psx-spx.htm
//! - Martin Korth's PSX SPU register map
//! - Avocado PS1 emulator (reference implementation)

/// SPU RAM size: 512 KB
const SPU_RAM_SIZE: usize = 512 * 1024;

/// Number of voices
const NUM_VOICES: usize = 24;

/// PS1 SPU sample rate (44100 Hz)
pub const SPU_SAMPLE_RATE: u32 = 44100;

/// XA-ADPCM positive filter coefficients (Q10 fixed-point × 64)
const POS_XA_ADPCM_TABLE: [i32; 5] = [0, 60, 115, 98, 122];

/// XA-ADPCM negative filter coefficients (Q10 fixed-point × 64)
const NEG_XA_ADPCM_TABLE: [i32; 5] = [0, 0, -52, -55, -60];

// ============================================================================
// ADSR envelope
// ============================================================================

/// ADSR envelope phase
#[derive(Debug, Clone, Copy, PartialEq, Default)]
enum AdsrPhase {
    #[default]
    Off,
    Attack,
    Decay,
    Sustain,
    Release,
}

/// ADSR envelope state for a voice
#[derive(Debug, Clone, Default)]
struct AdsrEnvelope {
    phase: AdsrPhase,
    /// Current volume (0..0x7FFF)
    volume: i32,
    /// Internal counter for rate control
    counter: u32,
}

/// Decode an ADSR rate (0..127) into (step, shift) where the volume is
/// updated by `step` every `1 << shift` samples.
fn adsr_rate_to_step_shift(rate: u8) -> (i32, u32) {
    // step = [7, 6, 5, 4][rate & 3]
    // shift = max(0, 11 - rate/4)
    let step = [7i32, 6, 5, 4][(rate & 3) as usize];
    let shift = if rate < 44 {
        11u32.saturating_sub((rate >> 2) as u32)
    } else {
        0
    };
    (step, shift)
}

impl AdsrEnvelope {
    /// Key-on: start the attack phase from silence.
    fn key_on(&mut self) {
        self.phase = AdsrPhase::Attack;
        self.volume = 0;
        self.counter = 0;
    }

    /// Key-off: begin the release phase.
    fn key_off(&mut self) {
        self.phase = AdsrPhase::Release;
        self.counter = 0;
    }

    /// Tick once per sample.  Returns the current volume (0..0x7FFF).
    /// `adsr` is the raw 32-bit ADSR register value.
    fn tick(&mut self, adsr: u32) -> i16 {
        if self.phase == AdsrPhase::Off {
            return 0;
        }

        let adsr_lo = adsr as u16;
        let adsr_hi = (adsr >> 16) as u16;

        // ---- Decode ADSR fields ----
        // Low word: [6:0]=attack_rate  [7]=attack_mode  [11:8]=decay_rate  [15:12]=sustain_level
        let attack_rate = (adsr_lo & 0x7F) as u8;
        let attack_mode_exp = adsr_lo & (1 << 7) != 0;
        // Decay rate: 4-bit field; internally the shift is decay_rate * 4
        let decay_rate = (((adsr_lo >> 8) & 0xF) as u8) << 2; // range 0..60
        let sustain_level = ((adsr_lo >> 12) & 0xF) as i32 * 0x800; // 0..0x7800

        // High word: [6:0]=sustain_rate  [7]=sustain_dir  [8]=sustain_mode
        //            [14:9]=release_rate  [15]=release_mode
        let sustain_rate = (adsr_hi & 0x7F) as u8;
        let sustain_dir_down = adsr_hi & (1 << 7) != 0;
        let sustain_mode_exp = adsr_hi & (1 << 8) != 0;
        let release_rate = (((adsr_hi >> 9) & 0x1F) as u8) << 2; // range 0..60
        let release_mode_exp = adsr_hi & (1 << 14) != 0;

        // Phase transitions
        match self.phase {
            AdsrPhase::Attack if self.volume >= 0x7FFF => {
                self.volume = 0x7FFF;
                self.phase = AdsrPhase::Decay;
                self.counter = 0;
            }
            AdsrPhase::Decay if self.volume <= sustain_level => {
                self.volume = sustain_level;
                self.phase = AdsrPhase::Sustain;
                self.counter = 0;
            }
            AdsrPhase::Release if self.volume <= 0 => {
                self.volume = 0;
                self.phase = AdsrPhase::Off;
                return 0;
            }
            _ => {}
        }

        // Compute step and shift for the current phase
        let (rate, direction_up, exponential) = match self.phase {
            AdsrPhase::Attack => (attack_rate, true, attack_mode_exp),
            AdsrPhase::Decay => (decay_rate, false, true), // decay is always exponential
            AdsrPhase::Sustain => (sustain_rate, !sustain_dir_down, sustain_mode_exp),
            AdsrPhase::Release => (release_rate, false, release_mode_exp),
            AdsrPhase::Off => return 0,
        };

        let (step, shift) = adsr_rate_to_step_shift(rate);
        let threshold = 1u32 << shift;
        self.counter = self.counter.wrapping_add(1);

        if self.counter >= threshold {
            self.counter = 0;
            let mut delta = if direction_up { step } else { -step };

            // Exponential adjustment
            if exponential {
                if direction_up && self.volume > 0x6000 {
                    delta >>= 2; // Scale down step when near maximum
                } else if !direction_up {
                    // Exponential decay: scale step by current volume
                    delta = (delta * self.volume) >> 15;
                    if delta == 0 {
                        delta = -1; // Always decrease at least 1
                    }
                }
            }

            self.volume = (self.volume + delta).clamp(0, 0x7FFF);
        }

        self.volume as i16
    }
}

// ============================================================================
// Voice
// ============================================================================

/// Per-voice ADPCM decoder state
#[derive(Debug, Clone, Default)]
struct AdpcmState {
    /// IIR filter history: previous decoded sample 1
    prev1: i32,
    /// IIR filter history: previous decoded sample 2
    prev2: i32,
    /// Currently decoded 28-sample block
    decoded: [i16; 28],
    /// Index into decoded block (0..28)
    sample_idx: u32,
    /// Flags byte of current block (loop info)
    block_flags: u8,
}

impl AdpcmState {
    /// Decode an ADPCM block (16 bytes) from SPU RAM into `decoded` buffer.
    fn decode_block(&mut self, block: &[u8]) {
        let shift = (block[0] & 0x0F).min(12);
        let filter = ((block[0] >> 4) & 0x0F).min(4) as usize;
        self.block_flags = block[1];

        let f0 = POS_XA_ADPCM_TABLE[filter];
        let f1 = NEG_XA_ADPCM_TABLE[filter];

        for i in 0..28usize {
            let byte = block[2 + i / 2];
            let nibble = if i % 2 == 0 { byte & 0xF } else { byte >> 4 };

            // Sign-extend 4-bit nibble to i32
            let nibble_signed = ((nibble << 4) as i8 >> 4) as i32;

            // Scale by shift: raw = nibble_signed << (12 - shift)
            let raw = nibble_signed << (12 - shift);

            // Apply IIR filter
            let sample = raw + (self.prev1 * f0 + self.prev2 * f1 + 32) / 64;
            let sample = sample.clamp(-32768, 32767);

            self.decoded[i] = sample as i16;
            self.prev2 = self.prev1;
            self.prev1 = sample;
        }
    }
}

/// A single SPU voice
#[derive(Debug, Clone, Default)]
struct Voice {
    /// Volume left (signed 15-bit sweep/envelope)
    vol_left: i16,
    /// Volume right (signed 15-bit sweep/envelope)
    vol_right: i16,
    /// ADPCM sample rate (0..0xFFFF; 0x1000 = 44100 Hz)
    sample_rate: u16,
    /// ADPCM start address in SPU RAM (in 8-byte units)
    start_addr: u16,
    /// ADSR register (32-bit)
    adsr: u32,
    /// ADSR volume readback
    adsr_volume: i16,
    /// ADPCM repeat/loop address (in 8-byte units)
    repeat_addr: u16,
    /// Current ADPCM address in SPU RAM (byte address)
    current_addr: u32,
    /// Pitch counter (Q12 fixed-point; 0x1000 = one ADPCM sample advance)
    pitch_counter: u32,
    /// Voice active flag
    active: bool,
    /// ADPCM decode state
    adpcm: AdpcmState,
    /// ADSR envelope state
    adsr_state: AdsrEnvelope,
}

impl Voice {
    /// Key-on: start voice playback from a given address, decoding the first block.
    /// `start_addr` is a byte address into SPU RAM.
    /// `first_block` is the 16-byte first ADPCM block at that address.
    fn key_on_with_block(&mut self, start_addr: u32, first_block: &[u8; 16]) {
        self.active = true;
        self.current_addr = start_addr;
        self.pitch_counter = 0;
        self.adpcm = AdpcmState::default();
        self.adsr_state.key_on();
        self.adpcm.decode_block(first_block);
    }

    /// Key-off: begin release phase.
    fn key_off(&mut self) {
        self.adsr_state.key_off();
    }
}

// ============================================================================
// SPU
// ============================================================================

/// PS1 SPU
pub struct Spu {
    /// Sound RAM (512 KB)
    ram: Vec<u8>,
    /// 24 voices
    voices: Vec<Voice>,
    /// Main volume left
    main_vol_left: i16,
    /// Main volume right
    main_vol_right: i16,
    /// Reverb output volume left
    reverb_vol_left: i16,
    /// Reverb output volume right
    reverb_vol_right: i16,
    /// Key on register (bits 0-23 = voices)
    key_on: u32,
    /// Key off register
    key_off: u32,
    /// Channel FM (pitch modulation) enable
    fm_enable: u32,
    /// Channel noise enable
    noise_enable: u32,
    /// Channel reverb enable
    reverb_enable: u32,
    /// SPU control register (SPUCNT)
    control: u16,
    /// SPU status register (SPUSTAT)
    status: u16,
    /// Transfer address (in bytes)
    transfer_addr: u32,
    /// Transfer FIFO write position
    transfer_pos: u32,
    /// CD audio volume left
    cd_vol_left: i16,
    /// CD audio volume right
    cd_vol_right: i16,
    /// IRQ flag
    pub irq: bool,
    /// Internal audio sample accumulation buffer (stereo interleaved)
    audio_buffer: Vec<i16>,
}

impl Default for Spu {
    fn default() -> Self {
        Self::new()
    }
}

impl Spu {
    pub fn new() -> Self {
        Self {
            ram: vec![0; SPU_RAM_SIZE],
            voices: (0..NUM_VOICES).map(|_| Voice::default()).collect(),
            main_vol_left: 0x3FFF,
            main_vol_right: 0x3FFF,
            reverb_vol_left: 0,
            reverb_vol_right: 0,
            key_on: 0,
            key_off: 0,
            fm_enable: 0,
            noise_enable: 0,
            reverb_enable: 0,
            control: 0,
            status: 0,
            transfer_addr: 0,
            transfer_pos: 0,
            cd_vol_left: 0,
            cd_vol_right: 0,
            irq: false,
            audio_buffer: Vec::with_capacity(4096),
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Read SPU register (16-bit). Offset is relative to 0x1F801C00.
    pub fn read_register(&self, offset: u32) -> u16 {
        match offset {
            // Voice registers: 0x00–0x17F (16 bytes per voice × 24)
            0x000..=0x17F => {
                let voice_idx = (offset / 0x10) as usize;
                let reg = offset & 0xF;
                if voice_idx < NUM_VOICES {
                    self.read_voice_register(voice_idx, reg)
                } else {
                    0
                }
            }
            0x180 => self.main_vol_left as u16,
            0x182 => self.main_vol_right as u16,
            0x184 => self.reverb_vol_left as u16,
            0x186 => self.reverb_vol_right as u16,
            0x188 => self.key_on as u16,
            0x18A => (self.key_on >> 16) as u16,
            0x18C => self.key_off as u16,
            0x18E => (self.key_off >> 16) as u16,
            0x190 => self.fm_enable as u16,
            0x192 => (self.fm_enable >> 16) as u16,
            0x194 => self.noise_enable as u16,
            0x196 => (self.noise_enable >> 16) as u16,
            0x198 => self.reverb_enable as u16,
            0x19A => (self.reverb_enable >> 16) as u16,
            // Channel on/off status
            0x19C => {
                let mut on = 0u16;
                for i in 0..16usize {
                    if i < NUM_VOICES && self.voices[i].active {
                        on |= 1 << i;
                    }
                }
                on
            }
            0x19E => {
                let mut on = 0u16;
                for i in 16..24usize {
                    if i < NUM_VOICES && self.voices[i].active {
                        on |= 1 << (i - 16);
                    }
                }
                on
            }
            0x1A6 => (self.transfer_addr >> 3) as u16,
            0x1AA => self.control,
            0x1AC => 0,                    // Transfer type
            0x1AE => self.status | 0x0080, // SPUSTAT: always report data transfer ready
            0x1B0 => self.cd_vol_left as u16,
            0x1B2 => self.cd_vol_right as u16,
            _ => 0,
        }
    }

    /// Write SPU register (16-bit). Offset is relative to 0x1F801C00.
    pub fn write_register(&mut self, offset: u32, val: u16) {
        match offset {
            0x000..=0x17F => {
                let voice_idx = (offset / 0x10) as usize;
                let reg = offset & 0xF;
                if voice_idx < NUM_VOICES {
                    self.write_voice_register(voice_idx, reg, val);
                }
            }
            0x180 => self.main_vol_left = val as i16,
            0x182 => self.main_vol_right = val as i16,
            0x184 => self.reverb_vol_left = val as i16,
            0x186 => self.reverb_vol_right = val as i16,
            0x188 => {
                self.key_on = (self.key_on & 0xFFFF_0000) | val as u32;
                self.apply_key_on_lo(val);
            }
            0x18A => {
                self.key_on = (self.key_on & 0x0000_FFFF) | ((val as u32) << 16);
                self.apply_key_on_hi(val);
            }
            0x18C => {
                self.key_off = (self.key_off & 0xFFFF_0000) | val as u32;
                self.apply_key_off_lo(val);
            }
            0x18E => {
                self.key_off = (self.key_off & 0x0000_FFFF) | ((val as u32) << 16);
                self.apply_key_off_hi(val);
            }
            0x190 => self.fm_enable = (self.fm_enable & 0xFFFF_0000) | val as u32,
            0x192 => self.fm_enable = (self.fm_enable & 0x0000_FFFF) | ((val as u32) << 16),
            0x194 => self.noise_enable = (self.noise_enable & 0xFFFF_0000) | val as u32,
            0x196 => self.noise_enable = (self.noise_enable & 0x0000_FFFF) | ((val as u32) << 16),
            0x198 => self.reverb_enable = (self.reverb_enable & 0xFFFF_0000) | val as u32,
            0x19A => self.reverb_enable = (self.reverb_enable & 0x0000_FFFF) | ((val as u32) << 16),
            0x1A6 => {
                // Transfer address (in 8-byte units)
                self.transfer_addr = (val as u32) << 3;
                self.transfer_pos = self.transfer_addr;
            }
            0x1A8 => {
                // Transfer data — write 16-bit to SPU RAM
                let addr = self.transfer_pos as usize;
                if addr + 1 < self.ram.len() {
                    self.ram[addr] = val as u8;
                    self.ram[addr + 1] = (val >> 8) as u8;
                }
                self.transfer_pos += 2;
            }
            0x1AA => self.control = val,
            0x1B0 => self.cd_vol_left = val as i16,
            0x1B2 => self.cd_vol_right = val as i16,
            _ => {}
        }
    }

    fn read_voice_register(&self, idx: usize, reg: u32) -> u16 {
        let v = &self.voices[idx];
        match reg {
            0x0 => v.vol_left as u16,
            0x2 => v.vol_right as u16,
            0x4 => v.sample_rate,
            0x6 => v.start_addr,
            0x8 => v.adsr as u16,
            0xA => (v.adsr >> 16) as u16,
            0xC => v.adsr_volume as u16,
            0xE => v.repeat_addr,
            _ => 0,
        }
    }

    fn write_voice_register(&mut self, idx: usize, reg: u32, val: u16) {
        let v = &mut self.voices[idx];
        match reg {
            0x0 => v.vol_left = val as i16,
            0x2 => v.vol_right = val as i16,
            0x4 => v.sample_rate = val,
            0x6 => v.start_addr = val,
            0x8 => v.adsr = (v.adsr & 0xFFFF_0000) | val as u32,
            0xA => v.adsr = (v.adsr & 0x0000_FFFF) | ((val as u32) << 16),
            0xC => v.adsr_volume = val as i16,
            0xE => v.repeat_addr = val,
            _ => {}
        }
    }

    fn apply_key_on_lo(&mut self, bits: u16) {
        for i in 0..16usize {
            if bits & (1 << i) != 0 && i < NUM_VOICES {
                // Read the first block (16 bytes) from RAM into a local array
                let start_addr = (self.voices[i].start_addr as u32) << 3;
                let mut block = [0u8; 16];
                let addr = start_addr as usize;
                if addr + 16 <= self.ram.len() {
                    block.copy_from_slice(&self.ram[addr..addr + 16]);
                }
                self.voices[i].key_on_with_block(start_addr, &block);
            }
        }
    }

    fn apply_key_on_hi(&mut self, bits: u16) {
        for i in 0..8usize {
            if bits & (1 << i) != 0 {
                let idx = 16 + i;
                if idx < NUM_VOICES {
                    let start_addr = (self.voices[idx].start_addr as u32) << 3;
                    let mut block = [0u8; 16];
                    let addr = start_addr as usize;
                    if addr + 16 <= self.ram.len() {
                        block.copy_from_slice(&self.ram[addr..addr + 16]);
                    }
                    self.voices[idx].key_on_with_block(start_addr, &block);
                }
            }
        }
    }

    fn apply_key_off_lo(&mut self, bits: u16) {
        for i in 0..16usize {
            if bits & (1 << i) != 0 && i < NUM_VOICES {
                self.voices[i].key_off();
            }
        }
    }

    fn apply_key_off_hi(&mut self, bits: u16) {
        for i in 0..8usize {
            if bits & (1 << i) != 0 {
                let idx = 16 + i;
                if idx < NUM_VOICES {
                    self.voices[idx].key_off();
                }
            }
        }
    }

    /// Generate one stereo output sample by mixing all active voices.
    /// Returns (left, right) as i16 pair.
    pub fn generate_sample(&mut self) -> (i16, i16) {
        let mut left_sum: i32 = 0;
        let mut right_sum: i32 = 0;

        for i in 0..NUM_VOICES {
            if !self.voices[i].active {
                continue;
            }

            // Advance pitch counter
            self.voices[i].pitch_counter += self.voices[i].sample_rate as u32;

            // Consume ADPCM samples until pitch counter < 0x1000
            loop {
                if self.voices[i].pitch_counter < 0x1000 {
                    break;
                }
                self.voices[i].pitch_counter -= 0x1000;
                self.voices[i].adpcm.sample_idx += 1;

                if self.voices[i].adpcm.sample_idx >= 28 {
                    self.voices[i].adpcm.sample_idx = 0;
                    self.voices[i].current_addr += 16;
                    self.voices[i].current_addr %= SPU_RAM_SIZE as u32;

                    // Copy only 16 bytes (one ADPCM block) — avoids full 512KB clone
                    let addr = self.voices[i].current_addr as usize;
                    if addr + 16 <= self.ram.len() {
                        let mut block = [0u8; 16];
                        block.copy_from_slice(&self.ram[addr..addr + 16]);
                        self.voices[i].adpcm.decode_block(&block);
                    }

                    let flags = self.voices[i].adpcm.block_flags;
                    if flags & 0x04 != 0 {
                        if flags & 0x02 != 0 {
                            // Loop-repeat: jump to loop start address
                            let loop_addr = (self.voices[i].repeat_addr as u32) << 3;
                            self.voices[i].current_addr = loop_addr;
                        } else {
                            // Loop-stop: deactivate voice
                            self.voices[i].active = false;
                            self.voices[i].adsr_state.phase = AdsrPhase::Off;
                            self.voices[i].adsr_volume = 0;
                            break;
                        }
                    }
                    // Record loop-start address if flag set
                    if flags & 0x01 != 0 {
                        self.voices[i].repeat_addr = (self.voices[i].current_addr >> 3) as u16;
                    }
                }
            }

            if !self.voices[i].active {
                continue;
            }

            // Get current decoded sample
            let sample = self.voices[i].adpcm.decoded[self.voices[i].adpcm.sample_idx as usize];

            // Tick ADSR envelope — copy adsr value to avoid borrow conflict
            let adsr = self.voices[i].adsr;
            let env_vol = self.voices[i].adsr_state.tick(adsr);
            self.voices[i].adsr_volume = env_vol;

            // Apply envelope to sample
            let scaled = (sample as i32 * env_vol as i32) >> 15;
            let scaled = scaled.clamp(-32768, 32767) as i16;

            // Apply voice volume
            let l = ((scaled as i32 * self.voices[i].vol_left as i32) >> 15).clamp(-32768, 32767)
                as i16;
            let r = ((scaled as i32 * self.voices[i].vol_right as i32) >> 15).clamp(-32768, 32767)
                as i16;

            left_sum += l as i32;
            right_sum += r as i32;
        }

        // Apply master volume (15-bit signed)
        left_sum = (left_sum * self.main_vol_left as i32) >> 15;
        right_sum = (right_sum * self.main_vol_right as i32) >> 15;

        let left = left_sum.clamp(-32768, 32767) as i16;
        let right = right_sum.clamp(-32768, 32767) as i16;
        (left, right)
    }

    /// Generate `count` stereo samples into the internal audio buffer.
    /// Samples are interleaved: [L0, R0, L1, R1, ...].
    pub fn tick_samples(&mut self, count: usize) {
        for _ in 0..count {
            let (l, r) = self.generate_sample();
            self.audio_buffer.push(l);
            self.audio_buffer.push(r);
        }
    }

    /// Drain `count` stereo samples (interleaved) from the audio buffer.
    /// If fewer samples are available, the remainder is filled with silence.
    pub fn get_audio_samples(&mut self, count: usize) -> Vec<i16> {
        let needed = count * 2; // stereo
        let available = self.audio_buffer.len();
        let mut out = Vec::with_capacity(needed);

        let take = needed.min(available);
        out.extend_from_slice(&self.audio_buffer[..take]);
        self.audio_buffer.drain(..take);

        // Pad with silence if needed
        while out.len() < needed {
            out.push(0);
        }
        out
    }

    /// Get buffered audio samples (legacy method — returns whatever is buffered).
    pub fn get_audio_buffer(&self) -> &[i16] {
        &self.audio_buffer
    }

    /// Clear the audio output buffer.
    pub fn clear_audio_buffer(&mut self) {
        self.audio_buffer.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_adpcm_block(shift: u8, filter: u8, flags: u8, nibbles: &[u8; 28]) -> [u8; 16] {
        let mut block = [0u8; 16];
        block[0] = shift | (filter << 4);
        block[1] = flags;
        for i in 0..14usize {
            let n0 = nibbles[i * 2];
            let n1 = nibbles[i * 2 + 1];
            block[2 + i] = (n0 & 0xF) | ((n1 & 0xF) << 4);
        }
        block
    }

    #[test]
    fn test_adpcm_decode_silence() {
        // All-zero nibbles with shift=0, filter=0 → all decoded samples should be 0
        let nibbles = [0u8; 28];
        let block = make_adpcm_block(0, 0, 0, &nibbles);
        let mut state = AdpcmState::default();
        state.decode_block(&block);
        for &s in &state.decoded {
            assert_eq!(s, 0, "zero nibbles with no filter should decode to 0");
        }
    }

    #[test]
    fn test_adpcm_decode_positive_value() {
        // Nibble 7 (maximum positive for 4-bit signed) with shift=12 → expected raw = 7
        let nibbles = [7u8; 28]; // nibble = 7 (0b0111)
        let block = make_adpcm_block(12, 0, 0, &nibbles);
        let mut state = AdpcmState::default();
        state.decode_block(&block);
        // With shift=12 and filter=0: raw = 7 << 0 = 7; sample = 7 + 0 = 7
        assert_eq!(state.decoded[0], 7);
        // Subsequent samples add filter contribution from prev1
        // prev1=7 after first sample; next raw=7; sample = 7 + 0 = 7 (filter=0, so no contribution)
        assert_eq!(state.decoded[1], 7);
    }

    #[test]
    fn test_adpcm_decode_negative_nibble() {
        // Nibble 8 = -8 (4-bit signed), shift=12 → raw=-8
        let nibbles = [8u8; 28];
        let block = make_adpcm_block(12, 0, 0, &nibbles);
        let mut state = AdpcmState::default();
        state.decode_block(&block);
        assert_eq!(state.decoded[0], -8);
    }

    #[test]
    fn test_adsr_attack_linear() {
        let mut env = AdsrEnvelope::default();
        env.key_on();

        // ADSR: attack_rate=127 (fastest), linear, decay=0, sustain_level=15 (max), no sustain change
        // attack_rate=127 → step=[7,6,5,4][3]=4, shift=max(0, 11-31)=0 → updates every sample
        let adsr: u32 = 0x007F_F07F; // sustain_level=15, decay=0, attack_mode=0, attack_rate=127
        let mut steps = 0;
        loop {
            let v = env.tick(adsr);
            steps += 1;
            if env.phase != AdsrPhase::Attack || steps > 100_000 {
                assert_eq!(
                    v, 0x7FFF,
                    "ADSR attack should reach max volume when exiting attack phase"
                );
                break;
            }
        }
        assert_eq!(env.volume, 0x7FFF, "ADSR attack should reach max volume");
    }

    #[test]
    fn test_adsr_key_off_enters_release() {
        let mut env = AdsrEnvelope::default();
        env.key_on();
        // Fast attack to get to sustain quickly
        let adsr: u32 = 0x007F_F07F;
        for _ in 0..50000 {
            env.tick(adsr);
            if env.phase == AdsrPhase::Sustain {
                break;
            }
        }
        env.key_off();
        assert_eq!(
            env.phase,
            AdsrPhase::Release,
            "key_off should enter Release phase"
        );
    }

    #[test]
    fn test_spu_silence_when_no_voices_active() {
        let mut spu = Spu::new();
        spu.main_vol_left = 0x7FFF;
        spu.main_vol_right = 0x7FFF;
        let (l, r) = spu.generate_sample();
        assert_eq!(l, 0);
        assert_eq!(r, 0);
    }

    #[test]
    fn test_spu_tick_samples_count() {
        let mut spu = Spu::new();
        spu.tick_samples(100);
        // Should have 200 values (stereo interleaved)
        assert_eq!(spu.audio_buffer.len(), 200);
    }

    #[test]
    fn test_spu_get_audio_samples_drains_buffer() {
        let mut spu = Spu::new();
        spu.tick_samples(50);
        let samples = spu.get_audio_samples(30); // request 30 stereo = 60 values
        assert_eq!(samples.len(), 60);
        // Buffer should have 40 values left (50*2 - 60 = 40)
        assert_eq!(spu.audio_buffer.len(), 40);
    }

    #[test]
    fn test_spu_get_audio_samples_pads_silence() {
        let mut spu = Spu::new();
        // Buffer empty, request 10 stereo samples
        let samples = spu.get_audio_samples(10);
        assert_eq!(samples.len(), 20);
        for &s in &samples {
            assert_eq!(s, 0, "silence when no data in buffer");
        }
    }
}
