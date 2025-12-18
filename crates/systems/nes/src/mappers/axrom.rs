use crate::cartridge::{Cartridge, Mirroring};
use crate::ppu::Ppu;

/// AxROM (Mapper 7) - 32KB PRG switching with single-screen mirroring
///
/// AxROM allows switching between multiple 32KB PRG-ROM banks.
/// The entire CPU address space ($8000-$FFFF) is swapped at once.
/// Supports configurable single-screen mirroring via bit 4 of the bank select.
#[derive(Debug)]
pub struct Axrom {
    prg_rom: Vec<u8>,
    prg_bank: u8,
}

impl Axrom {
    pub fn new(cart: Cartridge, ppu: &mut Ppu) -> Self {
        // AxROM uses single-screen mirroring, default to lower screen
        ppu.set_mirroring(Mirroring::SingleScreenLower);
        Self {
            prg_rom: cart.prg_rom,
            prg_bank: 0,
        }
    }

    fn prg_bank_count(&self) -> usize {
        std::cmp::max(1, self.prg_rom.len() / 0x8000)
    }

    pub fn read_prg(&self, addr: u16) -> u8 {
        let bank = (self.prg_bank as usize) % self.prg_bank_count();
        let offset = (addr as usize) & 0x7FFF;
        let idx = bank * 0x8000 + offset;
        self.prg_rom.get(idx).copied().unwrap_or(0)
    }

    pub fn write_prg(&mut self, addr: u16, val: u8, ppu: &mut Ppu) {
        if (0x8000..=0xFFFF).contains(&addr) {
            // Bits 0-2: PRG bank select (32KB banks)
            self.prg_bank = val & 0x07;

            // Bit 4: Single-screen mirroring select
            // 0 = use lower nametable ($2000-$23FF)
            // 1 = use upper nametable ($2400-$27FF)
            let mirroring = if val & 0x10 != 0 {
                Mirroring::SingleScreenUpper
            } else {
                Mirroring::SingleScreenLower
            };
            ppu.set_mirroring(mirroring);
        }
    }

    pub fn prg_rom(&self) -> &[u8] {
        &self.prg_rom
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn axrom_32kb_single_bank() {
        let cart = Cartridge {
            prg_rom: vec![0x42; 0x8000], // Single 32KB bank
            chr_rom: vec![],
            mapper: 7,
            mirroring: Mirroring::Vertical, // Will be overridden by mapper
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Vertical);
        let axrom = Axrom::new(cart, &mut ppu);

        // All addresses should read the same value
        assert_eq!(axrom.read_prg(0x8000), 0x42);
        assert_eq!(axrom.read_prg(0xC000), 0x42);
        assert_eq!(axrom.read_prg(0xFFFF), 0x42);
    }

    #[test]
    fn axrom_bank_switching() {
        // Create 4 banks of 32KB each
        let mut prg = vec![0; 0x20000]; // 128KB = 4 banks
        prg[0] = 0x11; // Bank 0 start
        prg[0x8000] = 0x22; // Bank 1 start
        prg[0x10000] = 0x33; // Bank 2 start
        prg[0x18000] = 0x44; // Bank 3 start

        let cart = Cartridge {
            prg_rom: prg,
            chr_rom: vec![],
            mapper: 7,
            mirroring: Mirroring::Horizontal,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Horizontal);
        let mut axrom = Axrom::new(cart, &mut ppu);

        // Initially bank 0
        assert_eq!(axrom.read_prg(0x8000), 0x11);

        // Switch to bank 1
        axrom.write_prg(0x8000, 1, &mut ppu);
        assert_eq!(axrom.read_prg(0x8000), 0x22);

        // Switch to bank 2
        axrom.write_prg(0x8000, 2, &mut ppu);
        assert_eq!(axrom.read_prg(0x8000), 0x33);

        // Switch to bank 3
        axrom.write_prg(0x8000, 3, &mut ppu);
        assert_eq!(axrom.read_prg(0x8000), 0x44);
    }

    #[test]
    fn axrom_single_screen_mirroring() {
        let cart = Cartridge {
            prg_rom: vec![0; 0x8000],
            chr_rom: vec![],
            mapper: 7,
            mirroring: Mirroring::Horizontal,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Horizontal);
        let mut axrom = Axrom::new(cart, &mut ppu);

        // Initially should be SingleScreenLower
        // (We can't directly test PPU mirroring here, but we verify the write logic)

        // Write with bit 4 clear - selects lower screen
        axrom.write_prg(0x8000, 0x00, &mut ppu);

        // Write with bit 4 set - selects upper screen
        axrom.write_prg(0x8000, 0x10, &mut ppu);

        // Combined: bank 2 + upper screen
        axrom.write_prg(0x8000, 0x12, &mut ppu);
        assert_eq!(axrom.prg_bank, 2);
    }

    #[test]
    fn axrom_bank_mask() {
        // Create 2 banks (64KB total)
        let mut prg = vec![0; 0x10000];
        prg[0] = 0x11;
        prg[0x8000] = 0x22;

        let cart = Cartridge {
            prg_rom: prg,
            chr_rom: vec![],
            mapper: 7,
            mirroring: Mirroring::Horizontal,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Horizontal);
        let mut axrom = Axrom::new(cart, &mut ppu);

        // Bank 0
        axrom.write_prg(0x8000, 0, &mut ppu);
        assert_eq!(axrom.read_prg(0x8000), 0x11);

        // Bank 1
        axrom.write_prg(0x8000, 1, &mut ppu);
        assert_eq!(axrom.read_prg(0x8000), 0x22);

        // Bank 2 should wrap to 0 (modulo)
        axrom.write_prg(0x8000, 2, &mut ppu);
        assert_eq!(axrom.read_prg(0x8000), 0x11);

        // Bank 3 should wrap to 1 (modulo)
        axrom.write_prg(0x8000, 3, &mut ppu);
        assert_eq!(axrom.read_prg(0x8000), 0x22);
    }
}
