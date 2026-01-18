//! Texas Instruments TMS9918A Video Display Processor
//!
//! The TMS9918A is a graphics chip used in multiple systems:
//! - Sega SG-1000
//! - ColecoVision
//! - Sega SC-3000
//! - MSX (original)
//! - TI-99/4A
//!
//! # Features
//! - 256×192 pixel resolution
//! - 16 color palette
//! - 4 graphics modes (Text, Graphics I, Graphics II, Multicolor)
//! - 32 hardware sprites (4 per scanline)
//! - Tilemap-based background rendering
//! - Frame interrupts

use crate::logging::{log, LogCategory, LogLevel};
use crate::renderer::Renderer;
use crate::types::Frame;

/// TMS9918A VDP state and rendering
pub struct Tms9918a {
    // Video RAM (16KB)
    vram: [u8; 0x4000],

    // VDP registers (8 registers)
    registers: [u8; 8],

    // Internal state
    address_register: u16,
    read_ahead_buffer: u8,
    write_latch: bool,
    read_mode: bool,

    // Rendering
    frame: Frame,

    // Interrupts
    frame_interrupt_pending: bool,

    // Sprite flags
    sprite_overflow: bool,
    sprite_collision: bool,
    fifth_sprite_number: u8,

    // Current scanline
    scanline: u16,

    // TMS9918A color palette (16 colors, ARGB8888 format)
    palette: [u32; 16],
}

impl Tms9918a {
    /// Create a new TMS9918A VDP
    pub fn new() -> Self {
        Self {
            vram: [0; 0x4000],
            registers: [0; 8],
            address_register: 0,
            read_ahead_buffer: 0,
            write_latch: false,
            read_mode: false,
            frame: Frame::new(256, 192),
            frame_interrupt_pending: false,
            sprite_overflow: false,
            sprite_collision: false,
            fifth_sprite_number: 31,
            scanline: 262, // Start at end of frame
            palette: [
                // TMS9918A standard palette (ARGB8888)
                0xFF000000, // 0: Transparent
                0xFF000000, // 1: Black
                0xFF21C842, // 2: Medium Green
                0xFF5EDC78, // 3: Light Green
                0xFF5455ED, // 4: Dark Blue
                0xFF7D76FC, // 5: Light Blue
                0xFFD4524D, // 6: Dark Red
                0xFF42EBF5, // 7: Cyan
                0xFFFC5554, // 8: Medium Red
                0xFFFF7978, // 9: Light Red
                0xFFD4C154, // A: Dark Yellow
                0xFFE6CE80, // B: Light Yellow
                0xFF21B03B, // C: Dark Green
                0xFFC95BBA, // D: Magenta
                0xFFCCCCCC, // E: Gray
                0xFFFFFFFF, // F: White
            ],
        }
    }

    /// Write to VDP control port
    pub fn write_control(&mut self, data: u8) {
        if !self.write_latch {
            // First byte - lower 8 bits of address
            self.address_register = (self.address_register & 0x3F00) | data as u16;
            self.write_latch = true;
        } else {
            // Second byte - determines mode
            self.write_latch = false;

            if (data & 0x80) != 0 {
                // Register write (bit 7 = 1)
                let reg = data & 0x07;
                let value = (self.address_register & 0xFF) as u8;
                self.registers[reg as usize] = value;
                log(LogCategory::PPU, LogLevel::Debug, || {
                    format!("TMS9918A: Register R{} = ${:02X}", reg, value)
                });
                self.read_mode = false;
            } else {
                // Address write (bit 7 = 0)
                self.address_register =
                    (self.address_register & 0x00FF) | ((data as u16 & 0x3F) << 8);

                // Bit 6 determines read/write mode
                self.read_mode = (data & 0x40) == 0;

                // If entering read mode, fill read-ahead buffer
                if self.read_mode {
                    self.read_ahead_buffer = self.vram[(self.address_register & 0x3FFF) as usize];
                    self.address_register = self.address_register.wrapping_add(1) & 0x3FFF;
                }
            }
        }
    }

