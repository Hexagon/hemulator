//! PlayStation 1 GPU — Graphics Processing Unit
//!
//! The PS1 GPU handles 2D/3D rendering with:
//! - 1MB VRAM (1024×512 pixels, 16-bit)
//! - Flat-shaded and Gouraud-shaded polygons
//! - Textured and untextured primitives
//! - 4-bit, 8-bit, and 15-bit texture modes
//! - Semi-transparency blending
//! - Sprite rendering
//! - Display resolution up to 640×480
//!
//! Communication via two 32-bit ports:
//! - GP0 (0x1F801810): Rendering commands and VRAM access
//! - GP1 (0x1F801814): Display control commands
//!
//! ## References
//! - nocash PSX-SPX GPU documentation
//! - Martin Korth's PSX GPU timing info

use emu_core::renderer::Renderer;
use emu_core::types::Frame;
use std::cell::Cell;

/// VRAM dimensions
const VRAM_WIDTH: usize = 1024;
const VRAM_HEIGHT: usize = 512;

/// GPU state machine for GP0 command processing
#[derive(Debug, Clone, Copy, PartialEq)]
enum Gp0Mode {
    /// Waiting for a new command
    Command,
    /// Receiving parameters for a command
    Params,
    /// Receiving pixel data for VRAM write (CPU→VRAM)
    VramWrite,
    /// Receiving variable-length vertex list for a polyline command
    Polyline,
}

/// Display area horizontal resolution
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
enum HRes {
    H256,
    H320,
    H368,
    H512,
    H640,
}

impl HRes {
    fn pixels(&self) -> u32 {
        match self {
            HRes::H256 => 256,
            HRes::H320 => 320,
            HRes::H368 => 368,
            HRes::H512 => 512,
            HRes::H640 => 640,
        }
    }
}

/// Display area vertical resolution
#[derive(Debug, Clone, Copy)]
enum VRes {
    V240,
    V480,
}

impl VRes {
    fn pixels(&self) -> u32 {
        match self {
            VRes::V240 => 240,
            VRes::V480 => 480,
        }
    }
}

/// Semi-transparency mode
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
enum SemiTransparency {
    /// B/2 + F/2
    Average,
    /// B + F
    Add,
    /// B - F
    Sub,
    /// B + F/4
    AddQuarter,
}

/// Texture depth
#[derive(Debug, Clone, Copy)]
#[allow(dead_code, clippy::enum_variant_names)]
enum TextureDepth {
    T4Bit,
    T8Bit,
    T15Bit,
}

/// DMA direction
#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(dead_code)]
enum DmaDirection {
    Off,
    Fifo,
    CpuToGp0,
    GpuReadToCpu,
}

/// PS1 GPU
pub struct Gpu {
    /// Video RAM (1024×512 × 16-bit pixels)
    vram: Vec<u16>,

    /// Current output frame buffer (RGB8)
    frame: Frame,

    // ========================================================================
    // GP1 display control state
    // ========================================================================
    /// Display area start X in VRAM
    display_vram_x: u32,
    /// Display area start Y in VRAM
    display_vram_y: u32,
    /// Horizontal display range (start, end)
    display_horiz_start: u32,
    display_horiz_end: u32,
    /// Vertical display range (start, end)
    display_vert_start: u32,
    display_vert_end: u32,
    /// Horizontal resolution
    hres: HRes,
    /// Vertical resolution
    vres: VRes,
    /// Video mode (false=NTSC, true=PAL)
    is_pal: bool,
    /// Color depth (false=15bit, true=24bit)
    display_24bit: bool,
    /// Interlace enable
    interlace: bool,
    /// Display enable (false=display on, true=display off / blanked)
    display_disabled: bool,
    /// DMA direction setting
    dma_direction: DmaDirection,

    // ========================================================================
    // GP0 rendering state
    // ========================================================================
    /// Drawing area top-left X
    draw_area_left: u32,
    /// Drawing area top-left Y
    draw_area_top: u32,
    /// Drawing area bottom-right X
    draw_area_right: u32,
    /// Drawing area bottom-right Y
    draw_area_bottom: u32,
    /// Drawing offset X
    draw_offset_x: i32,
    /// Drawing offset Y
    draw_offset_y: i32,
    /// Texture window mask X
    tex_window_mask_x: u8,
    /// Texture window mask Y
    tex_window_mask_y: u8,
    /// Texture window offset X
    tex_window_offset_x: u8,
    /// Texture window offset Y
    tex_window_offset_y: u8,
    /// Set mask bit when drawing
    set_mask_bit: bool,
    /// Check mask bit (don't draw to masked pixels)
    check_mask_bit: bool,
    /// Texture page X base (in 64-halfword steps, 0-15)
    texpage_x: u32,
    /// Texture page Y base (0 or 256)
    texpage_y: u32,
    /// Semi-transparency mode
    semi_transparency: SemiTransparency,
    /// Texture depth
    tex_depth: TextureDepth,
    /// Dithering enable
    dithering: bool,
    /// Draw to display area
    draw_to_display: bool,
    /// Texture disable
    texture_disable: bool,

    // ========================================================================
    // Command FIFO / state machine
    // ========================================================================
    /// GP0 processing mode
    gp0_mode: Gp0Mode,
    /// GP0 command buffer
    gp0_buffer: Vec<u32>,
    /// Number of words remaining for current command
    gp0_words_remaining: u32,
    /// Current GP0 command byte (for parameter phase)
    gp0_command: u8,
    /// True when current polyline is Gouraud-shaded (alternating color/vertex words)
    polyline_shaded: bool,
    /// Pending color word for shaded polyline (waiting for the matching vertex)
    polyline_pending_color: Option<u32>,
    /// Semi-transparency flag for the current primitive (bit 25 of command word)
    prim_semi_transparent: bool,

    // VRAM write transfer state (CPU → VRAM)
    vram_transfer_x: u32,
    vram_transfer_y: u32,
    vram_transfer_w: u32,
    vram_transfer_h: u32,
    vram_transfer_cx: u32,
    vram_transfer_cy: u32,

    // VRAM read transfer state (VRAM → CPU)
    vram_read_x: u32,
    vram_read_y: u32,
    vram_read_w: u32,
    vram_read_h: u32,
    vram_read_cx: Cell<u32>,
    vram_read_cy: Cell<u32>,
    /// True when a VRAM→CPU read transfer is in progress
    vram_read_active: Cell<bool>,

    /// GPUREAD latch (Cell for interior mutability during DMA/CPU reads)
    gpu_read_latch: Cell<u32>,

    /// Interrupt request flag
    pub irq: bool,

    /// Current scanline
    scanline: u32,
    /// Dot clock within scanline (for timing)
    #[allow(dead_code)]
    dot_clock: u32,
    /// In VBlank region
    pub in_vblank: bool,
}

impl Default for Gpu {
    fn default() -> Self {
        Self::new()
    }
}

