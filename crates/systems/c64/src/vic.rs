//! VIC-II (MOS 6569 PAL / MOS 6567 NTSC) Video Interface Chip
//!
//! ## Features
//! - 320×200 standard character mode
//! - 160×200 multicolor character mode
//! - 320×200 standard bitmap mode
//! - 160×200 multicolor bitmap mode
//! - Extended background color mode
//! - 8 hardware sprites (24×21 standard, 24×21 multicolor, expandable)
//! - Sprite-sprite and sprite-background collision detection
//! - Raster interrupt generation
//! - Smooth scrolling (3-bit X/Y)
//! - Border control (38/40 columns, 24/25 rows)
//!
//! ## PAL Timing
//! - 63 cycles per raster line
//! - 312 raster lines per frame (0–311)
//! - Visible area: lines 51–250 (200 lines), columns ~24–343 (320 pixels)
//! - Frame rate: ~50.125 Hz
//!
//! ## NTSC Timing
//! - 65 cycles per raster line
//! - 263 raster lines per frame
//! - Frame rate: ~59.826 Hz

use emu_core::types::Frame;

// PAL timing
pub const PAL_CYCLES_PER_LINE: u32 = 63;
pub const PAL_LINES_PER_FRAME: u32 = 312;
pub const PAL_CYCLES_PER_FRAME: u32 = PAL_CYCLES_PER_LINE * PAL_LINES_PER_FRAME;

// NTSC timing
pub const NTSC_CYCLES_PER_LINE: u32 = 65;
pub const NTSC_LINES_PER_FRAME: u32 = 263;
pub const NTSC_CYCLES_PER_FRAME: u32 = NTSC_CYCLES_PER_LINE * NTSC_LINES_PER_FRAME;

// Display constants
const VISIBLE_WIDTH: u32 = 320;
const VISIBLE_HEIGHT: u32 = 200;
const FIRST_VISIBLE_LINE: u32 = 51; // PAL first visible raster line
const LAST_VISIBLE_LINE: u32 = FIRST_VISIBLE_LINE + VISIBLE_HEIGHT - 1;

// Border area constants
const FIRST_DISPLAY_COL: u32 = 24; // First pixel of display window

/// C64 color palette (VICE-style colors)
/// 16 colors indexed 0–15
const PALETTE: [u32; 16] = [
    0x00_0000, // 0: Black
    0xFF_FFFF, // 1: White
    0x88_0000, // 2: Red
    0xAA_FFEE, // 3: Cyan
    0xCC_44CC, // 4: Purple
    0x00_CC55, // 5: Green
    0x00_00AA, // 6: Blue
    0xEE_EE77, // 7: Yellow
    0xDD_8855, // 8: Orange
    0x66_4400, // 9: Brown
    0xFF_7777, // 10: Light Red
    0x33_3333, // 11: Dark Grey
    0x77_7777, // 12: Medium Grey
    0xAA_FF66, // 13: Light Green
    0x00_88FF, // 14: Light Blue
    0xBB_BBBB, // 15: Light Grey
];

/// VIC-II state
pub struct Vic {
    /// VIC-II registers ($D000–$D03F, 64 bytes; only 47 used)
    pub regs: [u8; 64],

    /// Current raster line (0–311 PAL, 0–262 NTSC)
    raster_line: u32,
    /// Cycle within current raster line (0–62 PAL)
    cycle_in_line: u32,

    /// Frame buffer (320×200)
    frame: Frame,

    /// IRQ line active (directly drives CPU IRQ pin)
    pub irq_line: bool,

    /// Sprite-sprite collision register (latched, cleared on read)
    sprite_sprite_collision: u8,
    /// Sprite-background collision register (latched, cleared on read)
    sprite_bg_collision: u8,

    /// Whether this is a PAL or NTSC VIC-II
    is_pal: bool,

    /// Light pen X/Y (latched)
    light_pen_x: u8,
    light_pen_y: u8,

    /// Bad line state: true when VIC-II steals cycles from CPU
    pub is_bad_line: bool,

    /// VRAM: 16KB bank visible to VIC-II (synced from main RAM by system)
    pub vram: Vec<u8>,
    /// Color RAM: 1KB (synced from bus)
    pub color_ram: Vec<u8>,
}

