//! Motorola 68000 CPU emulation
//!
//! Implements the 68000 instruction set with all addressing modes.
//! 24-bit address bus, 16-bit data bus, 8 data registers, 8 address registers.

use emu_core::logging::{log, LogCategory, LogLevel};

/// Memory interface for the 68000
pub trait Memory68k {
    fn read_byte(&self, addr: u32) -> u8;
    fn read_word(&self, addr: u32) -> u16;
    fn write_byte(&mut self, addr: u32, val: u8);
    fn write_word(&mut self, addr: u32, val: u16);

    fn read_long(&self, addr: u32) -> u32 {
        let hi = self.read_word(addr) as u32;
        let lo = self.read_word(addr.wrapping_add(2)) as u32;
        (hi << 16) | lo
    }

    fn write_long(&mut self, addr: u32, val: u32) {
        self.write_word(addr, (val >> 16) as u16);
        self.write_word(addr.wrapping_add(2), val as u16);
    }
}

/// Status register flags
const SR_CARRY: u16 = 0x0001;
const SR_OVERFLOW: u16 = 0x0002;
const SR_ZERO: u16 = 0x0004;
const SR_NEGATIVE: u16 = 0x0008;
const SR_EXTEND: u16 = 0x0010;

const SR_SUPERVISOR: u16 = 0x2000;

/// Exception vector numbers
const VEC_RESET_SSP: u32 = 0x000;
const VEC_RESET_PC: u32 = 0x004;
const VEC_BUS_ERROR: u32 = 0x008;
const VEC_ADDRESS_ERROR: u32 = 0x00C;
const VEC_ILLEGAL_INSN: u32 = 0x010;
const VEC_ZERO_DIVIDE: u32 = 0x014;
const VEC_CHK: u32 = 0x018;
const VEC_TRAPV: u32 = 0x01C;
const VEC_PRIVILEGE: u32 = 0x020;
const VEC_TRACE: u32 = 0x024;
const VEC_LINE_A: u32 = 0x028;
const VEC_LINE_F: u32 = 0x02C;
const VEC_SPURIOUS_INT: u32 = 0x060;
const VEC_AUTO_INT_BASE: u32 = 0x064;
const VEC_TRAP_BASE: u32 = 0x080;

/// Motorola 68000 CPU
pub struct M68k<M: Memory68k> {
    /// Data registers D0-D7
    pub d: [u32; 8],
    /// Address registers A0-A7 (A7 is the active stack pointer)
    pub a: [u32; 8],
    /// Program counter (24-bit)
    pub pc: u32,
    /// Status register (CCR + supervisor byte)
    pub sr: u16,
    /// User Stack Pointer (saved when in supervisor mode)
    pub usp: u32,
    /// Supervisor Stack Pointer (saved when in user mode)
    pub ssp: u32,
    /// Prefetch queue (current instruction word)
    prefetch: u16,
    /// Whether the CPU is halted (by STOP or double fault)
    pub halted: bool,
    /// Whether the CPU is stopped (STOP instruction)
    pub stopped: bool,
    /// Pending interrupt level (0 = none, 1-7 = level)
    pending_interrupt: u8,
    /// Total cycles executed
    pub cycles: u64,
    /// Memory bus
    pub memory: M,
}

impl<M: Memory68k> M68k<M> {
    pub fn new(memory: M) -> Self {
        Self {
            d: [0; 8],
            a: [0; 8],
            pc: 0,
            sr: SR_SUPERVISOR | 0x0700, // Start in supervisor mode, interrupts masked
            usp: 0,
            ssp: 0,
            prefetch: 0,
            halted: false,
            stopped: false,
            pending_interrupt: 0,
            cycles: 0,
            memory,
        }
    }

    /// Reset the CPU — reads SSP from vector 0 and PC from vector 1
    pub fn reset(&mut self) {
        self.sr = SR_SUPERVISOR | 0x0700;
        self.ssp = self.memory.read_long(VEC_RESET_SSP);
        self.a[7] = self.ssp;
        self.pc = self.memory.read_long(VEC_RESET_PC);
        self.prefetch = self.memory.read_word(self.pc);
        self.halted = false;
        self.stopped = false;
        self.pending_interrupt = 0;
        self.cycles = 0;

        log(LogCategory::CPU, LogLevel::Info, || {
            format!("M68K RESET: PC=${:06X} SSP=${:06X}", self.pc, self.a[7])
        });
    }

    /// Request an interrupt at the given level (1-7)
    /// Higher levels take priority — only accept if higher than current pending
    pub fn interrupt(&mut self, level: u8) {
        if level > 0 && level <= 7 && level > self.pending_interrupt {
            self.pending_interrupt = level;
        }
    }

    /// Execute one instruction, return cycles consumed
    pub fn step(&mut self) -> u32 {
        if self.halted {
            return 4;
        }

        // Check for pending interrupts
        if self.pending_interrupt > 0 {
            let mask = ((self.sr >> 8) & 7) as u8;
            if self.pending_interrupt > mask || self.pending_interrupt == 7 {
                let level = self.pending_interrupt;
                self.pending_interrupt = 0;
                self.stopped = false;
                return self.do_interrupt(level);
            }
        }

        if self.stopped {
            return 4;
        }

        let opcode = self.prefetch;
        self.pc = self.pc.wrapping_add(2) & 0x00FF_FFFF;

        let cycles = self.execute(opcode);

        // Fetch next instruction word
        self.prefetch = self.memory.read_word(self.pc);
        self.cycles += cycles as u64;
        cycles
    }

    /// Disassemble instruction at given address, return (mnemonic, length)
    pub fn disassemble(&self, addr: u32) -> (String, u32) {
        let opcode = self.memory.read_word(addr);
        disassemble_opcode(&self.memory, addr, opcode)
    }

    // ── Helpers ─────────────────────────────────────────────────

    fn addr_mask(val: u32) -> u32 {
        val & 0x00FF_FFFF
    }

    fn is_supervisor(&self) -> bool {
        self.sr & SR_SUPERVISOR != 0
    }

    fn get_ccr(&self) -> u8 {
        (self.sr & 0x1F) as u8
    }

    fn flag_c(&self) -> bool {
        self.sr & SR_CARRY != 0
    }
    fn flag_v(&self) -> bool {
        self.sr & SR_OVERFLOW != 0
    }
    fn flag_z(&self) -> bool {
        self.sr & SR_ZERO != 0
    }
    fn flag_n(&self) -> bool {
        self.sr & SR_NEGATIVE != 0
    }
    fn flag_x(&self) -> bool {
        self.sr & SR_EXTEND != 0
    }

    fn set_flag(&mut self, mask: u16, val: bool) {
        if val {
            self.sr |= mask;
        } else {
            self.sr &= !mask;
        }
    }

    /// Set N, Z flags for a byte result
    fn set_nz_byte(&mut self, val: u8) {
        self.set_flag(SR_NEGATIVE, val & 0x80 != 0);
        self.set_flag(SR_ZERO, val == 0);
    }

    /// Set N, Z flags for a word result
    fn set_nz_word(&mut self, val: u16) {
        self.set_flag(SR_NEGATIVE, val & 0x8000 != 0);
        self.set_flag(SR_ZERO, val == 0);
    }

    /// Set N, Z flags for a long result
    fn set_nz_long(&mut self, val: u32) {
        self.set_flag(SR_NEGATIVE, val & 0x8000_0000 != 0);
        self.set_flag(SR_ZERO, val == 0);
    }

    /// Read the next extension word and advance PC
    fn fetch_word(&mut self) -> u16 {
        let w = self.memory.read_word(self.pc);
        self.pc = self.pc.wrapping_add(2) & 0x00FF_FFFF;
        w
    }

    fn fetch_long(&mut self) -> u32 {
        let hi = self.fetch_word() as u32;
        let lo = self.fetch_word() as u32;
        (hi << 16) | lo
    }

    /// Push a long onto the stack
    fn push_long(&mut self, val: u32) {
        self.a[7] = self.a[7].wrapping_sub(4) & 0x00FF_FFFF;
        self.memory.write_long(self.a[7], val);
    }

    /// Pop a long from the stack
    fn pop_long(&mut self) -> u32 {
        let val = self.memory.read_long(self.a[7]);
        self.a[7] = self.a[7].wrapping_add(4) & 0x00FF_FFFF;
        val
    }

    /// Push a word onto the stack
    fn push_word(&mut self, val: u16) {
        self.a[7] = self.a[7].wrapping_sub(2) & 0x00FF_FFFF;
        self.memory.write_word(self.a[7], val);
    }

    /// Pop a word from the stack
    fn pop_word(&mut self) -> u16 {
        let val = self.memory.read_word(self.a[7]);
        self.a[7] = self.a[7].wrapping_add(2) & 0x00FF_FFFF;
        val
    }

    /// Process an interrupt
    fn do_interrupt(&mut self, level: u8) -> u32 {
        let old_sr = self.sr;

        // Enter supervisor mode
        if !self.is_supervisor() {
            self.usp = self.a[7];
            self.a[7] = self.ssp;
            self.sr |= SR_SUPERVISOR;
        }

        // Set interrupt mask to current level
        self.sr = (self.sr & !0x0700) | ((level as u16) << 8);
        // Clear trace flag
        self.sr &= !0x8000;

        // Push PC and SR
        self.push_long(self.pc);
        self.push_word(old_sr);

        // Read vector
        let vector_addr = VEC_AUTO_INT_BASE + ((level as u32 - 1) * 4);
        let new_pc = self.memory.read_long(vector_addr);

        if new_pc == 0 {
            // Spurious interrupt
            self.pc = self.memory.read_long(VEC_SPURIOUS_INT);
        } else {
            self.pc = new_pc & 0x00FF_FFFF;
        }
        self.prefetch = self.memory.read_word(self.pc);

        44 // Interrupt processing takes ~44 cycles
    }

    /// Take an exception
    fn exception(&mut self, vector_addr: u32) -> u32 {
        let old_sr = self.sr;

        if !self.is_supervisor() {
            self.usp = self.a[7];
            self.a[7] = self.ssp;
            self.sr |= SR_SUPERVISOR;
        }
        self.sr &= !0x8000; // Clear trace

        self.push_long(self.pc);
        self.push_word(old_sr);

        self.pc = self.memory.read_long(vector_addr) & 0x00FF_FFFF;
        self.prefetch = self.memory.read_word(self.pc);
        34
    }

    /// Set SR, handling supervisor mode transitions
    fn set_sr(&mut self, new_sr: u16) {
        let was_super = self.is_supervisor();
        self.sr = new_sr;
        let is_super = self.is_supervisor();

        if was_super && !is_super {
            // Leaving supervisor mode
            self.ssp = self.a[7];
            self.a[7] = self.usp;
        } else if !was_super && is_super {
            // Entering supervisor mode
            self.usp = self.a[7];
            self.a[7] = self.ssp;
        }
    }

    // ── Effective Address Calculation ─────────────────────────────

    /// Calculate address for an effective address mode (returns address)
    /// mode = bits 5-3, reg = bits 2-0 of the EA field
    fn ea_address(&mut self, mode: u8, reg: u8, size: Size) -> u32 {
        match mode {
            2 => {
                // (An)
                self.a[reg as usize]
            }
            3 => {
                // (An)+
                let addr = self.a[reg as usize];
                let inc = if reg == 7 && size == Size::Byte {
                    2
                } else {
                    size.bytes()
                };
                self.a[reg as usize] = self.a[reg as usize].wrapping_add(inc) & 0x00FF_FFFF;
                addr
            }
            4 => {
                // -(An)
                let dec = if reg == 7 && size == Size::Byte {
                    2
                } else {
                    size.bytes()
                };
                self.a[reg as usize] = self.a[reg as usize].wrapping_sub(dec) & 0x00FF_FFFF;
                self.a[reg as usize]
            }
            5 => {
                // d16(An)
                let disp = self.fetch_word() as i16 as i32;
                (self.a[reg as usize] as i32).wrapping_add(disp) as u32 & 0x00FF_FFFF
            }
            6 => {
                // d8(An,Xn)
                let ext = self.fetch_word();
                self.calc_index(self.a[reg as usize], ext)
            }
            7 => {
                match reg {
                    0 => {
                        // (xxx).W
                        let addr = self.fetch_word() as i16 as i32 as u32;
                        addr & 0x00FF_FFFF
                    }
                    1 => {
                        // (xxx).L
                        let addr = self.fetch_long();
                        addr & 0x00FF_FFFF
                    }
                    2 => {
                        // d16(PC)
                        let base = self.pc;
                        let disp = self.fetch_word() as i16 as i32;
                        (base as i32).wrapping_add(disp) as u32 & 0x00FF_FFFF
                    }
                    3 => {
                        // d8(PC,Xn)
                        let base = self.pc;
                        let ext = self.fetch_word();
                        self.calc_index(base, ext)
                    }
                    _ => {
                        log(LogCategory::CPU, LogLevel::Error, || {
                            format!("M68K: Invalid EA mode 7, reg {}", reg)
                        });
                        0
                    }
                }
            }
            _ => {
                log(LogCategory::CPU, LogLevel::Error, || {
                    format!("M68K: ea_address called with register-direct mode {}", mode)
                });
                0
            }
        }
    }

