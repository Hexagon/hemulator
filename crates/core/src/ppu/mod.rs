//! Reusable PPU (Picture Processing Unit) components for tile-based video systems.
//!
//! This module provides common building blocks for implementing video/graphics
//! processors in various retro systems (NES, Game Boy, SNES, Genesis, etc.).
//!
//! Each system will have its own PPU implementation that uses these components
//! as appropriate for that system's specific architecture.

pub mod palette;
pub mod tile;

pub use palette::IndexedPalette;
pub use tile::{TileDecoder, TileFormat};
