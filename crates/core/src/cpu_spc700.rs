//! Sony SPC700 CPU core implementation
//!
//! The SPC700 is an 8-bit CPU used in the SNES Audio Processing Unit (APU).
//! It has its own 64KB address space with RAM, DSP registers, and I/O ports.
//!
//! **Architecture:**
//! - 8-bit accumulator and index registers
//! - 16-bit program counter
//! - 8-bit stack pointer (page 1: $0100-$01FF)
//! - 8-bit processor status word (PSW)
//! - 256 opcodes with various addressing modes
//! - 3 internal timers
//! - Communication ports for interfacing with main CPU
//!
//! **References:**
//! - https://problemkaputt.de/fullsnes.htm#snescpuspc700audiosystemapu
//! - https://wiki.superfamicom.org/spc700-reference

use crate::logging::{log, LogCategory, LogLevel};

/// Memory interface trait for the SPC700 CPU
///
/// The SPC700 has a 64KB address space containing:
/// - $0000-$00EF: RAM (page 0, with special uses)
/// - $00F0-$00FF: DSP and I/O registers
/// - $0100-$01FF: Stack page
/// - $0200-$FFBF: RAM
/// - $FFC0-$FFFF: IPL ROM (64 bytes, can be disabled)
pub trait MemorySpc700 {
    /// Read a byte from memory at the given address
    fn read(&self, addr: u16) -> u8;

    /// Write a byte to memory at the given address
    fn write(&mut self, addr: u16, val: u8);
}

/// SPC700 Processor Status Word (PSW) flags
#[allow(dead_code)]
mod psw_flags {
    pub const NEGATIVE: u8 = 0b1000_0000; // N - Sign flag
    pub const OVERFLOW: u8 = 0b0100_0000; // V - Overflow flag
    pub const DIRECT_PAGE: u8 = 0b0010_0000; // P - Direct page flag ($0000 or $0100)
    pub const BREAK: u8 = 0b0001_0000; // B - Break flag
    pub const HALF_CARRY: u8 = 0b0000_1000; // H - Half-carry flag (for BCD)
    pub const INTERRUPT: u8 = 0b0000_0100; // I - Interrupt enable flag
    pub const ZERO: u8 = 0b0000_0010; // Z - Zero flag
    pub const CARRY: u8 = 0b0000_0001; // C - Carry flag
}

/// Sony SPC700 CPU state and execution engine
///
/// This is a reusable SPC700 CPU implementation that works with any
/// memory interface through the `MemorySpc700` trait.
#[derive(Debug)]
pub struct CpuSpc700<M: MemorySpc700> {
    /// Accumulator register
    pub a: u8,
    /// X index register
    pub x: u8,
    /// Y index register
    pub y: u8,
    /// Stack pointer (points to $0100 + sp)
    pub sp: u8,
    /// Program counter
    pub pc: u16,
    /// Processor Status Word (PSW): NV P B H I Z C
    pub psw: u8,
    /// Total cycles executed
    pub cycles: u64,
    /// Memory interface
    pub memory: M,
}

impl<M: MemorySpc700> CpuSpc700<M> {
    /// Create a new SPC700 CPU with the given memory interface
    pub fn new(memory: M) -> Self {
        Self {
            a: 0,
            x: 0,
            y: 0,
            sp: 0,
            pc: 0xFFC0, // IPL ROM start address
            psw: 0,
            cycles: 0,
            memory,
        }
    }

    /// Reset the CPU to initial state (preserves memory)
    pub fn reset(&mut self) {
        self.a = 0;
        self.x = 0;
        self.y = 0;
        self.sp = 0;
        self.psw = 0;
        self.pc = 0xFFC0; // IPL ROM start
        self.cycles = 0;
    }

    /// Execute one instruction and return the number of cycles it took
    pub fn step(&mut self) -> u8 {
        let pc_before = self.pc;
        let opcode = self.fetch_byte();

        // Log execution from uploaded code region (addresses < $0100)
        if pc_before < 0x0100 {
            log(LogCategory::APU, LogLevel::Info, || {
                format!("SPC700: Executing uploaded code at PC=${:04X} opcode=${:02X} A=${:02X} X=${:02X} Y=${:02X}", 
                    pc_before, opcode, self.a, self.x, self.y)
            });
        }
        // Log every 1000th instruction from IPL ROM to avoid spam
        else if self.cycles.is_multiple_of(1000) {
            log(LogCategory::APU, LogLevel::Debug, || {
                format!(
                    "SPC700: PC=${:04X} opcode=${:02X} A=${:02X} X=${:02X} Y=${:02X}",
                    pc_before, opcode, self.a, self.x, self.y
                )
            });
        }

        let cycles = self.execute_opcode(opcode);
        self.cycles += cycles as u64;
        cycles
    }

    /// Read a byte from memory
    #[inline]
    fn read(&self, addr: u16) -> u8 {
        self.memory.read(addr)
    }

    /// Write a byte to memory
    #[inline]
    fn write(&mut self, addr: u16, val: u8) {
        self.memory.write(addr, val);
    }

    /// Fetch the next byte from PC and increment PC
    #[inline]
    fn fetch_byte(&mut self) -> u8 {
        let byte = self.read(self.pc);
        self.pc = self.pc.wrapping_add(1);
        byte
    }

    /// Fetch the next 16-bit word from PC (little-endian) and increment PC
    #[inline]
    fn fetch_word(&mut self) -> u16 {
        let low = self.fetch_byte() as u16;
        let high = self.fetch_byte() as u16;
        (high << 8) | low
    }

    /// Push a byte onto the stack
    #[inline]
    #[allow(dead_code)]
    fn push(&mut self, val: u8) {
        let addr = 0x0100 | (self.sp as u16);
        self.write(addr, val);
        self.sp = self.sp.wrapping_sub(1);
    }

    /// Pop a byte from the stack
    #[inline]
    #[allow(dead_code)]
    fn pop(&mut self) -> u8 {
        self.sp = self.sp.wrapping_add(1);
        let addr = 0x0100 | (self.sp as u16);
        self.read(addr)
    }

    /// Get the direct page offset (either $0000 or $0100 based on P flag)
    #[inline]
    fn direct_page(&self) -> u16 {
        if self.psw & psw_flags::DIRECT_PAGE != 0 {
            0x0100
        } else {
            0x0000
        }
    }

    // PSW flag manipulation
    #[inline]
    fn set_flag(&mut self, flag: u8, condition: bool) {
        if condition {
            self.psw |= flag;
        } else {
            self.psw &= !flag;
        }
    }

    #[inline]
    fn get_flag(&self, flag: u8) -> bool {
        self.psw & flag != 0
    }

    /// Update N and Z flags based on a value
    #[inline]
    fn update_nz(&mut self, val: u8) {
        self.set_flag(psw_flags::ZERO, val == 0);
        self.set_flag(psw_flags::NEGATIVE, val & 0x80 != 0);
    }

    /// Set carry flag
    #[inline]
    fn set_carry(&mut self, carry: bool) {
        self.set_flag(psw_flags::CARRY, carry);
    }

    /// Read a 16-bit word from memory (little-endian)
    #[inline]
    fn read_word(&self, addr: u16) -> u16 {
        let lo = self.read(addr) as u16;
        let hi = self.read(addr.wrapping_add(1)) as u16;
        (hi << 8) | lo
    }

    /// Write a 16-bit word to memory (little-endian)
    #[inline]
    fn write_word(&mut self, addr: u16, val: u16) {
        self.write(addr, (val & 0xFF) as u8);
        self.write(addr.wrapping_add(1), (val >> 8) as u8);
    }

