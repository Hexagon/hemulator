//! GTIA (George's Television Interface Adapter)
//!
//! The GTIA handles color generation, player-missile graphics, and collision detection.
//! It works in conjunction with the ANTIC chip which provides the playfield data.
//!
//! # Color Registers
//! - COLPM0-COLPM3 ($D012-$D015): Player/missile colors
//! - COLPF0-COLPF3 ($D016-$D019): Playfield colors
//! - COLBK ($D01A): Background color
//!
//! # Player-Missile Graphics
//! - HPOSP0-3 ($D000-$D003): Horizontal position of players
//! - HPOSM0-3 ($D004-$D007): Horizontal position of missiles
//! - SIZEP0-3 ($D008-$D00B): Player sizes
//! - SIZEM ($D00C): All missile sizes
//! - GRAFP0-3 ($D00D-$D010): Player graphics patterns
//! - GRAFM ($D011): Missile graphics patterns
//!
//! # Priority and Mode
//! - PRIOR ($D01B): Priority selection and GTIA modes
//!   - Bits 0-3: Priority control
//!   - Bits 6-7: GTIA display mode (0=normal, 1=mode 9, 2=mode 10, 3=mode 11)

use serde::{Deserialize, Serialize};

/// GTIA display mode (from PRIOR register bits 6-7)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum GtiaMode {
    /// Normal ANTIC playfield colors
    #[default]
    Normal,
    /// GTIA mode 9: 16 luminances, single hue from COLBK
    Mode9,
    /// GTIA mode 10: 9 colors from color registers
    Mode10,
    /// GTIA mode 11: 16 hues, single luminance from COLBK
    Mode11,
}

/// GTIA chip state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gtia {
    // Player horizontal positions
    hposp: [u8; 4],
    // Missile horizontal positions
    hposm: [u8; 4],
    // Player sizes (0=normal, 1=double, 3=quad)
    sizep: [u8; 4],
    // Missile sizes (2 bits each for M0-M3)
    sizem: u8,
    // Player graphics patterns
    grafp: [u8; 4],
    // Missile graphics pattern (2 bits each for M0-M3)
    grafm: u8,

    // Color registers
    colpm: [u8; 4], // Player/missile colors
    colpf: [u8; 4], // Playfield colors
    colbk: u8,      // Background color

    // Priority/mode register
    prior: u8,

    // Console keys
    consol: u8, // Start/Select/Option keys (active low)

    // Collision registers (active-high, set during rendering)
    m0pf: u8,
    m1pf: u8,
    m2pf: u8,
    m3pf: u8, // Missile-to-playfield
    p0pf: u8,
    p1pf: u8,
    p2pf: u8,
    p3pf: u8, // Player-to-playfield
    m0pl: u8,
    m1pl: u8,
    m2pl: u8,
    m3pl: u8, // Missile-to-player
    p0pl: u8,
    p1pl: u8,
    p2pl: u8,
    p3pl: u8, // Player-to-player

    // Trigger inputs (active low - 0 = pressed)
    trig: [u8; 4],

    // VDELAY register
    vdelay: u8,
    // GRACTL register
    gractl: u8,
}

impl Default for Gtia {
    fn default() -> Self {
        Self::new()
    }
}

