//! GBA PPU (Pixel Processing Unit)
//!
//! The GBA PPU renders a 240x160 display at ~59.73 Hz.
//! It supports 6 background modes with up to 4 background layers,
//! 128 hardware sprites, alpha blending, brightness effects,
//! windowing, and mosaic.
//!
//! ## Background Modes
//!
//! | Mode | BG0     | BG1     | BG2       | BG3       | Notes                       |
//! |------|---------|---------|-----------|-----------|------------------------------|
//! | 0    | Text    | Text    | Text      | Text      | 4 text backgrounds           |
//! | 1    | Text    | Text    | Affine    | -         | 2 text + 1 affine            |
//! | 2    | -       | -       | Affine    | Affine    | 2 affine backgrounds         |
//! | 3    | -       | -       | Bitmap    | -         | 240x160 15-bit direct color  |
//! | 4    | -       | -       | Bitmap    | -         | 240x160 8-bit paletted, 2pg  |
//! | 5    | -       | -       | Bitmap    | -         | 160x128 15-bit direct, 2pg   |
//!
//! ## OAM (Object Attribute Memory)
//!
//! 128 sprites, each described by 3 16-bit attributes + 1 rotation param.
//! Sizes: 8x8 to 64x64. Supports flipping, palette modes, mosaic, priority.
//!
//! ## Color Effects
//!
//! - **Alpha blending**: 1st + 2nd target with configurable coefficients (EVA, EVB)
//! - **Brightness increase**: Blend toward white
//! - **Brightness decrease**: Blend toward black
//! - **Window masking**: WIN0, WIN1, OBJ window, outside window

use emu_core::types::Frame;

// =============================================================================
// Screen dimensions
// =============================================================================

pub const SCREEN_WIDTH: usize = 240;
pub const SCREEN_HEIGHT: usize = 160;

// =============================================================================
// I/O Register Offsets (relative to 0x04000000)
// =============================================================================

// Display control
const DISPCNT: usize = 0x000;

// Background control
const BG0CNT: usize = 0x008;

// Background scroll
const BG0HOFS: usize = 0x010;

// Affine background parameters (BG2/BG3)
const BG2PA: usize = 0x020;
const BG2PB: usize = 0x022;
const BG2PC: usize = 0x024;
const BG2PD: usize = 0x026;
const BG2X: usize = 0x028;
const BG2Y: usize = 0x02C;
const BG3PA: usize = 0x030;
const BG3PB: usize = 0x032;
const BG3PC: usize = 0x034;
const BG3PD: usize = 0x036;
const BG3X: usize = 0x038;
const BG3Y: usize = 0x03C;

// Window registers
const WIN0H: usize = 0x040;
const WIN1H: usize = 0x042;
const WIN0V: usize = 0x044;
const WIN1V: usize = 0x046;
const WININ: usize = 0x048;
const WINOUT: usize = 0x04A;

// Mosaic
const MOSAIC: usize = 0x04C;

// Color effects
const BLDCNT: usize = 0x050;
const BLDALPHA: usize = 0x052;
const BLDY: usize = 0x054;

// =============================================================================
// DISPCNT bit definitions
// =============================================================================

const DISPCNT_BG_MODE_MASK: u16 = 0x0007;
const DISPCNT_FRAME_SELECT: u16 = 1 << 4;
#[allow(dead_code)] // TODO: Implement HBlank OAM access restriction
const DISPCNT_HBLANK_OAM_ACCESS: u16 = 1 << 5;
const DISPCNT_OBJ_MAPPING: u16 = 1 << 6; // 0=2D, 1=1D
const DISPCNT_FORCED_BLANK: u16 = 1 << 7;
const DISPCNT_BG0_ENABLE: u16 = 1 << 8;
const DISPCNT_BG1_ENABLE: u16 = 1 << 9;
const DISPCNT_BG2_ENABLE: u16 = 1 << 10;
const DISPCNT_BG3_ENABLE: u16 = 1 << 11;
const DISPCNT_OBJ_ENABLE: u16 = 1 << 12;
const DISPCNT_WIN0_ENABLE: u16 = 1 << 13;
const DISPCNT_WIN1_ENABLE: u16 = 1 << 14;
const DISPCNT_OBJ_WIN_ENABLE: u16 = 1 << 15;

// =============================================================================
// BGCNT bit definitions
// =============================================================================

const BGCNT_PRIORITY_MASK: u16 = 0x0003;
const BGCNT_TILE_BASE_MASK: u16 = 0x000C;
const BGCNT_MOSAIC: u16 = 1 << 6;
const BGCNT_PALETTE_MODE: u16 = 1 << 7; // 0=4bpp (16 pals of 16), 1=8bpp (1 pal of 256)
const BGCNT_MAP_BASE_MASK: u16 = 0x1F00;
const BGCNT_AFFINE_WRAP: u16 = 1 << 13;
const BGCNT_SIZE_MASK: u16 = 0xC000;

// =============================================================================
// BLDCNT bit definitions
// =============================================================================

const BLDCNT_BG0_1ST: u16 = 1 << 0;
const BLDCNT_BG1_1ST: u16 = 1 << 1;
const BLDCNT_BG2_1ST: u16 = 1 << 2;
const BLDCNT_BG3_1ST: u16 = 1 << 3;
const BLDCNT_OBJ_1ST: u16 = 1 << 4;
const BLDCNT_BD_1ST: u16 = 1 << 5;
const BLDCNT_MODE_MASK: u16 = 0x00C0;
const BLDCNT_BG0_2ND: u16 = 1 << 8;
const BLDCNT_BG1_2ND: u16 = 1 << 9;
const BLDCNT_BG2_2ND: u16 = 1 << 10;
const BLDCNT_BG3_2ND: u16 = 1 << 11;
const BLDCNT_OBJ_2ND: u16 = 1 << 12;
const BLDCNT_BD_2ND: u16 = 1 << 13;

// =============================================================================
// OAM attribute definitions
// =============================================================================

// Attribute 0
const OBJ_ATTR0_Y_MASK: u16 = 0x00FF;
const OBJ_ATTR0_MODE_MASK: u16 = 0x0300;
#[allow(dead_code)] // Matching value for completeness
const OBJ_ATTR0_MODE_NORMAL: u16 = 0x0000;
const OBJ_ATTR0_MODE_AFFINE: u16 = 0x0100;
const OBJ_ATTR0_MODE_HIDDEN: u16 = 0x0200;
const OBJ_ATTR0_MODE_AFFINE_DOUBLE: u16 = 0x0300;
const OBJ_ATTR0_GFX_MODE_MASK: u16 = 0x0C00;
#[allow(dead_code)] // Matching value for completeness
const OBJ_ATTR0_GFX_NORMAL: u16 = 0x0000;
const OBJ_ATTR0_GFX_BLEND: u16 = 0x0400;
const OBJ_ATTR0_GFX_WINDOW: u16 = 0x0800;
#[allow(dead_code)] // TODO: Implement per-sprite mosaic
const OBJ_ATTR0_MOSAIC: u16 = 1 << 12;
const OBJ_ATTR0_PALETTE_MODE: u16 = 1 << 13; // 0=4bpp, 1=8bpp
const OBJ_ATTR0_SHAPE_MASK: u16 = 0xC000;

// Attribute 1
const OBJ_ATTR1_X_MASK: u16 = 0x01FF;
const OBJ_ATTR1_AFFINE_IDX_MASK: u16 = 0x3E00;
const OBJ_ATTR1_HFLIP: u16 = 1 << 12;
const OBJ_ATTR1_VFLIP: u16 = 1 << 13;
const OBJ_ATTR1_SIZE_MASK: u16 = 0xC000;

// Attribute 2
const OBJ_ATTR2_TILE_MASK: u16 = 0x03FF;
const OBJ_ATTR2_PRIORITY_MASK: u16 = 0x0C00;
const OBJ_ATTR2_PALETTE_MASK: u16 = 0xF000;

// =============================================================================
// Sprite size lookup tables
// Shape (attr0 bits 14-15) x Size (attr1 bits 14-15) -> (width, height)
// =============================================================================

