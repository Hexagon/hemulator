use crate::cartridge::{Cartridge, Mirroring};
use crate::ppu::Ppu;
#[cfg(test)]
use emu_core::apu::TimingMode;

/// MMC3 (Mapper 4/TxROM) - Advanced mapper with PRG/CHR banking and scanline IRQ counter
///
/// # Hardware Behavior (per NESdev wiki)
/// - **PRG ROM**: Up to 512 KB, four 8KB banks mapped to CPU $8000-$FFFF
/// - **CHR ROM**: Up to 256 KB, eight 1KB banks mapped to PPU $0000-$1FFF
/// - **PRG Banking Modes** (controlled by bit 6 of $8000):
///   * Mode 0: R6 at $8000, (-2) at $A000, R7 at $C000, (-1) at $E000
///   * Mode 1: (-2) at $8000, R6 at $A000, R7 at $C000, (-1) at $E000
///   * (-2) = second-last bank, (-1) = last bank (fixed)
/// - **CHR Banking Modes** (controlled by bit 7 of $8000):
///   * Mode 0: Two 2KB banks at $0000/$0800, four 1KB banks at $1000-$1FFF
///   * Mode 1: Four 1KB banks at $0000-$0FFF, two 2KB banks at $1000/$1800
/// - **IRQ Counter**: Scanline-based counter triggered by PPU A12 rising edges
///   * $C000: IRQ latch (reload value)
///   * $C001: IRQ reload (clears counter, sets reload flag)
///   * $E000: IRQ disable (also clears pending)
///   * $E001: IRQ enable
///   * Uses "new" MMC3B/C behavior: IRQ fires after counter decrements to 0
///
/// # Implementation Notes
/// This implementation uses the "new/sharp" IRQ behavior where the counter
/// triggers IRQ only when it decrements to 0, not when it reloads to 0.
#[derive(Debug)]
pub struct Mmc3 {
    prg_rom: Vec<u8>,
    chr_rom: Vec<u8>,
    bank_select: u8,
    bank_regs: [u8; 8],
    prg_banks: [usize; 4], // four 8KB banks mapped to $8000/$A000/$C000/$E000
    chr_banks: [usize; 8], // eight 1KB banks mapped to $0000-$1FFF
    prg_mode: bool,
    chr_mode: bool,
    irq_latch: u8,
    irq_counter: u8,
    irq_reload: bool,
    irq_enabled: bool,
    irq_pending: bool,
    last_a12: bool,
}

