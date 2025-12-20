//! Intel 8080 CPU core implementation
//!
//! This module provides a reusable 8080 CPU implementation that can be used
//! by any system (Space Invaders, CP/M systems, etc.) by implementing the `Memory8080` trait.
//!
//! The 8080 is the foundation for the Z80 and Game Boy CPUs.

/// Memory interface trait for the 8080 CPU
///
/// Systems using the 8080 must implement this trait to provide memory access.
pub trait Memory8080 {
    /// Read a byte from memory at the given address
    fn read(&self, addr: u16) -> u8;

    /// Write a byte to memory at the given address
    fn write(&mut self, addr: u16, val: u8);

    /// Read a byte from I/O port (for IN instruction)
    fn io_read(&mut self, port: u8) -> u8 {
        let _ = port;
        0xFF // Default: return 0xFF for unconnected ports
    }

    /// Write a byte to I/O port (for OUT instruction)
    fn io_write(&mut self, port: u8, val: u8) {
        let _ = (port, val);
        // Default: no-op
    }
}

/// Intel 8080 CPU state and execution engine
#[derive(Debug)]
pub struct Cpu8080<M: Memory8080> {
    /// Accumulator register
    pub a: u8,
    /// B register
    pub b: u8,
    /// C register
    pub c: u8,
    /// D register
    pub d: u8,
    /// E register
    pub e: u8,
    /// H register
    pub h: u8,
    /// L register
    pub l: u8,
    /// Stack pointer
    pub sp: u16,
    /// Program counter
    pub pc: u16,
    /// Flags register (S Z 0 AC 0 P 1 C)
    pub flags: u8,
    /// Total cycles executed
    pub cycles: u64,
    /// Memory interface
    pub memory: M,
    /// Interrupt enabled flag
    pub inte: bool,
    /// Halted flag
    pub halted: bool,
}

// Flag bit positions
const FLAG_S: u8 = 0b10000000; // Sign
const FLAG_Z: u8 = 0b01000000; // Zero
const FLAG_AC: u8 = 0b00010000; // Auxiliary Carry (half-carry)
const FLAG_P: u8 = 0b00000100; // Parity
const FLAG_C: u8 = 0b00000001; // Carry

impl<M: Memory8080> Cpu8080<M> {
    /// Create a new 8080 CPU with the given memory interface
    pub fn new(memory: M) -> Self {
        Self {
            a: 0,
            b: 0,
            c: 0,
            d: 0,
            e: 0,
            h: 0,
            l: 0,
            sp: 0,
            pc: 0,
            flags: 0b00000010, // Bit 1 is always 1
            cycles: 0,
            memory,
            inte: false,
            halted: false,
        }
    }

    /// Reset the CPU to initial state
    pub fn reset(&mut self) {
        self.a = 0;
        self.b = 0;
        self.c = 0;
        self.d = 0;
        self.e = 0;
        self.h = 0;
        self.l = 0;
        self.sp = 0;
        self.pc = 0;
        self.flags = 0b00000010;
        self.cycles = 0;
        self.inte = false;
        self.halted = false;
    }

    /// Execute one instruction and return cycles consumed
    pub fn step(&mut self) -> u32 {
        if self.halted {
            return 4; // HLT consumes cycles
        }

        let opcode = self.read_pc();
        self.execute(opcode)
    }

    /// Trigger an interrupt (RST instruction to vector)
    pub fn interrupt(&mut self, vector: u8) {
        if self.inte {
            self.halted = false;
            self.inte = false;
            self.push_u16(self.pc);
            self.pc = (vector & 0x38) as u16; // RST 0-7
        }
    }

