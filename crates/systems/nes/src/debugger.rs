//! NES system debugger implementation.
//!
//! Provides debug introspection for the NES system including disassembly,
//! memory inspection, and CPU state tracking.

use crate::bus::Bus;
use emu_core::debug::{CpuRegister, CpuState, Debugger, DisassembledInstruction, MemoryRegion};
use emu_core::disasm_6502;

use crate::NesSystem;

impl Debugger for NesSystem {
    fn disassemble_instruction(&self, address: u32) -> Option<DisassembledInstruction> {
        // Read up to 3 bytes for the instruction (max 6502 instruction size)
        let memory = self.read_memory(address, 3)?;
        disasm_6502::disassemble_6502(&memory, address)
    }

    fn read_memory(&self, address: u32, length: usize) -> Option<Vec<u8>> {
        // 6502 has 16-bit address space
        if address > 0xFFFF {
            return None;
        }

        let mut result = Vec::with_capacity(length);
        let bus = self.cpu.bus()?;

        for i in 0..length {
            let addr = address.wrapping_add(i as u32);
            if addr > 0xFFFF {
                break;
            }
            result.push(bus.read(addr as u16));
        }

        Some(result)
    }

    fn get_memory_regions(&self) -> Vec<MemoryRegion> {
        vec![
            // CPU address space
            MemoryRegion::new(
                "Internal RAM",
                0x0000,
                0x07FF,
                "2KB internal RAM (mirrored to 0x1FFF)",
                true,
                true,
            ),
            MemoryRegion::new(
                "PPU Registers",
                0x2000,
                0x2007,
                "PPU control, status, and data registers (mirrored to 0x3FFF)",
                true,
                true,
            ),
            MemoryRegion::new(
                "APU/IO Registers",
                0x4000,
                0x4017,
                "APU and I/O registers",
                true,
                true,
            ),
            MemoryRegion::new(
                "Expansion Area",
                0x4020,
                0x7FFF,
                "Cartridge expansion area (mapper-specific)",
                true,
                false,
            ),
            MemoryRegion::new(
                "PRG ROM",
                0x8000,
                0xFFFF,
                "Program ROM (32KB, mapper-banked)",
                true,
                false,
            ),
        ]
    }

    fn get_cpu_state(&self) -> CpuState {
        // Note: CpuState stores the program counter both as a dedicated `pc` field
        // (set via CpuState::new) and as a register entry named "PC".
        // The `pc` field is used by the debugger for navigation (current instruction),
        // while the "PC" register is added below so it also appears in the generic
        // registers list shown in the UI.
        let mut state = CpuState::new(self.cpu.pc() as u32);

        // Add registers (including PC for display in the registers panel)
        state.add_register(CpuRegister::new_16bit("PC", self.cpu.pc()));
        state.add_register(CpuRegister::new_8bit("A", self.cpu.a()));
        state.add_register(CpuRegister::new_8bit("X", self.cpu.x()));
        state.add_register(CpuRegister::new_8bit("Y", self.cpu.y()));
        state.add_register(CpuRegister::new_8bit("SP", self.cpu.sp()));

        // Add status flags (NV-BDIZC format)
        // Bit 7 (0x80): N (Negative)
        // Bit 6 (0x40): V (Overflow)
        // Bit 5: Unused (always 1)
        // Bit 4 (0x10): B (Break)
        // Bit 3 (0x08): D (Decimal - unused on NES)
        // Bit 2 (0x04): I (IRQ Disable)
        // Bit 1 (0x02): Z (Zero)
        // Bit 0 (0x01): C (Carry)
        let status = self.cpu.status();
        state.add_flag("N", (status & 0x80) != 0); // Negative
        state.add_flag("V", (status & 0x40) != 0); // Overflow
        state.add_flag("B", (status & 0x10) != 0); // Break
        state.add_flag("D", (status & 0x08) != 0); // Decimal (unused on NES)
        state.add_flag("I", (status & 0x04) != 0); // IRQ Disable
        state.add_flag("Z", (status & 0x02) != 0); // Zero
        state.add_flag("C", (status & 0x01) != 0); // Carry

        state
    }

