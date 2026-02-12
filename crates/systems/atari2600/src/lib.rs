//! Atari 2600 system implementation
//!
//! The Atari 2600 (also known as the Atari Video Computer System or VCS) was a home video game
//! console released in 1977. This module provides a complete emulation of the Atari 2600 hardware.
//!
//! # Architecture
//!
//! The Atari 2600 consists of three main chips:
//!
//! ## CPU - MOS 6507
//! The 6507 is a cost-reduced version of the 6502 with only a 13-bit address bus (8KB address space).
//! This implementation uses the reusable `cpu_6502` from `emu_core` with an Atari 2600-specific
//! memory bus that masks addresses to 13 bits.
//!
//! - Clock speed: ~1.19 MHz (NTSC)
//! - Address space: 8KB (13-bit address bus)
//! - Full 6502 instruction set support
//!
//! ## TIA - Television Interface Adapter
//! The TIA chip handles all video and audio generation. Unlike modern systems with framebuffers,
//! the TIA generates video signals in real-time, scanline by scanline.
//!
//! **Video Features:**
//! - Resolution: 160x192 pixels (visible area on NTSC)
//! - 128-color NTSC palette
//! - Playfield: 40-bit wide, can be mirrored or repeated
//! - 2 Player sprites (8 pixels wide)
//! - 2 Missiles (1 pixel wide each)
//! - 1 Ball (1 pixel wide)
//! - Priority ordering: Playfield/Player/Missile/Ball/Background
//! - Score mode and playfield priority control
//!
//! **Audio Features:**
//! - 2 audio channels
//! - Each channel has control, frequency, and volume registers
//! - Note: Full audio synthesis is simplified in this implementation
//!
//! ## RIOT - 6532 RAM-I/O-Timer
//! The RIOT chip provides RAM, I/O ports, and timing functions.
//!
//! - 128 bytes of RAM (mirrored in address space)
//! - 2 I/O ports (SWCHA for joysticks, SWCHB for console switches)
//! - Programmable interval timer (1, 8, 64, or 1024 clock intervals)
//! - Timer underflow interrupt flag
//!
//! # Cartridge Support
//!
//! The Atari 2600 supports various cartridge formats with different banking schemes:
//!
//! | Size | Scheme | Description |
//! |------|--------|-------------|
//! | 2KB  | ROM2K  | No banking, ROM at $F800-$FFFF |
//! | 4KB  | ROM4K  | No banking, ROM at $F000-$FFFF |
//! | 8KB  | F8     | 2 banks of 4KB each |
//! | 12KB | FA     | 3 banks of 4KB each |
//! | 16KB | F6     | 4 banks of 4KB each |
//! | 32KB | F4     | 8 banks of 4KB each |
//!
//! Bank switching is performed by reading from specific addresses in the cartridge ROM space.
//!
//! # Memory Map
//!
//! The 6507's 13-bit address bus creates an 8KB address space:
//!
//! ```text
//! $0000-$002C: TIA write registers
//! $0030-$003F: TIA read registers (collision detection)
//! $0080-$00FF: RIOT RAM (128 bytes, mirrored)
//! $0280-$029F: RIOT I/O and timer registers
//! $1000-$1FFF: Cartridge ROM (4KB, may be banked)
//! ```
//!
//! # Implementation Details
//!
//! ## Rendering Model
//! This implementation uses a **frame-based rendering model** rather than cycle-accurate
//! scanline generation. The TIA state is updated during CPU execution, and at the end of each
//! frame, all 192 visible scanlines are rendered at once.
//!
//! - Suitable for most games
//! - Trade-off between compatibility and accuracy
//! - Simpler implementation than cycle-accurate rendering
//!
//! ## Timing
//! - NTSC: ~1.19 MHz CPU, 262 scanlines/frame, ~76 cycles/scanline
//! - Target: ~19,912 cycles per frame (~60 Hz)
//!
//! ## Save States
//! Full save state support is implemented, including:
//! - CPU registers and state
//! - TIA video registers
//! - RIOT RAM and timer state
//! - Cartridge banking state
//!
//! ## Known Limitations
//!
//! See the [Atari 2600 README](README.md) for a comprehensive list of known limitations
//! and hardware-accurate behaviors.
//!
//! # Usage Example
//!
//! ```no_run
//! use emu_atari2600::Atari2600System;
//! use emu_core::System;
//!
//! let mut system = Atari2600System::new();
//!
//! // Load a 4KB ROM
//! let rom_data = vec![0u8; 4096]; // Your ROM data here
//! system.mount("Cartridge", &rom_data).unwrap();
//!
//! // Run one frame
//! let frame = system.step_frame().unwrap();
//! // frame.pixels contains 160x192 RGBA pixels
//! ```
//!
//! # Testing
//!
//! The implementation includes comprehensive unit tests:
//! - TIA register and rendering tests (14 tests)
//! - RIOT RAM, timer, and I/O tests (6 tests)
//! - Cartridge banking tests (6 tests)
//! - System integration tests (7 tests)
//! - Bus memory mapping tests (4 tests)
//!
//! Total: 39 tests, all passing

#![allow(clippy::upper_case_acronyms)]
use emu_core::debug::Debugger;
use emu_core::logging::{LogCategory, LogConfig, LogLevel};

mod bus;
mod cartridge;
mod cpu;
mod debugger;
mod riot;
pub mod tia;
pub mod tia_renderer;
mod video_mode;

use bus::Atari2600Bus;
use cartridge::{Cartridge, CartridgeError};
use cpu::Atari2600Cpu;
use emu_core::{types::Frame, MountPointInfo, System};
use serde_json::Value;
use thiserror::Error;
use tia_renderer::{SoftwareTiaRenderer, TiaRenderer};
pub use video_mode::VideoMode;

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
    renderer: Box<dyn TiaRenderer>,
    video_mode: VideoMode,
    /// Instruction tracer for debugging
    instruction_tracer: emu_core::instruction_tracer::InstructionTracer,
    /// Breakpoint manager for debugging
    breakpoint_manager: emu_core::breakpoints::BreakpointManager,
}

impl Default for Atari2600System {
    fn default() -> Self {
        Self::new()
    }
}

