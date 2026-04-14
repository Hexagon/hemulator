//! VI (Video Interface) - Display controller for Nintendo 64
//!
//! The VI is responsible for:
//! - Reading the framebuffer from RDRAM
//! - Generating video output (NTSC/PAL timing)
//! - Handling display configuration (resolution, format, etc.)
//!
//! ## Memory Map
//!
//! VI registers are memory-mapped at 0x04400000-0x04400037:
//! - 0x04400000: VI_STATUS - Video control/status register
//! - 0x04400004: VI_ORIGIN - Framebuffer origin address
//! - 0x04400008: VI_WIDTH - Framebuffer line width
//! - 0x0440000C: VI_INTR - Vertical interrupt
//! - 0x04400010: VI_CURRENT - Current scanline
//! - 0x04400014: VI_BURST - Timing register
//! - 0x04400018: VI_V_SYNC - Vertical sync
//! - 0x0440001C: VI_H_SYNC - Horizontal sync
//! - 0x04400020: VI_LEAP - Horizontal sync leap
//! - 0x04400024: VI_H_START - Horizontal video start
//! - 0x04400028: VI_V_START - Vertical video start
//! - 0x0440002C: VI_V_BURST - Vertical burst start/end
//! - 0x04400030: VI_X_SCALE - Horizontal scale
//! - 0x04400034: VI_Y_SCALE - Vertical scale

/// VI register offsets (relative to 0x04400000)
const VI_STATUS: u32 = 0x00;
const VI_ORIGIN: u32 = 0x04;
const VI_WIDTH: u32 = 0x08;
const VI_INTR: u32 = 0x0C;
const VI_CURRENT: u32 = 0x10;
const VI_BURST: u32 = 0x14;
const VI_V_SYNC: u32 = 0x18;
const VI_H_SYNC: u32 = 0x1C;
const VI_LEAP: u32 = 0x20;
const VI_H_START: u32 = 0x24;
const VI_V_START: u32 = 0x28;
const VI_V_BURST: u32 = 0x2C;
const VI_X_SCALE: u32 = 0x30;
const VI_Y_SCALE: u32 = 0x34;

/// VI_STATUS register bits
#[allow(dead_code)]
const VI_STATUS_TYPE_16: u32 = 0x02; // 16-bit color mode
#[allow(dead_code)]
const VI_STATUS_TYPE_32: u32 = 0x03; // 32-bit color mode
#[allow(dead_code)]
const VI_STATUS_GAMMA_DITHER: u32 = 0x04;
#[allow(dead_code)]
const VI_STATUS_GAMMA: u32 = 0x08;
#[allow(dead_code)]
const VI_STATUS_DIVOT: u32 = 0x10;
#[allow(dead_code)]
const VI_STATUS_SERRATE: u32 = 0x40;
#[allow(dead_code)]
const VI_STATUS_AA_MODE_SHIFT: u32 = 8;

/// Inspector snapshot of all VI register values.
#[derive(Debug, Clone)]
pub struct ViInspectorData {
    pub status: u32,
    pub origin: u32,
    pub width: u32,
    pub intr: u32,
    pub current: u32,
    pub v_sync: u32,
    pub h_sync: u32,
    pub h_start: u32,
    pub v_start: u32,
    pub x_scale: u32,
    pub y_scale: u32,
    pub display_width: u32,
    pub display_height: u32,
    pub color_depth: u32,
    pub display_enabled: bool,
    pub video_standard: String,
}

/// Video Interface controller
pub struct VideoInterface {
    /// VI registers
    status: u32,
    origin: u32,  // Framebuffer address in RDRAM
    width: u32,   // Width in pixels
    intr: u32,    // Vertical interrupt scanline
    current: u32, // Current scanline
    burst: u32,
    v_sync: u32, // Vertical sync period
    h_sync: u32, // Horizontal sync period
    leap: u32,
    h_start: u32, // Horizontal display start/end
    v_start: u32, // Vertical display start/end
    v_burst: u32,
    x_scale: u32, // Horizontal scale factor
    y_scale: u32, // Vertical scale factor
}

