//! Bottom status bar

use egui::Ui;

pub struct StatusBarWidget {
    pub message: String,
}

impl StatusBarWidget {
    pub fn new() -> Self {
        Self {
            message: String::new(),
        }
    }

    pub fn set_message(&mut self, msg: String) {
        self.message = msg;
    }

    pub fn ui(&self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            if !self.message.is_empty() {
                ui.label(&self.message);
            } else {
                ui.label("Ready");
            }
        });
    }
}

impl Default for StatusBarWidget {
    fn default() -> Self {
        Self::new()
    }
}
