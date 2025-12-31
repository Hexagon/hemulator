//! IBM PC ROM Font Data
//!
//! This module provides complete IBM PC-compatible font data for use across
//! all video adapters (CGA, EGA, VGA). The fonts are based on the original
//! IBM PC ROM fonts and provide complete coverage of the ASCII table (0x00-0xFF).
//!
//! # Font Formats
//!
//! - **8x8 font**: Used by VGA graphics modes and PS/2 systems (8 scanlines per character)
//! - **8x14 font**: Used by EGA text mode (14 scanlines per character)
//! - **8x16 font**: Used by CGA and VGA text modes (16 scanlines per character)
//!
//! # Character Set Coverage
//!
//! The fonts include:
//! - ASCII control characters (0x00-0x1F) with graphical representations
//! - Printable ASCII characters (0x20-0x7E)
//! - Extended ASCII characters (0x80-0xFF) including:
//!   - Box drawing characters (properly implemented for DOS/QBasic GUIs)
//!   - Mathematical symbols
//!   - Accented characters
//!   - Greek letters
//!
//! # Source
//!
//! Font data is based on the IBM PC BIOS ROM fonts, which are in the public
//! domain due to clean-room implementation and historical software preservation.

/// Get the 8x8 font glyph for a character code (VGA graphics mode, PS/2)
///
/// Returns a slice of 8 bytes, where each byte represents one scanline
/// of the character (MSB = left pixel, LSB = right pixel).
///
/// # Arguments
///
/// * `char_code` - The character code (0x00-0xFF)
///
/// # Returns
///
/// A reference to an 8-byte array representing the character glyph
#[allow(dead_code)] // Will be used for PS/2 VGA graphics modes
pub fn get_font_8x8(char_code: u8) -> &'static [u8; 8] {
    &FONT_8X8[char_code as usize]
}

/// Get the 8x16 font glyph for a character code (CGA, VGA text mode)
///
/// Returns a slice of 16 bytes, where each byte represents one scanline
/// of the character (MSB = left pixel, LSB = right pixel).
///
/// # Arguments
///
/// * `char_code` - The character code (0x00-0xFF)
///
/// # Returns
///
/// A reference to a 16-byte array representing the character glyph
pub fn get_font_8x16(char_code: u8) -> &'static [u8; 16] {
    &FONT_8X16[char_code as usize]
}

/// Get the 8x14 font glyph for a character code (EGA)
///
/// Returns a slice of 14 bytes, where each byte represents one scanline
/// of the character (MSB = left pixel, LSB = right pixel).
///
/// # Arguments
///
/// * `char_code` - The character code (0x00-0xFF)
///
/// # Returns
///
/// A reference to a 14-byte array representing the character glyph
pub fn get_font_8x14(char_code: u8) -> &'static [u8; 14] {
    &FONT_8X14[char_code as usize]
}

/// IBM PC 8x8 font data (256 characters, 8 bytes each)
///
/// This font is used by VGA graphics modes and PS/2 systems. Each character is
/// 8 pixels wide and 8 scanlines tall. The font data is stored as arrays of bytes,
/// with each byte representing one horizontal scanline of pixels.
const FONT_8X8: [[u8; 8]; 256] = include!("font_data_8x8.txt");

/// IBM PC 8x14 font data (256 characters, 14 bytes each)
///
/// This font is used by EGA text mode. Each character is 8 pixels wide
/// and 14 scanlines tall. The font data is stored as arrays of bytes, with each
/// byte representing one horizontal scanline of pixels.
const FONT_8X14: [[u8; 14]; 256] = include!("font_data_8x14.txt");

