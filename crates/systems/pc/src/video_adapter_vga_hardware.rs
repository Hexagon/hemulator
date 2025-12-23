//! Hardware-accelerated VGA Video Adapter (OpenGL)
//!
//! This module implements the `VideoAdapter` trait using hardware-accelerated
//! (GPU-based) rendering for VGA modes via OpenGL.
//!
//! This is feature-gated behind the `opengl` feature flag and requires
//! OpenGL dependencies to be available.
//!
//! # Architecture
//!
//! Similar to the N64 OpenGL renderer, this adapter:
//! - Uses OpenGL for GPU-accelerated rendering
//! - Uploads VRAM textures to GPU
//! - Renders via shaders for performance
//! - Reads back framebuffer for Frame compatibility
//!
//! # Feature Flag
//!
//! Enable with: `--features opengl`

use super::video_adapter::VideoAdapter;
use super::video_adapter_vga_software::{VgaColor, VgaMode, DEFAULT_VGA_PALETTE};
use emu_core::types::Frame;

/// Hardware-accelerated VGA video adapter using OpenGL
///
/// This is a stub/example implementation. A real implementation would:
/// - Create OpenGL context
/// - Upload VRAM to GPU textures
/// - Use shaders for:
///   - Mode 13h: Direct palette lookup from linear buffer
///   - 640x480x16: Planar-to-packed conversion and palette lookup
/// - Render with hardware acceleration
/// - Use GPU for text rendering with font atlas
#[allow(dead_code)] // Feature-gated, may not be compiled
pub struct HardwareVgaAdapter {
    /// Framebuffer (rendered on GPU, copied back for Frame)
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
    // In a real OpenGL implementation, you would add:
    // - GL context handle (glow::Context)
    // - Texture IDs for VRAM and font atlas
    // - Shader program handles:
    //   - Mode 13h shader (linear addressing)
    //   - Planar shader (640x480x16)
    //   - Text mode shader (font atlas)
    // - Framebuffer objects (FBO)
    // - Vertex buffer objects (VBO)
    // - Palette texture (256 colors)
}

impl HardwareVgaAdapter {
    /// Create a new hardware-accelerated VGA adapter
    ///
    /// In a real implementation, this would:
    /// - Initialize OpenGL context
    /// - Compile shaders for:
    ///   - Mode 13h: Simple texture lookup with palette
    ///   - 640x480x16: Planar-to-packed conversion with palette
    ///   - Text mode: Font atlas rendering
    /// - Create textures and FBOs
    /// - Upload font atlas and palette
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            framebuffer: Frame::new(720, 400),
            mode: VgaMode::Text80x25,
            text_width: 80,
            text_height: 25,
            char_width: 9,
            char_height: 16,
            palette: DEFAULT_VGA_PALETTE,
        }
    }

    /// Set the video mode
    #[allow(dead_code)]
    pub fn set_mode(&mut self, mode: VgaMode) {
        if self.mode != mode {
            self.mode = mode;
            let (width, height) = self.get_mode_resolution();
            self.framebuffer = Frame::new(width as u32, height as u32);

            // Real implementation would:
            // - Resize GPU framebuffer
            // - Update viewport
            // - Switch shader programs
            // - Recreate render targets if needed
        }
    }

    /// Get current mode
    #[allow(dead_code)]
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
    #[allow(dead_code)]
    pub fn set_palette(&mut self, index: usize, color: VgaColor) {
        if index < 256 {
            self.palette[index] = color;

            // Real implementation would:
            // - Upload updated palette to GPU texture
            // - Trigger shader to use new palette values
        }
    }
}

impl Default for HardwareVgaAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl VideoAdapter for HardwareVgaAdapter {
    fn init(&mut self, width: usize, height: usize) {
        self.mode = match (width, height) {
            (720, 400) => VgaMode::Text80x25,
            (320, 200) => VgaMode::Graphics320x200,
            (640, 480) => VgaMode::Graphics640x480,
            _ => VgaMode::Text80x25,
        };
        self.framebuffer = Frame::new(width as u32, height as u32);

        // Real implementation would:
        // - Initialize OpenGL context and resources
        // - Set up viewport
        // - Compile and link shader programs
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

    fn render(&self, _vram: &[u8], pixels: &mut [u32]) {
        // Stub: Just clear to black
        pixels.fill(0xFF000000);

        // Real implementation would:
        // - Upload VRAM to GPU texture
        // - For Mode 13h: Render with linear palette lookup shader
        // - For 640x480x16: Use planar shader to convert 4 planes to packed
        // - For text mode: Render characters using font atlas
        // - Read back framebuffer to pixels array
    }

    fn reset(&mut self) {
        self.framebuffer.pixels.fill(0xFF000000);
        self.mode = VgaMode::Text80x25;
        self.palette = DEFAULT_VGA_PALETTE;

        // Real implementation would:
        // - Clear GPU framebuffer
        // - Reset palette texture
    }

    fn name(&self) -> &str {
        "Hardware VGA Adapter (OpenGL)"
    }

    fn is_hardware_accelerated(&self) -> bool {
        true
    }

    fn resize(&mut self, width: usize, height: usize) {
        self.init(width, height);

        // Real implementation would:
        // - Resize OpenGL framebuffer
        // - Update projection matrices
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hardware_adapter_creation() {
        let adapter = HardwareVgaAdapter::new();
        assert_eq!(adapter.get_mode(), VgaMode::Text80x25);
        assert_eq!(adapter.fb_width(), 720);
        assert_eq!(adapter.fb_height(), 400);
        assert_eq!(adapter.name(), "Hardware VGA Adapter (OpenGL)");
        assert!(adapter.is_hardware_accelerated());
    }

    #[test]
    fn test_hardware_adapter_mode_switching() {
        let mut adapter = HardwareVgaAdapter::new();

        adapter.set_mode(VgaMode::Graphics320x200);
        assert_eq!(adapter.get_mode(), VgaMode::Graphics320x200);
        assert_eq!(adapter.fb_width(), 320);
        assert_eq!(adapter.fb_height(), 200);

        adapter.set_mode(VgaMode::Graphics640x480);
        assert_eq!(adapter.get_mode(), VgaMode::Graphics640x480);
        assert_eq!(adapter.fb_width(), 640);
        assert_eq!(adapter.fb_height(), 480);
    }

    #[test]
    fn test_hardware_adapter_palette() {
        let mut adapter = HardwareVgaAdapter::new();
        let custom = VgaColor {
            r: 30,
            g: 30,
            b: 30,
        };
        adapter.set_palette(0, custom);
        assert_eq!(adapter.palette[0].r, 30);
    }

    #[test]
    fn test_hardware_adapter_properties() {
        let adapter = HardwareVgaAdapter::new();
        assert!(adapter.is_hardware_accelerated());
        assert_eq!(adapter.name(), "Hardware VGA Adapter (OpenGL)");
    }
}
