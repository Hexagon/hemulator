//! Popup window system for debug tools, help, and other UI elements
//!
//! This module provides a framework for managing popup windows within the main application window.
//! Each popup window is rendered as an overlay with its own event handling and content.

use crate::ui_render;

/// Popup window identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PopupWindowId {
    Debug,
    Help,
}

/// Tab identifier for debug window
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugTab {
    Logs,
    Memory,
    Graphics,
    Cpu,
    BusInterrupts,
}

impl DebugTab {
    pub fn label(&self) -> &'static str {
        match self {
            DebugTab::Logs => "Logs",
            DebugTab::Memory => "Memory",
            DebugTab::Graphics => "Graphics",
            DebugTab::Cpu => "CPU",
            DebugTab::BusInterrupts => "Bus/IRQ",
        }
    }

    pub fn all() -> &'static [DebugTab] {
        &[
            DebugTab::Logs,
            DebugTab::Memory,
            DebugTab::Graphics,
            DebugTab::Cpu,
            DebugTab::BusInterrupts,
        ]
    }
}

/// Debug window state
pub struct DebugWindow {
    pub active_tab: DebugTab,
    pub log_level: String,
    pub log_scope: String,
    pub memory_address: usize,
    pub scroll_offset: usize,
}

impl DebugWindow {
    pub fn new() -> Self {
        Self {
            active_tab: DebugTab::Cpu,
            log_level: "info".to_string(),
            log_scope: "all".to_string(),
            memory_address: 0,
            scroll_offset: 0,
        }
    }

