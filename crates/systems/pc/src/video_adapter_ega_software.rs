//! Software EGA Video Adapter - CPU-based rendering for EGA modes
//!
//! This module implements the `VideoAdapter` trait using software (CPU-based)
//! rendering for EGA (Enhanced Graphics Adapter) modes.
//!
//! # EGA Specifications
//!
//! - Text modes: 80x25 characters at 640x350 pixels
//! - Graphics modes:
//!   - 640x350 16-color (high resolution)
//!   - 320x200 16-color (medium resolution, CGA compatible)
//! - 64-color palette (6-bit: 2 bits each for R, G, B)
//! - 16 colors can be selected from the 64-color palette at a time
//! - Planar memory organization (4 bit planes)

use super::font;
use super::video_adapter::VideoAdapter;
use emu_core::types::Frame;

/// EGA video modes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EgaMode {
    /// Text mode: 80x25 characters at 640x350
    #[default]
    Text80x25,
    /// Graphics mode: 640x350, 16 colors
    Graphics640x350,
    /// Graphics mode: 320x200, 16 colors (CGA compatible)
    Graphics320x200,
}

/// EGA color in 64-color palette (6-bit RGB)
#[derive(Debug, Clone, Copy)]
pub struct EgaColor {
    /// Red component (0-3, 2 bits)
    pub r: u8,
    /// Green component (0-3, 2 bits)
    pub g: u8,
    /// Blue component (0-3, 2 bits)
    pub b: u8,
}

impl EgaColor {
    /// Create from 6-bit palette index
    pub fn from_palette_index(index: u8) -> Self {
        let index = index & 0x3F; // 6-bit palette
        Self {
            r: (index >> 4) & 0x03,
            g: (index >> 2) & 0x03,
            b: index & 0x03,
        }
    }

    /// Convert to ARGB8888 format
    pub fn to_argb(self) -> u32 {
        // Scale 2-bit components (0-3) to 8-bit (0-255)
        let r = ((self.r * 85) as u32) & 0xFF; // 85 = 255/3
        let g = ((self.g * 85) as u32) & 0xFF;
        let b = ((self.b * 85) as u32) & 0xFF;
        0xFF000000 | (r << 16) | (g << 8) | b
    }
}

/// Default EGA palette (matches IBM EGA default)
pub const DEFAULT_EGA_PALETTE: [u8; 16] = [
    0x00, // Black
    0x01, // Blue
    0x02, // Green
    0x03, // Cyan
    0x04, // Red
    0x05, // Magenta
    0x14, // Brown (dark yellow)
    0x07, // Light Gray
    0x38, // Dark Gray
    0x39, // Light Blue
    0x3A, // Light Green
    0x3B, // Light Cyan
    0x3C, // Light Red
    0x3D, // Light Magenta
    0x3E, // Yellow
    0x3F, // White
];

/// Software-based EGA video adapter
pub struct SoftwareEgaAdapter {
    /// Framebuffer
    framebuffer: Frame,
    /// Current video mode
    mode: EgaMode,
    /// Text mode dimensions
    text_width: usize,
    text_height: usize,
    /// Character cell size
    char_width: usize,
    char_height: usize,
    /// Active palette (16 colors selected from 64)
    palette: [u8; 16],
}

impl SoftwareEgaAdapter {
    /// Create a new EGA video adapter (starts in text mode)
    pub fn new() -> Self {
        Self {
            framebuffer: Frame::new(640, 350),
            mode: EgaMode::Text80x25,
            text_width: 80,
            text_height: 25,
            char_width: 8,
            char_height: 14, // EGA uses 14-scanline characters
            palette: DEFAULT_EGA_PALETTE,
        }
    }

    /// Set the video mode
    pub fn set_mode(&mut self, mode: EgaMode) {
        if self.mode != mode {
            self.mode = mode;
            let (width, height) = self.get_mode_resolution();
            self.framebuffer = Frame::new(width as u32, height as u32);
        }
    }

    /// Get current mode
    pub fn get_mode(&self) -> EgaMode {
        self.mode
    }

    /// Get resolution for the current mode
    fn get_mode_resolution(&self) -> (usize, usize) {
        match self.mode {
            EgaMode::Text80x25 => (640, 350),
            EgaMode::Graphics640x350 => (640, 350),
            EgaMode::Graphics320x200 => (320, 200),
        }
    }

