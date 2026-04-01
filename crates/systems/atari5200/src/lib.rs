//! Atari 5200 SuperSystem emulation
//!
//! The Atari 5200 (1982) is a home video game console based on the Atari 400/800 computer
//! architecture. This module provides emulation of the Atari 5200 hardware.
//!
//! # Architecture
//!
//! ## CPU - MOS 6502C
//! A CMOS variant of the 6502 running at 1.79 MHz (NTSC).
//! Uses the reusable `cpu_6502` from `emu_core`.
//!
//! ## ANTIC - Alphanumeric Television Interface Controller
//! A DMA-driven display list processor that generates the playfield display.
//! - Reads a "display list" program from memory
//! - Multiple character and bitmap modes
//! - Supports horizontal/vertical scrolling
//! - Generates DLI/VBI interrupts
//! - Resolution: up to 320×192 (hi-res) or 160×192 (standard)
//!
//! ## GTIA - George's Television Interface Adapter
//! Handles color generation, player-missile graphics, and collision detection.
//! - 9 color registers (4 player, 4 playfield, 1 background)
//! - 4 player sprites (8 pixels wide, full-screen tall)
//! - 4 missiles (2 pixels wide)
//! - 128-color NTSC palette (hue × luminance)
//! - Hardware collision detection
//!
//! ## POKEY - POtentiometer and KEYboard IC
//! Handles sound, input, timers, and serial I/O.
//! - 4-channel audio (square waves with distortion)
//! - Paddle/joystick analog input
//! - Random number generation
//! - Programmable timers
//!
//! # Memory Map
//!
//! ```text
//! $0000-$3FFF: 16KB RAM
//! $4000-$7FFF: Unused (open bus)
//! $8000-$BFFF: Cartridge ROM (8KB, 16KB, or 32KB banked)
//! $C000-$C0FF: GTIA registers
//! $D400-$D4FF: ANTIC registers
//! $E800-$E8FF: POKEY registers
//! $F800-$FFFF: Built-in BIOS ROM (2KB)
//! ```
//!
//! # Cartridge Support
//!
//! | Size | Banking | Description |
//! |------|---------|-------------|
//! | 8KB  | None    | ROM at $8000-$9FFF, mirrored |
//! | 16KB | None    | ROM at $8000-$BFFF |
//! | 32KB | F8-type | Two 16KB banks, switched via $BFE0-$BFEF |
//!
//! # Usage Example
//!
//! ```no_run
//! use emu_atari5200::Atari5200System;
//! use emu_core::System;
//!
//! let mut system = Atari5200System::new();
//! let rom_data = vec![0u8; 16384]; // Your ROM data here
//! system.mount("Cartridge", &rom_data).unwrap();
//! let frame = system.step_frame().unwrap();
//! // frame.pixels contains 320×192 RGBA pixels
//! ```

#![allow(clippy::upper_case_acronyms)]

pub mod antic;
mod bus;
mod cartridge;
mod cpu;
pub mod gtia;
pub mod pokey;

use antic::{Antic, AnticMode};
use bus::Atari5200Bus;
use cartridge::{Cartridge, CartridgeError};
use cpu::Atari5200Cpu;
use emu_core::{types::Frame, MountPointInfo, System};
use gtia::Gtia;
use serde_json::Value;
use thiserror::Error;

/// Display width in pixels (standard playfield)
pub const DISPLAY_WIDTH: usize = 320;
/// Display height in scanlines
pub const DISPLAY_HEIGHT: usize = 192;
/// Total scanlines per NTSC frame
const NTSC_SCANLINES: u16 = 262;
/// Visible scanline start (after VBLANK)
const VISIBLE_START: u16 = 16;
/// CPU cycles per scanline (~114 color clocks / 2 = ~57 machine cycles, but actually 114)
const CYCLES_PER_SCANLINE: u32 = 114;

