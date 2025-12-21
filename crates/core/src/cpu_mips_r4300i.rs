//! MIPS R4300i CPU core implementation
//!
//! This module provides a reusable, generic MIPS R4300i CPU implementation for N64 emulation.
//!
//! The R4300i is a 64-bit MIPS III RISC processor with:
//! - 32 general-purpose 64-bit registers
//! - 32 floating-point 64-bit registers
//! - 5-stage pipeline
//! - 32-bit address space (4GB)
//! - Runs at 93.75 MHz on N64

/// Memory interface trait for the MIPS R4300i CPU
///
/// Systems using the R4300i must implement this trait to provide memory access.
pub trait MemoryMips {
    /// Read a byte from memory at the given address
    fn read_byte(&self, addr: u32) -> u8;

    /// Read a halfword (16-bit) from memory at the given address
    fn read_halfword(&self, addr: u32) -> u16;

    /// Read a word (32-bit) from memory at the given address
    fn read_word(&self, addr: u32) -> u32;

    /// Read a doubleword (64-bit) from memory at the given address
    fn read_doubleword(&self, addr: u32) -> u64;

    /// Write a byte to memory at the given address
    fn write_byte(&mut self, addr: u32, val: u8);

    /// Write a halfword (16-bit) to memory at the given address
    fn write_halfword(&mut self, addr: u32, val: u16);

    /// Write a word (32-bit) to memory at the given address
    fn write_word(&mut self, addr: u32, val: u32);

    /// Write a doubleword (64-bit) to memory at the given address
    fn write_doubleword(&mut self, addr: u32, val: u64);
}

/// MIPS R4300i CPU state and execution engine
#[derive(Debug)]
pub struct CpuMips<M: MemoryMips> {
    /// General-purpose registers (R0-R31)
    /// Note: R0 is always zero
    pub gpr: [u64; 32],

    /// Program counter
    pub pc: u64,

    /// HI register (for multiply/divide results)
    pub hi: u64,

    /// LO register (for multiply/divide results)
    pub lo: u64,

    /// Floating-point registers
    pub fpr: [f64; 32],

    /// Floating-point control/status register
    pub fcr31: u32,

    /// CP0 registers (coprocessor 0 - system control)
    pub cp0: [u64; 32],

    /// Total cycles executed
    pub cycles: u64,

    /// Memory interface
    pub memory: M,
}

// CP0 register indices
#[allow(dead_code)]
const CP0_INDEX: usize = 0;
#[allow(dead_code)]
const CP0_RANDOM: usize = 1;
#[allow(dead_code)]
const CP0_ENTRYLO0: usize = 2;
#[allow(dead_code)]
const CP0_ENTRYLO1: usize = 3;
#[allow(dead_code)]
const CP0_CONTEXT: usize = 4;
#[allow(dead_code)]
const CP0_PAGEMASK: usize = 5;
#[allow(dead_code)]
const CP0_WIRED: usize = 6;
#[allow(dead_code)]
const CP0_BADVADDR: usize = 8;
#[allow(dead_code)]
const CP0_COUNT: usize = 9;
#[allow(dead_code)]
const CP0_ENTRYHI: usize = 10;
#[allow(dead_code)]
const CP0_COMPARE: usize = 11;
#[allow(dead_code)]
const CP0_STATUS: usize = 12;
#[allow(dead_code)]
const CP0_CAUSE: usize = 13;
#[allow(dead_code)]
const CP0_EPC: usize = 14;
#[allow(dead_code)]
const CP0_PRID: usize = 15;
#[allow(dead_code)]
const CP0_CONFIG: usize = 16;

impl<M: MemoryMips> CpuMips<M> {
    /// Create a new MIPS R4300i CPU with the given memory interface
    pub fn new(memory: M) -> Self {
        let mut cpu = Self {
            gpr: [0; 32],
            pc: 0xBFC0_0000, // Reset vector in BIOS ROM
            hi: 0,
            lo: 0,
            fpr: [0.0; 32],
            fcr31: 0,
            cp0: [0; 32],
            cycles: 0,
            memory,
        };

        // Initialize CP0 registers
        cpu.cp0[CP0_PRID] = 0x0B00; // Processor ID
        cpu.cp0[CP0_STATUS] = 0x3400_0000; // Status register
        cpu.cp0[CP0_CONFIG] = 0x7006_E463; // Config register

        cpu
    }