impl Mmc3 {
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
            irq_latch: 0,
            irq_counter: 0,
            irq_reload: false,
            irq_enabled: false,
            irq_pending: false,
            last_a12: false,
        };
        m.apply_banks(ppu);
        // Respect initial mirroring from header until mapper writes override it.
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

        // For CHR RAM carts, skip copying (PPU owns RAM). For CHR ROM, copy selected banks into 0x0000-0x1FFF view.
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
                // Out-of-range banks return 0s.
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

    pub fn write_prg(&mut self, addr: u16, val: u8, ppu: &mut Ppu) {
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
                    // PRG RAM protect (ignored)
                }
            }
            0xC000..=0xDFFF => {
                if addr & 1 == 0 {
                    self.irq_latch = val;
                } else {
                    // Per MMC3 spec, writing $C001 clears the counter immediately and
                    // requests a reload on the next A12 rising edge.
                    self.irq_counter = 0;
                    self.irq_reload = true;
                }
            }
            0xE000..=0xFFFF => {
                if addr & 1 == 0 {
                    self.irq_enabled = false;
                    self.irq_pending = false; // disabling also clears pending
                } else {
                    self.irq_enabled = true;
                }
            }
            _ => {}
        }
    }

    pub fn notify_a12(&mut self, a12_high: bool) {
        if !self.last_a12 && a12_high {
            // Rising edge clocks the counter per MMC3 spec.
            // When clocked: if counter==0 OR reload flag set, reload from latch; else decrement.
            let did_decrement = if self.irq_reload || self.irq_counter == 0 {
                self.irq_counter = self.irq_latch;
                self.irq_reload = false;
                false // Reloaded, did not decrement
            } else {
                self.irq_counter = self.irq_counter.saturating_sub(1);
                true // Decremented
            };

            // "New"/Sharp behavior: trigger IRQ when counter DECREMENTS to 0, not when RELOADED to 0.
            if self.irq_counter == 0 && self.irq_enabled && did_decrement {
                self.irq_pending = true;
            }
        }
        self.last_a12 = a12_high;
    }

    pub fn take_irq_pending(&mut self) -> bool {
        let was = self.irq_pending;
        self.irq_pending = false;
        was
    }

    pub fn prg_rom(&self) -> &[u8] {
        &self.prg_rom
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mmc3_prg_banking() {
        let mut prg = vec![0; 0x10000]; // 8 banks of 8KB
        prg[0] = 0x11; // Bank 0
        prg[0x2000] = 0x22; // Bank 1
        prg[0xE000] = 0x88; // Bank 7 (last)

        let cart = Cartridge {
            prg_rom: prg,
            chr_rom: vec![],
            mapper: 4,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Horizontal,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Horizontal);
        let mut mmc3 = Mmc3::new(cart, &mut ppu);

        // Default: bank6=0, bank7=0, second_last=6, last=7
        // Mode 0: [bank6=0, second_last=6, bank7=0, last=7] at $8000, $A000, $C000, $E000
        assert_eq!(mmc3.read_prg(0x8000), 0x11); // Bank 0
        assert_eq!(mmc3.read_prg(0xE000), 0x88); // Bank 7 (last)

        // Switch bank 6 to 1
        mmc3.write_prg(0x8000, 6, &mut ppu); // Select bank register 6
        mmc3.write_prg(0x8001, 1, &mut ppu); // Set it to 1

        assert_eq!(mmc3.read_prg(0x8000), 0x22); // Now bank 1
    }

    #[test]
    fn mmc3_irq_counter() {
        let cart = Cartridge {
            prg_rom: vec![0; 0x8000],
            chr_rom: vec![0; 0x2000],
            mapper: 4,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Horizontal,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Horizontal);
        let mut mmc3 = Mmc3::new(cart, &mut ppu);

        // Set IRQ latch to 2
        mmc3.write_prg(0xC000, 2, &mut ppu);
        // Reload counter (sets flag, actual reload happens on next A12 edge)
        mmc3.write_prg(0xC001, 0, &mut ppu);
        // Enable IRQ
        mmc3.write_prg(0xE001, 0, &mut ppu);

        // Counter hasn't been reloaded yet (no A12 edge)
        assert_eq!(mmc3.irq_counter, 0);
        assert!(!mmc3.irq_pending);

        // Simulate A12 rising edges (PPU fetches)
        mmc3.notify_a12(false);
        mmc3.notify_a12(true); // Counter reloaded to 2 because irq_reload was set
        assert_eq!(mmc3.irq_counter, 2);

        mmc3.notify_a12(false);
        mmc3.notify_a12(true); // Counter decrements to 1
        assert_eq!(mmc3.irq_counter, 1);

        mmc3.notify_a12(false);
        mmc3.notify_a12(true); // Counter decrements to 0, IRQ fires
        assert_eq!(mmc3.irq_counter, 0);
        assert!(mmc3.irq_pending);
    }

    #[test]
    fn mmc3_mirroring_control() {
        let cart = Cartridge {
            prg_rom: vec![0; 0x4000],
            chr_rom: vec![],
            mapper: 4,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Horizontal,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Horizontal);
        let mut mmc3 = Mmc3::new(cart, &mut ppu);

        // Switch to vertical mirroring
        mmc3.write_prg(0xA000, 0, &mut ppu);
        // PPU should now have vertical mirroring set
        // (We can't directly test this without accessing ppu.mirroring)

        // Switch to horizontal mirroring
        mmc3.write_prg(0xA000, 1, &mut ppu);
    }

    #[test]
    fn mmc3_irq_zero_latch() {
        let cart = Cartridge {
            prg_rom: vec![0; 0x8000],
            chr_rom: vec![0; 0x2000],
            mapper: 4,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Horizontal,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Horizontal);
        let mut mmc3 = Mmc3::new(cart, &mut ppu);

        // Set IRQ latch to 0
        mmc3.write_prg(0xC000, 0, &mut ppu);
        // Reload counter
        mmc3.write_prg(0xC001, 0, &mut ppu);
        // Enable IRQ
        mmc3.write_prg(0xE001, 0, &mut ppu);

        // First A12 edge should reload counter to 0
        mmc3.notify_a12(false);
        mmc3.notify_a12(true);

        // According to MMC3 spec, reloading to 0 should NOT fire IRQ
        // IRQ should only fire when counter DECREMENTS to 0
        assert!(
            !mmc3.take_irq_pending(),
            "IRQ should not fire when reloading to 0"
        );

        // Second A12 edge: counter is already 0, should reload again to 0
        mmc3.notify_a12(false);
        mmc3.notify_a12(true);
        assert!(
            !mmc3.take_irq_pending(),
            "IRQ should not fire when reloading to 0 again"
        );
    }
}
