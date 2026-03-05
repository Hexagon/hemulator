//! PS1 system debugger implementation.
//!
//! Provides debug introspection for the PlayStation 1 system including
//! MIPS R3000A disassembly, memory inspection, and CPU/GTE state tracking.

use emu_core::cpu_mips_r3000a::{MemoryR3000A, COP0_CAUSE, COP0_EPC, COP0_SR};
use emu_core::debug::{CpuRegister, CpuState, Debugger, DisassembledInstruction, MemoryRegion};
use emu_core::disasm_mips_r3000a;

use crate::Ps1System;

impl Debugger for Ps1System {
    fn disassemble_instruction(&self, address: u32) -> Option<DisassembledInstruction> {
        // MIPS R3000A instructions are always 4 bytes, little-endian
        let memory = self.read_memory(address, 4)?;
        if memory.len() < 4 {
            return None;
        }
        disasm_mips_r3000a::disassemble_r3000a(&memory, address)
    }

    fn read_memory(&self, address: u32, length: usize) -> Option<Vec<u8>> {
        let bus = self.bus();
        let mut result = Vec::with_capacity(length);

        for i in 0..length {
            let addr = address.wrapping_add(i as u32);
            // Translate KSEG0/KSEG1 virtual addresses to physical
            let phys = match addr {
                0x8000_0000..=0x9FFF_FFFF => addr & 0x1FFF_FFFF, // KSEG0: strip bit 31
                0xA000_0000..=0xBFFF_FFFF => addr & 0x1FFF_FFFF, // KSEG1: strip bits 31-29
                _ => addr,                                       // KUSEG or already physical
            };
            result.push(bus.read_byte(phys));
        }

        Some(result)
    }

    fn get_memory_regions(&self) -> Vec<MemoryRegion> {
        vec![
            MemoryRegion::new(
                "Main RAM",
                0x0000_0000,
                0x001F_FFFF,
                "2 MB main RAM (mirrored 4× to 0x007FFFFF)",
                true,
                true,
            ),
            MemoryRegion::new(
                "Scratchpad",
                0x1F80_0000,
                0x1F80_03FF,
                "1 KB data cache used as fast scratchpad",
                true,
                true,
            ),
            MemoryRegion::new(
                "I/O Ports",
                0x1F80_1000,
                0x1F80_2FFF,
                "Hardware registers (GPU, DMA, timers, CD-ROM, SPU, etc.)",
                true,
                true,
            ),
            MemoryRegion::new(
                "BIOS ROM",
                0x1FC0_0000,
                0x1FC7_FFFF,
                "512 KB BIOS ROM",
                true,
                false,
            ),
            // KSEG0 mirrors (cached)
            MemoryRegion::new(
                "KSEG0 RAM",
                0x8000_0000,
                0x801F_FFFF,
                "KSEG0 cached mirror of main RAM",
                true,
                true,
            ),
            MemoryRegion::new(
                "KSEG0 BIOS",
                0x9FC0_0000,
                0x9FC7_FFFF,
                "KSEG0 cached mirror of BIOS ROM",
                true,
                false,
            ),
            // KSEG1 mirrors (uncached)
            MemoryRegion::new(
                "KSEG1 RAM",
                0xA000_0000,
                0xA01F_FFFF,
                "KSEG1 uncached mirror of main RAM",
                true,
                true,
            ),
            MemoryRegion::new(
                "KSEG1 BIOS",
                0xBFC0_0000,
                0xBFC7_FFFF,
                "KSEG1 uncached mirror of BIOS ROM (reset vector entry)",
                true,
                false,
            ),
        ]
    }

    fn get_cpu_state(&self) -> CpuState {
        let pc = self.cpu.pc;
        let mut state = CpuState::new(pc);

        // Program Counter
        state.add_register(CpuRegister::new_32bit("PC", pc));

        // General Purpose Registers
        for i in 0..32u32 {
            let name = match i {
                0 => "R0/zero",
                1 => "R1/at",
                2 => "R2/v0",
                3 => "R3/v1",
                4 => "R4/a0",
                5 => "R5/a1",
                6 => "R6/a2",
                7 => "R7/a3",
                8 => "R8/t0",
                9 => "R9/t1",
                10 => "R10/t2",
                11 => "R11/t3",
                12 => "R12/t4",
                13 => "R13/t5",
                14 => "R14/t6",
                15 => "R15/t7",
                16 => "R16/s0",
                17 => "R17/s1",
                18 => "R18/s2",
                19 => "R19/s3",
                20 => "R20/s4",
                21 => "R21/s5",
                22 => "R22/s6",
                23 => "R23/s7",
                24 => "R24/t8",
                25 => "R25/t9",
                26 => "R26/k0",
                27 => "R27/k1",
                28 => "R28/gp",
                29 => "R29/sp",
                30 => "R30/fp",
                31 => "R31/ra",
                _ => unreachable!(),
            };
            state.add_register(CpuRegister::new_32bit(name, self.cpu.gpr[i as usize]));
        }

        // HI/LO
        state.add_register(CpuRegister::new_32bit("HI", self.cpu.hi));
        state.add_register(CpuRegister::new_32bit("LO", self.cpu.lo));

        // Key COP0 registers
        let sr = self.cpu.cop0[COP0_SR];
        let cause = self.cpu.cop0[COP0_CAUSE];
        let epc = self.cpu.cop0[COP0_EPC];

        state.add_register(CpuRegister::new_32bit("SR", sr));
        state.add_register(CpuRegister::new_32bit("Cause", cause));
        state.add_register(CpuRegister::new_32bit("EPC", epc));

        // Status Register flags
        state.add_flag("IEc", (sr & 0x01) != 0); // Current Interrupt Enable
        state.add_flag("KUc", (sr & 0x02) != 0); // Current Kernel/User mode
        state.add_flag("IEp", (sr & 0x04) != 0); // Previous Interrupt Enable
        state.add_flag("KUp", (sr & 0x08) != 0); // Previous Kernel/User
        state.add_flag("IEo", (sr & 0x10) != 0); // Old Interrupt Enable
        state.add_flag("KUo", (sr & 0x20) != 0); // Old Kernel/User
        state.add_flag("IsC", (sr & (1 << 16)) != 0); // Isolate Cache
        state.add_flag("BEV", (sr & (1 << 22)) != 0); // Boot Exception Vectors

        state
    }

