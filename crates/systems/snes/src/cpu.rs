//! SNES CPU wrapper for the 65C816 core

use crate::bus::SnesBus;
use emu_core::cpu_65c816::Cpu65c816;

/// SNES CPU wrapper
pub struct SnesCpu {
    pub cpu: Cpu65c816<SnesBus>,
}

impl SnesCpu {
    pub fn new(bus: SnesBus) -> Self {
        Self {
            cpu: Cpu65c816::new(bus),
        }
    }

    pub fn reset(&mut self) {
        self.cpu.reset();
    }

    pub fn step(&mut self) -> u32 {
        self.cpu.step()
    }

    pub fn bus(&self) -> &SnesBus {
        &self.cpu.memory
    }

    pub fn bus_mut(&mut self) -> &mut SnesBus {
        &mut self.cpu.memory
    }
}
