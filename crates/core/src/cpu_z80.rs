//! Zilog Z80 CPU core implementation
//!
//! The Z80 extends the 8080 with additional registers and instructions.
//! This module provides a reusable Z80 implementation.
//!
//! For detailed CPU reference documentation, see: `docs/references/cpu_z80.md`

/// Memory interface trait for the Z80 CPU
pub trait MemoryZ80 {
    /// Read a byte from memory
    fn read(&self, addr: u16) -> u8;

    /// Write a byte to memory
    fn write(&mut self, addr: u16, val: u8);

    /// Read from I/O port
    fn io_read(&mut self, port: u8) -> u8 {
        let _ = port;
        0xFF
    }

    /// Write to I/O port
    fn io_write(&mut self, port: u8, val: u8) {
        let _ = (port, val);
    }
}

/// Zilog Z80 CPU state
#[derive(Debug)]
pub struct CpuZ80<M: MemoryZ80> {
    /// Main registers
    pub a: u8,
    pub f: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub h: u8,
    pub l: u8,

    /// Shadow registers (Z80 specific)
    pub a_prime: u8,
    pub f_prime: u8,
    pub b_prime: u8,
    pub c_prime: u8,
    pub d_prime: u8,
    pub e_prime: u8,
    pub h_prime: u8,
    pub l_prime: u8,

    /// Index registers (Z80 specific)
    pub ix: u16,
    pub iy: u16,

    /// Special registers
    pub i: u8, // Interrupt vector
    pub r: u8, // Memory refresh

    /// Stack pointer
    pub sp: u16,
    /// Program counter
    pub pc: u16,

    /// Interrupt flags
    pub iff1: bool,
    pub iff2: bool,
    pub im: u8, // Interrupt mode (0, 1, or 2)

    /// State
    pub halted: bool,
    pub cycles: u64,

    /// Memory interface
    pub memory: M,
}

impl<M: MemoryZ80> CpuZ80<M> {
    /// Create a new Z80 CPU
    pub fn new(memory: M) -> Self {
        Self {
            a: 0,
            f: 0,
            b: 0,
            c: 0,
            d: 0,
            e: 0,
            h: 0,
            l: 0,
            a_prime: 0,
            f_prime: 0,
            b_prime: 0,
            c_prime: 0,
            d_prime: 0,
            e_prime: 0,
            h_prime: 0,
            l_prime: 0,
            ix: 0,
            iy: 0,
            i: 0,
            r: 0,
            sp: 0,
            pc: 0,
            iff1: false,
            iff2: false,
            im: 0,
            halted: false,
            cycles: 0,
            memory,
        }
    }

    /// Reset the CPU
    pub fn reset(&mut self) {
        self.a = 0;
        self.f = 0;
        self.b = 0;
        self.c = 0;
        self.d = 0;
        self.e = 0;
        self.h = 0;
        self.l = 0;
        self.sp = 0;
        self.pc = 0;
        self.iff1 = false;
        self.iff2 = false;
        self.im = 0;
        self.halted = false;
        self.cycles = 0;
    }

    /// Execute one instruction
    pub fn step(&mut self) -> u32 {
        if self.halted {
            return 4;
        }

        let opcode = self.read_pc();
        let cycles = self.execute(opcode);
        self.cycles += cycles as u64;
        cycles
    }

    // Helper methods
    fn read_pc(&mut self) -> u8 {
        let val = self.memory.read(self.pc);
        self.pc = self.pc.wrapping_add(1);
        self.r = (self.r & 0x80) | ((self.r.wrapping_add(1)) & 0x7F); // R refresh register
        val
    }

    fn read_pc_u16(&mut self) -> u16 {
        let lo = self.read_pc() as u16;
        let hi = self.read_pc() as u16;
        (hi << 8) | lo
    }

    fn push_u16(&mut self, val: u16) {
        self.sp = self.sp.wrapping_sub(1);
        self.memory.write(self.sp, (val >> 8) as u8);
        self.sp = self.sp.wrapping_sub(1);
        self.memory.write(self.sp, val as u8);
    }

