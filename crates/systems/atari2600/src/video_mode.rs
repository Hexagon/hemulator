//! Video mode (NTSC/PAL) configuration for Atari 2600
//!
//! The Atari 2600 was released in different video standards for different regions:
//! - **NTSC**: North America, Japan - 262 scanlines, 60 Hz
//! - **PAL**: Europe, Australia - 312 scanlines, 50 Hz
//!
//! This module provides the `VideoMode` enum to configure the emulator for the appropriate
//! video standard, affecting timing, scanline count, and color palette.

use serde::{Deserialize, Serialize};

/// Video standard (NTSC or PAL) for Atari 2600 emulation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum VideoMode {
    /// NTSC (North America, Japan): 262 scanlines, 60 Hz, 128 colors
    #[default]
    NTSC,
    /// PAL (Europe, Australia): 312 scanlines, 50 Hz, 104 colors
    PAL,
}

impl VideoMode {
    /// Number of scanlines per frame
    ///
    /// - NTSC: 262 scanlines
    /// - PAL: 312 scanlines
    pub fn scanlines_per_frame(self) -> u16 {
        match self {
            VideoMode::NTSC => 262,
            VideoMode::PAL => 312,
        }
    }

    /// Frame rate in Hz
    ///
    /// - NTSC: 60 Hz (59.94 Hz technically)
    /// - PAL: 50 Hz
    pub fn frame_rate(self) -> f64 {
        match self {
            VideoMode::NTSC => 60.0,
            VideoMode::PAL => 50.0,
        }
    }

    /// Color clock frequency in Hz
    ///
    /// - NTSC: 3.579545 MHz
    /// - PAL: 3.546894 MHz
    pub fn color_clock_hz(self) -> f64 {
        match self {
            VideoMode::NTSC => 3_579_545.0,
            VideoMode::PAL => 3_546_894.0,
        }
    }

    /// Number of visible scanlines in typical games
    ///
    /// - NTSC: ~192 scanlines visible
    /// - PAL: ~228 scanlines visible (more vertical space)
    pub fn visible_scanlines(self) -> u16 {
        match self {
            VideoMode::NTSC => 192,
            VideoMode::PAL => 228,
        }
    }

    /// Number of unique colors in the palette
    ///
    /// - NTSC: 128 colors
    /// - PAL: 104 colors (some values are duplicates/black)
    pub fn palette_colors(self) -> usize {
        match self {
            VideoMode::NTSC => 128,
            VideoMode::PAL => 104,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ntsc_values() {
        assert_eq!(VideoMode::NTSC.scanlines_per_frame(), 262);
        assert_eq!(VideoMode::NTSC.frame_rate(), 60.0);
        assert_eq!(VideoMode::NTSC.visible_scanlines(), 192);
        assert_eq!(VideoMode::NTSC.palette_colors(), 128);
    }

    #[test]
    fn test_pal_values() {
        assert_eq!(VideoMode::PAL.scanlines_per_frame(), 312);
        assert_eq!(VideoMode::PAL.frame_rate(), 50.0);
        assert_eq!(VideoMode::PAL.visible_scanlines(), 228);
        assert_eq!(VideoMode::PAL.palette_colors(), 104);
    }

    #[test]
    fn test_default_is_ntsc() {
        assert_eq!(VideoMode::default(), VideoMode::NTSC);
    }
}
