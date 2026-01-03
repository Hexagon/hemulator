//! Sega Master System emulator implementation
//!
//! This crate implements emulation of the Sega Master System and Game Gear.
//!
//! # Architecture
//!
//! - **CPU**: Zilog Z80A @ 3.58 MHz (NTSC) / 3.55 MHz (PAL)
//! - **VDP**: Sega 315-5124 (SMS 1) / 315-5246 (SMS 2)
//! - **PSG**: Texas Instruments SN76489 (Sega variant SN76496)
//! - **RAM**: 8 KB main RAM
//! - **VRAM**: 16 KB video RAM
//!
//! For detailed implementation information, see the SMS_IMPLEMENTATION_GUIDE.md

mod bus;
mod system;
mod vdp;

pub use system::SmsSystem;