    // Helper methods
    fn read_pc(&mut self) -> u8 {
        let val = self.memory.read(self.pc);
        self.pc = self.pc.wrapping_add(1);
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

    // Flag operations
    fn set_flag(&mut self, flag: u8, val: bool) {
        if val {
            self.flags |= flag;
        } else {
            self.flags &= !flag;
        }
    }

    fn get_flag(&self, flag: u8) -> bool {
        (self.flags & flag) != 0
    }

    fn update_flags_szp(&mut self, val: u8) {
        self.set_flag(FLAG_S, (val & 0x80) != 0);
        self.set_flag(FLAG_Z, val == 0);
        self.set_flag(FLAG_P, val.count_ones() % 2 == 0);
    }

    fn update_flags_ac_add(&mut self, a: u8, b: u8, carry: u8) {
        self.set_flag(FLAG_AC, ((a & 0x0F) + (b & 0x0F) + carry) > 0x0F);
    }

    fn update_flags_ac_sub(&mut self, a: u8, b: u8, borrow: u8) {
        // For subtraction, AC is set if borrow from bit 4
        self.set_flag(FLAG_AC, (a & 0x0F) < ((b & 0x0F) + borrow));
    }

    // Arithmetic operations
    fn add(&mut self, val: u8, carry: bool) {
        let c = if carry && self.get_flag(FLAG_C) { 1 } else { 0 };
        let result = self.a as u16 + val as u16 + c as u16;

        self.update_flags_ac_add(self.a, val, c);
        self.set_flag(FLAG_C, result > 0xFF);
        self.a = result as u8;
        self.update_flags_szp(self.a);
    }

    fn sub(&mut self, val: u8, carry: bool) {
        let c = if carry && self.get_flag(FLAG_C) { 1 } else { 0 };
        let result = self.a as i16 - val as i16 - c as i16;

        self.update_flags_ac_sub(self.a, val, c);
        self.set_flag(FLAG_C, result < 0);
        self.a = result as u8;
        self.update_flags_szp(self.a);
    }

    fn ana(&mut self, val: u8) {
        self.a &= val;
        self.set_flag(FLAG_C, false);
        self.set_flag(FLAG_AC, ((self.a | val) & 0x08) != 0); // Special AC behavior for AND
        self.update_flags_szp(self.a);
    }

    fn xra(&mut self, val: u8) {
        self.a ^= val;
        self.set_flag(FLAG_C, false);
        self.set_flag(FLAG_AC, false);
        self.update_flags_szp(self.a);
    }

    fn ora(&mut self, val: u8) {
        self.a |= val;
        self.set_flag(FLAG_C, false);
        self.set_flag(FLAG_AC, false);
        self.update_flags_szp(self.a);
    }

    fn cmp(&mut self, val: u8) {
        let result = self.a as i16 - val as i16;
        self.update_flags_ac_sub(self.a, val, 0);
        self.set_flag(FLAG_C, result < 0);
        self.update_flags_szp(result as u8);
    }

    fn inr(&mut self, val: u8) -> u8 {
        let result = val.wrapping_add(1);
        self.update_flags_ac_add(val, 1, 0);
        self.update_flags_szp(result);
        result
    }

    fn dcr(&mut self, val: u8) -> u8 {
        let result = val.wrapping_sub(1);
        self.update_flags_ac_sub(val, 1, 0);
        self.update_flags_szp(result);
        result
    }

    /// Execute a single instruction
    fn execute(&mut self, opcode: u8) -> u32 {
        match opcode {
            // NOP
            0x00 => 4,

            // LXI B,d16 / LXI D,d16 / LXI H,d16 / LXI SP,d16
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

            // STAX B / STAX D
            0x02 => {
                self.memory.write(self.bc(), self.a);
                7
            }
            0x12 => {
                self.memory.write(self.de(), self.a);
                7
            }

            // INX B / INX D / INX H / INX SP
            0x03 => {
                self.set_bc(self.bc().wrapping_add(1));
                5
            }
            0x13 => {
                self.set_de(self.de().wrapping_add(1));
                5
            }
            0x23 => {
                self.set_hl(self.hl().wrapping_add(1));
                5
            }
            0x33 => {
                self.sp = self.sp.wrapping_add(1);
                5
            }

            // INR B / INR C / INR D / INR E / INR H / INR L / INR M / INR A
            0x04 => {
                self.b = self.inr(self.b);
                5
            }
            0x0C => {
                self.c = self.inr(self.c);
                5
            }
            0x14 => {
                self.d = self.inr(self.d);
                5
            }
            0x1C => {
                self.e = self.inr(self.e);
                5
            }
            0x24 => {
                self.h = self.inr(self.h);
                5
            }
            0x2C => {
                self.l = self.inr(self.l);
                5
            }
            0x34 => {
                let addr = self.hl();
                let val = self.memory.read(addr);
                let result = self.inr(val);
                self.memory.write(addr, result);
                10
            }
            0x3C => {
                self.a = self.inr(self.a);
                5
            }

            // DCR B / DCR C / DCR D / DCR E / DCR H / DCR L / DCR M / DCR A
            0x05 => {
                self.b = self.dcr(self.b);
                5
            }
            0x0D => {
                self.c = self.dcr(self.c);
                5
            }
            0x15 => {
                self.d = self.dcr(self.d);
                5
            }
            0x1D => {
                self.e = self.dcr(self.e);
                5
            }
            0x25 => {
                self.h = self.dcr(self.h);
                5
            }
            0x2D => {
                self.l = self.dcr(self.l);
                5
            }
            0x35 => {
                let addr = self.hl();
                let val = self.memory.read(addr);
                let result = self.dcr(val);
                self.memory.write(addr, result);
                10
            }
            0x3D => {
                self.a = self.dcr(self.a);
                5
            }

            // MVI B,d8 / MVI C,d8 / MVI D,d8 / MVI E,d8 / MVI H,d8 / MVI L,d8 / MVI M,d8 / MVI A,d8
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

            // RLC / RRC / RAL / RAR
            0x07 => {
                let carry = (self.a & 0x80) != 0;
                self.a = (self.a << 1) | if carry { 1 } else { 0 };
                self.set_flag(FLAG_C, carry);
                4
            }
            0x0F => {
                let carry = (self.a & 0x01) != 0;
                self.a = (self.a >> 1) | if carry { 0x80 } else { 0 };
                self.set_flag(FLAG_C, carry);
                4
            }
            0x17 => {
                let old_carry = if self.get_flag(FLAG_C) { 1 } else { 0 };
                let new_carry = (self.a & 0x80) != 0;
                self.a = (self.a << 1) | old_carry;
                self.set_flag(FLAG_C, new_carry);
                4
            }
            0x1F => {
                let old_carry = if self.get_flag(FLAG_C) { 0x80 } else { 0 };
                let new_carry = (self.a & 0x01) != 0;
                self.a = (self.a >> 1) | old_carry;
                self.set_flag(FLAG_C, new_carry);
                4
            }

            // DAA / CMA / STC / CMC
            0x27 => {
                let mut correction = 0u8;
                let mut carry = self.get_flag(FLAG_C);
                if self.get_flag(FLAG_AC) || (self.a & 0x0F) > 9 {
                    correction |= 0x06;
                }
                if carry || self.a > 0x99 {
                    correction |= 0x60;
                    carry = true;
                }
                self.add(correction, false);
                self.set_flag(FLAG_C, carry);
                4
            }
            0x2F => {
                self.a = !self.a;
                4
            }
            0x37 => {
                self.set_flag(FLAG_C, true);
                4
            }
            0x3F => {
                self.set_flag(FLAG_C, !self.get_flag(FLAG_C));
                4
            }

            // DAD B / DAD D / DAD H / DAD SP
            0x09 => {
                let hl = self.hl();
                let bc = self.bc();
                let result = hl.wrapping_add(bc);
                self.set_flag(FLAG_C, result < hl);
                self.set_hl(result);
                10
            }
            0x19 => {
                let hl = self.hl();
                let de = self.de();
                let result = hl.wrapping_add(de);
                self.set_flag(FLAG_C, result < hl);
                self.set_hl(result);
                10
            }
            0x29 => {
                let hl = self.hl();
                let result = hl.wrapping_add(hl);
                self.set_flag(FLAG_C, result < hl);
                self.set_hl(result);
                10
            }
            0x39 => {
                let hl = self.hl();
                let sp = self.sp;
                let result = hl.wrapping_add(sp);
                self.set_flag(FLAG_C, result < hl);
                self.set_hl(result);
                10
            }

            // LDAX B / LDAX D
            0x0A => {
                self.a = self.memory.read(self.bc());
                7
            }
            0x1A => {
                self.a = self.memory.read(self.de());
                7
            }

            // DCX B / DCX D / DCX H / DCX SP
            0x0B => {
                self.set_bc(self.bc().wrapping_sub(1));
                5
            }
            0x1B => {
                self.set_de(self.de().wrapping_sub(1));
                5
            }
            0x2B => {
                self.set_hl(self.hl().wrapping_sub(1));
                5
            }
            0x3B => {
                self.sp = self.sp.wrapping_sub(1);
                5
            }

            // SHLD / LHLD / STA / LDA
            0x22 => {
                let addr = self.read_pc_u16();
                self.memory.write(addr, self.l);
                self.memory.write(addr.wrapping_add(1), self.h);
                16
            }
            0x2A => {
                let addr = self.read_pc_u16();
                self.l = self.memory.read(addr);
                self.h = self.memory.read(addr.wrapping_add(1));
                16
            }
            0x32 => {
                let addr = self.read_pc_u16();
                self.memory.write(addr, self.a);
                13
            }
            0x3A => {
                let addr = self.read_pc_u16();
                self.a = self.memory.read(addr);
                13
            }

            // MOV instructions (0x40-0x7F except 0x76)
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
                    7
                } else {
                    5
                }
            }

            // HLT
            0x76 => {
                self.halted = true;
                7
            }

            // ADD/ADC/SUB/SBB/ANA/XRA/ORA/CMP r
            0x80..=0xBF => {
                let op = (opcode >> 3) & 0x07;
                let reg = opcode & 0x07;
                let val = match reg {
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
                    0 => self.add(val, false),
                    1 => self.add(val, true),
                    2 => self.sub(val, false),
                    3 => self.sub(val, true),
                    4 => self.ana(val),
                    5 => self.xra(val),
                    6 => self.ora(val),
                    7 => self.cmp(val),
                    _ => unreachable!(),
                }
                if reg == 6 {
                    7
                } else {
                    4
                }
            }

            // Conditional returns
            0xC0 => {
                if !self.get_flag(FLAG_Z) {
                    self.pc = self.pop_u16();
                    11
                } else {
                    5
                }
            }
            0xC8 => {
                if self.get_flag(FLAG_Z) {
                    self.pc = self.pop_u16();
                    11
                } else {
                    5
                }
            }
            0xD0 => {
                if !self.get_flag(FLAG_C) {
                    self.pc = self.pop_u16();
                    11
                } else {
                    5
                }
            }
            0xD8 => {
                if self.get_flag(FLAG_C) {
                    self.pc = self.pop_u16();
                    11
                } else {
                    5
                }
            }
            0xE0 => {
                if !self.get_flag(FLAG_P) {
                    self.pc = self.pop_u16();
                    11
                } else {
                    5
                }
            }
            0xE8 => {
                if self.get_flag(FLAG_P) {
                    self.pc = self.pop_u16();
                    11
                } else {
                    5
                }
            }
            0xF0 => {
                if !self.get_flag(FLAG_S) {
                    self.pc = self.pop_u16();
                    11
                } else {
                    5
                }
            }
            0xF8 => {
                if self.get_flag(FLAG_S) {
                    self.pc = self.pop_u16();
                    11
                } else {
                    5
                }
            }

            // POP B/D/H/PSW
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
                self.flags = (val as u8) | 0x02;
                self.a = (val >> 8) as u8;
                10
            }

