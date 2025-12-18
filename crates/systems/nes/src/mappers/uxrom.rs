use crate::cartridge::Cartridge;
use crate::ppu::Ppu;

/// UxROM (Mapper 2) - Switchable 16KB PRG banks with fixed last bank
#[derive(Debug)]
pub struct Uxrom {
    prg_rom: Vec<u8>,
    bank_select: u8,
}

impl Uxrom {
    pub fn new(cart: Cartridge, ppu: &mut Ppu) -> Self {
        // UxROM uses fixed mirroring from the header.
        ppu.set_mirroring(cart.mirroring);
        Self {
            prg_rom: cart.prg_rom,
            bank_select: 0,
        }
    }

    fn prg_bank_count(&self) -> usize {
        std::cmp::max(1, self.prg_rom.len() / 0x4000)
    }

    pub fn read_prg(&self, addr: u16) -> u8 {
        let bank = if addr < 0xC000 {
            (self.bank_select as usize) % self.prg_bank_count()
        } else {
            // Fixed last bank at $C000-$FFFF.
            self.prg_bank_count().saturating_sub(1)
        };
        let offset = (addr as usize) & 0x3FFF;
        let idx = bank.saturating_mul(0x4000) + offset;
        self.prg_rom.get(idx).copied().unwrap_or(0)
    }

    pub fn write_prg(&mut self, addr: u16, val: u8, _ppu: &mut Ppu) {
        if (0x8000..=0xFFFF).contains(&addr) {
            // Select 16KB bank for $8000-$BFFF; upper bits ignored beyond available banks.
            self.bank_select = val & 0x0F;
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
    fn uxrom_bank_switching() {
        let mut prg = vec![0; 0x8000]; // 2 banks of 16KB each
        prg[0] = 0x11; // Bank 0 start
        prg[0x4000] = 0x22; // Bank 1 start

        let cart = Cartridge {
            prg_rom: prg,
            chr_rom: vec![],
            mapper: 2,
            mirroring: Mirroring::Vertical,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Vertical);
        let mut uxrom = Uxrom::new(cart, &mut ppu);

        // Initially bank 0 at $8000, bank 1 (last) at $C000
        assert_eq!(uxrom.read_prg(0x8000), 0x11);
        assert_eq!(uxrom.read_prg(0xC000), 0x22);

        // Switch to bank 1 at $8000
        uxrom.write_prg(0x8000, 1, &mut ppu);
        assert_eq!(uxrom.read_prg(0x8000), 0x22);
        assert_eq!(uxrom.read_prg(0xC000), 0x22); // Last bank stays fixed
    }

    #[test]
    fn uxrom_fixed_last_bank() {
        let mut prg = vec![0; 0xC000]; // 3 banks of 16KB each
        prg[0] = 0x11;
        prg[0x4000] = 0x22;
        prg[0x8000] = 0x33;

        let cart = Cartridge {
            prg_rom: prg,
            chr_rom: vec![],
            mapper: 2,
            mirroring: Mirroring::Vertical,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Vertical);
        let uxrom = Uxrom::new(cart, &mut ppu);

        // Last bank (2) should always be at $C000
        assert_eq!(uxrom.read_prg(0xC000), 0x33);
    }
}
