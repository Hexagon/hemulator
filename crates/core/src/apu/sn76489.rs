//! Texas Instruments SN76489 Programmable Sound Generator
//!
//! The SN76489 is a sound chip used in the Sega Master System, Game Gear,
//! and many other systems.
//!
//! # Architecture
//! - 3 square wave tone channels
//! - 1 noise channel
//! - 4-bit volume control per channel
//! - 10-bit frequency control for tone channels
//!
//! # Clock Prescaler
//! The SN76489 contains an internal ÷16 clock prescaler. All tone and noise
//! counters are clocked at `CLK/16`, not at the raw input clock rate. Given
//! a tone register value of `N`, the counter and toggle logic produce a
//! fundamental tone frequency of `f = CLK / (32 × (N + 1))` (÷16 prescaler
//! × ÷2 flip-flop × (N + 1) counter ticks). When emulating, `clock()` is
//! called at the full CPU clock rate and the prescaler fires every 16th call.
//! # Sega Variant (SN76496)
//! The Sega variant uses a 16-bit LFSR for noise (instead of 15-bit)

use crate::apu::{AudioChip, TimingMode};

/// SN76489 PSG state
pub struct Sn76489Psg {
    // Tone generators
    tone_freq: [u16; 3],    // 10-bit frequency values
    tone_counter: [u16; 3], // Current counter values
    tone_output: [bool; 3], // Current output state

    // Noise generator
    noise_control: u8,  // Noise control register
    noise_lfsr: u16,    // Linear feedback shift register (16-bit for Sega)
    noise_counter: u16, // Noise counter
    noise_output: bool, // Current noise output

    // Volume control (4-bit, 0=max, 15=min/mute)
    volume: [u8; 4],

    // Latched register
    latched_reg: u8,

    // Clock rate and timing
    timing_mode: TimingMode,

    // Internal ÷16 clock prescaler.
    // The SN76489 divides its input clock by 16 before feeding the tone/noise
    // counters, so `clock_once` only advances those counters every 16th call.
    prescaler: u8,
}

impl Sn76489Psg {
    /// Create a new SN76489 PSG
    ///
    /// # Arguments
    /// * `timing_mode` - NTSC or PAL timing mode
    pub fn new(timing_mode: TimingMode) -> Self {
        Self {
            tone_freq: [0; 3],
            tone_counter: [0; 3],
            tone_output: [false; 3],
            noise_control: 0,
            noise_lfsr: 0x8000, // Initial LFSR state
            noise_counter: 0,
            noise_output: false,
            volume: [0x0F; 4], // All channels muted initially
            latched_reg: 0,
            timing_mode,
            prescaler: 0,
        }
    }

    /// Write a byte to the PSG
    pub fn write(&mut self, data: u8) {
        if data & 0x80 != 0 {
            // Latch/data byte: bits 6-5 = channel, bit 4 = type (0=tone/noise, 1=volume)
            let channel = (data >> 5) & 0x03;
            let is_volume = (data >> 4) & 0x01;

            // Store full register identifier: (channel << 1) | is_volume
            self.latched_reg = (channel << 1) | is_volume;

            if is_volume != 0 {
                // Volume write
                self.volume[channel as usize] = data & 0x0F;
            } else if channel == 3 {
                // Noise control
                self.noise_control = data & 0x07;
                self.noise_lfsr = 0x8000; // Reset LFSR
            } else {
                // Tone frequency (low 4 bits)
                let ch = channel as usize;
                self.tone_freq[ch] = (self.tone_freq[ch] & 0x3F0) | ((data & 0x0F) as u16);
            }
        } else {
            // Data byte (continuation of previous latch)
            let channel = self.latched_reg >> 1;
            let is_volume = self.latched_reg & 1;

            if is_volume != 0 {
                // Volume update
                self.volume[channel as usize] = data & 0x0F;
            } else if channel == 3 {
                // Noise control update
                self.noise_control = data & 0x07;
                self.noise_lfsr = 0x8000;
            } else {
                // Tone frequency (high 6 bits)
                let ch = channel as usize;
                self.tone_freq[ch] = (self.tone_freq[ch] & 0x00F) | (((data & 0x3F) as u16) << 4);
            }
        }
    }

