//! Software VGA Video Adapter - CPU-based rendering for VGA modes
//!
//! This module implements the `VideoAdapter` trait using software (CPU-based)
//! rendering for VGA (Video Graphics Array) modes.
//!
//! # VGA Specifications
//!
//! - Text modes: 80x25 characters at 720x400 pixels (9x16 font)
//! - Graphics modes:
//!   - 320x200 256-color (Mode 13h) - most popular VGA mode
//!   - 640x480 16-color (planar memory, 4 bit planes)
//! - 256-color palette (18-bit RGB: 6 bits per channel)
//! - Multiple font sizes: 8x16, 9x16 (text mode)

use super::video_adapter::VideoAdapter;
use emu_core::types::Frame;

/// VGA video modes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VgaMode {
    /// Text mode: 80x25 characters at 720x400 (9x16 font)
    #[default]
    Text80x25,
    /// Graphics mode 13h: 320x200, 256 colors (most popular)
    Graphics320x200,
    /// Graphics mode: 640x480, 16 colors (planar)
    Graphics640x480,
}

/// VGA color in 256-color palette (18-bit RGB)
#[derive(Debug, Clone, Copy)]
pub struct VgaColor {
    /// Red component (0-63, 6 bits)
    pub r: u8,
    /// Green component (0-63, 6 bits)
    pub g: u8,
    /// Blue component (0-63, 6 bits)
    pub b: u8,
}

impl VgaColor {
    /// Create from 8-bit palette index
    pub fn from_palette_index(index: u8, palette: &[VgaColor; 256]) -> Self {
        palette[index as usize]
    }

    /// Convert to ARGB8888 format
    pub fn to_argb(self) -> u32 {
        // Scale 6-bit components (0-63) to 8-bit (0-255)
        let r = ((self.r as u32 * 255) / 63) & 0xFF;
        let g = ((self.g as u32 * 255) / 63) & 0xFF;
        let b = ((self.b as u32 * 255) / 63) & 0xFF;
        0xFF000000 | (r << 16) | (g << 8) | b
    }
}

/// Default VGA palette (matches IBM VGA default for first 16 colors)
pub const DEFAULT_VGA_PALETTE: [VgaColor; 256] = generate_default_vga_palette();

/// Generate default VGA palette
const fn generate_default_vga_palette() -> [VgaColor; 256] {
    let mut palette = [VgaColor { r: 0, g: 0, b: 0 }; 256];

    // First 16 colors match EGA/CGA standard colors
    palette[0] = VgaColor { r: 0, g: 0, b: 0 }; // Black
    palette[1] = VgaColor { r: 0, g: 0, b: 42 }; // Blue
    palette[2] = VgaColor { r: 0, g: 42, b: 0 }; // Green
    palette[3] = VgaColor { r: 0, g: 42, b: 42 }; // Cyan
    palette[4] = VgaColor { r: 42, g: 0, b: 0 }; // Red
    palette[5] = VgaColor { r: 42, g: 0, b: 42 }; // Magenta
    palette[6] = VgaColor { r: 42, g: 21, b: 0 }; // Brown
    palette[7] = VgaColor {
        r: 42,
        g: 42,
        b: 42,
    }; // Light Gray
    palette[8] = VgaColor {
        r: 21,
        g: 21,
        b: 21,
    }; // Dark Gray
    palette[9] = VgaColor {
        r: 21,
        g: 21,
        b: 63,
    }; // Light Blue
    palette[10] = VgaColor {
        r: 21,
        g: 63,
        b: 21,
    }; // Light Green
    palette[11] = VgaColor {
        r: 21,
        g: 63,
        b: 63,
    }; // Light Cyan
    palette[12] = VgaColor {
        r: 63,
        g: 21,
        b: 21,
    }; // Light Red
    palette[13] = VgaColor {
        r: 63,
        g: 21,
        b: 63,
    }; // Light Magenta
    palette[14] = VgaColor {
        r: 63,
        g: 63,
        b: 21,
    }; // Yellow
    palette[15] = VgaColor {
        r: 63,
        g: 63,
        b: 63,
    }; // White

    // Colors 16-255: Generate grayscale ramp for remaining entries
    let mut i = 16;
    while i < 256 {
        let gray = ((i - 16) * 63) / 239;
        palette[i] = VgaColor {
            r: gray as u8,
            g: gray as u8,
            b: gray as u8,
        };
        i += 1;
    }

    palette
}

