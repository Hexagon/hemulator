//! Mega Drive VDP (Video Display Processor) — Yamaha YM7101 (315-5313)
//!
//! Features:
//! - 64KB VRAM, 128 bytes CRAM (64 9-bit color entries), 80 bytes VSRAM
//! - Two scrollable background planes (A, B) + window plane
//! - Up to 80 sprites with per-line limits
//! - 320x224 (H40) or 256x224 (H32) resolution, optional 240-line mode
//! - Horizontal and vertical scrolling with multiple modes

use emu_core::types::Frame;

/// VDP register count
const VDP_REGS: usize = 24;
/// VRAM size (64KB)
const VRAM_SIZE: usize = 0x10000;
/// CRAM size (128 bytes = 64 entries × 2 bytes)
const CRAM_SIZE: usize = 128;
/// VSRAM size (80 bytes = 40 entries × 2 bytes)
const VSRAM_SIZE: usize = 80;

/// VDP status register bits
const STATUS_PAL: u16 = 0x0001;
const STATUS_DMA_BUSY: u16 = 0x0002;
const STATUS_HBLANK: u16 = 0x0004;
const STATUS_VBLANK: u16 = 0x0008;
const STATUS_ODD_FRAME: u16 = 0x0010;
const STATUS_SPRITE_COLLISION: u16 = 0x0020;
const STATUS_SPRITE_OVERFLOW: u16 = 0x0040;
const STATUS_VINT_PENDING: u16 = 0x0080;
const STATUS_FIFO_FULL: u16 = 0x0100;
const STATUS_FIFO_EMPTY: u16 = 0x0200;

/// Access target for control port writes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AccessTarget {
    Vram,
    Cram,
    Vsram,
}

/// DMA mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DmaMode {
    Off,
    /// 68K to VRAM/CRAM/VSRAM
    MemoryToVram,
    /// VRAM fill
    Fill,
    /// VRAM copy
    Copy,
}

/// Mega Drive VDP
pub struct Vdp {
    /// Registers
    pub regs: [u8; VDP_REGS],
    /// Video RAM
    pub vram: Vec<u8>,
    /// Color RAM (palette)
    pub cram: Vec<u8>,
    /// Vertical Scroll RAM
    pub vsram: Vec<u8>,
    /// Status register
    status: u16,
    /// Control port state
    control_pending: bool,
    control_code: u8,
    control_address: u16,
    /// Auto-increment value
    auto_inc: u16,
    /// Access target
    access_target: AccessTarget,
    /// DMA state
    dma_mode: DmaMode,
    dma_source: u32,
    dma_length: u16,
    dma_fill_pending: bool,
    /// H/V counter
    h_counter: u16,
    v_counter: u16,
    /// Current scanline
    scanline: u16,
    /// Frame buffer
    frame: Frame,
    /// Shadow/highlight per-pixel state: 0=shadow, 1=normal, 2=highlight
    shadow_buf: Vec<u8>,
    /// HInt counter
    hint_counter: u16,
    /// HInt pending
    hint_pending: bool,
    /// VInt pending
    vint_pending: bool,
    /// PAL region
    pub region_pal: bool,
}

impl Vdp {
    pub fn new() -> Self {
        Self {
            regs: [0; VDP_REGS],
            vram: vec![0; VRAM_SIZE],
            cram: vec![0; CRAM_SIZE],
            vsram: vec![0; VSRAM_SIZE],
            status: STATUS_VBLANK | STATUS_FIFO_EMPTY,
            control_pending: false,
            control_code: 0,
            control_address: 0,
            auto_inc: 0,
            access_target: AccessTarget::Vram,
            dma_mode: DmaMode::Off,
            dma_source: 0,
            dma_length: 0,
            dma_fill_pending: false,
            h_counter: 0,
            v_counter: 0,
            scanline: 0,
            frame: Frame::new(320, 224),
            shadow_buf: vec![1; 320],
            hint_counter: 0,
            hint_pending: false,
            vint_pending: false,
            region_pal: false,
        }
    }

    pub fn reset(&mut self) {
        self.regs = [0; VDP_REGS];
        self.vram.fill(0);
        self.cram.fill(0);
        self.vsram.fill(0);
        self.status = STATUS_VBLANK | STATUS_FIFO_EMPTY;
        self.control_pending = false;
        self.control_code = 0;
        self.control_address = 0;
        self.auto_inc = 0;
        self.access_target = AccessTarget::Vram;
        self.dma_mode = DmaMode::Off;
        self.dma_source = 0;
        self.dma_length = 0;
        self.dma_fill_pending = false;
        self.h_counter = 0;
        self.v_counter = 0;
        self.scanline = 0;
        self.hint_counter = 0;
        self.hint_pending = false;
        self.vint_pending = false;
        self.frame = Frame::new(320, 224);
        self.shadow_buf = vec![1; 320];
        // Set PAL status bit if in PAL region
        if self.region_pal {
            self.status |= STATUS_PAL;
        }
    }

    // ── Register Access ─────────────────────────────────────────

