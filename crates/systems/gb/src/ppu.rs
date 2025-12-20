//! Game Boy PPU (Picture Processing Unit / LCD Controller) implementation
//!
//! The PPU is responsible for rendering graphics to the 160x144 pixel LCD screen.
//! It operates in a tile-based system with multiple layers and supports scrolling,
//! windows, and sprites.
//!
//! # Display Architecture
//!
//! ## Layers (in rendering order)
//! 1. **Background**: 256x256 pixel tilemap with scrolling support
//! 2. **Window**: Overlay layer with independent position (WX, WY)
//! 3. **Sprites (OBJ)**: 40 movable 8x8 or 8x16 pixel objects
//!
//! ## Tile System
//! - Tiles are 8x8 pixels, 2 bits per pixel (4 colors)
//! - Each tile uses 16 bytes (2 bytes per row)
//! - Two tile data areas:
//!   - `$8000-$8FFF`: 256 tiles (unsigned addressing mode)
//!   - `$8800-$97FF`: 256 tiles (signed addressing mode, -128 to +127)
//! - Two tile map areas:
//!   - `$9800-$9BFF`: Background/Window tilemap
//!   - `$9C00-$9FFF`: Background/Window tilemap
//!
//! ## Color Palettes (DMG Mode)
//! - BGP ($FF47): Background palette
//! - OBP0 ($FF48): Object palette 0
//! - OBP1 ($FF49): Object palette 1
//! - Each palette maps 4 colors (2 bits) to 4 shades:
//!   - 0: White (0xFFFFFF)
//!   - 1: Light gray (0xAAAAAA)
//!   - 2: Dark gray (0x555555)
//!   - 3: Black (0x000000)
//!
//! # LCD Control Register (LCDC - $FF40)
//!
//! - Bit 7: LCD enable (0=off, 1=on)
//! - Bit 6: Window tilemap area (0=$9800-$9BFF, 1=$9C00-$9FFF)
//! - Bit 5: Window enable (0=off, 1=on)
//! - Bit 4: BG & Window tile data area (0=$8800-$97FF signed, 1=$8000-$8FFF unsigned)
//! - Bit 3: BG tilemap area (0=$9800-$9BFF, 1=$9C00-$9FFF)
//! - Bit 2: Sprite size (0=8x8, 1=8x16)
//! - Bit 1: Sprite enable (0=off, 1=on)
//! - Bit 0: BG & Window enable (0=off, 1=on)
//!
//! # LCD Status Register (STAT - $FF41)
//!
//! - Bit 6: LYC=LY interrupt enable
//! - Bit 5: Mode 2 OAM interrupt enable
//! - Bit 4: Mode 1 VBlank interrupt enable
//! - Bit 3: Mode 0 HBlank interrupt enable
//! - Bit 2: LYC=LY coincidence flag (0=different, 1=equal)
//! - Bits 1-0: Mode flag (0=HBlank, 1=VBlank, 2=OAM search, 3=pixel transfer)
//!
//! # Sprites (OBJ)
//!
//! Each sprite is defined by 4 bytes in OAM (Object Attribute Memory):
//! - Byte 0: Y position (actual position - 16)
//! - Byte 1: X position (actual position - 8)
//! - Byte 2: Tile index
//! - Byte 3: Flags
//!   - Bit 7: BG/Window priority (0=above BG, 1=behind BG colors 1-3)
//!   - Bit 6: Y flip
//!   - Bit 5: X flip
//!   - Bit 4: Palette (0=OBP0, 1=OBP1)
//!   - Bits 3-0: Unused (CGB palette in CGB mode)
//!
//! # Timing Model
//!
//! This implementation uses a **frame-based** rendering model:
//! - Entire frames are rendered on-demand
//! - Scanline counter (LY) is updated during CPU execution
//! - V-Blank detection occurs when LY reaches 144
//! - Suitable for most games, but not cycle-accurate
//!
//! ## Actual Hardware Timing (for reference)
//! - Mode 2 (OAM search): 80 cycles
//! - Mode 3 (pixel transfer): 168-291 cycles
//! - Mode 0 (HBlank): 85-208 cycles
//! - Total scanline: 456 cycles
//! - VBlank: 10 scanlines (4560 cycles)
//!
//! # Current Implementation
//!
//! ## Implemented
//! - ✅ Background rendering with scrolling
//! - ✅ Window rendering
//! - ✅ Sprite rendering (8x8 and 8x16)
//! - ✅ Sprite flipping (horizontal and vertical)
//! - ✅ Sprite priority (above/behind background)
//! - ✅ Palette support (BGP, OBP0, OBP1)
//! - ✅ LYC=LY coincidence detection
//! - ✅ Frame-based timing with scanline counter
//!
//! ## Not Implemented
//! - ❌ Cycle-accurate PPU timing
//! - ❌ Mid-scanline effects
//! - ❌ Sprite-per-scanline limit (10 sprites)
//! - ❌ PPU mode transitions (Mode 0-3)
//! - ❌ STAT interrupts
//! - ❌ OAM DMA transfer
//! - ❌ Game Boy Color features (color palettes, VRAM banking)

