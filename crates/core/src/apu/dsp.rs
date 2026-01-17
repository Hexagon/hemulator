//! S-DSP (Digital Signal Processor) for SNES audio
//!
//! The S-DSP is the SNES's audio generation chip, working alongside the SPC700 CPU.
//! It provides:
//! - 8 independent voices with ADPCM (BRR) sample playback
//! - ADSR envelope generation per voice
//! - Pitch control with 14-bit precision
//! - Gaussian interpolation for sample rate conversion
//! - Voice mixing to stereo output
//! - Echo/reverb FIR filter with 8 taps
//! - Noise generator
//! - Pitch modulation
//!
//! **DSP Register Map** (128 registers, accessed via $F2/$F3):
//!
//! Per-voice registers (0x00-0x7F, 16 bytes per voice):
//! - V0L-V7L ($x0): Left volume
//! - V0R-V7R ($x1): Right volume  
//! - V0PL-V7PL ($x2): Pitch low
//! - V0PH-V7PH ($x3): Pitch high
//! - V0SRCN-V7SRCN ($x4): Source/sample number
//! - V0ADSR1-V7ADSR1 ($x5): ADSR envelope 1 (attack, decay)
//! - V0ADSR2-V7ADSR2 ($x6): ADSR envelope 2 (sustain, release)
//! - V0GAIN-V7GAIN ($x7): Gain (if ADSR disabled)
//! - V0ENVX-V7ENVX ($x8): Current envelope value (read-only)
//! - V0OUTX-V7OUTX ($x9): Current sample output (read-only)
//!
//! Global registers:
//! - MVOLL ($0C): Master volume left
//! - MVOLR ($1C): Master volume right
//! - EVOLL ($2C): Echo volume left
//! - EVOLR ($3C): Echo volume right
//! - KON ($4C): Key on flags (bit per voice)
//! - KOF ($5C): Key off flags (bit per voice)
//! - FLG ($6C): DSP flags (reset, mute, echo disable, noise frequency)
//! - ENDX ($7C): Voice ended flags (read-only)
//! - EFB ($0D): Echo feedback
//! - PMON ($2D): Pitch modulation enable
//! - NON ($3D): Noise enable
//! - EON ($4D): Echo enable
//! - DIR ($5D): Sample table directory page
//! - ESA ($6D): Echo buffer start address
//! - EDL ($7D): Echo delay/length
//! - FIR0-FIR7 ($xF): Echo FIR filter coefficients
//!
//! **BRR (Bit Rate Reduction) Sample Format**:
//! - 4-bit ADPCM compression (9:1 ratio vs 16-bit PCM)
//! - Samples stored in 9-byte blocks (1 header + 16 nibbles)
//! - Header byte: RRRF LLLL (R=range/shift, F=filter, L=loop flags)
//! - Filters: 0=direct, 1=linear, 2=quadratic1, 3=quadratic2
//! - Loop flags: bit 0=end, bit 1=loop
//!
//! **References**:
//! - [S-DSP Registers](https://snes.nesdev.org/wiki/S-DSP_registers)
//! - [DSP Envelopes](https://snes.nesdev.org/wiki/DSP_envelopes)
//! - [BRR Samples](https://snes.nesdev.org/wiki/BRR_samples)
//! - [Fullsnes](https://problemkaputt.de/fullsnes.htm#snesapudspdigitalsignalprocessor)

use crate::logging::{log, LogCategory, LogLevel};

/// BRR (Bit Rate Reduction) ADPCM decoder
///
/// Decodes 4-bit ADPCM samples to 16-bit PCM with filtering
#[derive(Clone, Debug)]
struct BrrDecoder {
    /// Previous decoded sample (for filter)
    prev1: i16,
    /// Second previous decoded sample (for filter)
    prev2: i16,
}

impl BrrDecoder {
    fn new() -> Self {
        Self { prev1: 0, prev2: 0 }
    }

    /// Reset decoder state
    fn reset(&mut self) {
        self.prev1 = 0;
        self.prev2 = 0;
    }