    /// Write to control port
    pub fn write_control(&mut self, val: u16) {
        // Register writes ($8xxx) ALWAYS take effect, even when pending.
        // On real hardware, writing a register clears the pending flag.
        if val & 0xC000 == 0x8000 {
            self.control_pending = false;
            let reg = ((val >> 8) & 0x1F) as usize;
            let data = (val & 0xFF) as u8;
            if reg < VDP_REGS {
                self.write_register(reg, data);
            }
            self.control_code = 0; // Register write clears code
            return;
        }

        if self.control_pending {
            // Second word of command
            self.control_pending = false;
            self.control_code = (self.control_code & 0x03) | ((((val >> 4) & 0x0F) as u8) << 2);
            self.control_address = (self.control_address & 0x3FFF) | ((val & 0x03) << 14);

            // Decode access target from code
            self.access_target = match self.control_code & 0x0F {
                0x00 => AccessTarget::Vram,  // VRAM read
                0x01 => AccessTarget::Vram,  // VRAM write
                0x03 => AccessTarget::Cram,  // CRAM write
                0x04 => AccessTarget::Vsram, // VSRAM read
                0x05 => AccessTarget::Vsram, // VSRAM write
                0x08 => AccessTarget::Cram,  // CRAM read
                _ => self.access_target,
            };

            // Update auto-increment
            self.auto_inc = self.regs[15] as u16;

            // Check for DMA
            if self.control_code & 0x20 != 0 && self.regs[1] & 0x10 != 0 {
                let dma_mode_bits = self.regs[23] >> 6;
                match dma_mode_bits {
                    0 | 1 => {
                        // Memory to VRAM DMA
                        self.dma_mode = DmaMode::MemoryToVram;
                        self.dma_source = ((self.regs[23] as u32 & 0x7F) << 17)
                            | ((self.regs[22] as u32) << 9)
                            | ((self.regs[21] as u32) << 1);
                        self.dma_length = ((self.regs[20] as u16) << 8) | self.regs[19] as u16;
                    }
                    2 => {
                        // VRAM fill — triggered on next data port write
                        self.dma_mode = DmaMode::Fill;
                        self.dma_fill_pending = true;
                        self.dma_length = ((self.regs[20] as u16) << 8) | self.regs[19] as u16;
                    }
                    3 => {
                        // VRAM copy
                        self.dma_mode = DmaMode::Copy;
                        self.dma_source = ((self.regs[22] as u32) << 8) | self.regs[21] as u32;
                        self.dma_length = ((self.regs[20] as u16) << 8) | self.regs[19] as u16;
                        self.do_dma_copy();
                    }
                    _ => unreachable!(),
                }
            }
            return;
        }

        // First word of command
        self.control_pending = true;
        self.control_code = ((val >> 14) & 0x03) as u8;
        self.control_address = val & 0x3FFF;
    }

    /// Write to a VDP register
    fn write_register(&mut self, reg: usize, val: u8) {
        self.regs[reg] = val;

        match reg {
            0 => {
                // Mode register 1
                // Bit 4: HInt enable
            }
            1 => {
                // Mode register 2
                // Bit 6: Display enable
                // Bit 5: VInt enable
                // Bit 4: DMA enable
                // Bit 3: V30 mode (30 cell = 240 lines)
            }
            15 => {
                // Auto-increment
                self.auto_inc = val as u16;
            }
            _ => {}
        }
    }

    /// Read control port (status register)
    pub fn read_control(&mut self) -> u16 {
        self.control_pending = false;
        let status = self.status;
        // Clear VInt and sprite flags on read
        self.status &= !(STATUS_VINT_PENDING | STATUS_SPRITE_OVERFLOW | STATUS_SPRITE_COLLISION);
        self.vint_pending = false;
        self.hint_pending = false;
        status
    }

    /// Write to data port
    pub fn write_data(&mut self, val: u16) {
        self.control_pending = false;

        // DMA fill
        if self.dma_fill_pending {
            self.dma_fill_pending = false;
            self.do_dma_fill(val);
            return;
        }

        let addr = self.control_address as usize;

        match self.access_target {
            AccessTarget::Vram => {
                // VRAM word writes: byte at addr, byte at addr^1
                // On real hardware, the second byte address has bit 0 XOR'd
                let a = addr & 0xFFFE; // Word-align
                if a < VRAM_SIZE - 1 {
                    self.vram[a] = (val >> 8) as u8;
                    self.vram[a | 1] = val as u8;
                }
            }
            AccessTarget::Cram => {
                let cram_addr = addr & 0x7E; // Word-align for CRAM
                if cram_addr + 1 < CRAM_SIZE {
                    self.cram[cram_addr] = (val >> 8) as u8;
                    self.cram[cram_addr + 1] = val as u8;
                }
            }
            AccessTarget::Vsram => {
                let vsram_addr = addr & 0x7F;
                if vsram_addr < VSRAM_SIZE - 1 {
                    self.vsram[vsram_addr] = (val >> 8) as u8;
                    self.vsram[vsram_addr + 1] = val as u8;
                }
            }
        }

        self.control_address = self.control_address.wrapping_add(self.auto_inc);
    }