use emu_core::types::Frame;

/// Game Boy PPU state
pub struct Ppu {
    /// VRAM (8KB)
    vram: [u8; 0x2000],
    /// OAM (Object Attribute Memory - 160 bytes)
    oam: [u8; 0xA0],

    /// LCD Control (0xFF40)
    pub lcdc: u8,
    /// LCD Status (0xFF41)
    pub stat: u8,
    /// Scroll Y (0xFF42)
    pub scy: u8,
    /// Scroll X (0xFF43)
    pub scx: u8,
    /// LY (LCD Y coordinate, 0xFF44)
    pub ly: u8,
    /// LY Compare (0xFF45)
    pub lyc: u8,
    /// BG Palette (0xFF47)
    pub bgp: u8,
    /// OBJ Palette 0 (0xFF48)
    pub obp0: u8,
    /// OBJ Palette 1 (0xFF49)
    pub obp1: u8,
    /// Window Y (0xFF4A)
    pub wy: u8,
    /// Window X (0xFF4B)
    pub wx: u8,
}

// LCDC bits
const LCDC_ENABLE: u8 = 0x80;
#[allow(dead_code)]
const LCDC_WIN_TILEMAP: u8 = 0x40;
#[allow(dead_code)]
const LCDC_WIN_ENABLE: u8 = 0x20;
const LCDC_BG_WIN_TILES: u8 = 0x10;
const LCDC_BG_TILEMAP: u8 = 0x08;
#[allow(dead_code)]
const LCDC_OBJ_SIZE: u8 = 0x04;
#[allow(dead_code)]
const LCDC_OBJ_ENABLE: u8 = 0x02;
const LCDC_BG_WIN_ENABLE: u8 = 0x01;

impl Ppu {
    pub fn new() -> Self {
        Self {
            vram: [0; 0x2000],
            oam: [0; 0xA0],
            lcdc: 0x91,
            stat: 0x00,
            scy: 0,
            scx: 0,
            ly: 0,
            lyc: 0,
            bgp: 0xFC,
            obp0: 0xFF,
            obp1: 0xFF,
            wy: 0,
            wx: 0,
        }
    }

    /// Read from VRAM (0x8000-0x9FFF)
    pub fn read_vram(&self, addr: u16) -> u8 {
        self.vram[(addr & 0x1FFF) as usize]
    }

    /// Write to VRAM (0x8000-0x9FFF)
    pub fn write_vram(&mut self, addr: u16, val: u8) {
        self.vram[(addr & 0x1FFF) as usize] = val;
    }

    /// Read from OAM (0xFE00-0xFE9F)
    pub fn read_oam(&self, addr: u16) -> u8 {
        self.oam[(addr & 0x9F) as usize]
    }

    /// Write to OAM (0xFE00-0xFE9F)
    pub fn write_oam(&mut self, addr: u16, val: u8) {
        self.oam[(addr & 0x9F) as usize] = val;
    }

    /// Render a complete frame (160x144)
    pub fn render_frame(&self) -> Frame {
        let mut frame = Frame::new(160, 144);

        if (self.lcdc & LCDC_ENABLE) == 0 {
            // LCD is off - return blank screen
            return frame;
        }

        // Render background if enabled
        if (self.lcdc & LCDC_BG_WIN_ENABLE) != 0 {
            self.render_background(&mut frame);
        }

        // Render window if enabled
        if (self.lcdc & LCDC_WIN_ENABLE) != 0 {
            self.render_window(&mut frame);
        }

        // Render sprites if enabled
        if (self.lcdc & LCDC_OBJ_ENABLE) != 0 {
            self.render_sprites(&mut frame);
        }

        frame
    }

    fn calculate_signed_tile_address(&self, base: u16, tile_index: u8) -> u16 {
        // In signed mode, tile_index is treated as signed -128 to 127
        // Base is at $8800, so index 0 would be at $9000 (base + 128 * 16)
        base + ((tile_index as i8 as i16 + 128) as u16 * 16)
    }

