use crate::cartridge::Mirroring;
use emu_core::types::Frame;
use std::cell::{Cell, RefCell};
use std::fmt;

// 2C02 NES master palette (RGB), packed as 0xFFRRGGBB.
// This is a commonly used approximation; exact values vary by decoder.
const NES_MASTER_PALETTE: [u32; 64] = [
    0xFF545454, 0xFF001E74, 0xFF081090, 0xFF300088, 0xFF440064, 0xFF5C0030, 0xFF540400, 0xFF3C1800,
    0xFF202A00, 0xFF083A00, 0xFF004000, 0xFF003C00, 0xFF00323C, 0xFF000000, 0xFF000000, 0xFF000000,
    0xFF989698, 0xFF084CC4, 0xFF3032EC, 0xFF5C1EE4, 0xFF8814B0, 0xFFA01464, 0xFF982220, 0xFF783C00,
    0xFF545A00, 0xFF287200, 0xFF087C00, 0xFF007628, 0xFF006678, 0xFF000000, 0xFF000000, 0xFF000000,
    0xFFECEEEC, 0xFF4C9AEC, 0xFF787CEC, 0xFFB062EC, 0xFFE454EC, 0xFFEC58B4, 0xFFEC6A64, 0xFFD48820,
    0xFFA0AA00, 0xFF74C400, 0xFF4CD020, 0xFF38CC6C, 0xFF38B4CC, 0xFF3C3C3C, 0xFF000000, 0xFF000000,
    0xFFECEEEC, 0xFFA8CCEC, 0xFFBCBCEC, 0xFFD4B2EC, 0xFFECAEEC, 0xFFECAED4, 0xFFECC4B0, 0xFFE4D4A0,
    0xFFCCDCA0, 0xFFB4E4A0, 0xFFA8E4B4, 0xFFA0E4CC, 0xFFA0D4E4, 0xFFA0A2A0, 0xFF000000, 0xFF000000,
];

fn nes_palette_rgb(index: u8) -> u32 {
    NES_MASTER_PALETTE[(index & 0x3F) as usize]
}

fn palette_mirror_index(i: usize) -> usize {
    // Palette mirroring:
    // - $3F04/$3F08/$3F0C (bg palette color 0s) mirror $3F00 (universal bg)
    // - $3F10/$3F14/$3F18/$3F1C (sprite palette color 0s) also mirror $3F00
    match i & 0x1F {
        0x04 | 0x08 | 0x0C | 0x10 | 0x14 | 0x18 | 0x1C => 0x00,
        v => v,
    }
}

pub struct Ppu {
    pub chr: Vec<u8>,
    chr_is_ram: bool,
    pub vram: [u8; 0x800], // 2KB internal VRAM (nametables)
    pub palette: [u8; 32],
    pub oam: [u8; 256],
    mirroring: Mirroring,
    ctrl: u8,
    mask: u8,
    // Minimal PPUSTATUS bit7 (VBlank) flag.
    vblank: Cell<bool>,
    // PPUADDR latch
    addr_latch: Cell<bool>,
    pub vram_addr: Cell<u16>,
    read_buffer: Cell<u8>,
    #[allow(clippy::type_complexity)]
    a12_callback: RefCell<Option<Box<dyn FnMut(bool)>>>,
    scroll_x: u8,
    scroll_y: u8,
}

impl fmt::Debug for Ppu {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Ppu").finish_non_exhaustive()
    }
}

impl Ppu {
    pub fn new(chr: Vec<u8>, mirroring: Mirroring) -> Self {
        let (chr, chr_is_ram) = if chr.is_empty() {
            (vec![0u8; 0x2000], true)
        } else {
            (chr, false)
        };
        Self {
            chr,
            chr_is_ram,
            vram: [0; 0x800],
            palette: [0; 32],
            oam: [0; 256],
            mirroring,
            ctrl: 0,
            mask: 0,
            vblank: Cell::new(false),
            addr_latch: Cell::new(false),
            vram_addr: Cell::new(0),
            read_buffer: Cell::new(0),
            a12_callback: RefCell::new(None),
            scroll_x: 0,
            scroll_y: 0,
        }
    }

