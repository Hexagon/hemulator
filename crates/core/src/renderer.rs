//! Common renderer trait for all systems
//!
//! This module provides a unified abstraction for graphics rendering across all emulated systems.
//! The trait supports both software (CPU-based) and hardware-accelerated (GPU-based) rendering.
//!
//! # Design Philosophy
//!
//! All systems with graphics capabilities (NES, Game Boy, SNES, N64, PC, Atari 2600, etc.) follow
//! the same architectural pattern:
//!
//! ```text
//! System (state management) -> Renderer trait -> {Software, Hardware} implementations
//! ```
//!
//! This provides:
//! - **Consistency**: All systems use the same rendering interface
//! - **Flexibility**: Easy to add new rendering backends (OpenGL, Vulkan, Metal, etc.)
//! - **Testability**: Renderers can be tested independently
//! - **Performance**: GPU acceleration available where beneficial
//!
//! # Usage Examples
//!
//! ## Simple 2D Renderer (NES, Game Boy, Atari 2600)
//!
//! ```rust,ignore
//! use emu_core::renderer::Renderer;
//! use emu_core::types::Frame;
//!
//! struct NesPpuRenderer {
//!     frame: Frame,
//! }
//!
//! impl Renderer for NesPpuRenderer {
//!     fn get_frame(&self) -> &Frame {
//!         &self.frame
//!     }
//!
//!     fn clear(&mut self, color: u32) {
//!         for pixel in &mut self.frame.pixels {
//!             *pixel = color;
//!         }
//!     }
//!
//!     fn reset(&mut self) {
//!         self.clear(0xFF000000);
//!     }
//!
//!     fn name(&self) -> &str {
//!         "NES Software Renderer"
//!     }
//! }
//! ```
//!
//! ## Advanced 3D Renderer (N64)
//!
//! The trait also supports advanced 3D rendering operations for systems like the N64.
//! See the N64 RdpRenderer implementation for an example.

use crate::types::Frame;

/// Common renderer trait for all emulated graphics systems
///
/// This trait provides a unified interface for rendering operations across different
/// emulated systems. Each system implements this trait according to its specific
/// graphics capabilities (2D sprites/tiles, 3D polygons, text mode, etc.).
pub trait Renderer: Send {
    /// Get the current framebuffer (read-only)
    fn get_frame(&self) -> &Frame;

    /// Get mutable access to the framebuffer (for direct pixel manipulation)
    fn get_frame_mut(&mut self) -> &mut Frame {
        // Default implementation - systems can override if they manage frame differently
        // This is a workaround since we can't return &mut from &self
        unimplemented!("get_frame_mut requires override")
    }

    /// Clear the framebuffer with a solid color
    ///
    /// # Arguments
    /// * `color` - ARGB8888 color value (0xAARRGGBB)
    fn clear(&mut self, color: u32);

    /// Reset the renderer to its initial state
    ///
    /// This should clear the framebuffer and reset any internal state
    /// (Z-buffer, palettes, etc.) to their default values.
    fn reset(&mut self);

    /// Get the name of this renderer (for debugging/UI)
    ///
    /// Examples: "NES Software Renderer", "N64 OpenGL Renderer"
    fn name(&self) -> &str;

    /// Check if this renderer uses hardware acceleration
    ///
    /// Returns `true` for GPU-accelerated renderers (OpenGL, Vulkan, etc.)
    /// and `false` for CPU-based software renderers.
    fn is_hardware_accelerated(&self) -> bool {
        false
    }

    /// Resize the renderer to new dimensions
    ///
    /// This is called when the output resolution changes. The renderer should
    /// recreate its framebuffer and any resolution-dependent resources.
    ///
    /// # Arguments
    /// * `width` - New width in pixels
    /// * `height` - New height in pixels
    fn resize(&mut self, width: u32, height: u32);
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockRenderer {
        frame: Frame,
    }

    impl MockRenderer {
        fn new(width: u32, height: u32) -> Self {
            Self {
                frame: Frame::new(width, height),
            }
        }
    }

    impl Renderer for MockRenderer {
        fn get_frame(&self) -> &Frame {
            &self.frame
        }

        fn clear(&mut self, color: u32) {
            for pixel in &mut self.frame.pixels {
                *pixel = color;
            }
        }

        fn reset(&mut self) {
            self.clear(0xFF000000);
        }

        fn resize(&mut self, width: u32, height: u32) {
            self.frame = Frame::new(width, height);
        }

        fn name(&self) -> &str {
            "Mock Renderer"
        }
    }

    #[test]
    fn test_renderer_creation() {
        let renderer = MockRenderer::new(256, 240);
        assert_eq!(renderer.get_frame().width, 256);
        assert_eq!(renderer.get_frame().height, 240);
        assert_eq!(renderer.name(), "Mock Renderer");
        assert!(!renderer.is_hardware_accelerated());
    }

    #[test]
    fn test_renderer_clear() {
        let mut renderer = MockRenderer::new(256, 240);
        renderer.clear(0xFFFF0000); // Red

        let frame = renderer.get_frame();
        assert!(frame.pixels.iter().all(|&p| p == 0xFFFF0000));
    }

    #[test]
    fn test_renderer_reset() {
        let mut renderer = MockRenderer::new(256, 240);
        renderer.clear(0xFFFF0000); // Red
        renderer.reset(); // Should clear to black

        let frame = renderer.get_frame();
        assert!(frame.pixels.iter().all(|&p| p == 0xFF000000));
    }

    #[test]
    fn test_renderer_resize() {
        let mut renderer = MockRenderer::new(256, 240);
        renderer.resize(512, 480);

        let frame = renderer.get_frame();
        assert_eq!(frame.width, 512);
        assert_eq!(frame.height, 480);
        assert_eq!(frame.pixels.len(), 512 * 480);
    }
}
