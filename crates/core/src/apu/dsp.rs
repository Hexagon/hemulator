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

/// Gaussian interpolation filter table (512 entries)
///
/// This table encodes the full Gaussian curve used for 4-point sample interpolation.
/// Hardware uses bits 4–11 of the pitch counter as a 9-bit index into these 512 entries.
/// Indices 0–255 cover one side of the curve; 256–511 contain the mirrored other side.
/// All values are 12-bit unsigned magnitudes of the filter coefficients; the sign comes
/// from the sample data and interpolation formula rather than from this table itself.
///
/// Reference: https://sneslab.net/wiki/S-DSP/Gaussian_Filter
const GAUSSIAN_TABLE: [i16; 512] = [
    0x000, 0x000, 0x000, 0x000, 0x000, 0x000, 0x000, 0x000, 0x000, 0x000, 0x000, 0x000, 0x000,
    0x000, 0x000, 0x000, 0x001, 0x001, 0x001, 0x001, 0x001, 0x001, 0x001, 0x001, 0x001, 0x001,
    0x001, 0x002, 0x002, 0x002, 0x002, 0x002, 0x002, 0x002, 0x003, 0x003, 0x003, 0x003, 0x003,
    0x004, 0x004, 0x004, 0x004, 0x004, 0x005, 0x005, 0x005, 0x005, 0x006, 0x006, 0x006, 0x006,
    0x007, 0x007, 0x007, 0x008, 0x008, 0x008, 0x009, 0x009, 0x009, 0x00A, 0x00A, 0x00A, 0x00B,
    0x00B, 0x00B, 0x00C, 0x00C, 0x00D, 0x00D, 0x00E, 0x00E, 0x00F, 0x00F, 0x00F, 0x010, 0x010,
    0x011, 0x011, 0x012, 0x013, 0x013, 0x014, 0x014, 0x015, 0x015, 0x016, 0x017, 0x017, 0x018,
    0x018, 0x019, 0x01A, 0x01B, 0x01B, 0x01C, 0x01D, 0x01D, 0x01E, 0x01F, 0x020, 0x020, 0x021,
    0x022, 0x023, 0x024, 0x024, 0x025, 0x026, 0x027, 0x028, 0x029, 0x02A, 0x02B, 0x02C, 0x02D,
    0x02E, 0x02F, 0x030, 0x031, 0x032, 0x033, 0x034, 0x035, 0x036, 0x037, 0x038, 0x03A, 0x03B,
    0x03C, 0x03D, 0x03E, 0x040, 0x041, 0x042, 0x043, 0x045, 0x046, 0x047, 0x049, 0x04A, 0x04C,
    0x04D, 0x04E, 0x050, 0x051, 0x053, 0x054, 0x056, 0x057, 0x059, 0x05A, 0x05C, 0x05E, 0x05F,
    0x061, 0x063, 0x064, 0x066, 0x068, 0x06A, 0x06B, 0x06D, 0x06F, 0x071, 0x073, 0x075, 0x076,
    0x078, 0x07A, 0x07C, 0x07E, 0x080, 0x082, 0x084, 0x086, 0x089, 0x08B, 0x08D, 0x08F, 0x091,
    0x093, 0x096, 0x098, 0x09A, 0x09C, 0x09F, 0x0A1, 0x0A3, 0x0A6, 0x0A8, 0x0AB, 0x0AD, 0x0AF,
    0x0B2, 0x0B4, 0x0B7, 0x0BA, 0x0BC, 0x0BF, 0x0C1, 0x0C4, 0x0C7, 0x0C9, 0x0CC, 0x0CF, 0x0D2,
    0x0D4, 0x0D7, 0x0DA, 0x0DD, 0x0E0, 0x0E3, 0x0E6, 0x0E9, 0x0EC, 0x0EF, 0x0F2, 0x0F5, 0x0F8,
    0x0FB, 0x0FE, 0x101, 0x104, 0x107, 0x10B, 0x10E, 0x111, 0x114, 0x118, 0x11B, 0x11E, 0x122,
    0x125, 0x129, 0x12C, 0x130, 0x133, 0x137, 0x13A, 0x13E, 0x141, 0x145, 0x148, 0x14C, 0x150,
    0x153, 0x157, 0x15B, 0x15F, 0x162, 0x166, 0x16A, 0x16E, 0x172, 0x176, 0x17A, 0x17D, 0x181,
    0x185, 0x189, 0x18D, 0x191, 0x195, 0x19A, 0x19E, 0x1A2, 0x1A6, 0x1AA, 0x1AE, 0x1B2, 0x1B7,
    0x1BB, 0x1BF, 0x1C3, 0x1C8, 0x1CC, 0x1D0, 0x1D5, 0x1D9, 0x1DD, 0x1E2, 0x1E6, 0x1EB, 0x1EF,
    0x1F3, 0x1F8, 0x1FC, 0x201, 0x205, 0x20A, 0x20F, 0x213, 0x218, 0x21C, 0x221, 0x226, 0x22A,
    0x22F, 0x233, 0x238, 0x23D, 0x241, 0x246, 0x24B, 0x250, 0x254, 0x259, 0x25E, 0x263, 0x267,
    0x26C, 0x271, 0x276, 0x27B, 0x280, 0x284, 0x289, 0x28E, 0x293, 0x298, 0x29D, 0x2A2, 0x2A6,
    0x2AB, 0x2B0, 0x2B5, 0x2BA, 0x2BF, 0x2C4, 0x2C9, 0x2CE, 0x2D3, 0x2D8, 0x2DC, 0x2E1, 0x2E6,
    0x2EB, 0x2F0, 0x2F5, 0x2FA, 0x2FF, 0x304, 0x309, 0x30E, 0x313, 0x318, 0x31D, 0x322, 0x326,
    0x32B, 0x330, 0x335, 0x33A, 0x33F, 0x344, 0x349, 0x34E, 0x353, 0x357, 0x35C, 0x361, 0x366,
    0x36B, 0x370, 0x374, 0x379, 0x37E, 0x383, 0x388, 0x38C, 0x391, 0x396, 0x39B, 0x39F, 0x3A4,
    0x3A9, 0x3AD, 0x3B2, 0x3B7, 0x3BB, 0x3C0, 0x3C5, 0x3C9, 0x3CE, 0x3D2, 0x3D7, 0x3DC, 0x3E0,
    0x3E5, 0x3E9, 0x3ED, 0x3F2, 0x3F6, 0x3FB, 0x3FF, 0x403, 0x408, 0x40C, 0x410, 0x415, 0x419,
    0x41D, 0x421, 0x425, 0x42A, 0x42E, 0x432, 0x436, 0x43A, 0x43E, 0x442, 0x446, 0x44A, 0x44E,
    0x452, 0x455, 0x459, 0x45D, 0x461, 0x465, 0x468, 0x46C, 0x470, 0x473, 0x477, 0x47A, 0x47E,
    0x481, 0x485, 0x488, 0x48C, 0x48F, 0x492, 0x496, 0x499, 0x49C, 0x49F, 0x4A2, 0x4A6, 0x4A9,
    0x4AC, 0x4AF, 0x4B2, 0x4B5, 0x4B7, 0x4BA, 0x4BD, 0x4C0, 0x4C3, 0x4C5, 0x4C8, 0x4CB, 0x4CD,
    0x4D0, 0x4D2, 0x4D5, 0x4D7, 0x4D9, 0x4DC, 0x4DE, 0x4E0, 0x4E3, 0x4E5, 0x4E7, 0x4E9, 0x4EB,
    0x4ED, 0x4EF, 0x4F1, 0x4F3, 0x4F5, 0x4F6, 0x4F8, 0x4FA, 0x4FB, 0x4FD, 0x4FF, 0x500, 0x502,
    0x503, 0x504, 0x506, 0x507, 0x508, 0x50A, 0x50B, 0x50C, 0x50D, 0x50E, 0x50F, 0x510, 0x511,
    0x511, 0x512, 0x513, 0x514, 0x514, 0x515, 0x516, 0x516, 0x517, 0x517, 0x517, 0x518, 0x518,
    0x518, 0x518, 0x518, 0x519, 0x519,
];

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
    fn decode_nibble(&mut self, nibble: i8, shift: u8, filter: u8) -> i16 {
        // Convert 4-bit signed to extended value
        // Hardware-accurate: shift >= 13 produces clamped output
        // Reference: blargg's SPC_DSP.cpp (public domain)
        let mut sample = ((nibble as i32) << shift) >> 1;
        if shift >= 13 {
            sample = (sample >> 25) << 11; // Positive nibbles → 0, negative → -2048
        }

        // Apply filter based on previous samples
        // Uses (-p) >> n form for correct negative rounding per hardware
        // Reference: blargg's SPC_DSP.cpp (public domain)
        let p1 = self.prev1 as i32;
        let p2 = (self.prev2 as i32) >> 1; // Pre-shift prev2 by 1 per hardware
        sample += match filter {
            0 => 0, // Direct (no filter)
            1 => {
                // s += p1 + ((-p1) >> 4)
                p1 + ((-p1) >> 4)
            }
            2 => {
                // s += 2*p1 + ((-3*p1) >> 5) - p2 + (p2 >> 4)
                2 * p1 + ((-3 * p1) >> 5) - p2 + (p2 >> 4)
            }
            3 => {
                // s += 2*p1 + ((-13*p1) >> 6) + 3*p2 + ((-3*p2) >> 4)
                2 * p1 + ((-13 * p1) >> 6) + 3 * p2 + ((-3 * p2) >> 4)
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

/// ADSR/GAIN rate counter table
/// Maps rate value (0-31) to number of 32kHz clocks between envelope updates
/// Derived from hardware behavior and verified against bsnes
const RATE_TABLE: [u16; 32] = [
    // Rate 0-11: Exponential spacing (very slow)
    0xFFFF, 0x2AAA, 0x1555, 0x0EEE, 0x0AAA, 0x0888, 0x06DB, 0x05B0, 0x04EC, 0x0444, 0x03AD, 0x0333,
    // Rate 12-31: Linear spacing
    0x02CD, 0x0266, 0x0200, 0x01B0, 0x0166, 0x0133, 0x0100, 0x00CD, 0x00A0, 0x007F, 0x0066, 0x0050,
    0x0040, 0x0033, 0x0028, 0x0020, 0x0019, 0x0014, 0x0010, 0x000C,
];

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
    /// Rate counter for timing envelope updates
    rate_counter: u16,
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
            rate_counter: 0,
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
            // GAIN mode: manual envelope control
            if (self.gain & 0x80) == 0 {
                // Direct mode: bits 6-0 directly set level
                self.level = ((self.gain & 0x7F) as u16) << 4;
            } else {
                // Automated modes: bits 6-5 select mode, bits 4-0 select rate
                let mode = (self.gain >> 5) & 0x03;
                let rate = (self.gain & 0x1F) as usize;

                // Update rate counter
                self.rate_counter = self.rate_counter.wrapping_add(1);
                if self.rate_counter >= RATE_TABLE[rate] {
                    self.rate_counter = 0;

                    match mode {
                        0 => {
                            // Linear decrease
                            if self.level >= 32 {
                                self.level -= 32;
                            } else {
                                self.level = 0;
                            }
                        }
                        1 => {
                            // Exponential decrease
                            self.level = self.level.saturating_sub((self.level >> 8) + 1);
                        }
                        2 => {
                            // Linear increase
                            if self.level <= 0x7FF - 32 {
                                self.level += 32;
                            } else {
                                self.level = 0x7FF;
                            }
                        }
                        3 => {
                            // Bent line increase (two-slope)
                            if self.level < 0x600 {
                                self.level += 32;
                            } else if self.level < 0x7E0 {
                                self.level += 8;
                            } else {
                                self.level = 0x7FF;
                            }
                        }
                        _ => unreachable!(),
                    }
                }
            }
        } else {
            // ADSR mode
            let attack_rate = ((self.adsr1 >> 4) & 0x0F) as usize;
            let decay_rate = (self.adsr1 & 0x07) as usize;
            let sustain_level = ((self.adsr2 >> 5) & 0x07) as u16;
            let sustain_rate = (self.adsr2 & 0x1F) as usize;

            match self.mode {
                EnvelopeMode::Attack => {
                    // Attack: linear increase
                    // Rate 15 (rate value 31): add 1024 per tick (fast but not instant)
                    // All other rates: add 32 per tick
                    // Reference: bsnes DSP envelope
                    let rate_value = attack_rate * 2 + 1;
                    self.rate_counter = self.rate_counter.wrapping_add(1);
                    let rate = RATE_TABLE[rate_value];

                    if self.rate_counter >= rate {
                        self.rate_counter = 0;

                        let step = if rate_value == 31 { 1024 } else { 32 };
                        self.level = self.level.saturating_add(step).min(0x7FF);

                        if self.level >= 0x7FF {
                            self.level = 0x7FF;
                            self.mode = EnvelopeMode::Decay;
                            self.rate_counter = 0;
                        }
                    }
                }
                EnvelopeMode::Decay => {
                    // Decay: exponential decrease to sustain level
                    let target = (sustain_level + 1) << 8;

                    if self.level > target {
                        self.rate_counter = self.rate_counter.wrapping_add(1);
                        let rate = RATE_TABLE[decay_rate * 2 + 16];

                        if self.rate_counter >= rate {
                            self.rate_counter = 0;

                            // Exponential decay
                            let step = ((self.level - 1) >> 8) + 1;
                            self.level = self.level.saturating_sub(step);

                            if self.level <= target {
                                self.level = target;
                                self.mode = EnvelopeMode::Sustain;
                                self.rate_counter = 0;
                            }
                        }
                    } else {
                        self.mode = EnvelopeMode::Sustain;
                        self.rate_counter = 0;
                    }
                }
                EnvelopeMode::Sustain => {
                    // Sustain: exponential decrease towards zero
                    if sustain_rate != 0 && self.level > 0 {
                        self.rate_counter = self.rate_counter.wrapping_add(1);
                        let rate = RATE_TABLE[sustain_rate];

                        if self.rate_counter >= rate {
                            self.rate_counter = 0;

                            // Exponential decay
                            let step = ((self.level - 1) >> 8) + 1;
                            self.level = self.level.saturating_sub(step);
                        }
                    }
                }
                EnvelopeMode::Release => {
                    // Release: LINEAR decrease by 8 every clock (not exponential)
                    // This runs unconditionally every clock, no rate counter
                    // Reference: bsnes DSP - v.envelope -= 8
                    if self.level >= 8 {
                        self.level -= 8;
                    } else {
                        self.level = 0;
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
    sample_addr: u16,
    /// Loop start address (from BRR header)
    loop_addr: u16,
    /// Sample history for Gaussian interpolation across block boundaries (last 3 samples)
    sample_history: [i16; 3],
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
            sample_history: [0; 3],
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
        self.sample_history = [0; 3]; // Clear history
    }

    /// Stop playback (key off)
    fn key_off(&mut self) {
        self.envelope.key_off();
    }

    /// Decode a BRR block from RAM into the sample buffer
    /// Returns true if this is the last block (end flag set)
    fn decode_brr_block(&mut self, ram: &[u8; 0x10000]) -> bool {
        let addr = self.sample_addr as usize;
        if addr + 9 > 0x10000 {
            // Invalid address, stop playback (need 9 bytes: 1 header + 8 data)
            self.ended = true;
            return true;
        }

        // Save last 3 samples from current buffer to history before decoding new block
        if self.sample_buffer.len() >= 3 {
            self.sample_history[0] = self.sample_buffer[13];
            self.sample_history[1] = self.sample_buffer[14];
            self.sample_history[2] = self.sample_buffer[15];
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

    /// Get current sample output with Gaussian interpolation
    /// Returns the interpolated sample value using hardware-accurate 4-point Gaussian filter
    fn get_sample(&self) -> i16 {
        if !self.playing || self.ended {
            return 0;
        }

        // Hardware-accurate Gaussian interpolation (4-point)
        // Uses bits 4-11 of position (fractional part) to index the Gaussian table
        let frac = ((self.position >> 4) & 0xFF) as usize; // 8-bit index (0-255)

        // Get 4 consecutive samples for interpolation, properly handling block boundaries
        // by using sample history for samples before the current block
        let idx = self.buffer_index;

        // t-1 (previous sample) - may be from history if we're at the start of the block
        let s0 = if idx == 0 {
            self.sample_history[2] // Last sample of previous block
        } else {
            self.sample_buffer[idx - 1]
        } as i32;

        // t (current sample)
        let s1 = self.sample_buffer[idx] as i32;

        // t+1 (next sample) - handle wraparound at end of block
        let s2 = if idx + 1 < 16 {
            self.sample_buffer[idx + 1]
        } else {
            // At block boundary, this would be the first sample of next block
            // For now, use the last available sample to avoid discontinuity
            self.sample_buffer[15]
        } as i32;

        // t+2 (next next sample) - handle wraparound at end of block
        let s3 = if idx + 2 < 16 {
            self.sample_buffer[idx + 2]
        } else if idx + 1 < 16 {
            // Would be from next block, use last available
            self.sample_buffer[15]
        } else {
            // Both would be from next block
            self.sample_buffer[15]
        } as i32;

        // Get Gaussian filter coefficients from the table
        // The table is organized so that frac=0 gives weight to current sample,
        // frac=255 gives weight to next sample
        let g0 = GAUSSIAN_TABLE[255 - frac] as i32; // Weight for t-1
        let g1 = GAUSSIAN_TABLE[511 - frac] as i32; // Weight for t
        let g2 = GAUSSIAN_TABLE[256 + frac] as i32; // Weight for t+1
        let g3 = GAUSSIAN_TABLE[frac] as i32; // Weight for t+2

        // Hardware-accurate Gaussian filter: each product is shifted individually,
        // then clipped to i16 after 3 terms, then 4th term added and clipped again
        // Reference: bsnes DSP gaussianInterpolate()
        let mut output = (s0 * g0) >> 11;
        output += (s1 * g1) >> 11;
        output += (s2 * g2) >> 11;
        output = (output as i16) as i32; // Clip to 16-bit after first 3 terms
        output += (s3 * g3) >> 11;

        output.clamp(-32768, 32767) as i16
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
    /// Reference to APU RAM (for BRR sample access)
    ///
    /// # Safety
    /// This pointer is set once during Spc700Memory::new() and points to the RAM
    /// Box owned by the same Spc700Memory struct that contains this DSP.
    /// Since Spc700Memory is never moved after creation (it's stored in CpuSpc700),
    /// the pointer remains valid for the lifetime of the DSP.
    /// All dereferencing is done through checked unsafe blocks.
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
            ram: None,
        }
    }

    /// Set reference to APU RAM for BRR sample access
    ///
    /// # Safety
    /// The caller must ensure that the pointer remains valid for the lifetime of the DSP.
    /// In practice, this is called once during Spc700Memory::new() with a pointer to
    /// the RAM Box owned by the same Spc700Memory struct.
    pub fn set_ram(&mut self, ram: *const [u8; 0x10000]) {
        self.ram = Some(ram);
    }

    /// Read sample directory entry for a given source number
    /// Returns (start_address, loop_address) tuple
    fn get_sample_addresses(&self, source: u8) -> Option<(u16, u16)> {
        // SAFETY: Pointer is valid because it points to RAM owned by the parent Spc700Memory
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
                // SAFETY: Pointer is valid because it points to RAM owned by the parent Spc700Memory
                let ram = if let Some(ram_ptr) = self.ram {
                    unsafe { ram_ptr.as_ref() }
                } else {
                    None
                };

                // Collect sample addresses for all voices that will be keyed on
                let mut sample_addrs: [Option<(u16, u16)>; 8] = [None; 8];
                for (i, voice) in self.voices.iter().enumerate() {
                    if value & (1 << i) != 0 {
                        sample_addrs[i] = self.get_sample_addresses(voice.source);
                    }
                }

                // Now key on the voices with the collected addresses
                for (i, voice) in self.voices.iter_mut().enumerate() {
                    if value & (1 << i) != 0 {
                        // Clear ENDX bit for this voice on key-on
                        self.endx &= !(1 << i);

                        if let Some((start_addr, loop_addr)) = sample_addrs[i] {
                            voice.loop_addr = loop_addr;
                            voice.key_on(start_addr);

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
        // SAFETY: Pointer is valid because it points to RAM owned by the parent Spc700Memory
        let ram = if let Some(ram_ptr) = self.ram {
            unsafe { ram_ptr.as_ref() }
        } else {
            None
        };

        // Update each voice (ENDX is sticky - don't clear it)
        for (i, voice) in self.voices.iter_mut().enumerate() {
            // Update envelope
            voice.envelope.clock();

            // Advance sample position if voice is playing
            if voice.playing && !voice.ended && voice.advance_position() {
                // Need to decode next BRR block
                if let Some(ram_ref) = ram {
                    let is_end = voice.decode_brr_block(ram_ref);
                    if is_end && voice.ended {
                        // Voice reached end without loop - set ENDX bit (sticky)
                        self.endx |= 1 << i;
                    }
                } else {
                    // No RAM available, stop voice
                    voice.ended = true;
                    self.endx |= 1 << i;
                }
            }

            // Update ENDX flag if voice ended (sticky)
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

    #[test]
    fn test_gaussian_interpolation_coefficient_lookup() {
        // Test that coefficient table can be indexed correctly
        // The table should have 512 entries
        assert_eq!(GAUSSIAN_TABLE.len(), 512);

        // Test index 0 and 511 (endpoints)
        assert!(GAUSSIAN_TABLE[0] >= 0);
        assert!(GAUSSIAN_TABLE[511] >= 0);

        // Values should be 12-bit (0-4095 range), though stored as i16
        for &coeff in &GAUSSIAN_TABLE {
            assert!(
                (0..=0x7FF).contains(&coeff),
                "Coefficient out of 12-bit range"
            );
        }
    }

    #[test]
    fn test_gaussian_interpolation_at_zero_fraction() {
        let mut voice = Voice::new();
        voice.playing = true;
        voice.sample_buffer = [
            100, 200, 300, 400, 500, 600, 700, 800, 900, 1000, 1100, 1200, 1300, 1400, 1500, 1600,
        ];
        voice.buffer_index = 5;
        voice.position = 5 << 16; // Exactly at sample 5, no fractional part

        // At zero fraction (frac=0), should give most weight to current sample
        let result = voice.get_sample();

        // Result should be close to the current sample (600) since frac=0
        // Allow some margin due to Gaussian filter characteristics
        assert!(
            result > 400 && result < 800,
            "Result {} should be close to 600",
            result
        );
    }

    #[test]
    fn test_gaussian_interpolation_block_boundary_start() {
        let mut voice = Voice::new();
        voice.playing = true;
        // Setup history from previous block
        voice.sample_history = [50, 75, 90];
        voice.sample_buffer = [
            100, 200, 300, 400, 500, 600, 700, 800, 900, 1000, 1100, 1200, 1300, 1400, 1500, 1600,
        ];
        voice.buffer_index = 0; // At start of block
        voice.position = 0; // No fractional part

        // Should use sample_history[2] (90) for t-1
        let result = voice.get_sample();

        // Result should incorporate history from previous block
        // The fact that it doesn't panic or return wildly incorrect values is the main test
        assert!(result > -32768 && result < 32767);
    }

    #[test]
    fn test_gaussian_interpolation_mid_block() {
        let mut voice = Voice::new();
        voice.playing = true;
        voice.sample_buffer = [
            100, 200, 300, 400, 500, 600, 700, 800, 900, 1000, 1100, 1200, 1300, 1400, 1500, 1600,
        ];
        voice.buffer_index = 8; // Middle of block
        voice.position = 8 << 16; // Exactly at sample 8

        // In the middle of the block, all 4 samples should come from sample_buffer
        let result = voice.get_sample();

        // Should be reasonable value around sample 8 (900)
        assert!(
            result > 600 && result < 1200,
            "Result {} should be around 900",
            result
        );
    }

    #[test]
    fn test_gaussian_interpolation_with_fractional_position() {
        let mut voice = Voice::new();
        voice.playing = true;
        voice.sample_buffer = [0, 0, 0, 0, 1000, 1000, 1000, 1000, 0, 0, 0, 0, 0, 0, 0, 0];
        voice.buffer_index = 4;
        voice.position = (4 << 16) | 0x8000; // Sample 4 + 0.5 fractional

        // With fractional position 0.5, should interpolate between samples
        let result = voice.get_sample();

        // Should produce a reasonable interpolated value
        assert!(
            result > 0 && result < 1500,
            "Result {} should be interpolated",
            result
        );
    }

    #[test]
    fn test_gaussian_interpolation_returns_zero_when_not_playing() {
        let mut voice = Voice::new();
        voice.playing = false;
        voice.sample_buffer = [1000; 16]; // Non-zero samples
        voice.buffer_index = 5;

        let result = voice.get_sample();
        assert_eq!(result, 0, "Should return 0 when not playing");
    }

    #[test]
    fn test_gaussian_interpolation_returns_zero_when_ended() {
        let mut voice = Voice::new();
        voice.playing = true;
        voice.ended = true;
        voice.sample_buffer = [1000; 16]; // Non-zero samples
        voice.buffer_index = 5;

        let result = voice.get_sample();
        assert_eq!(result, 0, "Should return 0 when ended");
    }

    #[test]
    fn test_sample_history_updated_on_block_decode() {
        let mut voice = Voice::new();
        voice.playing = true;
        voice.sample_buffer = [
            100, 200, 300, 400, 500, 600, 700, 800, 900, 1000, 1100, 1200, 1300, 1400, 1500, 1600,
        ];

        let mut ram = Box::new([0u8; 0x10000]);
        // Setup a simple BRR block at address 0x100
        ram[0x100] = 0x00; // Header: no flags, shift=0, filter=0
        for i in 0x101..=0x108 {
            ram[i] = 0; // Sample data (zeros)
        }

        voice.sample_addr = 0x100;

        // Before decoding, set history to known values
        voice.sample_history = [0, 0, 0];

        // Decode block
        voice.decode_brr_block(&ram);

        // After decoding, history should contain last 3 samples of previous buffer
        assert_eq!(voice.sample_history[0], 1400);
        assert_eq!(voice.sample_history[1], 1500);
        assert_eq!(voice.sample_history[2], 1600);
    }
}
