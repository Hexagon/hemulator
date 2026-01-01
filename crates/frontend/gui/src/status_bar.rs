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
    pub ip: Option<u32>,              // Instruction pointer
    pub cycles: Option<u64>,          // Cycle count
    pub rendering_backend: String,    // "Software" or "OpenGL"
    pub cpu_freq_target: Option<f64>, // Target CPU frequency in MHz
    pub cpu_freq_actual: Option<f64>, // Actual CPU frequency in MHz
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
            rendering_backend: "Software".to_string(),
            cpu_freq_target: None,
            cpu_freq_actual: None,
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

        // Rendering backend indicator
        if !self.rendering_backend.is_empty() {
            right_parts.push(self.rendering_backend.clone());
        }

        // CPU frequency info (target and actual)
        if let Some(target) = self.cpu_freq_target {
            if let Some(actual) = self.cpu_freq_actual {
                // Show both target and actual if they differ significantly
                if (target - actual).abs() > 0.1 {
                    right_parts.push(format!("{:.1}/{:.1}MHz", actual, target));
                } else {
                    right_parts.push(format!("{:.1}MHz", target));
                }
            } else {
                right_parts.push(format!("{:.1}MHz", target));
            }
        }

        // Only show FPS if it's meaningful
        if self.fps > 0.1 {
            right_parts.push(format!("{:.0}fps", self.fps));
        }

        if let Some(ip) = self.ip {
            // Format IP based on its size
            // Use appropriate number of hex digits: 4 for 16-bit, 6 for 24-bit, 8 for 32-bit
            let ip_str = if ip > 0xFFFFFF {
                format!("${:08X}", ip) // 32-bit address (8 hex digits)
            } else if ip > 0xFFFF {
                format!("${:06X}", ip) // 24-bit address (6 hex digits)
            } else {
                format!("${:04X}", ip) // 16-bit address (4 hex digits)
            };
            right_parts.push(ip_str);
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
    #[allow(dead_code)]
    pub fn height() -> usize {
        STATUS_BAR_HEIGHT
    }
}

impl Default for StatusBar {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_bar_new() {
        let status_bar = StatusBar::new();
        assert_eq!(status_bar.message, "");
        assert_eq!(status_bar.fps, 0.0);
        assert_eq!(status_bar.system_name, "");
        assert!(!status_bar.paused);
        assert_eq!(status_bar.speed, 1.0);
        assert_eq!(status_bar.ip, None);
        assert_eq!(status_bar.cycles, None);
        assert_eq!(status_bar.rendering_backend, "Software");
        assert_eq!(status_bar.cpu_freq_target, None);
        assert_eq!(status_bar.cpu_freq_actual, None);
    }

    #[test]
    fn test_status_bar_ip_formatting() {
        let mut status_bar = StatusBar::new();

        // Test 16-bit IP (4 hex digits)
        status_bar.ip = Some(0x1234);
        let mut buffer = vec![0; 800 * 600];
        status_bar.render(&mut buffer, 800, 600);

        // Test 24-bit IP (6 hex digits)
        status_bar.ip = Some(0x123456);
        status_bar.render(&mut buffer, 800, 600);

        // Test 32-bit IP (8 hex digits)
        status_bar.ip = Some(0x12345678);
        status_bar.render(&mut buffer, 800, 600);
    }

    #[test]
    fn test_status_bar_cpu_freq() {
        let mut status_bar = StatusBar::new();

        // Test target frequency only
        status_bar.cpu_freq_target = Some(4.77);
        let mut buffer = vec![0; 800 * 600];
        status_bar.render(&mut buffer, 800, 600);

        // Test target and actual frequencies
        status_bar.cpu_freq_target = Some(4.77);
        status_bar.cpu_freq_actual = Some(4.75);
        status_bar.render(&mut buffer, 800, 600);
    }

    #[test]
    fn test_status_bar_rendering_backend() {
        let mut status_bar = StatusBar::new();

        // Test Software backend
        status_bar.rendering_backend = "Software".to_string();
        let mut buffer = vec![0; 800 * 600];
        status_bar.render(&mut buffer, 800, 600);

        // Test OpenGL backend
        status_bar.rendering_backend = "OpenGL".to_string();
        status_bar.render(&mut buffer, 800, 600);
    }
}
