//! PC CPU wrapper
//!
//! This module wraps the core 8086 CPU with PC-specific initialization and state.

use crate::bus::PcBus;
use emu_core::cpu_8086::{Cpu8086, CpuModel, Memory8086};
use emu_core::logging::{LogCategory, LogConfig, LogLevel};

/// BIOS video interrupt (INT 10h) - excluded from interrupt logging to reduce noise
#[allow(dead_code)]
const VIDEO_INTERRUPT: u8 = 0x10;

/// DOS error code: invalid file handle
const DOS_ERROR_INVALID_HANDLE: u16 = 0x0006;

/// BIOS Data Area: Hard drive count at 0x0040:0x0075
const BDA_HARD_DRIVE_COUNT: u32 = 0x475;

/// PC CPU wrapper
pub struct PcCpu {
    cpu: Cpu8086<PcBus>,
}

impl PcCpu {
    /// Create a new PC CPU with the given bus (defaults to 8086)
    #[allow(dead_code)] // Public API, used in tests
    pub fn new(bus: PcBus) -> Self {
        Self::with_model(bus, CpuModel::Intel8086)
    }

    /// Create a new PC CPU with a specific CPU model
    pub fn with_model(bus: PcBus, model: CpuModel) -> Self {
        let mut cpu = Cpu8086::with_model(bus, model);

        // IBM PC/XT boots at CS:IP = 0xFFFF:0x0000 (physical address 0xFFFF0)
        // This is the BIOS entry point
        cpu.cs = 0xFFFF;
        cpu.ip = 0x0000;

        // Initialize stack pointer
        cpu.ss = 0x0000;
        cpu.sp = 0xFFFE;

        // Initialize data segments
        cpu.ds = 0x0000;
        cpu.es = 0x0000;

        Self { cpu }
    }

    /// Get the CPU model
    pub fn model(&self) -> CpuModel {
        self.cpu.model()
    }

    /// Set the CPU model
    pub fn set_model(&mut self, model: CpuModel) {
        self.cpu.set_model(model);
    }

    /// Set CS register
    #[allow(dead_code)]
    pub fn set_cs(&mut self, value: u16) {
        self.cpu.cs = value;
    }

    /// Set IP register
    #[allow(dead_code)]
    pub fn set_ip(&mut self, value: u16) {
        self.cpu.ip = value as u32;
    }

    /// Reset the CPU to initial state
    pub fn reset(&mut self) {
        self.cpu.reset();

        // Restore PC boot state
        self.cpu.cs = 0xFFFF;
        self.cpu.ip = 0x0000u32;
        self.cpu.ss = 0x0000;
        self.cpu.sp = 0xFFFEu32;
        self.cpu.ds = 0x0000;
        self.cpu.es = 0x0000;
    }

    /// Check if the CPU is halted (e.g., waiting for keyboard input in INT 16h)
    pub fn is_halted(&self) -> bool {
        self.cpu.is_halted()
    }

    /// Execute one instruction
    pub fn step(&mut self) -> u32 {
        // Check if the next instruction is a BIOS/DOS interrupt we need to handle
        // Opcode 0xCD (INT) followed by interrupt number
        let cs = self.cpu.cs;
        let ip = self.cpu.ip;
        let physical_addr = ((cs as u32) << 4) + (ip as u32);

        // Peek at the instruction without advancing IP
        let opcode = self.cpu.memory.read(physical_addr);

        // Enable PC tracing with EMU_TRACE_PC=1
        if LogConfig::global().should_log(LogCategory::CPU, LogLevel::Trace) {
            // Only log if we're in the boot sector region or low memory (not ROM)
            // BUT: Always log F000 and FFFF segments to see BIOS execution
            let in_bios = cs == 0xF000 || cs == 0xFFFF;
            if physical_addr < 0xF0000 || in_bios {
                // Extra logging for suspicious addresses
                if physical_addr < 0x100 || (0x7D70..=0x7D80).contains(&physical_addr) || in_bios {
                    eprintln!(
                        "[PC] {:04X}:{:04X} -> {:08X} opcode={:02X} SP={:04X}",
                        cs, ip, physical_addr, opcode, self.cpu.sp
                    );
                } else {
                    eprintln!(
                        "[PC] {:04X}:{:04X} -> {:08X} opcode={:02X}",
                        cs, ip, physical_addr, opcode
                    );
                }
            }
        }

        // Handle I/O instructions by intercepting them before execution
        match opcode {
            // IN AL, imm8 (0xE4)
            0xE4 => {
                let port = self.cpu.memory.read(physical_addr + 1) as u16;
                let val = self.cpu.memory.io_read(port);
                self.cpu.ax = (self.cpu.ax & 0xFF00) | (val as u32);
                self.cpu.ip = self.cpu.ip.wrapping_add(2);
                return 10;
            }
            // IN AX, imm8 (0xE5)
            0xE5 => {
                let port = self.cpu.memory.read(physical_addr + 1) as u16;
                let val = self.cpu.memory.io_read(port);
                let val_high = self.cpu.memory.io_read(port.wrapping_add(1));
                self.cpu.ax = (val as u32) | ((val_high as u32) << 8);
                self.cpu.ip = self.cpu.ip.wrapping_add(2);
                return 10;
            }
            // OUT imm8, AL (0xE6)
            0xE6 => {
                let port = self.cpu.memory.read(physical_addr + 1) as u16;
                let val = (self.cpu.ax & 0xFF) as u8;
                self.cpu.memory.io_write(port, val);
                self.cpu.ip = self.cpu.ip.wrapping_add(2);
                return 10;
            }
            // OUT imm8, AX (0xE7)
            0xE7 => {
                let port = self.cpu.memory.read(physical_addr + 1) as u16;
                let val_low = (self.cpu.ax & 0xFF) as u8;
                let val_high = ((self.cpu.ax >> 8) & 0xFF) as u8;
                self.cpu.memory.io_write(port, val_low);
                self.cpu.memory.io_write(port.wrapping_add(1), val_high);
                self.cpu.ip = self.cpu.ip.wrapping_add(2);
                return 10;
            }
            // IN AL, DX (0xEC)
            0xEC => {
                let port = self.cpu.dx as u16;
                let val = self.cpu.memory.io_read(port);
                self.cpu.ax = (self.cpu.ax & 0xFF00) | (val as u32);
                self.cpu.ip = self.cpu.ip.wrapping_add(1);
                return 8;
            }
            // IN AX, DX (0xED)
            0xED => {
                let port = self.cpu.dx as u16;
                let val = self.cpu.memory.io_read(port);
                let val_high = self.cpu.memory.io_read(port.wrapping_add(1));
                self.cpu.ax = (val as u32) | ((val_high as u32) << 8);
                self.cpu.ip = self.cpu.ip.wrapping_add(1);
                return 8;
            }
            // OUT DX, AL (0xEE)
            0xEE => {
                let port = self.cpu.dx as u16;
                let val = (self.cpu.ax & 0xFF) as u8;
                self.cpu.memory.io_write(port, val);
                self.cpu.ip = self.cpu.ip.wrapping_add(1);
                return 8;
            }
            // OUT DX, AX (0xEF)
            0xEF => {
                let port = self.cpu.dx as u16;
                let val_low = (self.cpu.ax & 0xFF) as u8;
                let val_high = ((self.cpu.ax >> 8) & 0xFF) as u8;
                self.cpu.memory.io_write(port, val_low);
                self.cpu.memory.io_write(port.wrapping_add(1), val_high);
                self.cpu.ip = self.cpu.ip.wrapping_add(1);
                return 8;
            }
            _ => {}
        }

        // Handle INT instructions
        // We intercept INTs and handle them in Rust, but we must properly simulate
        // the CPU's INT behavior: push FLAGS/CS/IP, clear IF/TF
        if opcode == 0xCD {
            // This is an INT instruction, check the interrupt number
            let int_num = self.cpu.memory.read(physical_addr + 1);

            // Log interrupts for debugging (skip VIDEO_INTERRUPT to reduce noise)
            // Some frequently-called interrupts are only logged at trace level
            let cs = self.cpu.cs;
            let ip = self.cpu.ip;
            let ah = ((self.cpu.ax >> 8) & 0xFF) as u8;

            // Determine appropriate log level for this interrupt
            let is_high_frequency = (int_num == 0x28 && ah == 0x02) // DOS idle
                || (int_num == 0x16 && ah == 0x01); // Keyboard check (non-blocking)

            if is_high_frequency {
                // Only log these at trace level to avoid spam
                if LogConfig::global().should_log(LogCategory::Interrupts, LogLevel::Trace) {
                    eprintln!(
                        "INT 0x{:02X} AH=0x{:02X} called from {:04X}:{:04X}",
                        int_num, ah, cs, ip
                    );
                }
            } else {
                // Log all other interrupts at debug level
                if LogConfig::global().should_log(LogCategory::Interrupts, LogLevel::Debug) {
                    eprintln!(
                        "INT 0x{:02X} AH=0x{:02X} called from {:04X}:{:04X}",
                        int_num, ah, cs, ip
                    );
                }
            }

            // Generic interrupt override check: if the OS/DOS has installed a custom
            // handler for this interrupt, let the CPU execute it normally instead of
            // intercepting it with our BIOS/emulator handler
            if self.is_interrupt_overridden(int_num) {
                // OS has installed a custom handler, let CPU execute it
                // Fall through to normal execution
            } else {
                // No OS handler installed, check if we have a BIOS/emulator handler
                match int_num {
                    0x05 => return self.handle_int05h(), // Print Screen / BOUND
                    0x08 => return self.handle_int08h(), // Timer tick
                    0x09 => return self.handle_int09h(), // Keyboard hardware interrupt
                    0x10 => return self.handle_int10h(), // Video BIOS
                    0x11 => return self.handle_int11h(), // Equipment list
                    0x12 => return self.handle_int12h(), // Get memory size
                    0x13 => return self.handle_int13h(), // Disk services
                    0x14 => return self.handle_int14h(), // Serial port services
                    0x15 => return self.handle_int15h(), // Extended services
                    0x16 => return self.handle_int16h(), // Keyboard services
                    0x17 => return self.handle_int17h(), // Printer services
                    0x18 => return self.handle_int18h(), // Cassette BASIC / Boot failure
                    0x19 => return self.handle_int19h(), // Bootstrap loader
                    0x1A => return self.handle_int1ah(), // Time/Date services
                    0x21 => return self.handle_int21h(), // DOS API (fallback for standalone programs)
                    0x28 => return self.handle_int28h(), // DOS idle
                    0x29 => return self.handle_int29h(), // Fast console output
                    0x2A => return self.handle_int2ah(), // Network Installation API
                    0x2F => return self.handle_int2fh(), // Multiplex
                    0x31 => return self.handle_int31h(), // DPMI
                    0x33 => return self.handle_int33h(), // Mouse driver
                    0x4A => return self.handle_int4ah(), // RTC Alarm
                    _ => {} // No BIOS handler, let CPU handle it normally
                }
            }
        }

        // Execute normally
        self.cpu.step()
    }

    /// Handle INT 10h - Video BIOS services
    #[allow(dead_code)] // Called dynamically based on interrupt number
    fn handle_int10h(&mut self) -> u32 {
        // Simulate INT instruction: push FLAGS, CS, IP and advance past INT opcode
        self.simulate_int_call();

        // Get function code from AH register
        let ah = ((self.cpu.ax >> 8) & 0xFF) as u8;

        let cycles = match ah {
            0x00 => self.int10h_set_video_mode(),
            0x01 => self.int10h_set_cursor_shape(),
            0x02 => self.int10h_set_cursor_position(),
            0x03 => self.int10h_get_cursor_position(),
            0x05 => self.int10h_select_active_page(),
            0x06 => self.int10h_scroll_up(),
            0x07 => self.int10h_scroll_down(),
            0x08 => self.int10h_read_char_attr(),
            0x09 => self.int10h_write_char_attr(),
            0x0A => self.int10h_write_char_only(),
            0x0B => self.int10h_set_color_palette(),
            0x0C => self.int10h_write_pixel(),
            0x0D => self.int10h_read_pixel(),
            0x0E => self.int10h_teletype_output(),
            0x0F => self.int10h_get_video_mode(),
            0x10 => self.int10h_palette_functions(),
            0x11 => self.int10h_character_generator(),
            0x12 => self.int10h_video_subsystem_config(),
            0x13 => self.int10h_write_string(),
            0x1A => self.int10h_display_combination(),
            0x1B => self.int10h_get_video_state(),
            0xEF => self.int10h_stub_vga_function(0xEF),
            0xFA => self.int10h_stub_vga_function(0xFA),
            _ => {
                // Unsupported function - log and return
                self.log_stub_interrupt(0x10, Some(ah), "Video BIOS (unsupported subfunction)");
                51 // Approximate INT instruction timing
            }
        };

        // Simulate IRET to return from interrupt
        self.simulate_iret();

        cycles
    }

    /// INT 10h, AH=00h: Set video mode
    #[allow(dead_code)] // Called from handle_int10h
    fn int10h_set_video_mode(&mut self) -> u32 {
        // AL contains the mode number
        // For now, we just acknowledge the mode change
        // Actual mode switching would be done via the video adapter
        // Common modes: 0x00-0x03 (text), 0x04-0x06 (CGA graphics), 0x0D-0x13 (VGA)
        51
    }

    /// INT 10h, AH=01h: Set cursor shape
    #[allow(dead_code)] // Called from handle_int10h
    fn int10h_set_cursor_shape(&mut self) -> u32 {
        // CH = start scan line, CL = end scan line
        // We don't render cursor, but acknowledge the call
        51
    }

    /// INT 10h, AH=02h: Set cursor position
    #[allow(dead_code)] // Called from handle_int10h
    fn int10h_set_cursor_position(&mut self) -> u32 {
        // BH = page number, DH = row, DL = column
        // Store cursor position in BIOS data area at 0x40:0x50 + (page * 2)
        let page = ((self.cpu.bx >> 8) & 0xFF) as u8;
        let row = ((self.cpu.dx >> 8) & 0xFF) as u8;
        let col = (self.cpu.dx & 0xFF) as u8;

        // BIOS data area cursor position storage
        let cursor_addr = 0x450 + (page as u32 * 2);
        self.cpu.memory.write(cursor_addr, col);
        self.cpu.memory.write(cursor_addr + 1, row);
        51
    }

    /// INT 10h, AH=03h: Get cursor position
    #[allow(dead_code)] // Called from handle_int10h
    fn int10h_get_cursor_position(&mut self) -> u32 {
        // BH = page number
        // Returns: DH = row, DL = column, CH/CL = cursor shape
        let page = ((self.cpu.bx >> 8) & 0xFF) as u8;

        // Read cursor position from BIOS data area
        let cursor_addr = 0x450 + (page as u32 * 2);
        let col = self.cpu.memory.read(cursor_addr);
        let row = self.cpu.memory.read(cursor_addr + 1);

        self.cpu.dx = ((row as u32) << 8) | (col as u32);
        self.cpu.cx = 0x0607u32; // Default cursor shape
        51
    }

    /// INT 10h, AH=06h: Scroll up window
    #[allow(dead_code)] // Called from handle_int10h
    fn int10h_scroll_up(&mut self) -> u32 {
        // AL = lines to scroll (0 = clear), BH = attribute for blank lines
        // CH,CL = row,col of upper left, DH,DL = row,col of lower right
        let lines = (self.cpu.ax & 0xFF) as u8;
        let attr = ((self.cpu.bx >> 8) & 0xFF) as u8;
        let top = ((self.cpu.cx >> 8) & 0xFF) as u32;
        let left = (self.cpu.cx & 0xFF) as u32;
        let bottom = ((self.cpu.dx >> 8) & 0xFF) as u32;
        let right = (self.cpu.dx & 0xFF) as u32;

        if lines == 0 {
            // Clear window
            for row in top..=bottom {
                for col in left..=right {
                    let offset = (row * 80 + col) * 2;
                    let video_addr = 0xB8000 + offset;
                    self.cpu.memory.write(video_addr, b' ');
                    self.cpu.memory.write(video_addr + 1, attr);
                }
            }
        } else {
            // Scroll up by N lines
            for row in top..=bottom {
                for col in left..=right {
                    let offset = (row * 80 + col) * 2;
                    let video_addr = 0xB8000 + offset;

                    if row + (lines as u32) <= bottom {
                        // Copy from below
                        let src_offset = ((row + (lines as u32)) * 80 + col) * 2;
                        let src_addr = 0xB8000 + src_offset;
                        let ch = self.cpu.memory.read(src_addr);
                        let at = self.cpu.memory.read(src_addr + 1);
                        self.cpu.memory.write(video_addr, ch);
                        self.cpu.memory.write(video_addr + 1, at);
                    } else {
                        // Fill with blanks
                        self.cpu.memory.write(video_addr, b' ');
                        self.cpu.memory.write(video_addr + 1, attr);
                    }
                }
            }
        }
        51
    }

    /// INT 10h, AH=07h: Scroll down window
    #[allow(dead_code)] // Called from handle_int10h
    fn int10h_scroll_down(&mut self) -> u32 {
        // AL = lines to scroll (0 = clear), BH = attribute for blank lines
        // CH,CL = row,col of upper left, DH,DL = row,col of lower right
        let lines = (self.cpu.ax & 0xFF) as u8;
        let attr = ((self.cpu.bx >> 8) & 0xFF) as u8;
        let top = ((self.cpu.cx >> 8) & 0xFF) as u32;
        let left = (self.cpu.cx & 0xFF) as u32;
        let bottom = ((self.cpu.dx >> 8) & 0xFF) as u32;
        let right = (self.cpu.dx & 0xFF) as u32;

        if lines == 0 {
            // Clear window
            for row in top..=bottom {
                for col in left..=right {
                    let offset = (row * 80 + col) * 2;
                    let video_addr = 0xB8000 + offset;
                    self.cpu.memory.write(video_addr, b' ');
                    self.cpu.memory.write(video_addr + 1, attr);
                }
            }
        } else {
            // Scroll down by N lines (iterate in reverse to avoid overwriting)
            for row in (top..=bottom).rev() {
                for col in left..=right {
                    let offset = (row * 80 + col) * 2;
                    let video_addr = 0xB8000 + offset;

                    if row >= top + (lines as u32) {
                        // Copy from above
                        let src_offset = ((row - (lines as u32)) * 80 + col) * 2;
                        let src_addr = 0xB8000 + src_offset;
                        let ch = self.cpu.memory.read(src_addr);
                        let at = self.cpu.memory.read(src_addr + 1);
                        self.cpu.memory.write(video_addr, ch);
                        self.cpu.memory.write(video_addr + 1, at);
                    } else {
                        // Fill with blanks
                        self.cpu.memory.write(video_addr, b' ');
                        self.cpu.memory.write(video_addr + 1, attr);
                    }
                }
            }
        }
        51
    }

    /// INT 10h, AH=08h: Read character and attribute at cursor
    #[allow(dead_code)] // Called from handle_int10h
    fn int10h_read_char_attr(&mut self) -> u32 {
        // BH = page number
        // Returns: AL = character, AH = attribute
        let page = ((self.cpu.bx >> 8) & 0xFF) as u8;

        // Get cursor position
        let cursor_addr = 0x450 + (page as u32 * 2);
        let col = self.cpu.memory.read(cursor_addr) as u32;
        let row = self.cpu.memory.read(cursor_addr + 1) as u32;

        // Calculate offset in text mode video memory (0xB8000)
        // Each character is 2 bytes: char + attribute
        let offset = (row * 80 + col) * 2;
        let video_addr = 0xB8000 + offset;

        let ch = self.cpu.memory.read(video_addr);
        let attr = self.cpu.memory.read(video_addr + 1);

        self.cpu.ax = ((attr as u32) << 8) | (ch as u32);
        51
    }

    /// INT 10h, AH=09h: Write character and attribute at cursor
    #[allow(dead_code)] // Called from handle_int10h
    fn int10h_write_char_attr(&mut self) -> u32 {
        // AL = character, BL = attribute, BH = page, CX = count
        let ch = (self.cpu.ax & 0xFF) as u8;
        let attr = (self.cpu.bx & 0xFF) as u8;
        let page = ((self.cpu.bx >> 8) & 0xFF) as u8;
        let count = self.cpu.cx;

        // Get cursor position
        let cursor_addr = 0x450 + (page as u32 * 2);
        let mut col = self.cpu.memory.read(cursor_addr) as u32;
        let row = self.cpu.memory.read(cursor_addr + 1) as u32;

        // Write character(s) to video memory
        for _ in 0..count {
            let offset = (row * 80 + col) * 2;
            let video_addr = 0xB8000 + offset;

            self.cpu.memory.write(video_addr, ch);
            self.cpu.memory.write(video_addr + 1, attr);

            col += 1;
            if col >= 80 {
                break; // Don't wrap to next line
            }
        }

        51
    }

    /// Helper function to scroll the entire screen up by N lines
    fn scroll_screen_up(&mut self, lines: u32, attr: u8) {
        // Scroll entire screen (0,0) to (24,79)
        for row in 0..25 {
            for col in 0..80 {
                let offset = (row * 80 + col) * 2;
                let video_addr = 0xB8000 + offset;

                if row + lines < 25 {
                    // Copy from below
                    let src_offset = ((row + lines) * 80 + col) * 2;
                    let src_addr = 0xB8000 + src_offset;
                    let ch = self.cpu.memory.read(src_addr);
                    let at = self.cpu.memory.read(src_addr + 1);
                    self.cpu.memory.write(video_addr, ch);
                    self.cpu.memory.write(video_addr + 1, at);
                } else {
                    // Fill bottom lines with blanks
                    self.cpu.memory.write(video_addr, b' ');
                    self.cpu.memory.write(video_addr + 1, attr);
                }
            }
        }
    }

    /// INT 10h, AH=0Eh: Teletype output
    #[allow(dead_code)] // Called from handle_int10h
    fn int10h_teletype_output(&mut self) -> u32 {
        // AL = character, BH = page number, BL = foreground color (graphics mode)
        let ch = (self.cpu.ax & 0xFF) as u8;
        let page = ((self.cpu.bx >> 8) & 0xFF) as u8;

        // Log printable characters
        if (0x20..0x7F).contains(&ch) {
            eprint!("{}", ch as char);
        } else if ch == 0x0D {
            eprintln!(); // Carriage return + line feed for stderr
        }

        // Get cursor position
        let cursor_addr = 0x450 + (page as u32 * 2);
        let mut col = self.cpu.memory.read(cursor_addr) as u32;
        let mut row = self.cpu.memory.read(cursor_addr + 1) as u32;

        // Handle special characters
        match ch {
            0x08 => {
                // Backspace - ONLY move cursor back, do NOT erase character
                // DOS/applications handle erasure by outputting: backspace, space, backspace
                if col > 0 {
                    col = col.saturating_sub(1);
                }
            }
            0x0A => {
                // Line feed
                row += 1;
                if row >= 25 {
                    // Scroll screen up by 1 line
                    self.scroll_screen_up(1, 0x07);
                    row = 24; // Stay at bottom after scroll
                }
            }
            0x0D => {
                // Carriage return
                col = 0;
            }
            _ => {
                // Normal character - write to video memory
                let offset = (row * 80 + col) * 2;
                let video_addr = 0xB8000 + offset;

                self.cpu.memory.write(video_addr, ch);
                // Use default attribute (white on black)
                self.cpu.memory.write(video_addr + 1, 0x07);

                col += 1;
                if col >= 80 {
                    col = 0;
                    row += 1;
                    if row >= 25 {
                        // Scroll screen up by 1 line
                        self.scroll_screen_up(1, 0x07);
                        row = 24; // Stay at bottom after scroll
                    }
                }
            }
        }

        // Update cursor position
        self.cpu.memory.write(cursor_addr, col as u8);
        self.cpu.memory.write(cursor_addr + 1, row as u8);

        51
    }

    /// INT 10h, AH=0Fh: Get video mode
    #[allow(dead_code)] // Called from handle_int10h
    fn int10h_get_video_mode(&mut self) -> u32 {
        // Returns: AL = mode, AH = columns, BH = page
        // Default to mode 3 (80x25 color text)
        self.cpu.ax = 0x5003u32; // AH=80 columns, AL=mode 3
        self.cpu.bx &= 0x00FF; // BH=0 (page 0)
        51
    }

    /// INT 10h, AH=13h: Write string
    #[allow(dead_code)] // Called from handle_int10h
    fn int10h_write_string(&mut self) -> u32 {
        // AL = write mode, BH = page, BL = attribute
        // CX = string length, DH/DL = row/column
        // ES:BP = pointer to string
        let mode = (self.cpu.ax & 0xFF) as u8;
        let page = ((self.cpu.bx >> 8) & 0xFF) as u8;
        let attr = (self.cpu.bx & 0xFF) as u8;
        let length = self.cpu.cx;
        let row = ((self.cpu.dx >> 8) & 0xFF) as u32;
        let mut col = (self.cpu.dx & 0xFF) as u32;

        // String address: ES:BP
        let string_seg = self.cpu.es as u32;
        let string_off = self.cpu.bp as u32;
        let string_addr = (string_seg << 4) + string_off;

        // Write string to video memory
        for i in 0..length {
            let ch = self.cpu.memory.read(string_addr + i as u32);

            let offset = (row * 80 + col) * 2;
            let video_addr = 0xB8000 + offset;

            self.cpu.memory.write(video_addr, ch);
            self.cpu.memory.write(video_addr + 1, attr);

            col += 1;
            if col >= 80 {
                break;
            }
        }

        // Update cursor position if mode bit 1 is set
        if mode & 0x02 != 0 {
            let cursor_addr = 0x450 + (page as u32 * 2);
            self.cpu.memory.write(cursor_addr, col as u8);
            self.cpu.memory.write(cursor_addr + 1, row as u8);
        }

        51
    }

    /// INT 10h, AH=05h: Select active video page
    #[allow(dead_code)] // Called from handle_int10h
    fn int10h_select_active_page(&mut self) -> u32 {
        // AL = new page number (0..7 for text mode)
        // Store active page in BIOS data area at 0x40:0x62
        let page = (self.cpu.ax & 0xFF) as u8;
        self.cpu.memory.write(0x462, page & 0x07); // Limit to 8 pages
        51
    }

    /// INT 10h, AH=0Ah: Write character only at cursor
    #[allow(dead_code)] // Called from handle_int10h
    fn int10h_write_char_only(&mut self) -> u32 {
        // AL = character, BH = page, CX = count
        // Note: Does NOT write attribute, does NOT advance cursor
        let ch = (self.cpu.ax & 0xFF) as u8;
        let page = ((self.cpu.bx >> 8) & 0xFF) as u8;
        let count = self.cpu.cx;

        // Get cursor position
        let cursor_addr = 0x450 + (page as u32 * 2);
        let mut col = self.cpu.memory.read(cursor_addr) as u32;
        let row = self.cpu.memory.read(cursor_addr + 1) as u32;

        // Write character(s) to video memory (character bytes only)
        for _ in 0..count {
            let offset = (row * 80 + col) * 2;
            let video_addr = 0xB8000 + offset;

            // Write character only, preserve existing attribute
            self.cpu.memory.write(video_addr, ch);

            col += 1;
            if col >= 80 {
                break; // Don't wrap to next line
            }
        }

        51
    }

    /// INT 10h, AH=0Bh: Set color palette (CGA)
    /// This function controls the CGA color palette and border/background color
    #[allow(dead_code)] // Called from handle_int10h
    fn int10h_set_color_palette(&mut self) -> u32 {
        let bh = ((self.cpu.bx >> 8) & 0xFF) as u8;
        let bl = (self.cpu.bx & 0xFF) as u8;

        match bh {
            0x00 => {
                // Set background/border color (text mode)
                // BL = color (0-15)
                // In text mode, this sets the border color
                // We acknowledge but don't implement actual border color change
                emu_core::logging::log(LogCategory::Interrupts, LogLevel::Debug, || {
                    format!(
                        "INT 10h AH=0Bh BH=00h: Set background/border color to 0x{:02X}",
                        bl
                    )
                });
            }
            0x01 => {
                // Set palette (graphics mode)
                // BL = palette ID (0 or 1 for CGA 4-color modes)
                // We acknowledge but don't implement actual palette switching
                emu_core::logging::log(LogCategory::Interrupts, LogLevel::Debug, || {
                    format!("INT 10h AH=0Bh BH=01h: Set CGA palette to 0x{:02X}", bl)
                });
            }
            _ => {
                // Unknown subfunction
                emu_core::logging::log(LogCategory::Interrupts, LogLevel::Debug, || {
                    format!("INT 10h AH=0Bh: Unknown BH=0x{:02X}, BL=0x{:02X}", bh, bl)
                });
            }
        }

        51
    }

    /// INT 10h, AH=0Ch: Write pixel (graphics mode)
    #[allow(dead_code)] // Called from handle_int10h
    fn int10h_write_pixel(&mut self) -> u32 {
        // AL = pixel color, CX = column, DX = row
        // BH = page number (graphics mode)
        let color = (self.cpu.ax & 0xFF) as u8;
        let x = self.cpu.cx as u32;
        let y = self.cpu.dx as u32;
        let _page = ((self.cpu.bx >> 8) & 0xFF) as u8;

        // Mode 13h (320x200 256-color): Linear addressing
        // Each pixel is 1 byte at 0xA0000 + (y * 320 + x)
        // For other modes, this would require mode-specific calculations
        if x < 320 && y < 200 {
            let offset = y * 320 + x;
            let video_addr = 0xA0000 + offset;
            self.cpu.memory.write(video_addr, color);
        }

        51
    }

    /// INT 10h, AH=0Dh: Read pixel (graphics mode)
    #[allow(dead_code)] // Called from handle_int10h
    fn int10h_read_pixel(&mut self) -> u32 {
        // CX = column, DX = row, BH = page number
        // Returns: AL = pixel color
        let x = self.cpu.cx as u32;
        let y = self.cpu.dx as u32;
        let _page = ((self.cpu.bx >> 8) & 0xFF) as u8;

        // Mode 13h (320x200 256-color): Linear addressing
        let color = if x < 320 && y < 200 {
            let offset = y * 320 + x;
            let video_addr = 0xA0000 + offset;
            self.cpu.memory.read(video_addr)
        } else {
            0
        };

        // Return color in AL
        self.cpu.ax = (self.cpu.ax & 0xFF00) | (color as u32);
        51
    }

    /// INT 10h, AH=10h: Palette/DAC functions
    #[allow(dead_code)] // Called from handle_int10h
    fn int10h_palette_functions(&mut self) -> u32 {
        // AL = subfunction
        // 00h = Set individual palette register
        // 01h = Set overscan register
        // 02h = Set all palette registers
        // 03h = Toggle intensity/blinking
        // 10h-13h = DAC color registers
        let al = (self.cpu.ax & 0xFF) as u8;

        match al {
            0x03 => {
                // Toggle intensity/blinking
                // BL = 0: enable intensive colors
                // BL = 1: enable blinking
                // For now, just acknowledge
                51
            }
            _ => {
                // Other palette functions - stub
                51
            }
        }
    }

    /// INT 10h, AH=11h: Character generator functions
    #[allow(dead_code)] // Called from handle_int10h
    fn int10h_character_generator(&mut self) -> u32 {
        // AL = subfunction
        // 00h-04h = Load font
        // 10h-14h = Load font and program
        // 20h-24h = Load font and set block
        // 30h = Get font information
        // For now, just acknowledge
        51
    }

    /// INT 10h, AH=12h: Video subsystem configuration
    #[allow(dead_code)] // Called from handle_int10h
    fn int10h_video_subsystem_config(&mut self) -> u32 {
        // BL = subfunction
        // 10h = Get EGA info
        // 20h = Select alternate print screen
        // 30h = Select scan lines for text modes
        // 31h = Enable/disable default palette loading
        // 32h = Enable/disable video addressing
        // 33h = Enable/disable gray-scale summing
        // 34h = Enable/disable cursor emulation
        // 36h = Enable/disable video refresh
        // For now, just acknowledge
        51
    }

