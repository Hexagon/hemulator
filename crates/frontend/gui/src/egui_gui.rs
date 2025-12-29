//! Egui-based GUI for the emulator
//!
//! This module provides a menu system and tabbed interface using egui.

use crate::display_filter::DisplayFilter;

/// Active tab in the GUI
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveTab {
    Monitor,
    Debug,
    LogOutput,
}

/// GUI state managed by egui
pub struct EguiGui {
    /// Currently active tab
    pub active_tab: ActiveTab,
    
    /// Whether to show the GUI (can be toggled off for fullscreen)
    pub show_gui: bool,
    
    /// Status message to display in status bar
    pub status_message: String,
    
    /// FPS counter for status bar
    pub fps: f64,
    
    /// Log filter settings
    pub log_filter_cpu: bool,
    pub log_filter_bus: bool,
    pub log_filter_ppu: bool,
    pub log_filter_apu: bool,
    pub log_filter_interrupts: bool,
    pub log_filter_stubs: bool,
    
    /// Log level filter
    pub log_level: String,
    
    /// Whether a file dialog is open (prevents input to emulator)
    pub dialog_open: bool,
}

impl Default for EguiGui {
    fn default() -> Self {
        Self {
            active_tab: ActiveTab::Monitor,
            show_gui: true,
            status_message: String::new(),
            fps: 0.0,
            log_filter_cpu: false,
            log_filter_bus: false,
            log_filter_ppu: false,
            log_filter_apu: false,
            log_filter_interrupts: false,
            log_filter_stubs: false,
            log_level: "info".to_string(),
            dialog_open: false,
        }
    }
}

/// Actions that can be triggered from the GUI
#[derive(Debug, Clone, PartialEq)]
pub enum GuiAction {
    None,
    OpenProject,
    SaveProject,
    SaveProjectAs,
    Exit,
    ToggleDebug,
    SelectCrtFilter(DisplayFilter),
    OpenMountPoint(String), // mount point ID
    CreateBlankDisk,
    ShowSettings,
    TakeScreenshot,
    SaveState(u8),
    LoadState(u8),
    Reset,
}

impl EguiGui {
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Update GUI status message
    pub fn set_status(&mut self, message: String) {
        self.status_message = message;
    }
    
    /// Update FPS counter
    pub fn set_fps(&mut self, fps: f64) {
        self.fps = fps;
    }
    
