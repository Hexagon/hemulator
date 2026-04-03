//! Game Boy Advance (GBA) system emulator.
//!
//! The GBA uses an ARM7TDMI CPU running at 16.78 MHz with:
//! - 32KB internal WRAM (on-chip, fast)
//! - 256KB external WRAM (on-board, slower)
//! - 1KB palette RAM
//! - 96KB VRAM
//! - 1KB OAM (Object Attribute Memory)
//! - Up to 32MB cartridge ROM
//! - 64KB cartridge SRAM (battery-backed save)
//!
//! ## Memory Map
//!
//! | Address Range       | Size   | Description                    |
//! |---------------------|--------|--------------------------------|
//! | 0x00000000-0x00003FFF | 16KB  | BIOS ROM                      |
//! | 0x02000000-0x0203FFFF | 256KB | External WRAM (on-board)       |
//! | 0x03000000-0x03007FFF | 32KB  | Internal WRAM (on-chip)        |
//! | 0x04000000-0x040003FE | 1KB   | I/O Registers                  |
//! | 0x05000000-0x050003FF | 1KB   | Palette RAM                    |
//! | 0x06000000-0x06017FFF | 96KB  | VRAM                           |
//! | 0x07000000-0x070003FF | 1KB   | OAM                            |
//! | 0x08000000-0x09FFFFFF | 32MB  | Game Pak ROM (Wait State 0)    |
//! | 0x0A000000-0x0BFFFFFF | 32MB  | Game Pak ROM (Wait State 1)    |
//! | 0x0C000000-0x0DFFFFFF | 32MB  | Game Pak ROM (Wait State 2)    |
//! | 0x0E000000-0x0E00FFFF | 64KB  | Game Pak SRAM                  |

use std::cell::RefCell;

use emu_core::cpu_arm7tdmi::{Arm7Tdmi, MemoryArm7};
use emu_core::debug::Debugger;
use emu_core::{types::Frame, MountPointInfo, System};
use serde_json::Value;
use thiserror::Error;

pub mod apu;
pub mod cartridge;
pub mod debugger;
pub mod dma;
pub mod eeprom;
pub mod flash;
pub mod ppu;
pub mod timers;

#[derive(Debug, Error)]
pub enum GbaError {
    #[error("Cartridge not loaded")]
    NoCartridge,
    #[error("Invalid mount point: {0}")]
    InvalidMountPoint(String),
}

// =============================================================================
// GBA Memory Bus
// =============================================================================

/// GBA memory bus implementing the ARM7TDMI memory interface.
///
/// Handles the full GBA memory map with mirroring and wait states.
pub struct GbaBus {
    /// BIOS ROM (16KB) - read-only after boot, protected
    bios: Vec<u8>,
    /// External WRAM (256KB) - on-board, 2 wait states
    ewram: Vec<u8>,
    /// Internal WRAM (32KB) - on-chip, 0 wait states
    iwram: Vec<u8>,
    /// I/O registers (1KB)
    io: Vec<u8>,
    /// Palette RAM (1KB) - BG and OBJ palettes
    palette: Vec<u8>,
    /// Video RAM (96KB)
    vram: Vec<u8>,
    /// Object Attribute Memory (1KB)
    oam: Vec<u8>,
    /// Cartridge ROM (up to 32MB)
    rom: Vec<u8>,
    /// Cartridge SRAM (up to 64KB)
    sram: Vec<u8>,
    /// DMA controller (4 channels)
    dma: dma::Dma,
    /// Hardware timers (4 × 16-bit)
    timers: timers::Timers,
    /// EEPROM save storage (for games that use EEPROM)
    /// Wrapped in RefCell because EEPROM reads advance internal state
    /// but MemoryArm7::read_byte takes &self.
    eeprom: RefCell<eeprom::Eeprom>,
    /// Flash save storage (for games that use Flash)
    /// Wrapped in RefCell because Flash reads may update state
    /// but MemoryArm7::read_byte takes &self.
    flash: RefCell<flash::Flash>,
    /// Detected save type from cartridge header
    save_type: cartridge::SaveType,
    /// Whether an IRQ is currently pending (IE & IF & IME)
    irq_pending: bool,
    /// Interrupt Master Enable (0x04000208)
    ime: bool,
    /// Interrupt Enable (0x04000200)
    ie: u16,
    /// Interrupt Request Flags (0x04000202)
    if_flags: u16,
    /// Controller state: GBA KEYINPUT register (active-low)
    /// Bits: 0=A, 1=B, 2=Select, 3=Start, 4=Right, 5=Left, 6=Up, 7=Down, 8=R, 9=L
    /// 0 = pressed, 1 = released. Default: 0x03FF (all released)
    keyinput: u16,
    /// Dirty flags for affine reference point writes.
    /// Bit 0=BG2X, bit 1=BG2Y, bit 2=BG3X, bit 3=BG3Y.
    /// Set when the game writes to these registers; cleared after the PPU latches them.
    pub affine_ref_dirty: u8,
    /// Audio Processing Unit
    pub apu: apu::GbaApu,
    /// HALTCNT halt request - set by bus when game writes HALTCNT, consumed by CPU
    halt_requested: bool,
}

impl std::fmt::Debug for GbaBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GbaBus")
            .field("rom_size", &self.rom.len())
            .field("dma", &self.dma)
            .field("timers", &self.timers)
            .field("ime", &self.ime)
            .field("ie", &self.ie)
            .field("if_flags", &self.if_flags)
            .finish()
    }
}

impl Default for GbaBus {
    fn default() -> Self {
        Self::new()
    }
}

impl GbaBus {
    pub fn new() -> Self {
        // Stub BIOS with essential handlers for HLE operation
        let mut bios = vec![0u8; 0x4000];

        // 0x00: Reset vector - infinite loop (B 0x00)
        Self::write_arm_word(&mut bios, 0x00, 0xEAFF_FFFE);

        // 0x08: SWI vector - just return (safety net for unhandled SWIs)
        // MOVS PC, LR restores CPSR from SPSR and returns
        Self::write_arm_word(&mut bios, 0x08, 0xE1B0_F00E);

        // 0x18: IRQ vector - branch to handler at 0x80
        // (Vectors only have room for one instruction each; FIQ is at 0x1C)
        Self::write_arm_word(&mut bios, 0x18, 0xEA00_0018); // B 0x80

        // 0x80-0x98: IRQ handler matching real GBA BIOS behavior.
        // The real BIOS IRQ handler is very simple: save registers, call
        // the game's ISR at [0x03FFFFFC], restore registers, and return.
        // The game's ISR runs in IRQ mode with interrupts disabled (I=1),
        // and is responsible for reading IE/IF, acknowledging IF, and
        // updating BIOS IF at [0x03007FF8] for IntrWait/VBlankIntrWait.
        //
        // 0x80: STMFD SP!, {R0-R3, R12, LR} — Save registers on IRQ stack
        Self::write_arm_word(&mut bios, 0x80, 0xE92D_500F);
        // 0x84: MOV R0, #0x04000000         — I/O base for address calculation
        Self::write_arm_word(&mut bios, 0x84, 0xE3A0_0301);
        // 0x88: ADD LR, PC, #0              — LR = 0x90 (return address)
        Self::write_arm_word(&mut bios, 0x88, 0xE28F_E000);
        // 0x8C: LDR PC, [R0, #-4]           — Jump to game ISR at [0x03FFFFFC]
        Self::write_arm_word(&mut bios, 0x8C, 0xE510_F004);
        // --- Game ISR returns here (0x90) ---
        // 0x90: LDMFD SP!, {R0-R3, R12, LR} — Restore registers
        Self::write_arm_word(&mut bios, 0x90, 0xE8BD_500F);
        // 0x94: SUBS PC, LR, #4             — Return from IRQ (restores CPSR)
        Self::write_arm_word(&mut bios, 0x94, 0xE25E_F004);

        Self {
            bios,
            ewram: vec![0; 0x40000], // 256KB
            iwram: vec![0; 0x8000],  // 32KB
            io: vec![0; 0x400],      // 1KB
            palette: vec![0; 0x400], // 1KB
            vram: vec![0; 0x18000],  // 96KB
            oam: vec![0; 0x400],     // 1KB
            rom: Vec::new(),
            sram: vec![0xFF; 0x10000], // 64KB - 0xFF = erased/uninitialized (matches real hardware)
            dma: dma::Dma::new(),
            timers: timers::Timers::new(),
            eeprom: RefCell::new(eeprom::Eeprom::new()),
            flash: RefCell::new(flash::Flash::new(false)),
            save_type: cartridge::SaveType::None,
            irq_pending: false,
            ime: false,
            ie: 0,
            if_flags: 0,
            keyinput: 0x03FF, // All buttons released
            affine_ref_dirty: 0,
            apu: apu::GbaApu::new(),
            halt_requested: false,
        }
    }

