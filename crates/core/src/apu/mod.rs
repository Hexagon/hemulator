//! Core APU (Audio Processing Unit) components.
//!
//! This module provides reusable audio synthesis components used in various retro gaming
//! systems. The components are designed around the RP2A03 (NES NTSC) and RP2A07 (NES PAL)
//! chips but can be used in other systems with similar audio architectures.
//!
//! ## Components
//!
//! - **Pulse Channel**: Square wave generator with duty cycle control
//! - **Triangle Channel**: Triangle wave generator
//! - **Noise Channel**: Pseudo-random noise generator using LFSR
//! - **Length Counter**: Automatic note duration control
//! - **Envelope**: Volume envelope generator with decay
//! - **Frame Counter**: Timing controller for envelope and length counter units
//!
//! ## Timing Support
//!
//! All components support both NTSC and PAL timing modes for accurate emulation
//! of regional console variants.
//!
//! ## Reusability
//!
//! These components can be used in:
//! - NES (Famicom) - uses all components
//! - Other systems with RP2A03-based audio (e.g., Famicom clones)
//! - Custom audio synthesizers using similar waveform generation

pub mod envelope;
pub mod frame_counter;
pub mod length_counter;
pub mod noise;
pub mod pulse;
pub mod timing;
pub mod triangle;

pub use envelope::Envelope;
pub use frame_counter::FrameCounter;
pub use length_counter::{LengthCounter, LENGTH_TABLE};
pub use noise::NoiseChannel;
pub use pulse::PulseChannel;
pub use timing::TimingMode;
pub use triangle::TriangleChannel;
