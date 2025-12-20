use crate::cartridge::{Cartridge, Mirroring};
use crate::ppu::Ppu;
#[cfg(test)]
use emu_core::apu::TimingMode;

/// MMC1 (Mapper 1/SxROM) - Switchable PRG and CHR banks with configurable mirroring
#[derive(Debug)]
pub struct Mmc1 {
    prg_rom: Vec<u8>,
    chr_rom: Vec<u8>,
    shift_reg: u8,
    write_count: u8,
    control: u8,
    prg_bank: u8,
    chr_bank0: u8,
    chr_bank1: u8,
    prg_banks: [usize; 2], // two 16KB banks at $8000 and $C000
    chr_banks: [usize; 2], // two 4KB banks at $0000 and $1000
}

impl Mmc1 {
    pub fn new(cart: Cartridge, ppu: &mut Ppu) -> Self {
        let mut m = Self {
            prg_rom: cart.prg_rom,
            chr_rom: cart.chr_rom,
            shift_reg: 0x10,
            write_count: 0,
            control: 0x0C, // default: 16KB PRG switching, 8KB CHR
            prg_bank: 0,
            chr_bank0: 0,
            chr_bank1: 0,
            prg_banks: [0, 0],
            chr_banks: [0, 1],
        };
        // Respect header mirroring until mapper writes override it.
        ppu.set_mirroring(cart.mirroring);
        m.apply_banks(ppu);
        m
    }

    fn prg_bank_count(&self) -> usize {
        std::cmp::max(1, self.prg_rom.len() / 0x4000)
    }

    fn chr_bank_count(&self) -> usize {
        std::cmp::max(1, self.chr_rom.len() / 0x1000)
    }

    fn apply_banks(&mut self, ppu: &mut Ppu) {
        let prg_count = self.prg_bank_count();
        let last = prg_count.saturating_sub(1);
        let prg_mode = (self.control >> 2) & 0x03;
        // PRG bank is 4 bits (0-15), bit 4 is PRG RAM enable (ignored for banking)
        let select = ((self.prg_bank & 0x0F) as usize) % prg_count;

        self.prg_banks = match prg_mode {
            0 | 1 => {
                // 32KB mode: even bank paired with next bank
                // Bit 0 is ignored in 32KB mode
                let even = ((self.prg_bank & 0x0E) as usize) % prg_count;
                [even, (even + 1) % prg_count]
            }
            2 => [0, select],    // fix first, swap upper
            _ => [select, last], // swap lower, fix last
        };

        let chr_mode = (self.control >> 4) & 1 != 0;
        let chr_count = self.chr_bank_count();
        if !chr_mode {
            // 8KB mode
            let bank = (self.chr_bank0 & 0x1E) as usize % chr_count;
            self.chr_banks = [bank, (bank + 1) % chr_count];
        } else {
            self.chr_banks = [
                (self.chr_bank0 as usize) % chr_count,
                (self.chr_bank1 as usize) % chr_count,
            ];
        }

        // Mirroring: 0=single screen low, 1=single screen high, 2=vertical, 3=horizontal
        let mir = match self.control & 0x03 {
            0 => Mirroring::SingleScreenLower,
            1 => Mirroring::SingleScreenUpper,
            2 => Mirroring::Vertical,
            _ => Mirroring::Horizontal,
        };
        ppu.set_mirroring(mir);

        self.update_chr_mapping(ppu);
    }

