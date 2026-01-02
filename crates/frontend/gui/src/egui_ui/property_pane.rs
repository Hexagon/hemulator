//! Right-side property pane with collapsible sections

use crate::display_filter::DisplayFilter;
use egui::{ScrollArea, Ui};

/// Source of input configuration (global config.json or project-specific)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputConfigSource {
    Global,  // Using config.json
    Project, // Using project .hemu file override
}

/// Actions that can be triggered from the property pane
#[derive(Debug, Clone, PartialEq)]
pub enum PropertyAction {
    SaveState(u8),                    // Slot number 1-5
    LoadState(u8),                    // Slot number 1-5
    MountFile(String),                // Mount point ID
    EjectFile(String),                // Mount point ID
    ConfigureInput,                   // Open input configuration dialog
    SetInputSource(InputConfigSource), // Switch between global/project input config
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

    // FPS sparkline data (last 60 frames)
    fps_history: Vec<f32>,

    // Target FPS from system timing mode (for sparkline reference line)
    pub target_fps: f32,

    // PC-specific BDA values (only populated for PC system)
    pub pc_bda_values: Option<PcBdaValues>,

    // Settings
    pub display_filter: DisplayFilter,
    pub emulation_speed_percent: i32, // 0-400
    
    // Input configuration (can be global or project-specific)
    pub input_config_source: InputConfigSource, // Global or Project
    pub player1_enabled: bool,
    pub player2_enabled: bool,
    pub mouse_enabled: bool,
    pub mouse_sensitivity: f32,
    pub num_gamepads_detected: usize,
    pub num_joysticks_detected: usize,

    // PC-specific settings (only shown for PC system)
    pub pc_cpu_model: Option<String>,
    pub pc_memory_kb: Option<u32>,

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

/// PC-specific BDA (BIOS Data Area) values
#[derive(Clone, Debug)]
pub struct PcBdaValues {
    pub equipment_word: u16,
    pub memory_size_kb: u16,
    pub video_mode: u8,
    pub video_columns: u8,
    pub num_serial_ports: u8,
    pub num_parallel_ports: u8,
    pub num_hard_drives: u8,
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
            fps_history: Vec::with_capacity(60),
            target_fps: 60.0,
            pc_bda_values: None,
            display_filter: DisplayFilter::None,
            emulation_speed_percent: 100,
            input_config_source: InputConfigSource::Global,
            player1_enabled: true,
            player2_enabled: false,
            mouse_enabled: false,
            mouse_sensitivity: 1.0,
            num_gamepads_detected: 0,
            num_joysticks_detected: 0,
            pc_cpu_model: None,
            pc_memory_kb: None,
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

