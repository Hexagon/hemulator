//! SuperFX (GSU) Graphics Coprocessor Implementation
//!
//! The SuperFX is a custom 16-bit RISC graphics coprocessor used in games like
//! Star Fox, Yoshi's Island, and Doom. It provides:
//! - 16 general-purpose 16-bit registers (R0-R15)
//! - 512-byte instruction cache for fast execution
//! - Pixel plotting and graphics operations
//! - Up to 21.48 MHz operation (GSU-2)
//!
//! ## Opcode layout (from fullsnes.htm)
//! - 0x: STOP, NOP, CACHE, LSR, ROL, BRA, BGE, BLT, BNE..BVS
//! - 1x: TO R0-R15 (or MOVE Rd,Rs when B flag set)
//! - 2x: WITH R0-R15 (or MOVES Rd,Rs when B flag set)
//! - 3x: STW R0-R11, LOOP, ALT1, ALT2, ALT3
//! - 4x: LDW R0-R11, PLOT, SWAP, COLOR, NOT
//! - 5x: ADD R0-R15
//! - 6x: SUB R0-R15
//! - 7x: MERGE, AND R1-R15
//! - 8x: MULT R0-R15
//! - 9x: SBK, LINK#1-4, SEX, ASR/DIV2, ROR, JMP/LJMP R8-R13, LOB, FMULT/LMULT
//! - Ax: IBT R0-R15
//! - Bx: FROM R0-R15
//! - Cx: HIB, OR R1-R15
//! - Dx: INC R0-R14, GETC
//! - Ex: DEC R0-R14, GETB/GETBH/GETBL/GETBS
//! - Fx: IWT R0-R15
//!
//! ## ALT variants (prefix byte changes meaning of following opcode)
//! - ALT1 (0x3D): STB, LDB, RPIX, CMODE, ADC, SBC, UMULT, LINK, DIV2, LJMP, GETBS, SM
//! - ALT2 (0x3E): ADD#n, SUB#n, AND#n, MULT#n, OR#n, INC#n, DEC#n, GETBL, RAMB, LMS, SMS
//! - ALT3 (0x3F): ADC#n, CMP, BIC#n, UMULT#n, XOR#n, GETBH, ROMB, LM, SM
//!
//! ## References
//! - https://problemkaputt.de/fullsnes.htm#snescartgsugraphicssupportunit
//! - https://snes.nesdev.org/wiki/Super_FX
//! - https://wiki.superfamicom.org/super-fx-opcode-matrix

use super::{ChipType, EnhancementChip};
use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;

/// SuperFX Graphics Support Unit
#[derive(Clone, Serialize, Deserialize)]
pub struct SuperFx {
    /// 16 general-purpose registers (R0-R15)
    /// R14 = ROM address pointer, R15 = Program Counter
    regs: [u16; 16],

    /// Status/Flag Register fields
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
    /// RAM Bank Register (RAMBR) - 1-bit for RAM access (bank 70h/71h)
    rambr: u8,
    /// Cache Base Register (CBR) - Cache region base (upper 12 bits)
    cbr: u16,
    /// Screen Base Register (SCBR) - Pixel buffer base in 1KB units
    scbr: u8,
    /// Color Register (COLR) - Current plot color
    colr: u8,
    /// Plot Option Register (POR)
    por: u8,
    /// Screen Mode Register (SCMR)
    scmr: u8,
    /// Config Register (CFGR) - IRQ/multiplier speed config
    cfgr: u8,
    /// Clock Select Register (CLSR) - 0=10.7MHz, 1=21.4MHz
    clsr: u8,

    /// 512-byte instruction cache
    #[serde(with = "BigArray")]
    cache: [u8; 512],
    /// Cache line valid flags (32 lines of 16 bytes each)
    cache_valid: [bool; 32],

    /// GSU RAM (128 KB default)
    ram: Vec<u8>,
    /// ROM data
    rom: Vec<u8>,
    /// ROM read buffer (prefetched from [ROMBR:R14])
    rom_buffer: u8,
    /// Whether the ROM buffer is being filled
    rom_buffer_wait: bool,

    /// Source register index (set by FROM prefix, default R0)
    sreg: usize,
    /// Destination register index (set by TO prefix, default R0)
    dreg: usize,

    /// Last RAM address used (for SBK writeback)
    last_ram_addr: u16,

    /// Cycle counter for timing
    cycles: u64,

    /// Pixel cache - primary (8 pixels being drawn)
    pixel_cache: [u8; 8],
    /// Number of valid pixels in cache
    pixel_cache_count: u8,
    /// Pixel cache X offset
    pixel_cache_x: u16,
    /// Pixel cache Y coordinate
    pixel_cache_y: u16,

    /// Chip variant (false = SuperFX/GSU-1, true = SuperFX2/GSU-2)
    is_superfx2: bool,

    /// R15 latch for SNES CPU writes (low byte written first, applied on high byte write)
    r15_latch: u8,
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
            cache_valid: [false; 32],
            ram: vec![0; 128 * 1024],
            rom: Vec::new(),
            rom_buffer: 0,
            rom_buffer_wait: false,
            sreg: 0,
            dreg: 0,
            last_ram_addr: 0,
            cycles: 0,
            pixel_cache: [0; 8],
            pixel_cache_count: 0,
            pixel_cache_x: 0,
            pixel_cache_y: 0,
            is_superfx2: false,
            r15_latch: 0,
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

    /// Get program counter (R15)
    fn pc(&self) -> u16 {
        self.regs[15]
    }

    /// Get current color depth in bits per pixel (from SCMR.MD)
    fn color_depth(&self) -> u8 {
        match self.scmr & 0x03 {
            0 => 2, // 4-color
            1 => 4, // 16-color
            3 => 8, // 256-color
            _ => 2, // Reserved, treat as 2bpp
        }
    }

    /// Get screen height from SCMR bits 2-3
    #[allow(dead_code)]
    fn screen_height(&self) -> u16 {
        match (self.scmr >> 2) & 0x03 {
            0 => 128,
            1 => 160,
            2 => 192,
            3 => 256, // OBJ mode
            _ => 128,
        }
    }

    // ── Memory access ──────────────────────────────────────────────

    /// Fetch opcode byte from ROM/cache at PBR:addr
    fn fetch_byte(&self, addr: u32) -> u8 {
        // Check cache first
        let pc16 = addr as u16;
        let cache_offset = pc16.wrapping_sub(self.cbr) as usize;
        if cache_offset < 512 {
            let line = cache_offset / 16;
            if self.cache_valid[line] {
                return self.cache[cache_offset];
            }
        }
        // Fall through to ROM
        self.read_rom(addr)
    }

    /// Read a byte from ROM (flat 24-bit addressing)
    fn read_rom(&self, addr: u32) -> u8 {
        if self.rom.is_empty() {
            return 0;
        }
        let offset = (addr as usize) & 0x1FFFFF; // 2MB ROM max
        self.rom.get(offset % self.rom.len()).copied().unwrap_or(0)
    }

    /// Read a byte from GSU RAM (with RAMBR bank)
    fn read_ram_byte(&self, addr: u16) -> u8 {
        let bank = (self.rambr as usize) & 0x01;
        let ram_offset = (bank << 16) | (addr as usize);
        self.ram.get(ram_offset).copied().unwrap_or(0)
    }

    /// Write a byte to GSU RAM (with RAMBR bank)
    fn write_ram_byte(&mut self, addr: u16, val: u8) {
        let bank = (self.rambr as usize) & 0x01;
        let ram_offset = (bank << 16) | (addr as usize);
        if ram_offset < self.ram.len() {
            self.ram[ram_offset] = val;
        }
    }

    /// Read a word from GSU RAM (word-aligned: addr & !1)
    fn read_ram_word(&mut self, addr: u16) -> u16 {
        let aligned = addr & !1;
        let lo = self.read_ram_byte(aligned);
        let hi = self.read_ram_byte(aligned.wrapping_add(1));
        u16::from_le_bytes([lo, hi])
    }