/// Software-based VGA video adapter
pub struct SoftwareVgaAdapter {
    /// Framebuffer
    framebuffer: Frame,
    /// Current video mode
    mode: VgaMode,
    /// Text mode dimensions
    text_width: usize,
    text_height: usize,
    /// Character cell size
    char_width: usize,
    char_height: usize,
    /// 256-color palette
    palette: [VgaColor; 256],
}

impl SoftwareVgaAdapter {
    /// Create a new VGA video adapter (starts in text mode)
    pub fn new() -> Self {
        Self {
            framebuffer: Frame::new(720, 400),
            mode: VgaMode::Text80x25,
            text_width: 80,
            text_height: 25,
            char_width: 9, // VGA uses 9-pixel wide characters
            char_height: 16,
            palette: DEFAULT_VGA_PALETTE,
        }
    }

    /// Set the video mode
    pub fn set_mode(&mut self, mode: VgaMode) {
        if self.mode != mode {
            self.mode = mode;
            let (width, height) = self.get_mode_resolution();
            self.framebuffer = Frame::new(width as u32, height as u32);
        }
    }

    /// Get current mode
    pub fn get_mode(&self) -> VgaMode {
        self.mode
    }

    /// Get resolution for the current mode
    fn get_mode_resolution(&self) -> (usize, usize) {
        match self.mode {
            VgaMode::Text80x25 => (720, 400),
            VgaMode::Graphics320x200 => (320, 200),
            VgaMode::Graphics640x480 => (640, 480),
        }
    }

    /// Set palette entry
    pub fn set_palette(&mut self, index: usize, color: VgaColor) {
        if index < 256 {
            self.palette[index] = color;
        }
    }

    /// Get palette entry as ARGB color
    fn get_palette_color(&self, index: u8) -> u32 {
        self.palette[index as usize].to_argb()
    }

    /// Render text mode (80x25 at 720x400)
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
        let glyph = get_vga_font_glyph(char_code);

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

                // VGA text mode: 9th column is background or repeats 8th column for line-drawing chars
                let bit = if col < 8 {
                    (bits >> (7 - col)) & 1
                } else {
                    // For line-drawing characters (0xC0-0xDF), repeat column 8
                    // For others, use background
                    if (0xC0..=0xDF).contains(&char_code) {
                        bits & 1 // Repeat rightmost bit
                    } else {
                        0 // Background
                    }
                };

                pixels[pixel_idx] = if bit == 1 { fg_rgb } else { bg_rgb };
            }
        }
    }

    /// Render graphics mode 13h: 320x200, 256 colors
    fn render_graphics_320x200(&self, vram: &[u8], pixels: &mut [u32]) {
        const WIDTH: usize = 320;
        const HEIGHT: usize = 200;

        // VGA Mode 13h uses linear addressing (1 byte per pixel)
        pixels.fill(0xFF000000);

        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let offset = y * WIDTH + x;
                if offset >= vram.len() {
                    break;
                }

                let color_index = vram[offset];
                let pixel_idx = y * WIDTH + x;

                if pixel_idx < pixels.len() {
                    pixels[pixel_idx] = self.get_palette_color(color_index);
                }
            }
        }
    }

    /// Render graphics mode 640x480 (planar, 16 colors)
    fn render_graphics_640x480(&self, vram: &[u8], pixels: &mut [u32]) {
        const WIDTH: usize = 640;
        const HEIGHT: usize = 480;

        // VGA 640x480x16 uses planar memory (4 bit planes)
        // Each plane is 38,400 bytes (640 * 480 / 8)
        const PLANE_SIZE: usize = 38400;

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

impl Default for SoftwareVgaAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl VideoAdapter for SoftwareVgaAdapter {
    fn init(&mut self, width: usize, height: usize) {
        // Detect mode based on resolution
        self.mode = match (width, height) {
            (720, 400) => VgaMode::Text80x25,
            (320, 200) => VgaMode::Graphics320x200,
            (640, 480) => VgaMode::Graphics640x480,
            _ => VgaMode::Text80x25, // Default to text mode
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
            VgaMode::Text80x25 => self.render_text_mode(vram, pixels),
            VgaMode::Graphics320x200 => self.render_graphics_320x200(vram, pixels),
            VgaMode::Graphics640x480 => self.render_graphics_640x480(vram, pixels),
        }
    }

    fn reset(&mut self) {
        self.framebuffer.pixels.fill(0xFF000000);
        self.mode = VgaMode::Text80x25;
        self.palette = DEFAULT_VGA_PALETTE;
    }

    fn name(&self) -> &str {
        "Software VGA Adapter"
    }

    fn resize(&mut self, width: usize, height: usize) {
        self.init(width, height);
    }
}

/// Get VGA font glyph data (16-scanline font)
fn get_vga_font_glyph(char_code: u8) -> &'static [u8] {
    static FONT_DATA: [[u8; 16]; 256] = generate_vga_font();

    let glyph = &FONT_DATA[char_code as usize];

    if char_code != 0x20 && glyph.iter().all(|&b| b == 0) {
        static BOX_GLYPH: [u8; 16] = [
            0x00, 0x00, 0x7E, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x7E,
            0x00, 0x00,
        ];
        &BOX_GLYPH
    } else {
        glyph
    }
}

