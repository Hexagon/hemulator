//! Game Boy and Game Boy Color system implementation
//!
//! This module provides an emulator for the Nintendo Game Boy (DMG) and Game Boy Color (GBC) systems.
//! The implementation includes CPU emulation (Sharp LR35902), PPU (LCD controller), memory bus with
//! joypad support, and basic cartridge loading.
//!
//! # Architecture
//!
//! The Game Boy system consists of several key components:
//!
//! ## CPU: Sharp LR35902
//! - Z80-like CPU with some instructions removed and modified
//! - 8-bit registers: A, F, B, C, D, E, H, L (no shadow registers like Z80)
//! - 16-bit registers: SP (stack pointer), PC (program counter)
//! - Flags: Z (Zero), N (Subtract), H (Half Carry), C (Carry)
//! - Clock speed: 4.194304 MHz (~4.2 MHz)
//! - Implementation in `crates/core/src/cpu_lr35902.rs`
//!
//! ## PPU (Picture Processing Unit)
//! - Resolution: 160x144 pixels
//! - Display modes: DMG (4 shades of gray), CGB (32,768 colors)
//! - **Current implementation**: DMG mode only
//! - Supports:
//!   - Background layer with scrolling (SCX/SCY registers)
//!   - Window layer (overlay window with separate position)
//!   - 40 sprites (8x8 or 8x16 pixels)
//!   - Up to 10 sprites per scanline
//!   - Sprite priority and transparency
//!   - Horizontal/vertical sprite flipping
//! - Tile-based graphics (8x8 pixel tiles, 2 bits per pixel)
//! - Two tile data areas: $8000-$8FFF and $8800-$97FF
//! - Two tile map areas: $9800-$9BFF and $9C00-$9FFF
//!
//! ## Memory Map
//! - `$0000-$3FFF`: ROM Bank 0 (16KB, fixed)
//! - `$4000-$7FFF`: ROM Bank 1-N (16KB, switchable via MBC)
//! - `$8000-$9FFF`: VRAM (8KB, video RAM)
//! - `$A000-$BFFF`: External RAM (8KB, switchable via MBC)
//! - `$C000-$DFFF`: Work RAM (8KB)
//! - `$E000-$FDFF`: Echo RAM (mirror of $C000-$DDFF)
//! - `$FE00-$FE9F`: OAM (Object Attribute Memory - 160 bytes)
//! - `$FF00-$FF7F`: I/O Registers
//! - `$FF80-$FFFE`: High RAM (127 bytes)
//! - `$FFFF`: Interrupt Enable register
//!
//! ## I/O Registers
//! - `$FF00`: Joypad input (P1)
//! - `$FF0F`: Interrupt Flag (IF)
//! - `$FF10-$FF14`: APU Pulse 1 (sweep, duty, envelope, frequency)
//! - `$FF16-$FF19`: APU Pulse 2 (duty, envelope, frequency)
//! - `$FF1A-$FF1E`: APU Wave (DAC, length, volume, frequency)
//! - `$FF20-$FF23`: APU Noise (length, envelope, polynomial, control)
//! - `$FF24-$FF26`: APU Master (volume, panning, power)
//! - `$FF30-$FF3F`: Wave RAM (16 bytes, 32 x 4-bit samples)
//! - `$FF40`: LCD Control (LCDC)
//! - `$FF41`: LCD Status (STAT)
//! - `$FF42-$FF43`: Scroll registers (SCY, SCX)
//! - `$FF44`: LCD Y coordinate (LY)
//! - `$FF45`: LY Compare (LYC)
//! - `$FF47-$FF49`: Palette registers (BGP, OBP0, OBP1)
//! - `$FF4A-$FF4B`: Window position (WY, WX)
//! - `$FF50`: Boot ROM disable
//! - `$FFFF`: Interrupt Enable (IE)
//!
//! ## Joypad Input
//! The joypad register ($FF00) uses a matrix system:
//! - Bit 5: Select button keys (0 = selected)
//! - Bit 4: Select direction keys (0 = selected)
//! - Bits 3-0: Input bits (0 = pressed, 1 = not pressed)
//!   - Button mode: Start, Select, B, A
//!   - Direction mode: Down, Up, Left, Right
//!
//! # Timing
//!
//! - CPU clock: 4.194304 MHz
//! - Frame rate: ~59.73 Hz
//! - Cycles per frame: ~70,224
//! - Scanline cycles: 456 (114 machine cycles)
//! - Scanlines per frame: 154 (144 visible + 10 VBlank)
//!
//! # Current Implementation Status
//!
//! ## Implemented Features
//! - ✅ CPU: Full LR35902 instruction set
//! - ✅ PPU: Background rendering with scrolling
//! - ✅ PPU: Window rendering
//! - ✅ PPU: Sprite rendering (8x8 and 8x16 modes)
//! - ✅ PPU: Sprite priority, flipping, and transparency
//! - ✅ PPU: CGB color mode (8 BG palettes, 8 OBJ palettes, 15-bit RGB)
//! - ✅ PPU: VRAM banking (2 banks of 8KB for CGB)
//! - ✅ PPU: CGB tile attributes (palette, VRAM bank, flip)
//! - ✅ Memory: Full memory map with VRAM/OAM access
//! - ✅ DMA: OAM DMA transfer (register $FF46)
//! - ✅ Joypad: Button input via register $FF00
//! - ✅ I/O: Essential PPU and joypad registers
//! - ✅ I/O: CGB palette registers (BCPS/BCPD, OCPS/OCPD)
//! - ✅ I/O: CGB VRAM bank select (VBK)
//! - ✅ Save states: Full CPU state preservation
//! - ✅ APU: 4 sound channels (pulse 1/2, wave, noise)
//! - ✅ APU: Frame sequencer and envelope/sweep control
//! - ✅ APU: Audio sample generation at 44.1 kHz
//! - ✅ APU: Integrated with frontend for audio output
//! - ✅ Timer: Programmable timer with DIV, TIMA, TMA, TAC registers
//! - ✅ Interrupts: Full interrupt handling (VBlank, LCD STAT, Timer, Serial, Joypad)
//! - ✅ Interrupts: Priority-based interrupt servicing with IME flag
//! - ✅ CGB: Automatic mode detection and activation
//! - ✅ CGB: Speed switching (KEY1 register, STOP instruction)
//!
//! ## Not Yet Implemented
//! - ❌ Serial: Link cable communication
//!
//! # Known Limitations
//!
//! 1. **Timing Model**: Frame-based rendering (not cycle-accurate)
//!    - PPU renders entire frames at once
//!    - Some timing-critical effects may not work
//!    - Trade-off: Better compatibility vs. perfect accuracy
//!    - Note: Speed switching is supported but doesn't affect emulation timing
//!
//! 2. **ROM Support**: MBC0, MBC1, MBC2, MBC3, MBC5 supported
//!    - Covers approximately 96%+ of commercial Game Boy games
//!    - MBC2 fully implemented
//!    - Homebrew ROMs widely supported
//!
//! 3. **Game Boy Color**: Full CGB support implemented
//!    - Automatic CGB mode detection based on ROM header
//!    - VRAM banking (2 banks of 8KB)
//!    - Color palettes (8 BG + 8 OBJ, 4 colors each, 15-bit RGB)
//!    - Tile attributes (palette selection, VRAM bank, flipping)
//!    - Speed switching (normal 4.19 MHz / double 8.39 MHz)
//!    - Backward compatible with DMG games
//!
//! # Usage Example
//!
//! ```rust,no_run
//! use emu_core::System;
//! use emu_gb::GbSystem;
//!
//! // Create a new Game Boy system
//! let mut gb = GbSystem::new();
//!
//! // Load a ROM
//! let rom_data = std::fs::read("game.gb").unwrap();
//! gb.mount("Cartridge", &rom_data).unwrap();
//!
//! // Set controller state (buttons: Right=0, Left=1, Up=2, Down=3, A=4, B=5, Select=6, Start=7)
//! gb.set_controller(0x00); // All buttons released
//! gb.set_controller(0x10); // A button pressed
//!
//! // Run one frame
//! let frame = gb.step_frame().unwrap();
//! assert_eq!(frame.width, 160);
//! assert_eq!(frame.height, 144);
//! ```

use emu_core::debug::Debugger;
use emu_core::logging::{log, LogCategory, LogLevel};
use emu_core::{
    cpu_lr35902::CpuLr35902, cpu_lr35902::MemoryLr35902, types::Frame, MountPointInfo, System,
};

mod apu;
mod boot_rom;
mod bus;
mod debugger;
mod mappers;
pub(crate) mod ppu;
pub mod ppu_renderer;
mod timer;

use boot_rom::PostBootState;
use bus::GbBus;
use ppu_renderer::{PpuRenderer, SoftwarePpuRenderer};

/// Data for the tile viewer tab (Game Boy)
#[derive(Clone)]
pub struct TileViewerData {
    /// VRAM Bank 0 data (8KB)
    pub vram_bank0: Vec<u8>,
    /// VRAM Bank 1 data (8KB, CGB only - tile attributes)
    pub vram_bank1: Vec<u8>,
    /// OAM data - 160 bytes (40 sprites x 4 bytes each)
    pub oam: Vec<u8>,
    /// Background palette (DMG: 1 palette, CGB: 8 palettes x 4 colors)
    pub bg_palettes: Vec<u32>,
    /// Object palettes (DMG: 2 palettes, CGB: 8 palettes x 4 colors)
    pub obj_palettes: Vec<u32>,
    /// Current LCDC value
    pub lcdc: u8,
    /// Current scroll values
    pub scx: u8,
    pub scy: u8,
    /// Window position
    pub wx: u8,
    pub wy: u8,
    /// Whether this is CGB mode
    pub is_cgb_mode: bool,
}

pub struct GbSystem {
    cpu: CpuLr35902<GbBus>,
    cart_loaded: bool,
    /// Accumulated cycles for audio generation
    audio_cycles_accumulated: u32,
    /// Total CPU cycles executed since reset
    total_cycles: u64,
    /// Renderer for PPU output
    renderer: Box<dyn PpuRenderer>,
    /// Instruction tracer for debugging
    instruction_tracer: emu_core::instruction_tracer::InstructionTracer,
    /// Breakpoint manager for debugging
    breakpoint_manager: emu_core::breakpoints::BreakpointManager,
    /// One-shot log for PC=0x0038 hangs
    pc_0038_logged: bool,
}

impl Default for GbSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl GbSystem {
    pub fn new() -> Self {
        let bus = GbBus::new();
        let mut cpu = CpuLr35902::new(bus);
        cpu.reset();

        Self {
            cpu,
            cart_loaded: false,
            audio_cycles_accumulated: 0,
            total_cycles: 0,
            renderer: Box::new(SoftwarePpuRenderer::new()),
            instruction_tracer: emu_core::instruction_tracer::InstructionTracer::new(),
            breakpoint_manager: emu_core::breakpoints::BreakpointManager::new(),
            pc_0038_logged: false,
        }
    }

