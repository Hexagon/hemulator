use crate::cartridge::{Cartridge, Mirroring};
use crate::ppu::Ppu;
#[cfg(test)]
use emu_core::apu::TimingMode;

/// Camerica/Codemasters (Mapper 71) - UxROM variant with optional 1-screen mirroring
///
/// Similar to UxROM (mapper 2) with switchable 16KB PRG banks and fixed last bank.
/// Key differences:
/// - Uses 8KB CHR-RAM instead of CHR-ROM
/// - Some boards (Fire Hawk) support mapper-controlled 1-screen mirroring via writes to $9000
/// - Includes bus conflict prevention and CIC defeat circuitry
///
/// Bank switching:
/// - CPU $8000-$BFFF: Switchable 16KB PRG bank
/// - CPU $C000-$FFFF: Fixed to last 16KB PRG bank
/// - PPU $0000-$1FFF: 8KB CHR-RAM
///
/// Register ($8000-$FFFF, write):
/// - Bits 0-3: Select 16KB PRG bank (any address $8000-$FFFF)
/// - Bit 4: Mirroring control (only for writes to $9000-$9FFF)
///   - 0 = One-screen lower
///   - 1 = One-screen upper
///
/// Note: Mirroring control only applies to $9000-$9FFF writes.
/// This prevents breaking games like Micro Machines that write to $8000
/// for bank switching without intending to change mirroring.
///
/// Used in games like Fire Hawk, Micro Machines, Dizzy series, etc.
#[derive(Debug)]
pub struct Camerica {
    prg_rom: Vec<u8>,
    bank_select: u8,
}

impl Camerica {
    pub fn new(cart: Cartridge, ppu: &mut Ppu) -> Self {
        // Initialize mirroring from the cartridge header.
        // This will be overridden by mapper writes to $9000-$9FFF if the game
        // uses dynamic mirroring control (e.g., Fire Hawk).
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

    pub fn write_prg(&mut self, addr: u16, val: u8, ppu: &mut Ppu, _cpu_cycles: u64) {
        if (0x8000..=0xFFFF).contains(&addr) {
            // Select 16KB bank for $8000-$BFFF
            // Only lower 4 bits are used for bank selection
            self.bank_select = val & 0x0F;

            // Mapper-controlled mirroring via bit 4:
            // - Bit 4 = 0: One-screen lower
            // - Bit 4 = 1: One-screen upper
            //
            // IMPORTANT: Mirroring control only applies to writes to $9000-$9FFF!
            // This is crucial for compatibility:
            // - Fire Hawk writes to $9000 to control mirroring
            // - Micro Machines writes to $8000 for bank switching only
            //
            // FCEUX and other accurate emulators only apply mirroring control for
            // writes to $9000-$9FFF to avoid breaking Micro Machines and similar games.
            //
            // Reference: https://www.nesdev.org/wiki/INES_Mapper_071
            if (0x9000..=0x9FFF).contains(&addr) {
                let mirroring = if (val & 0x10) != 0 {
                    Mirroring::SingleScreenUpper
                } else {
                    Mirroring::SingleScreenLower
                };
                ppu.set_mirroring(mirroring);
            }
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
        camerica.write_prg(0x8000, 1, &mut ppu, 0);
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
        camerica.write_prg(0x8000, 0x12, &mut ppu, 0);
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
            camerica.write_prg(0x8000, i, &mut ppu, 0);
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
            mirroring: Mirroring::Vertical, // Initial mirroring from header
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Vertical);
        let mut camerica = Camerica::new(cart, &mut ppu);

        // Write to $8000: should NOT change mirroring (only bank select)
        camerica.write_prg(0x8000, 0x10, &mut ppu, 0); // bit 4 = 1
        assert_eq!(ppu.get_mirroring(), Mirroring::Vertical); // Stays vertical

        // Write to $9000 with bit 4 = 1: should change to single-screen upper
        camerica.write_prg(0x9000, 0x10, &mut ppu, 0);
        assert_eq!(ppu.get_mirroring(), Mirroring::SingleScreenUpper);

        // Write to $9000 with bit 4 = 0: should change to single-screen lower
        camerica.write_prg(0x9000, 0x03, &mut ppu, 0);
        assert_eq!(ppu.get_mirroring(), Mirroring::SingleScreenLower);

        // Write to $8FFF: should NOT change mirroring (outside $9000-$9FFF range)
        camerica.write_prg(0x8FFF, 0x10, &mut ppu, 0);
        assert_eq!(ppu.get_mirroring(), Mirroring::SingleScreenLower); // Unchanged

        // Write to $A000: should NOT change mirroring (outside $9000-$9FFF range)
        camerica.write_prg(0xA000, 0x10, &mut ppu, 0);
        assert_eq!(ppu.get_mirroring(), Mirroring::SingleScreenLower); // Unchanged

        // Write to $9FFF with bit 4 = 1: should change to single-screen upper
        camerica.write_prg(0x9FFF, 0x15, &mut ppu, 0);
        assert_eq!(ppu.get_mirroring(), Mirroring::SingleScreenUpper);
    }

    #[test]
    fn camerica_fixed_mirroring() {
        // Test that games not using mapper-controlled mirroring keep their header mirroring
        // This simulates Micro Machines behavior: writes to $8000 for bank switching only
        let prg = vec![0; 0x8000]; // 2 banks

        let cart = Cartridge {
            prg_rom: prg,
            chr_rom: vec![],
            mapper: 71,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Horizontal, // Fixed mirroring from header
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Horizontal);
        let mut camerica = Camerica::new(cart, &mut ppu);

        // Write multiple times to $8000 (typical Micro Machines behavior)
        camerica.write_prg(0x8000, 0x00, &mut ppu, 0); // bit 4 = 0
        assert_eq!(ppu.get_mirroring(), Mirroring::Horizontal); // Stays horizontal

        camerica.write_prg(0x8000, 0x01, &mut ppu, 0); // bit 4 = 0
        assert_eq!(ppu.get_mirroring(), Mirroring::Horizontal); // Still horizontal

        camerica.write_prg(0x8000, 0x10, &mut ppu, 0); // bit 4 = 1
        assert_eq!(ppu.get_mirroring(), Mirroring::Horizontal); // Still horizontal

        camerica.write_prg(0xC000, 0x02, &mut ppu, 0); // bit 4 = 0
        assert_eq!(ppu.get_mirroring(), Mirroring::Horizontal); // Still horizontal

        // Since all writes were to $8000-$8FFF and $C000-$FFFF,
        // mirroring should remain as the header value (horizontal)
    }
}
