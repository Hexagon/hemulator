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
mod mi;
mod pif;
mod rdp;
mod rdp_renderer;
#[cfg(feature = "opengl")]
mod rdp_renderer_opengl;
mod rdp_renderer_software;
mod rsp;
mod rsp_hle;
mod vi;

use bus::N64Bus;
#[cfg(test)]
use cartridge::N64_ROM_MAGIC;
use cpu::N64Cpu;
#[cfg(test)]
use cpu::{CP0_CONFIG_COMMERCIAL_BOOT, CP0_STATUS_COMMERCIAL_BOOT};
use emu_core::logging::{log, LogCategory, LogLevel};
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

/// Debug information for N64 system
///
/// Provides runtime information about the loaded ROM and system state
/// for display in debug overlays.
#[derive(Debug, Clone)]
pub struct DebugInfo {
    /// ROM name from cartridge header (20 bytes)
    pub rom_name: String,
    /// ROM size in megabytes
    pub rom_size_mb: f32,
    /// Current PC (program counter)
    pub pc: u64,
    /// RSP microcode type
    pub rsp_microcode: String,
    /// Number of vertices in RSP vertex buffer
    pub rsp_vertex_count: usize,
    /// RDP status flags
    pub rdp_status: u32,
    /// Frame buffer resolution
    pub framebuffer_resolution: String,
}

/// N64 system implementation
pub struct N64System {
    cpu: N64Cpu,
    frame_cycles: u32,
    current_cycles: u32,
}