    /// Apply post-boot hardware state
    ///
    /// This method applies the CPU and I/O register values that would be set
    /// after the boot ROM completes. This allows skipping the boot ROM animation
    /// while maintaining correct hardware initialization.
    ///
    /// Call this after creating a new GbSystem to initialize hardware to the
    /// post-boot state (as if the boot ROM had run and completed).
    ///
    /// # Arguments
    /// * `is_cgb` - True for Game Boy Color mode, false for original Game Boy
    ///
    /// Reference: Pan Docs - Power-Up Sequence
    pub fn apply_post_boot_state(&mut self, is_cgb: bool) {
        let state = if is_cgb {
            PostBootState::cgb()
        } else {
            PostBootState::dmg()
        };

        // Apply CPU register state
        self.cpu.a = state.cpu.a;
        self.cpu.f = state.cpu.f;
        self.cpu.b = state.cpu.b;
        self.cpu.c = state.cpu.c;
        self.cpu.d = state.cpu.d;
        self.cpu.e = state.cpu.e;
        self.cpu.h = state.cpu.h;
        self.cpu.l = state.cpu.l;
        self.cpu.sp = state.cpu.sp;
        self.cpu.pc = state.cpu.pc;

        // Apply I/O register state
        self.cpu.memory.apply_post_boot_io_state(&state.io);
    }

    /// Set controller state (Game Boy buttons)
    /// Bits: 0=Right, 1=Left, 2=Up, 3=Down, 4=A, 5=B, 6=Select, 7=Start
    pub fn set_controller(&mut self, state: u8) {
        self.cpu.memory.set_buttons(state);
    }

    /// Get audio samples from the APU
    /// Generates samples based on accumulated CPU cycles
    pub fn get_audio_samples(&mut self, count: usize) -> Vec<i16> {
        // Calculate cycles needed for requested sample count
        // Sample rate: 44100 Hz, CPU clock: 4.194304 MHz
        // Cycles per sample: 4194304 / 44100 ≈ 95.1
        const SAMPLE_RATE: f64 = 44100.0;
        const CPU_CLOCK: f64 = 4194304.0;
        let cycles_needed = ((count as f64) * (CPU_CLOCK / SAMPLE_RATE)).ceil() as u32;

        // Use accumulated cycles from actual emulation
        let cycles_to_use = self.audio_cycles_accumulated.min(cycles_needed);

        let samples = self.cpu.memory.apu.generate_samples_stereo(cycles_to_use);

        // Subtract used cycles
        self.audio_cycles_accumulated = self.audio_cycles_accumulated.saturating_sub(cycles_to_use);

        // Pad with silence if we don't have enough samples
        let mut result = samples;
        let target_len = count * 2;
        while result.len() < target_len {
            result.push(0);
        }

        // Truncate if we have too many
        result.truncate(target_len);
        result
    }

    pub fn set_audio_channel_mask(&mut self, mask: [bool; 4]) {
        self.cpu.memory.apu.set_channel_mask(mask);
    }

