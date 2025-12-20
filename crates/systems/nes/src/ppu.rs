//! NES PPU (Picture Processing Unit) implementation.
//!
//! This module implements the 2C02 PPU chip found in NTSC NES systems,
//! with support for PAL variants (2C07).
//!
//! ## Features
//!
//! - **Resolution**: 256x240 pixels
//! - **Colors**: 64-color master palette
//! - **Palettes**: 8 background + 8 sprite palettes (4 colors each)
//! - **Tiles**: 8x8 pixel tiles from CHR ROM/RAM
//! - **Sprites**: Up to 64 sprites (8x8 or 8x16 modes)
//! - **Scrolling**: Smooth scrolling with nametable switching
//! - **Mirroring**: Horizontal, vertical, four-screen, and single-screen
//!
//! ## Rendering Model
//!
//! This implementation uses a **frame-based** rendering model rather than
//! cycle-accurate scanline rendering:
//!
//! - Entire frames are rendered on-demand via `render_frame()`
//! - Scanlines can be rendered incrementally via `render_scanline()` for mapper CHR switching
//! - VBlank is simulated at the system level, not by the PPU
//! - **Sprite evaluation** is performed per scanline to set sprite overflow flag
//! - Sprite 0 hit detection is basic but functional
//!
//! This approach is suitable for most games but may not handle edge cases
//! requiring precise PPU timing (mid-scanline register changes, exact sprite 0 hit timing, etc.).
//!
//! ## Memory Map
//!
//! - **$0000-$1FFF**: CHR ROM/RAM (pattern tables)
//! - **$2000-$2FFF**: Nametables (mapped to 2KB internal VRAM via mirroring)
//! - **$3F00-$3FFF**: Palette RAM (32 bytes, mirrored)
//!
//! ## Register Interface
//!
//! - **$2000 (PPUCTRL)**: Control register (NMI enable, sprite size, etc.)
//! - **$2001 (PPUMASK)**: Mask register (enable background/sprites, grayscale, etc.)
//! - **$2002 (PPUSTATUS)**: Status register (VBlank flag, sprite 0 hit)
//! - **$2003 (OAMADDR)**: OAM address for $2004 access
//! - **$2004 (OAMDATA)**: OAM data read/write
//! - **$2005 (PPUSCROLL)**: Scroll position (write twice: X then Y)
//! - **$2006 (PPUADDR)**: VRAM address (write twice: high then low)
//! - **$2007 (PPUDATA)**: VRAM data read/write (with buffering)

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

// Offset to convert palette addresses ($3F00-$3FFF) to their mirrored nametable addresses ($2F00-$2FFF).
// When reading from palette RAM via PPUDATA, the internal buffer is filled with the mirrored nametable value.
const PALETTE_TO_NAMETABLE_OFFSET: u16 = 0x1000;

fn nes_palette_rgb(index: u8) -> u32 {
    NES_MASTER_PALETTE[(index & 0x3F) as usize]
}

fn palette_mirror_index(i: usize) -> usize {
    // Palette mirroring:
    // - $3F10/$3F14/$3F18/$3F1C (sprite palette color 0s) mirror $3F00/$3F04/$3F08/$3F0C
    // Note: $3F04/$3F08/$3F0C can contain unique data but are unused during rendering
    // since pattern value 0 always uses the backdrop color at $3F00
    match i & 0x1F {
        0x10 => 0x00,
        0x14 => 0x04,
        0x18 => 0x08,
        0x1C => 0x0C,
        v => v,
    }
}

