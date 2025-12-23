//! TIA Renderer - Abstraction for Atari 2600 video rendering
//!
//! This module provides a renderer implementation for the Atari 2600's TIA chip,
//! following the common `emu_core::renderer::Renderer` pattern.
//!
//! # Design Philosophy
//!
//! The TiaRenderer follows the common renderer pattern defined in `emu_core::renderer::Renderer`:
//! - **Software Renderer**: CPU-based scanline rendering, maximum compatibility
//! - **Hardware Renderer** (future): GPU-accelerated rendering for performance
//!
//! # Architecture
//!
//! ```text
//! Atari2600System (state) -> TiaRenderer trait -> {Software, Hardware} implementations
//!                                 ↓
//!                      (follows emu_core::renderer::Renderer pattern)
//! ```
//!
//! The system maintains TIA state (registers, colors, graphics objects) and delegates
//! actual scanline rendering to the renderer backend.

use emu_core::renderer::Renderer;
use emu_core::types::Frame;

use crate::tia::Tia;

/// Trait for TIA rendering backends
///
/// This trait follows the common `Renderer` pattern with Atari 2600-specific extensions.
/// It abstracts the actual scanline rendering work, allowing different implementations
/// (software vs. hardware-accelerated) to be used interchangeably.
///
/// # Core Methods (from Renderer pattern)
/// - `get_frame()`: Get the current framebuffer
/// - `clear()`: Clear the framebuffer with a color
/// - `reset()`: Reset renderer to initial state
/// - `resize()`: Resize the renderer
/// - `name()`: Get renderer name
///
/// # Atari 2600-Specific Methods
/// - Scanline-based rendering (160x192 resolution)
/// - TIA state rendering (playfield, players, missiles, ball)
pub trait TiaRenderer: Renderer {
    /// Render a single scanline using TIA state
    ///
    /// # Arguments
    /// * `tia` - TIA chip state (registers, colors, graphics)
    /// * `visible_line` - Visible scanline number (0-191)
    /// * `tia_scanline` - Actual TIA scanline number (0-261)
    fn render_scanline(&mut self, tia: &Tia, visible_line: usize, tia_scanline: u16);

    /// Render a complete frame using TIA state
    ///
    /// # Arguments
    /// * `tia` - TIA chip state
    /// * `visible_start` - First visible scanline in TIA coordinates
    fn render_frame(&mut self, tia: &Tia, visible_start: u16);
}

/// Software TIA renderer (CPU-based scanline rendering)
pub struct SoftwareTiaRenderer {
    framebuffer: Frame,
}

impl SoftwareTiaRenderer {
    /// Create a new software TIA renderer
    pub fn new() -> Self {
        Self {
            framebuffer: Frame::new(160, 192),
        }
    }
}

impl Default for SoftwareTiaRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl Renderer for SoftwareTiaRenderer {
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
        "Atari 2600 Software Renderer"
    }
}

impl TiaRenderer for SoftwareTiaRenderer {
    fn render_scanline(&mut self, tia: &Tia, visible_line: usize, tia_scanline: u16) {
        tia.render_scanline(&mut self.framebuffer.pixels, visible_line, tia_scanline);
    }

    fn render_frame(&mut self, tia: &Tia, visible_start: u16) {
        // Render 192 visible scanlines
        for visible_line in 0..192 {
            let tia_scanline = (visible_start + visible_line as u16) % 262;
            self.render_scanline(tia, visible_line, tia_scanline);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_software_renderer_creation() {
        let renderer = SoftwareTiaRenderer::new();
        assert_eq!(renderer.get_frame().width, 160);
        assert_eq!(renderer.get_frame().height, 192);
        assert_eq!(renderer.name(), "Atari 2600 Software Renderer");
        assert!(!renderer.is_hardware_accelerated());
    }

    #[test]
    fn test_software_renderer_clear() {
        let mut renderer = SoftwareTiaRenderer::new();
        renderer.clear(0xFFFF0000); // Red

        let frame = renderer.get_frame();
        assert!(frame.pixels.iter().all(|&p| p == 0xFFFF0000));
    }

    #[test]
    fn test_software_renderer_reset() {
        let mut renderer = SoftwareTiaRenderer::new();
        renderer.clear(0xFFFF0000); // Red
        renderer.reset(); // Should clear to black

        let frame = renderer.get_frame();
        assert!(frame.pixels.iter().all(|&p| p == 0xFF000000));
    }

    #[test]
    fn test_software_renderer_resize() {
        let mut renderer = SoftwareTiaRenderer::new();
        renderer.resize(320, 384);

        let frame = renderer.get_frame();
        assert_eq!(frame.width, 320);
        assert_eq!(frame.height, 384);
    }
}
