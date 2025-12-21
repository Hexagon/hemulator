//! N64 (Nintendo 64) emulation implementation.
//!
//! This module provides a basic N64 system emulator using the MIPS R4300i CPU core
//! from `emu_core`, along with N64-specific components:
//!
//! - **CPU**: MIPS R4300i (64-bit processor running at 93.75 MHz)
//! - **RCP**: Reality Co-Processor (graphics and audio)
//!   - **RDP**: Reality Display Processor (graphics rasterization)
//!   - **RSP**: Reality Signal Processor (geometry/audio processing - stub)
//! - **Memory**: 4MB RDRAM + cartridge ROM
//! - **Timing**: NTSC (~60 Hz frame rate)

#![allow(clippy::upper_case_acronyms)]

mod bus;
mod cartridge;
mod cpu;
mod rdp;
mod rdp_renderer;
mod rdp_renderer_software;
mod rsp;
mod vi;

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
        self.cpu.bus_mut().rdp_mut().reset();
        self.current_cycles = 0;
    }

    fn step_frame(&mut self) -> Result<Frame, Self::Error> {
        self.current_cycles = 0;

        // Execute CPU cycles for one frame
        while self.current_cycles < self.frame_cycles {
            let cycles = self.cpu.step();
            self.current_cycles += cycles;
        }

        // Get frame from RDP
        let frame = self.cpu.bus().rdp().get_frame().clone();
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
    fn test_rdp_integration() {
        let sys = N64System::new();
        let frame = sys.cpu.bus().rdp().get_frame();
        assert_eq!(frame.width, 320);
        assert_eq!(frame.height, 240);
        assert_eq!(frame.pixels.len(), 320 * 240);
    }

    #[test]
    fn test_rdp_register_access() {
        use emu_core::cpu_mips_r4300i::MemoryMips;

        let mut sys = N64System::new();
        let bus = sys.cpu.bus_mut();

        // Write to RDP START register
        bus.write_word(0x04100000, 0x00123456);
        assert_eq!(bus.read_word(0x04100000), 0x00123456);

        // Write to RDP END register
        bus.write_word(0x04100004, 0x00789ABC);
        assert_eq!(bus.read_word(0x04100004), 0x00789ABC);

        // Read STATUS register
        let status = bus.read_word(0x0410000C);
        assert_ne!(status, 0); // Should have CBUF_READY bit set
    }

    #[test]
    fn test_step_frame_returns_rdp_frame() {
        let mut sys = N64System::new();
        let result = sys.step_frame();
        assert!(result.is_ok());

        let frame = result.unwrap();
        assert_eq!(frame.width, 320);
        assert_eq!(frame.height, 240);
    }

    #[test]
    fn test_n64_display_list_smoke_test() {
        use emu_core::cpu_mips_r4300i::MemoryMips;

        let mut sys = N64System::new();
        let bus = sys.cpu.bus_mut();

        // Manually write display list commands to RDRAM at 0x00100000
        // This simulates what the CPU would do after booting

        // Command 1: SET_FILL_COLOR (0x37) - Red (0xFFFF0000)
        bus.write_word(0x00100000, 0x37000000);
        bus.write_word(0x00100004, 0xFFFF0000);

        // Command 2: FILL_RECTANGLE (0x36) - 100x100 rectangle at (50,50)
        // Coordinates in 10.2 fixed point: 50*4=200(0xC8), 150*4=600(0x258)
        // word0: cmd | XH << 14 | YH << 2
        // word1: XL << 14 | YL << 2
        bus.write_word(0x00100008, (0x36 << 24) | (0x258 << 14) | (0x258 << 2)); // XH=150, YH=150
        bus.write_word(0x0010000C, (0xC8 << 14) | (0xC8 << 2)); // XL=50, YL=50

        // Command 3: SET_FILL_COLOR (0x37) - Green (0xFF00FF00)
        bus.write_word(0x00100010, 0x37000000);
        bus.write_word(0x00100014, 0xFF00FF00);

        // Command 4: FILL_RECTANGLE (0x36) - 50x50 rectangle at (160,90)
        // 160*4=640(0x280), 210*4=840(0x348), 90*4=360(0x168), 140*4=560(0x230)
        bus.write_word(0x00100018, (0x36 << 24) | (0x348 << 14) | (0x230 << 2)); // XH=210, YH=140
        bus.write_word(0x0010001C, (0x280 << 14) | (0x168 << 2)); // XL=160, YL=90

        // Command 5: SYNC_FULL (0x29)
        bus.write_word(0x00100020, 0x29000000);
        bus.write_word(0x00100024, 0x00000000);

        // Trigger RDP by writing to DPC_START and DPC_END
        bus.write_word(0x04100000, 0x00100000); // DPC_START
        bus.write_word(0x04100004, 0x00100028); // DPC_END (40 bytes = 5 commands)

        // Get the rendered frame
        let frame = sys.cpu.bus().rdp().get_frame();
        assert_eq!(frame.width, 320);
        assert_eq!(frame.height, 240);

        // Verify that pixels have been colored by the display list
        // Red rectangle at (50,50) to (150,150)
        let red_pixel_idx = (100 * 320 + 100) as usize;
        let red_pixel = frame.pixels[red_pixel_idx];
        assert_eq!(red_pixel, 0xFFFF0000, "Expected red pixel at (100,100)");

        // Green rectangle at (160,90) to (210,140)
        let green_pixel_idx = (115 * 320 + 185) as usize;
        let green_pixel = frame.pixels[green_pixel_idx];
        assert_eq!(green_pixel, 0xFF00FF00, "Expected green pixel at (185,115)");

        // Check that a pixel outside both rectangles is still black
        let black_pixel = frame.pixels[0];
        assert_eq!(black_pixel, 0, "Expected black pixel at (0,0)");
    }

    #[test]
    fn test_n64_3d_rendering_demo() {
        // Demonstrate 3D triangle rendering with Z-buffer
        let mut sys = N64System::new();

        // Enable Z-buffer and clear it
        sys.cpu.bus_mut().rdp_mut().set_zbuffer_enabled(true);
        sys.cpu.bus_mut().rdp_mut().clear_zbuffer();

        // Draw a 3D scene with multiple triangles at different depths
        // This simulates a simple 3D pyramid

        // Back face (far) - Red triangle
        sys.cpu.bus_mut().rdp_mut().draw_triangle_zbuffer(
            160, 80, 0xC000, // Top vertex (far)
            220, 180, 0xC000, // Bottom-right
            100, 180, 0xC000,     // Bottom-left
            0xFFFF0000, // Red
        );

        // Left face (medium depth) - Green triangle
        sys.cpu.bus_mut().rdp_mut().draw_triangle_zbuffer(
            160, 80, 0x8000, // Top vertex (medium)
            100, 180, 0x8000, // Bottom-left
            80, 140, 0x8000,     // Side vertex
            0xFF00FF00, // Green
        );

        // Right face (near) - Blue triangle with Gouraud shading
        // Demonstrates both Z-buffer and color interpolation
        sys.cpu.bus_mut().rdp_mut().draw_triangle_shaded_zbuffer(
            160, 80, 0x6000, 0xFF0000FF, // Top: Blue, near
            220, 180, 0x6000, 0xFF00FFFF, // Bottom-right: Cyan, near
            240, 140, 0x6000, 0xFFFF00FF, // Side: Magenta, near
        );

        // Get the rendered frame
        let frame = sys.cpu.bus().rdp().get_frame();

        // Verify the scene was rendered
        // Check that the near blue face is visible (should have blue component)
        let near_pixel_idx = (120 * 320 + 200) as usize;
        let near_pixel = frame.pixels[near_pixel_idx];
        let blue_component = near_pixel & 0xFF;
        assert!(
            blue_component > 0,
            "Expected blue component in near triangle"
        );

        // Check that far pixels exist but may be occluded
        let far_pixel_idx = (120 * 320 + 160) as usize;
        let far_pixel = frame.pixels[far_pixel_idx];
        assert_ne!(far_pixel, 0, "Expected some color in the rendered scene");

        // Verify frame dimensions
        assert_eq!(frame.width, 320);
        assert_eq!(frame.height, 240);
    }

    #[test]
    #[ignore] // TODO: Debug why test ROM doesn't render - CPU executes but RDP not triggered
    fn test_n64_smoke_test_rom() {
        // Load the test ROM which displays colored rectangles
        let test_rom = include_bytes!("../../../../test_roms/n64/test.z64");

        let mut sys = N64System::default();

        // Mount the test ROM
        assert!(sys.mount("Cartridge", test_rom).is_ok());
        assert!(sys.is_mounted("Cartridge"));

        // Run a few frames to let the ROM execute and render
        // The test ROM writes to RDP registers and triggers display list processing
        let mut frame = sys.step_frame().unwrap();
        for _ in 0..5 {
            frame = sys.step_frame().unwrap();
        }

        // Verify frame dimensions
        assert_eq!(frame.width, 320);
        assert_eq!(frame.height, 240);
        assert_eq!(frame.pixels.len(), 320 * 240);

        // The test ROM displays two colored rectangles:
        // 1. Red rectangle at (50,50) to (150,150)
        // 2. Green rectangle at (160,90) to (210,140)

        // Verify red pixel in center of first rectangle (100,100)
        let red_pixel_idx = (100 * 320 + 100) as usize;
        let red_pixel = frame.pixels[red_pixel_idx];
        assert_eq!(
            red_pixel, 0xFFFF0000,
            "Expected red pixel at (100,100), got 0x{:08X}",
            red_pixel
        );

        // Verify green pixel in center of second rectangle (185,115)
        let green_pixel_idx = (115 * 320 + 185) as usize;
        let green_pixel = frame.pixels[green_pixel_idx];
        assert_eq!(
            green_pixel, 0xFF00FF00,
            "Expected green pixel at (185,115), got 0x{:08X}",
            green_pixel
        );

        // Verify background is black (pixel outside rectangles)
        let black_pixel = frame.pixels[0];
        assert_eq!(
            black_pixel, 0,
            "Expected black background at (0,0), got 0x{:08X}",
            black_pixel
        );
    }

    #[test]
    fn test_n64_cpu_boot_sequence() {
        // Test that CPU boots from PIF ROM and executes cartridge code
        let test_rom = include_bytes!("../../../../test_roms/n64/test.z64");
        let mut sys = N64System::default();

        // Initial PC should be at PIF ROM (0xBFC00000)
        assert_eq!(sys.cpu.cpu.pc, 0xBFC00000);

        // Mount ROM
        assert!(sys.mount("Cartridge", test_rom).is_ok());

        // After reset, PC should still be at PIF ROM
        assert_eq!(sys.cpu.cpu.pc, 0xBFC00000);

        // Execute a few instructions to complete PIF ROM boot sequence
        // PIF ROM now just has 4 instructions (lui, ori, jr, nop)
        for _ in 0..10 {
            sys.cpu.step();
        }

        // Verify we're now executing code from cartridge ROM
        // PC should be in range 0x90001000+ (possibly sign-extended to 0xFFFFFFFF90001000+)
        let pc_low = (sys.cpu.cpu.pc & 0xFFFFFFFF) as u32;
        assert!(
            (0x90001000..0x90002000).contains(&pc_low),
            "CPU should be in cartridge ROM after boot, but PC is 0x{:016X} (low 32 bits: 0x{:08X})",
            sys.cpu.cpu.pc,
            pc_low
        );
    }
}
