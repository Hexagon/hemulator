//! System-specific Inspector tabs

use super::tabs::{SystemTileData, TabManager};
use crate::rom_detect::SystemType;
use egui::Ui;

/// Inspector tab types - can be generic or system-specific
#[derive(Debug, Clone, PartialEq)]
pub enum InspectorTab {
    // Generic tabs (available for all systems)
    Log,
    Memory,
    Debug,  // Generic debugger with CPU state, memory, disassembly
    Mounts, // Mount points and loaded media (for non-cartridge systems like PC)

    // Cartridge tab (for cartridge-based systems - shows cartridge info + mount status)
    Cartridge,

    // System-specific tabs
    NesTiles,
    NesPalettes,
    NesNametables,

    GbTiles,
    GbPalettes,
    GbTilemaps, // Background/Window tilemaps

    SmsTiles,
    SmsPalettes,

    ColecoVisionTiles,
    ColecoVisionPalettes,
    ColecoVisionVdp, // VDP registers and state

    SnesTiles,
    SnesPalettes,
    SnesLayers,

    Atari2600Playfield,
    Atari2600Sprites,
    Atari2600Palette,
    Atari2600Collision,

    Chip8Display,
    Chip8Registers,

    PcBda, // BIOS Data Area
}

impl InspectorTab {
    /// Get the display title for this tab
    pub fn title(&self) -> &'static str {
        match self {
            InspectorTab::Log => "📋 Log",
            InspectorTab::Memory => "💾 Memory",
            InspectorTab::Debug => "🔧 Debug",
            InspectorTab::Mounts => "💿 Mounts",
            InspectorTab::Cartridge => "📦 Cartridge",
            InspectorTab::NesTiles => "🎨 Tiles",
            InspectorTab::NesPalettes => "🎨 Palettes",
            InspectorTab::NesNametables => "🗺️ Nametables",
            InspectorTab::GbTiles => "🎨 Tiles",
            InspectorTab::GbPalettes => "🎨 Palettes",
            InspectorTab::GbTilemaps => "🗺️ Tilemaps",
            InspectorTab::SmsTiles => "🎨 Tiles",
            InspectorTab::SmsPalettes => "🎨 Palettes",
            InspectorTab::ColecoVisionTiles => "🎨 Tiles",
            InspectorTab::ColecoVisionPalettes => "🎨 Palettes",
            InspectorTab::ColecoVisionVdp => "📺 VDP",
            InspectorTab::SnesTiles => "🎨 Tiles",
            InspectorTab::SnesPalettes => "🎨 Palettes",
            InspectorTab::SnesLayers => "📐 Layers",
            InspectorTab::Atari2600Playfield => "🎨 Playfield",
            InspectorTab::Atari2600Sprites => "👾 Sprites",
            InspectorTab::Atari2600Palette => "🎨 Palette",
            InspectorTab::Atari2600Collision => "💥 Collision",
            InspectorTab::Chip8Display => "📺 Display",
            InspectorTab::Chip8Registers => "📝 Registers",
            InspectorTab::PcBda => "🖥️ BDA/EBDA",
        }
    }

    /// Check if this tab is generic (available for all systems)
    pub fn is_generic(&self) -> bool {
        matches!(
            self,
            InspectorTab::Log | InspectorTab::Memory | InspectorTab::Debug | InspectorTab::Mounts
        )
    }
}

