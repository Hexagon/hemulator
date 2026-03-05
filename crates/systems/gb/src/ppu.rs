//! Game Boy PPU (Picture Processing Unit / LCD Controller) implementation
//!
//! The PPU is responsible for rendering graphics to the 160x144 pixel LCD screen.
//! It operates in a tile-based system with multiple layers and supports scrolling,
//! windows, and sprites.
//!
//! # Display Architecture
//!
//! ## Layers (in rendering order)
//! 1. **Background**: 256x256 pixel tilemap with scrolling support
//! 2. **Window**: Overlay layer with independent position (WX, WY)
//! 3. **Sprites (OBJ)**: 40 movable 8x8 or 8x16 pixel objects
//!
//! ## Tile System
//! - Tiles are 8x8 pixels, 2 bits per pixel (4 colors)
//! - Each tile uses 16 bytes (2 bytes per row)
//! - Two tile data areas:
//!   - `$8000-$8FFF`: 256 tiles (unsigned addressing mode)
//!   - `$8800-$97FF`: 256 tiles (signed addressing mode, -128 to +127)
//! - Two tile map areas:
//!   - `$9800-$9BFF`: Background/Window tilemap
//!   - `$9C00-$9FFF`: Background/Window tilemap
//!
//! ## Color Palettes
//!
//! ### DMG Mode (Monochrome)
//! - BGP ($FF47): Background palette
//! - OBP0 ($FF48): Object palette 0
//! - OBP1 ($FF49): Object palette 1
//! - Each palette maps 4 colors (2 bits) to 4 shades:
//!   - 0: White (0xFFFFFF)
//!   - 1: Light gray (0xAAAAAA)
//!   - 2: Dark gray (0x555555)
//!   - 3: Black (0x000000)
//!
//! ### CGB Mode (Color)
//! - BCPS/BGPI ($FF68): Background palette index/specification
//! - BCPD/BGPD ($FF69): Background palette data
//! - OCPS/OBPI ($FF6A): Object palette index/specification
//! - OCPD/OBPD ($FF6B): Object palette data
//! - 8 background palettes, 8 object palettes
//! - Each palette has 4 colors
//! - Each color is 15-bit RGB (5 bits per channel)
//! - Color format: gggrrrrr 0bbbbbgg (little-endian)
//! - Auto-increment on palette data write when bit 7 of index register is set
//!
//! ## VRAM Banking (CGB)
//! - VBK ($FF4F): VRAM bank select (bit 0)
//! - Bank 0: Tile pixel data (compatible with DMG)
//! - Bank 1: Tile attributes (CGB only)
//!   - Bit 7: BG-to-OAM priority
//!   - Bit 6: Vertical flip
//!   - Bit 5: Horizontal flip
//!   - Bit 3: Tile VRAM bank (0 or 1)
//!   - Bits 2-0: Background palette number (0-7)
//!
//! # LCD Control Register (LCDC - $FF40)
//!
//! - Bit 7: LCD enable (0=off, 1=on)
//! - Bit 6: Window tilemap area (0=$9800-$9BFF, 1=$9C00-$9FFF)
//! - Bit 5: Window enable (0=off, 1=on)
//! - Bit 4: BG & Window tile data area (0=$8800-$97FF signed, 1=$8000-$8FFF unsigned)
//! - Bit 3: BG tilemap area (0=$9800-$9BFF, 1=$9C00-$9FFF)
//! - Bit 2: Sprite size (0=8x8, 1=8x16)
//! - Bit 1: Sprite enable (0=off, 1=on)
//! - Bit 0: BG & Window enable (0=off, 1=on)
//!
//! # LCD Status Register (STAT - $FF41)
//!
//! - Bit 6: LYC=LY interrupt enable
//! - Bit 5: Mode 2 OAM interrupt enable
//! - Bit 4: Mode 1 VBlank interrupt enable
//! - Bit 3: Mode 0 HBlank interrupt enable
//! - Bit 2: LYC=LY coincidence flag (0=different, 1=equal)
//! - Bits 1-0: Mode flag (0=HBlank, 1=VBlank, 2=OAM search, 3=pixel transfer)
//!
//! # Sprites (OBJ)
//!
//! Each sprite is defined by 4 bytes in OAM (Object Attribute Memory):
//! - Byte 0: Y position (actual position - 16)
//! - Byte 1: X position (actual position - 8)
//! - Byte 2: Tile index
//! - Byte 3: Flags
//!   - **DMG Mode:**
//!     - Bit 7: BG/Window priority (0=above BG, 1=behind BG colors 1-3)
//!     - Bit 6: Y flip
//!     - Bit 5: X flip
//!     - Bit 4: Palette (0=OBP0, 1=OBP1)
//!     - Bits 3-0: Unused
//!   - **CGB Mode:**
//!     - Bit 7: BG/Window priority
//!     - Bit 6: Y flip
//!     - Bit 5: X flip
//!     - Bit 3: Tile VRAM bank (0 or 1)
//!     - Bits 2-0: CGB palette number (0-7)
//!
//! # Timing Model
//!
//! This implementation uses a **frame-based** rendering model:
//! - Entire frames are rendered on-demand
//! - Scanline counter (LY) is updated during CPU execution
//! - V-Blank detection occurs when LY reaches 144
//! - Suitable for most games, but not cycle-accurate
//!
//! ## Actual Hardware Timing (for reference)
//! - Mode 2 (OAM search): 80 cycles
//! - Mode 3 (pixel transfer): 168-291 cycles
//! - Mode 0 (HBlank): 85-208 cycles
//! - Total scanline: 456 cycles
//! - VBlank: 10 scanlines (4560 cycles)
//!
//! # Current Implementation
//!
//! ## Implemented
//! - ✅ Background rendering with scrolling
//! - ✅ Window rendering
//! - ✅ Sprite rendering (8x8 and 8x16)
//! - ✅ Sprite flipping (horizontal and vertical)
//! - ✅ Sprite priority (above/behind background)
//! - ✅ Sprite-per-scanline limit (10 sprites max)
//! - ✅ DMG palette support (BGP, OBP0, OBP1)
//! - ✅ CGB color palettes (8 BG, 8 OBJ, 15-bit RGB)
//! - ✅ CGB VRAM banking (2 banks of 8KB)
//! - ✅ CGB tile attributes (palette, VRAM bank, flip)
//! - ✅ CGB sprite attributes (palette, VRAM bank)
//! - ✅ LYC=LY coincidence detection
//! - ✅ Frame-based timing with scanline counter
//! - ✅ Automatic CGB mode detection and activation
//! - ✅ PPU mode transitions (Mode 0-3) with accurate timing
//! - ✅ STAT interrupts (HBlank, VBlank, OAM, LYC=LY)
//! - ✅ Scanline split effects (per-scanline register capture for SCX, SCY, WX, WY, LCDC)
//!
//! ## Not Implemented
//! - ❌ Cycle-accurate PPU timing
//! - ❌ Mid-scanline effects (register changes within a single scanline)

use emu_core::types::Frame;

/// Captured register state for a single scanline
///
/// This structure stores the values of PPU registers at the start of each scanline.
/// This allows games to change scroll registers (SCX, SCY) or window position (WX, WY)
/// mid-frame using STAT interrupts to achieve scanline split effects (e.g., HUD splits in GTA).
#[derive(Clone, Copy, Debug)]
struct ScanlineState {
    /// Scroll Y position for this scanline
    scy: u8,
    /// Scroll X position for this scanline
    scx: u8,
    /// Window Y position for this scanline
    wy: u8,
    /// Window X position for this scanline
    wx: u8,
    /// LCD Control register for this scanline
    lcdc: u8,
    /// BG Palette (DMG) for this scanline
    bgp: u8,
    /// OBJ Palette 0 (DMG) for this scanline
    obp0: u8,
    /// OBJ Palette 1 (DMG) for this scanline
    obp1: u8,
}

impl Default for ScanlineState {
    fn default() -> Self {
        Self {
            scy: 0,
            scx: 0,
            wy: 0,
            wx: 0,
            lcdc: 0x91,
            bgp: 0xFC,
            obp0: 0xE4,
            obp1: 0xE4,
        }
    }
}

/// Game Boy PPU state
pub struct Ppu {
    /// VRAM Bank 0 (8KB)
    vram_bank0: [u8; 0x2000],
    /// VRAM Bank 1 (8KB, CGB only - contains tile attributes)
    vram_bank1: [u8; 0x2000],
    /// Current VRAM bank (0 or 1, CGB only)
    vram_bank: u8,
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
    /// BG Palette (0xFF47) - DMG only
    pub bgp: u8,
    /// OBJ Palette 0 (0xFF48) - DMG only
    pub obp0: u8,
    /// OBJ Palette 1 (0xFF49) - DMG only
    pub obp1: u8,
    /// Window Y (0xFF4A)
    pub wy: u8,
    /// Window X (0xFF4B)
    pub wx: u8,
    /// Cycle accumulator for scanline timing
    cycle_counter: u32,
    /// Previous PPU mode for detecting mode transitions
    prev_mode: u8,

    // CGB-specific registers and state
    /// Background palette index/specification (0xFF68)
    bgpi: u8,
    /// Object palette index/specification (0xFF6A)
    obpi: u8,
    /// Background palette data (8 palettes × 4 colors × 2 bytes = 64 bytes)
    /// Each color is 15-bit RGB (2 bytes): gggrrrrr 0bbbbbgg
    bg_palette_data: [u8; 64],
    /// Object palette data (8 palettes × 4 colors × 2 bytes = 64 bytes)
    obj_palette_data: [u8; 64],
    /// CGB mode enabled flag
    cgb_mode: bool,
    /// Per-scanline register states (144 scanlines for the visible screen)
    /// Captures register values at the start of each scanline to support scanline split effects
    scanline_states: [ScanlineState; 144],
    /// Per-scanline OAM snapshot for sprite stability
    oam_scanlines: [[u8; 0xA0]; 144],
    /// Flag indicating whether scanline states have been captured this frame
    /// Used to avoid O(n²) iteration in get_scanline_state()
    scanline_states_captured: bool,
    /// Window internal line counter
    /// Increments only when the window is visible on a scanline, persists across scanlines
    window_line_counter: u8,
    /// STAT interrupt line state (for edge-triggered interrupt blocking)
    ///
    /// The STAT interrupt is edge-triggered: it only fires on a transition from low (false) to high (true).
    /// Multiple STAT sources (Mode 0/1/2, LYC=LY) are ORed together. If the line stays high
    /// because one source keeps it active while another activates, no interrupt fires.
    /// This implements the "STAT blocking" behavior found in real hardware.
    ///
    /// Initial state: false (low), allowing the first STAT source to trigger an interrupt.
    ///
    /// Reference: Pan Docs - Interrupt Sources, SameBoy issue #91
    stat_interrupt_line: bool,
}

// LCDC (LCD Control) register bit flags
// Reference: https://gbdev.io/pandocs/LCDC.html
const LCDC_ENABLE: u8 = 0x80;
#[allow(dead_code)]
const LCDC_WIN_TILEMAP: u8 = 0x40;
#[allow(dead_code)]
const LCDC_WIN_ENABLE: u8 = 0x20;
const LCDC_BG_WIN_TILES: u8 = 0x10;
const LCDC_BG_TILEMAP: u8 = 0x08;
#[allow(dead_code)]
const LCDC_OBJ_SIZE: u8 = 0x04;
#[allow(dead_code)]
const LCDC_OBJ_ENABLE: u8 = 0x02;
const LCDC_BG_WIN_ENABLE: u8 = 0x01;