impl Gpu {
    pub fn new() -> Self {
        Self {
            vram: vec![0; VRAM_WIDTH * VRAM_HEIGHT],
            frame: Frame::new(320, 240),
            display_vram_x: 0,
            display_vram_y: 0,
            display_horiz_start: 0x200,
            display_horiz_end: 0xC00,
            display_vert_start: 0x10,
            display_vert_end: 0x100,
            hres: HRes::H320,
            vres: VRes::V240,
            is_pal: false,
            display_24bit: false,
            interlace: false,
            display_disabled: true,
            dma_direction: DmaDirection::Off,
            draw_area_left: 0,
            draw_area_top: 0,
            draw_area_right: 0,
            draw_area_bottom: 0,
            draw_offset_x: 0,
            draw_offset_y: 0,
            tex_window_mask_x: 0,
            tex_window_mask_y: 0,
            tex_window_offset_x: 0,
            tex_window_offset_y: 0,
            set_mask_bit: false,
            check_mask_bit: false,
            texpage_x: 0,
            texpage_y: 0,
            semi_transparency: SemiTransparency::Average,
            tex_depth: TextureDepth::T4Bit,
            dithering: false,
            draw_to_display: false,
            texture_disable: false,
            gp0_mode: Gp0Mode::Command,
            gp0_buffer: Vec::with_capacity(16),
            gp0_words_remaining: 0,
            gp0_command: 0,
            polyline_shaded: false,
            polyline_pending_color: None,
            prim_semi_transparent: false,
            vram_transfer_x: 0,
            vram_transfer_y: 0,
            vram_transfer_w: 0,
            vram_transfer_h: 0,
            vram_transfer_cx: 0,
            vram_transfer_cy: 0,
            vram_read_x: 0,
            vram_read_y: 0,
            vram_read_w: 0,
            vram_read_h: 0,
            vram_read_cx: Cell::new(0),
            vram_read_cy: Cell::new(0),
            vram_read_active: Cell::new(false),
            gpu_read_latch: Cell::new(0),
            irq: false,
            scanline: 0,
            dot_clock: 0,
            in_vblank: false,
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }

    // ========================================================================
    // GPUSTAT (read at 0x1F801814)
    // ========================================================================

    /// Build the GPUSTAT register value.
    pub fn gpustat(&self) -> u32 {
        let mut stat = 0u32;

        // Bits 0-3: Texture page X base
        stat |= self.texpage_x & 0xF;
        // Bit 4: Texture page Y base (0 or 1)
        stat |= (self.texpage_y >> 8) << 4;
        // Bits 5-6: Semi-transparency mode
        stat |= match self.semi_transparency {
            SemiTransparency::Average => 0,
            SemiTransparency::Add => 1,
            SemiTransparency::Sub => 2,
            SemiTransparency::AddQuarter => 3,
        } << 5;
        // Bits 7-8: Texture depth
        stat |= match self.tex_depth {
            TextureDepth::T4Bit => 0,
            TextureDepth::T8Bit => 1,
            TextureDepth::T15Bit => 2,
        } << 7;
        // Bit 9: Dithering
        if self.dithering {
            stat |= 1 << 9;
        }
        // Bit 10: Draw to display area
        if self.draw_to_display {
            stat |= 1 << 10;
        }
        // Bit 11: Set mask bit
        if self.set_mask_bit {
            stat |= 1 << 11;
        }
        // Bit 12: Check mask bit
        if self.check_mask_bit {
            stat |= 1 << 12;
        }
        // Bit 13: Interlace field (not implemented)
        // Bit 14: Reverse flag (not standard, always 0)
        // Bit 15: Texture disable
        if self.texture_disable {
            stat |= 1 << 15;
        }
        // Bit 16: Horizontal resolution 2 (368 mode)
        if matches!(self.hres, HRes::H368) {
            stat |= 1 << 16;
        }
        // Bits 17-18: Horizontal resolution 1
        stat |= match self.hres {
            HRes::H256 => 0,
            HRes::H320 | HRes::H368 => 1,
            HRes::H512 => 2,
            HRes::H640 => 3,
        } << 17;
        // Bit 19: Vertical resolution
        if matches!(self.vres, VRes::V480) {
            stat |= 1 << 19;
        }
        // Bit 20: Video mode (0=NTSC, 1=PAL)
        if self.is_pal {
            stat |= 1 << 20;
        }
        // Bit 21: Display area color depth (0=15bit, 1=24bit)
        if self.display_24bit {
            stat |= 1 << 21;
        }
        // Bit 22: Vertical interlace
        if self.interlace {
            stat |= 1 << 22;
        }
        // Bit 23: Display enable (0=on, 1=off)
        if self.display_disabled {
            stat |= 1 << 23;
        }
        // Bit 24: IRQ1 flag
        if self.irq {
            stat |= 1 << 24;
        }
        // Bit 25: DMA / Data Request
        // When DMA direction is set, indicates readiness
        stat |= 1 << 25; // Always ready for simplicity

        // Bit 26: Ready to receive command
        if self.gp0_mode == Gp0Mode::Command {
            stat |= 1 << 26;
        }
        // Bit 27: Ready to send VRAM to CPU
        stat |= 1 << 27;
        // Bit 28: Ready to receive DMA block
        stat |= 1 << 28;

        // Bits 29-30: DMA direction
        stat |= match self.dma_direction {
            DmaDirection::Off => 0,
            DmaDirection::Fifo => 1,
            DmaDirection::CpuToGp0 => 2,
            DmaDirection::GpuReadToCpu => 3,
        } << 29;

        // Bit 31: Drawing even/odd line in interlace mode
        // (toggled each frame)
        if self.scanline % 2 == 1 {
            stat |= 1 << 31;
        }

        stat
    }

    // ========================================================================
    // GP0 — Rendering commands
    // ========================================================================

    /// Write a word to GP0 (command/data port).
    pub fn gp0_write(&mut self, val: u32) {
        match self.gp0_mode {
            Gp0Mode::Command => {
                let cmd = (val >> 24) as u8;
                let nparams = gp0_command_length(cmd);
                if nparams == 1 {
                    // Single-word command — execute immediately
                    self.gp0_buffer.clear();
                    self.gp0_buffer.push(val);
                    self.execute_gp0(cmd);
                } else {
                    // Multi-word command — collect parameters
                    self.gp0_command = cmd;
                    self.gp0_buffer.clear();
                    self.gp0_buffer.push(val);
                    self.gp0_words_remaining = nparams - 1;
                    self.gp0_mode = Gp0Mode::Params;
                }
            }
            Gp0Mode::Params => {
                self.gp0_buffer.push(val);
                self.gp0_words_remaining -= 1;
                if self.gp0_words_remaining == 0 {
                    self.gp0_mode = Gp0Mode::Command;
                    let cmd = self.gp0_command;
                    // Check if this is a polyline command that continues with more vertices
                    let is_polyline = matches!(cmd, 0x48..=0x4B | 0x58..=0x5B);
                    self.execute_gp0(cmd);
                    // After the minimum number of words, polylines continue in Polyline mode
                    if is_polyline {
                        self.polyline_shaded = matches!(cmd, 0x58..=0x5B);
                        self.gp0_mode = Gp0Mode::Polyline;
                    }
                }
            }
            Gp0Mode::VramWrite => {
                // Write two pixels per word (16-bit each)
                let p0 = val as u16;
                let p1 = (val >> 16) as u16;
                self.write_vram_transfer_pixel(p0);
                self.write_vram_transfer_pixel(p1);
            }
            Gp0Mode::Polyline => {
                // Polyline termination check: word matches 0x5???5??? pattern
                // (bits 12-15 == 0x5 AND bits 28-31 == 0x5)
                if val & 0xF000_F000 == 0x5000_5000 {
                    self.gp0_mode = Gp0Mode::Command;
                    self.polyline_pending_color = None;
                    return;
                }
                if self.polyline_shaded {
                    // Shaded polyline: alternating color and vertex words
                    match self.polyline_pending_color.take() {
                        None => {
                            // This word is a color; save it and wait for vertex
                            self.polyline_pending_color = Some(val);
                        }
                        Some(color_word) => {
                            // This word is the vertex; draw line from previous vertex
                            let n = self.gp0_buffer.len();
                            if n >= 2 {
                                // Previous vertex is the last word in the buffer
                                let v_prev = self.decode_vertex(self.gp0_buffer[n - 1]);
                                let v_next = self.decode_vertex(val);
                                let (r, g, b) = Self::decode_color(color_word);
                                self.draw_line(v_prev, v_next, r, g, b);
                            }
                            self.gp0_buffer.push(val); // Store current vertex
                        }
                    }
                } else {
                    // Mono polyline: just vertex words
                    let n = self.gp0_buffer.len();
                    if n >= 2 {
                        let (r, g, b) = Self::decode_color(self.gp0_buffer[0]);
                        let v_prev = self.decode_vertex(self.gp0_buffer[n - 1]);
                        let v_next = self.decode_vertex(val);
                        self.draw_line(v_prev, v_next, r, g, b);
                    }
                    self.gp0_buffer.push(val);
                }
            }
        }
    }

    fn write_vram_transfer_pixel(&mut self, pixel: u16) {
        let x = (self.vram_transfer_x + self.vram_transfer_cx) & 0x3FF;
        let y = (self.vram_transfer_y + self.vram_transfer_cy) & 0x1FF;

        let idx = (y as usize) * VRAM_WIDTH + (x as usize);
        if idx < self.vram.len() {
            self.vram[idx] = pixel;
        }

        self.vram_transfer_cx += 1;
        if self.vram_transfer_cx >= self.vram_transfer_w {
            self.vram_transfer_cx = 0;
            self.vram_transfer_cy += 1;
            if self.vram_transfer_cy >= self.vram_transfer_h {
                self.gp0_mode = Gp0Mode::Command;
            }
        }
    }

    fn execute_gp0(&mut self, cmd: u8) {
        // Extract semi-transparency flag from command word (bit 25)
        let cmd_word = self.gp0_buffer[0];
        self.prim_semi_transparent = cmd_word & (1 << 25) != 0;

        match cmd {
            0x00 => {} // NOP
            0x01 => {
                // Clear cache
            }
            0x02 => self.gp0_fill_rect(),
            0x20..=0x23 => self.gp0_mono_triangle(),
            0x24..=0x27 => self.gp0_textured_triangle(),
            0x28..=0x2B => self.gp0_mono_quad(),
            0x2C..=0x2F => self.gp0_textured_quad(),
            0x30..=0x33 => self.gp0_shaded_triangle(),
            0x34..=0x37 => self.gp0_shaded_textured_triangle(),
            0x38..=0x3B => self.gp0_shaded_quad(),
            0x3C..=0x3F => self.gp0_shaded_textured_quad(),
            0x40..=0x43 => self.gp0_mono_line(),
            0x48..=0x4B => self.gp0_mono_polyline(),
            0x50..=0x53 => self.gp0_shaded_line(),
            0x58..=0x5B => self.gp0_shaded_polyline(),
            0x60..=0x63 => self.gp0_mono_rect_variable(),
            0x64..=0x67 => self.gp0_textured_rect_variable(),
            0x68..=0x6B => self.gp0_mono_rect_1x1(),
            0x70..=0x73 => self.gp0_mono_rect_8x8(),
            0x74..=0x77 => self.gp0_textured_rect_8x8(),
            0x78..=0x7B => self.gp0_mono_rect_16x16(),
            0x7C..=0x7F => self.gp0_textured_rect_16x16(),
            0x80..=0x9F => self.gp0_vram_to_vram(),
            0xA0..=0xBF => self.gp0_cpu_to_vram(),
            0xC0..=0xDF => self.gp0_vram_to_cpu(),
            0xE1 => self.gp0_draw_mode(),
            0xE2 => self.gp0_texture_window(),
            0xE3 => self.gp0_draw_area_top_left(),
            0xE4 => self.gp0_draw_area_bottom_right(),
            0xE5 => self.gp0_draw_offset(),
            0xE6 => self.gp0_mask_bit(),
            _ => {
                // Unknown GP0 command — ignore
            }
        }
    }

    // ========================================================================
    // GP0 environment commands
    // ========================================================================

    fn gp0_draw_mode(&mut self) {
        let val = self.gp0_buffer[0];
        self.texpage_x = val & 0xF;
        self.texpage_y = ((val >> 4) & 1) * 256;
        self.semi_transparency = match (val >> 5) & 3 {
            0 => SemiTransparency::Average,
            1 => SemiTransparency::Add,
            2 => SemiTransparency::Sub,
            3 => SemiTransparency::AddQuarter,
            _ => unreachable!(),
        };
        self.tex_depth = match (val >> 7) & 3 {
            0 => TextureDepth::T4Bit,
            1 => TextureDepth::T8Bit,
            2 | 3 => TextureDepth::T15Bit,
            _ => unreachable!(),
        };
        self.dithering = val & (1 << 9) != 0;
        self.draw_to_display = val & (1 << 10) != 0;
        self.texture_disable = val & (1 << 11) != 0;
    }

    fn gp0_texture_window(&mut self) {
        let val = self.gp0_buffer[0];
        self.tex_window_mask_x = (val & 0x1F) as u8;
        self.tex_window_mask_y = ((val >> 5) & 0x1F) as u8;
        self.tex_window_offset_x = ((val >> 10) & 0x1F) as u8;
        self.tex_window_offset_y = ((val >> 15) & 0x1F) as u8;
    }

    fn gp0_draw_area_top_left(&mut self) {
        let val = self.gp0_buffer[0];
        self.draw_area_left = val & 0x3FF;
        self.draw_area_top = (val >> 10) & 0x1FF;
    }

    fn gp0_draw_area_bottom_right(&mut self) {
        let val = self.gp0_buffer[0];
        self.draw_area_right = val & 0x3FF;
        self.draw_area_bottom = (val >> 10) & 0x1FF;
    }

    fn gp0_draw_offset(&mut self) {
        let val = self.gp0_buffer[0];
        let x = (val & 0x7FF) as u16;
        let y = ((val >> 11) & 0x7FF) as u16;
        // Sign-extend 11-bit values
        self.draw_offset_x = ((x << 5) as i16 >> 5) as i32;
        self.draw_offset_y = ((y << 5) as i16 >> 5) as i32;
    }

    fn gp0_mask_bit(&mut self) {
        let val = self.gp0_buffer[0];
        self.set_mask_bit = val & 1 != 0;
        self.check_mask_bit = val & 2 != 0;
    }

    // ========================================================================
    // GP0 rendering commands
    // ========================================================================

    /// Decode a vertex from a GP0 word: X (bits 0-10), Y (bits 16-26)
    fn decode_vertex(&self, word: u32) -> (i32, i32) {
        let x = (word & 0x7FF) as i16 as i32;
        let y = ((word >> 16) & 0x7FF) as i16 as i32;
        (x + self.draw_offset_x, y + self.draw_offset_y)
    }

    /// Decode a color from a GP0 word: R (bits 0-7), G (bits 8-15), B (bits 16-23)
    fn decode_color(word: u32) -> (u8, u8, u8) {
        let r = (word & 0xFF) as u8;
        let g = ((word >> 8) & 0xFF) as u8;
        let b = ((word >> 16) & 0xFF) as u8;
        (r, g, b)
    }

    /// Set a pixel in VRAM (with draw area clipping, mask bit, and semi-transparency).
    fn set_pixel(&mut self, x: i32, y: i32, r: u8, g: u8, b: u8) {
        self.set_pixel_ex(x, y, r, g, b, self.prim_semi_transparent);
    }

    /// Set a pixel with explicit semi-transparency control.
    fn set_pixel_ex(&mut self, x: i32, y: i32, r: u8, g: u8, b: u8, semi_transparent: bool) {
        if x < self.draw_area_left as i32
            || x > self.draw_area_right as i32
            || y < self.draw_area_top as i32
            || y > self.draw_area_bottom as i32
        {
            return;
        }
        let x = x as u32 & 0x3FF;
        let y = y as u32 & 0x1FF;
        let idx = (y as usize) * VRAM_WIDTH + (x as usize);

        if self.check_mask_bit && (self.vram[idx] & 0x8000 != 0) {
            return;
        }

        let src_pixel = rgb_to_15bit(r, g, b);
        let mut pixel = if semi_transparent {
            let blend_mode = match self.semi_transparency {
                SemiTransparency::Average => 0,
                SemiTransparency::Add => 1,
                SemiTransparency::Sub => 2,
                SemiTransparency::AddQuarter => 3,
            };
            blend_semi_transparent(blend_mode, src_pixel, self.vram[idx])
        } else {
            src_pixel
        };

        if self.set_mask_bit {
            pixel |= 0x8000;
        }
        self.vram[idx] = pixel;
    }

    // ========================================================================
    // Flat-shaded triangles (monochrome)
    // ========================================================================

    fn gp0_mono_triangle(&mut self) {
        let (r, g, b) = Self::decode_color(self.gp0_buffer[0]);
        let v0 = self.decode_vertex(self.gp0_buffer[1]);
        let v1 = self.decode_vertex(self.gp0_buffer[2]);
        let v2 = self.decode_vertex(self.gp0_buffer[3]);
        self.draw_flat_triangle(v0, v1, v2, r, g, b);
    }

    fn gp0_mono_quad(&mut self) {
        let (r, g, b) = Self::decode_color(self.gp0_buffer[0]);
        let v0 = self.decode_vertex(self.gp0_buffer[1]);
        let v1 = self.decode_vertex(self.gp0_buffer[2]);
        let v2 = self.decode_vertex(self.gp0_buffer[3]);
        let v3 = self.decode_vertex(self.gp0_buffer[4]);
        self.draw_flat_triangle(v0, v1, v2, r, g, b);
        self.draw_flat_triangle(v1, v2, v3, r, g, b);
    }

    fn draw_flat_triangle(
        &mut self,
        v0: (i32, i32),
        v1: (i32, i32),
        v2: (i32, i32),
        r: u8,
        g: u8,
        b: u8,
    ) {
        // Simple scanline rasterizer
        let mut verts = [v0, v1, v2];
        // Sort by Y
        if verts[0].1 > verts[1].1 {
            verts.swap(0, 1);
        }
        if verts[0].1 > verts[2].1 {
            verts.swap(0, 2);
        }
        if verts[1].1 > verts[2].1 {
            verts.swap(1, 2);
        }

        let [top, mid, bot] = verts;
        let total_height = bot.1 - top.1;
        if total_height == 0 {
            return;
        }

        for y in top.1..=bot.1 {
            let second_half = y >= mid.1;
            let seg_height = if second_half {
                bot.1 - mid.1
            } else {
                mid.1 - top.1
            };
            if seg_height == 0 {
                continue;
            }

            let alpha = (y - top.1) as f32 / total_height as f32;
            let beta = if second_half {
                (y - mid.1) as f32 / seg_height as f32
            } else {
                (y - top.1) as f32 / seg_height as f32
            };

            let mut xa = top.0 as f32 + (bot.0 - top.0) as f32 * alpha;
            let mut xb = if second_half {
                mid.0 as f32 + (bot.0 - mid.0) as f32 * beta
            } else {
                top.0 as f32 + (mid.0 - top.0) as f32 * beta
            };

            if xa > xb {
                std::mem::swap(&mut xa, &mut xb);
            }

            for x in (xa as i32)..=(xb as i32) {
                self.set_pixel(x, y, r, g, b);
            }
        }
    }

    // ========================================================================
    // Gouraud-shaded triangles
    // ========================================================================

    fn gp0_shaded_triangle(&mut self) {
        let (r0, g0, b0) = Self::decode_color(self.gp0_buffer[0]);
        let v0 = self.decode_vertex(self.gp0_buffer[1]);
        let (r1, g1, b1) = Self::decode_color(self.gp0_buffer[2]);
        let v1 = self.decode_vertex(self.gp0_buffer[3]);
        let (r2, g2, b2) = Self::decode_color(self.gp0_buffer[4]);
        let v2 = self.decode_vertex(self.gp0_buffer[5]);
        self.draw_shaded_triangle(v0, (r0, g0, b0), v1, (r1, g1, b1), v2, (r2, g2, b2));
    }

    fn gp0_shaded_quad(&mut self) {
        let (r0, g0, b0) = Self::decode_color(self.gp0_buffer[0]);
        let v0 = self.decode_vertex(self.gp0_buffer[1]);
        let (r1, g1, b1) = Self::decode_color(self.gp0_buffer[2]);
        let v1 = self.decode_vertex(self.gp0_buffer[3]);
        let (r2, g2, b2) = Self::decode_color(self.gp0_buffer[4]);
        let v2 = self.decode_vertex(self.gp0_buffer[5]);
        let (r3, g3, b3) = Self::decode_color(self.gp0_buffer[6]);
        let v3 = self.decode_vertex(self.gp0_buffer[7]);
        self.draw_shaded_triangle(v0, (r0, g0, b0), v1, (r1, g1, b1), v2, (r2, g2, b2));
        self.draw_shaded_triangle(v1, (r1, g1, b1), v2, (r2, g2, b2), v3, (r3, g3, b3));
    }

    fn draw_shaded_triangle(
        &mut self,
        v0: (i32, i32),
        c0: (u8, u8, u8),
        v1: (i32, i32),
        c1: (u8, u8, u8),
        v2: (i32, i32),
        c2: (u8, u8, u8),
    ) {
        // Barycentric rasterizer for Gouraud shading
        let min_x = v0.0.min(v1.0).min(v2.0).max(self.draw_area_left as i32);
        let max_x = v0.0.max(v1.0).max(v2.0).min(self.draw_area_right as i32);
        let min_y = v0.1.min(v1.1).min(v2.1).max(self.draw_area_top as i32);
        let max_y = v0.1.max(v1.1).max(v2.1).min(self.draw_area_bottom as i32);

        let area = cross(v0, v1, v2);
        if area == 0 {
            return;
        }
        let inv_area = 1.0 / area as f32;

        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let p = (x, y);
                let w0 = cross(v1, v2, p);
                let w1 = cross(v2, v0, p);
                let w2 = cross(v0, v1, p);

                // Check sign consistency (all same sign or zero)
                let all_pos = w0 >= 0 && w1 >= 0 && w2 >= 0;
                let all_neg = w0 <= 0 && w1 <= 0 && w2 <= 0;
                if !all_pos && !all_neg {
                    continue;
                }

                let b0 = w0 as f32 * inv_area;
                let b1 = w1 as f32 * inv_area;
                let b2 = w2 as f32 * inv_area;

                let r = (c0.0 as f32 * b0 + c1.0 as f32 * b1 + c2.0 as f32 * b2) as u8;
                let g = (c0.1 as f32 * b0 + c1.1 as f32 * b1 + c2.1 as f32 * b2) as u8;
                let b = (c0.2 as f32 * b0 + c1.2 as f32 * b1 + c2.2 as f32 * b2) as u8;

                self.set_pixel(x, y, r, g, b);
            }
        }
    }

    // ========================================================================
    // Texture sampling
    // ========================================================================

    /// Decode texture coordinates from a GP0 word.
    /// Bits 0-7: U, Bits 8-15: V
    fn decode_texcoord(word: u32) -> (u8, u8) {
        let u = (word & 0xFF) as u8;
        let v = ((word >> 8) & 0xFF) as u8;
        (u, v)
    }

    /// Decode CLUT position from the texcoord word.
    /// The CLUT info is in bits 16-31 of the texcoord word.
    /// Bits 0-5: X/16 (CLUT X in VRAM, 16-pixel steps)
    /// Bits 6-14: Y (CLUT Y in VRAM)
    fn decode_clut(word: u32) -> (u32, u32) {
        let clut = word >> 16;
        let clut_x = (clut & 0x3F) * 16;
        let clut_y = (clut >> 6) & 0x1FF;
        (clut_x, clut_y)
    }

    /// Decode texpage info from a texcoord word (bits 16-31).
    /// Returns (texpage_x, texpage_y, tex_depth).
    fn decode_texpage(word: u32) -> (u32, u32, TextureDepth) {
        let page = word >> 16;
        let tx = (page & 0xF) * 64;
        let ty = ((page >> 4) & 1) * 256;
        let depth = match (page >> 7) & 3 {
            0 => TextureDepth::T4Bit,
            1 => TextureDepth::T8Bit,
            _ => TextureDepth::T15Bit,
        };
        (tx, ty, depth)
    }

    /// Apply texture window to coordinates.
    fn apply_tex_window(&self, u: u8, v: u8) -> (u8, u8) {
        let u_out = (u & !(self.tex_window_mask_x * 8))
            | ((self.tex_window_offset_x & self.tex_window_mask_x) * 8);
        let v_out = (v & !(self.tex_window_mask_y * 8))
            | ((self.tex_window_offset_y & self.tex_window_mask_y) * 8);
        (u_out, v_out)
    }

    /// Sample a texel from VRAM given texture coordinates, page, depth, and CLUT.
    /// Returns (r, g, b, transparent) where transparent means the pixel should be skipped.
    #[allow(clippy::too_many_arguments)]
    fn sample_texture(
        &self,
        u: u8,
        v: u8,
        texpage_x: u32,
        texpage_y: u32,
        depth: TextureDepth,
        clut_x: u32,
        clut_y: u32,
    ) -> (u8, u8, u8, bool) {
        match depth {
            TextureDepth::T4Bit => {
                // 4-bit indexed: 4 texels per VRAM pixel
                let texel_x = texpage_x + (u as u32 / 4);
                let texel_y = texpage_y + v as u32;
                let vx = (texel_x & 0x3FF) as usize;
                let vy = (texel_y & 0x1FF) as usize;
                let vram_pixel = self.vram[vy * VRAM_WIDTH + vx];
                // Extract 4-bit index based on position within the 16-bit word
                let shift = (u & 3) * 4;
                let index = ((vram_pixel >> shift) & 0xF) as u32;
                // Look up in CLUT
                let cx = ((clut_x + index) & 0x3FF) as usize;
                let cy = (clut_y & 0x1FF) as usize;
                let color = self.vram[cy * VRAM_WIDTH + cx];
                if color == 0 {
                    return (0, 0, 0, true); // Fully transparent
                }
                let r = ((color & 0x1F) << 3) as u8;
                let g = (((color >> 5) & 0x1F) << 3) as u8;
                let b = (((color >> 10) & 0x1F) << 3) as u8;
                (r, g, b, false)
            }
            TextureDepth::T8Bit => {
                // 8-bit indexed: 2 texels per VRAM pixel
                let texel_x = texpage_x + (u as u32 / 2);
                let texel_y = texpage_y + v as u32;
                let vx = (texel_x & 0x3FF) as usize;
                let vy = (texel_y & 0x1FF) as usize;
                let vram_pixel = self.vram[vy * VRAM_WIDTH + vx];
                // Extract 8-bit index
                let shift = (u & 1) * 8;
                let index = ((vram_pixel >> shift) & 0xFF) as u32;
                // Look up in CLUT
                let cx = ((clut_x + index) & 0x3FF) as usize;
                let cy = (clut_y & 0x1FF) as usize;
                let color = self.vram[cy * VRAM_WIDTH + cx];
                if color == 0 {
                    return (0, 0, 0, true);
                }
                let r = ((color & 0x1F) << 3) as u8;
                let g = (((color >> 5) & 0x1F) << 3) as u8;
                let b = (((color >> 10) & 0x1F) << 3) as u8;
                (r, g, b, false)
            }
            TextureDepth::T15Bit => {
                // Direct 15-bit color
                let texel_x = texpage_x + u as u32;
                let texel_y = texpage_y + v as u32;
                let vx = (texel_x & 0x3FF) as usize;
                let vy = (texel_y & 0x1FF) as usize;
                let color = self.vram[vy * VRAM_WIDTH + vx];
                if color == 0 {
                    return (0, 0, 0, true);
                }
                let r = ((color & 0x1F) << 3) as u8;
                let g = (((color >> 5) & 0x1F) << 3) as u8;
                let b = (((color >> 10) & 0x1F) << 3) as u8;
                (r, g, b, false)
            }
        }
    }

    /// Modulate a texture color by the primitive color (for texture blending).
    /// PS1 formula: result = (tex_color * prim_color) / 128, clamped to 255.
    fn modulate_color(tex: (u8, u8, u8), prim: (u8, u8, u8)) -> (u8, u8, u8) {
        let r = ((tex.0 as u32 * prim.0 as u32) >> 7).min(255) as u8;
        let g = ((tex.1 as u32 * prim.1 as u32) >> 7).min(255) as u8;
        let b = ((tex.2 as u32 * prim.2 as u32) >> 7).min(255) as u8;
        (r, g, b)
    }

    // ========================================================================
    // Textured primitives
    // ========================================================================

    fn gp0_textured_triangle(&mut self) {
        // Word layout: color+cmd, v0, tc0+CLUT, v1, tc1+texpage, v2, tc2
        let (pr, pg, pb) = Self::decode_color(self.gp0_buffer[0]);
        let raw_tex = (self.gp0_buffer[0] >> 24) & 1 != 0;
        let v0 = self.decode_vertex(self.gp0_buffer[1]);
        let (u0, v0t) = Self::decode_texcoord(self.gp0_buffer[2]);
        let (clut_x, clut_y) = Self::decode_clut(self.gp0_buffer[2]);
        let v1 = self.decode_vertex(self.gp0_buffer[3]);
        let (u1, v1t) = Self::decode_texcoord(self.gp0_buffer[4]);
        let (tp_x, tp_y, tp_depth) = Self::decode_texpage(self.gp0_buffer[4]);
        let v2 = self.decode_vertex(self.gp0_buffer[5]);
        let (u2, v2t) = Self::decode_texcoord(self.gp0_buffer[6]);

        if self.texture_disable {
            self.draw_flat_triangle(v0, v1, v2, pr, pg, pb);
            return;
        }

        self.draw_textured_triangle(
            v0,
            (u0, v0t),
            v1,
            (u1, v1t),
            v2,
            (u2, v2t),
            (pr, pg, pb),
            raw_tex,
            tp_x,
            tp_y,
            tp_depth,
            clut_x,
            clut_y,
        );
    }

    fn gp0_textured_quad(&mut self) {
        // 9 words: color+cmd, v0, tc0+clut, v1, tc1+texpage, v2, tc2, v3, tc3
        let (pr, pg, pb) = Self::decode_color(self.gp0_buffer[0]);
        let raw_tex = (self.gp0_buffer[0] >> 24) & 1 != 0;
        let v0 = self.decode_vertex(self.gp0_buffer[1]);
        let (u0, v0t) = Self::decode_texcoord(self.gp0_buffer[2]);
        let (clut_x, clut_y) = Self::decode_clut(self.gp0_buffer[2]);
        let v1 = self.decode_vertex(self.gp0_buffer[3]);
        let (u1, v1t) = Self::decode_texcoord(self.gp0_buffer[4]);
        let (tp_x, tp_y, tp_depth) = Self::decode_texpage(self.gp0_buffer[4]);
        let v2 = self.decode_vertex(self.gp0_buffer[5]);
        let (u2, v2t) = Self::decode_texcoord(self.gp0_buffer[6]);
        let v3 = self.decode_vertex(self.gp0_buffer[7]);
        let (u3, v3t) = Self::decode_texcoord(self.gp0_buffer[8]);

        if self.texture_disable {
            self.draw_flat_triangle(v0, v1, v2, pr, pg, pb);
            self.draw_flat_triangle(v1, v2, v3, pr, pg, pb);
            return;
        }

        self.draw_textured_triangle(
            v0,
            (u0, v0t),
            v1,
            (u1, v1t),
            v2,
            (u2, v2t),
            (pr, pg, pb),
            raw_tex,
            tp_x,
            tp_y,
            tp_depth,
            clut_x,
            clut_y,
        );
        self.draw_textured_triangle(
            v1,
            (u1, v1t),
            v2,
            (u2, v2t),
            v3,
            (u3, v3t),
            (pr, pg, pb),
            raw_tex,
            tp_x,
            tp_y,
            tp_depth,
            clut_x,
            clut_y,
        );
    }

    fn gp0_shaded_textured_triangle(&mut self) {
        // 9 words: c0+cmd, v0, tc0+clut, c1, v1, tc1+texpage, c2, v2, tc2
        let (r0, g0, b0) = Self::decode_color(self.gp0_buffer[0]);
        let raw_tex = (self.gp0_buffer[0] >> 24) & 1 != 0;
        let v0 = self.decode_vertex(self.gp0_buffer[1]);
        let (u0, v0t) = Self::decode_texcoord(self.gp0_buffer[2]);
        let (clut_x, clut_y) = Self::decode_clut(self.gp0_buffer[2]);
        let (r1, g1, b1) = Self::decode_color(self.gp0_buffer[3]);
        let v1 = self.decode_vertex(self.gp0_buffer[4]);
        let (u1, v1t) = Self::decode_texcoord(self.gp0_buffer[5]);
        let (tp_x, tp_y, tp_depth) = Self::decode_texpage(self.gp0_buffer[5]);
        let (r2, g2, b2) = Self::decode_color(self.gp0_buffer[6]);
        let v2 = self.decode_vertex(self.gp0_buffer[7]);
        let (u2, v2t) = Self::decode_texcoord(self.gp0_buffer[8]);

        if self.texture_disable {
            self.draw_shaded_triangle(v0, (r0, g0, b0), v1, (r1, g1, b1), v2, (r2, g2, b2));
            return;
        }

        self.draw_shaded_textured_triangle(
            v0,
            (u0, v0t),
            (r0, g0, b0),
            v1,
            (u1, v1t),
            (r1, g1, b1),
            v2,
            (u2, v2t),
            (r2, g2, b2),
            raw_tex,
            tp_x,
            tp_y,
            tp_depth,
            clut_x,
            clut_y,
        );
    }

    fn gp0_shaded_textured_quad(&mut self) {
        // 12 words: c0+cmd, v0, tc0+clut, c1, v1, tc1+texpage, c2, v2, tc2, c3, v3, tc3
        let (r0, g0, b0) = Self::decode_color(self.gp0_buffer[0]);
        let raw_tex = (self.gp0_buffer[0] >> 24) & 1 != 0;
        let v0 = self.decode_vertex(self.gp0_buffer[1]);
        let (u0, v0t) = Self::decode_texcoord(self.gp0_buffer[2]);
        let (clut_x, clut_y) = Self::decode_clut(self.gp0_buffer[2]);
        let (r1, g1, b1) = Self::decode_color(self.gp0_buffer[3]);
        let v1 = self.decode_vertex(self.gp0_buffer[4]);
        let (u1, v1t) = Self::decode_texcoord(self.gp0_buffer[5]);
        let (tp_x, tp_y, tp_depth) = Self::decode_texpage(self.gp0_buffer[5]);
        let (r2, g2, b2) = Self::decode_color(self.gp0_buffer[6]);
        let v2 = self.decode_vertex(self.gp0_buffer[7]);
        let (u2, v2t) = Self::decode_texcoord(self.gp0_buffer[8]);
        let (r3, g3, b3) = Self::decode_color(self.gp0_buffer[9]);
        let v3 = self.decode_vertex(self.gp0_buffer[10]);
        let (u3, v3t) = Self::decode_texcoord(self.gp0_buffer[11]);

        if self.texture_disable {
            self.draw_shaded_triangle(v0, (r0, g0, b0), v1, (r1, g1, b1), v2, (r2, g2, b2));
            self.draw_shaded_triangle(v1, (r1, g1, b1), v2, (r2, g2, b2), v3, (r3, g3, b3));
            return;
        }

        self.draw_shaded_textured_triangle(
            v0,
            (u0, v0t),
            (r0, g0, b0),
            v1,
            (u1, v1t),
            (r1, g1, b1),
            v2,
            (u2, v2t),
            (r2, g2, b2),
            raw_tex,
            tp_x,
            tp_y,
            tp_depth,
            clut_x,
            clut_y,
        );
        self.draw_shaded_textured_triangle(
            v1,
            (u1, v1t),
            (r1, g1, b1),
            v2,
            (u2, v2t),
            (r2, g2, b2),
            v3,
            (u3, v3t),
            (r3, g3, b3),
            raw_tex,
            tp_x,
            tp_y,
            tp_depth,
            clut_x,
            clut_y,
        );
    }

    /// Draw a textured triangle using barycentric rasterization.
    #[allow(clippy::too_many_arguments)]
    fn draw_textured_triangle(
        &mut self,
        v0: (i32, i32),
        uv0: (u8, u8),
        v1: (i32, i32),
        uv1: (u8, u8),
        v2: (i32, i32),
        uv2: (u8, u8),
        prim_color: (u8, u8, u8),
        raw_texture: bool,
        texpage_x: u32,
        texpage_y: u32,
        tex_depth: TextureDepth,
        clut_x: u32,
        clut_y: u32,
    ) {
        let min_x = v0.0.min(v1.0).min(v2.0).max(self.draw_area_left as i32);
        let max_x = v0.0.max(v1.0).max(v2.0).min(self.draw_area_right as i32);
        let min_y = v0.1.min(v1.1).min(v2.1).max(self.draw_area_top as i32);
        let max_y = v0.1.max(v1.1).max(v2.1).min(self.draw_area_bottom as i32);

        let area = cross(v0, v1, v2);
        if area == 0 {
            return;
        }
        let inv_area = 1.0 / area as f32;

        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let p = (x, y);
                let w0 = cross(v1, v2, p);
                let w1 = cross(v2, v0, p);
                let w2 = cross(v0, v1, p);

                let all_pos = w0 >= 0 && w1 >= 0 && w2 >= 0;
                let all_neg = w0 <= 0 && w1 <= 0 && w2 <= 0;
                if !all_pos && !all_neg {
                    continue;
                }

                let b0 = w0 as f32 * inv_area;
                let b1 = w1 as f32 * inv_area;
                let b2 = w2 as f32 * inv_area;

                // Interpolate texture coordinates
                let u = (uv0.0 as f32 * b0 + uv1.0 as f32 * b1 + uv2.0 as f32 * b2) as u8;
                let v = (uv0.1 as f32 * b0 + uv1.1 as f32 * b1 + uv2.1 as f32 * b2) as u8;
                let (u, v) = self.apply_tex_window(u, v);

                let (tr, tg, tb, transparent) =
                    self.sample_texture(u, v, texpage_x, texpage_y, tex_depth, clut_x, clut_y);
                if transparent {
                    continue;
                }

                let (fr, fg, fb) = if raw_texture {
                    (tr, tg, tb)
                } else {
                    Self::modulate_color((tr, tg, tb), prim_color)
                };

                self.set_pixel(x, y, fr, fg, fb);
            }
        }
    }

    /// Draw a Gouraud-shaded + textured triangle using barycentric rasterization.
    #[allow(clippy::too_many_arguments)]
    fn draw_shaded_textured_triangle(
        &mut self,
        v0: (i32, i32),
        uv0: (u8, u8),
        c0: (u8, u8, u8),
        v1: (i32, i32),
        uv1: (u8, u8),
        c1: (u8, u8, u8),
        v2: (i32, i32),
        uv2: (u8, u8),
        c2: (u8, u8, u8),
        raw_texture: bool,
        texpage_x: u32,
        texpage_y: u32,
        tex_depth: TextureDepth,
        clut_x: u32,
        clut_y: u32,
    ) {
        let min_x = v0.0.min(v1.0).min(v2.0).max(self.draw_area_left as i32);
        let max_x = v0.0.max(v1.0).max(v2.0).min(self.draw_area_right as i32);
        let min_y = v0.1.min(v1.1).min(v2.1).max(self.draw_area_top as i32);
        let max_y = v0.1.max(v1.1).max(v2.1).min(self.draw_area_bottom as i32);

        let area = cross(v0, v1, v2);
        if area == 0 {
            return;
        }
        let inv_area = 1.0 / area as f32;

        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let p = (x, y);
                let w0 = cross(v1, v2, p);
                let w1 = cross(v2, v0, p);
                let w2 = cross(v0, v1, p);

                let all_pos = w0 >= 0 && w1 >= 0 && w2 >= 0;
                let all_neg = w0 <= 0 && w1 <= 0 && w2 <= 0;
                if !all_pos && !all_neg {
                    continue;
                }

                let b0 = w0 as f32 * inv_area;
                let b1 = w1 as f32 * inv_area;
                let b2 = w2 as f32 * inv_area;

                // Interpolate texture coordinates
                let u = (uv0.0 as f32 * b0 + uv1.0 as f32 * b1 + uv2.0 as f32 * b2) as u8;
                let v = (uv0.1 as f32 * b0 + uv1.1 as f32 * b1 + uv2.1 as f32 * b2) as u8;
                let (u, v) = self.apply_tex_window(u, v);

                let (tr, tg, tb, transparent) =
                    self.sample_texture(u, v, texpage_x, texpage_y, tex_depth, clut_x, clut_y);
                if transparent {
                    continue;
                }

                let (fr, fg, fb) = if raw_texture {
                    (tr, tg, tb)
                } else {
                    // Interpolate Gouraud color
                    let gr = (c0.0 as f32 * b0 + c1.0 as f32 * b1 + c2.0 as f32 * b2) as u8;
                    let gg = (c0.1 as f32 * b0 + c1.1 as f32 * b1 + c2.1 as f32 * b2) as u8;
                    let gb = (c0.2 as f32 * b0 + c1.2 as f32 * b1 + c2.2 as f32 * b2) as u8;
                    Self::modulate_color((tr, tg, tb), (gr, gg, gb))
                };

                self.set_pixel(x, y, fr, fg, fb);
            }
        }
    }

    // ========================================================================
    // Lines
    // ========================================================================

    fn gp0_mono_line(&mut self) {
        let (r, g, b) = Self::decode_color(self.gp0_buffer[0]);
        let v0 = self.decode_vertex(self.gp0_buffer[1]);
        let v1 = self.decode_vertex(self.gp0_buffer[2]);
        self.draw_line(v0, v1, r, g, b);
    }

    fn gp0_mono_polyline(&mut self) {
        // Draw the first segment (cmd+color, v0, v1)
        // Subsequent vertices are handled in Gp0Mode::Polyline via gp0_write
        if self.gp0_buffer.len() >= 3 {
            let (r, g, b) = Self::decode_color(self.gp0_buffer[0]);
            let v0 = self.decode_vertex(self.gp0_buffer[1]);
            let v1 = self.decode_vertex(self.gp0_buffer[2]);
            self.draw_line(v0, v1, r, g, b);
        }
    }

    fn gp0_shaded_line(&mut self) {
        // Two-vertex shaded line: [c0+cmd, v0, c1, v1]
        if self.gp0_buffer.len() >= 4 {
            let (r, g, b) = Self::decode_color(self.gp0_buffer[0]);
            let v0 = self.decode_vertex(self.gp0_buffer[1]);
            let v1 = self.decode_vertex(self.gp0_buffer[3]);
            // Use the start color for the whole line (simplified; full Gouraud for lines is rare)
            self.draw_line(v0, v1, r, g, b);
        }
    }

    fn gp0_shaded_polyline(&mut self) {
        // Draw the first segment (cmd+c0, v0, c1, v1)
        // Subsequent color/vertex pairs are handled in Gp0Mode::Polyline via gp0_write
        if self.gp0_buffer.len() >= 4 {
            let (r, g, b) = Self::decode_color(self.gp0_buffer[2]);
            let v0 = self.decode_vertex(self.gp0_buffer[1]);
            let v1 = self.decode_vertex(self.gp0_buffer[3]);
            self.draw_line(v0, v1, r, g, b);
        }
    }

    fn draw_line(&mut self, v0: (i32, i32), v1: (i32, i32), r: u8, g: u8, b: u8) {
        // Bresenham's line algorithm
        let (mut x0, mut y0) = v0;
        let (x1, y1) = v1;
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;

        loop {
            self.set_pixel(x0, y0, r, g, b);
            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
        }
    }

    // ========================================================================
    // Rectangles
    // ========================================================================

    fn gp0_fill_rect(&mut self) {
        let (r, g, b) = Self::decode_color(self.gp0_buffer[0]);
        let x = self.gp0_buffer[1] & 0x3F0; // Rounded to 16-pixel boundary
        let y = (self.gp0_buffer[1] >> 16) & 0x1FF;
        let w = ((self.gp0_buffer[2] & 0x3FF) + 0xF) & !0xF; // Rounded up to 16
        let h = (self.gp0_buffer[2] >> 16) & 0x1FF;

        let pixel = rgb_to_15bit(r, g, b);
        for dy in 0..h {
            for dx in 0..w {
                let px = (x + dx) & 0x3FF;
                let py = (y + dy) & 0x1FF;
                let idx = (py as usize) * VRAM_WIDTH + (px as usize);
                if idx < self.vram.len() {
                    self.vram[idx] = pixel;
                }
            }
        }
    }

    fn gp0_mono_rect_variable(&mut self) {
        let (r, g, b) = Self::decode_color(self.gp0_buffer[0]);
        let (x, y) = self.decode_vertex(self.gp0_buffer[1]);
        let w = (self.gp0_buffer[2] & 0xFFFF) as i32;
        let h = (self.gp0_buffer[2] >> 16) as i32;
        for dy in 0..h {
            for dx in 0..w {
                self.set_pixel(x + dx, y + dy, r, g, b);
            }
        }
    }

    fn gp0_textured_rect_variable(&mut self) {
        // 4 words: color+cmd, vertex, texcoord+clut, size
        let (pr, pg, pb) = Self::decode_color(self.gp0_buffer[0]);
        let raw_tex = (self.gp0_buffer[0] >> 24) & 1 != 0;
        let (x, y) = self.decode_vertex(self.gp0_buffer[1]);
        let (u_base, v_base) = Self::decode_texcoord(self.gp0_buffer[2]);
        let (clut_x, clut_y) = Self::decode_clut(self.gp0_buffer[2]);
        let w = (self.gp0_buffer[3] & 0xFFFF) as i32;
        let h = (self.gp0_buffer[3] >> 16) as i32;

        if self.texture_disable {
            for dy in 0..h {
                for dx in 0..w {
                    self.set_pixel(x + dx, y + dy, pr, pg, pb);
                }
            }
            return;
        }

        let tp_x = self.texpage_x * 64;
        let tp_y = self.texpage_y;
        let tp_depth = self.tex_depth;

        for dy in 0..h {
            for dx in 0..w {
                let u = u_base.wrapping_add(dx as u8);
                let v = v_base.wrapping_add(dy as u8);
                let (u, v) = self.apply_tex_window(u, v);
                let (tr, tg, tb, transparent) =
                    self.sample_texture(u, v, tp_x, tp_y, tp_depth, clut_x, clut_y);
                if transparent {
                    continue;
                }
                let (fr, fg, fb) = if raw_tex {
                    (tr, tg, tb)
                } else {
                    Self::modulate_color((tr, tg, tb), (pr, pg, pb))
                };
                self.set_pixel(x + dx, y + dy, fr, fg, fb);
            }
        }
    }

    fn gp0_mono_rect_1x1(&mut self) {
        let (r, g, b) = Self::decode_color(self.gp0_buffer[0]);
        let (x, y) = self.decode_vertex(self.gp0_buffer[1]);
        self.set_pixel(x, y, r, g, b);
    }

    fn gp0_mono_rect_8x8(&mut self) {
        let (r, g, b) = Self::decode_color(self.gp0_buffer[0]);
        let (x, y) = self.decode_vertex(self.gp0_buffer[1]);
        for dy in 0..8 {
            for dx in 0..8 {
                self.set_pixel(x + dx, y + dy, r, g, b);
            }
        }
    }

    fn gp0_textured_rect_8x8(&mut self) {
        // 3 words: color+cmd, vertex, texcoord+clut
        let (pr, pg, pb) = Self::decode_color(self.gp0_buffer[0]);
        let raw_tex = (self.gp0_buffer[0] >> 24) & 1 != 0;
        let (x, y) = self.decode_vertex(self.gp0_buffer[1]);
        let (u_base, v_base) = Self::decode_texcoord(self.gp0_buffer[2]);
        let (clut_x, clut_y) = Self::decode_clut(self.gp0_buffer[2]);

        if self.texture_disable {
            for dy in 0..8 {
                for dx in 0..8 {
                    self.set_pixel(x + dx, y + dy, pr, pg, pb);
                }
            }
            return;
        }

        let tp_x = self.texpage_x * 64;
        let tp_y = self.texpage_y;
        let tp_depth = self.tex_depth;

        for dy in 0..8_i32 {
            for dx in 0..8_i32 {
                let u = u_base.wrapping_add(dx as u8);
                let v = v_base.wrapping_add(dy as u8);
                let (u, v) = self.apply_tex_window(u, v);
                let (tr, tg, tb, transparent) =
                    self.sample_texture(u, v, tp_x, tp_y, tp_depth, clut_x, clut_y);
                if transparent {
                    continue;
                }
                let (fr, fg, fb) = if raw_tex {
                    (tr, tg, tb)
                } else {
                    Self::modulate_color((tr, tg, tb), (pr, pg, pb))
                };
                self.set_pixel(x + dx, y + dy, fr, fg, fb);
            }
        }
    }

    fn gp0_mono_rect_16x16(&mut self) {
        let (r, g, b) = Self::decode_color(self.gp0_buffer[0]);
        let (x, y) = self.decode_vertex(self.gp0_buffer[1]);
        for dy in 0..16 {
            for dx in 0..16 {
                self.set_pixel(x + dx, y + dy, r, g, b);
            }
        }
    }

    fn gp0_textured_rect_16x16(&mut self) {
        // 3 words: color+cmd, vertex, texcoord+clut
        let (pr, pg, pb) = Self::decode_color(self.gp0_buffer[0]);
        let raw_tex = (self.gp0_buffer[0] >> 24) & 1 != 0;
        let (x, y) = self.decode_vertex(self.gp0_buffer[1]);
        let (u_base, v_base) = Self::decode_texcoord(self.gp0_buffer[2]);
        let (clut_x, clut_y) = Self::decode_clut(self.gp0_buffer[2]);

        if self.texture_disable {
            for dy in 0..16 {
                for dx in 0..16 {
                    self.set_pixel(x + dx, y + dy, pr, pg, pb);
                }
            }
            return;
        }

        let tp_x = self.texpage_x * 64;
        let tp_y = self.texpage_y;
        let tp_depth = self.tex_depth;

        for dy in 0..16_i32 {
            for dx in 0..16_i32 {
                let u = u_base.wrapping_add(dx as u8);
                let v = v_base.wrapping_add(dy as u8);
                let (u, v) = self.apply_tex_window(u, v);
                let (tr, tg, tb, transparent) =
                    self.sample_texture(u, v, tp_x, tp_y, tp_depth, clut_x, clut_y);
                if transparent {
                    continue;
                }
                let (fr, fg, fb) = if raw_tex {
                    (tr, tg, tb)
                } else {
                    Self::modulate_color((tr, tg, tb), (pr, pg, pb))
                };
                self.set_pixel(x + dx, y + dy, fr, fg, fb);
            }
        }
    }

    // ========================================================================
    // VRAM transfers
    // ========================================================================

    fn gp0_vram_to_vram(&mut self) {
        let src_x = self.gp0_buffer[1] & 0x3FF;
        let src_y = (self.gp0_buffer[1] >> 16) & 0x1FF;
        let dst_x = self.gp0_buffer[2] & 0x3FF;
        let dst_y = (self.gp0_buffer[2] >> 16) & 0x1FF;
        let w = ((self.gp0_buffer[3] & 0xFFFF).wrapping_sub(1) & 0x3FF) + 1;
        let h = (((self.gp0_buffer[3] >> 16).wrapping_sub(1)) & 0x1FF) + 1;

        for dy in 0..h {
            for dx in 0..w {
                let sx = (src_x + dx) & 0x3FF;
                let sy = (src_y + dy) & 0x1FF;
                let dx2 = (dst_x + dx) & 0x3FF;
                let dy2 = (dst_y + dy) & 0x1FF;
                let src_idx = (sy as usize) * VRAM_WIDTH + (sx as usize);
                let dst_idx = (dy2 as usize) * VRAM_WIDTH + (dx2 as usize);
                self.vram[dst_idx] = self.vram[src_idx];
            }
        }
    }

    fn gp0_cpu_to_vram(&mut self) {
        // Set up transfer parameters and switch to VramWrite mode
        self.vram_transfer_x = self.gp0_buffer[1] & 0x3FF;
        self.vram_transfer_y = (self.gp0_buffer[1] >> 16) & 0x1FF;
        self.vram_transfer_w = ((self.gp0_buffer[2] & 0xFFFF).wrapping_sub(1) & 0x3FF) + 1;
        self.vram_transfer_h = (((self.gp0_buffer[2] >> 16).wrapping_sub(1)) & 0x1FF) + 1;
        self.vram_transfer_cx = 0;
        self.vram_transfer_cy = 0;
        self.gp0_mode = Gp0Mode::VramWrite;
    }

    fn gp0_vram_to_cpu(&mut self) {
        // Set up VRAM→CPU read transfer
        self.vram_read_x = self.gp0_buffer[1] & 0x3FF;
        self.vram_read_y = (self.gp0_buffer[1] >> 16) & 0x1FF;
        self.vram_read_w = ((self.gp0_buffer[2] & 0xFFFF).wrapping_sub(1) & 0x3FF) + 1;
        self.vram_read_h = (((self.gp0_buffer[2] >> 16).wrapping_sub(1)) & 0x1FF) + 1;
        self.vram_read_cx.set(0);
        self.vram_read_cy.set(0);
        self.vram_read_active.set(true);
        // Pre-load the first word into the latch
        self.advance_vram_read_latch();
    }

    /// Advance the VRAM read latch by one 32-bit word (2 pixels).
    fn advance_vram_read_latch(&self) {
        if !self.vram_read_active.get() {
            return;
        }
        let p0 = self.read_vram_pixel_for_transfer();
        let p1 = self.read_vram_pixel_for_transfer();
        self.gpu_read_latch.set((p0 as u32) | ((p1 as u32) << 16));
    }

    /// Read and advance one pixel from the VRAM read transfer.
    fn read_vram_pixel_for_transfer(&self) -> u16 {
        if !self.vram_read_active.get() {
            return 0;
        }
        let cx = self.vram_read_cx.get();
        let cy = self.vram_read_cy.get();
        let vx = (self.vram_read_x + cx) & 0x3FF;
        let vy = (self.vram_read_y + cy) & 0x1FF;
        let idx = (vy as usize) * VRAM_WIDTH + (vx as usize);
        let pixel = if idx < self.vram.len() {
            self.vram[idx]
        } else {
            0
        };

        let next_cx = cx + 1;
        if next_cx >= self.vram_read_w {
            self.vram_read_cx.set(0);
            let next_cy = cy + 1;
            self.vram_read_cy.set(next_cy);
            if next_cy >= self.vram_read_h {
                self.vram_read_active.set(false);
            }
        } else {
            self.vram_read_cx.set(next_cx);
        }
        pixel
    }

    // ========================================================================
    // GP1 — Display control commands
    // ========================================================================

    /// Write a word to GP1 (display control port).
    pub fn gp1_write(&mut self, val: u32) {
        let cmd = (val >> 24) & 0x3F;
        match cmd {
            0x00 => self.gp1_reset(),
            0x01 => {
                // Reset command buffer
                self.gp0_mode = Gp0Mode::Command;
                self.gp0_buffer.clear();
                self.gp0_words_remaining = 0;
            }
            0x02 => {
                // Acknowledge IRQ1
                self.irq = false;
            }
            0x03 => {
                // Display enable
                self.display_disabled = val & 1 != 0;
            }
            0x04 => {
                // DMA direction
                self.dma_direction = match val & 3 {
                    0 => DmaDirection::Off,
                    1 => DmaDirection::Fifo,
                    2 => DmaDirection::CpuToGp0,
                    3 => DmaDirection::GpuReadToCpu,
                    _ => unreachable!(),
                };
            }
            0x05 => {
                // Display area start
                self.display_vram_x = val & 0x3FE; // Halfword aligned
                self.display_vram_y = (val >> 10) & 0x1FF;
            }
            0x06 => {
                // Horizontal display range
                self.display_horiz_start = val & 0xFFF;
                self.display_horiz_end = (val >> 12) & 0xFFF;
            }
            0x07 => {
                // Vertical display range
                self.display_vert_start = val & 0x3FF;
                self.display_vert_end = (val >> 10) & 0x3FF;
            }
            0x08 => {
                // Display mode
                let hr1 = val & 3;
                let hr2 = (val >> 6) & 1;
                self.hres = if hr2 != 0 {
                    HRes::H368
                } else {
                    match hr1 {
                        0 => HRes::H256,
                        1 => HRes::H320,
                        2 => HRes::H512,
                        3 => HRes::H640,
                        _ => unreachable!(),
                    }
                };
                self.vres = if val & (1 << 2) != 0 {
                    VRes::V480
                } else {
                    VRes::V240
                };
                self.is_pal = val & (1 << 3) != 0;
                self.display_24bit = val & (1 << 4) != 0;
                self.interlace = val & (1 << 5) != 0;
            }
            0x10..=0x1F => {
                // Get GPU info
                match val & 0xF {
                    3 => self
                        .gpu_read_latch
                        .set(self.draw_area_left | (self.draw_area_top << 10)),
                    4 => self
                        .gpu_read_latch
                        .set(self.draw_area_right | (self.draw_area_bottom << 10)),
                    5 => self.gpu_read_latch.set(
                        (self.draw_offset_x as u32 & 0x7FF)
                            | ((self.draw_offset_y as u32 & 0x7FF) << 11),
                    ),
                    7 => self.gpu_read_latch.set(2), // GPU version
                    _ => {}
                }
            }
            _ => {} // Unknown GP1 command
        }
    }

    fn gp1_reset(&mut self) {
        self.irq = false;
        self.display_disabled = true;
        self.dma_direction = DmaDirection::Off;
        self.display_vram_x = 0;
        self.display_vram_y = 0;
        self.display_horiz_start = 0x200;
        self.display_horiz_end = 0xC00;
        self.display_vert_start = 0x10;
        self.display_vert_end = 0x100;
        self.hres = HRes::H320;
        self.vres = VRes::V240;
        self.gp0_mode = Gp0Mode::Command;
        self.gp0_buffer.clear();
        self.gp0_words_remaining = 0;
        self.polyline_shaded = false;
        self.polyline_pending_color = None;
        self.prim_semi_transparent = false;
        self.draw_area_left = 0;
        self.draw_area_top = 0;
        self.draw_area_right = 0;
        self.draw_area_bottom = 0;
        self.draw_offset_x = 0;
        self.draw_offset_y = 0;
        self.tex_window_mask_x = 0;
        self.tex_window_mask_y = 0;
        self.tex_window_offset_x = 0;
        self.tex_window_offset_y = 0;
        self.set_mask_bit = false;
        self.check_mask_bit = false;
        self.texpage_x = 0;
        self.texpage_y = 0;
        self.semi_transparency = SemiTransparency::Average;
        self.tex_depth = TextureDepth::T4Bit;
        self.dithering = false;
        self.draw_to_display = false;
        self.texture_disable = false;
    }

    /// Read GP0 (GPUREAD — VRAM data or GPU info).
    pub fn gpuread(&self) -> u32 {
        let latch = self.gpu_read_latch.get();
        if self.vram_read_active.get() {
            // Advance by one word (2 pixels) for the next DMA/CPU read
            self.advance_vram_read_latch();
        }
        latch
    }

    // ========================================================================
    // Frame output
    // ========================================================================

    /// Build the output frame from VRAM display area.
    pub fn render_frame(&mut self) -> &Frame {
        let w = self.hres.pixels();
        let h = self.vres.pixels();

        if self.frame.width != w || self.frame.height != h {
            self.frame = Frame::new(w, h);
        }

        if self.display_disabled {
            // Black screen
            for pixel in self.frame.pixels.iter_mut() {
                *pixel = 0xFF00_0000; // ARGB black
            }
            return &self.frame;
        }

        let vram_x = self.display_vram_x as usize;
        let vram_y = self.display_vram_y as usize;

        for y in 0..h as usize {
            for x in 0..w as usize {
                let argb = if self.display_24bit {
                    // 24-bit mode: 3 consecutive bytes per pixel from VRAM (stored as 16-bit words)
                    // Every 3 bytes = 2 words: RG | B_ (little-endian)
                    // Pixel at screen X maps to byte offset = (vram_x*2 + x*3) in VRAM byte stream
                    let byte_offset = (vram_x * 2 + x * 3) & (VRAM_WIDTH * 2 - 1);
                    let vram_byte_y = (vram_y + y) & (VRAM_HEIGHT - 1);
                    // Each VRAM word = 2 bytes; word index = byte_offset >> 1
                    let word_idx0 = byte_offset >> 1;
                    let word_idx1 = (byte_offset + 1) >> 1;
                    let word_idx2 = (byte_offset + 2) >> 1;
                    let w0 = self.vram[vram_byte_y * VRAM_WIDTH + (word_idx0 & (VRAM_WIDTH - 1))];
                    let w1 = self.vram[vram_byte_y * VRAM_WIDTH + (word_idx1 & (VRAM_WIDTH - 1))];
                    let w2 = self.vram[vram_byte_y * VRAM_WIDTH + (word_idx2 & (VRAM_WIDTH - 1))];
                    // Even byte_offset = low byte of word, odd = high byte
                    let r = if byte_offset & 1 == 0 {
                        w0 as u8
                    } else {
                        (w0 >> 8) as u8
                    };
                    let g = if (byte_offset + 1) & 1 == 0 {
                        w1 as u8
                    } else {
                        (w1 >> 8) as u8
                    };
                    let b = if (byte_offset + 2) & 1 == 0 {
                        w2 as u8
                    } else {
                        (w2 >> 8) as u8
                    };
                    0xFF00_0000 | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
                } else {
                    let vx = (vram_x + x) & (VRAM_WIDTH - 1);
                    let vy = (vram_y + y) & (VRAM_HEIGHT - 1);
                    pixel15_to_argb(self.vram[vy * VRAM_WIDTH + vx])
                };

                self.frame.pixels[y * w as usize + x] = argb;
            }
        }

        &self.frame
    }

    /// Advance GPU timing by one scanline.
    pub fn step_scanline(&mut self) {
        self.scanline += 1;
        let total_lines = if self.is_pal { 314 } else { 263 };
        let vblank_start = if self.is_pal { 288 } else { 240 };

        if self.scanline >= total_lines {
            self.scanline = 0;
            self.in_vblank = false;
        }

        if self.scanline == vblank_start {
            self.in_vblank = true;
        }
    }
}

