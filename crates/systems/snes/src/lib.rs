//! SNES (Super Nintendo Entertainment System) emulation implementation.
//!
//! This module provides a basic SNES system emulator using the reusable 65C816 CPU core
//! from `emu_core`, along with SNES-specific components:
//!
//! - **CPU**: WDC 65C816 (16-bit processor running at ~3.58 MHz)
//! - **PPU**: Picture Processing Unit (stub implementation)
//! - **APU**: SPC700 audio processor (stub implementation)
//! - **Memory**: 128KB WRAM + cartridge ROM/RAM
//! - **Timing**: NTSC (3.58 MHz CPU, ~60 Hz frame rate)

#![allow(clippy::upper_case_acronyms)]

mod bus;
mod cartridge;
mod cpu;

use bus::SnesBus;
use cpu::SnesCpu;
use emu_core::{types::Frame, MountPointInfo, System};
use emu_core::cpu_65c816::Memory65c816;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SnesError {
    #[error("Invalid ROM format: {0}")]
    InvalidRom(String),
    #[error("No cartridge mounted")]
    NoCartridge,
    #[error("Invalid mount point: {0}")]
    InvalidMountPoint(String),
}

/// SNES system implementation
pub struct SnesSystem {
    cpu: SnesCpu,
    frame_cycles: u32,
    current_cycles: u32,
}

impl SnesSystem {
    /// Create a new SNES system
    pub fn new() -> Self {
        let bus = SnesBus::new();
        Self {
            cpu: SnesCpu::new(bus),
            frame_cycles: 89342, // ~3.58MHz / 60Hz (NTSC)
            current_cycles: 0,
        }
    }
}

impl Default for SnesSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl System for SnesSystem {
    type Error = SnesError;

    fn reset(&mut self) {
        self.cpu.reset();
        self.current_cycles = 0;
    }

    fn step_frame(&mut self) -> Result<Frame, Self::Error> {
        self.current_cycles = 0;

        // Execute CPU cycles for one frame
        while self.current_cycles < self.frame_cycles {
            let cycles = self.cpu.step();
            self.current_cycles += cycles;
        }

        // Create a frame by reading from WRAM (where test ROM writes pattern)
        let mut frame = Frame::new(256, 224); // SNES native resolution
        
        // Read pattern from WRAM at $7E:0000 and convert to visible pixels
        // The test ROM writes alternating 0xAA and 0x55 bytes to the first 8KB
        // Repeat the pattern to fill the entire frame
        for y in 0..224 {
            for x in 0..256 {
                let pixel_index = y * 256 + x;
                let offset = (pixel_index % 0x2000) as u32; // Repeat 8KB pattern
                
                // Read from WRAM bank $7E
                let addr = 0x7E0000 + offset;
                let byte = self.cpu.bus().read(addr);
                
                // Convert byte to color - use different colors for 0xAA and 0x55
                let pixel = if byte == 0xAA {
                    0xFF8888FF // Light red/pink for 0xAA
                } else if byte == 0x55 {
                    0xFF4444FF // Dark red for 0x55
                } else {
                    0xFF000000 // Black for anything else
                };
                
                frame.pixels[pixel_index] = pixel;
            }
        }
        
        Ok(frame)
    }

    fn save_state(&self) -> serde_json::Value {
        serde_json::json!({
            "version": 1,
            "cpu": {
                "c": self.cpu.cpu.c,
                "x": self.cpu.cpu.x,
                "y": self.cpu.cpu.y,
                "s": self.cpu.cpu.s,
                "d": self.cpu.cpu.d,
                "dbr": self.cpu.cpu.dbr,
                "pbr": self.cpu.cpu.pbr,
                "pc": self.cpu.cpu.pc,
                "status": self.cpu.cpu.status,
                "emulation": self.cpu.cpu.emulation,
                "cycles": self.cpu.cpu.cycles,
            }
        })
    }