impl Vic {
    pub fn new() -> Self {
        let mut vic = Self {
            regs: [0u8; 64],
            raster_line: 0,
            cycle_in_line: 0,
            frame: Frame::new(VISIBLE_WIDTH, VISIBLE_HEIGHT),
            irq_line: false,
            sprite_sprite_collision: 0,
            sprite_bg_collision: 0,
            is_pal: true,
            light_pen_x: 0,
            light_pen_y: 0,
            is_bad_line: false,
            vram: vec![0u8; 0x4000],
            color_ram: vec![0u8; 0x0400],
        };
        // Default register values matching power-on state
        vic.regs[0x11] = 0x1B; // DEN=1, RSEL=1, YSCROLL=3
        vic.regs[0x16] = 0xC8; // MCM=0, CSEL=1, XSCROLL=0
        vic.regs[0x18] = 0x15; // Screen at $0400, chars at $1000
        vic.regs[0x19] = 0x00; // No IRQ pending
        vic.regs[0x1A] = 0x00; // No IRQ enabled
        vic.regs[0x20] = 0x0E; // Border = light blue
        vic.regs[0x21] = 0x06; // Background = blue
        vic
    }

    pub fn reset(&mut self) {
        let new = Self::new();
        self.regs = new.regs;
        self.raster_line = 0;
        self.cycle_in_line = 0;
        self.irq_line = false;
        self.sprite_sprite_collision = 0;
        self.sprite_bg_collision = 0;
        self.is_bad_line = false;
        self.frame = Frame::new(VISIBLE_WIDTH, VISIBLE_HEIGHT);
    }

    /// Get cycles per line based on timing mode
    pub fn cycles_per_line(&self) -> u32 {
        if self.is_pal {
            PAL_CYCLES_PER_LINE
        } else {
            NTSC_CYCLES_PER_LINE
        }
    }

    /// Get lines per frame based on timing mode
    pub fn lines_per_frame(&self) -> u32 {
        if self.is_pal {
            PAL_LINES_PER_FRAME
        } else {
            NTSC_LINES_PER_FRAME
        }
    }

    /// Tick VIC-II by one CPU cycle. Returns true when a full frame is complete.
    pub fn tick(&mut self) -> bool {
        let mut frame_complete = false;
        let cpl = self.cycles_per_line();
        let lpf = self.lines_per_frame();

        self.cycle_in_line += 1;
        if self.cycle_in_line >= cpl {
            self.cycle_in_line = 0;
            self.raster_line += 1;

            if self.raster_line >= lpf {
                self.raster_line = 0;
                frame_complete = true;
            }

            // Update raster counter registers
            self.regs[0x12] = (self.raster_line & 0xFF) as u8;
            self.regs[0x11] = (self.regs[0x11] & 0x7F) | (((self.raster_line >> 8) & 1) as u8) << 7;

            // Check for raster IRQ
            let irq_line = self.raster_irq_line();
            if self.raster_line == irq_line {
                // Set raster IRQ flag
                self.regs[0x19] |= 0x01;
                // If raster IRQ is enabled in mask
                if self.regs[0x1A] & 0x01 != 0 {
                    self.regs[0x19] |= 0x80; // Set IRQ flag
                    self.irq_line = true;
                }
            }

            // Detect bad lines: when DEN=1, YSCROLL matches raster[0:2], and in display area
            let den = self.regs[0x11] & 0x10 != 0;
            let yscroll = (self.regs[0x11] & 0x07) as u32;
            self.is_bad_line = den
                && self.raster_line >= 0x30
                && self.raster_line <= 0xF7
                && (self.raster_line & 7) == yscroll;
        }

        // Render visible lines at the end of the line
        if self.cycle_in_line == 0 && self.raster_line > 0 {
            let prev_line = self.raster_line - 1;
            if (FIRST_VISIBLE_LINE..=LAST_VISIBLE_LINE).contains(&prev_line) {
                self.render_line(prev_line - FIRST_VISIBLE_LINE);
            }
        }

        frame_complete
    }

    /// Get the raster line for IRQ comparison
    fn raster_irq_line(&self) -> u32 {
        let lo = self.regs[0x12] as u32;
        let hi = ((self.regs[0x11] >> 7) & 1) as u32;
        lo | (hi << 8)
    }