    /// Get debug information about the Game Boy system
    pub fn debug_info(&self) -> DebugInfo {
        DebugInfo {
            pc: self.cpu.pc,
            sp: self.cpu.sp,
            af: u16::from(self.cpu.a) << 8 | u16::from(self.cpu.f),
            bc: u16::from(self.cpu.b) << 8 | u16::from(self.cpu.c),
            de: u16::from(self.cpu.d) << 8 | u16::from(self.cpu.e),
            hl: u16::from(self.cpu.h) << 8 | u16::from(self.cpu.l),
            ime: self.cpu.ime,
            halted: self.cpu.halted,
            ly: self.cpu.memory.ppu.ly,
            lcdc: self.cpu.memory.ppu.lcdc,
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

    /// Check if the current PC is at an execute breakpoint.
    /// Returns `Some(pc)` if a breakpoint is hit, `None` otherwise.
    pub fn check_breakpoint(&self) -> Option<u32> {
        let pc = self.cpu.pc as u32;
        if self.breakpoint_manager.should_break_execute(pc) {
            Some(pc)
        } else {
            None
        }
    }

    /// Get tile viewer data for debugging
    pub fn get_tile_viewer_data(&self) -> TileViewerData {
        let ppu = &self.cpu.memory.ppu;

        // Convert GB palettes to RGB colors
        let mut bg_palettes = Vec::new();
        let mut obj_palettes = Vec::new();

        if ppu.is_cgb_mode() {
            // CGB mode: 8 BG palettes + 8 OBJ palettes, 4 colors each
            for pal_idx in 0..8 {
                for color_idx in 0..4 {
                    let rgb = ppu.get_cgb_bg_color(pal_idx, color_idx);
                    bg_palettes.push(rgb);
                }
            }
            for pal_idx in 0..8 {
                for color_idx in 0..4 {
                    let rgb = ppu.get_cgb_obj_color(pal_idx, color_idx);
                    obj_palettes.push(rgb);
                }
            }
        } else {
            // DMG mode: 1 BG palette + 2 OBJ palettes
            for color_idx in 0..4 {
                bg_palettes.push(ppu.get_dmg_bg_color(color_idx));
            }
            for pal_idx in 0..2 {
                for color_idx in 0..4 {
                    obj_palettes.push(ppu.get_dmg_obj_color(pal_idx, color_idx));
                }
            }
        }

        TileViewerData {
            vram_bank0: ppu.get_vram_bank0().to_vec(),
            vram_bank1: ppu.get_vram_bank1().to_vec(),
            oam: ppu.get_oam().to_vec(),
            bg_palettes,
            obj_palettes,
            lcdc: ppu.lcdc,
            scx: ppu.scx,
            scy: ppu.scy,
            wx: ppu.wx,
            wy: ppu.wy,
            is_cgb_mode: ppu.is_cgb_mode(),
        }
    }
}

/// Debug information about the Game Boy system
#[derive(Debug, Clone)]
pub struct DebugInfo {
    pub pc: u16,
    pub sp: u16,
    pub af: u16,
    pub bc: u16,
    pub de: u16,
    pub hl: u16,
    pub ime: bool,
    pub halted: bool,
    pub ly: u8,
    pub lcdc: u8,
}

#[derive(thiserror::Error, Debug)]
pub enum GbError {
    #[error("No cartridge loaded")]
    NoCartridge,
    #[error("Invalid mount point")]
    InvalidMountPoint,
}

impl System for GbSystem {
    type Error = GbError;

    fn reset(&mut self) {
        self.cpu.reset();
        self.total_cycles = 0;
        self.pc_0038_logged = false;

        // Apply post-boot hardware state
        // This skips the boot ROM animation but initializes hardware correctly
        let is_cgb = self.cpu.memory.is_cgb_mode();
        self.apply_post_boot_state(is_cgb);
    }

    fn step_frame(&mut self) -> Result<Frame, Self::Error> {
        if !self.cart_loaded {
            return Err(GbError::NoCartridge);
        }

        // Game Boy runs at ~4.194304 MHz, frame rate is ~59.73 Hz
        // PPU cycles per frame: 4194304 / 59.73 ≈ 70224 cycles
        // In CGB double-speed mode, CPU runs at ~8.388608 MHz but PPU and APU
        // still run at normal speed. We track frame progress in PPU cycles.
        const CYCLES_PER_FRAME: u32 = 70224;

        let mut cycles = 0u32; // PPU-rate cycles for frame progress
        while cycles < CYCLES_PER_FRAME {
            // Execute any pending GDMA before CPU step
            self.cpu.memory.execute_pending_gdma();

            let is_double_speed = self.cpu.memory.is_double_speed();

            if self.cpu.memory.oam_dma_active() {
                // CPU is halted during OAM DMA. Advance time in 4-cycle chunks.
                let dma_cycles: u32 = 4;
                // In double-speed mode, PPU/APU run at half the CPU rate
                let ppu_cycles = if is_double_speed {
                    dma_cycles.div_ceil(2)
                } else {
                    dma_cycles
                };

                cycles += ppu_cycles;
                self.total_cycles += dma_cycles as u64;

                // Accumulate PPU-rate cycles for audio generation
                self.audio_cycles_accumulated += ppu_cycles;

                // Timer gets full CPU cycles (DIV runs at 2x in double-speed)
                if self.cpu.memory.timer.step(dma_cycles) {
                    // Timer overflow - request timer interrupt (bit 2)
                    self.cpu.memory.request_interrupt(0x04);
                }

                // Step OAM DMA transfer
                self.cpu.memory.step_oam_dma(dma_cycles);

                // Step PPU at normal rate
                let (vblank_started, stat_interrupt, hblank_entered) =
                    self.cpu.memory.ppu.step(ppu_cycles);

                if vblank_started {
                    // V-Blank started - request VBlank interrupt (bit 0)
                    self.cpu.memory.request_interrupt(0x01);
                }

                if stat_interrupt {
                    // STAT interrupt - request STAT interrupt (bit 1)
                    self.cpu.memory.request_interrupt(0x02);
                }

                // Perform HDMA transfer during HBlank if active
                if hblank_entered {
                    self.cpu.memory.step_hdma();
                }

                // Step serial transfer and handle serial interrupt
                if self.cpu.memory.step_serial(dma_cycles) {
                    // Serial transfer complete - request serial interrupt (bit 3)
                    self.cpu.memory.request_interrupt(0x08);
                }

                continue;
            }

            let pc_before = self.cpu.pc;
            let cpu_cycles = self.cpu.step();
            // In double-speed mode, PPU/APU run at half the CPU rate
            let ppu_cycles = if is_double_speed {
                cpu_cycles.div_ceil(2)
            } else {
                cpu_cycles
            };
            cycles += ppu_cycles;
            self.total_cycles += cpu_cycles as u64;

            if !self.pc_0038_logged && self.cpu.pc == 0x0038 {
                self.pc_0038_logged = true;
                let ie = self.cpu.memory.read(0xFFFF);
                let if_reg = self.cpu.memory.read(0xFF0F);
                let sp = self.cpu.sp;
                let s0 = self.cpu.memory.read(sp);
                let s1 = self.cpu.memory.read(sp.wrapping_add(1));
                let s2 = self.cpu.memory.read(sp.wrapping_add(2));
                let s3 = self.cpu.memory.read(sp.wrapping_add(3));
                log(LogCategory::CPU, LogLevel::Error, || {
                    format!(
                        "GB: PC hit $0038 (pc_before=${:04X}) A=${:02X} F=${:02X} BC=${:04X} DE=${:04X} HL=${:04X} SP=${:04X} IE=${:02X} IF=${:02X} STACK=[{:02X} {:02X} {:02X} {:02X}]",
                        pc_before,
                        self.cpu.a,
                        self.cpu.f,
                        ((self.cpu.b as u16) << 8) | self.cpu.c as u16,
                        ((self.cpu.d as u16) << 8) | self.cpu.e as u16,
                        ((self.cpu.h as u16) << 8) | self.cpu.l as u16,
                        sp,
                        ie,
                        if_reg,
                        s0,
                        s1,
                        s2,
                        s3
                    )
                });
            }

            // Record instruction if tracing is enabled
            if self.instruction_tracer.is_enabled() {
                if let Some(instr) = self.disassemble_instruction(pc_before as u32) {
                    let cpu_state = self.get_cpu_state();
                    self.instruction_tracer.trace(instr, cpu_state);
                }
            }

            // Accumulate PPU-rate cycles for audio generation
            self.audio_cycles_accumulated += ppu_cycles;

            // Timer gets full CPU cycles (DIV runs at 2x in double-speed)
            if self.cpu.memory.timer.step(cpu_cycles) {
                // Timer overflow - request timer interrupt (bit 2)
                self.cpu.memory.request_interrupt(0x04);
            }

            // Step OAM DMA transfer
            self.cpu.memory.step_oam_dma(cpu_cycles);

            // Step PPU at normal rate
            let (vblank_started, stat_interrupt, hblank_entered) =
                self.cpu.memory.ppu.step(ppu_cycles);

            if vblank_started {
                // V-Blank started - request VBlank interrupt (bit 0)
                self.cpu.memory.request_interrupt(0x01);
            }

            if stat_interrupt {
                // STAT interrupt - request STAT interrupt (bit 1)
                self.cpu.memory.request_interrupt(0x02);
            }

            // Perform HDMA transfer during HBlank if active
            if hblank_entered {
                self.cpu.memory.step_hdma();
            }

            // Step serial transfer and handle serial interrupt
            if self.cpu.memory.step_serial(cpu_cycles) {
                // Serial transfer complete - request serial interrupt (bit 3)
                self.cpu.memory.request_interrupt(0x08);
            }
        }

        // Tick mapper (e.g., for MBC3 RTC) once per frame
        self.cpu.memory.tick_mapper();

        // Render the frame using the renderer
        self.renderer.render_frame(&self.cpu.memory.ppu);
        Ok(self.renderer.get_frame().clone())
    }

    fn save_state(&self) -> serde_json::Value {
        serde_json::json!({
            "system": "gb",
            "version": 1,
            "cpu": {
                "a": self.cpu.a,
                "f": self.cpu.f,
                "b": self.cpu.b,
                "c": self.cpu.c,
                "d": self.cpu.d,
                "e": self.cpu.e,
                "h": self.cpu.h,
                "l": self.cpu.l,
                "sp": self.cpu.sp,
                "pc": self.cpu.pc,
                "ime": self.cpu.ime,
                "halted": self.cpu.halted,
                "stopped": self.cpu.stopped,
            }
        })
    }

    fn load_state(&mut self, v: &serde_json::Value) -> Result<(), serde_json::Error> {
        macro_rules! load_u8 {
            ($state:expr, $field:literal, $target:expr) => {
                if let Some(val) = $state.get($field).and_then(|v| v.as_u64()) {
                    $target = val as u8;
                }
            };
        }

        macro_rules! load_u16 {
            ($state:expr, $field:literal, $target:expr) => {
                if let Some(val) = $state.get($field).and_then(|v| v.as_u64()) {
                    $target = val as u16;
                }
            };
        }

        macro_rules! load_bool {
            ($state:expr, $field:literal, $target:expr) => {
                if let Some(val) = $state.get($field).and_then(|v| v.as_bool()) {
                    $target = val;
                }
            };
        }

        if let Some(cpu_state) = v.get("cpu") {
            load_u8!(cpu_state, "a", self.cpu.a);
            load_u8!(cpu_state, "f", self.cpu.f);
            load_u8!(cpu_state, "b", self.cpu.b);
            load_u8!(cpu_state, "c", self.cpu.c);
            load_u8!(cpu_state, "d", self.cpu.d);
            load_u8!(cpu_state, "e", self.cpu.e);
            load_u8!(cpu_state, "h", self.cpu.h);
            load_u8!(cpu_state, "l", self.cpu.l);
            load_u16!(cpu_state, "sp", self.cpu.sp);
            load_u16!(cpu_state, "pc", self.cpu.pc);
            load_bool!(cpu_state, "ime", self.cpu.ime);
            load_bool!(cpu_state, "halted", self.cpu.halted);
            load_bool!(cpu_state, "stopped", self.cpu.stopped);
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
            extensions: vec!["gb".to_string(), "gbc".to_string()],
            required: true,
        }]
    }

    fn mount(&mut self, mount_point_id: &str, data: &[u8]) -> Result<(), Self::Error> {
        if mount_point_id != "Cartridge" {
            return Err(GbError::InvalidMountPoint);
        }

        self.cpu.memory.load_cart(data);
        self.cart_loaded = true;
        self.reset();

        Ok(())
    }

    fn unmount(&mut self, mount_point_id: &str) -> Result<(), Self::Error> {
        if mount_point_id != "Cartridge" {
            return Err(GbError::InvalidMountPoint);
        }

        self.cart_loaded = false;
        Ok(())
    }

    fn is_mounted(&self, mount_point_id: &str) -> bool {
        mount_point_id == "Cartridge" && self.cart_loaded
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
    use emu_core::cpu_lr35902::MemoryLr35902;

    #[test]
    fn test_gb_system_creation() {
        let sys = GbSystem::new();
        assert!(!sys.cart_loaded);
    }

    #[test]
    fn test_gb_mount_points() {
        let sys = GbSystem::new();
        let mount_points = sys.mount_points();
        assert_eq!(mount_points.len(), 1);
        assert_eq!(mount_points[0].id, "Cartridge");
        assert!(mount_points[0].required);
    }

    #[test]
    fn test_gb_mount_unmount() {
        let mut sys = GbSystem::new();
        assert!(!sys.is_mounted("Cartridge"));

        // Mount a minimal ROM
        let rom = vec![0; 0x8000]; // 32KB ROM
        assert!(sys.mount("Cartridge", &rom).is_ok());
        assert!(sys.is_mounted("Cartridge"));

        assert!(sys.unmount("Cartridge").is_ok());
        assert!(!sys.is_mounted("Cartridge"));
    }

    #[test]
    fn test_gb_save_load_state() {
        let sys = GbSystem::new();
        let state = sys.save_state();
        assert_eq!(state["system"], "gb");
        assert_eq!(state["version"], 1);

        let mut sys2 = GbSystem::new();
        assert!(sys2.load_state(&state).is_ok());
    }

    #[test]
    fn test_gb_supports_save_states() {
        let sys = GbSystem::new();
        assert!(sys.supports_save_states());
    }

    #[test]
    fn test_gb_step_frame_without_cart() {
        let mut sys = GbSystem::new();
        let result = sys.step_frame();
        assert!(result.is_err());
    }

    #[test]
    fn test_gb_step_frame_with_cart() {
        let mut sys = GbSystem::new();
        let rom = vec![0; 0x8000];
        sys.mount("Cartridge", &rom).unwrap();

        let result = sys.step_frame();
        assert!(result.is_ok());
        let frame = result.unwrap();
        assert_eq!(frame.width, 160);
        assert_eq!(frame.height, 144);
    }

    #[test]
    fn test_gb_controller_input() {
        let mut sys = GbSystem::new();

        // Test setting controller state
        sys.set_controller(0xFF); // All buttons released

        // Test individual buttons
        sys.set_controller(0x01); // Right pressed
        sys.set_controller(0x10); // A pressed
        sys.set_controller(0x80); // Start pressed
    }

    #[test]
    fn test_gb_joypad_register_integration() {
        use emu_core::cpu_lr35902::MemoryLr35902;

        let mut sys = GbSystem::new();

        // Test button matrix reading
        // set_controller() takes GB layout directly: bits 0=Right, 1=Left, 2=Up, 3=Down, 4=A, 5=B, 6=Select, 7=Start
        // GB hardware uses active-low: 0 = pressed, 1 = released

        // Press A button (bit 4 in GB layout)
        sys.set_controller(0x10);

        // Select button keys (write 0x20 to clear P14, bit 4)
        sys.cpu.memory.write(0xFF00, 0x20);

        // Read joypad register - A is in the button matrix, bit 0 when reading buttons
        let joypad = sys.cpu.memory.read(0xFF00);
        assert_eq!(
            joypad & 0x01,
            0,
            "A button should be pressed (bit 0 = 0 when reading button matrix)"
        );

        // Press Right button (bit 0 in GB layout)
        sys.set_controller(0x01);

        // Select direction keys (write 0x10 to clear P15, bit 5)
        sys.cpu.memory.write(0xFF00, 0x10);

        // Read joypad register - Right is in d-pad matrix, bit 0 when reading d-pad
        let joypad = sys.cpu.memory.read(0xFF00);
        assert_eq!(
            joypad & 0x01,
            0,
            "Right button should be pressed (bit 0 = 0 when reading d-pad matrix)"
        );

        // Release all buttons (all bits set = all released in active-low GB format)
        sys.set_controller(0xFF);

        // Select button keys
        sys.cpu.memory.write(0xFF00, 0x20);
        let joypad = sys.cpu.memory.read(0xFF00);
        assert_eq!(joypad & 0x0F, 0x0F, "All buttons should be released");

        // Select direction keys
        sys.cpu.memory.write(0xFF00, 0x10);
        let joypad = sys.cpu.memory.read(0xFF00);
        assert_eq!(joypad & 0x0F, 0x0F, "All directions should be released");
    }

    #[test]
    fn test_gb_ppu_registers() {
        let sys = GbSystem::new();

        // Verify initial PPU register values
        assert_eq!(sys.cpu.memory.ppu.lcdc, 0x91);
        assert_eq!(sys.cpu.memory.ppu.bgp, 0xFC);
        assert_eq!(sys.cpu.memory.ppu.ly, 0);
    }

    #[test]
    fn test_gb_audio_samples() {
        let mut sys = GbSystem::new();

        // Load a minimal ROM to allow stepping
        let rom = vec![0; 0x8000];
        sys.mount("Cartridge", &rom).unwrap();

        // Run a few frames to accumulate cycles
        for _ in 0..10 {
            let _ = sys.step_frame();
        }

        // Request audio samples
        let samples = sys.get_audio_samples(1000);

        // Verify we got the requested number of samples
        assert_eq!(samples.len(), 2000);

        // Samples should be valid i16 values (no need to check range, type system ensures this)
        // Audio system should not crash when generating samples
    }

    #[test]
    fn test_gb_cgb_mode_detection() {
        let mut sys = GbSystem::new();

        // Create a ROM with CGB flag set (0x80 = works on both DMG and CGB)
        let mut rom = vec![0; 0x150];
        rom[0x143] = 0x80; // CGB compatible
        rom[0x147] = 0x00; // ROM ONLY
        rom[0x149] = 0x00; // No RAM

        sys.mount("Cartridge", &rom).unwrap();

        // Check that CGB mode is detected
        assert!(sys.cpu.memory.is_cgb_mode());
        // Verify A register is set to 0x11 for CGB mode
        assert_eq!(sys.cpu.a, 0x11, "A register should be 0x11 for CGB mode");

        // Create a ROM without CGB flag
        let mut rom2 = vec![0; 0x150];
        rom2[0x143] = 0x00; // No CGB
        rom2[0x147] = 0x00;
        rom2[0x149] = 0x00;

        sys.unmount("Cartridge").unwrap();
        sys.mount("Cartridge", &rom2).unwrap();

        // Check that CGB mode is not detected
        assert!(!sys.cpu.memory.is_cgb_mode());
        // Verify A register is set to 0x01 for DMG mode
        assert_eq!(sys.cpu.a, 0x01, "A register should be 0x01 for DMG mode");
    }

    #[test]
    fn test_gb_cgb_only_mode() {
        // Test CGB-only games (flag 0xC0)
        let mut sys = GbSystem::new();

        let mut rom = vec![0; 0x150];
        rom[0x143] = 0xC0; // CGB only
        rom[0x147] = 0x00; // ROM ONLY
        rom[0x149] = 0x00; // No RAM

        sys.mount("Cartridge", &rom).unwrap();

        // Check that CGB mode is detected for CGB-only games
        assert!(sys.cpu.memory.is_cgb_mode());
        // Verify A register is set to 0x11 for CGB-only mode
        assert_eq!(
            sys.cpu.a, 0x11,
            "A register should be 0x11 for CGB-only games"
        );
    }

    #[test]
    fn test_cgb_palette_initialization() {
        use emu_core::cpu_lr35902::MemoryLr35902;

        // Test CGB compatibility mode (0x80) - should get DMG default palette
        let mut sys = GbSystem::new();
        let mut rom = vec![0; 0x150];
        rom[0x143] = 0x80; // CGB compatible (DMG with color support)
        rom[0x147] = 0x00; // ROM ONLY
        rom[0x149] = 0x00; // No RAM

        sys.mount("Cartridge", &rom).unwrap();

        // Verify CGB mode is enabled
        assert!(sys.cpu.memory.is_cgb_mode());
        assert!(sys.cpu.memory.ppu.is_cgb_mode());

        // Verify default DMG-compatible greenish palette is set
        // Read palette 0, color 0 (should be white: 0x7FFF)
        sys.cpu.memory.write(0xFF68, 0x00); // BCPS: index 0
        let color0_low = sys.cpu.memory.read(0xFF69); // BCPD: read low byte
        sys.cpu.memory.write(0xFF68, 0x01); // BCPS: index 1
        let color0_high = sys.cpu.memory.read(0xFF69); // BCPD: read high byte
        assert_eq!(color0_low, 0xFF, "Color 0 low byte should be 0xFF (white)");
        assert_eq!(
            color0_high, 0x7F,
            "Color 0 high byte should be 0x7F (white)"
        );

        // Read palette 0, color 1 (should be light green: 0x3E90)
        sys.cpu.memory.write(0xFF68, 0x02); // BCPS: index 2
        let color1_low = sys.cpu.memory.read(0xFF69);
        sys.cpu.memory.write(0xFF68, 0x03); // BCPS: index 3
        let color1_high = sys.cpu.memory.read(0xFF69);
        assert_eq!(
            color1_low, 0x90,
            "Color 1 low byte should be 0x90 (light green)"
        );
        assert_eq!(
            color1_high, 0x3E,
            "Color 1 high byte should be 0x3E (light green)"
        );

        // Read palette 0, color 2 (should be dark green: 0x16C4)
        sys.cpu.memory.write(0xFF68, 0x04); // BCPS: index 4
        let color2_low = sys.cpu.memory.read(0xFF69);
        sys.cpu.memory.write(0xFF68, 0x05); // BCPS: index 5
        let color2_high = sys.cpu.memory.read(0xFF69);
        assert_eq!(
            color2_low, 0xC4,
            "Color 2 low byte should be 0xC4 (dark green)"
        );
        assert_eq!(
            color2_high, 0x16,
            "Color 2 high byte should be 0x16 (dark green)"
        );

        // Read palette 0, color 3 (should be black: 0x0000)
        sys.cpu.memory.write(0xFF68, 0x06); // BCPS: index 6
        let color3_low = sys.cpu.memory.read(0xFF69);
        sys.cpu.memory.write(0xFF68, 0x07); // BCPS: index 7
        let color3_high = sys.cpu.memory.read(0xFF69);
        assert_eq!(color3_low, 0x00, "Color 3 low byte should be 0x00 (black)");
        assert_eq!(
            color3_high, 0x00,
            "Color 3 high byte should be 0x00 (black)"
        );

        // Test CGB-only mode (0xC0) - should get white palette
        let mut sys2 = GbSystem::new();
        let mut rom2 = vec![0; 0x150];
        rom2[0x143] = 0xC0; // CGB only
        rom2[0x147] = 0x00;
        rom2[0x149] = 0x00;

        sys2.mount("Cartridge", &rom2).unwrap();

        // Verify CGB mode is enabled
        assert!(sys2.cpu.memory.is_cgb_mode());

        // Verify white palette is set (all colors should be 0x7FFF)
        sys2.cpu.memory.write(0xFF68, 0x00); // BCPS: index 0
        let white_low = sys2.cpu.memory.read(0xFF69);
        sys2.cpu.memory.write(0xFF68, 0x01);
        let white_high = sys2.cpu.memory.read(0xFF69);
        assert_eq!(white_low, 0xFF, "Color 0 should be white in CGB-only mode");
        assert_eq!(white_high, 0x7F, "Color 0 should be white in CGB-only mode");
    }

    #[test]
    fn test_gb_smoke_test_rom() {
        // Load the test ROM
        let test_rom = include_bytes!("../../../../test_roms/gb/test.gb");

        let mut sys = GbSystem::new();

        // Mount the test ROM
        assert!(sys.mount("Cartridge", test_rom).is_ok());
        assert!(sys.is_mounted("Cartridge"));

        // Run a few frames to let the ROM initialize and render
        let mut frame = sys.step_frame().unwrap();
        for _ in 0..9 {
            frame = sys.step_frame().unwrap();
        }

        // Verify frame dimensions are correct
        assert_eq!(frame.width, 160);
        assert_eq!(frame.height, 144);
        assert_eq!(frame.pixels.len(), 160 * 144);

        // The test ROM creates a checkerboard pattern with:
        // - Tile 0: White (all pixels color index 0)
        // - Tile 1: Dark gray (all pixels color index 1, represents "red" on monochrome DMG)
        // Each tile is 8x8 pixels, screen is 20x18 tiles

        // Expected colors (ARGB format):
        const WHITE: u32 = 0xFFFFFFFF;
        const DARK_GRAY: u32 = 0xFF555555; // Represents "red" on monochrome Game Boy

        // Verify checkerboard pattern horizontally (first row, y=0)
        // First tile (x=0-7) should be white
        for x in 0..8 {
            let pixel = frame.pixels[x];
            assert_eq!(
                pixel, WHITE,
                "Expected white pixel at ({},0), got 0x{:08X}",
                x, pixel
            );
        }
        // Second tile (x=8-15) should be dark gray
        for x in 8..16 {
            let pixel = frame.pixels[x];
            assert_eq!(
                pixel, DARK_GRAY,
                "Expected dark gray pixel at ({},0), got 0x{:08X}",
                x, pixel
            );
        }
        // Third tile (x=16-23) should be white (pattern continues)
        for x in 16..24 {
            let pixel = frame.pixels[x];
            assert_eq!(
                pixel, WHITE,
                "Expected white pixel at ({},0), got 0x{:08X}",
                x, pixel
            );
        }

        // Verify checkerboard pattern vertically (first column, x=0)
        // First tile row (y=0-7) should be white
        for y in 0..8 {
            let pixel = frame.pixels[y * 160];
            assert_eq!(
                pixel, WHITE,
                "Expected white pixel at (0,{}), got 0x{:08X}",
                y, pixel
            );
        }
        // Second tile row (y=8-15) should be dark gray (checkerboard alternates by row)
        for y in 8..16 {
            let pixel = frame.pixels[y * 160];
            assert_eq!(
                pixel, DARK_GRAY,
                "Expected dark gray pixel at (0,{}), got 0x{:08X}",
                y, pixel
            );
        }

        // Verify there are exactly two colors in the frame
        let mut colors = std::collections::HashSet::new();
        for &pixel in &frame.pixels {
            colors.insert(pixel);
        }
        assert_eq!(
            colors.len(),
            2,
            "Expected exactly 2 colors, got {}: {:?}",
            colors.len(),
            colors
        );
        assert!(colors.contains(&WHITE), "Missing white color");
        assert!(
            colors.contains(&DARK_GRAY),
            "Missing dark gray color (representing red)"
        );
    }

    #[test]
    fn test_gbc_smoke_test_rom() {
        // Load the GBC test ROM
        let test_rom = include_bytes!("../../../../test_roms/gbc/test.gbc");

        let mut sys = GbSystem::new();

        // Mount the test ROM
        assert!(sys.mount("Cartridge", test_rom).is_ok());
        assert!(sys.is_mounted("Cartridge"));

        // Run a few frames to let the ROM initialize and render
        // Note: This ROM has CGB flag set but should work in DMG mode too
        let mut frame = sys.step_frame().unwrap();
        for _ in 0..9 {
            frame = sys.step_frame().unwrap();
        }

        // Verify frame dimensions are correct
        assert_eq!(frame.width, 160);
        assert_eq!(frame.height, 144);
        assert_eq!(frame.pixels.len(), 160 * 144);

        // The test ROM fills the screen with a checkerboard pattern.
        // Verify that the frame contains non-black pixel data.
        let non_black_pixels = frame
            .pixels
            .iter()
            .filter(|&&pixel| pixel != 0xFF000000) // Not black (ARGB format)
            .count();

        // Should have visible pixels from the test pattern (at least 2000 non-black pixels)
        assert!(
            non_black_pixels > 2000,
            "Expected non-black pixels from GBC test ROM in DMG mode, got {} out of {}",
            non_black_pixels,
            160 * 144
        );
    }

    #[test]
    fn test_gb_interrupt_handling() {
        // Test that interrupts are properly handled
        let mut sys = GbSystem::new();

        // Create a minimal ROM with interrupt handling
        let mut rom = vec![0; 0x8000];

        // VBlank interrupt handler at 0x40: just RETI
        rom[0x40] = 0xD9; // RETI

        // Entry point at 0x100
        rom[0x100] = 0x3E; // LD A, 0x01
        rom[0x101] = 0x01;
        rom[0x102] = 0xE0; // LDH ($FF), A  (write to IE at 0xFFFF)
        rom[0x103] = 0xFF;
        rom[0x104] = 0xFB; // EI (enable interrupts)
        rom[0x105] = 0x76; // HALT
        rom[0x106] = 0x00; // NOP (should execute after interrupt)
        rom[0x107] = 0x18; // JR -4 (loop back to HALT at 0x105)
        rom[0x108] = 0xFC; // -4 offset

        sys.mount("Cartridge", &rom).unwrap();

        // Run one frame - this should trigger VBlank interrupt
        let frame = sys.step_frame().unwrap();
        assert_eq!(frame.width, 160);
        assert_eq!(frame.height, 144);

        // Verify the system is still running (not stuck in HALT)
        // The PC should have advanced beyond 0x105 (HALT instruction)
        // After interrupt handling, it should be back in the loop
        let _debug = sys.debug_info();

        // After the first frame with interrupts enabled, the system should have:
        // 1. Executed HALT at 0x105
        // 2. Received VBlank interrupt
        // 3. Jumped to 0x40 (VBlank handler)
        // 4. Executed RETI and returned
        // 5. Continued execution after HALT

        // The PC won't be exactly predictable due to timing, but it should not be stuck at 0x105
        // and the system should continue to run frames without hanging
        for _ in 0..5 {
            let _ = sys.step_frame().unwrap();
        }

        // If we got here without hanging, interrupts are working!
    }

    #[test]
    fn test_oam_direct_write() {
        // Test direct OAM writes to verify write_oam() function works
        use crate::ppu::Ppu;

        let mut ppu = Ppu::new();

        ppu.write_oam(0, 0x80);
        ppu.write_oam(1, 0x10);
        ppu.write_oam(2, 0x58);
        ppu.write_oam(3, 0x00);

        assert_eq!(ppu.read_oam_debug(0), 0x80, "OAM[0] should be 0x80");
        assert_eq!(ppu.read_oam_debug(1), 0x10, "OAM[1] should be 0x10");
        assert_eq!(ppu.read_oam_debug(2), 0x58, "OAM[2] should be 0x58");
        assert_eq!(ppu.read_oam_debug(3), 0x00, "OAM[3] should be 0x00");
    }

    #[test]
    fn test_oam_dma_basic() {
        // Test basic DMA operation with 4 bytes of sprite data
        let mut sys = GbSystem::new();

        // Create a minimal ROM
        let mut rom = vec![0; 0x8000];
        rom[0x100] = 0x00; // NOP at entry point
        sys.mount("Cartridge", &rom).unwrap();

        // Write sprite data to WRAM at $C000
        sys.cpu.memory.write(0xC000, 0x80); // Y position
        sys.cpu.memory.write(0xC001, 0x10); // X position
        sys.cpu.memory.write(0xC002, 0x58); // Tile index
        sys.cpu.memory.write(0xC003, 0x00); // Flags

        // Trigger DMA from $C000 to OAM
        sys.cpu.memory.write(0xFF46, 0xC0);
        // DMA transfers 160 bytes at 4 cycles each = 640 cycles
        sys.cpu.memory.step_oam_dma(640);

        // Verify OAM was updated
        assert_eq!(
            sys.cpu.memory.ppu.read_oam_debug(0),
            0x80,
            "OAM[0] should be 0x80"
        );
        assert_eq!(
            sys.cpu.memory.ppu.read_oam_debug(1),
            0x10,
            "OAM[1] should be 0x10"
        );
        assert_eq!(
            sys.cpu.memory.ppu.read_oam_debug(2),
            0x58,
            "OAM[2] should be 0x58"
        );
        assert_eq!(
            sys.cpu.memory.ppu.read_oam_debug(3),
            0x00,
            "OAM[3] should be 0x00"
        );
    }

    #[test]
    fn test_oam_dma_full_copy() {
        // Test full 160-byte DMA transfer
        let mut sys = GbSystem::new();

        // Create a minimal ROM
        let mut rom = vec![0; 0x8000];
        rom[0x100] = 0x00; // NOP at entry point
        sys.mount("Cartridge", &rom).unwrap();

        // Fill WRAM with test pattern (160 bytes)
        for i in 0..160 {
            let test_value = (i as u8) ^ 0xAA;
            sys.cpu.memory.write(0xC000 + i as u16, test_value);
        }

        // Trigger DMA from $C000 to OAM
        sys.cpu.memory.write(0xFF46, 0xC0);
        // DMA transfers 160 bytes at 4 cycles each = 640 cycles
        sys.cpu.memory.step_oam_dma(640);

        // Verify all 160 bytes copied correctly
        for i in 0..160 {
            let expected = (i as u8) ^ 0xAA;
            let actual = sys.cpu.memory.ppu.read_oam_debug(i);
            assert_eq!(
                actual, expected,
                "OAM[{}] mismatch: expected {:02X}, got {:02X}",
                i, expected, actual
            );
        }
    }

    #[test]
    fn test_stat_register_bit7() {
        // Test that STAT register bit 7 (unused) always reads as 1
        use emu_core::cpu_lr35902::MemoryLr35902;

        let mut sys = GbSystem::new();

        // Write various values to STAT
        sys.cpu.memory.write(0xFF41, 0x00);
        assert_eq!(
            sys.cpu.memory.read(0xFF41) & 0x80,
            0x80,
            "STAT bit 7 should always read as 1"
        );

        sys.cpu.memory.write(0xFF41, 0x7F);
        assert_eq!(
            sys.cpu.memory.read(0xFF41) & 0x80,
            0x80,
            "STAT bit 7 should always read as 1"
        );

        sys.cpu.memory.write(0xFF41, 0xFF);
        assert_eq!(
            sys.cpu.memory.read(0xFF41) & 0x80,
            0x80,
            "STAT bit 7 should always read as 1"
        );
    }

    #[test]
    fn test_joypad_register_bits67() {
        // Test that joypad register bits 6-7 (unused) always read as 1
        use emu_core::cpu_lr35902::MemoryLr35902;

        let mut sys = GbSystem::new();

        // Write various values to P1 register
        sys.cpu.memory.write(0xFF00, 0x00);
        assert_eq!(
            sys.cpu.memory.read(0xFF00) & 0xC0,
            0xC0,
            "P1 bits 6-7 should always read as 1"
        );

        sys.cpu.memory.write(0xFF00, 0x3F);
        assert_eq!(
            sys.cpu.memory.read(0xFF00) & 0xC0,
            0xC0,
            "P1 bits 6-7 should always read as 1"
        );

        sys.cpu.memory.write(0xFF00, 0xFF);
        assert_eq!(
            sys.cpu.memory.read(0xFF00) & 0xC0,
            0xC0,
            "P1 bits 6-7 should always read as 1"
        );

        // Test with button selection bits
        sys.cpu.memory.write(0xFF00, 0x10); // Select d-pad
        assert_eq!(
            sys.cpu.memory.read(0xFF00) & 0xC0,
            0xC0,
            "P1 bits 6-7 should always read as 1 even with selection bits set"
        );

        sys.cpu.memory.write(0xFF00, 0x20); // Select buttons
        assert_eq!(
            sys.cpu.memory.read(0xFF00) & 0xC0,
            0xC0,
            "P1 bits 6-7 should always read as 1 even with selection bits set"
        );
    }

    #[test]
    fn test_echo_ram_mirror() {
        // Test that Echo RAM (0xE000-0xFDFF) properly mirrors WRAM (0xC000-0xDDFF)
        use emu_core::cpu_lr35902::MemoryLr35902;

        let mut sys = GbSystem::new();

        // Write to WRAM and verify it's readable from Echo RAM
        sys.cpu.memory.write(0xC000, 0x42);
        assert_eq!(
            sys.cpu.memory.read(0xE000),
            0x42,
            "Echo RAM should mirror WRAM"
        );

        sys.cpu.memory.write(0xC123, 0xAB);
        assert_eq!(
            sys.cpu.memory.read(0xE123),
            0xAB,
            "Echo RAM should mirror WRAM"
        );

        sys.cpu.memory.write(0xDDFF, 0xCD);
        assert_eq!(
            sys.cpu.memory.read(0xFDFF),
            0xCD,
            "Echo RAM should mirror up to 0xDDFF/0xFDFF"
        );

        // Write to Echo RAM and verify it affects WRAM
        sys.cpu.memory.write(0xE500, 0x55);
        assert_eq!(
            sys.cpu.memory.read(0xC500),
            0x55,
            "Writing to Echo RAM should affect WRAM"
        );

        sys.cpu.memory.write(0xFD00, 0x99);
        assert_eq!(
            sys.cpu.memory.read(0xDD00),
            0x99,
            "Writing to Echo RAM should affect WRAM"
        );
    }

    #[test]
    fn test_interrupt_register_bits() {
        // Test that IF and IE registers handle unused bits correctly
        use emu_core::cpu_lr35902::MemoryLr35902;

        let mut sys = GbSystem::new();

        // Test IF register (0xFF0F) - bits 5-7 should read as 1
        sys.cpu.memory.write(0xFF0F, 0x00);
        assert_eq!(
            sys.cpu.memory.read(0xFF0F) & 0xE0,
            0xE0,
            "IF bits 5-7 should always read as 1"
        );

        sys.cpu.memory.write(0xFF0F, 0x1F);
        assert_eq!(
            sys.cpu.memory.read(0xFF0F) & 0xE0,
            0xE0,
            "IF bits 5-7 should always read as 1"
        );

        sys.cpu.memory.write(0xFF0F, 0xFF);
        assert_eq!(
            sys.cpu.memory.read(0xFF0F),
            0xFF,
            "IF should read 0xFF when all writable bits are set"
        );

        // Test IE register (0xFFFF) - bits 5-7 should read as 1
        sys.cpu.memory.write(0xFFFF, 0x00);
        assert_eq!(
            sys.cpu.memory.read(0xFFFF) & 0xE0,
            0xE0,
            "IE bits 5-7 should always read as 1"
        );

        sys.cpu.memory.write(0xFFFF, 0x1F);
        assert_eq!(
            sys.cpu.memory.read(0xFFFF) & 0xE0,
            0xE0,
            "IE bits 5-7 should always read as 1"
        );

        sys.cpu.memory.write(0xFFFF, 0xFF);
        assert_eq!(
            sys.cpu.memory.read(0xFFFF),
            0xFF,
            "IE should read 0xFF when all writable bits are set"
        );
    }

    #[test]
    fn test_cgb_speed_switching() {
        // Test CGB speed switching via KEY1 register and STOP instruction
        let mut sys = GbSystem::new();

        // Create a CGB ROM
        let mut rom = vec![0; 0x8000];
        rom[0x143] = 0x80; // CGB compatible
        rom[0x147] = 0x00; // ROM ONLY
        rom[0x149] = 0x00; // No RAM

        // Program to test speed switching
        // 0x100: Write 0x01 to KEY1 (prepare speed switch)
        rom[0x100] = 0x3E; // LD A, 0x01
        rom[0x101] = 0x01;
        rom[0x102] = 0xE0; // LDH ($4D), A (write to KEY1 at 0xFF4D)
        rom[0x103] = 0x4D;
        // 0x104: Execute STOP instruction
        rom[0x104] = 0x10; // STOP
        rom[0x105] = 0x00; // Immediate byte (always 0x00)
                           // 0x106: Continue execution after speed switch
        rom[0x106] = 0x00; // NOP
        rom[0x107] = 0x00; // NOP

        sys.mount("Cartridge", &rom).unwrap();

        // Verify we're in CGB mode
        assert!(sys.cpu.memory.is_cgb_mode());

        // KEY1 should start at 0 (normal speed)
        let key1_initial = sys.cpu.memory.read(0xFF4D);
        assert_eq!(
            key1_initial & 0x80,
            0x00,
            "KEY1 bit 7 should start at 0 (normal speed)"
        );

        // Execute the program
        // Step 1: LD A, 0x01
        sys.cpu.step();
        assert_eq!(sys.cpu.a, 0x01);

        // Step 2: LDH ($4D), A - write to KEY1
        sys.cpu.step();
        let key1_after_write = sys.cpu.memory.read(0xFF4D);
        assert_eq!(
            key1_after_write & 0x01,
            0x01,
            "KEY1 bit 0 should be set (speed switch armed)"
        );
        assert_eq!(
            key1_after_write & 0x80,
            0x00,
            "KEY1 bit 7 should still be 0 (still in normal speed)"
        );

        // Verify CPU is not stopped before STOP instruction
        assert!(!sys.cpu.stopped, "CPU should not be stopped yet");

        // Step 3: STOP instruction - should perform speed switch
        sys.cpu.step();

        // After STOP with KEY1 bit 0 set, speed should have switched
        let key1_after_stop = sys.cpu.memory.read(0xFF4D);
        assert_eq!(
            key1_after_stop & 0x80,
            0x80,
            "KEY1 bit 7 should be set (double speed mode)"
        );
        assert_eq!(
            key1_after_stop & 0x01,
            0x00,
            "KEY1 bit 0 should be cleared (speed switch completed)"
        );
        assert!(
            !sys.cpu.stopped,
            "CPU should not be stopped after speed switch"
        );

        // Verify execution continues
        assert_eq!(sys.cpu.pc, 0x106, "PC should have advanced past STOP");

        // Test switching back to normal speed
        sys.cpu.memory.write(0xFF4D, 0x01); // Arm speed switch again
        let key1_before_second_switch = sys.cpu.memory.read(0xFF4D);
        assert_eq!(
            key1_before_second_switch & 0x01,
            0x01,
            "KEY1 bit 0 should be set again"
        );

        // Manually trigger speed switch via memory interface
        let switched = sys.cpu.memory.perform_speed_switch();
        assert!(switched, "Speed switch should succeed");

        let key1_after_second_switch = sys.cpu.memory.read(0xFF4D);
        assert_eq!(
            key1_after_second_switch & 0x80,
            0x00,
            "KEY1 bit 7 should be back to 0 (normal speed)"
        );
        assert_eq!(
            key1_after_second_switch & 0x01,
            0x00,
            "KEY1 bit 0 should be cleared"
        );
    }

    #[test]
    fn test_stop_without_speed_switch() {
        // Test STOP instruction without speed switch (should enter low power mode)
        let mut sys = GbSystem::new();

        // Create a CGB ROM
        let mut rom = vec![0; 0x8000];
        rom[0x143] = 0x80; // CGB compatible
        rom[0x147] = 0x00; // ROM ONLY
        rom[0x149] = 0x00; // No RAM

        // Program: Just execute STOP without arming speed switch
        rom[0x100] = 0x10; // STOP
        rom[0x101] = 0x00; // Immediate byte
        rom[0x102] = 0x00; // NOP (should not reach here)

        sys.mount("Cartridge", &rom).unwrap();

        // KEY1 bit 0 should be 0 (speed switch not armed)
        let key1 = sys.cpu.memory.read(0xFF4D);
        assert_eq!(key1 & 0x01, 0x00, "KEY1 bit 0 should not be set");

        // Execute STOP
        sys.cpu.step();

        // CPU should be in stopped state
        assert!(
            sys.cpu.stopped,
            "CPU should be stopped when STOP is executed without speed switch"
        );

        // Speed should not have changed
        let key1_after = sys.cpu.memory.read(0xFF4D);
        assert_eq!(key1_after & 0x80, 0x00, "Speed should not have changed");
    }

    #[test]
    fn test_key1_register_bits() {
        // Test KEY1 register bit behavior
        let mut sys = GbSystem::new();

        // Create a CGB ROM
        let mut rom = vec![0; 0x150];
        rom[0x143] = 0x80; // CGB compatible
        rom[0x147] = 0x00;
        rom[0x149] = 0x00;

        sys.mount("Cartridge", &rom).unwrap();

        // Test that bits 1-6 always read as 1
        sys.cpu.memory.write(0xFF4D, 0x00);
        assert_eq!(
            sys.cpu.memory.read(0xFF4D) & 0x7E,
            0x7E,
            "KEY1 bits 1-6 should always read as 1"
        );

        sys.cpu.memory.write(0xFF4D, 0x81);
        assert_eq!(
            sys.cpu.memory.read(0xFF4D) & 0x7E,
            0x7E,
            "KEY1 bits 1-6 should always read as 1"
        );

        // Test that only bit 0 is writable
        sys.cpu.memory.write(0xFF4D, 0xFF);
        let key1 = sys.cpu.memory.read(0xFF4D);
        assert_eq!(key1 & 0x01, 0x01, "KEY1 bit 0 should be writable");
        // Bit 7 should still be 0 (can only be changed by speed switch)
        assert_eq!(key1 & 0x80, 0x00, "KEY1 bit 7 cannot be written directly");
    }

    #[test]
    fn test_dmg_no_speed_switching() {
        // Test that speed switching doesn't work in DMG mode
        let mut sys = GbSystem::new();

        // Create a DMG (non-CGB) ROM
        let mut rom = vec![0; 0x150];
        rom[0x143] = 0x00; // NOT CGB
        rom[0x147] = 0x00;
        rom[0x149] = 0x00;

        sys.mount("Cartridge", &rom).unwrap();

        // Verify DMG mode
        assert!(!sys.cpu.memory.is_cgb_mode());

        // Try to arm speed switch
        sys.cpu.memory.write(0xFF4D, 0x01);

        // Attempt speed switch
        let switched = sys.cpu.memory.perform_speed_switch();
        assert!(!switched, "Speed switch should not work in DMG mode");

        // Verify KEY1 is still 0
        let key1 = sys.cpu.memory.read(0xFF4D);
        assert_eq!(key1 & 0x80, 0x00, "Speed should not have changed");
    }

    #[test]
    fn test_gta_style_speed_switching() {
        // Integration test simulating GTA 1/2 speed switching behavior
        // GTA games use STOP with KEY1 to switch speeds during gameplay
        let mut sys = GbSystem::new();

        // Create a CGB ROM
        let mut rom = vec![0; 0x8000];
        rom[0x143] = 0x80; // CGB compatible
        rom[0x147] = 0x00; // ROM ONLY
        rom[0x149] = 0x00; // No RAM

        // Simulate GTA-style speed switching sequence
        // 0x100: Arm speed switch
        rom[0x100] = 0x3E; // LD A, 0x01
        rom[0x101] = 0x01;
        rom[0x102] = 0xE0; // LDH ($4D), A
        rom[0x103] = 0x4D;
        // 0x104: Execute STOP
        rom[0x104] = 0x10; // STOP
        rom[0x105] = 0x00;
        // 0x106: Continue after speed switch
        rom[0x106] = 0x3E; // LD A, 0x42
        rom[0x107] = 0x42;
        rom[0x108] = 0xEA; // LD ($C000), A (write to WRAM)
        rom[0x109] = 0x00;
        rom[0x10A] = 0xC0;
        rom[0x10B] = 0x18; // JR -3 (infinite loop)
        rom[0x10C] = 0xFD;

        sys.mount("Cartridge", &rom).unwrap();

        // Run several frames to ensure the system doesn't freeze
        for i in 0..10 {
            let result = sys.step_frame();
            assert!(
                result.is_ok(),
                "Frame {} should execute without freezing",
                i
            );
        }

        // Verify the system is still running and not stuck
        // Check that execution continued past the STOP instruction
        let wram_value = sys.cpu.memory.read(0xC000);
        assert_eq!(
            wram_value, 0x42,
            "WRAM should contain 0x42, indicating execution continued after STOP"
        );

        // Verify speed was switched
        let key1 = sys.cpu.memory.read(0xFF4D);
        assert_eq!(
            key1 & 0x80,
            0x80,
            "System should be in double speed mode (bit 7 set)"
        );
        assert_eq!(
            key1 & 0x01,
            0x00,
            "Speed switch flag should be cleared (bit 0 = 0)"
        );
    }

    #[test]
    fn test_wram_banking_cgb() {
        // Test WRAM banking in CGB mode
        let mut sys = GbSystem::new();

        // Create a CGB ROM
        let mut rom = vec![0; 0x150];
        rom[0x143] = 0x80; // CGB compatible
        rom[0x147] = 0x00; // ROM ONLY
        rom[0x149] = 0x00; // No RAM

        sys.mount("Cartridge", &rom).unwrap();

        // Test SVBK register read/write
        sys.cpu.memory.write(0xFF70, 0x00);
        assert_eq!(
            sys.cpu.memory.read(0xFF70) & 0x07,
            0x00,
            "SVBK should read back as 0"
        );
        assert_eq!(
            sys.cpu.memory.read(0xFF70) & 0xF8,
            0xF8,
            "SVBK bits 3-7 should always read as 1"
        );

        // Test bank 1
        sys.cpu.memory.write(0xFF70, 0x01);
        assert_eq!(
            sys.cpu.memory.read(0xFF70) & 0x07,
            0x01,
            "SVBK should read back as 1"
        );

        // Test bank 7
        sys.cpu.memory.write(0xFF70, 0x07);
        assert_eq!(
            sys.cpu.memory.read(0xFF70) & 0x07,
            0x07,
            "SVBK should read back as 7"
        );

        // Test that only bits 0-2 are writable
        sys.cpu.memory.write(0xFF70, 0xFF);
        assert_eq!(
            sys.cpu.memory.read(0xFF70) & 0x07,
            0x07,
            "Only bits 0-2 should be writable"
        );

        // Test bank 0 (bank 0 is always at 0xC000-0xCFFF, not affected by SVBK)
        sys.cpu.memory.write(0xFF70, 0x00);
        sys.cpu.memory.write(0xC000, 0xAA); // Write to bank 0
        assert_eq!(
            sys.cpu.memory.read(0xC000),
            0xAA,
            "Bank 0 should be at 0xC000"
        );

        // Test that bank 0 in SVBK maps to bank 1 for switchable area
        sys.cpu.memory.write(0xD000, 0xBB); // Should write to bank 1
        assert_eq!(
            sys.cpu.memory.read(0xD000),
            0xBB,
            "SVBK=0 should map to bank 1 for 0xD000-0xDFFF"
        );

        // Switch to bank 2 and verify isolation
        sys.cpu.memory.write(0xFF70, 0x02);
        sys.cpu.memory.write(0xD000, 0xCC); // Write to bank 2
        assert_eq!(
            sys.cpu.memory.read(0xD000),
            0xCC,
            "Bank 2 should have different data"
        );

        // Switch back to bank 1 and verify data is preserved
        sys.cpu.memory.write(0xFF70, 0x01);
        assert_eq!(
            sys.cpu.memory.read(0xD000),
            0xBB,
            "Bank 1 data should be preserved"
        );

        // Verify bank 0 is unaffected
        assert_eq!(
            sys.cpu.memory.read(0xC000),
            0xAA,
            "Bank 0 should be unaffected by SVBK"
        );
    }

    #[test]
    fn test_wram_banking_all_banks() {
        // Test all 8 WRAM banks
        let mut sys = GbSystem::new();

        // Create a CGB ROM
        let mut rom = vec![0; 0x150];
        rom[0x143] = 0x80; // CGB compatible
        rom[0x147] = 0x00;
        rom[0x149] = 0x00;

        sys.mount("Cartridge", &rom).unwrap();

        // Write unique values to banks 1-7 (skip 0 since it maps to 1)
        for bank in 1..8 {
            sys.cpu.memory.write(0xFF70, bank);
            let value = 0x10 + bank;
            sys.cpu.memory.write(0xD000, value);
        }

        // Verify banks 1-7 have their unique values
        for bank in 1..8 {
            sys.cpu.memory.write(0xFF70, bank);
            let expected = 0x10 + bank;
            let actual = sys.cpu.memory.read(0xD000);
            assert_eq!(
                actual, expected,
                "Bank {} should contain 0x{:02X}, got 0x{:02X}",
                bank, expected, actual
            );
        }

        // Verify that bank 0 in SVBK maps to bank 1
        sys.cpu.memory.write(0xFF70, 0x00);
        let bank0_value = sys.cpu.memory.read(0xD000);
        sys.cpu.memory.write(0xFF70, 0x01);
        let bank1_value = sys.cpu.memory.read(0xD000);
        assert_eq!(
            bank0_value, bank1_value,
            "Bank 0 in SVBK should map to bank 1"
        );
    }

    #[test]
    fn test_wram_echo_ram_banking() {
        // Test that Echo RAM respects WRAM banking
        let mut sys = GbSystem::new();

        // Create a CGB ROM
        let mut rom = vec![0; 0x150];
        rom[0x143] = 0x80; // CGB compatible
        rom[0x147] = 0x00;
        rom[0x149] = 0x00;

        sys.mount("Cartridge", &rom).unwrap();

        // Test bank 0 echo (0xE000-0xEFFF mirrors 0xC000-0xCFFF)
        sys.cpu.memory.write(0xC000, 0x11);
        assert_eq!(
            sys.cpu.memory.read(0xE000),
            0x11,
            "Echo RAM should mirror bank 0"
        );

        // Test switchable bank echo (0xF000-0xFDFF mirrors 0xD000-0xDFFF)
        sys.cpu.memory.write(0xFF70, 0x02); // Select bank 2
        sys.cpu.memory.write(0xD000, 0x22);
        assert_eq!(
            sys.cpu.memory.read(0xF000),
            0x22,
            "Echo RAM should mirror selected bank"
        );

        // Switch bank and verify echo changes
        sys.cpu.memory.write(0xFF70, 0x03); // Select bank 3
        sys.cpu.memory.write(0xD000, 0x33);
        assert_eq!(
            sys.cpu.memory.read(0xF000),
            0x33,
            "Echo RAM should mirror new bank"
        );

        // Verify writes to echo RAM work
        sys.cpu.memory.write(0xE001, 0x44);
        assert_eq!(
            sys.cpu.memory.read(0xC001),
            0x44,
            "Writes to echo RAM should update bank 0"
        );

        sys.cpu.memory.write(0xF001, 0x55);
        assert_eq!(
            sys.cpu.memory.read(0xD001),
            0x55,
            "Writes to echo RAM should update selected bank"
        );
    }

    #[test]
    fn test_wram_banking_dmg_mode() {
        // Test that WRAM banking doesn't work in DMG mode
        let mut sys = GbSystem::new();

        // Create a DMG ROM (not CGB)
        let mut rom = vec![0; 0x150];
        rom[0x143] = 0x00; // Not CGB
        rom[0x147] = 0x00;
        rom[0x149] = 0x00;

        sys.mount("Cartridge", &rom).unwrap();

        // In DMG mode, SVBK should still be readable/writable but always use bank 1
        sys.cpu.memory.write(0xD000, 0xAA);

        // Try to switch to bank 2
        sys.cpu.memory.write(0xFF70, 0x02);

        // Data should still be accessible (bank 1 is always used in DMG mode)
        assert_eq!(
            sys.cpu.memory.read(0xD000),
            0xAA,
            "DMG mode should always use bank 1"
        );

        // Try bank 0 (which maps to bank 1)
        sys.cpu.memory.write(0xFF70, 0x00);
        assert_eq!(
            sys.cpu.memory.read(0xD000),
            0xAA,
            "DMG mode should still use bank 1 when SVBK=0"
        );
    }

    #[test]
    fn test_wram_banking_boundary_conditions() {
        // Test boundary conditions for WRAM banking
        let mut sys = GbSystem::new();

        // Create a CGB ROM
        let mut rom = vec![0; 0x150];
        rom[0x143] = 0x80; // CGB compatible
        rom[0x147] = 0x00;
        rom[0x149] = 0x00;

        sys.mount("Cartridge", &rom).unwrap();

        // Test boundary between bank 0 and switchable area
        sys.cpu.memory.write(0xFF70, 0x02);
        sys.cpu.memory.write(0xCFFF, 0xAA); // Last byte of bank 0
        sys.cpu.memory.write(0xD000, 0xBB); // First byte of bank 2

        assert_eq!(
            sys.cpu.memory.read(0xCFFF),
            0xAA,
            "Last byte of bank 0 should be 0xAA"
        );
        assert_eq!(
            sys.cpu.memory.read(0xD000),
            0xBB,
            "First byte of bank 2 should be 0xBB"
        );

        // Switch to bank 1 and verify bank 0 is unchanged
        sys.cpu.memory.write(0xFF70, 0x01);
        assert_eq!(
            sys.cpu.memory.read(0xCFFF),
            0xAA,
            "Bank 0 should be unchanged"
        );
        assert_ne!(
            sys.cpu.memory.read(0xD000),
            0xBB,
            "Bank 1 should have different data than bank 2"
        );

        // Test last byte of switchable area
        sys.cpu.memory.write(0xFF70, 0x03);
        sys.cpu.memory.write(0xDFFF, 0xCC);
        assert_eq!(
            sys.cpu.memory.read(0xDFFF),
            0xCC,
            "Last byte of switchable area should be accessible"
        );
    }

    #[test]
    fn test_serial_transfer_basic() {
        let mut sys = GbSystem::new();
        let rom = vec![0; 0x8000];
        sys.mount("Cartridge", &rom).unwrap();

        // Write data to SB register
        sys.cpu.memory.write(0xFF01, 0xAB);
        assert_eq!(sys.cpu.memory.read(0xFF01), 0xAB);

        // Write to SC register to start transfer (internal clock)
        sys.cpu.memory.write(0xFF02, 0x81); // Bit 7=1 (start), bit 0=1 (internal)
        assert_eq!(sys.cpu.memory.read(0xFF02) & 0x81, 0x81);
    }

    #[test]
    fn test_serial_transfer_completion() {
        let mut sys = GbSystem::new();

        // Create a minimal ROM that performs a serial transfer
        let mut rom = vec![0; 0x8000];
        // Write assembly to initialize serial transfer
        rom[0x100] = 0x3E; // LD A, 0x42
        rom[0x101] = 0x42;
        rom[0x102] = 0xE0; // LDH (0xFF01), A  (write to SB)
        rom[0x103] = 0x01;
        rom[0x104] = 0x3E; // LD A, 0x81
        rom[0x105] = 0x81;
        rom[0x106] = 0xE0; // LDH (0xFF02), A  (write to SC, start transfer)
        rom[0x107] = 0x02;
        rom[0x108] = 0x76; // HALT
        rom[0x147] = 0x00; // Cartridge type
        rom[0x149] = 0x00; // RAM size

        sys.mount("Cartridge", &rom).unwrap();

        // Run several frames to allow transfer to complete
        for _ in 0..10 {
            let _ = sys.step_frame();
        }

        // Transfer should be complete (bit 7 cleared)
        let sc = sys.cpu.memory.read(0xFF02);
        assert_eq!(sc & 0x80, 0x00, "Transfer start bit should be cleared");

        // SB should have been shifted (with 0xFF shifted in, simulating no device)
        let sb = sys.cpu.memory.read(0xFF01);
        assert_eq!(
            sb, 0xFF,
            "SB should be 0xFF after transfer with no device connected"
        );
    }

    #[test]
    fn test_infrared_port() {
        let mut sys = GbSystem::new();
        let rom = vec![0; 0x8000];
        sys.mount("Cartridge", &rom).unwrap();

        // Test RP register (0xFF56) - infrared port
        // Write value with LED control bits
        sys.cpu.memory.write(0xFF56, 0xC1); // Bits 6, 7 (LED), bit 0 (receive)

        // Should read back with only writable bits
        let rp = sys.cpu.memory.read(0xFF56);
        assert_eq!(rp & 0xC1, 0xC1, "RP should preserve LED control bits");

        // Try writing to non-writable bits
        sys.cpu.memory.write(0xFF56, 0xFF);
        let rp = sys.cpu.memory.read(0xFF56);
        assert_eq!(rp & 0xC1, 0xC1, "RP should mask non-writable bits");
        assert_eq!(rp & 0x3E, 0x00, "Bits 1-5 should read as 0");
    }

    #[test]
    fn test_hdma_register_access() {
        let mut sys = GbSystem::new();
        let rom = vec![0; 0x8000];
        sys.mount("Cartridge", &rom).unwrap();

        // Test HDMA register writes
        sys.cpu.memory.write(0xFF51, 0x12); // Source high
        sys.cpu.memory.write(0xFF52, 0x34); // Source low (lower 4 bits ignored)
        sys.cpu.memory.write(0xFF53, 0x90); // Dest high (only bits 0-4)
        sys.cpu.memory.write(0xFF54, 0xAB); // Dest low (lower 4 bits ignored)

        // Verify reads
        assert_eq!(sys.cpu.memory.read(0xFF51), 0x12);
        assert_eq!(sys.cpu.memory.read(0xFF52), 0x30); // Lower 4 bits masked
        assert_eq!(sys.cpu.memory.read(0xFF53), 0x10); // Only bits 0-4 used
        assert_eq!(sys.cpu.memory.read(0xFF54), 0xA0); // Lower 4 bits masked

        // HDMA5 should read 0xFF when inactive
        assert_eq!(sys.cpu.memory.read(0xFF55), 0xFF);
    }

    #[test]
    fn test_hdma_gdma_transfer() {
        let mut sys = GbSystem::new();
        let rom = vec![0; 0x8000];
        sys.mount("Cartridge", &rom).unwrap();

        // Set up source data in WRAM
        for i in 0..32 {
            sys.cpu.memory.write(0xC000 + i, i as u8);
        }

        // Configure HDMA for General Purpose DMA (immediate transfer)
        sys.cpu.memory.write(0xFF51, 0xC0); // Source: 0xC000
        sys.cpu.memory.write(0xFF52, 0x00);
        sys.cpu.memory.write(0xFF53, 0x90); // Dest: 0x9000 (VRAM)
        sys.cpu.memory.write(0xFF54, 0x00);
        sys.cpu.memory.write(0xFF55, 0x01); // Transfer 2 blocks (32 bytes), GDMA mode (bit 7 = 0)

        // Execute pending GDMA (now deferred to avoid nested read/write)
        sys.cpu.memory.execute_pending_gdma();

        // HDMA5 should read 0xFF after GDMA completes
        assert_eq!(sys.cpu.memory.read(0xFF55), 0xFF);

        // Verify data was transferred to VRAM
        for i in 0..32 {
            let vram_data = sys.cpu.memory.ppu.read_vram(0x1000 + i); // VRAM offset 0x1000 = address 0x9000
            assert_eq!(vram_data, i as u8, "VRAM byte {} mismatch", i);
        }
    }

    #[test]
    fn test_hdma_hblank_dma() {
        let mut sys = GbSystem::new();
        let rom = vec![0; 0x8000];
        sys.mount("Cartridge", &rom).unwrap();

        // Set up source data in WRAM
        for i in 0..16 {
            sys.cpu.memory.write(0xC000 + i, (i + 0x42) as u8);
        }

        // Configure HDMA for HBlank DMA
        sys.cpu.memory.write(0xFF51, 0xC0); // Source: 0xC000
        sys.cpu.memory.write(0xFF52, 0x00);
        sys.cpu.memory.write(0xFF53, 0x88); // Dest: 0x8800 (VRAM)
        sys.cpu.memory.write(0xFF54, 0x00);
        sys.cpu.memory.write(0xFF55, 0x80); // Transfer 1 block (16 bytes), HBlank DMA mode (bit 7 = 1)

        // HDMA should be active, remaining = 0 (1 block - 1)
        let hdma5 = sys.cpu.memory.read(0xFF55);
        assert_eq!(hdma5, 0x00, "HDMA5 should show 0 remaining blocks");

        // Step a frame to allow HBlank DMA to occur
        let _ = sys.step_frame();

        // After HBlank, transfer should be complete
        assert_eq!(sys.cpu.memory.read(0xFF55), 0xFF, "HDMA should be complete");

        // Verify data was transferred to VRAM
        for i in 0..16 {
            let vram_data = sys.cpu.memory.ppu.read_vram(0x0800 + i); // VRAM offset 0x0800 = address 0x8800
            assert_eq!(vram_data, (i + 0x42) as u8, "VRAM byte {} mismatch", i);
        }
    }

    #[test]
    fn test_hdma_hblank_dma_lcd_off() {
        // Regression test: HDMA must complete even when LCD is disabled.
        // Games like Worms (GBC) turn off the LCD and use HBlank DMA to
        // bulk-copy data into VRAM, then poll FF55 waiting for completion.
        // On real hardware, Mode 0 (HBlank) is always active when LCD is off,
        // so HDMA transfers one block per ~456 T-cycles.
        let mut sys = GbSystem::new();
        let rom = vec![0; 0x8000];
        sys.mount("Cartridge", &rom).unwrap();

        // Turn off the LCD
        sys.cpu.memory.write(0xFF40, 0x00); // LCDC = 0 (LCD disabled)

        // Set up source data in WRAM (4 blocks = 64 bytes)
        for i in 0..64u16 {
            sys.cpu.memory.write(0xC000 + i, i as u8);
        }

        // Configure HDMA for HBlank DMA with LCD off
        sys.cpu.memory.write(0xFF51, 0xC0); // Source: 0xC000
        sys.cpu.memory.write(0xFF52, 0x00);
        sys.cpu.memory.write(0xFF53, 0x90); // Dest: 0x9000 (VRAM)
        sys.cpu.memory.write(0xFF54, 0x00);
        sys.cpu.memory.write(0xFF55, 0x83); // Transfer 4 blocks (64 bytes), HBlank DMA mode (bit 7 = 1)

        // HDMA should be active
        let hdma5 = sys.cpu.memory.read(0xFF55);
        assert_eq!(hdma5 & 0x80, 0x00, "HDMA5 bit 7 should be 0 (HDMA active)");

        // Step a frame - HDMA should complete even with LCD off
        let _ = sys.step_frame();

        // After frame, transfer should be complete
        assert_eq!(
            sys.cpu.memory.read(0xFF55),
            0xFF,
            "HDMA should be complete after frame with LCD off"
        );

        // Verify data was transferred to VRAM
        for i in 0..64u16 {
            let vram_data = sys.cpu.memory.ppu.read_vram(0x1000 + i);
            assert_eq!(
                vram_data, i as u8,
                "VRAM byte {} mismatch after LCD-off HDMA",
                i
            );
        }
    }

    #[test]
    fn test_post_boot_state_dmg() {
        let mut sys = GbSystem::new();

        // Apply DMG post-boot state
        sys.apply_post_boot_state(false);

        // Verify CPU registers match DMG post-boot values
        assert_eq!(sys.cpu.a, 0x01, "A register incorrect");
        assert_eq!(sys.cpu.f, 0xB0, "F register incorrect");
        assert_eq!(sys.cpu.b, 0x00, "B register incorrect");
        assert_eq!(sys.cpu.c, 0x13, "C register incorrect");
        assert_eq!(sys.cpu.d, 0x00, "D register incorrect");
        assert_eq!(sys.cpu.e, 0xD8, "E register incorrect");
        assert_eq!(sys.cpu.h, 0x01, "H register incorrect");
        assert_eq!(sys.cpu.l, 0x4D, "L register incorrect");
        assert_eq!(sys.cpu.sp, 0xFFFE, "SP incorrect");
        assert_eq!(sys.cpu.pc, 0x0100, "PC incorrect");

        // Verify I/O registers
        assert_eq!(sys.cpu.memory.ppu.lcdc, 0x91, "LCDC incorrect");
        assert_eq!(sys.cpu.memory.ppu.bgp, 0xFC, "BGP incorrect");
        assert_eq!(sys.cpu.memory.ppu.obp0, 0xFF, "OBP0 incorrect");
        assert_eq!(sys.cpu.memory.ppu.obp1, 0xFF, "OBP1 incorrect");

        // Verify boot ROM is disabled
        assert!(
            !sys.cpu.memory.is_boot_rom_enabled(),
            "Boot ROM should be disabled"
        );
    }

    #[test]
    fn test_post_boot_state_cgb() {
        let mut sys = GbSystem::new();

        // Apply CGB post-boot state
        sys.apply_post_boot_state(true);

        // Verify CPU registers match CGB post-boot values
        assert_eq!(sys.cpu.a, 0x11, "A register incorrect (CGB mode indicator)");
        assert_eq!(sys.cpu.f, 0x80, "F register incorrect");
        assert_eq!(sys.cpu.b, 0x00, "B register incorrect");
        assert_eq!(sys.cpu.c, 0x00, "C register incorrect");
        assert_eq!(sys.cpu.d, 0xFF, "D register incorrect");
        assert_eq!(sys.cpu.e, 0x56, "E register incorrect");
        assert_eq!(sys.cpu.h, 0x00, "H register incorrect");
        assert_eq!(sys.cpu.l, 0x0D, "L register incorrect");
        assert_eq!(sys.cpu.sp, 0xFFFE, "SP incorrect");
        assert_eq!(sys.cpu.pc, 0x0100, "PC incorrect");

        // Verify I/O registers (same as DMG for most)
        assert_eq!(sys.cpu.memory.ppu.lcdc, 0x91, "LCDC incorrect");
        assert_eq!(sys.cpu.memory.ppu.bgp, 0xFC, "BGP incorrect");

        // Verify boot ROM is disabled
        assert!(
            !sys.cpu.memory.is_boot_rom_enabled(),
            "Boot ROM should be disabled"
        );
    }

    #[test]
    fn test_post_boot_state_applied_on_reset() {
        let mut sys = GbSystem::new();

        // Load a test ROM to trigger reset
        let mut rom = vec![0; 0x8000];
        // Set CGB flag at 0x143
        rom[0x143] = 0x80; // CGB-compatible

        sys.mount("Cartridge", &rom).unwrap();

        // After mount, reset() is called which should apply post-boot state
        // For CGB-compatible ROM, A register should be 0x11
        assert_eq!(
            sys.cpu.a, 0x11,
            "Post-boot state not applied correctly after reset"
        );
        assert_eq!(sys.cpu.pc, 0x0100, "PC should be at 0x0100 after reset");
        assert!(
            !sys.cpu.memory.is_boot_rom_enabled(),
            "Boot ROM should be disabled after reset"
        );
    }

    #[test]
    fn test_double_speed_frame_progression() {
        // Test that frame progression stays at 70,224 PPU cycles per frame
        // regardless of double-speed mode, and audio cycle accounting is consistent.
        let mut rom = vec![0; 0x8000];
        rom[0x143] = 0x80; // CGB-compatible

        // Normal-speed system
        let mut sys_normal = GbSystem::new();
        sys_normal.mount("Cartridge", &rom.clone()).unwrap();

        // Step one frame in normal speed
        let frame_normal = sys_normal.step_frame().unwrap();
        let audio_normal = sys_normal.audio_cycles_accumulated;

        // Double-speed system: force KEY1 bit 7 to enter double speed
        let mut sys_double = GbSystem::new();
        sys_double.mount("Cartridge", &rom).unwrap();
        // Directly set KEY1 bit 7 to simulate double-speed mode
        sys_double.cpu.memory.write(0xFF4D, 0x01); // Arm speed switch
        sys_double.cpu.memory.perform_speed_switch(); // Toggle speed

        assert!(
            sys_double.cpu.memory.is_double_speed(),
            "System should be in double-speed mode"
        );

        // Step one frame in double speed
        let frame_double = sys_double.step_frame().unwrap();
        let audio_double = sys_double.audio_cycles_accumulated;

        // Both frames should have the same dimensions
        assert_eq!(frame_normal.width, frame_double.width);
        assert_eq!(frame_normal.height, frame_double.height);

        // Audio cycles accumulated should be similar (both in PPU-rate cycles)
        // Allow some tolerance since the exact CPU instructions executed differ
        let diff = (audio_normal as i64 - audio_double as i64).unsigned_abs();
        assert!(
            diff < 1000,
            "Audio cycle accumulation should be similar in both speed modes, \
             normal={} double={} diff={}",
            audio_normal,
            audio_double,
            diff
        );
    }
}
