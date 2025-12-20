//! Game Boy memory bus implementation

use emu_core::cpu_lr35902::MemoryLr35902;
use crate::ppu::Ppu;

/// Game Boy memory bus
pub struct GbBus {
    /// Work RAM (8KB)
    wram: [u8; 0x2000],
    /// High RAM (127 bytes)
    hram: [u8; 0x7F],
    /// Interrupt Enable register
    ie: u8,
    /// Interrupt Flag register
    if_reg: u8,
    /// Cart ROM (if loaded)
    cart_rom: Vec<u8>,
    /// Cart RAM (if present)
    cart_ram: Vec<u8>,
    /// Boot ROM enabled flag
    boot_rom_enabled: bool,
    /// PPU (Picture Processing Unit)
    pub ppu: Ppu,
    /// Joypad state register (0xFF00)
    joypad: u8,
    /// Joypad button state
    button_state: u8,
}

impl GbBus {
    pub fn new() -> Self {
        Self {
            wram: [0; 0x2000],
            hram: [0; 0x7F],
            ie: 0,
            if_reg: 0,
            cart_rom: Vec::new(),
            cart_ram: Vec::new(),
            boot_rom_enabled: true,
            ppu: Ppu::new(),
            joypad: 0xFF,
            button_state: 0xFF,
        }
    }

    /// Set joypad button state
    /// Bits: 0=Right, 1=Left, 2=Up, 3=Down, 4=A, 5=B, 6=Select, 7=Start
    pub fn set_buttons(&mut self, state: u8) {
        self.button_state = state;
    }

    pub fn load_cart(&mut self, data: &[u8]) {
        self.cart_rom = data.to_vec();
        // Parse cart header for RAM size
        if data.len() >= 0x150 {
            let ram_size_code = data[0x149];
            let ram_size = match ram_size_code {
                0x00 => 0,
                0x01 => 0, // Unused
                0x02 => 8 * 1024,
                0x03 => 32 * 1024,
                0x04 => 128 * 1024,
                0x05 => 64 * 1024,
                _ => 0,
            };
            if ram_size > 0 {
                self.cart_ram = vec![0; ram_size];
            }
        }
        self.boot_rom_enabled = false; // Skip boot ROM for now
    }
}

impl MemoryLr35902 for GbBus {
    fn read(&self, addr: u16) -> u8 {
        match addr {
            // ROM Bank 0
            0x0000..=0x3FFF => {
                if addr < 0x0100 && self.boot_rom_enabled {
                    // Boot ROM would go here
                    0xFF
                } else if (addr as usize) < self.cart_rom.len() {
                    self.cart_rom[addr as usize]
                } else {
                    0xFF
                }
            }
            // ROM Bank 1-N (switchable)
            0x4000..=0x7FFF => {
                if (addr as usize) < self.cart_rom.len() {
                    self.cart_rom[addr as usize]
                } else {
                    0xFF
                }
            }
            // VRAM (8KB) - delegate to PPU
            0x8000..=0x9FFF => self.ppu.read_vram(addr - 0x8000),
            // External RAM (switchable)
            0xA000..=0xBFFF => {
                let offset = (addr - 0xA000) as usize;
                if offset < self.cart_ram.len() {
                    self.cart_ram[offset]
                } else {
                    0xFF
                }
            }
            // Work RAM (8KB)
            0xC000..=0xDFFF => self.wram[(addr - 0xC000) as usize],
            // Echo RAM (mirror of C000-DDFF)
            0xE000..=0xFDFF => self.wram[(addr - 0xE000) as usize],
            // OAM (Object Attribute Memory) - delegate to PPU
            0xFE00..=0xFE9F => self.ppu.read_oam(addr - 0xFE00),
            // Not usable
            0xFEA0..=0xFEFF => 0xFF,
            // I/O Registers
            0xFF00..=0xFF7F => match addr {
                0xFF00 => {
                    // Joypad register
                    // Return button state based on selected mode
                    let select_buttons = (self.joypad & 0x20) == 0;
                    let select_dpad = (self.joypad & 0x10) == 0;
                    
                    let mut result = self.joypad & 0xF0;
                    if select_buttons {
                        result |= (self.button_state >> 4) & 0x0F;
                    } else if select_dpad {
                        result |= self.button_state & 0x0F;
                    } else {
                        result |= 0x0F;
                    }
                    result
                }
                0xFF0F => self.if_reg,
                0xFF40 => self.ppu.lcdc,
                0xFF41 => self.ppu.stat,
                0xFF42 => self.ppu.scy,
                0xFF43 => self.ppu.scx,
                0xFF44 => self.ppu.ly,
                0xFF45 => self.ppu.lyc,
                0xFF47 => self.ppu.bgp,
                0xFF48 => self.ppu.obp0,
                0xFF49 => self.ppu.obp1,
                0xFF4A => self.ppu.wy,
                0xFF4B => self.ppu.wx,
                _ => 0xFF,
            },
            // High RAM
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize],
            // Interrupt Enable
            0xFFFF => self.ie,
        }
    }

    fn write(&mut self, addr: u16, val: u8) {
        match addr {
            // ROM (read-only, but may trigger MBC commands)
            0x0000..=0x7FFF => {
                // MBC commands would go here
            }
            // VRAM - delegate to PPU
            0x8000..=0x9FFF => self.ppu.write_vram(addr - 0x8000, val),
            // External RAM
            0xA000..=0xBFFF => {
                let offset = (addr - 0xA000) as usize;
                if offset < self.cart_ram.len() {
                    self.cart_ram[offset] = val;
                }
            }
            // Work RAM
            0xC000..=0xDFFF => self.wram[(addr - 0xC000) as usize] = val,
            // Echo RAM
            0xE000..=0xFDFF => self.wram[(addr - 0xE000) as usize] = val,
            // OAM - delegate to PPU
            0xFE00..=0xFE9F => self.ppu.write_oam(addr - 0xFE00, val),
            // Not usable
            0xFEA0..=0xFEFF => {}
            // I/O Registers
            0xFF00..=0xFF7F => {
                match addr {
                    0xFF00 => self.joypad = val & 0x30, // Only bits 4-5 are writable
                    0xFF0F => self.if_reg = val,
                    0xFF40 => self.ppu.lcdc = val,
                    0xFF41 => self.ppu.stat = val,
                    0xFF42 => self.ppu.scy = val,
                    0xFF43 => self.ppu.scx = val,
                    0xFF44 => {}, // LY is read-only
                    0xFF45 => self.ppu.lyc = val,
                    0xFF47 => self.ppu.bgp = val,
                    0xFF48 => self.ppu.obp0 = val,
                    0xFF49 => self.ppu.obp1 = val,
                    0xFF4A => self.ppu.wy = val,
                    0xFF4B => self.ppu.wx = val,
                    0xFF50 => self.boot_rom_enabled = false, // Disable boot ROM
                    _ => {}
                }
            }
            // High RAM
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize] = val,
            // Interrupt Enable
            0xFFFF => self.ie = val,
        }
    }
}
