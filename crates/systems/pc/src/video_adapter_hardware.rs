//! Hardware-accelerated CGA Video Adapter (Example/Stub)
//!
//! This is an example stub showing how to implement a hardware-accelerated
//! video adapter using OpenGL or Vulkan. This demonstrates the extensibility
//! of the modular video adapter system.
//!
//! To implement a real hardware adapter:
//! 1. Add feature flag to Cargo.toml (e.g., `opengl` or `vulkan`)
//! 2. Add GPU dependencies (glow, glutin, etc.)
//! 3. Implement texture uploads and shader-based rendering
//! 4. Handle GL context creation and management
//!
//! # Example Usage
//!
//! ```rust,ignore
//! use emu_pc::HardwareCgaAdapter;
//!
//! let adapter = HardwareCgaAdapter::new();
//! // Use with PcSystem via Box<dyn VideoAdapter>
//! ```

use super::video_adapter::VideoAdapter;
use emu_core::types::Frame;

/// Hardware-accelerated CGA video adapter (stub)
///
/// This is a placeholder demonstrating the pattern. A real implementation would:
/// - Create OpenGL/Vulkan context
/// - Upload VRAM to GPU texture
/// - Render characters via GPU shaders
/// - Use hardware acceleration for scaling and filtering
#[allow(dead_code)] // This is an example stub
pub struct HardwareCgaAdapter {
    /// Framebuffer (rendered on GPU, copied back for Frame)
    framebuffer: Frame,
    /// Text mode width in characters
    width: usize,
    /// Text mode height in characters
    height: usize,
    /// Character width in pixels
    char_width: usize,
    /// Character height in pixels
    char_height: usize,
    // In a real implementation, you would add:
    // - GPU context handle
    // - Texture IDs for font atlas and framebuffer
    // - Shader program handles
    // - Vertex buffer objects
}

impl HardwareCgaAdapter {
    /// Create a new hardware-accelerated CGA adapter
    ///
    /// In a real implementation, this would:
    /// - Initialize GPU context (OpenGL, Vulkan, etc.)
    /// - Compile shaders for text rendering
    /// - Create font texture atlas from glyph data
    /// - Set up render targets
    #[allow(dead_code)]
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
}

impl VideoAdapter for HardwareCgaAdapter {
    fn init(&mut self, width: usize, height: usize) {
        let fb_width = width;
        let fb_height = height;
        self.framebuffer = Frame::new(fb_width as u32, fb_height as u32);
        self.width = fb_width / self.char_width;
        self.height = fb_height / self.char_height;

        // Real implementation would:
        // - Resize GPU framebuffer
        // - Update viewport
        // - Recreate render targets if needed
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

    fn render(&self, _vram: &[u8], _pixels: &mut [u32]) {
        // Real implementation would:
        // 1. Upload VRAM to GPU texture (or update existing texture)
        // 2. Bind font atlas texture
        // 3. For each character in VRAM:
        //    - Extract char code and attribute
        //    - Draw textured quad with correct glyph from atlas
        //    - Apply foreground/background colors via shader uniforms
        // 4. Read back framebuffer to pixels array
        //
        // Benefits of GPU rendering:
        // - Parallel character rendering
        // - Hardware-accelerated scaling/filtering
        // - Shader effects (CRT simulation, scanlines, etc.)
        // - Can render to window directly without CPU copy

        // For now, this stub does nothing
        // A real implementation would fill pixels with GPU-rendered content
    }

    fn reset(&mut self) {
        self.framebuffer.pixels.fill(0xFF000000);

        // Real implementation would:
        // - Clear GPU framebuffer
        // - Reset shader state
        // - Clear texture cache
    }

    fn name(&self) -> &str {
        "Hardware CGA Adapter (OpenGL/Vulkan)"
    }

    fn is_hardware_accelerated(&self) -> bool {
        true // This would be a GPU-accelerated implementation
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
        let adapter = HardwareCgaAdapter::new();
        assert_eq!(adapter.fb_width(), 640);
        assert_eq!(adapter.fb_height(), 400);
        assert_eq!(adapter.name(), "Hardware CGA Adapter (OpenGL/Vulkan)");
        assert!(adapter.is_hardware_accelerated());
    }

    #[test]
    fn test_hardware_adapter_properties() {
        let adapter = HardwareCgaAdapter::new();
        assert!(adapter.is_hardware_accelerated());
        assert!(adapter.name().contains("Hardware"));
    }
}
