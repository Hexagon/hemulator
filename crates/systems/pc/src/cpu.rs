//! PC CPU wrapper
//!
//! This module wraps the core 8086 CPU with PC-specific initialization and state.

use crate::bus::PcBus;
use emu_core::cpu_8086::{Cpu8086, CpuModel, Memory8086};

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

    /// Reset the CPU to initial state
    pub fn reset(&mut self) {
        self.cpu.reset();

        // Restore PC boot state
        self.cpu.cs = 0xFFFF;
        self.cpu.ip = 0x0000;
        self.cpu.ss = 0x0000;
        self.cpu.sp = 0xFFFE;
        self.cpu.ds = 0x0000;
        self.cpu.es = 0x0000;
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

        // Handle I/O instructions by intercepting them before execution
        match opcode {
            // IN AL, imm8 (0xE4)
            0xE4 => {
                let port = self.cpu.memory.read(physical_addr + 1) as u16;
                let val = self.cpu.memory.io_read(port);
                self.cpu.ax = (self.cpu.ax & 0xFF00) | (val as u16);
                self.cpu.ip = self.cpu.ip.wrapping_add(2);
                return 10;
            }
            // IN AX, imm8 (0xE5)
            0xE5 => {
                let port = self.cpu.memory.read(physical_addr + 1) as u16;
                let val = self.cpu.memory.io_read(port);
                let val_high = self.cpu.memory.io_read(port.wrapping_add(1));
                self.cpu.ax = (val as u16) | ((val_high as u16) << 8);
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
                let port = self.cpu.dx;
                let val = self.cpu.memory.io_read(port);
                self.cpu.ax = (self.cpu.ax & 0xFF00) | (val as u16);
                self.cpu.ip = self.cpu.ip.wrapping_add(1);
                return 8;
            }
            // IN AX, DX (0xED)
            0xED => {
                let port = self.cpu.dx;
                let val = self.cpu.memory.io_read(port);
                let val_high = self.cpu.memory.io_read(port.wrapping_add(1));
                self.cpu.ax = (val as u16) | ((val_high as u16) << 8);
                self.cpu.ip = self.cpu.ip.wrapping_add(1);
                return 8;
            }
            // OUT DX, AL (0xEE)
            0xEE => {
                let port = self.cpu.dx;
                let val = (self.cpu.ax & 0xFF) as u8;
                self.cpu.memory.io_write(port, val);
                self.cpu.ip = self.cpu.ip.wrapping_add(1);
                return 8;
            }
            // OUT DX, AX (0xEF)
            0xEF => {
                let port = self.cpu.dx;
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
        if opcode == 0xCD {
            // This is an INT instruction, check the interrupt number
            let int_num = self.cpu.memory.read(physical_addr + 1);
            match int_num {
                0x10 => return self.handle_int10h(), // Video BIOS
                0x13 => return self.handle_int13h(), // Disk services
                0x15 => return self.handle_int15h(), // Extended services
                0x16 => return self.handle_int16h(), // Keyboard services
                0x1A => return self.handle_int1ah(), // Time/Date services
                0x20 => return self.handle_int20h(), // DOS: Program terminate
                0x21 => return self.handle_int21h(), // DOS API
                0x2F => return self.handle_int2fh(), // Multiplex interrupt
                0x33 => return self.handle_int33h(), // Mouse services
                _ => {}                              // Let CPU handle other interrupts normally
            }
        }

        // Execute normally
        self.cpu.step()
    }

    /// Handle INT 10h - Video BIOS services
    #[allow(dead_code)] // Called dynamically based on interrupt number
    fn handle_int10h(&mut self) -> u32 {
        // Skip the INT 10h instruction (2 bytes: 0xCD 0x10)
        self.cpu.ip = self.cpu.ip.wrapping_add(2);

        // Get function code from AH register
        let ah = ((self.cpu.ax >> 8) & 0xFF) as u8;

        match ah {
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
            0x0C => self.int10h_write_pixel(),
            0x0D => self.int10h_read_pixel(),
            0x0E => self.int10h_teletype_output(),
            0x0F => self.int10h_get_video_mode(),
            0x10 => self.int10h_palette_functions(),
            0x11 => self.int10h_character_generator(),
            0x12 => self.int10h_video_subsystem_config(),
            0x13 => self.int10h_write_string(),
            0x1A => self.int10h_display_combination(),
            _ => {
                // Unsupported function - just return
                51 // Approximate INT instruction timing
            }
        }
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

        self.cpu.dx = ((row as u16) << 8) | (col as u16);
        self.cpu.cx = 0x0607; // Default cursor shape
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

        self.cpu.ax = ((attr as u16) << 8) | (ch as u16);
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

    /// INT 10h, AH=0Eh: Teletype output
    #[allow(dead_code)] // Called from handle_int10h
    fn int10h_teletype_output(&mut self) -> u32 {
        // AL = character, BH = page number, BL = foreground color (graphics mode)
        let ch = (self.cpu.ax & 0xFF) as u8;
        let page = ((self.cpu.bx >> 8) & 0xFF) as u8;

        // Get cursor position
        let cursor_addr = 0x450 + (page as u32 * 2);
        let mut col = self.cpu.memory.read(cursor_addr) as u32;
        let mut row = self.cpu.memory.read(cursor_addr + 1) as u32;

        // Handle special characters
        match ch {
            0x08 => {
                // Backspace
                col = col.saturating_sub(1);
            }
            0x0A => {
                // Line feed
                row += 1;
                if row >= 25 {
                    row = 24; // Stay at bottom
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
                        row = 24;
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
        self.cpu.ax = 0x5003; // AH=80 columns, AL=mode 3
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
        self.cpu.ax = (self.cpu.ax & 0xFF00) | (color as u16);
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
                self.cpu.bx = 0x0008; // VGA with color display
                51
            }
            _ => {
                // Set display combination - stub
                51
            }
        }
    }

    /// Handle INT 16h - Keyboard BIOS services
    #[allow(dead_code)] // Called dynamically based on interrupt number
    fn handle_int16h(&mut self) -> u32 {
        // Skip the INT 16h instruction (2 bytes: 0xCD 0x16)
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
        // For emulation, we just read from buffer (non-blocking behavior)

        if self.cpu.memory.keyboard.has_data() {
            let scancode = self.cpu.memory.keyboard.read_scancode();

            // Convert scancode to ASCII (simplified mapping)
            let ascii = scancode_to_ascii(scancode);

            // AH = scan code, AL = ASCII character
            self.cpu.ax = ((scancode as u16) << 8) | (ascii as u16);
        } else {
            // No key available - return 0
            self.cpu.ax = 0x0000;
        }
        51
    }

    /// INT 16h, AH=01h: Check for keystroke (non-blocking)
    fn int16h_check_keystroke(&mut self) -> u32 {
        // Returns: ZF = 1 if no key available, ZF = 0 if key available
        // If key available: AH = scan code, AL = ASCII character

        if self.cpu.memory.keyboard.has_data() {
            // Peek at the next scancode without consuming it
            let scancode = self.cpu.memory.keyboard.peek_scancode();
            let ascii = scancode_to_ascii(scancode);

            // Set ZF = 0 (key available)
            self.set_zero_flag(false);

            // AH = scan code, AL = ASCII character
            self.cpu.ax = ((scancode as u16) << 8) | (ascii as u16);
        } else {
            // No key available
            self.set_zero_flag(true); // ZF = 1 (no key)
            self.cpu.ax = 0x0000;
        }
        51
    }

    /// INT 16h, AH=02h: Get shift flags
    fn int16h_get_shift_flags(&mut self) -> u32 {
        // Returns: AL = shift flags
        // Bit 0 = right shift, Bit 1 = left shift, etc.
        // For now, return 0 (no keys pressed)
        self.cpu.ax &= 0xFF00;
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
            0x4C => self.int21h_terminate_with_code(),  // Terminate with return code
            _ => {
                // Unsupported function - just return
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
        // For now, return 0 (no input available)
        self.cpu.ax &= 0xFF00;
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
        self.cpu.ax = (self.cpu.ax & 0xFF00) | (ch as u16);
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
            // Read character - for now, return 0 and set ZF
            self.cpu.ax &= 0xFF00;
            self.set_zero_flag(true);
        } else {
            // Write character
            let saved_ax = self.cpu.ax;
            self.cpu.ax = (self.cpu.ax & 0xFF00) | (dl as u16);
            self.int10h_teletype_output();
            self.cpu.ax = saved_ax;
        }

        51
    }

    /// INT 21h, AH=07h: Direct stdin input without echo
    #[allow(dead_code)] // Called from handle_int21h
    fn int21h_direct_stdin(&mut self) -> u32 {
        // Returns: AL = character read
        self.cpu.ax &= 0xFF00;
        51
    }

    /// INT 21h, AH=08h: Read stdin without echo
    #[allow(dead_code)] // Called from handle_int21h
    fn int21h_stdin_no_echo(&mut self) -> u32 {
        // Returns: AL = character read
        self.cpu.ax &= 0xFF00;
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
            self.cpu.ax = (self.cpu.ax & 0xFF00) | (ch as u16);
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
        // For now, always return 0x00 (no input)
        self.cpu.ax &= 0xFF00;
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
        self.cpu.bx = 0x0000;
        51
    }

    /// INT 21h, AH=4Ch: Terminate with return code
    #[allow(dead_code)] // Called from handle_int21h
    fn int21h_terminate_with_code(&mut self) -> u32 {
        // AL = return code
        // For now, just halt like INT 20h
        51
    }

    /// Handle INT 13h BIOS disk services
    fn handle_int13h(&mut self) -> u32 {
        // Skip the INT 13h instruction (2 bytes: 0xCD 0x13)
        self.cpu.ip = self.cpu.ip.wrapping_add(2);

        // Get function code from AH register
        let ah = ((self.cpu.ax >> 8) & 0xFF) as u8;

        match ah {
            0x00 => self.int13h_reset(),
            0x02 => self.int13h_read_sectors(),
            0x03 => self.int13h_write_sectors(),
            0x08 => self.int13h_get_drive_params(),
            _ => {
                // Unsupported function - set error in AH
                self.cpu.ax = (self.cpu.ax & 0x00FF) | (0x01 << 8); // Invalid function
                self.set_carry_flag(true);
                51 // Approximate INT instruction timing
            }
        }
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

        // Copy buffer to memory at ES:BX
        if status == 0x00 {
            for (i, &byte) in buffer.iter().enumerate() {
                let offset = buffer_offset.wrapping_add(i as u16);
                self.cpu.write_byte(buffer_seg, offset, byte);
            }
        }

        // Set AH = status
        self.cpu.ax = (self.cpu.ax & 0x00FF) | ((status as u16) << 8);

        // Set carry flag based on status
        self.set_carry_flag(status != 0x00);

        // AL = number of sectors read (on success)
        if status == 0x00 {
            self.cpu.ax = (self.cpu.ax & 0xFF00) | (count as u16);
        }

        51 // Approximate INT instruction timing
    }

    /// INT 13h, AH=03h: Write sectors
    fn int13h_write_sectors(&mut self) -> u32 {
        use crate::disk::DiskRequest;

        // AL = number of sectors to write
        let count = (self.cpu.ax & 0xFF) as u8;

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
            let offset = buffer_offset.wrapping_add(i as u16);
            *byte = self.cpu.read_byte(buffer_seg, offset);
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
        self.cpu.ax = (self.cpu.ax & 0x00FF) | ((status as u16) << 8);

        // Set carry flag based on status
        self.set_carry_flag(status != 0x00);

        // AL = number of sectors written (on success)
        if status == 0x00 {
            self.cpu.ax = (self.cpu.ax & 0xFF00) | (count as u16);
        }

        51 // Approximate INT instruction timing
    }

    /// INT 13h, AH=08h: Get drive parameters
    fn int13h_get_drive_params(&mut self) -> u32 {
        use crate::disk::DiskController;

        // DL = drive number
        let drive = (self.cpu.dx & 0xFF) as u8;

        // Get drive parameters
        if let Some((cylinders, sectors_per_track, heads)) = DiskController::get_drive_params(drive)
        {
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

            self.cpu.cx = ((ch as u16) << 8) | (cl as u16);

            // DH = maximum head number (0-based)
            // DL = number of drives
            self.cpu.dx = (((heads - 1) as u16) << 8) | 0x01;

            // ES:DI = pointer to disk parameter table (set to 0x0000:0x0000 for now)
            self.cpu.es = 0x0000;
            self.cpu.di = 0x0000;

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

    /// Set or clear the carry flag
    fn set_carry_flag(&mut self, value: bool) {
        const FLAG_CF: u16 = 0x0001;
        if value {
            self.cpu.flags |= FLAG_CF;
        } else {
            self.cpu.flags &= !FLAG_CF;
        }
    }

    /// Set or clear the zero flag
    fn set_zero_flag(&mut self, value: bool) {
        const FLAG_ZF: u16 = 0x0040;
        if value {
            self.cpu.flags |= FLAG_ZF;
        } else {
            self.cpu.flags &= !FLAG_ZF;
        }
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
            _ => {
                // Unsupported function - do nothing
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
        self.cpu.cx = ((ticks >> 16) & 0xFFFF) as u16;
        self.cpu.dx = (ticks & 0xFFFF) as u16;

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

        self.cpu.cx = ((hours_bcd as u16) << 8) | (minutes_bcd as u16);
        self.cpu.dx = (seconds_bcd as u16) << 8;

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

        self.cpu.cx = ((century_bcd as u16) << 8) | (year_bcd as u16);
        self.cpu.dx = ((month_bcd as u16) << 8) | (day_bcd as u16);

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
        self.cpu.ax = ax;
        self.cpu.bx = bx;
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
        self.cpu.bx = buttons;
        self.cpu.cx = x as u16;
        self.cpu.dx = y as u16;
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
        let (buttons, count, x, y) = self.cpu.memory.mouse.get_button_press_info(button);
        self.cpu.ax = buttons;
        self.cpu.bx = count;
        self.cpu.cx = x as u16;
        self.cpu.dx = y as u16;
        51
    }

    /// INT 33h, AX=0006h - Get button release information
    #[allow(dead_code)] // Called from handle_int33h
    fn int33h_get_button_release_info(&mut self) -> u32 {
        let button = self.cpu.bx;
        let (buttons, count, x, y) = self.cpu.memory.mouse.get_button_release_info(button);
        self.cpu.ax = buttons;
        self.cpu.bx = count;
        self.cpu.cx = x as u16;
        self.cpu.dx = y as u16;
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
        self.cpu.memory.mouse.set_mickey_ratio(horiz, vert);
        51
    }

    /// INT 33h, AX=0024h - Get mouse driver version
    #[allow(dead_code)] // Called from handle_int33h
    fn int33h_get_driver_version(&mut self) -> u32 {
        let (version, mtype, irq) = self.cpu.memory.mouse.get_driver_version();
        self.cpu.bx = version;
        self.cpu.cx = ((mtype as u16) << 8) | (irq as u16);
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
            0x88 => self.int15h_get_extended_memory_size(),
            0xE8 => {
                // Get Extended Memory Size (32-bit)
                let al = (self.cpu.ax & 0xFF) as u8;
                match al {
                    0x01 => self.int15h_get_extended_memory_size_e801(),
                    0x20 => self.int15h_query_system_address_map(),
                    _ => {
                        // Unsupported function
                        self.set_carry_flag(true);
                        51
                    }
                }
            }
            _ => {
                // Unsupported function - set carry flag to indicate error
                self.set_carry_flag(true);
                51
            }
        }
    }

    /// INT 15h, AH=88h - Get Extended Memory Size
    #[allow(dead_code)] // Called from handle_int15h
    fn int15h_get_extended_memory_size(&mut self) -> u32 {
        // Return extended memory size in KB (above 1MB)
        let extended_kb = self.cpu.memory.xms.total_extended_memory_kb();
        self.cpu.ax = extended_kb.min(0xFFFF) as u16;
        self.set_carry_flag(false);
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

        self.cpu.ax = mem_1_16mb as u16;
        self.cpu.bx = mem_above_16mb as u16;
        self.cpu.cx = mem_1_16mb as u16;
        self.cpu.dx = mem_above_16mb as u16;
        self.set_carry_flag(false);
        51
    }

    /// INT 15h, AX=E820h - Query System Address Map (stub)
    #[allow(dead_code)] // Called from handle_int15h
    fn int15h_query_system_address_map(&mut self) -> u32 {
        // This would return memory map entries
        // For now, just indicate no more entries
        self.cpu.bx = 0; // No continuation
        self.set_carry_flag(true); // No more entries
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
            0x43 => self.int2fh_xms_installation_check(),
            _ => {
                // Unsupported function - just return
                51
            }
        }
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
                    self.cpu.ax = 0x00; // Not installed
                }
                51
            }
            0x10 => {
                // Get XMS driver address
                // In a real implementation, this would return ES:BX pointing to the XMS driver
                // For now, we'll use a fake segment:offset
                self.cpu.es = 0xC000; // Fake XMS driver segment
                self.cpu.bx = 0x0000; // Offset
                51
            }
            _ => 51,
        }
    }

    /// Get a reference to the bus
    pub fn bus(&self) -> &PcBus {
        &self.cpu.memory
    }

    /// Get a mutable reference to the bus
    pub fn bus_mut(&mut self) -> &mut PcBus {
        &mut self.cpu.memory
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
}

/// Convert PC scancode to ASCII character (simplified mapping)
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
        SCANCODE_ENTER => b'\r',
        SCANCODE_BACKSPACE => 0x08,
        SCANCODE_TAB => b'\t',
        SCANCODE_ESC => 0x1B,
        _ => 0, // No ASCII equivalent
    }
}