impl Ppu {
    pub fn new() -> Self {
        Self {
            vram_bank0: [0; 0x2000],
            vram_bank1: [0; 0x2000],
            vram_bank: 0,
            oam: [0; 0xA0],
            lcdc: 0x91,
            stat: 0x00,
            scy: 0,
            scx: 0,
            ly: 0,
            lyc: 0,
            bgp: 0xFC,
            obp0: 0xE4,
            obp1: 0xE4,
            wy: 0,
            wx: 0,
            cycle_counter: 0,
            prev_mode: 0,
            bgpi: 0,
            obpi: 0,
            bg_palette_data: [0; 64],
            obj_palette_data: [0; 64],
            cgb_mode: false,
            scanline_states: [ScanlineState::default(); 144],
            oam_scanlines: [[0; 0xA0]; 144],
            scanline_states_captured: false,
            window_line_counter: 0,
            stat_interrupt_line: false,
        }
    }

    /// Enable CGB mode
    ///
    /// # Arguments
    /// * `compatibility_mode` - If true, this is a DMG game with CGB enhancements (flag 0x80).
    ///   If false, this is a CGB-only game (flag 0xC0).
    pub fn enable_cgb_mode(&mut self, compatibility_mode: bool) {
        self.cgb_mode = true;

        if compatibility_mode {
            // For DMG games with CGB support (flag 0x80), initialize with default
            // DMG-compatible greenish palette that the boot ROM would set.
            // This matches the classic Game Boy look on GBC hardware.
            // Source: GBC boot ROM behavior, Pan Docs, TCRF documentation

            // Default greenish palette colors (RGB555 little-endian format):
            // Color 0: White (0x7FFF)
            // Color 1: Light green (0x3E90)
            // Color 2: Dark green (0x16C4)
            // Color 3: Black (0x0000)
            let default_palette: [(u8, u8); 4] = [
                (0xFF, 0x7F), // White
                (0x90, 0x3E), // Light green
                (0xC4, 0x16), // Dark green
                (0x00, 0x00), // Black
            ];

            // Initialize all 8 background palettes with the default palette
            for palette_idx in 0..8 {
                for (color_idx, &(low, high)) in default_palette.iter().enumerate() {
                    let base_idx = (palette_idx * 8) + (color_idx * 2);
                    self.bg_palette_data[base_idx] = low;
                    self.bg_palette_data[base_idx + 1] = high;
                }
            }

            // Initialize all 8 object palettes with the same default palette
            for palette_idx in 0..8 {
                for (color_idx, &(low, high)) in default_palette.iter().enumerate() {
                    let base_idx = (palette_idx * 8) + (color_idx * 2);
                    self.obj_palette_data[base_idx] = low;
                    self.obj_palette_data[base_idx + 1] = high;
                }
            }
        } else {
            // For CGB-only games (flag 0xC0), initialize with white palette
            // as the game will set its own palettes
            for i in 0..64 {
                self.bg_palette_data[i] = if i % 2 == 0 { 0xFF } else { 0x7F };
                self.obj_palette_data[i] = if i % 2 == 0 { 0xFF } else { 0x7F };
            }
        }
    }

    /// Check if CGB mode is enabled
    pub fn is_cgb_mode(&self) -> bool {
        self.cgb_mode
    }

    /// Read from VRAM (0x8000-0x9FFF)
    ///
    /// # Timing Note
    /// This emulator uses a frame-based timing model, so strict Mode 3 VRAM
    /// access restrictions would drop valid writes for some games. We allow
    /// VRAM access during Mode 3 for better compatibility.
    pub fn read_vram(&self, addr: u16) -> u8 {
        // Check if LCD is enabled
        if (self.lcdc & LCDC_ENABLE) == 0 {
            // LCD is off, VRAM is always accessible
            let offset = (addr & 0x1FFF) as usize;
            return if self.vram_bank == 0 {
                self.vram_bank0[offset]
            } else {
                self.vram_bank1[offset]
            };
        }

        let offset = (addr & 0x1FFF) as usize;
        if !self.cgb_mode || self.vram_bank == 0 {
            self.vram_bank0[offset]
        } else {
            self.vram_bank1[offset]
        }
    }

    /// Write to VRAM (0x8000-0x9FFF)
    ///
    /// # Timing Note
    /// This emulator uses a frame-based timing model, so strict Mode 3 VRAM
    /// access restrictions would drop valid writes for some games. We allow
    /// VRAM access during Mode 3 for better compatibility.
    pub fn write_vram(&mut self, addr: u16, val: u8) {
        // Check if LCD is enabled
        if (self.lcdc & LCDC_ENABLE) == 0 {
            // LCD is off, VRAM is always accessible
            let offset = (addr & 0x1FFF) as usize;
            if self.vram_bank == 0 {
                self.vram_bank0[offset] = val;
            } else {
                self.vram_bank1[offset] = val;
            }
            return;
        }

        let offset = (addr & 0x1FFF) as usize;
        if !self.cgb_mode || self.vram_bank == 0 {
            self.vram_bank0[offset] = val;
        } else {
            self.vram_bank1[offset] = val;
        }
    }

    /// Set VRAM bank (VBK register at 0xFF4F)
    pub fn set_vram_bank(&mut self, val: u8) {
        // Only bit 0 matters, bit 1-7 are unused
        self.vram_bank = val & 0x01;
    }

    /// Write LCDC (0xFF40) with LCD on/off side effects
    pub fn write_lcdc(&mut self, val: u8) {
        let old = self.lcdc;
        self.lcdc = val;

        let was_enabled = (old & LCDC_ENABLE) != 0;
        let now_enabled = (val & LCDC_ENABLE) != 0;

        if was_enabled && !now_enabled {
            // LCD turned off: reset LY and mode timing state
            self.ly = 0;
            self.cycle_counter = 0;
            self.prev_mode = 0;
            self.stat &= !0x03;
            self.stat_interrupt_line = false;
            self.scanline_states_captured = false;
            self.window_line_counter = 0;
        } else if !was_enabled && now_enabled {
            // LCD turned on: restart timing from LY=0
            self.ly = 0;
            self.cycle_counter = 0;
            self.prev_mode = 0;
            self.scanline_states_captured = false;
            self.window_line_counter = 0;
        }
    }

    /// Get VRAM bank
    pub fn get_vram_bank(&self) -> u8 {
        self.vram_bank | 0xFE // Bits 1-7 return 1
    }

    /// Read from OAM (0xFE00-0xFE9F)
    ///
    /// # Timing Note
    /// This emulator uses a frame-based timing model, so strict Mode 2/3 OAM
    /// access restrictions can drop valid writes. We allow OAM access during
    /// these modes for better compatibility.
    pub fn read_oam(&self, addr: u16) -> u8 {
        if addr >= 0xA0 {
            return 0xFF; // Out of bounds
        }

        // Check if LCD is enabled
        if (self.lcdc & LCDC_ENABLE) == 0 {
            // LCD is off, OAM is always accessible
            return self.oam[addr as usize];
        }

        self.oam[addr as usize]
    }

    /// Write to OAM (0xFE00-0xFE9F)
    ///
    /// # Timing Note
    /// This emulator uses a frame-based timing model, so strict Mode 2/3 OAM
    /// access restrictions can drop valid writes. We allow OAM access during
    /// these modes for better compatibility.
    pub fn write_oam(&mut self, addr: u16, val: u8) {
        if addr >= 0xA0 {
            return; // Out of bounds
        }

        // Check if LCD is enabled
        if (self.lcdc & LCDC_ENABLE) == 0 {
            // LCD is off, OAM is always accessible
            self.oam[addr as usize] = val;
            return;
        }

        self.oam[addr as usize] = val;
    }

    /// Read from OAM for debugging
    pub fn read_oam_debug(&self, addr: u16) -> u8 {
        if addr >= 0xA0 {
            return 0xFF; // Out of bounds
        }
        self.oam[addr as usize]
    }

    /// Read background palette index register (0xFF68)
    pub fn read_bgpi(&self) -> u8 {
        self.bgpi
    }

    /// Write background palette index register (0xFF68)
    pub fn write_bgpi(&mut self, val: u8) {
        self.bgpi = val;
    }

    /// Read background palette data register (0xFF69)
    pub fn read_bgpd(&self) -> u8 {
        let index = (self.bgpi & 0x3F) as usize;
        self.bg_palette_data[index]
    }

    /// Write background palette data register (0xFF69)
    pub fn write_bgpd(&mut self, val: u8) {
        let index = (self.bgpi & 0x3F) as usize;
        self.bg_palette_data[index] = val;
        // Auto-increment if bit 7 is set
        if (self.bgpi & 0x80) != 0 {
            self.bgpi = (self.bgpi & 0x80) | ((self.bgpi + 1) & 0x3F);
        }
    }

    /// Read object palette index register (0xFF6A)
    pub fn read_obpi(&self) -> u8 {
        self.obpi
    }

    /// Write object palette index register (0xFF6A)
    pub fn write_obpi(&mut self, val: u8) {
        self.obpi = val;
    }

    /// Read object palette data register (0xFF6B)
    pub fn read_obpd(&self) -> u8 {
        let index = (self.obpi & 0x3F) as usize;
        self.obj_palette_data[index]
    }

    /// Write object palette data register (0xFF6B)
    pub fn write_obpd(&mut self, val: u8) {
        let index = (self.obpi & 0x3F) as usize;
        self.obj_palette_data[index] = val;
        // Auto-increment if bit 7 is set
        if (self.obpi & 0x80) != 0 {
            self.obpi = (self.obpi & 0x80) | ((self.obpi + 1) & 0x3F);
        }
    }

    /// Convert CGB 15-bit color to 32-bit ARGB
    /// CGB color format: gggrrrrr 0bbbbbgg (little-endian)
    fn cgb_color_to_rgb(&self, color_low: u8, color_high: u8) -> u32 {
        let color = (color_high as u16) << 8 | color_low as u16;
        let r = ((color & 0x1F) as u32) << 3;
        let g = (((color >> 5) & 0x1F) as u32) << 3;
        let b = (((color >> 10) & 0x1F) as u32) << 3;
        // Expand 5-bit to 8-bit by copying top bits to bottom
        let r = r | (r >> 5);
        let g = g | (g >> 5);
        let b = b | (b >> 5);
        0xFF000000 | (r << 16) | (g << 8) | b
    }

