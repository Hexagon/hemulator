use crate::display_filter::DisplayFilter;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Standard controller button mapping (for NES, SNES, GB, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyMapping {
    pub a: String,
    pub b: String,
    #[serde(default = "default_empty_string")]
    pub x: String, // For SNES and future systems
    #[serde(default = "default_empty_string")]
    pub y: String, // For SNES and future systems
    #[serde(default = "default_empty_string")]
    pub l: String, // For SNES shoulder buttons
    #[serde(default = "default_empty_string")]
    pub r: String, // For SNES shoulder buttons
    pub select: String,
    pub start: String,
    pub up: String,
    pub down: String,
    pub left: String,
    pub right: String,
}

fn default_empty_string() -> String {
    String::new()
}

impl Default for KeyMapping {
    fn default() -> Self {
        Self {
            a: "Z".to_string(),
            b: "X".to_string(),
            x: String::new(), // Not mapped by default
            y: String::new(), // Not mapped by default
            l: String::new(), // Not mapped by default
            r: String::new(), // Not mapped by default
            select: "LeftShift".to_string(),
            start: "Enter".to_string(),
            up: "Up".to_string(),
            down: "Down".to_string(),
            left: "Left".to_string(),
            right: "Right".to_string(),
        }
    }
}

impl KeyMapping {
    /// Default mapping for Player 2
    pub fn player2_default() -> Self {
        Self {
            a: "U".to_string(),
            b: "O".to_string(),
            x: String::new(),
            y: String::new(),
            l: String::new(),
            r: String::new(),
            select: "RightShift".to_string(),
            start: "P".to_string(), // P key (single key on right side)
            up: "I".to_string(),
            down: "K".to_string(),
            left: "J".to_string(),
            right: "L".to_string(),
        }
    }

    /// Default mapping for Player 3 (unmapped by default, but structure available)
    pub fn player3_default() -> Self {
        Self {
            a: String::new(),
            b: String::new(),
            x: String::new(),
            y: String::new(),
            l: String::new(),
            r: String::new(),
            select: String::new(),
            start: String::new(),
            up: String::new(),
            down: String::new(),
            left: String::new(),
            right: String::new(),
        }
    }

    /// Default mapping for Player 4 (unmapped by default, but structure available)
    pub fn player4_default() -> Self {
        Self {
            a: String::new(),
            b: String::new(),
            x: String::new(),
            y: String::new(),
            l: String::new(),
            r: String::new(),
            select: String::new(),
            start: String::new(),
            up: String::new(),
            down: String::new(),
            left: String::new(),
            right: String::new(),
        }
    }
}

/// Input configuration for all players
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputConfig {
    #[serde(default)]
    pub player1: KeyMapping,
    #[serde(default)]
    pub player2: KeyMapping,
    #[serde(default)]
    pub player3: KeyMapping,
    #[serde(default)]
    pub player4: KeyMapping,
    /// Host modifier key for switching to host mode (function keys) in PC emulation
    /// Default: RightCtrl
    #[serde(default = "default_host_modifier")]
    pub host_modifier: String,
}

fn default_host_modifier() -> String {
    "RightCtrl".to_string()
}

impl Default for InputConfig {
    fn default() -> Self {
        Self {
            player1: KeyMapping::default(),
            player2: KeyMapping::player2_default(),
            player3: KeyMapping::player3_default(),
            player4: KeyMapping::player4_default(),
            host_modifier: default_host_modifier(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    // Backward compatibility: keep old keyboard field for migration
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keyboard: Option<KeyMapping>,

    // New input configuration supporting multiple players
    #[serde(default)]
    pub input: InputConfig,

    pub window_width: usize,
    pub window_height: usize,
    #[serde(default, skip_serializing)]
    pub last_rom_path: Option<String>, // Kept for backward compatibility reading only, not saved
    #[serde(default)]
    pub display_filter: DisplayFilter,
    #[serde(default, skip_serializing)] // Runtime only, not saved
    pub emulation_speed: f64, // Speed multiplier: 0.0 (pause), 0.25, 0.5, 1.0, 2.0, 10.0
    #[serde(default = "default_video_backend")]
    pub video_backend: String, // "software" or "opengl"
    #[serde(default, flatten, skip_serializing_if = "HashMap::is_empty")]
    pub extra: HashMap<String, Value>,
}

fn default_video_backend() -> String {
    "software".to_string()
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            keyboard: None, // Old field for backward compatibility
            input: InputConfig::default(),
            window_width: 512,  // 256 * 2 (default 2x scale)
            window_height: 480, // 240 * 2 (default 2x scale)
            last_rom_path: None,
            display_filter: DisplayFilter::default(),
            emulation_speed: 1.0,
            video_backend: "software".to_string(),
            extra: HashMap::new(),
        }
    }
}

impl Settings {
    /// Get the config file path relative to the current working directory
    pub fn config_path() -> PathBuf {
        let mut path = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        path.push("config.json");
        path
    }

    /// Load settings from config.json, falling back to defaults on error
    pub fn load() -> Self {
        let path = Self::config_path();
        match fs::read_to_string(&path) {
            Ok(contents) => match serde_json::from_str::<Settings>(&contents) {
                Ok(mut settings) => {
                    // Migrate old keyboard field to new input.player1
                    if let Some(old_keyboard) = settings.keyboard.take() {
                        settings.input.player1 = old_keyboard;
                    }
                    settings
                }
                Err(e) => {
                    eprintln!(
                        "Warning: Failed to parse config.json: {}. Using defaults.",
                        e
                    );
                    Self::default()
                }
            },
            Err(_) => {
                // File doesn't exist or can't be read, use defaults
                Self::default()
            }
        }
    }

    /// Save settings to config.json immediately
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = Self::config_path();
        let contents = serde_json::to_string_pretty(self)?;
        fs::write(&path, contents)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_settings() {
        let settings = Settings::default();
        assert_eq!(settings.input.player1.a, "Z");
        assert_eq!(settings.input.player1.b, "X");
        assert_eq!(settings.input.player2.a, "U");
        assert_eq!(settings.input.player2.b, "O");
        assert_eq!(settings.input.host_modifier, "RightCtrl");
        assert_eq!(settings.window_width, 512);
        assert_eq!(settings.window_height, 480);
        assert_eq!(settings.last_rom_path, None);
    }

    #[test]
    fn test_settings_serialization() {
        let settings = Settings::default();
        let json = serde_json::to_string(&settings).expect("Failed to serialize");
        let deserialized: Settings = serde_json::from_str(&json).expect("Failed to deserialize");
        assert_eq!(deserialized.input.player1.a, settings.input.player1.a);
        assert_eq!(deserialized.input.player2.a, settings.input.player2.a);
        assert_eq!(deserialized.window_width, settings.window_width);
    }

    #[test]
    fn test_settings_save_load() {
        use std::fs;

        let test_dir = std::env::temp_dir().join("hemulator_test_settings");
        fs::create_dir_all(&test_dir).unwrap();

        // Override config path for testing
        let test_config = test_dir.join("test_config.json");

        let settings = Settings {
            last_rom_path: Some("/test/path/game.nes".to_string()),
            window_width: 1024,
            window_height: 960,
            ..Default::default()
        };

        // Manually save to test path
        let contents = serde_json::to_string_pretty(&settings).unwrap();
        fs::write(&test_config, contents).unwrap();

        // Manually load from test path
        let loaded_contents = fs::read_to_string(&test_config).unwrap();
        let loaded: Settings = serde_json::from_str(&loaded_contents).unwrap();

        // last_rom_path is skip_serializing, so it won't be saved
        assert_eq!(loaded.last_rom_path, None);
        assert_eq!(loaded.window_width, 1024);
        assert_eq!(loaded.window_height, 960);

        // Clean up
        fs::remove_dir_all(&test_dir).unwrap();
    }

    #[test]
    fn test_backward_compatibility_migration() {
        // Test that old keyboard field migrates to input.player1
        let old_format = r#"{
            "keyboard": {
                "a": "Z",
                "b": "X",
                "select": "LeftShift",
                "start": "Enter",
                "up": "Up",
                "down": "Down",
                "left": "Left",
                "right": "Right"
            },
            "window_width": 512,
            "window_height": 480
        }"#;

        let settings: Settings = serde_json::from_str(old_format).unwrap();
        assert_eq!(settings.input.player1.a, "Z");
        assert_eq!(settings.input.player1.b, "X");
    }

    #[test]
    fn test_multi_player_defaults() {
        let settings = Settings::default();

        // Player 1 uses default mappings
        assert_eq!(settings.input.player1.a, "Z");
        assert_eq!(settings.input.player1.up, "Up");

        // Player 2 has different default mappings
        assert_eq!(settings.input.player2.a, "U");
        assert_eq!(settings.input.player2.b, "O");
        assert_eq!(settings.input.player2.up, "I");
        assert_eq!(settings.input.player2.start, "P");

        // Players 3 and 4 are unmapped by default
        assert!(settings.input.player3.a.is_empty());
        assert!(settings.input.player4.a.is_empty());
    }
}
