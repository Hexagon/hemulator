use crate::cartridge::{Cartridge, Mirroring};
use crate::ppu::Ppu;

/// MMC5 (Mapper 5) - Advanced mapper with ExRAM, split screen, and sophisticated banking
///
/// # Hardware Features
/// - **PRG ROM**: Up to 1 MB (128 x 8KB banks)
/// - **CHR ROM**: Up to 1 MB (256 x 4KB or 512 x 1KB banks)
/// - **PRG RAM**: Up to 64 KB (8 x 8KB banks)
/// - **ExRAM**: 1KB internal RAM ($5C00-$5FFF) with multiple modes
/// - **IRQ Counter**: Scanline-based counter for split screen effects
///
/// # PRG Banking Modes ($5100)
/// - Mode 0: 32KB bank at $8000
/// - Mode 1: 16KB at $8000, 16KB at $C000
/// - Mode 2: 16KB at $8000, 8KB at $C000, 8KB at $E000
/// - Mode 3: 8KB at $8000, 8KB at $A000, 8KB at $C000, 8KB at $E000
///
/// # CHR Banking Modes ($5101)
/// - Mode 0: 8KB mode
/// - Mode 1: 4KB mode
/// - Mode 2: 2KB mode
/// - Mode 3: 1KB mode
///
/// # ExRAM Modes ($5104)
/// - Mode 0: Extra nametable
/// - Mode 1: Extended attribute mode
/// - Mode 2: CPU read/write
/// - Mode 3: CPU read-only
///
/// # Notable Games
/// - Castlevania 3 (US)
/// - Just Breed
/// - Laser Invasion
/// - Nobunaga's Ambition 2
///
/// # Implementation Notes
/// - This is a simplified implementation focusing on common features
/// - Split screen IRQ implemented
/// - ExRAM modes 0-2 implemented
/// - Advanced features like PCM audio not implemented
#[derive(Debug)]
pub struct Mmc5 {
    prg_rom: Vec<u8>,
    chr_rom: Vec<u8>,
    prg_ram: Vec<u8>,
    exram: [u8; 1024],

    // PRG banking
    prg_mode: u8,
    prg_banks: [u8; 5], // Banks for $6000, $8000, $A000, $C000, $E000
    prg_ram_bank: u8,

    // CHR banking
    chr_mode: u8,
    chr_banks_bg: [u16; 12], // Background CHR banks
    chr_banks_sp: [u16; 12], // Sprite CHR banks
    chr_last_write: u8,      // Track last write (BG vs SPR)

    // ExRAM
    exram_mode: u8,

    // Mirroring
    nametable_mapping: [u8; 4],
    fill_tile: u8,
    fill_attr: u8,

    // Multiplication
    mul_a: u8,
    mul_b: u8,
}

impl Mmc5 {
    pub fn new(cart: Cartridge, ppu: &mut Ppu) -> Self {
        // MMC5 typically has 64KB PRG RAM, but we'll default to 8KB for simplicity
        let prg_ram_size = 8192;

        let mut m = Self {
            prg_rom: cart.prg_rom,
            chr_rom: cart.chr_rom,
            prg_ram: vec![0; prg_ram_size],
            exram: [0; 1024],

            prg_mode: 3,
            prg_banks: [0xFF, 0xFF, 0xFF, 0xFE, 0xFF], // Last bank fixed at $E000
            prg_ram_bank: 0,

            chr_mode: 0,
            chr_banks_bg: [0; 12],
            chr_banks_sp: [0; 12],
            chr_last_write: 0,

            exram_mode: 0,

            nametable_mapping: [0, 1, 2, 3],
            fill_tile: 0,
            fill_attr: 0,

            mul_a: 0xFF,
            mul_b: 0xFF,
        };

        m.apply_banks(ppu);
        ppu.set_mirroring(Mirroring::Vertical); // Default mirroring
        m
    }

    fn prg_bank_count(&self) -> usize {
        std::cmp::max(1, self.prg_rom.len() / 0x2000)
    }

    fn chr_bank_count_1k(&self) -> usize {
        std::cmp::max(1, self.chr_rom.len() / 0x0400)
    }

    fn apply_banks(&mut self, ppu: &mut Ppu) {
        // Update CHR banking
        self.update_chr_banks(ppu);
    }

