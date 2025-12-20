//! TIA (Television Interface Adapter) - Video and audio chip for Atari 2600
//!
//! The TIA handles all video and audio generation for the Atari 2600.
//! Unlike modern systems, it has no framebuffer and generates video scanline-by-scanline.

use serde::{Deserialize, Serialize};

/// TIA chip state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tia {
    // Video registers
    vsync: bool,
    vblank: bool,
    
    // Playfield
    pf0: u8,
    pf1: u8,
    pf2: u8,
    playfield_reflect: bool,
    playfield_score_mode: bool,
    playfield_priority: bool,
    
    // Colors (palette indices)
    colubk: u8, // Background color
    colupf: u8, // Playfield color
    colup0: u8, // Player 0 color
    colup1: u8, // Player 1 color
    
    // Players (sprites)
    grp0: u8, // Player 0 graphics
    grp1: u8, // Player 1 graphics
    player0_x: u8,
    player1_x: u8,
    player0_reflect: bool,
    player1_reflect: bool,
    
    // Missiles
    enam0: bool, // Missile 0 enable
    enam1: bool, // Missile 1 enable
    missile0_x: u8,
    missile1_x: u8,
    
    // Ball
    enabl: bool, // Ball enable
    ball_x: u8,
    
    // Horizontal motion
    hmp0: i8,
    hmp1: i8,
    hmm0: i8,
    hmm1: i8,
    hmbl: i8,
    
    // Current scanline and pixel position
    scanline: u16,
    pixel: u16,
    
    // Audio (simplified - just register storage)
    audc0: u8,
    audc1: u8,
    audf0: u8,
    audf1: u8,
    audv0: u8,
    audv1: u8,
}

impl Default for Tia {
    fn default() -> Self {
        Self::new()
    }
}

impl Tia {
    /// Create a new TIA chip
    pub fn new() -> Self {
        Self {
            vsync: false,
            vblank: false,
            pf0: 0,
            pf1: 0,
            pf2: 0,
            playfield_reflect: false,
            playfield_score_mode: false,
            playfield_priority: false,
            colubk: 0,
            colupf: 0,
            colup0: 0,
            colup1: 0,
            grp0: 0,
            grp1: 0,
            player0_x: 0,
            player1_x: 0,
            player0_reflect: false,
            player1_reflect: false,
            enam0: false,
            enam1: false,
            missile0_x: 0,
            missile1_x: 0,
            enabl: false,
            ball_x: 0,
            hmp0: 0,
            hmp1: 0,
            hmm0: 0,
            hmm1: 0,
            hmbl: 0,
            scanline: 0,
            pixel: 0,
            audc0: 0,
            audc1: 0,
            audf0: 0,
            audf1: 0,
            audv0: 0,
            audv1: 0,
        }
    }

    /// Reset TIA to power-on state
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Write to TIA register
    pub fn write(&mut self, addr: u8, val: u8) {
        match addr {
            0x00 => self.vsync = (val & 0x02) != 0,
            0x01 => self.vblank = (val & 0x02) != 0,
            0x02 => {} // WSYNC - handled by bus
            0x03 => {} // RSYNC
            
            // Player 0
            0x04 => {
                // NUSIZ0 - Player 0 number and size
                // Simplified: just store it
            }
            0x05 => {
                // NUSIZ1 - Player 1 number and size
            }
            0x06 => self.colup0 = val,
            0x07 => self.colup1 = val,
            0x08 => self.colupf = val,
            0x09 => self.colubk = val,
            
            // Playfield control
            0x0A => {
                self.playfield_reflect = (val & 0x01) != 0;
                self.playfield_score_mode = (val & 0x02) != 0;
                self.playfield_priority = (val & 0x04) != 0;
            }
            
            // Player reflect
            0x0B => {
                self.player0_reflect = (val & 0x08) != 0;
            }
            0x0C => {
                self.player1_reflect = (val & 0x08) != 0;
            }
            
            // Playfield
            0x0D => self.pf0 = val,
            0x0E => self.pf1 = val,
            0x0F => self.pf2 = val,
            
            // Player position resets
            0x10 => self.player0_x = (self.pixel as u8).wrapping_sub(5),
            0x11 => self.player1_x = (self.pixel as u8).wrapping_sub(5),
            0x12 => self.missile0_x = (self.pixel as u8).wrapping_sub(4),
            0x13 => self.missile1_x = (self.pixel as u8).wrapping_sub(4),
            0x14 => self.ball_x = (self.pixel as u8).wrapping_sub(4),
            
            // Audio
            0x15 => self.audc0 = val & 0x0F,
            0x16 => self.audc1 = val & 0x0F,
            0x17 => self.audf0 = val & 0x1F,
            0x18 => self.audf1 = val & 0x1F,
            0x19 => self.audv0 = val & 0x0F,
            0x1A => self.audv1 = val & 0x0F,
            
            // Player graphics
            0x1B => self.grp0 = val,
            0x1C => self.grp1 = val,
            
            // Enable missiles and ball
            0x1D => self.enam0 = (val & 0x02) != 0,
            0x1E => self.enam1 = (val & 0x02) != 0,
            0x1F => self.enabl = (val & 0x02) != 0,
            
            // Horizontal motion
            0x20 => self.hmp0 = (val as i8) >> 4,
            0x21 => self.hmp1 = (val as i8) >> 4,
            0x22 => self.hmm0 = (val as i8) >> 4,
            0x23 => self.hmm1 = (val as i8) >> 4,
            0x24 => self.hmbl = (val as i8) >> 4,
            
            // Clear horizontal motion
            0x2B => {
                self.hmp0 = 0;
                self.hmp1 = 0;
                self.hmm0 = 0;
                self.hmm1 = 0;
                self.hmbl = 0;
            }
            
            _ => {}
        }
    }

