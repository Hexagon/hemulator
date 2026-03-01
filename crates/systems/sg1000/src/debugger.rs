//! SG-1000 debugger implementation.
//!
//! Provides debug introspection for the SG-1000 system including disassembly,
//! memory inspection, and CPU state tracking.

use crate::Sg1000System;
use emu_core::cpu_z80::MemoryZ80;
use emu_core::debug::{CpuRegister, CpuState, Debugger, DisassembledInstruction, MemoryRegion};
use emu_core::disasm_z80;

impl Debugger for Sg1000System {
    fn disassemble_instruction(&self, address: u32) -> Option<DisassembledInstruction> {
        // Read up to 4 bytes for Z80 instruction (max size with prefixes)
        let memory = self.read_memory(address, 4)?;
        disasm_z80::disassemble_z80(&memory, address)
    }

    fn read_memory(&self, address: u32, length: usize) -> Option<Vec<u8>> {
        // Z80 has 16-bit address space
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
            // Cartridge ROM
            MemoryRegion::new(
                "Cartridge",
                0x0000,
                0xBFFF,
                "Cartridge ROM (up to 48KB)",
                true,
                false,
            ),
            // RAM
            MemoryRegion::new("RAM", 0xC000, 0xC3FF, "Work RAM (1KB)", true, true),
            // RAM Mirror
            MemoryRegion::new(
                "RAM Mirror",
                0xC400,
                0xFFFF,
                "Mirror of C000-C3FF",
                true,
                true,
            ),
        ]
    }

    fn get_cpu_state(&self) -> CpuState {
        let mut state = CpuState::new(self.cpu.pc as u32);

        // Add primary registers
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

        // Add shadow registers
        state.add_register(CpuRegister::new_8bit("A'", self.cpu.a_prime));
        state.add_register(CpuRegister::new_8bit("F'", self.cpu.f_prime));

        // Add index registers
        state.add_register(CpuRegister::new_16bit("IX", self.cpu.ix));
        state.add_register(CpuRegister::new_16bit("IY", self.cpu.iy));

        // Add special registers
        state.add_register(CpuRegister::new_8bit("I", self.cpu.i));
        state.add_register(CpuRegister::new_8bit("R", self.cpu.r));

        // Add flags (extracted from F register)
        // Z80 flags: S Z - H - P/V N C
        let flags = self.cpu.f;
        state.add_flag("S", (flags & 0x80) != 0); // Sign
        state.add_flag("Z", (flags & 0x40) != 0); // Zero
        state.add_flag("H", (flags & 0x10) != 0); // Half Carry
        state.add_flag("P/V", (flags & 0x04) != 0); // Parity/Overflow
        state.add_flag("N", (flags & 0x02) != 0); // Add/Subtract
        state.add_flag("C", (flags & 0x01) != 0); // Carry

        // Add interrupt state
        state.add_flag("IFF1", self.cpu.iff1);
        state.add_flag("IFF2", self.cpu.iff2);

        state
    }

    emu_core::impl_debugger_execution_history!();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sg1000_memory_regions() {
        let system = Sg1000System::new();
        let regions = system.get_memory_regions();

        // Should have 3 memory regions
        assert_eq!(regions.len(), 3);

        // Check Cartridge region
        let cart = regions.iter().find(|r| r.name == "Cartridge");
        assert!(cart.is_some());
        let cart = cart.unwrap();
        assert_eq!(cart.start, 0x0000);
        assert_eq!(cart.end, 0xBFFF);
        assert!(cart.readable);
        assert!(!cart.writable);

        // Check RAM region
        let ram = regions.iter().find(|r| r.name == "RAM");
        assert!(ram.is_some());
        let ram = ram.unwrap();
        assert_eq!(ram.start, 0xC000);
        assert_eq!(ram.end, 0xC3FF);
        assert!(ram.readable);
        assert!(ram.writable);
    }

    #[test]
    fn test_sg1000_cpu_state() {
        let system = Sg1000System::new();
        let state = system.get_cpu_state();

        // Should have all Z80 registers
        assert!(state.registers.iter().any(|r| r.name == "A"));
        assert!(state.registers.iter().any(|r| r.name == "B"));
        assert!(state.registers.iter().any(|r| r.name == "C"));
        assert!(state.registers.iter().any(|r| r.name == "D"));
        assert!(state.registers.iter().any(|r| r.name == "E"));
        assert!(state.registers.iter().any(|r| r.name == "H"));
        assert!(state.registers.iter().any(|r| r.name == "L"));
        assert!(state.registers.iter().any(|r| r.name == "F"));
        assert!(state.registers.iter().any(|r| r.name == "SP"));
        assert!(state.registers.iter().any(|r| r.name == "IX"));
        assert!(state.registers.iter().any(|r| r.name == "IY"));
        assert!(state.registers.iter().any(|r| r.name == "I"));
        assert!(state.registers.iter().any(|r| r.name == "R"));

        // Should have flags in correct order (S Z H P/V N C IFF1 IFF2)
        assert_eq!(state.flags.flags.len(), 8);
        let flag_names: Vec<&str> = state
            .flags
            .flags
            .iter()
            .map(|(name, _)| name.as_str())
            .collect();
        assert_eq!(
            flag_names,
            vec!["S", "Z", "H", "P/V", "N", "C", "IFF1", "IFF2"]
        );
    }

    #[test]
    fn test_sg1000_read_memory() {
        let system = Sg1000System::new();

        // Should be able to read from valid addresses
        let memory = system.read_memory(0x0000, 16);
        assert!(memory.is_some());
        assert_eq!(memory.unwrap().len(), 16);

        // Should return None for invalid addresses
        let invalid = system.read_memory(0x10000, 1);
        assert!(invalid.is_none());
    }

    #[test]
    fn test_sg1000_check_breakpoint() {
        let mut system = Sg1000System::new();

        // Initially no breakpoint should fire
        assert!(system.check_breakpoint().is_none());

        // Add a breakpoint at the current PC
        let pc = system.cpu.pc as u32;
        system.add_breakpoint(pc);
        assert!(system.check_breakpoint().is_some());
        assert_eq!(system.check_breakpoint().unwrap(), pc);

        // Removing the breakpoint should stop it from firing
        system.remove_breakpoint(pc);
        assert!(system.check_breakpoint().is_none());
    }
}
