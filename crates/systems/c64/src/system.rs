//! Commodore 64 system integration
//!
//! Ties together the MOS 6510 CPU (6502 + I/O port), VIC-II, SID, CIA 1/2, and bus.

use crate::bus::C64Bus;
use crate::cia::Cia;
use crate::sid::Sid;
use crate::vic::{Vic, PAL_CYCLES_PER_FRAME};
use emu_core::cpu_6502::{Cpu6502, Memory6502};
use emu_core::types::Frame;
use emu_core::{MountPointInfo, System};
use serde_json::Value;
use std::cell::RefCell;
use std::rc::Rc;
use thiserror::Error;

/// C64 emulator errors
#[derive(Debug, Error)]
pub enum C64Error {
    #[error("Invalid mount point: {0}")]
    InvalidMountPoint(String),
    #[error("Invalid PRG file")]
    InvalidPrg,
}

/// Debug information for the GUI
pub struct DebugInfo {
    pub pc: u16,
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub sp: u8,
    pub status: u8,
    pub raster_line: u32,
    pub io_port: u8,
}

/// Commodore 64 emulator
pub struct C64System {
    /// MOS 6510 CPU (6502 + I/O port)
    pub cpu: Cpu6502<C64Bus>,

    /// VIC-II video chip
    vic: Rc<RefCell<Vic>>,
    /// SID audio chip
    sid: Rc<RefCell<Sid>>,
    /// CIA 1 (keyboard, joystick 2, IRQ)
    cia1: Rc<RefCell<Cia>>,
    /// CIA 2 (VIC bank, serial, NMI)
    cia2: Rc<RefCell<Cia>>,

    /// Total CPU cycles executed
    total_cycles: u64,
    /// Cartridge/PRG loaded flag
    cartridge_loaded: bool,

    /// Debugging: instruction tracer
    pub(crate) instruction_tracer: emu_core::instruction_tracer::InstructionTracer,
    /// Debugging: breakpoint manager
    pub(crate) breakpoint_manager: emu_core::breakpoints::BreakpointManager,
}

impl C64System {
    pub fn new() -> Self {
        let vic = Rc::new(RefCell::new(Vic::new()));
        let sid = Rc::new(RefCell::new(Sid::new()));
        let cia1 = Rc::new(RefCell::new(Cia::new()));
        let cia2 = Rc::new(RefCell::new(Cia::new()));

        let bus = C64Bus::new(
            Rc::clone(&vic),
            Rc::clone(&sid),
            Rc::clone(&cia1),
            Rc::clone(&cia2),
        );

        let mut cpu = Cpu6502::new(bus);
        cpu.reset();

        Self {
            cpu,
            vic,
            sid,
            cia1,
            cia2,
            total_cycles: 0,
            cartridge_loaded: false,
            instruction_tracer: emu_core::instruction_tracer::InstructionTracer::new(),
            breakpoint_manager: emu_core::breakpoints::BreakpointManager::new(),
        }
    }

    /// Get debug information
    pub fn debug_info(&self) -> DebugInfo {
        DebugInfo {
            pc: self.cpu.pc,
            a: self.cpu.a,
            x: self.cpu.x,
            y: self.cpu.y,
            sp: self.cpu.sp,
            status: self.cpu.status,
            raster_line: self.vic.borrow().raster_line(),
            io_port: self.cpu.memory.io_port,
        }
    }

    /// Set joystick state for port (0=port 2/CIA1, 1=port 1/CIA2)
    /// Bits: 0=up, 1=down, 2=left, 3=right, 4=fire (active high input)
    pub fn set_controller(&mut self, port: usize, state: u8) {
        // Joystick bits are active LOW in CIA port registers
        let mut joy = 0xFF_u8;
        if state & 0x01 != 0 {
            joy &= !0x01;
        } // Up
        if state & 0x02 != 0 {
            joy &= !0x02;
        } // Down
        if state & 0x04 != 0 {
            joy &= !0x04;
        } // Left
        if state & 0x08 != 0 {
            joy &= !0x08;
        } // Right
        if state & 0x10 != 0 {
            joy &= !0x10;
        } // Fire

        match port {
            0 => self.cia1.borrow_mut().port_b &= joy, // Port 2 on CIA1 PB
            1 => self.cia2.borrow_mut().port_a &= joy & 0x1F, // Port 1 on CIA2 PA (lower 5 bits)
            _ => {}
        }
    }

    /// Get buffered audio samples (interleaved stereo i16)
    pub fn get_audio_samples(&mut self, count: usize) -> Vec<i16> {
        let mut samples = self.sid.borrow_mut().drain_samples();
        let needed = count * 2; // stereo
        if samples.len() >= needed {
            samples.truncate(needed);
        } else {
            samples.resize(needed, 0);
        }
        samples
    }

