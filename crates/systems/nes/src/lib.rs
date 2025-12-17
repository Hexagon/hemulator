//! Minimal NES system skeleton for wiring into the core.

use emu_core::{types::Frame, System};

#[derive(Debug, Default)]
pub struct NesSystem {
    // placeholder fields for CPU, PPU, APU, cartridge, mappers
}

#[derive(thiserror::Error, Debug)]
#[error("NES error")]
pub struct NesError;

impl System for NesSystem {
    type Error = NesError;

    fn reset(&mut self) {}

    fn step_frame(&mut self) -> Result<Frame, Self::Error> {
        // Return a default NTSC NES resolution 256x240 for now
        Ok(Frame::new(256, 240))
    }

    fn save_state(&self) -> serde_json::Value {
        serde_json::json!({ "system": "nes", "version": 1 })
    }

    fn load_state(&mut self, _v: &serde_json::Value) -> Result<(), serde_json::Error> {
        Ok(())
    }
}