    /// Set palette entry
    pub fn set_palette(&mut self, index: usize, color: u8) {
        if index < 16 {
            self.palette[index] = color & 0x3F;
        }
    }

    /// Get palette entry as ARGB color
    fn get_palette_color(&self, index: u8) -> u32 {
        let palette_index = self.palette[(index & 0x0F) as usize];
        EgaColor::from_palette_index(palette_index).to_argb()
    }

    /// Render text mode (80x25 at 640x350)
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

                let fg_color = attr & 0x0F;
                let bg_color = (attr >> 4) & 0x0F;

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

    /// Render a single character
    fn render_char(
        &self,
        char_code: u8,
        fg_color: u8,
        bg_color: u8,
        x: usize,
        y: usize,
        pixels: &mut [u32],
    ) {
        let fg_rgb = self.get_palette_color(fg_color);
        let bg_rgb = self.get_palette_color(bg_color);
        let glyph = font::get_font_8x14(char_code);

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

    /// Render graphics mode 640x350 (planar, 16 colors)
    fn render_graphics_640x350(&self, vram: &[u8], pixels: &mut [u32]) {
        const WIDTH: usize = 640;
        const HEIGHT: usize = 350;

        // EGA uses planar memory (4 bit planes)
        // Each plane is 28,000 bytes (640 * 350 / 8)
        const PLANE_SIZE: usize = 28000;

        pixels.fill(0xFF000000);

        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let byte_offset = y * (WIDTH / 8) + (x / 8);
                let bit_offset = 7 - (x % 8);

                // Read from all 4 planes to get 4-bit color
                let mut color = 0u8;
                for plane in 0..4 {
                    let plane_offset = plane * PLANE_SIZE + byte_offset;
                    if plane_offset < vram.len() {
                        let bit = (vram[plane_offset] >> bit_offset) & 1;
                        color |= bit << plane;
                    }
                }

                let pixel_idx = y * WIDTH + x;
                if pixel_idx < pixels.len() {
                    pixels[pixel_idx] = self.get_palette_color(color);
                }
            }
        }
    }

    /// Render graphics mode 320x200 (planar, 16 colors)
    fn render_graphics_320x200(&self, vram: &[u8], pixels: &mut [u32]) {
        const WIDTH: usize = 320;
        const HEIGHT: usize = 200;

        // Each plane is 8,000 bytes (320 * 200 / 8)
        const PLANE_SIZE: usize = 8000;

        pixels.fill(0xFF000000);

        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let byte_offset = y * (WIDTH / 8) + (x / 8);
                let bit_offset = 7 - (x % 8);

                // Read from all 4 planes to get 4-bit color
                let mut color = 0u8;
                for plane in 0..4 {
                    let plane_offset = plane * PLANE_SIZE + byte_offset;
                    if plane_offset < vram.len() {
                        let bit = (vram[plane_offset] >> bit_offset) & 1;
                        color |= bit << plane;
                    }
                }

                let pixel_idx = y * WIDTH + x;
                if pixel_idx < pixels.len() {
                    pixels[pixel_idx] = self.get_palette_color(color);
                }
            }
        }
    }
}

impl Default for SoftwareEgaAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl VideoAdapter for SoftwareEgaAdapter {
    fn init(&mut self, width: usize, height: usize) {
        // Detect mode based on resolution
        self.mode = match (width, height) {
            (640, 350) => EgaMode::Graphics640x350,
            (320, 200) => EgaMode::Graphics320x200,
            _ => EgaMode::Text80x25, // Default to text mode
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
            EgaMode::Text80x25 => self.render_text_mode(vram, pixels),
            EgaMode::Graphics640x350 => self.render_graphics_640x350(vram, pixels),
            EgaMode::Graphics320x200 => self.render_graphics_320x200(vram, pixels),
        }
    }

    fn reset(&mut self) {
        self.framebuffer.pixels.fill(0xFF000000);
        self.mode = EgaMode::Text80x25;
        self.palette = DEFAULT_EGA_PALETTE;
    }

    fn name(&self) -> &str {
        "Software EGA Adapter"
    }