    /// Decode a 4-bit nibble to 16-bit PCM sample
    ///
    /// - `nibble`: 4-bit signed value (-8 to 7)
    /// - `shift`: Range/shift factor (0-12)
    /// - `filter`: Filter type (0-3)
    #[allow(dead_code)] // Will be used when BRR decoding is implemented
    fn decode_nibble(&mut self, nibble: i8, shift: u8, filter: u8) -> i16 {
        // Convert 4-bit signed to extended value
        let mut sample = (nibble as i32) << shift;
        sample >>= 1; // Hardware quirk: right shift by 1 after left shift

        // Apply filter based on previous samples
        // Reference: https://problemkaputt.de/fullsnes.htm#snesapudspbrrsamples
        sample += match filter {
            0 => 0, // Direct (no filter)
            1 => {
                // Linear filter: prev1 * 15/16
                (self.prev1 as i32) - ((self.prev1 as i32) >> 4)
            }
            2 => {
                // Quadratic filter 1: prev1 * 61/32 - prev2 * 15/16
                let a = ((self.prev1 as i32) << 1) - ((self.prev1 as i32 * 3) >> 5);
                let b = ((self.prev2 as i32) >> 1) - ((self.prev2 as i32) >> 5);
                a - b
            }
            3 => {
                // Quadratic filter 2: prev1 * 115/64 - prev2 * 13/16
                let a = ((self.prev1 as i32) << 1) - ((self.prev1 as i32 * 13) >> 6);
                let b = ((self.prev2 as i32) >> 1) - ((self.prev2 as i32 * 3) >> 4);
                a - b
            }
            _ => 0, // Invalid filter (shouldn't happen)
        };

        // Clamp to 16-bit signed range and update history
        let result = sample.clamp(-32768, 32767) as i16;
        self.prev2 = self.prev1;
        self.prev1 = result;
        result
    }
}

/// ADSR envelope generator
///
/// Generates volume envelope with Attack, Decay, Sustain, Release phases
#[derive(Clone, Debug)]
struct Envelope {
    /// Current envelope level (0-0x7FF, 11-bit)
    level: u16,
    /// Current envelope mode
    mode: EnvelopeMode,
    /// ADSR1 register value
    adsr1: u8,
    /// ADSR2 register value
    adsr2: u8,
    /// GAIN register value
    gain: u8,
    /// Use GAIN mode instead of ADSR
    use_gain: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum EnvelopeMode {
    Attack,
    Decay,
    Sustain,
    Release,
}

impl Envelope {
    fn new() -> Self {
        Self {
            level: 0,
            mode: EnvelopeMode::Release,
            adsr1: 0,
            adsr2: 0,
            gain: 0,
            use_gain: true, // Default to GAIN mode
        }
    }

    /// Trigger key on (start attack phase)
    fn key_on(&mut self) {
        if !self.use_gain {
            self.mode = EnvelopeMode::Attack;
            self.level = 0;
        }
    }

    /// Trigger key off (start release phase)
    fn key_off(&mut self) {
        if !self.use_gain {
            self.mode = EnvelopeMode::Release;
        }
    }

    /// Clock the envelope (called at 32kHz)
    fn clock(&mut self) {
        if self.use_gain {
            // GAIN mode: direct level control
            // TODO: Implement GAIN modes (direct, linear increase/decrease, exponential)
            // For now, use direct mode
            self.level = ((self.gain & 0x7F) as u16) << 4;
        } else {
            // ADSR mode
            // TODO: Implement full ADSR envelope
            // For now, just set to max level in attack/sustain
            match self.mode {
                EnvelopeMode::Attack => {
                    // Simplified: instant attack to max level
                    self.level = 0x7FF;
                    self.mode = EnvelopeMode::Decay;
                }
                EnvelopeMode::Decay => {
                    // Simplified: instant transition to sustain
                    let sustain_level = ((self.adsr2 >> 5) & 0x07) as u16;
                    self.level = (sustain_level + 1) << 8;
                    self.mode = EnvelopeMode::Sustain;
                }
                EnvelopeMode::Sustain => {
                    // Stay at sustain level
                }
                EnvelopeMode::Release => {
                    // Simplified: decay to zero
                    if self.level > 0 {
                        self.level = self.level.saturating_sub(8);
                    }
                }
            }
        }
    }

