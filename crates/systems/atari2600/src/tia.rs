//! TIA (Television Interface Adapter) - Video and audio chip for Atari 2600
//!
//! The TIA handles all video and audio generation for the Atari 2600.
//! Unlike modern systems, it has no framebuffer and generates video scanline-by-scanline.
//!
//! # Video Generation
//!
//! The TIA generates NTSC video signals with the following capabilities:
//!
//! ## Resolution and Timing
//! - **Visible Area**: 160x192 pixels (NTSC)
//! - **Total Scanlines**: 262 (NTSC), including overscan and vblank
//! - **Color Clock**: 3.579545 MHz (NTSC)
//! - **Pixels per Scanline**: 160 visible, 228 total (including blanking)
//!
//! ## Graphics Objects
//!
//! The TIA can render several types of graphics objects simultaneously:
//!
//! ### Playfield
//! - 40-bit wide bitmap (20 pixels visible, each bit controls 4 color clocks)
//! - Split into 3 registers: PF0 (4 bits), PF1 (8 bits), PF2 (8 bits)
//! - Can be **mirrored** (left half repeats mirrored on right) or **repeated** (both halves identical)
//! - **Score mode**: Left half uses player 0 color, right half uses player 1 color
//! - **Priority mode**: Playfield drawn in front of players instead of behind
//!
//! ### Players (Sprites)
//! - 2 independent 8-pixel wide sprites (Player 0 and Player 1)
//! - Each player has:
//!   - Graphics register (8 bits = 8 pixels)
//!   - Horizontal position (set by strobing RESP0/RESP1 registers)
//!   - Color register (COLUP0/COLUP1)
//!   - Reflection flag (REFP0/REFP1)
//! - Can be sized (1x, 2x, 4x), duplicated (close, medium, wide), and positioned (NUSIZ registers)
//!
//! ### Missiles
//! - 2 missiles (one per player), typically 1 pixel wide
//! - Share color with their associated player
//! - Can be enabled/disabled independently (ENAM0/ENAM1)
//! - Horizontal positioning similar to players (RESM0/RESM1)
//!
//! ### Ball
//! - Single 1-pixel object
//! - Uses playfield color
//! - Can be enabled/disabled (ENABL)
//! - Horizontal positioning (RESBL)
//!
//! ## Colors
//!
//! The TIA uses a **128-color NTSC palette**:
//! - Upper 4 bits: Hue (0-15, representing different colors)
//! - Lower 3 bits: Luminance (0-7, controlling brightness)
//! - Bit 0 is unused in color registers
//!
//! This implementation includes a proper NTSC palette table mapping these values to RGB.
//!
//! ## Priority and Collision
//!
//! **Drawing Priority** (when playfield priority is off - default):
//! 1. Player 0 / Missile 0
//! 2. Player 1 / Missile 1
//! 3. Ball
//! 4. Playfield
//! 5. Background
//!
//! **Drawing Priority** (when playfield priority is on):
//! 1. Playfield / Ball
//! 2. Player 0 / Missile 0
//! 3. Player 1 / Missile 1
//! 4. Background
//!
//! **Collision Detection**: The TIA has hardware collision detection registers that set bits
//! when different objects overlap. This implementation tracks collisions pixel-by-pixel during
//! rendering and updates all 8 collision registers (CXM0P, CXM1P, CXP0FB, CXP1FB, CXM0FB, CXM1FB,
//! CXBLPF, CXPPMM). Collision registers can be cleared using CXCLR (0x2C).
//!
//! # Audio Generation
//!
//! The TIA has 2 audio channels, each with:
//! - **Control register** (AUDC0/AUDC1): 4 bits selecting waveform type (0-15)
//! - **Frequency register** (AUDF0/AUDF1): 5 bits controlling pitch (0-31)
//! - **Volume register** (AUDV0/AUDV1): 4 bits controlling volume (0-15)
//!
//! Audio synthesis uses polynomial counters to generate 16 different waveform types:
//! - **Type 0, 11**: Set to 1 (always on - pure DC)
//! - **Type 1**: 4-bit polynomial (buzzy tone)
//! - **Type 2**: Division by 2 (pure tone, one octave lower)
//! - **Type 3**: 4-bit AND 5-bit poly (complex tone)
//! - **Type 4, 5**: Pure tone via division
//! - **Type 6, 10**: Division by 31 (low pure tone)
//! - **Type 7, 9**: 5-bit polynomial (white noise-like)
//! - **Type 8**: 5-bit polynomial (noise)
//! - **Type 12, 13**: Pure tone with 4-bit poly
//! - **Type 14**: 4-bit polynomial
//! - **Type 15**: 4-bit XOR 5-bit (complex noise)
//!
//! # Implementation Details
//!
//! ## Rendering Model
//! This implementation uses **frame-based rendering** rather than cycle-accurate scanline generation:
//! - TIA state (colors, graphics) is updated during CPU execution
//! - At frame end, all 192 visible scanlines are rendered at once
//! - Each pixel's color is determined by checking all graphics objects at that position
//!
//! ## Implemented Features
//!
//! 1. **Player/Missile Sizing (NUSIZ)**: Full support for sprite sizing (1x, 2x, 4x) and duplication modes
//! 2. **Collision Detection**: All 8 collision registers with pixel-perfect detection
//! 3. **Delayed Graphics (VDELP0/VDELP1)**: Player graphics can be delayed by one scanline
//!
//! ## Known Limitations
//!
//! 1. **Frame-based rendering**: Uses scanline state latching rather than cycle-accurate generation
//! 2. **Paddle controllers**: Not implemented (INPT0-INPT3 always return 0)
//!
//! These limitations represent acceptable trade-offs for a functional emulator. Most games
//! will work correctly with the current implementation.

use emu_core::apu::PolynomialCounter;
use emu_core::logging::{LogCategory, LogConfig, LogLevel};
use serde::{Deserialize, Serialize};