    /// Read from TIA register (collision detection)
    pub fn read(&self, addr: u8) -> u8 {
        // TIA read registers are for collision detection
        // Simplified implementation - return 0 for now
        match addr & 0x0F {
            0x00..=0x07 => 0, // Collision registers
            0x08..=0x0D => 0, // Input ports (handled by RIOT)
            _ => 0,
        }
    }

    /// Clock the TIA for one CPU cycle (3 color clocks)
    pub fn clock(&mut self) {
        // Simplified: just advance pixel counter
        self.pixel += 3; // 3 color clocks per CPU cycle
        
        if self.pixel >= 228 {
            self.pixel = 0;
            self.scanline += 1;
            
            if self.scanline >= 262 {
                self.scanline = 0;
            }
        }
    }

    /// Check if in VBLANK
    pub fn in_vblank(&self) -> bool {
        self.vblank || self.vsync
    }

    /// Get current scanline
    pub fn get_scanline(&self) -> u16 {
        self.scanline
    }

    /// Render a single scanline to the given buffer
    /// Returns the scanline number rendered
    pub fn render_scanline(&self, buffer: &mut [u32], line: usize) {
        if line >= 192 {
            return; // Only visible lines
        }

        // Atari 2600 has 160 pixels per scanline
        for x in 0..160 {
            let color = self.get_pixel_color(x, line);
            buffer[line * 160 + x] = color;
        }
    }

    /// Get the color of a pixel at the given position
    fn get_pixel_color(&self, x: usize, _line: usize) -> u32 {
        // Simplified rendering - just show playfield and background
        
        // Check if this pixel is part of the playfield
        if self.is_playfield_pixel(x) {
            return ntsc_to_rgb(self.colupf);
        }
        
        // Background color
        ntsc_to_rgb(self.colubk)
    }

    /// Check if a pixel is part of the playfield
    fn is_playfield_pixel(&self, x: usize) -> bool {
        // Playfield is 40 bits wide, mirrored or repeated
        let pf_bit = if x < 80 {
            // Left half
            self.get_playfield_bit(x / 2)
        } else {
            // Right half
            let bit_pos = (x - 80) / 2;
            if self.playfield_reflect {
                // Mirrored
                self.get_playfield_bit(39 - bit_pos)
            } else {
                // Repeated
                self.get_playfield_bit(bit_pos)
            }
        };
        
        pf_bit
    }

    /// Get a single bit from the playfield
    fn get_playfield_bit(&self, bit: usize) -> bool {
        if bit < 4 {
            // PF0 (bits 4-7 map to playfield bits 0-3)
            (self.pf0 & (0x10 << bit)) != 0
        } else if bit < 12 {
            // PF1 (bits 7-0 map to playfield bits 4-11)
            (self.pf1 & (0x80 >> (bit - 4))) != 0
        } else if bit < 20 {
            // PF2 (bits 0-7 map to playfield bits 12-19)
            (self.pf2 & (0x01 << (bit - 12))) != 0
        } else {
            false
        }
    }
}