    /// Render one visible line (0–199)
    fn render_line(&mut self, visible_y: u32) {
        let den = self.regs[0x11] & 0x10 != 0;
        let border = PALETTE[(self.regs[0x20] & 0x0F) as usize];
        let offset = (visible_y * VISIBLE_WIDTH) as usize;

        if !den {
            // Display not enabled - render border color
            for x in 0..VISIBLE_WIDTH as usize {
                if offset + x < self.frame.pixels.len() {
                    self.frame.pixels[offset + x] = border;
                }
            }
            return;
        }

        let bmm = self.regs[0x11] & 0x20 != 0; // Bitmap mode
        let ecm = self.regs[0x11] & 0x40 != 0; // Extended color mode
        let mcm = self.regs[0x16] & 0x10 != 0; // Multicolor mode

        // XSCROLL ($D016 bits 0-2): shifts display right by 0–7 pixels.
        // YSCROLL ($D011 bits 0-2): shifts the character row start (default 3).
        let xscroll = (self.regs[0x16] & 0x07) as usize;
        let yscroll = (self.regs[0x11] & 0x07) as i32;

        // Effective Y for character row / pixel-row calculations.
        // With YSCROLL=3 (default): effective_y = visible_y (no change).
        let effective_y = ((visible_y as i32) + 3 - yscroll).max(0) as u32;

        // Pre-fill the entire line with border color.
        // This ensures the leftmost `xscroll` pixels (before the character display starts)
        // and the rightmost `xscroll` pixels (where the shifted display exceeds VISIBLE_WIDTH)
        // both show the border color without stale data from earlier frames.
        for x in 0..VISIBLE_WIDTH as usize {
            let idx = offset + x;
            if idx < self.frame.pixels.len() {
                self.frame.pixels[idx] = border;
            }
        }

        // Foreground-pixel mask for the current line (used by sprite priority and
        // sprite-background collision detection). A pixel is "foreground" when it is
        // set by a character or bitmap waveform (not the background color).
        let mut fg_mask = [false; VISIBLE_WIDTH as usize];

        if bmm {
            if mcm {
                self.render_multicolor_bitmap(visible_y, effective_y, xscroll, &mut fg_mask);
            } else {
                self.render_standard_bitmap(visible_y, effective_y, xscroll, &mut fg_mask);
            }
        } else if ecm {
            self.render_ecm(visible_y, effective_y, xscroll, &mut fg_mask);
        } else if mcm {
            self.render_multicolor_text(visible_y, effective_y, xscroll, &mut fg_mask);
        } else {
            self.render_standard_text(visible_y, effective_y, xscroll, &mut fg_mask);
        }

        // Render sprites on top (with priority and collision detection)
        self.render_sprites(visible_y, &fg_mask);
    }

    /// Render standard text mode (40×25 characters, 8×8 pixels each)
    fn render_standard_text(
        &mut self,
        visible_y: u32,
        effective_y: u32,
        xscroll: usize,
        fg_mask: &mut [bool; VISIBLE_WIDTH as usize],
    ) {
        let bg_color = PALETTE[(self.regs[0x21] & 0x0F) as usize];
        let char_row = (effective_y / 8) as usize;
        let pixel_row = (effective_y % 8) as usize;
        let offset = (visible_y * VISIBLE_WIDTH) as usize;

        for col in 0..40usize {
            let screen_code = self.read_screen_byte(char_row, col);
            let color_idx = self.read_color_byte(char_row, col) & 0x0F;
            let fg_color = PALETTE[color_idx as usize];
            let char_pixels = self.read_char_byte(screen_code as usize, pixel_row);

            for bit in 0..8usize {
                let px = col * 8 + bit + xscroll;
                if px >= VISIBLE_WIDTH as usize {
                    continue;
                }
                let set = char_pixels & (0x80 >> bit) != 0;
                let color = if set { fg_color } else { bg_color };
                let idx = offset + px;
                if idx < self.frame.pixels.len() {
                    self.frame.pixels[idx] = color;
                }
                if set {
                    fg_mask[px] = true;
                }
            }
        }
    }

