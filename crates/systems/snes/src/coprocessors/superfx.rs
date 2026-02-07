//! SuperFX (GSU) Graphics Coprocessor Implementation
//!
//! The SuperFX is a custom 16-bit RISC graphics coprocessor used in games like
//! Star Fox, Yoshi's Island, and Doom. It provides:
//! - 16 general-purpose 16-bit registers (R0-R15)
//! - 512-byte instruction cache for fast execution
//! - Pixel plotting and graphics operations
//! - Up to 21.48 MHz operation (GSU-2)
//!
//! ## Memory Mapping
//!
//! **Registers (SNES CPU access):**
//! - $3000-$32FF in banks $00-$3F and $80-$BF: GSU registers and control
//!
//! **GSU RAM (Game Pak RAM / Frame Buffer):**
//! - $700000-$71FFFF (128 KB) or higher for 256/512 KB variants
//! - Acts as frame buffer and general-purpose work RAM
//!
//! **ROM:**
//! - Up to 2 MB directly accessible via bank switching
//!
//! ## References
//!
//! - https://snes.nesdev.org/wiki/Super_FX
//! - https://sneslab.net/wiki/Super_FX
//! - https://jsgroth.dev/blog/posts/snes-coprocessors-part-7/
//! - https://wiki.superfamicom.org/super-fx-opcode-matrix
//! - https://en.wikibooks.org/wiki/Super_NES_Programming/Super_FX_tutorial

use super::{ChipType, EnhancementChip};
use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;

/// SuperFX flags in Status/Flag Register (SFR)
#[derive(Debug, Clone, Copy, Default)]
struct Flags {
    /// Zero flag
    z: bool,
    /// Carry flag
    cy: bool,
    /// Sign flag
    s: bool,
    /// Overflow flag
    ov: bool,
    /// Go flag - GSU is running
    g: bool,
    /// ROM buffer valid flag
    r: bool,
    /// ALT1 prefix mode
    alt1: bool,
    /// ALT2 prefix mode
    alt2: bool,
    /// Immediate low byte flag
    il: bool,
    /// Immediate high byte flag
    ih: bool,
    /// Branch flag
    b: bool,
    /// IRQ flag
    irq: bool,
}

impl Flags {
    /// Convert flags to SFR register value (low byte)
    fn to_sfr_low(self) -> u8 {
        let mut val = 0u8;
        if self.z {
            val |= 0x02;
        } // Z
        if self.cy {
            val |= 0x04;
        } // CY
        if self.s {
            val |= 0x08;
        } // S
        if self.ov {
            val |= 0x10;
        } // OV
        if self.g {
            val |= 0x20;
        } // G
        if self.r {
            val |= 0x40;
        } // R
        if self.alt1 {
            val |= 0x01;
        } // ALT1 (bit 0)
        val
    }

    /// Convert flags to SFR register value (high byte)
    fn to_sfr_high(self) -> u8 {
        let mut val = 0u8;
        if self.alt2 {
            val |= 0x01;
        } // ALT2
        if self.il {
            val |= 0x02;
        } // IL
        if self.ih {
            val |= 0x04;
        } // IH
        if self.b {
            val |= 0x08;
        } // B
        if self.irq {
            val |= 0x80;
        } // IRQ
        val
    }

    /// Set flags from SFR register value (low byte)
    fn set_sfr_low(&mut self, val: u8) {
        self.z = (val & 0x02) != 0;
        self.cy = (val & 0x04) != 0;
        self.s = (val & 0x08) != 0;
        self.ov = (val & 0x10) != 0;
        self.g = (val & 0x20) != 0;
        self.r = (val & 0x40) != 0;
        self.alt1 = (val & 0x01) != 0;
    }

    /// Set flags from SFR register value (high byte)
    fn set_sfr_high(&mut self, val: u8) {
        self.alt2 = (val & 0x01) != 0;
        self.il = (val & 0x02) != 0;
        self.ih = (val & 0x04) != 0;
        self.b = (val & 0x08) != 0;
        self.irq = (val & 0x80) != 0;
    }
}

/// SuperFX Graphics Support Unit
#[derive(Clone, Serialize, Deserialize)]
pub struct SuperFx {
    /// 16 general-purpose registers (R0-R15)
    /// R14 = ROM address pointer, R15 = Program Counter
    regs: [u16; 16],

    /// Status/Flag Register fields (not serialized as Flags struct)
    flags_z: bool,
    flags_cy: bool,
    flags_s: bool,
    flags_ov: bool,
    flags_g: bool,
    flags_r: bool,
    flags_alt1: bool,
    flags_alt2: bool,
    flags_il: bool,
    flags_ih: bool,
    flags_b: bool,
    flags_irq: bool,

    /// Program Bank Register (PBR) - 8-bit bank for PC
    pbr: u8,

    /// ROM Bank Register (ROMBR) - 8-bit bank for ROM reads
    rombr: u8,

    /// RAM Bank Register (RAMBR) - 1-bit for RAM access
    rambr: u8,

    /// Cache Base Register (CBR) - Cache region base
    cbr: u16,

    /// Screen Base Register (SCBR) - Pixel buffer base
    scbr: u8,

    /// Color Register (COLR) - Current plot color
    colr: u8,

    /// Plot Option Register (POR) - Plot options
    por: u8,

    /// Screen Mode Register (SCMR) - Screen mode config
    scmr: u8,

    /// Config Register (CFGR) - IRQ/multiplier speed config
    cfgr: u8,

    /// Clock Select Register (CLSR) - Clock speed select
    clsr: u8,

    /// 512-byte instruction cache (currently unused for simplicity)
    #[serde(with = "BigArray")]
    cache: [u8; 512],

    /// GSU RAM (128 KB default, up to 256 KB; acts as frame buffer)
    ram: Vec<u8>,