impl VideoInterface {
    /// Create a new Video Interface with NTSC defaults
    pub fn new() -> Self {
        Self {
            status: 0,
            origin: 0,
            width: 320,
            intr: 0x200, // Default to scanline 256 (0x200 >> 1 = 0x100 = 256) for NTSC vblank
            current: 0,
            burst: 0x03E52239, // NTSC burst
            v_sync: 0x020D,    // NTSC vertical sync (525 lines)
            h_sync: 0x0C15,    // NTSC horizontal sync
            leap: 0x0C150C15,
            h_start: 0x006C02EC, // NTSC horizontal start/end
            v_start: 0x00230203, // NTSC vertical start/end (35-515, height=240)
            v_burst: 0x000E0204,
            x_scale: 0x0200, // 1:1 scale
            y_scale: 0x0400, // 1:1 scale
        }
    }

    /// Reset to initial state
    #[allow(dead_code)] // Reserved for future use
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Read from VI register
    pub fn read_register(&self, offset: u32) -> u32 {
        match offset {
            VI_STATUS => self.status,
            VI_ORIGIN => self.origin,
            VI_WIDTH => self.width,
            VI_INTR => self.intr,
            VI_CURRENT => self.current,
            VI_BURST => self.burst,
            VI_V_SYNC => self.v_sync,
            VI_H_SYNC => self.h_sync,
            VI_LEAP => self.leap,
            VI_H_START => self.h_start,
            VI_V_START => self.v_start,
            VI_V_BURST => self.v_burst,
            VI_X_SCALE => self.x_scale,
            VI_Y_SCALE => self.y_scale,
            _ => 0,
        }
    }

    /// Write to VI register
    pub fn write_register(&mut self, offset: u32, value: u32) {
        use emu_core::logging::{log, LogCategory, LogLevel};

        // Log significant VI register writes
        if offset == VI_STATUS || offset == VI_ORIGIN || offset == VI_INTR {
            log(LogCategory::PPU, LogLevel::Info, || {
                format!("VI: Write to offset 0x{:02X} = 0x{:08X}", offset, value)
            });
        }

        match offset {
            VI_STATUS => self.status = value,
            VI_ORIGIN => {
                self.origin = value & 0x00FFFFFF; // 24-bit address
            }
            VI_WIDTH => self.width = value & 0xFFF,
            VI_INTR => {
                self.intr = value & 0x3FF;
                log(LogCategory::PPU, LogLevel::Info, || {
                    format!(
                        "VI: VI_INTR set to 0x{:03X} (scanline {})",
                        self.intr,
                        self.intr >> 1
                    )
                });
            }
            VI_CURRENT => {
                // Writing to VI_CURRENT clears interrupt
                self.current = 0;
            }
            VI_BURST => self.burst = value,
            VI_V_SYNC => self.v_sync = value & 0x3FF,
            VI_H_SYNC => self.h_sync = value & 0xFFF,
            VI_LEAP => self.leap = value,
            VI_H_START => self.h_start = value,
            VI_V_START => self.v_start = value,
            VI_V_BURST => self.v_burst = value,
            VI_X_SCALE => self.x_scale = value & 0xFFF,
            VI_Y_SCALE => self.y_scale = value & 0xFFF,
            _ => {}
        }
    }

    /// Update current scanline (called per frame)
    /// Returns true if scanline matches VI_INTR and interrupt should be triggered.
    /// VI_INTR and VI_CURRENT use half-line values (NTSC has 525 half-lines).
    /// Our scanline parameter is 0..262, so we compare using half-lines.
    pub fn update_scanline(&mut self, scanline: u32) -> bool {
        // Store current half-line (scanline * 2) for games that read VI_CURRENT
        let half_line = scanline * 2;
        self.current = half_line;

        // Check if either half-line for this scanline matches VI_INTR
        half_line == self.intr || half_line + 1 == self.intr
    }

    /// Get the framebuffer origin address
    #[allow(dead_code)]
    pub fn get_framebuffer_origin(&self) -> u32 {
        self.origin
    }

    /// Get the framebuffer width
    #[allow(dead_code)]
    pub fn get_width(&self) -> u32 {
        self.width
    }