    /// INT 10h, AH=1Ah: Display combination code
    #[allow(dead_code)] // Called from handle_int10h
    fn int10h_display_combination(&mut self) -> u32 {
        // AL = subfunction
        // 00h = Get display combination code
        // 01h = Set display combination code
        let al = (self.cpu.ax & 0xFF) as u8;

        match al {
            0x00 => {
                // Get display combination code
                // Return: AL = 1Ah (function supported)
                //         BL = active display code
                //         BH = alternate display code
                self.cpu.ax = (self.cpu.ax & 0xFF00) | 0x1A;
                self.cpu.bx = 0x0008u32; // VGA with color display
                51
            }
            _ => {
                // Set display combination - stub
                51
            }
        }
    }

    /// INT 10h, AH=1Bh: Get video state (VGA BIOS extension)
    /// Returns a pointer to a video state table
    #[allow(dead_code)] // Called from handle_int10h
    fn int10h_get_video_state(&mut self) -> u32 {
        // ES:DI = pointer to video state table
        // The table should contain video state information
        // For simplicity, we return a stub table with basic info

        // Return AL = 0x1B (function supported)
        self.cpu.ax = (self.cpu.ax & 0xFF00) | 0x1B;

        // Set ES:DI to point to a placeholder in BIOS data area
        // We'll use a location in conventional RAM that won't interfere
        self.cpu.es = 0x0040; // BIOS data area
        self.cpu.di = 0x0100u32; // Offset within BIOS data area

        emu_core::logging::log(LogCategory::Interrupts, LogLevel::Debug, || {
            "INT 10h AH=1Bh: Get video state (returning stub table)".to_string()
        });

        51
    }

    /// INT 10h, AH=EFh or FAh: Undocumented VGA functions
    /// These are used by QBasic and some other applications
    #[allow(dead_code)] // Called from handle_int10h
    fn int10h_stub_vga_function(&mut self, ah: u8) -> u32 {
        // Just acknowledge and return
        emu_core::logging::log(LogCategory::Interrupts, LogLevel::Debug, || {
            format!("INT 10h AH=0x{:02X}: Undocumented VGA function (stub)", ah)
        });

        51
    }

    /// Handle INT 16h - Keyboard BIOS services
    #[allow(dead_code)] // Called dynamically based on interrupt number
    fn handle_int16h(&mut self) -> u32 {
        // Skip the INT 16h instruction (2 bytes: 0xCD 0x16)
        // We intercept before CPU executes it, so just advance IP past it
        self.cpu.ip = self.cpu.ip.wrapping_add(2);

        // Get function code from AH register
        let ah = ((self.cpu.ax >> 8) & 0xFF) as u8;

        match ah {
            0x00 => self.int16h_read_keystroke(),
            0x01 => self.int16h_check_keystroke(),
            0x02 => self.int16h_get_shift_flags(),
            _ => {
                // Unsupported function
                51
            }
        }
    }

    /// INT 16h, AH=00h: Read keystroke (blocking)
    fn int16h_read_keystroke(&mut self) -> u32 {
        // Returns: AH = scan code, AL = ASCII character
        // Note: In a real BIOS, this would block until a key is available
        // We emulate blocking by halting the CPU until keyboard input arrives

        // Check for available keystroke (only make codes are in the buffer now)
        if self.cpu.memory.keyboard.has_data() {
            let scancode = self.cpu.memory.keyboard.read_scancode();

            // Convert scancode to ASCII
            let ascii = self.scancode_to_ascii(scancode);

            // Log at trace level for debugging
            if LogConfig::global().should_log(LogCategory::Interrupts, LogLevel::Trace) {
                eprintln!(
                    "INT 16h AH=00h: Read scancode=0x{:02X} ascii=0x{:02X} '{}'",
                    scancode,
                    ascii,
                    if (0x20..0x7F).contains(&ascii) {
                        ascii as char
                    } else {
                        '?'
                    }
                );
            }

            // AH = scan code, AL = ASCII character
            self.cpu.ax = ((scancode as u32) << 8) | (ascii as u32);

            // Ensure CPU is not halted when we return a key
            self.cpu.set_halted(false);
            return 51;
        }

        // No keys in buffer - halt CPU to wait for input
        // This emulates the blocking behavior of INT 16h AH=00h
        // The CPU will remain halted until keyboard input arrives and unhalts it
        self.cpu.set_halted(true);
        self.cpu.ax = 0x0000u32;
        51
    }

    /// INT 16h, AH=01h: Check for keystroke (non-blocking)
    fn int16h_check_keystroke(&mut self) -> u32 {
        // Returns: ZF = 1 if no key available, ZF = 0 if key available
        // If key available: AH = scan code, AL = ASCII character

        // Look for the first make code in the buffer (skip any break codes)
        if let Some(scancode) = self.cpu.memory.keyboard.peek_make_code() {
            let ascii = self.scancode_to_ascii(scancode);

            // Set ZF = 0 (key available)
            self.set_zero_flag(false);

            // AH = scan code, AL = ASCII character
            self.cpu.ax = ((scancode as u32) << 8) | (ascii as u32);
        } else {
            // No make code available
            self.set_zero_flag(true); // ZF = 1 (no key)
            self.cpu.ax = 0x0000u32;
        }

        51
    }

    /// INT 16h, AH=02h: Get shift flags
    fn int16h_get_shift_flags(&mut self) -> u32 {
        // Returns: AL = shift flags
        // Bit 0 = right shift, Bit 1 = left shift
        // Bit 2 = Ctrl, Bit 3 = Alt
        // Bit 4 = Scroll Lock, Bit 5 = Num Lock, Bit 6 = Caps Lock, Bit 7 = Insert
        let flags = self.cpu.memory.keyboard.get_shift_flags();
        self.cpu.ax = (self.cpu.ax & 0xFF00) | (flags as u32);
        51
    }

    /// Handle INT 20h - DOS: Program terminate
    #[allow(dead_code)] // Called dynamically based on interrupt number
    fn handle_int20h(&mut self) -> u32 {
        // Skip the INT 20h instruction (2 bytes: 0xCD 0x20)
        self.cpu.ip = self.cpu.ip.wrapping_add(2);

        // Program terminate - for now, just halt
        // In a real DOS environment, this would return to COMMAND.COM
        // We could set a flag here to indicate program termination
        51
    }

    /// Handle INT 21h - DOS API
    #[allow(dead_code)] // Called dynamically based on interrupt number
    fn handle_int21h(&mut self) -> u32 {
        // Skip the INT 21h instruction (2 bytes: 0xCD 0x21)
        // We intercept before CPU executes it, so just advance IP past it
        self.cpu.ip = self.cpu.ip.wrapping_add(2);

        // Get function code from AH register
        let ah = ((self.cpu.ax >> 8) & 0xFF) as u8;

        match ah {
            0x00 => self.int21h_terminate(),            // Program terminate
            0x01 => self.int21h_read_char_stdin(),      // Read character from stdin
            0x02 => self.int21h_write_char_stdout(),    // Write character to stdout
            0x06 => self.int21h_direct_console_io(),    // Direct console I/O
            0x07 => self.int21h_direct_stdin(),         // Direct stdin input (no echo)
            0x08 => self.int21h_stdin_no_echo(),        // Read stdin without echo
            0x09 => self.int21h_write_string(),         // Write string to stdout
            0x0A => self.int21h_buffered_input(),       // Buffered input
            0x0B => self.int21h_check_stdin(),          // Check stdin status
            0x25 => self.int21h_set_interrupt_vector(), // Set interrupt vector
            0x35 => self.int21h_get_interrupt_vector(), // Get interrupt vector
            0x3C => self.int21h_create_file(),          // Create or truncate file
            0x3D => self.int21h_open_file(),            // Open existing file
            0x3E => self.int21h_close_file(),           // Close file handle
            0x3F => self.int21h_read_file(),            // Read from file or device
            0x40 => self.int21h_write_file(),           // Write to file or device
            0x4C => self.int21h_terminate_with_code(),  // Terminate with return code
            _ => {
                // Unsupported function - log and return
                self.log_stub_interrupt(0x21, Some(ah), "DOS API (unsupported subfunction)");
                51
            }
        }
    }

    /// INT 21h, AH=00h: Program terminate
    #[allow(dead_code)] // Called from handle_int21h
    fn int21h_terminate(&mut self) -> u32 {
        // Same as INT 20h
        51
    }

    /// INT 21h, AH=01h: Read character from stdin with echo
    #[allow(dead_code)] // Called from handle_int21h
    fn int21h_read_char_stdin(&mut self) -> u32 {
        // Returns: AL = character read
        // This function should block until a character is available
        // Use INT 16h AH=00h to get keyboard input

        // Save current AX
        let saved_ax = self.cpu.ax;

        // Call INT 16h AH=00h (read keystroke) to get character
        self.cpu.ax = 0x0000u32; // AH=00h
        self.int16h_read_keystroke();

        // Get the ASCII character from AL (INT 16h returns scancode in AH, ASCII in AL)
        let ascii = (self.cpu.ax & 0xFF) as u8;

        // Restore AH, keep AL with the character
        self.cpu.ax = (saved_ax & 0xFF00) | (ascii as u32);

        51
    }

    /// INT 21h, AH=02h: Write character to stdout
    #[allow(dead_code)] // Called from handle_int21h
    fn int21h_write_char_stdout(&mut self) -> u32 {
        // DL = character to write
        let ch = (self.cpu.dx & 0xFF) as u8;

        // Use INT 10h teletype output to display character
        // Save current AX
        let saved_ax = self.cpu.ax;
        self.cpu.ax = (self.cpu.ax & 0xFF00) | (ch as u32);
        self.int10h_teletype_output();
        self.cpu.ax = saved_ax;

        51
    }

    /// INT 21h, AH=06h: Direct console I/O
    #[allow(dead_code)] // Called from handle_int21h
    fn int21h_direct_console_io(&mut self) -> u32 {
        // DL = 0xFF: read character, else: write character
        let dl = (self.cpu.dx & 0xFF) as u8;

        if dl == 0xFF {
            // Read character - use INT 16h AH=01h to check for keystroke
            let saved_ax = self.cpu.ax;
            self.cpu.ax = 0x0100u32; // AH=01h (check keystroke)
            self.int16h_check_keystroke();

            // Check zero flag to see if key is available
            let key_available = (self.cpu.flags & 0x0040) == 0; // ZF=0 means key available

            if key_available {
                // Key is available - read it with INT 16h AH=00h
                self.cpu.ax = 0x0000u32; // AH=00h (read keystroke)
                self.int16h_read_keystroke();

                // Get ASCII character from AL
                let ascii = (self.cpu.ax & 0xFF) as u8;

                // Restore AH, keep AL with the character, set ZF=0
                self.cpu.ax = (saved_ax & 0xFF00) | (ascii as u32);
                self.set_zero_flag(false);
            } else {
                // No key available - return 0 and set ZF=1
                self.cpu.ax = (saved_ax & 0xFF00) as u32;
                self.set_zero_flag(true);
            }
        } else {
            // Write character
            let saved_ax = self.cpu.ax;
            self.cpu.ax = (self.cpu.ax & 0xFF00) | (dl as u32);
            self.int10h_teletype_output();
            self.cpu.ax = saved_ax;
        }

        51
    }

    /// INT 21h, AH=07h: Direct stdin input without echo
    #[allow(dead_code)] // Called from handle_int21h
    fn int21h_direct_stdin(&mut self) -> u32 {
        // Returns: AL = character read (no echo)
        // This function should block until a character is available
        // Use INT 16h AH=00h to get keyboard input

        // Save current AX
        let saved_ax = self.cpu.ax;

        // Call INT 16h AH=00h (read keystroke) to get character
        self.cpu.ax = 0x0000u32; // AH=00h
        self.int16h_read_keystroke();

        // Get the ASCII character from AL
        let ascii = (self.cpu.ax & 0xFF) as u8;

        // Restore AH, keep AL with the character
        self.cpu.ax = (saved_ax & 0xFF00) | (ascii as u32);

        51
    }

    /// INT 21h, AH=08h: Read stdin without echo
    #[allow(dead_code)] // Called from handle_int21h
    fn int21h_stdin_no_echo(&mut self) -> u32 {
        // Returns: AL = character read (no echo)
        // This function should block until a character is available
        // Use INT 16h AH=00h to get keyboard input

        // Save current AX
        let saved_ax = self.cpu.ax;

        // Call INT 16h AH=00h (read keystroke) to get character
        self.cpu.ax = 0x0000u32; // AH=00h
        self.int16h_read_keystroke();

        // Get the ASCII character from AL
        let ascii = (self.cpu.ax & 0xFF) as u8;

        // Restore AH, keep AL with the character
        self.cpu.ax = (saved_ax & 0xFF00) | (ascii as u32);

        51
    }

    /// INT 21h, AH=09h: Write string to stdout
    #[allow(dead_code)] // Called from handle_int21h
    fn int21h_write_string(&mut self) -> u32 {
        // DS:DX = pointer to string (terminated by '$')
        let ds = self.cpu.ds as u32;
        let dx = self.cpu.dx as u32;
        let string_addr = (ds << 4) + dx;

        // Read and display characters until '$'
        let mut offset = 0;
        loop {
            let ch = self.cpu.memory.read(string_addr + offset);
            if ch == b'$' {
                break;
            }

            // Use INT 10h teletype output
            let saved_ax = self.cpu.ax;
            self.cpu.ax = (self.cpu.ax & 0xFF00) | (ch as u32);
            self.int10h_teletype_output();
            self.cpu.ax = saved_ax;

            offset += 1;
            if offset > 1000 {
                // Safety limit to prevent infinite loops
                break;
            }
        }

        51
    }

    /// INT 21h, AH=0Ah: Buffered input
    #[allow(dead_code)] // Called from handle_int21h
    fn int21h_buffered_input(&mut self) -> u32 {
        // DS:DX = pointer to input buffer
        // Buffer format: [max_length, actual_length, ...characters...]
        // For now, just return empty input
        let ds = self.cpu.ds as u32;
        let dx = self.cpu.dx as u32;
        let buffer_addr = (ds << 4) + dx;

        // Set actual length to 0
        self.cpu.memory.write(buffer_addr + 1, 0);

        51
    }

    /// INT 21h, AH=0Bh: Check stdin status
    #[allow(dead_code)] // Called from handle_int21h
    fn int21h_check_stdin(&mut self) -> u32 {
        // Returns: AL = 0xFF if character available, 0x00 if not
        // Use INT 16h AH=01h to check for keystroke

        // Save current AX
        let saved_ax = self.cpu.ax;

        // Call INT 16h AH=01h (check keystroke)
        self.cpu.ax = 0x0100u32; // AH=01h
        self.int16h_check_keystroke();

        // Check zero flag to see if key is available
        let key_available = (self.cpu.flags & 0x0040) == 0; // ZF=0 means key available

        // Set AL based on availability
        if key_available {
            self.cpu.ax = (saved_ax & 0xFF00) | 0xFF; // AL = 0xFF (character available)
        } else {
            self.cpu.ax = (saved_ax & 0xFF00) as u32; // AL = 0x00 (no character)
        }

        51
    }

    /// INT 21h, AH=25h: Set interrupt vector
    #[allow(dead_code)] // Called from handle_int21h
    fn int21h_set_interrupt_vector(&mut self) -> u32 {
        // AL = interrupt number, DS:DX = new vector
        // For now, just acknowledge (interrupt vectors not fully emulated)
        51
    }

    /// INT 21h, AH=35h: Get interrupt vector
    #[allow(dead_code)] // Called from handle_int21h
    fn int21h_get_interrupt_vector(&mut self) -> u32 {
        // AL = interrupt number
        // Returns: ES:BX = interrupt vector
        // For now, return a dummy value
        self.cpu.es = 0x0000;
        self.cpu.bx = 0x0000u32;
        51
    }

    /// INT 21h, AH=4Ch: Terminate with return code
    #[allow(dead_code)] // Called from handle_int21h
    fn int21h_terminate_with_code(&mut self) -> u32 {
        // AL = return code
        // For now, just halt like INT 20h
        51
    }

    /// INT 21h, AH=3Ch: Create or truncate file
    #[allow(dead_code)] // Called from handle_int21h
    fn int21h_create_file(&mut self) -> u32 {
        // DS:DX = pointer to ASCIIZ filename
        // CX = file attributes
        // Returns: CF clear if success, AX = file handle
        //          CF set if error, AX = error code (03h = path not found, 04h = no handles, 05h = access denied)

        // For now, return "path not found" error
        // In a real implementation, we would create the file on the mounted disk
        self.cpu.ax = (self.cpu.ax & 0xFF00) | 0x03; // Path not found
        self.set_carry_flag(true);
        51
    }

    /// INT 21h, AH=3Dh: Open existing file
    #[allow(dead_code)] // Called from handle_int21h
    fn int21h_open_file(&mut self) -> u32 {
        // DS:DX = pointer to ASCIIZ filename
        // AL = access mode (0 = read, 1 = write, 2 = read/write)
        // Returns: CF clear if success, AX = file handle
        //          CF set if error, AX = error code (02h = file not found, 03h = path not found, 04h = no handles, 05h = access denied, 0Ch = invalid access)

        // Read the filename from memory
        let ds = self.cpu.ds;
        let dx = self.cpu.dx;
        let mut filename = String::new();
        let mut offset = 0u16;
        loop {
            let addr = ((ds as u32) << 4) + (dx.wrapping_add(offset as u32));
            let byte = self.cpu.memory.read(addr);
            if byte == 0 {
                break;
            }
            filename.push(byte as char);
            offset = offset.wrapping_add(1);
            if offset > 255 {
                break; // Safety limit
            }
        }

        emu_core::logging::log(LogCategory::Interrupts, LogLevel::Debug, || {
            format!("INT 0x21 AH=0x3D: Attempting to open file: '{}'", filename)
        });

        // Check if this is a DOS device name (case-insensitive)
        // Standard DOS devices: CON, PRN, AUX, NUL, COM1-4, LPT1-3, CLOCK$
        let filename_upper = filename.to_uppercase();
        let device_name = filename_upper
            .split(':')
            .next()
            .unwrap_or(&filename_upper)
            .trim();

        match device_name {
            "CON" => {
                // Console device - bidirectional (read and write)
                // Access mode: 0=read, 1=write, 2=read/write
                let access_mode = (self.cpu.ax & 0xFF) as u8;
                let handle = match access_mode {
                    0 => 0, // Read mode - stdin
                    1 => 1, // Write mode - stdout
                    2 => 1, // Read/Write mode - stdout (CON is bidirectional, use stdout as primary)
                    _ => 1, // Default to stdout
                };
                self.cpu.ax = handle;
                self.set_carry_flag(false); // Success

                emu_core::logging::log(LogCategory::Interrupts, LogLevel::Debug, || {
                    format!(
                        "INT 0x21 AH=0x3D: Opened CON device, returning handle {} (access mode {})",
                        handle, access_mode
                    )
                });
            }
            "NUL" => {
                // Null device - all reads return EOF, all writes are discarded
                // Use handle 3 (stdaux) as a dummy handle for null device
                // Note: In this fallback implementation, handle 3 serves multiple purposes
                // Real DOS would use internal device driver chain for proper device handling
                self.cpu.ax = 3;
                self.set_carry_flag(false); // Success

                emu_core::logging::log(LogCategory::Interrupts, LogLevel::Debug, || {
                    "INT 0x21 AH=0x3D: Opened NUL device, returning handle 3 (dummy)".to_string()
                });
            }
            "PRN" | "LPT1" | "LPT2" | "LPT3" => {
                // Printer device - use handle 4 (stdprn)
                self.cpu.ax = 4;
                self.set_carry_flag(false); // Success

                emu_core::logging::log(LogCategory::Interrupts, LogLevel::Debug, || {
                    format!(
                        "INT 0x21 AH=0x3D: Opened printer device {}, returning handle 4",
                        device_name
                    )
                });
            }
            "AUX" | "COM1" | "COM2" | "COM3" | "COM4" => {
                // Serial/Auxiliary device - use handle 3 (stdaux)
                // Note: All serial devices map to the same handle in this simple fallback
                // Real DOS would use internal device driver chain for proper device handling
                self.cpu.ax = 3;
                self.set_carry_flag(false); // Success

                emu_core::logging::log(LogCategory::Interrupts, LogLevel::Debug, || {
                    format!(
                        "INT 0x21 AH=0x3D: Opened serial device {}, returning handle 3",
                        device_name
                    )
                });
            }
            "CLOCK$" => {
                // Clock device (special) - treat as NUL for simplicity
                // Maps to handle 3 in this fallback implementation
                self.cpu.ax = 3;
                self.set_carry_flag(false); // Success

                emu_core::logging::log(LogCategory::Interrupts, LogLevel::Debug, || {
                    "INT 0x21 AH=0x3D: Opened CLOCK$ device, returning handle 3 (dummy)".to_string()
                });
            }
            _ => {
                // Not a recognized device - treat as file (not supported)
                // Return "file not found" error
                self.cpu.ax = (self.cpu.ax & 0xFF00) | 0x02; // File not found
                self.set_carry_flag(true);

                emu_core::logging::log(LogCategory::Interrupts, LogLevel::Debug, || {
                    format!("INT 0x21 AH=0x3D: File '{}' not found (not a device, file I/O not supported)", filename)
                });
            }
        }

        51
    }

    /// INT 21h, AH=3Eh: Close file handle
    #[allow(dead_code)] // Called from handle_int21h
    fn int21h_close_file(&mut self) -> u32 {
        // BX = file handle
        // Returns: CF clear if success
        //          CF set if error, AX = error code (06h = invalid handle)

        let handle = self.cpu.bx;

        // Standard handles (0-4) cannot be closed
        // File handles >= 5 are user files, but not supported yet
        if handle >= 5 {
            // Return "invalid handle" error since we don't support file I/O
            self.cpu.ax = DOS_ERROR_INVALID_HANDLE as u32;
            self.set_carry_flag(true);
        } else {
            // Standard handles: succeed but do nothing (can't close stdin/stdout/stderr)
            self.set_carry_flag(false);
        }
        51
    }

    /// INT 21h, AH=3Fh: Read from file or device
    #[allow(dead_code)] // Called from handle_int21h
    fn int21h_read_file(&mut self) -> u32 {
        // BX = file handle
        // CX = number of bytes to read
        // DS:DX = pointer to buffer
        // Returns: CF clear if success, AX = number of bytes read
        //          CF set if error, AX = error code (05h = access denied, 06h = invalid handle)

        let handle = self.cpu.bx;

        // Standard DOS file handles:
        // 0 = stdin, 1 = stdout, 2 = stderr, 3 = stdaux, 4 = stdprn
        // Handles >= 5 are user-opened files

        if handle >= 5 {
            // File handles >= 5 are not supported (no file system implementation yet)
            // Return "invalid handle" error
            self.cpu.ax = DOS_ERROR_INVALID_HANDLE as u32;
            self.set_carry_flag(true);
        } else {
            // Standard handles: return 0 bytes read (EOF)
            // This is correct behavior for stdin when no input is available
            self.cpu.ax = 0x0000u32; // 0 bytes read
            self.set_carry_flag(false);
        }
        51
    }

    /// INT 21h, AH=40h: Write to file or device
    #[allow(dead_code)] // Called from handle_int21h
    fn int21h_write_file(&mut self) -> u32 {
        // BX = file handle
        // CX = number of bytes to write
        // DS:DX = pointer to buffer
        // Returns: CF clear if success, AX = number of bytes written
        //          CF set if error, AX = error code (05h = access denied, 06h = invalid handle)

        let handle = self.cpu.bx;
        let cx = self.cpu.cx;

        // Standard DOS file handles:
        // 0 = stdin, 1 = stdout, 2 = stderr, 3 = stdaux, 4 = stdprn
        // Handles >= 5 are user-opened files

        if handle >= 5 {
            // File handles >= 5 are not supported (no file system implementation yet)
            // Return "invalid handle" error
            self.cpu.ax = DOS_ERROR_INVALID_HANDLE as u32;
            self.set_carry_flag(true);
        } else {
            // Standard handles: report all bytes written (but don't actually write)
            // Real implementation would write to console/device
            self.cpu.ax = cx as u32; // Report all bytes written
            self.set_carry_flag(false);
        }
        51
    }

    /// Handle INT 05h - Print Screen / BOUND Exception
    #[allow(dead_code)] // Called dynamically based on interrupt number
    fn handle_int05h(&mut self) -> u32 {
        // Skip the INT 05h instruction (2 bytes: 0xCD 0x05)
        self.cpu.ip = self.cpu.ip.wrapping_add(2);

        // Log stub call
        self.log_stub_interrupt(0x05, None, "Print Screen/BOUND Exception");

        // INT 05h is used for:
        // 1. Print Screen (Shift+PrtSc) - not implemented in emulator
        // 2. BOUND instruction exception - array bounds check failure
        // For now, just return (ignore the exception)
        51
    }

    /// Handle INT 08h - Timer Tick (System Timer)
    /// Called by hardware timer 18.2065 times per second
    #[allow(dead_code)] // Called dynamically based on interrupt number
    fn handle_int08h(&mut self) -> u32 {
        // Skip the INT 08h instruction (2 bytes: 0xCD 0x08)
        self.cpu.ip = self.cpu.ip.wrapping_add(2);

        // Real hardware timer interrupt handler
        // This interrupt fires 18.2065 times per second (every 54.9254 ms)
        // BIOS uses this to maintain time-of-day counter at 0040:006Ch

        // Increment the timer tick counter in BIOS data area
        // Timer ticks stored at 0x0040:0x006C (4 bytes, little-endian)
        let tick_addr = 0x046C;
        let mut ticks = self.cpu.memory.read(tick_addr) as u32;
        ticks |= (self.cpu.memory.read(tick_addr + 1) as u32) << 8;
        ticks |= (self.cpu.memory.read(tick_addr + 2) as u32) << 16;
        ticks |= (self.cpu.memory.read(tick_addr + 3) as u32) << 24;

        ticks = ticks.wrapping_add(1);

        // Check for midnight rollover (1573040 ticks = 24 hours at 18.2065 ticks/sec)
        if ticks >= 0x001800B0 {
            // 1573040 decimal = 0x1800B0
            ticks = 0;
            // Set midnight flag at 0x0040:0x0070
            self.cpu.memory.write(0x0470, 1);
        }

        // Write back the updated tick count
        self.cpu.memory.write(tick_addr, (ticks & 0xFF) as u8);
        self.cpu
            .memory
            .write(tick_addr + 1, ((ticks >> 8) & 0xFF) as u8);
        self.cpu
            .memory
            .write(tick_addr + 2, ((ticks >> 16) & 0xFF) as u8);
        self.cpu
            .memory
            .write(tick_addr + 3, ((ticks >> 24) & 0xFF) as u8);

        // Call INT 1Ch (user timer tick handler)
        // This is the standard PC/AT BIOS behavior - INT 08h chains to INT 1Ch
        // Programs can hook INT 1Ch to execute code on every timer tick
        // Since we can't directly trigger an interrupt from here (trigger_interrupt is private),
        // we'll note that programs expecting INT 1Ch will need to hook INT 08h instead
        // The BIOS default INT 1Ch handler is just an IRET at F000:0040

        51
    }

    /// Handle INT 09h - Keyboard Hardware Interrupt
    /// Called by keyboard hardware when a key is pressed or released
    #[allow(dead_code)] // Called dynamically based on interrupt number
    fn handle_int09h(&mut self) -> u32 {
        // Skip the INT 09h instruction (2 bytes: 0xCD 0x09)
        self.cpu.ip = self.cpu.ip.wrapping_add(2);

        // Log stub call (partial implementation)
        self.log_stub_interrupt(0x09, None, "Keyboard Hardware Interrupt (partial stub)");

        // Hardware keyboard interrupt
        // This is typically triggered by keyboard controller when a key is pressed
        // The BIOS interrupt handler would:
        // 1. Read scancode from keyboard port (60h)
        // 2. Convert to ASCII if printable
        // 3. Store in keyboard buffer at 0040:001Eh
        // 4. Update buffer pointers
        // 5. Send EOI to interrupt controller

        // For emulator, keyboard input is handled by INT 16h services
        // We just acknowledge the interrupt
        51
    }

    /// Handle INT 11h - Equipment List
    /// Returns equipment flags in AX
    #[allow(dead_code)] // Called dynamically based on interrupt number
    fn handle_int11h(&mut self) -> u32 {
        // Skip the INT 11h instruction (2 bytes: 0xCD 0x11)
        self.cpu.ip = self.cpu.ip.wrapping_add(2);

        // Equipment list word format:
        // Bit 0: Floppy drive(s) installed
        // Bit 1: Math coprocessor installed
        // Bits 2-3: System RAM (00=16K, 01=32K, 10=48K, 11=64K+) - obsolete
        // Bits 4-5: Initial video mode (00=EGA/VGA, 01=CGA 40x25, 10=CGA 80x25, 11=MDA 80x25)
        // Bits 6-7: Number of floppy drives (00=1, 01=2, etc.) if bit 0 set
        // Bit 8: DMA installed (0=yes on PC/XT)
        // Bits 9-11: Number of serial ports
        // Bit 12: Game adapter installed
        // Bit 13: Serial printer installed (PCjr)
        // Bits 14-15: Number of parallel printers

        // Query actual system configuration
        let floppy_count = self.cpu.memory.floppy_count();
        let video_type = self.cpu.memory.video_adapter_type();

        // Build equipment flags dynamically
        let mut equipment_flags: u16 = 0;

        // Bit 0: Floppy drive(s) installed
        if floppy_count > 0 {
            equipment_flags |= 0b0000_0000_0000_0001;
        }

        // Bit 1: Math coprocessor - not emulated
        // equipment_flags |= 0b0000_0000_0000_0010; // Not set

        // Bits 2-3: System RAM (always 11 for 64K+)
        equipment_flags |= 0b0000_0000_0000_1100;

        // Bits 4-5: Initial video mode
        use crate::bus::VideoAdapterType;
        let video_mode_bits = match video_type {
            VideoAdapterType::None => 0b00, // Treat as EGA/VGA
            VideoAdapterType::Mda => 0b11,  // MDA 80x25
            VideoAdapterType::Cga => 0b10,  // CGA 80x25 color
            VideoAdapterType::Ega => 0b00,  // EGA
            VideoAdapterType::Vga => 0b00,  // VGA
        };
        equipment_flags |= (video_mode_bits << 4) as u16;

        // Bits 6-7: Number of floppy drives if bit 0 is set
        if floppy_count > 0 {
            let floppy_bits = ((floppy_count - 1) & 0b11) as u16;
            equipment_flags |= floppy_bits << 6;
        }

        // Bit 8: DMA installed (0 = yes, 1 = no) - we say no
        // equipment_flags |= 0b0000_0001_0000_0000; // Not set (DMA present)

        // Bits 9-11: Number of serial ports (1 port)
        equipment_flags |= 0b0000_0010_0000_0000;

        // Bit 12: Game adapter - not installed
        // equipment_flags |= 0b0001_0000_0000_0000; // Not set

        // Bit 13: Serial printer - not installed
        // equipment_flags |= 0b0010_0000_0000_0000; // Not set

        // Bits 14-15: Number of parallel printers (1 printer)
        equipment_flags |= 0b0100_0000_0000_0000;

        self.cpu.ax = equipment_flags as u32;

        if LogConfig::global().should_log(LogCategory::Interrupts, LogLevel::Debug) {
            eprintln!(
                "INT 11h: Returning equipment word 0x{:04X} (floppy={}, video={:?})",
                equipment_flags, floppy_count, video_type
            );
        }

        51
    }

    /// Handle INT 12h - Get Memory Size
    /// Returns the amount of conventional memory in KB in AX
    #[allow(dead_code)] // Called dynamically based on interrupt number
    fn handle_int12h(&mut self) -> u32 {
        // Skip the INT 12h instruction (2 bytes: 0xCD 0x12)
        self.cpu.ip = self.cpu.ip.wrapping_add(2);

        // Get conventional memory size from bus (max 640KB)
        let memory_kb = self.cpu.memory.conventional_memory_kb() as u16;

        // Return conventional memory size in AX
        self.cpu.ax = memory_kb as u32;

        // Clear carry flag (success)
        self.set_carry_flag(false);

        51
    }

