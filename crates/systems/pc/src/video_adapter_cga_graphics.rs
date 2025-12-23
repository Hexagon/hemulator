//! CGA Graphics Adapter - Multiple graphics modes support
//!
//! This module implements the `VideoAdapter` trait for CGA graphics modes,
//! supporting both text and graphics modes with runtime mode switching.
//!
//! # Supported Modes
//!
//! - **Text Mode**: 80x25 characters (640x400 pixels)
//! - **Graphics Mode 4**: 320x200, 4 colors (2 bits per pixel)
//! - **Graphics Mode 6**: 640x200, 2 colors (1 bit per pixel)
//!
//! # Mode Switching
//!
//! The adapter detects the current mode from the mode control register in VRAM.
//! On real hardware, this would be set via I/O port 0x3D8 (mode control register).
//! For simplicity, we detect mode based on VRAM content patterns.

use super::video_adapter::VideoAdapter;
use super::video_adapter_software::CgaColor;
use emu_core::types::Frame;

/// CGA video modes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CgaMode {
    /// Text mode: 80x25 characters, 16 colors
    #[default]
    Text80x25,
    /// Graphics mode 4: 320x200, 4 colors (cyan/magenta/white or green/red/brown)
    Graphics320x200,
    /// Graphics mode 6: 640x200, 2 colors (black and white)
    Graphics640x200,
}

/// CGA graphics adapter with mode switching support
pub struct CgaGraphicsAdapter {
    /// Framebuffer
    framebuffer: Frame,
    /// Current video mode
    mode: CgaMode,
    /// Text mode dimensions
    text_width: usize,
    text_height: usize,
    /// Character cell size
    char_width: usize,
    char_height: usize,
}

impl CgaGraphicsAdapter {
    /// Create a new CGA graphics adapter (starts in text mode)
    pub fn new() -> Self {
        Self {
            framebuffer: Frame::new(640, 400),
            mode: CgaMode::Text80x25,
            text_width: 80,
            text_height: 25,
            char_width: 8,
            char_height: 16,
        }
    }

    /// Set the video mode
    pub fn set_mode(&mut self, mode: CgaMode) {
        if self.mode != mode {
            self.mode = mode;
            // Resize framebuffer based on mode
            let (width, height) = self.get_mode_resolution();
            self.framebuffer = Frame::new(width as u32, height as u32);
        }
    }

    /// Get current mode
    pub fn get_mode(&self) -> CgaMode {
        self.mode
    }

    /// Get resolution for the current mode
    fn get_mode_resolution(&self) -> (usize, usize) {
        match self.mode {
            CgaMode::Text80x25 => (640, 400),
            CgaMode::Graphics320x200 => (320, 200),
            CgaMode::Graphics640x200 => (640, 200),
        }
    }