            // Conditional jumps
            0xC2 => {
                let addr = self.read_pc_u16();
                if !self.get_flag(FLAG_Z) {
                    self.pc = addr;
                }
                10
            }
            0xCA => {
                let addr = self.read_pc_u16();
                if self.get_flag(FLAG_Z) {
                    self.pc = addr;
                }
                10
            }
            0xD2 => {
                let addr = self.read_pc_u16();
                if !self.get_flag(FLAG_C) {
                    self.pc = addr;
                }
                10
            }
            0xDA => {
                let addr = self.read_pc_u16();
                if self.get_flag(FLAG_C) {
                    self.pc = addr;
                }
                10
            }
            0xE2 => {
                let addr = self.read_pc_u16();
                if !self.get_flag(FLAG_P) {
                    self.pc = addr;
                }
                10
            }
            0xEA => {
                let addr = self.read_pc_u16();
                if self.get_flag(FLAG_P) {
                    self.pc = addr;
                }
                10
            }
            0xF2 => {
                let addr = self.read_pc_u16();
                if !self.get_flag(FLAG_S) {
                    self.pc = addr;
                }
                10
            }
            0xFA => {
                let addr = self.read_pc_u16();
                if self.get_flag(FLAG_S) {
                    self.pc = addr;
                }
                10
            }