impl Renderer for Gpu {
    fn get_frame(&self) -> &Frame {
        &self.frame
    }

    fn get_frame_mut(&mut self) -> &mut Frame {
        &mut self.frame
    }

    fn clear(&mut self, _color: u32) {
        for pixel in self.vram.iter_mut() {
            *pixel = 0;
        }
    }

    fn reset(&mut self) {
        self.reset();
    }

    fn name(&self) -> &str {
        "PS1 GPU"
    }

    fn resize(&mut self, _width: u32, _height: u32) {
        // GPU resolution is controlled by GP1 commands
    }
}

// ============================================================================
// Helper functions
// ============================================================================

/// Convert RGB888 to PS1 15-bit pixel (XBBBBBGGGGGRRRRR).
fn rgb_to_15bit(r: u8, g: u8, b: u8) -> u16 {
    let r5 = (r >> 3) as u16;
    let g5 = (g >> 3) as u16;
    let b5 = (b >> 3) as u16;
    r5 | (g5 << 5) | (b5 << 10)
}

/// Convert PS1 15-bit pixel to ARGB8888.
fn pixel15_to_argb(pixel: u16) -> u32 {
    let r = ((pixel & 0x1F) << 3) as u32;
    let g = (((pixel >> 5) & 0x1F) << 3) as u32;
    let b = (((pixel >> 10) & 0x1F) << 3) as u32;
    0xFF00_0000 | (r << 16) | (g << 8) | b
}