    /// Write to VDP data port
    pub fn write_data(&mut self, data: u8) {
        self.write_latch = false;
        self.read_mode = false;

        // Write to VRAM
        self.vram[(self.address_register & 0x3FFF) as usize] = data;
        self.address_register = self.address_register.wrapping_add(1) & 0x3FFF;
    }

    /// Read from VDP data port
    pub fn read_data(&mut self) -> u8 {
        self.write_latch = false;

        // Return buffered value
        let value = self.read_ahead_buffer;

        // Read next byte into buffer
        self.read_ahead_buffer = self.vram[(self.address_register & 0x3FFF) as usize];
        self.address_register = self.address_register.wrapping_add(1) & 0x3FFF;

        value
    }

    /// Read from VDP status port
    pub fn read_status(&mut self) -> u8 {
        self.write_latch = false;
        let mut status = 0;

        // Bit 7: Frame interrupt pending
        if self.frame_interrupt_pending {
            status |= 0x80;
        }

        // Bit 6: 5th sprite flag (sprite overflow)
        if self.sprite_overflow {
            status |= 0x40;
        }

        // Bit 5: Sprite collision
        if self.sprite_collision {
            status |= 0x20;
        }

        // Bits 4-0: 5th sprite number
        // Contains the number of the first sprite that caused overflow (or 31 if no overflow)
        status |= self.fifth_sprite_number & 0x1F;

        // Clear interrupt flag on read (sprite flags persist until next frame)
        self.frame_interrupt_pending = false;

        status
    }

    /// Set current scanline (for cycle-accurate timing)
    pub fn set_scanline(&mut self, scanline: u16) {
        let old_scanline = self.scanline;

        // Only update if scanline has actually changed
        if scanline == old_scanline {
            return;
        }

        self.scanline = scanline;

        // Check for frame wrap (new frame started)
        if scanline < old_scanline {
            // Trigger frame interrupt at start of VBlank
            if (self.registers[1] & 0x20) != 0 {
                // Frame interrupt enable
                self.frame_interrupt_pending = true;
            }
        } else {
            // Trigger frame interrupt when crossing into VBlank (scanline 192)
            // Note: TMS9918A triggers VBlank interrupt at end of visible area (line 192)
            // Some sources mention line 242, but 192 is when active display ends
            if old_scanline < 192 && scanline >= 192 && (self.registers[1] & 0x20) != 0 {
                self.frame_interrupt_pending = true;
            }
        }
    }

    /// Render the full frame (called once per frame)
    pub fn render_frame(&mut self) {
        // Reset sprite overflow and collision flags at start of frame
        self.sprite_overflow = false;
        self.sprite_collision = false;
        self.fifth_sprite_number = 31;

        // Render all visible scanlines
        for line in 0..192 {
            self.render_scanline(line);
        }
    }

    /// Check if frame interrupt is pending
    pub fn frame_interrupt_pending(&self) -> bool {
        self.frame_interrupt_pending
    }

    /// Clear frame interrupt flag (used when interrupt is acknowledged)
    pub fn clear_frame_interrupt(&mut self) {
        self.frame_interrupt_pending = false;
    }

    /// Get tile viewer data for debugging (returns VRAM, palette, and register data)
    pub fn get_tile_viewer_data(&self) -> (Vec<u8>, Vec<u32>, Vec<u8>) {
        (self.vram.to_vec(), self.palette.to_vec(), self.registers.to_vec())
    }

    /// Get VDP state for save state
    pub fn get_state(&self) -> serde_json::Value {
        serde_json::json!({
            "vram": self.vram.to_vec(),
            "registers": self.registers.to_vec(),
            "address_register": self.address_register,
            "read_ahead_buffer": self.read_ahead_buffer,
            "write_latch": self.write_latch,
            "read_mode": self.read_mode,
            "frame_interrupt_pending": self.frame_interrupt_pending,
            "sprite_overflow": self.sprite_overflow,
            "sprite_collision": self.sprite_collision,
            "fifth_sprite_number": self.fifth_sprite_number,
            "scanline": self.scanline,
        })
    }

