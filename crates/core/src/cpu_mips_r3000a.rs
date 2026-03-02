//! MIPS R3000A CPU — PlayStation 1
//!
//! 32-bit MIPS I processor running at 33.8688 MHz.
//!
//! ## Features
//! - 32 × 32-bit general purpose registers
//! - HI/LO multiply/divide result registers
//! - COP0 system control coprocessor (exception handling, status)
//! - COP2 GTE (Geometry Transform Engine) — 3D math coprocessor
//! - 4 KB instruction cache, 1 KB data cache (scratchpad)
//! - Branch delay slots
//! - No FPU (COP1)
//! - No TLB (fixed KSEG segments)
//!
//! ## Memory Segments
//! | Segment  | Address Range         | Description              |
//! |----------|-----------------------|--------------------------|
//! | KUSEG    | 0x00000000-0x7FFFFFFF | User, cached             |
//! | KSEG0    | 0x80000000-0x9FFFFFFF | Kernel, cached           |
//! | KSEG1    | 0xA0000000-0xBFFFFFFF | Kernel, uncached         |
//! | KSEG2    | 0xC0000000-0xFFFFFFFF | Kernel, virtual          |
//!
//! ## References
//! - IDT R30xx Family Software Reference Manual
//! - nocash PSX specifications (https://problemkaputt.de/psx-spx.htm)

/// Memory interface for the R3000A CPU.
pub trait MemoryR3000A {
    fn read_byte(&self, addr: u32) -> u8;
    fn read_halfword(&self, addr: u32) -> u16;
    fn read_word(&self, addr: u32) -> u32;
    fn write_byte(&mut self, addr: u32, val: u8);
    fn write_halfword(&mut self, addr: u32, val: u16);
    fn write_word(&mut self, addr: u32, val: u32);

    /// Check if an IRQ is pending (active and unmasked).
    fn irq_pending(&self) -> bool {
        false
    }
}

// ============================================================================
// COP0 register indices
// ============================================================================

/// COP0 register 3: BPC — Breakpoint on execute
pub const COP0_BPC: usize = 3;
/// COP0 register 5: BDA — Breakpoint on data access
pub const COP0_BDA: usize = 5;
/// COP0 register 6: JUMPDEST — Target address on jump
pub const COP0_JUMPDEST: usize = 6;
/// COP0 register 7: DCIC — Debug and cache invalidate control
pub const COP0_DCIC: usize = 7;
/// COP0 register 8: BadVaddr — Bad virtual address
pub const COP0_BADVADDR: usize = 8;
/// COP0 register 9: BDAM — Data access breakpoint mask
pub const COP0_BDAM: usize = 9;
/// COP0 register 11: BPCM — Execute breakpoint mask
pub const COP0_BPCM: usize = 11;
/// COP0 register 12: SR — Status Register
pub const COP0_SR: usize = 12;
/// COP0 register 13: Cause — Exception cause
pub const COP0_CAUSE: usize = 13;
/// COP0 register 14: EPC — Exception Program Counter
pub const COP0_EPC: usize = 14;
/// COP0 register 15: PRId — Processor Revision Identifier
pub const COP0_PRID: usize = 15;

// ============================================================================
// Status Register (SR) bits
// ============================================================================

/// IEc — Current interrupt enable
const SR_IEC: u32 = 1 << 0;
/// KUc — Current kernel/user mode (0=kernel)
#[allow(dead_code)]
const SR_KUC: u32 = 1 << 1;
/// IEp — Previous interrupt enable
#[allow(dead_code)]
const SR_IEP: u32 = 1 << 2;
/// KUp — Previous kernel/user mode
#[allow(dead_code)]
const SR_KUP: u32 = 1 << 3;
/// IEo — Old interrupt enable
#[allow(dead_code)]
const SR_IEO: u32 = 1 << 4;
/// KUo — Old kernel/user mode
#[allow(dead_code)]
const SR_KUO: u32 = 1 << 5;
/// Interrupt mask bits (bits 8-15)
const SR_IM_MASK: u32 = 0xFF << 8;
/// IsC — Isolate cache (writes go to cache only)
const SR_ISC: u32 = 1 << 16;
/// BEV — Boot exception vectors (0=normal, 1=ROM)
const SR_BEV: u32 = 1 << 22;

// ============================================================================
// Cause register bits
// ============================================================================

/// Exception code field (bits 2-6)
const CAUSE_EXCODE_MASK: u32 = 0x1F << 2;
/// Pending interrupt bits (bits 8-15)
const CAUSE_IP_MASK: u32 = 0xFF << 8;
/// Branch delay flag (bit 31)
const CAUSE_BD: u32 = 1 << 31;

// ============================================================================
// Exception codes
// ============================================================================

const EXCODE_INT: u32 = 0x00; // Interrupt
const EXCODE_ADEL: u32 = 0x04; // Address error (load/fetch)
const EXCODE_ADES: u32 = 0x05; // Address error (store)
const EXCODE_SYS: u32 = 0x08; // Syscall
const EXCODE_BP: u32 = 0x09; // Breakpoint
const EXCODE_RI: u32 = 0x0A; // Reserved instruction
#[allow(dead_code)]
const EXCODE_CPU: u32 = 0x0B; // Coprocessor unusable
const EXCODE_OVF: u32 = 0x0C; // Arithmetic overflow

// ============================================================================
// Opcode constants
// ============================================================================

const OP_SPECIAL: u32 = 0x00;
const OP_BCOND: u32 = 0x01;
const OP_J: u32 = 0x02;
const OP_JAL: u32 = 0x03;
const OP_BEQ: u32 = 0x04;
const OP_BNE: u32 = 0x05;
const OP_BLEZ: u32 = 0x06;
const OP_BGTZ: u32 = 0x07;
const OP_ADDI: u32 = 0x08;
const OP_ADDIU: u32 = 0x09;
const OP_SLTI: u32 = 0x0A;
const OP_SLTIU: u32 = 0x0B;
const OP_ANDI: u32 = 0x0C;
const OP_ORI: u32 = 0x0D;
const OP_XORI: u32 = 0x0E;
const OP_LUI: u32 = 0x0F;
const OP_COP0: u32 = 0x10;
#[allow(dead_code)]
const OP_COP1: u32 = 0x11; // PS1 has no FPU
const OP_COP2: u32 = 0x12; // GTE
#[allow(dead_code)]
const OP_COP3: u32 = 0x13;
const OP_LB: u32 = 0x20;
const OP_LH: u32 = 0x21;
const OP_LWL: u32 = 0x22;
const OP_LW: u32 = 0x23;
const OP_LBU: u32 = 0x24;
const OP_LHU: u32 = 0x25;
const OP_LWR: u32 = 0x26;
const OP_SB: u32 = 0x28;
const OP_SH: u32 = 0x29;
const OP_SWL: u32 = 0x2A;
const OP_SW: u32 = 0x2B;
const OP_SWR: u32 = 0x2E;
const OP_LWC2: u32 = 0x32; // Load word to GTE
const OP_SWC2: u32 = 0x3A; // Store word from GTE