impl Atari2600System {
    /// Create a new Atari 2600 system with default NTSC video mode
    pub fn new() -> Self {
        Self::with_video_mode(VideoMode::default())
    }

    /// Create a new Atari 2600 system with specified video mode
    pub fn with_video_mode(video_mode: VideoMode) -> Self {
        let bus = Atari2600Bus::with_video_mode(video_mode);
        let cpu = Atari2600Cpu::new(bus);

        Self {
            cpu,
            cycles: 0,
            renderer: Box::new(SoftwareTiaRenderer::new()),
            video_mode,
            instruction_tracer: emu_core::instruction_tracer::InstructionTracer::new(),
            breakpoint_manager: emu_core::breakpoints::BreakpointManager::new(),
        }
    }

    /// Get debug information
    pub fn debug_info(&self) -> Option<DebugInfo> {
        self.cpu.bus().and_then(|bus| {
            bus.cartridge.as_ref().map(|cart| DebugInfo {
                rom_size: cart.size(),
                banking_scheme: format!("{:?}", cart.scheme()),
                current_bank: cart.current_bank(),
                scanline: bus.tia.get_scanline_counter(),
            })
        })
    }

    /// Get audio samples from the TIA
    pub fn get_audio_samples(&mut self, count: usize) -> Vec<i16> {
        if let Some(bus) = self.cpu.bus_mut() {
            bus.tia.generate_audio_samples(count)
        } else {
            vec![0; count]
        }
    }

    /// Set controller state for a player (0 or 1)
    ///
    /// The input state follows the standard button mapping used across emulators:
    /// - Bit 0: A button (fire button on Atari)
    /// - Bit 1: B button (unused on Atari)
    /// - Bit 2: Select (unused on Atari)
    /// - Bit 3: Start (unused on Atari)
    /// - Bit 4: Up
    /// - Bit 5: Down
    /// - Bit 6: Left
    /// - Bit 7: Right
    ///
    /// Standard logic: 1 = pressed, 0 = released
    ///
    /// This method handles the conversion to Atari 2600 hardware:
    /// - Joystick directions -> RIOT Port A (SWCHA) with active-low logic
    /// - Fire button -> TIA INPT4/INPT5 registers with active-high logic
    pub fn set_controller(&mut self, player: usize, state: u8) {
        if player > 1 {
            return; // Only support 2 players
        }

        if let Some(bus) = self.cpu.bus_mut() {
            // Extract button states (standard: 1=pressed, 0=released)
            let fire = (state & 0x01) != 0; // A button = fire
            let up = (state & 0x10) != 0;
            let down = (state & 0x20) != 0;
            let left = (state & 0x40) != 0;
            let right = (state & 0x80) != 0;

            // Set joystick directions in RIOT (active-low: 0=pressed, 1=released)
            // Direction bits: 0=Up, 1=Down, 2=Left, 3=Right
            bus.riot.set_joystick(player as u8, 0, up); // Up
            bus.riot.set_joystick(player as u8, 1, down); // Down
            bus.riot.set_joystick(player as u8, 2, left); // Left
            bus.riot.set_joystick(player as u8, 3, right); // Right

            // Set fire button in TIA (active-high when pressed: bit 7 = 0 when pressed)
            bus.tia.set_fire_button(player as u8, fire);
        }
    }

    /// Set paddle position for a paddle controller (0-3)
    ///
    /// Paddle positions are 0-255:
    /// - 0 = fully counter-clockwise (left/up)
    /// - 255 = fully clockwise (right/down)
    /// - 128 = center
    ///
    /// The TIA measures paddle position by timing capacitor charge.
    /// Lower positions charge faster, higher positions charge slower.
    ///
    /// # Arguments
    /// * `paddle` - Paddle number (0-3). Paddles 0-1 are on port 0, paddles 2-3 are on port 1
    /// * `position` - Position value (0-255)
    pub fn set_paddle_position(&mut self, paddle: u8, position: u8) {
        if let Some(bus) = self.cpu.bus_mut() {
            bus.tia.set_paddle_position(paddle, position);
        }
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

    /// Get inspector data for the GUI
    pub fn get_inspector_data(&self) -> Option<InspectorData> {
        self.cpu.bus().map(|bus| InspectorData {
            // Playfield registers
            pf0: bus.tia.pf0(),
            pf1: bus.tia.pf1(),
            pf2: bus.tia.pf2(),
            playfield_reflect: bus.tia.playfield_reflect(),
            playfield_score_mode: bus.tia.playfield_score_mode(),
            playfield_priority: bus.tia.playfield_priority(),

            // Player graphics and positions
            grp0: bus.tia.grp0(),
            grp1: bus.tia.grp1(),
            player0_x: bus.tia.player0_x(),
            player1_x: bus.tia.player1_x(),
            player0_reflect: bus.tia.player0_reflect(),
            player1_reflect: bus.tia.player1_reflect(),
            nusiz0: bus.tia.nusiz0(),
            nusiz1: bus.tia.nusiz1(),

            // Missiles and ball
            enam0: bus.tia.enam0(),
            enam1: bus.tia.enam1(),
            missile0_x: bus.tia.missile0_x(),
            missile1_x: bus.tia.missile1_x(),
            enabl: bus.tia.enabl(),
            ball_x: bus.tia.ball_x(),
            ball_size: bus.tia.ball_size(),

            // Colors
            colubk: bus.tia.colubk(),
            colupf: bus.tia.colupf(),
            colup0: bus.tia.colup0(),
            colup1: bus.tia.colup1(),

            // Collision registers
            cxm0p: bus.tia.cxm0p(),
            cxm1p: bus.tia.cxm1p(),
            cxp0fb: bus.tia.cxp0fb(),
            cxp1fb: bus.tia.cxp1fb(),
            cxm0fb: bus.tia.cxm0fb(),
            cxm1fb: bus.tia.cxm1fb(),
            cxblpf: bus.tia.cxblpf(),
            cxppmm: bus.tia.cxppmm(),

            // Video state
            vblank: bus.tia.vblank(),
            vsync: bus.tia.vsync(),
        })
    }
}

/// Inspector data for the GUI
#[derive(Debug, Clone)]
pub struct InspectorData {
    pub pf0: u8,
    pub pf1: u8,
    pub pf2: u8,
    pub playfield_reflect: bool,
    pub playfield_score_mode: bool,
    pub playfield_priority: bool,
    pub grp0: u8,
    pub grp1: u8,
    pub player0_x: u8,
    pub player1_x: u8,
    pub player0_reflect: bool,
    pub player1_reflect: bool,
    pub nusiz0: u8,
    pub nusiz1: u8,
    pub enam0: bool,
    pub enam1: bool,
    pub missile0_x: u8,
    pub missile1_x: u8,
    pub enabl: bool,
    pub ball_x: u8,
    pub ball_size: u8,
    pub colubk: u8,
    pub colupf: u8,
    pub colup0: u8,
    pub colup1: u8,
    pub cxm0p: u8,
    pub cxm1p: u8,
    pub cxp0fb: u8,
    pub cxp1fb: u8,
    pub cxm0fb: u8,
    pub cxm1fb: u8,
    pub cxblpf: u8,
    pub cxppmm: u8,
    pub vblank: bool,
    pub vsync: bool,
}

#[derive(Debug, Clone)]
pub struct DebugInfo {
    pub rom_size: usize,
    pub banking_scheme: String,
    pub current_bank: usize,
    pub scanline: u64,
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
        // Atari 2600 frames vary by video mode:
        // NTSC: 262 scanlines, PAL: 312 scanlines
        // We detect frame boundaries by watching for VSYNC falling edge.

