//! ARM7TDMI CPU core implementation
//!
//! The ARM7TDMI is a 32-bit RISC processor used in the Game Boy Advance.
//! It supports two instruction sets:
//! - **ARM** (32-bit): Full instruction set, all features
//! - **Thumb** (16-bit): Compact subset, higher code density
//!
//! ## Architecture Overview
//!
//! - 16 general-purpose 32-bit registers (R0-R15)
//!   - R13: Stack Pointer (SP) by convention
//!   - R14: Link Register (LR) - stores return address
//!   - R15: Program Counter (PC)
//! - Current Program Status Register (CPSR)
//! - Saved Program Status Registers (SPSR) per privileged mode
//! - 7 processor modes with banked registers
//! - 3-stage pipeline (Fetch → Decode → Execute)
//!
//! ## Processor Modes
//!
//! | Mode       | CPSR[4:0] | Description                   |
//! |------------|-----------|-------------------------------|
//! | User       | 0x10      | Normal execution              |
//! | FIQ        | 0x11      | Fast interrupt                |
//! | IRQ        | 0x12      | Normal interrupt              |
//! | Supervisor | 0x13      | Software interrupt (SWI)      |
//! | Abort      | 0x17      | Memory fault                  |
//! | Undefined  | 0x1B      | Undefined instruction         |
//! | System     | 0x1F      | Privileged user mode          |
//!
//! ## CPSR Flags
//!
//! | Bit  | Flag | Description                            |
//! |------|------|----------------------------------------|
//! | 31   | N    | Negative / Less than                   |
//! | 30   | Z    | Zero                                   |
//! | 29   | C    | Carry / Borrow / Extend                |
//! | 28   | V    | Overflow                               |
//! | 7    | I    | IRQ disable                            |
//! | 6    | F    | FIQ disable                            |
//! | 5    | T    | Thumb state (0=ARM, 1=Thumb)           |
//! | 4:0  | M    | Processor mode                         |
//!
//! ## References
//!
//! - ARM7TDMI Technical Reference Manual (ARM DDI 0029G)
//! - ARM Architecture Reference Manual (ARM DDI 0100E)
//! - GBATEK (https://problemkaputt.de/gbatek.htm)

use crate::logging::{log, LogCategory, LogLevel};

// =============================================================================
// Constants
// =============================================================================

// CPSR flag bit positions
const FLAG_N: u32 = 1 << 31; // Negative
const FLAG_Z: u32 = 1 << 30; // Zero
const FLAG_C: u32 = 1 << 29; // Carry
const FLAG_V: u32 = 1 << 28; // Overflow
const FLAG_I: u32 = 1 << 7; // IRQ disable
const FLAG_F: u32 = 1 << 6; // FIQ disable
const FLAG_T: u32 = 1 << 5; // Thumb state

// Processor modes (CPSR bits 4:0)
const MODE_USER: u32 = 0x10;
const MODE_FIQ: u32 = 0x11;
const MODE_IRQ: u32 = 0x12;
const MODE_SUPERVISOR: u32 = 0x13;
const MODE_ABORT: u32 = 0x17;
const MODE_UNDEFINED: u32 = 0x1B;
const MODE_SYSTEM: u32 = 0x1F;
const MODE_MASK: u32 = 0x1F;

// Exception vector addresses
const VECTOR_RESET: u32 = 0x00000000;
const VECTOR_UNDEFINED: u32 = 0x00000004;
const VECTOR_SWI: u32 = 0x00000008;
#[allow(dead_code)] // TODO: Implement prefetch abort exception
const VECTOR_PREFETCH_ABORT: u32 = 0x0000000C;
#[allow(dead_code)] // TODO: Implement data abort exception
const VECTOR_DATA_ABORT: u32 = 0x00000010;
const VECTOR_IRQ: u32 = 0x00000018;
#[allow(dead_code)] // TODO: Implement FIQ exception
const VECTOR_FIQ: u32 = 0x0000001C;

// ARM condition codes (bits 31:28 of instruction)
const COND_EQ: u32 = 0x0; // Z set
const COND_NE: u32 = 0x1; // Z clear
const COND_CS: u32 = 0x2; // C set (HS)
const COND_CC: u32 = 0x3; // C clear (LO)
const COND_MI: u32 = 0x4; // N set
const COND_PL: u32 = 0x5; // N clear
const COND_VS: u32 = 0x6; // V set
const COND_VC: u32 = 0x7; // V clear
const COND_HI: u32 = 0x8; // C set and Z clear
const COND_LS: u32 = 0x9; // C clear or Z set
const COND_GE: u32 = 0xA; // N == V
const COND_LT: u32 = 0xB; // N != V
const COND_GT: u32 = 0xC; // Z clear and N == V
const COND_LE: u32 = 0xD; // Z set or N != V
const COND_AL: u32 = 0xE; // Always

// =============================================================================
// Memory Interface
// =============================================================================

/// Memory interface trait for the ARM7TDMI CPU
///
/// Systems using the ARM7TDMI must implement this trait to provide memory access.
/// The GBA memory map is implemented in the system crate, not here.
pub trait MemoryArm7 {
    /// Read a byte from memory
    fn read_byte(&self, addr: u32) -> u8;

    /// Read a halfword (16-bit) from memory (must be 2-byte aligned)
    fn read_halfword(&self, addr: u32) -> u16;

    /// Read a word (32-bit) from memory (must be 4-byte aligned)
    fn read_word(&self, addr: u32) -> u32;

    /// Write a byte to memory
    fn write_byte(&mut self, addr: u32, val: u8);

    /// Write a halfword (16-bit) to memory
    fn write_halfword(&mut self, addr: u32, val: u16);

    /// Write a word (32-bit) to memory
    fn write_word(&mut self, addr: u32, val: u32);

    /// Check if an IRQ is pending (active and not masked by hardware)
    fn irq_pending(&self) -> bool {
        false
    }
}

// =============================================================================
// CPU State
// =============================================================================

/// ARM7TDMI processor modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessorMode {
    User,
    Fiq,
    Irq,
    Supervisor,
    Abort,
    Undefined,
    System,
}

impl ProcessorMode {
    /// Convert mode bits (CPSR[4:0]) to ProcessorMode
    pub fn from_bits(bits: u32) -> Option<Self> {
        match bits & MODE_MASK {
            MODE_USER => Some(Self::User),
            MODE_FIQ => Some(Self::Fiq),
            MODE_IRQ => Some(Self::Irq),
            MODE_SUPERVISOR => Some(Self::Supervisor),
            MODE_ABORT => Some(Self::Abort),
            MODE_UNDEFINED => Some(Self::Undefined),
            MODE_SYSTEM => Some(Self::System),
            _ => None,
        }
    }

    /// Convert ProcessorMode to mode bits
    pub fn to_bits(self) -> u32 {
        match self {
            Self::User => MODE_USER,
            Self::Fiq => MODE_FIQ,
            Self::Irq => MODE_IRQ,
            Self::Supervisor => MODE_SUPERVISOR,
            Self::Abort => MODE_ABORT,
            Self::Undefined => MODE_UNDEFINED,
            Self::System => MODE_SYSTEM,
        }
    }
}

/// ARM7TDMI CPU state and execution engine
///
/// Generic over the memory interface `M`, following the same pattern as
/// other CPU cores in this project (Cpu6502, CpuZ80, CpuMips).
#[derive(Debug)]
pub struct Arm7Tdmi<M: MemoryArm7> {
    // ---- General-purpose registers ----
    /// Registers R0-R15 for current mode
    /// R13 = SP, R14 = LR, R15 = PC
    pub gpr: [u32; 16],

    // ---- Banked registers ----
    // FIQ mode has banked R8-R14 + SPSR
    /// FIQ banked R8-R12
    fiq_r8_r12: [u32; 5],
    /// User/System R8-R12 (saved when switching to FIQ)
    usr_r8_r12: [u32; 5],
    /// FIQ banked R13 (SP) and R14 (LR)
    fiq_r13_r14: [u32; 2],
    /// IRQ banked R13 (SP) and R14 (LR)
    irq_r13_r14: [u32; 2],
    /// Supervisor banked R13 (SP) and R14 (LR)
    svc_r13_r14: [u32; 2],
    /// Abort banked R13 (SP) and R14 (LR)
    abt_r13_r14: [u32; 2],
    /// Undefined banked R13 (SP) and R14 (LR)
    und_r13_r14: [u32; 2],
    /// User/System R13 (SP) and R14 (LR)
    usr_r13_r14: [u32; 2],

    // ---- Status registers ----
    /// Current Program Status Register
    pub cpsr: u32,
    /// Saved PSRs (one per exception mode)
    spsr_fiq: u32,
    spsr_irq: u32,
    spsr_svc: u32,
    spsr_abt: u32,
    spsr_und: u32,

    // ---- Pipeline state ----
    /// Whether we just flushed the pipeline (branch, exception, etc.)
    pipeline_flushed: bool,

    // ---- Cycle counting ----
    /// Total cycles executed
    pub cycles: u64,

    // ---- Memory interface ----
    /// Memory bus (owned, like other CPU cores in this project)
    pub memory: M,
}

impl<M: MemoryArm7> Arm7Tdmi<M> {
    /// Create a new ARM7TDMI CPU in Supervisor mode (post-reset state).
    ///
    /// After reset:
    /// - PC = 0x00000000 (reset vector)
    /// - CPSR = Supervisor mode, ARM state, IRQ+FIQ disabled
    /// - All registers are undefined (we zero them)
    pub fn new(memory: M) -> Self {
        Self {
            gpr: [0; 16],
            fiq_r8_r12: [0; 5],
            usr_r8_r12: [0; 5],
            fiq_r13_r14: [0; 2],
            irq_r13_r14: [0; 2],
            svc_r13_r14: [0; 2],
            abt_r13_r14: [0; 2],
            und_r13_r14: [0; 2],
            usr_r13_r14: [0; 2],
            cpsr: MODE_SUPERVISOR | FLAG_I | FLAG_F, // Supervisor, ARM, IRQ+FIQ disabled
            spsr_fiq: 0,
            spsr_irq: 0,
            spsr_svc: 0,
            spsr_abt: 0,
            spsr_und: 0,
            pipeline_flushed: true,
            cycles: 0,
            memory,
        }
    }

    /// Reset the CPU to initial power-on state
    pub fn reset(&mut self) {
        self.gpr = [0; 16];
        self.fiq_r8_r12 = [0; 5];
        self.usr_r8_r12 = [0; 5];
        self.fiq_r13_r14 = [0; 2];
        self.irq_r13_r14 = [0; 2];
        self.svc_r13_r14 = [0; 2];
        self.abt_r13_r14 = [0; 2];
        self.und_r13_r14 = [0; 2];
        self.usr_r13_r14 = [0; 2];
        self.cpsr = MODE_SUPERVISOR | FLAG_I | FLAG_F;
        self.spsr_fiq = 0;
        self.spsr_irq = 0;
        self.spsr_svc = 0;
        self.spsr_abt = 0;
        self.spsr_und = 0;
        self.pipeline_flushed = true;
        self.cycles = 0;
        // PC = reset vector
        self.gpr[15] = VECTOR_RESET;
    }

    // =========================================================================
    // Register access helpers
    // =========================================================================

    /// Get the current processor mode
    #[inline]
    pub fn current_mode(&self) -> ProcessorMode {
        ProcessorMode::from_bits(self.cpsr).unwrap_or(ProcessorMode::System)
    }

    /// Check if CPU is in Thumb state
    #[inline]
    pub fn is_thumb(&self) -> bool {
        self.cpsr & FLAG_T != 0
    }

    /// Get the current PC value.
    /// In ARM state, PC reads as current instruction + 8.
    /// In Thumb state, PC reads as current instruction + 4.
    /// But we track the actual address of the instruction being executed.
    #[inline]
    pub fn pc(&self) -> u32 {
        self.gpr[15]
    }