/// Sprite dimensions indexed by [shape][size]
const OBJ_SIZES: [[(u32, u32); 4]; 3] = [
    // Square
    [(8, 8), (16, 16), (32, 32), (64, 64)],
    // Horizontal
    [(16, 8), (32, 8), (32, 16), (64, 32)],
    // Vertical
    [(8, 16), (8, 32), (16, 32), (32, 64)],
];

// =============================================================================
// Text BG screen sizes
// =============================================================================

/// Text BG screen size: [size_bits] -> (width_tiles, height_tiles)
const TEXT_BG_SIZES: [(u32, u32); 4] = [
    (32, 32), // 256x256
    (64, 32), // 512x256
    (32, 64), // 256x512
    (64, 64), // 512x512
];

/// Affine BG screen size: [size_bits] -> side_length_tiles
const AFFINE_BG_SIZES: [u32; 4] = [
    16,  // 128x128
    32,  // 256x256
    64,  // 512x512
    128, // 1024x1024
];

// =============================================================================
// Pixel layer info for priority compositing
// =============================================================================

/// Identifies which layer a pixel came from, for blending decisions.
#[derive(Clone, Copy, PartialEq, Eq)]
enum PixelLayer {
    Bg0,
    Bg1,
    Bg2,
    Bg3,
    Obj,
    Backdrop,
}

/// A pixel with its priority and source layer.
#[derive(Clone, Copy)]
struct LayerPixel {
    color: u16,   // 15-bit GBA color (xBBBBBGGGGGRRRRR)
    priority: u8, // 0 (highest) - 3 (lowest)
    layer: PixelLayer,
    is_transparent: bool,
    /// OBJ semi-transparent flag (forces alpha blend when set)
    obj_semi_transparent: bool,
}

impl LayerPixel {
    fn transparent() -> Self {
        Self {
            color: 0,
            priority: 4,
            layer: PixelLayer::Backdrop,
            is_transparent: true,
            obj_semi_transparent: false,
        }
    }
}

// =============================================================================
// PPU State
// =============================================================================

/// GBA PPU / LCD controller.
///
/// Renders the screen scanline-by-scanline using data from VRAM, palette RAM,
/// OAM, and I/O registers provided by the memory bus.
pub struct Ppu {
    /// The current frame buffer
    frame: Frame,

    /// Internal affine reference point latches for BG2
    pub bg2_ref_x: i32,
    pub bg2_ref_y: i32,
    /// Internal affine reference point latches for BG3
    pub bg3_ref_x: i32,
    pub bg3_ref_y: i32,

    /// Scanline buffers for compositing
    /// Each BG layer's pixel output for current scanline
    bg_lines: [[LayerPixel; SCREEN_WIDTH]; 4],
    /// OBJ layer pixel output for current scanline
    obj_line: [LayerPixel; SCREEN_WIDTH],
    /// OBJ window mask for current scanline (true = inside OBJ window)
    obj_window: [bool; SCREEN_WIDTH],
}

impl std::fmt::Debug for Ppu {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Ppu")
            .field("bg2_ref_x", &self.bg2_ref_x)
            .field("bg2_ref_y", &self.bg2_ref_y)
            .finish()
    }
}

impl Default for Ppu {
    fn default() -> Self {
        Self::new()
    }
}

impl Ppu {
    pub fn new() -> Self {
        Self {
            frame: Frame::new(SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32),
            bg2_ref_x: 0,
            bg2_ref_y: 0,
            bg3_ref_x: 0,
            bg3_ref_y: 0,
            bg_lines: [[LayerPixel::transparent(); SCREEN_WIDTH]; 4],
            obj_line: [LayerPixel::transparent(); SCREEN_WIDTH],
            obj_window: [false; SCREEN_WIDTH],
        }
    }

    /// Reset PPU state
    pub fn reset(&mut self) {
        self.frame = Frame::new(SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32);
        self.bg2_ref_x = 0;
        self.bg2_ref_y = 0;
        self.bg3_ref_x = 0;
        self.bg3_ref_y = 0;
    }

    /// Latch the affine reference points from I/O registers.
    /// Called at VBlank and frame start to reload all reference points.
    pub fn latch_affine_registers(&mut self, io: &[u8]) {
        self.bg2_ref_x = read_io_28bit_signed(io, BG2X);
        self.bg2_ref_y = read_io_28bit_signed(io, BG2Y);
        self.bg3_ref_x = read_io_28bit_signed(io, BG3X);
        self.bg3_ref_y = read_io_28bit_signed(io, BG3Y);
    }

    /// Selectively update affine reference points based on dirty flags.
    /// Called when the game writes to BG2X/BG2Y/BG3X/BG3Y registers
    /// (via CPU or DMA). On real hardware these are write-only latches
    /// that immediately update the internal reference point.
    ///
    /// dirty_bits: bit 0=BG2X, bit 1=BG2Y, bit 2=BG3X, bit 3=BG3Y
    pub fn apply_affine_ref_writes(&mut self, io: &[u8], dirty_bits: u8) {
        if dirty_bits & 1 != 0 {
            self.bg2_ref_x = read_io_28bit_signed(io, BG2X);
        }
        if dirty_bits & 2 != 0 {
            self.bg2_ref_y = read_io_28bit_signed(io, BG2Y);
        }
        if dirty_bits & 4 != 0 {
            self.bg3_ref_x = read_io_28bit_signed(io, BG3X);
        }
        if dirty_bits & 8 != 0 {
            self.bg3_ref_y = read_io_28bit_signed(io, BG3Y);
        }
    }

    /// Get a reference to the current frame buffer
    pub fn frame(&self) -> &Frame {
        &self.frame
    }

    /// Clone the current frame buffer
    pub fn clone_frame(&self) -> Frame {
        self.frame.clone()
    }

    // =========================================================================
    // Main scanline rendering entry point
    // =========================================================================