// SPECIAL function codes
const FUNCT_SLL: u32 = 0x00;
const FUNCT_SRL: u32 = 0x02;
const FUNCT_SRA: u32 = 0x03;
const FUNCT_SLLV: u32 = 0x04;
const FUNCT_SRLV: u32 = 0x06;
const FUNCT_SRAV: u32 = 0x07;
const FUNCT_JR: u32 = 0x08;
const FUNCT_JALR: u32 = 0x09;
const FUNCT_SYSCALL: u32 = 0x0C;
const FUNCT_BREAK: u32 = 0x0D;
const FUNCT_MFHI: u32 = 0x10;
const FUNCT_MTHI: u32 = 0x11;
const FUNCT_MFLO: u32 = 0x12;
const FUNCT_MTLO: u32 = 0x13;
const FUNCT_MULT: u32 = 0x18;
const FUNCT_MULTU: u32 = 0x19;
const FUNCT_DIV: u32 = 0x1A;
const FUNCT_DIVU: u32 = 0x1B;
const FUNCT_ADD: u32 = 0x20;
const FUNCT_ADDU: u32 = 0x21;
const FUNCT_SUB: u32 = 0x22;
const FUNCT_SUBU: u32 = 0x23;
const FUNCT_AND: u32 = 0x24;
const FUNCT_OR: u32 = 0x25;
const FUNCT_XOR: u32 = 0x26;
const FUNCT_NOR: u32 = 0x27;
const FUNCT_SLT: u32 = 0x2A;
const FUNCT_SLTU: u32 = 0x2B;

// ============================================================================
// GTE (Geometry Transform Engine) — COP2
// ============================================================================

/// GTE data registers (32 × 32-bit)
#[derive(Debug, Clone, Default)]
pub struct GteRegisters {
    /// Data registers (COP2 data, accessed via MFC2/MTC2/LWC2/SWC2)
    pub data: [u32; 32],
    /// Control registers (COP2 control, accessed via CFC2/CTC2)
    pub control: [u32; 32],
}

impl GteRegisters {
    pub fn new() -> Self {
        Self::default()
    }

    /// Execute a GTE command.
    /// `command` is bits 0-24 of the COP2 instruction.
    pub fn execute(&mut self, command: u32) {
        let opcode = command & 0x3F;
        let _sf = (command >> 19) & 1; // Shift fraction
        let _mx = (command >> 17) & 3; // Matrix selection
        let _v = (command >> 15) & 3; // Vector selection
        let _cv = (command >> 13) & 3; // Translation vector
        let _lm = (command >> 10) & 1; // Limiter

        // Clear FLAG register (control register 31) error bits for new command
        self.control[31] &= 0x7FFFF000;

        match opcode {
            0x01 => self.cmd_rtps(),         // Perspective transform single
            0x06 => self.cmd_nclip(),        // Normal clipping
            0x0C => self.cmd_op(),           // Outer product
            0x10 => self.cmd_dpcs(),         // Depth cue single
            0x11 => self.cmd_intpl(),        // Interpolation
            0x12 => self.cmd_mvmva(command), // Multiply vector by matrix and add
            0x13 => self.cmd_ncds(),         // Normal color depth single
            0x14 => self.cmd_cdp(),          // Color depth cue
            0x16 => self.cmd_ncdt(),         // Normal color depth triple
            0x1B => self.cmd_nccs(),         // Normal color color single
            0x1C => self.cmd_cc(),           // Color color
            0x1E => self.cmd_ncs(),          // Normal color single
            0x20 => self.cmd_nct(),          // Normal color triple
            0x28 => self.cmd_sqr(),          // Square of vector
            0x29 => self.cmd_dcpl(),         // Depth cue color light
            0x2A => self.cmd_dpct(),         // Depth cue triple
            0x2D => self.cmd_avsz3(),        // Average Z (3 values)
            0x2E => self.cmd_avsz4(),        // Average Z (4 values)
            0x30 => self.cmd_rtpt(),         // Perspective transform triple
            0x3D => self.cmd_gpf(),          // General purpose interpolation
            0x3E => self.cmd_gpl(),          // General purpose interpolation with base
            0x3F => self.cmd_ncct(),         // Normal color color triple
            _ => {
                // Unknown GTE command — no-op
            }
        }

        // Update FLAG register bit 31 (error summary)
        let flag = self.control[31];
        if flag & 0x7F87E000 != 0 {
            self.control[31] = flag | (1 << 31);
        }
    }

    // ========================================================================
    // GTE command stubs — each needs full implementation for accurate 3D
    // ========================================================================

    fn cmd_rtps(&mut self) {
        // TODO: Rotate, translate, perspective transform single vertex
        // This is the most critical GTE command — transforms a 3D vertex
        // to 2D screen coordinates with depth.
    }

    fn cmd_rtpt(&mut self) {
        // TODO: Like RTPS but for 3 vertices at once
        // Used heavily by 3D games to transform triangle vertices.
        self.cmd_rtps();
        self.cmd_rtps();
        self.cmd_rtps();
    }

    fn cmd_nclip(&mut self) {
        // Normal clipping: calculates cross product of 2D screen coords
        // to determine if a triangle is front- or back-facing.
        // MAC0 = SX0*(SY1-SY2) + SX1*(SY2-SY0) + SX2*(SY0-SY1)
        let sx0 = self.data[12] as i16 as i64;
        let sy0 = (self.data[12] >> 16) as i16 as i64;
        let sx1 = self.data[13] as i16 as i64;
        let sy1 = (self.data[13] >> 16) as i16 as i64;
        let sx2 = self.data[14] as i16 as i64;
        let sy2 = (self.data[14] >> 16) as i16 as i64;
        let mac0 = sx0 * (sy1 - sy2) + sx1 * (sy2 - sy0) + sx2 * (sy0 - sy1);
        self.data[24] = mac0 as i32 as u32; // MAC0
    }

    fn cmd_op(&mut self) {
        // TODO: Outer product of two vectors
    }

    fn cmd_dpcs(&mut self) {
        // TODO: Depth cue single
    }

    fn cmd_intpl(&mut self) {
        // TODO: Interpolation
    }

    fn cmd_mvmva(&mut self, _command: u32) {
        // TODO: Multiply vector by matrix and add vector
        // This is the general-purpose matrix*vector+vector operation
    }

    fn cmd_ncds(&mut self) {
        // TODO: Normal color depth single
    }

    fn cmd_cdp(&mut self) {
        // TODO: Color depth cue
    }

    fn cmd_ncdt(&mut self) {
        // TODO: Normal color depth triple
    }

    fn cmd_nccs(&mut self) {
        // TODO: Normal color color single
    }

    fn cmd_cc(&mut self) {
        // TODO: Color color
    }

    fn cmd_ncs(&mut self) {
        // TODO: Normal color single
    }

    fn cmd_nct(&mut self) {
        // TODO: Normal color triple
    }