    /// Reset the CPU to initial state
    pub fn reset(&mut self) {
        self.gpr = [0; 32];
        self.pc = 0xBFC0_0000;
        self.hi = 0;
        self.lo = 0;
        self.fpr = [0.0; 32];
        self.fcr31 = 0;
        self.cp0 = [0; 32];
        self.cp0[CP0_PRID] = 0x0B00;
        self.cp0[CP0_STATUS] = 0x3400_0000;
        self.cp0[CP0_CONFIG] = 0x7006_E463;
        self.cycles = 0;
    }

    /// Execute a single instruction and return cycles consumed
    pub fn step(&mut self) -> u32 {
        let start_cycles = self.cycles;

        // Fetch instruction
        let instr = self.memory.read_word(self.pc as u32);
        self.pc = self.pc.wrapping_add(4);

        // Decode opcode (bits 26-31)
        let opcode = (instr >> 26) & 0x3F;

        match opcode {
            0x00 => self.execute_special(instr),
            0x02 => self.execute_j(instr),     // J - Jump
            0x05 => self.execute_bne(instr),   // BNE - Branch Not Equal
            0x09 => self.execute_addiu(instr), // ADDIU - Add Immediate Unsigned
            0x0D => self.execute_ori(instr),   // ORI
            0x0F => self.execute_lui(instr),   // LUI
            0x23 => self.execute_lw(instr),    // LW
            0x28 => self.execute_sb(instr),    // SB - Store Byte
            0x2B => self.execute_sw(instr),    // SW
            _ => {
                // Unimplemented instruction
                self.cycles += 1;
            }
        }

        // R0 is always zero
        self.gpr[0] = 0;

        (self.cycles - start_cycles) as u32
    }

    /// Execute SPECIAL opcode instructions (opcode = 0x00)
    fn execute_special(&mut self, instr: u32) {
        let funct = instr & 0x3F;
        let rd = ((instr >> 11) & 0x1F) as usize;
        let rs = ((instr >> 21) & 0x1F) as usize;
        let rt = ((instr >> 16) & 0x1F) as usize;
        let shamt = (instr >> 6) & 0x1F;

        match funct {
            0x00 => {
                // SLL - Shift Left Logical
                self.gpr[rd] = (self.gpr[rt] as u32).wrapping_shl(shamt) as i32 as u64;
                self.cycles += 1;
            }
            0x21 => {
                // ADDU - Add Unsigned
                self.gpr[rd] =
                    (self.gpr[rs] as u32).wrapping_add(self.gpr[rt] as u32) as i32 as u64;
                self.cycles += 1;
            }
            0x25 => {
                // OR
                self.gpr[rd] = self.gpr[rs] | self.gpr[rt];
                self.cycles += 1;
            }
            _ => {
                self.cycles += 1;
            }
        }
    }

    /// Execute ORI - OR Immediate
    fn execute_ori(&mut self, instr: u32) {
        let rt = ((instr >> 16) & 0x1F) as usize;
        let rs = ((instr >> 21) & 0x1F) as usize;
        let imm = (instr & 0xFFFF) as u64;

        self.gpr[rt] = self.gpr[rs] | imm;
        self.cycles += 1;
    }

    /// Execute LUI - Load Upper Immediate
    fn execute_lui(&mut self, instr: u32) {
        let rt = ((instr >> 16) & 0x1F) as usize;
        let imm = instr & 0xFFFF;

        self.gpr[rt] = ((imm << 16) as i32) as u64;
        self.cycles += 1;
    }

    /// Execute LW - Load Word
    fn execute_lw(&mut self, instr: u32) {
        let rt = ((instr >> 16) & 0x1F) as usize;
        let rs = ((instr >> 21) & 0x1F) as usize;
        let offset = (instr & 0xFFFF) as i16 as i32;

        let addr = (self.gpr[rs] as i64).wrapping_add(offset as i64) as u32;
        let val = self.memory.read_word(addr);
        self.gpr[rt] = val as i32 as u64; // Sign-extend to 64-bit
        self.cycles += 1;
    }

