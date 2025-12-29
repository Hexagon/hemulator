//! Intel 8086 CPU core implementation
//!
//! This module provides a reusable, generic 8086 CPU implementation that can be used
//! by any system (IBM PC, PC XT, etc.) by implementing the `Memory8086` trait.
//!
//! Supports multiple CPU models: 8086, 80186, 80286, and their variants.
//!
//! For detailed CPU reference documentation, see: `docs/references/cpu_8086.md`

use crate::cpu_8086_protected::ProtectedModeState;
use crate::logging::{LogCategory, LogConfig, LogLevel};

/// CPU model/variant selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum CpuModel {
    /// Intel 8086 (1978) - Original 16-bit x86 processor
    #[default]
    Intel8086,
    /// Intel 8088 (1979) - 8-bit external bus variant of 8086
    Intel8088,
    /// Intel 80186 (1982) - Enhanced 8086 with additional instructions
    Intel80186,
    /// Intel 80188 (1982) - 8-bit external bus variant of 80186
    Intel80188,
    /// Intel 80286 (1982) - Protected mode support, 24-bit addressing
    Intel80286,
    /// Intel 80386 (1985) - 32-bit processor with 32-bit registers and addressing
    Intel80386,
    /// Intel 80486 (1989) - Integrated FPU, 8KB cache, pipelining
    Intel80486,
    /// Intel 80486 SX (1991) - 486 without FPU
    Intel80486SX,
    /// Intel 80486 DX2 (1992) - 486 with 2x clock multiplier
    Intel80486DX2,
    /// Intel 80486 SX2 (1992) - 486 SX with 2x clock multiplier
    Intel80486SX2,
    /// Intel 80486 DX4 (1994) - 486 with 3x clock multiplier (despite the name)
    Intel80486DX4,
    /// Intel Pentium (1993) - Superscalar architecture, dual integer pipelines
    IntelPentium,
    /// Intel Pentium MMX (1997) - Pentium with MMX SIMD extensions
    IntelPentiumMMX,
}

impl CpuModel {
    /// Returns true if this CPU model supports 80186+ instructions
    pub fn supports_80186_instructions(&self) -> bool {
        matches!(
            self,
            CpuModel::Intel80186
                | CpuModel::Intel80188
                | CpuModel::Intel80286
                | CpuModel::Intel80386
                | CpuModel::Intel80486
                | CpuModel::Intel80486SX
                | CpuModel::Intel80486DX2
                | CpuModel::Intel80486SX2
                | CpuModel::Intel80486DX4
                | CpuModel::IntelPentium
                | CpuModel::IntelPentiumMMX
        )
    }

    /// Returns true if this CPU model supports 80286+ instructions
    pub fn supports_80286_instructions(&self) -> bool {
        matches!(
            self,
            CpuModel::Intel80286
                | CpuModel::Intel80386
                | CpuModel::Intel80486
                | CpuModel::Intel80486SX
                | CpuModel::Intel80486DX2
                | CpuModel::Intel80486SX2
                | CpuModel::Intel80486DX4
                | CpuModel::IntelPentium
                | CpuModel::IntelPentiumMMX
        )
    }

    /// Returns true if this CPU model supports 80386+ instructions
    pub fn supports_80386_instructions(&self) -> bool {
        matches!(
            self,
            CpuModel::Intel80386
                | CpuModel::Intel80486
                | CpuModel::Intel80486SX
                | CpuModel::Intel80486DX2
                | CpuModel::Intel80486SX2
                | CpuModel::Intel80486DX4
                | CpuModel::IntelPentium
                | CpuModel::IntelPentiumMMX
        )
    }

    /// Returns true if this CPU model supports 80486+ instructions
    pub fn supports_80486_instructions(&self) -> bool {
        matches!(
            self,
            CpuModel::Intel80486
                | CpuModel::Intel80486SX
                | CpuModel::Intel80486DX2
                | CpuModel::Intel80486SX2
                | CpuModel::Intel80486DX4
                | CpuModel::IntelPentium
                | CpuModel::IntelPentiumMMX
        )
    }

    /// Returns true if this CPU model supports Pentium+ instructions
    pub fn supports_pentium_instructions(&self) -> bool {
        matches!(self, CpuModel::IntelPentium | CpuModel::IntelPentiumMMX)
    }

    /// Returns true if this CPU model supports MMX instructions
    pub fn supports_mmx_instructions(&self) -> bool {
        matches!(self, CpuModel::IntelPentiumMMX)
    }

    /// Returns the name of the CPU model as a string
    pub fn name(&self) -> &'static str {
        match self {
            CpuModel::Intel8086 => "Intel 8086",
            CpuModel::Intel8088 => "Intel 8088",
            CpuModel::Intel80186 => "Intel 80186",
            CpuModel::Intel80188 => "Intel 80188",
            CpuModel::Intel80286 => "Intel 80286",
            CpuModel::Intel80386 => "Intel 80386",
            CpuModel::Intel80486 => "Intel 80486",
            CpuModel::Intel80486SX => "Intel 80486 SX",
            CpuModel::Intel80486DX2 => "Intel 80486 DX2",
            CpuModel::Intel80486SX2 => "Intel 80486 SX2",
            CpuModel::Intel80486DX4 => "Intel 80486 DX4",
            CpuModel::IntelPentium => "Intel Pentium",
            CpuModel::IntelPentiumMMX => "Intel Pentium MMX",
        }
    }
}

/// Memory interface trait for the 8086 CPU
///
/// Systems using the 8086 must implement this trait to provide memory access.
pub trait Memory8086 {
    /// Read a byte from memory at the given address
    fn read(&self, addr: u32) -> u8;

    /// Write a byte to memory at the given address
    fn write(&mut self, addr: u32, val: u8);
}

/// Segment override specification for next instruction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SegmentOverride {
    /// Use ES segment
    ES,
    /// Use CS segment
    CS,
    /// Use SS segment
    SS,
    /// Use DS segment
    DS,
    /// Use FS segment (80386+)
    FS,
    /// Use GS segment (80386+)
    GS,
}

/// Intel 8086 CPU state and execution engine
///
/// This is a generic, reusable 8086 CPU implementation that works with any
/// system through the `Memory8086` trait.
#[derive(Debug)]
pub struct Cpu8086<M: Memory8086> {
    // General purpose registers (32-bit, 80386+)
    // For 8086/80186/80286: only low 16 bits are used, high 16 bits remain zero
    /// EAX register (accumulator) - low 16 bits: AX (AH:AL)
    pub ax: u32,
    /// EBX register (base) - low 16 bits: BX (BH:BL)
    pub bx: u32,
    /// ECX register (count) - low 16 bits: CX (CH:CL)
    pub cx: u32,
    /// EDX register (data) - low 16 bits: DX (DH:DL)
    pub dx: u32,

    // Index and pointer registers (32-bit, 80386+)
    /// ESI register (source index) - low 16 bits: SI
    pub si: u32,
    /// EDI register (destination index) - low 16 bits: DI
    pub di: u32,
    /// EBP register (base pointer) - low 16 bits: BP
    pub bp: u32,
    /// ESP register (stack pointer) - low 16 bits: SP
    pub sp: u32,

    // Segment registers
    /// CS register (code segment)
    pub cs: u16,
    /// DS register (data segment)
    pub ds: u16,
    /// ES register (extra segment)
    pub es: u16,
    /// SS register (stack segment)
    pub ss: u16,
    /// FS register (extra segment, 80386+ only)
    pub fs: u16,
    /// GS register (extra segment, 80386+ only)
    pub gs: u16,

    // Control registers (32-bit, 80386+)
    /// EIP register (instruction pointer) - low 16 bits: IP
    pub ip: u32,
    /// EFLAGS register (status flags) - low 16 bits: FLAGS
    pub flags: u32,

    /// Total cycles executed
    pub cycles: u64,

    /// Memory interface
    pub memory: M,

    /// Halt flag
    halted: bool,

    /// Segment override for next instruction
    /// Set by segment override prefixes (0x26 ES:, 0x2E CS:, 0x36 SS:, 0x3E DS:, 0x64 FS:, 0x65 GS:)
    /// Consumed and cleared after the next memory-accessing instruction
    segment_override: Option<SegmentOverride>,

    /// Operand-size override for next instruction (0x66 prefix, 80386+)
    /// When set, 16-bit operations become 32-bit and vice versa
    /// Consumed and cleared after the next instruction
    operand_size_override: bool,

    /// Address-size override for next instruction (0x67 prefix, 80386+)
    /// When set, 16-bit addressing becomes 32-bit and vice versa
    /// Consumed and cleared after the next instruction
    address_size_override: bool,

    /// CPU model (8086, 80186, 80286, etc.)
    model: CpuModel,

    /// Protected mode state (80286+ only)
    /// This is only used when model is Intel80286 or later
    protected_mode: ProtectedModeState,

    /// Time Stamp Counter (Pentium+ only)
    /// Increments with each cycle, used by RDTSC instruction
    tsc: u64,

    /// Model-Specific Registers (Pentium+ only)
    /// Simplified implementation: only stores a few common MSRs
    /// Real Pentium has hundreds of MSRs, we store only what's needed
    msrs: std::collections::HashMap<u32, u64>,

    /// MMX registers (Pentium MMX only)
    /// 8 MMX registers (MM0-MM7), each 64 bits
    /// These alias the FPU ST(0)-ST(7) registers in real hardware
    mmx_regs: [u64; 8],

    /// Instruction start IP - saved at the beginning of each instruction
    /// Used for CPU exceptions to point to the faulting instruction
    instruction_start_ip: u32,
}

// Flag bit positions in FLAGS/EFLAGS register
const FLAG_CF: u32 = 0x0001; // Carry Flag
const FLAG_PF: u32 = 0x0004; // Parity Flag
#[allow(dead_code)]
const FLAG_AF: u32 = 0x0010; // Auxiliary Carry Flag
                             // Note: AF is now calculated by all arithmetic operations (ADD, SUB, ADC, SBB,
                             // INC, DEC, NEG, CMP) as per CPU_REVIEW_RESULTS.md recommendations.
                             // BCD adjust instructions (DAA, DAS, AAA, AAS) also maintain AF correctly.
const FLAG_ZF: u32 = 0x0040; // Zero Flag
const FLAG_SF: u32 = 0x0080; // Sign Flag
#[allow(dead_code)]
const FLAG_TF: u32 = 0x0100; // Trap Flag
const FLAG_IF: u32 = 0x0200; // Interrupt Enable Flag
const FLAG_DF: u32 = 0x0400; // Direction Flag
const FLAG_OF: u32 = 0x0800; // Overflow Flag

impl<M: Memory8086> Cpu8086<M> {
    /// Create a new 8086 CPU with the given memory interface
    pub fn new(memory: M) -> Self {
        Self::with_model(memory, CpuModel::Intel8086)
    }

    /// Create a new CPU with a specific model
    pub fn with_model(memory: M, model: CpuModel) -> Self {
        Self {
            ax: 0,
            bx: 0,
            cx: 0,
            dx: 0,
            si: 0,
            di: 0,
            bp: 0,
            sp: 0,
            cs: 0xFFFF,
            ds: 0,
            es: 0,
            ss: 0,
            fs: 0,
            gs: 0,
            ip: 0,
            flags: 0x0002, // Reserved bit 1 is always set
            cycles: 0,
            memory,
            halted: false,
            segment_override: None,
            operand_size_override: false,
            address_size_override: false,
            model,
            protected_mode: ProtectedModeState::new(),
            tsc: 0,
            msrs: std::collections::HashMap::new(),
            mmx_regs: [0; 8],
            instruction_start_ip: 0,
        }
    }

    /// Get the CPU model
    pub fn model(&self) -> CpuModel {
        self.model
    }

    /// Set the CPU model
    pub fn set_model(&mut self, model: CpuModel) {
        self.model = model;
    }

    /// Reset the CPU to initial state (preserves memory and model)
    pub fn reset(&mut self) {
        self.ax = 0;
        self.bx = 0;
        self.cx = 0;
        self.dx = 0;
        self.si = 0;
        self.di = 0;
        self.bp = 0;
        self.sp = 0;
        self.cs = 0xFFFF;
        self.ds = 0;
        self.es = 0;
        self.ss = 0;
        self.fs = 0;
        self.gs = 0;
        self.ip = 0;
        self.flags = 0x0002;
        self.cycles = 0;
        self.halted = false;
        // Note: model is preserved across reset
        // Reset protected mode state
        self.protected_mode.reset();
        // Reset TSC and MSRs
        self.tsc = 0;
        self.msrs.clear();
        // Reset MMX registers
        self.mmx_regs = [0; 8];
    }

    /// Get reference to protected mode state (80286+ only)
    pub fn protected_mode(&self) -> &ProtectedModeState {
        &self.protected_mode
    }

    /// Get mutable reference to protected mode state (80286+ only)
    pub fn protected_mode_mut(&mut self) -> &mut ProtectedModeState {
        &mut self.protected_mode
    }

    /// Check if the CPU is halted
    pub fn is_halted(&self) -> bool {
        self.halted
    }

    /// Set the CPU halted state
    /// When halted, the CPU will not execute instructions until an interrupt occurs or it is unhalted
    pub fn set_halted(&mut self, halted: bool) {
        self.halted = halted;
    }

    /// Get the segment value for a segment override, or default segment if no override
    /// This consumes and clears the segment override
    #[inline]
    fn get_segment_with_override(&mut self, default: u16) -> u16 {
        match self.segment_override.take() {
            Some(SegmentOverride::ES) => self.es,
            Some(SegmentOverride::CS) => self.cs,
            Some(SegmentOverride::SS) => self.ss,
            Some(SegmentOverride::DS) => self.ds,
            Some(SegmentOverride::FS) => self.fs,
            Some(SegmentOverride::GS) => self.gs,
            None => default,
        }
    }

    /// Calculate physical address from segment:offset
    #[inline]
    fn physical_address(segment: u16, offset: u16) -> u32 {
        ((segment as u32) << 4) + (offset as u32)
    }

    /// Read a byte from memory using segment:offset
    #[inline]
    fn read(&self, segment: u16, offset: u16) -> u8 {
        let addr = Self::physical_address(segment, offset);
        self.memory.read(addr)
    }

    /// Write a byte to memory using segment:offset
    #[inline]
    fn write(&mut self, segment: u16, offset: u16, val: u8) {
        let addr = Self::physical_address(segment, offset);
        self.memory.write(addr, val);
    }

    /// Read a byte from code segment at IP
    #[inline]
    fn fetch_u8(&mut self) -> u8 {
        let val = self.read(self.cs, self.ip as u16);
        self.ip = (self.ip.wrapping_add(1)) & 0xFFFF; // Keep in 16-bit range for now
        val
    }

    /// Read a word (16-bit) from code segment at IP
    #[inline]
    fn fetch_u16(&mut self) -> u16 {
        // 8086 is little-endian: fetch low byte first, then high byte
        let low_byte = self.fetch_u8() as u16;
        let high_byte = self.fetch_u8() as u16;
        (high_byte << 8) | low_byte
    }

    /// Read a dword (32-bit) from code segment at IP
    #[inline]
    fn fetch_u32(&mut self) -> u32 {
        // x86 is little-endian: fetch low word first, then high word
        let low_word = self.fetch_u16() as u32;
        let high_word = self.fetch_u16() as u32;
        (high_word << 16) | low_word
    }

    /// Read a word from memory at segment:offset
    #[inline]
    fn read_u16(&self, segment: u16, offset: u16) -> u16 {
        // 8086 is little-endian: read low byte at offset, then high byte at offset + 1
        let low_byte = self.read(segment, offset) as u16;
        let high_byte = self.read(segment, offset.wrapping_add(1)) as u16;
        (high_byte << 8) | low_byte
    }

    /// Write a word to memory at segment:offset
    #[inline]
    fn write_u16(&mut self, segment: u16, offset: u16, val: u16) {
        let lo = (val & 0xFF) as u8;
        let hi = ((val >> 8) & 0xFF) as u8;
        self.write(segment, offset, lo);
        self.write(segment, offset.wrapping_add(1), hi);
    }

    /// Read a dword (32-bit) from memory at segment:offset
    #[inline]
    fn read_u32(&self, segment: u16, offset: u32) -> u32 {
        // x86 is little-endian: read low word first, then high word
        let low_word = self.read_u16(segment, offset as u16) as u32;
        let high_word = self.read_u16(segment, (offset as u16).wrapping_add(2)) as u32;
        (high_word << 16) | low_word
    }

    /// Write a dword (32-bit) to memory at segment:offset
    #[inline]
    fn write_u32(&mut self, segment: u16, offset: u32, val: u32) {
        let low_word = (val & 0xFFFF) as u16;
        let high_word = ((val >> 16) & 0xFFFF) as u16;
        self.write_u16(segment, offset as u16, low_word);
        self.write_u16(segment, (offset as u16).wrapping_add(2), high_word);
    }

    /// Push a word onto the stack
    #[inline]
    fn push(&mut self, val: u16) {
        self.sp = (self.sp.wrapping_sub(2u32)) & 0xFFFF; // Keep in 16-bit range for now
        self.write_u16(self.ss, self.sp as u16, val);
    }

    /// Pop a word from the stack
    #[inline]
    fn pop(&mut self) -> u16 {
        let val = self.read_u16(self.ss, self.sp as u16);
        self.sp = (self.sp.wrapping_add(2u32)) & 0xFFFF; // Keep in 16-bit range for now
        val
    }

    /// Trigger a software interrupt (INT) or CPU exception
    ///
    /// For software interrupts (is_exception=false): Uses current IP (after INT instruction)
    /// For CPU exceptions (is_exception=true): Uses instruction_start_ip (faulting instruction)
    #[inline]
    fn trigger_interrupt(&mut self, int_num: u8, is_exception: bool) {
        // Push FLAGS, CS, IP onto stack (in that order)
        self.push(self.flags as u16); // Only push low 16 bits of flags for now
        self.push(self.cs);

        // For exceptions, save IP of faulting instruction (to allow restart after fixing issue)
        // For software interrupts, save current IP (already advanced past INT instruction)
        let saved_ip = if is_exception {
            self.instruction_start_ip as u16
        } else {
            self.ip as u16
        };
        self.push(saved_ip);

        // Clear IF and TF flags
        self.set_flag(FLAG_IF, false);
        self.set_flag(FLAG_TF, false);

        // Read interrupt vector from IVT (Interrupt Vector Table) at 0x0000:int_num*4
        // Each IVT entry is 4 bytes: offset (2 bytes) + segment (2 bytes)
        let ivt_offset = (int_num as u16) * 4;
        let new_ip = self.read_u16(0, ivt_offset);
        let new_cs = self.read_u16(0, ivt_offset + 2);

        // Jump to interrupt handler
        self.ip = new_ip as u32;
        self.cs = new_cs;
    }

    /// Read a byte from I/O port (stub implementation - returns 0xFF)
    #[inline]
    fn io_read(&self, _port: u16) -> u8 {
        // For basic emulation, I/O reads return 0xFF
        // Systems can override this by wrapping the CPU
        0xFF
    }

    /// Write a byte to I/O port (stub implementation - does nothing)
    #[inline]
    fn io_write(&mut self, _port: u16, _val: u8) {
        // For basic emulation, I/O writes are no-ops
        // Systems can override this by wrapping the CPU
    }

    /// Read a word from I/O port (stub implementation - returns 0xFFFF)
    #[inline]
    fn io_read_word(&self, _port: u16) -> u16 {
        // For basic emulation, I/O reads return 0xFFFF
        // Systems can override this by wrapping the CPU
        0xFFFF
    }

    /// Write a word to I/O port (stub implementation - does nothing)
    #[inline]
    fn io_write_word(&mut self, _port: u16, _val: u16) {
        // For basic emulation, I/O writes are no-ops
        // Systems can override this by wrapping the CPU
    }

    /// Get 8-bit high register
    #[inline]
    #[allow(dead_code)]
    fn get_reg8_high(&self, reg: u8) -> u8 {
        debug_assert!(
            reg < 4,
            "Invalid 8-bit high register index: {} (must be 0-3)",
            reg
        );
        match reg {
            0 => ((self.ax >> 8) & 0xFF) as u8, // AH
            1 => ((self.cx >> 8) & 0xFF) as u8, // CH
            2 => ((self.dx >> 8) & 0xFF) as u8, // DH
            3 => ((self.bx >> 8) & 0xFF) as u8, // BH
            _ => unreachable!(),
        }
    }

    /// Get 8-bit low register
    #[inline]
    #[allow(dead_code)]
    fn get_reg8_low(&self, reg: u8) -> u8 {
        debug_assert!(
            reg < 4,
            "Invalid 8-bit low register index: {} (must be 0-3)",
            reg
        );
        match reg {
            0 => (self.ax & 0xFF) as u8, // AL
            1 => (self.cx & 0xFF) as u8, // CL
            2 => (self.dx & 0xFF) as u8, // DL
            3 => (self.bx & 0xFF) as u8, // BL
            _ => unreachable!(),
        }
    }

    /// Set 8-bit high register
    #[inline]
    fn set_reg8_high(&mut self, reg: u8, val: u8) {
        debug_assert!(
            reg < 4,
            "Invalid 8-bit high register index: {} (must be 0-3)",
            reg
        );
        match reg {
            0 => self.ax = (self.ax & 0xFFFF_00FF) | ((val as u32) << 8), // AH
            1 => self.cx = (self.cx & 0xFFFF_00FF) | ((val as u32) << 8), // CH
            2 => self.dx = (self.dx & 0xFFFF_00FF) | ((val as u32) << 8), // DH
            3 => self.bx = (self.bx & 0xFFFF_00FF) | ((val as u32) << 8), // BH
            _ => unreachable!(),
        }
    }

    /// Set 8-bit low register
    #[inline]
    fn set_reg8_low(&mut self, reg: u8, val: u8) {
        debug_assert!(
            reg < 4,
            "Invalid 8-bit low register index: {} (must be 0-3)",
            reg
        );
        match reg {
            0 => self.ax = (self.ax & 0xFFFF_FF00) | (val as u32), // AL
            1 => self.cx = (self.cx & 0xFFFF_FF00) | (val as u32), // CL
            2 => self.dx = (self.dx & 0xFFFF_FF00) | (val as u32), // DL
            3 => self.bx = (self.bx & 0xFFFF_FF00) | (val as u32), // BL
            _ => unreachable!(),
        }
    }

    /// Get 16-bit register (low 16 bits of 32-bit register)
    #[inline]
    fn get_reg16(&self, reg: u8) -> u16 {
        debug_assert!(
            reg < 8,
            "Invalid 16-bit register index: {} (must be 0-7)",
            reg
        );
        match reg {
            0 => (self.ax & 0xFFFF) as u16,
            1 => (self.cx & 0xFFFF) as u16,
            2 => (self.dx & 0xFFFF) as u16,
            3 => (self.bx & 0xFFFF) as u16,
            4 => (self.sp & 0xFFFF) as u16,
            5 => (self.bp & 0xFFFF) as u16,
            6 => (self.si & 0xFFFF) as u16,
            7 => (self.di & 0xFFFF) as u16,
            _ => unreachable!(),
        }
    }

    /// Set 16-bit register (updates low 16 bits, preserves high 16 bits)
    #[inline]
    fn set_reg16(&mut self, reg: u8, val: u16) {
        debug_assert!(
            reg < 8,
            "Invalid 16-bit register index: {} (must be 0-7)",
            reg
        );
        match reg {
            0 => self.ax = (self.ax & 0xFFFF_0000) | (val as u32),
            1 => self.cx = (self.cx & 0xFFFF_0000) | (val as u32),
            2 => self.dx = (self.dx & 0xFFFF_0000) | (val as u32),
            3 => self.bx = (self.bx & 0xFFFF_0000) | (val as u32),
            4 => self.sp = (self.sp & 0xFFFF_0000) | (val as u32),
            5 => self.bp = (self.bp & 0xFFFF_0000) | (val as u32),
            6 => self.si = (self.si & 0xFFFF_0000) | (val as u32),
            7 => self.di = (self.di & 0xFFFF_0000) | (val as u32),
            _ => unreachable!(),
        }
    }

    /// Get 32-bit register (full 32-bit value, 80386+ only)
    #[inline]
    fn get_reg32(&self, reg: u8) -> u32 {
        debug_assert!(
            reg < 8,
            "Invalid 32-bit register index: {} (must be 0-7)",
            reg
        );
        match reg {
            0 => self.ax,
            1 => self.cx,
            2 => self.dx,
            3 => self.bx,
            4 => self.sp,
            5 => self.bp,
            6 => self.si,
            7 => self.di,
            _ => unreachable!(),
        }
    }

    /// Set 32-bit register (full 32-bit value, 80386+ only)
    #[inline]
    fn set_reg32(&mut self, reg: u8, val: u32) {
        debug_assert!(
            reg < 8,
            "Invalid 32-bit register index: {} (must be 0-7)",
            reg
        );
        match reg {
            0 => self.ax = val,
            1 => self.cx = val,
            2 => self.dx = val,
            3 => self.bx = val,
            4 => self.sp = val,
            5 => self.bp = val,
            6 => self.si = val,
            7 => self.di = val,
            _ => unreachable!(),
        }
    }

    /// Get segment register
    #[inline]
    #[allow(dead_code)]
    fn get_seg(&self, seg: u8) -> u16 {
        match seg {
            0 => self.es,
            1 => self.cs,
            2 => self.ss,
            3 => self.ds,
            _ => panic!("Invalid segment register index: {} (must be 0-3)", seg),
        }
    }

    /// Set segment register
    #[inline]
    #[allow(dead_code)]
    fn set_seg(&mut self, seg: u8, val: u16) {
        match seg {
            0 => self.es = val,
            1 => self.cs = val,
            2 => self.ss = val,
            3 => self.ds = val,
            _ => panic!("Invalid segment register index: {} (must be 0-3)", seg),
        }
    }

    /// Set flag
    #[inline]
    fn set_flag(&mut self, flag: u32, value: bool) {
        if value {
            self.flags |= flag;
        } else {
            self.flags &= !flag;
        }
    }

    /// Get flag
    #[inline]
    fn get_flag(&self, flag: u32) -> bool {
        (self.flags & flag) != 0
    }

    /// Check condition code for conditional instructions
    /// Condition codes: 0=O, 1=NO, 2=B/C, 3=NB/NC, 4=E/Z, 5=NE/NZ, 6=BE, 7=NBE,
    ///                  8=S, 9=NS, A=P, B=NP, C=L, D=NL, E=LE, F=NLE
    #[inline]
    fn check_condition(&self, condition: u8) -> bool {
        match condition {
            0x0 => self.get_flag(FLAG_OF),  // O - Overflow
            0x1 => !self.get_flag(FLAG_OF), // NO - Not Overflow
            0x2 => self.get_flag(FLAG_CF),  // B/C - Below/Carry
            0x3 => !self.get_flag(FLAG_CF), // NB/NC - Not Below/Not Carry
            0x4 => self.get_flag(FLAG_ZF),  // E/Z - Equal/Zero
            0x5 => !self.get_flag(FLAG_ZF), // NE/NZ - Not Equal/Not Zero
            0x6 => self.get_flag(FLAG_CF) || self.get_flag(FLAG_ZF), // BE - Below or Equal
            0x7 => !self.get_flag(FLAG_CF) && !self.get_flag(FLAG_ZF), // NBE - Not Below or Equal
            0x8 => self.get_flag(FLAG_SF),  // S - Sign
            0x9 => !self.get_flag(FLAG_SF), // NS - Not Sign
            0xA => self.get_flag(FLAG_PF),  // P - Parity
            0xB => !self.get_flag(FLAG_PF), // NP - Not Parity
            0xC => self.get_flag(FLAG_SF) != self.get_flag(FLAG_OF), // L - Less
            0xD => self.get_flag(FLAG_SF) == self.get_flag(FLAG_OF), // NL - Not Less
            0xE => self.get_flag(FLAG_ZF) || (self.get_flag(FLAG_SF) != self.get_flag(FLAG_OF)), // LE - Less or Equal
            0xF => !self.get_flag(FLAG_ZF) && (self.get_flag(FLAG_SF) == self.get_flag(FLAG_OF)), // NLE - Not Less or Equal
            _ => false,
        }
    }

    /// Calculate parity (true if even number of 1 bits in low byte)
    #[inline]
    fn calc_parity(val: u8) -> bool {
        val.count_ones().is_multiple_of(2)
    }

    /// Calculate Auxiliary Flag for 8-bit addition
    /// AF is set when there's a carry from bit 3 to bit 4
    #[inline]
    fn calc_af_add_8(a: u8, b: u8) -> bool {
        (((a & 0x0F) + (b & 0x0F)) & 0x10) != 0
    }

    /// Calculate Auxiliary Flag for 8-bit subtraction
    /// AF is set when there's a borrow from bit 4 to bit 3
    #[inline]
    fn calc_af_sub_8(a: u8, b: u8) -> bool {
        (a & 0x0F) < (b & 0x0F)
    }

    /// Calculate Auxiliary Flag for 16-bit addition
    /// AF is set when there's a carry from bit 3 to bit 4 (in the low byte)
    #[inline]
    fn calc_af_add_16(a: u16, b: u16) -> bool {
        (((a & 0x0F) + (b & 0x0F)) & 0x10) != 0
    }

    /// Calculate Auxiliary Flag for 16-bit subtraction
    /// AF is set when there's a borrow from bit 4 to bit 3 (in the low byte)
    #[inline]
    fn calc_af_sub_16(a: u16, b: u16) -> bool {
        (a & 0x0F) < (b & 0x0F)
    }

    /// Calculate Auxiliary Flag for 32-bit addition
    /// AF is set when there's a carry from bit 3 to bit 4 (in the low byte)
    #[inline]
    fn calc_af_add_32(a: u32, b: u32) -> bool {
        (((a & 0x0F) + (b & 0x0F)) & 0x10) != 0
    }

    /// Calculate Auxiliary Flag for 32-bit subtraction
    /// AF is set when there's a borrow from bit 4 to bit 3 (in the low byte)
    #[inline]
    fn calc_af_sub_32(a: u32, b: u32) -> bool {
        (a & 0x0F) < (b & 0x0F)
    }

    /// Update flags after 8-bit arithmetic/logic operation
    fn update_flags_8(&mut self, result: u8) {
        self.set_flag(FLAG_ZF, result == 0);
        self.set_flag(FLAG_SF, (result & 0x80) != 0);
        self.set_flag(FLAG_PF, Self::calc_parity(result));
    }

    /// Update flags after 16-bit arithmetic/logic operation
    fn update_flags_16(&mut self, result: u16) {
        self.set_flag(FLAG_ZF, result == 0);
        self.set_flag(FLAG_SF, (result & 0x8000) != 0);
        self.set_flag(FLAG_PF, Self::calc_parity((result & 0xFF) as u8));
    }

    /// Update flags after 32-bit arithmetic/logic operation
    fn update_flags_32(&mut self, result: u32) {
        self.set_flag(FLAG_ZF, result == 0);
        self.set_flag(FLAG_SF, (result & 0x80000000) != 0);
        self.set_flag(FLAG_PF, Self::calc_parity((result & 0xFF) as u8));
    }

    /// Perform 8-bit shift/rotate operation
    fn shift_rotate_8(&mut self, val: u8, op: u8, count: u8) -> u8 {
        if count == 0 {
            return val;
        }

        // On 80186+, shift count is masked to 5 bits (0-31)
        // On 8086/8088, full 8-bit count is used (can shift by 0-255)
        let count = if self.model.supports_80186_instructions() {
            count & 0x1F
        } else {
            count
        };
        let mut result = val;

        match op {
            // ROL - Rotate left
            0b000 => {
                for _ in 0..count {
                    let carry_out = (result & 0x80) != 0;
                    result = (result << 1) | (if carry_out { 1 } else { 0 });
                    self.set_flag(FLAG_CF, carry_out);
                }
                // OF is set if sign bit changed (only for count=1)
                if count == 1 {
                    let msb = (result & 0x80) != 0;
                    self.set_flag(FLAG_OF, msb != self.get_flag(FLAG_CF));
                }
            }
            // ROR - Rotate right
            0b001 => {
                for _ in 0..count {
                    let carry_out = (result & 0x01) != 0;
                    result = (result >> 1) | (if carry_out { 0x80 } else { 0 });
                    self.set_flag(FLAG_CF, carry_out);
                }
                // OF is set if two high bits differ (only for count=1)
                if count == 1 {
                    let bit7 = (result & 0x80) != 0;
                    let bit6 = (result & 0x40) != 0;
                    self.set_flag(FLAG_OF, bit7 != bit6);
                }
            }
            // RCL - Rotate through carry left
            0b010 => {
                for _ in 0..count {
                    let old_cf = if self.get_flag(FLAG_CF) { 1 } else { 0 };
                    let carry_out = (result & 0x80) != 0;
                    result = (result << 1) | old_cf;
                    self.set_flag(FLAG_CF, carry_out);
                }
                // OF is set if sign bit changed (only for count=1)
                if count == 1 {
                    let msb = (result & 0x80) != 0;
                    self.set_flag(FLAG_OF, msb != self.get_flag(FLAG_CF));
                }
            }
            // RCR - Rotate through carry right
            0b011 => {
                for _ in 0..count {
                    let old_cf = if self.get_flag(FLAG_CF) { 0x80 } else { 0 };
                    let carry_out = (result & 0x01) != 0;
                    result = (result >> 1) | old_cf;
                    self.set_flag(FLAG_CF, carry_out);
                }
                // OF is set if two high bits differ (only for count=1)
                if count == 1 {
                    let bit7 = (result & 0x80) != 0;
                    let bit6 = (result & 0x40) != 0;
                    self.set_flag(FLAG_OF, bit7 != bit6);
                }
            }
            // SHL/SAL - Shift left
            0b100 | 0b110 => {
                for _ in 0..count {
                    let carry_out = (result & 0x80) != 0;
                    result <<= 1;
                    self.set_flag(FLAG_CF, carry_out);
                }
                self.update_flags_8(result);
                // OF is set if sign bit changed (only for count=1)
                if count == 1 {
                    let msb = (result & 0x80) != 0;
                    self.set_flag(FLAG_OF, msb != self.get_flag(FLAG_CF));
                }
            }
            // SHR - Shift right
            0b101 => {
                // OF is set to MSB of original value (only for count=1)
                if count == 1 {
                    self.set_flag(FLAG_OF, (val & 0x80) != 0);
                }
                for _ in 0..count {
                    let carry_out = (result & 0x01) != 0;
                    result >>= 1;
                    self.set_flag(FLAG_CF, carry_out);
                }
                self.update_flags_8(result);
            }
            // SAR - Shift arithmetic right
            0b111 => {
                let sign_bit = val & 0x80;
                if count == 1 {
                    self.set_flag(FLAG_OF, false); // Always 0 for SAR
                }
                for _ in 0..count {
                    let carry_out = (result & 0x01) != 0;
                    result = (result >> 1) | sign_bit;
                    self.set_flag(FLAG_CF, carry_out);
                }
                self.update_flags_8(result);
            }
            _ => {}
        }

        result
    }

