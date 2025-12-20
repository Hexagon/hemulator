//! Sharp LR35902 CPU core (Game Boy CPU)
//!
//! This is a Z80-like CPU used in Game Boy and Game Boy Color.
//! It's similar to Z80 but with some instructions removed and others modified.

/// Memory interface trait for the LR35902 CPU
pub trait MemoryLr35902 {
    /// Read a byte from memory
    fn read(&self, addr: u16) -> u8;
    
    /// Write a byte to memory
    fn write(&mut self, addr: u16, val: u8);
}

/// Sharp LR35902 CPU state
#[derive(Debug)]
pub struct CpuLr35902<M: MemoryLr35902> {
    /// Accumulator & Flags (combined as AF)
    pub a: u8,
    pub f: u8,
    /// BC register pair
    pub b: u8,
    pub c: u8,
    /// DE register pair
    pub d: u8,
    pub e: u8,
    /// HL register pair
    pub h: u8,
    pub l: u8,
    /// Stack pointer
    pub sp: u16,
    /// Program counter
    pub pc: u16,
    /// Interrupt Master Enable flag
    pub ime: bool,
    /// Halted state
    pub halted: bool,
    /// Stopped state (for STOP instruction)
    pub stopped: bool,
    /// Total cycles executed
    pub cycles: u64,
    /// Memory interface
    pub memory: M,
}

// Flag bit positions (in F register)
const FLAG_Z: u8 = 0b10000000; // Zero
const FLAG_N: u8 = 0b01000000; // Subtract (BCD)
const FLAG_H: u8 = 0b00100000; // Half Carry (BCD)
const FLAG_C: u8 = 0b00010000; // Carry