    /// Write a word to GSU RAM (word-aligned)
    fn write_ram_word(&mut self, addr: u16, val: u16) {
        let aligned = addr & !1;
        let [lo, hi] = val.to_le_bytes();
        self.write_ram_byte(aligned, lo);
        self.write_ram_byte(aligned.wrapping_add(1), hi);
    }

    // ── Flag helpers ───────────────────────────────────────────────

    fn update_zs(&mut self, val: u16) {
        self.flags_z = val == 0;
        self.flags_s = (val & 0x8000) != 0;
    }

    /// Reset prefix/register state after a non-prefix opcode
    fn reset_prefix(&mut self) {
        self.flags_alt1 = false;
        self.flags_alt2 = false;
        self.flags_b = false;
        self.sreg = 0;
        self.dreg = 0;
    }

    /// SFR low byte (bits 0-7)
    fn sfr_low(&self) -> u8 {
        let mut v = 0u8;
        // Bit 0: unused (always 0)
        if self.flags_z {
            v |= 0x02;
        }
        if self.flags_cy {
            v |= 0x04;
        }
        if self.flags_s {
            v |= 0x08;
        }
        if self.flags_ov {
            v |= 0x10;
        }
        if self.flags_g {
            v |= 0x20;
        }
        if self.flags_r {
            v |= 0x40;
        }
        // Bit 7: unused (always 0)
        v
    }

    /// SFR high byte (bits 8-15)
    fn sfr_high(&self) -> u8 {
        let mut v = 0u8;
        if self.flags_alt1 {
            v |= 0x01;
        }
        if self.flags_alt2 {
            v |= 0x02;
        }
        if self.flags_il {
            v |= 0x04;
        }
        if self.flags_ih {
            v |= 0x08;
        }
        if self.flags_b {
            v |= 0x10;
        }
        // Bits 13-14: unused (always 0)
        if self.flags_irq {
            v |= 0x80;
        }
        v
    }

    fn set_sfr_low(&mut self, val: u8) {
        self.flags_z = val & 0x02 != 0;
        self.flags_cy = val & 0x04 != 0;
        self.flags_s = val & 0x08 != 0;
        self.flags_ov = val & 0x10 != 0;
        self.flags_g = val & 0x20 != 0;
        // R flag bit 6 is read-only, ignore
    }

    fn set_sfr_high(&mut self, _val: u8) {
        // ALT/IL/IH/B bits 8-12 are mostly read-only from SNES side
        // IRQ bit 15 is cleared on read
    }

    // ── Pixel plot/read (bitplane format) ──────────────────────────

    /// Flush pixel cache to RAM (bitplane format)
    fn flush_pixel_cache(&mut self) {
        if self.pixel_cache_count == 0 {
            return;
        }

        let bpp = self.color_depth();
        let base = (self.scbr as u32) * 0x400;
        let x = self.pixel_cache_x & !7; // aligned to 8-pixel boundary
        let y = self.pixel_cache_y;

        // Calculate character-cell address
        // Screen is organized as rows of 8-pixel-wide character cells
        let screen_width_chars = if (self.scmr >> 2) & 0x03 == 3 {
            // OBJ mode uses a fixed 32-character width (256 pixels)
            32u32
        } else {
            // Normal modes: width determined by SCMR
            match (self.scmr >> 2) & 0x03 {
                0 => 32, // 128 height = 32 chars wide (by convention)
                1 => 32, // 160 height
                2 => 32, // 192 height
                _ => 32,
            }
        };

        let char_x = (x / 8) as u32;
        let char_y = (y / 8) as u32;
        let char_row = y & 7; // row within character (0-7)

        // Offset within the character grid
        let char_offset = (char_y * screen_width_chars + char_x) * (8 * bpp as u32);
        let row_base = base + char_offset + (char_row as u32) * 2;

        // Write bitplane data
        for bit in 0..bpp {
            let plane_offset = match bit {
                0 => 0u32,
                1 => 1,
                2 => 16,
                3 => 17,
                4 => 32,
                5 => 33,
                6 => 48,
                7 => 49,
                _ => 0,
            };

            let addr = row_base + plane_offset;
            let mut byte = 0u8;
            for px in 0..8 {
                if (self.pixel_cache[px] >> bit) & 1 != 0 {
                    byte |= 0x80 >> px;
                }
            }

            // Merge with existing data if not all pixels were written
            if self.pixel_cache_count < 8 {
                let existing = self.read_ram_byte(addr as u16);
                // Keep bits for pixels not in cache
                let mask = !(0xFF_u8 << (8 - self.pixel_cache_count));
                byte = (existing & mask) | (byte & !mask);
            }

            self.write_ram_byte(addr as u16, byte);
        }

        self.pixel_cache_count = 0;
    }

    /// Plot a pixel at (R1, R2) with current COLR, then increment R1
    fn plot_pixel(&mut self) {
        let x = self.regs[1];
        let y = self.regs[2];
        let color = self.colr;

        // Check transparency (POR bit 0)
        let bpp = self.color_depth();
        let transparent_mask = match bpp {
            2 => 0x03,
            4 => 0x0F,
            8 => {
                if self.por & 0x08 != 0 {
                    // Freeze-High
                    0x0F
                } else {
                    0xFF
                }
            }
            _ => 0x03,
        };

        if self.por & 0x01 == 0 && (color & transparent_mask) == 0 {
            // Transparent - skip plotting, just increment X
            self.regs[1] = self.regs[1].wrapping_add(1);
            return;
        }

        // Dither support (POR bit 1)
        let actual_color = if self.por & 0x02 != 0 {
            // Dither: alternate between COLR and transparent based on (x XOR y) bit 0
            if (x ^ y) & 1 != 0 {
                color
            } else {
                0 // transparent
            }
        } else {
            color
        };

        // Check if we need to flush the cache (different row or X block)
        let x_block = x & !7;
        if self.pixel_cache_count > 0 && (y != self.pixel_cache_y || x_block != self.pixel_cache_x)
        {
            self.flush_pixel_cache();
        }

        self.pixel_cache_x = x_block;
        self.pixel_cache_y = y;
        let px_idx = (x & 7) as usize;
        if px_idx < 8 {
            self.pixel_cache[px_idx] = actual_color;
            if self.pixel_cache_count <= px_idx as u8 {
                self.pixel_cache_count = px_idx as u8 + 1;
            }
        }

        // Auto-increment X
        self.regs[1] = self.regs[1].wrapping_add(1);
    }

    /// Read pixel at (R1, R2) from RAM (RPIX, ALT1 0x4C)
    fn read_pixel(&mut self) -> u8 {
        // Flush any pending pixel cache first
        self.flush_pixel_cache();

        let x = self.regs[1];
        let y = self.regs[2];
        let bpp = self.color_depth();
        let base = (self.scbr as u32) * 0x400;
        let screen_width_chars = 32u32;

        let char_x = (x / 8) as u32;
        let char_y = (y / 8) as u32;
        let char_row = y & 7;
        let px_bit = 7 - (x & 7); // bit position within byte (MSB first)

        let char_offset = (char_y * screen_width_chars + char_x) * (8 * bpp as u32);
        let row_base = base + char_offset + (char_row as u32) * 2;

        let mut color = 0u8;
        for bit in 0..bpp {
            let plane_offset = match bit {
                0 => 0u32,
                1 => 1,
                2 => 16,
                3 => 17,
                4 => 32,
                5 => 33,
                6 => 48,
                7 => 49,
                _ => 0,
            };
            let byte = self.read_ram_byte((row_base + plane_offset) as u16);
            if (byte >> px_bit) & 1 != 0 {
                color |= 1 << bit;
            }
        }
        color
    }

    /// Prefetch ROM buffer from [ROMBR:R14] (used by GETB/GETC)
    fn prefetch_rom_buffer(&mut self) {
        let addr = ((self.rombr as u32) << 16) | (self.regs[14] as u32);
        self.rom_buffer = self.read_rom(addr);
        self.flags_r = false; // R=0 means read complete
    }

