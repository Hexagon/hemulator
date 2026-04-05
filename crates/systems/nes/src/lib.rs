//! NES (Nintendo Entertainment System) emulation implementation.
//!
//! This module provides a complete NES system emulator using the reusable 6502 CPU core
//! from `emu_core`, along with NES-specific components:
//!
//! - **CPU**: Ricoh 2A03 (6502 without decimal mode)
//! - **PPU**: 2C02 Picture Processing Unit with scanline-based rendering
//! - **APU**: Audio Processing Unit with 2 pulse channels (expandable)
//! - **Mappers**: 14 cartridge mappers covering ~90%+ of NES games
//! - **Controllers**: Standard NES controller support (D-pad, A, B, Select, Start)
//! - **Timing**: Both NTSC (1.789773 MHz) and PAL (1.662607 MHz) modes
//!
//! ## Supported Mappers
//!
//! - **0 (NROM)**: No banking, 16KB or 32KB PRG ROM
//! - **1 (MMC1/SxROM)**: Switchable PRG/CHR banks, various modes
//! - **2 (UxROM)**: 16KB switchable + 16KB fixed PRG banks
//! - **3 (CNROM)**: Switchable CHR banks only
//! - **4 (MMC3/TxROM)**: Advanced banking with scanline IRQ counter
//! - **7 (AxROM)**: 32KB switchable PRG banks, single-screen mirroring
//! - **9 (MMC2/PxROM)**: Latch-based CHR switching (Punch-Out!!)
//! - **10 (MMC4/FxROM)**: Similar to MMC2 (Fire Emblem)
//! - **11 (Color Dreams)**: Simple PRG/CHR banking
//! - **34 (BNROM)**: 32KB switchable PRG banks
//! - **66 (GxROM)**: Combined PRG/CHR banking
//! - **71 (Camerica)**: 16KB switchable PRG banks
//! - **79 (NINA-03/06)**: AVE mapper with PRG/CHR banking
//! - **206 (Namco 118)**: Variant of MMC3 without IRQ support
//!
//! ## PPU Features
//!
//! - 256x240 resolution
//! - 64-color master palette
//! - 8 background palettes (4 colors each)
//! - 8 sprite palettes (4 colors each)
//! - Scrolling with nametable switching
//! - Sprite rendering (8x8 and 8x16 modes)
//! - Sprite priority and flipping
//! - Sprite 0 hit detection (basic)
//! - Scanline-based rendering (handles mid-frame register changes)
//!
//! ## APU Features
//!
//! - 2 pulse channels with duty cycle control
//! - **Sweep units** for frequency modulation (pitch bending) on both pulse channels
//! - Triangle channel with 32-step waveform
//! - Noise channel with pseudo-random LFSR
//! - DMC channel with full sample playback support
//!   - Memory reads from CPU address space
//!   - IRQ generation on completion
//!   - Loop support
//! - Length counter and envelope support
//! - Frame counter (4-step and 5-step modes)
//! - APU IRQ support (frame counter and DMC)
//! - 44.1 kHz audio output with non-linear mixing
//!
//! ## Timing Model
//!
//! The emulator uses **cycle-accurate PPU execution** for precise NMI/VBlank timing:
//!
//! - **NTSC**: ~29,780 CPU cycles per frame (~60.1 Hz)
//! - **PAL**: ~33,247 CPU cycles per frame (~50.0 Hz)
//! - **PPU Clock**: 3x CPU clock (3 PPU dots per CPU cycle)
//! - **VBlank**: Automatically set at scanline 241, dot 1
//! - **NMI**: Triggered precisely when VBlank flag set (if NMI enabled)
//! - **Scanline IRQs**: Synthesized for mappers like MMC3
//!
//! **Cycle-Accurate Features:**
//! - VBlank flag set at exact cycle (scanline 241, dot 1)
//! - $2002 read race condition handling (reading during VBlank transition)
//! - NMI suppression when reading $2002 at VBlank start
//! - Sprite flags cleared at scanline 261, dot 1 (pre-render scanline)
//! - Odd frame cycle skip (scanline 0, dot 0 -> dot 1 when rendering enabled)
//!
//! This provides excellent accuracy for games with tight timing requirements
//! while maintaining good performance.

#![allow(clippy::upper_case_acronyms)]
#![allow(clippy::unnecessary_cast)]
#![allow(clippy::manual_range_contains)]
#![allow(clippy::let_and_return)]

mod apu;
mod bus;
mod cartridge;
mod cpu;
mod debugger;
mod mappers;
pub mod ppu;
pub mod ppu_renderer;
#[cfg(feature = "opengl")]
pub mod ppu_renderer_opengl;
pub mod rom_db;

use crate::bus::Bus;
use crate::cartridge::Mirroring;
use bus::NesBus;
use cpu::NesCpu;
use emu_core::debug::Debugger;
use emu_core::logging::{log, LogCategory, LogLevel};
#[cfg(feature = "opengl")]
use emu_core::renderer::Renderer;
use emu_core::{apu::TimingMode, types::Frame, MountPointInfo, System};
use ppu::Ppu;
use ppu_renderer::{NesPpuRenderer, SoftwareNesPpuRenderer};
use std::collections::HashMap;
use std::rc::Rc;

/// Debug information for the NES system.
///
/// Provides runtime information about the loaded cartridge and system state
/// for display in debug overlays.
#[derive(Debug, Clone)]
pub struct DebugInfo {
    /// Current timing mode (NTSC or PAL)
    pub timing_mode: TimingMode,
    /// Human-readable mapper name (e.g., "MMC3/TxROM")
    pub mapper_name: String,
    /// iNES mapper number (0-4095 for iNES 2.0)
    pub mapper_number: u16,
    /// Number of 16KB PRG banks
    pub prg_banks: usize,
    /// Number of 8KB CHR banks (0 for CHR-RAM)
    pub chr_banks: usize,
}

/// Cartridge information for the inspector tab.
///
/// Contains metadata about the loaded cartridge including ROM database overrides.
#[derive(Debug, Clone)]
pub struct CartridgeInfo {
    /// Mapper number being used (after any DB overrides)
    pub mapper: u16,
    /// Submapper number from iNES 2.0 header (0-15)
    pub submapper: u8,
    /// Human-readable mapper name
    pub mapper_name: String,
    /// Mirroring mode being used (after any DB overrides)
    pub mirroring: String,
    /// Timing mode (NTSC or PAL)
    pub timing: TimingMode,
    /// CRC32 checksum of the entire ROM file (including header)
    pub crc32: u32,
    /// PRG ROM size in bytes
    pub prg_size: usize,
    /// CHR ROM size in bytes (0 for CHR-RAM)
    pub chr_size: usize,
    /// Mapper number from iNES header (before DB override)
    pub header_mapper: u16,
    /// Submapper number from iNES 2.0 header
    pub header_submapper: u8,
    /// Mirroring mode from iNES header (before DB override)
    pub header_mirroring: String,
    /// Whether mapper was overridden by ROM database
    pub db_mapper_override: bool,
    /// Whether mirroring was overridden by ROM database
    pub db_mirroring_override: bool,
    /// Board name from ROM database (if available)
    pub board_name: Option<String>,
}

