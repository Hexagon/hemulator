//! Selector popup windows for choosing options
//!
//! This module provides a modular framework for selection dialogs
//! (slot selector, speed selector, disk format selector, etc.)

use crate::ui_render;

/// Selector window type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum SelectorType {
    SaveSlot,
    LoadSlot,
    Speed,
    DiskFormat,
}

/// Generic selector window for choosing from a list of options
#[allow(dead_code)]
pub struct SelectorWindow {
    pub selector_type: SelectorType,
    pub selected_index: Option<usize>,
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

impl SelectorWindow {
    pub fn new(selector_type: SelectorType) -> Self {
        Self {
            selector_type,
            selected_index: None,
            x: 0,
            y: 0,
            width: 0,
            height: 0,
        }
    }

    /// Handle keyboard input for selector
    /// Returns Some(index) if an option was selected, None if cancelled
    pub fn handle_key(&mut self, key: char) -> Option<Option<usize>> {
        match self.selector_type {
            SelectorType::SaveSlot | SelectorType::LoadSlot => {
                if ('1'..='5').contains(&key) {
                    let index = (key as u8 - b'1') as usize;
                    Some(Some(index))
                } else {
                    None
                }
            }
            SelectorType::Speed => {
                if ('0'..='5').contains(&key) {
                    let index = (key as u8 - b'0') as usize;
                    Some(Some(index))
                } else {
                    None
                }
            }
            SelectorType::DiskFormat => {
                if ('1'..='9').contains(&key) {
                    let index = (key as u8 - b'1') as usize;
                    Some(Some(index))
                } else {
                    None
                }
            }
        }
    }

    /// Render the selector window
    pub fn render(&mut self, width: usize, height: usize, current_speed: f64) -> Vec<u32> {
        let win_width = width.min(500);
        let win_height = height.min(400);
        let x_offset = (width - win_width) / 2;
        let y_offset = (height - win_height) / 2;

        // Store dimensions for hit detection
        self.x = x_offset;
        self.y = y_offset;
        self.width = win_width;
        self.height = win_height;

        // Create window background
        let mut buffer = vec![0x00000000; width * height];

        // Draw semi-transparent overlay behind window
        for y in 0..height {
            for x in 0..width {
                buffer[y * width + x] = 0xC0000000;
            }
        }

        // Draw window background
        for y in y_offset..y_offset + win_height {
            for x in x_offset..x_offset + win_width {
                if y < height && x < width {
                    buffer[y * width + x] = 0xFF1E1E2E;
                }
            }
        }

        // Render based on selector type
        match self.selector_type {
            SelectorType::SaveSlot => {
                self.render_slot_selector(&mut buffer, width, height, x_offset, y_offset, "SAVE");
            }
            SelectorType::LoadSlot => {
                self.render_slot_selector(&mut buffer, width, height, x_offset, y_offset, "LOAD");
            }
            SelectorType::Speed => {
                self.render_speed_selector(
                    &mut buffer,
                    width,
                    height,
                    x_offset,
                    y_offset,
                    current_speed,
                );
            }
            SelectorType::DiskFormat => {
                self.render_disk_format_selector(&mut buffer, width, height, x_offset, y_offset);
            }
        }

        buffer
    }

    fn render_slot_selector(
        &self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        x_offset: usize,
        y_offset: usize,
        mode: &str,
    ) {
        let title = format!("SELECT SLOT TO {} (1-5)", mode);
        let line_height = 12;
        let mut y = y_offset + 10;

        ui_render::draw_text(buffer, width, height, &title, x_offset + 10, y, 0xFFEBDBB2);
        y += line_height * 2;

        for i in 1..=5 {
            let text = format!("  {} - Slot {}", i, i);
            ui_render::draw_text(buffer, width, height, &text, x_offset + 10, y, 0xFFEBDBB2);
            y += line_height;
        }

        y += line_height;
        ui_render::draw_text(
            buffer,
            width,
            height,
            "Press 1-5 to select, ESC to cancel",
            x_offset + 10,
            y,
            0xFF928374,
        );
    }

