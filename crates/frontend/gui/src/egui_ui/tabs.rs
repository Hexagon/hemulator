//! Tab manager for left panel

use crate::settings::ScalingMode;
use crate::system_adapter::{EnhancedDebugState, SystemDebugInfo};
use egui::{ScrollArea, TextureHandle, Ui};
use emu_core::debug::MemoryRegion;

/// Application version constant
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Emulator,
    NewProject,
    Log,
    Help,
    Debug,
    PcConfig, // PC-specific configuration tab (DBA: Disk/BIOS/Adapter)
    About,
}

/// Actions that can be triggered from tabs
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TabAction {
    CreateNewProject(String), // String is the system name
}

/// Debug actions that can be triggered from the debug tab
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DebugAction {
    Step,   // Step one instruction
    Pause,  // Pause emulation
    Resume, // Resume emulation
}

/// PC-specific configuration information for the DBA tab
#[derive(Clone)]
pub struct PcConfigInfo {
    pub cpu_model: String,
    pub memory_kb: u32,
    pub video_adapter: String,
    pub boot_priority: String,
    pub bios_mounted: bool,
    pub floppy_a_mounted: bool,
    pub floppy_b_mounted: bool,
    pub hdd_mounted: bool,
}

pub struct TabManager {
    pub active_tab: Tab,
    pub log_messages: Vec<String>,
    pub help_visible: bool,
    pub debug_visible: bool,
    pub pc_config_visible: bool,
    pub about_visible: bool,
    pub debug_info: Option<SystemDebugInfo>,
    pub enhanced_debug_state: Option<EnhancedDebugState>,
    pub new_project_visible: bool,
    pub selected_system: String,
    pub pending_action: Option<TabAction>,
    pub pending_debug_action: Option<DebugAction>,
    pub pc_config_info: Option<PcConfigInfo>,
    pub selected_memory_region_index: usize,
    pub memory_view_address: u32,
}

impl TabManager {
    pub fn new() -> Self {
        Self {
            active_tab: Tab::Emulator,
            log_messages: Vec::new(),
            help_visible: false,
            debug_visible: false,
            pc_config_visible: false,
            about_visible: false,
            debug_info: None,
            enhanced_debug_state: None,
            new_project_visible: false,
            selected_system: "NES".to_string(),
            pending_action: None,
            pending_debug_action: None,
            pc_config_info: None,
            selected_memory_region_index: 0,
            memory_view_address: 0,
        }
    }

    pub fn add_log(&mut self, message: String) {
        self.log_messages.push(message);
        // Keep only last 1000 messages
        if self.log_messages.len() > 1000 {
            self.log_messages.remove(0);
        }
    }

    pub fn show_pc_config_tab(&mut self) {
        self.pc_config_visible = true;
        self.active_tab = Tab::PcConfig;
    }

    pub fn update_pc_config_info(&mut self, info: PcConfigInfo) {
        self.pc_config_info = Some(info);
    }

    pub fn show_help_tab(&mut self) {
        self.help_visible = true;
        self.active_tab = Tab::Help;
    }

    pub fn show_debug_tab(&mut self) {
        self.debug_visible = true;
        self.active_tab = Tab::Debug;
    }

    pub fn show_about_tab(&mut self) {
        self.about_visible = true;
        self.active_tab = Tab::About;
    }

    pub fn show_new_project_tab(&mut self) {
        self.new_project_visible = true;
        self.active_tab = Tab::NewProject;
    }

    pub fn update_debug_info(&mut self, info: SystemDebugInfo) {
        self.debug_info = Some(info);
    }

    pub fn update_enhanced_debug_state(&mut self, state: EnhancedDebugState) {
        self.enhanced_debug_state = Some(state);
    }

    /// Get and clear any pending action
    pub fn take_action(&mut self) -> Option<TabAction> {
        self.pending_action.take()
    }

    /// Get and clear any pending debug action
    pub fn take_debug_action(&mut self) -> Option<DebugAction> {
        self.pending_debug_action.take()
    }

    pub fn ui(
        &mut self,
        ui: &mut Ui,
        emulator_texture: &Option<TextureHandle>,
        scaling_mode: ScalingMode,
    ) {
        // Tab bar with improved visual styling
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.active_tab, Tab::Emulator, "🎮 Emulator");

            if self.new_project_visible {
                ui.selectable_value(&mut self.active_tab, Tab::NewProject, "➕ New Project");
                // Use a colored button for the close icon to ensure visibility
                let close_button = egui::Button::new(
                    egui::RichText::new("✖").color(egui::Color32::from_rgb(220, 220, 220)),
                )
                .small();
                if ui
                    .add(close_button)
                    .on_hover_text("Close New Project tab")
                    .clicked()
                {
                    self.new_project_visible = false;
                    if self.active_tab == Tab::NewProject {
                        self.active_tab = Tab::Emulator;
                    }
                }
            }