    /// Set PC and flush the pipeline
    #[inline]
    fn set_pc(&mut self, addr: u32) {
        self.gpr[15] = addr;
        self.pipeline_flushed = true;
    }

    // ---- Flag accessors ----

    #[inline]
    fn flag_n(&self) -> bool {
        self.cpsr & FLAG_N != 0
    }

    #[inline]
    fn flag_z(&self) -> bool {
        self.cpsr & FLAG_Z != 0
    }

    #[inline]
    fn flag_c(&self) -> bool {
        self.cpsr & FLAG_C != 0
    }

    #[inline]
    fn flag_v(&self) -> bool {
        self.cpsr & FLAG_V != 0
    }

    #[inline]
    fn set_flag(&mut self, flag: u32, val: bool) {
        if val {
            self.cpsr |= flag;
        } else {
            self.cpsr &= !flag;
        }
    }

    /// Set N and Z flags based on a 32-bit result
    #[inline]
    fn set_nz(&mut self, result: u32) {
        self.set_flag(FLAG_N, result & 0x80000000 != 0);
        self.set_flag(FLAG_Z, result == 0);
    }

    // ---- SPSR access ----

    /// Get the SPSR for the current mode
    fn get_spsr(&self) -> u32 {
        match self.current_mode() {
            ProcessorMode::Fiq => self.spsr_fiq,
            ProcessorMode::Irq => self.spsr_irq,
            ProcessorMode::Supervisor => self.spsr_svc,
            ProcessorMode::Abort => self.spsr_abt,
            ProcessorMode::Undefined => self.spsr_und,
            // User and System modes don't have SPSR
            _ => self.cpsr,
        }
    }

    /// Set the SPSR for the current mode
    fn set_spsr(&mut self, val: u32) {
        match self.current_mode() {
            ProcessorMode::Fiq => self.spsr_fiq = val,
            ProcessorMode::Irq => self.spsr_irq = val,
            ProcessorMode::Supervisor => self.spsr_svc = val,
            ProcessorMode::Abort => self.spsr_abt = val,
            ProcessorMode::Undefined => self.spsr_und = val,
            // User and System modes - writes to SPSR are ignored
            _ => {}
        }
    }

    // ---- Mode switching ----

    /// Switch processor mode, banking/restoring registers as needed.
    ///
    /// The ARM7TDMI has banked registers per mode:
    /// - FIQ: R8-R14 banked
    /// - IRQ/SVC/ABT/UND: R13-R14 banked
    /// - User/System: share registers
    fn switch_mode(&mut self, new_mode: ProcessorMode) {
        let old_mode = self.current_mode();
        if old_mode == new_mode {
            return;
        }

        // Save current R13/R14 to old mode bank
        match old_mode {
            ProcessorMode::User | ProcessorMode::System => {
                self.usr_r13_r14 = [self.gpr[13], self.gpr[14]];
            }
            ProcessorMode::Fiq => {
                self.fiq_r13_r14 = [self.gpr[13], self.gpr[14]];
                // Also save FIQ's R8-R12
                self.fiq_r8_r12
                    .copy_from_slice(&self.gpr[8..13]);
            }
            ProcessorMode::Irq => {
                self.irq_r13_r14 = [self.gpr[13], self.gpr[14]];
            }
            ProcessorMode::Supervisor => {
                self.svc_r13_r14 = [self.gpr[13], self.gpr[14]];
            }
            ProcessorMode::Abort => {
                self.abt_r13_r14 = [self.gpr[13], self.gpr[14]];
            }
            ProcessorMode::Undefined => {
                self.und_r13_r14 = [self.gpr[13], self.gpr[14]];
            }
        }

        // If leaving FIQ, restore user R8-R12; if entering, save them
        if old_mode == ProcessorMode::Fiq && new_mode != ProcessorMode::Fiq {
            // Restore user R8-R12
            self.gpr[8..13].copy_from_slice(&self.usr_r8_r12);
        } else if old_mode != ProcessorMode::Fiq && new_mode == ProcessorMode::Fiq {
            // Save user R8-R12 and load FIQ R8-R12
            self.usr_r8_r12.copy_from_slice(&self.gpr[8..13]);
            self.gpr[8..13].copy_from_slice(&self.fiq_r8_r12);
        }

        // Load new mode's R13/R14
        match new_mode {
            ProcessorMode::User | ProcessorMode::System => {
                [self.gpr[13], self.gpr[14]] = self.usr_r13_r14;
            }
            ProcessorMode::Fiq => {
                [self.gpr[13], self.gpr[14]] = self.fiq_r13_r14;
            }
            ProcessorMode::Irq => {
                [self.gpr[13], self.gpr[14]] = self.irq_r13_r14;
            }
            ProcessorMode::Supervisor => {
                [self.gpr[13], self.gpr[14]] = self.svc_r13_r14;
            }
            ProcessorMode::Abort => {
                [self.gpr[13], self.gpr[14]] = self.abt_r13_r14;
            }
            ProcessorMode::Undefined => {
                [self.gpr[13], self.gpr[14]] = self.und_r13_r14;
            }
        }

        // Update CPSR mode bits
        self.cpsr = (self.cpsr & !MODE_MASK) | new_mode.to_bits();
    }

    // =========================================================================
    // Condition evaluation
    // =========================================================================

    /// Evaluate an ARM condition code (bits 31:28 of instruction).
    /// Returns true if the condition is met.
    #[inline]
    fn check_condition(&self, cond: u32) -> bool {
        match cond {
            COND_EQ => self.flag_z(),
            COND_NE => !self.flag_z(),
            COND_CS => self.flag_c(),
            COND_CC => !self.flag_c(),
            COND_MI => self.flag_n(),
            COND_PL => !self.flag_n(),
            COND_VS => self.flag_v(),
            COND_VC => !self.flag_v(),
            COND_HI => self.flag_c() && !self.flag_z(),
            COND_LS => !self.flag_c() || self.flag_z(),
            COND_GE => self.flag_n() == self.flag_v(),
            COND_LT => self.flag_n() != self.flag_v(),
            COND_GT => !self.flag_z() && (self.flag_n() == self.flag_v()),
            COND_LE => self.flag_z() || (self.flag_n() != self.flag_v()),
            COND_AL => true,
            0xF => true, // NV (ARMv4: unpredictable, but treat as AL for compatibility)
            _ => unreachable!(),
        }
    }

    // =========================================================================
    // Exception handling
    // =========================================================================

    /// Enter an exception (IRQ, FIQ, SWI, etc.)
    ///
    /// Common sequence for all exceptions:
    /// 1. Save CPSR to SPSR of new mode
    /// 2. Switch to new mode
    /// 3. Set LR to return address
    /// 4. Disable IRQs (and FIQs for FIQ/Reset)
    /// 5. Switch to ARM state
    /// 6. Jump to exception vector
    fn enter_exception(&mut self, vector: u32, new_mode: ProcessorMode, lr_offset: u32) {
        let old_cpsr = self.cpsr;
        let return_addr = self.gpr[15].wrapping_sub(lr_offset);

        // Switch to new mode (this banks registers)
        self.switch_mode(new_mode);

        // Save old CPSR to SPSR of new mode
        self.set_spsr(old_cpsr);

        // Set LR to return address
        self.gpr[14] = return_addr;

        // Disable IRQs
        self.cpsr |= FLAG_I;

        // FIQ and Reset also disable FIQs
        if new_mode == ProcessorMode::Fiq || vector == VECTOR_RESET {
            self.cpsr |= FLAG_F;
        }

        // Switch to ARM state
        self.cpsr &= !FLAG_T;

        // Jump to vector
        self.set_pc(vector);
    }

    /// Handle a Software Interrupt (SWI)
    fn handle_swi(&mut self) {
        let lr_offset = if self.is_thumb() { 2 } else { 4 };
        self.enter_exception(VECTOR_SWI, ProcessorMode::Supervisor, lr_offset);
    }

    /// Handle an IRQ
    fn handle_irq(&mut self) {
        // LR = PC of next instruction + 4 (ARM) or + 2 (Thumb)
        // The +4 is because the pipeline has advanced
        let lr_offset = 0;
        self.enter_exception(VECTOR_IRQ, ProcessorMode::Irq, lr_offset);
        // Note: The actual return address calculation is handled by the
        // instruction that was about to execute. The CPU saves PC+4 for ARM
        // or PC+4 for Thumb into LR, and SUBS PC, LR, #4 is used to return.
    }

    /// Handle an undefined instruction exception
    fn handle_undefined(&mut self) {
        let lr_offset = if self.is_thumb() { 2 } else { 4 };
        self.enter_exception(VECTOR_UNDEFINED, ProcessorMode::Undefined, lr_offset);
    }

    /// Check and handle pending interrupts
    fn check_interrupts(&mut self) -> bool {
        // Check IRQ: must be enabled (I flag clear) and pending from hardware
        if self.cpsr & FLAG_I == 0 && self.memory.irq_pending() {
            log(LogCategory::Interrupts, LogLevel::Debug, || "ARM7: IRQ taken".to_string());
            self.handle_irq();
            return true;
        }
        false
    }

    // =========================================================================
    // Barrel shifter
    // =========================================================================

    /// Perform a barrel shift operation, returning (result, carry_out).
    ///
    /// The ARM barrel shifter supports:
    /// - LSL (Logical Shift Left)
    /// - LSR (Logical Shift Right)
    /// - ASR (Arithmetic Shift Right)
    /// - ROR (Rotate Right)
    /// - RRX (Rotate Right Extended, shift amount = 0 with ROR)
    fn barrel_shift(&self, value: u32, shift_type: u32, amount: u32, carry_in: bool) -> (u32, bool) {
        match shift_type {
            0b00 => {
                // LSL
                if amount == 0 {
                    (value, carry_in)
                } else if amount < 32 {
                    let carry = (value >> (32 - amount)) & 1 != 0;
                    (value << amount, carry)
                } else if amount == 32 {
                    (0, value & 1 != 0)
                } else {
                    (0, false)
                }
            }
            0b01 => {
                // LSR
                if amount == 0 {
                    // LSR #0 encodes as LSR #32
                    (0, value >> 31 != 0)
                } else if amount < 32 {
                    let carry = (value >> (amount - 1)) & 1 != 0;
                    (value >> amount, carry)
                } else if amount == 32 {
                    (0, value >> 31 != 0)
                } else {
                    (0, false)
                }
            }
            0b10 => {
                // ASR
                if amount == 0 {
                    // ASR #0 encodes as ASR #32
                    let sign = value as i32 >> 31;
                    (sign as u32, value >> 31 != 0)
                } else if amount < 32 {
                    let carry = (value >> (amount - 1)) & 1 != 0;
                    ((value as i32 >> amount) as u32, carry)
                } else {
                    let sign = value as i32 >> 31;
                    (sign as u32, value >> 31 != 0)
                }
            }
            0b11 => {
                // ROR
                if amount == 0 {
                    // RRX (Rotate Right Extended): 33-bit rotate through carry
                    let carry = value & 1 != 0;
                    let result = (value >> 1) | if carry_in { 0x80000000 } else { 0 };
                    (result, carry)
                } else {
                    let amount = amount & 31;
                    if amount == 0 {
                        (value, value >> 31 != 0)
                    } else {
                        let result = value.rotate_right(amount);
                        let carry = result >> 31 != 0;
                        (result, carry)
                    }
                }
            }
            _ => unreachable!(),
        }
    }