    /// Write a 32-bit ARM instruction to the BIOS buffer (little-endian)
    fn write_arm_word(bios: &mut [u8], offset: usize, word: u32) {
        bios[offset] = word as u8;
        bios[offset + 1] = (word >> 8) as u8;
        bios[offset + 2] = (word >> 16) as u8;
        bios[offset + 3] = (word >> 24) as u8;
    }

    /// Load cartridge ROM data
    pub fn load_rom(&mut self, data: &[u8]) {
        self.rom = data.to_vec();
    }

    /// Clear cartridge ROM
    pub fn unload_rom(&mut self) {
        self.rom.clear();
    }

    /// Request an interrupt
    pub fn request_interrupt(&mut self, irq_bit: u16) {
        self.if_flags |= irq_bit;
        self.update_irq_line();
    }

    /// Update the aggregate IRQ pending flag
    fn update_irq_line(&mut self) {
        self.irq_pending = self.ime && (self.ie & self.if_flags) != 0;
    }

    /// Read an I/O register
    fn io_read(&self, addr: u32) -> u8 {
        let offset = (addr - 0x04000000) as usize;

        match addr {
            // DISPCNT (Display Control)
            0x04000000..=0x04000001 => self.io.get(offset).copied().unwrap_or(0),

            // DISPSTAT (Display Status) - bits 0-2 are PPU state flags
            0x04000004..=0x04000005 => self.io.get(offset).copied().unwrap_or(0),

            // VCOUNT (Vertical Counter) - current scanline
            0x04000006 => self.io.get(offset).copied().unwrap_or(0),
            0x04000007 => 0, // VCOUNT is only 8 bits

            // Sound registers (0x060-0x0A7) - includes SOUNDBIAS at 0x088
            0x04000060..=0x040000A7 => self.apu.read_register(addr),

            // DMA registers (0x040000B0-0x040000DF)
            0x040000B0..=0x040000DF => self.dma.read(addr - 0x040000B0),

            // Timer registers (0x04000100-0x0400010F)
            0x04000100..=0x0400010F => self.timers.read(addr - 0x04000100),

            // KEYINPUT (Key Status) - active-low button state
            0x04000130 => self.keyinput as u8,
            0x04000131 => (self.keyinput >> 8) as u8,

            // IE (Interrupt Enable)
            0x04000200 => self.ie as u8,
            0x04000201 => (self.ie >> 8) as u8,

            // IF (Interrupt Request Flags)
            0x04000202 => self.if_flags as u8,
            0x04000203 => (self.if_flags >> 8) as u8,

            // IME (Interrupt Master Enable)
            0x04000208 => self.ime as u8,
            0x04000209 => 0,

            // Other I/O registers
            _ => self.io.get(offset).copied().unwrap_or(0),
        }
    }

    /// Write an I/O register
    fn io_write(&mut self, addr: u32, val: u8) {
        let offset = (addr - 0x04000000) as usize;

        match addr {
            // IE (Interrupt Enable)
            0x04000200 => {
                self.ie = (self.ie & 0xFF00) | val as u16;
                self.update_irq_line();
            }
            0x04000201 => {
                self.ie = (self.ie & 0x00FF) | ((val as u16) << 8);
                self.update_irq_line();
            }

            // IF (Interrupt Flags) - writing 1 acknowledges/clears the flag
            0x04000202 => {
                self.if_flags &= !(val as u16);
                self.update_irq_line();
            }
            0x04000203 => {
                self.if_flags &= !((val as u16) << 8);
                self.update_irq_line();
            }

            // IME (Interrupt Master Enable)
            0x04000208 => {
                self.ime = val & 1 != 0;
                self.update_irq_line();
            }

            // DMA registers (0x040000B0-0x040000DF)
            0x040000B0..=0x040000DF => {
                self.dma.write(addr - 0x040000B0, val);
            }

            // Timer registers (0x04000100-0x0400010F)
            0x04000100..=0x0400010F => {
                self.timers.write(addr - 0x04000100, val);
            }

            // Sound registers (0x060-0x0A7)
            0x04000060..=0x040000A7 => {
                self.apu.write_register(addr, val);
                // Also store in I/O array for SOUNDBIAS reads
                if offset < self.io.len() {
                    self.io[offset] = val;
                }
            }

            // HALTCNT (0x04000301) - Halt/Stop
            0x04000301 => {
                if val & 0x80 == 0 {
                    // Bit 7 = 0: Halt mode (wait for interrupt)
                    self.halt_requested = true;
                }
                // Bit 7 = 1: Stop mode (deep sleep) — treat as halt for emulation
            }

            // Affine reference point registers - writing immediately updates
            // the PPU's internal reference point (they are write-only latches).
            0x04000028..=0x0400002B => {
                // BG2X
                if offset < self.io.len() {
                    self.io[offset] = val;
                }
                self.affine_ref_dirty |= 1;
            }
            0x0400002C..=0x0400002F => {
                // BG2Y
                if offset < self.io.len() {
                    self.io[offset] = val;
                }
                self.affine_ref_dirty |= 2;
            }
            0x04000038..=0x0400003B => {
                // BG3X
                if offset < self.io.len() {
                    self.io[offset] = val;
                }
                self.affine_ref_dirty |= 4;
            }
            0x0400003C..=0x0400003F => {
                // BG3Y
                if offset < self.io.len() {
                    self.io[offset] = val;
                }
                self.affine_ref_dirty |= 8;
            }

            // Other I/O registers
            _ => {
                if offset < self.io.len() {
                    self.io[offset] = val;
                }
            }
        }
    }
}