    /// ROM data (passed from cartridge for instruction fetching and GETC)
    rom: Vec<u8>,

    /// ROM buffer for ROM reads (GETC result)
    rom_buffer: u8,

    /// Current source/destination register (set by WITH instruction)
    sreg_dreg: usize,

    /// Multiplication result (32-bit)
    mult_result: u32,

    /// Cycle counter for timing
    cycles: u64,

    /// Chip variant (false = SuperFX/GSU-1, true = SuperFX2/GSU-2)
    is_superfx2: bool,
}

impl Default for SuperFx {
    fn default() -> Self {
        Self {
            regs: [0; 16],
            flags_z: false,
            flags_cy: false,
            flags_s: false,
            flags_ov: false,
            flags_g: false,
            flags_r: false,
            flags_alt1: false,
            flags_alt2: false,
            flags_il: false,
            flags_ih: false,
            flags_b: false,
            flags_irq: false,
            pbr: 0,
            rombr: 0,
            rambr: 0,
            cbr: 0,
            scbr: 0,
            colr: 0,
            por: 0,
            scmr: 0,
            cfgr: 0,
            clsr: 0,
            cache: [0; 512],
            ram: vec![0; 128 * 1024], // 128 KB default
            rom: Vec::new(),
            rom_buffer: 0,
            sreg_dreg: 0, // Default to R0
            mult_result: 0,
            cycles: 0,
            is_superfx2: false,
        }
    }
}

impl SuperFx {
    /// Create a new SuperFX instance
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new SuperFX2 instance
    pub fn new_superfx2() -> Self {
        Self {
            is_superfx2: true,
            ..Default::default()
        }
    }

    /// Set ROM data for instruction fetching and GETC operations
    pub fn set_rom(&mut self, rom: Vec<u8>) {
        self.rom = rom;
    }

    /// Get flags as Flags struct for easier manipulation
    fn get_flags(&self) -> Flags {
        Flags {
            z: self.flags_z,
            cy: self.flags_cy,
            s: self.flags_s,
            ov: self.flags_ov,
            g: self.flags_g,
            r: self.flags_r,
            alt1: self.flags_alt1,
            alt2: self.flags_alt2,
            il: self.flags_il,
            ih: self.flags_ih,
            b: self.flags_b,
            irq: self.flags_irq,
        }
    }

    /// Set flags from Flags struct
    fn set_flags(&mut self, flags: Flags) {
        self.flags_z = flags.z;
        self.flags_cy = flags.cy;
        self.flags_s = flags.s;
        self.flags_ov = flags.ov;
        self.flags_g = flags.g;
        self.flags_r = flags.r;
        self.flags_alt1 = flags.alt1;
        self.flags_alt2 = flags.alt2;
        self.flags_il = flags.il;
        self.flags_ih = flags.ih;
        self.flags_b = flags.b;
        self.flags_irq = flags.irq;
    }

    /// Get program counter (R15)
    fn pc(&self) -> u16 {
        self.regs[15]
    }

    /// Set program counter (R15)
    fn set_pc(&mut self, val: u16) {
        self.regs[15] = val;
    }

    /// Get source register (R0 by default, or last set by WITH)
    fn sreg(&self) -> usize {
        self.sreg_dreg
    }

    /// Get destination register (same as source)
    fn dreg(&self) -> usize {
        self.sreg_dreg
    }

    /// Read a byte from GSU address space (ROM or RAM)
    fn read_byte(&self, addr: u32) -> u8 {
        // GSU has its own address space
        // For simplicity, treat lower addresses as ROM and high addresses as RAM
        // Real implementation would use banking registers properly
        let addr = addr as usize;

        // Try ROM first (mirrored if needed)
        if !self.rom.is_empty() && addr < 0x800000 {
            let rom_addr = addr % self.rom.len();
            return self.rom[rom_addr];
        }

        // Fall back to RAM
        if addr < self.ram.len() {
            self.ram[addr]
        } else {
            0
        }
    }

    /// Write a byte to GSU address space
    fn write_byte(&mut self, addr: u32, val: u8) {
        let addr = addr as usize;
        if addr < self.ram.len() {
            self.ram[addr] = val;
        }
    }

    /// Read a word from GSU address space
    fn read_word(&mut self, addr: u32) -> u16 {
        let low = self.read_byte(addr);
        let high = self.read_byte(addr + 1);
        u16::from_le_bytes([low, high])
    }

    /// Write a word to GSU address space
    fn write_word(&mut self, addr: u32, val: u16) {
        let bytes = val.to_le_bytes();
        self.write_byte(addr, bytes[0]);
        self.write_byte(addr + 1, bytes[1]);
    }

    /// Update zero and sign flags based on value
    fn update_zs_flags(&mut self, val: u16) {
        self.flags_z = val == 0;
        self.flags_s = (val & 0x8000) != 0;
    }

