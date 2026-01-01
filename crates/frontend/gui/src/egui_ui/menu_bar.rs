//! Top menu bar

use egui::Ui;

/// Actions that can be triggered from the menu
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuAction {
    // File menu
    OpenRom,
    OpenProject,
    SaveProject,
    Exit,

    // Emulation menu
    Reset,
    Pause,
    Resume,

    // View menu
    Screenshot,
    
    // Help menu
    ShowHelp,
    About,
}

pub struct MenuBar {
    pub pending_action: Option<MenuAction>,
}

impl MenuBar {
    pub fn new() -> Self {
        Self {
            pending_action: None,
        }
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        egui::menu::bar(ui, |ui| {
            // File menu
            ui.menu_button("File", |ui| {
                if ui.button("Open ROM... (F3)").clicked() {
                    self.pending_action = Some(MenuAction::OpenRom);
                    ui.close_menu();
                }
                if ui.button("Open Project...").clicked() {
                    self.pending_action = Some(MenuAction::OpenProject);
                    ui.close_menu();
                }
                if ui.button("Save Project...").clicked() {
                    self.pending_action = Some(MenuAction::SaveProject);
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Exit (ESC)").clicked() {
                    self.pending_action = Some(MenuAction::Exit);
                    ui.close_menu();
                }
            });

            // Emulation menu
            ui.menu_button("Emulation", |ui| {
                if ui.button("Reset (F2)").clicked() {
                    self.pending_action = Some(MenuAction::Reset);
                    ui.close_menu();
                }
                if ui.button("Pause (P)").clicked() {
                    self.pending_action = Some(MenuAction::Pause);
                    ui.close_menu();
                }
                if ui.button("Resume").clicked() {
                    self.pending_action = Some(MenuAction::Resume);
                    ui.close_menu();
                }
            });

            // View menu
            ui.menu_button("View", |ui| {
                if ui.button("Screenshot (F4)").clicked() {
                    self.pending_action = Some(MenuAction::Screenshot);
                    ui.close_menu();
                }
            });

            // Help menu
            ui.menu_button("Help", |ui| {
                if ui.button("Controls & Help").clicked() {
                    self.pending_action = Some(MenuAction::ShowHelp);
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("About").clicked() {
                    self.pending_action = Some(MenuAction::About);
                    ui.close_menu();
                }
            });
        });
    }

    /// Get and clear any pending action
    pub fn take_action(&mut self) -> Option<MenuAction> {
        self.pending_action.take()
    }
}

impl Default for MenuBar {
    fn default() -> Self {
        Self::new()
    }
}
