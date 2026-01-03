//! Sega Master System Video Display Processor (VDP)
//!
//! The VDP is based on the Texas Instruments TMS9918A and handles all video output.
//!
//! # Features
//! - 256×192 pixel resolution
//! - 64 color palette (32 simultaneous)
//! - Tilemap-based background rendering
//! - 64 sprites with 8 per scanline limit
//! - Scrolling support
//! - Line and frame interrupts

use emu_core::renderer::Renderer;
use emu_core::types::Frame;

/// VDP state and rendering
pub struct Vdp {
    // Video RAM (16KB)
    vram: [u8; 0x4000],

    // Color RAM (32 bytes for palette)
    cram: [u8; 0x20],

    // VDP registers (11 registers)
    registers: [u8; 11],

    // Internal state
    address_register: u16,
    code_register: u8,
    read_buffer: u8,
    write_latch: bool,

    // Rendering
    frame: Frame,

    // Interrupts
    frame_interrupt_pending: bool,
    line_interrupt_pending: bool,
    line_counter: u8,

    // Current scanline
    scanline: u16,
}

impl Vdp {
    /// Create a new VDP
    pub fn new() -> Self {
        Self {
            vram: [0; 0x4000],
            cram: [0; 0x20],
            registers: [0; 11],
            address_register: 0,
            code_register: 0,
            read_buffer: 0,
            write_latch: false,
            frame: Frame::new(256, 192),
            frame_interrupt_pending: false,
            line_interrupt_pending: false,
            line_counter: 0,
            scanline: 0,
        }
    }

    /// Write to VDP control port (0xBF)
    pub fn write_control(&mut self, data: u8) {
        if !self.write_latch {
            // First byte - lower 8 bits of address
            self.address_register = (self.address_register & 0x3F00) | data as u16;
            self.write_latch = true;
        } else {
            // Second byte - upper 6 bits of address + code
            self.address_register = (self.address_register & 0x00FF) | ((data as u16 & 0x3F) << 8);
            self.code_register = (data >> 6) & 0x03;
            self.write_latch = false;

            // Check if this is a register write (code = 0x02)
            if self.code_register == 0x02 {
                let reg = data & 0x0F;
                if (reg as usize) < self.registers.len() {
                    self.registers[reg as usize] = (self.address_register & 0xFF) as u8;
                }
            }
        }
    }

    /// Write to VDP data port (0xBE)
    pub fn write_data(&mut self, data: u8) {
        self.write_latch = false;
        self.read_buffer = data;

        match self.code_register {
            0x03 => {
                // CRAM write
                self.cram[(self.address_register & 0x1F) as usize] = data;
            }
            _ => {
                // VRAM write
                self.vram[(self.address_register & 0x3FFF) as usize] = data;
            }
        }

        self.address_register = self.address_register.wrapping_add(1);
    }

    /// Read from VDP data port (0xBE)
    pub fn read_data(&mut self) -> u8 {
        self.write_latch = false;
        let value = self.read_buffer;
        self.read_buffer = self.vram[(self.address_register & 0x3FFF) as usize];
        self.address_register = self.address_register.wrapping_add(1);
        value
    }

    /// Read from VDP status port (0xBF)
    pub fn read_status(&mut self) -> u8 {
        self.write_latch = false;
        let mut status = 0;

        // Bit 7: Frame interrupt pending
        if self.frame_interrupt_pending {
            status |= 0x80;
        }

        // Bit 6: Sprite overflow
        // TODO: Implement sprite overflow detection

        // Bit 5: Sprite collision
        // TODO: Implement sprite collision detection

        // Clear frame interrupt flag on read
        self.frame_interrupt_pending = false;

        status
    }

    /// Read vertical counter
    pub fn read_vcounter(&self) -> u8 {
        // Return current scanline (simplified)
        (self.scanline & 0xFF) as u8
    }

    /// Step VDP by one scanline
    pub fn step_scanline(&mut self) {
        if self.scanline < 192 {
            // Render visible scanline
            self.render_scanline(self.scanline as u8);
        } else if self.scanline == 192 {
            // Frame interrupt occurs at start of VBlank
            if (self.registers[1] & 0x20) != 0 {
                // Frame interrupt enable
                self.frame_interrupt_pending = true;
            }
        }

        self.scanline += 1;
        if self.scanline >= 262 {
            // NTSC: 262 scanlines per frame
            self.scanline = 0;
        }
    }