    fn render_speed_selector(
        &self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        x_offset: usize,
        y_offset: usize,
        current_speed: f64,
    ) {
        let title = "EMULATION SPEED - Select (0-5)";
        let line_height = 12;
        let mut y = y_offset + 10;

        ui_render::draw_text(buffer, width, height, title, x_offset + 10, y, 0xFFEBDBB2);
        y += line_height * 2;

        let speeds = [
            (0.0, "0 - Pause (0x)"),
            (0.25, "1 - Slow Motion (0.25x)"),
            (0.5, "2 - Half Speed (0.5x)"),
            (1.0, "3 - Normal (1x)"),
            (2.0, "4 - Fast Forward (2x)"),
            (10.0, "5 - Turbo (10x)"),
        ];

        for (speed_value, label) in &speeds {
            let marker = if (*speed_value - current_speed).abs() < 0.01 {
                ">"
            } else {
                " "
            };
            let text = format!("{} {}", marker, label);
            ui_render::draw_text(buffer, width, height, &text, x_offset + 10, y, 0xFFEBDBB2);
            y += line_height;
        }

        y += line_height;
        ui_render::draw_text(
            buffer,
            width,
            height,
            "Press 0-5 to select, ESC to cancel",
            x_offset + 10,
            y,
            0xFF928374,
        );
    }

    fn render_disk_format_selector(
        &self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        x_offset: usize,
        y_offset: usize,
    ) {
        let title = "SELECT DISK FORMAT";
        let line_height = 12;
        let mut y = y_offset + 10;

        ui_render::draw_text(buffer, width, height, title, x_offset + 10, y, 0xFFEBDBB2);
        y += line_height * 2;

        let formats = [
            "1 - 360KB 5.25\" Floppy",
            "2 - 720KB 3.5\" Floppy",
            "3 - 1.2MB 5.25\" Floppy",
            "4 - 1.44MB 3.5\" Floppy",
            "5 - 10MB Hard Disk",
            "6 - 20MB Hard Disk",
        ];

        for format in &formats {
            ui_render::draw_text(buffer, width, height, format, x_offset + 10, y, 0xFFEBDBB2);
            y += line_height;
        }

        y += line_height;
        ui_render::draw_text(
            buffer,
            width,
            height,
            "Press number to select, ESC to cancel",
            x_offset + 10,
            y,
            0xFF928374,
        );
    }
}

/// Selector manager to handle all selector windows
pub struct SelectorManager {
    pub active_selector: Option<SelectorWindow>,
}

impl SelectorManager {
    pub fn new() -> Self {
        Self {
            active_selector: None,
        }
    }

    #[allow(dead_code)]
    pub fn show_save_slot_selector(&mut self) {
        self.active_selector = Some(SelectorWindow::new(SelectorType::SaveSlot));
    }

    #[allow(dead_code)]
    pub fn show_load_slot_selector(&mut self) {
        self.active_selector = Some(SelectorWindow::new(SelectorType::LoadSlot));
    }

    #[allow(dead_code)]
    pub fn show_speed_selector(&mut self) {
        self.active_selector = Some(SelectorWindow::new(SelectorType::Speed));
    }

    #[allow(dead_code)]
    pub fn show_disk_format_selector(&mut self) {
        self.active_selector = Some(SelectorWindow::new(SelectorType::DiskFormat));
    }

    pub fn close(&mut self) {
        self.active_selector = None;
    }

    pub fn is_open(&self) -> bool {
        self.active_selector.is_some()
    }

    /// Handle keyboard input - returns Some(Some(index)) if selection made,
    /// Some(None) if cancelled, None if key not handled
    #[allow(dead_code)]
    pub fn handle_key(&mut self, key: char) -> Option<Option<usize>> {
        if let Some(ref mut selector) = self.active_selector {
            selector.handle_key(key)
        } else {
            None
        }
    }

    /// Render the active selector if any
    #[allow(dead_code)]
    pub fn render(&mut self, width: usize, height: usize, current_speed: f64) -> Option<Vec<u32>> {
        self.active_selector.as_mut().map(|selector| selector.render(width, height, current_speed))
    }
}

impl Default for SelectorManager {
    fn default() -> Self {
        Self::new()
    }
}
