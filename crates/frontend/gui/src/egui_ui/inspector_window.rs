//! Inspector window for logging and debugging features
//!
//! This module implements a separate native window (the Inspector) for debug tools including:
//! - Log viewer with category filtering
//! - CPU state and debugger controls
//! - Memory viewer
//! - Tile/Pattern viewer
//! - Palette viewer

use crate::system_adapter::EnhancedDebugState;
use egui::{Context, ScrollArea, Ui, ViewportBuilder, ViewportId};

use super::tabs::SystemTileData;

/// Inspector window state and configuration
pub struct InspectorWindow {
    /// Whether the inspector window is open
    pub is_open: bool,
    
    /// The viewport ID for the inspector window
    viewport_id: ViewportId,
    
    /// Log messages
    log_messages: Vec<String>,
    
    /// Enhanced debug state (CPU, memory, disassembly)
    enhanced_debug_state: Option<EnhancedDebugState>,
    
    /// Tile viewer data
    system_tile_data: Option<SystemTileData>,
}

/// Inspector window tabs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InspectorTab {
    Log,
    Debugger,
    Memory,
    Tiles,
}

impl InspectorWindow {
    pub fn new() -> Self {
        Self {
            is_open: false,
            viewport_id: ViewportId::from_hash_of("inspector_window"),
            log_messages: Vec::new(),
            enhanced_debug_state: None,
            system_tile_data: None,
        }
    }
    
    /// Toggle inspector window open/closed
    pub fn toggle(&mut self) {
        self.is_open = !self.is_open;
    }
    
    /// Set inspector window open state
    pub fn set_open(&mut self, open: bool) {
        self.is_open = open;
    }
    
    /// Add a log message
    pub fn add_log(&mut self, message: String) {
        self.log_messages.push(message);
        // Keep only last 1000 messages
        if self.log_messages.len() > 1000 {
            self.log_messages.remove(0);
        }
    }
    
    /// Update enhanced debug state
    pub fn update_enhanced_debug_state(&mut self, state: EnhancedDebugState) {
        self.enhanced_debug_state = Some(state);
    }
    
    /// Update tile viewer data
    pub fn update_tile_data(&mut self, data: SystemTileData) {
        self.system_tile_data = Some(data);
    }
    
    /// Show the inspector window (call this from main UI update)
    pub fn show(&mut self, ctx: &Context) {
        if !self.is_open {
            return;
        }
        
        // Clone necessary data for the closure
        let log_messages = self.log_messages.clone();
        let enhanced_debug_state = self.enhanced_debug_state.clone();
        let system_tile_data = self.system_tile_data.clone();
        let viewport_id = self.viewport_id;
        
        // Create a separate viewport for the inspector window
        ctx.show_viewport_deferred(
            viewport_id,
            ViewportBuilder::default()
                .with_title("Hemulator Inspector")
                .with_inner_size([900.0, 700.0])
                .with_resizable(true),
            move |ctx, _class| {
                // Render the inspector window content
                egui::CentralPanel::default().show(ctx, |ui| {
                    // Use persistent UI state for active tab
                    let mut active_tab = ctx.data_mut(|d| {
                        d.get_persisted::<InspectorTab>(egui::Id::new("inspector_active_tab"))
                            .unwrap_or(InspectorTab::Log)
                    });
                    
                    // Tab bar
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut active_tab, InspectorTab::Log, "📋 Log");
                        ui.selectable_value(&mut active_tab, InspectorTab::Debugger, "🔧 Debugger");
                        ui.selectable_value(&mut active_tab, InspectorTab::Memory, "💾 Memory");
                        ui.selectable_value(&mut active_tab, InspectorTab::Tiles, "🎨 Tiles");
                    });
                    
                    ui.separator();
                    
                    // Tab content
                    match active_tab {
                        InspectorTab::Log => render_log_tab(ui, &log_messages),
                        InspectorTab::Debugger => render_debugger_tab(ui, &enhanced_debug_state),
                        InspectorTab::Memory => render_memory_tab(ui),
                        InspectorTab::Tiles => render_tiles_tab(ui, &system_tile_data),
                    }
                    
                    // Persist active tab
                    ctx.data_mut(|d| {
                        d.insert_persisted(egui::Id::new("inspector_active_tab"), active_tab);
                    });
                });
            },
        );
        
        // Handle window close after rendering
        if ctx.input(|i| i.raw.viewports.get(&self.viewport_id).map_or(false, |v| v.close_requested())) {
            self.is_open = false;
        }
    }
}

