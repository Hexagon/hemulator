//! Tile/pattern rendering utilities for tile-based video systems.
//!
//! Most retro systems use tile-based rendering where the screen is divided
//! into 8x8 pixel tiles. This module provides utilities for decoding and
//! rendering tile data in various formats.
//!
//! # Common Tile Formats
//!
//! - **NES (2bpp planar)**: Two bitplanes stored sequentially
//! - **Game Boy (2bpp planar)**: Similar to NES but slightly different layout
//! - **SNES (2/4/8bpp planar)**: Multiple bitplane configurations
//! - **Genesis (4bpp linear)**: 4 bits per pixel, stored linearly
//! - **Game Gear/Master System (4bpp linear)**: Similar to Genesis

/// Tile format specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileFormat {
    /// NES/Famicom: 2 bits per pixel, planar format.
    /// Each tile is 16 bytes: 8 bytes for low plane, 8 bytes for high plane.
    Nes2Bpp,

    /// Game Boy: 2 bits per pixel, planar format.
    /// Each tile is 16 bytes with interleaved bitplanes (2 bytes per row).
    GameBoy2Bpp,

    /// SNES: 2 bits per pixel, planar format (Mode 0-1).
    Snes2Bpp,

    /// SNES: 4 bits per pixel, planar format (Mode 1-4).
    Snes4Bpp,

    /// Genesis/Mega Drive: 4 bits per pixel, linear format.
    /// Each tile is 32 bytes with 4 bits per pixel stored sequentially.
    Genesis4Bpp,
}

/// Trait for decoding tile data into pixel indices.
pub trait TileDecoder {
    /// Decode a single pixel from a tile.
    ///
    /// # Arguments
    /// * `tile_data` - The raw tile data
    /// * `x` - X coordinate within the tile (0-7)
    /// * `y` - Y coordinate within the tile (0-7)
    ///
    /// # Returns
    /// The palette index for this pixel (0-3 for 2bpp, 0-15 for 4bpp, etc.)
    fn decode_pixel(&self, tile_data: &[u8], x: u8, y: u8) -> u8;

    /// Get the size of a single tile in bytes.
    fn tile_size(&self) -> usize;
}

/// NES/Famicom 2bpp planar tile decoder.
///
/// Each 8x8 tile is stored in 16 bytes:
/// - Bytes 0-7: Low bitplane (one bit per pixel for 8 rows)
/// - Bytes 8-15: High bitplane (one bit per pixel for 8 rows)
#[derive(Debug, Clone, Copy)]
pub struct Nes2BppDecoder;

impl TileDecoder for Nes2BppDecoder {
    fn decode_pixel(&self, tile_data: &[u8], x: u8, y: u8) -> u8 {
        if tile_data.len() < 16 || x > 7 || y > 7 {
            return 0;
        }

        let lo = tile_data[y as usize];
        let hi = tile_data[y as usize + 8];
        let bit = 7 - x;
        let lo_bit = (lo >> bit) & 1;
        let hi_bit = (hi >> bit) & 1;

        (hi_bit << 1) | lo_bit
    }

    fn tile_size(&self) -> usize {
        16
    }
}

/// Game Boy 2bpp planar tile decoder.
///
/// Each 8x8 tile is stored in 16 bytes with interleaved bitplanes:
/// - Bytes 0-1: Low and high bitplanes for row 0
/// - Bytes 2-3: Low and high bitplanes for row 1
/// - And so on...
#[derive(Debug, Clone, Copy)]
pub struct GameBoy2BppDecoder;

impl TileDecoder for GameBoy2BppDecoder {
    fn decode_pixel(&self, tile_data: &[u8], x: u8, y: u8) -> u8 {
        if tile_data.len() < 16 || x > 7 || y > 7 {
            return 0;
        }

        let row_offset = (y as usize) * 2;
        let lo = tile_data[row_offset];
        let hi = tile_data[row_offset + 1];
        let bit = 7 - x;
        let lo_bit = (lo >> bit) & 1;
        let hi_bit = (hi >> bit) & 1;

        (hi_bit << 1) | lo_bit
    }

    fn tile_size(&self) -> usize {
        16
    }
}

/// SNES 4bpp planar tile decoder.
///
/// Each 8x8 tile is stored in 32 bytes:
/// - Bytes 0-7: Bitplane 0 (one bit per pixel for 8 rows)
/// - Bytes 8-15: Bitplane 1 (one bit per pixel for 8 rows)
/// - Bytes 16-23: Bitplane 2 (one bit per pixel for 8 rows)
/// - Bytes 24-31: Bitplane 3 (one bit per pixel for 8 rows)
#[derive(Debug, Clone, Copy)]
pub struct Snes4BppDecoder;