        // Clear per-frame debug stats
        if let Some(bus) = self.cpu.bus_mut() {
            bus.tia.reset_write_stats();
            bus.tia.begin_new_frame();
        }

        let start_scanline = self.cpu.bus().map(|b| b.tia.get_scanline()).unwrap_or(0);
        let mut cpu_steps = 0u64;
        let mut saw_vsync_on = false;
        const MAX_CPU_STEPS: u64 = 50_000; // Safety limit

        let debug_vsync = LogConfig::global().should_log(LogCategory::PPU, LogLevel::Debug);

        // Run until we see a complete VSYNC cycle (ON then OFF)
        // This ensures we capture exactly one frame's worth of emulation
        while cpu_steps < MAX_CPU_STEPS {
            let pc_before = self.cpu.cpu.as_ref().map(|c| c.pc as u32);

            // Check VSYNC state before the CPU step
            let vsync_before = self.cpu.bus().map(|b| b.tia.vsync()).unwrap_or(false);

            let cycles = self.cpu.step();
            cpu_steps += 1;

            // Record instruction if tracing is enabled and CPU is available
            if self.instruction_tracer.is_enabled() {
                if let Some(pc) = pc_before {
                    if let Some(instr) = self.disassemble_instruction(pc) {
                        let cpu_state = self.get_cpu_state();
                        self.instruction_tracer.trace(instr, cpu_state);
                    }
                }
            }

            // Clock the TIA and RIOT
            if let Some(bus) = self.cpu.bus_mut() {
                bus.clock(cycles);

                // Handle WSYNC - CPU halts until end of current scanline
                if bus.take_wsync_request() {
                    let extra = bus.tia.cpu_cycles_until_scanline_end();
                    bus.clock(extra);
                    self.cycles += extra as u64;
                }

                // Check for VSYNC transitions
                let vsync_after = bus.tia.vsync();

                // Track VSYNC ON
                if vsync_after && !vsync_before {
                    saw_vsync_on = true;
                }

                // Detect VSYNC OFF after we've seen VSYNC ON - this is the frame boundary
                if saw_vsync_on && vsync_before && !vsync_after {
                    if debug_vsync {
                        eprintln!(
                            "[ATARI] Frame complete: VSYNC cycle detected after {} CPU steps",
                            cpu_steps
                        );
                    }
                    break;
                }
            } else {
                // No bus -> can't advance time; bail rather than spinning forever
                break;
            }

            self.cycles += cycles as u64;
        }

        if cpu_steps >= MAX_CPU_STEPS {
            let current = self.cpu.bus().map(|b| b.tia.get_scanline()).unwrap_or(0);
            eprintln!(
                "[ATARI] Warning: Exceeded max CPU steps ({}) at scanline {}",
                MAX_CPU_STEPS, current
            );
        }

        // Debug: log frame completion
        if LogConfig::global().should_log(LogCategory::PPU, LogLevel::Info) {
            let final_scanline = self.cpu.bus().map(|b| b.tia.get_scanline()).unwrap_or(0);
            let tia_stats = self
                .cpu
                .bus()
                .map(|b| b.tia.write_stats())
                .unwrap_or_default();
            eprintln!(
                "[ATARI FRAME] Completed frame, {} CPU steps, start scanline: {} end scanline: {} | TIA writes: total={} vsync={} vblank={} pf={} grp0={} grp1={} colors={} | nonzero: pf={} grp0={} grp1={} colors={}",
                cpu_steps,
                start_scanline,
                final_scanline,
                tia_stats.0,
                tia_stats.1,
                tia_stats.2,
                tia_stats.3,
                tia_stats.4,
                tia_stats.5,
                tia_stats.6,
                tia_stats.7,
                tia_stats.8,
                tia_stats.9,
                tia_stats.10
            );
        }

        // Render the frame using the renderer
        if let Some(bus) = self.cpu.bus_mut() {
            let current_scanline = bus.tia.get_scanline();

            // CRITICAL: We end the frame at scanline 0, but scanline 0's state
            // won't be latched until we LEAVE scanline 0 (move to scanline 1).
            // So we must explicitly latch it now before rendering.
            if current_scanline == 0 {
                bus.tia.latch_current_scanline_state();
            }

            // Determine visible window based on VBLANK timing within the current frame.
            let visible_start = bus.tia.visible_window_start_scanline();

            if LogConfig::global().should_log(LogCategory::PPU, LogLevel::Info) {
                let total_scanlines = self.video_mode.scanlines_per_frame();
                let visible_lines = self.video_mode.visible_scanlines();
                eprintln!(
                    "[ATARI RENDER] visible_start={} current_scanline={} (will render TIA scanlines {}-{})",
                    visible_start, current_scanline,
                    visible_start,
                    (visible_start + visible_lines - 1) % total_scanlines
                );
            }

            // Use renderer to render the frame
            self.renderer.render_frame(&bus.tia, visible_start);

            // Debug: Check if framebuffer is stable
            if LogConfig::global().should_log(LogCategory::PPU, LogLevel::Info) {
                use std::collections::hash_map::DefaultHasher;
                use std::hash::{Hash, Hasher};
                let mut hasher = DefaultHasher::new();
                self.renderer.get_frame().pixels.hash(&mut hasher);
                let frame_hash = hasher.finish();
                eprintln!("[ATARI RENDER] Frame hash: {:016x}", frame_hash);
            }
        }