    /// Calculate the shifter operand for a data processing instruction.
    /// Returns (value, carry_out).
    fn shifter_operand(&self, instr: u32) -> (u32, bool) {
        let carry_in = self.flag_c();

        if instr & (1 << 25) != 0 {
            // Immediate operand: 8-bit value rotated right by 2 * rotate
            let imm = instr & 0xFF;
            let rotate = (instr >> 8) & 0xF;
            if rotate == 0 {
                (imm, carry_in)
            } else {
                let result = imm.rotate_right(rotate * 2);
                let carry = result >> 31 != 0;
                (result, carry)
            }
        } else {
            // Register operand with shift
            let rm = (instr & 0xF) as usize;
            let rm_val = if rm == 15 {
                // PC + 8 in ARM mode (current instruction + 8 due to pipeline)
                self.gpr[15].wrapping_add(8)
            } else {
                self.gpr[rm]
            };

            let shift_type = (instr >> 5) & 0x3;

            if instr & (1 << 4) != 0 {
                // Register-specified shift amount (Rs)
                let rs = ((instr >> 8) & 0xF) as usize;
                let shift_amount = self.gpr[rs] & 0xFF;
                self.barrel_shift(rm_val, shift_type, shift_amount, carry_in)
            } else {
                // Immediate shift amount
                let shift_amount = (instr >> 7) & 0x1F;
                self.barrel_shift(rm_val, shift_type, shift_amount, carry_in)
            }
        }
    }

    // =========================================================================
    // Main execution
    // =========================================================================

    /// Execute a single instruction and return cycles consumed.
    ///
    /// The ARM7TDMI uses a 3-stage pipeline, but we model it simply:
    /// - PC points to the instruction being executed
    /// - After execution, PC advances to next instruction (unless branched)
    pub fn step(&mut self) -> u32 {
        let start_cycles = self.cycles;

        // Check for pending interrupts
        if self.check_interrupts() {
            self.cycles += 3; // IRQ entry takes ~3 cycles
            return (self.cycles - start_cycles) as u32;
        }

        if self.is_thumb() {
            self.step_thumb();
        } else {
            self.step_arm();
        }

        (self.cycles - start_cycles) as u32
    }

    // =========================================================================
    // ARM mode execution (32-bit instructions)
    // =========================================================================

    /// Execute one ARM (32-bit) instruction
    fn step_arm(&mut self) {
        let pc = self.gpr[15];
        let instr = self.memory.read_word(pc & !3); // Force word-aligned

        log(
            LogCategory::CPU,
            LogLevel::Trace,
            || format!("ARM: PC={:08X} instr={:08X}", pc, instr),
        );

        // Advance PC past this instruction (pipeline)
        self.gpr[15] = pc.wrapping_add(4);

        // Check condition code (bits 31:28)
        let cond = (instr >> 28) & 0xF;
        if !self.check_condition(cond) {
            self.cycles += 1; // 1S cycle for skipped instruction
            return;
        }

        // Decode instruction format based on bits 27:20 and 7:4
        let bits_27_20 = (instr >> 20) & 0xFF;
        let bits_7_4 = (instr >> 4) & 0xF;

        match (bits_27_20 >> 5, bits_7_4) {
            // Branch and exchange (BX)
            (0b000, 0b0001) if bits_27_20 & 0x1F == 0x12 => {
                self.arm_branch_exchange(instr);
            }

            // Multiply instructions
            (0b000, bits) if bits & 0b1001 == 0b1001 && bits_27_20 & 0xFC == 0x00 => {
                self.arm_multiply(instr);
            }

            // Multiply long
            (0b000, bits) if bits & 0b1001 == 0b1001 && bits_27_20 & 0xF8 == 0x08 => {
                self.arm_multiply_long(instr);
            }

            // Single data swap (SWP)
            (0b000, 0b1001) if bits_27_20 & 0xFB == 0x10 => {
                self.arm_swap(instr);
            }

            // Halfword data transfer (register offset)
            (0b000, bits)
                if bits & 0b1001 == 0b1001
                    && bits & 0b0110 != 0
                    && bits_27_20 & 0xE4 == 0x00 =>
            {
                self.arm_halfword_transfer(instr);
            }

            // Halfword data transfer (immediate offset)
            (0b000, bits)
                if bits & 0b1001 == 0b1001
                    && bits & 0b0110 != 0
                    && bits_27_20 & 0xE4 == 0x04 =>
            {
                self.arm_halfword_transfer(instr);
            }

            // MRS (transfer PSR to register)
            (0b000, 0b0000) if bits_27_20 & 0x1F == 0x10 => {
                self.arm_mrs(instr);
            }

            // MSR (transfer register/immediate to PSR)
            (0b000, 0b0000) if bits_27_20 & 0x1B == 0x12 => {
                self.arm_msr(instr);
            }
            (0b001, _) if bits_27_20 & 0x1B == 0x12 => {
                self.arm_msr(instr);
            }

            // Data processing (register shift by immediate or register)
            (0b000, _) | (0b001, _) => {
                self.arm_data_processing(instr);
            }

            // Single data transfer (LDR/STR)
            (0b010, _) | (0b011, _) => {
                self.arm_single_data_transfer(instr);
            }

            // Block data transfer (LDM/STM)
            (0b100, _) => {
                self.arm_block_data_transfer(instr);
            }

            // Branch / Branch with Link
            (0b101, _) => {
                self.arm_branch(instr);
            }

            // Software Interrupt (SWI)
            (0b111, _) if bits_27_20 & 0x10 != 0 => {
                self.handle_swi();
                self.cycles += 3;
            }

            _ => {
                // Undefined instruction
                log(
                    LogCategory::CPU,
                    LogLevel::Warn,
                    || format!(
                        "ARM: Undefined instruction {:08X} at PC={:08X}",
                        instr, pc
                    ),
                );
                self.handle_undefined();
                self.cycles += 1;
            }
        }
    }

    // ---- ARM: Data Processing ----

    /// ARM data-processing instructions (AND, EOR, SUB, RSB, ADD, ADC, SBC, RSC,
    /// TST, TEQ, CMP, CMN, ORR, MOV, BIC, MVN)
    fn arm_data_processing(&mut self, instr: u32) {
        let opcode = (instr >> 21) & 0xF;
        let set_flags = instr & (1 << 20) != 0;
        let rn = ((instr >> 16) & 0xF) as usize;
        let rd = ((instr >> 12) & 0xF) as usize;

        let rn_val = if rn == 15 {
            self.gpr[15].wrapping_add(4) // PC + 8 from original instruction address
        } else {
            self.gpr[rn]
        };

        let (op2, shifter_carry) = self.shifter_operand(instr);

        let (result, carry, overflow) = match opcode {
            0x0 => {
                // AND
                (rn_val & op2, shifter_carry, self.flag_v())
            }
            0x1 => {
                // EOR
                (rn_val ^ op2, shifter_carry, self.flag_v())
            }
            0x2 => {
                // SUB
                let (result, borrow) = rn_val.overflowing_sub(op2);
                let v = ((rn_val ^ op2) & (rn_val ^ result)) >> 31 != 0;
                (result, !borrow, v)
            }
            0x3 => {
                // RSB (Reverse Subtract)
                let (result, borrow) = op2.overflowing_sub(rn_val);
                let v = ((op2 ^ rn_val) & (op2 ^ result)) >> 31 != 0;
                (result, !borrow, v)
            }
            0x4 => {
                // ADD
                let (result, carry) = rn_val.overflowing_add(op2);
                let v = (!(rn_val ^ op2) & (rn_val ^ result)) >> 31 != 0;
                (result, carry, v)
            }
            0x5 => {
                // ADC (Add with Carry)
                let c_in = if self.flag_c() { 1u32 } else { 0 };
                let (r1, c1) = rn_val.overflowing_add(op2);
                let (result, c2) = r1.overflowing_add(c_in);
                let v = (!(rn_val ^ op2) & (rn_val ^ result)) >> 31 != 0;
                (result, c1 || c2, v)
            }
            0x6 => {
                // SBC (Subtract with Carry)
                let c_in = if self.flag_c() { 0u32 } else { 1 };
                let (r1, b1) = rn_val.overflowing_sub(op2);
                let (result, b2) = r1.overflowing_sub(c_in);
                let v = ((rn_val ^ op2) & (rn_val ^ result)) >> 31 != 0;
                (result, !(b1 || b2), v)
            }
            0x7 => {
                // RSC (Reverse Subtract with Carry)
                let c_in = if self.flag_c() { 0u32 } else { 1 };
                let (r1, b1) = op2.overflowing_sub(rn_val);
                let (result, b2) = r1.overflowing_sub(c_in);
                let v = ((op2 ^ rn_val) & (op2 ^ result)) >> 31 != 0;
                (result, !(b1 || b2), v)
            }
            0x8 => {
                // TST (Test) - like AND but result discarded
                let result = rn_val & op2;
                if set_flags {
                    self.set_nz(result);
                    self.set_flag(FLAG_C, shifter_carry);
                }
                self.cycles += 1;
                return;
            }
            0x9 => {
                // TEQ (Test Equivalence) - like EOR but result discarded
                let result = rn_val ^ op2;
                if set_flags {
                    self.set_nz(result);
                    self.set_flag(FLAG_C, shifter_carry);
                }
                self.cycles += 1;
                return;
            }
            0xA => {
                // CMP (Compare) - like SUB but result discarded
                let (result, borrow) = rn_val.overflowing_sub(op2);
                if set_flags {
                    self.set_nz(result);
                    self.set_flag(FLAG_C, !borrow);
                    let v = ((rn_val ^ op2) & (rn_val ^ result)) >> 31 != 0;
                    self.set_flag(FLAG_V, v);
                }
                self.cycles += 1;
                return;
            }
            0xB => {
                // CMN (Compare Negative) - like ADD but result discarded
                let (result, carry) = rn_val.overflowing_add(op2);
                if set_flags {
                    self.set_nz(result);
                    self.set_flag(FLAG_C, carry);
                    let v = (!(rn_val ^ op2) & (rn_val ^ result)) >> 31 != 0;
                    self.set_flag(FLAG_V, v);
                }
                self.cycles += 1;
                return;
            }
            0xC => {
                // ORR
                (rn_val | op2, shifter_carry, self.flag_v())
            }
            0xD => {
                // MOV
                (op2, shifter_carry, self.flag_v())
            }
            0xE => {
                // BIC (Bit Clear)
                (rn_val & !op2, shifter_carry, self.flag_v())
            }
            0xF => {
                // MVN (Move Not)
                (!op2, shifter_carry, self.flag_v())
            }
            _ => unreachable!(),
        };

        // Write result to Rd
        if rd == 15 {
            self.set_pc(result);
            if set_flags {
                // MOVS/SUBS with Rd=PC: restore CPSR from SPSR (exception return)
                let spsr = self.get_spsr();
                let new_mode =
                    ProcessorMode::from_bits(spsr).unwrap_or(ProcessorMode::System);
                self.switch_mode(new_mode);
                self.cpsr = spsr;
            }
        } else {
            self.gpr[rd] = result;
            if set_flags {
                self.set_nz(result);
                self.set_flag(FLAG_C, carry);
                self.set_flag(FLAG_V, overflow);
            }
        }

        self.cycles += 1; // 1S cycle
    }

    // ---- ARM: Branch ----

    /// ARM branch (B) and branch with link (BL)
    fn arm_branch(&mut self, instr: u32) {
        let link = instr & (1 << 24) != 0;
        // 24-bit signed offset, shifted left 2
        let offset = ((instr & 0x00FFFFFF) as i32) << 8 >> 6; // sign-extend and shift

        if link {
            // BL: save return address in LR
            self.gpr[14] = self.gpr[15]; // PC already advanced by 4
        }

        let target = (self.gpr[15] as i32).wrapping_add(offset).wrapping_add(4) as u32;
        self.set_pc(target);
        self.cycles += 3; // 2S + 1N
    }

    /// ARM branch and exchange (BX)
    fn arm_branch_exchange(&mut self, instr: u32) {
        let rm = (instr & 0xF) as usize;
        let addr = self.gpr[rm];

        // Bit 0 determines ARM/Thumb state
        if addr & 1 != 0 {
            self.cpsr |= FLAG_T; // Switch to Thumb
            self.set_pc(addr & !1);
        } else {
            self.cpsr &= !FLAG_T; // Stay in ARM
            self.set_pc(addr & !3);
        }

        self.cycles += 3; // 2S + 1N
    }

    // ---- ARM: Multiply ----

