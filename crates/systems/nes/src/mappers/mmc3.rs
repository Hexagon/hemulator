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
    /// PRG RAM protection register ($A001)
    /// Bit 7: 1=Enable chip, 0=Disable chip
    /// Bit 6: 0=Allow writes, 1=Deny writes
    /// Default: 0x80 (enabled, writable) to match common emulator behavior
    prg_ram_protect: u8,
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
            // Default to enabled and writable to match common emulator behavior
            // and previous "always on" behavior of NesBus.
            prg_ram_protect: 0x80,
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

        // CRITICAL: PRG bank ordering (DO NOT CHANGE - fixes Super Mario Bros. 3 and other games)
        // The key difference between mode 0 and mode 1 is WHERE the fixed banks appear,
        // while R6 and R7 swap positions between $A000 and $C000.
        //
        // This was verified against:
        // - NESdev wiki MMC3 documentation
        // - Mesen2 MMC3.cpp implementation
        // - Actual testing with Super Mario Bros. 3 (requires correct banking)
        //
        // Common mistake: Swapping R7 with second_last in mode 0, which breaks SMB3!
        if !self.prg_mode {
            // Mode 0: R6 at $8000, R7 at $A000, (-2) at $C000, (-1) at $E000
            // R7 MUST be at $A000, not $C000
            self.prg_banks = [bank6, bank7, second_last, last];
        } else {
            // Mode 1: (-2) at $8000, R7 at $A000, R6 at $C000, (-1) at $E000
            // R7 stays at $A000, R6 moves to $C000
            self.prg_banks = [second_last, bank7, bank6, last];
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
            self.chr_banks = [
                r0,
                (r0 + 1) % chr_count,
                r1,
                (r1 + 1) % chr_count,
                r2,
                r3,
                r4,
                r5,
            ];
        } else {
            self.chr_banks = [
                r2,
                r3,
                r4,
                r5,
                r0,
                (r0 + 1) % chr_count,
                r1,
                (r1 + 1) % chr_count,
            ];
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
                    // PRG RAM protect ($A001)
                    // Bit 7: 1=Enable chip, 0=Disable chip
                    // Bit 6: 0=Allow writes, 1=Deny writes
                    self.prg_ram_protect = val;
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

            // MMC3B/C (Sharp/new) behavior: trigger IRQ only when counter DECREMENTS to 0.
            // MMC3A (NEC/old/alternate) behavior would trigger when counter==0 regardless of reload.
            // We use MMC3B/C by default as it's more common and matches most emulator behavior.
            if self.irq_counter == 0 && self.irq_enabled && did_decrement {
                self.irq_pending = true;
            }
        }
        self.last_a12 = a12_high;
    }

    pub fn take_irq_pending(&mut self) -> bool {
        self.irq_pending
    }

    pub fn prg_rom(&self) -> &[u8] {
        &self.prg_rom
    }

    #[allow(dead_code)] // Part of mapper API, not yet integrated with bus
    pub fn wram_access(&self) -> (bool, bool) {
        // Bit 7: 1=Enable, 0=Disable
        // Bit 6: 0=Write Allow, 1=Write Deny
        let enabled = (self.prg_ram_protect & 0x80) != 0;
        let write_allow = (self.prg_ram_protect & 0x40) == 0;
        (enabled, enabled && write_allow)
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

    #[test]
    fn mmc3_irq_decrement_from_1_to_0() {
        let cart = Cartridge {
            prg_rom: vec![0; 0x8000],
            chr_rom: vec![0; 0x2000],
            mapper: 4,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Horizontal,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Horizontal);
        let mut mmc3 = Mmc3::new(cart, &mut ppu);

        // Set IRQ latch to 1
        mmc3.write_prg(0xC000, 1, &mut ppu);
        // Reload counter
        mmc3.write_prg(0xC001, 0, &mut ppu);
        // Enable IRQ
        mmc3.write_prg(0xE001, 0, &mut ppu);

        // First A12 edge: reload counter to 1
        mmc3.notify_a12(false);
        mmc3.notify_a12(true);
        assert!(!mmc3.take_irq_pending(), "No IRQ when reloading to 1");

        // Second A12 edge: decrement from 1 to 0, should fire IRQ
        mmc3.notify_a12(false);
        mmc3.notify_a12(true);
        assert!(
            mmc3.take_irq_pending(),
            "IRQ should fire when decrementing to 0"
        );
    }

    #[test]
    fn mmc3_irq_latch_value_1_cycle() {
        let cart = Cartridge {
            prg_rom: vec![0; 0x8000],
            chr_rom: vec![0; 0x2000],
            mapper: 4,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Horizontal,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Horizontal);
        let mut mmc3 = Mmc3::new(cart, &mut ppu);

        // Set IRQ latch to 1
        mmc3.write_prg(0xC000, 1, &mut ppu);
        // Reload counter
        mmc3.write_prg(0xC001, 0, &mut ppu);
        // Enable IRQ
        mmc3.write_prg(0xE001, 0, &mut ppu);

        // First A12 edge: reload to 1
        mmc3.notify_a12(false);
        mmc3.notify_a12(true);
        assert!(!mmc3.take_irq_pending());

        // Second A12 edge: decrement to 0, fires IRQ
        mmc3.notify_a12(false);
        mmc3.notify_a12(true);
        assert!(mmc3.take_irq_pending());

        // Acknowledge IRQ (disable then enable)
        mmc3.write_prg(0xE000, 0, &mut ppu);
        mmc3.write_prg(0xE001, 0, &mut ppu);

        // Third A12 edge: counter is 0, reload to 1
        mmc3.notify_a12(false);
        mmc3.notify_a12(true);
        assert!(!mmc3.take_irq_pending());

        // Fourth A12 edge: decrement to 0 again, fires IRQ
        mmc3.notify_a12(false);
        mmc3.notify_a12(true);
        assert!(mmc3.take_irq_pending());
    }

    // ============================================================================
    // MMC3 Edge Case Tests
    // ============================================================================

    #[test]
    fn mmc3_chr_bank_r0_r1_even_alignment() {
        // R0 and R1 are 2KB banks, so bit 0 should be masked to ensure even alignment
        let cart = Cartridge {
            prg_rom: vec![0; 0x8000],
            chr_rom: vec![0x11; 0x10000], // 128KB CHR (128 1KB banks)
            mapper: 4,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Horizontal,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Horizontal);
        let mut mmc3 = Mmc3::new(cart, &mut ppu);

        // Set R0 to an odd value - should be masked to even
        mmc3.write_prg(0x8000, 0, &mut ppu); // Select R0
        mmc3.write_prg(0x8001, 0x05, &mut ppu); // Try to set to 5 (odd)

        // R0 should actually be 4 (0x05 & 0xFE = 0x04)
        assert_eq!(
            mmc3.bank_regs[0], 0x05,
            "Bank register stores original value"
        );
        // But the actual bank used should be even
        let expected_bank = 0x05 & 0xFE; // Masked and wrapped
        assert_eq!(mmc3.chr_banks[0], expected_bank);
        assert_eq!(mmc3.chr_banks[1], expected_bank + 1);

        // Set R1 to an odd value
        mmc3.write_prg(0x8000, 1, &mut ppu); // Select R1
        mmc3.write_prg(0x8001, 0x07, &mut ppu); // Try to set to 7 (odd)

        let expected_bank = 0x07 & 0xFE;
        assert_eq!(mmc3.chr_banks[2], expected_bank);
        assert_eq!(mmc3.chr_banks[3], expected_bank + 1);
    }

    #[test]
    fn mmc3_chr_bank_overflow() {
        // Test that CHR banks wrap correctly when r0+1 or r1+1 would exceed bank count
        let cart = Cartridge {
            prg_rom: vec![0; 0x8000],
            chr_rom: vec![0; 0x2000], // Only 8 1KB banks
            mapper: 4,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Horizontal,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Horizontal);
        let mut mmc3 = Mmc3::new(cart, &mut ppu);

        // Set R0 to 6 (even) - R0+1 would be 7, which is the last valid bank
        mmc3.write_prg(0x8000, 0, &mut ppu);
        mmc3.write_prg(0x8001, 6, &mut ppu);

        // Banks 0 and 1 should be 6 and 7
        assert_eq!(mmc3.chr_banks[0], 6);
        assert_eq!(mmc3.chr_banks[1], 7);

        // Set R0 to 8 (which wraps to 0 after modulo)
        mmc3.write_prg(0x8001, 8, &mut ppu);
        assert_eq!(mmc3.chr_banks[0], 0); // 8 % 8 = 0
        assert_eq!(mmc3.chr_banks[1], 1); // (8 % 8) + 1 = 1
    }

    #[test]
    fn mmc3_prg_mode_switching() {
        let mut prg = vec![0; 0x10000]; // 8 banks of 8KB
        for i in 0..8 {
            prg[i * 0x2000] = (0x11 * (i + 1)) as u8; // Distinctive byte at start of each bank
        }

        let cart = Cartridge {
            prg_rom: prg,
            chr_rom: vec![],
            mapper: 4,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Horizontal,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Horizontal);
        let mut mmc3 = Mmc3::new(cart, &mut ppu);

        // Set R6 to bank 1, R7 to bank 2
        mmc3.write_prg(0x8000, 6, &mut ppu);
        mmc3.write_prg(0x8001, 1, &mut ppu);
        mmc3.write_prg(0x8000, 7, &mut ppu);
        mmc3.write_prg(0x8001, 2, &mut ppu);

        // Mode 0: R6 at $8000, R7 at $A000, (-2) at $C000, (-1) at $E000
        assert_eq!(mmc3.read_prg(0x8000), 0x22); // Bank 1 (R6)
        assert_eq!(mmc3.read_prg(0xA000), 0x33); // Bank 2 (R7)
        assert_eq!(mmc3.read_prg(0xC000), 0x77); // Bank 6 (second last)
        assert_eq!(mmc3.read_prg(0xE000), 0x88); // Bank 7 (last)

        // Switch to mode 1 by setting bit 6 of bank select
        mmc3.write_prg(0x8000, 0x40, &mut ppu);

        // Mode 1: (-2) at $8000, R7 at $A000, R6 at $C000, (-1) at $E000
        assert_eq!(mmc3.read_prg(0x8000), 0x77); // Bank 6 (second last)
        assert_eq!(mmc3.read_prg(0xA000), 0x33); // Bank 2 (R7)
        assert_eq!(mmc3.read_prg(0xC000), 0x22); // Bank 1 (R6)
        assert_eq!(mmc3.read_prg(0xE000), 0x88); // Bank 7 (last)
    }

    #[test]
    fn mmc3_chr_mode_switching() {
        let mut chr = vec![0; 0x4000]; // 16 1KB banks
        for i in 0..16 {
            chr[i * 0x0400] = (0x10 + i) as u8;
        }

        let cart = Cartridge {
            prg_rom: vec![0; 0x4000],
            chr_rom: chr,
            mapper: 4,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Horizontal,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Horizontal);
        let mut mmc3 = Mmc3::new(cart, &mut ppu);

        // Set bank registers
        mmc3.write_prg(0x8000, 0, &mut ppu);
        mmc3.write_prg(0x8001, 0, &mut ppu); // R0 = 0 (banks 0-1)
        mmc3.write_prg(0x8000, 1, &mut ppu);
        mmc3.write_prg(0x8001, 2, &mut ppu); // R1 = 2 (banks 2-3)
        mmc3.write_prg(0x8000, 2, &mut ppu);
        mmc3.write_prg(0x8001, 4, &mut ppu); // R2 = 4
        mmc3.write_prg(0x8000, 3, &mut ppu);
        mmc3.write_prg(0x8001, 5, &mut ppu); // R3 = 5
        mmc3.write_prg(0x8000, 4, &mut ppu);
        mmc3.write_prg(0x8001, 6, &mut ppu); // R4 = 6
        mmc3.write_prg(0x8000, 5, &mut ppu);
        mmc3.write_prg(0x8001, 7, &mut ppu); // R5 = 7

        // Mode 0: [R0, R0+1, R1, R1+1, R2, R3, R4, R5]
        assert_eq!(mmc3.chr_banks, [0, 1, 2, 3, 4, 5, 6, 7]);

        // Switch to mode 1 by setting bit 7
        mmc3.write_prg(0x8000, 0x80, &mut ppu);

        // Mode 1: [R2, R3, R4, R5, R0, R0+1, R1, R1+1]
        assert_eq!(mmc3.chr_banks, [4, 5, 6, 7, 0, 1, 2, 3]);
    }

    #[test]
    fn mmc3_irq_disabled_then_reenabled() {
        let cart = Cartridge {
            prg_rom: vec![0; 0x8000],
            chr_rom: vec![0; 0x2000],
            mapper: 4,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Horizontal,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Horizontal);
        let mut mmc3 = Mmc3::new(cart, &mut ppu);

        // Set IRQ latch and reload
        mmc3.write_prg(0xC000, 1, &mut ppu);
        mmc3.write_prg(0xC001, 0, &mut ppu);
        mmc3.write_prg(0xE001, 0, &mut ppu); // Enable IRQ

        // Clock to reload
        mmc3.notify_a12(false);
        mmc3.notify_a12(true);
        assert_eq!(mmc3.irq_counter, 1);

        // Clock to 0 - should fire
        mmc3.notify_a12(false);
        mmc3.notify_a12(true);
        assert!(mmc3.take_irq_pending());

        // Disable IRQ
        mmc3.write_prg(0xE000, 0, &mut ppu);
        assert!(!mmc3.irq_enabled);
        assert!(!mmc3.irq_pending, "Disabling IRQ should clear pending flag");

        // Clock again - no IRQ should fire
        mmc3.notify_a12(false);
        mmc3.notify_a12(true);
        assert!(!mmc3.take_irq_pending());

        // Re-enable IRQ
        mmc3.write_prg(0xE001, 0, &mut ppu);

        // Counter is now 1, clock to 0 should fire again
        mmc3.notify_a12(false);
        mmc3.notify_a12(true);
        assert!(mmc3.take_irq_pending());
    }

    #[test]
    fn mmc3_multiple_consecutive_a12_edges() {
        let cart = Cartridge {
            prg_rom: vec![0; 0x8000],
            chr_rom: vec![0; 0x2000],
            mapper: 4,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Horizontal,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Horizontal);
        let mut mmc3 = Mmc3::new(cart, &mut ppu);

        // Set IRQ latch to 3
        mmc3.write_prg(0xC000, 3, &mut ppu);
        mmc3.write_prg(0xC001, 0, &mut ppu);
        mmc3.write_prg(0xE001, 0, &mut ppu);

        // Only rising edges should clock the counter
        mmc3.notify_a12(false);
        mmc3.notify_a12(true); // Edge 1: reload to 3
        assert_eq!(mmc3.irq_counter, 3);

        // Multiple high states shouldn't clock
        mmc3.notify_a12(true);
        mmc3.notify_a12(true);
        assert_eq!(mmc3.irq_counter, 3, "No change without rising edge");

        mmc3.notify_a12(false);
        mmc3.notify_a12(true); // Edge 2: decrement to 2
        assert_eq!(mmc3.irq_counter, 2);

        mmc3.notify_a12(false);
        mmc3.notify_a12(true); // Edge 3: decrement to 1
        assert_eq!(mmc3.irq_counter, 1);

        mmc3.notify_a12(false);
        mmc3.notify_a12(true); // Edge 4: decrement to 0, fire IRQ
        assert_eq!(mmc3.irq_counter, 0);
        assert!(mmc3.take_irq_pending());
    }

    #[test]
    fn mmc3_bank_select_register_wrapping() {
        let cart = Cartridge {
            prg_rom: vec![0; 0x8000],
            chr_rom: vec![0; 0x2000],
            mapper: 4,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Horizontal,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Horizontal);
        let mut mmc3 = Mmc3::new(cart, &mut ppu);

        // Bank select register should only use lower 3 bits (0-7)
        mmc3.write_prg(0x8000, 0xFF, &mut ppu); // All bits set
        assert_eq!(mmc3.bank_select, 0x07, "Bank select should mask to 3 bits");

        // Verify we can still write to bank register 7
        mmc3.write_prg(0x8001, 0x42, &mut ppu);
        assert_eq!(mmc3.bank_regs[7], 0x42);

        // Try selecting register 8 (should wrap to 0)
        mmc3.write_prg(0x8000, 0x08, &mut ppu);
        assert_eq!(mmc3.bank_select, 0x00, "Register 8 should wrap to 0");
    }

    #[test]
    fn mmc3_mirroring_changes() {
        let cart = Cartridge {
            prg_rom: vec![0; 0x4000],
            chr_rom: vec![],
            mapper: 4,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Horizontal, // Start with horizontal
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Horizontal);
        let initial_mirroring = ppu.get_mirroring();
        assert_eq!(initial_mirroring, Mirroring::Horizontal);

        let mut mmc3 = Mmc3::new(cart, &mut ppu);

        // Switch to vertical mirroring (bit 0 = 0)
        mmc3.write_prg(0xA000, 0, &mut ppu);
        assert_eq!(ppu.get_mirroring(), Mirroring::Vertical);

        // Switch back to horizontal (bit 0 = 1)
        mmc3.write_prg(0xA000, 1, &mut ppu);
        assert_eq!(ppu.get_mirroring(), Mirroring::Horizontal);

        // Verify other bits are ignored
        mmc3.write_prg(0xA000, 0xFE, &mut ppu); // All bits except bit 0
        assert_eq!(
            ppu.get_mirroring(),
            Mirroring::Vertical,
            "Only bit 0 should matter"
        );
    }

    #[test]
    fn mmc3_irq_reload_clears_counter_immediately() {
        let cart = Cartridge {
            prg_rom: vec![0; 0x8000],
            chr_rom: vec![0; 0x2000],
            mapper: 4,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Horizontal,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Horizontal);
        let mut mmc3 = Mmc3::new(cart, &mut ppu);

        // Set latch to 5
        mmc3.write_prg(0xC000, 5, &mut ppu);
        mmc3.write_prg(0xC001, 0, &mut ppu); // Reload
        mmc3.write_prg(0xE001, 0, &mut ppu); // Enable

        // First A12 edge: reload to 5
        mmc3.notify_a12(false);
        mmc3.notify_a12(true);
        assert_eq!(mmc3.irq_counter, 5);

        // Decrement a few times
        mmc3.notify_a12(false);
        mmc3.notify_a12(true);
        assert_eq!(mmc3.irq_counter, 4);

        // Write to $C001 - should clear counter immediately
        mmc3.write_prg(0xC001, 0, &mut ppu);
        assert_eq!(
            mmc3.irq_counter, 0,
            "Writing to $C001 should clear counter immediately"
        );
        assert!(mmc3.irq_reload, "Reload flag should be set");

        // Next A12 edge should reload from latch
        mmc3.notify_a12(false);
        mmc3.notify_a12(true);
        assert_eq!(mmc3.irq_counter, 5, "Counter should reload from latch");
        assert!(!mmc3.irq_reload, "Reload flag should be cleared");
    }

    #[test]
    fn mmc3_prg_bank_wrapping() {
        // Test with only 4 banks (32KB ROM)
        let mut prg = vec![0; 0x8000]; // 4 banks
        for i in 0..4 {
            prg[i * 0x2000] = (0x10 + i) as u8;
        }

        let cart = Cartridge {
            prg_rom: prg,
            chr_rom: vec![],
            mapper: 4,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Horizontal,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Horizontal);
        let mut mmc3 = Mmc3::new(cart, &mut ppu);

        // Try to set R6 to bank 10 (should wrap to bank 2)
        mmc3.write_prg(0x8000, 6, &mut ppu);
        mmc3.write_prg(0x8001, 10, &mut ppu);

        // 10 % 4 = 2, so should read bank 2's data
        assert_eq!(mmc3.read_prg(0x8000), 0x12, "Bank should wrap with modulo");
    }

    #[test]
    fn mmc3_sequential_bank_register_writes() {
        let cart = Cartridge {
            prg_rom: vec![0; 0x8000],
            chr_rom: vec![0; 0x4000], // 16 banks
            mapper: 4,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Horizontal,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Horizontal);
        let mut mmc3 = Mmc3::new(cart, &mut ppu);

        // Write to all 8 bank registers in sequence
        for i in 0..8 {
            mmc3.write_prg(0x8000, i, &mut ppu); // Select register i
            mmc3.write_prg(0x8001, (i * 2) as u8, &mut ppu); // Write value i*2
        }

        // Verify all registers were set correctly
        for i in 0..8 {
            assert_eq!(
                mmc3.bank_regs[i as usize],
                (i * 2) as u8,
                "Register {} should be {}",
                i,
                i * 2
            );
        }
    }

    #[test]
    fn mmc3_chr_bank_odd_count_wrapping() {
        // Test with an odd number of CHR banks to ensure r0+1 and r1+1 wrap correctly
        let cart = Cartridge {
            prg_rom: vec![0; 0x8000],
            chr_rom: vec![0; 0x1C00], // 7 banks (7KB) - odd count
            mapper: 4,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Horizontal,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Horizontal);
        let mut mmc3 = Mmc3::new(cart, &mut ppu);

        // chr_count should be 7
        assert_eq!(mmc3.chr_bank_count(), 7);

        // Set R0 to 6 (even) - (6 & 0xFE) = 6, 6 % 7 = 6, (6+1) % 7 = 0
        mmc3.write_prg(0x8000, 0, &mut ppu);
        mmc3.write_prg(0x8001, 6, &mut ppu);

        // Banks should be [6, 0, ...] due to wrapping
        assert_eq!(mmc3.chr_banks[0], 6);
        assert_eq!(
            mmc3.chr_banks[1], 0,
            "Bank 7 should wrap to 0 for odd bank count"
        );

        // Set R1 to 4 - (4 & 0xFE) = 4, 4 % 7 = 4, (4+1) % 7 = 5
        mmc3.write_prg(0x8000, 1, &mut ppu);
        mmc3.write_prg(0x8001, 4, &mut ppu);

        assert_eq!(mmc3.chr_banks[2], 4);
        assert_eq!(mmc3.chr_banks[3], 5);

        // Try setting R0 to 8 - (8 & 0xFE) = 8, 8 % 7 = 1, (1+1) % 7 = 2
        mmc3.write_prg(0x8000, 0, &mut ppu);
        mmc3.write_prg(0x8001, 8, &mut ppu);

        assert_eq!(mmc3.chr_banks[0], 1, "8 & 0xFE = 8, 8 % 7 = 1");
        assert_eq!(mmc3.chr_banks[1], 2, "(1 + 1) % 7 = 2");
    }

    // ============================================================================
    // CRITICAL REGRESSION TESTS - DO NOT DELETE OR MODIFY
    // These tests verify fixes for Super Mario Bros. 3 and other games
    // ============================================================================

    #[test]
    fn regression_mmc3_prg_r7_at_a000_mode0() {
        // REGRESSION TEST: Verify R7 is at $A000 in mode 0, not at $C000
        // This was the bug that broke Super Mario Bros. 3
        // Reference: Fixed 2024-12-21
        let mut prg = vec![0; 0x10000]; // 8 banks
        for i in 0..8 {
            prg[i * 0x2000] = (0x10 + i) as u8;
        }

        let cart = Cartridge {
            prg_rom: prg,
            chr_rom: vec![],
            mapper: 4,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Horizontal,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Horizontal);
        let mut mmc3 = Mmc3::new(cart, &mut ppu);

        // Set R6=2, R7=3
        mmc3.write_prg(0x8000, 6, &mut ppu);
        mmc3.write_prg(0x8001, 2, &mut ppu);
        mmc3.write_prg(0x8000, 7, &mut ppu);
        mmc3.write_prg(0x8001, 3, &mut ppu);

        // Ensure we're in mode 0
        assert!(!mmc3.prg_mode);

        // Mode 0 MUST be: R6 at $8000, R7 at $A000, (-2) at $C000, (-1) at $E000
        assert_eq!(
            mmc3.read_prg(0x8000),
            0x12,
            "Mode 0: R6 (bank 2) must be at $8000"
        );
        assert_eq!(
            mmc3.read_prg(0xA000),
            0x13,
            "Mode 0: R7 (bank 3) MUST be at $A000 - this is critical for SMB3!"
        );
        assert_eq!(
            mmc3.read_prg(0xC000),
            0x16,
            "Mode 0: Bank 6 (second last) must be at $C000"
        );
        assert_eq!(
            mmc3.read_prg(0xE000),
            0x17,
            "Mode 0: Bank 7 (last) must be at $E000"
        );
    }

    #[test]
    fn regression_mmc3_prg_r7_at_a000_mode1() {
        // REGRESSION TEST: Verify R7 stays at $A000 in mode 1, R6 moves to $C000
        // Reference: Fixed 2024-12-21
        let mut prg = vec![0; 0x10000]; // 8 banks
        for i in 0..8 {
            prg[i * 0x2000] = (0x10 + i) as u8;
        }

        let cart = Cartridge {
            prg_rom: prg,
            chr_rom: vec![],
            mapper: 4,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Horizontal,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Horizontal);
        let mut mmc3 = Mmc3::new(cart, &mut ppu);

        // Set R6=2, R7=3
        mmc3.write_prg(0x8000, 6, &mut ppu);
        mmc3.write_prg(0x8001, 2, &mut ppu);
        mmc3.write_prg(0x8000, 7, &mut ppu);
        mmc3.write_prg(0x8001, 3, &mut ppu);

        // Switch to mode 1
        mmc3.write_prg(0x8000, 0x40, &mut ppu);
        assert!(mmc3.prg_mode);

        // Mode 1 MUST be: (-2) at $8000, R7 at $A000, R6 at $C000, (-1) at $E000
        assert_eq!(
            mmc3.read_prg(0x8000),
            0x16,
            "Mode 1: Bank 6 (second last) must be at $8000"
        );
        assert_eq!(
            mmc3.read_prg(0xA000),
            0x13,
            "Mode 1: R7 (bank 3) MUST stay at $A000"
        );
        assert_eq!(
            mmc3.read_prg(0xC000),
            0x12,
            "Mode 1: R6 (bank 2) must be at $C000"
        );
        assert_eq!(
            mmc3.read_prg(0xE000),
            0x17,
            "Mode 1: Bank 7 (last) must be at $E000"
        );
    }

    #[test]
    fn regression_mmc3_prg_ram_protection_default() {
        // REGRESSION TEST: Verify PRG RAM protection defaults to enabled+writable
        // Reference: Fixed 2024-12-21
        let cart = Cartridge {
            prg_rom: vec![0; 0x8000],
            chr_rom: vec![],
            mapper: 4,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Horizontal,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Horizontal);
        let mmc3 = Mmc3::new(cart, &mut ppu);

        let (enabled, writable) = mmc3.wram_access();
        assert!(enabled, "PRG RAM should be enabled by default");
        assert!(writable, "PRG RAM should be writable by default");
    }

    #[test]
    fn regression_mmc3_prg_ram_protection_write() {
        // REGRESSION TEST: Verify PRG RAM protection register ($A001) works correctly
        // Reference: Fixed 2024-12-21
        let cart = Cartridge {
            prg_rom: vec![0; 0x8000],
            chr_rom: vec![],
            mapper: 4,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Horizontal,
        };

        let mut ppu = Ppu::new(vec![], Mirroring::Horizontal);
        let mut mmc3 = Mmc3::new(cart, &mut ppu);

        // Disable chip (bit 7 = 0)
        mmc3.write_prg(0xA001, 0x00, &mut ppu);
        let (enabled, writable) = mmc3.wram_access();
        assert!(!enabled, "PRG RAM should be disabled when bit 7 is 0");
        assert!(!writable, "PRG RAM should not be writable when disabled");

        // Enable chip, deny writes (bit 7 = 1, bit 6 = 1)
        mmc3.write_prg(0xA001, 0xC0, &mut ppu);
        let (enabled, writable) = mmc3.wram_access();
        assert!(enabled, "PRG RAM should be enabled when bit 7 is 1");
        assert!(!writable, "PRG RAM writes should be denied when bit 6 is 1");

        // Enable chip, allow writes (bit 7 = 1, bit 6 = 0)
        mmc3.write_prg(0xA001, 0x80, &mut ppu);
        let (enabled, writable) = mmc3.wram_access();
        assert!(enabled, "PRG RAM should be enabled when bit 7 is 1");
        assert!(writable, "PRG RAM writes should be allowed when bit 6 is 0");
    }
}
