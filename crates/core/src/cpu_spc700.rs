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
        let opcode = self.fetch_byte();
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
    fn push(&mut self, val: u8) {
        let addr = 0x0100 | (self.sp as u16);
        self.write(addr, val);
        self.sp = self.sp.wrapping_sub(1);
    }

    /// Pop a byte from the stack
    #[inline]
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

            // Unknown opcode - log and treat as NOP
            _ => {
                log(LogCategory::Bus, LogLevel::Warn, || {
                    format!(
                        "SPC700: Unknown opcode ${:02X} at PC ${:04X}",
                        opcode,
                        self.pc.wrapping_sub(1)
                    )
                });
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
}
