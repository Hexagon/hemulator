//! Game Boy memory bus implementation
//!
//! The memory bus connects all components of the Game Boy system and handles
//! address decoding and routing. It implements the complete Game Boy memory map
//! and provides access to ROM, RAM, VRAM, OAM, and I/O registers.
//!
//! # Memory Map
//!
//! ```text
//! $0000-$00FF  Boot ROM (disabled after boot via $FF50)
//! $0000-$3FFF  ROM Bank 0 (16KB, fixed)
//! $4000-$7FFF  ROM Bank 1-N (16KB, switchable via MBC)
//! $8000-$9FFF  Video RAM (8KB)
//! $A000-$BFFF  External RAM (8KB, switchable via MBC, battery-backed)
//! $C000-$CFFF  Work RAM Bank 0 (4KB)
//! $D000-$DFFF  Work RAM Bank 1 (4KB) [CGB: Banks 1-7 switchable]
//! $E000-$FDFF  Echo RAM (mirror of $C000-$DDFF)
//! $FE00-$FE9F  OAM - Object Attribute Memory (160 bytes, 40 sprites × 4 bytes)
//! $FEA0-$FEFF  Not usable
//! $FF00-$FF7F  I/O Registers
//! $FF80-$FFFE  High RAM (127 bytes, fast RAM)
//! $FFFF        Interrupt Enable Register
//! ```
//!
//! # I/O Registers
//!
//! ## Joypad
//! - `$FF00 (P1)`: Joypad register
//!   - Bit 5: Select button keys (0=select)
//!   - Bit 4: Select direction keys (0=select)
//!   - Bits 3-0: Input (0=pressed, 1=released)
//!
//! ## Serial Transfer
//! - `$FF01 (SB)`: Serial transfer data
//! - `$FF02 (SC)`: Serial transfer control
//!
//! ## Timer
//! - `$FF04 (DIV)`: Divider register
//! - `$FF05 (TIMA)`: Timer counter
//! - `$FF06 (TMA)`: Timer modulo
//! - `$FF07 (TAC)`: Timer control
//!
//! ## Interrupts
//! - `$FF0F (IF)`: Interrupt flag
//! - `$FFFF (IE)`: Interrupt enable
//!
//! ## PPU Registers
//! - `$FF40 (LCDC)`: LCD control
//! - `$FF41 (STAT)`: LCD status
//! - `$FF42 (SCY)`: Scroll Y
//! - `$FF43 (SCX)`: Scroll X
//! - `$FF44 (LY)`: LCD Y coordinate (read-only)
//! - `$FF45 (LYC)`: LY compare
//! - `$FF47 (BGP)`: Background palette
//! - `$FF48 (OBP0)`: Object palette 0
//! - `$FF49 (OBP1)`: Object palette 1
//! - `$FF4A (WY)`: Window Y position
//! - `$FF4B (WX)`: Window X position
//!
//! ## Other
//! - `$FF50`: Boot ROM disable (write 1 to disable)
//!
//! # MBC (Memory Bank Controllers)
//!
//! MBCs allow games to use more than 32KB of ROM by bank switching.
//! Writes to ROM address space trigger MBC commands.
//!
//! ## Implemented
//! - MBC0: No mapper (32KB ROM max)
//! - MBC1: Most common (up to 2MB ROM, 32KB RAM)
//! - MBC3: With RTC support (up to 2MB ROM, 32KB RAM)
//! - MBC5: For larger ROMs (up to 8MB ROM, 128KB RAM)
//!
//! # Current Implementation
//!
//! ## Implemented
//! - ✅ Full memory map with proper mirroring
//! - ✅ VRAM access via PPU (8KB)
//! - ✅ OAM access via PPU (160 bytes)
//! - ✅ Work RAM (8KB)
//! - ✅ High RAM (127 bytes)
//! - ✅ Joypad register with matrix selection
//! - ✅ PPU registers (LCDC, STAT, palettes, scroll, etc.)
//! - ✅ APU registers (sound channels, master controls, wave RAM)
//! - ✅ Timer registers (DIV, TIMA, TMA, TAC)
//! - ✅ Interrupt registers (IF, IE)
//! - ✅ Boot ROM disable register
//! - ✅ Cartridge ROM loading (up to size)
//! - ✅ Cartridge RAM with size detection
//! - ✅ MBC0, MBC1, MBC3, MBC5 mappers
//!
//! ## Not Implemented
//! - ❌ MBC2 mapper (built-in 512×4 bits RAM)
//! - ❌ Serial transfer
//! - ❌ DMA register
//! - ❌ CGB-specific registers