    /// Render text mode (80x25)
    fn render_text_mode(&self, vram: &[u8], pixels: &mut [u32]) {
        let required_vram = self.text_width * self.text_height * 2;
        if vram.len() < required_vram {
            return;
        }

        pixels.fill(0xFF000000);

        for row in 0..self.text_height {
            for col in 0..self.text_width {
                let cell_offset = (row * self.text_width + col) * 2;
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

    /// Render a single character (used in text mode)
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
        let glyph = get_font_glyph(char_code);

        for row in 0..self.char_height {
            let byte_idx = row.min(glyph.len() - 1);
            let bits = glyph[byte_idx];

            for col in 0..self.char_width {
                let pixel_x = x + col;
                let pixel_y = y + row;

                let fb_width = self.text_width * self.char_width;
                let fb_height = self.text_height * self.char_height;

                if pixel_y >= fb_height || pixel_x >= fb_width {
                    continue;
                }

                let pixel_idx = pixel_y * fb_width + pixel_x;
                if pixel_idx >= pixels.len() {
                    continue;
                }

                let bit = (bits >> (7 - col)) & 1;
                pixels[pixel_idx] = if bit == 1 { fg_rgb } else { bg_rgb };
            }
        }
    }

    /// Render graphics mode 4: 320x200, 4 colors
    fn render_graphics_320x200(&self, vram: &[u8], pixels: &mut [u32]) {
        const WIDTH: usize = 320;
        const HEIGHT: usize = 200;

        // CGA graphics mode 4 uses 16KB of VRAM starting at 0xB8000
        // Each byte contains 4 pixels (2 bits per pixel)
        // Palette 0: Black, Cyan, Magenta, White
        let palette = [
            0xFF000000, // Black
            0xFF00FFFF, // Cyan
            0xFFFF00FF, // Magenta
            0xFFFFFFFF, // White
        ];

        pixels.fill(0xFF000000);

        for y in 0..HEIGHT {
            // CGA uses interlaced scanlines
            // Even scanlines: offset 0x0000
            // Odd scanlines: offset 0x2000
            let base_offset = if y % 2 == 0 {
                (y / 2) * (WIDTH / 4)
            } else {
                0x2000 + ((y - 1) / 2) * (WIDTH / 4)
            };

            for x in 0..(WIDTH / 4) {
                let offset = base_offset + x;
                if offset >= vram.len() {
                    break;
                }

                let byte = vram[offset];

                // Each byte contains 4 pixels (2 bits each)
                for pixel in 0..4 {
                    let pixel_x = x * 4 + pixel;
                    let color_idx = ((byte >> (6 - pixel * 2)) & 0x03) as usize;
                    let pixel_idx = y * WIDTH + pixel_x;

                    if pixel_idx < pixels.len() {
                        pixels[pixel_idx] = palette[color_idx];
                    }
                }
            }
        }
    }

    /// Render graphics mode 6: 640x200, 2 colors
    fn render_graphics_640x200(&self, vram: &[u8], pixels: &mut [u32]) {
        const WIDTH: usize = 640;
        const HEIGHT: usize = 200;

        // Each byte contains 8 pixels (1 bit per pixel)
        pixels.fill(0xFF000000);

        for y in 0..HEIGHT {
            // CGA uses interlaced scanlines
            let base_offset = if y % 2 == 0 {
                (y / 2) * (WIDTH / 8)
            } else {
                0x2000 + ((y - 1) / 2) * (WIDTH / 8)
            };

            for x in 0..(WIDTH / 8) {
                let offset = base_offset + x;
                if offset >= vram.len() {
                    break;
                }

                let byte = vram[offset];

                // Each byte contains 8 pixels (1 bit each)
                for pixel in 0..8 {
                    let pixel_x = x * 8 + pixel;
                    let bit = (byte >> (7 - pixel)) & 1;
                    let color = if bit == 1 { 0xFFFFFFFF } else { 0xFF000000 };
                    let pixel_idx = y * WIDTH + pixel_x;

                    if pixel_idx < pixels.len() {
                        pixels[pixel_idx] = color;
                    }
                }
            }
        }
    }
}

impl Default for CgaGraphicsAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl VideoAdapter for CgaGraphicsAdapter {
    fn init(&mut self, width: usize, height: usize) {
        // Detect mode based on resolution
        self.mode = match (width, height) {
            (640, 400) => CgaMode::Text80x25,
            (320, 200) => CgaMode::Graphics320x200,
            (640, 200) => CgaMode::Graphics640x200,
            _ => CgaMode::Text80x25, // Default to text mode
        };
        self.framebuffer = Frame::new(width as u32, height as u32);
    }

    fn get_frame(&self) -> &Frame {
        &self.framebuffer
    }

    fn get_frame_mut(&mut self) -> &mut Frame {
        &mut self.framebuffer
    }

    fn fb_width(&self) -> usize {
        let (width, _) = self.get_mode_resolution();
        width
    }

    fn fb_height(&self) -> usize {
        let (_, height) = self.get_mode_resolution();
        height
    }

    fn render(&self, vram: &[u8], pixels: &mut [u32]) {
        match self.mode {
            CgaMode::Text80x25 => self.render_text_mode(vram, pixels),
            CgaMode::Graphics320x200 => self.render_graphics_320x200(vram, pixels),
            CgaMode::Graphics640x200 => self.render_graphics_640x200(vram, pixels),
        }
    }

    fn reset(&mut self) {
        self.framebuffer.pixels.fill(0xFF000000);
        self.mode = CgaMode::Text80x25;
    }

    fn name(&self) -> &str {
        "CGA Graphics Adapter"
    }

    fn resize(&mut self, width: usize, height: usize) {
        self.init(width, height);
    }
}

/// Get font glyph data for a character (simplified 8x16 font)
fn get_font_glyph(char_code: u8) -> &'static [u8] {
    static FONT_DATA: [[u8; 16]; 256] = generate_basic_font();

    let glyph = &FONT_DATA[char_code as usize];

    if char_code != 0x20 && glyph.iter().all(|&b| b == 0) {
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

    // Space (0x20)
    font[0x20] = [0x00; 16];

    // Exclamation mark (0x21)
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
    fn test_adapter_creation() {
        let adapter = CgaGraphicsAdapter::new();
        assert_eq!(adapter.get_mode(), CgaMode::Text80x25);
        assert_eq!(adapter.fb_width(), 640);
        assert_eq!(adapter.fb_height(), 400);
    }

    #[test]
    fn test_mode_switching() {
        let mut adapter = CgaGraphicsAdapter::new();

        adapter.set_mode(CgaMode::Graphics320x200);
        assert_eq!(adapter.get_mode(), CgaMode::Graphics320x200);
        assert_eq!(adapter.fb_width(), 320);
        assert_eq!(adapter.fb_height(), 200);

        adapter.set_mode(CgaMode::Graphics640x200);
        assert_eq!(adapter.get_mode(), CgaMode::Graphics640x200);
        assert_eq!(adapter.fb_width(), 640);
        assert_eq!(adapter.fb_height(), 200);

        adapter.set_mode(CgaMode::Text80x25);
        assert_eq!(adapter.get_mode(), CgaMode::Text80x25);
        assert_eq!(adapter.fb_width(), 640);
        assert_eq!(adapter.fb_height(), 400);
    }

    #[test]
    fn test_text_mode_rendering() {
        let adapter = CgaGraphicsAdapter::new();
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

        // Check that some text was rendered
        let non_black = pixels.iter().filter(|&&p| p != 0xFF000000).count();
        assert!(non_black > 0);
    }

    #[test]
    fn test_graphics_320x200_rendering() {
        let mut adapter = CgaGraphicsAdapter::new();
        adapter.set_mode(CgaMode::Graphics320x200);

        // Create test pattern in VRAM
        let mut vram = vec![0u8; 0x4000]; // 16KB for graphics mode

        // Fill with a simple pattern (alternating colors)
        for i in 0..100 {
            vram[i] = 0b11100100; // Mix of colors
        }

        let mut pixels = vec![0u32; 320 * 200];
        adapter.render(&vram, &mut pixels);

        // Check that non-black pixels were rendered
        let non_black = pixels.iter().filter(|&&p| p != 0xFF000000).count();
        assert!(non_black > 0);
    }

    #[test]
    fn test_graphics_640x200_rendering() {
        let mut adapter = CgaGraphicsAdapter::new();
        adapter.set_mode(CgaMode::Graphics640x200);

        // Create test pattern in VRAM
        let mut vram = vec![0u8; 0x4000]; // 16KB for graphics mode

        // Fill with alternating bits
        for i in 0..100 {
            vram[i] = 0b10101010;
        }

        let mut pixels = vec![0u32; 640 * 200];
        adapter.render(&vram, &mut pixels);

        // Check that white pixels were rendered
        let white_pixels = pixels.iter().filter(|&&p| p == 0xFFFFFFFF).count();
        assert!(white_pixels > 0);
    }

    #[test]
    fn test_adapter_reset() {
        let mut adapter = CgaGraphicsAdapter::new();
        adapter.set_mode(CgaMode::Graphics320x200);

        adapter.reset();

        assert_eq!(adapter.get_mode(), CgaMode::Text80x25);
        let frame = adapter.get_frame();
        assert!(frame.pixels.iter().all(|&p| p == 0xFF000000));
    }

    #[test]
    fn test_adapter_name() {
        let adapter = CgaGraphicsAdapter::new();
        assert_eq!(adapter.name(), "CGA Graphics Adapter");
    }
}