/// Cross product for triangle rasterization.
fn cross(a: (i32, i32), b: (i32, i32), c: (i32, i32)) -> i32 {
    (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)
}

/// Apply PS1 semi-transparency blending to a pixel.
/// mode: 0=B/2+F/2, 1=B+F, 2=B-F, 3=B+F/4
/// src: foreground (new pixel) color as 15-bit
/// dst: background (existing VRAM) color as 15-bit
fn blend_semi_transparent(mode: u8, src: u16, dst: u16) -> u16 {
    let sr = (src & 0x1F) as i32;
    let sg = ((src >> 5) & 0x1F) as i32;
    let sb = ((src >> 10) & 0x1F) as i32;
    let dr = (dst & 0x1F) as i32;
    let dg = ((dst >> 5) & 0x1F) as i32;
    let db = ((dst >> 10) & 0x1F) as i32;

    let (r, g, b) = match mode {
        0 => (
            ((dr + sr) / 2).min(31),
            ((dg + sg) / 2).min(31),
            ((db + sb) / 2).min(31),
        ),
        1 => ((dr + sr).min(31), (dg + sg).min(31), (db + sb).min(31)),
        2 => ((dr - sr).max(0), (dg - sg).max(0), (db - sb).max(0)),
        3 => (
            (dr + sr / 4).min(31),
            (dg + sg / 4).min(31),
            (db + sb / 4).min(31),
        ),
        _ => (sr, sg, sb),
    };
    (r as u16) | ((g as u16) << 5) | ((b as u16) << 10)
}

