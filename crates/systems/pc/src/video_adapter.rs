//! Video Adapter Trait - Abstraction for different graphics backends
//!
//! This module provides a trait for pluggable video rendering backends,
//! allowing the PC emulator to use either software rasterization or
//! hardware-accelerated rendering.
//!
//! # Design Philosophy
//!
//! The separation follows the same pattern as the N64's `RdpRenderer` trait:
//! - **Software Renderer**: CPU-based rendering, maximum compatibility
//! - **Hardware Renderer** (future): GPU-accelerated rendering for performance
//!
//! # Architecture
//!
//! ```text
//! PcSystem (state management) -> VideoAdapter trait -> {Software, Hardware} implementations
//! ```
//!
//! The PcSystem maintains state (registers, memory, etc.) and delegates
//! actual rendering operations to the video adapter backend.

use emu_core::types::Frame;

/// Trait for PC video adapter backends
///
/// This trait abstracts the actual rendering work, allowing different
/// implementations (software vs. hardware-accelerated) to be used interchangeably.
pub trait VideoAdapter: Send {
    /// Initialize the adapter with the given dimensions
    #[allow(dead_code)] // Used by implementations, kept for API completeness
    fn init(&mut self, width: usize, height: usize);

    /// Get the current framebuffer
    #[allow(dead_code)] // Used by implementations, kept for API completeness
    fn get_frame(&self) -> &Frame;

    /// Get mutable access to the framebuffer
    #[allow(dead_code)] // Used by implementations, kept for API completeness
    fn get_frame_mut(&mut self) -> &mut Frame;

    /// Get the framebuffer width in pixels
    fn fb_width(&self) -> usize;

    /// Get the framebuffer height in pixels
    fn fb_height(&self) -> usize;

    /// Render video memory to the framebuffer
    ///
    /// # Arguments
    /// * `vram` - Video memory buffer
    /// * `pixels` - Output pixel buffer (ARGB8888 format)
    fn render(&self, vram: &[u8], pixels: &mut [u32]);

    /// Reset the adapter to initial state
    #[allow(dead_code)] // Used by implementations, kept for API completeness
    fn reset(&mut self);

    /// Get the name of this adapter (for debugging/UI)
    #[allow(dead_code)] // Used by implementations, kept for API completeness
    fn name(&self) -> &str;

    /// Check if this adapter is hardware-accelerated
    #[allow(dead_code)] // Used by implementations, kept for API completeness
    fn is_hardware_accelerated(&self) -> bool {
        false
    }

    /// Resize the adapter to new dimensions
    #[allow(dead_code)] // Used by implementations, kept for API completeness
    fn resize(&mut self, width: usize, height: usize);
}