impl MemoryArm7 for GbaBus {
    fn read_byte(&self, addr: u32) -> u8 {
        match addr {
            // BIOS (0x00000000 - 0x00003FFF)
            0x00000000..=0x00003FFF => self.bios.get(addr as usize).copied().unwrap_or(0),

            // External WRAM (0x02000000 - 0x0203FFFF, mirrored)
            0x02000000..=0x02FFFFFF => {
                let offset = (addr & 0x3FFFF) as usize;
                self.ewram[offset]
            }

            // Internal WRAM (0x03000000 - 0x03007FFF, mirrored)
            0x03000000..=0x03FFFFFF => {
                let offset = (addr & 0x7FFF) as usize;
                self.iwram[offset]
            }

            // I/O Registers (0x04000000 - 0x040003FE)
            0x04000000..=0x04FFFFFF => self.io_read(addr),

            // Palette RAM (0x05000000 - 0x050003FF, mirrored)
            0x05000000..=0x05FFFFFF => {
                let offset = (addr & 0x3FF) as usize;
                self.palette[offset]
            }

            // VRAM (0x06000000 - 0x06017FFF, mirrored)
            0x06000000..=0x06FFFFFF => {
                let offset = (addr & 0x1FFFF) as usize;
                // VRAM is 96KB, addresses 0x18000-0x1FFFF mirror 0x10000-0x17FFF
                let offset = if offset >= 0x18000 {
                    offset - 0x8000
                } else {
                    offset
                };
                self.vram.get(offset).copied().unwrap_or(0)
            }

            // OAM (0x07000000 - 0x070003FF, mirrored)
            0x07000000..=0x07FFFFFF => {
                let offset = (addr & 0x3FF) as usize;
                self.oam[offset]
            }

            // Cartridge ROM (0x08000000 - 0x0DFFFFFF, 3 wait state regions)
            0x08000000..=0x0DFFFFFF => {
                // Check for EEPROM access
                if self.save_type == cartridge::SaveType::Eeprom
                    && eeprom::Eeprom::is_eeprom_address(addr, self.rom.len())
                {
                    return self.eeprom.borrow_mut().read_bit() as u8;
                }
                let offset = (addr & 0x01FFFFFF) as usize;
                if offset < self.rom.len() {
                    self.rom[offset]
                } else {
                    // Open-bus: GBA returns (addr/2) as halfword for out-of-bounds ROM
                    let halfword = (addr >> 1) as u16;
                    if addr & 1 == 0 {
                        halfword as u8
                    } else {
                        (halfword >> 8) as u8
                    }
                }
            }

            // Cartridge SRAM / Flash (0x0E000000 - 0x0E00FFFF)
            0x0E000000..=0x0EFFFFFF => match self.save_type {
                cartridge::SaveType::Flash64K | cartridge::SaveType::Flash128K => {
                    self.flash.borrow().read((addr & 0xFFFF) as u16)
                }
                _ => {
                    let offset = (addr & 0xFFFF) as usize;
                    self.sram.get(offset).copied().unwrap_or(0)
                }
            },

            // Unused / open bus
            _ => 0,
        }
    }

    fn read_halfword(&self, addr: u32) -> u16 {
        let addr = addr & !1; // Force alignment

        // EEPROM reads: return single bit in bit 0, don't call read_byte twice
        if self.save_type == cartridge::SaveType::Eeprom
            && eeprom::Eeprom::is_eeprom_address(addr, self.rom.len())
        {
            return self.eeprom.borrow_mut().read_bit();
        }

        let lo = self.read_byte(addr) as u16;
        let hi = self.read_byte(addr + 1) as u16;
        lo | (hi << 8)
    }

    fn read_word(&self, addr: u32) -> u32 {
        let addr = addr & !3; // Force alignment
        let b0 = self.read_byte(addr) as u32;
        let b1 = self.read_byte(addr + 1) as u32;
        let b2 = self.read_byte(addr + 2) as u32;
        let b3 = self.read_byte(addr + 3) as u32;
        b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
    }

    fn write_byte(&mut self, addr: u32, val: u8) {
        match addr {
            // BIOS is read-only
            0x00000000..=0x00003FFF => {}

            // External WRAM
            0x02000000..=0x02FFFFFF => {
                let offset = (addr & 0x3FFFF) as usize;
                self.ewram[offset] = val;
            }

            // Internal WRAM
            0x03000000..=0x03FFFFFF => {
                let offset = (addr & 0x7FFF) as usize;
                self.iwram[offset] = val;
            }

            // I/O Registers
            0x04000000..=0x04FFFFFF => self.io_write(addr, val),

            // Palette RAM (byte writes are special: write to both bytes of halfword)
            0x05000000..=0x05FFFFFF => {
                let offset = (addr & 0x3FE) as usize; // Force halfword alignment
                self.palette[offset] = val;
                self.palette[offset + 1] = val;
            }

            // VRAM (byte writes: write to both bytes of halfword, OBJ region ignores)
            0x06000000..=0x06FFFFFF => {
                let offset = (addr & 0x1FFFF) as usize;
                let offset = if offset >= 0x18000 {
                    offset - 0x8000
                } else {
                    offset
                };
                // OBJ VRAM byte writes are ignored.
                // BG/OBJ boundary depends on display mode:
                //   Tile modes (0-2): OBJ VRAM starts at 0x10000
                //   Bitmap modes (3-5): OBJ VRAM starts at 0x14000
                let bg_mode = self.io[0] & 0x07;
                let obj_boundary = if bg_mode >= 3 { 0x14000 } else { 0x10000 };
                if offset < obj_boundary {
                    let aligned = offset & !1;
                    if aligned + 1 < self.vram.len() {
                        self.vram[aligned] = val;
                        self.vram[aligned + 1] = val;
                    }
                }
            }

            // OAM (byte writes are ignored)
            0x07000000..=0x07FFFFFF => {}

            // Cartridge ROM (writes to EEPROM address range)
            0x08000000..=0x0DFFFFFF => {
                if self.save_type == cartridge::SaveType::Eeprom
                    && eeprom::Eeprom::is_eeprom_address(addr, self.rom.len())
                {
                    self.eeprom.borrow_mut().write_bit(val as u16);
                }
            }

            // Cartridge SRAM / Flash
            0x0E000000..=0x0EFFFFFF => match self.save_type {
                cartridge::SaveType::Flash64K | cartridge::SaveType::Flash128K => {
                    self.flash.borrow_mut().write((addr & 0xFFFF) as u16, val);
                }
                _ => {
                    let offset = (addr & 0xFFFF) as usize;
                    if offset < self.sram.len() {
                        self.sram[offset] = val;
                    }
                }
            },

            _ => {}
        }
    }

    fn write_halfword(&mut self, addr: u32, val: u16) {
        let addr = addr & !1;
        match addr {
            // Palette RAM - halfword writes work normally (no byte-doubling)
            0x05000000..=0x05FFFFFF => {
                let offset = (addr & 0x3FF) as usize;
                if offset + 1 < self.palette.len() {
                    self.palette[offset] = val as u8;
                    self.palette[offset + 1] = (val >> 8) as u8;
                }
            }
            // VRAM - halfword writes work normally (no byte-doubling)
            0x06000000..=0x06FFFFFF => {
                let offset = (addr & 0x1FFFF) as usize;
                let offset = if offset >= 0x18000 {
                    offset - 0x8000
                } else {
                    offset
                };
                if offset + 1 < self.vram.len() {
                    self.vram[offset] = val as u8;
                    self.vram[offset + 1] = (val >> 8) as u8;
                }
            }
            // OAM - halfword writes work normally (not ignored like byte writes)
            0x07000000..=0x07FFFFFF => {
                let offset = (addr & 0x3FF) as usize;
                if offset + 1 < self.oam.len() {
                    self.oam[offset] = val as u8;
                    self.oam[offset + 1] = (val >> 8) as u8;
                }
            }
            // Everything else goes through write_byte (safe for non-special regions)
            _ => {
                // EEPROM writes: single bit per transfer, don't split into two write_byte calls
                if self.save_type == cartridge::SaveType::Eeprom
                    && eeprom::Eeprom::is_eeprom_address(addr, self.rom.len())
                {
                    self.eeprom.borrow_mut().write_bit(val);
                    return;
                }
                self.write_byte(addr, val as u8);
                self.write_byte(addr + 1, (val >> 8) as u8);
            }
        }
    }