    emu_core::impl_debugger_execution_history!();
}

#[cfg(test)]
mod tests {
    use super::*;
    use emu_core::System;

    #[test]
    fn test_nes_memory_regions() {
        let system = NesSystem::default();
        let regions = system.get_memory_regions();

        // Should have at least the basic CPU memory regions
        assert!(regions.len() >= 4);

        // Check that Internal RAM region exists
        let ram_region = regions.iter().find(|r| r.name == "Internal RAM");
        assert!(ram_region.is_some());
        let ram = ram_region.unwrap();
        assert_eq!(ram.start, 0x0000);
        assert_eq!(ram.end, 0x07FF);
        assert!(ram.readable);
        assert!(ram.writable);
    }

    #[test]
    fn test_nes_cpu_state() {
        let system = NesSystem::default();
        let state = system.get_cpu_state();

        // Should have PC register
        assert_eq!(state.pc as u16, system.cpu.pc());

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
    fn test_nes_cpu_flags_extraction() {
        let system = NesSystem::default();
        let state = system.get_cpu_state();

        // Verify all 7 flags exist and are booleans
        assert_eq!(state.flags.flags.len(), 7);

        // After reset, status should be 0x24 (I flag set, unused bit set)
        // Verify the flags are extracted correctly
        for (name, value) in &state.flags.flags {
            match name.as_str() {
                "N" | "V" | "B" | "D" | "Z" | "C" => {
                    // These should be false (bits 7, 6, 4, 3, 1, 0)
                    assert!(!value, "Flag {} should be false after reset", name);
                }
                "I" => {
                    // I flag (bit 2) should be true after reset (0x24 & 0x04 = 0x04)
                    assert!(*value, "I flag should be true after reset");
                }
                _ => unreachable!("Unexpected flag: {}", name),
            }
        }
    }

    #[test]
    fn test_nes_read_memory() {
        let system = NesSystem::default();

        // Should be able to read from valid addresses
        let memory = system.read_memory(0x0000, 16);
        assert!(memory.is_some());
        assert_eq!(memory.unwrap().len(), 16);

        // Should return None for invalid addresses
        let invalid = system.read_memory(0x10000, 1);
        assert!(invalid.is_none());
    }

    #[test]
    fn test_nes_disassemble() {
        let mut system = NesSystem::default();

        // Mount a simple test ROM that has some instructions
        let test_rom = create_minimal_test_rom();
        let _ = system.mount("Cartridge", &test_rom);

        // Try to disassemble from reset vector area
        if let Some(instr) = system.disassemble_instruction(0x8000) {
            // Should get a valid instruction
            assert!(!instr.bytes.is_empty());
            assert!(!instr.mnemonic.is_empty());
        }
    }

    /// Create a minimal iNES ROM for testing
    fn create_minimal_test_rom() -> Vec<u8> {
        let mut rom = Vec::new();

        // iNES header
        rom.extend_from_slice(b"NES\x1A"); // Magic
        rom.push(1); // 1 x 16KB PRG ROM
        rom.push(0); // 0 x 8KB CHR ROM (use CHR RAM)
        rom.push(0); // Mapper 0, vertical mirroring
        rom.push(0); // Mapper 0 upper bits
        rom.extend_from_slice(&[0; 8]); // Padding

        // PRG ROM (16KB) with some simple code
        let mut prg = vec![0; 16384];
        // Put a simple program at the start
        prg[0] = 0xA9; // LDA #$10
        prg[1] = 0x10;
        prg[2] = 0x4C; // JMP $8000
        prg[3] = 0x00;
        prg[4] = 0x80;

        // Reset vector points to $8000
        prg[0x3FFC] = 0x00;
        prg[0x3FFD] = 0x80;

        rom.extend_from_slice(&prg);

        rom
    }
}
