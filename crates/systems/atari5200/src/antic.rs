//! ANTIC (Alphanumeric Television Interface Controller)
//!
//! The ANTIC is a DMA-driven display list processor that generates the playfield
//! display for the Atari 5200. It reads a display list from memory which contains
//! instructions for each display line, then fetches playfield data as needed.
//!
//! # Display List
//! The display list is a program that ANTIC reads to determine how to render each
//! scanline. Instructions include:
//! - Blank lines (modes 0x00-0x07): 1-8 blank scanlines
//! - Character modes (modes 0x02-0x07): Text display with different resolutions
//! - Map/bitmap modes (modes 0x08-0x0F): Graphics modes with varying resolution/color
//!
//! # Display List Instruction Format
//! - Bits 0-3: Mode (display mode or blank line count)
//! - Bit 4: Horizontal scrolling enable
//! - Bit 5: Vertical scrolling enable
//! - Bit 6: DLI (Display List Interrupt) enable
//! - Bit 7: Line memory scan counter load (followed by 2-byte address)
//!
//! # Screen Dimensions
//! - Standard: 320×192 (high-res) or 160×192 (low-res)
//! - Narrow: 256×192 or 128×192
//! - Wide: 384×192 or 192×192
//!
//! # Registers (at $D400-$D40F)
//! - DMACTL ($D400): DMA control
//! - CHACTL ($D401): Character control
//! - DLISTL/H ($D402-$D403): Display list pointer
//! - HSCROL ($D404): Horizontal scroll
//! - VSCROL ($D405): Vertical scroll
//! - PMBASE ($D407): Player/missile base address
//! - CHBASE ($D409): Character set base address
//! - WSYNC ($D40A): Wait for horizontal sync
//! - VCOUNT ($D40B): Vertical line counter (read)
//! - PENH/V ($D40C-$D40D): Light pen (read)
//! - NMIEN ($D40E): NMI enable
//! - NMIST ($D40F): NMI status (read) / NMIRES (write)

use serde::{Deserialize, Serialize};

/// ANTIC display mode types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnticMode {
    /// Blank lines
    Blank(u8),
    /// Jump - load display list address
    Jump,
    /// Jump and wait for vertical blank
    JumpVBlank,
    /// Character mode 2: 40 chars/line, 8 scanlines/char, 2 colors
    Mode2,
    /// Character mode 3: 40 chars, 10 scanlines/char, 2 colors
    Mode3,
    /// Character mode 4: 40 chars, 8 scanlines/char, 5 colors
    Mode4,
    /// Character mode 5: 40 chars, 16 scanlines/char, 5 colors
    Mode5,
    /// Character mode 6: 20 chars, 8 scanlines/char, 5 colors (double width)
    Mode6,
    /// Character mode 7: 20 chars, 16 scanlines/char, 5 colors (double width)
    Mode7,
    /// Map mode 8: 40 pixels/line, 8 scanlines, 4 colors
    Mode8,
    /// Map mode 9: 80 pixels/line, 4 scanlines, 2 colors
    Mode9,
    /// Map mode A: 80 pixels/line, 4 scanlines, 4 colors
    ModeA,
    /// Map mode B: 160 pixels/line, 2 scanlines, 2 colors
    ModeB,
    /// Map mode C: 160 pixels/line, 1 scanline, 2 colors
    ModeC,
    /// Map mode D: 160 pixels/line, 2 scanlines, 4 colors
    ModeD,
    /// Map mode E: 160 pixels/line, 1 scanline, 4 colors
    ModeE,
    /// Map mode F: 320 pixels/line, 1 scanline, 2 colors (hi-res)
    ModeF,
}

/// Scanlines per mode row
impl AnticMode {
    pub fn scanlines_per_row(self) -> u16 {
        match self {
            AnticMode::Blank(n) => n as u16,
            AnticMode::Jump | AnticMode::JumpVBlank => 0,
            AnticMode::Mode2 => 8,
            AnticMode::Mode3 => 10,
            AnticMode::Mode4 => 8,
            AnticMode::Mode5 => 16,
            AnticMode::Mode6 => 8,
            AnticMode::Mode7 => 16,
            AnticMode::Mode8 => 8,
            AnticMode::Mode9 => 4,
            AnticMode::ModeA => 4,
            AnticMode::ModeB => 2,
            AnticMode::ModeC => 1,
            AnticMode::ModeD => 2,
            AnticMode::ModeE => 1,
            AnticMode::ModeF => 1,
        }
    }