#[derive(Debug, Error)]
pub enum Atari5200Error {
    #[error("Cartridge error: {0}")]
    Cartridge(#[from] CartridgeError),
    #[error("No cartridge loaded")]
    NoCartridge,
    #[error("Invalid mount point: {0}")]
    InvalidMountPoint(String),
}

/// Atari 5200 system
pub struct Atari5200System {
    cpu: Atari5200Cpu,
    cycles: u64,
    framebuffer: Vec<u32>,
    instruction_tracer: emu_core::instruction_tracer::InstructionTracer,
    breakpoint_manager: emu_core::breakpoints::BreakpointManager,
    bios_loaded: bool,
}

impl Default for Atari5200System {
    fn default() -> Self {
        Self::new()
    }
}

impl Atari5200System {
    pub fn new() -> Self {
        let bus = Atari5200Bus::new();
        let cpu = Atari5200Cpu::new(bus);

        Self {
            cpu,
            cycles: 0,
            framebuffer: vec![0xFF000000; DISPLAY_WIDTH * DISPLAY_HEIGHT],
            instruction_tracer: emu_core::instruction_tracer::InstructionTracer::new(),
            breakpoint_manager: emu_core::breakpoints::BreakpointManager::new(),
            bios_loaded: false,
        }
    }

    /// Get audio samples from POKEY
    pub fn get_audio_samples(&mut self, count: usize) -> Vec<i16> {
        if let Some(bus) = self.cpu.bus_mut() {
            bus.pokey.generate_audio_samples(count)
        } else {
            vec![0; count]
        }
    }

    /// Set controller state
    ///
    /// Standard button mapping:
    /// - Bit 0: A (fire button / bottom side button)
    /// - Bit 1: B (top side button)
    /// - Bit 2: Select (not used on 5200, mapped to * key)
    /// - Bit 3: Start
    /// - Bit 4: Up
    /// - Bit 5: Down
    /// - Bit 6: Left
    /// - Bit 7: Right
    pub fn set_controller(&mut self, player: usize, state: u8) {
        if player > 1 {
            return;
        }

        if let Some(bus) = self.cpu.bus_mut() {
            let fire = (state & 0x01) != 0;
            let fire2 = (state & 0x02) != 0;
            let start = (state & 0x08) != 0;
            let up = (state & 0x10) != 0;
            let down = (state & 0x20) != 0;
            let left = (state & 0x40) != 0;
            let right = (state & 0x80) != 0;

            // Fire buttons go to GTIA triggers
            bus.gtia.set_trigger(player * 2, fire);
            bus.gtia.set_trigger(player * 2 + 1, fire2);

            // Start goes to GTIA console keys
            let _select = (state & 0x04) != 0;
            bus.gtia.set_console_keys(start, false, false);

            // Joystick directions go to POKEY pots
            // The 5200 uses analog sticks read via POKEY pot inputs
            // Center = 114, full left/up = 14, full right/down = 214
            let pot_base = player * 2;
            let h_value = if left {
                14
            } else if right {
                214
            } else {
                114
            };
            let v_value = if up {
                14
            } else if down {
                214
            } else {
                114
            };
            bus.pokey.set_pot(pot_base, h_value);
            bus.pokey.set_pot(pot_base + 1, v_value);
        }
    }

    emu_core::impl_instruction_tracer_methods!();
    emu_core::impl_breakpoint_methods!();

    /// Get the breakpoint manager
    pub fn get_breakpoint_manager(&self) -> &emu_core::breakpoints::BreakpointManager {
        &self.breakpoint_manager
    }

    /// Check if the current PC is at a breakpoint
    pub fn check_breakpoint(&self) -> Option<u32> {
        let pc = self.cpu.cpu.as_ref()?.pc as u32;
        if self.breakpoint_manager.should_break_execute(pc) {
            Some(pc)
        } else {
            None
        }
    }

