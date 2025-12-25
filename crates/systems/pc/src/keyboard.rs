//! PC keyboard input handling
//!
//! This module implements basic keyboard input for the PC emulator.
//! It translates minifb Key events to PC keyboard scancodes.

use std::collections::VecDeque;

/// PC keyboard controller
pub struct Keyboard {
    /// Queue of scancodes waiting to be read
    scancode_buffer: VecDeque<u8>,
    /// Maximum buffer size
    max_buffer_size: usize,
}

impl Keyboard {
    /// Create a new keyboard controller
    pub fn new() -> Self {
        Self {
            scancode_buffer: VecDeque::with_capacity(16),
            max_buffer_size: 16,
        }
    }

    /// Check if there are scancodes available to read
    pub fn has_data(&self) -> bool {
        !self.scancode_buffer.is_empty()
    }

    /// Read a scancode from the buffer
    pub fn read_scancode(&mut self) -> u8 {
        self.scancode_buffer.pop_front().unwrap_or(0)
    }

    /// Peek at the next scancode without consuming it
    pub fn peek_scancode(&self) -> u8 {
        self.scancode_buffer.front().copied().unwrap_or(0)
    }

    /// Add a key press event (generates make code)
    pub fn key_press(&mut self, key: u8) {
        if self.scancode_buffer.len() < self.max_buffer_size {
            self.scancode_buffer.push_back(key);
        }
    }

    /// Add a key release event (generates break code)
    pub fn key_release(&mut self, key: u8) {
        if self.scancode_buffer.len() < self.max_buffer_size {
            self.scancode_buffer.push_back(key | 0x80); // Break code has high bit set
        }
    }

    /// Clear the scancode buffer
    pub fn clear(&mut self) {
        self.scancode_buffer.clear();
    }

    /// Check if ESC key is in the buffer (for boot abort)
    pub fn has_esc(&self) -> bool {
        self.scancode_buffer
            .iter()
            .any(|&code| code == SCANCODE_ESC)
    }
}

impl Default for Keyboard {
    fn default() -> Self {
        Self::new()
    }
}