/// Get the number of 32-bit words for a GP0 command.
fn gp0_command_length(cmd: u8) -> u32 {
    match cmd {
        0x00 => 1,         // NOP
        0x01 => 1,         // Clear cache
        0x02 => 3,         // Fill rect
        0x20..=0x23 => 4,  // Monochrome triangle
        0x24..=0x27 => 7,  // Textured triangle
        0x28..=0x2B => 5,  // Monochrome quad
        0x2C..=0x2F => 9,  // Textured quad
        0x30..=0x33 => 6,  // Shaded triangle
        0x34..=0x37 => 9,  // Shaded textured triangle
        0x38..=0x3B => 8,  // Shaded quad
        0x3C..=0x3F => 12, // Shaded textured quad
        0x40..=0x43 => 3,  // Monochrome line
        0x48..=0x4B => 3,  // Monochrome polyline (minimum)
        0x50..=0x53 => 4,  // Shaded line
        0x58..=0x5B => 4,  // Shaded polyline (minimum)
        0x60..=0x63 => 3,  // Variable-size mono rect
        0x64..=0x67 => 4,  // Variable-size textured rect
        0x68..=0x6B => 2,  // 1x1 mono rect
        0x70..=0x73 => 2,  // 8x8 mono rect
        0x74..=0x77 => 3,  // 8x8 textured rect
        0x78..=0x7B => 2,  // 16x16 mono rect
        0x7C..=0x7F => 3,  // 16x16 textured rect
        0x80..=0x9F => 4,  // VRAM-to-VRAM copy
        0xA0..=0xBF => 3,  // CPU-to-VRAM (header, then pixel data follows)
        0xC0..=0xDF => 3,  // VRAM-to-CPU
        0xE1..=0xE6 => 1,  // Environment commands
        _ => 1,            // Unknown, consume 1 word
    }
}