    /// Execute a single instruction, returning the number of bytes to advance PC
    fn execute_instruction(&mut self, opcode: u8) -> u16 {
        // Simplified instruction execution
        // Full implementation would handle all 98 opcodes

        let pc_increment = match opcode {
            // STOP - Stop GSU
            0x00 => {
                self.flags_g = false;
                1
            }
            // NOP - No operation
            0x01 => 1,
            // CACHE - Load cache
            0x02 => {
                // Simplified: just set flag
                self.flags_r = true;
                1
            }
            // LSR - Logical shift right
            0x03 => {
                let src = self.sreg();
                let val = self.regs[src];
                self.flags_cy = (val & 1) != 0;
                self.regs[src] = val >> 1;
                self.update_zs_flags(self.regs[src]);
                1
            }
            // ROL - Rotate left
            0x04 => {
                let src = self.sreg();
                let val = self.regs[src];
                let cy = if self.flags_cy { 1 } else { 0 };
                self.flags_cy = (val & 0x8000) != 0;
                self.regs[src] = (val << 1) | cy;
                self.update_zs_flags(self.regs[src]);
                1
            }
            // BRA - Branch always (relative)
            0x05 => {
                // Read signed byte offset from (PBR << 16) | (PC+1), branch is relative to PC+2
                let offset_addr = ((self.pbr as u32) << 16) | ((self.pc() + 1) as u32);
                let offset = self.read_byte(offset_addr) as i8;
                let new_pc = ((self.pc() + 2) as i32 + offset as i32) as u16;
                self.set_pc(new_pc);
                0 // PC already set
            }
            // BGE/BLT - Branch on sign flag
            0x06 => {
                if !self.flags_s {
                    // Read signed byte offset from (PBR << 16) | (PC+1), branch is relative to PC+2
                    let offset_addr = ((self.pbr as u32) << 16) | ((self.pc() + 1) as u32);
                    let offset = self.read_byte(offset_addr) as i8;
                    let new_pc = ((self.pc() + 2) as i32 + offset as i32) as u16;
                    self.set_pc(new_pc);
                    0 // PC already set
                } else {
                    2 // Skip branch, advance past opcode and offset
                }
            }
            // MERGE - Merge registers
            0x07 => {
                let r7 = self.regs[7];
                let r8 = self.regs[8];
                self.regs[self.dreg()] = (r7 & 0xFF00) | (r8 >> 8);
                self.update_zs_flags(self.regs[self.dreg()]);
                1
            }
            // MULT Rn (0x08-0x0F) - Multiply R6 by R(opcode - 0x08), i.e. R0-R7
            0x08..=0x0F => {
                let n = (opcode - 0x08) as usize;
                let r6 = self.regs[6] as i16;
                let rn = self.regs[n] as i16;
                self.mult_result = ((r6 as i32) * (rn as i32)) as u32;
                self.regs[4] = (self.mult_result & 0xFFFF) as u16;
                self.regs[5] = ((self.mult_result >> 16) & 0xFFFF) as u16;
                self.update_zs_flags(self.regs[4]);
                1
            }
            // TO Rn (0x10-0x1F) - Move to register
            0x10..=0x1F => {
                let n = (opcode & 0x0F) as usize;
                self.regs[n] = self.regs[self.sreg()];
                1
            }
            // WITH Rn (0x20-0x2F) - Set source/dest register
            0x20..=0x2F => {
                self.sreg_dreg = (opcode & 0x0F) as usize;
                1
            }
            // STW (Rn) (0x30-0x3B) - Store word to RAM
            0x30..=0x3B => {
                let n = (opcode & 0x0F) as usize;
                let addr = self.regs[n] as u32;
                let val = self.regs[self.sreg()];
                self.write_word(addr, val);
                1
            }
            // LOOP (0x3C) - Decrement R12 and branch if not zero
            0x3C => {
                self.regs[12] = self.regs[12].wrapping_sub(1);
                if self.regs[12] != 0 {
                    // Read signed byte offset from PC+1 (in current program bank), branch is relative to PC+2
                    let pc = self.pc();
                    let addr = ((self.pbr as u32) << 16) | (pc.wrapping_add(1) as u32);
                    let offset = self.read_byte(addr) as i8;
                    let new_pc = ((self.pc() + 2) as i32 + offset as i32) as u16;
                    self.set_pc(new_pc);
                    0 // PC already set
                } else {
                    2 // Skip branch, advance past opcode and offset
                }
            }
            // ALT1 (0x3D) - Set ALT1 prefix
            0x3D => {
                self.flags_alt1 = true;
                1
            }
            // ALT2 (0x3E) - Set ALT2 prefix
            0x3E => {
                self.flags_alt2 = true;
                1
            }
            // ALT3 (0x3F) - Set both ALT1 and ALT2
            0x3F => {
                self.flags_alt1 = true;
                self.flags_alt2 = true;
                1
            }
            // LDW (Rn) (0x40-0x4B) - Load word from RAM
            0x40..=0x4B => {
                let n = (opcode & 0x0F) as usize;
                let addr = self.regs[n] as u32;
                let val = self.read_word(addr);
                self.regs[self.dreg()] = val;
                self.update_zs_flags(val);
                1
            }
            // PLOT (0x4C) - Plot pixel
            0x4C => {
                // Plot pixel at (R1, R2) with color in COLR
                // This is a simplified implementation
                let x = self.regs[1];
                let y = self.regs[2];
                let color = self.colr;

                // Calculate pixel position in frame buffer
                // Simplified: assumes 256-pixel wide framebuffer
                let addr = (y as u32 * 256 + x as u32) as usize;
                if addr < self.ram.len() {
                    self.ram[addr] = color;
                }

                // Increment R1 for next pixel
                self.regs[1] = self.regs[1].wrapping_add(1);
                1
            }
            // RPIX (0x4D with ALT1, SuperFX2 only) - Read pixel
            0x4D if self.flags_alt1 && self.is_superfx2 => {
                // Read pixel at (R1, R2) into COLR
                let x = self.regs[1];
                let y = self.regs[2];

                // Calculate pixel position in frame buffer
                let addr = (y as u32 * 256 + x as u32) as usize;
                if addr < self.ram.len() {
                    self.colr = self.ram[addr];
                }

                self.flags_alt1 = false;
                1
            }
            // SWAP (0x4D) - Swap bytes
            0x4D => {
                let src = self.sreg();
                let val = self.regs[src];
                self.regs[src] = ((val & 0xFF) << 8) | ((val >> 8) & 0xFF);
                self.update_zs_flags(self.regs[src]);
                1
            }
            // COLOR (0x4E) - Set plot color
            0x4E => {
                if self.flags_alt1 {
                    // CMODE - Color mode
                    self.flags_alt1 = false;
                } else {
                    // COLOR - Set color from source register
                    self.colr = (self.regs[self.sreg()] & 0xFF) as u8;
                }
                1
            }
            // NOT (0x4F) - Bitwise NOT
            0x4F => {
                let src = self.sreg();
                self.regs[src] = !self.regs[src];
                self.update_zs_flags(self.regs[src]);
                1
            }
            // ADD Rn (0x50-0x5F) - Add register
            0x50..=0x5F => {
                let n = (opcode & 0x0F) as usize;
                let src = self.sreg();
                let (result, overflow) = self.regs[src].overflowing_add(self.regs[n]);
                self.flags_cy = overflow;
                self.flags_ov = ((self.regs[src] ^ result) & (self.regs[n] ^ result) & 0x8000) != 0;
                self.regs[src] = result;
                self.update_zs_flags(result);
                1
            }
            // SUB Rn (0x60-0x6F) - Subtract register
            0x60..=0x6F => {
                let n = (opcode & 0x0F) as usize;
                let src = self.sreg();
                let (result, overflow) = self.regs[src].overflowing_sub(self.regs[n]);
                self.flags_cy = overflow;
                self.flags_ov =
                    ((self.regs[src] ^ self.regs[n]) & (self.regs[src] ^ result) & 0x8000) != 0;
                self.regs[src] = result;
                self.update_zs_flags(result);
                1
            }
            // AND Rn (0x70-0x7F) - Bitwise AND
            0x70..=0x7F => {
                let n = (opcode & 0x0F) as usize;
                let src = self.sreg();
                self.regs[src] &= self.regs[n];
                self.update_zs_flags(self.regs[src]);
                1
            }
            // IWT Rn (immediate word to register)
            0x80..=0x8F if self.flags_alt1 => {
                let n = (opcode & 0x0F) as usize;
                // Read immediate bytes from (PBR << 16) | (PC+1) and (PBR << 16) | (PC+2)
                let low_addr = ((self.pbr as u32) << 16) | ((self.pc() + 1) as u32);
                let high_addr = ((self.pbr as u32) << 16) | ((self.pc() + 2) as u32);
                let low = self.read_byte(low_addr);
                let high = self.read_byte(high_addr);
                self.regs[n] = u16::from_le_bytes([low, high]);
                self.flags_alt1 = false;
                3
            }
            // OR Rn (0x80-0x8F) - Bitwise OR (when not ALT1)
            0x80..=0x8F => {
                let n = (opcode & 0x0F) as usize;
                let src = self.sreg();
                self.regs[src] |= self.regs[n];
                self.update_zs_flags(self.regs[src]);
                1
            }
            // INC Rn (0x90-0x9F) - Increment register
            0x90..=0x9F => {
                let n = (opcode & 0x0F) as usize;
                self.regs[n] = self.regs[n].wrapping_add(1);
                self.update_zs_flags(self.regs[n]);
                1
            }
            // DEC Rn (0xA0-0xAF) - Decrement register
            0xA0..=0xAF => {
                let n = (opcode & 0x0F) as usize;
                self.regs[n] = self.regs[n].wrapping_sub(1);
                self.update_zs_flags(self.regs[n]);
                1
            }
            // GETC (0xB0-0xBF) - Get byte from ROM
            0xB0..=0xBF => {
                // Read byte from ROM at (ROMBR:R14)
                let addr = ((self.rombr as u32) << 16) | (self.regs[14] as u32);
                self.rom_buffer = self.read_byte(addr);
                self.regs[14] = self.regs[14].wrapping_add(1);
                self.flags_r = true;
                1
            }
            // IBT Rn (immediate byte to register)
            0xC0..=0xCF if self.flags_alt1 => {
                let n = (opcode & 0x0F) as usize;
                // Read immediate byte from (PBR << 16) | (PC+1)
                let addr = ((self.pbr as u32) << 16) | ((self.pc() + 1) as u32);
                let val = self.read_byte(addr);
                self.regs[n] = val as u16;
                self.flags_alt1 = false;
                2
            }
            // XOR Rn (0xC0-0xCF) - Bitwise XOR (when not ALT1)
            0xC0..=0xCF => {
                let n = (opcode & 0x0F) as usize;
                let src = self.sreg();
                self.regs[src] ^= self.regs[n];
                self.update_zs_flags(self.regs[src]);
                1
            }
            // MOVE Rn (0xD0-0xDF) - Move from register (alternate encoding)
            0xD0..=0xDF => {
                let n = (opcode & 0x0F) as usize;
                let dest = self.dreg();
                self.regs[dest] = self.regs[n];
                self.update_zs_flags(self.regs[dest]);
                1
            }
            // LEA/LINK - Load effective address / Link
            0xE0..=0xEF if self.flags_alt1 => {
                // LINK - Link (used in function calls)
                let n = (opcode & 0x0F) as usize;
                self.regs[11] = self.regs[n];
                self.flags_alt1 = false;
                1
            }
            // LEA Rn (0xE0-0xEF) - Load effective address
            0xE0..=0xEF => {
                let n = (opcode & 0x0F) as usize;
                // LEA loads address of Rn into destination
                self.regs[self.dreg()] = self.regs[n];
                1
            }
            // UMULT/FMULT - Unsigned multiply / Fractional multiply
            0xF0..=0xFF if self.flags_alt1 && self.flags_alt2 => {
                // LMULT - Long multiply (returns full 32-bit result)
                let n = (opcode & 0x0F) as usize;
                let r6 = self.regs[6] as u32;
                let rn = self.regs[n] as u32;
                self.mult_result = r6 * rn;
                self.regs[4] = (self.mult_result & 0xFFFF) as u16;
                self.regs[5] = ((self.mult_result >> 16) & 0xFFFF) as u16;
                self.update_zs_flags(self.regs[4]);
                self.flags_alt1 = false;
                self.flags_alt2 = false;
                1
            }
            // UMULT Rn (0xF0-0xFF with ALT1) - Unsigned multiply
            0xF0..=0xFF if self.flags_alt1 => {
                let n = (opcode & 0x0F) as usize;
                let r6 = self.regs[6] as u32;
                let rn = self.regs[n] as u32;
                self.mult_result = r6 * rn;
                self.regs[4] = (self.mult_result & 0xFFFF) as u16;
                self.regs[5] = ((self.mult_result >> 16) & 0xFFFF) as u16;
                self.update_zs_flags(self.regs[4]);
                self.flags_alt1 = false;
                1
            }
            // FMULT Rn (0xF0-0xFF with ALT2) - Fractional multiply
            0xF0..=0xFF if self.flags_alt2 => {
                let n = (opcode & 0x0F) as usize;
                let r6 = self.regs[6] as i16;
                let rn = self.regs[n] as i16;
                // Fractional multiply: (a * b) >> 1
                let result = ((r6 as i32) * (rn as i32)) >> 1;
                self.mult_result = result as u32;
                self.regs[4] = (self.mult_result & 0xFFFF) as u16;
                self.regs[5] = ((self.mult_result >> 16) & 0xFFFF) as u16;
                self.update_zs_flags(self.regs[4]);
                self.flags_alt2 = false;
                1
            }
            // GETB/GETBL/GETBH Rn (0xF0-0xFF) - Get byte/low/high
            0xF0..=0xFF => {
                let n = (opcode & 0x0F) as usize;
                // GETB: Read byte from ROM buffer into register low byte
                self.regs[n] = (self.regs[n] & 0xFF00) | (self.rom_buffer as u16);
                1
            }
            // ASR/ASL/LSR - Shift operations with ALT flags
            0x03 if self.flags_alt1 => {
                // ASR - Arithmetic shift right
                let src = self.sreg();
                let val = self.regs[src] as i16;
                self.flags_cy = (val & 1) != 0;
                self.regs[src] = (val >> 1) as u16;
                self.update_zs_flags(self.regs[src]);
                self.flags_alt1 = false;
                1
            }
            // ROR - Rotate right
            0x04 if self.flags_alt1 => {
                let src = self.sreg();
                let val = self.regs[src];
                let cy = if self.flags_cy { 0x8000 } else { 0 };
                self.flags_cy = (val & 1) != 0;
                self.regs[src] = (val >> 1) | cy;
                self.update_zs_flags(self.regs[src]);
                self.flags_alt1 = false;
                1
            }
            // LOB - Get low byte
            0x0E if !self.flags_alt1 => {
                let src = self.sreg();
                self.regs[src] = self.regs[src] & 0xFF;
                self.update_zs_flags(self.regs[src]);
                1
            }
            // HIB - Get high byte
            0x0E if self.flags_alt1 => {
                let src = self.sreg();
                self.regs[src] = (self.regs[src] >> 8) & 0xFF;
                self.update_zs_flags(self.regs[src]);
                self.flags_alt1 = false;
                1
            }
            // BIC - Branch if carry
            0x07 if self.flags_alt1 => {
                if !self.flags_cy {
                    let offset_addr = ((self.pbr as u32) << 16) | ((self.pc() + 1) as u32);
                    let offset = self.read_byte(offset_addr) as i8;
                    let new_pc = ((self.pc() + 2) as i32 + offset as i32) as u16;
                    self.set_pc(new_pc);
                    self.flags_alt1 = false;
                    0
                } else {
                    self.flags_alt1 = false;
                    2
                }
            }
            // BVS/BVC - Branch on overflow
            0x06 if self.flags_alt1 => {
                // BVC - Branch if overflow clear
                if !self.flags_ov {
                    let offset_addr = ((self.pbr as u32) << 16) | ((self.pc() + 1) as u32);
                    let offset = self.read_byte(offset_addr) as i8;
                    let new_pc = ((self.pc() + 2) as i32 + offset as i32) as u16;
                    self.set_pc(new_pc);
                    self.flags_alt1 = false;
                    0
                } else {
                    self.flags_alt1 = false;
                    2
                }
            }
            // BEQ/BNE - Branch on zero
            0x05 if self.flags_alt1 => {
                // BNE - Branch if not zero
                if !self.flags_z {
                    let offset_addr = ((self.pbr as u32) << 16) | ((self.pc() + 1) as u32);
                    let offset = self.read_byte(offset_addr) as i8;
                    let new_pc = ((self.pc() + 2) as i32 + offset as i32) as u16;
                    self.set_pc(new_pc);
                    self.flags_alt1 = false;
                    0
                } else {
                    self.flags_alt1 = false;
                    2
                }
            }
            0x05 if self.flags_alt2 => {
                // BEQ - Branch if zero
                if self.flags_z {
                    let offset_addr = ((self.pbr as u32) << 16) | ((self.pc() + 1) as u32);
                    let offset = self.read_byte(offset_addr) as i8;
                    let new_pc = ((self.pc() + 2) as i32 + offset as i32) as u16;
                    self.set_pc(new_pc);
                    self.flags_alt2 = false;
                    0
                } else {
                    self.flags_alt2 = false;
                    2
                }
            }
            // BMI/BPL - Branch on sign
            0x06 if self.flags_alt2 => {
                // BMI - Branch if minus (sign set)
                if self.flags_s {
                    let offset_addr = ((self.pbr as u32) << 16) | ((self.pc() + 1) as u32);
                    let offset = self.read_byte(offset_addr) as i8;
                    let new_pc = ((self.pc() + 2) as i32 + offset as i32) as u16;
                    self.set_pc(new_pc);
                    self.flags_alt2 = false;
                    0
                } else {
                    self.flags_alt2 = false;
                    2
                }
            }
            // Unimplemented opcodes - treat as NOP
            _ => 1,
        };

        // Increment cycle counter
        self.cycles += 1;

        pc_increment
    }

