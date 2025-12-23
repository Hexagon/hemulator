//! Video Adapter Trait - Abstraction for different graphics backends
//!
//! This module provides a trait for pluggable video rendering backends,
//! allowing the PC emulator to use either software rasterization or
//! hardware-accelerated rendering.
//!
//! # Design Philosophy
//!
//! The VideoAdapter trait follows the common renderer pattern defined in
//! `emu_core::renderer::Renderer`. All emulated systems with graphics capabilities
//! follow the same architectural approach:
//!
//! - **Software Renderer**: CPU-based rendering, maximum compatibility
//! - **Hardware Renderer**: GPU-accelerated rendering for performance
//!
//! # Architecture
//!
//! ```text
//! PcSystem (state management) -> VideoAdapter trait -> {Software, Hardware} implementations
//!                                     ↓
//!                     (follows emu_core::renderer::Renderer pattern)
//! ```
//!
//! The PcSystem maintains state (registers, memory, etc.) and delegates
//! actual rendering operations to the video adapter backend.
//!
//! # Core Methods (Common Pattern)
//!
//! Following the `emu_core::renderer::Renderer` pattern, all adapters provide:
//! - `get_frame()`: Get the current framebuffer
//! - `reset()`: Reset adapter to initial state  
//! - `name()`: Get adapter name for debugging/UI
//! - `is_hardware_accelerated()`: Check if GPU-accelerated
//! - `resize()`: Resize the adapter
//!
//! # PC-Specific Methods
//!
//! In addition to the common pattern, PC adapters provide:
//! - `render()`: Render VRAM to framebuffer (text/graphics modes)
//! - `fb_width()`, `fb_height()`: Get framebuffer dimensions
//! - `init()`: Initialize with specific dimensions

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
