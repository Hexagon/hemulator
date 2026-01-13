use crate::cartridge::{Cartridge, Mirroring};
use crate::ppu::Ppu;
#[cfg(test)]
use emu_core::apu::TimingMode;

/// MMC2 (Mapper 9) - Used primarily in Punch-Out!!
///
/// Features PPU-triggered CHR bank switching via latch addresses.
///
/// # Hardware Behavior (per NESdev wiki)
/// - **PRG ROM**: 128 KB max, 8 KB switchable at $8000-$9FFF, fixed banks at $A000-$FFFF
/// - **CHR ROM**: Two 4 KB banks ($0000-$0FFF, $1000-$1FFF), each with dual-bank selection
/// - **Latch Mechanism**: When PPU reads from specific CHR addresses, latches switch
///   which bank is active. **Note:** MMC2 differs from MMC4 in address ranges:
///   * $0FD8: Sets latch 0 to $FD (affects $0000-$0FFF) - SINGLE ADDRESS
///   * $0FE8: Sets latch 0 to $FE (affects $0000-$0FFF) - SINGLE ADDRESS
///   * $1FD8-$1FDF: Sets latch 1 to $FD (affects $1000-$1FFF) - 8-BYTE RANGE
///   * $1FE8-$1FEF: Sets latch 1 to $FE (affects $1000-$1FFF) - 8-BYTE RANGE
///
/// # The "34th Tile" Snooping Mechanism
///
/// The latch triggers correspond to specific tiles in the pattern tables:
/// - **Tile $FD (253)**: Reading this tile triggers latch to $FD state
///   - Address $0FD8 = tile $FD, row 0, high bitplane (byte 8 of tile data)
/// - **Tile $FE (254)**: Reading this tile triggers latch to $FE state
///   - Address $0FE8 = tile $FE, row 0, high bitplane (byte 8 of tile data)
///
/// In Punch-Out!!, these tiles are used in sprites to dynamically switch CHR banks
/// during rendering. When the PPU fetches sprite graphics using tile $FD or $FE,
/// it automatically switches the active CHR bank for subsequent tiles. This is often
/// called the "34th tile" mechanism because these special tiles act as signals.
///
/// **Critical Implementation Detail**: The PPU must invoke the CHR read callback for
/// BOTH the low bitplane (byte 0-7) and high bitplane (byte 8-15) of each tile.
/// The latch triggers are specifically on the HIGH bitplane addresses.
///
/// # Implementation
/// Latch switching is now fully implemented via CHR read callbacks. When the PPU
/// reads from latch trigger addresses during rendering, the mapper tracks latch
/// state changes and applies CHR bank updates after each frame completes.
#[derive(Debug)]
pub struct Mmc2 {
    prg_rom: Vec<u8>,
    chr_rom: Vec<u8>,
    prg_bank: u8,
    // CHR banks for left pattern table ($0000-$0FFF)
    chr_bank_fd: u8, // Used when latch 0 is FD
    chr_bank_fe: u8, // Used when latch 0 is FE
    // CHR banks for right pattern table ($1000-$1FFF)
    chr_bank_1_fd: u8, // Used when latch 1 is FD
    chr_bank_1_fe: u8, // Used when latch 1 is FE
    // Latch states (FD or FE)
    latch_0: u8, // For $0000-$0FFF
    latch_1: u8, // For $1000-$1FFF
    // Track if CHR needs updating
    chr_dirty: bool,
}

impl Mmc2 {
    pub fn new(cart: Cartridge, ppu: &mut Ppu) -> Self {
        ppu.set_mirroring(cart.mirroring);
        let mmc2 = Self {
            prg_rom: cart.prg_rom,
            chr_rom: cart.chr_rom,
            prg_bank: 0,
            chr_bank_fd: 0,
            chr_bank_fe: 0,
            chr_bank_1_fd: 0,
            chr_bank_1_fe: 0,
            latch_0: 0xFE,
            latch_1: 0xFE,
            chr_dirty: false,
        };
        mmc2.update_chr_mapping(ppu);
        mmc2
    }

    fn prg_bank_count(&self) -> usize {
        // MMC2 uses 8KB PRG banks
        std::cmp::max(1, self.prg_rom.len() / 0x2000)
    }

    fn chr_bank_count(&self) -> usize {
        std::cmp::max(1, self.chr_rom.len() / 0x1000)
    }