    /// Set VDP state from save state
    pub fn set_state(&mut self, state: &serde_json::Value) -> Result<(), serde_json::Error> {
        macro_rules! load_u8 {
            ($state:expr, $field:literal, $target:expr) => {
                if let Some(val) = $state.get($field).and_then(|v| v.as_u64()) {
                    $target = val as u8;
                }
            };
        }

        macro_rules! load_u16 {
            ($state:expr, $field:literal, $target:expr) => {
                if let Some(val) = $state.get($field).and_then(|v| v.as_u64()) {
                    $target = val as u16;
                }
            };
        }

        macro_rules! load_bool {
            ($state:expr, $field:literal, $target:expr) => {
                if let Some(val) = $state.get($field).and_then(|v| v.as_bool()) {
                    $target = val;
                }
            };
        }

        // Load VRAM
        if let Some(vram) = state.get("vram").and_then(|v| v.as_array()) {
            for (i, val) in vram.iter().enumerate() {
                if i >= self.vram.len() {
                    break;
                }
                if let Some(byte) = val.as_u64() {
                    self.vram[i] = byte as u8;
                }
            }
        }

        // Load registers
        if let Some(registers) = state.get("registers").and_then(|v| v.as_array()) {
            for (i, val) in registers.iter().enumerate() {
                if i >= self.registers.len() {
                    break;
                }
                if let Some(byte) = val.as_u64() {
                    self.registers[i] = byte as u8;
                }
            }
        }

        load_u16!(state, "address_register", self.address_register);
        load_u8!(state, "read_ahead_buffer", self.read_ahead_buffer);
        load_bool!(state, "write_latch", self.write_latch);
        load_bool!(state, "read_mode", self.read_mode);
        load_bool!(
            state,
            "frame_interrupt_pending",
            self.frame_interrupt_pending
        );
        load_bool!(state, "sprite_overflow", self.sprite_overflow);
        load_bool!(state, "sprite_collision", self.sprite_collision);
        load_u8!(state, "fifth_sprite_number", self.fifth_sprite_number);
        load_u16!(state, "scanline", self.scanline);

        Ok(())
    }

    /// Render a single scanline
    fn render_scanline(&mut self, line: u8) {
        // Get graphics mode from registers
        let mode = self.get_graphics_mode();

        // Clear scanline to backdrop color (register 7, lower 4 bits)
        let backdrop_color = self.palette[(self.registers[7] & 0x0F) as usize];
        let line_offset = (line as usize) * 256;
        for x in 0..256 {
            self.frame.pixels[line_offset + x] = backdrop_color;
        }

        // Render based on graphics mode
        match mode {
            0 => self.render_graphics_i(line, line_offset),
            1 => self.render_text_mode(line, line_offset),
            2 => self.render_graphics_ii(line, line_offset),
            3 => self.render_multicolor_mode(line, line_offset),
            _ => {} // Invalid mode
        }

        // Render sprites if enabled
        if (self.registers[1] & 0x02) != 0 {
            self.render_sprites(line, line_offset);
        }
    }

    /// Get current graphics mode from register bits
    fn get_graphics_mode(&self) -> u8 {
        let m1 = (self.registers[0] & 0x02) != 0;
        let m2 = (self.registers[1] & 0x08) != 0;
        let m3 = (self.registers[1] & 0x10) != 0;

        match (m3, m2, m1) {
            (false, false, false) => 0, // Graphics I
            (false, false, true) => 1,  // Text
            (false, true, false) => 2,  // Graphics II
            (false, true, true) => 0,   // Invalid (treat as Graphics I)
            (true, false, false) => 3,  // Multicolor
            _ => 0,                     // Invalid (treat as Graphics I)
        }
    }