    /// Calculate index extension word address
    fn calc_index(&self, base: u32, ext: u16) -> u32 {
        let disp = (ext & 0xFF) as i8 as i32;
        let xn_reg = ((ext >> 12) & 7) as usize;
        let xn_val = if ext & 0x8000 != 0 {
            self.a[xn_reg]
        } else {
            self.d[xn_reg]
        };
        let xn_val = if ext & 0x0800 != 0 {
            xn_val as i32 // .L
        } else {
            xn_val as i16 as i32 // .W
        };
        (base as i32).wrapping_add(disp).wrapping_add(xn_val) as u32 & 0x00FF_FFFF
    }

    /// Read from an effective address
    fn read_ea(&mut self, mode: u8, reg: u8, size: Size) -> u32 {
        match mode {
            0 => {
                // Dn
                match size {
                    Size::Byte => self.d[reg as usize] & 0xFF,
                    Size::Word => self.d[reg as usize] & 0xFFFF,
                    Size::Long => self.d[reg as usize],
                }
            }
            1 => {
                // An
                match size {
                    Size::Byte => self.a[reg as usize] & 0xFF,
                    Size::Word => self.a[reg as usize] & 0xFFFF,
                    Size::Long => self.a[reg as usize],
                }
            }
            7 if reg == 4 => {
                // #imm
                match size {
                    Size::Byte => self.fetch_word() as u32 & 0xFF,
                    Size::Word => self.fetch_word() as u32,
                    Size::Long => self.fetch_long(),
                }
            }
            _ => {
                let addr = self.ea_address(mode, reg, size);
                match size {
                    Size::Byte => self.memory.read_byte(addr) as u32,
                    Size::Word => self.memory.read_word(addr) as u32,
                    Size::Long => self.memory.read_long(addr),
                }
            }
        }
    }

    /// Write to an effective address
    fn write_ea(&mut self, mode: u8, reg: u8, size: Size, val: u32) {
        match mode {
            0 => {
                // Dn
                match size {
                    Size::Byte => {
                        self.d[reg as usize] = (self.d[reg as usize] & 0xFFFF_FF00) | (val & 0xFF);
                    }
                    Size::Word => {
                        self.d[reg as usize] =
                            (self.d[reg as usize] & 0xFFFF_0000) | (val & 0xFFFF);
                    }
                    Size::Long => {
                        self.d[reg as usize] = val;
                    }
                }
            }
            1 => {
                // An — always writes full 32-bit
                self.a[reg as usize] = val;
            }
            _ => {
                let addr = self.ea_address(mode, reg, size);
                match size {
                    Size::Byte => self.memory.write_byte(addr, val as u8),
                    Size::Word => self.memory.write_word(addr, val as u16),
                    Size::Long => self.memory.write_long(addr, val),
                }
            }
        }
    }

    /// Read EA but get both address and value (for read-modify-write)
    fn read_ea_addr_val(&mut self, mode: u8, reg: u8, size: Size) -> (u32, u32) {
        match mode {
            0 => {
                let val = match size {
                    Size::Byte => self.d[reg as usize] & 0xFF,
                    Size::Word => self.d[reg as usize] & 0xFFFF,
                    Size::Long => self.d[reg as usize],
                };
                (0, val)
            }
            _ => {
                let addr = self.ea_address(mode, reg, size);
                let val = match size {
                    Size::Byte => self.memory.read_byte(addr) as u32,
                    Size::Word => self.memory.read_word(addr) as u32,
                    Size::Long => self.memory.read_long(addr),
                };
                (addr, val)
            }
        }
    }

    /// Write back to an EA that was previously read with read_ea_addr_val
    fn write_ea_back(&mut self, mode: u8, reg: u8, size: Size, addr: u32, val: u32) {
        match mode {
            0 => match size {
                Size::Byte => {
                    self.d[reg as usize] = (self.d[reg as usize] & 0xFFFF_FF00) | (val & 0xFF);
                }
                Size::Word => {
                    self.d[reg as usize] = (self.d[reg as usize] & 0xFFFF_0000) | (val & 0xFFFF);
                }
                Size::Long => {
                    self.d[reg as usize] = val;
                }
            },
            _ => match size {
                Size::Byte => self.memory.write_byte(addr, val as u8),
                Size::Word => self.memory.write_word(addr, val as u16),
                Size::Long => self.memory.write_long(addr, val),
            },
        }
    }

    // ── ALU Operations ──────────────────────────────────────────

    fn add_flags(&mut self, src: u32, dst: u32, result: u32, size: Size) {
        let sm = size.msb_mask();
        let s = src & sm != 0;
        let d = dst & sm != 0;
        let r = result & sm != 0;

        // For Long, u32 wrapping_add can't overflow u32, so detect carry via u64
        let carry = match size {
            Size::Byte => result > 0xFF,
            Size::Word => result > 0xFFFF,
            Size::Long => (src as u64).wrapping_add(dst as u64) > 0xFFFF_FFFF,
        };
        self.set_flag(SR_CARRY, carry);
        self.set_flag(SR_EXTEND, carry);
        self.set_flag(SR_OVERFLOW, (s && d && !r) || (!s && !d && r));
        self.set_flag(SR_ZERO, (result & size.mask()) == 0);
        self.set_flag(SR_NEGATIVE, r);
    }

    fn sub_flags(&mut self, src: u32, dst: u32, result: u32, size: Size) {
        let sm = size.msb_mask();
        let s = src & sm != 0;
        let d = dst & sm != 0;
        let r = result & sm != 0;

        // For SUB: borrow occurs when result > dst (unsigned) or equivalently
        // when src > dst for unsigned subtraction
        let borrow = src > dst; // carry on subtraction = borrow
        self.set_flag(SR_CARRY, borrow);
        self.set_flag(SR_EXTEND, borrow);
        self.set_flag(SR_OVERFLOW, (!s && d && !r) || (s && !d && r));
        self.set_flag(SR_ZERO, (result & size.mask()) == 0);
        self.set_flag(SR_NEGATIVE, r);
    }

    fn cmp_flags(&mut self, src: u32, dst: u32, size: Size) {
        let result = dst.wrapping_sub(src);
        let sm = size.msb_mask();
        let s = src & sm != 0;
        let d = dst & sm != 0;
        let r = result & sm != 0;

        let m = size.mask();
        self.set_flag(SR_CARRY, (src & m) > (dst & m));
        self.set_flag(SR_OVERFLOW, (!s && d && !r) || (s && !d && r));
        self.set_flag(SR_ZERO, (result & m) == 0);
        self.set_flag(SR_NEGATIVE, r);
    }

    // ── Main Instruction Decoder ────────────────────────────────

    fn execute(&mut self, opcode: u16) -> u32 {
        // Decode by top 4 bits
        match opcode >> 12 {
            0x0 => self.op_group0(opcode),
            0x1 => self.op_move_byte(opcode),
            0x2 => self.op_move_long(opcode),
            0x3 => self.op_move_word(opcode),
            0x4 => self.op_group4(opcode),
            0x5 => self.op_group5(opcode),
            0x6 => self.op_bcc(opcode),
            0x7 => self.op_moveq(opcode),
            0x8 => self.op_group8(opcode),
            0x9 => self.op_sub(opcode),
            0xA => self.exception(VEC_LINE_A),
            0xB => self.op_cmp_eor(opcode),
            0xC => self.op_group_c(opcode),
            0xD => self.op_add(opcode),
            0xE => self.op_shift(opcode),
            0xF => self.exception(VEC_LINE_F),
            _ => unreachable!(),
        }
    }

    // ── Group 0: Immediate, Bit ops, MOVEP ──────────────────────

    fn op_group0(&mut self, opcode: u16) -> u32 {
        // Check for bit operations first (BTST, BCHG, BCLR, BSET)
        if opcode & 0x0100 != 0 {
            // Dynamic bit operations (register specifies bit number)
            let bit_reg = ((opcode >> 9) & 7) as usize;
            let mode = ((opcode >> 3) & 7) as u8;
            let reg = (opcode & 7) as u8;

            // Check for MOVEP
            if mode == 1 {
                return self.op_movep(opcode);
            }

            let bit_num = self.d[bit_reg];
            let op_type = (opcode >> 6) & 3;
            return self.do_bit_op(op_type, bit_num, mode, reg);
        }

        if opcode & 0x0E00 == 0x0800 && (opcode & 0x0100) == 0 {
            // Static bit operations (immediate bit number, bits 11:9 = 100)
            let bit_num = self.fetch_word() as u32;
            let mode = ((opcode >> 3) & 7) as u8;
            let reg = (opcode & 7) as u8;
            let op_type = (opcode >> 6) & 3;
            return self.do_bit_op(op_type, bit_num, mode, reg);
        }

        // Immediate operations
        let size = match (opcode >> 6) & 3 {
            0 => Size::Byte,
            1 => Size::Word,
            2 => Size::Long,
            _ => return self.exception(VEC_ILLEGAL_INSN),
        };
        let mode = ((opcode >> 3) & 7) as u8;
        let reg = (opcode & 7) as u8;

        match (opcode >> 9) & 7 {
            0 => self.op_ori(size, mode, reg),
            1 => self.op_andi(size, mode, reg),
            2 => self.op_subi(size, mode, reg),
            3 => self.op_addi(size, mode, reg),
            4 => {
                // Static BTST/BCHG/BCLR/BSET (already handled above for #imm)
                // This shouldn't be reached because bit 11 is set.
                self.exception(VEC_ILLEGAL_INSN)
            }
            5 => self.op_eori(size, mode, reg),
            6 => self.op_cmpi(size, mode, reg),
            _ => self.exception(VEC_ILLEGAL_INSN),
        }
    }

    fn do_bit_op(&mut self, op_type: u16, bit_num: u32, mode: u8, reg: u8) -> u32 {
        let (size, bit) = if mode == 0 {
            // Data register: 32-bit, bit number mod 32
            (Size::Long, bit_num & 31)
        } else {
            // Memory: byte, bit number mod 8
            (Size::Byte, bit_num & 7)
        };

        let (addr, val) = self.read_ea_addr_val(mode, reg, size);
        let mask = 1u32 << bit;
        let bit_set = val & mask != 0;

        // BTST: only sets Z flag
        self.set_flag(SR_ZERO, !bit_set);

        match op_type {
            0 => {} // BTST — done
            1 => {
                // BCHG
                let result = val ^ mask;
                self.write_ea_back(mode, reg, size, addr, result);
            }
            2 => {
                // BCLR
                let result = val & !mask;
                self.write_ea_back(mode, reg, size, addr, result);
            }
            3 => {
                // BSET
                let result = val | mask;
                self.write_ea_back(mode, reg, size, addr, result);
            }
            _ => {}
        }

        if mode == 0 {
            if op_type == 0 {
                6
            } else {
                8
            }
        } else if op_type == 0 {
            4
        } else {
            8
        }
    }

    fn op_ori(&mut self, size: Size, mode: u8, reg: u8) -> u32 {
        let imm = match size {
            Size::Byte => self.fetch_word() as u32 & 0xFF,
            Size::Word => self.fetch_word() as u32,
            Size::Long => self.fetch_long(),
        };

        // ORI to CCR (byte) / ORI to SR (word)
        if mode == 7 && reg == 4 {
            if size == Size::Byte {
                // ORI to CCR — only lower 5 bits
                self.sr |= imm as u16 & 0x1F;
            } else {
                // ORI to SR — full 16-bit
                if !self.is_supervisor() {
                    return self.exception(VEC_PRIVILEGE);
                }
                self.set_sr(self.sr | imm as u16);
            }
            return 20;
        }

        let (addr, val) = self.read_ea_addr_val(mode, reg, size);
        let result = val | imm;
        self.write_ea_back(mode, reg, size, addr, result & size.mask());
        self.set_flag(SR_CARRY, false);
        self.set_flag(SR_OVERFLOW, false);
        match size {
            Size::Byte => self.set_nz_byte(result as u8),
            Size::Word => self.set_nz_word(result as u16),
            Size::Long => self.set_nz_long(result),
        }
        if mode == 0 {
            if size == Size::Long {
                16
            } else {
                8
            }
        } else {
            12
        }
    }

