//! Intel 80286 Protected Mode Support
//!
//! This module implements protected mode features for the 80286 CPU.
//! Protected mode is only activated when the CPU model is Intel80286 or later.
//!
//! Key features:
//! - Machine Status Word (MSW) / Control Register 0 (CR0) with PE bit
//! - Global Descriptor Table (GDT) and Interrupt Descriptor Table (IDT)
//! - Local Descriptor Table (LDT) support
//! - Task State Segment (TSS) for task switching
//! - Segment descriptors with base, limit, and access rights
//! - Privilege levels (Ring 0-3)
//! - Protected mode instructions (LGDT, LIDT, LLDT, LTR, LAR, LSL, VERR, VERW)

use serde::{Deserialize, Serialize};

/// 80286 Protected Mode State
///
/// This structure contains all the protected mode specific state for the 80286.
/// It is only used when the CPU is in 80286 or 80386 mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtectedModeState {
    /// Machine Status Word (MSW) / Control Register 0 (CR0)
    /// Bit 0 (PE): Protection Enable - 0=Real Mode, 1=Protected Mode
    /// Bit 1 (MP): Monitor Coprocessor
    /// Bit 2 (EM): Emulation - 1=No coprocessor present
    /// Bit 3 (TS): Task Switched
    pub msw: u16,

    /// Global Descriptor Table Register (GDTR)
    pub gdtr: DescriptorTableRegister,

    /// Interrupt Descriptor Table Register (IDTR)
    pub idtr: DescriptorTableRegister,

    /// Local Descriptor Table Register (LDTR)
    pub ldtr: u16,

    /// Task Register (TR)
    pub tr: u16,
}

/// Descriptor Table Register (for GDTR/IDTR)
///
/// Contains the base address and limit of a descriptor table.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct DescriptorTableRegister {
    /// Base address of the descriptor table (24-bit on 80286, 32-bit on 80386)
    pub base: u32,

    /// Limit of the descriptor table (16-bit, size in bytes - 1)
    pub limit: u16,
}

/// Segment Descriptor (8 bytes)
///
/// Format:
/// - Bytes 0-1: Segment Limit (bits 0-15)
/// - Bytes 2-3: Base Address (bits 0-15)
/// - Byte 4: Base Address (bits 16-23)
/// - Byte 5: Access Rights
/// - Byte 6: Limit (bits 16-19) + Flags
/// - Byte 7: Base Address (bits 24-31) [80386 only]
#[derive(Debug, Clone, Copy)]
pub struct SegmentDescriptor {
    /// Base address of the segment (24-bit on 80286)
    pub base: u32,

    /// Segment limit (size - 1)
    pub limit: u32,

    /// Access rights byte
    pub access: u8,

    /// Flags (granularity, 32-bit, etc.) [80386 only]
    pub flags: u8,
}

impl ProtectedModeState {
    /// Create a new protected mode state (starts in real mode)
    pub fn new() -> Self {
        Self {
            msw: 0, // PE=0, starts in real mode
            gdtr: DescriptorTableRegister { base: 0, limit: 0 },
            idtr: DescriptorTableRegister { base: 0, limit: 0 },
            ldtr: 0,
            tr: 0,
        }
    }

    /// Check if the CPU is in protected mode
    #[inline]
    pub fn is_protected_mode(&self) -> bool {
        (self.msw & 0x0001) != 0 // PE bit
    }

    /// Enable protected mode
    #[inline]
    pub fn enable_protected_mode(&mut self) {
        self.msw |= 0x0001; // Set PE bit
    }

    /// Disable protected mode (only possible on 80286 via reset, ignored on 80386)
    #[inline]
    pub fn disable_protected_mode(&mut self) {
        // On real 80286, once in protected mode, can only return to real mode via reset
        // For emulation purposes, we allow this for testing
        self.msw &= !0x0001; // Clear PE bit
    }

