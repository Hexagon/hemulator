//! BIOS implementation for PC emulation
//!
//! This provides boot functionality for the PC system.
//! The BIOS sets up the system and attempts to boot from disk.

pub use boot_priority::BootPriority;

mod boot_priority {
    /// Boot priority options
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
    pub enum BootPriority {
        /// Boot from floppy first, then hard drive
        #[default]
        FloppyFirst,
        /// Boot from hard drive first, then floppy
        HardDriveFirst,
        /// Boot from floppy only
        FloppyOnly,
        /// Boot from hard drive only
        HardDriveOnly,
    }
}

/// Generate a minimal BIOS ROM
///
/// This BIOS:
/// 1. Initializes segment registers and stack  
/// 2. Loops indefinitely (boot sector loaded by emulator)
///
/// Note: The "Hemu" logo is displayed by writing directly to video RAM
/// from the emulator, not from BIOS code.
///
/// Size: 64KB (standard BIOS size)
pub fn generate_minimal_bios() -> Vec<u8> {
    let mut bios = vec![0x00; 0x10000]; // 64KB of zeros

    // Main boot code at offset 0 (will be reached via entry point jump)
    let init_code: Vec<u8> = vec![
        0xFA, // CLI - disable interrupts
        0xB8, 0x00, 0x00, // MOV AX, 0x0000
        0x8E, 0xD8, // MOV DS, AX
        0x8E, 0xC0, // MOV ES, AX
        0x8E, 0xD0, // MOV SS, AX
        0xBC, 0xFE, 0xFF, // MOV SP, 0xFFFE
        0xFB, // STI - enable interrupts
        0xEB, 0xFE, // JMP short -2 (infinite loop - emulator loads boot sector)
    ];
    bios[0..init_code.len()].copy_from_slice(&init_code);

    // BIOS entry point at 0xFFF0 - CPU starts here (CS=0xFFFF, IP=0x0000)
    // Jump backward to start of BIOS code at offset 0
    // From 0xFFF0 to 0x0000 is -0xFFF0, but in 16-bit: 0x10 forward wraps to 0
    let entry_point_offset = 0xFFF0;
    let entry_code: Vec<u8> = vec![
        // We can't use a far jump, and near jump is too far
        // So let's just put the code here and skip the signature
        0xFA, // CLI
        0xB8, 0x00, 0x00, // MOV AX, 0
        0x8E, 0xD8, // MOV DS, AX
        0x8E, 0xC0, // MOV ES, AX
        0x8E, 0xD0, // MOV SS, AX
        0xBC, 0xFE, 0xFF, // MOV SP, 0xFFFE
        0xEB, 0x00, // JMP short +0 (skip to next instruction - placeholder)
    ];
    bios[entry_point_offset..entry_point_offset + entry_code.len()].copy_from_slice(&entry_code);

    // Add BIOS signature
    let date_offset = 0xFFF5;
    let date_str = b"12/23/24";
    bios[date_offset..date_offset + date_str.len()].copy_from_slice(date_str);

    // System model byte (0xFE = PC XT)
    bios[0xFFFE] = 0xFE;

    bios
}

/// Write "hemu" ASCII art logo directly to video RAM
/// This is called from the PC system to display the logo at boot
pub fn write_hemu_logo_to_vram(vram: &mut [u8]) {
    // ASCII art for "hemu" using dense block-style characters
    let logo_lines = [
        "#   # #### #   # #  #",
        "##### #    ## ## #  #",
        "#   # ###  # # # #  #",
        "#   # #    #   # #  #",
        "#   # #### #   #  ##",
    ];

    let attribute = 0x0E; // Yellow on black
    let start_row = 10;
    let start_col = 29; // Center the 21-char wide logo on an 80-char screen

    // Video RAM offset for text mode (0xB8000 - 0xA0000 = 0x18000)
    let text_offset = 0x18000;

    for (row_offset, line) in logo_lines.iter().enumerate() {
        let row = start_row + row_offset;
        let screen_offset = text_offset + (row * 80 + start_col) * 2;

        for (col_offset, ch) in line.chars().enumerate() {
            let offset = screen_offset + col_offset * 2;
            if offset + 1 < vram.len() {
                vram[offset] = ch as u8;
                vram[offset + 1] = attribute;
            }
        }
    }
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

        // Check CLI instruction at entry point
        assert_eq!(bios[0xFFF0], 0xFA); // CLI

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
        assert_eq!(date, b"12/23/24");
    }
}
