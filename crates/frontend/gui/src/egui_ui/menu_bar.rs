//! Top menu bar

use crate::display_filter::DisplayFilter;
use egui::Ui;

/// Actions that can be triggered from the menu
#[derive(Debug, Clone, PartialEq)]
pub enum MenuAction {
    // File menu
    NewProjectSystem(String), // String is the system name (e.g., "NES", "Game Boy", "PC")
    NewProjectAutoDetect,     // Auto-detect system from ROM file
    OpenRecentFile(String),   // Path to recent file
    ClearRecentFiles,
    OpenProject,
    SaveProject,
    Exit,

    // Emulation menu
    Reset,
    Pause,
    Resume,
    Step,
    SaveState(u8),  // Slot 1-5
    LoadState(u8),  // Slot 1-5
    SetSpeed(i32),  // Speed percentage (25, 50, 100, 200, 400)
    EjectCartridge, // Eject cartridge/ROM when single mount point

    // View menu
    Screenshot,
    ScalingOriginal,
    ScalingFit,
    ScalingStretch,
    Fullscreen,
    FullscreenWithGui,
    ShowInspector,                   // Toggle Inspector dock visibility
    SetDisplayFilter(DisplayFilter), // Set display filter
    ToggleMetrics,                   // Toggle Metrics section visibility
    ToggleControllerSettings,        // Toggle Controller Settings section visibility
    ToggleMountPoints,               // Toggle Mount Points section visibility

    // Help menu
    ShowHelp,
    About,
}

pub struct MenuBar {
    pub pending_action: Option<MenuAction>,
    pub recent_files: Vec<String>, // List of recent files to display
    pub system_loaded: bool,       // Whether a system is currently loaded
    pub rom_loaded: bool,          // Whether a ROM is mounted
    pub current_speed: i32,        // Current emulation speed percentage
    pub current_filter: DisplayFilter, // Current display filter
    pub metrics_visible: bool,     // Metrics section visibility
    pub controller_visible: bool,  // Controller section visibility
    pub mounts_visible: bool,      // Mount points section visibility
    pub single_mount_system: bool, // Whether system has only one mount point
}

impl MenuBar {
    pub fn new() -> Self {
        Self {
            pending_action: None,
            recent_files: Vec::new(),
            system_loaded: false,
            rom_loaded: false,
            current_speed: 100,
            current_filter: DisplayFilter::None,
            metrics_visible: false,
            controller_visible: false,
            mounts_visible: false,
            single_mount_system: false,
        }
    }

    /// Update the recent files list
    pub fn set_recent_files(&mut self, files: Vec<String>) {
        self.recent_files = files;
    }

    /// Update whether a system is loaded
    pub fn set_system_loaded(&mut self, loaded: bool) {
        self.system_loaded = loaded;
    }

    /// Update whether a ROM is mounted
    pub fn set_rom_loaded(&mut self, loaded: bool) {
        self.rom_loaded = loaded;
    }

