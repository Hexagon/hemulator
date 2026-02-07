//! N64 system debugger implementation.
//!
//! Provides debug introspection for the N64 system including disassembly,
//! memory inspection, and CPU state tracking.

use crate::N64System;
use emu_core::cpu_mips_r4300i::MemoryMips;
use emu_core::debug::{CpuRegister, CpuState, Debugger, DisassembledInstruction, MemoryRegion};
use emu_core::disasm_mips_r4300i;

impl Debugger for N64System {
    fn disassemble_instruction(&self, address: u32) -> Option<DisassembledInstruction> {
        // Read 4 bytes for the MIPS instruction (fixed 32-bit size)
        let memory = self.read_memory(address, 4)?;
        disasm_mips_r4300i::disassemble_mips(&memory, address)
    }

    fn read_memory(&self, address: u32, length: usize) -> Option<Vec<u8>> {
        let mut result = Vec::with_capacity(length);

        for i in 0..length {
            let addr = address.wrapping_add(i as u32);
            result.push(self.cpu.cpu.memory.read_byte(addr));
        }

        Some(result)
    }

    fn get_memory_regions(&self) -> Vec<MemoryRegion> {
        vec![
            // N64 memory map
            MemoryRegion::new(
                "RDRAM",
                0x00000000,
                0x003FFFFF,
                "4MB RDRAM (main system memory)",
                true,
                true,
            ),
            MemoryRegion::new(
                "RDRAM Registers",
                0x03F00000,
                0x03FFFFFF,
                "RDRAM interface registers",
                true,
                true,
            ),
            MemoryRegion::new(
                "SP DMEM",
                0x04000000,
                0x04000FFF,
                "RSP Data Memory (4KB)",
                true,
                true,
            ),
            MemoryRegion::new(
                "SP IMEM",
                0x04001000,
                0x04001FFF,
                "RSP Instruction Memory (4KB)",
                true,
                true,
            ),
            MemoryRegion::new(
                "SP Registers",
                0x04040000,
                0x040FFFFF,
                "RSP control registers",
                true,
                true,
            ),
            MemoryRegion::new(
                "DP Command Registers",
                0x04100000,
                0x041FFFFF,
                "RDP command registers",
                true,
                true,
            ),
            MemoryRegion::new(
                "MI Registers",
                0x04300000,
                0x043FFFFF,
                "MIPS Interface (interrupts)",
                true,
                true,
            ),
            MemoryRegion::new(
                "VI Registers",
                0x04400000,
                0x044FFFFF,
                "Video Interface registers",
                true,
                true,
            ),
            MemoryRegion::new(
                "AI Registers",
                0x04500000,
                0x045FFFFF,
                "Audio Interface registers",
                true,
                true,
            ),
            MemoryRegion::new(
                "PI Registers",
                0x04600000,
                0x046FFFFF,
                "Peripheral Interface registers",
                true,
                true,
            ),
            MemoryRegion::new(
                "RI Registers",
                0x04700000,
                0x047FFFFF,
                "RDRAM Interface registers",
                true,
                true,
            ),
            MemoryRegion::new(
                "SI Registers",
                0x04800000,
                0x048FFFFF,
                "Serial Interface (controllers)",
                true,
                true,
            ),
            MemoryRegion::new(
                "Cartridge ROM",
                0x10000000,
                0x1FBFFFFF,
                "Cartridge ROM address space",
                true,
                false,
            ),
            MemoryRegion::new(
                "PIF ROM",
                0x1FC00000,
                0x1FC007BF,
                "PIF Boot ROM (IPL)",
                true,
                false,
            ),
            MemoryRegion::new(
                "PIF RAM",
                0x1FC007C0,
                0x1FC007FF,
                "PIF RAM (controller commands)",
                true,
                true,
            ),
        ]
    }