    /// Sync VIC-II's video memory from main RAM (based on CIA2 bank select)
    fn sync_vic_memory(&self) {
        let cia2_port = self.cia2.borrow().port_a;
        // VIC bank is selected by bits 0-1 of CIA2 PA (active low)
        let vic_bank = (!cia2_port & 0x03) as usize;
        let bank_start = vic_bank * 0x4000;

        let mut vic = self.vic.borrow_mut();

        // Copy 16KB bank from RAM to VIC's view
        vic.vram.clear();
        vic.vram
            .extend_from_slice(&self.cpu.memory.ram[bank_start..bank_start + 0x4000]);

        // In banks 0 and 2, the VIC sees character ROM at $1000-$1FFF within the bank
        // instead of RAM. This is a hardware quirk.
        if vic_bank == 0 || vic_bank == 2 {
            let char_rom_offset = 0x1000;
            let char_rom_len = self.cpu.memory.char_rom.len().min(0x1000);
            vic.vram[char_rom_offset..char_rom_offset + char_rom_len]
                .copy_from_slice(&self.cpu.memory.char_rom[..char_rom_len]);
        }

        // Sync color RAM
        vic.color_ram.clear();
        vic.color_ram.extend_from_slice(&self.cpu.memory.color_ram);
    }

    // --- Debugging infrastructure ---

    pub fn get_breakpoint_manager(&self) -> &emu_core::breakpoints::BreakpointManager {
        &self.breakpoint_manager
    }

    pub fn set_breakpoints_enabled(&mut self, enabled: bool) {
        self.breakpoint_manager.set_enabled(enabled);
    }

    pub fn check_breakpoint(&self) -> Option<u32> {
        let pc = self.cpu.pc as u32;
        if self.breakpoint_manager.should_break_execute(pc) {
            Some(pc)
        } else {
            None
        }
    }

    pub fn is_instruction_tracing_enabled(&self) -> bool {
        self.instruction_tracer.is_enabled()
    }

    emu_core::impl_instruction_tracer_methods!();
    emu_core::impl_breakpoint_methods!();
}

impl Default for C64System {
    fn default() -> Self {
        Self::new()
    }
}

impl System for C64System {
    type Error = C64Error;

    fn reset(&mut self) {
        self.vic.borrow_mut().reset();
        self.sid.borrow_mut().reset();
        self.cia1.borrow_mut().reset();
        self.cia2.borrow_mut().reset();
        self.cpu.reset();
        self.total_cycles = 0;
    }

    fn step_frame(&mut self) -> Result<Frame, Self::Error> {
        // Sync VIC memory at frame start
        self.sync_vic_memory();

        let target_cycles = PAL_CYCLES_PER_FRAME as u64;
        let mut frame_cycles: u64 = 0;

        while frame_cycles < target_cycles {
            // Check breakpoints
            if self.breakpoint_manager.is_enabled() {
                if let Some(_pc) = self.check_breakpoint() {
                    break;
                }
            }

            let pc_before = self.cpu.pc;
            let cycles = self.cpu.step() as u64;

            // Instruction tracing
            if self.instruction_tracer.is_enabled() {
                let bytes = vec![
                    self.cpu.memory.read(pc_before),
                    self.cpu.memory.read(pc_before.wrapping_add(1)),
                    self.cpu.memory.read(pc_before.wrapping_add(2)),
                ];
                let instr = emu_core::disasm_6502::disassemble_6502(&bytes, pc_before as u32)
                    .unwrap_or_else(|| {
                        emu_core::debug::DisassembledInstruction::new(
                            pc_before as u32,
                            vec![bytes[0]],
                            format!("${:04X}", pc_before),
                        )
                    });
                let mut state = emu_core::debug::CpuState::new(self.cpu.pc as u32);
                state.add_register(emu_core::debug::CpuRegister::new_8bit("A", self.cpu.a));
                state.add_register(emu_core::debug::CpuRegister::new_8bit("X", self.cpu.x));
                state.add_register(emu_core::debug::CpuRegister::new_8bit("Y", self.cpu.y));
                state.add_register(emu_core::debug::CpuRegister::new_8bit("SP", self.cpu.sp));
                state.add_register(emu_core::debug::CpuRegister::new_8bit("P", self.cpu.status));
                state.add_register(emu_core::debug::CpuRegister::new_16bit("PC", self.cpu.pc));
                self.instruction_tracer.trace(instr, state);
            }

            frame_cycles += cycles;
            self.total_cycles += cycles;

            // Tick VIC-II for each CPU cycle
            for _ in 0..cycles {
                self.vic.borrow_mut().tick();
            }

            // Check VIC-II IRQ
            if self.vic.borrow().irq_line {
                self.cpu.trigger_irq();
            }

            // Tick CIA 1 (generates IRQ)
            {
                let mut cia = self.cia1.borrow_mut();
                cia.tick(cycles as u32);
                if cia.irq_line {
                    self.cpu.trigger_irq();
                }
            }

            // Tick CIA 2 (generates NMI)
            {
                let mut cia = self.cia2.borrow_mut();
                cia.tick(cycles as u32);
                if cia.irq_line {
                    self.cpu.trigger_nmi();
                    cia.irq_line = false; // NMI is edge-triggered
                }
            }

            // Tick SID
            self.sid.borrow_mut().clock(cycles as u32);

            // Handle bad lines: steal ~40 cycles from CPU
            // (simplified: just account for it in timing)
            if self.vic.borrow().is_bad_line {
                frame_cycles += 40;
                self.total_cycles += 40;
                // Tick peripherals for the stolen cycles too
                self.sid.borrow_mut().clock(40);
                for _ in 0..40 {
                    self.vic.borrow_mut().tick();
                }
            }
        }

        // Re-sync VIC memory before fetching frame (for mid-frame changes)
        self.sync_vic_memory();

        let frame = self.vic.borrow().get_frame().clone();
        Ok(frame)
    }