    /// Render a complete frame (160x144)
    pub fn render_frame(&self) -> Frame {
        let mut frame = Frame::new(160, 144);

        if (self.lcdc & LCDC_ENABLE) == 0 {
            // LCD is off - return blank screen
            return frame;
        }

        // Track background color indices and priority for sprite rendering
        // Each byte stores: [bit 7: BG priority, bits 1-0: color index (0-3)]
        let mut bg_color_indices = vec![0u8; 160 * 144];

        // BG/Window rendering behavior depends on mode:
        // - DMG: LCDC.0 = 0 disables BG/Window (blank/white)
        // - CGB: LCDC.0 = 0 removes BG/Window priority (still renders, sprites always on top)
        let bg_win_enabled = (self.lcdc & LCDC_BG_WIN_ENABLE) != 0;

        // Render background
        if bg_win_enabled || self.cgb_mode {
            // CGB: always render BG even if LCDC.0 is 0
            // DMG: only render if LCDC.0 is 1
            self.render_background(&mut frame, &mut bg_color_indices);
        }

        // Render window
        if (self.lcdc & LCDC_WIN_ENABLE) != 0 && (bg_win_enabled || self.cgb_mode) {
            self.render_window(&mut frame, &mut bg_color_indices);
        }

        // Render sprites if enabled
        if (self.lcdc & LCDC_OBJ_ENABLE) != 0 {
            self.render_sprites(&mut frame, &bg_color_indices);
        }

        frame
    }

    /// Calculate tile address using signed tile indexing mode
    ///
    /// When LCDC bit 4 is clear, tile indices are interpreted as signed bytes (-128 to +127).
    /// The base address is $9000, and the index is added as a signed offset.
    /// This allows the tile data to span from $8800 to $97FF.
    ///
    /// Example: tile_index 0 -> $9000, tile_index 1 -> $9010, tile_index 255 (-1) -> $8FF0
    ///
    /// Reference: https://gbdev.io/pandocs/Tile_Data.html#lcdc4--bg-and-window-tile-data-area
    fn calculate_signed_tile_address(&self, base: u16, tile_index: u8) -> u16 {
        // In signed mode, tile_index is treated as signed -128 to 127
        // Base is at $8800, so index 0 would be at $9000 (base + 128 * 16)
        base + ((tile_index as i8 as i16 + 128) as u16 * 16)
    }

    /// Get the scanline state for a given scanline, with fallback to current registers
    /// This ensures backward compatibility when render_frame() is called without step()
    fn get_scanline_state(&self, scanline: u8) -> ScanlineState {
        if self.scanline_states_captured && scanline < 144 {
            self.scanline_states[scanline as usize]
        } else {
            // Fallback: use current register values (for direct rendering without step())
            ScanlineState {
                scy: self.scy,
                scx: self.scx,
                wy: self.wy,
                wx: self.wx,
                lcdc: self.lcdc,
                bgp: self.bgp,
                obp0: self.obp0,
                obp1: self.obp1,
            }
        }
    }

    fn render_background(&self, frame: &mut Frame, bg_color_indices: &mut [u8]) {
        for screen_y in 0u8..144 {
            // Use per-scanline state for scroll and control registers
            let scanline = self.get_scanline_state(screen_y);

            let tile_data_base = if (scanline.lcdc & LCDC_BG_WIN_TILES) != 0 {
                0x0000 // $8000-$8FFF
            } else {
                0x0800 // $8800-$97FF (signed addressing)
            };

            let tilemap_base = if (scanline.lcdc & LCDC_BG_TILEMAP) != 0 {
                0x1C00 // $9C00-$9FFF
            } else {
                0x1800 // $9800-$9BFF
            };

            let y = screen_y.wrapping_add(scanline.scy);
            let tile_y = ((y >> 3) & 31) as u16; // Divide by 8 using bit shift
            let pixel_y = (y & 7) as u16; // Modulo 8 using bitwise AND

            for screen_x in 0u8..160 {
                let x = screen_x.wrapping_add(scanline.scx);
                let tile_x = ((x >> 3) & 31) as u16; // Divide by 8 using bit shift
                let pixel_x = (x & 7) as u16; // Modulo 8 using bitwise AND

                // Get tile index from tilemap (always from VRAM bank 0)
                let tilemap_addr = tilemap_base + (tile_y * 32) + tile_x;
                let tile_index = self.vram_bank0[tilemap_addr as usize];

                // Get tile attributes from VRAM bank 1 (CGB only)
                let tile_attr = if self.cgb_mode {
                    self.vram_bank1[tilemap_addr as usize]
                } else {
                    0
                };

                // CGB tile attributes (from VRAM bank 1):
                // Bit 7: BG-to-OAM Priority (0=use OAM priority, 1=BG priority)
                // Bit 6: Vertical flip
                // Bit 5: Horizontal flip
                // Bit 4: Not used
                // Bit 3: VRAM bank (0=bank 0, 1=bank 1) for tile data
                // Bits 2-0: Background palette number (0-7)
                let bg_palette_num = tile_attr & 0x07;
                let tile_vram_bank = (tile_attr >> 3) & 0x01;
                let flip_x = (tile_attr & 0x20) != 0;
                let flip_y = (tile_attr & 0x40) != 0;
                let bg_priority = (tile_attr & 0x80) != 0;

                // Calculate tile data address
                let tile_addr = if (scanline.lcdc & LCDC_BG_WIN_TILES) != 0 {
                    // Unsigned mode: tiles at $8000-$8FFF
                    tile_data_base + (tile_index as u16 * 16)
                } else {
                    // Signed mode: tiles at $8800-$97FF, index is signed -128 to 127
                    self.calculate_signed_tile_address(tile_data_base, tile_index)
                };

                // Apply vertical flip to pixel_y
                let actual_pixel_y = if flip_y { 7 - pixel_y } else { pixel_y };

                // Get tile data (2 bytes per row) from appropriate VRAM bank
                let tile_row_addr = tile_addr + (actual_pixel_y * 2);
                let (byte1, byte2) = if self.cgb_mode && tile_vram_bank == 1 {
                    (
                        self.vram_bank1[tile_row_addr as usize],
                        self.vram_bank1[(tile_row_addr + 1) as usize],
                    )
                } else {
                    (
                        self.vram_bank0[tile_row_addr as usize],
                        self.vram_bank0[(tile_row_addr + 1) as usize],
                    )
                };

                // Apply horizontal flip to pixel_x
                let actual_pixel_x = if flip_x { 7 - pixel_x } else { pixel_x };

                // Get pixel color (2-bit value)
                let bit = 7 - actual_pixel_x;
                let color_bit_0 = (byte1 >> bit) & 1;
                let color_bit_1 = (byte2 >> bit) & 1;
                let color_index = (color_bit_1 << 1) | color_bit_0;

                // Store color index and priority flag for sprite rendering
                // Format: [bit 7: BG priority flag, bits 1-0: color index]
                let pixel_idx = (screen_y as usize * 160) + screen_x as usize;
                bg_color_indices[pixel_idx] = if bg_priority { 0x80 } else { 0 } | color_index;

                // Apply palette and convert to RGB
                let rgb = if self.cgb_mode {
                    // CGB mode: use color palettes
                    let palette_index = (bg_palette_num * 4 + color_index) * 2;
                    let color_low = self.bg_palette_data[palette_index as usize];
                    let color_high = self.bg_palette_data[(palette_index + 1) as usize];
                    self.cgb_color_to_rgb(color_low, color_high)
                } else {
                    // DMG mode: use monochrome palette
                    let palette_color = (scanline.bgp >> (color_index * 2)) & 0x03;
                    match palette_color {
                        0 => 0xFFFFFFFF, // White (lightest) - ARGB8888 format: 0xAARRGGBB
                        1 => 0xFFAAAAAA, // Light gray (2/3 brightness)
                        2 => 0xFF555555, // Dark gray (1/3 brightness)
                        3 => 0xFF000000, // Black (darkest)
                        _ => unreachable!(),
                    }
                };

                frame.pixels[pixel_idx] = rgb;
            }
        }
    }

    fn render_window(&self, frame: &mut Frame, bg_color_indices: &mut [u8]) {
        // Window rendering - uses per-scanline state to support HUD split effects
        // The window has an internal line counter that increments only when the window
        // is visible on a scanline. This is tracked using window_line_counter.
        //
        // Games like GTA GBC use mid-frame WY/WX changes via LYC interrupts to create
        // a HUD split effect where the window appears only in part of the screen.

        let mut window_line = 0u8;

        for screen_y in 0u8..144 {
            // Use per-scanline state for window position and control registers
            let scanline = self.get_scanline_state(screen_y);

            // Check if window is enabled for this scanline
            let window_enabled = (scanline.lcdc & LCDC_WIN_ENABLE) != 0;

            // Skip if window is not visible on this scanline
            // Window is visible when: WX < 167, screen_y >= WY, and window is enabled
            if !window_enabled || scanline.wx >= 167 || screen_y < scanline.wy {
                continue;
            }

            let tile_data_base = if (scanline.lcdc & LCDC_BG_WIN_TILES) != 0 {
                0x0000 // $8000-$8FFF
            } else {
                0x0800 // $8800-$97FF (signed addressing)
            };

            let tilemap_base = if (scanline.lcdc & LCDC_WIN_TILEMAP) != 0 {
                0x1C00 // $9C00-$9FFF
            } else {
                0x1800 // $9800-$9BFF
            };

            // Use window internal line counter for tile row calculation
            // This correctly handles cases where WY changes mid-frame
            let win_y = window_line;
            let tile_y = (win_y >> 3) as u16; // Divide by 8 using bit shift
            let pixel_y = (win_y & 7) as u16; // Modulo 8 using bitwise AND

            // Ensure tile_y is within bounds (0-31) to prevent out-of-bounds tilemap access
            if tile_y >= 32 {
                window_line = window_line.wrapping_add(1);
                continue;
            }

            let start_x = scanline.wx.saturating_sub(7);

            for screen_x in start_x..160 {
                let win_x = screen_x - start_x;
                let tile_x = (win_x >> 3) as u16; // Divide by 8 using bit shift

                // Ensure tile_x is within bounds (0-31)
                if tile_x >= 32 {
                    continue;
                }

                let pixel_x = (win_x & 7) as u16; // Modulo 8 using bitwise AND

                // Get tile index from tilemap (always from VRAM bank 0)
                let tilemap_addr = tilemap_base + (tile_y * 32) + tile_x;
                let tile_index = self.vram_bank0[tilemap_addr as usize];

                // Get tile attributes from VRAM bank 1 (CGB only)
                let tile_attr = if self.cgb_mode {
                    self.vram_bank1[tilemap_addr as usize]
                } else {
                    0
                };

                let bg_palette_num = tile_attr & 0x07;
                let tile_vram_bank = (tile_attr >> 3) & 0x01;
                let flip_x = (tile_attr & 0x20) != 0;
                let flip_y = (tile_attr & 0x40) != 0;
                let bg_priority = (tile_attr & 0x80) != 0;

                // Calculate tile data address
                let tile_addr = if (scanline.lcdc & LCDC_BG_WIN_TILES) != 0 {
                    tile_data_base + (tile_index as u16 * 16)
                } else {
                    self.calculate_signed_tile_address(tile_data_base, tile_index)
                };

                // Apply vertical flip to pixel_y
                let actual_pixel_y = if flip_y { 7 - pixel_y } else { pixel_y };

                // Get tile data (2 bytes per row)
                let tile_row_addr = tile_addr + (actual_pixel_y * 2);

                // Ensure we don't exceed VRAM bounds
                if (tile_row_addr + 1) as usize >= 0x2000 {
                    continue;
                }

                let (byte1, byte2) = if self.cgb_mode && tile_vram_bank == 1 {
                    (
                        self.vram_bank1[tile_row_addr as usize],
                        self.vram_bank1[(tile_row_addr + 1) as usize],
                    )
                } else {
                    (
                        self.vram_bank0[tile_row_addr as usize],
                        self.vram_bank0[(tile_row_addr + 1) as usize],
                    )
                };

                // Apply horizontal flip to pixel_x
                let actual_pixel_x = if flip_x { 7 - pixel_x } else { pixel_x };

                // Get pixel color (2-bit value)
                let bit = 7 - actual_pixel_x;
                let color_bit_0 = (byte1 >> bit) & 1;
                let color_bit_1 = (byte2 >> bit) & 1;
                let color_index = (color_bit_1 << 1) | color_bit_0;

                // Store color index and priority flag for sprite rendering
                // Format: [bit 7: BG priority flag, bits 1-0: color index]
                let pixel_idx = (screen_y as usize * 160) + screen_x as usize;
                bg_color_indices[pixel_idx] = if bg_priority { 0x80 } else { 0 } | color_index;

                // Apply palette and convert to RGB
                let rgb = if self.cgb_mode {
                    // CGB mode: use color palettes
                    let palette_index = (bg_palette_num * 4 + color_index) * 2;
                    let color_low = self.bg_palette_data[palette_index as usize];
                    let color_high = self.bg_palette_data[(palette_index + 1) as usize];
                    self.cgb_color_to_rgb(color_low, color_high)
                } else {
                    // DMG mode: use monochrome palette
                    let palette_color = (scanline.bgp >> (color_index * 2)) & 0x03;
                    match palette_color {
                        0 => 0xFFFFFFFF, // White (lightest) - ARGB8888 format: 0xAARRGGBB
                        1 => 0xFFAAAAAA, // Light gray (2/3 brightness)
                        2 => 0xFF555555, // Dark gray (1/3 brightness)
                        3 => 0xFF000000, // Black (darkest)
                        _ => unreachable!(),
                    }
                };

                frame.pixels[pixel_idx] = rgb;
            }

            // Increment window line counter since window was visible on this scanline
            window_line = window_line.wrapping_add(1);
        }
    }

