//! Bottom status bar

use egui::Ui;

pub struct StatusBarWidget {
    pub message: String,
    pub fps: f32,
    pub show_fps: bool,
    pub show_shortcuts: bool,
}

impl StatusBarWidget {
    pub fn new() -> Self {
        Self {
            message: String::new(),
            fps: 0.0,
            show_fps: true,
            show_shortcuts: true,
        }
    }

    pub fn set_message(&mut self, msg: String) {
        self.message = msg;
    }

    pub fn set_fps(&mut self, fps: f32) {
        self.fps = fps;
    }

    pub fn ui(&self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            // Left side - status message
            if !self.message.is_empty() {
                ui.label(&self.message);
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