            // Log tab is always clickable
            ui.selectable_value(&mut self.active_tab, Tab::Log, "📋 Log");

            if self.help_visible {
                ui.selectable_value(&mut self.active_tab, Tab::Help, "❓ Help");
                // Use a colored button for the close icon to ensure visibility
                let close_button = egui::Button::new(
                    egui::RichText::new("✖").color(egui::Color32::from_rgb(220, 220, 220)),
                )
                .small();
                if ui
                    .add(close_button)
                    .on_hover_text("Close Help tab")
                    .clicked()
                {
                    self.help_visible = false;
                    if self.active_tab == Tab::Help {
                        self.active_tab = Tab::Emulator;
                    }
                }
            }

            if self.debug_visible {
                ui.selectable_value(&mut self.active_tab, Tab::Debug, "🔧 Debug");
                // Use a colored button for the close icon to ensure visibility
                let close_button = egui::Button::new(
                    egui::RichText::new("✖").color(egui::Color32::from_rgb(220, 220, 220)),
                )
                .small();
                if ui
                    .add(close_button)
                    .on_hover_text("Close Debug tab")
                    .clicked()
                {
                    self.debug_visible = false;
                    if self.active_tab == Tab::Debug {
                        self.active_tab = Tab::Emulator;
                    }
                }
            }

            if self.about_visible {
                ui.selectable_value(&mut self.active_tab, Tab::About, "ℹ️ About");
                // Use a colored button for the close icon to ensure visibility
                let close_button = egui::Button::new(
                    egui::RichText::new("✖").color(egui::Color32::from_rgb(220, 220, 220)),
                )
                .small();
                if ui
                    .add(close_button)
                    .on_hover_text("Close About tab")
                    .clicked()
                {
                    self.about_visible = false;
                    if self.active_tab == Tab::About {
                        self.active_tab = Tab::Emulator;
                    }
                }
            }