/// NES PPU (Picture Processing Unit).
///
/// Implements the 2C02 PPU with frame-based rendering.
///
/// # Memory Layout
///
/// - `chr`: 8KB CHR ROM/RAM (pattern tables)
/// - `vram`: 2KB internal VRAM (nametables)
/// - `palette`: 32 bytes palette RAM
/// - `oam`: 256 bytes Object Attribute Memory (sprites)
///
/// # Register State
///
/// - `ctrl`: PPUCTRL ($2000)
/// - `mask`: PPUMASK ($2001)
/// - `vblank`: VBlank flag (PPUSTATUS bit 7)
/// - `sprite_0_hit`: Sprite 0 hit flag (PPUSTATUS bit 6)
/// - `sprite_overflow`: Sprite overflow flag (PPUSTATUS bit 5)
/// - `nmi_pending`: Pending NMI request
/// - `vram_addr`: Current VRAM address
/// - `scroll_x`, `scroll_y`: Scroll position
///
/// # Callbacks
///
/// - `a12_callback`: Notifies mappers of A12 line changes (for IRQ timing)
/// - `chr_read_callback`: Notifies mappers of CHR reads (for latch switching)
pub struct Ppu {
    pub chr: Vec<u8>,
    chr_is_ram: bool,
    pub vram: [u8; 0x800], // 2KB internal VRAM (nametables)
    pub palette: [u8; 32],
    pub oam: [u8; 256],
    mirroring: Mirroring,
    ctrl: u8,
    mask: u8,
    // PPUSTATUS flags
    vblank: Cell<bool>,
    sprite_0_hit: Cell<bool>,
    sprite_overflow: Cell<bool>,
    nmi_pending: Cell<bool>,
    // PPUADDR latch
    addr_latch: Cell<bool>,
    pub vram_addr: Cell<u16>,
    read_buffer: Cell<u8>,
    #[allow(clippy::type_complexity)]
    a12_callback: RefCell<Option<Box<dyn FnMut(bool)>>>,
    #[allow(clippy::type_complexity)]
    chr_read_callback: RefCell<Option<Box<dyn FnMut(u16)>>>,
    suppress_a12: Cell<bool>,
    scroll_x: u8,
    scroll_y: u8,
    oam_addr: Cell<u8>,
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
            sprite_0_hit: Cell::new(false),
            sprite_overflow: Cell::new(false),
            nmi_pending: Cell::new(false),
            addr_latch: Cell::new(false),
            vram_addr: Cell::new(0),
            read_buffer: Cell::new(0),
            a12_callback: RefCell::new(None),
            chr_read_callback: RefCell::new(None),
            suppress_a12: Cell::new(false),
            scroll_x: 0,
            scroll_y: 0,
            oam_addr: Cell::new(0),
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
    #[cfg(test)]
    pub fn get_mirroring(&self) -> Mirroring {
        self.mirroring
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

    /// Set/clear the VBlank flag (PPUSTATUS bit 7).
    pub fn set_vblank(&self, v: bool) {
        let prev = self.vblank.replace(v);
        if v && !prev && self.nmi_enabled() {
            // VBlank just started and NMI is enabled.
            self.nmi_pending.set(true);
        }
        if !v {
            // Clear sprite 0 hit and sprite overflow at the start of vblank
            self.sprite_0_hit.set(false);
            self.sprite_overflow.set(false);
        }
    }

    pub fn vblank_flag(&self) -> bool {
        self.vblank.get()
    }

    /// Check and clear a pending NMI request generated by the PPU.
    pub fn take_nmi_pending(&self) -> bool {
        let was = self.nmi_pending.get();
        self.nmi_pending.set(false);
        was
    }

    pub fn set_a12_callback(&self, cb: Option<Box<dyn FnMut(bool)>>) {
        *self.a12_callback.borrow_mut() = cb;
    }

    pub fn set_chr_read_callback(&self, cb: Option<Box<dyn FnMut(u16)>>) {
        *self.chr_read_callback.borrow_mut() = cb;
    }

    fn chr_fetch(&self, addr: usize) -> u8 {
        // Notify mapper about PPU A12 line (bit 12 of CHR address) transitions.
        if !self.suppress_a12.get() {
            if let Some(cb) = &mut *self.a12_callback.borrow_mut() {
                let a12_high = (addr & 0x1000) != 0;
                cb(a12_high);
            }
        }
        // Notify mapper about CHR reads (for MMC2/MMC4 latch switching).
        // This runs even when suppress_a12 is true, during frame rendering.
        if let Some(cb) = &mut *self.chr_read_callback.borrow_mut() {
            cb(addr as u16);
        }
        self.chr.get(addr).copied().unwrap_or(0)
    }

    /// Read a PPU register (very partial implementation).
    pub fn read_register(&self, reg: u16) -> u8 {
        match reg & 0x7 {
            2 => {
                // PPUSTATUS: bit 7 = vblank, bit 6 = sprite 0 hit, bit 5 = sprite overflow
                let mut status = 0u8;
                if self.vblank.get() {
                    status |= 0x80;
                }
                if self.sprite_0_hit.get() {
                    status |= 0x40;
                }
                if self.sprite_overflow.get() {
                    status |= 0x20;
                }
                // Reading PPUSTATUS clears vblank and resets address latch.
                self.vblank.set(false);
                self.addr_latch.set(false);
                status
            }
            4 => {
                // OAMDATA read: return current OAM byte at oam_addr
                let addr = self.oam_addr.get() as usize;
                self.oam[addr]
            }
            7 => {
                // PPUDATA read with buffered behavior.
                let addr = self.vram_addr.get() & 0x3FFF;

                // Palette reads return the palette value immediately (not buffered),
                // but still update the internal buffer with the mirrored nametable value.
                // Palette addresses $3F00-$3FFF mirror the nametable at $2F00-$2FFF.
                if addr >= 0x3F00 {
                    let p = (addr - 0x3F00) & 0x1F;
                    let target = palette_mirror_index(p as usize);
                    let val = self.palette[target];

                    // Fill buffer with the mirrored nametable value underneath
                    let mirrored_nt_addr = addr - PALETTE_TO_NAMETABLE_OFFSET;
                    let idx = self.map_nametable_addr(mirrored_nt_addr);
                    self.read_buffer.set(self.vram[idx]);

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
                let old_nmi = (self.ctrl & 0x80) != 0;
                self.ctrl = val;
                let new_nmi = (self.ctrl & 0x80) != 0;
                // If NMI gets enabled while already in VBlank, the PPU triggers an NMI.
                if !old_nmi && new_nmi && self.vblank.get() {
                    self.nmi_pending.set(true);
                }
            }
            1 => {
                // PPUMASK
                self.mask = val;
            }
            3 => {
                // OAMADDR: set OAM address for $2004 access
                self.oam_addr.set(val);
            }
            4 => {
                // OAMDATA: write to OAM at current address, then increment
                let addr = self.oam_addr.get() as usize;
                self.oam[addr] = val;
                self.oam_addr.set(self.oam_addr.get().wrapping_add(1));
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
                } else if addr < 0x3F00 {
                    // Nametable VRAM space with mirroring
                    let idx = self.map_nametable_addr(addr);
                    self.vram[idx] = val;
                } else {
                    // Palette RAM: $3F00-$3FFF with 32-byte mirroring
                    // (addr is already masked to 0x3FFF, so this handles $3F00-$3FFF)
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

    /// Evaluate sprites for a scanline to determine sprite overflow.
    ///
    /// The NES PPU can only display 8 sprites per scanline. If more than 8 sprites
    /// are on the same scanline, the sprite overflow flag is set.
    ///
    /// This is a simplified version of the hardware sprite evaluation process.
    fn evaluate_sprites_for_scanline(&self, scanline: u32) {
        let sprite_size_16 = (self.ctrl & 0x20) != 0;
        let sprite_height = if sprite_size_16 { 16 } else { 8 };

        let mut sprites_found = 0;

        // Check all 64 sprites in OAM
        for i in 0..64 {
            let o = i * 4;
            let y_pos = self.oam[o] as i16 + 1;

            // Check if this sprite is on the current scanline
            let row = (scanline as i16) - y_pos;
            if row >= 0 && row < sprite_height {
                sprites_found += 1;

                // If we found more than 8 sprites on this scanline, set overflow
                if sprites_found > 8 {
                    self.sprite_overflow.set(true);
                    return;
                }
            }
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
    #[cfg(test)]
    pub fn render_frame(&self) -> Frame {
        // Rendering is done "out of band" (not cycle-accurate). Suppress A12 callbacks
        // so mappers like MMC3 don't see thousands of synthetic edges during draw.
        let prev_suppress = self.suppress_a12.replace(true);

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

        self.suppress_a12.set(prev_suppress);
        frame
    }

    /// Render a single scanline into an existing frame.
    ///
    /// This is a pragmatic helper for mappers (notably MMC3) that change CHR banks mid-frame.
    /// By rendering scanlines incrementally, the frame output can reflect CHR/scroll changes
    /// that occur between scanlines even in this non-cycle-accurate renderer.
    ///
    /// This version includes sprite evaluation to set sprite overflow flag.
    pub fn render_scanline(&self, y: u32, frame: &mut Frame) {
        if y >= 240 {
            return;
        }

        let prev_suppress = self.suppress_a12.replace(true);

        let width = 256u32;
        let height = 240u32;
        if frame.width != width || frame.height != height {
            // Only supports native NES output size.
            self.suppress_a12.set(prev_suppress);
            return;
        }

        let bg_enabled = (self.mask & 0x08) != 0;
        let sprites_enabled = (self.mask & 0x10) != 0;

        // Perform sprite evaluation for this scanline to determine sprite overflow
        if sprites_enabled {
            self.evaluate_sprites_for_scanline(y);
        }

        let bg_pattern_base: usize = if (self.ctrl & 0x10) != 0 {
            0x1000
        } else {
            0x0000
        };
        let base_nt = (self.ctrl & 0x03) as u8;

        let mut universal_bg_idx = self.palette[palette_mirror_index(0)];
        if (self.mask & 0x01) != 0 {
            universal_bg_idx &= 0x30;
        }
        let universal_bg = nes_palette_rgb(universal_bg_idx);

        let sx = self.scroll_x as u32;
        let sy = self.scroll_y as u32;

        // Background pixels for this scanline.
        if bg_enabled {
            for x in 0..width {
                let wx = x + sx;
                let wy = y + sy;

                let nt_x = ((wx / 256) & 1) as u8;
                let nt_y = ((wy / 240) & 1) as u8;
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

                let attr_x = tx / 4;
                let attr_y = ty / 4;
                let attr_addr = nt_addr + 0x03C0 + (attr_y as u16) * 8 + (attr_x as u16);
                let attr_byte = self.vram[self.map_nametable_addr(attr_addr)];
                let quadrant = ((ty % 4) / 2) * 2 + ((tx % 4) / 2);
                let shift = (quadrant * 2) as u8;
                let palette_idx = (attr_byte >> shift) & 0x03;

                let tile_addr = bg_pattern_base + (tile_index as usize) * 16;
                let lo = self.chr_fetch(tile_addr + fine_y);
                let hi = self.chr_fetch(tile_addr + fine_y + 8);
                let bit = 7 - fine_x;
                let lo_bit = (lo >> bit) & 1;
                let hi_bit = (hi >> bit) & 1;
                let color_in_tile = (hi_bit << 1) | lo_bit;

                let out = if color_in_tile == 0 {
                    universal_bg
                } else {
                    let pal_base = (palette_idx as usize) * 4;
                    let mut pal_entry =
                        self.palette[palette_mirror_index(pal_base + (color_in_tile as usize))];
                    if (self.mask & 0x01) != 0 {
                        pal_entry &= 0x30;
                    }
                    nes_palette_rgb(pal_entry)
                };

                frame.pixels[(y * width + x) as usize] = out;
            }
        } else {
            // Background disabled: fill this scanline with backdrop.
            let row_start = (y * width) as usize;
            for px in &mut frame.pixels[row_start..row_start + width as usize] {
                *px = universal_bg;
            }
        }

        // Sprites affecting this scanline.
        if sprites_enabled {
            let sprite_size_16 = (self.ctrl & 0x20) != 0;
            let sprite_pattern_base: usize = if (self.ctrl & 0x08) != 0 {
                0x1000
            } else {
                0x0000
            };

            for i in (0..64usize).rev() {
                let o = i * 4;
                let y_pos = self.oam[o] as i16 + 1;
                let tile = self.oam[o + 1];
                let attr = self.oam[o + 2];
                let x_pos = self.oam[o + 3] as i16;

                let pal = (attr & 0x03) as usize;
                let behind_bg = (attr & 0x20) != 0;
                let flip_h = (attr & 0x40) != 0;
                let flip_v = (attr & 0x80) != 0;

                let (tile0, pattern_base, height_px) = if sprite_size_16 {
                    let table = (tile & 1) as usize;
                    let base = if table != 0 { 0x1000 } else { 0x0000 };
                    (tile & 0xFE, base, 16)
                } else {
                    (tile, sprite_pattern_base, 8)
                };

                let row = (y as i16) - y_pos;
                if row < 0 || row >= height_px {
                    continue;
                }

                let sy = if flip_v { height_px - 1 - row } else { row };
                let (tile_index, fine_y) = if height_px == 16 {
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
                    let sx_bit = if flip_h { col } else { 7 - col };
                    let x = x_pos + col as i16;
                    if x < 0 || x >= width as i16 {
                        continue;
                    }

                    let lo_bit = (lo >> sx_bit) & 1;
                    let hi_bit = (hi >> sx_bit) & 1;
                    let color = (hi_bit << 1) | lo_bit;
                    if color == 0 {
                        continue;
                    }

                    let pal_base = 0x11 + pal * 4;
                    let mut pal_entry =
                        self.palette[palette_mirror_index(pal_base + (color as usize) - 1)];
                    if (self.mask & 0x01) != 0 {
                        pal_entry &= 0x30;
                    }
                    let rgb = nes_palette_rgb(pal_entry);

                    let idx = (y * width + x as u32) as usize;

                    // Sprite 0 hit detection
                    if i == 0 && bg_enabled && !self.sprite_0_hit.get() {
                        // Check if background pixel is opaque (approximate by color)
                        if frame.pixels[idx] != universal_bg && x < 255 {
                            // Check left clipping
                            let bg_clip = (self.mask & 0x02) == 0;
                            let spr_clip = (self.mask & 0x04) == 0;
                            if !((bg_clip || spr_clip) && x < 8) {
                                self.sprite_0_hit.set(true);
                            }
                        }
                    }

                    if behind_bg {
                        if frame.pixels[idx] == universal_bg {
                            frame.pixels[idx] = rgb;
                        }
                    } else {
                        frame.pixels[idx] = rgb;
                    }
                }
            }
        }

        self.suppress_a12.set(prev_suppress);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_palette_mirror_index() {
        // Universal background at $3F00
        assert_eq!(palette_mirror_index(0x00), 0x00);

        // BG palette 0 colors 1-3 should not mirror
        assert_eq!(palette_mirror_index(0x01), 0x01);
        assert_eq!(palette_mirror_index(0x02), 0x02);
        assert_eq!(palette_mirror_index(0x03), 0x03);

        // BG palette 1 color 0 - can hold unique data (not used in rendering)
        assert_eq!(palette_mirror_index(0x04), 0x04);
        // BG palette 1 colors 1-3 should not mirror
        assert_eq!(palette_mirror_index(0x05), 0x05);
        assert_eq!(palette_mirror_index(0x06), 0x06);
        assert_eq!(palette_mirror_index(0x07), 0x07);

        // BG palette 2 color 0 - can hold unique data (not used in rendering)
        assert_eq!(palette_mirror_index(0x08), 0x08);
        assert_eq!(palette_mirror_index(0x09), 0x09);
        assert_eq!(palette_mirror_index(0x0A), 0x0A);
        assert_eq!(palette_mirror_index(0x0B), 0x0B);

        // BG palette 3 color 0 - can hold unique data (not used in rendering)
        assert_eq!(palette_mirror_index(0x0C), 0x0C);
        assert_eq!(palette_mirror_index(0x0D), 0x0D);
        assert_eq!(palette_mirror_index(0x0E), 0x0E);
        assert_eq!(palette_mirror_index(0x0F), 0x0F);

        // Sprite palette 0 color 0 should mirror to $3F00
        assert_eq!(palette_mirror_index(0x10), 0x00);
        assert_eq!(palette_mirror_index(0x11), 0x11);
        assert_eq!(palette_mirror_index(0x12), 0x12);
        assert_eq!(palette_mirror_index(0x13), 0x13);

        // Sprite palette 1 color 0 should mirror to $3F04
        assert_eq!(palette_mirror_index(0x14), 0x04);
        assert_eq!(palette_mirror_index(0x15), 0x15);
        assert_eq!(palette_mirror_index(0x16), 0x16);
        assert_eq!(palette_mirror_index(0x17), 0x17);

        // Sprite palette 2 color 0 should mirror to $3F08
        assert_eq!(palette_mirror_index(0x18), 0x08);
        assert_eq!(palette_mirror_index(0x19), 0x19);
        assert_eq!(palette_mirror_index(0x1A), 0x1A);
        assert_eq!(palette_mirror_index(0x1B), 0x1B);

        // Sprite palette 3 color 0 should mirror to $3F0C
        assert_eq!(palette_mirror_index(0x1C), 0x0C);
        assert_eq!(palette_mirror_index(0x1D), 0x1D);
        assert_eq!(palette_mirror_index(0x1E), 0x1E);
        assert_eq!(palette_mirror_index(0x1F), 0x1F);
    }

    #[test]
    fn test_palette_writes_and_reads() {
        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal);

        // Write to universal background
        ppu.write_register(6, 0x3F); // PPUADDR high
        ppu.write_register(6, 0x00); // PPUADDR low
        ppu.write_register(7, 0x0F); // Write black to universal bg

        // Read back from universal background
        ppu.vram_addr.set(0x3F00);
        let val = ppu.read_register(7);
        assert_eq!(val, 0x0F);

        // Write to BG palette 1 color 0 - does NOT mirror, holds unique data
        ppu.write_register(6, 0x3F); // PPUADDR high
        ppu.write_register(6, 0x04); // PPUADDR low
        ppu.write_register(7, 0x30); // Write white

        // Read back from $3F04 - should see what we wrote
        ppu.vram_addr.set(0x3F04);
        let val = ppu.read_register(7);
        assert_eq!(val, 0x30);

        // Universal background should still be 0x0F (not affected)
        ppu.vram_addr.set(0x3F00);
        let val = ppu.read_register(7);
        assert_eq!(val, 0x0F);

        // Write to sprite palette 0 color 0 ($3F10) - should mirror to $3F00
        ppu.write_register(6, 0x3F); // PPUADDR high
        ppu.write_register(6, 0x10); // PPUADDR low
        ppu.write_register(7, 0x20); // Write a color

        // Read back from $3F00 - should see the mirrored value
        ppu.vram_addr.set(0x3F00);
        let val = ppu.read_register(7);
        assert_eq!(val, 0x20);

        // Read back from $3F10 - should also see the same value
        ppu.vram_addr.set(0x3F10);
        let val = ppu.read_register(7);
        assert_eq!(val, 0x20);

        // Write to sprite palette 1 color 0 ($3F14) - should mirror to $3F04
        ppu.write_register(6, 0x3F); // PPUADDR high
        ppu.write_register(6, 0x14); // PPUADDR low
        ppu.write_register(7, 0x25); // Write a color

        // Read back from $3F04 - should see the mirrored value
        ppu.vram_addr.set(0x3F04);
        let val = ppu.read_register(7);
        assert_eq!(val, 0x25);

        // Read back from $3F14 - should also see the same value
        ppu.vram_addr.set(0x3F14);
        let val = ppu.read_register(7);
        assert_eq!(val, 0x25);
    }

    #[test]
    fn test_background_palette_rendering() {
        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal);

        // Set up a simple 8x8 tile in CHR-ROM (requires CHR-RAM for test)
        ppu.chr_is_ram = true;
        // Tile pattern: checkerboard pattern
        // Low plane: 0b10101010
        ppu.chr[0] = 0b10101010;
        ppu.chr[1] = 0b01010101;
        ppu.chr[2] = 0b10101010;
        ppu.chr[3] = 0b01010101;
        ppu.chr[4] = 0b10101010;
        ppu.chr[5] = 0b01010101;
        ppu.chr[6] = 0b10101010;
        ppu.chr[7] = 0b01010101;
        // High plane: 0b11110000
        ppu.chr[8] = 0b11110000;
        ppu.chr[9] = 0b11110000;
        ppu.chr[10] = 0b11110000;
        ppu.chr[11] = 0b11110000;
        ppu.chr[12] = 0b00001111;
        ppu.chr[13] = 0b00001111;
        ppu.chr[14] = 0b00001111;
        ppu.chr[15] = 0b00001111;

        // Set up palette: universal bg + 3 colors for palette 0
        ppu.palette[0] = 0x0F; // Universal background (black)
        ppu.palette[1] = 0x30; // Color 1 (white)
        ppu.palette[2] = 0x16; // Color 2 (red)
        ppu.palette[3] = 0x27; // Color 3 (green)

        // Enable background rendering
        ppu.mask = 0x08; // Show background

        // Set first nametable tile to use tile 0
        ppu.vram[0] = 0;

        // Set attribute to use palette 0
        let attr_addr = ppu.map_nametable_addr(0x23C0);
        ppu.vram[attr_addr] = 0x00; // Palette 0 for all quadrants

        // Render frame
        let frame = ppu.render_frame();

        // Check that different colors are rendered
        // Top-left pixel should combine lo=1, hi=1 = color 3
        let pixel0 = frame.pixels[0];
        assert_eq!(pixel0, nes_palette_rgb(0x27)); // Color 3 (green)

        // Second pixel should combine lo=0, hi=1 = color 2
        let pixel1 = frame.pixels[1];
        assert_eq!(pixel1, nes_palette_rgb(0x16)); // Color 2 (red)
    }

    #[test]
    fn test_palette_color_zero_uses_backdrop() {
        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal);
        ppu.chr_is_ram = true;

        // Set up a tile where all pixels are color 0 (both planes 0)
        for i in 0..16 {
            ppu.chr[i] = 0;
        }

        // Set different values for universal bg and palette 1 color 0
        ppu.palette[0] = 0x0F; // Universal background (black)
        ppu.palette[4] = 0x30; // BG palette 1 color 0 (white) - should be ignored

        // Enable background rendering
        ppu.mask = 0x08;

        // Set first tile to use tile 0
        ppu.vram[0] = 0;

        // Set attribute to use palette 1 (not palette 0)
        let attr_addr = ppu.map_nametable_addr(0x23C0);
        ppu.vram[attr_addr] = 0x01; // Palette 1 for first quadrant

        // Render frame
        let frame = ppu.render_frame();

        // All pixels should use universal background, not palette 1 color 0
        let pixel = frame.pixels[0];
        assert_eq!(pixel, nes_palette_rgb(0x0F)); // Should be black, not white
    }

    #[test]
    fn test_nes_palette_rgb() {
        // Test that master palette lookup works correctly
        assert_eq!(nes_palette_rgb(0x0F), 0xFF000000); // Black
        assert_eq!(nes_palette_rgb(0x30), 0xFFECEEEC); // White

        // Test that only lower 6 bits are used (& 0x3F)
        assert_eq!(nes_palette_rgb(0x4F), nes_palette_rgb(0x0F)); // Same as 0x0F
        assert_eq!(nes_palette_rgb(0xFF), nes_palette_rgb(0x3F)); // Same as 0x3F
    }

    #[test]
    fn test_palette_ram_mirrors_throughout_range() {
        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal);

        // Write to $3F00 (universal background)
        ppu.write_register(6, 0x3F);
        ppu.write_register(6, 0x00);
        ppu.write_register(7, 0x0F); // Black

        // Read from $3F20 (should mirror to $3F00)
        ppu.vram_addr.set(0x3F20);
        let val = ppu.read_register(7);
        assert_eq!(val, 0x0F);

        // Read from $3F40 (should also mirror to $3F00)
        ppu.vram_addr.set(0x3F40);
        let val = ppu.read_register(7);
        assert_eq!(val, 0x0F);

        // Write to $3F25 (should mirror to $3F05)
        ppu.write_register(6, 0x3F);
        ppu.write_register(6, 0x25);
        ppu.write_register(7, 0x16); // Red

        // Read from $3F05 directly
        ppu.vram_addr.set(0x3F05);
        let val = ppu.read_register(7);
        assert_eq!(val, 0x16);

        // Read from $3F45 (should also mirror to $3F05)
        ppu.vram_addr.set(0x3F45);
        let val = ppu.read_register(7);
        assert_eq!(val, 0x16);

        // Write to $3FF0 (should mirror to $3F10, which mirrors to $3F00)
        ppu.write_register(6, 0x3F);
        ppu.write_register(6, 0xF0);
        ppu.write_register(7, 0x30); // White

        // Universal background should now be white
        ppu.vram_addr.set(0x3F00);
        let val = ppu.read_register(7);
        assert_eq!(val, 0x30);
    }

    #[test]
    fn test_sprite_overflow_flag() {
        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal);

        // Enable sprite rendering
        ppu.ctrl = 0x00; // 8x8 sprites
        ppu.mask = 0x10; // Show sprites

        // Place 9 sprites on scanline 100
        for i in 0..9 {
            ppu.oam[i * 4] = 99; // Y position (sprite top is Y+1, so scanline 100)
            ppu.oam[i * 4 + 1] = 0; // Tile index
            ppu.oam[i * 4 + 2] = 0; // Attributes
            ppu.oam[i * 4 + 3] = i as u8 * 8; // X position
        }

        // Evaluate sprites for scanline 100
        ppu.evaluate_sprites_for_scanline(100);

        // Sprite overflow flag should be set
        assert!(ppu.sprite_overflow.get());

        // Reading PPUSTATUS should return sprite overflow bit (bit 5)
        let status = ppu.read_register(2);
        assert_eq!(status & 0x20, 0x20);
    }

    #[test]
    fn test_sprite_overflow_not_set_with_8_sprites() {
        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal);

        // Enable sprite rendering
        ppu.ctrl = 0x00; // 8x8 sprites
        ppu.mask = 0x10; // Show sprites

        // Place exactly 8 sprites on scanline 100
        for i in 0..8 {
            ppu.oam[i * 4] = 99; // Y position
            ppu.oam[i * 4 + 1] = 0; // Tile index
            ppu.oam[i * 4 + 2] = 0; // Attributes
            ppu.oam[i * 4 + 3] = i as u8 * 8; // X position
        }

        // Evaluate sprites for scanline 100
        ppu.evaluate_sprites_for_scanline(100);

        // Sprite overflow flag should NOT be set
        assert!(!ppu.sprite_overflow.get());

        // Reading PPUSTATUS should not have sprite overflow bit set
        let status = ppu.read_register(2);
        assert_eq!(status & 0x20, 0x00);
    }

    #[test]
    fn test_sprite_overflow_with_16_pixel_sprites() {
        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal);

        // Enable 8x16 sprite mode
        ppu.ctrl = 0x20; // 8x16 sprites
        ppu.mask = 0x10; // Show sprites

        // Place 9 8x16 sprites on scanline 100
        for i in 0..9 {
            ppu.oam[i * 4] = 99; // Y position (sprite extends from scanline 100-115)
            ppu.oam[i * 4 + 1] = 0; // Tile index
            ppu.oam[i * 4 + 2] = 0; // Attributes
            ppu.oam[i * 4 + 3] = i as u8 * 8; // X position
        }

        // Evaluate sprites for scanline 100 (first scanline of the sprite)
        ppu.evaluate_sprites_for_scanline(100);

        // Sprite overflow flag should be set
        assert!(ppu.sprite_overflow.get());

        // Evaluate for scanline 110 (middle of 8x16 sprite)
        ppu.sprite_overflow.set(false); // Reset flag
        ppu.evaluate_sprites_for_scanline(110);

        // Should still detect overflow
        assert!(ppu.sprite_overflow.get());
    }

    #[test]
    fn test_vblank_clears_sprite_flags() {
        let ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal);

        // Set sprite 0 hit and sprite overflow
        ppu.sprite_0_hit.set(true);
        ppu.sprite_overflow.set(true);

        // Verify flags are set
        assert!(ppu.sprite_0_hit.get());
        assert!(ppu.sprite_overflow.get());

        // Start VBlank
        ppu.set_vblank(true);

        // Flags should still be set during VBlank
        assert!(ppu.sprite_0_hit.get());
        assert!(ppu.sprite_overflow.get());

        // End VBlank (start of new frame)
        ppu.set_vblank(false);

        // Flags should be cleared at start of frame
        assert!(!ppu.sprite_0_hit.get());
        assert!(!ppu.sprite_overflow.get());
    }