impl<M: MemoryLr35902> CpuLr35902<M> {
    /// Create a new LR35902 CPU
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
            sp: 0,
            pc: 0,
            ime: false,
            halted: false,
            stopped: false,
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
        self.pc = 0x100; // Game Boy starts at 0x100
        self.ime = false;
        self.halted = false;
        self.stopped = false;
        self.cycles = 0;
    }

    /// Execute one instruction
    pub fn step(&mut self) -> u32 {
        if self.halted || self.stopped {
            return 4;
        }

        let opcode = self.read_pc();
        self.execute(opcode)
    }

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

    fn af(&self) -> u16 {
        ((self.a as u16) << 8) | (self.f as u16)
    }

    fn set_af(&mut self, val: u16) {
        self.a = (val >> 8) as u8;
        self.f = val as u8 & 0xF0; // Lower 4 bits always 0
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

    fn update_flags_zn(&mut self, val: u8, subtract: bool) {
        self.set_flag(FLAG_Z, val == 0);
        self.set_flag(FLAG_N, subtract);
    }

    // Arithmetic operations
    fn add(&mut self, val: u8, carry: bool) {
        let c = if carry && self.get_flag(FLAG_C) { 1 } else { 0 };
        let result = self.a as u16 + val as u16 + c as u16;
        
        self.set_flag(FLAG_H, ((self.a & 0x0F) + (val & 0x0F) + c) > 0x0F);
        self.set_flag(FLAG_C, result > 0xFF);
        self.a = result as u8;
        self.update_flags_zn(self.a, false);
    }

    fn sub(&mut self, val: u8, carry: bool) {
        let c = if carry && self.get_flag(FLAG_C) { 1 } else { 0 };
        let result = self.a as i16 - val as i16 - c as i16;
        
        self.set_flag(FLAG_H, (self.a & 0x0F) < ((val & 0x0F) + c));
        self.set_flag(FLAG_C, result < 0);
        self.a = result as u8;
        self.update_flags_zn(self.a, true);
    }

    fn and(&mut self, val: u8) {
        self.a &= val;
        self.set_flag(FLAG_H, true);
        self.set_flag(FLAG_C, false);
        self.update_flags_zn(self.a, false);
    }

    fn xor(&mut self, val: u8) {
        self.a ^= val;
        self.f = 0;
        self.update_flags_zn(self.a, false);
    }

    fn or(&mut self, val: u8) {
        self.a |= val;
        self.f = 0;
        self.update_flags_zn(self.a, false);
    }

    fn cp(&mut self, val: u8) {
        let result = self.a as i16 - val as i16;
        self.set_flag(FLAG_H, (self.a & 0x0F) < (val & 0x0F));
        self.set_flag(FLAG_C, result < 0);
        self.update_flags_zn(result as u8, true);
    }

    fn inc(&mut self, val: u8) -> u8 {
        let result = val.wrapping_add(1);
        self.set_flag(FLAG_H, (val & 0x0F) == 0x0F);
        self.update_flags_zn(result, false);
        result
    }

    fn dec(&mut self, val: u8) -> u8 {
        let result = val.wrapping_sub(1);
        self.set_flag(FLAG_H, (val & 0x0F) == 0);
        self.update_flags_zn(result, true);
        result
    }

    fn add_hl(&mut self, val: u16) {
        let hl = self.hl();
        let result = hl.wrapping_add(val);
        self.set_flag(FLAG_N, false);
        self.set_flag(FLAG_H, ((hl & 0x0FFF) + (val & 0x0FFF)) > 0x0FFF);
        self.set_flag(FLAG_C, result < hl);
        self.set_hl(result);
    }

    fn rlc(&mut self, val: u8) -> u8 {
        let carry = (val & 0x80) != 0;
        let result = (val << 1) | if carry { 1 } else { 0 };
        self.f = 0;
        self.set_flag(FLAG_C, carry);
        self.set_flag(FLAG_Z, result == 0);
        result
    }

    fn rrc(&mut self, val: u8) -> u8 {
        let carry = (val & 0x01) != 0;
        let result = (val >> 1) | if carry { 0x80 } else { 0 };
        self.f = 0;
        self.set_flag(FLAG_C, carry);
        self.set_flag(FLAG_Z, result == 0);
        result
    }

    fn rl(&mut self, val: u8) -> u8 {
        let old_carry = if self.get_flag(FLAG_C) { 1 } else { 0 };
        let new_carry = (val & 0x80) != 0;
        let result = (val << 1) | old_carry;
        self.f = 0;
        self.set_flag(FLAG_C, new_carry);
        self.set_flag(FLAG_Z, result == 0);
        result
    }

    fn rr(&mut self, val: u8) -> u8 {
        let old_carry = if self.get_flag(FLAG_C) { 0x80 } else { 0 };
        let new_carry = (val & 0x01) != 0;
        let result = (val >> 1) | old_carry;
        self.f = 0;
        self.set_flag(FLAG_C, new_carry);
        self.set_flag(FLAG_Z, result == 0);
        result
    }

    fn sla(&mut self, val: u8) -> u8 {
        let carry = (val & 0x80) != 0;
        let result = val << 1;
        self.f = 0;
        self.set_flag(FLAG_C, carry);
        self.set_flag(FLAG_Z, result == 0);
        result
    }

    fn sra(&mut self, val: u8) -> u8 {
        let carry = (val & 0x01) != 0;
        let result = (val >> 1) | (val & 0x80);
        self.f = 0;
        self.set_flag(FLAG_C, carry);
        self.set_flag(FLAG_Z, result == 0);
        result
    }

    fn swap(&mut self, val: u8) -> u8 {
        let result = ((val & 0x0F) << 4) | ((val & 0xF0) >> 4);
        self.f = 0;
        self.set_flag(FLAG_Z, result == 0);
        result
    }

    fn srl(&mut self, val: u8) -> u8 {
        let carry = (val & 0x01) != 0;
        let result = val >> 1;
        self.f = 0;
        self.set_flag(FLAG_C, carry);
        self.set_flag(FLAG_Z, result == 0);
        result
    }

    fn bit(&mut self, bit: u8, val: u8) {
        self.set_flag(FLAG_Z, (val & (1 << bit)) == 0);
        self.set_flag(FLAG_N, false);
        self.set_flag(FLAG_H, true);
    }

    fn execute(&mut self, opcode: u8) -> u32 {
        match opcode {
            // NOP
            0x00 => 4,

            // LD BC,d16 / LD DE,d16 / LD HL,d16 / LD SP,d16
            0x01 => { let val = self.read_pc_u16(); self.set_bc(val); 12 }
            0x11 => { let val = self.read_pc_u16(); self.set_de(val); 12 }
            0x21 => { let val = self.read_pc_u16(); self.set_hl(val); 12 }
            0x31 => { self.sp = self.read_pc_u16(); 12 }

            // LD (BC),A / LD (DE),A / LD (HL+),A / LD (HL-),A
            0x02 => { self.memory.write(self.bc(), self.a); 8 }
            0x12 => { self.memory.write(self.de(), self.a); 8 }
            0x22 => { let addr = self.hl(); self.memory.write(addr, self.a); self.set_hl(addr.wrapping_add(1)); 8 }
            0x32 => { let addr = self.hl(); self.memory.write(addr, self.a); self.set_hl(addr.wrapping_sub(1)); 8 }

            // INC BC / INC DE / INC HL / INC SP
            0x03 => { self.set_bc(self.bc().wrapping_add(1)); 8 }
            0x13 => { self.set_de(self.de().wrapping_add(1)); 8 }
            0x23 => { self.set_hl(self.hl().wrapping_add(1)); 8 }
            0x33 => { self.sp = self.sp.wrapping_add(1); 8 }

            // INC r
            0x04 => { self.b = self.inc(self.b); 4 }
            0x0C => { self.c = self.inc(self.c); 4 }
            0x14 => { self.d = self.inc(self.d); 4 }
            0x1C => { self.e = self.inc(self.e); 4 }
            0x24 => { self.h = self.inc(self.h); 4 }
            0x2C => { self.l = self.inc(self.l); 4 }
            0x34 => { let addr = self.hl(); let val = self.memory.read(addr); let result = self.inc(val); self.memory.write(addr, result); 12 }
            0x3C => { self.a = self.inc(self.a); 4 }

            // DEC r
            0x05 => { self.b = self.dec(self.b); 4 }
            0x0D => { self.c = self.dec(self.c); 4 }
            0x15 => { self.d = self.dec(self.d); 4 }
            0x1D => { self.e = self.dec(self.e); 4 }
            0x25 => { self.h = self.dec(self.h); 4 }
            0x2D => { self.l = self.dec(self.l); 4 }
            0x35 => { let addr = self.hl(); let val = self.memory.read(addr); let result = self.dec(val); self.memory.write(addr, result); 12 }
            0x3D => { self.a = self.dec(self.a); 4 }

            // LD r,d8
            0x06 => { self.b = self.read_pc(); 8 }
            0x0E => { self.c = self.read_pc(); 8 }
            0x16 => { self.d = self.read_pc(); 8 }
            0x1E => { self.e = self.read_pc(); 8 }
            0x26 => { self.h = self.read_pc(); 8 }
            0x2E => { self.l = self.read_pc(); 8 }
            0x36 => { let val = self.read_pc(); self.memory.write(self.hl(), val); 12 }
            0x3E => { self.a = self.read_pc(); 8 }

            // RLCA / RRCA / RLA / RRA
            0x07 => { self.a = self.rlc(self.a); self.set_flag(FLAG_Z, false); 4 }
            0x0F => { self.a = self.rrc(self.a); self.set_flag(FLAG_Z, false); 4 }
            0x17 => { self.a = self.rl(self.a); self.set_flag(FLAG_Z, false); 4 }
            0x1F => { self.a = self.rr(self.a); self.set_flag(FLAG_Z, false); 4 }

            // LD (a16),SP
            0x08 => { let addr = self.read_pc_u16(); self.memory.write(addr, self.sp as u8); self.memory.write(addr.wrapping_add(1), (self.sp >> 8) as u8); 20 }

            // ADD HL,r16
            0x09 => { self.add_hl(self.bc()); 8 }
            0x19 => { self.add_hl(self.de()); 8 }
            0x29 => { self.add_hl(self.hl()); 8 }
            0x39 => { self.add_hl(self.sp); 8 }

            // LD A,(BC) / LD A,(DE) / LD A,(HL+) / LD A,(HL-)
            0x0A => { self.a = self.memory.read(self.bc()); 8 }
            0x1A => { self.a = self.memory.read(self.de()); 8 }
            0x2A => { let addr = self.hl(); self.a = self.memory.read(addr); self.set_hl(addr.wrapping_add(1)); 8 }
            0x3A => { let addr = self.hl(); self.a = self.memory.read(addr); self.set_hl(addr.wrapping_sub(1)); 8 }

            // DEC BC / DEC DE / DEC HL / DEC SP
            0x0B => { self.set_bc(self.bc().wrapping_sub(1)); 8 }
            0x1B => { self.set_de(self.de().wrapping_sub(1)); 8 }
            0x2B => { self.set_hl(self.hl().wrapping_sub(1)); 8 }
            0x3B => { self.sp = self.sp.wrapping_sub(1); 8 }

            // JR r8 / JR cc,r8
            0x18 => { let offset = self.read_pc() as i8; self.pc = self.pc.wrapping_add(offset as u16); 12 }
            0x20 => { let offset = self.read_pc() as i8; if !self.get_flag(FLAG_Z) { self.pc = self.pc.wrapping_add(offset as u16); 12 } else { 8 } }
            0x28 => { let offset = self.read_pc() as i8; if self.get_flag(FLAG_Z) { self.pc = self.pc.wrapping_add(offset as u16); 12 } else { 8 } }
            0x30 => { let offset = self.read_pc() as i8; if !self.get_flag(FLAG_C) { self.pc = self.pc.wrapping_add(offset as u16); 12 } else { 8 } }
            0x38 => { let offset = self.read_pc() as i8; if self.get_flag(FLAG_C) { self.pc = self.pc.wrapping_add(offset as u16); 12 } else { 8 } }

            // DAA / CPL / SCF / CCF
            0x27 => {
                let mut adjust = 0u8;
                let mut carry = false;
                
                if self.get_flag(FLAG_H) || (!self.get_flag(FLAG_N) && (self.a & 0x0F) > 9) {
                    adjust |= 0x06;
                }
                
                if self.get_flag(FLAG_C) || (!self.get_flag(FLAG_N) && self.a > 0x99) {
                    adjust |= 0x60;
                    carry = true;
                }
                
                if self.get_flag(FLAG_N) {
                    self.a = self.a.wrapping_sub(adjust);
                } else {
                    self.a = self.a.wrapping_add(adjust);
                }
                
                self.set_flag(FLAG_Z, self.a == 0);
                self.set_flag(FLAG_H, false);
                self.set_flag(FLAG_C, carry);
                4
            }
            0x2F => { self.a = !self.a; self.set_flag(FLAG_N, true); self.set_flag(FLAG_H, true); 4 }
            0x37 => { self.set_flag(FLAG_N, false); self.set_flag(FLAG_H, false); self.set_flag(FLAG_C, true); 4 }
            0x3F => { self.set_flag(FLAG_N, false); self.set_flag(FLAG_H, false); self.set_flag(FLAG_C, !self.get_flag(FLAG_C)); 4 }

            // STOP / HALT
            0x10 => { self.stopped = true; self.read_pc(); 4 }
            0x76 => { self.halted = true; 4 }

            // LD r,r (0x40-0x7F except 0x76 which is HALT)
            0x40..=0x7F if opcode != 0x76 => {
                let dst = (opcode >> 3) & 0x07;
                let src = opcode & 0x07;
                
                let val = match src {
                    0 => self.b, 1 => self.c, 2 => self.d, 3 => self.e,
                    4 => self.h, 5 => self.l, 6 => self.memory.read(self.hl()), 7 => self.a,
                    _ => unreachable!(),
                };
                
                match dst {
                    0 => self.b = val, 1 => self.c = val, 2 => self.d = val, 3 => self.e = val,
                    4 => self.h = val, 5 => self.l = val, 6 => self.memory.write(self.hl(), val), 7 => self.a = val,
                    _ => unreachable!(),
                }
                
                if src == 6 || dst == 6 { 8 } else { 4 }
            }

            // ADD/ADC/SUB/SBC/AND/XOR/OR/CP r (0x80-0xBF)
            0x80..=0xBF => {
                let op = (opcode >> 3) & 0x07;
                let reg = opcode & 0x07;
                
                let val = match reg {
                    0 => self.b, 1 => self.c, 2 => self.d, 3 => self.e,
                    4 => self.h, 5 => self.l, 6 => self.memory.read(self.hl()), 7 => self.a,
                    _ => unreachable!(),
                };
                
                match op {
                    0 => self.add(val, false),
                    1 => self.add(val, true),
                    2 => self.sub(val, false),
                    3 => self.sub(val, true),
                    4 => self.and(val),
                    5 => self.xor(val),
                    6 => self.or(val),
                    7 => self.cp(val),
                    _ => unreachable!(),
                }
                
                if reg == 6 { 8 } else { 4 }
            }

            // RET cc
            0xC0 => if !self.get_flag(FLAG_Z) { self.pc = self.pop_u16(); 20 } else { 8 }
            0xC8 => if self.get_flag(FLAG_Z) { self.pc = self.pop_u16(); 20 } else { 8 }
            0xD0 => if !self.get_flag(FLAG_C) { self.pc = self.pop_u16(); 20 } else { 8 }
            0xD8 => if self.get_flag(FLAG_C) { self.pc = self.pop_u16(); 20 } else { 8 }

            // POP BC/DE/HL/AF
            0xC1 => { let val = self.pop_u16(); self.set_bc(val); 12 }
            0xD1 => { let val = self.pop_u16(); self.set_de(val); 12 }
            0xE1 => { let val = self.pop_u16(); self.set_hl(val); 12 }
            0xF1 => { let val = self.pop_u16(); self.set_af(val); 12 }

            // JP cc,a16
            0xC2 => { let addr = self.read_pc_u16(); if !self.get_flag(FLAG_Z) { self.pc = addr; 16 } else { 12 } }
            0xCA => { let addr = self.read_pc_u16(); if self.get_flag(FLAG_Z) { self.pc = addr; 16 } else { 12 } }
            0xD2 => { let addr = self.read_pc_u16(); if !self.get_flag(FLAG_C) { self.pc = addr; 16 } else { 12 } }
            0xDA => { let addr = self.read_pc_u16(); if self.get_flag(FLAG_C) { self.pc = addr; 16 } else { 12 } }

            // JP a16
            0xC3 => { self.pc = self.read_pc_u16(); 16 }

            // CALL cc,a16
            0xC4 => { let addr = self.read_pc_u16(); if !self.get_flag(FLAG_Z) { self.push_u16(self.pc); self.pc = addr; 24 } else { 12 } }
            0xCC => { let addr = self.read_pc_u16(); if self.get_flag(FLAG_Z) { self.push_u16(self.pc); self.pc = addr; 24 } else { 12 } }
            0xD4 => { let addr = self.read_pc_u16(); if !self.get_flag(FLAG_C) { self.push_u16(self.pc); self.pc = addr; 24 } else { 12 } }
            0xDC => { let addr = self.read_pc_u16(); if self.get_flag(FLAG_C) { self.push_u16(self.pc); self.pc = addr; 24 } else { 12 } }

            // PUSH BC/DE/HL/AF
            0xC5 => { self.push_u16(self.bc()); 16 }
            0xD5 => { self.push_u16(self.de()); 16 }
            0xE5 => { self.push_u16(self.hl()); 16 }
            0xF5 => { self.push_u16(self.af()); 16 }

            // ADD/ADC/SUB/SBC/AND/XOR/OR/CP d8
            0xC6 => { let val = self.read_pc(); self.add(val, false); 8 }
            0xCE => { let val = self.read_pc(); self.add(val, true); 8 }
            0xD6 => { let val = self.read_pc(); self.sub(val, false); 8 }
            0xDE => { let val = self.read_pc(); self.sub(val, true); 8 }
            0xE6 => { let val = self.read_pc(); self.and(val); 8 }
            0xEE => { let val = self.read_pc(); self.xor(val); 8 }
            0xF6 => { let val = self.read_pc(); self.or(val); 8 }
            0xFE => { let val = self.read_pc(); self.cp(val); 8 }

            // RST n
            0xC7 | 0xCF | 0xD7 | 0xDF | 0xE7 | 0xEF | 0xF7 | 0xFF => {
                let addr = (opcode & 0x38) as u16;
                self.push_u16(self.pc);
                self.pc = addr;
                16
            }

            // RET
            0xC9 => { self.pc = self.pop_u16(); 16 }

            // RETI
            0xD9 => { self.pc = self.pop_u16(); self.ime = true; 16 }

            // JP (HL)
            0xE9 => { self.pc = self.hl(); 4 }

            // LD SP,HL
            0xF9 => { self.sp = self.hl(); 8 }

            // CALL a16
            0xCD => { let addr = self.read_pc_u16(); self.push_u16(self.pc); self.pc = addr; 24 }

            // CB prefix
            0xCB => {
                let cb_op = self.read_pc();
                self.execute_cb(cb_op)
            }

            // LDH (a8),A / LDH A,(a8)
            0xE0 => { let offset = self.read_pc() as u16; self.memory.write(0xFF00 + offset, self.a); 12 }
            0xF0 => { let offset = self.read_pc() as u16; self.a = self.memory.read(0xFF00 + offset); 12 }

            // LD (C),A / LD A,(C)
            0xE2 => { self.memory.write(0xFF00 + self.c as u16, self.a); 8 }
            0xF2 => { self.a = self.memory.read(0xFF00 + self.c as u16); 8 }

            // LD (a16),A / LD A,(a16)
            0xEA => { let addr = self.read_pc_u16(); self.memory.write(addr, self.a); 16 }
            0xFA => { let addr = self.read_pc_u16(); self.a = self.memory.read(addr); 16 }

            // DI / EI
            0xF3 => { self.ime = false; 4 }
            0xFB => { self.ime = true; 4 }

            // ADD SP,r8
            0xE8 => {
                let offset = self.read_pc() as i8 as i16 as u16;
                let result = self.sp.wrapping_add(offset);
                self.set_flag(FLAG_Z, false);
                self.set_flag(FLAG_N, false);
                self.set_flag(FLAG_H, ((self.sp & 0x000F) + (offset & 0x000F)) > 0x000F);
                self.set_flag(FLAG_C, ((self.sp & 0x00FF) + (offset & 0x00FF)) > 0x00FF);
                self.sp = result;
                16
            }

            // LD HL,SP+r8
            0xF8 => {
                let offset = self.read_pc() as i8 as i16 as u16;
                let result = self.sp.wrapping_add(offset);
                self.set_flag(FLAG_Z, false);
                self.set_flag(FLAG_N, false);
                self.set_flag(FLAG_H, ((self.sp & 0x000F) + (offset & 0x000F)) > 0x000F);
                self.set_flag(FLAG_C, ((self.sp & 0x00FF) + (offset & 0x00FF)) > 0x00FF);
                self.set_hl(result);
                12
            }

            _ => {
                // Unknown opcode
                4
            }
        }
    }

    fn execute_cb(&mut self, opcode: u8) -> u32 {
        let reg = opcode & 0x07;
        let op = (opcode >> 3) & 0x07;
        let bit = (opcode >> 3) & 0x07;
        
        // Get value
        let val = match reg {
            0 => self.b, 1 => self.c, 2 => self.d, 3 => self.e,
            4 => self.h, 5 => self.l, 6 => self.memory.read(self.hl()), 7 => self.a,
            _ => unreachable!(),
        };
        
        let result = match opcode >> 6 {
            0 => { // Rotates and shifts
                match op {
                    0 => self.rlc(val),
                    1 => self.rrc(val),
                    2 => self.rl(val),
                    3 => self.rr(val),
                    4 => self.sla(val),
                    5 => self.sra(val),
                    6 => self.swap(val),
                    7 => self.srl(val),
                    _ => unreachable!(),
                }
            }
            1 => { // BIT
                self.bit(bit, val);
                return if reg == 6 { 12 } else { 8 };
            }
            2 => val & !(1 << bit), // RES
            3 => val | (1 << bit),  // SET
            _ => unreachable!(),
        };
        
        // Store result
        match reg {
            0 => self.b = result, 1 => self.c = result, 2 => self.d = result, 3 => self.e = result,
            4 => self.h = result, 5 => self.l = result, 6 => self.memory.write(self.hl(), result), 7 => self.a = result,
            _ => unreachable!(),
        }
        
        if reg == 6 { 16 } else { 8 }
    }
}