    fn write_word(&mut self, addr: u32, val: u32) {
        let addr = addr & !3;
        match addr {
            // Palette RAM - word writes work normally
            0x05000000..=0x05FFFFFF => {
                let offset = (addr & 0x3FF) as usize;
                if offset + 3 < self.palette.len() {
                    self.palette[offset] = val as u8;
                    self.palette[offset + 1] = (val >> 8) as u8;
                    self.palette[offset + 2] = (val >> 16) as u8;
                    self.palette[offset + 3] = (val >> 24) as u8;
                }
            }
            // VRAM - word writes work normally
            0x06000000..=0x06FFFFFF => {
                let offset = (addr & 0x1FFFF) as usize;
                let offset = if offset >= 0x18000 {
                    offset - 0x8000
                } else {
                    offset
                };
                if offset + 3 < self.vram.len() {
                    self.vram[offset] = val as u8;
                    self.vram[offset + 1] = (val >> 8) as u8;
                    self.vram[offset + 2] = (val >> 16) as u8;
                    self.vram[offset + 3] = (val >> 24) as u8;
                }
            }
            // OAM - word writes work normally
            0x07000000..=0x07FFFFFF => {
                let offset = (addr & 0x3FF) as usize;
                if offset + 3 < self.oam.len() {
                    self.oam[offset] = val as u8;
                    self.oam[offset + 1] = (val >> 8) as u8;
                    self.oam[offset + 2] = (val >> 16) as u8;
                    self.oam[offset + 3] = (val >> 24) as u8;
                }
            }
            // DMA Sound FIFOs - word writes are the normal DMA path
            0x040000A0 | 0x040000A4 => {
                self.apu.write_fifo_word(addr, val);
            }
            // Everything else goes through write_byte
            _ => {
                self.write_byte(addr, val as u8);
                self.write_byte(addr + 1, (val >> 8) as u8);
                self.write_byte(addr + 2, (val >> 16) as u8);
                self.write_byte(addr + 3, (val >> 24) as u8);
            }
        }
    }

    fn irq_pending(&self) -> bool {
        self.irq_pending
    }

    fn halt_irq_pending(&self) -> bool {
        // HALT wakes when (IE & IF) != 0, regardless of IME
        (self.ie & self.if_flags) != 0
    }

    fn pre_irq_acknowledge(&mut self) {
        // Update BIOS IF at 0x03007FF8 so IntrWait/VBlankIntrWait can detect
        // which interrupts have fired. We do NOT clear hardware IF here because
        // the game's ISR reads IF directly to determine which interrupt fired
        // and acknowledges it itself.
        let acknowledged = self.ie & self.if_flags;
        if acknowledged != 0 {
            // Update BIOS IF at IWRAM offset 0x7FF8 (address 0x03007FF8)
            let bios_if = u16::from_le_bytes([self.iwram[0x7FF8], self.iwram[0x7FF9]]);
            let new_bios_if = bios_if | acknowledged;
            self.iwram[0x7FF8] = new_bios_if as u8;
            self.iwram[0x7FF9] = (new_bios_if >> 8) as u8;
        }
    }

    fn take_halt_request(&mut self) -> bool {
        let req = self.halt_requested;
        self.halt_requested = false;
        req
    }
}

// =============================================================================
// GBA System
// =============================================================================

/// GBA clock speed: 16.78 MHz (2^24 Hz = 16,777,216 Hz)
#[allow(dead_code)] // Used for reference timing calculations
const CPU_FREQ: u64 = 16_777_216;

/// Cycles per scanline: 1232 (280896 cycles/frame ÷ 228 scanlines)
const CYCLES_PER_SCANLINE: u64 = 1232;

/// Cycle within a scanline where HBlank begins (after 1006 cycles of draw)
const HBLANK_START: u64 = 1006;

/// Visible scanlines: 160
const VISIBLE_SCANLINES: u32 = 160;

/// Total scanlines per frame: 228 (160 visible + 68 VBlank)
const TOTAL_SCANLINES: u32 = 228;

/// Cycles per frame: 280896
const CYCLES_PER_FRAME: u64 = CYCLES_PER_SCANLINE * TOTAL_SCANLINES as u64;

// I/O register offsets
const REG_DISPSTAT: usize = 0x004;
const REG_VCOUNT: usize = 0x006;

// DISPSTAT bits
const DISPSTAT_VBLANK: u8 = 1 << 0;
const DISPSTAT_HBLANK: u8 = 1 << 1;
const DISPSTAT_VCOUNT_MATCH: u8 = 1 << 2;
const DISPSTAT_VBLANK_IRQ: u8 = 1 << 3;
const DISPSTAT_HBLANK_IRQ: u8 = 1 << 4;
const DISPSTAT_VCOUNT_IRQ: u8 = 1 << 5;

// IRQ bits
const IRQ_VBLANK: u16 = 1 << 0;
const IRQ_HBLANK: u16 = 1 << 1;
const IRQ_VCOUNT: u16 = 1 << 2;

/// Tile viewer data for debugging GBA PPU graphics.
///
/// Contains VRAM, palette RAM, OAM, and PPU state for visualization in the inspector.
#[derive(Debug, Clone)]
pub struct TileViewerData {
    /// VRAM data - 96KB (tile data and tilemaps)
    pub vram: Vec<u8>,
    /// Palette RAM - 1KB (512 colors: 256 BG + 256 OBJ, each 15-bit BGR)
    pub palette_ram: Vec<u8>,
    /// OAM data - 1KB (128 sprites × 8 bytes)
    pub oam: Vec<u8>,
    /// Converted master palette for display (512 colors as RGBA)
    pub master_palette: Vec<u32>,

    // PPU state registers
    /// DISPCNT - Display Control
    pub dispcnt: u16,
    /// BG0CNT - BG0 Control
    pub bg0cnt: u16,
    /// BG1CNT - BG1 Control
    pub bg1cnt: u16,
    /// BG2CNT - BG2 Control
    pub bg2cnt: u16,
    /// BG3CNT - BG3 Control
    pub bg3cnt: u16,
    /// BG scroll offsets (X and Y for each BG layer)
    pub bg_scroll: [(u16, u16); 4],
    /// BLDCNT - Color Special Effects Selection
    pub bldcnt: u16,
    /// BLDALPHA - Alpha Blending Coefficients
    pub bldalpha: u16,
}

