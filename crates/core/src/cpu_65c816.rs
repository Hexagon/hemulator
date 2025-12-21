//! WDC 65C816 CPU core implementation
//!
//! This module provides a reusable, generic 65C816 CPU implementation that can be used
//! by any system (SNES, Apple IIGS, etc.) by implementing the `Memory65c816` trait.
//!
//! The 65C816 is a 16-bit extension of the 6502 with:
//! - 16-bit accumulator and index registers (switchable to 8-bit)
//! - 24-bit address space (16MB)
//! - Additional addressing modes
//! - New instructions for 16-bit operations

/// Memory interface trait for the 65C816 CPU
///
/// Systems using the 65C816 must implement this trait to provide memory access.
pub trait Memory65c816 {
    /// Read a byte from memory at the given 24-bit address
    fn read(&self, addr: u32) -> u8;

    /// Write a byte to memory at the given 24-bit address
    fn write(&mut self, addr: u32, val: u8);
}

/// WDC 65C816 CPU state and execution engine
///
/// This is a generic, reusable 65C816 CPU implementation that works with any
/// system through the `Memory65c816` trait.
#[derive(Debug)]
pub struct Cpu65c816<M: Memory65c816> {
    /// Accumulator register (C: 16-bit)
    pub c: u16,
    /// X index register (16-bit)
    pub x: u16,
    /// Y index register (16-bit)
    pub y: u16,
    /// Stack pointer (16-bit)
    pub s: u16,
    /// Direct page register (16-bit)
    pub d: u16,
    /// Data bank register (8-bit)
    pub dbr: u8,
    /// Program bank register (8-bit)
    pub pbr: u8,
    /// Program counter (16-bit, combined with PBR for 24-bit address)
    pub pc: u16,
    /// Status register (NVmxDIZCe - where m=memory/accumulator, x=index, e=emulation)
    pub status: u8,
    /// Emulation mode flag (true = 6502 emulation mode, false = native 16-bit mode)
    pub emulation: bool,
    /// Total cycles executed
    pub cycles: u64,
    /// Memory interface
    pub memory: M,
}

// Status register flags
#[allow(dead_code)]
const FLAG_NEGATIVE: u8 = 0b1000_0000;
#[allow(dead_code)]
const FLAG_OVERFLOW: u8 = 0b0100_0000;
const FLAG_MEMORY: u8 = 0b0010_0000; // m flag: 0=16-bit A, 1=8-bit A
const FLAG_INDEX: u8 = 0b0001_0000; // x flag: 0=16-bit X/Y, 1=8-bit X/Y
#[allow(dead_code)]
const FLAG_DECIMAL: u8 = 0b0000_1000;
#[allow(dead_code)]
const FLAG_IRQ_DISABLE: u8 = 0b0000_0100;
#[allow(dead_code)]
const FLAG_ZERO: u8 = 0b0000_0010;
const FLAG_CARRY: u8 = 0b0000_0001;

impl<M: Memory65c816> Cpu65c816<M> {
    /// Create a new 65C816 CPU with the given memory interface
    pub fn new(memory: M) -> Self {
        Self {
            c: 0,
            x: 0,
            y: 0,
            s: 0x01FF,
            d: 0,
            dbr: 0,
            pbr: 0,
            pc: 0,
            status: 0x34,    // m=1, x=1, I=1 (start in 8-bit mode)
            emulation: true, // Start in emulation mode (6502 compatibility)
            cycles: 0,
            memory,
        }
    }

    /// Reset the CPU to initial state (preserves memory)
    pub fn reset(&mut self) {
        self.c = 0;
        self.x = 0;
        self.y = 0;
        self.s = 0x01FF;
        self.d = 0;
        self.dbr = 0;
        self.pbr = 0;
        self.status = 0x34;
        self.emulation = true;
        self.cycles = 0;

        // Load reset vector from $00FFFC-$00FFFD
        let lo = self.memory.read(0xFFFC) as u16;
        let hi = self.memory.read(0xFFFD) as u16;
        self.pc = (hi << 8) | lo;
    }

    /// Execute a single instruction and return cycles consumed
    pub fn step(&mut self) -> u32 {
        let start_cycles = self.cycles;

        // Fetch opcode
        let opcode = self.fetch_byte();

        // Decode and execute (stub implementation)
        match opcode {
            // NOP
            0xEA => {
                self.cycles += 2;
            }
            // XCE - Exchange Carry and Emulation bits
            0xFB => {
                let old_carry = self.status & FLAG_CARRY;
                if self.emulation {
                    self.status |= FLAG_CARRY;
                } else {
                    self.status &= !FLAG_CARRY;
                }
                self.emulation = old_carry != 0;

                // When switching to emulation mode, force 8-bit mode
                if self.emulation {
                    self.status |= FLAG_MEMORY | FLAG_INDEX;
                    // High bytes of X, Y cleared
                    self.x &= 0xFF;
                    self.y &= 0xFF;
                    // Stack pointer high byte forced to 0x01
                    self.s = 0x0100 | (self.s & 0xFF);
                }
                self.cycles += 2;
            }
            _ => {
                // Unimplemented instruction - just advance
                self.cycles += 2;
            }
        }

        (self.cycles - start_cycles) as u32
    }