    /// Set the Machine Status Word (MSW/CR0)
    #[inline]
    pub fn set_msw(&mut self, value: u16) {
        // Bits 4-15 are reserved and should be preserved
        self.msw = (value & 0x000F) | (self.msw & 0xFFF0);
    }

    /// Get the Machine Status Word (MSW/CR0)
    #[inline]
    pub fn get_msw(&self) -> u16 {
        self.msw
    }

    /// Get CR0 (Control Register 0) - alias for get_msw for 80386+
    #[inline]
    pub fn get_cr0(&self) -> u16 {
        self.msw
    }

    /// Set CR0 (Control Register 0) - alias for set_msw for 80386+
    #[inline]
    pub fn set_cr0(&mut self, value: u16) {
        self.set_msw(value);
    }

    /// Load the Global Descriptor Table Register
    pub fn load_gdtr(&mut self, base: u32, limit: u16) {
        self.gdtr.base = base & 0x00FFFFFF; // 24-bit on 80286
        self.gdtr.limit = limit;
    }

    /// Load the Interrupt Descriptor Table Register
    pub fn load_idtr(&mut self, base: u32, limit: u16) {
        self.idtr.base = base & 0x00FFFFFF; // 24-bit on 80286
        self.idtr.limit = limit;
    }

    /// Load the Local Descriptor Table Register
    pub fn load_ldtr(&mut self, selector: u16) {
        self.ldtr = selector;
    }

    /// Load the Task Register
    pub fn load_tr(&mut self, selector: u16) {
        self.tr = selector;
    }

    /// Reset protected mode state
    pub fn reset(&mut self) {
        self.msw = 0;
        self.gdtr = DescriptorTableRegister { base: 0, limit: 0 };
        self.idtr = DescriptorTableRegister { base: 0, limit: 0 };
        self.ldtr = 0;
        self.tr = 0;
    }
}

impl Default for ProtectedModeState {
    fn default() -> Self {
        Self::new()
    }
}

impl SegmentDescriptor {
    /// Parse a segment descriptor from 8 bytes in memory
    pub fn from_bytes(bytes: &[u8; 8]) -> Self {
        let limit_low = u16::from_le_bytes([bytes[0], bytes[1]]);
        let base_low = u16::from_le_bytes([bytes[2], bytes[3]]);
        let base_mid = bytes[4];
        let access = bytes[5];
        let limit_high_and_flags = bytes[6];
        let base_high = bytes[7];

        let limit = (limit_low as u32) | (((limit_high_and_flags & 0x0F) as u32) << 16);
        let base = (base_low as u32) | ((base_mid as u32) << 16) | ((base_high as u32) << 24);
        let flags = (limit_high_and_flags >> 4) & 0x0F;

        Self {
            base,
            limit,
            access,
            flags,
        }
    }

    /// Check if this descriptor is present
    #[inline]
    pub fn is_present(&self) -> bool {
        (self.access & 0x80) != 0
    }

    /// Get the descriptor privilege level (DPL)
    #[inline]
    pub fn dpl(&self) -> u8 {
        (self.access >> 5) & 0x03
    }

    /// Check if this is a code segment
    #[inline]
    pub fn is_code_segment(&self) -> bool {
        (self.access & 0x08) != 0
    }

    /// Check if this is a data segment
    #[inline]
    pub fn is_data_segment(&self) -> bool {
        !self.is_code_segment()
    }
}

/// Access Rights Byte flags
#[allow(dead_code)]
pub mod access_rights {
    /// Accessed bit
    pub const ACCESSED: u8 = 0x01;

    /// For data segments: Writable
    /// For code segments: Readable
    pub const WRITABLE_READABLE: u8 = 0x02;

    /// For data segments: Expand down
    /// For code segments: Conforming
    pub const EXPAND_DOWN_CONFORMING: u8 = 0x04;

    /// Code segment (1) vs Data segment (0)
    pub const CODE_SEGMENT: u8 = 0x08;

