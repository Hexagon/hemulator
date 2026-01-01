//! Top menu bar

use egui::Ui;

/// Actions that can be triggered from the menu
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuAction {
    // File menu
    NewProject,
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
    ScalingOriginal,
    ScalingFit,
    ScalingStretch,
    Fullscreen,
    FullscreenWithGui,
    ShowLog,
    ShowDebug,

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
        ui.horizontal(|ui| {
            // File menu
            ui.menu_button("File", |ui| {
                if ui.button("New Project...").clicked() {
                    self.pending_action = Some(MenuAction::NewProject);
                    ui.close();
                }
                ui.separator();
                if ui.button("Open ROM... (F3)").clicked() {
                    self.pending_action = Some(MenuAction::OpenRom);
                    ui.close();
                }
                if ui.button("Open Project...").clicked() {
                    self.pending_action = Some(MenuAction::OpenProject);
                    ui.close();
                }
                if ui.button("Save Project...").clicked() {
                    self.pending_action = Some(MenuAction::SaveProject);
                    ui.close();
                }
                ui.separator();
                if ui.button("Exit (ESC)").clicked() {
                    self.pending_action = Some(MenuAction::Exit);
                    ui.close();
                }
            });

            // Emulation menu
            ui.menu_button("Emulation", |ui| {
                if ui.button("Reset (F2)").clicked() {
                    self.pending_action = Some(MenuAction::Reset);
                    ui.close();
                }
                if ui.button("Pause (P)").clicked() {
                    self.pending_action = Some(MenuAction::Pause);
                    ui.close();
                }
                if ui.button("Resume").clicked() {
                    self.pending_action = Some(MenuAction::Resume);
                    ui.close();
                }
            });

            // View menu
            ui.menu_button("View", |ui| {
                ui.menu_button("Scaling", |ui| {
                    if ui.button("Original").clicked() {
                        self.pending_action = Some(MenuAction::ScalingOriginal);
                        ui.close();
                    }
                    if ui.button("Fit").clicked() {
                        self.pending_action = Some(MenuAction::ScalingFit);
                        ui.close();
                    }
                    if ui.button("Stretch").clicked() {
                        self.pending_action = Some(MenuAction::ScalingStretch);
                        ui.close();
                    }
                });

                ui.separator();

                if ui.button("Fullscreen").clicked() {
                    self.pending_action = Some(MenuAction::Fullscreen);
                    ui.close();
                }
                if ui.button("Fullscreen (With GUI)").clicked() {
                    self.pending_action = Some(MenuAction::FullscreenWithGui);
                    ui.close();
                }

                ui.separator();

                if ui.button("Log").clicked() {
                    self.pending_action = Some(MenuAction::ShowLog);
                    ui.close();
                }
                if ui.button("Debug").clicked() {
                    self.pending_action = Some(MenuAction::ShowDebug);
                    ui.close();
                }

                ui.separator();

                if ui.button("Screenshot (F4)").clicked() {
                    self.pending_action = Some(MenuAction::Screenshot);
                    ui.close();
                }
            });

            // Help menu
            ui.menu_button("Help", |ui| {
                if ui.button("Controls & Help").clicked() {
                    self.pending_action = Some(MenuAction::ShowHelp);
                    ui.close();
                }
                ui.separator();
                if ui.button("About").clicked() {
                    self.pending_action = Some(MenuAction::About);
                    ui.close();
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
