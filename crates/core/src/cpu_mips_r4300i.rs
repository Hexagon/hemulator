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
//!
//! For detailed CPU reference documentation, see: `docs/references/cpu_mips_r4300i.md`
//!
//! # Implementation Notes and Common Pitfalls
//!
//! ## Sign Extension
//!
//! The R4300i is a 64-bit CPU with 32-bit operations. Proper sign extension is critical:
//!
//! ### 32-bit Operations
//! - **Word loads (LW)**: Sign-extend to 64 bits via `val as i32 as u64`
//! - **Word arithmetic (ADD, ADDU, SUB, SUBU)**: Operate on low 32 bits, sign-extend result
//! - **Immediate values**: 16-bit immediates sign-extended to 32 or 64 bits
//!
//! ### Sign Extension Pattern
//! ```rust,ignore
//! // Correct: Sign-extend 32-bit value to 64-bit
//! self.gpr[rt] = value as i32 as u64;  // First to i32 (sign-extends), then to u64
//!
//! // Incorrect: Zero-extends instead of sign-extends
//! self.gpr[rt] = value as u64;  // Only use for unsigned loads (LWU, LBU, LHU)
//! ```
//!
//! ## Division by Zero
//!
//! All division instructions (DIV, DIVU, DDIV, DDIVU) check for zero divisor:
//! - If divisor is 0, the operation is skipped (HI/LO unchanged)
//! - This matches MIPS behavior where division by zero is unpredictable but doesn't trap
//!
//! ## Overflow Handling
//!
//! Signed arithmetic instructions (ADD, ADDI, SUB, DADD, DADDI, DSUB) should trap on overflow
//! per MIPS spec, but this implementation uses wrapping arithmetic:
//! - **Current behavior**: Uses `wrapping_add`/`wrapping_sub` (no trap)
//! - **Rationale**: Most N64 software doesn't rely on overflow traps
//! - **Unsigned variants**: ADDU, ADDIU, SUBU never trap (correct behavior)
//!
//! ## Memory Alignment
//!
//! Load/store instructions have specific alignment requirements:
//! - **LH/LHU/SH**: Must be 2-byte aligned
//! - **LW/LWU/SW**: Must be 4-byte aligned
//! - **LD/SD**: Must be 8-byte aligned
//! - **Unaligned access**: Logs a warning (alignment validated)
//! - **LWL/LWR/LDL/LDR**: Used for unaligned access, don't require alignment
//!
//! ## Register 0 Immutability
//!
//! GPR[0] is hardwired to zero. After every instruction execution:
//! ```rust,ignore
//! self.gpr[0] = 0;  // Enforced in step() after each instruction
//! ```
//!
//! ## Shift Operations
//!
//! Shift operations use Rust's `wrapping_shl`/`wrapping_shr` to prevent undefined behavior:
//! - **32-bit shifts**: Shift amount masked to 5 bits (0-31)
//! - **64-bit shifts**: Shift amount masked to 6 bits (0-63)
//! - Safe against overflow/underflow

use crate::logging::{log, LogCategory, LogLevel};

/// TLB entry data structure for CP0 TLB instructions
/// This is a simplified representation that matches CP0 register format
#[derive(Debug, Clone, Copy, Default)]
pub struct TlbEntryData {
    /// Virtual Page Number / 2 (from CP0 EntryHi)
    pub vpn2: u64,
    /// Address Space ID (from CP0 EntryHi)
    pub asid: u8,
    /// Global bit
    pub global: bool,
    /// Page mask (from CP0 PageMask)
    pub page_mask: u32,
    /// Even page physical frame number (from CP0 EntryLo0)
    pub pfn0: u32,
    /// Even page cache coherency
    pub c0: u8,
    /// Even page dirty bit
    pub d0: bool,
    /// Even page valid bit
    pub v0: bool,
    /// Odd page physical frame number (from CP0 EntryLo1)
    pub pfn1: u32,
    /// Odd page cache coherency
    pub c1: u8,
    /// Odd page dirty bit
    pub d1: bool,
    /// Odd page valid bit
    pub v1: bool,
}

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

    /// TLB operations for CP0 instructions (optional - default no-op)
    /// These are only needed for systems with TLB support (N64)
    ///
    /// Write TLB entry at specified index (TLBWI)
    fn tlb_write_indexed(&mut self, _index: usize, _entry: TlbEntryData) {
        // Default: no-op for systems without TLB
    }

    /// Write TLB entry at random index (TLBWR)
    /// The index parameter comes from CP0 Random register for determinism
    fn tlb_write_random(&mut self, _index: usize, _entry: TlbEntryData) {
        // Default: no-op for systems without TLB
    }

    /// Read TLB entry at specified index (TLBR)
    fn tlb_read_indexed(&self, _index: usize) -> Option<TlbEntryData> {
        // Default: no-op for systems without TLB
        None
    }

    /// Probe TLB for matching entry (TLBP)
    /// Returns index of matching entry, or None if no match
    fn tlb_probe(&self, _vpn2: u64, _asid: u8) -> Option<usize> {
        // Default: no-op for systems without TLB
        None
    }
}

/// MIPS R4300i CPU state and execution engine
#[derive(Debug)]
pub struct CpuMips<M: MemoryMips> {
    /// General-purpose registers (R0-R31)
    /// Note: R0 is always zero
    pub gpr: [u64; 32],

    /// Program counter
    pub pc: u64,

    /// Next PC (for branch delay slot handling)
    /// When a branch/jump executes, this is set to the target address.
    /// After the delay slot executes, PC is updated to next_pc.
    next_pc: u64,

    /// Whether we're currently in a branch delay slot
    /// When true, the next instruction is the last before a branch takes effect
    in_delay_slot: bool,

    /// HI register (for multiply/divide results)
    pub hi: u64,

    /// LO register (for multiply/divide results)
    pub lo: u64,

    /// Floating-point registers (stored as raw u64 bit patterns)
    pub fpr: [u64; 32],

    /// Floating-point control/status register
    pub fcr31: u32,

