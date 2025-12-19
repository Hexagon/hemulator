//! Minimal NES system skeleton for wiring into the core.

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

use crate::bus::Bus;
use crate::cartridge::Mirroring;
use bus::NesBus;
use cpu::NesCpu;
use emu_core::{apu::TimingMode, types::Frame, MountPointInfo, System};
use ppu::Ppu;
use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Debug, Clone)]
pub struct DebugInfo {
    pub timing_mode: TimingMode,
    pub mapper_name: String,
    pub mapper_number: u8,
    pub prg_banks: usize,
    pub chr_banks: usize,
}

fn trace_pc_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        matches!(
            std::env::var("EMU_TRACE_PC").as_deref(),
            Ok("1") | Ok("true") | Ok("TRUE")
        )
    })
}

fn log_irq() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        matches!(
            std::env::var("EMU_LOG_IRQ").as_deref(),
            Ok("1") | Ok("true") | Ok("TRUE")
        )
    })
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PcHotspot {
    pub pc: u16,
    pub count: u16,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct RuntimeStats {
    pub frame_index: u64,
    pub cpu_steps: u32,
    pub cpu_cycles: u32,
    pub irqs: u32,
    pub nmis: u32,
    pub mmc3_a12_edges: u32,
    pub ppu_ctrl: u8,
    pub ppu_mask: u8,
    pub ppu_vblank: bool,
    pub pc: u16,
    pub vec_reset: u16,
    pub vec_nmi: u16,
    pub vec_irq: u16,
    pub pc_hotspots: [PcHotspot; 3],
}

#[derive(Debug)]
pub struct NesSystem {
    cpu: NesCpu,
    timing: TimingMode,
    cartridge_loaded: bool,
    frame_index: u64,
    last_stats: RuntimeStats,
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
#[error("NES error")]
pub struct NesError;

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

        let mut pc_hist: Option<HashMap<u16, u16>> = if trace_pc_enabled() {
            Some(HashMap::with_capacity(1024))
        } else {
            None
        };

        // Prepare an output frame and render scanlines incrementally during visible time.
        let mut frame = Frame::new(256, 240);
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
                            b.ppu.render_scanline(rendered_scanlines, &mut frame);
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
                if log_irq() {
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
                    b.ppu.render_scanline(rendered_scanlines, &mut frame);
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

        // VBlank end
        if let Some(b) = self.cpu.bus_mut() {
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

        if std::env::var("EMU_TRACE_NES").is_ok() {
            // Log occasionally to avoid overwhelming the GUI.
            if (self.frame_index % 60) == 0 {
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

        if trace_pc_enabled() {
            if (self.frame_index % 60) == 0 {
                let h0 = self.last_stats.pc_hotspots[0];
                let h1 = self.last_stats.pc_hotspots[1];
                let h2 = self.last_stats.pc_hotspots[2];
                eprintln!(
                    "NES PC HOT: frame={} [0x{:04X} x{}] [0x{:04X} x{}] [0x{:04X} x{}]",
                    self.last_stats.frame_index, h0.pc, h0.count, h1.pc, h1.count, h2.pc, h2.count
                );
            }
        }

        Ok(frame)
    }

    fn save_state(&self) -> serde_json::Value {
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
            return Err(NesError);
        }
        self.load_rom(data).map_err(|_| NesError)
    }

    fn unmount(&mut self, mount_point_id: &str) -> Result<(), Self::Error> {
        if mount_point_id != "Cartridge" {
            return Err(NesError);
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
        let mut sys = NesSystem::default();

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
}