/// Per-scanline snapshot of TIA state for rendering
#[derive(Debug, Clone, Copy, Default)]
struct ScanlineState {
    vblank: bool,
    pf0: u8,
    pf1: u8,
    pf2: u8,
    playfield_reflect: bool,
    playfield_priority: bool,
    colubk: u8,
    colupf: u8,
    colup0: u8,
    colup1: u8,
    grp0: u8,
    grp1: u8,
    #[allow(dead_code)] // Stored for potential future rendering enhancements
    grp0_delayed: u8,
    #[allow(dead_code)] // Stored for potential future rendering enhancements
    grp1_delayed: u8,
    player0_x: u8,
    player1_x: u8,
    player0_reflect: bool,
    player1_reflect: bool,
    nusiz0: u8,
    nusiz1: u8,
    #[allow(dead_code)] // Stored for potential future rendering enhancements
    vdelp0: bool,
    #[allow(dead_code)] // Stored for potential future rendering enhancements
    vdelp1: bool,
    enam0: bool,
    enam1: bool,
    missile0_x: u8,
    missile1_x: u8,
    enabl: bool,
    ball_x: u8,
    ball_size: u8, // Ball size (1, 2, 4, or 8 pixels)
}

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
    grp0: u8,     // Player 0 graphics
    grp1: u8,     // Player 1 graphics
    grp0_old: u8, // Previous GRP0 value for delayed graphics
    grp1_old: u8, // Previous GRP1 value for delayed graphics
    player0_x: u8,
    player1_x: u8,
    player0_reflect: bool,
    player1_reflect: bool,
    nusiz0: u8,   // Player 0 number and size
    nusiz1: u8,   // Player 1 number and size
    vdelp0: bool, // Player 0 delayed graphics enable
    vdelp1: bool, // Player 1 delayed graphics enable

    // Missiles
    enam0: bool, // Missile 0 enable
    enam1: bool, // Missile 1 enable
    missile0_x: u8,
    missile1_x: u8,

    // Ball
    enabl: bool, // Ball enable
    ball_x: u8,
    ball_size: u8, // Ball size (1, 2, 4, or 8 pixels) from CTRLPF bits 4-5

    // Collision detection registers (CXM0P, CXM1P, CXP0FB, CXP1FB, CXM0FB, CXM1FB, CXBLPF, CXPPMM)
    cxm0p: u8,  // Missile 0 to Player collisions
    cxm1p: u8,  // Missile 1 to Player collisions
    cxp0fb: u8, // Player 0 to Playfield/Ball collisions
    cxp1fb: u8, // Player 1 to Playfield/Ball collisions
    cxm0fb: u8, // Missile 0 to Playfield/Ball collisions
    cxm1fb: u8, // Missile 1 to Playfield/Ball collisions
    cxblpf: u8, // Ball to Playfield collisions
    cxppmm: u8, // Player and Missile collisions

    // Horizontal motion
    hmp0: i8,
    hmp1: i8,
    hmm0: i8,
    hmm1: i8,
    hmbl: i8,

    // Input ports (fire buttons and paddles)
    // INPT4/INPT5: Joystick fire buttons (bit 7: 0=pressed, 1=not pressed)
    inpt4: u8, // Player 0 fire button
    inpt5: u8, // Player 1 fire button

    // Current scanline and pixel position
    scanline: u16,
    pixel: u16,

    // Monotonic scanline counter for debug/telemetry (does not wrap)
    #[serde(skip)]
    scanline_counter: u64,

    // Per-scanline state snapshots for rendering
    #[serde(skip)]
    scanline_states: Vec<ScanlineState>,

    // Audio channels
    #[serde(skip)]
    audio0: PolynomialCounter,
    #[serde(skip)]
    audio1: PolynomialCounter,

    // Audio registers
    audc0: u8,
    audc1: u8,
    audf0: u8,
    audf1: u8,
    audv0: u8,
    audv1: u8,

    // Debug/write statistics (per-frame; managed by system)
    #[serde(skip)]
    writes_total: u64,
    #[serde(skip)]
    writes_vsync: u64,
    #[serde(skip)]
    writes_vblank: u64,
    #[serde(skip)]
    writes_pf: u64,
    #[serde(skip)]
    writes_pf_nonzero: u64,
    #[serde(skip)]
    writes_grp0: u64,
    #[serde(skip)]
    writes_grp0_nonzero: u64,
    #[serde(skip)]
    writes_grp1: u64,
    #[serde(skip)]
    writes_grp1_nonzero: u64,
    #[serde(skip)]
    writes_colors: u64,
    #[serde(skip)]
    writes_colors_nonzero: u64,

    // Cached visible window start (to prevent vertical jumping)
    #[serde(skip)]
    cached_visible_start: Option<u16>,
}

impl Default for Tia {
    fn default() -> Self {
        Self::new()
    }
}

impl Tia {
    // Horizontal timing: ~68 color clocks of horizontal blank, 160 visible
    const HBLANK_COLOR_CLOCKS: i16 = 68;

    /// Get current visible x position (accounting for horizontal blank)
    fn current_visible_x(&self) -> u8 {
        let x = (self.pixel as i16) - Self::HBLANK_COLOR_CLOCKS;
        x.clamp(0, 159) as u8
    }

    /// Apply horizontal motion to a position
    fn apply_motion(&self, pos: u8, motion: i8) -> u8 {
        let p = pos as i16;
        let m = motion as i16;
        (p + m).clamp(0, 159) as u8
    }

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
            grp0_old: 0,
            grp1_old: 0,
            player0_x: 0,
            player1_x: 0,
            player0_reflect: false,
            player1_reflect: false,
            nusiz0: 0,
            nusiz1: 0,
            vdelp0: false,
            vdelp1: false,
            enam0: false,
            enam1: false,
            missile0_x: 0,
            missile1_x: 0,
            enabl: false,
            ball_x: 0,
            ball_size: 1, // Default to 1 pixel
            cxm0p: 0,
            cxm1p: 0,
            cxp0fb: 0,
            cxp1fb: 0,
            cxm0fb: 0,
            cxm1fb: 0,
            cxblpf: 0,
            cxppmm: 0,
            hmp0: 0,
            hmp1: 0,
            hmm0: 0,
            hmm1: 0,
            hmbl: 0,
            inpt4: 0x80, // Not pressed (bit 7 = 1)
            inpt5: 0x80, // Not pressed (bit 7 = 1)
            scanline: 0,
            pixel: 0,

            scanline_counter: 0,

            scanline_states: vec![ScanlineState::default(); 262],

            audio0: PolynomialCounter::new(),
            audio1: PolynomialCounter::new(),

            audc0: 0,
            audc1: 0,
            audf0: 0,
            audf1: 0,
            audv0: 0,
            audv1: 0,

            writes_total: 0,
            writes_vsync: 0,
            writes_vblank: 0,
            writes_pf: 0,
            writes_pf_nonzero: 0,
            writes_grp0: 0,
            writes_grp0_nonzero: 0,
            writes_grp1: 0,
            writes_grp1_nonzero: 0,
            writes_colors: 0,
            writes_colors_nonzero: 0,

