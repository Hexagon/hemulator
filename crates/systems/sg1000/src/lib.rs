//! SG-1000 emulator implementation
//!
//! This crate implements emulation of the Sega SG-1000 home console.
//!
//! # Architecture
//!
//! - **CPU**: Zilog Z80A @ 3.58 MHz
//! - **VDP**: Texas Instruments TMS9918A
//! - **PSG**: Texas Instruments SN76489
//! - **RAM**: 1 KB main RAM, 16 KB video RAM
//!
//! The SG-1000 shares very similar hardware with the ColecoVision,
//! using the same core components (Z80, TMS9918A VDP, SN76489 PSG).

mod bus;
mod debugger;
mod psg;
mod system;

pub use system::Sg1000System;
