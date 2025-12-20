//! Game Boy PPU (LCD Controller) implementation

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
const LCDC_WIN_TILEMAP: u8 = 0x40;
const LCDC_WIN_ENABLE: u8 = 0x20;
const LCDC_BG_WIN_TILES: u8 = 0x10;
const LCDC_BG_TILEMAP: u8 = 0x08;
const LCDC_OBJ_SIZE: u8 = 0x04;
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

    pub fn read_vram(&self, addr: u16) -> u8 {
        self.vram[(addr & 0x1FFF) as usize]
    }

    pub fn write_vram(&mut self, addr: u16, val: u8) {
        self.vram[(addr & 0x1FFF) as usize] = val;
    }

    pub fn read_oam(&self, addr: u16) -> u8 {
        self.oam[(addr & 0xFF) as usize]
    }

    pub fn write_oam(&mut self, addr: u16, val: u8) {
        self.oam[(addr & 0xFF) as usize] = val;
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

        // TODO: Render window
        // TODO: Render sprites

        frame
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
                    tile_data_base + (tile_index as u16 * 16)
                } else {
                    // Signed addressing mode
                    tile_data_base + ((tile_index as i8 as i16 + 128) as u16 * 16)
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
}
