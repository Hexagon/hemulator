//! Mega Drive / Genesis system integration
//!
//! Ties together the M68000 CPU, VDP, YM2612, PSG, and bus.

use crate::bus::MdBus;
use crate::m68k::{M68k, Memory68k};
use emu_core::debug::{CpuRegister, CpuState, Debugger, DisassembledInstruction, MemoryRegion};
use emu_core::types::Frame;
use emu_core::{MountPointInfo, System};
use serde_json::Value;
use thiserror::Error;

/// Mega Drive emulator errors
#[derive(Debug, Error)]
pub enum MegaDriveError {
    #[error("Invalid mount point")]
    InvalidMountPoint,
}

/// NTSC timing constants
const M68K_CLOCK: u64 = 7_670_453; // ~7.67 MHz
const NTSC_SCANLINES: u64 = 262;
const PAL_SCANLINES: u64 = 313;
const NTSC_FPS: u64 = 60;
const PAL_FPS: u64 = 50;
const NTSC_CYCLES_PER_FRAME: u64 = M68K_CLOCK / NTSC_FPS;
const PAL_CYCLES_PER_FRAME: u64 = M68K_CLOCK / PAL_FPS;

/// Sega Mega Drive / Genesis emulator
pub struct MegaDriveSystem {
    cpu: M68k<MdBus>,
    cartridge_loaded: bool,
    total_cycles: u64,

    // Debugging
    pub(crate) instruction_tracer: emu_core::instruction_tracer::InstructionTracer,
    pub(crate) breakpoint_manager: emu_core::breakpoints::BreakpointManager,
}

impl MegaDriveSystem {
    pub fn new() -> Self {
        let bus = MdBus::new();
        Self {
            cpu: M68k::new(bus),
            cartridge_loaded: false,
            total_cycles: 0,
            instruction_tracer: emu_core::instruction_tracer::InstructionTracer::new(),
            breakpoint_manager: emu_core::breakpoints::BreakpointManager::new(),
        }
    }

    /// Load a ROM
    pub fn load_rom(&mut self, data: &[u8]) {
        self.cpu.memory.load_rom(data);
        self.cartridge_loaded = true;
        self.reset();
    }

    /// Set controller 1 state
    /// Bits: 0=Up, 1=Down, 2=Left, 3=Right, 4=B, 5=C, 6=A, 7=Start
    /// Active LOW — 0 = pressed, 1 = released
    pub fn set_controller_1(&mut self, state: u16) {
        self.cpu.memory.controller_1 = state;
    }

    /// Set controller 2 state
    pub fn set_controller_2(&mut self, state: u16) {
        self.cpu.memory.controller_2 = state;
    }

    /// Get audio samples (interleaved stereo i16)
    pub fn get_audio_samples(&mut self, count: usize) -> Vec<i16> {
        // Mix YM2612 and PSG
        let fm_samples = self.cpu.memory.ym2612.generate_samples(count);
        let psg_samples = self.cpu.memory.psg.generate_samples(count);

        let mut output = Vec::with_capacity(count * 2);
        for i in 0..count {
            // FM is stereo (2 samples per frame), PSG is mono
            let fm_l = if i * 2 < fm_samples.len() {
                fm_samples[i * 2] as i32
            } else {
                0
            };
            let fm_r = if i * 2 + 1 < fm_samples.len() {
                fm_samples[i * 2 + 1] as i32
            } else {
                0
            };
            let psg = if i < psg_samples.len() {
                psg_samples[i] as i32
            } else {
                0
            };

            // Mix: FM + PSG (PSG is quieter)
            let psg_scaled = psg / 2;
            let left = (fm_l + psg_scaled).clamp(-32768, 32767) as i16;
            let right = (fm_r + psg_scaled).clamp(-32768, 32767) as i16;
            output.push(left);
            output.push(right);
        }
        output
    }

    emu_core::impl_instruction_tracer_methods!();

    pub fn is_instruction_tracing_enabled(&self) -> bool {
        self.instruction_tracer.is_enabled()
    }

    emu_core::impl_breakpoint_methods!();

    pub fn set_breakpoints_enabled(&mut self, enabled: bool) {
        self.breakpoint_manager.set_enabled(enabled);
    }

    pub fn get_breakpoint_manager(&self) -> &emu_core::breakpoints::BreakpointManager {
        &self.breakpoint_manager
    }

    pub fn check_breakpoint(&self) -> Option<u32> {
        let pc = self.cpu.pc;
        if self.breakpoint_manager.should_break_execute(pc) {
            Some(pc)
        } else {
            None
        }
    }
}

impl Default for MegaDriveSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl System for MegaDriveSystem {
    type Error = MegaDriveError;

    fn reset(&mut self) {
        self.cpu.memory.reset();
        self.cpu.reset();
        self.total_cycles = 0;
    }

    fn step_frame(&mut self) -> Result<Frame, Self::Error> {
        let is_pal = self.cpu.memory.region_pal;
        let scanlines = if is_pal { PAL_SCANLINES } else { NTSC_SCANLINES };
        let cycles_per_frame = if is_pal { PAL_CYCLES_PER_FRAME } else { NTSC_CYCLES_PER_FRAME };
        let cycles_per_scanline = cycles_per_frame / scanlines;
        let frame_start = self.cpu.cycles;

        for scanline in 0..scanlines {
            let target = frame_start + (scanline + 1) * cycles_per_scanline;

            // Run M68K until we've used up this scanline's cycles
            while self.cpu.cycles < target {
                // Check breakpoints
                if self.breakpoint_manager.is_enabled() {
                    if let Some(_pc) = self.check_breakpoint() {
                        if self.instruction_tracer.is_enabled() {
                            let instr = self.make_trace_instr(self.cpu.pc);
                            let state = self.make_trace_state();
                            self.instruction_tracer.trace(instr, state);
                        }
                        break;
                    }
                }

                let pc_before = self.cpu.pc;
                self.cpu.step();

                // Trace instructions
                if self.instruction_tracer.is_enabled() {
                    let instr = self.make_trace_instr(pc_before);
                    let state = self.make_trace_state();
                    self.instruction_tracer.trace(instr, state);
                }
            }

            self.total_cycles = self.cpu.cycles;

            // VDP: Render this scanline
            self.cpu
                .memory
                .vdp
                .borrow_mut()
                .set_scanline(scanline as u16);

            // Handle VDP interrupts
            if self.cpu.memory.vdp.borrow().vint_pending() {
                self.cpu.interrupt(6); // VBlank is level 6
                self.cpu.memory.vdp.borrow_mut().clear_vint();
            }
            if self.cpu.memory.vdp.borrow().hint_pending() {
                self.cpu.interrupt(4); // HBlank is level 4
                self.cpu.memory.vdp.borrow_mut().clear_hint();
            }
        }

        // Get frame from VDP
        let frame = self.cpu.memory.vdp.borrow().get_frame().clone();

        Ok(frame)
    }

