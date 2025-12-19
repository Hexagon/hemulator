//! CRT filter effects for post-processing emulator output
//!
//! This module provides various CRT (Cathode Ray Tube) display effects that can be applied
//! to the raw emulator frame buffer to simulate the appearance of old CRT monitors and TVs.

use serde::{Deserialize, Serialize};

/// Available CRT filter types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CrtFilter {
    /// No filter - raw pixels
    None,
    /// Scanlines effect - horizontal lines simulating CRT raster lines
    Scanlines,
    /// Phosphor effect - color bleeding and glow between pixels
    Phosphor,
    /// Full CRT effect - combines scanlines, phosphor, and subtle effects
    CrtMonitor,
}

impl CrtFilter {
    /// Get the name of the filter for display
    pub fn name(&self) -> &str {
        match self {
            CrtFilter::None => "None",
            CrtFilter::Scanlines => "Scanlines",
            CrtFilter::Phosphor => "Phosphor",
            CrtFilter::CrtMonitor => "CRT Monitor",
        }
    }

    /// Cycle to the next filter in the sequence
    pub fn next(&self) -> Self {
        match self {
            CrtFilter::None => CrtFilter::Scanlines,
            CrtFilter::Scanlines => CrtFilter::Phosphor,
            CrtFilter::Phosphor => CrtFilter::CrtMonitor,
            CrtFilter::CrtMonitor => CrtFilter::None,
        }
    }

    /// Apply the filter to a frame buffer
    ///
    /// # Arguments
    /// * `buffer` - The input frame buffer (will be modified in place)
    /// * `width` - Width of the frame
    /// * `height` - Height of the frame
    pub fn apply(&self, buffer: &mut [u32], width: usize, height: usize) {
        match self {
            CrtFilter::None => {
                // No processing needed
            }
            CrtFilter::Scanlines => {
                apply_scanlines(buffer, width, height);
            }
            CrtFilter::Phosphor => {
                apply_phosphor(buffer, width, height);
            }
            CrtFilter::CrtMonitor => {
                apply_crt_monitor(buffer, width, height);
            }
        }
    }
}

impl Default for CrtFilter {
    fn default() -> Self {
        CrtFilter::None
    }
}

/// Extract RGB components from a 0xRRGGBB color
#[inline]
fn unpack_rgb(color: u32) -> (u8, u8, u8) {
    let r = ((color >> 16) & 0xFF) as u8;
    let g = ((color >> 8) & 0xFF) as u8;
    let b = (color & 0xFF) as u8;
    (r, g, b)
}