    /// Get current envelope output value (0-0x7FF)
    fn output(&self) -> u16 {
        self.level
    }
}

/// Voice state for one of the 8 DSP voices
#[derive(Clone, Debug)]
struct Voice {
    /// Left volume (signed 8-bit)
    volume_left: i8,
    /// Right volume (signed 8-bit)
    volume_right: i8,
    /// Pitch (14-bit, 0-0x3FFF)
    pitch: u16,
    /// Sample source number (0-255)
    source: u8,
    /// ADSR envelope generator
    envelope: Envelope,
    /// BRR decoder
    brr_decoder: BrrDecoder,
    /// Current sample buffer position (16.16 fixed point)
    /// High 16 bits = sample index, low 16 bits = fractional position
    position: u32,
    /// Current BRR block buffer (16 decoded samples)
    sample_buffer: [i16; 16],
    /// Current sample buffer index
    buffer_index: usize,
    /// Voice is playing
    playing: bool,
    /// Voice ended (BRR end flag reached)
    ended: bool,
    /// Current sample address in RAM
    #[allow(dead_code)]
    sample_addr: u16,
    /// Loop start address (from BRR header)
    #[allow(dead_code)]
    loop_addr: u16,
}

impl Voice {
    fn new() -> Self {
        Self {
            volume_left: 0,
            volume_right: 0,
            pitch: 0,
            source: 0,
            envelope: Envelope::new(),
            brr_decoder: BrrDecoder::new(),
            position: 0,
            sample_buffer: [0; 16],
            buffer_index: 0,
            playing: false,
            ended: false,
            sample_addr: 0,
            loop_addr: 0,
        }
    }

    /// Start playback (key on)
    fn key_on(&mut self, sample_addr: u16) {
        self.playing = true;
        self.ended = false;
        self.position = 0;
        self.buffer_index = 0;
        self.sample_addr = sample_addr;
        self.brr_decoder.reset();
        self.envelope.key_on();
        self.sample_buffer = [0; 16]; // Clear sample buffer
    }

    /// Stop playback (key off)
    fn key_off(&mut self) {
        self.envelope.key_off();
    }

    /// Decode a BRR block from RAM into the sample buffer
    /// Returns true if this is the last block (end flag set)
    fn decode_brr_block(&mut self, ram: &[u8; 0x10000]) -> bool {
        let addr = self.sample_addr as usize;
        if addr + 8 >= 0x10000 {
            // Invalid address, stop playback
            self.ended = true;
            return true;
        }

        // Read BRR block header
        let header = ram[addr];
        let shift = (header >> 4) & 0x0F;
        let filter = (header >> 2) & 0x03;
        let end_flag = (header & 0x01) != 0;
        let loop_flag = (header & 0x02) != 0;

        // Decode 16 samples (8 bytes of nibbles)
        for i in 0..8 {
            let byte = ram[addr + 1 + i];

            // High nibble (bits 7-4)
            let high_nibble = ((byte >> 4) as i8) << 4 >> 4; // Sign-extend 4-bit to 8-bit
            self.sample_buffer[i * 2] = self.brr_decoder.decode_nibble(high_nibble, shift, filter);

            // Low nibble (bits 3-0)
            let low_nibble = ((byte & 0x0F) as i8) << 4 >> 4; // Sign-extend 4-bit to 8-bit
            self.sample_buffer[i * 2 + 1] =
                self.brr_decoder.decode_nibble(low_nibble, shift, filter);
        }

        // Advance to next block
        self.sample_addr = self.sample_addr.wrapping_add(9);

        // Handle end/loop flags
        if end_flag {
            if loop_flag {
                // Loop back to loop address
                self.sample_addr = self.loop_addr;
            } else {
                // End playback
                self.ended = true;
            }
            return true;
        }

        false
    }

    /// Advance sample position based on pitch
    /// Returns true if a new BRR block needs to be decoded
    fn advance_position(&mut self) -> bool {
        // Pitch is 14-bit, represents rate multiplier (4096 = 32kHz, same as DSP clock)
        // Position is 16.16 fixed point
        self.position = self.position.wrapping_add(self.pitch as u32);

        // Check if we've moved to the next sample
        let sample_index = (self.position >> 16) as usize;

        if sample_index >= 16 {
            // Wrapped past the buffer, need new BRR block
            self.position &= 0xFFFF; // Keep fractional part
            self.buffer_index = 0;
            return true;
        } else if sample_index != self.buffer_index {
            // Moved to next sample within buffer
            self.buffer_index = sample_index;
        }

        false
    }

