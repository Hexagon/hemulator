use crate::cartridge::Cartridge;
use crate::ppu::Ppu;
#[cfg(test)]
use emu_core::apu::TimingMode;

/// NINA-03/NINA-06 (Mapper 79) - AVE mapper with simple PRG/CHR switching
///
/// Used by American Video Entertainment (AVE) for NES cartridges.
/// Simple mapper with discrete logic (74xx series chips), no custom ASIC.
///
/// Hardware characteristics:
/// - NINA-03: 32KB PRG-ROM max, 32KB CHR-ROM max
/// - NINA-06: 64KB PRG-ROM max, 64KB CHR-ROM max
/// - Mirroring: Fixed (hardwired on board)
/// - No WRAM, no battery backup
/// - No IRQ or advanced features
///
/// Bank switching (register at $4100-$5FFF):
/// - Bits 0-2: Select 8KB CHR bank at PPU $0000-$1FFF
/// - Bit 3: Select 32KB PRG bank at CPU $8000-$FFFF
///
/// The unusual register range ($4100-$5FFF) is due to discrete logic
/// implementation rather than a custom mapper ASIC.
///
/// Used in games like Dudes with Attitude, Pyramid, F-15 City War
#[derive(Debug)]
pub struct Nina {
    prg_rom: Vec<u8>,
    chr_rom: Vec<u8>,
    prg_bank: u8,
    chr_bank: u8,
}

impl Nina {
    pub fn new(cart: Cartridge, ppu: &mut Ppu) -> Self {
        // NINA uses fixed mirroring from the header
        ppu.set_mirroring(cart.mirroring);
        let nina = Self {
            prg_rom: cart.prg_rom,
            chr_rom: cart.chr_rom,
            prg_bank: 0,
            chr_bank: 0,
        };
        nina.update_chr_mapping(ppu);
        nina
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

        // CHR-RAM carts skip copying
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
        if self.prg_rom.is_empty() {
            return 0;
        }
        let bank = (self.prg_bank as usize) % self.prg_bank_count();
        let offset = (addr as usize - 0x8000) & 0x7FFF;
        let idx = bank * 0x8000 + offset;
        self.prg_rom.get(idx).copied().unwrap_or(0)
    }