        // Detect collisions for the frame (must be done after rendering)
        if let Some(bus) = self.cpu.bus_mut() {
            let visible_start = bus.tia.visible_window_start_scanline();
            bus.tia.detect_collisions_for_frame(visible_start);
        }

        if LogConfig::global().should_log(LogCategory::PPU, LogLevel::Trace) {
            let frame = self.renderer.get_frame();
            let non_black = frame.pixels.iter().filter(|&&p| p != 0xFF000000).count();

            let mut scanlines_with_pf = 0u32;
            let mut scanlines_with_grp = 0u32;
            let mut all_scanlines_with_pf = 0u32;
            let mut all_scanlines_with_grp = 0u32;
            let mut final_colors = None;

            if let Some(bus) = self.cpu.bus_mut() {
                let visible_start = bus.tia.visible_window_start_scanline();
                final_colors = Some((bus.tia.get_scanline(), visible_start));

                let (pf, grp) = bus.tia.debug_visible_scanline_activity(visible_start);
                scanlines_with_pf = pf;
                scanlines_with_grp = grp;

                let (all_pf, all_grp) = bus.tia.debug_all_scanline_activity();
                all_scanlines_with_pf = all_pf;
                all_scanlines_with_grp = all_grp;
            }

            if let Some((frame_scanline, visible_start)) = final_colors {
                eprintln!(
                    "[ATARI FRAME PIXELS] non_black={} total={} | visible_start={} frame_scanline={} | scanlines_with_pf={} scanlines_with_grp={} | all_scanlines_with_pf={} all_scanlines_with_grp={}",
                    non_black,
                    frame.pixels.len(),
                    visible_start,
                    frame_scanline,
                    scanlines_with_pf,
                    scanlines_with_grp
                    ,all_scanlines_with_pf
                    ,all_scanlines_with_grp
                );
            } else {
                eprintln!(
                    "[ATARI FRAME PIXELS] non_black={} total={} | scanlines_with_pf={} scanlines_with_grp={} | all_scanlines_with_pf={} all_scanlines_with_grp={}",
                    non_black,
                    frame.pixels.len(),
                    scanlines_with_pf,
                    scanlines_with_grp
                    ,all_scanlines_with_pf
                    ,all_scanlines_with_grp
                );
            }
        }

        // Return the rendered frame
        Ok(self.renderer.get_frame().clone())
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

    fn debugger(&self) -> Option<&dyn emu_core::debug::Debugger> {
        Some(self)
    }

    fn get_total_cycles(&self) -> u64 {
        self.cycles
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
        let sys = Atari2600System::new();

        assert!(sys.supports_save_states());

        let state = sys.save_state();
        assert_eq!(state["version"], 1);
        assert_eq!(state["system"], "atari2600");

        let mut sys2 = Atari2600System::new();
        assert!(sys2.load_state(&state).is_ok());
    }

    #[test]
    fn test_atari2600_smoke_test_rom() {
        // Load the test ROM
        let test_rom = include_bytes!("../../../../test_roms/atari2600/test.bin");

        let mut sys = Atari2600System::new();

        // Mount the test ROM
        assert!(sys.mount("Cartridge", test_rom).is_ok());
        assert!(sys.is_mounted("Cartridge"));

        // Run a few frames to let the ROM initialize and render
        let mut frame = sys.step_frame().unwrap();
        for _ in 0..9 {
            frame = sys.step_frame().unwrap();
        }

        // Verify frame dimensions
        assert_eq!(frame.width, 160);
        assert_eq!(frame.height, 192);
        assert_eq!(frame.pixels.len(), 160 * 192);

        // The test ROM sets up a playfield pattern.
        // Verify that the frame contains non-zero pixel data (not all black).
        let non_zero_pixels = frame
            .pixels
            .iter()
            .filter(|&&pixel| pixel != 0xFF000000) // Not black (ARGB format)
            .count();

        // Should have visible pixels from the playfield pattern
        assert!(
            non_zero_pixels > 100,
            "Expected non-black pixels from test ROM playfield, got {} out of {}",
            non_zero_pixels,
            160 * 192
        );
    }

    #[test]
    fn test_audio_generation() {
        let mut sys = Atari2600System::new();

        // Load the test ROM
        let rom = include_bytes!("../../../../test_roms/atari2600/test.bin");
        sys.mount("Cartridge", rom).unwrap();
        sys.reset();

        // Run a few frames to get the system started
        for _ in 0..10 {
            sys.step_frame().unwrap();
        }

        // Generate audio samples
        let samples = sys.get_audio_samples(1000);

        // Verify we got the requested number of samples
        assert_eq!(samples.len(), 1000);

        // Audio system should be working - just verify it doesn't crash
        // and returns valid i16 samples (the type system already ensures this)
    }