// PC keyboard scan codes (Set 1) for common keys
pub const SCANCODE_ESC: u8 = 0x01;
pub const SCANCODE_1: u8 = 0x02;
pub const SCANCODE_2: u8 = 0x03;
pub const SCANCODE_3: u8 = 0x04;
pub const SCANCODE_4: u8 = 0x05;
pub const SCANCODE_5: u8 = 0x06;
pub const SCANCODE_6: u8 = 0x07;
pub const SCANCODE_7: u8 = 0x08;
pub const SCANCODE_8: u8 = 0x09;
pub const SCANCODE_9: u8 = 0x0A;
pub const SCANCODE_0: u8 = 0x0B;
pub const SCANCODE_MINUS: u8 = 0x0C;
pub const SCANCODE_EQUALS: u8 = 0x0D;
pub const SCANCODE_BACKSPACE: u8 = 0x0E;
pub const SCANCODE_TAB: u8 = 0x0F;
pub const SCANCODE_Q: u8 = 0x10;
pub const SCANCODE_W: u8 = 0x11;
pub const SCANCODE_E: u8 = 0x12;
pub const SCANCODE_R: u8 = 0x13;
pub const SCANCODE_T: u8 = 0x14;
pub const SCANCODE_Y: u8 = 0x15;
pub const SCANCODE_U: u8 = 0x16;
pub const SCANCODE_I: u8 = 0x17;
pub const SCANCODE_O: u8 = 0x18;
pub const SCANCODE_P: u8 = 0x19;
pub const SCANCODE_LEFT_BRACKET: u8 = 0x1A;
pub const SCANCODE_RIGHT_BRACKET: u8 = 0x1B;
pub const SCANCODE_ENTER: u8 = 0x1C;
pub const SCANCODE_LEFT_CTRL: u8 = 0x1D;
pub const SCANCODE_A: u8 = 0x1E;
pub const SCANCODE_S: u8 = 0x1F;
pub const SCANCODE_D: u8 = 0x20;
pub const SCANCODE_F: u8 = 0x21;
pub const SCANCODE_G: u8 = 0x22;
pub const SCANCODE_H: u8 = 0x23;
pub const SCANCODE_J: u8 = 0x24;
pub const SCANCODE_K: u8 = 0x25;
pub const SCANCODE_L: u8 = 0x26;
pub const SCANCODE_SEMICOLON: u8 = 0x27;
pub const SCANCODE_APOSTROPHE: u8 = 0x28;
pub const SCANCODE_BACKTICK: u8 = 0x29;
pub const SCANCODE_LEFT_SHIFT: u8 = 0x2A;
pub const SCANCODE_BACKSLASH: u8 = 0x2B;
pub const SCANCODE_Z: u8 = 0x2C;
pub const SCANCODE_X: u8 = 0x2D;
pub const SCANCODE_C: u8 = 0x2E;
pub const SCANCODE_V: u8 = 0x2F;
pub const SCANCODE_B: u8 = 0x30;
pub const SCANCODE_N: u8 = 0x31;
pub const SCANCODE_M: u8 = 0x32;
pub const SCANCODE_COMMA: u8 = 0x33;
pub const SCANCODE_PERIOD: u8 = 0x34;
pub const SCANCODE_SLASH: u8 = 0x35;
pub const SCANCODE_RIGHT_SHIFT: u8 = 0x36;
pub const SCANCODE_KP_STAR: u8 = 0x37;
pub const SCANCODE_LEFT_ALT: u8 = 0x38;
pub const SCANCODE_SPACE: u8 = 0x39;
pub const SCANCODE_CAPS_LOCK: u8 = 0x3A;
pub const SCANCODE_F1: u8 = 0x3B;
pub const SCANCODE_F2: u8 = 0x3C;
pub const SCANCODE_F3: u8 = 0x3D;
pub const SCANCODE_F4: u8 = 0x3E;
pub const SCANCODE_F5: u8 = 0x3F;
pub const SCANCODE_F6: u8 = 0x40;
pub const SCANCODE_F7: u8 = 0x41;
pub const SCANCODE_F8: u8 = 0x42;
pub const SCANCODE_F9: u8 = 0x43;
pub const SCANCODE_F10: u8 = 0x44;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keyboard_creation() {
        let kb = Keyboard::new();
        assert!(!kb.has_data());
    }

    #[test]
    fn test_key_press() {
        let mut kb = Keyboard::new();

        kb.key_press(SCANCODE_A);
        assert!(kb.has_data());

        let code = kb.read_scancode();
        assert_eq!(code, SCANCODE_A);
        assert!(!kb.has_data());
    }

    #[test]
    fn test_key_release() {
        let mut kb = Keyboard::new();

        kb.key_release(SCANCODE_A);
        assert!(kb.has_data());

        let code = kb.read_scancode();
        assert_eq!(code, SCANCODE_A | 0x80); // Break code
        assert!(!kb.has_data());
    }

    #[test]
    fn test_key_press_release() {
        let mut kb = Keyboard::new();

        kb.key_press(SCANCODE_A);
        kb.key_release(SCANCODE_A);

        assert!(kb.has_data());
        assert_eq!(kb.read_scancode(), SCANCODE_A);
        assert_eq!(kb.read_scancode(), SCANCODE_A | 0x80);
        assert!(!kb.has_data());
    }

    #[test]
    fn test_buffer_overflow() {
        let mut kb = Keyboard::new();

        // Fill buffer beyond capacity
        for i in 0..20 {
            kb.key_press(i);
        }

        // Should only have max_buffer_size items
        let mut count = 0;
        while kb.has_data() {
            kb.read_scancode();
            count += 1;
        }
        assert_eq!(count, 16); // max_buffer_size
    }

    #[test]
    fn test_clear() {
        let mut kb = Keyboard::new();

        kb.key_press(SCANCODE_A);
        kb.key_press(SCANCODE_B);
        assert!(kb.has_data());

        kb.clear();
        assert!(!kb.has_data());
    }

    #[test]
    fn test_peek_scancode() {
        let mut kb = Keyboard::new();

        kb.key_press(SCANCODE_A);
        kb.key_press(SCANCODE_B);

        // Peek should return first item without removing it
        assert_eq!(kb.peek_scancode(), SCANCODE_A);
        assert!(kb.has_data());

        // Peek again should return same value
        assert_eq!(kb.peek_scancode(), SCANCODE_A);

        // Read should consume the item
        assert_eq!(kb.read_scancode(), SCANCODE_A);
        assert_eq!(kb.peek_scancode(), SCANCODE_B);
        assert_eq!(kb.read_scancode(), SCANCODE_B);

        // Peek on empty buffer should return 0
        assert_eq!(kb.peek_scancode(), 0);
    }
}