    /// Render multicolor text mode
    fn render_multicolor_text(
        &mut self,
        visible_y: u32,
        effective_y: u32,
        xscroll: usize,
        fg_mask: &mut [bool; VISIBLE_WIDTH as usize],
    ) {
        let bg0 = PALETTE[(self.regs[0x21] & 0x0F) as usize];
        let bg1 = PALETTE[(self.regs[0x22] & 0x0F) as usize];
        let bg2 = PALETTE[(self.regs[0x23] & 0x0F) as usize];
        let char_row = (effective_y / 8) as usize;
        let pixel_row = (effective_y % 8) as usize;
        let offset = (visible_y * VISIBLE_WIDTH) as usize;

        for col in 0..40usize {
            let screen_code = self.read_screen_byte(char_row, col);
            let color_byte = self.read_color_byte(char_row, col);
            let is_multicolor = color_byte & 0x08 != 0;

            if is_multicolor {
                let fg_color = PALETTE[(color_byte & 0x07) as usize];
                let char_pixels = self.read_char_byte(screen_code as usize, pixel_row);

                for pair in 0..4usize {
                    let bits = (char_pixels >> (6 - pair * 2)) & 0x03;
                    let color = match bits {
                        0 => bg0,
                        1 => bg1,
                        2 => bg2,
                        3 => fg_color,
                        _ => unreachable!(),
                    };
                    let is_fg = bits != 0;
                    for sub in 0..2usize {
                        let px = col * 8 + pair * 2 + sub + xscroll;
                        if px >= VISIBLE_WIDTH as usize {
                            continue;
                        }
                        let idx = offset + px;
                        if idx < self.frame.pixels.len() {
                            self.frame.pixels[idx] = color;
                        }
                        if is_fg {
                            fg_mask[px] = true;
                        }
                    }
                }
            } else {
                // Non-multicolor character rendered as standard
                let fg_color = PALETTE[(color_byte & 0x0F) as usize];
                let char_pixels = self.read_char_byte(screen_code as usize, pixel_row);
                for bit in 0..8usize {
                    let px = col * 8 + bit + xscroll;
                    if px >= VISIBLE_WIDTH as usize {
                        continue;
                    }
                    let set = char_pixels & (0x80 >> bit) != 0;
                    let color = if set { fg_color } else { bg0 };
                    let idx = offset + px;
                    if idx < self.frame.pixels.len() {
                        self.frame.pixels[idx] = color;
                    }
                    if set {
                        fg_mask[px] = true;
                    }
                }
            }
        }
    }

    /// Render standard bitmap mode (320×200, 2 colors per 8×8 cell)
    fn render_standard_bitmap(
        &mut self,
        visible_y: u32,
        effective_y: u32,
        xscroll: usize,
        fg_mask: &mut [bool; VISIBLE_WIDTH as usize],
    ) {
        let char_row = (effective_y / 8) as usize;
        let pixel_row = (effective_y % 8) as usize;
        let offset = (visible_y * VISIBLE_WIDTH) as usize;

        for col in 0..40usize {
            let screen_byte = self.read_screen_byte(char_row, col);
            let fg_color = PALETTE[((screen_byte >> 4) & 0x0F) as usize];
            let bg_color = PALETTE[(screen_byte & 0x0F) as usize];

            let bitmap_addr = char_row * 40 * 8 + col * 8 + pixel_row;
            let bitmap_byte = self.read_bitmap_byte(bitmap_addr);

            for bit in 0..8usize {
                let px = col * 8 + bit + xscroll;
                if px >= VISIBLE_WIDTH as usize {
                    continue;
                }
                let set = bitmap_byte & (0x80 >> bit) != 0;
                let color = if set { fg_color } else { bg_color };
                let idx = offset + px;
                if idx < self.frame.pixels.len() {
                    self.frame.pixels[idx] = color;
                }
                if set {
                    fg_mask[px] = true;
                }
            }
        }
    }