    fn save_state(&self) -> Value {
        serde_json::json!({
            "system": "c64",
            "version": 1,
            "total_cycles": self.total_cycles,
            "cpu": {
                "a": self.cpu.a,
                "x": self.cpu.x,
                "y": self.cpu.y,
                "sp": self.cpu.sp,
                "status": self.cpu.status,
                "pc": self.cpu.pc,
            },
            "io_port": self.cpu.memory.io_port,
            "io_port_ddr": self.cpu.memory.io_port_ddr,
        })
    }

    fn load_state(&mut self, v: &Value) -> Result<(), serde_json::Error> {
        if let Some(cpu) = v.get("cpu") {
            if let Some(a) = cpu.get("a").and_then(|v| v.as_u64()) {
                self.cpu.a = a as u8;
            }
            if let Some(x) = cpu.get("x").and_then(|v| v.as_u64()) {
                self.cpu.x = x as u8;
            }
            if let Some(y) = cpu.get("y").and_then(|v| v.as_u64()) {
                self.cpu.y = y as u8;
            }
            if let Some(sp) = cpu.get("sp").and_then(|v| v.as_u64()) {
                self.cpu.sp = sp as u8;
            }
            if let Some(status) = cpu.get("status").and_then(|v| v.as_u64()) {
                self.cpu.status = status as u8;
            }
            if let Some(pc) = cpu.get("pc").and_then(|v| v.as_u64()) {
                self.cpu.pc = pc as u16;
            }
        }
        if let Some(io) = v.get("io_port").and_then(|v| v.as_u64()) {
            self.cpu.memory.io_port = io as u8;
        }
        if let Some(ddr) = v.get("io_port_ddr").and_then(|v| v.as_u64()) {
            self.cpu.memory.io_port_ddr = ddr as u8;
        }
        Ok(())
    }

    fn mount_points(&self) -> Vec<MountPointInfo> {
        vec![
            MountPointInfo {
                id: "cartridge".to_string(),
                name: "PRG / Cartridge".to_string(),
                extensions: vec![
                    "prg".to_string(),
                    "crt".to_string(),
                    "bin".to_string(),
                    "p00".to_string(),
                ],
                required: false,
            },
            MountPointInfo {
                id: "kernal".to_string(),
                name: "KERNAL ROM".to_string(),
                extensions: vec!["rom".to_string(), "bin".to_string()],
                required: false,
            },
            MountPointInfo {
                id: "basic".to_string(),
                name: "BASIC ROM".to_string(),
                extensions: vec!["rom".to_string(), "bin".to_string()],
                required: false,
            },
            MountPointInfo {
                id: "charrom".to_string(),
                name: "Character ROM".to_string(),
                extensions: vec!["rom".to_string(), "bin".to_string()],
                required: false,
            },
        ]
    }

    fn mount(&mut self, mount_point_id: &str, data: &[u8]) -> Result<(), Self::Error> {
        match mount_point_id {
            "cartridge" => {
                if data.len() < 2 {
                    return Err(C64Error::InvalidPrg);
                }
                self.cpu.memory.load_prg(data);
                self.cartridge_loaded = true;
                Ok(())
            }
            "kernal" => {
                if data.len() >= 0x2000 {
                    self.cpu
                        .memory
                        .load_kernal(data[data.len() - 0x2000..].to_vec());
                } else {
                    // Pad to 8KB
                    let mut padded = vec![0xEA; 0x2000];
                    padded[0x2000 - data.len()..].copy_from_slice(data);
                    self.cpu.memory.load_kernal(padded);
                }
                self.cpu.reset();
                Ok(())
            }
            "basic" => {
                if data.len() >= 0x2000 {
                    self.cpu
                        .memory
                        .load_basic(data[data.len() - 0x2000..].to_vec());
                } else {
                    let mut padded = vec![0xEA; 0x2000];
                    padded[0x2000 - data.len()..].copy_from_slice(data);
                    self.cpu.memory.load_basic(padded);
                }
                Ok(())
            }
            "charrom" => {
                if data.len() >= 0x1000 {
                    self.cpu
                        .memory
                        .load_char_rom(data[data.len() - 0x1000..].to_vec());
                } else {
                    let mut padded = vec![0; 0x1000];
                    padded[0x1000 - data.len()..].copy_from_slice(data);
                    self.cpu.memory.load_char_rom(padded);
                }
                Ok(())
            }
            _ => Err(C64Error::InvalidMountPoint(mount_point_id.to_string())),
        }
    }