    /// Render Graphics I mode
    fn render_graphics_i(&mut self, line: u8, line_offset: usize) {
        // Name table base address (register 2, bits 3-0)
        let name_table_base = ((self.registers[2] & 0x0F) as usize) << 10;

        // Pattern table base address (register 4, bits 2-0)
        let pattern_table_base = ((self.registers[4] & 0x07) as usize) << 11;

        // Color table base address (register 3)
        let color_table_base = (self.registers[3] as usize) << 6;

        // Calculate tile row
        let tile_row = (line / 8) as usize;

        // Render 32 tiles across
        for tile_col in 0..32 {
            let tile_index = tile_row * 32 + tile_col;
            let pattern_index = self.vram[name_table_base + tile_index] as usize;

            // Get color byte (one color byte per 8 patterns)
            let color_byte = self.vram[color_table_base + (pattern_index / 8)];
            let fg_color = self.palette[(color_byte >> 4) as usize];
            let bg_color = self.palette[(color_byte & 0x0F) as usize];

            // Get pattern data
            let pattern_row = (line % 8) as usize;
            let pattern_byte = self.vram[pattern_table_base + (pattern_index * 8) + pattern_row];

            // Render 8 pixels
            for bit in 0..8 {
                let pixel_on = (pattern_byte & (0x80 >> bit)) != 0;
                let color = if pixel_on { fg_color } else { bg_color };
                let x = tile_col * 8 + bit;
                self.frame.pixels[line_offset + x] = color;
            }
        }
    }

    /// Render Text mode (40 column)
    fn render_text_mode(&mut self, line: u8, line_offset: usize) {
        // Name table base address
        let name_table_base = ((self.registers[2] & 0x0F) as usize) << 10;

        // Pattern table base address
        let pattern_table_base = ((self.registers[4] & 0x07) as usize) << 11;

        // Foreground and background colors from register 7
        let fg_color = self.palette[(self.registers[7] >> 4) as usize];
        let bg_color = self.palette[(self.registers[7] & 0x0F) as usize];

        // Calculate tile row (text mode uses 6x8 characters in 40 columns)
        let tile_row = (line / 8) as usize;

        // Render 40 characters across
        for char_col in 0..40 {
            let char_index = tile_row * 40 + char_col;
            if char_index >= 960 {
                break; // 40x24 = 960 characters max
            }
            let pattern_index = self.vram[name_table_base + char_index] as usize;

            // Get pattern data
            let pattern_row = (line % 8) as usize;
            let pattern_byte = self.vram[pattern_table_base + (pattern_index * 8) + pattern_row];

            // Render 6 pixels (text mode uses 6-pixel wide characters)
            for bit in 0..6 {
                let pixel_on = (pattern_byte & (0x80 >> bit)) != 0;
                let color = if pixel_on { fg_color } else { bg_color };
                let x = char_col * 6 + bit;
                if x < 256 {
                    self.frame.pixels[line_offset + x] = color;
                }
            }
        }
    }

    /// Render Graphics II mode
    fn render_graphics_ii(&mut self, line: u8, line_offset: usize) {
        // Name table base address
        let name_table_base = ((self.registers[2] & 0x0F) as usize) << 10;

        // Pattern table base address
        let pattern_table_base = ((self.registers[4] & 0x07) as usize) << 11;

        // Color table base address
        let color_table_base = (self.registers[3] as usize) << 6;

        // Calculate tile row
        let tile_row = (line / 8) as usize;

        // Render 32 tiles across
        for tile_col in 0..32 {
            let tile_index = tile_row * 32 + tile_col;
            let pattern_index = self.vram[name_table_base + tile_index] as usize;

            // In Graphics II mode, pattern and color tables are divided into thirds
            let third = tile_row / 8;
            let pattern_base = pattern_table_base + (third * 0x800);
            let color_base = color_table_base + (third * 0x800);

            // Get pattern data
            let pattern_row = (line % 8) as usize;
            let pattern_byte = self.vram[pattern_base + (pattern_index * 8) + pattern_row];

            // Get color byte (one per pattern row in Graphics II)
            let color_byte = self.vram[color_base + (pattern_index * 8) + pattern_row];
            let fg_color = self.palette[(color_byte >> 4) as usize];
            let bg_color = self.palette[(color_byte & 0x0F) as usize];

            // Render 8 pixels
            for bit in 0..8 {
                let pixel_on = (pattern_byte & (0x80 >> bit)) != 0;
                let color = if pixel_on { fg_color } else { bg_color };
                let x = tile_col * 8 + bit;
                self.frame.pixels[line_offset + x] = color;
            }
        }
    }