    pub fn write_prg(&mut self, addr: u16, val: u8, ppu: &mut Ppu, _cpu_cycles: u64) {
        // NINA-03/06 register is at $4100-$5FFF
        // This unusual range is due to discrete logic implementation
        if (0x4100..=0x5FFF).contains(&addr) {
            // Bits 0-2: CHR bank (8KB)
            self.chr_bank = val & 0x07;
            // Bit 3: PRG bank (32KB)
            self.prg_bank = (val >> 3) & 0x01;
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
    fn nina_prg_bank_switching() {
        let mut prg = vec![0; 0x10000]; // 2 banks of 32KB each
        prg[0] = 0x11; // Bank 0 start
        prg[0x8000] = 0x22; // Bank 1 start

        let cart = Cartridge {
            prg_rom: prg,
            chr_rom: vec![0; 0x2000],
            mapper: 79,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Vertical,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Vertical);
        let mut nina = Nina::new(cart, &mut ppu);

        // Initially bank 0 should be selected
        assert_eq!(nina.read_prg(0x8000), 0x11);

        // Switch to bank 1 (bit 3 = 1, so value 0x08)
        nina.write_prg(0x4100, 0x08, &mut ppu, 0);
        assert_eq!(nina.read_prg(0x8000), 0x22);

        // Switch back to bank 0
        nina.write_prg(0x5000, 0x00, &mut ppu, 0);
        assert_eq!(nina.read_prg(0x8000), 0x11);
    }

    #[test]
    fn nina_chr_bank_switching() {
        // Create 4 banks of 8KB CHR each
        let mut chr = vec![0; 0x8000];
        chr[0] = 0xAA; // Bank 0
        chr[0x2000] = 0xBB; // Bank 1
        chr[0x4000] = 0xCC; // Bank 2
        chr[0x6000] = 0xDD; // Bank 3

        let cart = Cartridge {
            prg_rom: vec![0; 0x8000],
            chr_rom: chr,
            mapper: 79,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Horizontal,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Horizontal);
        let mut nina = Nina::new(cart, &mut ppu);

        // Initially bank 0 should be selected
        assert_eq!(ppu.chr[0], 0xAA);

        // Switch to bank 1 (bits 0-2 = 001)
        nina.write_prg(0x4100, 0x01, &mut ppu, 0);
        assert_eq!(ppu.chr[0], 0xBB);

        // Switch to bank 2
        nina.write_prg(0x4100, 0x02, &mut ppu, 0);
        assert_eq!(ppu.chr[0], 0xCC);

        // Switch to bank 3
        nina.write_prg(0x4100, 0x03, &mut ppu, 0);
        assert_eq!(ppu.chr[0], 0xDD);
    }

    #[test]
    fn nina_combined_switching() {
        let mut prg = vec![0; 0x10000]; // 2 PRG banks
        prg[0] = 0x11;
        prg[0x8000] = 0x22;

        let mut chr = vec![0; 0x4000]; // 2 CHR banks
        chr[0] = 0xAA;
        chr[0x2000] = 0xBB;

        let cart = Cartridge {
            prg_rom: prg,
            chr_rom: chr,
            mapper: 79,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Vertical,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Vertical);
        let mut nina = Nina::new(cart, &mut ppu);

        // Initially both banks 0
        assert_eq!(nina.read_prg(0x8000), 0x11);
        assert_eq!(ppu.chr[0], 0xAA);

        // Switch PRG to bank 1, CHR to bank 1
        // PRG bank 1 = bit 3 = 1 (0x08)
        // CHR bank 1 = bits 0-2 = 1 (0x01)
        // Combined: 0x08 | 0x01 = 0x09
        nina.write_prg(0x4100, 0x09, &mut ppu, 0);
        assert_eq!(nina.read_prg(0x8000), 0x22);
        assert_eq!(ppu.chr[0], 0xBB);

        // Switch PRG to bank 0, keep CHR at bank 1
        nina.write_prg(0x4100, 0x01, &mut ppu, 0);
        assert_eq!(nina.read_prg(0x8000), 0x11);
        assert_eq!(ppu.chr[0], 0xBB);
    }

    #[test]
    fn nina_register_range() {
        let mut prg = vec![0; 0x10000];
        prg[0] = 0x11;
        prg[0x8000] = 0x22;

        let cart = Cartridge {
            prg_rom: prg,
            chr_rom: vec![0; 0x2000],
            mapper: 79,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Vertical,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Vertical);
        let mut nina = Nina::new(cart, &mut ppu);

        // Test different addresses in the register range
        // $4100 should work
        nina.write_prg(0x4100, 0x08, &mut ppu, 0);
        assert_eq!(nina.read_prg(0x8000), 0x22);

        // $5000 should work
        nina.write_prg(0x5000, 0x00, &mut ppu, 0);
        assert_eq!(nina.read_prg(0x8000), 0x11);

        // $5FFF should work
        nina.write_prg(0x5FFF, 0x08, &mut ppu, 0);
        assert_eq!(nina.read_prg(0x8000), 0x22);

        // $6000 should NOT work (outside range)
        nina.write_prg(0x6000, 0x00, &mut ppu, 0);
        assert_eq!(nina.read_prg(0x8000), 0x22); // Should still be bank 1
    }

    #[test]
    fn nina_chr_bank_wrapping() {
        // Only 2 CHR banks
        let mut chr = vec![0; 0x4000];
        chr[0] = 0xAA;
        chr[0x2000] = 0xBB;

        let cart = Cartridge {
            prg_rom: vec![0; 0x8000],
            chr_rom: chr,
            mapper: 79,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Vertical,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Vertical);
        let mut nina = Nina::new(cart, &mut ppu);

        // Try to select bank 5 (bits 0-2 = 101), should wrap to bank 1 (5 % 2 = 1)
        nina.write_prg(0x4100, 0x05, &mut ppu, 0);
        assert_eq!(ppu.chr[0], 0xBB);
    }
}