    /// Clock the PSG and generate samples
    fn clock_once(&mut self) {
        // The SN76489 has an internal ÷16 prescaler: tone and noise counters
        // are only updated every 16 input clock cycles.
        self.prescaler = (self.prescaler + 1) % 16;
        if self.prescaler != 0 {
            return;
        }

        // Clock tone generators
        for i in 0..3 {
            if self.tone_counter[i] > 0 {
                self.tone_counter[i] -= 1;
            } else {
                // Reload counter
                self.tone_counter[i] = self.tone_freq[i];
                if self.tone_freq[i] > 0 {
                    self.tone_output[i] = !self.tone_output[i];
                }
            }
        }

        // Clock noise generator
        if self.noise_counter > 0 {
            self.noise_counter -= 1;
        } else {
            // Reload noise counter based on control register
            let noise_rate = self.noise_control & 0x03;
            self.noise_counter = match noise_rate {
                0 => 0x10,
                1 => 0x20,
                2 => 0x40,
                3 => self.tone_freq[2], // Use tone 2 frequency
                _ => unreachable!(),
            };

            // Clock LFSR
            let feedback = if (self.noise_control & 0x04) != 0 {
                // White noise - Sega variant: tapped at bits 0 and 3 (16-bit LFSR)
                ((self.noise_lfsr & 1) ^ ((self.noise_lfsr >> 3) & 1)) != 0
            } else {
                // Periodic noise
                (self.noise_lfsr & 1) != 0
            };

            self.noise_lfsr >>= 1;
            if feedback {
                self.noise_lfsr |= 0x8000;
            }

            self.noise_output = (self.noise_lfsr & 1) != 0;
        }
    }

    /// Generate a single audio sample
    fn generate_sample(&self) -> i16 {
        let mut output = 0.0;

        // Mix tone channels
        for i in 0..3 {
            let amplitude = self.volume_to_amplitude(self.volume[i]);
            output += if self.tone_output[i] {
                amplitude
            } else {
                -amplitude
            };
        }

        // Mix noise channel
        let noise_amplitude = self.volume_to_amplitude(self.volume[3]);
        output += if self.noise_output {
            noise_amplitude
        } else {
            -noise_amplitude
        };

        // Average, normalize, and convert to i16
        let normalized = output / 4.0;
        (normalized * 32767.0) as i16
    }

    /// Convert 4-bit volume to amplitude (0=max, 15=min)
    fn volume_to_amplitude(&self, volume: u8) -> f32 {
        if volume == 0x0F {
            0.0 // Muted
        } else {
            // Exponential volume curve (approximately -2dB per step)
            let attenuation = volume as f32 * 2.0;
            10_f32.powf(-attenuation / 20.0)
        }
    }

    /// Reset the PSG to initial state
    pub fn reset_state(&mut self) {
        self.tone_freq.fill(0);
        self.tone_counter.fill(0);
        self.tone_output.fill(false);
        self.noise_control = 0;
        self.noise_lfsr = 0x8000;
        self.noise_counter = 0;
        self.noise_output = false;
        self.volume.fill(0x0F);
        self.latched_reg = 0;
        self.prescaler = 0;
    }

    /// Get PSG state for save state
    pub fn get_state(&self) -> serde_json::Value {
        serde_json::json!({
            "tone_freq": self.tone_freq.to_vec(),
            "tone_counter": self.tone_counter.to_vec(),
            "tone_output": self.tone_output.to_vec(),
            "noise_control": self.noise_control,
            "noise_lfsr": self.noise_lfsr,
            "noise_counter": self.noise_counter,
            "noise_output": self.noise_output,
            "volume": self.volume.to_vec(),
            "latched_reg": self.latched_reg,
            "prescaler": self.prescaler,
            "timing_mode": match self.timing_mode {
                TimingMode::Ntsc | TimingMode::Gba => "ntsc",
                TimingMode::Pal => "pal",
            },
        })
    }