    /// Get the display height calculated from VI_V_START register
    /// VI_V_START contains start in upper 10 bits and end in lower 10 bits
    pub fn get_display_height(&self) -> u32 {
        let v_start = (self.v_start >> 16) & 0x3FF;
        let v_end = self.v_start & 0x3FF;
        if v_end > v_start {
            (v_end - v_start) / 2 // Divide by 2 for interlaced
        } else {
            240 // Default NTSC height
        }
    }

    /// Get display width calculated from VI_H_START register.
    /// VI_H_START bits [25:16] = h_start, bits [9:0] = h_end (half-pixel units).
    pub fn get_display_width(&self) -> u32 {
        let h_start = (self.h_start >> 16) & 0x3FF;
        let h_end = self.h_start & 0x3FF;
        if h_end > h_start {
            (h_end - h_start) / 2 // Half-pixel units → pixels
        } else {
            320 // Default NTSC width
        }
    }

    /// Check if display is enabled
    pub fn is_enabled(&self) -> bool {
        self.status & 0x03 != 0
    }

    /// Get color depth from status register
    /// 0 = blank, 2 = 16-bit (RGBA5551), 3 = 32-bit (RGBA8888)
    pub fn get_color_depth(&self) -> u32 {
        self.status & 0x03
    }

    /// Collect a complete inspector snapshot for the N64 VI inspector tab.
    pub fn inspector_data(&self) -> ViInspectorData {
        // Determine video standard from VI_V_SYNC value.
        // NTSC: 525 lines (VI_V_SYNC = 0x020D), PAL: 625 lines (VI_V_SYNC = 0x0271).
        let video_standard = match self.v_sync {
            0x020D => "NTSC",
            0x0271 => "PAL",
            _ => "Unknown",
        }
        .to_string();

        ViInspectorData {
            status: self.status,
            origin: self.origin,
            width: self.width,
            intr: self.intr,
            current: self.current,
            v_sync: self.v_sync,
            h_sync: self.h_sync,
            h_start: self.h_start,
            v_start: self.v_start,
            x_scale: self.x_scale,
            y_scale: self.y_scale,
            display_width: self.get_display_width(),
            display_height: self.get_display_height(),
            color_depth: self.get_color_depth(),
            display_enabled: self.is_enabled(),
            video_standard,
        }
    }
}

impl Default for VideoInterface {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vi_creation() {
        let vi = VideoInterface::new();
        assert_eq!(vi.width, 320);
        assert_eq!(vi.origin, 0);
    }

    #[test]
    fn test_vi_reset() {
        let mut vi = VideoInterface::new();
        vi.write_register(VI_ORIGIN, 0x00100000);
        vi.write_register(VI_WIDTH, 640);

        vi.reset();

        assert_eq!(vi.origin, 0);
        assert_eq!(vi.width, 320);
    }

    #[test]
    fn test_vi_origin_register() {
        let mut vi = VideoInterface::new();
        vi.write_register(VI_ORIGIN, 0x00100000);
        assert_eq!(vi.read_register(VI_ORIGIN), 0x00100000);
        assert_eq!(vi.get_framebuffer_origin(), 0x00100000);
    }

    #[test]
    fn test_vi_width_register() {
        let mut vi = VideoInterface::new();
        vi.write_register(VI_WIDTH, 640);
        assert_eq!(vi.read_register(VI_WIDTH), 640);
        assert_eq!(vi.get_width(), 640);
    }

    #[test]
    fn test_vi_current_register() {
        let mut vi = VideoInterface::new();
        vi.update_scanline(100);
        // VI_CURRENT reports half-lines (scanline * 2)
        assert_eq!(vi.read_register(VI_CURRENT), 200);

        // Writing to VI_CURRENT clears it
        vi.write_register(VI_CURRENT, 0);
        assert_eq!(vi.read_register(VI_CURRENT), 0);
    }

