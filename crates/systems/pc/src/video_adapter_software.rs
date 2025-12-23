//! Software CGA Video Adapter - CPU-based text mode rendering
//!
//! This module implements the `VideoAdapter` trait using software (CPU-based)
//! rendering for CGA text mode (80x25 characters). This is the default adapter.

use super::video_adapter::VideoAdapter;
use emu_core::types::Frame;

/// CGA text mode colors
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum CgaColor {
    Black = 0,
    Blue = 1,
    Green = 2,
    Cyan = 3,
    Red = 4,
    Magenta = 5,
    Brown = 6,
    LightGray = 7,
    DarkGray = 8,
    LightBlue = 9,
    LightGreen = 10,
    LightCyan = 11,
    LightRed = 12,
    LightMagenta = 13,
    Yellow = 14,
    White = 15,
}

impl CgaColor {
    /// Convert CGA color to RGB888
    pub fn to_rgb(self) -> u32 {
        match self {
            CgaColor::Black => 0xFF000000,
            CgaColor::Blue => 0xFF0000AA,
            CgaColor::Green => 0xFF00AA00,
            CgaColor::Cyan => 0xFF00AAAA,
            CgaColor::Red => 0xFFAA0000,
            CgaColor::Magenta => 0xFFAA00AA,
            CgaColor::Brown => 0xFFAA5500,
            CgaColor::LightGray => 0xFFAAAAAA,
            CgaColor::DarkGray => 0xFF555555,
            CgaColor::LightBlue => 0xFF5555FF,
            CgaColor::LightGreen => 0xFF55FF55,
            CgaColor::LightCyan => 0xFF55FFFF,
            CgaColor::LightRed => 0xFFFF5555,
            CgaColor::LightMagenta => 0xFFFF55FF,
            CgaColor::Yellow => 0xFFFFFF55,
            CgaColor::White => 0xFFFFFFFF,
        }
    }

    /// Create from 4-bit color value
    pub fn from_u8(val: u8) -> Self {
        match val & 0x0F {
            0 => CgaColor::Black,
            1 => CgaColor::Blue,
            2 => CgaColor::Green,
            3 => CgaColor::Cyan,
            4 => CgaColor::Red,
            5 => CgaColor::Magenta,
            6 => CgaColor::Brown,
            7 => CgaColor::LightGray,
            8 => CgaColor::DarkGray,
            9 => CgaColor::LightBlue,
            10 => CgaColor::LightGreen,
            11 => CgaColor::LightCyan,
            12 => CgaColor::LightRed,
            13 => CgaColor::LightMagenta,
            14 => CgaColor::Yellow,
            _ => CgaColor::White,
        }
    }
}

/// Software-based CGA text mode video adapter
pub struct SoftwareCgaAdapter {
    /// Framebuffer
    #[allow(dead_code)] // Used via trait methods get_frame/get_frame_mut
    framebuffer: Frame,
    /// Text mode width in characters
    width: usize,
    /// Text mode height in characters
    height: usize,
    /// Character width in pixels
    char_width: usize,
    /// Character height in pixels
    char_height: usize,
}

impl SoftwareCgaAdapter {
    /// Create a new CGA video adapter for 80x25 text mode
    pub fn new() -> Self {
        let width = 80;
        let height = 25;
        let char_width = 8;
        let char_height = 16;
        let fb_width = width * char_width;
        let fb_height = height * char_height;

        Self {
            framebuffer: Frame::new(fb_width as u32, fb_height as u32),
            width,
            height,
            char_width,
            char_height,
        }
    }

    /// Render a single character at the specified pixel position
    fn render_char(
        &self,
        char_code: u8,
        fg_color: CgaColor,
        bg_color: CgaColor,
        x: usize,
        y: usize,
        pixels: &mut [u32],
    ) {
        let fg_rgb = fg_color.to_rgb();
        let bg_rgb = bg_color.to_rgb();

        // Use IBM PC 8x16 font data
        let glyph = get_font_glyph(char_code);

        for row in 0..self.char_height {
            let byte_idx = row.min(glyph.len() - 1);
            let bits = glyph[byte_idx];

            for col in 0..self.char_width {
                let pixel_x = x + col;
                let pixel_y = y + row;

                if pixel_y >= self.fb_height() || pixel_x >= self.fb_width() {
                    continue;
                }

                let pixel_idx = pixel_y * self.fb_width() + pixel_x;
                if pixel_idx >= pixels.len() {
                    continue;
                }

                // Check if this pixel should be foreground or background
                let bit = (bits >> (7 - col)) & 1;
                pixels[pixel_idx] = if bit == 1 { fg_rgb } else { bg_rgb };
            }
        }
    }
}

