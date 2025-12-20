//! Basic BIOS stub for PC emulation
//!
//! This provides a minimal BIOS that allows booting DOS executables.
//! A real PC BIOS is much more complex, but for basic emulation we can
//! provide a simple stub that sets up the system and jumps to the loaded program.

/// Generate a minimal BIOS ROM
///
/// This BIOS:
/// 1. Initializes segment registers
/// 2. Sets up the stack
/// 3. Jumps to the loaded program at 0x0000:0x0100 (COM file convention)
///
/// Size: 64KB (standard BIOS size), but most is just padding
pub fn generate_minimal_bios() -> Vec<u8> {
    let mut bios = vec![0x00; 0x10000]; // 64KB of zeros

    // BIOS entry point at 0xFFFF:0x0000 (physical 0xFFFF0)
    // This is where the CPU starts executing on reset
    // We only have 16 bytes at the very end, so use a minimal stub
    let entry_point_offset = 0xFFF0;

    // Minimal boot code that fits in 16 bytes:
    // Jump to a larger boot routine at the start of the BIOS
    let boot_stub: Vec<u8> = vec![
        0xEA, 0x00, 0xF0, // JMP FAR 0xF000:0x0000 - jump to main boot code
        0x00, 0xF0, // (continuation of far jump: segment = 0xF000)
    ];

    // Main boot code at 0xF0000 (start of BIOS)
    let main_boot: Vec<u8> = vec![
        0xFA, // CLI - disable interrupts
        0xB8, 0x00, 0x00, // MOV AX, 0x0000
        0x8E, 0xD8, // MOV DS, AX - set data segment to 0
        0x8E, 0xC0, // MOV ES, AX - set extra segment to 0
        0x8E, 0xD0, // MOV SS, AX - set stack segment to 0
        0xBC, 0xFE, 0xFF, // MOV SP, 0xFFFE - set stack pointer
        0xFB, // STI - enable interrupts
        0xEA, 0x00, 0x01, // JMP FAR 0x0000:0x0100 - jump to COM program
        0x00, 0x00, // (continuation of far jump: segment = 0x0000)
    ];

    // Copy boot stub to entry point (must fit in 16 bytes)
    let stub_len = boot_stub.len();
    bios[entry_point_offset..entry_point_offset + stub_len].copy_from_slice(&boot_stub);

    // Copy main boot code to start of BIOS
    let main_len = main_boot.len();
    bios[0..main_len].copy_from_slice(&main_boot);

    // Add BIOS signature at end (some programs check for this)
    // Standard PC BIOS date: "01/01/88"
    let date_offset = 0xFFF5;
    let date_str = b"01/01/88";
    bios[date_offset..date_offset + date_str.len()].copy_from_slice(date_str);

    // System model byte (0xFF = PC, 0xFE = XT, 0xFC = AT)
    bios[0xFFFE] = 0xFE; // PC XT

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