    /// Run GSU for a number of cycles
    fn run(&mut self, target_cycles: u64) {
        let start_cycles = self.cycles;

        while self.flags_g && (self.cycles - start_cycles) < target_cycles {
            let pc = self.pc() as u32;
            let opcode = self.read_byte(pc);
            let pc_increment = self.execute_instruction(opcode);
            self.set_pc(self.pc().wrapping_add(pc_increment));
        }
    }

    /// Read from a GSU register (SNES CPU perspective)
    fn read_register(&mut self, addr: u32) -> u8 {
        let offset = (addr & 0xFF) as u8;

        match offset {
            // R0-R15 (low bytes at $00-$1E, high bytes at $01-$1F)
            0x00..=0x1F => {
                let reg = (offset / 2) as usize;
                if offset & 1 == 0 {
                    (self.regs[reg] & 0xFF) as u8
                } else {
                    ((self.regs[reg] >> 8) & 0xFF) as u8
                }
            }
            // SFR (Status/Flag Register) at $30-$31
            0x30 => {
                let flags = self.get_flags();
                flags.to_sfr_low()
            }
            0x31 => {
                let flags = self.get_flags();
                flags.to_sfr_high()
            }
            // PBR (Program Bank Register) at $34
            0x34 => self.pbr,
            // ROMBR (ROM Bank Register) at $36
            0x36 => self.rombr,
            // CFGR (Config Register) at $37
            0x37 => self.cfgr,
            // SCBR (Screen Base Register) at $38
            0x38 => self.scbr,
            // CLSR (Clock Select Register) at $39
            0x39 => self.clsr,
            // SCMR (Screen Mode Register) at $3A
            0x3A => self.scmr,
            // VCR (Version Code Register) at $3B - Return version 1.0
            0x3B => 0x10,
            // RAMBR (RAM Bank Register) at $3C
            0x3C => self.rambr,
            // CBR (Cache Base Register) at $3E-$3F
            0x3E => (self.cbr & 0xFF) as u8,
            0x3F => ((self.cbr >> 8) & 0xFF) as u8,
            // Default
            _ => 0,
        }
    }