    // ── Instruction execution ──────────────────────────────────────

    /// Execute a single instruction, returns the number of cycles consumed
    fn execute_instruction(&mut self, opcode: u8) -> u32 {
        let alt1 = self.flags_alt1;
        let alt2 = self.flags_alt2;
        let alt = (alt1 as u8) | ((alt2 as u8) << 1); // 0=none, 1=ALT1, 2=ALT2, 3=ALT3
        let b_flag = self.flags_b;

        let cycles: u32;
        match opcode {
            // ── 0x00 STOP ──
            0x00 => {
                self.flags_g = false;
                self.flags_irq = true;
                // Flush pixel cache on stop
                self.flush_pixel_cache();
                self.regs[15] = self.regs[15].wrapping_add(1);
                self.reset_prefix();
                cycles = 1;
            }

            // ── 0x01 NOP ──
            0x01 => {
                self.regs[15] = self.regs[15].wrapping_add(1);
                self.reset_prefix();
                cycles = 1;
            }

            // ── 0x02 CACHE ──
            0x02 => {
                let pc = self.regs[15];
                let new_cbr = pc & 0xFFF0;
                if self.cbr != new_cbr {
                    self.cbr = new_cbr;
                    self.cache_valid = [false; 32];
                }
                self.regs[15] = self.regs[15].wrapping_add(1);
                self.reset_prefix();
                cycles = 1;
            }

            // ── 0x03 LSR / (no alt variant at 0x03) ──
            0x03 => {
                let src = self.regs[self.sreg];
                self.flags_cy = src & 1 != 0;
                let result = src >> 1;
                self.regs[self.dreg] = result;
                self.update_zs(result);
                self.regs[15] = self.regs[15].wrapping_add(1);
                self.reset_prefix();
                cycles = 1;
            }

            // ── 0x04 ROL ──
            0x04 => {
                let src = self.regs[self.sreg];
                let old_cy = self.flags_cy as u16;
                self.flags_cy = src & 0x8000 != 0;
                let result = (src << 1) | old_cy;
                self.regs[self.dreg] = result;
                self.update_zs(result);
                self.regs[15] = self.regs[15].wrapping_add(1);
                self.reset_prefix();
                cycles = 1;
            }

            // ── 0x05 BRA ──
            0x05 => {
                let addr = ((self.pbr as u32) << 16) | (self.regs[15].wrapping_add(1) as u32);
                let offset = self.fetch_byte(addr) as i8;
                self.regs[15] = (self.regs[15].wrapping_add(2) as i32 + offset as i32) as u16;
                // Branches do NOT reset prefix
                cycles = 2;
            }

            // ── 0x06 BGE (if (S XOR V)=0) ──
            0x06 => {
                let addr = ((self.pbr as u32) << 16) | (self.regs[15].wrapping_add(1) as u32);
                let offset = self.fetch_byte(addr) as i8;
                if self.flags_s == self.flags_ov {
                    self.regs[15] = (self.regs[15].wrapping_add(2) as i32 + offset as i32) as u16;
                } else {
                    self.regs[15] = self.regs[15].wrapping_add(2);
                }
                cycles = 2;
            }

            // ── 0x07 BLT (if (S XOR V)=1) ──
            0x07 => {
                let addr = ((self.pbr as u32) << 16) | (self.regs[15].wrapping_add(1) as u32);
                let offset = self.fetch_byte(addr) as i8;
                if self.flags_s != self.flags_ov {
                    self.regs[15] = (self.regs[15].wrapping_add(2) as i32 + offset as i32) as u16;
                } else {
                    self.regs[15] = self.regs[15].wrapping_add(2);
                }
                cycles = 2;
            }

            // ── 0x08 BNE ──
            0x08 => {
                let addr = ((self.pbr as u32) << 16) | (self.regs[15].wrapping_add(1) as u32);
                let offset = self.fetch_byte(addr) as i8;
                if !self.flags_z {
                    self.regs[15] = (self.regs[15].wrapping_add(2) as i32 + offset as i32) as u16;
                } else {
                    self.regs[15] = self.regs[15].wrapping_add(2);
                }
                cycles = 2;
            }

            // ── 0x09 BEQ ──
            0x09 => {
                let addr = ((self.pbr as u32) << 16) | (self.regs[15].wrapping_add(1) as u32);
                let offset = self.fetch_byte(addr) as i8;
                if self.flags_z {
                    self.regs[15] = (self.regs[15].wrapping_add(2) as i32 + offset as i32) as u16;
                } else {
                    self.regs[15] = self.regs[15].wrapping_add(2);
                }
                cycles = 2;
            }

            // ── 0x0A BPL ──
            0x0A => {
                let addr = ((self.pbr as u32) << 16) | (self.regs[15].wrapping_add(1) as u32);
                let offset = self.fetch_byte(addr) as i8;
                if !self.flags_s {
                    self.regs[15] = (self.regs[15].wrapping_add(2) as i32 + offset as i32) as u16;
                } else {
                    self.regs[15] = self.regs[15].wrapping_add(2);
                }
                cycles = 2;
            }

            // ── 0x0B BMI ──
            0x0B => {
                let addr = ((self.pbr as u32) << 16) | (self.regs[15].wrapping_add(1) as u32);
                let offset = self.fetch_byte(addr) as i8;
                if self.flags_s {
                    self.regs[15] = (self.regs[15].wrapping_add(2) as i32 + offset as i32) as u16;
                } else {
                    self.regs[15] = self.regs[15].wrapping_add(2);
                }
                cycles = 2;
            }

            // ── 0x0C BCC ──
            0x0C => {
                let addr = ((self.pbr as u32) << 16) | (self.regs[15].wrapping_add(1) as u32);
                let offset = self.fetch_byte(addr) as i8;
                if !self.flags_cy {
                    self.regs[15] = (self.regs[15].wrapping_add(2) as i32 + offset as i32) as u16;
                } else {
                    self.regs[15] = self.regs[15].wrapping_add(2);
                }
                cycles = 2;
            }

            // ── 0x0D BCS ──
            0x0D => {
                let addr = ((self.pbr as u32) << 16) | (self.regs[15].wrapping_add(1) as u32);
                let offset = self.fetch_byte(addr) as i8;
                if self.flags_cy {
                    self.regs[15] = (self.regs[15].wrapping_add(2) as i32 + offset as i32) as u16;
                } else {
                    self.regs[15] = self.regs[15].wrapping_add(2);
                }
                cycles = 2;
            }

            // ── 0x0E BVC ──
            0x0E => {
                let addr = ((self.pbr as u32) << 16) | (self.regs[15].wrapping_add(1) as u32);
                let offset = self.fetch_byte(addr) as i8;
                if !self.flags_ov {
                    self.regs[15] = (self.regs[15].wrapping_add(2) as i32 + offset as i32) as u16;
                } else {
                    self.regs[15] = self.regs[15].wrapping_add(2);
                }
                cycles = 2;
            }

            // ── 0x0F BVS ──
            0x0F => {
                let addr = ((self.pbr as u32) << 16) | (self.regs[15].wrapping_add(1) as u32);
                let offset = self.fetch_byte(addr) as i8;
                if self.flags_ov {
                    self.regs[15] = (self.regs[15].wrapping_add(2) as i32 + offset as i32) as u16;
                } else {
                    self.regs[15] = self.regs[15].wrapping_add(2);
                }
                cycles = 2;
            }

            // ── 0x10-0x1F: TO Rn / MOVE Rd,Rs (when B flag set) ──
            0x10..=0x1F => {
                let n = (opcode & 0x0F) as usize;
                if b_flag {
                    // MOVE Rd,Rs: Rd=Rs (no flags)
                    self.regs[n] = self.regs[self.sreg];
                    self.regs[15] = self.regs[15].wrapping_add(1);
                    self.reset_prefix();
                } else {
                    // TO Rn: set destination register for next opcode
                    self.dreg = n;
                    self.regs[15] = self.regs[15].wrapping_add(1);
                    // TO does NOT reset prefix (it IS a prefix)
                }
                cycles = 1;
            }

            // ── 0x20-0x2F: WITH Rn / MOVES Rd,Rs (when B flag set) ──
            0x20..=0x2F => {
                let n = (opcode & 0x0F) as usize;
                if b_flag {
                    // MOVES Rd,Rs: Rd=Rs (sets OV to bit7 of result, sets S,Z)
                    let val = self.regs[self.sreg];
                    self.regs[n] = val;
                    self.flags_ov = val & 0x80 != 0;
                    self.update_zs(val);
                    self.regs[15] = self.regs[15].wrapping_add(1);
                    self.reset_prefix();
                } else {
                    // WITH Rn: set source AND destination, set B flag
                    self.sreg = n;
                    self.dreg = n;
                    self.flags_b = true;
                    self.regs[15] = self.regs[15].wrapping_add(1);
                    // WITH does NOT reset prefix
                }
                cycles = 1;
            }

            // ── 0x30-0x3B: STW (Rn) / STB (Rn) [ALT1] ──
            0x30..=0x3B => {
                let n = (opcode & 0x0F) as usize;
                let addr = self.regs[n];
                self.last_ram_addr = addr;
                if alt1 {
                    // STB: store byte
                    let val = (self.regs[self.sreg] & 0xFF) as u8;
                    self.write_ram_byte(addr, val);
                } else {
                    // STW: store word
                    let val = self.regs[self.sreg];
                    self.write_ram_word(addr, val);
                }
                self.regs[15] = self.regs[15].wrapping_add(1);
                self.reset_prefix();
                cycles = 1;
            }

            // ── 0x3C LOOP ──
            0x3C => {
                self.regs[12] = self.regs[12].wrapping_sub(1);
                self.update_zs(self.regs[12]);
                if !self.flags_z {
                    self.regs[15] = self.regs[13]; // jump to R13
                } else {
                    self.regs[15] = self.regs[15].wrapping_add(1);
                }
                self.reset_prefix();
                cycles = 1;
            }

            // ── 0x3D ALT1 ──
            0x3D => {
                self.flags_alt1 = true;
                self.regs[15] = self.regs[15].wrapping_add(1);
                // ALT1 is a prefix, does NOT reset other state
                cycles = 1;
            }

            // ── 0x3E ALT2 ──
            0x3E => {
                self.flags_alt2 = true;
                self.regs[15] = self.regs[15].wrapping_add(1);
                cycles = 1;
            }

            // ── 0x3F ALT3 ──
            0x3F => {
                self.flags_alt1 = true;
                self.flags_alt2 = true;
                self.regs[15] = self.regs[15].wrapping_add(1);
                cycles = 1;
            }

            // ── 0x40-0x4B: LDW (Rn) / LDB (Rn) [ALT1] ──
            0x40..=0x4B => {
                let n = (opcode & 0x0F) as usize;
                let addr = self.regs[n];
                self.last_ram_addr = addr;
                if alt1 {
                    // LDB: load byte (zero-extended)
                    let val = self.read_ram_byte(addr) as u16;
                    self.regs[self.dreg] = val;
                    self.update_zs(val);
                } else {
                    // LDW: load word
                    let val = self.read_ram_word(addr);
                    self.regs[self.dreg] = val;
                    self.update_zs(val);
                }
                self.regs[15] = self.regs[15].wrapping_add(1);
                self.reset_prefix();
                cycles = 1;
            }

            // ── 0x4C: PLOT / RPIX [ALT1] ──
            0x4C => {
                if alt1 {
                    // RPIX: read pixel at (R1,R2) into Dreg
                    let color = self.read_pixel();
                    self.regs[self.dreg] = color as u16;
                    self.update_zs(color as u16);
                    cycles = 5; // RPIX is slow (flushes pixel cache)
                } else {
                    // PLOT: plot pixel at (R1,R2), R1++
                    self.plot_pixel();
                    cycles = 1;
                }
                self.regs[15] = self.regs[15].wrapping_add(1);
                self.reset_prefix();
            }

            // ── 0x4D: SWAP / RPIX fallback ──
            0x4D => {
                let src = self.regs[self.sreg];
                let result = src.rotate_right(8);
                self.regs[self.dreg] = result;
                self.update_zs(result);
                self.regs[15] = self.regs[15].wrapping_add(1);
                self.reset_prefix();
                cycles = 1;
            }

            // ── 0x4E: COLOR / CMODE [ALT1] ──
            0x4E => {
                if alt1 {
                    // CMODE: set Plot Option Register from source register
                    self.por = (self.regs[self.sreg] & 0x1F) as u8;
                } else {
                    // COLOR: set COLR from source register
                    self.colr = (self.regs[self.sreg] & 0xFF) as u8;
                }
                self.regs[15] = self.regs[15].wrapping_add(1);
                self.reset_prefix();
                cycles = 1;
            }

            // ── 0x4F: NOT ──
            0x4F => {
                let result = self.regs[self.sreg] ^ 0xFFFF;
                self.regs[self.dreg] = result;
                self.update_zs(result);
                self.regs[15] = self.regs[15].wrapping_add(1);
                self.reset_prefix();
                cycles = 1;
            }

            // ── 0x50-0x5F: ADD Rn / ADC Rn [ALT1] / ADD #n [ALT2] / ADC #n [ALT3] ──
            0x50..=0x5F => {
                let n = (opcode & 0x0F) as usize;
                let a = self.regs[self.sreg];
                let b = match alt {
                    2 | 3 => n as u16, // immediate
                    _ => self.regs[n], // register
                };
                let carry = if alt & 1 != 0 {
                    self.flags_cy as u16
                } else {
                    0
                };
                let result32 = a as u32 + b as u32 + carry as u32;
                let result = result32 as u16;
                self.flags_cy = result32 > 0xFFFF;
                self.flags_ov = ((a ^ result) & (b ^ result) & 0x8000) != 0;
                self.regs[self.dreg] = result;
                self.update_zs(result);
                self.regs[15] = self.regs[15].wrapping_add(1);
                self.reset_prefix();
                cycles = 1;
            }

            // ── 0x60-0x6F: SUB Rn / SBC Rn [ALT1] / SUB #n [ALT2] / CMP Rn [ALT3] ──
            0x60..=0x6F => {
                let n = (opcode & 0x0F) as usize;
                let a = self.regs[self.sreg];
                let b = match alt {
                    2 => n as u16,     // SUB #n
                    _ => self.regs[n], // register
                };
                let carry = if alt == 1 {
                    (self.flags_cy as u16) ^ 1
                } else {
                    0
                };

                let result32 = (a as u32).wrapping_sub(b as u32).wrapping_sub(carry as u32);
                let result = result32 as u16;
                self.flags_cy = a >= b.wrapping_add(carry); // carry = no borrow
                self.flags_ov = ((a ^ b) & (a ^ result) & 0x8000) != 0;
                self.update_zs(result);
                if alt == 3 {
                    // CMP: don't store result
                } else {
                    self.regs[self.dreg] = result;
                }
                self.regs[15] = self.regs[15].wrapping_add(1);
                self.reset_prefix();
                cycles = 1;
            }

            // ── 0x70: MERGE ──
            0x70 => {
                let r7 = self.regs[7];
                let r8 = self.regs[8];
                let result = (r7 & 0xFF00) | ((r8 >> 8) & 0xFF);
                self.regs[self.dreg] = result;
                // MERGE sets flags specially: S = bit 15 of result, flags based on R7:R8
                self.flags_s = result & 0x8000 != 0;
                self.flags_z = result == 0;
                self.flags_ov = (r7 | r8) & 0xC0C0 != 0; // overflow from upper bits
                self.flags_cy = (r7 | r8) & 0xE0E0 != 0;
                self.regs[15] = self.regs[15].wrapping_add(1);
                self.reset_prefix();
                cycles = 1;
            }

            // ── 0x71-0x7F: AND Rn / BIC Rn [ALT1] / AND #n [ALT2] / BIC #n [ALT3] ──
            0x71..=0x7F => {
                let n = (opcode & 0x0F) as usize;
                let a = self.regs[self.sreg];
                let b = match alt {
                    2 | 3 => n as u16,
                    _ => self.regs[n],
                };
                let result = if alt & 1 != 0 {
                    a & !b // BIC: AND NOT
                } else {
                    a & b // AND
                };
                self.regs[self.dreg] = result;
                self.update_zs(result);
                self.regs[15] = self.regs[15].wrapping_add(1);
                self.reset_prefix();
                cycles = 1;
            }

            // ── 0x80-0x8F: MULT Rn / UMULT Rn [ALT1] / MULT #n [ALT2] / UMULT #n [ALT3] ──
            0x80..=0x8F => {
                let n = (opcode & 0x0F) as usize;
                let a_lo = (self.regs[self.sreg] & 0xFF) as u8;
                let b_lo = match alt {
                    2 | 3 => n as u8,
                    _ => (self.regs[n] & 0xFF) as u8,
                };
                let result = if alt & 1 != 0 {
                    // UMULT: unsigned 8×8→16
                    (a_lo as u16) * (b_lo as u16)
                } else {
                    // MULT: signed 8×8→16
                    ((a_lo as i8 as i16) * (b_lo as i8 as i16)) as u16
                };
                self.regs[self.dreg] = result;
                self.update_zs(result);
                self.regs[15] = self.regs[15].wrapping_add(1);
                self.reset_prefix();
                // Timing: 1 or 2 clocks depending on multiplier speed (CFGR.MS0)
                cycles = if self.cfgr & 0x20 != 0 { 1 } else { 2 };
            }

            // ── 0x90: SBK (store word back to last RAM address) ──
            0x90 => {
                let val = self.regs[self.sreg];
                self.write_ram_word(self.last_ram_addr, val);
                self.regs[15] = self.regs[15].wrapping_add(1);
                self.reset_prefix();
                cycles = 1;
            }

            // ── 0x91-0x94: LINK #1..#4 ──
            0x91..=0x94 => {
                let n = opcode - 0x90; // 1..4
                self.regs[11] = self.regs[15].wrapping_add(n as u16);
                self.regs[15] = self.regs[15].wrapping_add(1);
                self.reset_prefix();
                cycles = 1;
            }

            // ── 0x95: SEX (sign-extend low byte) ──
            0x95 => {
                let val = self.regs[self.sreg] as u8;
                let result = val as i8 as i16 as u16;
                self.regs[self.dreg] = result;
                self.update_zs(result);
                self.regs[15] = self.regs[15].wrapping_add(1);
                self.reset_prefix();
                cycles = 1;
            }

            // ── 0x96: ASR / DIV2 [ALT1] ──
            0x96 => {
                let src = self.regs[self.sreg];
                if alt1 {
                    // DIV2: SAR with special case for -1
                    let signed = src as i16;
                    self.flags_cy = src & 1 != 0;
                    let result = if signed == -1 {
                        0u16
                    } else {
                        (signed >> 1) as u16
                    };
                    self.regs[self.dreg] = result;
                    self.update_zs(result);
                } else {
                    // ASR: arithmetic shift right
                    self.flags_cy = src & 1 != 0;
                    let result = ((src as i16) >> 1) as u16;
                    self.regs[self.dreg] = result;
                    self.update_zs(result);
                }
                self.regs[15] = self.regs[15].wrapping_add(1);
                self.reset_prefix();
                cycles = 1;
            }

            // ── 0x97: ROR ──
            0x97 => {
                let src = self.regs[self.sreg];
                let old_cy = self.flags_cy as u16;
                self.flags_cy = src & 1 != 0;
                let result = (src >> 1) | (old_cy << 15);
                self.regs[self.dreg] = result;
                self.update_zs(result);
                self.regs[15] = self.regs[15].wrapping_add(1);
                self.reset_prefix();
                cycles = 1;
            }

            // ── 0x98-0x9D: JMP Rn / LJMP Rn [ALT1] (n=8..13) ──
            0x98..=0x9D => {
                let n = (opcode & 0x0F) as usize; // 8..13
                if alt1 {
                    // LJMP: PBR=Rn, R15=Rs, clear cache
                    self.pbr = (self.regs[n] & 0x7F) as u8; // bank 00-5F,70-71
                    self.regs[15] = self.regs[self.sreg];
                    self.cbr = 0;
                    self.cache_valid = [false; 32];
                } else {
                    // JMP: R15=Rn
                    self.regs[15] = self.regs[n];
                }
                self.reset_prefix();
                cycles = 1;
            }

            // ── 0x9E: LOB (low byte, zero-extend) ──
            0x9E => {
                let lo = self.regs[self.sreg] & 0xFF;
                self.regs[self.dreg] = lo;
                self.update_zs(lo);
                self.regs[15] = self.regs[15].wrapping_add(1);
                self.reset_prefix();
                cycles = 1;
            }

            // ── 0x9F: FMULT / LMULT [ALT1] ──
            0x9F => {
                let a = self.regs[self.sreg] as i16;
                let b = self.regs[6] as i16;
                let product = (a as i32) * (b as i32);
                if alt1 {
                    // LMULT: full 32-bit signed result → Dreg=high16, R4=low16
                    let result = product as u32;
                    self.regs[self.dreg] = (result >> 16) as u16;
                    self.regs[4] = result as u16;
                    self.update_zs(self.regs[self.dreg]);
                    self.flags_cy = self.regs[4] & 0x8000 != 0;
                    cycles = if self.cfgr & 0x20 != 0 { 5 } else { 9 };
                } else {
                    // FMULT: fractional → Dreg = (Rs*R6) >> 16, carry = bit15 of low product
                    let shifted = (product << 1) as u32;
                    self.regs[self.dreg] = (shifted >> 16) as u16;
                    self.flags_cy = (shifted & 0x8000) != 0;
                    self.update_zs(self.regs[self.dreg]);
                    cycles = if self.cfgr & 0x20 != 0 { 4 } else { 8 };
                }
                self.regs[15] = self.regs[15].wrapping_add(1);
                self.reset_prefix();
            }

            // ── 0xA0-0xAF: IBT Rn,#pp / LMS Rn,(yy) [ALT1] / SMS (yy),Rn [ALT2] / LM Rn,(hhll) [ALT3] ──
            0xA0..=0xAF => {
                let n = (opcode & 0x0F) as usize;
                let pc_base = ((self.pbr as u32) << 16) | (self.regs[15].wrapping_add(1) as u32);
                match alt {
                    1 => {
                        // LMS: load from RAM[yy*2]
                        let yy = self.fetch_byte(pc_base) as u16;
                        let addr = yy << 1;
                        self.last_ram_addr = addr;
                        let val = self.read_ram_word(addr);
                        self.regs[n] = val;
                        self.update_zs(val);
                        self.regs[15] = self.regs[15].wrapping_add(2);
                        cycles = 2; // + RAM access time
                    }
                    2 => {
                        // SMS: store to RAM[yy*2]
                        let yy = self.fetch_byte(pc_base) as u16;
                        let addr = yy << 1;
                        self.last_ram_addr = addr;
                        let val = self.regs[self.sreg];
                        self.write_ram_word(addr, val);
                        self.regs[15] = self.regs[15].wrapping_add(2);
                        cycles = 2;
                    }
                    3 => {
                        // LM: load from RAM[hhll]
                        let lo = self.fetch_byte(pc_base);
                        let hi = self.fetch_byte(pc_base + 1);
                        let addr = u16::from_le_bytes([lo, hi]);
                        self.last_ram_addr = addr;
                        let val = self.read_ram_word(addr);
                        self.regs[n] = val;
                        self.update_zs(val);
                        self.regs[15] = self.regs[15].wrapping_add(3);
                        cycles = 3;
                    }
                    _ => {
                        // IBT: Rn = sign-extended byte
                        let pp = self.fetch_byte(pc_base);
                        self.regs[n] = pp as i8 as i16 as u16;
                        self.regs[15] = self.regs[15].wrapping_add(2);
                        cycles = 2;
                    }
                }
                self.reset_prefix();
            }

            // ── 0xB0-0xBF: FROM Rn ──
            0xB0..=0xBF => {
                let n = (opcode & 0x0F) as usize;
                self.sreg = n;
                // FROM is a prefix, does NOT set B flag
                self.regs[15] = self.regs[15].wrapping_add(1);
                // FROM does NOT reset prefix (it IS a prefix, retains ALT flags)
                cycles = 1;
            }

            // ── 0xC0: HIB (high byte to low, zero-extend upper) ──
            0xC0 => {
                let hi = (self.regs[self.sreg] >> 8) & 0xFF;
                self.regs[self.dreg] = hi;
                self.update_zs(hi);
                self.regs[15] = self.regs[15].wrapping_add(1);
                self.reset_prefix();
                cycles = 1;
            }

            // ── 0xC1-0xCF: OR Rn / XOR Rn [ALT1] / OR #n [ALT2] / XOR #n [ALT3] ──
            0xC1..=0xCF => {
                let n = (opcode & 0x0F) as usize;
                let a = self.regs[self.sreg];
                let b = match alt {
                    2 | 3 => n as u16,
                    _ => self.regs[n],
                };
                let result = if alt & 1 != 0 {
                    a ^ b // XOR
                } else {
                    a | b // OR
                };
                self.regs[self.dreg] = result;
                self.update_zs(result);
                self.regs[15] = self.regs[15].wrapping_add(1);
                self.reset_prefix();
                cycles = 1;
            }

            // ── 0xD0-0xDE: INC Rn (n=0..14) ──
            0xD0..=0xDE => {
                let n = (opcode & 0x0F) as usize;
                self.regs[n] = self.regs[n].wrapping_add(1);
                self.update_zs(self.regs[n]);
                self.regs[15] = self.regs[15].wrapping_add(1);
                self.reset_prefix();
                cycles = 1;
            }

            // ── 0xDF: GETC / RAMB [ALT2] / ROMB [ALT3] ──
            0xDF => {
                match alt {
                    2 => {
                        // RAMB: RAMBR = Rs & 01h
                        self.rambr = (self.regs[self.sreg] & 0x01) as u8;
                    }
                    1 | 3 => {
                        // ROMB: ROMBR = Rs & FFh
                        self.rombr = (self.regs[self.sreg] & 0xFF) as u8;
                    }
                    _ => {
                        // GETC: COLR = ROM[ROMBR:R14]
                        self.prefetch_rom_buffer();
                        self.colr = self.rom_buffer;
                    }
                }
                self.regs[15] = self.regs[15].wrapping_add(1);
                self.reset_prefix();
                cycles = 1;
            }

            // ── 0xE0-0xEE: DEC Rn (n=0..14) ──
            0xE0..=0xEE => {
                let n = (opcode & 0x0F) as usize;
                self.regs[n] = self.regs[n].wrapping_sub(1);
                self.update_zs(self.regs[n]);
                self.regs[15] = self.regs[15].wrapping_add(1);
                self.reset_prefix();
                cycles = 1;
            }

            // ── 0xEF: GETB / GETBH [ALT1] / GETBL [ALT2] / GETBS [ALT3] ──
            0xEF => {
                self.prefetch_rom_buffer();
                let byte = self.rom_buffer;
                match alt {
                    1 => {
                        // GETBH: Rd.hi = byte, Rd.lo unchanged
                        self.regs[self.dreg] =
                            (self.regs[self.dreg] & 0x00FF) | ((byte as u16) << 8);
                    }
                    2 => {
                        // GETBL: Rd.lo = byte, Rd.hi unchanged
                        self.regs[self.dreg] = (self.regs[self.dreg] & 0xFF00) | (byte as u16);
                    }
                    3 => {
                        // GETBS: Rd = sign-extended byte
                        self.regs[self.dreg] = byte as i8 as i16 as u16;
                    }
                    _ => {
                        // GETB: Rd = zero-extended byte
                        self.regs[self.dreg] = byte as u16;
                    }
                }
                self.regs[15] = self.regs[15].wrapping_add(1);
                self.reset_prefix();
                cycles = 1;
            }

            // ── 0xF0-0xFF: IWT Rn,#yyxx / SM (hhll),Rn [ALT1] / SMS (yy),Rn [ALT2] / SM (hhll),Rn [ALT3] ──
            0xF0..=0xFF => {
                let n = (opcode & 0x0F) as usize;
                let pc_base = ((self.pbr as u32) << 16) | (self.regs[15].wrapping_add(1) as u32);
                match alt {
                    1 => {
                        // LM Rn,(hhll): load word from RAM (ALT1 with IWT = LM)
                        let lo = self.fetch_byte(pc_base);
                        let hi = self.fetch_byte(pc_base + 1);
                        let addr = u16::from_le_bytes([lo, hi]);
                        self.last_ram_addr = addr;
                        let val = self.read_ram_word(addr);
                        self.regs[n] = val;
                        self.update_zs(val);
                        self.regs[15] = self.regs[15].wrapping_add(3);
                        cycles = 3;
                    }
                    2 => {
                        // SM (yy),Rn: store word to RAM[yy*2]
                        let yy = self.fetch_byte(pc_base) as u16;
                        let addr = yy << 1;
                        self.last_ram_addr = addr;
                        let val = self.regs[self.sreg];
                        self.write_ram_word(addr, val);
                        self.regs[15] = self.regs[15].wrapping_add(2);
                        cycles = 2;
                    }
                    3 => {
                        // SM (hhll),Rn: store word to RAM
                        let lo = self.fetch_byte(pc_base);
                        let hi = self.fetch_byte(pc_base + 1);
                        let addr = u16::from_le_bytes([lo, hi]);
                        self.last_ram_addr = addr;
                        let val = self.regs[self.sreg];
                        self.write_ram_word(addr, val);
                        self.regs[15] = self.regs[15].wrapping_add(3);
                        cycles = 3;
                    }
                    _ => {
                        // IWT Rn,#yyxx: load immediate word
                        let lo = self.fetch_byte(pc_base);
                        let hi = self.fetch_byte(pc_base + 1);
                        self.regs[n] = u16::from_le_bytes([lo, hi]);
                        self.regs[15] = self.regs[15].wrapping_add(3);
                        cycles = 3;
                    }
                }
                self.reset_prefix();
            }
        };

        self.cycles += cycles as u64;
        cycles
    }

