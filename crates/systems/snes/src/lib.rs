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
pub mod coprocessors;
mod cpu;
mod debugger;
mod ppu;
pub mod ppu_renderer;

use emu_core::debug::Debugger;
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

/// Data for the tile viewer tab (SNES)
#[derive(Clone)]
pub struct TileViewerData {
    /// VRAM data (64KB)
    pub vram: Vec<u8>,
    /// CGRAM data (512 bytes - 256 colors x 2 bytes each)
    pub cgram: Vec<u8>,
    /// OAM data (544 bytes - 512 main + 32 high table)
    pub oam: Vec<u8>,
    /// Palette colors as RGB (256 colors)
    pub palette: Vec<u32>,
    /// Current BG mode (0-7)
    pub bg_mode: u8,
    /// Screen enable status
    pub screen_enabled: bool,
}

/// SNES system implementation
pub struct SnesSystem {
    cpu: SnesCpu,
    frame_cycles: u32,
    current_cycles: u32,
    /// Total CPU cycles executed since reset
    total_cycles: u64,
    renderer: Box<dyn SnesPpuRenderer>,
    /// Instruction tracer for debugging
    pub(crate) instruction_tracer: emu_core::instruction_tracer::InstructionTracer,
    /// Breakpoint manager for debugging
    pub(crate) breakpoint_manager: emu_core::breakpoints::BreakpointManager,
}

// SNES timing constants (NTSC)
const SNES_FRAME_CYCLES: u32 = 89342; // ~3.58MHz / 60Hz
const SNES_SCANLINE_CYCLES: u32 = 341; // Cycles per scanline (~3.58MHz / 262 scanlines / 60Hz)
const SNES_VISIBLE_SCANLINES: u32 = 224; // Visible scanlines