/// CPU register state for save/load
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CpuRegisters {
    pub ax: u16,
    pub bx: u16,
    pub cx: u16,
    pub dx: u16,
    pub si: u16,
    pub di: u16,
    pub bp: u16,
    pub sp: u16,
    pub cs: u16,
    pub ds: u16,
    pub es: u16,
    pub ss: u16,
    pub ip: u16,
    pub flags: u16,
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
        assert_eq!(cpu.cpu.ip, 0x0000);
        assert_eq!(cpu.cpu.ss, 0x0000);
        assert_eq!(cpu.cpu.sp, 0xFFFE);
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
        assert_eq!(cpu.cpu.ax, 0x0000);
        assert_eq!(cpu.cpu.cs, 0xFFFF);
        assert_eq!(cpu.cpu.ip, 0x0000);
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
        assert_eq!(cpu.cpu.ax, 0x0000);

        cpu.set_registers(&regs);
        assert_eq!(cpu.cpu.ax, 0x1234);
        assert_eq!(cpu.cpu.bx, 0x5678);
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
        assert_eq!((cpu.cpu.ax >> 8) & 0xFF, SCANCODE_A as u16); // AH = scancode
        assert_eq!(cpu.cpu.ax & 0xFF, b'a' as u16); // AL = ASCII 'a'
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
        assert_eq!((cpu.cpu.ax >> 8) & 0xFF, SCANCODE_B as u16); // AH = scancode
        assert_eq!(cpu.cpu.ax & 0xFF, b'b' as u16); // AL = ASCII 'b'
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
        assert_eq!(cpu.cpu.ax, 0x0000);
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
            assert_eq!((cpu.cpu.ax >> 8) & 0xFF, expected_scan as u16);
            assert_eq!(cpu.cpu.ax & 0xFF, expected_ascii as u16);
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
        let video_addr = 0xB8000 + (1 * 80) * 2;
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
        let video_addr = 0xB8000 + (1 * 80) * 2;
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
        assert_eq!(cpu.cpu.bx, 0x0008); // VGA with color display
    }
}