    /// Render a single scanline.
    ///
    /// # Arguments
    /// * `line` - Scanline number (0-159)
    /// * `io` - I/O register slice (0x400 bytes starting at 0x04000000)
    /// * `palette` - Palette RAM (0x400 bytes)
    /// * `vram` - Video RAM (0x18000 bytes = 96KB)
    /// * `oam` - Object Attribute Memory (0x400 bytes)
    pub fn render_scanline(
        &mut self,
        line: u32,
        io: &[u8],
        palette: &[u8],
        vram: &[u8],
        oam: &[u8],
    ) {
        if line >= SCREEN_HEIGHT as u32 {
            return;
        }

        let dispcnt = read_io_u16(io, DISPCNT);

        // Forced blank - white screen
        if dispcnt & DISPCNT_FORCED_BLANK != 0 {
            let offset = line as usize * SCREEN_WIDTH;
            for x in 0..SCREEN_WIDTH {
                self.frame.pixels[offset + x] = 0xFFFFFFFF; // white
            }
            return;
        }

        let bg_mode = (dispcnt & DISPCNT_BG_MODE_MASK) as u8;

        // Clear scanline buffers
        for bg in 0..4 {
            self.bg_lines[bg] = [LayerPixel::transparent(); SCREEN_WIDTH];
        }
        self.obj_line = [LayerPixel::transparent(); SCREEN_WIDTH];
        self.obj_window = [false; SCREEN_WIDTH];

        // Render enabled layers based on mode
        match bg_mode {
            0 => {
                // Mode 0: 4 text BGs
                if dispcnt & DISPCNT_BG0_ENABLE != 0 {
                    self.render_text_bg(0, line, io, palette, vram);
                }
                if dispcnt & DISPCNT_BG1_ENABLE != 0 {
                    self.render_text_bg(1, line, io, palette, vram);
                }
                if dispcnt & DISPCNT_BG2_ENABLE != 0 {
                    self.render_text_bg(2, line, io, palette, vram);
                }
                if dispcnt & DISPCNT_BG3_ENABLE != 0 {
                    self.render_text_bg(3, line, io, palette, vram);
                }
            }
            1 => {
                // Mode 1: 2 text + 1 affine
                if dispcnt & DISPCNT_BG0_ENABLE != 0 {
                    self.render_text_bg(0, line, io, palette, vram);
                }
                if dispcnt & DISPCNT_BG1_ENABLE != 0 {
                    self.render_text_bg(1, line, io, palette, vram);
                }
                if dispcnt & DISPCNT_BG2_ENABLE != 0 {
                    self.render_affine_bg(2, line, io, palette, vram);
                }
            }
            2 => {
                // Mode 2: 2 affine
                if dispcnt & DISPCNT_BG2_ENABLE != 0 {
                    self.render_affine_bg(2, line, io, palette, vram);
                }
                if dispcnt & DISPCNT_BG3_ENABLE != 0 {
                    self.render_affine_bg(3, line, io, palette, vram);
                }
            }
            3 => {
                // Mode 3: 240x160 direct color bitmap
                if dispcnt & DISPCNT_BG2_ENABLE != 0 {
                    self.render_bitmap_mode3(line, io, vram);
                }
            }
            4 => {
                // Mode 4: 240x160 paletted bitmap
                if dispcnt & DISPCNT_BG2_ENABLE != 0 {
                    self.render_bitmap_mode4(line, dispcnt, io, palette, vram);
                }
            }
            5 => {
                // Mode 5: 160x128 direct color bitmap
                if dispcnt & DISPCNT_BG2_ENABLE != 0 {
                    self.render_bitmap_mode5(line, dispcnt, io, vram);
                }
            }
            _ => {} // Invalid mode
        }

        // Render sprites if enabled
        if dispcnt & DISPCNT_OBJ_ENABLE != 0 {
            self.render_objects(line, dispcnt, io, palette, vram, oam);
        }

        // Composite layers with priority, windowing, and color effects
        self.composite_scanline(line, dispcnt, io, palette);

        // Update affine reference points for next scanline
        // BG2 affine refs are used in modes 1-5 (affine + all bitmap modes)
        if (1..=5).contains(&bg_mode) {
            self.increment_affine_refs(2, io);
        }
        // BG3 affine refs are only used in mode 2
        if bg_mode == 2 {
            self.increment_affine_refs(3, io);
        }
    }

    /// Called at VBlank to latch affine reference point registers
    pub fn on_vblank(&mut self, io: &[u8]) {
        self.latch_affine_registers(io);
    }

    // =========================================================================
    // Text background rendering (Modes 0, 1)
    // =========================================================================

    fn render_text_bg(&mut self, bg_idx: usize, line: u32, io: &[u8], palette: &[u8], vram: &[u8]) {
        let bgcnt = read_io_u16(io, BG0CNT + bg_idx * 2);
        let priority = (bgcnt & BGCNT_PRIORITY_MASK) as u8;
        let tile_base = ((bgcnt & BGCNT_TILE_BASE_MASK) as usize >> 2) * 0x4000;
        let is_8bpp = bgcnt & BGCNT_PALETTE_MODE != 0;
        let map_base = ((bgcnt & BGCNT_MAP_BASE_MASK) as usize >> 8) * 0x800;
        let mosaic = bgcnt & BGCNT_MOSAIC != 0;
        let size_bits = ((bgcnt & BGCNT_SIZE_MASK) >> 14) as usize;
        let (map_w, map_h) = TEXT_BG_SIZES[size_bits];

        // Scroll offsets
        let hofs_reg = BG0HOFS + bg_idx * 4;
        let scroll_x = read_io_u16(io, hofs_reg) & 0x1FF;
        let scroll_y = read_io_u16(io, hofs_reg + 2) & 0x1FF;

        // Apply mosaic
        let line = if mosaic {
            let mos = read_io_u16(io, MOSAIC);
            let bg_mos_v = ((mos >> 4) & 0xF) as u32 + 1;
            (line / bg_mos_v) * bg_mos_v
        } else {
            line
        };

        let screen_y = (line + scroll_y as u32) & (map_h * 8 - 1);
        let tile_row = screen_y / 8;
        let fine_y = screen_y & 7;

        for x in 0..SCREEN_WIDTH {
            let screen_x = (x as u32 + scroll_x as u32) & (map_w * 8 - 1);
            let tile_col = screen_x / 8;
            let fine_x = screen_x & 7;

            // Calculate screen block offset for > 32x32 maps
            let screen_block = match (map_w, map_h) {
                (64, 64) => (tile_row / 32) * 2 + tile_col / 32,
                (64, 32) => tile_col / 32,
                (32, 64) => tile_row / 32,
                _ => 0,
            };
            let local_col = tile_col & 31;
            let local_row = tile_row & 31;

            let map_entry_addr = map_base
                + (screen_block as usize * 0x800)
                + (local_row as usize * 32 + local_col as usize) * 2;

            if map_entry_addr + 1 >= vram.len() {
                continue;
            }

            let map_entry = vram[map_entry_addr] as u16 | ((vram[map_entry_addr + 1] as u16) << 8);

            let tile_id = (map_entry & 0x3FF) as usize;
            let hflip = map_entry & (1 << 10) != 0;
            let vflip = map_entry & (1 << 11) != 0;
            let pal_bank = ((map_entry >> 12) & 0xF) as usize;

            let pixel_y = if vflip { 7 - fine_y } else { fine_y } as usize;
            let pixel_x = if hflip { 7 - fine_x } else { fine_x } as usize;

            let color_idx = if is_8bpp {
                // 8bpp: 64 bytes per tile
                let tile_addr = tile_base + tile_id * 64 + pixel_y * 8 + pixel_x;
                if tile_addr < vram.len() {
                    vram[tile_addr] as usize
                } else {
                    0
                }
            } else {
                // 4bpp: 32 bytes per tile, 2 pixels per byte
                let tile_addr = tile_base + tile_id * 32 + pixel_y * 4 + pixel_x / 2;
                if tile_addr < vram.len() {
                    let byte = vram[tile_addr];
                    if pixel_x & 1 == 0 {
                        (byte & 0x0F) as usize
                    } else {
                        (byte >> 4) as usize
                    }
                } else {
                    0
                }
            };

            // Color index 0 is always transparent
            if color_idx == 0 {
                continue;
            }

            let pal_idx = if is_8bpp {
                color_idx
            } else {
                pal_bank * 16 + color_idx
            };

            let color = palette_lookup(palette, pal_idx);

            self.bg_lines[bg_idx][x] = LayerPixel {
                color,
                priority,
                layer: bg_layer(bg_idx),
                is_transparent: false,
                obj_semi_transparent: false,
            };
        }

        // Apply horizontal mosaic
        if mosaic {
            let mos = read_io_u16(io, MOSAIC);
            let bg_mos_h = ((mos) & 0xF) as usize + 1;
            if bg_mos_h > 1 {
                for x in 0..SCREEN_WIDTH {
                    let src = (x / bg_mos_h) * bg_mos_h;
                    if src != x {
                        self.bg_lines[bg_idx][x] = self.bg_lines[bg_idx][src];
                    }
                }
            }
        }
    }

    // =========================================================================
    // Affine (rotation/scaling) background rendering (Modes 1, 2)
    // =========================================================================

