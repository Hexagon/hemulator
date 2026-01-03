//! Bottom status bar

use egui::Ui;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageType {
    Info,
    Success,
    Warning,
    Error,
}

pub struct StatusBarWidget {
    pub message: String,
    pub message_type: MessageType,
    pub fps: f32,
    pub show_fps: bool,
    pub show_shortcuts: bool,
}

impl StatusBarWidget {
    pub fn new() -> Self {
        Self {
            message: String::new(),
            message_type: MessageType::Info,
            fps: 0.0,
            show_fps: true,
            show_shortcuts: true,
        }
    }

    pub fn set_message(&mut self, msg: String) {
        self.message = msg;
        self.message_type = MessageType::Info;
    }

    pub fn set_info(&mut self, msg: String) {
        self.message = msg;
        self.message_type = MessageType::Info;
    }

    pub fn set_success(&mut self, msg: String) {
        self.message = msg;
        self.message_type = MessageType::Success;
    }

    pub fn set_warning(&mut self, msg: String) {
        self.message = msg;
        self.message_type = MessageType::Warning;
    }

    pub fn set_error(&mut self, msg: String) {
        self.message = msg;
        self.message_type = MessageType::Error;
    }

    pub fn set_fps(&mut self, fps: f32) {
        self.fps = fps;
    }

    pub fn ui(&self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            // Left side - status message with color coding
            if !self.message.is_empty() {
                let (icon, color) = match self.message_type {
                    MessageType::Info => ("ℹ️", egui::Color32::from_rgb(180, 180, 180)),
                    MessageType::Success => ("✅", egui::Color32::from_rgb(0, 200, 0)),
                    MessageType::Warning => ("⚠️", egui::Color32::from_rgb(255, 180, 0)),
                    MessageType::Error => ("❌", egui::Color32::from_rgb(255, 80, 80)),
                };
                ui.label(icon);
                ui.colored_label(color, &self.message);
            } else {
                ui.label("Ready");
            }

            // Spacer to push content to the right
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Right side - FPS and keyboard shortcuts
                if self.show_fps && self.fps > 0.0 {
                    ui.label(format!("FPS: {:.1}", self.fps));
                    ui.separator();
                }

                if self.show_shortcuts {
                    ui.label("F3: Open ROM");
                    ui.separator();
                    ui.label("F2: Reset");
                    ui.separator();
                    ui.label("P: Pause");
                    ui.separator();
                    ui.label("F11: Fullscreen");
                }
            });
        });
    }
}

impl Default for StatusBarWidget {
    fn default() -> Self {
        Self::new()
    }
}