impl TileDecoder for Snes4BppDecoder {
    fn decode_pixel(&self, tile_data: &[u8], x: u8, y: u8) -> u8 {
        if tile_data.len() < 32 || x > 7 || y > 7 {
            return 0;
        }

        let bit = 7 - x;
        let plane0 = (tile_data[y as usize] >> bit) & 1;
        let plane1 = (tile_data[y as usize + 8] >> bit) & 1;
        let plane2 = (tile_data[y as usize + 16] >> bit) & 1;
        let plane3 = (tile_data[y as usize + 24] >> bit) & 1;

        (plane3 << 3) | (plane2 << 2) | (plane1 << 1) | plane0
    }

    fn tile_size(&self) -> usize {
        32
    }
}

/// Genesis/Mega Drive 4bpp linear tile decoder.
///
/// Each 8x8 tile is stored in 32 bytes with linear 4-bit pixels:
/// - Each pixel is 4 bits (0-15)
/// - Two pixels per byte (upper nibble first)
/// - Pixels are stored row by row, left to right
#[derive(Debug, Clone, Copy)]
pub struct Genesis4BppDecoder;

impl TileDecoder for Genesis4BppDecoder {
    fn decode_pixel(&self, tile_data: &[u8], x: u8, y: u8) -> u8 {
        if tile_data.len() < 32 || x > 7 || y > 7 {
            return 0;
        }

        // Each row is 4 bytes (8 pixels * 4 bits / 8 bits)
        let byte_offset = (y as usize) * 4 + (x as usize) / 2;
        let byte = tile_data[byte_offset];

        // Upper nibble is first pixel (even x), lower nibble is second (odd x)
        if x & 1 == 0 {
            byte >> 4
        } else {
            byte & 0x0F
        }
    }

    fn tile_size(&self) -> usize {
        32
    }
}