    /// Render the GUI and return any action to perform
    /// This is a simplified version that just renders a basic menu
    /// The actual integration with EmulatorSystem will be done in main.rs
    pub fn render_basic(&mut self, ctx: &egui::Context) -> GuiAction {
        let mut action = GuiAction::None;
        
        if !self.show_gui {
            return action;
        }
        
        // Menu bar
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                // File menu
                ui.menu_button("File", |ui| {
                    if ui.button("Open Project...").clicked() {
                        action = GuiAction::OpenProject;
                        ui.close_menu();
                    }
                    if ui.button("Save Project").clicked() {
                        action = GuiAction::SaveProject;
                        ui.close_menu();
                    }
                    if ui.button("Save Project As...").clicked() {
                        action = GuiAction::SaveProjectAs;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Exit").clicked() {
                        action = GuiAction::Exit;
                        ui.close_menu();
                    }
                });
                
                // Machine menu
                ui.menu_button("Machine", |ui| {
                    if ui.button("Settings").clicked() {
                        action = GuiAction::ShowSettings;
                        ui.close_menu();
                    }
                    if ui.button("Debug").clicked() {
                        action = GuiAction::ToggleDebug;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Reset").clicked() {
                        action = GuiAction::Reset;
                        ui.close_menu();
                    }
                });
                
                // Display menu
                ui.menu_button("Display", |ui| {
                    ui.menu_button("Filters", |ui| {
                        if ui.button("None").clicked() {
                            action = GuiAction::SelectCrtFilter(DisplayFilter::None);
                            ui.close_menu();
                        }
                        if ui.button("Sony Trinitron").clicked() {
                            action = GuiAction::SelectCrtFilter(DisplayFilter::SonyTrinitron);
                            ui.close_menu();
                        }
                        if ui.button("IBM 5151").clicked() {
                            action = GuiAction::SelectCrtFilter(DisplayFilter::Ibm5151);
                            ui.close_menu();
                        }
                        if ui.button("Commodore 1702").clicked() {
                            action = GuiAction::SelectCrtFilter(DisplayFilter::Commodore1702);
                            ui.close_menu();
                        }
                        if ui.button("Sharp LCD").clicked() {
                            action = GuiAction::SelectCrtFilter(DisplayFilter::SharpLcd);
                            ui.close_menu();
                        }
                        if ui.button("RCA Victor").clicked() {
                            action = GuiAction::SelectCrtFilter(DisplayFilter::RcaVictor);
                            ui.close_menu();
                        }
                    });
                    ui.separator();
                    if ui.button("Take Screenshot").clicked() {
                        action = GuiAction::TakeScreenshot;
                        ui.close_menu();
                    }
                });
                
                // Devices menu - simplified for now
                ui.menu_button("Devices", |ui| {
                    ui.label("(Mount points will be shown here)");
                });
                
                // Help/About
                ui.menu_button("Help", |ui| {
                    if ui.button("About").clicked() {
                        ui.close_menu();
                    }
                });
            });
        });
        
        // Tab bar
        egui::TopBottomPanel::top("tab_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.active_tab, ActiveTab::Monitor, "Monitor");
                ui.selectable_value(&mut self.active_tab, ActiveTab::Debug, "Debug");
                ui.selectable_value(&mut self.active_tab, ActiveTab::LogOutput, "Log Output");
            });
        });
        
        // Status bar at bottom
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("System: (awaiting integration)");
                ui.separator();
                ui.label(format!("FPS: {:.1}", self.fps));
                ui.separator();
                ui.label(&self.status_message);
            });
        });
        
        // Tab content panels
        match self.active_tab {
            ActiveTab::Monitor => {
                // Monitor tab shows the emulator display (handled by main render loop)
            }
            ActiveTab::Debug => {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.heading("Debug Information");
                    ui.label("(Debug information will be shown here when integrated)");
                });
            }
            ActiveTab::LogOutput => {
                egui::CentralPanel::default().show(ctx, |ui| {
                    self.render_log_tab(ui);
                });
            }
        }
        
        action
    }
    
    /// Render the log output tab content
    fn render_log_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("Log Output");
        ui.separator();
        
        ui.horizontal(|ui| {
            ui.label("Filter by category:");
            ui.checkbox(&mut self.log_filter_cpu, "CPU");
            ui.checkbox(&mut self.log_filter_bus, "Bus");
            ui.checkbox(&mut self.log_filter_ppu, "PPU");
            ui.checkbox(&mut self.log_filter_apu, "APU");
            ui.checkbox(&mut self.log_filter_interrupts, "Interrupts");
            ui.checkbox(&mut self.log_filter_stubs, "Stubs");
        });
        
        ui.horizontal(|ui| {
            ui.label("Log level:");
            egui::ComboBox::from_id_source("log_level")
                .selected_text(&self.log_level)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.log_level, "off".to_string(), "Off");
                    ui.selectable_value(&mut self.log_level, "error".to_string(), "Error");
                    ui.selectable_value(&mut self.log_level, "warn".to_string(), "Warn");
                    ui.selectable_value(&mut self.log_level, "info".to_string(), "Info");
                    ui.selectable_value(&mut self.log_level, "debug".to_string(), "Debug");
                    ui.selectable_value(&mut self.log_level, "trace".to_string(), "Trace");
                });
        });
        
        ui.separator();
        
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                ui.label("Log output would appear here");
                ui.label("(Log capture not yet implemented)");
            });
    }
}
