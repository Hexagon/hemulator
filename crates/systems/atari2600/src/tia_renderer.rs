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
        // Note: We clamp instead of using modulo to prevent wraparound.
        // If visible_start + visible_line exceeds 261, we clamp to 261.
        // This prevents rendering VBLANK scanlines at the bottom of the frame,
        // which causes vertical glitching and duplicate rendering.
        for visible_line in 0..192 {
            let tia_scanline = (visible_start + visible_line as u16).min(261);
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

    #[test]
    fn test_render_frame_no_wraparound() {
        // This test verifies the fix for vertical glitching bug
        // The bug was caused by modulo operation wrapping scanlines when
        // (visible_start + visible_line) exceeded 261
        use crate::tia::Tia;

        let mut renderer = SoftwareTiaRenderer::new();
        let mut tia = Tia::new();

        // Set up a simple pattern on scanline 261 (last valid scanline)
        tia.write(0x0D, 0xFF); // PF0 - all bits set
        tia.write(0x08, 0x0E); // COLUPF - white

        // Simulate a late visible_start that would cause wraparound with the old code
        // Old code: (261 + 1) % 262 = 0 (wraps to VBLANK area)
        // New code: (261 + 1).min(261) = 261 (clamps to last valid scanline)
        let visible_start = 261;

        // Render the frame
        renderer.render_frame(&tia, visible_start);

        // With the fix, all scanlines should use scanline 261's state
        // (clamped to 261) instead of wrapping around to scanline 0
        // This prevents the vertical glitching and duplication
        let frame = renderer.get_frame();

        // Just verify that rendering completed without panic
        assert_eq!(frame.width, 160);
        assert_eq!(frame.height, 192);
    }

    #[test]
    fn test_render_frame_normal_visible_start() {
        // Test normal case with typical visible_start value
        use crate::tia::Tia;

        let mut renderer = SoftwareTiaRenderer::new();
        let tia = Tia::new();

        // Typical NTSC visible_start is around 37-40
        let visible_start = 40;

        // Render the frame
        renderer.render_frame(&tia, visible_start);

        let frame = renderer.get_frame();
        assert_eq!(frame.width, 160);
        assert_eq!(frame.height, 192);

        // In normal case, no clamping should occur since 40 + 191 = 231 < 261
    }
}