    /// Render multicolor bitmap mode (160×200, 4 colors per 4×8 cell)
    fn render_multicolor_bitmap(
        &mut self,
        visible_y: u32,
        effective_y: u32,
        xscroll: usize,
        fg_mask: &mut [bool; VISIBLE_WIDTH as usize],
    ) {
        let bg0 = PALETTE[(self.regs[0x21] & 0x0F) as usize];
        let char_row = (effective_y / 8) as usize;
        let pixel_row = (effective_y % 8) as usize;
        let offset = (visible_y * VISIBLE_WIDTH) as usize;

        for col in 0..40usize {
            let screen_byte = self.read_screen_byte(char_row, col);
            let color_byte = self.read_color_byte(char_row, col);

            let c1 = PALETTE[((screen_byte >> 4) & 0x0F) as usize];
            let c2 = PALETTE[(screen_byte & 0x0F) as usize];
            let c3 = PALETTE[(color_byte & 0x0F) as usize];

            let bitmap_addr = char_row * 40 * 8 + col * 8 + pixel_row;
            let bitmap_byte = self.read_bitmap_byte(bitmap_addr);

            for pair in 0..4usize {
                let bits = (bitmap_byte >> (6 - pair * 2)) & 0x03;
                let color = match bits {
                    0 => bg0,
                    1 => c1,
                    2 => c2,
                    3 => c3,
                    _ => unreachable!(),
                };
                let is_fg = bits != 0;
                for sub in 0..2usize {
                    let px = col * 8 + pair * 2 + sub + xscroll;
                    if px >= VISIBLE_WIDTH as usize {
                        continue;
                    }
                    let idx = offset + px;
                    if idx < self.frame.pixels.len() {
                        self.frame.pixels[idx] = color;
                    }
                    if is_fg {
                        fg_mask[px] = true;
                    }
                }
            }
        }
    }

    /// Render Extended Color Mode (ECM)
    fn render_ecm(
        &mut self,
        visible_y: u32,
        effective_y: u32,
        xscroll: usize,
        fg_mask: &mut [bool; VISIBLE_WIDTH as usize],
    ) {
        let bg_colors = [
            PALETTE[(self.regs[0x21] & 0x0F) as usize],
            PALETTE[(self.regs[0x22] & 0x0F) as usize],
            PALETTE[(self.regs[0x23] & 0x0F) as usize],
            PALETTE[(self.regs[0x24] & 0x0F) as usize],
        ];
        let char_row = (effective_y / 8) as usize;
        let pixel_row = (effective_y % 8) as usize;
        let offset = (visible_y * VISIBLE_WIDTH) as usize;

        for col in 0..40usize {
            let screen_code = self.read_screen_byte(char_row, col);
            let color_idx = self.read_color_byte(char_row, col) & 0x0F;
            let fg_color = PALETTE[color_idx as usize];

            // In ECM, bits 6-7 of screen code select background color
            let bg_sel = (screen_code >> 6) & 0x03;
            let bg_color = bg_colors[bg_sel as usize];
            let char_code = screen_code & 0x3F; // Only lower 6 bits used for char

            let char_pixels = self.read_char_byte(char_code as usize, pixel_row);

            for bit in 0..8usize {
                let px = col * 8 + bit + xscroll;
                if px >= VISIBLE_WIDTH as usize {
                    continue;
                }
                let set = char_pixels & (0x80 >> bit) != 0;
                let color = if set { fg_color } else { bg_color };
                let idx = offset + px;
                if idx < self.frame.pixels.len() {
                    self.frame.pixels[idx] = color;
                }
                if set {
                    fg_mask[px] = true;
                }
            }
        }
    }