    fn save_state(&self) -> Value {
        serde_json::json!({
            "system": "megadrive",
            "version": 1,
            "total_cycles": self.total_cycles,
            "cpu": {
                "d": self.cpu.d.to_vec(),
                "a": self.cpu.a.to_vec(),
                "pc": self.cpu.pc,
                "sr": self.cpu.sr,
                "usp": self.cpu.usp,
                "ssp": self.cpu.ssp,
                "halted": self.cpu.halted,
                "stopped": self.cpu.stopped,
            },
            "ram": self.cpu.memory.ram.clone(),
            "z80_ram": self.cpu.memory.z80_ram.clone(),
            "vdp": self.cpu.memory.vdp.borrow().get_state(),
            "ym2612": self.cpu.memory.ym2612.get_state(),
            "psg": self.cpu.memory.psg.get_state(),
        })
    }

    fn load_state(&mut self, state: &Value) -> Result<(), serde_json::Error> {
        if let Some(tc) = state.get("total_cycles").and_then(|v| v.as_u64()) {
            self.total_cycles = tc;
        }

        // CPU
        if let Some(cpu) = state.get("cpu") {
            if let Some(d) = cpu.get("d").and_then(|v| v.as_array()) {
                for (i, val) in d.iter().enumerate() {
                    if i < 8 {
                        self.cpu.d[i] = val.as_u64().unwrap_or(0) as u32;
                    }
                }
            }
            if let Some(a) = cpu.get("a").and_then(|v| v.as_array()) {
                for (i, val) in a.iter().enumerate() {
                    if i < 8 {
                        self.cpu.a[i] = val.as_u64().unwrap_or(0) as u32;
                    }
                }
            }
            if let Some(v) = cpu.get("pc").and_then(|v| v.as_u64()) {
                self.cpu.pc = v as u32;
            }
            if let Some(v) = cpu.get("sr").and_then(|v| v.as_u64()) {
                self.cpu.sr = v as u16;
            }
            if let Some(v) = cpu.get("usp").and_then(|v| v.as_u64()) {
                self.cpu.usp = v as u32;
            }
            if let Some(v) = cpu.get("ssp").and_then(|v| v.as_u64()) {
                self.cpu.ssp = v as u32;
            }
            if let Some(v) = cpu.get("halted").and_then(|v| v.as_bool()) {
                self.cpu.halted = v;
            }
            if let Some(v) = cpu.get("stopped").and_then(|v| v.as_bool()) {
                self.cpu.stopped = v;
            }
        }

        // RAM
        if let Some(ram) = state.get("ram").and_then(|v| v.as_array()) {
            self.cpu.memory.ram = ram
                .iter()
                .filter_map(|v| v.as_u64().map(|n| n as u8))
                .collect();
        }
        if let Some(z80_ram) = state.get("z80_ram").and_then(|v| v.as_array()) {
            self.cpu.memory.z80_ram = z80_ram
                .iter()
                .filter_map(|v| v.as_u64().map(|n| n as u8))
                .collect();
        }

        // VDP
        if let Some(vdp_state) = state.get("vdp") {
            self.cpu.memory.vdp.borrow_mut().set_state(vdp_state)?;
        }

        // YM2612
        if let Some(ym_state) = state.get("ym2612") {
            self.cpu.memory.ym2612.set_state(ym_state)?;
        }

        // PSG
        if let Some(psg_state) = state.get("psg") {
            self.cpu.memory.psg.set_state(psg_state)?;
        }

        Ok(())
    }

    fn mount_points(&self) -> Vec<MountPointInfo> {
        vec![MountPointInfo {
            id: "cartridge".to_string(),
            name: "Cartridge".to_string(),
            extensions: vec![
                "md".to_string(),
                "gen".to_string(),
                "bin".to_string(),
                "smd".to_string(),
            ],
            required: true,
        }]
    }

    fn mount(&mut self, mount_point_id: &str, data: &[u8]) -> Result<(), Self::Error> {
        match mount_point_id {
            "cartridge" => {
                self.load_rom(data);
                Ok(())
            }
            _ => Err(MegaDriveError::InvalidMountPoint),
        }
    }

    fn unmount(&mut self, mount_point_id: &str) -> Result<(), Self::Error> {
        match mount_point_id {
            "cartridge" => {
                self.cpu.memory.rom.clear();
                self.cartridge_loaded = false;
                Ok(())
            }
            _ => Err(MegaDriveError::InvalidMountPoint),
        }
    }

    fn is_mounted(&self, mount_point_id: &str) -> bool {
        match mount_point_id {
            "cartridge" => self.cartridge_loaded,
            _ => false,
        }
    }

    fn debugger(&self) -> Option<&dyn emu_core::debug::Debugger> {
        Some(self)
    }

    fn get_total_cycles(&self) -> u64 {
        self.total_cycles
    }
}

impl Debugger for MegaDriveSystem {
    fn disassemble_instruction(&self, address: u32) -> Option<DisassembledInstruction> {
        if address > 0x00FF_FFFF {
            return None;
        }
        let (mnemonic, len) = self.cpu.disassemble(address);
        let mut bytes = Vec::with_capacity(len as usize);
        for i in 0..len {
            bytes.push(self.cpu.memory.read_byte(address + i));
        }
        Some(DisassembledInstruction::new(address, bytes, mnemonic))
    }

    fn read_memory(&self, address: u32, length: usize) -> Option<Vec<u8>> {
        if address > 0x00FF_FFFF {
            return None;
        }
        let mut result = Vec::with_capacity(length);
        for i in 0..length {
            let addr = address.wrapping_add(i as u32) & 0x00FF_FFFF;
            result.push(self.cpu.memory.read_byte(addr));
        }
        Some(result)
    }

    fn get_memory_regions(&self) -> Vec<MemoryRegion> {
        let rom_end = if self.cpu.memory.rom.is_empty() {
            0x3FFFFF
        } else {
            (self.cpu.memory.rom.len() as u32 - 1).min(0x3FFFFF)
        };
        vec![
            MemoryRegion::new(
                "Cartridge ROM",
                0x000000,
                rom_end,
                format!("Cartridge ROM ({}KB)", self.cpu.memory.rom.len() / 1024),
                true,
                false,
            ),
            MemoryRegion::new(
                "Z80 RAM",
                0xA00000,
                0xA01FFF,
                "8KB Z80 sound RAM",
                true,
                true,
            ),
            MemoryRegion::new(
                "I/O Registers",
                0xA10000,
                0xA1001F,
                "I/O area (controllers, Z80 bus control)",
                true,
                true,
            ),
            MemoryRegion::new(
                "VDP",
                0xC00000,
                0xC0001F,
                "VDP data, control, HV counter, PSG",
                true,
                true,
            ),
            MemoryRegion::new(
                "Work RAM",
                0xFF0000,
                0xFFFFFF,
                "64KB 68K work RAM",
                true,
                true,
            ),
        ]
    }