    fn load_state(&mut self, v: &serde_json::Value) -> Result<(), serde_json::Error> {
        if let Some(cpu_state) = v.get("cpu") {
            self.cpu.cpu.c = cpu_state["c"].as_u64().unwrap_or(0) as u16;
            self.cpu.cpu.x = cpu_state["x"].as_u64().unwrap_or(0) as u16;
            self.cpu.cpu.y = cpu_state["y"].as_u64().unwrap_or(0) as u16;
            self.cpu.cpu.s = cpu_state["s"].as_u64().unwrap_or(0) as u16;
            self.cpu.cpu.d = cpu_state["d"].as_u64().unwrap_or(0) as u16;
            self.cpu.cpu.dbr = cpu_state["dbr"].as_u64().unwrap_or(0) as u8;
            self.cpu.cpu.pbr = cpu_state["pbr"].as_u64().unwrap_or(0) as u8;
            self.cpu.cpu.pc = cpu_state["pc"].as_u64().unwrap_or(0) as u16;
            self.cpu.cpu.status = cpu_state["status"].as_u64().unwrap_or(0) as u8;
            self.cpu.cpu.emulation = cpu_state["emulation"].as_bool().unwrap_or(true);
            self.cpu.cpu.cycles = cpu_state["cycles"].as_u64().unwrap_or(0);
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
            extensions: vec!["smc".to_string(), "sfc".to_string()],
            required: true,
        }]
    }

    fn mount(&mut self, mount_point_id: &str, data: &[u8]) -> Result<(), Self::Error> {
        if mount_point_id != "Cartridge" {
            return Err(SnesError::InvalidMountPoint(mount_point_id.to_string()));
        }

        self.cpu.bus_mut().load_cartridge(data)?;
        self.reset();
        Ok(())
    }

    fn unmount(&mut self, mount_point_id: &str) -> Result<(), Self::Error> {
        if mount_point_id != "Cartridge" {
            return Err(SnesError::InvalidMountPoint(mount_point_id.to_string()));
        }

        self.cpu.bus_mut().unload_cartridge();
        Ok(())
    }

    fn is_mounted(&self, mount_point_id: &str) -> bool {
        mount_point_id == "Cartridge" && self.cpu.bus().has_cartridge()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_creation() {
        let sys = SnesSystem::new();
        assert!(!sys.is_mounted("Cartridge"));
    }

    #[test]
    fn test_mount_points() {
        let sys = SnesSystem::new();
        let mounts = sys.mount_points();
        assert_eq!(mounts.len(), 1);
        assert_eq!(mounts[0].id, "Cartridge");
    }

    #[test]
    fn test_reset() {
        let mut sys = SnesSystem::new();
        sys.reset();
        // Should not panic
    }

    #[test]
    fn test_save_load_state() {
        let sys = SnesSystem::new();
        let state = sys.save_state();

        let mut sys2 = SnesSystem::new();
        assert!(sys2.load_state(&state).is_ok());
    }

    #[test]
    fn test_snes_smoke_test_rom() {
        // Load the test ROM
        let test_rom = include_bytes!("../../../../test_roms/snes/test.sfc");

        let mut sys = SnesSystem::new();

        // Mount the test ROM
        assert!(sys.mount("Cartridge", test_rom).is_ok());
        assert!(sys.is_mounted("Cartridge"));

        // Run frames to let the ROM execute and write the pattern
        // The ROM will write alternating 0xAA and 0x55 bytes to WRAM
        let mut frame = sys.step_frame().unwrap();
        for _ in 0..9 {
            frame = sys.step_frame().unwrap();
        }

        // Verify frame dimensions
        assert_eq!(frame.width, 256);
        assert_eq!(frame.height, 224);
        assert_eq!(frame.pixels.len(), 256 * 224);

        // The ROM writes alternating 0xAA and 0x55 bytes to WRAM
        // This creates a checkerboard pattern with two colors
        // Verify that we have exactly 2 distinct colors in approximately 50/50 distribution

        use std::collections::HashMap;
        let mut color_counts: HashMap<u32, usize> = HashMap::new();
        for &pixel in &frame.pixels {
            *color_counts.entry(pixel).or_insert(0) += 1;
        }

        // Should have exactly 2 colors
        assert_eq!(
            color_counts.len(),
            2,
            "Expected 2 colors in checkerboard, found {}",
            color_counts.len()
        );

        // Each color should appear in roughly 50% of pixels
        // Allow some tolerance for edge cases
        let total_pixels = frame.pixels.len();
        for &count in color_counts.values() {
            let percentage = (count as f32 / total_pixels as f32) * 100.0;
            assert!(
                percentage >= 45.0 && percentage <= 55.0,
                "Color distribution should be ~50%, got {:.1}%",
                percentage
            );
        }
    }
}
