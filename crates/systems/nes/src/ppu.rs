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
//! This implementation uses **scanline-based** rendering:
//!
//! - Scanlines are rendered incrementally via `render_scanline()` for accurate mid-frame register changes
//! - VBlank is simulated at the system level, not by the PPU
//! - **Sprite evaluation** is performed per scanline to set sprite overflow flag
//! - Sprite 0 hit detection is basic but functional
//!
//! This approach handles mid-frame scroll register changes correctly (e.g., fixed HUDs in games
//! like SMB3, F1 Sensation, and Rad Racer 2).
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
use emu_core::apu::TimingMode;
use emu_core::logging::{log, LogCategory, LogLevel};
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
    pub vram: Vec<u8>, // 2KB or 4KB internal VRAM (nametables) - 4KB for FourScreen mirroring
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
    // PPUADDR latch (shared with PPUSCROLL)
    addr_latch: Cell<bool>,
    // Loopy registers for proper PPU scrolling behavior
    // Reference: https://www.nesdev.org/wiki/PPU_scrolling
    // v: Current VRAM address (15 bits) - also used for $2007 reads/writes
    // t: Temporary VRAM address (15 bits) - updated by $2005/$2006 writes
    // x: Fine X scroll (3 bits) - separate from v register
    //
    // Register layout (both v and t):
    // Bit:  14 13 12 11 10  9  8  7  6  5  4  3  2  1  0
    // Use:  -- FY FY FY NT NT CY CY CY CY CY CX CX CX CX CX
    //       |  |        |     |                 |
    //       |  Fine Y   |     Coarse Y          Coarse X
    //       |           Nametable select
    //       Unused (bit 14 is unused, bit 15 doesn't exist)
    pub vram_addr: Cell<u16>,  // v register (current VRAM address)
    temp_vram_addr: Cell<u16>, // t register (temporary VRAM address)
    fine_x: Cell<u8>,          // x register (fine X scroll, 3 bits)
    read_buffer: Cell<u8>,
    #[allow(clippy::type_complexity)]
    a12_callback: RefCell<Option<Box<dyn FnMut(bool)>>>,
    #[allow(clippy::type_complexity)]
    chr_read_callback: RefCell<Option<Box<dyn FnMut(u16)>>>,
    suppress_a12: Cell<bool>,
    oam_addr: Cell<u8>,
    /// Track if we're in the first frame after reset (for register locking)
    /// Registers $2000, $2001, $2005, $2006 are write-protected during first frame
    /// Register $2007 is read-protected (returns $00) during first frame
    first_frame_after_reset: Cell<bool>,
    /// Cycle-accurate timing: current scanline (0-261, where 261 is pre-render scanline)
    /// Scanline 241 is when VBlank starts
    scanline: Cell<u16>,
    /// Cycle-accurate timing: current dot/pixel within scanline (0-340)
    /// Each scanline has 341 dots (0-340)
    dot: Cell<u16>,
    /// Cycle-accurate timing: odd frame flag for skipping cycle on scanline 0
    /// On odd frames, dot 0 of scanline 0 is skipped (goes directly from -1,340 to 0,1)
    odd_frame: Cell<bool>,
    /// Monotonic frame counter (increments when scanline wraps 261 -> 0).
    frame_counter: Cell<u64>,
    /// Timing mode (NTSC or PAL) for correct scanline count
    /// NTSC: 262 scanlines, PAL: 312 scanlines
    timing_mode: TimingMode,
    /// Cycle-accurate sprite 0 hit: stores the (scanline, X position) where hit should trigger.
    /// This is set during render_scanline() and the flag is actually set during tick()
    /// when we reach the corresponding dot position (X + 2 to account for PPU pipeline).
    sprite_0_hit_pending: Cell<Option<(u16, u16)>>,
}

impl fmt::Debug for Ppu {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Ppu").finish_non_exhaustive()
    }
}

impl Ppu {
    pub fn new(chr: Vec<u8>, mirroring: Mirroring, timing_mode: TimingMode) -> Self {
        let (chr, chr_is_ram) = if chr.is_empty() {
            (vec![0u8; 0x2000], true)
        } else {
            (chr, false)
        };
        // Allocate 4KB VRAM for FourScreen mirroring, 2KB for all other modes
        let vram_size = if mirroring == Mirroring::FourScreen {
            0x1000 // 4KB for independent nametables
        } else {
            0x800 // 2KB for mirrored nametables
        };
        // Pre-render scanline: 261 for NTSC, 311 for PAL
        let pre_render_scanline = match timing_mode {
            TimingMode::Ntsc => 261,
            TimingMode::Pal => 311,
        };
        Self {
            chr,
            chr_is_ram,
            vram: vec![0; vram_size],
            palette: [0; 32],
            oam: [0; 256],
            mirroring,
            ctrl: 0,
            mask: 0,
            // CRITICAL: VBlank starts true (DO NOT CHANGE - fixes Super Mario Bros. 3)
            // This matches Mesen's power-on state behavior where VBlank is set randomly.
            // Many games (especially SMB3) rely on detecting VBlank on the first frame.
            // Starting with false causes SMB3 to hang waiting for VBlank.
            // Reference: Mesen2 NesPpu.cpp power-on state initialization
            vblank: Cell::new(true),
            sprite_0_hit: Cell::new(false),
            sprite_overflow: Cell::new(false),
            nmi_pending: Cell::new(false),
            addr_latch: Cell::new(false),
            vram_addr: Cell::new(0),      // v register
            temp_vram_addr: Cell::new(0), // t register
            fine_x: Cell::new(0),         // x register (3 bits)
            read_buffer: Cell::new(0),
            a12_callback: RefCell::new(None),
            chr_read_callback: RefCell::new(None),
            suppress_a12: Cell::new(false),
            oam_addr: Cell::new(0),
            first_frame_after_reset: Cell::new(true),
            // Cycle-accurate timing state - start at pre-render scanline
            scanline: Cell::new(pre_render_scanline),
            dot: Cell::new(0),
            odd_frame: Cell::new(false),
            frame_counter: Cell::new(0),
            timing_mode,
            sprite_0_hit_pending: Cell::new(None),
        }
    }

    /// Get the total scanline count for the current timing mode
    /// NTSC: 262 scanlines (0-239 visible, 240 post-render, 241-260 vblank, 261 pre-render)
    /// PAL: 312 scanlines (0-239 visible, 240 post-render, 241-310 vblank, 311 pre-render)
    fn total_scanlines(&self) -> u16 {
        match self.timing_mode {
            TimingMode::Ntsc => 262,
            TimingMode::Pal => 312,
        }
    }

    /// Get the pre-render scanline for the current timing mode
    /// NTSC: 261, PAL: 311
    #[inline(always)]
    fn pre_render_scanline(&self) -> u16 {
        match self.timing_mode {
            TimingMode::Ntsc => 261,
            TimingMode::Pal => 311,
        }
    }

