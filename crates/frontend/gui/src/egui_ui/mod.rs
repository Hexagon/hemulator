//! egui-based user interface for the emulator
//!
//! This module implements a modern, modular UI layout with:
//! - Menu bar at the top
//! - Two-column layout:
//!   - Left: Tabbed interface (Emulator, Log, Help, Debug)
//!   - Right: Property pane (Metrics, Settings, Mounts, Save States)
//! - Status bar at the bottom

mod layout;
pub mod menu_bar;
pub mod property_pane;
mod status_bar;
mod tabs;

pub use layout::EguiApp;
pub use tabs::{Tab, TabAction};