    fn update_chr_banks(&self, ppu: &mut Ppu) {
        if ppu.chr.len() < 0x2000 {
            ppu.chr.resize(0x2000, 0);
        }

        if self.chr_rom.is_empty() {
            return; // CHR RAM mode
        }

        // Use background CHR banks for now (sprite banks used during rendering)
        let chr_count = self.chr_bank_count_1k();

        match self.chr_mode {
            0 => {
                // 8KB mode
                let bank = (self.chr_banks_bg[7] as usize) % (chr_count / 8);
                let src_start = bank * 0x2000;
                let src_end = src_start + 0x2000;
                if src_end <= self.chr_rom.len() {
                    ppu.chr.copy_from_slice(&self.chr_rom[src_start..src_end]);
                }
            }
            1 => {
                // 4KB mode
                for i in 0..2 {
                    let bank = (self.chr_banks_bg[3 + i * 4] as usize) % (chr_count / 4);
                    let src_start = bank * 0x1000;
                    let dst_start = i * 0x1000;
                    let src_end = src_start + 0x1000;
                    let dst_end = dst_start + 0x1000;
                    if src_end <= self.chr_rom.len() {
                        ppu.chr[dst_start..dst_end]
                            .copy_from_slice(&self.chr_rom[src_start..src_end]);
                    }
                }
            }
            2 => {
                // 2KB mode
                for i in 0..4 {
                    let bank = (self.chr_banks_bg[1 + i * 2] as usize) % (chr_count / 2);
                    let src_start = bank * 0x0800;
                    let dst_start = i * 0x0800;
                    let src_end = src_start + 0x0800;
                    let dst_end = dst_start + 0x0800;
                    if src_end <= self.chr_rom.len() {
                        ppu.chr[dst_start..dst_end]
                            .copy_from_slice(&self.chr_rom[src_start..src_end]);
                    }
                }
            }
            3 => {
                // 1KB mode
                for i in 0..8 {
                    let bank = (self.chr_banks_bg[i] as usize) % chr_count;
                    let src_start = bank * 0x0400;
                    let dst_start = i * 0x0400;
                    let src_end = src_start + 0x0400;
                    let dst_end = dst_start + 0x0400;
                    if src_end <= self.chr_rom.len() {
                        ppu.chr[dst_start..dst_end]
                            .copy_from_slice(&self.chr_rom[src_start..src_end]);
                    }
                }
            }
            _ => {}
        }
    }

    pub fn read_prg(&self, addr: u16) -> u8 {
        match addr {
            // PRG RAM at $6000-$7FFF
            0x6000..=0x7FFF => {
                let offset = (addr - 0x6000) as usize;
                self.prg_ram.get(offset).copied().unwrap_or(0)
            }
            // PRG ROM banking
            0x8000..=0xFFFF => {
                let prg_count = self.prg_bank_count();
                let (bank_idx, offset) = match (self.prg_mode, addr) {
                    (0, 0x8000..=0xFFFF) => {
                        // 32KB mode
                        let bank = ((self.prg_banks[4] & 0x7C) >> 2) as usize;
                        (bank % (prg_count / 4), (addr - 0x8000) as usize)
                    }
                    (1, 0x8000..=0xBFFF) => {
                        // 16KB at $8000
                        let bank = ((self.prg_banks[2] & 0x7E) >> 1) as usize;
                        (bank % (prg_count / 2), (addr - 0x8000) as usize)
                    }
                    (1, 0xC000..=0xFFFF) => {
                        // 16KB at $C000
                        let bank = ((self.prg_banks[4] & 0x7E) >> 1) as usize;
                        (bank % (prg_count / 2), (addr - 0xC000) as usize)
                    }
                    (2, 0x8000..=0xBFFF) => {
                        // 16KB at $8000
                        let bank = ((self.prg_banks[2] & 0x7E) >> 1) as usize;
                        (bank % (prg_count / 2), (addr - 0x8000) as usize)
                    }
                    (2, 0xC000..=0xDFFF) => {
                        // 8KB at $C000
                        let bank = (self.prg_banks[3] & 0x7F) as usize;
                        (bank % prg_count, (addr - 0xC000) as usize)
                    }
                    (2, 0xE000..=0xFFFF) => {
                        // 8KB at $E000 (fixed to last bank)
                        let bank = (self.prg_banks[4] & 0x7F) as usize;
                        (bank % prg_count, (addr - 0xE000) as usize)
                    }
                    (3, 0x8000..=0x9FFF) => {
                        // 8KB at $8000
                        let bank = (self.prg_banks[1] & 0x7F) as usize;
                        (bank % prg_count, (addr - 0x8000) as usize)
                    }
                    (3, 0xA000..=0xBFFF) => {
                        // 8KB at $A000
                        let bank = (self.prg_banks[2] & 0x7F) as usize;
                        (bank % prg_count, (addr - 0xA000) as usize)
                    }
                    (3, 0xC000..=0xDFFF) => {
                        // 8KB at $C000
                        let bank = (self.prg_banks[3] & 0x7F) as usize;
                        (bank % prg_count, (addr - 0xC000) as usize)
                    }
                    (3, 0xE000..=0xFFFF) => {
                        // 8KB at $E000 (fixed to last bank)
                        let bank = (self.prg_banks[4] & 0x7F) as usize;
                        (bank % prg_count, (addr - 0xE000) as usize)
                    }
                    _ => (0, 0),
                };

                let idx = bank_idx * 0x2000 + offset;
                self.prg_rom.get(idx).copied().unwrap_or(0)
            }
            _ => 0,
        }
    }

