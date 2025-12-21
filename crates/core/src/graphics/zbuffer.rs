//! Z-buffer (depth buffer) implementation for hidden surface removal
//!
//! The Z-buffer is a fundamental component of 3D graphics rendering that stores
//! depth information for each pixel. It enables proper occlusion of objects based
//! on their distance from the camera.
//!
//! # Usage
//!
//! ```
//! use emu_core::graphics::ZBuffer;
//!
//! let mut zbuffer = ZBuffer::new(320, 240);
//! zbuffer.clear(); // Reset to far plane
//!
//! // Test if a pixel should be drawn (depth test)
//! if zbuffer.test_and_update(100, 100, 0x8000) {
//!     // Pixel is closer, draw it
//! }
//! ```

/// Z-buffer for depth testing
///
/// Stores 16-bit depth values where:
/// - 0x0000 = nearest (closest to camera)
/// - 0xFFFF = farthest (far plane)
pub struct ZBuffer {
    /// Width of the buffer in pixels
    width: u32,

    /// Height of the buffer in pixels
    height: u32,

    /// Depth values (u16 per pixel)
    /// Stored in row-major order: index = y * width + x
    buffer: Vec<u16>,

    /// Z-buffer enabled flag
    enabled: bool,
}

impl ZBuffer {
    /// Create a new Z-buffer with the specified dimensions
    ///
    /// The buffer is initialized with all pixels at maximum depth (0xFFFF, far plane)
    pub fn new(width: u32, height: u32) -> Self {
        let size = (width * height) as usize;
        Self {
            width,
            height,
            buffer: vec![0xFFFF; size],
            enabled: false,
        }
    }

    /// Clear the Z-buffer to maximum depth (far plane)
    pub fn clear(&mut self) {
        self.buffer.fill(0xFFFF);
    }

    /// Enable or disable depth testing
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Check if depth testing is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Get the width of the Z-buffer
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Get the height of the Z-buffer
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Test and update Z-buffer for a pixel
    ///
    /// Returns `true` if the pixel should be drawn (passes depth test), `false` otherwise.
    /// If the test passes, the Z-buffer is automatically updated with the new depth value.
    ///
    /// When Z-buffer is disabled, always returns `true` without updating the buffer.
    ///
    /// # Arguments
    ///
    /// * `x` - X coordinate of the pixel
    /// * `y` - Y coordinate of the pixel
    /// * `depth` - Depth value to test (0 = near, 0xFFFF = far)
    ///
    /// # Performance
    ///
    /// This is a hot path in rendering. Optimized for:
    /// - Bounds checking only when needed
    /// - Single buffer access for read-modify-write
    /// - Early return when disabled
    #[inline]
    pub fn test_and_update(&mut self, x: u32, y: u32, depth: u16) -> bool {
        if !self.enabled {
            return true; // Always pass if Z-buffer disabled
        }

        let idx = (y * self.width + x) as usize;
        if idx >= self.buffer.len() {
            return false; // Out of bounds
        }

        // Depth test: closer (smaller) values pass
        if depth < self.buffer[idx] {
            self.buffer[idx] = depth; // Update Z-buffer
            true
        } else {
            false
        }
    }

    /// Read depth value at a specific pixel (for testing/debugging)
    ///
    /// Returns `None` if coordinates are out of bounds
    pub fn read(&self, x: u32, y: u32) -> Option<u16> {
        let idx = (y * self.width + x) as usize;
        self.buffer.get(idx).copied()
    }

    /// Resize the Z-buffer to new dimensions
    ///
    /// This clears the buffer and reinitializes it to the far plane
    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        let size = (width * height) as usize;
        self.buffer = vec![0xFFFF; size];
    }
}

impl Default for ZBuffer {
    fn default() -> Self {
        Self::new(320, 240)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zbuffer_creation() {
        let zbuf = ZBuffer::new(320, 240);
        assert_eq!(zbuf.width(), 320);
        assert_eq!(zbuf.height(), 240);
        assert!(!zbuf.is_enabled());
    }

    #[test]
    fn test_zbuffer_initialization() {
        let zbuf = ZBuffer::new(100, 100);

        // All pixels should be initialized to far plane (0xFFFF)
        for y in 0..100 {
            for x in 0..100 {
                assert_eq!(zbuf.read(x, y), Some(0xFFFF));
            }
        }
    }

    #[test]
    fn test_zbuffer_clear() {
        let mut zbuf = ZBuffer::new(10, 10);
        zbuf.set_enabled(true);

        // Write some depth values
        zbuf.test_and_update(5, 5, 0x1000);
        zbuf.test_and_update(7, 7, 0x2000);

        // Clear buffer
        zbuf.clear();

        // All values should be back to far plane
        assert_eq!(zbuf.read(5, 5), Some(0xFFFF));
        assert_eq!(zbuf.read(7, 7), Some(0xFFFF));
    }

    #[test]
    fn test_zbuffer_enable_disable() {
        let mut zbuf = ZBuffer::new(10, 10);

        assert!(!zbuf.is_enabled());

        zbuf.set_enabled(true);
        assert!(zbuf.is_enabled());

        zbuf.set_enabled(false);
        assert!(!zbuf.is_enabled());
    }

    #[test]
    fn test_zbuffer_depth_test() {
        let mut zbuf = ZBuffer::new(10, 10);
        zbuf.set_enabled(true);

        // First write at depth 0x8000 should pass
        assert!(zbuf.test_and_update(5, 5, 0x8000));
        assert_eq!(zbuf.read(5, 5), Some(0x8000));

        // Write at farther depth (0x9000) should fail
        assert!(!zbuf.test_and_update(5, 5, 0x9000));
        assert_eq!(zbuf.read(5, 5), Some(0x8000)); // Unchanged

        // Write at closer depth (0x7000) should pass
        assert!(zbuf.test_and_update(5, 5, 0x7000));
        assert_eq!(zbuf.read(5, 5), Some(0x7000)); // Updated
    }

    #[test]
    fn test_zbuffer_disabled_always_passes() {
        let mut zbuf = ZBuffer::new(10, 10);
        // Z-buffer disabled by default

        // All depth tests should pass
        assert!(zbuf.test_and_update(5, 5, 0x8000));
        assert!(zbuf.test_and_update(5, 5, 0x9000));
        assert!(zbuf.test_and_update(5, 5, 0x7000));

        // Z-buffer should remain at far plane (not updated when disabled)
        assert_eq!(zbuf.read(5, 5), Some(0xFFFF));
    }

    #[test]
    fn test_zbuffer_bounds_checking() {
        let mut zbuf = ZBuffer::new(10, 10);
        zbuf.set_enabled(true);

        // Out of bounds should return false (fail test)
        assert!(!zbuf.test_and_update(100, 100, 0x8000));

        // Read out of bounds should return None
        assert_eq!(zbuf.read(100, 100), None);
    }

    #[test]
    fn test_zbuffer_resize() {
        let mut zbuf = ZBuffer::new(10, 10);
        zbuf.set_enabled(true);

        // Write some values
        zbuf.test_and_update(5, 5, 0x1000);

        // Resize
        zbuf.resize(20, 20);

        // Dimensions should be updated
        assert_eq!(zbuf.width(), 20);
        assert_eq!(zbuf.height(), 20);

        // Buffer should be cleared (reset to far plane)
        assert_eq!(zbuf.read(5, 5), Some(0xFFFF));

        // New coordinates should be accessible
        assert_eq!(zbuf.read(15, 15), Some(0xFFFF));
    }
}