    /// Descriptor type: System (0) or Code/Data (1)
    pub const DESCRIPTOR_TYPE: u8 = 0x10;

    /// Descriptor Privilege Level (2 bits)
    pub const DPL_MASK: u8 = 0x60;

    /// Present bit
    pub const PRESENT: u8 = 0x80;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protected_mode_state_initialization() {
        let state = ProtectedModeState::new();
        assert!(!state.is_protected_mode());
        assert_eq!(state.msw, 0);
    }

    #[test]
    fn test_enable_protected_mode() {
        let mut state = ProtectedModeState::new();
        assert!(!state.is_protected_mode());

        state.enable_protected_mode();
        assert!(state.is_protected_mode());
        assert_eq!(state.msw & 0x0001, 1);
    }

    #[test]
    fn test_disable_protected_mode() {
        let mut state = ProtectedModeState::new();
        state.enable_protected_mode();
        assert!(state.is_protected_mode());

        state.disable_protected_mode();
        assert!(!state.is_protected_mode());
    }

    #[test]
    fn test_set_msw() {
        let mut state = ProtectedModeState::new();
        state.set_msw(0x000F); // Set all 4 low bits
        assert_eq!(state.msw & 0x000F, 0x000F);
    }

    #[test]
    fn test_load_gdtr() {
        let mut state = ProtectedModeState::new();
        state.load_gdtr(0x12345678, 0xFFFF);
        assert_eq!(state.gdtr.base, 0x00345678); // 24-bit mask on 80286
        assert_eq!(state.gdtr.limit, 0xFFFF);
    }

    #[test]
    fn test_load_idtr() {
        let mut state = ProtectedModeState::new();
        state.load_idtr(0xABCDEF00, 0x1234);
        assert_eq!(state.idtr.base, 0x00CDEF00); // 24-bit mask
        assert_eq!(state.idtr.limit, 0x1234);
    }

    #[test]
    fn test_segment_descriptor_from_bytes() {
        // Create a descriptor: base=0x100000, limit=0xFFFFF, access=0x9A (code, present, DPL=0)
        let bytes = [
            0xFF, 0xFF, // Limit low (0xFFFF)
            0x00, 0x00, // Base low (0x0000)
            0x10, // Base mid (0x10)
            0x9A, // Access (code, present, readable, DPL=0)
            0xCF, // Limit high (0xF) + flags (0xC = granularity + 32-bit)
            0x00, // Base high (0x00)
        ];

        let desc = SegmentDescriptor::from_bytes(&bytes);
        assert_eq!(desc.base, 0x00100000);
        assert_eq!(desc.limit, 0x000FFFFF);
        assert_eq!(desc.access, 0x9A);
        assert!(desc.is_present());
        assert!(desc.is_code_segment());
        assert_eq!(desc.dpl(), 0);
    }

    #[test]
    fn test_segment_descriptor_data_segment() {
        let bytes = [
            0x00, 0x10, // Limit
            0x00, 0x00, // Base low
            0x00, // Base mid
            0x92, // Access (data, present, writable, DPL=0)
            0x00, // Flags
            0x00, // Base high
        ];

        let desc = SegmentDescriptor::from_bytes(&bytes);
        assert!(desc.is_data_segment());
        assert!(!desc.is_code_segment());
    }

    #[test]
    fn test_segment_descriptor_not_present() {
        let bytes = [
            0xFF, 0xFF, 0x00, 0x00, 0x00, 0x12, // Access without present bit
            0x00, 0x00,
        ];

        let desc = SegmentDescriptor::from_bytes(&bytes);
        assert!(!desc.is_present());
    }

    #[test]
    fn test_segment_descriptor_dpl() {
        let bytes = [
            0x00, 0x00, 0x00, 0x00, 0x00, 0xFA, // Access with DPL=3 (bits 5-6 = 11)
            0x00, 0x00,
        ];

        let desc = SegmentDescriptor::from_bytes(&bytes);
        assert_eq!(desc.dpl(), 3);
    }
}
