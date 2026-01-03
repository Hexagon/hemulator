use crate::cartridge::{Cartridge, Mirroring};
use crate::ppu::Ppu;
#[cfg(test)]
use emu_core::apu::TimingMode;

/// Namco 118 / Mapper 206 - MMC3-like mapper without IRQ support
///
/// This is a simplified version of MMC3 used by Namco and Tengen.
/// It provides the same PRG/CHR banking modes as MMC3 but lacks the scanline IRQ counter.
///
/// # Hardware Behavior
/// - **PRG ROM**: Up to 128 KB, four 8KB banks mapped to CPU $8000-$FFFF
/// - **CHR ROM**: Up to 128 KB, eight 1KB banks mapped to PPU $0000-$1FFF
/// - **PRG Banking Modes** (controlled by bit 6 of $8000):
///   * Mode 0: R6 at $8000, (-2) at $A000, R7 at $C000, (-1) at $E000
///   * Mode 1: (-2) at $8000, R6 at $A000, R7 at $C000, (-1) at $E000
///   * (-2) = second-last bank, (-1) = last bank (fixed)
/// - **CHR Banking Modes** (controlled by bit 7 of $8000):
///   * Mode 0: Two 2KB banks at $0000/$0800, four 1KB banks at $1000-$1FFF
///   * Mode 1: Four 1KB banks at $0000-$0FFF, two 2KB banks at $1000/$1800
/// - **Mirroring**: Switchable H/V via register at $A000 (some boards have fixed)
///
/// # Key Differences from MMC3
/// - No IRQ counter or IRQ-related registers
/// - Simpler implementation used in earlier Namco games
/// - Typically smaller ROM sizes
///
/// Used in games like Dragon Spirit, Famista, etc.
#[derive(Debug)]
pub struct Namco118 {
    prg_rom: Vec<u8>,
    chr_rom: Vec<u8>,
    bank_select: u8,
    bank_regs: [u8; 8],
    prg_banks: [usize; 4], // four 8KB banks mapped to $8000/$A000/$C000/$E000
    chr_banks: [usize; 8], // eight 1KB banks mapped to $0000-$1FFF
    prg_mode: bool,
    chr_mode: bool,
}

impl Namco118 {
    pub fn new(cart: Cartridge, ppu: &mut Ppu) -> Self {
        let mut m = Self {
            prg_rom: cart.prg_rom,
            chr_rom: cart.chr_rom,
            bank_select: 0,
            bank_regs: [0; 8],
            prg_banks: [0; 4],
            chr_banks: [0; 8],
            prg_mode: false,
            chr_mode: false,
        };
        m.apply_banks(ppu);
        // Respect initial mirroring from header until mapper writes override it
        ppu.set_mirroring(cart.mirroring);
        m
    }

    fn prg_bank_count(&self) -> usize {
        std::cmp::max(1, self.prg_rom.len() / 0x2000)
    }

    fn chr_bank_count(&self) -> usize {
        std::cmp::max(1, self.chr_rom.len() / 0x0400)
    }

    fn apply_banks(&mut self, ppu: &mut Ppu) {
        // PRG banking
        let prg_count = self.prg_bank_count();
        let last = prg_count.saturating_sub(1);
        let second_last = prg_count.saturating_sub(2);
        let bank6 = (self.bank_regs[6] as usize) % prg_count;
        let bank7 = (self.bank_regs[7] as usize) % prg_count;

        if !self.prg_mode {
            // Mode 0: R6 at $8000, (-2) at $A000, R7 at $C000, (-1) at $E000
            self.prg_banks = [bank6, second_last, bank7, last];
        } else {
            // Mode 1: (-2) at $8000, R6 at $A000, R7 at $C000, (-1) at $E000
            self.prg_banks = [second_last, bank6, bank7, last];
        }

        // CHR banking (1KB units with two 2KB registers)
        let chr_count = self.chr_bank_count();
        let r0 = (self.bank_regs[0] & 0xFE) as usize % chr_count;
        let r1 = (self.bank_regs[1] & 0xFE) as usize % chr_count;
        let r2 = (self.bank_regs[2] as usize) % chr_count;
        let r3 = (self.bank_regs[3] as usize) % chr_count;
        let r4 = (self.bank_regs[4] as usize) % chr_count;
        let r5 = (self.bank_regs[5] as usize) % chr_count;

        if !self.chr_mode {
            self.chr_banks = [r0, r0 + 1, r1, r1 + 1, r2, r3, r4, r5];
        } else {
            self.chr_banks = [r2, r3, r4, r5, r0, r0 + 1, r1, r1 + 1];
        }

        self.update_chr_mapping(ppu);
    }

