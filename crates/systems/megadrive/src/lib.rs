//! Sega Mega Drive / Genesis emulator implementation
//!
//! # Architecture
//!
//! - **Main CPU**: Motorola 68000 @ 7.67 MHz (NTSC) / 7.60 MHz (PAL)
//! - **Sound CPU**: Zilog Z80 @ 3.58 MHz (NTSC) / 3.55 MHz (PAL)
//! - **VDP**: Yamaha YM7101 (315-5313) — 64KB VRAM, 128B CRAM, 80B VSRAM
//! - **FM Synth**: Yamaha YM2612 (6 FM channels)
//! - **PSG**: Texas Instruments SN76489 (4 channels, integrated in VDP)
//! - **RAM**: 64KB main (68K), 8KB sound (Z80)
//!
//! # References
//!
//! - Sega Mega Drive / Genesis Technical Reference (Charles MacDonald)
//! - 68000 Programmer's Reference Manual (Motorola M68000PM/AD)
//! - YM2612 Application Manual (Yamaha)

#![allow(dead_code)]

mod bus;
mod m68k;
mod psg;
mod system;
mod vdp;
mod ym2612;

pub use system::MegaDriveSystem;