// ============================================================================
// Inspector data for debugging GUI
// ============================================================================

/// GPU inspector data exported for the debug UI
pub struct Ps1GpuInspectorData {
    pub gpustat: u32,
    pub display_vram_x: u32,
    pub display_vram_y: u32,
    pub display_horiz_start: u32,
    pub display_horiz_end: u32,
    pub display_vert_start: u32,
    pub display_vert_end: u32,
    pub hres_str: String,
    pub vres_str: String,
    pub is_pal: bool,
    pub display_24bit: bool,
    pub interlace: bool,
    pub display_disabled: bool,
    pub draw_area_left: u32,
    pub draw_area_top: u32,
    pub draw_area_right: u32,
    pub draw_area_bottom: u32,
    pub draw_offset_x: i32,
    pub draw_offset_y: i32,
    pub texpage_x: u32,
    pub texpage_y: u32,
    pub tex_depth_str: String,
    pub semi_transparency_str: String,
    pub dithering: bool,
    pub set_mask_bit: bool,
    pub check_mask_bit: bool,
    pub tex_window_mask_x: u8,
    pub tex_window_mask_y: u8,
    pub tex_window_offset_x: u8,
    pub tex_window_offset_y: u8,
    pub scanline: u32,
    pub in_vblank: bool,
    pub irq: bool,
}