    /// Perform 16-bit shift/rotate operation
    fn shift_rotate_16(&mut self, val: u16, op: u8, count: u8) -> u16 {
        if count == 0 {
            return val;
        }

        // On 80186+, shift count is masked to 5 bits (0-31)
        // On 8086/8088, full 8-bit count is used (can shift by 0-255)
        let count = if self.model.supports_80186_instructions() {
            count & 0x1F
        } else {
            count
        };
        let mut result = val;

        match op {
            // ROL - Rotate left
            0b000 => {
                for _ in 0..count {
                    let carry_out = (result & 0x8000) != 0;
                    result = (result << 1) | (if carry_out { 1 } else { 0 });
                    self.set_flag(FLAG_CF, carry_out);
                }
                if count == 1 {
                    let msb = (result & 0x8000) != 0;
                    self.set_flag(FLAG_OF, msb != self.get_flag(FLAG_CF));
                }
            }
            // ROR - Rotate right
            0b001 => {
                for _ in 0..count {
                    let carry_out = (result & 0x0001) != 0;
                    result = (result >> 1) | (if carry_out { 0x8000 } else { 0 });
                    self.set_flag(FLAG_CF, carry_out);
                }
                if count == 1 {
                    let bit15 = (result & 0x8000) != 0;
                    let bit14 = (result & 0x4000) != 0;
                    self.set_flag(FLAG_OF, bit15 != bit14);
                }
            }
            // RCL - Rotate through carry left
            0b010 => {
                for _ in 0..count {
                    let old_cf = if self.get_flag(FLAG_CF) { 1 } else { 0 };
                    let carry_out = (result & 0x8000) != 0;
                    result = (result << 1) | old_cf;
                    self.set_flag(FLAG_CF, carry_out);
                }
                if count == 1 {
                    let msb = (result & 0x8000) != 0;
                    self.set_flag(FLAG_OF, msb != self.get_flag(FLAG_CF));
                }
            }
            // RCR - Rotate through carry right
            0b011 => {
                for _ in 0..count {
                    let old_cf = if self.get_flag(FLAG_CF) { 0x8000 } else { 0 };
                    let carry_out = (result & 0x0001) != 0;
                    result = (result >> 1) | old_cf;
                    self.set_flag(FLAG_CF, carry_out);
                }
                if count == 1 {
                    let bit15 = (result & 0x8000) != 0;
                    let bit14 = (result & 0x4000) != 0;
                    self.set_flag(FLAG_OF, bit15 != bit14);
                }
            }
            // SHL/SAL - Shift left
            0b100 | 0b110 => {
                for _ in 0..count {
                    let carry_out = (result & 0x8000) != 0;
                    result <<= 1;
                    self.set_flag(FLAG_CF, carry_out);
                }
                self.update_flags_16(result);
                if count == 1 {
                    let msb = (result & 0x8000) != 0;
                    self.set_flag(FLAG_OF, msb != self.get_flag(FLAG_CF));
                }
            }
            // SHR - Shift right
            0b101 => {
                if count == 1 {
                    self.set_flag(FLAG_OF, (val & 0x8000) != 0);
                }
                for _ in 0..count {
                    let carry_out = (result & 0x0001) != 0;
                    result >>= 1;
                    self.set_flag(FLAG_CF, carry_out);
                }
                self.update_flags_16(result);
            }
            // SAR - Shift arithmetic right
            0b111 => {
                let sign_bit = val & 0x8000;
                if count == 1 {
                    self.set_flag(FLAG_OF, false); // Always 0 for SAR
                }
                for _ in 0..count {
                    let carry_out = (result & 0x0001) != 0;
                    result = (result >> 1) | sign_bit;
                    self.set_flag(FLAG_CF, carry_out);
                }
                self.update_flags_16(result);
            }
            _ => {}
        }

        result
    }

    /// Public method to read a byte from memory using segment:offset
    /// This is used for BIOS interrupt handlers that need to access memory
    #[inline]
    pub fn read_byte(&self, segment: u16, offset: u16) -> u8 {
        self.read(segment, offset)
    }

    /// Public method to write a byte to memory using segment:offset
    /// This is used for BIOS interrupt handlers that need to access memory
    #[inline]
    pub fn write_byte(&mut self, segment: u16, offset: u16, val: u8) {
        self.write(segment, offset, val);
    }

    /// Decode ModR/M byte and return (mod, reg, r/m)
    #[inline]
    fn decode_modrm(modrm: u8) -> (u8, u8, u8) {
        let modbits = (modrm >> 6) & 0x03; // Bits 7-6
        let reg = (modrm >> 3) & 0x07; // Bits 5-3
        let rm = modrm & 0x07; // Bits 2-0
        (modbits, reg, rm)
    }

    /// Calculate effective address from ModR/M byte
    /// Returns (segment, offset) and number of additional bytes consumed
    fn calc_effective_address(&mut self, modbits: u8, rm: u8) -> (u16, u16, u8) {
        let (default_seg, offset, bytes_read) = match modbits {
            // mod = 00: Memory mode with no displacement (except for special case rm=110)
            0b00 => {
                match rm {
                    0b000 => (self.ds, (self.bx as u16).wrapping_add(self.si as u16), 0), // [BX+SI]
                    0b001 => (self.ds, (self.bx as u16).wrapping_add(self.di as u16), 0), // [BX+DI]
                    0b010 => (self.ss, (self.bp as u16).wrapping_add(self.si as u16), 0), // [BP+SI]
                    0b011 => (self.ss, (self.bp as u16).wrapping_add(self.di as u16), 0), // [BP+DI]
                    0b100 => (self.ds, self.si as u16, 0),                                // [SI]
                    0b101 => (self.ds, self.di as u16, 0),                                // [DI]
                    0b110 => {
                        // Special case: direct address (16-bit displacement)
                        let disp = self.fetch_u16();
                        (self.ds, disp, 2)
                    }
                    0b111 => (self.ds, self.bx as u16, 0), // [BX]
                    _ => unreachable!(),
                }
            }
            // mod = 01: Memory mode with 8-bit signed displacement
            0b01 => {
                let disp = self.fetch_u8() as i8 as i16 as u16;
                match rm {
                    0b000 => (
                        self.ds,
                        (self.bx as u16)
                            .wrapping_add(self.si as u16)
                            .wrapping_add(disp),
                        1,
                    ), // [BX+SI+disp8]
                    0b001 => (
                        self.ds,
                        (self.bx as u16)
                            .wrapping_add(self.di as u16)
                            .wrapping_add(disp),
                        1,
                    ), // [BX+DI+disp8]
                    0b010 => (
                        self.ss,
                        (self.bp as u16)
                            .wrapping_add(self.si as u16)
                            .wrapping_add(disp),
                        1,
                    ), // [BP+SI+disp8]
                    0b011 => (
                        self.ss,
                        (self.bp as u16)
                            .wrapping_add(self.di as u16)
                            .wrapping_add(disp),
                        1,
                    ), // [BP+DI+disp8]
                    0b100 => (self.ds, (self.si as u16).wrapping_add(disp), 1), // [SI+disp8]
                    0b101 => (self.ds, (self.di as u16).wrapping_add(disp), 1), // [DI+disp8]
                    0b110 => (self.ss, (self.bp as u16).wrapping_add(disp), 1), // [BP+disp8]
                    0b111 => (self.ds, (self.bx as u16).wrapping_add(disp), 1), // [BX+disp8]
                    _ => unreachable!(),
                }
            }
            // mod = 10: Memory mode with 16-bit signed displacement
            0b10 => {
                let disp = self.fetch_u16();
                match rm {
                    0b000 => (
                        self.ds,
                        (self.bx as u16)
                            .wrapping_add(self.si as u16)
                            .wrapping_add(disp),
                        2,
                    ), // [BX+SI+disp16]
                    0b001 => (
                        self.ds,
                        (self.bx as u16)
                            .wrapping_add(self.di as u16)
                            .wrapping_add(disp),
                        2,
                    ), // [BX+DI+disp16]
                    0b010 => (
                        self.ss,
                        (self.bp as u16)
                            .wrapping_add(self.si as u16)
                            .wrapping_add(disp),
                        2,
                    ), // [BP+SI+disp16]
                    0b011 => (
                        self.ss,
                        (self.bp as u16)
                            .wrapping_add(self.di as u16)
                            .wrapping_add(disp),
                        2,
                    ), // [BP+DI+disp16]
                    0b100 => (self.ds, (self.si as u16).wrapping_add(disp), 2), // [SI+disp16]
                    0b101 => (self.ds, (self.di as u16).wrapping_add(disp), 2), // [DI+disp16]
                    0b110 => (self.ss, (self.bp as u16).wrapping_add(disp), 2), // [BP+disp16]
                    0b111 => (self.ds, (self.bx as u16).wrapping_add(disp), 2), // [BX+disp16]
                    _ => unreachable!(),
                }
            }
            // mod = 11: Register mode (no memory access)
            _ => (0, 0, 0), // Not used for register mode
        };

        // Apply segment override if present
        let seg = self.get_segment_with_override(default_seg);
        (seg, offset, bytes_read)
    }

    /// Calculate effective offset from ModR/M byte without consuming segment override
    /// Used by LEA which doesn't access memory
    /// Returns offset only
    fn calc_effective_offset(&mut self, modbits: u8, rm: u8) -> u16 {
        match modbits {
            // mod = 00: Memory mode with no displacement (except for special case rm=110)
            0b00 => {
                match rm {
                    0b000 => (self.bx as u16).wrapping_add(self.si as u16), // [BX+SI]
                    0b001 => (self.bx as u16).wrapping_add(self.di as u16), // [BX+DI]
                    0b010 => (self.bp as u16).wrapping_add(self.si as u16), // [BP+SI]
                    0b011 => (self.bp as u16).wrapping_add(self.di as u16), // [BP+DI]
                    0b100 => self.si as u16,                                // [SI]
                    0b101 => self.di as u16,                                // [DI]
                    0b110 => {
                        // Special case: direct address (16-bit displacement)
                        self.fetch_u16()
                    }
                    0b111 => self.bx as u16, // [BX]
                    _ => unreachable!(),
                }
            }
            // mod = 01: Memory mode with 8-bit signed displacement
            0b01 => {
                let disp = self.fetch_u8() as i8 as i16 as u16;
                match rm {
                    0b000 => (self.bx as u16)
                        .wrapping_add(self.si as u16)
                        .wrapping_add(disp), // [BX+SI+disp8]
                    0b001 => (self.bx as u16)
                        .wrapping_add(self.di as u16)
                        .wrapping_add(disp), // [BX+DI+disp8]
                    0b010 => (self.bp as u16)
                        .wrapping_add(self.si as u16)
                        .wrapping_add(disp), // [BP+SI+disp8]
                    0b011 => (self.bp as u16)
                        .wrapping_add(self.di as u16)
                        .wrapping_add(disp), // [BP+DI+disp8]
                    0b100 => (self.si as u16).wrapping_add(disp), // [SI+disp8]
                    0b101 => (self.di as u16).wrapping_add(disp), // [DI+disp8]
                    0b110 => (self.bp as u16).wrapping_add(disp), // [BP+disp8]
                    0b111 => (self.bx as u16).wrapping_add(disp), // [BX+disp8]
                    _ => unreachable!(),
                }
            }
            // mod = 10: Memory mode with 16-bit signed displacement
            0b10 => {
                let disp = self.fetch_u16();
                match rm {
                    0b000 => (self.bx as u16)
                        .wrapping_add(self.si as u16)
                        .wrapping_add(disp), // [BX+SI+disp16]
                    0b001 => (self.bx as u16)
                        .wrapping_add(self.di as u16)
                        .wrapping_add(disp), // [BX+DI+disp16]
                    0b010 => (self.bp as u16)
                        .wrapping_add(self.si as u16)
                        .wrapping_add(disp), // [BP+SI+disp16]
                    0b011 => (self.bp as u16)
                        .wrapping_add(self.di as u16)
                        .wrapping_add(disp), // [BP+DI+disp16]
                    0b100 => (self.si as u16).wrapping_add(disp), // [SI+disp16]
                    0b101 => (self.di as u16).wrapping_add(disp), // [DI+disp16]
                    0b110 => (self.bp as u16).wrapping_add(disp), // [BP+disp16]
                    0b111 => (self.bx as u16).wrapping_add(disp), // [BX+disp16]
                    _ => unreachable!(),
                }
            }
            // mod = 11: Register mode (no memory access)
            _ => 0, // Not used for register mode
        }
    }

    /// Decode SIB (Scale-Index-Base) byte for 32-bit addressing
    /// Returns: (scale, index_reg, base_reg, bytes_consumed)
    /// Scale values: 1, 2, 4, 8
    fn decode_sib(&mut self) -> (u32, u8, u8, u8) {
        let sib = self.fetch_u8();

        // SIB byte format: [SS][III][BBB]
        // SS = scale (00=1, 01=2, 10=4, 11=8)
        // III = index register (0-7, where 4 = none/ESP)
        // BBB = base register (0-7)
        let scale_bits = (sib >> 6) & 0b11;
        let index = (sib >> 3) & 0b111;
        let base = sib & 0b111;

        let scale = match scale_bits {
            0b00 => 1,
            0b01 => 2,
            0b10 => 4,
            0b11 => 8,
            _ => unreachable!(),
        };

        (scale, index, base, 1)
    }

    /// Calculate 32-bit effective address from ModR/M byte and optional SIB byte
    /// Returns: (segment, offset_32bit, bytes_consumed)
    fn calc_effective_address_32(&mut self, modbits: u8, rm: u8) -> (u16, u32, u8) {
        let (default_seg, offset, bytes_read) = match modbits {
            // mod = 00: Memory mode with no displacement (except for special cases)
            0b00 => {
                if rm == 0b100 {
                    // SIB byte follows
                    let (scale, index, base, sib_bytes) = self.decode_sib();

                    // Calculate index contribution
                    let index_val = if index == 4 {
                        // ESP as index means no index
                        0u32
                    } else {
                        self.get_reg32(index).wrapping_mul(scale)
                    };

                    // Calculate base contribution
                    if base == 5 {
                        // Special case: [disp32] or [index*scale+disp32]
                        let disp = self.fetch_u32();
                        (self.ds, index_val.wrapping_add(disp), sib_bytes + 4)
                    } else {
                        let base_val = self.get_reg32(base);
                        let default_seg = if base == 4 || base == 5 {
                            self.ss
                        } else {
                            self.ds
                        };
                        (default_seg, base_val.wrapping_add(index_val), sib_bytes)
                    }
                } else if rm == 0b101 {
                    // Special case: [disp32]
                    let disp = self.fetch_u32();
                    (self.ds, disp, 4)
                } else {
                    // Direct register: [EAX], [ECX], [EDX], [EBX], [ESI], [EDI]
                    let reg_val = self.get_reg32(rm);
                    let default_seg = if rm == 4 || rm == 5 { self.ss } else { self.ds };
                    (default_seg, reg_val, 0)
                }
            }
            // mod = 01: Memory mode with 8-bit signed displacement
            0b01 => {
                if rm == 0b100 {
                    // SIB byte follows, then disp8
                    let (scale, index, base, sib_bytes) = self.decode_sib();
                    let disp = self.fetch_u8() as i8 as i32 as u32;

                    let index_val = if index == 4 {
                        0u32
                    } else {
                        self.get_reg32(index).wrapping_mul(scale)
                    };

                    let base_val = self.get_reg32(base);
                    let default_seg = if base == 4 || base == 5 {
                        self.ss
                    } else {
                        self.ds
                    };
                    (
                        default_seg,
                        base_val.wrapping_add(index_val).wrapping_add(disp),
                        sib_bytes + 1,
                    )
                } else {
                    // Direct register + disp8
                    let disp = self.fetch_u8() as i8 as i32 as u32;
                    let reg_val = self.get_reg32(rm);
                    let default_seg = if rm == 4 || rm == 5 { self.ss } else { self.ds };
                    (default_seg, reg_val.wrapping_add(disp), 1)
                }
            }
            // mod = 10: Memory mode with 32-bit displacement
            0b10 => {
                if rm == 0b100 {
                    // SIB byte follows, then disp32
                    let (scale, index, base, sib_bytes) = self.decode_sib();
                    let disp = self.fetch_u32();

                    let index_val = if index == 4 {
                        0u32
                    } else {
                        self.get_reg32(index).wrapping_mul(scale)
                    };

                    let base_val = self.get_reg32(base);
                    let default_seg = if base == 4 || base == 5 {
                        self.ss
                    } else {
                        self.ds
                    };
                    (
                        default_seg,
                        base_val.wrapping_add(index_val).wrapping_add(disp),
                        sib_bytes + 4,
                    )
                } else {
                    // Direct register + disp32
                    let disp = self.fetch_u32();
                    let reg_val = self.get_reg32(rm);
                    let default_seg = if rm == 4 || rm == 5 { self.ss } else { self.ds };
                    (default_seg, reg_val.wrapping_add(disp), 4)
                }
            }
            // mod = 11: Register mode (no memory access)
            _ => (0, 0, 0), // Not used for register mode
        };

        // Apply segment override if present
        let seg = self.get_segment_with_override(default_seg);
        (seg, offset, bytes_read)
    }

    /// Read 8-bit value from ModR/M operand (either register or memory)
    fn read_rm8(&mut self, modbits: u8, rm: u8) -> u8 {
        if modbits == 0b11 {
            // Register mode
            if rm < 4 {
                self.get_reg8_low(rm)
            } else {
                self.get_reg8_high(rm - 4)
            }
        } else {
            // Memory mode
            let (seg, offset, _) = self.calc_effective_address(modbits, rm);
            self.read(seg, offset)
        }
    }

    /// Write 8-bit value to ModR/M operand (either register or memory)
    fn write_rm8(&mut self, modbits: u8, rm: u8, val: u8) {
        if modbits == 0b11 {
            // Register mode
            if rm < 4 {
                self.set_reg8_low(rm, val);
            } else {
                self.set_reg8_high(rm - 4, val);
            }
        } else {
            // Memory mode
            let (seg, offset, _) = self.calc_effective_address(modbits, rm);
            self.write(seg, offset, val);
        }
    }

    /// Read 16-bit value from ModR/M operand (either register or memory)
    fn read_rm16(&mut self, modbits: u8, rm: u8) -> u16 {
        if modbits == 0b11 {
            // Register mode
            self.get_reg16(rm)
        } else {
            // Memory mode
            let (seg, offset, _) = self.calc_effective_address(modbits, rm);
            self.read_u16(seg, offset)
        }
    }

    /// Write 16-bit value to ModR/M operand (either register or memory)
    fn write_rm16(&mut self, modbits: u8, rm: u8, val: u16) {
        if modbits == 0b11 {
            // Register mode
            self.set_reg16(rm, val);
        } else {
            // Memory mode
            let (seg, offset, _) = self.calc_effective_address(modbits, rm);
            self.write_u16(seg, offset, val);
        }
    }

    /// Helper for Read-Modify-Write operations on 16-bit values
    /// Returns (value_read, seg, offset) to avoid double-fetching EA
    fn read_rmw16(&mut self, modbits: u8, rm: u8) -> (u16, u16, u16) {
        if modbits == 0b11 {
            // Register mode - return dummy seg/offset
            (self.get_reg16(rm), 0, 0)
        } else {
            // Memory mode - calculate EA once and return it
            let (seg, offset, _) = self.calc_effective_address(modbits, rm);
            let val = self.read_u16(seg, offset);
            (val, seg, offset)
        }
    }

    /// Helper for writing result of Read-Modify-Write operations on 16-bit values
    /// Uses cached seg/offset to avoid recalculating EA
    fn write_rmw16(&mut self, modbits: u8, rm: u8, val: u16, seg: u16, offset: u16) {
        if modbits == 0b11 {
            // Register mode
            self.set_reg16(rm, val);
        } else {
            // Memory mode - use cached seg/offset
            self.write_u16(seg, offset, val);
        }
    }

    /// Helper for Read-Modify-Write operations on 8-bit values
    /// Returns (value_read, seg, offset) to avoid double-fetching EA
    fn read_rmw8(&mut self, modbits: u8, rm: u8) -> (u8, u16, u16) {
        if modbits == 0b11 {
            // Register mode - return dummy seg/offset
            let val = if rm < 4 {
                self.get_reg8_low(rm)
            } else {
                self.get_reg8_high(rm - 4)
            };
            (val, 0, 0)
        } else {
            // Memory mode - calculate EA once and return it
            let (seg, offset, _) = self.calc_effective_address(modbits, rm);
            let val = self.read(seg, offset);
            (val, seg, offset)
        }
    }

    /// Helper for writing result of Read-Modify-Write operations on 8-bit values
    /// Uses cached seg/offset to avoid recalculating EA
    fn write_rmw8(&mut self, modbits: u8, rm: u8, val: u8, seg: u16, offset: u16) {
        if modbits == 0b11 {
            // Register mode
            if rm < 4 {
                self.set_reg8_low(rm, val);
            } else {
                self.set_reg8_high(rm - 4, val);
            }
        } else {
            // Memory mode - use cached seg/offset
            self.write(seg, offset, val);
        }
    }

    /// Read 32-bit value from ModR/M operand (either register or memory)
    fn read_rm32(&mut self, modbits: u8, rm: u8) -> u32 {
        if modbits == 0b11 {
            // Register mode
            self.get_reg32(rm)
        } else {
            // Memory mode - use 32-bit addressing if override is set
            if self.address_size_override && self.model.supports_80386_instructions() {
                let (seg, offset, _) = self.calc_effective_address_32(modbits, rm);
                self.read_u32(seg, offset)
            } else {
                let (seg, offset, _) = self.calc_effective_address(modbits, rm);
                self.read_u32(seg, offset as u32)
            }
        }
    }

    /// Write 32-bit value to ModR/M operand (either register or memory)
    fn write_rm32(&mut self, modbits: u8, rm: u8, val: u32) {
        if modbits == 0b11 {
            // Register mode
            self.set_reg32(rm, val);
        } else {
            // Memory mode - use 32-bit addressing if override is set
            if self.address_size_override && self.model.supports_80386_instructions() {
                let (seg, offset, _) = self.calc_effective_address_32(modbits, rm);
                self.write_u32(seg, offset, val);
            } else {
                let (seg, offset, _) = self.calc_effective_address(modbits, rm);
                self.write_u32(seg, offset as u32, val);
            }
        }
    }

    /// Helper for Read-Modify-Write operations on 32-bit values
    /// Returns (value_read, seg, offset_u32) to avoid double-fetching EA
    fn read_rmw32(&mut self, modbits: u8, rm: u8) -> (u32, u16, u32) {
        if modbits == 0b11 {
            // Register mode - return dummy seg/offset
            (self.get_reg32(rm), 0, 0)
        } else {
            // Memory mode - calculate EA once and return it
            if self.address_size_override && self.model.supports_80386_instructions() {
                let (seg, offset, _) = self.calc_effective_address_32(modbits, rm);
                let val = self.read_u32(seg, offset);
                (val, seg, offset)
            } else {
                let (seg, offset, _) = self.calc_effective_address(modbits, rm);
                let val = self.read_u32(seg, offset as u32);
                (val, seg, offset as u32)
            }
        }
    }

    /// Helper for writing result of Read-Modify-Write operations on 32-bit values
    /// Uses cached seg/offset to avoid recalculating EA
    fn write_rmw32(&mut self, modbits: u8, rm: u8, val: u32, seg: u16, offset: u32) {
        if modbits == 0b11 {
            // Register mode
            self.set_reg32(rm, val);
        } else {
            // Memory mode - use cached seg/offset
            self.write_u32(seg, offset, val);
        }
    }

    /// Execute one instruction and return cycles used
    pub fn step(&mut self) -> u32 {
        if self.halted {
            // Even when halted, TSC continues to increment
            if self.model.supports_pentium_instructions() {
                self.tsc = self.tsc.wrapping_add(1u64);
            }
            return 1;
        }

        // Save instruction start IP for CPU exceptions
        self.instruction_start_ip = self.ip;

        let opcode = self.fetch_u8();

        let cycles_executed = match opcode {
            // REP/REPE/REPZ prefix (0xF3)
            0xF3 => {
                let next_opcode = self.fetch_u8();
                let mut total_cycles: u32 = 9; // Base prefix overhead

                match next_opcode {
                    // MOVSB
                    0xA4 => {
                        // Apply segment override to source (DS:SI), destination is always ES:DI
                        // Consume override once before the loop
                        let src_seg = self.get_segment_with_override(self.ds);
                        while self.cx != 0 {
                            let val = self.read(src_seg, self.si as u16);
                            self.write(self.es, self.di as u16, val);
                            if self.get_flag(FLAG_DF) {
                                self.si = self.si.wrapping_sub(1);
                                self.di = self.di.wrapping_sub(1);
                            } else {
                                self.si = self.si.wrapping_add(1);
                                self.di = self.di.wrapping_add(1);
                            }
                            self.cx = self.cx.wrapping_sub(1);
                            total_cycles += 17;
                        }
                        self.cycles += total_cycles as u64;
                        total_cycles
                    }
                    // MOVSW
                    0xA5 => {
                        // Apply segment override to source (DS:SI), destination is always ES:DI
                        // Consume override once before the loop
                        let src_seg = self.get_segment_with_override(self.ds);
                        while self.cx != 0 {
                            let val = self.read_u16(src_seg, self.si as u16);
                            self.write_u16(self.es, self.di as u16, val);
                            if self.get_flag(FLAG_DF) {
                                self.si = self.si.wrapping_sub(2u32);
                                self.di = self.di.wrapping_sub(2u32);
                            } else {
                                self.si = self.si.wrapping_add(2u32);
                                self.di = self.di.wrapping_add(2u32);
                            }
                            self.cx = self.cx.wrapping_sub(1);
                            total_cycles += 17;
                        }
                        self.cycles += total_cycles as u64;
                        total_cycles
                    }
                    // STOSB
                    0xAA => {
                        let al = (self.ax & 0xFF) as u8;
                        while self.cx != 0 {
                            self.write(self.es, self.di as u16, al);
                            if self.get_flag(FLAG_DF) {
                                self.di = self.di.wrapping_sub(1);
                            } else {
                                self.di = self.di.wrapping_add(1);
                            }
                            self.cx = self.cx.wrapping_sub(1);
                            total_cycles += 10;
                        }
                        self.cycles += total_cycles as u64;
                        total_cycles
                    }
                    // STOSW
                    0xAB => {
                        while self.cx != 0 {
                            self.write_u16(self.es, self.di as u16, self.ax as u16);
                            if self.get_flag(FLAG_DF) {
                                self.di = self.di.wrapping_sub(2u32);
                            } else {
                                self.di = self.di.wrapping_add(2u32);
                            }
                            self.cx = self.cx.wrapping_sub(1);
                            total_cycles += 10;
                        }
                        self.cycles += total_cycles as u64;
                        total_cycles
                    }
                    // LODSB
                    0xAC => {
                        // Apply segment override to source (DS:SI)
                        let src_seg = self.get_segment_with_override(self.ds);
                        while self.cx != 0 {
                            let val = self.read(src_seg, self.si as u16);
                            self.ax = (self.ax & 0xFFFF_FF00) | (val as u32);
                            if self.get_flag(FLAG_DF) {
                                self.si = self.si.wrapping_sub(1);
                            } else {
                                self.si = self.si.wrapping_add(1);
                            }
                            self.cx = self.cx.wrapping_sub(1);
                            total_cycles += 13;
                        }
                        self.cycles += total_cycles as u64;
                        total_cycles
                    }
                    // LODSW
                    0xAD => {
                        // Apply segment override to source (DS:SI)
                        let src_seg = self.get_segment_with_override(self.ds);
                        while self.cx != 0 {
                            self.ax = (self.ax & 0xFFFF_0000)
                                | (self.read_u16(src_seg, self.si as u16) as u32);
                            if self.get_flag(FLAG_DF) {
                                self.si = self.si.wrapping_sub(2u32);
                            } else {
                                self.si = self.si.wrapping_add(2u32);
                            }
                            self.cx = self.cx.wrapping_sub(1);
                            total_cycles += 13;
                        }
                        self.cycles += total_cycles as u64;
                        total_cycles
                    }
                    // CMPSB
                    0xA6 => {
                        // Debug: Log the comparison if enabled
                        let debug_cmpsb = std::env::var("EMU_DEBUG_CMPSB").is_ok();
                        // Apply segment override to source (DS:SI), destination is always ES:DI
                        let src_seg = self.get_segment_with_override(self.ds);
                        if debug_cmpsb {
                            eprintln!("[REPE CMPSB] Starting: CX={:04X} DS:SI={:04X}:{:04X} ES:DI={:04X}:{:04X}", 
                                self.cx, src_seg, self.si, self.es, self.di);
                        }

                        while self.cx != 0 {
                            let src = self.read(src_seg, self.si as u16);
                            let dst = self.read(self.es, self.di as u16);

                            if debug_cmpsb {
                                eprintln!("[REPE CMPSB] Comparing: SRC:{:04X}:{:04X}=0x{:02X} vs ES:{:04X}:{:04X}=0x{:02X} (CX={:04X})", 
                                    src_seg, self.si, src, self.es, self.di, dst, self.cx);
                            }

                            let result = src.wrapping_sub(dst);
                            let borrow = (src as u16) < (dst as u16);
                            let overflow = ((src ^ dst) & (src ^ result) & 0x80) != 0;
                            self.update_flags_8(result);
                            self.set_flag(FLAG_CF, borrow);
                            self.set_flag(FLAG_OF, overflow);
                            if self.get_flag(FLAG_DF) {
                                self.si = self.si.wrapping_sub(1);
                                self.di = self.di.wrapping_sub(1);
                            } else {
                                self.si = self.si.wrapping_add(1);
                                self.di = self.di.wrapping_add(1);
                            }
                            self.cx = self.cx.wrapping_sub(1);
                            total_cycles += 22;
                            // REPE: Exit if ZF=0
                            if !self.get_flag(FLAG_ZF) {
                                if debug_cmpsb {
                                    eprintln!("[REPE CMPSB] Mismatch! Exiting early. ZF=0");
                                }
                                break;
                            }
                        }

                        if debug_cmpsb {
                            eprintln!(
                                "[REPE CMPSB] Finished: CX={:04X} ZF={}",
                                self.cx,
                                if self.get_flag(FLAG_ZF) { 1 } else { 0 }
                            );
                        }

                        self.cycles += total_cycles as u64;
                        total_cycles
                    }
                    // CMPSW
                    0xA7 => {
                        // Apply segment override to source (DS:SI), destination is always ES:DI
                        let src_seg = self.get_segment_with_override(self.ds);
                        while self.cx != 0 {
                            let src = self.read_u16(src_seg, self.si as u16);
                            let dst = self.read_u16(self.es, self.di as u16);
                            let result = src.wrapping_sub(dst);
                            let borrow = (src as u32) < (dst as u32);
                            let overflow = ((src ^ dst) & (src ^ result) & 0x8000) != 0;
                            self.update_flags_16(result as u16);
                            self.set_flag(FLAG_CF, borrow);
                            self.set_flag(FLAG_OF, overflow);
                            if self.get_flag(FLAG_DF) {
                                self.si = self.si.wrapping_sub(2u32);
                                self.di = self.di.wrapping_sub(2u32);
                            } else {
                                self.si = self.si.wrapping_add(2u32);
                                self.di = self.di.wrapping_add(2u32);
                            }
                            self.cx = self.cx.wrapping_sub(1);
                            total_cycles += 22;
                            // REPE: Exit if ZF=0
                            if !self.get_flag(FLAG_ZF) {
                                break;
                            }
                        }
                        self.cycles += total_cycles as u64;
                        total_cycles
                    }
                    // SCASB
                    0xAE => {
                        let al = (self.ax & 0xFF) as u8;
                        while self.cx != 0 {
                            let val = self.read(self.es, self.di as u16);
                            let result = al.wrapping_sub(val);
                            let borrow = (al as u16) < (val as u16);
                            let overflow = ((al ^ val) & (al ^ result) & 0x80) != 0;
                            self.update_flags_8(result);
                            self.set_flag(FLAG_CF, borrow);
                            self.set_flag(FLAG_OF, overflow);
                            if self.get_flag(FLAG_DF) {
                                self.di = self.di.wrapping_sub(1);
                            } else {
                                self.di = self.di.wrapping_add(1);
                            }
                            self.cx = self.cx.wrapping_sub(1);
                            total_cycles += 15;
                            // REPE: Exit if ZF=0
                            if !self.get_flag(FLAG_ZF) {
                                break;
                            }
                        }
                        self.cycles += total_cycles as u64;
                        total_cycles
                    }
                    // SCASW
                    0xAF => {
                        while self.cx != 0 {
                            let val = self.read_u16(self.es, self.di as u16);
                            let result = (self.ax as u16).wrapping_sub(val);
                            let borrow = self.ax < (val as u32);
                            let overflow = (((self.ax as u16) ^ val)
                                & ((self.ax as u16) ^ (result as u16))
                                & 0x8000)
                                != 0;
                            self.update_flags_16(result);
                            self.set_flag(FLAG_CF, borrow);
                            self.set_flag(FLAG_OF, overflow);
                            if self.get_flag(FLAG_DF) {
                                self.di = self.di.wrapping_sub(2u32);
                            } else {
                                self.di = self.di.wrapping_add(2u32);
                            }
                            self.cx = self.cx.wrapping_sub(1);
                            total_cycles += 15;
                            // REPE: Exit if ZF=0
                            if !self.get_flag(FLAG_ZF) {
                                break;
                            }
                        }
                        self.cycles += total_cycles as u64;
                        total_cycles
                    }
                    _ => {
                        // REP prefix with non-string instruction
                        // According to x86 behavior, REP before non-string instructions is ignored
                        // We need to "un-fetch" the opcode and execute it normally
                        // Since we already fetched it, we decrement IP to put it back
                        self.ip = self.ip.wrapping_sub(1);
                        // Now execute the instruction normally by recursing
                        let cycles = self.step();
                        self.cycles = self.cycles.wrapping_sub(cycles as u64); // Remove the cycles we're about to re-add
                        cycles
                    }
                }
            }

            // REPNZ/REPNE prefix (0xF2)
            0xF2 => {
                let next_opcode = self.fetch_u8();
                let mut total_cycles: u32 = 9; // Base prefix overhead

                match next_opcode {
                    // CMPSB
                    0xA6 => {
                        let src_seg = self.get_segment_with_override(self.ds);
                        while self.cx != 0 {
                            let src = self.read(src_seg, self.si as u16);
                            let dst = self.read(self.es, self.di as u16);
                            let result = src.wrapping_sub(dst);
                            let borrow = (src as u16) < (dst as u16);
                            let overflow = ((src ^ dst) & (src ^ result) & 0x80) != 0;
                            self.update_flags_8(result);
                            self.set_flag(FLAG_CF, borrow);
                            self.set_flag(FLAG_OF, overflow);
                            if self.get_flag(FLAG_DF) {
                                self.si = self.si.wrapping_sub(1);
                                self.di = self.di.wrapping_sub(1);
                            } else {
                                self.si = self.si.wrapping_add(1);
                                self.di = self.di.wrapping_add(1);
                            }
                            self.cx = self.cx.wrapping_sub(1);
                            total_cycles += 22;
                            // REPNE: Exit if ZF=1
                            if self.get_flag(FLAG_ZF) {
                                break;
                            }
                        }
                        self.cycles += total_cycles as u64;
                        total_cycles
                    }
                    // CMPSW
                    0xA7 => {
                        let src_seg = self.get_segment_with_override(self.ds);
                        while self.cx != 0 {
                            let src = self.read_u16(src_seg, self.si as u16);
                            let dst = self.read_u16(self.es, self.di as u16);
                            let result = src.wrapping_sub(dst);
                            let borrow = (src as u32) < (dst as u32);
                            let overflow = ((src ^ dst) & (src ^ result) & 0x8000) != 0;
                            self.update_flags_16(result as u16);
                            self.set_flag(FLAG_CF, borrow);
                            self.set_flag(FLAG_OF, overflow);
                            if self.get_flag(FLAG_DF) {
                                self.si = self.si.wrapping_sub(2u32);
                                self.di = self.di.wrapping_sub(2u32);
                            } else {
                                self.si = self.si.wrapping_add(2u32);
                                self.di = self.di.wrapping_add(2u32);
                            }
                            self.cx = self.cx.wrapping_sub(1);
                            total_cycles += 22;
                            // REPNE: Exit if ZF=1
                            if self.get_flag(FLAG_ZF) {
                                break;
                            }
                        }
                        self.cycles += total_cycles as u64;
                        total_cycles
                    }
                    // SCASB
                    0xAE => {
                        let al = (self.ax & 0xFF) as u8;
                        while self.cx != 0 {
                            let val = self.read(self.es, self.di as u16);
                            let result = al.wrapping_sub(val);
                            let borrow = (al as u16) < (val as u16);
                            let overflow = ((al ^ val) & (al ^ result) & 0x80) != 0;
                            self.update_flags_8(result);
                            self.set_flag(FLAG_CF, borrow);
                            self.set_flag(FLAG_OF, overflow);
                            if self.get_flag(FLAG_DF) {
                                self.di = self.di.wrapping_sub(1);
                            } else {
                                self.di = self.di.wrapping_add(1);
                            }
                            self.cx = self.cx.wrapping_sub(1);
                            total_cycles += 15;
                            // REPNE: Exit if ZF=1
                            if self.get_flag(FLAG_ZF) {
                                break;
                            }
                        }
                        self.cycles += total_cycles as u64;
                        total_cycles
                    }
                    // SCASW
                    0xAF => {
                        while self.cx != 0 {
                            let val = self.read_u16(self.es, self.di as u16);
                            let result = (self.ax as u16).wrapping_sub(val);
                            let borrow = self.ax < (val as u32);
                            let overflow = (((self.ax as u16) ^ val)
                                & ((self.ax as u16) ^ (result as u16))
                                & 0x8000)
                                != 0;
                            self.update_flags_16(result);
                            self.set_flag(FLAG_CF, borrow);
                            self.set_flag(FLAG_OF, overflow);
                            if self.get_flag(FLAG_DF) {
                                self.di = self.di.wrapping_sub(2u32);
                            } else {
                                self.di = self.di.wrapping_add(2u32);
                            }
                            self.cx = self.cx.wrapping_sub(1);
                            total_cycles += 15;
                            // REPNE: Exit if ZF=1
                            if self.get_flag(FLAG_ZF) {
                                break;
                            }
                        }
                        self.cycles += total_cycles as u64;
                        total_cycles
                    }
                    _ => {
                        // REPNZ prefix with non-string instruction
                        // According to x86 behavior, REPNZ before non-string instructions is ignored
                        // We need to "un-fetch" the opcode and execute it normally
                        self.ip = self.ip.wrapping_sub(1);
                        // Now execute the instruction normally by recursing
                        let cycles = self.step();
                        self.cycles = self.cycles.wrapping_sub(cycles as u64); // Remove the cycles we're about to re-add
                        cycles
                    }
                }
            }

            // NOP
            0x90 => {
                self.cycles += 3;
                3
            }

            // HLT
            0xF4 => {
                self.halted = true;
                self.cycles += 2;
                2
            }

            // MOV reg8, imm8 (B0-B7)
            0xB0..=0xB7 => {
                let reg = opcode & 0x07;
                let val = self.fetch_u8();
                if reg < 4 {
                    self.set_reg8_low(reg, val);
                } else {
                    self.set_reg8_high(reg - 4, val);
                }
                self.cycles += 4;
                4
            }

            // MOV reg16, imm16 (B8-BF)
            0xB8..=0xBF => {
                let reg = opcode & 0x07;
                let val = self.fetch_u16();
                self.set_reg16(reg, val);
                self.cycles += 4;
                4
            }

            // MOV r/m8, r8 (0x88)
            0x88 => {
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);
                let val = if reg < 4 {
                    self.get_reg8_low(reg)
                } else {
                    self.get_reg8_high(reg - 4)
                };
                self.write_rm8(modbits, rm, val);
                self.cycles += if modbits == 0b11 { 2 } else { 9 };
                if modbits == 0b11 {
                    2
                } else {
                    9
                }
            }

            // MOV r/m16, r16 (0x89)
            0x89 => {
                // MOV r/m16/32, r16/32 - Move register to r/m
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);

                if self.operand_size_override && self.model.supports_80386_instructions() {
                    // 32-bit operand
                    let val = self.get_reg32(reg);
                    self.write_rm32(modbits, rm, val);
                } else {
                    // 16-bit operand (default)
                    let val = self.get_reg16(reg);
                    self.write_rm16(modbits, rm, val);
                }

                self.cycles += if modbits == 0b11 { 2 } else { 9 };
                if modbits == 0b11 {
                    2
                } else {
                    9
                }
            }

