use crate::cartridge::Cartridge;
use crate::ppu::Ppu;
#[cfg(test)]
use emu_core::apu::TimingMode;

/// Camerica/Codemasters (Mapper 71) - UxROM variant with optional 1-screen mirroring
///
/// Similar to UxROM (mapper 2) with switchable 16KB PRG banks and fixed last bank.
/// Key differences:
/// - Uses 8KB CHR-RAM instead of CHR-ROM
/// - Some boards (Fire Hawk) support mapper-controlled 1-screen mirroring
/// - Includes bus conflict prevention and CIC defeat circuitry
///
/// Bank switching:
/// - CPU $8000-$BFFF: Switchable 16KB PRG bank
/// - CPU $C000-$FFFF: Fixed to last 16KB PRG bank
/// - PPU $0000-$1FFF: 8KB CHR-RAM
///
/// Register ($8000-$FFFF, write):
/// - Bits 0-3: Select 16KB PRG bank
/// - Some boards may use higher bits for mirroring control
///
/// Used in games like Fire Hawk, Micro Machines, Dizzy series, etc.
#[derive(Debug)]
pub struct Camerica {
    prg_rom: Vec<u8>,
    bank_select: u8,
}

impl Camerica {
    pub fn new(cart: Cartridge, ppu: &mut Ppu) -> Self {
        // Camerica uses fixed mirroring from the header
        // (Fire Hawk variant supports 1-screen mirroring via mapper, but most use fixed)
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
            // $8000-$BFFF: Switchable bank
            (self.bank_select as usize) % self.prg_bank_count()
        } else {
            // $C000-$FFFF: Fixed to last bank
            self.prg_bank_count().saturating_sub(1)
        };
        let offset = (addr as usize) & 0x3FFF;
        let idx = bank.saturating_mul(0x4000) + offset;
        self.prg_rom.get(idx).copied().unwrap_or(0)
    }

    pub fn write_prg(&mut self, addr: u16, val: u8, ppu: &mut Ppu) {
        if (0x8000..=0xFFFF).contains(&addr) {
            // Select 16KB bank for $8000-$BFFF
            // Only lower 4 bits are used for bank selection
            self.bank_select = val & 0x0F;

            // Some boards (Fire Hawk, Micro Machines) use bit 4 for mirroring control:
            // - Bit 4 = 0: One-screen lower
            // - Bit 4 = 1: One-screen upper
            // This is board-specific: some games use fixed mirroring from the header,
            // while others dynamically control it via this bit.
            use crate::cartridge::Mirroring;
            let mirroring = if (val & 0x10) != 0 {
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
    use crate::cartridge::Mirroring;

    #[test]
    fn camerica_bank_switching() {
        let mut prg = vec![0; 0x8000]; // 2 banks of 16KB each
        prg[0] = 0x11; // Bank 0 start
        prg[0x4000] = 0x22; // Bank 1 start

        let cart = Cartridge {
            prg_rom: prg,
            chr_rom: vec![],
            mapper: 71,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Vertical,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Vertical);
        let mut camerica = Camerica::new(cart, &mut ppu);

        // Initially bank 0 at $8000, bank 1 (last) at $C000
        assert_eq!(camerica.read_prg(0x8000), 0x11);
        assert_eq!(camerica.read_prg(0xC000), 0x22);

        // Switch to bank 1 at $8000
        camerica.write_prg(0x8000, 1, &mut ppu);
        assert_eq!(camerica.read_prg(0x8000), 0x22);
        assert_eq!(camerica.read_prg(0xC000), 0x22); // Last bank stays fixed
    }

    #[test]
    fn camerica_fixed_last_bank() {
        let mut prg = vec![0; 0xC000]; // 3 banks of 16KB each
        prg[0] = 0x11;
        prg[0x4000] = 0x22;
        prg[0x8000] = 0x33;

        let cart = Cartridge {
            prg_rom: prg,
            chr_rom: vec![],
            mapper: 71,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Horizontal,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Horizontal);
        let camerica = Camerica::new(cart, &mut ppu);

        // Last bank (2) should always be at $C000
        assert_eq!(camerica.read_prg(0xC000), 0x33);
    }

    #[test]
    fn camerica_bank_masking() {
        let mut prg = vec![0; 0x8000]; // 2 banks
        prg[0] = 0x11;
        prg[0x4000] = 0x22;

        let cart = Cartridge {
            prg_rom: prg,
            chr_rom: vec![],
            mapper: 71,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Vertical,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Vertical);
        let mut camerica = Camerica::new(cart, &mut ppu);

        // Write 0x12 (bank 2, but only 4 bits used, so wraps)
        // 0x12 & 0x0F = 0x02, which wraps to bank 0 (2 % 2 = 0)
        camerica.write_prg(0x8000, 0x12, &mut ppu);
        assert_eq!(camerica.read_prg(0x8000), 0x11); // Wraps to bank 0
    }

    #[test]
    fn camerica_multiple_banks() {
        let mut prg = vec![0; 0x20000]; // 8 banks of 16KB each
        for i in 0..8 {
            prg[i * 0x4000] = 0x11 * (i as u8 + 1);
        }

        let cart = Cartridge {
            prg_rom: prg,
            chr_rom: vec![],
            mapper: 71,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Horizontal,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Horizontal);
        let mut camerica = Camerica::new(cart, &mut ppu);

        // Last bank should be bank 7
        assert_eq!(camerica.read_prg(0xC000), 0x88);

        // Test switching through first few banks
        for i in 0..8 {
            camerica.write_prg(0x8000, i, &mut ppu);
            assert_eq!(camerica.read_prg(0x8000), 0x11 * (i as u8 + 1));
            assert_eq!(camerica.read_prg(0xC000), 0x88); // Last bank always fixed
        }
    }

    #[test]
    fn camerica_mirroring_control() {
        let prg = vec![0; 0x8000]; // 2 banks

        let cart = Cartridge {
            prg_rom: prg,
            chr_rom: vec![],
            mapper: 71,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Vertical, // Initial mirroring (will be overridden)
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Vertical);
        let mut camerica = Camerica::new(cart, &mut ppu);

        // Write with bit 4 = 0: should select single-screen lower
        camerica.write_prg(0x8000, 0x00, &mut ppu);
        assert_eq!(ppu.get_mirroring(), Mirroring::SingleScreenLower);

        // Write with bit 4 = 1: should select single-screen upper
        camerica.write_prg(0x8000, 0x10, &mut ppu);
        assert_eq!(ppu.get_mirroring(), Mirroring::SingleScreenUpper);

        // Write bank select with bit 4 = 0: should select single-screen lower
        camerica.write_prg(0xC000, 0x03, &mut ppu); // Bank 3, bit 4 = 0
        assert_eq!(ppu.get_mirroring(), Mirroring::SingleScreenLower);

        // Write bank select with bit 4 = 1: should select single-screen upper
        camerica.write_prg(0xC000, 0x15, &mut ppu); // Bank 5, bit 4 = 1
        assert_eq!(ppu.get_mirroring(), Mirroring::SingleScreenUpper);
    }
}