    /// Bytes per scanline of data
    pub fn bytes_per_line(self) -> usize {
        match self {
            AnticMode::Blank(_) | AnticMode::Jump | AnticMode::JumpVBlank => 0,
            AnticMode::Mode2 | AnticMode::Mode3 | AnticMode::Mode4 | AnticMode::Mode5 => 40,
            AnticMode::Mode6 | AnticMode::Mode7 => 20,
            AnticMode::Mode8 => 10,
            AnticMode::Mode9 | AnticMode::ModeA => 20,
            AnticMode::ModeB | AnticMode::ModeC => 20,
            AnticMode::ModeD | AnticMode::ModeE => 40,
            AnticMode::ModeF => 40,
        }
    }

    /// Pixels per byte
    pub fn pixels_per_byte(self) -> usize {
        match self {
            AnticMode::Blank(_) | AnticMode::Jump | AnticMode::JumpVBlank => 0,
            // Character modes: pixels depend on character data
            AnticMode::Mode2 | AnticMode::Mode3 => 8,
            AnticMode::Mode4 | AnticMode::Mode5 => 8,
            AnticMode::Mode6 | AnticMode::Mode7 => 16,
            // Map modes: packed pixel data
            AnticMode::Mode8 => 32, // 4 color, 8 pixels/byte? Actually 40px from 10 bytes = 4px/byte
            AnticMode::Mode9 | AnticMode::ModeB | AnticMode::ModeC => 8, // 1bpp
            AnticMode::ModeA | AnticMode::ModeD | AnticMode::ModeE => 4, // 2bpp
            AnticMode::ModeF => 8,  // 1bpp hi-res
        }
    }
}

/// A decoded display list instruction
#[derive(Debug, Clone, Copy)]
pub struct DListInstruction {
    pub mode: u8,
    pub dli: bool,
    pub lms: bool, // Load memory scan
    pub hscroll: bool,
    pub vscroll: bool,
    pub lms_addr: u16, // Memory scan address (if lms is true)
}

/// ANTIC chip state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Antic {
    // Registers
    dmactl: u8, // DMA control
    chactl: u8, // Character control
    dlist: u16, // Display list pointer
    hscrol: u8, // Horizontal scroll (0-15)
    vscrol: u8, // Vertical scroll (0-15)
    pmbase: u8, // Player/missile base (page number)
    chbase: u8, // Character set base (page number)
    nmien: u8,  // NMI enable
    nmist: u8,  // NMI status

    // Internal state
    vcount: u8,    // Vertical line counter (in half-lines, 0-155)
    scanline: u16, // Current scanline (0-261 for NTSC)
    wsync_requested: bool,

    // Display list processing
    dlist_pc: u16,     // Current position in display list
    memory_scan: u16,  // Current data fetch address
    current_mode: u8,  // Current display list mode
    row_scanline: u16, // Current scanline within current mode row
    row_height: u16,   // Total scanlines for current mode row

    // NMI tracking
    dli_pending: bool,
    vbi_pending: bool,
}

impl Default for Antic {
    fn default() -> Self {
        Self::new()
    }
}

