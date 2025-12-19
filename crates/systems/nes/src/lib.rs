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

use crate::cartridge::Mirroring;
use bus::NesBus;
use cpu::NesCpu;
use emu_core::{types::Frame, System, MountPointInfo, apu::TimingMode, apu::TimingMode};
use ppu::Ppu;

#[derive(Debug, Clone)]
pub struct DebugInfo {
    pub timing_mode: TimingMode,
    pub mapper_name: String,
    pub mapper_number: u8,
    pub prg_banks: usize,
    pub chr_banks: usize,
}

#[derive(Debug)]
pub struct NesSystem {
    cpu: NesCpu,
    timing: TimingMode,
    cartridge_loaded: bool,
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

    /// Get debug information for the overlay
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
            // CHR size from PPU
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

    /// Return debug information useful for inspecting execution state.
    pub fn debug_state(&self) -> serde_json::Value {
        let pc = self.cpu.pc();
        let cycles = self.cpu.cycles();
        let mut vram_sample: Vec<u8> = Vec::new();
        let mut chr_sample: Vec<u8> = Vec::new();
        let mut vram_nonzero: usize = 0;
        let mut ppu_ctrl: u8 = 0;
        let mut ppu_mask: u8 = 0;
        let mut ppu_scroll: (u8, u8) = (0, 0);
        let mut ppu_addr: u16 = 0;
        let mut nmi_vec: u16 = 0;
        let mut reset_vec: u16 = 0;
        let mut irq_vec: u16 = 0;

        if let Some(b) = self.cpu.bus() {
            // take a small sample of VRAM and CHR for quick inspection
            let vlen = std::cmp::min(64, b.ppu.vram.len());
            vram_sample = b.ppu.vram[..vlen].to_vec();
            vram_nonzero = b.ppu.vram.iter().filter(|&&x| x != 0).count();
            chr_sample = b.ppu.chr.iter().take(64).cloned().collect();

            ppu_ctrl = b.ppu.ctrl();
            ppu_mask = b.ppu.mask();
            ppu_scroll = b.ppu.scroll();
            ppu_addr = b.ppu.vram_addr.get();

            if let Some(prg) = b.prg_rom() {
                let base = if prg.len() == 0x4000 {
                    0x0000
                } else {
                    prg.len().saturating_sub(0x4000)
                };
                let read_vec = |off: usize| -> u16 {
                    if prg.len() >= base + off + 2 {
                        (prg[base + off] as u16) | ((prg[base + off + 1] as u16) << 8)
                    } else {
                        0
                    }
                };
                nmi_vec = read_vec(0x3FFA);
                reset_vec = read_vec(0x3FFC);
                irq_vec = read_vec(0x3FFE);
            }
        }

        serde_json::json!({
            "pc": pc,
            "cycles": cycles,
            "ppu": {
                "ctrl": ppu_ctrl,
                "mask": ppu_mask,
                "scroll_x": ppu_scroll.0,
                "scroll_y": ppu_scroll.1,
                "vram_addr": ppu_addr,
                "nmi_enabled": (ppu_ctrl & 0x80) != 0,
                "bg_enabled": (ppu_mask & 0x08) != 0,
                "sprites_enabled": (ppu_mask & 0x10) != 0
            },
            "vectors": {
                "nmi": nmi_vec,
                "reset": reset_vec,
                "irq": irq_vec
            },
            "vram_sample": vram_sample,
            "vram_nonzero": vram_nonzero,
            "chr_sample_len": chr_sample.len(),
            "chr_sample": chr_sample,
        })
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

        // Visible portion (VBlank low)
        if let Some(b) = self.cpu.bus_mut() {
            b.ppu.set_vblank(false);
        }
        let mut cycles = 0u32;
        while cycles < visible_cycles {
            let used = self.cpu.step();
            cycles = cycles.wrapping_add(used);

            // Mapper IRQs now clocked by PPU A12 edges directly from PPU fetches.
            if let Some(b) = self.cpu.bus_mut() {
                if b.take_irq_pending() {
                    self.cpu.trigger_irq();
                }
            }
        }

        // Snapshot the frame at the end of visible time.
        let frame = if let Some(b) = self.cpu.bus() {
            b.ppu.render_frame()
        } else {
            Frame::new(256, 240)
        };

        // VBlank start
        if let Some(b) = self.cpu.bus_mut() {
            b.ppu.set_vblank(true);
            if b.ppu.nmi_enabled() {
                self.cpu.trigger_nmi();
            }
        }

        // Run the rest of the frame (VBlank time).
        while cycles < cycles_per_frame {
            cycles = cycles.wrapping_add(self.cpu.step());

            // Check for mapper IRQs during VBlank as well
            if let Some(b) = self.cpu.bus_mut() {
                if b.take_irq_pending() {
                    self.cpu.trigger_irq();
                }
            }
        }

        // VBlank end
        if let Some(b) = self.cpu.bus_mut() {
            b.ppu.set_vblank(false);
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
        
        // Note: ROM verification is handled by the frontend via ROM hash
        // Actual state restoration would go here
        // For now, this is a minimal implementation that validates the state structure
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