    /// Get current sample output with interpolation
    /// Returns the interpolated sample value
    fn get_sample(&self) -> i16 {
        if !self.playing || self.ended {
            return 0;
        }

        // Simple linear interpolation for now
        // TODO: Implement Gaussian interpolation for hardware accuracy
        let frac = self.position & 0xFFFF;
        let curr_idx = self.buffer_index;
        let next_idx = (curr_idx + 1) & 0xF; // Wrap at 16

        let curr_sample = self.sample_buffer[curr_idx] as i32;
        let next_sample = self.sample_buffer[next_idx] as i32;

        // Linear interpolation: curr + (next - curr) * frac / 65536
        let interpolated = curr_sample + ((next_sample - curr_sample) * frac as i32) / 65536;
        interpolated.clamp(-32768, 32767) as i16
    }

    /// Get voice output after applying envelope and volume
    fn output(&self) -> (i16, i16) {
        let sample = self.get_sample();
        let env = self.envelope.output() as i32;

        // Apply envelope (11-bit * 16-bit >> 11 = 16-bit)
        let with_envelope = (sample as i32 * env) >> 11;

        // Apply left/right volume (8-bit signed * 16-bit >> 7 = 16-bit)
        let left = ((with_envelope * self.volume_left as i32) >> 7) as i16;
        let right = ((with_envelope * self.volume_right as i32) >> 7) as i16;

        (left, right)
    }
}

/// S-DSP Digital Signal Processor
///
/// 8-voice stereo audio synthesis with BRR sample playback
pub struct Dsp {
    /// 8 voices
    voices: [Voice; 8],
    /// Master volume left (signed 8-bit)
    master_volume_left: i8,
    /// Master volume right (signed 8-bit)
    master_volume_right: i8,
    /// Echo volume left (signed 8-bit)
    echo_volume_left: i8,
    /// Echo volume right (signed 8-bit)
    echo_volume_right: i8,
    /// Key on flags (bit per voice, write triggers)
    key_on: u8,
    /// Key off flags (bit per voice, write triggers)
    key_off: u8,
    /// Sample table directory page ($xx00 address)
    sample_dir: u8,
    /// Flags register (reset, mute, echo disable, noise freq)
    flags: u8,
    /// Voice ended flags (read-only, bit per voice)
    endx: u8,
    /// Pitch modulation enable (bit per voice, voice 0 ignored)
    pitch_mod: u8,
    /// Noise enable (bit per voice)
    noise_enable: u8,
    /// Echo enable (bit per voice)
    echo_enable: u8,
    /// Echo feedback
    echo_feedback: i8,
    /// Echo buffer start address
    echo_addr: u8,
    /// Echo delay/length
    echo_delay: u8,
    /// FIR filter coefficients (8 taps)
    fir_coeff: [i8; 8],
    /// Cycle counter for 32kHz envelope updates
    cycle_counter: u32,
    /// Reference to APU RAM (for BRR sample access)
    /// This will be set from the SPC700 module
    ram: Option<*const [u8; 0x10000]>,
}

impl Dsp {
    /// Create a new DSP
    pub fn new() -> Self {
        Self {
            voices: [
                Voice::new(),
                Voice::new(),
                Voice::new(),
                Voice::new(),
                Voice::new(),
                Voice::new(),
                Voice::new(),
                Voice::new(),
            ],
            master_volume_left: 0,
            master_volume_right: 0,
            echo_volume_left: 0,
            echo_volume_right: 0,
            key_on: 0,
            key_off: 0,
            sample_dir: 0,
            flags: 0xE0, // Reset, mute, echo disable on startup
            endx: 0,
            pitch_mod: 0,
            noise_enable: 0,
            echo_enable: 0,
            echo_feedback: 0,
            echo_addr: 0,
            echo_delay: 0,
            fir_coeff: [0; 8],
            cycle_counter: 0,
            ram: None,
        }
    }

