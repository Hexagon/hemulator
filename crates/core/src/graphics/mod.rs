//! Reusable graphics utilities for emulator systems
//!
//! This module provides common graphics operations that can be shared across
//! different system implementations, promoting code reuse and reducing duplication.

pub mod color;
pub mod zbuffer;

pub use color::ColorOps;
pub use zbuffer::ZBuffer;