    fn map_nametable_addr(&self, addr: u16) -> usize {
        // Map $2000-$2FFF into internal 2KB VRAM using cartridge mirroring.
        let a = addr & 0x0FFF; // 0x0000..0x0FFF
        let table = (a / 0x0400) as u16; // 0..3
        let offset = (a % 0x0400) as u16;

        // For now, treat FourScreen as Vertical (we only have 2KB).
        let physical_table = match self.mirroring {
            Mirroring::Vertical | Mirroring::FourScreen => match table {
                0 | 2 => 0,
                1 | 3 => 1,
                _ => 0,
            },
            Mirroring::Horizontal => match table {
                0 | 1 => 0,
                2 | 3 => 1,
                _ => 0,
            },
            Mirroring::SingleScreenLower => 0,
            Mirroring::SingleScreenUpper => 1,
        };

        (physical_table * 0x0400 + offset) as usize & 0x07FF
    }

    pub fn set_mirroring(&mut self, mirroring: Mirroring) {
        self.mirroring = mirroring;
    }

    pub fn nmi_enabled(&self) -> bool {
        (self.ctrl & 0x80) != 0
    }

    pub fn ctrl(&self) -> u8 {
        self.ctrl
    }

    pub fn mask(&self) -> u8 {
        self.mask
    }

    pub fn scroll(&self) -> (u8, u8) {
        (self.scroll_x, self.scroll_y)
    }

    /// Set/clear the VBlank flag (PPUSTATUS bit 7).
    pub fn set_vblank(&self, v: bool) {
        self.vblank.set(v);
    }

    pub fn set_a12_callback(&self, cb: Option<Box<dyn FnMut(bool)>>) {
        *self.a12_callback.borrow_mut() = cb;
    }

    fn chr_fetch(&self, addr: usize) -> u8 {
        // Notify mapper about PPU A12 line (bit 12 of CHR address) transitions.
        if let Some(cb) = &mut *self.a12_callback.borrow_mut() {
            let a12_high = (addr & 0x1000) != 0;
            cb(a12_high);
        }
        self.chr.get(addr).copied().unwrap_or(0)
    }

    /// Read a PPU register (very partial implementation).
    pub fn read_register(&self, reg: u16) -> u8 {
        match reg & 0x7 {
            2 => {
                // PPUSTATUS: bit 7 = vblank
                let mut status = 0u8;
                if self.vblank.get() {
                    status |= 0x80;
                }
                // Reading PPUSTATUS clears vblank and resets address latch.
                self.vblank.set(false);
                self.addr_latch.set(false);
                status
            }
            7 => {
                // PPUDATA read with buffered behavior.
                let addr = self.vram_addr.get() & 0x3FFF;

                // Palette reads return immediately, no buffering.
                if (0x3F00..=0x3F1F).contains(&addr) {
                    let p = (addr - 0x3F00) & 0x1F;
                    let target = palette_mirror_index(p as usize);
                    let val = self.palette[target];
                    let inc = if (self.ctrl & 0x04) != 0 { 32 } else { 1 };
                    self.vram_addr.set(self.vram_addr.get().wrapping_add(inc));
                    return val;
                }

                // Return buffered value, then reload buffer from current addr.
                let buffered = self.read_buffer.get();
                let fetched = self.read_vram(addr);
                self.read_buffer.set(fetched);

                // Increment VRAM address.
                let inc = if (self.ctrl & 0x04) != 0 { 32 } else { 1 };
                self.vram_addr.set(self.vram_addr.get().wrapping_add(inc));

                buffered
            }
            _ => 0,
        }
    }

    pub fn write_register(&mut self, reg: u16, val: u8) {
        match reg & 0x7 {
            0 => {
                // PPUCTRL
                self.ctrl = val;
            }
            1 => {
                // PPUMASK
                self.mask = val;
            }
            5 => {
                // PPUSCROLL (write x then y), shares latch with PPUADDR.
                if !self.addr_latch.get() {
                    self.scroll_x = val;
                    self.addr_latch.set(true);
                } else {
                    self.scroll_y = val;
                    self.addr_latch.set(false);
                }
            }
            6 => {
                // PPUADDR (write high then low)
                if !self.addr_latch.get() {
                    let lo = self.vram_addr.get() & 0x00FF;
                    self.vram_addr.set(((val as u16) << 8) | lo);
                    self.addr_latch.set(true);
                } else {
                    let hi = self.vram_addr.get() & 0xFF00;
                    self.vram_addr.set(hi | val as u16);
                    self.addr_latch.set(false);
                }
            }
            7 => {
                // PPUDATA: write to vram or chr depending on address
                let addr = self.vram_addr.get() & 0x3FFF;
                if addr < 0x2000 {
                    // CHR-ROM is typically read-only; only allow writes for CHR-RAM.
                    if self.chr_is_ram && self.chr.len() >= (addr as usize + 1) {
                        self.chr[addr as usize] = val;
                    }
                } else if addr >= 0x2000 && addr < 0x3F00 {
                    // Nametable VRAM space with mirroring
                    let idx = self.map_nametable_addr(addr);
                    self.vram[idx] = val;
                } else if addr >= 0x3F00 && addr < 0x3F20 {
                    let p = (addr - 0x3F00) & 0x1F;
                    let target = palette_mirror_index(p as usize);
                    self.palette[target] = val;
                }
                // Increment VRAM address based on PPUCTRL bit 2.
                // 0 = increment by 1, 1 = increment by 32.
                let inc = if (self.ctrl & 0x04) != 0 { 32 } else { 1 };
                self.vram_addr.set(self.vram_addr.get().wrapping_add(inc));
            }
            _ => {
                // Other regs ignored for now
            }
        }
    }