pub struct GbaSystem {
    cpu: Arm7Tdmi<GbaBus>,
    ppu: ppu::Ppu,
    total_cycles: u64,
    scanline: u32,
    scanline_cycles: u64,
    /// Whether HBlank events (IRQ, DMA, flag) have been triggered for the current scanline
    hblank_triggered: bool,
    /// Parsed cartridge header (set on mount)
    header: Option<cartridge::GbaCartridgeHeader>,
    /// Instruction tracer for debugging
    instruction_tracer: emu_core::instruction_tracer::InstructionTracer,
}

impl std::fmt::Debug for GbaSystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GbaSystem")
            .field("total_cycles", &self.total_cycles)
            .field("scanline", &self.scanline)
            .field("ppu", &self.ppu)
            .field("header", &self.header)
            .finish()
    }
}

impl Default for GbaSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl GbaSystem {
    pub fn new() -> Self {
        let bus = GbaBus::new();
        let cpu = Arm7Tdmi::new(bus);

        Self {
            cpu,
            ppu: ppu::Ppu::new(),
            total_cycles: 0,
            scanline: 0,
            scanline_cycles: 0,
            hblank_triggered: false,
            header: None,
            instruction_tracer: emu_core::instruction_tracer::InstructionTracer::new(),
        }
    }

    emu_core::impl_instruction_tracer_methods!();

    /// Set the controller button state.
    ///
    /// The input `state` uses the standard frontend format:
    /// - Bit 0: A, Bit 1: B, Bit 2: Select, Bit 3: Start
    /// - Bit 4: Right, Bit 5: Left, Bit 6: Up, Bit 7: Down
    /// - Bit 8: R, Bit 9: L
    ///
    /// Active-high (1 = pressed). This is converted to GBA's active-low KEYINPUT format.
    pub fn set_controller(&mut self, state: u16) {
        // GBA KEYINPUT format (active-low: 0 = pressed, 1 = released):
        // Bit 0: A, Bit 1: B, Bit 2: Select, Bit 3: Start
        // Bit 4: Right, Bit 5: Left, Bit 6: Up, Bit 7: Down
        // Bit 8: R, Bit 9: L
        //
        // Frontend format uses the same bit layout but active-high.
        // So we just invert and mask to 10 bits.
        self.cpu.memory.keyinput = (!state) & 0x03FF;
    }

    /// Check if a ROM is loaded
    fn has_rom(&self) -> bool {
        !self.cpu.memory.rom.is_empty()
    }

    /// Get the parsed cartridge header (if a ROM is loaded)
    pub fn cartridge_header(&self) -> Option<&cartridge::GbaCartridgeHeader> {
        self.header.as_ref()
    }

    /// Get the loaded ROM size in bytes
    pub fn rom_size(&self) -> usize {
        self.cpu.memory.rom.len()
    }

    /// Generate audio samples for the last frame.
    ///
    /// Returns interleaved stereo i16 samples at 44,100 Hz.
    /// Samples were generated in real-time during step_frame;
    /// this method drains the buffer and pads/truncates to fit `count`.
    pub fn get_audio_samples(&mut self, count: usize) -> Vec<i16> {
        let stereo_count = count * 2;
        self.cpu.memory.apu.drain_samples(stereo_count)
    }

    /// Get tile viewer data for debugging PPU graphics.
    ///
    /// Provides VRAM, palette RAM, OAM, and PPU state for the inspector.
    pub fn get_tile_viewer_data(&self) -> TileViewerData {
        // Helper to read 16-bit I/O register
        let read_io_u16 = |offset: usize| -> u16 {
            if offset + 1 < self.cpu.memory.io.len() {
                u16::from_le_bytes([self.cpu.memory.io[offset], self.cpu.memory.io[offset + 1]])
            } else {
                0
            }
        };

        // Convert 15-bit BGR palette to 32-bit RGBA for display
        let convert_palette = |pal_ram: &[u8]| -> Vec<u32> {
            let mut result = Vec::with_capacity(512);
            for i in 0..512 {
                let offset = i * 2;
                if offset + 1 < pal_ram.len() {
                    let bgr555 = u16::from_le_bytes([pal_ram[offset], pal_ram[offset + 1]]);
                    let r = ((bgr555 & 0x1F) << 3) as u32;
                    let g = (((bgr555 >> 5) & 0x1F) << 3) as u32;
                    let b = (((bgr555 >> 10) & 0x1F) << 3) as u32;
                    result.push(0xFF00_0000 | (r << 16) | (g << 8) | b);
                } else {
                    result.push(0xFF00_0000); // Black
                }
            }
            result
        };

        // Read BG scroll offsets
        let bg_scroll = [
            (read_io_u16(0x010), read_io_u16(0x012)), // BG0HOFS, BG0VOFS
            (read_io_u16(0x014), read_io_u16(0x016)), // BG1HOFS, BG1VOFS
            (read_io_u16(0x018), read_io_u16(0x01A)), // BG2HOFS, BG2VOFS
            (read_io_u16(0x01C), read_io_u16(0x01E)), // BG3HOFS, BG3VOFS
        ];

        TileViewerData {
            vram: self.cpu.memory.vram.clone(),
            palette_ram: self.cpu.memory.palette.clone(),
            oam: self.cpu.memory.oam.clone(),
            master_palette: convert_palette(&self.cpu.memory.palette),
            dispcnt: read_io_u16(0x000),
            bg0cnt: read_io_u16(0x008),
            bg1cnt: read_io_u16(0x00A),
            bg2cnt: read_io_u16(0x00C),
            bg3cnt: read_io_u16(0x00E),
            bg_scroll,
            bldcnt: read_io_u16(0x050),
            bldalpha: read_io_u16(0x052),
        }
    }

    /// Execute any pending DMA transfers.
    ///
    /// DMA transfers read/write through the memory bus, bypassing I/O register
    /// side effects for DMA source/dest registers themselves.
    /// Returns the number of CPU cycles consumed by the DMA.
    fn execute_dma(&mut self) -> u64 {
        if !self.cpu.memory.dma.is_transferring() {
            return 0;
        }

        // Detect EEPROM size from DMA3 transfers targeting EEPROM addresses
        if self.cpu.memory.save_type == cartridge::SaveType::Eeprom {
            if let Some((word_count, dst_addr)) = self.cpu.memory.dma.dma3_transfer_info() {
                if eeprom::Eeprom::is_eeprom_address(dst_addr, self.cpu.memory.rom.len()) {
                    self.cpu
                        .memory
                        .eeprom
                        .borrow_mut()
                        .detect_size_from_dma(word_count);
                }
            }
        }

        // Take DMA out to avoid borrow conflicts between DMA and bus
        let mut dma = std::mem::take(&mut self.cpu.memory.dma);

        let (cycles, irq_bits) = dma.execute_with_bus(&mut self.cpu.memory);

        // Put DMA back
        self.cpu.memory.dma = dma;

        if irq_bits != 0 {
            self.cpu.memory.request_interrupt(irq_bits);
        }

        cycles
    }

