//! Input device abstraction layer
//!
//! This module provides an abstraction for different input devices (keyboard, mouse, gamepad, joystick)
//! and maps physical inputs to virtual controller buttons across different emulated systems.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Types of input devices supported by the emulator
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InputDeviceType {
    Keyboard,
    Mouse,
    Gamepad,
    Joystick,
}

/// Source of an input (which device and which button/axis)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum InputSource {
    /// Keyboard key by name (e.g., "Z", "Enter", "LeftShift")
    KeyboardKey(String),
    /// Mouse button (0 = left, 1 = middle, 2 = right)
    MouseButton(u8),
    /// Gamepad button by SDL2 button ID
    GamepadButton(u8),
    /// Gamepad axis by ID and direction (-1 for negative, 1 for positive)
    GamepadAxis { axis: u8, direction: i8 },
    /// Joystick button by ID
    JoystickButton(u8),
    /// Joystick axis by ID and direction
    JoystickAxis { axis: u8, direction: i8 },
    /// Joystick hat by ID and direction (bitmask: 1=up, 2=right, 4=down, 8=left)
    JoystickHat { hat: u8, direction: u8 },
}

/// Virtual controller button that can be mapped to physical inputs
/// This represents the logical buttons that games understand
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VirtualButton {
    // Standard controller buttons (NES, Game Boy, etc.)
    A,
    B,
    Select,
    Start,
    Up,
    Down,
    Left,
    Right,

    // Extended buttons (SNES, N64, etc.)
    X,
    Y,
    L,
    R,

    // N64 specific
    Z,
    CUp,
    CDown,
    CLeft,
    CRight,

    // System-specific buttons
    TurboA,
    TurboB,
}

/// Controller profile that maps physical inputs to virtual buttons
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControllerProfile {
    /// Profile name (e.g., "Keyboard WASD", "Xbox Controller", "PS4 DualShock")
    pub name: String,
    /// Mappings from virtual buttons to input sources
    pub mappings: HashMap<VirtualButton, InputSource>,
    /// Optional device filter (if specified, only applies to that device type)
    pub device_type: Option<InputDeviceType>,
}

impl ControllerProfile {
    /// Create a new controller profile
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            mappings: HashMap::new(),
            device_type: None,
        }
    }

    /// Add a button mapping
    pub fn map(mut self, button: VirtualButton, source: InputSource) -> Self {
        self.mappings.insert(button, source);
        self
    }

    /// Set the device type filter
    pub fn for_device(mut self, device_type: InputDeviceType) -> Self {
        self.device_type = Some(device_type);
        self
    }

    /// Default keyboard profile (Player 1 - arrow keys)
    pub fn keyboard_default() -> Self {
        Self::new("Keyboard (Default)")
            .for_device(InputDeviceType::Keyboard)
            .map(VirtualButton::A, InputSource::KeyboardKey("Z".to_string()))
            .map(VirtualButton::B, InputSource::KeyboardKey("X".to_string()))
            .map(
                VirtualButton::Select,
                InputSource::KeyboardKey("LeftShift".to_string()),
            )
            .map(
                VirtualButton::Start,
                InputSource::KeyboardKey("Enter".to_string()),
            )
            .map(
                VirtualButton::Up,
                InputSource::KeyboardKey("Up".to_string()),
            )
            .map(
                VirtualButton::Down,
                InputSource::KeyboardKey("Down".to_string()),
            )
            .map(
                VirtualButton::Left,
                InputSource::KeyboardKey("Left".to_string()),
            )
            .map(
                VirtualButton::Right,
                InputSource::KeyboardKey("Right".to_string()),
            )
    }

    /// Keyboard profile for Player 2 (IJKL)
    pub fn keyboard_player2() -> Self {
        Self::new("Keyboard (Player 2)")
            .for_device(InputDeviceType::Keyboard)
            .map(VirtualButton::A, InputSource::KeyboardKey("U".to_string()))
            .map(VirtualButton::B, InputSource::KeyboardKey("O".to_string()))
            .map(
                VirtualButton::Select,
                InputSource::KeyboardKey("RightShift".to_string()),
            )
            .map(
                VirtualButton::Start,
                InputSource::KeyboardKey("P".to_string()),
            )
            .map(VirtualButton::Up, InputSource::KeyboardKey("I".to_string()))
            .map(
                VirtualButton::Down,
                InputSource::KeyboardKey("K".to_string()),
            )
            .map(
                VirtualButton::Left,
                InputSource::KeyboardKey("J".to_string()),
            )
            .map(
                VirtualButton::Right,
                InputSource::KeyboardKey("L".to_string()),
            )
    }

    /// Default gamepad profile (SDL2 button mapping)
    pub fn gamepad_default() -> Self {
        Self::new("Gamepad (Default)")
            .for_device(InputDeviceType::Gamepad)
            // SDL2 GameController API button IDs
            .map(VirtualButton::A, InputSource::GamepadButton(0)) // A/Cross
            .map(VirtualButton::B, InputSource::GamepadButton(1)) // B/Circle
            .map(VirtualButton::X, InputSource::GamepadButton(2)) // X/Square
            .map(VirtualButton::Y, InputSource::GamepadButton(3)) // Y/Triangle
            .map(VirtualButton::L, InputSource::GamepadButton(4)) // L1/LB
            .map(VirtualButton::R, InputSource::GamepadButton(5)) // R1/RB
            .map(VirtualButton::Select, InputSource::GamepadButton(6)) // Back/Select
            .map(VirtualButton::Start, InputSource::GamepadButton(7)) // Start
            // D-pad via axis 0 (left/right) and axis 1 (up/down)
            .map(
                VirtualButton::Left,
                InputSource::GamepadAxis {
                    axis: 0,
                    direction: -1,
                },
            )
            .map(
                VirtualButton::Right,
                InputSource::GamepadAxis {
                    axis: 0,
                    direction: 1,
                },
            )
            .map(
                VirtualButton::Up,
                InputSource::GamepadAxis {
                    axis: 1,
                    direction: -1,
                },
            )
            .map(
                VirtualButton::Down,
                InputSource::GamepadAxis {
                    axis: 1,
                    direction: 1,
                },
            )
    }
}

