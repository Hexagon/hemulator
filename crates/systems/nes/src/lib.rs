//! Minimal NES system skeleton for wiring into the core.

mod bus;
mod cartridge;
mod cpu;
mod ppu;
mod apu;

use crate::cartridge::Mirroring;
use bus::NesBus;
use cpu::NesCpu;
use emu_core::{types::Frame, System};
use ppu::Ppu;

#[derive(Debug)]
pub struct NesSystem {
    cpu: NesCpu,
}

impl NesSystem {
    /// Set controller 0 or 1 button state (bits 0..7 correspond to controller buttons).
    pub fn set_controller(&mut self, idx: usize, state: u8) {
        if let Some(b) = &mut self.cpu.bus {
            b.set_controller(idx, state);
        }
    }

    /// Get audio samples from the APU
    pub fn get_audio_samples(&mut self, count: usize) -> Vec<i16> {
        if let Some(b) = &mut self.cpu.bus {
            b.apu.generate_samples(count)
        } else {
            vec![0; count]
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
        cpu.bus = Some(Box::new(bus));
        Self { cpu }
    }
}

impl NesSystem {
    /// Load a mapper-0 (NROM) iNES ROM into CPU memory. This writes PRG ROM
    /// into 0x8000.. and mirrors 16KB banks into 0xC000 when necessary.
    pub fn load_rom_from_path<P: AsRef<std::path::Path>>(
        &mut self,
        path: P,
    ) -> Result<(), std::io::Error> {
        let cart = cartridge::Cartridge::from_file(path)?;
        if cart.mapper != 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Unsupported mapper {} (only NROM/0 supported)", cart.mapper),
            ));
        }
        // Map PRG ROM into CPU memory (NROM): 16KB PRG is mirrored, 32KB PRG is direct.
        let prg = &cart.prg_rom;
        let chr = cart.chr_rom.clone();
        match prg.len() {
            0x4000 => {
                // 16KB PRG: load at 0x8000 and mirror at 0xC000
                self.cpu.memory[0x8000..0xC000].copy_from_slice(prg);
                self.cpu.memory[0xC000..0x10000].copy_from_slice(prg);
                let lo = prg[0x3FFC] as u16;
                let hi = prg[0x3FFD] as u16;
                self.cpu.pc = (hi << 8) | lo;
            }
            0x8000 => {
                // 32KB PRG: load at 0x8000
                self.cpu.memory[0x8000..0x10000].copy_from_slice(prg);
                let lo = prg[0x7FFC] as u16;
                let hi = prg[0x7FFD] as u16;
                self.cpu.pc = (hi << 8) | lo;
            }
            _ => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Unsupported PRG ROM size",
                ));
            }
        }

        // create a PPU with CHR and install into a new bus, then attach bus to CPU
        let ppu = Ppu::new(chr, cart.mirroring);
        let mut nb = NesBus::new(ppu);
        nb.install_cart(cart);
        self.cpu.bus = Some(Box::new(nb));
        Ok(())
    }

    /// Return debug information useful for inspecting execution state.
    pub fn debug_state(&self) -> serde_json::Value {
        let pc = self.cpu.pc;
        let cycles = self.cpu.cycles;
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

        if let Some(b) = &self.cpu.bus {
            // take a small sample of VRAM and CHR for quick inspection
            let vlen = std::cmp::min(64, b.ppu.vram.len());
            vram_sample = b.ppu.vram[..vlen].to_vec();
            vram_nonzero = b.ppu.vram.iter().filter(|&&x| x != 0).count();
            chr_sample = b.ppu.chr.iter().take(64).cloned().collect();

            ppu_ctrl = b.ppu.ctrl();
            ppu_mask = b.ppu.mask();
            ppu_scroll = b.ppu.scroll();
            ppu_addr = b.ppu.vram_addr;

            if let Some(cart) = &b.cart {
                let prg = &cart.prg_rom;
                let base = if prg.len() == 0x4000 { 0x0000 } else { 0x4000 };
                let read_vec = |off: usize| -> u16 {
                    if prg.len() >= base + off + 2 {
                        (prg[base + off] as u16) | ((prg[base + off + 1] as u16) << 8)
                    } else {
                        0
                    }
                };
                // Vectors are at end of CPU address space: $FFFA NMI, $FFFC RESET, $FFFE IRQ/BRK.
                // Map to PRG end (mirrored for 16KB).
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
        // Run CPU cycles for one frame (approx. 29780 CPU cycles for NTSC).
        // Model VBlank as the *tail* of the frame and trigger NMI at VBlank start.
        // IMPORTANT: render at the end of the *visible* portion (right before VBlank)
        // so we don't sample while games temporarily disable PPUMASK during their NMI.
        const CYCLES_PER_FRAME: u32 = 29780;
        const VBLANK_CYCLES: u32 = 2500;
        const VISIBLE_CYCLES: u32 = CYCLES_PER_FRAME - VBLANK_CYCLES;

        // Visible portion (VBlank low)
        if let Some(b) = &mut self.cpu.bus {
            b.ppu.set_vblank(false);
        }
        let mut cycles = 0u32;
        while cycles < VISIBLE_CYCLES {
            cycles = cycles.wrapping_add(self.cpu.step());
        }

        // Snapshot the frame at the end of visible time.
        let frame = if let Some(b) = &self.cpu.bus {
            b.ppu.render_frame()
        } else {
            Frame::new(256, 240)
        };

        // VBlank start
        if let Some(b) = &mut self.cpu.bus {
            b.ppu.set_vblank(true);
            if b.ppu.nmi_enabled() {
                self.cpu.trigger_nmi();
            }
        }

        // Run the rest of the frame (VBlank time).
        while cycles < CYCLES_PER_FRAME {
            cycles = cycles.wrapping_add(self.cpu.step());
        }

        // VBlank end
        if let Some(b) = &mut self.cpu.bus {
            b.ppu.set_vblank(false);
        }

        Ok(frame)
    }

    fn save_state(&self) -> serde_json::Value {
        serde_json::json!({ "system": "nes", "version": 1, "a": self.cpu.a })
    }

    fn load_state(&mut self, _v: &serde_json::Value) -> Result<(), serde_json::Error> {
        Ok(())
    }
}
