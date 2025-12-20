//! NES Mapper implementations
//!
//! This module contains implementations of various NES cartridge mappers
//! that handle PRG/CHR banking and other cartridge hardware features.

mod axrom;
mod bnrom;
mod camerica;
mod cnrom;
mod colordreams;
mod gxrom;
mod mmc1;
mod mmc2;
mod mmc3;
mod mmc4;
mod namco118;
mod nina;
mod nrom;
mod uxrom;

pub use axrom::Axrom;
pub use bnrom::Bnrom;
pub use camerica::Camerica;
pub use cnrom::Cnrom;
pub use colordreams::ColorDreams;
pub use gxrom::Gxrom;
pub use mmc1::Mmc1;
pub use mmc2::Mmc2;
pub use mmc3::Mmc3;
pub use mmc4::Mmc4;
pub use namco118::Namco118;
pub use nina::Nina;
pub use nrom::Nrom;
pub use uxrom::Uxrom;

use crate::cartridge::Cartridge;
use crate::ppu::Ppu;

/// Unified mapper enum that dispatches to specific implementations
#[derive(Debug)]
pub enum Mapper {
    Nrom(Nrom),
    Mmc1(Mmc1),
    Uxrom(Uxrom),
    Cnrom(Cnrom),
    Mmc3(Mmc3),
    Axrom(Axrom),
    Mmc2(Mmc2),
    Mmc4(Mmc4),
    ColorDreams(ColorDreams),
    Gxrom(Gxrom),
    Camerica(Camerica),
    Namco118(Namco118),
    Bnrom(Bnrom),
    Nina(Nina),
}

impl Mapper {
    /// Create a mapper from a cartridge, configuring the PPU as needed
    pub fn from_cart(cart: Cartridge, ppu: &mut Ppu) -> Self {
        match cart.mapper {
            1 => Mapper::Mmc1(Mmc1::new(cart, ppu)),
            2 => Mapper::Uxrom(Uxrom::new(cart, ppu)),
            3 => Mapper::Cnrom(Cnrom::new(cart, ppu)),
            4 => Mapper::Mmc3(Mmc3::new(cart, ppu)),
            7 => Mapper::Axrom(Axrom::new(cart, ppu)),
            9 => Mapper::Mmc2(Mmc2::new(cart, ppu)),
            10 => Mapper::Mmc4(Mmc4::new(cart, ppu)),
            11 => Mapper::ColorDreams(ColorDreams::new(cart, ppu)),
            34 => Mapper::Bnrom(Bnrom::new(cart, ppu)),
            66 => Mapper::Gxrom(Gxrom::new(cart, ppu)),
            71 => Mapper::Camerica(Camerica::new(cart, ppu)),
            79 => Mapper::Nina(Nina::new(cart, ppu)),
            206 => Mapper::Namco118(Namco118::new(cart, ppu)),
            _ => Mapper::Nrom(Nrom::new(cart)),
        }
    }

    /// Read from PRG ROM/RAM address space
    pub fn read_prg(&self, addr: u16) -> u8 {
        match self {
            Mapper::Nrom(m) => m.read_prg(addr),
            Mapper::Mmc1(m) => m.read_prg(addr),
            Mapper::Uxrom(m) => m.read_prg(addr),
            Mapper::Cnrom(m) => m.read_prg(addr),
            Mapper::Mmc3(m) => m.read_prg(addr),
            Mapper::Axrom(m) => m.read_prg(addr),
            Mapper::Mmc2(m) => m.read_prg(addr),
            Mapper::Mmc4(m) => m.read_prg(addr),
            Mapper::ColorDreams(m) => m.read_prg(addr),
            Mapper::Gxrom(m) => m.read_prg(addr),
            Mapper::Camerica(m) => m.read_prg(addr),
            Mapper::Namco118(m) => m.read_prg(addr),
            Mapper::Bnrom(m) => m.read_prg(addr),
            Mapper::Nina(m) => m.read_prg(addr),
        }
    }