/// Tile viewer data for debugging PPU graphics.
///
/// Contains CHR data, palettes, and PPU state for visualization.
#[derive(Debug, Clone)]
pub struct TileViewerData {
    /// CHR data (pattern tables) - typically 8KB for NES
    /// Uses Rc to avoid cloning the full CHR data on every call
    pub chr_data: Rc<Vec<u8>>,
    /// Palette data - 32 bytes (4 colors x 8 palettes)
    pub palette: Vec<u8>,
    /// NES master palette for color lookup (64 colors, RGB as 0xFFRRGGBB)
    pub master_palette: Vec<u32>,
    /// OAM data - 256 bytes (64 sprites x 4 bytes each)
    pub oam: Vec<u8>,
    /// VRAM data - 2KB nametables
    pub vram: Vec<u8>,
    /// Whether this is CHR-RAM (true) or CHR-ROM (false)
    pub chr_is_ram: bool,
    /// Current PPUCTRL value
    pub ppuctrl: u8,
    /// Current PPUMASK value
    pub ppumask: u8,
    /// Current X scroll value
    pub scroll_x: u8,
    /// Current Y scroll value
    pub scroll_y: u8,
    /// Current mirroring mode as string
    pub mirroring: String,
    /// Current sprite 0 hit status (for the inspector)
    pub sprite0_status: ppu::Sprite0Status,
    /// Current sprite 0 configuration (for the inspector)
    pub sprite0_config: ppu::Sprite0Config,
}

/// Program counter hotspot tracking for performance analysis.
///
/// Tracks the most frequently executed addresses to help identify
/// performance bottlenecks and infinite loops.
#[derive(Debug, Clone, Copy, Default)]
pub struct PcHotspot {
    /// Program counter address
    pub pc: u16,
    /// Number of times this address was executed in the frame
    pub count: u16,
}

/// Runtime statistics for debugging and performance monitoring.
///
/// Collected each frame and available via `get_runtime_stats()`.
#[derive(Debug, Clone, Copy, Default)]
pub struct RuntimeStats {
    /// Current frame number (wraps at u64::MAX)
    pub frame_index: u64,
    /// Number of CPU instructions executed this frame
    pub cpu_steps: u32,
    /// Total CPU cycles used this frame
    pub cpu_cycles: u32,
    /// Number of IRQ interrupts fired this frame
    pub irqs: u32,
    /// Number of NMI interrupts fired this frame
    pub nmis: u32,
    /// Number of MMC3 A12 rising edges this frame (for IRQ timing)
    pub mmc3_a12_edges: u32,
    /// Current PPUCTRL register value
    pub ppu_ctrl: u8,
    /// Current PPUMASK register value
    pub ppu_mask: u8,
    /// Current VBlank flag state
    pub ppu_vblank: bool,
    /// Current program counter
    pub pc: u16,
    /// Reset vector ($FFFC)
    pub vec_reset: u16,
    /// NMI vector ($FFFA)
    pub vec_nmi: u16,
    /// IRQ vector ($FFFE)
    pub vec_irq: u16,
    /// Top 3 most frequently executed addresses this frame
    pub pc_hotspots: [PcHotspot; 3],
}

/// NES system implementation.
///
/// Combines the 6502 CPU, PPU, APU, and cartridge mappers into a complete
/// NES emulator. Implements the `System` trait from `emu_core` for integration
/// with the frontend.
#[derive(Debug)]
pub struct NesSystem {
    cpu: NesCpu,
    timing: TimingMode,
    cartridge_loaded: bool,
    frame_index: u64,
    last_stats: RuntimeStats,
    renderer: Box<dyn NesPpuRenderer>,
    /// Instruction tracer for debugging
    instruction_tracer: emu_core::instruction_tracer::InstructionTracer,
    /// Breakpoint manager for debugging
    breakpoint_manager: emu_core::breakpoints::BreakpointManager,
    /// Total CPU cycles executed since reset
    total_cycles: u64,
    /// Cartridge information for inspector
    cartridge_info: Option<CartridgeInfo>,
}

impl NesSystem {
    /// Set controller 0 or 1 button state (bits 0..7 correspond to controller buttons).
    pub fn set_controller(&mut self, idx: usize, state: u8) {
        if let Some(b) = self.cpu.bus_mut() {
            b.set_controller(idx, state);
        }
    }

    /// Set the PPU renderer
    pub fn set_renderer(&mut self, renderer: Box<dyn NesPpuRenderer>) {
        self.renderer = renderer;
    }

    /// Get audio samples from the APU
    pub fn get_audio_samples(&mut self, count: usize) -> Vec<i16> {
        if let Some(b) = self.cpu.bus_mut() {
            b.apu.generate_samples(count)
        } else {
            vec![0; count]
        }
    }

    /// Set timing mode (NTSC/PAL)
    pub fn set_timing(&mut self, timing: TimingMode) {
        self.timing = timing;
        if let Some(b) = self.cpu.bus_mut() {
            b.apu.set_timing(timing);
        }
    }

    /// Get current timing mode
    pub fn timing(&self) -> TimingMode {
        self.timing
    }

    /// Get debug information for the GUI overlay.
    pub fn get_debug_info(&self) -> DebugInfo {
        let mut mapper_name = "Unknown".to_string();
        let mut mapper_number = 0u16;
        let mut prg_banks = 0;
        let mut chr_banks = 0;

        if let Some(b) = self.cpu.bus() {
            if let Some(num) = b.mapper_number() {
                mapper_number = num;
                mapper_name = match mapper_number {
                    0 => "NROM".to_string(),
                    1 => "MMC1/SxROM".to_string(),
                    2 => "UxROM".to_string(),
                    3 => "CNROM".to_string(),
                    4 => "MMC3/TxROM".to_string(),
                    7 => "AxROM".to_string(),
                    9 => "MMC2/PxROM".to_string(),
                    10 => "MMC4/FxROM".to_string(),
                    11 => "Color Dreams".to_string(),
                    _ => format!("Mapper {}", mapper_number),
                };
                prg_banks = (b.prg_rom_size() / 16384).max(1); // 16KB banks
            }

            chr_banks = if b.ppu.chr.is_empty() {
                0 // CHR-RAM
            } else {
                (b.ppu.chr.len() / 8192).max(1) // 8KB banks
            };
        }

        DebugInfo {
            timing_mode: self.timing,
            mapper_name,
            mapper_number,
            prg_banks,
            chr_banks,
        }
    }

    /// Get tile viewer data for debugging PPU graphics.
    ///
    /// Returns CHR data, palette, and PPU state for visualization in a tile viewer.
    pub fn get_tile_viewer_data(&self) -> TileViewerData {
        if let Some(b) = self.cpu.bus() {
            let mirroring_str = format!("{:?}", b.ppu.get_mirroring());
            TileViewerData {
                chr_data: Rc::new(b.ppu.chr.clone()),
                palette: b.ppu.palette.to_vec(),
                master_palette: Ppu::get_master_palette(),
                oam: b.ppu.oam.to_vec(),
                vram: b.ppu.vram.to_vec(),
                chr_is_ram: b.ppu.chr_is_ram(),
                ppuctrl: b.ppu.ctrl(),
                ppumask: b.ppu.mask(),
                scroll_x: b.ppu.scroll_x(),
                scroll_y: b.ppu.scroll_y(),
                mirroring: mirroring_str,
                sprite0_status: b.ppu.sprite0_status(),
                sprite0_config: b.ppu.sprite0_config.clone(),
            }
        } else {
            // Return empty data if no bus is available
            TileViewerData {
                chr_data: Rc::new(Vec::new()),
                palette: Vec::new(),
                master_palette: Ppu::get_master_palette(),
                oam: Vec::new(),
                vram: Vec::new(),
                chr_is_ram: false,
                ppuctrl: 0,
                ppumask: 0,
                scroll_x: 0,
                scroll_y: 0,
                mirroring: "Unknown".to_string(),
                sprite0_status: ppu::Sprite0Status::default(),
                sprite0_config: ppu::Sprite0Config::default(),
            }
        }
    }

