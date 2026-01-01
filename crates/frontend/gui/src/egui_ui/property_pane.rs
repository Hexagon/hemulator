//! Right-side property pane with collapsible sections

use egui::{ScrollArea, Ui};

/// Display filter options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayFilter {
    None,
    Scanlines,
    Phosphor,
    CRT,
}

impl DisplayFilter {
    pub fn as_str(&self) -> &str {
        match self {
            DisplayFilter::None => "None",
            DisplayFilter::Scanlines => "Scanlines",
            DisplayFilter::Phosphor => "Phosphor",
            DisplayFilter::CRT => "CRT",
        }
    }
}

/// Actions that can be triggered from the property pane
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropertyAction {
    SaveState(u8),  // Slot number 1-5
    LoadState(u8),  // Slot number 1-5
}

pub struct PropertyPane {
    // Machine metrics
    pub fps: f32,
    pub system_name: String,
    pub paused: bool,
    pub speed: f32,
    pub cpu_freq_target: Option<f64>,
    pub cpu_freq_actual: Option<f64>,
    pub rendering_backend: String,
    
    // Settings
    pub display_filter: DisplayFilter,
    pub emulation_speed_percent: i32, // 0-400
    
    // Mount points
    pub mount_points: Vec<MountPoint>,
    
    // Pending action
    pending_action: Option<PropertyAction>,
    
    // Collapsible section states
    metrics_open: bool,
    settings_open: bool,
    mounts_open: bool,
    save_states_open: bool,
}

#[derive(Clone)]
pub struct MountPoint {
    pub id: String,
    pub name: String,
    pub mounted_file: Option<String>,
}

impl PropertyPane {
    pub fn new() -> Self {
        Self {
            fps: 0.0,
            system_name: String::new(),
            paused: false,
            speed: 1.0,
            cpu_freq_target: None,
            cpu_freq_actual: None,
            rendering_backend: "Software".to_string(),
            display_filter: DisplayFilter::None,
            emulation_speed_percent: 100,
            mount_points: Vec::new(),
            metrics_open: true,
            settings_open: true,
            mounts_open: false,
            save_states_open: false,
            pending_action: None,
        }
    }

    /// Take the pending action if any
    pub fn take_action(&mut self) -> Option<PropertyAction> {
        self.pending_action.take()
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                // Machine Metrics section
                egui::CollapsingHeader::new("Machine Metrics")
                    .default_open(self.metrics_open)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label("FPS:");
                            ui.label(format!("{:.1}", self.fps));
                        });
                        
                        if !self.system_name.is_empty() {
                            ui.horizontal(|ui| {
                                ui.label("System:");
                                ui.label(&self.system_name);
                            });
                        }
                        
                        if self.paused {
                            ui.colored_label(egui::Color32::YELLOW, "⏸ PAUSED");
                        } else if self.speed != 1.0 {
                            ui.colored_label(egui::Color32::YELLOW, 
                                format!("⏩ {}%", (self.speed * 100.0) as u32));
                        }
                        
                        if let Some(target_freq) = self.cpu_freq_target {
                            ui.horizontal(|ui| {
                                ui.label("CPU Target:");
                                ui.label(format!("{:.2} MHz", target_freq));
                            });
                        }
                        
                        ui.horizontal(|ui| {
                            ui.label("Renderer:");
                            ui.label(&self.rendering_backend);
                        });
                    });

                ui.add_space(5.0);

                // Project Settings section
                egui::CollapsingHeader::new("Project Settings")
                    .default_open(self.settings_open)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Display Filter:");
                        });
                        egui::ComboBox::from_id_salt("display_filter")
                            .selected_text(self.display_filter.as_str())
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.display_filter, DisplayFilter::None, "None");
                                ui.selectable_value(&mut self.display_filter, DisplayFilter::Scanlines, "Scanlines");
                                ui.selectable_value(&mut self.display_filter, DisplayFilter::Phosphor, "Phosphor");
                                ui.selectable_value(&mut self.display_filter, DisplayFilter::CRT, "CRT");
                            });
                        
                        ui.horizontal(|ui| {
                            ui.label("Emulation Speed:");
                            ui.label(format!("{}%", self.emulation_speed_percent));
                        });
                        
                        ui.add(egui::Slider::new(&mut self.emulation_speed_percent, 0..=400)
                            .text("%"));
                        
                        ui.horizontal(|ui| {
                            if ui.button("25%").clicked() {
                                self.emulation_speed_percent = 25;
                            }
                            if ui.button("50%").clicked() {
                                self.emulation_speed_percent = 50;
                            }
                            if ui.button("100%").clicked() {
                                self.emulation_speed_percent = 100;
                            }
                            if ui.button("200%").clicked() {
                                self.emulation_speed_percent = 200;
                            }
                            if ui.button("400%").clicked() {
                                self.emulation_speed_percent = 400;
                            }
                        });
                    });

                ui.add_space(5.0);

                // Mount Points section
                egui::CollapsingHeader::new("Mount Points")
                    .default_open(self.mounts_open)
                    .show(ui, |ui| {
                        if self.mount_points.is_empty() {
                            ui.label("No mount points available");
                        } else {
                            for mount in &self.mount_points {
                                ui.horizontal(|ui| {
                                    ui.label(format!("{}:", mount.name));
                                    if let Some(ref file) = mount.mounted_file {
                                        ui.label(file);
                                        if ui.button("Eject").clicked() {
                                            // TODO: Handle eject
                                        }
                                    } else {
                                        if ui.button("Mount...").clicked() {
                                            // TODO: Handle mount
                                        }
                                    }
                                });
                            }
                        }
                    });

                ui.add_space(5.0);

                // Save States section
                egui::CollapsingHeader::new("Save States")
                    .default_open(self.save_states_open)
                    .show(ui, |ui| {
                        ui.label("Quick Save/Load:");
                        ui.horizontal(|ui| {
                            for i in 1..=5 {
                                if ui.button(format!("S{}", i)).on_hover_text(format!("Save to slot {} (F{})", i, i+4)).clicked() {
                                    self.pending_action = Some(PropertyAction::SaveState(i));
                                }
                            }
                        });
                        ui.horizontal(|ui| {
                            for i in 1..=5 {
                                if ui.button(format!("L{}", i)).on_hover_text(format!("Load from slot {} (Shift+F{})", i, i+4)).clicked() {
                                    self.pending_action = Some(PropertyAction::LoadState(i));
                                }
                            }
                        });
                    });
            });
    }
}

impl Default for PropertyPane {
    fn default() -> Self {
        Self::new()
    }
}