impl Gtia {
    pub fn new() -> Self {
        Self {
            hposp: [0; 4],
            hposm: [0; 4],
            sizep: [0; 4],
            sizem: 0,
            grafp: [0; 4],
            grafm: 0,
            colpm: [0; 4],
            colpf: [0; 4],
            colbk: 0,
            prior: 0,
            consol: 0x07, // All console keys released (active low)
            m0pf: 0,
            m1pf: 0,
            m2pf: 0,
            m3pf: 0,
            p0pf: 0,
            p1pf: 0,
            p2pf: 0,
            p3pf: 0,
            m0pl: 0,
            m1pl: 0,
            m2pl: 0,
            m3pl: 0,
            p0pl: 0,
            p1pl: 0,
            p2pl: 0,
            p3pl: 0,
            trig: [1; 4], // All triggers released
            vdelay: 0,
            gractl: 0,
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Get the current GTIA display mode
    pub fn mode(&self) -> GtiaMode {
        match (self.prior >> 6) & 0x03 {
            0 => GtiaMode::Normal,
            1 => GtiaMode::Mode9,
            2 => GtiaMode::Mode10,
            3 => GtiaMode::Mode11,
            _ => unreachable!(),
        }
    }

    /// Read a GTIA register ($D000-$D01F read addresses)
    pub fn read(&self, addr: u16) -> u8 {
        match addr & 0x1F {
            0x00 => self.m0pf,
            0x01 => self.m1pf,
            0x02 => self.m2pf,
            0x03 => self.m3pf,
            0x04 => self.p0pf,
            0x05 => self.p1pf,
            0x06 => self.p2pf,
            0x07 => self.p3pf,
            0x08 => self.m0pl,
            0x09 => self.m1pl,
            0x0A => self.m2pl,
            0x0B => self.m3pl,
            0x0C => self.p0pl,
            0x0D => self.p1pl,
            0x0E => self.p2pl,
            0x0F => self.p3pl,
            0x10 => self.trig[0],
            0x11 => self.trig[1],
            0x12 => self.trig[2],
            0x13 => self.trig[3],
            0x14 => 0x0F, // PAL flag: bit 1-3: always set on 5200 (NTSC)
            0x1F => self.consol & 0x0F,
            _ => 0xFF,
        }
    }

    /// Write a GTIA register ($D000-$D01F write addresses)
    pub fn write(&mut self, addr: u16, val: u8) {
        match addr & 0x1F {
            0x00 => self.hposp[0] = val,
            0x01 => self.hposp[1] = val,
            0x02 => self.hposp[2] = val,
            0x03 => self.hposp[3] = val,
            0x04 => self.hposm[0] = val,
            0x05 => self.hposm[1] = val,
            0x06 => self.hposm[2] = val,
            0x07 => self.hposm[3] = val,
            0x08 => self.sizep[0] = val,
            0x09 => self.sizep[1] = val,
            0x0A => self.sizep[2] = val,
            0x0B => self.sizep[3] = val,
            0x0C => self.sizem = val,
            0x0D => self.grafp[0] = val,
            0x0E => self.grafp[1] = val,
            0x0F => self.grafp[2] = val,
            0x10 => self.grafp[3] = val,
            0x11 => self.grafm = val,
            0x12 => self.colpm[0] = val,
            0x13 => self.colpm[1] = val,
            0x14 => self.colpm[2] = val,
            0x15 => self.colpm[3] = val,
            0x16 => self.colpf[0] = val,
            0x17 => self.colpf[1] = val,
            0x18 => self.colpf[2] = val,
            0x19 => self.colpf[3] = val,
            0x1A => self.colbk = val,
            0x1B => self.prior = val,
            0x1C => self.vdelay = val,
            0x1D => self.gractl = val,
            0x1E => self.clear_collisions(),
            0x1F => {
                /* CONSOL write - start/select/option */
                self.consol = (self.consol & 0x08) | (val & 0x07);
            }
            _ => {}
        }
    }

    /// Clear all collision registers
    pub fn clear_collisions(&mut self) {
        self.m0pf = 0;
        self.m1pf = 0;
        self.m2pf = 0;
        self.m3pf = 0;
        self.p0pf = 0;
        self.p1pf = 0;
        self.p2pf = 0;
        self.p3pf = 0;
        self.m0pl = 0;
        self.m1pl = 0;
        self.m2pl = 0;
        self.m3pl = 0;
        self.p0pl = 0;
        self.p1pl = 0;
        self.p2pl = 0;
        self.p3pl = 0;
    }

    /// Set trigger button state (0=pressed, 1=released)
    pub fn set_trigger(&mut self, index: usize, pressed: bool) {
        if index < 4 {
            self.trig[index] = if pressed { 0 } else { 1 };
        }
    }

    /// Set console button state
    pub fn set_console_keys(&mut self, start: bool, select: bool, option: bool) {
        self.consol = 0x08 // Speaker bit always set
            | if start { 0 } else { 0x01 }   // Start (active low)
            | if select { 0 } else { 0x02 }  // Select (active low)
            | if option { 0 } else { 0x04 }; // Option (active low)
    }

    /// Convert Atari color register value to RGB
    /// The Atari 5200 uses NTSC color encoding:
    /// - High nibble: hue (0-15)
    /// - Low nibble: luminance (0-15, even values only matter)
    pub fn color_to_rgb(color: u8) -> u32 {
        let hue = (color >> 4) & 0x0F;
        let lum = color & 0x0F;

        // NTSC palette lookup - 16 hues x 16 luminances
        // Based on the standard Atari NTSC palette
        let (r, g, b) = match hue {
            0x0 => Self::gray_shade(lum),                  // Gray
            0x1 => Self::tinted_shade(lum, 255, 200, 100), // Gold/Orange
            0x2 => Self::tinted_shade(lum, 255, 150, 80),  // Orange
            0x3 => Self::tinted_shade(lum, 255, 100, 80),  // Red-Orange
            0x4 => Self::tinted_shade(lum, 255, 80, 100),  // Pink
            0x5 => Self::tinted_shade(lum, 220, 80, 180),  // Purple
            0x6 => Self::tinted_shade(lum, 170, 80, 255),  // Purple-Blue
            0x7 => Self::tinted_shade(lum, 100, 100, 255), // Blue
            0x8 => Self::tinted_shade(lum, 80, 140, 255),  // Blue
            0x9 => Self::tinted_shade(lum, 60, 180, 255),  // Light Blue
            0xA => Self::tinted_shade(lum, 60, 200, 200),  // Cyan
            0xB => Self::tinted_shade(lum, 60, 220, 140),  // Blue-Green
            0xC => Self::tinted_shade(lum, 60, 220, 80),   // Green
            0xD => Self::tinted_shade(lum, 140, 220, 60),  // Yellow-Green
            0xE => Self::tinted_shade(lum, 200, 200, 60),  // Yellow
            0xF => Self::tinted_shade(lum, 240, 200, 80),  // Yellow-Orange
            _ => unreachable!(),
        };

        0xFF000000 | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
    }

    /// Generate a gray shade from luminance
    fn gray_shade(lum: u8) -> (u8, u8, u8) {
        let l = (lum as u16 * 17).min(255) as u8;
        (l, l, l)
    }

    /// Generate a tinted shade by mixing hue color with luminance
    fn tinted_shade(lum: u8, r: u8, g: u8, b: u8) -> (u8, u8, u8) {
        let l = lum as u16 * 17; // Scale 0-15 to 0-255
        let blend = |c: u8| -> u8 {
            let result = (c as u16 * l) / 255;
            result.min(255) as u8
        };
        (blend(r), blend(g), blend(b))
    }

    // Accessors for rendering
    pub fn colbk(&self) -> u8 {
        self.colbk
    }
    pub fn colpf(&self, idx: usize) -> u8 {
        self.colpf[idx.min(3)]
    }
    pub fn colpm(&self, idx: usize) -> u8 {
        self.colpm[idx.min(3)]
    }
    pub fn hposp(&self, idx: usize) -> u8 {
        self.hposp[idx.min(3)]
    }
    pub fn hposm(&self, idx: usize) -> u8 {
        self.hposm[idx.min(3)]
    }
    pub fn sizep(&self, idx: usize) -> u8 {
        self.sizep[idx.min(3)]
    }
    pub fn grafp(&self, idx: usize) -> u8 {
        self.grafp[idx.min(3)]
    }
    pub fn grafm(&self) -> u8 {
        self.grafm
    }
    pub fn sizem(&self) -> u8 {
        self.sizem
    }
    pub fn prior(&self) -> u8 {
        self.prior
    }
    pub fn gractl(&self) -> u8 {
        self.gractl
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_registers() {
        let mut gtia = Gtia::new();
        gtia.write(0xD016, 0x28); // COLPF0
        gtia.write(0xD01A, 0x94); // COLBK
        assert_eq!(gtia.colpf(0), 0x28);
        assert_eq!(gtia.colbk(), 0x94);
    }

    #[test]
    fn test_player_positions() {
        let mut gtia = Gtia::new();
        gtia.write(0xD000, 100); // HPOSP0
        gtia.write(0xD001, 150); // HPOSP1
        assert_eq!(gtia.hposp(0), 100);
        assert_eq!(gtia.hposp(1), 150);
    }

    #[test]
    fn test_collision_clear() {
        let mut gtia = Gtia::new();
        gtia.m0pf = 0xFF;
        gtia.p0pl = 0xFF;
        gtia.clear_collisions();
        assert_eq!(gtia.read(0x00), 0); // M0PF
        assert_eq!(gtia.read(0x0C), 0); // P0PL
    }

    #[test]
    fn test_trigger_buttons() {
        let mut gtia = Gtia::new();
        assert_eq!(gtia.read(0x10), 1); // TRIG0 released
        gtia.set_trigger(0, true);
        assert_eq!(gtia.read(0x10), 0); // TRIG0 pressed
    }

    #[test]
    fn test_gtia_modes() {
        let mut gtia = Gtia::new();
        assert_eq!(gtia.mode(), GtiaMode::Normal);
        gtia.write(0xD01B, 0x40); // PRIOR = mode 9
        assert_eq!(gtia.mode(), GtiaMode::Mode9);
        gtia.write(0xD01B, 0x80); // PRIOR = mode 10
        assert_eq!(gtia.mode(), GtiaMode::Mode10);
        gtia.write(0xD01B, 0xC0); // PRIOR = mode 11
        assert_eq!(gtia.mode(), GtiaMode::Mode11);
    }

    #[test]
    fn test_color_to_rgb() {
        // Black (hue 0, lum 0)
        let rgb = Gtia::color_to_rgb(0x00);
        assert_eq!(rgb & 0x00FFFFFF, 0x000000);

        // White (hue 0, lum 15)
        let rgb = Gtia::color_to_rgb(0x0F);
        assert_eq!(rgb & 0x00FFFFFF, 0xFFFFFF);

        // Non-zero hue should produce colored output
        let rgb = Gtia::color_to_rgb(0x78);
        let r = (rgb >> 16) & 0xFF;
        let b = rgb & 0xFF;
        // Blue hue should have more blue than red
        assert!(b > r);
    }
}