            // MOV r8, r/m8 (0x8A)
            0x8A => {
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);
                let val = self.read_rm8(modbits, rm);
                if reg < 4 {
                    self.set_reg8_low(reg, val);
                } else {
                    self.set_reg8_high(reg - 4, val);
                }
                self.cycles += if modbits == 0b11 { 2 } else { 8 };
                if modbits == 0b11 {
                    2
                } else {
                    8
                }
            }

            // MOV r16/32, r/m16/32 (0x8B)
            0x8B => {
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);

                if self.operand_size_override && self.model.supports_80386_instructions() {
                    // 32-bit operand
                    let val = self.read_rm32(modbits, rm);
                    self.set_reg32(reg, val);
                } else {
                    // 16-bit operand (default)
                    let val = self.read_rm16(modbits, rm);
                    self.set_reg16(reg, val);
                }

                self.cycles += if modbits == 0b11 { 2 } else { 8 };
                if modbits == 0b11 {
                    2
                } else {
                    8
                }
            }

            // MOV r/m16, Sreg (0x8C) - Move segment register to r/m16
            0x8C => {
                let modrm = self.fetch_u8();
                let (modbits, seg, rm) = Self::decode_modrm(modrm);
                let val = self.get_seg(seg & 0x03); // Only ES, CS, SS, DS (0-3)
                self.write_rm16(modbits, rm, val);
                self.cycles += if modbits == 0b11 { 2 } else { 9 };
                if modbits == 0b11 {
                    2
                } else {
                    9
                }
            }

            // MOV Sreg, r/m16 (0x8E) - Move r/m16 to segment register
            0x8E => {
                let modrm = self.fetch_u8();
                let (modbits, seg, rm) = Self::decode_modrm(modrm);
                let val = self.read_rm16(modbits, rm);
                self.set_seg(seg & 0x03, val); // Only ES, CS, SS, DS (0-3)
                self.cycles += if modbits == 0b11 { 2 } else { 8 };
                if modbits == 0b11 {
                    2
                } else {
                    8
                }
            }

            // POP r/m16 (0x8F) - Group 1A
            0x8F => {
                let modrm = self.fetch_u8();
                let (modbits, op, rm) = Self::decode_modrm(modrm);
                // Only op=0 is valid for POP (other values are undefined)
                if op == 0 {
                    let val = self.pop();
                    self.write_rm16(modbits, rm, val);
                    self.cycles += if modbits == 0b11 { 8 } else { 17 };
                    if modbits == 0b11 {
                        8
                    } else {
                        17
                    }
                } else {
                    // Undefined operation - treat as NOP
                    eprintln!(
                        "Undefined 0x8F operation with op={} at CS:IP={:04X}:{:04X}",
                        op,
                        self.cs,
                        self.ip.wrapping_sub(2u32)
                    );
                    self.cycles += 1;
                    1
                }
            }

            // ADD r/m8, r8 (0x00)
            0x00 => {
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);
                let reg_val = if reg < 4 {
                    self.get_reg8_low(reg)
                } else {
                    self.get_reg8_high(reg - 4)
                };

                // Use RMW helpers to avoid double-fetching EA
                let (rm_val, seg, offset) = self.read_rmw8(modbits, rm);
                let result = rm_val.wrapping_add(reg_val);
                let carry = (rm_val as u16 + reg_val as u16) > 0xFF;
                let overflow = ((rm_val ^ result) & (reg_val ^ result) & 0x80) != 0;
                let af = Self::calc_af_add_8(rm_val, reg_val);

                self.write_rmw8(modbits, rm, result, seg, offset);
                self.update_flags_8(result);
                self.set_flag(FLAG_CF, carry);
                self.set_flag(FLAG_OF, overflow);
                self.set_flag(FLAG_AF, af);
                self.cycles += if modbits == 0b11 { 3 } else { 16 };
                if modbits == 0b11 {
                    3
                } else {
                    16
                }
            }

            // ADD r/m16, r16 (0x01)
            0x01 => {
                // ADD r/m16/32, r16/32
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);

                if self.operand_size_override && self.model.supports_80386_instructions() {
                    // 32-bit operation
                    let reg_val = self.get_reg32(reg);
                    let (rm_val, seg, offset) = self.read_rmw32(modbits, rm);
                    let result = rm_val.wrapping_add(reg_val);
                    let carry = (rm_val as u64 + reg_val as u64) > 0xFFFFFFFF;
                    let overflow = ((rm_val ^ result) & (reg_val ^ result) & 0x80000000) != 0;
                    let af = Self::calc_af_add_32(rm_val, reg_val);

                    self.write_rmw32(modbits, rm, result, seg, offset);
                    self.update_flags_32(result);
                    self.set_flag(FLAG_CF, carry);
                    self.set_flag(FLAG_OF, overflow);
                    self.set_flag(FLAG_AF, af);
                    self.cycles += if modbits == 0b11 { 3 } else { 16 };
                    if modbits == 0b11 {
                        3
                    } else {
                        16
                    }
                } else {
                    // 16-bit operation
                    let reg_val = self.get_reg16(reg);
                    let (rm_val, seg, offset) = self.read_rmw16(modbits, rm);
                    let result = rm_val.wrapping_add(reg_val);
                    let carry = (rm_val as u32 + reg_val as u32) > 0xFFFF;
                    let overflow =
                        ((rm_val ^ (result as u16)) & (reg_val ^ (result as u16)) & 0x8000) != 0;
                    let af = Self::calc_af_add_16(rm_val, reg_val);

                    self.write_rmw16(modbits, rm, result, seg, offset);
                    self.update_flags_16(result);
                    self.set_flag(FLAG_CF, carry);
                    self.set_flag(FLAG_OF, overflow);
                    self.set_flag(FLAG_AF, af);
                    self.cycles += if modbits == 0b11 { 3 } else { 16 };
                    if modbits == 0b11 {
                        3
                    } else {
                        16
                    }
                }
            }

            // ADD r8, r/m8 (0x02)
            0x02 => {
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);
                let reg_val = if reg < 4 {
                    self.get_reg8_low(reg)
                } else {
                    self.get_reg8_high(reg - 4)
                };
                let rm_val = self.read_rm8(modbits, rm);
                let result = reg_val.wrapping_add(rm_val);
                let carry = (reg_val as u16 + rm_val as u16) > 0xFF;
                let overflow = ((reg_val ^ result) & (rm_val ^ result) & 0x80) != 0;
                let af = Self::calc_af_add_8(reg_val, rm_val);

                if reg < 4 {
                    self.set_reg8_low(reg, result);
                } else {
                    self.set_reg8_high(reg - 4, result);
                }
                self.update_flags_8(result);
                self.set_flag(FLAG_CF, carry);
                self.set_flag(FLAG_OF, overflow);
                self.set_flag(FLAG_AF, af);
                self.cycles += if modbits == 0b11 { 3 } else { 9 };
                if modbits == 0b11 {
                    3
                } else {
                    9
                }
            }

            // ADD r16/32, r/m16/32 (0x03)
            0x03 => {
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);

                if self.operand_size_override && self.model.supports_80386_instructions() {
                    // 32-bit operation
                    let reg_val = self.get_reg32(reg);
                    let rm_val = self.read_rm32(modbits, rm);
                    let result = reg_val.wrapping_add(rm_val);
                    let carry = (reg_val as u64 + rm_val as u64) > 0xFFFFFFFF;
                    let overflow = ((reg_val ^ result) & (rm_val ^ result) & 0x80000000) != 0;
                    let af = Self::calc_af_add_32(reg_val, rm_val);

                    self.set_reg32(reg, result);
                    self.update_flags_32(result);
                    self.set_flag(FLAG_CF, carry);
                    self.set_flag(FLAG_OF, overflow);
                    self.set_flag(FLAG_AF, af);
                    self.cycles += if modbits == 0b11 { 3 } else { 9 };
                    if modbits == 0b11 {
                        3
                    } else {
                        9
                    }
                } else {
                    // 16-bit operation
                    let reg_val = self.get_reg16(reg);
                    let rm_val = self.read_rm16(modbits, rm);
                    let result = reg_val.wrapping_add(rm_val);
                    let carry = (reg_val as u32 + rm_val as u32) > 0xFFFF;
                    let overflow =
                        ((reg_val ^ (result as u16)) & (rm_val ^ (result as u16)) & 0x8000) != 0;
                    let af = Self::calc_af_add_16(reg_val, rm_val);

                    self.set_reg16(reg, result);
                    self.update_flags_16(result);
                    self.set_flag(FLAG_CF, carry);
                    self.set_flag(FLAG_OF, overflow);
                    self.set_flag(FLAG_AF, af);
                    self.cycles += if modbits == 0b11 { 3 } else { 9 };
                    if modbits == 0b11 {
                        3
                    } else {
                        9
                    }
                }
            }

            // SUB r/m8, r8 (0x28)
            0x28 => {
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);
                let reg_val = if reg < 4 {
                    self.get_reg8_low(reg)
                } else {
                    self.get_reg8_high(reg - 4)
                };

                // Use RMW helpers to avoid double-fetching EA
                let (rm_val, seg, offset) = self.read_rmw8(modbits, rm);
                let result = rm_val.wrapping_sub(reg_val);
                let borrow = (rm_val as u16) < (reg_val as u16);
                let overflow = ((rm_val ^ reg_val) & (rm_val ^ result) & 0x80) != 0;
                let af = Self::calc_af_sub_8(rm_val, reg_val);

                self.write_rmw8(modbits, rm, result, seg, offset);
                self.update_flags_8(result);
                self.set_flag(FLAG_CF, borrow);
                self.set_flag(FLAG_OF, overflow);
                self.set_flag(FLAG_AF, af);
                self.cycles += if modbits == 0b11 { 3 } else { 16 };
                if modbits == 0b11 {
                    3
                } else {
                    16
                }
            }

            // SUB r/m16, r16 (0x29)
            0x29 => {
                // SUB r/m16/32, r16/32
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);

                if self.operand_size_override && self.model.supports_80386_instructions() {
                    // 32-bit operation
                    let reg_val = self.get_reg32(reg);
                    let (rm_val, seg, offset) = self.read_rmw32(modbits, rm);
                    let result = rm_val.wrapping_sub(reg_val);
                    let borrow = (rm_val as u64) < (reg_val as u64);
                    let overflow = ((rm_val ^ reg_val) & (rm_val ^ result) & 0x80000000) != 0;
                    let af = Self::calc_af_sub_32(rm_val, reg_val);

                    self.write_rmw32(modbits, rm, result, seg, offset);
                    self.update_flags_32(result);
                    self.set_flag(FLAG_CF, borrow);
                    self.set_flag(FLAG_OF, overflow);
                    self.set_flag(FLAG_AF, af);
                    self.cycles += if modbits == 0b11 { 3 } else { 16 };
                    if modbits == 0b11 {
                        3
                    } else {
                        16
                    }
                } else {
                    // 16-bit operation
                    let reg_val = self.get_reg16(reg);
                    let (rm_val, seg, offset) = self.read_rmw16(modbits, rm);
                    let result = rm_val.wrapping_sub(reg_val);
                    let borrow = (rm_val as u32) < (reg_val as u32);
                    let overflow = ((rm_val ^ reg_val) & (rm_val ^ (result as u16)) & 0x8000) != 0;
                    let af = Self::calc_af_sub_16(rm_val, reg_val);

                    self.write_rmw16(modbits, rm, result, seg, offset);
                    self.update_flags_16(result);
                    self.set_flag(FLAG_CF, borrow);
                    self.set_flag(FLAG_OF, overflow);
                    self.set_flag(FLAG_AF, af);
                    self.cycles += if modbits == 0b11 { 3 } else { 16 };
                    if modbits == 0b11 {
                        3
                    } else {
                        16
                    }
                }
            }

            // SUB r8, r/m8 (0x2A)
            0x2A => {
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);
                let reg_val = if reg < 4 {
                    self.get_reg8_low(reg)
                } else {
                    self.get_reg8_high(reg - 4)
                };
                let rm_val = self.read_rm8(modbits, rm);
                let result = reg_val.wrapping_sub(rm_val);
                let borrow = (reg_val as u16) < (rm_val as u16);
                let overflow = ((reg_val ^ rm_val) & (reg_val ^ result) & 0x80) != 0;
                let af = Self::calc_af_sub_8(reg_val, rm_val);

                if reg < 4 {
                    self.set_reg8_low(reg, result);
                } else {
                    self.set_reg8_high(reg - 4, result);
                }
                self.update_flags_8(result);
                self.set_flag(FLAG_CF, borrow);
                self.set_flag(FLAG_OF, overflow);
                self.set_flag(FLAG_AF, af);
                self.cycles += if modbits == 0b11 { 3 } else { 9 };
                if modbits == 0b11 {
                    3
                } else {
                    9
                }
            }

            // SUB r16/32, r/m16/32 (0x2B)
            0x2B => {
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);

                if self.operand_size_override && self.model.supports_80386_instructions() {
                    // 32-bit operation
                    let reg_val = self.get_reg32(reg);
                    let rm_val = self.read_rm32(modbits, rm);
                    let result = reg_val.wrapping_sub(rm_val);
                    let borrow = (reg_val as u64) < (rm_val as u64);
                    let overflow = ((reg_val ^ rm_val) & (reg_val ^ result) & 0x80000000) != 0;
                    let af = Self::calc_af_sub_32(reg_val, rm_val);

                    self.set_reg32(reg, result);
                    self.update_flags_32(result);
                    self.set_flag(FLAG_CF, borrow);
                    self.set_flag(FLAG_OF, overflow);
                    self.set_flag(FLAG_AF, af);
                    self.cycles += if modbits == 0b11 { 3 } else { 9 };
                    if modbits == 0b11 {
                        3
                    } else {
                        9
                    }
                } else {
                    // 16-bit operation
                    let reg_val = self.get_reg16(reg);
                    let rm_val = self.read_rm16(modbits, rm);
                    let result = reg_val.wrapping_sub(rm_val);
                    let borrow = (reg_val as u32) < (rm_val as u32);
                    let overflow = ((reg_val ^ rm_val) & (reg_val ^ (result as u16)) & 0x8000) != 0;
                    let af = Self::calc_af_sub_16(reg_val, rm_val);

                    self.set_reg16(reg, result);
                    self.update_flags_16(result);
                    self.set_flag(FLAG_CF, borrow);
                    self.set_flag(FLAG_OF, overflow);
                    self.set_flag(FLAG_AF, af);
                    self.cycles += if modbits == 0b11 { 3 } else { 9 };
                    if modbits == 0b11 {
                        3
                    } else {
                        9
                    }
                }
            }

            // CMP r/m8, r8 (0x38)
            0x38 => {
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);
                let reg_val = if reg < 4 {
                    self.get_reg8_low(reg)
                } else {
                    self.get_reg8_high(reg - 4)
                };
                let rm_val = self.read_rm8(modbits, rm);
                let result = rm_val.wrapping_sub(reg_val);
                let borrow = (rm_val as u16) < (reg_val as u16);
                let overflow = ((rm_val ^ reg_val) & (rm_val ^ result) & 0x80) != 0;
                let af = Self::calc_af_sub_8(rm_val, reg_val);

                self.update_flags_8(result);
                self.set_flag(FLAG_CF, borrow);
                self.set_flag(FLAG_OF, overflow);
                self.set_flag(FLAG_AF, af);
                self.cycles += if modbits == 0b11 { 3 } else { 9 };
                if modbits == 0b11 {
                    3
                } else {
                    9
                }
            }

            // CMP r/m16, r16 (0x39)
            0x39 => {
                // CMP r/m16/32, r16/32
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);

                if self.operand_size_override && self.model.supports_80386_instructions() {
                    // 32-bit operation
                    let reg_val = self.get_reg32(reg);
                    let rm_val = self.read_rm32(modbits, rm);
                    let result = rm_val.wrapping_sub(reg_val);
                    let borrow = (rm_val as u64) < (reg_val as u64);
                    let overflow = ((rm_val ^ reg_val) & (rm_val ^ result) & 0x80000000) != 0;
                    let af = Self::calc_af_sub_32(rm_val, reg_val);

                    self.update_flags_32(result);
                    self.set_flag(FLAG_CF, borrow);
                    self.set_flag(FLAG_OF, overflow);
                    self.set_flag(FLAG_AF, af);
                    self.cycles += if modbits == 0b11 { 3 } else { 9 };
                    if modbits == 0b11 {
                        3
                    } else {
                        9
                    }
                } else {
                    // 16-bit operation
                    let reg_val = self.get_reg16(reg);
                    let rm_val = self.read_rm16(modbits, rm);
                    let result = rm_val.wrapping_sub(reg_val);
                    let borrow = (rm_val as u32) < (reg_val as u32);
                    let overflow = ((rm_val ^ reg_val) & (rm_val ^ (result as u16)) & 0x8000) != 0;
                    let af = Self::calc_af_sub_16(rm_val, reg_val);

                    self.update_flags_16(result as u16);
                    self.set_flag(FLAG_CF, borrow);
                    self.set_flag(FLAG_OF, overflow);
                    self.set_flag(FLAG_AF, af);
                    self.cycles += if modbits == 0b11 { 3 } else { 9 };
                    if modbits == 0b11 {
                        3
                    } else {
                        9
                    }
                }
            }

            // CMP r8, r/m8 (0x3A)
            0x3A => {
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);
                let reg_val = if reg < 4 {
                    self.get_reg8_low(reg)
                } else {
                    self.get_reg8_high(reg - 4)
                };
                let rm_val = self.read_rm8(modbits, rm);
                let result = reg_val.wrapping_sub(rm_val);
                let borrow = (reg_val as u16) < (rm_val as u16);
                let overflow = ((reg_val ^ rm_val) & (reg_val ^ result) & 0x80) != 0;
                let af = Self::calc_af_sub_8(reg_val, rm_val);

                self.update_flags_8(result);
                self.set_flag(FLAG_CF, borrow);
                self.set_flag(FLAG_OF, overflow);
                self.set_flag(FLAG_AF, af);
                self.cycles += if modbits == 0b11 { 3 } else { 9 };
                if modbits == 0b11 {
                    3
                } else {
                    9
                }
            }

            // CMP r16/32, r/m16/32 (0x3B)
            0x3B => {
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);

                if self.operand_size_override && self.model.supports_80386_instructions() {
                    // 32-bit operation
                    let reg_val = self.get_reg32(reg);
                    let rm_val = self.read_rm32(modbits, rm);
                    let result = reg_val.wrapping_sub(rm_val);
                    let borrow = (reg_val as u64) < (rm_val as u64);
                    let overflow = ((reg_val ^ rm_val) & (reg_val ^ result) & 0x80000000) != 0;
                    let af = Self::calc_af_sub_32(reg_val, rm_val);

                    self.update_flags_32(result);
                    self.set_flag(FLAG_CF, borrow);
                    self.set_flag(FLAG_OF, overflow);
                    self.set_flag(FLAG_AF, af);
                    self.cycles += if modbits == 0b11 { 3 } else { 9 };
                    if modbits == 0b11 {
                        3
                    } else {
                        9
                    }
                } else {
                    // 16-bit operation
                    let reg_val = self.get_reg16(reg);
                    let rm_val = self.read_rm16(modbits, rm);
                    let result = reg_val.wrapping_sub(rm_val);
                    let borrow = (reg_val as u32) < (rm_val as u32);
                    let overflow = ((reg_val ^ rm_val) & (reg_val ^ (result as u16)) & 0x8000) != 0;
                    let af = Self::calc_af_sub_16(reg_val, rm_val);

                    self.update_flags_16(result as u16);
                    self.set_flag(FLAG_CF, borrow);
                    self.set_flag(FLAG_OF, overflow);
                    self.set_flag(FLAG_AF, af);
                    self.cycles += if modbits == 0b11 { 3 } else { 9 };
                    if modbits == 0b11 {
                        3
                    } else {
                        9
                    }
                }
            }

            // ADD AL, imm8
            0x04 => {
                let val = self.fetch_u8();
                let al = (self.ax & 0xFF) as u8;
                let result = al.wrapping_add(val);
                let carry = (al as u16 + val as u16) > 0xFF;
                let overflow = ((al ^ result) & (val ^ result) & 0x80) != 0;
                let af = Self::calc_af_add_8(al, val);

                self.ax = (self.ax & 0xFFFF_FF00) | (result as u32);
                self.update_flags_8(result);
                self.set_flag(FLAG_CF, carry);
                self.set_flag(FLAG_OF, overflow);
                self.set_flag(FLAG_AF, af);
                self.cycles += 4;
                4
            }

            // ADD AX, imm16
            0x05 => {
                let val = self.fetch_u16();
                let result = (self.ax as u16).wrapping_add(val);
                let carry = (self.ax as u32 + val as u32) > 0xFFFF;
                let overflow =
                    (((self.ax as u16) ^ (result as u16)) & (val ^ (result as u16)) & 0x8000) != 0;
                let af = Self::calc_af_add_16(self.ax as u16, val);

                self.ax = (self.ax & 0xFFFF_0000) | (result as u32);
                self.update_flags_16(result);
                self.set_flag(FLAG_CF, carry);
                self.set_flag(FLAG_OF, overflow);
                self.set_flag(FLAG_AF, af);
                self.cycles += 4;
                4
            }

            // PUSH ES (0x06)
            0x06 => {
                self.push(self.es);
                self.cycles += 10;
                10
            }

            // POP ES (0x07)
            0x07 => {
                self.es = self.pop();
                self.cycles += 8;
                8
            }

            // OR r/m8, r8 (0x08)
            0x08 => {
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);
                let reg_val = if reg < 4 {
                    self.get_reg8_low(reg)
                } else {
                    self.get_reg8_high(reg - 4)
                };
                let (rm_val, seg, offset) = self.read_rmw8(modbits, rm);
                let result = rm_val | reg_val;

                self.write_rmw8(modbits, rm, result, seg, offset);
                self.update_flags_8(result);
                self.set_flag(FLAG_CF, false);
                self.set_flag(FLAG_OF, false);
                self.cycles += if modbits == 0b11 { 3 } else { 16 };
                if modbits == 0b11 {
                    3
                } else {
                    16
                }
            }

            // OR r/m16, r16 (0x09)
            0x09 => {
                // OR r/m16/32, r16/32
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);

                if self.operand_size_override && self.model.supports_80386_instructions() {
                    // 32-bit operation
                    let reg_val = self.get_reg32(reg);
                    let (rm_val, seg, offset) = self.read_rmw32(modbits, rm);
                    let result = rm_val | reg_val;

                    self.write_rmw32(modbits, rm, result, seg, offset);
                    self.update_flags_32(result);
                    self.set_flag(FLAG_CF, false);
                    self.set_flag(FLAG_OF, false);
                    self.cycles += if modbits == 0b11 { 3 } else { 16 };
                    if modbits == 0b11 {
                        3
                    } else {
                        16
                    }
                } else {
                    // 16-bit operation
                    let reg_val = self.get_reg16(reg);
                    let (rm_val, seg, offset) = self.read_rmw16(modbits, rm);
                    let result = rm_val | reg_val;

                    self.write_rmw16(modbits, rm, result, seg, offset);
                    self.update_flags_16(result as u16);
                    self.set_flag(FLAG_CF, false);
                    self.set_flag(FLAG_OF, false);
                    self.cycles += if modbits == 0b11 { 3 } else { 16 };
                    if modbits == 0b11 {
                        3
                    } else {
                        16
                    }
                }
            }

            // OR r8, r/m8 (0x0A)
            0x0A => {
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);
                let reg_val = if reg < 4 {
                    self.get_reg8_low(reg)
                } else {
                    self.get_reg8_high(reg - 4)
                };
                let rm_val = self.read_rm8(modbits, rm);
                let result = reg_val | rm_val;

                if reg < 4 {
                    self.set_reg8_low(reg, result);
                } else {
                    self.set_reg8_high(reg - 4, result);
                }
                self.update_flags_8(result);
                self.set_flag(FLAG_CF, false);
                self.set_flag(FLAG_OF, false);
                self.cycles += if modbits == 0b11 { 3 } else { 9 };
                if modbits == 0b11 {
                    3
                } else {
                    9
                }
            }

            // OR r16/32, r/m16/32 (0x0B)
            0x0B => {
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);

                if self.operand_size_override && self.model.supports_80386_instructions() {
                    // 32-bit operation
                    let reg_val = self.get_reg32(reg);
                    let rm_val = self.read_rm32(modbits, rm);
                    let result = reg_val | rm_val;

                    self.set_reg32(reg, result);
                    self.update_flags_32(result);
                    self.set_flag(FLAG_CF, false);
                    self.set_flag(FLAG_OF, false);
                    self.cycles += if modbits == 0b11 { 3 } else { 9 };
                    if modbits == 0b11 {
                        3
                    } else {
                        9
                    }
                } else {
                    // 16-bit operation
                    let reg_val = self.get_reg16(reg);
                    let rm_val = self.read_rm16(modbits, rm);
                    let result = reg_val | rm_val;

                    self.set_reg16(reg, result);
                    self.update_flags_16(result as u16);
                    self.set_flag(FLAG_CF, false);
                    self.set_flag(FLAG_OF, false);
                    self.cycles += if modbits == 0b11 { 3 } else { 9 };
                    if modbits == 0b11 {
                        3
                    } else {
                        9
                    }
                }
            }

            // AND AL, imm8 (0x24) is already implemented

            // ES segment override prefix (0x26)
            0x26 => {
                // ES segment override prefix
                self.segment_override = Some(SegmentOverride::ES);
                self.step() // Execute next instruction with ES override
            }

            // DAA - Decimal Adjust After Addition (0x27)
            0x27 => {
                let mut al = (self.ax & 0xFF) as u8;
                let old_al = al;
                let old_cf = self.get_flag(FLAG_CF);

                if (al & 0x0F) > 9 || self.get_flag(FLAG_AF) {
                    al = al.wrapping_add(6u8);
                    self.set_flag(FLAG_AF, true);
                } else {
                    self.set_flag(FLAG_AF, false);
                }

                if old_al > 0x99 || old_cf {
                    al = al.wrapping_add(0x60);
                    self.set_flag(FLAG_CF, true);
                } else {
                    self.set_flag(FLAG_CF, false);
                }

                self.ax = (self.ax & 0xFFFF_FF00) | (al as u32);
                self.update_flags_8(al);
                self.cycles += 4;
                4
            }

            // CS segment override prefix (0x2E)
            0x2E => {
                // CS segment override prefix
                self.segment_override = Some(SegmentOverride::CS);
                self.step() // Execute next instruction with CS override
            }

            // DAS - Decimal Adjust After Subtraction (0x2F)
            0x2F => {
                let mut al = (self.ax & 0xFF) as u8;
                let old_al = al;
                let old_cf = self.get_flag(FLAG_CF);

                if (al & 0x0F) > 9 || self.get_flag(FLAG_AF) {
                    al = al.wrapping_sub(6u8);
                    self.set_flag(FLAG_AF, true);
                } else {
                    self.set_flag(FLAG_AF, false);
                }

                if old_al > 0x99 || old_cf {
                    al = al.wrapping_sub(0x60);
                    self.set_flag(FLAG_CF, true);
                } else {
                    self.set_flag(FLAG_CF, false);
                }

                self.ax = (self.ax & 0xFFFF_FF00) | (al as u32);
                self.update_flags_8(al);
                self.cycles += 4;
                4
            }

            // XOR r/m8, r8 (0x30)
            0x30 => {
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);
                let reg_val = if reg < 4 {
                    self.get_reg8_low(reg)
                } else {
                    self.get_reg8_high(reg - 4)
                };
                let (rm_val, seg, offset) = self.read_rmw8(modbits, rm);
                let result = rm_val ^ reg_val;

                self.write_rmw8(modbits, rm, result, seg, offset);
                self.update_flags_8(result);
                self.set_flag(FLAG_CF, false);
                self.set_flag(FLAG_OF, false);
                self.cycles += if modbits == 0b11 { 3 } else { 16 };
                if modbits == 0b11 {
                    3
                } else {
                    16
                }
            }

            // XOR r/m16, r16 (0x31)
            0x31 => {
                // XOR r/m16/32, r16/32
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);

                if self.operand_size_override && self.model.supports_80386_instructions() {
                    // 32-bit operation
                    let reg_val = self.get_reg32(reg);
                    let (rm_val, seg, offset) = self.read_rmw32(modbits, rm);
                    let result = rm_val ^ reg_val;

                    self.write_rmw32(modbits, rm, result, seg, offset);
                    self.update_flags_32(result);
                    self.set_flag(FLAG_CF, false);
                    self.set_flag(FLAG_OF, false);
                    self.cycles += if modbits == 0b11 { 3 } else { 16 };
                    if modbits == 0b11 {
                        3
                    } else {
                        16
                    }
                } else {
                    // 16-bit operation
                    let reg_val = self.get_reg16(reg);
                    let (rm_val, seg, offset) = self.read_rmw16(modbits, rm);
                    let result = rm_val ^ reg_val;

                    self.write_rmw16(modbits, rm, result, seg, offset);
                    self.update_flags_16(result as u16);
                    self.set_flag(FLAG_CF, false);
                    self.set_flag(FLAG_OF, false);
                    self.cycles += if modbits == 0b11 { 3 } else { 16 };
                    if modbits == 0b11 {
                        3
                    } else {
                        16
                    }
                }
            }

            // XOR r8, r/m8 (0x32)
            0x32 => {
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);
                let reg_val = if reg < 4 {
                    self.get_reg8_low(reg)
                } else {
                    self.get_reg8_high(reg - 4)
                };
                let rm_val = self.read_rm8(modbits, rm);
                let result = reg_val ^ rm_val;

                if reg < 4 {
                    self.set_reg8_low(reg, result);
                } else {
                    self.set_reg8_high(reg - 4, result);
                }
                self.update_flags_8(result);
                self.set_flag(FLAG_CF, false);
                self.set_flag(FLAG_OF, false);
                self.cycles += if modbits == 0b11 { 3 } else { 9 };
                if modbits == 0b11 {
                    3
                } else {
                    9
                }
            }

            // XOR r16/32, r/m16/32 (0x33)
            0x33 => {
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);

                if self.operand_size_override && self.model.supports_80386_instructions() {
                    // 32-bit operation
                    let reg_val = self.get_reg32(reg);
                    let rm_val = self.read_rm32(modbits, rm);
                    let result = reg_val ^ rm_val;

                    self.set_reg32(reg, result);
                    self.update_flags_32(result);
                    self.set_flag(FLAG_CF, false);
                    self.set_flag(FLAG_OF, false);
                    self.cycles += if modbits == 0b11 { 3 } else { 9 };
                    if modbits == 0b11 {
                        3
                    } else {
                        9
                    }
                } else {
                    // 16-bit operation
                    let reg_val = self.get_reg16(reg);
                    let rm_val = self.read_rm16(modbits, rm);
                    let result = reg_val ^ rm_val;

                    self.set_reg16(reg, result);
                    self.update_flags_16(result as u16);
                    self.set_flag(FLAG_CF, false);
                    self.set_flag(FLAG_OF, false);
                    self.cycles += if modbits == 0b11 { 3 } else { 9 };
                    if modbits == 0b11 {
                        3
                    } else {
                        9
                    }
                }
            }

            // SUB AL, imm8
            0x2C => {
                let val = self.fetch_u8();
                let al = (self.ax & 0xFF) as u8;
                let result = al.wrapping_sub(val);
                let borrow = (al as u16) < (val as u16);
                let overflow = ((al ^ val) & (al ^ result) & 0x80) != 0;
                let af = Self::calc_af_sub_8(al, val);

                self.ax = (self.ax & 0xFFFF_FF00) | (result as u32);
                self.update_flags_8(result);
                self.set_flag(FLAG_CF, borrow);
                self.set_flag(FLAG_OF, overflow);
                self.set_flag(FLAG_AF, af);
                self.cycles += 4;
                4
            }

            // SUB AX, imm16
            0x2D => {
                let val = self.fetch_u16();
                let result = (self.ax as u16).wrapping_sub(val);
                let borrow = self.ax < (val as u32);
                let overflow =
                    (((self.ax as u16) ^ val) & ((self.ax as u16) ^ (result as u16)) & 0x8000) != 0;
                let af = Self::calc_af_sub_16(self.ax as u16, val);

                self.ax = (self.ax & 0xFFFF_0000) | (result as u32);
                self.update_flags_16(result);
                self.set_flag(FLAG_CF, borrow);
                self.set_flag(FLAG_OF, overflow);
                self.set_flag(FLAG_AF, af);
                self.cycles += 4;
                4
            }

            // CMP AL, imm8
            0x3C => {
                let val = self.fetch_u8();
                let al = (self.ax & 0xFF) as u8;
                let result = al.wrapping_sub(val);
                let borrow = (al as u16) < (val as u16);
                let overflow = ((al ^ val) & (al ^ result) & 0x80) != 0;
                let af = Self::calc_af_sub_8(al, val);

                self.update_flags_8(result);
                self.set_flag(FLAG_CF, borrow);
                self.set_flag(FLAG_OF, overflow);
                self.set_flag(FLAG_AF, af);
                self.cycles += 4;
                4
            }

            // CMP AX, imm16
            0x3D => {
                let val = self.fetch_u16();
                let result = (self.ax as u16).wrapping_sub(val);
                let borrow = self.ax < (val as u32);
                let overflow =
                    (((self.ax as u16) ^ val) & ((self.ax as u16) ^ (result as u16)) & 0x8000) != 0;
                let af = Self::calc_af_sub_16(self.ax as u16, val);

                self.update_flags_16(result);
                self.set_flag(FLAG_CF, borrow);
                self.set_flag(FLAG_OF, overflow);
                self.set_flag(FLAG_AF, af);
                self.cycles += 4;
                4
            }

            // AND AL, imm8
            0x24 => {
                let val = self.fetch_u8();
                let al = (self.ax & 0xFF) as u8;
                let result = al & val;

                self.ax = (self.ax & 0xFFFF_FF00) | (result as u32);
                self.update_flags_8(result);
                self.set_flag(FLAG_CF, false);
                self.set_flag(FLAG_OF, false);
                self.cycles += 4;
                4
            }

            // AND AX, imm16
            0x25 => {
                let val = self.fetch_u16();
                let result = (self.ax as u16) & val;

                self.ax = (self.ax & 0xFFFF_0000) | (result as u32);
                self.update_flags_16(result);
                self.set_flag(FLAG_CF, false);
                self.set_flag(FLAG_OF, false);
                self.cycles += 4;
                4
            }

            // OR AL, imm8
            0x0C => {
                let val = self.fetch_u8();
                let al = (self.ax & 0xFF) as u8;
                let result = al | val;

                self.ax = (self.ax & 0xFFFF_FF00) | (result as u32);
                self.update_flags_8(result);
                self.set_flag(FLAG_CF, false);
                self.set_flag(FLAG_OF, false);
                self.cycles += 4;
                4
            }

            // OR AX, imm16
            0x0D => {
                let val = self.fetch_u16();
                let result = (self.ax as u16) | val;

                self.ax = (self.ax & 0xFFFF_0000) | (result as u32);
                self.update_flags_16(result);
                self.set_flag(FLAG_CF, false);
                self.set_flag(FLAG_OF, false);
                self.cycles += 4;
                4
            }

            // PUSH CS (0x0E)
            0x0E => {
                self.push(self.cs);
                self.cycles += 10;
                10
            }

            // Two-byte opcode prefix (0x0F) - 80286+ instructions
            0x0F => {
                let next_opcode = self.fetch_u8();
                match next_opcode {
                    // Group 7 instructions (0x0F 0x01) - 80286+
                    0x01 => {
                        if !self.model.supports_80286_instructions() {
                            // Invalid opcode on 8086/80186
                            self.cycles += 10;
                            return 10;
                        }

                        let modrm = self.fetch_u8();
                        let (modbits, reg, rm) = Self::decode_modrm(modrm);

                        match reg {
                            0 => {
                                // SGDT - Store Global Descriptor Table Register
                                // Stores GDTR to memory (6 bytes: 2-byte limit, 4-byte base)
                                let (segment, offset, _) = self.calc_effective_address(modbits, rm);
                                let limit = self.protected_mode.gdtr.limit;
                                let base = self.protected_mode.gdtr.base;

                                self.write(segment, offset, (limit & 0xFF) as u8);
                                self.write(segment, offset.wrapping_add(1), (limit >> 8) as u8);
                                self.write(segment, offset.wrapping_add(2), (base & 0xFF) as u8);
                                self.write(
                                    segment,
                                    offset.wrapping_add(3),
                                    ((base >> 8) & 0xFF) as u8,
                                );
                                self.write(
                                    segment,
                                    offset.wrapping_add(4),
                                    ((base >> 16) & 0xFF) as u8,
                                );
                                self.write(
                                    segment,
                                    offset.wrapping_add(5),
                                    ((base >> 24) & 0xFF) as u8,
                                );
                                self.cycles += 11;
                                11
                            }
                            1 => {
                                // SIDT - Store Interrupt Descriptor Table Register
                                let (segment, offset, _) = self.calc_effective_address(modbits, rm);
                                let limit = self.protected_mode.idtr.limit;
                                let base = self.protected_mode.idtr.base;

                                self.write(segment, offset, (limit & 0xFF) as u8);
                                self.write(segment, offset.wrapping_add(1), (limit >> 8) as u8);
                                self.write(segment, offset.wrapping_add(2), (base & 0xFF) as u8);
                                self.write(
                                    segment,
                                    offset.wrapping_add(3),
                                    ((base >> 8) & 0xFF) as u8,
                                );
                                self.write(
                                    segment,
                                    offset.wrapping_add(4),
                                    ((base >> 16) & 0xFF) as u8,
                                );
                                self.write(
                                    segment,
                                    offset.wrapping_add(5),
                                    ((base >> 24) & 0xFF) as u8,
                                );
                                self.cycles += 11;
                                11
                            }
                            2 => {
                                // LGDT - Load Global Descriptor Table Register
                                let (segment, offset, _) = self.calc_effective_address(modbits, rm);
                                let limit_low = self.read(segment, offset) as u16;
                                let limit_high = self.read(segment, offset.wrapping_add(1)) as u16;
                                let limit = limit_low | (limit_high << 8);

                                let base_0 = self.read(segment, offset.wrapping_add(2)) as u32;
                                let base_1 = self.read(segment, offset.wrapping_add(3)) as u32;
                                let base_2 = self.read(segment, offset.wrapping_add(4)) as u32;
                                let base_3 = self.read(segment, offset.wrapping_add(5)) as u32;
                                let base = base_0 | (base_1 << 8) | (base_2 << 16) | (base_3 << 24);

                                self.protected_mode.load_gdtr(base, limit);
                                self.cycles += 11;
                                11
                            }
                            3 => {
                                // LIDT - Load Interrupt Descriptor Table Register
                                let (segment, offset, _) = self.calc_effective_address(modbits, rm);
                                let limit_low = self.read(segment, offset) as u16;
                                let limit_high = self.read(segment, offset.wrapping_add(1)) as u16;
                                let limit = limit_low | (limit_high << 8);

                                let base_0 = self.read(segment, offset.wrapping_add(2)) as u32;
                                let base_1 = self.read(segment, offset.wrapping_add(3)) as u32;
                                let base_2 = self.read(segment, offset.wrapping_add(4)) as u32;
                                let base_3 = self.read(segment, offset.wrapping_add(5)) as u32;
                                let base = base_0 | (base_1 << 8) | (base_2 << 16) | (base_3 << 24);

                                self.protected_mode.load_idtr(base, limit);
                                self.cycles += 11;
                                11
                            }
                            4 => {
                                // SMSW - Store Machine Status Word
                                let msw = self.protected_mode.get_msw();
                                self.write_rm16(modbits, rm, msw);
                                self.cycles += 3;
                                3
                            }
                            6 => {
                                // LMSW - Load Machine Status Word
                                let val = self.read_rm16(modbits, rm);
                                self.protected_mode.set_msw(val);
                                self.cycles += 10;
                                10
                            }
                            7 => {
                                // INVLPG - Invalidate TLB Entry (80486+)
                                // Stub: No TLB implementation
                                self.cycles += 25;
                                25
                            }
                            _ => {
                                // Reserved
                                self.cycles += 10;
                                10
                            }
                        }
                    }
                    // LAR - Load Access Rights (0x0F 0x02) - 80286+
                    0x02 => {
                        if !self.model.supports_80286_instructions() {
                            self.cycles += 15;
                            return 15;
                        }

                        let modrm = self.fetch_u8();
                        let (modbits, reg, rm) = Self::decode_modrm(modrm);
                        let _selector = self.read_rm16(modbits, rm);

                        // Stub implementation: Set ZF=0 (invalid selector)
                        // In a full implementation, this would:
                        // 1. Check if selector is valid
                        // 2. Load access rights from descriptor
                        // 3. Set ZF=1 if valid, store access rights in destination
                        self.set_flag(FLAG_ZF, false);
                        self.set_reg16(reg, 0);

                        self.cycles += 15;
                        15
                    }
                    // LSL - Load Segment Limit (0x0F 0x03) - 80286+
                    0x03 => {
                        if !self.model.supports_80286_instructions() {
                            self.cycles += 15;
                            return 15;
                        }

                        let modrm = self.fetch_u8();
                        let (modbits, reg, rm) = Self::decode_modrm(modrm);
                        let _selector = self.read_rm16(modbits, rm);

                        // Stub implementation: Set ZF=0 (invalid selector)
                        self.set_flag(FLAG_ZF, false);
                        self.set_reg16(reg, 0);

                        self.cycles += 15;
                        15
                    }
                    // CLTS - Clear Task Switched Flag (0x0F 0x06) - 80286+
                    0x06 => {
                        if !self.model.supports_80286_instructions() {
                            self.cycles += 2;
                            return 2;
                        }

                        // Clear TS bit (bit 3) in MSW/CR0
                        let msw = self.protected_mode.get_msw();
                        self.protected_mode.set_msw(msw & !0x0008);
                        self.cycles += 2;
                        2
                    }
                    // Group 6 instructions (0x0F 0x00) - 80286+ descriptor table operations
                    0x00 => {
                        if !self.model.supports_80286_instructions() {
                            self.cycles += 10;
                            return 10;
                        }

                        let modrm = self.fetch_u8();
                        let (modbits, reg, rm) = Self::decode_modrm(modrm);

                        match reg {
                            0 => {
                                // SLDT - Store Local Descriptor Table Register
                                let ldtr = self.protected_mode.ldtr;
                                self.write_rm16(modbits, rm, ldtr);
                                self.cycles += 3;
                                3
                            }
                            1 => {
                                // STR - Store Task Register
                                let tr = self.protected_mode.tr;
                                self.write_rm16(modbits, rm, tr);
                                self.cycles += 3;
                                3
                            }
                            2 => {
                                // LLDT - Load Local Descriptor Table Register
                                let selector = self.read_rm16(modbits, rm);
                                self.protected_mode.load_ldtr(selector);
                                self.cycles += 17;
                                17
                            }
                            3 => {
                                // LTR - Load Task Register
                                let selector = self.read_rm16(modbits, rm);
                                self.protected_mode.load_tr(selector);
                                self.cycles += 17;
                                17
                            }
                            4 => {
                                // VERR - Verify Segment for Reading
                                let _selector = self.read_rm16(modbits, rm);
                                // Stub: Set ZF=0 (segment not readable)
                                // In full implementation: check descriptor access rights
                                self.set_flag(FLAG_ZF, false);
                                self.cycles += 10;
                                10
                            }
                            5 => {
                                // VERW - Verify Segment for Writing
                                let _selector = self.read_rm16(modbits, rm);
                                // Stub: Set ZF=0 (segment not writable)
                                // In full implementation: check descriptor access rights
                                self.set_flag(FLAG_ZF, false);
                                self.cycles += 10;
                                10
                            }
                            _ => {
                                // Reserved
                                self.cycles += 10;
                                10
                            }
                        }
                    }
                    // MOVSX - Move with Sign Extension (0x0F 0xBE, 0xBF) - 80386+
                    0xBE => {
                        if !self.model.supports_80386_instructions() {
                            // Invalid opcode on 8086/8088/80186/80286
                            self.cycles += 10;
                            return 10;
                        }
                        // MOVSX r16, r/m8
                        let modrm = self.fetch_u8();
                        let (modbits, reg, rm) = Self::decode_modrm(modrm);
                        let val = self.read_rm8(modbits, rm);
                        let extended = (val as i8) as i16 as u16; // Sign extend
                        self.set_reg16(reg, extended);
                        self.cycles += if modbits == 0b11 { 3 } else { 6 };
                        if modbits == 0b11 {
                            3
                        } else {
                            6
                        }
                    }
                    0xBF => {
                        if !self.model.supports_80386_instructions() {
                            // Invalid opcode on 8086/8088/80186/80286
                            self.cycles += 10;
                            return 10;
                        }
                        // MOVSX r32, r/m16 (80386 only - not fully supported yet)
                        let _modrm = self.fetch_u8();
                        self.cycles += 3;
                        3
                    }
                    // MOVZX - Move with Zero Extension (0x0F 0xB6, 0xB7) - 80386+
                    0xB6 => {
                        if !self.model.supports_80386_instructions() {
                            // Invalid opcode on 8086/8088/80186/80286
                            self.cycles += 10;
                            return 10;
                        }
                        // MOVZX r16, r/m8
                        let modrm = self.fetch_u8();
                        let (modbits, reg, rm) = Self::decode_modrm(modrm);
                        let val = self.read_rm8(modbits, rm);
                        self.set_reg16(reg, val as u16); // Zero extend
                        self.cycles += if modbits == 0b11 { 3 } else { 6 };
                        if modbits == 0b11 {
                            3
                        } else {
                            6
                        }
                    }
                    0xB7 => {
                        if !self.model.supports_80386_instructions() {
                            // Invalid opcode on 8086/8088/80186/80286
                            self.cycles += 10;
                            return 10;
                        }
                        // MOVZX r32, r/m16 (80386 only - not fully supported yet)
                        let _modrm = self.fetch_u8();
                        self.cycles += 3;
                        3
                    }
                    // BSF - Bit Scan Forward (0x0F 0xBC) - 80386+
                    0xBC => {
                        if !self.model.supports_80386_instructions() {
                            // Invalid opcode on 8086/8088/80186/80286
                            self.cycles += 10;
                            return 10;
                        }
                        let modrm = self.fetch_u8();
                        let (modbits, reg, rm) = Self::decode_modrm(modrm);
                        let val = self.read_rm16(modbits, rm);
                        if val == 0 {
                            // ZF = 1 if source is 0
                            self.set_flag(FLAG_ZF, true);
                        } else {
                            // Find first set bit from LSB
                            let bit_pos = val.trailing_zeros() as u16;
                            self.set_reg16(reg, bit_pos);
                            self.set_flag(FLAG_ZF, false);
                        }
                        self.cycles += if modbits == 0b11 { 10 } else { 11 };
                        if modbits == 0b11 {
                            10
                        } else {
                            11
                        }
                    }
                    // BSR - Bit Scan Reverse (0x0F 0xBD) - 80386+
                    0xBD => {
                        if !self.model.supports_80386_instructions() {
                            // Invalid opcode on 8086/8088/80186/80286
                            self.cycles += 10;
                            return 10;
                        }
                        let modrm = self.fetch_u8();
                        let (modbits, reg, rm) = Self::decode_modrm(modrm);
                        let val = self.read_rm16(modbits, rm);
                        if val == 0 {
                            // ZF = 1 if source is 0
                            self.set_flag(FLAG_ZF, true);
                        } else {
                            // Find first set bit from MSB
                            let bit_pos = 15 - val.leading_zeros() as u16;
                            self.set_reg16(reg, bit_pos);
                            self.set_flag(FLAG_ZF, false);
                        }
                        self.cycles += if modbits == 0b11 { 10 } else { 11 };
                        if modbits == 0b11 {
                            10
                        } else {
                            11
                        }
                    }
                    // BT - Bit Test (0x0F 0xA3) - 80386+
                    0xA3 => {
                        if !self.model.supports_80386_instructions() {
                            // Invalid opcode on 8086/8088/80186/80286
                            self.cycles += 10;
                            return 10;
                        }
                        let modrm = self.fetch_u8();
                        let (modbits, reg, rm) = Self::decode_modrm(modrm);
                        let bit_index = self.get_reg16(reg);
                        let val = self.read_rm16(modbits, rm);
                        let bit = (val >> (bit_index & 0x0F)) & 1;
                        self.set_flag(FLAG_CF, bit != 0);
                        self.cycles += if modbits == 0b11 { 3 } else { 12 };
                        if modbits == 0b11 {
                            3
                        } else {
                            12
                        }
                    }
                    // BTS - Bit Test and Set (0x0F 0xAB) - 80386+
                    0xAB => {
                        if !self.model.supports_80386_instructions() {
                            // Invalid opcode on 8086/8088/80186/80286
                            self.cycles += 10;
                            return 10;
                        }
                        let modrm = self.fetch_u8();
                        let (modbits, reg, rm) = Self::decode_modrm(modrm);
                        let bit_index = self.get_reg16(reg);
                        let val = self.read_rm16(modbits, rm);
                        let bit = (val >> (bit_index & 0x0F)) & 1;
                        self.set_flag(FLAG_CF, bit != 0);
                        let new_val = val | (1 << (bit_index & 0x0F));
                        self.write_rm16(modbits, rm, new_val);
                        self.cycles += if modbits == 0b11 { 6 } else { 13 };
                        if modbits == 0b11 {
                            6
                        } else {
                            13
                        }
                    }
                    // BTR - Bit Test and Reset (0x0F 0xB3) - 80386+
                    0xB3 => {
                        if !self.model.supports_80386_instructions() {
                            // Invalid opcode on 8086/8088/80186/80286
                            self.cycles += 10;
                            return 10;
                        }
                        let modrm = self.fetch_u8();
                        let (modbits, reg, rm) = Self::decode_modrm(modrm);
                        let bit_index = self.get_reg16(reg);
                        let val = self.read_rm16(modbits, rm);
                        let bit = (val >> (bit_index & 0x0F)) & 1;
                        self.set_flag(FLAG_CF, bit != 0);
                        let new_val = val & !(1 << (bit_index & 0x0F));
                        self.write_rm16(modbits, rm, new_val);
                        self.cycles += if modbits == 0b11 { 6 } else { 13 };
                        if modbits == 0b11 {
                            6
                        } else {
                            13
                        }
                    }
                    // BTC - Bit Test and Complement (0x0F 0xBB) - 80386+
                    0xBB => {
                        if !self.model.supports_80386_instructions() {
                            // Invalid opcode on 8086/8088/80186/80286
                            self.cycles += 10;
                            return 10;
                        }
                        let modrm = self.fetch_u8();
                        let (modbits, reg, rm) = Self::decode_modrm(modrm);
                        let bit_index = self.get_reg16(reg);
                        let val = self.read_rm16(modbits, rm);
                        let bit = (val >> (bit_index & 0x0F)) & 1;
                        self.set_flag(FLAG_CF, bit != 0);
                        let new_val = val ^ (1 << (bit_index & 0x0F));
                        self.write_rm16(modbits, rm, new_val);
                        self.cycles += if modbits == 0b11 { 6 } else { 13 };
                        if modbits == 0b11 {
                            6
                        } else {
                            13
                        }
                    }
                    // LSS - Load Far Pointer to SS (0x0F 0xB2) - 80386+
                    0xB2 => {
                        if !self.model.supports_80386_instructions() {
                            // Invalid opcode on 8086/8088/80186/80286
                            self.cycles += 10;
                            return 10;
                        }
                        let modrm = self.fetch_u8();
                        let (modbits, reg, rm) = Self::decode_modrm(modrm);
                        // LSS only works with memory operands
                        if modbits != 0b11 {
                            let (seg, offset_ea, _) = self.calc_effective_address(modbits, rm);
                            let offset = self.read_u16(seg, offset_ea);
                            let segment = self.read_u16(seg, offset_ea.wrapping_add(2));
                            self.set_reg16(reg, offset);
                            self.ss = segment;
                        }
                        self.cycles += 7;
                        7
                    }
                    // LFS - Load Far Pointer to FS (0x0F 0xB4) - 80386+
                    0xB4 => {
                        if !self.model.supports_80386_instructions() {
                            // Invalid opcode on 8086/8088/80186/80286
                            self.cycles += 10;
                            return 10;
                        }
                        let modrm = self.fetch_u8();
                        let (modbits, reg, rm) = Self::decode_modrm(modrm);
                        // LFS only works with memory operands
                        if modbits != 0b11 {
                            let (seg, offset_ea, _) = self.calc_effective_address(modbits, rm);
                            let offset = self.read_u16(seg, offset_ea);
                            let segment = self.read_u16(seg, offset_ea.wrapping_add(2));
                            self.set_reg16(reg, offset);
                            self.fs = segment;
                        }
                        self.cycles += 7;
                        7
                    }
                    // LGS - Load Far Pointer to GS (0x0F 0xB5) - 80386+
                    0xB5 => {
                        if !self.model.supports_80386_instructions() {
                            // Invalid opcode on 8086/8088/80186/80286
                            self.cycles += 10;
                            return 10;
                        }
                        let modrm = self.fetch_u8();
                        let (modbits, reg, rm) = Self::decode_modrm(modrm);
                        // LGS only works with memory operands
                        if modbits != 0b11 {
                            let (seg, offset_ea, _) = self.calc_effective_address(modbits, rm);
                            let offset = self.read_u16(seg, offset_ea);
                            let segment = self.read_u16(seg, offset_ea.wrapping_add(2));
                            self.set_reg16(reg, offset);
                            self.gs = segment;
                        }
                        self.cycles += 7;
                        7
                    }
                    // SHLD - Double Precision Shift Left (0x0F 0xA4, 0xA5) - 80386+
                    0xA4 => {
                        if !self.model.supports_80386_instructions() {
                            // Invalid opcode on 8086/8088/80186/80286
                            self.cycles += 10;
                            return 10;
                        }
                        let modrm = self.fetch_u8();
                        let (_modbits, _reg, _rm) = Self::decode_modrm(modrm);
                        let _count = self.fetch_u8();
                        // Stub: Not fully implemented
                        self.cycles += 3;
                        3
                    }
                    0xA5 => {
                        if !self.model.supports_80386_instructions() {
                            // Invalid opcode on 8086/8088/80186/80286
                            self.cycles += 10;
                            return 10;
                        }
                        let _modrm = self.fetch_u8();
                        // SHLD with CL
                        // Stub: Not fully implemented
                        self.cycles += 3;
                        3
                    }
                    // SHRD - Double Precision Shift Right (0x0F 0xAC, 0xAD) - 80386+
                    0xAC => {
                        if !self.model.supports_80386_instructions() {
                            // Invalid opcode on 8086/8088/80186/80286
                            self.cycles += 10;
                            return 10;
                        }
                        let modrm = self.fetch_u8();
                        let (_modbits, _reg, _rm) = Self::decode_modrm(modrm);
                        let _count = self.fetch_u8();
                        // Stub: Not fully implemented
                        self.cycles += 3;
                        3
                    }
                    0xAD => {
                        if !self.model.supports_80386_instructions() {
                            // Invalid opcode on 8086/8088/80186/80286
                            self.cycles += 10;
                            return 10;
                        }
                        let _modrm = self.fetch_u8();
                        // SHRD with CL
                        // Stub: Not fully implemented
                        self.cycles += 3;
                        3
                    }
                    // SETcc - Set Byte on Condition (0x0F 0x90-0x9F) - 80386+
                    0x90..=0x9F => {
                        if !self.model.supports_80386_instructions() {
                            // Invalid opcode on 8086/8088/80186/80286
                            self.cycles += 10;
                            return 10;
                        }
                        let modrm = self.fetch_u8();
                        let (modbits, _reg, rm) = Self::decode_modrm(modrm);
                        let condition = next_opcode & 0x0F;
                        let result = self.check_condition(condition);
                        self.write_rm8(modbits, rm, if result { 1 } else { 0 });
                        self.cycles += if modbits == 0b11 { 4 } else { 5 };
                        if modbits == 0b11 {
                            4
                        } else {
                            5
                        }
                    }
                    // PUSH FS (0x0F 0xA0) - 80386+
                    0xA0 => {
                        if !self.model.supports_80386_instructions() {
                            // Invalid opcode on 8086/8088/80186/80286
                            self.cycles += 10;
                            return 10;
                        }
                        self.push(self.fs);
                        self.cycles += 2;
                        2
                    }
                    // POP FS (0x0F 0xA1) - 80386+
                    0xA1 => {
                        if !self.model.supports_80386_instructions() {
                            // Invalid opcode on 8086/8088/80186/80286
                            self.cycles += 10;
                            return 10;
                        }
                        self.fs = self.pop();
                        self.cycles += 7;
                        7
                    }
                    // PUSH GS (0x0F 0xA8) - 80386+
                    0xA8 => {
                        if !self.model.supports_80386_instructions() {
                            // Invalid opcode on 8086/8088/80186/80286
                            self.cycles += 10;
                            return 10;
                        }
                        self.push(self.gs);
                        self.cycles += 2;
                        2
                    }
                    // POP GS (0x0F 0xA9) - 80386+
                    0xA9 => {
                        if !self.model.supports_80386_instructions() {
                            // Invalid opcode on 8086/8088/80186/80286
                            self.cycles += 10;
                            return 10;
                        }
                        self.gs = self.pop();
                        self.cycles += 7;
                        7
                    }
                    // MOV reg, CRx - Move from Control Register (0x0F 0x20) - 80386+
                    0x20 => {
                        if !self.model.supports_80386_instructions() {
                            self.cycles += 6;
                            return 6;
                        }

                        let modrm = self.fetch_u8();
                        let (_, reg, rm) = Self::decode_modrm(modrm);

                        // Read from control register (only CR0 is commonly used)
                        let cr_value = match reg {
                            0 => self.protected_mode.get_cr0(), // CR0
                            2 => 0, // CR2 (page fault linear address) - stub
                            3 => 0, // CR3 (page directory base) - stub
                            _ => 0, // Reserved
                        };

                        // Store to destination register
                        self.set_reg16(rm, cr_value);
                        self.cycles += 6;
                        6
                    }
                    // MOV CRx, reg - Move to Control Register (0x0F 0x22) - 80386+
                    0x22 => {
                        if !self.model.supports_80386_instructions() {
                            self.cycles += 10;
                            return 10;
                        }

                        let modrm = self.fetch_u8();
                        let (_, reg, rm) = Self::decode_modrm(modrm);

                        // Read from source register
                        let value = self.get_reg16(rm);

                        // Write to control register (only CR0 is commonly used)
                        match reg {
                            0 => self.protected_mode.set_cr0(value), // CR0
                            2 => {} // CR2 (page fault linear address) - stub
                            3 => {} // CR3 (page directory base) - stub
                            _ => {} // Reserved
                        }

                        self.cycles += 10;
                        10
                    }
                    // INVD - Invalidate Cache (0x0F 0x08) - 80486+
                    0x08 => {
                        if !self.model.supports_80486_instructions() {
                            self.cycles += 2;
                            return 2;
                        }
                        // Invalidate internal caches without writing back
                        // Since we don't emulate caches, this is a NOP
                        self.cycles += 15;
                        15
                    }
                    // WBINVD - Write-Back and Invalidate Cache (0x0F 0x09) - 80486+
                    0x09 => {
                        if !self.model.supports_80486_instructions() {
                            self.cycles += 2;
                            return 2;
                        }
                        // Write back and invalidate internal caches
                        // Since we don't emulate caches, this is a NOP
                        self.cycles += 500; // Very slow instruction on real hardware
                        500
                    }
                    // WRMSR - Write Model-Specific Register (0x0F 0x30) - Pentium+
                    0x30 => {
                        if !self.model.supports_pentium_instructions() {
                            self.cycles += 2;
                            return 2;
                        }
                        // ECX contains MSR index, value comes from EDX:EAX (full 32-bit)
                        let msr_index = self.cx as u32;
                        let value = (self.ax as u64) | ((self.dx as u64) << 32);
                        self.msrs.insert(msr_index, value);
                        self.cycles += 30;
                        30
                    }
                    // RDTSC - Read Time-Stamp Counter (0x0F 0x31) - Pentium+
                    0x31 => {
                        if !self.model.supports_pentium_instructions() {
                            self.cycles += 2;
                            return 2;
                        }
                        // Return TSC in EDX:EAX (high:low) - Full 32-bit registers
                        self.ax = (self.tsc & 0xFFFFFFFF) as u32;
                        self.dx = ((self.tsc >> 32) & 0xFFFFFFFF) as u32;
                        self.cycles += 6;
                        6
                    }
                    // RDMSR - Read Model-Specific Register (0x0F 0x32) - Pentium+
                    0x32 => {
                        if !self.model.supports_pentium_instructions() {
                            self.cycles += 2;
                            return 2;
                        }
                        // ECX contains MSR index, result goes in EDX:EAX - Full 32-bit registers
                        let msr_index = self.cx as u32;
                        let value = self.msrs.get(&msr_index).copied().unwrap_or(0);
                        // Split 64-bit value into EDX:EAX (high:low)
                        self.ax = (value & 0xFFFFFFFF) as u32;
                        self.dx = ((value >> 32) & 0xFFFFFFFF) as u32;
                        self.cycles += 20;
                        20
                    }
                    // CPUID - CPU Identification (0x0F 0xA2) - Pentium+
                    0xA2 => {
                        if !self.model.supports_pentium_instructions() {
                            self.cycles += 2;
                            return 2;
                        }
                        // Input: EAX = function number
                        // Output: EAX, EBX, ECX, EDX contain CPU information (full 32-bit)
                        let function = self.ax;
                        match function {
                            0 => {
                                // Maximum supported function and vendor ID
                                self.ax = 1; // Supports function 0 and 1
                                             // "GenuineIntel" in EBX, EDX, ECX
                                self.bx = 0x756E6547; // "Genu"
                                self.dx = 0x49656E69; // "ineI"
                                self.cx = 0x6C65746E; // "ntel"
                            }
                            1 => {
                                // Processor info and feature bits
                                // Family 5 (Pentium), Model 4 (standard), Stepping 3
                                self.ax = 0x0543; // Family:5, Model:4, Stepping:3
                                self.bx = 0; // Brand index, CLFLUSH size, etc.
                                             // Feature flags in EDX
                                self.dx = 0x00000001; // FPU present
                                self.cx = 0; // Extended features
                            }
                            _ => {
                                // Unsupported function - return zeros
                                self.ax = 0;
                                self.bx = 0;
                                self.cx = 0;
                                self.dx = 0;
                            }
                        }
                        self.cycles += 14;
                        14
                    }
                    // XADD - Exchange and Add (0x0F 0xC0, 0xC1) - 80486+
                    0xC0 => {
                        if !self.model.supports_80486_instructions() {
                            self.cycles += 2;
                            return 2;
                        }
                        // XADD r/m8, r8
                        let modrm = self.fetch_u8();
                        let (modbits, reg, rm) = Self::decode_modrm(modrm);
                        let reg_val = if reg < 4 {
                            self.get_reg8_low(reg)
                        } else {
                            self.get_reg8_high(reg - 4)
                        };
                        let (rm_val, seg, offset) = self.read_rmw8(modbits, rm);

                        // Exchange: temp = r/m, r/m = r/m + reg, reg = temp
                        let sum = rm_val.wrapping_add(reg_val);
                        self.write_rmw8(modbits, rm, sum, seg, offset);
                        if reg < 4 {
                            self.set_reg8_low(reg, rm_val);
                        } else {
                            self.set_reg8_high(reg - 4, rm_val);
                        }

                        // Update flags based on addition
                        self.update_flags_8(sum);
                        let carry = (rm_val as u16 + reg_val as u16) > 0xFF;
                        let overflow = ((rm_val ^ sum) & (reg_val ^ sum) & 0x80) != 0;
                        self.set_flag(FLAG_CF, carry);
                        self.set_flag(FLAG_OF, overflow);

                        self.cycles += if modbits == 0b11 { 3 } else { 10 };
                        if modbits == 0b11 {
                            3
                        } else {
                            10
                        }
                    }
                    0xC1 => {
                        if !self.model.supports_80486_instructions() {
                            self.cycles += 2;
                            return 2;
                        }
                        // XADD r/m16, r16
                        let modrm = self.fetch_u8();
                        let (modbits, reg, rm) = Self::decode_modrm(modrm);
                        let reg_val = self.get_reg16(reg);
                        let (rm_val, seg, offset) = self.read_rmw16(modbits, rm);

                        // Exchange: temp = r/m, r/m = r/m + reg, reg = temp
                        let sum = rm_val.wrapping_add(reg_val);
                        self.write_rmw16(modbits, rm, sum, seg, offset);
                        self.set_reg16(reg, rm_val);

                        // Update flags based on addition
                        self.update_flags_16(sum);
                        let carry = (rm_val as u32 + reg_val as u32) > 0xFFFF;
                        let overflow = ((rm_val ^ sum) & (reg_val ^ sum) & 0x8000) != 0;
                        self.set_flag(FLAG_CF, carry);
                        self.set_flag(FLAG_OF, overflow);

                        self.cycles += if modbits == 0b11 { 3 } else { 10 };
                        if modbits == 0b11 {
                            3
                        } else {
                            10
                        }
                    }
                    // CMPXCHG - Compare and Exchange (0x0F 0xB0, 0xB1) - 80486+
                    0xB0 => {
                        if !self.model.supports_80486_instructions() {
                            self.cycles += 2;
                            return 2;
                        }
                        // CMPXCHG r/m8, r8
                        let modrm = self.fetch_u8();
                        let (modbits, reg, rm) = Self::decode_modrm(modrm);
                        let reg_val = if reg < 4 {
                            self.get_reg8_low(reg)
                        } else {
                            self.get_reg8_high(reg - 4)
                        };
                        let (rm_val, seg, offset) = self.read_rmw8(modbits, rm);
                        let al = (self.ax & 0xFF) as u8;

                        // Compare AL with r/m8
                        if al == rm_val {
                            // If equal, ZF=1 and r/m8 = r8
                            self.set_flag(FLAG_ZF, true);
                            self.write_rmw8(modbits, rm, reg_val, seg, offset);
                        } else {
                            // If not equal, ZF=0 and AL = r/m8
                            self.set_flag(FLAG_ZF, false);
                            self.ax = (self.ax & 0xFFFF_FF00) | (rm_val as u32);
                        }

                        // Update other flags based on comparison
                        let result = al.wrapping_sub(rm_val);
                        self.set_flag(FLAG_SF, (result & 0x80) != 0);
                        self.set_flag(FLAG_PF, result.count_ones() % 2 == 0);
                        let carry = (al as u16) < (rm_val as u16);
                        let overflow = ((al ^ rm_val) & (al ^ result) & 0x80) != 0;
                        self.set_flag(FLAG_CF, carry);
                        self.set_flag(FLAG_OF, overflow);

                        self.cycles += if modbits == 0b11 { 6 } else { 10 };
                        if modbits == 0b11 {
                            6
                        } else {
                            10
                        }
                    }
                    0xB1 => {
                        if !self.model.supports_80486_instructions() {
                            self.cycles += 2;
                            return 2;
                        }
                        // CMPXCHG r/m16, r16
                        let modrm = self.fetch_u8();
                        let (modbits, reg, rm) = Self::decode_modrm(modrm);
                        let reg_val = self.get_reg16(reg);
                        let (rm_val, seg, offset) = self.read_rmw16(modbits, rm);

                        // Compare AX with r/m16
                        if (self.ax as u16) == rm_val {
                            // If equal, ZF=1 and r/m16 = r16
                            self.set_flag(FLAG_ZF, true);
                            self.write_rmw16(modbits, rm, reg_val, seg, offset);
                        } else {
                            // If not equal, ZF=0 and AX = r/m16
                            self.set_flag(FLAG_ZF, false);
                            self.ax = rm_val as u32;
                        }

                        // Update other flags based on comparison
                        let result = self.ax.wrapping_sub(rm_val as u32);
                        self.set_flag(FLAG_SF, (result & 0x8000) != 0);
                        self.set_flag(FLAG_PF, (result & 0xFF).count_ones() % 2 == 0);
                        let carry = self.ax < (rm_val as u32);
                        let overflow = (((self.ax as u16) ^ rm_val)
                            & ((self.ax as u16) ^ (result as u16))
                            & 0x8000)
                            != 0;
                        self.set_flag(FLAG_CF, carry);
                        self.set_flag(FLAG_OF, overflow);

                        self.cycles += if modbits == 0b11 { 6 } else { 10 };
                        if modbits == 0b11 {
                            6
                        } else {
                            10
                        }
                    }
                    // BSWAP - Byte Swap (0x0F 0xC8-0xCF) - 80486+
                    0xC8..=0xCF => {
                        if !self.model.supports_80486_instructions() {
                            self.cycles += 2;
                            return 2;
                        }
                        // BSWAP r32 - Reverses byte order of 32-bit register
                        // For 16-bit mode, we swap the high and low words
                        let reg = next_opcode & 0x07;
                        match reg {
                            0 => self.ax = self.ax.swap_bytes(), // AX
                            1 => self.cx = self.cx.swap_bytes(), // CX
                            2 => self.dx = self.dx.swap_bytes(), // DX
                            3 => self.bx = self.bx.swap_bytes(), // BX
                            4 => self.sp = self.sp.swap_bytes(), // SP
                            5 => self.bp = self.bp.swap_bytes(), // BP
                            6 => self.si = self.si.swap_bytes(), // SI
                            7 => self.di = self.di.swap_bytes(), // DI
                            _ => unreachable!(),
                        }
                        self.cycles += 1;
                        1
                    }
                    // CMPXCHG8B - Compare and Exchange 8 Bytes (0x0F 0xC7 /1) - Pentium+
                    0xC7 => {
                        if !self.model.supports_pentium_instructions() {
                            self.cycles += 2;
                            return 2;
                        }
                        let modrm = self.fetch_u8();
                        let (modbits, reg, rm) = Self::decode_modrm(modrm);

                        // Only valid with reg field = 1
                        if reg != 1 {
                            eprintln!("Invalid CMPXCHG8B reg field: {}", reg);
                            self.cycles += 2;
                            return 2;
                        }

                        // CMPXCHG8B m64
                        // Compare EDX:EAX with m64. If equal, set ZF and load ECX:EBX into m64.
                        // Else, clear ZF and load m64 into EDX:EAX.
                        // In 16-bit mode, we work with DX:AX and CX:BX (32 bits total)
                        let (segment, offset, _) = self.calc_effective_address(modbits, rm);

                        // Read 32-bit value from memory (4 bytes)
                        let mem_low = self.read_u16(segment, offset);
                        let mem_high = self.read_u16(segment, offset.wrapping_add(2));
                        let mem_val = (mem_low as u32) | ((mem_high as u32) << 16);

                        // Compare with DX:AX
                        let cmp_val = self.ax | (self.dx << 16);

                        if cmp_val == mem_val {
                            // Equal: ZF=1, write CX:BX to memory
                            self.set_flag(FLAG_ZF, true);
                            let new_val = self.bx | (self.cx << 16);
                            self.write_u16(segment, offset, (new_val & 0xFFFF) as u16);
                            self.write_u16(
                                segment,
                                offset.wrapping_add(2),
                                ((new_val >> 16) & 0xFFFF) as u16,
                            );
                        } else {
                            // Not equal: ZF=0, load memory into DX:AX
                            self.set_flag(FLAG_ZF, false);
                            self.ax = mem_low as u32;
                            self.dx = mem_high as u32;
                        }

                        self.cycles += 10;
                        10
                    }
                    // ===== MMX Instructions (Pentium MMX only) =====
                    // EMMS - Empty MMX State (0x0F 0x77)
                    0x77 => {
                        if !self.model.supports_mmx_instructions() {
                            self.cycles += 2;
                            return 2;
                        }
                        // Clear MMX state - in practice, this marks FPU registers as available
                        // For our simple implementation, we just reset the MMX registers
                        self.mmx_regs = [0; 8];
                        self.cycles += 1;
                        1
                    }
                    // MOVD - Move Doubleword (0x0F 0x6E, 0x7E)
                    0x6E => {
                        if !self.model.supports_mmx_instructions() {
                            self.cycles += 2;
                            return 2;
                        }
                        // MOVD mm, r/m32 - Move 32-bit value to MMX register (low 32 bits)
                        let modrm = self.fetch_u8();
                        let (modbits, reg, rm) = Self::decode_modrm(modrm);

                        let value = if modbits == 0b11 {
                            // From register (16-bit in our implementation)
                            self.get_reg16(rm) as u64
                        } else {
                            // From memory (read 32 bits)
                            let (segment, offset, _) = self.calc_effective_address(modbits, rm);
                            let low = self.read_u16(segment, offset);
                            let high = self.read_u16(segment, offset.wrapping_add(2));
                            ((high as u64) << 16) | (low as u64)
                        };

                        self.mmx_regs[reg as usize] = value;
                        self.cycles += if modbits == 0b11 { 1 } else { 2 };
                        if modbits == 0b11 {
                            1
                        } else {
                            2
                        }
                    }
                    0x7E => {
                        if !self.model.supports_mmx_instructions() {
                            self.cycles += 2;
                            return 2;
                        }
                        // MOVD r/m32, mm - Move MMX register (low 32 bits) to 32-bit location
                        let modrm = self.fetch_u8();
                        let (modbits, reg, rm) = Self::decode_modrm(modrm);

                        let value = self.mmx_regs[reg as usize];

                        if modbits == 0b11 {
                            // To register (16-bit in our implementation)
                            self.set_reg16(rm, (value & 0xFFFF) as u16);
                        } else {
                            // To memory (write 32 bits)
                            let (segment, offset, _) = self.calc_effective_address(modbits, rm);
                            self.write_u16(segment, offset, (value & 0xFFFF) as u16);
                            self.write_u16(
                                segment,
                                offset.wrapping_add(2),
                                ((value >> 16) & 0xFFFF) as u16,
                            );
                        }

                        self.cycles += if modbits == 0b11 { 1 } else { 2 };
                        if modbits == 0b11 {
                            1
                        } else {
                            2
                        }
                    }
                    // MOVQ - Move Quadword (0x0F 0x6F, 0x7F)
                    0x6F => {
                        if !self.model.supports_mmx_instructions() {
                            self.cycles += 2;
                            return 2;
                        }
                        // MOVQ mm, mm/m64 - Move 64-bit value to MMX register
                        let modrm = self.fetch_u8();
                        let (modbits, reg, rm) = Self::decode_modrm(modrm);

                        let value = if modbits == 0b11 {
                            // From MMX register
                            self.mmx_regs[rm as usize]
                        } else {
                            // From memory (read 64 bits)
                            let (segment, offset, _) = self.calc_effective_address(modbits, rm);
                            let mut val = 0u64;
                            for i in 0..4 {
                                let word = self.read_u16(segment, offset.wrapping_add(i * 2));
                                val |= (word as u64) << (i * 16);
                            }
                            val
                        };

                        self.mmx_regs[reg as usize] = value;
                        self.cycles += if modbits == 0b11 { 1 } else { 2 };
                        if modbits == 0b11 {
                            1
                        } else {
                            2
                        }
                    }
                    0x7F => {
                        if !self.model.supports_mmx_instructions() {
                            self.cycles += 2;
                            return 2;
                        }
                        // MOVQ mm/m64, mm - Move MMX register to 64-bit location
                        let modrm = self.fetch_u8();
                        let (modbits, reg, rm) = Self::decode_modrm(modrm);

                        let value = self.mmx_regs[reg as usize];

                        if modbits == 0b11 {
                            // To MMX register
                            self.mmx_regs[rm as usize] = value;
                        } else {
                            // To memory (write 64 bits)
                            let (segment, offset, _) = self.calc_effective_address(modbits, rm);
                            for i in 0..4 {
                                let word = ((value >> (i * 16)) & 0xFFFF) as u16;
                                self.write_u16(segment, offset.wrapping_add(i * 2), word);
                            }
                        }

                        self.cycles += if modbits == 0b11 { 1 } else { 2 };
                        if modbits == 0b11 {
                            1
                        } else {
                            2
                        }
                    }
                    // PADDB - Packed Add Bytes (0x0F 0xFC)
                    0xFC => {
                        if !self.model.supports_mmx_instructions() {
                            self.cycles += 2;
                            return 2;
                        }
                        let modrm = self.fetch_u8();
                        let (modbits, reg, rm) = Self::decode_modrm(modrm);

                        let src = if modbits == 0b11 {
                            self.mmx_regs[rm as usize]
                        } else {
                            let (segment, offset, _) = self.calc_effective_address(modbits, rm);
                            let mut val = 0u64;
                            for i in 0..4 {
                                let word = self.read_u16(segment, offset.wrapping_add(i * 2));
                                val |= (word as u64) << (i * 16);
                            }
                            val
                        };

                        let dst = self.mmx_regs[reg as usize];
                        let mut result = 0u64;

                        // Add 8 bytes independently with wraparound
                        for i in 0..8 {
                            let a = ((dst >> (i * 8)) & 0xFF) as u8;
                            let b = ((src >> (i * 8)) & 0xFF) as u8;
                            let sum = a.wrapping_add(b);
                            result |= (sum as u64) << (i * 8);
                        }

                        self.mmx_regs[reg as usize] = result;
                        self.cycles += if modbits == 0b11 { 1 } else { 2 };
                        if modbits == 0b11 {
                            1
                        } else {
                            2
                        }
                    }
                    // PADDW - Packed Add Words (0x0F 0xFD)
                    0xFD => {
                        if !self.model.supports_mmx_instructions() {
                            self.cycles += 2;
                            return 2;
                        }
                        let modrm = self.fetch_u8();
                        let (modbits, reg, rm) = Self::decode_modrm(modrm);

                        let src = if modbits == 0b11 {
                            self.mmx_regs[rm as usize]
                        } else {
                            let (segment, offset, _) = self.calc_effective_address(modbits, rm);
                            let mut val = 0u64;
                            for i in 0..4 {
                                let word = self.read_u16(segment, offset.wrapping_add(i * 2));
                                val |= (word as u64) << (i * 16);
                            }
                            val
                        };

                        let dst = self.mmx_regs[reg as usize];
                        let mut result = 0u64;

                        // Add 4 words independently with wraparound
                        for i in 0..4 {
                            let a = ((dst >> (i * 16)) & 0xFFFF) as u16;
                            let b = ((src >> (i * 16)) & 0xFFFF) as u16;
                            let sum = a.wrapping_add(b);
                            result |= (sum as u64) << (i * 16);
                        }

                        self.mmx_regs[reg as usize] = result;
                        self.cycles += if modbits == 0b11 { 1 } else { 2 };
                        if modbits == 0b11 {
                            1
                        } else {
                            2
                        }
                    }
                    // PADDD - Packed Add Dwords (0x0F 0xFE)
                    0xFE => {
                        if !self.model.supports_mmx_instructions() {
                            self.cycles += 2;
                            return 2;
                        }
                        let modrm = self.fetch_u8();
                        let (modbits, reg, rm) = Self::decode_modrm(modrm);

                        let src = if modbits == 0b11 {
                            self.mmx_regs[rm as usize]
                        } else {
                            let (segment, offset, _) = self.calc_effective_address(modbits, rm);
                            let mut val = 0u64;
                            for i in 0..4 {
                                let word = self.read_u16(segment, offset.wrapping_add(i * 2));
                                val |= (word as u64) << (i * 16);
                            }
                            val
                        };

                        let dst = self.mmx_regs[reg as usize];
                        let mut result = 0u64;

                        // Add 2 dwords independently with wraparound
                        for i in 0..2 {
                            let a = ((dst >> (i * 32)) & 0xFFFFFFFF) as u32;
                            let b = ((src >> (i * 32)) & 0xFFFFFFFF) as u32;
                            let sum = a.wrapping_add(b);
                            result |= (sum as u64) << (i * 32);
                        }

                        self.mmx_regs[reg as usize] = result;
                        self.cycles += if modbits == 0b11 { 1 } else { 2 };
                        if modbits == 0b11 {
                            1
                        } else {
                            2
                        }
                    }
                    // PSUBB - Packed Subtract Bytes (0x0F 0xF8)
                    0xF8 => {
                        if !self.model.supports_mmx_instructions() {
                            self.cycles += 2;
                            return 2;
                        }
                        let modrm = self.fetch_u8();
                        let (modbits, reg, rm) = Self::decode_modrm(modrm);

                        let src = if modbits == 0b11 {
                            self.mmx_regs[rm as usize]
                        } else {
                            let (segment, offset, _) = self.calc_effective_address(modbits, rm);
                            let mut val = 0u64;
                            for i in 0..4 {
                                let word = self.read_u16(segment, offset.wrapping_add(i * 2));
                                val |= (word as u64) << (i * 16);
                            }
                            val
                        };

                        let dst = self.mmx_regs[reg as usize];
                        let mut result = 0u64;

                        // Subtract 8 bytes independently with wraparound
                        for i in 0..8 {
                            let a = ((dst >> (i * 8)) & 0xFF) as u8;
                            let b = ((src >> (i * 8)) & 0xFF) as u8;
                            let diff = a.wrapping_sub(b);
                            result |= (diff as u64) << (i * 8);
                        }

                        self.mmx_regs[reg as usize] = result;
                        self.cycles += if modbits == 0b11 { 1 } else { 2 };
                        if modbits == 0b11 {
                            1
                        } else {
                            2
                        }
                    }
                    // PSUBW - Packed Subtract Words (0x0F 0xF9)
                    0xF9 => {
                        if !self.model.supports_mmx_instructions() {
                            self.cycles += 2;
                            return 2;
                        }
                        let modrm = self.fetch_u8();
                        let (modbits, reg, rm) = Self::decode_modrm(modrm);

                        let src = if modbits == 0b11 {
                            self.mmx_regs[rm as usize]
                        } else {
                            let (segment, offset, _) = self.calc_effective_address(modbits, rm);
                            let mut val = 0u64;
                            for i in 0..4 {
                                let word = self.read_u16(segment, offset.wrapping_add(i * 2));
                                val |= (word as u64) << (i * 16);
                            }
                            val
                        };

                        let dst = self.mmx_regs[reg as usize];
                        let mut result = 0u64;

                        // Subtract 4 words independently with wraparound
                        for i in 0..4 {
                            let a = ((dst >> (i * 16)) & 0xFFFF) as u16;
                            let b = ((src >> (i * 16)) & 0xFFFF) as u16;
                            let diff = a.wrapping_sub(b);
                            result |= (diff as u64) << (i * 16);
                        }

                        self.mmx_regs[reg as usize] = result;
                        self.cycles += if modbits == 0b11 { 1 } else { 2 };
                        if modbits == 0b11 {
                            1
                        } else {
                            2
                        }
                    }
                    // PSUBD - Packed Subtract Dwords (0x0F 0xFA)
                    0xFA => {
                        if !self.model.supports_mmx_instructions() {
                            self.cycles += 2;
                            return 2;
                        }
                        let modrm = self.fetch_u8();
                        let (modbits, reg, rm) = Self::decode_modrm(modrm);

                        let src = if modbits == 0b11 {
                            self.mmx_regs[rm as usize]
                        } else {
                            let (segment, offset, _) = self.calc_effective_address(modbits, rm);
                            let mut val = 0u64;
                            for i in 0..4 {
                                let word = self.read_u16(segment, offset.wrapping_add(i * 2));
                                val |= (word as u64) << (i * 16);
                            }
                            val
                        };

                        let dst = self.mmx_regs[reg as usize];
                        let mut result = 0u64;

                        // Subtract 2 dwords independently with wraparound
                        for i in 0..2 {
                            let a = ((dst >> (i * 32)) & 0xFFFFFFFF) as u32;
                            let b = ((src >> (i * 32)) & 0xFFFFFFFF) as u32;
                            let diff = a.wrapping_sub(b);
                            result |= (diff as u64) << (i * 32);
                        }

                        self.mmx_regs[reg as usize] = result;
                        self.cycles += if modbits == 0b11 { 1 } else { 2 };
                        if modbits == 0b11 {
                            1
                        } else {
                            2
                        }
                    }
                    // PAND - Packed AND (0x0F 0xDB)
                    0xDB => {
                        if !self.model.supports_mmx_instructions() {
                            self.cycles += 2;
                            return 2;
                        }
                        let modrm = self.fetch_u8();
                        let (modbits, reg, rm) = Self::decode_modrm(modrm);

                        let src = if modbits == 0b11 {
                            self.mmx_regs[rm as usize]
                        } else {
                            let (segment, offset, _) = self.calc_effective_address(modbits, rm);
                            let mut val = 0u64;
                            for i in 0..4 {
                                let word = self.read_u16(segment, offset.wrapping_add(i * 2));
                                val |= (word as u64) << (i * 16);
                            }
                            val
                        };

                        self.mmx_regs[reg as usize] &= src;
                        self.cycles += if modbits == 0b11 { 1 } else { 2 };
                        if modbits == 0b11 {
                            1
                        } else {
                            2
                        }
                    }
                    // POR - Packed OR (0x0F 0xEB)
                    0xEB => {
                        if !self.model.supports_mmx_instructions() {
                            self.cycles += 2;
                            return 2;
                        }
                        let modrm = self.fetch_u8();
                        let (modbits, reg, rm) = Self::decode_modrm(modrm);

                        let src = if modbits == 0b11 {
                            self.mmx_regs[rm as usize]
                        } else {
                            let (segment, offset, _) = self.calc_effective_address(modbits, rm);
                            let mut val = 0u64;
                            for i in 0..4 {
                                let word = self.read_u16(segment, offset.wrapping_add(i * 2));
                                val |= (word as u64) << (i * 16);
                            }
                            val
                        };

                        self.mmx_regs[reg as usize] |= src;
                        self.cycles += if modbits == 0b11 { 1 } else { 2 };
                        if modbits == 0b11 {
                            1
                        } else {
                            2
                        }
                    }
                    // PXOR - Packed XOR (0x0F 0xEF)
                    0xEF => {
                        if !self.model.supports_mmx_instructions() {
                            self.cycles += 2;
                            return 2;
                        }
                        let modrm = self.fetch_u8();
                        let (modbits, reg, rm) = Self::decode_modrm(modrm);

                        let src = if modbits == 0b11 {
                            self.mmx_regs[rm as usize]
                        } else {
                            let (segment, offset, _) = self.calc_effective_address(modbits, rm);
                            let mut val = 0u64;
                            for i in 0..4 {
                                let word = self.read_u16(segment, offset.wrapping_add(i * 2));
                                val |= (word as u64) << (i * 16);
                            }
                            val
                        };

                        self.mmx_regs[reg as usize] ^= src;
                        self.cycles += if modbits == 0b11 { 1 } else { 2 };
                        if modbits == 0b11 {
                            1
                        } else {
                            2
                        }
                    }
                    // PCMPEQB - Packed Compare Equal Bytes (0x0F 0x74)
                    0x74 => {
                        if !self.model.supports_mmx_instructions() {
                            self.cycles += 2;
                            return 2;
                        }
                        let modrm = self.fetch_u8();
                        let (modbits, reg, rm) = Self::decode_modrm(modrm);

                        let src = if modbits == 0b11 {
                            self.mmx_regs[rm as usize]
                        } else {
                            let (segment, offset, _) = self.calc_effective_address(modbits, rm);
                            let mut val = 0u64;
                            for i in 0..4 {
                                let word = self.read_u16(segment, offset.wrapping_add(i * 2));
                                val |= (word as u64) << (i * 16);
                            }
                            val
                        };

                        let dst = self.mmx_regs[reg as usize];
                        let mut result = 0u64;

                        // Compare 8 bytes, set all bits to 1 if equal, 0 if not
                        for i in 0..8 {
                            let a = ((dst >> (i * 8)) & 0xFF) as u8;
                            let b = ((src >> (i * 8)) & 0xFF) as u8;
                            let cmp = if a == b { 0xFF } else { 0x00 };
                            result |= (cmp as u64) << (i * 8);
                        }

                        self.mmx_regs[reg as usize] = result;
                        self.cycles += if modbits == 0b11 { 1 } else { 2 };
                        if modbits == 0b11 {
                            1
                        } else {
                            2
                        }
                    }
                    // PCMPEQW - Packed Compare Equal Words (0x0F 0x75)
                    0x75 => {
                        if !self.model.supports_mmx_instructions() {
                            self.cycles += 2;
                            return 2;
                        }
                        let modrm = self.fetch_u8();
                        let (modbits, reg, rm) = Self::decode_modrm(modrm);

                        let src = if modbits == 0b11 {
                            self.mmx_regs[rm as usize]
                        } else {
                            let (segment, offset, _) = self.calc_effective_address(modbits, rm);
                            let mut val = 0u64;
                            for i in 0..4 {
                                let word = self.read_u16(segment, offset.wrapping_add(i * 2));
                                val |= (word as u64) << (i * 16);
                            }
                            val
                        };

                        let dst = self.mmx_regs[reg as usize];
                        let mut result = 0u64;

                        // Compare 4 words, set all bits to 1 if equal, 0 if not
                        for i in 0..4 {
                            let a = ((dst >> (i * 16)) & 0xFFFF) as u16;
                            let b = ((src >> (i * 16)) & 0xFFFF) as u16;
                            let cmp = if a == b { 0xFFFF } else { 0x0000 };
                            result |= (cmp as u64) << (i * 16);
                        }

                        self.mmx_regs[reg as usize] = result;
                        self.cycles += if modbits == 0b11 { 1 } else { 2 };
                        if modbits == 0b11 {
                            1
                        } else {
                            2
                        }
                    }
                    // PCMPEQD - Packed Compare Equal Dwords (0x0F 0x76)
                    0x76 => {
                        if !self.model.supports_mmx_instructions() {
                            self.cycles += 2;
                            return 2;
                        }
                        let modrm = self.fetch_u8();
                        let (modbits, reg, rm) = Self::decode_modrm(modrm);

                        let src = if modbits == 0b11 {
                            self.mmx_regs[rm as usize]
                        } else {
                            let (segment, offset, _) = self.calc_effective_address(modbits, rm);
                            let mut val = 0u64;
                            for i in 0..4 {
                                let word = self.read_u16(segment, offset.wrapping_add(i * 2));
                                val |= (word as u64) << (i * 16);
                            }
                            val
                        };

                        let dst = self.mmx_regs[reg as usize];
                        let mut result = 0u64;

                        // Compare 2 dwords, set all bits to 1 if equal, 0 if not
                        for i in 0..2 {
                            let a = ((dst >> (i * 32)) & 0xFFFFFFFF) as u32;
                            let b = ((src >> (i * 32)) & 0xFFFFFFFF) as u32;
                            let cmp: u32 = if a == b { 0xFFFFFFFF } else { 0x00000000 };
                            result |= (cmp as u64) << (i * 32);
                        }

                        self.mmx_regs[reg as usize] = result;
                        self.cycles += if modbits == 0b11 { 1 } else { 2 };
                        if modbits == 0b11 {
                            1
                        } else {
                            2
                        }
                    }
                    _ => {
                        eprintln!(
                            "Two-byte opcode 0x0F 0x{:02X} not implemented at CS:IP={:04X}:{:04X}",
                            next_opcode,
                            self.cs,
                            self.ip.wrapping_sub(2u32)
                        );
                        self.cycles += 2;
                        2
                    }
                }
            }

            // ADC r/m8, r8 (0x10) - Add with Carry
            0x10 => {
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);
                let reg_val = if reg < 4 {
                    self.get_reg8_low(reg)
                } else {
                    self.get_reg8_high(reg - 4)
                };
                let (rm_val, seg, offset) = self.read_rmw8(modbits, rm);
                let carry_in = if self.get_flag(FLAG_CF) { 1 } else { 0 };
                let result = rm_val.wrapping_add(reg_val).wrapping_add(carry_in);
                let carry = (rm_val as u16 + reg_val as u16 + carry_in as u16) > 0xFF;
                let overflow = ((rm_val ^ result) & (reg_val ^ result) & 0x80) != 0;
                // AF calculation: check if carry from bit 3 to bit 4 including carry-in
                let af = ((rm_val & 0x0F) + (reg_val & 0x0F) + carry_in) > 0x0F;

                self.write_rmw8(modbits, rm, result, seg, offset);
                self.update_flags_8(result);
                self.set_flag(FLAG_CF, carry);
                self.set_flag(FLAG_OF, overflow);
                self.set_flag(FLAG_AF, af);
                self.cycles += if modbits == 0b11 { 3 } else { 16 };
                if modbits == 0b11 {
                    3
                } else {
                    16
                }
            }

            // ADC r/m16, r16 (0x11) - Add with Carry
            0x11 => {
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);
                let reg_val = self.get_reg16(reg);
                let (rm_val, seg, offset) = self.read_rmw16(modbits, rm);
                let carry_in = if self.get_flag(FLAG_CF) { 1 } else { 0 };
                let result = rm_val.wrapping_add(reg_val).wrapping_add(carry_in);
                let carry = (rm_val as u32 + reg_val as u32 + carry_in as u32) > 0xFFFF;
                let overflow =
                    ((rm_val ^ (result as u16)) & (reg_val ^ (result as u16)) & 0x8000) != 0;
                // AF calculation: check if carry from bit 3 to bit 4 in low byte including carry-in
                let af = (((rm_val & 0x0F) + (reg_val & 0x0F) + carry_in) & 0x10) != 0;

                self.write_rmw16(modbits, rm, result, seg, offset);
                self.update_flags_16(result);
                self.set_flag(FLAG_CF, carry);
                self.set_flag(FLAG_OF, overflow);
                self.set_flag(FLAG_AF, af);
                self.cycles += if modbits == 0b11 { 3 } else { 16 };
                if modbits == 0b11 {
                    3
                } else {
                    16
                }
            }

            // ADC r8, r/m8 (0x12) - Add with Carry
            0x12 => {
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);
                let reg_val = if reg < 4 {
                    self.get_reg8_low(reg)
                } else {
                    self.get_reg8_high(reg - 4)
                };
                let rm_val = self.read_rm8(modbits, rm);
                let carry_in = if self.get_flag(FLAG_CF) { 1 } else { 0 };
                let result = reg_val.wrapping_add(rm_val).wrapping_add(carry_in);
                let carry = (reg_val as u16 + rm_val as u16 + carry_in as u16) > 0xFF;
                let overflow = ((reg_val ^ result) & (rm_val ^ result) & 0x80) != 0;
                // AF calculation: check if carry from bit 3 to bit 4 including carry-in
                let af = ((reg_val & 0x0F) + (rm_val & 0x0F) + carry_in) > 0x0F;

                if reg < 4 {
                    self.set_reg8_low(reg, result);
                } else {
                    self.set_reg8_high(reg - 4, result);
                }
                self.update_flags_8(result);
                self.set_flag(FLAG_CF, carry);
                self.set_flag(FLAG_OF, overflow);
                self.set_flag(FLAG_AF, af);
                self.cycles += if modbits == 0b11 { 3 } else { 9 };
                if modbits == 0b11 {
                    3
                } else {
                    9
                }
            }

            // ADC r16, r/m16 (0x13) - Add with Carry
            0x13 => {
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);
                let reg_val = self.get_reg16(reg);
                let rm_val = self.read_rm16(modbits, rm);
                let carry_in = if self.get_flag(FLAG_CF) { 1 } else { 0 };
                let result = reg_val.wrapping_add(rm_val).wrapping_add(carry_in);
                let carry = (reg_val as u32 + rm_val as u32 + carry_in as u32) > 0xFFFF;
                let overflow =
                    ((reg_val ^ (result as u16)) & (rm_val ^ (result as u16)) & 0x8000) != 0;
                // AF calculation: check if carry from bit 3 to bit 4 in low byte including carry-in
                let af = (((reg_val & 0x0F) + (rm_val & 0x0F) + carry_in) & 0x10) != 0;

                self.set_reg16(reg, result);
                self.update_flags_16(result);
                self.set_flag(FLAG_CF, carry);
                self.set_flag(FLAG_OF, overflow);
                self.set_flag(FLAG_AF, af);
                self.cycles += if modbits == 0b11 { 3 } else { 9 };
                if modbits == 0b11 {
                    3
                } else {
                    9
                }
            }

            // ADC AL, imm8 (0x14) - Add with Carry
            0x14 => {
                let val = self.fetch_u8();
                let al = (self.ax & 0xFF) as u8;
                let carry_in = if self.get_flag(FLAG_CF) { 1 } else { 0 };
                let result = al.wrapping_add(val).wrapping_add(carry_in);
                let carry = (al as u16 + val as u16 + carry_in as u16) > 0xFF;
                let overflow = ((al ^ result) & (val ^ result) & 0x80) != 0;
                // AF calculation: check if carry from bit 3 to bit 4 including carry-in
                let af = ((al & 0x0F) + (val & 0x0F) + carry_in) > 0x0F;

                self.ax = (self.ax & 0xFFFF_FF00) | (result as u32);
                self.update_flags_8(result);
                self.set_flag(FLAG_CF, carry);
                self.set_flag(FLAG_OF, overflow);
                self.set_flag(FLAG_AF, af);
                self.cycles += 4;
                4
            }

            // ADC AX, imm16 (0x15) - Add with Carry
            0x15 => {
                let val = self.fetch_u16();
                let carry_in = if self.get_flag(FLAG_CF) { 1 } else { 0 };
                let result = self.ax.wrapping_add(val as u32).wrapping_add(carry_in);
                let carry = (self.ax as u32 + val as u32 + carry_in as u32) > 0xFFFF;
                let overflow =
                    (((self.ax as u16) ^ (result as u16)) & (val ^ (result as u16)) & 0x8000) != 0;
                // AF calculation: check if carry from bit 3 to bit 4 in low byte including carry-in
                let af =
                    ((((self.ax as u16) & 0x0F) + (val & 0x0F) + (carry_in as u16)) & 0x10) != 0;

                self.ax = (self.ax & 0xFFFF_0000) | (result as u32);
                self.update_flags_16(result as u16);
                self.set_flag(FLAG_CF, carry);
                self.set_flag(FLAG_OF, overflow);
                self.set_flag(FLAG_AF, af);
                self.cycles += 4;
                4
            }

            // PUSH SS (0x16)
            0x16 => {
                self.push(self.ss);
                self.cycles += 10;
                10
            }

            // POP SS (0x17)
            0x17 => {
                self.ss = self.pop();
                self.cycles += 8;
                8
            }

            // SBB r/m8, r8 (0x18) - Subtract with Borrow
            0x18 => {
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);
                let reg_val = if reg < 4 {
                    self.get_reg8_low(reg)
                } else {
                    self.get_reg8_high(reg - 4)
                };
                let (rm_val, seg, offset) = self.read_rmw8(modbits, rm);
                let carry = if self.get_flag(FLAG_CF) { 1 } else { 0 };
                let result = rm_val.wrapping_sub(reg_val).wrapping_sub(carry);
                let borrow = (rm_val as u16) < (reg_val as u16 + carry as u16);
                let overflow = ((rm_val ^ reg_val) & (rm_val ^ result) & 0x80) != 0;
                // AF calculation: check if borrow from bit 4 to bit 3 including carry-in
                let af = (rm_val & 0x0F) < ((reg_val & 0x0F) + carry);

                self.write_rmw8(modbits, rm, result, seg, offset);
                self.update_flags_8(result);
                self.set_flag(FLAG_CF, borrow);
                self.set_flag(FLAG_OF, overflow);
                self.set_flag(FLAG_AF, af);
                self.cycles += if modbits == 0b11 { 3 } else { 16 };
                if modbits == 0b11 {
                    3
                } else {
                    16
                }
            }

            // SBB r/m16, r16 (0x19) - Subtract with Borrow
            0x19 => {
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);
                let reg_val = self.get_reg16(reg);
                let (rm_val, seg, offset) = self.read_rmw16(modbits, rm);
                let carry = if self.get_flag(FLAG_CF) { 1 } else { 0 };
                let result = rm_val.wrapping_sub(reg_val).wrapping_sub(carry);
                let borrow = (rm_val as u32) < (reg_val as u32 + carry as u32);
                let overflow = ((rm_val ^ reg_val) & (rm_val ^ (result as u16)) & 0x8000) != 0;
                // AF calculation: check if borrow from bit 4 to bit 3 in low byte including carry-in
                let af = (rm_val & 0x0F) < ((reg_val & 0x0F) + carry);

                self.write_rmw16(modbits, rm, result, seg, offset);
                self.update_flags_16(result);
                self.set_flag(FLAG_CF, borrow);
                self.set_flag(FLAG_OF, overflow);
                self.set_flag(FLAG_AF, af);
                self.cycles += if modbits == 0b11 { 3 } else { 16 };
                if modbits == 0b11 {
                    3
                } else {
                    16
                }
            }

            // SBB r8, r/m8 (0x1A) - Subtract with Borrow
            0x1A => {
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);
                let reg_val = if reg < 4 {
                    self.get_reg8_low(reg)
                } else {
                    self.get_reg8_high(reg - 4)
                };
                let rm_val = self.read_rm8(modbits, rm);
                let carry = if self.get_flag(FLAG_CF) { 1 } else { 0 };
                let result = reg_val.wrapping_sub(rm_val).wrapping_sub(carry);
                let borrow = (reg_val as u16) < (rm_val as u16 + carry as u16);
                let overflow = ((reg_val ^ rm_val) & (reg_val ^ result) & 0x80) != 0;
                // AF calculation: check if borrow from bit 4 to bit 3 including carry-in
                let af = (reg_val & 0x0F) < ((rm_val & 0x0F) + carry);

                if reg < 4 {
                    self.set_reg8_low(reg, result);
                } else {
                    self.set_reg8_high(reg - 4, result);
                }
                self.update_flags_8(result);
                self.set_flag(FLAG_CF, borrow);
                self.set_flag(FLAG_OF, overflow);
                self.set_flag(FLAG_AF, af);
                self.cycles += if modbits == 0b11 { 3 } else { 9 };
                if modbits == 0b11 {
                    3
                } else {
                    9
                }
            }

            // SBB r16, r/m16 (0x1B) - Subtract with Borrow
            0x1B => {
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);
                let reg_val = self.get_reg16(reg);
                let rm_val = self.read_rm16(modbits, rm);
                let carry = if self.get_flag(FLAG_CF) { 1 } else { 0 };
                let result = reg_val.wrapping_sub(rm_val).wrapping_sub(carry);
                let borrow = (reg_val as u32) < (rm_val as u32 + carry as u32);
                let overflow = ((reg_val ^ rm_val) & (reg_val ^ (result as u16)) & 0x8000) != 0;
                // AF calculation: check if borrow from bit 4 to bit 3 in low byte including carry-in
                let af = (reg_val & 0x0F) < ((rm_val & 0x0F) + carry);

                self.set_reg16(reg, result);
                self.update_flags_16(result);
                self.set_flag(FLAG_CF, borrow);
                self.set_flag(FLAG_OF, overflow);
                self.set_flag(FLAG_AF, af);
                self.cycles += if modbits == 0b11 { 3 } else { 9 };
                if modbits == 0b11 {
                    3
                } else {
                    9
                }
            }

            // SBB AL, imm8 (0x1C) - Subtract with Borrow
            0x1C => {
                let val = self.fetch_u8();
                let al = (self.ax & 0xFF) as u8;
                let carry = if self.get_flag(FLAG_CF) { 1 } else { 0 };
                let result = al.wrapping_sub(val).wrapping_sub(carry);
                let borrow = (al as u16) < (val as u16 + carry as u16);
                let overflow = ((al ^ val) & (al ^ result) & 0x80) != 0;
                // AF calculation: check if borrow from bit 4 to bit 3 including carry-in
                let af = (al & 0x0F) < ((val & 0x0F) + (carry as u8));

                self.ax = (self.ax & 0xFFFF_FF00) | (result as u32);
                self.update_flags_8(result);
                self.set_flag(FLAG_CF, borrow);
                self.set_flag(FLAG_OF, overflow);
                self.set_flag(FLAG_AF, af);
                self.cycles += 4;
                4
            }

            // SBB AX, imm16 (0x1D) - Subtract with Borrow
            0x1D => {
                let val = self.fetch_u16();
                let carry = if self.get_flag(FLAG_CF) { 1 } else { 0 };
                let result = self.ax.wrapping_sub(val as u32).wrapping_sub(carry);
                let borrow = self.ax < (val as u32 + carry as u32);
                let overflow =
                    (((self.ax as u16) ^ val) & ((self.ax as u16) ^ (result as u16)) & 0x8000) != 0;
                // AF calculation: check if borrow from bit 4 to bit 3 in low byte including carry-in
                let af = ((self.ax as u16) & 0x0F) < ((val & 0x0F) + (carry as u16));

                self.ax = (self.ax & 0xFFFF_0000) | (result as u32);
                self.update_flags_16(result as u16);
                self.set_flag(FLAG_CF, borrow);
                self.set_flag(FLAG_OF, overflow);
                self.set_flag(FLAG_AF, af);
                self.cycles += 4;
                4
            }

            // PUSH DS (0x1E)
            0x1E => {
                self.push(self.ds);
                self.cycles += 10;
                10
            }

            // POP DS (0x1F)
            0x1F => {
                let val = self.pop();
                self.ds = val;
                self.cycles += 8;
                8
            }

            // AND r/m8, r8 (0x20)
            0x20 => {
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);
                let reg_val = if reg < 4 {
                    self.get_reg8_low(reg)
                } else {
                    self.get_reg8_high(reg - 4)
                };
                let (rm_val, seg, offset) = self.read_rmw8(modbits, rm);
                let result = rm_val & reg_val;

                self.write_rmw8(modbits, rm, result, seg, offset);
                self.update_flags_8(result);
                self.set_flag(FLAG_CF, false);
                self.set_flag(FLAG_OF, false);
                self.cycles += if modbits == 0b11 { 3 } else { 16 };
                if modbits == 0b11 {
                    3
                } else {
                    16
                }
            }

            // AND r/m16, r16 (0x21)
            0x21 => {
                // AND r/m16/32, r16/32
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);

                if self.operand_size_override && self.model.supports_80386_instructions() {
                    // 32-bit operation
                    let reg_val = self.get_reg32(reg);
                    let (rm_val, seg, offset) = self.read_rmw32(modbits, rm);
                    let result = rm_val & reg_val;

                    self.write_rmw32(modbits, rm, result, seg, offset);
                    self.update_flags_32(result);
                    self.set_flag(FLAG_CF, false);
                    self.set_flag(FLAG_OF, false);
                    self.cycles += if modbits == 0b11 { 3 } else { 16 };
                    if modbits == 0b11 {
                        3
                    } else {
                        16
                    }
                } else {
                    // 16-bit operation
                    let reg_val = self.get_reg16(reg);
                    let (rm_val, seg, offset) = self.read_rmw16(modbits, rm);
                    let result = rm_val & reg_val;

                    self.write_rmw16(modbits, rm, result, seg, offset);
                    self.update_flags_16(result as u16);
                    self.set_flag(FLAG_CF, false);
                    self.set_flag(FLAG_OF, false);
                    self.cycles += if modbits == 0b11 { 3 } else { 16 };
                    if modbits == 0b11 {
                        3
                    } else {
                        16
                    }
                }
            }

            // AND r8, r/m8 (0x22)
            0x22 => {
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);
                let reg_val = if reg < 4 {
                    self.get_reg8_low(reg)
                } else {
                    self.get_reg8_high(reg - 4)
                };
                let rm_val = self.read_rm8(modbits, rm);
                let result = reg_val & rm_val;

                if reg < 4 {
                    self.set_reg8_low(reg, result);
                } else {
                    self.set_reg8_high(reg - 4, result);
                }
                self.update_flags_8(result);
                self.set_flag(FLAG_CF, false);
                self.set_flag(FLAG_OF, false);
                self.cycles += if modbits == 0b11 { 3 } else { 9 };
                if modbits == 0b11 {
                    3
                } else {
                    9
                }
            }

            // AND r16/32, r/m16/32 (0x23)
            0x23 => {
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);

                if self.operand_size_override && self.model.supports_80386_instructions() {
                    // 32-bit operation
                    let reg_val = self.get_reg32(reg);
                    let rm_val = self.read_rm32(modbits, rm);
                    let result = reg_val & rm_val;

                    self.set_reg32(reg, result);
                    self.update_flags_32(result);
                    self.set_flag(FLAG_CF, false);
                    self.set_flag(FLAG_OF, false);
                    self.cycles += if modbits == 0b11 { 3 } else { 9 };
                    if modbits == 0b11 {
                        3
                    } else {
                        9
                    }
                } else {
                    // 16-bit operation
                    let reg_val = self.get_reg16(reg);
                    let rm_val = self.read_rm16(modbits, rm);
                    let result = reg_val & rm_val;

                    self.set_reg16(reg, result);
                    self.update_flags_16(result as u16);
                    self.set_flag(FLAG_CF, false);
                    self.set_flag(FLAG_OF, false);
                    self.cycles += if modbits == 0b11 { 3 } else { 9 };
                    if modbits == 0b11 {
                        3
                    } else {
                        9
                    }
                }
            }

            // XOR AL, imm8
            0x34 => {
                let val = self.fetch_u8();
                let al = (self.ax & 0xFF) as u8;
                let result = al ^ val;

                self.ax = (self.ax & 0xFFFF_FF00) | (result as u32);
                self.update_flags_8(result);
                self.set_flag(FLAG_CF, false);
                self.set_flag(FLAG_OF, false);
                self.cycles += 4;
                4
            }

            // XOR AX, imm16
            0x35 => {
                let val = self.fetch_u16();
                let result = (self.ax as u16) ^ val;

                self.ax = (self.ax & 0xFFFF_0000) | (result as u32);
                self.update_flags_16(result);
                self.set_flag(FLAG_CF, false);
                self.set_flag(FLAG_OF, false);
                self.cycles += 4;
                4
            }

            // SS segment override prefix (0x36)
            0x36 => {
                // SS segment override prefix
                self.segment_override = Some(SegmentOverride::SS);
                self.step() // Execute next instruction with SS override
            }

            // AAA - ASCII Adjust After Addition (0x37)
            0x37 => {
                let al = (self.ax & 0xFF) as u8;
                if (al & 0x0F) > 9 || self.get_flag(FLAG_AF) {
                    self.ax = self.ax.wrapping_add(0x106u32); // Add 1 to AH, 6 to AL
                    self.set_flag(FLAG_AF, true);
                    self.set_flag(FLAG_CF, true);
                } else {
                    self.set_flag(FLAG_AF, false);
                    self.set_flag(FLAG_CF, false);
                }
                self.ax &= 0xFF0F; // Clear upper nibble of AL
                self.cycles += 4;
                4
            }

            // DS segment override prefix (0x3E)
            0x3E => {
                // DS segment override prefix
                self.segment_override = Some(SegmentOverride::DS);
                self.step() // Execute next instruction with DS override
            }

            // AAS - ASCII Adjust After Subtraction (0x3F)
            0x3F => {
                let al = (self.ax & 0xFF) as u8;
                if (al & 0x0F) > 9 || self.get_flag(FLAG_AF) {
                    self.ax = self.ax.wrapping_sub(6u32); // Subtract 6 from AL
                    self.ax = (self.ax & 0xFF) | ((self.ax.wrapping_sub(0x100u32)) & 0xFF00); // Subtract 1 from AH
                    self.set_flag(FLAG_AF, true);
                    self.set_flag(FLAG_CF, true);
                } else {
                    self.set_flag(FLAG_AF, false);
                    self.set_flag(FLAG_CF, false);
                }
                self.ax &= 0xFF0F; // Clear upper nibble of AL
                self.cycles += 4;
                4
            }

            // Conditional jumps (0x70-0x7F)
            // JO - Jump if Overflow (0x70)
            0x70 => {
                let offset = self.fetch_u8() as i8;
                if self.get_flag(FLAG_OF) {
                    self.ip = (self.ip.wrapping_add((offset as i16) as u32)) & 0xFFFF;
                    self.cycles += 16;
                    16
                } else {
                    self.cycles += 4;
                    4
                }
            }

            // JNO - Jump if Not Overflow (0x71)
            0x71 => {
                let offset = self.fetch_u8() as i8;
                if !self.get_flag(FLAG_OF) {
                    self.ip = (self.ip.wrapping_add((offset as i16) as u32)) & 0xFFFF;
                    self.cycles += 16;
                    16
                } else {
                    self.cycles += 4;
                    4
                }
            }

            // JB/JC/JNAE - Jump if Below/Carry (0x72)
            0x72 => {
                let offset = self.fetch_u8() as i8;
                if self.get_flag(FLAG_CF) {
                    self.ip = (self.ip.wrapping_add((offset as i16) as u32)) & 0xFFFF;
                    self.cycles += 16;
                    16
                } else {
                    self.cycles += 4;
                    4
                }
            }

            // JNB/JNC/JAE - Jump if Not Below/No Carry (0x73)
            0x73 => {
                let offset = self.fetch_u8() as i8;
                if !self.get_flag(FLAG_CF) {
                    self.ip = (self.ip.wrapping_add((offset as i16) as u32)) & 0xFFFF;
                    self.cycles += 16;
                    16
                } else {
                    self.cycles += 4;
                    4
                }
            }

            // JE/JZ - Jump if Equal/Zero (0x74) - already implemented

            // JNE/JNZ - Jump if Not Equal/Not Zero (0x75)
            0x75 => {
                let offset = self.fetch_u8() as i8;
                if !self.get_flag(FLAG_ZF) {
                    self.ip = (self.ip.wrapping_add((offset as i16) as u32)) & 0xFFFF;
                    self.cycles += 16;
                    16
                } else {
                    self.cycles += 4;
                    4
                }
            }

            // JBE/JNA - Jump if Below or Equal/Not Above (0x76)
            0x76 => {
                let offset = self.fetch_u8() as i8;
                if self.get_flag(FLAG_CF) || self.get_flag(FLAG_ZF) {
                    self.ip = (self.ip.wrapping_add((offset as i16) as u32)) & 0xFFFF;
                    self.cycles += 16;
                    16
                } else {
                    self.cycles += 4;
                    4
                }
            }

            // JNBE/JA - Jump if Not Below or Equal/Above (0x77)
            0x77 => {
                let offset = self.fetch_u8() as i8;
                if !self.get_flag(FLAG_CF) && !self.get_flag(FLAG_ZF) {
                    self.ip = (self.ip.wrapping_add((offset as i16) as u32)) & 0xFFFF;
                    self.cycles += 16;
                    16
                } else {
                    self.cycles += 4;
                    4
                }
            }

            // JS - Jump if Sign (0x78)
            0x78 => {
                let offset = self.fetch_u8() as i8;
                if self.get_flag(FLAG_SF) {
                    self.ip = (self.ip.wrapping_add((offset as i16) as u32)) & 0xFFFF;
                    self.cycles += 16;
                    16
                } else {
                    self.cycles += 4;
                    4
                }
            }

            // JNS - Jump if Not Sign (0x79)
            0x79 => {
                let offset = self.fetch_u8() as i8;
                if !self.get_flag(FLAG_SF) {
                    self.ip = (self.ip.wrapping_add((offset as i16) as u32)) & 0xFFFF;
                    self.cycles += 16;
                    16
                } else {
                    self.cycles += 4;
                    4
                }
            }

            // JP/JPE - Jump if Parity/Parity Even (0x7A)
            0x7A => {
                let offset = self.fetch_u8() as i8;
                if self.get_flag(FLAG_PF) {
                    self.ip = (self.ip.wrapping_add((offset as i16) as u32)) & 0xFFFF;
                    self.cycles += 16;
                    16
                } else {
                    self.cycles += 4;
                    4
                }
            }

            // JNP/JPO - Jump if Not Parity/Parity Odd (0x7B)
            0x7B => {
                let offset = self.fetch_u8() as i8;
                if !self.get_flag(FLAG_PF) {
                    self.ip = (self.ip.wrapping_add((offset as i16) as u32)) & 0xFFFF;
                    self.cycles += 16;
                    16
                } else {
                    self.cycles += 4;
                    4
                }
            }

            // JL/JNGE - Jump if Less/Not Greater or Equal (0x7C)
            0x7C => {
                let offset = self.fetch_u8() as i8;
                if self.get_flag(FLAG_SF) != self.get_flag(FLAG_OF) {
                    self.ip = (self.ip.wrapping_add((offset as i16) as u32)) & 0xFFFF;
                    self.cycles += 16;
                    16
                } else {
                    self.cycles += 4;
                    4
                }
            }

            // JNL/JGE - Jump if Not Less/Greater or Equal (0x7D)
            0x7D => {
                let offset = self.fetch_u8() as i8;
                if self.get_flag(FLAG_SF) == self.get_flag(FLAG_OF) {
                    self.ip = (self.ip.wrapping_add((offset as i16) as u32)) & 0xFFFF;
                    self.cycles += 16;
                    16
                } else {
                    self.cycles += 4;
                    4
                }
            }

            // JLE/JNG - Jump if Less or Equal/Not Greater (0x7E)
            0x7E => {
                let offset = self.fetch_u8() as i8;
                if self.get_flag(FLAG_ZF) || (self.get_flag(FLAG_SF) != self.get_flag(FLAG_OF)) {
                    self.ip = (self.ip.wrapping_add((offset as i16) as u32)) & 0xFFFF;
                    self.cycles += 16;
                    16
                } else {
                    self.cycles += 4;
                    4
                }
            }

            // JNLE/JG - Jump if Not Less or Equal/Greater (0x7F)
            0x7F => {
                let offset = self.fetch_u8() as i8;
                if !self.get_flag(FLAG_ZF) && (self.get_flag(FLAG_SF) == self.get_flag(FLAG_OF)) {
                    self.ip = (self.ip.wrapping_add((offset as i16) as u32)) & 0xFFFF;
                    self.cycles += 16;
                    16
                } else {
                    self.cycles += 4;
                    4
                }
            }

            // Group 1 immediate operations (0x80-0x83)
            // These require full ModR/M decoding with immediate values
            // 0x80 - r/m8, imm8
            0x80 => {
                let modrm = self.fetch_u8();
                let (modbits, op, rm) = Self::decode_modrm(modrm);
                let (rm_val, cached_seg, cached_offset) = self.read_rmw8(modbits, rm);
                let imm = self.fetch_u8();
                let result = match op {
                    0 => {
                        // ADD
                        let r = rm_val.wrapping_add(imm);
                        let carry = (rm_val as u16 + imm as u16) > 0xFF;
                        let overflow = ((rm_val ^ r) & (imm ^ r) & 0x80) != 0;
                        let af = Self::calc_af_add_8(rm_val, imm);
                        self.set_flag(FLAG_CF, carry);
                        self.set_flag(FLAG_OF, overflow);
                        self.set_flag(FLAG_AF, af);
                        r
                    }
                    1 => {
                        // OR
                        let r = rm_val | imm;
                        self.set_flag(FLAG_CF, false);
                        self.set_flag(FLAG_OF, false);
                        r
                    }
                    2 => {
                        // ADC
                        let carry_in = if self.get_flag(FLAG_CF) { 1 } else { 0 };
                        let r = rm_val.wrapping_add(imm).wrapping_add(carry_in);
                        let carry = (rm_val as u16 + imm as u16 + carry_in as u16) > 0xFF;
                        let overflow = ((rm_val ^ r) & (imm ^ r) & 0x80) != 0;
                        let af = ((rm_val & 0x0F) + (imm & 0x0F) + carry_in) > 0x0F;
                        self.set_flag(FLAG_CF, carry);
                        self.set_flag(FLAG_OF, overflow);
                        self.set_flag(FLAG_AF, af);
                        r
                    }
                    3 => {
                        // SBB
                        let carry_in = if self.get_flag(FLAG_CF) { 1 } else { 0 };
                        let r = rm_val.wrapping_sub(imm).wrapping_sub(carry_in);
                        let borrow = (rm_val as u16) < (imm as u16 + carry_in as u16);
                        let overflow = ((rm_val ^ imm) & (rm_val ^ r) & 0x80) != 0;
                        let af = (rm_val & 0x0F) < ((imm & 0x0F) + carry_in);
                        self.set_flag(FLAG_CF, borrow);
                        self.set_flag(FLAG_OF, overflow);
                        self.set_flag(FLAG_AF, af);
                        r
                    }
                    4 => {
                        // AND
                        let r = rm_val & imm;
                        self.set_flag(FLAG_CF, false);
                        self.set_flag(FLAG_OF, false);
                        r
                    }
                    5 => {
                        // SUB
                        let r = rm_val.wrapping_sub(imm);
                        let borrow = (rm_val as u16) < (imm as u16);
                        let overflow = ((rm_val ^ imm) & (rm_val ^ r) & 0x80) != 0;
                        let af = Self::calc_af_sub_8(rm_val, imm);
                        self.set_flag(FLAG_CF, borrow);
                        self.set_flag(FLAG_OF, overflow);
                        self.set_flag(FLAG_AF, af);
                        r
                    }
                    6 => {
                        // XOR
                        let r = rm_val ^ imm;
                        self.set_flag(FLAG_CF, false);
                        self.set_flag(FLAG_OF, false);
                        r
                    }
                    7 => {
                        // CMP
                        let r = rm_val.wrapping_sub(imm);
                        let borrow = (rm_val as u16) < (imm as u16);
                        let overflow = ((rm_val ^ imm) & (rm_val ^ r) & 0x80) != 0;
                        let af = Self::calc_af_sub_8(rm_val, imm);
                        self.update_flags_8(r);
                        self.set_flag(FLAG_CF, borrow);
                        self.set_flag(FLAG_OF, overflow);
                        self.set_flag(FLAG_AF, af);
                        self.cycles += if modbits == 0b11 { 4 } else { 17 };
                        return if modbits == 0b11 { 4 } else { 17 };
                    }
                    _ => unreachable!(),
                };
                if op != 7 {
                    self.write_rmw8(modbits, rm, result, cached_seg, cached_offset);
                    self.update_flags_8(result);
                }
                self.cycles += if modbits == 0b11 { 4 } else { 17 };
                if modbits == 0b11 {
                    4
                } else {
                    17
                }
            }

            // 0x81 - r/m16, imm16
            0x81 => {
                let modrm = self.fetch_u8();
                let (modbits, op, rm) = Self::decode_modrm(modrm);

                // Check for operand-size override (0x66 prefix)
                if self.operand_size_override && self.model.supports_80386_instructions() {
                    // 32-bit operand size
                    let (rm_val, cached_seg, cached_offset) = self.read_rmw16(modbits, rm);
                    // Fetch 32-bit immediate but only use lower 16 bits for now
                    let imm_low = self.fetch_u16();
                    let _imm_high = self.fetch_u16();
                    let imm = imm_low; // Use only lower 16 bits in 16-bit mode
                                       // NOTE: In true 32-bit mode, we'd use the full 32-bit value
                                       // For now, just consume the bytes to keep instruction stream in sync

                    let result = match op {
                        0 => {
                            // ADD
                            let r = rm_val.wrapping_add(imm);
                            let carry = (rm_val as u32 + imm as u32) > 0xFFFF;
                            let overflow = ((rm_val ^ r) & (imm ^ r) & 0x8000) != 0;
                            let af = Self::calc_af_add_16(rm_val, imm);
                            self.set_flag(FLAG_CF, carry);
                            self.set_flag(FLAG_OF, overflow);
                            self.set_flag(FLAG_AF, af);
                            r
                        }
                        1 => {
                            // OR
                            let r = rm_val | imm;
                            self.set_flag(FLAG_CF, false);
                            self.set_flag(FLAG_OF, false);
                            r
                        }
                        2 => {
                            // ADC
                            let carry_in = if self.get_flag(FLAG_CF) { 1 } else { 0 };
                            let r = rm_val.wrapping_add(imm).wrapping_add(carry_in);
                            let carry = (rm_val as u32 + imm as u32 + carry_in as u32) > 0xFFFF;
                            let overflow = ((rm_val ^ r) & (imm ^ r) & 0x8000) != 0;
                            let af = (((rm_val & 0x0F) + (imm & 0x0F) + carry_in) & 0x10) != 0;
                            self.set_flag(FLAG_CF, carry);
                            self.set_flag(FLAG_OF, overflow);
                            self.set_flag(FLAG_AF, af);
                            r
                        }
                        3 => {
                            // SBB
                            let carry_in = if self.get_flag(FLAG_CF) { 1 } else { 0 };
                            let r = rm_val.wrapping_sub(imm).wrapping_sub(carry_in);
                            let borrow = (rm_val as u32) < (imm as u32 + carry_in as u32);
                            let overflow = ((rm_val ^ imm) & (rm_val ^ r) & 0x8000) != 0;
                            let af = (rm_val & 0x0F) < ((imm & 0x0F) + carry_in);
                            self.set_flag(FLAG_CF, borrow);
                            self.set_flag(FLAG_OF, overflow);
                            self.set_flag(FLAG_AF, af);
                            r
                        }
                        4 => {
                            // AND
                            let r = rm_val & imm;
                            self.set_flag(FLAG_CF, false);
                            self.set_flag(FLAG_OF, false);
                            r
                        }
                        5 => {
                            // SUB
                            let r = rm_val.wrapping_sub(imm);
                            let borrow = (rm_val as u32) < (imm as u32);
                            let overflow = ((rm_val ^ imm) & (rm_val ^ r) & 0x8000) != 0;
                            let af = Self::calc_af_sub_16(rm_val, imm);
                            self.set_flag(FLAG_CF, borrow);
                            self.set_flag(FLAG_OF, overflow);
                            self.set_flag(FLAG_AF, af);
                            r
                        }
                        6 => {
                            // XOR
                            let r = rm_val ^ imm;
                            self.set_flag(FLAG_CF, false);
                            self.set_flag(FLAG_OF, false);
                            r
                        }
                        7 => {
                            // CMP
                            let r = rm_val.wrapping_sub(imm);
                            let borrow = (rm_val as u32) < (imm as u32);
                            let overflow = ((rm_val ^ imm) & (rm_val ^ r) & 0x8000) != 0;
                            let af = Self::calc_af_sub_16(rm_val, imm);
                            self.update_flags_16(r);
                            self.set_flag(FLAG_CF, borrow);
                            self.set_flag(FLAG_OF, overflow);
                            self.set_flag(FLAG_AF, af);
                            self.cycles += if modbits == 0b11 { 4 } else { 17 };
                            return if modbits == 0b11 { 4 } else { 17 };
                        }
                        _ => unreachable!(),
                    };
                    if op != 7 {
                        self.write_rmw16(modbits, rm, result, cached_seg, cached_offset);
                        self.update_flags_16(result);
                    }
                    self.cycles += if modbits == 0b11 { 4 } else { 17 };
                    if modbits == 0b11 {
                        4
                    } else {
                        17
                    }
                } else {
                    // 16-bit operand size (normal mode)
                    let (rm_val, cached_seg, cached_offset) = self.read_rmw16(modbits, rm);
                    let imm = self.fetch_u16();
                    let result = match op {
                        0 => {
                            // ADD
                            let r = rm_val.wrapping_add(imm);
                            let carry = (rm_val as u32 + imm as u32) > 0xFFFF;
                            let overflow = ((rm_val ^ r) & (imm ^ r) & 0x8000) != 0;
                            let af = Self::calc_af_add_16(rm_val, imm);
                            self.set_flag(FLAG_CF, carry);
                            self.set_flag(FLAG_OF, overflow);
                            self.set_flag(FLAG_AF, af);
                            r
                        }
                        1 => {
                            // OR
                            let r = rm_val | imm;
                            self.set_flag(FLAG_CF, false);
                            self.set_flag(FLAG_OF, false);
                            r
                        }
                        2 => {
                            // ADC
                            let carry_in = if self.get_flag(FLAG_CF) { 1 } else { 0 };
                            let r = rm_val.wrapping_add(imm).wrapping_add(carry_in);
                            let carry = (rm_val as u32 + imm as u32 + carry_in as u32) > 0xFFFF;
                            let overflow = ((rm_val ^ r) & (imm ^ r) & 0x8000) != 0;
                            let af = (((rm_val & 0x0F) + (imm & 0x0F) + carry_in) & 0x10) != 0;
                            self.set_flag(FLAG_CF, carry);
                            self.set_flag(FLAG_OF, overflow);
                            self.set_flag(FLAG_AF, af);
                            r
                        }
                        3 => {
                            // SBB
                            let carry_in = if self.get_flag(FLAG_CF) { 1 } else { 0 };
                            let r = rm_val.wrapping_sub(imm).wrapping_sub(carry_in);
                            let borrow = (rm_val as u32) < (imm as u32 + carry_in as u32);
                            let overflow = ((rm_val ^ imm) & (rm_val ^ r) & 0x8000) != 0;
                            let af = (rm_val & 0x0F) < ((imm & 0x0F) + carry_in);
                            self.set_flag(FLAG_CF, borrow);
                            self.set_flag(FLAG_OF, overflow);
                            self.set_flag(FLAG_AF, af);
                            r
                        }
                        4 => {
                            // AND
                            let r = rm_val & imm;
                            self.set_flag(FLAG_CF, false);
                            self.set_flag(FLAG_OF, false);
                            r
                        }
                        5 => {
                            // SUB
                            let r = rm_val.wrapping_sub(imm);
                            let borrow = (rm_val as u32) < (imm as u32);
                            let overflow = ((rm_val ^ imm) & (rm_val ^ r) & 0x8000) != 0;
                            let af = Self::calc_af_sub_16(rm_val, imm);
                            self.set_flag(FLAG_CF, borrow);
                            self.set_flag(FLAG_OF, overflow);
                            self.set_flag(FLAG_AF, af);
                            r
                        }
                        6 => {
                            // XOR
                            let r = rm_val ^ imm;
                            self.set_flag(FLAG_CF, false);
                            self.set_flag(FLAG_OF, false);
                            r
                        }
                        7 => {
                            // CMP
                            let r = rm_val.wrapping_sub(imm);
                            let borrow = (rm_val as u32) < (imm as u32);
                            let overflow = ((rm_val ^ imm) & (rm_val ^ r) & 0x8000) != 0;
                            let af = Self::calc_af_sub_16(rm_val, imm);
                            self.update_flags_16(r);
                            self.set_flag(FLAG_CF, borrow);
                            self.set_flag(FLAG_OF, overflow);
                            self.set_flag(FLAG_AF, af);
                            self.cycles += if modbits == 0b11 { 4 } else { 17 };
                            return if modbits == 0b11 { 4 } else { 17 };
                        }
                        _ => unreachable!(),
                    };
                    if op != 7 {
                        self.write_rmw16(modbits, rm, result, cached_seg, cached_offset);
                        self.update_flags_16(result);
                    }
                    self.cycles += if modbits == 0b11 { 4 } else { 17 };
                    if modbits == 0b11 {
                        4
                    } else {
                        17
                    }
                }
            }

            // 0x82 - Same as 0x80 (alias for 8086)
            0x82 => {
                self.ip = self.ip.wrapping_sub(1); // Back up and execute as 0x80
                self.step()
            }

            // 0x83 - r/m16, imm8 (sign-extended)
            0x83 => {
                let modrm = self.fetch_u8();
                let (modbits, op, rm) = Self::decode_modrm(modrm);
                let (rm_val, cached_seg, cached_offset) = self.read_rmw16(modbits, rm);
                let imm = self.fetch_u8() as i8 as i16 as u16; // Sign extend
                let result = match op {
                    0 => {
                        // ADD
                        let r = rm_val.wrapping_add(imm);
                        let carry = (rm_val as u32 + imm as u32) > 0xFFFF;
                        let overflow = ((rm_val ^ r) & (imm ^ r) & 0x8000) != 0;
                        let af = Self::calc_af_add_16(rm_val, imm);
                        self.set_flag(FLAG_CF, carry);
                        self.set_flag(FLAG_OF, overflow);
                        self.set_flag(FLAG_AF, af);
                        r
                    }
                    1 => {
                        // OR
                        let r = rm_val | imm;
                        self.set_flag(FLAG_CF, false);
                        self.set_flag(FLAG_OF, false);
                        r
                    }
                    2 => {
                        // ADC
                        let carry_in = if self.get_flag(FLAG_CF) { 1 } else { 0 };
                        let r = rm_val.wrapping_add(imm).wrapping_add(carry_in);
                        let carry = (rm_val as u32 + imm as u32 + carry_in as u32) > 0xFFFF;
                        let overflow = ((rm_val ^ r) & (imm ^ r) & 0x8000) != 0;
                        let af = (((rm_val & 0x0F) + (imm & 0x0F) + carry_in) & 0x10) != 0;
                        self.set_flag(FLAG_CF, carry);
                        self.set_flag(FLAG_OF, overflow);
                        self.set_flag(FLAG_AF, af);
                        r
                    }
                    3 => {
                        // SBB
                        let carry_in = if self.get_flag(FLAG_CF) { 1 } else { 0 };
                        let r = rm_val.wrapping_sub(imm).wrapping_sub(carry_in);
                        let borrow = (rm_val as u32) < (imm as u32 + carry_in as u32);
                        let overflow = ((rm_val ^ imm) & (rm_val ^ r) & 0x8000) != 0;
                        let af = (rm_val & 0x0F) < ((imm & 0x0F) + carry_in);
                        self.set_flag(FLAG_CF, borrow);
                        self.set_flag(FLAG_OF, overflow);
                        self.set_flag(FLAG_AF, af);
                        r
                    }
                    4 => {
                        // AND
                        let r = rm_val & imm;
                        self.set_flag(FLAG_CF, false);
                        self.set_flag(FLAG_OF, false);
                        r
                    }
                    5 => {
                        // SUB
                        let r = rm_val.wrapping_sub(imm);
                        let borrow = (rm_val as u32) < (imm as u32);
                        let overflow = ((rm_val ^ imm) & (rm_val ^ r) & 0x8000) != 0;
                        let af = Self::calc_af_sub_16(rm_val, imm);
                        self.set_flag(FLAG_CF, borrow);
                        self.set_flag(FLAG_OF, overflow);
                        self.set_flag(FLAG_AF, af);
                        r
                    }
                    6 => {
                        // XOR
                        let r = rm_val ^ imm;
                        self.set_flag(FLAG_CF, false);
                        self.set_flag(FLAG_OF, false);
                        r
                    }
                    7 => {
                        // CMP
                        let r = rm_val.wrapping_sub(imm);
                        let borrow = (rm_val as u32) < (imm as u32);
                        let overflow = ((rm_val ^ imm) & (rm_val ^ r) & 0x8000) != 0;
                        let af = Self::calc_af_sub_16(rm_val, imm);
                        self.update_flags_16(r);
                        self.set_flag(FLAG_CF, borrow);
                        self.set_flag(FLAG_OF, overflow);
                        self.set_flag(FLAG_AF, af);
                        self.cycles += if modbits == 0b11 { 4 } else { 17 };
                        return if modbits == 0b11 { 4 } else { 17 };
                    }
                    _ => unreachable!(),
                };
                if op != 7 {
                    self.write_rmw16(modbits, rm, result, cached_seg, cached_offset);
                    self.update_flags_16(result);
                }
                self.cycles += if modbits == 0b11 { 4 } else { 17 };
                if modbits == 0b11 {
                    4
                } else {
                    17
                }
            }

            // TEST r/m8, r8 (0x84)
            0x84 => {
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);
                let reg_val = if reg < 4 {
                    self.get_reg8_low(reg)
                } else {
                    self.get_reg8_high(reg - 4)
                };
                let rm_val = self.read_rm8(modbits, rm);
                let result = rm_val & reg_val;

                self.update_flags_8(result);
                self.set_flag(FLAG_CF, false);
                self.set_flag(FLAG_OF, false);
                self.cycles += if modbits == 0b11 { 3 } else { 9 };
                if modbits == 0b11 {
                    3
                } else {
                    9
                }
            }

            // TEST r/m16, r16 (0x85)
            0x85 => {
                // TEST r/m16/32, r16/32
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);

                if self.operand_size_override && self.model.supports_80386_instructions() {
                    // 32-bit operation
                    let reg_val = self.get_reg32(reg);
                    let rm_val = self.read_rm32(modbits, rm);
                    let result = rm_val & reg_val;

                    self.update_flags_32(result);
                    self.set_flag(FLAG_CF, false);
                    self.set_flag(FLAG_OF, false);
                    self.cycles += if modbits == 0b11 { 3 } else { 9 };
                    if modbits == 0b11 {
                        3
                    } else {
                        9
                    }
                } else {
                    // 16-bit operation
                    let reg_val = self.get_reg16(reg);
                    let rm_val = self.read_rm16(modbits, rm);
                    let result = rm_val & reg_val;

                    self.update_flags_16(result as u16);
                    self.set_flag(FLAG_CF, false);
                    self.set_flag(FLAG_OF, false);
                    self.cycles += if modbits == 0b11 { 3 } else { 9 };
                    if modbits == 0b11 {
                        3
                    } else {
                        9
                    }
                }
            }

            // XCHG r8, r/m8 (0x86)
            0x86 => {
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);
                let reg_val = if reg < 4 {
                    self.get_reg8_low(reg)
                } else {
                    self.get_reg8_high(reg - 4)
                };
                // Use RMW helpers to avoid double-fetching displacement
                let (rm_val, seg, offset) = self.read_rmw8(modbits, rm);

                if reg < 4 {
                    self.set_reg8_low(reg, rm_val);
                } else {
                    self.set_reg8_high(reg - 4, rm_val);
                }
                self.write_rmw8(modbits, rm, reg_val, seg, offset);

                self.cycles += if modbits == 0b11 { 4 } else { 17 };
                if modbits == 0b11 {
                    4
                } else {
                    17
                }
            }

            // XCHG r16, r/m16 (0x87)
            0x87 => {
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);
                let reg_val = self.get_reg16(reg);
                // Use RMW helpers to avoid double-fetching displacement
                let (rm_val, seg, offset) = self.read_rmw16(modbits, rm);

                self.set_reg16(reg, rm_val);
                self.write_rmw16(modbits, rm, reg_val, seg, offset);

                self.cycles += if modbits == 0b11 { 4 } else { 17 };
                if modbits == 0b11 {
                    4
                } else {
                    17
                }
            }

            // LEA - Load Effective Address (0x8D)
            0x8D => {
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);
                // LEA only works with memory operands (not register mode)
                if modbits != 0b11 {
                    let offset_ea = self.calc_effective_offset(modbits, rm);
                    self.set_reg16(reg, offset_ea);
                }
                self.cycles += 2;
                2
            }

            // XCHG with AX (0x91-0x97)
            0x91..=0x97 => {
                let reg = opcode & 0x07;
                let temp = self.ax;
                self.ax = self.get_reg16(reg) as u32;
                self.set_reg16(reg, temp as u16);
                self.cycles += 3;
                3
            }

            // CBW - Convert Byte to Word (0x98)
            0x98 => {
                let al = (self.ax & 0xFF) as u8;
                let sign_extend = if al & 0x80 != 0 { 0xFF00 } else { 0x0000 };
                self.ax = (self.ax & 0x00FF) | sign_extend;
                self.cycles += 2;
                2
            }

            // CWD - Convert Word to Doubleword (0x99)
            0x99 => {
                let sign_extend = if self.ax & 0x8000 != 0 {
                    0xFFFF
                } else {
                    0x0000
                };
                self.dx = sign_extend as u32;
                self.cycles += 5;
                5
            }

            // WAIT (0x9B)
            0x9B => {
                // WAIT instruction - normally waits for FPU
                // For basic emulation, just consume cycles
                self.cycles += 3;
                3
            }

            // PUSHF - Push Flags (0x9C)
            0x9C => {
                self.push(self.flags as u16);
                self.cycles += 10;
                10
            }

            // POPF - Pop Flags (0x9D)
            0x9D => {
                self.flags = self.pop() as u32;
                self.cycles += 8;
                8
            }

            // SAHF - Store AH into Flags (0x9E)
            0x9E => {
                let ah = ((self.ax >> 8) & 0xFF) as u8;
                self.flags = (self.flags & 0xFFFF_FF00) | (ah as u32);
                self.cycles += 4;
                4
            }

            // LAHF - Load AH from Flags (0x9F)
            0x9F => {
                let flags_low = (self.flags & 0xFF) as u8;
                self.ax = (self.ax & 0xFFFF_FF00) | ((flags_low as u32) << 8);
                self.cycles += 4;
                4
            }

            // MOV AL, moffs8 (0xA0) - Direct memory to AL
            0xA0 => {
                let addr = self.fetch_u16();
                let seg = self.get_segment_with_override(self.ds);
                let val = self.read(seg, addr);
                self.ax = (self.ax & 0xFFFF_FF00) | (val as u32);
                self.cycles += 10;
                10
            }

            // MOV AX, moffs16 (0xA1) - Direct memory to AX
            0xA1 => {
                let addr = self.fetch_u16();
                let seg = self.get_segment_with_override(self.ds);
                let val = self.read_u16(seg, addr);
                self.ax = val as u32;
                self.cycles += 10;
                10
            }

            // MOV moffs8, AL (0xA2) - AL to direct memory
            0xA2 => {
                let addr = self.fetch_u16();
                let seg = self.get_segment_with_override(self.ds);
                let al = (self.ax & 0xFF) as u8;
                self.write(seg, addr, al);
                self.cycles += 10;
                10
            }

            // MOV moffs16, AX (0xA3) - AX to direct memory
            0xA3 => {
                let addr = self.fetch_u16();
                let seg = self.get_segment_with_override(self.ds);
                self.write_u16(seg, addr, self.ax as u16);
                self.cycles += 10;
                10
            }

            // TEST AL, imm8 (0xA8)
            0xA8 => {
                let val = self.fetch_u8();
                let al = (self.ax & 0xFF) as u8;
                let result = al & val;

                self.update_flags_8(result);
                self.set_flag(FLAG_CF, false);
                self.set_flag(FLAG_OF, false);
                self.cycles += 4;
                4
            }

            // TEST AX, imm16 (0xA9)
            0xA9 => {
                let val = self.fetch_u16();
                let result = (self.ax as u16) & val;

                self.update_flags_16(result);
                self.set_flag(FLAG_CF, false);
                self.set_flag(FLAG_OF, false);
                self.cycles += 4;
                4
            }

            // Group 3 opcodes (0xF6) - 8-bit operations (NOT, NEG, MUL, DIV, etc.)
            0xF6 => {
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);

                match reg {
                    // TEST r/m8, imm8 (subopcode 0 and 1 are both TEST)
                    0b000 | 0b001 => {
                        let val = self.read_rm8(modbits, rm);
                        let imm = self.fetch_u8();
                        let result = val & imm;
                        self.update_flags_8(result);
                        self.set_flag(FLAG_CF, false);
                        self.set_flag(FLAG_OF, false);
                        self.cycles += if modbits == 0b11 { 5 } else { 11 };
                        if modbits == 0b11 {
                            5
                        } else {
                            11
                        }
                    }
                    // NOT r/m8 - use RMW helpers to avoid double-fetching displacement
                    0b010 => {
                        let (val, seg, offset) = self.read_rmw8(modbits, rm);
                        let result = !val;
                        self.write_rmw8(modbits, rm, result, seg, offset);
                        self.cycles += if modbits == 0b11 { 3 } else { 16 };
                        if modbits == 0b11 {
                            3
                        } else {
                            16
                        }
                    }
                    // NEG r/m8 - use RMW helpers to avoid double-fetching displacement
                    0b011 => {
                        let (val, seg, offset) = self.read_rmw8(modbits, rm);
                        let result = 0u8.wrapping_sub(val);
                        // AF calculation for NEG: check if borrow from bit 4 to bit 3
                        let af = (val & 0x0F) != 0;
                        self.write_rmw8(modbits, rm, result, seg, offset);
                        self.update_flags_8(result);
                        self.set_flag(FLAG_CF, val != 0);
                        self.set_flag(FLAG_OF, val == 0x80);
                        self.set_flag(FLAG_AF, af);
                        self.cycles += if modbits == 0b11 { 3 } else { 16 };
                        if modbits == 0b11 {
                            3
                        } else {
                            16
                        }
                    }
                    // MUL r/m8 (unsigned multiply AL by r/m8, result in AX)
                    0b100 => {
                        let val = self.read_rm8(modbits, rm);
                        let al = (self.ax & 0xFF) as u8;
                        let result = (al as u16) * (val as u16);
                        self.ax = (self.ax & 0xFFFF_0000) | (result as u32);
                        // CF and OF are set if AH is non-zero
                        let high_byte_set = (result & 0xFF00) != 0;
                        self.set_flag(FLAG_CF, high_byte_set);
                        self.set_flag(FLAG_OF, high_byte_set);
                        // SF, ZF, PF are undefined but we'll update them
                        self.update_flags_16(result);
                        self.cycles += if modbits == 0b11 { 70 } else { 76 };
                        if modbits == 0b11 {
                            70
                        } else {
                            76
                        }
                    }
                    // IMUL r/m8 (signed multiply AL by r/m8, result in AX)
                    0b101 => {
                        let val = self.read_rm8(modbits, rm) as i8;
                        let al = (self.ax & 0xFF) as i8;
                        let result = (al as i16) * (val as i16);
                        self.ax = (self.ax & 0xFFFF_0000) | (result as u32);
                        // CF and OF are set if sign extension of AL != AH
                        let sign_extended = (al as i16) as u16;
                        let high_byte_set = (result as u16) != sign_extended;
                        self.set_flag(FLAG_CF, high_byte_set);
                        self.set_flag(FLAG_OF, high_byte_set);
                        self.update_flags_16(result as u16);
                        self.cycles += if modbits == 0b11 { 80 } else { 86 };
                        if modbits == 0b11 {
                            80
                        } else {
                            86
                        }
                    }
                    // DIV r/m8 (unsigned divide AX by r/m8, quotient in AL, remainder in AH)
                    0b110 => {
                        let divisor = self.read_rm8(modbits, rm);
                        if divisor == 0 {
                            // Division by zero - trigger INT 0 as exception
                            self.trigger_interrupt(0, true);
                        } else {
                            let dividend = (self.ax & 0xFFFF) as u16;
                            let quotient = dividend / (divisor as u16);
                            let remainder = dividend % (divisor as u16);
                            // Check for overflow (quotient > 255)
                            if quotient > 0xFF {
                                // Division overflow - trigger INT 0 as exception
                                self.trigger_interrupt(0, true);
                            } else {
                                self.ax = ((remainder as u32) << 8) | (quotient as u32);
                            }
                        }
                        self.cycles += if modbits == 0b11 { 80 } else { 86 };
                        if modbits == 0b11 {
                            80
                        } else {
                            86
                        }
                    }
                    // IDIV r/m8 (signed divide AX by r/m8, quotient in AL, remainder in AH)
                    0b111 => {
                        let divisor = self.read_rm8(modbits, rm) as i8;
                        if divisor == 0 {
                            // Division by zero - trigger INT 0 as exception
                            self.trigger_interrupt(0, true);
                        } else {
                            let dividend = self.ax as i16;
                            let quotient = dividend / (divisor as i16);
                            let remainder = dividend % (divisor as i16);
                            // Check for overflow (quotient out of -128..127 range)
                            if !(-128..=127).contains(&quotient) {
                                // Division overflow - trigger INT 0 as exception
                                self.trigger_interrupt(0, true);
                            } else {
                                let quot_u8 = quotient as u8;
                                let rem_u8 = remainder as u8;
                                self.ax = (((rem_u8 as u16) << 8) | (quot_u8 as u16)) as u32;
                            }
                        }
                        self.cycles += if modbits == 0b11 { 101 } else { 107 };
                        if modbits == 0b11 {
                            101
                        } else {
                            107
                        }
                    }
                    _ => {
                        eprintln!(
                            "Unimplemented 0xF6 subopcode: {} at CS:IP={:04X}:{:04X}",
                            reg,
                            self.cs,
                            self.ip.wrapping_sub(2u32)
                        );
                        self.cycles += 1;
                        1
                    }
                }
            }

            // Group 3 opcodes (0xF7) - 16-bit operations (NOT, NEG, MUL, DIV, etc.)
            0xF7 => {
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);

                match reg {
                    // TEST r/m16, imm16 (reg=0 or reg=1)
                    0b000 | 0b001 => {
                        let val = self.read_rm16(modbits, rm);
                        let imm = self.fetch_u16();
                        let result = val & imm;
                        self.update_flags_16(result as u16);
                        self.set_flag(FLAG_CF, false);
                        self.set_flag(FLAG_OF, false);
                        self.cycles += if modbits == 0b11 { 5 } else { 11 };
                        if modbits == 0b11 {
                            5
                        } else {
                            11
                        }
                    }
                    // NOT r/m16 - use RMW helpers to avoid double-fetching displacement
                    0b010 => {
                        let (val, seg, offset) = self.read_rmw16(modbits, rm);
                        let result = !val;
                        self.write_rmw16(modbits, rm, result, seg, offset);
                        self.cycles += if modbits == 0b11 { 3 } else { 16 };
                        if modbits == 0b11 {
                            3
                        } else {
                            16
                        }
                    }
                    // NEG r/m16 - use RMW helpers to avoid double-fetching displacement
                    0b011 => {
                        let (val, seg, offset) = self.read_rmw16(modbits, rm);
                        let result = 0u16.wrapping_sub(val);
                        // AF calculation for NEG: check if borrow from bit 4 to bit 3 in low byte
                        let af = (val & 0x0F) != 0;
                        self.write_rmw16(modbits, rm, result, seg, offset);
                        self.update_flags_16(result as u16);
                        self.set_flag(FLAG_CF, val != 0);
                        self.set_flag(FLAG_OF, val == 0x8000);
                        self.set_flag(FLAG_AF, af);
                        self.cycles += if modbits == 0b11 { 3 } else { 16 };
                        if modbits == 0b11 {
                            3
                        } else {
                            16
                        }
                    }
                    // MUL r/m16 (unsigned multiply AX by r/m16, result in DX:AX)
                    0b100 => {
                        let val = self.read_rm16(modbits, rm);
                        let result = (self.ax as u16 as u32) * (val as u32);
                        self.ax = (self.ax & 0xFFFF_0000) | ((result & 0xFFFF) as u32);
                        self.dx = (self.dx & 0xFFFF_0000) | (((result >> 16) & 0xFFFF) as u32);
                        // CF and OF are set if DX is non-zero
                        let high_word_set = self.dx != 0;
                        self.set_flag(FLAG_CF, high_word_set);
                        self.set_flag(FLAG_OF, high_word_set);
                        self.update_flags_16(self.ax as u16);
                        self.cycles += if modbits == 0b11 { 118 } else { 124 };
                        if modbits == 0b11 {
                            118
                        } else {
                            124
                        }
                    }
                    // IMUL r/m16 (signed multiply AX by r/m16, result in DX:AX)
                    0b101 => {
                        let val = self.read_rm16(modbits, rm) as i16;
                        let ax_signed = (self.ax as u16) as i16;
                        let result = (ax_signed as i32) * (val as i32);
                        self.ax = (self.ax & 0xFFFF_0000) | ((result & 0xFFFF) as u32);
                        self.dx = (self.dx & 0xFFFF_0000) | (((result >> 16) & 0xFFFF) as u32);
                        // CF and OF are set if sign extension of AX != DX
                        let sign_extended = if (self.ax & 0x8000) != 0 {
                            0xFFFF
                        } else {
                            0x0000
                        };
                        let overflow = self.dx != sign_extended;
                        self.set_flag(FLAG_CF, overflow);
                        self.set_flag(FLAG_OF, overflow);
                        self.update_flags_16(self.ax as u16);
                        self.cycles += if modbits == 0b11 { 128 } else { 134 };
                        if modbits == 0b11 {
                            128
                        } else {
                            134
                        }
                    }
                    // DIV r/m16 (unsigned divide DX:AX by r/m16, quotient in AX, remainder in DX)
                    0b110 => {
                        let divisor = self.read_rm16(modbits, rm);
                        if divisor == 0 {
                            // Division by zero - trigger INT 0 as exception
                            self.trigger_interrupt(0, true);
                        } else {
                            let dividend = (self.dx << 16) | self.ax;
                            let quotient = dividend / (divisor as u32);
                            let remainder = dividend % (divisor as u32);
                            // Check for overflow (quotient > 65535)
                            if quotient > 0xFFFF {
                                // Division overflow - trigger INT 0 as exception
                                self.trigger_interrupt(0, true);
                            } else {
                                self.ax = quotient as u16 as u32;
                                self.dx = remainder as u16 as u32;
                            }
                        }
                        self.cycles += if modbits == 0b11 { 144 } else { 150 };
                        if modbits == 0b11 {
                            144
                        } else {
                            150
                        }
                    }
                    // IDIV r/m16 (signed divide DX:AX by r/m16, quotient in AX, remainder in DX)
                    0b111 => {
                        let divisor = self.read_rm16(modbits, rm) as i16;
                        if divisor == 0 {
                            // Division by zero - trigger INT 0 as exception
                            self.trigger_interrupt(0, true);
                        } else {
                            let dividend = ((self.dx << 16) | self.ax) as i32;
                            let quotient = dividend / (divisor as i32);
                            let remainder = dividend % (divisor as i32);
                            // Check for overflow (quotient out of -32768..32767 range)
                            if !(-32768..=32767).contains(&quotient) {
                                // Division overflow - trigger INT 0 as exception
                                self.trigger_interrupt(0, true);
                            } else {
                                self.ax = quotient as u16 as u32;
                                self.dx = remainder as u16 as u32;
                            }
                        }
                        self.cycles += if modbits == 0b11 { 165 } else { 171 };
                        if modbits == 0b11 {
                            165
                        } else {
                            171
                        }
                    }
                    _ => {
                        eprintln!(
                            "Unimplemented 0xF7 subopcode: {} at CS:IP={:04X}:{:04X}",
                            reg,
                            self.cs,
                            self.ip.wrapping_sub(2u32)
                        );
                        self.cycles += 1;
                        1
                    }
                }
            }

            // LES - Load pointer to ES (0xC4)
            0xC4 => {
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);
                // LES only works with memory operands
                if modbits != 0b11 {
                    let (seg, offset_ea, _) = self.calc_effective_address(modbits, rm);
                    let offset = self.read_u16(seg, offset_ea);
                    let segment = self.read_u16(seg, offset_ea.wrapping_add(2));
                    self.set_reg16(reg, offset);
                    self.es = segment;
                }
                self.cycles += 16;
                16
            }

            // LDS - Load pointer to DS (0xC5)
            0xC5 => {
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);
                // LDS only works with memory operands
                if modbits != 0b11 {
                    let (seg, offset_ea, _) = self.calc_effective_address(modbits, rm);
                    let offset = self.read_u16(seg, offset_ea);
                    let segment = self.read_u16(seg, offset_ea.wrapping_add(2));
                    self.set_reg16(reg, offset);
                    self.ds = segment;
                }
                self.cycles += 16;
                16
            }

            // MOV r/m8, imm8 (0xC6) - Group 11
            // MOV r/m8, imm8 (0xC6) - Group 11
            0xC6 => {
                let modrm = self.fetch_u8();
                let (modbits, op, rm) = Self::decode_modrm(modrm);
                if op == 0 {
                    // Only op=0 is valid for MOV
                    // IMPORTANT: For memory operands with displacement, we must calculate
                    // the effective address (which fetches displacement bytes) BEFORE
                    // fetching the immediate value. So we handle register and memory cases separately.
                    if modbits == 0b11 {
                        // Register mode - no displacement to fetch
                        let imm = self.fetch_u8();
                        self.write_rm8(modbits, rm, imm);
                        self.cycles += 4;
                        4
                    } else {
                        // Memory mode - get effective address first to consume displacement bytes
                        let (seg, offset, _) = self.calc_effective_address(modbits, rm);
                        let imm = self.fetch_u8(); // Now fetch immediate after displacement
                        self.write(seg, offset, imm);
                        self.cycles += 10;
                        10
                    }
                } else {
                    // Undefined - consume bytes to prevent desync
                    // Calculate effective address to consume any displacement bytes
                    if modbits != 0b11 {
                        let _ = self.calc_effective_address(modbits, rm);
                    }
                    // Consume immediate byte
                    let _ = self.fetch_u8();
                    eprintln!(
                        "Undefined 0xC6 operation with op={} at CS:IP={:04X}:{:04X}",
                        op,
                        self.cs,
                        self.ip.wrapping_sub(2u32)
                    );
                    self.cycles += 1;
                    1
                }
            }

            // MOV r/m16, imm16 (0xC7) - Group 11
            // With 0x66 prefix (80386+): MOV r/m32, imm32
            0xC7 => {
                let modrm = self.fetch_u8();
                let (modbits, op, rm) = Self::decode_modrm(modrm);
                if op == 0 {
                    // Only op=0 is valid for MOV
                    // IMPORTANT: For memory operands with displacement, we must calculate
                    // the effective address (which fetches displacement bytes) BEFORE
                    // fetching the immediate value.

                    // Check if operand-size override (0x66) prefix is active
                    if self.operand_size_override && self.model.supports_80386_instructions() {
                        // 32-bit operand size: MOV r/m32, imm32
                        if modbits == 0b11 {
                            // Register mode - no displacement to fetch
                            let imm32 = self.fetch_u32();
                            self.write_rm32(modbits, rm, imm32);
                            self.cycles += 4;
                            4
                        } else {
                            // Memory mode - get effective address first to consume displacement bytes
                            let (seg, offset, _) = if self.address_size_override {
                                self.calc_effective_address_32(modbits, rm)
                            } else {
                                let (s, o, b) = self.calc_effective_address(modbits, rm);
                                (s, o as u32, b)
                            };
                            let imm32 = self.fetch_u32(); // Fetch immediate after displacement
                            self.write_u32(seg, offset, imm32);
                            self.cycles += 10;
                            10
                        }
                    } else {
                        // 16-bit operand size: MOV r/m16, imm16
                        if modbits == 0b11 {
                            // Register mode - no displacement to fetch
                            let imm = self.fetch_u16();
                            self.write_rm16(modbits, rm, imm);
                            self.cycles += 4;
                            4
                        } else {
                            // Memory mode - get effective address first to consume displacement bytes
                            let (seg, offset, _) = self.calc_effective_address(modbits, rm);
                            let imm = self.fetch_u16(); // Now fetch immediate after displacement
                            self.write_u16(seg, offset, imm);
                            self.cycles += 10;
                            10
                        }
                    }
                } else {
                    // Undefined operation - For opcode 0xC7 (Group 11), only op=0 is defined for MOV
                    // Consume bytes for this invalid 0xC7 instruction to prevent desync
                    // Calculate effective address to consume any displacement bytes
                    if modbits != 0b11 {
                        let _ = self.calc_effective_address(modbits, rm);
                    }
                    // Consume immediate bytes based on operand size
                    if self.operand_size_override && self.model.supports_80386_instructions() {
                        let _ = self.fetch_u32();
                    } else {
                        let _ = self.fetch_u16();
                    }
                    // Note: This is likely invalid code; treat as NOP to continue execution
                    eprintln!(
                        "Undefined 0xC7 operation with op={} at CS:IP={:04X}:{:04X} - treating as NOP",
                        op,
                        self.cs,
                        self.ip.wrapping_sub(2u32)
                    );
                    self.cycles += 1;
                    1
                }
            }

            // Group 2 opcodes (0xC0) - Shift/rotate r/m8 by immediate byte (80186+)
            0xC0 => {
                if !self.model.supports_80186_instructions() {
                    // Invalid opcode on 8086/8088
                    self.cycles += 10;
                    return 10;
                }
                let modrm = self.fetch_u8();
                let (modbits, op, rm) = Self::decode_modrm(modrm);
                let count = self.fetch_u8();
                // Use RMW helpers to avoid double-fetching displacement
                let (val, seg, offset) = self.read_rmw8(modbits, rm);
                let result = self.shift_rotate_8(val, op, count);
                self.write_rmw8(modbits, rm, result, seg, offset);
                self.cycles += if modbits == 0b11 {
                    5 + (4 * count as u64)
                } else {
                    17 + (4 * count as u64)
                };
                if modbits == 0b11 {
                    5 + (4 * count as u32)
                } else {
                    17 + (4 * count as u32)
                }
            }

            // Group 2 opcodes (0xC1) - Shift/rotate r/m16 by immediate byte (80186+)
            0xC1 => {
                if !self.model.supports_80186_instructions() {
                    // Invalid opcode on 8086/8088
                    self.cycles += 10;
                    return 10;
                }
                let modrm = self.fetch_u8();
                let (modbits, op, rm) = Self::decode_modrm(modrm);
                let count = self.fetch_u8();
                // Use RMW helpers to avoid double-fetching displacement
                let (val, seg, offset) = self.read_rmw16(modbits, rm);
                let result = self.shift_rotate_16(val, op, count);
                self.write_rmw16(modbits, rm, result, seg, offset);
                self.cycles += if modbits == 0b11 {
                    5 + (4 * count as u64)
                } else {
                    17 + (4 * count as u64)
                };
                if modbits == 0b11 {
                    5 + (4 * count as u32)
                } else {
                    17 + (4 * count as u32)
                }
            }

            // ENTER (0xC8) - 80186+ instruction
            0xC8 => {
                if !self.model.supports_80186_instructions() {
                    // Invalid opcode on 8086/8088
                    self.cycles += 10;
                    return 10;
                }
                // Debug: check what bytes we're about to read
                if LogConfig::global().should_log(LogCategory::CPU, LogLevel::Trace) {
                    let ip_before = self.ip;
                    let byte1 = self.read(self.cs, (ip_before) as u16);
                    let byte2 = self.read(self.cs, (ip_before.wrapping_add(1)) as u16);
                    let byte3 = self.read(self.cs, (ip_before.wrapping_add(2u32)) as u16);
                    let phys_start = ((self.cs as u32) << 4) + (ip_before as u32);
                    eprintln!(
                        "[ENTER DEBUG] CS:IP={:04X}:{:04X}, physical=0x{:05X}",
                        self.cs, ip_before, phys_start
                    );
                    eprintln!(
                        "[ENTER DEBUG] Next 3 bytes in memory: {:02X} {:02X} {:02X}",
                        byte1, byte2, byte3
                    );
                    eprintln!(
                        "[ENTER DEBUG] Will read as: size=0x{:02X}{:02X}, nesting=0x{:02X}",
                        byte2, byte1, byte3
                    );
                }

                let size = self.fetch_u16();
                let _nesting = self.fetch_u8();

                if LogConfig::global().should_log(LogCategory::CPU, LogLevel::Trace) {
                    eprintln!(
                        "[ENTER] BP before={:04X}, SP before={:04X}, size={:04X}, nesting={:02X}",
                        self.bp, self.sp, size, _nesting
                    );
                }

                // Simplified implementation
                self.push(self.bp as u16);
                let frame_temp = self.sp;
                self.bp = frame_temp;
                self.sp = self.sp.wrapping_sub(size as u32);

                if LogConfig::global().should_log(LogCategory::CPU, LogLevel::Trace) {
                    eprintln!("[ENTER] BP after={:04X}, SP after={:04X}", self.bp, self.sp);
                }

                self.cycles += 15;
                15
            }

            // LEAVE (0xC9) - 80186+ instruction
            0xC9 => {
                if !self.model.supports_80186_instructions() {
                    // Invalid opcode on 8086/8088
                    self.cycles += 10;
                    return 10;
                }
                self.sp = self.bp;
                self.bp = (self.bp & 0xFFFF_0000) | (self.pop() as u32);
                self.cycles += 8;
                8
            }

            // INT 3 (0xCC) - Breakpoint interrupt
            0xCC => {
                // Software interrupt - use current IP
                self.trigger_interrupt(3, false);
                self.cycles += 52;
                52
            }

            // INTO - Interrupt on Overflow (0xCE)
            0xCE => {
                if self.get_flag(FLAG_OF) {
                    // Software interrupt - use current IP
                    self.trigger_interrupt(4, false);
                    self.cycles += 53;
                    53
                } else {
                    self.cycles += 4;
                    4
                }
            }

            // Group 2 opcodes (0xD0) - Shift/rotate r/m8 by 1
            0xD0 => {
                let modrm = self.fetch_u8();
                let (modbits, op, rm) = Self::decode_modrm(modrm);
                // Use RMW helpers to avoid double-fetching displacement
                let (val, seg, offset) = self.read_rmw8(modbits, rm);
                let result = self.shift_rotate_8(val, op, 1);
                self.write_rmw8(modbits, rm, result, seg, offset);
                self.cycles += if modbits == 0b11 { 2 } else { 15 };
                if modbits == 0b11 {
                    2
                } else {
                    15
                }
            }

            // Group 2 opcodes (0xD1) - Shift/rotate r/m16 by 1
            0xD1 => {
                let modrm = self.fetch_u8();
                let (modbits, op, rm) = Self::decode_modrm(modrm);
                // Use RMW helpers to avoid double-fetching displacement
                let (val, seg, offset) = self.read_rmw16(modbits, rm);
                let result = self.shift_rotate_16(val, op, 1);
                self.write_rmw16(modbits, rm, result, seg, offset);
                self.cycles += if modbits == 0b11 { 2 } else { 15 };
                if modbits == 0b11 {
                    2
                } else {
                    15
                }
            }

            // Group 2 opcodes (0xD2) - Shift/rotate r/m8 by CL
            0xD2 => {
                let modrm = self.fetch_u8();
                let (modbits, op, rm) = Self::decode_modrm(modrm);
                let count = (self.cx & 0xFF) as u8;
                // Use RMW helpers to avoid double-fetching displacement
                let (val, seg, offset) = self.read_rmw8(modbits, rm);
                let result = self.shift_rotate_8(val, op, count);
                self.write_rmw8(modbits, rm, result, seg, offset);
                self.cycles += if modbits == 0b11 {
                    8 + (4 * count as u64)
                } else {
                    20 + (4 * count as u64)
                };
                if modbits == 0b11 {
                    8 + (4 * count as u32)
                } else {
                    20 + (4 * count as u32)
                }
            }

            // Group 2 opcodes (0xD3) - Shift/rotate r/m16 by CL
            0xD3 => {
                let modrm = self.fetch_u8();
                let (modbits, op, rm) = Self::decode_modrm(modrm);
                let count = (self.cx & 0xFF) as u8;
                // Use RMW helpers to avoid double-fetching displacement
                let (val, seg, offset) = self.read_rmw16(modbits, rm);
                let result = self.shift_rotate_16(val, op, count);
                self.write_rmw16(modbits, rm, result, seg, offset);
                self.cycles += if modbits == 0b11 {
                    8 + (4 * count as u64)
                } else {
                    20 + (4 * count as u64)
                };
                if modbits == 0b11 {
                    8 + (4 * count as u32)
                } else {
                    20 + (4 * count as u32)
                }
            }

            // AAM - ASCII Adjust After Multiply (0xD4)
            0xD4 => {
                let base = self.fetch_u8();
                let al = (self.ax & 0xFF) as u8;
                if base == 0 {
                    // Division by zero - trigger INT 0 as exception
                    self.trigger_interrupt(0, true);
                    self.cycles += 51; // Same as INT instruction
                    51
                } else {
                    let ah = al / base;
                    let al_new = al % base;
                    self.ax = (((ah as u16) << 8) | (al_new as u16)) as u32;
                    self.update_flags_8(al_new);
                    self.cycles += 83;
                    83
                }
            }

            // AAD - ASCII Adjust Before Division (0xD5)
            0xD5 => {
                let base = self.fetch_u8();
                let ah = ((self.ax >> 8) & 0xFF) as u8;
                let al = (self.ax & 0xFF) as u8;
                let result = al.wrapping_add(ah.wrapping_mul(base));
                self.ax = (self.ax & 0xFFFF_FF00) | (result as u32);
                self.ax &= 0x00FF; // Clear AH
                self.update_flags_8(result);
                self.cycles += 60;
                60
            }

            // SALC/SETALC (0xD6) - Undocumented opcode
            0xD6 => {
                // Set AL on Carry
                let al = if self.get_flag(FLAG_CF) { 0xFF } else { 0x00 };
                self.ax = (self.ax & 0xFFFF_FF00) | (al as u32);
                self.cycles += 3;
                3
            }

            // XLAT/XLATB (0xD7)
            0xD7 => {
                let al = (self.ax & 0xFF) as u8;
                let offset = self.bx.wrapping_add((al as u16) as u32);
                let seg = self.get_segment_with_override(self.ds);
                let val = self.read(seg, offset as u16);
                self.ax = (self.ax & 0xFFFF_FF00) | (val as u32);
                self.cycles += 11;
                11
            }

            // ESC opcodes (0xD8-0xDF) - FPU instructions
            // For basic emulation, treat as NOPs
            0xD8..=0xDF => {
                let modrm = self.fetch_u8();
                let (modbits, _, _) = Self::decode_modrm(modrm);
                // Just consume the ModR/M byte and any displacement
                self.cycles += if modbits == 0b11 { 2 } else { 8 };
                if modbits == 0b11 {
                    2
                } else {
                    8
                }
            }

            // LOOP variants (0xE0-0xE3)
            // LOOPNE/LOOPNZ (0xE0)
            0xE0 => {
                let offset = self.fetch_u8() as i8;
                self.cx = self.cx.wrapping_sub(1);
                if self.cx != 0 && !self.get_flag(FLAG_ZF) {
                    self.ip = (self.ip.wrapping_add((offset as i16) as u32)) & 0xFFFF;
                    self.cycles += 19;
                    19
                } else {
                    self.cycles += 5;
                    5
                }
            }

            // LOOPE/LOOPZ (0xE1)
            0xE1 => {
                let offset = self.fetch_u8() as i8;
                self.cx = self.cx.wrapping_sub(1);
                if self.cx != 0 && self.get_flag(FLAG_ZF) {
                    self.ip = (self.ip.wrapping_add((offset as i16) as u32)) & 0xFFFF;
                    self.cycles += 18;
                    18
                } else {
                    self.cycles += 6;
                    6
                }
            }

            // LOOP (0xE2)
            0xE2 => {
                let offset = self.fetch_u8() as i8;
                self.cx = self.cx.wrapping_sub(1);
                if self.cx != 0 {
                    self.ip = (self.ip.wrapping_add((offset as i16) as u32)) & 0xFFFF;
                    self.cycles += 17;
                    17
                } else {
                    self.cycles += 5;
                    5
                }
            }

            // JCXZ - Jump if CX is Zero (0xE3)
            0xE3 => {
                let offset = self.fetch_u8() as i8;
                if self.cx == 0 {
                    self.ip = (self.ip.wrapping_add((offset as i16) as u32)) & 0xFFFF;
                    self.cycles += 18;
                    18
                } else {
                    self.cycles += 6;
                    6
                }
            }

            // IN AL, imm8 (0xE4)
            0xE4 => {
                let _port = self.fetch_u8();
                // For basic emulation, return 0xFF
                self.ax = (self.ax & 0xFF00) | 0xFF;
                self.cycles += 10;
                10
            }

            // IN AX, imm8 (0xE5)
            0xE5 => {
                let _port = self.fetch_u8();
                // For basic emulation, return 0xFFFF
                self.ax = 0xFFFF;
                self.cycles += 10;
                10
            }

            // OUT imm8, AL (0xE6)
            0xE6 => {
                let _port = self.fetch_u8();
                let _al = (self.ax & 0xFF) as u8;
                // For basic emulation, do nothing
                self.cycles += 10;
                10
            }

            // OUT imm8, AX (0xE7)
            0xE7 => {
                let _port = self.fetch_u8();
                // For basic emulation, do nothing
                self.cycles += 10;
                10
            }

            // JMP near relative (0xE9)
            0xE9 => {
                let offset = self.fetch_u16() as i16;
                self.ip = (self.ip.wrapping_add((offset as i16) as u32)) & 0xFFFF;
                self.cycles += 15;
                15
            }

            // JMP far absolute (0xEA)
            0xEA => {
                let offset = self.fetch_u16();
                let segment = self.fetch_u16();
                self.ip = offset as u32;
                self.cs = segment;
                self.cycles += 15;
                15
            }

            // IN AL, DX (0xEC)
            0xEC => {
                // For basic emulation, return 0xFF
                self.ax = (self.ax & 0xFF00) | 0xFF;
                self.cycles += 8;
                8
            }

            // IN AX, DX (0xED)
            0xED => {
                // For basic emulation, return 0xFFFF
                self.ax = 0xFFFF;
                self.cycles += 8;
                8
            }

            // OUT DX, AL (0xEE)
            0xEE => {
                let _al = (self.ax & 0xFF) as u8;
                // For basic emulation, do nothing
                self.cycles += 8;
                8
            }

            // OUT DX, AX (0xEF)
            0xEF => {
                // For basic emulation, do nothing
                self.cycles += 8;
                8
            }

            // LOCK prefix (0xF0)
            0xF0 => {
                // LOCK prefix - for basic emulation, just execute next instruction
                let _next_opcode = self.fetch_u8();
                self.ip = self.ip.wrapping_sub(1);
                self.step()
            }

            // Undefined/INT1 (0xF1)
            0xF1 => {
                // Treat as NOP
                self.cycles += 2;
                2
            }

            // CMC - Complement Carry Flag (0xF5)
            0xF5 => {
                self.set_flag(FLAG_CF, !self.get_flag(FLAG_CF));
                self.cycles += 2;
                2
            }

            // Group 4 opcodes (0xFE) - INC/DEC r/m8
            0xFE => {
                let modrm = self.fetch_u8();
                let (modbits, op, rm) = Self::decode_modrm(modrm);

                match op {
                    0 => {
                        // INC r/m8 - use RMW helpers to avoid double-fetching displacement
                        let (val, seg, offset) = self.read_rmw8(modbits, rm);
                        let result = val.wrapping_add(1u8);
                        let overflow = val == 0x7F;
                        // AF calculation for INC: check if carry from bit 3 to bit 4
                        let af = (val & 0x0F) == 0x0F;
                        self.write_rmw8(modbits, rm, result, seg, offset);
                        self.update_flags_8(result);
                        self.set_flag(FLAG_OF, overflow);
                        self.set_flag(FLAG_AF, af);
                    }
                    1 => {
                        // DEC r/m8 - use RMW helpers to avoid double-fetching displacement
                        let (val, seg, offset) = self.read_rmw8(modbits, rm);
                        let result = val.wrapping_sub(1u8);
                        let overflow = val == 0x80;
                        // AF calculation for DEC: check if borrow from bit 4 to bit 3
                        let af = (val & 0x0F) == 0x00;
                        self.write_rmw8(modbits, rm, result, seg, offset);
                        self.update_flags_8(result);
                        self.set_flag(FLAG_OF, overflow);
                        self.set_flag(FLAG_AF, af);
                    }
                    _ => {
                        eprintln!(
                            "Undefined 0xFE operation with op={} at CS:IP={:04X}:{:04X}",
                            op,
                            self.cs,
                            self.ip.wrapping_sub(2u32)
                        );
                        // For undefined operations, we'll just NOP and continue
                        // This prevents the system from crashing completely
                    }
                }
                self.cycles += if modbits == 0b11 { 3 } else { 15 };
                if modbits == 0b11 {
                    3
                } else {
                    15
                }
            }

            // Group 5 opcodes (0xFF) - INC/DEC/CALL/JMP r/m16
            0xFF => {
                let modrm = self.fetch_u8();
                let (modbits, op, rm) = Self::decode_modrm(modrm);

                match op {
                    0 => {
                        // INC r/m16 - use RMW helpers to avoid double-fetching displacement
                        let (val, seg, offset) = self.read_rmw16(modbits, rm);
                        let result = val.wrapping_add(1);
                        let overflow = val == 0x7FFF;
                        // AF calculation for INC: check if carry from bit 3 to bit 4 in low byte
                        let af = (val & 0x0F) == 0x0F;
                        self.write_rmw16(modbits, rm, result, seg, offset);
                        self.update_flags_16(result as u16);
                        self.set_flag(FLAG_OF, overflow);
                        self.set_flag(FLAG_AF, af);
                        self.cycles += if modbits == 0b11 { 3 } else { 15 };
                        if modbits == 0b11 {
                            3
                        } else {
                            15
                        }
                    }
                    1 => {
                        // DEC r/m16 - use RMW helpers to avoid double-fetching displacement
                        let (val, seg, offset) = self.read_rmw16(modbits, rm);
                        let result = val.wrapping_sub(1);
                        let overflow = val == 0x8000;
                        // AF calculation for DEC: check if borrow from bit 4 to bit 3 in low byte
                        let af = (val & 0x0F) == 0x00;
                        self.write_rmw16(modbits, rm, result, seg, offset);
                        self.update_flags_16(result as u16);
                        self.set_flag(FLAG_OF, overflow);
                        self.set_flag(FLAG_AF, af);
                        self.cycles += if modbits == 0b11 { 3 } else { 15 };
                        if modbits == 0b11 {
                            3
                        } else {
                            15
                        }
                    }
                    2 => {
                        // CALL r/m16 (near)
                        let target = self.read_rm16(modbits, rm);
                        self.push(self.ip as u16);
                        self.ip = target as u32;
                        self.cycles += if modbits == 0b11 { 16 } else { 21 };
                        if modbits == 0b11 {
                            16
                        } else {
                            21
                        }
                    }
                    3 => {
                        // CALL m16:16 (far)
                        let (seg, offset_ea, _) = self.calc_effective_address(modbits, rm);
                        let offset = self.read_u16(seg, offset_ea);
                        let segment = self.read_u16(seg, offset_ea.wrapping_add(2));
                        self.push(self.cs);
                        self.push(self.ip as u16);
                        self.ip = offset as u32;
                        self.cs = segment;
                        self.cycles += 37;
                        37
                    }
                    4 => {
                        // JMP r/m16 (near)
                        let target = self.read_rm16(modbits, rm);
                        self.ip = target as u32;
                        self.cycles += if modbits == 0b11 { 11 } else { 18 };
                        if modbits == 0b11 {
                            11
                        } else {
                            18
                        }
                    }
                    5 => {
                        // JMP m16:16 (far)
                        let (seg, offset_ea, _) = self.calc_effective_address(modbits, rm);
                        let offset = self.read_u16(seg, offset_ea);
                        let segment = self.read_u16(seg, offset_ea.wrapping_add(2));
                        self.ip = offset as u32;
                        self.cs = segment;
                        self.cycles += 24;
                        24
                    }
                    6 => {
                        // PUSH r/m16
                        let val = self.read_rm16(modbits, rm);
                        self.push(val);
                        self.cycles += if modbits == 0b11 { 11 } else { 16 };
                        if modbits == 0b11 {
                            11
                        } else {
                            16
                        }
                    }
                    _ => {
                        // Undefined operation - Group 5 only supports ops 0-6
                        // Consume any displacement bytes to prevent desync
                        if modbits != 0b11 {
                            let _ = self.calc_effective_address(modbits, rm);
                        }
                        // Note: This is likely invalid code; treat as NOP to continue execution
                        eprintln!(
                            "Undefined 0xFF operation with op={} at CS:IP={:04X}:{:04X} - treating as NOP",
                            op,
                            self.cs,
                            self.ip.wrapping_sub(2u32)
                        );
                        self.cycles += 1;
                        1
                    }
                }
            }

            // INC reg16 (40-47)
            // Note: INC does not affect the Carry Flag (CF), only OF/SF/ZF/AF/PF
            0x40..=0x47 => {
                let reg = opcode & 0x07;
                let val = self.get_reg16(reg);
                let result = val.wrapping_add(1);
                let overflow = val == 0x7FFF;
                // AF calculation for INC: check if carry from bit 3 to bit 4 in low byte
                let af = (val & 0x0F) == 0x0F;

                self.set_reg16(reg, result);
                self.update_flags_16(result);
                self.set_flag(FLAG_OF, overflow);
                self.set_flag(FLAG_AF, af);
                self.cycles += 2;
                2
            }

            // DEC reg16 (48-4F)
            // Note: DEC does not affect the Carry Flag (CF), only OF/SF/ZF/AF/PF
            0x48..=0x4F => {
                let reg = opcode & 0x07;
                let val = self.get_reg16(reg);
                let result = val.wrapping_sub(1);
                let overflow = val == 0x8000;
                // AF calculation for DEC: check if borrow from bit 4 to bit 3 in low byte
                let af = (val & 0x0F) == 0x00;

                self.set_reg16(reg, result);
                self.update_flags_16(result);
                self.set_flag(FLAG_OF, overflow);
                self.set_flag(FLAG_AF, af);
                self.cycles += 2;
                2
            }

            // PUSH reg16 (50-57)
            0x50..=0x57 => {
                let reg = opcode & 0x07;
                let val = self.get_reg16(reg);
                self.push(val);
                self.cycles += 11;
                11
            }

            // POP reg16 (58-5F)
            0x58..=0x5F => {
                let reg = opcode & 0x07;
                let val = self.pop();
                self.set_reg16(reg, val);
                self.cycles += 8;
                8
            }

            // PUSHA - Push All General Registers (0x60) - 80186+
            0x60 => {
                if !self.model.supports_80186_instructions() {
                    // Invalid opcode on 8086/8088
                    self.cycles += 10;
                    return 10;
                }
                let temp_sp = self.sp;
                self.push(self.ax as u16);
                self.push(self.cx as u16);
                self.push(self.dx as u16);
                self.push(self.bx as u16);
                self.push(temp_sp as u16); // Push original SP value
                self.push(self.bp as u16);
                self.push(self.si as u16);
                self.push(self.di as u16);
                self.cycles += 36;
                36
            }

            // POPA - Pop All General Registers (0x61) - 80186+
            0x61 => {
                if !self.model.supports_80186_instructions() {
                    // Invalid opcode on 8086/8088
                    self.cycles += 10;
                    return 10;
                }
                self.di = (self.di & 0xFFFF_0000) | (self.pop() as u32);
                self.si = (self.si & 0xFFFF_0000) | (self.pop() as u32);
                self.bp = (self.bp & 0xFFFF_0000) | (self.pop() as u32);
                let _temp_sp = self.pop(); // Discard SP value
                self.bx = (self.bx & 0xFFFF_0000) | (self.pop() as u32);
                self.dx = (self.dx & 0xFFFF_0000) | (self.pop() as u32);
                self.cx = (self.cx & 0xFFFF_0000) | (self.pop() as u32);
                self.ax = (self.ax & 0xFFFF_0000) | (self.pop() as u32);
                self.cycles += 51;
                51
            }

            // BOUND - Check Array Index Against Bounds (0x62) - 80186+
            0x62 => {
                if !self.model.supports_80186_instructions() {
                    // Invalid opcode on 8086/8088
                    self.cycles += 10;
                    return 10;
                }
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);
                let index = self.get_reg16(reg) as i16;
                let (_seg, ea, _bytes) = self.calc_effective_address(modbits, rm);
                let lower_bound = self.read_u16(self.ds, ea) as i16;
                let upper_bound = self.read_u16(self.ds, ea.wrapping_add(2)) as i16;

                // If index is out of bounds, generate INT 5 as exception
                if index < lower_bound || index > upper_bound {
                    self.trigger_interrupt(5, true);
                    self.cycles += 33; // Approximate
                    33
                } else {
                    self.cycles += 10; // No interrupt case
                    10
                }
            }

            // ARPL r/m16, r16 (0x63) - 80286+ protected mode instruction
            // Adjust RPL field of selector: if RPL of r/m16 < RPL of r16, set r/m16's RPL to r16's RPL and set ZF
            // For real mode emulation, we stub this as a NOP that clears ZF
            0x63 => {
                let modrm = self.fetch_u8();
                let (modbits, _reg, _rm) = Self::decode_modrm(modrm);
                // In real mode, ARPL is effectively a NOP (or may behave as MOVSXD on x86-64)
                // We'll just clear ZF to indicate no adjustment was needed
                self.set_flag(FLAG_ZF, false);
                self.cycles += if modbits == 0b11 { 7 } else { 17 };
                if modbits == 0b11 {
                    7
                } else {
                    17
                }
            }

            // FS segment override prefix (0x64) - 80386+
            0x64 => {
                if !self.model.supports_80386_instructions() {
                    // Invalid opcode on 8086/8088/80186/80286
                    self.cycles += 10;
                    return 10;
                }
                // FS segment override prefix
                self.segment_override = Some(SegmentOverride::FS);
                self.step() // Execute next instruction with FS override
            }

            // GS segment override prefix (0x65) - 80386+
            0x65 => {
                if !self.model.supports_80386_instructions() {
                    // Invalid opcode on 8086/8088/80186/80286
                    self.cycles += 10;
                    return 10;
                }
                // GS segment override prefix
                self.segment_override = Some(SegmentOverride::GS);
                self.step() // Execute next instruction with GS override
            }

            // Operand-size override prefix (0x66) - 80386+
            0x66 => {
                if !self.model.supports_80386_instructions() {
                    // Invalid opcode on 8086/8088/80186/80286
                    self.cycles += 10;
                    return 10;
                }
                // Operand-size override prefix
                // On 80386+, this toggles between 16-bit and 32-bit operand size
                // For now, we set a flag and handle it in individual instructions
                self.operand_size_override = true;
                self.step() // Execute next instruction with operand size override
            }

            // Address-size override prefix (0x67) - 80386+
            0x67 => {
                if !self.model.supports_80386_instructions() {
                    // Invalid opcode on 8086/8088/80186/80286
                    self.cycles += 10;
                    return 10;
                }
                // Address-size override prefix
                // On 80386+, this toggles between 16-bit and 32-bit addressing
                // For now, we set a flag but don't fully implement 32-bit addressing modes.
                // IMPLEMENTATION NOTE: Full 32-bit addressing requires:
                //   - 32-bit registers (EAX, EBX, ECX, EDX, ESI, EDI, EBP, ESP)
                //   - SIB byte decoding for [EAX+EBX*4+disp32] style addressing
                //   - ModR/M extensions for 32-bit modes
                // Most 16-bit DOS/Windows software doesn't use this prefix in practice.
                // FUTURE ENHANCEMENT: Implement full 32-bit addressing for complete 386+ compatibility.
                //                     See CPU_REVIEW_RESULTS.md - marked as "Long Term" enhancement.
                self.address_size_override = true;
                self.step() // Execute next instruction with address size override
            }

            // PUSH immediate word (0x68) - 80186+
            0x68 => {
                if !self.model.supports_80186_instructions() {
                    // Invalid opcode on 8086/8088
                    self.cycles += 10;
                    return 10;
                }
                let val = self.fetch_u16();
                self.push(val);
                self.cycles += 3;
                3
            }

            // IMUL r16, r/m16, imm16 (0x69) - 80186+
            0x69 => {
                if !self.model.supports_80186_instructions() {
                    // Invalid opcode on 8086/8088
                    self.cycles += 10;
                    return 10;
                }
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);
                let rm_val = self.read_rm16(modbits, rm) as i16;
                let imm = self.fetch_u16() as i16;
                let result = rm_val.wrapping_mul(imm);

                self.set_reg16(reg, result as u16);

                // Set CF and OF if result doesn't fit in signed 16-bit
                // Check if the multiplication would overflow by comparing to extended multiply
                let extended_result = (rm_val as i32) * (imm as i32);
                let overflow = extended_result != (result as i32);
                self.set_flag(FLAG_CF, overflow);
                self.set_flag(FLAG_OF, overflow);

                self.cycles += if modbits == 0b11 { 21 } else { 24 };
                if modbits == 0b11 {
                    21
                } else {
                    24
                }
            }

            // PUSH immediate byte (0x6A) - 80186+
            0x6A => {
                if !self.model.supports_80186_instructions() {
                    // Invalid opcode on 8086/8088
                    self.cycles += 10;
                    return 10;
                }
                let val = self.fetch_u8() as i8 as i16 as u16; // Sign extend
                self.push(val);
                self.cycles += 3;
                3
            }

            // IMUL r16, r/m16, imm8 (0x6B) - 80186+
            0x6B => {
                if !self.model.supports_80186_instructions() {
                    // Invalid opcode on 8086/8088
                    self.cycles += 10;
                    return 10;
                }
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);
                let rm_val = self.read_rm16(modbits, rm) as i16;
                let imm = self.fetch_u8() as i8 as i16; // Sign extend
                let result = rm_val.wrapping_mul(imm);

                self.set_reg16(reg, result as u16);

                // Set CF and OF if result doesn't fit in signed 16-bit
                // Check if the multiplication would overflow by comparing to extended multiply
                let extended_result = (rm_val as i32) * (imm as i32);
                let overflow = extended_result != (result as i32);
                self.set_flag(FLAG_CF, overflow);
                self.set_flag(FLAG_OF, overflow);

                self.cycles += if modbits == 0b11 { 21 } else { 24 };
                if modbits == 0b11 {
                    21
                } else {
                    24
                }
            }

            // INSB - Input String Byte (0x6C) - 80186+
            0x6C => {
                if !self.model.supports_80186_instructions() {
                    // Invalid opcode on 8086/8088
                    self.cycles += 10;
                    return 10;
                }
                // Read from I/O port DX, write to ES:DI
                let port = (self.dx & 0xFFFF) as u16;
                let val = self.io_read(port);
                self.write(self.es, self.di as u16, val);

                // Update DI based on DF flag
                if self.get_flag(FLAG_DF) {
                    self.di = self.di.wrapping_sub(1);
                } else {
                    self.di = self.di.wrapping_add(1);
                }
                self.cycles += 14;
                14
            }

            // INSW - Input String Word (0x6D) - 80186+
            0x6D => {
                if !self.model.supports_80186_instructions() {
                    // Invalid opcode on 8086/8088
                    self.cycles += 10;
                    return 10;
                }
                // Read from I/O port DX, write to ES:DI
                let port = (self.dx & 0xFFFF) as u16;
                let val = self.io_read_word(port);
                self.write_u16(self.es, self.di as u16, val);

                // Update DI based on DF flag
                if self.get_flag(FLAG_DF) {
                    self.di = self.di.wrapping_sub(2u32);
                } else {
                    self.di = self.di.wrapping_add(2u32);
                }
                self.cycles += 14;
                14
            }

            // OUTSB - Output String Byte (0x6E) - 80186+
            0x6E => {
                if !self.model.supports_80186_instructions() {
                    // Invalid opcode on 8086/8088
                    self.cycles += 10;
                    return 10;
                }
                // Read from DS:SI, write to I/O port DX
                let val = self.read(self.ds, self.si as u16);
                let port = (self.dx & 0xFFFF) as u16;
                self.io_write(port, val);

                // Update SI based on DF flag
                if self.get_flag(FLAG_DF) {
                    self.si = self.si.wrapping_sub(1);
                } else {
                    self.si = self.si.wrapping_add(1);
                }
                self.cycles += 14;
                14
            }

            // OUTSW - Output String Word (0x6F) - 80186+
            0x6F => {
                if !self.model.supports_80186_instructions() {
                    // Invalid opcode on 8086/8088
                    self.cycles += 10;
                    return 10;
                }
                // Read from DS:SI, write to I/O port DX
                let val = self.read_u16(self.ds, self.si as u16);
                let port = (self.dx & 0xFFFF) as u16;
                self.io_write_word(port, val);

                // Update SI based on DF flag
                if self.get_flag(FLAG_DF) {
                    self.si = self.si.wrapping_sub(2u32);
                } else {
                    self.si = self.si.wrapping_add(2u32);
                }
                self.cycles += 14;
                14
            }

            // JMP short (EB)
            0xEB => {
                let offset = self.fetch_u8() as i8;
                // Add signed offset to IP (wrapping_add_signed would be clearer but requires i16 cast)
                self.ip = (self.ip.wrapping_add((offset as i16) as u32)) & 0xFFFF;
                self.cycles += 15;
                15
            }

            // JZ/JE (74) - Jump if Zero
            0x74 => {
                let offset = self.fetch_u8() as i8;
                if self.get_flag(FLAG_ZF) {
                    self.ip = (self.ip.wrapping_add((offset as i16) as u32)) & 0xFFFF;
                    self.cycles += 16;
                    16
                } else {
                    self.cycles += 4;
                    4
                }
            }

            // MOVSB - Move String Byte (0xA4)
            0xA4 => {
                // Move byte from DS:SI to ES:DI (with segment override support)
                let src_seg = self.get_segment_with_override(self.ds);
                let val = self.read(src_seg, self.si as u16);
                self.write(self.es, self.di as u16, val);

                // Update SI and DI based on DF flag
                if self.get_flag(FLAG_DF) {
                    self.si = self.si.wrapping_sub(1);
                    self.di = self.di.wrapping_sub(1);
                } else {
                    self.si = self.si.wrapping_add(1);
                    self.di = self.di.wrapping_add(1);
                }
                self.cycles += 18;
                18
            }

            // MOVSW - Move String Word (0xA5)
            0xA5 => {
                // Move word from DS:SI to ES:DI (with segment override support)
                let src_seg = self.get_segment_with_override(self.ds);
                let val = self.read_u16(src_seg, self.si as u16);
                self.write_u16(self.es, self.di as u16, val);

                // Update SI and DI based on DF flag
                if self.get_flag(FLAG_DF) {
                    self.si = self.si.wrapping_sub(2u32);
                    self.di = self.di.wrapping_sub(2u32);
                } else {
                    self.si = self.si.wrapping_add(2u32);
                    self.di = self.di.wrapping_add(2u32);
                }
                self.cycles += 18;
                18
            }

            // CMPSB - Compare String Byte (0xA6)
            0xA6 => {
                // Compare byte at DS:SI with byte at ES:DI (with segment override support)
                let src_seg = self.get_segment_with_override(self.ds);
                let src = self.read(src_seg, self.si as u16);
                let dst = self.read(self.es, self.di as u16);
                let result = src.wrapping_sub(dst);
                let borrow = (src as u16) < (dst as u16);
                let overflow = ((src ^ dst) & (src ^ result) & 0x80) != 0;

                self.update_flags_8(result);
                self.set_flag(FLAG_CF, borrow);
                self.set_flag(FLAG_OF, overflow);

                // Update SI and DI
                if self.get_flag(FLAG_DF) {
                    self.si = self.si.wrapping_sub(1);
                    self.di = self.di.wrapping_sub(1);
                } else {
                    self.si = self.si.wrapping_add(1);
                    self.di = self.di.wrapping_add(1);
                }
                self.cycles += 22;
                22
            }

            // CMPSW - Compare String Word (0xA7)
            0xA7 => {
                // Compare word at DS:SI with word at ES:DI (with segment override support)
                let src_seg = self.get_segment_with_override(self.ds);
                let src = self.read_u16(src_seg, self.si as u16);
                let dst = self.read_u16(self.es, self.di as u16);
                let result = src.wrapping_sub(dst);
                let borrow = (src as u32) < (dst as u32);
                let overflow = ((src ^ dst) & (src ^ result) & 0x8000) != 0;

                self.update_flags_16(result as u16);
                self.set_flag(FLAG_CF, borrow);
                self.set_flag(FLAG_OF, overflow);

                // Update SI and DI
                if self.get_flag(FLAG_DF) {
                    self.si = self.si.wrapping_sub(2u32);
                    self.di = self.di.wrapping_sub(2u32);
                } else {
                    self.si = self.si.wrapping_add(2u32);
                    self.di = self.di.wrapping_add(2u32);
                }
                self.cycles += 22;
                22
            }

            // STOSB - Store String Byte (0xAA)
            0xAA => {
                // Store AL to ES:DI
                let al = (self.ax & 0xFF) as u8;
                self.write(self.es, self.di as u16, al);

                // Update DI
                if self.get_flag(FLAG_DF) {
                    self.di = self.di.wrapping_sub(1);
                } else {
                    self.di = self.di.wrapping_add(1);
                }
                self.cycles += 11;
                11
            }

            // STOSW - Store String Word (0xAB)
            0xAB => {
                // Store AX to ES:DI
                self.write_u16(self.es, self.di as u16, self.ax as u16);

                // Update DI
                if self.get_flag(FLAG_DF) {
                    self.di = self.di.wrapping_sub(2u32);
                } else {
                    self.di = self.di.wrapping_add(2u32);
                }
                self.cycles += 11;
                11
            }

            // LODSB - Load String Byte (0xAC)
            0xAC => {
                // Load byte from DS:SI into AL (with segment override support)
                let src_seg = self.get_segment_with_override(self.ds);
                let val = self.read(src_seg, self.si as u16);
                self.ax = (self.ax & 0xFFFF_FF00) | (val as u32);

                // Update SI
                if self.get_flag(FLAG_DF) {
                    self.si = self.si.wrapping_sub(1);
                } else {
                    self.si = self.si.wrapping_add(1);
                }
                self.cycles += 12;
                12
            }

            // LODSW - Load String Word (0xAD)
            0xAD => {
                // Load word from DS:SI into AX (with segment override support)
                let src_seg = self.get_segment_with_override(self.ds);
                self.ax = (self.ax & 0xFFFF_0000) | (self.read_u16(src_seg, self.si as u16) as u32);

                // Update SI
                if self.get_flag(FLAG_DF) {
                    self.si = self.si.wrapping_sub(2u32);
                } else {
                    self.si = self.si.wrapping_add(2u32);
                }
                self.cycles += 12;
                12
            }

            // SCASB - Scan String Byte (0xAE)
            0xAE => {
                // Compare AL with byte at ES:DI
                let al = (self.ax & 0xFF) as u8;
                let val = self.read(self.es, self.di as u16);
                let result = al.wrapping_sub(val);
                let borrow = (al as u16) < (val as u16);
                let overflow = ((al ^ val) & (al ^ result) & 0x80) != 0;

                self.update_flags_8(result);
                self.set_flag(FLAG_CF, borrow);
                self.set_flag(FLAG_OF, overflow);

                // Update DI
                if self.get_flag(FLAG_DF) {
                    self.di = self.di.wrapping_sub(1);
                } else {
                    self.di = self.di.wrapping_add(1);
                }
                self.cycles += 15;
                15
            }

            // SCASW - Scan String Word (0xAF)
            0xAF => {
                // Compare AX with word at ES:DI
                let val = self.read_u16(self.es, self.di as u16);
                let result = (self.ax as u16).wrapping_sub(val);
                let borrow = self.ax < (val as u32);
                let overflow =
                    (((self.ax as u16) ^ val) & ((self.ax as u16) ^ (result as u16)) & 0x8000) != 0;

                self.update_flags_16(result);
                self.set_flag(FLAG_CF, borrow);
                self.set_flag(FLAG_OF, overflow);

                // Update DI
                if self.get_flag(FLAG_DF) {
                    self.di = self.di.wrapping_sub(2u32);
                } else {
                    self.di = self.di.wrapping_add(2u32);
                }
                self.cycles += 15;
                15
            }

            // CALL near relative (0xE8)
            0xE8 => {
                let offset = self.fetch_u16() as i16;
                // Push return address (current IP after fetching the offset)
                self.push(self.ip as u16);
                // Jump to target (IP + offset)
                self.ip = (self.ip.wrapping_add((offset as i16) as u32)) & 0xFFFF;
                self.cycles += 19;
                19
            }

            // RET near (0xC3)
            0xC3 => {
                self.ip = self.pop() as u32;
                self.cycles += 8;
                8
            }

            // RET near with immediate (0xC2) - pops return address and adds imm16 to SP
            0xC2 => {
                let pop_bytes = self.fetch_u16();
                self.ip = self.pop() as u32;
                self.sp = self.sp.wrapping_add(pop_bytes as u32);
                self.cycles += 12;
                12
            }

            // CALL far absolute (0x9A)
            0x9A => {
                let new_ip = self.fetch_u16();
                let new_cs = self.fetch_u16();
                // Push current CS and IP
                self.push(self.cs);
                self.push(self.ip as u16);
                // Jump to far address
                self.cs = new_cs;
                self.ip = new_ip as u32;
                self.cycles += 28;
                28
            }

            // RET far (0xCB)
            0xCB => {
                self.ip = self.pop() as u32;
                self.cs = self.pop();
                self.cycles += 18;
                18
            }

            // RET far with immediate (0xCA) - pops return address and adds imm16 to SP
            0xCA => {
                let pop_bytes = self.fetch_u16();
                let ret_ip = self.pop();
                let ret_cs = self.pop();

                if LogConfig::global().should_log(LogCategory::CPU, LogLevel::Trace) {
                    eprintln!(
                        "[RETF] SP before={:04X}, pop_bytes={:04X}, ret_ip={:04X}, ret_cs={:04X}",
                        self.sp.wrapping_add(4u32),
                        pop_bytes,
                        ret_ip,
                        ret_cs
                    );
                }

                self.ip = ret_ip as u32;
                self.cs = ret_cs;
                self.sp = self.sp.wrapping_add(pop_bytes as u32);
                self.cycles += 17;
                17
            }

            // CLC - Clear Carry Flag
            0xF8 => {
                self.set_flag(FLAG_CF, false);
                self.cycles += 2;
                2
            }

            // STC - Set Carry Flag
            0xF9 => {
                self.set_flag(FLAG_CF, true);
                self.cycles += 2;
                2
            }

            // CLI - Clear Interrupt Flag
            0xFA => {
                self.set_flag(FLAG_IF, false);
                self.cycles += 2;
                2
            }

            // STI - Set Interrupt Flag
            0xFB => {
                self.set_flag(FLAG_IF, true);
                self.cycles += 2;
                2
            }

            // CLD - Clear Direction Flag
            0xFC => {
                self.set_flag(FLAG_DF, false);
                self.cycles += 2;
                2
            }

            // STD - Set Direction Flag
            0xFD => {
                self.set_flag(FLAG_DF, true);
                self.cycles += 2;
                2
            }

            // INT n - Software Interrupt
            0xCD => {
                let int_num = self.fetch_u8();
                self.trigger_interrupt(int_num, false);
                self.cycles += 51; // Approximate timing for INT instruction
                51
            }

            // IRET - Return from Interrupt
            0xCF => {
                // Pop IP, CS, FLAGS from stack (reverse order of INT)
                self.ip = self.pop() as u32;
                self.cs = self.pop();
                self.flags = self.pop() as u32;

                self.cycles += 32; // Approximate timing for IRET instruction
                32
            }

            #[allow(unreachable_patterns)]
            _ => {
                // Unknown/unimplemented opcode
                // Note: This pattern is technically unreachable as all 256 opcodes are covered,
                // but we keep it for safety and to handle potential future refactoring
                eprintln!(
                    "Unknown 8086 opcode: 0x{:02X} at CS:IP={:04X}:{:04X}",
                    opcode,
                    self.cs,
                    self.ip.wrapping_sub(1)
                );
                self.cycles += 1;
                1
            }
        };

        // Clear override flags after instruction execution
        // These flags are only valid for the immediately following instruction
        self.operand_size_override = false;
        self.address_size_override = false;

        // Increment TSC on Pentium+ processors
        // TSC increments by the number of cycles executed
        if self.model.supports_pentium_instructions() {
            self.tsc = self.tsc.wrapping_add(cycles_executed as u64);
        }

        cycles_executed
    }
}