    fn update_chr_mapping(&self, ppu: &mut Ppu) {
        if ppu.chr.len() < 0x2000 {
            ppu.chr.resize(0x2000, 0);
        }

        // CHR-RAM carts skip copying
        if self.chr_rom.is_empty() {
            return;
        }

        let chr_count = self.chr_bank_count();

        // Left pattern table ($0000-$0FFF) - 4KB
        let bank_0 = if self.latch_0 == 0xFD {
            (self.chr_bank_fd as usize) % chr_count
        } else {
            (self.chr_bank_fe as usize) % chr_count
        };

        // Right pattern table ($1000-$1FFF) - 4KB
        let bank_1 = if self.latch_1 == 0xFD {
            (self.chr_bank_1_fd as usize) % chr_count
        } else {
            (self.chr_bank_1_fe as usize) % chr_count
        };

        // Copy CHR banks
        for (i, &bank) in [bank_0, bank_1].iter().enumerate() {
            let dst_start = i * 0x1000;
            let src_start = bank * 0x1000;
            let src_end = src_start + 0x1000;
            if src_end <= self.chr_rom.len() {
                ppu.chr[dst_start..dst_start + 0x1000]
                    .copy_from_slice(&self.chr_rom[src_start..src_end]);
            } else {
                for b in &mut ppu.chr[dst_start..dst_start + 0x1000] {
                    *b = 0;
                }
            }
        }
    }

    pub fn read_prg(&self, addr: u16) -> u8 {
        let prg_count = self.prg_bank_count();
        // MMC2 PRG layout (all 8KB banks):
        // $8000-$9FFF: switchable 8KB bank
        // $A000-$BFFF: fixed to bank -3 (third-to-last)
        // $C000-$DFFF: fixed to bank -2 (second-to-last)
        // $E000-$FFFF: fixed to bank -1 (last)
        let bank = match addr {
            0x8000..=0x9FFF => (self.prg_bank as usize) % prg_count,
            0xA000..=0xBFFF => prg_count.saturating_sub(3),
            0xC000..=0xDFFF => prg_count.saturating_sub(2),
            0xE000..=0xFFFF => prg_count.saturating_sub(1),
            _ => 0,
        };
        let offset = (addr as usize) & 0x1FFF;
        let idx = bank * 0x2000 + offset;
        self.prg_rom.get(idx).copied().unwrap_or(0)
    }

    pub fn write_prg(&mut self, addr: u16, val: u8, ppu: &mut Ppu, _cpu_cycles: u64) {
        match addr {
            0xA000..=0xAFFF => {
                // PRG ROM bank select
                self.prg_bank = val & 0x0F;
            }
            0xB000..=0xBFFF => {
                // CHR ROM $FD/0000 bank select
                self.chr_bank_fd = val & 0x1F;
                self.update_chr_mapping(ppu);
            }
            0xC000..=0xCFFF => {
                // CHR ROM $FE/0000 bank select
                self.chr_bank_fe = val & 0x1F;
                self.update_chr_mapping(ppu);
            }
            0xD000..=0xDFFF => {
                // CHR ROM $FD/1000 bank select
                self.chr_bank_1_fd = val & 0x1F;
                self.update_chr_mapping(ppu);
            }
            0xE000..=0xEFFF => {
                // CHR ROM $FE/1000 bank select
                self.chr_bank_1_fe = val & 0x1F;
                self.update_chr_mapping(ppu);
            }
            0xF000..=0xFFFF => {
                // Mirroring control
                let mirroring = if val & 0x01 != 0 {
                    Mirroring::Horizontal
                } else {
                    Mirroring::Vertical
                };
                ppu.set_mirroring(mirroring);
            }
            _ => {}
        }
    }

    /// Called by PPU when reading from pattern tables
    /// This handles the automatic latch switching per MMC2 specification.
    ///
    /// # Latch Address Ranges (per NESdev wiki)
    /// MMC2 uses SINGLE addresses for left latch, RANGES for right latch:
    /// - $0FD8: Latch 0 → $FD (left pattern table) - single address only
    /// - $0FE8: Latch 0 → $FE (left pattern table) - single address only
    /// - $1FD8-$1FDF: Latch 1 → $FD (right pattern table) - 8-byte range
    /// - $1FE8-$1FEF: Latch 1 → $FE (right pattern table) - 8-byte range
    ///
    /// This method is called via callback during PPU rendering. It updates
    /// internal latch state and marks CHR as dirty for later update.
    pub fn notify_chr_read(&mut self, addr: u16) {
        match addr {
            0x0FD8 => {
                if self.latch_0 != 0xFD {
                    self.latch_0 = 0xFD;
                    self.chr_dirty = true;
                }
            }
            0x0FE8 => {
                if self.latch_0 != 0xFE {
                    self.latch_0 = 0xFE;
                    self.chr_dirty = true;
                }
            }
            0x1FD8..=0x1FDF => {
                if self.latch_1 != 0xFD {
                    self.latch_1 = 0xFD;
                    self.chr_dirty = true;
                }
            }
            0x1FE8..=0x1FEF => {
                if self.latch_1 != 0xFE {
                    self.latch_1 = 0xFE;
                    self.chr_dirty = true;
                }
            }
            _ => {}
        }
    }