    /// Write to a GSU register (SNES CPU perspective)
    fn write_register(&mut self, addr: u32, val: u8) {
        use emu_core::logging::{log, LogCategory, LogLevel};
        let offset = (addr & 0xFF) as u8;

        // Log important register writes
        if matches!(offset, 0x30..=0x31 | 0x34 | 0x36) {
            log(LogCategory::Bus, LogLevel::Debug, || {
                format!("SuperFX: Write register ${:02X} = ${:02X}", offset, val)
            });
        }

        match offset {
            // R0-R15 (low bytes at $00-$1E, high bytes at $01-$1F)
            0x00..=0x1F => {
                let reg = (offset / 2) as usize;
                if offset & 1 == 0 {
                    self.regs[reg] = (self.regs[reg] & 0xFF00) | (val as u16);
                } else {
                    self.regs[reg] = (self.regs[reg] & 0x00FF) | ((val as u16) << 8);
                }
            }
            // SFR (Status/Flag Register) at $30-$31
            0x30 => {
                let mut flags = self.get_flags();
                flags.set_sfr_low(val);
                self.set_flags(flags);
                // Writing to G flag starts/stops GSU
                // The GSU will now run asynchronously via the tick() method
                use emu_core::logging::{log, LogCategory, LogLevel};
                if self.flags_g {
                    log(LogCategory::Bus, LogLevel::Info, || {
                        format!(
                            "SuperFX: GO flag set, PC=${:04X}, starting execution",
                            self.pc()
                        )
                    });
                } else {
                    log(LogCategory::Bus, LogLevel::Info, || {
                        format!(
                            "SuperFX: GO flag cleared, PC=${:04X}, stopping execution",
                            self.pc()
                        )
                    });
                }
            }
            0x31 => {
                let mut flags = self.get_flags();
                flags.set_sfr_high(val);
                self.set_flags(flags);
            }
            // PBR (Program Bank Register) at $34
            0x34 => self.pbr = val,
            // ROMBR (ROM Bank Register) at $36
            0x36 => self.rombr = val,
            // CFGR (Config Register) at $37
            0x37 => self.cfgr = val,
            // SCBR (Screen Base Register) at $38
            0x38 => self.scbr = val,
            // CLSR (Clock Select Register) at $39
            0x39 => self.clsr = val,
            // SCMR (Screen Mode Register) at $3A
            0x3A => self.scmr = val,
            // RAMBR (RAM Bank Register) at $3C
            0x3C => self.rambr = val & 0x01,
            // CBR (Cache Base Register) at $3E-$3F
            0x3E => self.cbr = (self.cbr & 0xFF00) | (val as u16),
            0x3F => self.cbr = (self.cbr & 0x00FF) | ((val as u16) << 8),
            // POR (Plot Option Register) at $3D
            0x3D => self.por = val,
            // COLR (Color Register)
            0x40 => self.colr = val,
            // Default
            _ => {}
        }
    }
}