/// Simple array-based memory for testing
pub struct ArrayMemory {
    data: Vec<u8>,
}

impl ArrayMemory {
    pub fn new() -> Self {
        Self {
            data: vec![0; 0x100000], // 1MB of memory
        }
    }

    /// Load a program at a specific physical address
    pub fn load_program(&mut self, addr: u32, program: &[u8]) {
        let start = addr as usize;
        let end = start + program.len();
        if end <= self.data.len() {
            self.data[start..end].copy_from_slice(program);
        }
    }

    /// Read a 16-bit word from memory (little-endian)
    pub fn read_u16(&self, addr: u32) -> u16 {
        let low = self.read(addr);
        let high = self.read(addr + 1);
        (high as u16) << 8 | low as u16
    }

    /// Write a 16-bit word to memory (little-endian)
    pub fn write_u16(&mut self, addr: u32, val: u16) {
        self.write(addr, (val & 0xFF) as u8);
        self.write(addr + 1, ((val >> 8) & 0xFF) as u8);
    }
}

impl Default for ArrayMemory {
    fn default() -> Self {
        Self::new()
    }
}

impl Memory8086 for ArrayMemory {
    fn read(&self, addr: u32) -> u8 {
        if (addr as usize) < self.data.len() {
            self.data[addr as usize]
        } else {
            0xFF
        }
    }

    fn write(&mut self, addr: u32, val: u8) {
        if (addr as usize) < self.data.len() {
            self.data[addr as usize] = val;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Include organized test modules
    mod tests_16bit;
    mod tests_32bit;
    mod tests_8bit;
    mod tests_addressing;
    mod tests_bcd;
    mod tests_blackbox;
    mod tests_flags;
    mod tests_jumps;
    mod tests_misc;
    mod tests_shifts;

    // Helper function for tests to calculate physical address
    fn physical_address(segment: u16, offset: u16) -> u32 {
        ((segment as u32) << 4) + (offset as u32)
    }
}
