//! SNES memory bus implementation

use crate::cartridge::Cartridge;
use crate::ppu::Ppu;
use crate::SnesError;
use emu_core::cpu_65c816::Memory65c816;

/// SNES memory bus
pub struct SnesBus {
    /// 128KB WRAM (work RAM)
    wram: [u8; 0x20000],
    /// Cartridge (optional)
    cartridge: Option<Cartridge>,
    /// PPU (Picture Processing Unit)
    ppu: Ppu,
    /// Frame counter for VBlank emulation
    frame_counter: u64,
}

impl SnesBus {
    pub fn new() -> Self {
        Self {
            wram: [0; 0x20000],
            cartridge: None,
            ppu: Ppu::new(),
            frame_counter: 0,
        }
    }

    pub fn load_cartridge(&mut self, data: &[u8]) -> Result<(), SnesError> {
        self.cartridge = Some(Cartridge::load(data)?);
        Ok(())
    }

    pub fn unload_cartridge(&mut self) {
        self.cartridge = None;
    }

    pub fn has_cartridge(&self) -> bool {
        self.cartridge.is_some()
    }

    pub fn ppu(&self) -> &Ppu {
        &self.ppu
    }

    pub fn tick_frame(&mut self) {
        self.frame_counter += 1;
    }

    pub fn get_rom_size(&self) -> usize {
        if let Some(ref cart) = self.cartridge {
            cart.rom_size()
        } else {
            0
        }
    }

    pub fn has_smc_header(&self) -> bool {
        if let Some(ref cart) = self.cartridge {
            cart.has_smc_header()
        } else {
            false
        }
    }
}

impl Default for SnesBus {
    fn default() -> Self {
        Self::new()
    }
}

impl Memory65c816 for SnesBus {
    fn read(&self, addr: u32) -> u8 {
        let bank = (addr >> 16) as u8;
        let offset = (addr & 0xFFFF) as u16;

        match bank {
            // Banks $00-$3F and $80-$BF: System area
            0x00..=0x3F | 0x80..=0xBF => {
                match offset {
                    // WRAM (shadow at $0000-$1FFF)
                    0x0000..=0x1FFF => self.wram[offset as usize],
                    // Hardware registers (PPU: $2100-$213F)
                    0x2100..=0x213F => self.ppu.read_register(offset),
                    // $4212 - HVBJOY - H/V Blank and Joypad Status
                    0x4212 => {
                        // Bit 7: VBlank flag (set during VBlank)
                        // Bit 6: HBlank flag
                        // For simplicity, alternate VBlank every ~30 frames (simulate ~60Hz at ~2Hz toggle)
                        if (self.frame_counter / 30).is_multiple_of(2) {
                            0x80 // VBlank
                        } else {
                            0x00 // Not in VBlank
                        }
                    }
                    // Other hardware registers
                    0x2000..=0x5FFF => 0, // Stub
                    // WRAM (full at $6000-$7FFF in banks $00-$3F)
                    0x6000..=0x7FFF if bank < 0x40 => self.wram[(offset - 0x6000) as usize],
                    // Cartridge ROM
                    0x8000..=0xFFFF => {
                        if let Some(ref cart) = self.cartridge {
                            cart.read(addr)
                        } else {
                            0
                        }
                    }
                    _ => 0,
                }
            }
            // Banks $7E-$7F: Full WRAM mirror
            0x7E..=0x7F => {
                let wram_addr = ((bank as usize - 0x7E) << 16) | offset as usize;
                self.wram[wram_addr]
            }
            // Banks $40-$6F and $C0-$FF: Cartridge ROM
            _ => {
                if let Some(ref cart) = self.cartridge {
                    cart.read(addr)
                } else {
                    0
                }
            }
        }
    }

    fn write(&mut self, addr: u32, val: u8) {
        let bank = (addr >> 16) as u8;
        let offset = (addr & 0xFFFF) as u16;

        match bank {
            // Banks $00-$3F and $80-$BF: System area
            0x00..=0x3F | 0x80..=0xBF => {
                match offset {
                    // WRAM (shadow at $0000-$1FFF)
                    0x0000..=0x1FFF => self.wram[offset as usize] = val,
                    // Hardware registers (PPU: $2100-$213F)
                    0x2100..=0x213F => self.ppu.write_register(offset, val),
                    // Other hardware registers
                    0x2000..=0x5FFF => {} // Stub - ignore writes
                    // WRAM (full at $6000-$7FFF in banks $00-$3F)
                    0x6000..=0x7FFF if bank < 0x40 => {
                        self.wram[(offset - 0x6000) as usize] = val;
                    }
                    // Cartridge ROM/RAM
                    0x8000..=0xFFFF => {
                        if let Some(ref mut cart) = self.cartridge {
                            cart.write(addr, val);
                        }
                    }
                    _ => {}
                }
            }
            // Banks $7E-$7F: Full WRAM mirror
            0x7E..=0x7F => {
                let wram_addr = ((bank as usize - 0x7E) << 16) | offset as usize;
                self.wram[wram_addr] = val;
            }
            // Banks $40-$6F and $C0-$FF: Cartridge ROM/RAM
            _ => {
                if let Some(ref mut cart) = self.cartridge {
                    cart.write(addr, val);
                }
            }
        }
    }
}