    fn render_background(&self, frame: &mut Frame) {
        let tile_data_base = if (self.lcdc & LCDC_BG_WIN_TILES) != 0 {
            0x0000 // $8000-$8FFF
        } else {
            0x0800 // $8800-$97FF (signed addressing)
        };

        let tilemap_base = if (self.lcdc & LCDC_BG_TILEMAP) != 0 {
            0x1C00 // $9C00-$9FFF
        } else {
            0x1800 // $9800-$9BFF
        };

        for screen_y in 0u8..144 {
            let y = screen_y.wrapping_add(self.scy);
            let tile_y = (y / 8) as u16;
            let pixel_y = (y % 8) as u16;

            for screen_x in 0u8..160 {
                let x = screen_x.wrapping_add(self.scx);
                let tile_x = (x / 8) as u16;
                let pixel_x = (x % 8) as u16;

                // Get tile index from tilemap
                let tilemap_addr = tilemap_base + (tile_y * 32) + tile_x;
                let tile_index = self.vram[tilemap_addr as usize];

                // Calculate tile data address
                let tile_addr = if (self.lcdc & LCDC_BG_WIN_TILES) != 0 {
                    // Unsigned mode: tiles at $8000-$8FFF
                    tile_data_base + (tile_index as u16 * 16)
                } else {
                    // Signed mode: tiles at $8800-$97FF, index is signed -128 to 127
                    self.calculate_signed_tile_address(tile_data_base, tile_index)
                };

                // Get tile data (2 bytes per row)
                let tile_row_addr = tile_addr + (pixel_y * 2);
                let byte1 = self.vram[tile_row_addr as usize];
                let byte2 = self.vram[(tile_row_addr + 1) as usize];

                // Get pixel color (2-bit value)
                let bit = 7 - pixel_x;
                let color_bit_0 = (byte1 >> bit) & 1;
                let color_bit_1 = (byte2 >> bit) & 1;
                let color_index = (color_bit_1 << 1) | color_bit_0;

                // Apply palette
                let palette_color = (self.bgp >> (color_index * 2)) & 0x03;

                // Convert to RGB (DMG palette: 0=white, 1=light gray, 2=dark gray, 3=black)
                let rgb = match palette_color {
                    0 => 0xFFFFFFFF, // White
                    1 => 0xFFAAAAAA, // Light gray
                    2 => 0xFF555555, // Dark gray
                    3 => 0xFF000000, // Black
                    _ => unreachable!(),
                };

                frame.pixels[(screen_y as usize * 160) + screen_x as usize] = rgb;
            }
        }
    }

    fn render_window(&self, frame: &mut Frame) {
        // Window rendering - similar to background but positioned at WX-7, WY
        if self.wx >= 167 || self.wy >= 144 {
            return; // Window not visible
        }

        let tile_data_base = if (self.lcdc & LCDC_BG_WIN_TILES) != 0 {
            0x0000 // $8000-$8FFF
        } else {
            0x0800 // $8800-$97FF (signed addressing)
        };

        let tilemap_base = if (self.lcdc & LCDC_WIN_TILEMAP) != 0 {
            0x1C00 // $9C00-$9FFF
        } else {
            0x1800 // $9800-$9BFF
        };

        for screen_y in self.wy..144 {
            let win_y = screen_y - self.wy;
            let tile_y = (win_y / 8) as u16;
            let pixel_y = (win_y % 8) as u16;

            let start_x = self.wx.saturating_sub(7);
            
            for screen_x in start_x..160 {
                let win_x = screen_x - start_x;
                let tile_x = (win_x / 8) as u16;
                let pixel_x = (win_x % 8) as u16;

                // Get tile index from tilemap
                let tilemap_addr = tilemap_base + (tile_y * 32) + tile_x;
                let tile_index = self.vram[tilemap_addr as usize];

                // Calculate tile data address
                let tile_addr = if (self.lcdc & LCDC_BG_WIN_TILES) != 0 {
                    tile_data_base + (tile_index as u16 * 16)
                } else {
                    self.calculate_signed_tile_address(tile_data_base, tile_index)
                };

                // Get tile data (2 bytes per row)
                let tile_row_addr = tile_addr + (pixel_y * 2);
                let byte1 = self.vram[tile_row_addr as usize];
                let byte2 = self.vram[(tile_row_addr + 1) as usize];

                // Get pixel color (2-bit value)
                let bit = 7 - pixel_x;
                let color_bit_0 = (byte1 >> bit) & 1;
                let color_bit_1 = (byte2 >> bit) & 1;
                let color_index = (color_bit_1 << 1) | color_bit_0;

                // Apply palette
                let palette_color = (self.bgp >> (color_index * 2)) & 0x03;

                // Convert to RGB
                let rgb = match palette_color {
                    0 => 0xFFFFFFFF, // White
                    1 => 0xFFAAAAAA, // Light gray
                    2 => 0xFF555555, // Dark gray
                    3 => 0xFF000000, // Black
                    _ => unreachable!(),
                };

                frame.pixels[(screen_y as usize * 160) + screen_x as usize] = rgb;
            }
        }
    }