impl EnhancementChip for SuperFx {
    fn read(&mut self, addr: u32) -> u8 {
        let bank = (addr >> 16) as u8;
        let offset = (addr & 0xFFFF) as u16;

        // Register access at $3000-$32FF
        if matches!(offset, 0x3000..=0x32FF) {
            return self.read_register(addr);
        }

        // GSU RAM access at $700000-$73FFFF (128KB-256KB)
        if (0x70..=0x73).contains(&bank) {
            let ram_offset = (((bank - 0x70) as usize) << 16) | (offset as usize);
            if ram_offset < self.ram.len() {
                return self.ram[ram_offset];
            }
        }

        // SuperFX ROM passthrough for CPU access
        // Banks $00-$3F, $80-$BF at $8000-$FFFF access ROM
        if matches!(bank, 0x00..=0x3F | 0x80..=0xBF) && offset >= 0x8000 {
            let effective_bank = bank & 0x3F; // Remove mirror bit
            let rom_offset =
                ((effective_bank as usize) << 15) | ((offset as usize - 0x8000) & 0x7FFF);
            if rom_offset < self.rom.len() {
                return self.rom[rom_offset];
            }
        }

        // Banks $40-$5F for extended ROM (LoROM upper banks)
        if matches!(bank, 0x40..=0x5F) {
            let rom_offset = (((bank - 0x40) as usize) << 16) | (offset as usize);
            if rom_offset < self.rom.len() {
                return self.rom[rom_offset];
            }
        }

        0
    }