    /// Render the debug window content
    pub fn render(
        &self,
        width: usize,
        height: usize,
        debug_info: Option<&dyn DebugInfo>,
        fps: f64,
        video_backend: &str,
    ) -> Vec<u32> {
        let win_width = width.min(800);
        let win_height = height.min(600);
        let x_offset = (width - win_width) / 2;
        let y_offset = (height - win_height) / 2;

        // Create window background
        let mut buffer = vec![0x00000000; width * height];

        // Draw semi-transparent overlay behind window
        for y in 0..height {
            for x in 0..width {
                buffer[y * width + x] = 0x80000000;
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

        // Draw window title bar
        let title_height = 30;
        for y in y_offset..y_offset + title_height {
            for x in x_offset..x_offset + win_width {
                if y < height && x < width {
                    buffer[y * width + x] = 0xFF282828;
                }
            }
        }

        // Draw title text
        ui_render::draw_text(
            &mut buffer,
            width,
            height,
            "Debug Window",
            x_offset + 10,
            y_offset + 11,
            0xFFEBDBB2,
        );

        // Draw tabs
        let tab_y = y_offset + title_height;
        let tab_height = 25;
        let tab_width = win_width / DebugTab::all().len();

        for (i, tab) in DebugTab::all().iter().enumerate() {
            let tab_x = x_offset + i * tab_width;
            let is_active = *tab == self.active_tab;
            let tab_color = if is_active { 0xFF3C3836 } else { 0xFF282828 };

            for y in tab_y..tab_y + tab_height {
                for x in tab_x..tab_x + tab_width {
                    if y < height && x < width {
                        buffer[y * width + x] = tab_color;
                    }
                }
            }

            // Draw tab label
            ui_render::draw_text(
                &mut buffer,
                width,
                height,
                tab.label(),
                tab_x + 10,
                tab_y + 8,
                if is_active { 0xFFEBDBB2 } else { 0xFF928374 },
            );
        }

        // Draw tab content based on active tab
        let content_y = tab_y + tab_height + 10;
        match self.active_tab {
            DebugTab::Cpu => {
                self.render_cpu_tab(
                    &mut buffer,
                    width,
                    height,
                    x_offset,
                    content_y,
                    win_width,
                    debug_info,
                );
            }
            DebugTab::Memory => {
                self.render_memory_tab(&mut buffer, width, height, x_offset, content_y, win_width);
            }
            DebugTab::Graphics => {
                self.render_graphics_tab(
                    &mut buffer,
                    width,
                    height,
                    x_offset,
                    content_y,
                    win_width,
                    debug_info,
                );
            }
            DebugTab::Logs => {
                self.render_logs_tab(&mut buffer, width, height, x_offset, content_y, win_width);
            }
            DebugTab::BusInterrupts => {
                self.render_bus_tab(
                    &mut buffer,
                    width,
                    height,
                    x_offset,
                    content_y,
                    win_width,
                    debug_info,
                );
            }
        }

        buffer
    }

    fn render_cpu_tab(
        &self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        x_offset: usize,
        y_offset: usize,
        _win_width: usize,
        debug_info: Option<&dyn DebugInfo>,
    ) {
        let mut y = y_offset;
        let line_height = 12;

        if let Some(info) = debug_info {
            let lines = info.get_cpu_lines();
            for line in lines {
                ui_render::draw_text(buffer, width, height, line, x_offset + 10, y, 0xFFEBDBB2);
                y += line_height;
                if y + line_height > height {
                    break;
                }
            }
        } else {
            ui_render::draw_text(
                buffer,
                width,
                height,
                "No debug info available",
                x_offset + 10,
                y,
                0xFF928374,
            );
        }
    }

    fn render_memory_tab(
        &self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        x_offset: usize,
        y_offset: usize,
        _win_width: usize,
    ) {
        let y = y_offset;
        ui_render::draw_text(
            buffer,
            width,
            height,
            &format!("Memory View - Address: ${:04X}", self.memory_address),
            x_offset + 10,
            y,
            0xFFEBDBB2,
        );
        // TODO: Implement memory dump viewer
    }

    fn render_graphics_tab(
        &self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        x_offset: usize,
        y_offset: usize,
        _win_width: usize,
        debug_info: Option<&dyn DebugInfo>,
    ) {
        let mut y = y_offset;
        let line_height = 12;

        if let Some(info) = debug_info {
            let lines = info.get_graphics_lines();
            for line in lines {
                ui_render::draw_text(buffer, width, height, line, x_offset + 10, y, 0xFFEBDBB2);
                y += line_height;
                if y + line_height > height {
                    break;
                }
            }
        } else {
            ui_render::draw_text(
                buffer,
                width,
                height,
                "No graphics debug info available",
                x_offset + 10,
                y,
                0xFF928374,
            );
        }
    }

    fn render_logs_tab(
        &self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        x_offset: usize,
        y_offset: usize,
        _win_width: usize,
    ) {
        let y = y_offset;
        ui_render::draw_text(
            buffer,
            width,
            height,
            &format!(
                "Log Viewer - Level: {} Scope: {}",
                self.log_level, self.log_scope
            ),
            x_offset + 10,
            y,
            0xFFEBDBB2,
        );
        // TODO: Implement log viewer with filtering
    }

    fn render_bus_tab(
        &self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        x_offset: usize,
        y_offset: usize,
        _win_width: usize,
        debug_info: Option<&dyn DebugInfo>,
    ) {
        let mut y = y_offset;
        let line_height = 12;

        if let Some(info) = debug_info {
            let lines = info.get_bus_lines();
            for line in lines {
                ui_render::draw_text(buffer, width, height, line, x_offset + 10, y, 0xFFEBDBB2);
                y += line_height;
                if y + line_height > height {
                    break;
                }
            }
        } else {
            ui_render::draw_text(
                buffer,
                width,
                height,
                "No bus/interrupt debug info available",
                x_offset + 10,
                y,
                0xFF928374,
            );
        }
    }
}

impl Default for DebugWindow {
    fn default() -> Self {
        Self::new()
    }
}

/// Help window state
pub struct HelpWindow;

impl HelpWindow {
    pub fn new() -> Self {
        Self
    }

    /// Render the help window content
    pub fn render(
        &self,
        width: usize,
        height: usize,
        settings: &crate::settings::Settings,
    ) -> Vec<u32> {
        let win_width = width.min(700);
        let win_height = height.min(550);
        let x_offset = (width - win_width) / 2;
        let y_offset = (height - win_height) / 2;

        // Create window background
        let mut buffer = vec![0x00000000; width * height];

        // Draw semi-transparent overlay behind window
        for y in 0..height {
            for x in 0..width {
                buffer[y * width + x] = 0x80000000;
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

        // Draw window title bar
        let title_height = 30;
        for y in y_offset..y_offset + title_height {
            for x in x_offset..x_offset + win_width {
                if y < height && x < width {
                    buffer[y * width + x] = 0xFF282828;
                }
            }
        }

        // Draw title text
        ui_render::draw_text(
            &mut buffer,
            width,
            height,
            "Help - HEMULATOR",
            x_offset + 10,
            y_offset + 11,
            0xFFEBDBB2,
        );

        // Draw help content
        let content_y = y_offset + title_height + 10;
        let line_height = 12;
        let mut y = content_y;

        // Player 1 controls
        let p1_a = format!("  {} - A", settings.input.player1.a);
        let p1_b = format!("  {} - B", settings.input.player1.b);
        let p1_select = format!("  {} - Select", settings.input.player1.select);
        let p1_start = format!("  {} - Start", settings.input.player1.start);
        let p1_dpad = format!(
            "  {} {} {} {} - D-pad",
            settings.input.player1.up,
            settings.input.player1.down,
            settings.input.player1.left,
            settings.input.player1.right
        );

        let help_lines: Vec<String> = vec![
            "Player 1 Controller:".to_string(),
            p1_a,
            p1_b,
            p1_select,
            p1_start,
            p1_dpad,
            "".to_string(),
            "Keyboard Shortcuts:".to_string(),
            "  Ctrl+O     - Open ROM".to_string(),
            "  Ctrl+S     - Save project".to_string(),
            "  Ctrl+R     - Reset system".to_string(),
            "  Ctrl+P     - Pause/Resume".to_string(),
            "  Ctrl+1-5   - Save state (slots 1-5)".to_string(),
            "  Ctrl+Shift+1-5 - Load state (slots 1-5)".to_string(),
            "  F1         - Help (this window)".to_string(),
            "  F4         - Screenshot".to_string(),
            "  F10        - Debug window".to_string(),
            "  F11        - CRT filter".to_string(),
            "  ESC        - Close window/Exit".to_string(),
            "".to_string(),
            "For PC systems: Hold Right Alt for shortcuts".to_string(),
            "".to_string(),
            "Press ESC or F1 to close this window".to_string(),
        ];

        for line in &help_lines {
            if y + line_height > y_offset + win_height - 10 {
                break;
            }
            ui_render::draw_text(
                &mut buffer,
                width,
                height,
                line,
                x_offset + 10,
                y,
                0xFFEBDBB2,
            );
            y += line_height;
        }

        buffer
    }
}

impl Default for HelpWindow {
    fn default() -> Self {
        Self::new()
    }
}

/// Popup window manager
pub struct PopupWindowManager {
    pub debug_window: Option<DebugWindow>,
    pub help_window: Option<HelpWindow>,
}

impl PopupWindowManager {
    pub fn new() -> Self {
        Self {
            debug_window: None,
            help_window: None,
        }
    }

    pub fn is_debug_open(&self) -> bool {
        self.debug_window.is_some()
    }

    pub fn is_help_open(&self) -> bool {
        self.help_window.is_some()
    }

    pub fn toggle_debug(&mut self) {
        if self.debug_window.is_some() {
            self.debug_window = None;
        } else {
            self.debug_window = Some(DebugWindow::new());
        }
    }

    pub fn toggle_help(&mut self) {
        if self.help_window.is_some() {
            self.help_window = None;
        } else {
            self.help_window = Some(HelpWindow::new());
        }
    }

    pub fn close_all(&mut self) {
        self.debug_window = None;
        self.help_window = None;
    }

    /// Returns true if any popup is open
    pub fn has_open_popup(&self) -> bool {
        self.debug_window.is_some() || self.help_window.is_some()
    }
}

impl Default for PopupWindowManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Unified debug info trait for rendering
pub trait DebugInfo {
    fn get_cpu_lines(&self) -> Vec<&str>;
    fn get_graphics_lines(&self) -> Vec<&str>;
    fn get_bus_lines(&self) -> Vec<&str>;
}

// Implement DebugInfo for different system debug info types
impl DebugInfo for emu_nes::DebugInfo {
    fn get_cpu_lines(&self) -> Vec<&str> {
        vec![] // TODO: Extract CPU state from NES debug info
    }

    fn get_graphics_lines(&self) -> Vec<&str> {
        vec![] // TODO: Extract graphics state from NES debug info
    }

    fn get_bus_lines(&self) -> Vec<&str> {
        vec![] // TODO: Extract bus state from NES debug info
    }
}