    /// Render the current frame from ANTIC display list
    fn render_frame(&mut self) {
        // Clear framebuffer to background color
        let bg_color = if let Some(bus) = self.cpu.bus() {
            Gtia::color_to_rgb(bus.gtia.colbk())
        } else {
            0xFF000000
        };
        self.framebuffer.fill(bg_color);

        let Some(bus) = self.cpu.bus() else { return };

        // Check if DMA is enabled
        if !bus.antic.dma_enabled() {
            return;
        }

        // Process display list
        let mut dlist_addr = bus.antic.dlist();
        let mut screen_y: usize = 0;
        let mut memory_scan: u16 = 0;
        let char_base = bus.antic.char_base();

        // Safety limit for display list processing
        let mut instructions = 0;
        const MAX_INSTRUCTIONS: usize = 256;

        while screen_y < DISPLAY_HEIGHT && instructions < MAX_INSTRUCTIONS {
            instructions += 1;

            // Read display list instruction
            let instr = bus.read_internal(dlist_addr);
            dlist_addr = dlist_addr.wrapping_add(1);

            let mode = instr & 0x0F;
            let lms = instr & 0x40 != 0;
            let _dli = instr & 0x80 != 0;

            // Handle LMS (Load Memory Scan) - next 2 bytes are address
            if lms && mode >= 0x02 {
                let low = bus.read_internal(dlist_addr);
                dlist_addr = dlist_addr.wrapping_add(1);
                let high = bus.read_internal(dlist_addr);
                dlist_addr = dlist_addr.wrapping_add(1);
                memory_scan = (high as u16) << 8 | low as u16;
            }

            let antic_mode = Antic::decode_mode(mode);

            match antic_mode {
                AnticMode::Blank(n) => {
                    // Blank lines - just advance screen_y
                    screen_y += n as usize;
                }
                AnticMode::JumpVBlank => {
                    // Jump and wait for VBlank - indicates end of display list
                    let low = bus.read_internal(dlist_addr);
                    dlist_addr = dlist_addr.wrapping_add(1);
                    let high = bus.read_internal(dlist_addr);
                    let _jump_addr = (high as u16) << 8 | low as u16;
                    break; // End of visible display
                }
                AnticMode::Jump => {
                    // Jump to new display list address
                    let low = bus.read_internal(dlist_addr);
                    dlist_addr = dlist_addr.wrapping_add(1);
                    let high = bus.read_internal(dlist_addr);
                    dlist_addr = (high as u16) << 8 | low as u16;
                }
                // Character modes
                AnticMode::Mode2 | AnticMode::Mode3 => {
                    let rows = antic_mode.scanlines_per_row() as usize;
                    let bytes = antic_mode.bytes_per_line();
                    render_char_mode_2color(
                        &mut self.framebuffer,
                        bus,
                        screen_y,
                        rows,
                        bytes,
                        memory_scan,
                        char_base,
                    );
                    screen_y += rows;
                    memory_scan = memory_scan.wrapping_add(bytes as u16);
                }
                AnticMode::Mode4 | AnticMode::Mode5 => {
                    let rows = antic_mode.scanlines_per_row() as usize;
                    let bytes = antic_mode.bytes_per_line();
                    render_char_mode_5color(
                        &mut self.framebuffer,
                        bus,
                        screen_y,
                        rows,
                        bytes,
                        memory_scan,
                        char_base,
                    );
                    screen_y += rows;
                    memory_scan = memory_scan.wrapping_add(bytes as u16);
                }
                AnticMode::Mode6 | AnticMode::Mode7 => {
                    let rows = antic_mode.scanlines_per_row() as usize;
                    let bytes = antic_mode.bytes_per_line();
                    render_char_mode_5color(
                        &mut self.framebuffer,
                        bus,
                        screen_y,
                        rows,
                        bytes,
                        memory_scan,
                        char_base,
                    );
                    screen_y += rows;
                    memory_scan = memory_scan.wrapping_add(bytes as u16);
                }
                // Map (bitmap) modes
                AnticMode::Mode8 => {
                    let rows = antic_mode.scanlines_per_row() as usize;
                    let bytes = antic_mode.bytes_per_line();
                    render_map_mode_4color(
                        &mut self.framebuffer,
                        bus,
                        screen_y,
                        rows,
                        bytes,
                        memory_scan,
                        4,
                    );
                    screen_y += rows;
                    memory_scan = memory_scan.wrapping_add(bytes as u16);
                }
                AnticMode::Mode9 | AnticMode::ModeB | AnticMode::ModeC => {
                    let rows = antic_mode.scanlines_per_row() as usize;
                    let bytes = antic_mode.bytes_per_line();
                    render_map_mode_2color(
                        &mut self.framebuffer,
                        bus,
                        screen_y,
                        rows,
                        bytes,
                        memory_scan,
                    );
                    screen_y += rows;
                    memory_scan = memory_scan.wrapping_add(bytes as u16);
                }
                AnticMode::ModeA | AnticMode::ModeD | AnticMode::ModeE => {
                    let rows = antic_mode.scanlines_per_row() as usize;
                    let bytes = antic_mode.bytes_per_line();
                    render_map_mode_4color(
                        &mut self.framebuffer,
                        bus,
                        screen_y,
                        rows,
                        bytes,
                        memory_scan,
                        2,
                    );
                    screen_y += rows;
                    memory_scan = memory_scan.wrapping_add(bytes as u16);
                }
                AnticMode::ModeF => {
                    let rows = antic_mode.scanlines_per_row() as usize;
                    let bytes = antic_mode.bytes_per_line();
                    render_map_mode_hires(
                        &mut self.framebuffer,
                        bus,
                        screen_y,
                        rows,
                        bytes,
                        memory_scan,
                    );
                    screen_y += rows;
                    memory_scan = memory_scan.wrapping_add(bytes as u16);
                }
            }
        }

        // Render player-missile graphics on top
        render_players(&mut self.framebuffer, bus);
    }