    emu_core::impl_debugger_execution_history!();
}

#[cfg(test)]
mod tests {
    use super::*;
    use emu_core::System;

    #[test]
    fn test_ps1_memory_regions() {
        let system = Ps1System::default();
        let regions = system.get_memory_regions();

        // Should have RAM, scratchpad, I/O, BIOS, KSEG mirrors
        assert!(regions.len() >= 6);

        let ram = regions.iter().find(|r| r.name == "Main RAM");
        assert!(ram.is_some());
        let ram = ram.unwrap();
        assert_eq!(ram.start, 0x0000_0000);
        assert_eq!(ram.end, 0x001F_FFFF);
        assert!(ram.readable);
        assert!(ram.writable);

        let bios = regions.iter().find(|r| r.name == "BIOS ROM");
        assert!(bios.is_some());
        let bios = bios.unwrap();
        assert_eq!(bios.start, 0x1FC0_0000);
        assert!(bios.readable);
        assert!(!bios.writable);
    }

    #[test]
    fn test_ps1_cpu_state() {
        let system = Ps1System::default();
        let state = system.get_cpu_state();

        // PC should be at reset vector
        assert_eq!(state.pc, 0xBFC0_0000);

        // Should have all 32 GPRs plus PC, HI, LO, SR, Cause, EPC = 38 registers
        assert_eq!(state.registers.len(), 38);

        // Check some register names
        assert!(state.registers.iter().any(|r| r.name == "PC"));
        assert!(state.registers.iter().any(|r| r.name == "R0/zero"));
        assert!(state.registers.iter().any(|r| r.name == "R29/sp"));
        assert!(state.registers.iter().any(|r| r.name == "R31/ra"));
        assert!(state.registers.iter().any(|r| r.name == "HI"));
        assert!(state.registers.iter().any(|r| r.name == "LO"));
        assert!(state.registers.iter().any(|r| r.name == "SR"));

        // Check flags from Status Register
        assert!(state.flags.flags.len() >= 6);
        let flag_names: Vec<&str> = state.flags.flags.iter().map(|(n, _)| n.as_str()).collect();
        assert!(flag_names.contains(&"IEc"));
        assert!(flag_names.contains(&"BEV"));
    }

    #[test]
    fn test_ps1_read_memory() {
        let mut system = Ps1System::new();
        // Write some values into RAM
        system.bus_mut().ram[0] = 0xAB;
        system.bus_mut().ram[1] = 0xCD;
        system.bus_mut().ram[2] = 0xEF;
        system.bus_mut().ram[3] = 0x12;

        // Read from physical RAM address
        let data = system.read_memory(0x0000_0000, 4).unwrap();
        assert_eq!(data, vec![0xAB, 0xCD, 0xEF, 0x12]);

        // Read from KSEG0 mirror
        let data = system.read_memory(0x8000_0000, 4).unwrap();
        assert_eq!(data, vec![0xAB, 0xCD, 0xEF, 0x12]);

        // Read from KSEG1 mirror
        let data = system.read_memory(0xA000_0000, 4).unwrap();
        assert_eq!(data, vec![0xAB, 0xCD, 0xEF, 0x12]);
    }

    #[test]
    fn test_ps1_disassemble() {
        let mut system = Ps1System::new();
        // Write a LUI instruction at address 0x1000
        // LUI $t0, 0x8000 => (0x0F << 26) | (8 << 16) | 0x8000
        let word = (0x0Fu32 << 26) | (8 << 16) | 0x8000;
        let bytes = word.to_le_bytes();
        system.bus_mut().ram[0x1000] = bytes[0];
        system.bus_mut().ram[0x1001] = bytes[1];
        system.bus_mut().ram[0x1002] = bytes[2];
        system.bus_mut().ram[0x1003] = bytes[3];

        let instr = system.disassemble_instruction(0x1000).unwrap();
        assert_eq!(instr.address, 0x1000);
        assert_eq!(instr.len(), 4);
        assert!(instr.mnemonic.contains("LUI"));
        assert!(instr.mnemonic.contains("t0"));
    }

    #[test]
    fn test_ps1_disassemble_range() {
        let mut system = Ps1System::new();
        // Write 3 NOP instructions
        for i in 0..3 {
            let offset = 0x1000 + i * 4;
            system.bus_mut().ram[offset] = 0;
            system.bus_mut().ram[offset + 1] = 0;
            system.bus_mut().ram[offset + 2] = 0;
            system.bus_mut().ram[offset + 3] = 0;
        }

        let instrs = system.disassemble_range(0x1000, 3);
        assert_eq!(instrs.len(), 3);
        for (i, instr) in instrs.iter().enumerate() {
            assert_eq!(instr.address, 0x1000 + (i as u32) * 4);
            assert_eq!(instr.mnemonic, "NOP");
        }
    }

    #[test]
    fn test_ps1_debugger_via_system_trait() {
        let system = Ps1System::default();
        let debugger = system.debugger();
        assert!(debugger.is_some(), "PS1 system should provide a debugger");
    }
}