    #[allow(dead_code)]
    pub fn dma_oam(&mut self, page: u8, read_mem: &dyn Fn(u16) -> u8) {
        let base = (page as u16) << 8;
        for i in 0..256u16 {
            self.oam[i as usize] = read_mem(base.wrapping_add(i));
        }
    }

    /// DMA helper accepting a prepared 256-byte buffer to avoid borrowing the bus during copy.
    #[allow(dead_code)]
    pub fn dma_oam_from_slice(&mut self, data: &[u8]) {
        for (i, b) in data.iter().take(256).enumerate() {
            self.oam[i] = *b;
        }
    }

    fn read_vram(&self, addr: u16) -> u8 {
        let a = addr & 0x3FFF;
        if a < 0x2000 {
            self.chr_fetch(a as usize)
        } else if a < 0x3F00 {
            let idx = self.map_nametable_addr(a);
            self.vram[idx]
        } else if a < 0x4000 {
            let p = (a - 0x3F00) & 0x1F;
            self.palette[palette_mirror_index(p as usize)]
        } else {
            0
        }
    }

    pub fn render_frame(&self) -> Frame {
        // Background-only renderer, with attribute table + palette.
        // Still very approximate, but produces colored and less-garbled output for many ROMs.
        let width = 256u32;
        let height = 240u32;
        let mut frame = Frame::new(width, height);

        let bg_enabled = (self.mask & 0x08) != 0;
        let sprites_enabled = (self.mask & 0x10) != 0;

        let bg_pattern_base: usize = if (self.ctrl & 0x10) != 0 {
            0x1000
        } else {
            0x0000
        };
        let base_nt = (self.ctrl & 0x03) as u8;

        // Universal background color is palette[$00].
        let mut universal_bg_idx = self.palette[palette_mirror_index(0)];
        if (self.mask & 0x01) != 0 {
            universal_bg_idx &= 0x30; // grayscale forces high bits only
        }
        let universal_bg = nes_palette_rgb(universal_bg_idx);

        // Apply scroll with basic nametable switching when crossing 256x240.
        // This approximates the PPU's coarse scroll behavior.
        let sx = self.scroll_x as u32;
        let sy = self.scroll_y as u32;

        // Background pass
        if bg_enabled {
            for y in 0..height {
                for x in 0..width {
                    let wx = x + sx;
                    let wy = y + sy;

                    let nt_x = ((wx / 256) & 1) as u8;
                    let nt_y = ((wy / 240) & 1) as u8;
                    // Choose nametable based on base + scroll crossing; avoid XOR so single-screen mirroring stays stable.
                    let nt = (base_nt + nt_x + (nt_y << 1)) & 0x03;

                    let world_x = wx % 256;
                    let world_y = wy % 240;

                    let tx = (world_x / 8) as usize;
                    let ty = (world_y / 8) as usize;
                    let fine_x = (world_x % 8) as usize;
                    let fine_y = (world_y % 8) as usize;

                    let nt_addr = 0x2000u16 + (nt as u16) * 0x0400;
                    let tile_addr = nt_addr + (ty as u16) * 32 + (tx as u16);
                    let tile_index = self.vram[self.map_nametable_addr(tile_addr)];

                    // Attribute table is at 0x3C0 within the nametable.
                    let attr_x = tx / 4;
                    let attr_y = ty / 4;
                    let attr_addr = nt_addr + 0x03C0 + (attr_y as u16) * 8 + (attr_x as u16);
                    let attr_byte = self.vram[self.map_nametable_addr(attr_addr)];
                    let quadrant = ((ty % 4) / 2) * 2 + ((tx % 4) / 2); // 0..3
                    let shift = (quadrant * 2) as u8;
                    let palette_idx = (attr_byte >> shift) & 0x03;

                    let tile_addr = bg_pattern_base + (tile_index as usize) * 16;
                    let lo = self.chr_fetch(tile_addr + fine_y);
                    let hi = self.chr_fetch(tile_addr + fine_y + 8);
                    let bit = 7 - fine_x;
                    let lo_bit = (lo >> bit) & 1;
                    let hi_bit = (hi >> bit) & 1;
                    let color_in_tile = (hi_bit << 1) | lo_bit; // 0..3

                    let out = if color_in_tile == 0 {
                        universal_bg
                    } else {
                        // Background palette layout in palette RAM:
                        // - $00 = universal background
                        // - $01..$03 = BG palette 0
                        // - $05..$07 = BG palette 1
                        // - $09..$0B = BG palette 2
                        // - $0D..$0F = BG palette 3
                        let pal_base = (palette_idx as usize) * 4;
                        let mut pal_entry =
                            self.palette[palette_mirror_index(pal_base + (color_in_tile as usize))];
                        if (self.mask & 0x01) != 0 {
                            pal_entry &= 0x30; // grayscale
                        }
                        nes_palette_rgb(pal_entry)
                    };

                    frame.pixels[(y * width + x) as usize] = out;
                }
            }
        } else {
            // Background disabled -> fill with universal background (close enough to black in many cases)
            for px in frame.pixels.iter_mut() {
                *px = universal_bg;
            }
        }

        // Sprite pass (minimal)
        if sprites_enabled {
            let sprite_size_16 = (self.ctrl & 0x20) != 0;
            let sprite_pattern_base: usize = if (self.ctrl & 0x08) != 0 {
                0x1000
            } else {
                0x0000
            };

            // NES draws sprites in reverse OAM order for priority; this is a simplification.
            for i in (0..64usize).rev() {
                let o = i * 4;
                let y_pos = self.oam[o] as i16 + 1; // OAM Y is sprite top minus 1
                let tile = self.oam[o + 1];
                let attr = self.oam[o + 2];
                let x_pos = self.oam[o + 3] as i16;

                let pal = (attr & 0x03) as usize;
                let behind_bg = (attr & 0x20) != 0;
                let flip_h = (attr & 0x40) != 0;
                let flip_v = (attr & 0x80) != 0;

                let (tile0, pattern_base, height_px) = if sprite_size_16 {
                    // 8x16: pattern table is selected by tile bit 0; tile index ignores bit 0.
                    let table = (tile & 1) as usize;
                    let base = if table != 0 { 0x1000 } else { 0x0000 };
                    (tile & 0xFE, base, 16)
                } else {
                    (tile, sprite_pattern_base, 8)
                };

                for row in 0..height_px {
                    let sy = if flip_v { height_px - 1 - row } else { row };
                    let y = y_pos + row as i16;
                    if y < 0 || y >= height as i16 {
                        continue;
                    }

                    let (tile_index, fine_y) = if height_px == 16 {
                        // top/bottom tile
                        if sy < 8 {
                            (tile0, sy as usize)
                        } else {
                            (tile0.wrapping_add(1), (sy - 8) as usize)
                        }
                    } else {
                        (tile0, sy as usize)
                    };

                    let addr = pattern_base + (tile_index as usize) * 16;
                    let lo = self.chr_fetch(addr + fine_y);
                    let hi = self.chr_fetch(addr + fine_y + 8);

                    for col in 0..8 {
                        let sx = if flip_h { col } else { 7 - col };
                        let x = x_pos + col as i16;
                        if x < 0 || x >= width as i16 {
                            continue;
                        }

                        let lo_bit = (lo >> sx) & 1;
                        let hi_bit = (hi >> sx) & 1;
                        let color = (hi_bit << 1) | lo_bit;
                        if color == 0 {
                            continue; // transparent
                        }

                        // Sprite palette layout:
                        // - $10 is sprite "universal" (mirrors $00), and $11..$13 are palette 0 colors, etc.
                        let pal_base = 0x11 + pal * 4;
                        let mut pal_entry =
                            self.palette[palette_mirror_index(pal_base + (color as usize) - 1)];
                        if (self.mask & 0x01) != 0 {
                            pal_entry &= 0x30; // grayscale
                        }
                        let rgb = nes_palette_rgb(pal_entry);

                        let idx = (y as u32 * width + x as u32) as usize;
                        if behind_bg {
                            // Behind background: only draw if background pixel is universal background.
                            if frame.pixels[idx] == universal_bg {
                                frame.pixels[idx] = rgb;
                            }
                        } else {
                            frame.pixels[idx] = rgb;
                        }
                    }
                }
            }
        }

        frame
    }
}
