//! Core APU (Audio Processing Unit) components.
//!
//! This module provides reusable audio synthesis components used in various retro gaming
//! systems. The components are designed to be generic and reusable across different
//! audio architectures.
//!
//! ## Core Components
//!
//! - **Pulse Channel**: Square wave generator with duty cycle control (NES, Game Boy)
//! - **Triangle Channel**: Triangle wave generator (NES)
//! - **Wave Channel**: Programmable waveform playback (Game Boy, potentially other systems)
//! - **Noise Channel**: Pseudo-random noise generator using LFSR (NES, Game Boy)
//! - **DMC Channel**: Delta modulation channel for sample playback (NES)
//! - **Polynomial Counter**: TIA-style waveform generation (Atari 2600)
//! - **Length Counter**: Automatic note duration control
//! - **Envelope**: Volume envelope generator with decay
//! - **Sweep Unit**: Frequency sweep for pitch modulation (Game Boy)
//! - **Frame Counter**: Timing controller for envelope and length counter units
//!
//! ## Audio Chips
//!
//! - **RP2A03**: NES NTSC audio chip
//! - **RP2A07**: NES PAL audio chip
//! - **AudioChip trait**: Common interface for pluggable audio chips
//!
//! ## Timing Support
//!
//! All components support both NTSC and PAL timing modes for accurate emulation
//! of regional console variants.
//!
//! ## Reusability
//!
//! These components are designed for use in:
//! - **NES (Famicom)**: Uses pulse, triangle, noise, DMC, envelope, length counter
//! - **Game Boy**: Uses pulse (with sweep), wave, noise, envelope, length counter
//! - **Atari 2600 (TIA)**: Uses polynomial counter for waveform generation
//! - **Future systems**: C64 (SID), ColecoVision (SN76489), Atari 8-bit (POKEY)
//! - Custom audio synthesizers using similar waveform generation

pub mod audio_chip;
pub mod dmc;
pub mod envelope;
pub mod frame_counter;
pub mod length_counter;
pub mod noise;
pub mod polynomial;
pub mod pulse;
pub mod rp2a03;
pub mod rp2a07;
pub mod sweep;
pub mod timing;
pub mod triangle;
pub mod wave;

pub use audio_chip::AudioChip;
pub use dmc::DmcChannel;
pub use envelope::Envelope;
pub use frame_counter::FrameCounter;
pub use length_counter::{LengthCounter, LENGTH_TABLE};
pub use noise::NoiseChannel;
pub use polynomial::PolynomialCounter;
pub use pulse::PulseChannel;
pub use rp2a03::Rp2a03Apu;
pub use rp2a07::Rp2a07Apu;
pub use sweep::SweepUnit;
pub use timing::TimingMode;
pub use triangle::TriangleChannel;
pub use wave::WaveChannel;