    /// Read from data port
    pub fn read_data(&mut self) -> u16 {
        self.control_pending = false;
        let addr = self.control_address as usize;

        let val = match self.access_target {
            AccessTarget::Vram => {
                // VRAM reads are word-aligned
                let a = addr & 0xFFFE;
                if a < VRAM_SIZE - 1 {
                    ((self.vram[a] as u16) << 8) | self.vram[a | 1] as u16
                } else {
                    0
                }
            }
            AccessTarget::Cram => {
                let cram_addr = addr & 0x7F;
                if cram_addr < CRAM_SIZE - 1 {
                    ((self.cram[cram_addr] as u16) << 8) | self.cram[cram_addr + 1] as u16
                } else {
                    0
                }
            }
            AccessTarget::Vsram => {
                let vsram_addr = addr & 0x7F;
                if vsram_addr < VSRAM_SIZE - 1 {
                    ((self.vsram[vsram_addr] as u16) << 8) | self.vsram[vsram_addr + 1] as u16
                } else {
                    0
                }
            }
        };

        self.control_address = self.control_address.wrapping_add(self.auto_inc);
        val
    }

    /// Read H/V counter
    pub fn read_hv_counter(&self) -> u16 {
        // V counter in upper byte, H counter in lower byte
        let v = self.v_counter & 0xFF;
        let h = (self.h_counter >> 1) & 0xFF;
        (v << 8) | h
    }

    // ── DMA operations ──────────────────────────────────────────

    /// Execute 68K-to-VRAM DMA, return words transferred
    pub fn do_dma_68k(&mut self, read_word: &dyn Fn(u32) -> u16) -> u32 {
        if self.dma_mode != DmaMode::MemoryToVram {
            return 0;
        }

        // Length 0 = 65536 on real hardware
        let count = if self.dma_length == 0 {
            0x10000u32
        } else {
            self.dma_length as u32
        };
        let mut src = self.dma_source;

        for _ in 0..count {
            let val = read_word(src);
            self.write_data(val);
            src = src.wrapping_add(2);
        }

        self.dma_source = src;
        self.dma_length = 0;
        self.dma_mode = DmaMode::Off;
        self.status &= !STATUS_DMA_BUSY;
        count
    }

    fn do_dma_fill(&mut self, fill_val: u16) {
        let fill_byte = (fill_val >> 8) as u8;
        // Length 0 = 65536 on real hardware
        let count = if self.dma_length == 0 {
            0x10000u32
        } else {
            self.dma_length as u32
        };

        // First word write (same as regular VRAM word write)
        let a = self.control_address as usize & 0xFFFE;
        if a < VRAM_SIZE - 1 {
            self.vram[a] = (fill_val >> 8) as u8;
            self.vram[a | 1] = fill_val as u8;
        }
        self.control_address = self.control_address.wrapping_add(self.auto_inc);

        // Remaining fills (byte only, using high byte)
        // On real hardware, DMA fill byte writes use address ^ 1
        for _ in 1..count {
            let a = self.control_address as usize;
            if (a ^ 1) < VRAM_SIZE {
                self.vram[a ^ 1] = fill_byte;
            }
            self.control_address = self.control_address.wrapping_add(self.auto_inc);
        }

        self.dma_length = 0;
        self.dma_mode = DmaMode::Off;
    }

    fn do_dma_copy(&mut self) {
        // Length 0 = 65536 on real hardware
        let count = if self.dma_length == 0 {
            0x10000u32
        } else {
            self.dma_length as u32
        };
        let mut src = self.dma_source as usize;
        let mut dst = self.control_address as usize;

        for _ in 0..count {
            if src < VRAM_SIZE && dst < VRAM_SIZE {
                self.vram[dst] = self.vram[src];
            }
            src = (src + 1) & 0xFFFF;
            dst = (dst.wrapping_add(self.auto_inc as usize)) & 0xFFFF;
        }

        self.dma_source = src as u32;
        self.control_address = dst as u16;
        self.dma_length = 0;
        self.dma_mode = DmaMode::Off;
    }

    // ── Scanline/timing ─────────────────────────────────────────

    /// Set the current scanline and update V counter / status
    pub fn set_scanline(&mut self, line: u16) {
        let visible_lines = if self.regs[1] & 0x08 != 0 { 240 } else { 224 };

        self.scanline = line;
        self.v_counter = line;

        if line < visible_lines {
            self.status &= !STATUS_VBLANK;

            // Render this scanline
            self.render_scanline(line);

            // HInt counter
            if line == 0 {
                self.hint_counter = self.regs[10] as u16;
            } else if self.hint_counter == 0 {
                self.hint_counter = self.regs[10] as u16;
                if self.regs[0] & 0x10 != 0 {
                    self.hint_pending = true;
                }
            } else {
                self.hint_counter -= 1;
            }
        } else if line == visible_lines {
            // Enter VBlank
            self.status |= STATUS_VBLANK | STATUS_VINT_PENDING;
            if self.regs[1] & 0x20 != 0 {
                self.vint_pending = true;
            }
        }
    }

    /// Check if VInt should be raised
    pub fn vint_pending(&self) -> bool {
        self.vint_pending
    }

    /// Check if HInt should be raised
    pub fn hint_pending(&self) -> bool {
        self.hint_pending
    }