    /// Set reference to APU RAM for BRR sample access
    pub fn set_ram(&mut self, ram: *const [u8; 0x10000]) {
        self.ram = Some(ram);
    }

    /// Read sample directory entry for a given source number
    /// Returns (start_address, loop_address) tuple
    fn get_sample_addresses(&self, source: u8) -> Option<(u16, u16)> {
        let ram = unsafe { self.ram?.as_ref()? };

        // Sample directory is at (sample_dir << 8), each entry is 4 bytes
        let dir_addr = (self.sample_dir as usize) << 8;
        let entry_addr = dir_addr + (source as usize) * 4;

        if entry_addr + 3 >= 0x10000 {
            return None;
        }

        // Read start address (16-bit little-endian)
        let start_addr = ram[entry_addr] as u16 | ((ram[entry_addr + 1] as u16) << 8);

        // Read loop address (16-bit little-endian)
        let loop_addr = ram[entry_addr + 2] as u16 | ((ram[entry_addr + 3] as u16) << 8);

        Some((start_addr, loop_addr))
    }

    /// Write to a DSP register
    pub fn write_register(&mut self, addr: u8, value: u8) {
        // DSP registers are organized in groups
        let voice = (addr >> 4) & 0x0F; // Voice number (0-7 for voice regs)
        let reg = addr & 0x0F; // Register within voice

        // Handle global registers first (they have specific low nibble values)
        match addr {
            // Master volumes
            0x0C => self.master_volume_left = value as i8,
            0x1C => self.master_volume_right = value as i8,
            0x2C => self.echo_volume_left = value as i8,
            0x3C => self.echo_volume_right = value as i8,
            0x4C => {
                // KON - Key On
                self.key_on = value;

                // Get RAM reference for initial BRR block decode
                let ram = if let Some(ram_ptr) = self.ram {
                    unsafe { ram_ptr.as_ref() }
                } else {
                    None
                };

                // Collect sample addresses for all voices that will be keyed on
                let mut sample_addrs: Vec<Option<(u16, u16)>> = Vec::with_capacity(8);
                for (i, voice) in self.voices.iter().enumerate() {
                    if value & (1 << i) != 0 {
                        sample_addrs.push(self.get_sample_addresses(voice.source));
                    } else {
                        sample_addrs.push(None);
                    }
                }

                // Now key on the voices with the collected addresses
                for (i, voice) in self.voices.iter_mut().enumerate() {
                    if value & (1 << i) != 0 {
                        if let Some(Some((start_addr, loop_addr))) = sample_addrs.get(i) {
                            voice.loop_addr = *loop_addr;
                            voice.key_on(*start_addr);

                            // Decode initial BRR block
                            if let Some(ram_ref) = ram {
                                voice.decode_brr_block(ram_ref);
                            }

                            log(LogCategory::APU, LogLevel::Debug, || {
                                format!(
                                    "DSP: Voice {} key on, source={}, start=${:04X}, loop=${:04X}",
                                    i, voice.source, start_addr, loop_addr
                                )
                            });
                        } else {
                            log(LogCategory::APU, LogLevel::Warn, || {
                                format!(
                                    "DSP: Voice {} key on failed - invalid sample source {}",
                                    i, voice.source
                                )
                            });
                        }
                    }
                }
            }
            0x5C => {
                // KOF - Key Off
                self.key_off = value;
                for (i, voice) in self.voices.iter_mut().enumerate() {
                    if value & (1 << i) != 0 {
                        voice.key_off();
                        log(LogCategory::APU, LogLevel::Debug, || {
                            format!("DSP: Voice {} key off", i)
                        });
                    }
                }
            }
            0x6C => self.flags = value,
            0x7C => {} // ENDX read-only
            // Other globals with D suffix
            0x0D => self.echo_feedback = value as i8,
            0x2D => self.pitch_mod = value,
            0x3D => self.noise_enable = value,
            0x4D => self.echo_enable = value,
            0x5D => self.sample_dir = value,
            0x6D => self.echo_addr = value,
            0x7D => self.echo_delay = value & 0x0F,
            // FIR filter coefficients (F suffix)
            0x0F | 0x1F | 0x2F | 0x3F | 0x4F | 0x5F | 0x6F | 0x7F => {
                let fir_idx = (addr >> 4) as usize;
                self.fir_coeff[fir_idx] = value as i8;
            }
            // Per-voice registers (everything else in 0x00-0x7F range)
            _ if addr < 0x80 => {
                if voice >= 8 {
                    return; // Invalid voice
                }
                let v = &mut self.voices[voice as usize];
                match reg {
                    0x0 => v.volume_left = value as i8,
                    0x1 => v.volume_right = value as i8,
                    0x2 => v.pitch = (v.pitch & 0x3F00) | (value as u16),
                    0x3 => v.pitch = (v.pitch & 0x00FF) | (((value & 0x3F) as u16) << 8),
                    0x4 => v.source = value,
                    0x5 => {
                        v.envelope.adsr1 = value;
                        v.envelope.use_gain = (value & 0x80) == 0;
                    }
                    0x6 => v.envelope.adsr2 = value,
                    0x7 => v.envelope.gain = value,
                    0x8 => {} // ENVX read-only
                    0x9 => {} // OUTX read-only
                    _ => {}
                }
            }
            _ => {}
        }
    }

