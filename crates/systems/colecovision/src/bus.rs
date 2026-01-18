//! ColecoVision memory bus implementation
//!
//! Memory map:
//! - 0x0000-0x1FFF: BIOS ROM (8KB)
//! - 0x6000-0x63FF: RAM (1KB, mirrored to 0x73FF)
//! - 0x8000-0xFFFF: Cartridge ROM (up to 32KB)
//!
//! I/O ports:
//! - 0xBE: VDP data
//! - 0xBF: VDP control/status
//! - 0xA0-0xA1: PSG
//! - 0xE0-0xFF: Controller ports

use crate::psg::ColecoVisionPsg;
use emu_core::cpu_z80::MemoryZ80;
use emu_core::logging::{log, LogCategory, LogLevel};
use emu_core::tms9918a::Tms9918a;
use std::cell::RefCell;
use std::rc::Rc;

/// ColecoVision memory bus
pub struct ColecoVisionMemory {
    // BIOS ROM (8KB)
    pub(crate) bios: Vec<u8>,

    // RAM (1KB)
    ram: [u8; 0x400],

    // Cartridge ROM
    pub(crate) rom: Vec<u8>,

    // Shared components
    vdp: Rc<RefCell<Tms9918a>>,
    psg: Rc<RefCell<ColecoVisionPsg>>,

    // Controller state
    controller1: u8,
    controller2: u8,
}

impl ColecoVisionMemory {
    /// Create a new ColecoVision memory bus
    pub fn new(
        bios: Vec<u8>,
        rom: Vec<u8>,
        vdp: Rc<RefCell<Tms9918a>>,
        psg: Rc<RefCell<ColecoVisionPsg>>,
    ) -> Self {
        Self {
            bios,
            ram: [0; 0x400],
            rom,
            vdp,
            psg,
            controller1: 0xFF,
            controller2: 0xFF,
        }
    }

    /// Set controller 1 state
    pub fn set_controller1(&mut self, state: u8) {
        self.controller1 = state;
    }

    /// Set controller 2 state
    pub fn set_controller2(&mut self, state: u8) {
        self.controller2 = state;
    }

    /// Get memory state for save state
    pub fn get_state(&self) -> serde_json::Value {
        serde_json::json!({
            "ram": self.ram.to_vec(),
        })
    }

    /// Set memory state from save state
    pub fn set_state(&mut self, state: &serde_json::Value) -> Result<(), serde_json::Error> {
        if let Some(ram) = state.get("ram").and_then(|v| v.as_array()) {
            for (i, val) in ram.iter().enumerate() {
                if i >= self.ram.len() {
                    break;
                }
                if let Some(byte) = val.as_u64() {
                    self.ram[i] = byte as u8;
                }
            }
        }

        Ok(())
    }
}

impl MemoryZ80 for ColecoVisionMemory {
    fn read(&self, addr: u16) -> u8 {
        match addr {
            // BIOS ROM (8KB)
            0x0000..=0x1FFF => {
                let index = (addr & 0x1FFF) as usize;
                if index < self.bios.len() {
                    self.bios[index]
                } else {
                    0xFF
                }
            }
            // RAM (1KB, mirrored to 0x73FF)
            0x6000..=0x73FF => self.ram[(addr & 0x3FF) as usize],
            // Cartridge ROM (up to 32KB)
            0x8000..=0xFFFF => {
                let index = (addr - 0x8000) as usize;
                if index < self.rom.len() {
                    self.rom[index]
                } else {
                    0xFF
                }
            }
            // Unmapped
            _ => 0xFF,
        }
    }

    fn write(&mut self, addr: u16, value: u8) {
        match addr {
            // BIOS ROM (read-only)
            0x0000..=0x1FFF => {}
            // RAM (1KB, mirrored to 0x73FF)
            0x6000..=0x73FF => {
                self.ram[(addr & 0x3FF) as usize] = value;
            }
            // Cartridge ROM (read-only)
            0x8000..=0xFFFF => {}
            // Unmapped
            _ => {}
        }
    }

    fn io_read(&mut self, port: u8) -> u8 {
        match port {
            // VDP data port
            0xBE => self.vdp.borrow_mut().read_data(),
            // VDP status port
            0xBF => self.vdp.borrow_mut().read_status(),
            // Controller port 1
            0xFC => self.controller1,
            // Controller port 2
            0xFF => self.controller2,
            _ => {
                log(LogCategory::Bus, LogLevel::Debug, || {
                    format!("ColecoVision: Read from unmapped port ${:02X}", port)
                });
                0xFF
            }
        }
    }

    fn io_write(&mut self, port: u8, value: u8) {
        match port {
            // VDP data port
            0xBE => self.vdp.borrow_mut().write_data(value),
            // VDP control port
            0xBF => self.vdp.borrow_mut().write_control(value),
            // PSG (both 0xA0 and 0xA1 map to PSG)
            0xA0 | 0xA1 => self.psg.borrow_mut().write(value),
            _ => {
                log(LogCategory::Bus, LogLevel::Debug, || {
                    format!(
                        "ColecoVision: Write ${:02X} to unmapped port ${:02X}",
                        value, port
                    )
                });
            }
        }
    }
}
