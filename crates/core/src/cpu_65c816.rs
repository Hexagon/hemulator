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
        let opcode = self.fetch_byte();

        match opcode {
            // BRK - Force Break
            0x00 => {
                self.push_word(self.pc.wrapping_add(1));
                self.push_byte(self.status);
                if self.emulation {
                    self.pc = self.read_word(0xFFFE);
                    self.status |= FLAG_DECIMAL;
                } else {
                    self.push_byte(self.pbr);
                    self.pc = self.read_word(0xFFE6);
                    self.pbr = 0;
                    self.status &= !FLAG_DECIMAL;
                }
                self.status |= FLAG_IRQ_DISABLE;
                self.cycles += if self.emulation { 7 } else { 8 };
            }

            // ORA - OR with Accumulator
            0x09 => {
                // ORA immediate
                if self.is_8bit_a() {
                    let val = self.fetch_byte();
                    let result = (self.c & 0xFF) as u8 | val;
                    self.c = (self.c & 0xFF00) | result as u16;
                    self.set_zn_8(result);
                    self.cycles += 2;
                } else {
                    let val = self.fetch_word();
                    self.c |= val;
                    self.set_zn_16(self.c);
                    self.cycles += 3;
                }
            }
            0x05 => {
                // ORA direct page
                let addr = self.fetch_byte() as u32 + self.d as u32;
                if self.is_8bit_a() {
                    let val = self.read(addr);
                    let result = (self.c & 0xFF) as u8 | val;
                    self.c = (self.c & 0xFF00) | result as u16;
                    self.set_zn_8(result);
                } else {
                    let val = self.read_word(addr);
                    self.c |= val;
                    self.set_zn_16(self.c);
                }
                self.cycles += if self.is_8bit_a() { 3 } else { 4 };
            }
            0x0D => {
                // ORA absolute
                let addr = ((self.dbr as u32) << 16) + self.fetch_word() as u32;
                if self.is_8bit_a() {
                    let val = self.read(addr);
                    let result = (self.c & 0xFF) as u8 | val;
                    self.c = (self.c & 0xFF00) | result as u16;
                    self.set_zn_8(result);
                } else {
                    let val = self.read_word(addr);
                    self.c |= val;
                    self.set_zn_16(self.c);
                }
                self.cycles += if self.is_8bit_a() { 4 } else { 5 };
            }

            // ASL - Arithmetic Shift Left
            0x0A => {
                // ASL accumulator
                if self.is_8bit_a() {
                    let val = (self.c & 0xFF) as u8;
                    if val & 0x80 != 0 {
                        self.status |= FLAG_CARRY;
                    } else {
                        self.status &= !FLAG_CARRY;
                    }
                    let result = val << 1;
                    self.c = (self.c & 0xFF00) | result as u16;
                    self.set_zn_8(result);
                } else {
                    if self.c & 0x8000 != 0 {
                        self.status |= FLAG_CARRY;
                    } else {
                        self.status &= !FLAG_CARRY;
                    }
                    self.c <<= 1;
                    self.set_zn_16(self.c);
                }
                self.cycles += 2;
            }

            // CLC - Clear Carry
            0x18 => {
                self.status &= !FLAG_CARRY;
                self.cycles += 2;
            }

            // AND - AND with Accumulator
            0x29 => {
                // AND immediate
                if self.is_8bit_a() {
                    let val = self.fetch_byte();
                    let result = (self.c & 0xFF) as u8 & val;
                    self.c = (self.c & 0xFF00) | result as u16;
                    self.set_zn_8(result);
                    self.cycles += 2;
                } else {
                    let val = self.fetch_word();
                    self.c &= val;
                    self.set_zn_16(self.c);
                    self.cycles += 3;
                }
            }

            // ROL - Rotate Left
            0x2A => {
                // ROL accumulator
                if self.is_8bit_a() {
                    let val = (self.c & 0xFF) as u8;
                    let carry_in = if self.status & FLAG_CARRY != 0 { 1 } else { 0 };
                    let result = (val << 1) | carry_in;
                    if val & 0x80 != 0 {
                        self.status |= FLAG_CARRY;
                    } else {
                        self.status &= !FLAG_CARRY;
                    }
                    self.c = (self.c & 0xFF00) | result as u16;
                    self.set_zn_8(result);
                } else {
                    let carry_in = if self.status & FLAG_CARRY != 0 { 1 } else { 0 };
                    let result = (self.c << 1) | carry_in;
                    if self.c & 0x8000 != 0 {
                        self.status |= FLAG_CARRY;
                    } else {
                        self.status &= !FLAG_CARRY;
                    }
                    self.c = result;
                    self.set_zn_16(self.c);
                }
                self.cycles += 2;
            }

            // SEC - Set Carry
            0x38 => {
                self.status |= FLAG_CARRY;
                self.cycles += 2;
            }

            // RTI - Return from Interrupt
            0x40 => {
                if self.emulation {
                    self.status = self.pop_byte();
                    self.pc = self.pop_word();
                    self.cycles += 6;
                } else {
                    self.status = self.pop_byte();
                    self.pc = self.pop_word();
                    self.pbr = self.pop_byte();
                    self.cycles += 7;
                }
            }

            // EOR - Exclusive OR with Accumulator
            0x49 => {
                // EOR immediate
                if self.is_8bit_a() {
                    let val = self.fetch_byte();
                    let result = (self.c & 0xFF) as u8 ^ val;
                    self.c = (self.c & 0xFF00) | result as u16;
                    self.set_zn_8(result);
                    self.cycles += 2;
                } else {
                    let val = self.fetch_word();
                    self.c ^= val;
                    self.set_zn_16(self.c);
                    self.cycles += 3;
                }
            }

            // LSR - Logical Shift Right
            0x4A => {
                // LSR accumulator
                if self.is_8bit_a() {
                    let val = (self.c & 0xFF) as u8;
                    if val & 1 != 0 {
                        self.status |= FLAG_CARRY;
                    } else {
                        self.status &= !FLAG_CARRY;
                    }
                    let result = val >> 1;
                    self.c = (self.c & 0xFF00) | result as u16;
                    self.set_zn_8(result);
                } else {
                    if self.c & 1 != 0 {
                        self.status |= FLAG_CARRY;
                    } else {
                        self.status &= !FLAG_CARRY;
                    }
                    self.c >>= 1;
                    self.set_zn_16(self.c);
                }
                self.cycles += 2;
            }

            // CLI - Clear Interrupt Disable
            0x58 => {
                self.status &= !FLAG_IRQ_DISABLE;
                self.cycles += 2;
            }

            // RTS - Return from Subroutine
            0x60 => {
                self.pc = self.pop_word().wrapping_add(1);
                self.cycles += 6;
            }

            // ADC - Add with Carry
            0x69 => {
                // ADC immediate
                if self.is_8bit_a() {
                    let val = self.fetch_byte();
                    self.adc_8(val);
                    self.cycles += 2;
                } else {
                    let val = self.fetch_word();
                    self.adc_16(val);
                    self.cycles += 3;
                }
            }

            // ROR - Rotate Right
            0x6A => {
                // ROR accumulator
                if self.is_8bit_a() {
                    let val = (self.c & 0xFF) as u8;
                    let carry_in = if self.status & FLAG_CARRY != 0 {
                        0x80
                    } else {
                        0
                    };
                    let result = (val >> 1) | carry_in;
                    if val & 1 != 0 {
                        self.status |= FLAG_CARRY;
                    } else {
                        self.status &= !FLAG_CARRY;
                    }
                    self.c = (self.c & 0xFF00) | result as u16;
                    self.set_zn_8(result);
                } else {
                    let carry_in = if self.status & FLAG_CARRY != 0 {
                        0x8000
                    } else {
                        0
                    };
                    let result = (self.c >> 1) | carry_in;
                    if self.c & 1 != 0 {
                        self.status |= FLAG_CARRY;
                    } else {
                        self.status &= !FLAG_CARRY;
                    }
                    self.c = result;
                    self.set_zn_16(self.c);
                }
                self.cycles += 2;
            }

            // SEI - Set Interrupt Disable
            0x78 => {
                self.status |= FLAG_IRQ_DISABLE;
                self.cycles += 2;
            }

            // STY - Store Y Register
            0x84 => {
                // STY direct page
                let addr = self.fetch_byte() as u32 + self.d as u32;
                if self.is_8bit_xy() {
                    self.write(addr, (self.y & 0xFF) as u8);
                } else {
                    self.write_word(addr, self.y);
                }
                self.cycles += if self.is_8bit_xy() { 3 } else { 4 };
            }
            0x8C => {
                // STY absolute
                let addr = ((self.dbr as u32) << 16) + self.fetch_word() as u32;
                if self.is_8bit_xy() {
                    self.write(addr, (self.y & 0xFF) as u8);
                } else {
                    self.write_word(addr, self.y);
                }
                self.cycles += if self.is_8bit_xy() { 4 } else { 5 };
            }

            // STA - Store Accumulator
            0x85 => {
                // STA direct page
                let addr = self.fetch_byte() as u32 + self.d as u32;
                if self.is_8bit_a() {
                    self.write(addr, (self.c & 0xFF) as u8);
                } else {
                    self.write_word(addr, self.c);
                }
                self.cycles += if self.is_8bit_a() { 3 } else { 4 };
            }
            0x8D => {
                // STA absolute
                let addr = ((self.dbr as u32) << 16) + self.fetch_word() as u32;
                if self.is_8bit_a() {
                    self.write(addr, (self.c & 0xFF) as u8);
                } else {
                    self.write_word(addr, self.c);
                }
                self.cycles += if self.is_8bit_a() { 4 } else { 5 };
            }
            0x9D => {
                // STA absolute,X
                let base = self.fetch_word() as u32;
                let addr = ((self.dbr as u32) << 16) + base + self.x as u32;
                if self.is_8bit_a() {
                    self.write(addr, (self.c & 0xFF) as u8);
                } else {
                    self.write_word(addr, self.c);
                }
                self.cycles += if self.is_8bit_a() { 5 } else { 6 };
            }

            // STX - Store X Register
            0x86 => {
                // STX direct page
                let addr = self.fetch_byte() as u32 + self.d as u32;
                if self.is_8bit_xy() {
                    self.write(addr, (self.x & 0xFF) as u8);
                } else {
                    self.write_word(addr, self.x);
                }
                self.cycles += if self.is_8bit_xy() { 3 } else { 4 };
            }
            0x8E => {
                // STX absolute
                let addr = ((self.dbr as u32) << 16) + self.fetch_word() as u32;
                if self.is_8bit_xy() {
                    self.write(addr, (self.x & 0xFF) as u8);
                } else {
                    self.write_word(addr, self.x);
                }
                self.cycles += if self.is_8bit_xy() { 4 } else { 5 };
            }

            // DEY - Decrement Y
            0x88 => {
                if self.is_8bit_xy() {
                    self.y = (self.y & 0xFF00) | ((self.y.wrapping_sub(1)) & 0xFF);
                    self.set_zn_8((self.y & 0xFF) as u8);
                } else {
                    self.y = self.y.wrapping_sub(1);
                    self.set_zn_16(self.y);
                }
                self.cycles += 2;
            }

            // TXA - Transfer X to A
            0x8A => {
                if self.is_8bit_a() {
                    self.c = (self.c & 0xFF00) | (self.x & 0xFF);
                    self.set_zn_8((self.c & 0xFF) as u8);
                } else {
                    self.c = self.x;
                    self.set_zn_16(self.c);
                }
                self.cycles += 2;
            }

            // TYA - Transfer Y to A
            0x98 => {
                if self.is_8bit_a() {
                    self.c = (self.c & 0xFF00) | (self.y & 0xFF);
                    self.set_zn_8((self.c & 0xFF) as u8);
                } else {
                    self.c = self.y;
                    self.set_zn_16(self.c);
                }
                self.cycles += 2;
            }

            // TXS - Transfer X to Stack Pointer
            0x9A => {
                if self.emulation {
                    self.s = 0x0100 | (self.x & 0xFF);
                } else {
                    self.s = self.x;
                }
                self.cycles += 2;
            }

            // TAY - Transfer A to Y
            0xA8 => {
                if self.is_8bit_xy() {
                    self.y = (self.y & 0xFF00) | (self.c & 0xFF);
                    self.set_zn_8((self.y & 0xFF) as u8);
                } else {
                    self.y = self.c;
                    self.set_zn_16(self.y);
                }
                self.cycles += 2;
            }

            // TAX - Transfer A to X
            0xAA => {
                if self.is_8bit_xy() {
                    self.x = (self.x & 0xFF00) | (self.c & 0xFF);
                    self.set_zn_8((self.x & 0xFF) as u8);
                } else {
                    self.x = self.c;
                    self.set_zn_16(self.x);
                }
                self.cycles += 2;
            }

            // LDY - Load Y Register
            0xA0 => {
                // LDY immediate
                if self.is_8bit_xy() {
                    let val = self.fetch_byte();
                    self.y = (self.y & 0xFF00) | val as u16;
                    self.set_zn_8(val);
                    self.cycles += 2;
                } else {
                    let val = self.fetch_word();
                    self.y = val;
                    self.set_zn_16(val);
                    self.cycles += 3;
                }
            }
            0xA4 => {
                // LDY direct page
                let addr = self.fetch_byte() as u32 + self.d as u32;
                if self.is_8bit_xy() {
                    let val = self.read(addr);
                    self.y = (self.y & 0xFF00) | val as u16;
                    self.set_zn_8(val);
                } else {
                    let val = self.read_word(addr);
                    self.y = val;
                    self.set_zn_16(val);
                }
                self.cycles += if self.is_8bit_xy() { 3 } else { 4 };
            }
            0xAC => {
                // LDY absolute
                let addr = ((self.dbr as u32) << 16) + self.fetch_word() as u32;
                if self.is_8bit_xy() {
                    let val = self.read(addr);
                    self.y = (self.y & 0xFF00) | val as u16;
                    self.set_zn_8(val);
                } else {
                    let val = self.read_word(addr);
                    self.y = val;
                    self.set_zn_16(val);
                }
                self.cycles += if self.is_8bit_xy() { 4 } else { 5 };
            }

            // LDA - Load Accumulator
            0xA9 => {
                // LDA immediate
                if self.is_8bit_a() {
                    let val = self.fetch_byte();
                    self.c = (self.c & 0xFF00) | val as u16;
                    self.set_zn_8(val);
                    self.cycles += 2;
                } else {
                    let val = self.fetch_word();
                    self.c = val;
                    self.set_zn_16(val);
                    self.cycles += 3;
                }
            }
            0xA5 => {
                // LDA direct page
                let addr = self.fetch_byte() as u32 + self.d as u32;
                if self.is_8bit_a() {
                    let val = self.read(addr);
                    self.c = (self.c & 0xFF00) | val as u16;
                    self.set_zn_8(val);
                } else {
                    let val = self.read_word(addr);
                    self.c = val;
                    self.set_zn_16(val);
                }
                self.cycles += if self.is_8bit_a() { 3 } else { 4 };
            }
            0xAD => {
                // LDA absolute
                let addr = ((self.dbr as u32) << 16) + self.fetch_word() as u32;
                if self.is_8bit_a() {
                    let val = self.read(addr);
                    self.c = (self.c & 0xFF00) | val as u16;
                    self.set_zn_8(val);
                } else {
                    let val = self.read_word(addr);
                    self.c = val;
                    self.set_zn_16(val);
                }
                self.cycles += if self.is_8bit_a() { 4 } else { 5 };
            }
            0xBD => {
                // LDA absolute,X
                let base = self.fetch_word() as u32;
                let addr = ((self.dbr as u32) << 16) + base + self.x as u32;
                if self.is_8bit_a() {
                    let val = self.read(addr);
                    self.c = (self.c & 0xFF00) | val as u16;
                    self.set_zn_8(val);
                } else {
                    let val = self.read_word(addr);
                    self.c = val;
                    self.set_zn_16(val);
                }
                self.cycles += if self.is_8bit_a() { 4 } else { 5 };
            }

            // LDX - Load X Register
            0xA2 => {
                // LDX immediate
                if self.is_8bit_xy() {
                    let val = self.fetch_byte();
                    self.x = (self.x & 0xFF00) | val as u16;
                    self.set_zn_8(val);
                    self.cycles += 2;
                } else {
                    let val = self.fetch_word();
                    self.x = val;
                    self.set_zn_16(val);
                    self.cycles += 3;
                }
            }
            0xA6 => {
                // LDX direct page
                let addr = self.fetch_byte() as u32 + self.d as u32;
                if self.is_8bit_xy() {
                    let val = self.read(addr);
                    self.x = (self.x & 0xFF00) | val as u16;
                    self.set_zn_8(val);
                } else {
                    let val = self.read_word(addr);
                    self.x = val;
                    self.set_zn_16(val);
                }
                self.cycles += if self.is_8bit_xy() { 3 } else { 4 };
            }
            0xAE => {
                // LDX absolute
                let addr = ((self.dbr as u32) << 16) + self.fetch_word() as u32;
                if self.is_8bit_xy() {
                    let val = self.read(addr);
                    self.x = (self.x & 0xFF00) | val as u16;
                    self.set_zn_8(val);
                } else {
                    let val = self.read_word(addr);
                    self.x = val;
                    self.set_zn_16(val);
                }
                self.cycles += if self.is_8bit_xy() { 4 } else { 5 };
            }

            // TSX - Transfer Stack Pointer to X
            0xBA => {
                if self.is_8bit_xy() {
                    self.x = (self.x & 0xFF00) | (self.s & 0xFF);
                    self.set_zn_8((self.s & 0xFF) as u8);
                } else {
                    self.x = self.s;
                    self.set_zn_16(self.s);
                }
                self.cycles += 2;
            }

            // CLV - Clear Overflow
            0xB8 => {
                self.status &= !FLAG_OVERFLOW;
                self.cycles += 2;
            }

            // CPY - Compare Y Register
            0xC0 => {
                // CPY immediate
                if self.is_8bit_xy() {
                    let val = self.fetch_byte();
                    let result = (self.y & 0xFF) as u8;
                    self.compare_8(result, val);
                    self.cycles += 2;
                } else {
                    let val = self.fetch_word();
                    self.compare_16(self.y, val);
                    self.cycles += 3;
                }
            }

            // CMP - Compare Accumulator
            0xC9 => {
                // CMP immediate
                if self.is_8bit_a() {
                    let val = self.fetch_byte();
                    self.compare_8((self.c & 0xFF) as u8, val);
                    self.cycles += 2;
                } else {
                    let val = self.fetch_word();
                    self.compare_16(self.c, val);
                    self.cycles += 3;
                }
            }

            // DEC - Decrement Memory
            0xC6 => {
                // DEC direct page
                let addr = self.fetch_byte() as u32 + self.d as u32;
                if self.is_8bit_a() {
                    let val = self.read(addr);
                    let result = val.wrapping_sub(1);
                    self.write(addr, result);
                    self.set_zn_8(result);
                } else {
                    let val = self.read_word(addr);
                    let result = val.wrapping_sub(1);
                    self.write_word(addr, result);
                    self.set_zn_16(result);
                }
                self.cycles += if self.is_8bit_a() { 5 } else { 6 };
            }

            // CLD - Clear Decimal Mode
            0xD8 => {
                self.status &= !FLAG_DECIMAL;
                self.cycles += 2;
            }

            // CPX - Compare X Register
            0xE0 => {
                // CPX immediate
                if self.is_8bit_xy() {
                    let val = self.fetch_byte();
                    self.compare_8((self.x & 0xFF) as u8, val);
                    self.cycles += 2;
                } else {
                    let val = self.fetch_word();
                    self.compare_16(self.x, val);
                    self.cycles += 3;
                }
            }

            // SBC - Subtract with Carry
            0xE9 => {
                // SBC immediate
                if self.is_8bit_a() {
                    let val = self.fetch_byte();
                    self.sbc_8(val);
                    self.cycles += 2;
                } else {
                    let val = self.fetch_word();
                    self.sbc_16(val);
                    self.cycles += 3;
                }
            }

            // INC - Increment Memory
            0xE6 => {
                // INC direct page
                let addr = self.fetch_byte() as u32 + self.d as u32;
                if self.is_8bit_a() {
                    let val = self.read(addr);
                    let result = val.wrapping_add(1);
                    self.write(addr, result);
                    self.set_zn_8(result);
                } else {
                    let val = self.read_word(addr);
                    let result = val.wrapping_add(1);
                    self.write_word(addr, result);
                    self.set_zn_16(result);
                }
                self.cycles += if self.is_8bit_a() { 5 } else { 6 };
            }

            // INX - Increment X
            0xE8 => {
                if self.is_8bit_xy() {
                    self.x = (self.x & 0xFF00) | ((self.x.wrapping_add(1)) & 0xFF);
                    self.set_zn_8((self.x & 0xFF) as u8);
                } else {
                    self.x = self.x.wrapping_add(1);
                    self.set_zn_16(self.x);
                }
                self.cycles += 2;
            }

            // NOP
            0xEA => {
                self.cycles += 2;
            }

            // INY - Increment Y
            0xC8 => {
                if self.is_8bit_xy() {
                    self.y = (self.y & 0xFF00) | ((self.y.wrapping_add(1)) & 0xFF);
                    self.set_zn_8((self.y & 0xFF) as u8);
                } else {
                    self.y = self.y.wrapping_add(1);
                    self.set_zn_16(self.y);
                }
                self.cycles += 2;
            }

            // DEX - Decrement X
            0xCA => {
                if self.is_8bit_xy() {
                    self.x = (self.x & 0xFF00) | ((self.x.wrapping_sub(1)) & 0xFF);
                    self.set_zn_8((self.x & 0xFF) as u8);
                } else {
                    self.x = self.x.wrapping_sub(1);
                    self.set_zn_16(self.x);
                }
                self.cycles += 2;
            }

            // SED - Set Decimal Mode
            0xF8 => {
                self.status |= FLAG_DECIMAL;
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
                    self.x &= 0xFF;
                    self.y &= 0xFF;
                    self.s = 0x0100 | (self.s & 0xFF);
                }
                self.cycles += 2;
            }

            // REP - Reset Status Bits
            0xC2 => {
                let mask = self.fetch_byte();
                self.status &= !mask;
                // In emulation mode, m and x flags cannot be cleared
                if self.emulation {
                    self.status |= FLAG_MEMORY | FLAG_INDEX;
                }
                self.cycles += 3;
            }

            // SEP - Set Status Bits
            0xE2 => {
                let mask = self.fetch_byte();
                self.status |= mask;
                self.cycles += 3;
            }

            // Branch instructions
            0x10 => {
                // BPL - Branch if Plus
                self.branch(self.status & FLAG_NEGATIVE == 0);
            }
            0x30 => {
                // BMI - Branch if Minus
                self.branch(self.status & FLAG_NEGATIVE != 0);
            }
            0x50 => {
                // BVC - Branch if Overflow Clear
                self.branch(self.status & FLAG_OVERFLOW == 0);
            }
            0x70 => {
                // BVS - Branch if Overflow Set
                self.branch(self.status & FLAG_OVERFLOW != 0);
            }
            0x90 => {
                // BCC - Branch if Carry Clear
                self.branch(self.status & FLAG_CARRY == 0);
            }
            0xB0 => {
                // BCS - Branch if Carry Set
                self.branch(self.status & FLAG_CARRY != 0);
            }
            0xD0 => {
                // BNE - Branch if Not Equal
                self.branch(self.status & FLAG_ZERO == 0);
            }
            0xF0 => {
                // BEQ - Branch if Equal
                self.branch(self.status & FLAG_ZERO != 0);
            }

            // JMP - Jump
            0x4C => {
                // JMP absolute
                self.pc = self.fetch_word();
                self.cycles += 3;
            }

            // JSR - Jump to Subroutine
            0x20 => {
                let target = self.fetch_word();
                let ret_addr = self.pc.wrapping_sub(1);
                self.push_word(ret_addr);
                self.pc = target;
                self.cycles += 6;
            }

            // Push/Pull instructions
            0x48 => {
                // PHA - Push Accumulator
                if self.is_8bit_a() {
                    self.push_byte((self.c & 0xFF) as u8);
                    self.cycles += 3;
                } else {
                    self.push_word(self.c);
                    self.cycles += 4;
                }
            }
            0x68 => {
                // PLA - Pull Accumulator
                if self.is_8bit_a() {
                    let val = self.pop_byte();
                    self.c = (self.c & 0xFF00) | val as u16;
                    self.set_zn_8(val);
                    self.cycles += 4;
                } else {
                    let val = self.pop_word();
                    self.c = val;
                    self.set_zn_16(val);
                    self.cycles += 5;
                }
            }
            0x08 => {
                // PHP - Push Processor Status
                self.push_byte(self.status);
                self.cycles += 3;
            }
            0x28 => {
                // PLP - Pull Processor Status
                self.status = self.pop_byte();
                if self.emulation {
                    self.status |= FLAG_MEMORY | FLAG_INDEX;
                }
                self.cycles += 4;
            }
            0xDA => {
                // PHX - Push X
                if self.is_8bit_xy() {
                    self.push_byte((self.x & 0xFF) as u8);
                    self.cycles += 3;
                } else {
                    self.push_word(self.x);
                    self.cycles += 4;
                }
            }
            0xFA => {
                // PLX - Pull X
                if self.is_8bit_xy() {
                    let val = self.pop_byte();
                    self.x = (self.x & 0xFF00) | val as u16;
                    self.set_zn_8(val);
                    self.cycles += 4;
                } else {
                    let val = self.pop_word();
                    self.x = val;
                    self.set_zn_16(val);
                    self.cycles += 5;
                }
            }
            0x5A => {
                // PHY - Push Y
                if self.is_8bit_xy() {
                    self.push_byte((self.y & 0xFF) as u8);
                    self.cycles += 3;
                } else {
                    self.push_word(self.y);
                    self.cycles += 4;
                }
            }
            0x7A => {
                // PLY - Pull Y
                if self.is_8bit_xy() {
                    let val = self.pop_byte();
                    self.y = (self.y & 0xFF00) | val as u16;
                    self.set_zn_8(val);
                    self.cycles += 4;
                } else {
                    let val = self.pop_word();
                    self.y = val;
                    self.set_zn_16(val);
                    self.cycles += 5;
                }
            }

            _ => {
                // Unimplemented instruction - log and skip
                if std::env::var("EMU_LOG_UNKNOWN_OPS").unwrap_or_default() == "1" {
                    eprintln!(
                        "Unknown 65C816 opcode: 0x{:02X} at PC=0x{:02X}:{:04X}",
                        opcode,
                        self.pbr,
                        self.pc.wrapping_sub(1)
                    );
                }
                // Skip this instruction to avoid infinite loop
                // This is a placeholder - real implementation would need proper opcode decoding
                self.cycles += 2;
            }
        }

        (self.cycles - start_cycles) as u32
    }

    /// ADC operation for 8-bit mode
    fn adc_8(&mut self, val: u8) {
        let a = (self.c & 0xFF) as u8;
        let carry = if self.status & FLAG_CARRY != 0 { 1 } else { 0 };
        let sum = a as u16 + val as u16 + carry;
        let result = sum as u8;

        // Set carry
        if sum > 0xFF {
            self.status |= FLAG_CARRY;
        } else {
            self.status &= !FLAG_CARRY;
        }

        // Set overflow: (~(A ^ M) & (A ^ R)) & 0x80
        if (!(a ^ val) & (a ^ result)) & 0x80 != 0 {
            self.status |= FLAG_OVERFLOW;
        } else {
            self.status &= !FLAG_OVERFLOW;
        }

        self.c = (self.c & 0xFF00) | result as u16;
        self.set_zn_8(result);
    }

    /// ADC operation for 16-bit mode
    fn adc_16(&mut self, val: u16) {
        let carry = if self.status & FLAG_CARRY != 0 { 1 } else { 0 };
        let sum = self.c as u32 + val as u32 + carry;
        let result = sum as u16;

        // Set carry
        if sum > 0xFFFF {
            self.status |= FLAG_CARRY;
        } else {
            self.status &= !FLAG_CARRY;
        }

        // Set overflow
        if (!(self.c ^ val) & (self.c ^ result)) & 0x8000 != 0 {
            self.status |= FLAG_OVERFLOW;
        } else {
            self.status &= !FLAG_OVERFLOW;
        }

        self.c = result;
        self.set_zn_16(result);
    }

    /// SBC operation for 8-bit mode
    fn sbc_8(&mut self, val: u8) {
        let a = (self.c & 0xFF) as u8;
        let carry = if self.status & FLAG_CARRY != 0 { 0 } else { 1 };
        let diff = a as i16 - val as i16 - carry as i16;
        let result = diff as u8;

        // Set carry (inverted borrow)
        if diff >= 0 {
            self.status |= FLAG_CARRY;
        } else {
            self.status &= !FLAG_CARRY;
        }

        // Set overflow
        if ((a ^ val) & (a ^ result)) & 0x80 != 0 {
            self.status |= FLAG_OVERFLOW;
        } else {
            self.status &= !FLAG_OVERFLOW;
        }

        self.c = (self.c & 0xFF00) | result as u16;
        self.set_zn_8(result);
    }

    /// SBC operation for 16-bit mode
    fn sbc_16(&mut self, val: u16) {
        let carry = if self.status & FLAG_CARRY != 0 { 0 } else { 1 };
        let diff = self.c as i32 - val as i32 - carry;
        let result = diff as u16;

        // Set carry (inverted borrow)
        if diff >= 0 {
            self.status |= FLAG_CARRY;
        } else {
            self.status &= !FLAG_CARRY;
        }

        // Set overflow
        if ((self.c ^ val) & (self.c ^ result)) & 0x8000 != 0 {
            self.status |= FLAG_OVERFLOW;
        } else {
            self.status &= !FLAG_OVERFLOW;
        }

        self.c = result;
        self.set_zn_16(result);
    }

    /// Compare operation for 8-bit values
    fn compare_8(&mut self, reg: u8, val: u8) {
        let result = reg.wrapping_sub(val);

        // Set carry if reg >= val
        if reg >= val {
            self.status |= FLAG_CARRY;
        } else {
            self.status &= !FLAG_CARRY;
        }

        self.set_zn_8(result);
    }

    /// Compare operation for 16-bit values
    fn compare_16(&mut self, reg: u16, val: u16) {
        let result = reg.wrapping_sub(val);

        // Set carry if reg >= val
        if reg >= val {
            self.status |= FLAG_CARRY;
        } else {
            self.status &= !FLAG_CARRY;
        }

        self.set_zn_16(result);
    }

    /// Execute a branch instruction
    fn branch(&mut self, condition: bool) {
        let offset = self.fetch_byte() as i8;
        if condition {
            self.pc = self.pc.wrapping_add(offset as u16);
            self.cycles += 3;
        } else {
            self.cycles += 2;
        }
    }

    /// Fetch a byte from memory at current PC and advance PC
    fn fetch_byte(&mut self) -> u8 {
        let addr = self.get_pc_address();
        let byte = self.memory.read(addr);
        self.pc = self.pc.wrapping_add(1);
        byte
    }

    /// Fetch a 16-bit word from memory at current PC and advance PC
    fn fetch_word(&mut self) -> u16 {
        let lo = self.fetch_byte() as u16;
        let hi = self.fetch_byte() as u16;
        (hi << 8) | lo
    }

    /// Get the current 24-bit PC address (PBR:PC)
    fn get_pc_address(&self) -> u32 {
        ((self.pbr as u32) << 16) | (self.pc as u32)
    }

    /// Read a byte from memory
    #[inline]
    fn read(&self, addr: u32) -> u8 {
        self.memory.read(addr)
    }

    /// Write a byte to memory
    #[inline]
    fn write(&mut self, addr: u32, val: u8) {
        self.memory.write(addr, val);
    }

    /// Read a 16-bit word from memory
    fn read_word(&self, addr: u32) -> u16 {
        let lo = self.read(addr) as u16;
        let hi = self.read(addr + 1) as u16;
        (hi << 8) | lo
    }

    /// Write a 16-bit word to memory
    fn write_word(&mut self, addr: u32, val: u16) {
        self.write(addr, (val & 0xFF) as u8);
        self.write(addr + 1, ((val >> 8) & 0xFF) as u8);
    }

    /// Check if accumulator is in 8-bit mode
    fn is_8bit_a(&self) -> bool {
        self.emulation || (self.status & FLAG_MEMORY) != 0
    }

    /// Check if index registers are in 8-bit mode
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

    /// Get X register value (8 or 16 bit depending on mode)
    pub fn get_x(&self) -> u16 {
        if self.is_8bit_xy() {
            self.x & 0xFF
        } else {
            self.x
        }
    }

    /// Set X register value (8 or 16 bit depending on mode)
    pub fn set_x(&mut self, val: u16) {
        if self.is_8bit_xy() {
            self.x = (self.x & 0xFF00) | (val & 0xFF);
        } else {
            self.x = val;
        }
    }

    /// Get Y register value (8 or 16 bit depending on mode)
    pub fn get_y(&self) -> u16 {
        if self.is_8bit_xy() {
            self.y & 0xFF
        } else {
            self.y
        }
    }

    /// Set Y register value (8 or 16 bit depending on mode)
    pub fn set_y(&mut self, val: u16) {
        if self.is_8bit_xy() {
            self.y = (self.y & 0xFF00) | (val & 0xFF);
        } else {
            self.y = val;
        }
    }

    /// Set zero and negative flags for 8-bit value
    fn set_zn_8(&mut self, val: u8) {
        if val == 0 {
            self.status |= FLAG_ZERO;
        } else {
            self.status &= !FLAG_ZERO;
        }
        if (val & 0x80) != 0 {
            self.status |= FLAG_NEGATIVE;
        } else {
            self.status &= !FLAG_NEGATIVE;
        }
    }

    /// Set zero and negative flags for 16-bit value
    fn set_zn_16(&mut self, val: u16) {
        if val == 0 {
            self.status |= FLAG_ZERO;
        } else {
            self.status &= !FLAG_ZERO;
        }
        if (val & 0x8000) != 0 {
            self.status |= FLAG_NEGATIVE;
        } else {
            self.status &= !FLAG_NEGATIVE;
        }
    }

    /// Push a byte onto the stack
    fn push_byte(&mut self, val: u8) {
        let addr = self.s as u32;
        self.write(addr, val);
        if self.emulation {
            // In emulation mode, S wraps within page $01
            self.s = 0x0100 | ((self.s - 1) & 0xFF);
        } else {
            self.s = self.s.wrapping_sub(1);
        }
    }

    /// Pop a byte from the stack
    fn pop_byte(&mut self) -> u8 {
        if self.emulation {
            self.s = 0x0100 | ((self.s + 1) & 0xFF);
        } else {
            self.s = self.s.wrapping_add(1);
        }
        let addr = self.s as u32;
        self.read(addr)
    }

    /// Push a 16-bit word onto the stack
    fn push_word(&mut self, val: u16) {
        self.push_byte(((val >> 8) & 0xFF) as u8);
        self.push_byte((val & 0xFF) as u8);
    }

    /// Pop a 16-bit word from the stack
    fn pop_word(&mut self) -> u16 {
        let lo = self.pop_byte() as u16;
        let hi = self.pop_byte() as u16;
        (hi << 8) | lo
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

    #[test]
    fn test_lda_immediate_8bit() {
        let mut mem = ArrayMemory::new();
        mem.write(0x8000, 0xA9); // LDA #$42
        mem.write(0x8001, 0x42);

        let mut cpu = Cpu65c816::new(mem);
        cpu.pc = 0x8000;
        cpu.pbr = 0;

        cpu.step();
        assert_eq!(cpu.get_a(), 0x42);
        assert_eq!(cpu.pc, 0x8002);
    }

    #[test]
    fn test_lda_immediate_16bit() {
        let mut mem = ArrayMemory::new();
        mem.write(0x8000, 0xA9); // LDA #$1234
        mem.write(0x8001, 0x34);
        mem.write(0x8002, 0x12);

        let mut cpu = Cpu65c816::new(mem);
        cpu.pc = 0x8000;
        cpu.pbr = 0;
        cpu.emulation = false;
        cpu.status &= !FLAG_MEMORY;

        cpu.step();
        assert_eq!(cpu.c, 0x1234);
        assert_eq!(cpu.pc, 0x8003);
    }

    #[test]
    fn test_ldx_ldy_8bit() {
        let mut mem = ArrayMemory::new();
        mem.write(0x8000, 0xA2); // LDX #$55
        mem.write(0x8001, 0x55);
        mem.write(0x8002, 0xA0); // LDY #$66
        mem.write(0x8003, 0x66);

        let mut cpu = Cpu65c816::new(mem);
        cpu.pc = 0x8000;
        cpu.pbr = 0;

        cpu.step();
        assert_eq!(cpu.get_x(), 0x55);

        cpu.step();
        assert_eq!(cpu.get_y(), 0x66);
    }

    #[test]
    fn test_sta_absolute() {
        let mut mem = ArrayMemory::new();
        mem.write(0x8000, 0x8D); // STA $2000
        mem.write(0x8001, 0x00);
        mem.write(0x8002, 0x20);

        let mut cpu = Cpu65c816::new(mem);
        cpu.pc = 0x8000;
        cpu.pbr = 0;
        cpu.dbr = 0;
        cpu.c = 0x42;

        cpu.step();
        // In 8-bit mode (emulation), only low byte is stored
        assert_eq!(cpu.memory.read(0x002000), 0x42);
    }

    #[test]
    fn test_adc_8bit() {
        let mut mem = ArrayMemory::new();
        mem.write(0x8000, 0xA9); // LDA #$40
        mem.write(0x8001, 0x40);
        mem.write(0x8002, 0x69); // ADC #$30
        mem.write(0x8003, 0x30);

        let mut cpu = Cpu65c816::new(mem);
        cpu.pc = 0x8000;
        cpu.pbr = 0;

        cpu.step(); // LDA
        cpu.step(); // ADC
        assert_eq!(cpu.get_a(), 0x70);
        assert_eq!(cpu.status & FLAG_CARRY, 0);
    }

    #[test]
    fn test_adc_8bit_with_carry() {
        let mut mem = ArrayMemory::new();
        mem.write(0x8000, 0xA9); // LDA #$FF
        mem.write(0x8001, 0xFF);
        mem.write(0x8002, 0x69); // ADC #$02
        mem.write(0x8003, 0x02);

        let mut cpu = Cpu65c816::new(mem);
        cpu.pc = 0x8000;
        cpu.pbr = 0;

        cpu.step(); // LDA
        cpu.step(); // ADC
        assert_eq!(cpu.get_a(), 0x01);
        assert_eq!(cpu.status & FLAG_CARRY, FLAG_CARRY);
    }

    #[test]
    fn test_sbc_8bit() {
        let mut mem = ArrayMemory::new();
        mem.write(0x8000, 0xA9); // LDA #$50
        mem.write(0x8001, 0x50);
        mem.write(0x8002, 0x38); // SEC
        mem.write(0x8003, 0xE9); // SBC #$20
        mem.write(0x8004, 0x20);

        let mut cpu = Cpu65c816::new(mem);
        cpu.pc = 0x8000;
        cpu.pbr = 0;

        cpu.step(); // LDA
        cpu.step(); // SEC
        cpu.step(); // SBC
        assert_eq!(cpu.get_a(), 0x30);
    }

    #[test]
    fn test_cmp_8bit() {
        let mut mem = ArrayMemory::new();
        mem.write(0x8000, 0xA9); // LDA #$50
        mem.write(0x8001, 0x50);
        mem.write(0x8002, 0xC9); // CMP #$50
        mem.write(0x8003, 0x50);

        let mut cpu = Cpu65c816::new(mem);
        cpu.pc = 0x8000;
        cpu.pbr = 0;

        cpu.step(); // LDA
        cpu.step(); // CMP
        assert_eq!(cpu.status & FLAG_ZERO, FLAG_ZERO);
        assert_eq!(cpu.status & FLAG_CARRY, FLAG_CARRY);
    }

    #[test]
    fn test_inx_iny_dex_dey_8bit() {
        let mut mem = ArrayMemory::new();
        mem.write(0x8000, 0xA2); // LDX #$10
        mem.write(0x8001, 0x10);
        mem.write(0x8002, 0xE8); // INX
        mem.write(0x8003, 0xCA); // DEX

        let mut cpu = Cpu65c816::new(mem);
        cpu.pc = 0x8000;
        cpu.pbr = 0;

        cpu.step(); // LDX
        assert_eq!(cpu.get_x(), 0x10);

        cpu.step(); // INX
        assert_eq!(cpu.get_x(), 0x11);

        cpu.step(); // DEX
        assert_eq!(cpu.get_x(), 0x10);
    }

    #[test]
    fn test_transfers() {
        let mut mem = ArrayMemory::new();
        mem.write(0x8000, 0xA9); // LDA #$42
        mem.write(0x8001, 0x42);
        mem.write(0x8002, 0xAA); // TAX
        mem.write(0x8003, 0xA8); // TAY

        let mut cpu = Cpu65c816::new(mem);
        cpu.pc = 0x8000;
        cpu.pbr = 0;

        cpu.step(); // LDA
        assert_eq!(cpu.get_a(), 0x42);

        cpu.step(); // TAX
        assert_eq!(cpu.get_x(), 0x42);

        cpu.step(); // TAY
        assert_eq!(cpu.get_y(), 0x42);
    }

    #[test]
    fn test_rep_sep() {
        let mut mem = ArrayMemory::new();
        mem.write(0x8000, 0xC2); // REP #$30 (clear m and x flags)
        mem.write(0x8001, 0x30);
        mem.write(0x8002, 0xE2); // SEP #$30 (set m and x flags)
        mem.write(0x8003, 0x30);

        let mut cpu = Cpu65c816::new(mem);
        cpu.pc = 0x8000;
        cpu.pbr = 0;
        cpu.emulation = false;
        cpu.status = 0xFF;

        cpu.step(); // REP
        assert_eq!(cpu.status & FLAG_MEMORY, 0);
        assert_eq!(cpu.status & FLAG_INDEX, 0);

        cpu.step(); // SEP
        assert_eq!(cpu.status & FLAG_MEMORY, FLAG_MEMORY);
        assert_eq!(cpu.status & FLAG_INDEX, FLAG_INDEX);
    }

    #[test]
    fn test_branch_taken() {
        let mut mem = ArrayMemory::new();
        mem.write(0x8000, 0xD0); // BNE +5
        mem.write(0x8001, 0x05);

        let mut cpu = Cpu65c816::new(mem);
        cpu.pc = 0x8000;
        cpu.pbr = 0;
        cpu.status &= !FLAG_ZERO; // Clear zero flag

        cpu.step();
        assert_eq!(cpu.pc, 0x8007); // 0x8002 + 5
    }

    #[test]
    fn test_branch_not_taken() {
        let mut mem = ArrayMemory::new();
        mem.write(0x8000, 0xD0); // BNE +5
        mem.write(0x8001, 0x05);

        let mut cpu = Cpu65c816::new(mem);
        cpu.pc = 0x8000;
        cpu.pbr = 0;
        cpu.status |= FLAG_ZERO; // Set zero flag

        cpu.step();
        assert_eq!(cpu.pc, 0x8002); // Branch not taken
    }

    #[test]
    fn test_jsr_rts() {
        let mut mem = ArrayMemory::new();
        mem.write(0x8000, 0x20); // JSR $9000
        mem.write(0x8001, 0x00);
        mem.write(0x8002, 0x90);
        mem.write(0x9000, 0x60); // RTS

        let mut cpu = Cpu65c816::new(mem);
        cpu.pc = 0x8000;
        cpu.pbr = 0;
        cpu.s = 0x01FF;

        cpu.step(); // JSR
        assert_eq!(cpu.pc, 0x9000);

        cpu.step(); // RTS
        assert_eq!(cpu.pc, 0x8003);
    }

    #[test]
    fn test_push_pull_8bit() {
        let mut mem = ArrayMemory::new();
        mem.write(0x8000, 0xA9); // LDA #$42
        mem.write(0x8001, 0x42);
        mem.write(0x8002, 0x48); // PHA
        mem.write(0x8003, 0xA9); // LDA #$00
        mem.write(0x8004, 0x00);
        mem.write(0x8005, 0x68); // PLA

        let mut cpu = Cpu65c816::new(mem);
        cpu.pc = 0x8000;
        cpu.pbr = 0;
        cpu.s = 0x01FF;

        cpu.step(); // LDA #$42
        cpu.step(); // PHA
        cpu.step(); // LDA #$00
        assert_eq!(cpu.get_a(), 0x00);
        cpu.step(); // PLA
        assert_eq!(cpu.get_a(), 0x42);
    }

    #[test]
    fn test_push_pull_16bit() {
        let mut mem = ArrayMemory::new();
        mem.write(0x8000, 0xA9); // LDA #$1234
        mem.write(0x8001, 0x34);
        mem.write(0x8002, 0x12);
        mem.write(0x8003, 0x48); // PHA
        mem.write(0x8004, 0xA9); // LDA #$0000
        mem.write(0x8005, 0x00);
        mem.write(0x8006, 0x00);
        mem.write(0x8007, 0x68); // PLA

        let mut cpu = Cpu65c816::new(mem);
        cpu.pc = 0x8000;
        cpu.pbr = 0;
        cpu.s = 0x01FF;
        cpu.emulation = false;
        cpu.status &= !FLAG_MEMORY;

        cpu.step(); // LDA #$1234
        cpu.step(); // PHA
        cpu.step(); // LDA #$0000
        assert_eq!(cpu.c, 0x0000);
        cpu.step(); // PLA
        assert_eq!(cpu.c, 0x1234);
    }

    #[test]
    fn test_asl_lsr_8bit() {
        let mut mem = ArrayMemory::new();
        mem.write(0x8000, 0xA9); // LDA #$80
        mem.write(0x8001, 0x80);
        mem.write(0x8002, 0x0A); // ASL
        mem.write(0x8003, 0x4A); // LSR

        let mut cpu = Cpu65c816::new(mem);
        cpu.pc = 0x8000;
        cpu.pbr = 0;

        cpu.step(); // LDA
        cpu.step(); // ASL
        assert_eq!(cpu.get_a(), 0x00);
        assert_eq!(cpu.status & FLAG_CARRY, FLAG_CARRY);

        cpu.step(); // LSR
        assert_eq!(cpu.get_a(), 0x00);
    }

    #[test]
    fn test_rol_ror_8bit() {
        let mut mem = ArrayMemory::new();
        mem.write(0x8000, 0xA9); // LDA #$81
        mem.write(0x8001, 0x81);
        mem.write(0x8002, 0x2A); // ROL
        mem.write(0x8003, 0x6A); // ROR

        let mut cpu = Cpu65c816::new(mem);
        cpu.pc = 0x8000;
        cpu.pbr = 0;
        cpu.status &= !FLAG_CARRY;

        cpu.step(); // LDA
        cpu.step(); // ROL
        assert_eq!(cpu.get_a(), 0x02);
        assert_eq!(cpu.status & FLAG_CARRY, FLAG_CARRY);

        cpu.step(); // ROR
        assert_eq!(cpu.get_a(), 0x81);
    }

    #[test]
    fn test_and_ora_eor_8bit() {
        let mut mem = ArrayMemory::new();
        mem.write(0x8000, 0xA9); // LDA #$FF
        mem.write(0x8001, 0xFF);
        mem.write(0x8002, 0x29); // AND #$0F
        mem.write(0x8003, 0x0F);
        mem.write(0x8004, 0x09); // ORA #$F0
        mem.write(0x8005, 0xF0);
        mem.write(0x8006, 0x49); // EOR #$AA
        mem.write(0x8007, 0xAA);

        let mut cpu = Cpu65c816::new(mem);
        cpu.pc = 0x8000;
        cpu.pbr = 0;

        cpu.step(); // LDA
        assert_eq!(cpu.get_a(), 0xFF);

        cpu.step(); // AND
        assert_eq!(cpu.get_a(), 0x0F);

        cpu.step(); // ORA
        assert_eq!(cpu.get_a(), 0xFF);

        cpu.step(); // EOR
        assert_eq!(cpu.get_a(), 0x55);
    }

    #[test]
    fn test_flag_instructions() {
        let mut mem = ArrayMemory::new();
        mem.write(0x8000, 0x18); // CLC
        mem.write(0x8001, 0x38); // SEC
        mem.write(0x8002, 0x58); // CLI
        mem.write(0x8003, 0x78); // SEI
        mem.write(0x8004, 0xB8); // CLV
        mem.write(0x8005, 0xD8); // CLD
        mem.write(0x8006, 0xF8); // SED

        let mut cpu = Cpu65c816::new(mem);
        cpu.pc = 0x8000;
        cpu.pbr = 0;

        cpu.step(); // CLC
        assert_eq!(cpu.status & FLAG_CARRY, 0);

        cpu.step(); // SEC
        assert_eq!(cpu.status & FLAG_CARRY, FLAG_CARRY);

        cpu.step(); // CLI
        assert_eq!(cpu.status & FLAG_IRQ_DISABLE, 0);

        cpu.step(); // SEI
        assert_eq!(cpu.status & FLAG_IRQ_DISABLE, FLAG_IRQ_DISABLE);

        cpu.status |= FLAG_OVERFLOW;
        cpu.step(); // CLV
        assert_eq!(cpu.status & FLAG_OVERFLOW, 0);

        cpu.step(); // CLD
        assert_eq!(cpu.status & FLAG_DECIMAL, 0);

        cpu.step(); // SED
        assert_eq!(cpu.status & FLAG_DECIMAL, FLAG_DECIMAL);
    }

    #[test]
    fn test_16bit_arithmetic() {
        let mut mem = ArrayMemory::new();
        mem.write(0x8000, 0xA9); // LDA #$1000
        mem.write(0x8001, 0x00);
        mem.write(0x8002, 0x10);
        mem.write(0x8003, 0x69); // ADC #$0500
        mem.write(0x8004, 0x00);
        mem.write(0x8005, 0x05);

        let mut cpu = Cpu65c816::new(mem);
        cpu.pc = 0x8000;
        cpu.pbr = 0;
        cpu.emulation = false;
        cpu.status &= !FLAG_MEMORY;

        cpu.step(); // LDA
        assert_eq!(cpu.c, 0x1000);

        cpu.step(); // ADC
        assert_eq!(cpu.c, 0x1500);
    }

    #[test]
    fn test_16bit_index_registers() {
        let mut mem = ArrayMemory::new();
        mem.write(0x8000, 0xA2); // LDX #$8000
        mem.write(0x8001, 0x00);
        mem.write(0x8002, 0x80);
        mem.write(0x8003, 0xE8); // INX

        let mut cpu = Cpu65c816::new(mem);
        cpu.pc = 0x8000;
        cpu.pbr = 0;
        cpu.emulation = false;
        cpu.status &= !FLAG_INDEX;

        cpu.step(); // LDX
        assert_eq!(cpu.x, 0x8000);

        cpu.step(); // INX
        assert_eq!(cpu.x, 0x8001);
    }
}