    /// Apply pending CHR bank updates if latches changed during rendering.
    /// Should be called after frame rendering completes.
    pub fn apply_chr_update(&mut self, ppu: &mut Ppu) {
        if self.chr_dirty {
            self.update_chr_mapping(ppu);
            self.chr_dirty = false;
        }
    }

    /// Legacy method kept for tests.
    /// In actual emulation, use notify_chr_read() + apply_chr_update() instead.
    #[allow(dead_code)]
    pub fn ppu_read_chr(&mut self, addr: u16, ppu: &mut Ppu) {
        match addr {
            0x0FD8 => {
                if self.latch_0 != 0xFD {
                    self.latch_0 = 0xFD;
                    self.update_chr_mapping(ppu);
                }
            }
            0x0FE8 => {
                if self.latch_0 != 0xFE {
                    self.latch_0 = 0xFE;
                    self.update_chr_mapping(ppu);
                }
            }
            0x1FD8..=0x1FDF => {
                if self.latch_1 != 0xFD {
                    self.latch_1 = 0xFD;
                    self.update_chr_mapping(ppu);
                }
            }
            0x1FE8..=0x1FEF => {
                if self.latch_1 != 0xFE {
                    self.latch_1 = 0xFE;
                    self.update_chr_mapping(ppu);
                }
            }
            _ => {}
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
    fn mmc2_prg_banking() {
        // MMC2 uses 8KB banks. Create 8 banks of 8KB each (64KB total)
        let mut prg = vec![0; 0x10000]; // 8 banks of 8KB each
        prg[0x0000] = 0x11; // Bank 0 ($8000-$9FFF when bank=0)
        prg[0x2000] = 0x22; // Bank 1 ($8000-$9FFF when bank=1)
        prg[0xA000] = 0x55; // Bank 5 ($A000-$BFFF, fixed third-to-last)
        prg[0xC000] = 0x66; // Bank 6 ($C000-$DFFF, fixed second-to-last)
        prg[0xE000] = 0x77; // Bank 7 ($E000-$FFFF, fixed last)

        let cart = Cartridge::new_test(
            prg,
            vec![0; 0x2000],
            9,
            Mirroring::Vertical,
            TimingMode::Ntsc,
        );

        let mut ppu = Ppu::new(vec![], Mirroring::Vertical, TimingMode::Ntsc);
        let mut mmc2 = Mmc2::new(cart, &mut ppu);

        // Initially bank 0 at $8000-$9FFF
        assert_eq!(mmc2.read_prg(0x8000), 0x11);

        // Switch to bank 1
        mmc2.write_prg(0xA000, 1, &mut ppu, 0);
        assert_eq!(mmc2.read_prg(0x8000), 0x22);

        // $A000-$BFFF fixed to bank 5 (third-to-last)
        assert_eq!(mmc2.read_prg(0xA000), 0x55);

        // $C000-$DFFF fixed to bank 6 (second-to-last)
        assert_eq!(mmc2.read_prg(0xC000), 0x66);

        // $E000-$FFFF fixed to bank 7 (last)
        assert_eq!(mmc2.read_prg(0xE000), 0x77);
    }

    #[test]
    fn mmc2_chr_latch_switching() {
        let mut chr = vec![0; 0x8000]; // 8 banks of 4KB each
        chr[0] = 0x11; // Bank 0
        chr[0x1000] = 0x22; // Bank 1
        chr[0x2000] = 0x33; // Bank 2

        let cart = Cartridge::new_test(
            vec![0; 0x8000],
            chr,
            9,
            Mirroring::Vertical,
            TimingMode::Ntsc,
        );

        let mut ppu = Ppu::new(vec![], Mirroring::Vertical, TimingMode::Ntsc);
        let mut mmc2 = Mmc2::new(cart, &mut ppu);

        // Set FD bank to 1 and FE bank to 2 for left pattern table
        mmc2.write_prg(0xB000, 1, &mut ppu, 0); // FD/0000
        mmc2.write_prg(0xC000, 2, &mut ppu, 0); // FE/0000

        // Initially latch is FE, so should see bank 2
        assert_eq!(ppu.chr[0], 0x33);

        // Trigger FD latch by simulating PPU read
        mmc2.ppu_read_chr(0x0FD8, &mut ppu);
        assert_eq!(ppu.chr[0], 0x22);

        // Trigger FE latch
        mmc2.ppu_read_chr(0x0FE8, &mut ppu);
        assert_eq!(ppu.chr[0], 0x33);
    }

    #[test]
    fn mmc2_mirroring_control() {
        let cart = Cartridge::new_test(
            vec![0; 0x8000],
            vec![0; 0x2000],
            9,
            Mirroring::Vertical,
            TimingMode::Ntsc,
        );

        let mut ppu = Ppu::new(vec![], Mirroring::Vertical, TimingMode::Ntsc);
        let mut mmc2 = Mmc2::new(cart, &mut ppu);

        // Switch to horizontal mirroring
        mmc2.write_prg(0xF000, 0x01, &mut ppu, 0);
        // (We can't directly test PPU mirroring state, but we verify the write logic)

        // Switch back to vertical
        mmc2.write_prg(0xF000, 0x00, &mut ppu, 0);
    }

    #[test]
    fn mmc2_latch_trigger_addresses() {
        // Test that the exact latch trigger addresses work correctly.
        // This is critical for Punch-Out!! where sprites with tile $FD/$FE trigger latches.
        let mut chr = vec![0; 0x8000]; // 8 banks of 4KB each
        chr[0] = 0x11; // Bank 0
        chr[0x1000] = 0x22; // Bank 1
        chr[0x2000] = 0x33; // Bank 2
        chr[0x3000] = 0x44; // Bank 3

        let cart = Cartridge {
            prg_rom: vec![0; 0x8000],
            chr_rom: chr,
            mapper: 9,
            submapper: 0,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Vertical,
            crc32: 0,
            header_mapper: 9,
            header_submapper: 0,
            header_mirroring: Mirroring::Vertical,
            db_mapper_override: false,
            db_mirroring_override: false,
            board_name: None,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Vertical, TimingMode::Ntsc);
        let mut mmc2 = Mmc2::new(cart, &mut ppu);

        // Configure banks for left pattern table ($0000-$0FFF)
        mmc2.write_prg(0xB000, 1, &mut ppu, 0); // FD/0000 -> bank 1
        mmc2.write_prg(0xC000, 2, &mut ppu, 0); // FE/0000 -> bank 2

        // Configure banks for right pattern table ($1000-$1FFF)
        mmc2.write_prg(0xD000, 3, &mut ppu, 0); // FD/1000 -> bank 3
        mmc2.write_prg(0xE000, 1, &mut ppu, 0); // FE/1000 -> bank 1

        // Test left pattern table latch switching
        // Initially latch is FE, so should see bank 2
        assert_eq!(ppu.chr[0], 0x33, "Initial left latch should be FE (bank 2)");

        // Trigger FD latch at $0FD8 (tile $FD, row 0, high bitplane)
        mmc2.notify_chr_read(0x0FD8);
        mmc2.apply_chr_update(&mut ppu);
        assert_eq!(
            ppu.chr[0], 0x22,
            "Left latch should switch to FD (bank 1) at $0FD8"
        );

        // Trigger FE latch at $0FE8 (tile $FE, row 0, high bitplane)
        mmc2.notify_chr_read(0x0FE8);
        mmc2.apply_chr_update(&mut ppu);
        assert_eq!(
            ppu.chr[0], 0x33,
            "Left latch should switch to FE (bank 2) at $0FE8"
        );

        // Test right pattern table latch switching
        // Initially latch is FE, so should see bank 1
        assert_eq!(
            ppu.chr[0x1000], 0x22,
            "Initial right latch should be FE (bank 1)"
        );

        // Trigger FD latch for right pattern table (range $1FD8-$1FDF)
        mmc2.notify_chr_read(0x1FD8);
        mmc2.apply_chr_update(&mut ppu);
        assert_eq!(
            ppu.chr[0x1000], 0x44,
            "Right latch should switch to FD (bank 3) at $1FD8"
        );

        // Test middle of FD range
        mmc2.notify_chr_read(0x1FDC);
        mmc2.apply_chr_update(&mut ppu);
        assert_eq!(ppu.chr[0x1000], 0x44, "Right latch should stay FD at $1FDC");

        // Test end of FD range
        mmc2.notify_chr_read(0x1FDF);
        mmc2.apply_chr_update(&mut ppu);
        assert_eq!(ppu.chr[0x1000], 0x44, "Right latch should stay FD at $1FDF");

        // Trigger FE latch for right pattern table (range $1FE8-$1FEF)
        mmc2.notify_chr_read(0x1FE8);
        mmc2.apply_chr_update(&mut ppu);
        assert_eq!(
            ppu.chr[0x1000], 0x22,
            "Right latch should switch to FE (bank 1) at $1FE8"
        );

        // Test end of FE range
        mmc2.notify_chr_read(0x1FEF);
        mmc2.apply_chr_update(&mut ppu);
        assert_eq!(ppu.chr[0x1000], 0x22, "Right latch should stay FE at $1FEF");
    }
}