            // JMP
            0xC3 => {
                self.pc = self.read_pc_u16();
                10
            }

            // OUT / IN
            0xD3 => {
                let port = self.read_pc();
                self.memory.io_write(port, self.a);
                10
            }
            0xDB => {
                let port = self.read_pc();
                self.a = self.memory.io_read(port);
                10
            }

            // XTHL
            0xE3 => {
                let l = self.l;
                let h = self.h;
                self.l = self.memory.read(self.sp);
                self.h = self.memory.read(self.sp.wrapping_add(1));
                self.memory.write(self.sp, l);
                self.memory.write(self.sp.wrapping_add(1), h);
                18
            }

            // DI / EI
            0xF3 => {
                self.inte = false;
                4
            }
            0xFB => {
                self.inte = true;
                4
            }

            // Conditional calls
            0xC4 => {
                let addr = self.read_pc_u16();
                if !self.get_flag(FLAG_Z) {
                    self.push_u16(self.pc);
                    self.pc = addr;
                    17
                } else {
                    11
                }
            }
            0xCC => {
                let addr = self.read_pc_u16();
                if self.get_flag(FLAG_Z) {
                    self.push_u16(self.pc);
                    self.pc = addr;
                    17
                } else {
                    11
                }
            }
            0xD4 => {
                let addr = self.read_pc_u16();
                if !self.get_flag(FLAG_C) {
                    self.push_u16(self.pc);
                    self.pc = addr;
                    17
                } else {
                    11
                }
            }
            0xDC => {
                let addr = self.read_pc_u16();
                if self.get_flag(FLAG_C) {
                    self.push_u16(self.pc);
                    self.pc = addr;
                    17
                } else {
                    11
                }
            }
            0xE4 => {
                let addr = self.read_pc_u16();
                if !self.get_flag(FLAG_P) {
                    self.push_u16(self.pc);
                    self.pc = addr;
                    17
                } else {
                    11
                }
            }
            0xEC => {
                let addr = self.read_pc_u16();
                if self.get_flag(FLAG_P) {
                    self.push_u16(self.pc);
                    self.pc = addr;
                    17
                } else {
                    11
                }
            }
            0xF4 => {
                let addr = self.read_pc_u16();
                if !self.get_flag(FLAG_S) {
                    self.push_u16(self.pc);
                    self.pc = addr;
                    17
                } else {
                    11
                }
            }
            0xFC => {
                let addr = self.read_pc_u16();
                if self.get_flag(FLAG_S) {
                    self.push_u16(self.pc);
                    self.pc = addr;
                    17
                } else {
                    11
                }
            }