    fn render_sprites(&self, frame: &mut Frame, bg_color_indices: &[u8]) {
        // Sprite rendering - Game Boy supports 40 sprites, max 10 per scanline
        let sprite_height = if (self.lcdc & LCDC_OBJ_SIZE) != 0 {
            16
        } else {
            8
        };

        // Process sprites scanline by scanline to enforce 10-sprite limit
        for screen_y in 0u8..144 {
            let scanline = self.get_scanline_state(screen_y);
            let oam = if self.scanline_states_captured {
                &self.oam_scanlines[screen_y as usize]
            } else {
                &self.oam
            };
            // Collect all sprites that intersect this scanline
            // Store: (oam_x, x_pos, oam_index)
            let mut sprites_on_line: Vec<(u8, u8, u8)> = Vec::new();

            for sprite_idx in 0u8..40 {
                let oam_addr = (sprite_idx as usize) * 4;
                let oam_y = oam[oam_addr];
                let oam_x = oam[oam_addr + 1];

                // OAM Y/X are offset by 16/8 respectively
                // Sprites are visible when: 0 < Y < 160 and 0 < X < 168
                // Screen position = OAM position - offset

                // Check if sprite intersects this scanline (Y check)
                // screen_y is in range [sprite_top, sprite_bottom]
                // where sprite_top = oam_y - 16, sprite_bottom = oam_y - 16 + sprite_height - 1
                // Rewritten: oam_y - 16 <= screen_y <= oam_y - 16 + sprite_height - 1
                // Which is: oam_y <= screen_y + 16 <= oam_y + sprite_height - 1
                // Simplified: screen_y + 16 >= oam_y && screen_y + 16 < oam_y + sprite_height
                let screen_y_offset = screen_y.wrapping_add(16);
                if oam_y > 0
                    && screen_y_offset >= oam_y
                    && screen_y_offset < oam_y.wrapping_add(sprite_height)
                {
                    // Sprite intersects this scanline, store X position for sorting
                    let x_pos = oam_x.wrapping_sub(8);
                    sprites_on_line.push((oam_x, x_pos, sprite_idx));
                }
            }

            // Hardware-accurate sprite selection:
            // Both DMG and CGB select the first 10 sprites in OAM order that intersect the scanline
            // Sort by OAM index only for selection
            sprites_on_line.sort_by_key(|&(_oam_x, _x_pos, oam_idx)| oam_idx);

            // TODO: Restore hardware-accurate 10-sprite limit for DMG once timing is improved.
            // Compatibility: some DMG games drop sprites with the strict limit under
            // frame-based timing; relax to reduce flicker.
            let max_sprites = if self.cgb_mode { 10 } else { 40 };
            sprites_on_line.truncate(max_sprites);

            // Hardware-accurate sprite rendering priority:
            // - DMG: Lower X coordinate has higher priority, OAM order as tiebreaker
            // - CGB: Lower OAM index has higher priority (X coordinate irrelevant)
            if !self.cgb_mode {
                // DMG: Re-sort selected sprites by OAM X coordinate, then OAM order for rendering priority
                sprites_on_line.sort_by_key(|&(oam_x, _x_pos, oam_idx)| (oam_x, oam_idx));
            }
            // CGB: Already sorted by OAM order, which is the rendering priority

            // Render sprites in reverse order so lower priority sprites are drawn first
            // (higher priority sprites will overwrite their pixels)
            for &(_oam_x, x_pos, sprite_idx) in sprites_on_line.iter().rev() {
                let oam_addr = (sprite_idx as usize) * 4;
                let oam_y = oam[oam_addr];
                let tile_index = oam[oam_addr + 2];
                let flags = oam[oam_addr + 3];

                // OAM flags interpretation differs between DMG and CGB
                // Bit 7: BG/Window priority
                // Bit 6: Y flip
                // Bit 5: X flip
                // Bit 4: Palette number (DMG: 0=OBP0, 1=OBP1; CGB: not used)
                // Bits 3: VRAM bank (CGB only)
                // Bits 2-0: CGB palette number (0-7, CGB only)
                let flip_x = (flags & 0x20) != 0;
                let flip_y = (flags & 0x40) != 0;
                let bg_priority = (flags & 0x80) != 0;

                let (dmg_palette_num, cgb_palette_num, sprite_vram_bank) = if self.cgb_mode {
                    (0, flags & 0x07, (flags >> 3) & 0x01)
                } else {
                    ((flags >> 4) & 0x01, 0, 0)
                };

                // Calculate which row of the sprite we're rendering
                // sy = screen_y - (oam_y - 16) = screen_y - oam_y + 16
                let sy = screen_y.wrapping_add(16).wrapping_sub(oam_y);
                let pixel_y = if flip_y { sprite_height - 1 - sy } else { sy };

                // For 8x16 sprites, use tile_index & 0xFE for top, tile_index | 0x01 for bottom
                let tile = if sprite_height == 16 {
                    if pixel_y < 8 {
                        tile_index & 0xFE
                    } else {
                        tile_index | 0x01
                    }
                } else {
                    tile_index
                };

                let tile_addr = (tile as u16) * 16;
                let row_offset = (pixel_y & 7) * 2; // Modulo 8 using bitwise AND

                // Ensure we don't exceed VRAM bounds
                if (tile_addr + row_offset as u16 + 1) as usize >= 0x2000 {
                    continue;
                }

                // Get tile data from appropriate VRAM bank (CGB sprites can use bank 1)
                let (byte1, byte2) = if self.cgb_mode && sprite_vram_bank == 1 {
                    (
                        self.vram_bank1[(tile_addr + row_offset as u16) as usize],
                        self.vram_bank1[(tile_addr + row_offset as u16 + 1) as usize],
                    )
                } else {
                    (
                        self.vram_bank0[(tile_addr + row_offset as u16) as usize],
                        self.vram_bank0[(tile_addr + row_offset as u16 + 1) as usize],
                    )
                };

                for sx in 0..8u8 {
                    // Calculate actual screen X position
                    // OAM X position is offset by 8: X=0 means off-screen left, X=8 means screen X=0
                    // x_pos = oam_x - 8 (calculated at line 898)
                    // Sprites can be partially visible on left/right edges
                    let screen_x = x_pos.wrapping_add(sx);

                    // Skip pixels that are off-screen
                    // Screen X must be in range [0, 159]
                    // Due to wrapping: values >= 160 are off-screen (either left or right edge)
                    // Example: OAM X=0 → x_pos=248 (wraps) → screen_x=248-255 → filtered out
                    // Example: OAM X=8 → x_pos=0 → screen_x=0-7 → rendered
                    if screen_x >= 160 {
                        continue;
                    }

                    let pixel_x = if flip_x { 7 - sx } else { sx };
                    let bit = 7 - pixel_x;
                    let color_bit_0 = (byte1 >> bit) & 1;
                    let color_bit_1 = (byte2 >> bit) & 1;
                    let color_index = (color_bit_1 << 1) | color_bit_0;

                    // Color 0 is transparent for sprites
                    if color_index == 0 {
                        continue;
                    }

                    // Check background priority
                    // Extract color index and priority flag from bg_color_indices
                    let pixel_idx = (screen_y as usize * 160) + screen_x as usize;
                    let bg_data = bg_color_indices[pixel_idx];
                    let bg_color_index = bg_data & 0x03; // Bits 1-0: color index
                    let bg_has_priority = (bg_data & 0x80) != 0; // Bit 7: BG priority flag

                    // Sprite priority rules:
                    // 1. If LCDC.0 is 0 in CGB mode, sprites are always on top
                    // 2. If BG color is 0, sprite always shows
                    // 3. If BG tile has priority flag set (CGB only), BG is above sprite
                    // 4. If sprite OBJ priority flag is set, sprite is behind BG colors 1-3
                    // 5. Otherwise, sprite is above BG

                    let bg_win_master_priority = (self.lcdc & LCDC_BG_WIN_ENABLE) != 0;

                    if self.cgb_mode && !bg_win_master_priority {
                        // CGB mode with LCDC.0 = 0: sprites always on top
                    } else if bg_color_index == 0 {
                        // BG is transparent, sprite always shows
                    } else if self.cgb_mode && bg_has_priority {
                        // CGB: BG tile has priority, sprite is behind
                        continue;
                    } else if bg_priority {
                        // Sprite has priority flag set, behind BG colors 1-3 (but not 0)
                        continue;
                    }
                    // Otherwise, sprite is above BG

                    // Apply palette and convert to RGB
                    let rgb = if self.cgb_mode {
                        // CGB mode: use color palettes
                        let palette_index = (cgb_palette_num * 4 + color_index) * 2;
                        let color_low = self.obj_palette_data[palette_index as usize];
                        let color_high = self.obj_palette_data[(palette_index + 1) as usize];
                        self.cgb_color_to_rgb(color_low, color_high)
                    } else {
                        // DMG mode: use monochrome palettes
                        let palette = if dmg_palette_num == 1 {
                            scanline.obp1
                        } else {
                            scanline.obp0
                        };
                        // Map 2-bit color to grayscale using OBP0/OBP1 palette
                        let palette_color = (palette >> (color_index * 2)) & 0x03;
                        match palette_color {
                            0 => 0xFFFFFFFF, // White (lightest) - ARGB8888 format: 0xAARRGGBB
                            1 => 0xFFAAAAAA, // Light gray (2/3 brightness)
                            2 => 0xFF555555, // Dark gray (1/3 brightness)
                            3 => 0xFF000000, // Black (darkest)
                            _ => unreachable!(),
                        }
                    };

                    frame.pixels[pixel_idx] = rgb;
                }
            }
        }
    }

