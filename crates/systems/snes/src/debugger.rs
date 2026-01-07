//! SNES system debugger implementation.
//!
//! Provides debug introspection for the SNES system including disassembly,
//! memory inspection, and CPU state tracking.

use crate::SnesSystem;
use emu_core::cpu_65c816::Memory65c816;
use emu_core::debug::{CpuRegister, CpuState, Debugger, DisassembledInstruction, MemoryRegion};
use emu_core::disasm_65c816;

impl Debugger for SnesSystem {
    fn disassemble_instruction(&self, address: u32) -> Option<DisassembledInstruction> {
        // Read up to 4 bytes for the instruction (max 65C816 instruction size)
        let memory = self.read_memory(address, 4)?;

        // Get current CPU flags for accurate disassembly
        let cpu = &self.cpu.cpu;
        let m_flag = (cpu.status & 0x20) != 0; // Memory/Accumulator size: 1=8-bit
        let x_flag = (cpu.status & 0x10) != 0; // Index register size: 1=8-bit

        // In emulation mode, M and X are always 1 (8-bit)
        let (m_flag, x_flag) = if cpu.emulation {
            (true, true)
        } else {
            (m_flag, x_flag)
        };

        disasm_65c816::disassemble_65c816_with_flags(&memory, address, m_flag, x_flag)
    }

    fn read_memory(&self, address: u32, length: usize) -> Option<Vec<u8>> {
        // 65C816 has 24-bit address space (16MB)
        if address > 0xFF_FFFF {
            return None;
        }

        let mut result = Vec::with_capacity(length);
        let bus = &self.cpu.cpu.memory;

        for i in 0..length {
            let addr = address.wrapping_add(i as u32);
            if addr > 0xFF_FFFF {
                break;
            }
            result.push(bus.read(addr));
        }

        Some(result)
    }

    fn get_memory_regions(&self) -> Vec<MemoryRegion> {
        vec![
            // SNES memory map (LoROM assumed for now)
            MemoryRegion::new(
                "WRAM (Low)",
                0x00_0000,
                0x00_1FFF,
                "128KB work RAM (low bank mirror)",
                true,
                true,
            ),
            MemoryRegion::new(
                "PPU/CPU Registers",
                0x00_2000,
                0x00_21FF,
                "PPU, CPU, and hardware registers",
                true,
                true,
            ),
            MemoryRegion::new(
                "APU Registers",
                0x00_2140,
                0x00_217F,
                "SPC700 communication ports",
                true,
                true,
            ),
            MemoryRegion::new(
                "WRAM (Full)",
                0x7E_0000,
                0x7F_FFFF,
                "128KB work RAM (full 64KB + 64KB banks)",
                true,
                true,
            ),
            MemoryRegion::new(
                "ROM (Bank 80+)",
                0x80_8000,
                0xFF_FFFF,
                "Cartridge ROM (LoROM upper banks)",
                true,
                false,
            ),
        ]
    }

