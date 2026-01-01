//! Tab manager for left panel

use crate::system_adapter::SystemDebugInfo;
use egui::{ScrollArea, TextureHandle, Ui};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Emulator,
    Log,
    Help,
    Debug,
}

pub struct TabManager {
    pub active_tab: Tab,
    pub log_messages: Vec<String>,
    pub help_visible: bool,
    pub debug_visible: bool,
    pub debug_info: Option<SystemDebugInfo>,
}

impl TabManager {
    pub fn new() -> Self {
        Self {
            active_tab: Tab::Emulator,
            log_messages: Vec::new(),
            help_visible: false,
            debug_visible: false,
            debug_info: None,
        }
    }

    pub fn add_log(&mut self, message: String) {
        self.log_messages.push(message);
        // Keep only last 1000 messages
        if self.log_messages.len() > 1000 {
            self.log_messages.remove(0);
        }
    }

    pub fn show_help_tab(&mut self) {
        self.help_visible = true;
        self.active_tab = Tab::Help;
    }

    pub fn show_debug_tab(&mut self) {
        self.debug_visible = true;
        self.active_tab = Tab::Debug;
    }

    pub fn update_debug_info(&mut self, info: SystemDebugInfo) {
        self.debug_info = Some(info);
    }

    pub fn ui(&mut self, ui: &mut Ui, emulator_texture: &Option<TextureHandle>) {
        // Tab bar
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.active_tab, Tab::Emulator, "Emulator");

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
                if ui.button("✖").on_hover_text("Close Help tab").clicked() {
                    self.help_visible = false;
                    if self.active_tab == Tab::Help {
                        self.active_tab = Tab::Emulator;
                    }
                }
            }

            if self.debug_visible {
                ui.selectable_value(&mut self.active_tab, Tab::Debug, "Debug");
                if ui.button("✖").on_hover_text("Close Debug tab").clicked() {
                    self.debug_visible = false;
                    if self.active_tab == Tab::Debug {
                        self.active_tab = Tab::Emulator;
                    }
                }
            }
        });

        ui.separator();

        // Tab content
        match self.active_tab {
            Tab::Emulator => self.render_emulator_tab(ui, emulator_texture),
            Tab::Log => self.render_log_tab(ui),
            Tab::Help => self.render_help_tab(ui),
            Tab::Debug => self.render_debug_tab(ui),
        }
    }

    fn render_emulator_tab(&self, ui: &mut Ui, emulator_texture: &Option<TextureHandle>) {
        ui.centered_and_justified(|ui| {
            if let Some(texture) = emulator_texture {
                // Display the emulator frame, maintaining aspect ratio
                let available_size = ui.available_size();
                let texture_size = texture.size_vec2();
                let aspect_ratio = texture_size.x / texture_size.y;

                let display_width = available_size.x.min(available_size.y * aspect_ratio);
                let display_height = display_width / aspect_ratio;

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
}

impl Default for TabManager {
    fn default() -> Self {
        Self::new()
    }
}