    fn op_andi(&mut self, size: Size, mode: u8, reg: u8) -> u32 {
        let imm = match size {
            Size::Byte => self.fetch_word() as u32 & 0xFF,
            Size::Word => self.fetch_word() as u32,
            Size::Long => self.fetch_long(),
        };

        // ANDI to CCR (byte) / ANDI to SR (word)
        if mode == 7 && reg == 4 {
            if size == Size::Byte {
                // ANDI to CCR — only affect lower 5 bits
                self.sr &= (imm as u16 & 0x1F) | 0xFFE0;
            } else {
                // ANDI to SR — full 16-bit
                if !self.is_supervisor() {
                    return self.exception(VEC_PRIVILEGE);
                }
                self.set_sr(self.sr & imm as u16);
            }
            return 20;
        }

        let (addr, val) = self.read_ea_addr_val(mode, reg, size);
        let result = val & imm;
        self.write_ea_back(mode, reg, size, addr, result & size.mask());
        self.set_flag(SR_CARRY, false);
        self.set_flag(SR_OVERFLOW, false);
        match size {
            Size::Byte => self.set_nz_byte(result as u8),
            Size::Word => self.set_nz_word(result as u16),
            Size::Long => self.set_nz_long(result),
        }
        if mode == 0 {
            if size == Size::Long {
                14
            } else {
                8
            }
        } else {
            12
        }
    }

    fn op_subi(&mut self, size: Size, mode: u8, reg: u8) -> u32 {
        let imm = match size {
            Size::Byte => self.fetch_word() as u32 & 0xFF,
            Size::Word => self.fetch_word() as u32,
            Size::Long => self.fetch_long(),
        };
        let (addr, val) = self.read_ea_addr_val(mode, reg, size);
        let result = val.wrapping_sub(imm);
        self.sub_flags(imm, val, result, size);
        self.write_ea_back(mode, reg, size, addr, result & size.mask());
        if mode == 0 {
            if size == Size::Long {
                16
            } else {
                8
            }
        } else {
            12
        }
    }

    fn op_addi(&mut self, size: Size, mode: u8, reg: u8) -> u32 {
        let imm = match size {
            Size::Byte => self.fetch_word() as u32 & 0xFF,
            Size::Word => self.fetch_word() as u32,
            Size::Long => self.fetch_long(),
        };
        let (addr, val) = self.read_ea_addr_val(mode, reg, size);
        let result = val.wrapping_add(imm);
        self.add_flags(imm, val, result, size);
        self.write_ea_back(mode, reg, size, addr, result & size.mask());
        if mode == 0 {
            if size == Size::Long {
                16
            } else {
                8
            }
        } else {
            12
        }
    }

    fn op_eori(&mut self, size: Size, mode: u8, reg: u8) -> u32 {
        let imm = match size {
            Size::Byte => self.fetch_word() as u32 & 0xFF,
            Size::Word => self.fetch_word() as u32,
            Size::Long => self.fetch_long(),
        };

        // EORI to CCR (byte) / EORI to SR (word)
        if mode == 7 && reg == 4 {
            if size == Size::Byte {
                // EORI to CCR — only lower 5 bits
                self.sr ^= imm as u16 & 0x1F;
            } else {
                // EORI to SR — full 16-bit
                if !self.is_supervisor() {
                    return self.exception(VEC_PRIVILEGE);
                }
                self.set_sr(self.sr ^ imm as u16);
            }
            return 20;
        }

        let (addr, val) = self.read_ea_addr_val(mode, reg, size);
        let result = val ^ imm;
        self.write_ea_back(mode, reg, size, addr, result & size.mask());
        self.set_flag(SR_CARRY, false);
        self.set_flag(SR_OVERFLOW, false);
        match size {
            Size::Byte => self.set_nz_byte(result as u8),
            Size::Word => self.set_nz_word(result as u16),
            Size::Long => self.set_nz_long(result),
        }
        if mode == 0 {
            if size == Size::Long {
                16
            } else {
                8
            }
        } else {
            12
        }
    }

    fn op_cmpi(&mut self, size: Size, mode: u8, reg: u8) -> u32 {
        let imm = match size {
            Size::Byte => self.fetch_word() as u32 & 0xFF,
            Size::Word => self.fetch_word() as u32,
            Size::Long => self.fetch_long(),
        };
        let val = self.read_ea(mode, reg, size);
        self.cmp_flags(imm, val, size);
        if mode == 0 {
            if size == Size::Long {
                14
            } else {
                8
            }
        } else {
            8
        }
    }

    fn op_movep(&mut self, opcode: u16) -> u32 {
        let data_reg = ((opcode >> 9) & 7) as usize;
        let addr_reg = (opcode & 7) as usize;
        let disp = self.fetch_word() as i16 as i32;
        let addr = (self.a[addr_reg] as i32).wrapping_add(disp) as u32 & 0x00FF_FFFF;

        match (opcode >> 6) & 3 {
            0 => {
                // MOVEP.W (d16,An), Dn — memory to register, word
                let hi = self.memory.read_byte(addr) as u32;
                let lo = self.memory.read_byte(addr.wrapping_add(2)) as u32;
                let val = (hi << 8) | lo;
                self.d[data_reg] = (self.d[data_reg] & 0xFFFF_0000) | val;
                16
            }
            1 => {
                // MOVEP.L (d16,An), Dn — memory to register, long
                let b3 = self.memory.read_byte(addr) as u32;
                let b2 = self.memory.read_byte(addr.wrapping_add(2)) as u32;
                let b1 = self.memory.read_byte(addr.wrapping_add(4)) as u32;
                let b0 = self.memory.read_byte(addr.wrapping_add(6)) as u32;
                self.d[data_reg] = (b3 << 24) | (b2 << 16) | (b1 << 8) | b0;
                24
            }
            2 => {
                // MOVEP.W Dn, (d16,An) — register to memory, word
                let val = self.d[data_reg];
                self.memory.write_byte(addr, (val >> 8) as u8);
                self.memory.write_byte(addr.wrapping_add(2), val as u8);
                16
            }
            3 => {
                // MOVEP.L Dn, (d16,An) — register to memory, long
                let val = self.d[data_reg];
                self.memory.write_byte(addr, (val >> 24) as u8);
                self.memory
                    .write_byte(addr.wrapping_add(2), (val >> 16) as u8);
                self.memory
                    .write_byte(addr.wrapping_add(4), (val >> 8) as u8);
                self.memory.write_byte(addr.wrapping_add(6), val as u8);
                24
            }
            _ => unreachable!(),
        }
    }

    // ── MOVE instructions ───────────────────────────────────────

    fn do_move(&mut self, opcode: u16, size: Size) -> u32 {
        let src_mode = ((opcode >> 3) & 7) as u8;
        let src_reg = (opcode & 7) as u8;
        let dst_reg = ((opcode >> 9) & 7) as u8;
        let dst_mode = ((opcode >> 6) & 7) as u8;

        let val = self.read_ea(src_mode, src_reg, size);

        // MOVEA doesn't set flags
        if dst_mode == 1 {
            // MOVEA
            let val = match size {
                Size::Word => val as u16 as i16 as i32 as u32, // Sign-extend word
                _ => val,
            };
            self.a[dst_reg as usize] = val;
            return 4;
        }

        // Regular MOVE: set flags
        self.set_flag(SR_CARRY, false);
        self.set_flag(SR_OVERFLOW, false);
        match size {
            Size::Byte => self.set_nz_byte(val as u8),
            Size::Word => self.set_nz_word(val as u16),
            Size::Long => self.set_nz_long(val),
        }

        self.write_ea(dst_mode, dst_reg, size, val & size.mask());
        4 // Simplified timing
    }

    fn op_move_byte(&mut self, opcode: u16) -> u32 {
        self.do_move(opcode, Size::Byte)
    }

    fn op_move_long(&mut self, opcode: u16) -> u32 {
        self.do_move(opcode, Size::Long)
    }

    fn op_move_word(&mut self, opcode: u16) -> u32 {
        self.do_move(opcode, Size::Word)
    }

    // ── Group 4: Miscellaneous ──────────────────────────────────