    /// Load Linked bit (for LL/SC atomic operations)
    ll_bit: bool,

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
            pc: 0xBFC0_0000,      // Reset vector in BIOS ROM
            next_pc: 0xBFC0_0004, // Next PC after first instruction
            in_delay_slot: false,
            hi: 0,
            lo: 0,
            fpr: [0; 32],
            fcr31: 0,
            ll_bit: false,
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
        self.next_pc = 0xBFC0_0004;
        self.in_delay_slot = false;
        self.hi = 0;
        self.lo = 0;
        self.fpr = [0; 32];
        self.fcr31 = 0;
        self.ll_bit = false;
        self.cp0 = [0; 32];
        self.cp0[CP0_PRID] = 0x0B00;
        self.cp0[CP0_STATUS] = 0x3400_0000;
        self.cp0[CP0_CONFIG] = 0x7006_E463;
        self.cycles = 0;
    }

    /// Test accessor for delay slot state
    #[cfg(test)]
    pub(crate) fn is_in_delay_slot(&self) -> bool {
        self.in_delay_slot
    }

    /// Execute a single instruction and return cycles consumed
    pub fn step(&mut self) -> u32 {
        let start_cycles = self.cycles;

        // Check for pending interrupts before fetching instruction
        if self.check_interrupts() {
            // Interrupt was handled, return early
            return (self.cycles - start_cycles) as u32;
        }

        // Fetch instruction at current PC
        let instr = self.memory.read_word(self.pc as u32);

        // Save current PC for branch calculations
        let current_pc = self.pc;

        // Check if we're in a delay slot (from a previous branch/jump)
        let was_in_delay_slot = self.in_delay_slot;

        // Save the pending branch target before executing the delay slot
        let pending_branch_target = self.next_pc;

        // Update PC to next sequential instruction
        self.pc = self.pc.wrapping_add(4);

        // Clear in_delay_slot flag before executing instruction
        // This allows the delay slot instruction to set its own branch if needed
        self.in_delay_slot = false;

        // Decode opcode (bits 26-31)
        let opcode = (instr >> 26) & 0x3F;

        // Execute the instruction
        // Branch/jump instructions will set next_pc
        match opcode {
            0x00 => self.execute_special(instr, current_pc), // R-type instructions
            0x01 => self.execute_regimm(instr, current_pc),  // REGIMM (branch instructions)
            0x02 => self.execute_j(instr, current_pc),       // J
            0x03 => self.execute_jal(instr, current_pc),     // JAL
            0x04 => self.execute_beq(instr, current_pc),     // BEQ
            0x05 => self.execute_bne(instr, current_pc),     // BNE
            0x06 => self.execute_blez(instr, current_pc),    // BLEZ
            0x07 => self.execute_bgtz(instr, current_pc),    // BGTZ
            0x08 => self.execute_addi(instr),                // ADDI
            0x09 => self.execute_addiu(instr),               // ADDIU
            0x0A => self.execute_slti(instr),                // SLTI
            0x0B => self.execute_sltiu(instr),               // SLTIU
            0x0C => self.execute_andi(instr),                // ANDI
            0x0D => self.execute_ori(instr),                 // ORI
            0x0E => self.execute_xori(instr),                // XORI
            0x0F => self.execute_lui(instr),                 // LUI
            0x10 => self.execute_cop0(instr, current_pc),    // COP0
            0x11 => self.execute_cop1(instr, current_pc),    // COP1
            0x14 => self.execute_beql(instr, current_pc),    // BEQL
            0x15 => self.execute_bnel(instr, current_pc),    // BNEL
            0x16 => self.execute_blezl(instr, current_pc),   // BLEZL
            0x17 => self.execute_bgtzl(instr, current_pc),   // BGTZL
            0x18 => self.execute_daddi(instr),               // DADDI
            0x19 => self.execute_daddiu(instr),              // DADDIU
            0x1A => self.execute_ldl(instr),                 // LDL
            0x1B => self.execute_ldr(instr),                 // LDR
            0x20 => self.execute_lb(instr),                  // LB
            0x21 => self.execute_lh(instr),                  // LH
            0x22 => self.execute_lwl(instr),                 // LWL
            0x23 => self.execute_lw(instr),                  // LW
            0x24 => self.execute_lbu(instr),                 // LBU
            0x25 => self.execute_lhu(instr),                 // LHU
            0x26 => self.execute_lwr(instr),                 // LWR
            0x27 => self.execute_lwu(instr),                 // LWU
            0x28 => self.execute_sb(instr),                  // SB
            0x29 => self.execute_sh(instr),                  // SH
            0x2A => self.execute_swl(instr),                 // SWL
            0x2B => self.execute_sw(instr),                  // SW
            0x2C => self.execute_sdl(instr),                 // SDL
            0x2D => self.execute_sdr(instr),                 // SDR
            0x2E => self.execute_swr(instr),                 // SWR
            0x2F => self.execute_cache(instr),               // CACHE
            0x30 => self.execute_ll(instr),                  // LL
            0x31 => self.execute_lwc1(instr),                // LWC1
            0x35 => self.execute_ldc1(instr),                // LDC1
            0x37 => self.execute_ld(instr),                  // LD
            0x38 => self.execute_sc(instr),                  // SC
            0x39 => self.execute_swc1(instr),                // SWC1
            0x3D => self.execute_sdc1(instr),                // SDC1
            0x3F => self.execute_sd(instr),                  // SD
            _ => {
                // Unimplemented instruction
                self.cycles += 1;
            }
        }

        // If we just executed the delay slot of a taken branch/jump, apply its pending branch target.
        // Note: was_in_delay_slot is only true if the previous instruction was a taken branch/jump.
        // If the delay slot instruction itself set a new branch, that branch's target is now in next_pc
        // and will be applied after executing its own delay slot, but we still complete the original
        // branch by updating pc to pending_branch_target here.
        if was_in_delay_slot {
            self.pc = pending_branch_target;
        }

        // R0 is always zero
        self.gpr[0] = 0;

        // Update CP0 Count register (increments at half the pipeline clock rate)
        // On real R4300i, Count increments once every 2 PCycles (i.e., once per instruction)
        // Count is a 32-bit register that wraps around
        let elapsed = self.cycles - start_cycles;
        let old_count = self.cp0[CP0_COUNT] & 0xFFFF_FFFF;
        let new_count = old_count.wrapping_add(elapsed) & 0xFFFF_FFFF;
        self.cp0[CP0_COUNT] = new_count;

        // Check if Count crossed Compare → set timer interrupt (IP7)
        let compare = self.cp0[CP0_COMPARE] & 0xFFFF_FFFF;
        if compare != 0
            && ((old_count < compare && new_count >= compare)
                || (old_count > new_count && (new_count >= compare || old_count < compare)))
        {
            // Set IP7 (bit 15 in Cause register = interrupt pending bit 7)
            self.cp0[CP0_CAUSE] |= 1u64 << 15;
        }

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

        // If we're in a delay slot, set BD bit in Cause and save EPC to the branch instruction
        if self.in_delay_slot {
            // Set BD (Branch Delay) bit in Cause register (bit 31)
            self.cp0[CP0_CAUSE] |= 1u64 << 31;
            // EPC should point to the branch instruction (PC - 4)
            self.cp0[CP0_EPC] = self.pc.wrapping_sub(4);
            // Clear delay slot state to prevent incorrect PC update after exception handler
            self.in_delay_slot = false;
        } else {
            // Clear BD bit
            self.cp0[CP0_CAUSE] &= !(1u64 << 31);
            // Save return address in EPC (current PC)
            self.cp0[CP0_EPC] = self.pc;
        }

        // Set exception code in Cause register
        self.cp0[CP0_CAUSE] &= !0x7C; // Clear exception code bits (2-6)
        self.cp0[CP0_CAUSE] |= (exception_code << 2) & 0x7C;

        // Jump to exception vector
        // Normal exception vector is at 0x80000180
        self.pc = 0x80000180;

        // Clear LL bit - exceptions break atomic sequences
        self.ll_bit = false;

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
    fn execute_special(&mut self, instr: u32, current_pc: u64) {
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
                // Set next_pc to register value, mark as in delay slot
                self.next_pc = self.gpr[rs];
                self.in_delay_slot = true;
                self.cycles += 1;
            }
            0x09 => {
                // JALR - Jump And Link Register
                // Save return address (PC+8, which is current_pc + 8)
                self.gpr[rd] = current_pc.wrapping_add(8);
                self.next_pc = self.gpr[rs];
                self.in_delay_slot = true;
                self.cycles += 1;
            }
            0x0C => {
                // SYSCALL - System Call
                self.handle_exception(8);
            }
            0x0D => {
                // BREAK - Breakpoint
                self.handle_exception(9);
            }
            0x0F => {
                // SYNC - Memory barrier (no-op in emulation)
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
                // MULT - Multiply (signed 32x32 -> 64)
                // Sign extension: Extend 32-bit values to 64-bit signed, then multiply
                let a = self.gpr[rs] as i32 as i64;
                let b = self.gpr[rt] as i32 as i64;
                let result = a.wrapping_mul(b);
                self.lo = result as u64;
                // High word is sign-extended from i32 to maintain proper sign
                self.hi = ((result >> 32) as i32) as u64;
                self.cycles += 1;
            }
            0x19 => {
                // MULTU - Multiply Unsigned (unsigned 32x32 -> 64)
                // Zero extension: Treat 32-bit values as unsigned
                let a = (self.gpr[rs] as u32) as u64;
                let b = (self.gpr[rt] as u32) as u64;
                let result = a.wrapping_mul(b);
                // Both low and high results are sign-extended to match MIPS behavior
                // This ensures the high 32 bits of the 64-bit register have proper sign
                self.lo = (result as u32) as i32 as u64;
                self.hi = ((result >> 32) as u32) as i32 as u64;
                self.cycles += 1;
            }
            0x1A => {
                // DIV - Divide (signed 32-bit)
                let dividend = self.gpr[rs] as i32;
                let divisor = self.gpr[rt] as i32;
                // Edge case: Division by zero is handled by skipping the operation
                // MIPS spec: Division by zero produces unpredictable results (no trap)
                if divisor != 0 {
                    self.lo = dividend.wrapping_div(divisor) as u64;
                    self.hi = dividend.wrapping_rem(divisor) as u64;
                }
                self.cycles += 1;
            }
            0x1B => {
                // DIVU - Divide Unsigned (unsigned 32-bit)
                let dividend = self.gpr[rs] as u32;
                let divisor = self.gpr[rt] as u32;
                // Edge case: Division by zero handled same as DIV
                if divisor != 0 {
                    // Results are sign-extended to match MIPS behavior
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
                // NOTE: Should trap on overflow, but not implemented in this emulator
                // Most N64 software doesn't rely on overflow traps
                let a = self.gpr[rs] as i32;
                let b = self.gpr[rt] as i32;
                // Result is sign-extended to 64 bits
                self.gpr[rd] = a.wrapping_add(b) as u64;
                self.cycles += 1;
            }
            0x21 => {
                // ADDU - Add Unsigned (no overflow trap)
                // Treat as 32-bit operation, sign-extend result to 64 bits
                self.gpr[rd] =
                    (self.gpr[rs] as u32).wrapping_add(self.gpr[rt] as u32) as i32 as u64;
                self.cycles += 1;
            }
            0x22 => {
                // SUB - Subtract (with overflow trap)
                // NOTE: Should trap on overflow, but not implemented
                let a = self.gpr[rs] as i32;
                let b = self.gpr[rt] as i32;
                // Result is sign-extended to 64 bits
                self.gpr[rd] = a.wrapping_sub(b) as u64;
                self.cycles += 1;
            }
            0x23 => {
                // SUBU - Subtract Unsigned (no overflow trap)
                // Treat as 32-bit operation, sign-extend result to 64 bits
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

        // Sign extension pattern: Shift into upper 16 bits of 32-bit word,
        // then sign-extend to 64 bits based on bit 31
        // Examples:
        //   LUI $t0, 0x1234 -> $t0 = 0x0000000012340000 (bit 31=0, zero-extends)
        //   LUI $t0, 0x8000 -> $t0 = 0xFFFFFFFF80000000 (bit 31=1, sign-extends)
        self.gpr[rt] = ((imm << 16) as i32) as u64;
        self.cycles += 1;
    }

    /// Execute LW - Load Word
    fn execute_lw(&mut self, instr: u32) {
        let rt = ((instr >> 16) & 0x1F) as usize;
        let rs = ((instr >> 21) & 0x1F) as usize;
        let offset = (instr & 0xFFFF) as i16 as i32;

        let addr = (self.gpr[rs] as i64).wrapping_add(offset as i64) as u32;

        // Validate 4-byte alignment
        if addr & 3 != 0 {
            log(LogCategory::CPU, LogLevel::Warn, || {
                format!(
                    "LW: Unaligned word access at 0x{:08X} (PC=0x{:016X})",
                    addr, self.pc
                )
            });
        }

        let val = self.memory.read_word(addr);
        // Critical: Sign-extend 32-bit word to 64 bits
        // Example: 0x80000000 becomes 0xFFFFFFFF80000000 (negative number)
        self.gpr[rt] = val as i32 as u64;
        self.cycles += 1;
    }

    /// Execute SW - Store Word
    fn execute_sw(&mut self, instr: u32) {
        let rt = ((instr >> 16) & 0x1F) as usize;
        let rs = ((instr >> 21) & 0x1F) as usize;
        let offset = (instr & 0xFFFF) as i16 as i32;

        let addr = (self.gpr[rs] as i64).wrapping_add(offset as i64) as u32;

        // Validate 4-byte alignment
        if addr & 3 != 0 {
            log(LogCategory::CPU, LogLevel::Warn, || {
                format!(
                    "SW: Unaligned word access at 0x{:08X} (PC=0x{:016X})",
                    addr, self.pc
                )
            });
        }

        self.memory.write_word(addr, self.gpr[rt] as u32);
        self.cycles += 1;
    }

    /// Execute LL - Load Linked Word (for atomic read-modify-write operations)
    fn execute_ll(&mut self, instr: u32) {
        let rt = ((instr >> 16) & 0x1F) as usize;
        let rs = ((instr >> 21) & 0x1F) as usize;
        let offset = (instr & 0xFFFF) as i16 as i32;

        let addr = (self.gpr[rs] as i64).wrapping_add(offset as i64) as u32;
        let val = self.memory.read_word(addr);
        self.gpr[rt] = val as i32 as u64;
        self.ll_bit = true;
        self.cycles += 1;
    }

    /// Execute SC - Store Conditional Word (completes atomic read-modify-write)
    fn execute_sc(&mut self, instr: u32) {
        let rt = ((instr >> 16) & 0x1F) as usize;
        let rs = ((instr >> 21) & 0x1F) as usize;
        let offset = (instr & 0xFFFF) as i16 as i32;

        if self.ll_bit {
            let addr = (self.gpr[rs] as i64).wrapping_add(offset as i64) as u32;
            self.memory.write_word(addr, self.gpr[rt] as u32);
            self.gpr[rt] = 1; // Success
        } else {
            self.gpr[rt] = 0; // Failure
        }
        self.ll_bit = false;
        self.cycles += 1;
    }

    /// Execute LWC1 - Load Word to Coprocessor 1 (FPU)
    fn execute_lwc1(&mut self, instr: u32) {
        let ft = ((instr >> 16) & 0x1F) as usize;
        let rs = ((instr >> 21) & 0x1F) as usize;
        let offset = (instr & 0xFFFF) as i16 as i32;

        let addr = (self.gpr[rs] as i64).wrapping_add(offset as i64) as u32;
        let val = self.memory.read_word(addr);
        self.fpr[ft] = val as u64;
        self.cycles += 1;
    }

    /// Execute SWC1 - Store Word from Coprocessor 1 (FPU)
    fn execute_swc1(&mut self, instr: u32) {
        let ft = ((instr >> 16) & 0x1F) as usize;
        let rs = ((instr >> 21) & 0x1F) as usize;
        let offset = (instr & 0xFFFF) as i16 as i32;

        let addr = (self.gpr[rs] as i64).wrapping_add(offset as i64) as u32;
        let val = self.fpr[ft] as u32;
        self.memory.write_word(addr, val);
        self.cycles += 1;
    }

    /// Execute LDC1 - Load Doubleword to Coprocessor 1 (FPU)
    fn execute_ldc1(&mut self, instr: u32) {
        let ft = ((instr >> 16) & 0x1F) as usize;
        let rs = ((instr >> 21) & 0x1F) as usize;
        let offset = (instr & 0xFFFF) as i16 as i32;

        let addr = (self.gpr[rs] as i64).wrapping_add(offset as i64) as u32;
        self.fpr[ft] = self.memory.read_doubleword(addr);
        self.cycles += 1;
    }

    /// Execute SDC1 - Store Doubleword from Coprocessor 1 (FPU)
    fn execute_sdc1(&mut self, instr: u32) {
        let ft = ((instr >> 16) & 0x1F) as usize;
        let rs = ((instr >> 21) & 0x1F) as usize;
        let offset = (instr & 0xFFFF) as i16 as i32;

        let addr = (self.gpr[rs] as i64).wrapping_add(offset as i64) as u32;
        self.memory.write_doubleword(addr, self.fpr[ft]);
        self.cycles += 1;
    }

    // ============================================================================
    // I-Type Instructions
    // ============================================================================

    /// Execute REGIMM (opcode 0x01) - Branch instructions
    fn execute_regimm(&mut self, instr: u32, current_pc: u64) {
        let rs = ((instr >> 21) & 0x1F) as usize;
        let rt = (instr >> 16) & 0x1F; // This is the regimm field
        let offset = ((instr & 0xFFFF) as i16 as i32) << 2;

        match rt {
            0x00 => {
                // BLTZ - Branch on Less Than Zero
                if (self.gpr[rs] as i64) < 0 {
                    // Branch target = (branch_addr + 4) + offset
                    self.next_pc =
                        (current_pc.wrapping_add(4) as i64).wrapping_add(offset as i64) as u64;
                    self.in_delay_slot = true;
                }
                self.cycles += 1;
            }
            0x01 => {
                // BGEZ - Branch on Greater Than or Equal to Zero
                if (self.gpr[rs] as i64) >= 0 {
                    self.next_pc =
                        (current_pc.wrapping_add(4) as i64).wrapping_add(offset as i64) as u64;
                    self.in_delay_slot = true;
                }
                self.cycles += 1;
            }
            0x02 => {
                // BLTZL - Branch on Less Than Zero Likely
                if (self.gpr[rs] as i64) < 0 {
                    // Branch taken: set delay slot
                    self.next_pc =
                        (current_pc.wrapping_add(4) as i64).wrapping_add(offset as i64) as u64;
                    self.in_delay_slot = true;
                } else {
                    // Branch not taken: nullify delay slot
                    self.pc = self.pc.wrapping_add(4);
                }
                self.cycles += 1;
            }
            0x03 => {
                // BGEZL - Branch on Greater Than or Equal to Zero Likely
                if (self.gpr[rs] as i64) >= 0 {
                    // Branch taken: set delay slot
                    self.next_pc =
                        (current_pc.wrapping_add(4) as i64).wrapping_add(offset as i64) as u64;
                    self.in_delay_slot = true;
                } else {
                    // Branch not taken: nullify delay slot
                    self.pc = self.pc.wrapping_add(4);
                }
                self.cycles += 1;
            }
            0x10 => {
                // BLTZAL - Branch on Less Than Zero And Link
                if (self.gpr[rs] as i64) < 0 {
                    self.gpr[31] = current_pc.wrapping_add(8); // Return address = PC + 8
                    self.next_pc =
                        (current_pc.wrapping_add(4) as i64).wrapping_add(offset as i64) as u64;
                    self.in_delay_slot = true;
                }
                self.cycles += 1;
            }
            0x11 => {
                // BGEZAL - Branch on Greater Than or Equal to Zero And Link
                if (self.gpr[rs] as i64) >= 0 {
                    self.gpr[31] = current_pc.wrapping_add(8);
                    self.next_pc =
                        (current_pc.wrapping_add(4) as i64).wrapping_add(offset as i64) as u64;
                    self.in_delay_slot = true;
                }
                self.cycles += 1;
            }
            0x12 => {
                // BLTZALL - Branch on Less Than Zero And Link Likely
                if (self.gpr[rs] as i64) < 0 {
                    self.gpr[31] = current_pc.wrapping_add(8);
                    self.next_pc =
                        (current_pc.wrapping_add(4) as i64).wrapping_add(offset as i64) as u64;
                    self.in_delay_slot = true;
                } else {
                    // Branch not taken: nullify delay slot
                    self.pc = self.pc.wrapping_add(4);
                }
                self.cycles += 1;
            }
            0x13 => {
                // BGEZALL - Branch on Greater Than or Equal to Zero And Link Likely
                if (self.gpr[rs] as i64) >= 0 {
                    self.gpr[31] = current_pc.wrapping_add(8);
                    self.next_pc =
                        (current_pc.wrapping_add(4) as i64).wrapping_add(offset as i64) as u64;
                    self.in_delay_slot = true;
                } else {
                    // Branch not taken: nullify delay slot
                    self.pc = self.pc.wrapping_add(4);
                }
                self.cycles += 1;
            }
            _ => {
                self.cycles += 1;
            }
        }
    }

    /// Execute J - Jump
    fn execute_j(&mut self, instr: u32, current_pc: u64) {
        let target = instr & 0x03FFFFFF;
        // Jump target = (delay_slot_addr & 0xFFFFFFFF_F0000000) | (target << 2)
        // delay_slot_addr = current_pc + 4
        let delay_slot_addr = current_pc.wrapping_add(4);
        self.next_pc = (delay_slot_addr & 0xFFFFFFFF_F0000000) | ((target << 2) as u64);
        self.in_delay_slot = true;
        self.cycles += 1;
    }

    /// Execute JAL - Jump And Link
    fn execute_jal(&mut self, instr: u32, current_pc: u64) {
        let target = instr & 0x03FFFFFF;
        // Save return address (PC + 8)
        self.gpr[31] = current_pc.wrapping_add(8);
        // Jump target uses upper bits of delay slot address
        let delay_slot_addr = current_pc.wrapping_add(4);
        self.next_pc = (delay_slot_addr & 0xFFFFFFFF_F0000000) | ((target << 2) as u64);
        self.in_delay_slot = true;
        self.cycles += 1;
    }

    /// Execute BEQ - Branch on Equal
    fn execute_beq(&mut self, instr: u32, current_pc: u64) {
        let rs = ((instr >> 21) & 0x1F) as usize;
        let rt = ((instr >> 16) & 0x1F) as usize;
        let offset = ((instr & 0xFFFF) as i16 as i32) << 2;

        if self.gpr[rs] == self.gpr[rt] {
            // Branch target = (branch_addr + 4) + offset
            self.next_pc = (current_pc.wrapping_add(4) as i64).wrapping_add(offset as i64) as u64;
            self.in_delay_slot = true;
        }
        self.cycles += 1;
    }

    /// Execute BNE - Branch on Not Equal
    fn execute_bne(&mut self, instr: u32, current_pc: u64) {
        let rs = ((instr >> 21) & 0x1F) as usize;
        let rt = ((instr >> 16) & 0x1F) as usize;
        let offset = ((instr & 0xFFFF) as i16 as i32) << 2;

        if self.gpr[rs] != self.gpr[rt] {
            self.next_pc = (current_pc.wrapping_add(4) as i64).wrapping_add(offset as i64) as u64;
            self.in_delay_slot = true;
        }
        self.cycles += 1;
    }

    /// Execute BLEZ - Branch on Less Than or Equal to Zero
    fn execute_blez(&mut self, instr: u32, current_pc: u64) {
        let rs = ((instr >> 21) & 0x1F) as usize;
        let offset = ((instr & 0xFFFF) as i16 as i32) << 2;

        if (self.gpr[rs] as i64) <= 0 {
            self.next_pc = (current_pc.wrapping_add(4) as i64).wrapping_add(offset as i64) as u64;
            self.in_delay_slot = true;
        }
        self.cycles += 1;
    }

    /// Execute BGTZ - Branch on Greater Than Zero
    fn execute_bgtz(&mut self, instr: u32, current_pc: u64) {
        let rs = ((instr >> 21) & 0x1F) as usize;
        let offset = ((instr & 0xFFFF) as i16 as i32) << 2;

        if (self.gpr[rs] as i64) > 0 {
            self.next_pc = (current_pc.wrapping_add(4) as i64).wrapping_add(offset as i64) as u64;
            self.in_delay_slot = true;
        }
        self.cycles += 1;
    }

    /// Execute BEQL - Branch on Equal Likely
    fn execute_beql(&mut self, instr: u32, current_pc: u64) {
        let rs = ((instr >> 21) & 0x1F) as usize;
        let rt = ((instr >> 16) & 0x1F) as usize;
        let offset = ((instr & 0xFFFF) as i16 as i32) << 2;

        if self.gpr[rs] == self.gpr[rt] {
            // Branch taken: set delay slot
            self.next_pc = (current_pc.wrapping_add(4) as i64).wrapping_add(offset as i64) as u64;
            self.in_delay_slot = true;
        } else {
            // Branch not taken: nullify delay slot by skipping it (PC += 8 total)
            self.pc = self.pc.wrapping_add(4); // PC was already incremented by 4, add another 4
        }
        self.cycles += 1;
    }

    /// Execute BNEL - Branch on Not Equal Likely
    fn execute_bnel(&mut self, instr: u32, current_pc: u64) {
        let rs = ((instr >> 21) & 0x1F) as usize;
        let rt = ((instr >> 16) & 0x1F) as usize;
        let offset = ((instr & 0xFFFF) as i16 as i32) << 2;

        if self.gpr[rs] != self.gpr[rt] {
            // Branch taken: set delay slot
            self.next_pc = (current_pc.wrapping_add(4) as i64).wrapping_add(offset as i64) as u64;
            self.in_delay_slot = true;
        } else {
            // Branch not taken: nullify delay slot
            self.pc = self.pc.wrapping_add(4);
        }
        self.cycles += 1;
    }

    /// Execute BLEZL - Branch on Less Than or Equal to Zero Likely
    fn execute_blezl(&mut self, instr: u32, current_pc: u64) {
        let rs = ((instr >> 21) & 0x1F) as usize;
        let offset = ((instr & 0xFFFF) as i16 as i32) << 2;

        if (self.gpr[rs] as i64) <= 0 {
            // Branch taken: set delay slot
            self.next_pc = (current_pc.wrapping_add(4) as i64).wrapping_add(offset as i64) as u64;
            self.in_delay_slot = true;
        } else {
            // Branch not taken: nullify delay slot
            self.pc = self.pc.wrapping_add(4);
        }
        self.cycles += 1;
    }

    /// Execute BGTZL - Branch on Greater Than Zero Likely
    fn execute_bgtzl(&mut self, instr: u32, current_pc: u64) {
        let rs = ((instr >> 21) & 0x1F) as usize;
        let offset = ((instr & 0xFFFF) as i16 as i32) << 2;

        if (self.gpr[rs] as i64) > 0 {
            // Branch taken: set delay slot
            self.next_pc = (current_pc.wrapping_add(4) as i64).wrapping_add(offset as i64) as u64;
            self.in_delay_slot = true;
        } else {
            // Branch not taken: nullify delay slot
            self.pc = self.pc.wrapping_add(4);
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

        // Validate 2-byte alignment
        if addr & 1 != 0 {
            log(LogCategory::CPU, LogLevel::Warn, || {
                format!(
                    "LH: Unaligned halfword access at 0x{:08X} (PC=0x{:016X})",
                    addr, self.pc
                )
            });
        }

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

        // Validate 2-byte alignment
        if addr & 1 != 0 {
            log(LogCategory::CPU, LogLevel::Warn, || {
                format!(
                    "LHU: Unaligned halfword access at 0x{:08X} (PC=0x{:016X})",
                    addr, self.pc
                )
            });
        }

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

        // Validate 4-byte alignment
        if addr & 3 != 0 {
            log(LogCategory::CPU, LogLevel::Warn, || {
                format!(
                    "LWU: Unaligned word access at 0x{:08X} (PC=0x{:016X})",
                    addr, self.pc
                )
            });
        }

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

        // Validate 8-byte alignment
        if addr & 7 != 0 {
            log(LogCategory::CPU, LogLevel::Warn, || {
                format!(
                    "LD: Unaligned doubleword access at 0x{:08X} (PC=0x{:016X})",
                    addr, self.pc
                )
            });
        }

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

        // Validate 2-byte alignment
        if addr & 1 != 0 {
            log(LogCategory::CPU, LogLevel::Warn, || {
                format!(
                    "SH: Unaligned halfword access at 0x{:08X} (PC=0x{:016X})",
                    addr, self.pc
                )
            });
        }

        self.memory.write_halfword(addr, self.gpr[rt] as u16);
        self.cycles += 1;
    }

    /// Execute SD - Store Doubleword
    fn execute_sd(&mut self, instr: u32) {
        let rs = ((instr >> 21) & 0x1F) as usize;
        let rt = ((instr >> 16) & 0x1F) as usize;
        let offset = (instr & 0xFFFF) as i16 as i32;

        let addr = (self.gpr[rs] as i64).wrapping_add(offset as i64) as u32;

        // Validate 8-byte alignment
        if addr & 7 != 0 {
            log(LogCategory::CPU, LogLevel::Warn, || {
                format!(
                    "SD: Unaligned doubleword access at 0x{:08X} (PC=0x{:016X})",
                    addr, self.pc
                )
            });
        }

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
    fn execute_cop0(&mut self, instr: u32, _current_pc: u64) {
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
                let value = self.gpr[rt];
                self.cp0[rd] = value;

                // Writing to CP0_COMPARE (register 11) clears the timer interrupt (IP7)
                if rd == CP0_COMPARE {
                    self.cp0[CP0_CAUSE] &= !(1u64 << 15);
                }

                self.cycles += 1;
            }
            0x10 => {
                // COP0 function
                let funct = instr & 0x3F;
                match funct {
                    0x01 => {
                        // TLBR - Read Indexed TLB Entry
                        self.execute_tlbr();
                    }
                    0x02 => {
                        // TLBWI - Write Indexed TLB Entry
                        self.execute_tlbwi();
                    }
                    0x06 => {
                        // TLBWR - Write Random TLB Entry
                        self.execute_tlbwr();
                    }
                    0x08 => {
                        // TLBP - Probe TLB for Matching Entry
                        self.execute_tlbp();
                    }
                    0x18 => {
                        // ERET - Exception Return
                        // ERET does NOT have a delay slot - it immediately returns to EPC
                        // This is different from branches/jumps
                        self.pc = self.cp0[CP0_EPC];
                        // Clear EXL bit to re-enable interrupts
                        self.cp0[CP0_STATUS] &= !0x02;
                        // Clear LL bit
                        self.ll_bit = false;
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
    fn execute_cop1(&mut self, instr: u32, current_pc: u64) {
        let rs = (instr >> 21) & 0x1F;
        let rt = ((instr >> 16) & 0x1F) as usize;
        let fs = ((instr >> 11) & 0x1F) as usize;
        let ft = ((instr >> 16) & 0x1F) as usize;
        let fd = ((instr >> 6) & 0x1F) as usize;
        let funct = instr & 0x3F;

        match rs {
            0x00 => {
                // MFC1 - Move From FPU (lower 32 bits)
                self.gpr[rt] = self.fpr[fs] as u32 as i32 as u64;
                self.cycles += 1;
            }
            0x01 => {
                // DMFC1 - Doubleword Move From FPU
                self.gpr[rt] = self.fpr[fs];
                self.cycles += 1;
            }
            0x02 => {
                // CFC1 - Move Control From FPU
                if fs == 31 {
                    self.gpr[rt] = self.fcr31 as i32 as u64; // Sign-extend
                } else if fs == 0 {
                    // FCR0 - FPU Implementation/Revision register
                    self.gpr[rt] = 0x0A00; // R4300i FPU
                }
                self.cycles += 1;
            }
            0x04 => {
                // MTC1 - Move To FPU (32-bit)
                self.fpr[fs] = self.gpr[rt] as u32 as u64;
                self.cycles += 1;
            }
            0x05 => {
                // DMTC1 - Doubleword Move To FPU
                self.fpr[fs] = self.gpr[rt];
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
                let nd = (instr >> 17) & 0x1; // Nullify delay slot bit
                let tf = (instr >> 16) & 0x1;
                let offset = ((instr & 0xFFFF) as i16 as i32) << 2;
                let condition = (self.fcr31 >> (23 + cc)) & 0x1;

                if condition == tf {
                    self.next_pc =
                        (current_pc.wrapping_add(4) as i64).wrapping_add(offset as i64) as u64;
                    self.in_delay_slot = true;
                } else if nd == 1 {
                    self.pc = self.pc.wrapping_add(4);
                }
                self.cycles += 1;
            }
            0x10 => {
                // Single-precision (fmt = S)
                let a = f32::from_bits(self.fpr[fs] as u32);
                let b = f32::from_bits(self.fpr[ft] as u32);
                match funct {
                    0x00 => self.fpr[fd] = (a + b).to_bits() as u64, // ADD.S
                    0x01 => self.fpr[fd] = (a - b).to_bits() as u64, // SUB.S
                    0x02 => self.fpr[fd] = (a * b).to_bits() as u64, // MUL.S
                    0x03 => self.fpr[fd] = (a / b).to_bits() as u64, // DIV.S
                    0x04 => self.fpr[fd] = a.sqrt().to_bits() as u64, // SQRT.S
                    0x05 => self.fpr[fd] = a.abs().to_bits() as u64, // ABS.S
                    0x06 => self.fpr[fd] = self.fpr[fs],             // MOV.S
                    0x07 => self.fpr[fd] = (-a).to_bits() as u64,    // NEG.S
                    0x09 => {
                        // TRUNC.L.S - Truncate to Long
                        self.fpr[fd] = (a as i64) as u64;
                    }
                    0x0D => {
                        // TRUNC.W.S - Truncate to Word
                        self.fpr[fd] = (a as i32 as u32) as u64;
                    }
                    0x21 => {
                        // CVT.D.S - Convert Single to Double
                        self.fpr[fd] = (a as f64).to_bits();
                    }
                    0x24 => {
                        // CVT.W.S - Convert Single to Word (uses rounding mode)
                        self.fpr[fd] = (a.round() as i32 as u32) as u64;
                    }
                    0x25 => {
                        // CVT.L.S - Convert Single to Long
                        self.fpr[fd] = (a.round() as i64) as u64;
                    }
                    0x30..=0x3F => {
                        // C.cond.S - Compare Single
                        let cond = funct & 0x0F;
                        let result = match cond {
                            0x00 => false,                              // C.F
                            0x01 => a.is_nan() || b.is_nan(),           // C.UN
                            0x02 => a == b,                             // C.EQ
                            0x03 => a == b || a.is_nan() || b.is_nan(), // C.UEQ
                            0x04 => a < b,                              // C.OLT
                            0x05 => a < b || a.is_nan() || b.is_nan(),  // C.ULT
                            0x06 => a <= b,                             // C.OLE
                            0x07 => a <= b || a.is_nan() || b.is_nan(), // C.ULE
                            0x0A => a == b,                             // C.SEQ
                            0x0C => a < b,                              // C.LT
                            0x0E => a <= b,                             // C.LE
                            _ => false,
                        };
                        if result {
                            self.fcr31 |= 1 << 23;
                        } else {
                            self.fcr31 &= !(1 << 23);
                        }
                    }
                    _ => {}
                }
                self.cycles += 1;
            }
            0x11 => {
                // Double-precision (fmt = D)
                let a = f64::from_bits(self.fpr[fs]);
                let b = f64::from_bits(self.fpr[ft]);
                match funct {
                    0x00 => self.fpr[fd] = (a + b).to_bits(),  // ADD.D
                    0x01 => self.fpr[fd] = (a - b).to_bits(),  // SUB.D
                    0x02 => self.fpr[fd] = (a * b).to_bits(),  // MUL.D
                    0x03 => self.fpr[fd] = (a / b).to_bits(),  // DIV.D
                    0x04 => self.fpr[fd] = a.sqrt().to_bits(), // SQRT.D
                    0x05 => self.fpr[fd] = a.abs().to_bits(),  // ABS.D
                    0x06 => self.fpr[fd] = self.fpr[fs],       // MOV.D
                    0x07 => self.fpr[fd] = (-a).to_bits(),     // NEG.D
                    0x09 => {
                        // TRUNC.L.D
                        self.fpr[fd] = (a as i64) as u64;
                    }
                    0x0D => {
                        // TRUNC.W.D
                        self.fpr[fd] = (a as i32 as u32) as u64;
                    }
                    0x20 => {
                        // CVT.S.D - Convert Double to Single
                        self.fpr[fd] = (a as f32).to_bits() as u64;
                    }
                    0x24 => {
                        // CVT.W.D - Convert Double to Word
                        self.fpr[fd] = (a.round() as i32 as u32) as u64;
                    }
                    0x25 => {
                        // CVT.L.D - Convert Double to Long
                        self.fpr[fd] = (a.round() as i64) as u64;
                    }
                    0x30..=0x3F => {
                        // C.cond.D - Compare Double
                        let cond = funct & 0x0F;
                        let result = match cond {
                            0x00 => false,
                            0x01 => a.is_nan() || b.is_nan(),
                            0x02 => a == b,
                            0x03 => a == b || a.is_nan() || b.is_nan(),
                            0x04 => a < b,
                            0x05 => a < b || a.is_nan() || b.is_nan(),
                            0x06 => a <= b,
                            0x07 => a <= b || a.is_nan() || b.is_nan(),
                            0x0A => a == b,
                            0x0C => a < b,
                            0x0E => a <= b,
                            _ => false,
                        };
                        if result {
                            self.fcr31 |= 1 << 23;
                        } else {
                            self.fcr31 &= !(1 << 23);
                        }
                    }
                    _ => {}
                }
                self.cycles += 1;
            }
            0x14 => {
                // Word fixed-point (fmt = W)
                let int_val = self.fpr[fs] as u32 as i32;
                match funct {
                    0x20 => {
                        // CVT.S.W - Convert Word to Single
                        self.fpr[fd] = (int_val as f32).to_bits() as u64;
                    }
                    0x21 => {
                        // CVT.D.W - Convert Word to Double
                        self.fpr[fd] = (int_val as f64).to_bits();
                    }
                    _ => {}
                }
                self.cycles += 1;
            }
            0x15 => {
                // Long fixed-point (fmt = L)
                let long_val = self.fpr[fs] as i64;
                match funct {
                    0x20 => {
                        // CVT.S.L - Convert Long to Single
                        self.fpr[fd] = (long_val as f32).to_bits() as u64;
                    }
                    0x21 => {
                        // CVT.D.L - Convert Long to Double
                        self.fpr[fd] = (long_val as f64).to_bits();
                    }
                    _ => {}
                }
                self.cycles += 1;
            }
            _ => {
                self.cycles += 1;
            }
        }
    }

    // ============================================================================
    // TLB Instructions (CP0)
    // ============================================================================

    /// Execute TLBR - Read Indexed TLB Entry
    /// Reads TLB entry at index specified by CP0 Index register
    /// and loads it into CP0 EntryHi, EntryLo0, EntryLo1, PageMask
    fn execute_tlbr(&mut self) {
        let index = (self.cp0[CP0_INDEX] & 0x1F) as usize; // Bottom 5 bits

        // Read the TLB entry, or use default if invalid index
        // This matches MIPS behavior where invalid indices still update CP0 registers
        let entry = self.memory.tlb_read_indexed(index).unwrap_or_default();

        // Write to CP0 registers
        // EntryHi: VPN2 (bits 39-13) and ASID (bits 7-0)
        self.cp0[CP0_ENTRYHI] = (entry.vpn2 << 13) | (entry.asid as u64);

        // EntryLo0: Even page (PFN, C, D, V, G)
        self.cp0[CP0_ENTRYLO0] = ((entry.pfn0 as u64) << 6)
            | ((entry.c0 as u64) << 3)
            | ((entry.d0 as u64) << 2)
            | ((entry.v0 as u64) << 1)
            | (entry.global as u64);

        // EntryLo1: Odd page (PFN, C, D, V, G)
        self.cp0[CP0_ENTRYLO1] = ((entry.pfn1 as u64) << 6)
            | ((entry.c1 as u64) << 3)
            | ((entry.d1 as u64) << 2)
            | ((entry.v1 as u64) << 1)
            | (entry.global as u64);

        // PageMask
        self.cp0[CP0_PAGEMASK] = entry.page_mask as u64;

        self.cycles += 1;
    }

    /// Execute TLBWI - Write Indexed TLB Entry
    /// Writes TLB entry from CP0 registers to index specified by CP0 Index register
    fn execute_tlbwi(&mut self) {
        let index = (self.cp0[CP0_INDEX] & 0x1F) as usize; // Bottom 5 bits
        let entry = self.cp0_to_tlb_entry();
        self.memory.tlb_write_indexed(index, entry);
        self.cycles += 1;
    }

    /// Execute TLBWR - Write Random TLB Entry
    /// Writes TLB entry from CP0 registers to random index (from CP0_RANDOM)
    fn execute_tlbwr(&mut self) {
        let entry = self.cp0_to_tlb_entry();
        // Use CP0 Random register (bottom 5 bits) for deterministic randomness
        // Random register is typically in range 8-31 (wired entries are 0-7)
        let random_index = (self.cp0[CP0_RANDOM] & 0x1F) as usize;
        self.memory.tlb_write_random(random_index, entry);
        self.cycles += 1;
    }

    /// Execute TLBP - Probe TLB for Matching Entry
    /// Searches TLB for entry matching CP0 EntryHi
    /// Sets CP0 Index to matching entry index, or sets bit 31 if no match
    fn execute_tlbp(&mut self) {
        let vpn2 = (self.cp0[CP0_ENTRYHI] >> 13) & 0x07FFFFFF;
        let asid = (self.cp0[CP0_ENTRYHI] & 0xFF) as u8;

        if let Some(index) = self.memory.tlb_probe(vpn2, asid) {
            // Found matching entry - set Index register
            self.cp0[CP0_INDEX] = index as u64;
        } else {
            // No match - set bit 31 (probe failure)
            self.cp0[CP0_INDEX] = 0x8000_0000;
        }

        self.cycles += 1;
    }

    /// Convert CP0 registers to TLB entry data
    fn cp0_to_tlb_entry(&self) -> TlbEntryData {
        // Extract from CP0 EntryHi
        let vpn2 = (self.cp0[CP0_ENTRYHI] >> 13) & 0x07FFFFFF;
        let asid = (self.cp0[CP0_ENTRYHI] & 0xFF) as u8;

        // Extract from CP0 EntryLo0 (even page)
        let pfn0 = ((self.cp0[CP0_ENTRYLO0] >> 6) & 0x00FFFFFF) as u32;
        let c0 = ((self.cp0[CP0_ENTRYLO0] >> 3) & 0x7) as u8;
        let d0 = ((self.cp0[CP0_ENTRYLO0] >> 2) & 0x1) != 0;
        let v0 = ((self.cp0[CP0_ENTRYLO0] >> 1) & 0x1) != 0;
        let g0 = (self.cp0[CP0_ENTRYLO0] & 0x1) != 0;

        // Extract from CP0 EntryLo1 (odd page)
        let pfn1 = ((self.cp0[CP0_ENTRYLO1] >> 6) & 0x00FFFFFF) as u32;
        let c1 = ((self.cp0[CP0_ENTRYLO1] >> 3) & 0x7) as u8;
        let d1 = ((self.cp0[CP0_ENTRYLO1] >> 2) & 0x1) != 0;
        let v1 = ((self.cp0[CP0_ENTRYLO1] >> 1) & 0x1) != 0;
        let g1 = (self.cp0[CP0_ENTRYLO1] & 0x1) != 0;

        // Global bit is set if both pages have G=1
        let global = g0 && g1;

        // Extract from CP0 PageMask
        let page_mask = (self.cp0[CP0_PAGEMASK] & 0x01FFE000) as u32;

        TlbEntryData {
            vpn2,
            asid,
            global,
            page_mask,
            pfn0,
            c0,
            d0,
            v0,
            pfn1,
            c1,
            d1,
            v1,
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
        // NOP delay slot
        cpu.memory.write_word(4, 0x00000000);
        cpu.step(); // Execute JR
        assert_eq!(cpu.pc, 4); // At delay slot
        cpu.step(); // Execute delay slot
        assert_eq!(cpu.pc, 0x1000); // Now at target
    }

    #[test]
    fn test_jalr() {
        let mem = ArrayMemory::new();
        let mut cpu = CpuMips::new(mem);
        cpu.pc = 0;
        cpu.gpr[1] = 0x1000;
        // JALR $31, $1
        cpu.memory.write_word(0, 0x0020F809);
        // NOP delay slot
        cpu.memory.write_word(4, 0x00000000);
        cpu.step(); // Execute JALR, PC becomes 4 (delay slot)
        assert_eq!(cpu.pc, 4); // Should be at delay slot
        assert_eq!(cpu.gpr[31], 8); // Return address should be PC+8 from JALR instruction
        cpu.step(); // Execute delay slot NOP, PC becomes target
        assert_eq!(cpu.pc, 0x1000); // Now at target
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

        // BEQ $1, $2, offset=8 (branch target = 4 + 8 = 12)
        cpu.memory.write_word(0, 0x10220002);
        // NOP delay slot
        cpu.memory.write_word(4, 0x00000000);
        cpu.step(); // Execute BEQ
        assert_eq!(cpu.pc, 4); // At delay slot
        cpu.step(); // Execute delay slot
        assert_eq!(cpu.pc, 12); // Branch target: (0 + 4) + 8 = 12

        cpu.pc = 0;
        cpu.gpr[2] = 20;
        // BNE $1, $2, offset=8 (branch target = 4 + 8 = 12)
        cpu.memory.write_word(0, 0x14220002);
        // NOP delay slot
        cpu.memory.write_word(4, 0x00000000);
        cpu.step(); // Execute BNE
        assert_eq!(cpu.pc, 4); // At delay slot
        cpu.step(); // Execute delay slot
        assert_eq!(cpu.pc, 12); // Branch target: (0 + 4) + 8 = 12
    }

    #[test]
    fn test_blez_bgtz() {
        let mem = ArrayMemory::new();
        let mut cpu = CpuMips::new(mem);
        cpu.pc = 0;
        cpu.gpr[1] = 0_u64.wrapping_sub(1); // -1

        // BLEZ $1, offset=8 (should branch)
        cpu.memory.write_word(0, 0x18200002);
        // NOP delay slot
        cpu.memory.write_word(4, 0x00000000);
        cpu.step(); // Execute BLEZ
        assert_eq!(cpu.pc, 4); // At delay slot
        cpu.step(); // Execute delay slot
        assert_eq!(cpu.pc, 12); // Branch target: (0 + 4) + 8 = 12

        cpu.pc = 0;
        cpu.gpr[1] = 10;
        // BGTZ $1, offset=8 (should branch)
        cpu.memory.write_word(0, 0x1C200002);
        // NOP delay slot
        cpu.memory.write_word(4, 0x00000000);
        cpu.step(); // Execute BGTZ
        assert_eq!(cpu.pc, 4); // At delay slot
        cpu.step(); // Execute delay slot
        assert_eq!(cpu.pc, 12); // Branch target: (0 + 4) + 8 = 12
    }

    #[test]
    fn test_branch_likely_not_taken() {
        let mem = ArrayMemory::new();
        let mut cpu = CpuMips::new(mem);

        // Test BEQL not taken - should skip delay slot
        cpu.pc = 0;
        cpu.gpr[1] = 10;
        cpu.gpr[2] = 20; // Not equal, branch not taken

        // BEQL $1, $2, offset=8
        cpu.memory.write_word(0, 0x50220002);
        // ORI $3, $0, 0x1234 (delay slot - should be skipped)
        cpu.memory.write_word(4, 0x34030000 | 0x1234);
        // ORI $4, $0, 0x5678 (next instruction after nullified delay slot)
        cpu.memory.write_word(8, 0x34040000 | 0x5678);

        cpu.step(); // Execute BEQL - not taken, delay slot nullified
        assert_eq!(cpu.pc, 8); // Should skip delay slot, PC = 0 + 4 + 4 = 8
        assert_eq!(cpu.gpr[3], 0); // Delay slot should NOT execute

        cpu.step(); // Execute instruction at PC=8
        assert_eq!(cpu.gpr[4], 0x5678); // This should execute

        // Test BLTZL not taken - should skip delay slot
        cpu.pc = 0;
        cpu.gpr[1] = 10; // Positive, branch not taken
        cpu.gpr[5] = 0;

        // BLTZL $1, offset=8 (REGIMM opcode 0x01, rt=0x02 for BLTZL)
        cpu.memory.write_word(0, 0x04220002);
        // ORI $5, $0, 0xABCD (delay slot - should be skipped)
        cpu.memory.write_word(4, 0x34050000 | 0xABCD);

        cpu.step(); // Execute BLTZL - not taken, delay slot nullified
        assert_eq!(cpu.pc, 8); // Should skip delay slot
        assert_eq!(cpu.gpr[5], 0); // Delay slot should NOT execute
    }

    #[test]
    fn test_branch_likely_taken() {
        let mem = ArrayMemory::new();
        let mut cpu = CpuMips::new(mem);

        // Test BEQL taken - should execute delay slot
        cpu.pc = 0;
        cpu.gpr[1] = 10;
        cpu.gpr[2] = 10; // Equal, branch taken

        // BEQL $1, $2, offset=8
        cpu.memory.write_word(0, 0x50220002);
        // ORI $3, $0, 0x1234 (delay slot - should execute)
        cpu.memory.write_word(4, 0x34030000 | 0x1234);
        // NOP at branch target
        cpu.memory.write_word(12, 0x00000000);

        cpu.step(); // Execute BEQL
        assert_eq!(cpu.pc, 4); // At delay slot

        cpu.step(); // Execute delay slot
        assert_eq!(cpu.gpr[3], 0x1234); // Delay slot SHOULD execute
        assert_eq!(cpu.pc, 12); // Branch target: (0 + 4) + 8 = 12
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
        // NOP delay slot
        cpu.memory.write_word(4, 0x00000000);
        cpu.step(); // Execute J, PC becomes 4 (delay slot)
        assert_eq!(cpu.pc, 4); // Should be at delay slot
        assert!(cpu.is_in_delay_slot()); // Should be marked as in delay slot
        cpu.step(); // Execute delay slot NOP, PC becomes target
        assert_eq!(cpu.pc, 0x4000); // Now at target

        cpu.pc = 0;
        // JAL 0x1000
        cpu.memory.write_word(0, 0x0C001000);
        // NOP delay slot
        cpu.memory.write_word(4, 0x00000000);
        cpu.step(); // Execute JAL, PC becomes 4 (delay slot)
        assert_eq!(cpu.pc, 4); // Should be at delay slot
        assert_eq!(cpu.gpr[31], 8); // Return address should be PC+8 from JAL instruction
        cpu.step(); // Execute delay slot NOP, PC becomes target
        assert_eq!(cpu.pc, 0x4000); // Now at target
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
        cpu.fpr[1] = 10.5_f64.to_bits();
        cpu.fpr[2] = 2.5_f64.to_bits();

        // ADD.D $f3, $f1, $f2
        cpu.memory.write_word(0, 0x462208C0);
        cpu.step();
        assert_eq!(cpu.fpr[3], 13.0_f64.to_bits());

        // SUB.D $f3, $f1, $f2
        cpu.memory.write_word(4, 0x462208C1);
        cpu.step();
        assert_eq!(cpu.fpr[3], 8.0_f64.to_bits());

        // MUL.D $f3, $f1, $f2
        cpu.memory.write_word(8, 0x462208C2);
        cpu.step();
        assert_eq!(cpu.fpr[3], 26.25_f64.to_bits());

        // DIV.D $f3, $f1, $f2
        cpu.memory.write_word(12, 0x462208C3);
        cpu.step();
        assert_eq!(cpu.fpr[3], 4.2_f64.to_bits());
    }
}
