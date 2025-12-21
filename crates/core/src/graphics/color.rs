//! Color operation utilities for graphics rendering
//!
//! Provides common color manipulation functions used across different graphics
//! systems. Colors are in ARGB8888 format (0xAARRGGBB).

/// Color operation utilities
pub struct ColorOps;

impl ColorOps {
    /// Linear interpolation between two ARGB colors
    ///
    /// # Arguments
    ///
    /// * `c0` - Start color (ARGB8888 format: 0xAARRGGBB)
    /// * `c1` - End color (ARGB8888 format: 0xAARRGGBB)
    /// * `t` - Interpolation factor (0.0 = c0, 1.0 = c1)
    ///
    /// # Example
    ///
    /// ```
    /// use emu_core::graphics::ColorOps;
    ///
    /// let red = 0xFFFF0000;
    /// let blue = 0xFF0000FF;
    /// let purple = ColorOps::lerp(red, blue, 0.5); // Mix 50/50
    /// ```
    #[inline]
    pub fn lerp(c0: u32, c1: u32, t: f32) -> u32 {
        let a0 = ((c0 >> 24) & 0xFF) as f32;
        let r0 = ((c0 >> 16) & 0xFF) as f32;
        let g0 = ((c0 >> 8) & 0xFF) as f32;
        let b0 = (c0 & 0xFF) as f32;

        let a1 = ((c1 >> 24) & 0xFF) as f32;
        let r1 = ((c1 >> 16) & 0xFF) as f32;
        let g1 = ((c1 >> 8) & 0xFF) as f32;
        let b1 = (c1 & 0xFF) as f32;

        let a = (a0 + (a1 - a0) * t).round() as u32;
        let r = (r0 + (r1 - r0) * t).round() as u32;
        let g = (g0 + (g1 - g0) * t).round() as u32;
        let b = (b0 + (b1 - b0) * t).round() as u32;

        (a << 24) | (r << 16) | (g << 8) | b
    }

    /// Adjust brightness of an ARGB color by a scaling factor
    ///
    /// # Arguments
    ///
    /// * `color` - Input color (ARGB8888 format: 0xAARRGGBB)
    /// * `factor` - Brightness factor (1.0 = unchanged, <1.0 = darker, >1.0 = brighter)
    ///
    /// Alpha channel is preserved unchanged. RGB channels are scaled and clamped to [0, 255].
    ///
    /// # Example
    ///
    /// ```
    /// use emu_core::graphics::ColorOps;
    ///
    /// let color = 0xFFFF8040;
    /// let darker = ColorOps::adjust_brightness(color, 0.5); // 50% brightness
    /// let brighter = ColorOps::adjust_brightness(color, 1.5); // 150% brightness
    /// ```
    #[inline]
    pub fn adjust_brightness(color: u32, factor: f32) -> u32 {
        let a = ((color >> 24) & 0xFF) as f32;
        let r = ((color >> 16) & 0xFF) as f32;
        let g = ((color >> 8) & 0xFF) as f32;
        let b = (color & 0xFF) as f32;

        let new_a = a; // Keep alpha unchanged
        let new_r = (r * factor).min(255.0).round() as u32;
        let new_g = (g * factor).min(255.0).round() as u32;
        let new_b = (b * factor).min(255.0).round() as u32;

        ((new_a as u32) << 24) | (new_r << 16) | (new_g << 8) | new_b
    }

    /// Extract red channel from ARGB color
    #[inline]
    pub fn red(color: u32) -> u8 {
        ((color >> 16) & 0xFF) as u8
    }

    /// Extract green channel from ARGB color
    #[inline]
    pub fn green(color: u32) -> u8 {
        ((color >> 8) & 0xFF) as u8
    }

    /// Extract blue channel from ARGB color
    #[inline]
    pub fn blue(color: u32) -> u8 {
        (color & 0xFF) as u8
    }

    /// Extract alpha channel from ARGB color
    #[inline]
    pub fn alpha(color: u32) -> u8 {
        ((color >> 24) & 0xFF) as u8
    }

    /// Construct ARGB color from components
    #[inline]
    pub fn from_argb(a: u8, r: u8, g: u8, b: u8) -> u32 {
        ((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
    }

    /// Construct RGB color with full alpha
    #[inline]
    pub fn from_rgb(r: u8, g: u8, b: u8) -> u32 {
        0xFF000000 | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_lerp() {
        // ARGB format: 0xAARRGGBB
        let c0 = 0xFFFF0000; // Red with full alpha
        let c1 = 0xFF0000FF; // Blue with full alpha

        // 50% interpolation should give purple
        let c_mid = ColorOps::lerp(c0, c1, 0.5);
        let a = ColorOps::alpha(c_mid);
        let r = ColorOps::red(c_mid);
        let g = ColorOps::green(c_mid);
        let b = ColorOps::blue(c_mid);

        assert_eq!(a, 255, "Alpha should be full");
        // Allow for rounding: 127 or 128 are both valid for 50%
        assert!((127..=128).contains(&r), "Red component should be ~50%");
        assert_eq!(g, 0, "Green component should be 0");
        assert!((127..=128).contains(&b), "Blue component should be ~50%");

        // 0% should give c0
        let c_start = ColorOps::lerp(c0, c1, 0.0);
        assert_eq!(c_start, c0);

        // 100% should give c1
        let c_end = ColorOps::lerp(c0, c1, 1.0);
        assert_eq!(c_end, c1);
    }

    #[test]
    fn test_color_adjust_brightness() {
        let color = 0xFFFF8040; // ARGB: Full alpha, R=255, G=128, B=64

        // Factor 1.0 should return original color
        let same = ColorOps::adjust_brightness(color, 1.0);
        assert_eq!(same, color);

        // Factor 0.5 should halve RGB values
        let darker = ColorOps::adjust_brightness(color, 0.5);
        let a = ColorOps::alpha(darker);
        let r = ColorOps::red(darker);
        let g = ColorOps::green(darker);
        let b = ColorOps::blue(darker);
        assert_eq!(a, 255, "Alpha should remain unchanged");
        assert!((127..=128).contains(&r), "Red should be halved (~128)");
        assert_eq!(g, 64, "Green should be halved (64)");
        assert_eq!(b, 32, "Blue should be halved (32)");

        // Factor 2.0 should double but cap at 255
        let brighter = ColorOps::adjust_brightness(0xFF804020, 2.0);
        let r2 = ColorOps::red(brighter);
        let g2 = ColorOps::green(brighter);
        let b2 = ColorOps::blue(brighter);
        assert_eq!(r2, 255, "Red should cap at 255");
        assert_eq!(g2, 128, "Green should double (128)");
        assert_eq!(b2, 64, "Blue should double (64)");
    }

    #[test]
    fn test_color_component_extraction() {
        let color = 0xAABBCCDD; // A=0xAA, R=0xBB, G=0xCC, B=0xDD

        assert_eq!(ColorOps::alpha(color), 0xAA);
        assert_eq!(ColorOps::red(color), 0xBB);
        assert_eq!(ColorOps::green(color), 0xCC);
        assert_eq!(ColorOps::blue(color), 0xDD);
    }

    #[test]
    fn test_color_from_argb() {
        let color = ColorOps::from_argb(0xAA, 0xBB, 0xCC, 0xDD);
        assert_eq!(color, 0xAABBCCDD);
    }

    #[test]
    fn test_color_from_rgb() {
        let color = ColorOps::from_rgb(0xBB, 0xCC, 0xDD);
        assert_eq!(color, 0xFFBBCCDD); // Full alpha
    }
}