    fn op_group4(&mut self, opcode: u16) -> u32 {
        let mode = ((opcode >> 3) & 7) as u8;
        let reg = (opcode & 7) as u8;

        // Check for specific instructions
        if opcode & 0xFFF8 == 0x4E70 {
            return match reg {
                0 => {
                    // RESET
                    // (Doesn't actually reset the CPU, just asserts RESET line)
                    132
                }
                1 => {
                    // NOP
                    4
                }
                2 => {
                    // STOP #imm
                    if !self.is_supervisor() {
                        return self.exception(VEC_PRIVILEGE);
                    }
                    let imm = self.fetch_word();
                    self.set_sr(imm);
                    self.stopped = true;
                    4
                }
                3 => {
                    // RTE
                    if !self.is_supervisor() {
                        return self.exception(VEC_PRIVILEGE);
                    }
                    let new_sr = self.pop_word();
                    let new_pc = self.pop_long();
                    self.set_sr(new_sr);
                    self.pc = new_pc & 0x00FF_FFFF;
                    self.prefetch = self.memory.read_word(self.pc);
                    20
                }
                5 => {
                    // RTS
                    self.pc = self.pop_long() & 0x00FF_FFFF;
                    self.prefetch = self.memory.read_word(self.pc);
                    16
                }
                6 => {
                    // TRAPV
                    if self.flag_v() {
                        self.exception(VEC_TRAPV)
                    } else {
                        4
                    }
                }
                7 => {
                    // RTR
                    let ccr = self.pop_word();
                    self.sr = (self.sr & 0xFF00) | (ccr & 0x1F);
                    self.pc = self.pop_long() & 0x00FF_FFFF;
                    self.prefetch = self.memory.read_word(self.pc);
                    20
                }
                _ => self.exception(VEC_ILLEGAL_INSN),
            };
        }

        // TRAP #vector
        if opcode & 0xFFF0 == 0x4E40 {
            let vector = (opcode & 0xF) as u32;
            return self.exception(VEC_TRAP_BASE + vector * 4);
        }

        // LINK
        if opcode & 0xFFF8 == 0x4E50 {
            let an = (opcode & 7) as usize;
            let disp = self.fetch_word() as i16 as i32;
            self.push_long(self.a[an]);
            self.a[an] = self.a[7];
            self.a[7] = (self.a[7] as i32).wrapping_add(disp) as u32 & 0x00FF_FFFF;
            return 16;
        }

        // UNLK
        if opcode & 0xFFF8 == 0x4E58 {
            let an = (opcode & 7) as usize;
            self.a[7] = self.a[an];
            self.a[an] = self.pop_long();
            return 12;
        }

        // MOVE USP
        if opcode & 0xFFF0 == 0x4E60 {
            if !self.is_supervisor() {
                return self.exception(VEC_PRIVILEGE);
            }
            let an = (opcode & 7) as usize;
            if opcode & 0x0008 != 0 {
                // MOVE USP, An
                self.a[an] = self.usp;
            } else {
                // MOVE An, USP
                self.usp = self.a[an];
            }
            return 4;
        }

        // JSR
        if opcode & 0xFFC0 == 0x4E80 {
            let addr = self.ea_address(mode, reg, Size::Word);
            self.push_long(self.pc);
            self.pc = addr & 0x00FF_FFFF;
            self.prefetch = self.memory.read_word(self.pc);
            return 16;
        }

        // JMP
        if opcode & 0xFFC0 == 0x4EC0 {
            let addr = self.ea_address(mode, reg, Size::Word);
            self.pc = addr & 0x00FF_FFFF;
            self.prefetch = self.memory.read_word(self.pc);
            return 8;
        }

        // LEA
        if opcode & 0x01C0 == 0x01C0 && (opcode >> 12) == 4 {
            let an = ((opcode >> 9) & 7) as usize;
            let addr = self.ea_address(mode, reg, Size::Long);
            self.a[an] = addr;
            return 4;
        }

        // Check by bits 11-6
        match (opcode >> 6) & 0x3F {
            0x00 => {
                // NEGX.B
                let (addr, val) = self.read_ea_addr_val(mode, reg, Size::Byte);
                let x = if self.flag_x() { 1u32 } else { 0 };
                let result = 0u32.wrapping_sub(val).wrapping_sub(x);
                self.sub_flags(val.wrapping_add(x), 0, result, Size::Byte);
                if result & 0xFF != 0 {
                    self.set_flag(SR_ZERO, false);
                }
                self.write_ea_back(mode, reg, Size::Byte, addr, result & 0xFF);
                if mode == 0 {
                    4
                } else {
                    8
                }
            }
            0x01 => {
                // NEGX.W
                let (addr, val) = self.read_ea_addr_val(mode, reg, Size::Word);
                let x = if self.flag_x() { 1u32 } else { 0 };
                let result = 0u32.wrapping_sub(val).wrapping_sub(x);
                self.sub_flags(val.wrapping_add(x), 0, result, Size::Word);
                if result & 0xFFFF != 0 {
                    self.set_flag(SR_ZERO, false);
                }
                self.write_ea_back(mode, reg, Size::Word, addr, result & 0xFFFF);
                if mode == 0 {
                    4
                } else {
                    8
                }
            }
            0x02 => {
                // NEGX.L
                let (addr, val) = self.read_ea_addr_val(mode, reg, Size::Long);
                let x = if self.flag_x() { 1u32 } else { 0 };
                let result = 0u32.wrapping_sub(val).wrapping_sub(x);
                self.sub_flags(val.wrapping_add(x), 0, result, Size::Long);
                if result != 0 {
                    self.set_flag(SR_ZERO, false);
                }
                self.write_ea_back(mode, reg, Size::Long, addr, result);
                if mode == 0 {
                    6
                } else {
                    12
                }
            }
            0x03 => {
                // MOVE from SR
                let val = self.sr as u32;
                self.write_ea(mode, reg, Size::Word, val);
                if mode == 0 {
                    6
                } else {
                    8
                }
            }
            0x08 => {
                // CLR.B
                self.write_ea(mode, reg, Size::Byte, 0);
                self.sr = (self.sr & !0x0F) | SR_ZERO; // N=0 Z=1 V=0 C=0
                if mode == 0 {
                    4
                } else {
                    8
                }
            }
            0x09 => {
                // CLR.W
                self.write_ea(mode, reg, Size::Word, 0);
                self.sr = (self.sr & !0x0F) | SR_ZERO;
                if mode == 0 {
                    4
                } else {
                    8
                }
            }
            0x0A => {
                // CLR.L
                self.write_ea(mode, reg, Size::Long, 0);
                self.sr = (self.sr & !0x0F) | SR_ZERO;
                if mode == 0 {
                    6
                } else {
                    12
                }
            }
            0x0B => {
                // MOVE to CCR (word read, only lower byte used)
                let val = self.read_ea(mode, reg, Size::Word);
                self.sr = (self.sr & 0xFF00) | (val as u16 & 0x1F);
                12
            }
            0x10 => {
                // NEG.B
                let (addr, val) = self.read_ea_addr_val(mode, reg, Size::Byte);
                let result = 0u32.wrapping_sub(val);
                self.sub_flags(val, 0, result, Size::Byte);
                self.write_ea_back(mode, reg, Size::Byte, addr, result & 0xFF);
                if mode == 0 {
                    4
                } else {
                    8
                }
            }
            0x11 => {
                // NEG.W
                let (addr, val) = self.read_ea_addr_val(mode, reg, Size::Word);
                let result = 0u32.wrapping_sub(val);
                self.sub_flags(val, 0, result, Size::Word);
                self.write_ea_back(mode, reg, Size::Word, addr, result & 0xFFFF);
                if mode == 0 {
                    4
                } else {
                    8
                }
            }
            0x12 => {
                // NEG.L
                let (addr, val) = self.read_ea_addr_val(mode, reg, Size::Long);
                let result = 0u32.wrapping_sub(val);
                self.sub_flags(val, 0, result, Size::Long);
                self.write_ea_back(mode, reg, Size::Long, addr, result);
                if mode == 0 {
                    6
                } else {
                    12
                }
            }
            0x13 => {
                // MOVE to CCR (opcode $44Cx)
                let val = self.read_ea(mode, reg, Size::Word);
                self.sr = (self.sr & 0xFF00) | (val as u16 & 0x1F);
                12
            }
            0x18 => {
                // NOT.B
                let (addr, val) = self.read_ea_addr_val(mode, reg, Size::Byte);
                let result = !val & 0xFF;
                self.set_flag(SR_CARRY, false);
                self.set_flag(SR_OVERFLOW, false);
                self.set_nz_byte(result as u8);
                self.write_ea_back(mode, reg, Size::Byte, addr, result);
                if mode == 0 {
                    4
                } else {
                    8
                }
            }
            0x19 => {
                // NOT.W
                let (addr, val) = self.read_ea_addr_val(mode, reg, Size::Word);
                let result = !val & 0xFFFF;
                self.set_flag(SR_CARRY, false);
                self.set_flag(SR_OVERFLOW, false);
                self.set_nz_word(result as u16);
                self.write_ea_back(mode, reg, Size::Word, addr, result);
                if mode == 0 {
                    4
                } else {
                    8
                }
            }
            0x1A => {
                // NOT.L
                let (addr, val) = self.read_ea_addr_val(mode, reg, Size::Long);
                let result = !val;
                self.set_flag(SR_CARRY, false);
                self.set_flag(SR_OVERFLOW, false);
                self.set_nz_long(result);
                self.write_ea_back(mode, reg, Size::Long, addr, result);
                if mode == 0 {
                    6
                } else {
                    12
                }
            }
            0x1B => {
                // MOVE to SR (opcode $46Cx)
                if !self.is_supervisor() {
                    return self.exception(VEC_PRIVILEGE);
                }
                let val = self.read_ea(mode, reg, Size::Word);
                self.set_sr(val as u16);
                12
            }
            0x20 => {
                // NBCD <ea>
                let addr = self.ea_address(mode, reg, Size::Byte);
                let val = if mode == 0 {
                    self.d[reg as usize] as u8
                } else {
                    self.memory.read_byte(addr)
                };
                let x = if self.flag_x() { 1u16 } else { 0 };

                // 0 - val - X in BCD
                let low = (0u16).wrapping_sub(val as u16 & 0x0F).wrapping_sub(x);
                let mut result = (0u16).wrapping_sub(val as u16).wrapping_sub(x);

                // BCD correction
                let mut carry = false;
                if low & 0x10 != 0 {
                    result = result.wrapping_sub(6);
                }
                if result & 0x100 != 0 {
                    result = result.wrapping_add(0xA0);
                    carry = true;
                }
                let result_byte = result as u8;

                self.set_flag(SR_CARRY, carry);
                self.set_flag(SR_EXTEND, carry);
                if result_byte != 0 {
                    self.set_flag(SR_ZERO, false);
                }
                self.set_flag(SR_NEGATIVE, result_byte & 0x80 != 0);

                self.write_ea_back(mode, reg, Size::Byte, addr, result_byte as u32);
                if mode == 0 {
                    6
                } else {
                    8
                }
            }
            0x21 => {
                // SWAP (0x4840-0x4847)
                if mode == 0 && opcode & 0xFFF8 == 0x4840 {
                    let r = reg as usize;
                    self.d[r] = self.d[r].rotate_left(16);
                    self.set_flag(SR_CARRY, false);
                    self.set_flag(SR_OVERFLOW, false);
                    self.set_nz_long(self.d[r]);
                    return 4;
                }
                // PEA (0x4840 with mode != 0)
                if opcode & 0xFFC0 == 0x4840 {
                    let addr = self.ea_address(mode, reg, Size::Long);
                    self.push_long(addr);
                    return 12;
                }
                self.exception(VEC_ILLEGAL_INSN)
            }
            0x22 => {
                // EXT.W (0x4880-0x4887) or MOVEM
                if mode == 0 && opcode & 0xFFF8 == 0x4880 {
                    let val = (self.d[reg as usize] & 0xFF) as i8 as i16 as u16;
                    self.d[reg as usize] = (self.d[reg as usize] & 0xFFFF_0000) | val as u32;
                    self.set_flag(SR_CARRY, false);
                    self.set_flag(SR_OVERFLOW, false);
                    self.set_nz_word(val);
                    return 4;
                }
                self.op_movem(opcode)
            }
            0x23 => {
                // EXT.L (0x48C0-0x48C7) or MOVEM
                if mode == 0 && opcode & 0xFFF8 == 0x48C0 {
                    let val = (self.d[reg as usize] & 0xFFFF) as i16 as i32 as u32;
                    self.d[reg as usize] = val;
                    self.set_flag(SR_CARRY, false);
                    self.set_flag(SR_OVERFLOW, false);
                    self.set_nz_long(val);
                    return 4;
                }
                self.op_movem(opcode)
            }
            0x28 => {
                // TST.B
                let val = self.read_ea(mode, reg, Size::Byte);
                self.set_flag(SR_CARRY, false);
                self.set_flag(SR_OVERFLOW, false);
                self.set_nz_byte(val as u8);
                4
            }
            0x29 => {
                // TST.W
                let val = self.read_ea(mode, reg, Size::Word);
                self.set_flag(SR_CARRY, false);
                self.set_flag(SR_OVERFLOW, false);
                self.set_nz_word(val as u16);
                4
            }
            0x2A => {
                // TST.L
                let val = self.read_ea(mode, reg, Size::Long);
                self.set_flag(SR_CARRY, false);
                self.set_flag(SR_OVERFLOW, false);
                self.set_nz_long(val);
                4
            }
            0x2B => {
                // TAS
                let (addr, val) = self.read_ea_addr_val(mode, reg, Size::Byte);
                self.set_flag(SR_CARRY, false);
                self.set_flag(SR_OVERFLOW, false);
                self.set_nz_byte(val as u8);
                self.write_ea_back(mode, reg, Size::Byte, addr, val | 0x80);
                if mode == 0 {
                    4
                } else {
                    14
                }
            }
            0x30..=0x33 => {
                // MOVEM (register to memory or memory to register)
                self.op_movem(opcode)
            }
            _ => {
                // CHK
                if opcode & 0x01C0 == 0x0180 {
                    let dn = ((opcode >> 9) & 7) as usize;
                    let bound = self.read_ea(mode, reg, Size::Word) as i16;
                    let val = self.d[dn] as i16;
                    if val < 0 {
                        self.set_flag(SR_NEGATIVE, true);
                        return self.exception(VEC_CHK);
                    }
                    if val > bound {
                        self.set_flag(SR_NEGATIVE, false);
                        return self.exception(VEC_CHK);
                    }
                    return 10;
                }

                log(LogCategory::CPU, LogLevel::Warn, || {
                    format!(
                        "M68K: Unhandled group 4 opcode ${:04X} at PC=${:06X}",
                        opcode,
                        self.pc.wrapping_sub(2)
                    )
                });
                self.exception(VEC_ILLEGAL_INSN)
            }
        }
    }

    fn op_movem(&mut self, opcode: u16) -> u32 {
        let dir = opcode & 0x0400 != 0; // 1 = memory to register
        let size = if opcode & 0x0040 != 0 {
            Size::Long
        } else {
            Size::Word
        };
        let mode = ((opcode >> 3) & 7) as u8;
        let reg = (opcode & 7) as u8;

        let mask = self.fetch_word();

        if dir {
            // Memory to register
            let mut addr = self.ea_address(mode, reg, size);
            let mut cycles = 12u32;

            for i in 0..16 {
                if mask & (1 << i) != 0 {
                    let val = if size == Size::Long {
                        cycles += 8;
                        let v = self.memory.read_long(addr);
                        addr = addr.wrapping_add(4) & 0x00FF_FFFF;
                        v
                    } else {
                        cycles += 4;
                        let v = self.memory.read_word(addr) as i16 as i32 as u32;
                        addr = addr.wrapping_add(2) & 0x00FF_FFFF;
                        v
                    };

                    if i < 8 {
                        self.d[i] = val;
                    } else {
                        self.a[i - 8] = val;
                    }
                }
            }

            // Post-increment: update An
            if mode == 3 {
                self.a[reg as usize] = addr;
            }

            cycles
        } else {
            // Register to memory
            let predec = mode == 4;
            let mut addr = if predec {
                self.a[reg as usize]
            } else {
                self.ea_address(mode, reg, size)
            };
            let mut cycles = 8u32;

            if predec {
                // Pre-decrement: registers stored in reverse order (A7..A0, D7..D0)
                // In predecrement mode, the mask bits are reversed in the instruction encoding:
                //   bit 0 = A7, bit 1 = A6, ..., bit 7 = A0, bit 8 = D7, ..., bit 15 = D0
                // The CPU processes registers from highest (A7) to lowest (D0), decrementing
                // address before each store. We iterate mask bits 0..15 (A7..D0 order).
                for i in 0..16 {
                    let reg_idx = 15 - i;
                    if mask & (1 << i) != 0 {
                        let val = if reg_idx < 8 {
                            self.d[reg_idx]
                        } else {
                            self.a[reg_idx - 8]
                        };

                        if size == Size::Long {
                            addr = addr.wrapping_sub(4) & 0x00FF_FFFF;
                            self.memory.write_long(addr, val);
                            cycles += 8;
                        } else {
                            addr = addr.wrapping_sub(2) & 0x00FF_FFFF;
                            self.memory.write_word(addr, val as u16);
                            cycles += 4;
                        }
                    }
                }
                self.a[reg as usize] = addr;
            } else {
                for i in 0..16 {
                    if mask & (1 << i) != 0 {
                        let val = if i < 8 { self.d[i] } else { self.a[i - 8] };

                        if size == Size::Long {
                            self.memory.write_long(addr, val);
                            addr = addr.wrapping_add(4) & 0x00FF_FFFF;
                            cycles += 8;
                        } else {
                            self.memory.write_word(addr, val as u16);
                            addr = addr.wrapping_add(2) & 0x00FF_FFFF;
                            cycles += 4;
                        }
                    }
                }
            }

            cycles
        }
    }