    /// Check if frame interrupt is pending
    pub fn frame_interrupt_pending(&self) -> bool {
        self.frame_interrupt_pending
    }

    /// Render a single scanline
    fn render_scanline(&mut self, line: u8) {
        // Clear scanline to backdrop color
        let backdrop_color = self.decode_color(self.cram[16] & 0x3F);
        let line_offset = (line as usize) * 256;
        for x in 0..256 {
            self.frame.pixels[line_offset + x] = backdrop_color;
        }

        // Render background if enabled
        if (self.registers[1] & 0x40) != 0 {
            self.render_background(line, line_offset);
        }

        // Render sprites if enabled
        if (self.registers[1] & 0x08) != 0 {
            self.render_sprites(line, line_offset);
        }
    }

    /// Render background layer for a scanline
    fn render_background(&mut self, line: u8, line_offset: usize) {
        let name_table_addr = ((self.registers[2] as u16) & 0x0E) << 10;

        // Get scroll values
        let scroll_x = self.registers[8];
        let scroll_y = if line < 16 && (self.registers[0] & 0x40) != 0 {
            0 // Vertical scroll lock for top 2 rows
        } else {
            self.registers[9]
        };

        let y = line.wrapping_add(scroll_y);
        let tile_row = (y >> 3) as u16;
        let pixel_y = (y & 7) as u16;

        for x in 0..256u16 {
            let adj_x = (x as u8).wrapping_sub(scroll_x);
            let tile_col = (adj_x >> 3) as u16;
            let pixel_x = (adj_x & 7) as u16;

            // Read name table entry (2 bytes per tile)
            let name_addr = name_table_addr + (tile_row * 32 + tile_col) * 2;
            if name_addr >= 0x3FFE {
                continue;
            }

            let tile_data_low = self.vram[name_addr as usize];
            let tile_data_high = self.vram[(name_addr + 1) as usize];
            let tile_data = tile_data_low as u16 | ((tile_data_high as u16) << 8);

            let tile_index = tile_data & 0x1FF;
            let palette = ((tile_data >> 11) & 1) as usize;
            let h_flip = (tile_data >> 9) & 1;
            let v_flip = (tile_data >> 10) & 1;

            // Calculate pixel position within tile
            let px = if h_flip != 0 { 7 - pixel_x } else { pixel_x };
            let py = if v_flip != 0 { 7 - pixel_y } else { pixel_y };

            // Read tile pattern (32 bytes per 8x8 tile, 4 bits per pixel)
            let tile_addr = tile_index * 32 + py * 4;
            if tile_addr >= 0x3FFC {
                continue;
            }

            let byte0 = self.vram[tile_addr as usize];
            let byte1 = self.vram[(tile_addr + 1) as usize];
            let byte2 = self.vram[(tile_addr + 2) as usize];
            let byte3 = self.vram[(tile_addr + 3) as usize];

            // Extract 4-bit pixel value
            let shift = 7 - px;
            let pixel = ((byte0 >> shift) & 1)
                | (((byte1 >> shift) & 1) << 1)
                | (((byte2 >> shift) & 1) << 2)
                | (((byte3 >> shift) & 1) << 3);

            // Pixel 0 is transparent
            if pixel != 0 {
                let color_index = palette * 16 + pixel as usize;
                let color = self.decode_color(self.cram[color_index] & 0x3F);
                self.frame.pixels[line_offset + x as usize] = color;
            }
        }
    }

