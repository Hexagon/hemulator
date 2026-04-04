//! Main egui application layout with docking support

use super::dock_layout::{DockLayout, InspectorTabViewer, PropertyTabViewer};
use super::menu_bar::MenuBar;
use super::property_pane::PropertyPane;
use super::status_bar::StatusBarWidget;
use super::tabs::TabManager;
use crate::settings::ScalingMode;
use egui::{CentralPanel, Context, Panel};
use egui_dock::{DockArea, Style};

/// Main egui application state
pub struct EguiApp {
    pub menu_bar: MenuBar,
    pub tab_manager: TabManager,
    pub property_pane: PropertyPane,
    pub status_bar: StatusBarWidget,
    pub dock_layout: DockLayout,

    /// Frame texture for emulator display
    pub emulator_texture: Option<egui::TextureHandle>,
}

impl EguiApp {
    pub fn new() -> Self {
        Self {
            menu_bar: MenuBar::new(),
            tab_manager: TabManager::new(),
            property_pane: PropertyPane::new(),
            status_bar: StatusBarWidget::new(),
            dock_layout: DockLayout::new(),
            emulator_texture: None,
        }
    }

    /// Update the emulator display texture
    pub fn update_emulator_texture(
        &mut self,
        ctx: &Context,
        pixels: &[u32],
        width: usize,
        height: usize,
    ) {
        // Convert ARGB to RGBA for egui
        // The egui_glow shader handles sRGB round-trip correctly:
        // texture sRGB → linear_from_srgba → linear → GL_FRAMEBUFFER_SRGB → sRGB
        // So we just upload the raw sRGB values directly
        let rgba_pixels: Vec<u8> = pixels
            .iter()
            .flat_map(|&pixel| {
                let a = ((pixel >> 24) & 0xFF) as u8;
                let r = ((pixel >> 16) & 0xFF) as u8;
                let g = ((pixel >> 8) & 0xFF) as u8;
                let b = (pixel & 0xFF) as u8;

                [r, g, b, a]
            })
            .collect();

        let color_image = egui::ColorImage::from_rgba_unmultiplied([width, height], &rgba_pixels);

        if let Some(texture) = &mut self.emulator_texture {
            texture.set(color_image, egui::TextureOptions::NEAREST);
        } else {
            self.emulator_texture = Some(ctx.load_texture(
                "emulator_frame",
                color_image,
                egui::TextureOptions::NEAREST,
            ));
        }
    }

    /// Update recent files list for the menu
    pub fn update_recent_files(&mut self, recent_files: Vec<String>) {
        self.menu_bar.set_recent_files(recent_files);
    }

    /// Update whether a system is currently loaded and its name
    pub fn set_system_loaded(&mut self, loaded: bool, system_name: &str) {
        self.menu_bar.set_system_loaded(loaded);
        self.tab_manager.system_loaded = loaded;
        self.tab_manager.system_name = system_name.to_string();
    }

    /// Render the UI
    #[allow(deprecated)] // Panel::show() is deprecated in egui 0.34 in favour of show_inside(),
                         // but show_inside() requires &mut Ui rather than &Context, which
                         // requires a larger refactor of the rendering pipeline.
    pub fn ui(&mut self, ctx: &Context, scaling_mode: ScalingMode) {
        let fg_color = egui::Color32::from_rgb(224, 224, 224);
        let white_color = egui::Color32::from_rgb(255, 255, 255);
        let panel_bg = egui::Color32::from_rgb(70, 70, 70);
        let dock_fill_color = egui::Color32::from_rgb(60, 60, 60);
        let main_bg_color = egui::Color32::from_rgb(0, 0, 0);

        // Set brighter text color globally
        let mut style = (*ctx.global_style()).clone();
        style.visuals.override_text_color = Some(fg_color);

        // Also brighten weak text color
        style.visuals.weak_text_alpha = 0.85;

        style.visuals.panel_fill = panel_bg;

        // Brighter widget text colors
        style.visuals.widgets.noninteractive.fg_stroke.color = fg_color;
        style.visuals.widgets.inactive.fg_stroke.color = fg_color;
        style.visuals.widgets.hovered.fg_stroke.color = white_color;
        style.visuals.widgets.active.fg_stroke.color = white_color;
        ctx.set_global_style(style);

        // Top menu bar
        Panel::top("menu_bar")
            .frame(egui::Frame::new().fill(panel_bg))
            .show(ctx, |ui| {
                self.menu_bar.ui(ui);
            });

        // Bottom status bar
        // Update status bar with current FPS from property pane
        self.status_bar.set_fps(self.property_pane.fps);
        Panel::bottom("status_bar")
            .frame(egui::Frame::new().fill(dock_fill_color))
            .show(ctx, |ui| {
                self.status_bar.ui(ui);
            });

        // Inspector dock at bottom (if visible)
        if self.dock_layout.inspector_visible {
            Panel::bottom("inspector_dock")
                .default_size(250.0)
                .min_size(100.0)
                .resizable(true)
                .frame(egui::Frame::new().fill(dock_fill_color))
                .show(ctx, |ui| {
                    let mut inspector_viewer = InspectorTabViewer {
                        tab_manager: &mut self.tab_manager,
                    };

                    DockArea::new(&mut self.dock_layout.inspector_state)
                        .style(Style::from_egui(ui.style().as_ref()))
                        .show_close_buttons(false)
                        .show_leaf_close_all_buttons(false)
                        .show_leaf_collapse_buttons(false)
                        .show_inside(ui, &mut inspector_viewer);
                });
        }

        // Property pane as a dockable right panel (only shown if any section visible)
        if self.property_pane.any_section_visible() {
            Panel::right("property_dock")
                .default_size(300.0)
                .min_size(200.0)
                .max_size(500.0)
                .resizable(true)
                .frame(egui::Frame::new().fill(dock_fill_color))
                .show(ctx, |ui| {
                    let mut property_viewer = PropertyTabViewer {
                        property_pane: &mut self.property_pane,
                    };

                    DockArea::new(&mut self.dock_layout.property_state)
                        .style(Style::from_egui(ui.style().as_ref()))
                        .show_close_buttons(false)
                        .show_leaf_close_all_buttons(false)
                        .show_leaf_collapse_buttons(false)
                        .show_inside(ui, &mut property_viewer);
                });
        }

        // Central panel with main tabs (Emulator, NewProject, Help, About)
        CentralPanel::default()
            .frame(egui::Frame::new().fill(main_bg_color))
            .show(ctx, |ui| {
                self.tab_manager
                    .ui(ui, &self.emulator_texture, scaling_mode);
            });
    }
}

impl Default for EguiApp {
    fn default() -> Self {
        Self::new()
    }
}