/// Get the list of tabs that should be shown for a given system
pub fn get_tabs_for_system(system_type: Option<&SystemType>) -> Vec<InspectorTab> {
    let mut tabs = vec![InspectorTab::Log, InspectorTab::Debug, InspectorTab::Memory];

    if let Some(sys_type) = system_type {
        match sys_type {
            SystemType::NES => {
                tabs.push(InspectorTab::Cartridge); // Unified cartridge tab instead of Mounts
                tabs.extend_from_slice(&[
                    InspectorTab::NesTiles,
                    InspectorTab::NesPalettes,
                    InspectorTab::NesNametables,
                ]);
            }
            SystemType::GameBoy => {
                tabs.push(InspectorTab::Cartridge); // Unified cartridge tab instead of Mounts
                tabs.extend_from_slice(&[
                    InspectorTab::GbTiles,
                    InspectorTab::GbPalettes,
                    InspectorTab::GbTilemaps,
                ]);
            }
            SystemType::SMS => {
                tabs.push(InspectorTab::Cartridge); // Unified cartridge tab instead of Mounts
                tabs.extend_from_slice(&[InspectorTab::SmsTiles, InspectorTab::SmsPalettes]);
            }
            SystemType::ColecoVision => {
                tabs.push(InspectorTab::Cartridge); // Unified cartridge tab instead of Mounts
                tabs.extend_from_slice(&[
                    InspectorTab::ColecoVisionTiles,
                    InspectorTab::ColecoVisionPalettes,
                    InspectorTab::ColecoVisionVdp,
                ]);
            }
            SystemType::SNES => {
                tabs.push(InspectorTab::Cartridge); // Unified cartridge tab instead of Mounts
                tabs.extend_from_slice(&[
                    InspectorTab::SnesTiles,
                    InspectorTab::SnesPalettes,
                    InspectorTab::SnesLayers,
                ]);
            }
            SystemType::Atari2600 => {
                tabs.push(InspectorTab::Cartridge); // Unified cartridge tab instead of Mounts
                tabs.extend_from_slice(&[
                    InspectorTab::Atari2600Playfield,
                    InspectorTab::Atari2600Sprites,
                    InspectorTab::Atari2600Palette,
                    InspectorTab::Atari2600Collision,
                ]);
            }
            SystemType::PC => {
                tabs.push(InspectorTab::Mounts); // PC uses Mounts tab (BIOS, floppies, HDD)
                tabs.push(InspectorTab::PcBda);
            }
            SystemType::Chip8 => {
                tabs.push(InspectorTab::Mounts); // CHIP-8 uses Mounts tab for "Program"
                tabs.extend_from_slice(&[InspectorTab::Chip8Display, InspectorTab::Chip8Registers]);
            }
            _ => {
                // For other systems (N64, SG-1000), show Cartridge tab if they have cartridge slot
                // N64 is cartridge-based
                tabs.push(InspectorTab::Cartridge);
            }
        }
    } else {
        // No system loaded - show generic Mounts tab
        tabs.push(InspectorTab::Mounts);
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
        InspectorTab::Memory => {
            tab_manager.render_memory_tab(ui);
        }
        InspectorTab::Mounts => {
            tab_manager.render_mounts_tab(ui);
        }
        InspectorTab::Cartridge => {
            render_cartridge_tab(ui, tab_manager);
        }
        InspectorTab::NesTiles
        | InspectorTab::GbTiles
        | InspectorTab::SmsTiles
        | InspectorTab::ColecoVisionTiles
        | InspectorTab::SnesTiles => {
            tab_manager.render_tiles_tab(ui);
        }
        InspectorTab::NesPalettes
        | InspectorTab::GbPalettes
        | InspectorTab::SmsPalettes
        | InspectorTab::ColecoVisionPalettes
        | InspectorTab::SnesPalettes => {
            render_palettes_tab(ui, tab_manager);
        }
        InspectorTab::ColecoVisionVdp => {
            render_colecovision_vdp_tab(ui, tab_manager);
        }
        InspectorTab::NesNametables | InspectorTab::GbTilemaps => {
            tab_manager.render_tilemaps_tab(ui);
        }
        InspectorTab::SnesLayers => {
            render_snes_layers_tab(ui, tab_manager);
        }
        InspectorTab::Atari2600Playfield => {
            tab_manager.render_atari2600_playfield_tab(ui);
        }
        InspectorTab::Atari2600Sprites => {
            tab_manager.render_atari2600_sprites_tab(ui);
        }
        InspectorTab::Atari2600Palette => {
            tab_manager.render_atari2600_palette_tab(ui);
        }
        InspectorTab::Atari2600Collision => {
            tab_manager.render_atari2600_collision_tab(ui);
        }
        InspectorTab::Chip8Display => {
            tab_manager.render_chip8_display_tab(ui);
        }
        InspectorTab::Chip8Registers => {
            tab_manager.render_chip8_registers_tab(ui);
        }
        InspectorTab::PcBda => {
            render_pc_bda_tab(ui, tab_manager);
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

    // Use horizontal layout: controls on left, log messages on right
    ui.horizontal_top(|ui| {
        // Left panel: Logging configuration (40% width)
        let left_width = ui.available_width() * 0.4;
        ui.allocate_ui_with_layout(
            egui::vec2(left_width, ui.available_height()),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ScrollArea::vertical()
                    .id_salt("inspector_log_config_scroll")
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                        // Top section: Log level controls
                        ui.heading("Logging Configuration");
                        ui.separator();
                        ui.add_space(5.0);

                        // Global log level
                        ui.label(egui::RichText::new("Global Level:").strong());
                        ui.add_space(5.0);

                        let global_level = log_config.get_global_level();

                        ui.vertical(|ui| {
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
                        ui.label("Override global level:");
                        ui.add_space(10.0);

                        egui::Grid::new("inspector_log_category_grid")
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
                                        if ui
                                            .selectable_label(current_level == *level, "•")
                                            .clicked()
                                        {
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
                        ui.label("Max logs/sec per category:");
                        ui.add_space(10.0);

                        let mut rate_limit = log_config.get_rate_limit() as i32;
                        let slider = egui::Slider::new(&mut rate_limit, 1..=1000)
                            .text("logs/sec")
                            .logarithmic(true);

                        if ui.add(slider).changed() {
                            log_config.set_rate_limit(rate_limit as usize);
                        }

                        ui.add_space(5.0);
                        ui.label(format!(
                            "Current: {} logs/sec per category",
                            log_config.get_rate_limit()
                        ));
                        ui.label("When exceeded, logs are dropped.");

                        ui.add_space(10.0);
                        ui.separator();
                        ui.add_space(10.0);

                        // Info section
                        ui.heading("About Logging");
                        ui.add_space(5.0);
                        ui.label("Messages are written to stderr by default.");
                        ui.label("Use --log-file <path> to log to a file.");
                        ui.label("Category levels override global level.");
                        ui.label("Set to 'Off' to use global level.");
                    });
            },
        );

        ui.separator();

        // Right panel: Log messages (60% width)
        ui.vertical(|ui| {
            ui.heading("Log Messages");
            ui.add_space(5.0);

            let messages = log_config.get_messages();
            if messages.is_empty() {
                ui.label(egui::RichText::new("No log messages yet").weak());
                ui.add_space(5.0);
                ui.label(
                    egui::RichText::new("Enable logging levels on the left to see messages")
                        .weak()
                        .italics(),
                );
            } else {
                // Show messages in a scrollable area
                let available_height = ui.available_height() - 40.0; // Reserve space for button
                egui::ScrollArea::vertical()
                    .id_salt("inspector_log_messages_scroll")
                    .auto_shrink([false; 2])
                    .max_height(available_height)
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
        });
    });
}

/// Render the Palettes tab (for systems with palette support)
fn render_palettes_tab(ui: &mut Ui, tab_manager: &TabManager) {
    use egui::ScrollArea;

    let available_height = ui.available_height();
    ScrollArea::vertical()
        .auto_shrink([false; 2])
        .max_height(available_height)
        .show(ui, |ui| {
            if let Some(ref sys_data) = tab_manager.system_tile_data {
                match sys_data {
                    crate::egui_ui::SystemTileData::SNES(snes_data) => {
                        ui.heading("🎨 SNES Palette Viewer");
                        ui.separator();
                        ui.add_space(10.0);

                        ui.label(format!(
                            "Total Colors: {} (16 palettes × 16 colors)",
                            snes_data.palette.len()
                        ));
                        ui.label(format!("CGRAM Size: {} bytes", snes_data.cgram.len()));
                        ui.add_space(10.0);

                        // Render palettes using the TabManager method
                        tab_manager.render_snes_palettes(ui, snes_data);
                    }
                    _ => {
                        ui.vertical_centered(|ui| {
                            ui.add_space(40.0);
                            ui.label(egui::RichText::new("🎨").size(48.0));
                            ui.add_space(10.0);
                            ui.heading("Palettes");
                            ui.add_space(10.0);
                            ui.label("System palette viewer");
                            ui.label(
                                egui::RichText::new("Available for NES, Game Boy, SMS, and SNES")
                                    .weak(),
                            );
                        });
                    }
                }
            } else {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.label(egui::RichText::new("🎨").size(48.0));
                    ui.add_space(10.0);
                    ui.heading("Palettes");
                    ui.add_space(10.0);
                    ui.label("System palette viewer");
                    ui.label(egui::RichText::new("Load a ROM to see palette data").weak());
                });
            }
        });
}

/// Render the SNES Layers tab
fn render_snes_layers_tab(ui: &mut Ui, tab_manager: &mut TabManager) {
    use egui::ScrollArea;

    let available_height = ui.available_height();
    ScrollArea::vertical()
        .auto_shrink([false; 2])
        .max_height(available_height)
        .show(ui, |ui| {
            if let Some(ref sys_data) = tab_manager.system_tile_data {
                if let crate::egui_ui::SystemTileData::SNES(snes_data) = sys_data {
                    // Header
                    ui.heading("📐 SNES Background Layers");
                    ui.separator();
                    ui.add_space(10.0);

                    // BG Mode information
                    ui.heading("Background Mode Configuration");
                    ui.add_space(5.0);

                    let mode_info = match snes_data.bg_mode {
                        0 => "Mode 0: 4 layers, 2bpp each (4 colors)",
                        1 => "Mode 1: BG1/BG2 4bpp (16 colors), BG3 2bpp (4 colors)",
                        2 => "Mode 2: BG1/BG2 4bpp with offset-per-tile",
                        3 => "Mode 3: BG1 8bpp (256 colors), BG2 4bpp (16 colors)",
                        4 => "Mode 4: BG1 8bpp, BG2 2bpp, offset-per-tile",
                        5 => "Mode 5: BG1 4bpp, BG2 2bpp (hi-res 512px)",
                        6 => "Mode 6: BG1 4bpp, offset-per-tile (hi-res 512px)",
                        7 => "Mode 7: BG1 8bpp (256 colors), rotation/scaling",
                        _ => "Unknown mode",
                    };

                    ui.label(
                        egui::RichText::new(format!("Current Mode: {}", snes_data.bg_mode))
                            .strong()
                            .size(16.0),
                    );
                    ui.label(mode_info);
                    ui.add_space(10.0);

                    // Layer-by-layer details
                    ui.heading("Layer Configuration");
                    ui.separator();
                    ui.add_space(5.0);

                    // Show what layers are active based on mode
                    let layers_info = match snes_data.bg_mode {
                        0 => vec![
                            ("BG1", "2bpp, 4 colors"),
                            ("BG2", "2bpp, 4 colors"),
                            ("BG3", "2bpp, 4 colors"),
                            ("BG4", "2bpp, 4 colors"),
                        ],
                        1 => vec![
                            ("BG1", "4bpp, 16 colors"),
                            ("BG2", "4bpp, 16 colors"),
                            ("BG3", "2bpp, 4 colors"),
                        ],
                        2 => vec![
                            ("BG1", "4bpp, offset-per-tile"),
                            ("BG2", "4bpp, offset-per-tile"),
                        ],
                        3 => vec![("BG1", "8bpp, 256 colors"), ("BG2", "4bpp, 16 colors")],
                        4 => vec![("BG1", "8bpp, 256 colors"), ("BG2", "2bpp, 4 colors")],
                        5 => vec![("BG1", "4bpp, hi-res"), ("BG2", "2bpp, hi-res")],
                        6 => vec![("BG1", "4bpp, hi-res, offset-per-tile")],
                        7 => vec![("BG1", "8bpp, rotation/scaling")],
                        _ => vec![],
                    };

                    egui::Grid::new("inspector_snes_layers_grid")
                        .num_columns(2)
                        .spacing([20.0, 8.0])
                        .striped(true)
                        .show(ui, |ui| {
                            ui.label(egui::RichText::new("Layer").strong());
                            ui.label(egui::RichText::new("Configuration").strong());
                            ui.end_row();

                            for (layer_name, config) in &layers_info {
                                ui.label(egui::RichText::new(*layer_name).monospace());
                                ui.label(*config);
                                ui.end_row();
                            }
                        });

                    ui.add_space(15.0);

                    // VRAM usage information
                    ui.heading("VRAM Organization");
                    ui.separator();
                    ui.add_space(5.0);

                    ui.label(format!("Total VRAM: {} KB", snes_data.vram.len() / 1024));
                    ui.label("VRAM contains:");
                    ui.label("  • Character (tile) data for background layers");
                    ui.label("  • Tilemap data for background layers");
                    ui.label("  • Sprite character data");
                    ui.add_space(5.0);
                    ui.label(
                        egui::RichText::new("Note: Use Tiles tab to view character data")
                            .weak()
                            .italics(),
                    );

                    ui.add_space(15.0);

                    // Future enhancements note
                    ui.heading("Layer Rendering Preview");
                    ui.separator();
                    ui.add_space(5.0);
                    ui.label(
                        egui::RichText::new("Individual layer preview coming soon...")
                            .weak()
                            .italics(),
                    );
                    ui.label(
                        egui::RichText::new(
                            "(Will show each BG layer separately with tilemap visualization)",
                        )
                        .weak()
                        .italics()
                        .size(12.0),
                    );
                } else {
                    ui.vertical_centered(|ui| {
                        ui.add_space(40.0);
                        ui.label(egui::RichText::new("📐").size(48.0));
                        ui.add_space(10.0);
                        ui.heading("No SNES Data");
                        ui.add_space(10.0);
                        ui.label("This tab is only available for SNES");
                    });
                }
            } else {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.label(egui::RichText::new("📐").size(48.0));
                    ui.add_space(10.0);
                    ui.heading("Layers");
                    ui.add_space(10.0);
                    ui.label("SNES background layer viewer");
                    ui.label("Load a SNES ROM to see layer data");
                });
            }
        });
}