    /// Fetch a byte from memory at current PC and advance PC
    fn fetch_byte(&mut self) -> u8 {
        let addr = self.get_pc_address();
        let byte = self.memory.read(addr);
        self.pc = self.pc.wrapping_add(1);
        byte
    }

    /// Get the current 24-bit PC address (PBR:PC)
    fn get_pc_address(&self) -> u32 {
        ((self.pbr as u32) << 16) | (self.pc as u32)
    }

    /// Check if accumulator is in 8-bit mode
    fn is_8bit_a(&self) -> bool {
        self.emulation || (self.status & FLAG_MEMORY) != 0
    }

    /// Check if index registers are in 8-bit mode
    #[allow(dead_code)]
    fn is_8bit_xy(&self) -> bool {
        self.emulation || (self.status & FLAG_INDEX) != 0
    }

    /// Get accumulator value (8 or 16 bit depending on mode)
    pub fn get_a(&self) -> u16 {
        if self.is_8bit_a() {
            self.c & 0xFF
        } else {
            self.c
        }
    }

    /// Set accumulator value (8 or 16 bit depending on mode)
    pub fn set_a(&mut self, val: u16) {
        if self.is_8bit_a() {
            self.c = (self.c & 0xFF00) | (val & 0xFF);
        } else {
            self.c = val;
        }
    }
}

/// Simple array-backed memory for testing
pub struct ArrayMemory {
    data: Vec<u8>,
}

impl ArrayMemory {
    pub fn new() -> Self {
        Self {
            data: vec![0; 16 * 1024 * 1024], // 16MB address space
        }
    }
}

impl Default for ArrayMemory {
    fn default() -> Self {
        Self::new()
    }
}

impl Memory65c816 for ArrayMemory {
    fn read(&self, addr: u32) -> u8 {
        self.data[(addr as usize) & 0xFFFFFF]
    }

    fn write(&mut self, addr: u32, val: u8) {
        self.data[(addr as usize) & 0xFFFFFF] = val;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpu_creation() {
        let mem = ArrayMemory::new();
        let cpu = Cpu65c816::new(mem);
        assert_eq!(cpu.c, 0);
        assert_eq!(cpu.x, 0);
        assert_eq!(cpu.y, 0);
        assert!(cpu.emulation);
    }

    #[test]
    fn test_reset() {
        let mut mem = ArrayMemory::new();
        mem.write(0xFFFC, 0x00);
        mem.write(0xFFFD, 0x80);

        let mut cpu = Cpu65c816::new(mem);
        cpu.reset();

        assert_eq!(cpu.pc, 0x8000);
        assert!(cpu.emulation);
        assert_eq!(cpu.pbr, 0);
    }

    #[test]
    fn test_nop() {
        let mut mem = ArrayMemory::new();
        mem.write(0x8000, 0xEA); // NOP
        mem.write(0xFFFC, 0x00);
        mem.write(0xFFFD, 0x80);

        let mut cpu = Cpu65c816::new(mem);
        cpu.reset();

        let cycles = cpu.step();
        assert_eq!(cycles, 2);
        assert_eq!(cpu.pc, 0x8001);
    }

    #[test]
    fn test_xce_to_native_mode() {
        let mut mem = ArrayMemory::new();
        mem.write(0xFFFC, 0x00);
        mem.write(0xFFFD, 0x80);
        mem.write(0x8000, 0xFB); // XCE

        let mut cpu = Cpu65c816::new(mem);
        cpu.reset();

        assert!(cpu.emulation);

        // Clear carry then XCE to switch to native mode
        cpu.status &= !FLAG_CARRY;
        cpu.pc = 0x8000;

        cpu.step();
        assert!(!cpu.emulation);
    }

    #[test]
    fn test_8bit_16bit_mode_switching() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu65c816::new(mem);

        // Start in emulation mode (8-bit)
        assert!(cpu.is_8bit_a());
        assert!(cpu.is_8bit_xy());

        // Switch to native mode
        cpu.emulation = false;
        cpu.status &= !FLAG_MEMORY;
        cpu.status &= !FLAG_INDEX;

        assert!(!cpu.is_8bit_a());
        assert!(!cpu.is_8bit_xy());
    }

    #[test]
    fn test_get_set_accumulator_8bit() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu65c816::new(mem);

        // In emulation mode (8-bit)
        cpu.set_a(0x1234);
        assert_eq!(cpu.get_a(), 0x34); // Only low byte
    }

    #[test]
    fn test_get_set_accumulator_16bit() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu65c816::new(mem);

        // Switch to native 16-bit mode
        cpu.emulation = false;
        cpu.status &= !FLAG_MEMORY;

        cpu.set_a(0x1234);
        assert_eq!(cpu.get_a(), 0x1234);
    }
}
