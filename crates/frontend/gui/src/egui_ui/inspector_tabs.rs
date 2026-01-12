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
    Debug,  // Generic debugger with CPU state, memory, disassembly
    Mounts, // Mount points and loaded media

    // System-specific tabs
    NesTiles,
    NesPalettes,
    NesNametables,

    GbTiles,
    GbPalettes,
    GbTilemaps, // Background/Window tilemaps

    SmsTiles,
    SmsPalettes,

    SnesTiles,
    SnesPalettes,
    SnesLayers,

    Atari2600Playfield,
    Atari2600Sprites,
    Atari2600Palette,
    Atari2600Collision,

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
            InspectorTab::NesTiles => "🎨 Tiles",
            InspectorTab::NesPalettes => "🎨 Palettes",
            InspectorTab::NesNametables => "🗺️ Nametables",
            InspectorTab::GbTiles => "🎨 Tiles",
            InspectorTab::GbPalettes => "🎨 Palettes",
            InspectorTab::GbTilemaps => "🗺️ Tilemaps",
            InspectorTab::SmsTiles => "🎨 Tiles",
            InspectorTab::SmsPalettes => "🎨 Palettes",
            InspectorTab::SnesTiles => "🎨 Tiles",
            InspectorTab::SnesPalettes => "🎨 Palettes",
            InspectorTab::SnesLayers => "📐 Layers",
            InspectorTab::Atari2600Playfield => "🎨 Playfield",
            InspectorTab::Atari2600Sprites => "👾 Sprites",
            InspectorTab::Atari2600Palette => "🎨 Palette",
            InspectorTab::Atari2600Collision => "💥 Collision",
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
    let mut tabs = vec![
        InspectorTab::Log,
        InspectorTab::Debug,
        InspectorTab::Memory,
        InspectorTab::Mounts,
    ];

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
                tabs.extend_from_slice(&[
                    InspectorTab::GbTiles,
                    InspectorTab::GbPalettes,
                    InspectorTab::GbTilemaps,
                ]);
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
            SystemType::Atari2600 => {
                tabs.extend_from_slice(&[
                    InspectorTab::Atari2600Playfield,
                    InspectorTab::Atari2600Sprites,
                    InspectorTab::Atari2600Palette,
                    InspectorTab::Atari2600Collision,
                ]);
            }
            _ => {
                // For other systems (N64, Chip8), just show generic tabs
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
        InspectorTab::Memory => {
            tab_manager.render_memory_tab(ui);
        }
        InspectorTab::Mounts => {
            tab_manager.render_mounts_tab(ui);
        }
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
        InspectorTab::NesNametables | InspectorTab::GbTilemaps => {
            tab_manager.render_tilemaps_tab(ui);
        }
        InspectorTab::SnesLayers => {
            render_snes_layers_tab(ui);
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
                    .id_salt("log_config_scroll")
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
                    .id_salt("log_messages_scroll")
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

                egui::Grid::new("bda_summary_grid")
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
                egui::Grid::new("equipment_bits_grid")
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

                egui::Grid::new("bda_map_grid")
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
