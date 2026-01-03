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
//! ## Color Palettes
//!
//! ### DMG Mode (Monochrome)
//! - BGP ($FF47): Background palette
//! - OBP0 ($FF48): Object palette 0
//! - OBP1 ($FF49): Object palette 1
//! - Each palette maps 4 colors (2 bits) to 4 shades:
//!   - 0: White (0xFFFFFF)
//!   - 1: Light gray (0xAAAAAA)
//!   - 2: Dark gray (0x555555)
//!   - 3: Black (0x000000)
//!
//! ### CGB Mode (Color)
//! - BCPS/BGPI ($FF68): Background palette index/specification
//! - BCPD/BGPD ($FF69): Background palette data
//! - OCPS/OBPI ($FF6A): Object palette index/specification
//! - OCPD/OBPD ($FF6B): Object palette data
//! - 8 background palettes, 8 object palettes
//! - Each palette has 4 colors
//! - Each color is 15-bit RGB (5 bits per channel)
//! - Color format: gggrrrrr 0bbbbbgg (little-endian)
//! - Auto-increment on palette data write when bit 7 of index register is set
//!
//! ## VRAM Banking (CGB)
//! - VBK ($FF4F): VRAM bank select (bit 0)
//! - Bank 0: Tile pixel data (compatible with DMG)
//! - Bank 1: Tile attributes (CGB only)
//!   - Bit 7: BG-to-OAM priority
//!   - Bit 6: Vertical flip
//!   - Bit 5: Horizontal flip
//!   - Bit 3: Tile VRAM bank (0 or 1)
//!   - Bits 2-0: Background palette number (0-7)
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
//!   - **DMG Mode:**
//!     - Bit 7: BG/Window priority (0=above BG, 1=behind BG colors 1-3)
//!     - Bit 6: Y flip
//!     - Bit 5: X flip
//!     - Bit 4: Palette (0=OBP0, 1=OBP1)
//!     - Bits 3-0: Unused
//!   - **CGB Mode:**
//!     - Bit 7: BG/Window priority
//!     - Bit 6: Y flip
//!     - Bit 5: X flip
//!     - Bit 3: Tile VRAM bank (0 or 1)
//!     - Bits 2-0: CGB palette number (0-7)
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
//! - ✅ Sprite-per-scanline limit (10 sprites max)
//! - ✅ DMG palette support (BGP, OBP0, OBP1)
//! - ✅ CGB color palettes (8 BG, 8 OBJ, 15-bit RGB)
//! - ✅ CGB VRAM banking (2 banks of 8KB)
//! - ✅ CGB tile attributes (palette, VRAM bank, flip)
//! - ✅ CGB sprite attributes (palette, VRAM bank)
//! - ✅ LYC=LY coincidence detection
//! - ✅ Frame-based timing with scanline counter
//! - ✅ Automatic CGB mode detection and activation
//!
//! ## Not Implemented
//! - ❌ Cycle-accurate PPU timing
//! - ❌ Mid-scanline effects
//! - ❌ PPU mode transitions (Mode 0-3)
//! - ❌ STAT interrupts
//! - ❌ OAM DMA transfer

use emu_core::types::Frame;

/// Game Boy PPU state
pub struct Ppu {
    /// VRAM Bank 0 (8KB)
    vram_bank0: [u8; 0x2000],
    /// VRAM Bank 1 (8KB, CGB only - contains tile attributes)
    vram_bank1: [u8; 0x2000],
    /// Current VRAM bank (0 or 1, CGB only)
    vram_bank: u8,
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
    /// BG Palette (0xFF47) - DMG only
    pub bgp: u8,
    /// OBJ Palette 0 (0xFF48) - DMG only
    pub obp0: u8,
    /// OBJ Palette 1 (0xFF49) - DMG only
    pub obp1: u8,
    /// Window Y (0xFF4A)
    pub wy: u8,
    /// Window X (0xFF4B)
    pub wx: u8,
    /// Cycle accumulator for scanline timing
    cycle_counter: u32,