    /// Set PSG state from save state
    pub fn set_state(&mut self, state: &serde_json::Value) -> Result<(), serde_json::Error> {
        macro_rules! load_u8 {
            ($state:expr, $field:literal, $target:expr) => {
                if let Some(val) = $state.get($field).and_then(|v| v.as_u64()) {
                    $target = val as u8;
                }
            };
        }

        macro_rules! load_u16 {
            ($state:expr, $field:literal, $target:expr) => {
                if let Some(val) = $state.get($field).and_then(|v| v.as_u64()) {
                    $target = val as u16;
                }
            };
        }

        macro_rules! load_bool {
            ($state:expr, $field:literal, $target:expr) => {
                if let Some(val) = $state.get($field).and_then(|v| v.as_bool()) {
                    $target = val;
                }
            };
        }

        // Load tone frequencies
        if let Some(tone_freq) = state.get("tone_freq").and_then(|v| v.as_array()) {
            for (i, val) in tone_freq.iter().enumerate() {
                if i >= self.tone_freq.len() {
                    break;
                }
                if let Some(freq) = val.as_u64() {
                    self.tone_freq[i] = freq as u16;
                }
            }
        }

        // Load tone counters
        if let Some(tone_counter) = state.get("tone_counter").and_then(|v| v.as_array()) {
            for (i, val) in tone_counter.iter().enumerate() {
                if i >= self.tone_counter.len() {
                    break;
                }
                if let Some(counter) = val.as_u64() {
                    self.tone_counter[i] = counter as u16;
                }
            }
        }

        // Load tone outputs
        if let Some(tone_output) = state.get("tone_output").and_then(|v| v.as_array()) {
            for (i, val) in tone_output.iter().enumerate() {
                if i >= self.tone_output.len() {
                    break;
                }
                if let Some(output) = val.as_bool() {
                    self.tone_output[i] = output;
                }
            }
        }

        load_u8!(state, "noise_control", self.noise_control);
        load_u16!(state, "noise_lfsr", self.noise_lfsr);
        load_u16!(state, "noise_counter", self.noise_counter);
        load_bool!(state, "noise_output", self.noise_output);
        load_u8!(state, "prescaler", self.prescaler);
        // Clamp to valid range in case the save state was corrupted or hand-edited.
        self.prescaler = self.prescaler.min(15);

        // Load volumes
        if let Some(volume) = state.get("volume").and_then(|v| v.as_array()) {
            for (i, val) in volume.iter().enumerate() {
                if i >= self.volume.len() {
                    break;
                }
                if let Some(vol) = val.as_u64() {
                    self.volume[i] = vol as u8;
                }
            }
        }

        load_u8!(state, "latched_reg", self.latched_reg);

        if let Some(timing_str) = state.get("timing_mode").and_then(|v| v.as_str()) {
            self.timing_mode = match timing_str {
                "pal" => TimingMode::Pal,
                _ => TimingMode::Ntsc,
            };
        }

        Ok(())
    }
}

impl AudioChip for Sn76489Psg {
    fn write_register(&mut self, _addr: u16, val: u8) {
        // SMS writes to PSG via I/O port, not memory-mapped
        self.write(val);
    }

    fn clock(&mut self) -> i16 {
        self.clock_once();
        self.generate_sample()
    }

    fn timing(&self) -> TimingMode {
        self.timing_mode
    }

    fn reset(&mut self) {
        self.reset_state();
    }
}

/// Generic adapter wrapping [`Sn76489Psg`] with CPU-clock-rate-based sample generation.
///
/// The SN76489 is used in several systems that run at different CPU clock rates.
/// This adapter handles cycle accumulation and downsampling to 44.1 kHz so that
/// each system only needs to provide its NTSC and PAL CPU frequencies.
///
/// Used by the SMS (`SmsPsg`), SG-1000 (`Sg1000Psg`), and ColecoVision
/// (`ColecoVisionPsg`) to avoid duplicating the sample-generation loop.
pub struct Sn76489Adapter {
    psg: Sn76489Psg,
    cycle_accum: f64,
    timing: TimingMode,
    cpu_hz_ntsc: f64,
    cpu_hz_pal: f64,
}

