//! C64 system debugger implementation.
//!
//! Provides debug introspection for the C64 including disassembly,
//! memory inspection, and CPU state tracking.

use crate::C64System;
use emu_core::debug::{CpuRegister, CpuState, Debugger, DisassembledInstruction, MemoryRegion};
use emu_core::disasm_6502;

impl Debugger for C64System {
    fn disassemble_instruction(&self, address: u32) -> Option<DisassembledInstruction> {
        let memory = self.read_memory(address, 3)?;
        disasm_6502::disassemble_6502(&memory, address)
    }

    fn read_memory(&self, address: u32, length: usize) -> Option<Vec<u8>> {
        if address > 0xFFFF {
            return None;
        }

        let mut result = Vec::with_capacity(length);
        for i in 0..length {
            let addr = address.wrapping_add(i as u32);
            if addr > 0xFFFF {
                break;
            }
            // Use peek() to avoid side effects on I/O registers (e.g. clearing
            // sprite collision latches $D01E/$D01F when the memory viewer is open).
            result.push(self.cpu.memory.peek(addr as u16));
        }

        Some(result)
    }

    fn get_memory_regions(&self) -> Vec<MemoryRegion> {
        vec![
            MemoryRegion::new(
                "Zero Page",
                0x0000,
                0x00FF,
                "6510 I/O port ($0000/$0001) + zero page RAM",
                true,
                true,
            ),
            MemoryRegion::new(
                "Stack",
                0x0100,
                0x01FF,
                "CPU stack (grows down from $01FF)",
                true,
                true,
            ),
            MemoryRegion::new(
                "RAM",
                0x0200,
                0x9FFF,
                "Main RAM (always visible)",
                true,
                true,
            ),
            MemoryRegion::new(
                "BASIC ROM / RAM",
                0xA000,
                0xBFFF,
                "BASIC ROM (when LORAM=1 && HIRAM=1), else RAM",
                true,
                true,
            ),
            MemoryRegion::new("RAM", 0xC000, 0xCFFF, "RAM (always visible)", true, true),
            MemoryRegion::new(
                "I/O / Char ROM / RAM",
                0xD000,
                0xDFFF,
                "VIC-II, SID, CIA1, CIA2 (when CHAREN=1), Char ROM (when CHAREN=0), or RAM",
                true,
                true,
            ),
            MemoryRegion::new(
                "KERNAL ROM / RAM",
                0xE000,
                0xFFFF,
                "KERNAL ROM (when HIRAM=1), else RAM",
                true,
                true,
            ),
        ]
    }

    fn get_cpu_state(&self) -> CpuState {
        let mut state = CpuState::new(self.cpu.pc as u32);

        state.add_register(CpuRegister::new_16bit("PC", self.cpu.pc));
        state.add_register(CpuRegister::new_8bit("A", self.cpu.a));
        state.add_register(CpuRegister::new_8bit("X", self.cpu.x));
        state.add_register(CpuRegister::new_8bit("Y", self.cpu.y));
        state.add_register(CpuRegister::new_8bit("SP", self.cpu.sp));
        state.add_register(CpuRegister::new_8bit("IO", self.cpu.memory.io_port));

        // Status flags: NV-BDIZC
        let p = self.cpu.status;
        state.add_flag("N", p & 0x80 != 0); // Negative
        state.add_flag("V", p & 0x40 != 0); // Overflow
        state.add_flag("B", p & 0x10 != 0); // Break
        state.add_flag("D", p & 0x08 != 0); // Decimal
        state.add_flag("I", p & 0x04 != 0); // IRQ Disable
        state.add_flag("Z", p & 0x02 != 0); // Zero
        state.add_flag("C", p & 0x01 != 0); // Carry

        state
    }

    emu_core::impl_debugger_execution_history!();
}

#[cfg(test)]
mod tests {
    use super::*;
    use emu_core::System;

    #[test]
    fn test_c64_memory_regions() {
        let system = C64System::new();
        let regions = system.get_memory_regions();
        assert!(regions.len() >= 5);

        let ram_region = regions
            .iter()
            .find(|r| r.name == "RAM" && r.start == 0x0200);
        assert!(ram_region.is_some());
    }

    #[test]
    fn test_c64_cpu_state() {
        let system = C64System::new();
        let state = system.get_cpu_state();

        assert!(state.registers.iter().any(|r| r.name == "A"));
        assert!(state.registers.iter().any(|r| r.name == "X"));
        assert!(state.registers.iter().any(|r| r.name == "Y"));
        assert!(state.registers.iter().any(|r| r.name == "SP"));

        assert_eq!(state.flags.flags.len(), 7);
        let flag_names: Vec<&str> = state.flags.flags.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(flag_names, vec!["N", "V", "B", "D", "I", "Z", "C"]);
    }

    #[test]
    fn test_c64_read_memory() {
        let system = C64System::new();
        let memory = system.read_memory(0x0000, 16);
        assert!(memory.is_some());
        assert_eq!(memory.unwrap().len(), 16);

        let invalid = system.read_memory(0x10000, 1);
        assert!(invalid.is_none());
    }

    #[test]
    fn test_c64_disassemble() {
        let mut system = C64System::new();
        // Write a known instruction to RAM
        use emu_core::cpu_6502::Memory6502;
        system.cpu.memory.write(0x0200, 0xA9); // LDA #$42
        system.cpu.memory.write(0x0201, 0x42);

        let instr = system.disassemble_instruction(0x0200);
        assert!(instr.is_some());
        let instr = instr.unwrap();
        assert!(!instr.mnemonic.is_empty());
        assert_eq!(instr.address, 0x0200);
    }

    #[test]
    fn test_c64_debugger_trait_object() {
        let system = C64System::new();
        // Verify the System trait exposes the debugger
        let debugger = system.debugger();
        assert!(debugger.is_some());
    }
}
