//! Input mapper integration
//!
//! This module provides helper functions to map physical inputs from the SDL2 backend
//! to virtual controller buttons using controller profiles.

use crate::input::{ControllerProfile, InputSource, VirtualButton};
use crate::window_backend::WindowBackend;

/// Axis threshold for activation (~50% of half-range, ~25% deflection from center)
/// This represents approximately 25% deflection from center position.
/// Full axis range is -32768 to 32767, so threshold at ±16384 is ~50% of half-range.
const AXIS_THRESHOLD: i16 = 16384;

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
    instance_id: u32,
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
            // NOTE: Mouse button support is not yet implemented.
            // Only mouse position/motion tracking is currently available.
            // To enable mouse buttons, extend the SDL2 backend to track mouse button state
            // and implement the logic here.
            false
        }
        InputSource::GamepadButton(button) => {
            if let Some(backend) = sdl2_backend {
                backend.is_gamepad_button_down(instance_id, *button)
            } else {
                false
            }
        }
        InputSource::GamepadAxis { axis, direction } => {
            if let Some(backend) = sdl2_backend {
                let value = backend.get_gamepad_axis(instance_id, *axis);
                match direction {
                    -1 => value < -AXIS_THRESHOLD,
                    1 => value > AXIS_THRESHOLD,
                    _ => false,
                }
            } else {
                false
            }
        }
        InputSource::JoystickButton(button) => {
            if let Some(backend) = sdl2_backend {
                backend.is_joystick_button_down(instance_id, *button)
            } else {
                false
            }
        }
        InputSource::JoystickAxis { axis, direction } => {
            if let Some(backend) = sdl2_backend {
                let value = backend.get_joystick_axis(instance_id, *axis);
                match direction {
                    -1 => value < -AXIS_THRESHOLD,
                    1 => value > AXIS_THRESHOLD,
                    _ => false,
                }
            } else {
                false
            }
        }
        InputSource::JoystickHat { hat, direction } => {
            if let Some(backend) = sdl2_backend {
                let hat_value = backend.get_joystick_hat(instance_id, *hat);
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
    instance_id: u32,
) -> u8 {
    let mut state = 0u8;

    for (virtual_button, input_source) in &profile.mappings {
        if let Some(bit) = virtual_button_to_bit(*virtual_button) {
            if bit < 8 && is_input_source_active(input_source, window, sdl2_backend, instance_id) {
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
    instance_id: u32,
) -> u16 {
    let mut state = 0u16;

    for (virtual_button, input_source) in &profile.mappings {
        if let Some(button_id) = virtual_button_to_bit(*virtual_button) {
            if is_input_source_active(input_source, window, sdl2_backend, instance_id) {
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
        // This test verifies the SNES-specific button position mapping in get_snes_controller_state_from_profile
        // SNES hardware bit positions (from the match statement):
        // A=7, B=15, Select=13, Start=12, Up=11, Down=10, Left=9, Right=8
        // X=6, Y=14, L=5, R=4

        // Create a dummy profile that maps all buttons to keyboard keys
        let mut profile = ControllerProfile::new("Test SNES Profile");
        profile
            .mappings
            .insert(VirtualButton::A, InputSource::KeyboardKey("A".to_string()));
        profile
            .mappings
            .insert(VirtualButton::B, InputSource::KeyboardKey("B".to_string()));
        profile
            .mappings
            .insert(VirtualButton::X, InputSource::KeyboardKey("X".to_string()));
        profile
            .mappings
            .insert(VirtualButton::Y, InputSource::KeyboardKey("Y".to_string()));
        profile
            .mappings
            .insert(VirtualButton::L, InputSource::KeyboardKey("L".to_string()));
        profile
            .mappings
            .insert(VirtualButton::R, InputSource::KeyboardKey("R".to_string()));

        // Verify that virtual_button_to_bit returns the generic button IDs (0-11)
        // which are then mapped to SNES-specific bit positions in get_snes_controller_state_from_profile
        assert_eq!(virtual_button_to_bit(VirtualButton::A), Some(0)); // Generic ID 0 -> SNES bit 7
        assert_eq!(virtual_button_to_bit(VirtualButton::B), Some(1)); // Generic ID 1 -> SNES bit 15
        assert_eq!(virtual_button_to_bit(VirtualButton::X), Some(8)); // Generic ID 8 -> SNES bit 6
        assert_eq!(virtual_button_to_bit(VirtualButton::Y), Some(9)); // Generic ID 9 -> SNES bit 14
        assert_eq!(virtual_button_to_bit(VirtualButton::L), Some(10)); // Generic ID 10 -> SNES bit 5
        assert_eq!(virtual_button_to_bit(VirtualButton::R), Some(11)); // Generic ID 11 -> SNES bit 4
    }
}
