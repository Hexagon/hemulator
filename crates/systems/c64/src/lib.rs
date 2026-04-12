//! Commodore 64 emulator implementation
//!
//! # Architecture
//!
//! - **CPU**: MOS 6510 (6502 with built-in I/O port) @ 0.985 MHz (PAL) / 1.023 MHz (NTSC)
//! - **VIC-II**: MOS 6569 (PAL) / 6567 (NTSC) — 320×200 resolution, 16 colors
//! - **SID**: MOS 6581 / 8580 — 3 voices, ADSR envelopes, multimode filter
//! - **CIA 1**: MOS 6526 — Keyboard matrix, joystick port 2, timers, IRQ
//! - **CIA 2**: MOS 6526 — VIC bank select, serial bus, joystick port 1, NMI
//! - **RAM**: 64KB main + 1KB color RAM
//! - **ROM**: 8KB KERNAL + 8KB BASIC + 4KB character generator

#![allow(dead_code)]
#![allow(clippy::upper_case_acronyms)]

mod bus;
mod cia;
mod debugger;
mod sid;
mod system;
mod vic;

pub use system::{C64Error, C64System, DebugInfo};
