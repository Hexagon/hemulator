//! System-specific Inspector tabs

use super::tabs::TabManager;
use crate::rom_detect::SystemType;
use egui::Ui;

/// Inspector tab types - can be generic or system-specific
#[derive(Debug, Clone, PartialEq)]
pub enum InspectorTab {
    // Generic tabs (available for all systems)
    Log,
    Memory,
    Debug, // Generic debugger with CPU state, memory, disassembly

    // System-specific tabs
    NesTiles,
    NesPalettes,
    NesNametables,

    GbTiles,
    GbPalettes,

    SmsTiles,
    SmsPalettes,

    SnesTiles,
    SnesPalettes,
    SnesLayers,

    PcBda, // BIOS Data Area
}

impl InspectorTab {
    /// Get the display title for this tab
    pub fn title(&self) -> &'static str {
        match self {
            InspectorTab::Log => "📋 Log",
            InspectorTab::Memory => "💾 Memory",
            InspectorTab::Debug => "🔧 Debug",
            InspectorTab::NesTiles => "🎨 Tiles",
            InspectorTab::NesPalettes => "🎨 Palettes",
            InspectorTab::NesNametables => "🗺️ Nametables",
            InspectorTab::GbTiles => "🎨 Tiles",
            InspectorTab::GbPalettes => "🎨 Palettes",
            InspectorTab::SmsTiles => "🎨 Tiles",
            InspectorTab::SmsPalettes => "🎨 Palettes",
            InspectorTab::SnesTiles => "🎨 Tiles",
            InspectorTab::SnesPalettes => "🎨 Palettes",
            InspectorTab::SnesLayers => "📐 Layers",
            InspectorTab::PcBda => "🖥️ BDA/EBDA",
        }
    }

    /// Check if this tab is generic (available for all systems)
    pub fn is_generic(&self) -> bool {
        matches!(
            self,
            InspectorTab::Log | InspectorTab::Memory | InspectorTab::Debug
        )
    }
}

/// Get the list of tabs that should be shown for a given system
pub fn get_tabs_for_system(system_type: Option<&SystemType>) -> Vec<InspectorTab> {
    let mut tabs = vec![InspectorTab::Log, InspectorTab::Debug, InspectorTab::Memory];

    if let Some(sys_type) = system_type {
        match sys_type {
            SystemType::NES => {
                tabs.extend_from_slice(&[
                    InspectorTab::NesTiles,
                    InspectorTab::NesPalettes,
                    InspectorTab::NesNametables,
                ]);
            }
            SystemType::GameBoy => {
                tabs.extend_from_slice(&[InspectorTab::GbTiles, InspectorTab::GbPalettes]);
            }
            SystemType::SMS => {
                tabs.extend_from_slice(&[InspectorTab::SmsTiles, InspectorTab::SmsPalettes]);
            }
            SystemType::SNES => {
                tabs.extend_from_slice(&[
                    InspectorTab::SnesTiles,
                    InspectorTab::SnesPalettes,
                    InspectorTab::SnesLayers,
                ]);
            }
            SystemType::PC => {
                tabs.push(InspectorTab::PcBda);
            }
            _ => {
                // For other systems (Atari2600, N64, Chip8), just show generic tabs
            }
        }
    }

    tabs
}

/// Render the content for an inspector tab
pub fn render_inspector_tab(tab: &InspectorTab, ui: &mut Ui, tab_manager: &mut TabManager) {
    match tab {
        InspectorTab::Log => render_log_tab(ui),
        InspectorTab::Debug => {
            tab_manager.render_debug_tab(ui);
        }
        InspectorTab::Memory => render_memory_tab(ui),
        InspectorTab::NesTiles
        | InspectorTab::GbTiles
        | InspectorTab::SmsTiles
        | InspectorTab::SnesTiles => {
            tab_manager.render_tiles_tab(ui);
        }
        InspectorTab::NesPalettes
        | InspectorTab::GbPalettes
        | InspectorTab::SmsPalettes
        | InspectorTab::SnesPalettes => {
            render_palettes_tab(ui);
        }
        InspectorTab::NesNametables => {
            render_nametables_tab(ui);
        }
        InspectorTab::SnesLayers => {
            render_snes_layers_tab(ui);
        }
        InspectorTab::PcBda => {
            render_pc_bda_tab(ui);
        }
    }
}