    fn get_cpu_state(&self) -> CpuState {
        let mut state = CpuState::new(self.cpu.pc);
        state.add_register(CpuRegister::new_32bit("PC", self.cpu.pc));
        for i in 0..8 {
            state.add_register(CpuRegister::new_32bit(format!("D{}", i), self.cpu.d[i]));
        }
        for i in 0..8 {
            state.add_register(CpuRegister::new_32bit(format!("A{}", i), self.cpu.a[i]));
        }
        state.add_register(CpuRegister::new_16bit("SR", self.cpu.sr));
        state.add_register(CpuRegister::new_32bit("USP", self.cpu.usp));
        state.add_register(CpuRegister::new_32bit("SSP", self.cpu.ssp));

        // Flags
        state.add_flag("C", self.cpu.sr & 0x0001 != 0);
        state.add_flag("V", self.cpu.sr & 0x0002 != 0);
        state.add_flag("Z", self.cpu.sr & 0x0004 != 0);
        state.add_flag("N", self.cpu.sr & 0x0008 != 0);
        state.add_flag("X", self.cpu.sr & 0x0010 != 0);
        state.add_flag("S", self.cpu.sr & 0x2000 != 0);
        state.add_flag("T", self.cpu.sr & 0x8000 != 0);

        state
    }
}

impl MegaDriveSystem {
    fn make_trace_instr(&self, pc: u32) -> DisassembledInstruction {
        let (mnemonic, len) = self.cpu.disassemble(pc);
        let mut bytes = Vec::new();
        for i in 0..len {
            bytes.push(self.cpu.memory.read_byte(pc + i));
        }
        DisassembledInstruction::new(pc, bytes, mnemonic)
    }