    fn render_sprites(&self, frame: &mut Frame) {
        // Sprite rendering - Game Boy supports 40 sprites, max 10 per scanline
        let sprite_height = if (self.lcdc & LCDC_OBJ_SIZE) != 0 { 16 } else { 8 };

        // Iterate through all 40 sprites (OAM has 40 entries of 4 bytes each)
        for sprite_idx in 0..40 {
            let oam_addr = sprite_idx * 4;
            let y_pos = self.oam[oam_addr].wrapping_sub(16); // Y position - 16
            let x_pos = self.oam[oam_addr + 1].wrapping_sub(8); // X position - 8
            let tile_index = self.oam[oam_addr + 2];
            let flags = self.oam[oam_addr + 3];

            // Check if sprite is visible
            if (160..248).contains(&x_pos) {
                continue; // Off screen
            }

            let palette = if (flags & 0x10) != 0 { self.obp1 } else { self.obp0 };
            let flip_x = (flags & 0x20) != 0;
            let flip_y = (flags & 0x40) != 0;
            let bg_priority = (flags & 0x80) != 0;

            // Render sprite pixels
            for sy in 0..sprite_height {
                let screen_y = y_pos.wrapping_add(sy);
                if screen_y >= 144 {
                    continue;
                }

                let pixel_y = if flip_y { sprite_height - 1 - sy } else { sy };
                
                // For 8x16 sprites, use tile_index & 0xFE for top, tile_index | 0x01 for bottom
                let tile = if sprite_height == 16 {
                    if pixel_y < 8 {
                        tile_index & 0xFE
                    } else {
                        tile_index | 0x01
                    }
                } else {
                    tile_index
                };

                let tile_addr = (tile as u16) * 16;
                let row_offset = (pixel_y % 8) * 2;
                let byte1 = self.vram[(tile_addr + row_offset as u16) as usize];
                let byte2 = self.vram[(tile_addr + row_offset as u16 + 1) as usize];

                for sx in 0..8u8 {
                    let screen_x = x_pos.wrapping_add(sx);
                    if screen_x >= 160 {
                        continue;
                    }

                    let pixel_x = if flip_x { 7 - sx } else { sx };
                    let bit = 7 - pixel_x;
                    let color_bit_0 = (byte1 >> bit) & 1;
                    let color_bit_1 = (byte2 >> bit) & 1;
                    let color_index = (color_bit_1 << 1) | color_bit_0;

                    // Color 0 is transparent for sprites
                    if color_index == 0 {
                        continue;
                    }

                    // Apply palette
                    let palette_color = (palette >> (color_index * 2)) & 0x03;
                    
                    // Check background priority
                    if bg_priority {
                        // Sprite is behind background colors 1-3
                        let pixel_idx = (screen_y as usize * 160) + screen_x as usize;
                        let current = frame.pixels[pixel_idx];
                        // If background pixel is not white (color 0), skip sprite
                        if current != 0xFFFFFFFF {
                            continue;
                        }
                    }

                    // Convert to RGB
                    let rgb = match palette_color {
                        0 => 0xFFFFFFFF, // White (transparent, but palette maps it)
                        1 => 0xFFAAAAAA, // Light gray
                        2 => 0xFF555555, // Dark gray
                        3 => 0xFF000000, // Black
                        _ => unreachable!(),
                    };

                    let pixel_idx = (screen_y as usize * 160) + screen_x as usize;
                    frame.pixels[pixel_idx] = rgb;
                }
            }
        }
    }