    /// Render Multicolor mode
    fn render_multicolor_mode(&mut self, line: u8, line_offset: usize) {
        // Name table base address
        let name_table_base = ((self.registers[2] & 0x0F) as usize) << 10;

        // Pattern table base address
        let pattern_table_base = ((self.registers[4] & 0x07) as usize) << 11;

        // Calculate tile row (each tile is 8x8 but uses 4x4 pixel blocks)
        let tile_row = (line / 8) as usize;
        let block_row = ((line % 8) / 4) as usize; // 0 or 1

        // Render 32 tiles across
        for tile_col in 0..32 {
            let tile_index = tile_row * 32 + tile_col;
            let pattern_index = self.vram[name_table_base + tile_index] as usize;

            // Get pattern data (2 bytes per pattern, one per 4-pixel-high block)
            let pattern_byte =
                self.vram[pattern_table_base + (pattern_index * 8) + (block_row * 2)];

            // Each nibble represents a 4x4 block color
            let left_color = self.palette[(pattern_byte >> 4) as usize];
            let right_color = self.palette[(pattern_byte & 0x0F) as usize];

            // Render 4 pixels (left block)
            for bit in 0..4 {
                let x = tile_col * 8 + bit;
                self.frame.pixels[line_offset + x] = left_color;
            }

            // Render 4 pixels (right block)
            for bit in 0..4 {
                let x = tile_col * 8 + 4 + bit;
                self.frame.pixels[line_offset + x] = right_color;
            }
        }
    }