    fn resize(&mut self, width: usize, height: usize) {
        self.init(width, height);
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ega_color_palette() {
        let color = EgaColor::from_palette_index(0x00);
        assert_eq!(color.to_argb(), 0xFF000000); // Black

        let color = EgaColor::from_palette_index(0x3F);
        assert_eq!(color.to_argb(), 0xFFFFFFFF); // White
    }

    #[test]
    fn test_adapter_creation() {
        let adapter = SoftwareEgaAdapter::new();
        assert_eq!(adapter.get_mode(), EgaMode::Text80x25);
        assert_eq!(adapter.fb_width(), 640);
        assert_eq!(adapter.fb_height(), 350);
        assert_eq!(adapter.name(), "Software EGA Adapter");
        assert!(!adapter.is_hardware_accelerated());
    }

    #[test]
    fn test_mode_switching() {
        let mut adapter = SoftwareEgaAdapter::new();

        adapter.set_mode(EgaMode::Graphics640x350);
        assert_eq!(adapter.get_mode(), EgaMode::Graphics640x350);
        assert_eq!(adapter.fb_width(), 640);
        assert_eq!(adapter.fb_height(), 350);

        adapter.set_mode(EgaMode::Graphics320x200);
        assert_eq!(adapter.get_mode(), EgaMode::Graphics320x200);
        assert_eq!(adapter.fb_width(), 320);
        assert_eq!(adapter.fb_height(), 200);
    }

    #[test]
    fn test_palette_setting() {
        let mut adapter = SoftwareEgaAdapter::new();
        adapter.set_palette(0, 0x3F); // Set color 0 to white
        assert_eq!(adapter.palette[0], 0x3F);
    }

    #[test]
    fn test_text_mode_rendering() {
        let adapter = SoftwareEgaAdapter::new();
        let mut vram = vec![0u8; 4000];

        // Write "Hello" at position 0
        let text = b"Hello";
        let attr = 0x0F; // White on black

        for (i, &ch) in text.iter().enumerate() {
            vram[i * 2] = ch;
            vram[i * 2 + 1] = attr;
        }

        let mut pixels = vec![0u32; 640 * 350];
        adapter.render(&vram, &mut pixels);

        // Check that some text was rendered
        let non_black = pixels.iter().filter(|&&p| p != 0xFF000000).count();
        assert!(non_black > 0);
    }

    #[test]
    fn test_graphics_640x350_rendering() {
        let mut adapter = SoftwareEgaAdapter::new();
        adapter.set_mode(EgaMode::Graphics640x350);

        // Create test pattern in VRAM (4 planes)
        let mut vram = vec![0u8; 112000]; // 4 planes * 28000 bytes

        // Fill first plane with pattern
        for byte in vram.iter_mut().take(100) {
            *byte = 0xFF;
        }

        let mut pixels = vec![0u32; 640 * 350];
        adapter.render(&vram, &mut pixels);

        // Check that some pixels were rendered
        let non_black = pixels.iter().filter(|&&p| p != 0xFF000000).count();
        assert!(non_black > 0);
    }

    #[test]
    fn test_graphics_320x200_rendering() {
        let mut adapter = SoftwareEgaAdapter::new();
        adapter.set_mode(EgaMode::Graphics320x200);

        // Create test pattern in VRAM (4 planes)
        let mut vram = vec![0u8; 32000]; // 4 planes * 8000 bytes

        // Fill with pattern
        for byte in vram.iter_mut().take(100) {
            *byte = 0xAA;
        }

        let mut pixels = vec![0u32; 320 * 200];
        adapter.render(&vram, &mut pixels);

        // Check that pixels were rendered
        let non_black = pixels.iter().filter(|&&p| p != 0xFF000000).count();
        assert!(non_black > 0);
    }

    #[test]
    fn test_adapter_reset() {
        let mut adapter = SoftwareEgaAdapter::new();
        adapter.set_mode(EgaMode::Graphics640x350);
        adapter.set_palette(0, 0x20);

        adapter.reset();

        assert_eq!(adapter.get_mode(), EgaMode::Text80x25);
        assert_eq!(adapter.palette[0], DEFAULT_EGA_PALETTE[0]);
    }

    #[test]
    fn test_default_palette() {
        let adapter = SoftwareEgaAdapter::new();
        // Verify default palette matches EGA standard
        assert_eq!(adapter.palette[0], 0x00); // Black
        assert_eq!(adapter.palette[15], 0x3F); // White
    }
}