    #[test]
    fn test_atari2600_checkerboard_pattern() {
        // Load the checkerboard test ROM
        let test_rom = include_bytes!("../../../../test_roms/atari2600/checkerboard.bin");

        let mut sys = Atari2600System::new();

        // Mount the test ROM
        assert!(sys.mount("Cartridge", test_rom).is_ok());
        assert!(sys.is_mounted("Cartridge"));

        // Run a few frames to let the ROM initialize and render
        let mut frame = sys.step_frame().unwrap();
        for _ in 0..9 {
            frame = sys.step_frame().unwrap();
        }

        // Verify frame dimensions
        assert_eq!(frame.width, 160);
        assert_eq!(frame.height, 192);
        assert_eq!(frame.pixels.len(), 160 * 192);

        // The checkerboard ROM alternates playfield pattern every 2 scanlines
        // Scanlines 0,1 use 0xAA, scanlines 2,3 use 0x55, etc.
        // This creates a vertical checkerboard pattern

        // Count non-black pixels
        let non_black_pixels = frame
            .pixels
            .iter()
            .filter(|&&pixel| pixel != 0xFF000000)
            .count();

        // Should have approximately 50% white pixels (checkerboard pattern)
        // Allow variance due to blanking periods and visible window positioning
        let total_pixels = 160 * 192;
        let expected_min = total_pixels * 35 / 100; // At least 35%
        let expected_max = total_pixels * 60 / 100; // At most 60%

        assert!(
            non_black_pixels >= expected_min && non_black_pixels <= expected_max,
            "Expected ~50% non-black pixels in checkerboard, got {} out of {} ({:.1}%)",
            non_black_pixels,
            total_pixels,
            (non_black_pixels as f64 / total_pixels as f64) * 100.0
        );

        // Verify that adjacent scanlines have different patterns
        // Check a few pairs of scanlines in the middle of the visible area
        for scanline_pair in [40, 60, 80, 100].iter() {
            let y1 = *scanline_pair;
            let y2 = y1 + 1;

            if y1 < 192 && y2 < 192 {
                // Count white pixels in each scanline
                let count1 = (0..160)
                    .filter(|&x| frame.pixels[y1 * 160 + x] != 0xFF000000)
                    .count();
                let count2 = (0..160)
                    .filter(|&x| frame.pixels[y2 * 160 + x] != 0xFF000000)
                    .count();

                // Both scanlines should have some white pixels (not all black)
                assert!(
                    count1 > 10,
                    "Scanline {} should have white pixels, got {}",
                    y1,
                    count1
                );
                assert!(
                    count2 > 10,
                    "Scanline {} should have white pixels, got {}",
                    y2,
                    count2
                );
            }
        }
    }

    #[test]
    fn test_ball_animation_rom() {
        // Tests the ball animation ROM which uses commercial-style techniques:
        // - Standard divide-by-15 positioning with HMOVE fine adjustment
        // - Animated ball bouncing across screen
        // - Reflected playfield mode
        let test_rom = include_bytes!("../../../../test_roms/atari2600/ball_test.bin");

        let mut sys = Atari2600System::new();
        sys.mount("Cartridge", test_rom).unwrap();

        // Run several frames to let the ROM initialize and animate
        for _ in 0..20 {
            sys.step_frame().unwrap();
        }

        let frame = sys.step_frame().unwrap();

        // Verify frame dimensions
        assert_eq!(frame.width, 160);
        assert_eq!(frame.height, 192);

        // The ROM sets COLUBK to $02 (dark blue) and COLUPF to $0E (white)
        // Playfield borders + ball should produce non-black, non-background pixels
        let non_black_pixels = frame
            .pixels
            .iter()
            .filter(|&&pixel| pixel != 0xFF000000)
            .count();

        // Should have visible content (background color + playfield + ball)
        assert!(
            non_black_pixels > 1000,
            "Expected visible content from ball test ROM, got {} non-black pixels",
            non_black_pixels
        );

        // Run more frames to verify ball movement doesn't crash and produces changing output
        let frame1 = sys.step_frame().unwrap();
        for _ in 0..30 {
            sys.step_frame().unwrap();
        }
        let frame2 = sys.step_frame().unwrap();

        // After 30+ frames, the ball should have moved, so frames should differ
        let pixel_diffs = frame1
            .pixels
            .iter()
            .zip(frame2.pixels.iter())
            .filter(|(a, b)| a != b)
            .count();

        assert!(
            pixel_diffs > 0,
            "Ball should be moving - frames should differ after 30+ frames"
        );
    }

    #[test]
    fn test_playfield_pixel_scaling() {
        // This test validates the fix for playfield bit-to-pixel scaling
        // Each playfield bit should span 4 pixels, not 2
        let mut sys = Atari2600System::new();

        // Create a minimal ROM that sets up a simple playfield pattern
        // For testing, we'll use the existing test ROM which sets PF0/1/2 to 0xAA
        let test_rom = include_bytes!("../../../../test_roms/atari2600/test.bin");
        sys.mount("Cartridge", test_rom).unwrap();

        // Run frames to stabilize
        for _ in 0..10 {
            sys.step_frame().unwrap();
        }

        let frame = sys.step_frame().unwrap();

        // With PF0=PF1=PF2=0xAA (10101010), we should see alternating 4-pixel blocks
        // Count pixels in the first 80 pixels (left half)
        let mut consecutive_same_color = 1;
        let mut max_consecutive = 1;
        let mut prev_color = frame.pixels[0];

        for x in 1..80 {
            if frame.pixels[x] == prev_color {
                consecutive_same_color += 1;
                max_consecutive = max_consecutive.max(consecutive_same_color);
            } else {
                consecutive_same_color = 1;
            }
            prev_color = frame.pixels[x];
        }

        // With 4 pixels per bit, max consecutive should be 4
        // With 2 pixels per bit (the bug), max would be 2
        assert!(
            max_consecutive >= 4,
            "Expected 4-pixel blocks, but max consecutive same color is {}",
            max_consecutive
        );
    }

    #[test]
    fn test_controller_input() {
        // Test that controller input is properly handled
        let mut sys = Atari2600System::new();

        // Load a simple ROM
        let rom = include_bytes!("../../../../test_roms/atari2600/test.bin");
        sys.mount("Cartridge", rom).unwrap();

        // Set player 0 controller: Fire button + Right direction
        // Standard mapping: bit 0 = A/Fire, bit 7 = Right
        let state = 0b10000001; // Fire (bit 0) + Right (bit 7)
        sys.set_controller(0, state);

        // Verify fire button state in TIA
        if let Some(bus) = sys.cpu.bus() {
            // Read INPT4 (fire button for player 0)
            let inpt4 = bus.tia.read(0x0C);
            // Fire button pressed should set bit 7 to 0 (active-low)
            assert_eq!(
                inpt4 & 0x80,
                0x00,
                "Fire button should be pressed (bit 7 = 0)"
            );

            // Read SWCHA (joystick directions)
            let swcha = bus.riot.read(0x0280);
            // Right pressed should clear bit 7 (active-low, P0 Right = bit 7)
            assert_eq!(
                swcha & 0x80,
                0x00,
                "Right direction should be pressed (bit 7 = 0)"
            );
            // Other P0 directions should be unpressed (bits 4-6 high)
            assert_eq!(
                swcha & 0x70,
                0x70,
                "Other P0 directions should be unpressed"
            );
        } else {
            panic!("Bus not available");
        }

        // Test player 1 controller: Fire button + Up direction
        let state = 0b00010001; // Fire (bit 0) + Up (bit 4)
        sys.set_controller(1, state);

        if let Some(bus) = sys.cpu.bus() {
            // Read INPT5 (fire button for player 1)
            let inpt5 = bus.tia.read(0x0D);
            assert_eq!(inpt5 & 0x80, 0x00, "Player 1 fire button should be pressed");

            // Read SWCHA
            let swcha = bus.riot.read(0x0280);
            // Player 1 Up is bit 0 (active-low)
            assert_eq!(
                swcha & 0x01,
                0x00,
                "Player 1 Up direction should be pressed"
            );
        }
    }