    fn pop_u16(&mut self) -> u16 {
        let lo = self.memory.read(self.sp) as u16;
        self.sp = self.sp.wrapping_add(1);
        let hi = self.memory.read(self.sp) as u16;
        self.sp = self.sp.wrapping_add(1);
        (hi << 8) | lo
    }

    // Register pair accessors
    fn bc(&self) -> u16 {
        ((self.b as u16) << 8) | (self.c as u16)
    }

    fn set_bc(&mut self, val: u16) {
        self.b = (val >> 8) as u8;
        self.c = val as u8;
    }

    fn de(&self) -> u16 {
        ((self.d as u16) << 8) | (self.e as u16)
    }

    fn set_de(&mut self, val: u16) {
        self.d = (val >> 8) as u8;
        self.e = val as u8;
    }

    fn hl(&self) -> u16 {
        ((self.h as u16) << 8) | (self.l as u16)
    }

    fn set_hl(&mut self, val: u16) {
        self.h = (val >> 8) as u8;
        self.l = val as u8;
    }

    fn af(&self) -> u16 {
        ((self.a as u16) << 8) | (self.f as u16)
    }

    fn set_af(&mut self, val: u16) {
        self.a = (val >> 8) as u8;
        self.f = val as u8;
    }

    // Flag operations
    fn set_flag(&mut self, flag: u8, val: bool) {
        if val {
            self.f |= flag;
        } else {
            self.f &= !flag;
        }
    }

    fn get_flag(&self, flag: u8) -> bool {
        (self.f & flag) != 0
    }

    // Update S, Z, P flags based on result
    fn update_flags_szp(&mut self, val: u8) {
        self.set_flag(0x80, (val & 0x80) != 0); // Sign
        self.set_flag(0x40, val == 0); // Zero
        self.set_flag(0x04, val.count_ones().is_multiple_of(2)); // Parity
    }

    // Arithmetic operations with flag updates
    fn add_a(&mut self, val: u8, carry: bool) {
        let c = if carry && self.get_flag(0x01) { 1 } else { 0 };
        let result = self.a as u16 + val as u16 + c as u16;

        // Half carry: carry from bit 3 to bit 4
        self.set_flag(0x10, ((self.a & 0x0F) + (val & 0x0F) + c) > 0x0F);
        self.set_flag(0x01, result > 0xFF);

        // Overflow: set if sign bits of operands are same but result differs
        let overflow = ((self.a ^ val) & 0x80) == 0 && ((self.a ^ result as u8) & 0x80) != 0;
        self.set_flag(0x04, overflow);

        self.a = result as u8;
        self.set_flag(0x02, false); // N flag
        self.update_flags_szp(self.a);
    }

    fn sub_a(&mut self, val: u8, carry: bool) {
        let c = if carry && self.get_flag(0x01) { 1 } else { 0 };
        let result = self.a as i16 - val as i16 - c as i16;

        // Half carry: borrow from bit 4
        self.set_flag(0x10, (self.a & 0x0F) < ((val & 0x0F) + c));
        self.set_flag(0x01, result < 0);

        // Overflow: set if sign bits differ and result has different sign than minuend
        let overflow = ((self.a ^ val) & 0x80) != 0 && ((self.a ^ result as u8) & 0x80) != 0;
        self.set_flag(0x04, overflow);

        self.a = result as u8;
        self.set_flag(0x02, true); // N flag
        self.update_flags_szp(self.a);
    }

    fn and_a(&mut self, val: u8) {
        self.a &= val;
        self.set_flag(0x01, false); // Carry
        self.set_flag(0x02, false); // N
        self.set_flag(0x10, true); // H (always set for AND)
        self.update_flags_szp(self.a);
    }

    fn xor_a(&mut self, val: u8) {
        self.a ^= val;
        self.set_flag(0x01, false);
        self.set_flag(0x02, false);
        self.set_flag(0x10, false);
        self.update_flags_szp(self.a);
    }

