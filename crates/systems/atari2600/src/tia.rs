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
//! - Can be sized, duplicated, and positioned (NUSIZ registers - stored but not fully implemented)
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
//! when different objects overlap. This implementation stores these registers but always returns 0
//! (simplified implementation).
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
//! ## Known Limitations
//!
//! 1. **Player/Missile Sizing**: NUSIZ registers are stored but not used for sizing/duplication
//! 2. **Horizontal Motion**: HMxx registers are stored but motion is not applied
//! 3. **Collision Detection**: Registers exist but always return 0
//! 4. **Delayed Graphics**: Old/new graphics registers not implemented
//!
//! These limitations represent acceptable trade-offs for a functional emulator. Most games
//! will display correctly with the current implementation.

use emu_core::apu::PolynomialCounter;
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

            audio0: PolynomialCounter::new(),
            audio1: PolynomialCounter::new(),

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
    #[allow(dead_code)]
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
        if !self.playfield_priority {
            // Check Player 0
            if self.is_player_pixel(0, x) {
                return ntsc_to_rgb(self.colup0);
            }

            // Check Missile 0
            if self.is_missile_pixel(0, x) {
                return ntsc_to_rgb(self.colup0);
            }

            // Check Player 1
            if self.is_player_pixel(1, x) {
                return ntsc_to_rgb(self.colup1);
            }

            // Check Missile 1
            if self.is_missile_pixel(1, x) {
                return ntsc_to_rgb(self.colup1);
            }

            // Check Ball
            if self.is_ball_pixel(x) {
                return ntsc_to_rgb(self.colupf);
            }
        }

        // Check playfield
        if self.is_playfield_pixel(x) {
            return ntsc_to_rgb(self.colupf);
        }

        // Check Ball (if playfield priority)
        if self.playfield_priority && self.is_ball_pixel(x) {
            return ntsc_to_rgb(self.colupf);
        }

        // Check players and missiles (if playfield priority)
        if self.playfield_priority {
            if self.is_player_pixel(0, x) {
                return ntsc_to_rgb(self.colup0);
            }
            if self.is_missile_pixel(0, x) {
                return ntsc_to_rgb(self.colup0);
            }
            if self.is_player_pixel(1, x) {
                return ntsc_to_rgb(self.colup1);
            }
            if self.is_missile_pixel(1, x) {
                return ntsc_to_rgb(self.colup1);
            }
        }

        // Background color
        ntsc_to_rgb(self.colubk)
    }

    /// Check if a player pixel is visible at the given x position
    fn is_player_pixel(&self, player: usize, x: usize) -> bool {
        let (grp, pos, reflect) = if player == 0 {
            (self.grp0, self.player0_x, self.player0_reflect)
        } else {
            (self.grp1, self.player1_x, self.player1_reflect)
        };

        // Calculate pixel offset from player position
        let offset = x.wrapping_sub(pos as usize);
        if offset >= 8 {
            return false; // Outside player sprite
        }

        // Get the bit from the graphics register
        let bit = if reflect {
            offset // Normal order when reflected
        } else {
            7 - offset // Reverse order when not reflected
        };

        (grp & (1 << bit)) != 0
    }

    /// Check if a missile pixel is visible at the given x position
    fn is_missile_pixel(&self, missile: usize, x: usize) -> bool {
        let (enabled, pos) = if missile == 0 {
            (self.enam0, self.missile0_x)
        } else {
            (self.enam1, self.missile1_x)
        };

        if !enabled {
            return false;
        }

        // Missiles are 1 pixel wide by default
        let offset = x.wrapping_sub(pos as usize);
        offset < 1
    }

    /// Check if the ball pixel is visible at the given x position
    fn is_ball_pixel(&self, x: usize) -> bool {
        if !self.enabl {
            return false;
        }

        // Ball is 1 pixel wide by default
        let offset = x.wrapping_sub(self.ball_x as usize);
        offset < 1
    }

    /// Check if a pixel is part of the playfield
    fn is_playfield_pixel(&self, x: usize) -> bool {
        // Playfield is 40 bits wide, mirrored or repeated
        if x < 80 {
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
        }
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
        tia.render_scanline(&mut frame, 0);

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
        tia.render_scanline(&mut frame, 0);

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
        tia.render_scanline(&mut frame, 0);

        // Ball should be visible at x=100
        assert_ne!(frame[100], ntsc_to_rgb(0));
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

        // Without priority, player should be in front
        tia.playfield_priority = false;
        tia.render_scanline(&mut frame, 0);
        let player_color = frame[0];

        // With priority, playfield should be in front
        tia.playfield_priority = true;
        tia.render_scanline(&mut frame, 0);
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
        tia.player0_reflect = false;
        tia.render_scanline(&mut frame_normal, 0);

        let mut frame_reflect = vec![0u32; 160];
        tia.player0_reflect = true;
        tia.render_scanline(&mut frame_reflect, 0);

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
}