    /// Get debug information
    pub fn debug_info(&self) -> Option<DebugInfo> {
        self.cpu.bus().and_then(|bus| {
            bus.cartridge.as_ref().map(|cart| DebugInfo {
                rom_size: cart.size(),
                banking_scheme: format!("{:?}", cart.scheme()),
                current_bank: cart.current_bank(),
                scanline: bus.antic.scanline(),
            })
        })
    }
}

/// Render a 2-color character mode line (modes 2, 3)
fn render_char_mode_2color(
    framebuffer: &mut [u32],
    bus: &Atari5200Bus,
    screen_y: usize,
    num_rows: usize,
    chars_per_line: usize,
    memory_scan: u16,
    char_base: u16,
) {
    let fg_color = Gtia::color_to_rgb(bus.gtia.colpf(1));
    let bg_color = Gtia::color_to_rgb(bus.gtia.colpf(2));

    for row in 0..num_rows {
        let y = screen_y + row;
        if y >= DISPLAY_HEIGHT {
            break;
        }

        let pixel_row = y * DISPLAY_WIDTH;
        let glyph_row = row;

        for ch_idx in 0..chars_per_line {
            let char_code = bus.read_internal(memory_scan.wrapping_add(ch_idx as u16));
            let glyph_addr = char_base
                .wrapping_add((char_code as u16) * 8)
                .wrapping_add(glyph_row as u16);
            let glyph_byte = bus.read_internal(glyph_addr);

            let x_start = ch_idx * 8;
            for bit in 0..8 {
                let x = x_start + bit;
                if x >= DISPLAY_WIDTH {
                    break;
                }
                let pixel = if glyph_byte & (0x80 >> bit) != 0 {
                    fg_color
                } else {
                    bg_color
                };
                framebuffer[pixel_row + x] = pixel;
            }
        }
    }
}

/// Render a 5-color character mode line (modes 4, 5, 6, 7)
fn render_char_mode_5color(
    framebuffer: &mut [u32],
    bus: &Atari5200Bus,
    screen_y: usize,
    num_rows: usize,
    chars_per_line: usize,
    memory_scan: u16,
    char_base: u16,
) {
    let colors = [
        Gtia::color_to_rgb(bus.gtia.colbk()),
        Gtia::color_to_rgb(bus.gtia.colpf(0)),
        Gtia::color_to_rgb(bus.gtia.colpf(1)),
        Gtia::color_to_rgb(bus.gtia.colpf(2)),
    ];

    for row in 0..num_rows {
        let y = screen_y + row;
        if y >= DISPLAY_HEIGHT {
            break;
        }

        let pixel_row = y * DISPLAY_WIDTH;
        let glyph_row = row;

        for ch_idx in 0..chars_per_line {
            let char_code = bus.read_internal(memory_scan.wrapping_add(ch_idx as u16));
            let glyph_addr = char_base
                .wrapping_add((char_code as u16) * 8)
                .wrapping_add(glyph_row as u16);
            let glyph_byte = bus.read_internal(glyph_addr);

            // 2 bits per pixel, 4 pixels per byte
            let x_start = ch_idx * 8;
            for pair in 0..4 {
                let color_idx = ((glyph_byte >> (6 - pair * 2)) & 0x03) as usize;
                let color = colors[color_idx];
                let x = x_start + pair * 2;
                if x < DISPLAY_WIDTH {
                    framebuffer[pixel_row + x] = color;
                }
                if x + 1 < DISPLAY_WIDTH {
                    framebuffer[pixel_row + x + 1] = color;
                }
            }
        }
    }
}

/// Render a 2-color map mode (modes 9, B, C)
fn render_map_mode_2color(
    framebuffer: &mut [u32],
    bus: &Atari5200Bus,
    screen_y: usize,
    num_rows: usize,
    bytes_per_line: usize,
    memory_scan: u16,
) {
    let fg_color = Gtia::color_to_rgb(bus.gtia.colpf(0));
    let bg_color = Gtia::color_to_rgb(bus.gtia.colbk());

    for row in 0..num_rows {
        let y = screen_y + row;
        if y >= DISPLAY_HEIGHT {
            break;
        }

        let pixel_row = y * DISPLAY_WIDTH;

        for byte_idx in 0..bytes_per_line {
            let data = bus.read_internal(memory_scan.wrapping_add(byte_idx as u16));

            for bit in 0..8 {
                let x = byte_idx * 8 + bit;
                if x >= DISPLAY_WIDTH {
                    break;
                }
                let pixel = if data & (0x80 >> bit) != 0 {
                    fg_color
                } else {
                    bg_color
                };
                framebuffer[pixel_row + x] = pixel;
            }
        }
    }
}

/// Render a 4-color map mode (modes 8, A, D, E)
fn render_map_mode_4color(
    framebuffer: &mut [u32],
    bus: &Atari5200Bus,
    screen_y: usize,
    num_rows: usize,
    bytes_per_line: usize,
    memory_scan: u16,
    pixels_wide: usize,
) {
    let colors = [
        Gtia::color_to_rgb(bus.gtia.colbk()),
        Gtia::color_to_rgb(bus.gtia.colpf(0)),
        Gtia::color_to_rgb(bus.gtia.colpf(1)),
        Gtia::color_to_rgb(bus.gtia.colpf(2)),
    ];

    for row in 0..num_rows {
        let y = screen_y + row;
        if y >= DISPLAY_HEIGHT {
            break;
        }

        let pixel_row = y * DISPLAY_WIDTH;

        for byte_idx in 0..bytes_per_line {
            let data = bus.read_internal(memory_scan.wrapping_add(byte_idx as u16));

            // 4 pixels per byte (2 bits each)
            for pair in 0..4 {
                let color_idx = ((data >> (6 - pair * 2)) & 0x03) as usize;
                let color = colors[color_idx];
                let x_start = byte_idx * 4 * pixels_wide + pair * pixels_wide;
                for px in 0..pixels_wide {
                    let x = x_start + px;
                    if x < DISPLAY_WIDTH {
                        framebuffer[pixel_row + x] = color;
                    }
                }
            }
        }
    }
}

/// Render hi-res mode (mode F: 320 pixels, 2 colors)
fn render_map_mode_hires(
    framebuffer: &mut [u32],
    bus: &Atari5200Bus,
    screen_y: usize,
    num_rows: usize,
    bytes_per_line: usize,
    memory_scan: u16,
) {
    let fg_color = Gtia::color_to_rgb(bus.gtia.colpf(1));
    let bg_color = Gtia::color_to_rgb(bus.gtia.colpf(2));

    for row in 0..num_rows {
        let y = screen_y + row;
        if y >= DISPLAY_HEIGHT {
            break;
        }

        let pixel_row = y * DISPLAY_WIDTH;

        for byte_idx in 0..bytes_per_line {
            let data = bus.read_internal(memory_scan.wrapping_add(byte_idx as u16));

            for bit in 0..8 {
                let x = byte_idx * 8 + bit;
                if x >= DISPLAY_WIDTH {
                    break;
                }
                let pixel = if data & (0x80 >> bit) != 0 {
                    fg_color
                } else {
                    bg_color
                };
                framebuffer[pixel_row + x] = pixel;
            }
        }
    }
}

/// Render player-missile graphics
fn render_players(framebuffer: &mut [u32], bus: &Atari5200Bus) {
    if !bus.antic.player_dma() {
        return;
    }

    let pm_base = bus.antic.pm_base();

    for player in 0..4 {
        let hpos = bus.gtia.hposp(player) as usize;
        let size = bus.gtia.sizep(player) & 0x03;
        let color = Gtia::color_to_rgb(bus.gtia.colpm(player));

        let width_multiplier = match size {
            0 => 1,
            1 => 2,
            3 => 4,
            _ => 1,
        };

        // Player data in PM area
        let player_base = pm_base.wrapping_add(0x400 + (player as u16) * 0x100);

        for y in 0..DISPLAY_HEIGHT {
            let scanline = y as u16 + VISIBLE_START;
            let gfx = bus.read_internal(player_base.wrapping_add(scanline));
            if gfx == 0 {
                continue;
            }

            let pixel_row = y * DISPLAY_WIDTH;
            for bit in 0..8 {
                if gfx & (0x80 >> bit) != 0 {
                    for w in 0..width_multiplier {
                        let x = hpos + bit * width_multiplier + w;
                        // Convert from color clock coords to pixel coords (approx 2:1)
                        let px = x.wrapping_sub(48); // HPOS offset
                        if px < DISPLAY_WIDTH {
                            framebuffer[pixel_row + px] = color;
                        }
                    }
                }
            }
        }
    }
}

/// Debug information
#[derive(Debug, Clone)]
pub struct DebugInfo {
    pub rom_size: usize,
    pub banking_scheme: String,
    pub current_bank: usize,
    pub scanline: u16,
}

impl System for Atari5200System {
    type Error = Atari5200Error;