use crate::apu::GbApu;
use crate::mappers::Mapper;
use crate::ppu::Ppu;
use crate::timer::Timer;
use emu_core::cpu_lr35902::MemoryLr35902;

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
    /// Cartridge mapper (handles ROM/RAM banking)
    mapper: Option<Mapper>,
    /// Boot ROM enabled flag
    boot_rom_enabled: bool,
    /// PPU (Picture Processing Unit)
    pub ppu: Ppu,
    /// APU (Audio Processing Unit)
    pub apu: GbApu,
    /// Timer
    pub timer: Timer,
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
            mapper: None,
            boot_rom_enabled: true,
            ppu: Ppu::new(),
            apu: GbApu::new(),
            timer: Timer::new(),
            joypad: 0xFF,
            button_state: 0xFF,
        }
    }

    /// Set joypad button state
    /// Bits: 0=Right, 1=Left, 2=Up, 3=Down, 4=A, 5=B, 6=Select, 7=Start
    pub fn set_buttons(&mut self, state: u8) {
        self.button_state = state;
    }

    /// Request an interrupt
    /// Bit 0: VBlank
    /// Bit 1: LCD STAT
    /// Bit 2: Timer
    /// Bit 3: Serial
    /// Bit 4: Joypad
    pub fn request_interrupt(&mut self, interrupt_bit: u8) {
        self.if_reg |= interrupt_bit;
    }

    /// Check if any interrupts are pending
    pub fn has_pending_interrupts(&self) -> bool {
        self.if_reg != 0
    }

    pub fn load_cart(&mut self, data: &[u8]) {
        // Parse cart header
        if data.len() < 0x150 {
            // Too small to be a valid cart, but load it anyway
            self.mapper = Some(Mapper::from_cart(data.to_vec(), vec![], 0x00));
            self.boot_rom_enabled = false;
            return;
        }

        let cart_type = data[0x147];
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

        let ram = if ram_size > 0 {
            vec![0; ram_size]
        } else {
            vec![]
        };

        self.mapper = Some(Mapper::from_cart(data.to_vec(), ram, cart_type));
        self.boot_rom_enabled = false; // Skip boot ROM for now
    }
}

impl MemoryLr35902 for GbBus {
    fn read(&self, addr: u16) -> u8 {
        match addr {
            // ROM Bank 0 and Bank 1-N (switchable)
            0x0000..=0x7FFF => {
                if addr < 0x0100 && self.boot_rom_enabled {
                    // Boot ROM would go here
                    0xFF
                } else if let Some(mapper) = &self.mapper {
                    mapper.read_rom(addr)
                } else {
                    0xFF
                }
            }
            // VRAM (8KB) - delegate to PPU
            0x8000..=0x9FFF => self.ppu.read_vram(addr - 0x8000),
            // External RAM (switchable)
            0xA000..=0xBFFF => {
                if let Some(mapper) = &self.mapper {
                    mapper.read_ram(addr)
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
                // Timer registers
                0xFF04..=0xFF07 => self.timer.read_register(addr),
                0xFF0F => self.if_reg,
                // APU registers
                0xFF10..=0xFF26 => self.apu.read_register(addr),
                0xFF30..=0xFF3F => self.apu.read_register(addr),
                // PPU registers
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
                if let Some(mapper) = &mut self.mapper {
                    mapper.write_rom(addr, val);
                }
            }
            // VRAM - delegate to PPU
            0x8000..=0x9FFF => self.ppu.write_vram(addr - 0x8000, val),
            // External RAM
            0xA000..=0xBFFF => {
                if let Some(mapper) = &mut self.mapper {
                    mapper.write_ram(addr, val);
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
                    // Timer registers
                    0xFF04..=0xFF07 => self.timer.write_register(addr, val),
                    0xFF0F => self.if_reg = val,
                    // APU registers
                    0xFF10..=0xFF26 => self.apu.write_register(addr, val),
                    0xFF30..=0xFF3F => self.apu.write_register(addr, val),
                    // PPU registers
                    0xFF40 => self.ppu.lcdc = val,
                    0xFF41 => self.ppu.stat = val,
                    0xFF42 => self.ppu.scy = val,
                    0xFF43 => self.ppu.scx = val,
                    0xFF44 => {} // LY is read-only
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