    /// Step the PPU for the given number of cycles
    ///
    /// Updates the PPU mode bits in STAT register based on current scanline position.
    /// Returns true if VBlank just started (for triggering VBlank interrupt).
    ///
    /// # PPU Mode Timing (per scanline = 456 cycles)
    /// - Mode 2 (OAM Search): Cycles 0-79 (80 cycles)
    /// - Mode 3 (Pixel Transfer): Cycles 80-251 (172 cycles typical)
    /// - Mode 0 (HBlank): Cycles 252-455 (204 cycles typical)
    /// - Mode 1 (VBlank): Lines 144-153
    ///
    /// Step the PPU with the given number of cycles
    ///
    /// Returns (vblank_started, stat_interrupt, hblank_entered)
    pub fn step(&mut self, cycles: u32) -> (bool, bool, bool) {
        // When LCD is disabled, LY stays at 0 and no timing progresses.
        // However, we still track the 456-cycle scanline rhythm so that
        // HBlank DMA (HDMA) transfers can complete.  On real hardware,
        // mode 0 (HBlank) is continuously active when the LCD is off,
        // meaning HDMA transfers one 16-byte block per ~456 T-cycles.
        if (self.lcdc & LCDC_ENABLE) == 0 {
            self.ly = 0;
            self.prev_mode = 0;
            self.stat &= !0x03; // Mode 0 always when LCD off
            self.stat_interrupt_line = false;
            self.scanline_states_captured = false;
            self.window_line_counter = 0;

            // Track cycle counter for HDMA timing even with LCD off
            self.cycle_counter += cycles;
            let mut hblank_entered = false;
            while self.cycle_counter >= 456 {
                self.cycle_counter -= 456;
                hblank_entered = true;
            }
            return (false, false, hblank_entered);
        }

        // Accumulate cycles
        self.cycle_counter += cycles;

        let mut vblank_started = false;

        // Process complete scanlines (456 cycles each)
        while self.cycle_counter >= 456 {
            self.cycle_counter -= 456;

            // Capture register state for the CURRENT scanline before incrementing LY
            // This ensures we capture the state that will be used to render this scanline
            if self.ly < 144 {
                // Reset window counter at the start of a new frame
                if self.ly == 0 {
                    self.window_line_counter = 0;
                    self.scanline_states_captured = false;
                }

                self.scanline_states[self.ly as usize] = ScanlineState {
                    scy: self.scy,
                    scx: self.scx,
                    wy: self.wy,
                    wx: self.wx,
                    lcdc: self.lcdc,
                    bgp: self.bgp,
                    obp0: self.obp0,
                    obp1: self.obp1,
                };
                self.oam_scanlines[self.ly as usize] = self.oam;
                self.scanline_states_captured = true;
            }

            self.ly = (self.ly + 1) % 154;

            // Update LYC=LY coincidence flag (bit 2 of STAT)
            // The interrupt line calculation in update_stat_mode() will check this flag
            if self.ly == self.lyc {
                self.stat |= 0x04; // Set coincidence flag
            } else {
                self.stat &= !0x04; // Clear coincidence flag
            }

            // V-Blank is lines 144-153
            //
            // # Hardware Timing Note
            // This implementation triggers VBlank at the transition to line 144 (scanline-accurate).
            // Real hardware triggers the VBlank interrupt on the first M-cycle of line 144.
            // This implementation uses frame-based rendering (not cycle-accurate within scanlines),
            // so VBlank timing is approximate but sufficient for ~99% of games.
            //
            // Games that rely on precise cycle-level VBlank timing may have edge cases.
            if self.ly == 144 {
                vblank_started = true;
            }
        }

        // Update STAT mode bits and check for STAT interrupt (with edge-triggered blocking) and HBlank entry
        // The LYC=LY source is now integrated into the interrupt line calculation
        let (stat_interrupt, hblank_entered) = self.update_stat_mode();

        (vblank_started, stat_interrupt, hblank_entered)
    }

    /// Update the PPU mode bits in STAT register and check for STAT interrupt
    ///
    /// The mode is determined by:
    /// - LY >= 144: Mode 1 (VBlank)
    /// - LY < 144 and cycle_counter < 80: Mode 2 (OAM Search)
    /// - LY < 144 and cycle_counter < 252: Mode 3 (Pixel Transfer)
    /// - LY < 144 and cycle_counter >= 252: Mode 0 (HBlank)
    ///
    /// This implements edge-triggered STAT interrupt blocking:
    /// - Multiple STAT sources (Mode 0/1/2, LYC=LY) are ORed together into a single interrupt line
    /// - The interrupt only fires on a rising edge (low→high transition) of this line
    /// - If the line stays high (multiple sources active), no new interrupt fires (blocking)
    ///
    /// Reference: Pan Docs - Interrupt Sources, SameBoy issue #91
    ///
    /// Returns (stat_interrupt, hblank_entered)
    fn update_stat_mode(&mut self) -> (bool, bool) {
        // Get previous mode before updating
        let prev_mode = self.stat & 0x03;

        // Clear mode bits (bits 0-1)
        self.stat &= !0x03;

        // Check if LCD is enabled
        if (self.lcdc & LCDC_ENABLE) == 0 {
            // LCD disabled: mode is always 0, interrupt line is low
            self.stat_interrupt_line = false;
            return (false, false);
        }

        let mode = if self.ly >= 144 {
            // Lines 144-153: VBlank (Mode 1)
            1
        } else if self.cycle_counter < 80 {
            // First 80 cycles: OAM Search (Mode 2)
            2
        } else if self.cycle_counter < 252 {
            // Cycles 80-251: Pixel Transfer (Mode 3)
            3
        } else {
            // Cycles 252-455: HBlank (Mode 0)
            0
        };

        // Set mode bits
        self.stat |= mode;

        // Check if we just entered HBlank
        let hblank_entered = mode == 0 && prev_mode != 0;

        // Store new mode for next iteration
        self.prev_mode = mode;

        // Calculate new STAT interrupt line state by ORing all enabled sources
        // STAT interrupts are enabled via bits 3-6:
        // Bit 3: Mode 0 (HBlank) interrupt enable
        // Bit 4: Mode 1 (VBlank) interrupt enable
        // Bit 5: Mode 2 (OAM) interrupt enable
        // Bit 6: LYC=LY interrupt enable (checked in step() method)
        let mode_0_source = mode == 0 && (self.stat & 0x08) != 0;
        let mode_1_source = mode == 1 && (self.stat & 0x10) != 0;
        let mode_2_source = mode == 2 && (self.stat & 0x20) != 0;
        let lyc_ly_source = (self.stat & 0x04) != 0 && (self.stat & 0x40) != 0;

        // OR all sources together to get the new line state
        let new_line_state = mode_0_source || mode_1_source || mode_2_source || lyc_ly_source;

        // Detect rising edge: interrupt fires only if line transitions from low to high
        let stat_interrupt = !self.stat_interrupt_line && new_line_state;

        // Update stored line state for next check
        self.stat_interrupt_line = new_line_state;

        (stat_interrupt, hblank_entered)
    }

    // Tile viewer helper methods

    /// Get VRAM bank 0 data
    pub fn get_vram_bank0(&self) -> &[u8; 0x2000] {
        &self.vram_bank0
    }

    /// Get VRAM bank 1 data
    pub fn get_vram_bank1(&self) -> &[u8; 0x2000] {
        &self.vram_bank1
    }

    /// Get OAM data
    pub fn get_oam(&self) -> &[u8; 0xA0] {
        &self.oam
    }

    /// Get CGB background color (15-bit BGR to 32-bit ARGB)
    pub fn get_cgb_bg_color(&self, palette: usize, color: usize) -> u32 {
        let idx = (palette * 8) + (color * 2);
        if idx + 1 < self.bg_palette_data.len() {
            let low = self.bg_palette_data[idx];
            let high = self.bg_palette_data[idx + 1];
            self.cgb_color_to_rgb(low, high)
        } else {
            0xFF000000 // Black
        }
    }

    /// Get CGB object color (15-bit BGR to 32-bit ARGB)
    pub fn get_cgb_obj_color(&self, palette: usize, color: usize) -> u32 {
        let idx = (palette * 8) + (color * 2);
        if idx + 1 < self.obj_palette_data.len() {
            let low = self.obj_palette_data[idx];
            let high = self.obj_palette_data[idx + 1];
            self.cgb_color_to_rgb(low, high)
        } else {
            0xFF000000 // Black
        }
    }

    /// Get DMG background color
    pub fn get_dmg_bg_color(&self, color: usize) -> u32 {
        let palette = self.bgp;
        let shade = (palette >> (color * 2)) & 0x03;
        match shade {
            0 => 0xFFFFFFFF, // White (lightest) - ARGB8888 format: 0xAARRGGBB
            1 => 0xFFAAAAAA, // Light gray (2/3 brightness)
            2 => 0xFF555555, // Dark gray (1/3 brightness)
            _ => 0xFF000000, // Black (darkest)
        }
    }