    /// Render sprites for a scanline
    fn render_sprites(&mut self, line: u8, line_offset: usize) {
        let sprite_attr_table = ((self.registers[5] as u16) & 0x7E) << 7;
        let sprite_size = if (self.registers[1] & 0x02) != 0 {
            16
        } else {
            8
        };

        let mut sprites_on_line = 0;

        // Sprites are rendered in reverse order (higher priority first)
        for i in (0..64).rev() {
            let y = self.vram[(sprite_attr_table + i) as usize];

            // Check for end marker
            if y == 0xD0 {
                break;
            }

            let y_pos = y.wrapping_add(1);
            if line < y_pos || line >= y_pos + sprite_size {
                continue;
            }

            sprites_on_line += 1;
            if sprites_on_line > 8 {
                // Sprite overflow - only 8 sprites per line
                break;
            }

            // Get sprite X position and tile number
            let x_pos = self.vram[(sprite_attr_table + 128 + i * 2) as usize];
            let tile_num = self.vram[(sprite_attr_table + 128 + i * 2 + 1) as usize];

            // Calculate sprite row
            let sprite_y = line - y_pos;

            // Read tile pattern (once per sprite row, not per pixel)
            let tile_addr = (tile_num as u16) * 32 + (sprite_y as u16) * 4;
            if tile_addr >= 0x3FFC {
                continue;
            }

            let byte0 = self.vram[tile_addr as usize];
            let byte1 = self.vram[(tile_addr + 1) as usize];
            let byte2 = self.vram[(tile_addr + 2) as usize];
            let byte3 = self.vram[(tile_addr + 3) as usize];

            // Render sprite pixels
            for px in 0..8u8 {
                let x = x_pos.wrapping_add(px);
                if x as u16 >= 256 {
                    continue;
                }

                let shift = 7 - px;
                let pixel = ((byte0 >> shift) & 1)
                    | (((byte1 >> shift) & 1) << 1)
                    | (((byte2 >> shift) & 1) << 2)
                    | (((byte3 >> shift) & 1) << 3);

                // Sprite pixel 0 is transparent
                if pixel != 0 {
                    let color_index = 16 + pixel as usize; // Sprites use second palette
                    let color = self.decode_color(self.cram[color_index] & 0x3F);
                    self.frame.pixels[line_offset + x as usize] = color;
                }
            }
        }
    }

    /// Decode 6-bit SMS color to 32-bit ARGB
    fn decode_color(&self, color: u8) -> u32 {
        // SMS uses 6-bit color: --BBGGRR
        let r = (color & 0x03) as u32;
        let g = ((color >> 2) & 0x03) as u32;
        let b = ((color >> 4) & 0x03) as u32;

        // Scale 2-bit to 8-bit (0-3 -> 0-255)
        let r8 = (r * 85) & 0xFF;
        let g8 = (g * 85) & 0xFF;
        let b8 = (b * 85) & 0xFF;

        // Return ARGB8888
        0xFF000000 | (r8 << 16) | (g8 << 8) | b8
    }
}

impl Default for Vdp {
    fn default() -> Self {
        Self::new()
    }
}

impl Renderer for Vdp {
    fn get_frame(&self) -> &Frame {
        &self.frame
    }

    fn clear(&mut self, color: u32) {
        self.frame.pixels.fill(color);
    }

    fn reset(&mut self) {
        self.vram.fill(0);
        self.cram.fill(0);
        self.registers.fill(0);
        self.address_register = 0;
        self.code_register = 0;
        self.read_buffer = 0;
        self.write_latch = false;
        self.frame_interrupt_pending = false;
        self.line_interrupt_pending = false;
        self.line_counter = 0;
        self.scanline = 0;
        self.clear(0xFF000000);
    }

    fn resize(&mut self, width: u32, height: u32) {
        // SMS has fixed 256×192 resolution, but allow resizing frame buffer
        self.frame = Frame::new(width, height);
    }

    fn name(&self) -> &str {
        "SMS VDP"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vdp_creation() {
        let vdp = Vdp::new();
        assert_eq!(vdp.frame.pixels.len(), 256 * 192);
    }

    #[test]
    fn test_vdp_register_write() {
        let mut vdp = Vdp::new();

        // Write to register 0
        vdp.write_control(0x00); // Low byte
        vdp.write_control(0x80); // High byte (register write, reg 0)

        assert_eq!(vdp.registers[0], 0x00);

        // Write to register 1 with value 0xA0
        vdp.write_control(0xA0); // Low byte (value)
        vdp.write_control(0x81); // High byte (register write, reg 1)

        assert_eq!(vdp.registers[1], 0xA0);
    }

    #[test]
    fn test_vdp_vram_write() {
        let mut vdp = Vdp::new();

        // Set VRAM address to 0x1234
        vdp.write_control(0x34); // Low byte
        vdp.write_control(0x52); // High byte (VRAM write, 0x12)

        // Write data
        vdp.write_data(0x42);

        assert_eq!(vdp.vram[0x1234], 0x42);
    }

    #[test]
    fn test_vdp_color_decode() {
        let vdp = Vdp::new();

        // Test black (all zeros)
        assert_eq!(vdp.decode_color(0x00), 0xFF000000);

        // Test white (all ones in 6 bits)
        assert_eq!(vdp.decode_color(0x3F), 0xFFFFFFFF);

        // Test red (0x03)
        assert_eq!(vdp.decode_color(0x03), 0xFFFF0000);
    }
}