    /// Clear VInt pending
    pub fn clear_vint(&mut self) {
        self.vint_pending = false;
    }

    /// Clear HInt pending
    pub fn clear_hint(&mut self) {
        self.hint_pending = false;
    }

    /// Whether DMA is pending (68K-to-VRAM)
    pub fn dma_pending(&self) -> bool {
        self.dma_mode == DmaMode::MemoryToVram
    }

    pub fn get_frame(&self) -> &Frame {
        &self.frame
    }

    /// Get display width (H40=320, H32=256)
    pub fn display_width(&self) -> u32 {
        if self.regs[12] & 0x81 != 0 {
            320
        } else {
            256
        }
    }

    /// Get display height
    pub fn display_height(&self) -> u32 {
        if self.regs[1] & 0x08 != 0 {
            240
        } else {
            224
        }
    }

    // ── Rendering ───────────────────────────────────────────────

    fn render_scanline(&mut self, line: u16) {
        let width = self.display_width();
        let height = self.display_height();

        // Ensure frame buffer is correct size
        if self.frame.width != width || self.frame.height != height {
            self.frame = Frame::new(width, height);
        }

        if line >= height as u16 {
            return;
        }

        let display_enabled = self.regs[1] & 0x40 != 0;

        if !display_enabled {
            // Display disabled — fill with backdrop color
            let bg_color = self.get_cram_color(0);
            let y = line as usize;
            let start = y * width as usize;
            for x in 0..width as usize {
                if start + x < self.frame.pixels.len() {
                    self.frame.pixels[start + x] = bg_color;
                }
            }
            return;
        }

        // Render layers from back to front
        let bg_color = self.get_cram_color((self.regs[7] & 0x3F) as usize);
        let y = line as usize;
        let start = y * width as usize;

        // Shadow/highlight mode (reg $0C bit 3)
        let shadow_highlight = self.regs[12] & 0x08 != 0;

        // Initialize shadow buffer
        if self.shadow_buf.len() < width as usize {
            self.shadow_buf.resize(width as usize, 1);
        }
        for x in 0..width as usize {
            self.shadow_buf[x] = if shadow_highlight { 0 } else { 1 }; // 0=shadow in S/H mode
        }

        // 1. Fill with backdrop
        for x in 0..width as usize {
            if start + x < self.frame.pixels.len() {
                self.frame.pixels[start + x] = bg_color;
            }
        }

        // 2. Render Plane B (low priority)
        self.render_plane(line, false, false, width);

        // 3. Render Plane A / Window (low priority)
        self.render_plane_a_with_window(line, false, width);

        // 4. Render sprites (low priority)
        self.render_sprites(line, false, width);

        // 5. Render Plane B (high priority)
        self.render_plane(line, false, true, width);

        // 6. Render Plane A / Window (high priority)
        self.render_plane_a_with_window(line, true, width);

        // 7. Render sprites (high priority)
        self.render_sprites(line, true, width);

        // 8. Apply shadow/highlight post-processing
        if shadow_highlight {
            let start = y * width as usize;
            for x in 0..width as usize {
                let idx = start + x;
                if idx >= self.frame.pixels.len() {
                    break;
                }
                match self.shadow_buf[x] {
                    0 => {
                        // Shadow: halve RGB
                        let pixel = self.frame.pixels[idx];
                        let r = ((pixel >> 16) & 0xFF) >> 1;
                        let g = ((pixel >> 8) & 0xFF) >> 1;
                        let b = (pixel & 0xFF) >> 1;
                        self.frame.pixels[idx] = 0xFF000000 | (r << 16) | (g << 8) | b;
                    }
                    2 => {
                        // Highlight: brighten RGB
                        let pixel = self.frame.pixels[idx];
                        let r = (((pixel >> 16) & 0xFF) >> 1) + 128;
                        let g = (((pixel >> 8) & 0xFF) >> 1) + 128;
                        let b = ((pixel & 0xFF) >> 1) + 128;
                        self.frame.pixels[idx] =
                            0xFF000000 | (r.min(255) << 16) | (g.min(255) << 8) | b.min(255);
                    }
                    _ => {} // Normal: no change
                }
            }
        }

        // 9. Column 0 left blanking (reg $00 bit 5)
        // When set, replaces leftmost 8 pixels with backdrop color to hide scroll artifacts
        if self.regs[0] & 0x20 != 0 {
            let start = y * width as usize;
            for x in 0..8.min(width as usize) {
                let idx = start + x;
                if idx < self.frame.pixels.len() {
                    self.frame.pixels[idx] = bg_color;
                }
            }
        }
    }

