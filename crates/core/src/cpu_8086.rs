//! Intel 8086 CPU core implementation
//!
//! This module provides a reusable, generic 8086 CPU implementation that can be used
//! by any system (IBM PC, PC XT, etc.) by implementing the `Memory8086` trait.
//!
//! Supports multiple CPU models: 8086, 80186, 80286, and their variants.

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
        )
    }

    /// Returns true if this CPU model supports 80286+ instructions
    pub fn supports_80286_instructions(&self) -> bool {
        matches!(self, CpuModel::Intel80286 | CpuModel::Intel80386)
    }

    /// Returns true if this CPU model supports 80386+ instructions
    pub fn supports_80386_instructions(&self) -> bool {
        matches!(self, CpuModel::Intel80386)
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
    // General purpose registers (16-bit)
    /// AX register (accumulator) - can be accessed as AH:AL
    pub ax: u16,
    /// BX register (base) - can be accessed as BH:BL
    pub bx: u16,
    /// CX register (count) - can be accessed as CH:CL
    pub cx: u16,
    /// DX register (data) - can be accessed as DH:DL
    pub dx: u16,

    // Index and pointer registers
    /// SI register (source index)
    pub si: u16,
    /// DI register (destination index)
    pub di: u16,
    /// BP register (base pointer)
    pub bp: u16,
    /// SP register (stack pointer)
    pub sp: u16,

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

    // Control registers
    /// IP register (instruction pointer)
    pub ip: u16,
    /// FLAGS register (status flags)
    pub flags: u16,

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

    /// CPU model (8086, 80186, 80286, etc.)
    model: CpuModel,

    /// Protected mode state (80286+ only)
    /// This is only used when model is Intel80286 or later
    protected_mode: ProtectedModeState,
}

