//! Modular video processing abstraction
//!
//! This module provides an abstraction layer for video processing and post-processing,
//! allowing different backends (software, OpenGL, etc.) to be used interchangeably.
//!
//! # Design Philosophy
//!
//! The VideoProcessor trait follows a similar pattern to `emu_core::renderer::Renderer`,
//! but focuses on post-processing rather than core rendering:
//!
//! - **Software Processor**: CPU-based post-processing, maximum compatibility
//! - **OpenGL Processor**: GPU-accelerated post-processing for performance
//!
//! # Architecture
//!
//! ```text
//! System Renderer -> Frame -> VideoProcessor -> Post-Processed Frame -> Display
//!                                   ↓
//!                    (follows similar pattern to Renderer)
//! ```
//!
//! The VideoProcessor applies effects (CRT filters, scaling, etc.) to frames
//! after they've been rendered by the system's renderer.
//!
//! # Core Methods (Common Pattern)
//!
//! Similar to `emu_core::renderer::Renderer`, processors provide:
//! - `init()`: Initialize with dimensions
//! - `process_frame()`: Apply effects to a frame
//! - `resize()`: Handle resolution changes
//! - `name()`: Get processor name for debugging/UI
//! - `is_hardware_accelerated()`: Check if GPU-accelerated

use crate::crt_filter::CrtFilter;

mod opengl;
pub use opengl::OpenGLProcessor;

/// Result type for video processor operations
#[allow(dead_code)]
pub type VideoResult<T> = Result<T, Box<dyn std::error::Error>>;

/// Video processor trait - abstraction for different rendering backends
#[allow(dead_code)]
pub trait VideoProcessor {
    /// Initialize the video processor with the given dimensions
    fn init(&mut self, width: usize, height: usize) -> VideoResult<()>;

    /// Process a frame buffer with the current filter settings
    ///
    /// # Arguments
    /// * `buffer` - Input frame buffer (ARGB format, 0xAARRGGBB)
    /// * `width` - Frame width
    /// * `height` - Frame height
    /// * `filter` - CRT filter to apply
    ///
    /// # Returns
    /// Processed frame buffer ready for display
    fn process_frame(
        &mut self,
        buffer: &[u32],
        width: usize,
        height: usize,
        filter: CrtFilter,
    ) -> VideoResult<Vec<u32>>;

    /// Resize the processor to new dimensions
    fn resize(&mut self, width: usize, height: usize) -> VideoResult<()>;

    /// Get the name of this processor (for debugging/UI)
    fn name(&self) -> &str;

    /// Check if this processor is hardware-accelerated
    fn is_hardware_accelerated(&self) -> bool {
        false
    }
}

/// Software-based video processor (current implementation)
#[allow(dead_code)]
pub struct SoftwareProcessor;

impl SoftwareProcessor {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self
    }
}

impl Default for SoftwareProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl VideoProcessor for SoftwareProcessor {
    fn init(&mut self, _width: usize, _height: usize) -> VideoResult<()> {
        // Software processor needs no initialization
        Ok(())
    }

    fn process_frame(
        &mut self,
        buffer: &[u32],
        width: usize,
        height: usize,
        filter: CrtFilter,
    ) -> VideoResult<Vec<u32>> {
        // Clone the buffer and apply filter in-place
        let mut output = buffer.to_vec();
        filter.apply(&mut output, width, height);
        Ok(output)
    }

    fn resize(&mut self, _width: usize, _height: usize) -> VideoResult<()> {
        // Software processor doesn't need to handle resize
        Ok(())
    }

    fn name(&self) -> &str {
        "Software Renderer"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_software_processor_creation() {
        let processor = SoftwareProcessor::new();
        assert_eq!(processor.name(), "Software Renderer");
        assert!(!processor.is_hardware_accelerated());
    }

    #[test]
    fn test_software_processor_init() {
        let mut processor = SoftwareProcessor::new();
        assert!(processor.init(256, 240).is_ok());
    }

    #[test]
    fn test_software_processor_process_frame() {
        let mut processor = SoftwareProcessor::new();
        processor.init(256, 240).unwrap();

        let buffer = vec![0xFFFFFFFF; 256 * 240];
        let result = processor.process_frame(&buffer, 256, 240, CrtFilter::None);
        assert!(result.is_ok());

        let processed = result.unwrap();
        assert_eq!(processed.len(), 256 * 240);
    }

    #[test]
    fn test_software_processor_with_filter() {
        let mut processor = SoftwareProcessor::new();
        processor.init(256, 240).unwrap();

        let buffer = vec![0xFFFFFFFF; 256 * 240];
        let result = processor.process_frame(&buffer, 256, 240, CrtFilter::Scanlines);
        assert!(result.is_ok());

        let processed = result.unwrap();
        // Scanlines should darken every other row
        assert_eq!(processed.len(), 256 * 240);
    }
}