/// Get a tile decoder for the specified format.
pub fn get_decoder(format: TileFormat) -> Box<dyn TileDecoder> {
    match format {
        TileFormat::Nes2Bpp => Box::new(Nes2BppDecoder),
        TileFormat::GameBoy2Bpp => Box::new(GameBoy2BppDecoder),
        TileFormat::Snes2Bpp => Box::new(Nes2BppDecoder), // Same as NES
        TileFormat::Snes4Bpp => Box::new(Snes4BppDecoder),
        TileFormat::Genesis4Bpp => Box::new(Genesis4BppDecoder),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nes_decoder_checkerboard() {
        let mut tile_data = vec![0u8; 16];

        // Create a checkerboard pattern
        // Low plane: alternating bits
        tile_data[0] = 0b10101010;
        tile_data[1] = 0b01010101;
        tile_data[2] = 0b10101010;
        tile_data[3] = 0b01010101;
        tile_data[4] = 0b10101010;
        tile_data[5] = 0b01010101;
        tile_data[6] = 0b10101010;
        tile_data[7] = 0b01010101;

        // High plane: solid in top half
        tile_data[8] = 0b11111111;
        tile_data[9] = 0b11111111;
        tile_data[10] = 0b11111111;
        tile_data[11] = 0b11111111;
        tile_data[12] = 0b00000000;
        tile_data[13] = 0b00000000;
        tile_data[14] = 0b00000000;
        tile_data[15] = 0b00000000;

        let decoder = Nes2BppDecoder;

        // Top-left pixel: lo=1, hi=1 = 3
        assert_eq!(decoder.decode_pixel(&tile_data, 0, 0), 3);

        // Second pixel: lo=0, hi=1 = 2
        assert_eq!(decoder.decode_pixel(&tile_data, 1, 0), 2);

        // Bottom-left pixel: lo=1, hi=0 = 1
        assert_eq!(decoder.decode_pixel(&tile_data, 0, 4), 1);

        // Bottom-second pixel: lo=0, hi=0 = 0
        assert_eq!(decoder.decode_pixel(&tile_data, 1, 4), 0);
    }

    #[test]
    fn test_nes_decoder_tile_size() {
        let decoder = Nes2BppDecoder;
        assert_eq!(decoder.tile_size(), 16);
    }

    #[test]
    fn test_nes_decoder_out_of_bounds() {
        let tile_data = vec![0u8; 16];
        let decoder = Nes2BppDecoder;

        // Out of bounds should return 0
        assert_eq!(decoder.decode_pixel(&tile_data, 8, 0), 0);
        assert_eq!(decoder.decode_pixel(&tile_data, 0, 8), 0);
    }

    #[test]
    fn test_gameboy_decoder_checkerboard() {
        let mut tile_data = vec![0u8; 16];

        // Create a simple pattern with interleaved format
        // Row 0: lo=10101010, hi=11111111 -> pixels 3,2,3,2,3,2,3,2
        tile_data[0] = 0b10101010; // Low plane
        tile_data[1] = 0b11111111; // High plane

        // Row 1: lo=01010101, hi=00000000 -> pixels 1,0,1,0,1,0,1,0
        tile_data[2] = 0b01010101; // Low plane
        tile_data[3] = 0b00000000; // High plane

        let decoder = GameBoy2BppDecoder;

        // Row 0, pixel 0: lo=1, hi=1 = 3
        assert_eq!(decoder.decode_pixel(&tile_data, 0, 0), 3);

        // Row 0, pixel 1: lo=0, hi=1 = 2
        assert_eq!(decoder.decode_pixel(&tile_data, 1, 0), 2);

        // Row 1, pixel 0: lo=0, hi=0 = 0
        assert_eq!(decoder.decode_pixel(&tile_data, 0, 1), 0);

        // Row 1, pixel 1: lo=1, hi=0 = 1
        assert_eq!(decoder.decode_pixel(&tile_data, 1, 1), 1);
    }

    #[test]
    fn test_gameboy_decoder_tile_size() {
        let decoder = GameBoy2BppDecoder;
        assert_eq!(decoder.tile_size(), 16);
    }

    #[test]
    fn test_get_decoder() {
        let decoder = get_decoder(TileFormat::Nes2Bpp);
        assert_eq!(decoder.tile_size(), 16);

        let decoder = get_decoder(TileFormat::GameBoy2Bpp);
        assert_eq!(decoder.tile_size(), 16);

        let decoder = get_decoder(TileFormat::Snes4Bpp);
        assert_eq!(decoder.tile_size(), 32);

        let decoder = get_decoder(TileFormat::Genesis4Bpp);
        assert_eq!(decoder.tile_size(), 32);
    }

    #[test]
    fn test_snes4bpp_decoder() {
        let mut tile_data = vec![0u8; 32];

        // Set up a test pattern in all 4 bitplanes
        // Pixel at (0,0): plane0=1, plane1=1, plane2=0, plane3=1 = 0b1011 = 11
        tile_data[0] = 0b10000000; // Plane 0, row 0
        tile_data[8] = 0b10000000; // Plane 1, row 0
        tile_data[16] = 0b00000000; // Plane 2, row 0
        tile_data[24] = 0b10000000; // Plane 3, row 0

        // Pixel at (1,0): plane0=0, plane1=0, plane2=1, plane3=0 = 0b0100 = 4
        tile_data[16] = 0b01000000; // Plane 2, row 0

        let decoder = Snes4BppDecoder;

        // First pixel should be 11 (0b1011)
        assert_eq!(decoder.decode_pixel(&tile_data, 0, 0), 11);

        // Second pixel should be 4 (0b0100)
        assert_eq!(decoder.decode_pixel(&tile_data, 1, 0), 4);
    }

    #[test]
    fn test_snes4bpp_decoder_tile_size() {
        let decoder = Snes4BppDecoder;
        assert_eq!(decoder.tile_size(), 32);
    }

    #[test]
    fn test_genesis4bpp_decoder() {
        let mut tile_data = vec![0u8; 32];

        // Genesis format: 2 pixels per byte, upper nibble first
        // Row 0, pixels 0-1: 0xF3 means pixel 0 = 15, pixel 1 = 3
        tile_data[0] = 0xF3;
        // Row 0, pixels 2-3: 0x5A means pixel 2 = 5, pixel 3 = 10
        tile_data[1] = 0x5A;

        let decoder = Genesis4BppDecoder;

        // First pixel should be 15
        assert_eq!(decoder.decode_pixel(&tile_data, 0, 0), 15);

        // Second pixel should be 3
        assert_eq!(decoder.decode_pixel(&tile_data, 1, 0), 3);

        // Third pixel should be 5
        assert_eq!(decoder.decode_pixel(&tile_data, 2, 0), 5);

        // Fourth pixel should be 10
        assert_eq!(decoder.decode_pixel(&tile_data, 3, 0), 10);
    }

    #[test]
    fn test_genesis4bpp_decoder_tile_size() {
        let decoder = Genesis4BppDecoder;
        assert_eq!(decoder.tile_size(), 32);
    }
}
