//! N64 CPU wrapper for the MIPS R4300i core

use crate::bus::N64Bus;
use emu_core::cpu_mips_r4300i::CpuMips;
use emu_core::logging::{log, LogCategory, LogLevel};

/// CP0_STATUS register value for commercial ROM boot
/// CU0=1 (Coprocessor 0 usable), CU1=1 (FPU usable), BEV=0 (use normal exception vectors)
/// IE=1 (Interrupts Enabled), IM3=1 (VI interrupt enabled on line 3)
/// Bit breakdown:
/// - Bit 0 (IE): 1 = Interrupts enabled
/// - Bit 11 (IM3): 1 = Allow interrupt line 3 (VI interrupt)
/// - Bits 28-29 (CU0, CU1): 1 = Coprocessors enabled
#[allow(dead_code)] // Used in tests
pub const CP0_STATUS_COMMERCIAL_BOOT: u64 = 0x34000801; // 0x34000000 | 0x01 (IE) | 0x800 (IM3)

/// CP0_CONFIG register value for commercial ROM boot
/// Standard configuration used by IPL3 bootloader
#[allow(dead_code)] // Used in tests
pub const CP0_CONFIG_COMMERCIAL_BOOT: u64 = 0x0006E463;

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
            self.cpu.cp0[12] = CP0_STATUS_COMMERCIAL_BOOT;
            self.cpu.cp0[16] = CP0_CONFIG_COMMERCIAL_BOOT;

            // Initialize GPRs that are expected by commercial ROMs
            // Based on real N64 IPL3 boot sequence
            self.cpu.gpr[11] = 0xFFFFFFFF_A4000040; // $t3 = cart domain 1 config address
            self.cpu.gpr[20] = 0x0000000000000001; // $s4 = 1
            self.cpu.gpr[22] = 0x000000000000003F; // $s6 = 0x3F
            self.cpu.gpr[29] = 0xFFFFFFFF_A4001FF0; // $sp = stack pointer (end of RDRAM - 0x10)
            self.cpu.gpr[31] = 0xFFFFFFFF_A4001550; // $ra = return address placeholder

            // Set PC to entry point (typically 0x80000400 or game-specific address)
            self.cpu.pc = entry_point;

            log(LogCategory::CPU, LogLevel::Info, || {
                format!(
                    "N64 CPU: Initialized CP0 and GPRs, PC now at 0x{:016X}",
                    self.cpu.pc
                )
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