    fn reset(&mut self) {
        self.cpu.reset();
        if let Some(bus) = self.cpu.bus_mut() {
            bus.reset();
        }
        self.cycles = 0;
    }

    fn step_frame(&mut self) -> Result<Frame, Self::Error> {
        let mut cpu_steps = 0u64;
        const MAX_CPU_STEPS: u64 = 50_000;

        // Run CPU until we've completed one frame (262 scanlines)
        let start_scanline = self.cpu.bus().map(|b| b.antic.scanline()).unwrap_or(0);
        let mut frame_complete = false;

        while cpu_steps < MAX_CPU_STEPS && !frame_complete {
            let cycles = self.cpu.step();
            cpu_steps += 1;
            self.cycles += cycles as u64;

            // Clock bus peripherals and check for interrupts
            let mut trigger_nmi = false;
            let mut trigger_irq = false;

            if let Some(bus) = self.cpu.bus_mut() {
                bus.clock(cycles);

                // Handle WSYNC
                if bus.take_wsync_request() {
                    let remaining = CYCLES_PER_SCANLINE.saturating_sub(cycles);
                    bus.clock(remaining);
                    self.cycles += remaining as u64;
                }

                // Check for new frame (VBI) based on scanline counting
                let total_cycles = self.cycles;
                let current_scanline =
                    ((total_cycles / CYCLES_PER_SCANLINE as u64) % NTSC_SCANLINES as u64) as u16;

                if current_scanline < start_scanline
                    || (start_scanline == 0 && current_scanline > 240)
                {
                    frame_complete = true;
                }

                // Handle NMI (VBI)
                if bus.antic.take_vbi_pending() {
                    trigger_nmi = true;
                    frame_complete = true;
                }

                // Handle IRQ from POKEY
                if bus.pokey.irq_pending() {
                    trigger_irq = true;
                }
            }

            // Apply interrupts after releasing the bus borrow
            if trigger_nmi {
                if let Some(cpu) = &mut self.cpu.cpu {
                    cpu.trigger_nmi();
                }
            }
            if trigger_irq {
                if let Some(cpu) = &mut self.cpu.cpu {
                    cpu.trigger_irq();
                }
            }
        }

        // Advance ANTIC scanline counter to sync
        if let Some(bus) = self.cpu.bus_mut() {
            let _ = bus.antic.clock_scanline();
        }

        // Render the frame
        self.render_frame();

        Ok(Frame {
            pixels: self.framebuffer.clone(),
            width: DISPLAY_WIDTH as u32,
            height: DISPLAY_HEIGHT as u32,
        })
    }