    /// Render sprites for a visible line, detecting sprite-background and
    /// sprite-sprite collisions and enforcing the per-sprite priority bit.
    fn render_sprites(&mut self, visible_y: u32, fg_mask: &[bool; VISIBLE_WIDTH as usize]) {
        let sprite_enable = self.regs[0x15];
        if sprite_enable == 0 {
            return;
        }

        let raster = visible_y + FIRST_VISIBLE_LINE;
        let offset = (visible_y * VISIBLE_WIDTH) as usize;

        // Per-pixel sprite presence bitmask for this line.
        // Each byte stores a bitmask of which sprite numbers have a non-transparent pixel
        // at that screen position.  When a second sprite writes to the same pixel, both
        // the current sprite's bit AND the already-present sprite bits are latched into
        // sprite_sprite_collision, matching real VIC-II behaviour.
        let mut sprite_pixel_mask = [0u8; VISIBLE_WIDTH as usize];

        // Save the pre-rendering collision state so we can detect 0→1 transitions.
        let prev_bg_collision = self.sprite_bg_collision;
        let prev_sprite_collision = self.sprite_sprite_collision;

        // Priority register: bit N set means sprite N renders behind background.
        let priority_reg = self.regs[0x1B];

        // Sprites are drawn in reverse order (sprite 0 has highest priority / drawn last).
        for sprite in (0..8usize).rev() {
            if sprite_enable & (1 << sprite) == 0 {
                continue;
            }

            let sprite_x =
                self.regs[sprite * 2] as u32 | (((self.regs[0x10] >> sprite) & 1) as u32) << 8;
            let sprite_y = self.regs[sprite * 2 + 1] as u32;
            let y_expand = self.regs[0x17] & (1 << sprite) != 0;
            let x_expand = self.regs[0x1D] & (1 << sprite) != 0;
            let multicolor = self.regs[0x1C] & (1 << sprite) != 0;
            let behind_bg = priority_reg & (1 << sprite) != 0;

            let sprite_height: u32 = if y_expand { 42 } else { 21 };

            // Check if this raster line intersects the sprite
            if raster < sprite_y || raster >= sprite_y + sprite_height {
                continue;
            }

            let line_in_sprite = if y_expand {
                (raster - sprite_y) / 2
            } else {
                raster - sprite_y
            } as usize;

            let sprite_color = PALETTE[(self.regs[0x27 + sprite] & 0x0F) as usize];
            let mc0 = PALETTE[(self.regs[0x25] & 0x0F) as usize]; // Sprite multicolor 0
            let mc1 = PALETTE[(self.regs[0x26] & 0x0F) as usize]; // Sprite multicolor 1

            // Sprite data is at pointer*64 + line*3
            let sprite_ptr = self.read_sprite_pointer(sprite);
            let data_base = sprite_ptr as usize * 64 + line_in_sprite * 3;

            for byte_idx in 0..3usize {
                let byte = self.read_sprite_data_byte(data_base + byte_idx);

                if multicolor {
                    // Multicolor sprite: 2 bits per pixel (12 pixels wide per byte)
                    for pair in 0..4usize {
                        let bits = (byte >> (6 - pair * 2)) & 0x03;
                        if bits == 0 {
                            continue; // Transparent
                        }
                        let color = match bits {
                            1 => mc0,
                            2 => sprite_color,
                            3 => mc1,
                            _ => continue,
                        };

                        for sub in 0..2usize {
                            let pixel_offset = byte_idx * 8 + pair * 2 + sub;
                            let px = if x_expand {
                                sprite_x as usize + pixel_offset * 2
                            } else {
                                sprite_x as usize + pixel_offset
                            };
                            // Adjust for border offset
                            if px >= FIRST_DISPLAY_COL as usize
                                && px < (FIRST_DISPLAY_COL + VISIBLE_WIDTH) as usize
                            {
                                let screen_px = px - FIRST_DISPLAY_COL as usize;

                                // Sprite-background collision
                                if fg_mask[screen_px] {
                                    self.sprite_bg_collision |= 1 << sprite;
                                }

                                // Sprite-sprite collision: latch bits for both sprites
                                let present = sprite_pixel_mask[screen_px];
                                if present != 0 {
                                    self.sprite_sprite_collision |= (1 << sprite) | present;
                                }
                                sprite_pixel_mask[screen_px] |= 1 << sprite;

                                // Sprite priority: skip if behind background fg pixel
                                if behind_bg && fg_mask[screen_px] {
                                    continue;
                                }

                                let idx = offset + screen_px;
                                if idx < self.frame.pixels.len() {
                                    self.frame.pixels[idx] = color;
                                }
                            }
                            if x_expand {
                                let px2 = px + 1;
                                if px2 >= FIRST_DISPLAY_COL as usize
                                    && px2 < (FIRST_DISPLAY_COL + VISIBLE_WIDTH) as usize
                                {
                                    let screen_px2 = px2 - FIRST_DISPLAY_COL as usize;

                                    if fg_mask[screen_px2] {
                                        self.sprite_bg_collision |= 1 << sprite;
                                    }
                                    let present2 = sprite_pixel_mask[screen_px2];
                                    if present2 != 0 {
                                        self.sprite_sprite_collision |= (1 << sprite) | present2;
                                    }
                                    sprite_pixel_mask[screen_px2] |= 1 << sprite;

                                    if behind_bg && fg_mask[screen_px2] {
                                        continue;
                                    }

                                    let idx = offset + screen_px2;
                                    if idx < self.frame.pixels.len() {
                                        self.frame.pixels[idx] = color;
                                    }
                                }
                            }
                        }
                    }
                } else {
                    // Standard sprite: 1 bit per pixel (24 pixels wide)
                    for bit in 0..8usize {
                        if byte & (0x80 >> bit) == 0 {
                            continue;
                        }
                        let pixel_offset = byte_idx * 8 + bit;
                        let px = if x_expand {
                            sprite_x as usize + pixel_offset * 2
                        } else {
                            sprite_x as usize + pixel_offset
                        };

                        if px >= FIRST_DISPLAY_COL as usize
                            && px < (FIRST_DISPLAY_COL + VISIBLE_WIDTH) as usize
                        {
                            let screen_px = px - FIRST_DISPLAY_COL as usize;

                            // Sprite-background collision
                            if fg_mask[screen_px] {
                                self.sprite_bg_collision |= 1 << sprite;
                            }

                            // Sprite-sprite collision: latch bits for both sprites
                            let present = sprite_pixel_mask[screen_px];
                            if present != 0 {
                                self.sprite_sprite_collision |= (1 << sprite) | present;
                            }
                            sprite_pixel_mask[screen_px] |= 1 << sprite;

                            // Sprite priority: skip if behind background fg pixel
                            if behind_bg && fg_mask[screen_px] {
                                continue;
                            }

                            let idx = offset + screen_px;
                            if idx < self.frame.pixels.len() {
                                self.frame.pixels[idx] = sprite_color;
                            }
                        }
                        if x_expand {
                            let px2 = px + 1;
                            if px2 >= FIRST_DISPLAY_COL as usize
                                && px2 < (FIRST_DISPLAY_COL + VISIBLE_WIDTH) as usize
                            {
                                let screen_px2 = px2 - FIRST_DISPLAY_COL as usize;

                                if fg_mask[screen_px2] {
                                    self.sprite_bg_collision |= 1 << sprite;
                                }
                                let present2 = sprite_pixel_mask[screen_px2];
                                if present2 != 0 {
                                    self.sprite_sprite_collision |= (1 << sprite) | present2;
                                }
                                sprite_pixel_mask[screen_px2] |= 1 << sprite;

                                if behind_bg && fg_mask[screen_px2] {
                                    continue;
                                }

                                let idx = offset + screen_px2;
                                if idx < self.frame.pixels.len() {
                                    self.frame.pixels[idx] = sprite_color;
                                }
                            }
                        }
                    }
                }
            }
        }

        // Trigger collision IRQs only for bits that are *newly* set this line (0→1
        // transition), preventing an interrupt storm on subsequent lines once the
        // CPU has acknowledged $D019 without yet reading $D01E/$D01F.
        let new_bg = self.sprite_bg_collision & !prev_bg_collision;
        if new_bg != 0 && self.regs[0x1A] & 0x02 != 0 {
            self.regs[0x19] |= 0x02 | 0x80;
            self.irq_line = true;
        }
        let new_ss = self.sprite_sprite_collision & !prev_sprite_collision;
        if new_ss != 0 && self.regs[0x1A] & 0x04 != 0 {
            self.regs[0x19] |= 0x04 | 0x80;
            self.irq_line = true;
        }
    }