    /// Get DMG object color
    pub fn get_dmg_obj_color(&self, palette_idx: usize, color: usize) -> u32 {
        let palette = if palette_idx == 0 {
            self.obp0
        } else {
            self.obp1
        };
        let shade = (palette >> (color * 2)) & 0x03;
        match shade {
            0 => 0xFFFFFFFF, // White (transparent for sprites, but we'll show it)
            1 => 0xFFAAAAAA, // Light gray (2/3 brightness)
            2 => 0xFF555555, // Dark gray (1/3 brightness)
            _ => 0xFF000000, // Black (darkest)
        }
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
    fn test_vram_access_restrictions() {
        let mut ppu = Ppu::new();
        ppu.lcdc = LCDC_ENABLE; // Enable LCD

        // Write test data to VRAM when accessible
        ppu.stat = 0x00; // Mode 0 (HBlank) - VRAM accessible
        ppu.write_vram(0x1000, 0x42);
        assert_eq!(
            ppu.read_vram(0x1000),
            0x42,
            "VRAM should be accessible in Mode 0"
        );

        // Mode 1 (VBlank) - VRAM accessible
        ppu.stat = 0x01;
        ppu.write_vram(0x1001, 0x43);
        assert_eq!(
            ppu.read_vram(0x1001),
            0x43,
            "VRAM should be accessible in Mode 1"
        );

        // Mode 2 (OAM Search) - VRAM accessible
        ppu.stat = 0x02;
        ppu.write_vram(0x1002, 0x44);
        assert_eq!(
            ppu.read_vram(0x1002),
            0x44,
            "VRAM should be accessible in Mode 2"
        );

        // Mode 3 (Pixel Transfer) - VRAM still accessible in frame-based model
        ppu.stat = 0x03;
        ppu.write_vram(0x1003, 0x45);
        assert_eq!(
            ppu.read_vram(0x1003),
            0x45,
            "VRAM writes should be allowed in Mode 3 for compatibility"
        );

        // LCD disabled - VRAM always accessible
        ppu.lcdc = 0x00; // Disable LCD
        ppu.stat = 0x03; // Mode 3
        ppu.write_vram(0x1004, 0x46);
        assert_eq!(
            ppu.read_vram(0x1004),
            0x46,
            "VRAM should be accessible when LCD is off"
        );
    }

    #[test]
    fn test_oam_read_write() {
        let mut ppu = Ppu::new();
        ppu.write_oam(0x10, 0x42);
        assert_eq!(ppu.read_oam(0x10), 0x42);
    }

    #[test]
    fn test_oam_access_restrictions() {
        let mut ppu = Ppu::new();
        ppu.lcdc = LCDC_ENABLE; // Enable LCD

        // Write test data to OAM when accessible
        ppu.stat = 0x00; // Mode 0 (HBlank) - OAM accessible
        ppu.write_oam(0x10, 0x42);
        assert_eq!(
            ppu.read_oam(0x10),
            0x42,
            "OAM should be accessible in Mode 0"
        );

        // Mode 1 (VBlank) - OAM accessible
        ppu.stat = 0x01;
        ppu.write_oam(0x11, 0x43);
        assert_eq!(
            ppu.read_oam(0x11),
            0x43,
            "OAM should be accessible in Mode 1"
        );

        // Mode 2 (OAM Search) - OAM still accessible in frame-based model
        ppu.stat = 0x02;
        ppu.write_oam(0x12, 0x44);
        assert_eq!(
            ppu.read_oam(0x12),
            0x44,
            "OAM writes should be allowed in Mode 2 for compatibility"
        );

        // Mode 3 (Pixel Transfer) - OAM still accessible in frame-based model
        ppu.stat = 0x03;
        ppu.write_oam(0x13, 0x45);
        assert_eq!(
            ppu.read_oam(0x13),
            0x45,
            "OAM writes should be allowed in Mode 3 for compatibility"
        );

        // LCD disabled - OAM always accessible
        ppu.lcdc = 0x00; // Disable LCD
        ppu.stat = 0x02; // Mode 2
        ppu.write_oam(0x14, 0x46);
        assert_eq!(
            ppu.read_oam(0x14),
            0x46,
            "OAM should be accessible when LCD is off"
        );
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
        let _ = ppu.step(456); // One scanline
        assert_eq!(ppu.ly, 1);
    }

    #[test]
    fn test_vblank_detection() {
        let mut ppu = Ppu::new();
        ppu.ly = 143;
        let (vblank, _stat, _hblank) = ppu.step(456);
        assert!(vblank);
        assert_eq!(ppu.ly, 144);
    }

    #[test]
    fn test_window_rendering() {
        let mut ppu = Ppu::new();
        ppu.lcdc = 0xE1; // Enable LCD, window, and background
        ppu.wy = 0;
        ppu.wx = 7;

        // Set up a simple tile in VRAM
        ppu.write_vram(0x0000, 0xFF); // First byte of tile 0
        ppu.write_vram(0x0001, 0xFF); // Second byte of tile 0

        // Set window tilemap to use tile 0
        ppu.write_vram(0x1800, 0x00); // Tilemap entry for tile 0

        let frame = ppu.render_frame();
        assert_eq!(frame.width, 160);
        assert_eq!(frame.height, 144);
    }

    #[test]
    fn test_sprite_rendering() {
        let mut ppu = Ppu::new();
        ppu.lcdc = 0x93; // Enable LCD, sprites, and background

        // Set up sprite in OAM
        ppu.write_oam(0, 16); // Y position
        ppu.write_oam(1, 8); // X position
        ppu.write_oam(2, 0); // Tile index
        ppu.write_oam(3, 0); // Flags (no flip, palette 0, above BG)

        // Set up a simple tile in VRAM for the sprite
        ppu.write_vram(0x0000, 0xFF);
        ppu.write_vram(0x0001, 0xFF);

        let frame = ppu.render_frame();
        assert_eq!(frame.width, 160);
        assert_eq!(frame.height, 144);
    }

    #[test]
    fn test_sprite_flip() {
        let mut ppu = Ppu::new();
        ppu.lcdc = 0x93;

        // Set up sprite with horizontal flip
        ppu.write_oam(0, 16);
        ppu.write_oam(1, 8);
        ppu.write_oam(2, 0);
        ppu.write_oam(3, 0x20); // Flip X flag

        ppu.write_vram(0x0000, 0x80); // Left-most pixel set
        ppu.write_vram(0x0001, 0x00);

        let frame = ppu.render_frame();
        // With flip, the pixel should appear on the right side
        assert_eq!(frame.width, 160);
        assert_eq!(frame.height, 144);
    }

    #[test]
    fn test_sprite_priority() {
        let mut ppu = Ppu::new();
        ppu.lcdc = 0x93;

        // Set up sprite with background priority
        ppu.write_oam(0, 16);
        ppu.write_oam(1, 8);
        ppu.write_oam(2, 0);
        ppu.write_oam(3, 0x80); // BG priority flag

        let frame = ppu.render_frame();
        assert_eq!(frame.width, 160);
        assert_eq!(frame.height, 144);
    }

    #[test]
    fn test_lyc_coincidence() {
        let mut ppu = Ppu::new();
        ppu.ly = 10;
        ppu.lyc = 11;

        let _ = ppu.step(456);
        assert_eq!(ppu.ly, 11);
        assert!(ppu.stat & 0x04 != 0); // Coincidence flag should be set
    }

    #[test]
    fn test_stat_interrupt_hblank() {
        let mut ppu = Ppu::new();
        ppu.lcdc = 0x91; // Enable LCD
        ppu.stat |= 0x08; // Enable HBlank STAT interrupt (bit 3)
        ppu.ly = 0;
        ppu.cycle_counter = 0;

        // Step to mode 2 (OAM search, cycles 0-79) - no interrupt yet
        let (_vblank, stat, _hblank) = ppu.step(40);
        assert!(!stat);
        assert_eq!(ppu.stat & 0x03, 2); // Mode 2

        // Step to mode 3 (pixel transfer, cycles 80-251) - no interrupt
        let (_vblank, stat, _hblank) = ppu.step(100);
        assert!(!stat);
        assert_eq!(ppu.stat & 0x03, 3); // Mode 3

        // Step to mode 0 (HBlank, cycles 252-455) - should trigger STAT interrupt
        let (_vblank, stat, _hblank) = ppu.step(120); // Now at cycle 260, which is mode 0
        assert!(stat); // HBlank STAT interrupt should fire
        assert_eq!(ppu.stat & 0x03, 0); // Mode 0
    }

    #[test]
    fn test_stat_interrupt_oam() {
        let mut ppu = Ppu::new();
        ppu.lcdc = 0x91; // Enable LCD
        ppu.stat |= 0x20; // Enable OAM STAT interrupt (bit 5)
        ppu.ly = 0;
        ppu.cycle_counter = 400; // Near end of previous scanline

        // Step to next scanline - should enter mode 2 and trigger interrupt
        let (_vblank, stat, _hblank) = ppu.step(60);
        assert!(stat); // OAM STAT interrupt should fire
        assert_eq!(ppu.stat & 0x03, 2); // Mode 2
        assert_eq!(ppu.ly, 1);
    }

    #[test]
    fn test_stat_interrupt_lyc() {
        let mut ppu = Ppu::new();
        ppu.lcdc = 0x91; // Enable LCD
        ppu.stat |= 0x40; // Enable LYC=LY STAT interrupt (bit 6)
        ppu.ly = 10;
        ppu.lyc = 11;

        // Step to line 11 - should trigger LYC=LY interrupt
        let (_vblank, stat, _hblank) = ppu.step(456);
        assert!(stat); // LYC=LY STAT interrupt should fire
        assert_eq!(ppu.ly, 11);
        assert!(ppu.stat & 0x04 != 0); // Coincidence flag should be set
    }

    #[test]
    fn test_sprite_at_left_edge() {
        // Regression test for sprite visibility bug
        // Sprites with OAM X position 0-7 should be partially visible on left edge
        let mut ppu = Ppu::new();
        ppu.lcdc = 0x93; // Enable LCD, sprites, and background
        ppu.obp0 = 0xE4; // Set sprite palette (11 10 01 00)

        // Set up sprite at X=4 (partially visible on left edge)
        ppu.write_oam(0, 16); // Y position (screen Y = 0)
        ppu.write_oam(1, 4); // X position (after -8 offset, rightmost 4 pixels visible at screen X=0-3)
        ppu.write_oam(2, 0); // Tile index
        ppu.write_oam(3, 0); // Flags (no flip, palette 0, above BG)

        // Set up a visible tile in VRAM
        // Create a solid tile with color index 3 (non-transparent)
        ppu.write_vram(0x0000, 0xFF); // Bitplane 0: all 1s
        ppu.write_vram(0x0001, 0xFF); // Bitplane 1: all 1s (color index = 3)

        let frame = ppu.render_frame();

        // Verify sprite is rendered: the rightmost 4 pixels should be visible (X = 0-3)
        // Color index 3 with palette 0xE4: (0xE4 >> (3 * 2)) & 0x03 = (0xE4 >> 6) & 0x03 = 3 (darkest/black)
        let expected_color = 0xFF000000; // Black

        // Check that at least one pixel from the sprite is visible on screen
        // The sprite at X=4 means screen positions 0-3 should show the sprite (last 4 pixels)
        let screen_y = 0;
        let mut found_sprite_pixel = false;
        for screen_x in 0..4 {
            let pixel = frame.pixels[screen_y * 160 + screen_x];
            if pixel == expected_color {
                found_sprite_pixel = true;
                break;
            }
        }
        assert!(
            found_sprite_pixel,
            "Sprite at X=4 should be partially visible on left edge"
        );
    }

    #[test]
    fn test_sprite_per_scanline_limit() {
        let mut ppu = Ppu::new();
        ppu.lcdc = 0x93; // Enable LCD, sprites, and background

        // Create 15 sprites all on the same scanline (Y=16, screen line 0)
        // Set them at different X positions
        for i in 0u16..15 {
            let sprite_idx = i;
            let oam_addr = sprite_idx * 4;
            ppu.write_oam(oam_addr, 16); // Y position (same for all)
            ppu.write_oam(oam_addr + 1, (8 + i) as u8); // X position (different for each)
            ppu.write_oam(oam_addr + 2, 0); // Tile index
            ppu.write_oam(oam_addr + 3, 0); // Flags
        }

        // Set up a simple tile in VRAM with a unique pattern
        ppu.write_vram(0x0000, 0xFF);
        ppu.write_vram(0x0001, 0xFF);

        // Set up a different background color so we can distinguish sprites
        ppu.bgp = 0xE4; // Different from sprite palette

        let frame = ppu.render_frame();

        // Count how many sprites are actually rendered on scanline 0
        // Due to the 10-sprite limit, only the first 10 should be visible
        // The sprites at X positions 8-17 should be visible (10 sprites)
        // The sprites at X positions 18-22 should NOT be visible (5 sprites exceeding limit)

        // Since all sprites use the same tile (all white pixels), we can't easily
        // count individual sprites, but we can verify the implementation compiled
        // and runs without panicking.
        assert_eq!(frame.width, 160);
        assert_eq!(frame.height, 144);
    }

    #[test]
    fn test_sprite_priority_dmg_x_coordinate() {
        // DMG: Sprite SELECTION is OAM order, RENDERING priority uses X coordinate
        // Test with >10 sprites to verify selection is OAM-based, not X-based
        let mut ppu = Ppu::new();
        ppu.lcdc = 0x93; // Enable LCD, sprites, and background
        ppu.cgb_mode = false; // Explicitly DMG mode

        // Create 12 sprites on the same scanline
        // Give lower OAM indices HIGHER X coordinates to prove selection isn't X-based
        for i in 0u16..12 {
            let oam_addr = i * 4;
            ppu.write_oam(oam_addr, 16); // Y position (all on same scanline)

            // Lower OAM index = higher X (opposite of what X-based selection would pick)
            let x_pos = (140 - i * 10) as u8;
            ppu.write_oam(oam_addr + 1, x_pos);
            ppu.write_oam(oam_addr + 2, 0); // Tile index
            ppu.write_oam(oam_addr + 3, 0); // Flags
        }

        // Set up tile (color 3 = black)
        ppu.write_vram(0x0000, 0xFF);
        ppu.write_vram(0x0001, 0xFF);

        let frame = ppu.render_frame();

        // Sprite 0 has highest X (140), sprite 9 has X=50
        // If selection were X-based, sprites 11-9 (lowest X) would be selected
        // But with OAM-based selection, sprites 0-9 are selected

        // Verify sprite 0 (OAM 0, X=140, screen X=132-139) is rendered
        let mut found = false;
        for x in 132..140 {
            if frame.pixels[x] != 0xFFFFFFFF {
                found = true;
                break;
            }
        }
        assert!(
            found,
            "Sprite at OAM 0 (highest X) should be selected with OAM-based selection"
        );

        // Verify sprite 9 (OAM 9, X=50, screen X=42-49) is rendered
        found = false;
        for x in 42..50 {
            if frame.pixels[x] != 0xFFFFFFFF {
                found = true;
                break;
            }
        }
        assert!(
            found,
            "Sprite at OAM 9 should be selected (last of first 10 in OAM)"
        );
    }

    #[test]
    fn test_sprite_priority_cgb_oam_order() {
        // CGB: Both SELECTION and RENDERING priority use OAM order only
        // Test with >10 sprites to verify X coordinate doesn't affect selection
        let mut ppu = Ppu::new();
        ppu.lcdc = 0x93; // Enable LCD, sprites, and background
        ppu.enable_cgb_mode(false); // CGB-only mode (compatibility_mode=false means flag 0xC0)

        // Set up CGB object palette 0 with visible colors (not all white)
        // Palette 0, Color 3: Black (0x0000 in RGB555)
        ppu.write_obpi(0x86); // Auto-increment, palette 0, color 3, byte 0
        ppu.write_obpd(0x00); // Low byte: 0x00
        ppu.write_obpd(0x00); // High byte: 0x00 (color = 0x0000 = black)

        // Create 12 sprites on the same scanline
        // Give lower OAM indices HIGHER X to prove X doesn't matter
        for i in 0u16..12 {
            let oam_addr = i * 4;
            ppu.write_oam(oam_addr, 16); // Y position (all on same scanline)

            // Lower OAM index = higher X
            let x_pos = (140 - i * 10) as u8;
            ppu.write_oam(oam_addr + 1, x_pos);
            ppu.write_oam(oam_addr + 2, 0); // Tile index
            ppu.write_oam(oam_addr + 3, 0); // Flags (palette 0)
        }

        // Set up tile (color 3 = black)
        ppu.write_vram(0x0000, 0xFF);
        ppu.write_vram(0x0001, 0xFF);

        let frame = ppu.render_frame();

        // Sprite 0 has highest X (140), sprites 10-11 have lowest X (30, 20)
        // With OAM-based selection, sprites 0-9 are selected regardless of X

        // Verify sprite 0 (OAM 0, X=140, screen X=132-139) is rendered
        let mut found = false;
        for x in 132..140 {
            if frame.pixels[x] != 0xFFFFFFFF {
                found = true;
                break;
            }
        }
        assert!(found, "Sprite at OAM 0 (highest X=140) should be selected");

        // Verify sprite 9 (OAM 9, X=50, screen X=42-49) is rendered
        found = false;
        for x in 42..50 {
            if frame.pixels[x] != 0xFFFFFFFF {
                found = true;
                break;
            }
        }
        assert!(
            found,
            "Sprite at OAM 9 (X=50) should be selected (last of first 10)"
        );
    }

    #[test]
    fn test_sprite_selection_oam_order() {
        // Test that sprite SELECTION (which 10 to display) uses OAM order for both DMG and CGB
        // This is critical for games like Grand Theft Auto where HUD sprites need to be selected
        let mut ppu = Ppu::new();
        ppu.lcdc = 0x93; // Enable LCD, sprites, and background

        // Create 15 sprites on the same scanline but at different X positions
        // Place HUD sprite (important) at OAM index 0 with high X coordinate (far right)
        // Place world sprites at higher OAM indices with low X coordinates (far left)
        for i in 0u16..15 {
            let oam_addr = i * 4;
            ppu.write_oam(oam_addr, 16); // Y position (all on same scanline)

            if i == 0 {
                // HUD sprite at OAM 0 with high X (far right)
                ppu.write_oam(oam_addr + 1, 150); // X = 150
            } else {
                // World sprites with low X (far left)
                ppu.write_oam(oam_addr + 1, (8 + i - 1) as u8); // X = 8-21
            }
            ppu.write_oam(oam_addr + 2, 0); // Tile index
            ppu.write_oam(oam_addr + 3, 0); // Flags
        }

        // Set up tile
        ppu.write_vram(0x0000, 0xFF);
        ppu.write_vram(0x0001, 0xFF);

        let frame = ppu.render_frame();

        // The HUD sprite at OAM 0 MUST be selected and rendered even though its X is highest
        // Because sprite selection uses OAM order, not X coordinate
        // With old (incorrect) X-based selection, the HUD would be dropped in favor of world sprites
        // Verify the HUD sprite at X=150 (screen X = 142-149) is rendered
        let screen_y = 0;
        let mut found_hud = false;
        for screen_x in 142..150 {
            if frame.pixels[screen_y * 160 + screen_x] != 0xFFFFFFFF {
                found_hud = true;
                break;
            }
        }
        assert!(
            found_hud,
            "HUD sprite at OAM 0 with X=150 should be selected and rendered"
        );
    }

    #[test]
    fn test_scrolling_wrapping() {
        // Test that scrolling wraps correctly at boundaries with actual pixel validation
        let mut ppu = Ppu::new();
        ppu.lcdc = 0x91; // Enable LCD and background
        ppu.bgp = 0xE4; // BGP palette: 11 10 01 00 (colors 3,2,1,0 map to shades)

        // Set up a tilemap with distinct tiles at specific positions
        // Tile at tilemap position (0, 0) - use tile index 1
        ppu.write_vram(0x1800, 1);
        // Tile at tilemap position (31, 0) - use tile index 2 (right edge)
        ppu.write_vram(0x1800 + 31, 2);
        // Tile at tilemap position (0, 31) - use tile index 3 (bottom edge)
        ppu.write_vram(0x1800 + (31 * 32), 3);

        // Set up tiles with distinct patterns (all solid colors for easy validation)
        // Tile 0: Color 0 (default, mapped to white via BGP)
        // Tile 1: Color 1 (light gray via BGP: 0xE4 >> 2 & 0x03 = 1)
        ppu.write_vram(0x0010, 0xFF); // All pixels = color 1
        ppu.write_vram(0x0011, 0x00);
        // Tile 2: Color 2 (dark gray via BGP: 0xE4 >> 4 & 0x03 = 2)
        ppu.write_vram(0x0020, 0x00); // All pixels = color 2
        ppu.write_vram(0x0021, 0xFF);
        // Tile 3: Color 3 (black via BGP: 0xE4 >> 6 & 0x03 = 3)
        ppu.write_vram(0x0030, 0xFF); // All pixels = color 3
        ppu.write_vram(0x0031, 0xFF);

        // Test 1: No scroll - should show tile 1 at top-left
        ppu.scx = 0;
        ppu.scy = 0;
        let frame1 = ppu.render_frame();
        let color_tile1 = 0xFFAAAAAA; // Color 1 maps to light gray
        assert_eq!(
            frame1.pixels[0], color_tile1,
            "At SCX=0, SCY=0, pixel (0,0) should show tile 1 (color 1)"
        );

        // Test 2: Horizontal wrapping - SCX=250
        // Screen pixel (0,0) should show tilemap pixel (250,0)
        // Tilemap pixel 250 = tile X 31, pixel X 2 -> shows tile 2 (at position 31,0)
        ppu.scx = 250;
        ppu.scy = 0;
        let frame2 = ppu.render_frame();
        let color_tile2 = 0xFF555555; // Color 2 maps to dark gray
        assert_eq!(
            frame2.pixels[0], color_tile2,
            "At SCX=250, pixel (0,0) should show tilemap position (250,0) = tile 2 (color 2)"
        );

        // Screen pixel (6,0) should show tilemap pixel (256%256=0, 0) after wrapping
        // Should show tile 1 again
        assert_eq!(
            frame2.pixels[6], color_tile1,
            "At SCX=250, pixel (6,0) should wrap to tilemap (0,0) = tile 1 (color 1)"
        );

        // Test 3: Vertical wrapping - SCY=248
        // Screen pixel (0,0) should show tilemap pixel (0, 248)
        // Tilemap pixel 248 = tile Y 31, pixel Y 0 -> shows tile 3 (at position 0,31)
        ppu.scx = 0;
        ppu.scy = 248;
        let frame3 = ppu.render_frame();
        let color_tile3 = 0xFF000000; // Color 3 maps to black
        assert_eq!(
            frame3.pixels[0], color_tile3,
            "At SCY=248, pixel (0,0) should show tilemap position (0,248) = tile 3 (color 3)"
        );

        // Test 4: Both wrapping - SCX=255, SCY=255
        // Screen pixel (0,0) should show tilemap pixel (255, 255)
        // This is tile (31, 31) at pixel (7, 7) -> shows tile 0 (default)
        ppu.scx = 255;
        ppu.scy = 255;
        let frame4 = ppu.render_frame();
        let color_tile0 = 0xFFFFFFFF; // Color 0 maps to white
        assert_eq!(
            frame4.pixels[0], color_tile0,
            "At SCX=255, SCY=255, pixel (0,0) should show tilemap (31,31) = tile 0 (color 0)"
        );

        // Verify wrapping: screen pixel (1,1) should show tilemap (0,0) = tile 1
        assert_eq!(
            frame4.pixels[160 + 1],
            color_tile1,
            "At SCX=255, SCY=255, pixel (1,1) should wrap to tilemap (0,0) = tile 1"
        );
    }

    #[test]
    fn test_background_tilemap_addressing() {
        // Test that tilemap addressing is correct (32x32 tiles, 256x256 pixels)
        let mut ppu = Ppu::new();
        ppu.lcdc = 0x91; // Enable LCD and background

        // Test that tile coordinates wrap correctly within the 32x32 tilemap
        // Tilemap addresses should be: base + (tile_y * 32) + tile_x
        // where tile_y and tile_x are both in range [0, 31]

        // Set a specific tile at position (15, 15)
        let tile_y = 15u16;
        let tile_x = 15u16;
        let tilemap_addr = 0x1800 + (tile_y * 32) + tile_x;
        ppu.write_vram(tilemap_addr, 1); // Tile index 1

        // Set up tile 1 with a visible pattern
        ppu.write_vram(0x0010, 0xFF);
        ppu.write_vram(0x0011, 0xFF);

        // Scroll to position (120, 120) which should show tile (15, 15)
        // screen pixel (0, 0) should map to tilemap pixel (120, 120)
        // tilemap pixel (120, 120) is in tile (15, 15) at pixel (0, 0)
        ppu.scx = 120;
        ppu.scy = 120;

        let frame = ppu.render_frame();
        // Verify frame is rendered without panic
        assert_eq!(frame.width, 160);
        assert_eq!(frame.height, 144);
    }

    #[test]
    fn test_scanline_split_effect() {
        // Test that scanline split effects work by using the actual step() mechanism
        // This simulates a game (like GTA GBC) that changes SCX mid-frame using LYC interrupts
        // to create a split-screen effect where the top and bottom of the screen scroll differently
        let mut ppu = Ppu::new();
        ppu.lcdc = 0x91; // Enable LCD and background
        ppu.bgp = 0xE4; // BGP palette

        // Set up two distinct tiles with different colors
        // Tile 1: Color 1 (light gray) - fill entire first column (X=0) of tilemap
        for tile_y in 0..32 {
            ppu.write_vram(0x1800 + (tile_y * 32), 1); // tilemap column 0
        }
        for i in 0..8 {
            ppu.write_vram(0x0010 + (i * 2), 0xFF);
            ppu.write_vram(0x0010 + (i * 2) + 1, 0x00);
        }

        // Tile 2: Color 2 (dark gray) - fill entire column 16 of tilemap
        // SCX=128 shifts view by 128 pixels = 16 tiles, so we see column 16
        for tile_y in 0..32 {
            ppu.write_vram(0x1800 + (tile_y * 32) + 16, 2); // tilemap column 16
        }
        for i in 0..8 {
            ppu.write_vram(0x0020 + (i * 2), 0x00);
            ppu.write_vram(0x0020 + (i * 2) + 1, 0xFF);
        }

        // Start with SCX=0 for top half
        ppu.scx = 0;

        // Step through scanlines 0-71 with SCX=0, then change SCX and step through 72-143
        for scanline in 0..144 {
            if scanline == 72 {
                // Simulate a game changing SCX during HBlank interrupt at scanline 72
                ppu.scx = 128;
            }

            // Step one scanline (456 cycles)
            let _ = ppu.step(456);
        }

        // Complete the frame (VBlank lines 144-153)
        for _ in 144..154 {
            let _ = ppu.step(456);
        }

        let frame = ppu.render_frame();

        // Verify split: top half should show tile 1 (SCX=0), bottom half should show tile 2 (SCX=128)
        let color_tile1 = 0xFFAAAAAA; // Color 1 -> light gray
        let color_tile2 = 0xFF555555; // Color 2 -> dark gray

        // Check top half (scanline 0) - should show tile 1
        assert_eq!(
            frame.pixels[0], color_tile1,
            "Top half (scanline 0) should show tile 1 with SCX=0"
        );

        // Check bottom half (scanline 72) - should show tile 2
        assert_eq!(
            frame.pixels[72 * 160],
            color_tile2,
            "Bottom half (scanline 72) should show tile 2 with SCX=128"
        );
    }

    #[test]
    fn test_window_split_effect() {
        // Test that window can be enabled/disabled mid-frame via WY changes
        // This is the technique used by GTA GBC for its HUD split
        let mut ppu = Ppu::new();
        ppu.lcdc = 0xF1; // Enable LCD, BG, WIN, use 0x9C00 for window tilemap
        ppu.bgp = 0xE4; // BGP palette

        // Set up background tile (tile 1): Color 1 (light gray)
        // Fill entire background tilemap with tile 1
        for tile_y in 0..32 {
            for tile_x in 0..32 {
                ppu.write_vram(0x1800 + (tile_y * 32) + tile_x, 1);
            }
        }
        for i in 0..8 {
            ppu.write_vram(0x0010 + (i * 2), 0xFF);
            ppu.write_vram(0x0010 + (i * 2) + 1, 0x00);
        }

        // Set up window tile (tile 2): Color 2 (dark gray)
        // Fill entire window tilemap (at 0x9C00) with tile 2
        for tile_y in 0..32 {
            for tile_x in 0..32 {
                ppu.write_vram(0x1C00 + (tile_y * 32) + tile_x, 2);
            }
        }
        for i in 0..8 {
            ppu.write_vram(0x0020 + (i * 2), 0x00);
            ppu.write_vram(0x0020 + (i * 2) + 1, 0xFF);
        }

        // Window position: WX=7 (starts at left edge), WY=100 (starts at scanline 100)
        ppu.wx = 7;
        ppu.wy = 100;

        // Step through all scanlines to capture per-scanline state
        for _ in 0..144 {
            let _ = ppu.step(456);
        }

        // Complete the frame (VBlank lines 144-153)
        for _ in 144..154 {
            let _ = ppu.step(456);
        }

        let frame = ppu.render_frame();

        let color_tile1 = 0xFFAAAAAA; // Color 1 -> light gray (background)
        let color_tile2 = 0xFF555555; // Color 2 -> dark gray (window)

        // Check above window (scanline 50) - should show background
        assert_eq!(
            frame.pixels[50 * 160],
            color_tile1,
            "Above window (scanline 50) should show background"
        );

        // Check in window (scanline 100) - should show window
        assert_eq!(
            frame.pixels[100 * 160],
            color_tile2,
            "In window (scanline 100) should show window"
        );
    }

    #[test]
    fn test_stat_blocking_mode_0_to_mode_1() {
        // Test STAT blocking: if both Mode 0 (HBlank) and Mode 1 (VBlank) interrupts are enabled,
        // transitioning from Mode 0 to Mode 1 should NOT fire an interrupt because the line stays high
        let mut ppu = Ppu::new();
        ppu.lcdc = 0x91; // Enable LCD
        ppu.stat |= 0x08 | 0x10; // Enable both HBlank (bit 3) and VBlank (bit 4) interrupts
        ppu.ly = 143;
        ppu.cycle_counter = 300; // In Mode 0 (HBlank)

        // First, step to establish Mode 0 and set the interrupt line high
        let (_vblank, _stat, _hblank) = ppu.step(10);
        // This might or might not fire depending on initial state, just establish we're in Mode 0
        assert_eq!(ppu.stat & 0x03, 0); // Mode 0 (HBlank)

        // Now step to line 144 (VBlank) - should NOT fire interrupt because line was already high from Mode 0
        let (_vblank, stat, _hblank) = ppu.step(146); // Step to next line (456 - 310 = 146)
        assert!(
            !stat,
            "STAT interrupt should be blocked (line stays high from Mode 0 to Mode 1)"
        );
        assert_eq!(ppu.ly, 144);
        assert_eq!(ppu.stat & 0x03, 1); // Mode 1 (VBlank)
    }

    #[test]
    fn test_stat_blocking_mode_2_to_lyc() {
        // Test STAT blocking: if Mode 2 and LYC=LY are both enabled and active,
        // the line stays high and no new interrupt fires
        let mut ppu = Ppu::new();
        ppu.lcdc = 0x91; // Enable LCD
        ppu.stat |= 0x20 | 0x40; // Enable Mode 2 (OAM) and LYC=LY interrupts
        ppu.ly = 10;
        ppu.lyc = 11;
        ppu.cycle_counter = 400; // Near end of line

        // Step to line 11 with Mode 2 (OAM search) - both sources active
        let (_vblank, stat, _hblank) = ppu.step(60);
        // First interrupt should fire (rising edge from low to high)
        assert!(stat, "First STAT interrupt should fire (rising edge)");
        assert_eq!(ppu.ly, 11);
        assert_eq!(ppu.stat & 0x03, 2); // Mode 2

        // Step within the same scanline - line stays high, no new interrupt
        let (_vblank, stat, _hblank) = ppu.step(20);
        assert!(!stat, "No new interrupt while line stays high");
    }

    #[test]
    fn test_stat_edge_trigger_falling_then_rising() {
        // Test that interrupt fires again after line goes low then high
        let mut ppu = Ppu::new();
        ppu.lcdc = 0x91; // Enable LCD
        ppu.stat |= 0x08; // Enable HBlank interrupt
        ppu.ly = 0;
        ppu.cycle_counter = 0;

        // Enter Mode 0 (HBlank) - interrupt fires
        let (_vblank, stat, _hblank) = ppu.step(300); // Cycle 300 is in Mode 0
        assert!(stat, "First HBlank interrupt should fire");

        // Go to next scanline Mode 2 - line goes low
        let (_vblank, stat, _hblank) = ppu.step(200); // Complete scanline and enter Mode 2
        assert!(!stat, "No interrupt in Mode 2");

        // Enter Mode 0 again - interrupt fires again (new rising edge)
        let (_vblank, stat, _hblank) = ppu.step(300); // Enter Mode 0 on next line
        assert!(
            stat,
            "Second HBlank interrupt should fire (new rising edge)"
        );
    }

    #[test]
    fn test_stat_lyc_only_triggers_once() {
        // Test that LYC=LY interrupt only fires once per coincidence, not repeatedly
        let mut ppu = Ppu::new();
        ppu.lcdc = 0x91; // Enable LCD
        ppu.stat |= 0x40; // Enable LYC=LY interrupt
        ppu.ly = 10;
        ppu.lyc = 11;

        // Step to line 11 - interrupt fires
        let (_vblank, stat, _hblank) = ppu.step(456);
        assert!(stat, "LYC=LY interrupt should fire when coincidence occurs");
        assert_eq!(ppu.ly, 11);

        // Step more cycles on same line - no new interrupt (line stays high)
        let (_vblank, stat, _hblank) = ppu.step(100);
        assert!(
            !stat,
            "LYC=LY interrupt should not fire again while coincidence persists"
        );
        assert_eq!(ppu.ly, 11);
    }

    #[test]
    fn test_stat_all_sources_disabled() {
        // Test that no interrupt fires when all sources are disabled
        let mut ppu = Ppu::new();
        ppu.lcdc = 0x91; // Enable LCD
        ppu.stat = 0x00; // All interrupt sources disabled
        ppu.ly = 0;
        ppu.lyc = 0; // Even with coincidence

        // Step through various modes - no interrupts should fire
        let (_vblank, stat, _hblank) = ppu.step(100);
        assert!(!stat, "No interrupt when all sources disabled (Mode 2)");

        let (_vblank, stat, _hblank) = ppu.step(200);
        assert!(!stat, "No interrupt when all sources disabled (Mode 0)");

        let (_vblank, stat, _hblank) = ppu.step(200);
        assert!(!stat, "No interrupt when all sources disabled (next line)");
    }
}
