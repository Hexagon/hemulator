//! In-application menu bar (rendered overlay)

use crate::ui_render;

const MENU_BAR_HEIGHT: usize = 24;
const MENU_ITEM_WIDTH: usize = 80;

/// Menu actions that can be triggered
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuAction {
    // File menu
    NewProject,
    OpenRom,
    OpenProject,
    SaveProject,
    MountPoints,
    Exit,

    // Emulation menu
    Reset,
    Pause,
    Resume,
    Speed(SpeedSetting),

    // State menu
    SaveState(u8), // 1-5
    LoadState(u8), // 1-5

    // View menu
    Screenshot,
    DebugInfo,
    CrtFilterToggle,
    StartLogging,
    StopLogging,

    // Help menu
    Help,
    About,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpeedSetting {
    Percent25,
    Percent50,
    Percent100,
    Percent200,
    Percent400,
}

impl SpeedSetting {
    pub fn to_float(&self) -> f32 {
        match self {
            SpeedSetting::Percent25 => 0.25,
            SpeedSetting::Percent50 => 0.5,
            SpeedSetting::Percent100 => 1.0,
            SpeedSetting::Percent200 => 2.0,
            SpeedSetting::Percent400 => 4.0,
        }
    }
}

/// Menu item definition
#[derive(Clone)]
pub struct MenuItem {
    pub label: String,
    pub action: MenuAction,
    pub shortcut: Option<String>,
    pub enabled: bool,
}

/// Submenu definition
pub struct Submenu {
    pub label: String,
    pub items: Vec<MenuItem>,
}

/// Menu bar system
pub struct MenuBar {
    pub menus: Vec<Submenu>,
    pub active_menu: Option<usize>,
    pub visible: bool,
}

impl MenuBar {
    pub fn new() -> Self {
        let mut menus = Vec::new();

        // File menu
        menus.push(Submenu {
            label: "File".to_string(),
            items: vec![
                MenuItem {
                    label: "New Project...".to_string(),
                    action: MenuAction::NewProject,
                    shortcut: Some("Ctrl+N".to_string()),
                    enabled: true,
                },
                MenuItem {
                    label: "Open ROM...".to_string(),
                    action: MenuAction::OpenRom,
                    shortcut: Some("Ctrl+O".to_string()),
                    enabled: true,
                },
                MenuItem {
                    label: "Open Project...".to_string(),
                    action: MenuAction::OpenProject,
                    shortcut: Some("Ctrl+Shift+O".to_string()),
                    enabled: true,
                },
                MenuItem {
                    label: "Save Project...".to_string(),
                    action: MenuAction::SaveProject,
                    shortcut: Some("Ctrl+S".to_string()),
                    enabled: false, // Disabled until project loaded
                },
                MenuItem {
                    label: "Mount Points...".to_string(),
                    action: MenuAction::MountPoints,
                    shortcut: None,
                    enabled: false, // Disabled until system loaded
                },
                MenuItem {
                    label: "Exit".to_string(),
                    action: MenuAction::Exit,
                    shortcut: Some("Esc".to_string()),
                    enabled: true,
                },
            ],
        });

        // Emulation menu
        menus.push(Submenu {
            label: "Emulation".to_string(),
            items: vec![
                MenuItem {
                    label: "Reset".to_string(),
                    action: MenuAction::Reset,
                    shortcut: Some("Ctrl+R".to_string()),
                    enabled: false, // Disabled until system loaded
                },
                MenuItem {
                    label: "Pause".to_string(),
                    action: MenuAction::Pause,
                    shortcut: Some("Ctrl+P".to_string()),
                    enabled: false, // Disabled until running
                },
                MenuItem {
                    label: "Resume".to_string(),
                    action: MenuAction::Resume,
                    shortcut: Some("Ctrl+P".to_string()),
                    enabled: false, // Disabled until paused
                },
                MenuItem {
                    label: "Speed: 25%".to_string(),
                    action: MenuAction::Speed(SpeedSetting::Percent25),
                    shortcut: None,
                    enabled: false,
                },
                MenuItem {
                    label: "Speed: 50%".to_string(),
                    action: MenuAction::Speed(SpeedSetting::Percent50),
                    shortcut: None,
                    enabled: false,
                },
                MenuItem {
                    label: "Speed: 100%".to_string(),
                    action: MenuAction::Speed(SpeedSetting::Percent100),
                    shortcut: None,
                    enabled: false,
                },
                MenuItem {
                    label: "Speed: 200%".to_string(),
                    action: MenuAction::Speed(SpeedSetting::Percent200),
                    shortcut: None,
                    enabled: false,
                },
                MenuItem {
                    label: "Speed: 400%".to_string(),
                    action: MenuAction::Speed(SpeedSetting::Percent400),
                    shortcut: None,
                    enabled: false,
                },
            ],
        });

        // State menu
        let mut state_items = Vec::new();
        for i in 1..=5 {
            state_items.push(MenuItem {
                label: format!("Save State Slot {}", i),
                action: MenuAction::SaveState(i),
                shortcut: Some(format!("Ctrl+{}", i)),
                enabled: false, // Disabled until system with save state support loaded
            });
        }
        for i in 1..=5 {
            state_items.push(MenuItem {
                label: format!("Load State Slot {}", i),
                action: MenuAction::LoadState(i),
                shortcut: Some(format!("Ctrl+Shift+{}", i)),
                enabled: false, // Disabled until system with save state support loaded
            });
        }
        menus.push(Submenu {
            label: "State".to_string(),
            items: state_items,
        });

        // View menu
        menus.push(Submenu {
            label: "View".to_string(),
            items: vec![
                MenuItem {
                    label: "Take Screenshot".to_string(),
                    action: MenuAction::Screenshot,
                    shortcut: Some("F4".to_string()),
                    enabled: false, // Disabled until system loaded
                },
                MenuItem {
                    label: "Debug Info".to_string(),
                    action: MenuAction::DebugInfo,
                    shortcut: Some("F10".to_string()),
                    enabled: true,
                },
                MenuItem {
                    label: "CRT Filter".to_string(),
                    action: MenuAction::CrtFilterToggle,
                    shortcut: Some("F11".to_string()),
                    enabled: true,
                },
                MenuItem {
                    label: "Start Logging".to_string(),
                    action: MenuAction::StartLogging,
                    shortcut: None,
                    enabled: true,
                },
                MenuItem {
                    label: "Stop Logging".to_string(),
                    action: MenuAction::StopLogging,
                    shortcut: None,
                    enabled: false, // Disabled until logging started
                },
            ],
        });

        // Help menu
        menus.push(Submenu {
            label: "Help".to_string(),
            items: vec![
                MenuItem {
                    label: "Help".to_string(),
                    action: MenuAction::Help,
                    shortcut: Some("F1".to_string()),
                    enabled: true,
                },
                MenuItem {
                    label: "About".to_string(),
                    action: MenuAction::About,
                    shortcut: None,
                    enabled: true,
                },
            ],
        });

        Self {
            menus,
            active_menu: None,
            visible: true, // Always visible by default
        }
    }

    /// Render the menu bar at the top of the buffer
    pub fn render(&self, buffer: &mut [u32], width: usize, height: usize) {
        if !self.visible {
            return;
        }

        // Draw menu bar background (dark gray)
        for y in 0..MENU_BAR_HEIGHT {
            for x in 0..width {
                let idx = y * width + x;
                if idx < buffer.len() {
                    buffer[idx] = 0xFF2A2A3E; // Dark gray/purple background
                }
            }
        }

        // Draw menu labels
        let mut x_offset = 8;
        for (i, menu) in self.menus.iter().enumerate() {
            let color = if Some(i) == self.active_menu {
                0xFF16F2B3 // Highlighted (cyan/green)
            } else {
                0xFFFFFFFF // White
            };

            ui_render::draw_text(buffer, width, height, &menu.label, x_offset, 8, color);

            x_offset += MENU_ITEM_WIDTH;
        }

        // Draw dropdown if a menu is active
        if let Some(menu_idx) = self.active_menu {
            if let Some(menu) = self.menus.get(menu_idx) {
                self.render_dropdown(buffer, width, height, menu_idx, menu);
            }
        }
    }

    fn render_dropdown(
        &self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        menu_idx: usize,
        menu: &Submenu,
    ) {
        let dropdown_x = 8 + menu_idx * MENU_ITEM_WIDTH;
        let dropdown_y = MENU_BAR_HEIGHT;
        let dropdown_width = 250; // Fixed width for dropdown
        let item_height = 20;
        let dropdown_height = menu.items.len() * item_height;

        // Ensure dropdown fits in window
        let dropdown_height = dropdown_height.min(height - dropdown_y);

        // Draw dropdown background
        for dy in 0..dropdown_height {
            let y = dropdown_y + dy;
            for dx in 0..dropdown_width.min(width.saturating_sub(dropdown_x)) {
                let x = dropdown_x + dx;
                if x < width && y < height {
                    let idx = y * width + x;
                    if idx < buffer.len() {
                        buffer[idx] = 0xFF1A1A2E; // Darker background for dropdown
                    }
                }
            }
        }

        // Draw menu items
        for (i, item) in menu.items.iter().enumerate() {
            let y = dropdown_y + i * item_height;
            if y + item_height > height {
                break; // Don't render items that don't fit
            }

            // Color based on enabled state
            let text_color = if item.enabled {
                0xFFFFFFFF // White text for enabled
            } else {
                0xFF666666 // Dark gray for disabled
            };

            // Draw item text
            ui_render::draw_text(
                buffer,
                width,
                height,
                &item.label,
                dropdown_x + 8,
                y + 6,
                text_color,
            );

            // Draw shortcut hint (if any)
            if let Some(ref shortcut) = item.shortcut {
                let shortcut_x = dropdown_x + dropdown_width - shortcut.len() * 8 - 8;
                let shortcut_color = if item.enabled {
                    0xFF888888 // Gray for enabled
                } else {
                    0xFF444444 // Darker gray for disabled
                };
                ui_render::draw_text(
                    buffer,
                    width,
                    height,
                    shortcut,
                    shortcut_x.min(dropdown_x + 180), // Ensure it doesn't overflow
                    y + 6,
                    shortcut_color,
                );
            }
        }
    }

    /// Toggle menu visibility
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    /// Check if menu bar is visible
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Get the height of the menu bar when visible
    pub fn height(&self) -> usize {
        if self.visible {
            MENU_BAR_HEIGHT
        } else {
            0
        }
    }

    /// Handle mouse click - returns action if a menu item was clicked
    pub fn handle_click(&mut self, x: usize, y: usize) -> Option<MenuAction> {
        if !self.visible {
            return None;
        }

        // Check if click is in menu bar
        if y < MENU_BAR_HEIGHT {
            // Check which menu was clicked
            let menu_idx = x / MENU_ITEM_WIDTH;
            if menu_idx < self.menus.len() {
                if Some(menu_idx) == self.active_menu {
                    // Clicking the same menu closes it
                    self.active_menu = None;
                } else {
                    // Open the clicked menu
                    self.active_menu = Some(menu_idx);
                }
            }
            return None;
        }

        // Check if click is in dropdown
        if let Some(menu_idx) = self.active_menu {
            if let Some(menu) = self.menus.get(menu_idx) {
                let dropdown_x = 8 + menu_idx * MENU_ITEM_WIDTH;
                let dropdown_y = MENU_BAR_HEIGHT;
                let dropdown_width = 250;
                let item_height = 20;

                if x >= dropdown_x && x < dropdown_x + dropdown_width && y >= dropdown_y {
                    let item_idx = (y - dropdown_y) / item_height;
                    if item_idx < menu.items.len() {
                        let item = &menu.items[item_idx];
                        // Only return action if item is enabled
                        if item.enabled {
                            let action = item.action.clone();
                            self.active_menu = None; // Close menu after selection
                            return Some(action);
                        }
                    }
                }
            }
        }

        // Click outside menu - close any open dropdown
        self.active_menu = None;
        None
    }

    /// Update menu state based on system status
    pub fn update_menu_state(
        &mut self,
        rom_loaded: bool,
        paused: bool,
        supports_save_states: bool,
        logging_active: bool,
    ) {
        // File menu
        if let Some(file_menu) = self.menus.get_mut(0) {
            for item in &mut file_menu.items {
                match item.action {
                    MenuAction::SaveProject => item.enabled = rom_loaded,
                    MenuAction::MountPoints => item.enabled = rom_loaded,
                    _ => {}
                }
            }
        }

        // Emulation menu
        if let Some(emu_menu) = self.menus.get_mut(1) {
            for item in &mut emu_menu.items {
                match item.action {
                    MenuAction::Reset => item.enabled = rom_loaded,
                    MenuAction::Pause => item.enabled = rom_loaded && !paused,
                    MenuAction::Resume => item.enabled = rom_loaded && paused,
                    MenuAction::Speed(_) => item.enabled = rom_loaded,
                    _ => {}
                }
            }
        }

        // State menu
        if let Some(state_menu) = self.menus.get_mut(2) {
            for item in &mut state_menu.items {
                match item.action {
                    MenuAction::SaveState(_) | MenuAction::LoadState(_) => {
                        item.enabled = rom_loaded && supports_save_states;
                    }
                    _ => {}
                }
            }
        }

        // View menu
        if let Some(view_menu) = self.menus.get_mut(3) {
            for item in &mut view_menu.items {
                match item.action {
                    MenuAction::Screenshot => item.enabled = rom_loaded,
                    MenuAction::StartLogging => item.enabled = !logging_active,
                    MenuAction::StopLogging => item.enabled = logging_active,
                    _ => {}
                }
            }
        }
    }
}

impl Default for MenuBar {
    fn default() -> Self {
        Self::new()
    }
}