impl SnesSystem {
    /// Create a new SNES system
    pub fn new() -> Self {
        let bus = SnesBus::new();
        Self {
            cpu: SnesCpu::new(bus),
            frame_cycles: SNES_FRAME_CYCLES,
            current_cycles: 0,
            total_cycles: 0,
            renderer: Box::new(SoftwareSnesPpuRenderer::new()),
            instruction_tracer: emu_core::instruction_tracer::InstructionTracer::new(),
            breakpoint_manager: emu_core::breakpoints::BreakpointManager::new(),
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

    emu_core::impl_instruction_tracer_methods!();

    /// Check if instruction tracing is enabled
    pub fn is_instruction_tracing_enabled(&self) -> bool {
        self.instruction_tracer.is_enabled()
    }

    emu_core::impl_breakpoint_methods!();

    /// Enable or disable breakpoints
    pub fn set_breakpoints_enabled(&mut self, enabled: bool) {
        self.breakpoint_manager.set_enabled(enabled);
    }

    /// Get the breakpoint manager
    pub fn get_breakpoint_manager(&self) -> &emu_core::breakpoints::BreakpointManager {
        &self.breakpoint_manager
    }

    /// Check if the current PC is at a breakpoint
    /// Returns Some(pc) if a breakpoint is hit, None otherwise
    pub fn check_breakpoint(&self) -> Option<u32> {
        let pc = ((self.cpu.cpu.pbr as u32) << 16) | (self.cpu.cpu.pc as u32);
        if self.breakpoint_manager.should_break_execute(pc) {
            Some(pc)
        } else {
            None
        }
    }

    /// Get tile viewer data for debugging
    pub fn get_tile_viewer_data(&self) -> TileViewerData {
        self.cpu.bus().ppu().get_tile_viewer_data()
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
        self.total_cycles = 0;

        // Pre-run the SPC700 APU to let it complete its boot sequence
        // The IPL ROM clears RAM and writes $AA/$BB signature (takes ~5000 cycles)
        // This ensures the ready signature is available when main CPU starts
        if let Some(ref mut spc700) = self.cpu.bus_mut().spc700_mut() {
            log(LogCategory::APU, LogLevel::Info, || {
                "SNES: Pre-running SPC700 for boot sequence".to_string()
            });
            spc700.run_cycles(6000);
        }
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

        // Initialize HDMA at start of frame
        self.cpu.bus_mut().init_hdma();

        // Execute CPU cycles for visible scanlines, with HDMA during H-blank
        for scanline in 0..SNES_VISIBLE_SCANLINES {
            let scanline_target = (scanline + 1) * SNES_SCANLINE_CYCLES;

            // Active display starts at the beginning of each scanline
            self.cpu.bus_mut().ppu_mut().set_hblank(false);

            // Log CPU state on first scanline of first few frames for debugging
            if scanline == 0 && self.current_cycles < 10000 {
                log(LogCategory::CPU, LogLevel::Debug, || {
                    format!(
                        "SNES CPU: PC=${:02X}:{:04X}, A=${:04X}, X=${:04X}, Y=${:04X}, S=${:04X}, P=${:02X}, E={}",
                        self.cpu.cpu.pbr, self.cpu.cpu.pc, self.cpu.cpu.c, self.cpu.cpu.x,
                        self.cpu.cpu.y, self.cpu.cpu.s, self.cpu.cpu.status, self.cpu.cpu.emulation
                    )
                });
            }

            // Execute CPU until end of active display portion of scanline
            while self.current_cycles < scanline_target.saturating_sub(40) {
                let pc_before = ((self.cpu.cpu.pbr as u32) << 16) | (self.cpu.cpu.pc as u32);
                self.cpu.bus_mut().set_last_cpu_pc(pc_before);
                let cycles = self.cpu.step();
                self.current_cycles += cycles;
                self.total_cycles += cycles as u64;
                self.cpu.bus_mut().tick_cycles(cycles);

                // Record instruction if tracing is enabled
                if self.instruction_tracer.is_enabled() {
                    if let Some(instr) = self.disassemble_instruction(pc_before) {
                        let cpu_state = self.get_cpu_state();
                        self.instruction_tracer.trace(instr, cpu_state);
                    }
                }
            }

            // Enter HBlank for the remainder of the scanline.
            // Many games (including SMW) rely on VRAM/CGRAM writes being possible during HBlank.
            self.cpu.bus_mut().ppu_mut().set_hblank(true);

            // Execute HDMA during H-blank (approximately 40 cycles)
            let _hdma_cycles = self.cpu.bus_mut().do_hdma();

            // Complete the scanline
            while self.current_cycles < scanline_target {
                let pc_before = ((self.cpu.cpu.pbr as u32) << 16) | (self.cpu.cpu.pc as u32);
                self.cpu.bus_mut().set_last_cpu_pc(pc_before);
                let cycles = self.cpu.step();
                self.current_cycles += cycles;
                self.total_cycles += cycles as u64;
                self.cpu.bus_mut().tick_cycles(cycles);

                // Record instruction if tracing is enabled
                if self.instruction_tracer.is_enabled() {
                    if let Some(instr) = self.disassemble_instruction(pc_before) {
                        let cpu_state = self.get_cpu_state();
                        self.instruction_tracer.trace(instr, cpu_state);
                    }
                }
            }
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
            log(LogCategory::Interrupts, LogLevel::Debug, || {
                format!(
                    "SNES: After NMI trigger, PC is now ${:02X}:{:04X}",
                    self.cpu.cpu.pbr, self.cpu.cpu.pc
                )
            });
        }

        // Execute remaining VBlank cycles
        while self.current_cycles < self.frame_cycles {
            let pc_before = ((self.cpu.cpu.pbr as u32) << 16) | (self.cpu.cpu.pc as u32);
            self.cpu.bus_mut().set_last_cpu_pc(pc_before);
            let cycles = self.cpu.step();
            self.current_cycles += cycles;
            self.total_cycles += cycles as u64;
            self.cpu.bus_mut().tick_cycles(cycles);

            // Record instruction if tracing is enabled
            if self.instruction_tracer.is_enabled() {
                if let Some(instr) = self.disassemble_instruction(pc_before) {
                    let cpu_state = self.get_cpu_state();
                    self.instruction_tracer.trace(instr, cpu_state);
                }
            }

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

    fn debugger(&self) -> Option<&dyn emu_core::debug::Debugger> {
        Some(self)
    }

    fn get_total_cycles(&self) -> u64 {
        self.total_cycles
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

    #[test]
    fn test_priority_rom() {
        // Load the priority test ROM
        let test_rom = include_bytes!("../../../../test_roms/snes/test_priority.sfc");

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
            "Priority test ROM should produce visible output, got {} non-black pixels",
            non_black_pixels
        );

        // Verify priority rendering:
        // Left half should have low-priority tiles (red/green)
        // Right half should have high-priority tiles (blue/yellow)
        // High-priority tiles should render in front of low-priority tiles

        // Sample from left half (low priority)
        let left_pixel = frame.pixels[112 * 256 + 64];
        assert_ne!(
            left_pixel, 0xFF000000,
            "Left side (low priority) should not be black"
        );

        // Sample from right half (high priority)
        let right_pixel = frame.pixels[112 * 256 + 192];
        assert_ne!(
            right_pixel, 0xFF000000,
            "Right side (high priority) should not be black"
        );
    }

    #[test]
    #[ignore] // TODO: Upload protocol needs further investigation - SPC700 isn't echoing indices
    fn test_apu_upload_protocol() {
        // This test simulates the FULL commercial game APU upload protocol:
        // 1. Wait for IPL ready ($BBAA)
        // 2. Send upload command ($CC)
        // 3. Upload data with index echoing
        // 4. End upload with $00 $00
        // 5. Clear ports and wait for ready AGAIN
        // This is what real games like Super Mario World do.

        let test_rom = include_bytes!("../../../../test_roms/snes/test_apu_upload.sfc");

        let mut sys = SnesSystem::default();
        assert!(sys.mount("Cartridge", test_rom).is_ok());

        use emu_core::debug::Debugger;

        for i in 0..100 {
            let _ = sys.step_frame().unwrap();

            // Check progress markers:
            // $0100 = $01: passed first ready wait
            // $0101 = $02: completed first upload
            // $0102 = $03: got end-of-upload echo
            // $0103 = $04: passed second ready wait (CRITICAL!)
            // $0110 = $FF: timeout occurred

            let markers = sys.read_memory(0x0100, 0x14).unwrap();

            // Also check APU ports to see what's happening
            let apu_ports = sys.read_memory(0x2140, 4).unwrap();

            if i > 5 && i % 5 == 0 {
                println!("Frame {}: Markers={:02X} {:02X} {:02X} {:02X}, APU ports={:02X} {:02X} {:02X} {:02X}, PC={:04X}", 
                    i, markers[0], markers[1], markers[2], markers[3],
                    apu_ports[0], apu_ports[1], apu_ports[2], apu_ports[3],
                    sys.get_cpu_state().pc);
            }

            if markers[0x10] == 0xFF {
                panic!(
                    "APU upload test TIMEOUT waiting for ready signal after {} frames.\n\
                     Progress: $0100-$0103 = {:02X} {:02X} {:02X} {:02X}",
                    i + 1,
                    markers[0],
                    markers[1],
                    markers[2],
                    markers[3]
                );
            }

            if markers[3] == 0x04 {
                println!("APU upload protocol test PASSED after {} frames", i + 1);
                println!(
                    "All markers: $0100=${:02X}, $0101=${:02X}, $0102=${:02X}, $0103=${:02X}",
                    markers[0], markers[1], markers[2], markers[3]
                );
                return;
            }

            if i > 20 && markers[0] == 0x01 && markers[3] == 0x00 {
                // We're stuck waiting for second ready
                panic!(
                    "APU upload test FAILED: Got stuck waiting for second ready signal after {} frames.\n\
                     Progress: $0100=${:02X} (first ready OK), $0101=${:02X} (upload status), \
                     $0102=${:02X} (echo status), $0103=${:02X} (second ready - STUCK!)\n\
                     APU ports: {:02X} {:02X} {:02X} {:02X}, CPU PC={:04X}",
                    i + 1, markers[0], markers[1], markers[2], markers[3],
                    apu_ports[0], apu_ports[1], apu_ports[2], apu_ports[3],
                    sys.get_cpu_state().pc
                );
            }
        }

        let markers = sys.read_memory(0x0100, 0x14).unwrap();
        panic!(
            "APU upload protocol test TIMEOUT after 100 frames.\n\
             Final markers: $0100-$0103 = {:02X} {:02X} {:02X} {:02X}, timeout flag $0110=${:02X}",
            markers[0], markers[1], markers[2], markers[3], markers[0x10]
        );
    }

    #[test]
    fn test_apu_double_upload_rom() {
        // This test simulates commercial game behavior where the main CPU
        // waits for the APU ready signature ($BBAA) twice during initialization.
        // This is critical for testing the timing-sensitive APU port synchronization.

        let test_rom = include_bytes!("../../../../test_roms/snes/test_apu_double_upload.sfc");

        let mut sys = SnesSystem::default();

        // Mount the test ROM
        assert!(sys.mount("Cartridge", test_rom).is_ok());
        assert!(sys.is_mounted("Cartridge"));

        // Run frames and check memory markers
        // The ROM writes $01 to $0100 after first APU ready check
        // and $02 to $0101 after second APU ready check
        use emu_core::debug::Debugger;

        for i in 0..100 {
            let _ = sys.step_frame().unwrap();

            // Check WRAM for progress markers
            let marker1_bytes = sys.read_memory(0x0100, 1).unwrap();
            let marker2_bytes = sys.read_memory(0x0101, 1).unwrap();
            let marker1 = marker1_bytes[0];
            let marker2 = marker2_bytes[0];

            if marker1 == 0x01 && marker2 == 0x02 {
                // Both APU ready checks succeeded!
                println!("APU double upload test PASSED after {} frames", i + 1);
                return;
            }

            if marker1 == 0x01 && marker2 == 0x00 && i > 20 {
                // First check passed but second check is stuck
                panic!(
                    "APU double upload test FAILED: First ready check passed but stuck on second check after {} frames",
                    i + 1
                );
            }
        }

        // If we get here, check what we achieved
        let marker1_bytes = sys.read_memory(0x0100, 1).unwrap();
        let marker2_bytes = sys.read_memory(0x0101, 1).unwrap();
        let marker1 = marker1_bytes[0];
        let marker2 = marker2_bytes[0];

        panic!(
            "APU double upload test TIMEOUT after 100 frames. Markers: $0100=${:02X}, $0101=${:02X}",
            marker1, marker2
        );
    }

    #[test]
    #[ignore] // SNES sprite rendering not fully implemented yet - sprites not showing up
    fn test_sprite_overflow_rom() {
        // Load the sprite overflow test ROM
        let test_rom = include_bytes!("../../../../test_roms/snes/test_sprite_overflow.sfc");

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

        // This ROM places 128 sprites all at Y=100
        // Due to the 32 sprite per scanline limit, only the first 32 should render
        // Count non-black pixels on scanline 100
        let mut pixels_on_scanline = 0;
        for x in 0..256 {
            if frame.pixels[100 * 256 + x] != 0xFF000000 {
                pixels_on_scanline += 1;
            }
        }

        // We should see some sprites (at least 10 pixels worth)
        // but not all 128 sprites (which would be ~128 pixels)
        assert!(
            pixels_on_scanline >= 10,
            "Should see some sprites on scanline 100, got {} pixels",
            pixels_on_scanline
        );

        // With 32 sprite limit and 8x8 sprites, we expect at most 32 sprites visible
        // That's at most 256 pixels (32 * 8), but in practice less due to spacing
        // Just verify we don't crash and produce some output
        let non_black_pixels = frame.pixels.iter().filter(|&&p| p != 0xFF000000).count();
        assert!(
            non_black_pixels > 10,
            "Sprite overflow ROM should produce visible output, got {} non-black pixels",
            non_black_pixels
        );
    }

    #[test]
    fn test_simple_sprite_rom() {
        // Load the simple sprite test ROM
        // This ROM displays a single 8x8 red sprite at position (100, 100)
        // with only OBJ layer enabled (TM=0x10), replicating SMW's config
        let test_rom = include_bytes!("../../../../test_roms/snes/test_simple_sprite.sfc");

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

        // This ROM places a single 8x8 sprite at (100, 100)
        // The sprite should be visible (solid red color index 1)
        // Note: SNES sprites appear 1 scanline later than their Y value,
        // so a sprite with Y=100 will render at Y=101-108

        // Count non-black pixels in the actual sprite area (101-108, 100-107)
        // The sprite appears at Y=101 (Y+1 offset) and is 8x8 pixels
        let mut sprite_pixels = 0;
        for y in 101..109 {
            for x in 100..108 {
                if frame.pixels[y * 256 + x] != 0xFF000000 {
                    sprite_pixels += 1;
                }
            }
        }

        // Count total non-black pixels
        let non_black_pixels = frame.pixels.iter().filter(|&&p| p != 0xFF000000).count();

        // We should see the full 8x8=64 pixel sprite
        // The sprite is solid (all pixels color index 1)
        const EXPECTED_SPRITE_PIXELS: usize = 64;
        assert!(
            sprite_pixels == EXPECTED_SPRITE_PIXELS,
            "Should see full 8x8 sprite at (100,100), got {} non-black pixels (expected {})",
            sprite_pixels,
            EXPECTED_SPRITE_PIXELS
        );

        // Verify the frame has the expected non-black pixels (should match sprite pixels exactly)
        assert!(
            non_black_pixels == EXPECTED_SPRITE_PIXELS,
            "Simple sprite ROM should produce full sprite output, got {} non-black pixels (expected {})",
            non_black_pixels,
            EXPECTED_SPRITE_PIXELS
        );
    }
}