    /// ARM multiply (MUL, MLA)
    fn arm_multiply(&mut self, instr: u32) {
        let accumulate = instr & (1 << 21) != 0;
        let set_flags = instr & (1 << 20) != 0;
        let rd = ((instr >> 16) & 0xF) as usize;
        let rn = ((instr >> 12) & 0xF) as usize;
        let rs = ((instr >> 8) & 0xF) as usize;
        let rm = (instr & 0xF) as usize;

        let result = if accumulate {
            // MLA: Rd = Rm * Rs + Rn
            self.gpr[rm]
                .wrapping_mul(self.gpr[rs])
                .wrapping_add(self.gpr[rn])
        } else {
            // MUL: Rd = Rm * Rs
            self.gpr[rm].wrapping_mul(self.gpr[rs])
        };

        self.gpr[rd] = result;

        if set_flags {
            self.set_nz(result);
            // C is destroyed (unpredictable on ARMv4)
        }

        // Multiply timing depends on Rs value (1-4 cycles)
        self.cycles += 2; // Minimum: 1S + 1I
    }

    /// ARM multiply long (UMULL, UMLAL, SMULL, SMLAL)
    fn arm_multiply_long(&mut self, instr: u32) {
        let signed = instr & (1 << 22) != 0;
        let accumulate = instr & (1 << 21) != 0;
        let set_flags = instr & (1 << 20) != 0;
        let rd_hi = ((instr >> 16) & 0xF) as usize;
        let rd_lo = ((instr >> 12) & 0xF) as usize;
        let rs = ((instr >> 8) & 0xF) as usize;
        let rm = (instr & 0xF) as usize;

        let result: u64 = if signed {
            let a = self.gpr[rm] as i32 as i64;
            let b = self.gpr[rs] as i32 as i64;
            if accumulate {
                let acc = ((self.gpr[rd_hi] as u64) << 32) | (self.gpr[rd_lo] as u64);
                (a.wrapping_mul(b) as u64).wrapping_add(acc)
            } else {
                a.wrapping_mul(b) as u64
            }
        } else {
            let a = self.gpr[rm] as u64;
            let b = self.gpr[rs] as u64;
            if accumulate {
                let acc = ((self.gpr[rd_hi] as u64) << 32) | (self.gpr[rd_lo] as u64);
                a.wrapping_mul(b).wrapping_add(acc)
            } else {
                a.wrapping_mul(b)
            }
        };

        self.gpr[rd_lo] = result as u32;
        self.gpr[rd_hi] = (result >> 32) as u32;

        if set_flags {
            self.set_flag(FLAG_N, result >> 63 != 0);
            self.set_flag(FLAG_Z, result == 0);
        }

        self.cycles += 3; // 1S + 2I minimum
    }

    // ---- ARM: Single Data Transfer (LDR/STR) ----

    /// ARM single data transfer (LDR, STR, LDRB, STRB)
    fn arm_single_data_transfer(&mut self, instr: u32) {
        let immediate = instr & (1 << 25) == 0; // Bit 25=0 means immediate offset
        let pre_index = instr & (1 << 24) != 0;
        let add_offset = instr & (1 << 23) != 0;
        let byte_transfer = instr & (1 << 22) != 0;
        let write_back = instr & (1 << 21) != 0;
        let is_load = instr & (1 << 20) != 0;
        let rn = ((instr >> 16) & 0xF) as usize;
        let rd = ((instr >> 12) & 0xF) as usize;

        let base = if rn == 15 {
            self.gpr[15].wrapping_add(4) // PC + 8
        } else {
            self.gpr[rn]
        };

        // Calculate offset
        let offset = if immediate {
            instr & 0xFFF
        } else {
            let rm = (instr & 0xF) as usize;
            let shift_type = (instr >> 5) & 0x3;
            let shift_amount = (instr >> 7) & 0x1F;
            let (shifted, _) =
                self.barrel_shift(self.gpr[rm], shift_type, shift_amount, self.flag_c());
            shifted
        };

        let addr = if pre_index {
            if add_offset {
                base.wrapping_add(offset)
            } else {
                base.wrapping_sub(offset)
            }
        } else {
            base
        };

        if is_load {
            let val = if byte_transfer {
                self.memory.read_byte(addr) as u32
            } else {
                // Word load: handle misaligned reads with rotation (ARM7TDMI behavior)
                let aligned_addr = addr & !3;
                let val = self.memory.read_word(aligned_addr);
                let rotate = (addr & 3) * 8;
                if rotate != 0 {
                    val.rotate_right(rotate)
                } else {
                    val
                }
            };

            if rd == 15 {
                self.set_pc(val & !3);
            } else {
                self.gpr[rd] = val;
            }
            self.cycles += 3; // 1S + 1N + 1I
        } else {
            let val = if rd == 15 {
                self.gpr[15].wrapping_add(4) // PC + 12
            } else {
                self.gpr[rd]
            };

            if byte_transfer {
                self.memory.write_byte(addr, val as u8);
            } else {
                self.memory.write_word(addr & !3, val);
            }
            self.cycles += 2; // 2N
        }

        // Post-index or write-back
        if !pre_index || write_back {
            let final_addr = if !pre_index {
                if add_offset {
                    base.wrapping_add(offset)
                } else {
                    base.wrapping_sub(offset)
                }
            } else {
                addr
            };
            if rn != 15 {
                self.gpr[rn] = final_addr;
            }
        }
    }

    // ---- ARM: Halfword/Signed Data Transfer ----

    /// ARM halfword and signed data transfer (LDRH, STRH, LDRSB, LDRSH)
    fn arm_halfword_transfer(&mut self, instr: u32) {
        let pre_index = instr & (1 << 24) != 0;
        let add_offset = instr & (1 << 23) != 0;
        let immediate_offset = instr & (1 << 22) != 0;
        let write_back = instr & (1 << 21) != 0;
        let is_load = instr & (1 << 20) != 0;
        let rn = ((instr >> 16) & 0xF) as usize;
        let rd = ((instr >> 12) & 0xF) as usize;
        let op = (instr >> 5) & 0x3;

        let base = if rn == 15 {
            self.gpr[15].wrapping_add(4)
        } else {
            self.gpr[rn]
        };

        let offset = if immediate_offset {
            ((instr >> 4) & 0xF0) | (instr & 0xF)
        } else {
            let rm = (instr & 0xF) as usize;
            self.gpr[rm]
        };

        let addr = if pre_index {
            if add_offset {
                base.wrapping_add(offset)
            } else {
                base.wrapping_sub(offset)
            }
        } else {
            base
        };

        if is_load {
            let val = match op {
                0b01 => {
                    // LDRH - unsigned halfword
                    self.memory.read_halfword(addr & !1) as u32
                }
                0b10 => {
                    // LDRSB - signed byte
                    self.memory.read_byte(addr) as i8 as i32 as u32
                }
                0b11 => {
                    // LDRSH - signed halfword
                    self.memory.read_halfword(addr & !1) as i16 as i32 as u32
                }
                _ => {
                    log(
                        LogCategory::CPU,
                        LogLevel::Warn,
                        || format!("ARM: Invalid halfword transfer op={}", op),
                    );
                    0
                }
            };

            if rd == 15 {
                self.set_pc(val);
            } else {
                self.gpr[rd] = val;
            }
            self.cycles += 3;
        } else {
            // STRH - store halfword
            let val = if rd == 15 {
                self.gpr[15].wrapping_add(4)
            } else {
                self.gpr[rd]
            };
            self.memory.write_halfword(addr & !1, val as u16);
            self.cycles += 2;
        }

        if !pre_index || write_back {
            let final_addr = if !pre_index {
                if add_offset {
                    base.wrapping_add(offset)
                } else {
                    base.wrapping_sub(offset)
                }
            } else {
                addr
            };
            if rn != 15 {
                self.gpr[rn] = final_addr;
            }
        }
    }

    // ---- ARM: Block Data Transfer (LDM/STM) ----

    /// ARM block data transfer (LDM, STM)
    fn arm_block_data_transfer(&mut self, instr: u32) {
        let pre_index = instr & (1 << 24) != 0;
        let add_offset = instr & (1 << 23) != 0;
        let psr_force_user = instr & (1 << 22) != 0;
        let write_back = instr & (1 << 21) != 0;
        let is_load = instr & (1 << 20) != 0;
        let rn = ((instr >> 16) & 0xF) as usize;
        let reg_list = instr & 0xFFFF;

        if reg_list == 0 {
            // Empty register list: unpredictable, but ARM7TDMI transfers R15 and
            // modifies Rn by +/- 0x40
            self.cycles += 1;
            return;
        }

        let base = self.gpr[rn];
        let reg_count = reg_list.count_ones();
        let total_size = reg_count * 4;

        // Calculate starting address
        let mut addr = if add_offset {
            if pre_index {
                base.wrapping_add(4)
            } else {
                base
            }
        } else if pre_index {
            base.wrapping_sub(total_size)
        } else {
            base.wrapping_sub(total_size).wrapping_add(4)
        };

        let _ = psr_force_user; // TODO: Handle S bit (force user banks)

        // Transfer registers
        for i in 0..16u32 {
            if reg_list & (1 << i) != 0 {
                if is_load {
                    let val = self.memory.read_word(addr & !3);
                    if i == 15 {
                        self.set_pc(val & !3);
                        if psr_force_user {
                            // LDM with S bit and R15: restore CPSR from SPSR
                            let spsr = self.get_spsr();
                            let new_mode =
                                ProcessorMode::from_bits(spsr).unwrap_or(ProcessorMode::System);
                            self.switch_mode(new_mode);
                            self.cpsr = spsr;
                        }
                    } else {
                        self.gpr[i as usize] = val;
                    }
                } else {
                    let val = if i == 15 {
                        self.gpr[15].wrapping_add(4) // PC + 12
                    } else {
                        self.gpr[i as usize]
                    };
                    self.memory.write_word(addr & !3, val);
                }
                addr = addr.wrapping_add(4);
            }
        }

        // Write-back
        if write_back {
            self.gpr[rn] = if add_offset {
                base.wrapping_add(total_size)
            } else {
                base.wrapping_sub(total_size)
            };
        }

        self.cycles += reg_count as u64 + if is_load { 2 } else { 1 };
    }

    // ---- ARM: Single Data Swap ----

    /// ARM single data swap (SWP, SWPB)
    fn arm_swap(&mut self, instr: u32) {
        let byte = instr & (1 << 22) != 0;
        let rn = ((instr >> 16) & 0xF) as usize;
        let rd = ((instr >> 12) & 0xF) as usize;
        let rm = (instr & 0xF) as usize;

        let addr = self.gpr[rn];

        if byte {
            let old = self.memory.read_byte(addr) as u32;
            self.memory.write_byte(addr, self.gpr[rm] as u8);
            self.gpr[rd] = old;
        } else {
            let aligned = addr & !3;
            let old = self.memory.read_word(aligned);
            let rotate = (addr & 3) * 8;
            let old_rotated = if rotate != 0 {
                old.rotate_right(rotate)
            } else {
                old
            };
            self.memory.write_word(aligned, self.gpr[rm]);
            self.gpr[rd] = old_rotated;
        }

        self.cycles += 4; // 1S + 2N + 1I
    }

    // ---- ARM: PSR Transfer ----

    /// ARM MRS (transfer PSR to register)
    fn arm_mrs(&mut self, instr: u32) {
        let use_spsr = instr & (1 << 22) != 0;
        let rd = ((instr >> 12) & 0xF) as usize;

        self.gpr[rd] = if use_spsr {
            self.get_spsr()
        } else {
            self.cpsr
        };

        self.cycles += 1;
    }

