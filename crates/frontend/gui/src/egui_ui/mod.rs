//! egui-based user interface for the emulator
//!
//! This module implements a modern, modular UI layout with:
//! - Menu bar at the top
//! - Two-column layout:
//!   - Left: Tabbed interface (Emulator, Help)
//!   - Right: Property pane (Metrics, Settings, Mounts, Save States)
//! - Status bar at the bottom
//! - Separate Inspector window for logging and debugging tools

mod inspector_window;
mod layout;
pub mod menu_bar;
pub mod property_pane;
mod status_bar;
mod tabs;

pub use inspector_window::InspectorWindow;
pub use layout::EguiApp;
pub use property_pane::{InputConfigSource, PropertyAction};
pub use tabs::{
    DebugAction, GbTileData, NesTileData, PcConfigInfo, SmsTileData, SnesTileData, SystemTileData,
    Tab, TabAction, TileViewerData,
};