    fn update_chr_mapping(&self, ppu: &mut Ppu) {
        if ppu.chr.len() < 0x2000 {
            ppu.chr.resize(0x2000, 0);
        }

        // For CHR RAM carts, skip copying (PPU owns RAM)
        if self.chr_rom.is_empty() {
            return;
        }

        for (i, bank) in self.chr_banks.iter().enumerate() {
            let dst_start = i * 0x0400;
            let src_start = bank.saturating_mul(0x0400);
            let src_end = src_start.saturating_add(0x0400);
            if src_end <= self.chr_rom.len() {
                ppu.chr[dst_start..dst_start + 0x0400]
                    .copy_from_slice(&self.chr_rom[src_start..src_end]);
            } else {
                // Out-of-range banks return 0s
                for b in &mut ppu.chr[dst_start..dst_start + 0x0400] {
                    *b = 0;
                }
            }
        }
    }

    pub fn read_prg(&self, addr: u16) -> u8 {
        let bank = ((addr - 0x8000) / 0x2000) as usize;
        let offset = (addr as usize) & 0x1FFF;
        if bank >= self.prg_banks.len() {
            return 0;
        }
        let base = self.prg_banks[bank].saturating_mul(0x2000);
        let idx = base + offset;
        self.prg_rom.get(idx).copied().unwrap_or(0)
    }

