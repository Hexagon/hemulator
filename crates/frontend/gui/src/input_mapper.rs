//! Input mapper integration
//!
//! This module provides helper functions to map physical inputs from the SDL2 backend
//! to virtual controller buttons using controller profiles.

use crate::input::{ControllerProfile, InputSource, VirtualButton};
use crate::window_backend::{Key, WindowBackend};

/// Map a virtual button to its bit position in the controller state
/// Returns None if the button is not part of the standard 8-button layout
pub fn virtual_button_to_bit(button: VirtualButton) -> Option<u8> {
    match button {
        VirtualButton::A => Some(0),
        VirtualButton::B => Some(1),
        VirtualButton::Select => Some(2),
        VirtualButton::Start => Some(3),
        VirtualButton::Up => Some(4),
        VirtualButton::Down => Some(5),
        VirtualButton::Left => Some(6),
        VirtualButton::Right => Some(7),
        // Extended buttons for 16-bit systems
        VirtualButton::X => Some(8),
        VirtualButton::Y => Some(9),
        VirtualButton::L => Some(10),
        VirtualButton::R => Some(11),
        _ => None, // N64 buttons, turbo, etc. not in standard layout
    }
}

/// Check if an input source is currently active
pub fn is_input_source_active(
    source: &InputSource,
    window: &dyn WindowBackend,
    sdl2_backend: Option<&crate::window_backend::Sdl2Backend>,
    gamepad_id: u32,
) -> bool {
    match source {
        InputSource::KeyboardKey(key_name) => {
            if let Some(key) = crate::window_backend::string_to_key(key_name) {
                window.is_key_down(key)
            } else {
                false
            }
        }
        InputSource::MouseButton(_button) => {
            // Mouse button support to be implemented
            false
        }
        InputSource::GamepadButton(button) => {
            if let Some(backend) = sdl2_backend {
                backend.is_gamepad_button_down(gamepad_id, *button)
            } else {
                false
            }
        }
        InputSource::GamepadAxis { axis, direction } => {
            if let Some(backend) = sdl2_backend {
                let value = backend.get_gamepad_axis(gamepad_id, *axis);
                // Threshold for axis activation (50% of full range)
                const THRESHOLD: i16 = 16384;
                match direction {
                    -1 => value < -THRESHOLD,
                    1 => value > THRESHOLD,
                    _ => false,
                }
            } else {
                false
            }
        }
        InputSource::JoystickButton(button) => {
            if let Some(backend) = sdl2_backend {
                backend.is_joystick_button_down(gamepad_id, *button)
            } else {
                false
            }
        }
        InputSource::JoystickAxis { axis, direction } => {
            if let Some(backend) = sdl2_backend {
                let value = backend.get_joystick_axis(gamepad_id, *axis);
                const THRESHOLD: i16 = 16384;
                match direction {
                    -1 => value < -THRESHOLD,
                    1 => value > THRESHOLD,
                    _ => false,
                }
            } else {
                false
            }
        }
        InputSource::JoystickHat { hat, direction } => {
            if let Some(backend) = sdl2_backend {
                let hat_value = backend.get_joystick_hat(gamepad_id, *hat);
                (hat_value & direction) != 0
            } else {
                false
            }
        }
    }
}

/// Get controller state from a profile (8-bit for NES/GB/Atari)
/// Returns a bitmask where each bit represents a button state (1 = pressed)
pub fn get_controller_state_from_profile(
    profile: &ControllerProfile,
    window: &dyn WindowBackend,
    sdl2_backend: Option<&crate::window_backend::Sdl2Backend>,
    gamepad_id: u32,
) -> u8 {
    let mut state = 0u8;

    for (virtual_button, input_source) in &profile.mappings {
        if let Some(bit) = virtual_button_to_bit(*virtual_button) {
            if bit < 8 && is_input_source_active(input_source, window, sdl2_backend, gamepad_id) {
                state |= 1 << bit;
            }
        }
    }

    state
}

/// Get SNES controller state from a profile (16-bit)
/// Returns a bitmask where each bit represents a button state (1 = pressed)
pub fn get_snes_controller_state_from_profile(
    profile: &ControllerProfile,
    window: &dyn WindowBackend,
    sdl2_backend: Option<&crate::window_backend::Sdl2Backend>,
    gamepad_id: u32,
) -> u16 {
    let mut state = 0u16;

    for (virtual_button, input_source) in &profile.mappings {
        if let Some(button_id) = virtual_button_to_bit(*virtual_button) {
            if is_input_source_active(input_source, window, sdl2_backend, gamepad_id) {
                // Map button IDs to SNES button positions
                let snes_bit = match button_id {
                    0 => 7,  // A -> bit 7
                    1 => 15, // B -> bit 15
                    2 => 13, // Select -> bit 13
                    3 => 12, // Start -> bit 12
                    4 => 11, // Up -> bit 11
                    5 => 10, // Down -> bit 10
                    6 => 9,  // Left -> bit 9
                    7 => 8,  // Right -> bit 8
                    8 => 6,  // X -> bit 6
                    9 => 14, // Y -> bit 14
                    10 => 5, // L -> bit 5
                    11 => 4, // R -> bit 4
                    _ => continue,
                };
                state |= 1u16 << snes_bit;
            }
        }
    }

    state
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_virtual_button_to_bit() {
        assert_eq!(virtual_button_to_bit(VirtualButton::A), Some(0));
        assert_eq!(virtual_button_to_bit(VirtualButton::B), Some(1));
        assert_eq!(virtual_button_to_bit(VirtualButton::Up), Some(4));
        assert_eq!(virtual_button_to_bit(VirtualButton::X), Some(8));
        assert_eq!(virtual_button_to_bit(VirtualButton::L), Some(10));
    }

    #[test]
    fn test_snes_button_mapping() {
        // Verify SNES button mapping logic
        // A=7, B=15, Select=13, Start=12, Up=11, Down=10, Left=9, Right=8
        // X=6, Y=14, L=5, R=4
        assert_eq!(virtual_button_to_bit(VirtualButton::A), Some(0));
        assert_eq!(virtual_button_to_bit(VirtualButton::B), Some(1));
        assert_eq!(virtual_button_to_bit(VirtualButton::X), Some(8));
        assert_eq!(virtual_button_to_bit(VirtualButton::Y), Some(9));
        assert_eq!(virtual_button_to_bit(VirtualButton::L), Some(10));
        assert_eq!(virtual_button_to_bit(VirtualButton::R), Some(11));
    }
}