    /// ARM MSR (transfer register/immediate to PSR)
    fn arm_msr(&mut self, instr: u32) {
        let use_spsr = instr & (1 << 22) != 0;
        let immediate = instr & (1 << 25) != 0;

        let value = if immediate {
            let imm = instr & 0xFF;
            let rotate = (instr >> 8) & 0xF;
            imm.rotate_right(rotate * 2)
        } else {
            let rm = (instr & 0xF) as usize;
            self.gpr[rm]
        };

        // Field mask (bits 19:16)
        let field_mask = (instr >> 16) & 0xF;
        let mut mask = 0u32;
        if field_mask & 1 != 0 {
            mask |= 0x000000FF;
        } // Control (mode, flags I/F/T)
        if field_mask & 2 != 0 {
            mask |= 0x0000FF00;
        } // Extension
        if field_mask & 4 != 0 {
            mask |= 0x00FF0000;
        } // Status
        if field_mask & 8 != 0 {
            mask |= 0xFF000000;
        } // Flags (N/Z/C/V)

        // In User mode, only flag bits can be changed
        if self.current_mode() == ProcessorMode::User {
            mask &= 0xFF000000;
        }

        if use_spsr {
            let spsr = self.get_spsr();
            self.set_spsr((spsr & !mask) | (value & mask));
        } else {
            let old_cpsr = self.cpsr;
            let new_cpsr = (old_cpsr & !mask) | (value & mask);

            // If mode bits changed, switch modes
            if (old_cpsr & MODE_MASK) != (new_cpsr & MODE_MASK) {
                if let Some(new_mode) = ProcessorMode::from_bits(new_cpsr) {
                    self.switch_mode(new_mode);
                }
            }
            self.cpsr = new_cpsr;
        }

        self.cycles += 1;
    }

    // =========================================================================
    // Thumb mode execution (16-bit instructions)
    // =========================================================================

    /// Execute one Thumb (16-bit) instruction
    fn step_thumb(&mut self) {
        let pc = self.gpr[15];
        let instr = self.memory.read_halfword(pc & !1) as u32;

        log(
            LogCategory::CPU,
            LogLevel::Trace,
            || format!("THUMB: PC={:08X} instr={:04X}", pc, instr),
        );

        // Advance PC
        self.gpr[15] = pc.wrapping_add(2);

        // Decode based on bits 15:8
        let bits_15_8 = (instr >> 8) & 0xFF;

        match bits_15_8 >> 5 {
            0b000 => {
                if (bits_15_8 >> 3) == 0b00011 {
                    // Format 2: Add/Subtract
                    self.thumb_add_sub(instr);
                } else {
                    // Format 1: Move shifted register
                    self.thumb_move_shifted(instr);
                }
            }
            0b001 => {
                // Format 3: Move/Compare/Add/Subtract immediate
                self.thumb_imm_ops(instr);
            }
            0b010 => {
                if bits_15_8 >> 2 == 0b010000 {
                    // Format 4: ALU operations
                    self.thumb_alu(instr);
                } else if bits_15_8 >> 2 == 0b010001 {
                    // Format 5: Hi register operations / Branch exchange
                    self.thumb_hi_reg_bx(instr);
                } else if bits_15_8 >> 3 == 0b01001 {
                    // Format 6: PC-relative load
                    self.thumb_pc_relative_load(instr);
                } else {
                    // Format 7/8: Load/Store with register offset
                    self.thumb_load_store_reg(instr);
                }
            }
            0b011 => {
                // Format 9: Load/Store with immediate offset
                self.thumb_load_store_imm(instr);
            }
            0b100 => {
                if bits_15_8 >> 4 == 0b1000 {
                    // Format 10: Load/Store halfword
                    self.thumb_load_store_halfword(instr);
                } else {
                    // Format 11: SP-relative Load/Store
                    self.thumb_sp_relative_load_store(instr);
                }
            }
            0b101 => {
                if bits_15_8 >> 4 == 0b1010 {
                    // Format 12: Load address
                    self.thumb_load_address(instr);
                } else if bits_15_8 == 0b10110000 {
                    // Format 13: Add offset to SP
                    self.thumb_add_offset_sp(instr);
                } else if (bits_15_8 & 0b11110110) == 0b10110100 {
                    // Format 14: Push/Pop registers
                    self.thumb_push_pop(instr);
                } else {
                    log(
                        LogCategory::CPU,
                        LogLevel::Warn,
                        || format!("THUMB: Undefined instruction {:04X} at PC={:08X}", instr, pc),
                    );
                    self.handle_undefined();
                    self.cycles += 1;
                }
            }
            0b110 => {
                if bits_15_8 >> 4 == 0b1100 {
                    // Format 15: Multiple Load/Store
                    self.thumb_multiple_load_store(instr);
                } else if bits_15_8 == 0b11011111 {
                    // Format 17: Software Interrupt
                    self.handle_swi();
                    self.cycles += 3;
                } else if bits_15_8 >> 4 == 0b1101 {
                    // Format 16: Conditional branch
                    self.thumb_conditional_branch(instr);
                } else {
                    log(
                        LogCategory::CPU,
                        LogLevel::Warn,
                        || format!("THUMB: Undefined instruction {:04X} at PC={:08X}", instr, pc),
                    );
                    self.handle_undefined();
                    self.cycles += 1;
                }
            }
            0b111 => {
                if bits_15_8 >> 3 == 0b11100 {
                    // Format 18: Unconditional branch
                    self.thumb_unconditional_branch(instr);
                } else {
                    // Format 19: Long branch with link
                    self.thumb_long_branch_link(instr);
                }
            }
            _ => unreachable!(),
        }
    }

    // ---- Thumb Format 1: Move shifted register ----
    fn thumb_move_shifted(&mut self, instr: u32) {
        let op = (instr >> 11) & 0x3;
        let offset = (instr >> 6) & 0x1F;
        let rs = ((instr >> 3) & 0x7) as usize;
        let rd = (instr & 0x7) as usize;

        let (result, carry) = self.barrel_shift(self.gpr[rs], op, offset, self.flag_c());

        self.gpr[rd] = result;
        self.set_nz(result);
        self.set_flag(FLAG_C, carry);
        self.cycles += 1;
    }

    // ---- Thumb Format 2: Add/Subtract ----
    fn thumb_add_sub(&mut self, instr: u32) {
        let is_immediate = instr & (1 << 10) != 0;
        let is_sub = instr & (1 << 9) != 0;
        let rn_or_imm = ((instr >> 6) & 0x7) as usize;
        let rs = ((instr >> 3) & 0x7) as usize;
        let rd = (instr & 0x7) as usize;

        let op2 = if is_immediate {
            rn_or_imm as u32
        } else {
            self.gpr[rn_or_imm]
        };

        let rs_val = self.gpr[rs];

        let (result, carry, overflow) = if is_sub {
            let (r, b) = rs_val.overflowing_sub(op2);
            let v = ((rs_val ^ op2) & (rs_val ^ r)) >> 31 != 0;
            (r, !b, v)
        } else {
            let (r, c) = rs_val.overflowing_add(op2);
            let v = (!(rs_val ^ op2) & (rs_val ^ r)) >> 31 != 0;
            (r, c, v)
        };

        self.gpr[rd] = result;
        self.set_nz(result);
        self.set_flag(FLAG_C, carry);
        self.set_flag(FLAG_V, overflow);
        self.cycles += 1;
    }

    // ---- Thumb Format 3: Move/Compare/Add/Subtract immediate ----
    fn thumb_imm_ops(&mut self, instr: u32) {
        let op = (instr >> 11) & 0x3;
        let rd = ((instr >> 8) & 0x7) as usize;
        let imm = instr & 0xFF;

        match op {
            0b00 => {
                // MOV
                self.gpr[rd] = imm;
                self.set_nz(imm);
            }
            0b01 => {
                // CMP
                let (result, borrow) = self.gpr[rd].overflowing_sub(imm);
                self.set_nz(result);
                self.set_flag(FLAG_C, !borrow);
                let v = ((self.gpr[rd] ^ imm) & (self.gpr[rd] ^ result)) >> 31 != 0;
                self.set_flag(FLAG_V, v);
            }
            0b10 => {
                // ADD
                let (result, carry) = self.gpr[rd].overflowing_add(imm);
                let v = (!(self.gpr[rd] ^ imm) & (self.gpr[rd] ^ result)) >> 31 != 0;
                self.gpr[rd] = result;
                self.set_nz(result);
                self.set_flag(FLAG_C, carry);
                self.set_flag(FLAG_V, v);
            }
            0b11 => {
                // SUB
                let (result, borrow) = self.gpr[rd].overflowing_sub(imm);
                let v = ((self.gpr[rd] ^ imm) & (self.gpr[rd] ^ result)) >> 31 != 0;
                self.gpr[rd] = result;
                self.set_nz(result);
                self.set_flag(FLAG_C, !borrow);
                self.set_flag(FLAG_V, v);
            }
            _ => unreachable!(),
        }
        self.cycles += 1;
    }

    // ---- Thumb Format 4: ALU operations ----
    fn thumb_alu(&mut self, instr: u32) {
        let op = (instr >> 6) & 0xF;
        let rs = ((instr >> 3) & 0x7) as usize;
        let rd = (instr & 0x7) as usize;

        let a = self.gpr[rd];
        let b = self.gpr[rs];

        match op {
            0x0 => {
                // AND
                let r = a & b;
                self.gpr[rd] = r;
                self.set_nz(r);
            }
            0x1 => {
                // EOR
                let r = a ^ b;
                self.gpr[rd] = r;
                self.set_nz(r);
            }
            0x2 => {
                // LSL
                let (r, c) = self.barrel_shift(a, 0b00, b & 0xFF, self.flag_c());
                self.gpr[rd] = r;
                self.set_nz(r);
                if b & 0xFF != 0 {
                    self.set_flag(FLAG_C, c);
                }
            }
            0x3 => {
                // LSR
                let (r, c) = self.barrel_shift(a, 0b01, b & 0xFF, self.flag_c());
                self.gpr[rd] = r;
                self.set_nz(r);
                if b & 0xFF != 0 {
                    self.set_flag(FLAG_C, c);
                }
            }
            0x4 => {
                // ASR
                let (r, c) = self.barrel_shift(a, 0b10, b & 0xFF, self.flag_c());
                self.gpr[rd] = r;
                self.set_nz(r);
                if b & 0xFF != 0 {
                    self.set_flag(FLAG_C, c);
                }
            }
            0x5 => {
                // ADC
                let c_in = if self.flag_c() { 1u32 } else { 0 };
                let (r1, c1) = a.overflowing_add(b);
                let (r, c2) = r1.overflowing_add(c_in);
                self.gpr[rd] = r;
                self.set_nz(r);
                self.set_flag(FLAG_C, c1 || c2);
                let v = (!(a ^ b) & (a ^ r)) >> 31 != 0;
                self.set_flag(FLAG_V, v);
            }
            0x6 => {
                // SBC
                let c_in = if self.flag_c() { 0u32 } else { 1 };
                let (r1, b1) = a.overflowing_sub(b);
                let (r, b2) = r1.overflowing_sub(c_in);
                self.gpr[rd] = r;
                self.set_nz(r);
                self.set_flag(FLAG_C, !(b1 || b2));
                let v = ((a ^ b) & (a ^ r)) >> 31 != 0;
                self.set_flag(FLAG_V, v);
            }
            0x7 => {
                // ROR
                let (r, c) = self.barrel_shift(a, 0b11, b & 0xFF, self.flag_c());
                self.gpr[rd] = r;
                self.set_nz(r);
                if b & 0xFF != 0 {
                    self.set_flag(FLAG_C, c);
                }
            }
            0x8 => {
                // TST
                let r = a & b;
                self.set_nz(r);
            }
            0x9 => {
                // NEG
                let (r, borrow) = 0u32.overflowing_sub(b);
                self.gpr[rd] = r;
                self.set_nz(r);
                self.set_flag(FLAG_C, !borrow);
                let v = (b & r) >> 31 != 0;
                self.set_flag(FLAG_V, v);
            }
            0xA => {
                // CMP
                let (r, borrow) = a.overflowing_sub(b);
                self.set_nz(r);
                self.set_flag(FLAG_C, !borrow);
                let v = ((a ^ b) & (a ^ r)) >> 31 != 0;
                self.set_flag(FLAG_V, v);
            }
            0xB => {
                // CMN
                let (r, carry) = a.overflowing_add(b);
                self.set_nz(r);
                self.set_flag(FLAG_C, carry);
                let v = (!(a ^ b) & (a ^ r)) >> 31 != 0;
                self.set_flag(FLAG_V, v);
            }
            0xC => {
                // ORR
                let r = a | b;
                self.gpr[rd] = r;
                self.set_nz(r);
            }
            0xD => {
                // MUL
                let r = a.wrapping_mul(b);
                self.gpr[rd] = r;
                self.set_nz(r);
                self.cycles += 1; // Extra cycle for multiply
            }
            0xE => {
                // BIC
                let r = a & !b;
                self.gpr[rd] = r;
                self.set_nz(r);
            }
            0xF => {
                // MVN
                let r = !b;
                self.gpr[rd] = r;
                self.set_nz(r);
            }
            _ => unreachable!(),
        }
        self.cycles += 1;
    }