    fn save_state(&self) -> Value {
        serde_json::json!({
            "version": 1,
            "system": "atari5200",
            "cycles": self.cycles,
            "bus": self.cpu.bus(),
        })
    }

    fn load_state(&mut self, v: &Value) -> Result<(), serde_json::Error> {
        if let Some(bus_value) = v.get("bus") {
            let bus: Atari5200Bus = serde_json::from_value(bus_value.clone())?;
            if let Some(old_bus) = self.cpu.bus() {
                let cartridge = old_bus.cartridge.as_ref().map(|_| ());
                let _ = cartridge; // Preserve cartridge reference
            }
            if let Some(cpu) = self.cpu.cpu.take() {
                self.cpu.cpu = Some(cpu.with_memory(bus));
            }
        }
        if let Some(cycles) = v.get("cycles").and_then(|c| c.as_u64()) {
            self.cycles = cycles;
        }
        Ok(())
    }

    fn supports_save_states(&self) -> bool {
        true
    }

    fn mount_points(&self) -> Vec<MountPointInfo> {
        vec![
            MountPointInfo {
                id: "BIOS".to_string(),
                name: "BIOS ROM".to_string(),
                extensions: vec!["bin".to_string(), "rom".to_string()],
                required: false, // Has built-in stub BIOS
            },
            MountPointInfo {
                id: "Cartridge".to_string(),
                name: "Cartridge Slot".to_string(),
                extensions: vec!["a52".to_string(), "bin".to_string()],
                required: true,
            },
        ]
    }

