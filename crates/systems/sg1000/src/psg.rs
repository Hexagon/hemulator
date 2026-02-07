//! SG-1000 PSG wrapper
//!
//! Wraps the core SN76489 implementation with SG-1000-specific timing and integration.

use emu_core::apu::sn76489::Sn76489Psg;
use emu_core::apu::{AudioChip, TimingMode};
use emu_core::types::AudioSample;

/// SG-1000 PSG (Texas Instruments SN76489)
pub struct Sg1000Psg {
    /// Core SN76489 implementation
    psg: Sn76489Psg,

    /// Cycle accumulator for downsampling
    cycle_accum: f64,

    /// Current timing mode (NTSC for SG-1000)
    timing: TimingMode,
}

impl Sg1000Psg {
    /// Create a new SG-1000 PSG
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

    /// Reset the PSG to initial state
    pub fn reset(&mut self) {
        self.psg.reset();
        self.cycle_accum = 0.0;
    }

    /// Generate audio samples for a given count, stepping PSG in CPU-cycle time
    /// using the configured timing mode and sample rate of 44.1 kHz.
    ///
    /// This method follows the same pattern as the SMS PSG:
    /// 1. Calculate how many CPU cycles correspond to each audio sample
    /// 2. Clock the PSG that many times per sample
    /// 3. Average the output over those cycles
    pub fn generate_samples(&mut self, sample_count: usize) -> Vec<AudioSample> {
        const SAMPLE_HZ: f64 = 44_100.0;
        // SG-1000 CPU clock rate (NTSC only)
        let cpu_hz = match self.timing {
            TimingMode::Ntsc => 3_579_545.0, // SG-1000 NTSC
            TimingMode::Pal => 3_579_545.0,  // SG-1000 was NTSC only, but keep for consistency
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

            // Clock PSG for all cycles and accumulate output
            let mut acc = 0i32;
            for _ in 0..cycles {
                let sample = self.psg.clock() as i32;
                acc += sample;
            }

            // Average over all cycles
            let avg = acc / cycles as i32;
            out.push(avg.clamp(-32768, 32767) as i16);
        }

        out
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

impl Default for Sg1000Psg {
    fn default() -> Self {
        Self::new()
    }
}