/// Render the PC BDA/EBDA tab
fn render_pc_bda_tab(ui: &mut Ui, tab_manager: &mut TabManager) {
    use egui::ScrollArea;

    let available_height = ui.available_height();
    ScrollArea::vertical()
        .auto_shrink([false; 2])
        .max_height(available_height)
        .show(ui, |ui| {
            if let Some(ref bda_data) = tab_manager.pc_bda_data {
                // Header
                ui.heading("🖥️ PC BIOS Data Area (BDA) Inspector");
                ui.separator();
                ui.add_space(10.0);

                // System Information Summary
                ui.heading("System Information");
                ui.add_space(5.0);

                egui::Grid::new("inspector_pc_bda_summary_grid")
                    .num_columns(2)
                    .spacing([40.0, 8.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new("Memory Size:").strong());
                        ui.label(format!("{} KB", bda_data.memory_size_kb));
                        ui.end_row();

                        ui.label(egui::RichText::new("Video Mode:").strong());
                        ui.label(format!("0x{:02X}", bda_data.video_mode));
                        ui.end_row();

                        ui.label(egui::RichText::new("Video Columns:").strong());
                        ui.label(format!("{}", bda_data.video_columns));
                        ui.end_row();

                        ui.label(egui::RichText::new("Serial Ports:").strong());
                        ui.label(format!("{}", bda_data.num_serial_ports));
                        ui.end_row();

                        ui.label(egui::RichText::new("Parallel Ports:").strong());
                        ui.label(format!("{}", bda_data.num_parallel_ports));
                        ui.end_row();

                        ui.label(egui::RichText::new("Hard Drives:").strong());
                        ui.label(format!("{}", bda_data.num_hard_drives));
                        ui.end_row();

                        ui.label(egui::RichText::new("Equipment Word:").strong());
                        ui.label(format!("0x{:04X}", bda_data.equipment_word));
                        ui.end_row();

                        ui.label(egui::RichText::new("EBDA Segment:").strong());
                        ui.label(format!("0x{:04X}", bda_data.ebda_segment));
                        ui.end_row();
                    });

                ui.add_space(15.0);
                ui.separator();
                ui.add_space(10.0);

                // Equipment Word Breakdown
                ui.heading("Equipment Word (0x0410-0x0411)");
                ui.add_space(5.0);
                ui.label("Bit flags indicating installed hardware:");
                ui.add_space(5.0);

                let eq = bda_data.equipment_word;
                egui::Grid::new("inspector_pc_equipment_bits_grid")
                    .num_columns(2)
                    .spacing([40.0, 8.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label("Bit 0: Floppy drives installed");
                        ui.label(if (eq & 0x0001) != 0 {
                            "✓ Yes"
                        } else {
                            "✗ No"
                        });
                        ui.end_row();

                        ui.label("Bit 1: Math coprocessor");
                        ui.label(if (eq & 0x0002) != 0 {
                            "✓ Yes"
                        } else {
                            "✗ No"
                        });
                        ui.end_row();

                        ui.label("Bits 2-3: System RAM");
                        let ram_size = (eq >> 2) & 0x03;
                        ui.label(match ram_size {
                            0 => "16 KB",
                            1 => "32 KB",
                            2 => "48 KB",
                            3 => "64 KB or more",
                            _ => unreachable!(),
                        });
                        ui.end_row();

                        ui.label("Bits 4-5: Initial video mode");
                        let video = (eq >> 4) & 0x03;
                        ui.label(match video {
                            0 => "Reserved",
                            1 => "40x25 CGA color",
                            2 => "80x25 CGA color",
                            3 => "80x25 MDA mono",
                            _ => unreachable!(),
                        });
                        ui.end_row();

                        ui.label("Bits 6-7: Floppy drive count");
                        let floppy_count = ((eq >> 6) & 0x03).wrapping_add(1);
                        ui.label(if (eq & 0x0001) != 0 {
                            format!("{}", floppy_count)
                        } else {
                            "0".to_string()
                        });
                        ui.end_row();

                        ui.label("Bit 8: DMA controller (0 = present)");
                        ui.label(if (eq & 0x0100) == 0 {
                            "✓ Present (bit = 0)"
                        } else {
                            "✗ Not present (bit = 1)"
                        });
                        ui.end_row();

                        ui.label("Bits 9-11: Serial ports");
                        ui.label(format!("{}", bda_data.num_serial_ports));
                        ui.end_row();

                        ui.label("Bit 12: Game port");
                        ui.label(if (eq & 0x1000) != 0 {
                            "✓ Yes"
                        } else {
                            "✗ No"
                        });
                        ui.end_row();

                        ui.label("Bit 13: Serial printer");
                        ui.label(if (eq & 0x2000) != 0 {
                            "✓ Yes"
                        } else {
                            "✗ No"
                        });
                        ui.end_row();

                        ui.label("Bits 14-15: Parallel ports");
                        ui.label(format!("{}", bda_data.num_parallel_ports));
                        ui.end_row();
                    });

                ui.add_space(15.0);
                ui.separator();
                ui.add_space(10.0);

                // BDA Memory Map
                ui.heading("BDA Memory Map (0x0400-0x04FF)");
                ui.add_space(5.0);
                ui.label("Key locations in the BIOS Data Area:");
                ui.add_space(5.0);

                egui::Grid::new("inspector_pc_bda_map_grid")
                    .num_columns(3)
                    .spacing([20.0, 8.0])
                    .striped(true)
                    .show(ui, |ui| {
                        // Header row
                        ui.label(egui::RichText::new("Address").strong());
                        ui.label(egui::RichText::new("Description").strong());
                        ui.label(egui::RichText::new("Value").strong());
                        ui.end_row();

                        // Helper function to read word from BDA
                        let read_word = |offset: usize| -> u16 {
                            if offset + 1 < bda_data.bda_raw.len() {
                                bda_data.bda_raw[offset] as u16
                                    | ((bda_data.bda_raw[offset + 1] as u16) << 8)
                            } else {
                                0
                            }
                        };

                        // Helper function to read byte from BDA
                        let read_byte = |offset: usize| -> u8 {
                            if offset < bda_data.bda_raw.len() {
                                bda_data.bda_raw[offset]
                            } else {
                                0
                            }
                        };

                        // COM port addresses
                        ui.label("0x400-0x407");
                        ui.label("COM1-COM4 I/O ports");
                        ui.label(format!(
                            "COM1: 0x{:04X}, COM2: 0x{:04X}",
                            read_word(0x00),
                            read_word(0x02)
                        ));
                        ui.end_row();

                        // LPT port addresses
                        ui.label("0x408-0x40F");
                        ui.label("LPT1-LPT4 I/O ports");
                        ui.label(format!("LPT1: 0x{:04X}", read_word(0x08)));
                        ui.end_row();

                        // Equipment word
                        ui.label("0x410-0x411");
                        ui.label("Equipment word");
                        ui.label(format!("0x{:04X}", bda_data.equipment_word));
                        ui.end_row();

                        // Memory size
                        ui.label("0x413-0x414");
                        ui.label("Memory size (KB)");
                        ui.label(format!("{} KB", bda_data.memory_size_kb));
                        ui.end_row();

                        // Keyboard shift flags
                        ui.label("0x417");
                        ui.label("Keyboard shift flags");
                        ui.label(format!("0x{:02X}", read_byte(0x17)));
                        ui.end_row();

                        // Keyboard buffer
                        ui.label("0x41A-0x41D");
                        ui.label("Keyboard buffer head/tail");
                        ui.label(format!(
                            "Head: 0x{:04X}, Tail: 0x{:04X}",
                            read_word(0x1A),
                            read_word(0x1C)
                        ));
                        ui.end_row();

                        // Video mode
                        ui.label("0x449");
                        ui.label("Current video mode");
                        ui.label(format!("0x{:02X}", bda_data.video_mode));
                        ui.end_row();

                        // Video columns
                        ui.label("0x44A");
                        ui.label("Video columns");
                        ui.label(format!("{}", bda_data.video_columns));
                        ui.end_row();

                        // Video page buffer size
                        ui.label("0x44C-0x44D");
                        ui.label("Video page buffer size");
                        ui.label(format!("0x{:04X} bytes", read_word(0x4C)));
                        ui.end_row();

                        // Cursor positions (showing page 0 only)
                        ui.label("0x450-0x45F");
                        ui.label("Cursor positions (8 pages)");
                        ui.label(format!(
                            "Page 0: Col {}, Row {}",
                            read_byte(0x50),
                            read_byte(0x51)
                        ));
                        ui.end_row();

                        // Cursor shape
                        ui.label("0x460-0x461");
                        ui.label("Cursor shape");
                        ui.label(format!(
                            "Start: {}, End: {}",
                            read_byte(0x60),
                            read_byte(0x61)
                        ));
                        ui.end_row();

                        // Active video page
                        ui.label("0x462");
                        ui.label("Active video page");
                        ui.label(format!("{}", read_byte(0x62)));
                        ui.end_row();

                        // Video adapter I/O port
                        ui.label("0x463-0x464");
                        ui.label("Video adapter I/O port");
                        ui.label(format!("0x{:04X}", read_word(0x63)));
                        ui.end_row();

                        // Timer ticks
                        ui.label("0x46C-0x46F");
                        ui.label("Timer ticks since midnight");
                        let ticks = read_byte(0x6C) as u32
                            | ((read_byte(0x6D) as u32) << 8)
                            | ((read_byte(0x6E) as u32) << 16)
                            | ((read_byte(0x6F) as u32) << 24);
                        ui.label(format!("0x{:08X} ({} ticks)", ticks, ticks));
                        ui.end_row();

                        // Hard drive count
                        ui.label("0x475");
                        ui.label("Hard drive count");
                        ui.label(format!("{}", bda_data.num_hard_drives));
                        ui.end_row();

                        // Keyboard buffer start/end
                        ui.label("0x480-0x483");
                        ui.label("Keyboard buffer start/end");
                        ui.label(format!(
                            "Start: 0x{:04X}, End: 0x{:04X}",
                            read_word(0x80),
                            read_word(0x82)
                        ));
                        ui.end_row();
                    });

                ui.add_space(15.0);
                ui.separator();
                ui.add_space(10.0);

                // Raw BDA hex dump (collapsible)
                egui::CollapsingHeader::new("Raw BDA Memory (0x0400-0x04FF)")
                    .default_open(false)
                    .show(ui, |ui| {
                        ui.add_space(5.0);
                        ui.label("256 bytes of BIOS Data Area:");
                        ui.add_space(5.0);

                        egui::ScrollArea::vertical()
                            .max_height(300.0)
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 0.0;
                                    ui.monospace("Addr    ");
                                    for i in 0..16 {
                                        ui.monospace(format!("{:02X} ", i));
                                    }
                                    ui.monospace("  ASCII");
                                });

                                for row in 0..16 {
                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing.x = 0.0;
                                        ui.monospace(format!("0x04{:02X}  ", row * 16));

                                        let mut ascii_str = String::new();
                                        for col in 0..16 {
                                            let idx = row * 16 + col;
                                            let byte = bda_data.bda_raw[idx];
                                            ui.monospace(format!("{:02X} ", byte));

                                            // ASCII representation
                                            if (0x20..=0x7E).contains(&byte) {
                                                ascii_str.push(byte as char);
                                            } else {
                                                ascii_str.push('.');
                                            }
                                        }
                                        ui.monospace(format!("  {}", ascii_str));
                                    });
                                }
                            });
                    });

                ui.add_space(15.0);
                ui.separator();
                ui.add_space(10.0);

                // Raw EBDA hex dump (collapsible)
                egui::CollapsingHeader::new(format!(
                    "Extended BIOS Data Area (0x{:04X}:0x0000)",
                    bda_data.ebda_segment
                ))
                .default_open(false)
                .show(ui, |ui| {
                    ui.add_space(5.0);
                    ui.label("1KB Extended BIOS Data Area:");
                    ui.add_space(5.0);

                    egui::ScrollArea::vertical()
                        .max_height(300.0)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 0.0;
                                ui.monospace("Offset  ");
                                for i in 0..16 {
                                    ui.monospace(format!("{:02X} ", i));
                                }
                                ui.monospace("  ASCII");
                            });

                            for row in 0..64 {
                                // 1KB = 64 rows of 16 bytes
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 0.0;
                                    ui.monospace(format!("0x{:04X}  ", row * 16));

                                    let mut ascii_str = String::new();
                                    for col in 0..16 {
                                        let idx = row * 16 + col;
                                        if idx < bda_data.ebda_raw.len() {
                                            let byte = bda_data.ebda_raw[idx];
                                            ui.monospace(format!("{:02X} ", byte));

                                            // ASCII representation
                                            if (0x20..=0x7E).contains(&byte) {
                                                ascii_str.push(byte as char);
                                            } else {
                                                ascii_str.push('.');
                                            }
                                        } else {
                                            ui.monospace("   ");
                                            ascii_str.push(' ');
                                        }
                                    }
                                    ui.monospace(format!("  {}", ascii_str));
                                });
                            }
                        });
                });

                ui.add_space(10.0);
            } else {
                // No BDA data available
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.label(egui::RichText::new("🖥️").size(48.0));
                    ui.add_space(10.0);
                    ui.heading("No BDA Data Available");
                    ui.add_space(10.0);
                    ui.label("Load a PC system to see BIOS Data Area information");
                });
            }
        });
}

