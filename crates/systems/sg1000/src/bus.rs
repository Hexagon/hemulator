//! SG-1000 memory bus implementation
//!
//! Memory map:
//! - 0x0000-0xBFFF: Cartridge ROM (up to 48KB)
//! - 0xC000-0xC3FF: RAM (1KB, mirrored to 0xFFFF)
//!
//! I/O ports:
//! - 0x40-0x7F: PSG (SN76489) - all ports mirrored
//! - 0x80-0xFF (even): VDP data - mirrored on all even ports
//! - 0x80-0xFF (odd): VDP control/status - mirrored on all odd ports
//! - 0xC0-0xFF (even): Controller 1 - mirrored on all even ports
//! - 0xC0-0xFF (odd): Controller 2 - mirrored on all odd ports

use crate::psg::Sg1000Psg;
use emu_core::cpu_z80::MemoryZ80;
use emu_core::logging::{log, LogCategory, LogLevel};
use emu_core::tms9918a::Tms9918a;
use std::cell::RefCell;
use std::rc::Rc;

/// SG-1000 memory bus
pub struct Sg1000Memory {
    // RAM (1KB)
    ram: [u8; 0x400],

    // Cartridge ROM
    pub(crate) rom: Vec<u8>,

    // Shared components
    vdp: Rc<RefCell<Tms9918a>>,
    psg: Rc<RefCell<Sg1000Psg>>,

    // Controller state
    controller1: u8,
    controller2: u8,
}

impl Sg1000Memory {
    /// Create a new SG-1000 memory bus
    pub fn new(rom: Vec<u8>, vdp: Rc<RefCell<Tms9918a>>, psg: Rc<RefCell<Sg1000Psg>>) -> Self {
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
        // Per SMS Power! SG-1000 I/O port map:
        // - VDP Data: all even ports from 0x80-0xFF
        // - VDP Status: all odd ports from 0x80-0xFF
        // - Controller 1: all even ports from 0xC0-0xFF
        // - Controller 2: all odd ports from 0xC0-0xFF
        // Controller ports at 0xC0+ take priority over VDP
        match port {
            // Controller ports (0xC0-0xFF) - these take priority
            0xC0..=0xFF if (port & 1) == 0 => self.controller1,
            0xC0..=0xFF => self.controller2,
            // VDP ports (mirrored on all even/odd from 0x80-0xBF)
            0x80..=0xBF => {
                if (port & 1) == 0 {
                    // Even port = VDP data
                    self.vdp.borrow_mut().read_data()
                } else {
                    // Odd port = VDP status
                    self.vdp.borrow_mut().read_status()
                }
            }
            _ => {
                log(LogCategory::Bus, LogLevel::Debug, || {
                    format!("SG-1000: Read from unmapped port ${:02X}", port)
                });
                0xFF
            }
        }
    }

