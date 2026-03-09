//! Shared types for hemu-* chip crates.
//!
//! This crate provides zero-dependency types shared between chip implementations
//! and the host application's debugger infrastructure.

/// A single disassembled machine instruction.
///
/// Returned by every chip crate's disassembler and consumed by the host
/// system's [`Debugger`] trait implementation.
///
/// [`Debugger`]: https://docs.rs/emu_core/latest/emu_core/debug/trait.Debugger.html
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisassembledInstruction {
    /// Program counter / address of instruction
    pub address: u32,
    /// Raw bytes of the instruction
    pub bytes: Vec<u8>,
    /// Disassembled mnemonic (e.g., "LDA #$10", "MOV AX, BX")
    pub mnemonic: String,
    /// Optional comment or annotation
    pub comment: Option<String>,
}

impl DisassembledInstruction {
    /// Create a new disassembled instruction
    pub fn new(address: u32, bytes: Vec<u8>, mnemonic: impl Into<String>) -> Self {
        Self {
            address,
            bytes,
            mnemonic: mnemonic.into(),
            comment: None,
        }
    }

    /// Add a comment to the instruction
    pub fn with_comment(mut self, comment: impl Into<String>) -> Self {
        self.comment = Some(comment.into());
        self
    }

    /// Get the length of this instruction in bytes
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Check if this instruction has zero length (should never happen)
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}

/// Console region timing configuration
///
/// Used by audio chip implementations to adapt sample rates and frequencies
/// to NTSC/PAL hardware differences.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TimingMode {
    /// NTSC (North America, Japan)
    #[default]
    Ntsc,
    /// PAL (Europe, Australia)
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

/// A trait for audio chips/APUs from various retro gaming systems.
///
/// Implementations exist for RP2A03 (NES NTSC), RP2A07 (NES PAL),
/// SN76489 (SMS, ColecoVision, SG-1000), SPC700 (SNES), and others.
pub trait AudioChip {
    /// Write to a register on the audio chip
    fn write_register(&mut self, addr: u16, val: u8);

    /// Read from a register on the audio chip (if supported)
    fn read_register(&self, addr: u16) -> u8 {
        let _ = addr;
        0
    }

    /// Clock the chip for one CPU cycle, returning an audio sample
    fn clock(&mut self) -> i16;

    /// Get the timing mode of this chip (NTSC/PAL)
    fn timing(&self) -> TimingMode;

    /// Generate multiple samples efficiently
    fn generate_samples(&mut self, count: usize) -> Vec<i16> {
        let mut samples = Vec::with_capacity(count);
        for _ in 0..count {
            samples.push(self.clock());
        }
        samples
    }

    /// Reset the chip to power-on state
    fn reset(&mut self);

    /// Get the native sample rate of this chip (in Hz)
    fn sample_rate(&self) -> f64 {
        self.timing().cpu_clock_hz()
    }
}