    // ---- Thumb Format 5: Hi register operations / Branch exchange ----
    fn thumb_hi_reg_bx(&mut self, instr: u32) {
        let op = (instr >> 8) & 0x3;
        let h1 = (instr >> 7) & 1; // High bit of Rd
        let h2 = (instr >> 6) & 1; // High bit of Rs
        let rs = (((h2 << 3) | ((instr >> 3) & 0x7)) as usize) & 0xF;
        let rd = (((h1 << 3) | (instr & 0x7)) as usize) & 0xF;

        let rs_val = if rs == 15 {
            self.gpr[15].wrapping_add(2) // PC + 4 (Thumb pipeline)
        } else {
            self.gpr[rs]
        };

        match op {
            0b00 => {
                // ADD (no flags)
                let result = self.gpr[rd].wrapping_add(rs_val);
                if rd == 15 {
                    self.set_pc(result & !1);
                } else {
                    self.gpr[rd] = result;
                }
            }
            0b01 => {
                // CMP
                let (result, borrow) = self.gpr[rd].overflowing_sub(rs_val);
                self.set_nz(result);
                self.set_flag(FLAG_C, !borrow);
                let v = ((self.gpr[rd] ^ rs_val) & (self.gpr[rd] ^ result)) >> 31 != 0;
                self.set_flag(FLAG_V, v);
            }
            0b10 => {
                // MOV (no flags)
                if rd == 15 {
                    self.set_pc(rs_val & !1);
                } else {
                    self.gpr[rd] = rs_val;
                }
            }
            0b11 => {
                // BX
                if rs_val & 1 != 0 {
                    self.cpsr |= FLAG_T; // Stay/switch to Thumb
                    self.set_pc(rs_val & !1);
                } else {
                    self.cpsr &= !FLAG_T; // Switch to ARM
                    self.set_pc(rs_val & !3);
                }
            }
            _ => unreachable!(),
        }
        self.cycles += 1;
    }

    // ---- Thumb Format 6: PC-relative load ----
    fn thumb_pc_relative_load(&mut self, instr: u32) {
        let rd = ((instr >> 8) & 0x7) as usize;
        let imm = (instr & 0xFF) << 2;
        // PC is word-aligned for this operation
        let addr = (self.gpr[15].wrapping_add(2) & !3).wrapping_add(imm);
        self.gpr[rd] = self.memory.read_word(addr);
        self.cycles += 3;
    }

    // ---- Thumb Format 7/8: Load/Store with register offset ----
    fn thumb_load_store_reg(&mut self, instr: u32) {
        let op = (instr >> 10) & 0x3;
        let byte_or_sign = (instr >> 10) & 1;
        let ro = ((instr >> 6) & 0x7) as usize;
        let rb = ((instr >> 3) & 0x7) as usize;
        let rd = (instr & 0x7) as usize;

        let addr = self.gpr[rb].wrapping_add(self.gpr[ro]);

        if instr & (1 << 9) != 0 {
            // Format 8: Sign-extended / halfword
            match (byte_or_sign, instr & (1 << 11) != 0) {
                (0, false) => {
                    // STRH
                    self.memory.write_halfword(addr & !1, self.gpr[rd] as u16);
                    self.cycles += 2;
                }
                (0, true) => {
                    // LDRH
                    self.gpr[rd] = self.memory.read_halfword(addr & !1) as u32;
                    self.cycles += 3;
                }
                (1, false) => {
                    // LDSB
                    self.gpr[rd] = self.memory.read_byte(addr) as i8 as i32 as u32;
                    self.cycles += 3;
                }
                (1, true) => {
                    // LDSH
                    self.gpr[rd] = self.memory.read_halfword(addr & !1) as i16 as i32 as u32;
                    self.cycles += 3;
                }
                _ => unreachable!(),
            }
        } else {
            // Format 7: byte/word
            match op {
                0b00 => {
                    // STR
                    self.memory.write_word(addr & !3, self.gpr[rd]);
                    self.cycles += 2;
                }
                0b01 => {
                    // STRB
                    self.memory.write_byte(addr, self.gpr[rd] as u8);
                    self.cycles += 2;
                }
                0b10 => {
                    // LDR
                    let aligned = addr & !3;
                    let val = self.memory.read_word(aligned);
                    let rotate = (addr & 3) * 8;
                    self.gpr[rd] = if rotate != 0 {
                        val.rotate_right(rotate)
                    } else {
                        val
                    };
                    self.cycles += 3;
                }
                0b11 => {
                    // LDRB
                    self.gpr[rd] = self.memory.read_byte(addr) as u32;
                    self.cycles += 3;
                }
                _ => unreachable!(),
            }
        }
    }

    // ---- Thumb Format 9: Load/Store with immediate offset ----
    fn thumb_load_store_imm(&mut self, instr: u32) {
        let byte = instr & (1 << 12) != 0;
        let is_load = instr & (1 << 11) != 0;
        let offset = (instr >> 6) & 0x1F;
        let rb = ((instr >> 3) & 0x7) as usize;
        let rd = (instr & 0x7) as usize;

        let addr = if byte {
            self.gpr[rb].wrapping_add(offset)
        } else {
            self.gpr[rb].wrapping_add(offset << 2)
        };

        if is_load {
            if byte {
                self.gpr[rd] = self.memory.read_byte(addr) as u32;
            } else {
                let aligned = addr & !3;
                let val = self.memory.read_word(aligned);
                let rotate = (addr & 3) * 8;
                self.gpr[rd] = if rotate != 0 {
                    val.rotate_right(rotate)
                } else {
                    val
                };
            }
            self.cycles += 3;
        } else {
            if byte {
                self.memory.write_byte(addr, self.gpr[rd] as u8);
            } else {
                self.memory.write_word(addr & !3, self.gpr[rd]);
            }
            self.cycles += 2;
        }
    }

    // ---- Thumb Format 10: Load/Store halfword ----
    fn thumb_load_store_halfword(&mut self, instr: u32) {
        let is_load = instr & (1 << 11) != 0;
        let offset = ((instr >> 6) & 0x1F) << 1;
        let rb = ((instr >> 3) & 0x7) as usize;
        let rd = (instr & 0x7) as usize;

        let addr = self.gpr[rb].wrapping_add(offset);

        if is_load {
            self.gpr[rd] = self.memory.read_halfword(addr & !1) as u32;
            self.cycles += 3;
        } else {
            self.memory.write_halfword(addr & !1, self.gpr[rd] as u16);
            self.cycles += 2;
        }
    }

    // ---- Thumb Format 11: SP-relative Load/Store ----
    fn thumb_sp_relative_load_store(&mut self, instr: u32) {
        let is_load = instr & (1 << 11) != 0;
        let rd = ((instr >> 8) & 0x7) as usize;
        let imm = (instr & 0xFF) << 2;

        let addr = self.gpr[13].wrapping_add(imm); // SP + offset

        if is_load {
            self.gpr[rd] = self.memory.read_word(addr & !3);
            self.cycles += 3;
        } else {
            self.memory.write_word(addr & !3, self.gpr[rd]);
            self.cycles += 2;
        }
    }

    // ---- Thumb Format 12: Load address ----
    fn thumb_load_address(&mut self, instr: u32) {
        let use_sp = instr & (1 << 11) != 0;
        let rd = ((instr >> 8) & 0x7) as usize;
        let imm = (instr & 0xFF) << 2;

        if use_sp {
            self.gpr[rd] = self.gpr[13].wrapping_add(imm);
        } else {
            // PC-relative: PC is word-aligned
            self.gpr[rd] = (self.gpr[15].wrapping_add(2) & !3).wrapping_add(imm);
        }
        self.cycles += 1;
    }

    // ---- Thumb Format 13: Add offset to SP ----
    fn thumb_add_offset_sp(&mut self, instr: u32) {
        let negative = instr & (1 << 7) != 0;
        let imm = (instr & 0x7F) << 2;

        if negative {
            self.gpr[13] = self.gpr[13].wrapping_sub(imm);
        } else {
            self.gpr[13] = self.gpr[13].wrapping_add(imm);
        }
        self.cycles += 1;
    }

    // ---- Thumb Format 14: Push/Pop registers ----
    fn thumb_push_pop(&mut self, instr: u32) {
        let is_pop = instr & (1 << 11) != 0;
        let pc_lr = instr & (1 << 8) != 0; // Push LR or Pop PC
        let reg_list = instr & 0xFF;

        let reg_count = reg_list.count_ones() + if pc_lr { 1 } else { 0 };

        if is_pop {
            // POP
            let mut addr = self.gpr[13];
            for i in 0..8u32 {
                if reg_list & (1 << i) != 0 {
                    self.gpr[i as usize] = self.memory.read_word(addr & !3);
                    addr = addr.wrapping_add(4);
                }
            }
            if pc_lr {
                let val = self.memory.read_word(addr & !3);
                self.set_pc(val & !1);
                // ARMv4T: bit 0 doesn't switch state on POP {PC}
                // ARMv5+: bit 0 does switch state
                addr = addr.wrapping_add(4);
            }
            self.gpr[13] = addr;
            self.cycles += reg_count as u64 + 2;
        } else {
            // PUSH
            let mut addr = self.gpr[13].wrapping_sub(reg_count * 4);
            self.gpr[13] = addr; // SP adjusted before storing

            for i in 0..8u32 {
                if reg_list & (1 << i) != 0 {
                    self.memory.write_word(addr & !3, self.gpr[i as usize]);
                    addr = addr.wrapping_add(4);
                }
            }
            if pc_lr {
                self.memory.write_word(addr & !3, self.gpr[14]); // Push LR
            }
            self.cycles += reg_count as u64 + 1;
        }
    }

    // ---- Thumb Format 15: Multiple Load/Store ----
    fn thumb_multiple_load_store(&mut self, instr: u32) {
        let is_load = instr & (1 << 11) != 0;
        let rb = ((instr >> 8) & 0x7) as usize;
        let reg_list = instr & 0xFF;

        let reg_count = reg_list.count_ones();
        let mut addr = self.gpr[rb];

        if reg_list == 0 {
            // Empty register list: unpredictable
            self.cycles += 1;
            return;
        }

        for i in 0..8u32 {
            if reg_list & (1 << i) != 0 {
                if is_load {
                    self.gpr[i as usize] = self.memory.read_word(addr & !3);
                } else {
                    self.memory.write_word(addr & !3, self.gpr[i as usize]);
                }
                addr = addr.wrapping_add(4);
            }
        }

        // Write-back (always for STMIA/LDMIA in Thumb)
        // Note: For LDMIA, write-back doesn't occur if Rb is in the register list
        if !is_load || (reg_list & (1 << rb)) == 0 {
            self.gpr[rb] = addr;
        }

        self.cycles += reg_count as u64 + if is_load { 2 } else { 1 };
    }

    // ---- Thumb Format 16: Conditional branch ----
    fn thumb_conditional_branch(&mut self, instr: u32) {
        let cond = (instr >> 8) & 0xF;

        if !self.check_condition(cond) {
            self.cycles += 1;
            return;
        }

        let offset = ((instr & 0xFF) as i8 as i32) << 1;
        let target = (self.gpr[15] as i32).wrapping_add(offset).wrapping_add(2) as u32;
        self.set_pc(target);
        self.cycles += 3;
    }