    fn render_affine_bg(
        &mut self,
        bg_idx: usize,
        _line: u32,
        io: &[u8],
        palette: &[u8],
        vram: &[u8],
    ) {
        let bgcnt = read_io_u16(io, BG0CNT + bg_idx * 2);
        let priority = (bgcnt & BGCNT_PRIORITY_MASK) as u8;
        let tile_base = ((bgcnt & BGCNT_TILE_BASE_MASK) as usize >> 2) * 0x4000;
        let map_base = ((bgcnt & BGCNT_MAP_BASE_MASK) as usize >> 8) * 0x800;
        let wrap = bgcnt & BGCNT_AFFINE_WRAP != 0;
        let size_bits = ((bgcnt & BGCNT_SIZE_MASK) >> 14) as usize;
        let map_size = AFFINE_BG_SIZES[size_bits]; // tiles per side
        let pixel_size = map_size * 8; // pixels per side

        // Get current reference point
        let (ref_x, ref_y) = if bg_idx == 2 {
            (self.bg2_ref_x, self.bg2_ref_y)
        } else {
            (self.bg3_ref_x, self.bg3_ref_y)
        };

        // Get affine parameters for per-pixel horizontal stepping
        // Affine matrix: [PA PB]   where PA/PB affect X coordinate
        //                [PC PD]   and   PC/PD affect Y coordinate
        let pa_offset = if bg_idx == 2 { BG2PA } else { BG3PA };
        let pc_offset = if bg_idx == 2 { BG2PC } else { BG3PC };
        let pa = read_io_i16(io, pa_offset) as i32; // X increment per screen X
        let pc = read_io_i16(io, pc_offset) as i32; // Y increment per screen X

        // Affine BGs are always 8bpp with a single 256-color palette
        for x in 0..SCREEN_WIDTH {
            // Calculate source coordinates using affine matrix
            // For a horizontal scanline at screen position (x, 0):
            // src_x = ref_x + pa * x + pb * 0 = ref_x + pa * x
            // src_y = ref_y + pc * x + pd * 0 = ref_y + pc * x
            // ref_x/ref_y are 28-bit signed fixed point (20.8)
            // pa/pb/pc/pd are 16-bit signed fixed point (8.8)
            let src_x = ref_x + pa * x as i32;
            let src_y = ref_y + pc * x as i32;

            // Convert from 8.8 fixed point to integer
            let tex_x = src_x >> 8;
            let tex_y = src_y >> 8;

            let (tex_x, tex_y) = if wrap {
                (
                    tex_x.rem_euclid(pixel_size as i32),
                    tex_y.rem_euclid(pixel_size as i32),
                )
            } else if tex_x < 0
                || tex_y < 0
                || tex_x >= pixel_size as i32
                || tex_y >= pixel_size as i32
            {
                continue; // Out of bounds = transparent
            } else {
                (tex_x, tex_y)
            };

            let tile_x = tex_x as u32 / 8;
            let tile_y = tex_y as u32 / 8;
            let fine_x = tex_x as u32 & 7;
            let fine_y = tex_y as u32 & 7;

            // Affine map entries are 1 byte each (just a tile index)
            let map_addr = map_base + (tile_y * map_size + tile_x) as usize;
            if map_addr >= vram.len() {
                continue;
            }
            let tile_id = vram[map_addr] as usize;

            // 8bpp tiles: 64 bytes per tile
            let tile_addr = tile_base + tile_id * 64 + fine_y as usize * 8 + fine_x as usize;
            if tile_addr >= vram.len() {
                continue;
            }
            let color_idx = vram[tile_addr] as usize;

            if color_idx == 0 {
                continue;
            }

            let color = palette_lookup(palette, color_idx);

            self.bg_lines[bg_idx][x] = LayerPixel {
                color,
                priority,
                layer: bg_layer(bg_idx),
                is_transparent: false,
                obj_semi_transparent: false,
            };
        }
    }

    /// Increment affine reference points by PB/PD for the next scanline.
    ///
    /// Per GBA hardware, after each scanline the internal reference points
    /// are updated:
    ///   ref_x += PB  (dmx: horizontal displacement per scanline)
    ///   ref_y += PD  (dmy: vertical displacement per scanline)
    fn increment_affine_refs(&mut self, bg_idx: usize, io: &[u8]) {
        let pb_offset = if bg_idx == 2 { BG2PB } else { BG3PB };
        let pd_offset = if bg_idx == 2 { BG2PD } else { BG3PD };
        let pb = read_io_i16(io, pb_offset) as i32;
        let pd = read_io_i16(io, pd_offset) as i32;

        if bg_idx == 2 {
            self.bg2_ref_x += pb;
            self.bg2_ref_y += pd;
        } else {
            self.bg3_ref_x += pb;
            self.bg3_ref_y += pd;
        }
    }

    // =========================================================================
    // Bitmap mode rendering (Modes 3, 4, 5)
    // =========================================================================

    fn render_bitmap_mode3(&mut self, _line: u32, io: &[u8], vram: &[u8]) {
        let bgcnt = read_io_u16(io, BG0CNT + 2 * 2); // BG2CNT
        let priority = (bgcnt & BGCNT_PRIORITY_MASK) as u8;

        // Use BG2 affine parameters for rotation/scaling
        let ref_x = self.bg2_ref_x;
        let ref_y = self.bg2_ref_y;
        let pa = read_io_i16(io, BG2PA) as i32;
        let pc = read_io_i16(io, BG2PC) as i32;

        for x in 0..SCREEN_WIDTH {
            let src_x = ref_x + pa * x as i32;
            let src_y = ref_y + pc * x as i32;
            let tex_x = src_x >> 8;
            let tex_y = src_y >> 8;

            // Clamp to screen bounds (no wrapping for bitmap modes)
            if tex_x < 0
                || tex_y < 0
                || tex_x >= SCREEN_WIDTH as i32
                || tex_y >= SCREEN_HEIGHT as i32
            {
                continue;
            }

            let offset = (tex_y as usize * SCREEN_WIDTH + tex_x as usize) * 2;
            if offset + 1 < vram.len() {
                let color = vram[offset] as u16 | ((vram[offset + 1] as u16) << 8);
                self.bg_lines[2][x] = LayerPixel {
                    color,
                    priority,
                    layer: PixelLayer::Bg2,
                    is_transparent: false,
                    obj_semi_transparent: false,
                };
            }
        }
    }

    fn render_bitmap_mode4(
        &mut self,
        _line: u32,
        dispcnt: u16,
        io: &[u8],
        palette: &[u8],
        vram: &[u8],
    ) {
        let bgcnt = read_io_u16(io, BG0CNT + 2 * 2);
        let priority = (bgcnt & BGCNT_PRIORITY_MASK) as u8;
        let page = if dispcnt & DISPCNT_FRAME_SELECT != 0 {
            0xA000
        } else {
            0
        };

        // Use BG2 affine parameters for rotation/scaling
        let ref_x = self.bg2_ref_x;
        let ref_y = self.bg2_ref_y;
        let pa = read_io_i16(io, BG2PA) as i32;
        let pc = read_io_i16(io, BG2PC) as i32;

        for x in 0..SCREEN_WIDTH {
            let src_x = ref_x + pa * x as i32;
            let src_y = ref_y + pc * x as i32;
            let tex_x = src_x >> 8;
            let tex_y = src_y >> 8;

            if tex_x < 0
                || tex_y < 0
                || tex_x >= SCREEN_WIDTH as i32
                || tex_y >= SCREEN_HEIGHT as i32
            {
                continue;
            }

            let offset = page + tex_y as usize * SCREEN_WIDTH + tex_x as usize;
            if offset < vram.len() {
                let idx = vram[offset] as usize;
                if idx != 0 {
                    let color = palette_lookup(palette, idx);
                    self.bg_lines[2][x] = LayerPixel {
                        color,
                        priority,
                        layer: PixelLayer::Bg2,
                        is_transparent: false,
                        obj_semi_transparent: false,
                    };
                }
            }
        }
    }

