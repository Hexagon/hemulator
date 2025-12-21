//! SNES PPU (Picture Processing Unit) - Minimal Implementation
//!
//! This is a minimal stub implementation to demonstrate basic functionality.
//! It supports:
//! - Basic VRAM access via registers $2116-$2119
//! - Basic CGRAM (palette) access via $2121-$2122
//! - Screen enable/disable via $2100
//! - Rendering a simple checkerboard pattern from VRAM
//!
//! NOT implemented (would require extensive work):
//! - Full PPU modes (Mode 0-7)
//! - Sprites (OAM)
//! - Background layers with proper tile mapping
//! - Scrolling
//! - Windows and masks
//! - HDMA effects
//! - Mosaic, color math, etc.

use emu_core::types::Frame;

const VRAM_SIZE: usize = 0x10000; // 64KB VRAM
const CGRAM_SIZE: usize = 512; // 256 colors * 2 bytes per color

/// Minimal SNES PPU implementation
pub struct Ppu {
    /// VRAM (64KB for tiles and tilemaps)
    vram: Vec<u8>,
    /// CGRAM (Color Generator RAM - 512 bytes for 256 colors)
    cgram: Vec<u8>,

    /// VRAM address register ($2116/$2117)
    vram_addr: u16,
    /// CGRAM address register ($2121)
    cgram_addr: u8,
    /// CGRAM write latch (alternates between low and high byte)
    cgram_write_latch: bool,

    /// Screen display register ($2100) - bit 7 = force blank, bits 0-3 = brightness
    screen_display: u8,
}

impl Ppu {
    pub fn new() -> Self {
        Self {
            vram: vec![0; VRAM_SIZE],
            cgram: vec![0; CGRAM_SIZE],
            vram_addr: 0,
            cgram_addr: 0,
            cgram_write_latch: false,
            screen_display: 0x80, // Start with screen blanked
        }
    }

    /// Write to PPU registers
    pub fn write_register(&mut self, addr: u16, val: u8) {
        match addr {
            // $2100 - INIDISP - Screen Display Register
            0x2100 => {
                self.screen_display = val;
            }

            // $2116 - VMADDL - VRAM Address (low byte)
            0x2116 => {
                self.vram_addr = (self.vram_addr & 0xFF00) | val as u16;
            }

            // $2117 - VMADDH - VRAM Address (high byte)
            0x2117 => {
                self.vram_addr = (self.vram_addr & 0x00FF) | ((val as u16) << 8);
            }

            // $2118 - VMDATAL - VRAM Data Write (low byte)
            0x2118 => {
                let addr = (self.vram_addr as usize) % (VRAM_SIZE / 2);
                self.vram[addr * 2] = val;
                // Auto-increment VRAM address
                self.vram_addr = self.vram_addr.wrapping_add(1);
            }

            // $2119 - VMDATAH - VRAM Data Write (high byte)
            0x2119 => {
                // Write to the current address minus 1 (since it was incremented by low byte write)
                let addr = (self.vram_addr.wrapping_sub(1) as usize) % (VRAM_SIZE / 2);
                self.vram[addr * 2 + 1] = val;
            }

            // $2121 - CGADD - CGRAM Address
            0x2121 => {
                self.cgram_addr = val;
                self.cgram_write_latch = false; // Reset write latch
            }

            // $2122 - CGDATA - CGRAM Data Write
            0x2122 => {
                let addr = if self.cgram_write_latch {
                    // High byte
                    (self.cgram_addr as usize * 2 + 1) % CGRAM_SIZE
                } else {
                    // Low byte
                    (self.cgram_addr as usize * 2) % CGRAM_SIZE
                };

                self.cgram[addr] = val;

                // Toggle latch and increment address after high byte
                if self.cgram_write_latch {
                    self.cgram_addr = self.cgram_addr.wrapping_add(1);
                }
                self.cgram_write_latch = !self.cgram_write_latch;
            }

            // Other registers - stub (just accept writes)
            _ => {}
        }
    }

    /// Read from PPU registers
    pub fn read_register(&self, addr: u16) -> u8 {
        match addr {
            // $2100 - INIDISP - Screen Display Register (write-only, return open bus)
            0x2100 => 0,

            // Most PPU registers are write-only
            // For now, return 0 for all reads
            _ => 0,
        }
    }

    /// Render a frame
    pub fn render_frame(&self) -> Frame {
        let mut frame = Frame::new(256, 224); // SNES resolution

        // If screen is blanked, return black frame
        if (self.screen_display & 0x80) != 0 {
            return frame;
        }

        // Very simple rendering: interpret VRAM as a direct color pattern
        // This is a stub - real SNES PPU would decode tiles, tilemaps, etc.
        // For our test ROM, we'll try to detect the tilemap and render it simply

        // The test ROM puts the tilemap at VRAM $0000 (32x32 tiles)
        // and tile data at VRAM $1000
        // Each tile is 8x8 pixels

        for tile_y in 0..28 {
            // 28 tiles vertically (224 pixels / 8)
            for tile_x in 0..32 {
                // 32 tiles horizontally (256 pixels / 8)
                // Read tile index from tilemap
                let tilemap_addr = (tile_y * 32 + tile_x) * 2;
                let tile_index = if tilemap_addr < VRAM_SIZE {
                    self.vram[tilemap_addr]
                } else {
                    0
                };

                // Render the tile
                self.render_tile(&mut frame, tile_x, tile_y, tile_index);
            }
        }

        frame
    }

