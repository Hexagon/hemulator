//! Minimal NES system skeleton for wiring into the core.

mod cpu;
mod cartridge;

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

impl NesSystem {
    /// Load a mapper-0 (NROM) iNES ROM into CPU memory. This writes PRG ROM
    /// into 0x8000.. and mirrors 16KB banks into 0xC000 when necessary.
    pub fn load_rom_from_path<P: AsRef<std::path::Path>>(&mut self, path: P) -> Result<(), std::io::Error> {
        let cart = cartridge::Cartridge::from_file(path)?;
        if cart.mapper != 0 {
            // only mapper 0 supported for now
            return Err(std::io::Error::new(std::io::ErrorKind::Other, "Unsupported mapper (only NROM/0 supported)"));
        }

        let prg = &cart.prg_rom;
        let prg_len = prg.len();
        if prg_len == 0 {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "PRG ROM empty"));
        }

        // If 16KB PRG, mirror it to 0xC000-0xFFFF; if 32KB, fill 0x8000-0xFFFF
        if prg_len == 16 * 1024 {
            let base = 0x8000usize;
            self.cpu.memory[base..base + prg_len].copy_from_slice(prg);
            // mirror
            let mirror_base = 0xC000usize;
            self.cpu.memory[mirror_base..mirror_base + prg_len].copy_from_slice(prg);
        } else if prg_len == 32 * 1024 {
            let base = 0x8000usize;
            self.cpu.memory[base..base + prg_len].copy_from_slice(prg);
        } else {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Unsupported PRG ROM size"));
        }

        // Reset vector should be inside PRG ROM; read and set PC accordingly
        let lo = self.cpu.memory[0xFFFC] as u16;
        let hi = self.cpu.memory[0xFFFD] as u16;
        let vec = (hi << 8) | lo;
        self.cpu.pc = vec;
        Ok(())
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
