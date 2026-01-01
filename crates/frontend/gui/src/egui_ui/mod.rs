//! egui-based user interface for the emulator
//!
//! This module implements a modern, modular UI layout with:
//! - Menu bar at the top
//! - Two-column layout:
//!   - Left: Tabbed interface (Emulator, Log, Help, Debug)
//!   - Right: Property pane (Metrics, Settings, Mounts, Save States)
//! - Status bar at the bottom

mod layout;
mod tabs;
mod property_pane;
mod menu_bar;
mod status_bar;

pub use layout::EguiApp;