impl Sn76489Adapter {
    /// Create a new adapter with the given initial timing mode and CPU clock rates.
    pub fn new(timing: TimingMode, cpu_hz_ntsc: f64, cpu_hz_pal: f64) -> Self {
        Self {
            psg: Sn76489Psg::new(timing),
            cycle_accum: 0.0,
            timing,
            cpu_hz_ntsc,
            cpu_hz_pal,
        }
    }

    /// Write a byte to the PSG.
    pub fn write(&mut self, data: u8) {
        self.psg.write(data);
    }

    /// Set timing mode (NTSC/PAL).
    ///
    /// Resets the cycle accumulator to prevent fractional-cycle drift when
    /// switching between clock rates, and propagates the new mode to the
    /// inner [`Sn76489Psg`] so that both fields stay in sync.
    pub fn set_timing(&mut self, timing: TimingMode) {
        if self.timing != timing {
            self.timing = timing;
            self.psg.timing_mode = timing;
            self.cycle_accum = 0.0;
        }
    }

    /// Reset the PSG and cycle accumulator to their initial states.
    pub fn reset(&mut self) {
        self.psg.reset();
        self.cycle_accum = 0.0;
    }

    /// Generate `sample_count` audio samples at 44.1 kHz by clocking the PSG at
    /// CPU speed and averaging the output over accumulated cycles.
    pub fn generate_samples(&mut self, sample_count: usize) -> Vec<i16> {
        const SAMPLE_HZ: f64 = 44_100.0;
        let cpu_hz = match self.timing {
            TimingMode::Ntsc | TimingMode::Gba => self.cpu_hz_ntsc,
            TimingMode::Pal => self.cpu_hz_pal,
        };
        let cycles_per_sample = cpu_hz / SAMPLE_HZ;

        let mut out = Vec::with_capacity(sample_count);
        for _ in 0..sample_count {
            self.cycle_accum += cycles_per_sample;
            let mut cycles = self.cycle_accum as u32;
            if cycles == 0 {
                cycles = 1; // Ensure we advance state even if timing slips
            }
            self.cycle_accum -= cycles as f64;

            let mut acc = 0i32;
            for _ in 0..cycles {
                acc += self.psg.clock() as i32;
            }

            out.push((acc / cycles as i32).clamp(-32768, 32767) as i16);
        }

        out
    }

    /// Serialise PSG state for save states.
    pub fn get_state(&self) -> serde_json::Value {
        serde_json::json!({
            "psg": self.psg.get_state(),
            "cycle_accum": self.cycle_accum,
            "timing": match self.timing {
                TimingMode::Ntsc | TimingMode::Gba => "ntsc",
                TimingMode::Pal => "pal",
            },
        })
    }

