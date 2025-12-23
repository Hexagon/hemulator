//! PC video hardware implementation (deprecated - use video_adapter modules)
//!
//! This module is kept for backward compatibility. New code should use the
//! modular video adapter system:
//! - `video_adapter::VideoAdapter` trait
//! - `video_adapter_software::SoftwareCgaAdapter` implementation
//!
//! The video adapter system follows the same pattern as N64's RdpRenderer,
//! allowing for both software and hardware-accelerated rendering.

// Re-export items from the new modular system for compatibility
#[allow(unused_imports)] // Re-exported for public API
pub use crate::video_adapter_software::{CgaColor, SoftwareCgaAdapter};

/// Legacy type alias for backward compatibility
///
/// DEPRECATED: Use `SoftwareCgaAdapter` directly or work with `VideoAdapter` trait
#[deprecated(
    since = "0.2.0",
    note = "Use SoftwareCgaAdapter or VideoAdapter trait instead"
)]
#[allow(dead_code)] // Kept for backward compatibility
pub type CgaVideo = SoftwareCgaAdapter;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::video_adapter::VideoAdapter;

    #[test]
    fn test_cga_color_to_rgb() {
        assert_eq!(CgaColor::Black.to_rgb(), 0xFF000000);
        assert_eq!(CgaColor::White.to_rgb(), 0xFFFFFFFF);
        assert_eq!(CgaColor::Blue.to_rgb(), 0xFF0000AA);
    }

    #[test]
    fn test_cga_color_from_u8() {
        assert_eq!(CgaColor::from_u8(0) as u8, CgaColor::Black as u8);
        assert_eq!(CgaColor::from_u8(15) as u8, CgaColor::White as u8);
        assert_eq!(CgaColor::from_u8(0x1F) as u8, CgaColor::White as u8); // Test masking
    }

    #[test]
    fn test_cga_video_creation() {
        #[allow(deprecated)]
        let video = CgaVideo::new();
        assert_eq!(video.fb_width(), 640);
        assert_eq!(video.fb_height(), 400);
    }

    #[test]
    fn test_render_empty_vram() {
        let video = SoftwareCgaAdapter::new();
        let vram = vec![0u8; 4000];
        let mut pixels = vec![0u32; 640 * 400];

        video.render(&vram, &mut pixels);

        // Should be all black (background)
        assert!(pixels.iter().all(|&p| p == 0xFF000000));
    }

    #[test]
    fn test_render_hello_world() {
        let video = SoftwareCgaAdapter::new();
        let mut vram = vec![0u8; 4000];

        // Write "Hello" at position 0
        let text = b"Hello";
        let attr = 0x0F; // White on black

        for (i, &ch) in text.iter().enumerate() {
            vram[i * 2] = ch;
            vram[i * 2 + 1] = attr;
        }

        let mut pixels = vec![0u32; 640 * 400];
        video.render(&vram, &mut pixels);

        // Check that not all pixels are black (some text was rendered)
        let non_black = pixels.iter().filter(|&&p| p != 0xFF000000).count();
        assert!(non_black > 0, "Expected some non-black pixels for text");
    }

    #[test]
    fn test_render_colored_text() {
        let video = SoftwareCgaAdapter::new();
        let mut vram = vec![0u8; 4000];

        // Write 'A' with light green on blue background
        vram[0] = b'A';
        vram[1] = 0x1A; // Light green (A) on blue (1) background

        let mut pixels = vec![0u32; 640 * 400];
        video.render(&vram, &mut pixels);

        // Check for blue background pixels
        let blue_pixels = pixels
            .iter()
            .filter(|&&p| p == CgaColor::Blue.to_rgb())
            .count();
        assert!(blue_pixels > 0, "Expected blue background pixels");
    }
}
