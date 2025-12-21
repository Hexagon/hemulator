//! N64 CPU wrapper for the MIPS R4300i core

use crate::bus::N64Bus;
use emu_core::cpu_mips_r4300i::CpuMips;

/// N64 CPU wrapper
pub struct N64Cpu {
    pub cpu: CpuMips<N64Bus>,
}

impl N64Cpu {
    pub fn new(bus: N64Bus) -> Self {
        Self {
            cpu: CpuMips::new(bus),
        }
    }

    pub fn reset(&mut self) {
        self.cpu.reset();
    }

    pub fn step(&mut self) -> u32 {
        self.cpu.step()
    }

    pub fn bus(&self) -> &N64Bus {
        &self.cpu.memory
    }

    pub fn bus_mut(&mut self) -> &mut N64Bus {
        &mut self.cpu.memory
    }
}
