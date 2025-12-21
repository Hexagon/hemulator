//! RDP Renderer Trait - Abstraction for different rendering backends
//!
//! This module provides a trait for pluggable RDP rendering backends,
//! allowing the N64 emulator to use either software rasterization or
//! hardware-accelerated (OpenGL) rendering.
//!
//! # Design Philosophy
//!
//! The separation follows the same pattern as the frontend's `VideoProcessor` trait:
//! - **Software Renderer**: CPU-based rasterization, maximum accuracy and compatibility
//! - **OpenGL Renderer** (future): GPU-accelerated rasterization for performance
//!
//! # Architecture
//!
//! ```text
//! RDP (state management) -> RdpRenderer trait -> {Software, OpenGL} implementations
//! ```
//!
//! The RDP struct maintains state (registers, TMEM, scissor, etc.) and delegates
//! actual drawing operations to the renderer backend.

use emu_core::types::Frame;

/// Scissor box for clipping (shared between RDP and renderers)
#[derive(Debug, Clone, Copy)]
pub struct ScissorBox {
    pub x_min: u32,
    pub y_min: u32,
    pub x_max: u32,
    pub y_max: u32,
}

/// Trait for RDP rendering backends
///
/// This trait abstracts the actual rasterization work, allowing different
/// implementations (software vs. hardware-accelerated) to be used interchangeably.
pub trait RdpRenderer: Send {
    /// Initialize the renderer with the given dimensions
    #[allow(dead_code)]
    fn init(&mut self, width: u32, height: u32);

    /// Get the current framebuffer
    fn get_frame(&self) -> &Frame;

    /// Get mutable access to the framebuffer
    #[allow(dead_code)]
    fn get_frame_mut(&mut self) -> &mut Frame;

    /// Clear the framebuffer with a color
    fn clear(&mut self, color: u32);

    /// Fill a rectangle with a color
    fn fill_rect(
        &mut self,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        color: u32,
        scissor: &ScissorBox,
    );

    /// Set a single pixel
    fn set_pixel(&mut self, x: u32, y: u32, color: u32);

    /// Draw a flat-shaded triangle (no Z-buffer)
    #[allow(clippy::too_many_arguments)]
    fn draw_triangle(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        color: u32,
        scissor: &ScissorBox,
    );

    /// Draw a flat-shaded triangle with Z-buffer
    #[allow(clippy::too_many_arguments)]
    fn draw_triangle_zbuffer(
        &mut self,
        x0: i32,
        y0: i32,
        z0: u16,
        x1: i32,
        y1: i32,
        z1: u16,
        x2: i32,
        y2: i32,
        z2: u16,
        color: u32,
        scissor: &ScissorBox,
    );

    /// Draw a Gouraud-shaded triangle (per-vertex color interpolation)
    #[allow(clippy::too_many_arguments)]
    fn draw_triangle_shaded(
        &mut self,
        x0: i32,
        y0: i32,
        c0: u32,
        x1: i32,
        y1: i32,
        c1: u32,
        x2: i32,
        y2: i32,
        c2: u32,
        scissor: &ScissorBox,
    );

    /// Draw a Gouraud-shaded triangle with Z-buffer
    #[allow(clippy::too_many_arguments)]
    fn draw_triangle_shaded_zbuffer(
        &mut self,
        x0: i32,
        y0: i32,
        z0: u16,
        c0: u32,
        x1: i32,
        y1: i32,
        z1: u16,
        c1: u32,
        x2: i32,
        y2: i32,
        z2: u16,
        c2: u32,
        scissor: &ScissorBox,
    );

    /// Clear the Z-buffer to maximum depth (far plane)
    fn clear_zbuffer(&mut self);

    /// Enable or disable Z-buffer testing
    fn set_zbuffer_enabled(&mut self, enabled: bool);

    /// Resize the renderer to new dimensions
    #[allow(dead_code)]
    fn resize(&mut self, width: u32, height: u32);

    /// Reset the renderer to initial state
    fn reset(&mut self);

    /// Get the name of this renderer (for debugging/UI)
    #[allow(dead_code)]
    fn name(&self) -> &str;

    /// Check if this renderer is hardware-accelerated
    #[allow(dead_code)]
    fn is_hardware_accelerated(&self) -> bool {
        false
    }
}