    fn io_write(&mut self, port: u8, value: u8) {
        // Per SMS Power! SG-1000 I/O port map:
        // - PSG: all ports from 0x40-0x7F (both even and odd)
        // - VDP Data: all even ports from 0x80-0xFF
        // - VDP Control: all odd ports from 0x80-0xFF
        match port {
            // PSG (mirrored on all ports from 0x40-0x7F)
            0x40..=0x7F => self.psg.borrow_mut().write(value),
            // VDP ports (mirrored on all even/odd from 0x80 onwards)
            0x80..=0xFF => {
                if (port & 1) == 0 {
                    // Even port = VDP data
                    self.vdp.borrow_mut().write_data(value)
                } else {
                    // Odd port = VDP control
                    self.vdp.borrow_mut().write_control(value)
                }
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    /// Create a test memory bus with minimal ROM and shared components
    fn create_test_bus() -> Sg1000Memory {
        let rom = vec![0; 0x8000]; // 32KB dummy ROM
        let vdp = Rc::new(RefCell::new(Tms9918a::new()));
        let psg = Rc::new(RefCell::new(Sg1000Psg::new()));
        Sg1000Memory::new(rom, vdp, psg)
    }

    #[test]
    fn test_psg_mirroring() {
        // PSG should be accessible on all ports 0x40-0x7F
        let mut bus = create_test_bus();

        // Write to different PSG ports - all should write to the same PSG
        bus.io_write(0x40, 0x80); // Tone 0 frequency low
        bus.io_write(0x50, 0x81); // Tone 0 frequency high
        bus.io_write(0x60, 0x90); // Tone 0 attenuation
        bus.io_write(0x7F, 0xBF); // Noise attenuation

        // All writes should have been accepted without error
        // (no way to read back PSG state directly, but no panic = success)
    }

    #[test]
    fn test_vdp_data_mirroring() {
        // VDP data should be accessible on all even ports 0x80-0xFF
        let mut bus = create_test_bus();

        // Write to different even VDP data ports
        bus.io_write(0x80, 0x12);
        bus.io_write(0x82, 0x34);
        bus.io_write(0xBE, 0x56);
        bus.io_write(0xE0, 0x78);
        bus.io_write(0xFE, 0x9A);

        // All writes should have been accepted
        // Reading from even ports should return VDP data
        let _data = bus.io_read(0x80);
        let _data = bus.io_read(0xA0);
        let _data = bus.io_read(0xFE);
    }

    #[test]
    fn test_vdp_control_mirroring() {
        // VDP control should be accessible on all odd ports 0x80-0xFF
        let mut bus = create_test_bus();

        // Write to different odd VDP control ports
        bus.io_write(0x81, 0x00);
        bus.io_write(0x83, 0x80);
        bus.io_write(0xBF, 0xC0);
        bus.io_write(0xE1, 0xE0);
        bus.io_write(0xFF, 0xFF);

        // All writes should have been accepted
        // Reading from odd ports should return VDP status
        let _status = bus.io_read(0x81);
        let _status = bus.io_read(0xA1);
        let _status = bus.io_read(0xFF);
    }

    #[test]
    fn test_controller_mirroring() {
        // Controller 1 should be readable from all even ports 0xC0-0xFF
        // Controller 2 should be readable from all odd ports 0xC0-0xFF
        let mut bus = create_test_bus();

        // Set controller states
        bus.set_controller(1, 0x12);
        bus.set_controller(2, 0x34);

        // Read controller 1 from various even ports in 0xC0-0xFF range
        assert_eq!(bus.io_read(0xC0), 0x12);
        assert_eq!(bus.io_read(0xC2), 0x12);
        assert_eq!(bus.io_read(0xFE), 0x12);

        // Read controller 2 from various odd ports in 0xC0-0xFF range
        assert_eq!(bus.io_read(0xC1), 0x34);
        assert_eq!(bus.io_read(0xC3), 0x34);
        assert_eq!(bus.io_read(0xFF), 0x34);
    }

    #[test]
    fn test_controller_priority_over_vdp() {
        // Controller ports (0xC0-0xFF) should take priority over VDP
        // Even though VDP is also mapped to 0x80-0xFF, controllers override at 0xC0+
        let mut bus = create_test_bus();

        // Set controller states
        bus.set_controller(1, 0xAA);
        bus.set_controller(2, 0x55);

        // Reading from 0xC0+ should return controller values, not VDP
        assert_eq!(bus.io_read(0xC0), 0xAA); // Controller 1, not VDP data
        assert_eq!(bus.io_read(0xC1), 0x55); // Controller 2, not VDP status
        assert_eq!(bus.io_read(0xFE), 0xAA); // Controller 1
        assert_eq!(bus.io_read(0xFF), 0x55); // Controller 2
    }

    #[test]
    fn test_vdp_below_controller_range() {
        // VDP should still work in 0x80-0xBF range (before controller priority)
        let mut bus = create_test_bus();

        // These should access VDP, not controllers
        bus.io_write(0x80, 0x12); // VDP data
        bus.io_write(0x81, 0x80); // VDP control
        bus.io_write(0xBE, 0x34); // VDP data
        bus.io_write(0xBF, 0xC0); // VDP control

        // Reading should return VDP values
        let _data = bus.io_read(0x80); // VDP data
        let _status = bus.io_read(0x81); // VDP status
    }

    #[test]
    fn test_unmapped_ports() {
        // Ports outside the defined ranges should return 0xFF
        let mut bus = create_test_bus();

        // Read from unmapped ports
        assert_eq!(bus.io_read(0x00), 0xFF);
        assert_eq!(bus.io_read(0x3F), 0xFF);

        // Writes to unmapped ports should not crash
        bus.io_write(0x00, 0x12);
        bus.io_write(0x3F, 0x34);
    }
}
