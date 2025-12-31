//! NES (Nintendo Entertainment System) emulation implementation.
//!
//! This module provides a complete NES system emulator using the reusable 6502 CPU core
//! from `emu_core`, along with NES-specific components:
//!
//! - **CPU**: Ricoh 2A03 (6502 without decimal mode)
//! - **PPU**: 2C02 Picture Processing Unit with frame-based rendering
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
//! - Frame-based rendering (not cycle-accurate)
//!
//! ## APU Features
//!
//! - 2 pulse channels with duty cycle control
//! - **Sweep units** for frequency modulation (pitch bending) on both pulse channels
//! - Triangle channel with 32-step waveform
//! - Noise channel with pseudo-random LFSR
//! - Length counter and envelope support
//! - Frame counter (4-step and 5-step modes)
//! - APU IRQ support (frame counter)
//! - 44.1 kHz audio output
//! - Note: DMC channel not yet implemented
//!
//! ## Timing Model
//!
//! The emulator uses a frame-based timing model rather than cycle-accurate PPU rendering:
//!
//! - **NTSC**: ~29,780 CPU cycles per frame (~60.1 Hz)
//! - **PAL**: ~33,247 CPU cycles per frame (~50.0 Hz)
//! - **VBlank**: Simulated at end of frame with appropriate cycle count
//! - **Scanline IRQs**: Synthesized for mappers like MMC3
//!
//! This model is suitable for most games but may not handle edge cases requiring
//! precise PPU timing (mid-scanline effects, exact sprite 0 hit timing, etc.).

#![allow(clippy::upper_case_acronyms)]
#![allow(clippy::unnecessary_cast)]
#![allow(clippy::manual_range_contains)]
#![allow(clippy::let_and_return)]

mod apu;
mod bus;
mod cartridge;
mod cpu;
mod mappers;
mod ppu;
pub mod ppu_renderer;
#[cfg(feature = "opengl")]
pub mod ppu_renderer_opengl;

use crate::bus::Bus;
use crate::cartridge::Mirroring;
use bus::NesBus;
use cpu::NesCpu;
use emu_core::logging::{LogCategory, LogConfig, LogLevel};
use emu_core::{apu::TimingMode, types::Frame, MountPointInfo, System};
use ppu::Ppu;
use ppu_renderer::{NesPpuRenderer, SoftwareNesPpuRenderer};
use std::collections::HashMap;

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
    /// iNES mapper number (0-255)
    pub mapper_number: u8,
    /// Number of 16KB PRG banks
    pub prg_banks: usize,
    /// Number of 8KB CHR banks (0 for CHR-RAM)
    pub chr_banks: usize,
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
}

impl NesSystem {
    /// Set controller 0 or 1 button state (bits 0..7 correspond to controller buttons).
    pub fn set_controller(&mut self, idx: usize, state: u8) {
        if let Some(b) = self.cpu.bus_mut() {
            b.set_controller(idx, state);
        }
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
        let mut mapper_number = 0u8;
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

    /// Get runtime stats for debugging / overlays.
    pub fn get_runtime_stats(&self) -> RuntimeStats {
        self.last_stats
    }
}

impl Default for NesSystem {
    fn default() -> Self {
        // create PPU with empty CHR and NesBus and attach to CPU
        let mut cpu = NesCpu::new();
        cpu.reset();
        let ppu = Ppu::new(vec![], Mirroring::Vertical);
        let bus = NesBus::new(ppu);
        cpu.set_bus(bus);
        Self {
            cpu,
            timing: TimingMode::Ntsc,
            cartridge_loaded: false,
            frame_index: 0,
            last_stats: RuntimeStats::default(),
            renderer: Box::new(SoftwareNesPpuRenderer::new()),
        }
    }
}

impl NesSystem {
    /// Common cartridge setup logic
    fn setup_cartridge(&mut self, cart: cartridge::Cartridge) -> Result<(), std::io::Error> {
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

        let ppu = Ppu::new(chr_backing, cart.mirroring);
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
    }