    #[test]
    fn test_palette_read_updates_buffer_with_mirrored_nametable() {
        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal);

        // Write a distinctive value to nametable at $2F00 (which mirrors to palette $3F00)
        let nt_addr = 0x2F00;
        let idx = ppu.map_nametable_addr(nt_addr);
        ppu.vram[idx] = 0xAB;

        // Write a palette value to $3F00
        ppu.write_register(6, 0x3F);
        ppu.write_register(6, 0x00);
        ppu.write_register(7, 0x1C); // Palette value

        // Reset address to read from palette $3F00
        ppu.write_register(6, 0x3F);
        ppu.write_register(6, 0x00);

        // First read from palette should return the palette value immediately
        let palette_val = ppu.read_register(7);
        assert_eq!(palette_val, 0x1C);

        // Now read from a non-palette address (e.g., $2000)
        ppu.write_register(6, 0x20);
        ppu.write_register(6, 0x00);

        // This should return the buffered value (the mirrored nametable value from $2F00)
        let buffered = ppu.read_register(7);
        assert_eq!(
            buffered, 0xAB,
            "Buffer should contain mirrored nametable value from palette read"
        );
    }

    #[test]
    fn test_palette_mirroring_multiple_addresses() {
        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal);

        // Test that different palette addresses ($3F00-$3FFF) mirror to corresponding nametable addresses
        // $3F00 -> $2F00, $3F10 -> $2F10, $3F20 -> $2F20 (with 32-byte palette mirroring)

        // Set up different values in nametable at $2F00, $2F10, $2F20
        let nt_addr_1 = 0x2F00;
        let idx_1 = ppu.map_nametable_addr(nt_addr_1);
        ppu.vram[idx_1] = 0x11;

        let nt_addr_2 = 0x2F10;
        let idx_2 = ppu.map_nametable_addr(nt_addr_2);
        ppu.vram[idx_2] = 0x22;

        let nt_addr_3 = 0x2F1F;
        let idx_3 = ppu.map_nametable_addr(nt_addr_3);
        ppu.vram[idx_3] = 0x33;

        // Read from palette $3F00
        ppu.write_register(6, 0x3F);
        ppu.write_register(6, 0x00);
        ppu.read_register(7); // Palette value (discard)

        // Read from CHR to get buffered value
        ppu.write_register(6, 0x00);
        ppu.write_register(6, 0x00);
        let buffered_1 = ppu.read_register(7);
        assert_eq!(buffered_1, 0x11, "Buffer should contain value from $2F00");

        // Read from palette $3F10 (mirrors to $3F10 in palette, $2F10 in nametable)
        ppu.write_register(6, 0x3F);
        ppu.write_register(6, 0x10);
        ppu.read_register(7); // Palette value (discard)

        // Read from CHR to get buffered value
        ppu.write_register(6, 0x00);
        ppu.write_register(6, 0x00);
        let buffered_2 = ppu.read_register(7);
        assert_eq!(buffered_2, 0x22, "Buffer should contain value from $2F10");

        // Read from palette $3F1F
        ppu.write_register(6, 0x3F);
        ppu.write_register(6, 0x1F);
        ppu.read_register(7); // Palette value (discard)

        // Read from CHR to get buffered value
        ppu.write_register(6, 0x00);
        ppu.write_register(6, 0x00);
        let buffered_3 = ppu.read_register(7);
        assert_eq!(buffered_3, 0x33, "Buffer should contain value from $2F1F");
    }

    #[test]
    fn test_palette_mirroring_with_32byte_wrap() {
        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal);

        // Test that palette addresses mirror every 32 bytes
        // $3F20 should mirror to $3F00 for palette data, but $2F20 for buffer

        let nt_addr = 0x2F20;
        let idx = ppu.map_nametable_addr(nt_addr);
        ppu.vram[idx] = 0xCD;

        // Write different values to $3F00 and verify $3F20 reads the same palette value
        ppu.write_register(6, 0x3F);
        ppu.write_register(6, 0x00);
        ppu.write_register(7, 0x0F); // Write to $3F00

        // Read from $3F20 (should return same palette value as $3F00 due to mirroring)
        ppu.write_register(6, 0x3F);
        ppu.write_register(6, 0x20);
        let palette_val = ppu.read_register(7);
        assert_eq!(palette_val, 0x0F, "Palette should mirror every 32 bytes");

        // But the buffer should contain the value from $2F20, not $2F00
        ppu.write_register(6, 0x00);
        ppu.write_register(6, 0x00);
        let buffered = ppu.read_register(7);
        assert_eq!(buffered, 0xCD, "Buffer should contain value from $2F20");
    }

    #[test]
    fn test_palette_mirroring_across_nametable_boundaries() {
        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::Vertical);

        // Test palette mirroring with different nametable mirroring modes
        // With vertical mirroring, $2F00 and $2F00+$400 map differently

        // Set value in first nametable
        let nt_addr_1 = 0x2F00;
        let idx_1 = ppu.map_nametable_addr(nt_addr_1);
        ppu.vram[idx_1] = 0xAA;

        // Set value in second nametable (vertical mirroring)
        let nt_addr_2 = 0x2F00 + 0x400;
        let idx_2 = ppu.map_nametable_addr(nt_addr_2);
        ppu.vram[idx_2] = 0xBB;

        // Read from palette $3F00 (mirrors to $2F00)
        ppu.write_register(6, 0x3F);
        ppu.write_register(6, 0x00);
        ppu.read_register(7); // Palette value (discard)

        // Check buffer contains value from first nametable
        ppu.write_register(6, 0x00);
        ppu.write_register(6, 0x00);
        let buffered_1 = ppu.read_register(7);
        assert_eq!(
            buffered_1, 0xAA,
            "Buffer should respect nametable mirroring"
        );

        // Read from palette $3F00+$400 (would map to $2F00+$400 = $3300)
        // But $3300 is outside palette range, so this tests normal VRAM reads
        ppu.write_register(6, 0x33);
        ppu.write_register(6, 0x00);
        ppu.read_register(7); // Discard buffered
        let nt_val = ppu.read_register(7);
        // This should read from $3300, which maps to nametable
        // Just verify it doesn't crash
        let _ = nt_val;
    }

    #[test]
    fn test_sequential_palette_reads_update_buffer() {
        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal);

        // Set up multiple values in nametable
        for i in 0..32 {
            let nt_addr = 0x2F00 + i;
            let idx = ppu.map_nametable_addr(nt_addr);
            ppu.vram[idx] = (0x50 + i) as u8;
        }

        // Set up palette with increment-by-1 mode
        ppu.ctrl = 0x00; // Increment by 1

        // Set palette address to $3F00
        ppu.write_register(6, 0x3F);
        ppu.write_register(6, 0x00);

        // Read multiple palette values sequentially
        for _ in 0..8 {
            ppu.read_register(7); // Each palette read updates the buffer
        }

        // Now read from a non-palette address to verify the buffer was updated
        ppu.write_register(6, 0x00);
        ppu.write_register(6, 0x00);
        let buffered = ppu.read_register(7);

        // Buffer should contain the mirrored value from the last palette read
        // Last read was from $3F00+7=$3F07, which mirrors to $2F07
        let expected_idx = ppu.map_nametable_addr(0x2F07);
        let expected = ppu.vram[expected_idx];
        assert_eq!(
            buffered, expected,
            "Sequential palette reads should update buffer each time"
        );
    }
}