/// Pack RGB components into 0xRRGGBB color
#[inline]
fn pack_rgb(r: u8, g: u8, b: u8) -> u32 {
    0xFF000000 | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

/// Blend two colors with a given ratio (0.0 = all color1, 1.0 = all color2)
#[inline]
fn blend_colors(color1: u32, color2: u32, ratio: f32) -> u32 {
    let (r1, g1, b1) = unpack_rgb(color1);
    let (r2, g2, b2) = unpack_rgb(color2);

    let r = (r1 as f32 * (1.0 - ratio) + r2 as f32 * ratio) as u8;
    let g = (g1 as f32 * (1.0 - ratio) + g2 as f32 * ratio) as u8;
    let b = (b1 as f32 * (1.0 - ratio) + b2 as f32 * ratio) as u8;

    pack_rgb(r, g, b)
}

/// Apply scanline effect - darkens every other horizontal line
fn apply_scanlines(buffer: &mut [u32], width: usize, height: usize) {
    for y in 0..height {
        // Darken every other scanline
        if y % 2 == 1 {
            for x in 0..width {
                let idx = y * width + x;
                if idx < buffer.len() {
                    let color = buffer[idx];
                    let (r, g, b) = unpack_rgb(color);
                    // Reduce brightness to 60% for scanlines
                    let r = (r as f32 * 0.6) as u8;
                    let g = (g as f32 * 0.6) as u8;
                    let b = (b as f32 * 0.6) as u8;
                    buffer[idx] = pack_rgb(r, g, b);
                }
            }
        }
    }
}

/// Apply phosphor effect - creates horizontal color bleeding between adjacent pixels
fn apply_phosphor(buffer: &mut [u32], width: usize, height: usize) {
    let temp_buffer = buffer.to_vec();

    for y in 0..height {
        for x in 0..width {
            let idx = y * width + x;
            if idx >= buffer.len() {
                continue;
            }

            let current = temp_buffer[idx];

            // Blend with neighboring pixels horizontally
            let mut blended = current;

            if x > 0 {
                let left = temp_buffer[idx - 1];
                blended = blend_colors(blended, left, 0.15);
            }

            if x < width - 1 {
                let right = temp_buffer[idx + 1];
                blended = blend_colors(blended, right, 0.15);
            }

            buffer[idx] = blended;
        }
    }
}

/// Apply full CRT monitor effect - combines scanlines with phosphor glow
fn apply_crt_monitor(buffer: &mut [u32], width: usize, height: usize) {
    // First apply phosphor effect for color bleeding
    apply_phosphor(buffer, width, height);

    // Then apply scanlines
    for y in 0..height {
        if y % 2 == 1 {
            for x in 0..width {
                let idx = y * width + x;
                if idx < buffer.len() {
                    let color = buffer[idx];
                    let (r, g, b) = unpack_rgb(color);
                    // Reduce brightness to 70% for scanlines (less aggressive than scanlines-only)
                    let r = (r as f32 * 0.7) as u8;
                    let g = (g as f32 * 0.7) as u8;
                    let b = (b as f32 * 0.7) as u8;
                    buffer[idx] = pack_rgb(r, g, b);
                }
            }
        }
    }

    // Add slight brightness boost to non-scanline rows for contrast
    for y in 0..height {
        if y % 2 == 0 {
            for x in 0..width {
                let idx = y * width + x;
                if idx < buffer.len() {
                    let color = buffer[idx];
                    let (r, g, b) = unpack_rgb(color);
                    // Slight brightness boost (105%)
                    let r = r.saturating_add((r as f32 * 0.05) as u8);
                    let g = g.saturating_add((g as f32 * 0.05) as u8);
                    let b = b.saturating_add((b as f32 * 0.05) as u8);
                    buffer[idx] = pack_rgb(r, g, b);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_cycling() {
        let filter = CrtFilter::None;
        assert_eq!(filter.next(), CrtFilter::Scanlines);

        let filter = CrtFilter::Scanlines;
        assert_eq!(filter.next(), CrtFilter::Phosphor);

        let filter = CrtFilter::Phosphor;
        assert_eq!(filter.next(), CrtFilter::CrtMonitor);

        let filter = CrtFilter::CrtMonitor;
        assert_eq!(filter.next(), CrtFilter::None);
    }

    #[test]
    fn test_filter_names() {
        assert_eq!(CrtFilter::None.name(), "None");
        assert_eq!(CrtFilter::Scanlines.name(), "Scanlines");
        assert_eq!(CrtFilter::Phosphor.name(), "Phosphor");
        assert_eq!(CrtFilter::CrtMonitor.name(), "CRT Monitor");
    }

    #[test]
    fn test_unpack_pack_rgb() {
        let color = 0xFFAABBCC;
        let (r, g, b) = unpack_rgb(color);
        assert_eq!(r, 0xAA);
        assert_eq!(g, 0xBB);
        assert_eq!(b, 0xCC);

        let packed = pack_rgb(r, g, b);
        assert_eq!(packed & 0x00FFFFFF, color & 0x00FFFFFF);
    }

    #[test]
    fn test_blend_colors() {
        let color1 = pack_rgb(100, 100, 100);
        let color2 = pack_rgb(200, 200, 200);

        let blended = blend_colors(color1, color2, 0.5);
        let (r, g, b) = unpack_rgb(blended);

        // Should be roughly midpoint
        assert!((r as i32 - 150).abs() <= 1);
        assert!((g as i32 - 150).abs() <= 1);
        assert!((b as i32 - 150).abs() <= 1);
    }

    #[test]
    fn test_scanlines_filter() {
        let width = 4;
        let height = 4;
        let mut buffer = vec![0xFFFFFFFF; width * height];

        apply_scanlines(&mut buffer, width, height);

        // Check that odd rows are darkened
        for y in 0..height {
            for x in 0..width {
                let idx = y * width + x;
                let (r, g, b) = unpack_rgb(buffer[idx]);
                if y % 2 == 1 {
                    // Darkened scanline
                    assert!(r < 255);
                    assert!(g < 255);
                    assert!(b < 255);
                } else {
                    // Original brightness
                    assert_eq!(r, 255);
                    assert_eq!(g, 255);
                    assert_eq!(b, 255);
                }
            }
        }
    }

    #[test]
    fn test_phosphor_filter() {
        let width = 3;
        let height = 1;
        let mut buffer = vec![0xFF000000, 0xFFFFFFFF, 0xFF000000]; // Black-White-Black

        apply_phosphor(&mut buffer, width, height);

        // Middle pixel should remain bright but will be slightly dimmed from blending
        let (r, g, b) = unpack_rgb(buffer[1]);
        assert!(r > 180); // Should still be quite bright (accounting for blending)

        // Edge pixels should have some bleed from the middle
        let (r0, g0, b0) = unpack_rgb(buffer[0]);
        let (r2, g2, b2) = unpack_rgb(buffer[2]);
        assert!(r0 > 0); // Should have some brightness from neighbor
        assert!(r2 > 0);
    }

    #[test]
    fn test_default_filter() {
        assert_eq!(CrtFilter::default(), CrtFilter::None);
    }
}
