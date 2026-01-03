//! Top menu bar

use egui::Ui;

/// Actions that can be triggered from the menu
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuAction {
    // File menu
    NewProject,
    OpenRom,
    OpenRecentFile(String), // Path to recent file
    ClearRecentFiles,
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
    pub recent_files: Vec<String>, // List of recent files to display
}

impl MenuBar {
    pub fn new() -> Self {
        Self {
            pending_action: None,
            recent_files: Vec::new(),
        }
    }

    /// Update the recent files list
    pub fn set_recent_files(&mut self, files: Vec<String>) {
        self.recent_files = files;
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            // File menu
            ui.menu_button("📁 File", |ui| {
                if ui
                    .button("➕ New Project...")
                    .on_hover_text("Create a new emulator system")
                    .clicked()
                {
                    self.pending_action = Some(MenuAction::NewProject);
                    ui.close();
                }
                ui.separator();
                if ui
                    .button("📂 Open ROM... (F3)")
                    .on_hover_text("Load a game ROM or disk image")
                    .clicked()
                {
                    self.pending_action = Some(MenuAction::OpenRom);
                    ui.close();
                }

                // Recent Files submenu
                ui.menu_button("🕒 Recent Files", |ui| {
                    if self.recent_files.is_empty() {
                        ui.label(egui::RichText::new("No recent files").weak());
                    } else {
                        for file_path in &self.recent_files.clone() {
                            // Extract filename from path
                            let display_name = std::path::Path::new(file_path)
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or(file_path);

                            if ui.button(display_name).on_hover_text(file_path).clicked() {
                                self.pending_action =
                                    Some(MenuAction::OpenRecentFile(file_path.clone()));
                                ui.close();
                            }
                        }
                        ui.separator();
                        if ui
                            .button("🗑️ Clear Recent Files")
                            .on_hover_text("Remove all recent files from the list")
                            .clicked()
                        {
                            self.pending_action = Some(MenuAction::ClearRecentFiles);
                            ui.close();
                        }
                    }
                });

                ui.separator();
                if ui
                    .button("📁 Open Project...")
                    .on_hover_text("Load a saved .hemu project file")
                    .clicked()
                {
                    self.pending_action = Some(MenuAction::OpenProject);
                    ui.close();
                }
                if ui
                    .button("💾 Save Project...")
                    .on_hover_text("Save current system configuration to a .hemu project file")
                    .clicked()
                {
                    self.pending_action = Some(MenuAction::SaveProject);
                    ui.close();
                }
                ui.separator();
                if ui
                    .button("🚪 Exit (ESC)")
                    .on_hover_text("Quit the emulator")
                    .clicked()
                {
                    self.pending_action = Some(MenuAction::Exit);
                    ui.close();
                }
            });

            // Emulation menu
            ui.menu_button("🎮 Emulation", |ui| {
                if ui
                    .button("🔄 Reset (F2)")
                    .on_hover_text("Reset the emulated system")
                    .clicked()
                {
                    self.pending_action = Some(MenuAction::Reset);
                    ui.close();
                }
                if ui
                    .button("⏸️ Pause (P)")
                    .on_hover_text("Pause emulation")
                    .clicked()
                {
                    self.pending_action = Some(MenuAction::Pause);
                    ui.close();
                }
                if ui
                    .button("▶️ Resume")
                    .on_hover_text("Resume emulation")
                    .clicked()
                {
                    self.pending_action = Some(MenuAction::Resume);
                    ui.close();
                }
            });

            // View menu
            ui.menu_button("👁️ View", |ui| {
                ui.menu_button("🔍 Scaling", |ui| {
                    if ui
                        .button("1️⃣ Original")
                        .on_hover_text("1:1 pixel mapping, no scaling")
                        .clicked()
                    {
                        self.pending_action = Some(MenuAction::ScalingOriginal);
                        ui.close();
                    }
                    if ui
                        .button("📐 Fit")
                        .on_hover_text("Scale to fit window, maintain aspect ratio")
                        .clicked()
                    {
                        self.pending_action = Some(MenuAction::ScalingFit);
                        ui.close();
                    }
                    if ui
                        .button("⬛ Stretch")
                        .on_hover_text("Stretch to fill window, ignore aspect ratio")
                        .clicked()
                    {
                        self.pending_action = Some(MenuAction::ScalingStretch);
                        ui.close();
                    }
                });

                ui.separator();

                if ui
                    .button("🖼️ Fullscreen (F11)")
                    .on_hover_text("Toggle fullscreen mode without GUI")
                    .clicked()
                {
                    self.pending_action = Some(MenuAction::Fullscreen);
                    ui.close();
                }
                if ui
                    .button("🖥️ Fullscreen with GUI (Host+F11)")
                    .on_hover_text("Toggle fullscreen mode with GUI visible")
                    .clicked()
                {
                    self.pending_action = Some(MenuAction::FullscreenWithGui);
                    ui.close();
                }

                ui.separator();

                if ui
                    .button("📋 Log")
                    .on_hover_text("Show emulation log messages")
                    .clicked()
                {
                    self.pending_action = Some(MenuAction::ShowLog);
                    ui.close();
                }
                if ui
                    .button("🔧 Debug")
                    .on_hover_text("Show system debug information")
                    .clicked()
                {
                    self.pending_action = Some(MenuAction::ShowDebug);
                    ui.close();
                }

                ui.separator();

                if ui
                    .button("📸 Screenshot (F4)")
                    .on_hover_text("Save a screenshot of the current frame")
                    .clicked()
                {
                    self.pending_action = Some(MenuAction::Screenshot);
                    ui.close();
                }
            });

            // Help menu
            ui.menu_button("❓ Help", |ui| {
                if ui
                    .button("⌨️ Controls & Help")
                    .on_hover_text("View keyboard controls and usage instructions")
                    .clicked()
                {
                    self.pending_action = Some(MenuAction::ShowHelp);
                    ui.close();
                }
                ui.separator();
                if ui
                    .button("ℹ️ About")
                    .on_hover_text("About Hemulator")
                    .clicked()
                {
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