    fn unmount(&mut self, mount_point_id: &str) -> Result<(), Self::Error> {
        match mount_point_id {
            "cartridge" => {
                self.cartridge_loaded = false;
                Ok(())
            }
            "kernal" => {
                self.cpu.memory.load_kernal(crate::bus::make_stub_kernal());
                self.cpu.reset();
                Ok(())
            }
            "basic" => {
                self.cpu.memory.load_basic(crate::bus::make_stub_basic());
                Ok(())
            }
            "charrom" => {
                self.cpu
                    .memory
                    .load_char_rom(crate::bus::make_default_char_rom());
                Ok(())
            }
            _ => Err(C64Error::InvalidMountPoint(mount_point_id.to_string())),
        }
    }

    fn is_mounted(&self, mount_point_id: &str) -> bool {
        match mount_point_id {
            "cartridge" => self.cartridge_loaded,
            "kernal" | "basic" | "charrom" => true, // Always have at least stubs
            _ => false,
        }
    }

    fn get_total_cycles(&self) -> u64 {
        self.total_cycles
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use emu_core::cpu_6502::Memory6502;

    #[test]
    fn test_new_system() {
        let sys = C64System::new();
        assert_eq!(sys.cpu.memory.io_port, 0x37);
        assert_eq!(sys.cpu.memory.io_port_ddr, 0x2F);
    }

    #[test]
    fn test_memory_banking() {
        let sys = C64System::new();

        // With default $0001 = $37 (all ROM visible):
        // $A000 should read BASIC ROM
        let basic_byte = sys.cpu.memory.read(0xA000);
        assert_eq!(basic_byte, 0xEA); // NOP from stub

        // $E000 should read KERNAL ROM
        let _kernal_byte = sys.cpu.memory.read(0xE000);
        // (Could be any byte from stub KERNAL)
    }

    #[test]
    fn test_io_port_banking() {
        let sys = C64System::new();
        // Default: HIRAM=1, LORAM=1, CHAREN=1 (IO visible at $D000)
        assert!(sys.cpu.memory.hiram());
        assert!(sys.cpu.memory.loram());
        assert!(sys.cpu.memory.charen());
    }

    #[test]
    fn test_ram_write_through() {
        let mut sys = C64System::new();
        // Writing to ROM area goes to underlying RAM
        sys.cpu.memory.write(0xA000, 0x42);
        // RAM should have the value
        assert_eq!(sys.cpu.memory.ram[0xA000], 0x42);
        // But reading should still return ROM
        let val = sys.cpu.memory.read(0xA000);
        assert_eq!(val, 0xEA); // BASIC ROM stub
    }

    #[test]
    fn test_color_ram() {
        let mut sys = C64System::new();
        // Write to color RAM via I/O
        sys.cpu.memory.write(0xD800, 0x0A);
        assert_eq!(sys.cpu.memory.color_ram[0], 0x0A);
    }

    #[test]
    fn test_prg_load() {
        let mut sys = C64System::new();
        // PRG: load at $0801
        let prg = vec![0x01, 0x08, 0xAA, 0xBB, 0xCC];
        sys.cpu.memory.load_prg(&prg);
        assert_eq!(sys.cpu.memory.ram[0x0801], 0xAA);
        assert_eq!(sys.cpu.memory.ram[0x0802], 0xBB);
        assert_eq!(sys.cpu.memory.ram[0x0803], 0xCC);
    }

    #[test]
    fn test_step_frame() {
        let mut sys = C64System::new();
        let result = sys.step_frame();
        assert!(result.is_ok());
        let frame = result.unwrap();
        assert_eq!(frame.width, 320);
        assert_eq!(frame.height, 200);
    }

    #[test]
    fn test_save_load_state() {
        let mut sys = C64System::new();
        sys.cpu.a = 0x42;
        sys.cpu.x = 0x10;
        let state = sys.save_state();

        let mut sys2 = C64System::new();
        sys2.load_state(&state).unwrap();
        assert_eq!(sys2.cpu.a, 0x42);
        assert_eq!(sys2.cpu.x, 0x10);
    }
}
