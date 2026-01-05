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
        
        // Log every 1000th instruction to avoid spam
        if self.cycles % 1000 == 0 {
            log(LogCategory::APU, LogLevel::Debug, || {
                format!("SPC700: PC=${:04X} opcode=${:02X} A=${:02X} X=${:02X} Y=${:02X}", 
                    pc_before, opcode, self.a, self.x, self.y)
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
        assert_eq!(cpu.psw & psw_flags::NEGATIVE, 0, "Negative flag should be clear");

        cpu.step(); // MOV A, #$80
        assert_eq!(cpu.a, 0x80);
        assert_eq!(cpu.psw & psw_flags::ZERO, 0, "Zero flag should be clear");
        assert_ne!(cpu.psw & psw_flags::NEGATIVE, 0, "Negative flag should be set");

        cpu.step(); // MOV A, #$01
        assert_eq!(cpu.a, 0x01);
        assert_eq!(cpu.psw & psw_flags::ZERO, 0, "Zero flag should be clear");
        assert_eq!(cpu.psw & psw_flags::NEGATIVE, 0, "Negative flag should be clear");
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
        assert_ne!(cpu.psw & psw_flags::ZERO, 0, "Zero flag should be set after INC $FF");

        cpu.step(); // DEC A
        assert_eq!(cpu.a, 0xFF);
        assert_ne!(cpu.psw & psw_flags::NEGATIVE, 0, "Negative flag should be set");
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
