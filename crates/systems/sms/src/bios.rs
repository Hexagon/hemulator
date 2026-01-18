//! SMS BIOS implementation
//!
//! The Sega Master System has a built-in BIOS ROM (typically 2KB or 8KB)
//! that provides:
//! - Boot sequence and SEGA logo display
//! - Basic game initialization
//! - Some utility functions
//!
//! This module provides:
//! 1. A minimal default BIOS that allows games to run without a real BIOS
//! 2. Support for loading real BIOS ROMs
//!
//! # BIOS Memory Map
//!
//! The BIOS is mapped to address 0x0000-0x1FFF (8KB) or 0x0000-0x03FF (1KB)
//! depending on the BIOS size. It can be disabled via bit 3 of the memory
//! control register (port 0x3E), which maps the cartridge ROM to 0x0000 instead.

/// Generate a minimal SMS BIOS
///
/// This BIOS provides the bare minimum to boot SMS games:
/// 1. Initialize the system (SP, interrupts)
/// 2. Disable the BIOS (set bit 3 of port 0x3E)
/// 3. Jump to cartridge entry point at 0x0000
///
/// Size: 1KB (minimal BIOS size)
#[allow(dead_code)] // This is part of the public API for users who want to use a BIOS
pub fn generate_minimal_bios() -> Vec<u8> {
    let mut bios = vec![0x00; 0x400]; // 1KB of zeros

    // Entry point at 0x0000: Initialize and boot
    let boot_code: Vec<u8> = vec![
        // Initialize stack pointer to top of RAM (0xDFFF)
        0x31, 0xFF, 0xDF, // LD SP, 0xDFFF
        // Disable interrupts during init
        0xF3, // DI
        // Set interrupt mode 1 (RST 38h)
        0xED, 0x56, // IM 1
        // Disable BIOS by setting bit 3 of memory control register
        // This maps cartridge ROM to 0x0000 instead of BIOS
        0x3E, 0x08, // LD A, 0x08
        0xD3, 0x3E, // OUT (0x3E), A
        // Enable interrupts
        0xFB, // EI
        // Jump to cartridge entry point at 0x0000
        // Since we just disabled the BIOS, 0x0000 now points to cartridge ROM
        0xC3, 0x00, 0x00, // JP 0x0000
    ];

    // Copy boot code to BIOS ROM
    bios[0..boot_code.len()].copy_from_slice(&boot_code);

    // RST 38h handler at 0x0038 (interrupt vector for IM 1)
    // This is a simple RETI (return from interrupt)
    bios[0x0038] = 0xED; // RETI prefix
    bios[0x0039] = 0x4D; // RETI opcode

    // NMI handler at 0x0066
    // Simple RETN (return from NMI)
    bios[0x0066] = 0xED; // RETN prefix
    bios[0x0067] = 0x45; // RETN opcode

    bios
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_minimal_bios() {
        let bios = generate_minimal_bios();

        // Check size
        assert_eq!(bios.len(), 0x400);

        // Check boot code starts correctly (LD SP, 0xDFFF)
        assert_eq!(bios[0], 0x31); // LD SP opcode
        assert_eq!(bios[1], 0xFF); // Low byte
        assert_eq!(bios[2], 0xDF); // High byte

        // Check RST 38h handler exists
        assert_eq!(bios[0x0038], 0xED); // RETI prefix
        assert_eq!(bios[0x0039], 0x4D); // RETI opcode

        // Check NMI handler exists
        assert_eq!(bios[0x0066], 0xED); // RETN prefix
        assert_eq!(bios[0x0067], 0x45); // RETN opcode
    }
}
