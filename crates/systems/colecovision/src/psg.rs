//! ColecoVision PSG wrapper
//!
//! Wraps the core SN76489 implementation with ColecoVision-specific timing and integration.

use emu_core::apu::sn76489::Sn76489;
use emu_core::apu::TimingMode;
use emu_core::types::AudioSample;

/// ColecoVision PSG (Texas Instruments SN76489)
pub struct ColecoVisionPsg {
    /// Core SN76489 implementation
    core: Sn76489,

    /// Timing mode (NTSC for ColecoVision)
    timing_mode: TimingMode,

    /// Cycle accumulator for downsampling
    cycle_accumulator: f64,

    /// CPU clock frequency (3.579545 MHz for NTSC)
    cpu_frequency: f64,

    /// Target audio sample rate (44.1 kHz)
    sample_rate: f64,

    /// Cycles per audio sample
    cycles_per_sample: f64,
}

impl ColecoVisionPsg {
    /// Create a new ColecoVision PSG
    pub fn new() -> Self {
        let timing_mode = TimingMode::Ntsc;
        let cpu_frequency = 3_579_545.0; // NTSC frequency
        let sample_rate = 44_100.0;
        let cycles_per_sample = cpu_frequency / sample_rate;

        Self {
            core: Sn76489::new(),
            timing_mode,
            cycle_accumulator: 0.0,
            cpu_frequency,
            sample_rate,
            cycles_per_sample,
        }
    }

    /// Write to PSG data port
    pub fn write(&mut self, data: u8) {
        self.core.write(data);
    }

    /// Step the PSG by the given number of CPU cycles and collect audio samples
    pub fn step(&mut self, cycles: u32) -> Vec<AudioSample> {
        let mut samples = Vec::new();

        // Add cycles to accumulator
        self.cycle_accumulator += cycles as f64;

        // Generate samples while we have enough cycles accumulated
        while self.cycle_accumulator >= self.cycles_per_sample {
            // Step the PSG core by the number of cycles per sample
            let sample = self.core.step(self.cycles_per_sample as u32);
            samples.push(sample);

            // Consume cycles
            self.cycle_accumulator -= self.cycles_per_sample;
        }

        samples
    }

    /// Get PSG state for save state
    pub fn get_state(&self) -> serde_json::Value {
        serde_json::json!({
            "core": self.core.get_state(),
            "timing_mode": format!("{:?}", self.timing_mode),
            "cycle_accumulator": self.cycle_accumulator,
        })
    }

    /// Set PSG state from save state
    pub fn set_state(&mut self, state: &serde_json::Value) -> Result<(), serde_json::Error> {
        if let Some(core_state) = state.get("core") {
            self.core.set_state(core_state)?;
        }

        if let Some(acc) = state.get("cycle_accumulator").and_then(|v| v.as_f64()) {
            self.cycle_accumulator = acc;
        }

        Ok(())
    }
}

impl Default for ColecoVisionPsg {
    fn default() -> Self {
        Self::new()
    }
}
