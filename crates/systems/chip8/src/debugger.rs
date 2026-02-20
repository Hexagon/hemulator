//! CHIP-8 system debugger implementation.
//!
//! Provides debug introspection for the CHIP-8 system including disassembly,
//! memory inspection, and CPU state tracking.

use crate::Chip8System;
use emu_core::debug::{CpuRegister, CpuState, Debugger, DisassembledInstruction, MemoryRegion};
use emu_core::disasm_chip8;

impl Debugger for Chip8System {
    fn disassemble_instruction(&self, address: u32) -> Option<DisassembledInstruction> {
        // Read 2 bytes for CHIP-8 instruction
        let memory = self.read_memory(address, 2)?;
        disasm_chip8::disassemble_chip8(&memory, address)
    }

    fn read_memory(&self, address: u32, length: usize) -> Option<Vec<u8>> {
        // CHIP-8 has variable memory size depending on mode
        let memory_size = self.memory.len();
        if address as usize >= memory_size {
            return None;
        }

        let mut result = Vec::with_capacity(length);
        for i in 0..length {
            let addr = address.wrapping_add(i as u32) as usize;
            if addr >= memory_size {
                break;
            }
            result.push(self.memory[addr]);
        }

        Some(result)
    }

    fn get_memory_regions(&self) -> Vec<MemoryRegion> {
        let memory_size = self.memory.len();
        vec![
            // Interpreter/Font area
            MemoryRegion::new(
                "Interpreter",
                0x0000,
                0x01FF,
                "Reserved for interpreter (includes font data)",
                true,
                false,
            ),
            // Program area
            MemoryRegion::new(
                "Program",
                0x0200,
                (memory_size - 1) as u32,
                "Program ROM and work RAM",
                true,
                true,
            ),
        ]
    }

    fn get_cpu_state(&self) -> CpuState {
        let mut state = CpuState::new(self.pc as u32);

        // Add program counter and other primary registers
        state.add_register(CpuRegister::new_16bit("PC", self.pc));
        state.add_register(CpuRegister::new_16bit("I", self.i));
        state.add_register(CpuRegister::new_8bit("SP", self.sp));
        state.add_register(CpuRegister::new_8bit("DT", self.delay_timer));
        state.add_register(CpuRegister::new_8bit("ST", self.sound_timer));

        // Add V registers (V0-VF)
        for (idx, val) in self.v.iter().enumerate() {
            state.add_register(CpuRegister::new_8bit(format!("V{:X}", idx), *val));
        }

        // CHIP-8 doesn't have traditional CPU flags, but we can show some useful info
        state.add_flag("HighRes", self.high_res);
        state.add_flag("WaitKey", self.waiting_for_key.is_some());
        state.add_flag("ProgLoaded", self.program_loaded);
        state.add_flag("DispUpdate", self.display_updated);

        state
    }

    emu_core::impl_debugger_execution_history!();
}

#[cfg(test)]
mod tests {
    use super::*;
    use emu_core::System;

    #[test]
    fn test_chip8_memory_regions() {
        let system = Chip8System::new();
        let regions = system.get_memory_regions();

        // Should have 2 regions
        assert_eq!(regions.len(), 2);

        // Check interpreter region
        let interp_region = regions.iter().find(|r| r.name == "Interpreter");
        assert!(interp_region.is_some());
        let interp = interp_region.unwrap();
        assert_eq!(interp.start, 0x0000);
        assert_eq!(interp.end, 0x01FF);
        assert!(interp.readable);
        assert!(!interp.writable);

        // Check program region
        let prog_region = regions.iter().find(|r| r.name == "Program");
        assert!(prog_region.is_some());
        let prog = prog_region.unwrap();
        assert_eq!(prog.start, 0x0200);
        assert!(prog.readable);
        assert!(prog.writable);
    }

    #[test]
    fn test_chip8_cpu_state() {
        let system = Chip8System::new();
        let state = system.get_cpu_state();

        // Should have PC, I, SP, DT, ST, and 16 V registers
        assert!(state.registers.iter().any(|r| r.name == "PC"));
        assert!(state.registers.iter().any(|r| r.name == "I"));
        assert!(state.registers.iter().any(|r| r.name == "SP"));
        assert!(state.registers.iter().any(|r| r.name == "DT"));
        assert!(state.registers.iter().any(|r| r.name == "ST"));

        // Check V registers
        for i in 0..16 {
            let vname = format!("V{:X}", i);
            assert!(state.registers.iter().any(|r| r.name == vname));
        }

        // Should have status flags
        assert_eq!(state.flags.flags.len(), 4);
    }

    #[test]
    fn test_chip8_read_memory() {
        let system = Chip8System::new();

        // Should be able to read from valid addresses
        let memory = system.read_memory(0x0000, 16);
        assert!(memory.is_some());
        assert_eq!(memory.unwrap().len(), 16);

        // Should be able to read from program area
        let memory = system.read_memory(0x0200, 32);
        assert!(memory.is_some());
        assert_eq!(memory.unwrap().len(), 32);

        // Should return None for invalid addresses (beyond memory size)
        let invalid = system.read_memory(0x10000, 1);
        assert!(invalid.is_none());
    }

    #[test]
    fn test_chip8_disassemble() {
        let mut system = Chip8System::new();

        // Mount a simple test program with a few instructions
        let test_program = vec![
            0x00, 0xE0, // CLS
            0x61, 0x23, // LD V1, 0x23
            0xA2, 0x34, // LD I, 0x234
            0x12, 0x06, // JP 0x206
        ];
        let _ = system.mount("Program", &test_program);

        // Disassemble from program start
        let instr = system.disassemble_instruction(0x200);
        assert!(instr.is_some());
        let instr = instr.unwrap();
        assert_eq!(instr.address, 0x200);
        assert_eq!(instr.mnemonic, "CLS");

        // Disassemble second instruction
        let instr = system.disassemble_instruction(0x202);
        assert!(instr.is_some());
        assert_eq!(instr.unwrap().mnemonic, "LD V1, 23");
    }

    #[test]
    fn test_chip8_check_breakpoint() {
        let mut system = Chip8System::new();

        // Initially no breakpoint should fire
        assert!(system.check_breakpoint().is_none());

        // Add a breakpoint at the current PC (0x200 = program start)
        system.add_breakpoint(0x200);
        assert!(system.check_breakpoint().is_some());
        assert_eq!(system.check_breakpoint().unwrap(), 0x200);

        // Removing the breakpoint should stop it from firing
        system.remove_breakpoint(0x200);
        assert!(system.check_breakpoint().is_none());
    }
}