    // ── Group 5: ADDQ/SUBQ/Scc/DBcc ─────────────────────────────

    fn op_group5(&mut self, opcode: u16) -> u32 {
        let mode = ((opcode >> 3) & 7) as u8;
        let reg = (opcode & 7) as u8;

        // Scc
        if opcode & 0x00C0 == 0x00C0 {
            let cond = ((opcode >> 8) & 0xF) as u8;

            // DBcc
            if mode == 1 {
                let disp = self.fetch_word() as i16 as i32;
                if !self.test_condition(cond) {
                    let dn = reg as usize;
                    let counter = (self.d[dn] as u16).wrapping_sub(1);
                    self.d[dn] = (self.d[dn] & 0xFFFF_0000) | counter as u32;
                    if counter != 0xFFFF {
                        // Branch
                        self.pc = (self.pc as i32).wrapping_add(disp).wrapping_sub(2) as u32
                            & 0x00FF_FFFF;
                        self.prefetch = self.memory.read_word(self.pc);
                        return 10;
                    }
                }
                return 14; // Condition true or counter expired
            }

            // Scc
            let val = if self.test_condition(cond) {
                0xFF
            } else {
                0x00
            };
            self.write_ea(mode, reg, Size::Byte, val);
            return if mode == 0 {
                if val != 0 {
                    6
                } else {
                    4
                }
            } else {
                8
            };
        }

        // ADDQ / SUBQ
        let size = match (opcode >> 6) & 3 {
            0 => Size::Byte,
            1 => Size::Word,
            2 => Size::Long,
            _ => unreachable!(),
        };

        let mut imm = ((opcode >> 9) & 7) as u32;
        if imm == 0 {
            imm = 8;
        }

        if opcode & 0x0100 == 0 {
            // ADDQ
            if mode == 1 {
                // ADDQ to An — no flags
                self.a[reg as usize] = self.a[reg as usize].wrapping_add(imm);
                return 8;
            }
            let (addr, val) = self.read_ea_addr_val(mode, reg, size);
            let result = val.wrapping_add(imm);
            self.add_flags(imm, val, result, size);
            self.write_ea_back(mode, reg, size, addr, result & size.mask());
            if mode == 0 {
                if size == Size::Long {
                    8
                } else {
                    4
                }
            } else {
                8
            }
        } else {
            // SUBQ
            if mode == 1 {
                // SUBQ to An — no flags
                self.a[reg as usize] = self.a[reg as usize].wrapping_sub(imm);
                return 8;
            }
            let (addr, val) = self.read_ea_addr_val(mode, reg, size);
            let result = val.wrapping_sub(imm);
            self.sub_flags(imm, val, result, size);
            self.write_ea_back(mode, reg, size, addr, result & size.mask());
            if mode == 0 {
                if size == Size::Long {
                    8
                } else {
                    4
                }
            } else {
                8
            }
        }
    }

    // ── Group 6: Bcc/BSR/BRA ────────────────────────────────────

    fn op_bcc(&mut self, opcode: u16) -> u32 {
        let cond = ((opcode >> 8) & 0xF) as u8;
        let mut disp = (opcode & 0xFF) as i8 as i32;

        // Branch base: displacement is relative to (instruction_address + 2),
        // which is the current PC before any extension word fetch.
        let branch_base = self.pc;

        if disp == 0 {
            // Word displacement
            disp = self.fetch_word() as i16 as i32;
        }

        // BRA (condition 0) or BSR (condition 1) always branch
        if cond == 0 {
            // BRA
            self.pc = (branch_base as i32).wrapping_add(disp) as u32 & 0x00FF_FFFF;
            self.prefetch = self.memory.read_word(self.pc);
            return 10;
        }

        if cond == 1 {
            // BSR — return address is after all extension words (self.pc)
            self.push_long(self.pc);
            self.pc = (branch_base as i32).wrapping_add(disp) as u32 & 0x00FF_FFFF;
            self.prefetch = self.memory.read_word(self.pc);
            return 18;
        }

        // Bcc
        if self.test_condition(cond) {
            self.pc = (branch_base as i32).wrapping_add(disp) as u32 & 0x00FF_FFFF;
            self.prefetch = self.memory.read_word(self.pc);
            10
        } else {
            8
        }
    }

    /// Test a condition code
    fn test_condition(&self, cond: u8) -> bool {
        match cond {
            0 => true,                                                // T
            1 => false,                                               // F
            2 => !self.flag_c() && !self.flag_z(),                    // HI
            3 => self.flag_c() || self.flag_z(),                      // LS
            4 => !self.flag_c(),                                      // CC/HS
            5 => self.flag_c(),                                       // CS/LO
            6 => !self.flag_z(),                                      // NE
            7 => self.flag_z(),                                       // EQ
            8 => !self.flag_v(),                                      // VC
            9 => self.flag_v(),                                       // VS
            10 => !self.flag_n(),                                     // PL
            11 => self.flag_n(),                                      // MI
            12 => self.flag_n() == self.flag_v(),                     // GE
            13 => self.flag_n() != self.flag_v(),                     // LT
            14 => !self.flag_z() && (self.flag_n() == self.flag_v()), // GT
            15 => self.flag_z() || (self.flag_n() != self.flag_v()),  // LE
            _ => false,
        }
    }

    // ── Group 7: MOVEQ ─────────────────────────────────────────

    fn op_moveq(&mut self, opcode: u16) -> u32 {
        let dn = ((opcode >> 9) & 7) as usize;
        let data = (opcode & 0xFF) as i8 as i32 as u32;
        self.d[dn] = data;
        self.set_flag(SR_CARRY, false);
        self.set_flag(SR_OVERFLOW, false);
        self.set_nz_long(data);
        4
    }

    // ── Group 8: OR/DIV/SBCD ────────────────────────────────────

    fn op_group8(&mut self, opcode: u16) -> u32 {
        let mode = ((opcode >> 3) & 7) as u8;
        let reg = (opcode & 7) as u8;
        let dn = ((opcode >> 9) & 7) as usize;

        // Check for DIVU/DIVS
        if opcode & 0x01C0 == 0x00C0 {
            // DIVU
            let src = self.read_ea(mode, reg, Size::Word) as u16 as u32;
            if src == 0 {
                return self.exception(VEC_ZERO_DIVIDE);
            }
            let dst = self.d[dn];
            let quotient = dst / src;
            let remainder = dst % src;
            if quotient > 0xFFFF {
                self.set_flag(SR_OVERFLOW, true);
                self.set_flag(SR_CARRY, false);
            } else {
                self.d[dn] = (remainder << 16) | (quotient & 0xFFFF);
                self.set_flag(SR_OVERFLOW, false);
                self.set_flag(SR_CARRY, false);
                self.set_nz_word(quotient as u16);
            }
            return 140; // Worst case
        }

        if opcode & 0x01C0 == 0x01C0 {
            // DIVS
            let src = self.read_ea(mode, reg, Size::Word) as i16 as i32;
            if src == 0 {
                return self.exception(VEC_ZERO_DIVIDE);
            }
            let dst = self.d[dn] as i32;
            let quotient = dst / src;
            let remainder = dst % src;
            if !(-32768..=32767).contains(&quotient) {
                self.set_flag(SR_OVERFLOW, true);
                self.set_flag(SR_CARRY, false);
            } else {
                self.d[dn] = ((remainder as u32) << 16) | (quotient as u16 as u32);
                self.set_flag(SR_OVERFLOW, false);
                self.set_flag(SR_CARRY, false);
                self.set_nz_word(quotient as u16);
            }
            return 158; // Worst case
        }

        // SBCD
        if opcode & 0x01F0 == 0x0100 {
            let rx = ((opcode >> 9) & 7) as usize;
            let ry = (opcode & 7) as usize;
            let rm = opcode & 0x0008 != 0;

            let (src, dst) = if rm {
                // -(Ay), -(Ax)
                self.a[ry] = self.a[ry].wrapping_sub(1);
                self.a[rx] = self.a[rx].wrapping_sub(1);
                let s = self.memory.read_byte(self.a[ry]) as u16;
                let d = self.memory.read_byte(self.a[rx]) as u16;
                (s, d)
            } else {
                // Dy, Dx
                (self.d[ry] as u16 & 0xFF, self.d[rx] as u16 & 0xFF)
            };

            let x = if self.flag_x() { 1u16 } else { 0 };

            let low = (dst & 0x0F).wrapping_sub(src & 0x0F).wrapping_sub(x);
            let mut result = dst.wrapping_sub(src).wrapping_sub(x);

            let mut carry = false;
            if low & 0x10 != 0 {
                result = result.wrapping_sub(6);
            }
            if result & 0x100 != 0 {
                result = result.wrapping_sub(0x60);
                carry = true;
            }
            let result_byte = result as u8;

            self.set_flag(SR_CARRY, carry);
            self.set_flag(SR_EXTEND, carry);
            if result_byte != 0 {
                self.set_flag(SR_ZERO, false);
            }
            self.set_flag(SR_NEGATIVE, result_byte & 0x80 != 0);

            if rm {
                self.memory.write_byte(self.a[rx], result_byte);
                return 18;
            } else {
                self.d[rx] = (self.d[rx] & 0xFFFF_FF00) | result_byte as u32;
                return 6;
            }
        }

        // OR
        let size = match (opcode >> 6) & 3 {
            0 => Size::Byte,
            1 => Size::Word,
            2 => Size::Long,
            _ => return self.exception(VEC_ILLEGAL_INSN),
        };

        if opcode & 0x0100 == 0 {
            // OR <ea>, Dn
            let src = self.read_ea(mode, reg, size);
            let result = self.d[dn] | src;
            match size {
                Size::Byte => {
                    self.d[dn] = (self.d[dn] & 0xFFFF_FF00) | (result & 0xFF);
                    self.set_nz_byte(result as u8);
                }
                Size::Word => {
                    self.d[dn] = (self.d[dn] & 0xFFFF_0000) | (result & 0xFFFF);
                    self.set_nz_word(result as u16);
                }
                Size::Long => {
                    self.d[dn] = result;
                    self.set_nz_long(result);
                }
            }
            self.set_flag(SR_CARRY, false);
            self.set_flag(SR_OVERFLOW, false);
            if size == Size::Long {
                8
            } else {
                4
            }
        } else {
            // OR Dn, <ea>
            let (addr, val) = self.read_ea_addr_val(mode, reg, size);
            let result = val | (self.d[dn] & size.mask());
            self.write_ea_back(mode, reg, size, addr, result & size.mask());
            self.set_flag(SR_CARRY, false);
            self.set_flag(SR_OVERFLOW, false);
            match size {
                Size::Byte => self.set_nz_byte(result as u8),
                Size::Word => self.set_nz_word(result as u16),
                Size::Long => self.set_nz_long(result),
            }
            12
        }
    }

    // ── Group 9: SUB ────────────────────────────────────────────