    /// Run GSU for a number of cycles
    fn run(&mut self, target_cycles: u64) {
        let start_cycles = self.cycles;
        while self.flags_g && (self.cycles - start_cycles) < target_cycles {
            let addr = ((self.pbr as u32) << 16) | (self.regs[15] as u32);
            let opcode = self.fetch_byte(addr);

            // Fill cache line if executing from cacheable region
            let pc16 = self.regs[15];
            let cache_offset = pc16.wrapping_sub(self.cbr) as usize;
            if cache_offset < 512 {
                let line = cache_offset / 16;
                if !self.cache_valid[line] {
                    // Load 16-byte cache line from ROM
                    let line_base = self.cbr.wrapping_add((line as u16) * 16);
                    let rom_base = ((self.pbr as u32) << 16) | (line_base as u32);
                    for i in 0..16 {
                        self.cache[(line * 16) + i] = self.read_rom(rom_base + i as u32);
                    }
                    self.cache_valid[line] = true;
                }
            }

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
            0x30 => self.sfr_low(),
            0x31 => {
                let val = self.sfr_high();
                // Reading SFR high byte clears IRQ flag
                self.flags_irq = false;
                val
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
            // VCR (Version Code Register) at $3B
            // GSU-1 = $01, GSU-2 = $04
            0x3B => {
                if self.is_superfx2 {
                    0x04
                } else {
                    0x01
                }
            }
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
            // R15 (PC) at $1E/$1F: low byte is latched, applied when high byte is written
            0x00..=0x1F => {
                let reg = (offset / 2) as usize;
                if reg == 15 {
                    // R15 (PC) special handling
                    if offset & 1 == 0 {
                        // Low byte: latch for later
                        self.r15_latch = val;
                    } else {
                        // High byte: apply both bytes, triggers GO
                        self.regs[15] = (self.r15_latch as u16) | ((val as u16) << 8);
                    }
                } else if offset & 1 == 0 {
                    self.regs[reg] = (self.regs[reg] & 0xFF00) | (val as u16);
                } else {
                    self.regs[reg] = (self.regs[reg] & 0x00FF) | ((val as u16) << 8);
                }
            }
            // SFR (Status/Flag Register) at $30-$31
            0x30 => {
                self.set_sfr_low(val);
                log(LogCategory::Bus, LogLevel::Info, || {
                    format!(
                        "SuperFX: SFR low write ${:02X}, GO={}, PC=${:04X}",
                        val,
                        self.flags_g,
                        self.pc()
                    )
                });
            }
            0x31 => {
                self.set_sfr_high(val);
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
            // POR (Plot Option Register) at $3D (shares address with ALT1 opcode, but different context)
            0x3D => self.por = val,
            // CBR (Cache Base Register) at $3E-$3F
            0x3E => self.cbr = (self.cbr & 0xFF00) | (val as u16),
            0x3F => self.cbr = (self.cbr & 0x00FF) | ((val as u16) << 8),
            // COLR (Color Register) at $40
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
    fn test_sfr_flags() {
        let mut sfx = SuperFx::new();

        sfx.flags_z = true;
        sfx.flags_cy = true;
        let sfr = sfx.sfr_low();
        assert_eq!(sfr & 0x02, 0x02); // Z flag
        assert_eq!(sfr & 0x04, 0x04); // CY flag

        // Test round-trip
        sfx.flags_z = false;
        sfx.flags_cy = false;
        sfx.set_sfr_low(sfr);
        assert!(sfx.flags_z);
        assert!(sfx.flags_cy);
    }

    #[test]
    fn test_stop_instruction() {
        let mut sfx = SuperFx::new();
        sfx.flags_g = true;

        sfx.execute_instruction(0x00); // STOP
        assert!(!sfx.flags_g);
        assert!(sfx.flags_irq);
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
    fn test_add_with_carry() {
        let mut sfx = SuperFx::new();
        sfx.regs[0] = 10;
        sfx.regs[1] = 20;
        sfx.flags_cy = true;
        sfx.flags_alt1 = true; // ALT1 = ADC

        sfx.execute_instruction(0x51); // ADC R1
        assert_eq!(sfx.regs[0], 31); // 10 + 20 + 1
    }

    #[test]
    fn test_mult_instruction() {
        let mut sfx = SuperFx::new();
        sfx.regs[0] = 10;
        sfx.regs[2] = 20;

        sfx.execute_instruction(0x82); // MULT R2 (0x80+n)
        assert_eq!(sfx.regs[0], 200); // result in Dreg (R0)
    }

    #[test]
    fn test_signed_mult() {
        let mut sfx = SuperFx::new();
        // -2 * 3 = -6 = 0xFFFA
        sfx.regs[0] = 0xFE; // -2 as u8
        sfx.regs[1] = 3;

        sfx.execute_instruction(0x81); // MULT R1
        assert_eq!(sfx.regs[0], 0xFFFA); // -6 signed
    }

    #[test]
    fn test_color_instruction() {
        let mut sfx = SuperFx::new();
        sfx.regs[0] = 0x42;

        sfx.execute_instruction(0x4E); // COLOR
        assert_eq!(sfx.colr, 0x42);
    }

    #[test]
    fn test_enhancement_chip_trait_read_write() {
        let mut sfx = SuperFx::new();

        // Test register read/write through EnhancementChip trait
        sfx.write(0x003002, 0x34); // R1 Low byte
        sfx.write(0x003003, 0x12); // R1 High byte
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
        sfx.regs[15] = 0x8000;
        sfx.colr = 0x42;
        sfx.flags_g = true;

        let state = sfx.save_state().expect("Save should succeed");
        assert!(!state.is_empty());

        sfx.regs[0] = 0;
        sfx.colr = 0;
        sfx.flags_g = false;

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
        assert_eq!(sfx.sreg, 0);
        assert_eq!(sfx.dreg, 0);

        sfx.execute_instruction(0x25); // WITH R5
        assert_eq!(sfx.sreg, 5);
        assert_eq!(sfx.dreg, 5);
        assert!(sfx.flags_b);
    }

    #[test]
    fn test_to_instruction() {
        let mut sfx = SuperFx::new();
        sfx.execute_instruction(0x13); // TO R3
        assert_eq!(sfx.dreg, 3);
        // sreg stays at default
        assert_eq!(sfx.sreg, 0);
    }

    #[test]
    fn test_from_instruction() {
        let mut sfx = SuperFx::new();
        sfx.execute_instruction(0xB7); // FROM R7
        assert_eq!(sfx.sreg, 7);
        // dreg stays at default
        assert_eq!(sfx.dreg, 0);
    }

    #[test]
    fn test_branch_instructions_pc() {
        let mut sfx = SuperFx::new();
        let test_rom = vec![
            0x05, 0x02, // BRA +2 (skip next 2 bytes)
            0x00, 0x00, // (skipped)
            0x01, // NOP (target)
        ];
        sfx.set_rom(test_rom);
        sfx.regs[15] = 0;

        sfx.execute_instruction(0x05); // BRA
                                       // PC should be at offset 4 (was 0, +2 for instruction, +2 for offset)
        assert_eq!(sfx.pc(), 4);
    }

    #[test]
    fn test_bne_taken() {
        let mut sfx = SuperFx::new();
        let test_rom = vec![0x08, 0x02]; // BNE +2
        sfx.set_rom(test_rom);
        sfx.regs[15] = 0;
        sfx.flags_z = false; // Not zero, branch taken

        sfx.execute_instruction(0x08);
        assert_eq!(sfx.pc(), 4); // 0 + 2 + 2
    }

    #[test]
    fn test_bne_not_taken() {
        let mut sfx = SuperFx::new();
        let test_rom = vec![0x08, 0x02]; // BNE +2
        sfx.set_rom(test_rom);
        sfx.regs[15] = 0;
        sfx.flags_z = true; // Zero, branch not taken

        sfx.execute_instruction(0x08);
        assert_eq!(sfx.pc(), 2); // Skip past opcode + offset
    }

    #[test]
    fn test_sub_instruction() {
        let mut sfx = SuperFx::new();
        sfx.regs[0] = 30;
        sfx.regs[1] = 10;

        sfx.execute_instruction(0x61); // SUB R1
        assert_eq!(sfx.regs[0], 20);
        assert!(!sfx.flags_z);
        assert!(!sfx.flags_s);
    }

    #[test]
    fn test_cmp_instruction() {
        let mut sfx = SuperFx::new();
        sfx.regs[0] = 10;
        sfx.regs[1] = 10;
        sfx.flags_alt1 = true;
        sfx.flags_alt2 = true; // ALT3 = CMP

        sfx.execute_instruction(0x61); // CMP R1
        assert!(sfx.flags_z);
        assert_eq!(sfx.regs[0], 10); // CMP doesn't modify Dreg
    }

    #[test]
    fn test_and_instruction() {
        let mut sfx = SuperFx::new();
        sfx.regs[0] = 0xFF0F;
        sfx.regs[1] = 0x0FFF;

        sfx.execute_instruction(0x71); // AND R1
        assert_eq!(sfx.regs[0], 0x0F0F);
    }

    #[test]
    fn test_or_instruction() {
        let mut sfx = SuperFx::new();
        sfx.regs[0] = 0xF000;
        sfx.regs[1] = 0x00FF;

        sfx.execute_instruction(0xC1); // OR R1
        assert_eq!(sfx.regs[0], 0xF0FF);
    }

    #[test]
    fn test_xor_instruction() {
        let mut sfx = SuperFx::new();
        sfx.regs[0] = 0xFF00;
        sfx.regs[1] = 0xFFFF;
        sfx.flags_alt1 = true; // XOR variant

        sfx.execute_instruction(0xC1); // XOR R1
        assert_eq!(sfx.regs[0], 0x00FF);
    }

    #[test]
    fn test_inc_dec() {
        let mut sfx = SuperFx::new();
        sfx.regs[3] = 10;

        sfx.execute_instruction(0xD3); // INC R3
        assert_eq!(sfx.regs[3], 11);

        sfx.execute_instruction(0xE3); // DEC R3
        assert_eq!(sfx.regs[3], 10);
    }

    #[test]
    fn test_lsr() {
        let mut sfx = SuperFx::new();
        sfx.regs[0] = 0x0004;

        sfx.execute_instruction(0x03); // LSR
        assert_eq!(sfx.regs[0], 0x0002);
        assert!(!sfx.flags_cy);
    }

    #[test]
    fn test_lsr_carry() {
        let mut sfx = SuperFx::new();
        sfx.regs[0] = 0x0005;

        sfx.execute_instruction(0x03); // LSR
        assert_eq!(sfx.regs[0], 0x0002);
        assert!(sfx.flags_cy);
    }

    #[test]
    fn test_swap() {
        let mut sfx = SuperFx::new();
        sfx.regs[0] = 0x1234;

        sfx.execute_instruction(0x4D); // SWAP
        assert_eq!(sfx.regs[0], 0x3412);
    }

    #[test]
    fn test_not() {
        let mut sfx = SuperFx::new();
        sfx.regs[0] = 0xFF00;

        sfx.execute_instruction(0x4F); // NOT
        assert_eq!(sfx.regs[0], 0x00FF);
    }

    #[test]
    fn test_merge() {
        let mut sfx = SuperFx::new();
        sfx.regs[7] = 0x1200;
        sfx.regs[8] = 0x3400;

        sfx.execute_instruction(0x70); // MERGE
        assert_eq!(sfx.regs[0], 0x1234);
    }

    #[test]
    fn test_sex() {
        let mut sfx = SuperFx::new();
        sfx.regs[0] = 0x00FE; // -2 as byte

        sfx.execute_instruction(0x95); // SEX
        assert_eq!(sfx.regs[0], 0xFFFE); // sign-extended
    }

    #[test]
    fn test_lob_hib() {
        let mut sfx = SuperFx::new();
        sfx.regs[0] = 0x1234;

        sfx.execute_instruction(0x9E); // LOB
        assert_eq!(sfx.regs[0], 0x0034);

        sfx.regs[0] = 0x1234;
        sfx.execute_instruction(0xC0); // HIB
        assert_eq!(sfx.regs[0], 0x0012);
    }

    #[test]
    fn test_loop_instruction() {
        let mut sfx = SuperFx::new();
        sfx.regs[12] = 3; // Loop counter
        sfx.regs[13] = 0x1000; // Loop target

        sfx.execute_instruction(0x3C); // LOOP
        assert_eq!(sfx.regs[12], 2);
        assert_eq!(sfx.pc(), 0x1000); // Jumped to R13
    }

    #[test]
    fn test_loop_exit() {
        let mut sfx = SuperFx::new();
        sfx.regs[12] = 1; // Last iteration
        sfx.regs[13] = 0x1000;
        sfx.regs[15] = 0x2000;

        sfx.execute_instruction(0x3C); // LOOP
        assert_eq!(sfx.regs[12], 0);
        assert!(sfx.flags_z);
        assert_eq!(sfx.pc(), 0x2001); // Fell through (PC+1)
    }

    #[test]
    fn test_link_instruction() {
        let mut sfx = SuperFx::new();
        sfx.regs[15] = 0x100;

        sfx.execute_instruction(0x91); // LINK #1
        assert_eq!(sfx.regs[11], 0x101); // R11 = PC + 1
    }

    #[test]
    fn test_sbk_instruction() {
        let mut sfx = SuperFx::new();
        sfx.regs[0] = 0x1234;
        sfx.last_ram_addr = 0x0100;

        sfx.execute_instruction(0x90); // SBK
        assert_eq!(sfx.read_ram_word(0x0100), 0x1234);
    }

    #[test]
    fn test_stw_ldw() {
        let mut sfx = SuperFx::new();
        sfx.regs[0] = 0xABCD; // value to store
        sfx.regs[1] = 0x0200; // address

        sfx.execute_instruction(0x31); // STW (R1)
        assert_eq!(sfx.read_ram_word(0x0200), 0xABCD);

        sfx.regs[0] = 0;
        sfx.execute_instruction(0x41); // LDW (R1)
        assert_eq!(sfx.regs[0], 0xABCD);
    }

    #[test]
    fn test_ibt_instruction() {
        let mut sfx = SuperFx::new();
        // IBT R2,#$FE (sign-extended to $FFFE)
        let test_rom = vec![0xA2, 0xFE];
        sfx.set_rom(test_rom);
        sfx.regs[15] = 0;

        sfx.execute_instruction(0xA2); // IBT R2
        assert_eq!(sfx.regs[2], 0xFFFE); // sign-extended
        assert_eq!(sfx.pc(), 2); // advanced past opcode + imm
    }

    #[test]
    fn test_iwt_instruction() {
        let mut sfx = SuperFx::new();
        // IWT R1,#$1234
        let test_rom = vec![0xF1, 0x34, 0x12];
        sfx.set_rom(test_rom);
        sfx.regs[15] = 0;

        sfx.execute_instruction(0xF1); // IWT R1
        assert_eq!(sfx.regs[1], 0x1234);
        assert_eq!(sfx.pc(), 3);
    }

    #[test]
    fn test_jmp_instruction() {
        let mut sfx = SuperFx::new();
        sfx.regs[8] = 0x4000; // Jump target in R8

        sfx.execute_instruction(0x98); // JMP R8
        assert_eq!(sfx.pc(), 0x4000);
    }

    #[test]
    fn test_getc_instruction() {
        let mut sfx = SuperFx::new();
        let test_rom = vec![0x12, 0x34, 0x56, 0x78];
        sfx.set_rom(test_rom);
        sfx.rombr = 0x00;
        sfx.regs[14] = 0x0002; // Point to third byte

        sfx.execute_instruction(0xDF); // GETC
        assert_eq!(sfx.colr, 0x56);
    }

    #[test]
    fn test_getb_instruction() {
        let mut sfx = SuperFx::new();
        let test_rom = vec![0x12, 0x34, 0x56];
        sfx.set_rom(test_rom);
        sfx.rombr = 0x00;
        sfx.regs[14] = 0x0001;

        sfx.execute_instruction(0xEF); // GETB
        assert_eq!(sfx.regs[0], 0x34);
    }

    #[test]
    fn test_fmult_instruction() {
        let mut sfx = SuperFx::new();
        sfx.regs[0] = 0x0080; // 128
        sfx.regs[6] = 0x0100; // 256
                              // FMULT: (Rs * R6) << 1, take high word
                              // 128 * 256 = 32768, << 1 = 65536 = 0x10000, high word = 1

        sfx.execute_instruction(0x9F); // FMULT
        assert_eq!(sfx.regs[0], 1);
    }

    #[test]
    fn test_prefix_reset_after_instruction() {
        let mut sfx = SuperFx::new();

        // Set ALT1
        sfx.execute_instruction(0x3D); // ALT1
        assert!(sfx.flags_alt1);

        // Execute LSR which should reset ALT flags
        sfx.regs[0] = 4;
        sfx.execute_instruction(0x03); // LSR (since ALT1 is set, this uses sreg)
        assert!(!sfx.flags_alt1);
        assert!(!sfx.flags_alt2);
        assert!(!sfx.flags_b);
    }

    #[test]
    fn test_version_register() {
        let mut sfx = SuperFx::new();
        assert_eq!(sfx.read_register(0x303B), 0x01); // GSU-1

        let mut sfx2 = SuperFx::new_superfx2();
        assert_eq!(sfx2.read_register(0x303B), 0x04); // GSU-2
    }
}