    /// Render a single 8x8 tile
    fn render_tile(&self, frame: &mut Frame, tile_x: usize, tile_y: usize, tile_index: u8) {
        // Tile data starts at $1000 in VRAM (as configured by test ROM)
        // Each tile is 16 bytes (8 rows * 2 bytes per row for 2-bit color)
        let tile_data_base = 0x1000 + (tile_index as usize * 16);

        for row in 0..8 {
            let pixel_y = tile_y * 8 + row;
            if pixel_y >= 224 {
                break;
            }

            // Read two bitplanes for this row
            let bp0_addr = tile_data_base + row;
            let bp1_addr = tile_data_base + row + 8;

            let bp0 = if bp0_addr < VRAM_SIZE {
                self.vram[bp0_addr]
            } else {
                0
            };
            let bp1 = if bp1_addr < VRAM_SIZE {
                self.vram[bp1_addr]
            } else {
                0
            };

            for col in 0..8 {
                let pixel_x = tile_x * 8 + col;
                if pixel_x >= 256 {
                    break;
                }

                // Extract color index from bitplanes
                let bit = 7 - col;
                let bit0 = (bp0 >> bit) & 1;
                let bit1 = (bp1 >> bit) & 1;
                let color_index = (bit1 << 1) | bit0;

                // Look up color in CGRAM
                let color = self.get_color(color_index);

                // Set pixel in frame
                let frame_offset = pixel_y * 256 + pixel_x;
                frame.pixels[frame_offset] = color;
            }
        }
    }

    /// Get RGB color from CGRAM
    fn get_color(&self, index: u8) -> u32 {
        let addr = (index as usize) * 2;
        if addr + 1 >= CGRAM_SIZE {
            return 0xFF000000; // Black
        }

        // SNES color format: 15-bit BGR (0bbbbbgggggrrrrr)
        let low = self.cgram[addr];
        let high = self.cgram[addr + 1];
        let color15 = (low as u16) | ((high as u16) << 8);

        // Convert from 5-bit per channel to 8-bit per channel
        // Simple shift by 3 (matches test expectations)
        let r = ((color15 & 0x001F) << 3) as u8;
        let g = (((color15 & 0x03E0) >> 5) << 3) as u8;
        let b = (((color15 & 0x7C00) >> 10) << 3) as u8;

        // Return as ARGB (0xAARRGGBB)
        0xFF000000 | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
    }
}

impl Default for Ppu {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ppu_creation() {
        let ppu = Ppu::new();
        assert_eq!(ppu.vram.len(), VRAM_SIZE);
        assert_eq!(ppu.cgram.len(), CGRAM_SIZE);
        assert_eq!(ppu.screen_display, 0x80); // Screen blanked by default
    }

    #[test]
    fn test_vram_write() {
        let mut ppu = Ppu::new();

        // Set VRAM address to $1000
        ppu.write_register(0x2116, 0x00);
        ppu.write_register(0x2117, 0x10);

        // Write data
        ppu.write_register(0x2118, 0xAA);
        ppu.write_register(0x2119, 0xBB);

        // Check that data was written and address incremented
        assert_eq!(ppu.vram[0x1000 * 2], 0xAA);
        assert_eq!(ppu.vram[0x1000 * 2 + 1], 0xBB);
        assert_eq!(ppu.vram_addr, 0x1001); // Incremented after low byte write
    }

    #[test]
    fn test_cgram_write() {
        let mut ppu = Ppu::new();

        // Set CGRAM address to color 1
        ppu.write_register(0x2121, 0x01);

        // Write color (white: $7FFF)
        ppu.write_register(0x2122, 0xFF); // Low byte
        ppu.write_register(0x2122, 0x7F); // High byte

        // Check that color was written
        assert_eq!(ppu.cgram[2], 0xFF);
        assert_eq!(ppu.cgram[3], 0x7F);
        assert_eq!(ppu.cgram_addr, 0x02); // Incremented
    }

    #[test]
    fn test_screen_blank() {
        let mut ppu = Ppu::new();

        // Screen starts blanked
        let frame = ppu.render_frame();
        assert_eq!(frame.width, 256);
        assert_eq!(frame.height, 224);
        // All pixels should be black
        assert!(frame.pixels.iter().all(|&p| p == 0));

        // Enable screen
        ppu.write_register(0x2100, 0x0F); // Brightness 15, not blanked

        // Frame should still be black (no data) but rendering is enabled
        let frame2 = ppu.render_frame();
        assert_eq!(frame2.width, 256);
        assert_eq!(frame2.height, 224);
    }

    #[test]
    fn test_color_conversion() {
        let mut ppu = Ppu::new();

        // Set up some test colors
        ppu.cgram[0] = 0x00; // Color 0: Black ($0000)
        ppu.cgram[1] = 0x00;

        ppu.cgram[2] = 0xFF; // Color 1: White ($7FFF)
        ppu.cgram[3] = 0x7F;

        ppu.cgram[4] = 0x1F; // Color 2: Red ($001F)
        ppu.cgram[5] = 0x00;

        ppu.cgram[6] = 0x00; // Color 3: Blue ($7C00)
        ppu.cgram[7] = 0x7C;

        assert_eq!(ppu.get_color(0), 0xFF000000); // Black
        assert_eq!(ppu.get_color(1), 0xFFF8F8F8); // White (5-bit max = 0xF8 in 8-bit)
        assert_eq!(ppu.get_color(2), 0xFFF80000); // Red (5-bit max = 0xF8 in 8-bit)
        assert_eq!(ppu.get_color(3), 0xFF0000F8); // Blue (5-bit max = 0xF8 in 8-bit)
    }
}