    fn op_sub(&mut self, opcode: u16) -> u32 {
        let mode = ((opcode >> 3) & 7) as u8;
        let reg = (opcode & 7) as u8;
        let dn = ((opcode >> 9) & 7) as usize;

        // SUBA
        if opcode & 0x00C0 == 0x00C0 {
            let size = if opcode & 0x0100 != 0 {
                Size::Long
            } else {
                Size::Word
            };
            let src = self.read_ea(mode, reg, size);
            let src = if size == Size::Word {
                src as u16 as i16 as i32 as u32
            } else {
                src
            };
            self.a[dn] = self.a[dn].wrapping_sub(src);
            return 8;
        }

        // SUBX
        if opcode & 0x0130 == 0x0100 && (mode == 0 || mode == 4) {
            let size = match (opcode >> 6) & 3 {
                0 => Size::Byte,
                1 => Size::Word,
                2 => Size::Long,
                _ => unreachable!(),
            };
            let x = if self.flag_x() { 1u32 } else { 0 };
            let rx = reg as usize;
            let ry = dn;

            if mode == 0 {
                // Register to register
                let src = self.d[rx] & size.mask();
                let dst = self.d[ry] & size.mask();
                let result = dst.wrapping_sub(src).wrapping_sub(x);
                self.sub_flags(src.wrapping_add(x), dst, result, size);
                if result & size.mask() != 0 {
                    self.set_flag(SR_ZERO, false);
                }
                match size {
                    Size::Byte => {
                        self.d[ry] = (self.d[ry] & 0xFFFF_FF00) | (result & 0xFF);
                    }
                    Size::Word => {
                        self.d[ry] = (self.d[ry] & 0xFFFF_0000) | (result & 0xFFFF);
                    }
                    Size::Long => {
                        self.d[ry] = result;
                    }
                }
                return if size == Size::Long { 8 } else { 4 };
            } else {
                // Memory to memory (pre-decrement)
                let dec = size.bytes();
                self.a[rx] = self.a[rx].wrapping_sub(dec);
                self.a[ry] = self.a[ry].wrapping_sub(dec);
                let src = match size {
                    Size::Byte => self.memory.read_byte(self.a[rx]) as u32,
                    Size::Word => self.memory.read_word(self.a[rx]) as u32,
                    Size::Long => self.memory.read_long(self.a[rx]),
                };
                let dst = match size {
                    Size::Byte => self.memory.read_byte(self.a[ry]) as u32,
                    Size::Word => self.memory.read_word(self.a[ry]) as u32,
                    Size::Long => self.memory.read_long(self.a[ry]),
                };
                let result = dst.wrapping_sub(src).wrapping_sub(x);
                self.sub_flags(src.wrapping_add(x), dst, result, size);
                if result & size.mask() != 0 {
                    self.set_flag(SR_ZERO, false);
                }
                match size {
                    Size::Byte => self.memory.write_byte(self.a[ry], result as u8),
                    Size::Word => self.memory.write_word(self.a[ry], result as u16),
                    Size::Long => self.memory.write_long(self.a[ry], result),
                }
                return 18;
            }
        }

        let size = match (opcode >> 6) & 3 {
            0 => Size::Byte,
            1 => Size::Word,
            2 => Size::Long,
            _ => unreachable!(), // Handled by SUBA above
        };

        if opcode & 0x0100 == 0 {
            // SUB <ea>, Dn
            let src = self.read_ea(mode, reg, size);
            let dst = self.d[dn] & size.mask();
            let result = dst.wrapping_sub(src);
            self.sub_flags(src, dst, result, size);
            match size {
                Size::Byte => {
                    self.d[dn] = (self.d[dn] & 0xFFFF_FF00) | (result & 0xFF);
                }
                Size::Word => {
                    self.d[dn] = (self.d[dn] & 0xFFFF_0000) | (result & 0xFFFF);
                }
                Size::Long => {
                    self.d[dn] = result;
                }
            }
            if size == Size::Long {
                8
            } else {
                4
            }
        } else {
            // SUB Dn, <ea>
            let (addr, val) = self.read_ea_addr_val(mode, reg, size);
            let src = self.d[dn] & size.mask();
            let result = val.wrapping_sub(src);
            self.sub_flags(src, val, result, size);
            self.write_ea_back(mode, reg, size, addr, result & size.mask());
            12
        }
    }

    // ── Group B: CMP/EOR ────────────────────────────────────────

    fn op_cmp_eor(&mut self, opcode: u16) -> u32 {
        let mode = ((opcode >> 3) & 7) as u8;
        let reg = (opcode & 7) as u8;
        let dn = ((opcode >> 9) & 7) as usize;

        // CMPA
        if opcode & 0x00C0 == 0x00C0 {
            let size = if opcode & 0x0100 != 0 {
                Size::Long
            } else {
                Size::Word
            };
            let src = self.read_ea(mode, reg, size);
            let src = if size == Size::Word {
                src as u16 as i16 as i32 as u32
            } else {
                src
            };
            self.cmp_flags(src, self.a[dn], Size::Long);
            return 6;
        }

        if opcode & 0x0100 != 0 {
            // EOR or CMPM
            if mode == 1 {
                // CMPM
                let size = match (opcode >> 6) & 3 {
                    0 => Size::Byte,
                    1 => Size::Word,
                    2 => Size::Long,
                    _ => unreachable!(),
                };
                let rx = reg as usize;
                let ry = dn;
                let src = match size {
                    Size::Byte => {
                        let v = self.memory.read_byte(self.a[rx]) as u32;
                        self.a[rx] = self.a[rx].wrapping_add(1);
                        v
                    }
                    Size::Word => {
                        let v = self.memory.read_word(self.a[rx]) as u32;
                        self.a[rx] = self.a[rx].wrapping_add(2);
                        v
                    }
                    Size::Long => {
                        let v = self.memory.read_long(self.a[rx]);
                        self.a[rx] = self.a[rx].wrapping_add(4);
                        v
                    }
                };
                let dst = match size {
                    Size::Byte => {
                        let v = self.memory.read_byte(self.a[ry]) as u32;
                        self.a[ry] = self.a[ry].wrapping_add(1);
                        v
                    }
                    Size::Word => {
                        let v = self.memory.read_word(self.a[ry]) as u32;
                        self.a[ry] = self.a[ry].wrapping_add(2);
                        v
                    }
                    Size::Long => {
                        let v = self.memory.read_long(self.a[ry]);
                        self.a[ry] = self.a[ry].wrapping_add(4);
                        v
                    }
                };
                self.cmp_flags(src, dst, size);
                return 12;
            }

            // EOR Dn, <ea>
            let size = match (opcode >> 6) & 3 {
                0 => Size::Byte,
                1 => Size::Word,
                2 => Size::Long,
                _ => unreachable!(),
            };
            let (addr, val) = self.read_ea_addr_val(mode, reg, size);
            let result = val ^ (self.d[dn] & size.mask());
            self.write_ea_back(mode, reg, size, addr, result & size.mask());
            self.set_flag(SR_CARRY, false);
            self.set_flag(SR_OVERFLOW, false);
            match size {
                Size::Byte => self.set_nz_byte(result as u8),
                Size::Word => self.set_nz_word(result as u16),
                Size::Long => self.set_nz_long(result),
            }
            if mode == 0 {
                if size == Size::Long {
                    8
                } else {
                    4
                }
            } else {
                12
            }
        } else {
            // CMP <ea>, Dn
            let size = match (opcode >> 6) & 3 {
                0 => Size::Byte,
                1 => Size::Word,
                2 => Size::Long,
                _ => unreachable!(),
            };
            let src = self.read_ea(mode, reg, size);
            let dst = self.d[dn] & size.mask();
            self.cmp_flags(src, dst, size);
            if size == Size::Long {
                6
            } else {
                4
            }
        }
    }

    // ── Group C: AND/MUL/ABCD/EXG ───────────────────────────────

    fn op_group_c(&mut self, opcode: u16) -> u32 {
        let mode = ((opcode >> 3) & 7) as u8;
        let reg = (opcode & 7) as u8;
        let dn = ((opcode >> 9) & 7) as usize;

        // MULU
        if opcode & 0x01C0 == 0x00C0 {
            let src = self.read_ea(mode, reg, Size::Word) as u16 as u32;
            let dst = self.d[dn] as u16 as u32;
            let result = src * dst;
            self.d[dn] = result;
            self.set_flag(SR_CARRY, false);
            self.set_flag(SR_OVERFLOW, false);
            self.set_nz_long(result);
            return 70; // Worst case
        }

        // MULS
        if opcode & 0x01C0 == 0x01C0 {
            let src = self.read_ea(mode, reg, Size::Word) as i16 as i32;
            let dst = self.d[dn] as i16 as i32;
            let result = (src * dst) as u32;
            self.d[dn] = result;
            self.set_flag(SR_CARRY, false);
            self.set_flag(SR_OVERFLOW, false);
            self.set_nz_long(result);
            return 70; // Worst case
        }

        // EXG
        if opcode & 0x01F8 == 0x0140 {
            // EXG Dn, Dn
            let rx = dn;
            let ry = reg as usize;
            self.d.swap(rx, ry);
            return 6;
        }
        if opcode & 0x01F8 == 0x0148 {
            // EXG An, An
            let rx = dn;
            let ry = reg as usize;
            self.a.swap(rx, ry);
            return 6;
        }
        if opcode & 0x01F8 == 0x0188 {
            // EXG Dn, An
            std::mem::swap(&mut self.d[dn], &mut self.a[reg as usize]);
            return 6;
        }

        // ABCD
        if opcode & 0x01F0 == 0x0100 {
            let rx = dn;
            let ry = reg as usize;
            let rm = opcode & 0x0008 != 0;

            let (src, dst) = if rm {
                // -(Ay), -(Ax)
                self.a[ry] = self.a[ry].wrapping_sub(1);
                self.a[rx] = self.a[rx].wrapping_sub(1);
                let s = self.memory.read_byte(self.a[ry]) as u16;
                let d = self.memory.read_byte(self.a[rx]) as u16;
                (s, d)
            } else {
                // Dy, Dx
                (self.d[ry] as u16 & 0xFF, self.d[rx] as u16 & 0xFF)
            };

            let x = if self.flag_x() { 1u16 } else { 0 };

            let low = (dst & 0x0F) + (src & 0x0F) + x;
            let mut result = dst + src + x;

            let mut carry = false;
            if low > 9 {
                result += 6;
            }
            if result > 0x99 {
                result -= 0xA0;
                carry = true;
            }
            let result_byte = result as u8;

            self.set_flag(SR_CARRY, carry);
            self.set_flag(SR_EXTEND, carry);
            if result_byte != 0 {
                self.set_flag(SR_ZERO, false);
            }
            self.set_flag(SR_NEGATIVE, result_byte & 0x80 != 0);

            if rm {
                self.memory.write_byte(self.a[rx], result_byte);
                return 18;
            } else {
                self.d[rx] = (self.d[rx] & 0xFFFF_FF00) | result_byte as u32;
                return 6;
            }
        }

        // AND
        let size = match (opcode >> 6) & 3 {
            0 => Size::Byte,
            1 => Size::Word,
            2 => Size::Long,
            _ => return self.exception(VEC_ILLEGAL_INSN),
        };

        if opcode & 0x0100 == 0 {
            // AND <ea>, Dn
            let src = self.read_ea(mode, reg, size);
            let result = self.d[dn] & src;
            match size {
                Size::Byte => {
                    self.d[dn] = (self.d[dn] & 0xFFFF_FF00) | (result & 0xFF);
                    self.set_nz_byte(result as u8);
                }
                Size::Word => {
                    self.d[dn] = (self.d[dn] & 0xFFFF_0000) | (result & 0xFFFF);
                    self.set_nz_word(result as u16);
                }
                Size::Long => {
                    self.d[dn] = result;
                    self.set_nz_long(result);
                }
            }
            self.set_flag(SR_CARRY, false);
            self.set_flag(SR_OVERFLOW, false);
            if size == Size::Long {
                8
            } else {
                4
            }
        } else {
            // AND Dn, <ea>
            let (addr, val) = self.read_ea_addr_val(mode, reg, size);
            let result = val & (self.d[dn] & size.mask());
            self.write_ea_back(mode, reg, size, addr, result & size.mask());
            self.set_flag(SR_CARRY, false);
            self.set_flag(SR_OVERFLOW, false);
            match size {
                Size::Byte => self.set_nz_byte(result as u8),
                Size::Word => self.set_nz_word(result as u16),
                Size::Long => self.set_nz_long(result),
            }
            12
        }
    }

    // ── Group D: ADD ────────────────────────────────────────────

