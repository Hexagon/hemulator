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
    pub ip: Option<u32>,     // Instruction pointer
    pub cycles: Option<u64>, // Cycle count
}

impl StatusBar {
    pub fn new() -> Self {
        Self {
            message: String::new(),
            fps: 0.0,
            system_name: String::new(),
            paused: false,
            speed: 1.0,
            ip: None,
            cycles: None,
        }
    }

    /// Render the status bar at the bottom of the buffer
    /// Returns the height of the status bar in pixels
    pub fn render(&self, buffer: &mut [u32], width: usize, height: usize) -> usize {
        if height < STATUS_BAR_HEIGHT {
            return 0; // Not enough space for status bar
        }

        let bar_y = height - STATUS_BAR_HEIGHT;

        // Draw modern flat background (darker, more subtle)
        for y in bar_y..height {
            for x in 0..width {
                let idx = y * width + x;
                if idx < buffer.len() {
                    buffer[idx] = 0xFF1E1E2E; // Darker flat background
                }
            }
        }

        // Left side: State indicators (compact)
        let mut left_parts = Vec::new();

        if self.paused {
            left_parts.push("⏸ PAUSED".to_string());
        } else if self.speed != 1.0 {
            left_parts.push(format!("⏩ {}%", (self.speed * 100.0) as u32));
        }

        if !left_parts.is_empty() {
            let left_text = left_parts.join(" ");
            ui_render::draw_text(
                buffer,
                width,
                height,
                &left_text,
                8,
                bar_y + 6,
                0xFFFABD2F, // Warm yellow for status
            );
        }

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
                0xFF8EC07C, // Softer green for messages
            );
        }

        // Right side: Compact runtime stats
        let mut right_parts = Vec::new();

        // Only show FPS if it's meaningful
        if self.fps > 0.1 {
            right_parts.push(format!("{:.0}fps", self.fps));
        }

        if let Some(ip) = self.ip {
            right_parts.push(format!("${:04X}", ip));
        }

        if let Some(cycles) = self.cycles {
            let cycles_str = if cycles >= 1_000_000 {
                format!("{}M", cycles / 1_000_000)
            } else if cycles >= 1_000 {
                format!("{}K", cycles / 1_000)
            } else {
                format!("{}", cycles)
            };
            right_parts.push(cycles_str);
        }

        if !right_parts.is_empty() {
            let right_text = right_parts.join(" · ");
            let right_width = right_text.len() * 8;
            let right_x = width.saturating_sub(right_width + 8);
            ui_render::draw_text(
                buffer,
                width,
                height,
                &right_text,
                right_x,
                bar_y + 6,
                0xFFABABAB, // Muted gray for stats
            );
        }

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
