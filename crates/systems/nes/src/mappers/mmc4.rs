use crate::cartridge::{Cartridge, Mirroring};
use crate::ppu::Ppu;
use emu_core::apu::TimingMode;

/// MMC4 (Mapper 10) - Similar to MMC2 but with different CHR latch addresses
///
/// Used in a few Japanese exclusive games like Fire Emblem.
/// Features PPU-triggered CHR bank switching via latch addresses.
#[derive(Debug)]
pub struct Mmc4 {
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
}

impl Mmc4 {
    pub fn new(cart: Cartridge, ppu: &mut Ppu) -> Self {
        ppu.set_mirroring(cart.mirroring);
        let mmc4 = Self {
            prg_rom: cart.prg_rom,
            chr_rom: cart.chr_rom,
            prg_bank: 0,
            chr_bank_fd: 0,
            chr_bank_fe: 0,
            chr_bank_1_fd: 0,
            chr_bank_1_fe: 0,
            latch_0: 0xFE,
            latch_1: 0xFE,
        };
        mmc4.update_chr_mapping(ppu);
        mmc4
    }

    fn prg_bank_count(&self) -> usize {
        std::cmp::max(1, self.prg_rom.len() / 0x4000)
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
        let bank = if addr < 0xC000 {
            // $8000-$BFFF: switchable 16KB bank
            (self.prg_bank as usize) % prg_count
        } else {
            // $C000-$FFFF: fixed to last 16KB bank
            prg_count.saturating_sub(1)
        };
        let offset = (addr as usize) & 0x3FFF;
        let idx = bank * 0x4000 + offset;
        self.prg_rom.get(idx).copied().unwrap_or(0)
    }

    pub fn write_prg(&mut self, addr: u16, val: u8, ppu: &mut Ppu) {
        match addr {
            0xA000..=0xAFFF => {
                // PRG ROM bank select (16KB)
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
    /// This handles the automatic latch switching
    /// MMC4 uses different addresses than MMC2
    #[allow(dead_code)]
    pub fn ppu_read_chr(&mut self, addr: u16, ppu: &mut Ppu) {
        match addr {
            0x0FD8..=0x0FDF => {
                if self.latch_0 != 0xFD {
                    self.latch_0 = 0xFD;
                    self.update_chr_mapping(ppu);
                }
            }
            0x0FE8..=0x0FEF => {
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
    fn mmc4_prg_banking() {
        let mut prg = vec![0; 0x10000]; // 4 banks of 16KB each
        prg[0] = 0x11; // Bank 0
        prg[0x4000] = 0x22; // Bank 1
        prg[0xC000] = 0x44; // Bank 3 (last bank)

        let cart = Cartridge {
            prg_rom: prg,
            chr_rom: vec![0; 0x2000],
            mapper: 10,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Vertical,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Vertical);
        let mut mmc4 = Mmc4::new(cart, &mut ppu);

        // Initially bank 0 at $8000
        assert_eq!(mmc4.read_prg(0x8000), 0x11);

        // Switch to bank 1
        mmc4.write_prg(0xA000, 1, &mut ppu);
        assert_eq!(mmc4.read_prg(0x8000), 0x22);

        // $C000-$FFFF should be fixed to last bank
        assert_eq!(mmc4.read_prg(0xC000), 0x44);
    }

    #[test]
    fn mmc4_chr_latch_switching() {
        let mut chr = vec![0; 0x8000]; // 8 banks of 4KB each
        chr[0] = 0x11; // Bank 0
        chr[0x1000] = 0x22; // Bank 1
        chr[0x2000] = 0x33; // Bank 2

        let cart = Cartridge {
            prg_rom: vec![0; 0x8000],
            chr_rom: chr,
            mapper: 10,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Vertical,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Vertical);
        let mut mmc4 = Mmc4::new(cart, &mut ppu);

        // Set FD bank to 1 and FE bank to 2 for left pattern table
        mmc4.write_prg(0xB000, 1, &mut ppu); // FD/0000
        mmc4.write_prg(0xC000, 2, &mut ppu); // FE/0000

        // Initially latch is FE, so should see bank 2
        assert_eq!(ppu.chr[0], 0x33);

        // Trigger FD latch by simulating PPU read (note range for MMC4)
        mmc4.ppu_read_chr(0x0FD8, &mut ppu);
        assert_eq!(ppu.chr[0], 0x22);

        // Trigger FE latch
        mmc4.ppu_read_chr(0x0FE8, &mut ppu);
        assert_eq!(ppu.chr[0], 0x33);
    }

    #[test]
    fn mmc4_mirroring_control() {
        let cart = Cartridge {
            prg_rom: vec![0; 0x8000],
            chr_rom: vec![0; 0x2000],
            mapper: 10,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Vertical,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Vertical);
        let mut mmc4 = Mmc4::new(cart, &mut ppu);

        // Switch to horizontal mirroring
        mmc4.write_prg(0xF000, 0x01, &mut ppu);

        // Switch back to vertical
        mmc4.write_prg(0xF000, 0x00, &mut ppu);
    }
}
