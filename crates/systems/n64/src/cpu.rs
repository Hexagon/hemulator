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
pub const CP0_STATUS_COMMERCIAL_BOOT: u64 = 0x34000000; // CU0, CU1 enabled; IE=0 (game enables interrupts after installing handler)

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
        if let Some(_entry_point) = self.bus().get_entry_point() {
            log(LogCategory::CPU, LogLevel::Info, || {
                format!(
                    "N64 CPU: Commercial ROM detected, starting IPL3 from DMEM (entry point 0x{:016X} stored for IPL3)",
                    _entry_point
                )
            });

            // Initialize CP0 registers for boot
            // These values represent the state after PIF ROM (IPL1/IPL2) execution
            self.cpu.cp0[12] = CP0_STATUS_COMMERCIAL_BOOT;
            self.cpu.cp0[16] = CP0_CONFIG_COMMERCIAL_BOOT;

            // Set PC to start of IPL3 code in SP DMEM
            // IPL3 is loaded from ROM[0x40..0x1000] into DMEM[0x40..0x1000]
            // IPL3 will: init RDRAM, DMA ROM to RDRAM, verify CRC, set up
            // registers, and jump to the game's entry point
            self.cpu.pc = 0xFFFF_FFFF_A400_0040;

            // Set SP to end of DMEM (IPL3 uses DMEM for its stack)
            self.cpu.gpr[29] = 0xFFFF_FFFF_A400_1FF0; // $sp

            // Set registers that PIF boot ROM (IPL1/IPL2) normally initializes
            // before jumping to IPL3. These values are based on CIC-NUS-6102
            // register state documented in N64 homebrew references.
            let cic_seed = self.cpu.memory.get_cic_seed();
            self.cpu.gpr[20] = 1; // $s4
            self.cpu.gpr[22] = cic_seed as u64; // $s6 = CIC seed
            self.cpu.gpr[23] = 1; // $s7

            log(LogCategory::CPU, LogLevel::Info, || {
                format!(
                    "N64 CPU: IPL3 boot, PC=0x{:016X}, SP=0x{:016X}",
                    self.cpu.pc, self.cpu.gpr[29]
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
        self.cpu.step()
    }

    pub fn bus(&self) -> &N64Bus {
        &self.cpu.memory
    }

    pub fn bus_mut(&mut self) -> &mut N64Bus {
        &mut self.cpu.memory
    }
}