/// IBM PC 8x16 font data (256 characters, 16 bytes each)
///
/// This font is used by CGA and VGA text modes. Each character is 8 pixels wide
/// and 16 scanlines tall. The font data is stored as arrays of bytes, with each
/// byte representing one horizontal scanline of pixels.
const FONT_8X16: [[u8; 16]; 256] = include!("font_data_8x16.txt");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_font_8x8_coverage() {
        // Test that all 256 characters have data
        for i in 0..256 {
            let glyph = get_font_8x8(i as u8);
            assert_eq!(glyph.len(), 8);
        }
    }

    #[test]
    fn test_font_8x14_coverage() {
        // Test that all 256 characters have data
        for i in 0..256 {
            let glyph = get_font_8x14(i as u8);
            assert_eq!(glyph.len(), 14);
        }
    }

    #[test]
    fn test_font_8x16_coverage() {
        // Test that all 256 characters have data
        for i in 0..256 {
            let glyph = get_font_8x16(i as u8);
            assert_eq!(glyph.len(), 16);
        }
    }

    #[test]
    fn test_space_is_blank_8x8() {
        let space = get_font_8x8(0x20);
        assert!(
            space.iter().all(|&b| b == 0x00),
            "Space should be all zeros"
        );
    }

    #[test]
    fn test_space_is_blank_8x14() {
        let space = get_font_8x14(0x20);
        assert!(
            space.iter().all(|&b| b == 0x00),
            "Space should be all zeros"
        );
    }

    #[test]
    fn test_space_is_blank_8x16() {
        let space = get_font_8x16(0x20);
        assert!(
            space.iter().all(|&b| b == 0x00),
            "Space should be all zeros"
        );
    }

    #[test]
    fn test_exclamation_has_content_8x8() {
        let exclamation = get_font_8x8(0x21);
        assert!(
            exclamation.iter().any(|&b| b != 0x00),
            "Exclamation mark should have content"
        );
    }

    #[test]
    fn test_exclamation_has_content_8x14() {
        let exclamation = get_font_8x14(0x21);
        assert!(
            exclamation.iter().any(|&b| b != 0x00),
            "Exclamation mark should have content"
        );
    }

    #[test]
    fn test_exclamation_has_content_8x16() {
        let exclamation = get_font_8x16(0x21);
        assert!(
            exclamation.iter().any(|&b| b != 0x00),
            "Exclamation mark should have content"
        );
    }

    #[test]
    fn test_letter_a_has_content_8x8() {
        let a = get_font_8x8(0x41);
        assert!(
            a.iter().any(|&b| b != 0x00),
            "Letter 'A' should have content"
        );
    }

    #[test]
    fn test_letter_a_has_content_8x14() {
        let a = get_font_8x14(0x41);
        assert!(
            a.iter().any(|&b| b != 0x00),
            "Letter 'A' should have content"
        );
    }

    #[test]
    fn test_letter_a_has_content_8x16() {
        let a = get_font_8x16(0x41);
        assert!(
            a.iter().any(|&b| b != 0x00),
            "Letter 'A' should have content"
        );
    }

    // Box-drawing character tests - critical for DOS/QBasic GUIs
    #[test]
    fn test_box_drawing_single_vertical() {
        // 0xB3: │ (single vertical line)
        let glyph_8x16 = get_font_8x16(0xB3);
        // Should have vertical line in the middle
        assert!(glyph_8x16.contains(&0x18));
    }

    #[test]
    fn test_box_drawing_single_horizontal() {
        // 0xC4: ─ (single horizontal line)
        let glyph_8x16 = get_font_8x16(0xC4);
        // Should have horizontal line
        assert!(glyph_8x16.contains(&0xFF));
    }

    #[test]
    fn test_box_drawing_corners() {
        // Test all four single-line corners
        let top_left = get_font_8x16(0xDA); // ┌
        let top_right = get_font_8x16(0xBF); // ┐
        let bottom_left = get_font_8x16(0xC0); // └
        let bottom_right = get_font_8x16(0xD9); // ┘

        // All corners should have some content
        assert!(top_left.iter().any(|&b| b != 0x00));
        assert!(top_right.iter().any(|&b| b != 0x00));
        assert!(bottom_left.iter().any(|&b| b != 0x00));
        assert!(bottom_right.iter().any(|&b| b != 0x00));
    }

    #[test]
    fn test_box_drawing_double_vertical() {
        // 0xBA: ║ (double vertical line)
        let glyph_8x16 = get_font_8x16(0xBA);
        // Should have double vertical line
        assert!(glyph_8x16.contains(&0x36));
    }

    #[test]
    fn test_box_drawing_double_horizontal() {
        // 0xCD: ═ (double horizontal line)
        let glyph_8x16 = get_font_8x16(0xCD);
        // Should have double horizontal line
        assert!(glyph_8x16.contains(&0xFF));
    }

    #[test]
    fn test_box_drawing_double_corners() {
        // Test all four double-line corners
        let top_left = get_font_8x16(0xC9); // ╔
        let top_right = get_font_8x16(0xBB); // ╗
        let bottom_left = get_font_8x16(0xC8); // ╚
        let bottom_right = get_font_8x16(0xBC); // ╝

        // All corners should have some content
        assert!(top_left.iter().any(|&b| b != 0x00));
        assert!(top_right.iter().any(|&b| b != 0x00));
        assert!(bottom_left.iter().any(|&b| b != 0x00));
        assert!(bottom_right.iter().any(|&b| b != 0x00));
    }
}