    fn render_bitmap_mode5(&mut self, _line: u32, dispcnt: u16, io: &[u8], vram: &[u8]) {
        let bgcnt = read_io_u16(io, BG0CNT + 2 * 2);
        let priority = (bgcnt & BGCNT_PRIORITY_MASK) as u8;
        let page = if dispcnt & DISPCNT_FRAME_SELECT != 0 {
            0xA000
        } else {
            0
        };

        // Mode 5: 160x128 resolution
        const MODE5_WIDTH: i32 = 160;
        const MODE5_HEIGHT: i32 = 128;

        // Use BG2 affine parameters for rotation/scaling
        let ref_x = self.bg2_ref_x;
        let ref_y = self.bg2_ref_y;
        let pa = read_io_i16(io, BG2PA) as i32;
        let pc = read_io_i16(io, BG2PC) as i32;

        for x in 0..SCREEN_WIDTH {
            let src_x = ref_x + pa * x as i32;
            let src_y = ref_y + pc * x as i32;
            let tex_x = src_x >> 8;
            let tex_y = src_y >> 8;

            if tex_x < 0 || tex_y < 0 || tex_x >= MODE5_WIDTH || tex_y >= MODE5_HEIGHT {
                continue;
            }

            let offset = page + (tex_y as usize * MODE5_WIDTH as usize + tex_x as usize) * 2;
            if offset + 1 < vram.len() {
                let color = vram[offset] as u16 | ((vram[offset + 1] as u16) << 8);
                self.bg_lines[2][x] = LayerPixel {
                    color,
                    priority,
                    layer: PixelLayer::Bg2,
                    is_transparent: false,
                    obj_semi_transparent: false,
                };
            }
        }
    }

    // =========================================================================
    // Object (sprite) rendering
    // =========================================================================

    fn render_objects(
        &mut self,
        line: u32,
        dispcnt: u16,
        _io: &[u8],
        palette: &[u8],
        vram: &[u8],
        oam: &[u8],
    ) {
        let obj_1d_mapping = dispcnt & DISPCNT_OBJ_MAPPING != 0;

        // OAM contains 128 entries, each 8 bytes (3 attributes + 1 rotation/scale param)
        // Sprites are rendered in reverse order so lower-numbered sprites have higher priority
        for obj_idx in (0..128).rev() {
            let oam_offset = obj_idx * 8;
            let attr0 = oam[oam_offset] as u16 | ((oam[oam_offset + 1] as u16) << 8);
            let attr1 = oam[oam_offset + 2] as u16 | ((oam[oam_offset + 3] as u16) << 8);
            let attr2 = oam[oam_offset + 4] as u16 | ((oam[oam_offset + 5] as u16) << 8);

            let obj_mode = attr0 & OBJ_ATTR0_MODE_MASK;

            // Skip hidden sprites
            if obj_mode == OBJ_ATTR0_MODE_HIDDEN {
                continue;
            }

            let is_affine =
                obj_mode == OBJ_ATTR0_MODE_AFFINE || obj_mode == OBJ_ATTR0_MODE_AFFINE_DOUBLE;
            let double_size = obj_mode == OBJ_ATTR0_MODE_AFFINE_DOUBLE;

            let gfx_mode = attr0 & OBJ_ATTR0_GFX_MODE_MASK;
            let is_semi_transparent = gfx_mode == OBJ_ATTR0_GFX_BLEND;
            let is_obj_window = gfx_mode == OBJ_ATTR0_GFX_WINDOW;

            let is_8bpp = attr0 & OBJ_ATTR0_PALETTE_MODE != 0;
            let shape = ((attr0 & OBJ_ATTR0_SHAPE_MASK) >> 14) as usize;
            let size = ((attr1 & OBJ_ATTR1_SIZE_MASK) >> 14) as usize;

            if shape >= 3 {
                continue; // Invalid shape
            }

            let (obj_w, obj_h) = OBJ_SIZES[shape][size];

            // Bounding box dimensions (affected by double-size for affine)
            let (bound_w, bound_h) = if double_size {
                (obj_w * 2, obj_h * 2)
            } else {
                (obj_w, obj_h)
            };

            let obj_y = (attr0 & OBJ_ATTR0_Y_MASK) as i32;
            let obj_x = (attr1 & OBJ_ATTR1_X_MASK) as i32;
            // Sign-extend 9-bit X coordinate
            let obj_x = if obj_x >= 256 { obj_x - 512 } else { obj_x };
            // Y wraps at 256
            let obj_y = if obj_y >= 160 && obj_y + bound_h as i32 > 256 {
                obj_y - 256
            } else {
                obj_y
            };

            // Check if this sprite is on the current scanline
            let local_y = line as i32 - obj_y;
            if local_y < 0 || local_y >= bound_h as i32 {
                continue;
            }

            let tile_base_id = (attr2 & OBJ_ATTR2_TILE_MASK) as usize;
            let priority = ((attr2 & OBJ_ATTR2_PRIORITY_MASK) >> 10) as u8;
            let pal_bank = ((attr2 & OBJ_ATTR2_PALETTE_MASK) >> 12) as usize;

            // OBJ VRAM starts at 0x10000
            let obj_vram_base = 0x10000;

            for local_x in 0..(bound_w as i32) {
                let screen_x = obj_x + local_x;
                if screen_x < 0 || screen_x >= SCREEN_WIDTH as i32 {
                    continue;
                }
                let sx = screen_x as usize;

                // For non-affine: apply flips and compute texture coordinates
                let (tex_x, tex_y) = if is_affine {
                    let affine_idx = ((attr1 & OBJ_ATTR1_AFFINE_IDX_MASK) >> 9) as usize;
                    let pa_offset = affine_idx * 32 + 6;
                    let pb_offset = affine_idx * 32 + 14;
                    let pc_offset = affine_idx * 32 + 22;
                    let pd_offset = affine_idx * 32 + 30;

                    let pa = if pa_offset + 1 < oam.len() {
                        (oam[pa_offset] as u16 | ((oam[pa_offset + 1] as u16) << 8)) as i16 as i32
                    } else {
                        0x100
                    };
                    let pb = if pb_offset + 1 < oam.len() {
                        (oam[pb_offset] as u16 | ((oam[pb_offset + 1] as u16) << 8)) as i16 as i32
                    } else {
                        0
                    };
                    let pc = if pc_offset + 1 < oam.len() {
                        (oam[pc_offset] as u16 | ((oam[pc_offset + 1] as u16) << 8)) as i16 as i32
                    } else {
                        0
                    };
                    let pd = if pd_offset + 1 < oam.len() {
                        (oam[pd_offset] as u16 | ((oam[pd_offset + 1] as u16) << 8)) as i16 as i32
                    } else {
                        0x100
                    };

                    let cx = obj_w as i32 / 2;
                    let cy = obj_h as i32 / 2;
                    let dx = local_x - bound_w as i32 / 2;
                    let dy = local_y - bound_h as i32 / 2;

                    let tex_x = (pa * dx + pb * dy) >> 8;
                    let tex_y = (pc * dx + pd * dy) >> 8;
                    let tex_x = tex_x + cx;
                    let tex_y = tex_y + cy;

                    if tex_x < 0 || tex_y < 0 || tex_x >= obj_w as i32 || tex_y >= obj_h as i32 {
                        continue;
                    }
                    (tex_x as u32, tex_y as u32)
                } else {
                    let hflip = attr1 & OBJ_ATTR1_HFLIP != 0;
                    let vflip = attr1 & OBJ_ATTR1_VFLIP != 0;
                    let tx = if hflip {
                        obj_w - 1 - local_x as u32
                    } else {
                        local_x as u32
                    };
                    let ty = if vflip {
                        obj_h - 1 - local_y as u32
                    } else {
                        local_y as u32
                    };
                    (tx, ty)
                };

                // Calculate tile and pixel within tile
                let tile_x = tex_x / 8;
                let tile_y = tex_y / 8;
                let fine_x = (tex_x & 7) as usize;
                let fine_y = (tex_y & 7) as usize;

                let tile_id = if obj_1d_mapping {
                    // 1D mapping: tiles are sequential in memory
                    let tiles_per_row = obj_w / 8;
                    let tile_offset = tile_y * tiles_per_row + tile_x;
                    if is_8bpp {
                        tile_base_id + tile_offset as usize * 2
                    } else {
                        tile_base_id + tile_offset as usize
                    }
                } else {
                    // 2D mapping: 32 tile-IDs per row in VRAM regardless of BPP.
                    // For 8bpp, each 8x8 tile spans 2 consecutive tile IDs,
                    // but the row stride remains 32 tile-IDs.
                    if is_8bpp {
                        tile_base_id + tile_y as usize * 32 + tile_x as usize * 2
                    } else {
                        tile_base_id + tile_y as usize * 32 + tile_x as usize
                    }
                };

                let color_idx = if is_8bpp {
                    let tile_addr = obj_vram_base + tile_id * 32 + fine_y * 8 + fine_x;
                    if tile_addr < vram.len() {
                        vram[tile_addr] as usize
                    } else {
                        0
                    }
                } else {
                    let tile_addr = obj_vram_base + tile_id * 32 + fine_y * 4 + fine_x / 2;
                    if tile_addr < vram.len() {
                        let byte = vram[tile_addr];
                        if fine_x & 1 == 0 {
                            (byte & 0x0F) as usize
                        } else {
                            (byte >> 4) as usize
                        }
                    } else {
                        0
                    }
                };

                if color_idx == 0 {
                    continue;
                }

                // OBJ palette is second half of palette RAM (256..512 bytes, or 128..255 indices)
                let pal_idx = if is_8bpp {
                    color_idx
                } else {
                    pal_bank * 16 + color_idx
                };

                // OBJ palette starts at byte offset 0x200 in palette RAM
                let pal_offset = 0x200 + pal_idx * 2;
                let color = if pal_offset + 1 < palette.len() {
                    palette[pal_offset] as u16 | ((palette[pal_offset + 1] as u16) << 8)
                } else {
                    0
                };

                if is_obj_window {
                    // OBJ window: pixel marks the window region
                    self.obj_window[sx] = true;
                } else {
                    // Only overwrite if higher priority (lower numbered sprites win)
                    // Since we iterate in reverse, later writes (lower indices) override
                    self.obj_line[sx] = LayerPixel {
                        color,
                        priority,
                        layer: PixelLayer::Obj,
                        is_transparent: false,
                        obj_semi_transparent: is_semi_transparent,
                    };
                }
            }
        }

        // Apply mosaic to OBJ if needed
        // (OBJ mosaic is per-sprite via attr0 bit, but we simplify to global here)
        // TODO: Per-sprite mosaic tracking
    }