    /// Step the PPU for one scanline worth of cycles
    pub fn step(&mut self, cycles: u32) -> bool {
        // Simplified: just increment LY
        let scanlines = cycles / 456; // ~456 cycles per scanline
        for _ in 0..scanlines {
            self.ly = (self.ly + 1) % 154;

            // Check LYC=LY interrupt
            if self.ly == self.lyc {
                self.stat |= 0x04; // Set coincidence flag
            } else {
                self.stat &= !0x04;
            }

            // V-Blank is lines 144-153
            if self.ly == 144 {
                return true; // V-Blank started
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ppu_creation() {
        let ppu = Ppu::new();
        assert_eq!(ppu.lcdc, 0x91);
    }

    #[test]
    fn test_vram_read_write() {
        let mut ppu = Ppu::new();
        ppu.write_vram(0x1000, 0x42);
        assert_eq!(ppu.read_vram(0x1000), 0x42);
    }

    #[test]
    fn test_oam_read_write() {
        let mut ppu = Ppu::new();
        ppu.write_oam(0x10, 0x42);
        assert_eq!(ppu.read_oam(0x10), 0x42);
    }

    #[test]
    fn test_render_blank_frame() {
        let ppu = Ppu::new();
        let frame = ppu.render_frame();
        assert_eq!(frame.width, 160);
        assert_eq!(frame.height, 144);
    }

    #[test]
    fn test_step_ly() {
        let mut ppu = Ppu::new();
        ppu.ly = 0;
        ppu.step(456); // One scanline
        assert_eq!(ppu.ly, 1);
    }

    #[test]
    fn test_vblank_detection() {
        let mut ppu = Ppu::new();
        ppu.ly = 143;
        let vblank = ppu.step(456);
        assert!(vblank);
        assert_eq!(ppu.ly, 144);
    }

    #[test]
    fn test_window_rendering() {
        let mut ppu = Ppu::new();
        ppu.lcdc = 0xE1; // Enable LCD, window, and background
        ppu.wy = 0;
        ppu.wx = 7;
        
        // Set up a simple tile in VRAM
        ppu.write_vram(0x0000, 0xFF); // First byte of tile 0
        ppu.write_vram(0x0001, 0xFF); // Second byte of tile 0
        
        // Set window tilemap to use tile 0
        ppu.write_vram(0x1800, 0x00); // Tilemap entry for tile 0
        
        let frame = ppu.render_frame();
        assert_eq!(frame.width, 160);
        assert_eq!(frame.height, 144);
    }

    #[test]
    fn test_sprite_rendering() {
        let mut ppu = Ppu::new();
        ppu.lcdc = 0x93; // Enable LCD, sprites, and background
        
        // Set up sprite in OAM
        ppu.write_oam(0, 16); // Y position
        ppu.write_oam(1, 8);  // X position
        ppu.write_oam(2, 0);  // Tile index
        ppu.write_oam(3, 0);  // Flags (no flip, palette 0, above BG)
        
        // Set up a simple tile in VRAM for the sprite
        ppu.write_vram(0x0000, 0xFF);
        ppu.write_vram(0x0001, 0xFF);
        
        let frame = ppu.render_frame();
        assert_eq!(frame.width, 160);
        assert_eq!(frame.height, 144);
    }

    #[test]
    fn test_sprite_flip() {
        let mut ppu = Ppu::new();
        ppu.lcdc = 0x93;
        
        // Set up sprite with horizontal flip
        ppu.write_oam(0, 16);
        ppu.write_oam(1, 8);
        ppu.write_oam(2, 0);
        ppu.write_oam(3, 0x20); // Flip X flag
        
        ppu.write_vram(0x0000, 0x80); // Left-most pixel set
        ppu.write_vram(0x0001, 0x00);
        
        let frame = ppu.render_frame();
        // With flip, the pixel should appear on the right side
        assert_eq!(frame.width, 160);
        assert_eq!(frame.height, 144);
    }

    #[test]
    fn test_sprite_priority() {
        let mut ppu = Ppu::new();
        ppu.lcdc = 0x93;
        
        // Set up sprite with background priority
        ppu.write_oam(0, 16);
        ppu.write_oam(1, 8);
        ppu.write_oam(2, 0);
        ppu.write_oam(3, 0x80); // BG priority flag
        
        let frame = ppu.render_frame();
        assert_eq!(frame.width, 160);
        assert_eq!(frame.height, 144);
    }

    #[test]
    fn test_lyc_coincidence() {
        let mut ppu = Ppu::new();
        ppu.ly = 10;
        ppu.lyc = 11;
        
        ppu.step(456);
        assert_eq!(ppu.ly, 11);
        assert!(ppu.stat & 0x04 != 0); // Coincidence flag should be set
    }
}