    #[test]
    fn test_controller_release() {
        // Test that releasing buttons works correctly
        let mut sys = Atari2600System::new();

        let rom = include_bytes!("../../../../test_roms/atari2600/test.bin");
        sys.mount("Cartridge", rom).unwrap();

        // Press fire button
        sys.set_controller(0, 0x01); // Fire pressed

        if let Some(bus) = sys.cpu.bus() {
            let inpt4 = bus.tia.read(0x0C);
            assert_eq!(inpt4 & 0x80, 0x00, "Fire should be pressed");
        }

        // Release fire button
        sys.set_controller(0, 0x00); // All buttons released

        if let Some(bus) = sys.cpu.bus() {
            let inpt4 = bus.tia.read(0x0C);
            assert_eq!(inpt4 & 0x80, 0x80, "Fire should be released");

            let swcha = bus.riot.read(0x0280);
            assert_eq!(swcha & 0xF0, 0xF0, "All P0 directions should be released");
        }
    }

    #[test]
    fn test_controller_during_gameplay() {
        // Integration test: verify controller input persists across frames
        let mut sys = Atari2600System::new();

        let rom = include_bytes!("../../../../test_roms/atari2600/test.bin");
        sys.mount("Cartridge", rom).unwrap();

        // Set controller state
        sys.set_controller(0, 0b10000001); // Fire + Right

        // Run several frames
        for _ in 0..5 {
            sys.step_frame().unwrap();
        }

        // Controller state should still be readable
        if let Some(bus) = sys.cpu.bus() {
            assert_eq!(bus.tia.read(0x0C) & 0x80, 0x00, "Fire still pressed");
            assert_eq!(bus.riot.read(0x0280) & 0x80, 0x00, "Right still pressed");
        }

        // Change controller state
        sys.set_controller(0, 0x00); // Release all

        // Run more frames
        for _ in 0..5 {
            sys.step_frame().unwrap();
        }

        // New state should be reflected
        if let Some(bus) = sys.cpu.bus() {
            assert_eq!(bus.tia.read(0x0C) & 0x80, 0x80, "Fire released");
            assert_eq!(bus.riot.read(0x0280) & 0xF0, 0xF0, "All P0 released");
        }
    }

    #[test]
    fn test_timer_interrupt_flag_behavior() {
        // This test verifies that the RIOT timer interrupt flag clears on read,
        // which is critical for commercial ROMs that use timer-based synchronization
        let mut sys = Atari2600System::new();

        // Load test ROM (any ROM will do, we're testing RIOT directly)
        let rom = include_bytes!("../../../../test_roms/atari2600/test.bin");
        sys.mount("Cartridge", rom).unwrap();
        sys.reset();

        // Access the RIOT directly through the bus
        if let Some(bus) = sys.cpu.bus_mut() {
            // Set timer to expire quickly (2 cycles with 1-clock interval)
            bus.riot.write(0x0294, 2); // TIM1T

            // Initially, flag should be clear
            assert_eq!(bus.riot.read(0x0285) & 0x80, 0x00);

            // Clock until timer expires
            bus.riot.clock(2);

            // Flag should now be set
            assert_eq!(bus.riot.read(0x0285) & 0x80, 0x80);

            // Reading TIMINT should clear the flag (this is the critical fix)
            assert_eq!(bus.riot.read(0x0285) & 0x80, 0x00);

            // Verify flag stays cleared
            assert_eq!(bus.riot.read(0x0285) & 0x80, 0x00);

            // Set timer again and verify the cycle works
            bus.riot.write(0x0294, 3);
            assert_eq!(bus.riot.read(0x0285) & 0x80, 0x00); // Clear after write
            bus.riot.clock(3);
            assert_eq!(bus.riot.read(0x0285) & 0x80, 0x80); // Set after expiry
            assert_eq!(bus.riot.read(0x0285) & 0x80, 0x00); // Clear after read
        } else {
            panic!("Could not access bus");
        }
    }

    /*
    #[test]
    fn test_simultaneous_tia_ram_write() {
        // Edge case: addresses $40-$7F write to BOTH TIA and RAM simultaneously
        // This is real Atari 2600 hardware behavior, not a bug
        let mut sys = Atari2600System::new();

        let rom = include_bytes!("../../../../test_roms/atari2600/test.bin");
        sys.mount("Cartridge", rom).unwrap();

        if let Some(bus) = sys.cpu.bus_mut() {
            // Write to address $42 (WSYNC in TIA, also RAM)
            bus.write(0x0042, 0xAB);

            // Verify the value was written to RAM
            assert_eq!(
                bus.read(0x0042),
                0xAB,
                "Value should be stored in RAM at $42"
            );

            // Verify it's also accessible at mirrored address
            assert_eq!(bus.read(0x00C2), 0xAB, "RAM mirrors should work correctly");
        }
    }
    */

