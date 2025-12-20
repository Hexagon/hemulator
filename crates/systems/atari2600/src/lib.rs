//! Atari 2600 system implementation

#![allow(clippy::upper_case_acronyms)]

mod bus;
mod cartridge;
mod cpu;
mod riot;
mod tia;

use bus::Atari2600Bus;
use cartridge::{Cartridge, CartridgeError};
use cpu::Atari2600Cpu;
use emu_core::{types::Frame, MountPointInfo, System};
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Atari2600Error {
    #[error("Cartridge error: {0}")]
    Cartridge(#[from] CartridgeError),
    #[error("No cartridge loaded")]
    NoCartridge,
    #[error("Invalid mount point: {0}")]
    InvalidMountPoint(String),
}

/// Atari 2600 system
pub struct Atari2600System {
    cpu: Atari2600Cpu,
    cycles: u64,
}

impl Default for Atari2600System {
    fn default() -> Self {
        Self::new()
    }
}

impl Atari2600System {
    /// Create a new Atari 2600 system
    pub fn new() -> Self {
        let bus = Atari2600Bus::new();
        let cpu = Atari2600Cpu::new(bus);
        
        Self {
            cpu,
            cycles: 0,
        }
    }

    /// Get debug information
    pub fn debug_info(&self) -> Option<DebugInfo> {
        self.cpu.bus().and_then(|bus| {
            bus.cartridge.as_ref().map(|cart| DebugInfo {
                rom_size: cart.size(),
                banking_scheme: format!("{:?}", cart.scheme()),
                current_bank: cart.current_bank(),
                scanline: bus.tia.get_scanline(),
            })
        })
    }
}

#[derive(Debug, Clone)]
pub struct DebugInfo {
    pub rom_size: usize,
    pub banking_scheme: String,
    pub current_bank: usize,
    pub scanline: u16,
}

impl System for Atari2600System {
    type Error = Atari2600Error;

    fn reset(&mut self) {
        self.cpu.reset();
        if let Some(bus) = self.cpu.bus_mut() {
            bus.reset();
        }
        self.cycles = 0;
    }

    fn step_frame(&mut self) -> Result<Frame, Self::Error> {
        // Atari 2600 runs at ~1.19 MHz (NTSC)
        // 262 scanlines per frame, ~76 cycles per scanline = ~19,912 cycles per frame
        const CYCLES_PER_FRAME: u32 = 19912;
        
        let mut frame = Frame::new(160, 192);
        let mut cycles_this_frame = 0u32;
        
        // Execute until we've completed a frame
        while cycles_this_frame < CYCLES_PER_FRAME {
            let cycles = self.cpu.step();
            
            // Clock the TIA and RIOT
            if let Some(bus) = self.cpu.bus_mut() {
                bus.clock(cycles);
            }
            
            cycles_this_frame += cycles;
            self.cycles += cycles as u64;
        }
        
        // Render the frame
        if let Some(bus) = self.cpu.bus() {
            // Render visible scanlines (40-231 are visible on NTSC)
            for line in 0..192 {
                bus.tia.render_scanline(&mut frame.pixels, line);
            }
        }
        
        Ok(frame)
    }

    fn save_state(&self) -> Value {
        serde_json::json!({
            "version": 1,
            "system": "atari2600",
            "cycles": self.cycles,
            "bus": self.cpu.bus(),
        })
    }

    fn load_state(&mut self, v: &Value) -> Result<(), serde_json::Error> {
        let version = v["version"].as_u64().unwrap_or(0);
        if version != 1 {
            return Err(serde_json::from_str::<()>("invalid").unwrap_err());
        }
        
        let system = v["system"].as_str().unwrap_or("");
        if system != "atari2600" {
            return Err(serde_json::from_str::<()>("invalid").unwrap_err());
        }
        
        self.cycles = v["cycles"].as_u64().unwrap_or(0);
        
        if let Some(bus_value) = v.get("bus") {
            let bus: Atari2600Bus = serde_json::from_value(bus_value.clone())?;
            // Create a new CPU with the loaded bus
            self.cpu = Atari2600Cpu::new(bus);
        }
        
        Ok(())
    }

    fn supports_save_states(&self) -> bool {
        true
    }

    fn mount_points(&self) -> Vec<MountPointInfo> {
        vec![MountPointInfo {
            id: "Cartridge".to_string(),
            name: "Cartridge Slot".to_string(),
            extensions: vec!["a26".to_string(), "bin".to_string()],
            required: true,
        }]
    }

    fn mount(&mut self, mount_point_id: &str, data: &[u8]) -> Result<(), Self::Error> {
        if mount_point_id != "Cartridge" {
            return Err(Atari2600Error::InvalidMountPoint(
                mount_point_id.to_string(),
            ));
        }
        
        let cartridge = Cartridge::new(data.to_vec())?;
        
        if let Some(bus) = self.cpu.bus_mut() {
            bus.load_cartridge(cartridge);
        }
        
        self.reset();
        Ok(())
    }

    fn unmount(&mut self, mount_point_id: &str) -> Result<(), Self::Error> {
        if mount_point_id != "Cartridge" {
            return Err(Atari2600Error::InvalidMountPoint(
                mount_point_id.to_string(),
            ));
        }
        
        if let Some(bus) = self.cpu.bus_mut() {
            bus.cartridge = None;
        }
        
        Ok(())
    }

    fn is_mounted(&self, mount_point_id: &str) -> bool {
        if mount_point_id != "Cartridge" {
            return false;
        }
        
        self.cpu
            .bus()
            .map(|bus| bus.cartridge.is_some())
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_creation() {
        let sys = Atari2600System::new();
        assert_eq!(sys.cycles, 0);
    }

    #[test]
    fn test_mount_points() {
        let sys = Atari2600System::new();
        let mounts = sys.mount_points();
        
        assert_eq!(mounts.len(), 1);
        assert_eq!(mounts[0].id, "Cartridge");
        assert!(mounts[0].required);
    }

    #[test]
    fn test_mount_cartridge() {
        let mut sys = Atari2600System::new();
        
        // Create a simple 4K ROM
        let rom = vec![0xFF; 4096];
        
        assert!(sys.mount("Cartridge", &rom).is_ok());
        assert!(sys.is_mounted("Cartridge"));
    }

    #[test]
    fn test_unmount_cartridge() {
        let mut sys = Atari2600System::new();
        
        let rom = vec![0xFF; 4096];
        sys.mount("Cartridge", &rom).unwrap();
        
        assert!(sys.unmount("Cartridge").is_ok());
        assert!(!sys.is_mounted("Cartridge"));
    }

    #[test]
    fn test_invalid_mount_point() {
        let mut sys = Atari2600System::new();
        let rom = vec![0xFF; 4096];
        
        assert!(sys.mount("Invalid", &rom).is_err());
    }

    #[test]
    fn test_reset() {
        let mut sys = Atari2600System::new();
        
        // Load a ROM and run for a bit
        let rom = vec![0xFF; 4096];
        sys.mount("Cartridge", &rom).unwrap();
        
        // Reset should work
        sys.reset();
        assert_eq!(sys.cycles, 0);
    }

    #[test]
    fn test_save_load_state() {
        let mut sys = Atari2600System::new();
        
        assert!(sys.supports_save_states());
        
        let state = sys.save_state();
        assert_eq!(state["version"], 1);
        assert_eq!(state["system"], "atari2600");
        
        let mut sys2 = Atari2600System::new();
        assert!(sys2.load_state(&state).is_ok());
    }
}