    /// Simulate INT instruction: push FLAGS, CS, IP and clear IF
    fn simulate_int_call(&mut self) {
        // Calculate return address (IP points to INT opcode, return is INT+2)
        let return_ip = self.cpu.ip.wrapping_add(2);

        // Push FLAGS
        let sp = self.cpu.sp.wrapping_sub(2);
        let ss = self.cpu.ss;
        let flags_addr = ((ss as u32) << 4) + (sp as u32);
        self.cpu
            .memory
            .write(flags_addr, (self.cpu.flags & 0xFF) as u8);
        self.cpu
            .memory
            .write(flags_addr + 1, ((self.cpu.flags >> 8) & 0xFF) as u8);
        self.cpu.sp = sp;

        // Clear IF flag to prevent nested interrupts (standard x86 INT behavior)
        const FLAG_IF: u32 = 0x0200;
        self.cpu.flags &= !FLAG_IF;

        // Push CS
        let sp = self.cpu.sp.wrapping_sub(2);
        let cs_addr = ((ss as u32) << 4) + (sp as u32);
        self.cpu.memory.write(cs_addr, (self.cpu.cs & 0xFF) as u8);
        self.cpu
            .memory
            .write(cs_addr + 1, ((self.cpu.cs >> 8) & 0xFF) as u8);
        self.cpu.sp = sp;

        // Push return IP
        let sp = self.cpu.sp.wrapping_sub(2);
        let ip_addr = ((ss as u32) << 4) + (sp as u32);
        self.cpu.memory.write(ip_addr, (return_ip & 0xFF) as u8);
        self.cpu
            .memory
            .write(ip_addr + 1, ((return_ip >> 8) & 0xFF) as u8);
        self.cpu.sp = sp;

        // Update IP to point past the INT instruction
        self.cpu.ip = return_ip;
    }

    /// Simulate IRET: pop IP, CS, FLAGS from stack
    fn simulate_iret(&mut self) {
        // Pop IP
        let sp = self.cpu.sp;
        let ss = self.cpu.ss;
        let ip_addr = ((ss as u32) << 4) + (sp as u32);
        let new_ip = self.cpu.memory.read(ip_addr) as u16
            | ((self.cpu.memory.read(ip_addr + 1) as u16) << 8);
        self.cpu.sp = sp.wrapping_add(2);

        // Pop CS
        let sp = self.cpu.sp;
        let cs_addr = ((ss as u32) << 4) + (sp as u32);
        let new_cs = self.cpu.memory.read(cs_addr) as u16
            | ((self.cpu.memory.read(cs_addr + 1) as u16) << 8);
        self.cpu.sp = sp.wrapping_add(2);

        // Pop FLAGS
        let sp = self.cpu.sp;
        let flags_addr = ((ss as u32) << 4) + (sp as u32);
        let new_flags = self.cpu.memory.read(flags_addr) as u16
            | ((self.cpu.memory.read(flags_addr + 1) as u16) << 8);
        self.cpu.sp = sp.wrapping_add(2);

        // Update CPU state
        self.cpu.ip = new_ip as u32;
        self.cpu.cs = new_cs;
        self.cpu.flags = new_flags as u32;
    }

    /// Update FLAGS value on stack (for BIOS interrupts that need to return modified flags)
    /// This should be called after simulate_int_call() and before simulate_iret()
    /// to ensure the modified flags are returned to the caller.
    fn update_flags_on_stack(&mut self) {
        // FLAGS are at SP+4 (after IP and CS which are 2 bytes each)
        let ss = self.cpu.ss;
        let sp = self.cpu.sp.wrapping_add(4);
        let flags_addr = ((ss as u32) << 4) + (sp as u32);

        // Write current FLAGS to stack
        self.cpu
            .memory
            .write(flags_addr, (self.cpu.flags & 0xFF) as u8);
        self.cpu
            .memory
            .write(flags_addr + 1, ((self.cpu.flags >> 8) & 0xFF) as u8);
    }

    /// Handle INT 13h BIOS disk services
    fn handle_int13h(&mut self) -> u32 {
        // Simulate INT instruction: push FLAGS, CS, IP and advance past INT opcode
        self.simulate_int_call();

        // Get function code from AH register
        let ah = ((self.cpu.ax >> 8) & 0xFF) as u8;

        // Count INT 13h calls for debugging
        if LogConfig::global().should_log(LogCategory::Bus, LogLevel::Debug) {
            static mut INT13H_CALL_COUNT: u32 = 0;
            unsafe {
                INT13H_CALL_COUNT += 1;
                let count = INT13H_CALL_COUNT; // Copy value to avoid shared reference
                if count % 10 == 1 {
                    eprintln!("INT 13h call #{}", count);
                }
                if count > 1000 {
                    eprintln!("!!! INT 13h called over 1000 times! Stopping...");
                    self.cpu.ax = (self.cpu.ax & 0x00FF) | (0x01 << 8); // Error
                    self.set_carry_flag(true);
                    return 51;
                }
            }
        }

        // Execute the appropriate INT 13h function
        // These functions will set AX (status in AH) and carry flag
        let cycles = match ah {
            0x00 => self.int13h_reset(),
            0x01 => self.int13h_get_status(),
            0x02 => self.int13h_read_sectors(),
            0x03 => self.int13h_write_sectors(),
            0x04 => self.int13h_verify_sectors(),
            0x05 => self.int13h_format_track(),
            0x08 => self.int13h_get_drive_params(),
            0x15 => self.int13h_get_disk_type(),
            0x16 => self.int13h_get_disk_change_status(),
            0x41 => self.int13h_check_extensions(),
            0x42 => self.int13h_extended_read(),
            0x43 => self.int13h_extended_write(),
            0x44 => self.int13h_extended_verify(),
            0x48 => self.int13h_get_extended_params(),
            _ => {
                eprintln!("!!! UNSUPPORTED INT 13h function: AH=0x{:02X} !!!", ah);
                // Unsupported function - set error in AH
                self.cpu.ax = (self.cpu.ax & 0x00FF) | (0x01 << 8); // Invalid function
                self.set_carry_flag(true);
                51 // Approximate INT instruction timing
            }
        };

        // Update FLAGS on stack to ensure modified flags are returned to caller
        // (INT 13h functions modify the carry flag to indicate success/error)
        self.update_flags_on_stack();

        // Simulate IRET to return from interrupt
        self.simulate_iret();

        cycles
    }

    /// INT 13h, AH=00h: Reset disk system
    fn int13h_reset(&mut self) -> u32 {
        // Get drive number from DL
        let _dl = (self.cpu.dx & 0xFF) as u8;

        // Reset the disk controller
        self.cpu.memory.disk_controller_mut().reset();

        // Clear AH (status = success)
        self.cpu.ax &= 0x00FF;

        // Clear carry flag (success)
        self.set_carry_flag(false);

        51 // Approximate INT instruction timing
    }

    /// INT 13h, AH=02h: Read sectors
    fn int13h_read_sectors(&mut self) -> u32 {
        use crate::disk::DiskRequest;

        // AL = number of sectors to read
        let count = (self.cpu.ax & 0xFF) as u8;

        // Validate count: must be < 128
        // NOTE: count=0 is valid and means "do nothing successfully" (used by DOS to test disk readiness)
        if count >= 128 {
            eprintln!("INT 13h AH=02h: Invalid sector count={}", count);
            self.cpu.ax = (self.cpu.ax & 0x00FF) | (0x01 << 8); // Invalid parameter
            self.set_carry_flag(true);
            return 51;
        }

        // Handle count=0 as a successful no-op (DOS uses this to check disk readiness)
        if count == 0 {
            self.cpu.ax &= 0x00FF; // AH=0 (success), AL=0 (sectors read)
            self.set_carry_flag(false);
            return 51;
        }

        // CH = cylinder (low 8 bits)
        // CL = sector number (bits 0-5), high 2 bits of cylinder (bits 6-7)
        let ch = ((self.cpu.cx >> 8) & 0xFF) as u8;
        let cl = (self.cpu.cx & 0xFF) as u8;
        let cylinder = ((cl as u16 & 0xC0) << 2) | (ch as u16);
        let sector = cl & 0x3F;

        // DH = head number
        let head = ((self.cpu.dx >> 8) & 0xFF) as u8;

        // DL = drive number
        let drive = (self.cpu.dx & 0xFF) as u8;

        // ES:BX = buffer address
        let buffer_seg = self.cpu.es;
        let buffer_offset = self.cpu.bx;

        // Check for 64KB boundary crossing and handle it by splitting the read
        let bytes_needed = (count as u32) * 512;
        if (buffer_offset as u32) + bytes_needed > 0x10000 {
            if LogConfig::global().should_log(LogCategory::Bus, LogLevel::Debug) {
                eprintln!(
                    "INT 13h AH=02h: Handling 64KB boundary crossing at ES:BX={:04X}:{:04X}, count={}",
                    buffer_seg, buffer_offset, count
                );
            }

            // Read data to temporary buffer
            let request = DiskRequest {
                drive,
                cylinder,
                head,
                sector,
                count,
            };

            let buffer_size = (count as usize) * 512;
            let mut buffer = vec![0u8; buffer_size];

            let status = self.cpu.memory.disk_read(&request, &mut buffer);
            if status != 0x00 {
                self.cpu.ax = (self.cpu.ax & 0x00FF) | ((status as u32) << 8);
                self.set_carry_flag(true);
                return 51;
            }

            // Write to memory with wrapping at 64KB boundary
            for (i, &byte) in buffer.iter().enumerate() {
                let offset = (buffer_offset as u16).wrapping_add(i as u16);
                self.cpu.write_byte(buffer_seg, offset as u16, byte);
            }

            // Success - return sectors read in AL, AH=0
            self.cpu.ax = count as u32; // AH=0, AL=count
            self.set_carry_flag(false);
            return 51;
        }

        if LogConfig::global().should_log(LogCategory::Bus, LogLevel::Debug) {
            eprintln!(
                "INT 13h AH=02h: count={}, C={}, H={}, S={}, drive=0x{:02X}, ES:BX={:04X}:{:04X}",
                count, cylinder, head, sector, drive, buffer_seg, buffer_offset
            );
        }

        // Create disk request
        let request = DiskRequest {
            drive,
            cylinder,
            head,
            sector,
            count,
        };

        // Prepare buffer
        let buffer_size = (count as usize) * 512;
        let mut buffer = vec![0u8; buffer_size];

        // Perform read using bus helper method
        let status = self.cpu.memory.disk_read(&request, &mut buffer);

        if LogConfig::global().should_log(LogCategory::Bus, LogLevel::Debug) {
            eprintln!(
                "INT 13h AH=02h: Status=0x{:02X}, C={}, H={}, S={}, count={}, drive=0x{:02X}",
                status, cylinder, head, sector, count, drive
            );
        }
        // Copy buffer to memory at ES:BX
        if status == 0x00 {
            if LogConfig::global().should_log(LogCategory::Bus, LogLevel::Debug) {
                eprintln!(
                    "INT 13h AH=02h: Starting to write {} bytes to memory...",
                    buffer.len()
                );
            }
            let should_log_progress =
                LogConfig::global().should_log(LogCategory::Bus, LogLevel::Debug);
            for (i, &byte) in buffer.iter().enumerate() {
                if should_log_progress && i % 128 == 0 {
                    eprintln!("  Written {} / {} bytes...", i, buffer.len());
                }
                let offset = (buffer_offset as u16).wrapping_add(i as u16);
                self.cpu.write_byte(buffer_seg, offset as u16, byte);
            }
            if LogConfig::global().should_log(LogCategory::Bus, LogLevel::Debug) {
                eprintln!(
                    "INT 13h AH=02h: Finished writing all {} bytes",
                    buffer.len()
                );

                // Verify the write by reading back the first 32 bytes
                eprint!(
                    "INT 13h AH=02h: Verifying first 32 bytes at {:04X}:{:04X}:",
                    buffer_seg, buffer_offset
                );
                for i in 0..32.min(buffer.len()) {
                    if i % 16 == 0 {
                        eprint!("\n  {:04X}:", i);
                    }
                    let offset = (buffer_offset as u16).wrapping_add(i as u16);
                    let byte = self.cpu.read_byte(buffer_seg, offset as u16);
                    eprint!(" {:02X}", byte);
                }
                eprintln!();
            }
        }

        // Set AH = status
        self.cpu.ax = (self.cpu.ax & 0x00FF) | ((status as u32) << 8);

        // Set carry flag based on status
        self.set_carry_flag(status != 0x00);

        // AL = number of sectors read (on success)
        if status == 0x00 {
            self.cpu.ax = (self.cpu.ax & 0xFF00) | (count as u32);

            // Note: INT 13h AH=02h does NOT modify ES:BX
            // The buffer pointer remains unchanged after the read
            // (unlike some other BIOS calls that advance pointers)
        }

        51 // Approximate INT instruction timing
    }

    /// INT 13h, AH=03h: Write sectors
    fn int13h_write_sectors(&mut self) -> u32 {
        use crate::disk::DiskRequest;

        // AL = number of sectors to write
        let count = (self.cpu.ax & 0xFF) as u8;

        // Validate count: must be < 128
        // NOTE: count=0 is valid and means "do nothing successfully"
        if count >= 128 {
            self.cpu.ax = (self.cpu.ax & 0x00FF) | (0x01 << 8); // Invalid parameter
            self.set_carry_flag(true);
            return 51;
        }

        // Handle count=0 as a successful no-op
        if count == 0 {
            self.cpu.ax &= 0x00FF; // AH=0 (success), AL=0 (sectors written)
            self.set_carry_flag(false);
            return 51;
        }

        // CH = cylinder (low 8 bits)
        // CL = sector number (bits 0-5), high 2 bits of cylinder (bits 6-7)
        let ch = ((self.cpu.cx >> 8) & 0xFF) as u8;
        let cl = (self.cpu.cx & 0xFF) as u8;
        let cylinder = ((cl as u16 & 0xC0) << 2) | (ch as u16);
        let sector = cl & 0x3F;

        // DH = head number
        let head = ((self.cpu.dx >> 8) & 0xFF) as u8;

        // DL = drive number
        let drive = (self.cpu.dx & 0xFF) as u8;

        // ES:BX = buffer address
        let buffer_seg = self.cpu.es;
        let buffer_offset = self.cpu.bx;

        // Read data from memory at ES:BX
        let buffer_size = (count as usize) * 512;
        let mut buffer = vec![0u8; buffer_size];
        for (i, byte) in buffer.iter_mut().enumerate() {
            let offset = (buffer_offset as u16).wrapping_add(i as u16);
            *byte = self.cpu.read_byte(buffer_seg, offset as u16);
        }

        // Create disk request
        let request = DiskRequest {
            drive,
            cylinder,
            head,
            sector,
            count,
        };

        // Perform write using bus helper method
        let status = self.cpu.memory.disk_write(&request, &buffer);

        // Set AH = status
        self.cpu.ax = (self.cpu.ax & 0x00FF) | ((status as u32) << 8);

        // Set carry flag based on status
        self.set_carry_flag(status != 0x00);

        // AL = number of sectors written (on success)
        if status == 0x00 {
            self.cpu.ax = (self.cpu.ax & 0xFF00) | (count as u32);

            // Note: INT 13h AH=03h does NOT modify ES:BX
            // The buffer pointer remains unchanged after the write
            // (just like AH=02h read sectors)
        }

        51 // Approximate INT instruction timing
    }

    /// INT 13h, AH=08h: Get drive parameters
    fn int13h_get_drive_params(&mut self) -> u32 {
        use crate::disk::DiskController;

        // DL = drive number
        let drive = (self.cpu.dx & 0xFF) as u8;

        eprintln!(
            "INT 13h AH=08h: Get drive parameters for drive 0x{:02X}",
            drive
        );

        // Check if drive exists
        let drive_exists = if drive < 0x80 {
            // Floppy drive - check if floppy is mounted
            self.cpu.memory.has_floppy(drive)
        } else if drive == 0x80 {
            // Hard drive C: - check if hard drive is mounted
            self.cpu.memory.has_hard_drive()
        } else {
            // Hard drive D: or higher - not supported
            false
        };

        if !drive_exists {
            eprintln!("INT 13h AH=08h: Drive 0x{:02X} does not exist", drive);
            // Invalid drive - return error
            self.cpu.ax = (self.cpu.ax & 0x00FF) | (0x01 << 8); // Invalid function
            self.set_carry_flag(true);
            return 51;
        }

        // Get drive parameters
        if let Some((cylinders, sectors_per_track, heads)) = DiskController::get_drive_params(drive)
        {
            eprintln!(
                "INT 13h AH=08h: Returning C={}, H={}, S={}",
                cylinders, heads, sectors_per_track
            );

            // BL = drive type (for floppies)
            if drive < 0x80 {
                self.cpu.bx = (self.cpu.bx & 0xFF00) | 0x04; // 1.44MB floppy
            } else {
                self.cpu.bx &= 0xFF00; // Hard drive
            }

            // CH = low 8 bits of maximum cylinder number
            // CL = sectors per track (bits 0-5), high 2 bits of cylinders (bits 6-7)
            let max_cylinder = cylinders - 1; // 0-based
            let ch = (max_cylinder & 0xFF) as u8;
            let cl_high = (((max_cylinder >> 8) & 0x03) << 6) as u8;
            let cl = cl_high | sectors_per_track;

            self.cpu.cx = ((ch as u32) << 8) | (cl as u32);

            // DH = maximum head number (0-based)
            // DL = number of drives of this type
            let num_drives = if drive < 0x80 {
                // Floppy drives: count floppy A and floppy B
                let mut count = 0u8;
                if self.cpu.memory.has_floppy(0x00) {
                    count += 1;
                }
                if self.cpu.memory.has_floppy(0x01) {
                    count += 1;
                }
                count
            } else {
                // Hard drives: read from BIOS Data Area at 0x0040:0x0075
                // If BDA is not initialized (0), count directly
                let bda_count = self.cpu.memory.read(BDA_HARD_DRIVE_COUNT);
                if bda_count > 0 {
                    bda_count
                } else if self.cpu.memory.has_hard_drive() {
                    1u8
                } else {
                    0u8
                }
            };
            self.cpu.dx = (((heads - 1) as u32) << 8) | (num_drives as u32);

            // ES:DI = pointer to disk parameter table (DPT) in BIOS ROM
            // The DPT is located at F000:0250 (see bios.rs DISK_PARAMETER_TABLE_OFFSET)
            self.cpu.es = 0xF000;
            self.cpu.di = 0x0250u32;

            // AH = 0 (success)
            self.cpu.ax &= 0x00FF;

            // Clear carry flag (success)
            self.set_carry_flag(false);
        } else {
            // Invalid drive
            self.cpu.ax = (self.cpu.ax & 0x00FF) | (0x01 << 8); // Invalid function
            self.set_carry_flag(true);
        }

        51 // Approximate INT instruction timing
    }

    /// INT 13h, AH=01h: Get disk status
    fn int13h_get_status(&mut self) -> u32 {
        // DL = drive number
        let _drive = (self.cpu.dx & 0xFF) as u8;

        // Return last status from disk controller
        let status = self.cpu.memory.disk_controller().status();

        // AH = status
        self.cpu.ax = (self.cpu.ax & 0x00FF) | ((status as u32) << 8);

        // Clear carry flag if status is 0, set if error
        self.set_carry_flag(status != 0x00);

        51
    }

    /// INT 13h, AH=04h: Verify sectors
    fn int13h_verify_sectors(&mut self) -> u32 {
        // AL = number of sectors to verify
        let count = (self.cpu.ax & 0xFF) as u8;

        // Parse CHS parameters (same as read/write)
        let ch = ((self.cpu.cx >> 8) & 0xFF) as u8;
        let cl = (self.cpu.cx & 0xFF) as u8;
        let _cylinder = ((cl as u16 & 0xC0) << 2) | (ch as u16);
        let _sector = cl & 0x3F;
        let _head = ((self.cpu.dx >> 8) & 0xFF) as u8;
        let _drive = (self.cpu.dx & 0xFF) as u8;

        // For now, always succeed (verification is implicit in read operations)
        // In a real system, this would read sectors and verify ECC/checksums

        // AH = 0 (success)
        self.cpu.ax &= 0x00FF;

        // AL = number of sectors verified
        self.cpu.ax = (self.cpu.ax & 0xFF00) | (count as u32);

        // Clear carry flag (success)
        self.set_carry_flag(false);

        51
    }

    /// INT 13h, AH=05h: Format track
    fn int13h_format_track(&mut self) -> u32 {
        // AL = number of sectors to format
        let _count = (self.cpu.ax & 0xFF) as u8;

        // Parse CHS parameters
        let ch = ((self.cpu.cx >> 8) & 0xFF) as u8;
        let cl = (self.cpu.cx & 0xFF) as u8;
        let _cylinder = ((cl as u16 & 0xC0) << 2) | (ch as u16);
        let _head = ((self.cpu.dx >> 8) & 0xFF) as u8;
        let drive = (self.cpu.dx & 0xFF) as u8;

        // ES:BX = pointer to address field buffer
        // Each entry is 4 bytes: C, H, S, N (cylinder, head, sector, sector size code)

        // For now, just mark track as formatted (fill with zeros in a real implementation)
        // This is a destructive operation that would write sector headers

        if drive < 0x80 {
            // Floppy: format succeeds
            self.cpu.ax &= 0x00FF; // AH = 0 (success)
            self.set_carry_flag(false);
        } else {
            // Hard drive: format not typically supported via this function
            self.cpu.ax = (self.cpu.ax & 0x00FF) | (0x01 << 8); // Error
            self.set_carry_flag(true);
        }

        51
    }

    /// INT 13h, AH=15h: Get disk type
    fn int13h_get_disk_type(&mut self) -> u32 {
        use crate::disk::DiskController;

        // DL = drive number
        let drive = (self.cpu.dx & 0xFF) as u8;

        if drive < 0x80 {
            // Floppy drive
            // Check if drive exists
            let has_disk = self.cpu.memory.has_floppy(drive);

            if has_disk {
                // AH = 01h (floppy with change-line support)
                self.cpu.ax = (self.cpu.ax & 0x00FF) | (0x01 << 8);
                self.set_carry_flag(false);
            } else {
                // AH = 00h (no such drive)
                self.cpu.ax &= 0x00FF;
                self.set_carry_flag(false);
            }
        } else {
            // Hard drive
            // Check if hard drive exists (only drive 0x80 supported)
            if drive != 0x80 || !self.cpu.memory.has_hard_drive() {
                // No such drive
                self.cpu.ax &= 0x00FF; // AH = 00h (no such drive)
                self.set_carry_flag(false);
                return 51;
            }

            if let Some((cylinders, sectors_per_track, heads)) =
                DiskController::get_drive_params(drive)
            {
                // AH = 03h (fixed disk)
                self.cpu.ax = (self.cpu.ax & 0x00FF) | (0x03 << 8);

                // CX:DX = number of sectors
                let total_sectors = cylinders as u32 * sectors_per_track as u32 * heads as u32;
                self.cpu.cx = ((total_sectors >> 16) & 0xFFFF) as u32;
                self.cpu.dx = (total_sectors & 0xFFFF) as u32;

                self.set_carry_flag(false);
            } else {
                // No such drive
                self.cpu.ax &= 0x00FF;
                self.set_carry_flag(false);
            }
        }

        51
    }

    /// INT 13h, AH=16h: Get disk change status (floppies only)
    fn int13h_get_disk_change_status(&mut self) -> u32 {
        // DL = drive number
        let drive = (self.cpu.dx & 0xFF) as u8;

        if drive < 0x80 {
            // Floppy drive
            // For now, always report "no change" (AH=00h)
            // A real implementation would track if the disk was changed
            self.cpu.ax &= 0x00FF; // AH = 0 (no change detected)
            self.set_carry_flag(false);
        } else {
            // Function not valid for hard drives
            self.cpu.ax = (self.cpu.ax & 0x00FF) | (0x01 << 8); // Invalid function
            self.set_carry_flag(true);
        }

        51
    }

    /// INT 13h, AH=41h: Check extensions present (Extended INT 13h)
    fn int13h_check_extensions(&mut self) -> u32 {
        // BX = 0x55AA (signature)
        // DL = drive number

        let bx = self.cpu.bx;
        let _drive = (self.cpu.dx & 0xFF) as u8;

        if bx == 0x55AA {
            // Extended INT 13h supported for all drives (floppy and hard disk)
            // Modern BIOS implementations support extensions for both drive types
            // BX = 0xAA55 (signature)
            self.cpu.bx = 0xAA55u32;

            // AH = major version (01h = 1.x, 20h = 2.0, 21h = 2.1, 30h = 3.0)
            // Let's report version 3.0 (full EDD 3.0 support)
            self.cpu.ax = (self.cpu.ax & 0x00FF) | (0x30 << 8);

            // CX = API subset support bitmap
            // Bit 0 = extended disk access functions (AH=42h-44h, 47h, 48h)
            // Bit 1 = removable drive controller functions (AH=45h, 46h, 48h, 49h, INT 15h AH=52h)
            // Bit 2 = enhanced disk drive (EDD) support (AH=48h)
            // We support bits 0 and 2
            self.cpu.cx = 0x0001 | 0x0004; // Bit 0 (extended access) + Bit 2 (EDD)

            self.set_carry_flag(false);
        } else {
            // Extensions not supported or invalid parameters (invalid BX signature)
            self.cpu.ax = (self.cpu.ax & 0x00FF) | (0x01 << 8); // Error
            self.set_carry_flag(true);
        }

        51
    }

    /// INT 13h, AH=42h: Extended Read (LBA)
    fn int13h_extended_read(&mut self) -> u32 {
        // DS:SI = pointer to Disk Address Packet (DAP)
        // DL = drive number

        let drive = (self.cpu.dx & 0xFF) as u8;
        let ds = self.cpu.ds;
        let si = self.cpu.si;

        // Read DAP structure from memory
        let dap_addr = ((ds as u32) << 4) + (si as u32);

        // DAP structure:
        // Offset 0: Size of DAP (10h or 18h)
        // Offset 1: Reserved (0)
        // Offset 2-3: Number of blocks to transfer (word)
        // Offset 4-7: Transfer buffer (segment:offset)
        // Offset 8-15: Starting absolute block number (LBA, qword)

        let dap_size = self.cpu.memory.read(dap_addr);
        if dap_size < 0x10 {
            // Invalid DAP size
            self.cpu.ax = (self.cpu.ax & 0x00FF) | (0x01 << 8); // Error
            self.set_carry_flag(true);
            return 51;
        }

        let num_sectors = u16::from_le_bytes([
            self.cpu.memory.read(dap_addr + 2),
            self.cpu.memory.read(dap_addr + 3),
        ]);

        let buffer_offset = u16::from_le_bytes([
            self.cpu.memory.read(dap_addr + 4),
            self.cpu.memory.read(dap_addr + 5),
        ]);

        let buffer_segment = u16::from_le_bytes([
            self.cpu.memory.read(dap_addr + 6),
            self.cpu.memory.read(dap_addr + 7),
        ]);

        // Read LBA (64-bit, but we only support 32-bit LBA for now)
        let lba = u32::from_le_bytes([
            self.cpu.memory.read(dap_addr + 8),
            self.cpu.memory.read(dap_addr + 9),
            self.cpu.memory.read(dap_addr + 10),
            self.cpu.memory.read(dap_addr + 11),
        ]);

        // Read sectors using LBA
        let buffer_size = (num_sectors as usize) * 512;
        let mut buffer = vec![0u8; buffer_size];

        // Perform LBA read
        let status = self
            .cpu
            .memory
            .disk_read_lba(drive, lba, num_sectors as u8, &mut buffer);

        // Copy to memory at buffer_segment:buffer_offset
        if status == 0x00 {
            for (i, &byte) in buffer.iter().enumerate() {
                let offset = (buffer_offset as u16).wrapping_add(i as u16);
                self.cpu.write_byte(buffer_segment, offset as u16, byte);
            }

            // Update DAP with actual number of sectors transferred
            self.cpu
                .memory
                .write(dap_addr + 2, (num_sectors & 0xFF) as u8);
            self.cpu
                .memory
                .write(dap_addr + 3, ((num_sectors >> 8) & 0xFF) as u8);
        }

        // Set return values according to INT 13h Extensions specification
        // On success: AH = 0x00, AL = number of sectors transferred, CF = 0
        // On error: AH = error code, AL = undefined, CF = 1
        if status == 0x00 {
            // Success: Set AX = (AH=0x00, AL=sectors_transferred)
            // Clear high bits and set only the low 16 bits
            self.cpu.ax = (self.cpu.ax & 0xFFFF0000) | (num_sectors as u32 & 0xFFFF);
            self.set_carry_flag(false);
        } else {
            // Error: AH = status code, preserve AL, set CF
            eprintln!("INT 13h AH=0x42: ERROR status=0x{:02X}", status);
            self.cpu.ax = (self.cpu.ax & 0x00FF) | ((status as u32) << 8);
            self.set_carry_flag(true);
        }

        51
    }

    /// INT 13h, AH=43h: Extended Write (LBA)
    fn int13h_extended_write(&mut self) -> u32 {
        // AL = write flags (bit 0: verify after write)
        // DS:SI = pointer to Disk Address Packet (DAP)
        // DL = drive number

        let drive = (self.cpu.dx & 0xFF) as u8;
        let ds = self.cpu.ds;
        let si = self.cpu.si;
        let _verify = (self.cpu.ax & 0x01) != 0;

        // Read DAP structure
        let dap_addr = ((ds as u32) << 4) + (si as u32);

        let dap_size = self.cpu.memory.read(dap_addr);
        if dap_size < 0x10 {
            self.cpu.ax = (self.cpu.ax & 0x00FF) | (0x01 << 8);
            self.set_carry_flag(true);
            return 51;
        }

        let num_sectors = u16::from_le_bytes([
            self.cpu.memory.read(dap_addr + 2),
            self.cpu.memory.read(dap_addr + 3),
        ]);

        let buffer_offset = u16::from_le_bytes([
            self.cpu.memory.read(dap_addr + 4),
            self.cpu.memory.read(dap_addr + 5),
        ]);

        let buffer_segment = u16::from_le_bytes([
            self.cpu.memory.read(dap_addr + 6),
            self.cpu.memory.read(dap_addr + 7),
        ]);

        let lba = u32::from_le_bytes([
            self.cpu.memory.read(dap_addr + 8),
            self.cpu.memory.read(dap_addr + 9),
            self.cpu.memory.read(dap_addr + 10),
            self.cpu.memory.read(dap_addr + 11),
        ]);

        // Read data from memory
        let buffer_size = (num_sectors as usize) * 512;
        let mut buffer = vec![0u8; buffer_size];
        for (i, byte) in buffer.iter_mut().enumerate() {
            let offset = (buffer_offset as u16).wrapping_add(i as u16);
            *byte = self.cpu.read_byte(buffer_segment, offset as u16);
        }

        // Perform LBA write
        let status = self
            .cpu
            .memory
            .disk_write_lba(drive, lba, num_sectors as u8, &buffer);

        // Set return values
        // AH = status, AL = sectors transferred
        if status == 0x00 {
            // Success: AL = sectors transferred, AH = 0
            self.cpu.ax = (num_sectors & 0xFF) as u32;
        } else {
            // Error: AH = status code, AL preserved
            self.cpu.ax = (self.cpu.ax & 0x00FF) | ((status as u32) << 8);
        }
        self.set_carry_flag(status != 0x00);

        51
    }

