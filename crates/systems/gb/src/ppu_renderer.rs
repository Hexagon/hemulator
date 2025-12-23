//! PPU Renderer - Abstraction for Game Boy video rendering
//!
//! This module provides a renderer implementation for the Game Boy's PPU,
//! following the common `emu_core::renderer::Renderer` pattern.
//!
//! # Design Philosophy
//!
//! The PpuRenderer follows the common renderer pattern defined in `emu_core::renderer::Renderer`:
//! - **Software Renderer**: CPU-based tile/sprite rendering, maximum compatibility
//! - **Hardware Renderer** (future): GPU-accelerated rendering for performance
//!
//! # Architecture
//!
//! ```text
//! GbSystem (state) -> PpuRenderer trait -> {Software, Hardware} implementations
//!                          ↓
//!               (follows emu_core::renderer::Renderer pattern)
//! ```
//!
//! The system maintains PPU state (registers, VRAM, OAM) and delegates
//! actual frame rendering to the renderer backend.

use emu_core::renderer::Renderer;
use emu_core::types::Frame;

use crate::ppu::Ppu;

/// Trait for PPU rendering backends
///
/// This trait follows the common `Renderer` pattern with Game Boy-specific extensions.
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
/// # Game Boy-Specific Methods
/// - Frame-based rendering (160x144 resolution)
/// - PPU state rendering (background, window, sprites)
pub trait PpuRenderer: Renderer {
    /// Render a complete frame using PPU state
    ///
    /// # Arguments
    /// * `ppu` - PPU chip state (registers, VRAM, OAM)
    fn render_frame(&mut self, ppu: &Ppu);
}

/// Software PPU renderer (CPU-based tile/sprite rendering)
pub struct SoftwarePpuRenderer {
    framebuffer: Frame,
}

impl SoftwarePpuRenderer {
    /// Create a new software PPU renderer
    pub fn new() -> Self {
        Self {
            framebuffer: Frame::new(160, 144),
        }
    }
}

impl Default for SoftwarePpuRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl Renderer for SoftwarePpuRenderer {
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
        "Game Boy Software Renderer"
    }
}

impl PpuRenderer for SoftwarePpuRenderer {
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
        let renderer = SoftwarePpuRenderer::new();
        assert_eq!(renderer.get_frame().width, 160);
        assert_eq!(renderer.get_frame().height, 144);
        assert_eq!(renderer.name(), "Game Boy Software Renderer");
        assert!(!renderer.is_hardware_accelerated());
    }

    #[test]
    fn test_software_renderer_clear() {
        let mut renderer = SoftwarePpuRenderer::new();
        renderer.clear(0xFFFF0000); // Red

        let frame = renderer.get_frame();
        assert!(frame.pixels.iter().all(|&p| p == 0xFFFF0000));
    }

    #[test]
    fn test_software_renderer_reset() {
        let mut renderer = SoftwarePpuRenderer::new();
        renderer.clear(0xFFFF0000); // Red
        renderer.reset(); // Should clear to black

        let frame = renderer.get_frame();
        assert!(frame.pixels.iter().all(|&p| p == 0xFF000000));
    }

    #[test]
    fn test_software_renderer_resize() {
        let mut renderer = SoftwarePpuRenderer::new();
        renderer.resize(320, 288);

        let frame = renderer.get_frame();
        assert_eq!(frame.width, 320);
        assert_eq!(frame.height, 288);
    }
}
