//! GBA system debugger implementation.
//!
//! Provides debug introspection for the GBA system including ARM/Thumb disassembly,
//! memory inspection, and CPU state tracking.

use emu_core::cpu_arm7tdmi::MemoryArm7;
use emu_core::debug::{CpuRegister, CpuState, Debugger, DisassembledInstruction, MemoryRegion};
use emu_core::disasm_arm7tdmi;

use crate::GbaSystem;

impl Debugger for GbaSystem {
    fn disassemble_instruction(&self, address: u32) -> Option<DisassembledInstruction> {
        let is_thumb = self.cpu.is_thumb();

        if is_thumb {
            // Thumb instructions are 2 bytes
            let memory = self.read_memory(address, 2)?;
            if memory.len() < 2 {
                return None;
            }
            let instr = u16::from_le_bytes([memory[0], memory[1]]);
            let mnemonic = disasm_arm7tdmi::disassemble_thumb(instr, address);
            Some(DisassembledInstruction::new(
                address,
                memory[..2].to_vec(),
                mnemonic,
            ))
        } else {
            // ARM instructions are 4 bytes
            let memory = self.read_memory(address, 4)?;
            if memory.len() < 4 {
                return None;
            }
            let instr = u32::from_le_bytes([memory[0], memory[1], memory[2], memory[3]]);
            let mnemonic = disasm_arm7tdmi::disassemble_arm(instr, address);
            Some(DisassembledInstruction::new(
                address,
                memory[..4].to_vec(),
                mnemonic,
            ))
        }
    }

    fn read_memory(&self, address: u32, length: usize) -> Option<Vec<u8>> {
        let mut result = Vec::with_capacity(length);

        for i in 0..length {
            let addr = address.wrapping_add(i as u32);
            result.push(self.cpu.memory.read_byte(addr));
        }

        Some(result)
    }

    fn get_memory_regions(&self) -> Vec<MemoryRegion> {
        let mut regions = vec![
            MemoryRegion::new(
                "BIOS",
                0x0000_0000,
                0x0000_3FFF,
                "System ROM (16KB, read-only, protected after boot)",
                true,
                false,
            ),
            MemoryRegion::new(
                "EWRAM",
                0x0200_0000,
                0x0203_FFFF,
                "External Work RAM (256KB, 2 wait states)",
                true,
                true,
            ),
            MemoryRegion::new(
                "IWRAM",
                0x0300_0000,
                0x0300_7FFF,
                "Internal Work RAM (32KB, fast, 0 wait states)",
                true,
                true,
            ),
            MemoryRegion::new(
                "I/O",
                0x0400_0000,
                0x0400_03FE,
                "I/O Registers (LCD, Sound, DMA, Timers, Serial, Keypad, System)",
                true,
                true,
            ),
            MemoryRegion::new(
                "Palette",
                0x0500_0000,
                0x0500_03FF,
                "Palette RAM (1KB: BG palette 0x000-0x1FF, OBJ palette 0x200-0x3FF)",
                true,
                true,
            ),
            MemoryRegion::new(
                "VRAM",
                0x0600_0000,
                0x0601_7FFF,
                "Video RAM (96KB: BG tiles/maps + OBJ tiles)",
                true,
                true,
            ),
            MemoryRegion::new(
                "OAM",
                0x0700_0000,
                0x0700_03FF,
                "Object Attribute Memory (1KB: 128 sprites × 8 bytes)",
                true,
                true,
            ),
        ];

        // ROM region - sized to actual ROM
        let rom_end = if self.cpu.memory.rom.is_empty() {
            0x0800_0000
        } else {
            0x0800_0000 + (self.cpu.memory.rom.len() as u32).saturating_sub(1)
        };

        regions.push(MemoryRegion::new(
            "ROM",
            0x0800_0000,
            rom_end,
            format!(
                "Game Pak ROM ({} KB, mirrored at 0x0A000000 and 0x0C000000)",
                self.cpu.memory.rom.len() / 1024
            ),
            true,
            false,
        ));

        regions.push(MemoryRegion::new(
            "SRAM",
            0x0E00_0000,
            0x0E00_FFFF,
            "Game Pak SRAM (64KB, battery-backed save RAM)",
            true,
            true,
        ));

        regions
    }

