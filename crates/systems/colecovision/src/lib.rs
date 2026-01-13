//! ColecoVision emulator implementation
//!
//! This crate implements emulation of the ColecoVision home console.
//!
//! # Architecture
//!
//! - **CPU**: Zilog Z80A @ 3.58 MHz
//! - **VDP**: Texas Instruments TMS9918A
//! - **PSG**: Texas Instruments SN76489
//! - **RAM**: 1 KB main RAM, 16 KB video RAM
//! - **BIOS**: 8 KB system ROM
//!
//! For detailed implementation information, see the README.md

mod bus;
mod debugger;
mod psg;
mod system;
mod vdp;

pub use system::ColecoVisionSystem;