    /// Render a background plane for one scanline
    fn render_plane(
        &mut self,
        line: u16,
        is_plane_a: bool,
        high_priority: bool,
        screen_width: u32,
    ) {
        let _h40 = self.regs[12] & 0x81 != 0;

        // Get nametable base address
        let nt_base = if is_plane_a {
            ((self.regs[2] as u16 & 0x38) << 10) as usize
        } else {
            ((self.regs[4] as u16 & 0x07) << 13) as usize
        };

        // Plane size (reg $10)
        let size_reg = self.regs[16];
        let plane_w_cells = match size_reg & 0x03 {
            0 => 32u16,
            1 => 64,
            3 => 128,
            _ => 32,
        };
        let plane_h_cells = match (size_reg >> 4) & 0x03 {
            0 => 32u16,
            1 => 64,
            3 => 128,
            _ => 32,
        };

        // Get scroll values
        let hscroll_base = ((self.regs[13] as u16 & 0x3F) << 10) as usize;
        let hscroll_mode = self.regs[11] & 0x03;

        let hscroll_addr = match hscroll_mode {
            0 => hscroll_base,                            // Per-plane
            1 => hscroll_base, // Per-8 lines (invalid on real hardware, treat as per-plane)
            2 => hscroll_base + (line as usize & !7) * 4, // Per-8 lines
            3 => hscroll_base + line as usize * 4, // Per-line
            _ => hscroll_base,
        };

        let hscroll_entry = if is_plane_a {
            hscroll_addr
        } else {
            hscroll_addr + 2
        };
        let hscroll = if hscroll_entry + 1 < VRAM_SIZE {
            (((self.vram[hscroll_entry] as u16) << 8) | self.vram[hscroll_entry + 1] as u16) & 0x3FF
        } else {
            0
        };

        // Vertical scroll (from VSRAM)
        let vscroll_mode = self.regs[11] & 0x04 != 0; // Per-2-cell column scroll

        for x in 0..screen_width {
            // Get V-scroll for this column
            let vscroll = if vscroll_mode {
                let col = (x / 16) as usize; // 2-cell = 16 pixels
                let vs_addr = if is_plane_a { col * 4 } else { col * 4 + 2 };
                if vs_addr + 1 < VSRAM_SIZE {
                    ((self.vsram[vs_addr] as u16) << 8) | self.vsram[vs_addr + 1] as u16
                } else {
                    0
                }
            } else if is_plane_a {
                if self.vsram.len() >= 2 {
                    ((self.vsram[0] as u16) << 8) | self.vsram[1] as u16
                } else {
                    0
                }
            } else if self.vsram.len() >= 4 {
                ((self.vsram[2] as u16) << 8) | self.vsram[3] as u16
            } else {
                0
            };

            let scrolled_y = line.wrapping_add(vscroll) % (plane_h_cells * 8);
            let scrolled_x = (x as u16).wrapping_sub(hscroll) % (plane_w_cells * 8);
            let cell_x = scrolled_x / 8;
            let cell_y = scrolled_y / 8;
            let pixel_x = scrolled_x % 8;
            let pixel_y = scrolled_y % 8;

            // Read nametable entry (2 bytes per cell)
            let nt_offset =
                nt_base + ((cell_y as usize * plane_w_cells as usize + cell_x as usize) * 2);
            if nt_offset + 1 >= VRAM_SIZE {
                continue;
            }
            let entry = ((self.vram[nt_offset] as u16) << 8) | self.vram[nt_offset + 1] as u16;

            // Decode nametable entry
            let priority = entry & 0x8000 != 0;
            if priority != high_priority {
                continue;
            }
            let palette = ((entry >> 13) & 0x03) as usize;
            let v_flip = entry & 0x1000 != 0;
            let h_flip = entry & 0x0800 != 0;
            let tile_idx = (entry & 0x07FF) as usize;

            // Calculate pixel position in tile
            let ty = if v_flip { 7 - pixel_y } else { pixel_y } as usize;
            let tx = if h_flip { 7 - pixel_x } else { pixel_x } as usize;

            // Read tile pixel (4bpp, 32 bytes per tile)
            let tile_addr = tile_idx * 32 + ty * 4 + (tx / 2);
            if tile_addr >= VRAM_SIZE {
                continue;
            }
            let pixel_byte = self.vram[tile_addr];
            let color_idx = if tx & 1 == 0 {
                (pixel_byte >> 4) & 0x0F
            } else {
                pixel_byte & 0x0F
            };

            // Skip transparent pixels
            if color_idx == 0 {
                continue;
            }

            let cram_idx = palette * 16 + color_idx as usize;
            let color = self.get_cram_color(cram_idx);

            let frame_x = x as usize;
            let frame_y = line as usize;
            let idx = frame_y * screen_width as usize + frame_x;
            if idx < self.frame.pixels.len() {
                self.frame.pixels[idx] = color;
                // In shadow/highlight mode, high-priority BG tiles are normal brightness
                if high_priority && frame_x < self.shadow_buf.len() {
                    self.shadow_buf[frame_x] = 1;
                }
            }
        }
    }

