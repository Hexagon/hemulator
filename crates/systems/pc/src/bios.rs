//! BIOS implementation for PC emulation
//!
//! This provides boot functionality for the PC system.
//! The BIOS sets up the system and attempts to boot from disk.

use emu_core::cpu_8086::CpuModel;

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

/// Generate a minimal BIOS ROM with interrupt handlers
///
/// This BIOS:
/// 1. Initializes segment registers and stack  
/// 2. Sets up interrupt vectors in low memory
/// 3. Provides basic INT 10h (video), INT 13h (disk), INT 16h (keyboard) handlers
/// 4. Jumps to boot sector at 0x0000:0x7C00 (loaded by emulator)
///
/// NOTE: INT 21h (DOS API) is NOT provided by BIOS - DOS installs it during boot
///
/// Size: 64KB (standard BIOS size)
pub fn generate_minimal_bios() -> Vec<u8> {
    let mut bios = vec![0x00; 0x10000]; // 64KB of zeros

    // Generic stub handler for interrupts (just IRET)
    let stub_offset = 0x40;
    let stub_handler: Vec<u8> = vec![
        0xCF, // IRET
    ];
    bios[stub_offset..stub_offset + stub_handler.len()].copy_from_slice(&stub_handler);

    // INT 0h handler at offset 0x50 - Divide Error Exception
    // This is called when division by zero or overflow occurs
    let int00h_offset = 0x50;
    let int00h_handler: Vec<u8> = vec![
        // For now, just return (ignore the error)
        // In a real system, this would display an error message
        0xCF, // IRET
    ];
    bios[int00h_offset..int00h_offset + int00h_handler.len()].copy_from_slice(&int00h_handler);

    // INT 10h handler at offset 0x100 - Video Services
    let int10h_offset = 0x100;
    let int10h_handler: Vec<u8> = vec![
        // For now, just return successfully (clear CF and return)
        0x50, // PUSH AX
        0x80, 0xFC, 0x0E, // CMP AH, 0x0E (teletype output)
        0x75, 0x01, // JNZ skip
        // If teletype, we could write to video memory here
        // For now just skip
        0x58, // POP AX (skip label)
        0xF8, // CLC (clear carry flag - success)
        0xCF, // IRET
    ];
    bios[int10h_offset..int10h_offset + int10h_handler.len()].copy_from_slice(&int10h_handler);

    // INT 12h handler at offset 0x180 - Get Memory Size
    let int12h_offset = 0x180;
    let int12h_handler: Vec<u8> = vec![
        // Return 640KB in AX (standard PC conventional memory)
        0xB8, 0x80, 0x02, // MOV AX, 0x0280 (640 in decimal)
        0xF8, // CLC (clear carry flag - success)
        0xCF, // IRET
    ];
    bios[int12h_offset..int12h_offset + int12h_handler.len()].copy_from_slice(&int12h_handler);

    // INT 13h handler at offset 0x200 - Disk Services
    let int13h_offset = 0x200;
    let int13h_handler: Vec<u8> = vec![
        // Return success for all disk operations
        0x30, 0xE4, // XOR AH, AH (AH = 0 = success)
        0xF8, // CLC (clear carry flag)
        0xCF, // IRET
    ];
    bios[int13h_offset..int13h_offset + int13h_handler.len()].copy_from_slice(&int13h_handler);

    // INT 16h handler at offset 0x300 - Keyboard Services
    let int16h_offset = 0x300;
    let int16h_handler: Vec<u8> = vec![
        // Return 0 in AX (no key available)
        0x30, 0xC0, // XOR AX, AX
        0xF8, // CLC
        0xCF, // IRET
    ];
    bios[int16h_offset..int16h_offset + int16h_handler.len()].copy_from_slice(&int16h_handler);

    // NOTE: INT 21h (DOS Services) is NOT provided by BIOS
    // DOS installs its own INT 21h handler when it loads (IO.SYS/MSDOS.SYS)
    // The BIOS must not provide an INT 21h handler or it will interfere with DOS initialization

    // Main boot code at offset 0 (will be reached via entry point jump)
    // This code checks if boot sector is loaded and either boots or halts
    let init_code: Vec<u8> = vec![
        0xFA, // CLI - disable interrupts
        0xB8, 0x00, 0x00, // MOV AX, 0x0000
        0x8E, 0xD8, // MOV DS, AX
        0x8E, 0xC0, // MOV ES, AX
        0x8E, 0xD0, // MOV SS, AX
        0xBC, 0xFE, 0xFF, // MOV SP, 0xFFFE
        // Set up interrupt vectors (all point to F000:offset)
        // INT 0x00 (Divide Error) at 0x0000
        0xB8, 0x50, 0x00, // MOV AX, 0x0050 (offset of INT 0h handler)
        0xA3, 0x00, 0x00, // MOV [0x0000], AX
        0xB8, 0x00, 0xF0, // MOV AX, 0xF000 (segment)
        0xA3, 0x02, 0x00, // MOV [0x0002], AX
        // INT 0x05 (Print Screen/BOUND) - stub
        0xB8, 0x40, 0x00, // MOV AX, 0x0040 (stub handler)
        0xA3, 0x14, 0x00, // MOV [0x0014], AX (INT 05h vector = 0x0014)
        0xB8, 0x00, 0xF0, // MOV AX, 0xF000
        0xA3, 0x16, 0x00, // MOV [0x0016], AX
        // INT 0x08 (Timer Tick) - stub
        0xB8, 0x40, 0x00, // MOV AX, 0x0040
        0xA3, 0x20, 0x00, // MOV [0x0020], AX (INT 08h vector = 0x0020)
        0xB8, 0x00, 0xF0, // MOV AX, 0xF000
        0xA3, 0x22, 0x00, // MOV [0x0022], AX
        // INT 0x09 (Keyboard Hardware) - stub
        0xB8, 0x40, 0x00, // MOV AX, 0x0040
        0xA3, 0x24, 0x00, // MOV [0x0024], AX (INT 09h vector = 0x0024)
        0xB8, 0x00, 0xF0, // MOV AX, 0xF000
        0xA3, 0x26, 0x00, // MOV [0x0026], AX
        // INT 0x10 (Video Services) at 0x0040
        0xB8, 0x00, 0x01, // MOV AX, 0x0100 (offset of INT 10h handler)
        0xA3, 0x40, 0x00, // MOV [0x0040], AX
        0xB8, 0x00, 0xF0, // MOV AX, 0xF000 (segment)
        0xA3, 0x42, 0x00, // MOV [0x0042], AX
        // INT 0x11 (Equipment List) - stub
        0xB8, 0x40, 0x00, // MOV AX, 0x0040
        0xA3, 0x44, 0x00, // MOV [0x0044], AX (INT 11h vector = 0x0044)
        0xB8, 0x00, 0xF0, // MOV AX, 0xF000
        0xA3, 0x46, 0x00, // MOV [0x0046], AX
        // INT 0x12 (Get Memory Size) at 0x0048
        0xB8, 0x80, 0x01, // MOV AX, 0x0180 (offset of INT 12h handler)
        0xA3, 0x48, 0x00, // MOV [0x0048], AX
        0xB8, 0x00, 0xF0, // MOV AX, 0xF000 (segment)
        0xA3, 0x4A, 0x00, // MOV [0x004A], AX
        // INT 0x13 (Disk Services) at 0x004C
        0xB8, 0x00, 0x02, // MOV AX, 0x0200
        0xA3, 0x4C, 0x00, // MOV [0x004C], AX
        0xB8, 0x00, 0xF0, // MOV AX, 0xF000
        0xA3, 0x4E, 0x00, // MOV [0x004E], AX
        // INT 0x14 (Serial Port) - stub
        0xB8, 0x40, 0x00, // MOV AX, 0x0040
        0xA3, 0x50, 0x00, // MOV [0x0050], AX (INT 14h vector = 0x0050)
        0xB8, 0x00, 0xF0, // MOV AX, 0xF000
        0xA3, 0x52, 0x00, // MOV [0x0052], AX
        // INT 0x16 (Keyboard Services) at 0x0058
        0xB8, 0x00, 0x03, // MOV AX, 0x0300
        0xA3, 0x58, 0x00, // MOV [0x0058], AX
        0xB8, 0x00, 0xF0, // MOV AX, 0xF000
        0xA3, 0x5A, 0x00, // MOV [0x005A], AX
        // INT 0x17 (Printer) - stub
        0xB8, 0x40, 0x00, // MOV AX, 0x0040
        0xA3, 0x5C, 0x00, // MOV [0x005C], AX (INT 17h vector = 0x005C)
        0xB8, 0x00, 0xF0, // MOV AX, 0xF000
        0xA3, 0x5E, 0x00, // MOV [0x005E], AX
        // INT 0x1A (Time/Date) - stub
        0xB8, 0x40, 0x00, // MOV AX, 0x0040
        0xA3, 0x68, 0x00, // MOV [0x0068], AX (INT 1Ah vector = 0x0068)
        0xB8, 0x00, 0xF0, // MOV AX, 0xF000
        0xA3, 0x6A, 0x00, // MOV [0x006A], AX
        // INT 0x1B (Ctrl-Break) - stub
        0xB8, 0x40, 0x00, // MOV AX, 0x0040
        0xA3, 0x6C, 0x00, // MOV [0x006C], AX (INT 1Bh vector = 0x006C)
        0xB8, 0x00, 0xF0, // MOV AX, 0xF000
        0xA3, 0x6E, 0x00, // MOV [0x006E], AX
        // INT 0x1C (Timer Tick User Handler) - stub
        0xB8, 0x40, 0x00, // MOV AX, 0x0040
        0xA3, 0x70, 0x00, // MOV [0x0070], AX (INT 1Ch vector = 0x0070)
        0xB8, 0x00, 0xF0, // MOV AX, 0xF000
        0xA3, 0x72, 0x00, // MOV [0x0072], AX
        // NOTE: INT 0x21 vector is NOT set up by BIOS - DOS will install it
        0xFB, // STI - enable interrupts
        // Check if boot sector is loaded by checking signature at 0x7C00 + 510
        // We'll check if byte at 0x7DFE is 0x55 and 0x7DFF is 0xAA
        0xBE, 0xFE, 0x7D, // MOV SI, 0x7DFE (offset of boot signature)
        0x8A, 0x04, // MOV AL, [SI]
        0x3C, 0x55, // CMP AL, 0x55
        0x75, 0x0C, // JNZ skip_boot (jump 12 bytes: 3+1+2+2+5)
        0x8A, 0x44, 0x01, // MOV AL, [SI+1]
        0x3C, 0xAA, // CMP AL, 0xAA
        0x75, 0x09, // JNZ skip_boot (jump 9 bytes: 2+2+5)
        // Boot signature valid - set DL to boot drive and jump to boot sector
        0xB2, 0x00, // MOV DL, 0x00 (boot drive = floppy A:)
        0xB6, 0x00, // MOV DH, 0x00 (clear DH as well for safety)
        0xEA, 0x00, 0x7C, 0x00, 0x00, // JMP FAR 0x0000:0x7C00
        // skip_boot: No valid boot sector - infinite loop (HLT)
        // 0xF4,       // HLT
        0xEB, 0xFE, // JMP -2 (infinite loop - simpler than HLT)
    ];
    bios[0..init_code.len()].copy_from_slice(&init_code);

    // BIOS entry point at 0xFFF0 - CPU starts here (CS=0xFFFF, IP=0x0000)
    // Add BIOS signature first at 0xFFF5 (date)
    let date_offset = 0xFFF5;
    let date_str = b"12/15/25"; // December 15, 2025
    bios[date_offset..date_offset + date_str.len()].copy_from_slice(date_str);

    // System model byte at 0xFFFE (PC XT)
    bios[0xFFFE] = 0xFE;

    // Entry code at 0xFFF0 - must not overwrite date or model byte
    // We have space from 0xFFF0 to 0xFFF4 (5 bytes) before the date
    let entry_point_offset = 0xFFF0;
    let entry_code: Vec<u8> = vec![
        // Jump to main init code at offset 0 (physical 0xF0000)
        // CS=0xF000, IP=0x0000 -> segment 0xF000 * 16 + 0x0000 = 0xF0000
        0xEA, 0x00, 0x00, 0x00, 0xF0, // JMP FAR 0xF000:0x0000 (5 bytes - fits!)
    ];
    bios[entry_point_offset..entry_point_offset + entry_code.len()].copy_from_slice(&entry_code);

    bios
}

