//! Main egui application layout

use super::menu_bar::MenuBar;
use super::property_pane::PropertyPane;
use super::status_bar::StatusBarWidget;
use super::tabs::TabManager;
use crate::settings::ScalingMode;
use egui::{CentralPanel, Context, SidePanel, TopBottomPanel};

/// Convert linear color component (0-255) to sRGB color space (0-255)
/// This compensates for GL_FRAMEBUFFER_SRGB incorrectly treating texture colors as linear
#[inline]
fn linear_to_srgb(linear: u8) -> u8 {
    let linear_f = linear as f32 / 255.0;
    let srgb_f = if linear_f <= 0.0031308 {
        linear_f * 12.92
    } else {
        1.055 * linear_f.powf(1.0 / 2.4) - 0.055
    };
    (srgb_f * 255.0).round().min(255.0) as u8
}

/// Helper to create egui Color32, applying inverse gamma to compensate for GL_FRAMEBUFFER_SRGB
#[inline]
fn color_from_rgb(r: u8, g: u8, b: u8) -> egui::Color32 {
    egui::Color32::from_rgb(linear_to_srgb(r), linear_to_srgb(g), linear_to_srgb(b))
}

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
        // Apply inverse gamma to compensate for GL_FRAMEBUFFER_SRGB
        // GL_FRAMEBUFFER_SRGB treats all colors as linear and converts to sRGB,
        // so we pre-apply gamma to cancel out that conversion
        let rgba_pixels: Vec<u8> = pixels
            .iter()
            .flat_map(|&pixel| {
                let a = ((pixel >> 24) & 0xFF) as u8;
                let r = ((pixel >> 16) & 0xFF) as u8;
                let g = ((pixel >> 8) & 0xFF) as u8;
                let b = (pixel & 0xFF) as u8;

                // Apply inverse gamma (linear→sRGB) to compensate for GL_FRAMEBUFFER_SRGB
                let r_corrected = linear_to_srgb(r);
                let g_corrected = linear_to_srgb(g);
                let b_corrected = linear_to_srgb(b);

                [r_corrected, g_corrected, b_corrected, a]
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
        // Set brighter text color globally
        let mut style = (*ctx.style()).clone();
        style.visuals.override_text_color = Some(color_from_rgb(204, 204, 204));
        // Brighter widget text colors
        style.visuals.widgets.noninteractive.fg_stroke.color = color_from_rgb(204, 204, 204);
        style.visuals.widgets.inactive.fg_stroke.color = color_from_rgb(204, 204, 204);
        style.visuals.widgets.hovered.fg_stroke.color = color_from_rgb(255, 255, 255);
        style.visuals.widgets.active.fg_stroke.color = color_from_rgb(255, 255, 255);
        ctx.set_style(style);

        // Top menu bar - VS Code menu bar color RGB(24,24,24)
        TopBottomPanel::top("menu_bar")
            .frame(egui::Frame::none().fill(color_from_rgb(24, 24, 24)))
            .show(ctx, |ui| {
                self.menu_bar.ui(ui);
            });

        // Bottom status bar - VS Code lighter area RGB(31,31,31)
        TopBottomPanel::bottom("status_bar")
            .frame(egui::Frame::none().fill(color_from_rgb(31, 31, 31)))
            .show(ctx, |ui| {
                self.status_bar.ui(ui);
            });

        // Right property pane - RGB(12,12,12)
        SidePanel::right("property_pane")
            .default_width(300.0)
            .min_width(200.0)
            .max_width(500.0)
            .resizable(true)
            .frame(egui::Frame::none().fill(color_from_rgb(12, 12, 12)))
            .show(ctx, |ui| {
                self.property_pane.ui(ui);
            });

        // Central tabbed interface - pitch black for emulator display
        CentralPanel::default()
            .frame(egui::Frame::none().fill(color_from_rgb(0, 0, 0)))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_srgb_to_linear_conversion() {
        // Test black (should stay black)
        assert_eq!(srgb_to_linear(0), 0);

        // Test white (should stay white)
        assert_eq!(srgb_to_linear(255), 255);

        // Test middle gray (sRGB 128 should convert to darker linear value)
        // sRGB 128/255 = 0.502, linear = ((0.502 + 0.055) / 1.055)^2.4 ≈ 0.214
        // linear 0.214 * 255 ≈ 55
        let result = srgb_to_linear(128);
        assert!((53..=57).contains(&result), "Expected ~55, got {}", result);

        // Test common sRGB value (187 is common in UI)
        // Should convert to a brighter linear value
        let result = srgb_to_linear(187);
        assert!(result > 100, "Linear value should be > 100 for sRGB 187");
    }
}