    fn cmd_sqr(&mut self) {
        // TODO: Square of vector components
    }

    fn cmd_dcpl(&mut self) {
        // TODO: Depth cue color light
    }

    fn cmd_dpct(&mut self) {
        // TODO: Depth cue triple
    }

    fn cmd_avsz3(&mut self) {
        // Average Z value (3 vertices)
        // MAC0 = ZSF3 * (SZ1 + SZ2 + SZ3)
        let zsf3 = self.control[29] as i16 as i64;
        let sz1 = (self.data[17] & 0xFFFF) as i64;
        let sz2 = (self.data[18] & 0xFFFF) as i64;
        let sz3 = (self.data[19] & 0xFFFF) as i64;
        let mac0 = zsf3 * (sz1 + sz2 + sz3);
        self.data[24] = mac0 as i32 as u32; // MAC0
        self.data[7] = (mac0 >> 12).clamp(0, 0xFFFF) as u32; // OTZ
    }

    fn cmd_avsz4(&mut self) {
        // Average Z value (4 vertices)
        let zsf4 = self.control[30] as i16 as i64;
        let sz0 = (self.data[16] & 0xFFFF) as i64;
        let sz1 = (self.data[17] & 0xFFFF) as i64;
        let sz2 = (self.data[18] & 0xFFFF) as i64;
        let sz3 = (self.data[19] & 0xFFFF) as i64;
        let mac0 = zsf4 * (sz0 + sz1 + sz2 + sz3);
        self.data[24] = mac0 as i32 as u32; // MAC0
        self.data[7] = (mac0 >> 12).clamp(0, 0xFFFF) as u32; // OTZ
    }

    fn cmd_gpf(&mut self) {
        // TODO: General purpose interpolation
    }

    fn cmd_gpl(&mut self) {
        // TODO: General purpose interpolation with base
    }

    fn cmd_ncct(&mut self) {
        // TODO: Normal color color triple
    }
}

// ============================================================================
// MIPS R3000A CPU
// ============================================================================

/// MIPS R3000A CPU for PlayStation 1.
pub struct CpuR3000A<M: MemoryR3000A> {
    /// General Purpose Registers (32 × 32-bit). R0 is always zero.
    pub gpr: [u32; 32],
    /// Program Counter
    pub pc: u32,
    /// Next PC (for branch delay slot handling)
    next_pc: u32,
    /// Currently in a branch delay slot
    in_delay_slot: bool,
    /// Multiply/divide result high
    pub hi: u32,
    /// Multiply/divide result low
    pub lo: u32,
    /// COP0 system control registers (32 × 32-bit)
    pub cop0: [u32; 32],
    /// GTE (COP2) registers
    pub gte: GteRegisters,
    /// Cycle counter
    pub cycles: u64,
    /// Memory bus
    pub memory: M,
    /// Pending load — for load delay slot emulation: (register, value)
    load_delay: (usize, u32),
}

impl<M: MemoryR3000A> CpuR3000A<M> {
    /// Create a new R3000A with the given memory bus.
    pub fn new(memory: M) -> Self {
        let mut cpu = Self {
            gpr: [0; 32],
            pc: 0xBFC0_0000, // Reset vector (BIOS entry)
            next_pc: 0xBFC0_0004,
            in_delay_slot: false,
            hi: 0,
            lo: 0,
            cop0: [0; 32],
            gte: GteRegisters::new(),
            cycles: 0,
            memory,
            load_delay: (0, 0), // No pending load (reg 0 = zero, discarded)
        };

        // PRId: R3000A processor identification
        cpu.cop0[COP0_PRID] = 0x0000_0002;

        cpu
    }

    /// Reset the CPU.
    pub fn reset(&mut self) {
        self.gpr = [0; 32];
        self.pc = 0xBFC0_0000;
        self.next_pc = 0xBFC0_0004;
        self.in_delay_slot = false;
        self.hi = 0;
        self.lo = 0;
        self.cop0 = [0; 32];
        self.cop0[COP0_PRID] = 0x0000_0002;
        self.cycles = 0;
        self.load_delay = (0, 0);
    }

    /// Get current PC.
    pub fn pc(&self) -> u32 {
        self.pc
    }

    // ========================================================================
    // Register access (R0 is hardwired to 0)
    // ========================================================================

    fn reg(&self, idx: u32) -> u32 {
        self.gpr[idx as usize]
    }

    fn set_reg(&mut self, idx: u32, val: u32) {
        self.gpr[idx as usize] = val;
        self.gpr[0] = 0; // R0 is always zero
    }

    // ========================================================================
    // Load delay slot
    // ========================================================================

    /// Apply the pending load delay and set a new one.
    fn apply_load_delay(&mut self) {
        let (reg, val) = self.load_delay;
        self.set_reg(reg as u32, val);
        self.load_delay = (0, 0);
    }

    /// Schedule a load for the delay slot.
    fn set_load(&mut self, reg: u32, val: u32) {
        self.load_delay = (reg as usize, val);
    }

    // ========================================================================
    // Memory access (with segment translation)
    // ========================================================================

    /// Translate virtual address to physical by masking off the top 3 bits
    /// for KSEG0/KSEG1, or passing through for KUSEG.
    /// KSEG2 addresses are also masked for PS1 (only 0xFFFE0130 is used).
    fn translate_addr(&self, vaddr: u32) -> u32 {
        // KSEG0 (0x80000000-0x9FFFFFFF) and KSEG1 (0xA0000000-0xBFFFFFFF)
        // both map to physical 0x00000000-0x1FFFFFFF
        // KSEG2 (0xC0000000-0xFFFFFFFF): On PS1, only 0xFFFE0130 is used
        match vaddr >> 29 {
            0b100 | 0b101 => vaddr & 0x1FFF_FFFF, // KSEG0 / KSEG1
            0b110 | 0b111 => vaddr & 0x1FFF_FFFF, // KSEG2 (cache control region)
            _ => vaddr,                           // KUSEG
        }
    }

    fn load8(&self, addr: u32) -> u8 {
        let phys = self.translate_addr(addr);
        self.memory.read_byte(phys)
    }

    fn load16(&self, addr: u32) -> u16 {
        let phys = self.translate_addr(addr);
        self.memory.read_halfword(phys)
    }

    fn load32(&self, addr: u32) -> u32 {
        let phys = self.translate_addr(addr);
        self.memory.read_word(phys)
    }

    fn store8(&mut self, addr: u32, val: u8) {
        // If cache is isolated, writes go to cache only (ignored for now)
        if self.cop0[COP0_SR] & SR_ISC != 0 {
            return;
        }
        let phys = self.translate_addr(addr);
        self.memory.write_byte(phys, val);
    }

    fn store16(&mut self, addr: u32, val: u16) {
        if self.cop0[COP0_SR] & SR_ISC != 0 {
            return;
        }
        let phys = self.translate_addr(addr);
        self.memory.write_halfword(phys, val);
    }

