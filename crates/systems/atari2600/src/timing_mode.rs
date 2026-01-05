//! Timing mode configuration for Atari 2600 emulation
//!
//! The Atari 2600 emulator supports two timing modes:
//!
//! ## Cycle-Accurate Mode (Default)
//! - Renders each pixel as it's generated (228 color clocks per scanline)
//! - Mid-scanline register updates affect remaining pixels on that line
//! - HMOVE effects happen at exact color clock boundaries
//! - "Racing the beam" techniques work exactly as on hardware
//! - More accurate but slightly slower (still runs at full speed on modern CPUs)
//!
//! ## Frame-Based Mode (Legacy)
//! - State is latched once per scanline
//! - Rendering happens after the scanline is complete
//! - Fast and efficient, works for 95%+ of games
//! - May not handle rapid mid-scanline updates perfectly
//!
//! The default is cycle-accurate mode for maximum compatibility.

use serde::{Deserialize, Serialize};

/// Timing mode for TIA emulation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimingMode {
    /// Cycle-accurate: Render each pixel as it's generated
    /// Best compatibility, handles mid-scanline register changes
    CycleAccurate,
    
    /// Frame-based: Latch state once per scanline
    /// Faster, works for most games but may miss some effects
    FrameBased,
}

impl Default for TimingMode {
    fn default() -> Self {
        TimingMode::CycleAccurate
    }
}

impl TimingMode {
    /// Get the name of the timing mode
    pub fn name(&self) -> &str {
        match self {
            TimingMode::CycleAccurate => "Cycle-Accurate",
            TimingMode::FrameBased => "Frame-Based",
        }
    }
    
    /// Parse timing mode from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "cycle_accurate" | "cycle-accurate" | "cycleaccurate" => Some(TimingMode::CycleAccurate),
            "frame_based" | "frame-based" | "framebased" => Some(TimingMode::FrameBased),
            _ => None,
        }
    }
    
    /// Convert timing mode to string
    pub fn to_string(&self) -> String {
        match self {
            TimingMode::CycleAccurate => "cycle_accurate".to_string(),
            TimingMode::FrameBased => "frame_based".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_default_is_cycle_accurate() {
        assert_eq!(TimingMode::default(), TimingMode::CycleAccurate);
    }
    
    #[test]
    fn test_timing_mode_names() {
        assert_eq!(TimingMode::CycleAccurate.name(), "Cycle-Accurate");
        assert_eq!(TimingMode::FrameBased.name(), "Frame-Based");
    }
    
    #[test]
    fn test_from_str() {
        assert_eq!(TimingMode::from_str("cycle_accurate"), Some(TimingMode::CycleAccurate));
        assert_eq!(TimingMode::from_str("cycle-accurate"), Some(TimingMode::CycleAccurate));
        assert_eq!(TimingMode::from_str("cycleaccurate"), Some(TimingMode::CycleAccurate));
        assert_eq!(TimingMode::from_str("frame_based"), Some(TimingMode::FrameBased));
        assert_eq!(TimingMode::from_str("frame-based"), Some(TimingMode::FrameBased));
        assert_eq!(TimingMode::from_str("framebased"), Some(TimingMode::FrameBased));
        assert_eq!(TimingMode::from_str("invalid"), None);
    }
    
    #[test]
    fn test_to_string() {
        assert_eq!(TimingMode::CycleAccurate.to_string(), "cycle_accurate");
        assert_eq!(TimingMode::FrameBased.to_string(), "frame_based");
    }
    
    #[test]
    fn test_roundtrip() {
        let modes = vec![TimingMode::CycleAccurate, TimingMode::FrameBased];
        for mode in modes {
            let s = mode.to_string();
            let parsed = TimingMode::from_str(&s).unwrap();
            assert_eq!(parsed, mode);
        }
    }
}
