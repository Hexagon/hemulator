//! Minimal NES system skeleton for wiring into the core.

mod cpu;

use cpu::NesCpu;
use emu_core::{types::Frame, System};

#[derive(Debug)]
pub struct NesSystem {
    cpu: NesCpu,
}

impl Default for NesSystem {
    fn default() -> Self {
        let mut cpu = NesCpu::new();
        cpu.reset();
        Self { cpu }
    }
}

#[derive(thiserror::Error, Debug)]
#[error("NES error")]
pub struct NesError;

impl System for NesSystem {
    type Error = NesError;

    fn reset(&mut self) {
        self.cpu.reset();
    }

    fn step_frame(&mut self) -> Result<Frame, Self::Error> {
        // For now, just run a fixed number of CPU steps and return a blank frame.
        // NES NTSC has ~29780 CPU cycles per frame; use a smaller number for the skeleton.
        for _ in 0..1000 {
            self.cpu.step();
        }
        Ok(Frame::new(256, 240))
    }

    fn save_state(&self) -> serde_json::Value {
        serde_json::json!({ "system": "nes", "version": 1, "a": self.cpu.a })
    }

    fn load_state(&mut self, _v: &serde_json::Value) -> Result<(), serde_json::Error> {
        Ok(())
    }
}