    /// Write to PRG ROM/RAM address space (for mapper registers)
    pub fn write_prg(&mut self, addr: u16, val: u8, ppu: &mut Ppu) {
        match self {
            Mapper::Nrom(_) => {
                // NROM ignores PRG writes
                let _ = (addr, val, ppu);
            }
            Mapper::Mmc1(m) => m.write_prg(addr, val, ppu),
            Mapper::Uxrom(m) => m.write_prg(addr, val, ppu),
            Mapper::Cnrom(m) => m.write_prg(addr, val, ppu),
            Mapper::Mmc3(m) => m.write_prg(addr, val, ppu),
            Mapper::Axrom(m) => m.write_prg(addr, val, ppu),
            Mapper::Mmc2(m) => m.write_prg(addr, val, ppu),
            Mapper::Mmc4(m) => m.write_prg(addr, val, ppu),
            Mapper::ColorDreams(m) => m.write_prg(addr, val, ppu),
            Mapper::Gxrom(m) => m.write_prg(addr, val, ppu),
            Mapper::Camerica(m) => m.write_prg(addr, val, ppu),
            Mapper::Namco118(m) => m.write_prg(addr, val, ppu),
            Mapper::Bnrom(m) => m.write_prg(addr, val, ppu),
            Mapper::Nina(m) => m.write_prg(addr, val, ppu),
        }
    }

    /// Get reference to the full PRG ROM
    pub fn prg_rom(&self) -> &[u8] {
        match self {
            Mapper::Nrom(m) => m.prg_rom(),
            Mapper::Mmc1(m) => m.prg_rom(),
            Mapper::Uxrom(m) => m.prg_rom(),
            Mapper::Cnrom(m) => m.prg_rom(),
            Mapper::Mmc3(m) => m.prg_rom(),
            Mapper::Axrom(m) => m.prg_rom(),
            Mapper::Mmc2(m) => m.prg_rom(),
            Mapper::Mmc4(m) => m.prg_rom(),
            Mapper::ColorDreams(m) => m.prg_rom(),
            Mapper::Gxrom(m) => m.prg_rom(),
            Mapper::Camerica(m) => m.prg_rom(),
            Mapper::Namco118(m) => m.prg_rom(),
            Mapper::Bnrom(m) => m.prg_rom(),
            Mapper::Nina(m) => m.prg_rom(),
        }
    }

    /// Check and clear pending IRQ flag (for mappers with IRQ support)
    pub fn take_irq_pending(&mut self) -> bool {
        match self {
            Mapper::Nrom(_) => false,
            Mapper::Mmc1(_) => false,
            Mapper::Uxrom(_) => false,
            Mapper::Cnrom(_) => false,
            Mapper::Mmc3(m) => m.take_irq_pending(),
            Mapper::Axrom(_) => false,
            Mapper::Mmc2(_) => false,
            Mapper::Mmc4(_) => false,
            Mapper::ColorDreams(_) => false,
            Mapper::Gxrom(_) => false,
            Mapper::Camerica(_) => false,
            Mapper::Namco118(_) => false,
            Mapper::Bnrom(_) => false,
            Mapper::Nina(_) => false,
        }
    }

    /// Notify mapper of PPU A12 line transitions (for IRQ timing)
    pub fn notify_a12(&mut self, a12_high: bool) {
        if let Mapper::Mmc3(m) = self {
            m.notify_a12(a12_high);
        }
    }

    /// Notify mapper of PPU CHR reads (for MMC2/MMC4 latch switching)
    pub fn notify_chr_read(&mut self, addr: u16) {
        match self {
            Mapper::Mmc2(m) => m.notify_chr_read(addr),
            Mapper::Mmc4(m) => m.notify_chr_read(addr),
            _ => {}
        }
    }

    /// Apply pending CHR updates for MMC2/MMC4 after frame rendering
    pub fn apply_chr_update(&mut self, ppu: &mut Ppu) {
        match self {
            Mapper::Mmc2(m) => m.apply_chr_update(ppu),
            Mapper::Mmc4(m) => m.apply_chr_update(ppu),
            _ => {}
        }
    }

    /// Get mapper number
    pub fn mapper_number(&self) -> u8 {
        match self {
            Mapper::Nrom(_) => 0,
            Mapper::Mmc1(_) => 1,
            Mapper::Uxrom(_) => 2,
            Mapper::Cnrom(_) => 3,
            Mapper::Mmc3(_) => 4,
            Mapper::Axrom(_) => 7,
            Mapper::Mmc2(_) => 9,
            Mapper::Mmc4(_) => 10,
            Mapper::ColorDreams(_) => 11,
            Mapper::Bnrom(_) => 34,
            Mapper::Gxrom(_) => 66,
            Mapper::Camerica(_) => 71,
            Mapper::Nina(_) => 79,
            Mapper::Namco118(_) => 206,
        }
    }
}
