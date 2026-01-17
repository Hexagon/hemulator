//! Right-side property pane with modular sections

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
    MountFile(String),                 // Mount point ID
    EjectFile(String),                 // Mount point ID
    ConfigureInput,                    // Open input configuration dialog
    SetInputSource(InputConfigSource), // Switch between global/project input config
    SetRenderer(String),               // Switch to specified renderer
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
    pub available_renderers: Vec<String>,

    // FPS sparkline data (last 60 frames)
    fps_history: Vec<f32>,

    // Target FPS from system timing mode
    pub target_fps: f32,

    // PC-specific BDA values
    pub pc_bda_values: Option<PcBdaValues>,

    // Settings
    pub emulation_speed_percent: i32,

    // Input configuration
    pub input_config_source: InputConfigSource,
    pub player1_enabled: bool,
    pub player2_enabled: bool,
    pub mouse_enabled: bool,
    pub mouse_sensitivity: f32,
    pub num_gamepads_detected: usize,
    pub num_joysticks_detected: usize,

    // PC-specific settings
    pub pc_cpu_model: Option<String>,
    pub pc_memory_kb: Option<u32>,

    // Mount points
    pub mount_points: Vec<MountPoint>,

    // Pending action
    pending_action: Option<PropertyAction>,

    // Section visibility (controlled by menu)
    pub metrics_visible: bool,
    pub controller_visible: bool,
    pub mounts_visible: bool,
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
            available_renderers: vec!["Software".to_string()],
            fps_history: Vec::with_capacity(60),
            target_fps: 60.0,
            pc_bda_values: None,
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
            metrics_visible: false,
            controller_visible: false,
            mounts_visible: false,
            pending_action: None,
        }
    }

    /// Check if any section is visible
    pub fn any_section_visible(&self) -> bool {
        self.metrics_visible || self.controller_visible || self.mounts_visible
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
                if self.metrics_visible {
                    egui::CollapsingHeader::new(egui::RichText::new("📊 Metrics").strong())
                        .default_open(true)
                        .show(ui, |ui| {
                            self.render_metrics(ui);
                        });
                    ui.add_space(5.0);
                }

                // Controller Settings section
                if self.controller_visible {
                    egui::CollapsingHeader::new(
                        egui::RichText::new("🎮 Controller Settings").strong(),
                    )
                    .default_open(true)
                    .show(ui, |ui| {
                        self.render_controller_settings(ui);
                    });
                    ui.add_space(5.0);
                }

                // Mount Points section
                if self.mounts_visible {
                    egui::CollapsingHeader::new(egui::RichText::new("💿 Mount Points").strong())
                        .default_open(true)
                        .show(ui, |ui| {
                            self.render_mount_points(ui);
                        });
                }

                // Show message if no sections visible
                if !self.any_section_visible() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(20.0);
                        ui.label(egui::RichText::new("No panels visible").weak().italics());
                        ui.add_space(5.0);
                        ui.label(
                            egui::RichText::new("Use View menu to show panels")
                                .weak()
                                .small(),
                        );
                    });
                }
            });
    }

    fn render_metrics(&mut self, ui: &mut Ui) {
        ui.add_space(3.0);

        // FPS display with visual indicator
        ui.horizontal(|ui| {
            ui.label("FPS:");
            let fps_color = if self.fps >= self.target_fps * 0.95 {
                egui::Color32::from_rgb(0, 200, 0)
            } else if self.fps >= self.target_fps * 0.8 {
                egui::Color32::from_rgb(255, 200, 0)
            } else {
                egui::Color32::from_rgb(200, 0, 0)
            };
            ui.colored_label(fps_color, format!("{:.1}", self.fps));
        });

        // FPS sparkline (last 60 frames)
        if !self.fps_history.is_empty() {
            ui.add_space(3.0);
            let max_fps = self
                .fps_history
                .iter()
                .fold(0.0f32, |a, &b| a.max(b))
                .max(self.target_fps);
            let min_fps = 0.0f32;

            use egui::*;
            let desired_size = vec2(ui.available_width(), 35.0);
            let (rect, _response) = ui.allocate_exact_size(desired_size, Sense::hover());

            if ui.is_rect_visible(rect) {
                let painter = ui.painter();
                painter.rect_filled(rect, 2.0, Color32::from_rgb(20, 20, 20));

                let mut points = Vec::new();
                for (i, &fps_val) in self.fps_history.iter().enumerate() {
                    let x = rect.left() + (i as f32 / self.fps_history.len() as f32) * rect.width();
                    let normalized = ((fps_val - min_fps) / (max_fps - min_fps)).clamp(0.0, 1.0);
                    let y = rect.bottom() - normalized * rect.height();
                    points.push(pos2(x, y));
                }

                if points.len() >= 2 {
                    painter.add(epaint::PathShape::line(
                        points,
                        Stroke::new(2.0, Color32::from_rgb(0, 220, 0)),
                    ));
                }

                if max_fps > self.target_fps {
                    let normalized_target = (self.target_fps - min_fps) / (max_fps - min_fps);
                    let y_target = rect.bottom() - normalized_target * rect.height();
                    painter.line_segment(
                        [pos2(rect.left(), y_target), pos2(rect.right(), y_target)],
                        Stroke::new(1.0, Color32::from_rgb(120, 120, 120)),
                    );
                }
            }
        }

        ui.add_space(5.0);

        if !self.system_name.is_empty() {
            ui.horizontal(|ui| {
                ui.label("System:");
                ui.label(egui::RichText::new(&self.system_name).strong());
            });
        }

        if self.paused {
            ui.add_space(3.0);
            ui.colored_label(egui::Color32::YELLOW, "⏸ PAUSED");
        } else if self.speed != 1.0 {
            ui.add_space(3.0);
            ui.colored_label(
                egui::Color32::YELLOW,
                format!("⏩ {}%", (self.speed * 100.0) as u32),
            );
        }

        if let Some(target_freq) = self.cpu_freq_target {
            ui.add_space(3.0);
            ui.horizontal(|ui| {
                ui.label("CPU Target:");
                ui.label(format!("{:.2} MHz", target_freq));
            });
        }

        // Renderer selection
        ui.add_space(5.0);
        ui.horizontal(|ui| {
            ui.label("Renderer:");
        });
        let current_renderer = self.rendering_backend.clone();
        egui::ComboBox::from_id_salt("renderer_select")
            .selected_text(&self.rendering_backend)
            .show_ui(ui, |ui| {
                for renderer in &self.available_renderers {
                    if ui
                        .selectable_value(&mut self.rendering_backend, renderer.clone(), renderer)
                        .clicked()
                        && renderer != &current_renderer
                    {
                        self.pending_action = Some(PropertyAction::SetRenderer(renderer.clone()));
                    }
                }
            });

        // PC-specific BDA values
        if let Some(ref bda) = self.pc_bda_values {
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(5.0);
            ui.label(egui::RichText::new("💾 BIOS Data Area").strong());
            ui.add_space(3.0);

            egui::Grid::new("bda_grid")
                .num_columns(2)
                .spacing([8.0, 3.0])
                .striped(false)
                .show(ui, |ui| {
                    ui.label("Video Mode:");
                    ui.label(format!("{:02X}h", bda.video_mode));
                    ui.end_row();

                    ui.label("Video Columns:");
                    ui.label(format!("{}", bda.video_columns));
                    ui.end_row();

                    ui.label("Memory (BDA):");
                    ui.label(format!("{} KB", bda.memory_size_kb));
                    ui.end_row();

                    ui.label("Serial Ports:");
                    ui.label(format!("{}", bda.num_serial_ports));
                    ui.end_row();

                    ui.label("Parallel Ports:");
                    ui.label(format!("{}", bda.num_parallel_ports));
                    ui.end_row();

                    ui.label("Hard Drives:");
                    ui.label(format!("{}", bda.num_hard_drives));
                    ui.end_row();

                    ui.label("Equipment:");
                    ui.label(format!("{:04X}h", bda.equipment_word));
                    ui.end_row();
                });
        }

        // PC-specific settings
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
                        for model in [
                            "Intel 8086",
                            "Intel 8088",
                            "Intel 80186",
                            "Intel 80188",
                            "Intel 80286",
                            "Intel 80386",
                            "Intel 80486",
                            "Intel 80486SX",
                            "Intel 80486DX2",
                            "Intel 80486SX2",
                            "Intel 80486DX4",
                            "Intel Pentium",
                            "Intel Pentium MMX",
                        ] {
                            ui.selectable_value(cpu_model, model.to_string(), model);
                        }
                    });
            }

            if let Some(ref mut memory_kb) = self.pc_memory_kb {
                ui.horizontal(|ui| {
                    ui.label("Memory:");
                });
                egui::ComboBox::from_id_salt("memory_select")
                    .selected_text(format!("{} KB", memory_kb))
                    .show_ui(ui, |ui| {
                        for (kb, label) in [
                            (64, "64 KB"),
                            (128, "128 KB"),
                            (256, "256 KB"),
                            (512, "512 KB"),
                            (640, "640 KB"),
                            (1024, "1024 KB (1 MB)"),
                            (2048, "2048 KB (2 MB)"),
                            (4096, "4096 KB (4 MB)"),
                            (8192, "8192 KB (8 MB)"),
                            (16384, "16384 KB (16 MB)"),
                        ] {
                            ui.selectable_value(memory_kb, kb, label);
                        }
                    });
            }
        }
    }

    fn render_controller_settings(&mut self, ui: &mut Ui) {
        ui.add_space(3.0);

        // Input source selection
        ui.horizontal(|ui| {
            ui.label("Config Source:");
            ui.add_space(5.0);
            if ui
                .selectable_label(
                    self.input_config_source == InputConfigSource::Global,
                    "Global",
                )
                .on_hover_text("Use global config.json settings")
                .clicked()
                && self.input_config_source != InputConfigSource::Global
            {
                self.pending_action =
                    Some(PropertyAction::SetInputSource(InputConfigSource::Global));
            }
            if ui
                .selectable_label(
                    self.input_config_source == InputConfigSource::Project,
                    "Project",
                )
                .on_hover_text("Use project-specific .hemu file settings")
                .clicked()
                && self.input_config_source != InputConfigSource::Project
            {
                self.pending_action =
                    Some(PropertyAction::SetInputSource(InputConfigSource::Project));
            }
        });

        // Input device status
        ui.add_space(8.0);
        egui::Grid::new("input_devices")
            .num_columns(2)
            .spacing([8.0, 3.0])
            .show(ui, |ui| {
                ui.label("Gamepads:");
                ui.label(format!("{} detected", self.num_gamepads_detected));
                ui.end_row();

                ui.label("Joysticks:");
                ui.label(format!("{} detected", self.num_joysticks_detected));
                ui.end_row();
            });

        // Player configuration
        ui.add_space(8.0);
        ui.checkbox(&mut self.player1_enabled, "Player 1 Enabled");
        ui.checkbox(&mut self.player2_enabled, "Player 2 Enabled");

        // Mouse configuration
        ui.add_space(8.0);
        ui.checkbox(&mut self.mouse_enabled, "Mouse Input Enabled");
        if self.mouse_enabled {
            ui.add_space(3.0);
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
    }

    fn render_mount_points(&mut self, ui: &mut Ui) {
        ui.add_space(3.0);

        if self.mount_points.is_empty() {
            ui.label(egui::RichText::new("No mount points available").italics());
            ui.add_space(3.0);
            ui.label(
                egui::RichText::new("Create a system or load a ROM to see mount points")
                    .small()
                    .italics()
                    .weak(),
            );
        } else {
            for mount in &self.mount_points {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(format!("{}:", mount.name)).strong());
                    if let Some(ref file) = mount.mounted_file {
                        let filename = std::path::Path::new(file)
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or(file);
                        ui.label(filename).on_hover_text(file);
                        if ui
                            .button("Eject")
                            .on_hover_text(format!("Unmount {} from {}", filename, mount.name))
                            .clicked()
                        {
                            self.pending_action = Some(PropertyAction::EjectFile(mount.id.clone()));
                        }
                    } else if ui
                        .button("Mount...")
                        .on_hover_text(format!("Load a file to mount in {}", mount.name))
                        .clicked()
                    {
                        self.pending_action = Some(PropertyAction::MountFile(mount.id.clone()));
                    }
                });
                ui.add_space(2.0);
            }
        }
    }
}

impl Default for PropertyPane {
    fn default() -> Self {
        Self::new()
    }
}