    fn or_a(&mut self, val: u8) {
        self.a |= val;
        self.set_flag(0x01, false);
        self.set_flag(0x02, false);
        self.set_flag(0x10, false);
        self.update_flags_szp(self.a);
    }

    fn cp_a(&mut self, val: u8) {
        let result = self.a as i16 - val as i16;
        self.set_flag(0x10, (self.a & 0x0F) < (val & 0x0F));
        self.set_flag(0x01, result < 0);

        let overflow = ((self.a ^ val) & 0x80) != 0 && ((self.a ^ result as u8) & 0x80) != 0;
        self.set_flag(0x04, overflow);

        self.set_flag(0x02, true);
        self.update_flags_szp(result as u8);
    }

    fn inc(&mut self, val: u8) -> u8 {
        let result = val.wrapping_add(1);
        self.set_flag(0x10, (val & 0x0F) == 0x0F);
        self.set_flag(0x04, val == 0x7F);
        self.set_flag(0x02, false);
        self.update_flags_szp(result);
        result
    }

    fn dec(&mut self, val: u8) -> u8 {
        let result = val.wrapping_sub(1);
        self.set_flag(0x10, (val & 0x0F) == 0);
        self.set_flag(0x04, val == 0x80);
        self.set_flag(0x02, true);
        self.update_flags_szp(result);
        result
    }

    // Condition code evaluation for conditional jumps/calls/returns
    fn check_condition(&self, cc: u8) -> bool {
        match cc {
            0 => !self.get_flag(0x40), // NZ
            1 => self.get_flag(0x40),  // Z
            2 => !self.get_flag(0x01), // NC
            3 => self.get_flag(0x01),  // C
            4 => !self.get_flag(0x04), // PO
            5 => self.get_flag(0x04),  // PE
            6 => !self.get_flag(0x80), // P
            7 => self.get_flag(0x80),  // M
            _ => false,
        }
    }