    /// Execute SW - Store Word
    fn execute_sw(&mut self, instr: u32) {
        let rt = ((instr >> 16) & 0x1F) as usize;
        let rs = ((instr >> 21) & 0x1F) as usize;
        let offset = (instr & 0xFFFF) as i16 as i32;

        let addr = (self.gpr[rs] as i64).wrapping_add(offset as i64) as u32;
        self.memory.write_word(addr, self.gpr[rt] as u32);
        self.cycles += 1;
    }

    /// Execute SB - Store Byte
    fn execute_sb(&mut self, instr: u32) {
        let rt = ((instr >> 16) & 0x1F) as usize;
        let rs = ((instr >> 21) & 0x1F) as usize;
        let offset = (instr & 0xFFFF) as i16 as i32;

        let addr = (self.gpr[rs] as i64).wrapping_add(offset as i64) as u32;
        self.memory.write_byte(addr, (self.gpr[rt] & 0xFF) as u8);
        self.cycles += 1;
    }

    /// Execute ADDIU - Add Immediate Unsigned
    fn execute_addiu(&mut self, instr: u32) {
        let rt = ((instr >> 16) & 0x1F) as usize;
        let rs = ((instr >> 21) & 0x1F) as usize;
        let imm = (instr & 0xFFFF) as i16 as i32;

        // Sign-extend immediate and add
        self.gpr[rt] = (self.gpr[rs] as i32).wrapping_add(imm) as i64 as u64;
        self.cycles += 1;
    }

    /// Execute BNE - Branch Not Equal
    fn execute_bne(&mut self, instr: u32) {
        let rs = ((instr >> 21) & 0x1F) as usize;
        let rt = ((instr >> 16) & 0x1F) as usize;
        let offset = (instr & 0xFFFF) as i16 as i32;

        if self.gpr[rs] != self.gpr[rt] {
            // Branch: PC + 4 + (offset << 2)
            // Note: PC already incremented by 4 in step()
            self.pc = (self.pc as i64 + (offset as i64 * 4)) as u64;
        }
        self.cycles += 1;
    }

    /// Execute J - Jump
    fn execute_j(&mut self, instr: u32) {
        let target = instr & 0x03FFFFFF;
        
        // Jump: PC = (PC & 0xF0000000) | (target << 2)
        // Note: PC already incremented by 4 in step(), use PC-4 for calculation
        let pc_region = (self.pc - 4) & 0xFFFFFFFF_F0000000;
        self.pc = pc_region | ((target as u64) << 2);
        self.cycles += 1;
    }
}

/// Simple array-backed memory for testing
pub struct ArrayMemory {
    data: Vec<u8>,
}

impl ArrayMemory {
    pub fn new() -> Self {
        Self {
            data: vec![0; 8 * 1024 * 1024], // 8MB
        }
    }
}

impl Default for ArrayMemory {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryMips for ArrayMemory {
    fn read_byte(&self, addr: u32) -> u8 {
        self.data[(addr as usize) & 0x7FFFFF]
    }

    fn read_halfword(&self, addr: u32) -> u16 {
        let addr = addr as usize & 0x7FFFFF;
        u16::from_be_bytes([self.data[addr], self.data[addr + 1]])
    }

    fn read_word(&self, addr: u32) -> u32 {
        let addr = addr as usize & 0x7FFFFF;
        u32::from_be_bytes([
            self.data[addr],
            self.data[addr + 1],
            self.data[addr + 2],
            self.data[addr + 3],
        ])
    }

    fn read_doubleword(&self, addr: u32) -> u64 {
        let addr = addr as usize & 0x7FFFFF;
        u64::from_be_bytes([
            self.data[addr],
            self.data[addr + 1],
            self.data[addr + 2],
            self.data[addr + 3],
            self.data[addr + 4],
            self.data[addr + 5],
            self.data[addr + 6],
            self.data[addr + 7],
        ])
    }

    fn write_byte(&mut self, addr: u32, val: u8) {
        self.data[(addr as usize) & 0x7FFFFF] = val;
    }

    fn write_halfword(&mut self, addr: u32, val: u16) {
        let addr = addr as usize & 0x7FFFFF;
        let bytes = val.to_be_bytes();
        self.data[addr] = bytes[0];
        self.data[addr + 1] = bytes[1];
    }