    /// Get the current sprite 0 hit configuration.
    pub fn get_sprite0_config(&self) -> ppu::Sprite0Config {
        self.cpu
            .bus()
            .map(|b| b.ppu.sprite0_config.clone())
            .unwrap_or_default()
    }

    /// Update the sprite 0 hit configuration.
    pub fn set_sprite0_config(&mut self, config: ppu::Sprite0Config) {
        if let Some(b) = self.cpu.bus_mut() {
            b.ppu.sprite0_config = config;
        }
    }

    /// Get cartridge information for the inspector tab.
    ///
    /// Returns metadata about the loaded cartridge including ROM database overrides.
    pub fn get_cartridge_info(&self) -> Option<CartridgeInfo> {
        self.cartridge_info.clone()
    }

    /// Get runtime stats for debugging / overlays.
    pub fn get_runtime_stats(&self) -> RuntimeStats {
        self.last_stats
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
        let pc = self.cpu.pc() as u32;
        if self.breakpoint_manager.should_break_execute(pc) {
            Some(pc)
        } else {
            None
        }
    }

    /// Enable OpenGL hardware rendering (requires OpenGL feature)
    /// This should be called from the frontend after obtaining a GL context
    #[cfg(feature = "opengl")]
    pub fn enable_opengl_renderer(&mut self, gl: glow::Context) -> Result<(), String> {
        use crate::ppu_renderer_opengl::OpenGLNesPpuRenderer;

        let mut new_renderer = Box::new(OpenGLNesPpuRenderer::new(gl, 256, 240)?);

        // Initialize to black
        new_renderer.clear(0x00000000);

        // Replace the software renderer with OpenGL renderer
        self.renderer = new_renderer;

        log(LogCategory::Stubs, LogLevel::Info, || {
            "NES PPU switched to OpenGL hardware renderer (256x240)".to_string()
        });

        Ok(())
    }

    /// Get the name of the current renderer
    pub fn renderer_name(&self) -> &str {
        self.renderer.name()
    }
}

impl Default for NesSystem {
    fn default() -> Self {
        // create PPU with empty CHR and NesBus and attach to CPU
        let mut cpu = NesCpu::new();
        cpu.reset();
        let ppu = Ppu::new(vec![], Mirroring::Vertical, TimingMode::Ntsc);
        let bus = NesBus::new(ppu);
        cpu.set_bus(bus);
        Self {
            cpu,
            timing: TimingMode::Ntsc,
            cartridge_loaded: false,
            frame_index: 0,
            last_stats: RuntimeStats::default(),
            renderer: Box::new(SoftwareNesPpuRenderer::new()),
            instruction_tracer: emu_core::instruction_tracer::InstructionTracer::new(),
            breakpoint_manager: emu_core::breakpoints::BreakpointManager::new(),
            total_cycles: 0,
            cartridge_info: None,
        }
    }
}

impl NesSystem {
    /// Common cartridge setup logic
    fn setup_cartridge(&mut self, cart: cartridge::Cartridge) -> Result<(), std::io::Error> {
        // Build cartridge info for inspector before moving the cart
        let mapper_name = match cart.mapper {
            0 => "NROM".to_string(),
            1 => "MMC1/SxROM".to_string(),
            2 => "UxROM".to_string(),
            3 => "CNROM".to_string(),
            4 => "MMC3/TxROM".to_string(),
            7 => "AxROM".to_string(),
            9 => "MMC2/PxROM".to_string(),
            10 => "MMC4/FxROM".to_string(),
            11 => "Color Dreams".to_string(),
            34 => "BNROM".to_string(),
            66 => "GxROM".to_string(),
            71 => "Camerica".to_string(),
            79 => "NINA-03/06".to_string(),
            206 => "Namco 118".to_string(),
            _ => format!("Mapper {}", cart.mapper),
        };

        self.cartridge_info = Some(CartridgeInfo {
            mapper: cart.mapper,
            submapper: cart.submapper,
            mapper_name,
            mirroring: format!("{:?}", cart.mirroring),
            timing: cart.timing,
            crc32: cart.crc32,
            prg_size: cart.prg_rom.len(),
            chr_size: cart.chr_rom.len(),
            header_mapper: cart.header_mapper,
            header_submapper: cart.header_submapper,
            header_mirroring: format!("{:?}", cart.header_mirroring),
            db_mapper_override: cart.db_mapper_override,
            db_mirroring_override: cart.db_mirroring_override,
            board_name: cart.board_name.clone(),
        });

        // Set timing mode from cartridge
        self.timing = cart.timing;

        // Derive the reset vector from the last PRG bank (mirrors hardware vectors).
        if cart.prg_rom.len() < 0x2000 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "PRG ROM too small",
            ));
        }
        let last_bank = cart.prg_rom.len() - 0x2000;
        let reset_lo = cart.prg_rom.get(last_bank + 0x1FFC).copied().unwrap_or(0) as u16;
        let reset_hi = cart.prg_rom.get(last_bank + 0x1FFD).copied().unwrap_or(0) as u16;
        self.cpu.set_pc((reset_hi << 8) | reset_lo);

        // For mappers with CHR banking (e.g., MMC3), provide a 8KB pattern slot the mapper fills.
        let chr_backing = if cart.mapper == 4 && !cart.chr_rom.is_empty() {
            vec![0u8; 0x2000]
        } else {
            cart.chr_rom.clone()
        };

        let ppu = Ppu::new(chr_backing, cart.mirroring, cart.timing);
        let mut nb = NesBus::new(ppu);
        // Set APU timing to match cartridge
        nb.apu.set_timing(cart.timing);
        nb.install_cart(cart);
        self.cpu.set_bus(nb);
        self.cartridge_loaded = true;
        Ok(())
    }

    /// Load a ROM from byte data
    pub fn load_rom(&mut self, data: &[u8]) -> Result<(), std::io::Error> {
        let cart = cartridge::Cartridge::from_bytes(data)?;
        self.setup_cartridge(cart)
    }

    /// Load a mapper-0 (NROM) iNES ROM into CPU memory. This writes PRG ROM
    /// into 0x8000.. and mirrors 16KB banks into 0xC000 when necessary.
    pub fn load_rom_from_path<P: AsRef<std::path::Path>>(
        &mut self,
        path: P,
    ) -> Result<(), std::io::Error> {
        let cart = cartridge::Cartridge::from_file(path)?;
        self.setup_cartridge(cart)
    }
}

#[derive(thiserror::Error, Debug)]
pub enum NesError {
    #[error("Invalid ROM format")]
    InvalidRom,
    #[error("Unsupported mapper: {0}")]
    UnsupportedMapper(u8),
    #[error("Invalid mount point: {0}")]
    InvalidMountPoint(String),
    #[error("ROM too small: expected at least {expected} bytes, got {actual}")]
    RomTooSmall { expected: usize, actual: usize },
}

impl System for NesSystem {
    type Error = NesError;

    fn reset(&mut self) {
        self.cpu.reset();
        self.total_cycles = 0;
    }