    // ---- VIC-II memory access helpers ----
    // These read from the VIC-II's view of memory, which depends on the VIC bank
    // set by CIA2 Port A bits 0-1. The bus module syncs the relevant data.

    /// Read screen byte at (char_row, col) from video matrix
    fn read_screen_byte(&self, char_row: usize, col: usize) -> u8 {
        let screen_base = ((self.regs[0x18] >> 4) & 0x0F) as usize * 0x0400;
        let addr = screen_base + char_row * 40 + col;
        self.read_vic_byte(addr)
    }

    /// Read color RAM byte at (char_row, col)
    fn read_color_byte(&self, char_row: usize, col: usize) -> u8 {
        let pos = char_row * 40 + col;
        if pos < 1024 {
            self.color_ram[pos] & 0x0F
        } else {
            0
        }
    }

    /// Read character generator byte
    fn read_char_byte(&self, char_code: usize, pixel_row: usize) -> u8 {
        let char_base = ((self.regs[0x18] >> 1) & 0x07) as usize * 0x0800;
        let addr = char_base + char_code * 8 + pixel_row;
        self.read_vic_byte(addr)
    }

    /// Read bitmap byte
    fn read_bitmap_byte(&self, offset: usize) -> u8 {
        let bitmap_base = ((self.regs[0x18] >> 3) & 0x01) as usize * 0x2000;
        self.read_vic_byte(bitmap_base + offset)
    }