    /// Update DISPSTAT and VCOUNT I/O registers for current scanline and dot position.
    /// Called after every CPU step to keep display status accurate for polling games.
    fn update_display_status(&mut self) {
        // VCOUNT register
        self.cpu.memory.io[REG_VCOUNT] = self.scanline as u8;

        // DISPSTAT register - preserve enable bits (bits 3-5) and VCount target (byte 1)
        let dispstat = self.cpu.memory.io[REG_DISPSTAT];
        let vcount_target = self.cpu.memory.io[REG_DISPSTAT + 1];

        let in_vblank = self.scanline >= VISIBLE_SCANLINES;
        let in_hblank = self.scanline_cycles >= HBLANK_START;
        let vcount_match = self.scanline as u8 == vcount_target;

        // Clear status bits (0-2) but preserve enable bits (3-5)
        let mut new_dispstat =
            dispstat & !(DISPSTAT_VBLANK | DISPSTAT_HBLANK | DISPSTAT_VCOUNT_MATCH);
        if in_vblank {
            new_dispstat |= DISPSTAT_VBLANK;
        }
        if in_hblank {
            new_dispstat |= DISPSTAT_HBLANK;
        }
        if vcount_match {
            new_dispstat |= DISPSTAT_VCOUNT_MATCH;
        }
        self.cpu.memory.io[REG_DISPSTAT] = new_dispstat;
    }
}

impl System for GbaSystem {
    type Error = GbaError;

    fn reset(&mut self) {
        self.cpu.reset();
        self.ppu.reset();
        self.cpu.memory.dma.reset();
        self.cpu.memory.timers.reset();
        self.cpu.memory.apu.reset();
        self.total_cycles = 0;
        self.scanline = 0;
        self.scanline_cycles = 0;
        self.hblank_triggered = false;
        // header is preserved across resets (only cleared on unmount)
    }

    fn step_frame(&mut self) -> Result<Frame, Self::Error> {
        if !self.has_rom() {
            return Err(GbaError::NoCartridge);
        }

        // Run CPU for one frame's worth of cycles
        let frame_end = self.total_cycles + CYCLES_PER_FRAME;

        while self.total_cycles < frame_end {
            // Execute one CPU instruction (or advance halted cycles)
            let pc_before = self.cpu.pc();
            let mut cycles = self.cpu.step() as u64;

            // Fast-forward halted CPU: batch multiple idle cycles together
            // When halted (step returns 1), advance to the nearest event boundary
            // instead of looping one cycle at a time.
            if cycles == 1 && self.cpu.halted {
                // Calculate cycles until next scanline event
                let cycles_to_hblank =
                    if !self.hblank_triggered && self.scanline_cycles < HBLANK_START {
                        HBLANK_START - self.scanline_cycles
                    } else {
                        u64::MAX
                    };
                let cycles_to_scanline_end = CYCLES_PER_SCANLINE - self.scanline_cycles;
                let cycles_to_frame_end = frame_end.saturating_sub(self.total_cycles);

                let mut batch = cycles_to_hblank
                    .min(cycles_to_scanline_end)
                    .min(cycles_to_frame_end);

                // Also limit by next timer overflow to keep FIFO fed accurately
                let timer_batch = self.cpu.memory.timers.cycles_until_overflow();
                if timer_batch > 0 {
                    batch = batch.min(timer_batch);
                }

                // Ensure at least 1 cycle
                cycles = batch.max(1);
            }

            self.total_cycles += cycles;
            self.scanline_cycles += cycles;

            // Record instruction if tracing is enabled
            if self.instruction_tracer.is_enabled() {
                if let Some(instr) = self.disassemble_instruction(pc_before) {
                    let cpu_state = self.get_cpu_state();
                    self.instruction_tracer.trace(instr, cpu_state);
                }

                if self.instruction_tracer.history_size()
                    >= self.instruction_tracer.get_max_history()
                {
                    // Prevent long-running traces from stalling the UI.
                    self.instruction_tracer.set_enabled(false);
                }
            }

            // Tick hardware timers
            let timer_irqs = self.cpu.memory.timers.tick(cycles as u32);
            if timer_irqs != 0 {
                self.cpu.memory.request_interrupt(timer_irqs);
            }

            // Feed APU FIFOs from timer overflows (Timer 0 and Timer 1 drive DMA sound)
            let mut sound_dma_requested = false;
            for timer_idx in 0..2u8 {
                let overflows = self.cpu.memory.timers.last_overflows[timer_idx as usize];
                for _ in 0..overflows {
                    let refill = self.cpu.memory.apu.on_timer_overflow(timer_idx);
                    if refill != 0 {
                        // Request DMA refill for FIFOs that are running low
                        self.cpu
                            .memory
                            .dma
                            .notify_timing(dma::DmaStartTiming::Special);
                        sound_dma_requested = true;
                    }
                }
            }

            // Execute sound DMA immediately so FIFO has fresh samples.
            // On real hardware, sound DMA fires as soon as the timer overflows
            // and the FIFO requests a refill — not at the next HBlank/VBlank.
            // Without this, the FIFO runs dry during VBlank (68 scanlines with
            // no HBlank DMA), causing audio freezes every frame.
            if sound_dma_requested {
                let dma_cycles = self.execute_dma();
                self.total_cycles += dma_cycles;
                self.scanline_cycles += dma_cycles;
                // APU continues running during DMA (hardware doesn't halt audio)
                self.cpu.memory.apu.tick(dma_cycles as u32);
            }

            // Clock APU and generate output samples in real-time.
            // Must happen AFTER timer ticks, FIFO pops, AND sound DMA refills
            // so mix_channels reads the current FIFO sample.
            self.cpu.memory.apu.tick(cycles as u32);

            // Execute any other pending DMA transfers (immediate or queued)
            if self.cpu.memory.dma.is_transferring() {
                let dma_cycles = self.execute_dma();
                self.total_cycles += dma_cycles;
                self.scanline_cycles += dma_cycles;
                // APU continues running during non-sound DMA too
                self.cpu.memory.apu.tick(dma_cycles as u32);
            }

            // Update DISPSTAT flags after every CPU step so polling games
            // see accurate HBlank/VBlank/VCount status.
            self.update_display_status();

            // HBlank start: fires once per scanline when we cross cycle 1006
            if self.scanline_cycles >= HBLANK_START && !self.hblank_triggered {
                self.hblank_triggered = true;

                // Fire HBlank IRQ if enabled
                let dispstat = self.cpu.memory.io[REG_DISPSTAT];
                if dispstat & DISPSTAT_HBLANK_IRQ != 0 {
                    self.cpu.memory.request_interrupt(IRQ_HBLANK);
                }

                if self.scanline < VISIBLE_SCANLINES {
                    // Apply any pending affine reference point writes
                    // (from CPU or DMA during the draw period)
                    let dirty = self.cpu.memory.affine_ref_dirty;
                    if dirty != 0 {
                        self.ppu.apply_affine_ref_writes(&self.cpu.memory.io, dirty);
                        self.cpu.memory.affine_ref_dirty = 0;
                    }

                    // Render this scanline before entering HBlank
                    self.ppu.render_scanline(
                        self.scanline,
                        &self.cpu.memory.io,
                        &self.cpu.memory.palette,
                        &self.cpu.memory.vram,
                        &self.cpu.memory.oam,
                    );

                    // Trigger HBlank DMA (only during visible scanlines)
                    self.cpu
                        .memory
                        .dma
                        .notify_timing(dma::DmaStartTiming::HBlank);
                    let dma_cycles = self.execute_dma();
                    self.total_cycles += dma_cycles;
                    self.scanline_cycles += dma_cycles;
                }
            }

            // Scanline boundary: advance to next scanline
            if self.scanline_cycles >= CYCLES_PER_SCANLINE {
                self.scanline_cycles -= CYCLES_PER_SCANLINE;
                self.hblank_triggered = false;

                // Advance to next scanline
                self.scanline += 1;

                // VBlank start (scanline 160)
                if self.scanline == VISIBLE_SCANLINES {
                    self.ppu.on_vblank(&self.cpu.memory.io);
                    self.cpu.memory.affine_ref_dirty = 0; // VBlank latch overrides pending writes

                    // VBlank IRQ
                    let dispstat = self.cpu.memory.io[REG_DISPSTAT];
                    if dispstat & DISPSTAT_VBLANK_IRQ != 0 {
                        self.cpu.memory.request_interrupt(IRQ_VBLANK);
                    }

                    // Trigger VBlank DMA
                    self.cpu
                        .memory
                        .dma
                        .notify_timing(dma::DmaStartTiming::VBlank);
                    let dma_cycles = self.execute_dma();
                    self.total_cycles += dma_cycles;
                    self.scanline_cycles += dma_cycles;
                }

                // Frame wrap
                if self.scanline >= TOTAL_SCANLINES {
                    self.scanline = 0;
                    // Re-latch affine registers at frame start
                    self.ppu.latch_affine_registers(&self.cpu.memory.io);
                    self.cpu.memory.affine_ref_dirty = 0;
                }

                // VCount match IRQ
                let vcount_target = self.cpu.memory.io[REG_DISPSTAT + 1];
                let dispstat = self.cpu.memory.io[REG_DISPSTAT];
                if self.scanline as u8 == vcount_target && dispstat & DISPSTAT_VCOUNT_IRQ != 0 {
                    self.cpu.memory.request_interrupt(IRQ_VCOUNT);
                }

                // Update display registers for the new scanline
                self.update_display_status();
            }
        }

        Ok(self.ppu.clone_frame())
    }