    fn write(&mut self, addr: u32, value: u8) {
        let bank = (addr >> 16) as u8;
        let offset = (addr & 0xFFFF) as u16;

        // Register access at $3000-$32FF
        if matches!(offset, 0x3000..=0x32FF) {
            self.write_register(addr, value);
            return;
        }

        // GSU RAM access at $700000-$71FFFF (simplified)
        if (0x70..=0x71).contains(&bank) {
            let ram_offset = (((bank - 0x70) as usize) << 16) | (offset as usize);
            if ram_offset < self.ram.len() {
                self.ram[ram_offset] = value;
            }
        }
    }

    fn reset(&mut self) {
        // Preserve ROM data and chip variant across reset
        let rom = self.rom.clone();
        let is_superfx2 = self.is_superfx2;

        // Reset all other internal state to defaults
        *self = Self::default();

        // Restore preserved fields
        self.rom = rom;
        self.is_superfx2 = is_superfx2;
    }

    fn chip_type(&self) -> ChipType {
        if self.is_superfx2 {
            ChipType::SuperFx2
        } else {
            ChipType::SuperFx
        }
    }

    fn save_state(&self) -> Result<String, String> {
        serde_json::to_string(self).map_err(|e| format!("Failed to serialize SuperFX state: {}", e))
    }

    fn load_state(&mut self, state: &str) -> Result<(), String> {
        // Deserialize and validate the state
        let loaded: SuperFx = serde_json::from_str(state)
            .map_err(|e| format!("Failed to deserialize SuperFX state: {}", e))?;

        // Basic sanity check
        if loaded.ram.is_empty() {
            return Err("Invalid SuperFX state: empty RAM".to_string());
        }

        // Replace current state
        *self = loaded;
        Ok(())
    }