    fn get_cpu_state(&self) -> CpuState {
        let cpu = &self.cpu.cpu;

        // Calculate the full 24-bit program counter (PBR:PC)
        let full_pc = ((cpu.pbr as u32) << 16) | (cpu.pc as u32);
        let mut state = CpuState::new(full_pc);

        // Add registers
        state.add_register(CpuRegister::new_32bit(
            "PC",
            ((cpu.pbr as u32) << 16) | (cpu.pc as u32),
        ));
        state.add_register(CpuRegister::new_16bit("C", cpu.c)); // Accumulator
        state.add_register(CpuRegister::new_16bit("X", cpu.x)); // X index
        state.add_register(CpuRegister::new_16bit("Y", cpu.y)); // Y index
        state.add_register(CpuRegister::new_16bit("S", cpu.s)); // Stack pointer
        state.add_register(CpuRegister::new_16bit("D", cpu.d)); // Direct page
        state.add_register(CpuRegister::new_8bit("DBR", cpu.dbr)); // Data bank
        state.add_register(CpuRegister::new_8bit("PBR", cpu.pbr)); // Program bank

        // Add status flags (NVmxDIZCe format for 65C816)
        // Bit 7 (0x80): N (Negative)
        // Bit 6 (0x40): V (Overflow)
        // Bit 5 (0x20): m (Memory/Accumulator size: 0=16-bit, 1=8-bit)
        // Bit 4 (0x10): x (Index register size: 0=16-bit, 1=8-bit)
        // Bit 3 (0x08): D (Decimal mode)
        // Bit 2 (0x04): I (IRQ disable)
        // Bit 1 (0x02): Z (Zero)
        // Bit 0 (0x01): C (Carry)
        let status = cpu.status;
        state.add_flag("N", (status & 0x80) != 0); // Negative
        state.add_flag("V", (status & 0x40) != 0); // Overflow
        state.add_flag("m", (status & 0x20) != 0); // Memory/Accumulator size
        state.add_flag("x", (status & 0x10) != 0); // Index register size
        state.add_flag("D", (status & 0x08) != 0); // Decimal
        state.add_flag("I", (status & 0x04) != 0); // IRQ disable
        state.add_flag("Z", (status & 0x02) != 0); // Zero
        state.add_flag("C", (status & 0x01) != 0); // Carry

        // Add emulation mode flag (special for 65C816)
        state.add_flag("e", cpu.emulation); // Emulation mode (6502 compatibility)

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
    fn test_snes_memory_regions() {
        let system = SnesSystem::default();
        let regions = system.get_memory_regions();

        // Should have at least the basic SNES memory regions
        assert!(regions.len() >= 4);

        // Check that WRAM region exists
        let wram_region = regions.iter().find(|r| r.name == "WRAM (Low)");
        assert!(wram_region.is_some());
        let wram = wram_region.unwrap();
        assert_eq!(wram.start, 0x00_0000);
        assert_eq!(wram.end, 0x00_1FFF);
        assert!(wram.readable);
        assert!(wram.writable);
    }

    #[test]
    fn test_snes_cpu_state() {
        let system = SnesSystem::default();
        let state = system.get_cpu_state();

        // Should have PC register
        let cpu = &system.cpu.cpu;
        let expected_pc = ((cpu.pbr as u32) << 16) | (cpu.pc as u32);
        assert_eq!(state.pc, expected_pc);

        // Should have all standard 65C816 registers
        assert!(state.registers.iter().any(|r| r.name == "C")); // Accumulator
        assert!(state.registers.iter().any(|r| r.name == "X")); // X index
        assert!(state.registers.iter().any(|r| r.name == "Y")); // Y index
        assert!(state.registers.iter().any(|r| r.name == "S")); // Stack pointer
        assert!(state.registers.iter().any(|r| r.name == "D")); // Direct page
        assert!(state.registers.iter().any(|r| r.name == "DBR")); // Data bank
        assert!(state.registers.iter().any(|r| r.name == "PBR")); // Program bank

        // Should have all status flags in correct order (NVmxDIZCe)
        assert_eq!(state.flags.flags.len(), 9);
        let flag_names: Vec<&str> = state
            .flags
            .flags
            .iter()
            .map(|(name, _)| name.as_str())
            .collect();
        assert_eq!(
            flag_names,
            vec!["N", "V", "m", "x", "D", "I", "Z", "C", "e"]
        );
    }

    #[test]
    fn test_snes_cpu_flags_extraction() {
        let system = SnesSystem::default();
        let state = system.get_cpu_state();

        // Verify all 9 flags exist (8 status bits + emulation mode)
        assert_eq!(state.flags.flags.len(), 9);

        // After reset, status should be 0x34 (m=1, x=1, I=1) and emulation=true
        for (name, value) in &state.flags.flags {
            match name.as_str() {
                "N" | "V" | "D" | "Z" | "C" => {
                    // These should be false after reset
                    assert!(!value, "Flag {} should be false after reset", name);
                }
                "m" | "x" | "I" => {
                    // These should be true after reset (0x34 = 0011_0100)
                    assert!(*value, "Flag {} should be true after reset", name);
                }
                "e" => {
                    // Emulation mode should be true after reset
                    assert!(*value, "Emulation flag should be true after reset");
                }
                _ => panic!("Unexpected flag: {}", name),
            }
        }
    }

    #[test]
    fn test_snes_read_memory() {
        let system = SnesSystem::default();

        // Should be able to read from valid addresses
        let memory = system.read_memory(0x00_0000, 16);
        assert!(memory.is_some());
        assert_eq!(memory.unwrap().len(), 16);

        // Should return None for invalid addresses
        let invalid = system.read_memory(0x1000_0000, 1);
        assert!(invalid.is_none());
    }

    #[test]
    fn test_snes_disassemble() {
        let system = SnesSystem::default();

        // Try to disassemble from a valid address
        if let Some(instr) = system.disassemble_instruction(0x00_8000) {
            // Should get a valid instruction
            assert!(!instr.bytes.is_empty());
            assert!(!instr.mnemonic.is_empty());
        }
    }
}