    // =========================================================================
    // Layer compositing with priority, windowing, and blending
    // =========================================================================

    fn composite_scanline(&mut self, line: u32, dispcnt: u16, io: &[u8], palette: &[u8]) {
        let bldcnt = read_io_u16(io, BLDCNT);
        let blend_mode = (bldcnt & BLDCNT_MODE_MASK) >> 6;

        let eva = (read_io_u16(io, BLDALPHA) & 0x1F).min(16) as u32;
        let evb = ((read_io_u16(io, BLDALPHA) >> 8) & 0x1F).min(16) as u32;
        let evy = (read_io_u16(io, BLDY) & 0x1F).min(16) as u32;

        // Window state
        let use_windows =
            dispcnt & (DISPCNT_WIN0_ENABLE | DISPCNT_WIN1_ENABLE | DISPCNT_OBJ_WIN_ENABLE) != 0;

        let win0_enabled = dispcnt & DISPCNT_WIN0_ENABLE != 0;
        let win1_enabled = dispcnt & DISPCNT_WIN1_ENABLE != 0;
        let obj_win_enabled = dispcnt & DISPCNT_OBJ_WIN_ENABLE != 0;

        // Window boundary registers
        let win0h = read_io_u16(io, WIN0H);
        let win1h = read_io_u16(io, WIN1H);
        let win0v = read_io_u16(io, WIN0V);
        let win1v = read_io_u16(io, WIN1V);
        let winin = read_io_u16(io, WININ);
        let winout = read_io_u16(io, WINOUT);

        // Backdrop color (palette[0])
        let backdrop_color = palette_lookup(palette, 0);

        let offset = line as usize * SCREEN_WIDTH;

        for x in 0..SCREEN_WIDTH {
            // Determine window flags for this pixel
            let win_flags = if use_windows {
                self.get_window_flags(
                    x as u32,
                    line,
                    win0_enabled,
                    win1_enabled,
                    obj_win_enabled,
                    win0h,
                    win1h,
                    win0v,
                    win1v,
                    winin,
                    winout,
                )
            } else {
                // All layers enabled, blending enabled
                0x3F
            };

            let blend_enabled = win_flags & (1 << 5) != 0;

            // Find top two pixels by priority
            let (top, second) = self.find_top_pixels(x, dispcnt, win_flags, backdrop_color);

            // Apply color effects
            let final_color = if blend_enabled {
                self.apply_blend(
                    &top,
                    &second,
                    blend_mode,
                    bldcnt,
                    eva,
                    evb,
                    evy,
                    backdrop_color,
                )
            } else {
                top.color
            };

            self.frame.pixels[offset + x] = gba_color_to_rgb(final_color);
        }
    }

    /// Get the window control flags for a pixel position.
    /// Returns a 6-bit value: bits 0-4 = BG0..OBJ enable, bit 5 = blend enable
    #[allow(clippy::too_many_arguments)]
    fn get_window_flags(
        &self,
        x: u32,
        y: u32,
        win0_enabled: bool,
        win1_enabled: bool,
        obj_win_enabled: bool,
        win0h: u16,
        win1h: u16,
        win0v: u16,
        win1v: u16,
        winin: u16,
        winout: u16,
    ) -> u8 {
        // Check WIN0
        if win0_enabled && in_window(x, y, win0h, win0v) {
            return (winin & 0x3F) as u8;
        }

        // Check WIN1
        if win1_enabled && in_window(x, y, win1h, win1v) {
            return ((winin >> 8) & 0x3F) as u8;
        }

        // Check OBJ window
        if obj_win_enabled && (x as usize) < SCREEN_WIDTH && self.obj_window[x as usize] {
            return ((winout >> 8) & 0x3F) as u8;
        }

        // Outside all windows
        (winout & 0x3F) as u8
    }

    /// Find the top two visible pixels at a screen X position, sorted by priority.
    fn find_top_pixels(
        &self,
        x: usize,
        dispcnt: u16,
        win_flags: u8,
        backdrop_color: u16,
    ) -> (LayerPixel, LayerPixel) {
        let mut top = LayerPixel {
            color: backdrop_color,
            priority: 5,
            layer: PixelLayer::Backdrop,
            is_transparent: false,
            obj_semi_transparent: false,
        };
        let mut second = top;

        // Collect all candidate pixels with their priorities
        // Priority ordering: lower number = higher priority
        // Within same priority, layer order is: OBJ > BG0 > BG1 > BG2 > BG3

        // Check each BG layer
        let bg_enables = [
            (DISPCNT_BG0_ENABLE, 0),
            (DISPCNT_BG1_ENABLE, 1),
            (DISPCNT_BG2_ENABLE, 2),
            (DISPCNT_BG3_ENABLE, 3),
        ];

        // Build sorted list of all non-transparent pixels
        // We iterate from lowest to highest priority to build the stack
        for &(enable_bit, bg_i) in bg_enables.iter().rev() {
            if dispcnt & enable_bit == 0 {
                continue;
            }
            if win_flags & (1 << bg_i) == 0 {
                continue;
            }
            let px = self.bg_lines[bg_i][x];
            if px.is_transparent {
                continue;
            }
            if px.priority < top.priority
                || (px.priority == top.priority && bg_i < layer_index(top.layer))
            {
                second = top;
                top = px;
            } else if px.priority < second.priority
                || (px.priority == second.priority && bg_i < layer_index(second.layer))
            {
                second = px;
            }
        }

        // Check OBJ
        if dispcnt & DISPCNT_OBJ_ENABLE != 0 && win_flags & (1 << 4) != 0 {
            let obj_px = self.obj_line[x];
            if !obj_px.is_transparent {
                if obj_px.priority <= top.priority {
                    second = top;
                    top = obj_px;
                } else if obj_px.priority <= second.priority {
                    second = obj_px;
                }
            }
        }

        (top, second)
    }

