//! SN76489 PSG (Programmable Sound Generator)
//!
//! Identical to SMS PSG: 3 square wave channels + 1 noise channel.
//! Integrated into the VDP on the Mega Drive.
//! Accessible via port $C00011 on the 68K bus.

/// PSG sample rate
const SAMPLE_RATE: u32 = 44100;
/// PSG clock (NTSC): master clock / 15 ≈ 223721 Hz
const PSG_CLOCK: u32 = 223721;

/// SN76489 PSG
pub struct Psg {
    /// Tone registers (10-bit, channels 0-2)
    tone: [u16; 3],
    /// Volume registers (4-bit attenuation, channels 0-3)
    volume: [u8; 4],
    /// Noise register
    noise_reg: u8,
    /// Noise shift register (16-bit LFSR)
    noise_lfsr: u16,
    /// Current channel counters
    counters: [u16; 4],
    /// Current output polarity
    polarity: [bool; 4],
    /// Latched channel/type for writes
    latched_channel: usize,
    latched_type: bool, // false = tone, true = volume

    /// Sample accumulator
    sample_counter: f64,
}

impl Psg {
    pub fn new() -> Self {
        Self {
            tone: [0; 3],
            volume: [0x0F; 4], // All channels muted
            noise_reg: 0,
            noise_lfsr: 0x8000,
            counters: [0; 4],
            polarity: [false; 4],
            latched_channel: 0,
            latched_type: false,
            sample_counter: 0.0,
        }
    }

    pub fn reset(&mut self) {
        self.tone = [0; 3];
        self.volume = [0x0F; 4];
        self.noise_reg = 0;
        self.noise_lfsr = 0x8000;
        self.counters = [0; 4];
        self.polarity = [false; 4];
        self.latched_channel = 0;
        self.latched_type = false;
        self.sample_counter = 0.0;
    }

    /// Write to PSG port
    pub fn write(&mut self, val: u8) {
        if val & 0x80 != 0 {
            // Latch/data byte
            self.latched_channel = ((val >> 5) & 0x03) as usize;
            self.latched_type = val & 0x10 != 0;

            if self.latched_type {
                // Volume
                self.volume[self.latched_channel] = val & 0x0F;
            } else if self.latched_channel < 3 {
                // Tone low 4 bits
                self.tone[self.latched_channel] =
                    (self.tone[self.latched_channel] & 0x3F0) | (val & 0x0F) as u16;
            } else {
                // Noise control
                self.noise_reg = val & 0x07;
                self.noise_lfsr = 0x8000;
            }
        } else {
            // Data byte (second half)
            if self.latched_type {
                self.volume[self.latched_channel] = val & 0x0F;
            } else if self.latched_channel < 3 {
                // Tone high 6 bits
                self.tone[self.latched_channel] =
                    (self.tone[self.latched_channel] & 0x00F) | ((val as u16 & 0x3F) << 4);
            } else {
                self.noise_reg = val & 0x07;
                self.noise_lfsr = 0x8000;
            }
        }
    }

    /// Clock the PSG one step
    fn clock(&mut self) {
        // Update tone channels
        for ch in 0..3 {
            if self.counters[ch] == 0 {
                self.counters[ch] = self.tone[ch];
                self.polarity[ch] = !self.polarity[ch];
            } else {
                self.counters[ch] -= 1;
            }
        }

        // Update noise channel
        if self.counters[3] == 0 {
            let period = match self.noise_reg & 0x03 {
                0 => 0x10,
                1 => 0x20,
                2 => 0x40,
                3 => self.tone[2], // Channel 2 frequency
                _ => unreachable!(),
            };
            self.counters[3] = period;
            self.polarity[3] = !self.polarity[3];

            if self.polarity[3] {
                // Clock LFSR
                let feedback = if self.noise_reg & 0x04 != 0 {
                    // White noise
                    let bit0 = self.noise_lfsr & 1;
                    let bit3 = (self.noise_lfsr >> 3) & 1;
                    bit0 ^ bit3
                } else {
                    // Periodic noise
                    self.noise_lfsr & 1
                };
                self.noise_lfsr = (self.noise_lfsr >> 1) | (feedback << 15);
            }
        } else {
            self.counters[3] -= 1;
        }
    }

    /// Get current sample value
    fn get_sample(&self) -> i16 {
        let mut output: i32 = 0;

        // Volume table (4-bit attenuation to amplitude)
        let vol_table: [i32; 16] = [
            8191, 6507, 5168, 4105, 3261, 2590, 2057, 1634, 1298, 1031, 819, 650, 516, 410, 326, 0,
        ];

        // Mix tone channels
        for ch in 0..3 {
            if self.polarity[ch] {
                output += vol_table[self.volume[ch] as usize];
            } else {
                output -= vol_table[self.volume[ch] as usize];
            }
        }

        // Mix noise channel
        let noise_out = if self.noise_lfsr & 1 != 0 { 1 } else { -1 };
        output += noise_out * vol_table[self.volume[3] as usize];

        // Scale down and clamp
        let sample = output / 4;
        sample.clamp(-32768, 32767) as i16
    }

    /// Generate audio samples (mono)
    pub fn generate_samples(&mut self, count: usize) -> Vec<i16> {
        let mut samples = Vec::with_capacity(count);
        let step = PSG_CLOCK as f64 / SAMPLE_RATE as f64;

        for _ in 0..count {
            self.sample_counter += step;
            while self.sample_counter >= 1.0 {
                self.sample_counter -= 1.0;
                self.clock();
            }
            samples.push(self.get_sample());
        }

        samples
    }

    // ── Save State ──────────────────────────────────────────────

    pub fn get_state(&self) -> serde_json::Value {
        serde_json::json!({
            "tone": self.tone.to_vec(),
            "volume": self.volume.to_vec(),
            "noise_reg": self.noise_reg,
            "noise_lfsr": self.noise_lfsr,
        })
    }

    pub fn set_state(&mut self, state: &serde_json::Value) -> Result<(), serde_json::Error> {
        if let Some(tone) = state.get("tone").and_then(|v| v.as_array()) {
            for (i, val) in tone.iter().enumerate() {
                if i < 3 {
                    self.tone[i] = val.as_u64().unwrap_or(0) as u16;
                }
            }
        }
        if let Some(vol) = state.get("volume").and_then(|v| v.as_array()) {
            for (i, val) in vol.iter().enumerate() {
                if i < 4 {
                    self.volume[i] = val.as_u64().unwrap_or(0) as u8;
                }
            }
        }
        if let Some(v) = state.get("noise_reg").and_then(|v| v.as_u64()) {
            self.noise_reg = v as u8;
        }
        if let Some(v) = state.get("noise_lfsr").and_then(|v| v.as_u64()) {
            self.noise_lfsr = v as u16;
        }
        Ok(())
    }
}