    /// Update FPS and add to sparkline history
    pub fn update_fps(&mut self, fps: f32) {
        self.fps = fps;
        self.fps_history.push(fps);
        if self.fps_history.len() > 60 {
            self.fps_history.remove(0);
        }
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

                        // FPS sparkline (last 60 frames)
                        if !self.fps_history.is_empty() {
                            let max_fps = self
                                .fps_history
                                .iter()
                                .fold(0.0f32, |a, &b| a.max(b))
                                .max(self.target_fps);
                            let min_fps = 0.0f32;

                            use egui::*;
                            let desired_size = vec2(ui.available_width(), 30.0);
                            let (rect, _response) =
                                ui.allocate_exact_size(desired_size, Sense::hover());

                            if ui.is_rect_visible(rect) {
                                let painter = ui.painter();

                                // Draw background
                                painter.rect_filled(rect, 2.0, Color32::from_rgb(30, 30, 30));

                                // Draw sparkline
                                let _points_per_pixel =
                                    self.fps_history.len() as f32 / rect.width();
                                let mut points = Vec::new();

                                for (i, &fps_val) in self.fps_history.iter().enumerate() {
                                    let x = rect.left()
                                        + (i as f32 / self.fps_history.len() as f32) * rect.width();
                                    let normalized =
                                        ((fps_val - min_fps) / (max_fps - min_fps)).clamp(0.0, 1.0);
                                    let y = rect.bottom() - normalized * rect.height();
                                    points.push(pos2(x, y));
                                }

                                // Draw line
                                if points.len() >= 2 {
                                    painter.add(epaint::PathShape::line(
                                        points,
                                        Stroke::new(1.5, Color32::from_rgb(0, 200, 0)),
                                    ));
                                }

                                // Draw reference line at target FPS
                                if max_fps > self.target_fps {
                                    let normalized_target =
                                        (self.target_fps - min_fps) / (max_fps - min_fps);
                                    let y_target =
                                        rect.bottom() - normalized_target * rect.height();
                                    painter.line_segment(
                                        [pos2(rect.left(), y_target), pos2(rect.right(), y_target)],
                                        Stroke::new(0.5, Color32::from_rgb(100, 100, 100)),
                                    );
                                }
                            }
                        }

                        if !self.system_name.is_empty() {
                            ui.horizontal(|ui| {
                                ui.label("System:");
                                ui.label(&self.system_name);
                            });
                        }

                        if self.paused {
                            ui.colored_label(egui::Color32::YELLOW, "⏸ PAUSED");
                        } else if self.speed != 1.0 {
                            ui.colored_label(
                                egui::Color32::YELLOW,
                                format!("⏩ {}%", (self.speed * 100.0) as u32),
                            );
                        }

                        if let Some(target_freq) = self.cpu_freq_target {
                            ui.horizontal(|ui| {
                                ui.label("CPU Target:");
                                ui.label(format!("{:.2} MHz", target_freq));
                            });
                        }

                        // Display PC-specific BDA values if available
                        if let Some(ref bda) = self.pc_bda_values {
                            ui.add_space(5.0);
                            ui.separator();
                            ui.label(egui::RichText::new("BIOS Data Area").strong());

                            ui.horizontal(|ui| {
                                ui.label("Video Mode:");
                                ui.label(format!("{:02X}h", bda.video_mode));
                            });

                            ui.horizontal(|ui| {
                                ui.label("Video Columns:");
                                ui.label(format!("{}", bda.video_columns));
                            });

                            ui.horizontal(|ui| {
                                ui.label("Memory (BDA):");
                                ui.label(format!("{} KB", bda.memory_size_kb));
                            });

                            ui.horizontal(|ui| {
                                ui.label("Serial Ports:");
                                ui.label(format!("{}", bda.num_serial_ports));
                            });

                            ui.horizontal(|ui| {
                                ui.label("Parallel Ports:");
                                ui.label(format!("{}", bda.num_parallel_ports));
                            });

                            ui.horizontal(|ui| {
                                ui.label("Hard Drives:");
                                ui.label(format!("{}", bda.num_hard_drives));
                            });

                            ui.horizontal(|ui| {
                                ui.label("Equipment:");
                                ui.label(format!("{:04X}h", bda.equipment_word));
                            });
                        }
                    });

                ui.add_space(5.0);