    /// Apply color blending effects.
    #[allow(clippy::too_many_arguments)]
    fn apply_blend(
        &self,
        top: &LayerPixel,
        second: &LayerPixel,
        blend_mode: u16,
        bldcnt: u16,
        eva: u32,
        evb: u32,
        evy: u32,
        _backdrop_color: u16,
    ) -> u16 {
        let is_first_target = is_blend_target(top.layer, bldcnt, true);
        let is_second_target = is_blend_target(second.layer, bldcnt, false);

        // OBJ semi-transparent always forces alpha blend if second target exists
        if top.obj_semi_transparent && is_second_target {
            return alpha_blend(top.color, second.color, eva, evb);
        }

        if !is_first_target {
            return top.color;
        }

        match blend_mode {
            0 => top.color, // No blending
            1 => {
                // Alpha blending (1st + 2nd target)
                if is_second_target {
                    alpha_blend(top.color, second.color, eva, evb)
                } else {
                    top.color
                }
            }
            2 => {
                // Brightness increase (toward white)
                brightness_increase(top.color, evy)
            }
            3 => {
                // Brightness decrease (toward black)
                brightness_decrease(top.color, evy)
            }
            _ => top.color,
        }
    }
}

// =============================================================================
// Helper functions
// =============================================================================

/// Convert GBA 15-bit color (xBBBBBGGGGGRRRRR) to 32-bit ARGB
pub fn gba_color_to_rgb(color: u16) -> u32 {
    let r = (color & 0x1F) as u32;
    let g = ((color >> 5) & 0x1F) as u32;
    let b = ((color >> 10) & 0x1F) as u32;
    let r8 = (r << 3) | (r >> 2);
    let g8 = (g << 3) | (g >> 2);
    let b8 = (b << 3) | (b >> 2);
    0xFF000000 | (r8 << 16) | (g8 << 8) | b8
}

/// Read a 16-bit value from I/O register array
fn read_io_u16(io: &[u8], offset: usize) -> u16 {
    if offset + 1 < io.len() {
        io[offset] as u16 | ((io[offset + 1] as u16) << 8)
    } else {
        0
    }
}

/// Read a signed 16-bit value from I/O register array
fn read_io_i16(io: &[u8], offset: usize) -> i16 {
    read_io_u16(io, offset) as i16
}

/// Read a 28-bit signed fixed-point value (20.8) from I/O registers
fn read_io_28bit_signed(io: &[u8], offset: usize) -> i32 {
    let lo = read_io_u16(io, offset) as u32;
    let hi = read_io_u16(io, offset + 2) as u32;
    let val = lo | (hi << 16);
    // Sign extend from 28 bits
    if val & (1 << 27) != 0 {
        (val | 0xF0000000) as i32
    } else {
        val as i32
    }
}

/// Look up a color from BG palette RAM (first 512 bytes)
fn palette_lookup(palette: &[u8], index: usize) -> u16 {
    let offset = index * 2;
    if offset + 1 < palette.len() {
        palette[offset] as u16 | ((palette[offset + 1] as u16) << 8)
    } else {
        0
    }
}

/// Map BG index to PixelLayer enum
fn bg_layer(idx: usize) -> PixelLayer {
    match idx {
        0 => PixelLayer::Bg0,
        1 => PixelLayer::Bg1,
        2 => PixelLayer::Bg2,
        3 => PixelLayer::Bg3,
        _ => PixelLayer::Backdrop,
    }
}

/// Get numeric index for a layer (for sorting within same priority)
fn layer_index(layer: PixelLayer) -> usize {
    match layer {
        PixelLayer::Bg0 => 0,
        PixelLayer::Bg1 => 1,
        PixelLayer::Bg2 => 2,
        PixelLayer::Bg3 => 3,
        PixelLayer::Obj => 4,
        PixelLayer::Backdrop => 5,
    }
}

/// Check if a layer is a blend target (1st or 2nd)
fn is_blend_target(layer: PixelLayer, bldcnt: u16, first: bool) -> bool {
    let bit = match (layer, first) {
        (PixelLayer::Bg0, true) => BLDCNT_BG0_1ST,
        (PixelLayer::Bg1, true) => BLDCNT_BG1_1ST,
        (PixelLayer::Bg2, true) => BLDCNT_BG2_1ST,
        (PixelLayer::Bg3, true) => BLDCNT_BG3_1ST,
        (PixelLayer::Obj, true) => BLDCNT_OBJ_1ST,
        (PixelLayer::Backdrop, true) => BLDCNT_BD_1ST,
        (PixelLayer::Bg0, false) => BLDCNT_BG0_2ND,
        (PixelLayer::Bg1, false) => BLDCNT_BG1_2ND,
        (PixelLayer::Bg2, false) => BLDCNT_BG2_2ND,
        (PixelLayer::Bg3, false) => BLDCNT_BG3_2ND,
        (PixelLayer::Obj, false) => BLDCNT_OBJ_2ND,
        (PixelLayer::Backdrop, false) => BLDCNT_BD_2ND,
    };
    bldcnt & bit != 0
}

/// Check if a point is inside a window
fn in_window(x: u32, y: u32, winh: u16, winv: u16) -> bool {
    let x1 = (winh >> 8) as u32;
    let x2 = (winh & 0xFF) as u32;
    let y1 = (winv >> 8) as u32;
    let y2 = (winv & 0xFF) as u32;

    let in_h = if x1 <= x2 {
        x >= x1 && x < x2
    } else {
        // Wrapping: x1 > x2 means outside [x2, x1)
        x >= x1 || x < x2
    };

    let in_v = if y1 <= y2 {
        y >= y1 && y < y2
    } else {
        y >= y1 || y < y2
    };

    in_h && in_v
}

/// Alpha blend two 15-bit colors with coefficients EVA and EVB (0-16)
fn alpha_blend(color_a: u16, color_b: u16, eva: u32, evb: u32) -> u16 {
    let r_a = (color_a & 0x1F) as u32;
    let g_a = ((color_a >> 5) & 0x1F) as u32;
    let b_a = ((color_a >> 10) & 0x1F) as u32;

    let r_b = (color_b & 0x1F) as u32;
    let g_b = ((color_b >> 5) & 0x1F) as u32;
    let b_b = ((color_b >> 10) & 0x1F) as u32;

    let r = ((r_a * eva + r_b * evb) >> 4).min(31);
    let g = ((g_a * eva + g_b * evb) >> 4).min(31);
    let b = ((b_a * eva + b_b * evb) >> 4).min(31);

    (r as u16) | ((g as u16) << 5) | ((b as u16) << 10)
}

/// Increase brightness of a 15-bit color toward white by EVY (0-16)
fn brightness_increase(color: u16, evy: u32) -> u16 {
    let r = (color & 0x1F) as u32;
    let g = ((color >> 5) & 0x1F) as u32;
    let b = ((color >> 10) & 0x1F) as u32;

    let r = r + (((31 - r) * evy) >> 4);
    let g = g + (((31 - g) * evy) >> 4);
    let b = b + (((31 - b) * evy) >> 4);

    (r.min(31) as u16) | ((g.min(31) as u16) << 5) | ((b.min(31) as u16) << 10)
}