    /// INT 13h, AH=44h: Extended Verify (LBA)
    fn int13h_extended_verify(&mut self) -> u32 {
        // DS:SI = pointer to Disk Address Packet
        // DL = drive number

        let ds = self.cpu.ds;
        let si = self.cpu.si;

        let dap_addr = ((ds as u32) << 4) + (si as u32);

        let dap_size = self.cpu.memory.read(dap_addr);
        if dap_size < 0x10 {
            self.cpu.ax = (self.cpu.ax & 0x00FF) | (0x01 << 8);
            self.set_carry_flag(true);
            return 51;
        }

        let num_sectors = u16::from_le_bytes([
            self.cpu.memory.read(dap_addr + 2),
            self.cpu.memory.read(dap_addr + 3),
        ]);

        // For verify, we just report success without actually reading
        // In a real system, this would read and verify sectors

        // AH = 0 (success), AL = number of sectors verified (low byte)
        self.cpu.ax = (num_sectors & 0xFF) as u32;
        self.set_carry_flag(false);

        51
    }

    /// INT 13h, AH=48h: Get Extended Drive Parameters
    fn int13h_get_extended_params(&mut self) -> u32 {
        use crate::disk::DiskController;

        // DS:SI = pointer to result buffer
        // DL = drive number

        let drive = (self.cpu.dx & 0xFF) as u8;
        let ds = self.cpu.ds;
        let si = self.cpu.si;

        // Get drive parameters
        if let Some((cylinders, sectors_per_track, heads)) = DiskController::get_drive_params(drive)
        {
            let buffer_addr = ((ds as u32) << 4) + (si as u32);

            // Read buffer size from first word
            let buffer_size = u16::from_le_bytes([
                self.cpu.memory.read(buffer_addr),
                self.cpu.memory.read(buffer_addr + 1),
            ]);

            if buffer_size < 0x1A {
                // Buffer too small
                self.cpu.ax = (self.cpu.ax & 0x00FF) | (0x01 << 8);
                self.set_carry_flag(true);
                return 51;
            }

            // Fill in EDD 1.x structure (26 bytes minimum)
            // Offset 0-1: Buffer size (word)
            self.cpu.memory.write(buffer_addr, 0x1A); // Size = 26 bytes
            self.cpu.memory.write(buffer_addr + 1, 0x00);

            // Offset 2-3: Information flags (word)
            // Bit 0: DMA boundary errors handled transparently
            // Bit 1: geometry is valid (CHS)
            // Bit 2: removable media
            self.cpu.memory.write(buffer_addr + 2, 0x02); // Geometry valid
            self.cpu.memory.write(buffer_addr + 3, 0x00);

            // Offset 4-7: Number of physical cylinders (dword)
            let cyl_bytes = (cylinders as u32).to_le_bytes();
            self.cpu.memory.write(buffer_addr + 4, cyl_bytes[0]);
            self.cpu.memory.write(buffer_addr + 5, cyl_bytes[1]);
            self.cpu.memory.write(buffer_addr + 6, cyl_bytes[2]);
            self.cpu.memory.write(buffer_addr + 7, cyl_bytes[3]);

            // Offset 8-11: Number of physical heads (dword)
            let head_bytes = (heads as u32).to_le_bytes();
            self.cpu.memory.write(buffer_addr + 8, head_bytes[0]);
            self.cpu.memory.write(buffer_addr + 9, head_bytes[1]);
            self.cpu.memory.write(buffer_addr + 10, head_bytes[2]);
            self.cpu.memory.write(buffer_addr + 11, head_bytes[3]);

            // Offset 12-15: Number of physical sectors per track (dword)
            let spt_bytes = (sectors_per_track as u32).to_le_bytes();
            self.cpu.memory.write(buffer_addr + 12, spt_bytes[0]);
            self.cpu.memory.write(buffer_addr + 13, spt_bytes[1]);
            self.cpu.memory.write(buffer_addr + 14, spt_bytes[2]);
            self.cpu.memory.write(buffer_addr + 15, spt_bytes[3]);

            // Offset 16-23: Total number of sectors (qword)
            let total_sectors = cylinders as u64 * heads as u64 * sectors_per_track as u64;
            let total_bytes = total_sectors.to_le_bytes();
            for i in 0..8 {
                self.cpu
                    .memory
                    .write(buffer_addr + 16 + i, total_bytes[i as usize]);
            }

            // Offset 24-25: Bytes per sector (word)
            self.cpu.memory.write(buffer_addr + 24, 0x00); // 512 bytes
            self.cpu.memory.write(buffer_addr + 25, 0x02);

            // AH = 0 (success)
            self.cpu.ax &= 0x00FF;
            self.set_carry_flag(false);
        } else {
            // Invalid drive
            self.cpu.ax = (self.cpu.ax & 0x00FF) | (0x01 << 8);
            self.set_carry_flag(true);
        }

        51
    }

    /// Set or clear the carry flag
    fn set_carry_flag(&mut self, value: bool) {
        const FLAG_CF: u32 = 0x0001;
        if value {
            self.cpu.flags |= FLAG_CF;
        } else {
            self.cpu.flags &= !FLAG_CF;
        }
    }

    /// Get the carry flag value
    #[allow(dead_code)] // Used in tests
    fn get_carry_flag(&self) -> bool {
        const FLAG_CF: u32 = 0x0001;
        (self.cpu.flags & FLAG_CF) != 0
    }

    /// Set or clear the zero flag
    fn set_zero_flag(&mut self, value: bool) {
        const FLAG_ZF: u32 = 0x0040;
        if value {
            self.cpu.flags |= FLAG_ZF;
        } else {
            self.cpu.flags &= !FLAG_ZF;
        }
    }

    /// Check if an interrupt vector has been overridden by DOS/OS
    ///
    /// Returns true if the OS has installed its own handler (vector doesn't point to BIOS ROM).
    /// Returns false if the vector still points to BIOS ROM (F000:xxxx) or is uninitialized (0000:0000).
    ///
    /// # Arguments
    /// * `int_num` - The interrupt number (0x00-0xFF)
    fn is_interrupt_overridden(&self, int_num: u8) -> bool {
        // Calculate vector table offset (int_num * 4)
        let vector_offset = (int_num as u32) * 4;

        // Read offset (low word) and segment (high word) from interrupt vector table
        let offset = self.cpu.memory.read(vector_offset) as u16
            | ((self.cpu.memory.read(vector_offset + 1) as u16) << 8);
        let segment = self.cpu.memory.read(vector_offset + 2) as u16
            | ((self.cpu.memory.read(vector_offset + 3) as u16) << 8);

        // Vector is overridden if:
        // 1. It's not null (0000:0000) AND
        // 2. It doesn't point to BIOS ROM (F000:xxxx)
        (segment != 0 || offset != 0) && segment != 0xF000
    }

    /// Handle INT 1Ah - Time and Date services
    #[allow(dead_code)] // Called dynamically based on interrupt number
    fn handle_int1ah(&mut self) -> u32 {
        // Skip the INT 1Ah instruction (2 bytes: 0xCD 0x1A)
        self.cpu.ip = self.cpu.ip.wrapping_add(2);

        // Get function code from AH register
        let ah = ((self.cpu.ax >> 8) & 0xFF) as u8;

        match ah {
            0x00 => self.int1ah_read_system_clock(),
            0x01 => self.int1ah_set_system_clock(),
            0x02 => self.int1ah_read_real_time_clock(),
            0x03 => self.int1ah_set_real_time_clock(),
            0x04 => self.int1ah_read_date(),
            0x05 => self.int1ah_set_date(),
            0xB1 => self.int1ah_pci_bios(),
            _ => {
                // Unsupported function - log and do nothing
                self.log_stub_interrupt(
                    0x1A,
                    Some(ah),
                    "Time/Date Services (unsupported subfunction)",
                );
                51
            }
        }
    }

    /// INT 1Ah, AH=00h - Read system clock counter
    #[allow(dead_code)] // Called from handle_int1ah
    fn int1ah_read_system_clock(&mut self) -> u32 {
        // Return tick count since midnight
        // PC timer ticks at 18.2065 Hz (65536 PIT ticks)
        // We'll use system time to calculate ticks since midnight

        use std::time::{SystemTime, UNIX_EPOCH};

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();

        // Get seconds since midnight
        let total_seconds = now.as_secs();
        let seconds_since_midnight = total_seconds % 86400;

        // Convert to ticks (18.2065 ticks per second)
        let ticks = (seconds_since_midnight as f64 * 18.2065) as u32;

        // CX:DX contains tick count
        self.cpu.cx = ((ticks >> 16) & 0xFFFF) as u32;
        self.cpu.dx = (ticks & 0xFFFF) as u32;

        // AL = midnight flag (0 = no midnight crossover since last read)
        self.cpu.ax &= 0xFF00;

        51
    }

    /// INT 1Ah, AH=01h - Set system clock counter (stub - read-only)
    #[allow(dead_code)] // Called from handle_int1ah
    fn int1ah_set_system_clock(&mut self) -> u32 {
        // Read-only implementation - ignore the set request
        51
    }

    /// INT 1Ah, AH=02h - Read real-time clock time (AT, PS/2)
    #[allow(dead_code)] // Called from handle_int1ah
    fn int1ah_read_real_time_clock(&mut self) -> u32 {
        use std::time::{SystemTime, UNIX_EPOCH};

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();

        // Get local time broken down (using simple UTC for now)
        let total_seconds = now.as_secs();
        let seconds_in_day = total_seconds % 86400;

        let hours = (seconds_in_day / 3600) as u8;
        let minutes = ((seconds_in_day % 3600) / 60) as u8;
        let seconds = (seconds_in_day % 60) as u8;

        // CH = hours (BCD)
        // CL = minutes (BCD)
        // DH = seconds (BCD)
        // DL = daylight savings flag (0)

        let hours_bcd = ((hours / 10) << 4) | (hours % 10);
        let minutes_bcd = ((minutes / 10) << 4) | (minutes % 10);
        let seconds_bcd = ((seconds / 10) << 4) | (seconds % 10);

        self.cpu.cx = ((hours_bcd as u32) << 8) | (minutes_bcd as u32);
        self.cpu.dx = (seconds_bcd as u32) << 8;

        // Clear carry flag (success)
        self.set_carry_flag(false);

        51
    }

    /// INT 1Ah, AH=03h - Set real-time clock time (stub - read-only)
    #[allow(dead_code)] // Called from handle_int1ah
    fn int1ah_set_real_time_clock(&mut self) -> u32 {
        // Read-only implementation - ignore the set request
        51
    }

    /// INT 1Ah, AH=04h - Read real-time clock date (AT, PS/2)
    #[allow(dead_code)] // Called from handle_int1ah
    fn int1ah_read_date(&mut self) -> u32 {
        use std::time::{SystemTime, UNIX_EPOCH};

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();

        // Convert to days since epoch
        let days_since_epoch = now.as_secs() / 86400;

        // Calculate year, month, day (simple algorithm for demonstration)
        // This is a simplified calculation - a proper implementation would use chrono
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

        // Simple month calculation (assuming non-leap year for simplicity)
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

        // CH = century (BCD) - 19 or 20
        // CL = year (BCD) - 00-99
        // DH = month (BCD) - 01-12
        // DL = day (BCD) - 01-31

        let century = year / 100;
        let year_part = year % 100;

        let century_bcd = ((century / 10) << 4) | (century % 10);
        let year_bcd = ((year_part / 10) << 4) | (year_part % 10);
        let month_bcd = ((month / 10) << 4) | (month % 10);
        let day_bcd = ((day / 10) << 4) | (day % 10);

        self.cpu.cx = ((century_bcd as u32) << 8) | (year_bcd as u32);
        self.cpu.dx = ((month_bcd as u32) << 8) | (day_bcd as u32);

        // Clear carry flag (success)
        self.set_carry_flag(false);

        51
    }

    /// INT 1Ah, AH=05h - Set real-time clock date (stub - read-only)
    #[allow(dead_code)] // Called from handle_int1ah
    fn int1ah_set_date(&mut self) -> u32 {
        // Read-only implementation - ignore the set request
        51
    }

    /// INT 1Ah, AH=B1h - PCI BIOS Services
    #[allow(dead_code)] // Called from handle_int1ah
    fn int1ah_pci_bios(&mut self) -> u32 {
        // Get subfunction from AL register
        let al = (self.cpu.ax & 0xFF) as u8;

        match al {
            0x01 => self.int1ah_pci_installation_check(),
            0x02 => self.int1ah_pci_find_device(),
            0x03 => self.int1ah_pci_find_class_code(),
            0x08 => self.int1ah_pci_read_config_byte(),
            0x09 => self.int1ah_pci_read_config_word(),
            0x0A => self.int1ah_pci_read_config_dword(),
            0x0B => self.int1ah_pci_write_config_byte(),
            0x0C => self.int1ah_pci_write_config_word(),
            0x0D => self.int1ah_pci_write_config_dword(),
            _ => {
                // Unsupported PCI BIOS function - log and return error
                self.log_stub_interrupt(
                    0x1A,
                    Some(0xB1),
                    &format!("PCI BIOS (unsupported subfunction AL=0x{:02X})", al),
                );
                // Set carry flag to indicate error
                self.set_carry_flag(true);
                // AH = error code (0x81 = function not supported)
                self.cpu.ax = (self.cpu.ax & 0x00FF) | 0x8100;
                51
            }
        }
    }

    /// INT 1Ah, AH=B1h, AL=01h - PCI BIOS Installation Check
    #[allow(dead_code)] // Called from int1ah_pci_bios
    fn int1ah_pci_installation_check(&mut self) -> u32 {
        // Return "PCI BIOS not present" to indicate no PCI bus
        // This is the correct response for a PC/XT which doesn't have PCI

        // Clear carry flag (function supported)
        self.set_carry_flag(false);

        // AH = 0x00 (function completed successfully)
        // But we set specific values to indicate "not present"
        self.cpu.ax &= 0x00FF;

        // DX should contain "PCI " signature (lower 16 bits of EDX) if present
        // We return 0 to indicate no PCI BIOS
        self.cpu.dx = 0x0000u32;

        // BX = PCI BIOS version (0x0000 = not present)
        self.cpu.bx = 0x0000u32;

        // CX = last PCI bus number (0 = no PCI)
        self.cpu.cx = 0x0000u32;

        51
    }

    /// INT 1Ah, AH=B1h, AL=02h - PCI Find Device
    #[allow(dead_code)] // Called from int1ah_pci_bios
    fn int1ah_pci_find_device(&mut self) -> u32 {
        // Return "device not found" since we have no PCI bus
        self.set_carry_flag(true);
        // AH = 0x86 (device not found)
        self.cpu.ax = (self.cpu.ax & 0x00FF) | 0x8600;
        51
    }

    /// INT 1Ah, AH=B1h, AL=03h - PCI Find Class Code
    #[allow(dead_code)] // Called from int1ah_pci_bios
    fn int1ah_pci_find_class_code(&mut self) -> u32 {
        // Return "device not found" since we have no PCI bus
        self.set_carry_flag(true);
        // AH = 0x86 (device not found)
        self.cpu.ax = (self.cpu.ax & 0x00FF) | 0x8600;
        51
    }

    /// INT 1Ah, AH=B1h, AL=08h - PCI Read Configuration Byte
    #[allow(dead_code)] // Called from int1ah_pci_bios
    fn int1ah_pci_read_config_byte(&mut self) -> u32 {
        // Return error since we have no PCI bus
        self.set_carry_flag(true);
        // AH = 0x87 (bad register number)
        self.cpu.ax = (self.cpu.ax & 0x00FF) | 0x8700;
        51
    }

    /// INT 1Ah, AH=B1h, AL=09h - PCI Read Configuration Word
    #[allow(dead_code)] // Called from int1ah_pci_bios
    fn int1ah_pci_read_config_word(&mut self) -> u32 {
        // Return error since we have no PCI bus
        self.set_carry_flag(true);
        // AH = 0x87 (bad register number)
        self.cpu.ax = (self.cpu.ax & 0x00FF) | 0x8700;
        51
    }

    /// INT 1Ah, AH=B1h, AL=0Ah - PCI Read Configuration Dword
    #[allow(dead_code)] // Called from int1ah_pci_bios
    fn int1ah_pci_read_config_dword(&mut self) -> u32 {
        // Return error since we have no PCI bus
        self.set_carry_flag(true);
        // AH = 0x87 (bad register number)
        self.cpu.ax = (self.cpu.ax & 0x00FF) | 0x8700;
        51
    }

    /// INT 1Ah, AH=B1h, AL=0Bh - PCI Write Configuration Byte
    #[allow(dead_code)] // Called from int1ah_pci_bios
    fn int1ah_pci_write_config_byte(&mut self) -> u32 {
        // Return error since we have no PCI bus
        self.set_carry_flag(true);
        // AH = 0x87 (bad register number)
        self.cpu.ax = (self.cpu.ax & 0x00FF) | 0x8700;
        51
    }

    /// INT 1Ah, AH=B1h, AL=0Ch - PCI Write Configuration Word
    #[allow(dead_code)] // Called from int1ah_pci_bios
    fn int1ah_pci_write_config_word(&mut self) -> u32 {
        // Return error since we have no PCI bus
        self.set_carry_flag(true);
        // AH = 0x87 (bad register number)
        self.cpu.ax = (self.cpu.ax & 0x00FF) | 0x8700;
        51
    }

    /// INT 1Ah, AH=B1h, AL=0Dh - PCI Write Configuration Dword
    #[allow(dead_code)] // Called from int1ah_pci_bios
    fn int1ah_pci_write_config_dword(&mut self) -> u32 {
        // Return error since we have no PCI bus
        self.set_carry_flag(true);
        // AH = 0x87 (bad register number)
        self.cpu.ax = (self.cpu.ax & 0x00FF) | 0x8700;
        51
    }

    /// Handle INT 33h - Microsoft Mouse Driver services
    #[allow(dead_code)] // Called dynamically based on interrupt number
    fn handle_int33h(&mut self) -> u32 {
        // Skip the INT 33h instruction (2 bytes: 0xCD 0x33)
        self.cpu.ip = self.cpu.ip.wrapping_add(2);

        // Get function code from AX register
        let ax = self.cpu.ax;

        match ax {
            0x0000 => self.int33h_reset(),
            0x0001 => self.int33h_show_cursor(),
            0x0002 => self.int33h_hide_cursor(),
            0x0003 => self.int33h_get_position_and_buttons(),
            0x0004 => self.int33h_set_position(),
            0x0005 => self.int33h_get_button_press_info(),
            0x0006 => self.int33h_get_button_release_info(),
            0x0007 => self.int33h_set_horizontal_limits(),
            0x0008 => self.int33h_set_vertical_limits(),
            0x000F => self.int33h_set_mickey_ratio(),
            0x0024 => self.int33h_get_driver_version(),
            _ => {
                // Unsupported function - do nothing
                51
            }
        }
    }

    /// INT 33h, AX=0000h - Reset mouse driver
    #[allow(dead_code)] // Called from handle_int33h
    fn int33h_reset(&mut self) -> u32 {
        let (ax, bx) = self.cpu.memory.mouse.reset();
        self.cpu.ax = ax as u32;
        self.cpu.bx = bx as u32;
        51
    }

    /// INT 33h, AX=0001h - Show mouse cursor
    #[allow(dead_code)] // Called from handle_int33h
    fn int33h_show_cursor(&mut self) -> u32 {
        self.cpu.memory.mouse.show_cursor();
        51
    }

    /// INT 33h, AX=0002h - Hide mouse cursor
    #[allow(dead_code)] // Called from handle_int33h
    fn int33h_hide_cursor(&mut self) -> u32 {
        self.cpu.memory.mouse.hide_cursor();
        51
    }

    /// INT 33h, AX=0003h - Get mouse position and button status
    #[allow(dead_code)] // Called from handle_int33h
    fn int33h_get_position_and_buttons(&mut self) -> u32 {
        let (buttons, x, y) = self.cpu.memory.mouse.get_position_and_buttons();
        self.cpu.bx = buttons as u32;
        self.cpu.cx = x as u32;
        self.cpu.dx = y as u32;
        51
    }

    /// INT 33h, AX=0004h - Set mouse cursor position
    #[allow(dead_code)] // Called from handle_int33h
    fn int33h_set_position(&mut self) -> u32 {
        let x = self.cpu.cx as i16;
        let y = self.cpu.dx as i16;
        self.cpu.memory.mouse.set_position(x, y);
        51
    }

    /// INT 33h, AX=0005h - Get button press information
    #[allow(dead_code)] // Called from handle_int33h
    fn int33h_get_button_press_info(&mut self) -> u32 {
        let button = self.cpu.bx;
        let (buttons, count, x, y) = self.cpu.memory.mouse.get_button_press_info(button as u16);
        self.cpu.ax = buttons as u32;
        self.cpu.bx = count as u32;
        self.cpu.cx = x as u32;
        self.cpu.dx = y as u32;
        51
    }

    /// INT 33h, AX=0006h - Get button release information
    #[allow(dead_code)] // Called from handle_int33h
    fn int33h_get_button_release_info(&mut self) -> u32 {
        let button = self.cpu.bx;
        let (buttons, count, x, y) = self.cpu.memory.mouse.get_button_release_info(button as u16);
        self.cpu.ax = buttons as u32;
        self.cpu.bx = count as u32;
        self.cpu.cx = x as u32;
        self.cpu.dx = y as u32;
        51
    }

    /// INT 33h, AX=0007h - Set horizontal min/max position
    #[allow(dead_code)] // Called from handle_int33h
    fn int33h_set_horizontal_limits(&mut self) -> u32 {
        let min = self.cpu.cx as i16;
        let max = self.cpu.dx as i16;
        self.cpu.memory.mouse.set_horizontal_limits(min, max);
        51
    }

    /// INT 33h, AX=0008h - Set vertical min/max position
    #[allow(dead_code)] // Called from handle_int33h
    fn int33h_set_vertical_limits(&mut self) -> u32 {
        let min = self.cpu.cx as i16;
        let max = self.cpu.dx as i16;
        self.cpu.memory.mouse.set_vertical_limits(min, max);
        51
    }

    /// INT 33h, AX=000Fh - Set mickey to pixel ratio
    #[allow(dead_code)] // Called from handle_int33h
    fn int33h_set_mickey_ratio(&mut self) -> u32 {
        let horiz = self.cpu.cx;
        let vert = self.cpu.dx;
        self.cpu
            .memory
            .mouse
            .set_mickey_ratio(horiz as u16, vert as u16);
        51
    }

    /// INT 33h, AX=0024h - Get mouse driver version
    #[allow(dead_code)] // Called from handle_int33h
    fn int33h_get_driver_version(&mut self) -> u32 {
        let (version, mtype, irq) = self.cpu.memory.mouse.get_driver_version();
        self.cpu.bx = version as u32;
        self.cpu.cx = ((mtype as u32) << 8) | (irq as u32);
        51
    }

    /// Handle INT 15h - Extended BIOS services
    #[allow(dead_code)] // Called dynamically based on interrupt number
    fn handle_int15h(&mut self) -> u32 {
        // Skip the INT 15h instruction (2 bytes: 0xCD 0x15)
        self.cpu.ip = self.cpu.ip.wrapping_add(2);

        // Get function code from AH register
        let ah = ((self.cpu.ax >> 8) & 0xFF) as u8;

        match ah {
            0x24 => self.int15h_a20_gate_control(),
            0x41 => self.int15h_wait_on_external_event(),
            0x4F => self.int15h_keyboard_intercept(),
            0x86 => self.int15h_wait(),
            0x87 => self.int15h_extended_memory_block_move(),
            0x88 => self.int15h_get_extended_memory_size(),
            0xC0 => self.int15h_get_system_configuration(),
            0xC1 => self.int15h_get_extended_bios_data_area(),
            0xC2 => self.int15h_pointing_device(),
            0xD8 => self.int15h_eisa_functions(),
            0xE8 => {
                // Get Extended Memory Size (32-bit)
                let al = (self.cpu.ax & 0xFF) as u8;
                match al {
                    0x01 => self.int15h_get_extended_memory_size_e801(),
                    0x20 => self.int15h_query_system_address_map(),
                    _ => {
                        // Unsupported function
                        self.log_stub_interrupt(
                            0x15,
                            Some(ah),
                            &format!("Extended Services, AL=0x{:02X} (unsupported)", al),
                        );
                        self.set_carry_flag(true);
                        51
                    }
                }
            }
            _ => {
                // Unsupported function - log, set carry flag to indicate error
                self.log_stub_interrupt(
                    0x15,
                    Some(ah),
                    "Extended Services (unsupported subfunction)",
                );
                self.set_carry_flag(true);
                51
            }
        }
    }

    /// INT 15h, AH=87h - Move Extended Memory Block
    /// Copies data between conventional and extended memory
    #[allow(dead_code)] // Called from handle_int15h
    fn int15h_extended_memory_block_move(&mut self) -> u32 {
        // CX = number of words to move (WORDs, not bytes!)
        // ES:SI = pointer to Global Descriptor Table (GDT)
        //
        // GDT format (48 bytes):
        // 00h-0Fh: Dummy descriptor (not used)
        // 10h-17h: GDT descriptor (not used)
        // 18h-1Fh: Source segment descriptor
        // 20h-27h: Destination segment descriptor
        // 28h-2Fh: BIOS CS descriptor (not used)
        // 30h-37h: Stack segment descriptor (not used)
        //
        // Segment descriptor format:
        // +0: WORD - Segment limit (low 16 bits)
        // +2: WORD - Base address (low 16 bits)
        // +4: BYTE - Base address (bits 16-23)
        // +5: BYTE - Access rights
        // +6: WORD - Reserved

        let cx = self.cpu.cx;
        let es = self.cpu.es as u32;
        let si = self.cpu.si as u32;
        let gdt_addr = (es << 4) + si;

        // Read source descriptor (offset 0x18)
        let src_base_low = self.cpu.memory.read(gdt_addr + 0x1A) as u32
            | ((self.cpu.memory.read(gdt_addr + 0x1B) as u32) << 8);
        let src_base_high = self.cpu.memory.read(gdt_addr + 0x1C) as u32;
        let src_addr = (src_base_high << 16) | src_base_low;

        // Read destination descriptor (offset 0x20)
        let dst_base_low = self.cpu.memory.read(gdt_addr + 0x22) as u32
            | ((self.cpu.memory.read(gdt_addr + 0x23) as u32) << 8);
        let dst_base_high = self.cpu.memory.read(gdt_addr + 0x24) as u32;
        let dst_addr = (dst_base_high << 16) | dst_base_low;

        // Copy CX words (CX * 2 bytes)
        let byte_count = (cx as u32) * 2;

        if LogConfig::global().should_log(LogCategory::Bus, LogLevel::Debug) {
            eprintln!(
                "INT 15h AH=87h: Move {} words ({} bytes) from 0x{:08X} to 0x{:08X}",
                cx, byte_count, src_addr, dst_addr
            );
        }

        for i in 0..byte_count {
            let byte = self.cpu.memory.read(src_addr + i);
            self.cpu.memory.write(dst_addr + i, byte);
        }

        // Clear carry flag (success)
        self.set_carry_flag(false);

        // AH = 0 (success)
        self.cpu.ax &= 0x00FF;

        51
    }

    /// INT 15h, AH=41h - Wait on External Event (PS/2)
    #[allow(dead_code)] // Called from handle_int15h
    fn int15h_wait_on_external_event(&mut self) -> u32 {
        // AL = event type
        // This is a PS/2 BIOS function for event waiting
        // We don't support this, so just return with carry set (not supported)
        self.set_carry_flag(true);
        self.cpu.ax = (self.cpu.ax & 0x00FF) | (0x86 << 8); // AH = 0x86 (function not supported)
        51
    }

    /// INT 15h, AH=24h - A20 Gate Control (PS/2 and later)
    /// Critical for HIMEM.SYS and extended memory access
    #[allow(dead_code)] // Called from handle_int15h
    fn int15h_a20_gate_control(&mut self) -> u32 {
        let al = (self.cpu.ax & 0xFF) as u8;

        match al {
            0x00 => {
                // Disable A20 gate
                self.cpu.memory.xms.set_a20_enabled(false);
                self.cpu.ax = (self.cpu.ax & 0xFF00) | 0x00; // AH = 0x00 (success)
                self.set_carry_flag(false);
                emu_core::logging::log(LogCategory::Interrupts, LogLevel::Debug, || {
                    "INT 15h AH=24h AL=00h: Disable A20 gate (acknowledged)".to_string()
                });
            }
            0x01 => {
                // Enable A20 gate
                self.cpu.memory.xms.set_a20_enabled(true);
                self.cpu.ax = (self.cpu.ax & 0xFF00) | 0x00; // AH = 0x00 (success)
                self.set_carry_flag(false);
                emu_core::logging::log(LogCategory::Interrupts, LogLevel::Debug, || {
                    "INT 15h AH=24h AL=01h: Enable A20 gate (acknowledged)".to_string()
                });
            }
            0x02 => {
                // Get A20 gate status
                // Return current A20 state from XMS driver
                let a20_enabled = if self.cpu.memory.xms.is_a20_enabled() {
                    1
                } else {
                    0
                };
                self.cpu.ax = (0x00 << 8) | a20_enabled; // AH = 0x00 (success), AL = status
                self.set_carry_flag(false);
                emu_core::logging::log(LogCategory::Interrupts, LogLevel::Debug, || {
                    format!(
                        "INT 15h AH=24h AL=02h: Get A20 status (returning {})",
                        if a20_enabled == 1 {
                            "enabled"
                        } else {
                            "disabled"
                        }
                    )
                });
            }
            0x03 => {
                // Get A20 gate support
                // Return: BX = 0x0001 (supported via keyboard controller or port 92h)
                self.cpu.bx = 0x0001u32; // Supported
                self.cpu.ax = (self.cpu.ax & 0xFF00) | 0x00; // AH = 0x00 (success)
                self.set_carry_flag(false);
                emu_core::logging::log(LogCategory::Interrupts, LogLevel::Debug, || {
                    "INT 15h AH=24h AL=03h: Get A20 support (returning supported)".to_string()
                });
            }
            _ => {
                // Unsupported subfunction
                self.log_stub_interrupt(
                    0x15,
                    Some(0x24),
                    &format!("A20 Gate Control, AL=0x{:02X} (unsupported)", al),
                );
                self.set_carry_flag(true);
                self.cpu.ax = (self.cpu.ax & 0xFF00) | 0x86; // AH = 0x86 (function not supported)
            }
        }

        51 // Return cycles
    }

    /// INT 15h, AH=C0h - Get System Configuration
    #[allow(dead_code)] // Called from handle_int15h
    fn int15h_get_system_configuration(&mut self) -> u32 {
        // Return pointer to system configuration table in ES:BX
        // The table is located in BIOS ROM at F000:E000
        let table_seg = 0xF000; // BIOS ROM segment
        let table_offset = 0xE000; // Offset in ROM

        // The table is pre-populated in the BIOS ROM by generate_minimal_bios()
        // System configuration table format (10 bytes total):
        // 00h-01h: WORD - Number of bytes following (8)
        // 02h: BYTE - Model (0xFC = AT, 0xFE = XT, 0xFF = PC)
        // 03h: BYTE - Submodel
        // 04h: BYTE - BIOS revision level
        // 05h: BYTE - Feature information byte 1
        // 06h: BYTE - Feature information byte 2
        // 07h: BYTE - Feature information byte 3
        // 08h: BYTE - Feature information byte 4
        // 09h: BYTE - Feature information byte 5

        // Return ES:BX pointing to the table in ROM
        self.cpu.es = table_seg;
        self.cpu.bx = table_offset;

        // Clear carry flag (success)
        self.set_carry_flag(false);

        // AH = 0 (success)
        self.cpu.ax &= 0x00FF;

        if LogConfig::global().should_log(LogCategory::Interrupts, LogLevel::Debug) {
            // Read and display the actual table contents
            let table_addr = ((table_seg as u32) << 4) + (table_offset as u32);
            let byte_count = self.cpu.memory.read(table_addr) as u16
                | ((self.cpu.memory.read(table_addr + 1) as u16) << 8);
            eprintln!(
                "INT 15h AH=C0h: Returning ES={:04X} BX={:04X} (table at {:08X})",
                self.cpu.es, self.cpu.bx, table_addr
            );
            eprintln!("  Table contents:");
            eprintln!("    Byte count: {} (0x{:04X})", byte_count, byte_count);
            eprintln!("    Model: 0x{:02X}", self.cpu.memory.read(table_addr + 2));
            eprintln!(
                "    Submodel: 0x{:02X}",
                self.cpu.memory.read(table_addr + 3)
            );
            eprintln!(
                "    BIOS rev: 0x{:02X}",
                self.cpu.memory.read(table_addr + 4)
            );
            eprintln!(
                "    Feature 1: 0x{:02X}",
                self.cpu.memory.read(table_addr + 5)
            );
            eprintln!(
                "    Feature 2: 0x{:02X}",
                self.cpu.memory.read(table_addr + 6)
            );
            eprintln!(
                "    Feature 3: 0x{:02X}",
                self.cpu.memory.read(table_addr + 7)
            );
            eprintln!(
                "    Feature 4: 0x{:02X}",
                self.cpu.memory.read(table_addr + 8)
            );
            eprintln!(
                "    Feature 5: 0x{:02X}",
                self.cpu.memory.read(table_addr + 9)
            );
        }

        51
    }