    fn step_frame(&mut self) -> Result<Frame, Self::Error> {
        // Run CPU cycles for one frame.
        // NTSC: ~29780 CPU cycles, PAL: ~33247 CPU cycles
        // Model VBlank as the *tail* of the frame and trigger NMI at VBlank start.
        // IMPORTANT: render at the end of the *visible* portion (right before VBlank)
        // so we don't sample while games temporarily disable PPUMASK during their NMI.

        let (cycles_per_frame, vblank_cycles) = match self.timing {
            TimingMode::Ntsc => (29780u32, 2500u32),
            TimingMode::Pal => (33247u32, 2798u32), // PAL has more cycles per frame
        };
        let visible_cycles = cycles_per_frame - vblank_cycles;

        // Approximate PPU scanline timing so mappers like MMC3 can clock their IRQ counter.
        // The NES PPU runs at 3x the CPU clock and has 341 PPU cycles per scanline.
        // In this frame-based renderer we synthesize one A12 rising edge per scanline.
        let mut ppu_cycles_accum: u32 = 0;
        let ppu_cycles_per_scanline: u32 = 341;

        self.frame_index = self.frame_index.wrapping_add(1);

        let mut cpu_steps: u32 = 0;
        let mut cpu_cycles_used: u32 = 0;
        let mut irqs: u32 = 0;
        let mut nmis: u32 = 0;
        let mut mmc3_a12_edges: u32 = 0;
        let mut rendering_happened: bool = false;

        let mut pc_hist: Option<HashMap<u16, u16>> =
            if LogConfig::global().should_log(LogCategory::CPU, LogLevel::Trace) {
                Some(HashMap::with_capacity(1024))
            } else {
                None
            };

        // Prepare an output frame and render scanlines incrementally during visible time.
        let mut rendered_scanlines: u32 = 0;

        // Visible portion (VBlank low)
        if let Some(b) = self.cpu.bus_mut() {
            b.ppu.set_vblank(false);
        }
        let mut cycles = 0u32;
        while cycles < visible_cycles {
            if let Some(h) = pc_hist.as_mut() {
                let pc = self.cpu.pc();
                let e = h.entry(pc).or_insert(0);
                *e = e.saturating_add(1);
            }

            let used = self.cpu.step();
            cpu_steps = cpu_steps.wrapping_add(1);
            cpu_cycles_used = cpu_cycles_used.wrapping_add(used);
            cycles = cycles.wrapping_add(used);

            // Clock APU IRQ counter
            if let Some(b) = self.cpu.bus_mut() {
                b.apu.clock_irq(used);
            }

            let mut irq_to_fire = false;
            let mut nmi_to_fire = false;

            // Synthesize scanline edges for mapper IRQs during visible time.
            // Only do this when rendering is enabled (background or sprites).
            if let Some(b) = self.cpu.bus_mut() {
                let rendering_enabled = (b.ppu.mask() & 0x18) != 0;
                if rendering_enabled {
                    rendering_happened = true;
                    ppu_cycles_accum = ppu_cycles_accum.saturating_add(used.saturating_mul(3));
                    while ppu_cycles_accum >= ppu_cycles_per_scanline {
                        ppu_cycles_accum -= ppu_cycles_per_scanline;

                        // Render the scanline that just completed using the state that was in
                        // effect during that scanline. MMC3 IRQ-triggered bank changes typically
                        // affect the *next* scanline.
                        if rendered_scanlines < 240 {
                            self.renderer
                                .render_scanline(&mut b.ppu, rendered_scanlines);
                            rendered_scanlines += 1;
                        }

                        b.clock_mapper_a12_rising_edge();
                        mmc3_a12_edges = mmc3_a12_edges.wrapping_add(1);
                        if b.take_irq_pending() {
                            irq_to_fire = true;
                        }
                    }
                }

                // Also check for any mapper IRQs not driven by the synthesized scanline clock.
                if b.take_irq_pending() {
                    irq_to_fire = true;
                }

                if b.ppu.take_nmi_pending() {
                    nmi_to_fire = true;
                }
            }

            if irq_to_fire {
                if LogConfig::global().should_log(LogCategory::Interrupts, LogLevel::Info) {
                    eprintln!("System: Firing IRQ! Mapper/APU pending.");
                }
                self.cpu.trigger_irq();
                irqs = irqs.wrapping_add(1);
            }
            if nmi_to_fire {
                self.cpu.trigger_nmi();
                nmis = nmis.wrapping_add(1);
            }
        }

        // If we didn't reach exactly 240 synthesized scanlines (e.g., timing edge cases),
        // render any remaining scanlines using the final visible-state.
        if rendered_scanlines < 240 {
            if let Some(b) = self.cpu.bus_mut() {
                while rendered_scanlines < 240 {
                    self.renderer
                        .render_scanline(&mut b.ppu, rendered_scanlines);
                    rendered_scanlines += 1;
                }
            }
        }

        // Apply any pending CHR updates from MMC2/MMC4 latch switching during rendering.
        if let Some(b) = self.cpu.bus_mut() {
            b.apply_mapper_chr_update();
        }

        // VBlank start
        if let Some(b) = self.cpu.bus_mut() {
            b.ppu.set_vblank(true);
        }

        // Run the rest of the frame (VBlank time).
        while cycles < cycles_per_frame {
            if let Some(h) = pc_hist.as_mut() {
                let pc = self.cpu.pc();
                let e = h.entry(pc).or_insert(0);
                *e = e.saturating_add(1);
            }

            let used = self.cpu.step();
            cpu_steps = cpu_steps.wrapping_add(1);
            cpu_cycles_used = cpu_cycles_used.wrapping_add(used);
            cycles = cycles.wrapping_add(used);

            // Check for mapper IRQs during VBlank as well.
            let mut irq_to_fire = false;
            let mut nmi_to_fire = false;
            if let Some(b) = self.cpu.bus_mut() {
                if b.take_irq_pending() {
                    irq_to_fire = true;
                }
                if b.ppu.take_nmi_pending() {
                    nmi_to_fire = true;
                }
            }
            if irq_to_fire {
                self.cpu.trigger_irq();
                irqs = irqs.wrapping_add(1);
            }
            if nmi_to_fire {
                self.cpu.trigger_nmi();
                nmis = nmis.wrapping_add(1);
            }
        }

        // VBlank end / Pre-render scanline start
        // Clear sprite flags (sprite 0 hit and sprite overflow) at start of pre-render scanline
        if let Some(b) = self.cpu.bus_mut() {
            b.ppu.clear_sprite_flags();
            b.ppu.set_vblank(false);
        }

        // Many MMC3 games clock the IRQ counter 241 times per frame (one per visible scanline plus
        // an additional clock during the pre-render scanline). Our frame model naturally produces
        // 240 clocks during the 240 visible scanlines, so add one extra "pre-render" clock when
        // rendering was enabled at any point during the frame.
        if rendering_happened {
            if let Some(b) = self.cpu.bus_mut() {
                b.clock_mapper_a12_rising_edge();
                mmc3_a12_edges = mmc3_a12_edges.wrapping_add(1);
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

        if LogConfig::global().should_log(LogCategory::CPU, LogLevel::Trace) {
            // Log occasionally to avoid overwhelming the GUI.
            if self.frame_index.is_multiple_of(60) {
                eprintln!(
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
                );
            }
        }

        if LogConfig::global().should_log(LogCategory::CPU, LogLevel::Trace)
            && self.frame_index.is_multiple_of(60)
        {
            let h0 = self.last_stats.pc_hotspots[0];
            let h1 = self.last_stats.pc_hotspots[1];
            let h2 = self.last_stats.pc_hotspots[2];
            eprintln!(
                "NES PC HOT: frame={} [0x{:04X} x{}] [0x{:04X} x{}] [0x{:04X} x{}]",
                self.last_stats.frame_index, h0.pc, h0.count, h1.pc, h1.count, h2.pc, h2.count
            );
        }

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
}