    fn step_frame(&mut self) -> Result<Frame, Self::Error> {
        self.frame_index = self.frame_index.wrapping_add(1);
        let debug_scanline_drift = self.frame_index.is_multiple_of(60);

        // Drive the frame boundary from the PPU itself.
        //
        // Rationale:
        // - NTSC has an odd-frame cycle skip when rendering is enabled, so "CPU cycles per frame"
        //   is not an integer constant.
        // - Using fixed CPU-cycle budgets causes the PPU (scanline/dot) phase to drift across
        //   frames, which shows up as rolling / HUD drift in scanline-based rendering.
        //
        // So we run until the PPU reports it completed one full frame.
        let start_ppu_frame = self
            .cpu
            .bus()
            .map(|b| b.ppu.get_frame_counter())
            .unwrap_or(0);
        let target_ppu_frame = start_ppu_frame.wrapping_add(1);

        let mut cpu_steps: u32 = 0;
        let mut cpu_cycles_used: u32 = 0;
        let mut irqs: u32 = 0;
        let mut nmis: u32 = 0;
        let mut mmc3_a12_edges: u32 = 0;

        // Track PC histogram for trace logging (only allocate when tracing is enabled)
        let mut pc_hist: Option<HashMap<u16, u16>> = if self.instruction_tracer.is_enabled() {
            Some(HashMap::with_capacity(1024))
        } else {
            None
        };

        // Prepare an output frame and render scanlines incrementally during visible time.
        let mut rendered_scanlines: u32 = 0;

        // NOTE: VBlank clearing is now handled by PPU.tick() at scanline 261, dot 1
        // No need to manually call set_vblank(false) here

        while self
            .cpu
            .bus()
            .map(|b| b.ppu.get_frame_counter() != target_ppu_frame)
            .unwrap_or(false)
        {
            // Declare interrupt flags for this iteration
            let mut irq_to_fire = false;
            let mut nmi_to_fire = false;

            if let Some(h) = pc_hist.as_mut() {
                let pc = self.cpu.pc();
                let e = h.entry(pc).or_insert(0);
                *e = e.saturating_add(1);
            }

            let pc_before = self.cpu.pc();
            let used = self.cpu.step();
            cpu_steps = cpu_steps.wrapping_add(1);

            // Check for OAM DMA stall cycles.
            // On real NES hardware, writing $4014 stalls the CPU for 513 cycles
            // while the DMA controller copies 256 bytes to PPU OAM. The PPU and
            // APU continue running during this stall. Without these cycles, the
            // PPU falls ~4.5 scanlines behind the CPU every frame, which causes
            // sprite 0 hit detection and mid-frame scroll splits to be misaligned.
            let dma_cycles = if let Some(b) = self.cpu.bus_mut() {
                b.take_pending_dma_cycles()
            } else {
                0
            };
            let total_cycles = used + dma_cycles;
            cpu_cycles_used = cpu_cycles_used.wrapping_add(total_cycles);

            // CYCLE-ACCURATE PPU EXECUTION
            // Tick the PPU 3 times for each CPU cycle (PPU runs at 3x CPU clock)
            // This provides cycle-accurate VBlank/NMI timing.
            //
            // IMPORTANT: Render scanlines early (dot 1) when the v register still contains
            // the correct scroll position for that scanline. The v register is incremented
            // at dot 256, so rendering after that point would use the wrong scroll values.
            // This is critical for sprite 0 hit detection which depends on correct background
            // rendering at the sprite's screen position.
            if let Some(b) = self.cpu.bus_mut() {
                for _ in 0..total_cycles {
                    for _ in 0..3 {
                        let dot_before = b.ppu.get_dot();
                        let scanline_before = b.ppu.get_scanline();
                        let nmi_triggered = b.ppu.tick();
                        if nmi_triggered {
                            nmi_to_fire = true;
                        }

                        // Render at the START of visible scanlines (dot 0->1 transition)
                        // At this point, the v register contains the correct scroll for this scanline.
                        // The horizontal bits were just restored from t at dot 257 of the previous scanline,
                        // and vertical bits are correct for the current scanline.
                        //
                        // On odd frames, dot 0 of scanline 0 is skipped. Also, due to CPU instruction
                        // boundaries, the frame may start at various dots within scanline 0.
                        // We trigger rendering when:
                        // 1. dot_before == 0 (standard trigger for all scanlines)
                        // 2. For scanline 0: any dot if we haven't rendered it yet (catches odd frames
                        //    and frames starting mid-scanline)
                        // We use rendered_scanlines to prevent double-rendering.
                        let should_render = if scanline_before < 240
                            && scanline_before as u32 >= rendered_scanlines
                        {
                            // Standard trigger: at dot 0
                            // Special case for scanline 0: trigger on any dot since we might have
                            // missed dot 0 due to odd frame skip or CPU instruction boundaries
                            dot_before == 0 || (scanline_before == 0 && rendered_scanlines == 0)
                        } else {
                            false
                        };
                        if should_render {
                            if debug_scanline_drift
                                && (scanline_before < 3
                                    || scanline_before >= 237
                                    || (scanline_before >= 14 && scanline_before <= 20))
                            {
                                let ppu_dot = b.ppu.get_dot();
                                let ppu_mask = b.ppu.mask();
                                let v = b.ppu.vram_addr.get();
                                log(LogCategory::PPU, LogLevel::Info, || {
                                    format!(
                                        "NES: frame={} render_scanline={} ppu=({}, {}) mask=0x{:02X} v=${:04X}",
                                        self.frame_index, scanline_before, b.ppu.get_scanline(), ppu_dot, ppu_mask, v
                                    )
                                });
                            }

                            self.renderer
                                .render_scanline(&mut b.ppu, scanline_before as u32);
                            rendered_scanlines = scanline_before as u32 + 1;

                            // Apply any pending CHR updates from MMC2/MMC4 latch switching.
                            // This allows CHR bank switches to take effect on the next scanline
                            // instead of waiting until the end of the frame, reducing glitches
                            // in games like Punch Out!! that use CHR latching for animations.
                            b.apply_mapper_chr_update();

                            // Approximate MMC3 scanline IRQ clocking once per visible scanline.
                            // Gate it by rendering enabled (BG or sprites), matching common emulator behavior.
                            let rendering_enabled = (b.ppu.mask() & 0x18) != 0;
                            if rendering_enabled {
                                b.clock_mapper_a12_rising_edge();
                                mmc3_a12_edges = mmc3_a12_edges.wrapping_add(1);
                                if b.take_irq_pending() {
                                    irq_to_fire = true;
                                }
                            }
                        }
                    }
                }
            }

            // Record instruction if tracing is enabled
            if self.instruction_tracer.is_enabled() {
                if let Some(instr) = self.disassemble_instruction(pc_before as u32) {
                    let cpu_state = self.get_cpu_state();
                    self.instruction_tracer.trace(instr, cpu_state);
                }
            }

            // Update bus cycle counter for mapper timing
            if let Some(b) = self.cpu.bus_mut() {
                b.add_cycles(total_cycles);
            }

            // Clock APU IRQ counter
            if let Some(b) = self.cpu.bus_mut() {
                b.apu.clock_irq(total_cycles);
            }

            // Clock DMC channel and handle memory reads
            if let Some(b) = self.cpu.bus_mut() {
                // Clock DMC for the number of CPU cycles just executed
                for _ in 0..total_cycles {
                    if let Some(addr) = b.apu.clock_dmc() {
                        // DMC needs to read a byte from memory
                        // Use the Bus::read trait method to properly access memory
                        let byte = b.read(addr);
                        b.apu.load_dmc_sample(byte);
                    }
                }
            }

            // Also check for any mapper IRQs and pending NMI.
            if let Some(b) = self.cpu.bus_mut() {
                if b.take_irq_pending() {
                    irq_to_fire = true;
                }
                if b.ppu.take_nmi_pending() {
                    nmi_to_fire = true;
                }
            }

            // CYCLE-ACCURATE INTERRUPT DISPATCH
            //
            // On real hardware, NMI/IRQ dispatch takes 7 CPU cycles (21 PPU dots).
            // During those cycles the PPU continues running. We measure the actual
            // cycles consumed and tick PPU/APU for them to keep timing synchronized.
            if irq_to_fire {
                let cycles_before = self.cpu.cycles();
                self.cpu.trigger_irq();
                let irq_cycles = (self.cpu.cycles().wrapping_sub(cycles_before)) as u32;
                if irq_cycles > 0 {
                    log(LogCategory::Interrupts, LogLevel::Info, || {
                        format!("System: IRQ dispatched ({} cycles)", irq_cycles)
                    });
                    irqs = irqs.wrapping_add(1);
                    cpu_cycles_used = cpu_cycles_used.wrapping_add(irq_cycles);
                    // Tick PPU for the interrupt overhead (3 PPU dots per CPU cycle)
                    if let Some(b) = self.cpu.bus_mut() {
                        for _ in 0..irq_cycles {
                            for _ in 0..3 {
                                let nmi_triggered = b.ppu.tick();
                                if nmi_triggered {
                                    nmi_to_fire = true;
                                }
                            }
                        }
                        b.add_cycles(irq_cycles);
                        b.apu.clock_irq(irq_cycles);
                    }
                }
            }
            if nmi_to_fire {
                let cycles_before = self.cpu.cycles();
                self.cpu.trigger_nmi();
                let nmi_cycles = (self.cpu.cycles().wrapping_sub(cycles_before)) as u32;
                if nmi_cycles > 0 {
                    log(LogCategory::Interrupts, LogLevel::Debug, || {
                        format!("System: NMI dispatched ({} cycles)", nmi_cycles)
                    });
                    nmis = nmis.wrapping_add(1);
                    cpu_cycles_used = cpu_cycles_used.wrapping_add(nmi_cycles);
                    // Tick PPU for the interrupt overhead (3 PPU dots per CPU cycle)
                    if let Some(b) = self.cpu.bus_mut() {
                        for _ in 0..nmi_cycles {
                            for _ in 0..3 {
                                let _ = b.ppu.tick();
                            }
                        }
                        b.add_cycles(nmi_cycles);
                        b.apu.clock_irq(nmi_cycles);
                    }
                }
            }
        }

        // Note: CHR updates for MMC2/MMC4 are now applied after each scanline
        // in the rendering loop above, not once at the end of the frame.
        // This reduces latency from 240 scanlines to 1 scanline.

        if debug_scanline_drift {
            if let Some(b) = self.cpu.bus() {
                let ppu_sl = b.ppu.get_scanline();
                let ppu_dot = b.ppu.get_dot();
                let ppu_mask = b.ppu.mask();
                log(LogCategory::PPU, LogLevel::Info, || {
                    format!(
                        "NES: frame={} end_frame rendered_scanlines={} ppu=({}, {}) mask=0x{:02X}",
                        self.frame_index, rendered_scanlines, ppu_sl, ppu_dot, ppu_mask
                    )
                });
            }
        }

        // Snapshot stats for overlay.
        let (ppu_ctrl, ppu_mask, ppu_vblank, vec_nmi, vec_reset, vec_irq) =
            if let Some(b) = self.cpu.bus() {
                let read_u16 = |a: u16| -> u16 {
                    let lo = b.read(a) as u16;
                    let hi = b.read(a.wrapping_add(1)) as u16;
                    (hi << 8) | lo
                };
                (
                    b.ppu.ctrl(),
                    b.ppu.mask(),
                    b.ppu.vblank_flag(),
                    read_u16(0xFFFA),
                    read_u16(0xFFFC),
                    read_u16(0xFFFE),
                )
            } else {
                (0, 0, false, 0, 0, 0)
            };
        let pc = self.cpu.pc();

        let mut hotspots = [
            PcHotspot::default(),
            PcHotspot::default(),
            PcHotspot::default(),
        ];
        if let Some(h) = pc_hist {
            for (pc, count) in h {
                let s = PcHotspot { pc, count };
                if s.count > hotspots[0].count {
                    hotspots[2] = hotspots[1];
                    hotspots[1] = hotspots[0];
                    hotspots[0] = s;
                } else if s.count > hotspots[1].count {
                    hotspots[2] = hotspots[1];
                    hotspots[1] = s;
                } else if s.count > hotspots[2].count {
                    hotspots[2] = s;
                }
            }
        }

        self.last_stats = RuntimeStats {
            frame_index: self.frame_index,
            cpu_steps,
            cpu_cycles: cpu_cycles_used,
            irqs,
            nmis,
            mmc3_a12_edges,
            ppu_ctrl,
            ppu_mask,
            ppu_vblank,
            pc,
            vec_reset,
            vec_nmi,
            vec_irq,
            pc_hotspots: hotspots,
        };

        // Log frame statistics at trace level
        log(LogCategory::CPU, LogLevel::Trace, || {
            // Log occasionally to avoid overwhelming the output
            if self.frame_index.is_multiple_of(60) {
                format!(
                    "NES TRACE: frame={} pc=0x{:04X} steps={} cycles={} irq={} nmi={} a12_edges={} ppu_ctrl=0x{:02X} ppu_mask=0x{:02X} vec_reset=0x{:04X} vec_nmi=0x{:04X} vec_irq=0x{:04X}",
                    self.last_stats.frame_index,
                    self.last_stats.pc,
                    self.last_stats.cpu_steps,
                    self.last_stats.cpu_cycles,
                    self.last_stats.irqs,
                    self.last_stats.nmis,
                    self.last_stats.mmc3_a12_edges,
                    self.last_stats.ppu_ctrl,
                    self.last_stats.ppu_mask,
                    self.last_stats.vec_reset,
                    self.last_stats.vec_nmi,
                    self.last_stats.vec_irq
                )
            } else {
                String::new()
            }
        });

        // Log PC hotspots at trace level
        log(LogCategory::CPU, LogLevel::Trace, || {
            if self.frame_index.is_multiple_of(60) {
                let h0 = self.last_stats.pc_hotspots[0];
                let h1 = self.last_stats.pc_hotspots[1];
                let h2 = self.last_stats.pc_hotspots[2];
                format!(
                    "NES PC HOT: frame={} [0x{:04X} x{}] [0x{:04X} x{}] [0x{:04X} x{}]",
                    self.last_stats.frame_index, h0.pc, h0.count, h1.pc, h1.count, h2.pc, h2.count
                )
            } else {
                String::new()
            }
        });

        // Track total cycles
        self.total_cycles += cpu_cycles_used as u64;

        // Return the rendered frame from the renderer by taking ownership
        // This avoids cloning 61,440 pixels (245KB) every frame (60 times/second)
        Ok(self.renderer.take_frame())
    }

