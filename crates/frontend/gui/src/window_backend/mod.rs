//! Window backend abstraction
//!
//! This module provides an abstraction layer for window management and rendering,
//! supporting both software rendering and OpenGL rendering via SDL2.

use std::error::Error;

mod sdl2_backend;

pub use sdl2_backend::Sdl2Backend;

/// Common key codes used across backends
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    // Function keys
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,
    
    // Number keys
    Key0, Key1, Key2, Key3, Key4, Key5, Key6, Key7, Key8, Key9,
    
    // Letter keys
    A, B, C, D, E, F, G, H, I, J, K, L, M,
    N, O, P, Q, R, S, T, U, V, W, X, Y, Z,
    
    // Arrow keys
    Up, Down, Left, Right,
    
    // Special keys
    Escape, Enter, Space, Tab, Backspace,
    LeftShift, RightShift, LeftCtrl, RightCtrl, LeftAlt, RightAlt,
}

/// Window backend trait
pub trait WindowBackend {
    /// Check if window is still open
    fn is_open(&self) -> bool;
    
    /// Check if a key is currently pressed
    fn is_key_down(&self, key: Key) -> bool;
    
    /// Check if a key was just pressed (with repeat control)
    fn is_key_pressed(&self, key: Key, allow_repeat: bool) -> bool;
    
    /// Update window with new frame buffer
    /// Buffer format: ARGB (0xAARRGGBB)
    fn update_with_buffer(&mut self, buffer: &[u32], width: usize, height: usize) -> Result<(), Box<dyn Error>>;
    
    /// Get current window size
    fn get_size(&self) -> (usize, usize);
    
    /// Process window events (for event-based backends like winit)
    fn poll_events(&mut self);
    
    /// Get the backend name (for debugging)
    fn name(&self) -> &str;
}

/// Convert a string key name to our unified Key enum
pub fn string_to_key(s: &str) -> Option<Key> {
    match s {
        "F1" => Some(Key::F1),
        "F2" => Some(Key::F2),
        "F3" => Some(Key::F3),
        "F4" => Some(Key::F4),
        "F5" => Some(Key::F5),
        "F6" => Some(Key::F6),
        "F7" => Some(Key::F7),
        "F8" => Some(Key::F8),
        "F9" => Some(Key::F9),
        "F10" => Some(Key::F10),
        "F11" => Some(Key::F11),
        "F12" => Some(Key::F12),
        "0" => Some(Key::Key0),
        "1" => Some(Key::Key1),
        "2" => Some(Key::Key2),
        "3" => Some(Key::Key3),
        "4" => Some(Key::Key4),
        "5" => Some(Key::Key5),
        "6" => Some(Key::Key6),
        "7" => Some(Key::Key7),
        "8" => Some(Key::Key8),
        "9" => Some(Key::Key9),
        "A" => Some(Key::A),
        "B" => Some(Key::B),
        "C" => Some(Key::C),
        "D" => Some(Key::D),
        "E" => Some(Key::E),
        "F" => Some(Key::F),
        "G" => Some(Key::G),
        "H" => Some(Key::H),
        "I" => Some(Key::I),
        "J" => Some(Key::J),
        "K" => Some(Key::K),
        "L" => Some(Key::L),
        "M" => Some(Key::M),
        "N" => Some(Key::N),
        "O" => Some(Key::O),
        "P" => Some(Key::P),
        "Q" => Some(Key::Q),
        "R" => Some(Key::R),
        "S" => Some(Key::S),
        "T" => Some(Key::T),
        "U" => Some(Key::U),
        "V" => Some(Key::V),
        "W" => Some(Key::W),
        "X" => Some(Key::X),
        "Y" => Some(Key::Y),
        "Z" => Some(Key::Z),
        "Up" => Some(Key::Up),
        "Down" => Some(Key::Down),
        "Left" => Some(Key::Left),
        "Right" => Some(Key::Right),
        "Escape" => Some(Key::Escape),
        "Enter" => Some(Key::Enter),
        "Space" => Some(Key::Space),
        "Tab" => Some(Key::Tab),
        "Backspace" => Some(Key::Backspace),
        "LeftShift" => Some(Key::LeftShift),
        "RightShift" => Some(Key::RightShift),
        "LeftCtrl" => Some(Key::LeftCtrl),
        "RightCtrl" => Some(Key::RightCtrl),
        "LeftAlt" => Some(Key::LeftAlt),
        "RightAlt" => Some(Key::RightAlt),
        _ => None,
    }
}