    fn store32(&mut self, addr: u32, val: u32) {
        if self.cop0[COP0_SR] & SR_ISC != 0 {
            return;
        }
        let phys = self.translate_addr(addr);
        self.memory.write_word(phys, val);
    }

    // ========================================================================
    // Branch helper
    // ========================================================================

    fn branch(&mut self, offset: u32) {
        let offset = (offset as i16 as i32 as u32) << 2;
        self.next_pc = self.pc.wrapping_add(offset);
        // Note: self.pc already advanced past the branch instruction
        // so offset is relative to (branch instruction address + 4)
    }

    // ========================================================================
    // Exception handling
    // ========================================================================

    fn exception(&mut self, excode: u32) {
        // Push the KU/IE stack (fields 0-5 of SR shifted left by 2)
        let sr = self.cop0[COP0_SR];
        self.cop0[COP0_SR] = (sr & !0x3F) | ((sr & 0x0F) << 2);

        // Set exception code in Cause register
        self.cop0[COP0_CAUSE] =
            (self.cop0[COP0_CAUSE] & !CAUSE_EXCODE_MASK) | ((excode << 2) & CAUSE_EXCODE_MASK);

        // Set BD bit if we're in a delay slot
        if self.in_delay_slot {
            self.cop0[COP0_CAUSE] |= CAUSE_BD;
            // EPC points to the branch instruction, not the delay slot
            self.cop0[COP0_EPC] = self.pc.wrapping_sub(4);
        } else {
            self.cop0[COP0_CAUSE] &= !CAUSE_BD;
            self.cop0[COP0_EPC] = self.pc;
        }

        // Jump to exception vector
        let vector = if sr & SR_BEV != 0 {
            0xBFC0_0180 // ROM exception vector
        } else {
            0x8000_0080 // RAM exception vector
        };

        self.pc = vector;
        self.next_pc = vector.wrapping_add(4);
        self.in_delay_slot = false;
    }

    // ========================================================================
    // Main execution step
    // ========================================================================