            // PUSH B/D/H/PSW
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
                self.push_u16(((self.a as u16) << 8) | (self.flags as u16));
                11
            }

            // ADI/ACI/SUI/SBI/ANI/XRI/ORI/CPI
            0xC6 => {
                let val = self.read_pc();
                self.add(val, false);
                7
            }
            0xCE => {
                let val = self.read_pc();
                self.add(val, true);
                7
            }
            0xD6 => {
                let val = self.read_pc();
                self.sub(val, false);
                7
            }
            0xDE => {
                let val = self.read_pc();
                self.sub(val, true);
                7
            }
            0xE6 => {
                let val = self.read_pc();
                self.ana(val);
                7
            }
            0xEE => {
                let val = self.read_pc();
                self.xra(val);
                7
            }
            0xF6 => {
                let val = self.read_pc();
                self.ora(val);
                7
            }
            0xFE => {
                let val = self.read_pc();
                self.cmp(val);
                7
            }

            // RST 0-7
            0xC7 | 0xCF | 0xD7 | 0xDF | 0xE7 | 0xEF | 0xF7 | 0xFF => {
                let vector = opcode & 0x38;
                self.push_u16(self.pc);
                self.pc = vector as u16;
                11
            }

            // RET
            0xC9 => {
                self.pc = self.pop_u16();
                10
            }

            // PCHL
            0xE9 => {
                self.pc = self.hl();
                5
            }

            // SPHL
            0xF9 => {
                self.sp = self.hl();
                5
            }

            // CALL
            0xCD => {
                let addr = self.read_pc_u16();
                self.push_u16(self.pc);
                self.pc = addr;
                17
            }

            // XCHG
            0xEB => {
                let de = self.de();
                let hl = self.hl();
                self.set_de(hl);
                self.set_hl(de);
                4
            }

            _ => {
                // Unknown opcode - treat as NOP
                4
            }
        }
    }
}

impl<M: Memory8080> crate::Cpu for Cpu8080<M> {
    fn reset(&mut self) {
        self.reset();
    }

    fn step(&mut self) -> u32 {
        self.step()
    }
}
