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
mod ppu;
pub mod ppu_renderer;

use emu_core::logging::{log, LogCategory, LogLevel};

/// SNES controller button constants
pub mod controller {
    /// SNES controller button bit positions (for 16-bit button state)
    /// Button layout: B Y Select Start Up Down Left Right A X L R 0 0 0 0
    pub const B: u16 = 1 << 15; // 0x8000
    pub const Y: u16 = 1 << 14; // 0x4000
    pub const SELECT: u16 = 1 << 13; // 0x2000
    pub const START: u16 = 1 << 12; // 0x1000
    pub const UP: u16 = 1 << 11; // 0x0800
    pub const DOWN: u16 = 1 << 10; // 0x0400
    pub const LEFT: u16 = 1 << 9; // 0x0200
    pub const RIGHT: u16 = 1 << 8; // 0x0100
    pub const A: u16 = 1 << 7; // 0x0080
    pub const X: u16 = 1 << 6; // 0x0040
    pub const L: u16 = 1 << 5; // 0x0020
    pub const R: u16 = 1 << 4; // 0x0010
}

use bus::SnesBus;
use cpu::SnesCpu;
use emu_core::{types::Frame, MountPointInfo, System};
use ppu_renderer::{SnesPpuRenderer, SoftwareSnesPpuRenderer};
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

/// Debug information for SNES system
pub struct DebugInfo {
    pub rom_size: usize,
    pub has_smc_header: bool,
    pub pc: u16,
    pub pbr: u8,
    pub emulation_mode: bool,
}

/// SNES system implementation
pub struct SnesSystem {
    cpu: SnesCpu,
    frame_cycles: u32,
    current_cycles: u32,
    renderer: Box<dyn SnesPpuRenderer>,
}

// SNES timing constants (NTSC)
const SNES_FRAME_CYCLES: u32 = 89342; // ~3.58MHz / 60Hz
const SNES_VISIBLE_CYCLES: u32 = 76400; // ~85.5% of frame before VBlank

impl SnesSystem {
    /// Create a new SNES system
    pub fn new() -> Self {
        let bus = SnesBus::new();
        Self {
            cpu: SnesCpu::new(bus),
            frame_cycles: SNES_FRAME_CYCLES,
            current_cycles: 0,
            renderer: Box::new(SoftwareSnesPpuRenderer::new()),
        }
    }

    /// Get debug information for the SNES system
    pub fn get_debug_info(&self) -> DebugInfo {
        let bus = self.cpu.bus();
        let cartridge_info = if bus.has_cartridge() {
            // Try to get cartridge info from the bus
            (bus.get_rom_size(), bus.has_smc_header())
        } else {
            (0, false)
        };

        DebugInfo {
            rom_size: cartridge_info.0,
            has_smc_header: cartridge_info.1,
            pc: self.cpu.cpu.pc,
            pbr: self.cpu.cpu.pbr,
            emulation_mode: self.cpu.cpu.emulation,
        }
    }

    /// Set controller state for player 1 or 2 (idx: 0 or 1)
    /// Button layout (16 bits): B Y Select Start Up Down Left Right A X L R 0 0 0 0
    /// Example: 0x8000 = B button, 0x0080 = A button
    pub fn set_controller(&mut self, idx: usize, state: u16) {
        self.cpu.bus_mut().set_controller(idx, state);
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
        log(LogCategory::CPU, LogLevel::Info, || {
            "SNES: System reset".to_string()
        });
        self.cpu.reset();
        self.current_cycles = 0;
    }