impl<M: MemoryLr35902> crate::Cpu for CpuLr35902<M> {
    fn reset(&mut self) {
        self.reset();
    }

    fn step(&mut self) -> u32 {
        self.step()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct ArrayMemory([u8; 65536]);

    impl MemoryLr35902 for ArrayMemory {
        fn read(&self, addr: u16) -> u8 {
            self.0[addr as usize]
        }

        fn write(&mut self, addr: u16, val: u8) {
            self.0[addr as usize] = val;
        }
    }

    fn make_cpu() -> CpuLr35902<ArrayMemory> {
        CpuLr35902::new(ArrayMemory([0; 65536]))
    }

    #[test]
    fn test_nop() {
        let mut cpu = make_cpu();
        cpu.pc = 0;
        cpu.memory.0[0] = 0x00; // NOP
        let cycles = cpu.step();
        assert_eq!(cycles, 4);
        assert_eq!(cpu.pc, 1);
    }

    #[test]
    fn test_ld_bc_d16() {
        let mut cpu = make_cpu();
        cpu.pc = 0;
        cpu.memory.0[0] = 0x01; // LD BC,0x1234
        cpu.memory.0[1] = 0x34;
        cpu.memory.0[2] = 0x12;
        cpu.step();
        assert_eq!(cpu.bc(), 0x1234);
    }

    #[test]
    fn test_inc_dec() {
        let mut cpu = make_cpu();
        cpu.pc = 0;
        cpu.b = 0xFF;
        cpu.memory.0[0] = 0x04; // INC B
        cpu.step();
        assert_eq!(cpu.b, 0x00);
        assert!(cpu.get_flag(FLAG_Z));

        cpu.pc = 0;
        cpu.memory.0[0] = 0x05; // DEC B
        cpu.step();
        assert_eq!(cpu.b, 0xFF);
    }

    #[test]
    fn test_ld_imm() {
        let mut cpu = make_cpu();
        cpu.pc = 0;
        cpu.memory.0[0] = 0x3E; // LD A,0x42
        cpu.memory.0[1] = 0x42;
        cpu.step();
        assert_eq!(cpu.a, 0x42);
    }

    #[test]
    fn test_add() {
        let mut cpu = make_cpu();
        cpu.a = 0x10;
        cpu.b = 0x20;
        cpu.pc = 0;
        cpu.memory.0[0] = 0x80; // ADD A,B
        cpu.step();
        assert_eq!(cpu.a, 0x30);
        assert!(!cpu.get_flag(FLAG_C));
    }

    #[test]
    fn test_add_carry() {
        let mut cpu = make_cpu();
        cpu.a = 0xFF;
        cpu.b = 0x01;
        cpu.pc = 0;
        cpu.memory.0[0] = 0x80; // ADD A,B
        cpu.step();
        assert_eq!(cpu.a, 0x00);
        assert!(cpu.get_flag(FLAG_C));
        assert!(cpu.get_flag(FLAG_Z));
    }

    #[test]
    fn test_sub() {
        let mut cpu = make_cpu();
        cpu.a = 0x30;
        cpu.b = 0x10;
        cpu.pc = 0;
        cpu.memory.0[0] = 0x90; // SUB B
        cpu.step();
        assert_eq!(cpu.a, 0x20);
    }

    #[test]
    fn test_and() {
        let mut cpu = make_cpu();
        cpu.a = 0xF0;
        cpu.b = 0x0F;
        cpu.pc = 0;
        cpu.memory.0[0] = 0xA0; // AND B
        cpu.step();
        assert_eq!(cpu.a, 0x00);
        assert!(cpu.get_flag(FLAG_Z));
    }

    #[test]
    fn test_xor() {
        let mut cpu = make_cpu();
        cpu.a = 0xFF;
        cpu.b = 0xFF;
        cpu.pc = 0;
        cpu.memory.0[0] = 0xA8; // XOR B
        cpu.step();
        assert_eq!(cpu.a, 0x00);
        assert!(cpu.get_flag(FLAG_Z));
    }

    #[test]
    fn test_or() {
        let mut cpu = make_cpu();
        cpu.a = 0xF0;
        cpu.b = 0x0F;
        cpu.pc = 0;
        cpu.memory.0[0] = 0xB0; // OR B
        cpu.step();
        assert_eq!(cpu.a, 0xFF);
    }

    #[test]
    fn test_cp() {
        let mut cpu = make_cpu();
        cpu.a = 0x10;
        cpu.b = 0x10;
        cpu.pc = 0;
        cpu.memory.0[0] = 0xB8; // CP B
        cpu.step();
        assert!(cpu.get_flag(FLAG_Z));
    }

    #[test]
    fn test_jp() {
        let mut cpu = make_cpu();
        cpu.pc = 0;
        cpu.memory.0[0] = 0xC3; // JP 0x1234
        cpu.memory.0[1] = 0x34;
        cpu.memory.0[2] = 0x12;
        cpu.step();
        assert_eq!(cpu.pc, 0x1234);
    }

    #[test]
    fn test_jr() {
        let mut cpu = make_cpu();
        cpu.pc = 0;
        cpu.memory.0[0] = 0x18; // JR 0x10
        cpu.memory.0[1] = 0x10;
        cpu.step();
        assert_eq!(cpu.pc, 0x12); // 2 (after reading opcode+offset) + 0x10
    }

    #[test]
    fn test_call_ret() {
        let mut cpu = make_cpu();
        cpu.sp = 0x8000;
        cpu.pc = 0x100;
        cpu.memory.0[0x100] = 0xCD; // CALL 0x200
        cpu.memory.0[0x101] = 0x00;
        cpu.memory.0[0x102] = 0x02;
        cpu.step();
        assert_eq!(cpu.pc, 0x200);
        assert_eq!(cpu.sp, 0x7FFE);

        cpu.memory.0[0x200] = 0xC9; // RET
        cpu.step();
        assert_eq!(cpu.pc, 0x103);
        assert_eq!(cpu.sp, 0x8000);
    }

    #[test]
    fn test_push_pop() {
        let mut cpu = make_cpu();
        cpu.sp = 0x8000;
        cpu.set_bc(0x1234);
        cpu.pc = 0;
        cpu.memory.0[0] = 0xC5; // PUSH BC
        cpu.step();
        assert_eq!(cpu.sp, 0x7FFE);

        cpu.set_bc(0x0000);
        cpu.memory.0[1] = 0xC1; // POP BC
        cpu.step();
        assert_eq!(cpu.bc(), 0x1234);
        assert_eq!(cpu.sp, 0x8000);
    }

    #[test]
    fn test_inc_dec_16bit() {
        let mut cpu = make_cpu();
        cpu.set_bc(0xFFFF);
        cpu.pc = 0;
        cpu.memory.0[0] = 0x03; // INC BC
        cpu.step();
        assert_eq!(cpu.bc(), 0x0000);

        cpu.memory.0[1] = 0x0B; // DEC BC
        cpu.step();
        assert_eq!(cpu.bc(), 0xFFFF);
    }

    #[test]
    fn test_add_hl() {
        let mut cpu = make_cpu();
        cpu.set_hl(0x1000);
        cpu.set_bc(0x0100);
        cpu.pc = 0;
        cpu.memory.0[0] = 0x09; // ADD HL,BC
        cpu.step();
        assert_eq!(cpu.hl(), 0x1100);
    }

    #[test]
    fn test_rlc() {
        let mut cpu = make_cpu();
        cpu.a = 0b10000001;
        cpu.pc = 0;
        cpu.memory.0[0] = 0x07; // RLCA
        cpu.step();
        assert_eq!(cpu.a, 0b00000011);
        assert!(cpu.get_flag(FLAG_C));
    }

    #[test]
    fn test_rrc() {
        let mut cpu = make_cpu();
        cpu.a = 0b10000001;
        cpu.pc = 0;
        cpu.memory.0[0] = 0x0F; // RRCA
        cpu.step();
        assert_eq!(cpu.a, 0b11000000);
        assert!(cpu.get_flag(FLAG_C));
    }

    #[test]
    fn test_halt() {
        let mut cpu = make_cpu();
        cpu.pc = 0;
        cpu.memory.0[0] = 0x76; // HALT
        cpu.step();
        assert!(cpu.halted);
    }

    #[test]
    fn test_di_ei() {
        let mut cpu = make_cpu();
        cpu.pc = 0;
        cpu.memory.0[0] = 0xFB; // EI
        cpu.step();
        assert!(cpu.ime);

        cpu.memory.0[1] = 0xF3; // DI
        cpu.step();
        assert!(!cpu.ime);
    }

    #[test]
    fn test_ld_indirect() {
        let mut cpu = make_cpu();
        cpu.a = 0x42;
        cpu.set_bc(0x1234);
        cpu.pc = 0;
        cpu.memory.0[0] = 0x02; // LD (BC),A
        cpu.step();
        assert_eq!(cpu.memory.0[0x1234], 0x42);

        cpu.a = 0x00;
        cpu.memory.0[1] = 0x0A; // LD A,(BC)
        cpu.step();
        assert_eq!(cpu.a, 0x42);
    }

    #[test]
    fn test_ld_hl_inc() {
        let mut cpu = make_cpu();
        cpu.a = 0x42;
        cpu.set_hl(0x1000);
        cpu.pc = 0;
        cpu.memory.0[0] = 0x22; // LD (HL+),A
        cpu.step();
        assert_eq!(cpu.memory.0[0x1000], 0x42);
        assert_eq!(cpu.hl(), 0x1001);
    }

    #[test]
    fn test_ld_hl_dec() {
        let mut cpu = make_cpu();
        cpu.a = 0x42;
        cpu.set_hl(0x1000);
        cpu.pc = 0;
        cpu.memory.0[0] = 0x32; // LD (HL-),A
        cpu.step();
        assert_eq!(cpu.memory.0[0x1000], 0x42);
        assert_eq!(cpu.hl(), 0x0FFF);
    }

    #[test]
    fn test_cb_rlc() {
        let mut cpu = make_cpu();
        cpu.b = 0b10000001;
        cpu.pc = 0;
        cpu.memory.0[0] = 0xCB; // CB prefix
        cpu.memory.0[1] = 0x00; // RLC B
        cpu.step();
        assert_eq!(cpu.b, 0b00000011);
        assert!(cpu.get_flag(FLAG_C));
    }

    #[test]
    fn test_cb_bit() {
        let mut cpu = make_cpu();
        cpu.b = 0b10000000;
        cpu.pc = 0;
        cpu.memory.0[0] = 0xCB; // CB prefix
        cpu.memory.0[1] = 0x47; // BIT 0,A (bit 0 of A)
        cpu.a = 0b00000001;
        cpu.step();
        assert!(!cpu.get_flag(FLAG_Z)); // Bit 0 is set

        cpu.pc = 0;
        cpu.a = 0b00000000;
        cpu.step();
        assert!(cpu.get_flag(FLAG_Z)); // Bit 0 is not set
    }

    #[test]
    fn test_cb_res() {
        let mut cpu = make_cpu();
        cpu.b = 0xFF;
        cpu.pc = 0;
        cpu.memory.0[0] = 0xCB; // CB prefix
        cpu.memory.0[1] = 0x80; // RES 0,B
        cpu.step();
        assert_eq!(cpu.b, 0xFE);
    }

    #[test]
    fn test_cb_set() {
        let mut cpu = make_cpu();
        cpu.b = 0x00;
        cpu.pc = 0;
        cpu.memory.0[0] = 0xCB; // CB prefix
        cpu.memory.0[1] = 0xC0; // SET 0,B
        cpu.step();
        assert_eq!(cpu.b, 0x01);
    }

    #[test]
    fn test_ldh() {
        let mut cpu = make_cpu();
        cpu.a = 0x42;
        cpu.pc = 0;
        cpu.memory.0[0] = 0xE0; // LDH (0xFF00+a8),A
        cpu.memory.0[1] = 0x50;
        cpu.step();
        assert_eq!(cpu.memory.0[0xFF50], 0x42);

        cpu.a = 0x00;
        cpu.memory.0[2] = 0xF0; // LDH A,(0xFF00+a8)
        cpu.memory.0[3] = 0x50;
        cpu.step();
        assert_eq!(cpu.a, 0x42);
    }

    #[test]
    fn test_conditional_jr() {
        let mut cpu = make_cpu();
        cpu.set_flag(FLAG_Z, false);
        cpu.pc = 0;
        cpu.memory.0[0] = 0x20; // JR NZ,r8
        cpu.memory.0[1] = 0x10;
        cpu.step();
        assert_eq!(cpu.pc, 0x12); // Jump taken

        cpu.set_flag(FLAG_Z, true);
        cpu.pc = 0;
        cpu.step();
        assert_eq!(cpu.pc, 2); // Jump not taken
    }

    #[test]
    fn test_rst() {
        let mut cpu = make_cpu();
        cpu.sp = 0x8000;
        cpu.pc = 0x100;
        cpu.memory.0[0x100] = 0xC7; // RST 0x00
        cpu.step();
        assert_eq!(cpu.pc, 0x00);
        assert_eq!(cpu.sp, 0x7FFE);
    }

    #[test]
    fn test_swap() {
        let mut cpu = make_cpu();
        cpu.b = 0x12;
        cpu.pc = 0;
        cpu.memory.0[0] = 0xCB; // CB prefix
        cpu.memory.0[1] = 0x30; // SWAP B
        cpu.step();
        assert_eq!(cpu.b, 0x21);
    }

    #[test]
    fn test_add_sp_r8() {
        let mut cpu = make_cpu();
        cpu.sp = 0x1000;
        cpu.pc = 0;
        cpu.memory.0[0] = 0xE8; // ADD SP,r8
        cpu.memory.0[1] = 0x10;
        cpu.step();
        assert_eq!(cpu.sp, 0x1010);
    }

    #[test]
    fn test_ld_hl_sp_r8() {
        let mut cpu = make_cpu();
        cpu.sp = 0x1000;
        cpu.pc = 0;
        cpu.memory.0[0] = 0xF8; // LD HL,SP+r8
        cpu.memory.0[1] = 0x10;
        cpu.step();
        assert_eq!(cpu.hl(), 0x1010);
    }
}