            cached_visible_start: None,
        }
    }

    pub fn reset_write_stats(&mut self) {
        self.writes_total = 0;
        self.writes_vsync = 0;
        self.writes_vblank = 0;
        self.writes_pf = 0;
        self.writes_pf_nonzero = 0;
        self.writes_grp0 = 0;
        self.writes_grp0_nonzero = 0;
        self.writes_grp1 = 0;
        self.writes_grp1_nonzero = 0;
        self.writes_colors = 0;
        self.writes_colors_nonzero = 0;
    }

    pub fn write_stats(&self) -> (u64, u64, u64, u64, u64, u64, u64, u64, u64, u64, u64) {
        (
            self.writes_total,
            self.writes_vsync,
            self.writes_vblank,
            self.writes_pf,
            self.writes_grp0,
            self.writes_grp1,
            self.writes_colors,
            self.writes_pf_nonzero,
            self.writes_grp0_nonzero,
            self.writes_grp1_nonzero,
            self.writes_colors_nonzero,
        )
    }

    /// Set fire button state for a player (0 or 1)
    ///
    /// Fire button state in TIA uses active-low logic for bit 7:
    /// - pressed = true -> INPT bit 7 = 0
    /// - pressed = false -> INPT bit 7 = 1
    pub fn set_fire_button(&mut self, player: u8, pressed: bool) {
        let value = if pressed { 0x00 } else { 0x80 };
        match player {
            0 => self.inpt4 = value,
            1 => self.inpt5 = value,
            _ => {}
        }
    }

    /// Get a monotonically increasing scanline counter (increments once per scanline)
    pub fn get_scanline_counter(&self) -> u64 {
        self.scanline_counter
    }

    /// Latch current TIA state for a scanline (for later rendering)
    fn latch_scanline_state(&mut self, scanline: u16) {
        let idx = (scanline as usize).min(261);
        self.scanline_states[idx] = ScanlineState {
            vblank: self.vblank,
            pf0: self.pf0,
            pf1: self.pf1,
            pf2: self.pf2,
            playfield_reflect: self.playfield_reflect,
            playfield_priority: self.playfield_priority,
            colubk: self.colubk,
            colupf: self.colupf,
            colup0: self.colup0,
            colup1: self.colup1,
            grp0: if self.vdelp0 {
                self.grp0_old
            } else {
                self.grp0
            },
            grp1: if self.vdelp1 {
                self.grp1_old
            } else {
                self.grp1
            },
            grp0_delayed: self.grp0_old,
            grp1_delayed: self.grp1_old,
            player0_x: self.player0_x,
            player1_x: self.player1_x,
            player0_reflect: self.player0_reflect,
            player1_reflect: self.player1_reflect,
            nusiz0: self.nusiz0,
            nusiz1: self.nusiz1,
            vdelp0: self.vdelp0,
            vdelp1: self.vdelp1,
            enam0: self.enam0,
            enam1: self.enam1,
            missile0_x: self.missile0_x,
            missile1_x: self.missile1_x,
            enabl: self.enabl,
            ball_x: self.ball_x,
            ball_size: self.ball_size,
        };
    }

    /// Reset TIA to power-on state
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Write to TIA register
    pub fn write(&mut self, addr: u8, val: u8) {
        self.writes_total = self.writes_total.saturating_add(1);

        // Comprehensive write logging (first 1000 writes only)
        if self.writes_total <= 1000
            && LogConfig::global().should_log(LogCategory::PPU, LogLevel::Debug)
        {
            eprintln!(
                "[TIA WRITE #{}] addr=0x{:02X} val=0x{:02X} scanline={}",
                self.writes_total, addr, val, self.scanline
            );
        }

        match addr {
            0x00 => {
                self.writes_vsync = self.writes_vsync.saturating_add(1);
                self.vsync = (val & 0x02) != 0;
            }
            0x01 => {
                self.writes_vblank = self.writes_vblank.saturating_add(1);
                self.vblank = (val & 0x02) != 0;
            }
            0x02 => {} // WSYNC - handled by bus
            0x03 => {} // RSYNC

            // Player 0
            0x04 => {
                // NUSIZ0 - Player 0 number and size
                self.nusiz0 = val;
            }
            0x05 => {
                // NUSIZ1 - Player 1 number and size
                self.nusiz1 = val;
            }
            0x06 => {
                self.writes_colors = self.writes_colors.saturating_add(1);
                if val != 0 {
                    self.writes_colors_nonzero = self.writes_colors_nonzero.saturating_add(1);
                }
                self.colup0 = val;
            }
            0x07 => {
                self.writes_colors = self.writes_colors.saturating_add(1);
                if val != 0 {
                    self.writes_colors_nonzero = self.writes_colors_nonzero.saturating_add(1);
                }
                self.colup1 = val;
            }
            0x08 => {
                self.writes_colors = self.writes_colors.saturating_add(1);
                if val != 0 {
                    self.writes_colors_nonzero = self.writes_colors_nonzero.saturating_add(1);
                }
                self.colupf = val;
            }
            0x09 => {
                self.writes_colors = self.writes_colors.saturating_add(1);
                if val != 0 {
                    self.writes_colors_nonzero = self.writes_colors_nonzero.saturating_add(1);
                }
                self.colubk = val;
            }

            // Playfield control
            0x0A => {
                self.playfield_reflect = (val & 0x01) != 0;
                self.playfield_score_mode = (val & 0x02) != 0;
                self.playfield_priority = (val & 0x04) != 0;
                // Bits 4-5 control ball size: 00=1px, 01=2px, 10=4px, 11=8px
                self.ball_size = match (val >> 4) & 0x03 {
                    0x00 => 1,
                    0x01 => 2,
                    0x02 => 4,
                    0x03 => 8,
                    _ => 1,
                };
            }

            // Player reflect
            0x0B => {
                self.player0_reflect = (val & 0x08) != 0;
            }
            0x0C => {
                self.player1_reflect = (val & 0x08) != 0;
            }

            // Playfield
            0x0D => {
                self.writes_pf = self.writes_pf.saturating_add(1);
                if val != 0 {
                    self.writes_pf_nonzero = self.writes_pf_nonzero.saturating_add(1);
                }
                self.pf0 = val;
            }
            0x0E => {
                self.writes_pf = self.writes_pf.saturating_add(1);
                if val != 0 {
                    self.writes_pf_nonzero = self.writes_pf_nonzero.saturating_add(1);
                }
                self.pf1 = val;
            }
            0x0F => {
                self.writes_pf = self.writes_pf.saturating_add(1);
                if val != 0 {
                    self.writes_pf_nonzero = self.writes_pf_nonzero.saturating_add(1);
                }
                self.pf2 = val;
            }

            // Player position resets (RESP0, RESP1, RESM0, RESM1, RESBL)
            0x10 => self.player0_x = self.current_visible_x(),
            0x11 => self.player1_x = self.current_visible_x(),
            0x12 => self.missile0_x = self.current_visible_x(),
            0x13 => self.missile1_x = self.current_visible_x(),
            0x14 => self.ball_x = self.current_visible_x(),

            // Audio
            0x15 => {
                self.audc0 = val & 0x0F;
                self.audio0.control = self.audc0;
            }
            0x16 => {
                self.audc1 = val & 0x0F;
                self.audio1.control = self.audc1;
            }
            0x17 => {
                self.audf0 = val & 0x1F;
                self.audio0.frequency = self.audf0;
            }
            0x18 => {
                self.audf1 = val & 0x1F;
                self.audio1.frequency = self.audf1;
            }
            0x19 => {
                self.audv0 = val & 0x0F;
                self.audio0.volume = self.audv0;
            }
            0x1A => {
                self.audv1 = val & 0x0F;
                self.audio1.volume = self.audv1;
            }

            // Player graphics
            0x1B => {
                self.writes_grp0 = self.writes_grp0.saturating_add(1);
                if val != 0 {
                    self.writes_grp0_nonzero = self.writes_grp0_nonzero.saturating_add(1);
                    if LogConfig::global().should_log(LogCategory::PPU, LogLevel::Debug) {
                        eprintln!("[TIA] GRP0 = 0x{:02X} at scanline {}", val, self.scanline);
                    }
                }
                self.grp0_old = self.grp0; // Save old value before writing new
                self.grp0 = val;
            }
            0x1C => {
                self.writes_grp1 = self.writes_grp1.saturating_add(1);
                if val != 0 {
                    self.writes_grp1_nonzero = self.writes_grp1_nonzero.saturating_add(1);
                    if LogConfig::global().should_log(LogCategory::PPU, LogLevel::Debug) {
                        eprintln!("[TIA] GRP1 = 0x{:02X} at scanline {}", val, self.scanline);
                    }
                }
                self.grp1_old = self.grp1; // Save old value before writing new
                self.grp1 = val;
            }

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

            // Delayed graphics enable
            0x25 => self.vdelp0 = (val & 0x01) != 0, // VDELP0
            0x26 => self.vdelp1 = (val & 0x01) != 0, // VDELP1

            // Apply horizontal motion (HMOVE)
            0x2A => {
                self.player0_x = self.apply_motion(self.player0_x, self.hmp0);
                self.player1_x = self.apply_motion(self.player1_x, self.hmp1);
                self.missile0_x = self.apply_motion(self.missile0_x, self.hmm0);
                self.missile1_x = self.apply_motion(self.missile1_x, self.hmm1);
                self.ball_x = self.apply_motion(self.ball_x, self.hmbl);
            }

            // Clear horizontal motion
            0x2B => {
                self.hmp0 = 0;
                self.hmp1 = 0;
                self.hmm0 = 0;
                self.hmm1 = 0;
                self.hmbl = 0;
            }

            // Clear collision detection latches (CXCLR)
            0x2C => {
                self.cxm0p = 0;
                self.cxm1p = 0;
                self.cxp0fb = 0;
                self.cxp1fb = 0;
                self.cxm0fb = 0;
                self.cxm1fb = 0;
                self.cxblpf = 0;
                self.cxppmm = 0;
            }

            _ => {}
        }

        // Latch state for current scanline after register write
        // (games often write graphics data during the scanline)
        self.latch_scanline_state(self.scanline);
    }

    /// Read from TIA register (collision detection and input)
    pub fn read(&self, addr: u8) -> u8 {
        // TIA read registers are for collision detection and input
        match addr & 0x0F {
            0x00 => self.cxm0p,  // Missile 0 to Player collisions
            0x01 => self.cxm1p,  // Missile 1 to Player collisions
            0x02 => self.cxp0fb, // Player 0 to Playfield/Ball collisions
            0x03 => self.cxp1fb, // Player 1 to Playfield/Ball collisions
            0x04 => self.cxm0fb, // Missile 0 to Playfield/Ball collisions
            0x05 => self.cxm1fb, // Missile 1 to Playfield/Ball collisions
            0x06 => self.cxblpf, // Ball to Playfield collisions
            0x07 => self.cxppmm, // Player and Missile collisions
            0x08..=0x0B => 0,    // Input ports 0-3 (paddles, not implemented)
            0x0C => self.inpt4,  // Input port 4 (Player 0 fire button)
            0x0D => self.inpt5,  // Input port 5 (Player 1 fire button)
            _ => 0,
        }
    }

    /// Clock the TIA for one CPU cycle (3 color clocks)
    pub fn clock(&mut self) {
        // Simplified: just advance pixel counter
        self.pixel += 3; // 3 color clocks per CPU cycle

        if self.pixel >= 228 {
            self.pixel -= 228; // Wrap pixel properly (was = 0, which loses remainders)
            let old_scanline = self.scanline;

            // Latch the state of the OLD scanline BEFORE advancing to the new one
            // This ensures we capture the final state of the scanline after all register writes
            self.latch_scanline_state(old_scanline);

            self.scanline += 1;

            self.scanline_counter = self.scanline_counter.saturating_add(1);

            if self.scanline >= 262 {
                self.scanline = 0;
            }

            // Debug logging
            if LogConfig::global().should_log(LogCategory::PPU, LogLevel::Trace) {
                eprintln!("[TIA CLOCK] Scanline {} -> {}", old_scanline, self.scanline);
            }
        }
    }

    /// Calculate CPU cycles remaining until end of scanline (for WSYNC)
    pub fn cpu_cycles_until_scanline_end(&self) -> u32 {
        let pixel = self.pixel.min(227) as u32;
        let remaining_color_clocks = 228u32.saturating_sub(pixel);
        let extra = remaining_color_clocks.div_ceil(3);
        extra.max(1)
    }

    /// Check if in VBLANK
    #[allow(dead_code)]
    pub fn in_vblank(&self) -> bool {
        self.vblank || self.vsync
    }

    /// Get current scanline
    pub fn get_scanline(&self) -> u16 {
        self.scanline
    }

    /// Try to infer the start of the visible picture area based on VBLANK timing
    ///
    /// This method caches the first detected visible start to prevent vertical jumping
    /// between frames. Once a valid VBLANK transition is detected, that value is used
    /// for all subsequent frames to ensure stable rendering.
    pub fn visible_window_start_scanline(&mut self) -> u16 {
        // If we have a cached value, use it for stability
        if let Some(cached) = self.cached_visible_start {
            return cached;
        }

        // Find where VBLANK transitions from true to false
        let debug = LogConfig::global().should_log(LogCategory::PPU, LogLevel::Debug);

        for i in 1..262 {
            let prev = self.scanline_states.get(i - 1).copied().unwrap_or_default();
            let cur = self.scanline_states.get(i).copied().unwrap_or_default();

            if debug && i < 100 {
                eprintln!(
                    "[VISIBLE] scanline {} prev.vblank={} cur.vblank={}",
                    i, prev.vblank, cur.vblank
                );
            }

            if prev.vblank && !cur.vblank {
                if debug {
                    eprintln!(
                        "[VISIBLE] Found transition at scanline {}, caching for stability",
                        i
                    );
                }
                self.cached_visible_start = Some(i as u16);
                return i as u16;
            }
        }

        // Fallback: common NTSC visible start is around scanline ~37-40
        if debug {
            eprintln!("[VISIBLE] No transition found, using fallback 40");
        }
        self.cached_visible_start = Some(40);
        40
    }

    /// Debug helper: count how many of the 192 visible scanlines have any playfield/player bits.
    pub fn debug_visible_scanline_activity(&self, visible_start: u16) -> (u32, u32) {
        let mut scanlines_with_pf = 0u32;
        let mut scanlines_with_grp = 0u32;

        for visible_line in 0..192u16 {
            let tia_scanline = (visible_start + visible_line) % 262;
            let state = self
                .scanline_states
                .get(tia_scanline as usize)
                .copied()
                .unwrap_or_default();

            if state.pf0 != 0 || state.pf1 != 0 || state.pf2 != 0 {
                scanlines_with_pf += 1;
            }
            if state.grp0 != 0 || state.grp1 != 0 {
                scanlines_with_grp += 1;
            }
        }

        (scanlines_with_pf, scanlines_with_grp)
    }

    /// Debug helper: count PF/GRP activity across all 262 scanlines.
    pub fn debug_all_scanline_activity(&self) -> (u32, u32) {
        let mut scanlines_with_pf = 0u32;
        let mut scanlines_with_grp = 0u32;

        for scanline in 0..262usize {
            let state = self
                .scanline_states
                .get(scanline)
                .copied()
                .unwrap_or_default();
            if state.pf0 != 0 || state.pf1 != 0 || state.pf2 != 0 {
                scanlines_with_pf += 1;
            }
            if state.grp0 != 0 || state.grp1 != 0 {
                scanlines_with_grp += 1;
            }
        }

        (scanlines_with_pf, scanlines_with_grp)
    }

    /// Render a single visible scanline using latched state
    /// `visible_line` is 0-191, `tia_scanline` is the actual TIA scanline (0-261)
    pub fn render_scanline(&self, buffer: &mut [u32], visible_line: usize, tia_scanline: u16) {
        if visible_line >= 192 {
            return; // Only visible lines
        }

        // Get latched state for this scanline
        let state = self
            .scanline_states
            .get((tia_scanline as usize).min(261))
            .copied()
            .unwrap_or_default();

        // Atari 2600 has 160 pixels per scanline
        for x in 0..160 {
            let color = Self::get_pixel_color(&state, x);
            buffer[visible_line * 160 + x] = color;
        }
    }

    /// Detect and record collisions for a scanline (called during frame rendering)
    /// This should be called once per scanline to update collision registers
    fn detect_collisions_for_scanline(&mut self, tia_scanline: u16) {
        let state = self
            .scanline_states
            .get((tia_scanline as usize).min(261))
            .copied()
            .unwrap_or_default();

        // Check all 160 pixels for collisions
        for x in 0..160 {
            let p0 = Self::is_player_pixel(&state, 0, x);
            let p1 = Self::is_player_pixel(&state, 1, x);
            let m0 = Self::is_missile_pixel(&state, 0, x);
            let m1 = Self::is_missile_pixel(&state, 1, x);
            let bl = Self::is_ball_pixel(&state, x);
            let pf = Self::is_playfield_pixel(&state, x);

            // Missile 0 to Player collisions (CXM0P)
            if m0 && p1 {
                self.cxm0p |= 0x80; // M0P1
            }
            if m0 && p0 {
                self.cxm0p |= 0x40; // M0P0
            }

            // Missile 1 to Player collisions (CXM1P)
            if m1 && p0 {
                self.cxm1p |= 0x80; // M1P0
            }
            if m1 && p1 {
                self.cxm1p |= 0x40; // M1P1
            }

            // Player 0 to Playfield/Ball collisions (CXP0FB)
            if p0 && pf {
                self.cxp0fb |= 0x80; // P0PF
            }
            if p0 && bl {
                self.cxp0fb |= 0x40; // P0BL
            }

            // Player 1 to Playfield/Ball collisions (CXP1FB)
            if p1 && pf {
                self.cxp1fb |= 0x80; // P1PF
            }
            if p1 && bl {
                self.cxp1fb |= 0x40; // P1BL
            }

            // Missile 0 to Playfield/Ball collisions (CXM0FB)
            if m0 && pf {
                self.cxm0fb |= 0x80; // M0PF
            }
            if m0 && bl {
                self.cxm0fb |= 0x40; // M0BL
            }

            // Missile 1 to Playfield/Ball collisions (CXM1FB)
            if m1 && pf {
                self.cxm1fb |= 0x80; // M1PF
            }
            if m1 && bl {
                self.cxm1fb |= 0x40; // M1BL
            }

            // Ball to Playfield collisions (CXBLPF)
            if bl && pf {
                self.cxblpf |= 0x80; // BLPF
            }

            // Player and Missile collisions (CXPPMM)
            if m0 && m1 {
                self.cxppmm |= 0x80; // M0M1
            }
            if p0 && p1 {
                self.cxppmm |= 0x40; // P0P1
            }
        }
    }

    /// Detect collisions for the entire frame (should be called after rendering)
    /// This updates the collision registers based on the current frame state
    pub fn detect_collisions_for_frame(&mut self, visible_start: u16) {
        // Detect collisions for all 192 visible scanlines
        for visible_line in 0..192 {
            let tia_scanline = (visible_start + visible_line) % 262;
            self.detect_collisions_for_scanline(tia_scanline);
        }
    }

    /// Get the color of a pixel at the given position using latched state
    fn get_pixel_color(state: &ScanlineState, x: usize) -> u32 {
        // During VBLANK, all pixels are black (video signal is blanked)
        if state.vblank {
            return 0xFF000000; // Black
        }

        // Priority order (when playfield priority is off):
        // 1. Player 0, Missile 0
        // 2. Player 1, Missile 1
        // 3. Ball
        // 4. Playfield
        // 5. Background

        // With playfield priority:
        // 1. Playfield, Ball
        // 2. Player 0, Missile 0
        // 3. Player 1, Missile 1
        // 4. Background

        // Check players and missiles first (if priority is normal)
        if !state.playfield_priority {
            // Check Player 0
            if Self::is_player_pixel(state, 0, x) {
                return ntsc_to_rgb(state.colup0);
            }

            // Check Missile 0
            if Self::is_missile_pixel(state, 0, x) {
                return ntsc_to_rgb(state.colup0);
            }

            // Check Player 1
            if Self::is_player_pixel(state, 1, x) {
                return ntsc_to_rgb(state.colup1);
            }

            // Check Missile 1
            if Self::is_missile_pixel(state, 1, x) {
                return ntsc_to_rgb(state.colup1);
            }

            // Check Ball
            if Self::is_ball_pixel(state, x) {
                return ntsc_to_rgb(state.colupf);
            }
        }

        // Check playfield
        if Self::is_playfield_pixel(state, x) {
            return ntsc_to_rgb(state.colupf);
        }

        // Check Ball (if playfield priority)
        if state.playfield_priority && Self::is_ball_pixel(state, x) {
            return ntsc_to_rgb(state.colupf);
        }

        // Check players and missiles (if playfield priority)
        if state.playfield_priority {
            if Self::is_player_pixel(state, 0, x) {
                return ntsc_to_rgb(state.colup0);
            }
            if Self::is_missile_pixel(state, 0, x) {
                return ntsc_to_rgb(state.colup0);
            }
            if Self::is_player_pixel(state, 1, x) {
                return ntsc_to_rgb(state.colup1);
            }
            if Self::is_missile_pixel(state, 1, x) {
                return ntsc_to_rgb(state.colup1);
            }
        }

        // Background color
        ntsc_to_rgb(state.colubk)
    }

    /// Check if a player pixel is visible at the given x position
    fn is_player_pixel(state: &ScanlineState, player: usize, x: usize) -> bool {
        let (grp, pos, reflect, nusiz) = if player == 0 {
            (
                state.grp0,
                state.player0_x,
                state.player0_reflect,
                state.nusiz0,
            )
        } else {
            (
                state.grp1,
                state.player1_x,
                state.player1_reflect,
                state.nusiz1,
            )
        };

        // NUSIZ bits 0-2 control number and size
        // Bits 0-2: 000=one, 001=two close, 010=two medium, 011=three close,
        //           100=two wide, 101=double size, 110=three medium, 111=quad size
        let nusiz_mode = nusiz & 0x07;

        // Get player size (1x, 2x, or 4x)
        let player_size = match nusiz_mode {
            0x05 => 2, // Double width (2x)
            0x07 => 4, // Quad width (4x)
            _ => 1,    // Normal width (1x)
        };

        // Get number of copies and their spacing
        let (num_copies, spacing) = match nusiz_mode {
            0x00 => (1, 0),  // One copy
            0x01 => (2, 16), // Two copies close together
            0x02 => (2, 32), // Two copies medium spacing
            0x03 => (3, 16), // Three copies close together
            0x04 => (2, 64), // Two copies wide spacing
            0x05 => (1, 0),  // One double-width copy
            0x06 => (3, 32), // Three copies medium spacing
            0x07 => (1, 0),  // One quad-width copy
            _ => (1, 0),
        };

        // Check each copy
        for copy in 0..num_copies {
            let copy_pos = pos as usize + copy * spacing;
            if copy_pos >= 160 {
                continue;
            }
            let offset = x.wrapping_sub(copy_pos);

            if offset < 8 * player_size {
                // Which pixel of the 8-pixel sprite?
                let sprite_pixel = offset / player_size;

                // Get the bit from the graphics register
                let bit = if reflect {
                    sprite_pixel // Normal order when reflected
                } else {
                    7 - sprite_pixel // Reverse order when not reflected
                };

                if (grp & (1 << bit)) != 0 {
                    return true;
                }
            }
        }

        false
    }

    /// Check if a missile pixel is visible at the given x position
    fn is_missile_pixel(state: &ScanlineState, missile: usize, x: usize) -> bool {
        let (enabled, pos, nusiz) = if missile == 0 {
            (state.enam0, state.missile0_x, state.nusiz0)
        } else {
            (state.enam1, state.missile1_x, state.nusiz1)
        };

        if !enabled {
            return false;
        }

        // NUSIZ bits 4-5 control missile width
        // 00=1 pixel, 01=2 pixels, 10=4 pixels, 11=8 pixels
        let missile_size = match (nusiz >> 4) & 0x03 {
            0x00 => 1,
            0x01 => 2,
            0x02 => 4,
            0x03 => 8,
            _ => 1,
        };

        // Missiles use the same duplication pattern as players (bits 0-2)
        let nusiz_mode = nusiz & 0x07;
        let (num_copies, spacing) = match nusiz_mode {
            0x00 => (1, 0),  // One copy
            0x01 => (2, 16), // Two copies close together
            0x02 => (2, 32), // Two copies medium spacing
            0x03 => (3, 16), // Three copies close together
            0x04 => (2, 64), // Two copies wide spacing
            0x05 => (1, 0),  // One copy (double size doesn't affect missiles)
            0x06 => (3, 32), // Three copies medium spacing
            0x07 => (1, 0),  // One copy (quad size doesn't affect missiles)
            _ => (1, 0),
        };

        // Check each copy
        for copy in 0..num_copies {
            let copy_pos = pos as usize + copy * spacing;
            if copy_pos >= 160 {
                continue;
            }
            let offset = x.wrapping_sub(copy_pos);

            if offset < missile_size {
                return true;
            }
        }

        false
    }

    /// Check if the ball pixel is visible at the given x position
    fn is_ball_pixel(state: &ScanlineState, x: usize) -> bool {
        if !state.enabl {
            return false;
        }

        // Ball size is controlled by CTRLPF bits 4-5 (1, 2, 4, or 8 pixels)
        let offset = x.wrapping_sub(state.ball_x as usize);
        offset < state.ball_size as usize
    }

    /// Check if a pixel is part of the playfield
    fn is_playfield_pixel(state: &ScanlineState, x: usize) -> bool {
        // Playfield is 40 bits wide, each bit controls 4 pixels
        // Playfield is mirrored or repeated for left/right halves
        if x < 80 {
            // Left half: pixels 0-79, bits 0-19
            // Each bit covers 4 pixels
            Self::get_playfield_bit(state, x / 4)
        } else {
            // Right half: pixels 80-159, bits 0-19 (mirrored or repeated)
            // Each bit covers 4 pixels
            let bit_pos = (x - 80) / 4;
            if state.playfield_reflect {
                // Mirrored
                Self::get_playfield_bit(state, 19 - bit_pos)
            } else {
                // Repeated
                Self::get_playfield_bit(state, bit_pos)
            }
        }
    }

    /// Get a single bit from the playfield
    fn get_playfield_bit(state: &ScanlineState, bit: usize) -> bool {
        if bit < 4 {
            // PF0 (bits 4-7 map to playfield bits 0-3)
            (state.pf0 & (0x10 << bit)) != 0
        } else if bit < 12 {
            // PF1 (bits 7-0 map to playfield bits 4-11)
            (state.pf1 & (0x80 >> (bit - 4))) != 0
        } else if bit < 20 {
            // PF2 (bits 0-7 map to playfield bits 12-19)
            (state.pf2 & (0x01 << (bit - 12))) != 0
        } else {
            false
        }
    }

    /// Generate audio samples for a given count
    /// TIA runs at 31.4 kHz (color clock / 114), but we output at 44.1 kHz
    pub fn generate_audio_samples(&mut self, sample_count: usize) -> Vec<i16> {
        const SAMPLE_HZ: f64 = 44_100.0;
        const TIA_AUDIO_HZ: f64 = 31_400.0; // Approximate TIA audio clock rate
        const TIA_CLOCKS_PER_SAMPLE: f64 = TIA_AUDIO_HZ / SAMPLE_HZ;
        // 15 represents the midpoint when both channels are at max (15+15)/2 = 15
        const AUDIO_OFFSET: i32 = 15360; // 15 * 1024

        let mut samples = Vec::with_capacity(sample_count);
        let mut accum = 0.0;

        for _ in 0..sample_count {
            // Determine how many TIA clocks to run for this sample
            accum += TIA_CLOCKS_PER_SAMPLE;
            let tia_clocks = accum as u32;
            accum -= tia_clocks as f64;

            // Clock both audio channels and mix
            let clocks_to_run = tia_clocks.max(1);
            let mut mixed = 0i32;
            for _ in 0..clocks_to_run {
                let s0 = self.audio0.clock() as i32;
                let s1 = self.audio1.clock() as i32;
                mixed += s0 + s1;
            }

            // Average and scale to 16-bit range
            let avg = mixed / clocks_to_run as i32;
            // Scale from 0-30 (max 15+15) to approximately -16384 to 16384
            // Using bit shift for efficiency: avg * 1024 - 15360
            let scaled = (avg << 10) - AUDIO_OFFSET;
            samples.push(scaled.clamp(-32768, 32767) as i16);
        }

        samples
    }
}