    /// Render Plane A with window plane overlay
    /// The window plane replaces Plane A in the region defined by regs $11/$12
    fn render_plane_a_with_window(&mut self, line: u16, high_priority: bool, screen_width: u32) {
        let h40 = self.regs[12] & 0x81 != 0;

        // Window horizontal position (reg $11)
        let win_h = self.regs[17];
        let win_h_right = win_h & 0x80 != 0; // 1=window is on right side
        let win_h_pos = (win_h & 0x1F) as u16 * 16; // In pixels (units of 2 cells)

        // Window vertical position (reg $12)
        let win_v = self.regs[18];
        let win_v_down = win_v & 0x80 != 0; // 1=window is below line
        let win_v_pos = (win_v & 0x1F) as u16 * 8; // In pixels (units of 1 cell)

        // Window nametable base (reg $03)
        let win_nt_base = if h40 {
            ((self.regs[3] as u16 & 0x3C) << 10) as usize
        } else {
            ((self.regs[3] as u16 & 0x3E) << 10) as usize
        };
        let win_width_cells = if h40 { 64u16 } else { 32 };

        // Determine if this scanline is in the window vertical region
        let in_win_v = if win_v_down {
            line >= win_v_pos
        } else {
            line < win_v_pos
        };

        for x in 0..screen_width {
            // Determine if this pixel is in the window horizontal region
            let in_win_h = if win_h_right {
                x as u16 >= win_h_pos
            } else {
                (x as u16) < win_h_pos
            };

            let use_window = in_win_v || in_win_h;

            if use_window {
                // Render window tile
                let wx = x as u16;
                let wy = line;
                let cell_x = wx / 8;
                let cell_y = wy / 8;
                let pixel_x = wx % 8;
                let pixel_y = wy % 8;

                let nt_offset = win_nt_base
                    + ((cell_y as usize * win_width_cells as usize + cell_x as usize) * 2);
                if nt_offset + 1 >= VRAM_SIZE {
                    continue;
                }
                let entry = ((self.vram[nt_offset] as u16) << 8) | self.vram[nt_offset + 1] as u16;

                let priority = entry & 0x8000 != 0;
                if priority != high_priority {
                    continue;
                }
                let palette = ((entry >> 13) & 0x03) as usize;
                let v_flip = entry & 0x1000 != 0;
                let h_flip = entry & 0x0800 != 0;
                let tile_idx = (entry & 0x07FF) as usize;

                let ty = if v_flip { 7 - pixel_y } else { pixel_y } as usize;
                let tx = if h_flip { 7 - pixel_x } else { pixel_x } as usize;

                let tile_addr = tile_idx * 32 + ty * 4 + (tx / 2);
                if tile_addr >= VRAM_SIZE {
                    continue;
                }
                let pixel_byte = self.vram[tile_addr];
                let color_idx = if tx & 1 == 0 {
                    (pixel_byte >> 4) & 0x0F
                } else {
                    pixel_byte & 0x0F
                };

                if color_idx == 0 {
                    continue;
                }

                let cram_idx = palette * 16 + color_idx as usize;
                let color = self.get_cram_color(cram_idx);

                let idx = line as usize * screen_width as usize + x as usize;
                if idx < self.frame.pixels.len() {
                    self.frame.pixels[idx] = color;
                }
            } else {
                // Render Plane A tile at this position
                self.render_plane_a_pixel(line, x, high_priority, screen_width);
            }
        }
    }

    /// Render a single Plane A pixel (used by window plane logic)
    fn render_plane_a_pixel(&mut self, line: u16, x: u32, high_priority: bool, screen_width: u32) {
        let nt_base = ((self.regs[2] as u16 & 0x38) << 10) as usize;

        let size_reg = self.regs[16];
        let plane_w_cells = match size_reg & 0x03 {
            0 => 32u16,
            1 => 64,
            3 => 128,
            _ => 32,
        };
        let plane_h_cells = match (size_reg >> 4) & 0x03 {
            0 => 32u16,
            1 => 64,
            3 => 128,
            _ => 32,
        };

        // Scroll
        let hscroll_base = ((self.regs[13] as u16 & 0x3F) << 10) as usize;
        let hscroll_mode = self.regs[11] & 0x03;
        let hscroll_addr = match hscroll_mode {
            0 => hscroll_base,
            2 => hscroll_base + (line as usize & !7) * 4,
            3 => hscroll_base + line as usize * 4,
            _ => hscroll_base,
        };
        let hscroll = if hscroll_addr + 1 < VRAM_SIZE {
            (((self.vram[hscroll_addr] as u16) << 8) | self.vram[hscroll_addr + 1] as u16) & 0x3FF
        } else {
            0
        };

        // V-scroll with per-column support
        let vscroll_mode = self.regs[11] & 0x04 != 0;
        let vscroll = if vscroll_mode {
            let col = (x / 16) as usize;
            let vs_addr = col * 4;
            if vs_addr + 1 < VSRAM_SIZE {
                ((self.vsram[vs_addr] as u16) << 8) | self.vsram[vs_addr + 1] as u16
            } else {
                0
            }
        } else if self.vsram.len() >= 2 {
            ((self.vsram[0] as u16) << 8) | self.vsram[1] as u16
        } else {
            0
        };

        let scrolled_y = line.wrapping_add(vscroll) % (plane_h_cells * 8);
        let scrolled_x = (x as u16).wrapping_sub(hscroll) % (plane_w_cells * 8);
        let cell_x = scrolled_x / 8;
        let cell_y = scrolled_y / 8;
        let pixel_x = scrolled_x % 8;
        let pixel_y = scrolled_y % 8;

        let nt_offset =
            nt_base + ((cell_y as usize * plane_w_cells as usize + cell_x as usize) * 2);
        if nt_offset + 1 >= VRAM_SIZE {
            return;
        }
        let entry = ((self.vram[nt_offset] as u16) << 8) | self.vram[nt_offset + 1] as u16;

        let priority = entry & 0x8000 != 0;
        if priority != high_priority {
            return;
        }
        let palette = ((entry >> 13) & 0x03) as usize;
        let v_flip = entry & 0x1000 != 0;
        let h_flip = entry & 0x0800 != 0;
        let tile_idx = (entry & 0x07FF) as usize;

        let ty = if v_flip { 7 - pixel_y } else { pixel_y } as usize;
        let tx = if h_flip { 7 - pixel_x } else { pixel_x } as usize;

        let tile_addr = tile_idx * 32 + ty * 4 + (tx / 2);
        if tile_addr >= VRAM_SIZE {
            return;
        }
        let pixel_byte = self.vram[tile_addr];
        let color_idx = if tx & 1 == 0 {
            (pixel_byte >> 4) & 0x0F
        } else {
            pixel_byte & 0x0F
        };

        if color_idx == 0 {
            return;
        }

        let cram_idx = palette * 16 + color_idx as usize;
        let color = self.get_cram_color(cram_idx);

        let idx = line as usize * screen_width as usize + x as usize;
        if idx < self.frame.pixels.len() {
            self.frame.pixels[idx] = color;
        }
    }