    /// Restore PSG state from a save state.
    ///
    /// Note: the CPU clock rates (`cpu_hz_ntsc`/`cpu_hz_pal`) are **not** stored
    /// in the save state; they are preserved from the current adapter instance.
    /// This is intentional — the clock rates are system constants baked in at
    /// construction time and must not change across a save/load cycle.
    pub fn set_state(&mut self, state: &serde_json::Value) -> Result<(), serde_json::Error> {
        if let Some(psg_state) = state.get("psg") {
            self.psg.set_state(psg_state)?;
        }
        if let Some(acc) = state.get("cycle_accum").and_then(|v| v.as_f64()) {
            self.cycle_accum = acc;
        }
        if let Some(timing_str) = state.get("timing").and_then(|v| v.as_str()) {
            let timing = match timing_str {
                "pal" => TimingMode::Pal,
                _ => TimingMode::Ntsc,
            };
            // Keep both the adapter-level field and the inner PSG in sync.
            self.timing = timing;
            self.psg.timing_mode = timing;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_psg_creation() {
        let psg = Sn76489Psg::new(TimingMode::Ntsc);
        assert_eq!(psg.volume, [0x0F; 4]); // All muted
    }

    #[test]
    fn test_psg_volume_write() {
        let mut psg = Sn76489Psg::new(TimingMode::Ntsc);

        // Latch tone 0, volume
        psg.write(0x90); // Channel 0, volume, value 0 (max)
        assert_eq!(psg.volume[0], 0x00);

        // Latch tone 1, volume 5
        psg.write(0xB5); // Channel 1, volume, value 5
        assert_eq!(psg.volume[1], 0x05);

        // Latch tone 2, volume F (mute)
        psg.write(0xDF); // Channel 2, volume, value F
        assert_eq!(psg.volume[2], 0x0F);
    }

    #[test]
    fn test_psg_tone_frequency() {
        let mut psg = Sn76489Psg::new(TimingMode::Ntsc);

        // Set channel 0 frequency to 0x1A4
        psg.write(0x84); // Latch tone 0, data, low 4 bits = 0x4
        psg.write(0x1A); // High 6 bits = 0x1A

        assert_eq!(psg.tone_freq[0], 0x1A4);
    }

    #[test]
    fn test_psg_noise_control() {
        let mut psg = Sn76489Psg::new(TimingMode::Ntsc);

        // Set noise to white noise, rate 3 (uses tone 2)
        psg.write(0xE7); // Latch noise, control = 0x7

        assert_eq!(psg.noise_control, 0x07);
    }

    #[test]
    fn test_volume_to_amplitude() {
        let psg = Sn76489Psg::new(TimingMode::Ntsc);

        // Volume 0 should be maximum amplitude
        let max_amp = psg.volume_to_amplitude(0);
        assert!(max_amp > 0.9);

        // Volume 15 should be muted
        assert_eq!(psg.volume_to_amplitude(15), 0.0);

        // Volume 8 should be approximately -16dB
        let vol_8 = psg.volume_to_amplitude(8);
        assert!(vol_8 > 0.15 && vol_8 < 0.17);
    }

    // -------------------------------------------------------------------------
    // Sn76489Adapter tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_adapter_generate_samples_length() {
        let mut adapter = Sn76489Adapter::new(TimingMode::Ntsc, 3_579_545.0, 3_546_894.0);
        let samples = adapter.generate_samples(100);
        assert_eq!(samples.len(), 100);
    }

    #[test]
    fn test_adapter_generate_samples_silent_by_default() {
        let mut adapter = Sn76489Adapter::new(TimingMode::Ntsc, 3_579_545.0, 3_546_894.0);
        let samples = adapter.generate_samples(100);
        assert!(
            samples.iter().all(|&s| s == 0),
            "Expected silent output from a freshly created adapter (all channels muted)"
        );
    }

    #[test]
    fn test_adapter_set_timing_resets_accumulator() {
        let mut adapter = Sn76489Adapter::new(TimingMode::Ntsc, 3_579_545.0, 3_546_894.0);

        // Prime the accumulator with a few samples.
        let _ = adapter.generate_samples(5);

        // Switch timing — accumulator must be reset to 0.
        adapter.set_timing(TimingMode::Pal);
        assert_eq!(adapter.cycle_accum, 0.0);

        // Switching to the same mode must not reset anything.
        let _ = adapter.generate_samples(5); // advance accumulator again
        let accum_before = adapter.cycle_accum;
        adapter.set_timing(TimingMode::Pal); // same mode — no-op
        assert_eq!(adapter.cycle_accum, accum_before);
    }

    #[test]
    fn test_adapter_set_timing_keeps_psg_in_sync() {
        let mut adapter = Sn76489Adapter::new(TimingMode::Ntsc, 3_579_545.0, 3_546_894.0);
        adapter.set_timing(TimingMode::Pal);
        assert_eq!(adapter.timing, TimingMode::Pal);
        assert_eq!(adapter.psg.timing_mode, TimingMode::Pal);
    }

    #[test]
    fn test_adapter_get_set_state_roundtrip() {
        let mut adapter = Sn76489Adapter::new(TimingMode::Ntsc, 3_579_545.0, 3_546_894.0);

        // Make some state changes so the round-trip is non-trivial.
        adapter.write(0x90); // Channel 0 volume = 0 (max)
        adapter.write(0x80 | 0x04); // Tone 0 low bits
        adapter.write(0x01); // Tone 0 high bits
        adapter.set_timing(TimingMode::Pal);
        let _ = adapter.generate_samples(10); // advance cycle_accum

        let state = adapter.get_state();

        // Create a fresh adapter and restore state into it.
        let mut restored = Sn76489Adapter::new(TimingMode::Ntsc, 3_579_545.0, 3_546_894.0);
        restored.set_state(&state).unwrap();

        // Timing must be restored and consistent between adapter and inner PSG.
        assert_eq!(restored.timing, TimingMode::Pal);
        assert_eq!(restored.psg.timing_mode, TimingMode::Pal);

        // cycle_accum must match.
        let cycle_accum_saved = state.get("cycle_accum").and_then(|v| v.as_f64()).unwrap();
        assert!((restored.cycle_accum - cycle_accum_saved).abs() < 1e-9);
    }

    /// Verify that the ÷16 prescaler correctly gates the tone counters.
    ///
    /// With a non-zero frequency register, the tone output must not toggle
    /// before 16 `clock_once()` calls have been made (prescaler has not fired
    /// yet), and must toggle exactly on the 16th call (first prescaler fire
    /// hits a zero-initialized counter → reload + toggle).
    #[test]
    fn test_prescaler_tone_advances_every_16_clocks() {
        let mut psg = Sn76489Psg::new(TimingMode::Ntsc);

        // Set tone 0 to frequency register = 1, unmute channel 0.
        psg.write(0x81); // latch tone 0, low 4 bits = 1
        psg.write(0x00); // high bits = 0 → freq register = 1
        psg.write(0x90); // channel 0 volume = 0 (max)

        let initial_output = psg.tone_output[0];

        // Clocks 1–15: prescaler counter advances from 0 to 15 but has not
        // wrapped back to 0, so tone counters are never decremented and the
        // output must stay unchanged.
        for i in 0..15 {
            psg.clock_once();
            assert_eq!(
                psg.tone_output[0],
                initial_output,
                "output must not toggle before the prescaler fires (call {})",
                i + 1
            );
        }

        // Clock 16: prescaler fires (counter wraps to 0). tone_counter[0] == 0
        // → reload to 1 and toggle the output.
        psg.clock_once();
        assert_ne!(
            psg.tone_output[0], initial_output,
            "output must toggle on the 16th clock() call (first prescaler fire)"
        );
    }

    /// A corrupted/hand-edited prescaler value (> 15) loaded from a save state
    /// must not cause a panic or wrap-around.  After loading, `clock_once` must
    /// still behave correctly (prescaler fires again within 16 calls, never panics).
    #[test]
    fn test_prescaler_clamped_after_state_load() {
        let mut adapter = Sn76489Adapter::new(TimingMode::Ntsc, 3_579_545.0, 3_546_894.0);

        // Build a state JSON with an out-of-range prescaler.
        let mut state = adapter.get_state();
        let psg_state = state.get_mut("psg").unwrap();
        *psg_state.get_mut("prescaler").unwrap() = serde_json::json!(200u8);

        // Loading must not panic.
        adapter.set_state(&state).unwrap();

        // After clamping, prescaler must be in [0, 15].
        assert!(
            adapter.psg.prescaler <= 15,
            "prescaler should be clamped to 0..=15, got {}",
            adapter.psg.prescaler
        );

        // Clocking must work normally after the load.
        let _ = adapter.generate_samples(5);
    }
}