    /// INT 15h, AH=88h - Get Extended Memory Size
    #[allow(dead_code)] // Called from handle_int15h
    fn int15h_get_extended_memory_size(&mut self) -> u32 {
        // Return extended memory size in KB (above 1MB)
        let extended_kb = self.cpu.memory.xms.total_extended_memory_kb();
        self.cpu.ax = extended_kb.min(0xFFFF) as u32;
        self.set_carry_flag(false);
        emu_core::logging::log(
            emu_core::logging::LogCategory::Interrupts,
            emu_core::logging::LogLevel::Trace,
            || {
                format!(
                    "INT 15h AH=88h: Returning extended memory size = {}KB",
                    extended_kb
                )
            },
        );
        51
    }

    /// INT 15h, AX=E801h - Get Extended Memory Size (alternate)
    #[allow(dead_code)] // Called from handle_int15h
    fn int15h_get_extended_memory_size_e801(&mut self) -> u32 {
        let extended_kb = self.cpu.memory.xms.total_extended_memory_kb();

        // AX/CX = extended memory between 1MB and 16MB in KB
        let mem_1_16mb = extended_kb.min(15 * 1024);
        // BX/DX = extended memory above 16MB in 64KB blocks
        let mem_above_16mb = if extended_kb > 15 * 1024 {
            (extended_kb - 15 * 1024) / 64
        } else {
            0
        };

        self.cpu.ax = mem_1_16mb as u32;
        self.cpu.bx = mem_above_16mb as u32;
        self.cpu.cx = mem_1_16mb as u32;
        self.cpu.dx = mem_above_16mb as u32;
        self.set_carry_flag(false);
        51
    }

    /// INT 15h, AX=E820h - Query System Address Map
    /// Returns memory map entries (CRITICAL for Linux boot)
    #[allow(dead_code)] // Called from handle_int15h
    fn int15h_query_system_address_map(&mut self) -> u32 {
        // Input: EAX = 0xE820, EBX = continuation value (0 for first call)
        //        ES:DI = buffer pointer (20 bytes minimum)
        //        ECX = buffer size (should be >= 20)
        //        EDX = signature 'SMAP' (0x534D4150)
        // Output: EAX = 'SMAP' (0x534D4150) on success
        //         EBX = continuation value (0 if last entry)
        //         ECX = bytes written (20 or 24)
        //         ES:DI = filled with entry
        //         CF = clear on success, set on error

        // Note: In 16-bit real mode BIOS, we can't verify the full 32-bit EDX signature (0x534D4150)
        // The caller (boot loader) is responsible for setting up the full 32-bit registers
        // We'll trust that if they called E820h, they set EDX correctly

        // Get continuation index from BX (lower 16 bits of EBX)
        let entry_index = self.cpu.bx;

        // Get memory sizes from bus
        let conventional_kb = self.cpu.memory.conventional_memory_kb();
        let extended_kb = self.cpu.memory.extended_memory_kb();

        // Define memory map entries
        // Entry format: base_low (4), base_high (4), length_low (4), length_high (4), type (4) = 20 bytes
        // Type 1 = Available RAM, Type 2 = Reserved/In use

        let entries: Vec<(u64, u64, u32)> = vec![
            // Entry 0: Conventional memory (0x00000000 - 0x0009FFFF) - 640KB max
            (0x00000000, (conventional_kb as u64) * 1024, 1), // Type 1 (available)
            // Entry 1: VGA/BIOS reserved (0x000A0000 - 0x000FFFFF) - 384KB
            (0x000A0000, 0x00060000, 2), // Type 2 (reserved)
            // Entry 2: Extended memory (1MB+)
            // Only include if we have extended memory
            (0x00100000, (extended_kb as u64) * 1024, 1), // Type 1 (available)
        ];

        // Filter out entries with zero length (if no extended memory)
        let valid_entry_index = if entry_index == 0 {
            0
        } else if entry_index == 1 {
            1
        } else if entry_index == 2 && extended_kb > 0 {
            2
        } else {
            // No more entries
            self.cpu.bx = 0;
            self.set_carry_flag(true);
            return 51;
        };

        // Check if this is the last valid entry
        let is_last = if extended_kb > 0 {
            valid_entry_index >= 2
        } else {
            valid_entry_index >= 1
        };

        if valid_entry_index >= entries.len() as u32 {
            // No more entries
            self.cpu.bx = 0;
            self.set_carry_flag(true);
            return 51;
        }

        let (base, length, entry_type) = entries[valid_entry_index as usize];

        // Write entry to ES:DI
        let buffer_addr = ((self.cpu.es as u32) << 4) + (self.cpu.di as u32);

        // Write base address (64-bit, little-endian)
        self.cpu.memory.write(buffer_addr + 0, (base & 0xFF) as u8);
        self.cpu
            .memory
            .write(buffer_addr + 1, ((base >> 8) & 0xFF) as u8);
        self.cpu
            .memory
            .write(buffer_addr + 2, ((base >> 16) & 0xFF) as u8);
        self.cpu
            .memory
            .write(buffer_addr + 3, ((base >> 24) & 0xFF) as u8);
        self.cpu
            .memory
            .write(buffer_addr + 4, ((base >> 32) & 0xFF) as u8);
        self.cpu
            .memory
            .write(buffer_addr + 5, ((base >> 40) & 0xFF) as u8);
        self.cpu
            .memory
            .write(buffer_addr + 6, ((base >> 48) & 0xFF) as u8);
        self.cpu
            .memory
            .write(buffer_addr + 7, ((base >> 56) & 0xFF) as u8);

        // Write length (64-bit, little-endian)
        self.cpu
            .memory
            .write(buffer_addr + 8, (length & 0xFF) as u8);
        self.cpu
            .memory
            .write(buffer_addr + 9, ((length >> 8) & 0xFF) as u8);
        self.cpu
            .memory
            .write(buffer_addr + 10, ((length >> 16) & 0xFF) as u8);
        self.cpu
            .memory
            .write(buffer_addr + 11, ((length >> 24) & 0xFF) as u8);
        self.cpu
            .memory
            .write(buffer_addr + 12, ((length >> 32) & 0xFF) as u8);
        self.cpu
            .memory
            .write(buffer_addr + 13, ((length >> 40) & 0xFF) as u8);
        self.cpu
            .memory
            .write(buffer_addr + 14, ((length >> 48) & 0xFF) as u8);
        self.cpu
            .memory
            .write(buffer_addr + 15, ((length >> 56) & 0xFF) as u8);

        // Write type (32-bit, little-endian)
        self.cpu
            .memory
            .write(buffer_addr + 16, (entry_type & 0xFF) as u8);
        self.cpu
            .memory
            .write(buffer_addr + 17, ((entry_type >> 8) & 0xFF) as u8);
        self.cpu
            .memory
            .write(buffer_addr + 18, ((entry_type >> 16) & 0xFF) as u8);
        self.cpu
            .memory
            .write(buffer_addr + 19, ((entry_type >> 24) & 0xFF) as u8);

        // Set return values (16-bit registers, caller manages 32-bit EAX/ECX)
        // EAX should be 0x534D4150 ('SMAP'), we set lower 16 bits
        self.cpu.ax = 0x4150u32; // Lower 16 bits of 'SMAP' (0x534D4150)
        self.cpu.cx = 20; // Bytes written (basic 20-byte format)

        // Set continuation value (0 if last entry, next index otherwise)
        if is_last {
            self.cpu.bx = 0; // No more entries
        } else {
            self.cpu.bx = entry_index + 1; // Next entry index
        }

        self.set_carry_flag(false); // Success

        emu_core::logging::log(LogCategory::Interrupts, LogLevel::Debug, || {
            format!(
                "INT 15h E820h: Entry {} - base=0x{:016X} len=0x{:016X} type={}",
                valid_entry_index, base, length, entry_type
            )
        });

        51
    }

    /// INT 15h, AH=4Fh - Keyboard Intercept
    /// Called by INT 09h to allow programs to intercept keystrokes
    #[allow(dead_code)] // Called from handle_int15h
    fn int15h_keyboard_intercept(&mut self) -> u32 {
        // AL = scan code
        // Programs can hook this to intercept keystrokes before they reach the keyboard buffer
        // If CF is set on return, the keystroke is discarded
        // If CF is clear, AL contains the (possibly modified) scan code to process

        // Default behavior: pass the scan code through unchanged
        self.set_carry_flag(false); // CF=0 means process the keystroke
        51
    }

    /// INT 15h, AH=86h - Wait (Delay)
    /// Used by programs to create precise delays
    #[allow(dead_code)] // Called from handle_int15h
    fn int15h_wait(&mut self) -> u32 {
        // CX:DX = interval in microseconds
        // This should delay for the specified time
        // For now, we just return success immediately
        // A proper implementation would track cycles or use a timer

        self.set_carry_flag(false); // CF=0 means success
        51
    }

    /// INT 15h, AH=C1h - Get Extended BIOS Data Area Segment
    /// Returns the segment address of the Extended BIOS Data Area (EBDA)
    #[allow(dead_code)] // Called from handle_int15h
    fn int15h_get_extended_bios_data_area(&mut self) -> u32 {
        // Returns: ES = segment of EBDA
        // The EBDA is typically located at the top of conventional memory
        // For compatibility, we return "not supported"

        self.set_carry_flag(true); // CF=1 means function not supported
        self.cpu.ax = (self.cpu.ax & 0x00FF) | (0x86 << 8); // AH = 0x86 (function not supported)
        51
    }

    /// INT 15h, AH=C2h - Pointing Device (PS/2 Mouse) Functions
    /// Provides access to PS/2 mouse operations
    #[allow(dead_code)] // Called from handle_int15h
    fn int15h_pointing_device(&mut self) -> u32 {
        // AL = subfunction
        // Various subfunctions for mouse initialization, status, etc.
        // For compatibility, we return "not supported"

        self.set_carry_flag(true); // CF=1 means function not supported
        self.cpu.ax = (self.cpu.ax & 0x00FF) | (0x86 << 8); // AH = 0x86 (function not supported)
        51
    }

    /// INT 15h, AH=D8h - EISA System Functions
    /// EISA (Extended Industry Standard Architecture) specific functions
    /// These are hardware-specific and not applicable to most systems
    #[allow(dead_code)] // Called from handle_int15h
    fn int15h_eisa_functions(&mut self) -> u32 {
        // AL = subfunction
        // EISA-specific operations for system configuration
        // These functions are only present on EISA systems
        // Return "not supported" without logging spam

        self.set_carry_flag(true); // CF=1 means function not supported
        self.cpu.ax = (self.cpu.ax & 0x00FF) | (0x86 << 8); // AH = 0x86 (function not supported)
        51
    }

    /// Handle INT 14h - Serial Port Services
    #[allow(dead_code)] // Called dynamically based on interrupt number
    fn handle_int14h(&mut self) -> u32 {
        // Skip the INT 14h instruction (2 bytes: 0xCD 0x14)
        self.cpu.ip = self.cpu.ip.wrapping_add(2);

        // Get function code from AH register
        let ah = ((self.cpu.ax >> 8) & 0xFF) as u8;

        // Log stub call
        self.log_stub_interrupt(0x14, Some(ah), "Serial Port Services (stub)");

        match ah {
            0x00 => {
                // Initialize serial port
                // AL = port parameters (baud rate, parity, stop bits, word length)
                // DX = port number (0-3)
                // Returns: AH = line status, AL = modem status
                self.cpu.ax = 0x0000u32; // Success
                51
            }
            0x01 => {
                // Transmit character
                // AL = character to send, DX = port number
                // Returns: AH = status (bit 7 = timeout if set)
                self.cpu.ax &= 0x00FF; // AH = 0 (success)
                51
            }
            0x02 => {
                // Receive character
                // DX = port number
                // Returns: AH = status, AL = received character
                self.cpu.ax = 0x0000u32; // No data available
                51
            }
            0x03 => {
                // Get port status
                // DX = port number
                // Returns: AH = line status, AL = modem status
                self.cpu.ax = 0x0000u32;
                51
            }
            _ => {
                // Unsupported function
                51
            }
        }
    }

    /// Handle INT 17h - Printer Services
    #[allow(dead_code)] // Called dynamically based on interrupt number
    fn handle_int17h(&mut self) -> u32 {
        // Skip the INT 17h instruction (2 bytes: 0xCD 0x17)
        self.cpu.ip = self.cpu.ip.wrapping_add(2);

        // Get function code from AH register
        let ah = ((self.cpu.ax >> 8) & 0xFF) as u8;

        // Log stub call
        self.log_stub_interrupt(0x17, Some(ah), "Printer Services (stub)");

        match ah {
            0x00 => {
                // Print character
                // AL = character to print, DX = printer number (0-2)
                // Returns: AH = printer status
                self.cpu.ax = (self.cpu.ax & 0x00FF) | 0x9000; // AH = 0x90 (ready, no errors)
                51
            }
            0x01 => {
                // Initialize printer
                // DX = printer number
                // Returns: AH = printer status
                self.cpu.ax = (self.cpu.ax & 0x00FF) | 0x9000; // AH = 0x90 (ready)
                51
            }
            0x02 => {
                // Get printer status
                // DX = printer number
                // Returns: AH = printer status
                self.cpu.ax = (self.cpu.ax & 0x00FF) | 0x9000; // AH = 0x90 (ready)
                51
            }
            _ => {
                // Unsupported function
                51
            }
        }
    }

    /// Handle INT 18h - Cassette BASIC / Boot Failure
    /// On modern systems, this indicates boot failure
    #[allow(dead_code)] // Called dynamically based on interrupt number
    fn handle_int18h(&mut self) -> u32 {
        // Skip the INT 18h instruction (2 bytes: 0xCD 0x18)
        self.cpu.ip = self.cpu.ip.wrapping_add(2);

        // Log stub call
        self.log_stub_interrupt(0x18, None, "Cassette BASIC / Boot Failure (stub)");

        // On IBM PC/XT/AT, this would start ROM BASIC
        // On clones and modern systems, this indicates no bootable disk
        // We'll just halt the system
        // The BIOS or bootloader calls this when boot fails

        // In a real implementation, this might:
        // 1. Display "No ROM BASIC" message
        // 2. Attempt network boot
        // 3. Halt the system

        // For emulator, just return (let the system continue)
        51
    }

    /// Handle INT 19h - Bootstrap Loader / System Reboot
    #[allow(dead_code)] // Called dynamically based on interrupt number
    fn handle_int19h(&mut self) -> u32 {
        // Skip the INT 19h instruction (2 bytes: 0xCD 0x19)
        self.cpu.ip = self.cpu.ip.wrapping_add(2);

        // Log stub call (should trigger reboot)
        self.log_stub_interrupt(
            0x19,
            None,
            "Bootstrap Loader / System Reboot (stub - should trigger reboot)",
        );

        // INT 19h is the bootstrap loader interrupt
        // Called by:
        // 1. BIOS after POST to load the OS
        // 2. Programs to reboot the computer (warm boot)

        // In a real system, this would:
        // 1. Reset hardware (except memory)
        // 2. Load boot sector from boot device to 0000:7C00
        // 3. Jump to boot sector (JMP 0000:7C00)

        // For emulator, we should trigger a system reboot
        // For now, just acknowledge (actual reboot would need system-level support)
        51
    }

    /// Handle INT 1Bh - Ctrl-Break Handler
    /// Called by INT 09h (keyboard interrupt) when Ctrl-Break is pressed
    #[allow(dead_code)] // Called dynamically based on interrupt number
    fn handle_int1bh(&mut self) -> u32 {
        // Skip the INT 1Bh instruction (2 bytes: 0xCD 0x1B)
        self.cpu.ip = self.cpu.ip.wrapping_add(2);

        // Log stub call
        self.log_stub_interrupt(0x1B, None, "Ctrl-Break Handler (stub)");

        // Ctrl-Break handler
        // Default BIOS handler does nothing and returns
        // Programs can hook this interrupt to handle Ctrl-Break
        51
    }

    /// Handle INT 1Ch - Timer Tick Handler
    /// Called by INT 08h (timer interrupt) on every tick (18.2 Hz)
    #[allow(dead_code)] // Called dynamically based on interrupt number
    fn handle_int1ch(&mut self) -> u32 {
        // Skip the INT 1Ch instruction (2 bytes: 0xCD 0x1C)
        self.cpu.ip = self.cpu.ip.wrapping_add(2);

        // Log stub call (this is called frequently, so maybe don't log by default)
        // Only log if EMU_TRACE_INTERRUPTS is set
        if LogConfig::global().should_log(LogCategory::Interrupts, LogLevel::Debug) {
            self.log_stub_interrupt(0x1C, None, "Timer Tick User Handler (stub)");
        }

        // User timer tick handler
        // Called 18.2065 times per second by INT 08h
        // Default BIOS handler is just IRET
        // Programs can hook this to execute code on every timer tick
        51
    }

    /// Handle INT 4Ah - Real-Time Clock Alarm
    /// Called when RTC alarm fires
    #[allow(dead_code)] // Called dynamically based on interrupt number
    fn handle_int4ah(&mut self) -> u32 {
        // Skip the INT 4Ah instruction (2 bytes: 0xCD 0x4A)
        self.cpu.ip = self.cpu.ip.wrapping_add(2);

        // Log stub call
        self.log_stub_interrupt(0x4A, None, "Real-Time Clock Alarm (stub)");

        // RTC Alarm handler
        // Called by RTC hardware when alarm time is reached
        // Default handler does nothing
        51
    }

    /// Handle INT 28h - DOS Idle Interrupt
    /// Called by DOS when waiting for keyboard input
    #[allow(dead_code)] // Called dynamically based on interrupt number
    fn handle_int28h(&mut self) -> u32 {
        // Skip the INT 28h instruction (2 bytes: 0xCD 0x28)
        self.cpu.ip = self.cpu.ip.wrapping_add(2);

        // DOS idle hook - TSRs and background programs can hook this
        // For emulator, just return immediately (noop)
        // DOS calls this in a loop while waiting for input
        51
    }

    /// Handle INT 29h - Fast Console Output
    /// Used by DOS for fast character output (bypasses normal INT 21h)
    #[allow(dead_code)] // Called dynamically based on interrupt number
    fn handle_int29h(&mut self) -> u32 {
        // Skip the INT 29h instruction (2 bytes: 0xCD 0x29)
        self.cpu.ip = self.cpu.ip.wrapping_add(2);

        // AL = character to output
        let character = (self.cpu.ax & 0xFF) as u8;

        // Use INT 10h teletype output to display the character
        // This is a fast path that DOS uses for console output
        // We directly call int10h_teletype_output() which reads AH=0Eh and AL=character
        // Save current AX to preserve it across the call
        let saved_ax = self.cpu.ax;

        // Set up AX for INT 10h AH=0Eh (teletype output), AL=character
        self.cpu.ax = 0x0E00 | (character as u32);
        self.int10h_teletype_output();

        // Restore original AX value
        self.cpu.ax = saved_ax;

        51
    }

    /// Handle INT 2Ah - Network Installation API
    /// This is a DOS network function used during initialization
    #[allow(dead_code)] // Called dynamically based on interrupt number
    fn handle_int2ah(&mut self) -> u32 {
        // Skip the INT 2Ah instruction (2 bytes: 0xCD 0x2A)
        self.cpu.ip = self.cpu.ip.wrapping_add(2);

        // Get function code from AH register
        let ah = ((self.cpu.ax >> 8) & 0xFF) as u8;

        // Log the call for debugging
        if LogConfig::global().should_log(LogCategory::Stubs, LogLevel::Debug) {
            eprintln!(
                "INT 0x2A AH=0x{:02X} called from {:04X}:{:04X}",
                ah,
                self.cpu.cs,
                self.cpu.ip.wrapping_sub(2)
            );
        }

        // Network Installation API stub
        // All functions return AL=0 (not installed/not supported)
        self.cpu.ax &= 0xFF00; // AL = 0 (not installed)
        self.set_carry_flag(true); // CF = 1 (error/not installed)

        51
    }

    /// Handle INT 2Fh - Multiplex interrupt
    #[allow(dead_code)] // Called dynamically based on interrupt number
    fn handle_int2fh(&mut self) -> u32 {
        // Skip the INT 2Fh instruction (2 bytes: 0xCD 0x2F)
        self.cpu.ip = self.cpu.ip.wrapping_add(2);

        // Get function code from AH register
        let ah = ((self.cpu.ax >> 8) & 0xFF) as u8;

        match ah {
            0x11 => self.int2fh_network_installation_check(),
            0x16 => self.int2fh_dpmi_installation_check(),
            0x43 => self.int2fh_xms_installation_check(),
            _ => {
                // Unsupported function - log and return
                self.log_stub_interrupt(
                    0x2F,
                    Some(ah),
                    "Multiplex Interrupt (unsupported subfunction)",
                );
                51
            }
        }
    }

    /// INT 2Fh, AH=11h - Network Redirector / Installation Check
    #[allow(dead_code)] // Called from handle_int2fh
    fn int2fh_network_installation_check(&mut self) -> u32 {
        // AL contains subfunction
        // This is used by DOS to check for network redirector
        // We don't support networking, so return "not installed"
        // AL = 0xFF means "not installed"
        self.cpu.ax = (self.cpu.ax & 0xFF00) | 0xFFu32;
        51
    }

    /// INT 2Fh, AH=43h - XMS Installation Check
    #[allow(dead_code)] // Called from handle_int2fh
    fn int2fh_xms_installation_check(&mut self) -> u32 {
        let al = (self.cpu.ax & 0xFF) as u8;

        match al {
            0x00 => {
                // XMS installation check
                if self.cpu.memory.xms.is_installed() {
                    self.cpu.ax = 0x80 << 8; // AL = 0x80 = installed
                } else {
                    self.cpu.ax = 0x00u32; // Not installed
                }
                51
            }
            0x10 => {
                // Get XMS driver address
                // Return ES:BX pointing to XMS driver entry point in BIOS ROM
                // The entry point is at F000:E010 in the BIOS ROM
                self.cpu.es = 0xF000; // BIOS segment
                self.cpu.bx = 0xE010u32; // Offset of XMS entry point
                51
            }
            _ => 51,
        }
    }

    /// INT 2Fh, AH=16h - DPMI Installation Check
    #[allow(dead_code)] // Called from handle_int2fh
    fn int2fh_dpmi_installation_check(&mut self) -> u32 {
        let al = (self.cpu.ax & 0xFF) as u8;

        match al {
            0x00 => {
                // DPMI installation check
                if self.cpu.memory.dpmi.is_installed() {
                    // DPMI is installed
                    self.cpu.ax = 0x0000u32; // AX = 0 (supported)
                    self.cpu.bx = 0x0001u32; // BX = flags (bit 0 = 32-bit support)
                    self.cpu.cx = self.cpu.memory.dpmi.processor_type() as u32; // Processor type
                    self.cpu.dx = self.cpu.memory.dpmi.version() as u32; // DPMI version (BCD)

                    // Entry point in ES:DI
                    self.cpu.es = self.cpu.memory.dpmi.entry_segment();
                    self.cpu.di = self.cpu.memory.dpmi.entry_offset() as u32;
                } else {
                    self.cpu.ax = 0x8001u32; // Not supported
                }
                51
            }
            _ => 51,
        }
    }

    /// Handle INT 31h - DPMI services
    #[allow(dead_code)] // Called dynamically based on interrupt number
    fn handle_int31h(&mut self) -> u32 {
        // Skip the INT 31h instruction (2 bytes: 0xCD 0x31)
        self.cpu.ip = self.cpu.ip.wrapping_add(2);

        // Get function code from AX register
        let ax = self.cpu.ax;

        match ax {
            0x0000 => self.int31h_allocate_descriptors(),
            0x0001 => self.int31h_free_descriptor(),
            0x0006 => self.int31h_get_segment_base(),
            0x0007 => self.int31h_set_segment_base(),
            0x0008 => self.int31h_get_segment_limit(),
            0x0009 => self.int31h_set_segment_limit(),
            0x0500 => self.int31h_get_free_memory_info(),
            0x0501 => self.int31h_allocate_memory(),
            0x0502 => self.int31h_free_memory(),
            _ => {
                // Unsupported function - set carry flag to indicate error
                self.set_carry_flag(true);
                self.cpu.ax = 0x8001u32; // Function not supported
                51
            }
        }
    }

    /// INT 31h, AX=0000h - Allocate LDT Descriptors
    #[allow(dead_code)] // Called from handle_int31h
    fn int31h_allocate_descriptors(&mut self) -> u32 {
        let count = self.cpu.cx;

        match self.cpu.memory.dpmi.allocate_descriptor(count as u16) {
            Ok(selector) => {
                self.cpu.ax = selector as u32; // Base selector
                self.set_carry_flag(false);
            }
            Err(err_code) => {
                self.cpu.ax = err_code as u32;
                self.set_carry_flag(true);
            }
        }
        51
    }

    /// INT 31h, AX=0001h - Free LDT Descriptor
    #[allow(dead_code)] // Called from handle_int31h
    fn int31h_free_descriptor(&mut self) -> u32 {
        let selector = self.cpu.bx;

        match self.cpu.memory.dpmi.free_descriptor(selector as u16) {
            Ok(()) => {
                self.set_carry_flag(false);
            }
            Err(err_code) => {
                self.cpu.ax = err_code as u32;
                self.set_carry_flag(true);
            }
        }
        51
    }

    /// INT 31h, AX=0006h - Get Segment Base Address
    #[allow(dead_code)] // Called from handle_int31h
    fn int31h_get_segment_base(&mut self) -> u32 {
        let selector = self.cpu.bx;

        match self.cpu.memory.dpmi.get_segment_base(selector as u16) {
            Ok(base) => {
                self.cpu.cx = ((base >> 16) & 0xFFFF) as u32; // High word
                self.cpu.dx = (base & 0xFFFF) as u32; // Low word
                self.set_carry_flag(false);
            }
            Err(err_code) => {
                self.cpu.ax = err_code as u32;
                self.set_carry_flag(true);
            }
        }
        51
    }

    /// INT 31h, AX=0007h - Set Segment Base Address
    #[allow(dead_code)] // Called from handle_int31h
    fn int31h_set_segment_base(&mut self) -> u32 {
        let selector = self.cpu.bx;
        let base = ((self.cpu.cx as u32) << 16) | (self.cpu.dx as u32);

        match self.cpu.memory.dpmi.set_segment_base(selector as u16, base) {
            Ok(()) => {
                self.set_carry_flag(false);
            }
            Err(err_code) => {
                self.cpu.ax = err_code as u32;
                self.set_carry_flag(true);
            }
        }
        51
    }

    /// INT 31h, AX=0008h - Get Segment Limit
    #[allow(dead_code)] // Called from handle_int31h
    fn int31h_get_segment_limit(&mut self) -> u32 {
        let selector = self.cpu.bx;

        match self.cpu.memory.dpmi.get_segment_limit(selector as u16) {
            Ok(limit) => {
                self.cpu.cx = ((limit >> 16) & 0xFFFF) as u32; // High word
                self.cpu.dx = (limit & 0xFFFF) as u32; // Low word
                self.set_carry_flag(false);
            }
            Err(err_code) => {
                self.cpu.ax = err_code as u32;
                self.set_carry_flag(true);
            }
        }
        51
    }

    /// INT 31h, AX=0009h - Set Segment Limit
    #[allow(dead_code)] // Called from handle_int31h
    fn int31h_set_segment_limit(&mut self) -> u32 {
        let selector = self.cpu.bx;
        let limit = ((self.cpu.cx as u32) << 16) | (self.cpu.dx as u32);

        match self
            .cpu
            .memory
            .dpmi
            .set_segment_limit(selector as u16, limit)
        {
            Ok(()) => {
                self.set_carry_flag(false);
            }
            Err(err_code) => {
                self.cpu.ax = err_code as u32;
                self.set_carry_flag(true);
            }
        }
        51
    }

    /// INT 31h, AX=0500h - Get Free Memory Information
    #[allow(dead_code)] // Called from handle_int31h
    fn int31h_get_free_memory_info(&mut self) -> u32 {
        let (largest, max_unlocked, _lockable) = self.cpu.memory.dpmi.get_free_memory_info();

        // ES:DI points to buffer that receives memory info structure
        // For simplicity, we'll just set registers
        self.cpu.bx = (largest & 0xFFFF) as u32;
        self.cpu.cx = ((largest >> 16) & 0xFFFF) as u32;
        self.cpu.dx = (max_unlocked & 0xFFFF) as u32;
        self.set_carry_flag(false);
        51
    }

    /// INT 31h, AX=0501h - Allocate Memory Block
    #[allow(dead_code)] // Called from handle_int31h
    fn int31h_allocate_memory(&mut self) -> u32 {
        let size = ((self.cpu.bx as u32) << 16) | (self.cpu.cx as u32);

        match self.cpu.memory.dpmi.allocate_memory(size) {
            Ok((linear_addr, handle)) => {
                self.cpu.bx = ((linear_addr >> 16) & 0xFFFF) as u32; // Linear address high
                self.cpu.cx = (linear_addr & 0xFFFF) as u32; // Linear address low
                self.cpu.si = ((handle >> 16) & 0xFFFF) as u32; // Handle high
                self.cpu.di = (handle & 0xFFFF) as u32; // Handle low
                self.set_carry_flag(false);
            }
            Err(err_code) => {
                self.cpu.ax = err_code as u32;
                self.set_carry_flag(true);
            }
        }
        51
    }

    /// INT 31h, AX=0502h - Free Memory Block
    #[allow(dead_code)] // Called from handle_int31h
    fn int31h_free_memory(&mut self) -> u32 {
        let handle = ((self.cpu.si as u32) << 16) | (self.cpu.di as u32);

        match self.cpu.memory.dpmi.free_memory(handle) {
            Ok(()) => {
                self.set_carry_flag(false);
            }
            Err(err_code) => {
                self.cpu.ax = err_code as u32;
                self.set_carry_flag(true);
            }
        }
        51
    }

    /// Get a reference to the bus
    pub fn bus(&self) -> &PcBus {
        &self.cpu.memory
    }

    /// Get a mutable reference to the bus
    pub fn bus_mut(&mut self) -> &mut PcBus {
        &mut self.cpu.memory
    }

    /// Unhalt the CPU (used when keyboard input arrives)
    /// This wakes up the CPU from a halted state caused by INT 16h AH=00h waiting for input
    pub fn unhalt(&mut self) {
        self.cpu.set_halted(false);
    }

    /// Get CPU register state for debugging/save states
    pub fn get_registers(&self) -> CpuRegisters {
        CpuRegisters {
            ax: self.cpu.ax,
            bx: self.cpu.bx,
            cx: self.cpu.cx,
            dx: self.cpu.dx,
            si: self.cpu.si,
            di: self.cpu.di,
            bp: self.cpu.bp,
            sp: self.cpu.sp,
            cs: self.cpu.cs,
            ds: self.cpu.ds,
            es: self.cpu.es,
            ss: self.cpu.ss,
            ip: self.cpu.ip,
            flags: self.cpu.flags,
            model: self.cpu.model(),
        }
    }