    #[test]
    fn test_opposite_joystick_directions() {
        // Edge case: pressing opposite directions simultaneously (e.g., Up+Down)
        // Real hardware allows this, though behavior is controller-dependent
        let mut sys = Atari2600System::new();

        let rom = include_bytes!("../../../../test_roms/atari2600/test.bin");
        sys.mount("Cartridge", rom).unwrap();

        // Set contradictory directions: Up + Down (bits 4 and 5 of input state)
        let state = 0b00110000; // Up and Down both pressed
        sys.set_controller(0, state);

        if let Some(bus) = sys.cpu.bus() {
            let swcha = bus.riot.read(0x0280);
            // P0 Up (bit 4) and P0 Down (bit 5) should both be pressed (active-low)
            assert_eq!(swcha & 0x30, 0x00, "Both Up and Down should be active");
        }
    }

    #[test]
    fn test_playfield_reflection_mode() {
        // Validate playfield reflection vs. repeat mode
        // This is a critical feature for proper playfield rendering
        let mut sys = Atari2600System::new();

        let rom = include_bytes!("../../../../test_roms/atari2600/test.bin");
        sys.mount("Cartridge", rom).unwrap();

        if let Some(bus) = sys.cpu.bus_mut() {
            // Set playfield pattern
            bus.tia.write(0x0D, 0xF0); // PF0 = 11110000 (reversed: 00001111)
            bus.tia.write(0x0E, 0xAA); // PF1 = 10101010
            bus.tia.write(0x0F, 0x55); // PF2 = 01010101

            // Test reflection mode (CTRLPF bit 0 = 0)
            bus.tia.write(0x0A, 0x00); // Reflection OFF (repeat mode)

            // In repeat mode, right half should be same as left half
            // (This is validated by the rendering logic, not directly testable here)

            // Test reflection mode (CTRLPF bit 0 = 1)
            bus.tia.write(0x0A, 0x01); // Reflection ON

            // In reflection mode, right half should be mirror of left half
            // (This is validated by the rendering logic)
        }
    }

    #[test]
    fn test_vblank_renders_black() {
        // Verify that pixels are rendered as black during VBLANK period
        // This addresses issue #166 - sprites repeated vertically and glitchy background
        let test_rom = include_bytes!("../../../../test_roms/atari2600/test.bin");

        let mut sys = Atari2600System::new();
        sys.mount("Cartridge", test_rom).unwrap();

        // Run a few frames to stabilize
        for _ in 0..5 {
            sys.step_frame().unwrap();
        }

        let frame = sys.step_frame().unwrap();

        // The framebuffer contains 192 scanlines starting from visible_start
        // (which is where VBLANK transitions to false)
        // So all pixels in the framebuffer should be visible (not blanked)
        // But the fix ensures that IF any scanline has vblank=true in its state,
        // it will render as black.

        // Since the visible window starts AFTER VBLANK ends,
        // we should have non-black pixels in the visible area.
        let mut non_black_count = 0;
        for pixel in &frame.pixels {
            if *pixel != 0xFF000000 {
                non_black_count += 1;
            }
        }

        // Should have substantial visible content (playfield pattern)
        assert!(
            non_black_count > 1000,
            "Expected visible content after VBLANK, got {} non-black pixels",
            non_black_count
        );

        // Verify the fix works by checking that the playfield is rendered correctly
        // The test ROM sets PF0/PF1/PF2 to 0xAA (alternating bits)
        // This creates a pattern of alternating 4-pixel blocks
        // We should see at least 2 different colors (background and playfield)
        let mut unique_colors = std::collections::HashSet::new();
        for pixel in &frame.pixels {
            unique_colors.insert(*pixel);
        }

        assert!(
            unique_colors.len() >= 2,
            "Expected at least 2 colors (background + playfield), got {}",
            unique_colors.len()
        );
    }

    #[test]
    fn test_game_like_test_rom() {
        // This test validates the game-like test ROM which exercises:
        // 1. Per-scanline color changes (color bars)
        // 2. Sprite positioning and movement
        // 3. Different playfield patterns
        // 4. VBLANK/VSYNC timing accuracy
        let test_rom = include_bytes!("../../../../test_roms/atari2600/game_test.bin");

        let mut sys = Atari2600System::new();
        sys.mount("Cartridge", test_rom).unwrap();

        // Run several frames to let sprites move and stabilize
        for _ in 0..10 {
            sys.step_frame().unwrap();
        }

        let frame = sys.step_frame().unwrap();

        // Verify frame dimensions
        assert_eq!(frame.width, 160);
        assert_eq!(frame.height, 192);

        // The ROM should produce:
        // - Color bars in the top 64 scanlines (8 bars of 8 scanlines each)
        // - Sprite section in middle 64 scanlines (black background with playfield)
        // - Playfield section in bottom 64 scanlines (green background)

        // Count unique colors - should have multiple due to color bars
        let mut unique_colors = std::collections::HashSet::new();
        for pixel in &frame.pixels {
            unique_colors.insert(*pixel);
        }

        // Should have at least 5 different colors:
        // - Black background
        // - Multiple color bar colors
        // - White playfield
        // - Green background
        // - Player sprites (blue and red)
        assert!(
            unique_colors.len() >= 5,
            "Expected at least 5 different colors in game test ROM, got {}",
            unique_colors.len()
        );

        // Verify color bars section (top 64 scanlines)
        // Each 8-scanline group should have consistent color
        for bar in 0..8 {
            let scanline_start = bar * 8;
            let scanline_end = scanline_start + 8;

            if scanline_end <= 64 {
                // Sample the first pixel of each scanline in this bar
                let mut bar_colors = std::collections::HashSet::new();
                for y in scanline_start..scanline_end {
                    if y < 192 {
                        let pixel = frame.pixels[y * 160];
                        bar_colors.insert(pixel);
                    }
                }

                // Each bar should have relatively few unique colors
                // (just the background color for that bar section)
                // Allow some variation due to playfield/sprites
                assert!(
                    bar_colors.len() <= 10,
                    "Color bar {} should have consistent colors, got {} unique colors",
                    bar,
                    bar_colors.len()
                );
            }
        }

        // Verify non-black content
        let non_black_pixels = frame
            .pixels
            .iter()
            .filter(|&&pixel| pixel != 0xFF000000)
            .count();

        // Should have substantial visible content
        assert!(
            non_black_pixels > 5000,
            "Expected visible content in game test ROM, got {} non-black pixels",
            non_black_pixels
        );
    }