    fn save_state(&self) -> Value {
        use base64::{engine::general_purpose::STANDARD, Engine};

        // Get CPU state
        let cpu_state = self.cpu.get_state();

        serde_json::json!({
            "system": "gba",
            "version": 1,
            "total_cycles": self.total_cycles,
            "scanline": self.scanline,
            "scanline_cycles": self.scanline_cycles,
            "cpu": {
                "gpr": cpu_state.gpr,
                "cpsr": cpu_state.cpsr,
                "fiq_r8_r12": cpu_state.fiq_r8_r12,
                "usr_r8_r12": cpu_state.usr_r8_r12,
                "fiq_r13_r14": cpu_state.fiq_r13_r14,
                "irq_r13_r14": cpu_state.irq_r13_r14,
                "svc_r13_r14": cpu_state.svc_r13_r14,
                "abt_r13_r14": cpu_state.abt_r13_r14,
                "und_r13_r14": cpu_state.und_r13_r14,
                "usr_r13_r14": cpu_state.usr_r13_r14,
                "spsr_fiq": cpu_state.spsr_fiq,
                "spsr_irq": cpu_state.spsr_irq,
                "spsr_svc": cpu_state.spsr_svc,
                "spsr_abt": cpu_state.spsr_abt,
                "spsr_und": cpu_state.spsr_und,
                "pipeline_flushed": cpu_state.pipeline_flushed,
                "halted": cpu_state.halted,
                "intr_wait_flags": cpu_state.intr_wait_flags,
                "cycles": cpu_state.cycles,
            },
            "memory": {
                "ewram": STANDARD.encode(&self.cpu.memory.ewram),
                "iwram": STANDARD.encode(&self.cpu.memory.iwram),
                "io": STANDARD.encode(&self.cpu.memory.io),
                "palette": STANDARD.encode(&self.cpu.memory.palette),
                "vram": STANDARD.encode(&self.cpu.memory.vram),
                "oam": STANDARD.encode(&self.cpu.memory.oam),
                "sram": STANDARD.encode(&self.cpu.memory.sram),
                "eeprom": STANDARD.encode(self.cpu.memory.eeprom.borrow().data()),
                "flash": STANDARD.encode(self.cpu.memory.flash.borrow().data()),
            },
            "interrupts": {
                "ime": self.cpu.memory.ime,
                "ie": self.cpu.memory.ie,
                "if_flags": self.cpu.memory.if_flags,
            },
            "ppu": {
                "bg2_ref_x": self.ppu.bg2_ref_x,
                "bg2_ref_y": self.ppu.bg2_ref_y,
                "bg3_ref_x": self.ppu.bg3_ref_x,
                "bg3_ref_y": self.ppu.bg3_ref_y,
            }
        })
    }