            // PC Config tab is deprecated - all info now shown in property pane
            // Keep the data structure for backward compatibility but don't show the tab
            // if self.pc_config_visible {
            //     ui.selectable_value(&mut self.active_tab, Tab::PcConfig, "PC Config");
            //     ...
            // }
        });

        ui.separator();

        // Tab content
        match self.active_tab {
            Tab::Emulator => self.render_emulator_tab(ui, emulator_texture, scaling_mode),
            Tab::NewProject => self.render_new_project_tab(ui),
            Tab::Log => self.render_log_tab(ui),
            Tab::Help => self.render_help_tab(ui),
            Tab::Debug => self.render_debug_tab(ui),
            Tab::About => self.render_about_tab(ui),
            // Keep PcConfig render for backward compat, but it won't be accessible
            Tab::PcConfig => self.render_pc_config_tab(ui),
        }
    }

    fn render_emulator_tab(
        &self,
        ui: &mut Ui,
        emulator_texture: &Option<TextureHandle>,
        scaling_mode: ScalingMode,
    ) {
        ui.centered_and_justified(|ui| {
            if let Some(texture) = emulator_texture {
                let available_size = ui.available_size();
                let texture_size = texture.size_vec2();
                let aspect_ratio = texture_size.x / texture_size.y;

                let (display_width, display_height) = match scaling_mode {
                    ScalingMode::Original => {
                        // 1:1 pixel mapping - use original texture size
                        (texture_size.x, texture_size.y)
                    }
                    ScalingMode::Fit => {
                        // Fit to window while maintaining aspect ratio
                        let display_width = available_size.x.min(available_size.y * aspect_ratio);
                        let display_height = display_width / aspect_ratio;
                        (display_width, display_height)
                    }
                    ScalingMode::Stretch => {
                        // Fill entire window, ignoring aspect ratio
                        (available_size.x, available_size.y)
                    }
                };

                let image = egui::Image::from_texture(texture)
                    .fit_to_exact_size(egui::vec2(display_width, display_height));
                ui.add(image);
            } else {
                // Welcome screen when no ROM is loaded
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.heading(
                        egui::RichText::new("🎮 Welcome to Hemulator")
                            .size(28.0)
                            .strong(),
                    );
                    ui.add_space(10.0);
                    ui.label(
                        egui::RichText::new("Multi-System Console Emulator")
                            .size(16.0)
                            .weak(),
                    );
                    ui.add_space(30.0);

                    // Quick start instructions
                    ui.label(egui::RichText::new("Quick Start").size(18.0).strong());
                    ui.add_space(10.0);

                    egui::Frame::new()
                        .fill(egui::Color32::from_rgb(30, 30, 30))
                        .corner_radius(8.0)
                        .inner_margin(15.0)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new("1.").strong().size(16.0));
                                ui.label("Press F3 or use File → Open ROM to load a game");
                            });
                            ui.add_space(5.0);
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new("2.").strong().size(16.0));
                                ui.label("Or use File → New Project to create a new system");
                            });
                            ui.add_space(5.0);
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new("3.").strong().size(16.0));
                                ui.label(
                                    "Use the property pane on the right to configure settings",
                                );
                            });
                        });

                    ui.add_space(20.0);

                    // Keyboard shortcuts
                    ui.label(
                        egui::RichText::new("Keyboard Shortcuts")
                            .size(18.0)
                            .strong(),
                    );
                    ui.add_space(10.0);

                    egui::Grid::new("shortcuts_grid")
                        .num_columns(2)
                        .spacing([20.0, 5.0])
                        .striped(true)
                        .show(ui, |ui| {
                            ui.label(egui::RichText::new("F3").strong());
                            ui.label("Open ROM");
                            ui.end_row();

                            ui.label(egui::RichText::new("F2").strong());
                            ui.label("Reset system");
                            ui.end_row();

                            ui.label(egui::RichText::new("P").strong());
                            ui.label("Pause/Resume");
                            ui.end_row();

                            ui.label(egui::RichText::new("F4").strong());
                            ui.label("Take screenshot");
                            ui.end_row();

                            ui.label(egui::RichText::new("F11").strong());
                            ui.label("Fullscreen");
                            ui.end_row();

                            ui.label(egui::RichText::new("F5-F9").strong());
                            ui.label("Save state (slots 1-5)");
                            ui.end_row();

                            ui.label(egui::RichText::new("Shift+F5-F9").strong());
                            ui.label("Load state (slots 1-5)");
                            ui.end_row();
                        });
                });
            }
        });
    }

    fn render_log_tab(&self, ui: &mut Ui) {
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

        // Top section: Log level controls
        ui.heading("Logging Configuration");
        ui.separator();
        ui.add_space(5.0);

        ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
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

                // Info section
                ui.heading("About Logging");
                ui.add_space(5.0);
                ui.label("Log messages are written to stderr by default.");
                ui.label("Use --log-file <path> CLI argument to log to a file.");
                ui.label("Category-specific levels override the global level.");
                ui.label("Set a category to 'Off' to use the global level.");

                ui.add_space(10.0);

                // Legacy log messages section (kept for backward compatibility)
                if !self.log_messages.is_empty() {
                    ui.add_space(15.0);
                    ui.separator();
                    ui.add_space(10.0);
                    ui.heading("Application Messages");
                    ui.add_space(5.0);

                    for msg in &self.log_messages {
                        ui.label(msg);
                    }
                }
            });
    }

    fn render_new_project_tab(&mut self, ui: &mut Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(20.0);
            ui.heading("Create New Project");
            ui.add_space(10.0);
            ui.label("Select the system you want to emulate:");
            ui.add_space(10.0);

            // Scrollable area for system selection boxes
            ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    // Define systems with their descriptions
                    let systems = vec![
                        (
                            "NES",
                            "Nintendo Entertainment System",
                            "Classic 8-bit console with extensive game library",
                        ),
                        (
                            "Game Boy",
                            "Game Boy / Game Boy Color",
                            "Portable gaming system with monochrome and color support",
                        ),
                        (
                            "Atari 2600",
                            "Atari 2600",
                            "Pioneering home video game console from 1977",
                        ),
                        (
                            "SMS",
                            "Sega Master System",
                            "8-bit console with tile-based graphics and Z80 CPU",
                        ),
                        (
                            "CHIP-8",
                            "CHIP-8",
                            "Classic interpreted programming language for vintage computers",
                        ),
                        (
                            "SNES",
                            "Super Nintendo Entertainment System",
                            "16-bit console with advanced graphics and sound",
                        ),
                        (
                            "N64",
                            "Nintendo 64",
                            "First Nintendo console with 3D graphics capabilities",
                        ),
                        (
                            "PC",
                            "IBM PC/XT Compatible",
                            "DOS-based personal computer emulation",
                        ),
                    ];

                    for (system_id, system_name, description) in systems {
                        let is_selected = self.selected_system == system_id;

                        // Create a clickable frame for each system
                        let frame = egui::Frame::new()
                            .fill(if is_selected {
                                ui.visuals().selection.bg_fill
                            } else {
                                ui.visuals().window_fill()
                            })
                            .stroke(if is_selected {
                                ui.visuals().selection.stroke
                            } else {
                                ui.visuals().widgets.noninteractive.bg_stroke
                            })
                            .corner_radius(4.0)
                            .inner_margin(12.0);

                        let response = frame.show(ui, |ui| {
                            ui.set_min_width(ui.available_width());
                            ui.vertical(|ui| {
                                ui.heading(system_name);
                                ui.add_space(4.0);
                                ui.label(description);
                            });
                        });

                        // Make the entire frame clickable
                        if response.response.interact(egui::Sense::click()).clicked() {
                            self.selected_system = system_id.to_string();
                            // Immediately create the project when a system is clicked
                            self.pending_action =
                                Some(TabAction::CreateNewProject(system_id.to_string()));
                            self.new_project_visible = false;
                            self.active_tab = Tab::Emulator;
                        }

                        ui.add_space(8.0);
                    }
                });

            ui.add_space(10.0);
            ui.label("Click a system to create a new blank project.");
            ui.label("After creating, load ROMs/disks via File > Open ROM.");
        });
    }

    fn render_help_tab(&self, ui: &mut Ui) {
        ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(10.0);
                    ui.heading(egui::RichText::new("⌨️ Controls & Help").size(24.0).strong());
                    ui.add_space(5.0);
                    ui.label(egui::RichText::new("Keyboard shortcuts and game controls").weak());
                });

                ui.add_space(15.0);
                ui.separator();
                ui.add_space(10.0);

                // File Operations
                ui.heading(egui::RichText::new("📁 File Operations").strong());
                ui.add_space(5.0);
                egui::Grid::new("file_ops_grid")
                    .num_columns(2)
                    .spacing([15.0, 5.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new("F3").strong().monospace());
                        ui.label("Open ROM or disk image");
                        ui.end_row();

                        ui.label(egui::RichText::new("ESC").strong().monospace());
                        ui.label("Exit emulator");
                        ui.end_row();
                    });
                ui.add_space(10.0);

                // Emulation Control
                ui.heading(egui::RichText::new("🎮 Emulation Control").strong());
                ui.add_space(5.0);
                egui::Grid::new("emulation_grid")
                    .num_columns(2)
                    .spacing([15.0, 5.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new("F2").strong().monospace());
                        ui.label("Reset system");
                        ui.end_row();

                        ui.label(egui::RichText::new("P").strong().monospace());
                        ui.label("Pause/Resume emulation");
                        ui.end_row();
                    });
                ui.add_space(10.0);

                // Save States
                ui.heading(egui::RichText::new("💾 Save States").strong());
                ui.add_space(5.0);
                egui::Grid::new("save_states_grid")
                    .num_columns(2)
                    .spacing([15.0, 5.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new("F5-F9").strong().monospace());
                        ui.label("Quick save to slots 1-5");
                        ui.end_row();

                        ui.label(egui::RichText::new("Shift+F5-F9").strong().monospace());
                        ui.label("Quick load from slots 1-5");
                        ui.end_row();
                    });
                ui.add_space(10.0);

                // View Options
                ui.heading(egui::RichText::new("👁️ View Options").strong());
                ui.add_space(5.0);
                egui::Grid::new("view_grid")
                    .num_columns(2)
                    .spacing([15.0, 5.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new("F4").strong().monospace());
                        ui.label("Take screenshot");
                        ui.end_row();

                        ui.label(egui::RichText::new("F11").strong().monospace());
                        ui.label("Toggle fullscreen (no GUI)");
                        ui.end_row();

                        ui.label(egui::RichText::new("Host+F11").strong().monospace());
                        ui.label("Toggle fullscreen (with GUI)");
                        ui.end_row();
                    });
                ui.add_space(10.0);

                // Default Game Controls
                ui.heading(egui::RichText::new("🕹️ Default Game Controls (Player 1)").strong());
                ui.add_space(5.0);
                egui::Grid::new("controls_grid")
                    .num_columns(2)
                    .spacing([15.0, 5.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new("Arrow Keys").strong().monospace());
                        ui.label("D-Pad (Up/Down/Left/Right)");
                        ui.end_row();

                        ui.label(egui::RichText::new("Z").strong().monospace());
                        ui.label("A Button (Confirm/Jump)");
                        ui.end_row();

                        ui.label(egui::RichText::new("X").strong().monospace());
                        ui.label("B Button (Back/Action)");
                        ui.end_row();

                        ui.label(egui::RichText::new("Enter").strong().monospace());
                        ui.label("Start Button");
                        ui.end_row();

                        ui.label(egui::RichText::new("Left Shift").strong().monospace());
                        ui.label("Select Button");
                        ui.end_row();
                    });
                ui.add_space(10.0);

                // Player 2 Controls
                ui.heading(egui::RichText::new("🎮 Player 2 Controls").strong());
                ui.add_space(5.0);
                egui::Grid::new("p2_controls_grid")
                    .num_columns(2)
                    .spacing([15.0, 5.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new("I/J/K/L").strong().monospace());
                        ui.label("D-Pad (I=Up, K=Down, J=Left, L=Right)");
                        ui.end_row();

                        ui.label(egui::RichText::new("U").strong().monospace());
                        ui.label("A Button");
                        ui.end_row();

                        ui.label(egui::RichText::new("O").strong().monospace());
                        ui.label("B Button");
                        ui.end_row();

                        ui.label(egui::RichText::new("P").strong().monospace());
                        ui.label("Start Button");
                        ui.end_row();

                        ui.label(egui::RichText::new("Right Shift").strong().monospace());
                        ui.label("Select Button");
                        ui.end_row();
                    });
                ui.add_space(15.0);

                // Note about customization
                egui::Frame::new()
                    .fill(egui::Color32::from_rgb(30, 30, 30))
                    .corner_radius(8.0)
                    .inner_margin(12.0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("ℹ️").size(18.0));
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new("Tip:").strong());
                                ui.label("All controls can be customized in config.json");
                                ui.label("Use the Property Pane → Project Settings → Input Configuration");
                                ui.label("to configure input devices and button mappings.");
                            });
                        });
                    });

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(10.0);

                ui.label("For system-specific information and advanced features,");
                ui.label("see the user manual: docs/MANUAL.md");
            });
    }

    fn render_debug_tab(&mut self, ui: &mut Ui) {
        // If we have enhanced debug state, show the comprehensive 3-panel view
        if let Some(state) = self.enhanced_debug_state.clone() {
            self.render_enhanced_debug_view(ui, &state);
        } else if let Some(ref debug_info) = self.debug_info {
            // Fallback to simple legacy view
            self.render_legacy_debug_view(ui, debug_info);
        } else {
            // No debug information available
            let available_height = ui.available_height();
            ScrollArea::vertical()
                .auto_shrink([false; 2])
                .max_height(available_height)
                .show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(40.0);
                        ui.label(egui::RichText::new("🔧").size(48.0));
                        ui.add_space(10.0);
                        ui.heading("No Debug Information Available");
                        ui.add_space(10.0);
                        ui.label("Load a ROM to see system-specific debug information");
                        ui.add_space(5.0);
                        ui.label(
                            egui::RichText::new("Debug info includes CPU state, memory maps, and")
                                .weak(),
                        );
                        ui.label(
                            egui::RichText::new("disassembly for troubleshooting and analysis.")
                                .weak(),
                        );
                    });
                });
        }
    }

    fn render_enhanced_debug_view(&mut self, ui: &mut Ui, state: &EnhancedDebugState) {
        let total_available_height = ui.available_height();

        ui.vertical(|ui| {
            // Header
            ui.vertical_centered(|ui| {
                ui.add_space(5.0);
                ui.heading(
                    egui::RichText::new(format!("🔧 {} Debugger", state.system_type))
                        .size(20.0)
                        .strong(),
                );
            });

            ui.add_space(5.0);
            ui.separator();

            // Debugger toolbar
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Controls:").strong());

                if ui.button("⏸ Pause").clicked() {
                    self.pending_debug_action = Some(DebugAction::Pause);
                }

                if ui.button("▶ Resume").clicked() {
                    self.pending_debug_action = Some(DebugAction::Resume);
                }

                if ui.button("⏭ Step").clicked() {
                    self.pending_debug_action = Some(DebugAction::Step);
                }
            });

            ui.add_space(5.0);
            ui.separator();
            ui.add_space(5.0);

            // 3-column layout: Disassembly | Memory | CPU State
            // Use horizontal layout with equal-width columns that fill available height
            let header_height = 120.0; // Approximate height used by header elements above (including toolbar)
            ui.horizontal_top(|ui| {
                let available_width = ui.available_width();
                let column_width = available_width / 3.0 - 10.0; // 3 columns with spacing
                let content_height = total_available_height - header_height;

                // Left panel: Disassembly
                ui.vertical(|ui| {
                    ui.set_width(column_width);
                    ui.set_height(content_height);
                    ui.heading("📜 Disassembly");
                    ui.separator();
                    ScrollArea::vertical()
                        .auto_shrink([false; 2])
                        .show(ui, |ui| {
                            self.render_disassembly_panel(ui, state);
                        });
                });

                ui.add_space(10.0);

                // Middle panel: Memory Explorer
                ui.vertical(|ui| {
                    ui.set_width(column_width);
                    ui.set_height(content_height);
                    ui.heading("💾 Memory");
                    ui.separator();
                    ScrollArea::vertical()
                        .auto_shrink([false; 2])
                        .show(ui, |ui| {
                            self.render_memory_panel(ui, state);
                        });
                });

                ui.add_space(10.0);

                // Right panel: CPU State
                ui.vertical(|ui| {
                    ui.set_width(column_width);
                    ui.set_height(content_height);
                    ui.heading("🖥️ CPU State");
                    ui.separator();
                    ScrollArea::vertical()
                        .auto_shrink([false; 2])
                        .show(ui, |ui| {
                            self.render_cpu_state_panel(ui, state);
                        });
                });
            });
        });
    }

    fn render_disassembly_panel(&self, ui: &mut Ui, state: &EnhancedDebugState) {
        if state.disassembly.is_empty() {
            ui.label(egui::RichText::new("No disassembly available").weak());
            return;
        }

        // Show disassembled instructions
        egui::Grid::new("disasm_grid")
            .num_columns(3)
            .spacing([5.0, 2.0])
            .striped(false)
            .show(ui, |ui| {
                for instr in &state.disassembly {
                    let is_current = instr.address == state.current_pc;

                    // Highlight current instruction
                    let bg_color = if is_current {
                        egui::Color32::from_rgb(60, 80, 100)
                    } else {
                        ui.visuals().window_fill()
                    };

                    egui::Frame::new().fill(bg_color).show(ui, |ui| {
                        // Address
                        let addr_text = if is_current {
                            egui::RichText::new(format!("▶ {:04X}", instr.address))
                                .monospace()
                                .strong()
                        } else {
                            egui::RichText::new(format!("  {:04X}", instr.address)).monospace()
                        };
                        ui.label(addr_text);
                    });

                    egui::Frame::new().fill(bg_color).show(ui, |ui| {
                        // Bytes
                        let bytes_str: String = instr
                            .bytes
                            .iter()
                            .map(|b| format!("{:02X}", b))
                            .collect::<Vec<_>>()
                            .join(" ");
                        ui.label(egui::RichText::new(bytes_str).monospace());
                    });

                    egui::Frame::new().fill(bg_color).show(ui, |ui| {
                        // Mnemonic with optional comment
                        let mnem_with_comment = if let Some(ref comment) = instr.comment {
                            format!("{}  ; {}", instr.mnemonic, comment)
                        } else {
                            instr.mnemonic.clone()
                        };

                        let mnem_text = if is_current {
                            egui::RichText::new(&mnem_with_comment).monospace().strong()
                        } else {
                            egui::RichText::new(&mnem_with_comment).monospace()
                        };
                        ui.label(mnem_text);
                    });

                    ui.end_row();
                }
            });
    }

    fn render_memory_panel(&mut self, ui: &mut Ui, state: &EnhancedDebugState) {
        if state.memory_regions.is_empty() {
            ui.label(egui::RichText::new("No memory regions defined").weak());
            return;
        }

        // Dropdown for memory region selection
        ui.horizontal(|ui| {
            ui.label("Region:");
            egui::ComboBox::from_id_salt("memory_region_selector")
                .selected_text(
                    state
                        .memory_regions
                        .get(self.selected_memory_region_index)
                        .map(|r| r.name.as_str())
                        .unwrap_or("Select region"),
                )
                .show_ui(ui, |ui| {
                    for (idx, region) in state.memory_regions.iter().enumerate() {
                        if ui
                            .selectable_value(
                                &mut self.selected_memory_region_index,
                                idx,
                                &region.name,
                            )
                            .clicked()
                        {
                            // Reset view address to region start when changing regions
                            self.memory_view_address = region.start;
                        }
                    }
                });
        });

        ui.separator();

        // Show selected region info and hex viewer
        if let Some(region) = state.memory_regions.get(self.selected_memory_region_index) {
            // Region info
            ui.horizontal(|ui| {
                ui.label("Range:");
                ui.label(
                    egui::RichText::new(format!("${:04X} - ${:04X}", region.start, region.end))
                        .monospace(),
                );
            });
            ui.horizontal(|ui| {
                ui.label("Size:");
                ui.label(egui::RichText::new(format!("{} bytes", region.size())).monospace());
            });
            ui.horizontal(|ui| {
                ui.label("Access:");
                let access = match (region.readable, region.writable) {
                    (true, true) => "Read/Write",
                    (true, false) => "Read-only",
                    (false, true) => "Write-only",
                    (false, false) => "No access",
                };
                ui.label(egui::RichText::new(access).monospace());
            });

            ui.add_space(5.0);
            ui.separator();

            // Address navigation
            ui.horizontal(|ui| {
                ui.label("Address:");
                let mut addr_input = format!("{:04X}", self.memory_view_address);
                if ui.text_edit_singleline(&mut addr_input).changed() {
                    if let Ok(addr) = u32::from_str_radix(&addr_input, 16) {
                        self.memory_view_address = addr.clamp(region.start, region.end);
                    }
                }
                if ui.button("⬆").clicked() && self.memory_view_address >= region.start + 16 {
                    self.memory_view_address -= 16;
                }
                if ui.button("⬇").clicked() && self.memory_view_address + 16 <= region.end {
                    self.memory_view_address += 16;
                }
            });

            ui.add_space(5.0);

            // Hex viewer placeholder (to be filled with actual memory data)
            ui.label(egui::RichText::new("Memory Viewer:").strong());
            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    self.render_hex_dump(ui, region);
                });
        }
    }

    fn render_hex_dump(&self, ui: &mut Ui, _region: &MemoryRegion) {
        // Display hex dump - note: actual memory reading would be done in main.rs
        // For now, show placeholder
        ui.label(
            egui::RichText::new(format!("Memory view at ${:04X}", self.memory_view_address))
                .monospace()
                .weak(),
        );
        ui.label(
            egui::RichText::new("(Memory content to be populated from debugger)")
                .weak()
                .italics(),
        );

        // Show sample hex dump format
        ui.add_space(5.0);
        ui.label(egui::RichText::new("Format:").weak());
        ui.label(
            egui::RichText::new("ADDR: 00 01 02 03 04 05 06 07 08 09 0A 0B 0C 0D 0E 0F  ASCII")
                .monospace()
                .weak(),
        );
    }

    fn render_cpu_state_panel(&self, ui: &mut Ui, state: &EnhancedDebugState) {
        if let Some(ref cpu_state) = state.cpu_state {
            // Program Counter
            ui.heading("Program Counter");
            ui.label(
                egui::RichText::new(format!("${:04X}", cpu_state.pc))
                    .monospace()
                    .strong(),
            );
            ui.add_space(10.0);

            // Registers
            ui.heading("Registers");
            egui::Grid::new("registers_grid")
                .num_columns(2)
                .spacing([15.0, 5.0])
                .striped(true)
                .show(ui, |ui| {
                    for reg in &cpu_state.registers {
                        ui.label(egui::RichText::new(&reg.name).strong());
                        let value_str = match reg.width {
                            8 => format!("${:02X}", reg.value),
                            16 => format!("${:04X}", reg.value),
                            32 => format!("${:08X}", reg.value),
                            _ => format!("${:X}", reg.value),
                        };
                        ui.label(egui::RichText::new(value_str).monospace());
                        ui.end_row();
                    }
                });

            ui.add_space(10.0);

            // Flags
            ui.heading("Flags");
            egui::Grid::new("flags_grid")
                .num_columns(2)
                .spacing([15.0, 5.0])
                .striped(true)
                .show(ui, |ui| {
                    for (flag_name, flag_value) in &cpu_state.flags.flags {
                        ui.label(egui::RichText::new(flag_name).strong());
                        let flag_str = if *flag_value { "1" } else { "0" };
                        let flag_color = if *flag_value {
                            egui::Color32::from_rgb(100, 255, 100)
                        } else {
                            egui::Color32::from_rgb(150, 150, 150)
                        };
                        ui.label(egui::RichText::new(flag_str).monospace().color(flag_color));
                        ui.end_row();
                    }
                });
        } else {
            ui.label(egui::RichText::new("No CPU state available").weak());
        }
    }

    fn render_legacy_debug_view(&self, ui: &mut Ui, debug_info: &SystemDebugInfo) {
        ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(10.0);
                    ui.heading(
                        egui::RichText::new(format!(
                            "🔧 {} Debug Information",
                            debug_info.system_type
                        ))
                        .size(24.0)
                        .strong(),
                    );
                    ui.add_space(5.0);
                    ui.label(
                        egui::RichText::new("System internals and diagnostic information").weak(),
                    );
                });

                ui.add_space(15.0);
                ui.separator();
                ui.add_space(10.0);

                egui::Grid::new("debug_grid")
                    .num_columns(2)
                    .spacing([40.0, 8.0])
                    .striped(true)
                    .show(ui, |ui| {
                        for (label, value) in &debug_info.fields {
                            ui.label(egui::RichText::new(label).strong());
                            ui.label(egui::RichText::new(value).monospace());
                            ui.end_row();
                        }
                    });
            });
    }

    fn render_pc_config_tab(&self, ui: &mut Ui) {
        ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                if let Some(ref config) = self.pc_config_info {
                    ui.heading("PC System Configuration");
                    ui.separator();

                    egui::Grid::new("pc_config_grid")
                        .num_columns(2)
                        .spacing([40.0, 8.0])
                        .striped(true)
                        .show(ui, |ui| {
                            ui.label("CPU Model:");
                            ui.label(&config.cpu_model);
                            ui.end_row();

                            ui.label("Memory:");
                            ui.label(format!("{} KB", config.memory_kb));
                            ui.end_row();

                            ui.label("Video Adapter:");
                            ui.label(&config.video_adapter);
                            ui.end_row();

                            ui.label("Boot Priority:");
                            ui.label(&config.boot_priority);
                            ui.end_row();
                        });

                    ui.add_space(10.0);
                    ui.heading("Mounted Devices");
                    ui.separator();

                    egui::Grid::new("pc_mounts_grid")
                        .num_columns(2)
                        .spacing([40.0, 8.0])
                        .striped(true)
                        .show(ui, |ui| {
                            ui.label("BIOS:");
                            ui.label(if config.bios_mounted {
                                "✓ Mounted"
                            } else {
                                "✗ Not mounted"
                            });
                            ui.end_row();

                            ui.label("Floppy A:");
                            ui.label(if config.floppy_a_mounted {
                                "✓ Mounted"
                            } else {
                                "✗ Not mounted"
                            });
                            ui.end_row();

                            ui.label("Floppy B:");
                            ui.label(if config.floppy_b_mounted {
                                "✓ Mounted"
                            } else {
                                "✗ Not mounted"
                            });
                            ui.end_row();

                            ui.label("Hard Drive:");
                            ui.label(if config.hdd_mounted {
                                "✓ Mounted"
                            } else {
                                "✗ Not mounted"
                            });
                            ui.end_row();
                        });

                    ui.add_space(10.0);
                    ui.label("This tab shows PC-specific configuration and mounted devices.");
                    ui.label("Use the Mount Points panel to manage disk images.");
                } else {
                    ui.label("No PC configuration available");
                    ui.label("This tab is only available when a PC system is loaded");
                }
            });
    }

    fn render_about_tab(&self, ui: &mut Ui) {
        ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(20.0);
                    ui.heading(egui::RichText::new("🎮 Hemulator").size(36.0).strong());
                    ui.add_space(5.0);
                    ui.label(
                        egui::RichText::new("Multi-System Console Emulator")
                            .size(16.0)
                            .italics(),
                    );
                    ui.add_space(3.0);
                    ui.label(
                        egui::RichText::new(format!("Version {}", APP_VERSION))
                            .size(14.0)
                            .weak(),
                    );
                    ui.add_space(20.0);
                });

                ui.separator();
                ui.add_space(10.0);

                // About section
                ui.heading(egui::RichText::new("📖 About").strong());
                ui.add_space(5.0);
                ui.label("A cross-platform, multi-system console emulator written in Rust,");
                ui.label("supporting NES, Atari 2600, Game Boy, SNES, N64, and PC emulation");
                ui.label("with comprehensive save state management and customizable controls.");
                ui.add_space(10.0);

                // Supported Systems
                ui.heading(egui::RichText::new("🖥️ Supported Systems").strong());
                ui.add_space(5.0);
                egui::Grid::new("systems_grid")
                    .num_columns(2)
                    .spacing([10.0, 5.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label("✅ NES");
                        ui.label("Nintendo Entertainment System - Fully working");
                        ui.end_row();

                        ui.label("⚠️ PC");
                        ui.label("IBM PC/XT - Functional");
                        ui.end_row();

                        ui.label("🚧 Atari 2600");
                        ui.label("In development");
                        ui.end_row();

                        ui.label("🚧 Game Boy");
                        ui.label("Game Boy / Game Boy Color - In development");
                        ui.end_row();

                        ui.label("🚧 SNES");
                        ui.label("Super Nintendo - In development");
                        ui.end_row();

                        ui.label("🚧 N64");
                        ui.label("Nintendo 64 - In development");
                        ui.end_row();
                    });
                ui.add_space(10.0);

                // Features
                ui.heading(egui::RichText::new("✨ Features").strong());
                ui.add_space(5.0);
                ui.label("💾 Save States - 5 slots per game with instant save/load");
                ui.label("⚙️ Persistent Settings - Customizable controls and window scaling");
                ui.label("🎨 CRT Filters - Hardware-accelerated shader-based effects");
                ui.label("🎵 Audio Support - Integrated audio playback via rodio");
                ui.label("📁 ROM Auto-Detection - Automatic format detection");
                ui.label("🖱️ Modern GUI - Menu bar and status bar with mouse support");
                ui.add_space(10.0);

                // License
                ui.heading(egui::RichText::new("📜 License").strong());
                ui.add_space(5.0);
                ui.label("MIT License - Copyright (c) 2025");
                ui.add_space(10.0);

                // Links
                ui.heading(egui::RichText::new("🔗 Links").strong());
                ui.add_space(5.0);
                ui.horizontal(|ui| {
                    ui.label("GitHub:");
                    ui.hyperlink_to(
                        "github.com/Hexagon/hemulator",
                        "https://github.com/Hexagon/hemulator",
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("📚 User Manual:");
                    ui.hyperlink_to(
                        "MANUAL.md",
                        "https://github.com/Hexagon/hemulator/blob/main/docs/MANUAL.md",
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("📖 Documentation:");
                    ui.hyperlink_to(
                        "README.md",
                        "https://github.com/Hexagon/hemulator/blob/main/README.md",
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("📄 License:");
                    ui.hyperlink_to(
                        "MIT License",
                        "https://github.com/Hexagon/hemulator/blob/main/LICENSE",
                    );
                });
            });
    }
}

impl Default for TabManager {
    fn default() -> Self {
        Self::new()
    }
}