    fn execute(&mut self, opcode: u8) -> u32 {
        match opcode {
            // NOP
            0x00 => 4,

            // LD BC,nn / LD DE,nn / LD HL,nn / LD SP,nn
            0x01 => {
                let val = self.read_pc_u16();
                self.set_bc(val);
                10
            }
            0x11 => {
                let val = self.read_pc_u16();
                self.set_de(val);
                10
            }
            0x21 => {
                let val = self.read_pc_u16();
                self.set_hl(val);
                10
            }
            0x31 => {
                self.sp = self.read_pc_u16();
                10
            }

            // LD (BC),A / LD (DE),A
            0x02 => {
                self.memory.write(self.bc(), self.a);
                7
            }
            0x12 => {
                self.memory.write(self.de(), self.a);
                7
            }

            // LD A,(BC) / LD A,(DE)
            0x0A => {
                self.a = self.memory.read(self.bc());
                7
            }
            0x1A => {
                self.a = self.memory.read(self.de());
                7
            }

            // LD (nn),HL / LD (nn),A
            0x22 => {
                let addr = self.read_pc_u16();
                let hl = self.hl();
                self.memory.write(addr, hl as u8);
                self.memory.write(addr.wrapping_add(1), (hl >> 8) as u8);
                16
            }
            0x32 => {
                let addr = self.read_pc_u16();
                self.memory.write(addr, self.a);
                13
            }

            // LD HL,(nn) / LD A,(nn)
            0x2A => {
                let addr = self.read_pc_u16();
                let lo = self.memory.read(addr) as u16;
                let hi = self.memory.read(addr.wrapping_add(1)) as u16;
                self.set_hl((hi << 8) | lo);
                16
            }
            0x3A => {
                let addr = self.read_pc_u16();
                self.a = self.memory.read(addr);
                13
            }

            // INC BC / INC DE / INC HL / INC SP
            0x03 => {
                self.set_bc(self.bc().wrapping_add(1));
                6
            }
            0x13 => {
                self.set_de(self.de().wrapping_add(1));
                6
            }
            0x23 => {
                self.set_hl(self.hl().wrapping_add(1));
                6
            }
            0x33 => {
                self.sp = self.sp.wrapping_add(1);
                6
            }

            // DEC BC / DEC DE / DEC HL / DEC SP
            0x0B => {
                self.set_bc(self.bc().wrapping_sub(1));
                6
            }
            0x1B => {
                self.set_de(self.de().wrapping_sub(1));
                6
            }
            0x2B => {
                self.set_hl(self.hl().wrapping_sub(1));
                6
            }
            0x3B => {
                self.sp = self.sp.wrapping_sub(1);
                6
            }

            // INC r (B, C, D, E, H, L, (HL), A)
            0x04 => {
                self.b = self.inc(self.b);
                4
            }
            0x0C => {
                self.c = self.inc(self.c);
                4
            }
            0x14 => {
                self.d = self.inc(self.d);
                4
            }
            0x1C => {
                self.e = self.inc(self.e);
                4
            }
            0x24 => {
                self.h = self.inc(self.h);
                4
            }
            0x2C => {
                self.l = self.inc(self.l);
                4
            }
            0x34 => {
                let addr = self.hl();
                let val = self.memory.read(addr);
                let result = self.inc(val);
                self.memory.write(addr, result);
                11
            }
            0x3C => {
                self.a = self.inc(self.a);
                4
            }

            // DEC r (B, C, D, E, H, L, (HL), A)
            0x05 => {
                self.b = self.dec(self.b);
                4
            }
            0x0D => {
                self.c = self.dec(self.c);
                4
            }
            0x15 => {
                self.d = self.dec(self.d);
                4
            }
            0x1D => {
                self.e = self.dec(self.e);
                4
            }
            0x25 => {
                self.h = self.dec(self.h);
                4
            }
            0x2D => {
                self.l = self.dec(self.l);
                4
            }
            0x35 => {
                let addr = self.hl();
                let val = self.memory.read(addr);
                let result = self.dec(val);
                self.memory.write(addr, result);
                11
            }
            0x3D => {
                self.a = self.dec(self.a);
                4
            }

            // LD r,n (B, C, D, E, H, L, (HL), A)
            0x06 => {
                self.b = self.read_pc();
                7
            }
            0x0E => {
                self.c = self.read_pc();
                7
            }
            0x16 => {
                self.d = self.read_pc();
                7
            }
            0x1E => {
                self.e = self.read_pc();
                7
            }
            0x26 => {
                self.h = self.read_pc();
                7
            }
            0x2E => {
                self.l = self.read_pc();
                7
            }
            0x36 => {
                let val = self.read_pc();
                self.memory.write(self.hl(), val);
                10
            }
            0x3E => {
                self.a = self.read_pc();
                7
            }

            // Rotate and shift instructions
            0x07 => {
                // RLCA
                let carry = (self.a & 0x80) != 0;
                self.a = (self.a << 1) | (if carry { 1 } else { 0 });
                self.set_flag(0x02, false); // N = 0
                self.set_flag(0x10, false); // H = 0
                self.set_flag(0x01, carry); // C = old bit 7
                4
            }
            0x0F => {
                // RRCA
                let carry = (self.a & 0x01) != 0;
                self.a = (self.a >> 1) | (if carry { 0x80 } else { 0 });
                self.set_flag(0x02, false); // N = 0
                self.set_flag(0x10, false); // H = 0
                self.set_flag(0x01, carry); // C = old bit 0
                4
            }
            0x17 => {
                // RLA
                let old_carry = if self.get_flag(0x01) { 1 } else { 0 };
                let new_carry = (self.a & 0x80) != 0;
                self.a = (self.a << 1) | old_carry;
                self.set_flag(0x02, false); // N = 0
                self.set_flag(0x10, false); // H = 0
                self.set_flag(0x01, new_carry);
                4
            }
            0x1F => {
                // RRA
                let old_carry = if self.get_flag(0x01) { 0x80 } else { 0 };
                let new_carry = (self.a & 0x01) != 0;
                self.a = (self.a >> 1) | old_carry;
                self.set_flag(0x02, false); // N = 0
                self.set_flag(0x10, false); // H = 0
                self.set_flag(0x01, new_carry);
                4
            }

            // ADD HL,rr (BC, DE, HL, SP)
            0x09 => {
                let hl = self.hl();
                let bc = self.bc();
                let result = hl.wrapping_add(bc);
                self.set_flag(0x02, false); // N = 0
                self.set_flag(0x10, ((hl & 0x0FFF) + (bc & 0x0FFF)) > 0x0FFF); // H
                self.set_flag(0x01, (hl as u32 + bc as u32) > 0xFFFF); // C
                self.set_hl(result);
                11
            }
            0x19 => {
                let hl = self.hl();
                let de = self.de();
                let result = hl.wrapping_add(de);
                self.set_flag(0x02, false);
                self.set_flag(0x10, ((hl & 0x0FFF) + (de & 0x0FFF)) > 0x0FFF);
                self.set_flag(0x01, (hl as u32 + de as u32) > 0xFFFF);
                self.set_hl(result);
                11
            }
            0x29 => {
                let hl = self.hl();
                let result = hl.wrapping_add(hl);
                self.set_flag(0x02, false);
                self.set_flag(0x10, ((hl & 0x0FFF) + (hl & 0x0FFF)) > 0x0FFF);
                self.set_flag(0x01, (hl as u32 + hl as u32) > 0xFFFF);
                self.set_hl(result);
                11
            }
            0x39 => {
                let hl = self.hl();
                let sp = self.sp;
                let result = hl.wrapping_add(sp);
                self.set_flag(0x02, false);
                self.set_flag(0x10, ((hl & 0x0FFF) + (sp & 0x0FFF)) > 0x0FFF);
                self.set_flag(0x01, (hl as u32 + sp as u32) > 0xFFFF);
                self.set_hl(result);
                11
            }

            // JR cc,e (conditional relative jumps)
            0x20 => {
                let offset = self.read_pc() as i8;
                if !self.get_flag(0x40) {
                    // NZ
                    self.pc = self.pc.wrapping_add(offset as u16);
                    12
                } else {
                    7
                }
            }
            0x28 => {
                let offset = self.read_pc() as i8;
                if self.get_flag(0x40) {
                    // Z
                    self.pc = self.pc.wrapping_add(offset as u16);
                    12
                } else {
                    7
                }
            }
            0x30 => {
                let offset = self.read_pc() as i8;
                if !self.get_flag(0x01) {
                    // NC
                    self.pc = self.pc.wrapping_add(offset as u16);
                    12
                } else {
                    7
                }
            }
            0x38 => {
                let offset = self.read_pc() as i8;
                if self.get_flag(0x01) {
                    // C
                    self.pc = self.pc.wrapping_add(offset as u16);
                    12
                } else {
                    7
                }
            }

            // DJNZ e (Decrement B and jump if not zero)
            0x10 => {
                let offset = self.read_pc() as i8;
                self.b = self.b.wrapping_sub(1);
                if self.b != 0 {
                    self.pc = self.pc.wrapping_add(offset as u16);
                    13
                } else {
                    8
                }
            }

            // JR e (unconditional relative jump)
            0x18 => {
                let offset = self.read_pc() as i8;
                self.pc = self.pc.wrapping_add(offset as u16);
                12
            }

            // LD r,r' (8-bit register to register moves)
            // Row 0x40-0x7F contains most LD r,r' instructions
            0x40..=0x7F if opcode != 0x76 => {
                let dst = (opcode >> 3) & 0x07;
                let src = opcode & 0x07;

                let val = match src {
                    0 => self.b,
                    1 => self.c,
                    2 => self.d,
                    3 => self.e,
                    4 => self.h,
                    5 => self.l,
                    6 => self.memory.read(self.hl()),
                    7 => self.a,
                    _ => unreachable!(),
                };

                match dst {
                    0 => self.b = val,
                    1 => self.c = val,
                    2 => self.d = val,
                    3 => self.e = val,
                    4 => self.h = val,
                    5 => self.l = val,
                    6 => self.memory.write(self.hl(), val),
                    7 => self.a = val,
                    _ => unreachable!(),
                }

                if src == 6 || dst == 6 {
                    7 // Memory access
                } else {
                    4 // Register to register
                }
            }

            // HALT
            0x76 => {
                self.halted = true;
                4
            }

            // ADD A,r / ADC A,r / SUB r / SBC A,r / AND r / XOR r / OR r / CP r
            0x80..=0xBF => {
                let op = (opcode >> 3) & 0x07;
                let src = opcode & 0x07;

                let val = match src {
                    0 => self.b,
                    1 => self.c,
                    2 => self.d,
                    3 => self.e,
                    4 => self.h,
                    5 => self.l,
                    6 => self.memory.read(self.hl()),
                    7 => self.a,
                    _ => unreachable!(),
                };

                match op {
                    0 => self.add_a(val, false), // ADD
                    1 => self.add_a(val, true),  // ADC
                    2 => self.sub_a(val, false), // SUB
                    3 => self.sub_a(val, true),  // SBC
                    4 => self.and_a(val),        // AND
                    5 => self.xor_a(val),        // XOR
                    6 => self.or_a(val),         // OR
                    7 => self.cp_a(val),         // CP
                    _ => unreachable!(),
                }

                if src == 6 {
                    7 // Memory access
                } else {
                    4 // Register
                }
            }

            // RET cc (conditional returns)
            0xC0 | 0xC8 | 0xD0 | 0xD8 | 0xE0 | 0xE8 | 0xF0 | 0xF8 => {
                let cc = (opcode >> 3) & 0x07;
                if self.check_condition(cc) {
                    self.pc = self.pop_u16();
                    11
                } else {
                    5
                }
            }

            // POP BC / POP DE / POP HL / POP AF
            0xC1 => {
                let val = self.pop_u16();
                self.set_bc(val);
                10
            }
            0xD1 => {
                let val = self.pop_u16();
                self.set_de(val);
                10
            }
            0xE1 => {
                let val = self.pop_u16();
                self.set_hl(val);
                10
            }
            0xF1 => {
                let val = self.pop_u16();
                self.set_af(val);
                10
            }

            // JP cc,nn (conditional jumps)
            0xC2 | 0xCA | 0xD2 | 0xDA | 0xE2 | 0xEA | 0xF2 | 0xFA => {
                let addr = self.read_pc_u16();
                let cc = (opcode >> 3) & 0x07;
                if self.check_condition(cc) {
                    self.pc = addr;
                }
                10
            }

            // JP nn (unconditional jump)
            0xC3 => {
                self.pc = self.read_pc_u16();
                10
            }

            // CALL cc,nn (conditional calls)
            0xC4 | 0xCC | 0xD4 | 0xDC | 0xE4 | 0xEC | 0xF4 | 0xFC => {
                let addr = self.read_pc_u16();
                let cc = (opcode >> 3) & 0x07;
                if self.check_condition(cc) {
                    self.push_u16(self.pc);
                    self.pc = addr;
                    17
                } else {
                    10
                }
            }

            // PUSH BC / PUSH DE / PUSH HL / PUSH AF
            0xC5 => {
                self.push_u16(self.bc());
                11
            }
            0xD5 => {
                self.push_u16(self.de());
                11
            }
            0xE5 => {
                self.push_u16(self.hl());
                11
            }
            0xF5 => {
                self.push_u16(self.af());
                11
            }

            // ADD A,n / ADC A,n / SUB n / SBC A,n / AND n / XOR n / OR n / CP n
            0xC6 => {
                let val = self.read_pc();
                self.add_a(val, false);
                7
            }
            0xCE => {
                let val = self.read_pc();
                self.add_a(val, true);
                7
            }
            0xD6 => {
                let val = self.read_pc();
                self.sub_a(val, false);
                7
            }
            0xDE => {
                let val = self.read_pc();
                self.sub_a(val, true);
                7
            }
            0xE6 => {
                let val = self.read_pc();
                self.and_a(val);
                7
            }
            0xEE => {
                let val = self.read_pc();
                self.xor_a(val);
                7
            }
            0xF6 => {
                let val = self.read_pc();
                self.or_a(val);
                7
            }
            0xFE => {
                let val = self.read_pc();
                self.cp_a(val);
                7
            }

            // RST n (restart - call to fixed addresses)
            0xC7 | 0xCF | 0xD7 | 0xDF | 0xE7 | 0xEF | 0xF7 | 0xFF => {
                let addr = (opcode & 0x38) as u16;
                self.push_u16(self.pc);
                self.pc = addr;
                11
            }

            // RET (unconditional return)
            0xC9 => {
                self.pc = self.pop_u16();
                10
            }

            // CALL nn (unconditional call)
            0xCD => {
                let addr = self.read_pc_u16();
                self.push_u16(self.pc);
                self.pc = addr;
                17
            }

            // OUT (n),A
            0xD3 => {
                let port = self.read_pc();
                self.memory.io_write(port, self.a);
                11
            }

            // IN A,(n)
            0xDB => {
                let port = self.read_pc();
                self.a = self.memory.io_read(port);
                11
            }

            // EX (SP),HL
            0xE3 => {
                let sp_val = self.pop_u16();
                self.push_u16(self.hl());
                self.set_hl(sp_val);
                19
            }

            // JP (HL)
            0xE9 => {
                self.pc = self.hl();
                4
            }

            // EX DE,HL
            0xEB => {
                let de = self.de();
                let hl = self.hl();
                self.set_de(hl);
                self.set_hl(de);
                4
            }

            // DI (Disable interrupts)
            0xF3 => {
                self.iff1 = false;
                self.iff2 = false;
                4
            }

            // EI (Enable interrupts)
            0xFB => {
                self.iff1 = true;
                self.iff2 = true;
                4
            }

            // LD SP,HL
            0xF9 => {
                self.sp = self.hl();
                6
            }

            // Extended instruction sets (prefixes)
            0xCB => self.execute_cb(),
            0xED => self.execute_ed(),
            0xDD => self.execute_dd(),
            0xFD => self.execute_fd(),

            // Undefined/unsupported opcodes
            _ => 4,
        }
    }

