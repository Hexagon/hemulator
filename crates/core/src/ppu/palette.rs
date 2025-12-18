//! Generic indexed palette system for retro video hardware.
//!
//! Many retro systems use indexed palettes where pixel values are indices
//! into a palette RAM that stores the actual RGB colors. This module provides
//! a reusable abstraction for such systems.
//!
//! # Examples
//!
//! - NES: 32-byte palette RAM with special mirroring rules
//! - Game Boy: 4 palettes of 4 colors each (monochrome)
//! - Game Boy Color: 8 background + 8 sprite palettes of 4 colors (RGB555)
//! - SNES: 256-color CGRAM with multiple palette configurations
//! - Genesis: 4 palettes of 16 colors (RGB444)

/// Generic indexed palette that maps color indices to RGB values.
///
/// Systems can wrap this with their own logic for:
/// - Mirroring (e.g., NES sprite palette 0 mirrors backdrop)
/// - Multiple palettes (e.g., separate BG and sprite palettes)
/// - Hardware registers (e.g., palette RAM address register)
pub trait IndexedPalette {
    /// Get the RGB color for a palette index.
    /// Returns a 32-bit ARGB color (0xAARRGGBB).
    fn get_color(&self, index: usize) -> u32;

    /// Set the RGB color for a palette index.
    fn set_color(&mut self, index: usize, color: u32);

    /// Get the number of colors in this palette.
    fn len(&self) -> usize;

    /// Check if the palette is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// A simple RAM-based palette storage.
///
/// This provides basic indexed palette functionality that can be used
/// by most systems. System-specific logic (mirroring, multiple palettes, etc.)
/// should be implemented in the system's PPU.
#[derive(Debug, Clone)]
pub struct RamPalette {
    colors: Vec<u32>,
}

impl RamPalette {
    /// Create a new palette with the specified number of colors.
    pub fn new(size: usize) -> Self {
        Self {
            colors: vec![0xFF000000; size], // Default to opaque black
        }
    }

    /// Create a palette from a list of colors.
    pub fn from_colors(colors: Vec<u32>) -> Self {
        Self { colors }
    }

    /// Get a slice of all colors.
    pub fn colors(&self) -> &[u32] {
        &self.colors
    }

    /// Get a mutable slice of all colors.
    pub fn colors_mut(&mut self) -> &mut [u32] {
        &mut self.colors
    }
}

impl IndexedPalette for RamPalette {
    fn get_color(&self, index: usize) -> u32 {
        self.colors.get(index).copied().unwrap_or(0xFF000000)
    }

    fn set_color(&mut self, index: usize, color: u32) {
        if index < self.colors.len() {
            self.colors[index] = color;
        }
    }

    fn len(&self) -> usize {
        self.colors.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ram_palette_creation() {
        let palette = RamPalette::new(16);
        assert_eq!(palette.len(), 16);
        assert!(!palette.is_empty());
        
        // All colors should default to opaque black
        for i in 0..16 {
            assert_eq!(palette.get_color(i), 0xFF000000);
        }
    }

    #[test]
    fn test_ram_palette_set_get() {
        let mut palette = RamPalette::new(4);
        
        palette.set_color(0, 0xFF0000FF); // Blue
        palette.set_color(1, 0xFF00FF00); // Green
        palette.set_color(2, 0xFFFF0000); // Red
        palette.set_color(3, 0xFFFFFFFF); // White
        
        assert_eq!(palette.get_color(0), 0xFF0000FF);
        assert_eq!(palette.get_color(1), 0xFF00FF00);
        assert_eq!(palette.get_color(2), 0xFFFF0000);
        assert_eq!(palette.get_color(3), 0xFFFFFFFF);
    }

    #[test]
    fn test_ram_palette_out_of_bounds() {
        let mut palette = RamPalette::new(4);
        
        // Out of bounds reads should return black
        assert_eq!(palette.get_color(10), 0xFF000000);
        
        // Out of bounds writes should not panic
        palette.set_color(10, 0xFFFF0000);
    }

    #[test]
    fn test_ram_palette_from_colors() {
        let colors = vec![0xFF0000FF, 0xFF00FF00, 0xFFFF0000];
        let palette = RamPalette::from_colors(colors.clone());
        
        assert_eq!(palette.len(), 3);
        assert_eq!(palette.get_color(0), 0xFF0000FF);
        assert_eq!(palette.get_color(1), 0xFF00FF00);
        assert_eq!(palette.get_color(2), 0xFFFF0000);
    }
}
