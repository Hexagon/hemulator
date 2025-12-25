//! Microsoft Mouse Driver (INT 33h) implementation
//!
//! Provides mouse support for DOS and Windows programs through INT 33h interface.
//! This is essential for Windows 3.1 compatibility.

#![allow(dead_code)] // Many methods used only by host integration, not tests

/// Mouse button state
#[derive(Debug, Clone, Copy, Default)]
pub struct MouseButtons {
    /// Left button pressed
    pub left: bool,
    /// Right button pressed
    pub right: bool,
    /// Middle button pressed (not common in early mice)
    pub middle: bool,
}

impl MouseButtons {
    /// Convert to button state word for INT 33h
    /// Bit 0: Left button, Bit 1: Right button, Bit 2: Middle button
    pub fn to_word(self) -> u16 {
        let mut word = 0u16;
        if self.left {
            word |= 0x01;
        }
        if self.right {
            word |= 0x02;
        }
        if self.middle {
            word |= 0x04;
        }
        word
    }

    /// Create from button state word
    pub fn from_word(word: u16) -> Self {
        Self {
            left: (word & 0x01) != 0,
            right: (word & 0x02) != 0,
            middle: (word & 0x04) != 0,
        }
    }
}

/// Microsoft Mouse Driver state
pub struct Mouse {
    /// Driver installed flag
    installed: bool,
    /// Mouse visible/hidden state (cursor visibility counter)
    visibility_counter: i16,
    /// Current X position (virtual coordinates)
    x: i16,
    /// Current Y position (virtual coordinates)
    y: i16,
    /// Button state
    buttons: MouseButtons,
    /// Virtual screen width (default 640)
    virtual_width: u16,
    /// Virtual screen height (default 200)
    virtual_height: u16,
    /// Minimum X coordinate
    min_x: i16,
    /// Maximum X coordinate
    max_x: i16,
    /// Minimum Y coordinate
    min_y: i16,
    /// Maximum Y coordinate
    max_y: i16,
    /// Horizontal mickey to pixel ratio (default 8:8)
    mickey_ratio_x: (u16, u16),
    /// Vertical mickey to pixel ratio (default 16:8)
    mickey_ratio_y: (u16, u16),
    /// Left button press count
    left_press_count: u16,
    /// Right button press count
    right_press_count: u16,
    /// Last left button press position
    left_press_pos: (i16, i16),
    /// Last right button press position
    right_press_pos: (i16, i16),
    /// Last left button release position
    left_release_pos: (i16, i16),
    /// Last right button release position
    right_release_pos: (i16, i16),
}

impl Mouse {
    /// Create a new mouse driver instance
    pub fn new() -> Self {
        Self {
            installed: false,
            visibility_counter: -1, // Hidden by default
            x: 0,
            y: 0,
            buttons: MouseButtons::default(),
            virtual_width: 640,
            virtual_height: 200,
            min_x: 0,
            max_x: 639,
            min_y: 0,
            max_y: 199,
            mickey_ratio_x: (8, 8),
            mickey_ratio_y: (16, 8),
            left_press_count: 0,
            right_press_count: 0,
            left_press_pos: (0, 0),
            right_press_pos: (0, 0),
            left_release_pos: (0, 0),
            right_release_pos: (0, 0),
        }
    }

    /// Reset the mouse driver (INT 33h AX=0000h)
    pub fn reset(&mut self) -> (u16, u16) {
        self.installed = true;
        self.visibility_counter = -1;
        self.x = self.virtual_width as i16 / 2;
        self.y = self.virtual_height as i16 / 2;
        self.buttons = MouseButtons::default();
        self.left_press_count = 0;
        self.right_press_count = 0;

        // Return: AX = 0xFFFF (mouse installed), BX = 2 (number of buttons)
        (0xFFFF, 0x0002)
    }

    /// Show mouse cursor (INT 33h AX=0001h)
    pub fn show_cursor(&mut self) {
        self.visibility_counter += 1;
    }

    /// Hide mouse cursor (INT 33h AX=0002h)
    pub fn hide_cursor(&mut self) {
        self.visibility_counter -= 1;
    }

    /// Get mouse position and button status (INT 33h AX=0003h)
    pub fn get_position_and_buttons(&self) -> (u16, i16, i16) {
        (self.buttons.to_word(), self.x, self.y)
    }

