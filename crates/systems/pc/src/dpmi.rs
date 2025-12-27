//! DPMI (DOS Protected Mode Interface) implementation
//!
//! This module implements DPMI 0.9 specification for Windows 3.1 Enhanced mode support.
//! DPMI provides a standardized interface for DOS programs to run in protected mode.
//!
//! Reference: DPMI Specification Version 0.9 (1990)

#![allow(dead_code)] // Some methods used by tests and future features

use crate::CpuModel;

/// DPMI (DOS Protected Mode Interface) driver state
///
/// Implements DPMI 0.9 specification for protected mode memory management,
/// interrupt handling, and descriptor table operations.
#[derive(Debug, Clone)]
pub struct DpmiDriver {
    /// Whether DPMI is installed
    installed: bool,
    /// DPMI version (major.minor in BCD format)
    version: u16,
    /// Processor type (03h = 80386, 04h = 80486, 05h = Pentium, etc.)
    processor_type: u8,
    /// Current DPMI mode (0 = 16-bit, 1 = 32-bit)
    mode: u8,
    /// Maximum number of descriptors available
    max_descriptors: u16,
    /// Number of descriptors currently allocated
    allocated_descriptors: u16,
    /// Protected mode entry point (segment:offset)
    entry_point: (u16, u16),
    /// Real mode callback list (selector, offset pairs)
    callbacks: Vec<(u16, u32)>,
    /// Allocated descriptors (selector -> base/limit/flags)
    descriptors: Vec<DpmiDescriptor>,
}

/// DPMI descriptor entry
#[derive(Debug, Clone)]
struct DpmiDescriptor {
    selector: u16,
    base: u32,
    limit: u32,
    access_rights: u8,
    flags: u8,
}

impl DpmiDriver {
    /// Create a new DPMI driver (not installed by default)
    /// Defaults to 80386 processor type
    pub fn new() -> Self {
        Self::with_cpu_model(CpuModel::Intel80386)
    }

    /// Create a new DPMI driver with a specific CPU model
    /// This sets the appropriate processor_type based on the CPU
    pub fn with_cpu_model(cpu_model: CpuModel) -> Self {
        let processor_type = match cpu_model {
            CpuModel::Intel8086 | CpuModel::Intel8088 => 0x00, // 8086
            CpuModel::Intel80186 | CpuModel::Intel80188 => 0x01, // 80186
            CpuModel::Intel80286 => 0x02,                      // 80286
            CpuModel::Intel80386 => 0x03,                      // 80386
            CpuModel::Intel80486
            | CpuModel::Intel80486SX
            | CpuModel::Intel80486DX2
            | CpuModel::Intel80486SX2
            | CpuModel::Intel80486DX4 => 0x04, // 80486
            CpuModel::IntelPentium | CpuModel::IntelPentiumMMX => 0x05, // Pentium
        };

        DpmiDriver {
            installed: false,
            version: 0x0090, // DPMI 0.9 in BCD
            processor_type,  // Set based on CPU model
            mode: 0,         // 16-bit mode
            max_descriptors: 256,
            allocated_descriptors: 0,
            entry_point: (0xF000, 0xE000), // Fake entry point
            callbacks: Vec::new(),
            descriptors: Vec::new(),
        }
    }

    /// Install DPMI driver
    pub fn install(&mut self) {
        self.installed = true;
    }

    /// Check if DPMI is installed
    pub fn is_installed(&self) -> bool {
        self.installed
    }

    /// Get DPMI version (BCD format: major.minor)
    pub fn version(&self) -> u16 {
        self.version
    }

    /// Get processor type
    pub fn processor_type(&self) -> u8 {
        self.processor_type
    }

    /// Set processor type based on CPU model
    pub fn set_processor_type_for_cpu(&mut self, cpu_model: CpuModel) {
        self.processor_type = match cpu_model {
            CpuModel::Intel8086 | CpuModel::Intel8088 => 0x00, // 8086
            CpuModel::Intel80186 | CpuModel::Intel80188 => 0x01, // 80186
            CpuModel::Intel80286 => 0x02,                      // 80286
            CpuModel::Intel80386 => 0x03,                      // 80386
            CpuModel::Intel80486
            | CpuModel::Intel80486SX
            | CpuModel::Intel80486DX2
            | CpuModel::Intel80486SX2
            | CpuModel::Intel80486DX4 => 0x04, // 80486
            CpuModel::IntelPentium | CpuModel::IntelPentiumMMX => 0x05, // Pentium
        };
    }