    fn mount(&mut self, mount_point_id: &str, data: &[u8]) -> Result<(), Self::Error> {
        match mount_point_id {
            "BIOS" => {
                if let Some(bus) = self.cpu.bus_mut() {
                    bus.load_bios(data);
                }
                self.bios_loaded = true;
                Ok(())
            }
            "Cartridge" => {
                let cartridge = Cartridge::new(data.to_vec())?;
                if let Some(bus) = self.cpu.bus_mut() {
                    bus.load_cartridge(cartridge);
                }
                self.reset();
                Ok(())
            }
            _ => Err(Atari5200Error::InvalidMountPoint(
                mount_point_id.to_string(),
            )),
        }
    }

    fn unmount(&mut self, mount_point_id: &str) -> Result<(), Self::Error> {
        match mount_point_id {
            "BIOS" => {
                if let Some(bus) = self.cpu.bus_mut() {
                    bus.reset_bios();
                }
                self.bios_loaded = false;
                Ok(())
            }
            "Cartridge" => {
                if let Some(bus) = self.cpu.bus_mut() {
                    bus.cartridge = None;
                }
                Ok(())
            }
            _ => Err(Atari5200Error::InvalidMountPoint(
                mount_point_id.to_string(),
            )),
        }
    }

    fn is_mounted(&self, mount_point_id: &str) -> bool {
        match mount_point_id {
            "BIOS" => self.bios_loaded,
            "Cartridge" => self
                .cpu
                .bus()
                .map(|b| b.cartridge.is_some())
                .unwrap_or(false),
            _ => false,
        }
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
        let sys = Atari5200System::new();
        assert_eq!(sys.cycles, 0);
        assert_eq!(sys.framebuffer.len(), DISPLAY_WIDTH * DISPLAY_HEIGHT);
    }