    fn get_cpu_state(&self) -> CpuState {
        let pc = self.cpu.pc();
        let mut state = CpuState::new(pc);

        // Program Counter
        state.add_register(CpuRegister::new_32bit("PC", pc));

        // General purpose registers R0-R12
        for i in 0..=12 {
            state.add_register(CpuRegister::new_32bit(format!("R{}", i), self.cpu.gpr[i]));
        }

        // SP (R13), LR (R14)
        state.add_register(CpuRegister::new_32bit("SP", self.cpu.gpr[13]));
        state.add_register(CpuRegister::new_32bit("LR", self.cpu.gpr[14]));

        // CPSR
        state.add_register(CpuRegister::new_32bit("CPSR", self.cpu.cpsr));

        // Status flags from CPSR
        state.add_flag("N", (self.cpu.cpsr & (1 << 31)) != 0); // Negative
        state.add_flag("Z", (self.cpu.cpsr & (1 << 30)) != 0); // Zero
        state.add_flag("C", (self.cpu.cpsr & (1 << 29)) != 0); // Carry
        state.add_flag("V", (self.cpu.cpsr & (1 << 28)) != 0); // Overflow
        state.add_flag("I", (self.cpu.cpsr & (1 << 7)) != 0); // IRQ disable
        state.add_flag("F", (self.cpu.cpsr & (1 << 6)) != 0); // FIQ disable
        state.add_flag("T", (self.cpu.cpsr & (1 << 5)) != 0); // Thumb state

        // Mode
        let mode = self.cpu.current_mode();
        state.add_flag(format!("Mode={:?}", mode), true);

        state
    }

    emu_core::impl_debugger_execution_history!();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cartridge::GbaCartridgeHeader;
    use emu_core::System;

    /// Create a minimal GBA test ROM
    fn create_test_rom() -> Vec<u8> {
        let mut rom = vec![0u8; 0x200];

        // Entry point: B 0 (infinite loop in ARM)
        rom[0] = 0xFE;
        rom[1] = 0xFF;
        rom[2] = 0xFF;
        rom[3] = 0xEA;

        // Nintendo logo (not needed for loading, but for header test)
        // Skip logo for minimal test

        // Fixed value at 0xB2
        rom[0xB2] = 0x96;

        // Title: "TEST"
        rom[0xA0] = b'T';
        rom[0xA1] = b'E';
        rom[0xA2] = b'S';
        rom[0xA3] = b'T';

        // Game code: "AXXE"
        rom[0xAC] = b'A';
        rom[0xAD] = b'X';
        rom[0xAE] = b'X';
        rom[0xAF] = b'E';

        // Maker code: "01"
        rom[0xB0] = b'0';
        rom[0xB1] = b'1';

        // Compute complement checksum
        let mut sum: u8 = 0;
        for &byte in &rom[0xA0..0xBD] {
            sum = sum.wrapping_add(byte);
        }
        rom[0xBD] = (-(sum as i8).wrapping_add(0x19)) as u8;

        rom
    }

    #[test]
    fn test_gba_memory_regions() {
        let system = GbaSystem::default();
        let regions = system.get_memory_regions();

        // Should have BIOS, EWRAM, IWRAM, I/O, Palette, VRAM, OAM, ROM, SRAM
        assert!(regions.len() >= 9);

        // Check BIOS region
        let bios = regions.iter().find(|r| r.name == "BIOS");
        assert!(bios.is_some());
        let bios = bios.unwrap();
        assert_eq!(bios.start, 0x0000_0000);
        assert_eq!(bios.end, 0x0000_3FFF);
        assert!(bios.readable);
        assert!(!bios.writable);

        // Check IWRAM region
        let iwram = regions.iter().find(|r| r.name == "IWRAM");
        assert!(iwram.is_some());
        let iwram = iwram.unwrap();
        assert_eq!(iwram.start, 0x0300_0000);
        assert_eq!(iwram.end, 0x0300_7FFF);
        assert!(iwram.readable);
        assert!(iwram.writable);

        // Check VRAM region
        let vram = regions.iter().find(|r| r.name == "VRAM");
        assert!(vram.is_some());
        let vram = vram.unwrap();
        assert_eq!(vram.start, 0x0600_0000);
        assert_eq!(vram.end, 0x0601_7FFF);
    }