/// Input mapper that resolves virtual button states from physical inputs
pub struct InputMapper {
    /// Active controller profiles (indexed by player number, 0-3)
    pub profiles: [ControllerProfile; 4],
}

impl InputMapper {
    /// Create a new input mapper with default profiles
    pub fn new() -> Self {
        Self {
            profiles: [
                ControllerProfile::keyboard_default(),
                ControllerProfile::keyboard_player2(),
                ControllerProfile::new("Player 3 (Unmapped)"),
                ControllerProfile::new("Player 4 (Unmapped)"),
            ],
        }
    }

    /// Get the profile for a specific player
    pub fn get_profile(&self, player: usize) -> Option<&ControllerProfile> {
        self.profiles.get(player)
    }

    /// Set the profile for a specific player
    pub fn set_profile(&mut self, player: usize, profile: ControllerProfile) {
        if player < 4 {
            self.profiles[player] = profile;
        }
    }
}

impl Default for InputMapper {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_controller_profile_creation() {
        let profile = ControllerProfile::keyboard_default();
        assert_eq!(profile.name, "Keyboard (Default)");
        assert_eq!(profile.device_type, Some(InputDeviceType::Keyboard));
        assert!(profile.mappings.contains_key(&VirtualButton::A));
        assert!(profile.mappings.contains_key(&VirtualButton::Up));
    }

    #[test]
    fn test_input_mapper_creation() {
        let mapper = InputMapper::new();
        assert_eq!(mapper.profiles.len(), 4);
        assert_eq!(mapper.profiles[0].name, "Keyboard (Default)");
        assert_eq!(mapper.profiles[1].name, "Keyboard (Player 2)");
    }

    #[test]
    fn test_custom_profile() {
        let profile = ControllerProfile::new("Custom")
            .map(VirtualButton::A, InputSource::GamepadButton(0))
            .map(VirtualButton::B, InputSource::GamepadButton(1));

        assert_eq!(profile.mappings.len(), 2);
        assert!(matches!(
            profile.mappings.get(&VirtualButton::A),
            Some(InputSource::GamepadButton(0))
        ));
    }

    #[test]
    fn test_gamepad_profile() {
        let profile = ControllerProfile::gamepad_default();
        assert_eq!(profile.device_type, Some(InputDeviceType::Gamepad));
        assert!(profile.mappings.contains_key(&VirtualButton::A));
        assert!(profile.mappings.contains_key(&VirtualButton::X));
        assert!(profile.mappings.contains_key(&VirtualButton::L));
    }
}