    // CB prefix instructions (bit operations)
    fn execute_cb(&mut self) -> u32 {
        let _opcode = self.read_pc();
        // TODO: Implement full CB instruction set
        // For now, return basic timing
        8
    }

    // ED prefix instructions (extended instructions)
    fn execute_ed(&mut self) -> u32 {
        let opcode = self.read_pc();
        match opcode {
            // RETI (Return from interrupt)
            0x4D => {
                self.pc = self.pop_u16();
                self.iff1 = self.iff2;
                14
            }
            // RETN (Return from NMI)
            0x45 => {
                self.pc = self.pop_u16();
                self.iff1 = self.iff2;
                14
            }
            // IM 0
            0x46 => {
                self.im = 0;
                8
            }
            // IM 1
            0x56 => {
                self.im = 1;
                8
            }
            // IM 2
            0x5E => {
                self.im = 2;
                8
            }
            // LD I,A
            0x47 => {
                self.i = self.a;
                9
            }
            // LD R,A
            0x4F => {
                self.r = self.a;
                9
            }
            // LD A,I
            0x57 => {
                self.a = self.i;
                9
            }
            // LD A,R
            0x5F => {
                self.a = self.r;
                9
            }
            _ => 8,
        }
    }

    // DD prefix instructions (IX operations)
    fn execute_dd(&mut self) -> u32 {
        let _opcode = self.read_pc();
        // TODO: Implement IX indexed operations
        8
    }

    // FD prefix instructions (IY operations)
    fn execute_fd(&mut self) -> u32 {
        let _opcode = self.read_pc();
        // TODO: Implement IY indexed operations
        8
    }
}

impl<M: MemoryZ80> crate::Cpu for CpuZ80<M> {
    fn reset(&mut self) {
        self.reset();
    }

    fn step(&mut self) -> u32 {
        self.step()
    }
}