    fn step_frame(&mut self) -> Result<Frame, Self::Error> {
        self.current_cycles = 0;

        // Tick the frame counter for VBlank emulation
        self.cpu.cpu.memory.tick_frame();

        // Clear VBlank at start of frame
        self.cpu.bus_mut().ppu_mut().set_vblank(false);
        log(LogCategory::PPU, LogLevel::Trace, || {
            "SNES: Frame start, VBlank cleared".to_string()
        });

        // Execute CPU cycles for visible portion
        while self.current_cycles < SNES_VISIBLE_CYCLES {
            let cycles = self.cpu.step();
            self.current_cycles += cycles;
            // Update cycle counter in bus for VBlank timing
            self.cpu.bus_mut().tick_cycles(cycles);
        }

        // Render frame at end of visible scanlines
        self.renderer.render_frame(self.cpu.bus().ppu());

        // Enter VBlank and trigger NMI if enabled
        self.cpu.bus_mut().ppu_mut().set_vblank(true);
        log(LogCategory::PPU, LogLevel::Debug, || {
            format!(
                "SNES: VBlank started (cycle {}), NMI enabled: {}",
                self.current_cycles,
                self.cpu.bus_mut().ppu_mut().nmi_enable
            )
        });

        // Check for NMI and trigger it on the 65C816
        if self.cpu.bus_mut().ppu_mut().take_nmi_pending() {
            log(LogCategory::Interrupts, LogLevel::Debug, || {
                "SNES: NMI triggered".to_string()
            });
            self.cpu.cpu.trigger_nmi();
        }

        // Execute remaining VBlank cycles
        while self.current_cycles < self.frame_cycles {
            let cycles = self.cpu.step();
            self.current_cycles += cycles;
            self.cpu.bus_mut().tick_cycles(cycles);

            // Check for additional NMI requests during VBlank
            if self.cpu.bus_mut().ppu_mut().take_nmi_pending() {
                log(LogCategory::Interrupts, LogLevel::Debug, || {
                    "SNES: Additional NMI triggered during VBlank".to_string()
                });
                self.cpu.cpu.trigger_nmi();
            }
        }

        // Clear VBlank at end of frame
        self.cpu.bus_mut().ppu_mut().set_vblank(false);
        log(LogCategory::PPU, LogLevel::Trace, || {
            "SNES: Frame end, VBlank cleared".to_string()
        });

        Ok(self.renderer.get_frame().clone())
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
            log(LogCategory::Bus, LogLevel::Warn, || {
                format!("SNES: Invalid mount point: {}", mount_point_id)
            });
            return Err(SnesError::InvalidMountPoint(mount_point_id.to_string()));
        }

