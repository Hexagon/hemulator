//! PC keyboard input handling
//!
//! This module implements basic keyboard input for the PC emulator.
//! It translates window backend Key events to PC keyboard scancodes.

use std::collections::VecDeque;

/// PC keyboard controller
pub struct Keyboard {
    /// Queue of scancodes waiting to be read
    scancode_buffer: VecDeque<u8>,
    /// Maximum buffer size
    max_buffer_size: usize,
    /// Modifier key states (for INT 16h AH=02h)
    /// Bit 0 = Right Shift, Bit 1 = Left Shift
    /// Bit 2 = Ctrl, Bit 3 = Alt
    /// Bit 4 = Scroll Lock, Bit 5 = Num Lock, Bit 6 = Caps Lock, Bit 7 = Insert
    shift_flags: u8,
    /// Track Right Alt (AltGr) separately for international character support
    altgr_pressed: bool,
}

impl Keyboard {
    /// Create a new keyboard controller
    pub fn new() -> Self {
        Self {
            scancode_buffer: VecDeque::with_capacity(16),
            max_buffer_size: 16,
            shift_flags: 0,
            altgr_pressed: false,
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

    /// Find the first make code (key press) in the buffer, skipping break codes
    /// Returns None if no make code is found
    pub fn peek_make_code(&self) -> Option<u8> {
        self.scancode_buffer
            .iter()
            .find(|&&code| code & 0x80 == 0) // Find first code with high bit clear (make code)
            .copied()
    }

    /// Add a key press event (generates make code)
    pub fn key_press(&mut self, key: u8) {
        // Update shift flags for modifier keys
        match key {
            SCANCODE_LEFT_SHIFT => self.shift_flags |= 0x02, // Bit 1 = Left Shift
            SCANCODE_RIGHT_SHIFT => self.shift_flags |= 0x01, // Bit 0 = Right Shift
            SCANCODE_LEFT_CTRL => self.shift_flags |= 0x04,  // Bit 2 = Ctrl
            SCANCODE_RIGHT_CTRL => self.shift_flags |= 0x04, // Bit 2 = Ctrl
            SCANCODE_LEFT_ALT => self.shift_flags |= 0x08,   // Bit 3 = Alt (Left Alt)
            SCANCODE_RIGHT_ALT => {
                self.shift_flags |= 0x08; // Bit 3 = Alt (for compatibility)
                self.altgr_pressed = true; // Track AltGr separately
            }
            _ => {}
        }

        // Only store make codes (key presses) in the buffer for INT 16h
        // Break codes (key releases) are not needed for keyboard input
        if self.scancode_buffer.len() < self.max_buffer_size {
            self.scancode_buffer.push_back(key);
        }
    }

    /// Add a key release event (updates shift flags only, no scancode buffered)
    pub fn key_release(&mut self, key: u8) {
        // Update shift flags for modifier keys
        match key {
            SCANCODE_LEFT_SHIFT => self.shift_flags &= !0x02, // Clear bit 1
            SCANCODE_RIGHT_SHIFT => self.shift_flags &= !0x01, // Clear bit 0
            SCANCODE_LEFT_CTRL => self.shift_flags &= !0x04,  // Clear bit 2
            SCANCODE_RIGHT_CTRL => self.shift_flags &= !0x04, // Clear bit 2
            SCANCODE_LEFT_ALT => self.shift_flags &= !0x08,   // Clear bit 3
            SCANCODE_RIGHT_ALT => {
                self.shift_flags &= !0x08; // Clear bit 3
                self.altgr_pressed = false; // Clear AltGr flag
            }
            _ => {}
        }

        // Do NOT buffer break codes - INT 16h only needs make codes
        // The break code was needed for hardware keyboard controllers, but not for
        // BIOS keyboard services which only report key presses, not releases
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

    /// Get the current shift flags (for INT 16h AH=02h)
    pub fn get_shift_flags(&self) -> u8 {
        self.shift_flags
    }

    /// Check if Ctrl is pressed
    pub fn is_ctrl_pressed(&self) -> bool {
        self.shift_flags & 0x04 != 0
    }

    /// Check if Alt is pressed
    pub fn is_alt_pressed(&self) -> bool {
        self.shift_flags & 0x08 != 0
    }

    /// Check if Shift is pressed
    pub fn is_shift_pressed(&self) -> bool {
        self.shift_flags & 0x03 != 0 // Either left or right shift
    }

    /// Check if AltGr (Right Alt) is pressed
    pub fn is_altgr_pressed(&self) -> bool {
        self.altgr_pressed
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
// Extended scancodes (normally E0-prefixed, but we use simplified values)
pub const SCANCODE_RIGHT_CTRL: u8 = 0x5D; // Right Ctrl (extended scancode E0 1D)
pub const SCANCODE_RIGHT_ALT: u8 = 0x5E; // Right Alt/AltGr (extended scancode E0 38)

/// Convert SDL2-style scancode (u32) to PC scancode (u8)
/// SDL2 scancodes are physical key positions that match PC keyboard layout
/// This allows direct mapping without going through character translation
#[allow(dead_code)]
pub fn sdl2_scancode_to_pc(sdl_scancode: u32) -> Option<u8> {
    // SDL2 scancodes match USB HID scancodes which are similar to PC scancodes
    // See: https://wiki.libsdl.org/SDL2/SDL_Scancode
    match sdl_scancode {
        // Function keys
        58 => Some(SCANCODE_F1),  // SDL_SCANCODE_F1
        59 => Some(SCANCODE_F2),  // SDL_SCANCODE_F2
        60 => Some(SCANCODE_F3),  // SDL_SCANCODE_F3
        61 => Some(SCANCODE_F4),  // SDL_SCANCODE_F4
        62 => Some(SCANCODE_F5),  // SDL_SCANCODE_F5
        63 => Some(SCANCODE_F6),  // SDL_SCANCODE_F6
        64 => Some(SCANCODE_F7),  // SDL_SCANCODE_F7
        65 => Some(SCANCODE_F8),  // SDL_SCANCODE_F8
        66 => Some(SCANCODE_F9),  // SDL_SCANCODE_F9
        67 => Some(SCANCODE_F10), // SDL_SCANCODE_F10
        // Number row
        39 => Some(SCANCODE_0), // SDL_SCANCODE_0
        30 => Some(SCANCODE_1), // SDL_SCANCODE_1
        31 => Some(SCANCODE_2), // SDL_SCANCODE_2
        32 => Some(SCANCODE_3), // SDL_SCANCODE_3
        33 => Some(SCANCODE_4), // SDL_SCANCODE_4
        34 => Some(SCANCODE_5), // SDL_SCANCODE_5
        35 => Some(SCANCODE_6), // SDL_SCANCODE_6
        36 => Some(SCANCODE_7), // SDL_SCANCODE_7
        37 => Some(SCANCODE_8), // SDL_SCANCODE_8
        38 => Some(SCANCODE_9), // SDL_SCANCODE_9
        // Letter keys (QWERTY layout)
        4 => Some(SCANCODE_A),  // SDL_SCANCODE_A
        5 => Some(SCANCODE_B),  // SDL_SCANCODE_B
        6 => Some(SCANCODE_C),  // SDL_SCANCODE_C
        7 => Some(SCANCODE_D),  // SDL_SCANCODE_D
        8 => Some(SCANCODE_E),  // SDL_SCANCODE_E
        9 => Some(SCANCODE_F),  // SDL_SCANCODE_F
        10 => Some(SCANCODE_G), // SDL_SCANCODE_G
        11 => Some(SCANCODE_H), // SDL_SCANCODE_H
        12 => Some(SCANCODE_I), // SDL_SCANCODE_I
        13 => Some(SCANCODE_J), // SDL_SCANCODE_J
        14 => Some(SCANCODE_K), // SDL_SCANCODE_K
        15 => Some(SCANCODE_L), // SDL_SCANCODE_L
        16 => Some(SCANCODE_M), // SDL_SCANCODE_M
        17 => Some(SCANCODE_N), // SDL_SCANCODE_N
        18 => Some(SCANCODE_O), // SDL_SCANCODE_O
        19 => Some(SCANCODE_P), // SDL_SCANCODE_P
        20 => Some(SCANCODE_Q), // SDL_SCANCODE_Q
        21 => Some(SCANCODE_R), // SDL_SCANCODE_R
        22 => Some(SCANCODE_S), // SDL_SCANCODE_S
        23 => Some(SCANCODE_T), // SDL_SCANCODE_T
        24 => Some(SCANCODE_U), // SDL_SCANCODE_U
        25 => Some(SCANCODE_V), // SDL_SCANCODE_V
        26 => Some(SCANCODE_W), // SDL_SCANCODE_W
        27 => Some(SCANCODE_X), // SDL_SCANCODE_X
        28 => Some(SCANCODE_Y), // SDL_SCANCODE_Y
        29 => Some(SCANCODE_Z), // SDL_SCANCODE_Z
        // Special keys
        41 => Some(SCANCODE_ESC),       // SDL_SCANCODE_ESCAPE
        40 => Some(SCANCODE_ENTER),     // SDL_SCANCODE_RETURN
        42 => Some(SCANCODE_BACKSPACE), // SDL_SCANCODE_BACKSPACE
        43 => Some(SCANCODE_TAB),       // SDL_SCANCODE_TAB
        44 => Some(SCANCODE_SPACE),     // SDL_SCANCODE_SPACE
        // Modifiers
        225 => Some(SCANCODE_LEFT_SHIFT),  // SDL_SCANCODE_LSHIFT
        229 => Some(SCANCODE_RIGHT_SHIFT), // SDL_SCANCODE_RSHIFT
        224 => Some(SCANCODE_LEFT_CTRL),   // SDL_SCANCODE_LCTRL
        228 => Some(SCANCODE_RIGHT_CTRL),  // SDL_SCANCODE_RCTRL
        226 => Some(SCANCODE_LEFT_ALT),    // SDL_SCANCODE_LALT
        230 => Some(SCANCODE_RIGHT_ALT),   // SDL_SCANCODE_RALT (AltGr)
        // Punctuation
        54 => Some(SCANCODE_COMMA),         // SDL_SCANCODE_COMMA
        55 => Some(SCANCODE_PERIOD),        // SDL_SCANCODE_PERIOD
        56 => Some(SCANCODE_SLASH),         // SDL_SCANCODE_SLASH
        51 => Some(SCANCODE_SEMICOLON),     // SDL_SCANCODE_SEMICOLON
        52 => Some(SCANCODE_APOSTROPHE),    // SDL_SCANCODE_APOSTROPHE
        47 => Some(SCANCODE_LEFT_BRACKET),  // SDL_SCANCODE_LEFTBRACKET
        48 => Some(SCANCODE_RIGHT_BRACKET), // SDL_SCANCODE_RIGHTBRACKET
        49 => Some(SCANCODE_BACKSLASH),     // SDL_SCANCODE_BACKSLASH
        45 => Some(SCANCODE_MINUS),         // SDL_SCANCODE_MINUS
        46 => Some(SCANCODE_EQUALS),        // SDL_SCANCODE_EQUALS
        53 => Some(SCANCODE_BACKTICK),      // SDL_SCANCODE_GRAVE
        _ => None,
    }
}

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
        // Key releases don't generate scancodes anymore (INT 16h only needs make codes)
        assert!(!kb.has_data());
    }

    #[test]
    fn test_key_press_release() {
        let mut kb = Keyboard::new();

        kb.key_press(SCANCODE_A);
        kb.key_release(SCANCODE_A);

        // Only the make code should be in the buffer
        assert!(kb.has_data());
        assert_eq!(kb.read_scancode(), SCANCODE_A);
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

    #[test]
    fn test_peek_make_code() {
        let mut kb = Keyboard::new();

        // With the new behavior, key_release doesn't add scancodes
        // So we just test with make codes
        kb.key_press(SCANCODE_A); // Make code
        kb.key_press(SCANCODE_B); // Make code

        // peek_make_code should find the first make code
        assert_eq!(kb.peek_make_code(), Some(SCANCODE_A));

        // peek_scancode should return the first item
        assert_eq!(kb.peek_scancode(), SCANCODE_A);

        // Reading should get first make code
        assert_eq!(kb.read_scancode(), SCANCODE_A);

        // Now peek_make_code should return the second make code
        assert_eq!(kb.peek_make_code(), Some(SCANCODE_B));

        // Read the second make code
        assert_eq!(kb.read_scancode(), SCANCODE_B);

        // No more data
        assert_eq!(kb.peek_make_code(), None);
    }

    #[test]
    fn test_peek_make_code_only_break_codes() {
        let mut kb = Keyboard::new();

        // With the new behavior, key_release doesn't add scancodes
        // So the buffer should remain empty
        kb.key_release(SCANCODE_A);
        kb.key_release(SCANCODE_B);

        // Should return None since no make codes (and buffer is empty)
        assert_eq!(kb.peek_make_code(), None);
    }
}