    fn make_trace_state(&self) -> CpuState {
        let mut state = CpuState::new(self.cpu.pc);
        for i in 0..8 {
            state.add_register(CpuRegister::new_32bit(format!("D{}", i), self.cpu.d[i]));
        }
        for i in 0..8 {
            state.add_register(CpuRegister::new_32bit(format!("A{}", i), self.cpu.a[i]));
        }
        state.add_register(CpuRegister::new_32bit("PC", self.cpu.pc));
        state.add_register(CpuRegister::new_16bit("SR", self.cpu.sr));
        state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_creation() {
        let system = MegaDriveSystem::new();
        let mounts = system.mount_points();
        assert_eq!(mounts.len(), 1);
        assert_eq!(mounts[0].id, "cartridge");
        assert!(mounts[0].required);
    }

    #[test]
    fn test_mount_unmount() {
        let mut system = MegaDriveSystem::new();
        assert!(!system.is_mounted("cartridge"));

        // Create a minimal valid ROM: needs vectors and code within bounds
        let mut rom = vec![0u8; 1024];
        // Initial SSP at $000000: 0x00FF0000
        rom[0] = 0x00;
        rom[1] = 0xFF;
        rom[2] = 0x00;
        rom[3] = 0x00;
        // Initial PC at $000004: 0x00000200
        rom[4] = 0x00;
        rom[5] = 0x00;
        rom[6] = 0x02;
        rom[7] = 0x00;
        // NOP at $200
        rom[0x200] = 0x4E;
        rom[0x201] = 0x71;
        // Another NOP at $202 (for prefetch after first NOP)
        rom[0x202] = 0x4E;
        rom[0x203] = 0x71;

        system.mount("cartridge", &rom).unwrap();
        assert!(system.is_mounted("cartridge"));

        system.unmount("cartridge").unwrap();
        assert!(!system.is_mounted("cartridge"));
    }

    /// Build a test ROM with M68K code that initializes the VDP and DMAs tile data
    fn build_vdp_test_rom() -> Vec<u8> {
        let mut rom = vec![0u8; 0x1000]; // 4KB ROM

        // Exception vector table
        // $000: SSP = $00FF0000
        rom[0] = 0x00;
        rom[1] = 0xFF;
        rom[2] = 0x00;
        rom[3] = 0x00;
        // $004: PC = $00000200
        rom[4] = 0x00;
        rom[5] = 0x00;
        rom[6] = 0x02;
        rom[7] = 0x00;
        // Fill interrupt vectors with a loop (JMP self) at $100
        for i in (8..0x100).step_by(4) {
            rom[i] = 0x00;
            rom[i + 1] = 0x00;
            rom[i + 2] = 0x01;
            rom[i + 3] = 0x00;
        }
        // At $100: JMP $100 (infinite loop for unused vectors)
        rom[0x100] = 0x4E;
        rom[0x101] = 0xF9; // JMP (xxx).L
        rom[0x102] = 0x00;
        rom[0x103] = 0x00;
        rom[0x104] = 0x01;
        rom[0x105] = 0x00;

        // Tile data at $300: a simple 8x8 tile (32 bytes, 4bpp)
        // Row 0: pixel color 1 for all 8 pixels = 0x11 0x11 0x11 0x11
        for row in 0..8 {
            let base = 0x300 + row * 4;
            rom[base] = 0x11;
            rom[base + 1] = 0x11;
            rom[base + 2] = 0x11;
            rom[base + 3] = 0x11;
        }

        // Code at $200:
        let mut pc = 0x200;

        // Helper: write_vdp_reg(reg, val) = write word $8000 | (reg << 8) | val to $C00004
        // We'll use MOVE.W #imm,($C00004).L encoding:
        // 33FC xxxx 00C0 0004

        let write_reg = |rom: &mut Vec<u8>, pc: &mut usize, reg: u8, val: u8| {
            let cmd: u16 = 0x8000 | ((reg as u16) << 8) | val as u16;
            // MOVE.W #cmd, ($C00004).L
            rom[*pc] = 0x33;
            rom[*pc + 1] = 0xFC; // MOVE.W #imm, (xxx).L
            rom[*pc + 2] = (cmd >> 8) as u8;
            rom[*pc + 3] = cmd as u8;
            rom[*pc + 4] = 0x00;
            rom[*pc + 5] = 0xC0;
            rom[*pc + 6] = 0x00;
            rom[*pc + 7] = 0x04;
            *pc += 8;
        };

        let write_vdp_ctrl_word = |rom: &mut Vec<u8>, pc: &mut usize, val: u16| {
            // MOVE.W #val, ($C00004).L
            rom[*pc] = 0x33;
            rom[*pc + 1] = 0xFC;
            rom[*pc + 2] = (val >> 8) as u8;
            rom[*pc + 3] = val as u8;
            rom[*pc + 4] = 0x00;
            rom[*pc + 5] = 0xC0;
            rom[*pc + 6] = 0x00;
            rom[*pc + 7] = 0x04;
            *pc += 8;
        };

        let write_vdp_data_word = |rom: &mut Vec<u8>, pc: &mut usize, val: u16| {
            // MOVE.W #val, ($C00000).L
            rom[*pc] = 0x33;
            rom[*pc + 1] = 0xFC;
            rom[*pc + 2] = (val >> 8) as u8;
            rom[*pc + 3] = val as u8;
            rom[*pc + 4] = 0x00;
            rom[*pc + 5] = 0xC0;
            rom[*pc + 6] = 0x00;
            rom[*pc + 7] = 0x00;
            *pc += 8;
        };

        // 1. Set up VDP registers
        write_reg(&mut rom, &mut pc, 0, 0x04); // R0: HInt disabled
        write_reg(&mut rom, &mut pc, 1, 0x74); // R1: Display ON, VInt ON, DMA ON, 224 lines
        write_reg(&mut rom, &mut pc, 2, 0x30); // R2: Plane A at $C000 (0x30 << 10 = $C000)
        write_reg(&mut rom, &mut pc, 3, 0x3C); // R3: Window at $F000
        write_reg(&mut rom, &mut pc, 4, 0x07); // R4: Plane B at $E000
        write_reg(&mut rom, &mut pc, 5, 0x6C); // R5: Sprite table at $D800
        write_reg(&mut rom, &mut pc, 7, 0x00); // R7: Backdrop color = palette 0, color 0
        write_reg(&mut rom, &mut pc, 10, 0xFF); // R10: HInt counter = 255
        write_reg(&mut rom, &mut pc, 11, 0x00); // R11: Full screen scroll
        write_reg(&mut rom, &mut pc, 12, 0x81); // R12: H40 mode (320 pixels)
        write_reg(&mut rom, &mut pc, 13, 0x37); // R13: HScroll at $DC00
        write_reg(&mut rom, &mut pc, 15, 0x02); // R15: Auto-increment = 2
        write_reg(&mut rom, &mut pc, 16, 0x01); // R16: Plane size = 64x32

        // 2. Write a color to CRAM (palette entry 1 = green)
        // Control port: set CRAM write to address $0002 (palette entry 1)
        // CRAM write code = 0b11 in bits 15-14, address in bits 0-13
        // Word 1: bits 15-14 = code bits 1-0 = 11, bits 13-0 = addr bits 13-0
        //   addr = $0002, code = 0b0011 → word1 = 0xC002
        // Word 2: code bits 3-2 in bits 5-4. code=3 → bits 3-2 = 0b00 → word2 = $0000
        write_vdp_ctrl_word(&mut rom, &mut pc, 0xC002); // First word
        write_vdp_ctrl_word(&mut rom, &mut pc, 0x0000); // Second word

        // Data port write: green = $0E0 (BGR format: 0000_BBB0_GGG0_RRR0 → green = bits 5-1)
        // Actually MD CRAM format: ----BBB-GGG-RRR- so green = 0x00E0
        write_vdp_data_word(&mut rom, &mut pc, 0x00E0);

        // 3. Write tile data directly to VRAM at address $0020 (tile 1)
        // VRAM write: code=0b0001, address=$0020
        // Word 1: bits 15-14 = 01, bits 13-0 = addr = $0020 → 0x4020
        // Word 2: code bits 3-2 in bits 5-4. code=1 → bits 3-2 = 0b00 → 0x0000
        write_vdp_ctrl_word(&mut rom, &mut pc, 0x4020); // First word: VRAM write at $0020
        write_vdp_ctrl_word(&mut rom, &mut pc, 0x0000); // Second word

        // Write 32 bytes of tile data (8 words): all pixels = color 1
        for _ in 0..8 {
            write_vdp_data_word(&mut rom, &mut pc, 0x1111);
        }

        // 4. Write to Plane A nametable at $C000 to place tile 1
        // VRAM write at $C000: Word1 = $4000 | ($C000 & $3FFF) = $4000, Word2 = ($C000 >> 14) << 0 = $0003
        write_vdp_ctrl_word(&mut rom, &mut pc, 0x4000); // First word
        write_vdp_ctrl_word(&mut rom, &mut pc, 0x0003); // Second word: upper addr bits

        // Nametable entry: priority=0, palette=0, vflip=0, hflip=0, tile=1
        // = $0001
        write_vdp_data_word(&mut rom, &mut pc, 0x0001);

        // 5. Infinite loop: JMP to self
        // 4EF9 00xx xxxx = JMP (xxx).L
        rom[pc] = 0x4E;
        rom[pc + 1] = 0xF9;
        let loop_addr = pc as u32;
        rom[pc + 2] = (loop_addr >> 24) as u8;
        rom[pc + 3] = (loop_addr >> 16) as u8;
        rom[pc + 4] = (loop_addr >> 8) as u8;
        rom[pc + 5] = loop_addr as u8;

        rom
    }

    #[test]
    fn test_vdp_renders_tile() {
        let rom = build_vdp_test_rom();
        let mut system = MegaDriveSystem::new();
        system.load_rom(&rom);

        // Verify reset vectors loaded correctly
        assert_eq!(system.cpu.pc, 0x200, "PC should start at $200");
        assert_eq!(system.cpu.a[7], 0x00FF0000, "A7/SSP should be $00FF0000");

        // Execute several frames to let the init code run
        for _ in 0..5 {
            system.step_frame().unwrap();
        }

        // Check VDP state
        let vdp = system.cpu.memory.vdp.borrow();

        // Verify register writes happened
        assert_eq!(
            vdp.regs[1] & 0x40,
            0x40,
            "Display should be enabled (reg 1 bit 6)"
        );
        assert_eq!(vdp.regs[15], 0x02, "Auto-increment should be 2");

        // Verify CRAM has our green color at entry 1
        // CRAM entry 1 is at byte offset 2 (2 bytes per entry)
        let cram_val = ((vdp.cram[2] as u16) << 8) | vdp.cram[3] as u16;
        assert_eq!(
            cram_val, 0x00E0,
            "CRAM entry 1 should be green ($00E0), got ${:04X}",
            cram_val
        );

        // Verify tile data was written to VRAM at $0020
        let tile_word = ((vdp.vram[0x20] as u16) << 8) | vdp.vram[0x21] as u16;
        assert_ne!(
            tile_word, 0x0000,
            "Tile data at VRAM $0020 should not be empty"
        );

        // Verify nametable entry at $C000
        let nt_entry = ((vdp.vram[0xC000] as u16) << 8) | vdp.vram[0xC001] as u16;
        assert_eq!(
            nt_entry, 0x0001,
            "Nametable at $C000 should reference tile 1, got ${:04X}",
            nt_entry
        );

        // Verify frame has non-black pixels
        let frame = vdp.get_frame();
        let has_non_black = frame.pixels.iter().any(|&p| p != 0xFF000000 && p != 0);
        assert!(
            has_non_black,
            "Frame should contain non-black pixels after VDP setup"
        );

        drop(vdp);
    }

    /// Test DMA from ROM to VRAM (how real games load tile data)
    #[test]
    fn test_dma_rom_to_vram() {
        let mut rom = vec![0u8; 0x2000]; // 8KB

        // Vectors
        rom[0] = 0x00;
        rom[1] = 0xFF;
        rom[2] = 0x00;
        rom[3] = 0x00; // SSP
        rom[4] = 0x00;
        rom[5] = 0x00;
        rom[6] = 0x02;
        rom[7] = 0x00; // PC
                       // Fill interrupt vectors
        for i in (8..0x100).step_by(4) {
            rom[i] = 0x00;
            rom[i + 1] = 0x00;
            rom[i + 2] = 0x01;
            rom[i + 3] = 0x00;
        }
        rom[0x100] = 0x4E;
        rom[0x101] = 0xF9;
        rom[0x102] = 0x00;
        rom[0x103] = 0x00;
        rom[0x104] = 0x01;
        rom[0x105] = 0x00;

        // Source data at $1000: tile pattern
        for i in 0..32 {
            rom[0x1000 + i] = 0x11; // All pixels = color 1
        }

        let mut pc = 0x200;
        let write_vdp_reg = |rom: &mut Vec<u8>, pc: &mut usize, reg: u8, val: u8| {
            let cmd: u16 = 0x8000 | ((reg as u16) << 8) | val as u16;
            rom[*pc] = 0x33;
            rom[*pc + 1] = 0xFC;
            rom[*pc + 2] = (cmd >> 8) as u8;
            rom[*pc + 3] = cmd as u8;
            rom[*pc + 4] = 0x00;
            rom[*pc + 5] = 0xC0;
            rom[*pc + 6] = 0x00;
            rom[*pc + 7] = 0x04;
            *pc += 8;
        };
        let write_ctrl = |rom: &mut Vec<u8>, pc: &mut usize, val: u16| {
            rom[*pc] = 0x33;
            rom[*pc + 1] = 0xFC;
            rom[*pc + 2] = (val >> 8) as u8;
            rom[*pc + 3] = val as u8;
            rom[*pc + 4] = 0x00;
            rom[*pc + 5] = 0xC0;
            rom[*pc + 6] = 0x00;
            rom[*pc + 7] = 0x04;
            *pc += 8;
        };

        // Set DMA enable, display on
        write_vdp_reg(&mut rom, &mut pc, 1, 0x74); // R1: Display ON, VInt ON, DMA ON
        write_vdp_reg(&mut rom, &mut pc, 15, 0x02); // R15: Auto-inc = 2

        // Set up DMA: 16 words from ROM $1000 to VRAM $0000
        // DMA length = 16 words → regs 19,20
        write_vdp_reg(&mut rom, &mut pc, 19, 16); // R19: DMA length low = 16
        write_vdp_reg(&mut rom, &mut pc, 20, 0); // R20: DMA length high = 0
                                                 // DMA source = $1000 >> 1 = $800 → regs 21,22,23
                                                 // Source/2: $800 = $00 (low), $08 (mid), $00 (high)
        write_vdp_reg(&mut rom, &mut pc, 21, 0x00); // R21: source low
        write_vdp_reg(&mut rom, &mut pc, 22, 0x08); // R22: source mid
        write_vdp_reg(&mut rom, &mut pc, 23, 0x00); // R23: source high (bits 6-0, bit 7=0 for 68K DMA)

        // Trigger DMA: write to control port
        // VRAM write at address $0000 with DMA: code = 0b100001
        // Word 1: bits 15-14 = code[1:0] = 01, bits 13-0 = addr[13:0] = $0000
        //   → word1 = $4000
        // Word 2: bits 7 = 1 (DMA), bits 5-4 = code[3:2] = 00, bits 1-0 = addr[15:14] = 0
        //   → word2 = $0080
        write_ctrl(&mut rom, &mut pc, 0x4000);
        write_ctrl(&mut rom, &mut pc, 0x0080);

        // Loop
        rom[pc] = 0x4E;
        rom[pc + 1] = 0xF9;
        let loop_addr = pc as u32;
        rom[pc + 2] = (loop_addr >> 24) as u8;
        rom[pc + 3] = (loop_addr >> 16) as u8;
        rom[pc + 4] = (loop_addr >> 8) as u8;
        rom[pc + 5] = loop_addr as u8;

        let mut system = MegaDriveSystem::new();
        system.load_rom(&rom);

        // Run a few frames
        for _ in 0..3 {
            system.step_frame().unwrap();
        }

        let vdp = system.cpu.memory.vdp.borrow();

        // Check DMA happened — VRAM at $0000 should have tile data
        let first_word = ((vdp.vram[0] as u16) << 8) | vdp.vram[1] as u16;
        assert_eq!(
            first_word, 0x1111,
            "DMA should have copied tile data to VRAM $0000, got ${:04X}",
            first_word
        );

        // Verify all 32 bytes
        for i in 0..32 {
            assert_eq!(
                vdp.vram[i], 0x11,
                "VRAM[${:04X}] should be $11, got ${:02X}",
                i, vdp.vram[i]
            );
        }
    }

    /// Test a realistic game boot sequence (Sonic-like init pattern)
    /// This exercises: MOVE to SR, TST, BNE, LEA, MOVE.W (A0)+/(An),
    /// MOVEQ, DBF, MOVE.L to control port (2-word command), and DMA
    #[test]
    fn test_realistic_boot_sequence() {
        let mut rom = vec![0u8; 0x2000]; // 8KB

        // Exception vector table
        rom[0] = 0x00;
        rom[1] = 0xFF;
        rom[2] = 0xFE;
        rom[3] = 0x00; // SSP = $00FFFE00
        rom[4] = 0x00;
        rom[5] = 0x00;
        rom[6] = 0x02;
        rom[7] = 0x00; // PC = $000200
                       // Fill exception vectors with RTE at $100
        for i in (8..0x100).step_by(4) {
            rom[i] = 0x00;
            rom[i + 1] = 0x00;
            rom[i + 2] = 0x01;
            rom[i + 3] = 0x00;
        }
        // At $100: RTE ($4E73) then NOP pad
        rom[0x100] = 0x4E;
        rom[0x101] = 0x73; // RTE
        rom[0x102] = 0x4E;
        rom[0x103] = 0x71; // NOP

        // VDP register table at $180 (24 entries, each 2 bytes = $8Rxx format)
        let vdp_regs: [(u8, u8); 11] = [
            (0, 0x04),  // R0: HInt disabled
            (1, 0x74),  // R1: Display ON, VInt ON, DMA ON
            (2, 0x30),  // R2: Plane A at $C000
            (4, 0x07),  // R4: Plane B at $E000
            (5, 0x6C),  // R5: Sprite table at $D800
            (7, 0x01),  // R7: Backdrop = palette 0, color 1
            (10, 0xFF), // R10: HInt counter
            (11, 0x00), // R11: Full screen scroll
            (12, 0x81), // R12: H40 mode
            (15, 0x02), // R15: Auto-increment = 2
            (16, 0x01), // R16: Plane size = 64x32
        ];
        let reg_count = vdp_regs.len();
        for (i, &(reg, val)) in vdp_regs.iter().enumerate() {
            let cmd = 0x8000u16 | ((reg as u16) << 8) | val as u16;
            let off = 0x180 + i * 2;
            rom[off] = (cmd >> 8) as u8;
            rom[off + 1] = cmd as u8;
        }

        // Palette data at $1000 (color 0 = black, color 1 = white)
        rom[0x1000] = 0x00;
        rom[0x1001] = 0x00; // Color 0: black
        rom[0x1002] = 0x0E;
        rom[0x1003] = 0xEE; // Color 1: white ($0EEE)

        // Code at $200 — mimics real game boot
        let mut pc = 0x200;

        // 1. MOVE.W #$2700, SR — disable all interrupts (THE critical instruction!)
        rom[pc] = 0x46;
        rom[pc + 1] = 0xFC; // $46FC = MOVE to SR, immediate
        rom[pc + 2] = 0x27;
        rom[pc + 3] = 0x00; // $2700
        pc += 4;

        // 2. LEA ($C00004).L, A6 — VDP control port
        rom[pc] = 0x4D;
        rom[pc + 1] = 0xF9; // LEA (xxx).L, A6
        rom[pc + 2] = 0x00;
        rom[pc + 3] = 0xC0;
        rom[pc + 4] = 0x00;
        rom[pc + 5] = 0x04;
        pc += 6;

        // 3. LEA VdpRegs(PC), A0 — PC-relative pointer to register table
        // LEA d16(PC), A0 = $41FA d16
        // Displacement is relative to the extension word address (instruction + 2)
        let disp = (0x180i32 - (pc as i32 + 2)) as i16;
        rom[pc] = 0x41;
        rom[pc + 1] = 0xFA; // LEA d16(PC), A0
        rom[pc + 2] = (disp >> 8) as u8;
        rom[pc + 3] = disp as u8;
        pc += 4;

        // 4. MOVEQ #(reg_count-1), D7 — loop counter
        rom[pc] = 0x7E;
        rom[pc + 1] = (reg_count - 1) as u8; // MOVEQ #n, D7
        pc += 2;

        // 5. MOVE.W (A0)+, (A6) — write VDP register from table
        // MOVE.W (A0)+, (A6): src=mode3/reg0, dst=mode2/reg6
        // Encoding: 0011 DDD MMM SSS sss
        //   dst_reg=6, dst_mode=2, src_mode=3, src_reg=0
        //   = 0011_110_010_011_000 = $3C98
        rom[pc] = 0x3C;
        rom[pc + 1] = 0x98;
        pc += 2;

        // 6. DBF D7, -4 (loop back to MOVE.W)
        // DBF D7 = $51CF, displacement = -4
        rom[pc] = 0x51;
        rom[pc + 1] = 0xCF;
        rom[pc + 2] = 0xFF;
        rom[pc + 3] = 0xFC; // -4
        pc += 4;

        // 7. DMA palette from ROM $1000 to CRAM $0000 (2 words = 4 bytes)
        // Set DMA length = 2 words
        let write_reg = |rom: &mut Vec<u8>, pc: &mut usize, reg: u8, val: u8| {
            let cmd: u16 = 0x8000 | ((reg as u16) << 8) | val as u16;
            // MOVE.W #cmd, (A6) = $3CBC cmd
            rom[*pc] = 0x3C;
            rom[*pc + 1] = 0xBC;
            rom[*pc + 2] = (cmd >> 8) as u8;
            rom[*pc + 3] = cmd as u8;
            *pc += 4;
        };
        write_reg(&mut rom, &mut pc, 19, 2); // DMA length low = 2
        write_reg(&mut rom, &mut pc, 20, 0); // DMA length high = 0
                                             // Source = $1000 >> 1 = $800
        write_reg(&mut rom, &mut pc, 21, 0x00); // source low byte of word addr
        write_reg(&mut rom, &mut pc, 22, 0x08); // source mid
        write_reg(&mut rom, &mut pc, 23, 0x00); // source high, mode = 68K DMA

        // Trigger CRAM DMA: MOVE.L #$C0000080, (A6)
        // MOVE.L #imm, (An): src=mode7/reg4(imm), dst=mode2/reg6
        // Encoding: 0010_DDD_MMM_SSS_sss → 0010_110_010_111_100 = $2CBC
        rom[pc] = 0x2C;
        rom[pc + 1] = 0xBC;
        // Immediate long: $C0000080 (CRAM write at $0000 + DMA flag)
        rom[pc + 2] = 0xC0;
        rom[pc + 3] = 0x00;
        rom[pc + 4] = 0x00;
        rom[pc + 5] = 0x80;
        pc += 6;

        // 8. Infinite loop: BRA.S -2
        rom[pc] = 0x60;
        rom[pc + 1] = 0xFE;

        // Run the system
        let mut system = MegaDriveSystem::new();
        system.load_rom(&rom);

        assert_eq!(system.cpu.pc, 0x200, "PC should start at $200");

        // Run several frames
        for _ in 0..5 {
            system.step_frame().unwrap();
        }

        // Verify CPU state
        assert_eq!(
            system.cpu.sr & 0xFF00,
            0x2700 & 0xFF00,
            "SR upper byte should reflect MOVE.W #$2700,SR"
        );

        // Verify VDP registers
        let vdp = system.cpu.memory.vdp.borrow();
        assert_eq!(
            vdp.regs[1] & 0x40,
            0x40,
            "Display should be enabled (R1 bit 6)"
        );
        assert_eq!(vdp.regs[12] & 0x81, 0x81, "H40 mode should be set (R12)");
        assert_eq!(vdp.regs[7], 0x01, "Backdrop should be palette 0 color 1");
        assert_eq!(vdp.regs[15], 0x02, "Auto-increment should be 2");

        // Verify CRAM DMA worked — color 1 should be $0EEE (white)
        let cram_val = ((vdp.cram[2] as u16) << 8) | vdp.cram[3] as u16;
        assert_eq!(
            cram_val, 0x0EEE,
            "CRAM entry 1 should be $0EEE (white), got ${:04X}",
            cram_val
        );

        // Verify frame has non-black pixels (backdrop = color 1 = white)
        let frame = vdp.get_frame();
        let non_black_count = frame
            .pixels
            .iter()
            .filter(|&&p| p != 0xFF000000 && p != 0)
            .count();
        assert!(
            non_black_count > 0,
            "Frame should have non-black pixels (backdrop is white)"
        );

        // Most pixels should be white (the backdrop)
        assert!(
            non_black_count > (320 * 200) as usize,
            "Most of the screen should be white backdrop, got {} non-black pixels",
            non_black_count
        );
    }

    /// Test that VDP register writes work even when control_pending is true.
    /// This is critical because games may write register commands ($8xxx) at any time,
    /// and the VDP must always treat them as register writes, not as the second word
    /// of a two-word command.
    #[test]
    fn test_vdp_register_write_during_pending() {
        let mut vdp = crate::vdp::Vdp::new();

        // Write first word of a VRAM command — sets control_pending = true
        vdp.write_control(0x4000); // First word: VRAM write at $0000

        // Now write a register command while pending is true
        // Register 15 = auto-increment = $02
        vdp.write_control(0x8F02);

        // The register write should have worked AND cleared pending
        assert_eq!(
            vdp.regs[15], 0x02,
            "Register 15 (auto-increment) should be $02 despite pending flag"
        );

        // Write a proper two-word command — should work normally now
        vdp.write_control(0x4000); // First word: VRAM at $0000
        vdp.write_control(0x0000); // Second word: complete command

        // Write some data
        vdp.write_data(0x1234);

        // Verify the data was written to VRAM
        assert_eq!(vdp.vram[0], 0x12, "VRAM byte 0 should be $12");
        assert_eq!(vdp.vram[1], 0x34, "VRAM byte 1 should be $34");
    }

    /// Test ANDI/ORI to SR properly affect the interrupt mask
    #[test]
    fn test_andi_ori_to_sr() {
        let mut rom = vec![0u8; 0x1000];

        // Vectors
        rom[0] = 0x00;
        rom[1] = 0xFF;
        rom[2] = 0x00;
        rom[3] = 0x00; // SSP
        rom[4] = 0x00;
        rom[5] = 0x00;
        rom[6] = 0x02;
        rom[7] = 0x00; // PC = $200
        for i in (8..0x100).step_by(4) {
            rom[i] = 0x00;
            rom[i + 1] = 0x00;
            rom[i + 2] = 0x01;
            rom[i + 3] = 0x00;
        }
        rom[0x100] = 0x4E;
        rom[0x101] = 0x73; // RTE

        let mut pc = 0x200;

        // 1. MOVE.W #$2700, SR (disable all interrupts)
        rom[pc] = 0x46;
        rom[pc + 1] = 0xFC;
        rom[pc + 2] = 0x27;
        rom[pc + 3] = 0x00;
        pc += 4;

        // 2. ANDI.W #$F8FF, SR (clear interrupt mask → enable interrupts)
        // Opcode $027C, immediate $F8FF
        rom[pc] = 0x02;
        rom[pc + 1] = 0x7C;
        rom[pc + 2] = 0xF8;
        rom[pc + 3] = 0xFF;
        pc += 4;

        // 3. ORI.W #$0700, SR (set interrupt mask to 7 → disable interrupts)
        // Opcode $007C, immediate $0700
        rom[pc] = 0x00;
        rom[pc + 1] = 0x7C;
        rom[pc + 2] = 0x07;
        rom[pc + 3] = 0x00;
        pc += 4;

        // 4. BRA.S -2 (infinite loop)
        rom[pc] = 0x60;
        rom[pc + 1] = 0xFE;

        let mut system = MegaDriveSystem::new();
        system.load_rom(&rom);

        // Run a frame
        system.step_frame().unwrap();

        // After ANDI #$F8FF, SR: mask should be 0 (bits 10:8 cleared)
        // After ORI #$0700, SR: mask should be 7 (bits 10:8 set back)
        // Final SR should have mask = 7 (0x0700)
        let mask = (system.cpu.sr >> 8) & 7;
        assert_eq!(
            mask, 7,
            "Interrupt mask should be 7 after ORI #$0700. SR=${:04X}",
            system.cpu.sr
        );

        // Supervisor bit should still be set
        assert!(
            system.cpu.sr & 0x2000 != 0,
            "Supervisor mode should still be active. SR=${:04X}",
            system.cpu.sr
        );
    }

    /// Test that Z80 bus request doesn't trap the CPU in an infinite loop.
    /// Every real Mega Drive game does: write $0100 to $A11100, then loops
    /// reading $A11100 until bit 0 is 0 (bus granted). Since we don't emulate
    /// the Z80, the bus must be immediately granted.
    #[test]
    fn test_z80_bus_request_grant() {
        let mut rom = vec![0u8; 0x1000];

        // Vectors
        rom[0] = 0x00;
        rom[1] = 0xFF;
        rom[2] = 0x00;
        rom[3] = 0x00; // SSP
        rom[4] = 0x00;
        rom[5] = 0x00;
        rom[6] = 0x02;
        rom[7] = 0x00; // PC = $200
        for i in (8..0x100).step_by(4) {
            rom[i] = 0x00;
            rom[i + 1] = 0x00;
            rom[i + 2] = 0x01;
            rom[i + 3] = 0x00;
        }
        rom[0x100] = 0x4E;
        rom[0x101] = 0x73; // RTE

        let mut pc = 0x200;

        // 1. MOVE.W #$2700, SR
        rom[pc] = 0x46;
        rom[pc + 1] = 0xFC;
        rom[pc + 2] = 0x27;
        rom[pc + 3] = 0x00;
        pc += 4;

        // 2. MOVE.W #$0100, ($A11100).L — request Z80 bus
        // MOVE.W #imm, (xxx).L = $33FC imm addr
        rom[pc] = 0x33;
        rom[pc + 1] = 0xFC;
        rom[pc + 2] = 0x01;
        rom[pc + 3] = 0x00;
        rom[pc + 4] = 0x00;
        rom[pc + 5] = 0xA1;
        rom[pc + 6] = 0x11;
        rom[pc + 7] = 0x00;
        pc += 8;

        // 3. BTST #0, ($A11100).L — test bus grant
        // BTST #imm, (xxx).L = $0839 bit_num addr
        rom[pc] = 0x08;
        rom[pc + 1] = 0x39; // BTST #imm, (xxx).L
        rom[pc + 2] = 0x00;
        rom[pc + 3] = 0x00; // bit number = 0
        rom[pc + 4] = 0x00;
        rom[pc + 5] = 0xA1;
        rom[pc + 6] = 0x11;
        rom[pc + 7] = 0x00; // address = $A11100
        pc += 8;

        // 4. BNE.S back to BTST (would loop if bus not granted)
        // BNE.S = $6600 disp
        rom[pc] = 0x66;
        rom[pc + 1] = 0xF6; // displacement = -10 (back to BTST)
        pc += 2;

        // 5. If we get here, bus was granted. Set D0 = $42 as a marker.
        // MOVEQ #$42, D0
        rom[pc] = 0x70;
        rom[pc + 1] = 0x42;
        pc += 2;

        // 6. Infinite loop
        rom[pc] = 0x60;
        rom[pc + 1] = 0xFE;

        let mut system = MegaDriveSystem::new();
        system.load_rom(&rom);

        // Run one frame
        system.step_frame().unwrap();

        // If the bus grant works correctly, D0 should be $42
        assert_eq!(
            system.cpu.d[0], 0x42,
            "D0 should be $42 (bus grant succeeded), got ${:08X}. \
             CPU stuck in Z80 bus wait loop!",
            system.cpu.d[0]
        );
    }

    /// Test that word-sized branch displacements use the correct base address.
    /// The displacement is relative to (instruction_address + 2), not
    /// (instruction_address + 4).
    #[test]
    fn test_bra_word_displacement() {
        let mut rom = vec![0u8; 0x1000];

        // Vectors
        rom[0] = 0x00;
        rom[1] = 0xFF;
        rom[2] = 0x00;
        rom[3] = 0x00; // SSP
        rom[4] = 0x00;
        rom[5] = 0x00;
        rom[6] = 0x02;
        rom[7] = 0x00; // PC = $200

        // Exception handler
        for i in (8..0x100).step_by(4) {
            rom[i] = 0x00;
            rom[i + 1] = 0x00;
            rom[i + 2] = 0x01;
            rom[i + 3] = 0x00;
        }
        rom[0x100] = 0x4E;
        rom[0x101] = 0x73; // RTE

        let mut pc = 0x200;

        // MOVEQ #0, D0
        rom[pc] = 0x70;
        rom[pc + 1] = 0x00;
        pc += 2; // $202

        // BRA.W $0006 — jump forward 6 bytes from (instruction_addr + 2)
        // instruction_addr = $202, so target = $204 + $0006 = $20A
        rom[pc] = 0x60;
        rom[pc + 1] = 0x00; // word displacement follows
        rom[pc + 2] = 0x00;
        rom[pc + 3] = 0x06; // displacement = +6
        pc += 4; // $206

        // These should be SKIPPED by the BRA.W:
        // MOVEQ #$11, D0 (at $206 — wrong target if off by +2)
        rom[pc] = 0x70;
        rom[pc + 1] = 0x11;
        pc += 2; // $208

        // MOVEQ #$22, D0 (at $208 — also wrong)
        rom[pc] = 0x70;
        rom[pc + 1] = 0x22;
        pc += 2; // $20A

        // Correct target: MOVEQ #$42, D0 (at $20A)
        rom[pc] = 0x70;
        rom[pc + 1] = 0x42;
        pc += 2; // $20C

        // Infinite loop
        rom[pc] = 0x60;
        rom[pc + 1] = 0xFE;

        let mut system = MegaDriveSystem::new();
        system.load_rom(&rom);
        system.step_frame().unwrap();

        assert_eq!(
            system.cpu.d[0], 0x42,
            "BRA.W landed at wrong address. D0=${:02X} (expected $42). \
             Word displacement base is likely off by 2.",
            system.cpu.d[0]
        );
    }

    /// Test DMA fill byte swap — fill byte should go to addr^1 (odd byte in each word)
    #[test]
    fn test_dma_fill_byte_swap() {
        let mut vdp = crate::vdp::Vdp::new();

        // Set up VDP registers
        vdp.write_control(0x8F02); // Register 15: auto-increment = 2
        vdp.write_control(0x8174); // Register 1: display on, VInt enable, DMA enable

        // Set DMA length = 4 words
        vdp.write_control(0x9304); // R19: DMA length low = 4
        vdp.write_control(0x9400); // R20: DMA length high = 0

        // Set DMA mode to fill (reg 23 bits 7:6 = 10)
        vdp.write_control(0x9780); // R23: DMA mode = fill

        // Set up VRAM write at address $1000 with DMA
        // Word 1: CD1:CD0=01 (VRAM write), addr[13:0]=0x1000
        vdp.write_control(0x5000); // $4000 | $1000
        // Word 2: CD5=1 (DMA), addr[15:14]=0
        vdp.write_control(0x0080);

        // Write fill value to data port (triggers DMA fill)
        vdp.write_data(0xAB00);

        // First word write at $1000: vram[$1000]=$AB, vram[$1001]=$00
        assert_eq!(vdp.vram[0x1000], 0xAB, "First word high byte");
        assert_eq!(vdp.vram[0x1001], 0x00, "First word low byte");

        // Subsequent fills: fill_byte ($AB) goes to addr^1
        // addr=$1002: vram[$1002^1=$1003]=$AB
        assert_eq!(
            vdp.vram[0x1003], 0xAB,
            "DMA fill byte should go to addr^1 (odd byte)"
        );
        // addr=$1004: vram[$1004^1=$1005]=$AB
        assert_eq!(vdp.vram[0x1005], 0xAB, "DMA fill at $1004^1=$1005");
        // addr=$1006: vram[$1006^1=$1007]=$AB
        assert_eq!(vdp.vram[0x1007], 0xAB, "DMA fill at $1006^1=$1007");

        // Even bytes at $1002, $1004, $1006 should be unchanged (0)
        assert_eq!(vdp.vram[0x1002], 0x00, "Even byte should be unchanged");
        assert_eq!(vdp.vram[0x1004], 0x00, "Even byte should be unchanged");
        assert_eq!(vdp.vram[0x1006], 0x00, "Even byte should be unchanged");
    }
}