    pub fn write_prg(&mut self, addr: u16, val: u8, ppu: &mut Ppu, _cpu_cycles: u64) {
        match addr {
            0x8000..=0x9FFF => {
                if addr & 1 == 0 {
                    // Bank select
                    self.bank_select = val & 0x07;
                    self.prg_mode = (val & 0x40) != 0;
                    self.chr_mode = (val & 0x80) != 0;
                    self.apply_banks(ppu);
                } else {
                    // Bank data
                    self.bank_regs[self.bank_select as usize] = val;
                    self.apply_banks(ppu);
                }
            }
            0xA000..=0xBFFF => {
                if addr & 1 == 0 {
                    // Mirroring control: 0=vertical, 1=horizontal
                    let mir = if val & 1 == 0 {
                        Mirroring::Vertical
                    } else {
                        Mirroring::Horizontal
                    };
                    ppu.set_mirroring(mir);
                } else {
                    // PRG RAM protect (ignored - mapper 206 has no PRG RAM)
                }
            }
            // Note: No IRQ-related registers ($C000-$FFFF) unlike MMC3
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
    fn namco118_prg_banking_mode0() {
        let mut prg = vec![0; 0x20000]; // 16 banks of 8KB
        for i in 0..16 {
            prg[i * 0x2000] = (i + 1) as u8;
        }

        let cart = Cartridge {
            prg_rom: prg,
            chr_rom: vec![0; 0x2000],
            mapper: 206,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Vertical,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Vertical);
        let mut n118 = Namco118::new(cart, &mut ppu);

        // Mode 0 (default): R6 at $8000, (-2) at $A000, R7 at $C000, (-1) at $E000
        // Set R6 = 5, R7 = 7
        n118.write_prg(0x8000, 6, &mut ppu, 0); // Select register 6
        n118.write_prg(0x8001, 5, &mut ppu, 0); // R6 = 5
        n118.write_prg(0x8000, 7, &mut ppu, 0); // Select register 7
        n118.write_prg(0x8001, 7, &mut ppu, 0); // R7 = 7

        assert_eq!(n118.read_prg(0x8000), 6); // R6 (bank 5)
        assert_eq!(n118.read_prg(0xA000), 15); // (-2) = bank 14
        assert_eq!(n118.read_prg(0xC000), 8); // R7 (bank 7)
        assert_eq!(n118.read_prg(0xE000), 16); // (-1) = bank 15
    }

    #[test]
    fn namco118_prg_banking_mode1() {
        let mut prg = vec![0; 0x20000]; // 16 banks
        for i in 0..16 {
            prg[i * 0x2000] = (i + 1) as u8;
        }

        let cart = Cartridge {
            prg_rom: prg,
            chr_rom: vec![0; 0x2000],
            mapper: 206,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Vertical,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Vertical);
        let mut n118 = Namco118::new(cart, &mut ppu);

        // Enable PRG mode 1 (bit 6 = 1)
        n118.write_prg(0x8000, 0x46, &mut ppu, 0); // Select R6, enable PRG mode 1
        n118.write_prg(0x8001, 5, &mut ppu, 0);
        n118.write_prg(0x8000, 0x47, &mut ppu, 0); // Select R7
        n118.write_prg(0x8001, 7, &mut ppu, 0);

        // Mode 1: (-2) at $8000, R6 at $A000, R7 at $C000, (-1) at $E000
        assert_eq!(n118.read_prg(0x8000), 15); // (-2) = bank 14
        assert_eq!(n118.read_prg(0xA000), 6); // R6 (bank 5)
        assert_eq!(n118.read_prg(0xC000), 8); // R7 (bank 7)
        assert_eq!(n118.read_prg(0xE000), 16); // (-1) = bank 15
    }

    #[test]
    fn namco118_chr_banking_mode0() {
        let mut chr = vec![0; 0x2000]; // 8 banks of 1KB
        for i in 0..8 {
            chr[i * 0x400] = (i + 1) as u8;
        }

        let cart = Cartridge {
            prg_rom: vec![0; 0x8000],
            chr_rom: chr,
            mapper: 206,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Horizontal,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Horizontal);
        let mut n118 = Namco118::new(cart, &mut ppu);

        // Mode 0 (default): two 2KB banks at $0000/$0800, four 1KB banks at $1000-$1FFF
        // R0/R1 are 2KB (ignore LSB), R2-R5 are 1KB
        n118.write_prg(0x8000, 0, &mut ppu, 0); // Select R0
        n118.write_prg(0x8001, 2, &mut ppu, 0); // R0 = 2 (2KB at banks 2-3)
        n118.write_prg(0x8000, 1, &mut ppu, 0); // Select R1
        n118.write_prg(0x8001, 4, &mut ppu, 0); // R1 = 4 (2KB at banks 4-5)

        assert_eq!(ppu.chr[0x0000], 3); // Bank 2 (R0, LSB cleared)
        assert_eq!(ppu.chr[0x0400], 4); // Bank 3 (R0+1)
        assert_eq!(ppu.chr[0x0800], 5); // Bank 4 (R1, LSB cleared)
        assert_eq!(ppu.chr[0x0C00], 6); // Bank 5 (R1+1)
    }

    #[test]
    fn namco118_mirroring() {
        let cart = Cartridge {
            prg_rom: vec![0; 0x8000],
            chr_rom: vec![0; 0x2000],
            mapper: 206,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Vertical,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Vertical);
        let mut n118 = Namco118::new(cart, &mut ppu);

        // Initially vertical (from cartridge)
        assert_eq!(ppu.get_mirroring(), Mirroring::Vertical);

        // Switch to horizontal
        n118.write_prg(0xA000, 1, &mut ppu, 0);
        assert_eq!(ppu.get_mirroring(), Mirroring::Horizontal);

        // Switch back to vertical
        n118.write_prg(0xA000, 0, &mut ppu, 0);
        assert_eq!(ppu.get_mirroring(), Mirroring::Vertical);
    }

    #[test]
    fn namco118_bank_wrapping() {
        let prg = vec![0x42; 0x4000]; // 2 banks only
        let cart = Cartridge {
            prg_rom: prg,
            chr_rom: vec![0; 0x2000],
            mapper: 206,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Vertical,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Vertical);
        let mut n118 = Namco118::new(cart, &mut ppu);

        // Try to select bank 5, should wrap (5 % 2 = 1)
        n118.write_prg(0x8000, 6, &mut ppu, 0);
        n118.write_prg(0x8001, 5, &mut ppu, 0);

        // Should read from bank 1 (wrapped)
        assert_eq!(n118.read_prg(0x8000), 0x42);
    }
}