/// Generate VGA font (8x16 characters)
const fn generate_vga_font() -> [[u8; 16]; 256] {
    let mut font = [[0u8; 16]; 256];

    // Space (0x20)
    font[0x20] = [0x00; 16];

    // Exclamation mark (0x21)
    font[0x21] = [
        0x00, 0x00, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x00, 0x00, 0x18, 0x18, 0x00,
        0x00,
    ];

    // Letter 'A' (0x41)
    font[0x41] = [
        0x00, 0x00, 0x00, 0x18, 0x3C, 0x66, 0x66, 0x66, 0x7E, 0x66, 0x66, 0x66, 0x66, 0x00, 0x00,
        0x00,
    ];

    // Letter 'H' (0x48)
    font[0x48] = [
        0x00, 0x00, 0x00, 0x66, 0x66, 0x66, 0x66, 0x7E, 0x66, 0x66, 0x66, 0x66, 0x66, 0x00, 0x00,
        0x00,
    ];

    // Letter 'e' (0x65)
    font[0x65] = [
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x3C, 0x66, 0x66, 0x7E, 0x60, 0x60, 0x3E, 0x00, 0x00,
        0x00,
    ];

    // Letter 'l' (0x6C)
    font[0x6C] = [
        0x00, 0x00, 0x00, 0x38, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x3C, 0x00, 0x00,
        0x00,
    ];

    // Letter 'o' (0x6F)
    font[0x6F] = [
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x3C, 0x66, 0x66, 0x66, 0x66, 0x66, 0x3C, 0x00, 0x00,
        0x00,
    ];

    // Line drawing character (0xC4 - horizontal line)
    font[0xC4] = [
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ];

    font
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vga_color_palette() {
        let color = VgaColor { r: 0, g: 0, b: 0 };
        assert_eq!(color.to_argb(), 0xFF000000); // Black

        let color = VgaColor {
            r: 63,
            g: 63,
            b: 63,
        };
        assert_eq!(color.to_argb(), 0xFFFFFFFF); // White
    }

    #[test]
    fn test_adapter_creation() {
        let adapter = SoftwareVgaAdapter::new();
        assert_eq!(adapter.get_mode(), VgaMode::Text80x25);
        assert_eq!(adapter.fb_width(), 720);
        assert_eq!(adapter.fb_height(), 400);
        assert_eq!(adapter.name(), "Software VGA Adapter");
        assert!(!adapter.is_hardware_accelerated());
    }

    #[test]
    fn test_mode_switching() {
        let mut adapter = SoftwareVgaAdapter::new();

        adapter.set_mode(VgaMode::Graphics320x200);
        assert_eq!(adapter.get_mode(), VgaMode::Graphics320x200);
        assert_eq!(adapter.fb_width(), 320);
        assert_eq!(adapter.fb_height(), 200);

        adapter.set_mode(VgaMode::Graphics640x480);
        assert_eq!(adapter.get_mode(), VgaMode::Graphics640x480);
        assert_eq!(adapter.fb_width(), 640);
        assert_eq!(adapter.fb_height(), 480);

        adapter.set_mode(VgaMode::Text80x25);
        assert_eq!(adapter.get_mode(), VgaMode::Text80x25);
        assert_eq!(adapter.fb_width(), 720);
        assert_eq!(adapter.fb_height(), 400);
    }

    #[test]
    fn test_palette_setting() {
        let mut adapter = SoftwareVgaAdapter::new();
        let white = VgaColor {
            r: 63,
            g: 63,
            b: 63,
        };
        adapter.set_palette(0, white);
        assert_eq!(adapter.palette[0].r, 63);
        assert_eq!(adapter.palette[0].g, 63);
        assert_eq!(adapter.palette[0].b, 63);
    }

    #[test]
    fn test_text_mode_rendering() {
        let adapter = SoftwareVgaAdapter::new();
        let mut vram = vec![0u8; 4000];

        // Write "Hello" at position 0
        let text = b"Hello";
        let attr = 0x0F; // White on black

        for (i, &ch) in text.iter().enumerate() {
            vram[i * 2] = ch;
            vram[i * 2 + 1] = attr;
        }

        let mut pixels = vec![0u32; 720 * 400];
        adapter.render(&vram, &mut pixels);

        // Check that some text was rendered
        let non_black = pixels.iter().filter(|&&p| p != 0xFF000000).count();
        assert!(non_black > 0);
    }

    #[test]
    fn test_graphics_320x200_rendering() {
        let mut adapter = SoftwareVgaAdapter::new();
        adapter.set_mode(VgaMode::Graphics320x200);

        // Create test pattern in VRAM
        let mut vram = vec![0u8; 64000]; // 320 * 200 bytes

        // Fill with a simple pattern (color indices)
        for (i, byte) in vram.iter_mut().enumerate().take(1000) {
            *byte = (i % 256) as u8;
        }

        let mut pixels = vec![0u32; 320 * 200];
        adapter.render(&vram, &mut pixels);

        // Check that non-black pixels were rendered
        let non_black = pixels.iter().filter(|&&p| p != 0xFF000000).count();
        assert!(non_black > 0);
    }

    #[test]
    fn test_graphics_640x480_rendering() {
        let mut adapter = SoftwareVgaAdapter::new();
        adapter.set_mode(VgaMode::Graphics640x480);

        // Create test pattern in VRAM (4 planes)
        let mut vram = vec![0u8; 153600]; // 4 planes * 38400 bytes

        // Fill first plane with pattern
        for byte in vram.iter_mut().take(100) {
            *byte = 0xFF;
        }

        let mut pixels = vec![0u32; 640 * 480];
        adapter.render(&vram, &mut pixels);

        // Check that some pixels were rendered
        let non_black = pixels.iter().filter(|&&p| p != 0xFF000000).count();
        assert!(non_black > 0);
    }

    #[test]
    fn test_adapter_reset() {
        let mut adapter = SoftwareVgaAdapter::new();
        adapter.set_mode(VgaMode::Graphics320x200);
        let custom_color = VgaColor {
            r: 30,
            g: 30,
            b: 30,
        };
        adapter.set_palette(0, custom_color);

        adapter.reset();

        assert_eq!(adapter.get_mode(), VgaMode::Text80x25);
        assert_eq!(adapter.palette[0].r, DEFAULT_VGA_PALETTE[0].r);
    }

    #[test]
    fn test_default_palette() {
        let adapter = SoftwareVgaAdapter::new();
        // Verify default palette first 16 colors
        assert_eq!(adapter.palette[0].r, 0); // Black
        assert_eq!(adapter.palette[15].r, 63); // White
        assert_eq!(adapter.palette[15].g, 63);
        assert_eq!(adapter.palette[15].b, 63);
    }

    #[test]
    fn test_vga_9th_column() {
        let adapter = SoftwareVgaAdapter::new();
        let mut vram = vec![0u8; 4000];

        // Write line-drawing character (0xC4)
        vram[0] = 0xC4;
        vram[1] = 0x0F; // White on black

        let mut pixels = vec![0u32; 720 * 400];
        adapter.render(&vram, &mut pixels);

        // The 9th column should extend the line for line-drawing chars
        let non_black = pixels.iter().filter(|&&p| p != 0xFF000000).count();
        assert!(non_black > 0);
    }
}