/// Write "hemu" ASCII art logo directly to video RAM
/// This is called from the PC system to display the logo at boot
#[allow(dead_code)]
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

/// Write BIOS POST screen to video RAM
/// This displays a traditional PC BIOS Power-On Self-Test screen
///
/// # Arguments
/// * `vram` - Video RAM buffer to write to
/// * `cpu_model` - CPU model to display
/// * `memory_kb` - Memory size in KB to display
/// * `cpu_speed_mhz` - CPU speed in MHz to display
pub fn write_post_screen_to_vram(
    vram: &mut [u8],
    cpu_model: CpuModel,
    memory_kb: u32,
    cpu_speed_mhz: f64,
) {
    use std::time::{SystemTime, UNIX_EPOCH};

    // Get current system time
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();

    let total_seconds = now.as_secs();
    let seconds_in_day = total_seconds % 86400;

    // Calculate time
    let hours = (seconds_in_day / 3600) as u8;
    let minutes = ((seconds_in_day % 3600) / 60) as u8;
    let seconds = (seconds_in_day % 60) as u8;

    // Calculate date
    let days_since_epoch = total_seconds / 86400;
    let mut year = 1970;
    let mut remaining_days = days_since_epoch as u32;

    loop {
        let days_in_year = if (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0) {
            366
        } else {
            365
        };

        if remaining_days >= days_in_year {
            remaining_days -= days_in_year;
            year += 1;
        } else {
            break;
        }
    }

    // Simple month calculation
    let days_per_month = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut month = 1;
    let mut day = remaining_days + 1;

    for (m, &days) in days_per_month.iter().enumerate() {
        if day > days {
            day -= days;
            month = m + 2;
        } else {
            month = m + 1;
            break;
        }
    }

    // Format date and time strings
    let date_str = format!("{:02}/{:02}/{:04}", month, day, year);
    let time_str = format!("{:02}:{:02}:{:02}", hours, minutes, seconds);

    // Video RAM offset for text mode (0xB8000 - 0xA0000 = 0x18000)
    let text_offset = 0x18000;

    // Clear screen first (fill with spaces and default attribute)
    for i in (0..4000).step_by(2) {
        let offset = text_offset + i;
        if offset + 1 < vram.len() {
            vram[offset] = b' ';
            vram[offset + 1] = 0x07; // Light gray on black
        }
    }

    // Helper function to write a line of text
    let mut write_line = |row: usize, col: usize, text: &str, attr: u8| {
        let screen_offset = text_offset + (row * 80 + col) * 2;
        for (i, ch) in text.chars().enumerate() {
            let offset = screen_offset + i * 2;
            if offset + 1 < vram.len() {
                vram[offset] = ch as u8;
                vram[offset + 1] = attr;
            }
        }
    };

    // BIOS header (white on blue)
    let header_attr = 0x1F; // White on blue
    write_line(0, 0, &" ".repeat(80), header_attr);
    write_line(0, 2, "Hemu BIOS v1.0  (C) 2025 Hexagon", header_attr);
    write_line(0, 50, &date_str, header_attr);
    write_line(0, 65, &time_str, header_attr);

    // Separator
    write_line(1, 0, &"=".repeat(80), 0x07);

    // POST messages
    let post_attr = 0x0F; // Bright white on black
    let label_attr = 0x07; // Light gray on black

    write_line(3, 2, "Hemu PC/XT Compatible BIOS", post_attr);
    write_line(5, 2, "Processor:", label_attr);

    // Display CPU model name with actual emulated speed
    let cpu_name_base = match cpu_model {
        CpuModel::Intel8086 => "Intel 8086",
        CpuModel::Intel8088 => "Intel 8088",
        CpuModel::Intel80186 => "Intel 80186",
        CpuModel::Intel80188 => "Intel 80188",
        CpuModel::Intel80286 => "Intel 80286",
        CpuModel::Intel80386 => "Intel 80386",
    };
    // Display the actual emulated CPU speed
    let cpu_display = if cpu_speed_mhz.fract() == 0.0 {
        format!("{} @ {} MHz", cpu_name_base, cpu_speed_mhz as u32)
    } else {
        format!("{} @ {:.2} MHz", cpu_name_base, cpu_speed_mhz)
    };
    write_line(5, 15, &cpu_display, 0x0E); // Yellow

    write_line(7, 2, "Memory Test:", label_attr);
    let memory_str = format!("{}K OK", memory_kb);
    write_line(7, 15, &memory_str, 0x0A); // Bright green

    write_line(9, 2, "Disk Drives:", label_attr);
    write_line(10, 4, "Floppy A: Not present", 0x08); // Dark gray
    write_line(11, 4, "Floppy B: Not present", 0x08);
    write_line(12, 4, "Hard Disk C: Not present", 0x08);

    write_line(14, 2, "Boot Priority:", label_attr);
    write_line(14, 18, "Floppy First", 0x0E); // Yellow

    // Instructions at bottom
    let help_attr = 0x0B; // Bright cyan on black
    write_line(20, 2, "Press F3 to mount disks", help_attr);
    write_line(21, 2, "Press ESC to abort boot countdown", help_attr);
    write_line(22, 2, "Press F8 to save virtual machine", help_attr);

    // Bottom line (white on blue)
    write_line(24, 0, &" ".repeat(80), header_attr);
    write_line(
        24,
        2,
        "No bootable disk found. Insert disk and reset.",
        header_attr,
    );
}