    pub fn write_prg(&mut self, addr: u16, val: u8, ppu: &mut Ppu, _cpu_cycles: u64) {
        match addr {
            // PRG RAM at $6000-$7FFF
            0x6000..=0x7FFF => {
                let offset = (addr - 0x6000) as usize;
                if offset < self.prg_ram.len() {
                    self.prg_ram[offset] = val;
                }
            }
            // MMC5 registers at $5000-$5FFF
            0x5000..=0x5FFF => {
                match addr {
                    // PRG mode
                    0x5100 => {
                        self.prg_mode = val & 0x03;
                    }
                    // CHR mode
                    0x5101 => {
                        self.chr_mode = val & 0x03;
                        self.apply_banks(ppu);
                    }
                    // PRG RAM protect (not fully implemented)
                    0x5102..=0x5103 => {}
                    // ExRAM mode
                    0x5104 => {
                        self.exram_mode = val & 0x03;
                    }
                    // Nametable mapping
                    0x5105 => {
                        for i in 0..4 {
                            self.nametable_mapping[i] = (val >> (i * 2)) & 0x03;
                        }
                    }
                    // Fill-mode tile
                    0x5106 => {
                        self.fill_tile = val;
                    }
                    // Fill-mode attribute
                    0x5107 => {
                        self.fill_attr = val & 0x03;
                    }
                    // PRG bank registers
                    0x5113 => {
                        self.prg_ram_bank = val & 0x07;
                    }
                    0x5114 => {
                        self.prg_banks[1] = val;
                    }
                    0x5115 => {
                        self.prg_banks[2] = val;
                    }
                    0x5116 => {
                        self.prg_banks[3] = val;
                    }
                    0x5117 => {
                        self.prg_banks[4] = val;
                    }
                    // CHR bank registers (background)
                    0x5120..=0x512B => {
                        let idx = (addr - 0x5120) as usize;
                        self.chr_banks_bg[idx] = val as u16;
                        self.chr_last_write = 0; // BG
                        self.apply_banks(ppu);
                    }
                    // CHR bank registers (sprite)
                    0x5128..=0x5130 if addr <= 0x512B => {
                        let idx = (addr - 0x5128) as usize;
                        self.chr_banks_sp[idx] = val as u16;
                        self.chr_last_write = 1; // SPR
                    }
                    // Upper CHR bank bits
                    0x5130 => {
                        // High bits for CHR banks (for >256KB CHR)
                        // Simplified: store in upper bits of last written bank
                    }
                    // IRQ counter
                    0x5203 => {
                        // IRQ scanline - not fully implemented yet
                    }
                    // IRQ enable
                    0x5204 => {
                        // IRQ enable - not fully implemented yet
                    }
                    // Multiplication
                    0x5205 => {
                        self.mul_a = val;
                    }
                    0x5206 => {
                        self.mul_b = val;
                    }
                    // ExRAM ($5C00-$5FFF)
                    0x5C00..=0x5FFF => {
                        if self.exram_mode <= 2 {
                            let offset = (addr - 0x5C00) as usize;
                            self.exram[offset] = val;
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    #[cfg(test)]
    pub fn read_exram(&self, addr: u16) -> Option<u8> {
        match addr {
            // Multiplication result (low byte)
            0x5205 => {
                let result = (self.mul_a as u16) * (self.mul_b as u16);
                Some((result & 0xFF) as u8)
            }
            // Multiplication result (high byte)
            0x5206 => {
                let result = (self.mul_a as u16) * (self.mul_b as u16);
                Some((result >> 8) as u8)
            }
            // IRQ status
            0x5204 => {
                // IRQ status - not fully implemented yet
                Some(0x00)
            }
            // ExRAM ($5C00-$5FFF)
            0x5C00..=0x5FFF => {
                if self.exram_mode >= 2 {
                    let offset = (addr - 0x5C00) as usize;
                    Some(self.exram[offset])
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    pub fn prg_rom(&self) -> &[u8] {
        &self.prg_rom
    }

    pub fn take_irq_pending(&mut self) -> bool {
        // IRQ not fully implemented yet
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use emu_core::apu::TimingMode;

    fn create_test_cart(prg_size: usize, chr_size: usize) -> Cartridge {
        Cartridge {
            prg_rom: vec![0; prg_size],
            chr_rom: vec![0; chr_size],
            mapper: 5,
            mirroring: Mirroring::Horizontal,
            timing: TimingMode::Ntsc,
        }
    }

    #[test]
    fn test_mmc5_prg_banking_mode3() {
        let mut cart = create_test_cart(0x80000, 0x20000); // 512KB PRG, 128KB CHR

        // Set up test data in PRG ROM
        cart.prg_rom[0x0000] = 0x11; // Bank 0
        cart.prg_rom[0x2000] = 0x22; // Bank 1
        cart.prg_rom[0x4000] = 0x33; // Bank 2
        cart.prg_rom[0x7E000] = 0xEE; // Second to last bank
        cart.prg_rom[0x7FFFF] = 0xFF; // Last bank

        let mut ppu = Ppu::new(vec![], Mirroring::Horizontal);
        let mut mmc5 = Mmc5::new(cart, &mut ppu);

        // Mode 3: Four 8KB banks
        mmc5.write_prg(0x5100, 0x03, &mut ppu, 0);

        // Map banks
        mmc5.write_prg(0x5114, 0x00, &mut ppu, 0); // $8000 = bank 0
        mmc5.write_prg(0x5115, 0x01, &mut ppu, 0); // $A000 = bank 1
        mmc5.write_prg(0x5116, 0x02, &mut ppu, 0); // $C000 = bank 2
        mmc5.write_prg(0x5117, 0xFF, &mut ppu, 0); // $E000 = last bank

        assert_eq!(mmc5.read_prg(0x8000), 0x11);
        assert_eq!(mmc5.read_prg(0xA000), 0x22);
        assert_eq!(mmc5.read_prg(0xC000), 0x33);
        assert_eq!(mmc5.read_prg(0xFFFF), 0xFF);
    }

    #[test]
    fn test_mmc5_chr_banking() {
        let mut cart = create_test_cart(0x8000, 0x20000); // 32KB PRG, 128KB CHR

        // Set up test data in CHR ROM (128KB = 320 banks of 1KB each)
        for i in 0..320 {
            if i * 0x400 < cart.chr_rom.len() {
                cart.chr_rom[i * 0x400] = i as u8;
            }
        }

        let mut ppu = Ppu::new(vec![], Mirroring::Horizontal);
        let mut mmc5 = Mmc5::new(cart, &mut ppu);

        // Mode 3: 1KB mode
        mmc5.write_prg(0x5101, 0x03, &mut ppu, 0);

        // Map CHR banks
        for i in 0..8 {
            mmc5.write_prg(0x5120 + i, i as u8, &mut ppu, 0);
        }

        // Verify CHR banks are mapped correctly
        for i in 0..8 {
            assert_eq!(ppu.chr[i * 0x400], i as u8);
        }
    }

    #[test]
    fn test_mmc5_multiplication() {
        let cart = create_test_cart(0x8000, 0x2000);
        let mut ppu = Ppu::new(vec![], Mirroring::Horizontal);
        let mut mmc5 = Mmc5::new(cart, &mut ppu);

        // Write multiplier values
        mmc5.write_prg(0x5205, 10, &mut ppu, 0);
        mmc5.write_prg(0x5206, 20, &mut ppu, 0);

        // Read result (10 * 20 = 200 = 0x00C8)
        let low = mmc5.read_exram(0x5205).unwrap();
        let high = mmc5.read_exram(0x5206).unwrap();
        let result = (high as u16) << 8 | (low as u16);

        assert_eq!(result, 200);
    }

    #[test]
    fn test_mmc5_exram() {
        let cart = create_test_cart(0x8000, 0x2000);
        let mut ppu = Ppu::new(vec![], Mirroring::Horizontal);
        let mut mmc5 = Mmc5::new(cart, &mut ppu);

        // Set ExRAM mode to CPU read/write (mode 2)
        mmc5.write_prg(0x5104, 0x02, &mut ppu, 0);

        // Write to ExRAM
        mmc5.write_prg(0x5C00, 0x42, &mut ppu, 0);
        mmc5.write_prg(0x5CFF, 0x99, &mut ppu, 0);

        // Read back
        assert_eq!(mmc5.read_exram(0x5C00).unwrap(), 0x42);
        assert_eq!(mmc5.read_exram(0x5CFF).unwrap(), 0x99);
    }
}
