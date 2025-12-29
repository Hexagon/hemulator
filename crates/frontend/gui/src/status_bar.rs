//! Status bar rendering at the bottom of the window

use crate::ui_render;

const STATUS_BAR_HEIGHT: usize = 20;

/// Status bar information to display
pub struct StatusBar {
    pub message: String,
    pub fps: f32,
    pub system_name: String,
    pub paused: bool,
    pub speed: f32,
}

impl StatusBar {
    pub fn new() -> Self {
        Self {
            message: String::new(),
            fps: 0.0,
            system_name: String::new(),
            paused: false,
            speed: 1.0,
        }
    }

    /// Render the status bar at the bottom of the buffer
    /// Returns the height of the status bar in pixels
    pub fn render(&self, buffer: &mut [u32], width: usize, height: usize) -> usize {
        if height < STATUS_BAR_HEIGHT {
            return 0; // Not enough space for status bar
        }

        let bar_y = height - STATUS_BAR_HEIGHT;

        // Draw background for status bar (dark gray)
        for y in bar_y..height {
            for x in 0..width {
                let idx = y * width + x;
                buffer[idx] = 0xFF2A2A3E; // Dark gray/purple background
            }
        }

        // Left side: System name and state
        let mut left_text = self.system_name.clone();
        if self.paused {
            left_text.push_str(" [PAUSED]");
        } else if self.speed != 1.0 {
            left_text.push_str(&format!(" [{}%]", (self.speed * 100.0) as u32));
        }

        ui_render::draw_text(
            buffer,
            width,
            height,
            &left_text,
            4,
            bar_y + 6,
            0xFFFFFFFF, // White text
        );

        // Center: Status message (if any)
        if !self.message.is_empty() {
            let msg_width = self.message.len() * 8;
            let msg_x = (width.saturating_sub(msg_width)) / 2;
            ui_render::draw_text(
                buffer,
                width,
                height,
                &self.message,
                msg_x,
                bar_y + 6,
                0xFF16F2B3, // Cyan/green text for messages
            );
        }

        // Right side: FPS
        let fps_text = format!("{:.1} FPS", self.fps);
        let fps_width = fps_text.len() * 8;
        let fps_x = width.saturating_sub(fps_width + 4);
        ui_render::draw_text(
            buffer,
            width,
            height,
            &fps_text,
            fps_x,
            bar_y + 6,
            0xFFFFFFFF, // White text
        );

        STATUS_BAR_HEIGHT
    }

    /// Get the height of the status bar
    pub fn height() -> usize {
        STATUS_BAR_HEIGHT
    }
}

impl Default for StatusBar {
    fn default() -> Self {
        Self::new()
    }
}
