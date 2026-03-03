//! ColecoVision PSG wrapper.
//!
//! Thin wrapper around [`emu_core::apu::sn76489::Sn76489Adapter`] using the
//! ColecoVision CPU clock rate (3.579545 MHz, NTSC only).

use emu_core::apu::{sn76489::Sn76489Adapter, TimingMode};

/// ColecoVision CPU clock frequency (Hz). The ColecoVision was NTSC-only.
const CPU_HZ: f64 = 3_579_545.0;

/// ColecoVision PSG (Texas Instruments SN76489).
pub struct ColecoVisionPsg(Sn76489Adapter);

impl ColecoVisionPsg {
    /// Create a new ColecoVision PSG.
    pub fn new() -> Self {
        Self(Sn76489Adapter::new(TimingMode::Ntsc, CPU_HZ, CPU_HZ))
    }

    /// Write to the PSG data port.
    pub fn write(&mut self, data: u8) {
        self.0.write(data);
    }

    /// Reset the PSG to its power-on state.
    pub fn reset(&mut self) {
        self.0.reset();
    }

    /// Generate `count` audio samples at 44.1 kHz.
    pub fn generate_samples(&mut self, count: usize) -> Vec<i16> {
        self.0.generate_samples(count)
    }

    /// Serialise PSG state for save states.
    pub fn get_state(&self) -> serde_json::Value {
        self.0.get_state()
    }

    /// Restore PSG state from a save state.
    pub fn set_state(&mut self, state: &serde_json::Value) -> Result<(), serde_json::Error> {
        self.0.set_state(state)
    }
}

impl Default for ColecoVisionPsg {
    fn default() -> Self {
        Self::new()
    }
}