    /// Set CPU register state (for loading save states)
    ///
    /// Note: PC systems don't use save states, but this is kept for API compatibility
    #[allow(dead_code)]
    pub fn set_registers(&mut self, regs: &CpuRegisters) {
        self.cpu.ax = regs.ax;
        self.cpu.bx = regs.bx;
        self.cpu.cx = regs.cx;
        self.cpu.dx = regs.dx;
        self.cpu.si = regs.si;
        self.cpu.di = regs.di;
        self.cpu.bp = regs.bp;
        self.cpu.sp = regs.sp;
        self.cpu.cs = regs.cs;
        self.cpu.ds = regs.ds;
        self.cpu.es = regs.es;
        self.cpu.ss = regs.ss;
        self.cpu.ip = regs.ip;
        self.cpu.flags = regs.flags;
        self.cpu.set_model(regs.model);
    }

    /// Get total cycles executed
    #[allow(dead_code)]
    pub fn cycles(&self) -> u64 {
        self.cpu.cycles
    }

    /// Log a notice that a stub interrupt handler was called
    /// This helps identify which interrupts are not fully implemented
    fn log_stub_interrupt(&self, int_num: u8, ah: Option<u8>, description: &str) {
        if let Some(ah_val) = ah {
            eprintln!(
                "NOTICE: Stub interrupt handler called: INT 0x{:02X}, AH=0x{:02X} ({}) at {:04X}:{:04X}",
                int_num, ah_val, description, self.cpu.cs, self.cpu.ip
            );
        } else {
            eprintln!(
                "NOTICE: Stub interrupt handler called: INT 0x{:02X} ({}) at {:04X}:{:04X}",
                int_num, description, self.cpu.cs, self.cpu.ip
            );
        }
    }

    /// Convert PC scancode to ASCII character
    /// Handles Ctrl, Shift, and Alt modifiers
    fn scancode_to_ascii(&self, scancode: u8) -> u8 {
        use crate::keyboard::*;

        // Extended ASCII character constants for AltGr combinations
        const SECTION_SIGN: u8 = 0xA7; // §
        const CENT_SIGN: u8 = 0xA2; // ¢
        const AE_LIGATURE: u8 = 0xE6; // æ

        // Skip break codes (high bit set)
        if scancode & 0x80 != 0 {
            return 0;
        }

        // Check modifier states
        let ctrl_pressed = self.cpu.memory.keyboard.is_ctrl_pressed();
        let shift_pressed = self.cpu.memory.keyboard.is_shift_pressed();
        let altgr_pressed = self.cpu.memory.keyboard.is_altgr_pressed();

        // Handle Ctrl+key combinations for letters (generate control characters)
        if ctrl_pressed {
            match scancode {
                SCANCODE_A => return 0x01, // Ctrl+A
                SCANCODE_B => return 0x02, // Ctrl+B
                SCANCODE_C => return 0x03, // Ctrl+C (break)
                SCANCODE_D => return 0x04, // Ctrl+D
                SCANCODE_E => return 0x05, // Ctrl+E
                SCANCODE_F => return 0x06, // Ctrl+F
                SCANCODE_G => return 0x07, // Ctrl+G (bell)
                SCANCODE_H => return 0x08, // Ctrl+H (backspace)
                SCANCODE_I => return 0x09, // Ctrl+I (tab)
                SCANCODE_J => return 0x0A, // Ctrl+J (line feed)
                SCANCODE_K => return 0x0B, // Ctrl+K
                SCANCODE_L => return 0x0C, // Ctrl+L (form feed)
                SCANCODE_M => return 0x0D, // Ctrl+M (carriage return)
                SCANCODE_N => return 0x0E, // Ctrl+N
                SCANCODE_O => return 0x0F, // Ctrl+O
                SCANCODE_P => return 0x10, // Ctrl+P
                SCANCODE_Q => return 0x11, // Ctrl+Q
                SCANCODE_R => return 0x12, // Ctrl+R
                SCANCODE_S => return 0x13, // Ctrl+S
                SCANCODE_T => return 0x14, // Ctrl+T
                SCANCODE_U => return 0x15, // Ctrl+U
                SCANCODE_V => return 0x16, // Ctrl+V
                SCANCODE_W => return 0x17, // Ctrl+W
                SCANCODE_X => return 0x18, // Ctrl+X
                SCANCODE_Y => return 0x19, // Ctrl+Y
                SCANCODE_Z => return 0x1A, // Ctrl+Z (suspend)
                _ => {}
            }
        }

        // Handle AltGr combinations (for international characters)
        // Common US International keyboard layout
        if altgr_pressed {
            match scancode {
                SCANCODE_2 => return b'@',
                SCANCODE_3 => return b'#',
                SCANCODE_4 => return b'$',
                SCANCODE_5 => return SECTION_SIGN, // § (section sign)
                SCANCODE_7 => return b'{',
                SCANCODE_8 => return b'[',
                SCANCODE_9 => return b']',
                SCANCODE_0 => return b'}',
                SCANCODE_EQUALS => return b'\\',
                SCANCODE_Q => return AE_LIGATURE, // æ (ae ligature)
                SCANCODE_E => return CENT_SIGN,   // ¢ (cent sign)
                SCANCODE_BACKSLASH => return b'|',
                _ => {}
            }
        }

        // Handle letters with Shift modifier
        if shift_pressed {
            match scancode {
                SCANCODE_A => return b'A',
                SCANCODE_B => return b'B',
                SCANCODE_C => return b'C',
                SCANCODE_D => return b'D',
                SCANCODE_E => return b'E',
                SCANCODE_F => return b'F',
                SCANCODE_G => return b'G',
                SCANCODE_H => return b'H',
                SCANCODE_I => return b'I',
                SCANCODE_J => return b'J',
                SCANCODE_K => return b'K',
                SCANCODE_L => return b'L',
                SCANCODE_M => return b'M',
                SCANCODE_N => return b'N',
                SCANCODE_O => return b'O',
                SCANCODE_P => return b'P',
                SCANCODE_Q => return b'Q',
                SCANCODE_R => return b'R',
                SCANCODE_S => return b'S',
                SCANCODE_T => return b'T',
                SCANCODE_U => return b'U',
                SCANCODE_V => return b'V',
                SCANCODE_W => return b'W',
                SCANCODE_X => return b'X',
                SCANCODE_Y => return b'Y',
                SCANCODE_Z => return b'Z',
                // Shifted number keys
                SCANCODE_0 => return b')',
                SCANCODE_1 => return b'!',
                SCANCODE_2 => return b'@',
                SCANCODE_3 => return b'#',
                SCANCODE_4 => return b'$',
                SCANCODE_5 => return b'%',
                SCANCODE_6 => return b'^',
                SCANCODE_7 => return b'&',
                SCANCODE_8 => return b'*',
                SCANCODE_9 => return b'(',
                // Shifted punctuation
                SCANCODE_COMMA => return b'<',
                SCANCODE_PERIOD => return b'>',
                SCANCODE_SLASH => return b'?',
                SCANCODE_SEMICOLON => return b':',
                SCANCODE_APOSTROPHE => return b'"',
                SCANCODE_LEFT_BRACKET => return b'{',
                SCANCODE_RIGHT_BRACKET => return b'}',
                SCANCODE_BACKSLASH => return b'|',
                SCANCODE_MINUS => return b'_',
                SCANCODE_EQUALS => return b'+',
                SCANCODE_BACKTICK => return b'~',
                _ => {}
            }
        }

        // Normal character mapping (without modifiers)
        match scancode {
            SCANCODE_A => b'a',
            SCANCODE_B => b'b',
            SCANCODE_C => b'c',
            SCANCODE_D => b'd',
            SCANCODE_E => b'e',
            SCANCODE_F => b'f',
            SCANCODE_G => b'g',
            SCANCODE_H => b'h',
            SCANCODE_I => b'i',
            SCANCODE_J => b'j',
            SCANCODE_K => b'k',
            SCANCODE_L => b'l',
            SCANCODE_M => b'm',
            SCANCODE_N => b'n',
            SCANCODE_O => b'o',
            SCANCODE_P => b'p',
            SCANCODE_Q => b'q',
            SCANCODE_R => b'r',
            SCANCODE_S => b's',
            SCANCODE_T => b't',
            SCANCODE_U => b'u',
            SCANCODE_V => b'v',
            SCANCODE_W => b'w',
            SCANCODE_X => b'x',
            SCANCODE_Y => b'y',
            SCANCODE_Z => b'z',
            SCANCODE_0 => b'0',
            SCANCODE_1 => b'1',
            SCANCODE_2 => b'2',
            SCANCODE_3 => b'3',
            SCANCODE_4 => b'4',
            SCANCODE_5 => b'5',
            SCANCODE_6 => b'6',
            SCANCODE_7 => b'7',
            SCANCODE_8 => b'8',
            SCANCODE_9 => b'9',
            SCANCODE_SPACE => b' ',
            SCANCODE_ENTER => b'\r', // Carriage return (0x0D) - DOS expects CR not LF
            SCANCODE_BACKSPACE => 0x08,
            SCANCODE_TAB => b'\t',
            SCANCODE_ESC => 0x1B,
            SCANCODE_COMMA => b',',
            SCANCODE_PERIOD => b'.',
            SCANCODE_SLASH => b'/',
            SCANCODE_SEMICOLON => b';',
            SCANCODE_APOSTROPHE => b'\'',
            SCANCODE_LEFT_BRACKET => b'[',
            SCANCODE_RIGHT_BRACKET => b']',
            SCANCODE_BACKSLASH => b'\\',
            SCANCODE_MINUS => b'-',
            SCANCODE_EQUALS => b'=',
            SCANCODE_BACKTICK => b'`',
            _ => 0, // No ASCII equivalent
        }
    }
}

/// Convert PC scancode to ASCII character (simplified mapping)
/// This is kept for compatibility but should not be used internally
#[allow(dead_code)]
fn scancode_to_ascii(scancode: u8) -> u8 {
    use crate::keyboard::*;

    // Skip break codes (high bit set)
    if scancode & 0x80 != 0 {
        return 0;
    }

    match scancode {
        SCANCODE_A => b'a',
        SCANCODE_B => b'b',
        SCANCODE_C => b'c',
        SCANCODE_D => b'd',
        SCANCODE_E => b'e',
        SCANCODE_F => b'f',
        SCANCODE_G => b'g',
        SCANCODE_H => b'h',
        SCANCODE_I => b'i',
        SCANCODE_J => b'j',
        SCANCODE_K => b'k',
        SCANCODE_L => b'l',
        SCANCODE_M => b'm',
        SCANCODE_N => b'n',
        SCANCODE_O => b'o',
        SCANCODE_P => b'p',
        SCANCODE_Q => b'q',
        SCANCODE_R => b'r',
        SCANCODE_S => b's',
        SCANCODE_T => b't',
        SCANCODE_U => b'u',
        SCANCODE_V => b'v',
        SCANCODE_W => b'w',
        SCANCODE_X => b'x',
        SCANCODE_Y => b'y',
        SCANCODE_Z => b'z',
        SCANCODE_0 => b'0',
        SCANCODE_1 => b'1',
        SCANCODE_2 => b'2',
        SCANCODE_3 => b'3',
        SCANCODE_4 => b'4',
        SCANCODE_5 => b'5',
        SCANCODE_6 => b'6',
        SCANCODE_7 => b'7',
        SCANCODE_8 => b'8',
        SCANCODE_9 => b'9',
        SCANCODE_SPACE => b' ',
        SCANCODE_ENTER => b'\r', // Carriage return (0x0D) - DOS expects CR not LF
        SCANCODE_BACKSPACE => 0x08,
        SCANCODE_TAB => b'\t',
        SCANCODE_ESC => 0x1B,
        SCANCODE_COMMA => b',',
        SCANCODE_PERIOD => b'.',
        SCANCODE_SLASH => b'/',
        SCANCODE_SEMICOLON => b';',
        SCANCODE_APOSTROPHE => b'\'',
        SCANCODE_LEFT_BRACKET => b'[',
        SCANCODE_RIGHT_BRACKET => b']',
        SCANCODE_BACKSLASH => b'\\',
        SCANCODE_MINUS => b'-',
        SCANCODE_EQUALS => b'=',
        SCANCODE_BACKTICK => b'`',
        _ => 0, // No ASCII equivalent
    }
}