    /// Set mouse cursor position (INT 33h AX=0004h)
    pub fn set_position(&mut self, x: i16, y: i16) {
        self.x = x.clamp(self.min_x, self.max_x);
        self.y = y.clamp(self.min_y, self.max_y);
    }

    /// Get button press information (INT 33h AX=0005h)
    /// Returns: (button_state, press_count, x, y)
    pub fn get_button_press_info(&mut self, button: u16) -> (u16, u16, i16, i16) {
        match button {
            0 => {
                // Left button
                let count = self.left_press_count;
                self.left_press_count = 0;
                (
                    self.buttons.to_word(),
                    count,
                    self.left_press_pos.0,
                    self.left_press_pos.1,
                )
            }
            1 => {
                // Right button
                let count = self.right_press_count;
                self.right_press_count = 0;
                (
                    self.buttons.to_word(),
                    count,
                    self.right_press_pos.0,
                    self.right_press_pos.1,
                )
            }
            _ => (self.buttons.to_word(), 0, 0, 0),
        }
    }

    /// Get button release information (INT 33h AX=0006h)
    /// Returns: (button_state, release_count, x, y)
    pub fn get_button_release_info(&mut self, button: u16) -> (u16, u16, i16, i16) {
        match button {
            0 => (
                self.buttons.to_word(),
                0,
                self.left_release_pos.0,
                self.left_release_pos.1,
            ),
            1 => (
                self.buttons.to_word(),
                0,
                self.right_release_pos.0,
                self.right_release_pos.1,
            ),
            _ => (self.buttons.to_word(), 0, 0, 0),
        }
    }

    /// Set horizontal min/max position (INT 33h AX=0007h)
    pub fn set_horizontal_limits(&mut self, min: i16, max: i16) {
        self.min_x = min;
        self.max_x = max;
        self.virtual_width = (max - min + 1).max(1) as u16;
        // Clamp current position to new limits
        self.x = self.x.clamp(min, max);
    }

    /// Set vertical min/max position (INT 33h AX=0008h)
    pub fn set_vertical_limits(&mut self, min: i16, max: i16) {
        self.min_y = min;
        self.max_y = max;
        self.virtual_height = (max - min + 1).max(1) as u16;
        // Clamp current position to new limits
        self.y = self.y.clamp(min, max);
    }

    /// Set mickey to pixel ratio (INT 33h AX=000Fh)
    pub fn set_mickey_ratio(&mut self, horiz_mickeys: u16, vert_mickeys: u16) {
        self.mickey_ratio_x = (horiz_mickeys, 8);
        self.mickey_ratio_y = (vert_mickeys, 8);
    }

    /// Get driver version (INT 33h AX=0024h)
    pub fn get_driver_version(&self) -> (u16, u8, u8) {
        // Return: BX = version (6.26 = 0x0626), CH = type (1=bus mouse), CL = IRQ (0=none)
        (0x0626, 0x01, 0x00)
    }

    /// Check if mouse cursor is visible
    pub fn is_cursor_visible(&self) -> bool {
        self.visibility_counter >= 0
    }

    /// Update mouse position from host input (delta movement)
    pub fn update_position_delta(&mut self, dx: i16, dy: i16) {
        // Apply mickey to pixel ratio
        let scaled_dx = (dx * self.mickey_ratio_x.1 as i16) / self.mickey_ratio_x.0 as i16;
        let scaled_dy = (dy * self.mickey_ratio_y.1 as i16) / self.mickey_ratio_y.0 as i16;

        self.x = (self.x + scaled_dx).clamp(self.min_x, self.max_x);
        self.y = (self.y + scaled_dy).clamp(self.min_y, self.max_y);
    }

    /// Update mouse button state from host input
    pub fn update_buttons(&mut self, buttons: MouseButtons) {
        // Track button presses
        if buttons.left && !self.buttons.left {
            self.left_press_count += 1;
            self.left_press_pos = (self.x, self.y);
        }
        if buttons.right && !self.buttons.right {
            self.right_press_count += 1;
            self.right_press_pos = (self.x, self.y);
        }

        // Track button releases
        if !buttons.left && self.buttons.left {
            self.left_release_pos = (self.x, self.y);
        }
        if !buttons.right && self.buttons.right {
            self.right_release_pos = (self.x, self.y);
        }

        self.buttons = buttons;
    }

