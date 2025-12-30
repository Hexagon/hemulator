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

        // Draw background for status bar (dark gray)
        for y in bar_y..height {
            for x in 0..width {
                let idx = y * width + x;
                if idx < buffer.len() {
                    buffer[idx] = 0xFF2A2A3E; // Dark gray/purple background
                }
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

        // Right side: FPS, IP, and Cycles
        let mut right_parts = vec![format!("{:.1} FPS", self.fps)];

        if let Some(ip) = self.ip {
            right_parts.push(format!("IP:{:04X}", ip));
        }

        if let Some(cycles) = self.cycles {
            // Format cycles with commas for readability
            let cycles_str = if cycles >= 1_000_000 {
                format!("{}M", cycles / 1_000_000)
            } else if cycles >= 1_000 {
                format!("{}K", cycles / 1_000)
            } else {
                format!("{}", cycles)
            };
            right_parts.push(format!("Cyc:{}", cycles_str));
        }

        let right_text = right_parts.join(" | ");
        let right_width = right_text.len() * 8;
        let right_x = width.saturating_sub(right_width + 4);
        ui_render::draw_text(
            buffer,
            width,
            height,
            &right_text,
            right_x,
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
