//! APU timing configuration for different console regions.

/// Console region timing configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TimingMode {
    /// NTSC (North America, Japan) - 1.789773 MHz CPU clock
    #[default]
    Ntsc,
    /// PAL (Europe, Australia) - 1.662607 MHz CPU clock
    Pal,
}

impl TimingMode {
    /// Get the CPU clock frequency in Hz for this timing mode
    pub fn cpu_clock_hz(&self) -> f64 {
        match self {
            TimingMode::Ntsc => 1_789_773.0,
            TimingMode::Pal => 1_662_607.0,
        }
    }

    /// Get the frame rate in Hz for this timing mode
    pub fn frame_rate_hz(&self) -> f64 {
        match self {
            TimingMode::Ntsc => 60.0988,
            TimingMode::Pal => 50.0070,
        }
    }

    /// Get the frame counter frequency in Hz (240Hz NTSC, 200Hz PAL)
    pub fn frame_counter_hz(&self) -> f64 {
        match self {
            TimingMode::Ntsc => 240.0,
            TimingMode::Pal => 200.0,
        }
    }
}