impl Antic {
    pub fn new() -> Self {
        Self {
            dmactl: 0,
            chactl: 0,
            dlist: 0,
            hscrol: 0,
            vscrol: 0,
            pmbase: 0,
            chbase: 0,
            nmien: 0,
            nmist: 0,
            vcount: 0,
            scanline: 0,
            wsync_requested: false,
            dlist_pc: 0,
            memory_scan: 0,
            current_mode: 0,
            row_scanline: 0,
            row_height: 0,
            dli_pending: false,
            vbi_pending: false,
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Read an ANTIC register
    pub fn read(&self, addr: u16) -> u8 {
        match addr & 0x0F {
            0x0B => self.vcount,
            0x0C => 0, // PENH - light pen horizontal (stub)
            0x0D => 0, // PENV - light pen vertical (stub)
            0x0F => self.nmist,
            _ => 0xFF,
        }
    }

    /// Write an ANTIC register
    pub fn write(&mut self, addr: u16, val: u8) {
        match addr & 0x0F {
            0x00 => self.dmactl = val,
            0x01 => self.chactl = val,
            0x02 => self.dlist = (self.dlist & 0xFF00) | (val as u16),
            0x03 => self.dlist = (self.dlist & 0x00FF) | ((val as u16) << 8),
            0x04 => self.hscrol = val & 0x0F,
            0x05 => self.vscrol = val & 0x0F,
            0x07 => self.pmbase = val,
            0x09 => self.chbase = val,
            0x0A => self.wsync_requested = true,
            0x0E => self.nmien = val,
            0x0F => {
                // NMIRES - reset NMI status
                self.nmist = 0;
                self.dli_pending = false;
                self.vbi_pending = false;
            }
            _ => {}
        }
    }

    /// Check and clear WSYNC request
    pub fn take_wsync_request(&mut self) -> bool {
        let requested = self.wsync_requested;
        self.wsync_requested = false;
        requested
    }

    /// Clock ANTIC for one scanline
    /// Returns true if a new frame started (VBI)
    pub fn clock_scanline(&mut self) -> bool {
        self.scanline += 1;
        self.vcount = (self.scanline / 2) as u8;

        // NTSC: 262 scanlines per frame
        if self.scanline >= 262 {
            self.scanline = 0;
            self.vcount = 0;
            self.dlist_pc = self.dlist;
            self.row_scanline = 0;
            self.row_height = 0;

            // Set VBI status and trigger NMI if enabled
            self.nmist |= 0x40; // VBI flag
            if self.nmien & 0x40 != 0 {
                self.vbi_pending = true;
            }
            return true;
        }

        // Check for DLI at end of mode row
        if self.row_height > 0 {
            self.row_scanline += 1;
            if self.row_scanline >= self.row_height {
                self.row_scanline = 0;
                self.row_height = 0;
                // DLI triggers at last scanline of row
            }
        }

        false
    }

    /// Get playfield width in pixels based on DMACTL
    pub fn playfield_width(&self) -> usize {
        match self.dmactl & 0x03 {
            0 => 0,   // No playfield
            1 => 256, // Narrow
            2 => 320, // Standard
            3 => 384, // Wide
            _ => unreachable!(),
        }
    }

    /// Check if DMA is enabled
    pub fn dma_enabled(&self) -> bool {
        self.dmactl & 0x03 != 0
    }

    /// Check if player DMA is enabled
    pub fn player_dma(&self) -> bool {
        self.dmactl & 0x08 != 0
    }

    /// Check if missile DMA is enabled
    pub fn missile_dma(&self) -> bool {
        self.dmactl & 0x04 != 0
    }

    /// Check if single-line player/missile resolution
    pub fn single_line_pm(&self) -> bool {
        self.dmactl & 0x10 != 0
    }

    /// Get the display list pointer
    pub fn dlist(&self) -> u16 {
        self.dlist
    }

    /// Get display list PC
    pub fn dlist_pc(&self) -> u16 {
        self.dlist_pc
    }

    /// Set display list PC
    pub fn set_dlist_pc(&mut self, addr: u16) {
        self.dlist_pc = addr;
    }

    /// Get memory scan counter
    pub fn memory_scan(&self) -> u16 {
        self.memory_scan
    }

    /// Set memory scan counter
    pub fn set_memory_scan(&mut self, addr: u16) {
        self.memory_scan = addr;
    }

    /// Advance memory scan by n bytes
    pub fn advance_memory_scan(&mut self, n: u16) {
        self.memory_scan = self.memory_scan.wrapping_add(n);
    }

    /// Get current mode
    pub fn current_mode(&self) -> u8 {
        self.current_mode
    }

    /// Set current mode
    pub fn set_current_mode(&mut self, mode: u8) {
        self.current_mode = mode;
    }

    /// Set row height
    pub fn set_row_height(&mut self, height: u16) {
        self.row_height = height;
        self.row_scanline = 0;
    }

    /// Get row scanline
    pub fn row_scanline(&self) -> u16 {
        self.row_scanline
    }

    /// Get current scanline
    pub fn scanline(&self) -> u16 {
        self.scanline
    }

    /// Get character base address
    pub fn char_base(&self) -> u16 {
        (self.chbase as u16) << 8
    }

    /// Get player/missile base address
    pub fn pm_base(&self) -> u16 {
        (self.pmbase as u16) << 8
    }

    /// Get character control register
    pub fn chactl(&self) -> u8 {
        self.chactl
    }

    /// Check and take VBI pending
    pub fn take_vbi_pending(&mut self) -> bool {
        let pending = self.vbi_pending;
        self.vbi_pending = false;
        pending
    }

    /// Check and take DLI pending
    pub fn take_dli_pending(&mut self) -> bool {
        let pending = self.dli_pending;
        self.dli_pending = false;
        pending
    }

    /// Set DLI pending
    pub fn set_dli_pending(&mut self) {
        if self.nmien & 0x80 != 0 {
            self.nmist |= 0x80;
            self.dli_pending = true;
        }
    }

    /// Get vertical scroll value
    pub fn vscrol(&self) -> u8 {
        self.vscrol
    }

    /// Get horizontal scroll value
    pub fn hscrol(&self) -> u8 {
        self.hscrol
    }

    /// Get DMACTL
    pub fn dmactl(&self) -> u8 {
        self.dmactl
    }

    /// Decode mode byte to AnticMode
    pub fn decode_mode(mode_byte: u8) -> AnticMode {
        match mode_byte & 0x0F {
            0x00 => AnticMode::Blank(1),
            0x01 => AnticMode::JumpVBlank,
            0x02 => AnticMode::Mode2,
            0x03 => AnticMode::Mode3,
            0x04 => AnticMode::Mode4,
            0x05 => AnticMode::Mode5,
            0x06 => AnticMode::Mode6,
            0x07 => AnticMode::Mode7,
            0x08 => AnticMode::Mode8,
            0x09 => AnticMode::Mode9,
            0x0A => AnticMode::ModeA,
            0x0B => AnticMode::ModeB,
            0x0C => AnticMode::ModeC,
            0x0D => AnticMode::ModeD,
            0x0E => AnticMode::ModeE,
            0x0F => AnticMode::ModeF,
            _ => AnticMode::Blank(1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dma_control() {
        let mut antic = Antic::new();
        assert!(!antic.dma_enabled());
        antic.write(0xD400, 0x22); // Standard playfield
        assert!(antic.dma_enabled());
        assert_eq!(antic.playfield_width(), 320);
    }

    #[test]
    fn test_display_list_pointer() {
        let mut antic = Antic::new();
        antic.write(0xD402, 0x00); // DLISTL
        antic.write(0xD403, 0x40); // DLISTH
        assert_eq!(antic.dlist(), 0x4000);
    }

    #[test]
    fn test_wsync() {
        let mut antic = Antic::new();
        assert!(!antic.take_wsync_request());
        antic.write(0xD40A, 0x00); // WSYNC
        assert!(antic.take_wsync_request());
        assert!(!antic.take_wsync_request()); // Should be cleared
    }

    #[test]
    fn test_scanline_counter() {
        let mut antic = Antic::new();
        for _ in 0..261 {
            antic.clock_scanline();
        }
        assert_eq!(antic.scanline(), 261);
        // Next scanline should wrap to 0 (new frame)
        let new_frame = antic.clock_scanline();
        assert!(new_frame);
        assert_eq!(antic.scanline(), 0);
    }

    #[test]
    fn test_mode_decode() {
        assert_eq!(Antic::decode_mode(0x02).scanlines_per_row(), 8);
        assert_eq!(Antic::decode_mode(0x0F).scanlines_per_row(), 1);
        assert_eq!(Antic::decode_mode(0x0F).bytes_per_line(), 40);
    }
}
