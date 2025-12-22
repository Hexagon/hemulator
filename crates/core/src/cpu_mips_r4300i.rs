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

        // Check for pending interrupts before fetching instruction
        if self.check_interrupts() {
            // Interrupt was handled, return early
            return (self.cycles - start_cycles) as u32;
        }

        // Fetch instruction
        let instr = self.memory.read_word(self.pc as u32);
        self.pc = self.pc.wrapping_add(4);

        // Decode opcode (bits 26-31)
        let opcode = (instr >> 26) & 0x3F;

        match opcode {
            0x00 => self.execute_special(instr), // R-type instructions
            0x01 => self.execute_regimm(instr),  // REGIMM (branch instructions)
            0x02 => self.execute_j(instr),       // J
            0x03 => self.execute_jal(instr),     // JAL
            0x04 => self.execute_beq(instr),     // BEQ
            0x05 => self.execute_bne(instr),     // BNE
            0x06 => self.execute_blez(instr),    // BLEZ
            0x07 => self.execute_bgtz(instr),    // BGTZ
            0x08 => self.execute_addi(instr),    // ADDI
            0x09 => self.execute_addiu(instr),   // ADDIU
            0x0A => self.execute_slti(instr),    // SLTI
            0x0B => self.execute_sltiu(instr),   // SLTIU
            0x0C => self.execute_andi(instr),    // ANDI
            0x0D => self.execute_ori(instr),     // ORI
            0x0E => self.execute_xori(instr),    // XORI
            0x0F => self.execute_lui(instr),     // LUI
            0x10 => self.execute_cop0(instr),    // COP0
            0x11 => self.execute_cop1(instr),    // COP1
            0x14 => self.execute_beql(instr),    // BEQL
            0x15 => self.execute_bnel(instr),    // BNEL
            0x16 => self.execute_blezl(instr),   // BLEZL
            0x17 => self.execute_bgtzl(instr),   // BGTZL
            0x18 => self.execute_daddi(instr),   // DADDI
            0x19 => self.execute_daddiu(instr),  // DADDIU
            0x1A => self.execute_ldl(instr),     // LDL
            0x1B => self.execute_ldr(instr),     // LDR
            0x20 => self.execute_lb(instr),      // LB
            0x21 => self.execute_lh(instr),      // LH
            0x22 => self.execute_lwl(instr),     // LWL
            0x23 => self.execute_lw(instr),      // LW
            0x24 => self.execute_lbu(instr),     // LBU
            0x25 => self.execute_lhu(instr),     // LHU
            0x26 => self.execute_lwr(instr),     // LWR
            0x27 => self.execute_lwu(instr),     // LWU
            0x28 => self.execute_sb(instr),      // SB
            0x29 => self.execute_sh(instr),      // SH
            0x2A => self.execute_swl(instr),     // SWL
            0x2B => self.execute_sw(instr),      // SW
            0x2C => self.execute_sdl(instr),     // SDL
            0x2D => self.execute_sdr(instr),     // SDR
            0x2E => self.execute_swr(instr),     // SWR
            0x2F => self.execute_cache(instr),   // CACHE
            0x37 => self.execute_ld(instr),      // LD
            0x3F => self.execute_sd(instr),      // SD
            _ => {
                // Unimplemented instruction
                self.cycles += 1;
            }
        }

        // R0 is always zero
        self.gpr[0] = 0;

        (self.cycles - start_cycles) as u32
    }

    /// Check for pending interrupts and handle if enabled
    /// Returns true if an interrupt was handled
    fn check_interrupts(&mut self) -> bool {
        // Check if interrupts are globally enabled (IE bit in Status register)
        let status = self.cp0[CP0_STATUS];
        let ie = status & 0x01;
        let exl = (status >> 1) & 0x01;
        let erl = (status >> 2) & 0x01;

        // Interrupts are enabled if IE=1 and EXL=0 and ERL=0
        if ie == 0 || exl != 0 || erl != 0 {
            return false;
        }

        // Check if any interrupts are pending and unmasked
        let cause = self.cp0[CP0_CAUSE];
        let im = (status >> 8) & 0xFF; // Interrupt mask in Status
        let ip = (cause >> 8) & 0xFF; // Interrupt pending in Cause

        // Check if any unmasked interrupt is pending
        if (im & ip) != 0 {
            self.handle_exception(0); // Exception code 0 = Interrupt
            true
        } else {
            false
        }
    }

    /// Handle an exception/interrupt
    fn handle_exception(&mut self, exception_code: u64) {
        // Set EXL bit in Status register (disable further interrupts)
        self.cp0[CP0_STATUS] |= 0x02; // Set EXL bit

        // Save return address in EPC (current PC, not incremented)
        self.cp0[CP0_EPC] = self.pc;

        // Set exception code in Cause register
        self.cp0[CP0_CAUSE] &= !0x7C; // Clear exception code bits (2-6)
        self.cp0[CP0_CAUSE] |= (exception_code << 2) & 0x7C;

        // Jump to exception vector
        // Normal exception vector is at 0x80000180
        self.pc = 0x80000180;

        self.cycles += 1; // Exception handling takes cycles
    }

    /// Set a pending interrupt in the Cause register (called by memory interface)
    pub fn set_interrupt(&mut self, interrupt_bit: u8) {
        // Interrupt pending bits are in Cause register bits 8-15
        let bit = 1u64 << (8 + interrupt_bit);
        self.cp0[CP0_CAUSE] |= bit;
    }

    /// Clear a pending interrupt in the Cause register
    #[allow(dead_code)] // Reserved for future use
    pub fn clear_interrupt(&mut self, interrupt_bit: u8) {
        // Interrupt pending bits are in Cause register bits 8-15
        let bit = 1u64 << (8 + interrupt_bit);
        self.cp0[CP0_CAUSE] &= !bit;
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
            0x02 => {
                // SRL - Shift Right Logical
                self.gpr[rd] = (self.gpr[rt] as u32).wrapping_shr(shamt) as i32 as u64;
                self.cycles += 1;
            }
            0x03 => {
                // SRA - Shift Right Arithmetic
                self.gpr[rd] = ((self.gpr[rt] as i32) >> shamt) as u64;
                self.cycles += 1;
            }
            0x04 => {
                // SLLV - Shift Left Logical Variable
                let shift = self.gpr[rs] & 0x1F;
                self.gpr[rd] = (self.gpr[rt] as u32).wrapping_shl(shift as u32) as i32 as u64;
                self.cycles += 1;
            }
            0x06 => {
                // SRLV - Shift Right Logical Variable
                let shift = self.gpr[rs] & 0x1F;
                self.gpr[rd] = (self.gpr[rt] as u32).wrapping_shr(shift as u32) as i32 as u64;
                self.cycles += 1;
            }
            0x07 => {
                // SRAV - Shift Right Arithmetic Variable
                let shift = self.gpr[rs] & 0x1F;
                self.gpr[rd] = ((self.gpr[rt] as i32) >> shift) as u64;
                self.cycles += 1;
            }
            0x08 => {
                // JR - Jump Register
                self.pc = self.gpr[rs];
                self.cycles += 1;
            }
            0x09 => {
                // JALR - Jump And Link Register
                self.gpr[rd] = self.pc;
                self.pc = self.gpr[rs];
                self.cycles += 1;
            }
            0x10 => {
                // MFHI - Move From HI
                self.gpr[rd] = self.hi;
                self.cycles += 1;
            }
            0x11 => {
                // MTHI - Move To HI
                self.hi = self.gpr[rs];
                self.cycles += 1;
            }
            0x12 => {
                // MFLO - Move From LO
                self.gpr[rd] = self.lo;
                self.cycles += 1;
            }
            0x13 => {
                // MTLO - Move To LO
                self.lo = self.gpr[rs];
                self.cycles += 1;
            }
            0x14 => {
                // DSLLV - Doubleword Shift Left Logical Variable
                let shift = self.gpr[rs] & 0x3F;
                self.gpr[rd] = self.gpr[rt].wrapping_shl(shift as u32);
                self.cycles += 1;
            }
            0x16 => {
                // DSRLV - Doubleword Shift Right Logical Variable
                let shift = self.gpr[rs] & 0x3F;
                self.gpr[rd] = self.gpr[rt].wrapping_shr(shift as u32);
                self.cycles += 1;
            }
            0x17 => {
                // DSRAV - Doubleword Shift Right Arithmetic Variable
                let shift = self.gpr[rs] & 0x3F;
                self.gpr[rd] = ((self.gpr[rt] as i64) >> shift) as u64;
                self.cycles += 1;
            }
            0x18 => {
                // MULT - Multiply
                let a = self.gpr[rs] as i32 as i64;
                let b = self.gpr[rt] as i32 as i64;
                let result = a.wrapping_mul(b);
                self.lo = result as u64;
                self.hi = ((result >> 32) as i32) as u64;
                self.cycles += 1;
            }
            0x19 => {
                // MULTU - Multiply Unsigned
                let a = (self.gpr[rs] as u32) as u64;
                let b = (self.gpr[rt] as u32) as u64;
                let result = a.wrapping_mul(b);
                self.lo = (result as u32) as i32 as u64;
                self.hi = ((result >> 32) as u32) as i32 as u64;
                self.cycles += 1;
            }
            0x1A => {
                // DIV - Divide
                let dividend = self.gpr[rs] as i32;
                let divisor = self.gpr[rt] as i32;
                if divisor != 0 {
                    self.lo = dividend.wrapping_div(divisor) as u64;
                    self.hi = dividend.wrapping_rem(divisor) as u64;
                }
                self.cycles += 1;
            }
            0x1B => {
                // DIVU - Divide Unsigned
                let dividend = self.gpr[rs] as u32;
                let divisor = self.gpr[rt] as u32;
                if divisor != 0 {
                    self.lo = (dividend / divisor) as i32 as u64;
                    self.hi = (dividend % divisor) as i32 as u64;
                }
                self.cycles += 1;
            }
            0x1C => {
                // DMULT - Doubleword Multiply
                let a = self.gpr[rs] as i64 as i128;
                let b = self.gpr[rt] as i64 as i128;
                let result = a.wrapping_mul(b);
                self.lo = result as u64;
                self.hi = (result >> 64) as u64;
                self.cycles += 1;
            }
            0x1D => {
                // DMULTU - Doubleword Multiply Unsigned
                let a = self.gpr[rs] as u128;
                let b = self.gpr[rt] as u128;
                let result = a.wrapping_mul(b);
                self.lo = result as u64;
                self.hi = (result >> 64) as u64;
                self.cycles += 1;
            }
            0x1E => {
                // DDIV - Doubleword Divide
                let dividend = self.gpr[rs] as i64;
                let divisor = self.gpr[rt] as i64;
                if divisor != 0 {
                    self.lo = dividend.wrapping_div(divisor) as u64;
                    self.hi = dividend.wrapping_rem(divisor) as u64;
                }
                self.cycles += 1;
            }
            0x1F => {
                // DDIVU - Doubleword Divide Unsigned
                let dividend = self.gpr[rs];
                let divisor = self.gpr[rt];
                if divisor != 0 {
                    self.lo = dividend / divisor;
                    self.hi = dividend % divisor;
                }
                self.cycles += 1;
            }
            0x20 => {
                // ADD - Add (with overflow trap)
                let a = self.gpr[rs] as i32;
                let b = self.gpr[rt] as i32;
                // For now, we don't implement traps, just perform the addition
                self.gpr[rd] = a.wrapping_add(b) as u64;
                self.cycles += 1;
            }
            0x21 => {
                // ADDU - Add Unsigned
                self.gpr[rd] =
                    (self.gpr[rs] as u32).wrapping_add(self.gpr[rt] as u32) as i32 as u64;
                self.cycles += 1;
            }
            0x22 => {
                // SUB - Subtract (with overflow trap)
                let a = self.gpr[rs] as i32;
                let b = self.gpr[rt] as i32;
                // For now, we don't implement traps, just perform the subtraction
                self.gpr[rd] = a.wrapping_sub(b) as u64;
                self.cycles += 1;
            }
            0x23 => {
                // SUBU - Subtract Unsigned
                self.gpr[rd] =
                    (self.gpr[rs] as u32).wrapping_sub(self.gpr[rt] as u32) as i32 as u64;
                self.cycles += 1;
            }
            0x24 => {
                // AND
                self.gpr[rd] = self.gpr[rs] & self.gpr[rt];
                self.cycles += 1;
            }
            0x25 => {
                // OR
                self.gpr[rd] = self.gpr[rs] | self.gpr[rt];
                self.cycles += 1;
            }
            0x26 => {
                // XOR
                self.gpr[rd] = self.gpr[rs] ^ self.gpr[rt];
                self.cycles += 1;
            }
            0x27 => {
                // NOR
                self.gpr[rd] = !(self.gpr[rs] | self.gpr[rt]);
                self.cycles += 1;
            }
            0x2A => {
                // SLT - Set on Less Than
                self.gpr[rd] = if (self.gpr[rs] as i64) < (self.gpr[rt] as i64) {
                    1
                } else {
                    0
                };
                self.cycles += 1;
            }
            0x2B => {
                // SLTU - Set on Less Than Unsigned
                self.gpr[rd] = if self.gpr[rs] < self.gpr[rt] { 1 } else { 0 };
                self.cycles += 1;
            }
            0x2C => {
                // DADD - Doubleword Add (with overflow trap)
                let a = self.gpr[rs] as i64;
                let b = self.gpr[rt] as i64;
                // For now, we don't implement traps, just perform the addition
                self.gpr[rd] = a.wrapping_add(b) as u64;
                self.cycles += 1;
            }
            0x2D => {
                // DADDU - Doubleword Add Unsigned
                self.gpr[rd] = self.gpr[rs].wrapping_add(self.gpr[rt]);
                self.cycles += 1;
            }
            0x2E => {
                // DSUB - Doubleword Subtract (with overflow trap)
                let a = self.gpr[rs] as i64;
                let b = self.gpr[rt] as i64;
                // For now, we don't implement traps, just perform the subtraction
                self.gpr[rd] = a.wrapping_sub(b) as u64;
                self.cycles += 1;
            }
            0x2F => {
                // DSUBU - Doubleword Subtract Unsigned
                self.gpr[rd] = self.gpr[rs].wrapping_sub(self.gpr[rt]);
                self.cycles += 1;
            }
            0x38 => {
                // DSLL - Doubleword Shift Left Logical
                self.gpr[rd] = self.gpr[rt].wrapping_shl(shamt);
                self.cycles += 1;
            }
            0x3A => {
                // DSRL - Doubleword Shift Right Logical
                self.gpr[rd] = self.gpr[rt].wrapping_shr(shamt);
                self.cycles += 1;
            }
            0x3B => {
                // DSRA - Doubleword Shift Right Arithmetic
                self.gpr[rd] = ((self.gpr[rt] as i64) >> shamt) as u64;
                self.cycles += 1;
            }
            0x3C => {
                // DSLL32 - Doubleword Shift Left Logical + 32
                self.gpr[rd] = self.gpr[rt].wrapping_shl(shamt + 32);
                self.cycles += 1;
            }
            0x3E => {
                // DSRL32 - Doubleword Shift Right Logical + 32
                self.gpr[rd] = self.gpr[rt].wrapping_shr(shamt + 32);
                self.cycles += 1;
            }
            0x3F => {
                // DSRA32 - Doubleword Shift Right Arithmetic + 32
                self.gpr[rd] = ((self.gpr[rt] as i64) >> (shamt + 32)) as u64;
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

    // ============================================================================
    // I-Type Instructions
    // ============================================================================

    /// Execute REGIMM (opcode 0x01) - Branch instructions
    fn execute_regimm(&mut self, instr: u32) {
        let rs = ((instr >> 21) & 0x1F) as usize;
        let rt = (instr >> 16) & 0x1F; // This is the regimm field
        let offset = ((instr & 0xFFFF) as i16 as i32) << 2;

        match rt {
            0x00 => {
                // BLTZ - Branch on Less Than Zero
                if (self.gpr[rs] as i64) < 0 {
                    self.pc = (self.pc as i64).wrapping_add(offset as i64) as u64;
                }
                self.cycles += 1;
            }
            0x01 => {
                // BGEZ - Branch on Greater Than or Equal to Zero
                if (self.gpr[rs] as i64) >= 0 {
                    self.pc = (self.pc as i64).wrapping_add(offset as i64) as u64;
                }
                self.cycles += 1;
            }
            0x02 => {
                // BLTZL - Branch on Less Than Zero Likely
                if (self.gpr[rs] as i64) < 0 {
                    self.pc = (self.pc as i64).wrapping_add(offset as i64) as u64;
                }
                self.cycles += 1;
            }
            0x03 => {
                // BGEZL - Branch on Greater Than or Equal to Zero Likely
                if (self.gpr[rs] as i64) >= 0 {
                    self.pc = (self.pc as i64).wrapping_add(offset as i64) as u64;
                }
                self.cycles += 1;
            }
            0x10 => {
                // BLTZAL - Branch on Less Than Zero And Link
                if (self.gpr[rs] as i64) < 0 {
                    self.gpr[31] = self.pc;
                    self.pc = (self.pc as i64).wrapping_add(offset as i64) as u64;
                }
                self.cycles += 1;
            }
            0x11 => {
                // BGEZAL - Branch on Greater Than or Equal to Zero And Link
                if (self.gpr[rs] as i64) >= 0 {
                    self.gpr[31] = self.pc;
                    self.pc = (self.pc as i64).wrapping_add(offset as i64) as u64;
                }
                self.cycles += 1;
            }
            0x12 => {
                // BLTZALL - Branch on Less Than Zero And Link Likely
                if (self.gpr[rs] as i64) < 0 {
                    self.gpr[31] = self.pc;
                    self.pc = (self.pc as i64).wrapping_add(offset as i64) as u64;
                }
                self.cycles += 1;
            }
            0x13 => {
                // BGEZALL - Branch on Greater Than or Equal to Zero And Link Likely
                if (self.gpr[rs] as i64) >= 0 {
                    self.gpr[31] = self.pc;
                    self.pc = (self.pc as i64).wrapping_add(offset as i64) as u64;
                }
                self.cycles += 1;
            }
            _ => {
                self.cycles += 1;
            }
        }
    }

    /// Execute J - Jump
    fn execute_j(&mut self, instr: u32) {
        let target = instr & 0x03FFFFFF;
        self.pc = (self.pc & 0xFFFFFFFF_F0000000) | ((target << 2) as u64);
        self.cycles += 1;
    }

    /// Execute JAL - Jump And Link
    fn execute_jal(&mut self, instr: u32) {
        let target = instr & 0x03FFFFFF;
        self.gpr[31] = self.pc;
        self.pc = (self.pc & 0xFFFFFFFF_F0000000) | ((target << 2) as u64);
        self.cycles += 1;
    }

    /// Execute BEQ - Branch on Equal
    fn execute_beq(&mut self, instr: u32) {
        let rs = ((instr >> 21) & 0x1F) as usize;
        let rt = ((instr >> 16) & 0x1F) as usize;
        let offset = ((instr & 0xFFFF) as i16 as i32) << 2;

        if self.gpr[rs] == self.gpr[rt] {
            self.pc = (self.pc as i64).wrapping_add(offset as i64) as u64;
        }
        self.cycles += 1;
    }

    /// Execute BNE - Branch on Not Equal
    fn execute_bne(&mut self, instr: u32) {
        let rs = ((instr >> 21) & 0x1F) as usize;
        let rt = ((instr >> 16) & 0x1F) as usize;
        let offset = ((instr & 0xFFFF) as i16 as i32) << 2;

        if self.gpr[rs] != self.gpr[rt] {
            self.pc = (self.pc as i64).wrapping_add(offset as i64) as u64;
        }
        self.cycles += 1;
    }

    /// Execute BLEZ - Branch on Less Than or Equal to Zero
    fn execute_blez(&mut self, instr: u32) {
        let rs = ((instr >> 21) & 0x1F) as usize;
        let offset = ((instr & 0xFFFF) as i16 as i32) << 2;

        if (self.gpr[rs] as i64) <= 0 {
            self.pc = (self.pc as i64).wrapping_add(offset as i64) as u64;
        }
        self.cycles += 1;
    }

    /// Execute BGTZ - Branch on Greater Than Zero
    fn execute_bgtz(&mut self, instr: u32) {
        let rs = ((instr >> 21) & 0x1F) as usize;
        let offset = ((instr & 0xFFFF) as i16 as i32) << 2;

        if (self.gpr[rs] as i64) > 0 {
            self.pc = (self.pc as i64).wrapping_add(offset as i64) as u64;
        }
        self.cycles += 1;
    }

    /// Execute BEQL - Branch on Equal Likely
    fn execute_beql(&mut self, instr: u32) {
        let rs = ((instr >> 21) & 0x1F) as usize;
        let rt = ((instr >> 16) & 0x1F) as usize;
        let offset = ((instr & 0xFFFF) as i16 as i32) << 2;

        if self.gpr[rs] == self.gpr[rt] {
            self.pc = (self.pc as i64).wrapping_add(offset as i64) as u64;
        }
        self.cycles += 1;
    }

    /// Execute BNEL - Branch on Not Equal Likely
    fn execute_bnel(&mut self, instr: u32) {
        let rs = ((instr >> 21) & 0x1F) as usize;
        let rt = ((instr >> 16) & 0x1F) as usize;
        let offset = ((instr & 0xFFFF) as i16 as i32) << 2;

        if self.gpr[rs] != self.gpr[rt] {
            self.pc = (self.pc as i64).wrapping_add(offset as i64) as u64;
        }
        self.cycles += 1;
    }

    /// Execute BLEZL - Branch on Less Than or Equal to Zero Likely
    fn execute_blezl(&mut self, instr: u32) {
        let rs = ((instr >> 21) & 0x1F) as usize;
        let offset = ((instr & 0xFFFF) as i16 as i32) << 2;

        if (self.gpr[rs] as i64) <= 0 {
            self.pc = (self.pc as i64).wrapping_add(offset as i64) as u64;
        }
        self.cycles += 1;
    }

    /// Execute BGTZL - Branch on Greater Than Zero Likely
    fn execute_bgtzl(&mut self, instr: u32) {
        let rs = ((instr >> 21) & 0x1F) as usize;
        let offset = ((instr & 0xFFFF) as i16 as i32) << 2;

        if (self.gpr[rs] as i64) > 0 {
            self.pc = (self.pc as i64).wrapping_add(offset as i64) as u64;
        }
        self.cycles += 1;
    }

    /// Execute ADDI - Add Immediate
    fn execute_addi(&mut self, instr: u32) {
        let rs = ((instr >> 21) & 0x1F) as usize;
        let rt = ((instr >> 16) & 0x1F) as usize;
        let imm = (instr & 0xFFFF) as i16 as i32;

        // For now, we don't implement traps
        self.gpr[rt] = (self.gpr[rs] as i32).wrapping_add(imm) as u64;
        self.cycles += 1;
    }

    /// Execute ADDIU - Add Immediate Unsigned
    fn execute_addiu(&mut self, instr: u32) {
        let rs = ((instr >> 21) & 0x1F) as usize;
        let rt = ((instr >> 16) & 0x1F) as usize;
        let imm = (instr & 0xFFFF) as i16 as i32;

        self.gpr[rt] = (self.gpr[rs] as i32).wrapping_add(imm) as u64;
        self.cycles += 1;
    }

    /// Execute SLTI - Set on Less Than Immediate
    fn execute_slti(&mut self, instr: u32) {
        let rs = ((instr >> 21) & 0x1F) as usize;
        let rt = ((instr >> 16) & 0x1F) as usize;
        let imm = (instr & 0xFFFF) as i16 as i64;

        self.gpr[rt] = if (self.gpr[rs] as i64) < imm { 1 } else { 0 };
        self.cycles += 1;
    }

    /// Execute SLTIU - Set on Less Than Immediate Unsigned
    fn execute_sltiu(&mut self, instr: u32) {
        let rs = ((instr >> 21) & 0x1F) as usize;
        let rt = ((instr >> 16) & 0x1F) as usize;
        let imm = (instr & 0xFFFF) as i16 as i64 as u64;

        self.gpr[rt] = if self.gpr[rs] < imm { 1 } else { 0 };
        self.cycles += 1;
    }

    /// Execute ANDI - AND Immediate
    fn execute_andi(&mut self, instr: u32) {
        let rs = ((instr >> 21) & 0x1F) as usize;
        let rt = ((instr >> 16) & 0x1F) as usize;
        let imm = (instr & 0xFFFF) as u64;

        self.gpr[rt] = self.gpr[rs] & imm;
        self.cycles += 1;
    }

    /// Execute XORI - XOR Immediate
    fn execute_xori(&mut self, instr: u32) {
        let rs = ((instr >> 21) & 0x1F) as usize;
        let rt = ((instr >> 16) & 0x1F) as usize;
        let imm = (instr & 0xFFFF) as u64;

        self.gpr[rt] = self.gpr[rs] ^ imm;
        self.cycles += 1;
    }

    /// Execute DADDI - Doubleword Add Immediate
    fn execute_daddi(&mut self, instr: u32) {
        let rs = ((instr >> 21) & 0x1F) as usize;
        let rt = ((instr >> 16) & 0x1F) as usize;
        let imm = (instr & 0xFFFF) as i16 as i64;

        // For now, we don't implement traps
        self.gpr[rt] = (self.gpr[rs] as i64).wrapping_add(imm) as u64;
        self.cycles += 1;
    }

    /// Execute DADDIU - Doubleword Add Immediate Unsigned
    fn execute_daddiu(&mut self, instr: u32) {
        let rs = ((instr >> 21) & 0x1F) as usize;
        let rt = ((instr >> 16) & 0x1F) as usize;
        let imm = (instr & 0xFFFF) as i16 as i64;

        self.gpr[rt] = (self.gpr[rs] as i64).wrapping_add(imm) as u64;
        self.cycles += 1;
    }

    // ============================================================================
    // Load/Store Instructions
    // ============================================================================

    /// Execute LB - Load Byte
    fn execute_lb(&mut self, instr: u32) {
        let rs = ((instr >> 21) & 0x1F) as usize;
        let rt = ((instr >> 16) & 0x1F) as usize;
        let offset = (instr & 0xFFFF) as i16 as i32;

        let addr = (self.gpr[rs] as i64).wrapping_add(offset as i64) as u32;
        let val = self.memory.read_byte(addr);
        self.gpr[rt] = val as i8 as i64 as u64; // Sign-extend to 64-bit
        self.cycles += 1;
    }

    /// Execute LBU - Load Byte Unsigned
    fn execute_lbu(&mut self, instr: u32) {
        let rs = ((instr >> 21) & 0x1F) as usize;
        let rt = ((instr >> 16) & 0x1F) as usize;
        let offset = (instr & 0xFFFF) as i16 as i32;

        let addr = (self.gpr[rs] as i64).wrapping_add(offset as i64) as u32;
        let val = self.memory.read_byte(addr);
        self.gpr[rt] = val as u64; // Zero-extend to 64-bit
        self.cycles += 1;
    }

    /// Execute LH - Load Halfword
    fn execute_lh(&mut self, instr: u32) {
        let rs = ((instr >> 21) & 0x1F) as usize;
        let rt = ((instr >> 16) & 0x1F) as usize;
        let offset = (instr & 0xFFFF) as i16 as i32;

        let addr = (self.gpr[rs] as i64).wrapping_add(offset as i64) as u32;
        let val = self.memory.read_halfword(addr);
        self.gpr[rt] = val as i16 as i64 as u64; // Sign-extend to 64-bit
        self.cycles += 1;
    }

    /// Execute LHU - Load Halfword Unsigned
    fn execute_lhu(&mut self, instr: u32) {
        let rs = ((instr >> 21) & 0x1F) as usize;
        let rt = ((instr >> 16) & 0x1F) as usize;
        let offset = (instr & 0xFFFF) as i16 as i32;

        let addr = (self.gpr[rs] as i64).wrapping_add(offset as i64) as u32;
        let val = self.memory.read_halfword(addr);
        self.gpr[rt] = val as u64; // Zero-extend to 64-bit
        self.cycles += 1;
    }

    /// Execute LWU - Load Word Unsigned
    fn execute_lwu(&mut self, instr: u32) {
        let rs = ((instr >> 21) & 0x1F) as usize;
        let rt = ((instr >> 16) & 0x1F) as usize;
        let offset = (instr & 0xFFFF) as i16 as i32;

        let addr = (self.gpr[rs] as i64).wrapping_add(offset as i64) as u32;
        let val = self.memory.read_word(addr);
        self.gpr[rt] = val as u64; // Zero-extend to 64-bit
        self.cycles += 1;
    }

    /// Execute LD - Load Doubleword
    fn execute_ld(&mut self, instr: u32) {
        let rs = ((instr >> 21) & 0x1F) as usize;
        let rt = ((instr >> 16) & 0x1F) as usize;
        let offset = (instr & 0xFFFF) as i16 as i32;

        let addr = (self.gpr[rs] as i64).wrapping_add(offset as i64) as u32;
        let val = self.memory.read_doubleword(addr);
        self.gpr[rt] = val;
        self.cycles += 1;
    }

    /// Execute LWL - Load Word Left
    fn execute_lwl(&mut self, instr: u32) {
        let rs = ((instr >> 21) & 0x1F) as usize;
        let rt = ((instr >> 16) & 0x1F) as usize;
        let offset = (instr & 0xFFFF) as i16 as i32;

        let addr = (self.gpr[rs] as i64).wrapping_add(offset as i64) as u32;
        let aligned_addr = addr & !3;
        let byte_offset = addr & 3;
        let word = self.memory.read_word(aligned_addr);

        let shift = (3 - byte_offset) * 8;
        let mask = u32::MAX << shift;
        let current = self.gpr[rt] as u32;
        let result = (current & !mask) | (word << shift);
        self.gpr[rt] = result as i32 as u64;
        self.cycles += 1;
    }

    /// Execute LWR - Load Word Right
    fn execute_lwr(&mut self, instr: u32) {
        let rs = ((instr >> 21) & 0x1F) as usize;
        let rt = ((instr >> 16) & 0x1F) as usize;
        let offset = (instr & 0xFFFF) as i16 as i32;

        let addr = (self.gpr[rs] as i64).wrapping_add(offset as i64) as u32;
        let aligned_addr = addr & !3;
        let byte_offset = addr & 3;
        let word = self.memory.read_word(aligned_addr);

        let shift = byte_offset * 8;
        let mask = u32::MAX >> shift;
        let current = self.gpr[rt] as u32;
        let result = (current & !mask) | (word >> shift);
        self.gpr[rt] = result as i32 as u64;
        self.cycles += 1;
    }

    /// Execute LDL - Load Doubleword Left
    fn execute_ldl(&mut self, instr: u32) {
        let rs = ((instr >> 21) & 0x1F) as usize;
        let rt = ((instr >> 16) & 0x1F) as usize;
        let offset = (instr & 0xFFFF) as i16 as i32;

        let addr = (self.gpr[rs] as i64).wrapping_add(offset as i64) as u32;
        let aligned_addr = addr & !7;
        let byte_offset = addr & 7;
        let dword = self.memory.read_doubleword(aligned_addr);

        let shift = (7 - byte_offset) * 8;
        let mask = u64::MAX << shift;
        let current = self.gpr[rt];
        let result = (current & !mask) | (dword << shift);
        self.gpr[rt] = result;
        self.cycles += 1;
    }

    /// Execute LDR - Load Doubleword Right
    fn execute_ldr(&mut self, instr: u32) {
        let rs = ((instr >> 21) & 0x1F) as usize;
        let rt = ((instr >> 16) & 0x1F) as usize;
        let offset = (instr & 0xFFFF) as i16 as i32;

        let addr = (self.gpr[rs] as i64).wrapping_add(offset as i64) as u32;
        let aligned_addr = addr & !7;
        let byte_offset = addr & 7;
        let dword = self.memory.read_doubleword(aligned_addr);

        let shift = byte_offset * 8;
        let mask = u64::MAX >> shift;
        let current = self.gpr[rt];
        let result = (current & !mask) | (dword >> shift);
        self.gpr[rt] = result;
        self.cycles += 1;
    }

    /// Execute SB - Store Byte
    fn execute_sb(&mut self, instr: u32) {
        let rs = ((instr >> 21) & 0x1F) as usize;
        let rt = ((instr >> 16) & 0x1F) as usize;
        let offset = (instr & 0xFFFF) as i16 as i32;

        let addr = (self.gpr[rs] as i64).wrapping_add(offset as i64) as u32;
        self.memory.write_byte(addr, self.gpr[rt] as u8);
        self.cycles += 1;
    }

    /// Execute SH - Store Halfword
    fn execute_sh(&mut self, instr: u32) {
        let rs = ((instr >> 21) & 0x1F) as usize;
        let rt = ((instr >> 16) & 0x1F) as usize;
        let offset = (instr & 0xFFFF) as i16 as i32;

        let addr = (self.gpr[rs] as i64).wrapping_add(offset as i64) as u32;
        self.memory.write_halfword(addr, self.gpr[rt] as u16);
        self.cycles += 1;
    }

    /// Execute SD - Store Doubleword
    fn execute_sd(&mut self, instr: u32) {
        let rs = ((instr >> 21) & 0x1F) as usize;
        let rt = ((instr >> 16) & 0x1F) as usize;
        let offset = (instr & 0xFFFF) as i16 as i32;

        let addr = (self.gpr[rs] as i64).wrapping_add(offset as i64) as u32;
        self.memory.write_doubleword(addr, self.gpr[rt]);
        self.cycles += 1;
    }

    /// Execute SWL - Store Word Left
    fn execute_swl(&mut self, instr: u32) {
        let rs = ((instr >> 21) & 0x1F) as usize;
        let rt = ((instr >> 16) & 0x1F) as usize;
        let offset = (instr & 0xFFFF) as i16 as i32;

        let addr = (self.gpr[rs] as i64).wrapping_add(offset as i64) as u32;
        let aligned_addr = addr & !3;
        let byte_offset = addr & 3;
        let word = self.memory.read_word(aligned_addr);

        let shift = (3 - byte_offset) * 8;
        let mask = u32::MAX >> shift;
        let val = self.gpr[rt] as u32;
        let result = (word & !mask) | (val >> shift);
        self.memory.write_word(aligned_addr, result);
        self.cycles += 1;
    }

    /// Execute SWR - Store Word Right
    fn execute_swr(&mut self, instr: u32) {
        let rs = ((instr >> 21) & 0x1F) as usize;
        let rt = ((instr >> 16) & 0x1F) as usize;
        let offset = (instr & 0xFFFF) as i16 as i32;

        let addr = (self.gpr[rs] as i64).wrapping_add(offset as i64) as u32;
        let aligned_addr = addr & !3;
        let byte_offset = addr & 3;
        let word = self.memory.read_word(aligned_addr);

        let shift = byte_offset * 8;
        let mask = u32::MAX << shift;
        let val = self.gpr[rt] as u32;
        let result = (word & !mask) | (val << shift);
        self.memory.write_word(aligned_addr, result);
        self.cycles += 1;
    }

    /// Execute SDL - Store Doubleword Left
    fn execute_sdl(&mut self, instr: u32) {
        let rs = ((instr >> 21) & 0x1F) as usize;
        let rt = ((instr >> 16) & 0x1F) as usize;
        let offset = (instr & 0xFFFF) as i16 as i32;

        let addr = (self.gpr[rs] as i64).wrapping_add(offset as i64) as u32;
        let aligned_addr = addr & !7;
        let byte_offset = addr & 7;
        let dword = self.memory.read_doubleword(aligned_addr);

        let shift = (7 - byte_offset) * 8;
        let mask = u64::MAX >> shift;
        let val = self.gpr[rt];
        let result = (dword & !mask) | (val >> shift);
        self.memory.write_doubleword(aligned_addr, result);
        self.cycles += 1;
    }

    /// Execute SDR - Store Doubleword Right
    fn execute_sdr(&mut self, instr: u32) {
        let rs = ((instr >> 21) & 0x1F) as usize;
        let rt = ((instr >> 16) & 0x1F) as usize;
        let offset = (instr & 0xFFFF) as i16 as i32;

        let addr = (self.gpr[rs] as i64).wrapping_add(offset as i64) as u32;
        let aligned_addr = addr & !7;
        let byte_offset = addr & 7;
        let dword = self.memory.read_doubleword(aligned_addr);

        let shift = byte_offset * 8;
        let mask = u64::MAX << shift;
        let val = self.gpr[rt];
        let result = (dword & !mask) | (val << shift);
        self.memory.write_doubleword(aligned_addr, result);
        self.cycles += 1;
    }

    /// Execute CACHE - Cache operation (NOP for now)
    fn execute_cache(&mut self, _instr: u32) {
        // Cache operations are implementation-specific
        // For basic emulation, we can treat this as a NOP
        self.cycles += 1;
    }

    // ============================================================================
    // Coprocessor Instructions
    // ============================================================================

    /// Execute COP0 (Coprocessor 0) instructions
    fn execute_cop0(&mut self, instr: u32) {
        let rs = (instr >> 21) & 0x1F;
        let rt = ((instr >> 16) & 0x1F) as usize;
        let rd = ((instr >> 11) & 0x1F) as usize;

        match rs {
            0x00 => {
                // MFC0 - Move From CP0
                self.gpr[rt] = self.cp0[rd] as i32 as u64; // Sign-extend
                self.cycles += 1;
            }
            0x04 => {
                // MTC0 - Move To CP0
                self.cp0[rd] = self.gpr[rt];
                self.cycles += 1;
            }
            0x10 => {
                // COP0 function
                let funct = instr & 0x3F;
                match funct {
                    0x01 => {
                        // TLBR - Read Indexed TLB Entry (NOP for now)
                        self.cycles += 1;
                    }
                    0x02 => {
                        // TLBWI - Write Indexed TLB Entry (NOP for now)
                        self.cycles += 1;
                    }
                    0x06 => {
                        // TLBWR - Write Random TLB Entry (NOP for now)
                        self.cycles += 1;
                    }
                    0x08 => {
                        // TLBP - Probe TLB for Matching Entry (NOP for now)
                        self.cycles += 1;
                    }
                    0x18 => {
                        // ERET - Exception Return (basic implementation)
                        self.pc = self.cp0[CP0_EPC];
                        self.cycles += 1;
                    }
                    _ => {
                        self.cycles += 1;
                    }
                }
            }
            _ => {
                self.cycles += 1;
            }
        }
    }

    /// Execute COP1 (Coprocessor 1 - FPU) instructions
    fn execute_cop1(&mut self, instr: u32) {
        let rs = (instr >> 21) & 0x1F;
        let rt = ((instr >> 16) & 0x1F) as usize;
        let fs = ((instr >> 11) & 0x1F) as usize;
        let ft = ((instr >> 16) & 0x1F) as usize;
        let fd = ((instr >> 6) & 0x1F) as usize;
        let funct = instr & 0x3F;

        match rs {
            0x00 => {
                // MFC1 - Move From FPU
                self.gpr[rt] = self.fpr[fs].to_bits() as i32 as u64; // Sign-extend
                self.cycles += 1;
            }
            0x01 => {
                // DMFC1 - Doubleword Move From FPU
                self.gpr[rt] = self.fpr[fs].to_bits();
                self.cycles += 1;
            }
            0x02 => {
                // CFC1 - Move Control From FPU
                if fs == 31 {
                    self.gpr[rt] = self.fcr31 as i32 as u64; // Sign-extend
                }
                self.cycles += 1;
            }
            0x04 => {
                // MTC1 - Move To FPU
                self.fpr[fs] = f64::from_bits((self.gpr[rt] as u32) as u64);
                self.cycles += 1;
            }
            0x05 => {
                // DMTC1 - Doubleword Move To FPU
                self.fpr[fs] = f64::from_bits(self.gpr[rt]);
                self.cycles += 1;
            }
            0x06 => {
                // CTC1 - Move Control To FPU
                if fs == 31 {
                    self.fcr31 = self.gpr[rt] as u32;
                }
                self.cycles += 1;
            }
            0x08 => {
                // BC1 - Branch on FPU condition
                let cc = (instr >> 18) & 0x7;
                let _nd = (instr >> 17) & 0x1; // Nullify delay slot (not implemented)
                let tf = (instr >> 16) & 0x1;
                let offset = ((instr & 0xFFFF) as i16 as i32) << 2;
                let condition = (self.fcr31 >> (23 + cc)) & 0x1;

                if condition == tf {
                    self.pc = (self.pc as i64).wrapping_add(offset as i64) as u64;
                }
                self.cycles += 1;
            }
            0x10 | 0x11 => {
                // FPU operations (fmt = 0x10 for single, 0x11 for double)
                match funct {
                    0x00 => {
                        // ADD.fmt
                        self.fpr[fd] = self.fpr[fs] + self.fpr[ft];
                        self.cycles += 1;
                    }
                    0x01 => {
                        // SUB.fmt
                        self.fpr[fd] = self.fpr[fs] - self.fpr[ft];
                        self.cycles += 1;
                    }
                    0x02 => {
                        // MUL.fmt
                        self.fpr[fd] = self.fpr[fs] * self.fpr[ft];
                        self.cycles += 1;
                    }
                    0x03 => {
                        // DIV.fmt
                        self.fpr[fd] = self.fpr[fs] / self.fpr[ft];
                        self.cycles += 1;
                    }
                    0x04 => {
                        // SQRT.fmt
                        self.fpr[fd] = self.fpr[fs].sqrt();
                        self.cycles += 1;
                    }
                    0x05 => {
                        // ABS.fmt
                        self.fpr[fd] = self.fpr[fs].abs();
                        self.cycles += 1;
                    }
                    0x06 => {
                        // MOV.fmt
                        self.fpr[fd] = self.fpr[fs];
                        self.cycles += 1;
                    }
                    0x07 => {
                        // NEG.fmt
                        self.fpr[fd] = -self.fpr[fs];
                        self.cycles += 1;
                    }
                    0x20 => {
                        // CVT.S - Convert to Single
                        self.fpr[fd] = self.fpr[fs]; // Already f64
                        self.cycles += 1;
                    }
                    0x21 => {
                        // CVT.D - Convert to Double
                        self.fpr[fd] = self.fpr[fs]; // Already f64
                        self.cycles += 1;
                    }
                    0x24 => {
                        // CVT.W - Convert to Word
                        self.fpr[fd] = f64::from_bits(self.fpr[fs].round() as i32 as u64);
                        self.cycles += 1;
                    }
                    0x25 => {
                        // CVT.L - Convert to Long
                        self.fpr[fd] = f64::from_bits(self.fpr[fs].round() as i64 as u64);
                        self.cycles += 1;
                    }
                    0x30..=0x3F => {
                        // C.cond.fmt - FPU Compare
                        let cond = funct & 0x0F;
                        let result = match cond {
                            0x00 => false,                        // C.F (always false)
                            0x01 => false,                        // C.UN (unordered)
                            0x02 => self.fpr[fs] == self.fpr[ft], // C.EQ
                            0x03 => self.fpr[fs] == self.fpr[ft], // C.UEQ
                            0x04 => self.fpr[fs] < self.fpr[ft],  // C.OLT
                            0x05 => self.fpr[fs] < self.fpr[ft],  // C.ULT
                            0x06 => self.fpr[fs] <= self.fpr[ft], // C.OLE
                            0x07 => self.fpr[fs] <= self.fpr[ft], // C.ULE
                            _ => false,
                        };
                        if result {
                            self.fcr31 |= 1 << 23;
                        } else {
                            self.fcr31 &= !(1 << 23);
                        }
                        self.cycles += 1;
                    }
                    _ => {
                        self.cycles += 1;
                    }
                }
            }
            _ => {
                self.cycles += 1;
            }
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

    // ============================================================================
    // R-Type Instruction Tests
    // ============================================================================

    #[test]
    fn test_srl() {
        let mem = ArrayMemory::new();
        let mut cpu = CpuMips::new(mem);
        cpu.pc = 0;
        cpu.gpr[2] = 20;
        // SRL $2, $2, 2
        cpu.memory.write_word(0, 0x00021082);
        cpu.step();
        assert_eq!(cpu.gpr[2], 5);
    }

    #[test]
    fn test_sra() {
        let mem = ArrayMemory::new();
        let mut cpu = CpuMips::new(mem);
        cpu.pc = 0;
        cpu.gpr[2] = 0xFFFF_FFFF_FFFF_FFF0_u64; // Negative number
                                                // SRA $2, $2, 2
        cpu.memory.write_word(0, 0x00021083);
        cpu.step();
        assert_eq!(cpu.gpr[2] as i32, -4);
    }

    #[test]
    fn test_sllv() {
        let mem = ArrayMemory::new();
        let mut cpu = CpuMips::new(mem);
        cpu.pc = 0;
        cpu.gpr[1] = 2;
        cpu.gpr[2] = 5;
        // SLLV $3, $2, $1
        cpu.memory.write_word(0, 0x00221804);
        cpu.step();
        assert_eq!(cpu.gpr[3], 20);
    }

    #[test]
    fn test_jr() {
        let mem = ArrayMemory::new();
        let mut cpu = CpuMips::new(mem);
        cpu.pc = 0;
        cpu.gpr[1] = 0x1000;
        // JR $1
        cpu.memory.write_word(0, 0x00200008);
        cpu.step();
        assert_eq!(cpu.pc, 0x1000);
    }

    #[test]
    fn test_jalr() {
        let mem = ArrayMemory::new();
        let mut cpu = CpuMips::new(mem);
        cpu.pc = 0;
        cpu.gpr[1] = 0x1000;
        // JALR $31, $1
        cpu.memory.write_word(0, 0x0020F809);
        cpu.step();
        assert_eq!(cpu.pc, 0x1000);
        assert_eq!(cpu.gpr[31], 4);
    }

    #[test]
    fn test_mult() {
        let mem = ArrayMemory::new();
        let mut cpu = CpuMips::new(mem);
        cpu.pc = 0;
        cpu.gpr[1] = 100;
        cpu.gpr[2] = 200;
        // MULT $1, $2
        cpu.memory.write_word(0, 0x00220018);
        cpu.step();
        assert_eq!(cpu.lo, 20000);
    }

    #[test]
    fn test_multu() {
        let mem = ArrayMemory::new();
        let mut cpu = CpuMips::new(mem);
        cpu.pc = 0;
        cpu.gpr[1] = 0xFFFFFFFF; // Max u32
        cpu.gpr[2] = 2;
        // MULTU $1, $2
        cpu.memory.write_word(0, 0x00220019);
        cpu.step();
        assert_eq!(cpu.lo as u32, 0xFFFFFFFE);
        assert_eq!(cpu.hi as u32, 1);
    }

    #[test]
    fn test_div() {
        let mem = ArrayMemory::new();
        let mut cpu = CpuMips::new(mem);
        cpu.pc = 0;
        cpu.gpr[1] = 100;
        cpu.gpr[2] = 7;
        // DIV $1, $2
        cpu.memory.write_word(0, 0x0022001A);
        cpu.step();
        assert_eq!(cpu.lo as i32, 14);
        assert_eq!(cpu.hi as i32, 2);
    }

    #[test]
    fn test_divu() {
        let mem = ArrayMemory::new();
        let mut cpu = CpuMips::new(mem);
        cpu.pc = 0;
        cpu.gpr[1] = 100;
        cpu.gpr[2] = 7;
        // DIVU $1, $2
        cpu.memory.write_word(0, 0x0022001B);
        cpu.step();
        assert_eq!(cpu.lo as u32, 14);
        assert_eq!(cpu.hi as u32, 2);
    }

    #[test]
    fn test_mfhi_mthi() {
        let mem = ArrayMemory::new();
        let mut cpu = CpuMips::new(mem);
        cpu.pc = 0;
        cpu.gpr[1] = 0x12345678;
        // MTHI $1
        cpu.memory.write_word(0, 0x00200011);
        cpu.step();
        assert_eq!(cpu.hi, 0x12345678);

        // MFHI $2
        cpu.memory.write_word(4, 0x00001010);
        cpu.step();
        assert_eq!(cpu.gpr[2], 0x12345678);
    }

    #[test]
    fn test_mflo_mtlo() {
        let mem = ArrayMemory::new();
        let mut cpu = CpuMips::new(mem);
        cpu.pc = 0;
        cpu.gpr[1] = 0x12345678;
        // MTLO $1
        cpu.memory.write_word(0, 0x00200013);
        cpu.step();
        assert_eq!(cpu.lo, 0x12345678);

        // MFLO $2
        cpu.memory.write_word(4, 0x00001012);
        cpu.step();
        assert_eq!(cpu.gpr[2], 0x12345678);
    }

    #[test]
    fn test_add_sub() {
        let mem = ArrayMemory::new();
        let mut cpu = CpuMips::new(mem);
        cpu.pc = 0;
        cpu.gpr[1] = 10;
        cpu.gpr[2] = 20;
        // ADD $3, $1, $2
        cpu.memory.write_word(0, 0x00221820);
        cpu.step();
        assert_eq!(cpu.gpr[3], 30);

        cpu.gpr[1] = 50;
        cpu.gpr[2] = 20;
        // SUB $3, $1, $2
        cpu.memory.write_word(4, 0x00221822);
        cpu.step();
        assert_eq!(cpu.gpr[3], 30);
    }

    #[test]
    fn test_subu() {
        let mem = ArrayMemory::new();
        let mut cpu = CpuMips::new(mem);
        cpu.pc = 0;
        cpu.gpr[1] = 50;
        cpu.gpr[2] = 20;
        // SUBU $3, $1, $2
        cpu.memory.write_word(0, 0x00221823);
        cpu.step();
        assert_eq!(cpu.gpr[3], 30);
    }

    #[test]
    fn test_and_xor_nor() {
        let mem = ArrayMemory::new();
        let mut cpu = CpuMips::new(mem);
        cpu.pc = 0;
        cpu.gpr[1] = 0xFF00;
        cpu.gpr[2] = 0x0FF0;

        // AND $3, $1, $2
        cpu.memory.write_word(0, 0x00221824);
        cpu.step();
        assert_eq!(cpu.gpr[3], 0x0F00);

        // XOR $3, $1, $2
        cpu.memory.write_word(4, 0x00221826);
        cpu.step();
        assert_eq!(cpu.gpr[3], 0xF0F0);

        // NOR $3, $1, $2
        cpu.memory.write_word(8, 0x00221827);
        cpu.step();
        assert_eq!(cpu.gpr[3], 0xFFFFFFFF_FFFF000F);
    }

    #[test]
    fn test_slt_sltu() {
        let mem = ArrayMemory::new();
        let mut cpu = CpuMips::new(mem);
        cpu.pc = 0;
        cpu.gpr[1] = 10;
        cpu.gpr[2] = 20;

        // SLT $3, $1, $2
        cpu.memory.write_word(0, 0x0022182A);
        cpu.step();
        assert_eq!(cpu.gpr[3], 1);

        // SLT $3, $2, $1
        cpu.memory.write_word(4, 0x0041182A);
        cpu.step();
        assert_eq!(cpu.gpr[3], 0);

        // SLTU $3, $1, $2
        cpu.memory.write_word(8, 0x0022182B);
        cpu.step();
        assert_eq!(cpu.gpr[3], 1);
    }

    // ============================================================================
    // 64-bit Instruction Tests
    // ============================================================================

    #[test]
    fn test_dadd_daddu() {
        let mem = ArrayMemory::new();
        let mut cpu = CpuMips::new(mem);
        cpu.pc = 0;
        cpu.gpr[1] = 0x1234567890ABCDEF;
        cpu.gpr[2] = 0x1111111111111111;

        // DADDU $3, $1, $2
        cpu.memory.write_word(0, 0x0022182D);
        cpu.step();
        assert_eq!(cpu.gpr[3], 0x23456789A1BCDF00);
    }

    #[test]
    fn test_dsub_dsubu() {
        let mem = ArrayMemory::new();
        let mut cpu = CpuMips::new(mem);
        cpu.pc = 0;
        cpu.gpr[1] = 0x1234567890ABCDEF;
        cpu.gpr[2] = 0x1111111111111111;

        // DSUBU $3, $1, $2
        cpu.memory.write_word(0, 0x0022182F);
        cpu.step();
        assert_eq!(cpu.gpr[3], 0x012345677F9ABCDE);
    }

    #[test]
    fn test_dmult() {
        let mem = ArrayMemory::new();
        let mut cpu = CpuMips::new(mem);
        cpu.pc = 0;
        cpu.gpr[1] = 1000000;
        cpu.gpr[2] = 2000000;

        // DMULT $1, $2
        cpu.memory.write_word(0, 0x0022001C);
        cpu.step();
        assert_eq!(cpu.lo, 2000000000000);
    }

    #[test]
    fn test_dmultu() {
        let mem = ArrayMemory::new();
        let mut cpu = CpuMips::new(mem);
        cpu.pc = 0;
        cpu.gpr[1] = 1000000;
        cpu.gpr[2] = 2000000;

        // DMULTU $1, $2
        cpu.memory.write_word(0, 0x0022001D);
        cpu.step();
        assert_eq!(cpu.lo, 2000000000000);
    }

    #[test]
    fn test_ddiv() {
        let mem = ArrayMemory::new();
        let mut cpu = CpuMips::new(mem);
        cpu.pc = 0;
        cpu.gpr[1] = 1000;
        cpu.gpr[2] = 7;

        // DDIV $1, $2
        cpu.memory.write_word(0, 0x0022001E);
        cpu.step();
        assert_eq!(cpu.lo, 142);
        assert_eq!(cpu.hi, 6);
    }

    #[test]
    fn test_ddivu() {
        let mem = ArrayMemory::new();
        let mut cpu = CpuMips::new(mem);
        cpu.pc = 0;
        cpu.gpr[1] = 1000;
        cpu.gpr[2] = 7;

        // DDIVU $1, $2
        cpu.memory.write_word(0, 0x0022001F);
        cpu.step();
        assert_eq!(cpu.lo, 142);
        assert_eq!(cpu.hi, 6);
    }

    #[test]
    fn test_dsll() {
        let mem = ArrayMemory::new();
        let mut cpu = CpuMips::new(mem);
        cpu.pc = 0;
        cpu.gpr[2] = 5;

        // DSLL $2, $2, 2 (opcode=0, rt=2, rd=2, shamt=2, funct=0x38)
        cpu.memory.write_word(0, 0x000210B8);
        cpu.step();
        assert_eq!(cpu.gpr[2], 20);
    }

    #[test]
    fn test_dsrl() {
        let mem = ArrayMemory::new();
        let mut cpu = CpuMips::new(mem);
        cpu.pc = 0;
        cpu.gpr[2] = 20;

        // DSRL $2, $2, 2 (opcode=0, rt=2, rd=2, shamt=2, funct=0x3A)
        cpu.memory.write_word(0, 0x000210BA);
        cpu.step();
        assert_eq!(cpu.gpr[2], 5);
    }

    #[test]
    fn test_dsra() {
        let mem = ArrayMemory::new();
        let mut cpu = CpuMips::new(mem);
        cpu.pc = 0;
        cpu.gpr[2] = 0xFFFFFFFFFFFFFFF0_u64; // -16

        // DSRA $2, $2, 2 (opcode=0, rt=2, rd=2, shamt=2, funct=0x3B)
        cpu.memory.write_word(0, 0x000210BB);
        cpu.step();
        assert_eq!(cpu.gpr[2] as i64, -4);
    }

    #[test]
    fn test_dsll32() {
        let mem = ArrayMemory::new();
        let mut cpu = CpuMips::new(mem);
        cpu.pc = 0;
        cpu.gpr[2] = 1;

        // DSLL32 $2, $2, 0 (shift by 32)
        cpu.memory.write_word(0, 0x0002103C);
        cpu.step();
        assert_eq!(cpu.gpr[2], 0x100000000);
    }

    // ============================================================================
    // I-Type Instruction Tests
    // ============================================================================

    #[test]
    fn test_beq_bne() {
        let mem = ArrayMemory::new();
        let mut cpu = CpuMips::new(mem);
        cpu.pc = 0;
        cpu.gpr[1] = 10;
        cpu.gpr[2] = 10;

        // BEQ $1, $2, offset=8
        cpu.memory.write_word(0, 0x10220002);
        cpu.step();
        assert_eq!(cpu.pc, 12); // 4 + 8

        cpu.pc = 0;
        cpu.gpr[2] = 20;
        // BNE $1, $2, offset=8
        cpu.memory.write_word(0, 0x14220002);
        cpu.step();
        assert_eq!(cpu.pc, 12); // 4 + 8
    }

    #[test]
    fn test_blez_bgtz() {
        let mem = ArrayMemory::new();
        let mut cpu = CpuMips::new(mem);
        cpu.pc = 0;
        cpu.gpr[1] = 0_u64.wrapping_sub(1); // -1

        // BLEZ $1, offset=8
        cpu.memory.write_word(0, 0x18200002);
        cpu.step();
        assert_eq!(cpu.pc, 12); // 4 + 8

        cpu.pc = 0;
        cpu.gpr[1] = 10;
        // BGTZ $1, offset=8
        cpu.memory.write_word(0, 0x1C200002);
        cpu.step();
        assert_eq!(cpu.pc, 12); // 4 + 8
    }

    #[test]
    fn test_addi_addiu() {
        let mem = ArrayMemory::new();
        let mut cpu = CpuMips::new(mem);
        cpu.pc = 0;
        cpu.gpr[1] = 10;

        // ADDIU $2, $1, 20
        cpu.memory.write_word(0, 0x24220014);
        cpu.step();
        assert_eq!(cpu.gpr[2], 30);
    }

    #[test]
    fn test_slti_sltiu() {
        let mem = ArrayMemory::new();
        let mut cpu = CpuMips::new(mem);
        cpu.pc = 0;
        cpu.gpr[1] = 10;

        // SLTI $2, $1, 20
        cpu.memory.write_word(0, 0x28220014);
        cpu.step();
        assert_eq!(cpu.gpr[2], 1);

        // SLTI $2, $1, 5
        cpu.memory.write_word(4, 0x28220005);
        cpu.step();
        assert_eq!(cpu.gpr[2], 0);
    }

    #[test]
    fn test_andi_xori() {
        let mem = ArrayMemory::new();
        let mut cpu = CpuMips::new(mem);
        cpu.pc = 0;
        cpu.gpr[1] = 0xFF00;

        // ANDI $2, $1, 0x0FF0
        cpu.memory.write_word(0, 0x30220FF0);
        cpu.step();
        assert_eq!(cpu.gpr[2], 0x0F00);

        // XORI $2, $1, 0x0FF0
        cpu.memory.write_word(4, 0x38220FF0);
        cpu.step();
        assert_eq!(cpu.gpr[2], 0xF0F0);
    }

    #[test]
    fn test_j_jal() {
        let mem = ArrayMemory::new();
        let mut cpu = CpuMips::new(mem);
        cpu.pc = 0;

        // J 0x1000 (target address = 0x4000)
        cpu.memory.write_word(0, 0x08001000);
        cpu.step();
        assert_eq!(cpu.pc, 0x4000);

        cpu.pc = 0;
        // JAL 0x1000
        cpu.memory.write_word(0, 0x0C001000);
        cpu.step();
        assert_eq!(cpu.pc, 0x4000);
        assert_eq!(cpu.gpr[31], 4);
    }

    // ============================================================================
    // Load/Store Instruction Tests
    // ============================================================================

    #[test]
    fn test_lb_lbu() {
        let mem = ArrayMemory::new();
        let mut cpu = CpuMips::new(mem);
        cpu.pc = 0;
        cpu.gpr[1] = 0x1000;
        cpu.memory.write_byte(0x1000, 0xFF);

        // LB $2, 0($1) - Sign-extend
        cpu.memory.write_word(0, 0x80220000);
        cpu.step();
        assert_eq!(cpu.gpr[2] as i8, -1);

        // LBU $2, 0($1) - Zero-extend
        cpu.memory.write_word(4, 0x90220000);
        cpu.step();
        assert_eq!(cpu.gpr[2], 0xFF);
    }

    #[test]
    fn test_lh_lhu() {
        let mem = ArrayMemory::new();
        let mut cpu = CpuMips::new(mem);
        cpu.pc = 0;
        cpu.gpr[1] = 0x1000;
        cpu.memory.write_halfword(0x1000, 0xFFFF);

        // LH $2, 0($1) - Sign-extend
        cpu.memory.write_word(0, 0x84220000);
        cpu.step();
        assert_eq!(cpu.gpr[2] as i16, -1);

        // LHU $2, 0($1) - Zero-extend
        cpu.memory.write_word(4, 0x94220000);
        cpu.step();
        assert_eq!(cpu.gpr[2], 0xFFFF);
    }

    #[test]
    fn test_lwu() {
        let mem = ArrayMemory::new();
        let mut cpu = CpuMips::new(mem);
        cpu.pc = 0;
        cpu.gpr[1] = 0x1000;
        cpu.memory.write_word(0x1000, 0xFFFFFFFF);

        // LWU $2, 0($1) - Zero-extend to 64-bit
        cpu.memory.write_word(0, 0x9C220000);
        cpu.step();
        assert_eq!(cpu.gpr[2], 0xFFFFFFFF);
    }

    #[test]
    fn test_ld_sd() {
        let mem = ArrayMemory::new();
        let mut cpu = CpuMips::new(mem);
        cpu.pc = 0;
        cpu.gpr[1] = 0x1000;
        cpu.gpr[2] = 0x1234567890ABCDEF;

        // SD $2, 0($1)
        cpu.memory.write_word(0, 0xFC220000);
        cpu.step();

        // LD $3, 0($1)
        cpu.memory.write_word(4, 0xDC230000);
        cpu.step();

        assert_eq!(cpu.gpr[3], 0x1234567890ABCDEF);
    }

    #[test]
    fn test_sb_sh() {
        let mem = ArrayMemory::new();
        let mut cpu = CpuMips::new(mem);
        cpu.pc = 0;
        cpu.gpr[1] = 0x1000;
        cpu.gpr[2] = 0x12;

        // SB $2, 0($1)
        cpu.memory.write_word(0, 0xA0220000);
        cpu.step();
        assert_eq!(cpu.memory.read_byte(0x1000), 0x12);

        cpu.gpr[2] = 0x1234;
        // SH $2, 2($1)
        cpu.memory.write_word(4, 0xA4220002);
        cpu.step();
        assert_eq!(cpu.memory.read_halfword(0x1002), 0x1234);
    }

    // ============================================================================
    // Coprocessor Instruction Tests
    // ============================================================================

    #[test]
    fn test_mfc0_mtc0() {
        let mem = ArrayMemory::new();
        let mut cpu = CpuMips::new(mem);
        cpu.pc = 0;
        cpu.gpr[1] = 0x12345678;

        // MTC0 $1, $12 (Status register)
        cpu.memory.write_word(0, 0x40816000);
        cpu.step();
        assert_eq!(cpu.cp0[12], 0x12345678);

        // MFC0 $2, $12
        cpu.memory.write_word(4, 0x40026000);
        cpu.step();
        assert_eq!(cpu.gpr[2], 0x12345678);
    }

    #[test]
    fn test_fpu_basic() {
        let mem = ArrayMemory::new();
        let mut cpu = CpuMips::new(mem);
        cpu.pc = 0;
        cpu.fpr[1] = 10.5;
        cpu.fpr[2] = 2.5;

        // ADD.D $f3, $f1, $f2
        cpu.memory.write_word(0, 0x462208C0);
        cpu.step();
        assert_eq!(cpu.fpr[3], 13.0);

        // SUB.D $f3, $f1, $f2
        cpu.memory.write_word(4, 0x462208C1);
        cpu.step();
        assert_eq!(cpu.fpr[3], 8.0);

        // MUL.D $f3, $f1, $f2
        cpu.memory.write_word(8, 0x462208C2);
        cpu.step();
        assert_eq!(cpu.fpr[3], 26.25);

        // DIV.D $f3, $f1, $f2
        cpu.memory.write_word(12, 0x462208C3);
        cpu.step();
        assert_eq!(cpu.fpr[3], 4.2);
    }
}
