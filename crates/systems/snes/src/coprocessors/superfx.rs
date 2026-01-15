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

    /// 512-byte instruction cache
    #[serde(with = "BigArray")]
    cache: [u8; 512],

    /// GSU RAM (256 KB max, acts as frame buffer)
    ram: Vec<u8>,

    /// Pixel cache buffer for plot operations
    pixel_cache: [u8; 8],

    /// Current pixel cache position
    pixel_cache_pos: u8,

    /// ROM buffer for ROM reads
    rom_buffer: u8,

    /// Multiplication result (32-bit)
    mult_result: u32,

    /// Cycle counter for timing
    cycles: u64,
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
            pixel_cache: [0; 8],
            pixel_cache_pos: 0,
            rom_buffer: 0,
            mult_result: 0,
            cycles: 0,
        }
    }
}

impl SuperFx {
    /// Create a new SuperFX instance
    pub fn new() -> Self {
        Self::default()
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

    /// Get source register (R0 by default, or last used)
    fn sreg(&self) -> usize {
        // In SuperFX, the source register is typically R0
        // Some instructions modify this
        0
    }

    /// Get destination register (typically same as source)
    fn dreg(&self) -> usize {
        self.sreg()
    }

    /// Read a byte from GSU address space
    fn read_byte(&mut self, addr: u32) -> u8 {
        // GSU has its own address space separate from SNES
        // This is a simplified implementation
        let addr = addr as usize;
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

    /// Execute a single instruction
    fn execute_instruction(&mut self, opcode: u8) {
        // Simplified instruction execution
        // Full implementation would handle all 98 opcodes

        match opcode {
            // STOP - Stop GSU
            0x00 => {
                self.flags_g = false;
            }
            // NOP - No operation
            0x01 => {
                // Do nothing
            }
            // CACHE - Load cache
            0x02 => {
                // Simplified: just set flag
                self.flags_r = true;
            }
            // LSR - Logical shift right
            0x03 => {
                let src = self.sreg();
                let val = self.regs[src];
                self.flags_cy = (val & 1) != 0;
                self.regs[src] = val >> 1;
                self.update_zs_flags(self.regs[src]);
            }
            // ROL - Rotate left
            0x04 => {
                let src = self.sreg();
                let val = self.regs[src];
                let cy = if self.flags_cy { 1 } else { 0 };
                self.flags_cy = (val & 0x8000) != 0;
                self.regs[src] = (val << 1) | cy;
                self.update_zs_flags(self.regs[src]);
            }
            // BRA - Branch always (relative)
            0x05 => {
                // Read signed byte offset
                let offset = self.read_byte(self.pc() as u32) as i8;
                let new_pc = (self.pc() as i32 + offset as i32) as u16;
                self.set_pc(new_pc);
            }
            // BGE/BLT - Branch on sign flag
            0x06 => {
                if !self.flags_s {
                    let offset = self.read_byte(self.pc() as u32) as i8;
                    let new_pc = (self.pc() as i32 + offset as i32) as u16;
                    self.set_pc(new_pc);
                }
            }
            // MERGE - Merge registers
            0x07 => {
                let r7 = self.regs[7];
                let r8 = self.regs[8];
                self.regs[self.dreg()] = (r7 & 0xFF00) | (r8 >> 8);
                self.update_zs_flags(self.regs[self.dreg()]);
            }
            // MULT Rn (0x08-0x0F) - Multiply R6 by Rn
            0x08..=0x0F => {
                let n = (opcode & 0x0F) as usize;
                let r6 = self.regs[6] as i16;
                let rn = self.regs[n] as i16;
                self.mult_result = ((r6 as i32) * (rn as i32)) as u32;
                self.regs[4] = (self.mult_result & 0xFFFF) as u16;
                self.regs[5] = ((self.mult_result >> 16) & 0xFFFF) as u16;
                self.update_zs_flags(self.regs[4]);
            }
            // TO Rn (0x10-0x1F) - Move to register
            0x10..=0x1F => {
                let n = (opcode & 0x0F) as usize;
                self.regs[n] = self.regs[self.sreg()];
            }
            // WITH Rn (0x20-0x2F) - Set source/dest register
            0x20..=0x2F => {
                // This sets the default source/destination register
                // Simplified: we just track R0 by default
            }
            // STW (Rn) (0x30-0x3B) - Store word to RAM
            0x30..=0x3B => {
                let n = (opcode & 0x0F) as usize;
                let addr = self.regs[n] as u32;
                let val = self.regs[self.sreg()];
                self.write_word(addr, val);
            }
            // LOOP (0x3C) - Decrement R12 and branch if not zero
            0x3C => {
                self.regs[12] = self.regs[12].wrapping_sub(1);
                if self.regs[12] != 0 {
                    let offset = self.read_byte(self.pc() as u32) as i8;
                    let new_pc = (self.pc() as i32 + offset as i32) as u16;
                    self.set_pc(new_pc);
                }
            }
            // ALT1 (0x3D) - Set ALT1 prefix
            0x3D => {
                self.flags_alt1 = true;
            }
            // ALT2 (0x3E) - Set ALT2 prefix
            0x3E => {
                self.flags_alt2 = true;
            }
            // ALT3 (0x3F) - Set both ALT1 and ALT2
            0x3F => {
                self.flags_alt1 = true;
                self.flags_alt2 = true;
            }
            // LDW (Rn) (0x40-0x4B) - Load word from RAM
            0x40..=0x4B => {
                let n = (opcode & 0x0F) as usize;
                let addr = self.regs[n] as u32;
                let val = self.read_word(addr);
                self.regs[self.dreg()] = val;
                self.update_zs_flags(val);
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
            }
            // SWAP (0x4D) - Swap bytes
            0x4D => {
                let src = self.sreg();
                let val = self.regs[src];
                self.regs[src] = ((val & 0xFF) << 8) | ((val >> 8) & 0xFF);
                self.update_zs_flags(self.regs[src]);
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
            }
            // NOT (0x4F) - Bitwise NOT
            0x4F => {
                let src = self.sreg();
                self.regs[src] = !self.regs[src];
                self.update_zs_flags(self.regs[src]);
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
            }
            // AND Rn (0x70-0x7F) - Bitwise AND
            0x70..=0x7F => {
                let n = (opcode & 0x0F) as usize;
                let src = self.sreg();
                self.regs[src] &= self.regs[n];
                self.update_zs_flags(self.regs[src]);
            }
            // IWT Rn (immediate word to register)
            0x80..=0x8F if self.flags_alt1 => {
                let n = (opcode & 0x0F) as usize;
                let low = self.read_byte(self.pc() as u32);
                let high = self.read_byte((self.pc() + 1) as u32);
                self.regs[n] = u16::from_le_bytes([low, high]);
                self.set_pc(self.pc().wrapping_add(2));
                self.flags_alt1 = false;
            }
            // OR Rn (0x80-0x8F) - Bitwise OR (when not ALT1)
            0x80..=0x8F => {
                let n = (opcode & 0x0F) as usize;
                let src = self.sreg();
                self.regs[src] |= self.regs[n];
                self.update_zs_flags(self.regs[src]);
            }
            // INC Rn (0x90-0x9F) - Increment register
            0x90..=0x9F => {
                let n = (opcode & 0x0F) as usize;
                self.regs[n] = self.regs[n].wrapping_add(1);
                self.update_zs_flags(self.regs[n]);
            }
            // DEC Rn (0xA0-0xAF) - Decrement register
            0xA0..=0xAF => {
                let n = (opcode & 0x0F) as usize;
                self.regs[n] = self.regs[n].wrapping_sub(1);
                self.update_zs_flags(self.regs[n]);
            }
            // GETC (0xB0-0xBF) - Get byte from ROM
            0xB0..=0xBF => {
                // Simplified ROM read
                self.rom_buffer = 0;
                self.flags_r = true;
            }
            // IBT Rn (immediate byte to register)
            0xC0..=0xCF if self.flags_alt1 => {
                let n = (opcode & 0x0F) as usize;
                let val = self.read_byte(self.pc() as u32);
                self.regs[n] = val as u16;
                self.set_pc(self.pc().wrapping_add(1));
                self.flags_alt1 = false;
            }
            // XOR Rn (0xC0-0xCF) - Bitwise XOR (when not ALT1)
            0xC0..=0xCF => {
                let n = (opcode & 0x0F) as usize;
                let src = self.sreg();
                self.regs[src] ^= self.regs[n];
                self.update_zs_flags(self.regs[src]);
            }
            // MOVE Rn (0xD0-0xDF) - Move from register (alternate encoding)
            0xD0..=0xDF => {
                let n = (opcode & 0x0F) as usize;
                let dest = self.dreg();
                self.regs[dest] = self.regs[n];
                self.update_zs_flags(self.regs[dest]);
            }
            // Unimplemented opcodes
            _ => {
                // For now, treat as NOP
            }
        }

        // Clear ALT flags after instruction (if not explicitly set)
        if opcode != 0x3D && opcode != 0x3E && opcode != 0x3F {
            self.flags_alt1 = false;
            self.flags_alt2 = false;
        }

        // Increment PC
        self.set_pc(self.pc().wrapping_add(1));

        // Increment cycle counter
        self.cycles += 1;
    }

    /// Run GSU for a number of cycles
    fn run(&mut self, target_cycles: u64) {
        let start_cycles = self.cycles;

        while self.flags_g && (self.cycles - start_cycles) < target_cycles {
            let pc = self.pc() as u32;
            let opcode = self.read_byte(pc);
            self.execute_instruction(opcode);
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
        let offset = (addr & 0xFF) as u8;

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
                if self.flags_g {
                    // Start GSU execution
                    self.run(1000); // Run for some cycles
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

        // GSU RAM access at $700000-$71FFFF (simplified)
        if (0x70..=0x71).contains(&bank) {
            let ram_offset = (((bank - 0x70) as usize) << 16) | (offset as usize);
            if ram_offset < self.ram.len() {
                return self.ram[ram_offset];
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
        *self = Self::default();
    }

    fn chip_type(&self) -> ChipType {
        ChipType::SuperFx
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
        sfx.regs[8] = 20;

        sfx.execute_instruction(0x08); // MULT R8
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
}