    fn save_state(&self) -> serde_json::Value {
        // Note: This is a minimal save state implementation.
        // A complete implementation would include:
        // - CPU registers (A, X, Y, SP, P, PC)
        // - RAM and WRAM contents
        // - PPU registers and VRAM
        // - APU state
        // - Mapper state (bank registers, IRQ counters, etc.)
        // - Controller latch state
        //
        // Currently only saves a minimal placeholder to validate the interface.
        // ROM verification is handled by the frontend via ROM hash.
        serde_json::json!({ "system": "nes", "version": 1, "a": self.cpu.a() })
    }

    fn load_state(&mut self, v: &serde_json::Value) -> Result<(), serde_json::Error> {
        // Basic validation: check system type if present
        if let Some(system) = v.get("system").and_then(|s| s.as_str()) {
            if system != "nes" {
                // Wrong system type - use deserialization to generate proper error
                let _: () = serde_json::from_value(v.clone())?;
            }
        }

        // Note: ROM verification is handled by the frontend via ROM hash.
        // Full state restoration will be implemented when save state format is finalized.
        // Currently validates the state structure only.
        Ok(())
    }

    fn supports_save_states(&self) -> bool {
        // Only support save states when a cartridge is loaded
        self.cartridge_loaded
    }

    fn mount_points(&self) -> Vec<MountPointInfo> {
        vec![MountPointInfo {
            id: "Cartridge".to_string(),
            name: "Cartridge Slot".to_string(),
            extensions: vec!["nes".to_string(), "unf".to_string()],
            required: true,
        }]
    }