    /// Execute one instruction. Returns the number of cycles consumed.
    pub fn step(&mut self) -> u32 {
        // Check for pending interrupts
        let sr = self.cop0[COP0_SR];
        let _cause = self.cop0[COP0_CAUSE];

        // Set hardware IRQ bit in Cause if bus signals an interrupt
        if self.memory.irq_pending() {
            self.cop0[COP0_CAUSE] |= 1 << 10; // IP2 (hardware IRQ)
        } else {
            self.cop0[COP0_CAUSE] &= !(1 << 10);
        }

        // Fire interrupt if enabled and pending
        if sr & SR_IEC != 0 {
            let pending = self.cop0[COP0_CAUSE] & CAUSE_IP_MASK;
            let enabled = sr & SR_IM_MASK;
            if pending & enabled != 0 {
                self.exception(EXCODE_INT);
                return 1;
            }
        }

        // Fetch instruction
        let instruction = self.load32(self.pc);

        // Advance PC (delay slot handling)
        let current_pc = self.pc;
        self.pc = self.next_pc;
        self.next_pc = self.next_pc.wrapping_add(4);

        let was_in_delay = self.in_delay_slot;
        self.in_delay_slot = false;

        // Apply pending load delay from previous instruction
        self.apply_load_delay();

        // Decode
        let opcode = instruction >> 26;
        let rs = (instruction >> 21) & 0x1F;
        let rt = (instruction >> 16) & 0x1F;
        let rd = (instruction >> 11) & 0x1F;
        let sa = (instruction >> 6) & 0x1F;
        let funct = instruction & 0x3F;
        let imm16 = instruction & 0xFFFF;
        let imm_se = imm16 as i16 as i32 as u32; // Sign-extended
        let target = instruction & 0x03FF_FFFF;

        match opcode {
            OP_SPECIAL => match funct {
                FUNCT_SLL => {
                    let val = self.reg(rt) << sa;
                    self.set_reg(rd, val);
                }
                FUNCT_SRL => {
                    let val = self.reg(rt) >> sa;
                    self.set_reg(rd, val);
                }
                FUNCT_SRA => {
                    let val = (self.reg(rt) as i32 >> sa) as u32;
                    self.set_reg(rd, val);
                }
                FUNCT_SLLV => {
                    let val = self.reg(rt) << (self.reg(rs) & 0x1F);
                    self.set_reg(rd, val);
                }
                FUNCT_SRLV => {
                    let val = self.reg(rt) >> (self.reg(rs) & 0x1F);
                    self.set_reg(rd, val);
                }
                FUNCT_SRAV => {
                    let val = (self.reg(rt) as i32 >> (self.reg(rs) & 0x1F)) as u32;
                    self.set_reg(rd, val);
                }
                FUNCT_JR => {
                    self.next_pc = self.reg(rs);
                    self.in_delay_slot = true;
                }
                FUNCT_JALR => {
                    let return_addr = self.next_pc; // Address after delay slot
                    self.next_pc = self.reg(rs);
                    self.set_reg(rd, return_addr);
                    self.in_delay_slot = true;
                }
                FUNCT_SYSCALL => {
                    self.pc = current_pc; // Exception at current instruction
                    self.in_delay_slot = was_in_delay;
                    self.exception(EXCODE_SYS);
                }
                FUNCT_BREAK => {
                    self.pc = current_pc;
                    self.in_delay_slot = was_in_delay;
                    self.exception(EXCODE_BP);
                }
                FUNCT_MFHI => {
                    self.set_reg(rd, self.hi);
                }
                FUNCT_MTHI => {
                    self.hi = self.reg(rs);
                }
                FUNCT_MFLO => {
                    self.set_reg(rd, self.lo);
                }
                FUNCT_MTLO => {
                    self.lo = self.reg(rs);
                }
                FUNCT_MULT => {
                    let a = self.reg(rs) as i32 as i64;
                    let b = self.reg(rt) as i32 as i64;
                    let result = a * b;
                    self.lo = result as u32;
                    self.hi = (result >> 32) as u32;
                }
                FUNCT_MULTU => {
                    let a = self.reg(rs) as u64;
                    let b = self.reg(rt) as u64;
                    let result = a * b;
                    self.lo = result as u32;
                    self.hi = (result >> 32) as u32;
                }
                FUNCT_DIV => {
                    let n = self.reg(rs) as i32;
                    let d = self.reg(rt) as i32;
                    if d == 0 {
                        // Division by zero: defined behavior on R3000A
                        self.lo = if n >= 0 { 0xFFFF_FFFF } else { 1 };
                        self.hi = n as u32;
                    } else if n as u32 == 0x8000_0000 && d == -1 {
                        // Overflow
                        self.lo = 0x8000_0000;
                        self.hi = 0;
                    } else {
                        self.lo = (n / d) as u32;
                        self.hi = (n % d) as u32;
                    }
                }
                FUNCT_DIVU => {
                    let n = self.reg(rs);
                    let d = self.reg(rt);
                    if d == 0 {
                        self.lo = 0xFFFF_FFFF;
                        self.hi = n;
                    } else {
                        self.lo = n / d;
                        self.hi = n % d;
                    }
                }
                FUNCT_ADD => {
                    let a = self.reg(rs) as i32;
                    let b = self.reg(rt) as i32;
                    match a.checked_add(b) {
                        Some(result) => self.set_reg(rd, result as u32),
                        None => {
                            self.pc = current_pc;
                            self.in_delay_slot = was_in_delay;
                            self.exception(EXCODE_OVF);
                        }
                    }
                }
                FUNCT_ADDU => {
                    let val = self.reg(rs).wrapping_add(self.reg(rt));
                    self.set_reg(rd, val);
                }
                FUNCT_SUB => {
                    let a = self.reg(rs) as i32;
                    let b = self.reg(rt) as i32;
                    match a.checked_sub(b) {
                        Some(result) => self.set_reg(rd, result as u32),
                        None => {
                            self.pc = current_pc;
                            self.in_delay_slot = was_in_delay;
                            self.exception(EXCODE_OVF);
                        }
                    }
                }
                FUNCT_SUBU => {
                    let val = self.reg(rs).wrapping_sub(self.reg(rt));
                    self.set_reg(rd, val);
                }
                FUNCT_AND => {
                    self.set_reg(rd, self.reg(rs) & self.reg(rt));
                }
                FUNCT_OR => {
                    self.set_reg(rd, self.reg(rs) | self.reg(rt));
                }
                FUNCT_XOR => {
                    self.set_reg(rd, self.reg(rs) ^ self.reg(rt));
                }
                FUNCT_NOR => {
                    self.set_reg(rd, !(self.reg(rs) | self.reg(rt)));
                }
                FUNCT_SLT => {
                    let val = if (self.reg(rs) as i32) < (self.reg(rt) as i32) {
                        1
                    } else {
                        0
                    };
                    self.set_reg(rd, val);
                }
                FUNCT_SLTU => {
                    let val = if self.reg(rs) < self.reg(rt) { 1 } else { 0 };
                    self.set_reg(rd, val);
                }
                _ => {
                    // Reserved instruction
                    self.pc = current_pc;
                    self.in_delay_slot = was_in_delay;
                    self.exception(EXCODE_RI);
                }
            },

            OP_BCOND => {
                // BLTZ, BGEZ, BLTZAL, BGEZAL
                let test = (self.reg(rs) as i32) < 0;
                let link = rt & 0x10 != 0; // Bit 4: link (save return address)
                let branch_if = rt & 0x01 != 0; // Bit 0: 0=LTZ, 1=GEZ

                let cond = if branch_if { !test } else { test };

                if link {
                    self.set_reg(31, self.next_pc);
                }

                if cond {
                    self.branch(imm16);
                }
                self.in_delay_slot = true;
            }

            OP_J => {
                self.next_pc = (self.pc & 0xF000_0000) | (target << 2);
                self.in_delay_slot = true;
            }
            OP_JAL => {
                self.set_reg(31, self.next_pc);
                self.next_pc = (self.pc & 0xF000_0000) | (target << 2);
                self.in_delay_slot = true;
            }
            OP_BEQ => {
                if self.reg(rs) == self.reg(rt) {
                    self.branch(imm16);
                }
                self.in_delay_slot = true;
            }
            OP_BNE => {
                if self.reg(rs) != self.reg(rt) {
                    self.branch(imm16);
                }
                self.in_delay_slot = true;
            }
            OP_BLEZ => {
                if (self.reg(rs) as i32) <= 0 {
                    self.branch(imm16);
                }
                self.in_delay_slot = true;
            }
            OP_BGTZ => {
                if (self.reg(rs) as i32) > 0 {
                    self.branch(imm16);
                }
                self.in_delay_slot = true;
            }

            OP_ADDI => {
                let a = self.reg(rs) as i32;
                let b = imm_se as i32;
                match a.checked_add(b) {
                    Some(result) => self.set_reg(rt, result as u32),
                    None => {
                        self.pc = current_pc;
                        self.in_delay_slot = was_in_delay;
                        self.exception(EXCODE_OVF);
                    }
                }
            }
            OP_ADDIU => {
                self.set_reg(rt, self.reg(rs).wrapping_add(imm_se));
            }
            OP_SLTI => {
                let val = if (self.reg(rs) as i32) < (imm_se as i32) {
                    1
                } else {
                    0
                };
                self.set_reg(rt, val);
            }
            OP_SLTIU => {
                let val = if self.reg(rs) < imm_se { 1 } else { 0 };
                self.set_reg(rt, val);
            }
            OP_ANDI => {
                self.set_reg(rt, self.reg(rs) & imm16);
            }
            OP_ORI => {
                self.set_reg(rt, self.reg(rs) | imm16);
            }
            OP_XORI => {
                self.set_reg(rt, self.reg(rs) ^ imm16);
            }
            OP_LUI => {
                self.set_reg(rt, imm16 << 16);
            }

            // ================================================================
            // COP0 — System Control Coprocessor
            // ================================================================
            OP_COP0 => {
                match rs {
                    0x00 => {
                        // MFC0 rt, rd — Move from COP0
                        let val = self.cop0[rd as usize];
                        self.set_load(rt, val);
                    }
                    0x04 => {
                        // MTC0 rt, rd — Move to COP0
                        self.cop0[rd as usize] = self.reg(rt);
                    }
                    0x10 => {
                        // RFE — Return from exception
                        // Restore KU/IE stack (right-shift bits 0-5 by 2)
                        let sr = self.cop0[COP0_SR];
                        self.cop0[COP0_SR] = (sr & !0x0F) | ((sr >> 2) & 0x0F);
                    }
                    _ => {
                        // Unknown COP0 sub-op
                    }
                }
            }

            // ================================================================
            // COP2 — GTE (Geometry Transform Engine)
            // ================================================================
            OP_COP2 => {
                // Check if COP2 is accessible (CU2 bit in SR, bit 30)
                // PS1 BIOS usually sets this early. Some games access without it.
                // For compatibility, allow access regardless.
                if rs & 0x10 != 0 {
                    // COP2 command (bit 25 set): execute GTE operation
                    self.gte.execute(instruction & 0x01FF_FFFF);
                } else {
                    match rs {
                        0x00 => {
                            // MFC2 rt, rd — Move from GTE data register
                            let val = self.gte.data[rd as usize];
                            self.set_load(rt, val);
                        }
                        0x02 => {
                            // CFC2 rt, rd — Move from GTE control register
                            let val = self.gte.control[rd as usize];
                            self.set_load(rt, val);
                        }
                        0x04 => {
                            // MTC2 rt, rd — Move to GTE data register
                            self.gte.data[rd as usize] = self.reg(rt);
                        }
                        0x06 => {
                            // CTC2 rt, rd — Move to GTE control register
                            self.gte.control[rd as usize] = self.reg(rt);
                        }
                        _ => {}
                    }
                }
            }

            // ================================================================
            // Load instructions
            // ================================================================
            OP_LB => {
                let addr = self.reg(rs).wrapping_add(imm_se);
                let val = self.load8(addr) as i8 as i32 as u32;
                self.set_load(rt, val);
            }
            OP_LH => {
                let addr = self.reg(rs).wrapping_add(imm_se);
                if addr & 1 != 0 {
                    self.cop0[COP0_BADVADDR] = addr;
                    self.pc = current_pc;
                    self.in_delay_slot = was_in_delay;
                    self.exception(EXCODE_ADEL);
                } else {
                    let val = self.load16(addr) as i16 as i32 as u32;
                    self.set_load(rt, val);
                }
            }
            OP_LW => {
                let addr = self.reg(rs).wrapping_add(imm_se);
                if addr & 3 != 0 {
                    self.cop0[COP0_BADVADDR] = addr;
                    self.pc = current_pc;
                    self.in_delay_slot = was_in_delay;
                    self.exception(EXCODE_ADEL);
                } else {
                    let val = self.load32(addr);
                    self.set_load(rt, val);
                }
            }
            OP_LBU => {
                let addr = self.reg(rs).wrapping_add(imm_se);
                let val = self.load8(addr) as u32;
                self.set_load(rt, val);
            }
            OP_LHU => {
                let addr = self.reg(rs).wrapping_add(imm_se);
                if addr & 1 != 0 {
                    self.cop0[COP0_BADVADDR] = addr;
                    self.pc = current_pc;
                    self.in_delay_slot = was_in_delay;
                    self.exception(EXCODE_ADEL);
                } else {
                    let val = self.load16(addr) as u32;
                    self.set_load(rt, val);
                }
            }
            OP_LWL => {
                // Load word left (unaligned load helper)
                let addr = self.reg(rs).wrapping_add(imm_se);
                let aligned = addr & !3;
                let mem = self.load32(aligned);
                let cur = self.reg(rt); // Current value in target register
                let val = match addr & 3 {
                    0 => (cur & 0x00FF_FFFF) | (mem << 24),
                    1 => (cur & 0x0000_FFFF) | (mem << 16),
                    2 => (cur & 0x0000_00FF) | (mem << 8),
                    3 => mem,
                    _ => unreachable!(),
                };
                self.set_load(rt, val);
            }
            OP_LWR => {
                // Load word right (unaligned load helper)
                let addr = self.reg(rs).wrapping_add(imm_se);
                let aligned = addr & !3;
                let mem = self.load32(aligned);
                let cur = self.reg(rt);
                let val = match addr & 3 {
                    0 => mem,
                    1 => (cur & 0xFF00_0000) | (mem >> 8),
                    2 => (cur & 0xFFFF_0000) | (mem >> 16),
                    3 => (cur & 0xFFFF_FF00) | (mem >> 24),
                    _ => unreachable!(),
                };
                self.set_load(rt, val);
            }
            OP_LWC2 => {
                // Load word to GTE data register
                let addr = self.reg(rs).wrapping_add(imm_se);
                if addr & 3 != 0 {
                    self.cop0[COP0_BADVADDR] = addr;
                    self.pc = current_pc;
                    self.in_delay_slot = was_in_delay;
                    self.exception(EXCODE_ADEL);
                } else {
                    let val = self.load32(addr);
                    self.gte.data[rt as usize] = val;
                }
            }

            // ================================================================
            // Store instructions
            // ================================================================
            OP_SB => {
                let addr = self.reg(rs).wrapping_add(imm_se);
                self.store8(addr, self.reg(rt) as u8);
            }
            OP_SH => {
                let addr = self.reg(rs).wrapping_add(imm_se);
                if addr & 1 != 0 {
                    self.cop0[COP0_BADVADDR] = addr;
                    self.pc = current_pc;
                    self.in_delay_slot = was_in_delay;
                    self.exception(EXCODE_ADES);
                } else {
                    self.store16(addr, self.reg(rt) as u16);
                }
            }
            OP_SW => {
                let addr = self.reg(rs).wrapping_add(imm_se);
                if addr & 3 != 0 {
                    self.cop0[COP0_BADVADDR] = addr;
                    self.pc = current_pc;
                    self.in_delay_slot = was_in_delay;
                    self.exception(EXCODE_ADES);
                } else {
                    self.store32(addr, self.reg(rt));
                }
            }
            OP_SWL => {
                let addr = self.reg(rs).wrapping_add(imm_se);
                let aligned = addr & !3;
                let mem = self.load32(aligned);
                let val = match addr & 3 {
                    0 => (mem & 0xFFFF_FF00) | (self.reg(rt) >> 24),
                    1 => (mem & 0xFFFF_0000) | (self.reg(rt) >> 16),
                    2 => (mem & 0xFF00_0000) | (self.reg(rt) >> 8),
                    3 => self.reg(rt),
                    _ => unreachable!(),
                };
                self.store32(aligned, val);
            }
            OP_SWR => {
                let addr = self.reg(rs).wrapping_add(imm_se);
                let aligned = addr & !3;
                let mem = self.load32(aligned);
                let val = match addr & 3 {
                    0 => self.reg(rt),
                    1 => (mem & 0x0000_00FF) | (self.reg(rt) << 8),
                    2 => (mem & 0x0000_FFFF) | (self.reg(rt) << 16),
                    3 => (mem & 0x00FF_FFFF) | (self.reg(rt) << 24),
                    _ => unreachable!(),
                };
                self.store32(aligned, val);
            }
            OP_SWC2 => {
                // Store word from GTE data register
                let addr = self.reg(rs).wrapping_add(imm_se);
                if addr & 3 != 0 {
                    self.cop0[COP0_BADVADDR] = addr;
                    self.pc = current_pc;
                    self.in_delay_slot = was_in_delay;
                    self.exception(EXCODE_ADES);
                } else {
                    let val = self.gte.data[rt as usize];
                    self.store32(addr, val);
                }
            }

            _ => {
                // Reserved / unimplemented opcode
                self.pc = current_pc;
                self.in_delay_slot = was_in_delay;
                self.exception(EXCODE_RI);
            }
        }

        self.cycles += 1;
        1
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Simple test memory: 4MB flat address space.
    struct TestMemory {
        data: Vec<u8>,
    }

    impl TestMemory {
        fn new() -> Self {
            Self {
                data: vec![0; 4 * 1024 * 1024],
            }
        }
    }

    impl MemoryR3000A for TestMemory {
        fn read_byte(&self, addr: u32) -> u8 {
            let addr = (addr & 0x1FFF_FFFF) as usize % self.data.len();
            self.data.get(addr).copied().unwrap_or(0)
        }

        fn read_halfword(&self, addr: u32) -> u16 {
            let addr = (addr & 0x1FFF_FFFF) as usize % self.data.len();
            let lo = self.data.get(addr).copied().unwrap_or(0) as u16;
            let hi = self.data.get(addr + 1).copied().unwrap_or(0) as u16;
            lo | (hi << 8)
        }

        fn read_word(&self, addr: u32) -> u32 {
            let addr = (addr & 0x1FFF_FFFF) as usize % self.data.len();
            let b0 = self.data.get(addr).copied().unwrap_or(0) as u32;
            let b1 = self.data.get(addr + 1).copied().unwrap_or(0) as u32;
            let b2 = self.data.get(addr + 2).copied().unwrap_or(0) as u32;
            let b3 = self.data.get(addr + 3).copied().unwrap_or(0) as u32;
            b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
        }

        fn write_byte(&mut self, addr: u32, val: u8) {
            let addr = (addr & 0x1FFF_FFFF) as usize % self.data.len();
            if addr < self.data.len() {
                self.data[addr] = val;
            }
        }

        fn write_halfword(&mut self, addr: u32, val: u16) {
            let addr = (addr & 0x1FFF_FFFF) as usize % self.data.len();
            if addr + 1 < self.data.len() {
                self.data[addr] = val as u8;
                self.data[addr + 1] = (val >> 8) as u8;
            }
        }

        fn write_word(&mut self, addr: u32, val: u32) {
            let addr = (addr & 0x1FFF_FFFF) as usize % self.data.len();
            if addr + 3 < self.data.len() {
                self.data[addr] = val as u8;
                self.data[addr + 1] = (val >> 8) as u8;
                self.data[addr + 2] = (val >> 16) as u8;
                self.data[addr + 3] = (val >> 24) as u8;
            }
        }
    }

    fn write_instr(mem: &mut TestMemory, addr: u32, instr: u32) {
        let phys = (addr & 0x1FFF_FFFF) as usize % mem.data.len();
        mem.data[phys] = instr as u8;
        mem.data[phys + 1] = (instr >> 8) as u8;
        mem.data[phys + 2] = (instr >> 16) as u8;
        mem.data[phys + 3] = (instr >> 24) as u8;
    }

    fn make_cpu() -> CpuR3000A<TestMemory> {
        CpuR3000A::new(TestMemory::new())
    }

    // Instruction encoding helpers
    fn encode_r(funct: u32, rs: u32, rt: u32, rd: u32, sa: u32) -> u32 {
        (rs << 21) | (rt << 16) | (rd << 11) | (sa << 6) | funct
    }

    fn encode_i(op: u32, rs: u32, rt: u32, imm: u16) -> u32 {
        (op << 26) | (rs << 21) | (rt << 16) | imm as u32
    }

    fn encode_j(op: u32, target: u32) -> u32 {
        (op << 26) | (target & 0x03FF_FFFF)
    }

    #[test]
    fn test_reset_vector() {
        let cpu = make_cpu();
        assert_eq!(cpu.pc, 0xBFC0_0000);
    }

    #[test]
    fn test_r0_always_zero() {
        let mut cpu = make_cpu();
        cpu.set_reg(0, 0xDEAD_BEEF);
        assert_eq!(cpu.gpr[0], 0);
    }

    #[test]
    fn test_lui() {
        let mut cpu = make_cpu();
        // LUI r1, 0x1234
        write_instr(&mut cpu.memory, 0xBFC0_0000, encode_i(OP_LUI, 0, 1, 0x1234));
        // NOP (so load delay resolves)
        write_instr(&mut cpu.memory, 0xBFC0_0004, 0);
        cpu.step();
        cpu.step();
        assert_eq!(cpu.gpr[1], 0x1234_0000);
    }

    #[test]
    fn test_ori() {
        let mut cpu = make_cpu();
        // LUI r1, 0x8000
        write_instr(&mut cpu.memory, 0xBFC0_0000, encode_i(OP_LUI, 0, 1, 0x8000));
        // ORI r1, r1, 0x0001
        write_instr(&mut cpu.memory, 0xBFC0_0004, encode_i(OP_ORI, 1, 1, 0x0001));
        // NOP
        write_instr(&mut cpu.memory, 0xBFC0_0008, 0);
        cpu.step();
        cpu.step();
        cpu.step();
        assert_eq!(cpu.gpr[1], 0x8000_0001);
    }

    #[test]
    fn test_addiu() {
        let mut cpu = make_cpu();
        // ADDIU r2, r0, 42
        write_instr(&mut cpu.memory, 0xBFC0_0000, encode_i(OP_ADDIU, 0, 2, 42));
        write_instr(&mut cpu.memory, 0xBFC0_0004, 0);
        cpu.step();
        cpu.step();
        assert_eq!(cpu.gpr[2], 42);
    }

    #[test]
    fn test_addu() {
        let mut cpu = make_cpu();
        // ADDIU r1, r0, 10
        write_instr(&mut cpu.memory, 0xBFC0_0000, encode_i(OP_ADDIU, 0, 1, 10));
        // ADDIU r2, r0, 20
        write_instr(&mut cpu.memory, 0xBFC0_0004, encode_i(OP_ADDIU, 0, 2, 20));
        // ADDU r3, r1, r2
        write_instr(
            &mut cpu.memory,
            0xBFC0_0008,
            encode_r(FUNCT_ADDU, 1, 2, 3, 0),
        );
        // NOP
        write_instr(&mut cpu.memory, 0xBFC0_000C, 0);
        cpu.step(); // ADDIU r1
        cpu.step(); // ADDIU r2 (r1 resolved)
        cpu.step(); // ADDU r3 (r2 resolved)
        cpu.step(); // NOP
        assert_eq!(cpu.gpr[3], 30);
    }

    #[test]
    fn test_subu() {
        let mut cpu = make_cpu();
        write_instr(&mut cpu.memory, 0xBFC0_0000, encode_i(OP_ADDIU, 0, 1, 100));
        write_instr(&mut cpu.memory, 0xBFC0_0004, encode_i(OP_ADDIU, 0, 2, 30));
        write_instr(
            &mut cpu.memory,
            0xBFC0_0008,
            encode_r(FUNCT_SUBU, 1, 2, 3, 0),
        );
        write_instr(&mut cpu.memory, 0xBFC0_000C, 0);
        cpu.step();
        cpu.step();
        cpu.step();
        cpu.step();
        assert_eq!(cpu.gpr[3], 70);
    }

    #[test]
    fn test_and_or_xor() {
        let mut cpu = make_cpu();
        write_instr(&mut cpu.memory, 0xBFC0_0000, encode_i(OP_ADDIU, 0, 1, 0xFF));
        write_instr(&mut cpu.memory, 0xBFC0_0004, encode_i(OP_ADDIU, 0, 2, 0x0F));
        // AND r3, r1, r2
        write_instr(
            &mut cpu.memory,
            0xBFC0_0008,
            encode_r(FUNCT_AND, 1, 2, 3, 0),
        );
        write_instr(&mut cpu.memory, 0xBFC0_000C, 0);
        for _ in 0..4 {
            cpu.step();
        }
        assert_eq!(cpu.gpr[3], 0x0F);
    }

    #[test]
    fn test_slt() {
        let mut cpu = make_cpu();
        write_instr(&mut cpu.memory, 0xBFC0_0000, encode_i(OP_ADDIU, 0, 1, 5));
        write_instr(&mut cpu.memory, 0xBFC0_0004, encode_i(OP_ADDIU, 0, 2, 10));
        // SLT r3, r1, r2 (5 < 10 = 1)
        write_instr(
            &mut cpu.memory,
            0xBFC0_0008,
            encode_r(FUNCT_SLT, 1, 2, 3, 0),
        );
        write_instr(&mut cpu.memory, 0xBFC0_000C, 0);
        for _ in 0..4 {
            cpu.step();
        }
        assert_eq!(cpu.gpr[3], 1);
    }

    #[test]
    fn test_shift() {
        let mut cpu = make_cpu();
        write_instr(&mut cpu.memory, 0xBFC0_0000, encode_i(OP_ADDIU, 0, 1, 1));
        // SLL r2, r1, 4 (1 << 4 = 16)
        write_instr(
            &mut cpu.memory,
            0xBFC0_0004,
            encode_r(FUNCT_SLL, 0, 1, 2, 4),
        );
        write_instr(&mut cpu.memory, 0xBFC0_0008, 0);
        for _ in 0..3 {
            cpu.step();
        }
        assert_eq!(cpu.gpr[2], 16);
    }

    #[test]
    fn test_mult_div() {
        let mut cpu = make_cpu();
        write_instr(&mut cpu.memory, 0xBFC0_0000, encode_i(OP_ADDIU, 0, 1, 7));
        write_instr(&mut cpu.memory, 0xBFC0_0004, encode_i(OP_ADDIU, 0, 2, 3));
        // MULTU r1, r2
        write_instr(
            &mut cpu.memory,
            0xBFC0_0008,
            encode_r(FUNCT_MULTU, 1, 2, 0, 0),
        );
        // MFLO r3
        write_instr(
            &mut cpu.memory,
            0xBFC0_000C,
            encode_r(FUNCT_MFLO, 0, 0, 3, 0),
        );
        write_instr(&mut cpu.memory, 0xBFC0_0010, 0);
        for _ in 0..5 {
            cpu.step();
        }
        assert_eq!(cpu.gpr[3], 21); // 7 * 3 = 21
    }

    #[test]
    fn test_sw_lw() {
        let mut cpu = make_cpu();
        // ADDIU r1, r0, 0x1234  (value to store)
        write_instr(
            &mut cpu.memory,
            0xBFC0_0000,
            encode_i(OP_ADDIU, 0, 1, 0x1234),
        );
        // LUI r2, 0x8000 (base address)
        write_instr(&mut cpu.memory, 0xBFC0_0004, encode_i(OP_LUI, 0, 2, 0x8000));
        // SW r1, 0(r2) (store to 0x80000000)
        write_instr(&mut cpu.memory, 0xBFC0_0008, encode_i(OP_SW, 2, 1, 0));
        // LW r3, 0(r2) (load back)
        write_instr(&mut cpu.memory, 0xBFC0_000C, encode_i(OP_LW, 2, 3, 0));
        // NOP (let load delay resolve)
        write_instr(&mut cpu.memory, 0xBFC0_0010, 0);
        for _ in 0..5 {
            cpu.step();
        }
        assert_eq!(cpu.gpr[3], 0x1234);
    }

    #[test]
    fn test_j_jump() {
        let mut cpu = make_cpu();
        // J 0x1FC00010 (maps to 0xBFC00010 in upper segment)
        // target field = 0x1FC00010 >> 2 = 0x07F00004
        write_instr(
            &mut cpu.memory,
            0xBFC0_0000,
            encode_j(OP_J, 0xBFC0_0010 >> 2),
        );
        // NOP (delay slot)
        write_instr(&mut cpu.memory, 0xBFC0_0004, 0);
        // ADDIU r1, r0, 0x42 (at target)
        write_instr(&mut cpu.memory, 0xBFC0_0010, encode_i(OP_ADDIU, 0, 1, 0x42));
        write_instr(&mut cpu.memory, 0xBFC0_0014, 0);
        cpu.step(); // J
        cpu.step(); // delay slot NOP
        cpu.step(); // ADDIU at target
        cpu.step(); // resolve
        assert_eq!(cpu.gpr[1], 0x42);
    }

    #[test]
    fn test_beq_taken() {
        let mut cpu = make_cpu();
        // BEQ r0, r0, +2 (always taken, skip 2 instructions)
        write_instr(&mut cpu.memory, 0xBFC0_0000, encode_i(OP_BEQ, 0, 0, 2));
        // NOP (delay slot, executed)
        write_instr(&mut cpu.memory, 0xBFC0_0004, 0);
        // ADDIU r1, r0, 0xBB (skipped)
        write_instr(&mut cpu.memory, 0xBFC0_0008, encode_i(OP_ADDIU, 0, 1, 0xBB));
        // ADDIU r2, r0, 0xCC (branch target)
        write_instr(&mut cpu.memory, 0xBFC0_000C, encode_i(OP_ADDIU, 0, 2, 0xCC));
        write_instr(&mut cpu.memory, 0xBFC0_0010, 0);
        cpu.step(); // BEQ
        cpu.step(); // delay slot
        cpu.step(); // at target: ADDIU r2
        cpu.step(); // resolve
        assert_eq!(cpu.gpr[1], 0); // Skipped
        assert_eq!(cpu.gpr[2], 0xCC); // Target executed
    }

    #[test]
    fn test_jal_link() {
        let mut cpu = make_cpu();
        // JAL target
        write_instr(
            &mut cpu.memory,
            0xBFC0_0000,
            encode_j(OP_JAL, 0xBFC0_0020 >> 2),
        );
        // NOP (delay slot)
        write_instr(&mut cpu.memory, 0xBFC0_0004, 0);
        cpu.step(); // JAL
        cpu.step(); // delay slot
                    // RA should be address after delay slot = 0xBFC00008
        assert_eq!(cpu.gpr[31], 0xBFC0_0008);
        assert_eq!(cpu.pc, 0xBFC0_0020);
    }

    #[test]
    fn test_syscall_exception() {
        let mut cpu = make_cpu();
        write_instr(
            &mut cpu.memory,
            0xBFC0_0000,
            encode_r(FUNCT_SYSCALL, 0, 0, 0, 0),
        );
        cpu.step();
        // Should jump to boot exception vector (BEV=0 by default => 0x80000080)
        // Wait, default SR has BEV=0 since cop0 is zeroed
        assert_eq!(cpu.pc, 0x8000_0080);
        // EPC should point to the syscall instruction
        assert_eq!(cpu.cop0[COP0_EPC], 0xBFC0_0000);
        // Cause exception code should be SYSCALL (8)
        assert_eq!((cpu.cop0[COP0_CAUSE] & CAUSE_EXCODE_MASK) >> 2, EXCODE_SYS);
    }
}
