use crate::cartridge::{Cartridge, Mirroring};
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
    /// Track the previous value of bit 4 to detect mirroring changes
    /// This helps distinguish between boards with mapper-controlled mirroring
    /// and those with fixed mirroring from the cartridge header
    previous_bit4: Option<bool>,
    /// Count how many times bit 4 has changed
    /// If this stays at 0, the game likely uses fixed mirroring
    mirroring_change_count: u8,
}

impl Camerica {
    pub fn new(cart: Cartridge, ppu: &mut Ppu) -> Self {
        // Initialize mirroring from the cartridge header.
        // This will be overridden by mapper writes if the game uses dynamic mirroring control.
        ppu.set_mirroring(cart.mirroring);
        Self {
            prg_rom: cart.prg_rom,
            bank_select: 0,
            previous_bit4: None, // Will be set on first write
            mirroring_change_count: 0,
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

            // Mapper-controlled mirroring via bit 4:
            // - Bit 4 = 0: One-screen lower
            // - Bit 4 = 1: One-screen upper
            //
            // Some Camerica boards (Fire Hawk) use this feature for dynamic mirroring control,
            // while others (some Micro Machines variants) use fixed mirroring from the cartridge header.
            //
            // To distinguish board variants at runtime, we track if bit 4 changes:
            // - If bit 4 toggles, the game uses mapper-controlled mirroring
            // - If bit 4 never changes, the game uses fixed mirroring from the header
            //
            // We only update mirroring after detecting at least one bit 4 transition.
            let current_bit4 = (val & 0x10) != 0;

            // Check if bit 4 has changed since the last write
            if let Some(prev) = self.previous_bit4 {
                if current_bit4 != prev {
                    // Bit 4 changed - increment change counter
                    self.mirroring_change_count = self.mirroring_change_count.saturating_add(1);
                }
            }
            // Always update previous_bit4 to track the current value
            self.previous_bit4 = Some(current_bit4);

            // Only update mirroring if we've detected bit 4 changes (mapper-controlled mirroring)
            // This prevents unwanted mirroring updates on games with fixed mirroring
            if self.mirroring_change_count > 0 {
                let mirroring = if current_bit4 {
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
            mirroring: Mirroring::Vertical, // Initial mirroring (stays unless bit 4 toggles)
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Vertical);
        let mut camerica = Camerica::new(cart, &mut ppu);

        // First write with bit 4 = 0: should NOT change mirroring yet (no toggle detected)
        camerica.write_prg(0x8000, 0x00, &mut ppu);
        assert_eq!(ppu.get_mirroring(), Mirroring::Vertical); // Stays as initial

        // Write with bit 4 = 1: should detect toggle and change to single-screen upper
        camerica.write_prg(0x8000, 0x10, &mut ppu);
        assert_eq!(ppu.get_mirroring(), Mirroring::SingleScreenUpper);

        // Write bank select with bit 4 = 0: should change to single-screen lower
        camerica.write_prg(0xC000, 0x03, &mut ppu); // Bank 3, bit 4 = 0
        assert_eq!(ppu.get_mirroring(), Mirroring::SingleScreenLower);

        // Write bank select with bit 4 = 1: should change to single-screen upper
        camerica.write_prg(0xC000, 0x15, &mut ppu); // Bank 5, bit 4 = 1
        assert_eq!(ppu.get_mirroring(), Mirroring::SingleScreenUpper);
    }

    #[test]
    fn camerica_fixed_mirroring() {
        // Test that games not using mapper-controlled mirroring keep their header mirroring
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

        // Write multiple times with same bit 4 value (no toggle)
        camerica.write_prg(0x8000, 0x00, &mut ppu); // bit 4 = 0
        assert_eq!(ppu.get_mirroring(), Mirroring::Horizontal); // Stays horizontal

        camerica.write_prg(0x8000, 0x01, &mut ppu); // bit 4 = 0
        assert_eq!(ppu.get_mirroring(), Mirroring::Horizontal); // Still horizontal

        camerica.write_prg(0xC000, 0x02, &mut ppu); // bit 4 = 0
        assert_eq!(ppu.get_mirroring(), Mirroring::Horizontal); // Still horizontal

        // Since bit 4 never toggled, mirroring should remain as the header value
    }
}
