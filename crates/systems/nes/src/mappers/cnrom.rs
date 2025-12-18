use crate::cartridge::Cartridge;
use crate::ppu::Ppu;

/// CNROM (Mapper 3) - Simple CHR bank switching
///
/// CNROM allows switching between multiple 8KB CHR-ROM banks.
/// The entire CHR address space ($0000-$1FFF) is swapped at once.
/// PRG-ROM uses NROM-style addressing (16KB or 32KB, mirrored as needed).
#[derive(Debug)]
pub struct Cnrom {
    prg_rom: Vec<u8>,
    chr_rom: Vec<u8>,
    chr_bank: u8,
}

impl Cnrom {
    pub fn new(cart: Cartridge, ppu: &mut Ppu) -> Self {
        ppu.set_mirroring(cart.mirroring);
        let cnrom = Self {
            prg_rom: cart.prg_rom,
            chr_rom: cart.chr_rom,
            chr_bank: 0,
        };
        cnrom.update_chr_mapping(ppu);
        cnrom
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
        let prg = &self.prg_rom;
        let len = prg.len();
        if len == 0 {
            return 0;
        }
        // NROM-style PRG addressing: mirror 16KB to both halves if needed
        let off = if len == 0x4000 {
            (addr as usize - 0x8000) % 0x4000
        } else {
            (addr as usize - 0x8000) % len
        };
        prg[off]
    }

    pub fn write_prg(&mut self, addr: u16, val: u8, ppu: &mut Ppu) {
        if (0x8000..=0xFFFF).contains(&addr) {
            // Select CHR bank (lower 2 bits typically, but some carts use more)
            // We'll mask to support up to 256 banks (8 bits)
            self.chr_bank = val;
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
    fn cnrom_chr_bank_switching() {
        // Create 2 banks of CHR ROM
        let mut chr = vec![0; 0x4000];
        chr[0] = 0x11; // Bank 0 start
        chr[0x2000] = 0x22; // Bank 1 start

        let cart = Cartridge {
            prg_rom: vec![0x42; 0x8000], // 32KB PRG
            chr_rom: chr,
            mapper: 3,
            mirroring: Mirroring::Vertical,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Vertical);
        let mut cnrom = Cnrom::new(cart, &mut ppu);

        // Initially bank 0 should be selected
        assert_eq!(ppu.chr[0], 0x11);
        assert_eq!(ppu.chr[0x1000], 0x00); // Later in bank 0

        // Switch to bank 1
        cnrom.write_prg(0x8000, 1, &mut ppu);
        assert_eq!(ppu.chr[0], 0x22);
    }

    #[test]
    fn cnrom_prg_rom_nrom_style() {
        let cart = Cartridge {
            prg_rom: vec![0x42; 0x4000], // 16KB PRG
            chr_rom: vec![0; 0x2000],
            mapper: 3,
            mirroring: Mirroring::Horizontal,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Horizontal);
        let cnrom = Cnrom::new(cart, &mut ppu);

        // 16KB PRG should mirror at both halves
        assert_eq!(cnrom.read_prg(0x8000), 0x42);
        assert_eq!(cnrom.read_prg(0xC000), 0x42);
        assert_eq!(cnrom.read_prg(0xFFFF), 0x42);
    }

    #[test]
    fn cnrom_32kb_prg() {
        let mut prg = vec![0; 0x8000]; // 32KB PRG
        prg[0] = 0x11;
        prg[0x4000] = 0x22;

        let cart = Cartridge {
            prg_rom: prg,
            chr_rom: vec![0; 0x2000],
            mapper: 3,
            mirroring: Mirroring::Horizontal,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Horizontal);
        let cnrom = Cnrom::new(cart, &mut ppu);

        // 32KB should not mirror
        assert_eq!(cnrom.read_prg(0x8000), 0x11);
        assert_eq!(cnrom.read_prg(0xC000), 0x22);
    }

    #[test]
    fn cnrom_multiple_banks() {
        // Create 4 banks of CHR ROM
        let mut chr = vec![0; 0x8000];
        chr[0] = 0x11; // Bank 0
        chr[0x2000] = 0x22; // Bank 1
        chr[0x4000] = 0x33; // Bank 2
        chr[0x6000] = 0x44; // Bank 3

        let cart = Cartridge {
            prg_rom: vec![0; 0x8000],
            chr_rom: chr,
            mapper: 3,
            mirroring: Mirroring::Horizontal,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Horizontal);
        let mut cnrom = Cnrom::new(cart, &mut ppu);

        // Test all 4 banks
        for bank in 0..4 {
            cnrom.write_prg(0x8000, bank, &mut ppu);
            let expected = 0x11 + (bank * 0x11);
            assert_eq!(ppu.chr[0], expected);
        }
    }
}
