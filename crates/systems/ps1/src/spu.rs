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
//! - nocash PSX-SPX SPU documentation
//! - Martin Korth's PSX SPU register map

/// SPU RAM size: 512 KB
const SPU_RAM_SIZE: usize = 512 * 1024;

/// Number of voices
const NUM_VOICES: usize = 24;

/// A single SPU voice
#[derive(Debug, Clone, Default)]
struct Voice {
    /// Volume left
    vol_left: i16,
    /// Volume right
    vol_right: i16,
    /// ADPCM sample rate
    sample_rate: u16,
    /// ADPCM start address (in 8-byte units)
    start_addr: u16,
    /// ADSR: Attack/Decay/Sustain/Release
    adsr: u32,
    /// Current ADSR volume
    adsr_volume: i16,
    /// ADPCM repeat address
    repeat_addr: u16,
    /// Current address counter
    current_addr: u32,
    /// Current pitch counter
    pitch_counter: u32,
    /// Active flag
    active: bool,
}

/// PS1 SPU
pub struct Spu {
    /// Sound RAM
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
    /// SPU control register
    control: u16,
    /// SPU status register
    status: u16,
    /// Transfer address
    transfer_addr: u32,
    /// Transfer FIFO write position
    transfer_pos: u32,
    /// CD audio volume left
    cd_vol_left: i16,
    /// CD audio volume right
    cd_vol_right: i16,
    /// Audio output buffer
    output_buffer: Vec<i16>,
    /// Output buffer position
    output_pos: usize,
    /// IRQ flag
    pub irq: bool,
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
            main_vol_left: 0,
            main_vol_right: 0,
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
            output_buffer: vec![0; 2048],
            output_pos: 0,
            irq: false,
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Read SPU register (16-bit). Offset is relative to 0x1F801C00.
    pub fn read_register(&self, offset: u32) -> u16 {
        match offset {
            // Voice registers: 0x00-0x17F (16 bytes per voice × 24)
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
            0x1AA => self.control,
            0x1AE => self.status,
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
                self.apply_key_on();
            }
            0x18A => {
                self.key_on = (self.key_on & 0x0000_FFFF) | ((val as u32) << 16);
                self.apply_key_on();
            }
            0x18C => {
                self.key_off = (self.key_off & 0xFFFF_0000) | val as u32;
                self.apply_key_off();
            }
            0x18E => {
                self.key_off = (self.key_off & 0x0000_FFFF) | ((val as u32) << 16);
                self.apply_key_off();
            }
            0x190 => self.fm_enable = (self.fm_enable & 0xFFFF_0000) | val as u32,
            0x192 => self.fm_enable = (self.fm_enable & 0x0000_FFFF) | ((val as u32) << 16),
            0x194 => self.noise_enable = (self.noise_enable & 0xFFFF_0000) | val as u32,
            0x196 => self.noise_enable = (self.noise_enable & 0x0000_FFFF) | ((val as u32) << 16),
            0x198 => self.reverb_enable = (self.reverb_enable & 0xFFFF_0000) | val as u32,
            0x19A => self.reverb_enable = (self.reverb_enable & 0x0000_FFFF) | ((val as u32) << 16),
            0x1AA => self.control = val,
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

    fn apply_key_on(&mut self) {
        for i in 0..NUM_VOICES {
            if self.key_on & (1 << i) != 0 {
                self.voices[i].active = true;
                self.voices[i].current_addr = (self.voices[i].start_addr as u32) << 3;
                self.voices[i].pitch_counter = 0;
                self.voices[i].adsr_volume = 0;
            }
        }
    }

    fn apply_key_off(&mut self) {
        for i in 0..NUM_VOICES {
            if self.key_off & (1 << i) != 0 {
                self.voices[i].active = false;
            }
        }
    }

    /// Generate audio samples. Called at ~44.1 kHz.
    /// Returns (left, right) sample pair.
    pub fn generate_sample(&mut self) -> (i16, i16) {
        // TODO: Implement proper ADPCM decoding, ADSR envelope, reverb

        let mut left_sum: i32 = 0;
        let mut right_sum: i32 = 0;

        for voice in &self.voices {
            if !voice.active {
                continue;
            }
            // Stub: generate silence for active voices
            // Real implementation would decode ADPCM samples from SPU RAM
            let _ = voice;
        }

        // Apply master volume
        left_sum = (left_sum * self.main_vol_left as i32) >> 15;
        right_sum = (right_sum * self.main_vol_right as i32) >> 15;

        let left = left_sum.clamp(-32768, 32767) as i16;
        let right = right_sum.clamp(-32768, 32767) as i16;
        (left, right)
    }

    /// Get buffered audio samples.
    pub fn get_audio_buffer(&self) -> &[i16] {
        &self.output_buffer[..self.output_pos]
    }

    /// Clear the audio output buffer.
    pub fn clear_audio_buffer(&mut self) {
        self.output_pos = 0;
    }
}
