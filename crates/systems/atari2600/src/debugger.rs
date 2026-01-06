//! Atari 2600 system debugger implementation.
//!
//! Provides debug introspection for the Atari 2600 system including disassembly,
//! memory inspection, and CPU state tracking.

use crate::Atari2600System;
use emu_core::cpu_6502::Memory6502;
use emu_core::debug::{CpuRegister, CpuState, Debugger, DisassembledInstruction, MemoryRegion};
use emu_core::disasm_6502;

impl Debugger for Atari2600System {
    fn disassemble_instruction(&self, address: u32) -> Option<DisassembledInstruction> {
        // Read up to 3 bytes for the instruction (max 6502 instruction size)
        let memory = self.read_memory(address, 3)?;
        disasm_6502::disassemble_6502(&memory, address)
    }

    fn read_memory(&self, address: u32, length: usize) -> Option<Vec<u8>> {
        // 6507 has 13-bit address space (8KB)
        if address > 0x1FFF {
            return None;
        }

        let mut result = Vec::with_capacity(length);
        let bus = self.cpu.bus()?;

        for i in 0..length {
            let addr = address.wrapping_add(i as u32);
            if addr > 0x1FFF {
                break;
            }
            result.push(bus.read(addr as u16));
        }

        Some(result)
    }

    fn get_memory_regions(&self) -> Vec<MemoryRegion> {
        vec![
            // TIA registers (write)
            MemoryRegion::new(
                "TIA Write",
                0x0000,
                0x002F,
                "TIA write registers (graphics, sound, input)",
                false,
                true,
            ),
            // TIA registers (read)
            MemoryRegion::new(
                "TIA Read",
                0x0030,
                0x003F,
                "TIA read registers (collision, input)",
                true,
                false,
            ),
            // PIA RAM
            MemoryRegion::new(
                "RAM",
                0x0080,
                0x00FF,
                "128 bytes RAM (mirrored to 0x01FF)",
                true,
                true,
            ),
            // RIOT I/O
            MemoryRegion::new(
                "RIOT I/O",
                0x0280,
                0x0297,
                "RIOT I/O registers (timer, ports)",
                true,
                true,
            ),
            // Cartridge ROM
            MemoryRegion::new(
                "ROM",
                0x1000,
                0x1FFF,
                "Cartridge ROM (4KB, may be banked)",
                true,
                false,
            ),
        ]
    }

    fn get_cpu_state(&self) -> CpuState {
        let cpu_ref = match self.cpu.cpu.as_ref() {
            Some(c) => c,
            None => {
                // Return a default state if CPU is not available
                let mut state = CpuState::new(0);
                state.add_register(CpuRegister::new_16bit("PC", 0));
                state.add_register(CpuRegister::new_8bit("A", 0));
                state.add_register(CpuRegister::new_8bit("X", 0));
                state.add_register(CpuRegister::new_8bit("Y", 0));
                state.add_register(CpuRegister::new_8bit("SP", 0));
                return state;
            }
        };

        let mut state = CpuState::new(cpu_ref.pc as u32);

        // Add registers (including PC for display in the registers panel)
        state.add_register(CpuRegister::new_16bit("PC", cpu_ref.pc));
        state.add_register(CpuRegister::new_8bit("A", cpu_ref.a));
        state.add_register(CpuRegister::new_8bit("X", cpu_ref.x));
        state.add_register(CpuRegister::new_8bit("Y", cpu_ref.y));
        state.add_register(CpuRegister::new_8bit("SP", cpu_ref.sp));

        // Add status flags (NV-BDIZC format)
        let status = cpu_ref.status;
        state.add_flag("N", (status & 0x80) != 0); // Negative
        state.add_flag("V", (status & 0x40) != 0); // Overflow
        state.add_flag("B", (status & 0x10) != 0); // Break
        state.add_flag("D", (status & 0x08) != 0); // Decimal
        state.add_flag("I", (status & 0x04) != 0); // IRQ Disable
        state.add_flag("Z", (status & 0x02) != 0); // Zero
        state.add_flag("C", (status & 0x01) != 0); // Carry

        state
    }

    fn get_execution_history(&self) -> Vec<emu_core::debug::ExecutionTrace> {
        self.instruction_tracer.get_history()
    }

    fn has_execution_history(&self) -> bool {
        self.instruction_tracer.is_enabled()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atari2600_memory_regions() {
        let system = Atari2600System::new();
        let regions = system.get_memory_regions();

        // Should have the basic memory regions
        assert_eq!(regions.len(), 5);

        // Check that RAM region exists
        let ram_region = regions.iter().find(|r| r.name == "RAM");
        assert!(ram_region.is_some());
        let ram = ram_region.unwrap();
        assert_eq!(ram.start, 0x0080);
        assert_eq!(ram.end, 0x00FF);
        assert!(ram.readable);
        assert!(ram.writable);
    }

    #[test]
    fn test_atari2600_cpu_state() {
        let system = Atari2600System::new();
        let state = system.get_cpu_state();

        // Should have all standard 6502 registers
        assert!(state.registers.iter().any(|r| r.name == "A"));
        assert!(state.registers.iter().any(|r| r.name == "X"));
        assert!(state.registers.iter().any(|r| r.name == "Y"));
        assert!(state.registers.iter().any(|r| r.name == "SP"));

        // Should have all status flags in correct order (NV-BDIZC)
        assert_eq!(state.flags.flags.len(), 7);
        let flag_names: Vec<&str> = state
            .flags
            .flags
            .iter()
            .map(|(name, _)| name.as_str())
            .collect();
        assert_eq!(flag_names, vec!["N", "V", "B", "D", "I", "Z", "C"]);
    }

    #[test]
    fn test_atari2600_read_memory() {
        let system = Atari2600System::new();

        // Should be able to read from valid addresses
        let memory = system.read_memory(0x0080, 16);
        assert!(memory.is_some());
        assert_eq!(memory.unwrap().len(), 16);

        // Should return None for invalid addresses (beyond 13-bit address space)
        let invalid = system.read_memory(0x2000, 1);
        assert!(invalid.is_none());
    }
}