impl Gpu {
    /// Get inspector data for the debug UI
    pub fn get_inspector_data(&self) -> Ps1GpuInspectorData {
        Ps1GpuInspectorData {
            gpustat: self.gpustat(),
            display_vram_x: self.display_vram_x,
            display_vram_y: self.display_vram_y,
            display_horiz_start: self.display_horiz_start,
            display_horiz_end: self.display_horiz_end,
            display_vert_start: self.display_vert_start,
            display_vert_end: self.display_vert_end,
            hres_str: format!("{:?}", self.hres),
            vres_str: format!("{:?}", self.vres),
            is_pal: self.is_pal,
            display_24bit: self.display_24bit,
            interlace: self.interlace,
            display_disabled: self.display_disabled,
            draw_area_left: self.draw_area_left,
            draw_area_top: self.draw_area_top,
            draw_area_right: self.draw_area_right,
            draw_area_bottom: self.draw_area_bottom,
            draw_offset_x: self.draw_offset_x,
            draw_offset_y: self.draw_offset_y,
            texpage_x: self.texpage_x,
            texpage_y: self.texpage_y,
            tex_depth_str: format!("{:?}", self.tex_depth),
            semi_transparency_str: format!("{:?}", self.semi_transparency),
            dithering: self.dithering,
            set_mask_bit: self.set_mask_bit,
            check_mask_bit: self.check_mask_bit,
            tex_window_mask_x: self.tex_window_mask_x,
            tex_window_mask_y: self.tex_window_mask_y,
            tex_window_offset_x: self.tex_window_offset_x,
            tex_window_offset_y: self.tex_window_offset_y,
            scanline: self.scanline,
            in_vblank: self.in_vblank,
            irq: self.irq,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_gpu() -> Gpu {
        let mut gpu = Gpu::new();
        // Set full draw area
        gpu.draw_area_left = 0;
        gpu.draw_area_right = 1023;
        gpu.draw_area_top = 0;
        gpu.draw_area_bottom = 511;
        gpu
    }

    #[test]
    fn test_gpu_vram_to_cpu_transfer() {
        let mut gpu = make_gpu();

        // Write known pixel values to VRAM at (10, 5)
        let vram_x = 10usize;
        let vram_y = 5usize;
        gpu.vram[vram_y * VRAM_WIDTH + vram_x] = 0x1234;
        gpu.vram[vram_y * VRAM_WIDTH + vram_x + 1] = 0x5678;

        // Set up VRAM→CPU transfer via GP0 0xC0 command
        gpu.gp0_write(0xC000_0000);
        gpu.gp0_write((vram_y as u32) << 16 | vram_x as u32);
        gpu.gp0_write(0x0001_0002); // h=1, w=2

        // Read back the VRAM data via GPUREAD
        let word0 = gpu.gpuread();
        // Should be 0x5678_1234 (two pixels packed LE)
        assert_eq!(
            word0, 0x5678_1234,
            "VRAM→CPU transfer should return packed pixels"
        );
    }

    #[test]
    fn test_gpu_vram_to_cpu_single_pixel() {
        let mut gpu = make_gpu();

        // Write a single known pixel
        gpu.vram[0] = 0xABCD;
        // Set up a 1×1 transfer from (0, 0)
        gpu.gp0_write(0xC000_0000);
        gpu.gp0_write(0x0000_0000); // position (0, 0)
        gpu.gp0_write(0x0001_0001); // h=1, w=1

        let word = gpu.gpuread();
        // Low 16 bits should be the pixel; high 16 bits undefined (second read advances)
        assert_eq!(
            word & 0xFFFF,
            0xABCD,
            "single pixel VRAM read should match written value"
        );
    }

    #[test]
    fn test_gpu_semi_transparency_add() {
        let mut gpu = make_gpu();

        // Set background: pure blue (B=31 in 15-bit → 0x7C00)
        gpu.vram[0] = 0x7C00;

        // Draw with Add semi-transparency: src = pure red (R=31 in 15-bit → 0x001F)
        gpu.semi_transparency = SemiTransparency::Add;
        gpu.prim_semi_transparent = true;
        gpu.set_pixel(0, 0, 0xF8, 0, 0); // R=248 → 5-bit R=31

        // Expected: bg(0x7C00) + src(0x001F) = 0x7C1F (blue + red = purple)
        assert_eq!(gpu.vram[0], 0x7C1F, "Add blend should sum channels");
    }

    #[test]
    fn test_gpu_semi_transparency_average() {
        let mut gpu = make_gpu();

        // Background: full white (0x7FFF)
        gpu.vram[100] = 0x7FFF;

        // Draw with Average mode: src = full white (0x7FFF)
        gpu.semi_transparency = SemiTransparency::Average;
        gpu.prim_semi_transparent = true;
        gpu.set_pixel(100, 0, 0xF8, 0xF8, 0xF8); // near-white (R=G=B=31 after 5-bit)

        // Average(0x7FFF, 0x7FFF) = 0x7FFF (all channels stay at max)
        assert_eq!(
            gpu.vram[100], 0x7FFF,
            "average blend of two full-whites should stay white"
        );
    }

    #[test]
    fn test_gpu_polyline_draws_multiple_segments() {
        let mut gpu = make_gpu();

        // Mono polyline: draw two segments (0,0)→(10,0)→(20,0) then terminate
        gpu.gp0_write(0x4800_FF00); // cmd=0x48, G=255 → green polyline
        gpu.gp0_write(0x0000_0000); // v0 = (0, 0)
        gpu.gp0_write(0x0000_000A); // v1 = (10, 0) — first segment drawn here
                                    // Now in Polyline mode
        gpu.gp0_write(0x0000_0014); // v2 = (20, 0) — second segment
        gpu.gp0_write(0x5000_5000); // terminator

        // Pixel at (0,0) should have been written (start of first segment)
        let p0 = gpu.vram[0];
        let g0 = (p0 >> 5) & 0x1F;
        assert!(
            g0 > 0,
            "pixel at (0,0) should be green (G channel > 0), got 0x{:04X}",
            p0
        );

        // Pixel at (15,0) should have been written (middle of second segment)
        let p15 = gpu.vram[15];
        let g15 = (p15 >> 5) & 0x1F;
        assert!(
            g15 > 0,
            "pixel at (15,0) should be green (G channel > 0), got 0x{:04X}",
            p15
        );
    }

    #[test]
    fn test_gpu_polyline_terminator_exits_polyline_mode() {
        let mut gpu = make_gpu();

        // Start a mono polyline
        gpu.gp0_write(0x48FF_0000); // red polyline
        gpu.gp0_write(0x0000_0000); // v0
        gpu.gp0_write(0x0000_0005); // v1 — now in Polyline mode
        gpu.gp0_write(0x5000_5000); // terminator — should return to Command mode

        // Send a NOP — should work without panic or garbage
        gpu.gp0_write(0x0000_0000);
        assert_eq!(
            gpu.gp0_mode,
            Gp0Mode::Command,
            "GPU should be in Command mode after polyline terminator"
        );
    }

    #[test]
    fn test_gpu_fill_rect_ignores_mask_check() {
        let mut gpu = make_gpu();

        // Fill rect (0x02) should NOT check mask bit — it always writes
        gpu.vram[0] = 0x8000; // set mask bit in background
        gpu.check_mask_bit = true;

        // Draw a fill rect that covers pixel (0,0)
        gpu.gp0_write(0x0200_00FF); // R=255 → 5-bit R=31
        gpu.gp0_write(0x0000_0000); // x=0, y=0
        gpu.gp0_write(0x0001_0010); // h=1, w=16

        // fill_rect (0x02) always writes regardless of mask
        let p = gpu.vram[0];
        assert_eq!(p & 0x1F, 31, "fill_rect should write to masked pixels");
    }

    #[test]
    fn test_gpu_set_pixel_respects_draw_area() {
        let mut gpu = Gpu::new();
        gpu.draw_area_left = 10;
        gpu.draw_area_right = 20;
        gpu.draw_area_top = 10;
        gpu.draw_area_bottom = 20;
        gpu.prim_semi_transparent = false;

        // Pixel outside draw area should not be written
        gpu.set_pixel(5, 5, 255, 0, 0);
        assert_eq!(
            gpu.vram[5 * VRAM_WIDTH + 5],
            0,
            "pixel outside draw area should not be written"
        );

        // Pixel inside draw area should be written
        gpu.set_pixel(15, 15, 255, 0, 0);
        assert_ne!(
            gpu.vram[15 * VRAM_WIDTH + 15],
            0,
            "pixel inside draw area should be written"
        );
    }

    #[test]
    fn test_gpu_gpuinfo_draw_area_latch() {
        let mut gpu = make_gpu();
        gpu.draw_area_left = 100;
        gpu.draw_area_top = 50;

        // GP1 0x10_0003 should load draw area top-left into GPU read latch
        gpu.gp1_write(0x1000_0003);
        let latch = gpu.gpuread();
        assert_eq!(
            latch & 0x3FF,
            100,
            "draw area left should be in bits 0-9 of GPU info latch"
        );
        assert_eq!(
            (latch >> 10) & 0x1FF,
            50,
            "draw area top should be in bits 10-18 of GPU info latch"
        );
    }
}