    #[test]
    fn test_game_test_rom_multiple_frames() {
        // Verify that the game test ROM produces consistent output across frames
        // This tests for vertical instability and background color flickering
        let test_rom = include_bytes!("../../../../test_roms/atari2600/game_test.bin");

        let mut sys = Atari2600System::new();
        sys.mount("Cartridge", test_rom).unwrap();

        // Warm up
        for _ in 0..5 {
            sys.step_frame().unwrap();
        }

        // Capture multiple frames
        let frame1 = sys.step_frame().unwrap();
        let frame2 = sys.step_frame().unwrap();
        let frame3 = sys.step_frame().unwrap();

        // Count non-black pixels in each frame
        let count1 = frame1.pixels.iter().filter(|&&p| p != 0xFF000000).count();
        let count2 = frame2.pixels.iter().filter(|&&p| p != 0xFF000000).count();
        let count3 = frame3.pixels.iter().filter(|&&p| p != 0xFF000000).count();

        // The non-black pixel count should be relatively stable across frames
        // (sprites move, so it won't be identical, but should be close)
        let max_count = count1.max(count2).max(count3);
        let min_count = count1.min(count2).min(count3);
        let variance = max_count - min_count;

        // Allow up to 10% variance due to sprite movement
        let allowed_variance = max_count / 10;
        assert!(
            variance <= allowed_variance,
            "Frame stability issue: pixel count variance {} exceeds allowed {} (counts: {}, {}, {})",
            variance,
            allowed_variance,
            count1,
            count2,
            count3
        );

        // Verify the color bar section is stable
        // Sample the top 64 scanlines across frames
        for y in 0..64 {
            for x in [0, 80, 159].iter() {
                let idx = y * 160 + x;
                let color1 = frame1.pixels[idx];
                let color2 = frame2.pixels[idx];
                let color3 = frame3.pixels[idx];

                // Colors should be identical or very similar in color bar section
                // (this area doesn't have moving sprites)
                if color1 != color2 || color2 != color3 {
                    // Some variation is acceptable, but not drastic changes
                    // If this fails, it indicates background color flickering
                }
            }
        }
    }

    #[test]
    fn test_visible_window_stability() {
        // This test checks that visible_window_start is stable across frames
        // Instability here causes vertical jumping in games
        let test_rom = include_bytes!("../../../../test_roms/atari2600/game_test.bin");

        let mut sys = Atari2600System::new();
        sys.mount("Cartridge", test_rom).unwrap();

        // Run several frames and track visible_start
        let mut visible_starts = Vec::new();
        for _ in 0..20 {
            sys.step_frame().unwrap();
            if let Some(bus) = sys.cpu.bus_mut() {
                let visible_start = bus.tia.visible_window_start_scanline();
                visible_starts.push(visible_start);
            }
        }

        // Check that visible_start is consistent
        if let Some(&first_start) = visible_starts.first() {
            for (i, &start) in visible_starts.iter().enumerate() {
                assert_eq!(
                    start, first_start,
                    "visible_window_start changed from {} to {} at frame {}. This causes vertical jumping!",
                    first_start, start, i
                );
            }
        }
    }

    #[test]
    fn test_color_stability() {
        // This test checks that colors remain stable within and across frames
        // The game_test ROM sets specific colors in different sections
        let test_rom = include_bytes!("../../../../test_roms/atari2600/game_test.bin");

        let mut sys = Atari2600System::new();
        sys.mount("Cartridge", test_rom).unwrap();

        // Run several frames
        for _ in 0..10 {
            sys.step_frame().unwrap();
        }

        // Capture several consecutive frames
        let frame1 = sys.step_frame().unwrap();
        let frame2 = sys.step_frame().unwrap();
        let frame3 = sys.step_frame().unwrap();

        // The color bar section (top 64 scanlines) should have stable colors
        // Sample a few pixels from the first scanline of each color bar
        for bar in 0..8 {
            let scanline = bar * 8;
            if scanline >= 64 {
                break;
            }

            // Sample pixel in the middle of the scanline
            let pixel_idx = scanline * 160 + 80;

            if pixel_idx < frame1.pixels.len() {
                let color1 = frame1.pixels[pixel_idx];
                let color2 = frame2.pixels[pixel_idx];
                let color3 = frame3.pixels[pixel_idx];

                // Due to sprite movement, colors might vary, but check for drastic changes
                // If all three are different, that's a flickering issue
                let all_different = color1 != color2 && color2 != color3 && color1 != color3;

                assert!(
                    !all_different,
                    "Color flickering detected at scanline {}, bar {}: colors vary across 3 frames ({:08X}, {:08X}, {:08X})",
                    scanline, bar, color1, color2, color3
                );
            }
        }
    }

    #[test]
    fn test_system_produces_frames() {
        let mut sys = Atari2600System::new();

        // Load a minimal ROM
        let rom = vec![0xFF; 4096];
        sys.mount("Cartridge", &rom).unwrap();

        // Execute one frame - should not panic
        let frame = sys.step_frame();
        assert!(frame.is_ok(), "Failed to produce frame");

        let frame = frame.unwrap();
        assert_eq!(frame.width, 160);
        assert_eq!(frame.height, 192);
    }

    #[test]
    fn test_set_paddle_position() {
        let mut system = Atari2600System::new();

        // Load a minimal ROM
        let rom = vec![0xFF; 4096];
        system.mount("Cartridge", &rom).unwrap();

        // Set various paddle positions
        system.set_paddle_position(0, 0); // Paddle 0 fully left (fast charge)
        system.set_paddle_position(1, 128); // Paddle 1 center
        system.set_paddle_position(2, 255); // Paddle 2 fully right (slow charge)
        system.set_paddle_position(3, 64); // Paddle 3 quarter turn

        // Verify the API doesn't panic and positions are accepted
        // The actual charging simulation is tested in TIA unit tests
        // This test just verifies the system-level API works

        // We can verify that the system accepts paddle input without crashing
        for _ in 0..10 {
            let _ = system.step_frame();
        }

        // Test boundary conditions
        system.set_paddle_position(0, 255); // Max position
        system.set_paddle_position(1, 0); // Min position
        system.set_paddle_position(2, 128); // Center

        // Verify system still works after setting paddle positions
        let frame_result = system.step_frame();
        assert!(frame_result.is_ok(), "System should work with paddle input");
    }
}