/// Convert NTSC palette value to RGB
/// Atari 2600 uses NTSC color encoding with 128 colors
/// Upper 4 bits: hue (0-15), Lower 3 bits: luminance (0-7, bit 0 unused)
fn ntsc_to_rgb(ntsc: u8) -> u32 {
    // NTSC palette table for Atari 2600
    // Organized by hue (16 hues) x luminance (8 levels) = 128 colors
    // Each row is one hue with 8 luminance levels from darkest to brightest
    const NTSC_PALETTE: [u32; 128] = [
        // Hue 0 (Gray) - Luminance 0-7 (darkest to brightest)
        0xFF000000, 0xFF404040, 0xFF6C6C6C, 0xFF909090, 0xFFB0B0B0, 0xFFC8C8C8, 0xFFDCDCDC,
        0xFFECECEC, // Hue 1 (Gold/Yellow) - Luminance 0-7
        0xFF444400, 0xFF646410, 0xFF848424, 0xFFA0A034, 0xFFB8B840, 0xFFD0D050, 0xFFE8E85C,
        0xFFFCFC68, // Hue 2 (Orange) - Luminance 0-7
        0xFF702800, 0xFF844414, 0xFF985C28, 0xFFAC783C, 0xFFBC8C4C, 0xFFCCA05C, 0xFFDCB468,
        0xFFECC878, // Luminance 3
        0xFF841800, 0xFF983418, 0xFFAC5030, 0xFFC06848, 0xFFD0805C, 0xFFE09470, 0xFFECA880,
        0xFFFCBC94, // Luminance 4
        0xFF880000, 0xFF9C2020, 0xFFB03C3C, 0xFFC05858, 0xFFD07070, 0xFFE08888, 0xFFECA0A0,
        0xFFFCB4B4, // Luminance 5
        0xFF78005C, 0xFF8C2074, 0xFFA03C88, 0xFFB0589C, 0xFFC070B0, 0xFFD084C0, 0xFFDC9CD0,
        0xFFECB0E0, // Luminance 6
        0xFF480078, 0xFF602090, 0xFF783CA4, 0xFF8C58B8, 0xFFA070CC, 0xFFB484DC, 0xFFC49CEC,
        0xFFD4B0FC, // Luminance 7
        0xFF140084, 0xFF302098, 0xFF4C3CAC, 0xFF6858C0, 0xFF7C70D0, 0xFF9488E0, 0xFFA8A0EC,
        0xFFBCB4FC, // Luminance 8
        0xFF000088, 0xFF1C209C, 0xFF3840B0, 0xFF505CC0, 0xFF6874D0, 0xFF7C8CE0, 0xFF90A4EC,
        0xFFA4B8FC, // Luminance 9
        0xFF00187C, 0xFF1C3890, 0xFF3854A8, 0xFF5070BC, 0xFF6888CC, 0xFF7C9CDC, 0xFF90B4EC,
        0xFFA4C8FC, // Luminance 10
        0xFF002C5C, 0xFF1C4C78, 0xFF386890, 0xFF5084AC, 0xFF689CC0, 0xFF7CB4D4, 0xFF90CCE8,
        0xFFA4E0FC, // Luminance 11
        0xFF003C2C, 0xFF1C5C48, 0xFF387C64, 0xFF509C80, 0xFF68B494, 0xFF7CD0AC, 0xFF90E4C0,
        0xFFA4FCD4, // Luminance 12
        0xFF003C00, 0xFF205C20, 0xFF407C40, 0xFF5C9C5C, 0xFF74B474, 0xFF8CD08C, 0xFFA4E4A4,
        0xFFB8FCB8, // Luminance 13
        0xFF143800, 0xFF345C1C, 0xFF507C38, 0xFF6C9850, 0xFF84B468, 0xFF9CCC7C, 0xFFB4E490,
        0xFFC8FCA4, // Luminance 14
        0xFF2C3000, 0xFF4C501C, 0xFF687034, 0xFF848C4C, 0xFF9CA864, 0xFFB4C078, 0xFFCCD488,
        0xFFE0EC9C, // Hue 15 (brightest)
        0xFF442800, 0xFF644818, 0xFF846830, 0xFFA08444, 0xFFB89C58, 0xFFD0B46C, 0xFFE8CC7C,
        0xFFFCE08C,
    ];

    // Mask to 7 bits to ensure we're within the 128-color palette bounds
    // NTSC color encoding only uses bits 1-7 (bit 0 is unused)
    NTSC_PALETTE[ntsc as usize & 0x7F]
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

    #[test]
    fn test_tia_player_rendering() {
        let mut tia = Tia::new();

        // Set player 0 position and graphics
        tia.player0_x = 80;
        tia.write(0x1B, 0xFF); // GRP0 - all bits set
        tia.write(0x06, 0x28); // COLUP0 - orange

        // Create a small frame buffer
        let mut frame = vec![0u32; 160];

        // Render a scanline
        tia.render_scanline(&mut frame, 0, 0);

        // Player should be visible at x=80-87
        assert_ne!(frame[80], ntsc_to_rgb(0)); // Should be player color, not background
        assert_ne!(frame[87], ntsc_to_rgb(0)); // Last pixel of player
    }

    #[test]
    fn test_tia_missile_rendering() {
        let mut tia = Tia::new();

        // Enable missile 0
        tia.missile0_x = 50;
        tia.write(0x1D, 0x02); // ENAM0
        tia.write(0x06, 0x28); // COLUP0

        let mut frame = vec![0u32; 160];
        tia.render_scanline(&mut frame, 0, 0);

        // Missile should be visible at x=50
        assert_ne!(frame[50], ntsc_to_rgb(0));
    }

    #[test]
    fn test_tia_ball_rendering() {
        let mut tia = Tia::new();

        // Enable ball
        tia.ball_x = 100;
        tia.write(0x1F, 0x02); // ENABL
        tia.write(0x08, 0x0E); // COLUPF - white

        let mut frame = vec![0u32; 160];
        tia.render_scanline(&mut frame, 0, 0);

        // Ball should be visible at x=100
        assert_ne!(frame[100], ntsc_to_rgb(0));
    }

    #[test]
    fn test_tia_ball_size() {
        let mut tia = Tia::new();

        tia.write(0x14, 0x00); // RESBL - position ball at x=0
        tia.write(0x1F, 0x02); // ENABL - enable ball

        // Test 1-pixel ball (CTRLPF bits 4-5 = 00)
        tia.write(0x0A, 0x00);
        tia.latch_scanline_state(0);
        let state = tia.scanline_states[0];
        assert_eq!(state.ball_size, 1);
        assert!(Tia::is_ball_pixel(&state, 0));
        assert!(!Tia::is_ball_pixel(&state, 1));

        // Test 2-pixel ball (CTRLPF bits 4-5 = 01)
        tia.write(0x0A, 0x10);
        tia.latch_scanline_state(0);
        let state = tia.scanline_states[0];
        assert_eq!(state.ball_size, 2);
        assert!(Tia::is_ball_pixel(&state, 0));
        assert!(Tia::is_ball_pixel(&state, 1));
        assert!(!Tia::is_ball_pixel(&state, 2));

        // Test 4-pixel ball (CTRLPF bits 4-5 = 10)
        tia.write(0x0A, 0x20);
        tia.latch_scanline_state(0);
        let state = tia.scanline_states[0];
        assert_eq!(state.ball_size, 4);
        assert!(Tia::is_ball_pixel(&state, 0));
        assert!(Tia::is_ball_pixel(&state, 3));
        assert!(!Tia::is_ball_pixel(&state, 4));

        // Test 8-pixel ball (CTRLPF bits 4-5 = 11)
        tia.write(0x0A, 0x30);
        tia.latch_scanline_state(0);
        let state = tia.scanline_states[0];
        assert_eq!(state.ball_size, 8);
        assert!(Tia::is_ball_pixel(&state, 0));
        assert!(Tia::is_ball_pixel(&state, 7));
        assert!(!Tia::is_ball_pixel(&state, 8));
    }

    #[test]
    fn test_tia_playfield_priority() {
        let mut tia = Tia::new();

        // Set up playfield
        tia.write(0x0D, 0xF0); // PF0
        tia.write(0x08, 0x0E); // COLUPF - white

        // Set up player at same position
        tia.player0_x = 0;
        tia.write(0x1B, 0xFF); // GRP0
        tia.write(0x06, 0x28); // COLUP0 - orange

        let mut frame = vec![0u32; 160];

        // Without priority, player should be in front (CTRLPF bit 2 = 0)
        tia.write(0x0A, 0x00);
        tia.render_scanline(&mut frame, 0, 0);
        let player_color = frame[0];

        // With priority, playfield should be in front (CTRLPF bit 2 = 1)
        tia.write(0x0A, 0x04);
        tia.render_scanline(&mut frame, 0, 0);
        let pf_color = frame[0];

        // Colors should be different
        assert_ne!(player_color, pf_color);
    }

    #[test]
    fn test_tia_player_reflect() {
        let mut tia = Tia::new();

        // Set player with specific pattern
        tia.player0_x = 80;
        tia.write(0x1B, 0b10101010); // GRP0 - alternating pattern
        tia.write(0x06, 0x28); // COLUP0

        let mut frame_normal = vec![0u32; 160];
        // REFP0 bit 3 controls reflection
        tia.write(0x0B, 0x00);
        tia.render_scanline(&mut frame_normal, 0, 0);

        let mut frame_reflect = vec![0u32; 160];
        tia.write(0x0B, 0x08);
        tia.render_scanline(&mut frame_reflect, 0, 0);

        // The patterns should be different
        assert_ne!(frame_normal[80], frame_reflect[80]);
    }

    #[test]
    fn test_ntsc_palette() {
        // Test a few known colors
        let black = ntsc_to_rgb(0x00);
        let white = ntsc_to_rgb(0x0E);

        // Black should be dark, white should be bright
        assert_eq!(black, 0xFF000000);
        assert_ne!(white, black);

        // Test color range
        for i in 0..128 {
            let color = ntsc_to_rgb(i);
            // Should have alpha channel set
            assert_eq!(color & 0xFF000000, 0xFF000000);
        }
    }

    #[test]
    fn test_nusiz_normal_width() {
        let mut tia = Tia::new();

        // Set NUSIZ0 to normal width (mode 000)
        tia.write(0x04, 0x00);
        tia.player0_x = 80;
        tia.write(0x1B, 0xFF); // GRP0 - all bits set
        tia.write(0x06, 0x28); // COLUP0

        let mut frame = vec![0u32; 160];
        tia.latch_scanline_state(0);
        tia.render_scanline(&mut frame, 0, 0);

        // Normal width: 8 pixels
        assert_ne!(frame[80], ntsc_to_rgb(0));
        assert_ne!(frame[87], ntsc_to_rgb(0));
        assert_eq!(frame[88], ntsc_to_rgb(0)); // Outside sprite
    }

    #[test]
    fn test_nusiz_double_width() {
        let mut tia = Tia::new();

        // Set NUSIZ0 to double width (mode 101)
        tia.write(0x04, 0x05);
        tia.player0_x = 80;
        tia.write(0x1B, 0xFF); // GRP0 - all bits set
        tia.write(0x06, 0x28); // COLUP0

        let mut frame = vec![0u32; 160];
        tia.latch_scanline_state(0);
        tia.render_scanline(&mut frame, 0, 0);

        // Double width: 16 pixels (8 pixels * 2)
        assert_ne!(frame[80], ntsc_to_rgb(0));
        assert_ne!(frame[95], ntsc_to_rgb(0));
        assert_eq!(frame[96], ntsc_to_rgb(0)); // Outside sprite
    }

    #[test]
    fn test_nusiz_quad_width() {
        let mut tia = Tia::new();

        // Set NUSIZ0 to quad width (mode 111)
        tia.write(0x04, 0x07);
        tia.player0_x = 80;
        tia.write(0x1B, 0xFF); // GRP0 - all bits set
        tia.write(0x06, 0x28); // COLUP0

        let mut frame = vec![0u32; 160];
        tia.latch_scanline_state(0);
        tia.render_scanline(&mut frame, 0, 0);

        // Quad width: 32 pixels (8 pixels * 4)
        assert_ne!(frame[80], ntsc_to_rgb(0));
        assert_ne!(frame[111], ntsc_to_rgb(0));
        assert_eq!(frame[112], ntsc_to_rgb(0)); // Outside sprite
    }

    #[test]
    fn test_nusiz_two_copies_close() {
        let mut tia = Tia::new();

        // Set NUSIZ0 to two copies close (mode 001)
        tia.write(0x04, 0x01);
        tia.player0_x = 80;
        tia.write(0x1B, 0xFF); // GRP0
        tia.write(0x06, 0x28); // COLUP0

        let mut frame = vec![0u32; 160];
        tia.latch_scanline_state(0);
        tia.render_scanline(&mut frame, 0, 0);

        // First copy at x=80
        assert_ne!(frame[80], ntsc_to_rgb(0));
        assert_ne!(frame[87], ntsc_to_rgb(0));

        // Second copy at x=96 (80 + 16)
        assert_ne!(frame[96], ntsc_to_rgb(0));
        assert_ne!(frame[103], ntsc_to_rgb(0));
    }

    #[test]
    fn test_nusiz_three_copies_close() {
        let mut tia = Tia::new();

        // Set NUSIZ0 to three copies close (mode 011)
        tia.write(0x04, 0x03);
        tia.player0_x = 50;
        tia.write(0x1B, 0xFF); // GRP0
        tia.write(0x06, 0x28); // COLUP0

        let mut frame = vec![0u32; 160];
        tia.latch_scanline_state(0);
        tia.render_scanline(&mut frame, 0, 0);

        // First copy at x=50
        assert_ne!(frame[50], ntsc_to_rgb(0));

        // Second copy at x=66 (50 + 16 spacing)
        assert_ne!(frame[66], ntsc_to_rgb(0));

        // Third copy at x=82 (50 + 16 + 16 spacing)
        assert_ne!(frame[82], ntsc_to_rgb(0));
    }

    #[test]
    fn test_missile_nusiz_width() {
        let mut tia = Tia::new();

        // Set NUSIZ0 bits 4-5 to 10 (4 pixel width)
        tia.write(0x04, 0x20);
        tia.missile0_x = 80;
        tia.write(0x1D, 0x02); // ENAM0
        tia.write(0x06, 0x28); // COLUP0

        let mut frame = vec![0u32; 160];
        tia.latch_scanline_state(0);
        tia.render_scanline(&mut frame, 0, 0);

        // 4 pixel wide missile
        assert_ne!(frame[80], ntsc_to_rgb(0));
        assert_ne!(frame[83], ntsc_to_rgb(0));
        assert_eq!(frame[84], ntsc_to_rgb(0)); // Outside missile
    }

    #[test]
    fn test_collision_player_playfield() {
        let mut tia = Tia::new();

        // Set up playfield
        tia.write(0x0D, 0xF0); // PF0
        tia.write(0x08, 0x0E); // COLUPF

        // Set up player overlapping playfield
        tia.player0_x = 0;
        tia.write(0x1B, 0xFF); // GRP0
        tia.write(0x06, 0x28); // COLUP0

        // Detect collisions
        tia.latch_scanline_state(0);
        tia.detect_collisions_for_scanline(0);

        // Read collision register - CXP0FB should have P0PF bit set
        assert_ne!(tia.read(0x02) & 0x80, 0); // CXP0FB bit 7 (P0PF)
    }

    #[test]
    fn test_collision_player_player() {
        let mut tia = Tia::new();

        // Set up both players at same position
        tia.player0_x = 80;
        tia.player1_x = 80;
        tia.write(0x1B, 0xFF); // GRP0
        tia.write(0x1C, 0xFF); // GRP1
        tia.write(0x06, 0x28); // COLUP0
        tia.write(0x07, 0x38); // COLUP1

        // Detect collisions
        tia.latch_scanline_state(0);
        tia.detect_collisions_for_scanline(0);

        // Read collision register - CXPPMM should have P0P1 bit set
        assert_ne!(tia.read(0x07) & 0x40, 0); // CXPPMM bit 6 (P0P1)
    }

    #[test]
    fn test_collision_clear() {
        let mut tia = Tia::new();

        // Set up collision
        tia.player0_x = 80;
        tia.player1_x = 80;
        tia.write(0x1B, 0xFF);
        tia.write(0x1C, 0xFF);

        // Detect collisions
        tia.latch_scanline_state(0);
        tia.detect_collisions_for_scanline(0);

        // Verify collision is set
        assert_ne!(tia.read(0x07), 0);

        // Clear collisions with CXCLR
        tia.write(0x2C, 0x00);

        // Verify collision is cleared
        assert_eq!(tia.read(0x07), 0);
    }

    #[test]
    fn test_vdelp_delayed_graphics() {
        let mut tia = Tia::new();

        // Write initial graphics
        tia.write(0x1B, 0xAA); // GRP0 = 0xAA
        assert_eq!(tia.grp0, 0xAA);
        assert_eq!(tia.grp0_old, 0x00); // Old value is 0

        // Write new graphics - old value should be saved
        tia.write(0x1B, 0xFF); // GRP0 = 0xFF
        assert_eq!(tia.grp0, 0xFF);
        assert_eq!(tia.grp0_old, 0xAA); // Old value saved

        // Enable delayed graphics
        tia.write(0x25, 0x01); // VDELP0
        assert!(tia.vdelp0);

        // When latching state, delayed graphics should use old value
        tia.latch_scanline_state(0);
        let state = tia.scanline_states[0];
        assert_eq!(state.grp0, 0xAA); // Uses old value when VDELP0 is set
    }
}