/// Render the ColecoVision VDP tab
fn render_colecovision_vdp_tab(ui: &mut Ui, tab_manager: &mut TabManager) {
    egui::ScrollArea::vertical()
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            if let Some(ref sys_data) = tab_manager.system_tile_data {
                if let SystemTileData::ColecoVision(colecovision_data) = sys_data {
                    ui.heading("TMS9918A VDP Registers");
                    ui.add_space(10.0);

                    egui::Grid::new("colecovision_vdp_registers")
                        .num_columns(2)
                        .striped(true)
                        .show(ui, |ui| {
                            for (i, &reg) in colecovision_data.registers.iter().enumerate() {
                                ui.monospace(format!("R{}", i));
                                ui.monospace(format!("${:02X} ({})", reg, reg));
                                ui.end_row();
                            }
                        });

                    ui.add_space(20.0);
                    ui.heading("Graphics Mode");
                    ui.add_space(5.0);

                    // Decode graphics mode from registers
                    let reg0 = colecovision_data.registers.first().copied().unwrap_or(0);
                    let reg1 = colecovision_data.registers.get(1).copied().unwrap_or(0);

                    let m1 = (reg0 & 0x02) != 0;
                    let m2 = (reg1 & 0x08) != 0;
                    let m3 = (reg1 & 0x10) != 0;

                    let mode_name = match (m3, m2, m1) {
                        (false, false, false) => "Graphics I",
                        (false, false, true) => "Text",
                        (false, true, false) => "Graphics II",
                        (true, false, false) => "Multicolor",
                        _ => "Invalid",
                    };

                    ui.label(format!("Mode: {}", mode_name));
                    ui.label(format!("M1: {}, M2: {}, M3: {}", m1, m2, m3));

                    ui.add_space(10.0);

                    // Display enable flags
                    ui.heading("Display Settings");
                    ui.add_space(5.0);

                    egui::Grid::new("colecovision_vdp_settings")
                        .num_columns(2)
                        .striped(true)
                        .show(ui, |ui| {
                            ui.label("Blank");
                            ui.label(if (reg1 & 0x40) != 0 {
                                "Enabled"
                            } else {
                                "Disabled"
                            });
                            ui.end_row();

                            ui.label("Frame Interrupt");
                            ui.label(if (reg1 & 0x20) != 0 {
                                "Enabled"
                            } else {
                                "Disabled"
                            });
                            ui.end_row();

                            ui.label("Sprites");
                            ui.label(if (reg1 & 0x02) != 0 {
                                "Enabled"
                            } else {
                                "Disabled"
                            });
                            ui.end_row();

                            ui.label("Sprite Size");
                            ui.label(if (reg1 & 0x02) != 0 { "16x16" } else { "8x8" });
                            ui.end_row();

                            ui.label("Sprite Magnification");
                            ui.label(if (reg1 & 0x01) != 0 { "2x" } else { "1x" });
                            ui.end_row();
                        });
                } else {
                    ui.vertical_centered(|ui| {
                        ui.add_space(40.0);
                        ui.label(egui::RichText::new("📺").size(48.0));
                        ui.add_space(10.0);
                        ui.heading("No VDP Data Available");
                        ui.add_space(10.0);
                        ui.label("Load a ColecoVision game to see VDP information");
                    });
                }
            } else {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.label(egui::RichText::new("📺").size(48.0));
                    ui.add_space(10.0);
                    ui.heading("No VDP Data Available");
                    ui.add_space(10.0);
                    ui.label("Load a ColecoVision game to see VDP information");
                });
            }
        });
}