/// Render the Log tab with actual log messages from the logging system
fn render_log_tab(ui: &mut Ui) {
    use egui::ScrollArea;
    use emu_core::logging::{LogCategory, LogConfig, LogLevel};

    let log_config = LogConfig::global();

    // Define levels array once for reuse
    let levels = [
        (LogLevel::Off, "Off"),
        (LogLevel::Error, "Error"),
        (LogLevel::Warn, "Warn"),
        (LogLevel::Info, "Info"),
        (LogLevel::Debug, "Debug"),
        (LogLevel::Trace, "Trace"),
    ];

    let categories = [
        (LogCategory::CPU, "CPU"),
        (LogCategory::Bus, "Bus"),
        (LogCategory::PPU, "PPU"),
        (LogCategory::APU, "APU"),
        (LogCategory::Interrupts, "Interrupts"),
        (LogCategory::Stubs, "Stubs"),
    ];

    ScrollArea::vertical()
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            // Top section: Log level controls
            ui.heading("Logging Configuration");
            ui.separator();
            ui.add_space(5.0);

            // Global log level
            ui.horizontal(|ui| {
                ui.label("Global Level:");
                ui.add_space(10.0);

                let global_level = log_config.get_global_level();

                for (level, name) in &levels {
                    if ui.selectable_label(global_level == *level, *name).clicked() {
                        log_config.set_global_level(*level);
                    }
                }
            });

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);

            // Category-specific log levels
            ui.heading("Component-Specific Levels");
            ui.add_space(5.0);
            ui.label("Override global level for specific components:");
            ui.add_space(10.0);

            egui::Grid::new("log_category_grid")
                .num_columns(7)
                .spacing([10.0, 8.0])
                .striped(true)
                .show(ui, |ui| {
                    // Header row
                    ui.label("");
                    for (_, name) in &levels {
                        ui.label(*name);
                    }
                    ui.end_row();

                    // Category rows
                    for (category, name) in &categories {
                        ui.label(format!("{}:", name));
                        let current_level = log_config.get_level(*category);

                        for (level, _) in &levels {
                            if ui.selectable_label(current_level == *level, "•").clicked() {
                                log_config.set_level(*category, *level);
                            }
                        }
                        ui.end_row();
                    }
                });

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);

            // Rate limit configuration
            ui.heading("Rate Limiting");
            ui.add_space(5.0);
            ui.label("Control the maximum number of logs per second per category:");
            ui.add_space(10.0);

            ui.horizontal(|ui| {
                ui.label("Max logs/second:");
                ui.add_space(10.0);

                let mut rate_limit = log_config.get_rate_limit() as i32;
                let slider = egui::Slider::new(&mut rate_limit, 1..=1000)
                    .text("logs/sec")
                    .logarithmic(true);

                if ui.add(slider).changed() {
                    log_config.set_rate_limit(rate_limit as usize);
                }
            });

            ui.add_space(5.0);
            ui.label(format!(
                "Current limit: {} logs per second per category",
                log_config.get_rate_limit()
            ));
            ui.label("When exceeded, logs are dropped and a warning is emitted.");

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);

            // Log messages
            ui.heading("Log Messages");
            ui.add_space(5.0);

            let messages = log_config.get_messages();
            if messages.is_empty() {
                ui.label(egui::RichText::new("No log messages yet").weak());
                ui.add_space(5.0);
                ui.label(
                    egui::RichText::new("Enable logging levels above to see messages")
                        .weak()
                        .italics(),
                );
            } else {
                // Show messages in a scrollable area
                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .max_height(300.0)
                    .show(ui, |ui| {
                        for msg in messages.iter().rev() {
                            let color = match msg.level {
                                LogLevel::Error => egui::Color32::from_rgb(255, 100, 100),
                                LogLevel::Warn => egui::Color32::from_rgb(255, 200, 100),
                                LogLevel::Info => egui::Color32::from_rgb(150, 200, 255),
                                LogLevel::Debug => egui::Color32::from_rgb(200, 200, 200),
                                LogLevel::Trace => egui::Color32::from_rgb(150, 150, 150),
                                _ => egui::Color32::from_rgb(200, 200, 200),
                            };

                            ui.horizontal(|ui| {
                                ui.colored_label(color, format!("[{:?}]", msg.category));
                                ui.label(&msg.message);
                            });
                        }
                    });

                ui.add_space(5.0);
                if ui.button("Clear Messages").clicked() {
                    log_config.clear_messages();
                }
            }

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);

            // Info section
            ui.heading("About Logging");
            ui.add_space(5.0);
            ui.label("Log messages are written to stderr by default.");
            ui.label("Use --log-file <path> CLI argument to log to a file.");
            ui.label("Category-specific levels override the global level.");
            ui.label("Set a category to 'Off' to use the global level.");
        });
}

/// Render the Memory Inspector tab (generic, works for all systems)
fn render_memory_tab(ui: &mut Ui) {
    ui.vertical_centered(|ui| {
        ui.add_space(40.0);
        ui.label(egui::RichText::new("💾").size(48.0));
        ui.add_space(10.0);
        ui.heading("Memory Inspector");
        ui.add_space(10.0);
        ui.label("Generic memory inspector");
        ui.label(egui::RichText::new("(To be implemented)").weak());
    });
}

/// Render the Palettes tab (for systems with palette support)
fn render_palettes_tab(ui: &mut Ui) {
    ui.vertical_centered(|ui| {
        ui.add_space(40.0);
        ui.label(egui::RichText::new("🎨").size(48.0));
        ui.add_space(10.0);
        ui.heading("Palettes");
        ui.add_space(10.0);
        ui.label("System palette viewer");
        ui.label(egui::RichText::new("(To be implemented)").weak());
    });
}

/// Render the NES Nametables tab
fn render_nametables_tab(ui: &mut Ui) {
    ui.vertical_centered(|ui| {
        ui.add_space(40.0);
        ui.label(egui::RichText::new("🗺️").size(48.0));
        ui.add_space(10.0);
        ui.heading("Nametables");
        ui.add_space(10.0);
        ui.label("NES nametable viewer");
        ui.label(egui::RichText::new("(To be implemented)").weak());
    });
}

/// Render the SNES Layers tab
fn render_snes_layers_tab(ui: &mut Ui) {
    ui.vertical_centered(|ui| {
        ui.add_space(40.0);
        ui.label(egui::RichText::new("📐").size(48.0));
        ui.add_space(10.0);
        ui.heading("Layers");
        ui.add_space(10.0);
        ui.label("SNES background layer viewer");
        ui.label(egui::RichText::new("(To be implemented)").weak());
    });
}

/// Render the PC BDA/EBDA tab
fn render_pc_bda_tab(ui: &mut Ui) {
    ui.vertical_centered(|ui| {
        ui.add_space(40.0);
        ui.label(egui::RichText::new("🖥️").size(48.0));
        ui.add_space(10.0);
        ui.heading("BIOS Data Area");
        ui.add_space(10.0);
        ui.label("PC BDA/EBDA inspector");
        ui.label(
            egui::RichText::new("(To be implemented - will show BIOS data area contents)").weak(),
        );
    });
}