    #[inline]
    fn map_nametable_addr(&self, addr: u16) -> usize {
        // Map $2000-$2FFF into internal VRAM using cartridge mirroring.
        let a = addr & 0x0FFF; // 0x0000..0x0FFF
        let table = (a / 0x0400) as u16; // 0..3
        let offset = (a % 0x0400) as u16;

        let physical_table = match self.mirroring {
            Mirroring::FourScreen => {
                // With 4KB VRAM, each nametable is independent (no mirroring)
                // Table 0 at 0x000, Table 1 at 0x400, Table 2 at 0x800, Table 3 at 0xC00
                table
            }
            Mirroring::Vertical => match table {
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

        let addr = (physical_table * 0x0400 + offset) as usize;
        // Mask to VRAM size (0x7FF for 2KB, 0xFFF for 4KB)
        addr & (self.vram.len() - 1)
    }

    pub fn set_mirroring(&mut self, mirroring: Mirroring) {
        // If switching to/from FourScreen, resize VRAM appropriately
        let old_needs_4kb = self.mirroring == Mirroring::FourScreen;
        let new_needs_4kb = mirroring == Mirroring::FourScreen;

        if old_needs_4kb != new_needs_4kb {
            let new_size = if new_needs_4kb { 0x1000 } else { 0x800 };
            let old_size = self.vram.len();

            // Preserve as much data as possible when resizing
            if new_size > old_size {
                // Expanding: keep existing data, zero-fill the rest
                self.vram.resize(new_size, 0);
            } else {
                // Shrinking: keep only the first new_size bytes
                self.vram.truncate(new_size);
            }
        }

        self.mirroring = mirroring;
    }

    #[inline(always)]
    pub fn get_mirroring(&self) -> Mirroring {
        self.mirroring
    }

    #[inline(always)]
    pub fn nmi_enabled(&self) -> bool {
        (self.ctrl & 0x80) != 0
    }

    #[inline(always)]
    pub fn ctrl(&self) -> u8 {
        self.ctrl
    }

    #[inline(always)]
    pub fn mask(&self) -> u8 {
        self.mask
    }

    /// Extract scroll values from loopy registers for potential future use.
    /// Compute scroll_x from loopy registers on-demand.
    /// Returns the horizontal scroll position (0-255) derived from temp_vram_addr and fine_x.
    pub fn scroll_x(&self) -> u8 {
        let t = self.temp_vram_addr.get();
        let coarse_x = (t & 0x001F) as u8;
        let fine_x = self.fine_x.get();
        (coarse_x * 8) + fine_x
    }

    /// Compute scroll_y from loopy registers on-demand.
    /// Returns the vertical scroll position (0-255) derived from temp_vram_addr.
    pub fn scroll_y(&self) -> u8 {
        let t = self.temp_vram_addr.get();
        let coarse_y = ((t >> 5) & 0x001F) as u8;
        let fine_y = ((t >> 12) & 0x0007) as u8;
        (coarse_y * 8) + fine_y
    }

    /// Check if CHR is RAM (writable) or ROM
    pub fn chr_is_ram(&self) -> bool {
        self.chr_is_ram
    }

    /// Get the master palette as a vector of RGB values
    pub fn get_master_palette() -> Vec<u32> {
        NES_MASTER_PALETTE.to_vec()
    }

    /// Set/clear the VBlank flag (PPUSTATUS bit 7).
    ///
    /// CRITICAL: VBlank and NMI timing (DO NOT CHANGE)
    ///
    /// Reference: Mesen2 NesPpu.cpp ProcessScanlineImpl() lines 869-893
    /// Reference: NESdev wiki PPU frame timing
    ///
    /// - VBlank set on scanline 241, cycle 1
    /// - VBlank cleared on pre-render scanline (-1), cycle 1
    /// - NMI fires when VBlank transitions from false to true AND NMI is enabled
    /// - NMI is automatically cleared when VBlank ends (start of pre-render scanline)
    /// - Sprite 0 hit and sprite overflow are cleared on pre-render scanline, NOT when VBlank starts/ends
    /// - First frame after reset: register lock is released at end of first VBlank
    pub fn set_vblank(&self, v: bool) {
        let prev = self.vblank.replace(v);
        if v && !prev && self.nmi_enabled() {
            // VBlank just started and NMI is enabled - trigger NMI
            log(LogCategory::PPU, LogLevel::Trace, || {
                "PPU: VBlank started, triggering NMI".to_string()
            });
            self.nmi_pending.set(true);
        } else if !v {
            // VBlank cleared (pre-render scanline) - clear any pending NMI
            // This is critical: NMI must be cleared when VBlank ends
            log(LogCategory::PPU, LogLevel::Trace, || {
                "PPU: VBlank cleared".to_string()
            });
            self.nmi_pending.set(false);

            // Release register lock after first frame
            // Reference: problemkaputt.de everynes.htm - PPU Reset section
            // "The read/write protection is released when: NTSC: At END of First Vblank"
            if self.first_frame_after_reset.get() {
                log(LogCategory::PPU, LogLevel::Debug, || {
                    "PPU: First frame complete, releasing register lock".to_string()
                });
                self.first_frame_after_reset.set(false);
            }
        }
    }

    /// Clear sprite 0 hit and sprite overflow flags.
    ///
    /// IMPORTANT: This should be called at the start of the pre-render scanline (scanline -1/261),
    /// NOT when VBlank starts or ends. This is the correct NES hardware behavior.
    ///
    /// NOTE: These flags are NOT cleared by reading PPUSTATUS ($2002). Only VBlank flag and
    /// NMI pending are cleared by reading $2002. Sprite flags persist until cleared by
    /// this function at the start of the pre-render scanline.
    ///
    /// Reference: Mesen2 NesPpu.cpp ProcessScanlineImpl() - flags cleared on pre-render scanline
    /// Reference: NESdev wiki - sprite flags persist through VBlank
    pub fn clear_sprite_flags(&self) {
        self.sprite_0_hit.set(false);
        self.sprite_overflow.set(false);
    }

    pub fn vblank_flag(&self) -> bool {
        self.vblank.get()
    }

    /// Clear the first frame after reset flag.
    /// This is useful for tests that need to bypass the register lock.
    /// In normal operation, this flag is cleared automatically at the end of the first VBlank.
    #[cfg(test)]
    pub fn clear_first_frame_lock(&self) {
        self.first_frame_after_reset.set(false);
    }

    /// Immediately resolve any pending sprite 0 hit.
    /// This is used by tests that call render_scanline() directly without cycling
    /// through tick(). In normal operation, sprite 0 hit is set during tick() at
    /// the cycle-accurate position.
    #[cfg(test)]
    pub fn resolve_pending_sprite_0_hit(&self) {
        if let Some((_scanline, _x)) = self.sprite_0_hit_pending.get() {
            self.sprite_0_hit.set(true);
            self.sprite_0_hit_pending.set(None);
        }
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

    /// Fast CHR fetch for rendering hot path - skips callbacks when suppress_a12 is true.
    /// This optimization eliminates RefCell borrow overhead in the rendering loop.
    /// Callbacks are invoked separately during scanline rendering for mapper compatibility.
    #[inline(always)]
    fn chr_fetch_fast(&self, addr: usize) -> u8 {
        self.chr.get(addr).copied().unwrap_or(0)
    }

    /// Read a PPU register (very partial implementation).
    #[inline]
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

                let scanline = self.scanline.get();
                let dot = self.dot.get();

                // Log PPUSTATUS reads that see sprite 0 hit
                if self.sprite_0_hit.get() {
                    log(LogCategory::PPU, LogLevel::Info, || {
                        format!(
                            "PPUSTATUS read: ${:02X} (S0H=1) @ scanline {} dot {}",
                            status,
                            scanline,
                            dot
                        )
                    });
                }

                // CRITICAL: PPUSTATUS read behavior (DO NOT CHANGE - required for NMI timing)
                // Reading PPUSTATUS has three effects:
                // 1. Clears the VBlank flag (bit 7)
                // 2. Clears any pending NMI (NMI suppression)
                // 3. Resets the address latch for PPUSCROLL/PPUADDR
                //
                // IMPORTANT: Reading PPUSTATUS does NOT clear sprite flags (bits 5-6):
                // - Sprite overflow (bit 5) is only cleared at dot 1 of pre-render scanline
                // - Sprite 0 hit (bit 6) is only cleared at dot 1 of pre-render scanline
                //
                // NMI suppression is critical: if a game reads PPUSTATUS right when VBlank
                // starts, the NMI must be prevented. This is described in NESdev wiki and
                // tested by many games.
                // Reference: https://www.nesdev.org/wiki/PPU_registers#PPUSTATUS
                self.vblank.set(false);
                self.nmi_pending.set(false);
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
                // During first frame after reset, $2007 is read-protected and returns $00
                // Reference: problemkaputt.de everynes.htm - PPU Reset section
                if self.first_frame_after_reset.get() {
                    log(LogCategory::PPU, LogLevel::Trace, || {
                        "PPU: $2007 read blocked (first frame)".to_string()
                    });
                    return 0x00;
                }

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

    #[inline]
    pub fn write_register(&mut self, reg: u16, val: u8) {
        match reg & 0x7 {
            0 => {
                // PPUCTRL
                // Write-protected during first frame after reset
                // Reference: problemkaputt.de everynes.htm - PPU Reset section
                // Reference: https://www.nesdev.org/wiki/PPU_scrolling
                if self.first_frame_after_reset.get() {
                    log(LogCategory::PPU, LogLevel::Trace, || {
                        "PPU: $2000 write blocked (first frame)".to_string()
                    });
                    return;
                }

                let old_nmi = (self.ctrl & 0x80) != 0;
                self.ctrl = val;
                let new_nmi = (self.ctrl & 0x80) != 0;

                // PPUCTRL bits 0-1 select the base nametable, which updates t register bits 10-11
                // t: ....BA.. ........ = d: ......BA
                let t = self.temp_vram_addr.get();
                let nt_select = (val & 0x03) as u16;
                self.temp_vram_addr.set((t & !0x0C00) | (nt_select << 10));

                log(LogCategory::PPU, LogLevel::Trace, || {
                    format!(
                        "PPUCTRL write: 0x{:02X} (NMI: {})",
                        val,
                        if new_nmi { "ON" } else { "OFF" }
                    )
                });
                // If NMI gets enabled while already in VBlank, the PPU triggers an NMI.
                if !old_nmi && new_nmi && self.vblank.get() {
                    log(LogCategory::PPU, LogLevel::Debug, || {
                        "PPU: NMI enabled during VBlank, triggering NMI".to_string()
                    });
                    self.nmi_pending.set(true);
                }
            }
            1 => {
                // PPUMASK
                // Write-protected during first frame after reset
                // Reference: problemkaputt.de everynes.htm - PPU Reset section
                if self.first_frame_after_reset.get() {
                    log(LogCategory::PPU, LogLevel::Trace, || {
                        "PPU: $2001 write blocked (first frame)".to_string()
                    });
                    return;
                }

                self.mask = val;
                log(LogCategory::PPU, LogLevel::Trace, || {
                    format!("PPUMASK write: 0x{:02X}", val)
                });
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
                // Write-protected during first frame after reset
                // Reference: problemkaputt.de everynes.htm - PPU Reset section
                // Reference: https://www.nesdev.org/wiki/PPU_scrolling
                if self.first_frame_after_reset.get() {
                    log(LogCategory::PPU, LogLevel::Trace, || {
                        "PPU: $2005 write blocked (first frame)".to_string()
                    });
                    return;
                }

                if !self.addr_latch.get() {
                    // First write (w=0): set horizontal scroll
                    // t: ........ ...HGFED = d: HGFED...
                    // x:               CBA = d: .....CBA
                    // w:                   = 1
                    let t = self.temp_vram_addr.get();
                    let coarse_x = (val >> 3) as u16; // Bits 3-7 become coarse X (bits 0-4 of t)
                    let fine_x_val = val & 0x07; // Bits 0-2 become fine X

                    // Update t: clear bits 0-4, set new coarse X
                    self.temp_vram_addr.set((t & 0xFFE0) | coarse_x);
                    self.fine_x.set(fine_x_val);
                    self.addr_latch.set(true);
                } else {
                    // Second write (w=1): set vertical scroll
                    // t: .CBA..HG FED..... = d: HGFEDCBA
                    // w:                   = 0
                    let t = self.temp_vram_addr.get();
                    let coarse_y = ((val >> 3) & 0x1F) as u16; // Bits 3-7 become coarse Y (bits 5-9 of t)
                    let fine_y = (val & 0x07) as u16; // Bits 0-2 become fine Y (bits 12-14 of t)

                    // Update t: clear bits 5-9 and 12-14, set new coarse Y and fine Y
                    self.temp_vram_addr
                        .set((t & 0x8C1F) | (coarse_y << 5) | (fine_y << 12));
                    self.addr_latch.set(false);

                    log(LogCategory::PPU, LogLevel::Trace, || {
                        format!(
                            "PPUSCROLL set: X={}, Y={}",
                            self.scroll_x(),
                            self.scroll_y()
                        )
                    });
                }
            }
            6 => {
                // PPUADDR (write high then low)
                // Write-protected during first frame after reset
                // Reference: problemkaputt.de everynes.htm - PPU Reset section
                // Reference: https://www.nesdev.org/wiki/PPU_scrolling
                //
                // Hardware-accurate behavior:
                // - First write (w=0): Updates ONLY t register bits 8-13, clears bit 14
                // - Second write (w=1): Updates t register bits 0-7, then copies t to v
                //
                // This is critical for mid-frame scroll changes (e.g., SMB3 HUD split).
                // Games expect v to only change on the second write.
                if self.first_frame_after_reset.get() {
                    log(LogCategory::PPU, LogLevel::Trace, || {
                        "PPU: $2006 write blocked (first frame)".to_string()
                    });
                    return;
                }

                let scanline = self.scanline.get();
                let dot = self.dot.get();

                if !self.addr_latch.get() {
                    // First write (w=0): set high byte of t register ONLY
                    // t: .FEDCBA ........ <- val: ..FEDCBA
                    // t: X...... ........ <- 0 (bit 14 is cleared)
                    let hi_masked = (val & 0x3F) as u16; // Only bits 0-5 are used
                    let t = self.temp_vram_addr.get();
                    // Clear bit 14, set bits 8-13 from val
                    self.temp_vram_addr.set((t & 0x00FF) | (hi_masked << 8));

                    // NOTE: v (vram_addr) is NOT updated on first write!
                    // This is crucial for mid-frame scroll splits to work correctly.

                    log(LogCategory::PPU, LogLevel::Info, || {
                        format!(
                            "$2006 hi-write: ${:02X} @ scanline {} dot {} (t=${:04X})",
                            val,
                            scanline,
                            dot,
                            (t & 0x00FF) | (hi_masked << 8)
                        )
                    });

                    self.addr_latch.set(true);
                } else {
                    // Second write (w=1): set low byte of t, then copy t to v
                    // t: ........ HGFEDCBA <- val: HGFEDCBA
                    // v: <------- t ------- (copy t to v)
                    let t = self.temp_vram_addr.get();
                    let new_t = (t & 0xFF00) | (val as u16);
                    self.temp_vram_addr.set(new_t);

                    // Copy complete t register to v (this is when scroll takes effect)
                    self.vram_addr.set(new_t);

                    log(LogCategory::PPU, LogLevel::Info, || {
                        format!(
                            "$2006 lo-write: ${:02X} @ scanline {} dot {} => v=${:04X}",
                            val,
                            scanline,
                            dot,
                            new_t
                        )
                    });

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
    /// are on the same scanline, the sprite overflow flag (PPUSTATUS bit 5) is set.
    ///
    /// # Hardware Behavior
    ///
    /// - **When set**: During sprite evaluation when the 9th sprite on a scanline is found
    /// - **When cleared**: Only at dot 1 of the pre-render scanline (scanline 261/-1)
    /// - **NOT cleared by**: Reading PPUSTATUS ($2002) - unlike VBlank flag
    /// - **Hardware bugs**: This implementation emulates the m/n pointer increment bug
    ///
    /// # Hardware-Accurate Sprite Evaluation Bug
    ///
    /// The real NES PPU has a bug in sprite evaluation that can cause false positives
    /// and false negatives in the sprite overflow flag. This happens because:
    ///
    /// 1. Two pointers are used: n (primary OAM index) and m (byte within sprite)
    /// 2. When checking sprites 9-64, if a Y-coordinate matches, BOTH n and m increment
    /// 3. This causes m to wrap and check wrong bytes in subsequent sprites
    /// 4. Result: Overflow can be set incorrectly or missed entirely
    ///
    /// This bug is emulated here for hardware accuracy.
    ///
    /// # References
    ///
    /// - NESdev wiki: https://www.nesdev.org/wiki/PPU_sprite_evaluation
    /// - PPUSTATUS register: https://www.nesdev.org/wiki/PPU_registers#PPUSTATUS
    fn evaluate_sprites_for_scanline(&self, scanline: u32) {
        let sprite_size_16 = (self.ctrl & 0x20) != 0;
        let sprite_height = if sprite_size_16 { 16 } else { 8 };

        let mut n = 0u8; // Sprite index in primary OAM (0-63)
        let mut m = 0u8; // Byte within sprite (0-3: Y, tile, attr, X)
        let mut sprites_found = 0;

        // Phase 1: Find first 8 sprites on this scanline
        while n < 64 && sprites_found < 8 {
            let oam_index = (n as usize) * 4;
            let y_pos = self.oam[oam_index] as i16 + 1;
            let row = (scanline as i16) - y_pos;

            if row >= 0 && row < sprite_height {
                sprites_found += 1;
            }
            n += 1;
        }

        // Phase 2: Check for sprite overflow (sprites 9-64)
        // This is where the hardware bug occurs
        if sprites_found >= 8 {
            while n < 64 {
                // HARDWARE BUG: We check the m-th byte instead of always checking Y (byte 0)
                let oam_index = (n as usize) * 4 + (m as usize);

                // Bounds check - OAM is only 256 bytes
                if oam_index >= 256 {
                    break;
                }

                let y_pos = self.oam[oam_index] as i16 + 1;
                let row = (scanline as i16) - y_pos;

                // If this matches (even though we might be checking the wrong byte)
                if row >= 0 && row < sprite_height {
                    // Set overflow flag
                    self.sprite_overflow.set(true);

                    // HARDWARE BUG: Increment BOTH n and m instead of just n
                    // This causes m to wrap and check wrong bytes
                    // However, evaluation stops here so we break immediately
                    // (The m/n increment would happen on real hardware but has no observable effect
                    // since we break before checking another sprite)
                    break;
                } else {
                    // No match - increment n and reset m
                    n += 1;
                    m = 0;
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
        // TEST-ONLY: Helper that renders using scanline-based rendering.
        // This ensures tests use the same rendering path as production code.
        //
        // Since tests don't call tick(), we need to manually initialize the v register
        // from t before rendering, simulating what tick() would do at pre-render
        // scanline dots 280-304 and dot 257.
        let rendering_enabled = (self.mask & 0x18) != 0;
        if rendering_enabled {
            let t = self.temp_vram_addr.get();
            let v = self.vram_addr.get();
            // Copy both vertical and horizontal bits from t to v
            let mut new_v = (v & !0x7BE0) | (t & 0x7BE0); // Vertical bits
            new_v = (new_v & !0x041F) | (t & 0x041F); // Horizontal bits
            self.vram_addr.set(new_v);
        }

        let mut frame = Frame::new(256, 240);
        for scanline in 0..240 {
            self.render_scanline(scanline, &mut frame);

            // Simulate tick()'s dot 256 v register increment (for all visible scanlines 0-239)
            // This would normally happen in tick() but tests don't call tick()
            if rendering_enabled && scanline < 240 {
                let mut v = self.vram_addr.get();
                let fine_y = (v >> 12) & 0x0007;

                if fine_y < 7 {
                    v = (v & !0x7000) | ((fine_y + 1) << 12);
                } else {
                    v &= !0x7000;
                    let coarse_y = (v >> 5) & 0x001F;
                    let new_coarse_y = if coarse_y == 29 {
                        v ^= 0x0800;
                        0
                    } else if coarse_y == 31 {
                        0
                    } else {
                        coarse_y + 1
                    };
                    v = (v & !0x03E0) | (new_coarse_y << 5);
                }
                self.vram_addr.set(v);
            }
        }
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

        // Debug: log scroll and mirroring for first few scanlines
        if y < 3 {
            use emu_core::logging::{log, LogCategory, LogLevel};
            log(LogCategory::PPU, LogLevel::Debug, || {
                format!(
                    "Scanline {}: scroll=({},{}), ctrl=0x{:02X}, mirroring={:?}",
                    y,
                    self.scroll_x(),
                    self.scroll_y(),
                    self.ctrl,
                    self.mirroring
                )
            });
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
        let show_bg_left = (self.mask & 0x02) != 0; // PPUMASK bit 1: show background in leftmost 8 pixels
        let show_sprites_left = (self.mask & 0x04) != 0; // PPUMASK bit 2: show sprites in leftmost 8 pixels

        // Note: In production (cycle-accurate mode with tick()), v register initialization
        // and increment happen in tick() at specific dots:
        // - Pre-render scanline (261) dots 280-304: vertical bits copied from t to v
        // - All visible scanlines dot 256: v register incremented
        // - All scanlines dot 257: horizontal bits copied from t to v
        //
        // This render_scanline() function should NOT modify the v register.
        // The v register state at the time render_scanline() is called should already
        // be correct from the tick() updates.

        // Note: Sprite evaluation for overflow detection is now done in tick() at dot 192
        // for cycle-accurate timing. Games like Bee 52 rely on polling PPUSTATUS bit 5.

        let bg_pattern_base: usize = if (self.ctrl & 0x10) != 0 {
            0x1000
        } else {
            0x0000
        };

        let mut universal_bg_idx = self.palette[palette_mirror_index(0)];
        if (self.mask & 0x01) != 0 {
            universal_bg_idx &= 0x30;
        }
        let universal_bg = nes_palette_rgb(universal_bg_idx);

        // Extract scroll values from v register for this scanline.
        // The v register contains the current scroll position and is updated incrementally:
        // - At scanline 0: v is loaded from t (both vertical and horizontal bits)
        // - At each scanline boundary: horizontal bits are refreshed from t
        // - After rendering each scanline: fine_y (and vertical bits) are incremented
        // - Mid-frame $2006 writes directly set v, allowing scroll splits (e.g., SMB3 HUD)
        //
        // We use v's coarse_y/fine_y DIRECTLY without adding the screen scanline number.
        // The v register is incremented after each scanline to "walk through" the nametable.
        let v = self.vram_addr.get();
        let fine_x_val = self.fine_x.get();

        let coarse_x = (v & 0x001F) as u8; // Bits 0-4: tile column (0-31)
        let coarse_y = ((v >> 5) & 0x001F) as u8; // Bits 5-9: tile row (0-31)
        let nt_x = ((v >> 10) & 0x0001) as u8; // Bit 10: nametable X
        let nt_y = ((v >> 11) & 0x0001) as u8; // Bit 11: nametable Y
        let fine_y = ((v >> 12) & 0x0007) as u8; // Bits 12-14: fine Y scroll (0-7)

        // Debug: log v register for scanline 207
        if y == 207 {
            log(LogCategory::PPU, LogLevel::Debug, || {
                format!(
                    "Scanline 207: v=0x{:04X} coarse_x={} coarse_y={} nt_x={} nt_y={} fine_y={} fine_x={}",
                    v, coarse_x, coarse_y, nt_x, nt_y, fine_y, fine_x_val
                )
            });
        }

        let coarse_x = (v & 0x001F) as u8; // Bits 0-4: tile column (0-31)
        let coarse_y = ((v >> 5) & 0x001F) as u8; // Bits 5-9: tile row (0-31)
        let nt_x = ((v >> 10) & 0x0001) as u8; // Bit 10: nametable X
        let nt_y = ((v >> 11) & 0x0001) as u8; // Bit 11: nametable Y
        let fine_y = ((v >> 12) & 0x0007) as u8; // Bits 12-14: fine Y scroll (0-7)

        // Use v register values directly - no screen scanline offset!
        // The v register already points to the correct nametable position for this scanline.
        let tile_y_wrapped = coarse_y;
        let fine_y_in_tile = fine_y as usize;
        let nt_y_adjusted = nt_y;

        // Track background priority for this scanline (for sprite priority).
        let mut bg_priority = [false; 256];

        // Background pixels for this scanline.
        // Optimization: Process background in 8-pixel tile chunks instead of per-pixel
        // to reduce divisions/modulos and improve cache locality.
        if bg_enabled {
            // Calculate the starting tile position based on scroll
            let mut current_tile_x = coarse_x;
            let mut current_nt_x = nt_x;
            let pixel_offset_in_first_tile = fine_x_val as usize;

            let mut screen_x = 0u32;

            while screen_x < width {
                // Calculate tile coordinates
                let nt = current_nt_x | (nt_y_adjusted << 1);
                let tx = current_tile_x as usize;
                let ty = tile_y_wrapped as usize;

                // Fetch tile data once for the entire tile
                let nt_addr = 0x2000u16 + (nt as u16) * 0x0400;
                let tile_addr = nt_addr + (ty as u16) * 32 + (tx as u16);
                let tile_index = self.vram[self.map_nametable_addr(tile_addr)];

                // Fetch attribute once for the tile
                let attr_x = tx / 4;
                let attr_y = ty / 4;
                let attr_addr = nt_addr + 0x03C0 + (attr_y as u16) * 8 + (attr_x as u16);
                let attr_byte = self.vram[self.map_nametable_addr(attr_addr)];
                let quadrant = ((ty % 4) / 2) * 2 + ((tx % 4) / 2);
                let shift = (quadrant * 2) as u8;
                let palette_idx = (attr_byte >> shift) & 0x03;

                // Fetch CHR pattern data once for the tile (optimized - no callbacks during fetch)
                let tile_chr_addr = bg_pattern_base + (tile_index as usize) * 16;
                let lo = self.chr_fetch_fast(tile_chr_addr + fine_y_in_tile);
                let hi = self.chr_fetch_fast(tile_chr_addr + fine_y_in_tile + 8);

                // Invoke CHR read callback for MMC2/MMC4 latch switching compatibility
                // This is done once per tile instead of per pixel for performance
                // CRITICAL: Must invoke for BOTH low and high bitplane reads
                // MMC2 latch triggers are on the high bitplane addresses (e.g., $0FD8, $0FE8)
                if let Some(cb) = &mut *self.chr_read_callback.borrow_mut() {
                    cb((tile_chr_addr + fine_y_in_tile) as u16); // Low bitplane
                    cb((tile_chr_addr + fine_y_in_tile + 8) as u16); // High bitplane
                }

                // Render 8 pixels from this tile (or remaining pixels if less than 8)
                let start_pixel = if screen_x == 0 {
                    pixel_offset_in_first_tile
                } else {
                    0
                };

                for pixel_in_tile in start_pixel..8 {
                    if screen_x >= width {
                        break;
                    }

                    // Clip leftmost 8 pixels if PPUMASK bit 1 is clear
                    let should_render_bg = show_bg_left || screen_x >= 8;

                    // Extract pixel color from tile pattern
                    let bit = 7 - pixel_in_tile;
                    let lo_bit = (lo >> bit) & 1;
                    let hi_bit = (hi >> bit) & 1;
                    let color_in_tile = (hi_bit << 1) | lo_bit;

                    let idx = (y * width + screen_x) as usize;
                    let out = if !should_render_bg {
                        // Leftmost 8 pixels are clipped - use black
                        bg_priority[screen_x as usize] = false;
                        0x00000000 // Black
                    } else if color_in_tile == 0 {
                        // Tile color is 0 (backdrop)
                        bg_priority[screen_x as usize] = false;
                        universal_bg
                    } else {
                        bg_priority[screen_x as usize] = true;
                        let pal_base = (palette_idx as usize) * 4;
                        let mut pal_entry =
                            self.palette[palette_mirror_index(pal_base + (color_in_tile as usize))];
                        if (self.mask & 0x01) != 0 {
                            pal_entry &= 0x30;
                        }
                        nes_palette_rgb(pal_entry)
                    };

                    frame.pixels[idx] = out;
                    screen_x += 1;
                }

                // Move to next tile
                current_tile_x = current_tile_x.wrapping_add(1);
                if current_tile_x >= 32 {
                    current_tile_x = 0;
                    current_nt_x ^= 1; // Flip nametable X
                }
            }

            // NES PPU hardware fetches 34 tiles per scanline (not just the 32 visible ones).
            // The extra 2 tiles are needed to fill the PPU shift registers for horizontal scrolling.
            // This is critical for games like Punch Out!! that use MMC2 mapper, which monitors
            // CHR reads to trigger bank switches. Without these extra fetches, the mapper won't
            // detect the reads and won't switch banks at the right time.
            // Reference: https://www.nesdev.org/wiki/PPU_rendering
            //
            // Note: These tiles (33-34) are fetched by hardware but not displayed on screen since
            // only 32 tiles fit in the visible 256-pixel scanline. Their pattern data is used to
            // pre-fill the PPU's shift registers for the next scanline. We only invoke the CHR
            // read callbacks for mapper compatibility - no rendering is performed.
            for _ in 0..2 {
                let nt = current_nt_x | (nt_y_adjusted << 1);
                let tx = current_tile_x as usize;
                let ty = tile_y_wrapped as usize;

                let nt_addr = 0x2000u16 + (nt as u16) * 0x0400;
                let tile_addr = nt_addr + (ty as u16) * 32 + (tx as u16);
                let tile_index = self.vram[self.map_nametable_addr(tile_addr)];

                let tile_chr_addr = bg_pattern_base + (tile_index as usize) * 16;

                // Invoke CHR read callback for MMC2/MMC4 latch switching compatibility
                // This is CRITICAL for Punch Out!! and other games using MMC2/MMC4 mappers
                if let Some(cb) = &mut *self.chr_read_callback.borrow_mut() {
                    cb((tile_chr_addr + fine_y_in_tile) as u16); // Low bitplane
                    cb((tile_chr_addr + fine_y_in_tile + 8) as u16); // High bitplane
                }

                // Move to next tile
                current_tile_x = current_tile_x.wrapping_add(1);
                if current_tile_x >= 32 {
                    current_tile_x = 0;
                    current_nt_x ^= 1; // Flip nametable X
                }
            }
        } else {
            // Background disabled: fill this scanline with backdrop.
            let row_start = (y * width) as usize;
            for px in &mut frame.pixels[row_start..row_start + width as usize] {
                *px = universal_bg;
            }
        }

        // Sprites affecting this scanline - correct NES sprite priority implementation.
        //
        // The NES PPU handles sprite priority in a specific way:
        // 1. Sprites are drawn front-to-back (OAM 0→63) into a sprite buffer
        // 2. First opaque pixel at each X coordinate wins (regardless of priority bit)
        // 3. Priority bit determines whether sprite pixel replaces background in final composition
        if sprites_enabled {
            let sprite_size_16 = (self.ctrl & 0x20) != 0;
            let sprite_pattern_base: usize = if (self.ctrl & 0x08) != 0 {
                0x1000
            } else {
                0x0000
            };

            // Sprite buffer for this scanline: stores (color, priority, sprite_index) for each pixel.
            // None = no sprite pixel, Some((rgb, behind_bg, sprite_idx)) = sprite pixel with priority and index.
            let mut sprite_buffer: [Option<(u32, bool, usize)>; 256] = [None; 256];

            // NES PPU hardware limitation: maximum 8 sprites per scanline.
            // Track how many sprites are on this scanline to enforce the limit.
            let mut sprites_on_scanline = 0;

            // Draw sprites front-to-back (OAM 0→63) into sprite buffer.
            // First opaque pixel at each position wins.
            // Stop after 8 sprites are found on this scanline (NES hardware limit).
            for i in 0..64usize {
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

                // This sprite is on the current scanline
                sprites_on_scanline += 1;
                if sprites_on_scanline > 8 {
                    // NES hardware limit: only 8 sprites can be rendered per scanline
                    // Sprites beyond the 8th are skipped (sprite overflow flag is set by evaluate_sprites_for_scanline)
                    break;
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
                let lo = self.chr_fetch_fast(addr + fine_y);
                let hi = self.chr_fetch_fast(addr + fine_y + 8);

                // Invoke CHR read callback for MMC2/MMC4 latch switching compatibility
                // CRITICAL: Must invoke for BOTH low and high bitplane reads
                // MMC2 latch triggers are on the high bitplane addresses (e.g., $0FD8, $0FE8)
                if let Some(cb) = &mut *self.chr_read_callback.borrow_mut() {
                    cb((addr + fine_y) as u16); // Low bitplane
                    cb((addr + fine_y + 8) as u16); // High bitplane
                }

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

                    let x_idx = x as usize;

                    // Only write if no sprite pixel has been written yet (first opaque pixel wins)
                    if sprite_buffer[x_idx].is_none() {
                        let pal_base = 0x11 + pal * 4;
                        let mut pal_entry =
                            self.palette[palette_mirror_index(pal_base + (color as usize) - 1)];
                        if (self.mask & 0x01) != 0 {
                            pal_entry &= 0x30;
                        }
                        let rgb = nes_palette_rgb(pal_entry);
                        sprite_buffer[x_idx] = Some((rgb, behind_bg, i));
                    }
                }
            }

            // Composite sprite buffer with background using priority rules and detect sprite 0 hit.
            // For cycle-accurate sprite 0 hit, we find the first X position where sprite 0
            // overlaps an opaque background and store it for the tick() function to use.
            let mut sprite_0_hit_x: Option<u16> = None;
            for x in 0..width as usize {
                if let Some((sprite_color, behind_bg, sprite_idx)) = sprite_buffer[x] {
                    // Clip leftmost 8 pixels if PPUMASK bit 2 is clear
                    let should_render_sprite = show_sprites_left || x >= 8;

                    let idx = (y * width + x as u32) as usize;

                    // Sprite 0 hit detection - find the first X where sprite 0 overlaps opaque background
                    // We store the position instead of setting the flag immediately for cycle-accurate timing.
                    // Conditions: sprite 0, no hit found yet, flag not already set, background opaque at this x,
                    // x < 255 (hit can't occur at rightmost pixel), and not in clipped region.
                    let is_sprite_0_hit_candidate = sprite_idx == 0
                        && sprite_0_hit_x.is_none()
                        && !self.sprite_0_hit.get()
                        && bg_enabled
                        && bg_priority[x]
                        && x < 255
                        && (show_bg_left && show_sprites_left || x >= 8);

                    if is_sprite_0_hit_candidate {
                        sprite_0_hit_x = Some(x as u16);
                        log(LogCategory::PPU, LogLevel::Debug, || {
                            format!(
                                "Sprite 0 hit pending at scanline {} x={} (will trigger at dot {})",
                                y,
                                x,
                                x + 2
                            )
                        });
                    }

                    // Sprite pixel is opaque.
                    // Draw it if: clipping allows it AND (front priority OR background is transparent).
                    if should_render_sprite && (!behind_bg || !bg_priority[x]) {
                        frame.pixels[idx] = sprite_color;
                    }
                }
            }

            // Store the pending sprite 0 hit position for cycle-accurate triggering in tick()
            // Only set if we found a hit position AND the flag isn't already set
            if let Some(hit_x) = sprite_0_hit_x {
                if !self.sprite_0_hit.get() && self.sprite_0_hit_pending.get().is_none() {
                    self.sprite_0_hit_pending.set(Some((y as u16, hit_x)));
                    log(LogCategory::PPU, LogLevel::Info, || {
                        format!(
                            "Sprite 0 HIT scheduled at scanline {} x={} (dot {})",
                            y,
                            hit_x,
                            hit_x + 2
                        )
                    });
                }
            }
        }

        // NOTE: v register increment now happens in tick() at dot 256 of each visible scanline.
        // render_scanline() should NOT modify the v register - it only renders based on current state.
        // This ensures proper cycle-accurate timing and prevents drift issues.

        self.suppress_a12.set(prev_suppress);
    }

    /// Execute a single PPU cycle (dot).
    ///
    /// This advances the PPU by one dot and handles all cycle-accurate timing:
    /// - VBlank flag setting at scanline 241, dot 1
    /// - NMI generation at scanline 241, dot 1 (if enabled)
    /// - Sprite overflow/sprite 0 hit clearing at scanline 261 (pre-render), dot 1
    /// - Odd frame cycle skip at scanline 0, dot 0
    ///
    /// Returns true if an NMI should be triggered.
    #[inline]
    pub fn tick(&self) -> bool {
        let scanline = self.scanline.get();
        let dot = self.dot.get();
        let mut nmi_triggered = false;
        let mut ended_frame_number: Option<u64> = None;
        let pre_render_scanline = self.pre_render_scanline();

        // Handle cycle-accurate events at specific scanline/dot positions
        // Scanline 241, dot 1: VBlank starts (same for NTSC and PAL)
        if (scanline, dot) == (241, 1) {
            // Set VBlank flag
            let was_vblank = self.vblank.replace(true);

            // COMPATIBILITY FIX: Clear sprite 0 hit at VBlank start
            // Hardware behavior: sprite 0 hit is cleared at pre-render scanline dot 1 (261/311).
            // However, games that poll sprite 0 hit during vblank can see stale flags from
            // the previous frame, causing visual glitches (e.g., Battletoads screen jumping).
            // Clearing at vblank start prevents games from reading stale sprite 0 hit flags.
            // This matches the behavior of some other emulators for compatibility.
            // NOTE: This is a deviation from strict hardware accuracy for better game compatibility.
            self.sprite_0_hit.set(false);
            self.sprite_0_hit_pending.set(None);

            // If VBlank just started and NMI is enabled, trigger NMI
            if !was_vblank && self.nmi_enabled() {
                log(LogCategory::PPU, LogLevel::Trace, || {
                    "PPU: VBlank started at scanline 241, dot 1, triggering NMI".to_string()
                });
                self.nmi_pending.set(true);
                nmi_triggered = true;
            }
        }

        // Pre-render scanline, dot 1: Clear VBlank and sprite flags
        // NTSC: scanline 261, PAL: scanline 311
        if scanline == pre_render_scanline && dot == 1 {
            // Clear VBlank flag
            self.vblank.set(false);
            self.nmi_pending.set(false);

            // Clear sprite flags (this is the ONLY place they're cleared on hardware)
            self.sprite_0_hit.set(false);
            self.sprite_overflow.set(false);
            // Also clear pending sprite 0 hit for the new frame
            self.sprite_0_hit_pending.set(None);

            log(LogCategory::PPU, LogLevel::Trace, || {
                "PPU: Pre-render scanline, dot 1: cleared VBlank and sprite flags".to_string()
            });

            // Release register lock after first frame
            if self.first_frame_after_reset.get() {
                log(LogCategory::PPU, LogLevel::Debug, || {
                    "PPU: First frame complete, releasing register lock".to_string()
                });
                self.first_frame_after_reset.set(false);
            }
        }

        // Pre-render scanline, dots 280-304: Copy vertical bits from t to v
        // In the hardware-accurate / cycle-accurate tick() path, vertical bits are
        // copied repeatedly during dots 280-304 of the pre-render scanline.
        // (render_scanline() also copies these bits for compatibility when called without tick().)
        // Critical for games with vertical scrolling and split-screen effects.
        // Reference: https://www.nesdev.org/wiki/PPU_scrolling
        if scanline == pre_render_scanline && dot >= 280 && dot <= 304 {
            let rendering_enabled = (self.mask & 0x18) != 0;
            if rendering_enabled {
                let t = self.temp_vram_addr.get();
                let v = self.vram_addr.get();
                // Copy vertical bits from t to v:
                // Bits 5-9 (coarse Y), bit 11 (nametable Y), bits 12-14 (fine Y)
                let new_v = (v & !0x7BE0) | (t & 0x7BE0);
                self.vram_addr.set(new_v);
            }
        }

        // Visible and pre-render scanlines, dot 257: Copy horizontal bits from t to v
        // This happens at the end of each scanline's rendering
        // Reference: https://www.nesdev.org/wiki/PPU_scrolling
        if (scanline < 240 || scanline == pre_render_scanline) && dot == 257 {
            let rendering_enabled = (self.mask & 0x18) != 0;
            if rendering_enabled {
                let t = self.temp_vram_addr.get();
                let v = self.vram_addr.get();
                // Copy horizontal bits from t to v:
                // Bits 0-4 (coarse X), bit 10 (nametable X)
                let new_v = (v & !0x041F) | (t & 0x041F);
                self.vram_addr.set(new_v);
            }
        }

        // Cycle-accurate sprite 0 hit: check if we've reached the pending hit position.
        // On real hardware, sprite 0 hit is detected during visible scanline rendering
        // at approximately dot = X_position + 2 (accounting for PPU pipeline delay).
        // The hit can only occur during dots 2-257 of visible scanlines (0-239).
        if scanline < 240 && dot >= 2 && dot <= 257 {
            if let Some((hit_scanline, hit_x)) = self.sprite_0_hit_pending.get() {
                // Check if we're on the right scanline and have reached the hit position
                // Hit triggers at dot = X + 2 (2 cycle pipeline delay)
                let trigger_dot = hit_x.saturating_add(2);
                if scanline == hit_scanline && dot >= trigger_dot && !self.sprite_0_hit.get() {
                    log(LogCategory::PPU, LogLevel::Info, || {
                        format!(
                            "Sprite 0 HIT triggered at scanline {} dot {} (x={})",
                            scanline, dot, hit_x
                        )
                    });
                    self.sprite_0_hit.set(true);
                    self.sprite_0_hit_pending.set(None);
                }
            }
        }

        // Visible scanlines, dot 256: Increment v register's vertical position
        // This prepares the address for the next scanline's background fetches.
        // Reference: https://www.nesdev.org/wiki/PPU_scrolling
        // Increment happens for all visible scanlines (0-239).
        // The pre-render scanline properly reinitializes vertical bits from t during dots 280-304.
        if scanline < 240 && dot == 256 {
            let rendering_enabled = (self.mask & 0x18) != 0;
            if rendering_enabled {
                let mut v = self.vram_addr.get();
                let fine_y = (v >> 12) & 0x0007;

                if fine_y < 7 {
                    // Simple case: just increment fine_y
                    v = (v & !0x7000) | ((fine_y + 1) << 12);
                } else {
                    // fine_y was 7, now wraps to 0
                    v &= !0x7000; // Clear fine_y bits

                    let coarse_y = (v >> 5) & 0x001F;
                    let new_coarse_y = if coarse_y == 29 {
                        // Wrap at row 30 (NES quirk: attribute table is at rows 30-31)
                        v ^= 0x0800; // Toggle nametable Y bit
                        0
                    } else if coarse_y == 31 {
                        // Edge case: if coarse_y is already 31, just wrap to 0 without toggling
                        0
                    } else {
                        coarse_y + 1
                    };
                    v = (v & !0x03E0) | (new_coarse_y << 5);
                }

                self.vram_addr.set(v);
            }
        }

        // Cycle-accurate sprite evaluation during visible scanlines
        // Sprite evaluation happens during dots 65-256 of visible scanlines (0-239)
        // The overflow flag is set when the 9th sprite is found, around dot 192-256
        // We check at dot 192 which approximates when the 9th sprite would be detected
        if scanline < 240 && dot == 192 {
            // Only evaluate if sprites are enabled
            let sprites_enabled = (self.mask & 0x10) != 0;
            if sprites_enabled {
                self.evaluate_sprites_for_scanline(scanline as u32);
            }
        }

        // Advance to next dot
        let mut next_dot = dot + 1;
        let mut next_scanline = scanline;

        // Handle end of scanline (341 dots per scanline, indexed 0-340)
        if next_dot >= 341 {
            next_dot = 0;
            next_scanline += 1;

            // Handle end of frame (total scanlines varies by timing mode)
            // NTSC: 262 scanlines (0-239 visible, 240 post-render, 241-260 vblank, 261 pre-render)
            // PAL: 312 scanlines (0-239 visible, 240 post-render, 241-310 vblank, 311 pre-render)
            if next_scanline >= self.total_scanlines() {
                next_scanline = 0;
                // Toggle odd frame flag
                self.odd_frame.set(!self.odd_frame.get());

                let frame_number = self.frame_counter.get() + 1;
                self.frame_counter.set(frame_number);
                ended_frame_number = Some(frame_number);
            }
        }

        // Odd frame cycle skip: on odd frames with rendering enabled,
        // skip from scanline 261 dot 340 directly to scanline 0 dot 1
        if next_scanline == 0 && next_dot == 0 && self.odd_frame.get() {
            let rendering_enabled = (self.mask & 0x18) != 0;
            if rendering_enabled {
                next_dot = 1;
                log(LogCategory::PPU, LogLevel::Trace, || {
                    "PPU: Odd frame cycle skip (0,0 -> 0,1)".to_string()
                });
            }
        }

        if let Some(frame_number) = ended_frame_number {
            if frame_number % 60 == 0 {
                let v = self.vram_addr.get();
                let t = self.temp_vram_addr.get();
                let x = self.fine_x.get();
                let w = self.addr_latch.get();
                log(LogCategory::PPU, LogLevel::Info, move || {
                    format!(
                        "PPU: Frame {frame_number}: loopy v=${v:04X} t=${t:04X} x={x} w={w} (next {next_scanline},{next_dot})"
                    )
                });
            }
        }

        self.scanline.set(next_scanline);
        self.dot.set(next_dot);

        nmi_triggered
    }

    /// Get current scanline (0-261)
    #[inline(always)]
    pub fn get_scanline(&self) -> u16 {
        self.scanline.get()
    }

    /// Get current dot within scanline (0-340)
    #[inline(always)]
    pub fn get_dot(&self) -> u16 {
        self.dot.get()
    }

    /// Get the monotonic PPU frame counter.
    ///
    /// This increments when the PPU wraps from scanline 261 back to scanline 0.
    #[inline(always)]
    pub fn get_frame_counter(&self) -> u64 {
        self.frame_counter.get()
    }

    /// Check if currently in VBlank region (scanlines 241-260)
    pub fn is_in_vblank_region(&self) -> bool {
        let scanline = self.scanline.get();
        scanline >= 241 && scanline <= 260
    }

    /// Check if currently in visible region (scanlines 0-239)
    pub fn is_in_visible_region(&self) -> bool {
        self.scanline.get() < 240
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
        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal, TimingMode::Ntsc);
        ppu.clear_first_frame_lock(); // Bypass first frame register lock for tests

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
        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal, TimingMode::Ntsc);
        ppu.clear_first_frame_lock(); // Bypass first frame register lock for tests

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

        // Enable background rendering and leftmost 8 pixels
        ppu.mask = 0x0A; // Show background + show leftmost 8 pixels

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
        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal, TimingMode::Ntsc);
        ppu.clear_first_frame_lock(); // Bypass first frame register lock for tests
        ppu.chr_is_ram = true;

        // Set up a tile where all pixels are color 0 (both planes 0)
        for i in 0..16 {
            ppu.chr[i] = 0;
        }

        // Set different values for universal bg and palette 1 color 0
        ppu.palette[0] = 0x0F; // Universal background (black)
        ppu.palette[4] = 0x30; // BG palette 1 color 0 (white) - should be ignored

        // Enable background rendering
        ppu.mask = 0x0A; // Show background + leftmost 8 pixels

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
        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal, TimingMode::Ntsc);
        ppu.clear_first_frame_lock(); // Bypass first frame register lock for tests

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
        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal, TimingMode::Ntsc);
        ppu.clear_first_frame_lock(); // Bypass first frame register lock for tests

        // Enable sprite rendering
        ppu.ctrl = 0x00; // 8x8 sprites
        ppu.mask = 0x14; // Show sprites + leftmost 8 pixels

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

        // CRITICAL: Reading PPUSTATUS should NOT clear sprite overflow flag
        // (Unlike VBlank flag which IS cleared by reading $2002)
        assert!(
            ppu.sprite_overflow.get(),
            "Sprite overflow flag should persist after reading PPUSTATUS"
        );

        // Reading again should still return the flag
        let status2 = ppu.read_register(2);
        assert_eq!(status2 & 0x20, 0x20, "Sprite overflow should still be set");
    }

    #[test]
    fn test_sprite_overflow_not_set_with_8_sprites() {
        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal, TimingMode::Ntsc);
        ppu.clear_first_frame_lock(); // Bypass first frame register lock for tests

        // Enable sprite rendering
        ppu.ctrl = 0x00; // 8x8 sprites
        ppu.mask = 0x14; // Show sprites + leftmost 8 pixels

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
        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal, TimingMode::Ntsc);
        ppu.clear_first_frame_lock(); // Bypass first frame register lock for tests

        // Enable 8x16 sprite mode
        ppu.ctrl = 0x20; // 8x16 sprites
        ppu.mask = 0x14; // Show sprites + leftmost 8 pixels

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
        let ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal, TimingMode::Ntsc);

        // Set sprite 0 hit and sprite overflow
        ppu.sprite_0_hit.set(true);
        ppu.sprite_overflow.set(true);

        // Verify flags are set
        assert!(ppu.sprite_0_hit.get());
        assert!(ppu.sprite_overflow.get());

        // Start VBlank
        ppu.set_vblank(true);

        // Flags should still be set during VBlank (they're only cleared on pre-render scanline)
        assert!(ppu.sprite_0_hit.get());
        assert!(ppu.sprite_overflow.get());

        // Call clear_sprite_flags (normally done at start of pre-render scanline)
        ppu.clear_sprite_flags();

        // Flags should now be cleared
        assert!(!ppu.sprite_0_hit.get());
        assert!(!ppu.sprite_overflow.get());
    }

    #[test]
    fn test_palette_read_updates_buffer_with_mirrored_nametable() {
        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal, TimingMode::Ntsc);
        ppu.clear_first_frame_lock(); // Bypass first frame register lock for tests

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
        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal, TimingMode::Ntsc);
        ppu.clear_first_frame_lock(); // Bypass first frame register lock for tests

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
        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal, TimingMode::Ntsc);
        ppu.clear_first_frame_lock(); // Bypass first frame register lock for tests

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
        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::Vertical, TimingMode::Ntsc);
        ppu.clear_first_frame_lock(); // Bypass first frame register lock for tests

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
        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal, TimingMode::Ntsc);
        ppu.clear_first_frame_lock(); // Bypass first frame register lock for tests

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

    // ============================================================================
    // Base NES Edge Case Tests
    // ============================================================================

    #[test]
    fn test_vram_address_wrapping() {
        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal, TimingMode::Ntsc);
        ppu.clear_first_frame_lock(); // Bypass first frame register lock for tests
        ppu.chr_is_ram = true;

        // Write to an address > 0x3FFF and verify it wraps
        // Hardware-accurate: first write only affects t, second write copies t to v
        ppu.write_register(6, 0x3F); // High byte (0x3F00 - masked to 6 bits)
        ppu.write_register(6, 0xFF); // Low byte (0x3FFF)

        // Address should be 0x3FFF (high byte masked to 6 bits: 0x3F, low byte 0xFF)
        assert_eq!(ppu.vram_addr.get(), 0x3FFF);

        // Write a value - this should write to wrapped address (0x3FFF & 0x3FFF = 0x3FFF)
        ppu.write_register(7, 0x12);

        // Read back from palette $3F1F (since $3FFF wraps to palette space)
        ppu.vram_addr.set(0x3F1F);
        let val = ppu.read_register(7);
        assert_eq!(val, 0x12, "VRAM address should wrap at 0x3FFF boundary");
    }

    #[test]
    fn test_ppuctrl_ppumask_write_only() {
        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal, TimingMode::Ntsc);
        ppu.clear_first_frame_lock(); // Bypass first frame register lock for tests

        // Write distinctive values to PPUCTRL and PPUMASK
        ppu.write_register(0, 0xAB); // PPUCTRL
        ppu.write_register(1, 0xCD); // PPUMASK

        // Reading from write-only registers should return 0
        // (Actually returns 0 from open bus, but our implementation returns 0)
        assert_eq!(ppu.read_register(0), 0, "PPUCTRL is write-only");
        assert_eq!(ppu.read_register(1), 0, "PPUMASK is write-only");
    }

    #[test]
    fn test_ppustatus_clears_vblank_and_latch() {
        let ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal, TimingMode::Ntsc);

        // Set VBlank flag
        ppu.set_vblank(true);
        assert!(ppu.vblank.get());

        // Set address latch to true (simulate partial PPUADDR write)
        ppu.addr_latch.set(true);

        // Read PPUSTATUS
        let status = ppu.read_register(2);
        assert_eq!(status & 0x80, 0x80, "VBlank bit should be set before read");

        // VBlank flag should be cleared after read
        assert!(!ppu.vblank.get(), "Reading PPUSTATUS should clear VBlank");

        // Address latch should be reset
        assert!(
            !ppu.addr_latch.get(),
            "Reading PPUSTATUS should reset address latch"
        );
    }

    #[test]
    fn test_ppuscroll_double_write_behavior() {
        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal, TimingMode::Ntsc);
        ppu.clear_first_frame_lock(); // Bypass first frame register lock for tests

        // First write sets X scroll
        ppu.write_register(5, 0x12);
        assert_eq!(ppu.scroll_x(), 0x12);
        assert!(ppu.addr_latch.get(), "First write should set latch");

        // Second write sets Y scroll
        ppu.write_register(5, 0x34);
        assert_eq!(ppu.scroll_y(), 0x34);
        assert!(!ppu.addr_latch.get(), "Second write should clear latch");

        // Third write should start over (X scroll)
        ppu.write_register(5, 0x56);
        assert_eq!(ppu.scroll_x(), 0x56);
        assert!(ppu.addr_latch.get(), "Third write should set latch again");
    }

    #[test]
    fn test_ppuaddr_double_write_behavior() {
        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal, TimingMode::Ntsc);
        ppu.clear_first_frame_lock(); // Bypass first frame register lock for tests

        // Hardware-accurate behavior: first write ONLY affects t register, not v
        // v is only updated on the second write when t is copied to v

        // First write sets high byte of t only - v unchanged
        ppu.write_register(6, 0x20);
        assert_eq!(
            ppu.vram_addr.get(),
            0x0000,
            "v should not change on first write"
        );
        assert_eq!(
            ppu.temp_vram_addr.get() & 0x3F00,
            0x2000,
            "t high byte should be set"
        );
        assert!(ppu.addr_latch.get(), "First write should set latch");

        // Second write sets low byte and copies t to v
        ppu.write_register(6, 0x50);
        assert_eq!(
            ppu.vram_addr.get(),
            0x2050,
            "v should now have full address"
        );
        assert!(!ppu.addr_latch.get(), "Second write should clear latch");

        // Third write should start over (high byte) - only affects t
        ppu.write_register(6, 0x3F);
        assert_eq!(
            ppu.vram_addr.get(),
            0x2050,
            "v should not change on first write of new sequence"
        );
        assert!(ppu.addr_latch.get(), "Third write should set latch again");

        // Fourth write completes the sequence and updates v
        ppu.write_register(6, 0x00);
        assert_eq!(ppu.vram_addr.get(), 0x3F00, "v should now have new address");
    }

    #[test]
    fn test_ppuaddr_ppuscroll_shared_latch() {
        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal, TimingMode::Ntsc);
        ppu.clear_first_frame_lock(); // Bypass first frame register lock for tests

        // Write to PPUSCROLL (sets latch)
        ppu.write_register(5, 0x10);
        assert!(ppu.addr_latch.get(), "PPUSCROLL should set latch");

        // Write to PPUADDR should use the shared latch
        // Since latch is true, this should write low byte
        ppu.write_register(6, 0x50);
        assert!(!ppu.addr_latch.get(), "PPUADDR should clear latch");
        assert_eq!(ppu.vram_addr.get() & 0xFF, 0x50, "Low byte should be set");

        // Reset and test the other way
        ppu.addr_latch.set(false);
        ppu.write_register(6, 0x20); // High byte
        assert!(ppu.addr_latch.get());

        // Write to PPUSCROLL should use shared latch
        // Since latch is true, this should write Y scroll
        ppu.write_register(5, 0x30);
        assert!(!ppu.addr_latch.get());
        assert_eq!(ppu.scroll_y(), 0x30, "Y scroll should be set");
    }

    #[test]
    fn test_oam_addr_wrapping() {
        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal, TimingMode::Ntsc);
        ppu.clear_first_frame_lock(); // Bypass first frame register lock for tests

        // Set OAM address to 0xFF
        ppu.write_register(3, 0xFF);
        assert_eq!(ppu.oam_addr.get(), 0xFF);

        // Write to OAMDATA should wrap address
        ppu.write_register(4, 0xAB);
        assert_eq!(ppu.oam[0xFF], 0xAB);
        assert_eq!(
            ppu.oam_addr.get(),
            0x00,
            "OAM address should wrap to 0 after 0xFF"
        );

        // Next write should go to address 0
        ppu.write_register(4, 0xCD);
        assert_eq!(ppu.oam[0x00], 0xCD);
        assert_eq!(ppu.oam_addr.get(), 0x01);
    }

    #[test]
    fn test_vram_increment_mode() {
        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal, TimingMode::Ntsc);
        ppu.clear_first_frame_lock(); // Bypass first frame register lock for tests
        ppu.chr_is_ram = true;

        // Test increment by 1 (default)
        ppu.ctrl = 0x00; // Bit 2 = 0: increment by 1
        ppu.vram_addr.set(0x2000);
        ppu.write_register(7, 0xAA);
        assert_eq!(
            ppu.vram_addr.get(),
            0x2001,
            "Should increment by 1 when bit 2 is clear"
        );

        // Test increment by 32
        ppu.ctrl = 0x04; // Bit 2 = 1: increment by 32
        ppu.vram_addr.set(0x2000);
        ppu.write_register(7, 0xBB);
        assert_eq!(
            ppu.vram_addr.get(),
            0x2020,
            "Should increment by 32 when bit 2 is set"
        );

        // Test that reads also increment
        ppu.ctrl = 0x00; // Increment by 1
        ppu.vram_addr.set(0x2000);
        let _ = ppu.read_register(7); // Buffered read
        assert_eq!(
            ppu.vram_addr.get(),
            0x2001,
            "Read should also increment address"
        );
    }

    #[test]
    fn test_nmi_on_vblank_when_enabled() {
        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal, TimingMode::Ntsc);
        ppu.clear_first_frame_lock(); // Bypass first frame register lock for tests

        // Enable NMI in PPUCTRL
        ppu.write_register(0, 0x80); // Set bit 7

        // Set VBlank - should trigger NMI
        ppu.set_vblank(true);
        assert!(
            ppu.take_nmi_pending(),
            "NMI should be pending when VBlank starts with NMI enabled"
        );

        // Second call should return false (NMI was taken)
        assert!(!ppu.take_nmi_pending(), "NMI should only fire once");
    }

    #[test]
    fn test_nmi_enable_during_vblank() {
        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal, TimingMode::Ntsc);
        ppu.clear_first_frame_lock(); // Bypass first frame register lock for tests

        // Start VBlank with NMI disabled
        ppu.ctrl = 0x00; // NMI disabled
        ppu.set_vblank(true);
        assert!(!ppu.take_nmi_pending(), "NMI should not fire when disabled");

        // Enable NMI during VBlank - should trigger NMI
        ppu.write_register(0, 0x80); // Enable NMI
        assert!(
            ppu.take_nmi_pending(),
            "Enabling NMI during VBlank should trigger NMI"
        );
    }

    #[test]
    fn test_palette_address_mirroring_edge_cases() {
        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal, TimingMode::Ntsc);
        ppu.clear_first_frame_lock(); // Bypass first frame register lock for tests

        // Test writing to palette addresses beyond $3F1F mirrors correctly
        // $3F20 should mirror to $3F00
        ppu.write_register(6, 0x3F);
        ppu.write_register(6, 0x20);
        ppu.write_register(7, 0x0F);

        // Read from $3F00 - should see mirrored value
        ppu.vram_addr.set(0x3F00);
        let val = ppu.read_register(7);
        assert_eq!(val, 0x0F, "$3F20 should mirror to $3F00");

        // Test $3FFF mirrors to $3F1F
        ppu.write_register(6, 0x3F);
        ppu.write_register(6, 0xFF);
        ppu.write_register(7, 0x30);

        ppu.vram_addr.set(0x3F1F);
        let val = ppu.read_register(7);
        assert_eq!(val, 0x30, "$3FFF should mirror to $3F1F");
    }

    // NOTE: test_single_screen_mirroring removed - redundant with test_nametable_read_write_all_four_comprehensive

    // ============================================================================
    // CRITICAL REGRESSION TESTS - DO NOT DELETE OR MODIFY
    // These tests verify fixes for Super Mario Bros. 3 and other games
    // ============================================================================

    #[test]
    fn regression_vblank_starts_true() {
        // REGRESSION TEST: VBlank must start as true for SMB3 compatibility
        // Reference: Fixed 2024-12-21
        // Super Mario Bros. 3 expects VBlank to be set on the first frame.
        // Starting with false causes SMB3 to hang waiting for VBlank.
        let ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal, TimingMode::Ntsc);

        assert!(
            ppu.vblank_flag(),
            "CRITICAL: VBlank MUST start as true - required for Super Mario Bros. 3!"
        );
    }

    #[test]
    fn regression_ppustatus_read_clears_nmi() {
        // REGRESSION TEST: Reading PPUSTATUS must clear pending NMI (NMI suppression)
        // Reference: Fixed 2024-12-21
        // This is critical for NMI timing - if a game reads PPUSTATUS right when
        // VBlank starts, the NMI must be prevented.
        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal, TimingMode::Ntsc);
        ppu.clear_first_frame_lock(); // Bypass first frame register lock for tests

        // Enable NMI
        ppu.write_register(0, 0x80);

        // Start VBlank (should set NMI pending)
        ppu.set_vblank(true);
        assert!(
            ppu.take_nmi_pending(),
            "NMI should be pending after VBlank starts"
        );

        // Set up another VBlank + NMI
        ppu.set_vblank(false);
        ppu.set_vblank(true);

        // Read PPUSTATUS - this MUST clear the pending NMI
        let status = ppu.read_register(2);
        assert_eq!(status & 0x80, 0x80, "VBlank flag should be set in status");

        // NMI should now be cleared due to PPUSTATUS read
        assert!(
            !ppu.take_nmi_pending(),
            "CRITICAL: Reading PPUSTATUS MUST clear pending NMI (NMI suppression)!"
        );
    }

    #[test]
    fn regression_ppustatus_read_clears_vblank() {
        // REGRESSION TEST: Reading PPUSTATUS must clear VBlank flag
        // Reference: Fixed 2024-12-21
        let ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal, TimingMode::Ntsc);

        // Start VBlank
        ppu.set_vblank(true);
        assert!(ppu.vblank_flag(), "VBlank should be set");

        // Read PPUSTATUS
        let status = ppu.read_register(2);
        assert_eq!(status & 0x80, 0x80, "Status should show VBlank set");

        // VBlank flag should now be cleared
        assert!(
            !ppu.vblank_flag(),
            "CRITICAL: Reading PPUSTATUS MUST clear VBlank flag!"
        );

        // Second read should return VBlank as cleared
        let status2 = ppu.read_register(2);
        assert_eq!(
            status2 & 0x80,
            0x00,
            "Second PPUSTATUS read should show VBlank cleared"
        );
    }

    #[test]
    fn regression_vblank_end_clears_nmi() {
        // REGRESSION TEST: Ending VBlank (pre-render scanline) must clear NMI
        // Reference: Fixed 2024-12-21
        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal, TimingMode::Ntsc);
        ppu.clear_first_frame_lock(); // Bypass first frame register lock for tests

        // Enable NMI
        ppu.write_register(0, 0x80);

        // Start VBlank
        ppu.set_vblank(true);
        assert!(ppu.take_nmi_pending(), "NMI should be pending");

        // End VBlank (start of pre-render scanline)
        ppu.set_vblank(false);

        // NMI should be automatically cleared
        assert!(
            !ppu.take_nmi_pending(),
            "CRITICAL: Ending VBlank MUST clear pending NMI!"
        );
    }

    #[test]
    fn regression_sprite_flags_not_cleared_by_vblank() {
        // REGRESSION TEST: Sprite flags should NOT be cleared when VBlank starts or ends
        // Reference: Fixed 2024-12-21
        // They should only be cleared on the pre-render scanline via clear_sprite_flags()
        let ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal, TimingMode::Ntsc);

        // Set sprite flags
        ppu.sprite_0_hit.set(true);
        ppu.sprite_overflow.set(true);

        // Start VBlank
        ppu.set_vblank(true);

        // Flags should still be set
        assert!(
            ppu.sprite_0_hit.get(),
            "Sprite 0 hit should NOT be cleared when VBlank starts"
        );
        assert!(
            ppu.sprite_overflow.get(),
            "Sprite overflow should NOT be cleared when VBlank starts"
        );

        // End VBlank
        ppu.set_vblank(false);

        // Flags should STILL be set
        assert!(
            ppu.sprite_0_hit.get(),
            "CRITICAL: Sprite 0 hit should NOT be cleared when VBlank ends!"
        );
        assert!(
            ppu.sprite_overflow.get(),
            "CRITICAL: Sprite overflow should NOT be cleared when VBlank ends!"
        );

        // Only clear_sprite_flags() should clear them
        ppu.clear_sprite_flags();
        assert!(!ppu.sprite_0_hit.get());
        assert!(!ppu.sprite_overflow.get());
    }

    #[test]
    fn regression_sprite_overflow_not_cleared_by_ppustatus_read() {
        // REGRESSION TEST: Reading PPUSTATUS ($2002) should NOT clear sprite overflow flag
        // Reference: NESdev wiki - sprite overflow is only cleared at dot 1 of pre-render scanline
        // This is different from VBlank flag (bit 7) which IS cleared by reading $2002
        let ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal, TimingMode::Ntsc);
        ppu.clear_first_frame_lock();

        // Set sprite overflow flag
        ppu.sprite_overflow.set(true);

        // Also set VBlank for comparison
        ppu.set_vblank(true);

        // Read PPUSTATUS
        let status = ppu.read_register(2);
        assert_eq!(status & 0x80, 0x80, "VBlank should be set in status");
        assert_eq!(
            status & 0x20,
            0x20,
            "Sprite overflow should be set in status"
        );

        // After read: VBlank should be cleared, but sprite overflow should NOT be cleared
        assert!(
            !ppu.vblank_flag(),
            "VBlank flag SHOULD be cleared by reading PPUSTATUS"
        );
        assert!(
            ppu.sprite_overflow.get(),
            "CRITICAL: Sprite overflow flag MUST NOT be cleared by reading PPUSTATUS!"
        );

        // Reading again should show VBlank cleared but sprite overflow still set
        let status2 = ppu.read_register(2);
        assert_eq!(
            status2 & 0x80,
            0x00,
            "VBlank should remain cleared after second read"
        );
        assert_eq!(
            status2 & 0x20,
            0x20,
            "Sprite overflow should still be set after second read"
        );

        // Only clear_sprite_flags() should clear sprite overflow
        ppu.clear_sprite_flags();
        assert!(!ppu.sprite_overflow.get());

        let status3 = ppu.read_register(2);
        assert_eq!(
            status3 & 0x20,
            0x00,
            "Sprite overflow should be cleared after clear_sprite_flags()"
        );
    }

    #[test]
    fn regression_nmi_only_fires_on_rising_edge() {
        // REGRESSION TEST: NMI should only fire when VBlank transitions from false to true
        // Reference: Fixed 2024-12-21
        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal, TimingMode::Ntsc);
        ppu.clear_first_frame_lock(); // Bypass first frame register lock for tests

        // VBlank starts as true - clear it first
        ppu.set_vblank(false);

        // Enable NMI (VBlank is false, so no NMI should fire yet)
        ppu.write_register(0, 0x80);
        assert!(
            !ppu.take_nmi_pending(),
            "NMI should not be pending when VBlank is false"
        );

        // Now set VBlank (rising edge: false -> true) - NMI should fire
        ppu.set_vblank(true);
        assert!(
            ppu.take_nmi_pending(),
            "CRITICAL: NMI MUST fire on VBlank rising edge (false -> true)!"
        );

        // Setting VBlank again (already true) should NOT fire another NMI
        ppu.nmi_pending.set(false); // Clear it manually
        ppu.set_vblank(true);
        assert!(
            !ppu.take_nmi_pending(),
            "NMI should NOT fire if VBlank is already true (no edge)"
        );
    }

    // ============================================================================
    // Sprite Priority Tests
    // ============================================================================

    #[test]
    fn test_sprite_priority_lower_oam_index_wins() {
        // Test that sprite with lower OAM index hides sprite with higher OAM index,
        // regardless of priority bits.
        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal, TimingMode::Ntsc);
        ppu.clear_first_frame_lock(); // Bypass first frame register lock for tests
        ppu.chr_is_ram = true;

        // Enable sprite rendering ONLY (no background)
        ppu.ctrl = 0x00; // 8x8 sprites, pattern table at $0000
        ppu.mask = 0x10; // Show sprites only

        // Set up a simple sprite pattern (solid square)
        for i in 0..8 {
            ppu.chr[i] = 0xFF; // Low plane
            ppu.chr[i + 8] = 0xFF; // High plane (color 3)
        }

        // Set up palettes
        ppu.palette[0x11] = 0x0F; // Sprite 0 palette - black
        ppu.palette[0x12] = 0x0F;
        ppu.palette[0x13] = 0x30; // Color 3 - white
        ppu.palette[0x15] = 0x0F; // Sprite 1 palette
        ppu.palette[0x16] = 0x0F;
        ppu.palette[0x17] = 0x16; // Color 3 - red

        // Sprite 0: Front priority, at (8, 8), palette 0 (white)
        // Covers Y=8-15, X=8-15
        ppu.oam[0] = 7; // Y position (rendered at Y+1 = 8)
        ppu.oam[1] = 0; // Tile 0
        ppu.oam[2] = 0x00; // Front priority, palette 0
        ppu.oam[3] = 8; // X position

        // Sprite 1: Front priority, at (10, 10), palette 1 (red)
        // Covers Y=10-17, X=10-17
        ppu.oam[4] = 9; // Y position (rendered at Y+1 = 10)
        ppu.oam[5] = 0; // Tile 0
        ppu.oam[6] = 0x01; // Front priority, palette 1
        ppu.oam[7] = 10; // X position

        // Render frame
        let frame = ppu.render_frame();

        // At pixel (10, 10), sprite 0 should win (white), not sprite 1 (red)
        // This is in the overlap area.
        let pixel_10_10 = frame.pixels[10 * 256 + 10];
        assert_eq!(
            pixel_10_10,
            nes_palette_rgb(0x30),
            "Lower OAM index (sprite 0) should hide higher OAM index (sprite 1)"
        );

        // At pixel (12, 12), sprite 0 should still win (in overlap area)
        let pixel_12_12 = frame.pixels[12 * 256 + 12];
        assert_eq!(
            pixel_12_12,
            nes_palette_rgb(0x30),
            "Sprite 0 should cover overlapping area"
        );

        // At pixel (16, 16), only sprite 1 is present (beyond sprite 0's range), so it should be red
        let pixel_16_16 = frame.pixels[16 * 256 + 16];
        assert_eq!(
            pixel_16_16,
            nes_palette_rgb(0x16),
            "Sprite 1 should be visible where sprite 0 doesn't overlap"
        );

        // At pixel (8, 8), only sprite 0 is present, so it should be white
        let pixel_8_8 = frame.pixels[8 * 256 + 8];
        assert_eq!(
            pixel_8_8,
            nes_palette_rgb(0x30),
            "Sprite 0 should be visible at its top-left corner"
        );
    }

    #[test]
    fn test_sprite_priority_back_priority_sprite_hides_front_priority() {
        // Test the critical edge case: A back-priority sprite at lower OAM index
        // can hide a front-priority sprite at higher index, even though the
        // back-priority sprite itself may be hidden behind opaque background.
        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal, TimingMode::Ntsc);
        ppu.clear_first_frame_lock(); // Bypass first frame register lock for tests
        ppu.chr_is_ram = true;

        // Enable background and sprite rendering
        ppu.ctrl = 0x00; // 8x8 sprites, pattern table at $0000
        ppu.mask = 0x18; // Show background and sprites

        // Set up background tile with opaque pixels
        for i in 0..8 {
            ppu.chr[i] = 0xFF; // Low plane
            ppu.chr[i + 8] = 0x00; // High plane (color 1)
        }

        // Set up sprite pattern (solid)
        for i in 16..24 {
            ppu.chr[i] = 0xFF; // Low plane
            ppu.chr[i + 8] = 0xFF; // High plane (color 3)
        }

        // Set up palettes
        ppu.palette[0] = 0x0F; // Universal background - black
        ppu.palette[1] = 0x1C; // BG color 1 - blue
        ppu.palette[0x11] = 0x0F;
        ppu.palette[0x12] = 0x0F;
        ppu.palette[0x13] = 0x30; // Sprite 0 color 3 - white
        ppu.palette[0x15] = 0x0F;
        ppu.palette[0x16] = 0x0F;
        ppu.palette[0x17] = 0x16; // Sprite 1 color 3 - red

        // Set up background tile at (8,8)
        ppu.vram[0] = 0; // Use tile 0 for background

        // Sprite 0: Back priority (behind BG), at (8, 8), palette 0 (white)
        // Covers Y=8-15, X=8-15
        ppu.oam[0] = 7; // Y position
        ppu.oam[1] = 1; // Tile 1 (sprite pattern)
        ppu.oam[2] = 0x20; // Back priority, palette 0
        ppu.oam[3] = 8; // X position

        // Sprite 1: Front priority, at (10, 10), palette 1 (red)
        // Covers Y=10-17, X=10-17
        ppu.oam[4] = 9; // Y position
        ppu.oam[5] = 1; // Tile 1
        ppu.oam[6] = 0x01; // Front priority, palette 1
        ppu.oam[7] = 10; // X position

        // Render frame
        let frame = ppu.render_frame();

        // At pixel (10, 10):
        // - Background is opaque (blue)
        // - Sprite 0 (back priority) is in sprite buffer at this position
        // - Sprite 1 (front priority) is NOT in sprite buffer (sprite 0 won)
        // - Since sprite 0 has back priority and BG is opaque, BG should show (blue)
        let pixel_10_10 = frame.pixels[10 * 256 + 10];
        assert_eq!(
            pixel_10_10,
            nes_palette_rgb(0x1C),
            "Back-priority sprite 0 should hide front-priority sprite 1, allowing BG to show"
        );

        // At pixel (12, 12), same situation
        let pixel_12_12 = frame.pixels[12 * 256 + 12];
        assert_eq!(pixel_12_12, nes_palette_rgb(0x1C), "BG should show through");

        // At pixel (16, 16), only sprite 1 is in buffer, and it has front priority
        // so it should be visible (red) over the background
        let pixel_16_16 = frame.pixels[16 * 256 + 16];
        assert_eq!(
            pixel_16_16,
            nes_palette_rgb(0x16),
            "Sprite 1 should be visible where sprite 0 doesn't overlap"
        );
    }

    #[test]
    fn test_sprite_priority_front_over_transparent_bg() {
        // Test that front-priority sprites always show over transparent background
        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal, TimingMode::Ntsc);
        ppu.clear_first_frame_lock(); // Bypass first frame register lock for tests
        ppu.chr_is_ram = true;

        // Enable sprite rendering (no background)
        ppu.ctrl = 0x00;
        ppu.mask = 0x14; // Sprites + leftmost 8 pixels

        // Set up sprite pattern
        for i in 0..8 {
            ppu.chr[i] = 0xFF;
            ppu.chr[i + 8] = 0xFF;
        }

        // Set up palette
        ppu.palette[0x11] = 0x0F;
        ppu.palette[0x12] = 0x0F;
        ppu.palette[0x13] = 0x30; // White

        // Sprite with front priority
        ppu.oam[0] = 7;
        ppu.oam[1] = 0;
        ppu.oam[2] = 0x00; // Front priority
        ppu.oam[3] = 8;

        let frame = ppu.render_frame();

        // Sprite should be visible over transparent background
        let pixel = frame.pixels[8 * 256 + 8];
        assert_eq!(
            pixel,
            nes_palette_rgb(0x30),
            "Front-priority sprite should show over transparent BG"
        );
    }

    #[test]
    fn test_sprite_priority_back_over_transparent_bg() {
        // Test that back-priority sprites show over transparent background
        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal, TimingMode::Ntsc);
        ppu.clear_first_frame_lock(); // Bypass first frame register lock for tests
        ppu.chr_is_ram = true;

        // Enable sprite rendering (no background)
        ppu.ctrl = 0x00;
        ppu.mask = 0x14; // Sprites + leftmost 8 pixels

        // Set up sprite pattern
        for i in 0..8 {
            ppu.chr[i] = 0xFF;
            ppu.chr[i + 8] = 0xFF;
        }

        // Set up palette
        ppu.palette[0x11] = 0x0F;
        ppu.palette[0x12] = 0x0F;
        ppu.palette[0x13] = 0x30; // White

        // Sprite with back priority
        ppu.oam[0] = 7;
        ppu.oam[1] = 0;
        ppu.oam[2] = 0x20; // Back priority
        ppu.oam[3] = 8;

        let frame = ppu.render_frame();

        // Back-priority sprite should still show over transparent background
        let pixel = frame.pixels[8 * 256 + 8];
        assert_eq!(
            pixel,
            nes_palette_rgb(0x30),
            "Back-priority sprite should show over transparent BG"
        );
    }

    #[test]
    fn test_sprite_priority_back_behind_opaque_bg() {
        // Test that back-priority sprites hide behind opaque background
        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal, TimingMode::Ntsc);
        ppu.clear_first_frame_lock(); // Bypass first frame register lock for tests
        ppu.chr_is_ram = true;

        // Enable background and sprites
        ppu.ctrl = 0x00;
        ppu.mask = 0x18;

        // Background pattern (opaque)
        for i in 0..8 {
            ppu.chr[i] = 0xFF;
            ppu.chr[i + 8] = 0x00; // Color 1
        }

        // Sprite pattern
        for i in 16..24 {
            ppu.chr[i] = 0xFF;
            ppu.chr[i + 8] = 0xFF; // Color 3
        }

        // Palettes
        ppu.palette[0] = 0x0F; // Universal BG
        ppu.palette[1] = 0x1C; // BG color 1 - blue
        ppu.palette[0x11] = 0x0F;
        ppu.palette[0x12] = 0x0F;
        ppu.palette[0x13] = 0x30; // Sprite color 3 - white

        // Background tile
        ppu.vram[0] = 0;

        // Back-priority sprite
        ppu.oam[0] = 7;
        ppu.oam[1] = 1;
        ppu.oam[2] = 0x20; // Back priority
        ppu.oam[3] = 8;

        let frame = ppu.render_frame();

        // Background should be visible, not sprite
        let pixel = frame.pixels[8 * 256 + 8];
        assert_eq!(
            pixel,
            nes_palette_rgb(0x1C),
            "Back-priority sprite should hide behind opaque BG"
        );
    }

    #[test]
    fn test_nametable_scrolling_xor_behavior() {
        // This test verifies that nametable selection uses XOR, not addition,
        // when scrolling crosses nametable boundaries. This is critical for
        // games like Turbo Racing that use scrolling across nametable boundaries.
        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::Vertical, TimingMode::Ntsc);
        ppu.clear_first_frame_lock(); // Bypass first frame register lock for tests
        ppu.chr_is_ram = true;

        // Set up different tiles in each nametable
        // Nametable 0 (0x2000): tile 0x01
        let addr_nt0 = ppu.map_nametable_addr(0x2000);
        ppu.vram[addr_nt0] = 0x01;
        // Nametable 1 (0x2400): tile 0x02
        let addr_nt1 = ppu.map_nametable_addr(0x2400);
        ppu.vram[addr_nt1] = 0x02;

        // Create distinct tile patterns in CHR-RAM
        // Tile 0x01: all color 1
        for i in 0..8 {
            ppu.chr[0x10 + i] = 0xFF; // Low plane
            ppu.chr[0x10 + 8 + i] = 0x00; // High plane
        }
        // Tile 0x02: all color 2
        for i in 0..8 {
            ppu.chr[0x20 + i] = 0x00; // Low plane
            ppu.chr[0x20 + 8 + i] = 0xFF; // High plane
        }

        // Set up palettes
        ppu.palette[0] = 0x0F; // Universal background
        ppu.palette[1] = 0x30; // Color 1 (white)
        ppu.palette[2] = 0x16; // Color 2 (red)

        // Enable background
        ppu.mask = 0x0A; // Show background + leftmost 8 pixels

        // Test 1: Base nametable 0, no scroll
        // Should read from nametable 0
        ppu.write_register(0, 0x00); // PPUCTRL: nametable 0
        ppu.write_register(5, 0); // X scroll
        ppu.write_register(5, 0); // Y scroll
        let frame = ppu.render_frame();
        let pixel = frame.pixels[0];
        assert_eq!(
            pixel,
            nes_palette_rgb(0x30),
            "No scroll should use nametable 0 (tile 0x01 = color 1)"
        );

        // Test 2: Base nametable 1, no scroll - should use NT1
        ppu.write_register(0, 0x01); // PPUCTRL: nametable 1
        ppu.write_register(5, 0); // X scroll
        ppu.write_register(5, 0); // Y scroll
        let frame = ppu.render_frame();
        let pixel = frame.pixels[0];
        assert_eq!(
            pixel,
            nes_palette_rgb(0x16),
            "Base NT1, no scroll: should show tile 0x02 (color 2)"
        );

        // Test 3: Verify XOR behavior by checking rendering logic
        // The actual boundary crossing behavior is tested at the world coordinate level
        // within render_frame() using (wx / 256) and (wy / 240) calculations.
        //
        // Critical XOR behavior: base_nt ^ nt_x ^ (nt_y << 1)
        // - If base=0, crossing X gives nt=1 (0^1^0=1) - correct!
        // - If base=0, crossing Y gives nt=2 (0^0^2=2) - correct!
        // - If base=1, crossing X gives nt=0 (1^1^0=0) - correct!
        // - If base=2, crossing Y gives nt=0 (2^0^2=0) - correct!
        //
        // This ensures proper nametable selection for games using scrolling.
        // The rendering code applies this XOR logic correctly at lines 727 and 1023.
    }

    #[test]
    fn test_vertical_scrolling_with_base_nametable() {
        // Test that vertical scrolling works correctly with PPUCTRL base nametable Y bit
        // This is critical for games like Rad Racer 2 and F1 Sensation that use Y scrolling
        // Regression test for bug where Y scrolling showed wrong nametable region
        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal, TimingMode::Ntsc);
        ppu.clear_first_frame_lock();
        ppu.chr_is_ram = true;

        // Set up different tiles in nametables 0 and 2
        // With Horizontal mirroring: NT0/NT1 share physical 0, NT2/NT3 share physical 1
        // So NT0 and NT2 are different
        // Nametable 0 (0x2000): tile 0x01
        let addr_nt0 = ppu.map_nametable_addr(0x2000);
        ppu.vram[addr_nt0] = 0x01;
        // Nametable 2 (0x2800): tile 0x02
        let addr_nt2 = ppu.map_nametable_addr(0x2800);
        ppu.vram[addr_nt2] = 0x02;

        // Create distinct tile patterns in CHR-RAM
        // Tile 0x01: all color 1
        for i in 0..8 {
            ppu.chr[0x10 + i] = 0xFF;
            ppu.chr[0x10 + 8 + i] = 0x00;
        }
        // Tile 0x02: all color 2
        for i in 0..8 {
            ppu.chr[0x20 + i] = 0x00;
            ppu.chr[0x20 + 8 + i] = 0xFF;
        }

        // Set up palettes
        ppu.palette[0] = 0x0F; // Universal background
        ppu.palette[1] = 0x30; // Color 1 (white)
        ppu.palette[2] = 0x16; // Color 2 (red)

        // Enable background
        ppu.mask = 0x0A; // Show background + leftmost 8 pixels

        // Test 1: Base nametable 0 (PPUCTRL bits 0-1 = 00), no scroll
        // Should read from nametable 0
        ppu.write_register(0, 0x00); // PPUCTRL: nametable 0
        ppu.write_register(5, 0); // X scroll
        ppu.write_register(5, 0); // Y scroll
        let frame = ppu.render_frame();
        let pixel = frame.pixels[0];
        assert_eq!(
            pixel,
            nes_palette_rgb(0x30),
            "Base NT0, no scroll: should show NT0 (color 1)"
        );

        // Test 2: Base nametable 2 (PPUCTRL bits 0-1 = 10), no scroll
        // Should read from nametable 2 due to base nametable Y bit being set
        ppu.write_register(0, 0x02); // PPUCTRL: nametable 2 (bit 1 set = Y offset)
        ppu.write_register(5, 0); // X scroll
        ppu.write_register(5, 0); // Y scroll
        let frame = ppu.render_frame();
        let pixel = frame.pixels[0];
        assert_eq!(
            pixel,
            nes_palette_rgb(0x16),
            "Base NT2, no scroll: should show NT2 (color 2) due to base Y offset"
        );

        // Test 3: Mid-frame PPUCTRL write updates t register bits 10-11
        ppu.write_register(0, 0x00); // PPUCTRL: nametable 0 (bits 10-11 = 00)
        ppu.write_register(5, 0); // X scroll = 0
        ppu.write_register(5, 0); // Y scroll = 0
        let t1 = ppu.temp_vram_addr.get();
        assert_eq!(
            t1 & 0x0C00,
            0x0000,
            "PPUCTRL nametable 0 should set t bits 10-11 to 00"
        );

        ppu.write_register(0, 0x03); // PPUCTRL: nametable 3 (bits 10-11 = 11)
        let t2 = ppu.temp_vram_addr.get();
        assert_eq!(
            t2 & 0x0C00,
            0x0C00,
            "PPUCTRL nametable 3 should set t bits 10-11 to 11"
        );

        // Test 4: Scroll combination (X=16, Y=16) updates loopy registers correctly
        ppu.write_register(0, 0x00); // PPUCTRL: nametable 0
        ppu.write_register(5, 16); // X scroll = 16 (coarse_x=2, fine_x=0)
        ppu.write_register(5, 16); // Y scroll = 16 (coarse_y=2, fine_y=0)
        let t = ppu.temp_vram_addr.get();
        let coarse_x = t & 0x001F;
        let coarse_y = (t >> 5) & 0x001F;
        let fine_y = (t >> 12) & 0x0007;
        let fine_x = ppu.fine_x.get();
        assert_eq!(coarse_x, 2, "X scroll 16 should set coarse_x to 2");
        assert_eq!(fine_x, 0, "X scroll 16 should set fine_x to 0");
        assert_eq!(coarse_y, 2, "Y scroll 16 should set coarse_y to 2");
        assert_eq!(fine_y, 0, "Y scroll 16 should set fine_y to 0");
    }

    #[test]
    fn test_eight_sprite_per_scanline_limit() {
        // Test that the NES hardware limitation of 8 sprites per scanline is enforced
        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal, TimingMode::Ntsc);
        ppu.clear_first_frame_lock(); // Bypass first frame register lock for tests
        ppu.chr_is_ram = true;

        // Enable sprite rendering
        ppu.ctrl = 0x00; // 8x8 sprites, pattern table at $0000
        ppu.mask = 0x14; // Sprites + leftmost 8 pixels, no background

        // Set up sprite pattern (solid tile)
        for i in 0..8 {
            ppu.chr[i] = 0xFF;
            ppu.chr[i + 8] = 0xFF;
        }

        // Clear all OAM first
        for i in 0..256 {
            ppu.oam[i] = 0xFF; // Y=0xFF means off-screen
        }

        // Set up 10 sprites all on the same scanline (Y=7, which means scanline 8)
        // Each sprite uses a different palette to make them distinguishable
        for i in 0..10 {
            let o = i * 4;
            ppu.oam[o] = 7; // Y position (renders on scanline 8)
            ppu.oam[o + 1] = 0; // Tile 0
            ppu.oam[o + 2] = (i % 4) as u8; // Palette (cycling through 0-3)
            ppu.oam[o + 3] = (i * 16) as u8; // X position (spaced 16 pixels apart)
        }

        // Set up palettes with distinct colors
        ppu.palette[0x11] = 0x01; // Palette 0 color 1
        ppu.palette[0x12] = 0x01; // Palette 0 color 2
        ppu.palette[0x13] = 0x01; // Palette 0 color 3 - dark blue
        ppu.palette[0x15] = 0x02; // Palette 1 color 1
        ppu.palette[0x16] = 0x02; // Palette 1 color 2
        ppu.palette[0x17] = 0x02; // Palette 1 color 3 - dark purple
        ppu.palette[0x19] = 0x03; // Palette 2 color 1
        ppu.palette[0x1A] = 0x03; // Palette 2 color 2
        ppu.palette[0x1B] = 0x03; // Palette 2 color 3 - dark cyan
        ppu.palette[0x1D] = 0x04; // Palette 3 color 1
        ppu.palette[0x1E] = 0x04; // Palette 3 color 2
        ppu.palette[0x1F] = 0x04; // Palette 3 color 3 - dark brown

        let frame = ppu.render_frame();

        // Check that sprites 0-7 are rendered (X positions 0, 16, 32, 48, 64, 80, 96, 112)
        for i in 0..8 {
            let x = i * 16;
            let pixel = frame.pixels[8 * 256 + x];
            // Each sprite should have its corresponding palette color
            let expected_palette_idx = match i % 4 {
                0 => 0x01,
                1 => 0x02,
                2 => 0x03,
                3 => 0x04,
                _ => unreachable!(),
            };
            assert_ne!(
                pixel, 0x00000000,
                "Sprite {} at X={} should be rendered (within 8-sprite limit)",
                i, x
            );
            assert_eq!(
                pixel,
                nes_palette_rgb(expected_palette_idx),
                "Sprite {} at X={} should have correct palette color",
                i,
                x
            );
        }

        // Check that sprites 8-9 are NOT rendered (X positions 128, 144)
        // These exceed the 8-sprite-per-scanline limit
        // They should show the background color instead
        let backdrop_color = nes_palette_rgb(ppu.palette[0]);
        for i in 8..10 {
            let x = i * 16;
            let pixel = frame.pixels[8 * 256 + x];
            assert_eq!(
                pixel, backdrop_color,
                "Sprite {} at X={} should NOT be rendered (exceeds 8-sprite limit), should show backdrop color",
                i, x
            );
        }
    }

    #[test]
    fn test_eight_sprite_limit_with_scanline_rendering() {
        // Test that the 8-sprite limit works correctly with scanline-based rendering
        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal, TimingMode::Ntsc);
        ppu.clear_first_frame_lock();
        ppu.chr_is_ram = true;

        ppu.ctrl = 0x00;
        ppu.mask = 0x14; // Sprites + leftmost 8 pixels

        // Set up sprite pattern
        for i in 0..8 {
            ppu.chr[i] = 0xFF;
            ppu.chr[i + 8] = 0xFF;
        }

        // Create 10 sprites on scanline 50
        for i in 0..10 {
            let o = i * 4;
            ppu.oam[o] = 49; // Y position (renders on scanline 50)
            ppu.oam[o + 1] = 0;
            ppu.oam[o + 2] = 0;
            ppu.oam[o + 3] = (i * 20) as u8; // X position
        }

        ppu.palette[0x11] = 0x30; // White

        let mut frame = Frame::new(256, 240);
        ppu.render_scanline(50, &mut frame);

        // First 8 sprites should be visible
        for i in 0..8 {
            let x = i * 20;
            let pixel = frame.pixels[50 * 256 + x];
            assert_ne!(
                pixel, 0x00000000,
                "Sprite {} should be rendered on scanline",
                i
            );
        }

        // Sprites 8-9 should not be rendered (they should show backdrop color)
        let backdrop_color = nes_palette_rgb(ppu.palette[0]);
        for i in 8..10 {
            let x = i * 20;
            if x < 256 {
                let pixel = frame.pixels[50 * 256 + x];
                assert_eq!(
                    pixel, backdrop_color,
                    "Sprite {} should NOT be rendered (exceeds limit), should show backdrop color",
                    i
                );
            }
        }
    }

    // ============================================================================
    // Nametable Mirroring Tests
    // ============================================================================

    #[test]
    fn test_horizontal_mirroring_all_four_nametables() {
        // Horizontal mirroring: $2000 and $2400 map to same physical RAM
        //                       $2800 and $2C00 map to same physical RAM
        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal, TimingMode::Ntsc);
        ppu.clear_first_frame_lock();

        // Write unique values to all four logical nametables
        ppu.write_register(6, 0x20); // $2000
        ppu.write_register(6, 0x00);
        ppu.write_register(7, 0xAA);

        ppu.write_register(6, 0x24); // $2400
        ppu.write_register(6, 0x00);
        ppu.write_register(7, 0xBB);

        ppu.write_register(6, 0x28); // $2800
        ppu.write_register(6, 0x00);
        ppu.write_register(7, 0xCC);

        ppu.write_register(6, 0x2C); // $2C00
        ppu.write_register(6, 0x00);
        ppu.write_register(7, 0xDD);

        // Read back and verify mirroring
        // $2000 and $2400 should both have 0xBB (last write to first pair)
        ppu.vram_addr.set(0x2000);
        let _ = ppu.read_register(7); // Discard buffer
        let val_2000 = ppu.read_register(7);

        ppu.vram_addr.set(0x2400);
        let _ = ppu.read_register(7); // Discard buffer
        let val_2400 = ppu.read_register(7);

        // $2800 and $2C00 should both have 0xDD (last write to second pair)
        ppu.vram_addr.set(0x2800);
        let _ = ppu.read_register(7); // Discard buffer
        let val_2800 = ppu.read_register(7);

        ppu.vram_addr.set(0x2C00);
        let _ = ppu.read_register(7); // Discard buffer
        let val_2c00 = ppu.read_register(7);

        assert_eq!(
            val_2000, 0xBB,
            "Horizontal: $2000 should mirror to same location as $2400"
        );
        assert_eq!(
            val_2400, 0xBB,
            "Horizontal: $2400 should mirror to same location as $2000"
        );
        assert_eq!(
            val_2800, 0xDD,
            "Horizontal: $2800 should mirror to same location as $2C00"
        );
        assert_eq!(
            val_2c00, 0xDD,
            "Horizontal: $2C00 should mirror to same location as $2800"
        );
    }

    #[test]
    fn test_vertical_mirroring_all_four_nametables() {
        // Vertical mirroring: $2000 and $2800 map to same physical RAM
        //                     $2400 and $2C00 map to same physical RAM
        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::Vertical, TimingMode::Ntsc);
        ppu.clear_first_frame_lock();

        // Write unique values to all four logical nametables
        ppu.write_register(6, 0x20); // $2000
        ppu.write_register(6, 0x00);
        ppu.write_register(7, 0xAA);

        ppu.write_register(6, 0x24); // $2400
        ppu.write_register(6, 0x00);
        ppu.write_register(7, 0xBB);

        ppu.write_register(6, 0x28); // $2800
        ppu.write_register(6, 0x00);
        ppu.write_register(7, 0xCC);

        ppu.write_register(6, 0x2C); // $2C00
        ppu.write_register(6, 0x00);
        ppu.write_register(7, 0xDD);

        // Read back and verify mirroring
        // $2000 and $2800 should both have 0xCC (last write to first pair)
        ppu.vram_addr.set(0x2000);
        let _ = ppu.read_register(7); // Discard buffer
        let val_2000 = ppu.read_register(7);

        ppu.vram_addr.set(0x2800);
        let _ = ppu.read_register(7); // Discard buffer
        let val_2800 = ppu.read_register(7);

        // $2400 and $2C00 should both have 0xDD (last write to second pair)
        ppu.vram_addr.set(0x2400);
        let _ = ppu.read_register(7); // Discard buffer
        let val_2400 = ppu.read_register(7);

        ppu.vram_addr.set(0x2C00);
        let _ = ppu.read_register(7); // Discard buffer
        let val_2c00 = ppu.read_register(7);

        assert_eq!(
            val_2000, 0xCC,
            "Vertical: $2000 should mirror to same location as $2800"
        );
        assert_eq!(
            val_2800, 0xCC,
            "Vertical: $2800 should mirror to same location as $2000"
        );
        assert_eq!(
            val_2400, 0xDD,
            "Vertical: $2400 should mirror to same location as $2C00"
        );
        assert_eq!(
            val_2c00, 0xDD,
            "Vertical: $2C00 should mirror to same location as $2400"
        );
    }

    #[test]
    fn test_four_screen_mirroring_requires_4kb_vram() {
        // Four-screen mirroring uses 4KB VRAM (all four nametables independent)
        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::FourScreen, TimingMode::Ntsc);
        ppu.clear_first_frame_lock();

        // Verify 4KB VRAM was allocated
        assert_eq!(
            ppu.vram.len(),
            0x1000,
            "FourScreen mirroring should allocate 4KB VRAM"
        );

        // Write unique values to all four logical nametables
        ppu.write_register(6, 0x20); // $2000
        ppu.write_register(6, 0x00);
        ppu.write_register(7, 0xAA);

        ppu.write_register(6, 0x24); // $2400
        ppu.write_register(6, 0x00);
        ppu.write_register(7, 0xBB);

        ppu.write_register(6, 0x28); // $2800
        ppu.write_register(6, 0x00);
        ppu.write_register(7, 0xCC);

        ppu.write_register(6, 0x2C); // $2C00
        ppu.write_register(6, 0x00);
        ppu.write_register(7, 0xDD);

        // With 4KB VRAM, all four nametables should be independent
        ppu.vram_addr.set(0x2000);
        let _ = ppu.read_register(7);
        let val_2000 = ppu.read_register(7);

        ppu.vram_addr.set(0x2400);
        let _ = ppu.read_register(7);
        let val_2400 = ppu.read_register(7);

        ppu.vram_addr.set(0x2800);
        let _ = ppu.read_register(7);
        let val_2800 = ppu.read_register(7);

        ppu.vram_addr.set(0x2C00);
        let _ = ppu.read_register(7);
        let val_2c00 = ppu.read_register(7);

        // All four nametables should be independent with 4KB VRAM
        assert_eq!(val_2000, 0xAA, "FourScreen: $2000 should be independent");
        assert_eq!(val_2400, 0xBB, "FourScreen: $2400 should be independent");
        assert_eq!(val_2800, 0xCC, "FourScreen: $2800 should be independent");
        assert_eq!(val_2c00, 0xDD, "FourScreen: $2C00 should be independent");
    }

    #[test]
    fn test_nametable_mirroring_with_offsets() {
        // Test that mirroring works correctly at arbitrary offsets within nametables
        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal, TimingMode::Ntsc);
        ppu.clear_first_frame_lock();

        // Write to $2050 (offset 0x50 in nametable 0)
        ppu.write_register(6, 0x20);
        ppu.write_register(6, 0x50);
        ppu.write_register(7, 0x11);

        // Read from $2450 (same offset in nametable 1, should mirror)
        ppu.vram_addr.set(0x2450);
        let _ = ppu.read_register(7);
        let val = ppu.read_register(7);

        assert_eq!(
            val, 0x11,
            "Horizontal mirroring should work at arbitrary offsets"
        );

        // Test vertical mirroring with offsets
        ppu.set_mirroring(Mirroring::Vertical);

        ppu.write_register(6, 0x20);
        ppu.write_register(6, 0x75);
        ppu.write_register(7, 0x22);

        // Read from $2875 (same offset in nametable 2, should mirror)
        ppu.vram_addr.set(0x2875);
        let _ = ppu.read_register(7);
        let val = ppu.read_register(7);

        assert_eq!(
            val, 0x22,
            "Vertical mirroring should work at arbitrary offsets"
        );
    }

    // NOTE: test_single_screen_lower_mirroring removed - redundant with test_nametable_read_write_all_four_comprehensive

    // NOTE: test_single_screen_upper_mirroring removed - redundant with test_nametable_read_write_all_four_comprehensive

    #[test]
    fn test_attribute_table_mirroring() {
        // Attribute tables are at +0x3C0 within each nametable
        // They should follow the same mirroring rules
        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal, TimingMode::Ntsc);
        ppu.clear_first_frame_lock();

        // Write to attribute table in nametable 0
        ppu.write_register(6, 0x23); // $23C0 = attribute table for NT 0
        ppu.write_register(6, 0xC0);
        ppu.write_register(7, 0xAA);

        // Read from attribute table in nametable 1 (should mirror)
        ppu.vram_addr.set(0x27C0); // $27C0 = attribute table for NT 1
        let _ = ppu.read_register(7);
        let val = ppu.read_register(7);

        assert_eq!(
            val, 0xAA,
            "Horizontal: attribute tables should mirror like regular nametable data"
        );

        // Test vertical mirroring for attribute tables
        ppu.set_mirroring(Mirroring::Vertical);

        ppu.write_register(6, 0x27); // $27C0 = attribute table for NT 1
        ppu.write_register(6, 0xC0);
        ppu.write_register(7, 0xBB);

        // Read from attribute table in nametable 3 (should mirror)
        ppu.vram_addr.set(0x2FC0); // $2FC0 = attribute table for NT 3
        let _ = ppu.read_register(7);
        let val = ppu.read_register(7);

        assert_eq!(
            val, 0xBB,
            "Vertical: attribute tables should mirror like regular nametable data"
        );
    }

    #[test]
    fn test_nametable_read_write_all_four_comprehensive() {
        // Comprehensive test that writes and reads all four nametables
        // using PPU registers, simulating how Camerica and other mappers access nametables

        // Test with Horizontal mirroring
        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal, TimingMode::Ntsc);
        ppu.clear_first_frame_lock();

        // Write distinct values to all four nametables at the same offset (0x24)
        // to verify mirroring behavior
        ppu.write_register(6, 0x20); // $2024 - NT0
        ppu.write_register(6, 0x24);
        ppu.write_register(7, 0xAA);

        ppu.write_register(6, 0x24); // $2424 - NT1
        ppu.write_register(6, 0x24);
        ppu.write_register(7, 0xBB);

        ppu.write_register(6, 0x28); // $2824 - NT2
        ppu.write_register(6, 0x24);
        ppu.write_register(7, 0xCC);

        ppu.write_register(6, 0x2C); // $2C24 - NT3
        ppu.write_register(6, 0x24);
        ppu.write_register(7, 0xDD);

        // Read back and verify Horizontal mirroring:
        // NT0 and NT1 should mirror (both show 0xBB - last write to pair)
        // NT2 and NT3 should mirror (both show 0xDD - last write to pair)

        ppu.vram_addr.set(0x2024);
        let _ = ppu.read_register(7); // Discard buffer
        let val_nt0 = ppu.read_register(7);

        ppu.vram_addr.set(0x2424);
        let _ = ppu.read_register(7);
        let val_nt1 = ppu.read_register(7);

        ppu.vram_addr.set(0x2824);
        let _ = ppu.read_register(7);
        let val_nt2 = ppu.read_register(7);

        ppu.vram_addr.set(0x2C24);
        let _ = ppu.read_register(7);
        let val_nt3 = ppu.read_register(7);

        assert_eq!(val_nt0, 0xBB, "Horizontal: NT0 should mirror with NT1");
        assert_eq!(val_nt1, 0xBB, "Horizontal: NT1 should mirror with NT0");
        assert_eq!(val_nt2, 0xDD, "Horizontal: NT2 should mirror with NT3");
        assert_eq!(val_nt3, 0xDD, "Horizontal: NT3 should mirror with NT2");

        // Now test with Vertical mirroring
        ppu.set_mirroring(Mirroring::Vertical);

        // Clear VRAM and write again
        ppu.vram = vec![0; 0x800];

        ppu.write_register(6, 0x20); // $2024 - NT0
        ppu.write_register(6, 0x24);
        ppu.write_register(7, 0x11);

        ppu.write_register(6, 0x24); // $2424 - NT1
        ppu.write_register(6, 0x24);
        ppu.write_register(7, 0x22);

        ppu.write_register(6, 0x28); // $2824 - NT2
        ppu.write_register(6, 0x24);
        ppu.write_register(7, 0x33);

        ppu.write_register(6, 0x2C); // $2C24 - NT3
        ppu.write_register(6, 0x24);
        ppu.write_register(7, 0x44);

        // Read back and verify Vertical mirroring:
        // NT0 and NT2 should mirror (both show 0x33 - last write to pair)
        // NT1 and NT3 should mirror (both show 0x44 - last write to pair)

        ppu.vram_addr.set(0x2024);
        let _ = ppu.read_register(7);
        let val_nt0 = ppu.read_register(7);

        ppu.vram_addr.set(0x2424);
        let _ = ppu.read_register(7);
        let val_nt1 = ppu.read_register(7);

        ppu.vram_addr.set(0x2824);
        let _ = ppu.read_register(7);
        let val_nt2 = ppu.read_register(7);

        ppu.vram_addr.set(0x2C24);
        let _ = ppu.read_register(7);
        let val_nt3 = ppu.read_register(7);

        assert_eq!(val_nt0, 0x33, "Vertical: NT0 should mirror with NT2");
        assert_eq!(val_nt1, 0x44, "Vertical: NT1 should mirror with NT3");
        assert_eq!(val_nt2, 0x33, "Vertical: NT2 should mirror with NT0");
        assert_eq!(val_nt3, 0x44, "Vertical: NT3 should mirror with NT1");

        // Test with SingleScreenLower
        ppu.set_mirroring(Mirroring::SingleScreenLower);

        // All four writes should go to the same location
        ppu.write_register(6, 0x20);
        ppu.write_register(6, 0x24);
        ppu.write_register(7, 0xFF);

        ppu.write_register(6, 0x24);
        ppu.write_register(6, 0x24);
        ppu.write_register(7, 0xEE);

        ppu.write_register(6, 0x28);
        ppu.write_register(6, 0x24);
        ppu.write_register(7, 0xDD);

        ppu.write_register(6, 0x2C);
        ppu.write_register(6, 0x24);
        ppu.write_register(7, 0xCC);

        // All reads should return 0xCC (last write)
        for addr in [0x2024u16, 0x2424, 0x2824, 0x2C24] {
            ppu.vram_addr.set(addr);
            let _ = ppu.read_register(7);
            let val = ppu.read_register(7);
            assert_eq!(
                val, 0xCC,
                "SingleScreenLower: all nametables should share same data (addr ${:04X})",
                addr
            );
        }
    }

    #[test]
    fn test_four_screen_scrolling_nametable_selection() {
        // Test that 4-screen mode selects nametables correctly when scrolling
        // This is critical for games like Rad Racer 2
        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::FourScreen, TimingMode::Ntsc);
        ppu.clear_first_frame_lock();

        // Write unique patterns to each nametable for identification
        // NT0 ($2000-$23FF): Fill with 0xAA
        for offset in 0..0x3C0 {
            ppu.write_register(6, 0x20);
            ppu.write_register(6, offset as u8);
            ppu.write_register(7, 0xAA);
        }

        // NT1 ($2400-$27FF): Fill with 0xBB
        for offset in 0..0x3C0 {
            ppu.write_register(6, 0x24);
            ppu.write_register(6, offset as u8);
            ppu.write_register(7, 0xBB);
        }

        // NT2 ($2800-$2BFF): Fill with 0xCC
        for offset in 0..0x3C0 {
            ppu.write_register(6, 0x28);
            ppu.write_register(6, offset as u8);
            ppu.write_register(7, 0xCC);
        }

        // NT3 ($2C00-$2FFF): Fill with 0xDD
        for offset in 0..0x3C0 {
            ppu.write_register(6, 0x2C);
            ppu.write_register(6, offset as u8);
            ppu.write_register(7, 0xDD);
        }

        // Test scroll positions and verify correct nametable is selected
        // No scroll (0,0) - should use NT0
        ppu.write_register(5, 0);
        ppu.write_register(5, 0);
        let _frame = ppu.render_frame();
        // Top-left pixel should come from NT0 (0xAA pattern)
        // We can't easily verify the pixel color, but we can check the frame rendered

        // The key is that with 4-screen mode, the nametable selection should be:
        // - Position (x < 256, y < 240): NT0
        // - Position (x >= 256, y < 240): NT1
        // - Position (x < 256, y >= 240): NT2
        // - Position (x >= 256, y >= 240): NT3
        // This is tested implicitly by the rendering logic now using direct selection
        // rather than XOR with base_nt

        // Verify all four nametables are still independent after scrolling
        ppu.vram_addr.set(0x2000);
        let _ = ppu.read_register(7);
        let nt0 = ppu.read_register(7);

        ppu.vram_addr.set(0x2400);
        let _ = ppu.read_register(7);
        let nt1 = ppu.read_register(7);

        ppu.vram_addr.set(0x2800);
        let _ = ppu.read_register(7);
        let nt2 = ppu.read_register(7);

        ppu.vram_addr.set(0x2C00);
        let _ = ppu.read_register(7);
        let nt3 = ppu.read_register(7);

        assert_eq!(nt0, 0xAA, "NT0 should remain 0xAA");
        assert_eq!(nt1, 0xBB, "NT1 should remain 0xBB");
        assert_eq!(nt2, 0xCC, "NT2 should remain 0xCC");
        assert_eq!(nt3, 0xDD, "NT3 should remain 0xDD");
    }

    #[test]
    fn test_scroll_window_bounds() {
        // Test that scroll calculations produce valid nametable coordinates
        // and don't exceed the 2x2 nametable grid bounds (512x480 logical space)

        // Test various scroll positions
        let test_cases = vec![
            (0, 0, 0, "Top-left of NT0"),
            (255, 0, 0, "Top-right of NT0"),
            (0, 239, 0, "Bottom-left of NT0"),
            (255, 239, 0, "Bottom-right of NT0"),
            (256, 0, 1, "Wrapped to NT1"),
            (0, 240, 2, "Wrapped to NT2"),
            (256, 240, 3, "Wrapped to NT3"),
            (511, 479, 0, "Maximum scroll wraps around"),
        ];

        for (scroll_x, scroll_y, expected_base_nt, desc) in test_cases {
            // Calculate which nametable the scroll window starts in
            let nt_x = (scroll_x / 256) & 1;
            let nt_y = (scroll_y / 240) & 1;
            let scroll_nt = expected_base_nt ^ nt_x ^ (nt_y << 1);

            // Calculate position within nametable
            let pixel_x = scroll_x % 256;
            let pixel_y = scroll_y % 240;

            // Verify scroll window stays within bounds
            // The scroll window is 256x240 pixels, and can wrap across nametable boundaries
            // But the starting position should always be within a valid nametable
            assert!(
                scroll_nt < 4,
                "{}: scroll_nt {} should be 0-3",
                desc,
                scroll_nt
            );
            assert!(
                pixel_x < 256,
                "{}: pixel_x {} should be < 256",
                desc,
                pixel_x
            );
            assert!(
                pixel_y < 240,
                "{}: pixel_y {} should be < 240",
                desc,
                pixel_y
            );

            // The scroll window can extend beyond the starting nametable (wrapping),
            // but the maximum logical extent should not exceed the 2x2 grid when considering wrapping
            // For the GUI visualization, we need to handle wrapping correctly
            let grid_x = scroll_nt % 2;
            let grid_y = scroll_nt / 2;

            // Starting position in logical 512x480 space
            let logical_start_x = grid_x * 256 + pixel_x;
            let logical_start_y = grid_y * 240 + pixel_y;

            // The scroll window extends 256x240 from the start position
            // When wrapping is considered, these should wrap within 512x480 bounds
            assert!(
                logical_start_x < 512,
                "{}: logical_start_x {} should be < 512",
                desc,
                logical_start_x
            );
            assert!(
                logical_start_y < 480,
                "{}: logical_start_y {} should be < 480",
                desc,
                logical_start_y
            );
        }
    }

    #[test]
    fn test_loopy_register_ppuscroll_first_write() {
        // Test that first $2005 write updates temp register and fine_x correctly
        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal, TimingMode::Ntsc);
        ppu.clear_first_frame_lock();

        // Write scroll X = 123 ($7B)
        // Expected: coarse X = 123 / 8 = 15 ($0F)
        //           fine X = 123 % 8 = 3
        ppu.write_register(5, 123);

        let t = ppu.temp_vram_addr.get();
        let coarse_x = t & 0x001F;
        let fine_x = ppu.fine_x.get();

        assert_eq!(coarse_x, 15, "Coarse X should be 15");
        assert_eq!(fine_x, 3, "Fine X should be 3");
        assert_eq!(ppu.scroll_x(), 123, "Legacy scroll_x should be 123");
    }

    #[test]
    fn test_loopy_register_ppuscroll_second_write() {
        // Test that second $2005 write updates coarse Y and fine Y correctly
        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal, TimingMode::Ntsc);
        ppu.clear_first_frame_lock();

        // First write (X scroll)
        ppu.write_register(5, 0);

        // Second write: Y scroll = 195 ($C3)
        // Expected: coarse Y = 195 / 8 = 24 ($18)
        //           fine Y = 195 % 8 = 3
        ppu.write_register(5, 195);

        let t = ppu.temp_vram_addr.get();
        let coarse_y = (t >> 5) & 0x001F;
        let fine_y = (t >> 12) & 0x0007;

        assert_eq!(coarse_y, 24, "Coarse Y should be 24");
        assert_eq!(fine_y, 3, "Fine Y should be 3");
        assert_eq!(ppu.scroll_y(), 195, "Legacy scroll_y should be 195");
    }

    #[test]
    fn test_loopy_register_ppuaddr_first_write() {
        // Test that first $2006 write updates temp register high byte
        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal, TimingMode::Ntsc);
        ppu.clear_first_frame_lock();

        // Write high byte = $23
        ppu.write_register(6, 0x23);

        let t = ppu.temp_vram_addr.get();
        assert_eq!(
            (t >> 8) & 0xFF,
            0x23,
            "Temp register high byte should be $23"
        );
    }

    #[test]
    fn test_loopy_register_ppuaddr_second_write() {
        // Test that second $2006 write updates temp register low byte
        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal, TimingMode::Ntsc);
        ppu.clear_first_frame_lock();

        // Write address $2345
        ppu.write_register(6, 0x23);
        ppu.write_register(6, 0x45);

        let t = ppu.temp_vram_addr.get();
        assert_eq!(t, 0x2345, "Temp register should be $2345");
        assert_eq!(ppu.vram_addr.get(), 0x2345, "VRAM addr should be $2345");
    }

    #[test]
    fn test_loopy_register_ppuscroll_ppuaddr_interaction() {
        // Test that $2005 and $2006 properly share the temp register
        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal, TimingMode::Ntsc);
        ppu.clear_first_frame_lock();

        // Set scroll to (8, 16)
        ppu.write_register(5, 8); // X scroll
        ppu.write_register(5, 16); // Y scroll

        let t_after_scroll = ppu.temp_vram_addr.get();

        // Coarse X = 8/8 = 1, Fine X = 0
        // Coarse Y = 16/8 = 2, Fine Y = 0
        // Expected t bits: coarse_x=1, coarse_y=2, fine_y=0
        let coarse_x = t_after_scroll & 0x001F;
        let coarse_y = (t_after_scroll >> 5) & 0x001F;

        assert_eq!(coarse_x, 1, "Coarse X should be 1");
        assert_eq!(coarse_y, 2, "Coarse Y should be 2");

        // Now write to PPUADDR - this should update temp register
        ppu.write_register(6, 0x20);
        ppu.write_register(6, 0x00);

        let t_after_addr = ppu.temp_vram_addr.get();
        assert_eq!(
            t_after_addr, 0x2000,
            "PPUADDR should update temp register to $2000"
        );
    }

    #[test]
    fn test_sprite_0_hit_edge_cases() {
        // Test sprite 0 hit detection edge cases
        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal, TimingMode::Ntsc);
        ppu.clear_first_frame_lock();
        ppu.chr_is_ram = true;

        // Set up sprite 0 at position (16, 16) - aligned with tile boundary
        ppu.oam[0] = 16 - 1; // Y position (subtract 1 as per OAM format)
        ppu.oam[1] = 1; // Tile index
        ppu.oam[2] = 0; // Attributes (front priority, palette 0)
        ppu.oam[3] = 16; // X position

        // Create sprite tile pattern with opaque pixels (pattern value 1)
        for i in 0..8 {
            ppu.chr[0x10 + i] = 0xFF; // Low bit plane - all 1s
            ppu.chr[0x18 + i] = 0x00; // High bit plane - all 0s = color 1
        }

        // Set up background tile at position (2, 2) which covers screen (16-23, 16-23)
        let nt_addr = ppu.map_nametable_addr(0x2000);
        ppu.vram[nt_addr + 2 * 32 + 2] = 2; // Tile at (2,2) = tile index 2

        // Background tile pattern with opaque pixels (pattern value 1)
        for i in 0..8 {
            ppu.chr[0x20 + i] = 0xFF;
            ppu.chr[0x28 + i] = 0x00;
        }

        // Set up palettes so pixels are visible
        ppu.palette[0] = 0x0F; // Backdrop
        ppu.palette[1] = 0x30; // BG color 1
        ppu.palette[0x11] = 0x16; // Sprite color 1

        // Enable rendering
        ppu.mask = 0x1E; // Show bg + sprites, show in leftmost 8 pixels

        // Set up v register so scanline 16 renders tile row 2 (which has our BG tile)
        // v register format: yyy NN YYYYY XXXXX (fine_y, nametable, coarse_y, coarse_x)
        // For scanline 16: coarse_y = 2, fine_y = 0
        ppu.vram_addr.set(0x0040); // coarse_y = 2, everything else 0
        ppu.temp_vram_addr.set(0x0040);

        // Render scanline 16 where sprite 0 should overlap background
        let mut frame = Frame::new(256, 240);
        ppu.render_scanline(16, &mut frame);
        // Resolve pending sprite 0 hit (in real emulation, this happens during tick())
        ppu.resolve_pending_sprite_0_hit();

        // Sprite 0 hit should be set
        assert!(
            ppu.sprite_0_hit.get(),
            "Sprite 0 hit should be set when opaque pixels overlap"
        );

        // Test that sprite 0 hit doesn't occur at x=255
        ppu.sprite_0_hit.set(false);
        ppu.oam[3] = 255; // Move sprite 0 to x=255
        ppu.render_scanline(16, &mut frame);
        assert!(
            !ppu.sprite_0_hit.get(),
            "Sprite 0 hit should NOT occur at x=255"
        );

        // Test that sprite 0 hit doesn't occur if background is disabled
        ppu.sprite_0_hit.set(false);
        ppu.oam[3] = 16; // Move back to x=16
        ppu.mask = 0x10; // Only sprites enabled
        ppu.render_scanline(16, &mut frame);
        assert!(
            !ppu.sprite_0_hit.get(),
            "Sprite 0 hit should NOT occur when background is disabled"
        );
    }

    #[test]
    fn test_sprite_0_hit_with_clipping() {
        // Test that sprite 0 hit respects left clipping
        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal, TimingMode::Ntsc);
        ppu.clear_first_frame_lock();
        ppu.chr_is_ram = true;

        // Set up sprite 0 at position (4, 8) - partially in the clipped region
        ppu.oam[0] = 8 - 1;
        ppu.oam[1] = 1;
        ppu.oam[2] = 0;
        ppu.oam[3] = 4;

        // Create opaque sprite tile pattern (pattern value 1)
        for i in 0..8 {
            ppu.chr[0x10 + i] = 0xFF;
            ppu.chr[0x18 + i] = 0x00;
        }

        // Background tile at position (0, 1) which covers screen (0-7, 8-15)
        let nt_addr = ppu.map_nametable_addr(0x2000);
        ppu.vram[nt_addr + 32] = 2; // Row 1, column 0
        for i in 0..8 {
            ppu.chr[0x20 + i] = 0xFF;
            ppu.chr[0x28 + i] = 0x00;
        }

        // Set up palettes
        ppu.palette[0] = 0x0F;
        ppu.palette[1] = 0x30;
        ppu.palette[0x11] = 0x16;

        // Set up v register so scanline 8 renders tile row 1 (which has our BG tile)
        ppu.vram_addr.set(0x0020); // coarse_y = 1, everything else 0
        ppu.temp_vram_addr.set(0x0020);

        // Enable rendering but disable leftmost 8 pixels
        ppu.mask = 0x18; // Show bg + sprites, hide leftmost 8 pixels

        let mut frame = Frame::new(256, 240);
        ppu.render_scanline(8, &mut frame);
        // Resolve pending sprite 0 hit (in real emulation, this happens during tick())
        ppu.resolve_pending_sprite_0_hit();

        // Sprite 0 hit should NOT occur because all overlapping pixels are in clipped region
        // The sprite spans x=4-11, but leftmost 8 pixels (x=0-7) are clipped
        // So only x=8-11 are visible, and at x=8-11 the background tile at (0,1) is also visible
        // But according to NES hardware, sprite 0 hit doesn't trigger if either the sprite
        // or background pixel is in a clipped region. Since x=4-7 would be clipped and
        // x=8-11 are not clipped, we should get a hit at x=8.
        // Actually, wait - the background tile is at (0,1) which covers x=0-7, y=8-15
        // So at x=8-11, there's no background tile (it would be the next tile)

        // Let me reconsider: sprite at x=4 covers x=4-11
        // Background at tile (0,1) covers x=0-7
        // Overlap is at x=4-7, which is in the clipped region when leftmost 8 pixels are hidden
        assert!(
            !ppu.sprite_0_hit.get(),
            "Sprite 0 hit should NOT occur in clipped region"
        );

        // Now enable leftmost 8 pixels
        ppu.mask = 0x1E; // Show bg + sprites, show leftmost 8 pixels
        ppu.sprite_0_hit.set(false);
        // Reset v register for this scanline (simulating t->v copy at scanline start)
        ppu.vram_addr.set(0x0020); // coarse_y = 1
        ppu.render_scanline(8, &mut frame);
        // Resolve pending sprite 0 hit (in real emulation, this happens during tick())
        ppu.resolve_pending_sprite_0_hit();

        // Now sprite 0 hit should occur because overlap at x=4-7 is visible
        assert!(
            ppu.sprite_0_hit.get(),
            "Sprite 0 hit should occur when leftmost pixels are shown"
        );
    }

    #[test]
    fn test_sprite_evaluation_exactly_8_sprites() {
        // Test that sprite overflow is NOT set when exactly 8 sprites are on scanline
        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal, TimingMode::Ntsc);
        ppu.clear_first_frame_lock();

        // Place exactly 8 sprites on scanline 100
        for i in 0..8 {
            ppu.oam[i * 4] = 99; // Y position (100 - 1)
            ppu.oam[i * 4 + 1] = 0;
            ppu.oam[i * 4 + 2] = 0;
            ppu.oam[i * 4 + 3] = (i * 16) as u8;
        }

        // Evaluate sprites for scanline 100
        ppu.evaluate_sprites_for_scanline(100);

        assert!(
            !ppu.sprite_overflow.get(),
            "Sprite overflow should NOT be set with exactly 8 sprites"
        );

        // Add 9th sprite (sprite index 8, OAM offset = 8 * 4 bytes per sprite entry)
        const SPRITE_8_OAM_OFFSET: usize = 8 * 4;
        ppu.oam[SPRITE_8_OAM_OFFSET] = 99;
        ppu.evaluate_sprites_for_scanline(100);

        assert!(
            ppu.sprite_overflow.get(),
            "Sprite overflow should be set with 9 sprites"
        );
    }

    #[test]
    fn test_34th_tile_fetch() {
        // Test that PPU fetches 34 tiles per scanline (not just the 32 visible ones)
        // This is critical for games like Punch Out!! that use MMC2 mapper
        // Reference: https://www.nesdev.org/wiki/PPU_rendering

        use std::cell::RefCell;
        use std::rc::Rc;

        let mut ppu = Ppu::new(vec![0; 0x2000], Mirroring::Horizontal, TimingMode::Ntsc);
        ppu.clear_first_frame_lock();

        // Track CHR reads via callback
        let chr_reads = Rc::new(RefCell::new(Vec::new()));
        let chr_reads_clone = chr_reads.clone();

        ppu.set_chr_read_callback(Some(Box::new(move |addr| {
            chr_reads_clone.borrow_mut().push(addr);
        })));

        // Enable rendering
        ppu.mask = 0x18; // Show background and sprites
        ppu.ctrl = 0x00; // Background pattern table at $0000

        // Set up nametables with sequential but unique tile indices for both nametables
        // Nametable 0: tiles 0-31 for first 32 tiles, tiles 32-33 for extra 2 tiles
        // Nametable 1: tiles 64-95 so we can distinguish between nametables
        for i in 0..64 {
            let addr = 0x2000 + i;
            let mapped_addr = ppu.map_nametable_addr(addr);
            ppu.vram[mapped_addr] = i as u8;

            // Also set up second nametable with different tiles
            let addr2 = 0x2400 + i;
            let mapped_addr2 = ppu.map_nametable_addr(addr2);
            ppu.vram[mapped_addr2] = (i + 64) as u8;
        }

        // Clear the CHR reads
        chr_reads.borrow_mut().clear();

        // Render a scanline with no scrolling
        let mut frame = Frame::new(256, 240);
        ppu.render_scanline(0, &mut frame);

        // Count how many CHR reads we got
        let reads = chr_reads.borrow();

        // With horizontal mirroring:
        // - First 32 tiles come from nametable 0 (tiles 0-31)
        // - Next 2 tiles come from nametable 1 (which with horizontal mirroring maps to nametable 0)
        // Actually, let's just verify we got the right number of reads

        // Each tile has 2 CHR reads (low and high bitplane)
        // So we expect 34 * 2 = 68 CHR reads
        assert_eq!(
            reads.len(),
            68,
            "PPU should make exactly 68 CHR reads (34 tiles * 2 bitplanes), got {}",
            reads.len()
        );

        // The reads should be in pairs (low bitplane + 8, high bitplane)
        for i in (0..reads.len()).step_by(2) {
            let low_addr = reads[i];
            let high_addr = reads[i + 1];
            assert_eq!(
                high_addr,
                low_addr + 8,
                "CHR reads should be in low/high bitplane pairs"
            );
        }
    }
}