    #[test]
    fn test_mount_cartridge() {
        let mut sys = Atari5200System::new();

        // Create a minimal 16KB ROM
        let mut rom = vec![0xEA; 16384]; // NOP fill
                                         // Set reset vector to point to start of cart ($8000)
        rom[0x3FFC] = 0x00; // Low byte
        rom[0x3FFD] = 0x80; // High byte

        assert!(sys.mount("Cartridge", &rom).is_ok());
        assert!(sys.is_mounted("Cartridge"));
    }

    #[test]
    fn test_unmount() {
        let mut sys = Atari5200System::new();
        let rom = vec![0xEA; 16384];
        sys.mount("Cartridge", &rom).unwrap();
        assert!(sys.is_mounted("Cartridge"));
        sys.unmount("Cartridge").unwrap();
        assert!(!sys.is_mounted("Cartridge"));
    }

    #[test]
    fn test_invalid_mount_point() {
        let mut sys = Atari5200System::new();
        let rom = vec![0xEA; 16384];
        assert!(sys.mount("Invalid", &rom).is_err());
    }

    #[test]
    fn test_mount_points() {
        let sys = Atari5200System::new();
        let mounts = sys.mount_points();
        assert_eq!(mounts.len(), 2);
        assert_eq!(mounts[0].id, "BIOS");
        assert_eq!(mounts[1].id, "Cartridge");
        assert!(mounts[1].extensions.contains(&"a52".to_string()));
    }

    #[test]
    fn test_save_load_state() {
        let mut sys = Atari5200System::new();
        let rom = vec![0xEA; 16384];
        sys.mount("Cartridge", &rom).unwrap();

        let state = sys.save_state();
        assert!(state.get("system").is_some());
        assert_eq!(state["system"], "atari5200");
    }

    #[test]
    fn test_step_frame() {
        let mut sys = Atari5200System::new();

        // Create a minimal ROM that loops
        let mut rom = vec![0x4C; 16384]; // JMP opcode fill
        rom[0] = 0x4C; // JMP
        rom[1] = 0x00; // Low byte ($8000)
        rom[2] = 0x80; // High byte
                       // Cart vectors
        rom[0x3FFC] = 0x00;
        rom[0x3FFD] = 0x80;

        sys.mount("Cartridge", &rom).unwrap();
        let result = sys.step_frame();
        assert!(result.is_ok());

        let frame = result.unwrap();
        assert_eq!(frame.width, DISPLAY_WIDTH as u32);
        assert_eq!(frame.height, DISPLAY_HEIGHT as u32);
        assert_eq!(frame.pixels.len(), DISPLAY_WIDTH * DISPLAY_HEIGHT);
    }

    #[test]
    fn test_controller_input() {
        let mut sys = Atari5200System::new();
        // Set controller with fire button pressed
        sys.set_controller(0, 0x01);
        // Shouldn't crash
        sys.set_controller(1, 0xFF);
        // Player 2+ should be ignored
        sys.set_controller(2, 0xFF);
    }
}