// Flag bit positions in FLAGS register
const FLAG_CF: u16 = 0x0001; // Carry Flag
const FLAG_PF: u16 = 0x0004; // Parity Flag
#[allow(dead_code)]
const FLAG_AF: u16 = 0x0010; // Auxiliary Carry Flag
const FLAG_ZF: u16 = 0x0040; // Zero Flag
const FLAG_SF: u16 = 0x0080; // Sign Flag
#[allow(dead_code)]
const FLAG_TF: u16 = 0x0100; // Trap Flag
const FLAG_IF: u16 = 0x0200; // Interrupt Enable Flag
const FLAG_DF: u16 = 0x0400; // Direction Flag
const FLAG_OF: u16 = 0x0800; // Overflow Flag

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
            model,
            protected_mode: ProtectedModeState::new(),
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
        let val = self.read(self.cs, self.ip);
        self.ip = self.ip.wrapping_add(1);
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

    /// Push a word onto the stack
    #[inline]
    fn push(&mut self, val: u16) {
        self.sp = self.sp.wrapping_sub(2);
        self.write_u16(self.ss, self.sp, val);
    }

    /// Pop a word from the stack
    #[inline]
    fn pop(&mut self) -> u16 {
        let val = self.read_u16(self.ss, self.sp);
        self.sp = self.sp.wrapping_add(2);
        val
    }

    /// Trigger a software interrupt (INT)
    /// This is used for both explicit INT instructions and CPU-generated exceptions (e.g., divide error)
    #[inline]
    fn trigger_interrupt(&mut self, int_num: u8) {
        // Push FLAGS, CS, IP onto stack (in that order)
        self.push(self.flags);
        self.push(self.cs);
        self.push(self.ip);

        // Clear IF and TF flags
        self.set_flag(FLAG_IF, false);
        self.set_flag(FLAG_TF, false);

        // Read interrupt vector from IVT (Interrupt Vector Table) at 0x0000:int_num*4
        // Each IVT entry is 4 bytes: offset (2 bytes) + segment (2 bytes)
        let ivt_offset = (int_num as u16) * 4;
        let new_ip = self.read_u16(0, ivt_offset);
        let new_cs = self.read_u16(0, ivt_offset + 2);

        // Debug: Log INT 0 (divide error) calls (disabled - too verbose)
        // if int_num == 0 {
        //     eprintln!(
        //         "INT 0 triggered: IVT[0] = {:04X}:{:04X}, returning to CS:IP={:04X}:{:04X}",
        //         new_cs, new_ip, self.cs, self.ip
        //     );
        // }

        // Jump to interrupt handler
        self.ip = new_ip;
        self.cs = new_cs;
    }

    /// Trigger a CPU fault (exception) with a specific faulting IP
    ///
    /// Unlike normal interrupts where the saved IP points to the next instruction,
    /// faults save the IP of the faulting instruction so it can be retried after
    /// the fault handler fixes the problem (or aborted if unfixable).
    ///
    /// # Arguments
    /// * `int_num` - Interrupt vector number (e.g., 0 for divide error)
    /// * `faulting_ip` - IP of the instruction that caused the fault
    fn trigger_fault(&mut self, int_num: u8, faulting_ip: u16) {
        // Push FLAGS, CS, faulting IP onto stack (in that order)
        self.push(self.flags);
        self.push(self.cs);
        self.push(faulting_ip);

        // Clear IF and TF flags
        self.set_flag(FLAG_IF, false);
        self.set_flag(FLAG_TF, false);

        // Read interrupt vector from IVT (Interrupt Vector Table) at 0x0000:int_num*4
        // Each IVT entry is 4 bytes: offset (2 bytes) + segment (2 bytes)
        let ivt_offset = (int_num as u16) * 4;
        let new_ip = self.read_u16(0, ivt_offset);
        let new_cs = self.read_u16(0, ivt_offset + 2);

        // Jump to interrupt handler
        self.ip = new_ip;
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
            0 => (self.ax >> 8) as u8, // AH
            1 => (self.cx >> 8) as u8, // CH
            2 => (self.dx >> 8) as u8, // DH
            3 => (self.bx >> 8) as u8, // BH
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
            0 => self.ax = (self.ax & 0x00FF) | ((val as u16) << 8), // AH
            1 => self.cx = (self.cx & 0x00FF) | ((val as u16) << 8), // CH
            2 => self.dx = (self.dx & 0x00FF) | ((val as u16) << 8), // DH
            3 => self.bx = (self.bx & 0x00FF) | ((val as u16) << 8), // BH
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
            0 => self.ax = (self.ax & 0xFF00) | (val as u16), // AL
            1 => self.cx = (self.cx & 0xFF00) | (val as u16), // CL
            2 => self.dx = (self.dx & 0xFF00) | (val as u16), // DL
            3 => self.bx = (self.bx & 0xFF00) | (val as u16), // BL
            _ => unreachable!(),
        }
    }

    /// Get 16-bit register
    #[inline]
    fn get_reg16(&self, reg: u8) -> u16 {
        debug_assert!(
            reg < 8,
            "Invalid 16-bit register index: {} (must be 0-7)",
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

    /// Set 16-bit register
    #[inline]
    fn set_reg16(&mut self, reg: u8, val: u16) {
        debug_assert!(
            reg < 8,
            "Invalid 16-bit register index: {} (must be 0-7)",
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
    fn set_flag(&mut self, flag: u16, value: bool) {
        if value {
            self.flags |= flag;
        } else {
            self.flags &= !flag;
        }
    }

    /// Get flag
    #[inline]
    fn get_flag(&self, flag: u16) -> bool {
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

    /// Perform 8-bit shift/rotate operation
    fn shift_rotate_8(&mut self, val: u8, op: u8, count: u8) -> u8 {
        if count == 0 {
            return val;
        }

        let count = count & 0x1F; // 8086: only lower 5 bits used
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

        let count = count & 0x1F; // 8086: only lower 5 bits used
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
                    0b000 => (self.ds, self.bx.wrapping_add(self.si), 0), // [BX+SI]
                    0b001 => (self.ds, self.bx.wrapping_add(self.di), 0), // [BX+DI]
                    0b010 => (self.ss, self.bp.wrapping_add(self.si), 0), // [BP+SI]
                    0b011 => (self.ss, self.bp.wrapping_add(self.di), 0), // [BP+DI]
                    0b100 => (self.ds, self.si, 0),                       // [SI]
                    0b101 => (self.ds, self.di, 0),                       // [DI]
                    0b110 => {
                        // Special case: direct address (16-bit displacement)
                        let disp = self.fetch_u16();
                        (self.ds, disp, 2)
                    }
                    0b111 => (self.ds, self.bx, 0), // [BX]
                    _ => unreachable!(),
                }
            }
            // mod = 01: Memory mode with 8-bit signed displacement
            0b01 => {
                let disp = self.fetch_u8() as i8 as i16 as u16;
                match rm {
                    0b000 => (self.ds, self.bx.wrapping_add(self.si).wrapping_add(disp), 1), // [BX+SI+disp8]
                    0b001 => (self.ds, self.bx.wrapping_add(self.di).wrapping_add(disp), 1), // [BX+DI+disp8]
                    0b010 => (self.ss, self.bp.wrapping_add(self.si).wrapping_add(disp), 1), // [BP+SI+disp8]
                    0b011 => (self.ss, self.bp.wrapping_add(self.di).wrapping_add(disp), 1), // [BP+DI+disp8]
                    0b100 => (self.ds, self.si.wrapping_add(disp), 1), // [SI+disp8]
                    0b101 => (self.ds, self.di.wrapping_add(disp), 1), // [DI+disp8]
                    0b110 => (self.ss, self.bp.wrapping_add(disp), 1), // [BP+disp8]
                    0b111 => (self.ds, self.bx.wrapping_add(disp), 1), // [BX+disp8]
                    _ => unreachable!(),
                }
            }
            // mod = 10: Memory mode with 16-bit signed displacement
            0b10 => {
                let disp = self.fetch_u16();
                match rm {
                    0b000 => (self.ds, self.bx.wrapping_add(self.si).wrapping_add(disp), 2), // [BX+SI+disp16]
                    0b001 => (self.ds, self.bx.wrapping_add(self.di).wrapping_add(disp), 2), // [BX+DI+disp16]
                    0b010 => (self.ss, self.bp.wrapping_add(self.si).wrapping_add(disp), 2), // [BP+SI+disp16]
                    0b011 => (self.ss, self.bp.wrapping_add(self.di).wrapping_add(disp), 2), // [BP+DI+disp16]
                    0b100 => (self.ds, self.si.wrapping_add(disp), 2), // [SI+disp16]
                    0b101 => (self.ds, self.di.wrapping_add(disp), 2), // [DI+disp16]
                    0b110 => (self.ss, self.bp.wrapping_add(disp), 2), // [BP+disp16]
                    0b111 => (self.ds, self.bx.wrapping_add(disp), 2), // [BX+disp16]
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

    /// Execute one instruction and return cycles used
    pub fn step(&mut self) -> u32 {
        if self.halted {
            return 1;
        }

        // Save IP at instruction start for fault handling
        // (faults need to save the IP of the faulting instruction, not the next one)
        let instruction_start_ip = self.ip;

        let opcode = self.fetch_u8();

        match opcode {
            // REP/REPE/REPZ prefix (0xF3)
            0xF3 => {
                let next_opcode = self.fetch_u8();
                let mut total_cycles: u32 = 9; // Base prefix overhead

                match next_opcode {
                    // MOVSB
                    0xA4 => {
                        while self.cx != 0 {
                            let val = self.read(self.ds, self.si);
                            self.write(self.es, self.di, val);
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
                        while self.cx != 0 {
                            let val = self.read_u16(self.ds, self.si);
                            self.write_u16(self.es, self.di, val);
                            if self.get_flag(FLAG_DF) {
                                self.si = self.si.wrapping_sub(2);
                                self.di = self.di.wrapping_sub(2);
                            } else {
                                self.si = self.si.wrapping_add(2);
                                self.di = self.di.wrapping_add(2);
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
                            self.write(self.es, self.di, al);
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
                            self.write_u16(self.es, self.di, self.ax);
                            if self.get_flag(FLAG_DF) {
                                self.di = self.di.wrapping_sub(2);
                            } else {
                                self.di = self.di.wrapping_add(2);
                            }
                            self.cx = self.cx.wrapping_sub(1);
                            total_cycles += 10;
                        }
                        self.cycles += total_cycles as u64;
                        total_cycles
                    }
                    // LODSB
                    0xAC => {
                        while self.cx != 0 {
                            let val = self.read(self.ds, self.si);
                            self.ax = (self.ax & 0xFF00) | (val as u16);
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
                        while self.cx != 0 {
                            self.ax = self.read_u16(self.ds, self.si);
                            if self.get_flag(FLAG_DF) {
                                self.si = self.si.wrapping_sub(2);
                            } else {
                                self.si = self.si.wrapping_add(2);
                            }
                            self.cx = self.cx.wrapping_sub(1);
                            total_cycles += 13;
                        }
                        self.cycles += total_cycles as u64;
                        total_cycles
                    }
                    // CMPSB
                    0xA6 => {
                        while self.cx != 0 {
                            let src = self.read(self.ds, self.si);
                            let dst = self.read(self.es, self.di);
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
                                break;
                            }
                        }
                        self.cycles += total_cycles as u64;
                        total_cycles
                    }
                    // CMPSW
                    0xA7 => {
                        while self.cx != 0 {
                            let src = self.read_u16(self.ds, self.si);
                            let dst = self.read_u16(self.es, self.di);
                            let result = src.wrapping_sub(dst);
                            let borrow = (src as u32) < (dst as u32);
                            let overflow = ((src ^ dst) & (src ^ result) & 0x8000) != 0;
                            self.update_flags_16(result);
                            self.set_flag(FLAG_CF, borrow);
                            self.set_flag(FLAG_OF, overflow);
                            if self.get_flag(FLAG_DF) {
                                self.si = self.si.wrapping_sub(2);
                                self.di = self.di.wrapping_sub(2);
                            } else {
                                self.si = self.si.wrapping_add(2);
                                self.di = self.di.wrapping_add(2);
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
                            let val = self.read(self.es, self.di);
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
                            let val = self.read_u16(self.es, self.di);
                            let result = self.ax.wrapping_sub(val);
                            let borrow = (self.ax as u32) < (val as u32);
                            let overflow = ((self.ax ^ val) & (self.ax ^ result) & 0x8000) != 0;
                            self.update_flags_16(result);
                            self.set_flag(FLAG_CF, borrow);
                            self.set_flag(FLAG_OF, overflow);
                            if self.get_flag(FLAG_DF) {
                                self.di = self.di.wrapping_sub(2);
                            } else {
                                self.di = self.di.wrapping_add(2);
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
                        while self.cx != 0 {
                            let src = self.read(self.ds, self.si);
                            let dst = self.read(self.es, self.di);
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
                        while self.cx != 0 {
                            let src = self.read_u16(self.ds, self.si);
                            let dst = self.read_u16(self.es, self.di);
                            let result = src.wrapping_sub(dst);
                            let borrow = (src as u32) < (dst as u32);
                            let overflow = ((src ^ dst) & (src ^ result) & 0x8000) != 0;
                            self.update_flags_16(result);
                            self.set_flag(FLAG_CF, borrow);
                            self.set_flag(FLAG_OF, overflow);
                            if self.get_flag(FLAG_DF) {
                                self.si = self.si.wrapping_sub(2);
                                self.di = self.di.wrapping_sub(2);
                            } else {
                                self.si = self.si.wrapping_add(2);
                                self.di = self.di.wrapping_add(2);
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
                            let val = self.read(self.es, self.di);
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
                            let val = self.read_u16(self.es, self.di);
                            let result = self.ax.wrapping_sub(val);
                            let borrow = (self.ax as u32) < (val as u32);
                            let overflow = ((self.ax ^ val) & (self.ax ^ result) & 0x8000) != 0;
                            self.update_flags_16(result);
                            self.set_flag(FLAG_CF, borrow);
                            self.set_flag(FLAG_OF, overflow);
                            if self.get_flag(FLAG_DF) {
                                self.di = self.di.wrapping_sub(2);
                            } else {
                                self.di = self.di.wrapping_add(2);
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
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);
                let val = self.get_reg16(reg);
                self.write_rm16(modbits, rm, val);
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

            // MOV r16, r/m16 (0x8B)
            0x8B => {
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);
                let val = self.read_rm16(modbits, rm);
                self.set_reg16(reg, val);
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
                        self.ip.wrapping_sub(2)
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

                self.write_rmw8(modbits, rm, result, seg, offset);
                self.update_flags_8(result);
                self.set_flag(FLAG_CF, carry);
                self.set_flag(FLAG_OF, overflow);
                self.cycles += if modbits == 0b11 { 3 } else { 16 };
                if modbits == 0b11 {
                    3
                } else {
                    16
                }
            }

            // ADD r/m16, r16 (0x01)
            0x01 => {
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);
                let reg_val = self.get_reg16(reg);

                // Use RMW helpers to avoid double-fetching EA
                let (rm_val, seg, offset) = self.read_rmw16(modbits, rm);
                let result = rm_val.wrapping_add(reg_val);
                let carry = (rm_val as u32 + reg_val as u32) > 0xFFFF;
                let overflow = ((rm_val ^ result) & (reg_val ^ result) & 0x8000) != 0;

                self.write_rmw16(modbits, rm, result, seg, offset);
                self.update_flags_16(result);
                self.set_flag(FLAG_CF, carry);
                self.set_flag(FLAG_OF, overflow);
                self.cycles += if modbits == 0b11 { 3 } else { 16 };
                if modbits == 0b11 {
                    3
                } else {
                    16
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

                if reg < 4 {
                    self.set_reg8_low(reg, result);
                } else {
                    self.set_reg8_high(reg - 4, result);
                }
                self.update_flags_8(result);
                self.set_flag(FLAG_CF, carry);
                self.set_flag(FLAG_OF, overflow);
                self.cycles += if modbits == 0b11 { 3 } else { 9 };
                if modbits == 0b11 {
                    3
                } else {
                    9
                }
            }

            // ADD r16, r/m16 (0x03)
            0x03 => {
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);
                let reg_val = self.get_reg16(reg);
                let rm_val = self.read_rm16(modbits, rm);
                let result = reg_val.wrapping_add(rm_val);
                let carry = (reg_val as u32 + rm_val as u32) > 0xFFFF;
                let overflow = ((reg_val ^ result) & (rm_val ^ result) & 0x8000) != 0;

                self.set_reg16(reg, result);
                self.update_flags_16(result);
                self.set_flag(FLAG_CF, carry);
                self.set_flag(FLAG_OF, overflow);
                self.cycles += if modbits == 0b11 { 3 } else { 9 };
                if modbits == 0b11 {
                    3
                } else {
                    9
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

                self.write_rmw8(modbits, rm, result, seg, offset);
                self.update_flags_8(result);
                self.set_flag(FLAG_CF, borrow);
                self.set_flag(FLAG_OF, overflow);
                self.cycles += if modbits == 0b11 { 3 } else { 16 };
                if modbits == 0b11 {
                    3
                } else {
                    16
                }
            }

            // SUB r/m16, r16 (0x29)
            0x29 => {
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);
                let reg_val = self.get_reg16(reg);

                // Use RMW helpers to avoid double-fetching EA
                let (rm_val, seg, offset) = self.read_rmw16(modbits, rm);
                let result = rm_val.wrapping_sub(reg_val);
                let borrow = (rm_val as u32) < (reg_val as u32);
                let overflow = ((rm_val ^ reg_val) & (rm_val ^ result) & 0x8000) != 0;

                self.write_rmw16(modbits, rm, result, seg, offset);
                self.update_flags_16(result);
                self.set_flag(FLAG_CF, borrow);
                self.set_flag(FLAG_OF, overflow);
                self.cycles += if modbits == 0b11 { 3 } else { 16 };
                if modbits == 0b11 {
                    3
                } else {
                    16
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

                if reg < 4 {
                    self.set_reg8_low(reg, result);
                } else {
                    self.set_reg8_high(reg - 4, result);
                }
                self.update_flags_8(result);
                self.set_flag(FLAG_CF, borrow);
                self.set_flag(FLAG_OF, overflow);
                self.cycles += if modbits == 0b11 { 3 } else { 9 };
                if modbits == 0b11 {
                    3
                } else {
                    9
                }
            }

            // SUB r16, r/m16 (0x2B)
            0x2B => {
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);
                let reg_val = self.get_reg16(reg);
                let rm_val = self.read_rm16(modbits, rm);
                let result = reg_val.wrapping_sub(rm_val);
                let borrow = (reg_val as u32) < (rm_val as u32);
                let overflow = ((reg_val ^ rm_val) & (reg_val ^ result) & 0x8000) != 0;

                self.set_reg16(reg, result);
                self.update_flags_16(result);
                self.set_flag(FLAG_CF, borrow);
                self.set_flag(FLAG_OF, overflow);
                self.cycles += if modbits == 0b11 { 3 } else { 9 };
                if modbits == 0b11 {
                    3
                } else {
                    9
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

                self.update_flags_8(result);
                self.set_flag(FLAG_CF, borrow);
                self.set_flag(FLAG_OF, overflow);
                self.cycles += if modbits == 0b11 { 3 } else { 9 };
                if modbits == 0b11 {
                    3
                } else {
                    9
                }
            }

            // CMP r/m16, r16 (0x39)
            0x39 => {
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);
                let reg_val = self.get_reg16(reg);
                let rm_val = self.read_rm16(modbits, rm);
                let result = rm_val.wrapping_sub(reg_val);
                let borrow = (rm_val as u32) < (reg_val as u32);
                let overflow = ((rm_val ^ reg_val) & (rm_val ^ result) & 0x8000) != 0;

                self.update_flags_16(result);
                self.set_flag(FLAG_CF, borrow);
                self.set_flag(FLAG_OF, overflow);
                self.cycles += if modbits == 0b11 { 3 } else { 9 };
                if modbits == 0b11 {
                    3
                } else {
                    9
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

                self.update_flags_8(result);
                self.set_flag(FLAG_CF, borrow);
                self.set_flag(FLAG_OF, overflow);
                self.cycles += if modbits == 0b11 { 3 } else { 9 };
                if modbits == 0b11 {
                    3
                } else {
                    9
                }
            }

            // CMP r16, r/m16 (0x3B)
            0x3B => {
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);
                let reg_val = self.get_reg16(reg);
                let rm_val = self.read_rm16(modbits, rm);
                let result = reg_val.wrapping_sub(rm_val);
                let borrow = (reg_val as u32) < (rm_val as u32);
                let overflow = ((reg_val ^ rm_val) & (reg_val ^ result) & 0x8000) != 0;

                self.update_flags_16(result);
                self.set_flag(FLAG_CF, borrow);
                self.set_flag(FLAG_OF, overflow);
                self.cycles += if modbits == 0b11 { 3 } else { 9 };
                if modbits == 0b11 {
                    3
                } else {
                    9
                }
            }

            // ADD AL, imm8
            0x04 => {
                let val = self.fetch_u8();
                let al = (self.ax & 0xFF) as u8;
                let result = al.wrapping_add(val);
                let carry = (al as u16 + val as u16) > 0xFF;
                let overflow = ((al ^ result) & (val ^ result) & 0x80) != 0;

                self.ax = (self.ax & 0xFF00) | (result as u16);
                self.update_flags_8(result);
                self.set_flag(FLAG_CF, carry);
                self.set_flag(FLAG_OF, overflow);
                self.cycles += 4;
                4
            }

            // ADD AX, imm16
            0x05 => {
                let val = self.fetch_u16();
                let result = self.ax.wrapping_add(val);
                let carry = (self.ax as u32 + val as u32) > 0xFFFF;
                let overflow = ((self.ax ^ result) & (val ^ result) & 0x8000) != 0;

                self.ax = result;
                self.update_flags_16(result);
                self.set_flag(FLAG_CF, carry);
                self.set_flag(FLAG_OF, overflow);
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
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);
                let reg_val = self.get_reg16(reg);
                let (rm_val, seg, offset) = self.read_rmw16(modbits, rm);
                let result = rm_val | reg_val;

                self.write_rmw16(modbits, rm, result, seg, offset);
                self.update_flags_16(result);
                self.set_flag(FLAG_CF, false);
                self.set_flag(FLAG_OF, false);
                self.cycles += if modbits == 0b11 { 3 } else { 16 };
                if modbits == 0b11 {
                    3
                } else {
                    16
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

            // OR r16, r/m16 (0x0B)
            0x0B => {
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);
                let reg_val = self.get_reg16(reg);
                let rm_val = self.read_rm16(modbits, rm);
                let result = reg_val | rm_val;

                self.set_reg16(reg, result);
                self.update_flags_16(result);
                self.set_flag(FLAG_CF, false);
                self.set_flag(FLAG_OF, false);
                self.cycles += if modbits == 0b11 { 3 } else { 9 };
                if modbits == 0b11 {
                    3
                } else {
                    9
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
                    al = al.wrapping_add(6);
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

                self.ax = (self.ax & 0xFF00) | (al as u16);
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
                    al = al.wrapping_sub(6);
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

                self.ax = (self.ax & 0xFF00) | (al as u16);
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
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);
                let reg_val = self.get_reg16(reg);
                let (rm_val, seg, offset) = self.read_rmw16(modbits, rm);
                let result = rm_val ^ reg_val;

                self.write_rmw16(modbits, rm, result, seg, offset);
                self.update_flags_16(result);
                self.set_flag(FLAG_CF, false);
                self.set_flag(FLAG_OF, false);
                self.cycles += if modbits == 0b11 { 3 } else { 16 };
                if modbits == 0b11 {
                    3
                } else {
                    16
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

            // XOR r16, r/m16 (0x33)
            0x33 => {
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);
                let reg_val = self.get_reg16(reg);
                let rm_val = self.read_rm16(modbits, rm);
                let result = reg_val ^ rm_val;

                self.set_reg16(reg, result);
                self.update_flags_16(result);
                self.set_flag(FLAG_CF, false);
                self.set_flag(FLAG_OF, false);
                self.cycles += if modbits == 0b11 { 3 } else { 9 };
                if modbits == 0b11 {
                    3
                } else {
                    9
                }
            }

            // SUB AL, imm8
            0x2C => {
                let val = self.fetch_u8();
                let al = (self.ax & 0xFF) as u8;
                let result = al.wrapping_sub(val);
                let borrow = (al as u16) < (val as u16);
                let overflow = ((al ^ val) & (al ^ result) & 0x80) != 0;

                self.ax = (self.ax & 0xFF00) | (result as u16);
                self.update_flags_8(result);
                self.set_flag(FLAG_CF, borrow);
                self.set_flag(FLAG_OF, overflow);
                self.cycles += 4;
                4
            }

            // SUB AX, imm16
            0x2D => {
                let val = self.fetch_u16();
                let result = self.ax.wrapping_sub(val);
                let borrow = (self.ax as u32) < (val as u32);
                let overflow = ((self.ax ^ val) & (self.ax ^ result) & 0x8000) != 0;

                self.ax = result;
                self.update_flags_16(result);
                self.set_flag(FLAG_CF, borrow);
                self.set_flag(FLAG_OF, overflow);
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

                self.update_flags_8(result);
                self.set_flag(FLAG_CF, borrow);
                self.set_flag(FLAG_OF, overflow);
                self.cycles += 4;
                4
            }

            // CMP AX, imm16
            0x3D => {
                let val = self.fetch_u16();
                let result = self.ax.wrapping_sub(val);
                let borrow = (self.ax as u32) < (val as u32);
                let overflow = ((self.ax ^ val) & (self.ax ^ result) & 0x8000) != 0;

                self.update_flags_16(result);
                self.set_flag(FLAG_CF, borrow);
                self.set_flag(FLAG_OF, overflow);
                self.cycles += 4;
                4
            }

            // AND AL, imm8
            0x24 => {
                let val = self.fetch_u8();
                let al = (self.ax & 0xFF) as u8;
                let result = al & val;

                self.ax = (self.ax & 0xFF00) | (result as u16);
                self.update_flags_8(result);
                self.set_flag(FLAG_CF, false);
                self.set_flag(FLAG_OF, false);
                self.cycles += 4;
                4
            }

            // AND AX, imm16
            0x25 => {
                let val = self.fetch_u16();
                let result = self.ax & val;

                self.ax = result;
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

                self.ax = (self.ax & 0xFF00) | (result as u16);
                self.update_flags_8(result);
                self.set_flag(FLAG_CF, false);
                self.set_flag(FLAG_OF, false);
                self.cycles += 4;
                4
            }

            // OR AX, imm16
            0x0D => {
                let val = self.fetch_u16();
                let result = self.ax | val;

                self.ax = result;
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
                        // MOVSX r32, r/m16 (80386 only - not fully supported yet)
                        let _modrm = self.fetch_u8();
                        self.cycles += 3;
                        3
                    }
                    // MOVZX - Move with Zero Extension (0x0F 0xB6, 0xB7) - 80386+
                    0xB6 => {
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
                        // MOVZX r32, r/m16 (80386 only - not fully supported yet)
                        let _modrm = self.fetch_u8();
                        self.cycles += 3;
                        3
                    }
                    // BSF - Bit Scan Forward (0x0F 0xBC) - 80386+
                    0xBC => {
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
                    // SHLD - Double Precision Shift Left (0x0F 0xA4, 0xA5) - 80386+
                    0xA4 => {
                        let modrm = self.fetch_u8();
                        let (_modbits, _reg, _rm) = Self::decode_modrm(modrm);
                        let _count = self.fetch_u8();
                        // Stub: Not fully implemented
                        self.cycles += 3;
                        3
                    }
                    0xA5 => {
                        let _modrm = self.fetch_u8();
                        // SHLD with CL
                        // Stub: Not fully implemented
                        self.cycles += 3;
                        3
                    }
                    // SHRD - Double Precision Shift Right (0x0F 0xAC, 0xAD) - 80386+
                    0xAC => {
                        let modrm = self.fetch_u8();
                        let (_modbits, _reg, _rm) = Self::decode_modrm(modrm);
                        let _count = self.fetch_u8();
                        // Stub: Not fully implemented
                        self.cycles += 3;
                        3
                    }
                    0xAD => {
                        let _modrm = self.fetch_u8();
                        // SHRD with CL
                        // Stub: Not fully implemented
                        self.cycles += 3;
                        3
                    }
                    // SETcc - Set Byte on Condition (0x0F 0x90-0x9F) - 80386+
                    0x90..=0x9F => {
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
                    _ => {
                        eprintln!(
                            "Two-byte opcode 0x0F 0x{:02X} not implemented at CS:IP={:04X}:{:04X}",
                            next_opcode,
                            self.cs,
                            self.ip.wrapping_sub(2)
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

                self.write_rmw8(modbits, rm, result, seg, offset);
                self.update_flags_8(result);
                self.set_flag(FLAG_CF, carry);
                self.set_flag(FLAG_OF, overflow);
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
                let overflow = ((rm_val ^ result) & (reg_val ^ result) & 0x8000) != 0;

                self.write_rmw16(modbits, rm, result, seg, offset);
                self.update_flags_16(result);
                self.set_flag(FLAG_CF, carry);
                self.set_flag(FLAG_OF, overflow);
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

                if reg < 4 {
                    self.set_reg8_low(reg, result);
                } else {
                    self.set_reg8_high(reg - 4, result);
                }
                self.update_flags_8(result);
                self.set_flag(FLAG_CF, carry);
                self.set_flag(FLAG_OF, overflow);
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
                let overflow = ((reg_val ^ result) & (rm_val ^ result) & 0x8000) != 0;

                self.set_reg16(reg, result);
                self.update_flags_16(result);
                self.set_flag(FLAG_CF, carry);
                self.set_flag(FLAG_OF, overflow);
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

                self.ax = (self.ax & 0xFF00) | (result as u16);
                self.update_flags_8(result);
                self.set_flag(FLAG_CF, carry);
                self.set_flag(FLAG_OF, overflow);
                self.cycles += 4;
                4
            }

            // ADC AX, imm16 (0x15) - Add with Carry
            0x15 => {
                let val = self.fetch_u16();
                let carry_in = if self.get_flag(FLAG_CF) { 1 } else { 0 };
                let result = self.ax.wrapping_add(val).wrapping_add(carry_in);
                let carry = (self.ax as u32 + val as u32 + carry_in as u32) > 0xFFFF;
                let overflow = ((self.ax ^ result) & (val ^ result) & 0x8000) != 0;

                self.ax = result;
                self.update_flags_16(result);
                self.set_flag(FLAG_CF, carry);
                self.set_flag(FLAG_OF, overflow);
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

                self.write_rmw8(modbits, rm, result, seg, offset);
                self.update_flags_8(result);
                self.set_flag(FLAG_CF, borrow);
                self.set_flag(FLAG_OF, overflow);
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
                let overflow = ((rm_val ^ reg_val) & (rm_val ^ result) & 0x8000) != 0;

                self.write_rmw16(modbits, rm, result, seg, offset);
                self.update_flags_16(result);
                self.set_flag(FLAG_CF, borrow);
                self.set_flag(FLAG_OF, overflow);
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

                if reg < 4 {
                    self.set_reg8_low(reg, result);
                } else {
                    self.set_reg8_high(reg - 4, result);
                }
                self.update_flags_8(result);
                self.set_flag(FLAG_CF, borrow);
                self.set_flag(FLAG_OF, overflow);
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
                let overflow = ((reg_val ^ rm_val) & (reg_val ^ result) & 0x8000) != 0;

                self.set_reg16(reg, result);
                self.update_flags_16(result);
                self.set_flag(FLAG_CF, borrow);
                self.set_flag(FLAG_OF, overflow);
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

                self.ax = (self.ax & 0xFF00) | (result as u16);
                self.update_flags_8(result);
                self.set_flag(FLAG_CF, borrow);
                self.set_flag(FLAG_OF, overflow);
                self.cycles += 4;
                4
            }

            // SBB AX, imm16 (0x1D) - Subtract with Borrow
            0x1D => {
                let val = self.fetch_u16();
                let carry = if self.get_flag(FLAG_CF) { 1 } else { 0 };
                let result = self.ax.wrapping_sub(val).wrapping_sub(carry);
                let borrow = (self.ax as u32) < (val as u32 + carry as u32);
                let overflow = ((self.ax ^ val) & (self.ax ^ result) & 0x8000) != 0;

                self.ax = result;
                self.update_flags_16(result);
                self.set_flag(FLAG_CF, borrow);
                self.set_flag(FLAG_OF, overflow);
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
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);
                let reg_val = self.get_reg16(reg);
                let (rm_val, seg, offset) = self.read_rmw16(modbits, rm);
                let result = rm_val & reg_val;

                self.write_rmw16(modbits, rm, result, seg, offset);
                self.update_flags_16(result);
                self.set_flag(FLAG_CF, false);
                self.set_flag(FLAG_OF, false);
                self.cycles += if modbits == 0b11 { 3 } else { 16 };
                if modbits == 0b11 {
                    3
                } else {
                    16
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

            // AND r16, r/m16 (0x23)
            0x23 => {
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);
                let reg_val = self.get_reg16(reg);
                let rm_val = self.read_rm16(modbits, rm);
                let result = reg_val & rm_val;

                self.set_reg16(reg, result);
                self.update_flags_16(result);
                self.set_flag(FLAG_CF, false);
                self.set_flag(FLAG_OF, false);
                self.cycles += if modbits == 0b11 { 3 } else { 9 };
                if modbits == 0b11 {
                    3
                } else {
                    9
                }
            }

            // XOR AL, imm8
            0x34 => {
                let val = self.fetch_u8();
                let al = (self.ax & 0xFF) as u8;
                let result = al ^ val;

                self.ax = (self.ax & 0xFF00) | (result as u16);
                self.update_flags_8(result);
                self.set_flag(FLAG_CF, false);
                self.set_flag(FLAG_OF, false);
                self.cycles += 4;
                4
            }

            // XOR AX, imm16
            0x35 => {
                let val = self.fetch_u16();
                let result = self.ax ^ val;

                self.ax = result;
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
                    self.ax = self.ax.wrapping_add(0x106); // Add 1 to AH, 6 to AL
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
                    self.ax = self.ax.wrapping_sub(6); // Subtract 6 from AL
                    self.ax = (self.ax & 0xFF) | ((self.ax.wrapping_sub(0x100)) & 0xFF00); // Subtract 1 from AH
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
                    self.ip = self.ip.wrapping_add(offset as i16 as u16);
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
                    self.ip = self.ip.wrapping_add(offset as i16 as u16);
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
                    self.ip = self.ip.wrapping_add(offset as i16 as u16);
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
                    self.ip = self.ip.wrapping_add(offset as i16 as u16);
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
                    self.ip = self.ip.wrapping_add(offset as i16 as u16);
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
                    self.ip = self.ip.wrapping_add(offset as i16 as u16);
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
                    self.ip = self.ip.wrapping_add(offset as i16 as u16);
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
                    self.ip = self.ip.wrapping_add(offset as i16 as u16);
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
                    self.ip = self.ip.wrapping_add(offset as i16 as u16);
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
                    self.ip = self.ip.wrapping_add(offset as i16 as u16);
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
                    self.ip = self.ip.wrapping_add(offset as i16 as u16);
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
                    self.ip = self.ip.wrapping_add(offset as i16 as u16);
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
                    self.ip = self.ip.wrapping_add(offset as i16 as u16);
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
                    self.ip = self.ip.wrapping_add(offset as i16 as u16);
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
                    self.ip = self.ip.wrapping_add(offset as i16 as u16);
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
                        self.set_flag(FLAG_CF, carry);
                        self.set_flag(FLAG_OF, overflow);
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
                        self.set_flag(FLAG_CF, carry);
                        self.set_flag(FLAG_OF, overflow);
                        r
                    }
                    3 => {
                        // SBB
                        let carry_in = if self.get_flag(FLAG_CF) { 1 } else { 0 };
                        let r = rm_val.wrapping_sub(imm).wrapping_sub(carry_in);
                        let borrow = (rm_val as u16) < (imm as u16 + carry_in as u16);
                        let overflow = ((rm_val ^ imm) & (rm_val ^ r) & 0x80) != 0;
                        self.set_flag(FLAG_CF, borrow);
                        self.set_flag(FLAG_OF, overflow);
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
                        self.set_flag(FLAG_CF, borrow);
                        self.set_flag(FLAG_OF, overflow);
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
                        self.update_flags_8(r);
                        self.set_flag(FLAG_CF, borrow);
                        self.set_flag(FLAG_OF, overflow);
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
                let (rm_val, cached_seg, cached_offset) = self.read_rmw16(modbits, rm);
                let imm = self.fetch_u16();
                let result = match op {
                    0 => {
                        // ADD
                        let r = rm_val.wrapping_add(imm);
                        let carry = (rm_val as u32 + imm as u32) > 0xFFFF;
                        let overflow = ((rm_val ^ r) & (imm ^ r) & 0x8000) != 0;
                        self.set_flag(FLAG_CF, carry);
                        self.set_flag(FLAG_OF, overflow);
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
                        self.set_flag(FLAG_CF, carry);
                        self.set_flag(FLAG_OF, overflow);
                        r
                    }
                    3 => {
                        // SBB
                        let carry_in = if self.get_flag(FLAG_CF) { 1 } else { 0 };
                        let r = rm_val.wrapping_sub(imm).wrapping_sub(carry_in);
                        let borrow = (rm_val as u32) < (imm as u32 + carry_in as u32);
                        let overflow = ((rm_val ^ imm) & (rm_val ^ r) & 0x8000) != 0;
                        self.set_flag(FLAG_CF, borrow);
                        self.set_flag(FLAG_OF, overflow);
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
                        self.set_flag(FLAG_CF, borrow);
                        self.set_flag(FLAG_OF, overflow);
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
                        self.update_flags_16(r);
                        self.set_flag(FLAG_CF, borrow);
                        self.set_flag(FLAG_OF, overflow);
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
                        self.set_flag(FLAG_CF, carry);
                        self.set_flag(FLAG_OF, overflow);
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
                        self.set_flag(FLAG_CF, carry);
                        self.set_flag(FLAG_OF, overflow);
                        r
                    }
                    3 => {
                        // SBB
                        let carry_in = if self.get_flag(FLAG_CF) { 1 } else { 0 };
                        let r = rm_val.wrapping_sub(imm).wrapping_sub(carry_in);
                        let borrow = (rm_val as u32) < (imm as u32 + carry_in as u32);
                        let overflow = ((rm_val ^ imm) & (rm_val ^ r) & 0x8000) != 0;
                        self.set_flag(FLAG_CF, borrow);
                        self.set_flag(FLAG_OF, overflow);
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
                        self.set_flag(FLAG_CF, borrow);
                        self.set_flag(FLAG_OF, overflow);
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
                        self.update_flags_16(r);
                        self.set_flag(FLAG_CF, borrow);
                        self.set_flag(FLAG_OF, overflow);
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
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);
                let reg_val = self.get_reg16(reg);
                let rm_val = self.read_rm16(modbits, rm);
                let result = rm_val & reg_val;

                self.update_flags_16(result);
                self.set_flag(FLAG_CF, false);
                self.set_flag(FLAG_OF, false);
                self.cycles += if modbits == 0b11 { 3 } else { 9 };
                if modbits == 0b11 {
                    3
                } else {
                    9
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
                    let (_, offset_ea, _) = self.calc_effective_address(modbits, rm);
                    self.set_reg16(reg, offset_ea);
                }
                self.cycles += 2;
                2
            }

            // XCHG with AX (0x91-0x97)
            0x91..=0x97 => {
                let reg = opcode & 0x07;
                let temp = self.ax;
                self.ax = self.get_reg16(reg);
                self.set_reg16(reg, temp);
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
                self.dx = sign_extend;
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
                self.push(self.flags);
                self.cycles += 10;
                10
            }

            // POPF - Pop Flags (0x9D)
            0x9D => {
                self.flags = self.pop();
                self.cycles += 8;
                8
            }

            // SAHF - Store AH into Flags (0x9E)
            0x9E => {
                let ah = ((self.ax >> 8) & 0xFF) as u8;
                self.flags = (self.flags & 0xFF00) | (ah as u16);
                self.cycles += 4;
                4
            }

            // LAHF - Load AH from Flags (0x9F)
            0x9F => {
                let flags_low = (self.flags & 0xFF) as u8;
                self.ax = (self.ax & 0x00FF) | ((flags_low as u16) << 8);
                self.cycles += 4;
                4
            }

            // MOV AL, moffs8 (0xA0) - Direct memory to AL
            0xA0 => {
                let addr = self.fetch_u16();
                let val = self.read(self.ds, addr);
                self.ax = (self.ax & 0xFF00) | (val as u16);
                self.cycles += 10;
                10
            }

            // MOV AX, moffs16 (0xA1) - Direct memory to AX
            0xA1 => {
                let addr = self.fetch_u16();
                let val = self.read_u16(self.ds, addr);
                self.ax = val;
                self.cycles += 10;
                10
            }

            // MOV moffs8, AL (0xA2) - AL to direct memory
            0xA2 => {
                let addr = self.fetch_u16();
                let al = (self.ax & 0xFF) as u8;
                self.write(self.ds, addr, al);
                self.cycles += 10;
                10
            }

            // MOV moffs16, AX (0xA3) - AX to direct memory
            0xA3 => {
                let addr = self.fetch_u16();
                self.write_u16(self.ds, addr, self.ax);
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
                let result = self.ax & val;

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
                    // TEST r/m8, imm8
                    0b000 => {
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
                        self.write_rmw8(modbits, rm, result, seg, offset);
                        self.update_flags_8(result);
                        self.set_flag(FLAG_CF, val != 0);
                        self.set_flag(FLAG_OF, val == 0x80);
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
                        self.ax = result;
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
                        self.ax = result as u16;
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
                            // Division by zero - trigger INT 0 fault
                            self.trigger_fault(0, instruction_start_ip);
                        } else {
                            let dividend = self.ax;
                            let quotient = dividend / (divisor as u16);
                            let remainder = dividend % (divisor as u16);
                            // Check for overflow (quotient > 255)
                            if quotient > 0xFF {
                                // Division overflow - trigger INT 0 fault
                                self.trigger_fault(0, instruction_start_ip);
                            } else {
                                self.ax = (remainder << 8) | quotient;
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
                            // Division by zero - trigger INT 0 fault
                            self.trigger_fault(0, instruction_start_ip);
                        } else {
                            let dividend = self.ax as i16;
                            let quotient = dividend / (divisor as i16);
                            let remainder = dividend % (divisor as i16);
                            // Check for overflow (quotient out of -128..127 range)
                            if !(-128..=127).contains(&quotient) {
                                // Division overflow - trigger INT 0 fault
                                self.trigger_fault(0, instruction_start_ip);
                            } else {
                                let quot_u8 = quotient as u8;
                                let rem_u8 = remainder as u8;
                                self.ax = ((rem_u8 as u16) << 8) | (quot_u8 as u16);
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
                            self.ip.wrapping_sub(2)
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
                        self.update_flags_16(result);
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
                        self.write_rmw16(modbits, rm, result, seg, offset);
                        self.update_flags_16(result);
                        self.set_flag(FLAG_CF, val != 0);
                        self.set_flag(FLAG_OF, val == 0x8000);
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
                        let result = (self.ax as u32) * (val as u32);
                        self.ax = (result & 0xFFFF) as u16;
                        self.dx = ((result >> 16) & 0xFFFF) as u16;
                        // CF and OF are set if DX is non-zero
                        let high_word_set = self.dx != 0;
                        self.set_flag(FLAG_CF, high_word_set);
                        self.set_flag(FLAG_OF, high_word_set);
                        self.update_flags_16(self.ax);
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
                        let ax_signed = self.ax as i16;
                        let result = (ax_signed as i32) * (val as i32);
                        self.ax = (result & 0xFFFF) as u16;
                        self.dx = ((result >> 16) & 0xFFFF) as u16;
                        // CF and OF are set if sign extension of AX != DX
                        let sign_extended = if (self.ax & 0x8000) != 0 {
                            0xFFFF
                        } else {
                            0x0000
                        };
                        let overflow = self.dx != sign_extended;
                        self.set_flag(FLAG_CF, overflow);
                        self.set_flag(FLAG_OF, overflow);
                        self.update_flags_16(self.ax);
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
                            // Division by zero - trigger INT 0 fault
                            self.trigger_fault(0, instruction_start_ip);
                        } else {
                            let dividend = ((self.dx as u32) << 16) | (self.ax as u32);
                            let quotient = dividend / (divisor as u32);
                            let remainder = dividend % (divisor as u32);
                            // Check for overflow (quotient > 65535)
                            if quotient > 0xFFFF {
                                // Division overflow - trigger INT 0 fault
                                self.trigger_fault(0, instruction_start_ip);
                            } else {
                                self.ax = quotient as u16;
                                self.dx = remainder as u16;
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
                            // Division by zero - trigger INT 0 fault
                            self.trigger_fault(0, instruction_start_ip);
                        } else {
                            let dividend = (((self.dx as u32) << 16) | (self.ax as u32)) as i32;
                            let quotient = dividend / (divisor as i32);
                            let remainder = dividend % (divisor as i32);
                            // Check for overflow (quotient out of -32768..32767 range)
                            if !(-32768..=32767).contains(&quotient) {
                                // Division overflow - trigger INT 0 fault
                                self.trigger_fault(0, instruction_start_ip);
                            } else {
                                self.ax = quotient as u16;
                                self.dx = remainder as u16;
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
                            self.ip.wrapping_sub(2)
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
                    // Undefined
                    eprintln!(
                        "Undefined 0xC6 operation with op={} at CS:IP={:04X}:{:04X}",
                        op,
                        self.cs,
                        self.ip.wrapping_sub(2)
                    );
                    self.cycles += 1;
                    1
                }
            }

            // MOV r/m16, imm16 (0xC7) - Group 11
            0xC7 => {
                let modrm = self.fetch_u8();
                let (modbits, op, rm) = Self::decode_modrm(modrm);
                if op == 0 {
                    // Only op=0 is valid for MOV
                    // IMPORTANT: For memory operands with displacement, we must calculate
                    // the effective address (which fetches displacement bytes) BEFORE
                    // fetching the immediate value.
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
                } else {
                    // Undefined
                    eprintln!(
                        "Undefined 0xC7 operation with op={} at CS:IP={:04X}:{:04X}",
                        op,
                        self.cs,
                        self.ip.wrapping_sub(2)
                    );
                    self.cycles += 1;
                    1
                }
            }

            // Group 2 opcodes (0xC0) - Shift/rotate r/m8 by immediate byte (80186+)
            0xC0 => {
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
                // Debug: check what bytes we're about to read
                if LogConfig::global().should_log(LogCategory::CPU, LogLevel::Trace) {
                    let ip_before = self.ip;
                    let byte1 = self.read(self.cs, ip_before);
                    let byte2 = self.read(self.cs, ip_before.wrapping_add(1));
                    let byte3 = self.read(self.cs, ip_before.wrapping_add(2));
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
                self.push(self.bp);
                let frame_temp = self.sp;
                self.bp = frame_temp;
                self.sp = self.sp.wrapping_sub(size);

                if LogConfig::global().should_log(LogCategory::CPU, LogLevel::Trace) {
                    eprintln!("[ENTER] BP after={:04X}, SP after={:04X}", self.bp, self.sp);
                }

                self.cycles += 15;
                15
            }

            // LEAVE (0xC9) - 80186+ instruction
            0xC9 => {
                self.sp = self.bp;
                self.bp = self.pop();
                self.cycles += 8;
                8
            }

            // INT 3 (0xCC) - Breakpoint interrupt
            0xCC => {
                // Push flags, CS, IP
                self.push(self.flags);
                self.push(self.cs);
                self.push(self.ip);

                // Clear IF and TF
                self.set_flag(FLAG_IF, false);
                self.set_flag(FLAG_TF, false);

                // Jump to interrupt vector 3
                let ivt_offset = 3 * 4;
                let new_ip = self.read_u16(0, ivt_offset);
                let new_cs = self.read_u16(0, ivt_offset + 2);

                self.ip = new_ip;
                self.cs = new_cs;

                self.cycles += 52;
                52
            }

            // INTO - Interrupt on Overflow (0xCE)
            0xCE => {
                if self.get_flag(FLAG_OF) {
                    // Push flags, CS, IP
                    self.push(self.flags);
                    self.push(self.cs);
                    self.push(self.ip);

                    // Clear IF and TF
                    self.set_flag(FLAG_IF, false);
                    self.set_flag(FLAG_TF, false);

                    // Jump to interrupt vector 4
                    let ivt_offset = 4 * 4;
                    let new_ip = self.read_u16(0, ivt_offset);
                    let new_cs = self.read_u16(0, ivt_offset + 2);

                    self.ip = new_ip;
                    self.cs = new_cs;

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
                    // Division by zero - trigger INT 0 fault (divide error exception)
                    self.trigger_fault(0, instruction_start_ip);
                    self.cycles += 51; // Same as INT instruction
                    51
                } else {
                    let ah = al / base;
                    let al_new = al % base;
                    self.ax = ((ah as u16) << 8) | (al_new as u16);
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
                self.ax = (self.ax & 0xFF00) | (result as u16);
                self.ax &= 0x00FF; // Clear AH
                self.update_flags_8(result);
                self.cycles += 60;
                60
            }

            // SALC/SETALC (0xD6) - Undocumented opcode
            0xD6 => {
                // Set AL on Carry
                let al = if self.get_flag(FLAG_CF) { 0xFF } else { 0x00 };
                self.ax = (self.ax & 0xFF00) | (al as u16);
                self.cycles += 3;
                3
            }

            // XLAT/XLATB (0xD7)
            0xD7 => {
                let al = (self.ax & 0xFF) as u8;
                let offset = self.bx.wrapping_add(al as u16);
                let val = self.read(self.ds, offset);
                self.ax = (self.ax & 0xFF00) | (val as u16);
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
                    self.ip = self.ip.wrapping_add(offset as i16 as u16);
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
                    self.ip = self.ip.wrapping_add(offset as i16 as u16);
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
                    self.ip = self.ip.wrapping_add(offset as i16 as u16);
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
                    self.ip = self.ip.wrapping_add(offset as i16 as u16);
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
                self.ip = self.ip.wrapping_add(offset as u16);
                self.cycles += 15;
                15
            }

            // JMP far absolute (0xEA)
            0xEA => {
                let offset = self.fetch_u16();
                let segment = self.fetch_u16();
                self.ip = offset;
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
                        let result = val.wrapping_add(1);
                        let overflow = val == 0x7F;
                        self.write_rmw8(modbits, rm, result, seg, offset);
                        self.update_flags_8(result);
                        self.set_flag(FLAG_OF, overflow);
                    }
                    1 => {
                        // DEC r/m8 - use RMW helpers to avoid double-fetching displacement
                        let (val, seg, offset) = self.read_rmw8(modbits, rm);
                        let result = val.wrapping_sub(1);
                        let overflow = val == 0x80;
                        self.write_rmw8(modbits, rm, result, seg, offset);
                        self.update_flags_8(result);
                        self.set_flag(FLAG_OF, overflow);
                    }
                    _ => {
                        eprintln!(
                            "Undefined 0xFE operation with op={} at CS:IP={:04X}:{:04X}",
                            op,
                            self.cs,
                            self.ip.wrapping_sub(2)
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
                        self.write_rmw16(modbits, rm, result, seg, offset);
                        self.update_flags_16(result);
                        self.set_flag(FLAG_OF, overflow);
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
                        self.write_rmw16(modbits, rm, result, seg, offset);
                        self.update_flags_16(result);
                        self.set_flag(FLAG_OF, overflow);
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
                        self.push(self.ip);
                        self.ip = target;
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
                        self.push(self.ip);
                        self.ip = offset;
                        self.cs = segment;
                        self.cycles += 37;
                        37
                    }
                    4 => {
                        // JMP r/m16 (near)
                        let target = self.read_rm16(modbits, rm);
                        self.ip = target;
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
                        self.ip = offset;
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
                        eprintln!(
                            "Undefined 0xFF operation with op={} at CS:IP={:04X}:{:04X}",
                            op,
                            self.cs,
                            self.ip.wrapping_sub(2)
                        );
                        // For undefined operations, we'll just NOP and continue
                        // This prevents the system from crashing completely
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

                self.set_reg16(reg, result);
                self.update_flags_16(result);
                self.set_flag(FLAG_OF, overflow);
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

                self.set_reg16(reg, result);
                self.update_flags_16(result);
                self.set_flag(FLAG_OF, overflow);
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
                let temp_sp = self.sp;
                self.push(self.ax);
                self.push(self.cx);
                self.push(self.dx);
                self.push(self.bx);
                self.push(temp_sp); // Push original SP value
                self.push(self.bp);
                self.push(self.si);
                self.push(self.di);
                self.cycles += 36;
                36
            }

            // POPA - Pop All General Registers (0x61) - 80186+
            0x61 => {
                self.di = self.pop();
                self.si = self.pop();
                self.bp = self.pop();
                let _temp_sp = self.pop(); // Discard SP value
                self.bx = self.pop();
                self.dx = self.pop();
                self.cx = self.pop();
                self.ax = self.pop();
                self.cycles += 51;
                51
            }

            // BOUND - Check Array Index Against Bounds (0x62) - 80186+
            0x62 => {
                let modrm = self.fetch_u8();
                let (modbits, reg, rm) = Self::decode_modrm(modrm);
                let index = self.get_reg16(reg) as i16;
                let (_seg, ea, _bytes) = self.calc_effective_address(modbits, rm);
                let lower_bound = self.read_u16(self.ds, ea) as i16;
                let upper_bound = self.read_u16(self.ds, ea.wrapping_add(2)) as i16;

                // If index is out of bounds, generate INT 5
                if index < lower_bound || index > upper_bound {
                    // Push flags, CS, IP
                    self.push(self.flags);
                    self.push(self.cs);
                    self.push(self.ip);

                    // Clear IF and TF
                    self.set_flag(FLAG_IF, false);
                    self.set_flag(FLAG_TF, false);

                    // Jump to interrupt vector 5
                    let ivt_offset = 5 * 4;
                    let new_ip = self.read_u16(0, ivt_offset);
                    let new_cs = self.read_u16(0, ivt_offset + 2);

                    self.ip = new_ip;
                    self.cs = new_cs;
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
                // FS segment override prefix
                self.segment_override = Some(SegmentOverride::FS);
                self.step() // Execute next instruction with FS override
            }

            // GS segment override prefix (0x65) - 80386+
            0x65 => {
                // GS segment override prefix
                self.segment_override = Some(SegmentOverride::GS);
                self.step() // Execute next instruction with GS override
            }

            // PUSH immediate word (0x68) - 80186+
            0x68 => {
                let val = self.fetch_u16();
                self.push(val);
                self.cycles += 3;
                3
            }

            // IMUL r16, r/m16, imm16 (0x69) - 80186+
            0x69 => {
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
                let val = self.fetch_u8() as i8 as i16 as u16; // Sign extend
                self.push(val);
                self.cycles += 3;
                3
            }

            // IMUL r16, r/m16, imm8 (0x6B) - 80186+
            0x6B => {
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
                // Read from I/O port DX, write to ES:DI
                let port = self.dx;
                let val = self.io_read(port);
                self.write(self.es, self.di, val);

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
                // Read from I/O port DX, write to ES:DI
                let port = self.dx;
                let val = self.io_read_word(port);
                self.write_u16(self.es, self.di, val);

                // Update DI based on DF flag
                if self.get_flag(FLAG_DF) {
                    self.di = self.di.wrapping_sub(2);
                } else {
                    self.di = self.di.wrapping_add(2);
                }
                self.cycles += 14;
                14
            }

            // OUTSB - Output String Byte (0x6E) - 80186+
            0x6E => {
                // Read from DS:SI, write to I/O port DX
                let val = self.read(self.ds, self.si);
                let port = self.dx;
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
                // Read from DS:SI, write to I/O port DX
                let val = self.read_u16(self.ds, self.si);
                let port = self.dx;
                self.io_write_word(port, val);

                // Update SI based on DF flag
                if self.get_flag(FLAG_DF) {
                    self.si = self.si.wrapping_sub(2);
                } else {
                    self.si = self.si.wrapping_add(2);
                }
                self.cycles += 14;
                14
            }

            // JMP short (EB)
            0xEB => {
                let offset = self.fetch_u8() as i8;
                // Add signed offset to IP (wrapping_add_signed would be clearer but requires i16 cast)
                self.ip = self.ip.wrapping_add(offset as i16 as u16);
                self.cycles += 15;
                15
            }

            // JZ/JE (74) - Jump if Zero
            0x74 => {
                let offset = self.fetch_u8() as i8;
                if self.get_flag(FLAG_ZF) {
                    self.ip = self.ip.wrapping_add(offset as i16 as u16);
                    self.cycles += 16;
                    16
                } else {
                    self.cycles += 4;
                    4
                }
            }

            // MOVSB - Move String Byte (0xA4)
            0xA4 => {
                // Move byte from DS:SI to ES:DI
                let val = self.read(self.ds, self.si);
                self.write(self.es, self.di, val);

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
                // Move word from DS:SI to ES:DI
                let val = self.read_u16(self.ds, self.si);
                self.write_u16(self.es, self.di, val);

                // Update SI and DI based on DF flag
                if self.get_flag(FLAG_DF) {
                    self.si = self.si.wrapping_sub(2);
                    self.di = self.di.wrapping_sub(2);
                } else {
                    self.si = self.si.wrapping_add(2);
                    self.di = self.di.wrapping_add(2);
                }
                self.cycles += 18;
                18
            }

            // CMPSB - Compare String Byte (0xA6)
            0xA6 => {
                // Compare byte at DS:SI with byte at ES:DI
                let src = self.read(self.ds, self.si);
                let dst = self.read(self.es, self.di);
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
                // Compare word at DS:SI with word at ES:DI
                let src = self.read_u16(self.ds, self.si);
                let dst = self.read_u16(self.es, self.di);
                let result = src.wrapping_sub(dst);
                let borrow = (src as u32) < (dst as u32);
                let overflow = ((src ^ dst) & (src ^ result) & 0x8000) != 0;

                self.update_flags_16(result);
                self.set_flag(FLAG_CF, borrow);
                self.set_flag(FLAG_OF, overflow);

                // Update SI and DI
                if self.get_flag(FLAG_DF) {
                    self.si = self.si.wrapping_sub(2);
                    self.di = self.di.wrapping_sub(2);
                } else {
                    self.si = self.si.wrapping_add(2);
                    self.di = self.di.wrapping_add(2);
                }
                self.cycles += 22;
                22
            }

            // STOSB - Store String Byte (0xAA)
            0xAA => {
                // Store AL to ES:DI
                let al = (self.ax & 0xFF) as u8;
                self.write(self.es, self.di, al);

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
                self.write_u16(self.es, self.di, self.ax);

                // Update DI
                if self.get_flag(FLAG_DF) {
                    self.di = self.di.wrapping_sub(2);
                } else {
                    self.di = self.di.wrapping_add(2);
                }
                self.cycles += 11;
                11
            }

            // LODSB - Load String Byte (0xAC)
            0xAC => {
                // Load byte from DS:SI into AL
                let val = self.read(self.ds, self.si);
                self.ax = (self.ax & 0xFF00) | (val as u16);

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
                // Load word from DS:SI into AX
                self.ax = self.read_u16(self.ds, self.si);

                // Update SI
                if self.get_flag(FLAG_DF) {
                    self.si = self.si.wrapping_sub(2);
                } else {
                    self.si = self.si.wrapping_add(2);
                }
                self.cycles += 12;
                12
            }

            // SCASB - Scan String Byte (0xAE)
            0xAE => {
                // Compare AL with byte at ES:DI
                let al = (self.ax & 0xFF) as u8;
                let val = self.read(self.es, self.di);
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
                let val = self.read_u16(self.es, self.di);
                let result = self.ax.wrapping_sub(val);
                let borrow = (self.ax as u32) < (val as u32);
                let overflow = ((self.ax ^ val) & (self.ax ^ result) & 0x8000) != 0;

                self.update_flags_16(result);
                self.set_flag(FLAG_CF, borrow);
                self.set_flag(FLAG_OF, overflow);

                // Update DI
                if self.get_flag(FLAG_DF) {
                    self.di = self.di.wrapping_sub(2);
                } else {
                    self.di = self.di.wrapping_add(2);
                }
                self.cycles += 15;
                15
            }

            // CALL near relative (0xE8)
            0xE8 => {
                let offset = self.fetch_u16() as i16;
                // Push return address (current IP after fetching the offset)
                self.push(self.ip);
                // Jump to target (IP + offset)
                self.ip = self.ip.wrapping_add(offset as u16);
                self.cycles += 19;
                19
            }

            // RET near (0xC3)
            0xC3 => {
                self.ip = self.pop();
                self.cycles += 8;
                8
            }

            // RET near with immediate (0xC2) - pops return address and adds imm16 to SP
            0xC2 => {
                let pop_bytes = self.fetch_u16();
                self.ip = self.pop();
                self.sp = self.sp.wrapping_add(pop_bytes);
                self.cycles += 12;
                12
            }

            // CALL far absolute (0x9A)
            0x9A => {
                let new_ip = self.fetch_u16();
                let new_cs = self.fetch_u16();
                // Push current CS and IP
                self.push(self.cs);
                self.push(self.ip);
                // Jump to far address
                self.cs = new_cs;
                self.ip = new_ip;
                self.cycles += 28;
                28
            }

            // RET far (0xCB)
            0xCB => {
                self.ip = self.pop();
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
                        self.sp.wrapping_add(4),
                        pop_bytes,
                        ret_ip,
                        ret_cs
                    );
                }

                self.ip = ret_ip;
                self.cs = ret_cs;
                self.sp = self.sp.wrapping_add(pop_bytes);
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
                self.trigger_interrupt(int_num);
                self.cycles += 51; // Approximate timing for INT instruction
                51
            }

            // IRET - Return from Interrupt
            0xCF => {
                // Pop IP, CS, FLAGS from stack (reverse order of INT)
                self.ip = self.pop();
                self.cs = self.pop();
                self.flags = self.pop();

                self.cycles += 32; // Approximate timing for IRET instruction
                32
            }

            _ => {
                // Unknown/unimplemented opcode
                eprintln!(
                    "Unknown 8086 opcode: 0x{:02X} at CS:IP={:04X}:{:04X}",
                    opcode,
                    self.cs,
                    self.ip.wrapping_sub(1)
                );
                self.cycles += 1;
                1
            }
        }
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

    // Helper function for tests to calculate physical address
    fn physical_address(segment: u16, offset: u16) -> u32 {
        ((segment as u32) << 4) + (offset as u32)
    }

    #[test]
    fn test_cpu_initialization() {
        let mem = ArrayMemory::new();
        let cpu = Cpu8086::new(mem);

        assert_eq!(cpu.ax, 0);
        assert_eq!(cpu.bx, 0);
        assert_eq!(cpu.cx, 0);
        assert_eq!(cpu.dx, 0);
        assert_eq!(cpu.cs, 0xFFFF);
        assert_eq!(cpu.ds, 0);
        assert_eq!(cpu.flags & 0x0002, 0x0002); // Reserved bit
    }

    #[test]
    fn test_reset() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        cpu.ax = 0x1234;
        cpu.bx = 0x5678;
        cpu.flags = 0xFFFF;

        cpu.reset();

        assert_eq!(cpu.ax, 0);
        assert_eq!(cpu.bx, 0);
        assert_eq!(cpu.flags & 0x0002, 0x0002);
    }

    #[test]
    fn test_physical_address() {
        let addr = Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x0234);
        assert_eq!(addr, 0x10234);

        let addr = Cpu8086::<ArrayMemory>::physical_address(0xFFFF, 0xFFFF);
        assert_eq!(addr, 0x10FFEF);
    }

    #[test]
    fn test_mov_immediate_8bit() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // MOV AL, 0x42
        cpu.memory.load_program(0xFFFF0, &[0xB0, 0x42]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;

        let cycles = cpu.step();
        assert_eq!(cycles, 4);
        assert_eq!(cpu.ax & 0xFF, 0x42);
        assert_eq!((cpu.ax >> 8) & 0xFF, 0);
    }

    #[test]
    fn test_mov_immediate_16bit() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // MOV AX, 0x1234
        cpu.memory.load_program(0xFFFF0, &[0xB8, 0x34, 0x12]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;

        let cycles = cpu.step();
        assert_eq!(cycles, 4);
        assert_eq!(cpu.ax, 0x1234);
    }

    #[test]
    fn test_add_immediate() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // ADD AL, 0x10
        cpu.memory.load_program(0xFFFF0, &[0x04, 0x10]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.ax = 0x0005;

        cpu.step();
        assert_eq!(cpu.ax & 0xFF, 0x15);
        assert!(!cpu.get_flag(FLAG_ZF));
        assert!(!cpu.get_flag(FLAG_CF));
    }

    #[test]
    fn test_add_with_carry() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // ADD AL, 0xFF (0xFF + 0xFF = 0x1FE, should set carry)
        cpu.memory.load_program(0xFFFF0, &[0x04, 0xFF]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.ax = 0x00FF;

        cpu.step();
        assert_eq!(cpu.ax & 0xFF, 0xFE);
        assert!(cpu.get_flag(FLAG_CF));
    }

    #[test]
    fn test_sub_immediate() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // SUB AL, 0x05
        cpu.memory.load_program(0xFFFF0, &[0x2C, 0x05]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.ax = 0x0010;

        cpu.step();
        assert_eq!(cpu.ax & 0xFF, 0x0B);
        assert!(!cpu.get_flag(FLAG_CF));
    }

    #[test]
    fn test_sub_with_borrow() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // SUB AL, 0x10 (0x05 - 0x10, should set carry/borrow)
        cpu.memory.load_program(0xFFFF0, &[0x2C, 0x10]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.ax = 0x0005;

        cpu.step();
        assert_eq!(cpu.ax & 0xFF, 0xF5);
        assert!(cpu.get_flag(FLAG_CF));
    }

    #[test]
    fn test_cmp_sets_flags() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // CMP AL, 0x42 (should set zero flag when equal)
        cpu.memory.load_program(0xFFFF0, &[0x3C, 0x42]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.ax = 0x0042;

        cpu.step();
        assert_eq!(cpu.ax & 0xFF, 0x42); // CMP doesn't modify register
        assert!(cpu.get_flag(FLAG_ZF));
    }

    #[test]
    fn test_and_immediate() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // AND AL, 0x0F
        cpu.memory.load_program(0xFFFF0, &[0x24, 0x0F]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.ax = 0x00FF;

        cpu.step();
        assert_eq!(cpu.ax & 0xFF, 0x0F);
        assert!(!cpu.get_flag(FLAG_CF));
        assert!(!cpu.get_flag(FLAG_OF));
    }

    #[test]
    fn test_or_immediate() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // OR AL, 0xF0
        cpu.memory.load_program(0xFFFF0, &[0x0C, 0xF0]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.ax = 0x000F;

        cpu.step();
        assert_eq!(cpu.ax & 0xFF, 0xFF);
    }

    #[test]
    fn test_xor_immediate() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // XOR AL, 0xFF
        cpu.memory.load_program(0xFFFF0, &[0x34, 0xFF]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.ax = 0x00AA;

        cpu.step();
        assert_eq!(cpu.ax & 0xFF, 0x55);
    }

    #[test]
    fn test_inc_dec_register() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // INC AX
        cpu.memory.load_program(0xFFFF0, &[0x40]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.ax = 0x0010;

        cpu.step();
        assert_eq!(cpu.ax, 0x0011);

        // DEC BX
        cpu.memory.load_program(0xFFFF0, &[0x4B]);
        cpu.ip = 0x0000;
        cpu.bx = 0x0010;

        cpu.step();
        assert_eq!(cpu.bx, 0x000F);
    }

    #[test]
    fn test_push_pop() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);
        cpu.ss = 0x1000;
        cpu.sp = 0x0100;

        // PUSH AX
        cpu.memory.load_program(0xFFFF0, &[0x50]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.ax = 0x1234;

        cpu.step();
        assert_eq!(cpu.sp, 0x00FE);

        // POP BX
        cpu.memory.load_program(0xFFFF0, &[0x5B]);
        cpu.ip = 0x0000;

        cpu.step();
        assert_eq!(cpu.bx, 0x1234);
        assert_eq!(cpu.sp, 0x0100);
    }

    #[test]
    fn test_jump_short() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // JMP short +5
        cpu.memory.load_program(0xFFFF0, &[0xEB, 0x05]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;

        cpu.step();
        assert_eq!(cpu.ip, 0x0007); // 2 (instruction size) + 5 (offset)
    }

    #[test]
    fn test_conditional_jump_taken() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // JZ +3 (should jump when ZF is set)
        cpu.memory.load_program(0xFFFF0, &[0x74, 0x03]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.set_flag(FLAG_ZF, true);

        let cycles = cpu.step();
        assert_eq!(cycles, 16);
        assert_eq!(cpu.ip, 0x0005); // 2 + 3
    }

    #[test]
    fn test_conditional_jump_not_taken() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // JZ +3 (should not jump when ZF is clear)
        cpu.memory.load_program(0xFFFF0, &[0x74, 0x03]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.set_flag(FLAG_ZF, false);

        let cycles = cpu.step();
        assert_eq!(cycles, 4);
        assert_eq!(cpu.ip, 0x0002); // Just past instruction
    }

    #[test]
    fn test_flag_instructions() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // CLC
        cpu.memory.load_program(0xFFFF0, &[0xF8]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.set_flag(FLAG_CF, true);

        cpu.step();
        assert!(!cpu.get_flag(FLAG_CF));

        // STC
        cpu.memory.load_program(0xFFFF0, &[0xF9]);
        cpu.ip = 0x0000;

        cpu.step();
        assert!(cpu.get_flag(FLAG_CF));
    }

    #[test]
    fn test_nop() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        cpu.memory.load_program(0xFFFF0, &[0x90]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        let old_ip = cpu.ip;

        let cycles = cpu.step();
        assert_eq!(cycles, 3);
        assert_eq!(cpu.ip, old_ip + 1);
    }

    #[test]
    fn test_int_instruction() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // Setup interrupt vector for INT 0x10 at IVT address 0x0000:0x0040 (0x10 * 4)
        // IVT entry: offset=0x1000, segment=0xF000
        cpu.memory.write(0x0040, 0x00); // IP low
        cpu.memory.write(0x0041, 0x10); // IP high
        cpu.memory.write(0x0042, 0x00); // CS low
        cpu.memory.write(0x0043, 0xF0); // CS high

        // Load INT 0x10 instruction at CS:IP
        cpu.memory.load_program(0xFFFF0, &[0xCD, 0x10]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.ss = 0x0000;
        cpu.sp = 0xFFFE;
        cpu.flags = 0x0202; // IF=1

        let old_ip = cpu.ip;
        let old_cs = cpu.cs;
        let old_flags = cpu.flags;

        cpu.step();

        // Check that CS:IP jumped to interrupt handler
        assert_eq!(cpu.cs, 0xF000);
        assert_eq!(cpu.ip, 0x1000);

        // Check that FLAGS, CS, IP were pushed to stack
        assert_eq!(cpu.sp, 0xFFF8); // Stack pointer moved down by 6 bytes

        // Read pushed values from stack (pushed in order: FLAGS, CS, IP)
        // Last pushed (IP) is at SP, first pushed (FLAGS) is at SP+4
        let pushed_ip = cpu.memory.read(0xFFF8) as u16 | ((cpu.memory.read(0xFFF9) as u16) << 8);
        let pushed_cs = cpu.memory.read(0xFFFA) as u16 | ((cpu.memory.read(0xFFFB) as u16) << 8);
        let pushed_flags = cpu.memory.read(0xFFFC) as u16 | ((cpu.memory.read(0xFFFD) as u16) << 8);

        // IP should point to next instruction (after INT)
        assert_eq!(pushed_ip, old_ip + 2);
        assert_eq!(pushed_cs, old_cs);
        assert_eq!(pushed_flags, old_flags);

        // Check that IF flag was cleared
        assert!(!cpu.get_flag(FLAG_IF));
    }

    #[test]
    fn test_iret_instruction() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // Setup stack with return values
        cpu.ss = 0x0000;
        cpu.sp = 0xFFF8;

        // IRET pops in order: IP, CS, FLAGS
        // So stack layout from SP upwards is: IP, CS, FLAGS
        cpu.memory.write(0xFFF8, 0x78); // IP low
        cpu.memory.write(0xFFF9, 0x56); // IP high
        cpu.memory.write(0xFFFA, 0x34); // CS low
        cpu.memory.write(0xFFFB, 0x12); // CS high
        cpu.memory.write(0xFFFC, 0x02); // FLAGS low
        cpu.memory.write(0xFFFD, 0x02); // FLAGS high

        // Load IRET instruction
        cpu.memory.load_program(0xF0000, &[0xCF]);
        cpu.ip = 0x0000;
        cpu.cs = 0xF000;

        cpu.step();

        // Check that IP, CS, FLAGS were popped
        assert_eq!(cpu.ip, 0x5678);
        assert_eq!(cpu.cs, 0x1234);
        assert_eq!(cpu.flags, 0x0202);
        assert_eq!(cpu.sp, 0xFFFE); // Stack pointer restored
    }

    #[test]
    fn test_halt() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        cpu.memory.load_program(0xFFFF0, &[0xF4]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;

        cpu.step();
        assert!(cpu.halted);

        // Further steps should do nothing
        let cycles = cpu.step();
        assert_eq!(cycles, 1);
    }

    #[test]
    fn test_parity_flag() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // AND AL, 0x03 (result = 0x03, has 2 ones = even parity)
        cpu.memory.load_program(0xFFFF0, &[0x24, 0x03]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.ax = 0x00FF;

        cpu.step();
        assert!(cpu.get_flag(FLAG_PF));

        // AND AL, 0x01 (result = 0x01, has 1 one = odd parity)
        cpu.memory.load_program(0xFFFF0, &[0x24, 0x01]);
        cpu.ip = 0x0000;
        cpu.ax = 0x00FF;

        cpu.step();
        assert!(!cpu.get_flag(FLAG_PF));
    }

    #[test]
    fn test_call_near() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        cpu.ss = 0x1000;
        cpu.sp = 0x0100;
        cpu.cs = 0x2000;
        cpu.ip = 0x0010;

        // CALL near with offset +0x0050 (0xE8, 0x50, 0x00)
        cpu.memory.load_program(0x20010, &[0xE8, 0x50, 0x00]);

        let old_sp = cpu.sp;
        cpu.step();

        // IP should be at offset location (0x0010 + 3 (instruction size) + 0x0050)
        assert_eq!(cpu.ip, 0x0063);

        // Stack should contain return address (0x0013 - after the CALL instruction)
        assert_eq!(cpu.sp, old_sp - 2);
        let return_addr = cpu.read_u16(cpu.ss, cpu.sp);
        assert_eq!(return_addr, 0x0013);
    }

    #[test]
    fn test_ret_near() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        cpu.ss = 0x1000;
        cpu.sp = 0x00FE;
        cpu.cs = 0x2000;

        // Push return address onto stack
        cpu.memory.write(
            Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x00FE),
            0x34,
        );
        cpu.memory.write(
            Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x00FF),
            0x12,
        );

        // RET (0xC3)
        cpu.memory.load_program(0x20000, &[0xC3]);
        cpu.ip = 0x0000;

        let old_sp = cpu.sp;
        cpu.step();

        // IP should be restored to return address
        assert_eq!(cpu.ip, 0x1234);
        // Stack pointer should be restored
        assert_eq!(cpu.sp, old_sp + 2);
    }

    #[test]
    fn test_ret_near_with_immediate() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        cpu.ss = 0x1000;
        cpu.sp = 0x00F8;
        cpu.cs = 0x2000;

        // Push return address onto stack
        cpu.memory.write(
            Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x00F8),
            0x78,
        );
        cpu.memory.write(
            Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x00F9),
            0x56,
        );

        // RET 0x0004 (0xC2, 0x04, 0x00) - pops return address and adds 4 to SP
        cpu.memory.load_program(0x20000, &[0xC2, 0x04, 0x00]);
        cpu.ip = 0x0000;

        cpu.step();

        // IP should be restored to return address
        assert_eq!(cpu.ip, 0x5678);
        // Stack pointer should be restored plus the immediate value
        assert_eq!(cpu.sp, 0x00F8 + 2 + 4); // Original SP + 2 (pop) + 4 (immediate)
    }

    #[test]
    fn test_call_ret_roundtrip() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        cpu.ss = 0x1000;
        cpu.sp = 0x0100;
        cpu.cs = 0x2000;
        cpu.ip = 0x0010;

        // CALL near with offset +0x0020
        cpu.memory.load_program(0x20010, &[0xE8, 0x20, 0x00]);
        cpu.step();
        assert_eq!(cpu.ip, 0x0033); // 0x0010 + 3 + 0x0020

        // RET
        cpu.memory.load_program(0x20033, &[0xC3]);
        cpu.step();
        assert_eq!(cpu.ip, 0x0013); // Return to address after CALL
        assert_eq!(cpu.sp, 0x0100); // Stack pointer restored
    }

    #[test]
    fn test_test_rm8_r8() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // TEST CL, AL (0x84 with ModR/M 0b11_000_001)
        cpu.memory.load_program(0xFFFF0, &[0x84, 0b11_000_001]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.ax = 0x00FF; // AL = 0xFF
        cpu.cx = 0x00AA; // CL = 0xAA

        let old_ax = cpu.ax;
        let old_cx = cpu.cx;
        cpu.step();

        // TEST doesn't modify operands
        assert_eq!(cpu.ax, old_ax);
        assert_eq!(cpu.cx, old_cx);

        // Flags should be set based on AL & CL = 0xFF & 0xAA = 0xAA
        assert!(!cpu.get_flag(FLAG_ZF)); // Result is not zero
        assert!(cpu.get_flag(FLAG_SF)); // Result has sign bit set
        assert!(!cpu.get_flag(FLAG_CF)); // CF cleared
        assert!(!cpu.get_flag(FLAG_OF)); // OF cleared
    }

    #[test]
    fn test_test_al_imm8_zero() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // TEST AL, 0x0F (0xA8, 0x0F)
        cpu.memory.load_program(0xFFFF0, &[0xA8, 0x0F]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.ax = 0x00F0; // AL = 0xF0

        cpu.step();

        // AL & 0x0F = 0xF0 & 0x0F = 0x00
        assert!(cpu.get_flag(FLAG_ZF)); // Result is zero
    }

    #[test]
    fn test_test_ax_imm16() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // TEST AX, 0x8000 (0xA9, 0x00, 0x80)
        cpu.memory.load_program(0xFFFF0, &[0xA9, 0x00, 0x80]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.ax = 0x8080;

        cpu.step();

        // AX & 0x8000 = 0x8080 & 0x8000 = 0x8000
        assert!(!cpu.get_flag(FLAG_ZF));
        assert!(cpu.get_flag(FLAG_SF)); // Sign bit set
    }

    #[test]
    fn test_not_r8() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // NOT AL (0xF6 with ModR/M 0b11_010_000)
        cpu.memory.load_program(0xFFFF0, &[0xF6, 0b11_010_000]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.ax = 0x00AA; // AL = 0xAA

        cpu.step();

        // AL should be ~0xAA = 0x55
        assert_eq!(cpu.ax & 0xFF, 0x55);
    }

    #[test]
    fn test_not_r16() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // NOT AX (0xF7 with ModR/M 0b11_010_000)
        cpu.memory.load_program(0xFFFF0, &[0xF7, 0b11_010_000]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.ax = 0xAAAA;

        cpu.step();

        // AX should be ~0xAAAA = 0x5555
        assert_eq!(cpu.ax, 0x5555);
    }

    #[test]
    fn test_not_memory() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        cpu.ds = 0x1000;
        cpu.bx = 0x0100;

        // Write value to memory
        let addr = Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x0100);
        cpu.memory.write(addr, 0xF0);

        // NOT byte ptr [BX] (0xF6 with ModR/M 0b00_010_111)
        cpu.memory.load_program(0xFFFF0, &[0xF6, 0b00_010_111]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;

        cpu.step();

        // Memory should contain ~0xF0 = 0x0F
        assert_eq!(cpu.memory.read(addr), 0x0F);
    }

    #[test]
    fn test_neg_r8() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // NEG AL (0xF6 with ModR/M 0b11_011_000)
        cpu.memory.load_program(0xFFFF0, &[0xF6, 0b11_011_000]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.ax = 0x0005; // AL = 5

        cpu.step();

        // AL should be -5 = 0xFB (two's complement)
        assert_eq!(cpu.ax & 0xFF, 0xFB);
        assert!(cpu.get_flag(FLAG_CF)); // CF set when operand is not zero
        assert!(!cpu.get_flag(FLAG_ZF));
        assert!(cpu.get_flag(FLAG_SF));
    }

    #[test]
    fn test_neg_r16() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // NEG AX (0xF7 with ModR/M 0b11_011_000)
        cpu.memory.load_program(0xFFFF0, &[0xF7, 0b11_011_000]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.ax = 0x1000;

        cpu.step();

        // AX should be -0x1000 = 0xF000 (two's complement)
        assert_eq!(cpu.ax, 0xF000);
        assert!(cpu.get_flag(FLAG_CF));
        assert!(cpu.get_flag(FLAG_SF));
    }

    #[test]
    fn test_neg_zero() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // NEG AL with AL=0 (0xF6 with ModR/M 0b11_011_000)
        cpu.memory.load_program(0xFFFF0, &[0xF6, 0b11_011_000]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.ax = 0x0000; // AL = 0

        cpu.step();

        // AL should remain 0
        assert_eq!(cpu.ax & 0xFF, 0);
        assert!(!cpu.get_flag(FLAG_CF)); // CF cleared when operand is zero
        assert!(cpu.get_flag(FLAG_ZF));
    }

    #[test]
    fn test_neg_overflow() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // NEG AL with AL=0x80 (0xF6 with ModR/M 0b11_011_000)
        cpu.memory.load_program(0xFFFF0, &[0xF6, 0b11_011_000]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.ax = 0x0080; // AL = -128

        cpu.step();

        // AL should become 0x80 (overflow: -(-128) cannot be represented in 8-bit signed)
        assert_eq!(cpu.ax & 0xFF, 0x80);
        assert!(cpu.get_flag(FLAG_OF)); // OF set for overflow
        assert!(cpu.get_flag(FLAG_CF)); // CF set when operand is not zero
    }

    #[test]
    fn test_call_far() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        cpu.ss = 0x1000;
        cpu.sp = 0x0100;
        cpu.cs = 0x2000;
        cpu.ip = 0x0010;

        // CALL far to 0x3000:0x0050 (0x9A, 0x50, 0x00, 0x00, 0x30)
        cpu.memory
            .load_program(0x20010, &[0x9A, 0x50, 0x00, 0x00, 0x30]);

        let old_sp = cpu.sp;
        cpu.step();

        // CS:IP should be at far address
        assert_eq!(cpu.cs, 0x3000);
        assert_eq!(cpu.ip, 0x0050);

        // Stack should contain old CS and IP
        assert_eq!(cpu.sp, old_sp - 4);
        let return_ip = cpu.read_u16(cpu.ss, old_sp - 4); // IP is pushed last
        let return_cs = cpu.read_u16(cpu.ss, old_sp - 2); // CS is pushed first
        assert_eq!(return_ip, 0x0015); // After CALL instruction
        assert_eq!(return_cs, 0x2000);
    }

    #[test]
    fn test_ret_far() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        cpu.ss = 0x1000;
        cpu.sp = 0x00FC;
        cpu.cs = 0x3000;

        // Push return CS and IP onto stack (IP first, then CS)
        cpu.memory.write(
            Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x00FC),
            0x34,
        ); // IP low
        cpu.memory.write(
            Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x00FD),
            0x12,
        ); // IP high
        cpu.memory.write(
            Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x00FE),
            0x00,
        ); // CS low
        cpu.memory.write(
            Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x00FF),
            0x20,
        ); // CS high

        // RET far (0xCB)
        cpu.memory.load_program(0x30000, &[0xCB]);
        cpu.ip = 0x0000;

        cpu.step();

        // CS:IP should be restored
        assert_eq!(cpu.ip, 0x1234);
        assert_eq!(cpu.cs, 0x2000);
        assert_eq!(cpu.sp, 0x0100); // SP restored
    }

    #[test]
    fn test_mov_r8_rm8_register() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // MOV AL, CL (0x8A with ModR/M 0b11_000_001)
        // AL = reg field (000), CL = r/m field (001)
        cpu.memory.load_program(0xFFFF0, &[0x8A, 0b11_000_001]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.cx = 0x0042; // CL = 0x42

        cpu.step();
        assert_eq!(cpu.ax & 0xFF, 0x42);
    }

    #[test]
    fn test_mov_r8_rm8_memory() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        cpu.ds = 0x1000;
        cpu.bx = 0x0100;

        // Write test value to memory at DS:BX
        let addr = Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x0100);
        cpu.memory.write(addr, 0x55);

        // MOV AL, [BX] (0x8A with ModR/M 0b00_000_111)
        cpu.memory.load_program(0xFFFF0, &[0x8A, 0b00_000_111]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;

        cpu.step();
        assert_eq!(cpu.ax & 0xFF, 0x55);
    }

    #[test]
    fn test_mov_rm8_r8_register() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // MOV CL, AL (0x88 with ModR/M 0b11_000_001)
        // AL = reg field (000), CL = r/m field (001)
        cpu.memory.load_program(0xFFFF0, &[0x88, 0b11_000_001]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.ax = 0x0099; // AL = 0x99

        cpu.step();
        assert_eq!(cpu.cx & 0xFF, 0x99);
    }

    #[test]
    fn test_mov_rm8_r8_memory() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        cpu.ds = 0x1000;
        cpu.bx = 0x0100;
        cpu.ax = 0x00AA; // AL = 0xAA

        // MOV [BX], AL (0x88 with ModR/M 0b00_000_111)
        cpu.memory.load_program(0xFFFF0, &[0x88, 0b00_000_111]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;

        cpu.step();

        // Verify memory was written
        let addr = Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x0100);
        assert_eq!(cpu.memory.read(addr), 0xAA);
    }

    #[test]
    fn test_mov_r16_rm16_register() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // MOV AX, CX (0x8B with ModR/M 0b11_000_001)
        cpu.memory.load_program(0xFFFF0, &[0x8B, 0b11_000_001]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.cx = 0x1234;

        cpu.step();
        assert_eq!(cpu.ax, 0x1234);
    }

    #[test]
    fn test_mov_r16_rm16_memory() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        cpu.ds = 0x1000;
        cpu.si = 0x0200;

        // Write test value to memory at DS:SI
        let addr = Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x0200);
        cpu.memory.write(addr, 0x78); // Low byte
        cpu.memory.write(addr + 1, 0x56); // High byte

        // MOV AX, [SI] (0x8B with ModR/M 0b00_000_100)
        cpu.memory.load_program(0xFFFF0, &[0x8B, 0b00_000_100]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;

        cpu.step();
        assert_eq!(cpu.ax, 0x5678);
    }

    #[test]
    fn test_mov_rm16_r16_register() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // MOV CX, AX (0x89 with ModR/M 0b11_000_001)
        cpu.memory.load_program(0xFFFF0, &[0x89, 0b11_000_001]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.ax = 0xABCD;

        cpu.step();
        assert_eq!(cpu.cx, 0xABCD);
    }

    #[test]
    fn test_mov_rm16_r16_memory() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        cpu.ds = 0x1000;
        cpu.di = 0x0300;
        cpu.ax = 0x9876;

        // MOV [DI], AX (0x89 with ModR/M 0b00_000_101)
        cpu.memory.load_program(0xFFFF0, &[0x89, 0b00_000_101]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;

        cpu.step();

        // Verify memory was written (little-endian)
        let addr = Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x0300);
        assert_eq!(cpu.memory.read(addr), 0x76); // Low byte
        assert_eq!(cpu.memory.read(addr + 1), 0x98); // High byte
    }

    #[test]
    fn test_add_rm8_r8_register() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // ADD CL, AL (0x00 with ModR/M 0b11_000_001)
        // AL = reg (000), CL = r/m (001)
        cpu.memory.load_program(0xFFFF0, &[0x00, 0b11_000_001]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.ax = 0x0005; // AL = 5
        cpu.cx = 0x0003; // CL = 3

        cpu.step();
        assert_eq!(cpu.cx & 0xFF, 8); // CL should be 3 + 5 = 8
        assert!(!cpu.get_flag(FLAG_ZF));
        assert!(!cpu.get_flag(FLAG_CF));
    }

    #[test]
    fn test_add_rm16_r16_memory() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        cpu.ds = 0x1000;
        cpu.bx = 0x0100;
        cpu.ax = 0x0020; // AX = 32

        // Write initial value to memory
        let addr = Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x0100);
        cpu.memory.write(addr, 0x10); // Low byte = 16
        cpu.memory.write(addr + 1, 0x00); // High byte = 0

        // ADD [BX], AX (0x01 with ModR/M 0b00_000_111)
        cpu.memory.load_program(0xFFFF0, &[0x01, 0b00_000_111]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;

        cpu.step();

        // Memory should now contain 16 + 32 = 48
        let result = cpu.memory.read(addr) as u16 | ((cpu.memory.read(addr + 1) as u16) << 8);
        assert_eq!(result, 48);
    }

    #[test]
    fn test_add_r8_rm8_register() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // ADD AL, CL (0x02 with ModR/M 0b11_000_001)
        cpu.memory.load_program(0xFFFF0, &[0x02, 0b11_000_001]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.ax = 0x0010; // AL = 16
        cpu.cx = 0x0020; // CL = 32

        cpu.step();
        assert_eq!(cpu.ax & 0xFF, 48); // AL should be 16 + 32 = 48
    }

    #[test]
    fn test_sub_rm8_r8_register() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // SUB CL, AL (0x28 with ModR/M 0b11_000_001)
        cpu.memory.load_program(0xFFFF0, &[0x28, 0b11_000_001]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.ax = 0x0005; // AL = 5
        cpu.cx = 0x000A; // CL = 10

        cpu.step();
        assert_eq!(cpu.cx & 0xFF, 5); // CL should be 10 - 5 = 5
        assert!(!cpu.get_flag(FLAG_ZF));
        assert!(!cpu.get_flag(FLAG_CF));
    }

    #[test]
    fn test_sub_r16_rm16_memory() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        cpu.ds = 0x1000;
        cpu.si = 0x0200;
        cpu.ax = 0x0050; // AX = 80

        // Write value to memory
        let addr = Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x0200);
        cpu.memory.write(addr, 0x1E); // Low byte = 30
        cpu.memory.write(addr + 1, 0x00); // High byte = 0

        // SUB AX, [SI] (0x2B with ModR/M 0b00_000_100)
        cpu.memory.load_program(0xFFFF0, &[0x2B, 0b00_000_100]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;

        cpu.step();
        assert_eq!(cpu.ax, 50); // AX should be 80 - 30 = 50
    }

    #[test]
    fn test_cmp_rm8_r8_sets_zero_flag() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // CMP CL, AL (0x38 with ModR/M 0b11_000_001)
        cpu.memory.load_program(0xFFFF0, &[0x38, 0b11_000_001]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.ax = 0x0042; // AL = 0x42
        cpu.cx = 0x0042; // CL = 0x42

        let old_cx = cpu.cx;
        cpu.step();
        assert_eq!(cpu.cx, old_cx); // CMP doesn't modify operand
        assert!(cpu.get_flag(FLAG_ZF)); // Should set zero flag when equal
    }

    #[test]
    fn test_cmp_r16_rm16_sets_carry_flag() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // CMP AX, CX (0x3B with ModR/M 0b11_000_001)
        cpu.memory.load_program(0xFFFF0, &[0x3B, 0b11_000_001]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.ax = 0x0010; // AX = 16
        cpu.cx = 0x0020; // CX = 32

        cpu.step();
        assert_eq!(cpu.ax, 0x0010); // CMP doesn't modify operand
        assert!(cpu.get_flag(FLAG_CF)); // Should set carry when AX < CX
    }

    #[test]
    fn test_decode_modrm() {
        // Test ModR/M byte decoding
        let (modbits, reg, rm) = Cpu8086::<ArrayMemory>::decode_modrm(0b11_010_001);
        assert_eq!(modbits, 0b11); // Register mode
        assert_eq!(reg, 0b010); // DX
        assert_eq!(rm, 0b001); // CX

        let (modbits, reg, rm) = Cpu8086::<ArrayMemory>::decode_modrm(0b00_101_110);
        assert_eq!(modbits, 0b00); // Memory mode, no displacement
        assert_eq!(reg, 0b101); // BP
        assert_eq!(rm, 0b110); // Direct address

        let (modbits, reg, rm) = Cpu8086::<ArrayMemory>::decode_modrm(0b01_000_100);
        assert_eq!(modbits, 0b01); // Memory mode, 8-bit displacement
        assert_eq!(reg, 0b000); // AX
        assert_eq!(rm, 0b100); // [SI+disp8]
    }

    #[test]
    fn test_effective_address_register_mode() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        cpu.bx = 0x1000;
        cpu.si = 0x0200;

        // Register mode should not calculate addresses
        // We're just testing that the function is callable
        let modbits = 0b11;
        let rm = 0b000;
        let (seg, offset, bytes) = cpu.calc_effective_address(modbits, rm);
        assert_eq!(bytes, 0);
        // In register mode, seg/offset are not used
        let _ = (seg, offset); // Suppress unused warning
    }

    #[test]
    fn test_effective_address_no_displacement() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        cpu.ds = 0x1000;
        cpu.bx = 0x0100;
        cpu.si = 0x0020;

        // mod=00, rm=000: [BX+SI]
        let (seg, offset, bytes) = cpu.calc_effective_address(0b00, 0b000);
        assert_eq!(seg, 0x1000);
        assert_eq!(offset, 0x0120); // BX + SI
        assert_eq!(bytes, 0);

        // mod=00, rm=111: [BX]
        let (seg, offset, bytes) = cpu.calc_effective_address(0b00, 0b111);
        assert_eq!(seg, 0x1000);
        assert_eq!(offset, 0x0100); // BX
        assert_eq!(bytes, 0);
    }

    #[test]
    fn test_effective_address_direct() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        cpu.ds = 0x1000;
        cpu.memory.load_program(0xFFFF0, &[0x34, 0x12]); // 16-bit displacement: 0x1234
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;

        // mod=00, rm=110: Direct address (16-bit displacement)
        let (seg, offset, bytes) = cpu.calc_effective_address(0b00, 0b110);
        assert_eq!(seg, 0x1000);
        assert_eq!(offset, 0x1234);
        assert_eq!(bytes, 2);
    }

    #[test]
    fn test_effective_address_8bit_displacement() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        cpu.ds = 0x1000;
        cpu.si = 0x0100;
        cpu.memory.load_program(0xFFFF0, &[0x10]); // 8-bit displacement: +16
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;

        // mod=01, rm=100: [SI+disp8]
        let (seg, offset, bytes) = cpu.calc_effective_address(0b01, 0b100);
        assert_eq!(seg, 0x1000);
        assert_eq!(offset, 0x0110); // SI + 0x10
        assert_eq!(bytes, 1);
    }

    #[test]
    fn test_effective_address_16bit_displacement() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        cpu.ds = 0x1000;
        cpu.bx = 0x0200;
        cpu.di = 0x0050;
        cpu.memory.load_program(0xFFFF0, &[0x00, 0x10]); // 16-bit displacement: 0x1000
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;

        // mod=10, rm=001: [BX+DI+disp16]
        let (seg, offset, bytes) = cpu.calc_effective_address(0b10, 0b001);
        assert_eq!(seg, 0x1000);
        assert_eq!(offset, 0x1250); // BX + DI + 0x1000
        assert_eq!(bytes, 2);
    }

    #[test]
    fn test_read_write_rm8_register() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // Set AL to 0x42
        cpu.ax = 0x0042;

        // Read AL using ModR/M (mod=11, rm=000 for AL)
        let val = cpu.read_rm8(0b11, 0b000);
        assert_eq!(val, 0x42);

        // Write to CL (mod=11, rm=001 for CL)
        cpu.write_rm8(0b11, 0b001, 0x55);
        assert_eq!(cpu.cx & 0xFF, 0x55);
    }

    #[test]
    fn test_read_write_rm8_memory() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        cpu.ds = 0x1000;
        cpu.bx = 0x0100;

        // Write to memory using ModR/M (mod=00, rm=111 for [BX])
        cpu.write_rm8(0b00, 0b111, 0xAA);

        // Read it back
        let val = cpu.read_rm8(0b00, 0b111);
        assert_eq!(val, 0xAA);

        // Verify it's at the right physical address
        let physical_addr = Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x0100);
        assert_eq!(cpu.memory.read(physical_addr), 0xAA);
    }

    #[test]
    fn test_read_write_rm16_register() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // Set AX to 0x1234
        cpu.ax = 0x1234;

        // Read AX using ModR/M (mod=11, rm=000 for AX)
        let val = cpu.read_rm16(0b11, 0b000);
        assert_eq!(val, 0x1234);

        // Write to CX (mod=11, rm=001 for CX)
        cpu.write_rm16(0b11, 0b001, 0x5678);
        assert_eq!(cpu.cx, 0x5678);
    }

    #[test]
    fn test_read_write_rm16_memory() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        cpu.ds = 0x1000;
        cpu.bx = 0x0100;

        // Write to memory using ModR/M (mod=00, rm=111 for [BX])
        cpu.write_rm16(0b00, 0b111, 0xAABB);

        // Read it back
        let val = cpu.read_rm16(0b00, 0b111);
        assert_eq!(val, 0xAABB);

        // Verify it's at the right physical address (little-endian)
        let physical_addr = Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x0100);
        assert_eq!(cpu.memory.read(physical_addr), 0xBB); // Low byte
        assert_eq!(cpu.memory.read(physical_addr + 1), 0xAA); // High byte
    }

    #[test]
    fn test_cpu_model_default() {
        let mem = ArrayMemory::new();
        let cpu = Cpu8086::new(mem);
        assert_eq!(cpu.model(), CpuModel::Intel8086);
    }

    #[test]
    fn test_cpu_model_with_model() {
        let mem = ArrayMemory::new();
        let cpu = Cpu8086::with_model(mem, CpuModel::Intel80186);
        assert_eq!(cpu.model(), CpuModel::Intel80186);
    }

    #[test]
    fn test_cpu_model_set() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);
        assert_eq!(cpu.model(), CpuModel::Intel8086);

        cpu.set_model(CpuModel::Intel80286);
        assert_eq!(cpu.model(), CpuModel::Intel80286);
    }

    #[test]
    fn test_cpu_model_preserved_on_reset() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80186);

        cpu.ax = 0x1234;
        assert_eq!(cpu.model(), CpuModel::Intel80186);

        cpu.reset();

        assert_eq!(cpu.ax, 0); // Registers reset
        assert_eq!(cpu.model(), CpuModel::Intel80186); // Model preserved
    }

    #[test]
    fn test_cpu_model_feature_flags() {
        assert!(!CpuModel::Intel8086.supports_80186_instructions());
        assert!(!CpuModel::Intel8088.supports_80186_instructions());
        assert!(CpuModel::Intel80186.supports_80186_instructions());
        assert!(CpuModel::Intel80188.supports_80186_instructions());
        assert!(CpuModel::Intel80286.supports_80186_instructions());

        assert!(!CpuModel::Intel8086.supports_80286_instructions());
        assert!(!CpuModel::Intel80186.supports_80286_instructions());
        assert!(CpuModel::Intel80286.supports_80286_instructions());
    }

    #[test]
    fn test_cpu_model_names() {
        assert_eq!(CpuModel::Intel8086.name(), "Intel 8086");
        assert_eq!(CpuModel::Intel8088.name(), "Intel 8088");
        assert_eq!(CpuModel::Intel80186.name(), "Intel 80186");
        assert_eq!(CpuModel::Intel80188.name(), "Intel 80188");
        assert_eq!(CpuModel::Intel80286.name(), "Intel 80286");
    }

    // ===== Multiply/Divide Tests =====

    #[test]
    fn test_mul_8bit() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // MUL CL (0xF6 with ModR/M 0b11_100_001)
        cpu.memory.load_program(0xFFFF0, &[0xF6, 0b11_100_001]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.ax = 0x0005; // AL = 5
        cpu.cx = 0x0006; // CL = 6

        cpu.step();
        assert_eq!(cpu.ax, 30); // 5 * 6 = 30
        assert!(!cpu.get_flag(FLAG_CF)); // High byte is zero
        assert!(!cpu.get_flag(FLAG_OF));
    }

    #[test]
    fn test_mul_8bit_overflow() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // MUL CL
        cpu.memory.load_program(0xFFFF0, &[0xF6, 0b11_100_001]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.ax = 0x0080; // AL = 128
        cpu.cx = 0x0002; // CL = 2

        cpu.step();
        assert_eq!(cpu.ax, 256); // 128 * 2 = 256 (0x0100)
        assert!(cpu.get_flag(FLAG_CF)); // High byte is non-zero
        assert!(cpu.get_flag(FLAG_OF));
    }

    #[test]
    fn test_mul_16bit() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // MUL CX (0xF7 with ModR/M 0b11_100_001)
        cpu.memory.load_program(0xFFFF0, &[0xF7, 0b11_100_001]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.ax = 0x1000; // AX = 4096
        cpu.cx = 0x0010; // CX = 16

        cpu.step();
        assert_eq!(cpu.ax, 0x0000); // Low word of 65536
        assert_eq!(cpu.dx, 0x0001); // High word of 65536
        assert!(cpu.get_flag(FLAG_CF)); // DX is non-zero
        assert!(cpu.get_flag(FLAG_OF));
    }

    #[test]
    fn test_imul_8bit() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // IMUL CL (0xF6 with ModR/M 0b11_101_001)
        cpu.memory.load_program(0xFFFF0, &[0xF6, 0b11_101_001]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.ax = 0x00FB; // AL = -5 (signed)
        cpu.cx = 0x0006; // CL = 6

        cpu.step();
        // -5 * 6 = -30 = 0xFFE2 in 16-bit two's complement
        assert_eq!(cpu.ax, 0xFFE2);
    }

    #[test]
    fn test_div_8bit() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // DIV CL (0xF6 with ModR/M 0b11_110_001)
        cpu.memory.load_program(0xFFFF0, &[0xF6, 0b11_110_001]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.ax = 100; // Dividend
        cpu.cx = 7; // CL = divisor

        cpu.step();
        // 100 / 7 = 14 remainder 2
        // AL = quotient, AH = remainder
        assert_eq!(cpu.ax & 0xFF, 14); // AL = quotient
        assert_eq!((cpu.ax >> 8) & 0xFF, 2); // AH = remainder
    }

    #[test]
    fn test_div_16bit() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // DIV CX (0xF7 with ModR/M 0b11_110_001)
        cpu.memory.load_program(0xFFFF0, &[0xF7, 0b11_110_001]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.dx = 0x0001; // High word of dividend
        cpu.ax = 0x0000; // Low word: 0x10000 = 65536
        cpu.cx = 100; // Divisor

        cpu.step();
        // 65536 / 100 = 655 remainder 36
        assert_eq!(cpu.ax, 655); // Quotient
        assert_eq!(cpu.dx, 36); // Remainder
    }

    #[test]
    fn test_idiv_8bit() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // IDIV CL (0xF6 with ModR/M 0b11_111_001)
        cpu.memory.load_program(0xFFFF0, &[0xF6, 0b11_111_001]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.ax = (-50i16) as u16; // -50 as signed dividend
        cpu.cx = 0x0007; // CL = 7

        cpu.step();
        // -50 / 7 = -7 remainder -1
        assert_eq!((cpu.ax & 0xFF) as i8, -7); // AL = quotient
        assert_eq!(((cpu.ax >> 8) & 0xFF) as i8, -1); // AH = remainder
    }

    // ===== Shift/Rotate Tests =====

    #[test]
    fn test_shl_8bit_by_1() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // SHL AL, 1 (0xD0 with ModR/M 0b11_100_000)
        cpu.memory.load_program(0xFFFF0, &[0xD0, 0b11_100_000]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.ax = 0x0042; // AL = 0x42

        cpu.step();
        assert_eq!(cpu.ax & 0xFF, 0x84); // 0x42 << 1 = 0x84
        assert!(!cpu.get_flag(FLAG_CF)); // No bit shifted out
    }

    #[test]
    fn test_shl_8bit_with_carry() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // SHL AL, 1
        cpu.memory.load_program(0xFFFF0, &[0xD0, 0b11_100_000]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.ax = 0x0080; // AL = 0x80 (bit 7 set)

        cpu.step();
        assert_eq!(cpu.ax & 0xFF, 0x00); // 0x80 << 1 = 0x00 (wraps)
        assert!(cpu.get_flag(FLAG_CF)); // Bit 7 was shifted into CF
    }

    #[test]
    fn test_shr_8bit() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // SHR AL, 1 (0xD0 with ModR/M 0b11_101_000)
        cpu.memory.load_program(0xFFFF0, &[0xD0, 0b11_101_000]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.ax = 0x0042; // AL = 0x42

        cpu.step();
        assert_eq!(cpu.ax & 0xFF, 0x21); // 0x42 >> 1 = 0x21
        assert!(!cpu.get_flag(FLAG_CF));
    }

    #[test]
    fn test_sar_8bit() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // SAR AL, 1 (0xD0 with ModR/M 0b11_111_000)
        cpu.memory.load_program(0xFFFF0, &[0xD0, 0b11_111_000]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.ax = 0x0084; // AL = 0x84 (negative in signed)

        cpu.step();
        assert_eq!(cpu.ax & 0xFF, 0xC2); // Sign bit preserved: 0x84 >> 1 = 0xC2
        assert!(!cpu.get_flag(FLAG_CF));
    }

    #[test]
    fn test_rol_8bit() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // ROL AL, 1 (0xD0 with ModR/M 0b11_000_000)
        cpu.memory.load_program(0xFFFF0, &[0xD0, 0b11_000_000]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.ax = 0x0081; // AL = 0x81

        cpu.step();
        assert_eq!(cpu.ax & 0xFF, 0x03); // 0x81 rotated left = 0x03
        assert!(cpu.get_flag(FLAG_CF)); // Bit 7 rotated into CF
    }

    #[test]
    fn test_ror_8bit() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // ROR AL, 1 (0xD0 with ModR/M 0b11_001_000)
        cpu.memory.load_program(0xFFFF0, &[0xD0, 0b11_001_000]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.ax = 0x0081; // AL = 0x81

        cpu.step();
        assert_eq!(cpu.ax & 0xFF, 0xC0); // 0x81 rotated right = 0xC0
        assert!(cpu.get_flag(FLAG_CF)); // Bit 0 rotated into CF
    }

    #[test]
    fn test_rcl_8bit() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // RCL AL, 1 (0xD0 with ModR/M 0b11_010_000)
        cpu.memory.load_program(0xFFFF0, &[0xD0, 0b11_010_000]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.ax = 0x0081; // AL = 0x81
        cpu.set_flag(FLAG_CF, true); // CF = 1

        cpu.step();
        assert_eq!(cpu.ax & 0xFF, 0x03); // 0x81 << 1 with CF=1 becomes 0x03
        assert!(cpu.get_flag(FLAG_CF)); // Old bit 7 moved to CF
    }

    #[test]
    fn test_rcr_8bit() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // RCR AL, 1 (0xD0 with ModR/M 0b11_011_000)
        cpu.memory.load_program(0xFFFF0, &[0xD0, 0b11_011_000]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.ax = 0x0081; // AL = 0x81
        cpu.set_flag(FLAG_CF, true); // CF = 1

        cpu.step();
        assert_eq!(cpu.ax & 0xFF, 0xC0); // 0x81 >> 1 with CF=1 becomes 0xC0
        assert!(cpu.get_flag(FLAG_CF)); // Old bit 0 moved to CF
    }

    #[test]
    fn test_shl_by_cl() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // SHL AL, CL (0xD2 with ModR/M 0b11_100_000)
        cpu.memory.load_program(0xFFFF0, &[0xD2, 0b11_100_000]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.ax = 0x0002; // AL = 2
        cpu.cx = 0x0003; // CL = 3

        cpu.step();
        assert_eq!(cpu.ax & 0xFF, 16); // 2 << 3 = 16
    }

    #[test]
    fn test_shl_16bit() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // SHL AX, 1 (0xD1 with ModR/M 0b11_100_000)
        cpu.memory.load_program(0xFFFF0, &[0xD1, 0b11_100_000]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.ax = 0x1234;

        cpu.step();
        assert_eq!(cpu.ax, 0x2468); // 0x1234 << 1 = 0x2468
    }

    #[test]
    fn test_ror_16bit() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // ROR AX, 1 (0xD1 with ModR/M 0b11_001_000)
        cpu.memory.load_program(0xFFFF0, &[0xD1, 0b11_001_000]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.ax = 0x8001;

        cpu.step();
        assert_eq!(cpu.ax, 0xC000); // Bit 0 rotates to bit 15
        assert!(cpu.get_flag(FLAG_CF));
    }

    // ===== Segment Register Tests =====

    #[test]
    fn test_mov_seg_to_reg() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // MOV AX, DS (0x8C with ModR/M 0b11_011_000)
        // seg=3 (DS), rm=0 (AX)
        cpu.memory.load_program(0xFFFF0, &[0x8C, 0b11_011_000]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.ds = 0x1234;

        cpu.step();
        assert_eq!(cpu.ax, 0x1234); // AX should now contain DS value
    }

    #[test]
    fn test_mov_reg_to_seg() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // MOV DS, AX (0x8E with ModR/M 0b11_011_000)
        // seg=3 (DS), rm=0 (AX)
        cpu.memory.load_program(0xFFFF0, &[0x8E, 0b11_011_000]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;
        cpu.ax = 0x5678;

        cpu.step();
        assert_eq!(cpu.ds, 0x5678); // DS should now contain AX value
    }

    #[test]
    fn test_mov_seg_to_memory() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        cpu.ds = 0x1000;
        cpu.bx = 0x0100;
        cpu.es = 0x2345; // ES value to store

        // MOV [BX], ES (0x8C with ModR/M 0b00_000_111)
        // seg=0 (ES), rm=7 ([BX])
        cpu.memory.load_program(0xFFFF0, &[0x8C, 0b00_000_111]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;

        cpu.step();

        // Verify ES was written to memory
        let addr = Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x0100);
        let value = cpu.memory.read(addr) as u16 | ((cpu.memory.read(addr + 1) as u16) << 8);
        assert_eq!(value, 0x2345);
    }

    #[test]
    fn test_mov_memory_to_seg() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        cpu.ds = 0x1000;
        cpu.bx = 0x0200;

        // Write test value to memory
        let addr = Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x0200);
        cpu.memory.write(addr, 0xCD); // Low byte
        cpu.memory.write(addr + 1, 0xAB); // High byte

        // MOV SS, [BX] (0x8E with ModR/M 0b00_010_111)
        // seg=2 (SS), rm=7 ([BX])
        cpu.memory.load_program(0xFFFF0, &[0x8E, 0b00_010_111]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;

        cpu.step();
        assert_eq!(cpu.ss, 0xABCD); // SS should contain value from memory
    }

    // ===== String Operation Tests =====

    #[test]
    fn test_movsb() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        cpu.ds = 0x1000;
        cpu.es = 0x2000;
        cpu.si = 0x0100;
        cpu.di = 0x0200;

        // Write source data
        let src_addr = Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x0100);
        cpu.memory.write(src_addr, 0x42);

        // MOVSB (0xA4)
        cpu.memory.load_program(0xFFFF0, &[0xA4]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;

        cpu.step();

        // Verify data copied
        let dst_addr = Cpu8086::<ArrayMemory>::physical_address(0x2000, 0x0200);
        assert_eq!(cpu.memory.read(dst_addr), 0x42);

        // Verify SI and DI incremented (DF=0)
        assert_eq!(cpu.si, 0x0101);
        assert_eq!(cpu.di, 0x0201);
    }

    #[test]
    fn test_movsw() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        cpu.ds = 0x1000;
        cpu.es = 0x2000;
        cpu.si = 0x0100;
        cpu.di = 0x0200;

        // Write source word
        let src_addr = Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x0100);
        cpu.memory.write(src_addr, 0x34);
        cpu.memory.write(src_addr + 1, 0x12);

        // MOVSW (0xA5)
        cpu.memory.load_program(0xFFFF0, &[0xA5]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;

        cpu.step();

        // Verify word copied
        let dst_addr = Cpu8086::<ArrayMemory>::physical_address(0x2000, 0x0200);
        assert_eq!(cpu.memory.read(dst_addr), 0x34);
        assert_eq!(cpu.memory.read(dst_addr + 1), 0x12);

        // Verify SI and DI incremented by 2
        assert_eq!(cpu.si, 0x0102);
        assert_eq!(cpu.di, 0x0202);
    }

    #[test]
    fn test_movsb_with_df() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        cpu.ds = 0x1000;
        cpu.es = 0x2000;
        cpu.si = 0x0100;
        cpu.di = 0x0200;
        cpu.set_flag(FLAG_DF, true); // Set direction flag

        // Write source data
        let src_addr = Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x0100);
        cpu.memory.write(src_addr, 0xAB);

        // MOVSB
        cpu.memory.load_program(0xFFFF0, &[0xA4]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;

        cpu.step();

        // Verify SI and DI decremented (DF=1)
        assert_eq!(cpu.si, 0x00FF);
        assert_eq!(cpu.di, 0x01FF);
    }

    #[test]
    fn test_stosb() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        cpu.es = 0x2000;
        cpu.di = 0x0100;
        cpu.ax = 0x00FF; // AL = 0xFF

        // STOSB (0xAA)
        cpu.memory.load_program(0xFFFF0, &[0xAA]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;

        cpu.step();

        // Verify AL stored to ES:DI
        let addr = Cpu8086::<ArrayMemory>::physical_address(0x2000, 0x0100);
        assert_eq!(cpu.memory.read(addr), 0xFF);
        assert_eq!(cpu.di, 0x0101);
    }

    #[test]
    fn test_stosw() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        cpu.es = 0x2000;
        cpu.di = 0x0100;
        cpu.ax = 0xABCD;

        // STOSW (0xAB)
        cpu.memory.load_program(0xFFFF0, &[0xAB]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;

        cpu.step();

        // Verify AX stored to ES:DI
        let addr = Cpu8086::<ArrayMemory>::physical_address(0x2000, 0x0100);
        assert_eq!(cpu.memory.read(addr), 0xCD);
        assert_eq!(cpu.memory.read(addr + 1), 0xAB);
        assert_eq!(cpu.di, 0x0102);
    }

    #[test]
    fn test_lodsb() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        cpu.ds = 0x1000;
        cpu.si = 0x0100;

        // Write test data
        let addr = Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x0100);
        cpu.memory.write(addr, 0x55);

        // LODSB (0xAC)
        cpu.memory.load_program(0xFFFF0, &[0xAC]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;

        cpu.step();

        // Verify data loaded into AL
        assert_eq!(cpu.ax & 0xFF, 0x55);
        assert_eq!(cpu.si, 0x0101);
    }

    #[test]
    fn test_lodsw() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        cpu.ds = 0x1000;
        cpu.si = 0x0100;

        // Write test word
        let addr = Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x0100);
        cpu.memory.write(addr, 0x78);
        cpu.memory.write(addr + 1, 0x56);

        // LODSW (0xAD)
        cpu.memory.load_program(0xFFFF0, &[0xAD]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;

        cpu.step();

        // Verify word loaded into AX
        assert_eq!(cpu.ax, 0x5678);
        assert_eq!(cpu.si, 0x0102);
    }

    #[test]
    fn test_scasb() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        cpu.es = 0x2000;
        cpu.di = 0x0100;
        cpu.ax = 0x0042; // AL = 0x42

        // Write test data
        let addr = Cpu8086::<ArrayMemory>::physical_address(0x2000, 0x0100);
        cpu.memory.write(addr, 0x42);

        // SCASB (0xAE)
        cpu.memory.load_program(0xFFFF0, &[0xAE]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;

        cpu.step();

        // Verify ZF set (AL == [ES:DI])
        assert!(cpu.get_flag(FLAG_ZF));
        assert_eq!(cpu.di, 0x0101);
    }

    #[test]
    fn test_scasw() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        cpu.es = 0x2000;
        cpu.di = 0x0100;
        cpu.ax = 0x1234;

        // Write different word
        let addr = Cpu8086::<ArrayMemory>::physical_address(0x2000, 0x0100);
        cpu.memory.write(addr, 0x56);
        cpu.memory.write(addr + 1, 0x78);

        // SCASW (0xAF)
        cpu.memory.load_program(0xFFFF0, &[0xAF]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;

        cpu.step();

        // Verify ZF clear (AX != [ES:DI])
        assert!(!cpu.get_flag(FLAG_ZF));
        assert_eq!(cpu.di, 0x0102);
    }

    #[test]
    fn test_cmpsb() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        cpu.ds = 0x1000;
        cpu.es = 0x2000;
        cpu.si = 0x0100;
        cpu.di = 0x0200;

        // Write matching bytes
        let src_addr = Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x0100);
        let dst_addr = Cpu8086::<ArrayMemory>::physical_address(0x2000, 0x0200);
        cpu.memory.write(src_addr, 0x55);
        cpu.memory.write(dst_addr, 0x55);

        // CMPSB (0xA6)
        cpu.memory.load_program(0xFFFF0, &[0xA6]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;

        cpu.step();

        // Verify ZF set (bytes equal)
        assert!(cpu.get_flag(FLAG_ZF));
        assert_eq!(cpu.si, 0x0101);
        assert_eq!(cpu.di, 0x0201);
    }

    #[test]
    fn test_cmpsw() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        cpu.ds = 0x1000;
        cpu.es = 0x2000;
        cpu.si = 0x0100;
        cpu.di = 0x0200;

        // Write different words
        let src_addr = Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x0100);
        let dst_addr = Cpu8086::<ArrayMemory>::physical_address(0x2000, 0x0200);
        cpu.memory.write(src_addr, 0x34);
        cpu.memory.write(src_addr + 1, 0x12);
        cpu.memory.write(dst_addr, 0x78);
        cpu.memory.write(dst_addr + 1, 0x56);

        // CMPSW (0xA7)
        cpu.memory.load_program(0xFFFF0, &[0xA7]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;

        cpu.step();

        // Verify ZF clear (words not equal)
        assert!(!cpu.get_flag(FLAG_ZF));
        assert_eq!(cpu.si, 0x0102);
        assert_eq!(cpu.di, 0x0202);
    }

    #[test]
    fn test_rep_stosb() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        cpu.es = 0x2000;
        cpu.di = 0x0100;
        cpu.ax = 0x00AA; // AL = 0xAA
        cpu.cx = 5; // Repeat 5 times

        // REP STOSB (0xF3 0xAA)
        cpu.memory.load_program(0xFFFF0, &[0xF3, 0xAA]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;

        cpu.step();

        // Verify 5 bytes written
        let addr = Cpu8086::<ArrayMemory>::physical_address(0x2000, 0x0100);
        for i in 0..5 {
            assert_eq!(cpu.memory.read(addr + i), 0xAA);
        }
        assert_eq!(cpu.di, 0x0105);
        assert_eq!(cpu.cx, 0); // CX should be 0
    }

    #[test]
    fn test_rep_movsb() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        cpu.ds = 0x1000;
        cpu.es = 0x2000;
        cpu.si = 0x0100;
        cpu.di = 0x0200;
        cpu.cx = 3;

        // Write source data
        let src_addr = Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x0100);
        cpu.memory.write(src_addr, 0x11);
        cpu.memory.write(src_addr + 1, 0x22);
        cpu.memory.write(src_addr + 2, 0x33);

        // REP MOVSB (0xF3 0xA4)
        cpu.memory.load_program(0xFFFF0, &[0xF3, 0xA4]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;

        cpu.step();

        // Verify all bytes copied
        let dst_addr = Cpu8086::<ArrayMemory>::physical_address(0x2000, 0x0200);
        assert_eq!(cpu.memory.read(dst_addr), 0x11);
        assert_eq!(cpu.memory.read(dst_addr + 1), 0x22);
        assert_eq!(cpu.memory.read(dst_addr + 2), 0x33);
        assert_eq!(cpu.cx, 0);
    }

    #[test]
    fn test_repe_scasb_match() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        cpu.es = 0x2000;
        cpu.di = 0x0100;
        cpu.ax = 0x00FF; // AL = 0xFF
        cpu.cx = 5;

        // Fill memory with 0xFF
        let addr = Cpu8086::<ArrayMemory>::physical_address(0x2000, 0x0100);
        for i in 0..5 {
            cpu.memory.write(addr + i, 0xFF);
        }

        // REPE SCASB (0xF3 0xAE) - scan while equal
        cpu.memory.load_program(0xFFFF0, &[0xF3, 0xAE]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;

        cpu.step();

        // Should scan all 5 bytes and stop when CX=0
        assert_eq!(cpu.cx, 0);
        assert_eq!(cpu.di, 0x0105);
        assert!(cpu.get_flag(FLAG_ZF)); // Last comparison was equal
    }

    #[test]
    fn test_repe_scasb_mismatch() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        cpu.es = 0x2000;
        cpu.di = 0x0100;
        cpu.ax = 0x00FF; // AL = 0xFF
        cpu.cx = 5;

        // Fill first 2 with 0xFF, then different value
        let addr = Cpu8086::<ArrayMemory>::physical_address(0x2000, 0x0100);
        cpu.memory.write(addr, 0xFF);
        cpu.memory.write(addr + 1, 0xFF);
        cpu.memory.write(addr + 2, 0xAA); // Different

        // REPE SCASB - should stop at mismatch
        cpu.memory.load_program(0xFFFF0, &[0xF3, 0xAE]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;

        cpu.step();

        // Should stop after 3 comparisons (2 matches + 1 mismatch)
        assert_eq!(cpu.cx, 2); // 5 - 3 = 2 remaining
        assert_eq!(cpu.di, 0x0103);
        assert!(!cpu.get_flag(FLAG_ZF)); // Last comparison was not equal
    }

    #[test]
    fn test_repne_scasb() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        cpu.es = 0x2000;
        cpu.di = 0x0100;
        cpu.ax = 0x0000; // AL = 0x00 (looking for null)
        cpu.cx = 10;

        // Fill with non-zero, then zero at position 5
        let addr = Cpu8086::<ArrayMemory>::physical_address(0x2000, 0x0100);
        for i in 0..5 {
            cpu.memory.write(addr + i, 0xFF);
        }
        cpu.memory.write(addr + 5, 0x00); // Match at position 5

        // REPNE SCASB (0xF2 0xAE) - scan while not equal
        cpu.memory.load_program(0xFFFF0, &[0xF2, 0xAE]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;

        cpu.step();

        // Should stop when it finds 0x00 at position 5
        assert_eq!(cpu.cx, 4); // 10 - 6 = 4 remaining
        assert_eq!(cpu.di, 0x0106);
        assert!(cpu.get_flag(FLAG_ZF)); // Found match
    }

    // ===== 80186 Instruction Tests =====

    #[test]
    fn test_pusha() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80186);

        cpu.ax = 0x1111;
        cpu.cx = 0x2222;
        cpu.dx = 0x3333;
        cpu.bx = 0x4444;
        cpu.sp = 0x0100;
        cpu.bp = 0x5555;
        cpu.si = 0x6666;
        cpu.di = 0x7777;
        cpu.ss = 0x1000;

        // PUSHA (0x60)
        cpu.memory.load_program(0xFFFF0, &[0x60]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;

        cpu.step();

        // SP should be decremented by 16 (8 words)
        assert_eq!(cpu.sp, 0x00F0);

        // Check values on stack
        let base = physical_address(0x1000, 0x00F0);
        assert_eq!(cpu.memory.read_u16(base), 0x7777); // DI
        assert_eq!(cpu.memory.read_u16(base + 2), 0x6666); // SI
        assert_eq!(cpu.memory.read_u16(base + 4), 0x5555); // BP
        assert_eq!(cpu.memory.read_u16(base + 6), 0x0100); // Original SP
        assert_eq!(cpu.memory.read_u16(base + 8), 0x4444); // BX
        assert_eq!(cpu.memory.read_u16(base + 10), 0x3333); // DX
        assert_eq!(cpu.memory.read_u16(base + 12), 0x2222); // CX
        assert_eq!(cpu.memory.read_u16(base + 14), 0x1111); // AX
    }

    #[test]
    fn test_popa() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80186);

        cpu.sp = 0x00F0;
        cpu.ss = 0x1000;

        // Set up stack with test values
        let base = physical_address(0x1000, 0x00F0);
        cpu.memory.write_u16(base, 0x7777); // DI
        cpu.memory.write_u16(base + 2, 0x6666); // SI
        cpu.memory.write_u16(base + 4, 0x5555); // BP
        cpu.memory.write_u16(base + 6, 0x9999); // SP (discarded)
        cpu.memory.write_u16(base + 8, 0x4444); // BX
        cpu.memory.write_u16(base + 10, 0x3333); // DX
        cpu.memory.write_u16(base + 12, 0x2222); // CX
        cpu.memory.write_u16(base + 14, 0x1111); // AX

        // POPA (0x61)
        cpu.memory.load_program(0xFFFF0, &[0x61]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;

        cpu.step();

        // Check registers
        assert_eq!(cpu.ax, 0x1111);
        assert_eq!(cpu.cx, 0x2222);
        assert_eq!(cpu.dx, 0x3333);
        assert_eq!(cpu.bx, 0x4444);
        assert_eq!(cpu.bp, 0x5555);
        assert_eq!(cpu.si, 0x6666);
        assert_eq!(cpu.di, 0x7777);
        // SP should be incremented by 16
        assert_eq!(cpu.sp, 0x0100);
    }

    #[test]
    fn test_push_immediate_word() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80186);

        cpu.sp = 0x0100;
        cpu.ss = 0x1000;

        // PUSH imm16 (0x68) - Push 0x1234
        cpu.memory.load_program(0xFFFF0, &[0x68, 0x34, 0x12]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;

        cpu.step();

        // SP should be decremented by 2
        assert_eq!(cpu.sp, 0x00FE);

        // Check value on stack
        let val = cpu.memory.read_u16(physical_address(0x1000, 0x00FE));
        assert_eq!(val, 0x1234);
    }

    #[test]
    fn test_push_immediate_byte() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80186);

        cpu.sp = 0x0100;
        cpu.ss = 0x1000;

        // PUSH imm8 (0x6A) - Push 0x7F (positive, sign extends to 0x007F)
        cpu.memory.load_program(0xFFFF0, &[0x6A, 0x7F]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;

        cpu.step();

        // Check value on stack (should be sign-extended)
        let val = cpu.memory.read_u16(physical_address(0x1000, 0x00FE));
        assert_eq!(val, 0x007F);

        // Test with negative value (0xFF should sign extend to 0xFFFF)
        cpu.sp = 0x0100;
        cpu.memory.load_program(0xFFFF0, &[0x6A, 0xFF]);
        cpu.ip = 0x0000;

        cpu.step();

        let val = cpu.memory.read_u16(physical_address(0x1000, 0x00FE));
        assert_eq!(val, 0xFFFF);
    }

    #[test]
    fn test_imul_immediate_word() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80186);

        cpu.bx = 10;

        // IMUL AX, BX, 20 (0x69 ModRM imm16) - AX = BX * 20
        cpu.memory.load_program(0xFFFF0, &[0x69, 0xC3, 0x14, 0x00]); // ModRM=0xC3 (AX, BX)
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;

        cpu.step();

        // AX should be 10 * 20 = 200
        assert_eq!(cpu.ax, 200);
        // No overflow for this multiplication
        assert!(!cpu.get_flag(FLAG_CF));
        assert!(!cpu.get_flag(FLAG_OF));
    }

    #[test]
    fn test_imul_immediate_byte() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80186);

        cpu.dx = 5;

        // IMUL AX, DX, 7 (0x6B ModRM imm8) - AX = DX * 7
        cpu.memory.load_program(0xFFFF0, &[0x6B, 0xC2, 0x07]); // ModRM=0xC2 (AX, DX)
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;

        cpu.step();

        // AX should be 5 * 7 = 35
        assert_eq!(cpu.ax, 35);
        // No overflow
        assert!(!cpu.get_flag(FLAG_CF));
        assert!(!cpu.get_flag(FLAG_OF));
    }

    #[test]
    fn test_bound_in_range() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80186);

        cpu.ax = 50; // Index to test
        cpu.ds = 0x1000;

        // Set up bounds in memory at DS:0x0100
        // Lower bound: 10, Upper bound: 100
        let addr = physical_address(0x1000, 0x0100);
        cpu.memory.write_u16(addr, 10); // Lower bound
        cpu.memory.write_u16(addr + 2, 100); // Upper bound

        // BOUND AX, [0x0100] (0x62 ModRM disp16)
        cpu.memory.load_program(0xFFFF0, &[0x62, 0x06, 0x00, 0x01]); // ModRM=0x06 (direct addr)
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;

        let old_ip = cpu.ip;
        cpu.step();

        // Should not trigger interrupt, IP should advance
        assert_ne!(cpu.ip, old_ip);
    }

    #[test]
    fn test_enter_leave() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80186);

        cpu.sp = 0x0100;
        cpu.bp = 0x5555;
        cpu.ss = 0x1000;

        // ENTER 16, 0 (0xC8 size_low size_high nesting)
        cpu.memory.load_program(0xFFFF0, &[0xC8, 0x10, 0x00, 0x00]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;

        cpu.step();

        // BP should be saved and set to old SP - 2
        let expected_bp = 0x00FE;
        assert_eq!(cpu.bp, expected_bp);
        // SP should be decremented by 2 (push BP) + 16 (local space)
        assert_eq!(cpu.sp, 0x00EE);

        // Now test LEAVE (0xC9)
        cpu.memory.load_program(0xFFFF0, &[0xC9]);
        cpu.ip = 0x0000;

        cpu.step();

        // SP should be restored to BP + 2 (after popping BP)
        assert_eq!(cpu.sp, 0x0100);
        // BP should be popped (restored to 0x5555)
        assert_eq!(cpu.bp, 0x5555);
    }

    #[test]
    fn test_ins_outs_byte() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80186);

        cpu.dx = 0x60; // Port
        cpu.es = 0x1000;
        cpu.di = 0x0100;

        // INSB (0x6C) - Input from port DX to ES:DI
        cpu.memory.load_program(0xFFFF0, &[0x6C]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;

        cpu.step();

        // DI should be incremented
        assert_eq!(cpu.di, 0x0101);
        // Value should be written (0xFF from stub I/O)
        let val = cpu.memory.read(physical_address(0x1000, 0x0100));
        assert_eq!(val, 0xFF);

        // Test OUTSB (0x6E)
        cpu.ds = 0x1000;
        cpu.si = 0x0200;
        cpu.memory.write(physical_address(0x1000, 0x0200), 0x42);

        cpu.memory.load_program(0xFFFF0, &[0x6E]);
        cpu.ip = 0x0000;

        cpu.step();

        // SI should be incremented
        assert_eq!(cpu.si, 0x0201);
    }

    #[test]
    fn test_ins_outs_word() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80186);

        cpu.dx = 0x60; // Port
        cpu.es = 0x1000;
        cpu.di = 0x0100;

        // INSW (0x6D) - Input word from port DX to ES:DI
        cpu.memory.load_program(0xFFFF0, &[0x6D]);
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;

        cpu.step();

        // DI should be incremented by 2
        assert_eq!(cpu.di, 0x0102);
        // Value should be written (0xFFFF from stub I/O)
        let val = cpu.memory.read_u16(physical_address(0x1000, 0x0100));
        assert_eq!(val, 0xFFFF);

        // Test OUTSW (0x6F)
        cpu.ds = 0x1000;
        cpu.si = 0x0200;
        cpu.memory
            .write_u16(physical_address(0x1000, 0x0200), 0x1234);

        cpu.memory.load_program(0xFFFF0, &[0x6F]);
        cpu.ip = 0x0000;

        cpu.step();

        // SI should be incremented by 2
        assert_eq!(cpu.si, 0x0202);
    }

    // ===== 80286/80386 Instruction Tests =====

    #[test]
    fn test_movsx() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

        cpu.bx = 0x00FF; // Set BL to 0xFF (negative byte)

        // MOVSX AX, BL (0x0F 0xBE ModRM)
        cpu.memory.load_program(0xFFFF0, &[0x0F, 0xBE, 0xC3]); // ModRM=0xC3 (AX, BX)
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;

        cpu.step();

        // 0xFF sign-extended to 16-bit should be 0xFFFF
        assert_eq!(cpu.ax, 0xFFFF);

        // Test with positive value
        cpu.bx = 0x007F; // Set BL to 0x7F (positive byte)
        cpu.memory.load_program(0xFFFF0, &[0x0F, 0xBE, 0xC3]);
        cpu.ip = 0x0000;

        cpu.step();

        // 0x7F sign-extended to 16-bit should be 0x007F
        assert_eq!(cpu.ax, 0x007F);
    }

    #[test]
    fn test_movzx() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

        cpu.bx = 0xFFFF; // Set BL to 0xFF

        // MOVZX AX, BL (0x0F 0xB6 ModRM)
        cpu.memory.load_program(0xFFFF0, &[0x0F, 0xB6, 0xC3]); // ModRM=0xC3 (AX, BX)
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;

        cpu.step();

        // 0xFF zero-extended to 16-bit should be 0x00FF
        assert_eq!(cpu.ax, 0x00FF);
    }

    #[test]
    fn test_bsf() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

        cpu.bx = 0x0018; // Binary: 0000 0000 0001 1000

        // BSF AX, BX (0x0F 0xBC ModRM) - Find first set bit
        cpu.memory.load_program(0xFFFF0, &[0x0F, 0xBC, 0xC3]); // ModRM=0xC3 (AX, BX)
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;

        cpu.step();

        // First set bit from LSB is at position 3
        assert_eq!(cpu.ax, 3);
        assert!(!cpu.get_flag(FLAG_ZF));

        // Test with zero
        cpu.bx = 0;
        cpu.memory.load_program(0xFFFF0, &[0x0F, 0xBC, 0xC3]);
        cpu.ip = 0x0000;

        cpu.step();

        // ZF should be set for zero
        assert!(cpu.get_flag(FLAG_ZF));
    }

    #[test]
    fn test_bsr() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

        cpu.bx = 0x0018; // Binary: 0000 0000 0001 1000

        // BSR AX, BX (0x0F 0xBD ModRM) - Find last set bit
        cpu.memory.load_program(0xFFFF0, &[0x0F, 0xBD, 0xC3]); // ModRM=0xC3 (AX, BX)
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;

        cpu.step();

        // First set bit from MSB is at position 4
        assert_eq!(cpu.ax, 4);
        assert!(!cpu.get_flag(FLAG_ZF));
    }

    #[test]
    fn test_bt() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

        cpu.ax = 3; // Bit index
        cpu.bx = 0x0008; // Binary: 0000 0000 0000 1000 (bit 3 set)

        // BT BX, AX (0x0F 0xA3 ModRM) - Test bit
        cpu.memory.load_program(0xFFFF0, &[0x0F, 0xA3, 0xC3]); // ModRM=0xC3 (BX, AX)
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;

        cpu.step();

        // Bit 3 is set, so CF should be set
        assert!(cpu.get_flag(FLAG_CF));

        // Test with bit not set
        cpu.ax = 5; // Bit index
        cpu.memory.load_program(0xFFFF0, &[0x0F, 0xA3, 0xC3]);
        cpu.ip = 0x0000;

        cpu.step();

        // Bit 5 is not set, so CF should be clear
        assert!(!cpu.get_flag(FLAG_CF));
    }

    #[test]
    fn test_bts() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

        cpu.ax = 5; // Bit index
        cpu.bx = 0x0008; // Binary: 0000 0000 0000 1000

        // BTS BX, AX (0x0F 0xAB ModRM) - Test and set bit
        cpu.memory.load_program(0xFFFF0, &[0x0F, 0xAB, 0xC3]); // ModRM=0xC3 (BX, AX)
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;

        cpu.step();

        // Bit 5 was not set, so CF should be clear
        assert!(!cpu.get_flag(FLAG_CF));
        // Bit 5 should now be set: 0x0008 | 0x0020 = 0x0028
        assert_eq!(cpu.bx, 0x0028);
    }

    #[test]
    fn test_btr() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

        cpu.ax = 3; // Bit index
        cpu.bx = 0x0028; // Binary: 0000 0000 0010 1000

        // BTR BX, AX (0x0F 0xB3 ModRM) - Test and reset bit
        cpu.memory.load_program(0xFFFF0, &[0x0F, 0xB3, 0xC3]); // ModRM=0xC3 (BX, AX)
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;

        cpu.step();

        // Bit 3 was set, so CF should be set
        assert!(cpu.get_flag(FLAG_CF));
        // Bit 3 should now be clear: 0x0028 & ~0x0008 = 0x0020
        assert_eq!(cpu.bx, 0x0020);
    }

    #[test]
    fn test_btc() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

        cpu.ax = 3; // Bit index
        cpu.bx = 0x0008; // Binary: 0000 0000 0000 1000

        // BTC BX, AX (0x0F 0xBB ModRM) - Test and complement bit
        cpu.memory.load_program(0xFFFF0, &[0x0F, 0xBB, 0xC3]); // ModRM=0xC3 (BX, AX)
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;

        cpu.step();

        // Bit 3 was set, so CF should be set
        assert!(cpu.get_flag(FLAG_CF));
        // Bit 3 should now be clear: 0x0008 ^ 0x0008 = 0x0000
        assert_eq!(cpu.bx, 0x0000);

        // Test complement again (from 0 to 1)
        cpu.memory.load_program(0xFFFF0, &[0x0F, 0xBB, 0xC3]);
        cpu.ip = 0x0000;

        cpu.step();

        // Bit 3 was clear, so CF should be clear
        assert!(!cpu.get_flag(FLAG_CF));
        // Bit 3 should now be set: 0x0000 ^ 0x0008 = 0x0008
        assert_eq!(cpu.bx, 0x0008);
    }

    #[test]
    fn test_setcc() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

        // Set ZF flag
        cpu.flags = FLAG_ZF;

        // SETE BL (0x0F 0x94 ModRM) - Set if equal/zero
        cpu.memory.load_program(0xFFFF0, &[0x0F, 0x94, 0xC3]); // ModRM=0xC3 (BL)
        cpu.ip = 0x0000;
        cpu.cs = 0xFFFF;

        cpu.step();

        // ZF is set, so BL should be 1
        assert_eq!(cpu.bx & 0xFF, 1);

        // Clear ZF flag
        cpu.flags = 0;

        // SETE BL again
        cpu.memory.load_program(0xFFFF0, &[0x0F, 0x94, 0xC3]);
        cpu.ip = 0x0000;

        cpu.step();

        // ZF is clear, so BL should be 0
        assert_eq!(cpu.bx & 0xFF, 0);
    }

    #[test]
    fn test_cpu_model_80386() {
        let mem = ArrayMemory::new();
        let cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);
        assert_eq!(cpu.model(), CpuModel::Intel80386);
        assert_eq!(CpuModel::Intel80386.name(), "Intel 80386");
        assert!(CpuModel::Intel80386.supports_80186_instructions());
        assert!(CpuModel::Intel80386.supports_80286_instructions());
        assert!(CpuModel::Intel80386.supports_80386_instructions());
    }

    /// Regression test for RMW (Read-Modify-Write) displacement bug
    ///
    /// This test ensures that instructions which read from and write to memory
    /// (like ADD [BP+disp], AX) don't fetch the displacement bytes twice.
    ///
    /// The bug was: read_rm16() would fetch displacement, then write_rm16() would
    /// fetch it again, causing IP to advance by extra bytes and execute misaligned code.
    ///
    /// Fix: Use read_rmw16/write_rmw16 helpers that cache the effective address.
    #[test]
    fn test_rmw_displacement_not_fetched_twice_add() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        // Set up: BP=0x7C00, SP=0x7B00, value at [BP-0x10]=0x1234
        cpu.bp = 0x7C00;
        cpu.sp = 0x7B00;
        cpu.ss = 0x0000;
        cpu.ds = 0x0000;
        cpu.ax = 0x0100; // Value to add

        // Write test value at BP-0x10 = 0x7BF0
        cpu.memory.write(0x7BF0, 0x34);
        cpu.memory.write(0x7BF1, 0x12);

        // Instruction: ADD [BP-0x10], AX at 0x0000:0x0100
        // Encoding: 01 86 F0 FF
        // - 0x01: ADD r/m16, r16
        // - 0x86: ModR/M byte (mod=10, reg=000 (AX), rm=110 (BP+disp16))
        // - 0xF0 0xFF: Displacement -0x10 (two's complement of 16)
        cpu.cs = 0x0000;
        cpu.ip = 0x0100;
        cpu.memory.write(0x0100, 0x01); // ADD r/m16, r16
        cpu.memory.write(0x0101, 0x86); // ModR/M: mod=10, reg=000, rm=110
        cpu.memory.write(0x0102, 0xF0); // disp16 low byte
        cpu.memory.write(0x0103, 0xFF); // disp16 high byte

        // Execute the instruction
        cpu.step();

        // IP should advance by exactly 4 bytes (opcode + modrm + disp16)
        assert_eq!(cpu.ip, 0x0104, "IP should advance by 4 bytes, not more");

        // Memory at BP-0x10 should be 0x1234 + 0x0100 = 0x1334
        let result_lo = cpu.memory.read(0x7BF0);
        let result_hi = cpu.memory.read(0x7BF1);
        let result = (result_hi as u16) << 8 | result_lo as u16;
        assert_eq!(result, 0x1334, "ADD result should be correct");
    }

    #[test]
    fn test_rmw_displacement_not_fetched_twice_or() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        cpu.bp = 0x1000;
        cpu.ds = 0x0000;
        cpu.cs = 0x0000;
        cpu.ax = 0x00FF;

        cpu.memory.write(0x0FF0, 0xF0); // Value at BP-0x10
        cpu.memory.write(0x0FF1, 0x0F);

        // OR [BP-0x10], AX
        cpu.ip = 0x0200;
        cpu.memory.write(0x0200, 0x09); // OR r/m16, r16
        cpu.memory.write(0x0201, 0x86); // ModR/M
        cpu.memory.write(0x0202, 0xF0); // disp16 low
        cpu.memory.write(0x0203, 0xFF); // disp16 high

        cpu.step();

        assert_eq!(cpu.ip, 0x0204, "IP should advance by exactly 4 bytes");

        let result = (cpu.memory.read(0x0FF1) as u16) << 8 | cpu.memory.read(0x0FF0) as u16;
        assert_eq!(result, 0x0FFF, "OR result should be correct");
    }

    #[test]
    fn test_rmw_displacement_not_fetched_twice_and() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        cpu.bp = 0x2000;
        cpu.ds = 0x0000;
        cpu.cs = 0x0000;
        cpu.ax = 0xFF00;

        cpu.memory.write(0x1FE0, 0xFF); // Value at BP-0x20
        cpu.memory.write(0x1FE1, 0x0F);

        // AND [BP-0x20], AX
        cpu.ip = 0x0300;
        cpu.memory.write(0x0300, 0x21); // AND r/m16, r16
        cpu.memory.write(0x0301, 0x86); // ModR/M
        cpu.memory.write(0x0302, 0xE0); // disp16 low
        cpu.memory.write(0x0303, 0xFF); // disp16 high

        cpu.step();

        assert_eq!(cpu.ip, 0x0304, "IP should advance by exactly 4 bytes");

        let result = (cpu.memory.read(0x1FE1) as u16) << 8 | cpu.memory.read(0x1FE0) as u16;
        assert_eq!(result, 0x0F00, "AND result should be correct");
    }

    #[test]
    fn test_rmw_displacement_not_fetched_twice_sub() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        cpu.bp = 0x3000;
        cpu.ds = 0x0000;
        cpu.cs = 0x0000;
        cpu.ax = 0x0001;

        cpu.memory.write(0x2FF0, 0x00); // Value at BP-0x10 = 0x1000
        cpu.memory.write(0x2FF1, 0x10);

        // SUB [BP-0x10], AX
        cpu.ip = 0x0400;
        cpu.memory.write(0x0400, 0x29); // SUB r/m16, r16
        cpu.memory.write(0x0401, 0x86); // ModR/M
        cpu.memory.write(0x0402, 0xF0); // disp16 low
        cpu.memory.write(0x0403, 0xFF); // disp16 high

        cpu.step();

        assert_eq!(cpu.ip, 0x0404, "IP should advance by exactly 4 bytes");

        let result = (cpu.memory.read(0x2FF1) as u16) << 8 | cpu.memory.read(0x2FF0) as u16;
        assert_eq!(result, 0x0FFF, "SUB result should be correct");
    }

    #[test]
    fn test_rmw_displacement_not_fetched_twice_xor() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        cpu.bp = 0x4000;
        cpu.ds = 0x0000;
        cpu.cs = 0x0000;
        cpu.ax = 0x5555;

        cpu.memory.write(0x3FE0, 0xAA); // Value at BP-0x20 = 0xAAAA
        cpu.memory.write(0x3FE1, 0xAA);

        // XOR [BP-0x20], AX
        cpu.ip = 0x0500;
        cpu.memory.write(0x0500, 0x31); // XOR r/m16, r16
        cpu.memory.write(0x0501, 0x86); // ModR/M
        cpu.memory.write(0x0502, 0xE0); // disp16 low
        cpu.memory.write(0x0503, 0xFF); // disp16 high

        cpu.step();

        assert_eq!(cpu.ip, 0x0504, "IP should advance by exactly 4 bytes");

        let result = (cpu.memory.read(0x3FE1) as u16) << 8 | cpu.memory.read(0x3FE0) as u16;
        assert_eq!(result, 0xFFFF, "XOR result should be correct");
    }

    #[test]
    fn test_rmw_displacement_not_fetched_twice_adc() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        cpu.bp = 0x5000;
        cpu.ds = 0x0000;
        cpu.cs = 0x0000;
        cpu.ax = 0x0001;
        cpu.set_flag(FLAG_CF, true); // Set carry flag

        cpu.memory.write(0x4FF0, 0xFF); // Value at BP-0x10 = 0x00FF
        cpu.memory.write(0x4FF1, 0x00);

        // ADC [BP-0x10], AX
        cpu.ip = 0x0600;
        cpu.memory.write(0x0600, 0x11); // ADC r/m16, r16
        cpu.memory.write(0x0601, 0x86); // ModR/M
        cpu.memory.write(0x0602, 0xF0); // disp16 low
        cpu.memory.write(0x0603, 0xFF); // disp16 high

        cpu.step();

        assert_eq!(cpu.ip, 0x0604, "IP should advance by exactly 4 bytes");

        let result = (cpu.memory.read(0x4FF1) as u16) << 8 | cpu.memory.read(0x4FF0) as u16;
        assert_eq!(result, 0x0101, "ADC result should include carry");
    }

    #[test]
    fn test_rmw_displacement_not_fetched_twice_sbb() {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::new(mem);

        cpu.bp = 0x6000;
        cpu.ds = 0x0000;
        cpu.cs = 0x0000;
        cpu.ax = 0x0001;
        cpu.set_flag(FLAG_CF, true); // Set borrow flag

        cpu.memory.write(0x5FF0, 0x00); // Value at BP-0x10 = 0x0100
        cpu.memory.write(0x5FF1, 0x01);

        // SBB [BP-0x10], AX
        cpu.ip = 0x0700;
        cpu.memory.write(0x0700, 0x19); // SBB r/m16, r16
        cpu.memory.write(0x0701, 0x86); // ModR/M
        cpu.memory.write(0x0702, 0xF0); // disp16 low
        cpu.memory.write(0x0703, 0xFF); // disp16 high

        cpu.step();

        assert_eq!(cpu.ip, 0x0704, "IP should advance by exactly 4 bytes");

        let result = (cpu.memory.read(0x5FF1) as u16) << 8 | cpu.memory.read(0x5FF0) as u16;
        assert_eq!(result, 0x00FE, "SBB result should include borrow");
    }

    #[test]
    fn test_division_error_fault_saves_correct_ip() {
        // This test verifies that when a DIV/IDIV instruction causes a fault
        // (divide by zero or overflow), the saved IP points to the DIV instruction
        // itself, not the next instruction. This allows the fault handler to either
        // fix the problem and retry, or abort the program.

        let mut mem = ArrayMemory::new();

        // Set up INT 0 vector to point to a handler that does IRET
        // IVT entry for INT 0 at 0x0000 (offset) and 0x0002 (segment)
        mem.write(0x00000, 0x00); // Offset low
        mem.write(0x00001, 0x10); // Offset high (handler at 0x1000)
        mem.write(0x00002, 0x00); // Segment low
        mem.write(0x00003, 0x00); // Segment high (CS = 0x0000)

        // Write INT 0 handler at 0x0000:0x1000 - just IRET (0xCF)
        mem.write(0x01000, 0xCF); // IRET

        // Write test code at 0x0000:0x7C00
        let code_addr = 0x07C00;

        // MOV AX, 0x0001
        mem.write(code_addr, 0xB8);
        mem.write(code_addr + 1, 0x01);
        mem.write(code_addr + 2, 0x00);

        // MOV CL, 0x00  (divisor = 0)
        mem.write(code_addr + 3, 0xB1);
        mem.write(code_addr + 4, 0x00);

        // DIV CL  (divide AX by CL, should fault)
        mem.write(code_addr + 5, 0xF6);
        mem.write(code_addr + 6, 0xF1); // ModR/M: mod=11, reg=110 (DIV), rm=001 (CL)

        // NOP (should NOT reach here if fault loops)
        mem.write(code_addr + 7, 0x90);

        // HLT
        mem.write(code_addr + 8, 0xF4);

        // Create CPU
        let mut cpu = Cpu8086::new(mem);
        cpu.cs = 0x0000;
        cpu.ip = 0x7C00;

        // Step through code
        cpu.step(); // MOV AX, 1
        assert_eq!(cpu.ax, 0x0001);
        assert_eq!(cpu.ip, 0x7C03);

        cpu.step(); // MOV CL, 0
        assert_eq!((cpu.cx & 0xFF) as u8, 0x00);
        assert_eq!(cpu.ip, 0x7C05);

        // This should trigger INT 0 fault
        cpu.step(); // DIV CL - this triggers the fault and jumps to INT 0 handler

        // CPU should now be at the INT 0 handler (0x0000:0x1000)
        assert_eq!(cpu.ip, 0x1000, "IP should be at INT 0 handler");
        assert_eq!(cpu.cs, 0x0000);

        // Execute the IRET in the handler
        cpu.step(); // IRET

        // After INT 0 handler (IRET), IP should return to the DIV instruction
        // NOT to the next instruction (NOP)
        assert_eq!(
            cpu.ip, 0x7C05,
            "IP should point back to DIV instruction after fault"
        );
        assert_eq!(cpu.cs, 0x0000);

        // If we step again, it should fault again (infinite loop unless we fix the divisor)
        // But let's fix the divisor first
        cpu.cx = 0x0002; // Set CL = 2

        // Now DIV should succeed
        cpu.step(); // DIV CL
        assert_eq!(cpu.ax & 0xFF, 0x00); // AL = quotient = 1/2 = 0
        assert_eq!((cpu.ax >> 8) & 0xFF, 0x01); // AH = remainder = 1%2 = 1
        assert_eq!(cpu.ip, 0x7C07); // Should now be at NOP
    }
}
