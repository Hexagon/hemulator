//! PPU Renderer - Abstraction for SNES video rendering
//!
//! This module provides a renderer implementation for the SNES PPU,
//! following the common `emu_core::renderer::Renderer` pattern.
//!
//! # Design Philosophy
//!
//! The SnesPpuRenderer follows the common renderer pattern defined in `emu_core::renderer::Renderer`:
//! - **Software Renderer**: CPU-based tile/sprite rendering, maximum compatibility
//! - **Hardware Renderer** (future): GPU-accelerated rendering for performance
//!
//! # Architecture
//!
//! ```text
//! SnesSystem (state) -> SnesPpuRenderer trait -> {Software, Hardware} implementations
//!                            ↓
//!                 (follows emu_core::renderer::Renderer pattern)
//! ```
//!
//! The system maintains PPU state (registers, VRAM, CGRAM) and delegates
//! actual frame rendering to the renderer backend.

use emu_core::renderer::Renderer;
use emu_core::types::Frame;

use crate::ppu::Ppu;

/// Trait for SNES PPU rendering backends
///
/// This trait follows the common `Renderer` pattern with SNES-specific extensions.
/// It abstracts the actual rendering work, allowing different implementations
/// (software vs. hardware-accelerated) to be used interchangeably.
///
/// # Core Methods (from Renderer pattern)
/// - `get_frame()`: Get the current framebuffer
/// - `clear()`: Clear the framebuffer with a color
/// - `reset()`: Reset renderer to initial state
/// - `resize()`: Resize the renderer
/// - `name()`: Get renderer name
///
/// # SNES-Specific Methods
/// - Frame-based rendering (256x224 resolution)
/// - PPU state rendering (background layers, sprites)
pub trait SnesPpuRenderer: Renderer {
    /// Render a complete frame using PPU state
    ///
    /// # Arguments
    /// * `ppu` - PPU chip state (registers, VRAM, CGRAM)
    fn render_frame(&mut self, ppu: &Ppu);
}

/// Software SNES PPU renderer (CPU-based tile/sprite rendering)
pub struct SoftwareSnesPpuRenderer {
    framebuffer: Frame,
}

impl SoftwareSnesPpuRenderer {
    /// Create a new software SNES PPU renderer
    pub fn new() -> Self {
        Self {
            framebuffer: Frame::new(256, 224),
        }
    }
}

impl Default for SoftwareSnesPpuRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl Renderer for SoftwareSnesPpuRenderer {
    fn get_frame(&self) -> &Frame {
        &self.framebuffer
    }

    fn clear(&mut self, color: u32) {
        for pixel in &mut self.framebuffer.pixels {
            *pixel = color;
        }
    }

    fn reset(&mut self) {
        self.clear(0xFF000000); // Black
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.framebuffer = Frame::new(width, height);
    }

    fn name(&self) -> &str {
        "SNES Software Renderer"
    }
}

impl SnesPpuRenderer for SoftwareSnesPpuRenderer {
    fn render_frame(&mut self, ppu: &Ppu) {
        // Delegate to the PPU's existing render logic and copy the result
        let rendered = ppu.render_frame();
        self.framebuffer = rendered;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_software_renderer_creation() {
        let renderer = SoftwareSnesPpuRenderer::new();
        assert_eq!(renderer.get_frame().width, 256);
        assert_eq!(renderer.get_frame().height, 224);
        assert_eq!(renderer.name(), "SNES Software Renderer");
        assert!(!renderer.is_hardware_accelerated());
    }

    #[test]
    fn test_software_renderer_clear() {
        let mut renderer = SoftwareSnesPpuRenderer::new();
        renderer.clear(0xFFFF0000); // Red

        let frame = renderer.get_frame();
        assert!(frame.pixels.iter().all(|&p| p == 0xFFFF0000));
    }

    #[test]
    fn test_software_renderer_reset() {
        let mut renderer = SoftwareSnesPpuRenderer::new();
        renderer.clear(0xFFFF0000); // Red
        renderer.reset(); // Should clear to black

        let frame = renderer.get_frame();
        assert!(frame.pixels.iter().all(|&p| p == 0xFF000000));
    }

    #[test]
    fn test_software_renderer_resize() {
        let mut renderer = SoftwareSnesPpuRenderer::new();
        renderer.resize(512, 448);

        let frame = renderer.get_frame();
        assert_eq!(frame.width, 512);
        assert_eq!(frame.height, 448);
    }
}
