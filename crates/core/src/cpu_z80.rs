//! Zilog Z80 CPU core implementation
//!
//! The Z80 extends the 8080 with additional registers and instructions.
//! This module provides a reusable Z80 implementation.
//!
//! For detailed CPU reference documentation, see: `docs/references/cpu_z80.md`

// Z80 Flag bits
const FLAG_S: u8 = 0b10000000; // Sign
const FLAG_Z: u8 = 0b01000000; // Zero
const FLAG_H: u8 = 0b00010000; // Half Carry
const FLAG_P: u8 = 0b00000100; // Parity/Overflow
const FLAG_N: u8 = 0b00000010; // Subtract (BCD)
const FLAG_C: u8 = 0b00000001; // Carry

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

    /// Trigger a maskable interrupt (honors IFF1 and interrupt mode)
    ///
    /// # Parameters
    /// - `data`: Data byte from interrupting device (used in IM 0 and IM 2)
    ///
    /// # Interrupt Modes
    /// - **IM 0**: Device provides instruction (usually RST), executes `data` as opcode
    /// - **IM 1**: Jump to $0038 (most common, used by SMS/Game Gear)
    /// - **IM 2**: Vectored interrupt, forms address from I register and `data`
    pub fn interrupt(&mut self, data: u8) {
        if !self.iff1 {
            return; // Interrupts disabled
        }

        // Exit halt state
        self.halted = false;

        // Disable interrupts
        self.iff1 = false;
        self.iff2 = false;

        // Push current PC to stack
        self.push_u16(self.pc);

        // Handle based on interrupt mode
        match self.im {
            0 => {
                // IM 0: Execute the provided instruction (usually RST)
                // Note: Full IM 0 requires executing arbitrary instructions from the data byte.
                // For simplicity, we assume RST instructions (most common case) and jump to
                // the RST vector. This works for typical hardware like the SMS.
                self.pc = (data & 0x38) as u16;
            }
            1 => {
                // IM 1: Jump to $0038
                self.pc = 0x0038;
            }
            2 => {
                // IM 2: Vectored interrupt
                // Vector address = (I << 8) | (data & 0xFE)
                let vector_addr = ((self.i as u16) << 8) | ((data & 0xFE) as u16);
                let lo = self.memory.read(vector_addr) as u16;
                let hi = self.memory.read(vector_addr.wrapping_add(1)) as u16;
                self.pc = (hi << 8) | lo;
            }
            _ => {
                // Default to IM 1 behavior for invalid modes
                self.pc = 0x0038;
            }
        }
    }

    /// Trigger a non-maskable interrupt (NMI)
    ///
    /// NMI cannot be disabled and always jumps to $0066.
    /// IFF1 is copied to IFF2 so it can be restored by RETN.
    pub fn nmi(&mut self) {
        // Exit halt state
        self.halted = false;

        // Save IFF1 to IFF2 (for RETN to restore)
        self.iff2 = self.iff1;

        // Disable maskable interrupts
        self.iff1 = false;

        // Push current PC to stack
        self.push_u16(self.pc);

        // Jump to NMI vector
        self.pc = 0x0066;
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
        let opcode = self.read_pc();
        let reg = opcode & 0x07;
        let bit = (opcode >> 3) & 0x07;

        // RLC, RRC, RL, RR, SLA, SRA, SLL, SRL (0x00-0x3F)
        if opcode < 0x40 {
            let op = (opcode >> 3) & 0x07;
            let mut val = self.get_reg8(reg);

            val = match op {
                0 => self.rlc(val), // RLC
                1 => self.rrc(val), // RRC
                2 => self.rl(val),  // RL
                3 => self.rr(val),  // RR
                4 => self.sla(val), // SLA
                5 => self.sra(val), // SRA
                6 => self.sll(val), // SLL (undocumented)
                7 => self.srl(val), // SRL
                _ => val,
            };

            self.set_reg8(reg, val);
            if reg == 6 {
                15
            } else {
                8
            } // (HL) takes longer
        }
        // BIT b,r (0x40-0x7F)
        else if opcode < 0x80 {
            let val = self.get_reg8(reg);
            let bit_val = (val >> bit) & 1;

            self.set_flag(FLAG_Z, bit_val == 0);
            self.set_flag(FLAG_N, false);
            self.set_flag(FLAG_H, true);

            if reg == 6 {
                12
            } else {
                8
            } // (HL) takes longer
        }
        // RES b,r (0x80-0xBF)
        else if opcode < 0xC0 {
            let val = self.get_reg8(reg);
            let result = val & !(1 << bit);
            self.set_reg8(reg, result);

            if reg == 6 {
                15
            } else {
                8
            } // (HL) takes longer
        }
        // SET b,r (0xC0-0xFF)
        else {
            let val = self.get_reg8(reg);
            let result = val | (1 << bit);
            self.set_reg8(reg, result);

            if reg == 6 {
                15
            } else {
                8
            } // (HL) takes longer
        }
    }

    // Helper for CB rotate/shift operations
    fn rlc(&mut self, val: u8) -> u8 {
        let carry = (val & 0x80) != 0;
        let result = (val << 1) | if carry { 1 } else { 0 };
        self.set_flag(FLAG_C, carry);
        self.set_flag(FLAG_N, false);
        self.set_flag(FLAG_H, false);
        self.set_sz_flags(result);
        result
    }

    fn rrc(&mut self, val: u8) -> u8 {
        let carry = (val & 0x01) != 0;
        let result = (val >> 1) | if carry { 0x80 } else { 0 };
        self.set_flag(FLAG_C, carry);
        self.set_flag(FLAG_N, false);
        self.set_flag(FLAG_H, false);
        self.set_sz_flags(result);
        result
    }

    fn rl(&mut self, val: u8) -> u8 {
        let old_carry = if self.get_flag(FLAG_C) { 1 } else { 0 };
        let carry = (val & 0x80) != 0;
        let result = (val << 1) | old_carry;
        self.set_flag(FLAG_C, carry);
        self.set_flag(FLAG_N, false);
        self.set_flag(FLAG_H, false);
        self.set_sz_flags(result);
        result
    }

    fn rr(&mut self, val: u8) -> u8 {
        let old_carry = if self.get_flag(FLAG_C) { 0x80 } else { 0 };
        let carry = (val & 0x01) != 0;
        let result = (val >> 1) | old_carry;
        self.set_flag(FLAG_C, carry);
        self.set_flag(FLAG_N, false);
        self.set_flag(FLAG_H, false);
        self.set_sz_flags(result);
        result
    }

    fn sla(&mut self, val: u8) -> u8 {
        let carry = (val & 0x80) != 0;
        let result = val << 1;
        self.set_flag(FLAG_C, carry);
        self.set_flag(FLAG_N, false);
        self.set_flag(FLAG_H, false);
        self.set_sz_flags(result);
        result
    }

    fn sra(&mut self, val: u8) -> u8 {
        let carry = (val & 0x01) != 0;
        let result = (val >> 1) | (val & 0x80); // Keep sign bit
        self.set_flag(FLAG_C, carry);
        self.set_flag(FLAG_N, false);
        self.set_flag(FLAG_H, false);
        self.set_sz_flags(result);
        result
    }

    fn sll(&mut self, val: u8) -> u8 {
        let carry = (val & 0x80) != 0;
        let result = (val << 1) | 1; // Undocumented, shifts in 1
        self.set_flag(FLAG_C, carry);
        self.set_flag(FLAG_N, false);
        self.set_flag(FLAG_H, false);
        self.set_sz_flags(result);
        result
    }

    fn srl(&mut self, val: u8) -> u8 {
        let carry = (val & 0x01) != 0;
        let result = val >> 1;
        self.set_flag(FLAG_C, carry);
        self.set_flag(FLAG_N, false);
        self.set_flag(FLAG_H, false);
        self.set_sz_flags(result);
        result
    }

    // Get register for CB instructions (0-7: B,C,D,E,H,L,(HL),A)
    fn get_reg8(&self, reg: u8) -> u8 {
        match reg {
            0 => self.b,
            1 => self.c,
            2 => self.d,
            3 => self.e,
            4 => self.h,
            5 => self.l,
            6 => self.memory.read(self.hl()),
            7 => self.a,
            _ => 0,
        }
    }

    fn set_reg8(&mut self, reg: u8, val: u8) {
        match reg {
            0 => self.b = val,
            1 => self.c = val,
            2 => self.d = val,
            3 => self.e = val,
            4 => self.h = val,
            5 => self.l = val,
            6 => self.memory.write(self.hl(), val),
            7 => self.a = val,
            _ => {}
        }
    }

    fn set_sz_flags(&mut self, val: u8) {
        self.set_flag(FLAG_S, (val & 0x80) != 0);
        self.set_flag(FLAG_Z, val == 0);
        self.set_flag(FLAG_P, self.parity(val));
    }

    // Calculate parity (even parity = true)
    fn parity(&self, val: u8) -> bool {
        val.count_ones().is_multiple_of(2)
    }

    // ED prefix instructions (extended instructions)
    fn execute_ed(&mut self) -> u32 {
        let opcode = self.read_pc();
        match opcode {
            // IN r,(C) - 0x40, 0x48, 0x50, 0x58, 0x60, 0x68, 0x78
            0x40 => {
                self.b = self.memory.io_read(self.c);
                self.set_sz_flags(self.b);
                self.set_flag(FLAG_N, false);
                self.set_flag(FLAG_H, false);
                12
            }
            0x48 => {
                self.c = self.memory.io_read(self.c);
                self.set_sz_flags(self.c);
                self.set_flag(FLAG_N, false);
                self.set_flag(FLAG_H, false);
                12
            }
            0x50 => {
                self.d = self.memory.io_read(self.c);
                self.set_sz_flags(self.d);
                self.set_flag(FLAG_N, false);
                self.set_flag(FLAG_H, false);
                12
            }
            0x58 => {
                self.e = self.memory.io_read(self.c);
                self.set_sz_flags(self.e);
                self.set_flag(FLAG_N, false);
                self.set_flag(FLAG_H, false);
                12
            }
            0x60 => {
                self.h = self.memory.io_read(self.c);
                self.set_sz_flags(self.h);
                self.set_flag(FLAG_N, false);
                self.set_flag(FLAG_H, false);
                12
            }
            0x68 => {
                self.l = self.memory.io_read(self.c);
                self.set_sz_flags(self.l);
                self.set_flag(FLAG_N, false);
                self.set_flag(FLAG_H, false);
                12
            }
            0x78 => {
                self.a = self.memory.io_read(self.c);
                self.set_sz_flags(self.a);
                self.set_flag(FLAG_N, false);
                self.set_flag(FLAG_H, false);
                12
            }

            // OUT (C),r - 0x41, 0x49, 0x51, 0x59, 0x61, 0x69, 0x79
            0x41 => {
                self.memory.io_write(self.c, self.b);
                12
            }
            0x49 => {
                self.memory.io_write(self.c, self.c);
                12
            }
            0x51 => {
                self.memory.io_write(self.c, self.d);
                12
            }
            0x59 => {
                self.memory.io_write(self.c, self.e);
                12
            }
            0x61 => {
                self.memory.io_write(self.c, self.h);
                12
            }
            0x69 => {
                self.memory.io_write(self.c, self.l);
                12
            }
            0x79 => {
                self.memory.io_write(self.c, self.a);
                12
            }

            // SBC HL,rr
            0x42 => {
                self.sbc_hl(self.bc());
                15
            }
            0x52 => {
                self.sbc_hl(self.de());
                15
            }
            0x62 => {
                self.sbc_hl(self.hl());
                15
            }
            0x72 => {
                self.sbc_hl(self.sp);
                15
            }

            // ADC HL,rr
            0x4A => {
                self.adc_hl(self.bc());
                15
            }
            0x5A => {
                self.adc_hl(self.de());
                15
            }
            0x6A => {
                self.adc_hl(self.hl());
                15
            }
            0x7A => {
                self.adc_hl(self.sp);
                15
            }

            // LD (nn),rr
            0x43 => {
                let addr = self.read_pc_u16();
                let val = self.bc();
                self.memory.write(addr, (val & 0xFF) as u8);
                self.memory.write(addr.wrapping_add(1), (val >> 8) as u8);
                20
            }
            0x53 => {
                let addr = self.read_pc_u16();
                let val = self.de();
                self.memory.write(addr, (val & 0xFF) as u8);
                self.memory.write(addr.wrapping_add(1), (val >> 8) as u8);
                20
            }
            0x63 => {
                let addr = self.read_pc_u16();
                let val = self.hl();
                self.memory.write(addr, (val & 0xFF) as u8);
                self.memory.write(addr.wrapping_add(1), (val >> 8) as u8);
                20
            }
            0x73 => {
                let addr = self.read_pc_u16();
                self.memory.write(addr, (self.sp & 0xFF) as u8);
                self.memory
                    .write(addr.wrapping_add(1), (self.sp >> 8) as u8);
                20
            }

            // LD rr,(nn)
            0x4B => {
                let addr = self.read_pc_u16();
                let low = self.memory.read(addr) as u16;
                let high = self.memory.read(addr.wrapping_add(1)) as u16;
                self.set_bc((high << 8) | low);
                20
            }
            0x5B => {
                let addr = self.read_pc_u16();
                let low = self.memory.read(addr) as u16;
                let high = self.memory.read(addr.wrapping_add(1)) as u16;
                self.set_de((high << 8) | low);
                20
            }
            0x6B => {
                let addr = self.read_pc_u16();
                let low = self.memory.read(addr) as u16;
                let high = self.memory.read(addr.wrapping_add(1)) as u16;
                self.set_hl((high << 8) | low);
                20
            }
            0x7B => {
                let addr = self.read_pc_u16();
                let low = self.memory.read(addr) as u16;
                let high = self.memory.read(addr.wrapping_add(1)) as u16;
                self.sp = (high << 8) | low;
                20
            }

            // NEG
            0x44 => {
                let a = self.a;
                self.a = 0;
                self.sub_a(a, false);
                8
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

            // RETI (Return from interrupt)
            0x4D => {
                self.pc = self.pop_u16();
                self.iff1 = self.iff2;
                14
            }

            // IM 1
            0x56 => {
                self.im = 1;
                8
            }

            // LD A,I
            0x57 => {
                self.a = self.i;
                9
            }

            // IM 2
            0x5E => {
                self.im = 2;
                8
            }

            // LD A,R
            0x5F => {
                self.a = self.r;
                9
            }

            // RRD
            0x67 => {
                let hl = self.hl();
                let val = self.memory.read(hl);
                let low_a = self.a & 0x0F;
                self.a = (self.a & 0xF0) | (val & 0x0F);
                self.memory.write(hl, (val >> 4) | (low_a << 4));
                self.set_sz_flags(self.a);
                self.set_flag(FLAG_N, false);
                self.set_flag(FLAG_H, false);
                18
            }

            // RLD
            0x6F => {
                let hl = self.hl();
                let val = self.memory.read(hl);
                let low_a = self.a & 0x0F;
                self.a = (self.a & 0xF0) | (val >> 4);
                self.memory.write(hl, (val << 4) | low_a);
                self.set_sz_flags(self.a);
                self.set_flag(FLAG_N, false);
                self.set_flag(FLAG_H, false);
                18
            }

            // LDI - Load and increment
            0xA0 => {
                let hl = self.hl();
                let de = self.de();
                let val = self.memory.read(hl);
                self.memory.write(de, val);
                self.set_hl(hl.wrapping_add(1));
                self.set_de(de.wrapping_add(1));
                let bc = self.bc().wrapping_sub(1);
                self.set_bc(bc);
                self.set_flag(FLAG_P, bc != 0);
                self.set_flag(FLAG_N, false);
                self.set_flag(FLAG_H, false);
                16
            }

            // CPI - Compare and increment
            0xA1 => {
                let hl = self.hl();
                let val = self.memory.read(hl);
                self.cp_a(val);
                self.set_hl(hl.wrapping_add(1));
                let bc = self.bc().wrapping_sub(1);
                self.set_bc(bc);
                self.set_flag(FLAG_P, bc != 0);
                16
            }

            // INI - Input and increment
            0xA2 => {
                let hl = self.hl();
                let val = self.memory.io_read(self.c);
                self.memory.write(hl, val);
                self.set_hl(hl.wrapping_add(1));
                self.b = self.b.wrapping_sub(1);
                self.set_flag(FLAG_Z, self.b == 0);
                self.set_flag(FLAG_N, true);
                16
            }

            // OUTI - Output and increment
            0xA3 => {
                let hl = self.hl();
                let val = self.memory.read(hl);
                self.memory.io_write(self.c, val);
                self.set_hl(hl.wrapping_add(1));
                self.b = self.b.wrapping_sub(1);
                self.set_flag(FLAG_Z, self.b == 0);
                self.set_flag(FLAG_N, true);
                16
            }

            // LDD - Load and decrement
            0xA8 => {
                let hl = self.hl();
                let de = self.de();
                let val = self.memory.read(hl);
                self.memory.write(de, val);
                self.set_hl(hl.wrapping_sub(1));
                self.set_de(de.wrapping_sub(1));
                let bc = self.bc().wrapping_sub(1);
                self.set_bc(bc);
                self.set_flag(FLAG_P, bc != 0);
                self.set_flag(FLAG_N, false);
                self.set_flag(FLAG_H, false);
                16
            }

            // CPD - Compare and decrement
            0xA9 => {
                let hl = self.hl();
                let val = self.memory.read(hl);
                self.cp_a(val);
                self.set_hl(hl.wrapping_sub(1));
                let bc = self.bc().wrapping_sub(1);
                self.set_bc(bc);
                self.set_flag(FLAG_P, bc != 0);
                16
            }

            // IND - Input and decrement
            0xAA => {
                let hl = self.hl();
                let val = self.memory.io_read(self.c);
                self.memory.write(hl, val);
                self.set_hl(hl.wrapping_sub(1));
                self.b = self.b.wrapping_sub(1);
                self.set_flag(FLAG_Z, self.b == 0);
                self.set_flag(FLAG_N, true);
                16
            }

            // OUTD - Output and decrement
            0xAB => {
                let hl = self.hl();
                let val = self.memory.read(hl);
                self.memory.io_write(self.c, val);
                self.set_hl(hl.wrapping_sub(1));
                self.b = self.b.wrapping_sub(1);
                self.set_flag(FLAG_Z, self.b == 0);
                self.set_flag(FLAG_N, true);
                16
            }

            // LDIR - Load, increment, repeat
            0xB0 => {
                let hl = self.hl();
                let de = self.de();
                let val = self.memory.read(hl);
                self.memory.write(de, val);
                self.set_hl(hl.wrapping_add(1));
                self.set_de(de.wrapping_add(1));
                let bc = self.bc().wrapping_sub(1);
                self.set_bc(bc);
                self.set_flag(FLAG_P, false);
                self.set_flag(FLAG_N, false);
                self.set_flag(FLAG_H, false);
                if bc != 0 {
                    self.pc = self.pc.wrapping_sub(2);
                    21
                } else {
                    16
                }
            }

            // CPIR - Compare, increment, repeat
            0xB1 => {
                let hl = self.hl();
                let val = self.memory.read(hl);
                self.cp_a(val);
                self.set_hl(hl.wrapping_add(1));
                let bc = self.bc().wrapping_sub(1);
                self.set_bc(bc);
                let z = self.get_flag(FLAG_Z);
                if bc != 0 && !z {
                    self.pc = self.pc.wrapping_sub(2);
                    21
                } else {
                    self.set_flag(FLAG_P, bc != 0);
                    16
                }
            }

            // INIR - Input, increment, repeat
            0xB2 => {
                let hl = self.hl();
                let val = self.memory.io_read(self.c);
                self.memory.write(hl, val);
                self.set_hl(hl.wrapping_add(1));
                self.b = self.b.wrapping_sub(1);
                self.set_flag(FLAG_N, true);
                if self.b != 0 {
                    self.pc = self.pc.wrapping_sub(2);
                    21
                } else {
                    self.set_flag(FLAG_Z, true);
                    16
                }
            }

            // OTIR - Output, increment, repeat
            0xB3 => {
                let hl = self.hl();
                let val = self.memory.read(hl);
                self.memory.io_write(self.c, val);
                self.set_hl(hl.wrapping_add(1));
                self.b = self.b.wrapping_sub(1);
                self.set_flag(FLAG_N, true);
                if self.b != 0 {
                    self.pc = self.pc.wrapping_sub(2);
                    21
                } else {
                    self.set_flag(FLAG_Z, true);
                    16
                }
            }

            // LDDR - Load, decrement, repeat
            0xB8 => {
                let hl = self.hl();
                let de = self.de();
                let val = self.memory.read(hl);
                self.memory.write(de, val);
                self.set_hl(hl.wrapping_sub(1));
                self.set_de(de.wrapping_sub(1));
                let bc = self.bc().wrapping_sub(1);
                self.set_bc(bc);
                self.set_flag(FLAG_P, false);
                self.set_flag(FLAG_N, false);
                self.set_flag(FLAG_H, false);
                if bc != 0 {
                    self.pc = self.pc.wrapping_sub(2);
                    21
                } else {
                    16
                }
            }

            // CPDR - Compare, decrement, repeat
            0xB9 => {
                let hl = self.hl();
                let val = self.memory.read(hl);
                self.cp_a(val);
                self.set_hl(hl.wrapping_sub(1));
                let bc = self.bc().wrapping_sub(1);
                self.set_bc(bc);
                let z = self.get_flag(FLAG_Z);
                if bc != 0 && !z {
                    self.pc = self.pc.wrapping_sub(2);
                    21
                } else {
                    self.set_flag(FLAG_P, bc != 0);
                    16
                }
            }

            // INDR - Input, decrement, repeat
            0xBA => {
                let hl = self.hl();
                let val = self.memory.io_read(self.c);
                self.memory.write(hl, val);
                self.set_hl(hl.wrapping_sub(1));
                self.b = self.b.wrapping_sub(1);
                self.set_flag(FLAG_N, true);
                if self.b != 0 {
                    self.pc = self.pc.wrapping_sub(2);
                    21
                } else {
                    self.set_flag(FLAG_Z, true);
                    16
                }
            }

            // OTDR - Output, decrement, repeat
            0xBB => {
                let hl = self.hl();
                let val = self.memory.read(hl);
                self.memory.io_write(self.c, val);
                self.set_hl(hl.wrapping_sub(1));
                self.b = self.b.wrapping_sub(1);
                self.set_flag(FLAG_N, true);
                if self.b != 0 {
                    self.pc = self.pc.wrapping_sub(2);
                    21
                } else {
                    self.set_flag(FLAG_Z, true);
                    16
                }
            }

            _ => 8,
        }
    }

    // Helper for SBC HL,rr
    fn sbc_hl(&mut self, val: u16) {
        let hl = self.hl();
        let carry = if self.get_flag(FLAG_C) { 1 } else { 0 };
        let result = hl.wrapping_sub(val).wrapping_sub(carry);

        self.set_flag(FLAG_S, (result & 0x8000) != 0);
        self.set_flag(FLAG_Z, result == 0);
        self.set_flag(FLAG_H, (hl & 0x0FFF) < (val & 0x0FFF) + carry);
        self.set_flag(FLAG_P, ((hl ^ val) & (hl ^ result) & 0x8000) != 0);
        self.set_flag(FLAG_N, true);
        self.set_flag(FLAG_C, hl < val.wrapping_add(carry));

        self.set_hl(result);
    }

    // Helper for ADC HL,rr
    fn adc_hl(&mut self, val: u16) {
        let hl = self.hl();
        let carry = if self.get_flag(FLAG_C) { 1 } else { 0 };
        let result = hl.wrapping_add(val).wrapping_add(carry);

        self.set_flag(FLAG_S, (result & 0x8000) != 0);
        self.set_flag(FLAG_Z, result == 0);
        self.set_flag(FLAG_H, ((hl & 0x0FFF) + (val & 0x0FFF) + carry) > 0x0FFF);
        self.set_flag(
            FLAG_P,
            (!((hl ^ val) & 0x8000) != 0) && (((hl ^ result) & 0x8000) != 0),
        );
        self.set_flag(FLAG_N, false);
        self.set_flag(
            FLAG_C,
            ((hl as u32) + (val as u32) + (carry as u32)) > 0xFFFF,
        );

        self.set_hl(result);
    }

    // DD prefix instructions (IX operations)
    fn execute_dd(&mut self) -> u32 {
        let opcode = self.read_pc();
        match opcode {
            // LD IX,nn
            0x21 => {
                self.ix = self.read_pc_u16();
                14
            }
            // LD (nn),IX
            0x22 => {
                let addr = self.read_pc_u16();
                self.memory.write(addr, (self.ix & 0xFF) as u8);
                self.memory
                    .write(addr.wrapping_add(1), (self.ix >> 8) as u8);
                20
            }
            // INC IX
            0x23 => {
                self.ix = self.ix.wrapping_add(1);
                10
            }
            // LD IX,(nn)
            0x2A => {
                let addr = self.read_pc_u16();
                let low = self.memory.read(addr) as u16;
                let high = self.memory.read(addr.wrapping_add(1)) as u16;
                self.ix = (high << 8) | low;
                20
            }
            // DEC IX
            0x2B => {
                self.ix = self.ix.wrapping_sub(1);
                10
            }
            // INC (IX+d)
            0x34 => {
                let offset = self.read_pc() as i8;
                let addr = self.ix.wrapping_add(offset as u16);
                let val = self.memory.read(addr);
                let result = self.inc(val);
                self.memory.write(addr, result);
                23
            }
            // DEC (IX+d)
            0x35 => {
                let offset = self.read_pc() as i8;
                let addr = self.ix.wrapping_add(offset as u16);
                let val = self.memory.read(addr);
                let result = self.dec(val);
                self.memory.write(addr, result);
                23
            }
            // LD (IX+d),n
            0x36 => {
                let offset = self.read_pc() as i8;
                let val = self.read_pc();
                let addr = self.ix.wrapping_add(offset as u16);
                self.memory.write(addr, val);
                19
            }
            // ADD IX,BC / ADD IX,DE / ADD IX,IX / ADD IX,SP
            0x09 => {
                self.ix = self.add16(self.ix, self.bc());
                15
            }
            0x19 => {
                self.ix = self.add16(self.ix, self.de());
                15
            }
            0x29 => {
                self.ix = self.add16(self.ix, self.ix);
                15
            }
            0x39 => {
                self.ix = self.add16(self.ix, self.sp);
                15
            }
            // LD r,(IX+d) - Load register from (IX+offset)
            0x46 | 0x4E | 0x56 | 0x5E | 0x66 | 0x6E | 0x7E => {
                let offset = self.read_pc() as i8;
                let addr = self.ix.wrapping_add(offset as u16);
                let val = self.memory.read(addr);
                let reg = (opcode >> 3) & 0x07;
                match reg {
                    0 => self.b = val,
                    1 => self.c = val,
                    2 => self.d = val,
                    3 => self.e = val,
                    4 => self.h = val,
                    5 => self.l = val,
                    7 => self.a = val,
                    _ => {}
                }
                19
            }
            // LD (IX+d),r - Store register to (IX+offset)
            0x70 | 0x71 | 0x72 | 0x73 | 0x74 | 0x75 | 0x77 => {
                let offset = self.read_pc() as i8;
                let addr = self.ix.wrapping_add(offset as u16);
                let reg = opcode & 0x07;
                let val = match reg {
                    0 => self.b,
                    1 => self.c,
                    2 => self.d,
                    3 => self.e,
                    4 => self.h,
                    5 => self.l,
                    7 => self.a,
                    _ => 0,
                };
                self.memory.write(addr, val);
                19
            }
            // ADD A,(IX+d) / ADC A,(IX+d) / SUB (IX+d) / SBC A,(IX+d)
            // AND (IX+d) / XOR (IX+d) / OR (IX+d) / CP (IX+d)
            0x86 | 0x8E | 0x96 | 0x9E | 0xA6 | 0xAE | 0xB6 | 0xBE => {
                let offset = self.read_pc() as i8;
                let addr = self.ix.wrapping_add(offset as u16);
                let val = self.memory.read(addr);
                match opcode & 0x38 {
                    0x00 => self.add_a(val, false), // ADD
                    0x08 => self.add_a(val, true),  // ADC
                    0x10 => self.sub_a(val, false), // SUB
                    0x18 => self.sub_a(val, true),  // SBC
                    0x20 => self.and_a(val),        // AND
                    0x28 => self.xor_a(val),        // XOR
                    0x30 => self.or_a(val),         // OR
                    0x38 => self.cp_a(val),         // CP
                    _ => {}
                }
                19
            }
            // POP IX
            0xE1 => {
                self.ix = self.pop_u16();
                14
            }
            // EX (SP),IX
            0xE3 => {
                let sp_val = self.pop_u16();
                self.push_u16(self.ix);
                self.ix = sp_val;
                23
            }
            // PUSH IX
            0xE5 => {
                self.push_u16(self.ix);
                15
            }
            // JP (IX)
            0xE9 => {
                self.pc = self.ix;
                8
            }
            // LD SP,IX
            0xF9 => {
                self.sp = self.ix;
                10
            }
            // CB prefix with IX offset
            0xCB => {
                let offset = self.read_pc() as i8;
                let cb_opcode = self.read_pc();
                self.execute_ddcb(offset, cb_opcode)
            }
            _ => 8,
        }
    }

    // FD prefix instructions (IY operations)
    fn execute_fd(&mut self) -> u32 {
        let opcode = self.read_pc();
        match opcode {
            // LD IY,nn
            0x21 => {
                self.iy = self.read_pc_u16();
                14
            }
            // LD (nn),IY
            0x22 => {
                let addr = self.read_pc_u16();
                self.memory.write(addr, (self.iy & 0xFF) as u8);
                self.memory
                    .write(addr.wrapping_add(1), (self.iy >> 8) as u8);
                20
            }
            // INC IY
            0x23 => {
                self.iy = self.iy.wrapping_add(1);
                10
            }
            // LD IY,(nn)
            0x2A => {
                let addr = self.read_pc_u16();
                let low = self.memory.read(addr) as u16;
                let high = self.memory.read(addr.wrapping_add(1)) as u16;
                self.iy = (high << 8) | low;
                20
            }
            // DEC IY
            0x2B => {
                self.iy = self.iy.wrapping_sub(1);
                10
            }
            // INC (IY+d)
            0x34 => {
                let offset = self.read_pc() as i8;
                let addr = self.iy.wrapping_add(offset as u16);
                let val = self.memory.read(addr);
                let result = self.inc(val);
                self.memory.write(addr, result);
                23
            }
            // DEC (IY+d)
            0x35 => {
                let offset = self.read_pc() as i8;
                let addr = self.iy.wrapping_add(offset as u16);
                let val = self.memory.read(addr);
                let result = self.dec(val);
                self.memory.write(addr, result);
                23
            }
            // LD (IY+d),n
            0x36 => {
                let offset = self.read_pc() as i8;
                let val = self.read_pc();
                let addr = self.iy.wrapping_add(offset as u16);
                self.memory.write(addr, val);
                19
            }
            // ADD IY,BC / ADD IY,DE / ADD IY,IY / ADD IY,SP
            0x09 => {
                self.iy = self.add16(self.iy, self.bc());
                15
            }
            0x19 => {
                self.iy = self.add16(self.iy, self.de());
                15
            }
            0x29 => {
                self.iy = self.add16(self.iy, self.iy);
                15
            }
            0x39 => {
                self.iy = self.add16(self.iy, self.sp);
                15
            }
            // LD r,(IY+d) - Load register from (IY+offset)
            0x46 | 0x4E | 0x56 | 0x5E | 0x66 | 0x6E | 0x7E => {
                let offset = self.read_pc() as i8;
                let addr = self.iy.wrapping_add(offset as u16);
                let val = self.memory.read(addr);
                let reg = (opcode >> 3) & 0x07;
                match reg {
                    0 => self.b = val,
                    1 => self.c = val,
                    2 => self.d = val,
                    3 => self.e = val,
                    4 => self.h = val,
                    5 => self.l = val,
                    7 => self.a = val,
                    _ => {}
                }
                19
            }
            // LD (IY+d),r - Store register to (IY+offset)
            0x70 | 0x71 | 0x72 | 0x73 | 0x74 | 0x75 | 0x77 => {
                let offset = self.read_pc() as i8;
                let addr = self.iy.wrapping_add(offset as u16);
                let reg = opcode & 0x07;
                let val = match reg {
                    0 => self.b,
                    1 => self.c,
                    2 => self.d,
                    3 => self.e,
                    4 => self.h,
                    5 => self.l,
                    7 => self.a,
                    _ => 0,
                };
                self.memory.write(addr, val);
                19
            }
            // ADD A,(IY+d) / ADC A,(IY+d) / SUB (IY+d) / SBC A,(IY+d)
            // AND (IY+d) / XOR (IY+d) / OR (IY+d) / CP (IY+d)
            0x86 | 0x8E | 0x96 | 0x9E | 0xA6 | 0xAE | 0xB6 | 0xBE => {
                let offset = self.read_pc() as i8;
                let addr = self.iy.wrapping_add(offset as u16);
                let val = self.memory.read(addr);
                match opcode & 0x38 {
                    0x00 => self.add_a(val, false), // ADD
                    0x08 => self.add_a(val, true),  // ADC
                    0x10 => self.sub_a(val, false), // SUB
                    0x18 => self.sub_a(val, true),  // SBC
                    0x20 => self.and_a(val),        // AND
                    0x28 => self.xor_a(val),        // XOR
                    0x30 => self.or_a(val),         // OR
                    0x38 => self.cp_a(val),         // CP
                    _ => {}
                }
                19
            }
            // POP IY
            0xE1 => {
                self.iy = self.pop_u16();
                14
            }
            // EX (SP),IY
            0xE3 => {
                let sp_val = self.pop_u16();
                self.push_u16(self.iy);
                self.iy = sp_val;
                23
            }
            // PUSH IY
            0xE5 => {
                self.push_u16(self.iy);
                15
            }
            // JP (IY)
            0xE9 => {
                self.pc = self.iy;
                8
            }
            // LD SP,IY
            0xF9 => {
                self.sp = self.iy;
                10
            }
            // CB prefix with IY offset
            0xCB => {
                let offset = self.read_pc() as i8;
                let cb_opcode = self.read_pc();
                self.execute_fdcb(offset, cb_opcode)
            }
            _ => 8,
        }
    }

    // DDCB prefix (IX+offset bit operations)
    fn execute_ddcb(&mut self, offset: i8, opcode: u8) -> u32 {
        let addr = self.ix.wrapping_add(offset as u16);
        let mut val = self.memory.read(addr);
        let bit = (opcode >> 3) & 0x07;

        // Rotate/shift operations (0x00-0x3F)
        if opcode < 0x40 {
            let op = (opcode >> 3) & 0x07;
            val = match op {
                0 => self.rlc(val),
                1 => self.rrc(val),
                2 => self.rl(val),
                3 => self.rr(val),
                4 => self.sla(val),
                5 => self.sra(val),
                6 => self.sll(val),
                7 => self.srl(val),
                _ => val,
            };
            self.memory.write(addr, val);
        }
        // BIT b,(IX+d) (0x40-0x7F)
        else if opcode < 0x80 {
            let bit_val = (val >> bit) & 1;
            self.set_flag(FLAG_Z, bit_val == 0);
            self.set_flag(FLAG_N, false);
            self.set_flag(FLAG_H, true);
            return 20;
        }
        // RES b,(IX+d) (0x80-0xBF)
        else if opcode < 0xC0 {
            val &= !(1 << bit);
            self.memory.write(addr, val);
        }
        // SET b,(IX+d) (0xC0-0xFF)
        else {
            val |= 1 << bit;
            self.memory.write(addr, val);
        }
        23
    }

    // FDCB prefix (IY+offset bit operations)
    fn execute_fdcb(&mut self, offset: i8, opcode: u8) -> u32 {
        let addr = self.iy.wrapping_add(offset as u16);
        let mut val = self.memory.read(addr);
        let bit = (opcode >> 3) & 0x07;

        // Rotate/shift operations (0x00-0x3F)
        if opcode < 0x40 {
            let op = (opcode >> 3) & 0x07;
            val = match op {
                0 => self.rlc(val),
                1 => self.rrc(val),
                2 => self.rl(val),
                3 => self.rr(val),
                4 => self.sla(val),
                5 => self.sra(val),
                6 => self.sll(val),
                7 => self.srl(val),
                _ => val,
            };
            self.memory.write(addr, val);
        }
        // BIT b,(IY+d) (0x40-0x7F)
        else if opcode < 0x80 {
            let bit_val = (val >> bit) & 1;
            self.set_flag(FLAG_Z, bit_val == 0);
            self.set_flag(FLAG_N, false);
            self.set_flag(FLAG_H, true);
            return 20;
        }
        // RES b,(IY+d) (0x80-0xBF)
        else if opcode < 0xC0 {
            val &= !(1 << bit);
            self.memory.write(addr, val);
        }
        // SET b,(IY+d) (0xC0-0xFF)
        else {
            val |= 1 << bit;
            self.memory.write(addr, val);
        }
        23
    }

    // Helper for 16-bit addition with carry flag
    fn add16(&mut self, a: u16, b: u16) -> u16 {
        let result = a.wrapping_add(b);
        self.set_flag(FLAG_N, false);
        self.set_flag(FLAG_H, ((a & 0x0FFF) + (b & 0x0FFF)) > 0x0FFF);
        self.set_flag(FLAG_C, (a as u32 + b as u32) > 0xFFFF);
        result
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

#[cfg(test)]
mod tests {
    use super::*;

    // Simple test memory implementation
    struct TestMemory {
        ram: [u8; 0x10000],
    }

    impl TestMemory {
        fn new() -> Self {
            Self { ram: [0; 0x10000] }
        }

        fn with_program(program: &[u8]) -> Self {
            let mut mem = Self::new();
            mem.ram[..program.len()].copy_from_slice(program);
            mem
        }
    }

    impl MemoryZ80 for TestMemory {
        fn read(&self, addr: u16) -> u8 {
            self.ram[addr as usize]
        }

        fn write(&mut self, addr: u16, val: u8) {
            self.ram[addr as usize] = val;
        }
    }

    #[test]
    fn test_interrupt_im0() {
        let memory = TestMemory::new();
        let mut cpu = CpuZ80::new(memory);
        cpu.reset();
        cpu.pc = 0x1234;
        cpu.sp = 0xFFFE;
        cpu.im = 0;
        cpu.iff1 = true; // Enable interrupts

        // Trigger interrupt with RST 0x10 (vector 0x10)
        cpu.interrupt(0x10);

        assert_eq!(cpu.pc, 0x0010); // Should jump to RST vector
        assert!(!cpu.iff1); // Interrupts should be disabled
        assert!(!cpu.iff2);
        assert_eq!(cpu.sp, 0xFFFC); // Stack should have PC pushed
    }

    #[test]
    fn test_interrupt_im1() {
        let memory = TestMemory::new();
        let mut cpu = CpuZ80::new(memory);
        cpu.reset();
        cpu.pc = 0x1234;
        cpu.sp = 0xFFFE;
        cpu.im = 1;
        cpu.iff1 = true; // Enable interrupts

        // Trigger interrupt (data byte doesn't matter in IM 1)
        cpu.interrupt(0xFF);

        assert_eq!(cpu.pc, 0x0038); // Should jump to $0038
        assert!(!cpu.iff1); // Interrupts should be disabled
        assert!(!cpu.iff2);
        assert_eq!(cpu.sp, 0xFFFC); // Stack should have PC pushed
    }

    #[test]
    fn test_interrupt_im2() {
        let mut memory = TestMemory::new();
        // Set up interrupt vector table at $8000
        memory.ram[0x80FE] = 0x34; // Low byte of handler address
        memory.ram[0x80FF] = 0x12; // High byte of handler address

        let mut cpu = CpuZ80::new(memory);
        cpu.reset();
        cpu.pc = 0x5678;
        cpu.sp = 0xFFFE;
        cpu.im = 2;
        cpu.i = 0x80; // Interrupt vector high byte
        cpu.iff1 = true; // Enable interrupts

        // Trigger interrupt with device byte 0xFF (uses $80FE as vector table address)
        cpu.interrupt(0xFF);

        assert_eq!(cpu.pc, 0x1234); // Should jump to handler at $1234
        assert!(!cpu.iff1); // Interrupts should be disabled
        assert!(!cpu.iff2);
        assert_eq!(cpu.sp, 0xFFFC); // Stack should have PC pushed
    }

    #[test]
    fn test_interrupt_disabled() {
        let memory = TestMemory::new();
        let mut cpu = CpuZ80::new(memory);
        cpu.reset();
        cpu.pc = 0x1234;
        cpu.sp = 0xFFFE;
        cpu.im = 1;
        cpu.iff1 = false; // Interrupts disabled

        // Try to trigger interrupt
        cpu.interrupt(0xFF);

        // PC should not change
        assert_eq!(cpu.pc, 0x1234);
        assert_eq!(cpu.sp, 0xFFFE); // Stack should not change
    }

    #[test]
    fn test_interrupt_exits_halt() {
        let memory = TestMemory::new();
        let mut cpu = CpuZ80::new(memory);
        cpu.reset();
        cpu.pc = 0x1234;
        cpu.sp = 0xFFFE;
        cpu.im = 1;
        cpu.iff1 = true;
        cpu.halted = true; // CPU is halted

        // Trigger interrupt
        cpu.interrupt(0xFF);

        assert!(!cpu.halted); // Should exit halt state
        assert_eq!(cpu.pc, 0x0038); // Should jump to interrupt vector
    }

    #[test]
    fn test_nmi() {
        let memory = TestMemory::new();
        let mut cpu = CpuZ80::new(memory);
        cpu.reset();
        cpu.pc = 0x1234;
        cpu.sp = 0xFFFE;
        cpu.iff1 = true; // Interrupts enabled

        // Trigger NMI
        cpu.nmi();

        assert_eq!(cpu.pc, 0x0066); // Should jump to $0066
        assert!(!cpu.iff1); // IFF1 should be disabled
        assert!(cpu.iff2); // IFF2 should save previous IFF1 state (true)
        assert_eq!(cpu.sp, 0xFFFC); // Stack should have PC pushed
    }

    #[test]
    fn test_nmi_when_interrupts_disabled() {
        let memory = TestMemory::new();
        let mut cpu = CpuZ80::new(memory);
        cpu.reset();
        cpu.pc = 0x1234;
        cpu.sp = 0xFFFE;
        cpu.iff1 = false; // Interrupts disabled

        // Trigger NMI (should still work)
        cpu.nmi();

        assert_eq!(cpu.pc, 0x0066); // Should jump to $0066
        assert!(!cpu.iff1); // IFF1 should be disabled
        assert!(!cpu.iff2); // IFF2 should save previous IFF1 state (false)
        assert_eq!(cpu.sp, 0xFFFC); // Stack should have PC pushed
    }

    #[test]
    fn test_nmi_exits_halt() {
        let memory = TestMemory::new();
        let mut cpu = CpuZ80::new(memory);
        cpu.reset();
        cpu.pc = 0x1234;
        cpu.sp = 0xFFFE;
        cpu.halted = true; // CPU is halted

        // Trigger NMI
        cpu.nmi();

        assert!(!cpu.halted); // Should exit halt state
        assert_eq!(cpu.pc, 0x0066); // Should jump to NMI vector
    }

    #[test]
    fn test_ei_instruction() {
        let program = [0xFB]; // EI instruction
        let memory = TestMemory::with_program(&program);
        let mut cpu = CpuZ80::new(memory);
        cpu.reset();

        cpu.step();

        assert!(cpu.iff1); // Interrupts should be enabled
        assert!(cpu.iff2);
    }

    #[test]
    fn test_di_instruction() {
        let program = [0xF3]; // DI instruction
        let memory = TestMemory::with_program(&program);
        let mut cpu = CpuZ80::new(memory);
        cpu.reset();
        cpu.iff1 = true;
        cpu.iff2 = true;

        cpu.step();

        assert!(!cpu.iff1); // Interrupts should be disabled
        assert!(!cpu.iff2);
    }
}
