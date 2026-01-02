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

        // Reset CPU to initial state
        self.cpu.reset();

        // Check if we have a commercial ROM loaded with an entry point
        // We need to access the bus through our wrapper methods
        if let Some(entry_point) = self.bus().get_entry_point() {
            log(LogCategory::CPU, LogLevel::Info, || {
                format!(
                    "N64 CPU: Commercial ROM detected, initializing CP0 and jumping to entry point 0x{:016X}",
                    entry_point
                )
            });

            // Initialize CP0 registers for commercial ROM boot
            // These values are set by the real IPL3 bootloader
            self.cpu.cp0[12] = 0x34000000; // CP0_STATUS: CU0=1, CU1=1, BEV=0
            self.cpu.cp0[16] = 0x0006E463; // CP0_CONFIG: Typical configuration

            // Set PC to entry point (typically 0x80000400)
            self.cpu.pc = entry_point;

            log(LogCategory::CPU, LogLevel::Info, || {
                format!("N64 CPU: Initialized CP0, PC now at 0x{:016X}", self.cpu.pc)
            });
        } else {
            // Test ROM or no ROM - use default PIF boot sequence
            log(LogCategory::CPU, LogLevel::Info, || {
                "N64 CPU: Test ROM mode, booting from PIF at 0xBFC00000".to_string()
            });
        }
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
