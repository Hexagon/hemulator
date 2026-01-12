//! Game Boy system debugger implementation.
//!
//! Provides debug introspection for the Game Boy system including disassembly,
//! memory inspection, and CPU state tracking.

use crate::GbSystem;
use emu_core::cpu_lr35902::MemoryLr35902;
use emu_core::debug::{CpuRegister, CpuState, Debugger, DisassembledInstruction, MemoryRegion};
use emu_core::disasm_lr35902;

impl Debugger for GbSystem {
    fn disassemble_instruction(&self, address: u32) -> Option<DisassembledInstruction> {
        // Read up to 3 bytes for the instruction (max LR35902 instruction size)
        let memory = self.read_memory(address, 3)?;
        disasm_lr35902::disassemble_lr35902(&memory, address)
    }

    fn read_memory(&self, address: u32, length: usize) -> Option<Vec<u8>> {
        // LR35902 has 16-bit address space
        if address > 0xFFFF {
            return None;
        }

        let mut result = Vec::with_capacity(length);
        for i in 0..length {
            let addr = address.wrapping_add(i as u32);
            if addr > 0xFFFF {
                break;
            }
            result.push(self.cpu.memory.read(addr as u16));
        }

        Some(result)
    }

    fn get_memory_regions(&self) -> Vec<MemoryRegion> {
        vec![
            // ROM Bank 0
            MemoryRegion::new(
                "ROM Bank 0",
                0x0000,
                0x3FFF,
                "Fixed ROM bank (16KB)",
                true,
                false,
            ),
            // ROM Bank 1-N
            MemoryRegion::new(
                "ROM Bank 1-N",
                0x4000,
                0x7FFF,
                "Switchable ROM bank (16KB)",
                true,
                false,
            ),
            // VRAM
            MemoryRegion::new("VRAM", 0x8000, 0x9FFF, "Video RAM (8KB)", true, true),
            // External RAM
            MemoryRegion::new(
                "External RAM",
                0xA000,
                0xBFFF,
                "Switchable external RAM (8KB)",
                true,
                true,
            ),
            // Work RAM
            MemoryRegion::new("WRAM", 0xC000, 0xDFFF, "Work RAM (8KB)", true, true),
            // Echo RAM
            MemoryRegion::new(
                "Echo RAM",
                0xE000,
                0xFDFF,
                "Mirror of C000-DDFF",
                true,
                true,
            ),
            // OAM
            MemoryRegion::new(
                "OAM",
                0xFE00,
                0xFE9F,
                "Object Attribute Memory (160 bytes)",
                true,
                true,
            ),
            // I/O Registers
            MemoryRegion::new(
                "I/O Registers",
                0xFF00,
                0xFF7F,
                "I/O and control registers",
                true,
                true,
            ),
            // High RAM
            MemoryRegion::new("HRAM", 0xFF80, 0xFFFE, "High RAM (127 bytes)", true, true),
            // Interrupt Enable
            MemoryRegion::new(
                "IE Register",
                0xFFFF,
                0xFFFF,
                "Interrupt Enable register",
                true,
                true,
            ),
        ]
    }

    fn get_cpu_state(&self) -> CpuState {
        let mut state = CpuState::new(self.cpu.pc as u32);

        // Add registers
        state.add_register(CpuRegister::new_16bit("PC", self.cpu.pc));
        state.add_register(CpuRegister::new_16bit("SP", self.cpu.sp));
        state.add_register(CpuRegister::new_8bit("A", self.cpu.a));
        state.add_register(CpuRegister::new_8bit("F", self.cpu.f));
        state.add_register(CpuRegister::new_8bit("B", self.cpu.b));
        state.add_register(CpuRegister::new_8bit("C", self.cpu.c));
        state.add_register(CpuRegister::new_8bit("D", self.cpu.d));
        state.add_register(CpuRegister::new_8bit("E", self.cpu.e));
        state.add_register(CpuRegister::new_8bit("H", self.cpu.h));
        state.add_register(CpuRegister::new_8bit("L", self.cpu.l));

        // Add flags (extracted from F register)
        // LR35902 flags: Z N H C - - - -
        let flags = self.cpu.f;
        state.add_flag("Z", (flags & 0x80) != 0); // Zero
        state.add_flag("N", (flags & 0x40) != 0); // Subtract
        state.add_flag("H", (flags & 0x20) != 0); // Half Carry
        state.add_flag("C", (flags & 0x10) != 0); // Carry

        // Add other important state
        state.add_flag("IME", self.cpu.ime); // Interrupt Master Enable
        state.add_flag("HALT", self.cpu.halted); // Halted state

        state
    }

    emu_core::impl_debugger_execution_history!();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gb_memory_regions() {
        let system = GbSystem::new();
        let regions = system.get_memory_regions();

        // Should have 10 memory regions
        assert_eq!(regions.len(), 10);

        // Check ROM Bank 0
        let rom0 = regions.iter().find(|r| r.name == "ROM Bank 0");
        assert!(rom0.is_some());
        let rom0 = rom0.unwrap();
        assert_eq!(rom0.start, 0x0000);
        assert_eq!(rom0.end, 0x3FFF);
        assert!(rom0.readable);
        assert!(!rom0.writable);

        // Check VRAM
        let vram = regions.iter().find(|r| r.name == "VRAM");
        assert!(vram.is_some());
        let vram = vram.unwrap();
        assert_eq!(vram.start, 0x8000);
        assert_eq!(vram.end, 0x9FFF);
        assert!(vram.readable);
        assert!(vram.writable);
    }

    #[test]
    fn test_gb_cpu_state() {
        let system = GbSystem::new();
        let state = system.get_cpu_state();

        // Should have all LR35902 registers
        assert!(state.registers.iter().any(|r| r.name == "A"));
        assert!(state.registers.iter().any(|r| r.name == "B"));
        assert!(state.registers.iter().any(|r| r.name == "C"));
        assert!(state.registers.iter().any(|r| r.name == "D"));
        assert!(state.registers.iter().any(|r| r.name == "E"));
        assert!(state.registers.iter().any(|r| r.name == "H"));
        assert!(state.registers.iter().any(|r| r.name == "L"));
        assert!(state.registers.iter().any(|r| r.name == "F"));
        assert!(state.registers.iter().any(|r| r.name == "SP"));

        // Should have flags in correct order (Z N H C IME HALT)
        assert_eq!(state.flags.flags.len(), 6);
        let flag_names: Vec<&str> = state
            .flags
            .flags
            .iter()
            .map(|(name, _)| name.as_str())
            .collect();
        assert_eq!(flag_names, vec!["Z", "N", "H", "C", "IME", "HALT"]);
    }

    #[test]
    fn test_gb_read_memory() {
        let system = GbSystem::new();

        // Should be able to read from valid addresses
        let memory = system.read_memory(0x0000, 16);
        assert!(memory.is_some());
        assert_eq!(memory.unwrap().len(), 16);

        // Should return None for invalid addresses
        let invalid = system.read_memory(0x10000, 1);
        assert!(invalid.is_none());
    }
}