/// Render log tab
fn render_log_tab(ui: &mut Ui, log_messages: &[String]) {
    ScrollArea::vertical()
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            ui.heading("Log Messages");
            ui.add_space(10.0);
            
            // Get log configuration
            let log_config = emu_core::logging::LogConfig::global();
            
            // Global log level control
            ui.horizontal(|ui| {
                ui.label("Global Level:");
                let mut global_level = log_config.get_global_level();
                egui::ComboBox::from_id_salt("global_log_level")
                    .selected_text(format!("{:?}", global_level))
                    .show_ui(ui, |ui| {
                        for level in [
                            emu_core::logging::LogLevel::Off,
                            emu_core::logging::LogLevel::Error,
                            emu_core::logging::LogLevel::Warn,
                            emu_core::logging::LogLevel::Info,
                            emu_core::logging::LogLevel::Debug,
                            emu_core::logging::LogLevel::Trace,
                        ] {
                            if ui
                                .selectable_value(&mut global_level, level, format!("{:?}", level))
                                .clicked()
                            {
                                log_config.set_global_level(level);
                            }
                        }
                    });
            });
            
            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);
            
            // Category-specific log levels
            ui.heading("Category Levels");
            ui.add_space(5.0);
            
            for (category, name) in [
                (emu_core::logging::LogCategory::CPU, "CPU"),
                (emu_core::logging::LogCategory::Bus, "Bus"),
                (emu_core::logging::LogCategory::PPU, "PPU"),
                (emu_core::logging::LogCategory::APU, "APU"),
                (emu_core::logging::LogCategory::Interrupts, "Interrupts"),
                (emu_core::logging::LogCategory::Stubs, "Stubs"),
            ] {
                ui.horizontal(|ui| {
                    ui.label(format!("{}:", name));
                    let mut category_level = log_config.get_level(category);
                    egui::ComboBox::from_id_salt(format!("log_level_{:?}", category))
                        .selected_text(format!("{:?}", category_level))
                        .show_ui(ui, |ui| {
                            for level in [
                                emu_core::logging::LogLevel::Off,
                                emu_core::logging::LogLevel::Error,
                                emu_core::logging::LogLevel::Warn,
                                emu_core::logging::LogLevel::Info,
                                emu_core::logging::LogLevel::Debug,
                                emu_core::logging::LogLevel::Trace,
                            ] {
                                if ui
                                    .selectable_value(&mut category_level, level, format!("{:?}", level))
                                    .clicked()
                                {
                                    log_config.set_level(category, level);
                                }
                            }
                        });
                });
            }
            
            ui.add_space(15.0);
            ui.separator();
            ui.add_space(10.0);
            
            // Legacy log messages section
            if !log_messages.is_empty() {
                ui.heading("Application Messages");
                ui.add_space(5.0);
                
                for msg in log_messages {
                    ui.label(msg);
                }
            }
        });
}

/// Render debugger tab (CPU state, disassembly, controls)
fn render_debugger_tab(ui: &mut Ui, enhanced_debug_state: &Option<EnhancedDebugState>) {
    if let Some(ref state) = enhanced_debug_state {
        // Header
        ui.heading(format!("🔧 {} Debugger", state.system_type));
        ui.separator();
        
        // TODO: Add debugger controls (pause, resume, step)
        // TODO: Add disassembly view
        // TODO: Add CPU state view
        
        ui.label("Debugger view (implementation in progress)");
    } else {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.label(egui::RichText::new("🔧").size(48.0));
            ui.add_space(10.0);
            ui.heading("No Debug Information Available");
            ui.add_space(10.0);
            ui.label("Load a ROM to see system-specific debug information");
        });
    }
}

/// Render memory tab
fn render_memory_tab(ui: &mut Ui) {
    ui.heading("💾 Memory Viewer");
    ui.separator();
    
    // TODO: Add memory viewer implementation
    ui.label("Memory viewer (implementation in progress)");
}

/// Render tiles tab
fn render_tiles_tab(ui: &mut Ui, system_tile_data: &Option<SystemTileData>) {
    ui.heading("🎨 Tile/Pattern Viewer");
    ui.separator();
    
    if system_tile_data.is_some() {
        // TODO: Add tile viewer implementation
        ui.label("Tile viewer (implementation in progress)");
    } else {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.label(egui::RichText::new("🎨").size(48.0));
            ui.add_space(10.0);
            ui.heading("No Tile Data Available");
            ui.add_space(10.0);
            ui.label("Load a ROM with graphics to view tiles and palettes");
        });
    }
}

impl Default for InspectorWindow {
    fn default() -> Self {
        Self::new()
    }
}
