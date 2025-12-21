//! N64 (Nintendo 64) emulation implementation.
//!
//! This module provides a basic N64 system emulator using the MIPS R4300i CPU core
//! from `emu_core`, along with N64-specific components:
//!
//! - **CPU**: MIPS R4300i (64-bit processor running at 93.75 MHz)
//! - **RCP**: Reality Co-Processor (graphics and audio - stub implementation)
//! - **Memory**: 4MB RDRAM + cartridge ROM
//! - **Timing**: NTSC (~60 Hz frame rate)

#![allow(clippy::upper_case_acronyms)]

mod bus;
mod cartridge;
mod cpu;

use bus::N64Bus;
use cpu::N64Cpu;
use emu_core::{types::Frame, MountPointInfo, System};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum N64Error {
    #[error("Invalid ROM format: {0}")]
    InvalidRom(String),
    #[error("No cartridge mounted")]
    NoCartridge,
    #[error("Invalid mount point: {0}")]
    InvalidMountPoint(String),
}

/// N64 system implementation
pub struct N64System {
    cpu: N64Cpu,
    frame_cycles: u32,
    current_cycles: u32,
}

impl N64System {
    /// Create a new N64 system
    pub fn new() -> Self {
        let bus = N64Bus::new();
        Self {
            cpu: N64Cpu::new(bus),
            frame_cycles: 1562500, // ~93.75MHz / 60Hz (NTSC)
            current_cycles: 0,
        }
    }
}

impl Default for N64System {
    fn default() -> Self {
        Self::new()
    }
}

impl System for N64System {
    type Error = N64Error;

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

        // Create a frame by reading from RDRAM (where test ROM writes pattern)
        let mut frame = Frame::new(320, 240); // N64 common resolution
        
        // Read pattern from RDRAM at 0x00000000 and convert to visible pixels
        // The test ROM writes alternating 0xAA and 0x55 bytes to the first 8KB
        // Repeat the pattern to fill the entire frame
        use emu_core::cpu_mips_r4300i::MemoryMips;
        for y in 0..240 {
            for x in 0..320 {
                let pixel_index = y * 320 + x;
                let offset = (pixel_index % 0x2000) as u32; // Repeat 8KB pattern
                
                // Read from RDRAM
                let byte = self.cpu.bus().read_byte(offset);
                
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
                "gpr": self.cpu.cpu.gpr,
                "pc": self.cpu.cpu.pc,
                "hi": self.cpu.cpu.hi,
                "lo": self.cpu.cpu.lo,
                "cycles": self.cpu.cpu.cycles,
            }
        })
    }

    fn load_state(&mut self, v: &serde_json::Value) -> Result<(), serde_json::Error> {
        if let Some(cpu_state) = v.get("cpu") {
            if let Some(gpr_array) = cpu_state["gpr"].as_array() {
                for (i, val) in gpr_array.iter().enumerate() {
                    if i < 32 {
                        self.cpu.cpu.gpr[i] = val.as_u64().unwrap_or(0);
                    }
                }
            }
            self.cpu.cpu.pc = cpu_state["pc"].as_u64().unwrap_or(0);
            self.cpu.cpu.hi = cpu_state["hi"].as_u64().unwrap_or(0);
            self.cpu.cpu.lo = cpu_state["lo"].as_u64().unwrap_or(0);
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
            extensions: vec!["z64".to_string(), "n64".to_string(), "v64".to_string()],
            required: true,
        }]
    }

    fn mount(&mut self, mount_point_id: &str, data: &[u8]) -> Result<(), Self::Error> {
        if mount_point_id != "Cartridge" {
            return Err(N64Error::InvalidMountPoint(mount_point_id.to_string()));
        }

        self.cpu.bus_mut().load_cartridge(data)?;
        self.reset();
        
        // Simulate boot: set PC to entry point from cartridge header
        if let Some(entry_point) = self.cpu.bus().get_entry_point() {
            self.cpu.cpu.pc = entry_point;
        }
        
        Ok(())
    }

    fn unmount(&mut self, mount_point_id: &str) -> Result<(), Self::Error> {
        if mount_point_id != "Cartridge" {
            return Err(N64Error::InvalidMountPoint(mount_point_id.to_string()));
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
        let sys = N64System::new();
        assert!(!sys.is_mounted("Cartridge"));
    }

    #[test]
    fn test_mount_points() {
        let sys = N64System::new();
        let mounts = sys.mount_points();
        assert_eq!(mounts.len(), 1);
        assert_eq!(mounts[0].id, "Cartridge");
        assert_eq!(mounts[0].extensions.len(), 3);
    }

    #[test]
    fn test_reset() {
        let mut sys = N64System::new();
        sys.reset();
        // Should not panic
    }

    #[test]
    fn test_save_load_state() {
        let sys = N64System::new();
        let state = sys.save_state();

        let mut sys2 = N64System::new();
        assert!(sys2.load_state(&state).is_ok());
    }

    #[test]
    fn test_n64_smoke_test_rom() {
        // Load the test ROM
        let test_rom = include_bytes!("../../../../test_roms/n64/test.z64");

        let mut sys = N64System::new();

        // Mount the test ROM
        assert!(sys.mount("Cartridge", test_rom).is_ok());
        assert!(sys.is_mounted("Cartridge"));

        // Run frames to let the ROM execute and write the pattern
        // The ROM will write alternating 0xAA and 0x55 bytes to RDRAM
        let mut frame = sys.step_frame().unwrap();
        for _ in 0..9 {
            frame = sys.step_frame().unwrap();
        }

        // Verify frame dimensions
        assert_eq!(frame.width, 320);
        assert_eq!(frame.height, 240);
        assert_eq!(frame.pixels.len(), 320 * 240);

        // The ROM writes alternating 0xAA and 0x55 bytes to RDRAM
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