    /// Read sprite pointer for sprite n
    fn read_sprite_pointer(&self, sprite: usize) -> u8 {
        let screen_base = ((self.regs[0x18] >> 4) & 0x0F) as usize * 0x0400;
        let ptr_addr = screen_base + 0x03F8 + sprite;
        self.read_vic_byte(ptr_addr)
    }

    /// Read sprite data byte at absolute offset
    fn read_sprite_data_byte(&self, addr: usize) -> u8 {
        self.read_vic_byte(addr)
    }

    /// Read a byte from VIC-II's view of memory (within current 16KB bank)
    fn read_vic_byte(&self, addr: usize) -> u8 {
        let bank_addr = addr & 0x3FFF;
        if bank_addr < self.vram.len() {
            self.vram[bank_addr]
        } else {
            0
        }
    }

    // ---- Register access ----

    /// Read VIC-II register
    pub fn read_reg(&mut self, reg: u8) -> u8 {
        let r = (reg & 0x3F) as usize;
        match r {
            0x11 => {
                // Control register 1 - bit 7 is raster line bit 8
                (self.regs[0x11] & 0x7F) | (((self.raster_line >> 8) & 1) as u8) << 7
            }
            0x12 => {
                // Current raster line (low 8 bits)
                (self.raster_line & 0xFF) as u8
            }
            0x13 => self.light_pen_x,
            0x14 => self.light_pen_y,
            0x19 => {
                // Interrupt register - read and acknowledge
                self.regs[0x19] | 0x70 // Bits 4-6 always 1
            }
            0x1E => {
                // Sprite-sprite collision (cleared on read)
                let val = self.sprite_sprite_collision;
                self.sprite_sprite_collision = 0;
                val
            }
            0x1F => {
                // Sprite-background collision (cleared on read)
                let val = self.sprite_bg_collision;
                self.sprite_bg_collision = 0;
                val
            }
            0x20..=0x2E => self.regs[r] | 0xF0, // Color regs: upper 4 bits always 1
            0x2F..=0x3F => 0xFF,                // Unused registers read as $FF
            _ => self.regs[r],
        }
    }

    /// Write VIC-II register
    pub fn write_reg(&mut self, reg: u8, val: u8) {
        let r = (reg & 0x3F) as usize;
        match r {
            0x11 => {
                self.regs[0x11] = val;
                // Raster compare IRQ line may have changed - recheck
                let irq_line = self.raster_irq_line();
                if self.raster_line == irq_line {
                    self.regs[0x19] |= 0x01;
                    if self.regs[0x1A] & 0x01 != 0 {
                        self.regs[0x19] |= 0x80;
                        self.irq_line = true;
                    }
                }
            }
            0x12 => {
                self.regs[0x12] = val;
                // Raster compare line changed
            }
            0x19 => {
                // Acknowledge IRQ: writing 1 clears the corresponding bit
                self.regs[0x19] &= !val & 0x0F;
                // If no more pending IRQs, clear the IRQ line
                if self.regs[0x19] & self.regs[0x1A] & 0x0F == 0 {
                    self.regs[0x19] &= 0x7F; // Clear bit 7
                    self.irq_line = false;
                }
            }
            0x1A => {
                self.regs[0x1A] = val & 0x0F;
                // Re-evaluate IRQ
                if self.regs[0x19] & self.regs[0x1A] & 0x0F != 0 {
                    self.regs[0x19] |= 0x80;
                    self.irq_line = true;
                } else {
                    self.regs[0x19] &= 0x7F;
                    self.irq_line = false;
                }
            }
            0x1E | 0x1F => {
                // Collision registers are read-only
            }
            0x2F..=0x3F => {
                // Unused registers - ignore writes
            }
            _ => {
                self.regs[r] = val;
            }
        }
    }

    /// Get the current frame buffer
    pub fn get_frame(&self) -> &Frame {
        &self.frame
    }

    /// Get current raster line
    pub fn raster_line(&self) -> u32 {
        self.raster_line
    }
}

impl Default for Vic {
    fn default() -> Self {
        Self::new()
    }
}