    #[test]
    fn test_gba_cpu_state() {
        let system = GbaSystem::default();
        let state = system.get_cpu_state();

        // Should have PC register
        assert!(state.registers.iter().any(|r| r.name == "PC"));

        // Should have all GP registers R0-R12
        for i in 0..=12 {
            let name = format!("R{}", i);
            assert!(
                state.registers.iter().any(|r| r.name == name),
                "Missing register {}",
                name
            );
        }

        // Should have SP, LR, CPSR
        assert!(state.registers.iter().any(|r| r.name == "SP"));
        assert!(state.registers.iter().any(|r| r.name == "LR"));
        assert!(state.registers.iter().any(|r| r.name == "CPSR"));

        // Should have ARM7TDMI flags
        let flag_names: Vec<&str> = state
            .flags
            .flags
            .iter()
            .map(|(name, _)| name.as_str())
            .collect();
        assert!(flag_names.contains(&"N"));
        assert!(flag_names.contains(&"Z"));
        assert!(flag_names.contains(&"C"));
        assert!(flag_names.contains(&"V"));
        assert!(flag_names.contains(&"I"));
        assert!(flag_names.contains(&"F"));
        assert!(flag_names.contains(&"T"));
    }

    #[test]
    fn test_gba_cpu_flags_after_reset() {
        let system = GbaSystem::default();
        let state = system.get_cpu_state();

        // After GBA reset: system mode (0x1F), ARM state, IRQ+FIQ enabled
        // But note: GbaSystem::new() sets CPSR to 0x1F (System mode) after mount
        // Default Arm7Tdmi::new() sets CPSR to Supervisor + I + F disabled

        for (name, value) in &state.flags.flags {
            if name.as_str() == "T" {
                assert!(!value, "Should be in ARM state (T=0)");
            }
        }
    }

    #[test]
    fn test_gba_read_memory() {
        let mut system = GbaSystem::default();

        // Write to IWRAM and read back
        system.cpu.memory.iwram[0] = 0x42;
        system.cpu.memory.iwram[1] = 0x43;

        let data = system.read_memory(0x0300_0000, 4);
        assert!(data.is_some());
        let data = data.unwrap();
        assert_eq!(data[0], 0x42);
        assert_eq!(data[1], 0x43);
    }

    #[test]
    fn test_gba_disassemble_arm() {
        let mut system = GbaSystem::default();

        // Write an ARM NOP (MOV R0, R0 = 0xE1A00000) to IWRAM
        let nop: u32 = 0xE1A0_0000;
        let bytes = nop.to_le_bytes();
        for (i, &b) in bytes.iter().enumerate() {
            system.cpu.memory.iwram[i] = b;
        }

        // Ensure CPU is in ARM mode
        system.cpu.cpsr &= !(1 << 5); // Clear T bit

        let instr = system.disassemble_instruction(0x0300_0000);
        assert!(instr.is_some());
        let instr = instr.unwrap();
        assert_eq!(instr.bytes.len(), 4);
        assert_eq!(instr.address, 0x0300_0000);
    }

    #[test]
    fn test_gba_disassemble_thumb() {
        let mut system = GbaSystem::default();

        // Write a Thumb NOP (MOV R8, R8 = 0x46C0) to IWRAM
        let nop: u16 = 0x46C0;
        let bytes = nop.to_le_bytes();
        system.cpu.memory.iwram[0] = bytes[0];
        system.cpu.memory.iwram[1] = bytes[1];

        // Set CPU to Thumb mode
        system.cpu.cpsr |= 1 << 5; // Set T bit

        let instr = system.disassemble_instruction(0x0300_0000);
        assert!(instr.is_some());
        let instr = instr.unwrap();
        assert_eq!(instr.bytes.len(), 2);
        assert_eq!(instr.address, 0x0300_0000);
    }

    #[test]
    fn test_cartridge_header_parsing() {
        let rom = create_test_rom();
        let header = GbaCartridgeHeader::from_bytes(&rom);
        assert!(header.is_some());
        let header = header.unwrap();
        assert_eq!(header.title, "TEST");
        assert_eq!(header.game_code, "AXXE");
        assert_eq!(header.maker_code, "01");
    }

    #[test]
    fn test_mount_and_debug() {
        let mut system = GbaSystem::default();
        let rom = create_test_rom();

        // Mount should succeed
        let result = system.mount("Cartridge", &rom);
        assert!(result.is_ok());

        // Should be able to read ROM via debugger
        let data = system.read_memory(0x0800_0000, 4);
        assert!(data.is_some());
        let data = data.unwrap();
        // Entry point bytes
        assert_eq!(data[0], 0xFE);

        // CPU state should show ROM entry point
        let state = system.get_cpu_state();
        assert_eq!(state.pc, 0x0800_0000);

        // Should have cartridge header info
        assert!(system.cartridge_header().is_some());
    }
}
