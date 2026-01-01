//! Main egui application layout

use super::menu_bar::MenuBar;
use super::property_pane::PropertyPane;
use super::status_bar::StatusBarWidget;
use super::tabs::TabManager;
use crate::settings::ScalingMode;
use egui::{CentralPanel, Context, SidePanel, TopBottomPanel};

/// Main egui application state
pub struct EguiApp {
    pub menu_bar: MenuBar,
    pub tab_manager: TabManager,
    pub property_pane: PropertyPane,
    pub status_bar: StatusBarWidget,

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

    /// Render the UI
    pub fn ui(&mut self, ctx: &Context, scaling_mode: ScalingMode) {
        // Top menu bar
        TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            self.menu_bar.ui(ui);
        });

        // Bottom status bar
        TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            self.status_bar.ui(ui);
        });

        // Right property pane
        SidePanel::right("property_pane")
            .default_width(300.0)
            .min_width(200.0)
            .max_width(500.0)
            .resizable(true)
            .show(ctx, |ui| {
                self.property_pane.ui(ui);
            });

        // Central tabbed interface
        CentralPanel::default().show(ctx, |ui| {
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