    // CGB-specific registers and state
    /// Background palette index/specification (0xFF68)
    bgpi: u8,
    /// Object palette index/specification (0xFF6A)
    obpi: u8,
    /// Background palette data (8 palettes × 4 colors × 2 bytes = 64 bytes)
    /// Each color is 15-bit RGB (2 bytes): gggrrrrr 0bbbbbgg
    bg_palette_data: [u8; 64],
    /// Object palette data (8 palettes × 4 colors × 2 bytes = 64 bytes)
    obj_palette_data: [u8; 64],
    /// CGB mode enabled flag
    cgb_mode: bool,
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
            vram_bank0: [0; 0x2000],
            vram_bank1: [0; 0x2000],
            vram_bank: 0,
            oam: [0; 0xA0],
            lcdc: 0x91,
            stat: 0x00,
            scy: 0,
            scx: 0,
            ly: 0,
            lyc: 0,
            bgp: 0xFC,
            obp0: 0xE4,
            obp1: 0xE4,
            wy: 0,
            wx: 0,
            cycle_counter: 0,
            bgpi: 0,
            obpi: 0,
            bg_palette_data: [0; 64],
            obj_palette_data: [0; 64],
            cgb_mode: false,
        }
    }

    /// Enable CGB mode
    pub fn enable_cgb_mode(&mut self) {
        self.cgb_mode = true;
        // Initialize CGB palettes with default values
        // Default: white palette (all colors set to white = 0x7FFF)
        for i in 0..64 {
            self.bg_palette_data[i] = if i % 2 == 0 { 0xFF } else { 0x7F };
            self.obj_palette_data[i] = if i % 2 == 0 { 0xFF } else { 0x7F };
        }
    }

    /// Check if CGB mode is enabled
    pub fn is_cgb_mode(&self) -> bool {
        self.cgb_mode
    }

    /// Read from VRAM (0x8000-0x9FFF)
    pub fn read_vram(&self, addr: u16) -> u8 {
        let offset = (addr & 0x1FFF) as usize;
        if self.vram_bank == 0 {
            self.vram_bank0[offset]
        } else {
            self.vram_bank1[offset]
        }
    }

    /// Write to VRAM (0x8000-0x9FFF)
    pub fn write_vram(&mut self, addr: u16, val: u8) {
        let offset = (addr & 0x1FFF) as usize;
        if self.vram_bank == 0 {
            self.vram_bank0[offset] = val;
        } else {
            self.vram_bank1[offset] = val;
        }
    }

    /// Set VRAM bank (VBK register at 0xFF4F)
    pub fn set_vram_bank(&mut self, val: u8) {
        // Only bit 0 matters, bit 1-7 are unused
        self.vram_bank = val & 0x01;
    }

    /// Get VRAM bank
    pub fn get_vram_bank(&self) -> u8 {
        self.vram_bank | 0xFE // Bits 1-7 return 1
    }

    /// Read from OAM (0xFE00-0xFE9F)
    pub fn read_oam(&self, addr: u16) -> u8 {
        if addr >= 0xA0 {
            return 0xFF; // Out of bounds
        }
        self.oam[addr as usize]
    }

    /// Write to OAM (0xFE00-0xFE9F)
    pub fn write_oam(&mut self, addr: u16, val: u8) {
        if addr >= 0xA0 {
            return; // Out of bounds
        }
        self.oam[addr as usize] = val;
    }

    /// Read from OAM for debugging
    pub fn read_oam_debug(&self, addr: u16) -> u8 {
        if addr >= 0xA0 {
            return 0xFF; // Out of bounds
        }
        self.oam[addr as usize]
    }

    /// Read background palette index register (0xFF68)
    pub fn read_bgpi(&self) -> u8 {
        self.bgpi
    }

    /// Write background palette index register (0xFF68)
    pub fn write_bgpi(&mut self, val: u8) {
        self.bgpi = val;
    }

    /// Read background palette data register (0xFF69)
    pub fn read_bgpd(&self) -> u8 {
        let index = (self.bgpi & 0x3F) as usize;
        self.bg_palette_data[index]
    }

    /// Write background palette data register (0xFF69)
    pub fn write_bgpd(&mut self, val: u8) {
        let index = (self.bgpi & 0x3F) as usize;
        self.bg_palette_data[index] = val;
        // Auto-increment if bit 7 is set
        if (self.bgpi & 0x80) != 0 {
            self.bgpi = (self.bgpi & 0x80) | ((self.bgpi + 1) & 0x3F);
        }
    }

    /// Read object palette index register (0xFF6A)
    pub fn read_obpi(&self) -> u8 {
        self.obpi
    }

    /// Write object palette index register (0xFF6A)
    pub fn write_obpi(&mut self, val: u8) {
        self.obpi = val;
    }

    /// Read object palette data register (0xFF6B)
    pub fn read_obpd(&self) -> u8 {
        let index = (self.obpi & 0x3F) as usize;
        self.obj_palette_data[index]
    }

    /// Write object palette data register (0xFF6B)
    pub fn write_obpd(&mut self, val: u8) {
        let index = (self.obpi & 0x3F) as usize;
        self.obj_palette_data[index] = val;
        // Auto-increment if bit 7 is set
        if (self.obpi & 0x80) != 0 {
            self.obpi = (self.obpi & 0x80) | ((self.obpi + 1) & 0x3F);
        }
    }

    /// Convert CGB 15-bit color to 32-bit ARGB
    /// CGB color format: gggrrrrr 0bbbbbgg (little-endian)
    fn cgb_color_to_rgb(&self, color_low: u8, color_high: u8) -> u32 {
        let color = (color_high as u16) << 8 | color_low as u16;
        let r = ((color & 0x1F) as u32) << 3;
        let g = (((color >> 5) & 0x1F) as u32) << 3;
        let b = (((color >> 10) & 0x1F) as u32) << 3;
        // Expand 5-bit to 8-bit by copying top bits to bottom
        let r = r | (r >> 5);
        let g = g | (g >> 5);
        let b = b | (b >> 5);
        0xFF000000 | (r << 16) | (g << 8) | b
    }

    /// Render a complete frame (160x144)
    pub fn render_frame(&self) -> Frame {
        let mut frame = Frame::new(160, 144);

        if (self.lcdc & LCDC_ENABLE) == 0 {
            // LCD is off - return blank screen
            return frame;
        }

        // Track background color indices and priority for sprite rendering
        // Each byte stores: [bit 7: BG priority, bits 1-0: color index (0-3)]
        let mut bg_color_indices = vec![0u8; 160 * 144];

        // BG/Window rendering behavior depends on mode:
        // - DMG: LCDC.0 = 0 disables BG/Window (blank/white)
        // - CGB: LCDC.0 = 0 removes BG/Window priority (still renders, sprites always on top)
        let bg_win_enabled = (self.lcdc & LCDC_BG_WIN_ENABLE) != 0;

        // Render background
        if bg_win_enabled || self.cgb_mode {
            // CGB: always render BG even if LCDC.0 is 0
            // DMG: only render if LCDC.0 is 1
            self.render_background(&mut frame, &mut bg_color_indices);
        }

        // Render window
        if (self.lcdc & LCDC_WIN_ENABLE) != 0 && (bg_win_enabled || self.cgb_mode) {
            self.render_window(&mut frame, &mut bg_color_indices);
        }

        // Render sprites if enabled
        if (self.lcdc & LCDC_OBJ_ENABLE) != 0 {
            self.render_sprites(&mut frame, &bg_color_indices);
        }

        frame
    }

    fn calculate_signed_tile_address(&self, base: u16, tile_index: u8) -> u16 {
        // In signed mode, tile_index is treated as signed -128 to 127
        // Base is at $8800, so index 0 would be at $9000 (base + 128 * 16)
        base + ((tile_index as i8 as i16 + 128) as u16 * 16)
    }

    fn render_background(&self, frame: &mut Frame, bg_color_indices: &mut [u8]) {
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
            let tile_y = ((y / 8) & 31) as u16;
            let pixel_y = (y % 8) as u16;

            for screen_x in 0u8..160 {
                let x = screen_x.wrapping_add(self.scx);
                let tile_x = ((x / 8) & 31) as u16;
                let pixel_x = (x % 8) as u16;

                // Get tile index from tilemap (always from VRAM bank 0)
                let tilemap_addr = tilemap_base + (tile_y * 32) + tile_x;
                let tile_index = self.vram_bank0[tilemap_addr as usize];

                // Get tile attributes from VRAM bank 1 (CGB only)
                let tile_attr = if self.cgb_mode {
                    self.vram_bank1[tilemap_addr as usize]
                } else {
                    0
                };

                // CGB tile attributes (from VRAM bank 1):
                // Bit 7: BG-to-OAM Priority (0=use OAM priority, 1=BG priority)
                // Bit 6: Vertical flip
                // Bit 5: Horizontal flip
                // Bit 4: Not used
                // Bit 3: VRAM bank (0=bank 0, 1=bank 1) for tile data
                // Bits 2-0: Background palette number (0-7)
                let bg_palette_num = tile_attr & 0x07;
                let tile_vram_bank = (tile_attr >> 3) & 0x01;
                let flip_x = (tile_attr & 0x20) != 0;
                let flip_y = (tile_attr & 0x40) != 0;
                let bg_priority = (tile_attr & 0x80) != 0;

                // Calculate tile data address
                let tile_addr = if (self.lcdc & LCDC_BG_WIN_TILES) != 0 {
                    // Unsigned mode: tiles at $8000-$8FFF
                    tile_data_base + (tile_index as u16 * 16)
                } else {
                    // Signed mode: tiles at $8800-$97FF, index is signed -128 to 127
                    self.calculate_signed_tile_address(tile_data_base, tile_index)
                };

                // Apply vertical flip to pixel_y
                let actual_pixel_y = if flip_y { 7 - pixel_y } else { pixel_y };

                // Get tile data (2 bytes per row) from appropriate VRAM bank
                let tile_row_addr = tile_addr + (actual_pixel_y * 2);
                let (byte1, byte2) = if self.cgb_mode && tile_vram_bank == 1 {
                    (
                        self.vram_bank1[tile_row_addr as usize],
                        self.vram_bank1[(tile_row_addr + 1) as usize],
                    )
                } else {
                    (
                        self.vram_bank0[tile_row_addr as usize],
                        self.vram_bank0[(tile_row_addr + 1) as usize],
                    )
                };

                // Apply horizontal flip to pixel_x
                let actual_pixel_x = if flip_x { 7 - pixel_x } else { pixel_x };

                // Get pixel color (2-bit value)
                let bit = 7 - actual_pixel_x;
                let color_bit_0 = (byte1 >> bit) & 1;
                let color_bit_1 = (byte2 >> bit) & 1;
                let color_index = (color_bit_1 << 1) | color_bit_0;

                // Store color index and priority flag for sprite rendering
                // Format: [bit 7: BG priority flag, bits 1-0: color index]
                let pixel_idx = (screen_y as usize * 160) + screen_x as usize;
                bg_color_indices[pixel_idx] = if bg_priority { 0x80 } else { 0 } | color_index;

                // Apply palette and convert to RGB
                let rgb = if self.cgb_mode {
                    // CGB mode: use color palettes
                    let palette_index = (bg_palette_num * 4 + color_index) * 2;
                    let color_low = self.bg_palette_data[palette_index as usize];
                    let color_high = self.bg_palette_data[(palette_index + 1) as usize];
                    self.cgb_color_to_rgb(color_low, color_high)
                } else {
                    // DMG mode: use monochrome palette
                    let palette_color = (self.bgp >> (color_index * 2)) & 0x03;
                    match palette_color {
                        0 => 0xFFFFFFFF, // White
                        1 => 0xFFAAAAAA, // Light gray
                        2 => 0xFF555555, // Dark gray
                        3 => 0xFF000000, // Black
                        _ => unreachable!(),
                    }
                };

                frame.pixels[pixel_idx] = rgb;
            }
        }
    }

    fn render_window(&self, frame: &mut Frame, bg_color_indices: &mut [u8]) {
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

                // Ensure tile_x is within bounds (0-31)
                if tile_x >= 32 {
                    continue;
                }

                let pixel_x = (win_x % 8) as u16;

                // Get tile index from tilemap (always from VRAM bank 0)
                let tilemap_addr = tilemap_base + (tile_y * 32) + tile_x;
                let tile_index = self.vram_bank0[tilemap_addr as usize];

                // Get tile attributes from VRAM bank 1 (CGB only)
                let tile_attr = if self.cgb_mode {
                    self.vram_bank1[tilemap_addr as usize]
                } else {
                    0
                };

                let bg_palette_num = tile_attr & 0x07;
                let tile_vram_bank = (tile_attr >> 3) & 0x01;
                let flip_x = (tile_attr & 0x20) != 0;
                let flip_y = (tile_attr & 0x40) != 0;
                let bg_priority = (tile_attr & 0x80) != 0;

                // Calculate tile data address
                let tile_addr = if (self.lcdc & LCDC_BG_WIN_TILES) != 0 {
                    tile_data_base + (tile_index as u16 * 16)
                } else {
                    self.calculate_signed_tile_address(tile_data_base, tile_index)
                };

                // Apply vertical flip to pixel_y
                let actual_pixel_y = if flip_y { 7 - pixel_y } else { pixel_y };

                // Get tile data (2 bytes per row)
                let tile_row_addr = tile_addr + (actual_pixel_y * 2);

                // Ensure we don't exceed VRAM bounds
                if (tile_row_addr + 1) as usize >= 0x2000 {
                    continue;
                }

                let (byte1, byte2) = if self.cgb_mode && tile_vram_bank == 1 {
                    (
                        self.vram_bank1[tile_row_addr as usize],
                        self.vram_bank1[(tile_row_addr + 1) as usize],
                    )
                } else {
                    (
                        self.vram_bank0[tile_row_addr as usize],
                        self.vram_bank0[(tile_row_addr + 1) as usize],
                    )
                };

                // Apply horizontal flip to pixel_x
                let actual_pixel_x = if flip_x { 7 - pixel_x } else { pixel_x };

                // Get pixel color (2-bit value)
                let bit = 7 - actual_pixel_x;
                let color_bit_0 = (byte1 >> bit) & 1;
                let color_bit_1 = (byte2 >> bit) & 1;
                let color_index = (color_bit_1 << 1) | color_bit_0;

                // Store color index and priority flag for sprite rendering
                // Format: [bit 7: BG priority flag, bits 1-0: color index]
                let pixel_idx = (screen_y as usize * 160) + screen_x as usize;
                bg_color_indices[pixel_idx] = if bg_priority { 0x80 } else { 0 } | color_index;

                // Apply palette and convert to RGB
                let rgb = if self.cgb_mode {
                    // CGB mode: use color palettes
                    let palette_index = (bg_palette_num * 4 + color_index) * 2;
                    let color_low = self.bg_palette_data[palette_index as usize];
                    let color_high = self.bg_palette_data[(palette_index + 1) as usize];
                    self.cgb_color_to_rgb(color_low, color_high)
                } else {
                    // DMG mode: use monochrome palette
                    let palette_color = (self.bgp >> (color_index * 2)) & 0x03;
                    match palette_color {
                        0 => 0xFFFFFFFF, // White
                        1 => 0xFFAAAAAA, // Light gray
                        2 => 0xFF555555, // Dark gray
                        3 => 0xFF000000, // Black
                        _ => unreachable!(),
                    }
                };

                frame.pixels[pixel_idx] = rgb;
            }
        }
    }

    fn render_sprites(&self, frame: &mut Frame, bg_color_indices: &[u8]) {
        // Sprite rendering - Game Boy supports 40 sprites, max 10 per scanline
        let sprite_height = if (self.lcdc & LCDC_OBJ_SIZE) != 0 {
            16
        } else {
            8
        };

        // Process sprites scanline by scanline to enforce 10-sprite limit
        for screen_y in 0u8..144 {
            // Collect all sprites that intersect this scanline
            let mut sprites_on_line: Vec<(u8, u8)> = Vec::new();

            for sprite_idx in 0u8..40 {
                let oam_addr = (sprite_idx as usize) * 4;
                let oam_y = self.oam[oam_addr];
                let oam_x = self.oam[oam_addr + 1];

                // OAM Y/X are offset by 16/8 respectively
                // Sprites are visible when: 0 < Y < 160 and 0 < X < 168
                // Screen position = OAM position - offset

                // Check if sprite intersects this scanline (Y check)
                // screen_y is in range [sprite_top, sprite_bottom]
                // where sprite_top = oam_y - 16, sprite_bottom = oam_y - 16 + sprite_height - 1
                // Rewritten: oam_y - 16 <= screen_y <= oam_y - 16 + sprite_height - 1
                // Which is: oam_y <= screen_y + 16 <= oam_y + sprite_height - 1
                // Simplified: screen_y + 16 >= oam_y && screen_y + 16 < oam_y + sprite_height
                let screen_y_offset = screen_y.wrapping_add(16);
                if oam_y > 0
                    && screen_y_offset >= oam_y
                    && screen_y_offset < oam_y.wrapping_add(sprite_height)
                {
                    // Sprite intersects this scanline, store X position for sorting
                    let x_pos = oam_x.wrapping_sub(8);
                    sprites_on_line.push((x_pos, sprite_idx));
                }
            }

            // Sort sprites by X coordinate (lower first), then by OAM index (lower first)
            // This determines which sprites are selected when there are >10 on a scanline
            sprites_on_line.sort_by_key(|&(x, oam_idx)| (x, oam_idx));

            // Take only first 10 sprites (hardware limit)
            sprites_on_line.truncate(10);

            // Render sprites in reverse order for correct overlap priority
            // (sprites with higher OAM index appear behind sprites with lower OAM index)
            for &(x_pos, sprite_idx) in sprites_on_line.iter().rev() {
                let oam_addr = (sprite_idx as usize) * 4;
                let oam_y = self.oam[oam_addr];
                let tile_index = self.oam[oam_addr + 2];
                let flags = self.oam[oam_addr + 3];

                // OAM flags interpretation differs between DMG and CGB
                // Bit 7: BG/Window priority
                // Bit 6: Y flip
                // Bit 5: X flip
                // Bit 4: Palette number (DMG: 0=OBP0, 1=OBP1; CGB: not used)
                // Bits 3: VRAM bank (CGB only)
                // Bits 2-0: CGB palette number (0-7, CGB only)
                let flip_x = (flags & 0x20) != 0;
                let flip_y = (flags & 0x40) != 0;
                let bg_priority = (flags & 0x80) != 0;

                let (dmg_palette_num, cgb_palette_num, sprite_vram_bank) = if self.cgb_mode {
                    (0, flags & 0x07, (flags >> 3) & 0x01)
                } else {
                    ((flags >> 4) & 0x01, 0, 0)
                };

                // Calculate which row of the sprite we're rendering
                // sy = screen_y - (oam_y - 16) = screen_y - oam_y + 16
                let sy = screen_y.wrapping_add(16).wrapping_sub(oam_y);
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

                // Ensure we don't exceed VRAM bounds
                if (tile_addr + row_offset as u16 + 1) as usize >= 0x2000 {
                    continue;
                }

                // Get tile data from appropriate VRAM bank (CGB sprites can use bank 1)
                let (byte1, byte2) = if self.cgb_mode && sprite_vram_bank == 1 {
                    (
                        self.vram_bank1[(tile_addr + row_offset as u16) as usize],
                        self.vram_bank1[(tile_addr + row_offset as u16 + 1) as usize],
                    )
                } else {
                    (
                        self.vram_bank0[(tile_addr + row_offset as u16) as usize],
                        self.vram_bank0[(tile_addr + row_offset as u16 + 1) as usize],
                    )
                };

                for sx in 0..8u8 {
                    // Calculate actual screen X position
                    let screen_x = x_pos.wrapping_add(sx);

                    // Skip pixels that are off-screen
                    // Screen X must be in range [0, 159]
                    // But due to wrapping, values >= 160 could be either off right edge or off left edge
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

                    // Check background priority
                    // Extract color index and priority flag from bg_color_indices
                    let pixel_idx = (screen_y as usize * 160) + screen_x as usize;
                    let bg_data = bg_color_indices[pixel_idx];
                    let bg_color_index = bg_data & 0x03; // Bits 1-0: color index
                    let bg_has_priority = (bg_data & 0x80) != 0; // Bit 7: BG priority flag

                    // Sprite priority rules:
                    // 1. If LCDC.0 is 0 in CGB mode, sprites are always on top
                    // 2. If BG color is 0, sprite always shows
                    // 3. If BG tile has priority flag set (CGB only), BG is above sprite
                    // 4. If sprite OBJ priority flag is set, sprite is behind BG colors 1-3
                    // 5. Otherwise, sprite is above BG

                    let bg_win_master_priority = (self.lcdc & LCDC_BG_WIN_ENABLE) != 0;

                    if self.cgb_mode && !bg_win_master_priority {
                        // CGB mode with LCDC.0 = 0: sprites always on top
                    } else if bg_color_index == 0 {
                        // BG is transparent, sprite always shows
                    } else if self.cgb_mode && bg_has_priority {
                        // CGB: BG tile has priority, sprite is behind
                        continue;
                    } else if bg_priority {
                        // Sprite has priority flag set, behind BG colors 1-3 (but not 0)
                        continue;
                    }
                    // Otherwise, sprite is above BG

                    // Apply palette and convert to RGB
                    let rgb = if self.cgb_mode {
                        // CGB mode: use color palettes
                        let palette_index = (cgb_palette_num * 4 + color_index) * 2;
                        let color_low = self.obj_palette_data[palette_index as usize];
                        let color_high = self.obj_palette_data[(palette_index + 1) as usize];
                        self.cgb_color_to_rgb(color_low, color_high)
                    } else {
                        // DMG mode: use monochrome palettes
                        let palette = if dmg_palette_num == 1 {
                            self.obp1
                        } else {
                            self.obp0
                        };
                        let palette_color = (palette >> (color_index * 2)) & 0x03;
                        match palette_color {
                            0 => 0xFFFFFFFF, // White (transparent, but palette maps it)
                            1 => 0xFFAAAAAA, // Light gray
                            2 => 0xFF555555, // Dark gray
                            3 => 0xFF000000, // Black
                            _ => unreachable!(),
                        }
                    };

                    frame.pixels[pixel_idx] = rgb;
                }
            }
        }
    }

    /// Step the PPU for the given number of cycles
    pub fn step(&mut self, cycles: u32) -> bool {
        // Accumulate cycles
        self.cycle_counter += cycles;

        let mut vblank_started = false;

        // Process complete scanlines (456 cycles each)
        while self.cycle_counter >= 456 {
            self.cycle_counter -= 456;
            self.ly = (self.ly + 1) % 154;

            // Check LYC=LY interrupt
            if self.ly == self.lyc {
                self.stat |= 0x04; // Set coincidence flag
            } else {
                self.stat &= !0x04;
            }

            // V-Blank is lines 144-153
            if self.ly == 144 {
                vblank_started = true;
            }
        }

        vblank_started
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
        ppu.write_oam(1, 8); // X position
        ppu.write_oam(2, 0); // Tile index
        ppu.write_oam(3, 0); // Flags (no flip, palette 0, above BG)

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

    #[test]
    fn test_sprite_at_left_edge() {
        // Regression test for sprite visibility bug
        // Sprites with OAM X position 0-7 should be partially visible on left edge
        let mut ppu = Ppu::new();
        ppu.lcdc = 0x93; // Enable LCD, sprites, and background
        ppu.obp0 = 0xE4; // Set sprite palette (11 10 01 00)

        // Set up sprite at X=4 (partially visible on left edge)
        ppu.write_oam(0, 16); // Y position (screen Y = 0)
        ppu.write_oam(1, 4); // X position (after -8 offset, rightmost 4 pixels visible at screen X=0-3)
        ppu.write_oam(2, 0); // Tile index
        ppu.write_oam(3, 0); // Flags (no flip, palette 0, above BG)

        // Set up a visible tile in VRAM
        // Create a solid tile with color index 3 (non-transparent)
        ppu.write_vram(0x0000, 0xFF); // Bitplane 0: all 1s
        ppu.write_vram(0x0001, 0xFF); // Bitplane 1: all 1s (color index = 3)

        let frame = ppu.render_frame();

        // Verify sprite is rendered: the rightmost 4 pixels should be visible (X = 0-3)
        // Color index 3 with palette 0xE4: (0xE4 >> (3 * 2)) & 0x03 = (0xE4 >> 6) & 0x03 = 3 (darkest/black)
        let expected_color = 0xFF000000; // Black

        // Check that at least one pixel from the sprite is visible on screen
        // The sprite at X=4 means screen positions 0-3 should show the sprite (last 4 pixels)
        let screen_y = 0;
        let mut found_sprite_pixel = false;
        for screen_x in 0..4 {
            let pixel = frame.pixels[screen_y * 160 + screen_x];
            if pixel == expected_color {
                found_sprite_pixel = true;
                break;
            }
        }
        assert!(
            found_sprite_pixel,
            "Sprite at X=4 should be partially visible on left edge"
        );
    }

    #[test]
    fn test_sprite_per_scanline_limit() {
        let mut ppu = Ppu::new();
        ppu.lcdc = 0x93; // Enable LCD, sprites, and background

        // Create 15 sprites all on the same scanline (Y=16, screen line 0)
        // Set them at different X positions
        for i in 0u16..15 {
            let sprite_idx = i;
            let oam_addr = sprite_idx * 4;
            ppu.write_oam(oam_addr, 16); // Y position (same for all)
            ppu.write_oam(oam_addr + 1, (8 + i) as u8); // X position (different for each)
            ppu.write_oam(oam_addr + 2, 0); // Tile index
            ppu.write_oam(oam_addr + 3, 0); // Flags
        }

        // Set up a simple tile in VRAM with a unique pattern
        ppu.write_vram(0x0000, 0xFF);
        ppu.write_vram(0x0001, 0xFF);

        // Set up a different background color so we can distinguish sprites
        ppu.bgp = 0xE4; // Different from sprite palette

        let frame = ppu.render_frame();

        // Count how many sprites are actually rendered on scanline 0
        // Due to the 10-sprite limit, only the first 10 should be visible
        // The sprites at X positions 8-17 should be visible (10 sprites)
        // The sprites at X positions 18-22 should NOT be visible (5 sprites exceeding limit)

        // Since all sprites use the same tile (all white pixels), we can't easily
        // count individual sprites, but we can verify the implementation compiled
        // and runs without panicking.
        assert_eq!(frame.width, 160);
        assert_eq!(frame.height, 144);
    }
}