impl Default for SoftwareCgaAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl VideoAdapter for SoftwareCgaAdapter {
    fn init(&mut self, width: usize, height: usize) {
        let fb_width = width;
        let fb_height = height;
        self.framebuffer = Frame::new(fb_width as u32, fb_height as u32);
        // Recalculate text mode dimensions based on character size
        self.width = fb_width / self.char_width;
        self.height = fb_height / self.char_height;
    }

    fn get_frame(&self) -> &Frame {
        &self.framebuffer
    }

    fn get_frame_mut(&mut self) -> &mut Frame {
        &mut self.framebuffer
    }

    fn fb_width(&self) -> usize {
        self.width * self.char_width
    }

    fn fb_height(&self) -> usize {
        self.height * self.char_height
    }

    fn render(&self, vram: &[u8], pixels: &mut [u32]) {
        // Ensure we have enough VRAM for text mode
        let required_vram = self.width * self.height * 2;
        if vram.len() < required_vram {
            return;
        }

        // Clear the framebuffer to black
        pixels.fill(0xFF000000);

        // Render each character cell
        for row in 0..self.height {
            for col in 0..self.width {
                let cell_offset = (row * self.width + col) * 2;
                let char_code = vram[cell_offset];
                let attr = vram[cell_offset + 1];

                let fg_color = CgaColor::from_u8(attr & 0x0F);
                let bg_color = CgaColor::from_u8((attr >> 4) & 0x0F);

                self.render_char(
                    char_code,
                    fg_color,
                    bg_color,
                    col * self.char_width,
                    row * self.char_height,
                    pixels,
                );
            }
        }
    }

    fn reset(&mut self) {
        // Clear the framebuffer
        self.framebuffer.pixels.fill(0xFF000000);
    }

    fn name(&self) -> &str {
        "Software CGA Adapter"
    }

    fn resize(&mut self, width: usize, height: usize) {
        self.init(width, height);
    }
}

/// Get font glyph data for a character (simplified 8x16 font)
/// This is a simplified version - a real implementation would use the full IBM PC font ROM
fn get_font_glyph(char_code: u8) -> &'static [u8] {
    // Basic 8x16 bitmap font for common ASCII characters
    // Each character is 16 bytes, one byte per row, MSB on the left
    static FONT_DATA: [[u8; 16]; 256] = generate_basic_font();

    let glyph = &FONT_DATA[char_code as usize];

    // If the glyph is all zeros (except for space which should be blank),
    // use a simple box pattern for undefined characters
    if char_code != 0x20 && glyph.iter().all(|&b| b == 0) {
        // Return a reference to a static box pattern
        static BOX_GLYPH: [u8; 16] = [
            0x00, 0x7E, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x7E, 0x00,
            0x00, 0x00,
        ];
        &BOX_GLYPH
    } else {
        glyph
    }
}

