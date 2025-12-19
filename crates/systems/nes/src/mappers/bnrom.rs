use crate::cartridge::Cartridge;
use crate::ppu::Ppu;
#[cfg(test)]
use emu_core::apu::TimingMode;

/// BNROM (Mapper 34) - Simple 32KB PRG bank switching with CHR-RAM
///
/// BNROM is one of the simplest NES mappers, providing basic bank switching
/// for larger games. It's often used in homebrew due to its simplicity.
///
/// Hardware characteristics:
/// - PRG-ROM: 32KB, 64KB, or 128KB (switchable in 32KB banks)
/// - CHR-RAM: 8KB (no CHR-ROM)
/// - Mirroring: Fixed (hardwired on board)
/// - No WRAM or battery backup
///
/// Bank switching:
/// - CPU $8000-$FFFF: Switchable 32KB PRG bank
/// - Write to $8000-$FFFF: Select PRG bank (low bits only)
/// - PPU $0000-$1FFF: 8KB CHR-RAM (writable)
///
/// Note: Mapper 34 also includes NINA-001, but this implementation
/// focuses on BNROM (NES 2.0 submapper 2), the more common variant.
///
/// Used in games like Deadly Towers, some homebrew titles
#[derive(Debug)]
pub struct Bnrom {
    prg_rom: Vec<u8>,
    bank_select: u8,
}

impl Bnrom {
    pub fn new(cart: Cartridge, ppu: &mut Ppu) -> Self {
        // BNROM uses fixed mirroring from the header
        ppu.set_mirroring(cart.mirroring);
        Self {
            prg_rom: cart.prg_rom,
            bank_select: 0,
        }
    }

    fn prg_bank_count(&self) -> usize {
        std::cmp::max(1, self.prg_rom.len() / 0x8000)
    }

    pub fn read_prg(&self, addr: u16) -> u8 {
        if self.prg_rom.is_empty() {
            return 0;
        }
        let bank = (self.bank_select as usize) % self.prg_bank_count();
        let offset = (addr as usize - 0x8000) & 0x7FFF;
        let idx = bank * 0x8000 + offset;
        self.prg_rom.get(idx).copied().unwrap_or(0)
    }

    pub fn write_prg(&mut self, addr: u16, val: u8, _ppu: &mut Ppu) {
        if (0x8000..=0xFFFF).contains(&addr) {
            // Select 32KB bank
            // Only use lower bits based on ROM size
            // For 128KB (4 banks), use 2 bits; for 64KB (2 banks), use 1 bit
            let bank_mask = (self.prg_bank_count() as u8).saturating_sub(1);
            self.bank_select = val & bank_mask;
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
    fn bnrom_bank_switching() {
        let mut prg = vec![0; 0x10000]; // 2 banks of 32KB each (64KB total)
        prg[0] = 0x11; // Bank 0 start
        prg[0x8000] = 0x22; // Bank 1 start

        let cart = Cartridge {
            prg_rom: prg,
            chr_rom: vec![], // BNROM uses CHR-RAM
            mapper: 34,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Vertical,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Vertical);
        let mut bnrom = Bnrom::new(cart, &mut ppu);

        // Initially bank 0 should be selected
        assert_eq!(bnrom.read_prg(0x8000), 0x11);
        assert_eq!(bnrom.read_prg(0xFFFF), 0x00);

        // Switch to bank 1
        bnrom.write_prg(0x8000, 1, &mut ppu);
        assert_eq!(bnrom.read_prg(0x8000), 0x22);
        assert_eq!(bnrom.read_prg(0xFFFF), 0x00);
    }

    #[test]
    fn bnrom_32kb_single_bank() {
        let mut prg = vec![0; 0x8000]; // Single 32KB bank
        prg[0] = 0xAA;
        prg[0x7FFF] = 0xBB;

        let cart = Cartridge {
            prg_rom: prg,
            chr_rom: vec![],
            mapper: 34,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Horizontal,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Horizontal);
        let bnrom = Bnrom::new(cart, &mut ppu);

        // With only one bank, should always read from bank 0
        assert_eq!(bnrom.read_prg(0x8000), 0xAA);
        assert_eq!(bnrom.read_prg(0xFFFF), 0xBB);
    }

    #[test]
    fn bnrom_128kb_four_banks() {
        let mut prg = vec![0; 0x20000]; // 4 banks of 32KB each (128KB total)
        for i in 0..4 {
            prg[i * 0x8000] = (i + 1) as u8;
        }

        let cart = Cartridge {
            prg_rom: prg,
            chr_rom: vec![],
            mapper: 34,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Vertical,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Vertical);
        let mut bnrom = Bnrom::new(cart, &mut ppu);

        // Test switching through all 4 banks
        for i in 0..4 {
            bnrom.write_prg(0x8000, i, &mut ppu);
            assert_eq!(bnrom.read_prg(0x8000), (i + 1) as u8);
        }
    }

    #[test]
    fn bnrom_bank_masking() {
        let mut prg = vec![0; 0x10000]; // 2 banks
        prg[0] = 0x11;
        prg[0x8000] = 0x22;

        let cart = Cartridge {
            prg_rom: prg,
            chr_rom: vec![],
            mapper: 34,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Vertical,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Vertical);
        let mut bnrom = Bnrom::new(cart, &mut ppu);

        // Try to select bank 3, should wrap to bank 1 (3 % 2 = 1)
        // With 2 banks, mask is 1, so writing 3 & 1 = 1
        bnrom.write_prg(0x8000, 3, &mut ppu);
        assert_eq!(bnrom.read_prg(0x8000), 0x22);

        // Writing 0xFF should still wrap
        bnrom.write_prg(0x8000, 0xFF, &mut ppu);
        assert_eq!(bnrom.read_prg(0x8000), 0x22); // 0xFF & 1 = 1
    }

    #[test]
    fn bnrom_full_32kb_coverage() {
        let mut prg = vec![0; 0x10000]; // 2 banks
        prg[0] = 0xAA;
        prg[0x7FFF] = 0xBB;
        prg[0x8000] = 0xCC;
        prg[0xFFFF] = 0xDD;

        let cart = Cartridge {
            prg_rom: prg,
            chr_rom: vec![],
            mapper: 34,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Horizontal,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Horizontal);
        let mut bnrom = Bnrom::new(cart, &mut ppu);

        // Bank 0: full 32KB range
        assert_eq!(bnrom.read_prg(0x8000), 0xAA);
        assert_eq!(bnrom.read_prg(0xFFFF), 0xBB);

        // Switch to bank 1
        bnrom.write_prg(0x8000, 1, &mut ppu);
        assert_eq!(bnrom.read_prg(0x8000), 0xCC);
        assert_eq!(bnrom.read_prg(0xFFFF), 0xDD);
    }
}