    /// Get entry point segment
    pub fn entry_segment(&self) -> u16 {
        self.entry_point.0
    }

    /// Get entry point offset
    pub fn entry_offset(&self) -> u16 {
        self.entry_point.1
    }

    /// Get number of available descriptors
    pub fn available_descriptors(&self) -> u16 {
        self.max_descriptors - self.allocated_descriptors
    }

    /// Allocate a descriptor (INT 31h, AX=0000h)
    pub fn allocate_descriptor(&mut self, count: u16) -> Result<u16, u16> {
        if count == 0 {
            return Err(0x8021); // Invalid value
        }

        if self.allocated_descriptors + count > self.max_descriptors {
            return Err(0x8011); // Descriptor unavailable
        }

        // Allocate first selector
        let selector = 0x0008 + (self.allocated_descriptors * 8);

        // Create descriptors
        for i in 0..count {
            let desc = DpmiDescriptor {
                selector: selector + (i * 8),
                base: 0,
                limit: 0,
                access_rights: 0x92, // Present, DPL=0, Data segment
                flags: 0,
            };
            self.descriptors.push(desc);
        }

        self.allocated_descriptors += count;
        Ok(selector)
    }

    /// Free a descriptor (INT 31h, AX=0001h)
    pub fn free_descriptor(&mut self, selector: u16) -> Result<(), u16> {
        // Find and remove descriptor
        if let Some(pos) = self.descriptors.iter().position(|d| d.selector == selector) {
            self.descriptors.remove(pos);
            self.allocated_descriptors = self.allocated_descriptors.saturating_sub(1);
            Ok(())
        } else {
            Err(0x8022) // Invalid selector
        }
    }

    /// Get segment base address (INT 31h, AX=0006h)
    pub fn get_segment_base(&self, selector: u16) -> Result<u32, u16> {
        if let Some(desc) = self.descriptors.iter().find(|d| d.selector == selector) {
            Ok(desc.base)
        } else {
            Err(0x8022) // Invalid selector
        }
    }

    /// Set segment base address (INT 31h, AX=0007h)
    pub fn set_segment_base(&mut self, selector: u16, base: u32) -> Result<(), u16> {
        if let Some(desc) = self.descriptors.iter_mut().find(|d| d.selector == selector) {
            desc.base = base;
            Ok(())
        } else {
            Err(0x8022) // Invalid selector
        }
    }

    /// Get segment limit (INT 31h, AX=0008h)
    pub fn get_segment_limit(&self, selector: u16) -> Result<u32, u16> {
        if let Some(desc) = self.descriptors.iter().find(|d| d.selector == selector) {
            Ok(desc.limit)
        } else {
            Err(0x8022) // Invalid selector
        }
    }

    /// Set segment limit (INT 31h, AX=0009h)
    pub fn set_segment_limit(&mut self, selector: u16, limit: u32) -> Result<(), u16> {
        if let Some(desc) = self.descriptors.iter_mut().find(|d| d.selector == selector) {
            desc.limit = limit;
            Ok(())
        } else {
            Err(0x8022) // Invalid selector
        }
    }

    /// Allocate memory block (INT 31h, AX=0501h)
    pub fn allocate_memory(&mut self, size: u32) -> Result<(u32, u32), u16> {
        // Simple allocation - return fake addresses
        // In real implementation, this would allocate from extended memory
        if size == 0 {
            return Err(0x8021); // Invalid value
        }

        // Return fake linear address and handle
        let linear_addr = 0x00100000 + (self.allocated_descriptors as u32 * 0x10000);
        let handle = self.allocated_descriptors as u32 + 1;

        Ok((linear_addr, handle))
    }

    /// Free memory block (INT 31h, AX=0502h)
    pub fn free_memory(&mut self, _handle: u32) -> Result<(), u16> {
        // Stub implementation
        Ok(())
    }

    /// Get free memory information (INT 31h, AX=0500h)
    pub fn get_free_memory_info(&self) -> (u32, u32, u32) {
        // Return fake memory info
        // largest_available_block, maximum_unlocked_page_size, largest_lockable_size
        (0x00100000, 0x00100000, 0x00100000) // 1MB each
    }