    fn mount(&mut self, mount_point_id: &str, data: &[u8]) -> Result<(), Self::Error> {
        if mount_point_id != "Cartridge" {
            return Err(NesError::InvalidMountPoint(mount_point_id.to_string()));
        }
        self.load_rom(data).map_err(|_| NesError::InvalidRom)
    }

    fn unmount(&mut self, mount_point_id: &str) -> Result<(), Self::Error> {
        if mount_point_id != "Cartridge" {
            return Err(NesError::InvalidMountPoint(mount_point_id.to_string()));
        }
        // Reset to default state (no cartridge)
        *self = Self::default();
        Ok(())
    }

    fn is_mounted(&self, mount_point_id: &str) -> bool {
        mount_point_id == "Cartridge" && self.cartridge_loaded
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
    use emu_core::System;

    #[test]
    fn test_nes_mount_points() {
        let sys = NesSystem::default();
        let mount_points = sys.mount_points();

        assert_eq!(mount_points.len(), 1);
        assert_eq!(mount_points[0].id, "Cartridge");
        assert_eq!(mount_points[0].name, "Cartridge Slot");
        assert!(mount_points[0].required);
        assert!(mount_points[0].extensions.contains(&"nes".to_string()));
    }

    #[test]
    fn test_nes_save_state_support() {
        let sys = NesSystem::default();

        // Should not support save states without a cartridge
        assert!(!sys.supports_save_states());

        // After mounting a valid ROM, should support save states
        // Note: We'd need a valid test ROM to fully test this
    }

    #[test]
    fn test_nes_mount_unmount() {
        let mut sys = NesSystem::default();

        // Initially not mounted
        assert!(!sys.is_mounted("Cartridge"));

        // Trying to mount to wrong mount point should fail
        assert!(sys.mount("BIOS", &[]).is_err());

        // Trying to unmount wrong mount point should fail
        assert!(sys.unmount("BIOS").is_err());
    }

    #[test]
    fn test_nes_load_state_validation() {
        let mut sys = NesSystem::default();

        // Should succeed with valid NES state (cartridge check is done via ROM hash in frontend)
        let state = serde_json::json!({"system": "nes", "version": 1});
        assert!(sys.load_state(&state).is_ok());

        // Should fail with wrong system type
        let wrong_state = serde_json::json!({"system": "gb", "version": 1});
        assert!(sys.load_state(&wrong_state).is_err());
    }

    #[test]
    fn test_nes_controller_input() {
        use crate::bus::Bus;

        let mut sys = NesSystem::default();

        // Set controller 0 state: A=1, B=1, others=0
        // NES button order: A, B, Select, Start, Up, Down, Left, Right
        let buttons = 0b00000011; // A and B pressed
        sys.set_controller(0, buttons);

        // Verify controller state was set in the bus
        if let Some(bus) = sys.cpu.bus() {
            assert_eq!(bus.controller_state[0], buttons);
        }

        // Controller 1 should be unaffected
        if let Some(bus) = sys.cpu.bus() {
            assert_eq!(bus.controller_state[1], 0);
        }

        // Set controller 1 state
        let buttons2 = 0b11110000; // D-pad all pressed
        sys.set_controller(1, buttons2);

        if let Some(bus) = sys.cpu.bus() {
            assert_eq!(bus.controller_state[0], buttons);
            assert_eq!(bus.controller_state[1], buttons2);
        }

        // Test controller strobe and shift behavior
        if let Some(bus) = sys.cpu.bus_mut() {
            // Strobe controller to latch state
            bus.write(0x4016, 1);
            bus.write(0x4016, 0);

            // Read 8 bits from controller 0
            assert_eq!(bus.read(0x4016) & 1, 1); // A button
            assert_eq!(bus.read(0x4016) & 1, 1); // B button
            assert_eq!(bus.read(0x4016) & 1, 0); // Select
            assert_eq!(bus.read(0x4016) & 1, 0); // Start
            assert_eq!(bus.read(0x4016) & 1, 0); // Up
            assert_eq!(bus.read(0x4016) & 1, 0); // Down
            assert_eq!(bus.read(0x4016) & 1, 0); // Left
            assert_eq!(bus.read(0x4016) & 1, 0); // Right
        }
    }

    #[test]
    fn test_nes_controller_reads_beyond_8_bits() {
        // Edge case: Reading beyond the standard 8 button bits
        // Hardware behavior: After 8 reads, subsequent reads should return 1 (open bus)
        use crate::bus::Bus;

        let mut sys = NesSystem::default();
        let buttons = 0b11111111; // All buttons pressed

        sys.set_controller(0, buttons);

        if let Some(bus) = sys.cpu.bus_mut() {
            // Strobe controller to latch state
            bus.write(0x4016, 1);
            bus.write(0x4016, 0);

            // Read 8 bits (should match button state)
            for i in 0..8 {
                let expected = (buttons >> i) & 1;
                assert_eq!(bus.read(0x4016) & 1, expected, "Bit {} mismatch", i);
            }

            // Read beyond 8 bits - should return 1 (open bus behavior)
            for i in 8..16 {
                let val = bus.read(0x4016) & 1;
                assert_eq!(
                    val, 1,
                    "Bit {} beyond valid range should return 1 (open bus)",
                    i
                );
            }
        }
    }

    #[test]
    fn test_nes_controller_ninth_read_is_open_bus() {
        // Specific test for the 9th read to verify off-by-one handling
        // When all buttons are released (0), first 8 reads return 0, 9th returns 1
        use crate::bus::Bus;

        let mut sys = NesSystem::default();
        sys.set_controller(0, 0); // No buttons pressed

        if let Some(bus) = sys.cpu.bus_mut() {
            bus.write(0x4016, 1);
            bus.write(0x4016, 0);

            // First 8 reads should return 0 (no buttons pressed)
            for i in 0..8 {
                assert_eq!(
                    bus.read(0x4016) & 1,
                    0,
                    "Read {} should return 0 (button not pressed)",
                    i + 1
                );
            }

            // 9th read should return 1 (open bus), not 0
            assert_eq!(
                bus.read(0x4016) & 1,
                1,
                "9th read should return 1 (open bus)"
            );

            // 10th read should also return 1 (open bus)
            assert_eq!(
                bus.read(0x4016) & 1,
                1,
                "10th read should return 1 (open bus)"
            );
        }
    }

    #[test]
    fn test_nes_controller_strobe_during_reads() {
        // Edge case: Strobing controller during reads should reset the shift register
        use crate::bus::Bus;

        let mut sys = NesSystem::default();
        let buttons = 0b10101010;

        sys.set_controller(0, buttons);

        if let Some(bus) = sys.cpu.bus_mut() {
            // Strobe to latch
            bus.write(0x4016, 1);
            bus.write(0x4016, 0);

            // Read first 3 bits
            assert_eq!(bus.read(0x4016) & 1, 0); // bit 0
            assert_eq!(bus.read(0x4016) & 1, 1); // bit 1
            assert_eq!(bus.read(0x4016) & 1, 0); // bit 2

            // Re-strobe (reset shift register)
            bus.write(0x4016, 1);
            bus.write(0x4016, 0);

            // Should start from bit 0 again
            assert_eq!(bus.read(0x4016) & 1, 0); // bit 0
            assert_eq!(bus.read(0x4016) & 1, 1); // bit 1
        }
    }

    #[test]
    fn test_nes_controller_read_while_strobed() {
        // Edge case: Reading while strobe is high should always return button A state
        use crate::bus::Bus;

        let mut sys = NesSystem::default();
        let buttons = 0b00000001; // Only A button pressed

        sys.set_controller(0, buttons);

        if let Some(bus) = sys.cpu.bus_mut() {
            // Set strobe high
            bus.write(0x4016, 1);

            // Multiple reads while strobed should all return A button state
            for _ in 0..10 {
                assert_eq!(
                    bus.read(0x4016) & 1,
                    1,
                    "While strobed, should return A button state"
                );
            }

            // Disable strobe
            bus.write(0x4016, 0);

            // Now should shift normally
            assert_eq!(bus.read(0x4016) & 1, 1); // A
            assert_eq!(bus.read(0x4016) & 1, 0); // B (not pressed)
        }
    }

    #[test]
    fn test_nes_audio_no_dc_offset() {
        // Test that audio doesn't have a DC offset when no sound is being played
        // This was a bug where the triangle channel always output its current
        // waveform value even when disabled, causing a DC offset
        let mut sys = NesSystem::default();

        // Load the test ROM
        let test_rom = include_bytes!("../../../../test_roms/nes/test.nes");
        assert!(sys.mount("Cartridge", test_rom).is_ok());

        // Run a few frames to initialize
        for _ in 0..5 {
            let _ = sys.step_frame();
        }

        // Get audio samples
        let audio_samples = sys.get_audio_samples(735);
        assert_eq!(audio_samples.len(), 735);

        // Calculate average to detect DC offset
        let sum: i64 = audio_samples.iter().map(|&s| s as i64).sum();
        let avg = sum / audio_samples.len() as i64;

        // The average should be close to 0 (no DC offset)
        // Allow small variation due to normal audio content, but not 2048 which was the bug
        assert!(
            avg.abs() < 500,
            "Audio has DC offset of {}, expected close to 0",
            avg
        );
    }

    #[test]
    fn test_nes_ram_mirroring_boundaries() {
        // Edge case: Internal RAM is 2KB (0x0000-0x07FF) but mirrored 4 times
        // Writing to 0x0800, 0x1000, 0x1800 should all mirror to 0x0000-0x07FF
        use crate::bus::Bus;

        let mut sys = NesSystem::default();

        if let Some(bus) = sys.cpu.bus_mut() {
            // Write to base address
            bus.write(0x0042, 0xAA);
            assert_eq!(bus.read(0x0042), 0xAA);

            // Verify mirroring at 0x0800 boundary
            assert_eq!(bus.read(0x0842), 0xAA, "RAM should mirror at 0x0800");

            // Verify mirroring at 0x1000 boundary
            assert_eq!(bus.read(0x1042), 0xAA, "RAM should mirror at 0x1000");

            // Verify mirroring at 0x1800 boundary
            assert_eq!(bus.read(0x1842), 0xAA, "RAM should mirror at 0x1800");

            // Write through mirror
            bus.write(0x1543, 0x55);
            assert_eq!(
                bus.read(0x0543),
                0x55,
                "Write through mirror should affect base"
            );
            assert_eq!(bus.read(0x0D43), 0x55, "Mirror should be consistent");
        }
    }

    #[test]
    fn test_nes_wram_boundaries() {
        // Edge case: WRAM is at 0x6000-0x7FFF (8KB)
        // Verify no mirroring/wrapping within this range
        // NOTE: WRAM is only present for mappers that support it (MMC1, MMC3, MMC5)
        use crate::bus::Bus;
        use crate::cartridge::{Cartridge, Mirroring};
        use emu_core::apu::TimingMode;

        let mut sys = NesSystem::default();

        // Create a minimal MMC1 cartridge (mapper 1) which has WRAM
        let cart = Cartridge::new_test(
            vec![0; 0x8000], // 32KB PRG ROM
            vec![0; 0x2000], // 8KB CHR ROM
            1,               // MMC1 mapper
            Mirroring::Horizontal,
            TimingMode::Ntsc,
        );
        sys.setup_cartridge(cart)
            .expect("Failed to load test cartridge");

        if let Some(bus) = sys.cpu.bus_mut() {
            // Write to start and end of WRAM
            bus.write(0x6000, 0x11);
            bus.write(0x7FFF, 0x22);

            assert_eq!(bus.read(0x6000), 0x11);
            assert_eq!(bus.read(0x7FFF), 0x22);

            // Verify they don't interfere
            assert_ne!(bus.read(0x6000), 0x22);
            assert_ne!(bus.read(0x7FFF), 0x11);

            // Test boundary between RAM and WRAM
            bus.write(0x1FFF, 0x33); // Last mirrored RAM address
            bus.write(0x6000, 0x44); // First WRAM address

            // They should be different regions
            assert_eq!(bus.read(0x1FFF), 0x33);
            assert_eq!(bus.read(0x6000), 0x44);
        }
    }

    #[test]
    fn test_nes_wram_not_present_for_axrom() {
        // Verify WRAM is NOT present for mappers without PRG RAM (e.g., AxROM)
        // This is important for games like Battletoads that rely on open bus behavior
        // Open bus returns the last value read from the data bus
        // Reference: https://www.nesdev.org/wiki/Open_bus_behavior
        use crate::bus::Bus;
        use crate::cartridge::{Cartridge, Mirroring};
        use emu_core::apu::TimingMode;

        let mut sys = NesSystem::default();

        // Create a minimal AxROM cartridge (mapper 7) which does NOT have WRAM
        let cart = Cartridge::new_test(
            vec![0xAB; 0x8000], // 32KB PRG ROM filled with 0xAB
            vec![0; 0x2000],    // 8KB CHR ROM
            7,                  // AxROM mapper
            Mirroring::Horizontal,
            TimingMode::Ntsc,
        );
        sys.setup_cartridge(cart)
            .expect("Failed to load test cartridge");

        if let Some(bus) = sys.cpu.bus_mut() {
            // First read from PRG ROM to set the open bus value
            let prg_value = bus.read(0x8000);
            assert_eq!(prg_value, 0xAB, "PRG ROM should return 0xAB");

            // Write to WRAM area should be ignored (no WRAM present)
            bus.write(0x6000, 0x42);

            // Read from WRAM area should return open bus (last value read = 0xAB)
            assert_eq!(
                bus.read(0x6000),
                0xAB,
                "AxROM should not have WRAM - reads should return open bus (last value read)"
            );
        }
    }

    #[test]
    fn test_nes_ppu_register_mirroring() {
        // Edge case: PPU registers (0x2000-0x2007) are mirrored throughout 0x2008-0x3FFF
        use crate::bus::Bus;

        let mut sys = NesSystem::default();

        if let Some(bus) = sys.cpu.bus_mut() {
            // Write to PPUCTRL (0x2000)
            bus.write(0x2000, 0x80);

            // Read from mirrored addresses
            // Note: PPUCTRL is write-only, so we test with PPUSTATUS (0x2002) which is readable
            bus.write(0x2006, 0x3F); // PPUADDR high byte
            bus.write(0x2006, 0x00); // PPUADDR low byte

            // Test mirroring at various boundaries
            let status1 = bus.read(0x2002); // Base address
            let status2 = bus.read(0x200A); // +8 (first mirror)
            let status3 = bus.read(0x2102); // +256
            let status4 = bus.read(0x3FFE); // Near end of range (0x3FFE % 8 = 6, but should map to 2)

            // All should return status (even though values might differ due to side effects)
            // The important thing is they all access the PPU, not crash
            // We can't assert equality because reading PPUSTATUS has side effects
            let _ = (status1, status2, status3, status4);
        }
    }

    #[test]
    fn test_nes_smoke_test_rom() {
        // Load the test ROM
        let test_rom = include_bytes!("../../../../test_roms/nes/test.nes");

        let mut sys = NesSystem::default();

        // Mount the test ROM
        assert!(sys.mount("Cartridge", test_rom).is_ok());
        assert!(sys.is_mounted("Cartridge"));

        // Run a few frames to let the ROM initialize and render
        let mut frame = sys.step_frame().unwrap();
        for _ in 0..9 {
            frame = sys.step_frame().unwrap();
        }

        // Verify frame dimensions
        assert_eq!(frame.width, 256);
        assert_eq!(frame.height, 240);
        assert_eq!(frame.pixels.len(), 256 * 240);

        // The test ROM displays a checkerboard pattern with two alternating colors.
        // Verify that:
        // 1. Exactly 2 distinct colors are present
        // 2. The distribution is approximately 50/50

        use std::collections::HashMap;
        let mut color_counts: HashMap<u32, usize> = HashMap::new();
        for &pixel in &frame.pixels {
            *color_counts.entry(pixel).or_insert(0) += 1;
        }

        assert_eq!(
            color_counts.len(),
            2,
            "Expected exactly 2 colors for checkerboard pattern, got {}",
            color_counts.len()
        );

        // Check that both colors have roughly equal distribution (45-55% each)
        let total_pixels = frame.pixels.len();
        for (color, count) in color_counts.iter() {
            let percentage = (*count as f64 / total_pixels as f64) * 100.0;
            assert!(
                percentage >= 45.0 && percentage <= 55.0,
                "Color 0x{:08X} has {:.1}% of pixels, expected 45-55% for checkerboard",
                color,
                percentage
            );
        }
    }

    #[test]
    fn test_nes_pal_timing_detection() {
        // Create a minimal PAL ROM using NES 2.0 format
        let mut data = vec![
            0x4E, 0x45, 0x53, 0x1A, // NES<EOF>
            0x01, 0x00, // 16KB PRG, no CHR
            0x00, // Flags 6: NROM, horizontal mirroring
            0x08, // Flags 7: NES 2.0 format (bits 2-3 = 10)
            0x00, 0x00, 0x00, 0x00, 0x01, // Byte 12: PAL timing (bits 0-1 = 01)
            0x00, 0x00, 0x00,
        ];
        // Add PRG ROM (16KB) with proper reset vector
        let mut prg = vec![0; 16 * 1024];
        prg[0x1FFC] = 0x00;
        prg[0x1FFD] = 0x80;
        data.extend(prg);

        let mut sys = NesSystem::default();
        sys.load_rom(&data).expect("Failed to load PAL ROM");

        // Verify timing mode was set to PAL
        assert_eq!(sys.timing(), TimingMode::Pal);
    }

    #[test]
    fn test_nes_ntsc_timing_detection() {
        // Create a minimal NTSC ROM using NES 2.0 format
        let mut data = vec![
            0x4E, 0x45, 0x53, 0x1A, // NES<EOF>
            0x01, 0x00, // 16KB PRG, no CHR
            0x00, // Flags 6: NROM, horizontal mirroring
            0x08, // Flags 7: NES 2.0 format (bits 2-3 = 10)
            0x00, 0x00, 0x00, 0x00, 0x00, // Byte 12: NTSC timing (bits 0-1 = 00)
            0x00, 0x00, 0x00,
        ];
        // Add PRG ROM (16KB) with proper reset vector
        let mut prg = vec![0; 16 * 1024];
        prg[0x1FFC] = 0x00;
        prg[0x1FFD] = 0x80;
        data.extend(prg);

        let mut sys = NesSystem::default();
        sys.load_rom(&data).expect("Failed to load NTSC ROM");

        // Verify timing mode was set to NTSC
        assert_eq!(sys.timing(), TimingMode::Ntsc);
    }

    #[test]
    fn test_nes_timing_propagates_to_apu() {
        // Create a PAL ROM
        let mut data = vec![
            0x4E, 0x45, 0x53, 0x1A, // NES<EOF>
            0x01, 0x00, // 16KB PRG, no CHR
            0x00, 0x08, // NES 2.0 format
            0x00, 0x00, 0x00, 0x00, 0x01, // PAL timing
            0x00, 0x00, 0x00,
        ];
        let mut prg = vec![0; 16 * 1024];
        prg[0x1FFC] = 0x00;
        prg[0x1FFD] = 0x80;
        data.extend(prg);

        let mut sys = NesSystem::default();
        sys.load_rom(&data).expect("Failed to load PAL ROM");

        // Verify we can change timing mode dynamically
        sys.set_timing(TimingMode::Ntsc);
        assert_eq!(sys.timing(), TimingMode::Ntsc);
        sys.set_timing(TimingMode::Pal);
        assert_eq!(sys.timing(), TimingMode::Pal);
    }

    #[test]
    fn test_nes_first_frame_register_protection() {
        // Test that PPU registers are properly write-protected during the first frame after reset
        // Reference: problemkaputt.de everynes.htm - PPU Reset section
        use crate::bus::Bus;
        use crate::cartridge::{Cartridge, Mirroring};
        use emu_core::apu::TimingMode;

        let mut sys = NesSystem::default();

        // Create a minimal test cartridge
        let cart = Cartridge::new_test(
            vec![0; 0x8000], // 32KB PRG ROM
            vec![0; 0x2000], // 8KB CHR ROM
            0,               // NROM mapper
            Mirroring::Horizontal,
            TimingMode::Ntsc,
        );
        sys.setup_cartridge(cart)
            .expect("Failed to load test cartridge");

        if let Some(bus) = sys.cpu.bus_mut() {
            // During first frame, these writes should be blocked:
            // - $2000 (PPUCTRL)
            // - $2001 (PPUMASK)
            // - $2005 (PPUSCROLL)
            // - $2006 (PPUADDR)
            // And PPUDATA ($2007) reads should return 0

            // Test PPUCTRL ($2000) write protection
            bus.write(0x2000, 0xFF);
            // PPUCTRL should remain 0 (can't read it directly, but we can verify side effects)

            // Test PPUMASK ($2001) write protection
            bus.write(0x2001, 0xFF);
            // PPUMASK should remain 0 (can't read directly)

            // Test PPUSCROLL ($2005) write protection
            bus.write(0x2005, 0x12);
            bus.write(0x2005, 0x34);
            // Scroll should not be affected (can't verify directly without reading internal state)

            // Test PPUADDR ($2006) write protection
            bus.write(0x2006, 0x20);
            bus.write(0x2006, 0x00);
            // VRAM address should not be set

            // Test PPUDATA ($2007) read protection - should return 0
            let data = bus.read(0x2007);
            assert_eq!(
                data, 0x00,
                "PPUDATA read during first frame should return 0x00"
            );

            // Now advance past the first frame by simulating VBlank end
            // The first frame protection is cleared at the end of the first VBlank
            // For this test, we'll manually clear it
            bus.ppu.clear_first_frame_lock();

            // After first frame, writes should work
            bus.write(0x2000, 0x80); // Enable NMI
            bus.write(0x2001, 0x1E); // Enable rendering
            bus.write(0x2006, 0x20); // Set VRAM address high
            bus.write(0x2006, 0x00); // Set VRAM address low

            // Write some data to VRAM
            bus.write(0x2007, 0x42);

            // Reset address and read back
            bus.write(0x2006, 0x20);
            bus.write(0x2006, 0x00);
            // Skip buffered read
            let _ = bus.read(0x2007);
            // Read actual data
            let read_data = bus.read(0x2007);
            assert_eq!(
                read_data, 0x42,
                "After first frame, PPUDATA should be readable"
            );
        }
    }
}