    fn write_word(&mut self, addr: u32, val: u32) {
        let addr = addr as usize & 0x7FFFFF;
        let bytes = val.to_be_bytes();
        self.data[addr] = bytes[0];
        self.data[addr + 1] = bytes[1];
        self.data[addr + 2] = bytes[2];
        self.data[addr + 3] = bytes[3];
    }

    fn write_doubleword(&mut self, addr: u32, val: u64) {
        let addr = addr as usize & 0x7FFFFF;
        let bytes = val.to_be_bytes();
        self.data[addr..(8 + addr)].copy_from_slice(&bytes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpu_creation() {
        let mem = ArrayMemory::new();
        let cpu = CpuMips::new(mem);
        assert_eq!(cpu.pc, 0xBFC0_0000);
        assert_eq!(cpu.gpr[0], 0);
    }

    #[test]
    fn test_reset() {
        let mem = ArrayMemory::new();
        let mut cpu = CpuMips::new(mem);
        cpu.gpr[1] = 0x1234;
        cpu.reset();

        assert_eq!(cpu.pc, 0xBFC0_0000);
        assert_eq!(cpu.gpr[1], 0);
    }

    #[test]
    fn test_r0_always_zero() {
        let mem = ArrayMemory::new();
        let mut cpu = CpuMips::new(mem);
        cpu.pc = 0;
        cpu.memory.write_word(0, 0x34000000 | 0x1234); // ORI $0, $0, 0x1234
        cpu.step();

        assert_eq!(cpu.gpr[0], 0);
    }

    #[test]
    fn test_ori() {
        let mem = ArrayMemory::new();
        let mut cpu = CpuMips::new(mem);
        cpu.pc = 0;
        cpu.memory.write_word(0, 0x34010000 | 0x1234); // ORI $1, $0, 0x1234
        cpu.step();

        assert_eq!(cpu.gpr[1], 0x1234);
    }

    #[test]
    fn test_lui() {
        let mem = ArrayMemory::new();
        let mut cpu = CpuMips::new(mem);
        cpu.pc = 0;
        cpu.memory.write_word(0, 0x3C010000 | 0x1234); // LUI $1, 0x1234
        cpu.step();

        assert_eq!(cpu.gpr[1] as u32, 0x12340000);
    }

    #[test]
    fn test_addu() {
        let mem = ArrayMemory::new();
        let mut cpu = CpuMips::new(mem);
        cpu.pc = 0;
        cpu.gpr[1] = 10;
        cpu.gpr[2] = 20;
        cpu.memory.write_word(0, 0x00221821); // ADDU $3, $1, $2
        cpu.step();

        assert_eq!(cpu.gpr[3], 30);
    }

    #[test]
    fn test_or() {
        let mem = ArrayMemory::new();
        let mut cpu = CpuMips::new(mem);
        cpu.pc = 0;
        cpu.gpr[1] = 0xF0;
        cpu.gpr[2] = 0x0F;
        cpu.memory.write_word(0, 0x00221825); // OR $3, $1, $2
        cpu.step();

        assert_eq!(cpu.gpr[3], 0xFF);
    }

    #[test]
    fn test_lw_sw() {
        let mem = ArrayMemory::new();
        let mut cpu = CpuMips::new(mem);
        cpu.pc = 0;
        cpu.gpr[1] = 0x1000;
        cpu.gpr[2] = 0xDEADBEEF;

        // SW $2, 0($1) - Store word
        cpu.memory.write_word(0, 0xAC220000);
        cpu.step();

        // LW $3, 0($1) - Load word
        cpu.memory.write_word(4, 0x8C230000);
        cpu.step();

        assert_eq!(cpu.gpr[3] as u32, 0xDEADBEEF);
    }

    #[test]
    fn test_sll() {
        let mem = ArrayMemory::new();
        let mut cpu = CpuMips::new(mem);
        cpu.pc = 0;
        cpu.gpr[2] = 5;
        // SLL $2, $2, 2 (shift left by 2): 0000_00ss_sss0_0000_dddd_daaa_aa00_0000
        // opcode=0, rs=0, rt=2, rd=2, shamt=2, funct=0
        cpu.memory.write_word(0, 0x00021080);
        cpu.step();

        assert_eq!(cpu.gpr[2], 20);
    }
}