/// Render the unified Cartridge tab (for all cartridge-based systems)
fn render_cartridge_tab(ui: &mut Ui, tab_manager: &mut TabManager) {
    use egui::ScrollArea;

    let available_height = ui.available_height();
    ScrollArea::vertical()
        .auto_shrink([false; 2])
        .max_height(available_height)
        .show(ui, |ui| {
            if let Some(ref cart_data) = tab_manager.cartridge_data {
                // Header
                ui.heading(format!("📦 {} Cartridge Information", cart_data.system_name));
                ui.separator();
                ui.add_space(10.0);

                // Mount Status Section
                ui.heading("Mount Status");
                ui.add_space(5.0);

                // Find the cartridge mount point from mount_info
                let cartridge_mount = tab_manager
                    .mount_info
                    .iter()
                    .find(|m| m.id == "Cartridge" || m.id == "cartridge");

                if let Some(mount) = cartridge_mount {
                    egui::Grid::new("inspector_cart_mount_grid")
                        .num_columns(2)
                        .spacing([40.0, 8.0])
                        .striped(true)
                        .show(ui, |ui| {
                            ui.label(egui::RichText::new("Slot:").strong());
                            ui.label(&mount.name);
                            ui.end_row();

                            ui.label(egui::RichText::new("Status:").strong());
                            if mount.mounted_file.is_some() {
                                ui.label(
                                    egui::RichText::new("✅ Mounted")
                                        .color(egui::Color32::from_rgb(100, 200, 100)),
                                );
                            } else {
                                ui.label(
                                    egui::RichText::new("⚠️ Empty")
                                        .color(egui::Color32::from_rgb(200, 200, 100)),
                                );
                            }
                            ui.end_row();

                            if let Some(ref file_path) = mount.mounted_file {
                                let filename = std::path::Path::new(file_path)
                                    .file_name()
                                    .and_then(|n| n.to_str())
                                    .unwrap_or(file_path);

                                ui.label(egui::RichText::new("File:").strong());
                                ui.label(
                                    egui::RichText::new(filename)
                                        .monospace()
                                        .color(egui::Color32::from_rgb(100, 200, 100)),
                                );
                                ui.end_row();

                                if filename != file_path {
                                    ui.label("");
                                    ui.label(
                                        egui::RichText::new(file_path)
                                            .weak()
                                            .italics()
                                            .size(10.0),
                                    );
                                    ui.end_row();
                                }
                            }

                            ui.label(egui::RichText::new("Accepted:").strong());
                            ui.label(
                                egui::RichText::new(mount.extensions.join(", "))
                                    .monospace()
                                    .size(12.0),
                            );
                            ui.end_row();
                        });
                }

                ui.add_space(15.0);
                ui.separator();
                ui.add_space(10.0);

                // ROM Information
                ui.heading("ROM Information");
                ui.add_space(5.0);

                egui::Grid::new("inspector_cart_rom_grid")
                    .num_columns(2)
                    .spacing([40.0, 8.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new("CRC32:").strong());
                        ui.monospace(format!("0x{:08X}", cart_data.crc32));
                        ui.end_row();

                        ui.label(egui::RichText::new("Size:").strong());
                        ui.label(format!(
                            "{} KB ({} bytes)",
                            cart_data.rom_size / 1024,
                            cart_data.rom_size
                        ));
                        ui.end_row();
                    });

                // NES-specific information (if available)
                if cart_data.nes_mapper.is_some() {
                    ui.add_space(15.0);
                    ui.separator();
                    ui.add_space(10.0);

                    ui.heading("NES Mapper Configuration");
                    ui.add_space(5.0);

                    egui::Grid::new("inspector_cart_nes_mapper_grid")
                        .num_columns(2)
                        .spacing([40.0, 8.0])
                        .striped(true)
                        .show(ui, |ui| {
                            if let Some(mapper) = cart_data.nes_mapper {
                                ui.label(egui::RichText::new("Mapper:").strong());
                                if let Some(submapper) = cart_data.nes_submapper {
                                    if submapper > 0 {
                                        ui.label(format!(
                                            "{} (Mapper {}, Submapper {})",
                                            cart_data.nes_mapper_name.as_deref().unwrap_or("Unknown"),
                                            mapper,
                                            submapper
                                        ));
                                    } else {
                                        ui.label(format!(
                                            "{} ({})",
                                            cart_data.nes_mapper_name.as_deref().unwrap_or("Unknown"),
                                            mapper
                                        ));
                                    }
                                } else {
                                    ui.label(format!(
                                        "{} ({})",
                                        cart_data.nes_mapper_name.as_deref().unwrap_or("Unknown"),
                                        mapper
                                    ));
                                }
                                ui.end_row();
                            }

                            if let Some(ref mirroring) = cart_data.nes_mirroring {
                                ui.label(egui::RichText::new("Mirroring:").strong());
                                ui.label(mirroring);
                                ui.end_row();
                            }

                            if let Some(ref timing) = cart_data.nes_timing {
                                ui.label(egui::RichText::new("Timing:").strong());
                                ui.label(timing);
                                ui.end_row();
                            }

                            if let Some(prg_size) = cart_data.nes_prg_size {
                                ui.label(egui::RichText::new("PRG ROM:").strong());
                                ui.label(format!("{} KB ({} bytes)", prg_size / 1024, prg_size));
                                ui.end_row();
                            }

                            if let Some(chr_size) = cart_data.nes_chr_size {
                                ui.label(egui::RichText::new("CHR ROM:").strong());
                                if chr_size > 0 {
                                    ui.label(format!("{} KB ({} bytes)", chr_size / 1024, chr_size));
                                } else {
                                    ui.label("CHR-RAM (no CHR ROM)");
                                }
                                ui.end_row();
                            }

                            if let Some(ref board) = cart_data.nes_board_name {
                                ui.label(egui::RichText::new("Board:").strong());
                                ui.label(board);
                                ui.end_row();
                            }
                        });

                    // Database Overrides Section (NES-specific)
                    if cart_data.nes_db_mapper_override || cart_data.nes_db_mirroring_override {
                        ui.add_space(15.0);
                        ui.separator();
                        ui.add_space(10.0);

                        ui.heading("ROM Database Overrides");
                        ui.add_space(5.0);
                        ui.label(
                            egui::RichText::new(
                                "⚠ This ROM has known incorrect header information",
                            )
                            .color(egui::Color32::from_rgb(255, 200, 100)),
                        );
                        ui.add_space(5.0);

                        egui::Grid::new("inspector_cart_nes_db_grid")
                            .num_columns(3)
                            .spacing([20.0, 8.0])
                            .striped(true)
                            .show(ui, |ui| {
                                ui.label(egui::RichText::new("Property").strong());
                                ui.label(egui::RichText::new("Header Value").strong());
                                ui.label(egui::RichText::new("Corrected Value").strong());
                                ui.end_row();

                                if cart_data.nes_db_mapper_override {
                                    ui.label("Mapper:");
                                    if let (Some(h_mapper), Some(h_submapper)) = (
                                        cart_data.nes_header_mapper,
                                        cart_data.nes_header_submapper,
                                    ) {
                                        if h_submapper > 0 {
                                            ui.label(format!("{}.{}", h_mapper, h_submapper));
                                        } else {
                                            ui.label(format!("{}", h_mapper));
                                        }
                                    } else {
                                        ui.label("?");
                                    }

                                    if let (Some(mapper), Some(submapper)) =
                                        (cart_data.nes_mapper, cart_data.nes_submapper)
                                    {
                                        if submapper > 0 {
                                            ui.label(
                                                egui::RichText::new(format!(
                                                    "{}.{} ✓",
                                                    mapper, submapper
                                                ))
                                                .color(egui::Color32::from_rgb(100, 255, 100)),
                                            );
                                        } else {
                                            ui.label(
                                                egui::RichText::new(format!("{} ✓", mapper))
                                                    .color(egui::Color32::from_rgb(100, 255, 100)),
                                            );
                                        }
                                    }
                                    ui.end_row();
                                }

                                if cart_data.nes_db_mirroring_override {
                                    ui.label("Mirroring:");
                                    if let Some(ref h_mirroring) = cart_data.nes_header_mirroring {
                                        ui.label(h_mirroring);
                                    } else {
                                        ui.label("?");
                                    }
                                    if let Some(ref mirroring) = cart_data.nes_mirroring {
                                        ui.label(
                                            egui::RichText::new(format!("{} ✓", mirroring))
                                                .color(egui::Color32::from_rgb(100, 255, 100)),
                                        );
                                    }
                                    ui.end_row();
                                }
                            });

                        ui.add_space(10.0);
                        ui.label(
                            egui::RichText::new(
                                "Values shown above are from the ROM database and override the iNES header.",
                            )
                            .weak()
                            .italics(),
                        );
                    } else {
                        ui.add_space(15.0);
                        ui.separator();
                        ui.add_space(10.0);

                        ui.heading("Header Information");
                        ui.add_space(5.0);
                        ui.label(
                            egui::RichText::new(
                                "✓ Header information is correct (no database overrides needed)",
                            )
                            .color(egui::Color32::from_rgb(100, 255, 100)),
                        );
                    }

                    ui.add_space(15.0);
                    ui.separator();
                    ui.add_space(10.0);

                    ui.heading("About");
                    ui.add_space(5.0);
                    ui.label(
                        "The CRC32 checksum is calculated from the entire ROM file (including iNES header).",
                    );
                    ui.label(
                        "The ROM database can override incorrect mapper or mirroring values from the header.",
                    );
                    ui.label("See crates/systems/nes/src/rom_db.rs to add new ROM database entries.");
                }
            } else {
                // No cartridge data available
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.label(egui::RichText::new("📦").size(48.0));
                    ui.add_space(10.0);
                    ui.heading("No Cartridge Loaded");
                    ui.add_space(10.0);
                    ui.label("Load a ROM to see cartridge information");
                });
            }
        });
}