    /// Render sprites
    fn render_sprites(&mut self, line: u8, line_offset: usize) {
        // Sprite attribute table base address (register 5, bits 6-0)
        let sprite_attr_base = ((self.registers[5] & 0x7F) as usize) << 7;

        // Sprite pattern table base address (register 6, bits 2-0)
        let sprite_pattern_base = ((self.registers[6] & 0x07) as usize) << 11;

        // Sprite size (register 1, bit 1: 0=8x8, 1=16x16)
        let sprite_size = if (self.registers[1] & 0x02) != 0 {
            16
        } else {
            8
        };

        // Sprite magnification (register 1, bit 0)
        let mag = if (self.registers[1] & 0x01) != 0 {
            2
        } else {
            1
        };
        let actual_size = sprite_size * mag;

        let mut sprite_count = 0;

        // Track which pixels have sprites for collision detection
        // Per SMS Power!: collision only occurs between sprites, not sprite-background
        let mut sprite_line_buffer = [false; 256];

        // Scan through sprite attribute table (32 sprites max)
        for sprite_num in 0..32 {
            let attr_addr = sprite_attr_base + (sprite_num * 4);

            // Get sprite Y position
            // Per TMS9918A datasheet: Y coordinate is stored as Y+1
            // Y=0 means position -1 (off top of screen), Y=1 means position 0, etc.
            let sprite_y = (self.vram[attr_addr] as i16).wrapping_sub(1);

            // Check for end-of-sprite-list marker (0xD0 = 208, or position 207 after -1 offset)
            if self.vram[attr_addr] == 0xD0 {
                break;
            }

            // Check if sprite is on this scanline
            let line_i16 = line as i16;
            if line_i16 >= sprite_y && line_i16 < sprite_y + actual_size as i16 {
                sprite_count += 1;

                // Check for sprite overflow (more than 4 sprites on a line)
                if sprite_count > 4 {
                    self.sprite_overflow = true;
                    // Record the number of the 5th sprite (first one that couldn't be displayed)
                    if self.fifth_sprite_number == 31 {
                        self.fifth_sprite_number = sprite_num as u8;
                    }
                    break;
                }

                // Get sprite attributes
                let sprite_x = self.vram[attr_addr + 1] as i16;
                let pattern_num = self.vram[attr_addr + 2] as usize;
                let sprite_color = self.palette[(self.vram[attr_addr + 3] & 0x0F) as usize];

                // Early color flag (bit 7 of attribute 3)
                let early_clock = (self.vram[attr_addr + 3] & 0x80) != 0;
                let x_offset = if early_clock { -32 } else { 0 };

                // Calculate pattern row
                let pattern_row = ((line_i16 - sprite_y) / mag as i16) as usize;

                // Get pattern data
                let pattern_addr = if sprite_size == 16 {
                    // 16x16 sprites use 4 consecutive patterns
                    let quad = (pattern_num & 0xFC) * 8;
                    if pattern_row < 8 {
                        quad + pattern_row
                    } else {
                        quad + 16 + (pattern_row - 8)
                    }
                } else {
                    // 8x8 sprites
                    (pattern_num * 8) + pattern_row
                };

                let pattern_byte = self.vram[sprite_pattern_base + pattern_addr];

                // Render sprite pixels
                for bit in 0..8 {
                    let pixel_on = (pattern_byte & (0x80 >> bit)) != 0;
                    if pixel_on {
                        for mx in 0..mag {
                            let x = sprite_x + x_offset + (bit * mag) as i16 + mx as i16;
                            if (0..256).contains(&x) {
                                let pixel_idx = line_offset + x as usize;
                                let x_idx = x as usize;

                                // Check for sprite-to-sprite collision (not sprite-to-background)
                                // Per SMS Power!: collision flag is set when opaque sprite pixels overlap
                                if sprite_line_buffer[x_idx] {
                                    self.sprite_collision = true;
                                }

                                // Mark this pixel as having a sprite
                                sprite_line_buffer[x_idx] = true;

                                // Draw the sprite pixel
                                self.frame.pixels[pixel_idx] = sprite_color;
                            }
                        }
                    }
                }

                // Handle 16x16 sprites (need to render right half)
                if sprite_size == 16 {
                    let pattern_addr = if pattern_row < 8 {
                        sprite_pattern_base + ((pattern_num & 0xFC) * 8) + 8 + pattern_row
                    } else {
                        sprite_pattern_base + ((pattern_num & 0xFC) * 8) + 24 + (pattern_row - 8)
                    };

                    let pattern_byte = self.vram[pattern_addr];

                    for bit in 0..8 {
                        let pixel_on = (pattern_byte & (0x80 >> bit)) != 0;
                        if pixel_on {
                            for mx in 0..mag {
                                let x = sprite_x + x_offset + ((8 + bit) * mag) as i16 + mx as i16;
                                if (0..256).contains(&x) {
                                    let pixel_idx = line_offset + x as usize;
                                    let x_idx = x as usize;

                                    // Check for sprite-to-sprite collision
                                    if sprite_line_buffer[x_idx] {
                                        self.sprite_collision = true;
                                    }

                                    // Mark this pixel as having a sprite
                                    sprite_line_buffer[x_idx] = true;

                                    // Draw the sprite pixel
                                    self.frame.pixels[pixel_idx] = sprite_color;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

impl Default for Tms9918a {
    fn default() -> Self {
        Self::new()
    }
}

impl Renderer for Tms9918a {
    fn get_frame(&self) -> &Frame {
        &self.frame
    }

    fn clear(&mut self, color: u32) {
        self.frame.pixels.fill(color);
    }

    fn reset(&mut self) {
        *self = Self::new();
    }

    fn resize(&mut self, _width: u32, _height: u32) {
        // TMS9918A has fixed resolution
    }

    fn name(&self) -> &str {
        "TMS9918A VDP"
    }
}