    // ---- Thumb Format 18: Unconditional branch ----
    fn thumb_unconditional_branch(&mut self, instr: u32) {
        // 11-bit signed offset, shifted left 1
        let offset = ((instr & 0x7FF) << 21) as i32 >> 20;
        let target = (self.gpr[15] as i32).wrapping_add(offset).wrapping_add(2) as u32;
        self.set_pc(target);
        self.cycles += 3;
    }

    // ---- Thumb Format 19: Long branch with link (two-instruction sequence) ----
    fn thumb_long_branch_link(&mut self, instr: u32) {
        let h = (instr >> 11) & 0x1;

        if h == 0 {
            // First instruction: LR = PC + (offset << 12)
            let offset = ((instr & 0x7FF) << 21) as i32 >> 9; // sign-extend and shift left 12
            self.gpr[14] = (self.gpr[15] as i32).wrapping_add(offset).wrapping_add(2) as u32;
            self.cycles += 1;
        } else {
            // Second instruction: temp = next instruction address; PC = LR + (offset << 1); LR = temp | 1
            let offset = (instr & 0x7FF) << 1;
            let next_instr_addr = self.gpr[15]; // Already advanced by 2
            let target = self.gpr[14].wrapping_add(offset);
            self.gpr[14] = next_instr_addr | 1; // Set bit 0 to indicate Thumb
            self.set_pc(target);
            self.cycles += 3;
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Simple test memory: 64KB flat space
    struct TestMemory {
        data: Vec<u8>,
    }

    impl TestMemory {
        fn new() -> Self {
            Self {
                data: vec![0; 0x10000],
            }
        }

        fn write_word_at(&mut self, addr: u32, val: u32) {
            let a = addr as usize;
            if a + 3 < self.data.len() {
                self.data[a] = val as u8;
                self.data[a + 1] = (val >> 8) as u8;
                self.data[a + 2] = (val >> 16) as u8;
                self.data[a + 3] = (val >> 24) as u8;
            }
        }

        fn write_halfword_at(&mut self, addr: u32, val: u16) {
            let a = addr as usize;
            if a + 1 < self.data.len() {
                self.data[a] = val as u8;
                self.data[a + 1] = (val >> 8) as u8;
            }
        }
    }

    impl MemoryArm7 for TestMemory {
        fn read_byte(&self, addr: u32) -> u8 {
            let a = (addr as usize) % self.data.len();
            self.data[a]
        }

        fn read_halfword(&self, addr: u32) -> u16 {
            let a = (addr as usize) % self.data.len();
            u16::from_le_bytes([self.data[a], self.data[a + 1]])
        }

        fn read_word(&self, addr: u32) -> u32 {
            let a = (addr as usize) % self.data.len();
            u32::from_le_bytes([
                self.data[a],
                self.data[a + 1],
                self.data[a + 2],
                self.data[a + 3],
            ])
        }

        fn write_byte(&mut self, addr: u32, val: u8) {
            let a = (addr as usize) % self.data.len();
            self.data[a] = val;
        }

        fn write_halfword(&mut self, addr: u32, val: u16) {
            let a = (addr as usize) % self.data.len();
            let bytes = val.to_le_bytes();
            self.data[a] = bytes[0];
            self.data[a + 1] = bytes[1];
        }

        fn write_word(&mut self, addr: u32, val: u32) {
            let a = (addr as usize) % self.data.len();
            let bytes = val.to_le_bytes();
            self.data[a] = bytes[0];
            self.data[a + 1] = bytes[1];
            self.data[a + 2] = bytes[2];
            self.data[a + 3] = bytes[3];
        }
    }

    fn make_cpu() -> Arm7Tdmi<TestMemory> {
        Arm7Tdmi::new(TestMemory::new())
    }

    // Helper to encode ARM instructions
    fn arm_encode_dp(cond: u32, opcode: u32, s: bool, rn: u32, rd: u32, op2: u32) -> u32 {
        (cond << 28)
            | (opcode << 21)
            | (if s { 1 << 20 } else { 0 })
            | (rn << 16)
            | (rd << 12)
            | op2
    }

    fn arm_encode_imm(cond: u32, opcode: u32, s: bool, rn: u32, rd: u32, rotate: u32, imm: u32) -> u32 {
        (cond << 28)
            | (0b001 << 25)
            | (opcode << 21)
            | (if s { 1 << 20 } else { 0 })
            | (rn << 16)
            | (rd << 12)
            | (rotate << 8)
            | (imm & 0xFF)
    }

    #[test]
    fn test_initial_state() {
        let cpu = make_cpu();
        assert_eq!(cpu.cpsr & MODE_MASK, MODE_SUPERVISOR);
        assert!(cpu.cpsr & FLAG_I != 0); // IRQ disabled
        assert!(cpu.cpsr & FLAG_F != 0); // FIQ disabled
        assert!(cpu.cpsr & FLAG_T == 0); // ARM mode
        assert_eq!(cpu.gpr[15], 0); // PC at reset vector
    }

    #[test]
    fn test_reset() {
        let mut cpu = make_cpu();
        cpu.gpr[0] = 0x42;
        cpu.gpr[15] = 0x1000;
        cpu.cycles = 999;
        cpu.reset();
        assert_eq!(cpu.gpr[0], 0);
        assert_eq!(cpu.gpr[15], 0);
        assert_eq!(cpu.cycles, 0);
        assert_eq!(cpu.cpsr & MODE_MASK, MODE_SUPERVISOR);
    }

    #[test]
    fn test_condition_codes() {
        let cpu = make_cpu();

        // AL always passes
        assert!(cpu.check_condition(COND_AL));

        // EQ requires Z
        assert!(!cpu.check_condition(COND_EQ));
        assert!(!cpu.check_condition(COND_MI));
    }

    #[test]
    fn test_arm_mov_immediate() {
        let mut cpu = make_cpu();
        // MOV R0, #42 (AL condition)
        let instr = arm_encode_imm(COND_AL, 0xD, false, 0, 0, 0, 42);
        cpu.memory.write_word_at(0, instr);
        cpu.step();
        assert_eq!(cpu.gpr[0], 42);
    }

    #[test]
    fn test_arm_movs_sets_flags() {
        let mut cpu = make_cpu();
        // MOVS R0, #0 (should set Z flag)
        let instr = arm_encode_imm(COND_AL, 0xD, true, 0, 0, 0, 0);
        cpu.memory.write_word_at(0, instr);
        cpu.step();
        assert_eq!(cpu.gpr[0], 0);
        assert!(cpu.flag_z());
        assert!(!cpu.flag_n());
    }

    #[test]
    fn test_arm_add() {
        let mut cpu = make_cpu();
        cpu.gpr[1] = 10;
        cpu.gpr[2] = 20;
        // ADD R0, R1, R2
        let instr = arm_encode_dp(COND_AL, 0x4, false, 1, 0, 2);
        cpu.memory.write_word_at(0, instr);
        cpu.step();
        assert_eq!(cpu.gpr[0], 30);
    }

    #[test]
    fn test_arm_adds_overflow() {
        let mut cpu = make_cpu();
        cpu.gpr[1] = 0x7FFFFFFF; // Max positive i32
        // ADDS R0, R1, #1
        let instr = arm_encode_imm(COND_AL, 0x4, true, 1, 0, 0, 1);
        cpu.memory.write_word_at(0, instr);
        cpu.step();
        assert_eq!(cpu.gpr[0], 0x80000000);
        assert!(cpu.flag_n()); // Result is negative
        assert!(cpu.flag_v()); // Overflow occurred
        assert!(!cpu.flag_z());
    }

    #[test]
    fn test_arm_sub() {
        let mut cpu = make_cpu();
        cpu.gpr[1] = 50;
        // SUB R0, R1, #20
        let instr = arm_encode_imm(COND_AL, 0x2, false, 1, 0, 0, 20);
        cpu.memory.write_word_at(0, instr);
        cpu.step();
        assert_eq!(cpu.gpr[0], 30);
    }

    #[test]
    fn test_arm_and() {
        let mut cpu = make_cpu();
        cpu.gpr[1] = 0xFF;
        cpu.gpr[2] = 0x0F;
        // AND R0, R1, R2
        let instr = arm_encode_dp(COND_AL, 0x0, false, 1, 0, 2);
        cpu.memory.write_word_at(0, instr);
        cpu.step();
        assert_eq!(cpu.gpr[0], 0x0F);
    }

    #[test]
    fn test_arm_orr() {
        let mut cpu = make_cpu();
        cpu.gpr[1] = 0xF0;
        cpu.gpr[2] = 0x0F;
        // ORR R0, R1, R2
        let instr = arm_encode_dp(COND_AL, 0xC, false, 1, 0, 2);
        cpu.memory.write_word_at(0, instr);
        cpu.step();
        assert_eq!(cpu.gpr[0], 0xFF);
    }

    #[test]
    fn test_arm_cmp() {
        let mut cpu = make_cpu();
        cpu.gpr[0] = 42;
        // CMP R0, #42 (should set Z)
        let instr = arm_encode_imm(COND_AL, 0xA, true, 0, 0, 0, 42);
        cpu.memory.write_word_at(0, instr);
        cpu.step();
        assert!(cpu.flag_z());
        assert!(!cpu.flag_n());
        assert!(cpu.flag_c()); // No borrow
    }

    #[test]
    fn test_arm_branch() {
        let mut cpu = make_cpu();
        // B +8 (PC-relative, offset in words: 0x000002 → jumps forward 8 bytes from PC+8)
        // B instruction: cond=AL, 101, L=0, offset=0x000000
        // Target = PC+8 + offset*4 = 0+8+0 = 8 (but we need to encode the offset)
        // Let's jump to address 0x10: offset = (0x10 - (0+8)) / 4 = 2
        let instr: u32 = 0xEA000002; // B PC+8+8 = B to 0x10
        cpu.memory.write_word_at(0, instr);
        cpu.step();
        assert_eq!(cpu.gpr[15], 0x10);
    }

    #[test]
    fn test_arm_branch_link() {
        let mut cpu = make_cpu();
        // BL to +16: offset = (16 - 8) / 4 = 2
        let instr: u32 = 0xEB000002; // BL to 0x10
        cpu.memory.write_word_at(0, instr);
        cpu.step();
        assert_eq!(cpu.gpr[15], 0x10);
        assert_eq!(cpu.gpr[14], 4); // LR = next instruction after BL
    }

    #[test]
    fn test_arm_bx_to_thumb() {
        let mut cpu = make_cpu();
        cpu.gpr[0] = 0x101; // Bit 0 set = switch to Thumb
        // BX R0: 0xE12FFF10
        let instr: u32 = 0xE12FFF10;
        cpu.memory.write_word_at(0, instr);
        cpu.step();
        assert!(cpu.is_thumb());
        assert_eq!(cpu.gpr[15], 0x100);
    }

    #[test]
    fn test_arm_ldr_str() {
        let mut cpu = make_cpu();

        // First store a value: MOV R0, #0x42
        let mov = arm_encode_imm(COND_AL, 0xD, false, 0, 0, 0, 0x42);
        cpu.memory.write_word_at(0, mov);
        cpu.step();
        assert_eq!(cpu.gpr[0], 0x42);

        // MOV R1, #0x80 (base address)
        let mov2 = arm_encode_imm(COND_AL, 0xD, false, 0, 1, 0, 0x80);
        cpu.memory.write_word_at(4, mov2);
        cpu.step();

        // STR R0, [R1] (store R0 at addr in R1)
        // STR: cond=AL, 01, I=0, P=1, U=1, B=0, W=0, L=0, Rn=1, Rd=0, offset=0
        let str_instr: u32 = 0xE5810000;
        cpu.memory.write_word_at(8, str_instr);
        cpu.step();

        // Verify memory
        assert_eq!(cpu.memory.read_word(0x80), 0x42);

        // LDR R2, [R1] (load from addr in R1 to R2)
        let ldr_instr: u32 = 0xE5912000;
        cpu.memory.write_word_at(12, ldr_instr);
        cpu.step();
        assert_eq!(cpu.gpr[2], 0x42);
    }

    #[test]
    fn test_arm_multiply() {
        let mut cpu = make_cpu();
        cpu.gpr[1] = 6;
        cpu.gpr[2] = 7;
        // MUL R0, R1, R2: 0xE0000291
        let instr: u32 = 0xE0000291;
        cpu.memory.write_word_at(0, instr);
        cpu.step();
        assert_eq!(cpu.gpr[0], 42);
    }

    #[test]
    fn test_arm_conditional_skip() {
        let mut cpu = make_cpu();
        // Ensure Z flag is clear
        cpu.cpsr &= !FLAG_Z;
        // MOVEQ R0, #99 (should be skipped because Z is clear)
        let instr = arm_encode_imm(COND_EQ, 0xD, false, 0, 0, 0, 99);
        cpu.memory.write_word_at(0, instr);
        cpu.step();
        assert_eq!(cpu.gpr[0], 0); // Should not have changed
    }

    #[test]
    fn test_mode_switch() {
        let mut cpu = make_cpu();
        // Start in Supervisor
        assert_eq!(cpu.current_mode(), ProcessorMode::Supervisor);

        // Set SP in Supervisor mode
        cpu.gpr[13] = 0x1000;

        // Switch to IRQ mode
        cpu.switch_mode(ProcessorMode::Irq);
        assert_eq!(cpu.current_mode(), ProcessorMode::Irq);

        // Set SP in IRQ mode
        cpu.gpr[13] = 0x2000;

        // Switch back to Supervisor
        cpu.switch_mode(ProcessorMode::Supervisor);
        assert_eq!(cpu.gpr[13], 0x1000); // SP should be restored

        // Switch to IRQ again
        cpu.switch_mode(ProcessorMode::Irq);
        assert_eq!(cpu.gpr[13], 0x2000); // IRQ SP should be restored
    }

    #[test]
    fn test_barrel_shift_lsl() {
        let cpu = make_cpu();
        let (result, carry) = cpu.barrel_shift(0x80000001, 0b00, 1, false);
        assert_eq!(result, 0x00000002);
        assert!(carry); // Bit 31 shifted out
    }

    #[test]
    fn test_barrel_shift_lsr() {
        let cpu = make_cpu();
        let (result, carry) = cpu.barrel_shift(0x80000002, 0b01, 1, false);
        assert_eq!(result, 0x40000001);
        assert!(!carry); // Bit 0 shifted out was 0
    }

    #[test]
    fn test_barrel_shift_asr() {
        let cpu = make_cpu();
        // ASR preserves sign
        let (result, _) = cpu.barrel_shift(0x80000000, 0b10, 1, false);
        assert_eq!(result, 0xC0000000); // Sign bit preserved
    }

    #[test]
    fn test_barrel_shift_ror() {
        let cpu = make_cpu();
        let (result, _) = cpu.barrel_shift(0x00000001, 0b11, 1, false);
        assert_eq!(result, 0x80000000);
    }

    #[test]
    fn test_thumb_mov_immediate() {
        let mut cpu = make_cpu();
        cpu.cpsr |= FLAG_T; // Switch to Thumb mode

        // MOV R0, #42 → 0x202A
        let instr: u16 = 0x202A;
        cpu.memory.write_halfword_at(0, instr);
        cpu.step();
        assert_eq!(cpu.gpr[0], 42);
    }

    #[test]
    fn test_thumb_add_registers() {
        let mut cpu = make_cpu();
        cpu.cpsr |= FLAG_T;
        cpu.gpr[1] = 10;
        cpu.gpr[2] = 20;

        // ADD R0, R1, R2 (Format 2) → 0x1888
        let instr: u16 = 0x1888;
        cpu.memory.write_halfword_at(0, instr);
        cpu.step();
        assert_eq!(cpu.gpr[0], 30);
    }

    #[test]
    fn test_thumb_push_pop() {
        let mut cpu = make_cpu();
        cpu.cpsr |= FLAG_T;
        cpu.gpr[13] = 0x100; // SP

        cpu.gpr[0] = 0xAA;
        cpu.gpr[1] = 0xBB;

        // PUSH {R0, R1} → 0xB403
        let push: u16 = 0xB403;
        cpu.memory.write_halfword_at(0, push);
        cpu.step();
        assert_eq!(cpu.gpr[13], 0xF8); // SP decremented by 8

        cpu.gpr[0] = 0;
        cpu.gpr[1] = 0;

        // POP {R0, R1} → 0xBC03
        let pop: u16 = 0xBC03;
        cpu.memory.write_halfword_at(2, pop);
        cpu.step();
        assert_eq!(cpu.gpr[0], 0xAA);
        assert_eq!(cpu.gpr[1], 0xBB);
        assert_eq!(cpu.gpr[13], 0x100); // SP restored
    }

    #[test]
    fn test_thumb_conditional_branch() {
        let mut cpu = make_cpu();
        cpu.cpsr |= FLAG_T;

        // Set Z flag
        cpu.cpsr |= FLAG_Z;

        // BEQ +4 → 0xD002 (cond=0, offset=2 → target = PC+4+4 = PC+8)
        let instr: u16 = 0xD002;
        cpu.memory.write_halfword_at(0, instr);
        cpu.step();
        // Should have branched: target = (PC+2) + signed_offset*2 + 2
        // PC was 0, after advance = 2, offset = 2*2=4, + 2 = 8
        assert_eq!(cpu.gpr[15], 8);
    }

    #[test]
    fn test_thumb_unconditional_branch() {
        let mut cpu = make_cpu();
        cpu.cpsr |= FLAG_T;

        // B +16 → offset = (16 - 4) / 2 = 6 → 0xE006
        let instr: u16 = 0xE006;
        cpu.memory.write_halfword_at(0, instr);
        cpu.step();
        // target = PC(2) + 6*2 + 2 = 16
        assert_eq!(cpu.gpr[15], 16);
    }

    #[test]
    fn test_arm_ldm_stm() {
        let mut cpu = make_cpu();
        cpu.gpr[0] = 0x11;
        cpu.gpr[1] = 0x22;
        cpu.gpr[2] = 0x33;
        cpu.gpr[4] = 0x200; // Base address

        // STMIA R4!, {R0, R1, R2} → 0xE8A40007
        let stm: u32 = 0xE8A40007;
        cpu.memory.write_word_at(0, stm);
        cpu.step();
        assert_eq!(cpu.gpr[4], 0x20C); // Write-back
        assert_eq!(cpu.memory.read_word(0x200), 0x11);
        assert_eq!(cpu.memory.read_word(0x204), 0x22);
        assert_eq!(cpu.memory.read_word(0x208), 0x33);

        // Clear registers
        cpu.gpr[0] = 0;
        cpu.gpr[1] = 0;
        cpu.gpr[2] = 0;
        cpu.gpr[4] = 0x200;

        // LDMIA R4!, {R0, R1, R2} → 0xE8B40007
        let ldm: u32 = 0xE8B40007;
        cpu.memory.write_word_at(4, ldm);
        cpu.step();
        assert_eq!(cpu.gpr[0], 0x11);
        assert_eq!(cpu.gpr[1], 0x22);
        assert_eq!(cpu.gpr[2], 0x33);
    }

    #[test]
    fn test_arm_mvn() {
        let mut cpu = make_cpu();
        // MVN R0, #0 → R0 = 0xFFFFFFFF
        let instr = arm_encode_imm(COND_AL, 0xF, false, 0, 0, 0, 0);
        cpu.memory.write_word_at(0, instr);
        cpu.step();
        assert_eq!(cpu.gpr[0], 0xFFFFFFFF);
    }

    #[test]
    fn test_arm_bic() {
        let mut cpu = make_cpu();
        cpu.gpr[1] = 0xFF;
        // BIC R0, R1, #0x0F → R0 = 0xF0
        let instr = arm_encode_imm(COND_AL, 0xE, false, 1, 0, 0, 0x0F);
        cpu.memory.write_word_at(0, instr);
        cpu.step();
        assert_eq!(cpu.gpr[0], 0xF0);
    }

    #[test]
    fn test_arm_eor() {
        let mut cpu = make_cpu();
        cpu.gpr[1] = 0xFF;
        cpu.gpr[2] = 0x0F;
        // EOR R0, R1, R2
        let instr = arm_encode_dp(COND_AL, 0x1, false, 1, 0, 2);
        cpu.memory.write_word_at(0, instr);
        cpu.step();
        assert_eq!(cpu.gpr[0], 0xF0);
    }

    #[test]
    fn test_arm_rsb() {
        let mut cpu = make_cpu();
        cpu.gpr[1] = 10;
        // RSB R0, R1, #30 → R0 = 30 - 10 = 20
        let instr = arm_encode_imm(COND_AL, 0x3, false, 1, 0, 0, 30);
        cpu.memory.write_word_at(0, instr);
        cpu.step();
        assert_eq!(cpu.gpr[0], 20);
    }

    #[test]
    fn test_arm_swap() {
        let mut cpu = make_cpu();
        cpu.gpr[1] = 0x100; // address
        cpu.gpr[2] = 0xAA; // value to write
        cpu.memory.write_word_at(0x100, 0xBB); // existing value

        // SWP R0, R2, [R1]: 0xE1010092
        let instr: u32 = 0xE1010092;
        cpu.memory.write_word_at(0, instr);
        cpu.step();
        assert_eq!(cpu.gpr[0], 0xBB); // Old value read
        assert_eq!(cpu.memory.read_word(0x100), 0xAA); // New value written
    }

    #[test]
    fn test_arm_mrs_msr() {
        let mut cpu = make_cpu();

        // MRS R0, CPSR: 0xE10F0000
        let mrs: u32 = 0xE10F0000;
        cpu.memory.write_word_at(0, mrs);
        cpu.step();
        assert_eq!(cpu.gpr[0], cpu.cpsr);

        // Set some flags via MSR
        cpu.gpr[1] = cpu.cpsr | FLAG_Z | FLAG_C;
        // MSR CPSR_f, R1 (flags only): 0xE128F001
        let msr: u32 = 0xE128F001;
        cpu.memory.write_word_at(4, msr);
        cpu.step();
        assert!(cpu.flag_z());
        assert!(cpu.flag_c());
    }

    #[test]
    fn test_arm_halfword_load_store() {
        let mut cpu = make_cpu();
        cpu.gpr[1] = 0x100;
        cpu.gpr[0] = 0x1234;

        // STRH R0, [R1]: 0xE1C100B0
        let strh: u32 = 0xE1C100B0;
        cpu.memory.write_word_at(0, strh);
        cpu.step();
        assert_eq!(cpu.memory.read_halfword(0x100), 0x1234);

        // LDRH R2, [R1]: 0xE1D120B0
        let ldrh: u32 = 0xE1D120B0;
        cpu.memory.write_word_at(4, ldrh);
        cpu.step();
        assert_eq!(cpu.gpr[2], 0x1234);
    }

    #[test]
    fn test_thumb_lsl() {
        let mut cpu = make_cpu();
        cpu.cpsr |= FLAG_T;
        cpu.gpr[1] = 1;

        // LSL R0, R1, #4 → 0x0108
        let instr: u16 = 0x0108;
        cpu.memory.write_halfword_at(0, instr);
        cpu.step();
        assert_eq!(cpu.gpr[0], 16);
    }

    #[test]
    fn test_thumb_cmp() {
        let mut cpu = make_cpu();
        cpu.cpsr |= FLAG_T;
        cpu.gpr[0] = 42;

        // CMP R0, #42 → 0x282A
        let instr: u16 = 0x282A;
        cpu.memory.write_halfword_at(0, instr);
        cpu.step();
        assert!(cpu.flag_z());
    }
}
