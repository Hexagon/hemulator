//! BIOS implementation for PC emulation
//!
//! This provides boot functionality for the PC system.
//! The BIOS sets up the system and attempts to boot from disk.

pub use boot_priority::BootPriority;

mod boot_priority {
    /// Boot priority options
    #[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
    pub enum BootPriority {
        /// Boot from floppy first, then hard drive
        FloppyFirst,
        /// Boot from hard drive first, then floppy
        HardDriveFirst,
        /// Boot from floppy only
        FloppyOnly,
        /// Boot from hard drive only
        HardDriveOnly,
    }

    impl Default for BootPriority {
        fn default() -> Self {
            BootPriority::FloppyFirst
        }
    }
}

/// Generate a minimal BIOS ROM
///
/// This BIOS:
/// 1. Initializes segment registers and stack
/// 2. Attempts to boot from disk (handled by emulator)
/// 3. If boot fails, halts
///
/// Size: 64KB (standard BIOS size)
pub fn generate_minimal_bios() -> Vec<u8> {
    let mut bios = vec![0x00; 0x10000]; // 64KB of zeros

    // Main boot code at 0xF000:0x0000 (start of BIOS ROM)
    // The emulator will intercept execution at specific points to handle boot logic
    let main_boot: Vec<u8> = vec![
        // CLI - disable interrupts during setup
        0xFA,
        
        // Initialize segment registers
        0xB8, 0x00, 0x00,       // MOV AX, 0x0000
        0x8E, 0xD8,             // MOV DS, AX
        0x8E, 0xC0,             // MOV ES, AX
        0x8E, 0xD0,             // MOV SS, AX
        0xBC, 0xFE, 0xFF,       // MOV SP, 0xFFFE

        // STI - enable interrupts
        0xFB,

        // Boot from disk - the emulator intercepts this and loads boot sector
        // For now, just jump to boot sector location (0x0000:0x7C00)
        // The emulator will load the boot sector before executing this
        0xEA, 0x00, 0x7C,       // JMP FAR 0x0000:0x7C00
        0x00, 0x00,             // (continuation: segment = 0x0000)

        // If we return here, boot failed - halt
        0xF4,                   // HLT
    ];

    // Copy main boot code to start of BIOS
    bios[0..main_boot.len()].copy_from_slice(&main_boot);

    // BIOS entry point at 0xFFFF:0x0000 (physical 0xFFFF0)
    // This is where the CPU starts executing on reset
    let entry_point_offset = 0xFFF0;
    let boot_stub: Vec<u8> = vec![
        0xEA, 0x00, 0xF0,       // JMP FAR 0xF000:0x0000
        0x00, 0xF0,             // (continuation: segment = 0xF000)
    ];
    bios[entry_point_offset..entry_point_offset + boot_stub.len()].copy_from_slice(&boot_stub);

    // Add BIOS signature at end
    let date_offset = 0xFFF5;
    let date_str = b"01/01/88";
    bios[date_offset..date_offset + date_str.len()].copy_from_slice(date_str);

    // System model byte (0xFE = PC XT)
    bios[0xFFFE] = 0xFE;

    bios
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bios_generation() {
        let bios = generate_minimal_bios();

        // Check size
        assert_eq!(bios.len(), 0x10000);

        // Check that entry point has code (not all zeros)
        assert_ne!(bios[0xFFF0], 0x00);

        // Check JMP FAR instruction at entry point
        assert_eq!(bios[0xFFF0], 0xEA); // JMP FAR

        // Check system model byte
        assert_eq!(bios[0xFFFE], 0xFE); // PC XT

        // Check that main boot code exists at start
        assert_eq!(bios[0], 0xFA); // CLI instruction
    }

    #[test]
    fn test_bios_date_signature() {
        let bios = generate_minimal_bios();

        let date_offset = 0xFFF5;
        let date = &bios[date_offset..date_offset + 8];
        assert_eq!(date, b"01/01/88");
    }
}
