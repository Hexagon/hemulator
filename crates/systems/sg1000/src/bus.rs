//! SG-1000 memory bus implementation
//!
//! Memory map:
//! - 0x0000-0xBFFF: Cartridge ROM (up to 48KB)
//! - 0xC000-0xC3FF: RAM (1KB, mirrored to 0xFFFF)
//!
//! I/O ports:
//! - 0xBE: VDP data
//! - 0xBF: VDP control/status
//! - 0x7F: PSG
//! - 0xDC-0xDF: Controller ports

use crate::psg::Sg1000Psg;
use crate::vdp::Vdp;
use emu_core::cpu_z80::MemoryZ80;
use emu_core::logging::{log, LogCategory, LogLevel};
use std::cell::RefCell;
use std::rc::Rc;

/// SG-1000 memory bus
pub struct Sg1000Memory {
    // RAM (1KB)
    ram: [u8; 0x400],

    // Cartridge ROM
    pub(crate) rom: Vec<u8>,

    // Shared components
    vdp: Rc<RefCell<Vdp>>,
    psg: Rc<RefCell<Sg1000Psg>>,

    // Controller state
    controller1: u8,
    controller2: u8,
}

impl Sg1000Memory {
    /// Create a new SG-1000 memory bus
    pub fn new(rom: Vec<u8>, vdp: Rc<RefCell<Vdp>>, psg: Rc<RefCell<Sg1000Psg>>) -> Self {
        Self {
            ram: [0; 0x400],
            rom,
            vdp,
            psg,
            controller1: 0xFF,
            controller2: 0xFF,
        }
    }

    /// Set controller state
    pub fn set_controller(&mut self, port: u8, state: u8) {
        if port == 1 {
            self.controller1 = state;
        } else if port == 2 {
            self.controller2 = state;
        }
    }

    /// Get state for save state
    pub fn get_state(&self) -> serde_json::Value {
        serde_json::json!({
            "ram": self.ram.to_vec(),
            "controller1": self.controller1,
            "controller2": self.controller2,
        })
    }

    /// Set state from save state
    pub fn set_state(&mut self, state: &serde_json::Value) -> Result<(), serde_json::Error> {
        if let Some(ram) = state.get("ram").and_then(|v| v.as_array()) {
            for (i, val) in ram.iter().enumerate() {
                if i < self.ram.len() {
                    if let Some(byte) = val.as_u64() {
                        self.ram[i] = byte as u8;
                    }
                }
            }
        }

        if let Some(c1) = state.get("controller1").and_then(|v| v.as_u64()) {
            self.controller1 = c1 as u8;
        }

        if let Some(c2) = state.get("controller2").and_then(|v| v.as_u64()) {
            self.controller2 = c2 as u8;
        }

        Ok(())
    }
}

impl MemoryZ80 for Sg1000Memory {
    fn read(&self, addr: u16) -> u8 {
        match addr {
            // Cartridge ROM (up to 48KB)
            0x0000..=0xBFFF => {
                let index = addr as usize;
                if index < self.rom.len() {
                    self.rom[index]
                } else {
                    0xFF
                }
            }
            // RAM (1KB, mirrored to 0xFFFF)
            0xC000..=0xFFFF => self.ram[((addr - 0xC000) & 0x3FF) as usize],
        }
    }

    fn write(&mut self, addr: u16, value: u8) {
        match addr {
            // Cartridge ROM (read-only)
            0x0000..=0xBFFF => {}
            // RAM (1KB, mirrored to 0xFFFF)
            0xC000..=0xFFFF => {
                self.ram[((addr - 0xC000) & 0x3FF) as usize] = value;
            }
        }
    }

    fn io_read(&mut self, port: u8) -> u8 {
        match port {
            // VDP data port
            0xBE => self.vdp.borrow_mut().read_data(),
            // VDP status port
            0xBF => self.vdp.borrow_mut().read_status(),
            // Controller port 1
            0xDC | 0xDD => self.controller1,
            // Controller port 2
            0xDE | 0xDF => self.controller2,
            _ => {
                log(LogCategory::Bus, LogLevel::Debug, || {
                    format!("SG-1000: Read from unmapped port ${:02X}", port)
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
            // PSG
            0x7F => self.psg.borrow_mut().write(value),
            _ => {
                log(LogCategory::Bus, LogLevel::Debug, || {
                    format!(
                        "SG-1000: Write ${:02X} to unmapped port ${:02X}",
                        value, port
                    )
                });
            }
        }
    }
}