    /// Read from a DSP register
    pub fn read_register(&self, addr: u8) -> u8 {
        let voice = (addr >> 4) & 0x0F;
        let reg = addr & 0x0F;

        // Handle global registers first
        match addr {
            // Master volumes
            0x0C => self.master_volume_left as u8,
            0x1C => self.master_volume_right as u8,
            0x2C => self.echo_volume_left as u8,
            0x3C => self.echo_volume_right as u8,
            0x4C => 0, // KON write-only
            0x5C => 0, // KOF write-only
            0x6C => self.flags,
            0x7C => self.endx,
            // Other globals with D suffix
            0x0D => self.echo_feedback as u8,
            0x2D => self.pitch_mod,
            0x3D => self.noise_enable,
            0x4D => self.echo_enable,
            0x5D => self.sample_dir,
            0x6D => self.echo_addr,
            0x7D => self.echo_delay,
            // FIR filter coefficients (F suffix)
            0x0F | 0x1F | 0x2F | 0x3F | 0x4F | 0x5F | 0x6F | 0x7F => {
                let fir_idx = (addr >> 4) as usize;
                self.fir_coeff[fir_idx] as u8
            }
            // Per-voice registers
            _ if addr < 0x80 => {
                if voice >= 8 {
                    return 0;
                }
                let v = &self.voices[voice as usize];
                match reg {
                    0x0 => v.volume_left as u8,
                    0x1 => v.volume_right as u8,
                    0x2 => (v.pitch & 0xFF) as u8,
                    0x3 => ((v.pitch >> 8) & 0x3F) as u8,
                    0x4 => v.source,
                    0x5 => v.envelope.adsr1,
                    0x6 => v.envelope.adsr2,
                    0x7 => v.envelope.gain,
                    0x8 => (v.envelope.output() >> 4) as u8, // ENVX (11-bit >> 4 = 7-bit)
                    0x9 => (v.get_sample() >> 8) as u8,      // OUTX (high 8 bits of sample)
                    _ => 0,
                }
            }
            _ => 0,
        }
    }

    /// Clock the DSP and generate one stereo sample
    /// Should be called at 32kHz (every 32 CPU cycles at 1.024 MHz)
    pub fn clock(&mut self) -> (i16, i16) {
        // Check if muted
        if self.flags & 0x40 != 0 {
            return (0, 0);
        }

        // Get RAM reference for BRR decoding
        let ram = if let Some(ram_ptr) = self.ram {
            unsafe { ram_ptr.as_ref() }
        } else {
            None
        };

        // Update each voice
        self.endx = 0; // Clear ended flags
        for (i, voice) in self.voices.iter_mut().enumerate() {
            // Update envelope
            voice.envelope.clock();

            // Advance sample position if voice is playing
            if voice.playing && !voice.ended && voice.advance_position() {
                // Need to decode next BRR block
                if let Some(ram_ref) = ram {
                    let is_end = voice.decode_brr_block(ram_ref);
                    if is_end && voice.ended {
                        // Voice reached end without loop
                        self.endx |= 1 << i;
                    }
                } else {
                    // No RAM available, stop voice
                    voice.ended = true;
                    self.endx |= 1 << i;
                }
            }

            // Update ENDX flag if voice ended
            if voice.ended {
                self.endx |= 1 << i;
            }
        }

        // Mix all voices
        let mut left = 0i32;
        let mut right = 0i32;

        for voice in &self.voices {
            let (l, r) = voice.output();
            left += l as i32;
            right += r as i32;
        }

        // Apply master volume (8-bit signed * 16-bit >> 7 = 16-bit)
        left = (left * self.master_volume_left as i32) >> 7;
        right = (right * self.master_volume_right as i32) >> 7;

        // Clamp to 16-bit range
        let left = left.clamp(-32768, 32767) as i16;
        let right = right.clamp(-32768, 32767) as i16;

        (left, right)
    }