/// Update the boot countdown on the POST screen
///
/// # Arguments
/// * `vram` - Video RAM buffer to write to
/// * `seconds_remaining` - Number of seconds remaining before boot
pub fn update_post_screen_countdown(vram: &mut [u8], seconds_remaining: u32) {
    let text_offset = 0x18000;

    let mut write_line = |row: usize, col: usize, text: &str, attr: u8| {
        let screen_offset = text_offset + (row * 80 + col) * 2;
        for (i, ch) in text.chars().enumerate() {
            let offset = screen_offset + i * 2;
            if offset + 1 < vram.len() {
                vram[offset] = ch as u8;
                vram[offset + 1] = attr;
            }
        }
    };

    // Update countdown message
    let countdown_attr = 0x0E; // Yellow on black
    if seconds_remaining > 0 {
        let message = format!(
            "Booting in {} second{}... Press ESC to stay in BIOS",
            seconds_remaining,
            if seconds_remaining == 1 { "" } else { "s" }
        );
        // Center the message on row 18
        let col = (80 - message.len()) / 2;
        write_line(18, col, &message, countdown_attr);
    } else {
        // Clear the countdown line when boot starts
        write_line(18, 0, &" ".repeat(80), 0x07);
    }
}

/// Update the disk drive status on the POST screen
pub fn update_post_screen_mounts(
    vram: &mut [u8],
    floppy_a: bool,
    floppy_b: bool,
    hard_drive: bool,
    boot_priority: BootPriority,
) {
    let text_offset = 0x18000;

    let mut write_line = |row: usize, col: usize, text: &str, attr: u8| {
        let screen_offset = text_offset + (row * 80 + col) * 2;
        for (i, ch) in text.chars().enumerate() {
            let offset = screen_offset + i * 2;
            if offset + 1 < vram.len() {
                vram[offset] = ch as u8;
                vram[offset + 1] = attr;
            }
        }
    };

    // Update disk status
    let present_attr = 0x0A; // Bright green
    let absent_attr = 0x08; // Dark gray

    // Floppy A
    if floppy_a {
        write_line(10, 4, "Floppy A: Present          ", present_attr);
    } else {
        write_line(10, 4, "Floppy A: Not present      ", absent_attr);
    }

    // Floppy B
    if floppy_b {
        write_line(11, 4, "Floppy B: Present          ", present_attr);
    } else {
        write_line(11, 4, "Floppy B: Not present      ", absent_attr);
    }

    // Hard Drive C
    if hard_drive {
        write_line(12, 4, "Hard Disk C: Present       ", present_attr);
    } else {
        write_line(12, 4, "Hard Disk C: Not present   ", absent_attr);
    }

    // Update boot priority
    let boot_text = match boot_priority {
        BootPriority::FloppyFirst => "Floppy First   ",
        BootPriority::HardDriveFirst => "Hard Drive First",
        BootPriority::FloppyOnly => "Floppy Only    ",
        BootPriority::HardDriveOnly => "Hard Drive Only",
    };
    write_line(14, 18, boot_text, 0x0E);

    // Update bottom message based on disk availability
    let header_attr = 0x1F;
    if floppy_a || floppy_b || hard_drive {
        write_line(
            24,
            2,
            "Bootable drive detected - See countdown above  ",
            header_attr,
        );
    } else {
        write_line(
            24,
            2,
            "No bootable disk found. Insert disk and reset.",
            header_attr,
        );
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

        // Check JMP FAR instruction at entry point (0xEA = JMP FAR)
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
        assert_eq!(date, b"12/15/25"); // December 15, 2025
    }
}