    fn op_add(&mut self, opcode: u16) -> u32 {
        let mode = ((opcode >> 3) & 7) as u8;
        let reg = (opcode & 7) as u8;
        let dn = ((opcode >> 9) & 7) as usize;

        // ADDA
        if opcode & 0x00C0 == 0x00C0 {
            let size = if opcode & 0x0100 != 0 {
                Size::Long
            } else {
                Size::Word
            };
            let src = self.read_ea(mode, reg, size);
            let src = if size == Size::Word {
                src as u16 as i16 as i32 as u32
            } else {
                src
            };
            self.a[dn] = self.a[dn].wrapping_add(src);
            return 8;
        }

        // ADDX
        if opcode & 0x0130 == 0x0100 && (mode == 0 || mode == 4) {
            let size = match (opcode >> 6) & 3 {
                0 => Size::Byte,
                1 => Size::Word,
                2 => Size::Long,
                _ => unreachable!(),
            };
            let x = if self.flag_x() { 1u32 } else { 0 };
            let rx = reg as usize;
            let ry = dn;

            if mode == 0 {
                let src = self.d[rx] & size.mask();
                let dst = self.d[ry] & size.mask();
                let result = dst.wrapping_add(src).wrapping_add(x);
                self.add_flags(src.wrapping_add(x), dst, result, size);
                if result & size.mask() != 0 {
                    self.set_flag(SR_ZERO, false);
                }
                match size {
                    Size::Byte => {
                        self.d[ry] = (self.d[ry] & 0xFFFF_FF00) | (result & 0xFF);
                    }
                    Size::Word => {
                        self.d[ry] = (self.d[ry] & 0xFFFF_0000) | (result & 0xFFFF);
                    }
                    Size::Long => {
                        self.d[ry] = result;
                    }
                }
                return if size == Size::Long { 8 } else { 4 };
            } else {
                // Memory to memory
                let dec = size.bytes();
                self.a[rx] = self.a[rx].wrapping_sub(dec);
                self.a[ry] = self.a[ry].wrapping_sub(dec);
                let src = match size {
                    Size::Byte => self.memory.read_byte(self.a[rx]) as u32,
                    Size::Word => self.memory.read_word(self.a[rx]) as u32,
                    Size::Long => self.memory.read_long(self.a[rx]),
                };
                let dst = match size {
                    Size::Byte => self.memory.read_byte(self.a[ry]) as u32,
                    Size::Word => self.memory.read_word(self.a[ry]) as u32,
                    Size::Long => self.memory.read_long(self.a[ry]),
                };
                let result = dst.wrapping_add(src).wrapping_add(x);
                self.add_flags(src.wrapping_add(x), dst, result, size);
                if result & size.mask() != 0 {
                    self.set_flag(SR_ZERO, false);
                }
                match size {
                    Size::Byte => self.memory.write_byte(self.a[ry], result as u8),
                    Size::Word => self.memory.write_word(self.a[ry], result as u16),
                    Size::Long => self.memory.write_long(self.a[ry], result),
                }
                return 18;
            }
        }

        let size = match (opcode >> 6) & 3 {
            0 => Size::Byte,
            1 => Size::Word,
            2 => Size::Long,
            _ => unreachable!(),
        };

        if opcode & 0x0100 == 0 {
            // ADD <ea>, Dn
            let src = self.read_ea(mode, reg, size);
            let dst = self.d[dn] & size.mask();
            let result = dst.wrapping_add(src);
            self.add_flags(src, dst, result, size);
            match size {
                Size::Byte => {
                    self.d[dn] = (self.d[dn] & 0xFFFF_FF00) | (result & 0xFF);
                }
                Size::Word => {
                    self.d[dn] = (self.d[dn] & 0xFFFF_0000) | (result & 0xFFFF);
                }
                Size::Long => {
                    self.d[dn] = result;
                }
            }
            if size == Size::Long {
                8
            } else {
                4
            }
        } else {
            // ADD Dn, <ea>
            let (addr, val) = self.read_ea_addr_val(mode, reg, size);
            let src = self.d[dn] & size.mask();
            let result = val.wrapping_add(src);
            self.add_flags(src, val, result, size);
            self.write_ea_back(mode, reg, size, addr, result & size.mask());
            12
        }
    }

    // ── Group E: Shifts and Rotates ─────────────────────────────

    fn op_shift(&mut self, opcode: u16) -> u32 {
        let mode = ((opcode >> 3) & 7) as u8;
        let reg = (opcode & 7) as u8;

        // Memory shifts (size = word, count = 1)
        if opcode & 0x00C0 == 0x00C0 {
            let direction = opcode & 0x0100 != 0; // 1 = left
            let (addr, val) = self.read_ea_addr_val(mode, reg, Size::Word);
            let result = match (opcode >> 9) & 3 {
                0 => self.do_asr(val, 1, Size::Word, direction),
                1 => self.do_lsr(val, 1, Size::Word, direction),
                2 => self.do_roxr(val, 1, Size::Word, direction),
                3 => self.do_ror(val, 1, Size::Word, direction),
                _ => unreachable!(),
            };
            self.write_ea_back(mode, reg, Size::Word, addr, result & 0xFFFF);
            return 8;
        }

        // Register shifts
        let size = match (opcode >> 6) & 3 {
            0 => Size::Byte,
            1 => Size::Word,
            2 => Size::Long,
            _ => unreachable!(),
        };

        let direction = opcode & 0x0100 != 0; // 1 = left
        let ir = opcode & 0x0020 != 0; // 1 = register, 0 = immediate
        let count = if ir {
            self.d[((opcode >> 9) & 7) as usize] & 63
        } else {
            let c = ((opcode >> 9) & 7) as u32;
            if c == 0 {
                8
            } else {
                c
            }
        };

        let val = self.d[reg as usize] & size.mask();

        let result = match (opcode >> 3) & 3 {
            0 => self.do_asr(val, count, size, direction),
            1 => self.do_lsr(val, count, size, direction),
            2 => self.do_roxr(val, count, size, direction),
            3 => self.do_ror(val, count, size, direction),
            _ => unreachable!(),
        };

        match size {
            Size::Byte => {
                self.d[reg as usize] = (self.d[reg as usize] & 0xFFFF_FF00) | (result & 0xFF);
            }
            Size::Word => {
                self.d[reg as usize] = (self.d[reg as usize] & 0xFFFF_0000) | (result & 0xFFFF);
            }
            Size::Long => {
                self.d[reg as usize] = result;
            }
        }

        6 + 2 * count
    }

    fn do_asr(&mut self, val: u32, count: u32, size: Size, left: bool) -> u32 {
        if count == 0 {
            self.set_flag(SR_CARRY, false);
            self.set_flag(SR_OVERFLOW, false);
            match size {
                Size::Byte => self.set_nz_byte(val as u8),
                Size::Word => self.set_nz_word(val as u16),
                Size::Long => self.set_nz_long(val),
            }
            return val;
        }

        let result = if left {
            // ASL
            let bits = size.bits();
            let shifted = if count >= bits { 0 } else { val << count };
            let carry = if count == 0 || count > bits {
                false
            } else {
                val & (1 << (bits - count)) != 0
            };
            self.set_flag(SR_CARRY, carry);
            self.set_flag(SR_EXTEND, carry);
            // V is set if the sign bit changes at any point during the shift
            let msb = size.msb_mask();
            let mut overflow = false;
            let mut v = val;
            for _ in 0..count.min(size.bits()) {
                let old_sign = v & msb != 0;
                v <<= 1;
                v &= size.mask();
                let new_sign = v & msb != 0;
                if old_sign != new_sign {
                    overflow = true;
                }
            }
            self.set_flag(SR_OVERFLOW, overflow);
            shifted & size.mask()
        } else {
            // ASR (arithmetic — sign extends)
            let bits = size.bits();
            let sign = val & size.msb_mask() != 0;
            let result = if count >= bits {
                if sign {
                    size.mask()
                } else {
                    0
                }
            } else {
                let shifted = val >> count;
                if sign {
                    // Fill upper bits with 1s
                    shifted | (size.mask() << (bits - count)) & size.mask()
                } else {
                    shifted
                }
            };
            let carry = if count >= bits {
                sign
            } else {
                val & (1 << (count - 1)) != 0
            };
            self.set_flag(SR_CARRY, carry);
            self.set_flag(SR_EXTEND, carry);
            self.set_flag(SR_OVERFLOW, false);
            result & size.mask()
        };

        match size {
            Size::Byte => self.set_nz_byte(result as u8),
            Size::Word => self.set_nz_word(result as u16),
            Size::Long => self.set_nz_long(result),
        }
        result
    }

    fn do_lsr(&mut self, val: u32, count: u32, size: Size, left: bool) -> u32 {
        if count == 0 {
            self.set_flag(SR_CARRY, false);
            self.set_flag(SR_OVERFLOW, false);
            match size {
                Size::Byte => self.set_nz_byte(val as u8),
                Size::Word => self.set_nz_word(val as u16),
                Size::Long => self.set_nz_long(val),
            }
            return val;
        }

        let bits = size.bits();
        let result = if left {
            let r = if count >= bits { 0 } else { val << count };
            let carry = if count > bits {
                false
            } else {
                val & (1 << (bits - count)) != 0
            };
            self.set_flag(SR_CARRY, carry);
            self.set_flag(SR_EXTEND, carry);
            r & size.mask()
        } else {
            let r = if count >= bits { 0 } else { val >> count };
            let carry = if count > bits {
                false
            } else {
                val & (1 << (count - 1)) != 0
            };
            self.set_flag(SR_CARRY, carry);
            self.set_flag(SR_EXTEND, carry);
            r & size.mask()
        };

        self.set_flag(SR_OVERFLOW, false);
        match size {
            Size::Byte => self.set_nz_byte(result as u8),
            Size::Word => self.set_nz_word(result as u16),
            Size::Long => self.set_nz_long(result),
        }
        result
    }

    fn do_ror(&mut self, val: u32, count: u32, size: Size, left: bool) -> u32 {
        if count == 0 {
            self.set_flag(SR_CARRY, false);
            self.set_flag(SR_OVERFLOW, false);
            match size {
                Size::Byte => self.set_nz_byte(val as u8),
                Size::Word => self.set_nz_word(val as u16),
                Size::Long => self.set_nz_long(val),
            }
            return val;
        }

        let bits = size.bits();
        let count = count % bits;
        let result = if left {
            ((val << count) | (val >> (bits - count))) & size.mask()
        } else {
            ((val >> count) | (val << (bits - count))) & size.mask()
        };

        let carry = if left {
            result & 1 != 0
        } else {
            result & size.msb_mask() != 0
        };
        self.set_flag(SR_CARRY, carry);
        self.set_flag(SR_OVERFLOW, false);
        match size {
            Size::Byte => self.set_nz_byte(result as u8),
            Size::Word => self.set_nz_word(result as u16),
            Size::Long => self.set_nz_long(result),
        }
        result
    }

    fn do_roxr(&mut self, val: u32, count: u32, size: Size, left: bool) -> u32 {
        let bits = size.bits();
        let _total_bits = bits + 1; // Include X bit
        let mut result = val;
        let mut x = self.flag_x();

        for _ in 0..count {
            if left {
                let msb = result & size.msb_mask() != 0;
                result = ((result << 1) & size.mask()) | (if x { 1 } else { 0 });
                x = msb;
            } else {
                let lsb = result & 1 != 0;
                result = (result >> 1) | (if x { size.msb_mask() } else { 0 });
                x = lsb;
            }
        }

        self.set_flag(SR_CARRY, x);
        self.set_flag(SR_EXTEND, x);
        self.set_flag(SR_OVERFLOW, false);
        match size {
            Size::Byte => self.set_nz_byte(result as u8),
            Size::Word => self.set_nz_word(result as u16),
            Size::Long => self.set_nz_long(result),
        }
        result & size.mask()
    }
}

/// Operand size
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Size {
    Byte,
    Word,
    Long,
}

impl Size {
    fn bytes(self) -> u32 {
        match self {
            Size::Byte => 1,
            Size::Word => 2,
            Size::Long => 4,
        }
    }

    fn bits(self) -> u32 {
        match self {
            Size::Byte => 8,
            Size::Word => 16,
            Size::Long => 32,
        }
    }

    fn mask(self) -> u32 {
        match self {
            Size::Byte => 0xFF,
            Size::Word => 0xFFFF,
            Size::Long => 0xFFFF_FFFF,
        }
    }

    fn msb_mask(self) -> u32 {
        match self {
            Size::Byte => 0x80,
            Size::Word => 0x8000,
            Size::Long => 0x8000_0000,
        }
    }

    fn max_val(self) -> u32 {
        self.mask()
    }
}