    /// Reset the DSP
    pub fn reset(&mut self) {
        for voice in &mut self.voices {
            *voice = Voice::new();
        }
        self.master_volume_left = 0;
        self.master_volume_right = 0;
        self.echo_volume_left = 0;
        self.echo_volume_right = 0;
        self.key_on = 0;
        self.key_off = 0;
        self.sample_dir = 0;
        self.flags = 0xE0;
        self.endx = 0;
        self.pitch_mod = 0;
        self.noise_enable = 0;
        self.echo_enable = 0;
        self.echo_feedback = 0;
        self.echo_addr = 0;
        self.echo_delay = 0;
        self.fir_coeff = [0; 8];
        self.cycle_counter = 0;
    }
}

impl Default for Dsp {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brr_decoder_direct_filter() {
        let mut decoder = BrrDecoder::new();
        // Test direct filter (no filtering)
        let sample = decoder.decode_nibble(7, 0, 0); // Max positive nibble, no shift
        assert_eq!(sample, 3); // 7 >> 1 = 3
    }

    #[test]
    fn test_envelope_key_on_off() {
        let mut env = Envelope::new();
        env.use_gain = false;
        env.adsr1 = 0xFF; // Fast attack
        env.adsr2 = 0xFF; // Max sustain

        env.key_on();
        assert_eq!(env.mode, EnvelopeMode::Attack);

        env.key_off();
        assert_eq!(env.mode, EnvelopeMode::Release);
    }

    #[test]
    fn test_dsp_register_write_read() {
        let mut dsp = Dsp::new();

        // Test voice 0 left volume
        dsp.write_register(0x00, 0x7F);
        assert_eq!(dsp.read_register(0x00), 0x7F);

        // Test master volume left
        dsp.write_register(0x0C, 0x40);
        assert_eq!(dsp.read_register(0x0C), 0x40);
    }

    #[test]
    fn test_dsp_key_on() {
        let mut dsp = Dsp::new();
        
        // Setup a minimal RAM with sample directory
        let mut ram = Box::new([0u8; 0x10000]);
        
        // Setup sample directory at $0000
        // Entry 0: start=$0100, loop=$0100
        ram[0] = 0x00; // Start address low
        ram[1] = 0x01; // Start address high
        ram[2] = 0x00; // Loop address low
        ram[3] = 0x01; // Loop address high
        
        // Add a minimal BRR block at $0100
        // Header: no end flag, no loop, no filter, shift=0
        ram[0x100] = 0x00; // No flags
        // 8 bytes of sample data (all zeros)
        for i in 0x101..=0x108 {
            ram[i] = 0;
        }
        
        dsp.set_ram(&*ram as *const [u8; 0x10000]);
        
        // Set source to 0
        dsp.write_register(0x04, 0x00);

        // Key on voice 0
        dsp.write_register(0x4C, 0x01);
        assert!(dsp.voices[0].playing);
        assert!(!dsp.voices[0].ended);
    }

    #[test]
    fn test_dsp_silence_when_muted() {
        let mut dsp = Dsp::new();

        // Set mute flag
        dsp.write_register(0x6C, 0x40);

        // Clock should return silence
        let (left, right) = dsp.clock();
        assert_eq!(left, 0);
        assert_eq!(right, 0);
    }
}
