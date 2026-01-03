use crate::cartridge::Cartridge;
use crate::ppu::Ppu;
#[cfg(test)]
use emu_core::apu::TimingMode;

/// GxROM (Mapper 66) - Simple dual PRG/CHR bank switching
///
/// GxROM allows switching between 32KB PRG banks and 8KB CHR banks.
/// Both banks are selected via a single write to $8000-$FFFF:
/// - Bits 0-1: Select 32KB PRG bank
/// - Bits 4-5: Select 8KB CHR bank
///
/// Used in games like SMB + Duck Hunt, Doraemon, etc.
#[derive(Debug)]
pub struct Gxrom {
    prg_rom: Vec<u8>,
    chr_rom: Vec<u8>,
    prg_bank: u8,
    chr_bank: u8,
}

impl Gxrom {
    pub fn new(cart: Cartridge, ppu: &mut Ppu) -> Self {
        // GxROM uses fixed mirroring from the header
        ppu.set_mirroring(cart.mirroring);
        let gxrom = Self {
            prg_rom: cart.prg_rom,
            chr_rom: cart.chr_rom,
            prg_bank: 0,
            chr_bank: 0,
        };
        gxrom.update_chr_mapping(ppu);
        gxrom
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

        // CHR-RAM carts skip copying since PPU owns the RAM view
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
        let offset = (addr as usize - 0x8000) % 0x8000;
        let idx = bank * 0x8000 + offset;
        self.prg_rom.get(idx).copied().unwrap_or(0)
    }

    pub fn write_prg(&mut self, addr: u16, val: u8, ppu: &mut Ppu, _cpu_cycles: u64) {
        if (0x8000..=0xFFFF).contains(&addr) {
            // Bits 0-1: PRG bank (32KB)
            // Bits 4-5: CHR bank (8KB)
            self.prg_bank = val & 0x03;
            self.chr_bank = (val >> 4) & 0x03;
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
    fn gxrom_prg_bank_switching() {
        // Create 2 banks of 32KB PRG each
        let mut prg = vec![0; 0x10000];
        prg[0] = 0x11; // Bank 0 start
        prg[0x8000] = 0x22; // Bank 1 start

        let cart = Cartridge {
            prg_rom: prg,
            chr_rom: vec![0; 0x2000],
            mapper: 66,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Vertical,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Vertical);
        let mut gxrom = Gxrom::new(cart, &mut ppu);

        // Initially bank 0 should be selected
        assert_eq!(gxrom.read_prg(0x8000), 0x11);
        assert_eq!(gxrom.read_prg(0xFFFF), 0x00);

        // Switch to bank 1 (write value with bits 0-1 = 01)
        gxrom.write_prg(0x8000, 0x01, &mut ppu, 0);
        assert_eq!(gxrom.read_prg(0x8000), 0x22);
    }

    #[test]
    fn gxrom_chr_bank_switching() {
        // Create 2 banks of 8KB CHR each
        let mut chr = vec![0; 0x4000];
        chr[0] = 0x33; // Bank 0 start
        chr[0x2000] = 0x44; // Bank 1 start

        let cart = Cartridge {
            prg_rom: vec![0; 0x8000],
            chr_rom: chr,
            mapper: 66,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Horizontal,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Horizontal);
        let mut gxrom = Gxrom::new(cart, &mut ppu);

        // Initially bank 0 should be selected
        assert_eq!(ppu.chr[0], 0x33);

        // Switch to bank 1 (write value with bits 4-5 = 01, so value 0x10)
        gxrom.write_prg(0x8000, 0x10, &mut ppu, 0);
        assert_eq!(ppu.chr[0], 0x44);
    }

    #[test]
    fn gxrom_dual_bank_switching() {
        // Create 4 banks of PRG (32KB each) and 4 banks of CHR (8KB each)
        let mut prg = vec![0; 0x20000];
        prg[0] = 0x11;
        prg[0x8000] = 0x22;
        prg[0x10000] = 0x33;
        prg[0x18000] = 0x44;

        let mut chr = vec![0; 0x8000];
        chr[0] = 0xAA;
        chr[0x2000] = 0xBB;
        chr[0x4000] = 0xCC;
        chr[0x6000] = 0xDD;

        let cart = Cartridge {
            prg_rom: prg,
            chr_rom: chr,
            mapper: 66,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Vertical,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Vertical);
        let mut gxrom = Gxrom::new(cart, &mut ppu);

        // Test switching both PRG and CHR
        // PRG bank 2, CHR bank 3: bits 0-1 = 10 (2), bits 4-5 = 11 (3)
        // Value = 0b00110010 = 0x32
        gxrom.write_prg(0x8000, 0x32, &mut ppu, 0);
        assert_eq!(gxrom.read_prg(0x8000), 0x33); // PRG bank 2
        assert_eq!(ppu.chr[0], 0xDD); // CHR bank 3

        // PRG bank 1, CHR bank 1
        gxrom.write_prg(0x8000, 0x11, &mut ppu, 0);
        assert_eq!(gxrom.read_prg(0x8000), 0x22); // PRG bank 1
        assert_eq!(ppu.chr[0], 0xBB); // CHR bank 1
    }

    #[test]
    fn gxrom_bank_masking() {
        // Create 2 PRG banks and 2 CHR banks
        let mut prg = vec![0; 0x10000];
        prg[0] = 0x11;
        prg[0x8000] = 0x22;

        let mut chr = vec![0; 0x4000];
        chr[0] = 0xAA;
        chr[0x2000] = 0xBB;

        let cart = Cartridge {
            prg_rom: prg,
            chr_rom: chr,
            mapper: 66,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Vertical,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Vertical);
        let mut gxrom = Gxrom::new(cart, &mut ppu);

        // Try to select bank 3, should wrap to bank 1 (3 % 2 = 1)
        // PRG bank 3, CHR bank 3: 0x33
        gxrom.write_prg(0x8000, 0x33, &mut ppu, 0);
        assert_eq!(gxrom.read_prg(0x8000), 0x22); // Wraps to bank 1
        assert_eq!(ppu.chr[0], 0xBB); // Wraps to bank 1
    }
}