        log(LogCategory::Bus, LogLevel::Info, || {
            format!("SNES: Mounting cartridge ({} bytes)", data.len())
        });
        self.cpu.bus_mut().load_cartridge(data)?;
        self.reset();
        Ok(())
    }

    fn unmount(&mut self, mount_point_id: &str) -> Result<(), Self::Error> {
        if mount_point_id != "Cartridge" {
            log(LogCategory::Bus, LogLevel::Warn, || {
                format!("SNES: Invalid mount point for unmount: {}", mount_point_id)
            });
            return Err(SnesError::InvalidMountPoint(mount_point_id.to_string()));
        }

        log(LogCategory::Bus, LogLevel::Info, || {
            "SNES: Unmounting cartridge".to_string()
        });
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

        let mut sys = SnesSystem::default();

        // Mount the test ROM
        assert!(sys.mount("Cartridge", test_rom).is_ok());
        assert!(sys.is_mounted("Cartridge"));

        // Run multiple frames to allow the ROM to initialize
        // The test ROM initializes graphics during RESET and then enters a WAI loop
        // We need to give it time to set up VRAM, CGRAM, and the tilemap
        let mut frame = sys.step_frame().unwrap();
        for _ in 0..10 {
            frame = sys.step_frame().unwrap();
        }

        // Verify frame dimensions
        assert_eq!(frame.width, 256);
        assert_eq!(frame.height, 224);
        assert_eq!(frame.pixels.len(), 256 * 224);

        // Verify we have a checkerboard pattern
        // The test ROM creates alternating tiles (blue and red) in a checkerboard
        // Tile 0 (blue): color 3 = 0xFF0000F8 (blue with 5-bit to 8-bit conversion)
        // Tile 1 (red): color 2 = 0xFFF80000 (red)

        // Helper to get pixel at tile position
        let get_tile_color = |tx: usize, ty: usize| -> u32 {
            // Get pixel from center of tile to avoid edge effects
            let x = tx * 8 + 4;
            let y = ty * 8 + 4;
            frame.pixels[y * 256 + x]
        };

        // Verify horizontal checkerboard: adjacent tiles horizontally should differ
        for ty in 0..4 {
            for tx in 0..7 {
                let color1 = get_tile_color(tx, ty);
                let color2 = get_tile_color(tx + 1, ty);
                assert_ne!(
                    color1, color2,
                    "Horizontal checkerboard failed at tile ({}, {}): both tiles are 0x{:08X}",
                    tx, ty, color1
                );
            }
        }

        // Verify vertical checkerboard: adjacent tiles vertically should differ
        for ty in 0..3 {
            for tx in 0..8 {
                let color1 = get_tile_color(tx, ty);
                let color2 = get_tile_color(tx, ty + 1);
                assert_ne!(
                    color1, color2,
                    "Vertical checkerboard failed at tile ({}, {}): both tiles are 0x{:08X}",
                    tx, ty, color1
                );
            }
        }

        // Verify we actually have two distinct colors (not all black or all one color)
        use std::collections::HashSet;
        let mut unique_colors = HashSet::new();
        for ty in 0..4 {
            for tx in 0..8 {
                unique_colors.insert(get_tile_color(tx, ty));
            }
        }
        assert_eq!(
            unique_colors.len(),
            2,
            "Expected exactly 2 unique colors in checkerboard, got {}: {:?}",
            unique_colors.len(),
            unique_colors
                .iter()
                .map(|c| format!("0x{:08X}", c))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_controller_api() {
        let mut snes = SnesSystem::new();

        // Test setting controller with button constants
        snes.set_controller(0, controller::A | controller::B);

        // Verify the state was set in the bus
        let bus = snes.cpu.bus();
        assert_eq!(bus.controller_state[0], controller::A | controller::B);
    }

    #[test]
    fn test_controller_buttons() {
        let mut snes = SnesSystem::new();

        // Test individual buttons
        snes.set_controller(0, controller::START);
        assert_eq!(snes.cpu.bus().controller_state[0], 0x1000);

        // Test multiple buttons
        snes.set_controller(
            0,
            controller::A | controller::B | controller::UP | controller::DOWN,
        );
        assert_eq!(snes.cpu.bus().controller_state[0], 0x8C80);

        // Test controller 2
        snes.set_controller(
            1,
            controller::X | controller::Y | controller::L | controller::R,
        );
        assert_eq!(snes.cpu.bus().controller_state[1], 0x4070);
    }

    #[test]
    fn test_enhanced_rom() {
        // Load the enhanced test ROM
        let test_rom = include_bytes!("../../../../test_roms/snes/test_enhanced.sfc");

        let mut sys = SnesSystem::default();

        // Mount the test ROM
        assert!(sys.mount("Cartridge", test_rom).is_ok());
        assert!(sys.is_mounted("Cartridge"));

        // Run multiple frames to allow the ROM to initialize
        let mut frame = sys.step_frame().unwrap();
        for _ in 0..10 {
            frame = sys.step_frame().unwrap();
        }

        // Verify frame dimensions
        assert_eq!(frame.width, 256);
        assert_eq!(frame.height, 224);
        assert_eq!(frame.pixels.len(), 256 * 224);

        // Check that we have visible output (non-black pixels)
        let non_black_pixels = frame.pixels.iter().filter(|&&p| p != 0xFF000000).count();

        assert!(
            non_black_pixels > 1000,
            "Enhanced ROM should produce visible output, got {} non-black pixels",
            non_black_pixels
        );

        // Verify specific features:
        // 1. BG1 should have horizontal stripes (white, red, blue)
        // 2. Check top area (should be white or red or blue, not black)
        let sample_pixel = frame.pixels[64 * 256 + 128]; // Middle of screen
        assert_ne!(
            sample_pixel, 0xFF000000,
            "Middle of screen should not be black"
        );

        // 3. Sprites should be visible at positions (64, 64) and (128, 64)
        // Check area around sprite position
        let mut sprite_area_pixels = 0;
        for y in 60..72 {
            for x in 60..72 {
                if frame.pixels[y * 256 + x] != 0xFF000000 {
                    sprite_area_pixels += 1;
                }
            }
        }

        assert!(
            sprite_area_pixels > 10,
            "Sprite at (64, 64) should be visible, got {} non-black pixels in area",
            sprite_area_pixels
        );
    }
}