// Re-export controller types for convenience
pub use pif::{ControllerButtons, ControllerState};

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

    /// Update controller 1 state
    pub fn set_controller1(&mut self, state: ControllerState) {
        self.cpu.bus_mut().set_controller1(state);
    }

    /// Update controller 2 state
    pub fn set_controller2(&mut self, state: ControllerState) {
        self.cpu.bus_mut().set_controller2(state);
    }

    /// Update controller 3 state
    pub fn set_controller3(&mut self, state: ControllerState) {
        self.cpu.bus_mut().set_controller3(state);
    }

    /// Update controller 4 state
    pub fn set_controller4(&mut self, state: ControllerState) {
        self.cpu.bus_mut().set_controller4(state);
    }

    /// Get debug information for the GUI overlay
    pub fn get_debug_info(&self) -> DebugInfo {
        let bus = self.cpu.bus();

        // Get ROM name from cartridge header (if available)
        let rom_name = if let Some(cart) = bus.cartridge() {
            // N64 ROM header has game name at offset 0x20 (32 bytes into ROM)
            let name_bytes = cart.read_range(0x20, 20);
            String::from_utf8_lossy(&name_bytes)
                .trim_end_matches('\0')
                .trim()
                .to_string()
        } else {
            "No ROM".to_string()
        };

        // Get ROM size
        let rom_size_mb = if let Some(cart) = bus.cartridge() {
            cart.size() as f32 / (1024.0 * 1024.0)
        } else {
            0.0
        };

        // Get RSP microcode type
        let rsp_microcode = match bus.rsp().microcode_type() {
            rsp_hle::MicrocodeType::F3DEX => "F3DEX".to_string(),
            rsp_hle::MicrocodeType::F3DEX2 => "F3DEX2".to_string(),
            rsp_hle::MicrocodeType::Audio => "Audio".to_string(),
            rsp_hle::MicrocodeType::Unknown => "Unknown".to_string(),
        };

        // Get RSP vertex count
        let rsp_vertex_count = bus.rsp().vertex_count();

        // Get RDP status
        let rdp_status = bus.rdp().read_register(0x0C); // DPC_STATUS register

        // Get framebuffer resolution
        let frame = bus.rdp().get_frame();
        let framebuffer_resolution = format!("{}x{}", frame.width, frame.height);

        DebugInfo {
            rom_name,
            rom_size_mb,
            pc: self.cpu.cpu.pc,
            rsp_microcode,
            rsp_vertex_count,
            rdp_status,
            framebuffer_resolution,
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

        // Log every 60th frame (once per second at 60fps)
        static mut FRAME_COUNTER: u32 = 0;
        unsafe {
            FRAME_COUNTER += 1;
            if FRAME_COUNTER % 60 == 0 {
                log(LogCategory::PPU, LogLevel::Info, || {
                    format!("N64: Frame {} complete", FRAME_COUNTER)
                });
            }
        }

        // NTSC has 262 scanlines per frame
        const SCANLINES_PER_FRAME: u32 = 262;
        let cycles_per_scanline = self.frame_cycles / SCANLINES_PER_FRAME;

        // Execute CPU cycles for one frame, updating VI scanline
        for scanline in 0..SCANLINES_PER_FRAME {
            let target_cycles = (scanline + 1) * cycles_per_scanline;

            // Execute CPU until we reach the cycles for this scanline
            while self.current_cycles < target_cycles {
                let cycles = self.cpu.step();
                self.current_cycles += cycles;

                // Check for pending interrupts in MI and route them to CPU
                let bus = self.cpu.bus();
                let pending = bus.mi().get_pending_interrupts();
                if pending != 0 {
                    // Map MI interrupt bits to MIPS interrupt lines
                    // SP (bit 0) -> IP2 (interrupt 2)
                    if pending & crate::mi::MI_INTR_SP != 0 {
                        self.cpu.cpu.set_interrupt(2);
                    }
                    // VI (bit 3) -> IP3 (interrupt 3)
                    if pending & crate::mi::MI_INTR_VI != 0 {
                        self.cpu.cpu.set_interrupt(3);
                    }
                    // DP (bit 5) -> IP5 (interrupt 5)
                    if pending & crate::mi::MI_INTR_DP != 0 {
                        self.cpu.cpu.set_interrupt(5);
                    }
                }
            }

            // Update VI scanline and check for interrupt
            let bus = self.cpu.bus_mut();
            if bus.vi_mut().update_scanline(scanline) {
                // VI interrupt triggered - set interrupt in MI
                bus.mi_mut().set_interrupt(crate::mi::MI_INTR_VI);

                log(LogCategory::Interrupts, LogLevel::Info, || {
                    format!("N64: VI interrupt triggered at scanline {}", scanline)
                });

                // Set interrupt pending bit in CPU's Cause register
                // VI interrupt is typically mapped to hardware interrupt 3 (bit 11 in Cause)
                self.cpu.cpu.set_interrupt(3);
            }
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
    use emu_core::cpu_mips_r4300i::MemoryMips;

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
        // word0: cmd(8) | XH(12) | YH(12)
        // word1: XL(12) | YL(12)
        bus.write_word(0x00100008, (0x36 << 24) | (0x258 << 12) | 0x258); // XH=150, YH=150
        bus.write_word(0x0010000C, (0xC8 << 12) | 0xC8); // XL=50, YL=50

        // Command 3: SET_FILL_COLOR (0x37) - Green (0xFF00FF00)
        bus.write_word(0x00100010, 0x37000000);
        bus.write_word(0x00100014, 0xFF00FF00);

        // Command 4: FILL_RECTANGLE (0x36) - 50x50 rectangle at (160,90)
        // 160*4=640(0x280), 210*4=840(0x348), 90*4=360(0x168), 140*4=560(0x230)
        bus.write_word(0x00100018, (0x36 << 24) | (0x348 << 12) | 0x230); // XH=210, YH=140
        bus.write_word(0x0010001C, (0x280 << 12) | 0x168); // XL=160, YL=90

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
        // Test that CPU boots properly with commercial ROM boot sequence
        let test_rom = include_bytes!("../../../../test_roms/n64/test.z64");
        let mut sys = N64System::default();

        // Initial PC should be at PIF ROM (0xBFC00000)
        assert_eq!(sys.cpu.cpu.pc, 0xBFC00000);

        // Mount ROM
        assert!(sys.mount("Cartridge", test_rom).is_ok());

        // After reset with commercial ROM, PC should be at entry point (0x80000400)
        // The IPL3 bootloader has copied the ROM to RDRAM and initialized the CPU
        assert_eq!(
            sys.cpu.cpu.pc, 0x80000400,
            "PC should be at ROM entry point after IPL3 boot, got 0x{:016X}",
            sys.cpu.cpu.pc
        );

        // Verify ROM was copied to RDRAM
        // Check that the first 4 bytes of RDRAM match the ROM header magic
        let rdram = sys.cpu.bus().rdram();
        assert_eq!(
            &rdram[0..4],
            &[0x80, 0x37, 0x12, 0x40],
            "ROM header should be copied to RDRAM"
        );

        // Verify code was copied to RDRAM at offset 0x1000
        // The test ROM has code starting at offset 0x1000
        let code_at_1000 =
            u32::from_be_bytes([rdram[0x1000], rdram[0x1001], rdram[0x1002], rdram[0x1003]]);
        assert_ne!(code_at_1000, 0, "Code should be copied to RDRAM at 0x1000");

        // Execute a few instructions from the entry point
        for _ in 0..10 {
            sys.cpu.step();
        }

        // Verify CPU is executing (PC should have changed)
        assert_ne!(
            sys.cpu.cpu.pc, 0x80000400,
            "CPU should have executed instructions and PC should have changed"
        );
    }

    #[test]
    fn test_ipl3_boot_complete() {
        // Comprehensive test for IPL3 bootloader functionality
        let test_rom = include_bytes!("../../../../test_roms/n64/test.z64");
        let mut sys = N64System::default();

        // Mount the ROM
        assert!(sys.mount("Cartridge", test_rom).is_ok());

        // Verify IPL3 completed all steps:

        // 1. ROM header copied to RDRAM (first 0x1000 bytes)
        let rdram = sys.cpu.bus().rdram();
        assert_eq!(&rdram[0..4], &N64_ROM_MAGIC, "ROM magic in RDRAM");

        // Check entry point in header (offset 0x08)
        let entry_in_rdram =
            u32::from_be_bytes([rdram[0x08], rdram[0x09], rdram[0x0A], rdram[0x0B]]);
        assert_eq!(entry_in_rdram, 0x80000400, "Entry point in RDRAM header");

        // 2. ROM code copied to RDRAM (from 0x1000 onwards)
        // Verify some code exists at 0x1000
        let has_code = rdram[0x1000..0x1100].iter().any(|&byte| byte != 0);
        assert!(has_code, "Code should be present in RDRAM at 0x1000+");

        // 3. Exception vector set up at 0x0180
        let exception_vec =
            u32::from_be_bytes([rdram[0x0180], rdram[0x0181], rdram[0x0182], rdram[0x0183]]);
        // Should be eret (0x42000018)
        assert_eq!(exception_vec, 0x42000018, "Exception vector set up");

        // 4. CP0 registers initialized
        assert_eq!(
            sys.cpu.cpu.cp0[12], CP0_STATUS_COMMERCIAL_BOOT,
            "CP0_STATUS initialized"
        );
        assert_eq!(
            sys.cpu.cpu.cp0[16], CP0_CONFIG_COMMERCIAL_BOOT,
            "CP0_CONFIG initialized"
        );

        // 5. PC set to entry point
        assert_eq!(sys.cpu.cpu.pc, 0x80000400, "PC at entry point");
    }

    #[test]
    fn test_mi_register_access() {
        // Test that MI registers can be accessed through memory bus
        let mut sys = N64System::new();
        let bus = sys.cpu.bus_mut();

        // Test reading MI_VERSION
        let version = bus.read_word(0x04300004);
        assert_eq!(version, 0x02020102);

        // Test writing to MI_INTR_MASK (enable VI interrupt)
        bus.write_word(0x0430000C, 0x0800); // Set VI interrupt mask
        let mask = bus.read_word(0x0430000C);
        assert_eq!(mask, 0x08); // VI interrupt bit should be set

        // Test reading MI_INTR (should be 0 initially)
        let intr = bus.read_word(0x04300008);
        assert_eq!(intr, 0);
    }

    #[test]
    fn test_vi_interrupt_generation() {
        // Test that VI generates interrupts when scanline matches VI_INTR
        let mut sys = N64System::new();

        // Enable VI interrupt in MI_INTR_MASK
        sys.cpu.bus_mut().write_word(0x0430000C, 0x0800);

        // Set VI_INTR to trigger on scanline 100 (stored as 200)
        sys.cpu.bus_mut().write_word(0x0440000C, 200);

        // Initially no interrupt
        let intr = sys.cpu.bus().read_word(0x04300008);
        assert_eq!(intr, 0, "No interrupt should be pending initially");

        // Manually trigger VI interrupt by updating scanline
        let should_trigger = sys.cpu.bus_mut().vi_mut().update_scanline(100);
        assert!(should_trigger, "VI should signal interrupt on scanline 100");

        // Set the interrupt in MI
        if should_trigger {
            sys.cpu
                .bus_mut()
                .mi_mut()
                .set_interrupt(crate::mi::MI_INTR_VI);
        }

        // Verify interrupt is now pending
        let intr = sys.cpu.bus().read_word(0x04300008);
        assert_eq!(intr, 0x08, "VI interrupt should be pending");

        // Verify MI reports pending interrupt
        assert!(sys.cpu.bus().mi().has_pending_interrupt());
    }

    #[test]
    fn test_interrupt_acknowledge() {
        // Test that writing to MI_INTR clears the interrupt
        let mut sys = N64System::new();

        // Set VI interrupt
        sys.cpu
            .bus_mut()
            .mi_mut()
            .set_interrupt(crate::mi::MI_INTR_VI);
        let intr = sys.cpu.bus().read_word(0x04300008);
        assert_eq!(intr, 0x08);

        // Clear interrupt by writing to MI_INTR
        sys.cpu.bus_mut().write_word(0x04300008, 0x08);
        let intr = sys.cpu.bus().read_word(0x04300008);
        assert_eq!(intr, 0, "Interrupt should be cleared");
    }

    #[test]
    fn test_cpu_interrupt_handling() {
        // Test that CPU responds to interrupts
        let mut sys = N64System::new();

        // Enable interrupts in CPU Status register
        // Set IE (bit 0) and enable interrupt 3 (VI) in IM field (bit 11)
        let status = sys.cpu.cpu.cp0[12]; // CP0_STATUS
        sys.cpu.cpu.cp0[12] = status | 0x01 | (1 << 11); // IE=1, IM3=1

        // Set a known PC
        sys.cpu.cpu.pc = 0x80000000;

        // Trigger an interrupt in CPU
        sys.cpu.cpu.set_interrupt(3); // VI interrupt

        // Check that interrupt is pending in Cause register
        let cause = sys.cpu.cpu.cp0[13]; // CP0_CAUSE
        assert_ne!(
            cause & (1 << 11),
            0,
            "Interrupt 3 should be pending in Cause"
        );

        // Execute one instruction - should handle interrupt
        let old_pc = sys.cpu.cpu.pc;
        sys.cpu.step();

        // After interrupt, PC should be at exception vector
        assert_eq!(
            sys.cpu.cpu.pc, 0x80000180,
            "PC should jump to exception vector"
        );

        // EPC should contain the return address
        let epc = sys.cpu.cpu.cp0[14]; // CP0_EPC
        assert_eq!(epc, old_pc, "EPC should contain return address");

        // EXL bit should be set in Status
        let status = sys.cpu.cpu.cp0[12];
        assert_ne!(status & 0x02, 0, "EXL bit should be set in Status");
    }

    #[test]
    fn test_full_interrupt_flow() {
        // Integration test: VI generates interrupt, MI propagates it, CPU handles it
        let mut sys = N64System::new();

        // Setup: Enable interrupts in CPU and MI
        let status = sys.cpu.cpu.cp0[12];
        sys.cpu.cpu.cp0[12] = status | 0x01 | (1 << 11); // IE=1, IM3=1
        sys.cpu.bus_mut().write_word(0x0430000C, 0x0800); // Enable VI in MI

        // Set VI_INTR to trigger on scanline 50
        sys.cpu.bus_mut().write_word(0x0440000C, 100); // Scanline 50 * 2

        // Set a known PC
        sys.cpu.cpu.pc = 0x80000000;
        let old_pc = sys.cpu.cpu.pc;

        // Trigger VI interrupt manually (simulating what happens during frame execution)
        let should_trigger = sys.cpu.bus_mut().vi_mut().update_scanline(50);
        assert!(should_trigger);

        sys.cpu
            .bus_mut()
            .mi_mut()
            .set_interrupt(crate::mi::MI_INTR_VI);
        sys.cpu.cpu.set_interrupt(3);

        // Execute instruction - should handle interrupt
        sys.cpu.step();

        // Verify interrupt was handled
        assert_eq!(
            sys.cpu.cpu.pc, 0x80000180,
            "PC should be at exception vector"
        );
        assert_eq!(
            sys.cpu.cpu.cp0[14], old_pc,
            "EPC should contain return address"
        );
    }

    #[test]
    fn test_controller_input_integration() {
        // Test that controller input flows from system to PIF correctly
        use emu_core::cpu_mips_r4300i::MemoryMips;

        let mut sys = N64System::new();

        // Create a controller state with some buttons pressed
        let mut controller_state = crate::pif::ControllerState::default();
        controller_state.buttons.a = true;
        controller_state.buttons.start = true;
        controller_state.buttons.d_up = true;
        controller_state.stick_x = 64;
        controller_state.stick_y = -32;

        // Set controller 1 state
        sys.set_controller1(controller_state);

        // Simulate game reading controller via PIF RAM (at 0x1FC007C0)
        let bus = sys.cpu.bus_mut();

        // Write controller read command
        bus.write_byte(0x1FC007C0, 0x01); // T=1 byte
        bus.write_byte(0x1FC007C1, 0x04); // R=4 bytes
        bus.write_byte(0x1FC007C2, 0x01); // Command 0x01 (read controller)

        // Read response
        let buttons_hi = bus.read_byte(0x1FC007C3);
        let buttons_lo = bus.read_byte(0x1FC007C4);
        let stick_x = bus.read_byte(0x1FC007C5) as i8;
        let stick_y = bus.read_byte(0x1FC007C6) as i8;

        // Verify button bits (A=bit 15, Start=bit 12, D-Up=bit 11)
        let buttons = u16::from_be_bytes([buttons_hi, buttons_lo]);
        assert_ne!(buttons & (1 << 15), 0, "A button should be pressed");
        assert_ne!(buttons & (1 << 12), 0, "Start button should be pressed");
        assert_ne!(buttons & (1 << 11), 0, "D-Up should be pressed");
        assert_eq!(buttons & (1 << 14), 0, "B button should not be pressed");

        // Verify analog stick
        assert_eq!(stick_x, 64);
        assert_eq!(stick_y, -32);
    }

    #[test]
    fn test_controller_multi_player() {
        // Test that multiple controllers work independently
        use emu_core::cpu_mips_r4300i::MemoryMips;

        let mut sys = N64System::new();

        // Set different states for controllers 1 and 2
        let mut state1 = crate::pif::ControllerState::default();
        state1.buttons.a = true;
        sys.set_controller1(state1);

        let mut state2 = crate::pif::ControllerState::default();
        state2.buttons.b = true;
        sys.set_controller2(state2);

        let bus = sys.cpu.bus_mut();

        // Read controller 1 (at 0x1FC007C0)
        bus.write_byte(0x1FC007C0, 0x01);
        bus.write_byte(0x1FC007C1, 0x04);
        bus.write_byte(0x1FC007C2, 0x01);
        let buttons1 = u16::from_be_bytes([bus.read_byte(0x1FC007C3), bus.read_byte(0x1FC007C4)]);

        // Read controller 2 (at 0x1FC007C8)
        bus.write_byte(0x1FC007C8, 0x01);
        bus.write_byte(0x1FC007C9, 0x04);
        bus.write_byte(0x1FC007CA, 0x01);
        let buttons2 = u16::from_be_bytes([bus.read_byte(0x1FC007CB), bus.read_byte(0x1FC007CC)]);

        // Verify controller 1 has A pressed
        assert_ne!(buttons1 & (1 << 15), 0);
        assert_eq!(buttons1 & (1 << 14), 0);

        // Verify controller 2 has B pressed
        assert_eq!(buttons2 & (1 << 15), 0);
        assert_ne!(buttons2 & (1 << 14), 0);
    }
}
