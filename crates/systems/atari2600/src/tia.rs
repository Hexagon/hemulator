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
//!
//! The emulator now implements paddle controller support via capacitor charge simulation.

use emu_core::apu::PolynomialCounter;
use emu_core::logging::{LogCategory, LogConfig, LogLevel};
use serde::{Deserialize, Serialize};

use crate::video_mode::VideoMode;

/// Maximum number of mid-scanline graphics changes we track per scanline
const MAX_GRP_CHANGES: usize = 8;

/// A mid-scanline graphics change: (pixel position, value)
#[derive(Debug, Clone, Copy, Default)]
struct GrpChange {
    pixel: u8, // Horizontal pixel position (0-159) when the change occurred
    value: u8, // The graphics value written
}

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
    // Mid-scanline graphics changes for racing-the-beam effects
    grp0_changes: [GrpChange; MAX_GRP_CHANGES],
    grp0_change_count: u8,
    grp1_changes: [GrpChange; MAX_GRP_CHANGES],
    grp1_change_count: u8,
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
    resmp0: bool, // Reset missile 0 to player 0 position
    resmp1: bool, // Reset missile 1 to player 1 position

    // Ball
    enabl: bool,   // Ball enable
    enabl_old: u8, // Previous ENABL value for delayed graphics
    ball_x: u8,
    ball_size: u8, // Ball size (1, 2, 4, or 8 pixels) from CTRLPF bits 4-5
    vdelbl: bool,  // Ball delayed graphics enable

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

    // INPT0-INPT3: Paddle controllers (bit 7: 0=charged, 1=not charged)
    // Paddles use capacitor charging time to measure position
    inpt0: u8, // Paddle 0 (Port 0 X)
    inpt1: u8, // Paddle 1 (Port 0 Y)
    inpt2: u8, // Paddle 2 (Port 1 X)
    inpt3: u8, // Paddle 3 (Port 1 Y)

    // Paddle state
    paddle_positions: [u8; 4], // 0-255 for each paddle (0 = left/up, 255 = right/down)
    paddle_charge_time: [u32; 4], // Color clocks since capacitor dump
    paddle_dump_enabled: bool, // VBLANK bit 7: dump paddle capacitors
    paddle_latch_enabled: bool, // VBLANK bit 6: latch paddle fire buttons

    // Current scanline and pixel position
    scanline: u16,
    pixel: u16,

    // Mid-scanline graphics change tracking for current scanline
    #[serde(skip)]
    current_grp0_changes: [GrpChange; MAX_GRP_CHANGES],
    #[serde(skip)]
    current_grp0_change_count: u8,
    #[serde(skip)]
    current_grp1_changes: [GrpChange; MAX_GRP_CHANGES],
    #[serde(skip)]
    current_grp1_change_count: u8,

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

    // Frame counter for visible window detection stability
    #[serde(skip)]
    visible_window_frame_count: u32,

    // Video mode (NTSC/PAL)
    video_mode: VideoMode,
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
    /// Note: TIA horizontal motion is inverted from what you'd expect:
    /// - Positive values (+1 to +7) move LEFT (decrease x position)
    /// - Negative values (-1 to -8) move RIGHT (increase x position)
    fn apply_motion(&self, pos: u8, motion: i8) -> u8 {
        let p = pos as i16;
        let m = motion as i16;
        // Subtract motion because TIA motion values are inverted:
        // +7 means "move left 7 clocks" (subtract from position)
        // -8 means "move right 8 clocks" (add to position)
        let result = p - m;
        // Wrap around the 160-pixel screen width
        // The TIA hardware wraps positions, not clamps them
        if result < 0 {
            ((result % 160) + 160) as u8
        } else {
            (result % 160) as u8
        }
    }

    /// Create a new TIA chip with default NTSC video mode
    pub fn new() -> Self {
        Self::with_video_mode(VideoMode::default())
    }

    /// Create a new TIA chip with specified video mode
    pub fn with_video_mode(video_mode: VideoMode) -> Self {
        let total_scanlines = video_mode.scanlines_per_frame() as usize;

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
            resmp0: false,
            resmp1: false,
            enabl: false,
            enabl_old: 0,
            ball_x: 0,
            ball_size: 1, // Default to 1 pixel
            vdelbl: false,
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
            inpt0: 0x80, // Paddle not charged (bit 7 = 1)
            inpt1: 0x80,
            inpt2: 0x80,
            inpt3: 0x80,
            paddle_positions: [128, 128, 128, 128], // Center position
            paddle_charge_time: [0, 0, 0, 0],
            paddle_dump_enabled: false,
            paddle_latch_enabled: false,
            scanline: 0,
            pixel: 0,

            current_grp0_changes: [GrpChange::default(); MAX_GRP_CHANGES],
            current_grp0_change_count: 0,
            current_grp1_changes: [GrpChange::default(); MAX_GRP_CHANGES],
            current_grp1_change_count: 0,

            scanline_counter: 0,

            scanline_states: vec![ScanlineState::default(); total_scanlines],

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
            visible_window_frame_count: 0,

            video_mode,
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

    /// Set paddle position for a paddle (0-3)
    ///
    /// Paddle positions are 0-255:
    /// - 0 = fully counter-clockwise (left/up)
    /// - 255 = fully clockwise (right/down)
    /// - 128 = center
    ///
    /// The TIA measures paddle position by timing capacitor charge.
    /// Lower positions charge faster, higher positions charge slower.
    pub fn set_paddle_position(&mut self, paddle: u8, position: u8) {
        if paddle < 4 {
            self.paddle_positions[paddle as usize] = position;
        }
    }

    /// Update paddle capacitor charging simulation
    /// Called each color clock to simulate the analog capacitor charging
    fn update_paddle_charging(&mut self) {
        if !self.paddle_dump_enabled {
            // Capacitors are charging
            for i in 0..4 {
                self.paddle_charge_time[i] += 1;

                // Calculate charge threshold based on paddle position
                // Position 0 (left) = fast charge (small threshold)
                // Position 255 (right) = slow charge (large threshold)
                // Typical range: ~56000 to ~80000 color clocks for full range
                let threshold = 56000 + (self.paddle_positions[i] as u32 * 100);

                // Update INPTx bit 7 based on whether capacitor has charged
                if self.paddle_charge_time[i] >= threshold {
                    // Capacitor charged - bit 7 goes high
                    match i {
                        0 => self.inpt0 |= 0x80,
                        1 => self.inpt1 |= 0x80,
                        2 => self.inpt2 |= 0x80,
                        3 => self.inpt3 |= 0x80,
                        _ => {}
                    }
                }
            }
        }
    }

    /// Get a monotonically increasing scanline counter (increments once per scanline)
    pub fn get_scanline_counter(&self) -> u64 {
        self.scanline_counter
    }

    /// Get the current video mode (NTSC/PAL)
    pub fn video_mode(&self) -> VideoMode {
        self.video_mode
    }

    /// Latch the current scanline's state immediately (public wrapper for render timing)
    pub fn latch_current_scanline_state(&mut self) {
        use emu_core::logging::{LogCategory, LogConfig, LogLevel};
        if LogConfig::global().should_log(LogCategory::PPU, LogLevel::Debug) {
            eprintln!(
                "[TIA LATCH] Explicitly latching scanline {} state",
                self.scanline
            );
        }
        self.latch_scanline_state(self.scanline);
    }

    /// Latch current TIA state for a scanline (for later rendering)
    fn latch_scanline_state(&mut self, scanline: u16) {
        // Apply RESMP: lock missiles to player positions if enabled
        if self.resmp0 {
            self.missile0_x = self.player0_x.saturating_add(4); // Center of 8-pixel player
        }
        if self.resmp1 {
            self.missile1_x = self.player1_x.saturating_add(4); // Center of 8-pixel player
        }

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
            enabl: if self.vdelbl {
                (self.enabl_old & 0x02) != 0
            } else {
                self.enabl
            },
            ball_x: self.ball_x,
            ball_size: self.ball_size,
            // Copy mid-scanline graphics changes
            grp0_changes: self.current_grp0_changes,
            grp0_change_count: self.current_grp0_change_count,
            grp1_changes: self.current_grp1_changes,
            grp1_change_count: self.current_grp1_change_count,
        };

        // Clear current scanline's changes for the next scanline
        self.current_grp0_change_count = 0;
        self.current_grp1_change_count = 0;
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
                let new_vsync = (val & 0x02) != 0;

                // Detect VSYNC falling edge (ON -> OFF transition)
                // This is when a new frame begins - the game has finished VSYNC
                // and is about to start the vertical blank period
                if self.vsync && !new_vsync {
                    // Reset scanline counter to 0 at the start of a new frame
                    // This synchronizes our timing with the game's frame structure
                    self.scanline = 0;
                    self.pixel = 0;

                    if LogConfig::global().should_log(LogCategory::PPU, LogLevel::Debug) {
                        eprintln!("[TIA] VSYNC falling edge: reset scanline to 0");
                    }
                }

                self.vsync = new_vsync;
            }
            0x01 => {
                self.writes_vblank = self.writes_vblank.saturating_add(1);
                self.vblank = (val & 0x02) != 0;

                // Bit 6: Latch paddle fire buttons (optional, not commonly used)
                self.paddle_latch_enabled = (val & 0x40) != 0;

                // Bit 7: Dump paddle capacitors to ground
                let new_dump = (val & 0x80) != 0;
                if new_dump && !self.paddle_dump_enabled {
                    // Rising edge: start dumping (grounding capacitors)
                    self.paddle_charge_time = [0, 0, 0, 0];
                    self.inpt0 = 0x00; // Bit 7 = 0 when dumping
                    self.inpt1 = 0x00;
                    self.inpt2 = 0x00;
                    self.inpt3 = 0x00;
                } else if !new_dump && self.paddle_dump_enabled {
                    // Falling edge: stop dumping, begin charging
                    // Capacitors start charging from ground
                    self.paddle_charge_time = [0, 0, 0, 0];
                }
                self.paddle_dump_enabled = new_dump;
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
                    _ => unreachable!(), // Only 2 bits, so only 0-3 possible
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
            // The TIA has a hardware delay of approximately 4-5 color clocks
            // between the strobe and when the position counter actually resets.
            // This effectively adds 4-5 pixels to the x position.
            0x10 => {
                let x = self.current_visible_x();
                // Add 4 pixel delay for positioning (accounts for TIA hardware delay)
                self.player0_x = (x.saturating_add(4)).min(159);
            }
            0x11 => {
                let x = self.current_visible_x();
                self.player1_x = (x.saturating_add(4)).min(159);
            }
            0x12 => {
                if !self.resmp0 {
                    // Only set position if not locked to player
                    let x = self.current_visible_x();
                    self.missile0_x = (x.saturating_add(4)).min(159);
                }
            }
            0x13 => {
                if !self.resmp1 {
                    // Only set position if not locked to player
                    let x = self.current_visible_x();
                    self.missile1_x = (x.saturating_add(4)).min(159);
                }
            }
            0x14 => {
                let x = self.current_visible_x();
                self.ball_x = (x.saturating_add(4)).min(159);
            }

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
            // Per Stella Programmer's Guide: Writing GRP0 also loads the delayed register
            // for player 1 from the current GRP1 value (and vice versa).
            // This allows 6-digit displays using both players with vertical delay.
            0x1B => {
                self.writes_grp0 = self.writes_grp0.saturating_add(1);
                if val != 0 {
                    self.writes_grp0_nonzero = self.writes_grp0_nonzero.saturating_add(1);
                    if LogConfig::global().should_log(LogCategory::PPU, LogLevel::Debug) {
                        eprintln!(
                            "[TIA] GRP0 = 0x{:02X} at scanline {} pixel {}",
                            val, self.scanline, self.pixel
                        );
                    }
                }
                self.grp0 = val;
                // Writing GRP0 copies GRP1 to GRP1_OLD (delayed register for player 1)
                self.grp1_old = self.grp1;
                // Record mid-scanline change for racing-the-beam rendering
                if (self.current_grp0_change_count as usize) < MAX_GRP_CHANGES {
                    let idx = self.current_grp0_change_count as usize;
                    // Convert color clock position to visible pixel (0-159)
                    // Visible area starts at color clock 68
                    let visible_pixel = if self.pixel >= 68 {
                        ((self.pixel - 68) as u8).min(159)
                    } else {
                        0
                    };
                    self.current_grp0_changes[idx] = GrpChange {
                        pixel: visible_pixel,
                        value: val,
                    };
                    self.current_grp0_change_count += 1;
                }
            }
            0x1C => {
                self.writes_grp1 = self.writes_grp1.saturating_add(1);
                if val != 0 {
                    self.writes_grp1_nonzero = self.writes_grp1_nonzero.saturating_add(1);
                    if LogConfig::global().should_log(LogCategory::PPU, LogLevel::Debug) {
                        eprintln!(
                            "[TIA] GRP1 = 0x{:02X} at scanline {} pixel {}",
                            val, self.scanline, self.pixel
                        );
                    }
                }
                self.grp1 = val;
                // Writing GRP1 copies GRP0 to GRP0_OLD (delayed register for player 0)
                // and also copies ENABL to ENABL_OLD (delayed register for ball)
                self.grp0_old = self.grp0;
                self.enabl_old = if self.enabl { 0x02 } else { 0x00 };
                // Record mid-scanline change for racing-the-beam rendering
                if (self.current_grp1_change_count as usize) < MAX_GRP_CHANGES {
                    let idx = self.current_grp1_change_count as usize;
                    // Convert color clock position to visible pixel (0-159)
                    // Visible area starts at color clock 68
                    let visible_pixel = if self.pixel >= 68 {
                        ((self.pixel - 68) as u8).min(159)
                    } else {
                        0
                    };
                    self.current_grp1_changes[idx] = GrpChange {
                        pixel: visible_pixel,
                        value: val,
                    };
                    self.current_grp1_change_count += 1;
                }
            }

            // Enable missiles and ball
            0x1D => self.enam0 = (val & 0x02) != 0,
            0x1E => self.enam1 = (val & 0x02) != 0,
            0x1F => {
                self.enabl = (val & 0x02) != 0;
            }

            // Horizontal motion
            0x20 => self.hmp0 = (val as i8) >> 4,
            0x21 => self.hmp1 = (val as i8) >> 4,
            0x22 => self.hmm0 = (val as i8) >> 4,
            0x23 => self.hmm1 = (val as i8) >> 4,
            0x24 => self.hmbl = (val as i8) >> 4,

            // Delayed graphics enable
            0x25 => self.vdelp0 = (val & 0x01) != 0, // VDELP0
            0x26 => self.vdelp1 = (val & 0x01) != 0, // VDELP1
            0x27 => self.vdelbl = (val & 0x01) != 0, // VDELBL

            // Reset missile to player
            0x28 => self.resmp0 = (val & 0x02) != 0, // RESMP0
            0x29 => self.resmp1 = (val & 0x02) != 0, // RESMP1

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
            0x08 => self.inpt0,  // Input port 0 (Paddle 0)
            0x09 => self.inpt1,  // Input port 1 (Paddle 1)
            0x0A => self.inpt2,  // Input port 2 (Paddle 2)
            0x0B => self.inpt3,  // Input port 3 (Paddle 3)
            0x0C => self.inpt4,  // Input port 4 (Player 0 fire button)
            0x0D => self.inpt5,  // Input port 5 (Player 1 fire button)
            _ => 0,
        }
    }

    /// Clock the TIA for one CPU cycle (3 color clocks)
    pub fn clock(&mut self) {
        // Update paddle capacitor charging (every color clock)
        self.update_paddle_charging();

        // Cycle-accurate: Process each color clock individually
        // This ensures mid-scanline register changes affect pixels correctly
        for _ in 0..3 {
            self.clock_color_clock();
        }
    }

    /// Clock one color clock (used in cycle-accurate mode)
    fn clock_color_clock(&mut self) {
        self.pixel += 1;

        if self.pixel >= 228 {
            self.pixel = 0;
            let old_scanline = self.scanline;

            // In cycle-accurate mode, latch state at scanline boundaries
            self.latch_scanline_state(old_scanline);

            self.scanline += 1;
            self.scanline_counter = self.scanline_counter.saturating_add(1);

            let total_scanlines = self.video_mode.scanlines_per_frame();
            if self.scanline >= total_scanlines {
                self.scanline = 0;
            }

            // Debug logging
            if LogConfig::global().should_log(LogCategory::PPU, LogLevel::Debug)
                && (old_scanline == 261 || self.scanline <= 1)
            {
                eprintln!(
                    "[TIA CLOCK] Scanline {} -> {} (latched {})",
                    old_scanline, self.scanline, old_scanline
                );
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

    /// Get current VSYNC state
    pub fn vsync(&self) -> bool {
        self.vsync
    }

    /// Prepare for a new frame capture.
    ///
    /// The Atari 2600 can generate frames with slightly varying scanline counts;
    /// for stable host-side rendering we treat VSYNC boundaries as frame delimiters.
    /// Clearing latched scanline state here ensures the renderer uses a coherent
    /// set of scanlines from a single frame.
    pub fn begin_new_frame(&mut self) {
        for state in &mut self.scanline_states {
            *state = ScanlineState::default();
        }
        // DO NOT clear cached_visible_start here - it must persist across frames
        // to prevent vertical jumping (as documented in visible_window_start_scanline)

        // DO NOT reset scanline or pixel counters here!
        // The TIA continues to run with consistent timing across frame boundaries.
        // Resetting these counters creates timing discontinuities that break horizontal
        // positioning (current_visible_x() becomes incorrect for sprite RESPx/RESMx/RESBL).
        // Instead, the frame boundary is tracked externally and rendering uses modulo
        // arithmetic to map TIA scanlines to framebuffer rows.
    }

    /// Determine the first visible scanline in *absolute* TIA scanline coordinates,
    /// searching in frame order starting from a caller-provided frame boundary.
    ///
    /// This avoids the classic "rolling" symptom when the host renders 192 lines
    /// from a drifting reference point.
    pub fn visible_window_start_scanline_from(&self, frame_start_scanline: u16) -> u16 {
        let debug = LogConfig::global().should_log(LogCategory::PPU, LogLevel::Debug);
        let total_scanlines = self.video_mode.scanlines_per_frame();

        for offset in 1..total_scanlines {
            let prev_idx = (frame_start_scanline + offset - 1) % total_scanlines;
            let cur_idx = (frame_start_scanline + offset) % total_scanlines;

            let prev = self
                .scanline_states
                .get(prev_idx as usize)
                .copied()
                .unwrap_or_default();
            let cur = self
                .scanline_states
                .get(cur_idx as usize)
                .copied()
                .unwrap_or_default();

            if debug && offset < 100 {
                eprintln!(
                    "[VISIBLE] abs_prev={} abs_cur={} prev.vblank={} cur.vblank={}",
                    prev_idx, cur_idx, prev.vblank, cur.vblank
                );
            }

            if prev.vblank && !cur.vblank {
                if debug {
                    eprintln!(
                        "[VISIBLE] Found transition at abs scanline {} (frame_start={})",
                        cur_idx, frame_start_scanline
                    );
                }
                return cur_idx;
            }
        }

        // Fallback: common visible start is around scanline ~40 after VSYNC (NTSC)
        // For PAL, this is also reasonable as a fallback
        (frame_start_scanline + 40) % total_scanlines
    }

    /// Try to infer the start of the visible picture area based on VBLANK timing
    ///
    /// This method caches the first detected visible start to prevent vertical jumping
    /// between frames. However, it waits for a few frames to ensure stable detection
    /// before caching the value.
    pub fn visible_window_start_scanline(&mut self) -> u16 {
        // Increment frame counter for detection stability
        self.visible_window_frame_count = self.visible_window_frame_count.saturating_add(1);

        // If we already have a cached value, always use it for stability
        // This prevents vertical jumping even if VBLANK timing varies slightly
        if let Some(cached) = self.cached_visible_start {
            return cached;
        }

        let total_scanlines = self.video_mode.scanlines_per_frame() as usize;

        // Debug: dump VBLANK pattern on first detection
        if LogConfig::global().should_log(LogCategory::PPU, LogLevel::Debug) {
            self.debug_vblank_pattern();
        }

        // Standard Atari 2600 frame structure:
        // - Scanlines 0-2: VSYNC (3 lines)
        // - Scanlines 3-39: VBLANK (37 lines)
        // - Scanlines 40-231: Visible (192 lines)
        // - Scanlines 232-261: Overscan (30 lines)
        //
        // We look for VBLANK turning OFF, but only consider transitions
        // after scanline 10 to avoid false positives from early VBLANK changes.
        // The visible area typically starts between scanlines 30-50.

        // First detection: find where VBLANK transitions from true to false
        // Skip the first 10 scanlines to avoid noise during VSYNC period
        let mut detected_scanline: Option<u16> = None;

        for i in 10..total_scanlines.min(80) {
            let prev = self.scanline_states.get(i - 1).copied().unwrap_or_default();
            let cur = self.scanline_states.get(i).copied().unwrap_or_default();

            if prev.vblank && !cur.vblank {
                detected_scanline = Some(i as u16);
                if LogConfig::global().should_log(LogCategory::PPU, LogLevel::Debug) {
                    eprintln!(
                        "[TIA] visible_window_start VBLANK transition detected at scanline {}",
                        i
                    );
                }
                break;
            }
        }

        // If no VBLANK transition found, try content-based detection
        if detected_scanline.is_none() {
            for i in 10..total_scanlines.min(80) {
                let state = self.scanline_states.get(i).copied().unwrap_or_default();
                // Check if this scanline has any playfield or player graphics, but only when VBLANK is off
                if !state.vblank
                    && (state.pf0 != 0
                        || state.pf1 != 0
                        || state.pf2 != 0
                        || state.grp0 != 0
                        || state.grp1 != 0)
                {
                    detected_scanline = Some(i as u16);
                    if LogConfig::global().should_log(LogCategory::PPU, LogLevel::Debug) {
                        eprintln!(
                            "[TIA] visible_window_start content-based fallback to scanline {}",
                            i
                        );
                    }
                    break;
                }
            }
        }

        // Use detected scanline or fallback to 40
        let scanline = detected_scanline.unwrap_or(40);

        // Only cache after we've seen a few frames to ensure stable detection
        // This prevents caching an incorrect value from the first incomplete frame
        const FRAMES_BEFORE_CACHE: u32 = 3;
        if self.visible_window_frame_count >= FRAMES_BEFORE_CACHE {
            if LogConfig::global().should_log(LogCategory::PPU, LogLevel::Debug) {
                eprintln!(
                    "[TIA] Caching visible_window_start at scanline {} (frame {})",
                    scanline, self.visible_window_frame_count
                );
            }
            self.cached_visible_start = Some(scanline);
        } else if LogConfig::global().should_log(LogCategory::PPU, LogLevel::Debug) {
            eprintln!(
                "[TIA] visible_window_start at scanline {} (frame {}, not caching yet)",
                scanline, self.visible_window_frame_count
            );
        }

        scanline
    }

    /// Debug helper: count how many of the visible scanlines have any playfield/player bits.
    pub fn debug_visible_scanline_activity(&self, visible_start: u16) -> (u32, u32) {
        let mut scanlines_with_pf = 0u32;
        let mut scanlines_with_grp = 0u32;
        let total_scanlines = self.video_mode.scanlines_per_frame();
        let visible_lines = self.video_mode.visible_scanlines();

        for visible_line in 0..visible_lines {
            let tia_scanline = (visible_start + visible_line) % total_scanlines;
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

    /// Debug helper: count PF/GRP activity across all scanlines.
    pub fn debug_all_scanline_activity(&self) -> (u32, u32) {
        let mut scanlines_with_pf = 0u32;
        let mut scanlines_with_grp = 0u32;
        let total_scanlines = self.video_mode.scanlines_per_frame() as usize;

        for scanline in 0..total_scanlines {
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

    /// Debug helper: dump VBLANK pattern to understand visible window detection
    pub fn debug_vblank_pattern(&self) {
        let total_scanlines = self.video_mode.scanlines_per_frame() as usize;
        let mut transitions: Vec<(usize, bool, bool)> = Vec::new();

        for i in 1..total_scanlines {
            let prev = self.scanline_states.get(i - 1).copied().unwrap_or_default();
            let cur = self.scanline_states.get(i).copied().unwrap_or_default();
            if prev.vblank != cur.vblank {
                transitions.push((i, prev.vblank, cur.vblank));
            }
        }

        eprintln!("[TIA DEBUG] VBLANK transitions:");
        for (scanline, prev, cur) in &transitions {
            eprintln!(
                "  Scanline {}: {} -> {}",
                scanline,
                if *prev { "VBLANK" } else { "visible" },
                if *cur { "VBLANK" } else { "visible" }
            );
        }
        if transitions.is_empty() {
            eprintln!("  (no transitions found - all scanlines have same VBLANK state)");
            let first_state = self.scanline_states.first().copied().unwrap_or_default();
            eprintln!("  First scanline VBLANK = {}", first_state.vblank);
        }
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
            let color = Self::get_pixel_color(&state, x, self.video_mode);
            buffer[visible_line * 160 + x] = color;
        }
    }

    /// Detect and record collisions for a scanline (called during frame rendering)
    /// This should be called once per scanline to update collision registers
    fn detect_collisions_for_scanline(&mut self, tia_scanline: u16) {
        let total_scanlines = self.video_mode.scanlines_per_frame() as usize;
        let state = self
            .scanline_states
            .get((tia_scanline as usize).min(total_scanlines - 1))
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
        // Detect collisions for all visible scanlines
        let visible_lines = self.video_mode.visible_scanlines();
        let total_scanlines = self.video_mode.scanlines_per_frame();

        for visible_line in 0..visible_lines {
            let tia_scanline = (visible_start + visible_line) % total_scanlines;
            self.detect_collisions_for_scanline(tia_scanline);
        }
    }

    /// Get the color of a pixel at the given position using latched state
    fn get_pixel_color(state: &ScanlineState, x: usize, video_mode: VideoMode) -> u32 {
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
                return palette_to_rgb(state.colup0, video_mode);
            }

            // Check Missile 0
            if Self::is_missile_pixel(state, 0, x) {
                return palette_to_rgb(state.colup0, video_mode);
            }

            // Check Player 1
            if Self::is_player_pixel(state, 1, x) {
                return palette_to_rgb(state.colup1, video_mode);
            }

            // Check Missile 1
            if Self::is_missile_pixel(state, 1, x) {
                return palette_to_rgb(state.colup1, video_mode);
            }

            // Check Ball
            if Self::is_ball_pixel(state, x) {
                return palette_to_rgb(state.colupf, video_mode);
            }
        }

        // Check playfield
        if Self::is_playfield_pixel(state, x) {
            return palette_to_rgb(state.colupf, video_mode);
        }

        // Check Ball (if playfield priority)
        if state.playfield_priority && Self::is_ball_pixel(state, x) {
            return palette_to_rgb(state.colupf, video_mode);
        }

        // Check players and missiles (if playfield priority)
        if state.playfield_priority {
            if Self::is_player_pixel(state, 0, x) {
                return palette_to_rgb(state.colup0, video_mode);
            }
            if Self::is_missile_pixel(state, 0, x) {
                return palette_to_rgb(state.colup0, video_mode);
            }
            if Self::is_player_pixel(state, 1, x) {
                return palette_to_rgb(state.colup1, video_mode);
            }
            if Self::is_missile_pixel(state, 1, x) {
                return palette_to_rgb(state.colup1, video_mode);
            }
        }

        // Background color
        palette_to_rgb(state.colubk, video_mode)
    }

    /// Get the GRP0 value that was in effect at a given horizontal pixel position
    /// This handles mid-scanline graphics changes for racing-the-beam effects
    fn get_grp0_at_pixel(state: &ScanlineState, x: usize) -> u8 {
        // If there are no mid-scanline changes, use the latched value
        // (which has VDELP already applied in latch_scanline_state)
        if state.grp0_change_count == 0 {
            return state.grp0;
        }

        // For racing-the-beam displays, find the GRP value that was in effect at position x
        // The changes array contains writes during the scanline in chronological order

        // Find the most recent change at or before this pixel position
        // Start with the value that was latched at the beginning of the scanline
        // (state.grp0 already has VDELP applied from latch_scanline_state)
        let mut result = state.grp0;

        for i in 0..state.grp0_change_count as usize {
            if state.grp0_changes[i].pixel as usize <= x {
                result = state.grp0_changes[i].value;
            } else {
                break; // Changes are in order, no need to check further
            }
        }
        result
    }

    /// Get the GRP1 value that was in effect at a given horizontal pixel position
    /// This handles mid-scanline graphics changes for racing-the-beam effects
    fn get_grp1_at_pixel(state: &ScanlineState, x: usize) -> u8 {
        // If there are no mid-scanline changes, use the latched value
        if state.grp1_change_count == 0 {
            return state.grp1;
        }

        // Find the most recent change at or before this pixel position
        // Start with the value that was latched at the beginning of the scanline
        // (state.grp1 already has VDELP applied from latch_scanline_state)
        let mut result = state.grp1;

        for i in 0..state.grp1_change_count as usize {
            if state.grp1_changes[i].pixel as usize <= x {
                result = state.grp1_changes[i].value;
            } else {
                break;
            }
        }
        result
    }

    /// Check if a player pixel is visible at the given x position
    fn is_player_pixel(state: &ScanlineState, player: usize, x: usize) -> bool {
        let (pos, reflect, nusiz) = if player == 0 {
            (state.player0_x, state.player0_reflect, state.nusiz0)
        } else {
            (state.player1_x, state.player1_reflect, state.nusiz1)
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
            let copy_pos = (pos as usize + copy * spacing) % 160;

            // Check if x is within this copy's range
            if x >= copy_pos && x < copy_pos + 8 * player_size {
                let offset = x - copy_pos;

                // Get the graphics value at the START of this copy's position
                // This is critical for racing-the-beam effects with multiple copies
                let grp = if player == 0 {
                    Self::get_grp0_at_pixel(state, copy_pos)
                } else {
                    Self::get_grp1_at_pixel(state, copy_pos)
                };

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
            let copy_pos = (pos as usize + copy * spacing) % 160;

            // Check if x is within this copy's range
            if x >= copy_pos && x < copy_pos + missile_size {
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
        let ball_pos = state.ball_x as usize;
        let ball_size = state.ball_size as usize;

        // Check if x is within ball's range
        x >= ball_pos && x < ball_pos + ball_size
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

/// Convert palette value to RGB based on video mode
/// - NTSC: 128 colors
/// - PAL: Uses NTSC palette as approximation (proper PAL palette could be added later)
fn palette_to_rgb(value: u8, video_mode: VideoMode) -> u32 {
    match video_mode {
        VideoMode::NTSC => ntsc_to_rgb(value),
        VideoMode::PAL => pal_to_rgb(value),
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

/// Convert PAL palette value to RGB
/// Atari 2600 PAL uses a different color encoding with 104 colors
/// The palette is organized in 8 luminance steps across 13 chroma values
fn pal_to_rgb(pal: u8) -> u32 {
    // PAL palette table for Atari 2600
    // Organized by hue (13 hues) x luminance (8 levels) = 104 colors
    // Based on accurate PAL color values from Lospec and RandomTerrain
    const PAL_PALETTE: [u32; 104] = [
        // Hue 0 (Gray) - Luminance 0-7
        0xFF000000, 0xFF404040, 0xFF6C6C6C, 0xFF909090, 0xFFB0B0B0, 0xFFC8C8C8, 0xFFDCDCDC,
        0xFFECECEC, // Hue 1 (Blue) - Luminance 0-7
        0xFF000088, 0xFF20209C, 0xFF3C3CB0, 0xFF5858C0, 0xFF7070D0, 0xFF8484E0, 0xFF9C9CEC,
        0xFFB0B0FC, // Hue 2 (Purple/Violet) - Luminance 0-7
        0xFF3C0080, 0xFF542094, 0xFF6C3CA8, 0xFF8058BC, 0xFF9470CC, 0xFFA884DC, 0xFFB89CEC,
        0xFFC8B0FC, // Hue 3 (Blue-Cyan) - Luminance 0-7
        0xFF002070, 0xFF1C3C88, 0xFF3858A0, 0xFF5074B4, 0xFF6888C8, 0xFF7CA0DC, 0xFF90B4EC,
        0xFFA4C8FC, // Hue 4 (Magenta) - Luminance 0-7
        0xFF580070, 0xFF6C2088, 0xFF803CA0, 0xFF9458B4, 0xFFA470C8, 0xFFB484DC, 0xFFC49CEC,
        0xFFD4B0FC, // Hue 5 (Cyan) - Luminance 0-7
        0xFF003C70, 0xFF1C5888, 0xFF3874A0, 0xFF508CB4, 0xFF68A4C8, 0xFF7CB8DC, 0xFF90CCEC,
        0xFFA4E0FC, // Hue 6 (Pink/Magenta) - Luminance 0-7
        0xFF70005C, 0xFF842074, 0xFF943C88, 0xFFA8589C, 0xFFB470B0, 0xFFC484C0, 0xFFD09CD0,
        0xFFE0B0E0, // Hue 7 (Cyan-Green) - Luminance 0-7
        0xFF005C5C, 0xFF207474, 0xFF3C8C8C, 0xFF58A4A4, 0xFF70B8B8, 0xFF84C8C8, 0xFF9CDCDC,
        0xFFB0ECEC, // Hue 8 (Red) - Luminance 0-7
        0xFF700014, 0xFF882034, 0xFFA03C50, 0xFFB4586C, 0xFFC87084, 0xFFDC849C, 0xFFEC9CB4,
        0xFFFCB0C8, // Hue 9 (Green) - Luminance 0-7
        0xFF006414, 0xFF208034, 0xFF3C9850, 0xFF58B06C, 0xFF70C484, 0xFF84D89C, 0xFF9CE8B4,
        0xFFB0FCC8, // Hue 10 (Orange-Brown) - Luminance 0-7
        0xFF703400, 0xFF885020, 0xFFA0683C, 0xFFB48458, 0xFFC89870, 0xFFDCAC84, 0xFFECC09C,
        0xFFFCD4B0, // Hue 11 (Yellow-Green) - Luminance 0-7
        0xFF445C00, 0xFF5C7820, 0xFF74903C, 0xFF8CAC58, 0xFFA0C070, 0xFFB0D484, 0xFFC0E89C,
        0xFFD4FCB0, // Hue 12 (Yellow-Orange) - Luminance 0-7
        0xFF805800, 0xFF947020, 0xFFA8843C, 0xFFBC9C58, 0xFFCCAC70, 0xFFDCC084, 0xFFECD09C,
        0xFFFCE0B0,
    ];

    // PAL uses 104 colors, so we need to map the 7-bit value appropriately
    // The encoding is slightly different from NTSC
    let index = pal as usize & 0x7F;

    // Map to 104-color palette (some values may repeat)
    if index < PAL_PALETTE.len() {
        PAL_PALETTE[index]
    } else {
        // For values beyond 104, wrap around or use black
        PAL_PALETTE[index % PAL_PALETTE.len()]
    }
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

        tia.write(0x14, 0x00); // RESBL - position ball (hardware adds 4-pixel delay)
        tia.write(0x1F, 0x02); // ENABL - enable ball

        // Test 1-pixel ball (CTRLPF bits 4-5 = 00)
        tia.write(0x0A, 0x00);
        tia.latch_scanline_state(0);
        let state = tia.scanline_states[0];
        assert_eq!(state.ball_size, 1);
        // Ball is at position 4 due to hardware delay
        assert!(Tia::is_ball_pixel(&state, 4));
        assert!(!Tia::is_ball_pixel(&state, 5));

        // Test 2-pixel ball (CTRLPF bits 4-5 = 01)
        tia.write(0x0A, 0x10);
        tia.latch_scanline_state(0);
        let state = tia.scanline_states[0];
        assert_eq!(state.ball_size, 2);
        assert!(Tia::is_ball_pixel(&state, 4));
        assert!(Tia::is_ball_pixel(&state, 5));
        assert!(!Tia::is_ball_pixel(&state, 6));

        // Test 4-pixel ball (CTRLPF bits 4-5 = 10)
        tia.write(0x0A, 0x20);
        tia.latch_scanline_state(0);
        let state = tia.scanline_states[0];
        assert_eq!(state.ball_size, 4);
        assert!(Tia::is_ball_pixel(&state, 4));
        assert!(Tia::is_ball_pixel(&state, 7));
        assert!(!Tia::is_ball_pixel(&state, 8));

        // Test 8-pixel ball (CTRLPF bits 4-5 = 11)
        tia.write(0x0A, 0x30);
        tia.latch_scanline_state(0);
        let state = tia.scanline_states[0];
        assert_eq!(state.ball_size, 8);
        assert!(Tia::is_ball_pixel(&state, 4));
        assert!(Tia::is_ball_pixel(&state, 11));
        assert!(!Tia::is_ball_pixel(&state, 12));
    }

    #[test]
    fn test_vdelbl_delayed_ball_graphics() {
        let mut tia = Tia::new();

        tia.write(0x14, 0x00); // RESBL - position ball at x=0
        tia.write(0x08, 0x0E); // COLUPF - set color

        // Enable delayed ball graphics
        tia.write(0x27, 0x01); // VDELBL

        // Enable ball first
        tia.write(0x1F, 0x02); // ENABL = 1
        tia.latch_scanline_state(0);
        let state = tia.scanline_states[0];
        // With VDELBL, should still be using old value (which was 0 at start)
        assert!(!state.enabl);

        // Write to GRP1 to trigger the delayed ball update
        // According to TIA spec, writing GRP1 updates the delayed ball state
        tia.write(0x1C, 0x00); // GRP1
        tia.latch_scanline_state(1);
        let state = tia.scanline_states[1];
        // Now it should show the previous ENABL write's enabled state
        assert!(state.enabl);

        // Disable ball
        tia.write(0x1F, 0x00); // ENABL = 0
        tia.latch_scanline_state(2);
        let state = tia.scanline_states[2];
        // Should still be enabled (showing previous state)
        assert!(state.enabl);

        // Write to GRP1 again to update the delayed ball state
        tia.write(0x1C, 0x00); // GRP1
        tia.latch_scanline_state(3);
        let state = tia.scanline_states[3];
        // Now it should be disabled
        assert!(!state.enabl);
    }

    #[test]
    fn test_resmp_missile_to_player() {
        let mut tia = Tia::new();

        // Position player 0 at x=50 (pixel is in color clocks, not screen pixels)
        tia.pixel = 68 + 50; // HBLANK + 50 color clocks
        tia.write(0x10, 0x00); // RESP0

        // Enable missile 0
        tia.write(0x1D, 0x02); // ENAM0

        // Position missile 0 at x=10 initially (without RESMP)
        tia.pixel = 68 + 10;
        // Hardware adds 4-pixel delay to positioning
        tia.write(0x12, 0x00); // RESM0
        assert_eq!(tia.missile0_x, 14); // 10 + 4

        // Enable RESMP0 - lock missile to player
        tia.write(0x28, 0x02); // RESMP0
        tia.latch_scanline_state(0);

        // Missile should now be at player position + 4 (center of 8-pixel player)
        // Player is at 50 + 4 = 54, missile at 54 + 4 = 58
        assert_eq!(tia.missile0_x, 58);

        // Try to move missile - should be ignored when RESMP is on
        tia.pixel = 68 + 100;
        tia.write(0x12, 0x00); // RESM0 - should be ignored
        tia.latch_scanline_state(1);
        assert_eq!(tia.missile0_x, 58); // Still locked to player + 4

        // Disable RESMP0
        tia.write(0x28, 0x00); // RESMP0 = 0
        tia.pixel = 68 + 20;
        tia.write(0x12, 0x00); // RESM0 - should work now
        assert_eq!(tia.missile0_x, 24); // Free to move again (20 + 4)
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
        assert_eq!(tia.grp0_old, 0x00); // Old value is 0 (not updated by writing GRP0)

        // Write to GRP1 to trigger the delayed register update for GRP0
        // According to TIA spec, writing GRP1 copies current GRP0 to GRP0_OLD
        tia.write(0x1C, 0x55); // GRP1 = 0x55
        assert_eq!(tia.grp0, 0xAA); // GRP0 unchanged
        assert_eq!(tia.grp0_old, 0xAA); // Now GRP0_OLD has the GRP0 value

        // Write new graphics to GRP0
        tia.write(0x1B, 0xFF); // GRP0 = 0xFF
        assert_eq!(tia.grp0, 0xFF);

        // Enable delayed graphics
        tia.write(0x25, 0x01); // VDELP0
        assert!(tia.vdelp0);

        // When latching state, delayed graphics should use old value
        tia.latch_scanline_state(0);
        let state = tia.scanline_states[0];
        assert_eq!(state.grp0, 0xAA); // Uses old value when VDELP0 is set
    }

    #[test]
    fn test_color_register_addresses() {
        // Verify all color registers map to correct addresses per spec
        let mut tia = Tia::new();

        // COLUP0 = $06
        tia.write(0x06, 0x42);
        assert_eq!(tia.colup0, 0x42);

        // COLUP1 = $07
        tia.write(0x07, 0x84);
        assert_eq!(tia.colup1, 0x84);

        // COLUPF = $08
        tia.write(0x08, 0xC6);
        assert_eq!(tia.colupf, 0xC6);

        // COLUBK = $09
        tia.write(0x09, 0x00);
        assert_eq!(tia.colubk, 0x00);
    }

    #[test]
    fn test_playfield_register_addresses() {
        // Verify playfield registers at correct addresses per spec
        let mut tia = Tia::new();

        // PF0 = $0D (4-bit, reversed)
        tia.write(0x0D, 0xF0);
        assert_eq!(tia.pf0, 0xF0);

        // PF1 = $0E (8-bit, MSB first)
        tia.write(0x0E, 0xAA);
        assert_eq!(tia.pf1, 0xAA);

        // PF2 = $0F (8-bit)
        tia.write(0x0F, 0x55);
        assert_eq!(tia.pf2, 0x55);
    }

    #[test]
    fn test_ctrlpf_ball_sizing() {
        // CTRLPF bits 4-5 control ball size per spec
        let mut tia = Tia::new();

        // Size 00 = 1 pixel
        tia.write(0x0A, 0x00);
        assert_eq!(tia.ball_size, 1);

        // Size 01 = 2 pixels
        tia.write(0x0A, 0x10);
        assert_eq!(tia.ball_size, 2);

        // Size 10 = 4 pixels
        tia.write(0x0A, 0x20);
        assert_eq!(tia.ball_size, 4);

        // Size 11 = 8 pixels
        tia.write(0x0A, 0x30);
        assert_eq!(tia.ball_size, 8);
    }

    #[test]
    fn test_ctrlpf_playfield_modes() {
        // CTRLPF bits 0-2 control playfield behavior per spec
        let mut tia = Tia::new();

        // Bit 0 = reflection
        tia.write(0x0A, 0x01);
        assert!(tia.playfield_reflect);
        assert!(!tia.playfield_score_mode);
        assert!(!tia.playfield_priority);

        // Bit 1 = score mode
        tia.write(0x0A, 0x02);
        assert!(!tia.playfield_reflect);
        assert!(tia.playfield_score_mode);
        assert!(!tia.playfield_priority);

        // Bit 2 = priority
        tia.write(0x0A, 0x04);
        assert!(!tia.playfield_reflect);
        assert!(!tia.playfield_score_mode);
        assert!(tia.playfield_priority);

        // Multiple bits
        tia.write(0x0A, 0x07);
        assert!(tia.playfield_reflect);
        assert!(tia.playfield_score_mode);
        assert!(tia.playfield_priority);
    }

    #[test]
    fn test_horizontal_motion_signed_values() {
        // HMxx registers use signed 4-bit values (upper nibble)
        let mut tia = Tia::new();

        // Positive motion: $10 = +1
        tia.write(0x20, 0x10);
        assert_eq!(tia.hmp0, 1);

        // Negative motion: $F0 = -1
        tia.write(0x21, 0xF0);
        assert_eq!(tia.hmp1, -1);

        // Maximum positive: $70 = +7
        tia.write(0x22, 0x70);
        assert_eq!(tia.hmm0, 7);

        // Maximum negative: $80 = -8
        tia.write(0x23, 0x80);
        assert_eq!(tia.hmm1, -8);
    }

    #[test]
    fn test_audio_register_masking() {
        // Audio registers have specific bit masks per spec
        let mut tia = Tia::new();

        // AUDC (4-bit control)
        tia.write(0x15, 0xFF);
        assert_eq!(tia.audc0, 0x0F); // Only lower 4 bits

        // AUDF (5-bit frequency)
        tia.write(0x17, 0xFF);
        assert_eq!(tia.audf0, 0x1F); // Only lower 5 bits

        // AUDV (4-bit volume)
        tia.write(0x19, 0xFF);
        assert_eq!(tia.audv0, 0x0F); // Only lower 4 bits
    }

    #[test]
    fn test_enable_registers_bit_1() {
        // ENAM0, ENAM1, ENABL use bit 1 (0x02) per spec
        let mut tia = Tia::new();

        // Test ENAM0
        tia.write(0x1D, 0x00);
        assert!(!tia.enam0);
        tia.write(0x1D, 0x02);
        assert!(tia.enam0);
        tia.write(0x1D, 0xFF); // Other bits don't matter
        assert!(tia.enam0);

        // Test ENAM1
        tia.write(0x1E, 0x00);
        assert!(!tia.enam1);
        tia.write(0x1E, 0x02);
        assert!(tia.enam1);

        // Test ENABL
        tia.write(0x1F, 0x00);
        assert!(!tia.enabl);
        tia.write(0x1F, 0x02);
        assert!(tia.enabl);
    }

    #[test]
    fn test_vsync_vblank_bit_1() {
        // VSYNC and VBLANK use bit 1 (0x02) per spec
        let mut tia = Tia::new();

        // Test VSYNC
        tia.write(0x00, 0x00);
        assert!(!tia.vsync);
        tia.write(0x00, 0x02);
        assert!(tia.vsync);
        tia.write(0x00, 0xFF); // Other bits don't matter
        assert!(tia.vsync);

        // Test VBLANK
        tia.write(0x01, 0x00);
        assert!(!tia.vblank);
        tia.write(0x01, 0x02);
        assert!(tia.vblank);
    }

    #[test]
    fn test_player_reflect_bit_3() {
        // REFP0/REFP1 use bit 3 (0x08) per spec
        let mut tia = Tia::new();

        // Test REFP0
        tia.write(0x0B, 0x00);
        assert!(!tia.player0_reflect);
        tia.write(0x0B, 0x08);
        assert!(tia.player0_reflect);

        // Test REFP1
        tia.write(0x0C, 0x00);
        assert!(!tia.player1_reflect);
        tia.write(0x0C, 0x08);
        assert!(tia.player1_reflect);
    }

    #[test]
    fn test_resmp_bit_1() {
        // RESMP0/RESMP1 use bit 1 (0x02) per spec
        let mut tia = Tia::new();

        // Test RESMP0
        tia.write(0x28, 0x00);
        assert!(!tia.resmp0);
        tia.write(0x28, 0x02);
        assert!(tia.resmp0);

        // Test RESMP1
        tia.write(0x29, 0x00);
        assert!(!tia.resmp1);
        tia.write(0x29, 0x02);
        assert!(tia.resmp1);
    }

    #[test]
    fn test_vdel_bit_0() {
        // VDELP0, VDELP1, VDELBL use bit 0 (0x01) per spec
        let mut tia = Tia::new();

        // Test VDELP0
        tia.write(0x25, 0x00);
        assert!(!tia.vdelp0);
        tia.write(0x25, 0x01);
        assert!(tia.vdelp0);

        // Test VDELP1
        tia.write(0x26, 0x00);
        assert!(!tia.vdelp1);
        tia.write(0x26, 0x01);
        assert!(tia.vdelp1);

        // Test VDELBL
        tia.write(0x27, 0x00);
        assert!(!tia.vdelbl);
        tia.write(0x27, 0x01);
        assert!(tia.vdelbl);
    }

    #[test]
    fn test_collision_register_read_addresses() {
        // Verify collision registers at correct read addresses per spec
        let mut tia = Tia::new();

        // Set collision bits manually for testing
        tia.cxm0p = 0x80;
        tia.cxm1p = 0x40;
        tia.cxp0fb = 0xC0;
        tia.cxp1fb = 0x80;
        tia.cxm0fb = 0x40;
        tia.cxm1fb = 0xC0;
        tia.cxblpf = 0x80;
        tia.cxppmm = 0x40;

        // Read collision registers
        assert_eq!(tia.read(0x00), 0x80); // CXM0P
        assert_eq!(tia.read(0x01), 0x40); // CXM1P
        assert_eq!(tia.read(0x02), 0xC0); // CXP0FB
        assert_eq!(tia.read(0x03), 0x80); // CXP1FB
        assert_eq!(tia.read(0x04), 0x40); // CXM0FB
        assert_eq!(tia.read(0x05), 0xC0); // CXM1FB
        assert_eq!(tia.read(0x06), 0x80); // CXBLPF
        assert_eq!(tia.read(0x07), 0x40); // CXPPMM
    }

    #[test]
    fn test_input_register_read_addresses() {
        // Verify input registers at correct read addresses per spec
        let mut tia = Tia::new();

        // Set fire button states
        tia.inpt4 = 0x00; // Pressed (bit 7 = 0)
        tia.inpt5 = 0x80; // Released (bit 7 = 1)

        // Read input registers
        assert_eq!(tia.read(0x0C), 0x00); // INPT4 - pressed
        assert_eq!(tia.read(0x0D), 0x80); // INPT5 - released
    }

    #[test]
    fn test_paddle_position_setting() {
        // Test setting paddle positions via public API
        let mut tia = Tia::new();

        // Set paddle positions
        tia.set_paddle_position(0, 0); // Fully left
        tia.set_paddle_position(1, 128); // Center
        tia.set_paddle_position(2, 255); // Fully right
        tia.set_paddle_position(3, 64); // Quarter turn

        assert_eq!(tia.paddle_positions[0], 0);
        assert_eq!(tia.paddle_positions[1], 128);
        assert_eq!(tia.paddle_positions[2], 255);
        assert_eq!(tia.paddle_positions[3], 64);

        // Out of range paddle should be ignored
        tia.set_paddle_position(4, 100);
        // No crash, just ignored
    }

    #[test]
    fn test_paddle_capacitor_dump() {
        // Test VBLANK bit 7 (dump paddle capacitors)
        let mut tia = Tia::new();

        // Initially not dumping
        assert!(!tia.paddle_dump_enabled);

        // Enable dump (VBLANK bit 7 = 1)
        tia.write(0x01, 0x80);
        assert!(tia.paddle_dump_enabled);

        // All paddle inputs should read 0 when dumping
        assert_eq!(tia.read(0x08), 0x00); // INPT0
        assert_eq!(tia.read(0x09), 0x00); // INPT1
        assert_eq!(tia.read(0x0A), 0x00); // INPT2
        assert_eq!(tia.read(0x0B), 0x00); // INPT3

        // Disable dump (VBLANK bit 7 = 0)
        tia.write(0x01, 0x00);
        assert!(!tia.paddle_dump_enabled);
    }

    #[test]
    fn test_paddle_charging_simulation() {
        // Test that paddle capacitors charge after dump is released
        let mut tia = Tia::new();

        // Set a paddle position
        tia.set_paddle_position(0, 0); // Fast charge (low resistance)

        // Dump capacitors
        tia.write(0x01, 0x80);
        assert_eq!(tia.read(0x08) & 0x80, 0x00); // Not charged

        // Release dump and let it charge
        tia.write(0x01, 0x00);

        // Simulate time passing by calling update_paddle_charging
        // For position 0, threshold is ~56000 color clocks
        for _ in 0..20000 {
            tia.update_paddle_charging();
        }
        // Should still be charging
        assert_eq!(tia.read(0x08) & 0x80, 0x00);

        // Continue charging
        for _ in 0..40000 {
            tia.update_paddle_charging();
        }
        // Should now be charged (bit 7 = 1)
        assert_eq!(tia.read(0x08) & 0x80, 0x80);
    }

    #[test]
    fn test_paddle_position_affects_charge_time() {
        // Test that different paddle positions result in different charge times
        let mut tia1 = Tia::new();
        let mut tia2 = Tia::new();

        // Set different positions
        tia1.set_paddle_position(0, 0); // Fast charge
        tia2.set_paddle_position(0, 255); // Slow charge

        // Dump and release
        tia1.write(0x01, 0x80);
        tia2.write(0x01, 0x80);
        tia1.write(0x01, 0x00);
        tia2.write(0x01, 0x00);

        // Charge for same amount of time
        for _ in 0..60000 {
            tia1.update_paddle_charging();
            tia2.update_paddle_charging();
        }

        // Position 0 should be charged
        assert_eq!(tia1.read(0x08) & 0x80, 0x80);

        // Position 255 should not yet be charged (needs more time)
        assert_eq!(tia2.read(0x08) & 0x80, 0x00);
    }

    #[test]
    fn test_paddle_register_addresses() {
        // Test that INPT0-3 are at correct addresses
        let mut tia = Tia::new();

        // Set paddle inputs manually
        tia.inpt0 = 0x80;
        tia.inpt1 = 0x00;
        tia.inpt2 = 0x80;
        tia.inpt3 = 0x00;

        assert_eq!(tia.read(0x08), 0x80); // INPT0
        assert_eq!(tia.read(0x09), 0x00); // INPT1
        assert_eq!(tia.read(0x0A), 0x80); // INPT2
        assert_eq!(tia.read(0x0B), 0x00); // INPT3
    }

    #[test]
    fn test_vblank_bit6_latch() {
        // Test VBLANK bit 6 (latch paddle fire buttons)
        let mut tia = Tia::new();

        // Initially not latched
        assert!(!tia.paddle_latch_enabled);

        // Enable latch (VBLANK bit 6 = 1, also set bit 7 to test combination)
        tia.write(0x01, 0x40);
        assert!(tia.paddle_latch_enabled);
        assert!(!tia.paddle_dump_enabled);

        // Both bits
        tia.write(0x01, 0xC0);
        assert!(tia.paddle_latch_enabled);
        assert!(tia.paddle_dump_enabled);

        // Disable
        tia.write(0x01, 0x00);
        assert!(!tia.paddle_latch_enabled);
        assert!(!tia.paddle_dump_enabled);
    }

    #[test]
    fn test_pal_palette_differs_from_ntsc() {
        use crate::video_mode::VideoMode;

        // Test that PAL and NTSC palettes produce different colors
        let ntsc_color = palette_to_rgb(0x20, VideoMode::NTSC);
        let pal_color = palette_to_rgb(0x20, VideoMode::PAL);

        // They should be different
        assert_ne!(ntsc_color, pal_color);

        // PAL should have valid RGB values
        assert_ne!(pal_color, 0);
        assert_eq!(pal_color & 0xFF000000, 0xFF000000); // Alpha channel should be 0xFF
    }

    #[test]
    fn test_pal_palette_gray_scale() {
        use crate::video_mode::VideoMode;

        // Test PAL grayscale (hue 0, different luminance values)
        let black = palette_to_rgb(0x00, VideoMode::PAL);
        let mid_gray = palette_to_rgb(0x03, VideoMode::PAL);
        let light_gray = palette_to_rgb(0x06, VideoMode::PAL);

        // Black should be darkest
        assert_eq!(black, 0xFF000000);

        // Extract RGB values (ignore alpha)
        let mid_val = (mid_gray & 0x00FFFFFF) as i64;
        let light_val = (light_gray & 0x00FFFFFF) as i64;

        // Mid gray should be lighter than black
        assert!(mid_val > 0);

        // Light gray should be lighter than mid gray
        // (In the palette, higher luminance = brighter)
        assert!(light_val > mid_val);
    }
}