/// Generate a basic font covering essential ASCII characters
const fn generate_basic_font() -> [[u8; 16]; 256] {
    let mut font = [[0u8; 16]; 256];

    // We'll define some basic characters here
    // This is a simplified implementation - ideally would use full IBM PC font ROM data

    // Space (0x20)
    font[0x20] = [0x00; 16];

    // Exclamation mark (0x21) - '!'
    font[0x21] = [
        0x00, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x00, 0x18, 0x18, 0x00, 0x00, 0x00,
        0x00,
    ];

    // Letter 'A' (0x41)
    font[0x41] = [
        0x00, 0x00, 0x18, 0x3C, 0x66, 0x66, 0x66, 0x7E, 0x66, 0x66, 0x66, 0x66, 0x00, 0x00, 0x00,
        0x00,
    ];

    // Letter 'H' (0x48)
    font[0x48] = [
        0x00, 0x00, 0x66, 0x66, 0x66, 0x66, 0x7E, 0x66, 0x66, 0x66, 0x66, 0x66, 0x00, 0x00, 0x00,
        0x00,
    ];

    // Letter 'e' (0x65)
    font[0x65] = [
        0x00, 0x00, 0x00, 0x00, 0x00, 0x3C, 0x66, 0x66, 0x7E, 0x60, 0x60, 0x3E, 0x00, 0x00, 0x00,
        0x00,
    ];

    // Letter 'l' (0x6C)
    font[0x6C] = [
        0x00, 0x00, 0x38, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x3C, 0x00, 0x00, 0x00,
        0x00,
    ];

    // Letter 'o' (0x6F)
    font[0x6F] = [
        0x00, 0x00, 0x00, 0x00, 0x00, 0x3C, 0x66, 0x66, 0x66, 0x66, 0x66, 0x3C, 0x00, 0x00, 0x00,
        0x00,
    ];

    font
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cga_color_to_rgb() {
        assert_eq!(CgaColor::Black.to_rgb(), 0xFF000000);
        assert_eq!(CgaColor::White.to_rgb(), 0xFFFFFFFF);
        assert_eq!(CgaColor::Blue.to_rgb(), 0xFF0000AA);
    }

    #[test]
    fn test_cga_color_from_u8() {
        assert_eq!(CgaColor::from_u8(0) as u8, CgaColor::Black as u8);
        assert_eq!(CgaColor::from_u8(15) as u8, CgaColor::White as u8);
        assert_eq!(CgaColor::from_u8(0x1F) as u8, CgaColor::White as u8); // Test masking
    }

    #[test]
    fn test_adapter_creation() {
        let adapter = SoftwareCgaAdapter::new();
        assert_eq!(adapter.fb_width(), 640);
        assert_eq!(adapter.fb_height(), 400);
        assert_eq!(adapter.name(), "Software CGA Adapter");
        assert!(!adapter.is_hardware_accelerated());
    }

    #[test]
    fn test_adapter_init() {
        let mut adapter = SoftwareCgaAdapter::new();
        adapter.init(640, 400);
        assert_eq!(adapter.fb_width(), 640);
        assert_eq!(adapter.fb_height(), 400);
    }

    #[test]
    fn test_render_empty_vram() {
        let adapter = SoftwareCgaAdapter::new();
        let vram = vec![0u8; 4000];
        let mut pixels = vec![0u32; 640 * 400];

        adapter.render(&vram, &mut pixels);

        // Should be all black (background)
        assert!(pixels.iter().all(|&p| p == 0xFF000000));
    }

    #[test]
    fn test_render_hello_world() {
        let adapter = SoftwareCgaAdapter::new();
        let mut vram = vec![0u8; 4000];

        // Write "Hello" at position 0
        let text = b"Hello";
        let attr = 0x0F; // White on black

        for (i, &ch) in text.iter().enumerate() {
            vram[i * 2] = ch;
            vram[i * 2 + 1] = attr;
        }

        let mut pixels = vec![0u32; 640 * 400];
        adapter.render(&vram, &mut pixels);

        // Check that not all pixels are black (some text was rendered)
        let non_black = pixels.iter().filter(|&&p| p != 0xFF000000).count();
        assert!(non_black > 0, "Expected some non-black pixels for text");
    }

    #[test]
    fn test_render_colored_text() {
        let adapter = SoftwareCgaAdapter::new();
        let mut vram = vec![0u8; 4000];

        // Write 'A' with light green on blue background
        vram[0] = b'A';
        vram[1] = 0x1A; // Light green (A) on blue (1) background

        let mut pixels = vec![0u32; 640 * 400];
        adapter.render(&vram, &mut pixels);

        // Check for blue background pixels
        let blue_pixels = pixels
            .iter()
            .filter(|&&p| p == CgaColor::Blue.to_rgb())
            .count();
        assert!(blue_pixels > 0, "Expected blue background pixels");
    }

    #[test]
    fn test_adapter_reset() {
        let mut adapter = SoftwareCgaAdapter::new();
        adapter.reset();
        let frame = adapter.get_frame();
        // All pixels should be black after reset
        assert!(frame.pixels.iter().all(|&p| p == 0xFF000000));
    }

    #[test]
    fn test_adapter_resize() {
        let mut adapter = SoftwareCgaAdapter::new();
        adapter.resize(320, 200);
        assert_eq!(adapter.fb_width(), 320);
        assert_eq!(adapter.fb_height(), 192); // Adjusted to char_height boundary (12 * 16)
    }

    #[test]
    fn test_get_frame_mut() {
        let mut adapter = SoftwareCgaAdapter::new();
        let frame = adapter.get_frame_mut();
        frame.pixels[0] = 0xFFFF0000; // Red pixel
        assert_eq!(adapter.get_frame().pixels[0], 0xFFFF0000);
    }
}
