//! egui-based user interface for the emulator
//!
//! This module implements a modern, modular UI layout with:
//! - Menu bar at the top
//! - Main content area with tabs (Emulator, NewProject, Help, About)
//! - Inspector dock (bottom, moveable) with system-specific tabs
//! - Property pane dock (right, moveable)
//! - Status bar at the bottom

mod dock_layout;
mod inspector_tabs;
mod layout;
pub mod menu_bar;
pub mod property_pane;
mod status_bar;
mod tabs;

pub use layout::EguiApp;
pub use menu_bar::MenuAction;
pub use property_pane::{InputConfigSource, PropertyAction};
pub use tabs::{
    Atari2600TileData, CartridgeData, Chip8TileData, ColecoVisionTileData, DebugAction, GbTileData,
    GbaTileData, MountInfo, N64RdpData, N64ViData, NesTileData, PcBdaData, Ps1GpuData,
    Sg1000TileData, SmsTileData, SnesTileData, SystemTileData, Tab, TabAction,
};
