use crate::cartridge::Cartridge;
use crate::ppu::Ppu;
#[cfg(test)]
use emu_core::apu::TimingMode;

/// Color Dreams (Mapper 11) - Simple PRG and CHR bank switching
///
/// Used primarily in unlicensed Color Dreams and Wisdom Tree games.
/// Supports up to 4 PRG banks (32KB each) and 16 CHR banks (8KB each).
/// Bank selection is via writes to $8000-$FFFF.
#[derive(Debug)]
pub struct ColorDreams {
    prg_rom: Vec<u8>,
    chr_rom: Vec<u8>,
    prg_bank: u8,
    chr_bank: u8,
}

impl ColorDreams {
    pub fn new(cart: Cartridge, ppu: &mut Ppu) -> Self {
        ppu.set_mirroring(cart.mirroring);
        let cd = Self {
            prg_rom: cart.prg_rom,
            chr_rom: cart.chr_rom,
            prg_bank: 0,
            chr_bank: 0,
        };
        cd.update_chr_mapping(ppu);
        cd
    }

    fn prg_bank_count(&self) -> usize {
        std::cmp::max(1, self.prg_rom.len() / 0x8000)
    }

    fn chr_bank_count(&self) -> usize {
        std::cmp::max(1, self.chr_rom.len() / 0x2000)
    }

    fn update_chr_mapping(&self, ppu: &mut Ppu) {
        if ppu.chr.len() < 0x2000 {
            ppu.chr.resize(0x2000, 0);
        }

        // CHR-RAM carts skip copying since PPU owns the RAM view.
        if self.chr_rom.is_empty() {
            return;
        }

        let bank = (self.chr_bank as usize) % self.chr_bank_count();
        let src_start = bank * 0x2000;
        let src_end = src_start + 0x2000;

        if src_end <= self.chr_rom.len() {
            ppu.chr[0..0x2000].copy_from_slice(&self.chr_rom[src_start..src_end]);
        } else {
            // Clear CHR if bank out of range
            for b in &mut ppu.chr[0..0x2000] {
                *b = 0;
            }
        }
    }

    pub fn read_prg(&self, addr: u16) -> u8 {
        let bank = (self.prg_bank as usize) % self.prg_bank_count();
        let offset = (addr as usize) & 0x7FFF;
        let idx = bank * 0x8000 + offset;
        self.prg_rom.get(idx).copied().unwrap_or(0)
    }

    pub fn write_prg(&mut self, addr: u16, val: u8, ppu: &mut Ppu, _cpu_cycles: u64) {
        if (0x8000..=0xFFFF).contains(&addr) {
            // Bits 0-1: PRG bank (32KB)
            // Bits 4-7: CHR bank (8KB) - some sources say bits 2-5, we'll use all high bits
            self.prg_bank = val & 0x03;
            self.chr_bank = (val >> 4) & 0x0F;
            self.update_chr_mapping(ppu);
        }
    }

    pub fn prg_rom(&self) -> &[u8] {
        &self.prg_rom
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cartridge::Mirroring;

    #[test]
    fn colordreams_prg_bank_switching() {
        // Create 4 banks of 32KB each
        let mut prg = vec![0; 0x20000]; // 128KB = 4 banks
        prg[0] = 0x11; // Bank 0 start
        prg[0x8000] = 0x22; // Bank 1 start
        prg[0x10000] = 0x33; // Bank 2 start
        prg[0x18000] = 0x44; // Bank 3 start

        let cart = Cartridge {
            prg_rom: prg,
            chr_rom: vec![0; 0x2000],
            mapper: 11,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Vertical,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Vertical);
        let mut cd = ColorDreams::new(cart, &mut ppu);

        // Initially bank 0
        assert_eq!(cd.read_prg(0x8000), 0x11);

        // Switch to bank 1 (bits 0-1 = 01)
        cd.write_prg(0x8000, 0x01, &mut ppu, 0);
        assert_eq!(cd.read_prg(0x8000), 0x22);

        // Switch to bank 2
        cd.write_prg(0x8000, 0x02, &mut ppu, 0);
        assert_eq!(cd.read_prg(0x8000), 0x33);

        // Switch to bank 3
        cd.write_prg(0x8000, 0x03, &mut ppu, 0);
        assert_eq!(cd.read_prg(0x8000), 0x44);
    }

    #[test]
    fn colordreams_chr_bank_switching() {
        // Create 2 banks of CHR ROM
        let mut chr = vec![0; 0x4000];
        chr[0] = 0xAA; // Bank 0 start
        chr[0x2000] = 0xBB; // Bank 1 start

        let cart = Cartridge {
            prg_rom: vec![0; 0x8000], // 32KB PRG
            chr_rom: chr,
            mapper: 11,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Horizontal,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Horizontal);
        let mut cd = ColorDreams::new(cart, &mut ppu);

        // Initially bank 0
        assert_eq!(ppu.chr[0], 0xAA);

        // Switch to CHR bank 1 (bits 4-7, so 0x10)
        cd.write_prg(0x8000, 0x10, &mut ppu, 0);
        assert_eq!(ppu.chr[0], 0xBB);
    }

    #[test]
    fn colordreams_combined_switching() {
        // Test simultaneous PRG and CHR switching
        let mut prg = vec![0; 0x10000]; // 2 PRG banks
        prg[0] = 0x11;
        prg[0x8000] = 0x22;

        let mut chr = vec![0; 0x4000]; // 2 CHR banks
        chr[0] = 0xAA;
        chr[0x2000] = 0xBB;

        let cart = Cartridge {
            prg_rom: prg,
            chr_rom: chr,
            mapper: 11,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Vertical,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Vertical);
        let mut cd = ColorDreams::new(cart, &mut ppu);

        // Switch to PRG bank 1 and CHR bank 1 (0x11)
        cd.write_prg(0x8000, 0x11, &mut ppu, 0);
        assert_eq!(cd.read_prg(0x8000), 0x22);
        assert_eq!(ppu.chr[0], 0xBB);
    }
}