    /// Render sprites for one scanline
    fn render_sprites(&mut self, line: u16, high_priority: bool, screen_width: u32) {
        // Sprite attribute table base
        let sat_base = ((self.regs[5] as u16 & 0x7F) << 9) as usize;

        // Sprite dimensions and limits
        let h40 = self.regs[12] & 0x81 != 0;
        let max_sprites = if h40 { 80 } else { 64 };
        let max_per_line = if h40 { 20 } else { 16 };
        let max_dots_per_line: u16 = if h40 { 320 } else { 256 };

        let mut sprites_on_line = 0;
        let mut dots_on_line: u16 = 0;
        let mut sprite_idx = 0usize;

        for _ in 0..max_sprites {
            let sat_offset = sat_base + sprite_idx * 8;
            if sat_offset + 7 >= VRAM_SIZE {
                break;
            }

            // Read sprite attributes
            let y_pos =
                (((self.vram[sat_offset] as u16) << 8) | self.vram[sat_offset + 1] as u16) & 0x3FF;
            let size_byte = self.vram[sat_offset + 2];
            let h_cells = ((size_byte >> 2) & 0x03) as u16 + 1; // 1-4 cells wide
            let v_cells = (size_byte & 0x03) as u16 + 1; // 1-4 cells tall
            let link = self.vram[sat_offset + 3] & 0x7F;

            let attr = ((self.vram[sat_offset + 4] as u16) << 8) | self.vram[sat_offset + 5] as u16;
            let x_pos = (((self.vram[sat_offset + 6] as u16) << 8)
                | self.vram[sat_offset + 7] as u16)
                & 0x3FF;

            // Sprite Y is offset by 128, X by 128
            let sprite_y = y_pos.wrapping_sub(128);
            let sprite_x = x_pos.wrapping_sub(128);
            let sprite_h = v_cells * 8;

            // Check if sprite is on this line
            let dy = line.wrapping_sub(sprite_y);
            if dy < sprite_h {
                if sprites_on_line >= max_per_line || dots_on_line >= max_dots_per_line {
                    self.status |= STATUS_SPRITE_OVERFLOW;
                    break;
                }
                sprites_on_line += 1;
                dots_on_line += h_cells * 8;

                let priority = attr & 0x8000 != 0;
                if priority != high_priority {
                    // Continue to next sprite (still count toward line limit)
                    if link == 0 || link as usize >= max_sprites {
                        break;
                    }
                    sprite_idx = link as usize;
                    continue;
                }

                let palette = ((attr >> 13) & 0x03) as usize;
                let v_flip = attr & 0x1000 != 0;
                let h_flip = attr & 0x0800 != 0;
                let tile_idx = (attr & 0x07FF) as usize;

                let ty = if v_flip { sprite_h - 1 - dy } else { dy };

                let sprite_w = h_cells * 8;
                for sx in 0..sprite_w {
                    let screen_x = sprite_x.wrapping_add(sx);
                    if screen_x >= screen_width as u16 {
                        continue;
                    }

                    let tx = if h_flip { sprite_w - 1 - sx } else { sx };

                    // Calculate tile index within multi-cell sprite
                    let cell_col = tx / 8;
                    let cell_row = ty / 8;
                    let cell_tile =
                        tile_idx + (cell_col as usize * v_cells as usize) + cell_row as usize;

                    let pixel_x = (tx % 8) as usize;
                    let pixel_y = (ty % 8) as usize;

                    let tile_addr = cell_tile * 32 + pixel_y * 4 + (pixel_x / 2);
                    if tile_addr >= VRAM_SIZE {
                        continue;
                    }

                    let pixel_byte = self.vram[tile_addr];
                    let color_idx = if pixel_x & 1 == 0 {
                        (pixel_byte >> 4) & 0x0F
                    } else {
                        pixel_byte & 0x0F
                    };

                    if color_idx == 0 {
                        continue;
                    }

                    let sx_usize = screen_x as usize;

                    // Shadow/highlight mode: palette 3 color 14/15 are special markers
                    let shadow_highlight = self.regs[12] & 0x08 != 0;
                    if shadow_highlight && palette == 3 {
                        if color_idx == 14 {
                            // Shadow marker: darken underlying pixel
                            if sx_usize < self.shadow_buf.len() && self.shadow_buf[sx_usize] > 0 {
                                self.shadow_buf[sx_usize] -= 1;
                            }
                            continue;
                        } else if color_idx == 15 {
                            // Highlight marker: brighten underlying pixel
                            if sx_usize < self.shadow_buf.len() && self.shadow_buf[sx_usize] < 2 {
                                self.shadow_buf[sx_usize] += 1;
                            }
                            continue;
                        }
                    }

                    let cram_idx = palette * 16 + color_idx as usize;
                    let color = self.get_cram_color(cram_idx);

                    let idx = line as usize * screen_width as usize + screen_x as usize;
                    if idx < self.frame.pixels.len() {
                        self.frame.pixels[idx] = color;
                        // Non-special sprites set normal brightness in S/H mode
                        if shadow_highlight && sx_usize < self.shadow_buf.len() {
                            self.shadow_buf[sx_usize] = 1;
                        }
                    }
                }
            }

            // Follow link chain
            if link == 0 || link as usize >= max_sprites {
                break;
            }
            sprite_idx = link as usize;
        }
    }

