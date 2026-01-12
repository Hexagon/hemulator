//! ColecoVision PSG wrapper
//!
//! Wraps the core SN76489 implementation with ColecoVision-specific timing and integration.

use emu_core::apu::sn76489::Sn76489Psg;
use emu_core::apu::{AudioChip, TimingMode};
use emu_core::types::AudioSample;

/// ColecoVision PSG (Texas Instruments SN76489)
pub struct ColecoVisionPsg {
    /// Core SN76489 implementation
    psg: Sn76489Psg,

    /// Cycle accumulator for downsampling
    cycle_accum: f64,

    /// Current timing mode (NTSC for ColecoVision)
    timing: TimingMode,
}

impl ColecoVisionPsg {
    /// Create a new ColecoVision PSG
    pub fn new() -> Self {
        let timing = TimingMode::Ntsc;
        Self {
            psg: Sn76489Psg::new(timing),
            cycle_accum: 0.0,
            timing,
        }
    }

    /// Write to PSG data port
    pub fn write(&mut self, data: u8) {
        self.psg.write(data);
    }

    /// Step the PSG by the given number of CPU cycles and collect audio samples
    pub fn step(&mut self, _cycles: u32) -> Vec<AudioSample> {
        // For now, generate based on a fixed sample rate
        const SAMPLE_HZ: f64 = 44_100.0;
        let cpu_hz = 3_579_545.0; // ColecoVision NTSC CPU frequency
        let cycles_per_sample = cpu_hz / SAMPLE_HZ;

        let mut samples = Vec::new();

        // Add cycles to accumulator
        self.cycle_accum += _cycles as f64;

        // Generate samples while we have enough cycles accumulated
        while self.cycle_accum >= cycles_per_sample {
            // Clock PSG for one sample period
            let mut acc = 0i32;
            let cycles = cycles_per_sample as u32;
            for _ in 0..cycles {
                let sample = self.psg.clock() as i32;
                acc += sample;
            }

            // Average over all cycles
            let avg = acc / cycles as i32;
            samples.push(avg.clamp(-32768, 32767) as i16);

            // Consume cycles
            self.cycle_accum -= cycles_per_sample;
        }

        samples
    }

    /// Get PSG state for save state
    pub fn get_state(&self) -> serde_json::Value {
        serde_json::json!({
            "psg": self.psg.get_state(),
            "cycle_accum": self.cycle_accum,
            "timing": match self.timing {
                TimingMode::Ntsc => "ntsc",
                TimingMode::Pal => "pal",
            },
        })
    }

    /// Set PSG state from save state
    pub fn set_state(&mut self, state: &serde_json::Value) -> Result<(), serde_json::Error> {
        if let Some(psg_state) = state.get("psg") {
            self.psg.set_state(psg_state)?;
        }

        if let Some(acc) = state.get("cycle_accum").and_then(|v| v.as_f64()) {
            self.cycle_accum = acc;
        }

        if let Some(timing_str) = state.get("timing").and_then(|v| v.as_str()) {
            self.timing = match timing_str {
                "pal" => TimingMode::Pal,
                _ => TimingMode::Ntsc,
            };
        }

        Ok(())
    }
}

impl Default for ColecoVisionPsg {
    fn default() -> Self {
        Self::new()
    }
}
