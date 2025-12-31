//! PPU Renderer - Abstraction for NES video rendering
//!
//! This module provides a renderer implementation for the NES PPU,
//! following the common `emu_core::renderer::Renderer` pattern.
//!
//! # Design Philosophy
//!
//! The NesPpuRenderer follows the common renderer pattern defined in `emu_core::renderer::Renderer`:
//! - **Software Renderer**: CPU-based tile/sprite rendering, maximum compatibility
//! - **Hardware Renderer** (future): GPU-accelerated rendering for performance
//!
//! # Architecture
//!
//! ```text
//! NesSystem (state) -> NesPpuRenderer trait -> {Software, Hardware} implementations
//!                           ↓
//!                (follows emu_core::renderer::Renderer pattern)
//! ```
//!
//! The system maintains PPU state (registers, VRAM, OAM) and delegates
//! actual frame/scanline rendering to the renderer backend.

use emu_core::renderer::Renderer;
use emu_core::types::Frame;

use crate::ppu::Ppu;

/// Trait for NES PPU rendering backends
///
/// This trait follows the common `Renderer` pattern with NES-specific extensions.
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
/// # NES-Specific Methods
/// - Scanline-based rendering (256x240 resolution)
/// - Incremental rendering for mapper compatibility (MMC3 IRQ, MMC2/MMC4 CHR switching)
pub trait NesPpuRenderer: Renderer + std::fmt::Debug {
    /// Get mutable access to the framebuffer for direct scanline rendering
    fn get_frame_mut(&mut self) -> &mut Frame;

    /// Take ownership of the current frame and replace it with a new empty frame.
    /// This avoids cloning the frame buffer (61,440 pixels) every frame.
    fn take_frame(&mut self) -> Frame;

    /// Render a single scanline using PPU state
    ///
    /// # Arguments
    /// * `ppu` - PPU chip state (registers, VRAM, OAM)
    /// * `scanline` - Scanline number (0-239)
    fn render_scanline(&mut self, ppu: &mut Ppu, scanline: u32);

    /// Render a complete frame using PPU state
    ///
    /// # Arguments
    /// * `ppu` - PPU chip state
    #[allow(dead_code)]
    fn render_frame(&mut self, ppu: &Ppu);
}

/// Software NES PPU renderer (CPU-based tile/sprite rendering)
#[derive(Debug)]
pub struct SoftwareNesPpuRenderer {
    framebuffer: Frame,
}

impl SoftwareNesPpuRenderer {
    /// Create a new software NES PPU renderer
    pub fn new() -> Self {
        Self {
            framebuffer: Frame::new(256, 240),
        }
    }
}

impl Default for SoftwareNesPpuRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl Renderer for SoftwareNesPpuRenderer {
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
        "NES Software Renderer"
    }
}

impl NesPpuRenderer for SoftwareNesPpuRenderer {
    fn get_frame_mut(&mut self) -> &mut Frame {
        &mut self.framebuffer
    }

    fn take_frame(&mut self) -> Frame {
        // Take ownership of the current frame and replace with a new empty frame
        // This avoids cloning 61,440 pixels (245KB) every frame (60 times/second)
        std::mem::replace(&mut self.framebuffer, Frame::new(256, 240))
    }

    fn render_scanline(&mut self, ppu: &mut Ppu, scanline: u32) {
        // Delegate to the PPU's existing scanline render logic
        ppu.render_scanline(scanline, &mut self.framebuffer);
    }

    fn render_frame(&mut self, ppu: &Ppu) {
        // For NES, we use scanline-based rendering during step_frame,
        // so this method renders all scanlines at once.
        // Note: This bypasses incremental rendering and may not handle
        // mapper CHR switching correctly. Use scanline rendering instead.
        for scanline in 0..240 {
            // We need &mut Ppu to render scanlines
            // Since we only have &Ppu here, we can't actually render
            // This method is primarily for testing or future use
            let _ = (ppu, scanline); // Silence unused warnings
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_software_renderer_creation() {
        let renderer = SoftwareNesPpuRenderer::new();
        assert_eq!(renderer.get_frame().width, 256);
        assert_eq!(renderer.get_frame().height, 240);
        assert_eq!(renderer.name(), "NES Software Renderer");
        assert!(!renderer.is_hardware_accelerated());
    }

    #[test]
    fn test_software_renderer_clear() {
        let mut renderer = SoftwareNesPpuRenderer::new();
        renderer.clear(0xFFFF0000); // Red

        let frame = renderer.get_frame();
        assert!(frame.pixels.iter().all(|&p| p == 0xFFFF0000));
    }

    #[test]
    fn test_software_renderer_reset() {
        let mut renderer = SoftwareNesPpuRenderer::new();
        renderer.clear(0xFFFF0000); // Red
        renderer.reset(); // Should clear to black

        let frame = renderer.get_frame();
        assert!(frame.pixels.iter().all(|&p| p == 0xFF000000));
    }

    #[test]
    fn test_software_renderer_resize() {
        let mut renderer = SoftwareNesPpuRenderer::new();
        renderer.resize(512, 480);

        let frame = renderer.get_frame();
        assert_eq!(frame.width, 512);
        assert_eq!(frame.height, 480);
    }
}