    /// Convert CRAM entry to RGBA pixel
    fn get_cram_color(&self, index: usize) -> u32 {
        let addr = (index * 2) % CRAM_SIZE;
        if addr + 1 >= CRAM_SIZE {
            return 0xFF000000;
        }
        let entry = ((self.cram[addr] as u16) << 8) | self.cram[addr + 1] as u16;

        // Mega Drive CRAM format: ----BBB-GGG-RRR-
        let r = ((entry >> 1) & 0x07) as u8;
        let g = ((entry >> 5) & 0x07) as u8;
        let b = ((entry >> 9) & 0x07) as u8;

        // Scale 3-bit to 8-bit (0-7 → 0-255)
        let r8 = (r << 5) | (r << 2) | (r >> 1);
        let g8 = (g << 5) | (g << 2) | (g >> 1);
        let b8 = (b << 5) | (b << 2) | (b >> 1);

        0xFF000000 | (r8 as u32) << 16 | (g8 as u32) << 8 | b8 as u32
    }

    // ── Save State ──────────────────────────────────────────────

    pub fn get_state(&self) -> serde_json::Value {
        serde_json::json!({
            "regs": self.regs.to_vec(),
            "vram": self.vram,
            "cram": self.cram,
            "vsram": self.vsram,
            "status": self.status,
            "control_pending": self.control_pending,
            "control_code": self.control_code,
            "control_address": self.control_address,
            "auto_inc": self.auto_inc,
            "scanline": self.scanline,
            "v_counter": self.v_counter,
            "h_counter": self.h_counter,
            "hint_counter": self.hint_counter,
        })
    }

    pub fn set_state(&mut self, state: &serde_json::Value) -> Result<(), serde_json::Error> {
        if let Some(regs) = state.get("regs").and_then(|v| v.as_array()) {
            for (i, val) in regs.iter().enumerate() {
                if i < VDP_REGS {
                    self.regs[i] = val.as_u64().unwrap_or(0) as u8;
                }
            }
        }
        if let Some(vram) = state.get("vram").and_then(|v| v.as_array()) {
            for (i, val) in vram.iter().enumerate() {
                if i < VRAM_SIZE {
                    self.vram[i] = val.as_u64().unwrap_or(0) as u8;
                }
            }
        }
        if let Some(cram) = state.get("cram").and_then(|v| v.as_array()) {
            for (i, val) in cram.iter().enumerate() {
                if i < CRAM_SIZE {
                    self.cram[i] = val.as_u64().unwrap_or(0) as u8;
                }
            }
        }
        if let Some(vsram) = state.get("vsram").and_then(|v| v.as_array()) {
            for (i, val) in vsram.iter().enumerate() {
                if i < VSRAM_SIZE {
                    self.vsram[i] = val.as_u64().unwrap_or(0) as u8;
                }
            }
        }
        if let Some(v) = state.get("status").and_then(|v| v.as_u64()) {
            self.status = v as u16;
        }
        if let Some(v) = state.get("control_pending").and_then(|v| v.as_bool()) {
            self.control_pending = v;
        }
        if let Some(v) = state.get("control_code").and_then(|v| v.as_u64()) {
            self.control_code = v as u8;
        }
        if let Some(v) = state.get("control_address").and_then(|v| v.as_u64()) {
            self.control_address = v as u16;
        }
        if let Some(v) = state.get("auto_inc").and_then(|v| v.as_u64()) {
            self.auto_inc = v as u16;
        }
        if let Some(v) = state.get("scanline").and_then(|v| v.as_u64()) {
            self.scanline = v as u16;
        }
        Ok(())
    }
}