    fn update_chr_mapping(&self, ppu: &mut Ppu) {
        if ppu.chr.len() < 0x2000 {
            ppu.chr.resize(0x2000, 0);
        }

        // CHR RAM carts skip copying since PPU owns the RAM view.
        if self.chr_rom.is_empty() {
            return;
        }

        for (i, bank) in self.chr_banks.iter().enumerate() {
            let dst_start = i * 0x1000;
            let src_start = bank.saturating_mul(0x1000);
            let src_end = src_start.saturating_add(0x1000);
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

    fn latch_write(&mut self, addr: u16, val: u8, ppu: &mut Ppu) {
        if val & 0x80 != 0 {
            // Reset shift register
            self.shift_reg = 0x10;
            self.write_count = 0;
            self.control = 0x0C;
            self.apply_banks(ppu);
            return;
        }

        // Collect 5 bits, LSB first.
        self.shift_reg = (self.shift_reg >> 1) | ((val & 1) << 4);
        self.write_count = self.write_count.saturating_add(1);

        if self.write_count < 5 {
            return;
        }

        let data = self.shift_reg & 0x1F;
        let target = (addr >> 13) & 0x03; // 0: control, 1: CHR0, 2: CHR1, 3: PRG
        match target {
            0 => self.control = data,
            1 => self.chr_bank0 = data,
            2 => self.chr_bank1 = data,
            3 => self.prg_bank = data,
            _ => {}
        }

        self.shift_reg = 0x10;
        self.write_count = 0;
        self.apply_banks(ppu);
    }

    pub fn read_prg(&self, addr: u16) -> u8 {
        let bank = ((addr - 0x8000) / 0x4000) as usize;
        let offset = (addr as usize) & 0x3FFF;
        let prg_bank = *self.prg_banks.get(bank).unwrap_or(&0);
        let idx = prg_bank.saturating_mul(0x4000) + offset;
        self.prg_rom.get(idx).copied().unwrap_or(0)
    }

    pub fn write_prg(&mut self, addr: u16, val: u8, ppu: &mut Ppu) {
        if (0x8000..=0xFFFF).contains(&addr) {
            self.latch_write(addr, val, ppu);
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
    fn mmc1_serial_write() {
        let cart = Cartridge {
            prg_rom: vec![0; 0x8000], // 2 banks
            chr_rom: vec![0; 0x2000], // 2 banks
            mapper: 1,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Horizontal,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Horizontal);
        let mut mmc1 = Mmc1::new(cart, &mut ppu);

        // Write 5 bits to control register (address $8000-9FFF)
        // Writing 0x0C (binary 01100) for vertical mirroring + 16KB PRG mode
        mmc1.write_prg(0x8000, 0, &mut ppu); // bit 0
        mmc1.write_prg(0x8000, 0, &mut ppu); // bit 1
        mmc1.write_prg(0x8000, 1, &mut ppu); // bit 2
        mmc1.write_prg(0x8000, 1, &mut ppu); // bit 3
        mmc1.write_prg(0x8000, 0, &mut ppu); // bit 4

        // Control register should now be 0x0C
        assert_eq!(mmc1.control, 0x0C);
    }

    #[test]
    fn mmc1_reset_on_bit7() {
        let cart = Cartridge {
            prg_rom: vec![0; 0x4000],
            chr_rom: vec![],
            mapper: 1,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Horizontal,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Horizontal);
        let mut mmc1 = Mmc1::new(cart, &mut ppu);

        // Start a write sequence
        mmc1.write_prg(0x8000, 1, &mut ppu);
        mmc1.write_prg(0x8000, 1, &mut ppu);

        // Reset with bit 7 set
        mmc1.write_prg(0x8000, 0x80, &mut ppu);

        // Write count should be reset
        assert_eq!(mmc1.write_count, 0);
        assert_eq!(mmc1.shift_reg, 0x10);
    }

    #[test]
    fn mmc1_prg_banking_modes() {
        let mut prg = vec![0; 0x10000]; // 4 banks
        prg[0] = 0x11; // Bank 0
        prg[0x4000] = 0x22; // Bank 1
        prg[0x8000] = 0x33; // Bank 2
        prg[0xC000] = 0x44; // Bank 3

        let cart = Cartridge {
            prg_rom: prg,
            chr_rom: vec![],
            mapper: 1,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Horizontal,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Horizontal);
        let mmc1 = Mmc1::new(cart, &mut ppu);

        // Default mode (3): swap lower bank, fix last
        // Bank 0 at $8000, Bank 3 at $C000
        assert_eq!(mmc1.read_prg(0x8000), 0x11);
        assert_eq!(mmc1.read_prg(0xC000), 0x44);
    }

    #[test]
    fn mmc1_partial_write_sequence() {
        let cart = Cartridge {
            prg_rom: vec![0; 0x8000],
            chr_rom: vec![],
            mapper: 1,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Horizontal,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Horizontal);
        let mut mmc1 = Mmc1::new(cart, &mut ppu);

        // Write only 3 bits (incomplete sequence)
        mmc1.write_prg(0x8000, 1, &mut ppu);
        mmc1.write_prg(0x8000, 1, &mut ppu);
        mmc1.write_prg(0x8000, 1, &mut ppu);

        // Control register should not have changed yet
        assert_eq!(
            mmc1.control, 0x0C,
            "Partial write should not update register"
        );
        assert_eq!(mmc1.write_count, 3, "Write count should be 3");
    }

    #[test]
    fn mmc1_32kb_prg_mode() {
        let mut prg = vec![0; 0x10000]; // 4 banks
        for i in 0..4 {
            prg[i * 0x4000] = (0x10 + i) as u8;
        }

        let cart = Cartridge {
            prg_rom: prg,
            chr_rom: vec![],
            mapper: 1,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Horizontal,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Horizontal);
        let mut mmc1 = Mmc1::new(cart, &mut ppu);

        // Switch to 32KB mode (control bits [3:2] = 00 or 01)
        // Write 0x00 to control: mirroring=single lower, CHR=8KB, PRG=32KB
        for _i in 0..5 {
            mmc1.write_prg(0x8000, 0, &mut ppu);
        }
        assert_eq!(mmc1.control, 0x00);

        // Select bank 2 via PRG register (will select banks 2-3 in 32KB mode)
        for i in 0..5 {
            mmc1.write_prg(0xE000, (2 >> i) & 1, &mut ppu);
        }

        // In 32KB mode, bit 0 is ignored, so bank 2 -> banks 2-3
        assert_eq!(mmc1.read_prg(0x8000), 0x12, "Lower 16KB should be bank 2");
        assert_eq!(mmc1.read_prg(0xC000), 0x13, "Upper 16KB should be bank 3");
    }

    #[test]
    fn mmc1_fix_first_bank_mode() {
        let mut prg = vec![0; 0x10000]; // 4 banks
        for i in 0..4 {
            prg[i * 0x4000] = (0x10 + i) as u8;
        }

        let cart = Cartridge {
            prg_rom: prg,
            chr_rom: vec![],
            mapper: 1,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Horizontal,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Horizontal);
        let mut mmc1 = Mmc1::new(cart, &mut ppu);

        // Switch to "fix first" mode (control bits [3:2] = 10)
        // Write 0x08 to control: mirroring=single lower, CHR=8KB, PRG=fix first
        for i in 0..5 {
            mmc1.write_prg(0x8000, (0x08 >> i) & 1, &mut ppu);
        }
        assert_eq!(mmc1.control, 0x08);

        // Select bank 2 via PRG register
        for i in 0..5 {
            mmc1.write_prg(0xE000, (2 >> i) & 1, &mut ppu);
        }

        // In fix-first mode: bank 0 at $8000, selected bank at $C000
        assert_eq!(
            mmc1.read_prg(0x8000),
            0x10,
            "First bank should be fixed at $8000"
        );
        assert_eq!(mmc1.read_prg(0xC000), 0x12, "Bank 2 should be at $C000");
    }

    #[test]
    fn mmc1_chr_4kb_mode() {
        let mut chr = vec![0; 0x4000]; // 4 CHR banks
        for i in 0..4 {
            chr[i * 0x1000] = (0x20 + i) as u8;
        }

        let cart = Cartridge {
            prg_rom: vec![0; 0x8000],
            chr_rom: chr,
            mapper: 1,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Horizontal,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Horizontal);
        let mut mmc1 = Mmc1::new(cart, &mut ppu);

        // Switch to 4KB CHR mode (control bit 4 = 1)
        // Write 0x1C to control: mirroring=horiz, CHR=4KB, PRG=fix last
        for i in 0..5 {
            mmc1.write_prg(0x8000, (0x1C >> i) & 1, &mut ppu);
        }
        assert_eq!(mmc1.control, 0x1C);

        // Select CHR bank 1 for $0000-$0FFF
        for i in 0..5 {
            mmc1.write_prg(0xA000, (1 >> i) & 1, &mut ppu);
        }

        // Select CHR bank 2 for $1000-$1FFF
        for i in 0..5 {
            mmc1.write_prg(0xC000, (2 >> i) & 1, &mut ppu);
        }

        // Verify CHR banks
        assert_eq!(ppu.chr[0], 0x21, "CHR bank 1 at $0000");
        assert_eq!(ppu.chr[0x1000], 0x22, "CHR bank 2 at $1000");
    }

    #[test]
    fn mmc1_chr_8kb_mode() {
        let mut chr = vec![0; 0x4000]; // 4 CHR banks
        for i in 0..4 {
            chr[i * 0x1000] = (0x30 + i) as u8;
        }

        let cart = Cartridge {
            prg_rom: vec![0; 0x8000],
            chr_rom: chr,
            mapper: 1,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Horizontal,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Horizontal);
        let mut mmc1 = Mmc1::new(cart, &mut ppu);

        // Default is 8KB CHR mode (control bit 4 = 0)
        // CHR bank 0 register selects 8KB bank (bit 0 ignored)
        // Select CHR banks 2-3 (write even value 2)
        for i in 0..5 {
            mmc1.write_prg(0xA000, (2 >> i) & 1, &mut ppu);
        }

        assert_eq!(ppu.chr[0], 0x32, "CHR bank 2 at $0000 in 8KB mode");
        assert_eq!(ppu.chr[0x1000], 0x33, "CHR bank 3 at $1000 in 8KB mode");
    }

    #[test]
    fn mmc1_all_mirroring_modes() {
        let cart = Cartridge {
            prg_rom: vec![0; 0x8000],
            chr_rom: vec![],
            mapper: 1,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Horizontal,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Horizontal);
        let mut mmc1 = Mmc1::new(cart, &mut ppu);

        // Test single-screen lower (control & 0x03 = 0)
        for i in 0..5 {
            mmc1.write_prg(0x8000, (0x0C >> i) & 1, &mut ppu); // 0x0C has bits [1:0] = 00
        }
        assert_eq!(ppu.get_mirroring(), Mirroring::SingleScreenLower);

        // Test single-screen upper (control & 0x03 = 1)
        for i in 0..5 {
            mmc1.write_prg(0x8000, (0x0D >> i) & 1, &mut ppu); // 0x0D has bits [1:0] = 01
        }
        assert_eq!(ppu.get_mirroring(), Mirroring::SingleScreenUpper);

        // Test vertical (control & 0x03 = 2)
        for i in 0..5 {
            mmc1.write_prg(0x8000, (0x0E >> i) & 1, &mut ppu); // 0x0E has bits [1:0] = 10
        }
        assert_eq!(ppu.get_mirroring(), Mirroring::Vertical);

        // Test horizontal (control & 0x03 = 3)
        for i in 0..5 {
            mmc1.write_prg(0x8000, (0x0F >> i) & 1, &mut ppu); // 0x0F has bits [1:0] = 11
        }
        assert_eq!(ppu.get_mirroring(), Mirroring::Horizontal);
    }

    #[test]
    fn mmc1_prg_bank_wrapping() {
        let mut prg = vec![0; 0x8000]; // 2 banks
        prg[0] = 0x11;
        prg[0x4000] = 0x22;

        let cart = Cartridge {
            prg_rom: prg,
            chr_rom: vec![],
            mapper: 1,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Horizontal,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Horizontal);
        let mut mmc1 = Mmc1::new(cart, &mut ppu);

        // Try to select bank 10 (should wrap to 10 % 2 = 0)
        for i in 0..5 {
            mmc1.write_prg(0xE000, (10 >> i) & 1, &mut ppu);
        }

        assert_eq!(mmc1.read_prg(0x8000), 0x11, "Bank 10 should wrap to bank 0");
    }

    #[test]
    fn mmc1_chr_bank_wrapping() {
        let mut chr = vec![0; 0x2000]; // 2 CHR banks
        chr[0] = 0xAA;
        chr[0x1000] = 0xBB;

        let cart = Cartridge {
            prg_rom: vec![0; 0x8000],
            chr_rom: chr,
            mapper: 1,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Horizontal,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Horizontal);
        let mut mmc1 = Mmc1::new(cart, &mut ppu);

        // Switch to 4KB CHR mode
        for i in 0..5 {
            mmc1.write_prg(0x8000, (0x1C >> i) & 1, &mut ppu);
        }

        // Try to select CHR bank 5 at $0000 (should wrap to 5 % 2 = 1)
        for i in 0..5 {
            mmc1.write_prg(0xA000, (5 >> i) & 1, &mut ppu);
        }

        assert_eq!(ppu.chr[0], 0xBB, "CHR bank 5 should wrap to bank 1");
    }
}