    fn get_cpu_state(&self) -> CpuState {
        let pc = self.cpu.cpu.pc;
        let mut state = CpuState::new(pc as u32);

        // Add general-purpose registers
        for i in 0..32 {
            let name = match i {
                0 => "zero",
                1 => "at",
                2 => "v0",
                3 => "v1",
                4 => "a0",
                5 => "a1",
                6 => "a2",
                7 => "a3",
                8 => "t0",
                9 => "t1",
                10 => "t2",
                11 => "t3",
                12 => "t4",
                13 => "t5",
                14 => "t6",
                15 => "t7",
                16 => "s0",
                17 => "s1",
                18 => "s2",
                19 => "s3",
                20 => "s4",
                21 => "s5",
                22 => "s6",
                23 => "s7",
                24 => "t8",
                25 => "t9",
                26 => "k0",
                27 => "k1",
                28 => "gp",
                29 => "sp",
                30 => "fp",
                31 => "ra",
                _ => unreachable!(),
            };
            state.add_register(CpuRegister::new_32bit(
                format!("${}", name),
                self.cpu.cpu.gpr[i] as u32,
            ));
        }

        // Add special registers
        state.add_register(CpuRegister::new_32bit("PC", self.cpu.cpu.pc as u32));
        state.add_register(CpuRegister::new_32bit("HI", self.cpu.cpu.hi as u32));
        state.add_register(CpuRegister::new_32bit("LO", self.cpu.cpu.lo as u32));

        // Add CP0 registers (key ones)
        state.add_register(CpuRegister::new_32bit(
            "CP0_Status",
            self.cpu.cpu.cp0[12] as u32,
        ));
        state.add_register(CpuRegister::new_32bit(
            "CP0_Cause",
            self.cpu.cpu.cp0[13] as u32,
        ));
        state.add_register(CpuRegister::new_32bit(
            "CP0_EPC",
            self.cpu.cpu.cp0[14] as u32,
        ));

        state
    }

    emu_core::impl_debugger_execution_history!();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore] // Requires GL context
    fn test_n64_memory_regions() {
        let system = N64System::new_for_test();
        let regions = system.get_memory_regions();

        // Should have 15 memory regions
        assert_eq!(regions.len(), 15);

        // Check RDRAM region
        let rdram = regions.iter().find(|r| r.name == "RDRAM");
        assert!(rdram.is_some());
        let rdram = rdram.unwrap();
        assert_eq!(rdram.start, 0x00000000);
        assert_eq!(rdram.end, 0x003FFFFF);
        assert!(rdram.readable);
        assert!(rdram.writable);

        // Check cartridge ROM region
        let rom = regions.iter().find(|r| r.name == "Cartridge ROM");
        assert!(rom.is_some());
        let rom = rom.unwrap();
        assert_eq!(rom.start, 0x10000000);
        assert_eq!(rom.end, 0x1FBFFFFF);
        assert!(rom.readable);
        assert!(!rom.writable);
    }

    #[test]
    #[ignore] // Requires GL context
    fn test_n64_cpu_state() {
        let system = N64System::new_for_test();
        let state = system.get_cpu_state();

        // Should have 32 general-purpose registers
        for i in 0..32 {
            let name = match i {
                0 => "$zero",
                1 => "$at",
                2 => "$v0",
                3 => "$v1",
                4 => "$a0",
                5 => "$a1",
                6 => "$a2",
                7 => "$a3",
                8 => "$t0",
                9 => "$t1",
                10 => "$t2",
                11 => "$t3",
                12 => "$t4",
                13 => "$t5",
                14 => "$t6",
                15 => "$t7",
                16 => "$s0",
                17 => "$s1",
                18 => "$s2",
                19 => "$s3",
                20 => "$s4",
                21 => "$s5",
                22 => "$s6",
                23 => "$s7",
                24 => "$t8",
                25 => "$t9",
                26 => "$k0",
                27 => "$k1",
                28 => "$gp",
                29 => "$sp",
                30 => "$fp",
                31 => "$ra",
                _ => unreachable!(),
            };
            assert!(
                state.registers.iter().any(|r| r.name == name),
                "Missing register: {}",
                name
            );
        }

        // Should have special registers
        assert!(state.registers.iter().any(|r| r.name == "PC"));
        assert!(state.registers.iter().any(|r| r.name == "HI"));
        assert!(state.registers.iter().any(|r| r.name == "LO"));

        // Should have CP0 registers
        assert!(state.registers.iter().any(|r| r.name == "CP0_Status"));
        assert!(state.registers.iter().any(|r| r.name == "CP0_Cause"));
        assert!(state.registers.iter().any(|r| r.name == "CP0_EPC"));
    }

    #[test]
    #[ignore] // Requires GL context
    fn test_n64_read_memory() {
        let system = N64System::new_for_test();

        // Should be able to read from valid addresses
        let memory = system.read_memory(0x00000000, 16);
        assert!(memory.is_some());
        assert_eq!(memory.unwrap().len(), 16);

        // Should be able to read from RDRAM
        let memory = system.read_memory(0x00100000, 32);
        assert!(memory.is_some());
        assert_eq!(memory.unwrap().len(), 32);
    }

    #[test]
    #[ignore] // Requires GL context
    fn test_n64_disassemble() {
        let system = N64System::new_for_test();

        // Try to disassemble from a valid address (should be all zeros initially)
        if let Some(instr) = system.disassemble_instruction(0x00000000) {
            // Should get a valid instruction (NOP or similar)
            assert!(!instr.bytes.is_empty());
            assert!(!instr.mnemonic.is_empty());
            assert_eq!(instr.bytes.len(), 4); // MIPS instructions are always 4 bytes
        }
    }
}