    /// Check if driver is installed
    pub fn is_installed(&self) -> bool {
        self.installed
    }
}

impl Default for Mouse {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mouse_creation() {
        let mouse = Mouse::new();
        assert!(!mouse.is_installed());
        assert_eq!(mouse.visibility_counter, -1);
    }

    #[test]
    fn test_mouse_reset() {
        let mut mouse = Mouse::new();
        let (ax, bx) = mouse.reset();
        assert_eq!(ax, 0xFFFF);
        assert_eq!(bx, 0x0002);
        assert!(mouse.is_installed());
        assert_eq!(mouse.x, 320); // Center of 640
        assert_eq!(mouse.y, 100); // Center of 200
    }

    #[test]
    fn test_show_hide_cursor() {
        let mut mouse = Mouse::new();
        mouse.reset();

        assert!(!mouse.is_cursor_visible()); // Starts hidden (-1)

        mouse.show_cursor();
        assert!(mouse.is_cursor_visible()); // Now visible (0)

        mouse.show_cursor();
        assert!(mouse.is_cursor_visible()); // Still visible (1)

        mouse.hide_cursor();
        assert!(mouse.is_cursor_visible()); // Still visible (0)

        mouse.hide_cursor();
        assert!(!mouse.is_cursor_visible()); // Hidden again (-1)
    }

    #[test]
    fn test_position_setting() {
        let mut mouse = Mouse::new();
        mouse.reset();

        mouse.set_position(100, 50);
        let (_, x, y) = mouse.get_position_and_buttons();
        assert_eq!(x, 100);
        assert_eq!(y, 50);
    }

    #[test]
    fn test_position_clamping() {
        let mut mouse = Mouse::new();
        mouse.reset();

        mouse.set_position(1000, -50);
        let (_, x, y) = mouse.get_position_and_buttons();
        assert_eq!(x, 639); // Clamped to max
        assert_eq!(y, 0); // Clamped to min
    }

    #[test]
    fn test_button_state() {
        let mut mouse = Mouse::new();
        mouse.reset();

        let buttons = MouseButtons {
            left: true,
            right: false,
            middle: false,
        };
        mouse.update_buttons(buttons);

        let (button_word, _, _) = mouse.get_position_and_buttons();
        assert_eq!(button_word, 0x01);
    }

    #[test]
    fn test_button_press_tracking() {
        let mut mouse = Mouse::new();
        mouse.reset();
        mouse.set_position(100, 100);

        // Press left button
        let buttons = MouseButtons {
            left: true,
            right: false,
            middle: false,
        };
        mouse.update_buttons(buttons);

        let (_, count, x, y) = mouse.get_button_press_info(0);
        assert_eq!(count, 1);
        assert_eq!(x, 100);
        assert_eq!(y, 100);

        // Count should be cleared after reading
        let (_, count2, _, _) = mouse.get_button_press_info(0);
        assert_eq!(count2, 0);
    }

    #[test]
    fn test_horizontal_limits() {
        let mut mouse = Mouse::new();
        mouse.reset();

        mouse.set_horizontal_limits(100, 500);
        assert_eq!(mouse.virtual_width, 401);

        // Set position after setting limits
        mouse.set_position(50, 100); // Below min
        let (_, x, _) = mouse.get_position_and_buttons();
        assert_eq!(x, 100); // Clamped to min
    }

    #[test]
    fn test_mickey_ratio() {
        let mut mouse = Mouse::new();
        mouse.reset();
        mouse.set_position(100, 100);

        mouse.set_mickey_ratio(16, 16); // Half sensitivity

        mouse.update_position_delta(16, 16);
        let (_, x, y) = mouse.get_position_and_buttons();
        assert_eq!(x, 108); // Moved 8 pixels (16 * 8 / 16)
        assert_eq!(y, 108);
    }

    #[test]
    fn test_driver_version() {
        let mouse = Mouse::new();
        let (version, mtype, irq) = mouse.get_driver_version();
        assert_eq!(version, 0x0626);
        assert_eq!(mtype, 0x01);
        assert_eq!(irq, 0x00);
    }
}