    /// Execute a single opcode and return cycles taken
    fn execute_opcode(&mut self, opcode: u8) -> u8 {
        match opcode {
            // NOP
            0x00 => 2,

            // CLRP - Clear direct page flag (P = 0, direct page = $0000)
            0x20 => {
                self.psw &= !psw_flags::DIRECT_PAGE;
                2
            }

            // SETP - Set direct page flag (P = 1, direct page = $0100)
            0x40 => {
                self.psw |= psw_flags::DIRECT_PAGE;
                2
            }

            // BRA rel - Branch always
            0x2F => {
                let offset = self.fetch_byte() as i8;
                self.pc = self.pc.wrapping_add(offset as u16);
                4
            }

            // BEQ rel - Branch if zero flag set
            0xF0 => {
                let offset = self.fetch_byte() as i8;
                if self.get_flag(psw_flags::ZERO) {
                    self.pc = self.pc.wrapping_add(offset as u16);
                    4
                } else {
                    2
                }
            }

            // BNE rel - Branch if zero flag clear
            0xD0 => {
                let offset = self.fetch_byte() as i8;
                if !self.get_flag(psw_flags::ZERO) {
                    self.pc = self.pc.wrapping_add(offset as u16);
                    4
                } else {
                    2
                }
            }

            // CMP A, #imm - Compare A with immediate
            0x68 => {
                let val = self.fetch_byte();
                let result = self.a.wrapping_sub(val);
                self.update_nz(result);
                self.set_flag(psw_flags::CARRY, self.a >= val);
                2
            }

            // CMP A, dp - Compare A with direct page
            0x64 => {
                let addr = self.direct_page() | (self.fetch_byte() as u16);
                let val = self.read(addr);
                let result = self.a.wrapping_sub(val);
                self.update_nz(result);
                self.set_flag(psw_flags::CARRY, self.a >= val);
                3
            }

            // INC A - Increment accumulator
            0xBC => {
                self.a = self.a.wrapping_add(1);
                self.update_nz(self.a);
                2
            }

            // INC X - Increment X
            0x3D => {
                self.x = self.x.wrapping_add(1);
                self.update_nz(self.x);
                2
            }

            // INC Y - Increment Y
            0xFC => {
                self.y = self.y.wrapping_add(1);
                self.update_nz(self.y);
                2
            }

            // DEC A - Decrement accumulator
            0x9C => {
                self.a = self.a.wrapping_sub(1);
                self.update_nz(self.a);
                2
            }

            // DEC X - Decrement X
            0x1D => {
                self.x = self.x.wrapping_sub(1);
                self.update_nz(self.x);
                2
            }

            // DEC Y - Decrement Y
            0xDC => {
                self.y = self.y.wrapping_sub(1);
                self.update_nz(self.y);
                2
            }

            // MOV A, #imm
            0xE8 => {
                self.a = self.fetch_byte();
                self.update_nz(self.a);
                2
            }

            // MOV X, #imm
            0xCD => {
                self.x = self.fetch_byte();
                self.update_nz(self.x);
                2
            }

            // MOV Y, #imm
            0x8D => {
                self.y = self.fetch_byte();
                self.update_nz(self.y);
                2
            }

            // MOV A, X
            0x7D => {
                self.a = self.x;
                self.update_nz(self.a);
                2
            }

            // MOV A, Y
            0xDD => {
                self.a = self.y;
                self.update_nz(self.a);
                2
            }

            // MOV X, A
            0x5D => {
                self.x = self.a;
                self.update_nz(self.x);
                2
            }

            // MOV Y, A
            0xFD => {
                self.y = self.a;
                self.update_nz(self.y);
                2
            }

            // MOV dp, A (direct page)
            0xC4 => {
                let addr = self.direct_page() | (self.fetch_byte() as u16);
                self.write(addr, self.a);
                4
            }

            // MOV A, dp (direct page)
            0xE4 => {
                let addr = self.direct_page() | (self.fetch_byte() as u16);
                self.a = self.read(addr);
                self.update_nz(self.a);
                3
            }

            // MOV dp, X (direct page)
            0xD4 => {
                let addr = self.direct_page() | (self.fetch_byte() as u16);
                self.write(addr, self.x);
                4
            }

            // MOV X, dp (direct page)
            0xF8 => {
                let addr = self.direct_page() | (self.fetch_byte() as u16);
                self.x = self.read(addr);
                self.update_nz(self.x);
                3
            }

            // MOV dp, Y (direct page)
            0xCB => {
                let addr = self.direct_page() | (self.fetch_byte() as u16);
                self.write(addr, self.y);
                4
            }

            // MOV Y, dp (direct page)
            0xEB => {
                let addr = self.direct_page() | (self.fetch_byte() as u16);
                self.y = self.read(addr);
                self.update_nz(self.y);
                3
            }

            // MOV !abs, A (absolute)
            0xC5 => {
                let addr = self.fetch_word();
                self.write(addr, self.a);
                5
            }

            // MOV A, !abs (absolute)
            0xE5 => {
                let addr = self.fetch_word();
                self.a = self.read(addr);
                self.update_nz(self.a);
                4
            }

            // MOV A, !abs+X (absolute indexed by X)
            0xF5 => {
                let addr = self.fetch_word().wrapping_add(self.x as u16);
                self.a = self.read(addr);
                self.update_nz(self.a);
                5
            }

            // MOV A, !abs+Y (absolute indexed by Y)
            0xF6 => {
                let addr = self.fetch_word().wrapping_add(self.y as u16);
                self.a = self.read(addr);
                self.update_nz(self.a);
                5
            }

            // MOV !abs+X, A (absolute indexed by X)
            0xD5 => {
                let addr = self.fetch_word().wrapping_add(self.x as u16);
                self.write(addr, self.a);
                6
            }

            // MOV !abs+Y, A (absolute indexed by Y)
            0xD6 => {
                let addr = self.fetch_word().wrapping_add(self.y as u16);
                self.write(addr, self.a);
                6
            }

            // MOV (X)+, A - Move A to address in X, then increment X
            0xAF => {
                let addr = self.direct_page() | (self.x as u16);
                self.write(addr, self.a);
                self.x = self.x.wrapping_add(1);
                4
            }

            // MOV A, (X)+ - Move from address in X to A, then increment X
            0xBF => {
                let addr = self.direct_page() | (self.x as u16);
                self.a = self.read(addr);
                self.update_nz(self.a);
                self.x = self.x.wrapping_add(1);
                4
            }

            // SLEEP - halt CPU until interrupt
            0xEF => {
                // For now, just NOP
                log(LogCategory::Bus, LogLevel::Debug, || {
                    "SPC700: SLEEP instruction executed".to_string()
                });
                3
            }

            // STOP - halt CPU and oscillator
            0xFF => {
                log(LogCategory::Bus, LogLevel::Debug, || {
                    "SPC700: STOP instruction executed".to_string()
                });
                3
            }

            // MOV SP, X - Move X to stack pointer
            0xBD => {
                self.sp = self.x;
                2
            }

            // MOV (X), A - Store A at address in X (indirect X)
            0xC6 => {
                self.memory.write(self.x as u16, self.a);
                4
            }

            // MOV dp, #imm - Move immediate to direct page
            0x8F => {
                let val = self.fetch_byte();
                let dp = self.fetch_byte();
                let addr = self.direct_page() | (dp as u16);
                self.write(addr, val);
                5
            }

            // SETC - Set carry flag
            0x80 => {
                self.set_carry(true);
                2
            }

            // MOV A, dp+X - Move direct page indexed by X to A
            0xF4 => {
                let dp = self.fetch_byte();
                let addr = (dp.wrapping_add(self.x)) as u16;
                self.a = self.read(addr);
                self.update_nz(self.a);
                4
            }

            // INC dp+X - Increment direct page indexed by X
            0xBB => {
                let dp = self.fetch_byte();
                let addr = (dp.wrapping_add(self.x)) as u16;
                let val = self.read(addr).wrapping_add(1);
                self.write(addr, val);
                self.update_nz(val);
                5
            }

            // MOV1 C, mem.bit - Move bit to carry (bit operations)
            0xAA => {
                // Fetch 13-bit address: low 8 bits, then high 5 bits + 3-bit number
                let addr_low = self.fetch_byte();
                let addr_high_and_bit = self.fetch_byte();
                let addr = ((addr_high_and_bit as u16 & 0x1F) << 8) | addr_low as u16;
                let bit_num = (addr_high_and_bit >> 5) & 0x07;

                let val = self.read(addr);
                let bit = (val >> bit_num) & 1;
                self.set_carry(bit != 0);
                4
            }

            // CMP $F4, #imm - Compare direct page with immediate
            0x78 => {
                let imm = self.fetch_byte();
                let addr = self.fetch_byte() as u16;
                let val = self.memory.read(addr);
                let result = val.wrapping_sub(imm);
                self.update_nz(result);
                self.set_carry(val >= imm);
                4
            }

            // CMP Y, $F4 - Compare Y with direct page
            0x7E => {
                let addr = self.fetch_byte() as u16;
                let val = self.memory.read(addr);
                let result = self.y.wrapping_sub(val);
                self.update_nz(result);
                self.set_carry(self.y >= val);
                3
            }

            // MOV ($00)+Y, A - Move A to (direct page + Y)
            0xD7 => {
                let dp = self.fetch_byte() as u16;
                let addr_lo = self.memory.read(dp);
                let addr_hi = self.memory.read(dp.wrapping_add(1));
                let base_addr = ((addr_hi as u16) << 8) | (addr_lo as u16);
                let addr = base_addr.wrapping_add(self.y as u16);
                self.memory.write(addr, self.a);
                7
            }

            // INC $01 - Increment direct page
            0xAB => {
                let addr = self.fetch_byte() as u16;
                let val = self.memory.read(addr);
                let result = val.wrapping_add(1);
                self.memory.write(addr, result);
                self.update_nz(result);
                4
            }

            // BPL rel - Branch if plus (N flag clear)
            0x10 => {
                let offset = self.fetch_byte() as i8;
                if self.psw & psw_flags::NEGATIVE == 0 {
                    self.pc = self.pc.wrapping_add(offset as u16);
                    4
                } else {
                    2
                }
            }

            // RET - Return from subroutine
            0x6F => {
                let lo = self.pop();
                let hi = self.pop();
                self.pc = ((hi as u16) << 8) | (lo as u16);
                5
            }

            // CALL !abs - Call subroutine at absolute address
            0x3F => {
                let addr = self.fetch_word();
                let ret_addr = self.pc;
                self.push((ret_addr >> 8) as u8);
                self.push((ret_addr & 0xFF) as u8);
                self.pc = addr;
                8
            }

            // JMP !abs - Jump to absolute address
            0x5F => {
                self.pc = self.fetch_word();
                3
            }

            // JMP (! abs+X) - Jump indirect indexed
            0x1F => {
                let addr = self.fetch_word().wrapping_add(self.x as u16);
                self.pc = self.read_word(addr);
                6
            }

            // BMI rel - Branch if minus (N flag set)
            0x30 => {
                let offset = self.fetch_byte() as i8;
                if self.get_flag(psw_flags::NEGATIVE) {
                    self.pc = self.pc.wrapping_add(offset as u16);
                    4
                } else {
                    2
                }
            }

            // BVS rel - Branch if overflow set
            0x70 => {
                let offset = self.fetch_byte() as i8;
                if self.get_flag(psw_flags::OVERFLOW) {
                    self.pc = self.pc.wrapping_add(offset as u16);
                    4
                } else {
                    2
                }
            }

            // BVC rel - Branch if overflow clear
            0x50 => {
                let offset = self.fetch_byte() as i8;
                if !self.get_flag(psw_flags::OVERFLOW) {
                    self.pc = self.pc.wrapping_add(offset as u16);
                    4
                } else {
                    2
                }
            }

            // BCC rel - Branch if carry clear
            0x90 => {
                let offset = self.fetch_byte() as i8;
                if !self.get_flag(psw_flags::CARRY) {
                    self.pc = self.pc.wrapping_add(offset as u16);
                    4
                } else {
                    2
                }
            }

            // BCS rel - Branch if carry set
            0xB0 => {
                let offset = self.fetch_byte() as i8;
                if self.get_flag(psw_flags::CARRY) {
                    self.pc = self.pc.wrapping_add(offset as u16);
                    4
                } else {
                    2
                }
            }

            // PUSH A
            0x2D => {
                self.push(self.a);
                4
            }

            // PUSH X
            0x4D => {
                self.push(self.x);
                4
            }

            // PUSH Y
            0x6D => {
                self.push(self.y);
                4
            }

            // PUSH PSW
            0x0D => {
                self.push(self.psw);
                4
            }

            // POP A
            0xAE => {
                self.a = self.pop();
                self.update_nz(self.a);
                4
            }

            // POP X
            0xCE => {
                self.x = self.pop();
                self.update_nz(self.x);
                4
            }

            // POP Y
            0xEE => {
                self.y = self.pop();
                self.update_nz(self.y);
                4
            }

            // POP PSW
            0x8E => {
                self.psw = self.pop();
                4
            }

            // AND A, #imm
            0x28 => {
                let val = self.fetch_byte();
                self.a &= val;
                self.update_nz(self.a);
                2
            }

            // AND A, dp
            0x24 => {
                let addr = self.direct_page() | (self.fetch_byte() as u16);
                self.a &= self.read(addr);
                self.update_nz(self.a);
                3
            }

            // AND A, !abs
            0x25 => {
                let addr = self.fetch_word();
                self.a &= self.read(addr);
                self.update_nz(self.a);
                4
            }

            // AND A, (X)
            0x26 => {
                self.a &= self.read(self.x as u16);
                self.update_nz(self.a);
                3
            }

            // AND A, dp+X
            0x34 => {
                let dp = self.fetch_byte();
                self.a &= self.read(dp.wrapping_add(self.x) as u16);
                self.update_nz(self.a);
                4
            }

            // AND A, !abs+X
            0x35 => {
                let addr = self.fetch_word().wrapping_add(self.x as u16);
                self.a &= self.read(addr);
                self.update_nz(self.a);
                5
            }

            // AND A, !abs+Y
            0x36 => {
                let addr = self.fetch_word().wrapping_add(self.y as u16);
                self.a &= self.read(addr);
                self.update_nz(self.a);
                5
            }

            // OR A, #imm
            0x08 => {
                let val = self.fetch_byte();
                self.a |= val;
                self.update_nz(self.a);
                2
            }

            // OR A, dp
            0x04 => {
                let addr = self.direct_page() | (self.fetch_byte() as u16);
                self.a |= self.read(addr);
                self.update_nz(self.a);
                3
            }

            // OR A, !abs
            0x05 => {
                let addr = self.fetch_word();
                self.a |= self.read(addr);
                self.update_nz(self.a);
                4
            }

            // OR A, (X)
            0x06 => {
                self.a |= self.read(self.x as u16);
                self.update_nz(self.a);
                3
            }

            // OR A, dp+X
            0x14 => {
                let dp = self.fetch_byte();
                self.a |= self.read(dp.wrapping_add(self.x) as u16);
                self.update_nz(self.a);
                4
            }

            // OR A, !abs+X
            0x15 => {
                let addr = self.fetch_word().wrapping_add(self.x as u16);
                self.a |= self.read(addr);
                self.update_nz(self.a);
                5
            }

            // OR A, !abs+Y
            0x16 => {
                let addr = self.fetch_word().wrapping_add(self.y as u16);
                self.a |= self.read(addr);
                self.update_nz(self.a);
                5
            }

            // EOR A, #imm
            0x48 => {
                let val = self.fetch_byte();
                self.a ^= val;
                self.update_nz(self.a);
                2
            }

            // EOR A, dp
            0x44 => {
                let addr = self.direct_page() | (self.fetch_byte() as u16);
                self.a ^= self.read(addr);
                self.update_nz(self.a);
                3
            }

            // EOR A, !abs
            0x45 => {
                let addr = self.fetch_word();
                self.a ^= self.read(addr);
                self.update_nz(self.a);
                4
            }

            // EOR A, (X)
            0x46 => {
                self.a ^= self.read(self.x as u16);
                self.update_nz(self.a);
                3
            }

            // EOR A, dp+X
            0x54 => {
                let dp = self.fetch_byte();
                self.a ^= self.read(dp.wrapping_add(self.x) as u16);
                self.update_nz(self.a);
                4
            }

            // EOR A, !abs+X
            0x55 => {
                let addr = self.fetch_word().wrapping_add(self.x as u16);
                self.a ^= self.read(addr);
                self.update_nz(self.a);
                5
            }

            // EOR A, !abs+Y
            0x56 => {
                let addr = self.fetch_word().wrapping_add(self.y as u16);
                self.a ^= self.read(addr);
                self.update_nz(self.a);
                5
            }

            // ADC A, #imm
            0x88 => {
                let val = self.fetch_byte();
                let carry = if self.get_flag(psw_flags::CARRY) {
                    1
                } else {
                    0
                };
                let result = self.a as u16 + val as u16 + carry;
                self.set_flag(psw_flags::CARRY, result > 0xFF);
                self.set_flag(
                    psw_flags::OVERFLOW,
                    (!(self.a ^ val) & (self.a ^ result as u8) & 0x80) != 0,
                );
                self.set_flag(
                    psw_flags::HALF_CARRY,
                    ((self.a & 0x0F) + (val & 0x0F) + carry as u8) > 0x0F,
                );
                self.a = result as u8;
                self.update_nz(self.a);
                2
            }

            // ADC A, dp
            0x84 => {
                let addr = self.direct_page() | (self.fetch_byte() as u16);
                let val = self.read(addr);
                let carry = if self.get_flag(psw_flags::CARRY) {
                    1
                } else {
                    0
                };
                let result = self.a as u16 + val as u16 + carry;
                self.set_flag(psw_flags::CARRY, result > 0xFF);
                self.set_flag(
                    psw_flags::OVERFLOW,
                    (!(self.a ^ val) & (self.a ^ result as u8) & 0x80) != 0,
                );
                self.set_flag(
                    psw_flags::HALF_CARRY,
                    ((self.a & 0x0F) + (val & 0x0F) + carry as u8) > 0x0F,
                );
                self.a = result as u8;
                self.update_nz(self.a);
                3
            }

            // ADC A, !abs
            0x85 => {
                let addr = self.fetch_word();
                let val = self.read(addr);
                let carry = if self.get_flag(psw_flags::CARRY) {
                    1
                } else {
                    0
                };
                let result = self.a as u16 + val as u16 + carry;
                self.set_flag(psw_flags::CARRY, result > 0xFF);
                self.set_flag(
                    psw_flags::OVERFLOW,
                    (!(self.a ^ val) & (self.a ^ result as u8) & 0x80) != 0,
                );
                self.set_flag(
                    psw_flags::HALF_CARRY,
                    ((self.a & 0x0F) + (val & 0x0F) + carry as u8) > 0x0F,
                );
                self.a = result as u8;
                self.update_nz(self.a);
                4
            }

            // SBC A, #imm
            0xA8 => {
                let val = self.fetch_byte();
                let carry = if self.get_flag(psw_flags::CARRY) {
                    0
                } else {
                    1
                };
                let result = self.a as i16 - val as i16 - carry as i16;
                self.set_flag(psw_flags::CARRY, result >= 0);
                self.set_flag(
                    psw_flags::OVERFLOW,
                    ((self.a ^ val) & (self.a ^ result as u8) & 0x80) != 0,
                );
                self.set_flag(
                    psw_flags::HALF_CARRY,
                    ((self.a & 0x0F) as i16 - (val & 0x0F) as i16 - carry as i16) < 0,
                );
                self.a = result as u8;
                self.update_nz(self.a);
                2
            }

            // SBC A, dp
            0xA4 => {
                let addr = self.direct_page() | (self.fetch_byte() as u16);
                let val = self.read(addr);
                let carry = if self.get_flag(psw_flags::CARRY) {
                    0
                } else {
                    1
                };
                let result = self.a as i16 - val as i16 - carry as i16;
                self.set_flag(psw_flags::CARRY, result >= 0);
                self.set_flag(
                    psw_flags::OVERFLOW,
                    ((self.a ^ val) & (self.a ^ result as u8) & 0x80) != 0,
                );
                self.set_flag(
                    psw_flags::HALF_CARRY,
                    ((self.a & 0x0F) as i16 - (val & 0x0F) as i16 - carry as i16) < 0,
                );
                self.a = result as u8;
                self.update_nz(self.a);
                3
            }

            // SBC A, !abs
            0xA5 => {
                let addr = self.fetch_word();
                let val = self.read(addr);
                let carry = if self.get_flag(psw_flags::CARRY) {
                    0
                } else {
                    1
                };
                let result = self.a as i16 - val as i16 - carry as i16;
                self.set_flag(psw_flags::CARRY, result >= 0);
                self.set_flag(
                    psw_flags::OVERFLOW,
                    ((self.a ^ val) & (self.a ^ result as u8) & 0x80) != 0,
                );
                self.set_flag(
                    psw_flags::HALF_CARRY,
                    ((self.a & 0x0F) as i16 - (val & 0x0F) as i16 - carry as i16) < 0,
                );
                self.a = result as u8;
                self.update_nz(self.a);
                4
            }

            // CMP X, #imm
            0xC8 => {
                let val = self.fetch_byte();
                let result = self.x.wrapping_sub(val);
                self.update_nz(result);
                self.set_flag(psw_flags::CARRY, self.x >= val);
                2
            }

            // CMP X, dp
            0x3E => {
                let addr = self.direct_page() | (self.fetch_byte() as u16);
                let val = self.read(addr);
                let result = self.x.wrapping_sub(val);
                self.update_nz(result);
                self.set_flag(psw_flags::CARRY, self.x >= val);
                3
            }

            // CMP Y, #imm
            0xAD => {
                let val = self.fetch_byte();
                let result = self.y.wrapping_sub(val);
                self.update_nz(result);
                self.set_flag(psw_flags::CARRY, self.y >= val);
                2
            }

            // CMP A, !abs+X
            0x75 => {
                let addr = self.fetch_word().wrapping_add(self.x as u16);
                let val = self.read(addr);
                let result = self.a.wrapping_sub(val);
                self.update_nz(result);
                self.set_flag(psw_flags::CARRY, self.a >= val);
                5
            }

            // CMP A, !abs+Y
            0x76 => {
                let addr = self.fetch_word().wrapping_add(self.y as u16);
                let val = self.read(addr);
                let result = self.a.wrapping_sub(val);
                self.update_nz(result);
                self.set_flag(psw_flags::CARRY, self.a >= val);
                5
            }

            // ASL A
            0x1C => {
                let carry = (self.a & 0x80) != 0;
                self.a <<= 1;
                self.set_carry(carry);
                self.update_nz(self.a);
                2
            }

            // ASL dp
            0x0B => {
                let addr = self.direct_page() | (self.fetch_byte() as u16);
                let val = self.read(addr);
                let carry = (val & 0x80) != 0;
                let result = val << 1;
                self.write(addr, result);
                self.set_carry(carry);
                self.update_nz(result);
                4
            }

            // LSR A
            0x5C => {
                let carry = (self.a & 0x01) != 0;
                self.a >>= 1;
                self.set_carry(carry);
                self.update_nz(self.a);
                2
            }

            // LSR dp
            0x4B => {
                let addr = self.direct_page() | (self.fetch_byte() as u16);
                let val = self.read(addr);
                let carry = (val & 0x01) != 0;
                let result = val >> 1;
                self.write(addr, result);
                self.set_carry(carry);
                self.update_nz(result);
                4
            }

            // ROL A
            0x3C => {
                let old_carry = if self.get_flag(psw_flags::CARRY) {
                    1
                } else {
                    0
                };
                let new_carry = (self.a & 0x80) != 0;
                self.a = (self.a << 1) | old_carry;
                self.set_carry(new_carry);
                self.update_nz(self.a);
                2
            }

            // ROL dp
            0x2B => {
                let addr = self.direct_page() | (self.fetch_byte() as u16);
                let val = self.read(addr);
                let old_carry = if self.get_flag(psw_flags::CARRY) {
                    1
                } else {
                    0
                };
                let new_carry = (val & 0x80) != 0;
                let result = (val << 1) | old_carry;
                self.write(addr, result);
                self.set_carry(new_carry);
                self.update_nz(result);
                4
            }

            // ROR A
            0x7C => {
                let old_carry = if self.get_flag(psw_flags::CARRY) {
                    0x80
                } else {
                    0
                };
                let new_carry = (self.a & 0x01) != 0;
                self.a = (self.a >> 1) | old_carry;
                self.set_carry(new_carry);
                self.update_nz(self.a);
                2
            }

            // ROR dp
            0x6B => {
                let addr = self.direct_page() | (self.fetch_byte() as u16);
                let val = self.read(addr);
                let old_carry = if self.get_flag(psw_flags::CARRY) {
                    0x80
                } else {
                    0
                };
                let new_carry = (val & 0x01) != 0;
                let result = (val >> 1) | old_carry;
                self.write(addr, result);
                self.set_carry(new_carry);
                self.update_nz(result);
                4
            }

            // MOVW YA, dp - Move 16-bit word from memory to YA
            0xBA => {
                let addr = self.direct_page() | (self.fetch_byte() as u16);
                self.a = self.read(addr);
                self.y = self.read(addr.wrapping_add(1));
                self.update_nz(self.y); // Only Y affects flags
                5
            }

            // MOVW dp, YA - Move 16-bit word from YA to memory
            0xDA => {
                let addr = self.direct_page() | (self.fetch_byte() as u16);
                self.write(addr, self.a);
                self.write(addr.wrapping_add(1), self.y);
                5
            }

            // INCW dp - Increment 16-bit word
            0x3A => {
                let addr = self.direct_page() | (self.fetch_byte() as u16);
                let val = self.read_word(addr);
                let result = val.wrapping_add(1);
                self.write_word(addr, result);
                self.set_flag(psw_flags::ZERO, result == 0);
                self.set_flag(psw_flags::NEGATIVE, (result & 0x8000) != 0);
                6
            }

            // DECW dp - Decrement 16-bit word
            0x1A => {
                let addr = self.direct_page() | (self.fetch_byte() as u16);
                let val = self.read_word(addr);
                let result = val.wrapping_sub(1);
                self.write_word(addr, result);
                self.set_flag(psw_flags::ZERO, result == 0);
                self.set_flag(psw_flags::NEGATIVE, (result & 0x8000) != 0);
                6
            }

            // ADDW YA, dp - Add 16-bit word to YA
            0x7A => {
                let addr = self.direct_page() | (self.fetch_byte() as u16);
                let val = self.read_word(addr);
                let ya = ((self.y as u16) << 8) | (self.a as u16);
                let result = ya.wrapping_add(val);
                self.a = (result & 0xFF) as u8;
                self.y = (result >> 8) as u8;
                // Carry is set if there's overflow in 16-bit addition
                let (_, overflow) = ya.overflowing_add(val);
                self.set_flag(psw_flags::CARRY, overflow);
                self.set_flag(psw_flags::ZERO, result == 0);
                self.set_flag(psw_flags::NEGATIVE, (result & 0x8000) != 0);
                self.set_flag(
                    psw_flags::HALF_CARRY,
                    ((ya & 0x0FFF) + (val & 0x0FFF)) > 0x0FFF,
                );
                self.set_flag(
                    psw_flags::OVERFLOW,
                    (!(ya ^ val) & (ya ^ result) & 0x8000) != 0,
                );
                5
            }

            // SUBW YA, dp - Subtract 16-bit word from YA
            0x9A => {
                let addr = self.direct_page() | (self.fetch_byte() as u16);
                let val = self.read_word(addr);
                let ya = ((self.y as u16) << 8) | (self.a as u16);
                let result = ya.wrapping_sub(val);
                self.a = (result & 0xFF) as u8;
                self.y = (result >> 8) as u8;
                self.set_flag(psw_flags::CARRY, ya >= val);
                self.set_flag(psw_flags::ZERO, result == 0);
                self.set_flag(psw_flags::NEGATIVE, (result & 0x8000) != 0);
                self.set_flag(psw_flags::HALF_CARRY, (ya & 0x0FFF) < (val & 0x0FFF));
                self.set_flag(
                    psw_flags::OVERFLOW,
                    ((ya ^ val) & (ya ^ result) & 0x8000) != 0,
                );
                5
            }

            // CMPW YA, dp - Compare 16-bit word with YA
            0x5A => {
                let addr = self.direct_page() | (self.fetch_byte() as u16);
                let val = self.read_word(addr);
                let ya = ((self.y as u16) << 8) | (self.a as u16);
                let result = ya.wrapping_sub(val);
                self.set_flag(psw_flags::ZERO, result == 0);
                self.set_flag(psw_flags::NEGATIVE, (result & 0x8000) != 0);
                self.set_flag(psw_flags::CARRY, ya >= val);
                4
            }

            // MUL YA - Multiply Y * A -> YA
            0xCF => {
                let result = (self.y as u16) * (self.a as u16);
                self.a = (result & 0xFF) as u8;
                self.y = (result >> 8) as u8;
                self.update_nz(self.y);
                9
            }

            // DIV YA, X - Divide YA by X
            0x9E => {
                let ya = ((self.y as u16) << 8) | (self.a as u16);
                if self.x == 0 {
                    // Division by zero
                    self.a = 0xFF;
                    self.y = 0xFF;
                    self.set_flag(psw_flags::OVERFLOW, true);
                    self.set_flag(psw_flags::HALF_CARRY, true);
                } else {
                    let quotient = ya / (self.x as u16);
                    let remainder = ya % (self.x as u16);
                    self.a = (quotient & 0xFF) as u8;
                    self.y = (remainder & 0xFF) as u8;
                    self.set_flag(psw_flags::OVERFLOW, quotient > 0xFF);
                    self.set_flag(psw_flags::HALF_CARRY, (self.y & 0x0F) >= (self.x & 0x0F));
                }
                self.update_nz(self.a);
                12
            }

            // XCN A - Exchange nibbles in A
            0x9F => {
                self.a = self.a.rotate_left(4);
                self.update_nz(self.a);
                5
            }

            // CLRC - Clear carry flag
            0x60 => {
                self.set_carry(false);
                2
            }

            // CLRV - Clear overflow and half-carry flags
            0xE0 => {
                self.psw &= !(psw_flags::OVERFLOW | psw_flags::HALF_CARRY);
                2
            }

            // NOTC - Complement carry flag
            0xED => {
                self.set_carry(!self.get_flag(psw_flags::CARRY));
                3
            }

            // EI - Enable interrupts
            0xA0 => {
                self.psw |= psw_flags::INTERRUPT;
                3
            }

            // DI - Disable interrupts
            0xC0 => {
                self.psw &= !psw_flags::INTERRUPT;
                3
            }

            // MOV X, SP - Move stack pointer to X
            0x9D => {
                self.x = self.sp;
                self.update_nz(self.x);
                2
            }

            // DEC dp
            0x8B => {
                let addr = self.direct_page() | (self.fetch_byte() as u16);
                let val = self.read(addr);
                let result = val.wrapping_sub(1);
                self.write(addr, result);
                self.update_nz(result);
                4
            }

            // DEC !abs
            0x8C => {
                let addr = self.fetch_word();
                let val = self.read(addr);
                let result = val.wrapping_sub(1);
                self.write(addr, result);
                self.update_nz(result);
                5
            }

            // INC !abs
            0xAC => {
                let addr = self.fetch_word();
                let val = self.read(addr);
                let result = val.wrapping_add(1);
                self.write(addr, result);
                self.update_nz(result);
                5
            }

            // TCALL family (Table calls) - 16 opcodes
            // TCALL uses table at $FFDE (for TCALL 0) and decrements by 2 for each
            0x01 => {
                let addr = self.read_word(0xFFDE);
                let ret = self.pc;
                self.push((ret >> 8) as u8);
                self.push((ret & 0xFF) as u8);
                self.pc = addr;
                8
            }
            0x11 => {
                let addr = self.read_word(0xFFDC);
                let ret = self.pc;
                self.push((ret >> 8) as u8);
                self.push((ret & 0xFF) as u8);
                self.pc = addr;
                8
            }
            0x21 => {
                let addr = self.read_word(0xFFDA);
                let ret = self.pc;
                self.push((ret >> 8) as u8);
                self.push((ret & 0xFF) as u8);
                self.pc = addr;
                8
            }
            0x31 => {
                let addr = self.read_word(0xFFD8);
                let ret = self.pc;
                self.push((ret >> 8) as u8);
                self.push((ret & 0xFF) as u8);
                self.pc = addr;
                8
            }
            0x41 => {
                let addr = self.read_word(0xFFD6);
                let ret = self.pc;
                self.push((ret >> 8) as u8);
                self.push((ret & 0xFF) as u8);
                self.pc = addr;
                8
            }
            0x51 => {
                let addr = self.read_word(0xFFD4);
                let ret = self.pc;
                self.push((ret >> 8) as u8);
                self.push((ret & 0xFF) as u8);
                self.pc = addr;
                8
            }
            0x61 => {
                let addr = self.read_word(0xFFD2);
                let ret = self.pc;
                self.push((ret >> 8) as u8);
                self.push((ret & 0xFF) as u8);
                self.pc = addr;
                8
            }
            0x71 => {
                let addr = self.read_word(0xFFD0);
                let ret = self.pc;
                self.push((ret >> 8) as u8);
                self.push((ret & 0xFF) as u8);
                self.pc = addr;
                8
            }
            0x81 => {
                let addr = self.read_word(0xFFCE);
                let ret = self.pc;
                self.push((ret >> 8) as u8);
                self.push((ret & 0xFF) as u8);
                self.pc = addr;
                8
            }
            0x91 => {
                let addr = self.read_word(0xFFCC);
                let ret = self.pc;
                self.push((ret >> 8) as u8);
                self.push((ret & 0xFF) as u8);
                self.pc = addr;
                8
            }
            0xA1 => {
                let addr = self.read_word(0xFFCA);
                let ret = self.pc;
                self.push((ret >> 8) as u8);
                self.push((ret & 0xFF) as u8);
                self.pc = addr;
                8
            }
            0xB1 => {
                let addr = self.read_word(0xFFC8);
                let ret = self.pc;
                self.push((ret >> 8) as u8);
                self.push((ret & 0xFF) as u8);
                self.pc = addr;
                8
            }
            0xC1 => {
                let addr = self.read_word(0xFFC6);
                let ret = self.pc;
                self.push((ret >> 8) as u8);
                self.push((ret & 0xFF) as u8);
                self.pc = addr;
                8
            }
            0xD1 => {
                let addr = self.read_word(0xFFC4);
                let ret = self.pc;
                self.push((ret >> 8) as u8);
                self.push((ret & 0xFF) as u8);
                self.pc = addr;
                8
            }
            0xE1 => {
                let addr = self.read_word(0xFFC2);
                let ret = self.pc;
                self.push((ret >> 8) as u8);
                self.push((ret & 0xFF) as u8);
                self.pc = addr;
                8
            }
            0xF1 => {
                let addr = self.read_word(0xFFC0);
                let ret = self.pc;
                self.push((ret >> 8) as u8);
                self.push((ret & 0xFF) as u8);
                self.pc = addr;
                8
            }

            // SET1/CLR1 - Set/clear bit in direct page (8 variants each)
            0x02 => {
                let addr = self.direct_page() | (self.fetch_byte() as u16);
                let val = self.read(addr) | 0x01;
                self.write(addr, val);
                4
            }
            0x22 => {
                let addr = self.direct_page() | (self.fetch_byte() as u16);
                let val = self.read(addr) | 0x02;
                self.write(addr, val);
                4
            }
            0x42 => {
                let addr = self.direct_page() | (self.fetch_byte() as u16);
                let val = self.read(addr) | 0x04;
                self.write(addr, val);
                4
            }
            0x62 => {
                let addr = self.direct_page() | (self.fetch_byte() as u16);
                let val = self.read(addr) | 0x08;
                self.write(addr, val);
                4
            }
            0x82 => {
                let addr = self.direct_page() | (self.fetch_byte() as u16);
                let val = self.read(addr) | 0x10;
                self.write(addr, val);
                4
            }
            0xA2 => {
                let addr = self.direct_page() | (self.fetch_byte() as u16);
                let val = self.read(addr) | 0x20;
                self.write(addr, val);
                4
            }
            0xC2 => {
                let addr = self.direct_page() | (self.fetch_byte() as u16);
                let val = self.read(addr) | 0x40;
                self.write(addr, val);
                4
            }
            0xE2 => {
                let addr = self.direct_page() | (self.fetch_byte() as u16);
                let val = self.read(addr) | 0x80;
                self.write(addr, val);
                4
            }

            0x12 => {
                let addr = self.direct_page() | (self.fetch_byte() as u16);
                let val = self.read(addr) & !0x01;
                self.write(addr, val);
                4
            }
            0x32 => {
                let addr = self.direct_page() | (self.fetch_byte() as u16);
                let val = self.read(addr) & !0x02;
                self.write(addr, val);
                4
            }
            0x52 => {
                let addr = self.direct_page() | (self.fetch_byte() as u16);
                let val = self.read(addr) & !0x04;
                self.write(addr, val);
                4
            }
            0x72 => {
                let addr = self.direct_page() | (self.fetch_byte() as u16);
                let val = self.read(addr) & !0x08;
                self.write(addr, val);
                4
            }
            0x92 => {
                let addr = self.direct_page() | (self.fetch_byte() as u16);
                let val = self.read(addr) & !0x10;
                self.write(addr, val);
                4
            }
            0xB2 => {
                let addr = self.direct_page() | (self.fetch_byte() as u16);
                let val = self.read(addr) & !0x20;
                self.write(addr, val);
                4
            }
            0xD2 => {
                let addr = self.direct_page() | (self.fetch_byte() as u16);
                let val = self.read(addr) & !0x40;
                self.write(addr, val);
                4
            }
            0xF2 => {
                let addr = self.direct_page() | (self.fetch_byte() as u16);
                let val = self.read(addr) & !0x80;
                self.write(addr, val);
                4
            }

            // BBS/BBC - Branch if bit set/clear (8 variants each)
            0x03 => {
                let dp = self.fetch_byte();
                let offset = self.fetch_byte() as i8;
                let addr = self.direct_page() | (dp as u16);
                if (self.read(addr) & 0x01) != 0 {
                    self.pc = self.pc.wrapping_add(offset as u16);
                    7
                } else {
                    5
                }
            }
            0x23 => {
                let dp = self.fetch_byte();
                let offset = self.fetch_byte() as i8;
                let addr = self.direct_page() | (dp as u16);
                if (self.read(addr) & 0x02) != 0 {
                    self.pc = self.pc.wrapping_add(offset as u16);
                    7
                } else {
                    5
                }
            }
            0x43 => {
                let dp = self.fetch_byte();
                let offset = self.fetch_byte() as i8;
                let addr = self.direct_page() | (dp as u16);
                if (self.read(addr) & 0x04) != 0 {
                    self.pc = self.pc.wrapping_add(offset as u16);
                    7
                } else {
                    5
                }
            }
            0x63 => {
                let dp = self.fetch_byte();
                let offset = self.fetch_byte() as i8;
                let addr = self.direct_page() | (dp as u16);
                if (self.read(addr) & 0x08) != 0 {
                    self.pc = self.pc.wrapping_add(offset as u16);
                    7
                } else {
                    5
                }
            }
            0x83 => {
                let dp = self.fetch_byte();
                let offset = self.fetch_byte() as i8;
                let addr = self.direct_page() | (dp as u16);
                if (self.read(addr) & 0x10) != 0 {
                    self.pc = self.pc.wrapping_add(offset as u16);
                    7
                } else {
                    5
                }
            }
            0xA3 => {
                let dp = self.fetch_byte();
                let offset = self.fetch_byte() as i8;
                let addr = self.direct_page() | (dp as u16);
                if (self.read(addr) & 0x20) != 0 {
                    self.pc = self.pc.wrapping_add(offset as u16);
                    7
                } else {
                    5
                }
            }
            0xC3 => {
                let dp = self.fetch_byte();
                let offset = self.fetch_byte() as i8;
                let addr = self.direct_page() | (dp as u16);
                if (self.read(addr) & 0x40) != 0 {
                    self.pc = self.pc.wrapping_add(offset as u16);
                    7
                } else {
                    5
                }
            }
            0xE3 => {
                let dp = self.fetch_byte();
                let offset = self.fetch_byte() as i8;
                let addr = self.direct_page() | (dp as u16);
                if (self.read(addr) & 0x80) != 0 {
                    self.pc = self.pc.wrapping_add(offset as u16);
                    7
                } else {
                    5
                }
            }

            0x13 => {
                let dp = self.fetch_byte();
                let offset = self.fetch_byte() as i8;
                let addr = self.direct_page() | (dp as u16);
                if (self.read(addr) & 0x01) == 0 {
                    self.pc = self.pc.wrapping_add(offset as u16);
                    7
                } else {
                    5
                }
            }
            0x33 => {
                let dp = self.fetch_byte();
                let offset = self.fetch_byte() as i8;
                let addr = self.direct_page() | (dp as u16);
                if (self.read(addr) & 0x02) == 0 {
                    self.pc = self.pc.wrapping_add(offset as u16);
                    7
                } else {
                    5
                }
            }
            0x53 => {
                let dp = self.fetch_byte();
                let offset = self.fetch_byte() as i8;
                let addr = self.direct_page() | (dp as u16);
                if (self.read(addr) & 0x04) == 0 {
                    self.pc = self.pc.wrapping_add(offset as u16);
                    7
                } else {
                    5
                }
            }
            0x73 => {
                let dp = self.fetch_byte();
                let offset = self.fetch_byte() as i8;
                let addr = self.direct_page() | (dp as u16);
                if (self.read(addr) & 0x08) == 0 {
                    self.pc = self.pc.wrapping_add(offset as u16);
                    7
                } else {
                    5
                }
            }
            0x93 => {
                let dp = self.fetch_byte();
                let offset = self.fetch_byte() as i8;
                let addr = self.direct_page() | (dp as u16);
                if (self.read(addr) & 0x10) == 0 {
                    self.pc = self.pc.wrapping_add(offset as u16);
                    7
                } else {
                    5
                }
            }
            0xB3 => {
                let dp = self.fetch_byte();
                let offset = self.fetch_byte() as i8;
                let addr = self.direct_page() | (dp as u16);
                if (self.read(addr) & 0x20) == 0 {
                    self.pc = self.pc.wrapping_add(offset as u16);
                    7
                } else {
                    5
                }
            }
            0xD3 => {
                let dp = self.fetch_byte();
                let offset = self.fetch_byte() as i8;
                let addr = self.direct_page() | (dp as u16);
                if (self.read(addr) & 0x40) == 0 {
                    self.pc = self.pc.wrapping_add(offset as u16);
                    7
                } else {
                    5
                }
            }
            0xF3 => {
                let dp = self.fetch_byte();
                let offset = self.fetch_byte() as i8;
                let addr = self.direct_page() | (dp as u16);
                if (self.read(addr) & 0x80) == 0 {
                    self.pc = self.pc.wrapping_add(offset as u16);
                    7
                } else {
                    5
                }
            }

            // More MOV variants and other critical opcodes
            // MOV A, (dp)+Y
            0xF7 => {
                let dp = self.fetch_byte() as u16;
                let addr_lo = self.read(dp);
                let addr_hi = self.read(dp.wrapping_add(1));
                let base_addr = ((addr_hi as u16) << 8) | (addr_lo as u16);
                let addr = base_addr.wrapping_add(self.y as u16);
                self.a = self.read(addr);
                self.update_nz(self.a);
                6
            }

            // MOV A, (X)
            0xE6 => {
                self.a = self.read(self.x as u16);
                self.update_nz(self.a);
                3
            }

            // MOV dp, dp
            0xFA => {
                let src_dp = self.fetch_byte();
                let dst_dp = self.fetch_byte();
                let val = self.read(src_dp as u16);
                self.write(dst_dp as u16, val);
                5
            }

            // OR/AND dp, dp
            0x09 => {
                let src_dp = self.fetch_byte();
                let dst_dp = self.fetch_byte();
                let src_val = self.read(src_dp as u16);
                let dst_val = self.read(dst_dp as u16);
                let result = dst_val | src_val;
                self.write(dst_dp as u16, result);
                self.update_nz(result);
                6
            }

            0x29 => {
                let src_dp = self.fetch_byte();
                let dst_dp = self.fetch_byte();
                let src_val = self.read(src_dp as u16);
                let dst_val = self.read(dst_dp as u16);
                let result = dst_val & src_val;
                self.write(dst_dp as u16, result);
                self.update_nz(result);
                6
            }

            // OR/AND dp, #imm
            0x18 => {
                let imm = self.fetch_byte();
                let dp = self.fetch_byte();
                let val = self.read(dp as u16);
                let result = val | imm;
                self.write(dp as u16, result);
                self.update_nz(result);
                5
            }

            0x38 => {
                let imm = self.fetch_byte();
                let dp = self.fetch_byte();
                let val = self.read(dp as u16);
                let result = val & imm;
                self.write(dp as u16, result);
                self.update_nz(result);
                5
            }

            // ADC A, (X)
            0x86 => {
                let val = self.read(self.x as u16);
                let carry = if self.get_flag(psw_flags::CARRY) {
                    1
                } else {
                    0
                };
                let result = self.a as u16 + val as u16 + carry;
                self.set_flag(psw_flags::CARRY, result > 0xFF);
                self.set_flag(
                    psw_flags::OVERFLOW,
                    (!(self.a ^ val) & (self.a ^ result as u8) & 0x80) != 0,
                );
                self.set_flag(
                    psw_flags::HALF_CARRY,
                    ((self.a & 0x0F) + (val & 0x0F) + carry as u8) > 0x0F,
                );
                self.a = result as u8;
                self.update_nz(self.a);
                3
            }

            // ADC A, (dp+X)
            0x87 => {
                let dp = self.fetch_byte() as u16;
                let addr_lo = self.read(dp.wrapping_add(self.x as u16));
                let addr_hi = self.read(dp.wrapping_add(self.x as u16).wrapping_add(1));
                let addr = ((addr_hi as u16) << 8) | (addr_lo as u16);
                let val = self.read(addr);
                let carry = if self.get_flag(psw_flags::CARRY) {
                    1
                } else {
                    0
                };
                let result = self.a as u16 + val as u16 + carry;
                self.set_flag(psw_flags::CARRY, result > 0xFF);
                self.set_flag(
                    psw_flags::OVERFLOW,
                    (!(self.a ^ val) & (self.a ^ result as u8) & 0x80) != 0,
                );
                self.set_flag(
                    psw_flags::HALF_CARRY,
                    ((self.a & 0x0F) + (val & 0x0F) + carry as u8) > 0x0F,
                );
                self.a = result as u8;
                self.update_nz(self.a);
                6
            }

            // ADC A, dp+X
            0x94 => {
                let dp = self.fetch_byte();
                let val = self.read(dp.wrapping_add(self.x) as u16);
                let carry = if self.get_flag(psw_flags::CARRY) {
                    1
                } else {
                    0
                };
                let result = self.a as u16 + val as u16 + carry;
                self.set_flag(psw_flags::CARRY, result > 0xFF);
                self.set_flag(
                    psw_flags::OVERFLOW,
                    (!(self.a ^ val) & (self.a ^ result as u8) & 0x80) != 0,
                );
                self.set_flag(
                    psw_flags::HALF_CARRY,
                    ((self.a & 0x0F) + (val & 0x0F) + carry as u8) > 0x0F,
                );
                self.a = result as u8;
                self.update_nz(self.a);
                4
            }

            // ADC A, !abs+X
            0x95 => {
                let addr = self.fetch_word().wrapping_add(self.x as u16);
                let val = self.read(addr);
                let carry = if self.get_flag(psw_flags::CARRY) {
                    1
                } else {
                    0
                };
                let result = self.a as u16 + val as u16 + carry;
                self.set_flag(psw_flags::CARRY, result > 0xFF);
                self.set_flag(
                    psw_flags::OVERFLOW,
                    (!(self.a ^ val) & (self.a ^ result as u8) & 0x80) != 0,
                );
                self.set_flag(
                    psw_flags::HALF_CARRY,
                    ((self.a & 0x0F) + (val & 0x0F) + carry as u8) > 0x0F,
                );
                self.a = result as u8;
                self.update_nz(self.a);
                5
            }

            // ADC A, !abs+Y
            0x96 => {
                let addr = self.fetch_word().wrapping_add(self.y as u16);
                let val = self.read(addr);
                let carry = if self.get_flag(psw_flags::CARRY) {
                    1
                } else {
                    0
                };
                let result = self.a as u16 + val as u16 + carry;
                self.set_flag(psw_flags::CARRY, result > 0xFF);
                self.set_flag(
                    psw_flags::OVERFLOW,
                    (!(self.a ^ val) & (self.a ^ result as u8) & 0x80) != 0,
                );
                self.set_flag(
                    psw_flags::HALF_CARRY,
                    ((self.a & 0x0F) + (val & 0x0F) + carry as u8) > 0x0F,
                );
                self.a = result as u8;
                self.update_nz(self.a);
                5
            }

            // ADC A, (dp)+Y
            0x97 => {
                let dp = self.fetch_byte() as u16;
                let addr_lo = self.read(dp);
                let addr_hi = self.read(dp.wrapping_add(1));
                let base = ((addr_hi as u16) << 8) | (addr_lo as u16);
                let addr = base.wrapping_add(self.y as u16);
                let val = self.read(addr);
                let carry = if self.get_flag(psw_flags::CARRY) {
                    1
                } else {
                    0
                };
                let result = self.a as u16 + val as u16 + carry;
                self.set_flag(psw_flags::CARRY, result > 0xFF);
                self.set_flag(
                    psw_flags::OVERFLOW,
                    (!(self.a ^ val) & (self.a ^ result as u8) & 0x80) != 0,
                );
                self.set_flag(
                    psw_flags::HALF_CARRY,
                    ((self.a & 0x0F) + (val & 0x0F) + carry as u8) > 0x0F,
                );
                self.a = result as u8;
                self.update_nz(self.a);
                6
            }

            // ADC dp, dp
            0x99 => {
                let src = self.fetch_byte();
                let dst = self.fetch_byte();
                let src_val = self.read(src as u16);
                let dst_val = self.read(dst as u16);
                let carry = if self.get_flag(psw_flags::CARRY) {
                    1
                } else {
                    0
                };
                let result = dst_val as u16 + src_val as u16 + carry;
                self.set_flag(psw_flags::CARRY, result > 0xFF);
                self.set_flag(
                    psw_flags::OVERFLOW,
                    (!(dst_val ^ src_val) & (dst_val ^ result as u8) & 0x80) != 0,
                );
                self.set_flag(
                    psw_flags::HALF_CARRY,
                    ((dst_val & 0x0F) + (src_val & 0x0F) + carry as u8) > 0x0F,
                );
                self.write(dst as u16, result as u8);
                self.update_nz(result as u8);
                6
            }

            0xA6 => {
                let val = self.read(self.x as u16);
                let carry = if self.get_flag(psw_flags::CARRY) {
                    0
                } else {
                    1
                };
                let result = self.a as i16 - val as i16 - carry;
                self.set_flag(psw_flags::CARRY, result >= 0);
                self.a = result as u8;
                self.update_nz(self.a);
                3
            }
            0xA7 => {
                let dp = self.fetch_byte() as u16;
                let addr_lo = self.read(dp);
                let addr_hi = self.read(dp.wrapping_add(1));
                let addr = ((addr_hi as u16) << 8) | (addr_lo as u16);
                let addr = addr.wrapping_add(self.x as u16);
                let val = self.read(addr);
                let carry = if self.get_flag(psw_flags::CARRY) {
                    0
                } else {
                    1
                };
                let result = self.a as i16 - val as i16 - carry;
                self.set_flag(psw_flags::CARRY, result >= 0);
                self.a = result as u8;
                self.update_nz(self.a);
                6
            }
            0xB4 => {
                let dp = self.fetch_byte();
                let val = self.read(dp.wrapping_add(self.x) as u16);
                let carry = if self.get_flag(psw_flags::CARRY) {
                    0
                } else {
                    1
                };
                let result = self.a as i16 - val as i16 - carry;
                self.set_flag(psw_flags::CARRY, result >= 0);
                self.a = result as u8;
                self.update_nz(self.a);
                4
            }
            0xB5 => {
                let addr = self.fetch_word().wrapping_add(self.x as u16);
                let val = self.read(addr);
                let carry = if self.get_flag(psw_flags::CARRY) {
                    0
                } else {
                    1
                };
                let result = self.a as i16 - val as i16 - carry;
                self.set_flag(psw_flags::CARRY, result >= 0);
                self.a = result as u8;
                self.update_nz(self.a);
                5
            }
            0xB6 => {
                let addr = self.fetch_word().wrapping_add(self.y as u16);
                let val = self.read(addr);
                let carry = if self.get_flag(psw_flags::CARRY) {
                    0
                } else {
                    1
                };
                let result = self.a as i16 - val as i16 - carry;
                self.set_flag(psw_flags::CARRY, result >= 0);
                self.a = result as u8;
                self.update_nz(self.a);
                5
            }
            0xB7 => {
                let dp = self.fetch_byte() as u16;
                let addr_lo = self.read(dp);
                let addr_hi = self.read(dp.wrapping_add(1));
                let base = ((addr_hi as u16) << 8) | (addr_lo as u16);
                let addr = base.wrapping_add(self.y as u16);
                let val = self.read(addr);
                let carry = if self.get_flag(psw_flags::CARRY) {
                    0
                } else {
                    1
                };
                let result = self.a as i16 - val as i16 - carry;
                self.set_flag(psw_flags::CARRY, result >= 0);
                self.a = result as u8;
                self.update_nz(self.a);
                6
            }
            0xB9 => {
                let src = self.fetch_byte();
                let dst = self.fetch_byte();
                let src_val = self.read(src as u16);
                let dst_val = self.read(dst as u16);
                let carry = if self.get_flag(psw_flags::CARRY) {
                    0
                } else {
                    1
                };
                let result = dst_val as i16 - src_val as i16 - carry;
                self.set_flag(psw_flags::CARRY, result >= 0);
                self.write(dst as u16, result as u8);
                self.update_nz(result as u8);
                6
            }

            // Remaining opcodes to complete the 256-opcode SPC700 instruction set
            // OR A, (dp+X)
            0x07 => {
                let dp = self.fetch_byte() as u16;
                let addr_lo = self.read(dp.wrapping_add(self.x as u16));
                let addr_hi = self.read(dp.wrapping_add(self.x as u16).wrapping_add(1));
                let addr = ((addr_hi as u16) << 8) | (addr_lo as u16);
                self.a |= self.read(addr);
                self.update_nz(self.a);
                6
            }
            // OR1 C, mem.bit
            0x0A => {
                let addr_low = self.fetch_byte();
                let addr_high_and_bit = self.fetch_byte();
                let addr = ((addr_high_and_bit as u16 & 0x1F) << 8) | addr_low as u16;
                let bit = (addr_high_and_bit >> 5) & 0x07;
                let val = self.read(addr);
                let bit_val = ((val >> bit) & 1) != 0;
                let carry = self.get_flag(psw_flags::CARRY);
                self.set_carry(carry | bit_val);
                5
            }
            // ASL !abs
            0x0C => {
                let addr = self.fetch_word();
                let val = self.read(addr);
                let carry = (val & 0x80) != 0;
                let result = val << 1;
                self.write(addr, result);
                self.set_carry(carry);
                self.update_nz(result);
                5
            }
            // TSET1 !abs - Test and set bits with A
            0x0E => {
                let addr = self.fetch_word();
                let val = self.read(addr);
                let result = self.a.wrapping_sub(val);
                self.update_nz(result);
                self.write(addr, val | self.a);
                6
            }
            // BRK - Software interrupt
            0x0F => {
                let ret = self.pc.wrapping_add(1);
                self.push((ret >> 8) as u8);
                self.push((ret & 0xFF) as u8);
                self.push(self.psw);
                self.psw |= psw_flags::BREAK;
                self.psw &= !psw_flags::INTERRUPT;
                self.pc = self.read_word(0xFFDE);
                8
            }

            // OR A, (dp)+Y
            0x17 => {
                let dp = self.fetch_byte() as u16;
                let addr_lo = self.read(dp);
                let addr_hi = self.read(dp.wrapping_add(1));
                let base = ((addr_hi as u16) << 8) | (addr_lo as u16);
                let addr = base.wrapping_add(self.y as u16);
                self.a |= self.read(addr);
                self.update_nz(self.a);
                6
            }
            // OR (X), (Y)
            0x19 => {
                let x_val = self.read(self.x as u16);
                let y_val = self.read(self.y as u16);
                let result = x_val | y_val;
                self.write(self.x as u16, result);
                self.update_nz(result);
                5
            }
            // ASL dp+X
            0x1B => {
                let dp = self.fetch_byte();
                let addr = dp.wrapping_add(self.x) as u16;
                let val = self.read(addr);
                let carry = (val & 0x80) != 0;
                let result = val << 1;
                self.write(addr, result);
                self.set_carry(carry);
                self.update_nz(result);
                5
            }
            // CMP X, !abs
            0x1E => {
                let addr = self.fetch_word();
                let val = self.read(addr);
                let result = self.x.wrapping_sub(val);
                self.update_nz(result);
                self.set_carry(self.x >= val);
                4
            }

            // AND A, (dp+X)
            0x27 => {
                let dp = self.fetch_byte() as u16;
                let addr_lo = self.read(dp.wrapping_add(self.x as u16));
                let addr_hi = self.read(dp.wrapping_add(self.x as u16).wrapping_add(1));
                let addr = ((addr_hi as u16) << 8) | (addr_lo as u16);
                self.a &= self.read(addr);
                self.update_nz(self.a);
                6
            }
            // AND1 C, mem.bit
            0x2A => {
                let addr_low = self.fetch_byte();
                let addr_high_and_bit = self.fetch_byte();
                let addr = ((addr_high_and_bit as u16 & 0x1F) << 8) | addr_low as u16;
                let bit = (addr_high_and_bit >> 5) & 0x07;
                let val = self.read(addr);
                let bit_val = ((val >> bit) & 1) != 0;
                let carry = self.get_flag(psw_flags::CARRY);
                self.set_carry(carry & bit_val);
                4
            }
            // ROL !abs
            0x2C => {
                let addr = self.fetch_word();
                let val = self.read(addr);
                let old_carry = if self.get_flag(psw_flags::CARRY) {
                    1
                } else {
                    0
                };
                let new_carry = (val & 0x80) != 0;
                let result = (val << 1) | old_carry;
                self.write(addr, result);
                self.set_carry(new_carry);
                self.update_nz(result);
                5
            }
            // CBNE dp, rel - Compare and branch if not equal
            0x2E => {
                let dp = self.fetch_byte();
                let offset = self.fetch_byte() as i8;
                let addr = self.direct_page() | (dp as u16);
                let val = self.read(addr);
                if self.a != val {
                    self.pc = self.pc.wrapping_add(offset as u16);
                    7
                } else {
                    5
                }
            }

            // AND A, (dp)+Y
            0x37 => {
                let dp = self.fetch_byte() as u16;
                let addr_lo = self.read(dp);
                let addr_hi = self.read(dp.wrapping_add(1));
                let base = ((addr_hi as u16) << 8) | (addr_lo as u16);
                let addr = base.wrapping_add(self.y as u16);
                self.a &= self.read(addr);
                self.update_nz(self.a);
                6
            }
            // AND (X), (Y)
            0x39 => {
                let x_val = self.read(self.x as u16);
                let y_val = self.read(self.y as u16);
                let result = x_val & y_val;
                self.write(self.x as u16, result);
                self.update_nz(result);
                5
            }
            // ROL dp+X
            0x3B => {
                let dp = self.fetch_byte();
                let addr = dp.wrapping_add(self.x) as u16;
                let val = self.read(addr);
                let old_carry = if self.get_flag(psw_flags::CARRY) {
                    1
                } else {
                    0
                };
                let new_carry = (val & 0x80) != 0;
                let result = (val << 1) | old_carry;
                self.write(addr, result);
                self.set_carry(new_carry);
                self.update_nz(result);
                5
            }

            // EOR A, (dp+X)
            0x47 => {
                let dp = self.fetch_byte() as u16;
                let addr_lo = self.read(dp.wrapping_add(self.x as u16));
                let addr_hi = self.read(dp.wrapping_add(self.x as u16).wrapping_add(1));
                let addr = ((addr_hi as u16) << 8) | (addr_lo as u16);
                self.a ^= self.read(addr);
                self.update_nz(self.a);
                6
            }
            // EOR dp, dp
            0x49 => {
                let src = self.fetch_byte();
                let dst = self.fetch_byte();
                let src_val = self.read(src as u16);
                let dst_val = self.read(dst as u16);
                let result = dst_val ^ src_val;
                self.write(dst as u16, result);
                self.update_nz(result);
                6
            }
            // AND1 C, /mem.bit - AND carry with inverted bit
            0x4A => {
                let addr_low = self.fetch_byte();
                let addr_high_and_bit = self.fetch_byte();
                let addr = ((addr_high_and_bit as u16 & 0x1F) << 8) | addr_low as u16;
                let bit = (addr_high_and_bit >> 5) & 0x07;
                let val = self.read(addr);
                let bit_val = ((val >> bit) & 1) == 0;
                let carry = self.get_flag(psw_flags::CARRY);
                self.set_carry(carry & bit_val);
                4
            }
            // LSR !abs
            0x4C => {
                let addr = self.fetch_word();
                let val = self.read(addr);
                let carry = (val & 0x01) != 0;
                let result = val >> 1;
                self.write(addr, result);
                self.set_carry(carry);
                self.update_nz(result);
                5
            }
            // TCLR1 !abs - Test and clear bits with A
            0x4E => {
                let addr = self.fetch_word();
                let val = self.read(addr);
                let result = self.a.wrapping_sub(val);
                self.update_nz(result);
                self.write(addr, val & !self.a);
                6
            }
            // PCALL u - Page call (call within page $FF)
            0x4F => {
                let offset = self.fetch_byte();
                let ret = self.pc;
                self.push((ret >> 8) as u8);
                self.push((ret & 0xFF) as u8);
                self.pc = 0xFF00 | (offset as u16);
                6
            }

            // EOR A, (dp)+Y
            0x57 => {
                let dp = self.fetch_byte() as u16;
                let addr_lo = self.read(dp);
                let addr_hi = self.read(dp.wrapping_add(1));
                let base = ((addr_hi as u16) << 8) | (addr_lo as u16);
                let addr = base.wrapping_add(self.y as u16);
                self.a ^= self.read(addr);
                self.update_nz(self.a);
                6
            }
            // EOR dp, #imm
            0x58 => {
                let imm = self.fetch_byte();
                let dp = self.fetch_byte();
                let val = self.read(dp as u16);
                let result = val ^ imm;
                self.write(dp as u16, result);
                self.update_nz(result);
                5
            }
            // EOR (X), (Y)
            0x59 => {
                let x_val = self.read(self.x as u16);
                let y_val = self.read(self.y as u16);
                let result = x_val ^ y_val;
                self.write(self.x as u16, result);
                self.update_nz(result);
                5
            }
            // LSR dp+X
            0x5B => {
                let dp = self.fetch_byte();
                let addr = dp.wrapping_add(self.x) as u16;
                let val = self.read(addr);
                let carry = (val & 0x01) != 0;
                let result = val >> 1;
                self.write(addr, result);
                self.set_carry(carry);
                self.update_nz(result);
                5
            }
            // LSR X
            0x5E => {
                let carry = (self.x & 0x01) != 0;
                self.x >>= 1;
                self.set_carry(carry);
                self.update_nz(self.x);
                2
            }

            // CMP A, (dp)
            0x65 => {
                let dp = self.fetch_byte() as u16;
                let addr_lo = self.read(dp);
                let addr_hi = self.read(dp.wrapping_add(1));
                let addr = ((addr_hi as u16) << 8) | (addr_lo as u16);
                let val = self.read(addr);
                let result = self.a.wrapping_sub(val);
                self.update_nz(result);
                self.set_flag(psw_flags::CARRY, self.a >= val);
                4
            }
            // CMP A, (X)
            0x66 => {
                let val = self.read(self.x as u16);
                let result = self.a.wrapping_sub(val);
                self.update_nz(result);
                self.set_flag(psw_flags::CARRY, self.a >= val);
                3
            }
            // CMP A, (dp+X)
            0x67 => {
                let dp = self.fetch_byte() as u16;
                let addr_lo = self.read(dp.wrapping_add(self.x as u16));
                let addr_hi = self.read(dp.wrapping_add(self.x as u16).wrapping_add(1));
                let addr = ((addr_hi as u16) << 8) | (addr_lo as u16);
                let val = self.read(addr);
                let result = self.a.wrapping_sub(val);
                self.update_nz(result);
                self.set_flag(psw_flags::CARRY, self.a >= val);
                6
            }
            // CMP dp, dp
            0x69 => {
                let src = self.fetch_byte();
                let dst = self.fetch_byte();
                let src_val = self.read(src as u16);
                let dst_val = self.read(dst as u16);
                let result = dst_val.wrapping_sub(src_val);
                self.update_nz(result);
                self.set_flag(psw_flags::CARRY, dst_val >= src_val);
                6
            }
            // AND1 C, /mem.bit (variant)
            0x6A => {
                let addr_low = self.fetch_byte();
                let addr_high_and_bit = self.fetch_byte();
                let addr = ((addr_high_and_bit as u16 & 0x1F) << 8) | addr_low as u16;
                let bit = (addr_high_and_bit >> 5) & 0x07;
                let val = self.read(addr);
                let bit_val = ((val >> bit) & 1) == 0;
                let carry = self.get_flag(psw_flags::CARRY);
                self.set_carry(carry & bit_val);
                4
            }
            // ROR !abs
            0x6C => {
                let addr = self.fetch_word();
                let val = self.read(addr);
                let old_carry = if self.get_flag(psw_flags::CARRY) {
                    0x80
                } else {
                    0
                };
                let new_carry = (val & 0x01) != 0;
                let result = (val >> 1) | old_carry;
                self.write(addr, result);
                self.set_carry(new_carry);
                self.update_nz(result);
                5
            }
            // DBNZ dp, rel - Decrement and branch if not zero
            0x6E => {
                let dp = self.fetch_byte();
                let offset = self.fetch_byte() as i8;
                let addr = self.direct_page() | (dp as u16);
                let val = self.read(addr).wrapping_sub(1);
                self.write(addr, val);
                if val != 0 {
                    self.pc = self.pc.wrapping_add(offset as u16);
                    7
                } else {
                    5
                }
            }

            // MOV (dp), A
            0x74 => {
                let dp = self.fetch_byte() as u16;
                let addr_lo = self.read(dp);
                let addr_hi = self.read(dp.wrapping_add(1));
                let addr = ((addr_hi as u16) << 8) | (addr_lo as u16);
                self.write(addr, self.a);
                5
            }
            // CMP A, (dp)+Y
            0x77 => {
                let dp = self.fetch_byte() as u16;
                let addr_lo = self.read(dp);
                let addr_hi = self.read(dp.wrapping_add(1));
                let base = ((addr_hi as u16) << 8) | (addr_lo as u16);
                let addr = base.wrapping_add(self.y as u16);
                let val = self.read(addr);
                let result = self.a.wrapping_sub(val);
                self.update_nz(result);
                self.set_flag(psw_flags::CARRY, self.a >= val);
                6
            }
            // CMP dp, #imm
            0x79 => {
                let imm = self.fetch_byte();
                let dp = self.fetch_byte();
                let val = self.read(dp as u16);
                let result = val.wrapping_sub(imm);
                self.update_nz(result);
                self.set_flag(psw_flags::CARRY, val >= imm);
                5
            }
            // ROR dp+X
            0x7B => {
                let dp = self.fetch_byte();
                let addr = dp.wrapping_add(self.x) as u16;
                let val = self.read(addr);
                let old_carry = if self.get_flag(psw_flags::CARRY) {
                    0x80
                } else {
                    0
                };
                let new_carry = (val & 0x01) != 0;
                let result = (val >> 1) | old_carry;
                self.write(addr, result);
                self.set_carry(new_carry);
                self.update_nz(result);
                5
            }
            // RETI - Return from interrupt
            0x7F => {
                self.psw = self.pop();
                let lo = self.pop();
                let hi = self.pop();
                self.pc = ((hi as u16) << 8) | (lo as u16);
                6
            }

            // ADC dp, dp
            0x89 => {
                let src = self.fetch_byte();
                let dst = self.fetch_byte();
                let src_val = self.read(src as u16);
                let dst_val = self.read(dst as u16);
                let carry = if self.get_flag(psw_flags::CARRY) {
                    1
                } else {
                    0
                };
                let result = dst_val as u16 + src_val as u16 + carry;
                self.set_flag(psw_flags::CARRY, result > 0xFF);
                self.write(dst as u16, result as u8);
                self.update_nz(result as u8);
                6
            }
            // EOR1 C, mem.bit
            0x8A => {
                let addr_low = self.fetch_byte();
                let addr_high_and_bit = self.fetch_byte();
                let addr = ((addr_high_and_bit as u16 & 0x1F) << 8) | addr_low as u16;
                let bit = (addr_high_and_bit >> 5) & 0x07;
                let val = self.read(addr);
                let bit_val = ((val >> bit) & 1) != 0;
                let carry = self.get_flag(psw_flags::CARRY);
                self.set_carry(carry ^ bit_val);
                5
            }
            // ADC/SBC (X), (Y)
            0x98 => {
                let x_val = self.read(self.x as u16);
                let y_val = self.read(self.y as u16);
                let carry = if self.get_flag(psw_flags::CARRY) {
                    1
                } else {
                    0
                };
                let result = x_val as u16 + y_val as u16 + carry;
                self.set_flag(psw_flags::CARRY, result > 0xFF);
                self.write(self.x as u16, result as u8);
                self.update_nz(result as u8);
                5
            }
            // SBC dp, dp
            0x9B => {
                let src = self.fetch_byte();
                let dst = self.fetch_byte();
                let src_val = self.read(src as u16);
                let dst_val = self.read(dst as u16);
                let carry = if self.get_flag(psw_flags::CARRY) {
                    0
                } else {
                    1
                };
                let result = dst_val as i16 - src_val as i16 - carry;
                self.set_flag(psw_flags::CARRY, result >= 0);
                self.write(dst as u16, result as u8);
                self.update_nz(result as u8);
                6
            }
            // SBC (X), (Y)
            0xB8 => {
                let x_val = self.read(self.x as u16);
                let y_val = self.read(self.y as u16);
                let carry = if self.get_flag(psw_flags::CARRY) {
                    0
                } else {
                    1
                };
                let result = x_val as i16 - y_val as i16 - carry;
                self.set_flag(psw_flags::CARRY, result >= 0);
                self.write(self.x as u16, result as u8);
                self.update_nz(result as u8);
                5
            }

            // SBC A, #imm (Note: A8 is already implemented above)
            // SBC A, #imm (Note: A8 is already implemented above)
            0xA9 => {
                let val = self.fetch_byte();
                let carry = if self.get_flag(psw_flags::CARRY) {
                    0
                } else {
                    1
                };
                let result = self.a as i16 - val as i16 - carry;
                self.set_flag(psw_flags::CARRY, result >= 0);
                self.a = result as u8;
                self.update_nz(self.a);
                2
            }

            // DAS - Decimal adjust for subtraction
            0xBE => {
                // Adjust low nibble if half-carry is clear or low nibble > 9
                if !self.get_flag(psw_flags::HALF_CARRY) {
                    self.a = self.a.wrapping_sub(0x06);
                }
                // Adjust high nibble if carry is clear or A > 0x99
                if !self.get_flag(psw_flags::CARRY) {
                    self.a = self.a.wrapping_sub(0x60);
                    self.set_carry(false);
                }
                self.update_nz(self.a);
                3
            }

            // MOV A, (dp)
            0xE7 => {
                let dp = self.fetch_byte() as u16;
                let addr_lo = self.read(dp);
                let addr_hi = self.read(dp.wrapping_add(1));
                let addr = ((addr_hi as u16) << 8) | (addr_lo as u16);
                self.a = self.read(addr);
                self.update_nz(self.a);
                4
            }

            // EOR dp, dp (not bit operation - simple EOR)
            0xE9 => {
                let src = self.fetch_byte();
                let dst = self.fetch_byte();
                let src_val = self.read(src as u16);
                let dst_val = self.read(dst as u16);
                let result = dst_val ^ src_val;
                self.write(dst as u16, result);
                self.update_nz(result);
                6
            }

            // NOT1 mem.bit
            0xEA => {
                let addr_low = self.fetch_byte();
                let addr_high_and_bit = self.fetch_byte();
                let addr = ((addr_high_and_bit as u16 & 0x1F) << 8) | addr_low as u16;
                let bit = (addr_high_and_bit >> 5) & 0x07;
                let val = self.read(addr);
                let result = val ^ (1 << bit);
                self.write(addr, result);
                5
            }
            // MOV Y, !abs
            0xEC => {
                let addr = self.fetch_word();
                self.y = self.read(addr);
                self.update_nz(self.y);
                4
            }

            // MOV dp+Y, A
            0xD9 => {
                let dp = self.fetch_byte();
                let addr = dp.wrapping_add(self.y) as u16;
                self.write(addr, self.a);
                5
            }
            // MOVW dp, YA (duplicate removed - already implemented)
            0xDB => {
                let dp = self.fetch_byte();
                let addr = self.direct_page() | (dp as u16);
                self.x = self.read(addr);
                self.y = self.read(addr.wrapping_add(1));
                self.update_nz(self.y);
                5
            }
            // MOV Y, dp+X
            0xFB => {
                let dp = self.fetch_byte();
                let val = self.read(dp.wrapping_add(self.x) as u16);
                self.y = val;
                self.update_nz(self.y);
                4
            }
            // CBNE dp+X, rel
            0xDE => {
                let dp = self.fetch_byte();
                let offset = self.fetch_byte() as i8;
                let addr = dp.wrapping_add(self.x) as u16;
                let val = self.read(addr);
                if self.a != val {
                    self.pc = self.pc.wrapping_add(offset as u16);
                    8
                } else {
                    6
                }
            }

            // DAA - Decimal adjust for addition
            0xDF => {
                // Adjust low nibble if half-carry is set or low nibble > 9
                if self.get_flag(psw_flags::HALF_CARRY) || (self.a & 0x0F) > 0x09 {
                    self.a = self.a.wrapping_add(0x06);
                }
                // Adjust high nibble if carry is set or A > 0x99 (before adding 0x60)
                if self.get_flag(psw_flags::CARRY) || self.a > 0x99 {
                    self.a = self.a.wrapping_add(0x60);
                    self.set_carry(true);
                }
                self.update_nz(self.a);
                3
            }

            // DBNZ Y, rel
            0xFE => {
                let offset = self.fetch_byte() as i8;
                self.y = self.y.wrapping_sub(1);
                if self.y != 0 {
                    self.pc = self.pc.wrapping_add(offset as u16);
                    6
                } else {
                    4
                }
            }
            // MOV (dp), Y
            0xD8 => {
                let dp = self.fetch_byte() as u16;
                let addr_lo = self.read(dp);
                let addr_hi = self.read(dp.wrapping_add(1));
                let addr = ((addr_hi as u16) << 8) | (addr_lo as u16);
                self.write(addr, self.y);
                5
            }
            // MOV1 C, mem.bit (duplicate removed - already implemented)
            0xCA => {
                let src = self.fetch_byte();
                let dst = self.fetch_byte();
                let val = self.read(src as u16);
                self.write(dst as u16, val);
                self.update_nz(val);
                5
            }
            // MOV1 mem.bit, C
            0xC7 => {
                let addr_low = self.fetch_byte();
                let addr_high_and_bit = self.fetch_byte();
                let addr = ((addr_high_and_bit as u16 & 0x1F) << 8) | addr_low as u16;
                let bit = (addr_high_and_bit >> 5) & 0x07;
                let val = self.read(addr);
                let result = if self.get_flag(psw_flags::CARRY) {
                    val | (1 << bit)
                } else {
                    val & !(1 << bit)
                };
                self.write(addr, result);
                6
            }
            // CMP X, dp
            0xC9 => {
                let dp = self.fetch_byte();
                let val = self.read(dp as u16);
                let result = self.x.wrapping_sub(val);
                self.update_nz(result);
                self.set_flag(psw_flags::CARRY, self.x >= val);
                3
            }
            // MOV !abs, X
            0xCC => {
                let addr = self.fetch_word();
                self.write(addr, self.x);
                5
            }
            // MOV Y, X
            0xF9 => {
                self.y = self.x;
                self.update_nz(self.y);
                2
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Simple RAM-based memory for testing
    struct TestMemory {
        ram: [u8; 0x10000],
    }

    impl TestMemory {
        fn new() -> Self {
            Self { ram: [0; 0x10000] }
        }
    }

    impl MemorySpc700 for TestMemory {
        fn read(&self, addr: u16) -> u8 {
            self.ram[addr as usize]
        }

        fn write(&mut self, addr: u16, val: u8) {
            self.ram[addr as usize] = val;
        }
    }

    #[test]
    fn test_spc700_creation() {
        let cpu = CpuSpc700::new(TestMemory::new());
        assert_eq!(cpu.pc, 0xFFC0); // Should start at IPL ROM
        assert_eq!(cpu.a, 0);
        assert_eq!(cpu.x, 0);
        assert_eq!(cpu.y, 0);
    }

    #[test]
    fn test_spc700_mov_immediate() {
        let mut mem = TestMemory::new();
        // MOV A, #$42
        mem.ram[0xFFC0] = 0xE8;
        mem.ram[0xFFC1] = 0x42;
        // NOP
        mem.ram[0xFFC2] = 0x00;

        let mut cpu = CpuSpc700::new(mem);
        let cycles = cpu.step();
        assert_eq!(cycles, 2);
        assert_eq!(cpu.a, 0x42);
        assert_eq!(cpu.pc, 0xFFC2);
    }

    #[test]
    fn test_spc700_mov_registers() {
        let mut mem = TestMemory::new();
        // MOV A, #$42
        mem.ram[0xFFC0] = 0xE8;
        mem.ram[0xFFC1] = 0x42;
        // MOV X, A
        mem.ram[0xFFC2] = 0x5D;
        // MOV Y, A
        mem.ram[0xFFC3] = 0xFD;

        let mut cpu = CpuSpc700::new(mem);
        cpu.step(); // MOV A, #$42
        cpu.step(); // MOV X, A
        cpu.step(); // MOV Y, A

        assert_eq!(cpu.a, 0x42);
        assert_eq!(cpu.x, 0x42);
        assert_eq!(cpu.y, 0x42);
    }

    #[test]
    fn test_spc700_mov_absolute() {
        let mut mem = TestMemory::new();
        // Set up test data at $1234
        mem.ram[0x1234] = 0x99;

        // MOV A, !abs ($E5)
        mem.ram[0xFFC0] = 0xE5;
        mem.ram[0xFFC1] = 0x34;
        mem.ram[0xFFC2] = 0x12;

        let mut cpu = CpuSpc700::new(mem);
        cpu.step();

        assert_eq!(cpu.a, 0x99);
        assert_eq!(cpu.pc, 0xFFC3);
    }

    #[test]
    fn test_spc700_mov_absolute_indexed_x() {
        let mut mem = TestMemory::new();
        // Set up test data at $1240
        mem.ram[0x1240] = 0x77;

        // MOV X, #$10
        mem.ram[0xFFC0] = 0xCD;
        mem.ram[0xFFC1] = 0x10;
        // MOV A, !abs+X ($F5) - read from $1230 + $10 = $1240
        mem.ram[0xFFC2] = 0xF5;
        mem.ram[0xFFC3] = 0x30;
        mem.ram[0xFFC4] = 0x12;

        let mut cpu = CpuSpc700::new(mem);
        cpu.step(); // MOV X, #$10
        cpu.step(); // MOV A, !abs+X

        assert_eq!(cpu.x, 0x10);
        assert_eq!(cpu.a, 0x77);
    }

    #[test]
    fn test_spc700_mov_absolute_indexed_y() {
        let mut mem = TestMemory::new();
        // Set up test data at $2050
        mem.ram[0x2050] = 0x88;

        // MOV Y, #$50
        mem.ram[0xFFC0] = 0x8D;
        mem.ram[0xFFC1] = 0x50;
        // MOV A, !abs+Y ($F6) - read from $2000 + $50 = $2050
        mem.ram[0xFFC2] = 0xF6;
        mem.ram[0xFFC3] = 0x00;
        mem.ram[0xFFC4] = 0x20;

        let mut cpu = CpuSpc700::new(mem);
        cpu.step(); // MOV Y, #$50
        cpu.step(); // MOV A, !abs+Y

        assert_eq!(cpu.y, 0x50);
        assert_eq!(cpu.a, 0x88);
    }

    #[test]
    fn test_spc700_mov_write_absolute_indexed() {
        let mut mem = TestMemory::new();

        // MOV A, #$AA
        mem.ram[0xFFC0] = 0xE8;
        mem.ram[0xFFC1] = 0xAA;
        // MOV X, #$05
        mem.ram[0xFFC2] = 0xCD;
        mem.ram[0xFFC3] = 0x05;
        // MOV !abs+X, A ($D5) - write to $1000 + $05 = $1005
        mem.ram[0xFFC4] = 0xD5;
        mem.ram[0xFFC5] = 0x00;
        mem.ram[0xFFC6] = 0x10;

        let mut cpu = CpuSpc700::new(mem);
        cpu.step(); // MOV A, #$AA
        cpu.step(); // MOV X, #$05
        cpu.step(); // MOV !abs+X, A

        assert_eq!(cpu.memory.ram[0x1005], 0xAA);
    }

    #[test]
    fn test_spc700_direct_page_flag() {
        let mut mem = TestMemory::new();

        // CLRP - clear direct page flag
        mem.ram[0xFFC0] = 0x20;
        // SETP - set direct page flag
        mem.ram[0xFFC1] = 0x40;

        let mut cpu = CpuSpc700::new(mem);

        cpu.step(); // CLRP
        assert_eq!(cpu.psw & psw_flags::DIRECT_PAGE, 0);

        cpu.step(); // SETP
        assert_ne!(cpu.psw & psw_flags::DIRECT_PAGE, 0);
    }

    #[test]
    fn test_spc700_nz_flags() {
        let mut mem = TestMemory::new();

        // MOV A, #$00 - should set Z flag
        mem.ram[0xFFC0] = 0xE8;
        mem.ram[0xFFC1] = 0x00;
        // MOV A, #$80 - should set N flag
        mem.ram[0xFFC2] = 0xE8;
        mem.ram[0xFFC3] = 0x80;
        // MOV A, #$01 - should clear both flags
        mem.ram[0xFFC4] = 0xE8;
        mem.ram[0xFFC5] = 0x01;

        let mut cpu = CpuSpc700::new(mem);

        cpu.step(); // MOV A, #$00
        assert_eq!(cpu.a, 0x00);
        assert_ne!(cpu.psw & psw_flags::ZERO, 0, "Zero flag should be set");
        assert_eq!(
            cpu.psw & psw_flags::NEGATIVE,
            0,
            "Negative flag should be clear"
        );

        cpu.step(); // MOV A, #$80
        assert_eq!(cpu.a, 0x80);
        assert_eq!(cpu.psw & psw_flags::ZERO, 0, "Zero flag should be clear");
        assert_ne!(
            cpu.psw & psw_flags::NEGATIVE,
            0,
            "Negative flag should be set"
        );

        cpu.step(); // MOV A, #$01
        assert_eq!(cpu.a, 0x01);
        assert_eq!(cpu.psw & psw_flags::ZERO, 0, "Zero flag should be clear");
        assert_eq!(
            cpu.psw & psw_flags::NEGATIVE,
            0,
            "Negative flag should be clear"
        );
    }

    #[test]
    fn test_spc700_inc_dec() {
        let mut mem = TestMemory::new();

        // MOV A, #$FF
        mem.ram[0xFFC0] = 0xE8;
        mem.ram[0xFFC1] = 0xFF;
        // INC A ($BC)
        mem.ram[0xFFC2] = 0xBC;
        // DEC A ($9C)
        mem.ram[0xFFC3] = 0x9C;

        let mut cpu = CpuSpc700::new(mem);

        cpu.step(); // MOV A, #$FF
        assert_eq!(cpu.a, 0xFF);

        cpu.step(); // INC A
        assert_eq!(cpu.a, 0x00);
        assert_ne!(
            cpu.psw & psw_flags::ZERO,
            0,
            "Zero flag should be set after INC $FF"
        );

        cpu.step(); // DEC A
        assert_eq!(cpu.a, 0xFF);
        assert_ne!(
            cpu.psw & psw_flags::NEGATIVE,
            0,
            "Negative flag should be set"
        );
    }

    #[test]
    fn test_spc700_inc_x_y() {
        let mut mem = TestMemory::new();

        // MOV X, #$FE
        mem.ram[0xFFC0] = 0xCD;
        mem.ram[0xFFC1] = 0xFE;
        // INX ($3D)
        mem.ram[0xFFC2] = 0x3D;
        // MOV Y, #$01
        mem.ram[0xFFC3] = 0x8D;
        mem.ram[0xFFC4] = 0x01;
        // DEY ($DC)
        mem.ram[0xFFC5] = 0xDC;

        let mut cpu = CpuSpc700::new(mem);

        cpu.step(); // MOV X, #$FE
        cpu.step(); // INX
        assert_eq!(cpu.x, 0xFF);

        cpu.step(); // MOV Y, #$01
        cpu.step(); // DEY
        assert_eq!(cpu.y, 0x00);
        assert_ne!(cpu.psw & psw_flags::ZERO, 0);
    }

    #[test]
    fn test_spc700_auto_increment() {
        let mut mem = TestMemory::new();
        mem.ram[0x0005] = 0x42;

        // MOV X, #$05
        mem.ram[0xFFC0] = 0xCD;
        mem.ram[0xFFC1] = 0x05;
        // MOV A, (X)+ ($BF) - read from (X) then increment X
        mem.ram[0xFFC2] = 0xBF;

        let mut cpu = CpuSpc700::new(mem);

        cpu.step(); // MOV X, #$05
        assert_eq!(cpu.x, 0x05);

        cpu.step(); // MOV A, (X)+
        assert_eq!(cpu.a, 0x42);
        assert_eq!(cpu.x, 0x06); // X should be incremented
    }

    #[test]
    fn test_spc700_cycles() {
        let mut mem = TestMemory::new();

        // MOV A, #$42 (2 cycles)
        mem.ram[0xFFC0] = 0xE8;
        mem.ram[0xFFC1] = 0x42;
        // NOP (2 cycles)
        mem.ram[0xFFC2] = 0x00;

        let mut cpu = CpuSpc700::new(mem);
        assert_eq!(cpu.cycles, 0);

        let cycles1 = cpu.step();
        assert_eq!(cycles1, 2);
        assert_eq!(cpu.cycles, 2);

        let cycles2 = cpu.step();
        assert_eq!(cycles2, 2);
        assert_eq!(cpu.cycles, 4);
    }
}