/// CPU register state for save/load
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CpuRegisters {
    pub ax: u32,
    pub bx: u32,
    pub cx: u32,
    pub dx: u32,
    pub si: u32,
    pub di: u32,
    pub bp: u32,
    pub sp: u32,
    pub cs: u16,
    pub ds: u16,
    pub es: u16,
    pub ss: u16,
    pub ip: u32,
    pub flags: u32,
    #[serde(default)]
    pub model: CpuModel,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpu_initialization() {
        let bus = PcBus::new();
        let cpu = PcCpu::new(bus);

        // Check PC boot state
        assert_eq!(cpu.cpu.cs, 0xFFFF);
        assert_eq!(cpu.cpu.ip, 0x0000u32);
        assert_eq!(cpu.cpu.ss, 0x0000);
        assert_eq!(cpu.cpu.sp, 0xFFFEu32);
    }

    #[test]
    fn test_cpu_reset() {
        let bus = PcBus::new();
        let mut cpu = PcCpu::new(bus);

        // Modify some registers
        cpu.cpu.ax = 0x1234;
        cpu.cpu.cs = 0x0100;

        cpu.reset();

        // Should be back to boot state
        assert_eq!(cpu.cpu.ax, 0x0000u32);
        assert_eq!(cpu.cpu.cs, 0xFFFF);
        assert_eq!(cpu.cpu.ip, 0x0000u32);
    }

    #[test]
    fn test_register_save_load() {
        let bus = PcBus::new();
        let mut cpu = PcCpu::new(bus);

        cpu.cpu.ax = 0x1234;
        cpu.cpu.bx = 0x5678;
        cpu.cpu.cs = 0xABCD;

        let regs = cpu.get_registers();
        assert_eq!(regs.ax, 0x1234);
        assert_eq!(regs.bx, 0x5678);
        assert_eq!(regs.cs, 0xABCD);

        cpu.reset();
        assert_eq!(cpu.cpu.ax, 0x0000u32);

        cpu.set_registers(&regs);
        assert_eq!(cpu.cpu.ax, 0x1234u32);
        assert_eq!(cpu.cpu.bx, 0x5678u32);
        assert_eq!(cpu.cpu.cs, 0xABCD);
    }

    #[test]
    fn test_int13h_reset() {
        let bus = PcBus::new();
        let mut cpu = PcCpu::new(bus);

        // Move CPU to a RAM location where we can write test code
        cpu.cpu.cs = 0x0000;
        cpu.cpu.ip = 0x1000;

        // Setup: Write INT 13h instruction at current IP
        let cs = cpu.cpu.cs;
        let ip = cpu.cpu.ip;
        let addr = ((cs as u32) << 4) + (ip as u32);

        cpu.cpu.memory.write(addr, 0xCD); // INT
        cpu.cpu.memory.write(addr + 1, 0x13); // 13h

        // Setup registers for AH=00h (reset)
        cpu.cpu.ax = 0x0000; // AH=00h (reset)
        cpu.cpu.dx = 0x0080; // DL=80h (hard drive)

        // Execute INT 13h
        let cycles = cpu.step();

        // Should have executed and advanced IP by 2
        assert_eq!(cpu.cpu.ip, ip.wrapping_add(2));

        // AH should be 0 (success)
        assert_eq!((cpu.cpu.ax >> 8) & 0xFF, 0x00);

        // Carry flag should be clear
        assert_eq!(cpu.cpu.flags & 0x0001, 0);

        // Should have taken cycles
        assert!(cycles > 0);
    }

    #[test]
    fn test_int13h_read_sectors_no_disk() {
        let bus = PcBus::new();
        let mut cpu = PcCpu::new(bus);

        // Move CPU to RAM
        cpu.cpu.cs = 0x0000;
        cpu.cpu.ip = 0x1000;

        // Setup: Write INT 13h instruction
        let cs = cpu.cpu.cs;
        let ip = cpu.cpu.ip;
        let addr = ((cs as u32) << 4) + (ip as u32);

        cpu.cpu.memory.write(addr, 0xCD); // INT
        cpu.cpu.memory.write(addr + 1, 0x13); // 13h

        // Setup registers for AH=02h (read sectors)
        cpu.cpu.ax = 0x0201; // AH=02h (read), AL=01 (1 sector)
        cpu.cpu.cx = 0x0001; // CH=00, CL=01 (cylinder 0, sector 1)
        cpu.cpu.dx = 0x0000; // DH=00 (head 0), DL=00 (floppy A)
        cpu.cpu.es = 0x0000;
        cpu.cpu.bx = 0x7C00; // Buffer at 0x0000:0x7C00

        // Execute INT 13h
        cpu.step();

        // Should fail with timeout (no disk mounted)
        assert_eq!((cpu.cpu.ax >> 8) & 0xFF, 0x80); // Status = timeout

        // Carry flag should be set (error)
        assert_eq!(cpu.cpu.flags & 0x0001, 1);
    }

    #[test]
    fn test_int13h_read_sectors_success() {
        let mut bus = PcBus::new();

        // Create a floppy image with test data
        let mut floppy = vec![0; 1474560]; // 1.44MB

        // Fill first sector with test pattern
        for (i, byte) in floppy.iter_mut().enumerate().take(512) {
            *byte = (i % 256) as u8;
        }

        bus.mount_floppy_a(floppy);

        let mut cpu = PcCpu::new(bus);

        // Move CPU to RAM
        cpu.cpu.cs = 0x0000;
        cpu.cpu.ip = 0x1000;

        // Setup: Write INT 13h instruction
        let cs = cpu.cpu.cs;
        let ip = cpu.cpu.ip;
        let addr = ((cs as u32) << 4) + (ip as u32);

        cpu.cpu.memory.write(addr, 0xCD); // INT
        cpu.cpu.memory.write(addr + 1, 0x13); // 13h

        // Setup registers for AH=02h (read sectors)
        cpu.cpu.ax = 0x0201; // AH=02h (read), AL=01 (1 sector)
        cpu.cpu.cx = 0x0001; // CH=00, CL=01 (cylinder 0, sector 1)
        cpu.cpu.dx = 0x0000; // DH=00 (head 0), DL=00 (floppy A)
        cpu.cpu.es = 0x0000;
        cpu.cpu.bx = 0x7C00; // Buffer at 0x0000:0x7C00

        // Execute INT 13h
        cpu.step();

        // Should succeed
        assert_eq!((cpu.cpu.ax >> 8) & 0xFF, 0x00); // Status = success
        assert_eq!(cpu.cpu.ax & 0xFF, 0x01); // AL = sectors read

        // Carry flag should be clear
        assert_eq!(cpu.cpu.flags & 0x0001, 0);

        // Verify data was copied to buffer
        let buffer_addr = 0x7C00;
        assert_eq!(cpu.cpu.memory.read(buffer_addr), 0);
        assert_eq!(cpu.cpu.memory.read(buffer_addr + 255), 255);
        assert_eq!(cpu.cpu.memory.read(buffer_addr + 256), 0);
    }

    #[test]
    fn test_int13h_write_sectors() {
        let mut bus = PcBus::new();

        // Create a blank floppy image
        let floppy = vec![0; 1474560]; // 1.44MB
        bus.mount_floppy_a(floppy);

        let mut cpu = PcCpu::new(bus);

        // Move CPU to RAM
        cpu.cpu.cs = 0x0000;
        cpu.cpu.ip = 0x1000;

        // Setup: Write test data to memory at 0x0000:0x7C00
        let buffer_addr = 0x7C00;
        for i in 0..512 {
            cpu.cpu.memory.write(buffer_addr + i, (i % 256) as u8);
        }

        // Setup: Write INT 13h instruction
        let cs = cpu.cpu.cs;
        let ip = cpu.cpu.ip;
        let addr = ((cs as u32) << 4) + (ip as u32);

        cpu.cpu.memory.write(addr, 0xCD); // INT
        cpu.cpu.memory.write(addr + 1, 0x13); // 13h

        // Setup registers for AH=03h (write sectors)
        cpu.cpu.ax = 0x0301; // AH=03h (write), AL=01 (1 sector)
        cpu.cpu.cx = 0x0001; // CH=00, CL=01 (cylinder 0, sector 1)
        cpu.cpu.dx = 0x0000; // DH=00 (head 0), DL=00 (floppy A)
        cpu.cpu.es = 0x0000;
        cpu.cpu.bx = 0x7C00; // Buffer at 0x0000:0x7C00

        // Execute INT 13h
        cpu.step();

        // Should succeed
        assert_eq!((cpu.cpu.ax >> 8) & 0xFF, 0x00); // Status = success
        assert_eq!(cpu.cpu.ax & 0xFF, 0x01); // AL = sectors written

        // Carry flag should be clear
        assert_eq!(cpu.cpu.flags & 0x0001, 0);

        // Verify data was written to floppy
        let floppy = cpu.cpu.memory.floppy_a().unwrap();
        assert_eq!(floppy[0], 0);
        assert_eq!(floppy[255], 255);
        assert_eq!(floppy[256], 0);
    }

    #[test]
    fn test_int13h_get_drive_params_floppy() {
        let mut bus = PcBus::new();
        // Mount a floppy disk so the drive exists
        bus.mount_floppy_a(vec![0u8; 1474560]); // 1.44MB floppy
        let mut cpu = PcCpu::new(bus);

        // Move CPU to RAM
        cpu.cpu.cs = 0x0000;
        cpu.cpu.ip = 0x1000;

        // Setup: Write INT 13h instruction
        let cs = cpu.cpu.cs;
        let ip = cpu.cpu.ip;
        let addr = ((cs as u32) << 4) + (ip as u32);

        cpu.cpu.memory.write(addr, 0xCD); // INT
        cpu.cpu.memory.write(addr + 1, 0x13); // 13h

        // Setup registers for AH=08h (get drive params)
        cpu.cpu.ax = 0x0800; // AH=08h (get drive params)
        cpu.cpu.dx = 0x0000; // DL=00 (floppy A)

        // Execute INT 13h
        cpu.step();

        // Should succeed
        assert_eq!((cpu.cpu.ax >> 8) & 0xFF, 0x00); // Status = success

        // Carry flag should be clear
        assert_eq!(cpu.cpu.flags & 0x0001, 0);

        // Check returned parameters (1.44MB floppy: 80 cylinders, 18 sectors, 2 heads)
        let ch = (cpu.cpu.cx >> 8) & 0xFF;
        let cl = cpu.cpu.cx & 0xFF;
        let sectors = cl & 0x3F;
        let cylinder_high = (cl & 0xC0) >> 6;
        let cylinder = (cylinder_high << 8) | ch;

        assert_eq!(cylinder, 79); // Max cylinder (0-based)
        assert_eq!(sectors, 18); // Sectors per track

        let dh = (cpu.cpu.dx >> 8) & 0xFF;
        assert_eq!(dh, 1); // Max head (0-based, so 2 heads = 0-1)

        // BL should indicate floppy type
        let bl = cpu.cpu.bx & 0xFF;
        assert_eq!(bl, 0x04); // 1.44MB floppy
    }

    #[test]
    fn test_int13h_get_drive_params_hard_drive() {
        let mut bus = PcBus::new();
        // Mount a hard drive so it exists
        bus.mount_hard_drive(vec![0u8; 10 * 1024 * 1024]); // 10MB hard drive
        let mut cpu = PcCpu::new(bus);

        // Move CPU to RAM
        cpu.cpu.cs = 0x0000;
        cpu.cpu.ip = 0x1000;

        // Setup: Write INT 13h instruction
        let cs = cpu.cpu.cs;
        let ip = cpu.cpu.ip;
        let addr = ((cs as u32) << 4) + (ip as u32);

        cpu.cpu.memory.write(addr, 0xCD); // INT
        cpu.cpu.memory.write(addr + 1, 0x13); // 13h

        // Setup registers for AH=08h (get drive params)
        cpu.cpu.ax = 0x0800; // AH=08h (get drive params)
        cpu.cpu.dx = 0x0080; // DL=80h (hard drive C)

        // Execute INT 13h
        cpu.step();

        // Should succeed
        assert_eq!((cpu.cpu.ax >> 8) & 0xFF, 0x00); // Status = success

        // Carry flag should be clear
        assert_eq!(cpu.cpu.flags & 0x0001, 0);

        // Check returned parameters (10MB drive: 306 cylinders, 17 sectors, 4 heads)
        let ch = (cpu.cpu.cx >> 8) & 0xFF;
        let cl = cpu.cpu.cx & 0xFF;
        let sectors = cl & 0x3F;
        let cylinder_high = (cl & 0xC0) >> 6;
        let cylinder = (cylinder_high << 8) | ch;

        assert_eq!(cylinder, 305); // Max cylinder (0-based)
        assert_eq!(sectors, 17); // Sectors per track

        let dh = (cpu.cpu.dx >> 8) & 0xFF;
        assert_eq!(dh, 3); // Max head (0-based, so 4 heads = 0-3)

        // Check DL = number of hard drives (should be 1)
        let dl = cpu.cpu.dx & 0xFF;
        assert_eq!(dl, 1); // 1 hard drive installed
    }

    #[test]
    fn test_int13h_get_drive_params_returns_correct_drive_count() {
        // Test that INT 13h AH=08h returns correct drive count in DL
        let mut bus = PcBus::new();

        // Mount a hard drive
        bus.mount_hard_drive(vec![0u8; 10 * 1024 * 1024]);

        // Manually set the hard drive count in BDA
        // The system should read this value
        bus.write(BDA_HARD_DRIVE_COUNT, 1); // 1 hard drive

        let mut cpu = PcCpu::new(bus);

        // Move CPU to RAM
        cpu.cpu.cs = 0x0000;
        cpu.cpu.ip = 0x1000;

        // Setup: Write INT 13h instruction
        let addr = 0x1000;
        cpu.cpu.memory.write(addr, 0xCD); // INT
        cpu.cpu.memory.write(addr + 1, 0x13); // 13h

        // Setup registers for AH=08h (get drive params), DL=80h (hard drive)
        cpu.cpu.ax = 0x0800;
        cpu.cpu.dx = 0x0080;

        // Execute INT 13h
        cpu.step();

        // Check that DL contains the number of hard drives
        let dl = cpu.cpu.dx & 0xFF;
        assert_eq!(dl, 1, "DL should contain number of hard drives (1)");

        // Test with floppy drives
        let mut bus2 = PcBus::new();
        bus2.mount_floppy_a(vec![0u8; 1474560]); // Mount floppy A
        let mut cpu2 = PcCpu::new(bus2);

        // Move CPU to RAM
        cpu2.cpu.cs = 0x0000;
        cpu2.cpu.ip = 0x1000;

        // Setup: Write INT 13h instruction
        let addr2 = 0x1000;
        cpu2.cpu.memory.write(addr2, 0xCD); // INT
        cpu2.cpu.memory.write(addr2 + 1, 0x13); // 13h

        // Setup registers for AH=08h (get drive params), DL=00h (floppy A)
        cpu2.cpu.ax = 0x0800;
        cpu2.cpu.dx = 0x0000;

        // Execute INT 13h
        cpu2.step();

        // Check that DL contains the number of floppy drives (should be 1)
        let dl_floppy = cpu2.cpu.dx & 0xFF;
        assert_eq!(
            dl_floppy, 1,
            "DL should contain number of floppy drives (1)"
        );
    }

    #[test]
    fn test_int13h_unsupported_function() {
        let bus = PcBus::new();
        let mut cpu = PcCpu::new(bus);

        // Move CPU to RAM
        cpu.cpu.cs = 0x0000;
        cpu.cpu.ip = 0x1000;

        // Setup: Write INT 13h instruction
        let cs = cpu.cpu.cs;
        let ip = cpu.cpu.ip;
        let addr = ((cs as u32) << 4) + (ip as u32);

        cpu.cpu.memory.write(addr, 0xCD); // INT
        cpu.cpu.memory.write(addr + 1, 0x13); // 13h

        // Setup registers for unsupported function (AH=FFh)
        cpu.cpu.ax = 0xFF00; // AH=FFh (unsupported)

        // Execute INT 13h
        cpu.step();

        // Should fail with invalid function
        assert_eq!((cpu.cpu.ax >> 8) & 0xFF, 0x01); // Status = invalid function

        // Carry flag should be set (error)
        assert_eq!(cpu.cpu.flags & 0x0001, 1);
    }

    #[test]
    fn test_int13h_read_multiple_sectors() {
        let mut bus = PcBus::new();

        // Create a floppy image with test data
        let mut floppy = vec![0; 1474560]; // 1.44MB

        // Fill first 3 sectors with different patterns
        for sector in 0..3 {
            for i in 0..512 {
                floppy[sector * 512 + i] = ((sector * 100 + i) % 256) as u8;
            }
        }

        bus.mount_floppy_a(floppy);

        let mut cpu = PcCpu::new(bus);

        // Move CPU to RAM
        cpu.cpu.cs = 0x0000;
        cpu.cpu.ip = 0x1000;

        // Setup: Write INT 13h instruction
        let cs = cpu.cpu.cs;
        let ip = cpu.cpu.ip;
        let addr = ((cs as u32) << 4) + (ip as u32);

        cpu.cpu.memory.write(addr, 0xCD); // INT
        cpu.cpu.memory.write(addr + 1, 0x13); // 13h

        // Setup registers for AH=02h (read 3 sectors)
        cpu.cpu.ax = 0x0203; // AH=02h (read), AL=03 (3 sectors)
        cpu.cpu.cx = 0x0001; // CH=00, CL=01 (cylinder 0, sector 1)
        cpu.cpu.dx = 0x0000; // DH=00 (head 0), DL=00 (floppy A)
        cpu.cpu.es = 0x0000;
        cpu.cpu.bx = 0x7C00; // Buffer at 0x0000:0x7C00

        // Execute INT 13h
        cpu.step();

        // Should succeed
        assert_eq!((cpu.cpu.ax >> 8) & 0xFF, 0x00); // Status = success
        assert_eq!(cpu.cpu.ax & 0xFF, 0x03); // AL = 3 sectors read

        // Verify all 3 sectors were read
        let buffer_addr = 0x7C00;
        assert_eq!(cpu.cpu.memory.read(buffer_addr), 0); // Sector 0, byte 0
        assert_eq!(cpu.cpu.memory.read(buffer_addr + 512), 100); // Sector 1, byte 0
        assert_eq!(cpu.cpu.memory.read(buffer_addr + 1024), 200); // Sector 2, byte 0
    }

    #[test]
    fn test_int16h_read_keystroke() {
        use crate::keyboard::SCANCODE_A;

        let mut bus = PcBus::new();

        // Add a key to the keyboard buffer
        bus.keyboard.key_press(SCANCODE_A);

        let mut cpu = PcCpu::new(bus);

        // Move CPU to RAM
        cpu.cpu.cs = 0x0000;
        cpu.cpu.ip = 0x1000;

        // Setup: Write INT 16h instruction
        let cs = cpu.cpu.cs;
        let ip = cpu.cpu.ip;
        let addr = ((cs as u32) << 4) + (ip as u32);

        cpu.cpu.memory.write(addr, 0xCD); // INT
        cpu.cpu.memory.write(addr + 1, 0x16); // 16h

        // Setup registers for AH=00h (read keystroke)
        cpu.cpu.ax = 0x0000; // AH=00h

        // Execute INT 16h
        cpu.step();

        // Should return scancode in AH, ASCII in AL
        assert_eq!((cpu.cpu.ax >> 8) & 0xFF, SCANCODE_A as u32); // AH = scancode
        assert_eq!(cpu.cpu.ax & 0xFF, b'a' as u32); // AL = ASCII 'a'
    }

    #[test]
    fn test_int16h_check_keystroke_available() {
        use crate::keyboard::SCANCODE_B;

        let mut bus = PcBus::new();

        // Add a key to the keyboard buffer
        bus.keyboard.key_press(SCANCODE_B);

        let mut cpu = PcCpu::new(bus);

        // Move CPU to RAM
        cpu.cpu.cs = 0x0000;
        cpu.cpu.ip = 0x1000;

        // Setup: Write INT 16h instruction
        let cs = cpu.cpu.cs;
        let ip = cpu.cpu.ip;
        let addr = ((cs as u32) << 4) + (ip as u32);

        cpu.cpu.memory.write(addr, 0xCD); // INT
        cpu.cpu.memory.write(addr + 1, 0x16); // 16h

        // Setup registers for AH=01h (check keystroke)
        cpu.cpu.ax = 0x0100; // AH=01h

        // Execute INT 16h
        cpu.step();

        // Should return scancode in AH, ASCII in AL, and ZF=0
        assert_eq!((cpu.cpu.ax >> 8) & 0xFF, SCANCODE_B as u32); // AH = scancode
        assert_eq!(cpu.cpu.ax & 0xFF, b'b' as u32); // AL = ASCII 'b'
        assert_eq!(cpu.cpu.flags & 0x0040, 0); // ZF = 0 (key available)

        // Key should still be in buffer (peek doesn't consume)
        assert!(cpu.cpu.memory.keyboard.has_data());
    }

    #[test]
    fn test_int16h_check_keystroke_not_available() {
        let bus = PcBus::new();
        let mut cpu = PcCpu::new(bus);

        // Move CPU to RAM
        cpu.cpu.cs = 0x0000;
        cpu.cpu.ip = 0x1000;

        // Setup: Write INT 16h instruction
        let cs = cpu.cpu.cs;
        let ip = cpu.cpu.ip;
        let addr = ((cs as u32) << 4) + (ip as u32);

        cpu.cpu.memory.write(addr, 0xCD); // INT
        cpu.cpu.memory.write(addr + 1, 0x16); // 16h

        // Setup registers for AH=01h (check keystroke)
        cpu.cpu.ax = 0x0100; // AH=01h

        // Execute INT 16h
        cpu.step();

        // Should return 0 and ZF=1 (no key)
        assert_eq!(cpu.cpu.ax, 0x0000u32);
        assert_eq!(cpu.cpu.flags & 0x0040, 0x0040); // ZF = 1 (no key available)
    }

    #[test]
    fn test_int16h_multiple_keystrokes() {
        use crate::keyboard::{SCANCODE_E, SCANCODE_H, SCANCODE_L, SCANCODE_O};

        let mut bus = PcBus::new();

        // Add multiple keys to simulate typing "HELLO"
        bus.keyboard.key_press(SCANCODE_H);
        bus.keyboard.key_press(SCANCODE_E);
        bus.keyboard.key_press(SCANCODE_L);
        bus.keyboard.key_press(SCANCODE_L);
        bus.keyboard.key_press(SCANCODE_O);

        let mut cpu = PcCpu::new(bus);

        // Read each keystroke
        let expected = vec![
            (SCANCODE_H, b'h'),
            (SCANCODE_E, b'e'),
            (SCANCODE_L, b'l'),
            (SCANCODE_L, b'l'),
            (SCANCODE_O, b'o'),
        ];

        for (expected_scan, expected_ascii) in expected {
            // Move CPU to RAM
            cpu.cpu.cs = 0x0000;
            cpu.cpu.ip = 0x1000;

            // Setup: Write INT 16h instruction
            let cs = cpu.cpu.cs;
            let ip = cpu.cpu.ip;
            let addr = ((cs as u32) << 4) + (ip as u32);

            cpu.cpu.memory.write(addr, 0xCD); // INT
            cpu.cpu.memory.write(addr + 1, 0x16); // 16h

            // Setup registers for AH=00h (read keystroke)
            cpu.cpu.ax = 0x0000;

            // Execute INT 16h
            cpu.step();

            // Verify scancode and ASCII
            assert_eq!((cpu.cpu.ax >> 8) & 0xFF, expected_scan as u32);
            assert_eq!(cpu.cpu.ax & 0xFF, expected_ascii as u32);
        }

        // Buffer should now be empty
        assert!(!cpu.cpu.memory.keyboard.has_data());
    }

    #[test]
    fn test_int10h_select_active_page() {
        let bus = PcBus::new();
        let mut cpu = PcCpu::new(bus);

        // Move CPU to RAM
        cpu.cpu.cs = 0x0000;
        cpu.cpu.ip = 0x1000;

        // Write INT 10h instruction
        let addr = ((cpu.cpu.cs as u32) << 4) + (cpu.cpu.ip as u32);
        cpu.cpu.memory.write(addr, 0xCD); // INT
        cpu.cpu.memory.write(addr + 1, 0x10); // 10h

        // AH=05h, AL=3 (select page 3)
        cpu.cpu.ax = 0x0503;

        // Execute INT 10h
        cpu.step();

        // Verify page was stored in BIOS data area
        assert_eq!(cpu.cpu.memory.read(0x462), 3);
    }

    #[test]
    fn test_int10h_write_char_only() {
        let bus = PcBus::new();
        let mut cpu = PcCpu::new(bus);

        // Move CPU to RAM
        cpu.cpu.cs = 0x0000;
        cpu.cpu.ip = 0x1000;

        // Write INT 10h instruction
        let addr = ((cpu.cpu.cs as u32) << 4) + (cpu.cpu.ip as u32);
        cpu.cpu.memory.write(addr, 0xCD); // INT
        cpu.cpu.memory.write(addr + 1, 0x10); // 10h

        // Set cursor to known position (row 5, col 10)
        cpu.cpu.memory.write(0x450, 10); // Column
        cpu.cpu.memory.write(0x451, 5); // Row

        // Preset some data at position with attributes
        let video_offset = (5 * 80 + 10) * 2;
        let video_addr = 0xB8000 + video_offset;
        cpu.cpu.memory.write(video_addr, b'A'); // Preset char
        cpu.cpu.memory.write(video_addr + 1, 0x1F); // Preset attribute
        cpu.cpu.memory.write(video_addr + 2, b'B');
        cpu.cpu.memory.write(video_addr + 3, 0x2E);
        cpu.cpu.memory.write(video_addr + 4, b'C');
        cpu.cpu.memory.write(video_addr + 5, 0x3D);

        // AH=0Ah, AL='X', BH=0 (page), CX=3 (count)
        cpu.cpu.ax = 0x0A58; // 'X'
        cpu.cpu.bx = 0x0000;
        cpu.cpu.cx = 3;

        // Execute INT 10h
        cpu.step();

        // Verify characters were written without changing attributes
        assert_eq!(cpu.cpu.memory.read(video_addr), b'X');
        assert_eq!(cpu.cpu.memory.read(video_addr + 1), 0x1F); // Attribute unchanged
        assert_eq!(cpu.cpu.memory.read(video_addr + 2), b'X');
        assert_eq!(cpu.cpu.memory.read(video_addr + 3), 0x2E); // Attribute unchanged
        assert_eq!(cpu.cpu.memory.read(video_addr + 4), b'X');
        assert_eq!(cpu.cpu.memory.read(video_addr + 5), 0x3D); // Attribute unchanged
    }

    #[test]
    fn test_int10h_write_pixel() {
        let bus = PcBus::new();
        let mut cpu = PcCpu::new(bus);

        // Move CPU to RAM
        cpu.cpu.cs = 0x0000;
        cpu.cpu.ip = 0x1000;

        // Write INT 10h instruction
        let addr = ((cpu.cpu.cs as u32) << 4) + (cpu.cpu.ip as u32);
        cpu.cpu.memory.write(addr, 0xCD); // INT
        cpu.cpu.memory.write(addr + 1, 0x10); // 10h

        // AH=0Ch, AL=14 (color yellow in mode 13h), CX=100 (x), DX=50 (y)
        cpu.cpu.ax = 0x0C0E; // Color 14
        cpu.cpu.cx = 100; // X
        cpu.cpu.dx = 50; // Y

        // Execute INT 10h
        cpu.step();

        // Verify pixel was written (Mode 13h: 0xA0000 + y*320 + x)
        let offset = 50 * 320 + 100;
        let pixel_addr = 0xA0000 + offset;
        assert_eq!(cpu.cpu.memory.read(pixel_addr), 14);
    }

    #[test]
    fn test_int10h_read_pixel() {
        let bus = PcBus::new();
        let mut cpu = PcCpu::new(bus);

        // Move CPU to RAM
        cpu.cpu.cs = 0x0000;
        cpu.cpu.ip = 0x1000;

        // Write test pixel first
        let offset = 75 * 320 + 150;
        let pixel_addr = 0xA0000 + offset;
        cpu.cpu.memory.write(pixel_addr, 42); // Write color 42

        // Write INT 10h instruction
        let addr = ((cpu.cpu.cs as u32) << 4) + (cpu.cpu.ip as u32);
        cpu.cpu.memory.write(addr, 0xCD); // INT
        cpu.cpu.memory.write(addr + 1, 0x10); // 10h

        // AH=0Dh, CX=150 (x), DX=75 (y)
        cpu.cpu.ax = 0x0D00;
        cpu.cpu.cx = 150; // X
        cpu.cpu.dx = 75; // Y

        // Execute INT 10h
        cpu.step();

        // Verify color was returned in AL
        assert_eq!(cpu.cpu.ax & 0xFF, 42);
    }

    #[test]
    fn test_int10h_scroll_up_clear() {
        let bus = PcBus::new();
        let mut cpu = PcCpu::new(bus);

        // Move CPU to RAM
        cpu.cpu.cs = 0x0000;
        cpu.cpu.ip = 0x1000;

        // Fill window with test data
        for row in 5..=10 {
            for col in 10..=20 {
                let offset = (row * 80 + col) * 2;
                let video_addr = 0xB8000 + offset;
                cpu.cpu.memory.write(video_addr, b'X');
                cpu.cpu.memory.write(video_addr + 1, 0x07);
            }
        }

        // Write INT 10h instruction
        let addr = ((cpu.cpu.cs as u32) << 4) + (cpu.cpu.ip as u32);
        cpu.cpu.memory.write(addr, 0xCD); // INT
        cpu.cpu.memory.write(addr + 1, 0x10); // 10h

        // AH=06h, AL=0 (clear), BH=0x1F (attribute), CH,CL=5,10 (top), DH,DL=10,20 (bottom)
        cpu.cpu.ax = 0x0600; // Clear
        cpu.cpu.bx = 0x1F00; // Attribute
        cpu.cpu.cx = 0x050A; // Row 5, Col 10
        cpu.cpu.dx = 0x0A14; // Row 10, Col 20

        // Execute INT 10h
        cpu.step();

        // Verify window was cleared with new attribute
        for row in 5..=10 {
            for col in 10..=20 {
                let offset = (row * 80 + col) * 2;
                let video_addr = 0xB8000 + offset;
                assert_eq!(cpu.cpu.memory.read(video_addr), b' ');
                assert_eq!(cpu.cpu.memory.read(video_addr + 1), 0x1F);
            }
        }
    }

    #[test]
    fn test_int10h_scroll_up_lines() {
        let bus = PcBus::new();
        let mut cpu = PcCpu::new(bus);

        // Move CPU to RAM
        cpu.cpu.cs = 0x0000;
        cpu.cpu.ip = 0x1000;

        // Fill window with identifiable data (row number as character)
        for row in 0..=5 {
            for col in 0..=10 {
                let offset = (row * 80 + col) * 2;
                let video_addr = 0xB8000 + offset;
                cpu.cpu.memory.write(video_addr, b'0' + (row as u8));
                cpu.cpu.memory.write(video_addr + 1, 0x07);
            }
        }

        // Write INT 10h instruction
        let addr = ((cpu.cpu.cs as u32) << 4) + (cpu.cpu.ip as u32);
        cpu.cpu.memory.write(addr, 0xCD); // INT
        cpu.cpu.memory.write(addr + 1, 0x10); // 10h

        // AH=06h, AL=2 (scroll 2 lines), BH=0x07, CH,CL=0,0, DH,DL=5,10
        cpu.cpu.ax = 0x0602; // Scroll 2 lines
        cpu.cpu.bx = 0x0700;
        cpu.cpu.cx = 0x0000; // Top-left: 0,0
        cpu.cpu.dx = 0x050A; // Bottom-right: 5,10

        // Execute INT 10h
        cpu.step();

        // Verify lines were scrolled up by 2
        // Row 0 should now contain what was in row 2 ('2')
        let video_addr = 0xB8000;
        assert_eq!(cpu.cpu.memory.read(video_addr), b'2');

        // Row 1 should now contain what was in row 3 ('3')
        let video_addr = 0xB8000 + 80 * 2;
        assert_eq!(cpu.cpu.memory.read(video_addr), b'3');

        // Rows 4-5 should be filled with spaces
        let video_addr = 0xB8000 + (4 * 80) * 2;
        assert_eq!(cpu.cpu.memory.read(video_addr), b' ');
        let video_addr = 0xB8000 + (5 * 80) * 2;
        assert_eq!(cpu.cpu.memory.read(video_addr), b' ');
    }

    #[test]
    fn test_int10h_scroll_down_lines() {
        let bus = PcBus::new();
        let mut cpu = PcCpu::new(bus);

        // Move CPU to RAM
        cpu.cpu.cs = 0x0000;
        cpu.cpu.ip = 0x1000;

        // Fill window with identifiable data (row number as character)
        for row in 0..=5 {
            for col in 0..=10 {
                let offset = (row * 80 + col) * 2;
                let video_addr = 0xB8000 + offset;
                cpu.cpu.memory.write(video_addr, b'0' + (row as u8));
                cpu.cpu.memory.write(video_addr + 1, 0x07);
            }
        }

        // Write INT 10h instruction
        let addr = ((cpu.cpu.cs as u32) << 4) + (cpu.cpu.ip as u32);
        cpu.cpu.memory.write(addr, 0xCD); // INT
        cpu.cpu.memory.write(addr + 1, 0x10); // 10h

        // AH=07h, AL=2 (scroll 2 lines), BH=0x07, CH,CL=0,0, DH,DL=5,10
        cpu.cpu.ax = 0x0702; // Scroll down 2 lines
        cpu.cpu.bx = 0x0700;
        cpu.cpu.cx = 0x0000; // Top-left: 0,0
        cpu.cpu.dx = 0x050A; // Bottom-right: 5,10

        // Execute INT 10h
        cpu.step();

        // Verify lines were scrolled down by 2
        // Rows 0-1 should be filled with spaces
        let video_addr = 0xB8000;
        assert_eq!(cpu.cpu.memory.read(video_addr), b' ');
        let video_addr = 0xB8000 + 80 * 2;
        assert_eq!(cpu.cpu.memory.read(video_addr), b' ');

        // Row 2 should now contain what was in row 0 ('0')
        let video_addr = 0xB8000 + (2 * 80) * 2;
        assert_eq!(cpu.cpu.memory.read(video_addr), b'0');

        // Row 3 should now contain what was in row 1 ('1')
        let video_addr = 0xB8000 + (3 * 80) * 2;
        assert_eq!(cpu.cpu.memory.read(video_addr), b'1');
    }

    #[test]
    fn test_int10h_display_combination() {
        let bus = PcBus::new();
        let mut cpu = PcCpu::new(bus);

        // Move CPU to RAM
        cpu.cpu.cs = 0x0000;
        cpu.cpu.ip = 0x1000;

        // Write INT 10h instruction
        let addr = ((cpu.cpu.cs as u32) << 4) + (cpu.cpu.ip as u32);
        cpu.cpu.memory.write(addr, 0xCD); // INT
        cpu.cpu.memory.write(addr + 1, 0x10); // 10h

        // AH=1Ah, AL=00h (get display combination)
        cpu.cpu.ax = 0x1A00;

        // Execute INT 10h
        cpu.step();

        // Verify function supported (AL=1Ah) and VGA returned
        assert_eq!(cpu.cpu.ax & 0xFF, 0x1A);
        assert_eq!(cpu.cpu.bx, 0x0008u32); // VGA with color display
    }

    #[test]
    fn test_int21h_open_file() {
        let bus = PcBus::new();
        let mut cpu = PcCpu::new(bus);

        // Move CPU to RAM
        cpu.cpu.cs = 0x0000;
        cpu.cpu.ip = 0x1000;

        // Write INT 21h instruction
        let addr = ((cpu.cpu.cs as u32) << 4) + (cpu.cpu.ip as u32);
        cpu.cpu.memory.write(addr, 0xCD); // INT
        cpu.cpu.memory.write(addr + 1, 0x21); // 21h

        // Write filename at DS:DX
        let filename_addr = 0x2000u32;
        let filename = b"IO.SYS\0";
        for (i, &byte) in filename.iter().enumerate() {
            cpu.cpu.memory.write(filename_addr + i as u32, byte);
        }

        // AH=3Dh (open file), AL=00h (read only)
        cpu.cpu.ax = 0x3D00;
        cpu.cpu.ds = 0x0000;
        cpu.cpu.dx = 0x2000;

        // Execute INT 21h
        cpu.step();

        // Verify carry flag is set (file not found)
        assert!(cpu.get_carry_flag());
        // Verify error code is 02h (file not found)
        assert_eq!(cpu.cpu.ax & 0xFF, 0x02);
    }

    #[test]
    fn test_int21h_create_file() {
        let bus = PcBus::new();
        let mut cpu = PcCpu::new(bus);

        // Move CPU to RAM
        cpu.cpu.cs = 0x0000;
        cpu.cpu.ip = 0x1000;

        // Write INT 21h instruction
        let addr = ((cpu.cpu.cs as u32) << 4) + (cpu.cpu.ip as u32);
        cpu.cpu.memory.write(addr, 0xCD); // INT
        cpu.cpu.memory.write(addr + 1, 0x21); // 21h

        // Write filename at DS:DX
        let filename_addr = 0x2000u32;
        let filename = b"TEST.TXT\0";
        for (i, &byte) in filename.iter().enumerate() {
            cpu.cpu.memory.write(filename_addr + i as u32, byte);
        }

        // AH=3Ch (create file), CX=0 (normal attributes)
        cpu.cpu.ax = 0x3C00;
        cpu.cpu.cx = 0x0000;
        cpu.cpu.ds = 0x0000;
        cpu.cpu.dx = 0x2000;

        // Execute INT 21h
        cpu.step();

        // Verify carry flag is set (not implemented, returns error)
        assert!(cpu.get_carry_flag());
    }

    #[test]
    fn test_int21h_read_file_invalid_handle() {
        let bus = PcBus::new();
        let mut cpu = PcCpu::new(bus);

        // Move CPU to RAM
        cpu.cpu.cs = 0x0000;
        cpu.cpu.ip = 0x1000;

        // Write INT 21h instruction
        let addr = ((cpu.cpu.cs as u32) << 4) + (cpu.cpu.ip as u32);
        cpu.cpu.memory.write(addr, 0xCD); // INT
        cpu.cpu.memory.write(addr + 1, 0x21); // 21h

        // AH=3Fh (read file), BX=file handle >= 5, CX=bytes to read
        cpu.cpu.ax = 0x3F00;
        cpu.cpu.bx = 0x0005; // file handle (user file, not supported)
        cpu.cpu.cx = 0x0040; // 64 bytes
        cpu.cpu.ds = 0x0000;
        cpu.cpu.dx = 0x3000; // buffer address

        // Execute INT 21h
        cpu.step();

        // Verify error (CF set) since file handles >= 5 are not supported
        assert!(cpu.get_carry_flag());
        // Verify error code (invalid handle)
        assert_eq!(cpu.cpu.ax, DOS_ERROR_INVALID_HANDLE as u32);
    }

    #[test]
    fn test_int21h_read_file_stdin() {
        let bus = PcBus::new();
        let mut cpu = PcCpu::new(bus);

        // Move CPU to RAM
        cpu.cpu.cs = 0x0000;
        cpu.cpu.ip = 0x1000;

        // Write INT 21h instruction
        let addr = ((cpu.cpu.cs as u32) << 4) + (cpu.cpu.ip as u32);
        cpu.cpu.memory.write(addr, 0xCD); // INT
        cpu.cpu.memory.write(addr + 1, 0x21); // 21h

        // AH=3Fh (read file), BX=0 (stdin), CX=bytes to read
        cpu.cpu.ax = 0x3F00;
        cpu.cpu.bx = 0x0000; // stdin
        cpu.cpu.cx = 0x0040; // 64 bytes
        cpu.cpu.ds = 0x0000;
        cpu.cpu.dx = 0x3000; // buffer address

        // Execute INT 21h
        cpu.step();

        // Verify no error (CF clear) for stdin
        assert!(!cpu.get_carry_flag());
        // Verify 0 bytes read (EOF/no input available)
        assert_eq!(cpu.cpu.ax, 0x0000u32);
    }

    #[test]
    fn test_int21h_write_file_invalid_handle() {
        let bus = PcBus::new();
        let mut cpu = PcCpu::new(bus);

        // Move CPU to RAM
        cpu.cpu.cs = 0x0000;
        cpu.cpu.ip = 0x1000;

        // Write INT 21h instruction
        let addr = ((cpu.cpu.cs as u32) << 4) + (cpu.cpu.ip as u32);
        cpu.cpu.memory.write(addr, 0xCD); // INT
        cpu.cpu.memory.write(addr + 1, 0x21); // 21h

        // AH=40h (write file), BX=file handle >= 5, CX=bytes to write
        cpu.cpu.ax = 0x4000;
        cpu.cpu.bx = 0x0005; // file handle (user file, not supported)
        cpu.cpu.cx = 0x0020; // 32 bytes
        cpu.cpu.ds = 0x0000;
        cpu.cpu.dx = 0x3000; // buffer address

        // Execute INT 21h
        cpu.step();

        // Verify error (CF set) since file handles >= 5 are not supported
        assert!(cpu.get_carry_flag());
        // Verify error code (invalid handle)
        assert_eq!(cpu.cpu.ax, DOS_ERROR_INVALID_HANDLE as u32);
    }

    #[test]
    fn test_int21h_write_file_stdout() {
        let bus = PcBus::new();
        let mut cpu = PcCpu::new(bus);

        // Move CPU to RAM
        cpu.cpu.cs = 0x0000;
        cpu.cpu.ip = 0x1000;

        // Write INT 21h instruction
        let addr = ((cpu.cpu.cs as u32) << 4) + (cpu.cpu.ip as u32);
        cpu.cpu.memory.write(addr, 0xCD); // INT
        cpu.cpu.memory.write(addr + 1, 0x21); // 21h

        // AH=40h (write file), BX=1 (stdout), CX=bytes to write
        cpu.cpu.ax = 0x4000;
        cpu.cpu.bx = 0x0001; // stdout
        cpu.cpu.cx = 0x0020; // 32 bytes
        cpu.cpu.ds = 0x0000;
        cpu.cpu.dx = 0x3000; // buffer address

        // Execute INT 21h
        cpu.step();

        // Verify no error (CF clear) for stdout
        assert!(!cpu.get_carry_flag());
        // Verify all bytes reported as written
        assert_eq!(cpu.cpu.ax, 0x0020u32);
    }

    #[test]
    fn test_int21h_close_file_invalid_handle() {
        let bus = PcBus::new();
        let mut cpu = PcCpu::new(bus);

        // Move CPU to RAM
        cpu.cpu.cs = 0x0000;
        cpu.cpu.ip = 0x1000;

        // Write INT 21h instruction
        let addr = ((cpu.cpu.cs as u32) << 4) + (cpu.cpu.ip as u32);
        cpu.cpu.memory.write(addr, 0xCD); // INT
        cpu.cpu.memory.write(addr + 1, 0x21); // 21h

        // AH=3Eh (close file), BX=file handle >= 5
        cpu.cpu.ax = 0x3E00;
        cpu.cpu.bx = 0x0005; // file handle (user file, not supported)

        // Execute INT 21h
        cpu.step();

        // Verify error (CF set) since file handles >= 5 are not supported
        assert!(cpu.get_carry_flag());
        // Verify error code (invalid handle)
        assert_eq!(cpu.cpu.ax, DOS_ERROR_INVALID_HANDLE as u32);
    }

    #[test]
    fn test_int21h_close_file_stdout() {
        let bus = PcBus::new();
        let mut cpu = PcCpu::new(bus);

        // Move CPU to RAM
        cpu.cpu.cs = 0x0000;
        cpu.cpu.ip = 0x1000;

        // Write INT 21h instruction
        let addr = ((cpu.cpu.cs as u32) << 4) + (cpu.cpu.ip as u32);
        cpu.cpu.memory.write(addr, 0xCD); // INT
        cpu.cpu.memory.write(addr + 1, 0x21); // 21h

        // AH=3Eh (close file), BX=1 (stdout)
        cpu.cpu.ax = 0x3E00;
        cpu.cpu.bx = 0x0001; // stdout (standard handle, cannot be closed but no error)

        // Execute INT 21h
        cpu.step();

        // Verify no error (CF clear) for standard handles
        assert!(!cpu.get_carry_flag());
    }

    #[test]
    fn test_int11h_equipment_list() {
        use crate::bus::VideoAdapterType;

        // Test with default configuration (CGA, no floppies)
        let bus = PcBus::new();
        let mut cpu = PcCpu::new(bus);

        // Move CPU to RAM
        cpu.cpu.cs = 0x0000;
        cpu.cpu.ip = 0x1000;

        // Write INT 11h instruction
        let addr = ((cpu.cpu.cs as u32) << 4) + (cpu.cpu.ip as u32);
        cpu.cpu.memory.write(addr, 0xCD); // INT
        cpu.cpu.memory.write(addr + 1, 0x11); // 11h

        // Execute INT 11h
        cpu.step();

        // Equipment flags should reflect no floppy drives (bit 0 = 0)
        // CGA 80x25 (bits 4-5 = 10)
        let equipment = cpu.cpu.ax;
        assert_eq!(equipment & 0x01, 0x00); // No floppy drives
        assert_eq!((equipment >> 4) & 0x03, 0b10); // CGA 80x25

        // Test with floppy drives mounted
        let mut bus = PcBus::new();
        bus.mount_floppy_a(vec![0; 1440 * 1024]); // 1.44MB floppy
        let mut cpu = PcCpu::new(bus);

        // Move CPU to RAM
        cpu.cpu.cs = 0x0000;
        cpu.cpu.ip = 0x1000;

        // Write INT 11h instruction
        let addr = ((cpu.cpu.cs as u32) << 4) + (cpu.cpu.ip as u32);
        cpu.cpu.memory.write(addr, 0xCD); // INT
        cpu.cpu.memory.write(addr + 1, 0x11); // 11h

        // Execute INT 11h
        cpu.step();

        // Equipment flags should reflect one floppy drive
        let equipment = cpu.cpu.ax;
        assert_eq!(equipment & 0x01, 0x01); // Floppy drive installed
        assert_eq!((equipment >> 6) & 0x03, 0b00); // 1 floppy drive (0b00 = 1 drive)

        // Test with VGA adapter
        let mut bus = PcBus::new();
        bus.set_video_adapter_type(VideoAdapterType::Vga);
        let mut cpu = PcCpu::new(bus);

        // Move CPU to RAM
        cpu.cpu.cs = 0x0000;
        cpu.cpu.ip = 0x1000;

        // Write INT 11h instruction
        let addr = ((cpu.cpu.cs as u32) << 4) + (cpu.cpu.ip as u32);
        cpu.cpu.memory.write(addr, 0xCD); // INT
        cpu.cpu.memory.write(addr + 1, 0x11); // 11h

        // Execute INT 11h
        cpu.step();

        // Equipment flags should reflect VGA (bits 4-5 = 00)
        let equipment = cpu.cpu.ax;
        assert_eq!((equipment >> 4) & 0x03, 0b00); // VGA
    }

    #[test]
    fn test_int13h_read_zero_sectors() {
        // Test that reading 0 sectors succeeds without error (DOS 6.21 compatibility)
        let bus = PcBus::new();
        let mut cpu = PcCpu::new(bus);

        // Move CPU to RAM
        cpu.cpu.cs = 0x0000;
        cpu.cpu.ip = 0x1000;

        // Write INT 13h instruction
        let addr = ((cpu.cpu.cs as u32) << 4) + (cpu.cpu.ip as u32);
        cpu.cpu.memory.write(addr, 0xCD); // INT
        cpu.cpu.memory.write(addr + 1, 0x13); // 13h

        // Setup registers for AH=02h (read), AL=00h (0 sectors)
        cpu.cpu.ax = 0x0200; // AH=02h (read), AL=00 (0 sectors)
        cpu.cpu.cx = 0x0001; // CH=00, CL=01 (cylinder 0, sector 1)
        cpu.cpu.dx = 0x0000; // DH=00 (head 0), DL=00 (floppy A)
        cpu.cpu.es = 0x0000;
        cpu.cpu.bx = 0x7C00;

        // Execute INT 13h
        cpu.step();

        // Should succeed with AH=0, AL=0
        assert_eq!(cpu.cpu.ax, 0x0000u32);

        // Carry flag should be clear (success)
        assert!(!cpu.get_carry_flag());
    }

    #[test]
    fn test_int13h_write_zero_sectors() {
        // Test that writing 0 sectors succeeds without error
        let bus = PcBus::new();
        let mut cpu = PcCpu::new(bus);

        // Move CPU to RAM
        cpu.cpu.cs = 0x0000;
        cpu.cpu.ip = 0x1000;

        // Write INT 13h instruction
        let addr = ((cpu.cpu.cs as u32) << 4) + (cpu.cpu.ip as u32);
        cpu.cpu.memory.write(addr, 0xCD); // INT
        cpu.cpu.memory.write(addr + 1, 0x13); // 13h

        // Setup registers for AH=03h (write), AL=00h (0 sectors)
        cpu.cpu.ax = 0x0300; // AH=03h (write), AL=00 (0 sectors)
        cpu.cpu.cx = 0x0001; // CH=00, CL=01 (cylinder 0, sector 1)
        cpu.cpu.dx = 0x0000; // DH=00 (head 0), DL=00 (floppy A)
        cpu.cpu.es = 0x0000;
        cpu.cpu.bx = 0x7C00;

        // Execute INT 13h
        cpu.step();

        // Should succeed with AH=0, AL=0
        assert_eq!(cpu.cpu.ax, 0x0000u32);

        // Carry flag should be clear (success)
        assert!(!cpu.get_carry_flag());
    }

    #[test]
    fn test_int15h_get_system_configuration() {
        // Test INT 15h AH=C0h (Get System Configuration)
        let mut bus = PcBus::new();

        // Load BIOS so the configuration table exists at F000:E000
        let bios = crate::bios::generate_minimal_bios(CpuModel::Intel8086);
        bus.load_bios(&bios);

        let mut cpu = PcCpu::new(bus);

        // Move CPU to RAM
        cpu.cpu.cs = 0x0000;
        cpu.cpu.ip = 0x1000;

        // Write INT 15h instruction
        let addr = ((cpu.cpu.cs as u32) << 4) + (cpu.cpu.ip as u32);
        cpu.cpu.memory.write(addr, 0xCD); // INT
        cpu.cpu.memory.write(addr + 1, 0x15); // 15h

        // Setup registers for AH=C0h
        cpu.cpu.ax = 0xC000; // AH=C0h

        // Execute INT 15h
        cpu.step();

        // Should succeed (CF clear, AH=0)
        assert!(!cpu.get_carry_flag());
        assert_eq!((cpu.cpu.ax >> 8) & 0xFF, 0x00);

        // ES:BX should point to configuration table (in BIOS ROM at F000:E000)
        assert_eq!(cpu.cpu.es, 0xF000);
        assert_eq!(cpu.cpu.bx, 0xE000u32);

        // Verify configuration table in memory
        let table_addr = ((cpu.cpu.es as u32) << 4) + (cpu.cpu.bx as u32);

        // First word should be 8 (number of bytes following)
        let size = cpu.cpu.memory.read(table_addr) as u16
            | ((cpu.cpu.memory.read(table_addr + 1) as u16) << 8);
        assert_eq!(size, 8);

        // Model byte should be 0xFE (PC/XT) - the default when using Intel8086
        let model = cpu.cpu.memory.read(table_addr + 2);
        assert_eq!(model, 0xFE);
    }

    #[test]
    fn test_int15h_wait_on_external_event() {
        // Test INT 15h AH=41h (Wait on External Event) - should return not supported
        let bus = PcBus::new();
        let mut cpu = PcCpu::new(bus);

        // Move CPU to RAM
        cpu.cpu.cs = 0x0000;
        cpu.cpu.ip = 0x1000;

        // Write INT 15h instruction
        let addr = ((cpu.cpu.cs as u32) << 4) + (cpu.cpu.ip as u32);
        cpu.cpu.memory.write(addr, 0xCD); // INT
        cpu.cpu.memory.write(addr + 1, 0x15); // 15h

        // Setup registers for AH=41h
        cpu.cpu.ax = 0x4100; // AH=41h, AL=00

        // Execute INT 15h
        cpu.step();

        // Should fail (CF set, AH=86h = function not supported)
        assert!(cpu.get_carry_flag());
        assert_eq!((cpu.cpu.ax >> 8) & 0xFF, 0x86);
    }

    #[test]
    fn test_int15h_eisa_functions() {
        // Test INT 15h AH=D8h (EISA Functions) - should return not supported
        let bus = PcBus::new();
        let mut cpu = PcCpu::new(bus);

        // Move CPU to RAM
        cpu.cpu.cs = 0x0000;
        cpu.cpu.ip = 0x1000;

        // Write INT 15h instruction
        let addr = ((cpu.cpu.cs as u32) << 4) + (cpu.cpu.ip as u32);
        cpu.cpu.memory.write(addr, 0xCD); // INT
        cpu.cpu.memory.write(addr + 1, 0x15); // 15h

        // Setup registers for AH=D8h (EISA functions)
        cpu.cpu.ax = 0xD800; // AH=D8h, AL=00

        // Execute INT 15h
        cpu.step();

        // Should fail gracefully (CF set, AH=86h = function not supported)
        assert!(cpu.get_carry_flag());
        assert_eq!((cpu.cpu.ax >> 8) & 0xFF, 0x86);
    }

    #[test]
    fn test_int15h_keyboard_intercept() {
        // Test INT 15h AH=4Fh (Keyboard Intercept) - should return success
        let bus = PcBus::new();
        let mut cpu = PcCpu::new(bus);

        // Move CPU to RAM
        cpu.cpu.cs = 0x0000;
        cpu.cpu.ip = 0x1000;

        // Write INT 15h instruction
        let addr = ((cpu.cpu.cs as u32) << 4) + (cpu.cpu.ip as u32);
        cpu.cpu.memory.write(addr, 0xCD); // INT
        cpu.cpu.memory.write(addr + 1, 0x15); // 15h

        // Setup registers for AH=4Fh (keyboard intercept)
        cpu.cpu.ax = 0x4F1C; // AH=4Fh, AL=1Ch (scan code)

        // Execute INT 15h
        cpu.step();

        // Should succeed (CF clear, AL unchanged)
        assert!(!cpu.get_carry_flag());
        assert_eq!(cpu.cpu.ax & 0xFF, 0x1C); // AL should be unchanged
    }

    #[test]
    fn test_int15h_wait() {
        // Test INT 15h AH=86h (Wait) - should return success
        let bus = PcBus::new();
        let mut cpu = PcCpu::new(bus);

        // Move CPU to RAM
        cpu.cpu.cs = 0x0000;
        cpu.cpu.ip = 0x1000;

        // Write INT 15h instruction
        let addr = ((cpu.cpu.cs as u32) << 4) + (cpu.cpu.ip as u32);
        cpu.cpu.memory.write(addr, 0xCD); // INT
        cpu.cpu.memory.write(addr + 1, 0x15); // 15h

        // Setup registers for AH=86h (wait)
        cpu.cpu.ax = 0x8600; // AH=86h
        cpu.cpu.cx = 0x0001; // CX:DX = microseconds to wait
        cpu.cpu.dx = 0x0000;

        // Execute INT 15h
        cpu.step();

        // Should succeed (CF clear)
        assert!(!cpu.get_carry_flag());
    }

    #[test]
    fn test_int2fh_network_installation_check() {
        // Test INT 2Fh AH=11h (Network Installation Check)
        let bus = PcBus::new();
        let mut cpu = PcCpu::new(bus);

        // Move CPU to RAM
        cpu.cpu.cs = 0x0000;
        cpu.cpu.ip = 0x1000;

        // Write INT 2Fh instruction
        let addr = ((cpu.cpu.cs as u32) << 4) + (cpu.cpu.ip as u32);
        cpu.cpu.memory.write(addr, 0xCD); // INT
        cpu.cpu.memory.write(addr + 1, 0x2F); // 2Fh

        // Setup registers for AH=11h
        cpu.cpu.ax = 0x1100; // AH=11h, AL=00

        // Execute INT 2Fh
        cpu.step();

        // AL should be 0xFF (not installed)
        assert_eq!(cpu.cpu.ax & 0xFF, 0xFF);
    }

    #[test]
    fn test_interrupt_override() {
        // Test that OS-installed interrupt handlers are respected
        // This tests INT 2Ah, but the same logic applies to INT 21h, 28h, 29h, 2Fh, 31h, 33h
        let bus = PcBus::new();
        let mut cpu = PcCpu::new(bus);

        // Create a custom handler in RAM at 0x2000:0x0100
        // Handler will increment AX and return
        let handler_segment = 0x2000u16;
        let handler_offset = 0x0100u16;
        let handler_addr = ((handler_segment as u32) << 4) + (handler_offset as u32);

        // Write custom handler code:
        // INC AX (40)
        cpu.cpu.memory.write(handler_addr, 0x40); // INC AX
        // IRET (CF)
        cpu.cpu.memory.write(handler_addr + 1, 0xCF); // IRET

        // Install custom handler for INT 2Ah (vector at 0x00A8)
        // NOTE: We manually install this vector since we're not running BIOS init
        let vector_addr = 0x2A * 4;
        cpu.cpu.memory.write(vector_addr, (handler_offset & 0xFF) as u8);
        cpu.cpu
            .memory
            .write(vector_addr + 1, ((handler_offset >> 8) & 0xFF) as u8);
        cpu.cpu.memory.write(vector_addr + 2, (handler_segment & 0xFF) as u8);
        cpu.cpu
            .memory
            .write(vector_addr + 3, ((handler_segment >> 8) & 0xFF) as u8);

        // Verify the vector was set correctly
        let read_offset = cpu.cpu.memory.read(vector_addr) as u16
            | ((cpu.cpu.memory.read(vector_addr + 1) as u16) << 8);
        let read_segment = cpu.cpu.memory.read(vector_addr + 2) as u16
            | ((cpu.cpu.memory.read(vector_addr + 3) as u16) << 8);
        assert_eq!(
            read_offset, handler_offset,
            "Vector offset should be correctly set"
        );
        assert_eq!(
            read_segment, handler_segment,
            "Vector segment should be correctly set"
        );

        // Verify is_interrupt_overridden returns true
        assert!(
            cpu.is_interrupt_overridden(0x2A),
            "INT 2Ah should be detected as overridden"
        );

        // Set up code to call INT 2Ah
        cpu.cpu.cs = 0x0000;
        cpu.cpu.ip = 0x1000;
        cpu.cpu.ss = 0x0000;
        cpu.cpu.sp = 0xFFFE;
        let call_addr = ((cpu.cpu.cs as u32) << 4) + (cpu.cpu.ip as u32);

        // Write INT 2Ah instruction
        cpu.cpu.memory.write(call_addr, 0xCD); // INT
        cpu.cpu.memory.write(call_addr + 1, 0x2A); // 2Ah

        // Set AX to 0x1234 before calling
        cpu.cpu.ax = 0x1234;

        // Execute INT 2Ah - should call our custom handler, not the BIOS stub
        cpu.step(); // Execute INT (jumps to handler)
        cpu.step(); // Execute INC AX in handler
        cpu.step(); // Execute IRET (return from handler)

        // Our custom handler should have incremented AX to 0x1235
        assert_eq!(
            cpu.cpu.ax, 0x1235,
            "Custom INT 2Ah handler should have been called and incremented AX"
        );
        
        // Should have returned to the instruction after INT
        assert_eq!(cpu.cpu.cs, 0x0000, "Should return to original CS");
        assert_eq!(cpu.cpu.ip, 0x1002, "Should return to instruction after INT");
    }

    #[test]
    fn test_interrupt_override_bios_default() {
        // Test that BIOS handlers are used when no OS handler is installed
        let bus = PcBus::new();
        let mut cpu = PcCpu::new(bus);

        // Move CPU to RAM
        cpu.cpu.cs = 0x0000;
        cpu.cpu.ip = 0x1000;

        // Write INT 2Ah instruction
        let addr = ((cpu.cpu.cs as u32) << 4) + (cpu.cpu.ip as u32);
        cpu.cpu.memory.write(addr, 0xCD); // INT
        cpu.cpu.memory.write(addr + 1, 0x2A); // 2Ah

        // Setup registers for a network API call
        cpu.cpu.ax = 0x0100; // AH=01h (some function)

        // Execute INT 2Ah - should use BIOS stub (since vector points to F000:0040)
        cpu.step();

        // BIOS stub returns AL=0 (not installed) and sets CF
        assert_eq!(cpu.cpu.ax & 0xFF, 0x00, "AL should be 0 (not installed)");
        assert!(cpu.get_carry_flag(), "CF should be set (error/not installed)");
    }

    #[test]
    fn test_core_cpu_interrupt_vector() {
        // Test that the core CPU correctly executes custom interrupt handlers
        // This bypasses the PC wrapper to isolate the issue
        use emu_core::cpu_8086::Cpu8086;
        
        let bus = PcBus::new();
        let mut cpu = Cpu8086::new(bus);
        
        // Set up stack
        cpu.ss = 0x0000;
        cpu.sp = 0xFFFE;
        
        // Create custom handler at 0x2000:0x0100
        let handler_addr = 0x20100u32;  // Physical address
        cpu.memory.write(handler_addr, 0x40); // INC AX
        cpu.memory.write(handler_addr + 1, 0xCF); // IRET
        
        // Install vector for INT 2Ah
        let vector_addr = 0x2A * 4;
        cpu.memory.write(vector_addr, 0x00); // Offset low: 0x0100
        cpu.memory.write(vector_addr + 1, 0x01); // Offset high
        cpu.memory.write(vector_addr + 2, 0x00); // Segment low: 0x2000
        cpu.memory.write(vector_addr + 3, 0x20); // Segment high
        
        // Write INT 2Ah at 0x0000:0x1000
        cpu.cs = 0x0000;
        cpu.ip = 0x1000;
        cpu.memory.write(0x1000, 0xCD); // INT
        cpu.memory.write(0x1001, 0x2A); // 2Ah
        
        cpu.ax = 0x1234;
        
        // Execute INT (this will jump to the handler)
        cpu.step();
        
        // Now we're at the handler - execute INC AX
        cpu.step();
        
        // Execute IRET to return
        cpu.step();
        
        // Now we should be back and AX should be incremented
        assert_eq!(cpu.ax, 0x1235, "Core CPU should execute custom handler and increment AX");
        assert_eq!(cpu.cs, 0x0000, "Should return to original CS");
        assert_eq!(cpu.ip, 0x1002, "Should return to instruction after INT");
    }

    #[test]
    fn test_int13h_read_sectors_advances_esbx() {
        // Test that INT 13h AH=02h advances ES:BX pointer after reading
        let mut bus = PcBus::new();

        // Create a floppy image with test data
        let mut floppy = vec![0; 1474560]; // 1.44MB

        // Fill first two sectors with test patterns
        for (i, byte) in floppy.iter_mut().enumerate().take(512) {
            *byte = (i % 256) as u8; // First sector
        }
        for (i, byte) in floppy.iter_mut().enumerate().skip(512).take(512) {
            *byte = (i % 256) as u8; // Second sector
        }

        bus.mount_floppy_a(floppy);

        let mut cpu = PcCpu::new(bus);

        // Move CPU to RAM
        cpu.cpu.cs = 0x0000;
        cpu.cpu.ip = 0x1000;

        // Setup: Write INT 13h instruction
        let cs = cpu.cpu.cs;
        let ip = cpu.cpu.ip;
        let addr = ((cs as u32) << 4) + (ip as u32);

        cpu.cpu.memory.write(addr, 0xCD); // INT
        cpu.cpu.memory.write(addr + 1, 0x13); // 13h

        // Setup registers for AH=02h (read 1 sector)
        cpu.cpu.ax = 0x0201; // AH=02h (read), AL=01 (1 sector)
        cpu.cpu.cx = 0x0001; // CH=00, CL=01 (cylinder 0, sector 1)
        cpu.cpu.dx = 0x0000; // DH=00 (head 0), DL=00 (floppy A)
        cpu.cpu.es = 0x0000;
        cpu.cpu.bx = 0x7C00; // Buffer at 0x0000:0x7C00

        // Execute INT 13h
        cpu.step();

        // Should succeed
        assert_eq!((cpu.cpu.ax >> 8) & 0xFF, 0x00); // Status = success
        assert_eq!(cpu.cpu.ax & 0xFF, 0x01); // AL = sectors read

        // ES:BX should NOT be modified (INT 13h AH=02h leaves pointer unchanged)
        assert_eq!(cpu.cpu.es, 0x0000, "ES should remain unchanged");
        assert_eq!(cpu.cpu.bx, 0x7C00, "BX should remain unchanged");

        // Verify data was copied to buffer
        let buffer_addr = 0x7C00;
        assert_eq!(cpu.cpu.memory.read(buffer_addr), 0);
        assert_eq!(cpu.cpu.memory.read(buffer_addr + 255), 255);
    }

    #[test]
    fn test_int13h_read_multiple_sectors_does_not_modify_esbx() {
        // Test that INT 13h AH=02h advances ES:BX correctly for multiple sectors
        let mut bus = PcBus::new();

        // Create a floppy image with test data
        let floppy = vec![0xAA; 1474560]; // 1.44MB, all 0xAA

        bus.mount_floppy_a(floppy);

        let mut cpu = PcCpu::new(bus);

        // Move CPU to RAM
        cpu.cpu.cs = 0x0000;
        cpu.cpu.ip = 0x1000;

        // Setup: Write INT 13h instruction
        let cs = cpu.cpu.cs;
        let ip = cpu.cpu.ip;
        let addr = ((cs as u32) << 4) + (ip as u32);

        cpu.cpu.memory.write(addr, 0xCD); // INT
        cpu.cpu.memory.write(addr + 1, 0x13); // 13h

        // Setup registers for AH=02h (read 5 sectors)
        cpu.cpu.ax = 0x0205; // AH=02h (read), AL=05 (5 sectors)
        cpu.cpu.cx = 0x0001; // CH=00, CL=01 (cylinder 0, sector 1)
        cpu.cpu.dx = 0x0000; // DH=00 (head 0), DL=00 (floppy A)
        cpu.cpu.es = 0x0000;
        cpu.cpu.bx = 0x8000; // Buffer at 0x0000:0x8000

        // Execute INT 13h
        cpu.step();

        // Should succeed
        assert_eq!((cpu.cpu.ax >> 8) & 0xFF, 0x00); // Status = success
        assert_eq!(cpu.cpu.ax & 0xFF, 0x05); // AL = 5 sectors read

        // ES:BX should NOT be modified (INT 13h AH=02h leaves pointer unchanged)
        assert_eq!(cpu.cpu.es, 0x0000, "ES should remain unchanged");
        assert_eq!(cpu.cpu.bx, 0x8000, "BX should remain unchanged");
    }

    #[test]
    fn test_int13h_read_large_buffer_does_not_modify_esbx() {
        // Test that INT 13h AH=02h handles ES:BX advancement past segment boundary
        let mut bus = PcBus::new();

        // Create a floppy image
        let floppy = vec![0xBB; 1474560]; // 1.44MB, all 0xBB

        bus.mount_floppy_a(floppy);

        let mut cpu = PcCpu::new(bus);

        // Move CPU to RAM
        cpu.cpu.cs = 0x0000;
        cpu.cpu.ip = 0x1000;

        // Setup: Write INT 13h instruction
        let cs = cpu.cpu.cs;
        let ip = cpu.cpu.ip;
        let addr = ((cs as u32) << 4) + (ip as u32);

        cpu.cpu.memory.write(addr, 0xCD); // INT
        cpu.cpu.memory.write(addr + 1, 0x13); // 13h

        // Setup registers for AH=02h (read 4 sectors)
        // Start at offset where reading won't cross 64KB within the read itself,
        // but the final BX will be > 0xFFFF (demonstrating BX wrapping)
        cpu.cpu.ax = 0x0204; // AH=02h (read), AL=04 (4 sectors = 2048 bytes)
        cpu.cpu.cx = 0x0001; // CH=00, CL=01 (cylinder 0, sector 1)
        cpu.cpu.dx = 0x0000; // DH=00 (head 0), DL=00 (floppy A)
        cpu.cpu.es = 0x1000;
        cpu.cpu.bx = 0xF800; // Buffer at 0x1000:0xF800
                             // 0xF800 + 2048 = 0xF800 + 0x800 = 0x10000 (exactly at boundary)

        // Execute INT 13h
        cpu.step();

        // Should succeed
        assert_eq!((cpu.cpu.ax >> 8) & 0xFF, 0x00); // Status = success
        assert_eq!(cpu.cpu.ax & 0xFF, 0x04); // AL = 4 sectors read

        // ES:BX should NOT be modified (INT 13h AH=02h leaves pointer unchanged)
        assert_eq!(cpu.cpu.es, 0x1000, "ES should remain unchanged");
        assert_eq!(cpu.cpu.bx, 0xF800, "BX should remain unchanged");
    }

    #[test]
    fn test_int13h_write_sectors_does_not_modify_esbx() {
        // Test that INT 13h AH=03h advances ES:BX pointer after writing
        let mut bus = PcBus::new();

        // Create a blank floppy image
        let floppy = vec![0; 1474560]; // 1.44MB
        bus.mount_floppy_a(floppy);

        let mut cpu = PcCpu::new(bus);

        // Move CPU to RAM
        cpu.cpu.cs = 0x0000;
        cpu.cpu.ip = 0x1000;

        // Setup: Write test data to memory at 0x0000:0x7C00
        let buffer_addr = 0x7C00;
        for i in 0..512 {
            cpu.cpu.memory.write(buffer_addr + i, (i % 256) as u8);
        }

        // Setup: Write INT 13h instruction
        let cs = cpu.cpu.cs;
        let ip = cpu.cpu.ip;
        let addr = ((cs as u32) << 4) + (ip as u32);

        cpu.cpu.memory.write(addr, 0xCD); // INT
        cpu.cpu.memory.write(addr + 1, 0x13); // 13h

        // Setup registers for AH=03h (write 1 sector)
        cpu.cpu.ax = 0x0301; // AH=03h (write), AL=01 (1 sector)
        cpu.cpu.cx = 0x0001; // CH=00, CL=01 (cylinder 0, sector 1)
        cpu.cpu.dx = 0x0000; // DH=00 (head 0), DL=00 (floppy A)
        cpu.cpu.es = 0x0000;
        cpu.cpu.bx = 0x7C00; // Buffer at 0x0000:0x7C00

        // Execute INT 13h
        cpu.step();

        // Should succeed
        assert_eq!((cpu.cpu.ax >> 8) & 0xFF, 0x00); // Status = success
        assert_eq!(cpu.cpu.ax & 0xFF, 0x01); // AL = sectors written

        // ES:BX should NOT be modified (INT 13h AH=03h leaves pointer unchanged)
        assert_eq!(cpu.cpu.es, 0x0000, "ES should remain unchanged");
        assert_eq!(cpu.cpu.bx, 0x7C00, "BX should remain unchanged");
    }

    #[test]
    fn test_int13h_read_file_from_disk_image() {
        // Test reading a simulated file from a disk image
        // This simulates what happens when DOS reads a file using INT 13h
        let mut bus = PcBus::new();

        // Create a floppy image with simulated file content
        let mut floppy = vec![0; 1474560]; // 1.44MB

        // Simulate a file in sectors 10-12 (CHS sector numbers, 1-based)
        // These correspond to LBA 9-11 (0-based)
        // Fill these sectors with recognizable test data
        let file_start_lba = 9; // LBA for CHS sector 10
        let file_sectors = 3;

        for lba in 0..file_sectors {
            let offset = (file_start_lba + lba) * 512;
            for i in 0..512 {
                // Simple pattern: just use byte offset within sector
                floppy[offset + i] = (i % 256) as u8;
            }
        }

        bus.mount_floppy_a(floppy);

        let mut cpu = PcCpu::new(bus);

        // Move CPU to RAM
        cpu.cpu.cs = 0x0000;
        cpu.cpu.ip = 0x1000;

        // Test 1: Read first sector of the "file" (CHS sector 10 = LBA 9)
        let addr = ((cpu.cpu.cs as u32) << 4) + (cpu.cpu.ip as u32);
        cpu.cpu.memory.write(addr, 0xCD); // INT
        cpu.cpu.memory.write(addr + 1, 0x13); // 13h

        cpu.cpu.ax = 0x0201; // AH=02h (read), AL=01 (1 sector)
        cpu.cpu.cx = 0x000A; // CH=00, CL=10 (cylinder 0, sector 10)
        cpu.cpu.dx = 0x0000; // DH=00 (head 0), DL=00 (floppy A)
        cpu.cpu.es = 0x0000;
        cpu.cpu.bx = 0x7C00; // Buffer

        cpu.step();

        // Should succeed
        assert!(!cpu.get_carry_flag(), "Read should succeed");
        assert_eq!((cpu.cpu.ax >> 8) & 0xFF, 0x00, "Status should be success");

        // Verify data pattern from first sector
        let buffer = 0x7C00;
        assert_eq!(cpu.cpu.memory.read(buffer), 0); // offset 0
        assert_eq!(cpu.cpu.memory.read(buffer + 15), 15); // offset 15
        assert_eq!(cpu.cpu.memory.read(buffer + 255), 255); // offset 255

        // Test 2: Read all file sectors at once (CHS sectors 10-12 = LBA 9-11)
        cpu.cpu.ip = 0x1000; // Reset IP
        cpu.cpu.ax = 0x0203; // AH=02h (read), AL=03 (3 sectors)
        cpu.cpu.cx = 0x000A; // CH=00, CL=10 (cylinder 0, sector 10)
        cpu.cpu.dx = 0x0000; // DH=00 (head 0), DL=00 (floppy A)
        cpu.cpu.es = 0x0000;
        cpu.cpu.bx = 0x8000; // Different buffer

        cpu.step();

        // Should succeed
        assert!(!cpu.get_carry_flag(), "Multi-sector read should succeed");
        assert_eq!((cpu.cpu.ax >> 8) & 0xFF, 0x00, "Status should be success");

        // Verify data from all three sectors
        let buffer2 = 0x8000;
        // First sector
        assert_eq!(cpu.cpu.memory.read(buffer2), 0);
        assert_eq!(cpu.cpu.memory.read(buffer2 + 255), 255);
        // Second sector (same pattern repeats)
        assert_eq!(cpu.cpu.memory.read(buffer2 + 512), 0);
        assert_eq!(cpu.cpu.memory.read(buffer2 + 512 + 255), 255);
        // Third sector
        assert_eq!(cpu.cpu.memory.read(buffer2 + 1024), 0);
        assert_eq!(cpu.cpu.memory.read(buffer2 + 1024 + 255), 255);

        // Test 3: Try to read beyond the "file" (CHS sector 13 = LBA 12, empty)
        cpu.cpu.ip = 0x1000; // Reset IP
        cpu.cpu.ax = 0x0201; // AH=02h (read), AL=01 (1 sector)
        cpu.cpu.cx = 0x000D; // CH=00, CL=13 (cylinder 0, sector 13) - past the file
        cpu.cpu.dx = 0x0000; // DH=00 (head 0), DL=00 (floppy A)
        cpu.cpu.es = 0x0000;
        cpu.cpu.bx = 0x9000; // Another buffer

        cpu.step();

        // Should succeed (sector exists on disk)
        assert!(
            !cpu.get_carry_flag(),
            "Read beyond file should succeed if sector exists"
        );
        assert_eq!((cpu.cpu.ax >> 8) & 0xFF, 0x00, "Status should be success");

        // Verify data is zero (empty sector)
        let buffer3 = 0x9000;
        assert_eq!(cpu.cpu.memory.read(buffer3), 0x00);
        assert_eq!(cpu.cpu.memory.read(buffer3 + 511), 0x00);
    }

    #[test]
    fn test_int13h_write_multiple_sectors_does_not_modify_esbx() {
        // Test that INT 13h AH=03h advances ES:BX correctly for multiple sectors
        let mut bus = PcBus::new();

        // Create a blank floppy image
        let floppy = vec![0; 1474560]; // 1.44MB
        bus.mount_floppy_a(floppy);

        let mut cpu = PcCpu::new(bus);

        // Move CPU to RAM
        cpu.cpu.cs = 0x0000;
        cpu.cpu.ip = 0x1000;

        // Setup: Write test data to memory
        let buffer_addr = 0x8000;
        for i in 0..(3 * 512) {
            cpu.cpu.memory.write(buffer_addr + i, 0xCC);
        }

        // Setup: Write INT 13h instruction
        let cs = cpu.cpu.cs;
        let ip = cpu.cpu.ip;
        let addr = ((cs as u32) << 4) + (ip as u32);

        cpu.cpu.memory.write(addr, 0xCD); // INT
        cpu.cpu.memory.write(addr + 1, 0x13); // 13h

        // Setup registers for AH=03h (write 3 sectors)
        cpu.cpu.ax = 0x0303; // AH=03h (write), AL=03 (3 sectors)
        cpu.cpu.cx = 0x0001; // CH=00, CL=01 (cylinder 0, sector 1)
        cpu.cpu.dx = 0x0000; // DH=00 (head 0), DL=00 (floppy A)
        cpu.cpu.es = 0x0000;
        cpu.cpu.bx = 0x8000; // Buffer at 0x0000:0x8000

        // Execute INT 13h
        cpu.step();

        // Should succeed
        assert_eq!((cpu.cpu.ax >> 8) & 0xFF, 0x00); // Status = success
        assert_eq!(cpu.cpu.ax & 0xFF, 0x03); // AL = 3 sectors written

        // ES:BX should NOT be modified (INT 13h AH=03h leaves pointer unchanged)
        assert_eq!(cpu.cpu.es, 0x0000, "ES should remain unchanged");
        assert_eq!(cpu.cpu.bx, 0x8000, "BX should remain unchanged");
    }

    #[test]
    fn test_keyboard_shift_modifier() {
        use crate::keyboard::*;

        let bus = PcBus::new();
        let mut cpu = PcCpu::new(bus);

        // Test lowercase 'a' without shift
        cpu.cpu.memory.keyboard.key_press(SCANCODE_A);
        let ascii = cpu.scancode_to_ascii(SCANCODE_A);
        assert_eq!(ascii, b'a', "Without shift, 'a' should be lowercase");

        // Test uppercase 'A' with shift
        cpu.cpu.memory.keyboard.key_press(SCANCODE_LEFT_SHIFT);
        cpu.cpu.memory.keyboard.key_press(SCANCODE_A);
        let ascii = cpu.scancode_to_ascii(SCANCODE_A);
        assert_eq!(ascii, b'A', "With shift, 'a' should be uppercase 'A'");
        cpu.cpu.memory.keyboard.key_release(SCANCODE_A);
        cpu.cpu.memory.keyboard.key_release(SCANCODE_LEFT_SHIFT);

        // Test Shift+1 = !
        cpu.cpu.memory.keyboard.key_press(SCANCODE_LEFT_SHIFT);
        cpu.cpu.memory.keyboard.key_press(SCANCODE_1);
        let ascii = cpu.scancode_to_ascii(SCANCODE_1);
        assert_eq!(ascii, b'!', "Shift+1 should produce '!'");
        cpu.cpu.memory.keyboard.key_release(SCANCODE_1);
        cpu.cpu.memory.keyboard.key_release(SCANCODE_LEFT_SHIFT);

        // Test plain '1' without shift
        cpu.cpu.memory.keyboard.key_press(SCANCODE_1);
        let ascii = cpu.scancode_to_ascii(SCANCODE_1);
        assert_eq!(ascii, b'1', "Without shift, '1' should be '1'");
    }

    #[test]
    fn test_keyboard_shift_symbols() {
        use crate::keyboard::*;

        let bus = PcBus::new();
        let mut cpu = PcCpu::new(bus);

        // Test all shifted number keys
        cpu.cpu.memory.keyboard.key_press(SCANCODE_LEFT_SHIFT);

        assert_eq!(cpu.scancode_to_ascii(SCANCODE_1), b'!');
        assert_eq!(cpu.scancode_to_ascii(SCANCODE_2), b'@');
        assert_eq!(cpu.scancode_to_ascii(SCANCODE_3), b'#');
        assert_eq!(cpu.scancode_to_ascii(SCANCODE_4), b'$');
        assert_eq!(cpu.scancode_to_ascii(SCANCODE_5), b'%');
        assert_eq!(cpu.scancode_to_ascii(SCANCODE_6), b'^');
        assert_eq!(cpu.scancode_to_ascii(SCANCODE_7), b'&');
        assert_eq!(cpu.scancode_to_ascii(SCANCODE_8), b'*');
        assert_eq!(cpu.scancode_to_ascii(SCANCODE_9), b'(');
        assert_eq!(cpu.scancode_to_ascii(SCANCODE_0), b')');

        // Test shifted punctuation
        assert_eq!(cpu.scancode_to_ascii(SCANCODE_MINUS), b'_');
        assert_eq!(cpu.scancode_to_ascii(SCANCODE_EQUALS), b'+');
        assert_eq!(cpu.scancode_to_ascii(SCANCODE_SEMICOLON), b':');
        assert_eq!(cpu.scancode_to_ascii(SCANCODE_APOSTROPHE), b'"');
        assert_eq!(cpu.scancode_to_ascii(SCANCODE_COMMA), b'<');
        assert_eq!(cpu.scancode_to_ascii(SCANCODE_PERIOD), b'>');
        assert_eq!(cpu.scancode_to_ascii(SCANCODE_SLASH), b'?');
        assert_eq!(cpu.scancode_to_ascii(SCANCODE_BACKTICK), b'~');

        cpu.cpu.memory.keyboard.key_release(SCANCODE_LEFT_SHIFT);
    }

    #[test]
    fn test_keyboard_altgr_modifier() {
        use crate::keyboard::*;

        let bus = PcBus::new();
        let mut cpu = PcCpu::new(bus);

        // Test AltGr+2 = @
        cpu.cpu.memory.keyboard.key_press(SCANCODE_RIGHT_ALT);
        cpu.cpu.memory.keyboard.key_press(SCANCODE_2);
        let ascii = cpu.scancode_to_ascii(SCANCODE_2);
        assert_eq!(ascii, b'@', "AltGr+2 should produce '@'");
        cpu.cpu.memory.keyboard.key_release(SCANCODE_2);
        cpu.cpu.memory.keyboard.key_release(SCANCODE_RIGHT_ALT);

        // Test AltGr+7 = {
        cpu.cpu.memory.keyboard.key_press(SCANCODE_RIGHT_ALT);
        assert_eq!(cpu.scancode_to_ascii(SCANCODE_7), b'{');
        assert_eq!(cpu.scancode_to_ascii(SCANCODE_8), b'[');
        assert_eq!(cpu.scancode_to_ascii(SCANCODE_9), b']');
        assert_eq!(cpu.scancode_to_ascii(SCANCODE_0), b'}');
        cpu.cpu.memory.keyboard.key_release(SCANCODE_RIGHT_ALT);
    }
}
