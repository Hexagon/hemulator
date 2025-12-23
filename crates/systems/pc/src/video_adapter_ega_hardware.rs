//! Hardware-accelerated EGA Video Adapter (OpenGL)
//!
//! This module implements the `VideoAdapter` trait using hardware-accelerated
//! (GPU-based) rendering for EGA modes via OpenGL.
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
use super::video_adapter_ega_software::{EgaMode, DEFAULT_EGA_PALETTE};
use emu_core::types::Frame;

/// Hardware-accelerated EGA video adapter using OpenGL
///
/// This is a stub/example implementation. A real implementation would:
/// - Create OpenGL context
/// - Upload VRAM to GPU textures
/// - Use shaders for planar-to-packed conversion
/// - Render with palette lookup
/// - Use hardware acceleration for scaling
#[allow(dead_code)] // Feature-gated, may not be compiled
pub struct HardwareEgaAdapter {
    /// Framebuffer (rendered on GPU, copied back for Frame)
    framebuffer: Frame,
    /// Current video mode
    mode: EgaMode,
    /// Text mode dimensions
    text_width: usize,
    text_height: usize,
    /// Character cell size
    char_width: usize,
    char_height: usize,
    /// Active palette
    palette: [u8; 16],
    // In a real OpenGL implementation, you would add:
    // - GL context handle (glow::Context)
    // - Texture IDs for planes and font atlas
    // - Shader program handles
    // - Framebuffer objects (FBO)
    // - Vertex buffer objects (VBO)
}

impl HardwareEgaAdapter {
    /// Create a new hardware-accelerated EGA adapter
    ///
    /// In a real implementation, this would:
    /// - Initialize OpenGL context
    /// - Compile shaders for:
    ///   - Planar-to-packed conversion
    ///   - Palette lookup
    ///   - Text rendering with font atlas
    /// - Create textures and FBOs
    /// - Upload font atlas
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            framebuffer: Frame::new(640, 350),
            mode: EgaMode::Text80x25,
            text_width: 80,
            text_height: 25,
            char_width: 8,
            char_height: 14,
            palette: DEFAULT_EGA_PALETTE,
        }
    }

    /// Set the video mode
    #[allow(dead_code)]
    pub fn set_mode(&mut self, mode: EgaMode) {
        if self.mode != mode {
            self.mode = mode;
            let (width, height) = self.get_mode_resolution();
            self.framebuffer = Frame::new(width as u32, height as u32);

            // Real implementation would:
            // - Resize GPU framebuffer
            // - Update viewport
            // - Recreate render targets if needed
        }
    }

    /// Get current mode
    #[allow(dead_code)]
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
    #[allow(dead_code)]
    pub fn set_palette(&mut self, index: usize, color: u8) {
        if index < 16 {
            self.palette[index] = color & 0x3F;

            // Real implementation would:
            // - Update palette uniform in shader
            // - Or upload palette texture
        }
    }
}

impl VideoAdapter for HardwareEgaAdapter {
    fn init(&mut self, width: usize, height: usize) {
        self.mode = match (width, height) {
            (640, 350) => EgaMode::Graphics640x350,
            (320, 200) => EgaMode::Graphics320x200,
            _ => EgaMode::Text80x25,
        };
        self.framebuffer = Frame::new(width as u32, height as u32);

        // Real implementation would:
        // - Resize GPU framebuffer
        // - Update viewport
        // - Recreate render targets
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

    fn render(&self, _vram: &[u8], _pixels: &mut [u32]) {
        // Real OpenGL implementation would:
        //
        // **For Graphics Modes:**
        // 1. Upload 4 plane textures to GPU (R, G, B, A channels or separate textures)
        // 2. Bind planar-to-packed shader
        // 3. Render fullscreen quad that:
        //    - Samples all 4 planes
        //    - Combines into 4-bit color index
        //    - Looks up color in palette texture/uniform
        //    - Outputs final ARGB color
        // 4. Read back framebuffer to pixels array
        //
        // **For Text Mode:**
        // 1. Upload VRAM (char codes + attributes) as texture
        // 2. Bind text rendering shader
        // 3. For each character position:
        //    - Look up char code and attribute
        //    - Sample font atlas texture
        //    - Apply foreground/background colors from palette
        // 4. Read back framebuffer to pixels array
        //
        // **Benefits of GPU rendering:**
        // - Parallel processing of all pixels
        // - Hardware texture filtering
        // - Can render directly to window (no CPU copy)
        // - Shader effects (scanlines, CRT, etc.)
        // - Much faster than software rendering

        // For now, this stub does nothing
    }

    fn reset(&mut self) {
        self.framebuffer.pixels.fill(0xFF000000);
        self.mode = EgaMode::Text80x25;
        self.palette = DEFAULT_EGA_PALETTE;

        // Real implementation would:
        // - Clear GPU framebuffer
        // - Reset shader state
        // - Clear texture cache
    }

    fn name(&self) -> &str {
        "Hardware EGA Adapter (OpenGL)"
    }

    fn is_hardware_accelerated(&self) -> bool {
        true
    }

    fn resize(&mut self, width: usize, height: usize) {
        self.init(width, height);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hardware_adapter_creation() {
        let adapter = HardwareEgaAdapter::new();
        assert_eq!(adapter.fb_width(), 640);
        assert_eq!(adapter.fb_height(), 350);
        assert_eq!(adapter.name(), "Hardware EGA Adapter (OpenGL)");
        assert!(adapter.is_hardware_accelerated());
    }

    #[test]
    fn test_hardware_adapter_mode_switching() {
        let mut adapter = HardwareEgaAdapter::new();

        adapter.set_mode(EgaMode::Graphics320x200);
        assert_eq!(adapter.get_mode(), EgaMode::Graphics320x200);
        assert_eq!(adapter.fb_width(), 320);
        assert_eq!(adapter.fb_height(), 200);
    }

    #[test]
    fn test_hardware_adapter_palette() {
        let mut adapter = HardwareEgaAdapter::new();
        adapter.set_palette(0, 0x3F);
        assert_eq!(adapter.palette[0], 0x3F);
    }

    #[test]
    fn test_hardware_adapter_properties() {
        let adapter = HardwareEgaAdapter::new();
        assert!(adapter.is_hardware_accelerated());
        assert!(adapter.name().contains("Hardware"));
        assert!(adapter.name().contains("OpenGL"));
    }
}