    fn load_state(&mut self, v: &Value) -> Result<(), serde_json::Error> {
        use base64::{engine::general_purpose::STANDARD, Engine};

        // Validate version
        if v["version"].as_u64() != Some(1) {
            return Err(serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid save state version",
            )));
        }

        // Restore basic state
        self.total_cycles = v["total_cycles"].as_u64().unwrap_or(0);
        self.scanline = v["scanline"].as_u64().unwrap_or(0) as u32;
        self.scanline_cycles = v["scanline_cycles"].as_u64().unwrap_or(0);

        // Restore CPU state
        let cpu_json = &v["cpu"];
        let mut cpu_state = emu_core::cpu_arm7tdmi::CpuState {
            gpr: [0; 16],
            cpsr: cpu_json["cpsr"].as_u64().unwrap_or(0) as u32,
            fiq_r8_r12: [0; 5],
            usr_r8_r12: [0; 5],
            fiq_r13_r14: [0; 2],
            irq_r13_r14: [0; 2],
            svc_r13_r14: [0; 2],
            abt_r13_r14: [0; 2],
            und_r13_r14: [0; 2],
            usr_r13_r14: [0; 2],
            spsr_fiq: cpu_json["spsr_fiq"].as_u64().unwrap_or(0) as u32,
            spsr_irq: cpu_json["spsr_irq"].as_u64().unwrap_or(0) as u32,
            spsr_svc: cpu_json["spsr_svc"].as_u64().unwrap_or(0) as u32,
            spsr_abt: cpu_json["spsr_abt"].as_u64().unwrap_or(0) as u32,
            spsr_und: cpu_json["spsr_und"].as_u64().unwrap_or(0) as u32,
            pipeline_flushed: cpu_json["pipeline_flushed"].as_bool().unwrap_or(false),
            halted: cpu_json["halted"].as_bool().unwrap_or(false),
            intr_wait_flags: cpu_json["intr_wait_flags"].as_u64().unwrap_or(0) as u16,
            cycles: cpu_json["cycles"].as_u64().unwrap_or(0),
        };

        // Restore GPRs
        if let Some(gpr_arr) = cpu_json["gpr"].as_array() {
            for (i, val) in gpr_arr.iter().enumerate() {
                if i < 16 {
                    cpu_state.gpr[i] = val.as_u64().unwrap_or(0) as u32;
                }
            }
        }

        // Restore banked registers (similar pattern for all)
        macro_rules! restore_banked {
            ($field:ident, $size:expr) => {
                if let Some(arr) = cpu_json[stringify!($field)].as_array() {
                    for (i, val) in arr.iter().enumerate() {
                        if i < $size {
                            cpu_state.$field[i] = val.as_u64().unwrap_or(0) as u32;
                        }
                    }
                }
            };
        }

        restore_banked!(fiq_r8_r12, 5);
        restore_banked!(usr_r8_r12, 5);
        restore_banked!(fiq_r13_r14, 2);
        restore_banked!(irq_r13_r14, 2);
        restore_banked!(svc_r13_r14, 2);
        restore_banked!(abt_r13_r14, 2);
        restore_banked!(und_r13_r14, 2);
        restore_banked!(usr_r13_r14, 2);

        self.cpu.set_state(&cpu_state);

        // Restore memory
        let mem_json = &v["memory"];
        if let Some(ewram_str) = mem_json["ewram"].as_str() {
            if let Ok(data) = STANDARD.decode(ewram_str) {
                let len = data.len().min(self.cpu.memory.ewram.len());
                self.cpu.memory.ewram[..len].copy_from_slice(&data[..len]);
            }
        }

        if let Some(iwram_str) = mem_json["iwram"].as_str() {
            if let Ok(data) = STANDARD.decode(iwram_str) {
                let len = data.len().min(self.cpu.memory.iwram.len());
                self.cpu.memory.iwram[..len].copy_from_slice(&data[..len]);
            }
        }

        if let Some(io_str) = mem_json["io"].as_str() {
            if let Ok(data) = STANDARD.decode(io_str) {
                let len = data.len().min(self.cpu.memory.io.len());
                self.cpu.memory.io[..len].copy_from_slice(&data[..len]);
            }
        }

        if let Some(palette_str) = mem_json["palette"].as_str() {
            if let Ok(data) = STANDARD.decode(palette_str) {
                let len = data.len().min(self.cpu.memory.palette.len());
                self.cpu.memory.palette[..len].copy_from_slice(&data[..len]);
            }
        }

        if let Some(vram_str) = mem_json["vram"].as_str() {
            if let Ok(data) = STANDARD.decode(vram_str) {
                let len = data.len().min(self.cpu.memory.vram.len());
                self.cpu.memory.vram[..len].copy_from_slice(&data[..len]);
            }
        }

        if let Some(oam_str) = mem_json["oam"].as_str() {
            if let Ok(data) = STANDARD.decode(oam_str) {
                let len = data.len().min(self.cpu.memory.oam.len());
                self.cpu.memory.oam[..len].copy_from_slice(&data[..len]);
            }
        }

        if let Some(sram_str) = mem_json["sram"].as_str() {
            if let Ok(data) = STANDARD.decode(sram_str) {
                let len = data.len().min(self.cpu.memory.sram.len());
                self.cpu.memory.sram[..len].copy_from_slice(&data[..len]);
            }
        }

        if let Some(eeprom_str) = mem_json["eeprom"].as_str() {
            if let Ok(data) = STANDARD.decode(eeprom_str) {
                self.cpu.memory.eeprom.borrow_mut().set_data(&data);
            }
        }

        if let Some(flash_str) = mem_json["flash"].as_str() {
            if let Ok(data) = STANDARD.decode(flash_str) {
                self.cpu.memory.flash.borrow_mut().load_data(&data);
            }
        }

        // Restore interrupts
        let int_json = &v["interrupts"];
        self.cpu.memory.ime = int_json["ime"].as_bool().unwrap_or(false);
        self.cpu.memory.ie = int_json["ie"].as_u64().unwrap_or(0) as u16;
        self.cpu.memory.if_flags = int_json["if_flags"].as_u64().unwrap_or(0) as u16;

        // Restore PPU state
        let ppu_json = &v["ppu"];
        self.ppu.bg2_ref_x = ppu_json["bg2_ref_x"].as_i64().unwrap_or(0) as i32;
        self.ppu.bg2_ref_y = ppu_json["bg2_ref_y"].as_i64().unwrap_or(0) as i32;
        self.ppu.bg3_ref_x = ppu_json["bg3_ref_x"].as_i64().unwrap_or(0) as i32;
        self.ppu.bg3_ref_y = ppu_json["bg3_ref_y"].as_i64().unwrap_or(0) as i32;

        Ok(())
    }

    fn mount_points(&self) -> Vec<MountPointInfo> {
        vec![MountPointInfo {
            id: "Cartridge".to_string(),
            name: "Cartridge Slot".to_string(),
            extensions: vec!["gba".to_string(), "agb".to_string()],
            required: true,
        }]
    }

    fn mount(&mut self, mount_point_id: &str, data: &[u8]) -> Result<(), Self::Error> {
        match mount_point_id {
            "Cartridge" => {
                // Parse cartridge header before loading
                self.header = cartridge::GbaCartridgeHeader::from_bytes(data);

                // Set save type on bus so memory map knows about EEPROM/Flash
                if let Some(ref header) = self.header {
                    self.cpu.memory.save_type = header.save_type;
                    // Initialize Flash with the correct size if needed
                    match header.save_type {
                        cartridge::SaveType::Flash128K => {
                            self.cpu.memory.flash = RefCell::new(flash::Flash::new(true));
                        }
                        cartridge::SaveType::Flash64K => {
                            self.cpu.memory.flash = RefCell::new(flash::Flash::new(false));
                        }
                        _ => {}
                    }
                }

                self.cpu.memory.load_rom(data);
                self.reset();

                // After reset, set PC to ROM entry point
                // GBA ROMs start at 0x08000000 and have an ARM branch instruction
                // at the cartridge header. The BIOS normally jumps here after boot.
                // For now, skip BIOS and jump directly to ROM.
                self.cpu.gpr[15] = 0x08000000;
                // Set initial register values (as BIOS would after boot)
                self.cpu.gpr[0] = 0x08000000; // R0 = entry point (BIOS convention)
                self.cpu.gpr[1] = 0x000000EA; // R1 = boot mode indicator
                self.cpu.gpr[13] = 0x03007F00; // SP_usr/sys

                // Switch to System mode (post-BIOS state)
                self.cpu.cpsr = 0x1F; // System mode, ARM, IRQ+FIQ enabled

                // Initialize banked stack pointers (as the real BIOS does)
                // SP_irq = 0x03007FA0
                self.cpu
                    .set_banked_sp(emu_core::cpu_arm7tdmi::ProcessorMode::Irq, 0x03007FA0);
                // SP_svc = 0x03007FE0
                self.cpu.set_banked_sp(
                    emu_core::cpu_arm7tdmi::ProcessorMode::Supervisor,
                    0x03007FE0,
                );

                // Set post-boot I/O register state (as the real BIOS leaves them)
                self.cpu.memory.io[0x300] = 0x01; // POSTFLG = 1 (boot complete)
                self.cpu.memory.io[0x088] = 0x00; // SOUNDBIAS low byte
                self.cpu.memory.io[0x089] = 0x02; // SOUNDBIAS = 0x0200 (default)

                Ok(())
            }
            _ => Err(GbaError::InvalidMountPoint(mount_point_id.to_string())),
        }
    }

    fn unmount(&mut self, mount_point_id: &str) -> Result<(), Self::Error> {
        match mount_point_id {
            "Cartridge" => {
                self.cpu.memory.unload_rom();
                self.header = None;
                Ok(())
            }
            _ => Err(GbaError::InvalidMountPoint(mount_point_id.to_string())),
        }
    }

    fn is_mounted(&self, mount_point_id: &str) -> bool {
        match mount_point_id {
            "Cartridge" => !self.cpu.memory.rom.is_empty(),
            _ => false,
        }
    }

    fn get_total_cycles(&self) -> u64 {
        self.total_cycles
    }

    fn debugger(&self) -> Option<&dyn emu_core::debug::Debugger> {
        Some(self)
    }
}