/// Disassemble a single opcode (basic disassembler for debugging)
pub fn disassemble_opcode<M: Memory68k>(mem: &M, addr: u32, opcode: u16) -> (String, u32) {
    // Simplified disassembler — handles common instructions
    let pc = addr.wrapping_add(2);

    match opcode >> 12 {
        0x4 => {
            if opcode & 0xFFF8 == 0x4E70 {
                let s = match opcode & 7 {
                    0 => "RESET",
                    1 => "NOP",
                    3 => "RTE",
                    5 => "RTS",
                    6 => "TRAPV",
                    7 => "RTR",
                    _ => "???",
                };
                return (s.to_string(), 2);
            }
            if opcode & 0xFFC0 == 0x4E80 {
                return (format!("JSR ${:06X}", mem.read_long(pc)), 6);
            }
            if opcode & 0xFFC0 == 0x4EC0 {
                return (format!("JMP ${:06X}", mem.read_long(pc)), 6);
            }
        }
        0x6 => {
            let cond = (opcode >> 8) & 0xF;
            let cond_name = match cond {
                0 => "BRA",
                1 => "BSR",
                2 => "BHI",
                3 => "BLS",
                4 => "BCC",
                5 => "BCS",
                6 => "BNE",
                7 => "BEQ",
                8 => "BVC",
                9 => "BVS",
                10 => "BPL",
                11 => "BMI",
                12 => "BGE",
                13 => "BLT",
                14 => "BGT",
                15 => "BLE",
                _ => "B??",
            };
            let disp = (opcode & 0xFF) as i8;
            if disp == 0 {
                let w = mem.read_word(pc) as i16;
                let target = (pc as i32 + w as i32) as u32;
                return (format!("{} ${:06X}", cond_name, target), 4);
            }
            let target = (pc as i32 + disp as i32) as u32;
            return (format!("{}.S ${:06X}", cond_name, target), 2);
        }
        0x7 => {
            let dn = (opcode >> 9) & 7;
            let data = (opcode & 0xFF) as i8;
            return (format!("MOVEQ #${:02X},D{}", data as u8, dn), 2);
        }
        _ => {}
    }

    (format!("DC.W ${:04X}", opcode), 2)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestMem {
        data: Vec<u8>,
    }

    impl TestMem {
        fn new(size: usize) -> Self {
            Self {
                data: vec![0; size],
            }
        }
    }

    impl Memory68k for TestMem {
        fn read_byte(&self, addr: u32) -> u8 {
            let addr = addr as usize & (self.data.len() - 1);
            self.data[addr]
        }

        fn read_word(&self, addr: u32) -> u16 {
            let addr = addr as usize & (self.data.len() - 1);
            let hi = self.data[addr] as u16;
            let lo = self.data[addr.wrapping_add(1) % self.data.len()] as u16;
            (hi << 8) | lo
        }

        fn write_byte(&mut self, addr: u32, val: u8) {
            let addr = addr as usize & (self.data.len() - 1);
            self.data[addr] = val;
        }

        fn write_word(&mut self, addr: u32, val: u16) {
            let addr = addr as usize & (self.data.len() - 1);
            let len = self.data.len();
            self.data[addr] = (val >> 8) as u8;
            self.data[addr.wrapping_add(1) % len] = val as u8;
        }
    }

    fn setup_cpu() -> M68k<TestMem> {
        let mut mem = TestMem::new(0x10000);
        // Set up reset vectors
        // SSP at $000000 = $0000FF00
        mem.data[0] = 0x00;
        mem.data[1] = 0x00;
        mem.data[2] = 0xFF;
        mem.data[3] = 0x00;
        // PC at $000004 = $00000400
        mem.data[4] = 0x00;
        mem.data[5] = 0x00;
        mem.data[6] = 0x04;
        mem.data[7] = 0x00;

        let mut cpu = M68k::new(mem);
        cpu.reset();
        cpu
    }

    fn write_opcode(cpu: &mut M68k<TestMem>, addr: u32, opcode: u16) {
        cpu.memory.write_word(addr, opcode);
        // If writing at the current PC, refresh the prefetch so the CPU sees the new opcode
        if addr == cpu.pc {
            cpu.prefetch = cpu.memory.read_word(cpu.pc);
        }
    }

    #[test]
    fn test_reset() {
        let cpu = setup_cpu();
        assert_eq!(cpu.pc, 0x0400);
        assert_eq!(cpu.a[7], 0x0000_FF00);
        assert!(cpu.is_supervisor());
    }

    #[test]
    fn test_nop() {
        let mut cpu = setup_cpu();
        write_opcode(&mut cpu, 0x0400, 0x4E71); // NOP
        write_opcode(&mut cpu, 0x0402, 0x4E71); // NOP
        let cycles = cpu.step();
        assert_eq!(cpu.pc, 0x0402);
        assert_eq!(cycles, 4);
    }

    #[test]
    fn test_moveq() {
        let mut cpu = setup_cpu();
        // MOVEQ #42, D0 = 0x7042
        write_opcode(&mut cpu, 0x0400, 0x7000 | 42);
        cpu.step();
        assert_eq!(cpu.d[0], 42);
        assert!(!cpu.flag_z());
        assert!(!cpu.flag_n());
    }

    #[test]
    fn test_moveq_negative() {
        let mut cpu = setup_cpu();
        // MOVEQ #-1, D0 = 0x70FF
        write_opcode(&mut cpu, 0x0400, 0x70FF);
        cpu.step();
        assert_eq!(cpu.d[0], 0xFFFF_FFFF);
        assert!(cpu.flag_n());
    }

    #[test]
    fn test_bra() {
        let mut cpu = setup_cpu();
        // BRA.S +4 (byte displacement: PC + 4 from after fetching opcode)
        write_opcode(&mut cpu, 0x0400, 0x6004); // BRA.S $0406
        cpu.step();
        assert_eq!(cpu.pc, 0x0406);
    }

    #[test]
    fn test_add_dn() {
        let mut cpu = setup_cpu();
        cpu.d[0] = 10;
        cpu.d[1] = 20;
        // ADD.L D0, D1 = 0xD280
        write_opcode(&mut cpu, 0x0400, 0xD280);
        cpu.step();
        assert_eq!(cpu.d[1], 30);
    }

    #[test]
    fn test_sub_dn() {
        let mut cpu = setup_cpu();
        cpu.d[0] = 10;
        cpu.d[1] = 30;
        // SUB.L D0, D1 = 0x9280
        write_opcode(&mut cpu, 0x0400, 0x9280);
        cpu.step();
        assert_eq!(cpu.d[1], 20);
    }

    #[test]
    fn test_clr() {
        let mut cpu = setup_cpu();
        cpu.d[0] = 0x12345678;
        // CLR.L D0 = 0x4280
        write_opcode(&mut cpu, 0x0400, 0x4280);
        cpu.step();
        assert_eq!(cpu.d[0], 0);
        assert!(cpu.flag_z());
    }

    #[test]
    fn test_cmp() {
        let mut cpu = setup_cpu();
        cpu.d[0] = 10;
        cpu.d[1] = 10;
        // CMP.L D0, D1 = 0xB280
        write_opcode(&mut cpu, 0x0400, 0xB280);
        cpu.step();
        assert!(cpu.flag_z());
        assert!(!cpu.flag_n());
    }

    #[test]
    fn test_swap() {
        let mut cpu = setup_cpu();
        cpu.d[0] = 0x12345678;
        // SWAP D0 = 0x4840
        write_opcode(&mut cpu, 0x0400, 0x4840);
        cpu.step();
        assert_eq!(cpu.d[0], 0x56781234);
    }

    #[test]
    fn test_ext_word() {
        let mut cpu = setup_cpu();
        cpu.d[0] = 0x000000FF;
        // EXT.W D0 = 0x4880
        write_opcode(&mut cpu, 0x0400, 0x4880);
        cpu.step();
        assert_eq!(cpu.d[0] & 0xFFFF, 0xFFFF);
    }

    #[test]
    fn test_ext_long() {
        let mut cpu = setup_cpu();
        cpu.d[0] = 0x0000FFFF;
        // EXT.L D0 = 0x48C0
        write_opcode(&mut cpu, 0x0400, 0x48C0);
        cpu.step();
        assert_eq!(cpu.d[0], 0xFFFF_FFFF);
    }

    #[test]
    fn test_jsr_rts() {
        let mut cpu = setup_cpu();
        // JSR $0410 (absolute long)
        write_opcode(&mut cpu, 0x0400, 0x4EB9); // JSR (xxx).L
        cpu.memory.write_long(0x0402, 0x00000410);
        // At $0410: RTS
        write_opcode(&mut cpu, 0x0410, 0x4E75);

        let sp_before = cpu.a[7];
        cpu.step(); // JSR
        assert_eq!(cpu.pc, 0x0410);
        assert_eq!(cpu.a[7], sp_before - 4);

        cpu.step(); // RTS
        assert_eq!(cpu.pc, 0x0406);
    }

    #[test]
    fn test_move_to_sr() {
        let mut cpu = setup_cpu();
        // MOVE.W #$2700, SR = $46FC $2700
        // This is the first instruction in almost every Mega Drive game
        write_opcode(&mut cpu, 0x0400, 0x46FC); // MOVE to SR, immediate
        cpu.memory.write_word(0x0402, 0x2700);
        cpu.step();
        // SR should be $2700: supervisor mode, interrupt mask = 7
        assert_eq!(cpu.sr, 0x2700);
        assert!(cpu.is_supervisor());
        assert_eq!((cpu.sr >> 8) & 7, 7); // interrupt mask = 7
    }

    #[test]
    fn test_move_to_ccr() {
        let mut cpu = setup_cpu();
        // MOVE.W #$001F, CCR = $44FC $001F
        write_opcode(&mut cpu, 0x0400, 0x44FC); // MOVE to CCR, immediate
        cpu.memory.write_word(0x0402, 0x001F);
        let old_sr_upper = cpu.sr & 0xFF00;
        cpu.step();
        // Only lower 5 bits (CCR) should be affected, upper byte unchanged
        assert_eq!(cpu.sr & 0x1F, 0x1F);
        assert_eq!(cpu.sr & 0xFF00, old_sr_upper);
    }

    #[test]
    fn test_add_long_carry() {
        let mut cpu = setup_cpu();
        cpu.d[0] = 0xFFFF_FFFF;
        cpu.d[1] = 0x0000_0001;
        // ADD.L D0, D1 = 0xD280
        write_opcode(&mut cpu, 0x0400, 0xD280);
        cpu.step();
        assert_eq!(cpu.d[1], 0); // wraps to 0
        assert!(cpu.flag_c(), "carry must be set on 32-bit overflow");
        assert!(cpu.flag_x(), "extend must mirror carry");
        assert!(cpu.flag_z(), "result is zero");
    }

    #[test]
    fn test_add_long_no_carry() {
        let mut cpu = setup_cpu();
        cpu.d[0] = 0x0000_0001;
        cpu.d[1] = 0x0000_0002;
        // ADD.L D0, D1 = 0xD280
        write_opcode(&mut cpu, 0x0400, 0xD280);
        cpu.step();
        assert_eq!(cpu.d[1], 3);
        assert!(!cpu.flag_c(), "no carry for small add");
    }

    #[test]
    fn test_cmpi_word() {
        let mut cpu = setup_cpu();
        cpu.d[0] = 0x0042;
        // CMPI.W #$0042, D0 = 0x0C40 0x0042
        write_opcode(&mut cpu, 0x0400, 0x0C40);
        cpu.memory.write_word(0x0402, 0x0042);
        cpu.step();
        assert!(cpu.flag_z(), "CMPI equal should set Z");
        assert!(!cpu.flag_n(), "CMPI equal should clear N");
        assert!(!cpu.flag_c(), "CMPI equal should clear C");
    }

    #[test]
    fn test_cmpi_not_equal() {
        let mut cpu = setup_cpu();
        cpu.d[0] = 0x0010;
        // CMPI.W #$0042, D0 — D0 - #$42 = negative
        write_opcode(&mut cpu, 0x0400, 0x0C40);
        cpu.memory.write_word(0x0402, 0x0042);
        cpu.step();
        assert!(!cpu.flag_z(), "CMPI not-equal should clear Z");
    }

    #[test]
    fn test_eori_byte() {
        let mut cpu = setup_cpu();
        cpu.d[0] = 0xFF;
        // EORI.B #$FF, D0 = 0x0A00 0x00FF
        write_opcode(&mut cpu, 0x0400, 0x0A00);
        cpu.memory.write_word(0x0402, 0x00FF);
        cpu.step();
        assert_eq!(cpu.d[0] & 0xFF, 0x00, "0xFF ^ 0xFF should be 0");
        assert!(cpu.flag_z(), "result is zero");
    }
}