/// Convert NTSC palette value to RGB
/// This is a simplified conversion - real Atari 2600 uses NTSC color encoding
fn ntsc_to_rgb(ntsc: u8) -> u32 {
    // Simplified palette - just use the NTSC value directly for now
    // In a full implementation, this would use a proper NTSC color table
    let luminance = (ntsc & 0x0F) as u32;
    let hue = ((ntsc >> 4) & 0x0F) as u32;
    
    // Very basic color mapping
    let r = ((luminance * 16) + (hue * 8)) & 0xFF;
    let g = ((luminance * 16) + (hue * 4)) & 0xFF;
    let b = ((luminance * 16) + (hue * 2)) & 0xFF;
    
    0xFF000000 | (r << 16) | (g << 8) | b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tia_creation() {
        let tia = Tia::new();
        assert_eq!(tia.scanline, 0);
        assert_eq!(tia.pixel, 0);
    }

    #[test]
    fn test_tia_vsync() {
        let mut tia = Tia::new();
        tia.write(0x00, 0x02);
        assert!(tia.vsync);
        tia.write(0x00, 0x00);
        assert!(!tia.vsync);
    }

    #[test]
    fn test_tia_vblank() {
        let mut tia = Tia::new();
        tia.write(0x01, 0x02);
        assert!(tia.vblank);
        assert!(tia.in_vblank());
    }

    #[test]
    fn test_tia_colors() {
        let mut tia = Tia::new();
        tia.write(0x06, 0x42); // COLUP0
        tia.write(0x07, 0x84); // COLUP1
        tia.write(0x08, 0x26); // COLUPF
        tia.write(0x09, 0x00); // COLUBK
        
        assert_eq!(tia.colup0, 0x42);
        assert_eq!(tia.colup1, 0x84);
        assert_eq!(tia.colupf, 0x26);
        assert_eq!(tia.colubk, 0x00);
    }

    #[test]
    fn test_tia_playfield() {
        let mut tia = Tia::new();
        tia.write(0x0D, 0xF0); // PF0
        tia.write(0x0E, 0xAA); // PF1
        tia.write(0x0F, 0x55); // PF2
        
        assert_eq!(tia.pf0, 0xF0);
        assert_eq!(tia.pf1, 0xAA);
        assert_eq!(tia.pf2, 0x55);
    }

    #[test]
    fn test_tia_playfield_control() {
        let mut tia = Tia::new();
        tia.write(0x0A, 0x01); // Reflect
        assert!(tia.playfield_reflect);
        
        tia.write(0x0A, 0x02); // Score mode
        assert!(tia.playfield_score_mode);
        
        tia.write(0x0A, 0x04); // Priority
        assert!(tia.playfield_priority);
    }

    #[test]
    fn test_tia_clock() {
        let mut tia = Tia::new();
        tia.clock();
        assert_eq!(tia.pixel, 3);
        
        // Clock through a scanline
        for _ in 0..75 {
            tia.clock();
        }
        assert_eq!(tia.scanline, 1);
        assert_eq!(tia.pixel, 0);
    }

    #[test]
    fn test_tia_audio() {
        let mut tia = Tia::new();
        tia.write(0x15, 0x0F); // AUDC0
        tia.write(0x17, 0x1F); // AUDF0
        tia.write(0x19, 0x0F); // AUDV0
        
        assert_eq!(tia.audc0, 0x0F);
        assert_eq!(tia.audf0, 0x1F);
        assert_eq!(tia.audv0, 0x0F);
    }

    #[test]
    fn test_tia_player_graphics() {
        let mut tia = Tia::new();
        tia.write(0x1B, 0xFF); // GRP0
        tia.write(0x1C, 0xAA); // GRP1
        
        assert_eq!(tia.grp0, 0xFF);
        assert_eq!(tia.grp1, 0xAA);
    }

    #[test]
    fn test_tia_reset() {
        let mut tia = Tia::new();
        tia.write(0x06, 0x42);
        tia.write(0x0D, 0xF0);
        tia.scanline = 100;
        
        tia.reset();
        
        assert_eq!(tia.colup0, 0);
        assert_eq!(tia.pf0, 0);
        assert_eq!(tia.scanline, 0);
    }
}