    /// Update single mount system state
    pub fn set_single_mount_system(&mut self, single: bool) {
        self.single_mount_system = single;
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            // File menu
            ui.menu_button("File", |ui| {
                // New Project submenu with system choices
                ui.menu_button("New Project", |ui| {
                    ui.label(
                        egui::RichText::new("Select System Type")
                            .strong()
                            .size(14.0),
                    );
                    ui.separator();

                    // Auto-detect from ROM option
                    if ui
                        .button("Auto Detect from ROM...")
                        .on_hover_text("Load a ROM and automatically detect the system type")
                        .clicked()
                    {
                        self.pending_action = Some(MenuAction::NewProjectAutoDetect);
                        ui.close();
                    }

                    ui.separator();

                    // Individual system options
                    if ui
                        .button("NES")
                        .on_hover_text("Nintendo Entertainment System")
                        .clicked()
                    {
                        self.pending_action = Some(MenuAction::NewProjectSystem("NES".to_string()));
                        ui.close();
                    }

                    if ui
                        .button("Game Boy")
                        .on_hover_text("Game Boy / Game Boy Color")
                        .clicked()
                    {
                        self.pending_action =
                            Some(MenuAction::NewProjectSystem("Game Boy".to_string()));
                        ui.close();
                    }

                    if ui.button("GBA").on_hover_text("Game Boy Advance").clicked() {
                        self.pending_action = Some(MenuAction::NewProjectSystem("GBA".to_string()));
                        ui.close();
                    }

                    if ui
                        .button("Atari 2600")
                        .on_hover_text("Atari 2600")
                        .clicked()
                    {
                        self.pending_action =
                            Some(MenuAction::NewProjectSystem("Atari 2600".to_string()));
                        ui.close();
                    }

                    if ui
                        .button("Atari 5200")
                        .on_hover_text("Atari 5200 SuperSystem")
                        .clicked()
                    {
                        self.pending_action =
                            Some(MenuAction::NewProjectSystem("Atari 5200".to_string()));
                        ui.close();
                    }

                    if ui
                        .button("SMS")
                        .on_hover_text("Sega Master System")
                        .clicked()
                    {
                        self.pending_action = Some(MenuAction::NewProjectSystem("SMS".to_string()));
                        ui.close();
                    }

                    if ui
                        .button("Game Gear")
                        .on_hover_text("Sega Game Gear")
                        .clicked()
                    {
                        self.pending_action =
                            Some(MenuAction::NewProjectSystem("Game Gear".to_string()));
                        ui.close();
                    }

                    if ui
                        .button("Mega Drive")
                        .on_hover_text("Sega Mega Drive / Genesis")
                        .clicked()
                    {
                        self.pending_action =
                            Some(MenuAction::NewProjectSystem("Mega Drive".to_string()));
                        ui.close();
                    }

                    if ui.button("C64").on_hover_text("Commodore 64").clicked() {
                        self.pending_action = Some(MenuAction::NewProjectSystem("C64".to_string()));
                        ui.close();
                    }

                    if ui
                        .button("ColecoVision")
                        .on_hover_text("ColecoVision")
                        .clicked()
                    {
                        self.pending_action =
                            Some(MenuAction::NewProjectSystem("ColecoVision".to_string()));
                        ui.close();
                    }

                    if ui.button("SG-1000").on_hover_text("Sega SG-1000").clicked() {
                        self.pending_action =
                            Some(MenuAction::NewProjectSystem("SG-1000".to_string()));
                        ui.close();
                    }

                    if ui
                        .button("CHIP-8")
                        .on_hover_text("CHIP-8 / Super-CHIP / XO-CHIP")
                        .clicked()
                    {
                        self.pending_action =
                            Some(MenuAction::NewProjectSystem("CHIP-8".to_string()));
                        ui.close();
                    }

                    if ui
                        .button("Game & Watch")
                        .on_hover_text("Nintendo Game & Watch (SM510)")
                        .clicked()
                    {
                        self.pending_action =
                            Some(MenuAction::NewProjectSystem("Game & Watch".to_string()));
                        ui.close();
                    }

                    if ui
                        .button("SNES")
                        .on_hover_text("Super Nintendo Entertainment System")
                        .clicked()
                    {
                        self.pending_action =
                            Some(MenuAction::NewProjectSystem("SNES".to_string()));
                        ui.close();
                    }

                    if ui.button("N64").on_hover_text("Nintendo 64").clicked() {
                        self.pending_action = Some(MenuAction::NewProjectSystem("N64".to_string()));
                        ui.close();
                    }

                    if ui.button("PS1").on_hover_text("Sony PlayStation").clicked() {
                        self.pending_action = Some(MenuAction::NewProjectSystem("PS1".to_string()));
                        ui.close();
                    }

                    if ui
                        .button("PC")
                        .on_hover_text("IBM PC/XT Compatible")
                        .clicked()
                    {
                        self.pending_action = Some(MenuAction::NewProjectSystem("PC".to_string()));
                        ui.close();
                    }
                });

                ui.separator();

                // Recent Files submenu
                ui.menu_button("Recent Files", |ui| {
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
                            .button("Clear Recent Files")
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
                    .button("Open Project...")
                    .on_hover_text("Load a saved .hemu project file")
                    .clicked()
                {
                    self.pending_action = Some(MenuAction::OpenProject);
                    ui.close();
                }

                ui.add_enabled_ui(self.system_loaded, |ui| {
                    if ui
                        .button("Save Project...")
                        .on_hover_text(if self.system_loaded {
                            "Save current system configuration to a .hemu project file"
                        } else {
                            "No system loaded - create a system first"
                        })
                        .clicked()
                    {
                        self.pending_action = Some(MenuAction::SaveProject);
                        ui.close();
                    }
                });

                ui.separator();
                if ui
                    .button("Exit")
                    .on_hover_text("Quit the emulator")
                    .clicked()
                {
                    self.pending_action = Some(MenuAction::Exit);
                    ui.close();
                }
            });

            // Emulation menu - only enabled when a system is loaded
            ui.add_enabled_ui(self.system_loaded, |ui| {
                ui.menu_button("Emulation", |ui| {
                    if ui
                        .button("🔄 Reset")
                        .on_hover_text("Reset the emulated system")
                        .clicked()
                    {
                        self.pending_action = Some(MenuAction::Reset);
                        ui.close();
                    }
                    if ui
                        .button("⏸️ Pause")
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
                    if ui
                        .button("⏭️ Step")
                        .on_hover_text("Step one instruction (when paused)")
                        .clicked()
                    {
                        self.pending_action = Some(MenuAction::Step);
                        ui.close();
                    }

                    ui.separator();

                    // Eject cartridge - only shown if single mount system and ROM loaded
                    ui.add_enabled_ui(self.rom_loaded && self.single_mount_system, |ui| {
                        if ui
                            .button("Eject Cartridge")
                            .on_hover_text("Eject the current cartridge/ROM")
                            .clicked()
                        {
                            self.pending_action = Some(MenuAction::EjectCartridge);
                            ui.close();
                        }
                    });

                    ui.separator();

                    // Speed submenu
                    ui.menu_button(format!("Speed ({}%)", self.current_speed), |ui| {
                        for speed in [25, 50, 100, 200, 400] {
                            let label = if speed == self.current_speed {
                                format!("► {}%", speed)
                            } else {
                                format!("  {}%", speed)
                            };
                            if ui.button(label).clicked() {
                                self.pending_action = Some(MenuAction::SetSpeed(speed));
                                ui.close();
                            }
                        }
                    });

                    ui.separator();

                    // Save submenu - only enabled if ROM loaded
                    ui.add_enabled_ui(self.rom_loaded, |ui| {
                        ui.menu_button("Save State", |ui| {
                            for slot in 1..=5 {
                                if ui
                                    .button(format!("Slot {} (F{})", slot, slot + 4))
                                    .clicked()
                                {
                                    self.pending_action = Some(MenuAction::SaveState(slot));
                                    ui.close();
                                }
                            }
                        });
                    });

                    // Load submenu - only enabled if ROM loaded
                    ui.add_enabled_ui(self.rom_loaded, |ui| {
                        ui.menu_button("Load State", |ui| {
                            for slot in 1..=5 {
                                if ui
                                    .button(format!("Slot {} (Shift+F{})", slot, slot + 4))
                                    .clicked()
                                {
                                    self.pending_action = Some(MenuAction::LoadState(slot));
                                    ui.close();
                                }
                            }
                        });
                    });
                });
            });

            // View menu - Screenshot only enabled when system is loaded, others always available
            ui.menu_button("View", |ui| {
                ui.menu_button("Scaling", |ui| {
                    if ui
                        .button("Original")
                        .on_hover_text("1:1 pixel mapping, no scaling")
                        .clicked()
                    {
                        self.pending_action = Some(MenuAction::ScalingOriginal);
                        ui.close();
                    }
                    if ui
                        .button("Fit")
                        .on_hover_text("Scale to fit window, maintain aspect ratio")
                        .clicked()
                    {
                        self.pending_action = Some(MenuAction::ScalingFit);
                        ui.close();
                    }
                    if ui
                        .button("Stretch")
                        .on_hover_text("Stretch to fill window, ignore aspect ratio")
                        .clicked()
                    {
                        self.pending_action = Some(MenuAction::ScalingStretch);
                        ui.close();
                    }
                });

                // Display filters submenu
                ui.menu_button(format!("Filters ({})", self.current_filter.name()), |ui| {
                    let filters = [
                        (DisplayFilter::None, "None", "No filter, pure pixels"),
                        (
                            DisplayFilter::PhosphorPersistence,
                            "Phosphor Persistence",
                            "Temporal frame blending",
                        ),
                        (
                            DisplayFilter::SonyTrinitron,
                            "Sony Trinitron",
                            "CRT simulation",
                        ),
                        (DisplayFilter::Ibm5151, "IBM 5151", "Monochrome monitor"),
                        (
                            DisplayFilter::Commodore1702,
                            "Commodore 1702",
                            "Color monitor",
                        ),
                        (DisplayFilter::SharpLcd, "Sharp LCD", "LCD simulation"),
                        (DisplayFilter::RcaVictor, "RCA Victor", "CRT television"),
                    ];
                    for (filter, name, tooltip) in filters {
                        let label = if self.current_filter == filter {
                            format!("> {}", name)
                        } else {
                            format!("  {}", name)
                        };
                        if ui.button(label).on_hover_text(tooltip).clicked() {
                            self.pending_action = Some(MenuAction::SetDisplayFilter(filter));
                            ui.close();
                        }
                    }
                });

                ui.separator();

                if ui
                    .button("Fullscreen")
                    .on_hover_text("Toggle fullscreen mode without GUI")
                    .clicked()
                {
                    self.pending_action = Some(MenuAction::Fullscreen);
                    ui.close();
                }
                if ui
                    .button("Fullscreen with GUI")
                    .on_hover_text("Toggle fullscreen mode with GUI visible")
                    .clicked()
                {
                    self.pending_action = Some(MenuAction::FullscreenWithGui);
                    ui.close();
                }

                ui.separator();

                if ui
                    .button("Inspector")
                    .on_hover_text("Toggle Inspector panel (Debug, Log, Tiles)")
                    .clicked()
                {
                    self.pending_action = Some(MenuAction::ShowInspector);
                    ui.close();
                }

                ui.separator();

                // Property pane section toggles
                ui.label(egui::RichText::new("Property Panels").strong());

                let metrics_label = if self.metrics_visible {
                    "☑ Metrics"
                } else {
                    "☐ Metrics"
                };
                if ui
                    .button(metrics_label)
                    .on_hover_text("Toggle Metrics panel")
                    .clicked()
                {
                    self.pending_action = Some(MenuAction::ToggleMetrics);
                    ui.close();
                }

                let controller_label = if self.controller_visible {
                    "☑ Controller Settings"
                } else {
                    "☐ Controller Settings"
                };
                if ui
                    .button(controller_label)
                    .on_hover_text("Toggle Controller Settings panel")
                    .clicked()
                {
                    self.pending_action = Some(MenuAction::ToggleControllerSettings);
                    ui.close();
                }

                let mounts_label = if self.mounts_visible {
                    "☑ Mount Points"
                } else {
                    "☐ Mount Points"
                };
                if ui
                    .button(mounts_label)
                    .on_hover_text("Toggle Mount Points panel")
                    .clicked()
                {
                    self.pending_action = Some(MenuAction::ToggleMountPoints);
                    ui.close();
                }

                ui.separator();

                // Screenshot only enabled when system is loaded
                ui.add_enabled_ui(self.system_loaded, |ui| {
                    if ui
                        .button("Screenshot")
                        .on_hover_text(if self.system_loaded {
                            "Save a screenshot of the current frame"
                        } else {
                            "No system loaded - nothing to capture"
                        })
                        .clicked()
                    {
                        self.pending_action = Some(MenuAction::Screenshot);
                        ui.close();
                    }
                });
            });

            // Help menu
            ui.menu_button("Help", |ui| {
                if ui
                    .button("User Manual (browser)")
                    .on_hover_text("View keyboard controls and usage instructions")
                    .clicked()
                {
                    self.pending_action = Some(MenuAction::ShowHelp);
                    ui.close();
                }
                ui.separator();
                if ui
                    .button("About")
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
