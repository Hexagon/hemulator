//! N64 CPU wrapper for the MIPS R4300i core

use crate::bus::N64Bus;
use emu_core::cpu_mips_r4300i::CpuMips;
use emu_core::logging::{log, LogCategory, LogLevel};

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
        log(LogCategory::CPU, LogLevel::Info, || {
            format!("N64 CPU: Reset to PC=0x{:016X}", self.cpu.pc)
        });
        self.cpu.reset();
    }

    pub fn step(&mut self) -> u32 {
        let old_pc = self.cpu.pc;
        
        log(LogCategory::CPU, LogLevel::Trace, || {
            format!("N64 CPU: PC=0x{:016X}", old_pc)
        });
        
        let cycles = self.cpu.step();
        
        // Log if we jumped to a new location (not just PC+4)
        if self.cpu.pc != old_pc.wrapping_add(4) {
            log(LogCategory::CPU, LogLevel::Debug, || {
                format!(
                    "N64 CPU: Jump/Branch from 0x{:016X} to 0x{:016X}",
                    old_pc, self.cpu.pc
                )
            });
        }
        
        cycles
    }

    pub fn bus(&self) -> &N64Bus {
        &self.cpu.memory
    }

    pub fn bus_mut(&mut self) -> &mut N64Bus {
        &mut self.cpu.memory
    }
}