    #[test]
    fn test_vi_status_register() {
        let mut vi = VideoInterface::new();
        assert!(!vi.is_enabled());

        vi.write_register(VI_STATUS, VI_STATUS_TYPE_16);
        assert!(vi.is_enabled());
        assert_eq!(vi.get_color_depth(), 2);

        vi.write_register(VI_STATUS, VI_STATUS_TYPE_32);
        assert_eq!(vi.get_color_depth(), 3);
    }

    #[test]
    fn test_vi_scale_registers() {
        let mut vi = VideoInterface::new();

        vi.write_register(VI_X_SCALE, 0x0400);
        vi.write_register(VI_Y_SCALE, 0x0800);

        assert_eq!(vi.read_register(VI_X_SCALE), 0x0400);
        assert_eq!(vi.read_register(VI_Y_SCALE), 0x0800);
    }

    #[test]
    fn test_vi_sync_registers() {
        let mut vi = VideoInterface::new();

        // Test default NTSC values
        assert_eq!(vi.read_register(VI_V_SYNC), 0x020D);
        assert_eq!(vi.read_register(VI_H_SYNC), 0x0C15);

        // Test writing new values (PAL mode)
        vi.write_register(VI_V_SYNC, 0x0271);
        assert_eq!(vi.read_register(VI_V_SYNC), 0x0271);
    }

    #[test]
    fn test_vi_interrupt_generation() {
        let mut vi = VideoInterface::new();

        // Set interrupt line to scanline 100 (stored as 200 in VI_INTR)
        vi.write_register(VI_INTR, 200);

        // Update to different scanline - no interrupt
        assert!(!vi.update_scanline(50));

        // Update to matching scanline - interrupt triggered
        assert!(vi.update_scanline(100));

        // Update past the interrupt line - no interrupt
        assert!(!vi.update_scanline(150));
    }

    #[test]
    fn test_vi_intr_register() {
        let mut vi = VideoInterface::new();

        // VI_INTR stores the scanline * 2
        vi.write_register(VI_INTR, 0x200); // Scanline 256
        assert_eq!(vi.read_register(VI_INTR), 0x200);

        // Verify masking (10 bits)
        vi.write_register(VI_INTR, 0xFFFFFFFF);
        assert_eq!(vi.read_register(VI_INTR), 0x3FF);
    }

    #[test]
    fn test_vi_display_width_ntsc() {
        let vi = VideoInterface::new();
        // NTSC default: h_start = 0x006C02EC → start=0x6C=108, end=0x2EC=748
        // width = (748 - 108) / 2 = 320
        assert_eq!(vi.get_display_width(), 320);
    }

    #[test]
    fn test_vi_display_width_custom() {
        let mut vi = VideoInterface::new();
        // Set h_start: start=0x80 (128), end=0x380 (896) → (896-128)/2 = 384
        vi.write_register(VI_H_START, (0x080u32 << 16) | 0x380);
        assert_eq!(vi.get_display_width(), 384);
    }

    #[test]
    fn test_vi_display_width_fallback() {
        let mut vi = VideoInterface::new();
        // Set h_end <= h_start to trigger fallback
        vi.write_register(VI_H_START, (0x300u32 << 16) | 0x100);
        assert_eq!(vi.get_display_width(), 320);
    }

    #[test]
    fn test_vi_inspector_data_video_standard() {
        let mut vi = VideoInterface::new();

        // NTSC default (v_sync = 0x020D)
        let data = vi.inspector_data();
        assert_eq!(data.video_standard, "NTSC");

        // PAL (v_sync = 0x0271)
        vi.write_register(VI_V_SYNC, 0x0271);
        let data = vi.inspector_data();
        assert_eq!(data.video_standard, "PAL");

        // Unknown value
        vi.write_register(VI_V_SYNC, 0x01FF);
        let data = vi.inspector_data();
        assert_eq!(data.video_standard, "Unknown");
    }

    #[test]
    fn test_vi_inspector_data_display_width() {
        let vi = VideoInterface::new();
        let data = vi.inspector_data();
        // With NTSC defaults, display_width should be 320 (from H_START)
        assert_eq!(data.display_width, 320);
        // framebuffer width (VI_WIDTH) is also 320
        assert_eq!(data.width, 320);
    }
}
