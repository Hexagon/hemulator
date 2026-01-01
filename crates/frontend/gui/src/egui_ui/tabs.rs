//! Tab manager for left panel

use crate::settings::ScalingMode;
use crate::system_adapter::SystemDebugInfo;
use egui::{ScrollArea, TextureHandle, Ui};

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
    pub new_project_visible: bool,
    pub selected_system: String,
    pub pending_action: Option<TabAction>,
    pub pc_config_info: Option<PcConfigInfo>,
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
            new_project_visible: false,
            selected_system: "NES".to_string(),
            pending_action: None,
            pc_config_info: None,
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

    /// Get and clear any pending action
    pub fn take_action(&mut self) -> Option<TabAction> {
        self.pending_action.take()
    }

    pub fn ui(
        &mut self,
        ui: &mut Ui,
        emulator_texture: &Option<TextureHandle>,
        scaling_mode: ScalingMode,
    ) {
        // Tab bar
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.active_tab, Tab::Emulator, "Emulator");

            if self.new_project_visible {
                ui.selectable_value(&mut self.active_tab, Tab::NewProject, "New Project");
                // Use a colored button for the close icon to ensure visibility
                let close_button = egui::Button::new(
                    egui::RichText::new("✖").color(egui::Color32::from_rgb(220, 220, 220)),
                );
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

            if self.log_messages.is_empty() {
                ui.add_enabled(false, egui::Button::new("Log"));
            } else {
                ui.selectable_value(
                    &mut self.active_tab,
                    Tab::Log,
                    format!("Log ({})", self.log_messages.len()),
                );
            }

            if self.help_visible {
                ui.selectable_value(&mut self.active_tab, Tab::Help, "Help");
                // Use a colored button for the close icon to ensure visibility
                let close_button = egui::Button::new(
                    egui::RichText::new("✖").color(egui::Color32::from_rgb(220, 220, 220)),
                );
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
                ui.selectable_value(&mut self.active_tab, Tab::Debug, "Debug");
                // Use a colored button for the close icon to ensure visibility
                let close_button = egui::Button::new(
                    egui::RichText::new("✖").color(egui::Color32::from_rgb(220, 220, 220)),
                );
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
                ui.selectable_value(&mut self.active_tab, Tab::About, "About");
                // Use a colored button for the close icon to ensure visibility
                let close_button = egui::Button::new(
                    egui::RichText::new("✖").color(egui::Color32::from_rgb(220, 220, 220)),
                );
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
                ui.label("No emulator output");
            }
        });
    }

    fn render_log_tab(&self, ui: &mut Ui) {
        ScrollArea::vertical()
            .auto_shrink([false; 2])
            .stick_to_bottom(true)
            .show(ui, |ui| {
                for msg in &self.log_messages {
                    ui.label(msg);
                }
                if self.log_messages.is_empty() {
                    ui.label("No log messages");
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
                ui.heading("Hemulator - Controls & Help");
                ui.separator();

                ui.heading("File Operations");
                ui.label("F3 - Open ROM");
                ui.label("ESC - Exit");
                ui.add_space(10.0);

                ui.heading("Emulation");
                ui.label("F2 - Reset");
                ui.label("P - Pause/Resume");
                ui.add_space(10.0);

                ui.heading("Save States");
                ui.label("F5-F9 - Save to slots 1-5");
                ui.label("Shift+F5-F9 - Load from slots 1-5");
                ui.add_space(10.0);

                ui.heading("View");
                ui.label("F4 - Screenshot");
                ui.label("F1 - Toggle Debug Info");
                ui.add_space(10.0);

                ui.heading("Default Controls");
                ui.label("Z - A Button");
                ui.label("X - B Button");
                ui.label("Enter - Start");
                ui.label("Shift - Select");
                ui.label("Arrow Keys - D-Pad");
                ui.add_space(10.0);

                ui.label("For system-specific information and additional controls,");
                ui.label("see the manual at docs/MANUAL.md");
            });
    }

    fn render_debug_tab(&self, ui: &mut Ui) {
        ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                if let Some(ref debug_info) = self.debug_info {
                    ui.heading(format!("{} Debug Information", debug_info.system_type));
                    ui.separator();

                    egui::Grid::new("debug_grid")
                        .num_columns(2)
                        .spacing([40.0, 4.0])
                        .striped(true)
                        .show(ui, |ui| {
                            for (label, value) in &debug_info.fields {
                                ui.label(label);
                                ui.label(value);
                                ui.end_row();
                            }
                        });
                } else {
                    ui.label("No debug information available");
                    ui.label("Load a ROM to see system-specific debug info");
                }
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
                    ui.heading(egui::RichText::new("Hemulator").size(32.0));
                    ui.add_space(5.0);
                    ui.label(egui::RichText::new("Multi-System Console Emulator").size(16.0));
                    ui.add_space(20.0);
                });

                ui.separator();
                ui.add_space(10.0);

                ui.heading("Version Information");
                ui.label(format!("Version: {}", APP_VERSION));
                ui.add_space(10.0);

                ui.heading("License");
                ui.label("MIT License");
                ui.label("Copyright (c) 2025");
                ui.add_space(10.0);

                ui.heading("About");
                ui.label("A cross-platform, multi-system console emulator written in Rust,");
                ui.label("supporting NES, Atari 2600, Game Boy, SNES, N64, and PC emulation");
                ui.label("with comprehensive save state management and customizable controls.");
                ui.add_space(10.0);

                ui.heading("Supported Systems");
                ui.label("✅ NES (Nintendo Entertainment System) - Fully working");
                ui.label("⚠️ PC (IBM PC/XT) - Functional");
                ui.label("🚧 Atari 2600 - In development");
                ui.label("🚧 Game Boy / Game Boy Color - In development");
                ui.label("🚧 SNES (Super Nintendo) - In development");
                ui.label("🚧 N64 (Nintendo 64) - In development");
                ui.add_space(10.0);

                ui.heading("Links");
                ui.horizontal(|ui| {
                    ui.label("GitHub:");
                    ui.hyperlink_to(
                        "github.com/Hexagon/hemulator",
                        "https://github.com/Hexagon/hemulator",
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("README:");
                    ui.hyperlink_to(
                        "Documentation",
                        "https://github.com/Hexagon/hemulator/blob/main/README.md",
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("User Manual:");
                    ui.hyperlink_to(
                        "MANUAL.md",
                        "https://github.com/Hexagon/hemulator/blob/main/docs/MANUAL.md",
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("License:");
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
