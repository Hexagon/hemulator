//! NES Mapper implementations
//!
//! This module contains implementations of various NES cartridge mappers
//! that handle PRG/CHR banking and other cartridge hardware features.

mod mmc1;
mod mmc3;
mod nrom;
mod uxrom;

pub use mmc1::Mmc1;
pub use mmc3::Mmc3;
pub use nrom::Nrom;
pub use uxrom::Uxrom;

use crate::cartridge::{Cartridge, Mirroring};
use crate::ppu::Ppu;

/// Unified mapper enum that dispatches to specific implementations
#[derive(Debug)]
pub enum Mapper {
    Nrom(Nrom),
    Mmc3(Mmc3),
    Mmc1(Mmc1),
    Uxrom(Uxrom),
}

impl Mapper {
    /// Create a mapper from a cartridge, configuring the PPU as needed
    pub fn from_cart(cart: Cartridge, ppu: &mut Ppu) -> Self {
        match cart.mapper {
            4 => Mapper::Mmc3(Mmc3::new(cart, ppu)),
            1 => Mapper::Mmc1(Mmc1::new(cart, ppu)),
            2 => Mapper::Uxrom(Uxrom::new(cart, ppu)),
            _ => Mapper::Nrom(Nrom::new(cart)),
        }
    }

    /// Read from PRG ROM/RAM address space
    pub fn read_prg(&self, addr: u16) -> u8 {
        match self {
            Mapper::Nrom(m) => m.read_prg(addr),
            Mapper::Mmc3(m) => m.read_prg(addr),
            Mapper::Mmc1(m) => m.read_prg(addr),
            Mapper::Uxrom(m) => m.read_prg(addr),
        }
    }

    /// Write to PRG ROM/RAM address space (for mapper registers)
    pub fn write_prg(&mut self, addr: u16, val: u8, ppu: &mut Ppu) {
        match self {
            Mapper::Nrom(_) => {
                // NROM ignores PRG writes
                let _ = (addr, val, ppu);
            }
            Mapper::Mmc3(m) => m.write_prg(addr, val, ppu),
            Mapper::Mmc1(m) => m.write_prg(addr, val, ppu),
            Mapper::Uxrom(m) => m.write_prg(addr, val, ppu),
        }
    }

    /// Get reference to the full PRG ROM
    pub fn prg_rom(&self) -> &[u8] {
        match self {
            Mapper::Nrom(m) => m.prg_rom(),
            Mapper::Mmc3(m) => m.prg_rom(),
            Mapper::Mmc1(m) => m.prg_rom(),
            Mapper::Uxrom(m) => m.prg_rom(),
        }
    }

    /// Check and clear pending IRQ flag (for mappers with IRQ support)
    pub fn take_irq_pending(&mut self) -> bool {
        match self {
            Mapper::Nrom(_) => false,
            Mapper::Mmc3(m) => m.take_irq_pending(),
            Mapper::Mmc1(_) => false,
            Mapper::Uxrom(_) => false,
        }
    }

    /// Notify mapper of PPU A12 line transitions (for IRQ timing)
    pub fn notify_a12(&mut self, a12_high: bool) {
        if let Mapper::Mmc3(m) = self {
            m.notify_a12(a12_high);
        }
    }
}