    /// Reset the DPMI driver
    pub fn reset(&mut self) {
        self.descriptors.clear();
        self.allocated_descriptors = 0;
        self.callbacks.clear();
    }
}

impl Default for DpmiDriver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dpmi_creation() {
        let dpmi = DpmiDriver::new();
        assert!(!dpmi.is_installed());
        assert_eq!(dpmi.version(), 0x0090);
        assert_eq!(dpmi.processor_type(), 0x03);
    }

    #[test]
    fn test_dpmi_install() {
        let mut dpmi = DpmiDriver::new();
        assert!(!dpmi.is_installed());
        dpmi.install();
        assert!(dpmi.is_installed());
    }

    #[test]
    fn test_allocate_descriptor() {
        let mut dpmi = DpmiDriver::new();
        dpmi.install();

        let result = dpmi.allocate_descriptor(1);
        assert!(result.is_ok());
        let selector = result.unwrap();
        assert_eq!(selector, 0x0008);
        assert_eq!(dpmi.allocated_descriptors, 1);
    }

    #[test]
    fn test_allocate_multiple_descriptors() {
        let mut dpmi = DpmiDriver::new();
        dpmi.install();

        let result = dpmi.allocate_descriptor(3);
        assert!(result.is_ok());
        let selector = result.unwrap();
        assert_eq!(selector, 0x0008);
        assert_eq!(dpmi.allocated_descriptors, 3);
    }

    #[test]
    fn test_free_descriptor() {
        let mut dpmi = DpmiDriver::new();
        dpmi.install();

        let selector = dpmi.allocate_descriptor(1).unwrap();
        assert_eq!(dpmi.allocated_descriptors, 1);

        let result = dpmi.free_descriptor(selector);
        assert!(result.is_ok());
        assert_eq!(dpmi.allocated_descriptors, 0);
    }

    #[test]
    fn test_segment_base() {
        let mut dpmi = DpmiDriver::new();
        dpmi.install();

        let selector = dpmi.allocate_descriptor(1).unwrap();

        // Initially should be 0
        let base = dpmi.get_segment_base(selector).unwrap();
        assert_eq!(base, 0);

        // Set to new value
        dpmi.set_segment_base(selector, 0x12345678).unwrap();
        let base = dpmi.get_segment_base(selector).unwrap();
        assert_eq!(base, 0x12345678);
    }

    #[test]
    fn test_segment_limit() {
        let mut dpmi = DpmiDriver::new();
        dpmi.install();

        let selector = dpmi.allocate_descriptor(1).unwrap();

        // Initially should be 0
        let limit = dpmi.get_segment_limit(selector).unwrap();
        assert_eq!(limit, 0);

        // Set to new value
        dpmi.set_segment_limit(selector, 0xFFFF).unwrap();
        let limit = dpmi.get_segment_limit(selector).unwrap();
        assert_eq!(limit, 0xFFFF);
    }

    #[test]
    fn test_allocate_memory() {
        let mut dpmi = DpmiDriver::new();
        dpmi.install();

        let result = dpmi.allocate_memory(0x10000);
        assert!(result.is_ok());
        let (linear_addr, handle) = result.unwrap();
        assert!(linear_addr >= 0x00100000);
        assert!(handle > 0);
    }

    #[test]
    fn test_free_memory() {
        let mut dpmi = DpmiDriver::new();
        dpmi.install();

        let (_addr, handle) = dpmi.allocate_memory(0x10000).unwrap();
        let result = dpmi.free_memory(handle);
        assert!(result.is_ok());
    }

    #[test]
    fn test_get_free_memory_info() {
        let dpmi = DpmiDriver::new();
        let (largest, max_unlocked, lockable) = dpmi.get_free_memory_info();
        assert!(largest > 0);
        assert!(max_unlocked > 0);
        assert!(lockable > 0);
    }

    #[test]
    fn test_reset() {
        let mut dpmi = DpmiDriver::new();
        dpmi.install();

        dpmi.allocate_descriptor(3).unwrap();
        assert_eq!(dpmi.allocated_descriptors, 3);

        dpmi.reset();
        assert_eq!(dpmi.allocated_descriptors, 0);
        assert_eq!(dpmi.descriptors.len(), 0);
    }
}