                // Project Settings section
                egui::CollapsingHeader::new("Project Settings")
                    .default_open(self.settings_open)
                    .show(ui, |ui| {
                        // Renderer selection (moved from Machine Metrics)
                        ui.horizontal(|ui| {
                            ui.label("Renderer:");
                        });
                        let backend_clone = self.rendering_backend.clone();
                        egui::ComboBox::from_id_salt("renderer_select")
                            .selected_text(&self.rendering_backend)
                            .show_ui(ui, |ui| {
                                // Note: This is currently display-only as renderer switching
                                // would require significant refactoring. For now, just show current value.
                                ui.selectable_value(
                                    &mut self.rendering_backend,
                                    backend_clone.clone(),
                                    &backend_clone,
                                );
                            });

                        // PC-specific settings: CPU Model
                        if self.pc_cpu_model.is_some() {
                            ui.add_space(5.0);
                            ui.separator();
                            ui.label(egui::RichText::new("PC Configuration").strong());

                            if let Some(ref mut cpu_model) = self.pc_cpu_model {
                                ui.horizontal(|ui| {
                                    ui.label("CPU Model:");
                                });
                                egui::ComboBox::from_id_salt("cpu_model_select")
                                    .selected_text(cpu_model.as_str())
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(
                                            cpu_model,
                                            "Intel 8086".to_string(),
                                            "Intel 8086",
                                        );
                                        ui.selectable_value(
                                            cpu_model,
                                            "Intel 8088".to_string(),
                                            "Intel 8088",
                                        );
                                        ui.selectable_value(
                                            cpu_model,
                                            "Intel 80186".to_string(),
                                            "Intel 80186",
                                        );
                                        ui.selectable_value(
                                            cpu_model,
                                            "Intel 80188".to_string(),
                                            "Intel 80188",
                                        );
                                        ui.selectable_value(
                                            cpu_model,
                                            "Intel 80286".to_string(),
                                            "Intel 80286",
                                        );
                                        ui.selectable_value(
                                            cpu_model,
                                            "Intel 80386".to_string(),
                                            "Intel 80386",
                                        );
                                        ui.selectable_value(
                                            cpu_model,
                                            "Intel 80486".to_string(),
                                            "Intel 80486",
                                        );
                                        ui.selectable_value(
                                            cpu_model,
                                            "Intel 80486SX".to_string(),
                                            "Intel 80486SX",
                                        );
                                        ui.selectable_value(
                                            cpu_model,
                                            "Intel 80486DX2".to_string(),
                                            "Intel 80486DX2",
                                        );
                                        ui.selectable_value(
                                            cpu_model,
                                            "Intel 80486SX2".to_string(),
                                            "Intel 80486SX2",
                                        );
                                        ui.selectable_value(
                                            cpu_model,
                                            "Intel 80486DX4".to_string(),
                                            "Intel 80486DX4",
                                        );
                                        ui.selectable_value(
                                            cpu_model,
                                            "Intel Pentium".to_string(),
                                            "Intel Pentium",
                                        );
                                        ui.selectable_value(
                                            cpu_model,
                                            "Intel Pentium MMX".to_string(),
                                            "Intel Pentium MMX",
                                        );
                                    });
                            }

                            // PC-specific settings: Memory
                            if let Some(ref mut memory_kb) = self.pc_memory_kb {
                                ui.horizontal(|ui| {
                                    ui.label("Memory:");
                                });
                                egui::ComboBox::from_id_salt("memory_select")
                                    .selected_text(format!("{} KB", memory_kb))
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(memory_kb, 64, "64 KB");
                                        ui.selectable_value(memory_kb, 128, "128 KB");
                                        ui.selectable_value(memory_kb, 256, "256 KB");
                                        ui.selectable_value(memory_kb, 512, "512 KB");
                                        ui.selectable_value(memory_kb, 640, "640 KB");
                                        ui.selectable_value(memory_kb, 1024, "1024 KB (1 MB)");
                                        ui.selectable_value(memory_kb, 2048, "2048 KB (2 MB)");
                                        ui.selectable_value(memory_kb, 4096, "4096 KB (4 MB)");
                                        ui.selectable_value(memory_kb, 8192, "8192 KB (8 MB)");
                                        ui.selectable_value(memory_kb, 16384, "16384 KB (16 MB)");
                                    });
                            }

                            ui.add_space(5.0);
                            ui.separator();
                        }

                        // Display filter
                        ui.horizontal(|ui| {
                            ui.label("Display Filter:");
                        });
                        egui::ComboBox::from_id_salt("display_filter")
                            .selected_text(self.display_filter.name())
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.display_filter,
                                    DisplayFilter::None,
                                    "None",
                                );
                                ui.selectable_value(
                                    &mut self.display_filter,
                                    DisplayFilter::SonyTrinitron,
                                    "Sony Trinitron",
                                );
                                ui.selectable_value(
                                    &mut self.display_filter,
                                    DisplayFilter::Ibm5151,
                                    "IBM 5151",
                                );
                                ui.selectable_value(
                                    &mut self.display_filter,
                                    DisplayFilter::Commodore1702,
                                    "Commodore 1702",
                                );
                                ui.selectable_value(
                                    &mut self.display_filter,
                                    DisplayFilter::SharpLcd,
                                    "Sharp LCD",
                                );
                                ui.selectable_value(
                                    &mut self.display_filter,
                                    DisplayFilter::RcaVictor,
                                    "RCA Victor",
                                );
                            });

                        ui.horizontal(|ui| {
                            ui.label("Emulation Speed:");
                            ui.label(format!("{}%", self.emulation_speed_percent));
                        });

                        // Removed slider per requirements, only showing buttons
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
                        
                        // Input Configuration section
                        ui.add_space(10.0);
                        ui.separator();
                        ui.label(egui::RichText::new("Input Configuration").strong());
                        
                        // Input source selection (Global vs Project)
                        ui.horizontal(|ui| {
                            ui.label("Config Source:");
                            if ui
                                .selectable_label(
                                    self.input_config_source == InputConfigSource::Global,
                                    "Global"
                                )
                                .on_hover_text("Use global config.json settings for all projects")
                                .clicked()
                            {
                                if self.input_config_source != InputConfigSource::Global {
                                    self.pending_action =
                                        Some(PropertyAction::SetInputSource(InputConfigSource::Global));
                                }
                            }
                            if ui
                                .selectable_label(
                                    self.input_config_source == InputConfigSource::Project,
                                    "Project"
                                )
                                .on_hover_text("Use project-specific .hemu file settings")
                                .clicked()
                            {
                                if self.input_config_source != InputConfigSource::Project {
                                    self.pending_action =
                                        Some(PropertyAction::SetInputSource(InputConfigSource::Project));
                                }
                            }
                        });
                        
                        // Input device status
                        ui.add_space(5.0);
                        ui.horizontal(|ui| {
                            ui.label("Gamepads:");
                            ui.label(format!("{} detected", self.num_gamepads_detected));
                        });
                        ui.horizontal(|ui| {
                            ui.label("Joysticks:");
                            ui.label(format!("{} detected", self.num_joysticks_detected));
                        });
                        
                        // Player configuration
                        ui.add_space(5.0);
                        ui.checkbox(&mut self.player1_enabled, "Player 1 Enabled");
                        ui.checkbox(&mut self.player2_enabled, "Player 2 Enabled");
                        
                        // Mouse configuration
                        ui.add_space(5.0);
                        ui.checkbox(&mut self.mouse_enabled, "Mouse Input Enabled");
                        if self.mouse_enabled {
                            ui.horizontal(|ui| {
                                ui.label("Sensitivity:");
                                ui.add(
                                    egui::Slider::new(&mut self.mouse_sensitivity, 0.1..=5.0)
                                        .step_by(0.1)
                                        .show_value(true),
                                );
                            });
                        }
                        
                        // Configure button
                        ui.add_space(5.0);
                        if ui
                            .button("Configure Buttons...")
                            .on_hover_text("Open detailed input configuration dialog")
                            .clicked()
                        {
                            self.pending_action = Some(PropertyAction::ConfigureInput);
                        }
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
                                            self.pending_action =
                                                Some(PropertyAction::EjectFile(mount.id.clone()));
                                        }
                                    } else if ui.button("Mount...").clicked() {
                                        self.pending_action =
                                            Some(PropertyAction::MountFile(mount.id.clone()));
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
                                if ui
                                    .button(format!("S{}", i))
                                    .on_hover_text(format!("Save to slot {} (F{})", i, i + 4))
                                    .clicked()
                                {
                                    self.pending_action = Some(PropertyAction::SaveState(i));
                                }
                            }
                        });
                        ui.horizontal(|ui| {
                            for i in 1..=5 {
                                if ui
                                    .button(format!("L{}", i))
                                    .on_hover_text(format!(
                                        "Load from slot {} (Shift+F{})",
                                        i,
                                        i + 4
                                    ))
                                    .clicked()
                                {
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
