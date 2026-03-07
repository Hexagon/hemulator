//! Debugger implementation for the Game & Watch system.

use emu_core::debug::{CpuRegister, CpuState, DisassembledInstruction, MemoryRegion};

use crate::GameAndWatchSystem;

impl emu_core::debug::Debugger for GameAndWatchSystem {
    fn disassemble_instruction(&self, address: u32) -> Option<DisassembledInstruction> {
        if address as usize >= self.cpu.rom.len() {
            return None;
        }

        let (mnemonic, len) = crate::sm510::Sm510::disassemble(&self.cpu.rom, address as u16);

        let mut bytes = vec![self.cpu.rom[address as usize]];
        if len == 2 && (address as usize + 1) < self.cpu.rom.len() {
            bytes.push(self.cpu.rom[address as usize + 1]);
        }

        Some(DisassembledInstruction::new(address, bytes, mnemonic))
    }

    fn read_memory(&self, address: u32, length: usize) -> Option<Vec<u8>> {
        // Memory map:
        // 0x0000-0x0FFF: ROM (4 KB)
        // 0x1000-0x107F: RAM (128 nibbles, stored as bytes)
        let mut result = Vec::with_capacity(length);
        for i in 0..length {
            let addr = address + i as u32;
            let byte = if addr < 0x1000 {
                // ROM
                if (addr as usize) < self.cpu.rom.len() {
                    self.cpu.rom[addr as usize]
                } else {
                    0
                }
            } else if addr < 0x1080 {
                // RAM (nibbles stored as bytes)
                let ram_addr = (addr - 0x1000) as usize;
                if ram_addr < 128 {
                    self.cpu.ram[ram_addr]
                } else {
                    0
                }
            } else {
                0
            };
            result.push(byte);
        }
        Some(result)
    }

    fn get_memory_regions(&self) -> Vec<MemoryRegion> {
        vec![
            MemoryRegion::new("ROM", 0x0000, 0x0FFF, "Program ROM (4 KB)", true, false),
            MemoryRegion::new(
                "RAM",
                0x1000,
                0x107F,
                "128 × 4-bit RAM (nibbles)",
                true,
                true,
            ),
        ]
    }

    fn get_cpu_state(&self) -> CpuState {
        let mut state = CpuState::new(self.cpu.pc as u32);

        state.add_register(CpuRegister::new("PC", self.cpu.pc as u32, 12));
        state.add_register(CpuRegister::new_8bit("ACC", self.cpu.acc));
        state.add_register(CpuRegister::new_8bit("BL", self.cpu.bl));
        state.add_register(CpuRegister::new_8bit("BM", self.cpu.bm));
        state.add_register(CpuRegister::new_16bit("Stack", self.cpu.stack));
        state.add_register(CpuRegister::new_16bit("Divider", self.cpu.divider));
        state.add_register(CpuRegister::new_8bit("S", self.cpu.output_s));
        state.add_register(CpuRegister::new_8bit("R", self.cpu.output_r));
        state.add_register(CpuRegister::new_8bit("K", self.cpu.input_k));

        state.add_flag("C", self.cpu.carry);
        state.add_flag("SBM", self.cpu.sbm);
        state.add_flag("Skip", self.cpu.skip);
        state.add_flag("Halt", self.cpu.halted);
        state.add_flag("F1", self.cpu.f1_flag);
        state.add_flag("F4", self.cpu.f4_flag);
        state.add_flag("Melody", self.cpu.melody_enabled);
        state.add_flag("Buzzer", self.cpu.buzzer_active);

        state
    }
}