    fn tick(&mut self, cycles: u64) {
        // Run SuperFX for the given number of cycles if the GO flag is set
        if self.flags_g {
            use emu_core::logging::{log, LogCategory, LogLevel};
            log(LogCategory::Bus, LogLevel::Trace, || {
                format!(
                    "SuperFX: Running {} cycles, PC=${:04X}, GO={}",
                    cycles,
                    self.pc(),
                    self.flags_g
                )
            });
            self.run(cycles);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_superfx_creation() {
        let sfx = SuperFx::new();
        assert_eq!(sfx.regs[0], 0);
        assert_eq!(sfx.pc(), 0);
        assert!(!sfx.flags_g);
    }

    #[test]
    fn test_register_read_write() {
        let mut sfx = SuperFx::new();

        // Write to R0 low byte
        sfx.write_register(0x3000, 0x42);
        assert_eq!(sfx.regs[0], 0x0042);

        // Write to R0 high byte
        sfx.write_register(0x3001, 0x12);
        assert_eq!(sfx.regs[0], 0x1242);

        // Read back
        assert_eq!(sfx.read_register(0x3000), 0x42);
        assert_eq!(sfx.read_register(0x3001), 0x12);
    }

    #[test]
    fn test_flags() {
        let mut sfx = SuperFx::new();

        sfx.flags_z = true;
        sfx.flags_cy = true;
        let flags = sfx.get_flags();
        let sfr = flags.to_sfr_low();
        assert_eq!(sfr & 0x02, 0x02); // Z flag
        assert_eq!(sfr & 0x04, 0x04); // CY flag

        let mut flags2 = Flags::default();
        flags2.set_sfr_low(sfr);
        assert!(flags2.z);
        assert!(flags2.cy);
    }

    #[test]
    fn test_stop_instruction() {
        let mut sfx = SuperFx::new();
        sfx.flags_g = true;

        sfx.execute_instruction(0x00); // STOP
        assert!(!sfx.flags_g);
    }

    #[test]
    fn test_add_instruction() {
        let mut sfx = SuperFx::new();
        sfx.regs[0] = 10;
        sfx.regs[1] = 20;

        sfx.execute_instruction(0x51); // ADD R1
        assert_eq!(sfx.regs[0], 30);
        assert!(!sfx.flags_z);
        assert!(!sfx.flags_s);
    }

    #[test]
    fn test_mult_instruction() {
        let mut sfx = SuperFx::new();
        sfx.regs[6] = 10;
        sfx.regs[2] = 20;

        sfx.execute_instruction(0x0A); // MULT R2
        assert_eq!(sfx.regs[4], 200); // Low word
        assert_eq!(sfx.regs[5], 0); // High word
    }

    #[test]
    fn test_color_instruction() {
        let mut sfx = SuperFx::new();
        sfx.regs[0] = 0x42;

        sfx.execute_instruction(0x4E); // COLOR
        assert_eq!(sfx.colr, 0x42);
    }

    #[test]
    fn test_plot_instruction() {
        let mut sfx = SuperFx::new();
        sfx.regs[1] = 10; // X
        sfx.regs[2] = 20; // Y
        sfx.colr = 0x12;

        sfx.execute_instruction(0x4C); // PLOT

        // Check pixel was written
        let addr = (20 * 256 + 10) as usize;
        assert_eq!(sfx.ram[addr], 0x12);

        // Check X was incremented
        assert_eq!(sfx.regs[1], 11);
    }

    #[test]
    fn test_enhancement_chip_trait_read_write() {
        let mut sfx = SuperFx::new();

        // Test register read/write through EnhancementChip trait
        // Write to R1 via SNES CPU (bank $00, offset $3002-$3003)
        sfx.write(0x003002, 0x34); // Low byte
        sfx.write(0x003003, 0x12); // High byte
        assert_eq!(sfx.read(0x003002), 0x34);
        assert_eq!(sfx.read(0x003003), 0x12);

        // Test GSU RAM access (bank $70)
        sfx.write(0x700100, 0xAB);
        assert_eq!(sfx.read(0x700100), 0xAB);
    }

    #[test]
    fn test_save_load_state() {
        let mut sfx = SuperFx::new();
        sfx.regs[0] = 0x1234;
        sfx.regs[15] = 0x8000; // PC
        sfx.colr = 0x42;
        sfx.flags_g = true;

        // Save state
        let state = sfx.save_state().expect("Save should succeed");
        assert!(!state.is_empty());

        // Modify chip
        sfx.regs[0] = 0;
        sfx.colr = 0;
        sfx.flags_g = false;

        // Load state
        sfx.load_state(&state).expect("Load should succeed");
        assert_eq!(sfx.regs[0], 0x1234);
        assert_eq!(sfx.regs[15], 0x8000);
        assert_eq!(sfx.colr, 0x42);
        assert!(sfx.flags_g);
    }

    #[test]
    fn test_reset() {
        let mut sfx = SuperFx::new();
        sfx.regs[0] = 0x1234;
        sfx.colr = 0x42;
        sfx.flags_g = true;

        sfx.reset();

        assert_eq!(sfx.regs[0], 0);
        assert_eq!(sfx.colr, 0);
        assert!(!sfx.flags_g);
    }

    #[test]
    fn test_chip_type() {
        let sfx = SuperFx::new();
        assert_eq!(sfx.chip_type(), ChipType::SuperFx);

        let sfx2 = SuperFx::new_superfx2();
        assert_eq!(sfx2.chip_type(), ChipType::SuperFx2);
    }

    #[test]
    fn test_with_instruction() {
        let mut sfx = SuperFx::new();
        assert_eq!(sfx.sreg(), 0); // Default R0

        sfx.execute_instruction(0x25); // WITH R5
        assert_eq!(sfx.sreg(), 5);
        assert_eq!(sfx.dreg(), 5);
    }

    #[test]
    fn test_getc_instruction() {
        let mut sfx = SuperFx::new();
        let test_rom = vec![0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0];
        sfx.set_rom(test_rom);

        sfx.rombr = 0x00;
        sfx.regs[14] = 0x0002; // Point to third byte

        sfx.execute_instruction(0xB0); // GETC

        assert_eq!(sfx.rom_buffer, 0x56);
        assert_eq!(sfx.regs[14], 0x0003); // R14 incremented
        assert!(sfx.flags_r); // ROM buffer valid flag set
    }

    #[test]
    fn test_rpix_instruction_superfx2() {
        let mut sfx2 = SuperFx::new_superfx2();

        // Set up a pixel in RAM
        sfx2.regs[1] = 10; // X
        sfx2.regs[2] = 20; // Y
        let addr = (20 * 256 + 10) as usize;
        sfx2.ram[addr] = 0x42;

        sfx2.flags_alt1 = true;
        sfx2.execute_instruction(0x4D); // RPIX (SWAP with ALT1 on SuperFX2)

        assert_eq!(sfx2.colr, 0x42);
        assert!(!sfx2.flags_alt1); // ALT1 should be cleared
    }

    #[test]
    fn test_branch_instructions_pc() {
        let mut sfx = SuperFx::new();
        // Set up a simple test ROM with BRA instruction
        let test_rom = vec![
            0x05, 0x02, // BRA +2 (skip next 2 bytes)
            0x00, 0x00, // (skipped)
            0x01, // NOP (target)
        ];
        sfx.set_rom(test_rom);

        sfx.set_pc(0); // Start at beginning

        // Execute BRA
        let bytes = sfx.execute_instruction(0x05);
        assert_eq!(bytes, 0); // Branch sets PC explicitly, returns 0

        // PC should be at offset 4 (PC was 0, +2 for instruction size, +2 for offset)
        assert_eq!(sfx.pc(), 4);
    }

    #[test]
    fn test_immediate_instructions_pc() {
        let mut sfx = SuperFx::new();
        // Test IWT (immediate word to register)
        let test_rom = vec![
            0x3D, // ALT1
            0x81, // IWT R1
            0x34, 0x12, // Immediate value 0x1234
        ];
        sfx.set_rom(test_rom);

        sfx.set_pc(0);
        sfx.execute_instruction(0x3D); // ALT1
        assert!(sfx.flags_alt1);

        sfx.set_pc(1);
        let bytes = sfx.execute_instruction(0x81); // IWT R1
        assert_eq!(bytes, 3); // Consumes 3 bytes (opcode + 2 immediate)
        assert_eq!(sfx.regs[1], 0x1234);
    }
}
