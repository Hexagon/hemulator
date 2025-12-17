//! Minimal Game Boy system skeleton for wiring into the core.

use emu_core::{types::Frame, System};

#[derive(Debug, Default)]
pub struct GbSystem {
    // placeholder fields for CPU, PPU (LCD), MMU, cartridge
}

#[derive(thiserror::Error, Debug)]
#[error("GB error")]
pub struct GbError;

impl System for GbSystem {
    type Error = GbError;

    fn reset(&mut self) {}

    fn step_frame(&mut self) -> Result<Frame, Self::Error> {
        // Game Boy native framebuffer is 160x144
        Ok(Frame::new(160, 144))
    }

    fn save_state(&self) -> serde_json::Value {
        serde_json::json!({ "system": "gb", "version": 1 })
    }

    fn load_state(&mut self, _v: &serde_json::Value) -> Result<(), serde_json::Error> {
        Ok(())
    }
}