/// Decrease brightness of a 15-bit color toward black by EVY (0-16)
fn brightness_decrease(color: u16, evy: u32) -> u16 {
    let r = (color & 0x1F) as u32;
    let g = ((color >> 5) & 0x1F) as u32;
    let b = ((color >> 10) & 0x1F) as u32;

    let r = r - ((r * evy) >> 4);
    let g = g - ((g * evy) >> 4);
    let b = b - ((b * evy) >> 4);

    (r as u16) | ((g as u16) << 5) | ((b as u16) << 10)
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gba_color_to_rgb() {
        // Pure red (R=31, G=0, B=0)
        let rgb = gba_color_to_rgb(0x001F);
        assert_eq!(rgb, 0xFF_FF_00_00); // 0xFFFF0000

        // Pure green (R=0, G=31, B=0)
        let rgb = gba_color_to_rgb(0x03E0);
        assert_eq!(rgb, 0xFF_00_FF_00);

        // Pure blue (R=0, G=0, B=31)
        let rgb = gba_color_to_rgb(0x7C00);
        assert_eq!(rgb, 0xFF_00_00_FF);

        // Black
        let rgb = gba_color_to_rgb(0x0000);
        assert_eq!(rgb, 0xFF_00_00_00);

        // White
        let rgb = gba_color_to_rgb(0x7FFF);
        assert_eq!(rgb, 0xFF_FF_FF_FF);
    }

    #[test]
    fn test_alpha_blend() {
        // 50/50 blend of white and black
        let result = alpha_blend(0x7FFF, 0x0000, 8, 8);
        let r = result & 0x1F;
        let g = (result >> 5) & 0x1F;
        let b = (result >> 10) & 0x1F;
        // 31 * 8 / 16 = 15.5 -> 15
        assert_eq!(r, 15);
        assert_eq!(g, 15);
        assert_eq!(b, 15);
    }

    #[test]
    fn test_alpha_blend_full_first() {
        // 100% first target
        let result = alpha_blend(0x7FFF, 0x0000, 16, 0);
        assert_eq!(result, 0x7FFF);
    }

    #[test]
    fn test_brightness_increase() {
        // Full increase = white
        let result = brightness_increase(0x0000, 16);
        assert_eq!(result, 0x7FFF);

        // No increase
        let result = brightness_increase(0x001F, 0);
        assert_eq!(result, 0x001F);
    }

    #[test]
    fn test_brightness_decrease() {
        // Full decrease = black
        let result = brightness_decrease(0x7FFF, 16);
        assert_eq!(result, 0x0000);

        // No decrease
        let result = brightness_decrease(0x001F, 0);
        assert_eq!(result, 0x001F);
    }

    #[test]
    fn test_palette_lookup() {
        let mut palette = vec![0u8; 512];
        // Set palette entry 1 to pure red
        palette[2] = 0x1F;
        palette[3] = 0x00;
        assert_eq!(palette_lookup(&palette, 1), 0x001F);
    }

    #[test]
    fn test_window_check() {
        // Simple window: x=[10, 50), y=[20, 100)
        let winh = (10 << 8) | 50;
        let winv = (20 << 8) | 100;

        assert!(in_window(10, 20, winh, winv));
        assert!(in_window(49, 99, winh, winv));
        assert!(!in_window(50, 20, winh, winv));
        assert!(!in_window(9, 20, winh, winv));
        assert!(!in_window(10, 100, winh, winv));
    }

    #[test]
    fn test_window_wrap() {
        // Wrapping window: x=[200, 50) (wraps around)
        let winh = (200 << 8) | 50;
        let winv = 160;

        assert!(in_window(210, 80, winh, winv));
        assert!(in_window(10, 80, winh, winv));
        assert!(!in_window(100, 80, winh, winv));
    }

    #[test]
    fn test_ppu_new() {
        let ppu = Ppu::new();
        assert_eq!(ppu.frame().width, SCREEN_WIDTH as u32);
        assert_eq!(ppu.frame().height, SCREEN_HEIGHT as u32);
    }

    #[test]
    fn test_forced_blank() {
        let mut ppu = Ppu::new();
        let mut io = vec![0u8; 0x400];
        let palette = vec![0u8; 0x400];
        let vram = vec![0u8; 0x18000];
        let oam = vec![0u8; 0x400];

        // Set forced blank
        io[DISPCNT + 1] = 0; // high byte
        io[DISPCNT] = DISPCNT_FORCED_BLANK as u8;

        ppu.render_scanline(0, &io, &palette, &vram, &oam);

        // Check that scanline 0 is all white
        for x in 0..SCREEN_WIDTH {
            assert_eq!(ppu.frame.pixels[x], 0xFFFFFFFF);
        }
    }

    #[test]
    fn test_mode3_render() {
        let mut ppu = Ppu::new();
        let mut io = vec![0u8; 0x400];
        let palette = vec![0u8; 0x400];
        let mut vram = vec![0u8; 0x18000];
        let oam = vec![0u8; 0x400];

        // Mode 3, BG2 enabled
        io[DISPCNT] = 3; // mode 3
        io[DISPCNT + 1] = (DISPCNT_BG2_ENABLE >> 8) as u8;

        // Write a red pixel at (0, 0)
        vram[0] = 0x1F; // R=31
        vram[1] = 0x00;

        ppu.render_scanline(0, &io, &palette, &vram, &oam);

        // Should be pure red
        assert_eq!(ppu.frame.pixels[0], 0xFF_FF_00_00);
    }

    #[test]
    fn test_mode4_render() {
        let mut ppu = Ppu::new();
        let mut io = vec![0u8; 0x400];
        let mut palette = vec![0u8; 0x400];
        let mut vram = vec![0u8; 0x18000];
        let oam = vec![0u8; 0x400];

        // Mode 4, BG2 enabled
        io[DISPCNT] = 4;
        io[DISPCNT + 1] = (DISPCNT_BG2_ENABLE >> 8) as u8;

        // Set palette entry 1 to green
        palette[2] = 0xE0; // G=31 in low bits
        palette[3] = 0x03; // G=31 in high bits

        // Set pixel (0,0) to palette index 1
        vram[0] = 1;

        ppu.render_scanline(0, &io, &palette, &vram, &oam);

        assert_eq!(ppu.frame.pixels[0], gba_color_to_rgb(0x03E0));
    }

    #[test]
    fn test_text_bg_simple() {
        let mut ppu = Ppu::new();
        let mut io = vec![0u8; 0x400];
        let mut palette = vec![0u8; 0x400];
        let mut vram = vec![0u8; 0x18000];
        let oam = vec![0u8; 0x400];

        // Mode 0, BG0 enabled
        io[DISPCNT] = 0;
        io[DISPCNT + 1] = (DISPCNT_BG0_ENABLE >> 8) as u8;

        // BG0CNT: priority=0, tile_base=0, 4bpp, map_base=0, size=0 (32x32)
        io[BG0CNT] = 0;
        io[BG0CNT + 1] = 0;

        // Set palette entry 1 (in sub-palette 0) to blue
        palette[2] = 0x00;
        palette[3] = 0x7C; // B=31

        // Create a simple tile at tile 1: first row, first pixel = palette index 1
        // 4bpp: 32 bytes per tile, each byte = 2 pixels
        // Tile 1 starts at offset 32
        vram[32] = 0x01; // pixel 0 = palette 1, pixel 1 = 0

        // Map entry at (0,0): tile 1, palette 0, no flip
        vram[0] = 1; // Tile ID = 1 (low byte in map, map base defaults to 0 but tiles start at tile_base)

        // Actually, map_base = 0 and tile_base = 0 overlap!
        // Let's put the map at a different base
        io[BG0CNT + 1] = 0x1C; // map_base = 0x0E (screen block 14 = 0x7000)

        // Put map at 0x7000
        let map_addr = 0x7000;
        vram[map_addr] = 1; // tile 1
        vram[map_addr + 1] = 0; // no flip, palette 0

        ppu.render_scanline(0, &io, &palette, &vram, &oam);

        // Pixel (0,0) should be blue
        assert_eq!(ppu.frame.pixels[0], gba_color_to_rgb(0x7C00));
    }

    #[test]
    fn test_obj_sizes() {
        // Verify sprite size table
        assert_eq!(OBJ_SIZES[0][0], (8, 8));
        assert_eq!(OBJ_SIZES[0][3], (64, 64));
        assert_eq!(OBJ_SIZES[1][0], (16, 8));
        assert_eq!(OBJ_SIZES[2][0], (8, 16));
    }

    #[test]
    fn test_read_io_28bit_signed() {
        let mut io = vec![0u8; 0x400];

        // Positive value: 0x00012345
        io[BG2X] = 0x45;
        io[BG2X + 1] = 0x23;
        io[BG2X + 2] = 0x01;
        io[BG2X + 3] = 0x00;
        assert_eq!(read_io_28bit_signed(&io, BG2X), 0x00012345);

        // Negative value: sign bit set (bit 27)
        io[BG2X + 3] = 0x0F; // bits 24-27 = 0xF, bit 27 set
        let val = read_io_28bit_signed(&io, BG2X);
        assert!(val < 0);
    }
}
