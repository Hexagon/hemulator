use crate::display_filter::DisplayFilter;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Scaling mode for emulator display
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ScalingMode {
    /// Original size: 1:1 pixel mapping, centered
    Original,
    /// Fit to window: Scale to fit maintaining aspect ratio
    #[default]
    Fit,
    /// Stretch: Fill entire window, ignoring aspect ratio
    Stretch,
}

impl ScalingMode {
    pub fn name(&self) -> &str {
        match self {
            ScalingMode::Original => "Original",
            ScalingMode::Fit => "Fit",
            ScalingMode::Stretch => "Stretch",
        }
    }
}

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

    #[serde(default = "default_window_width")]
    pub window_width: usize,
    #[serde(default = "default_window_height")]
    pub window_height: usize,
    #[serde(default, skip_serializing)]
    pub last_rom_path: Option<String>, // Kept for backward compatibility reading only, not saved
    #[serde(default)]
    pub display_filter: DisplayFilter,
    #[serde(default = "default_emulation_speed", skip_serializing)] // Runtime only, not saved
    pub emulation_speed: f64, // Speed multiplier: 0.0 (pause), 0.25, 0.5, 1.0, 2.0, 10.0
    #[serde(default = "default_video_backend")]
    pub video_backend: String, // "software" or "opengl"
    #[serde(default)]
    pub scaling_mode: ScalingMode, // How to scale emulator display
    #[serde(default, skip_serializing)] // Runtime only, not saved
    pub fullscreen: bool, // Fullscreen state
    #[serde(default, skip_serializing)] // Runtime only, not saved
    pub fullscreen_with_gui: bool, // Fullscreen with GUI overlay
    #[serde(default, flatten, skip_serializing_if = "HashMap::is_empty")]
    pub extra: HashMap<String, Value>,
}

fn default_window_width() -> usize {
    512 // 256 * 2 (default 2x scale)
}

fn default_window_height() -> usize {
    480 // 240 * 2 (default 2x scale)
}

fn default_emulation_speed() -> f64 {
    1.0 // Normal speed
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
            scaling_mode: ScalingMode::default(),
            fullscreen: false,
            fullscreen_with_gui: false,
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

#[test]
fn test_settings_missing_fields_deserialize() {
    // Test the exact scenario from the bug report:
    // When config.json has been serialized but is missing some fields,
    // deserialization should use defaults for missing fields

    // Case 1: Missing window_width and window_height
    let json_missing_size = r#"{
  "input": {
    "player1": {
      "a": "Z",
      "b": "X",
      "select": "LeftShift",
      "start": "Enter",
      "up": "Up",
      "down": "Down",
      "left": "Left",
      "right": "Right"
    },
    "player2": {
      "a": "U",
      "b": "O",
      "select": "RightShift",
      "start": "P",
      "up": "I",
      "down": "K",
      "left": "J",
      "right": "L"
    },
    "player3": {
      "a": "",
      "b": "",
      "select": "",
      "start": "",
      "up": "",
      "down": "",
      "left": "",
      "right": ""
    },
    "player4": {
      "a": "",
      "b": "",
      "select": "",
      "start": "",
      "up": "",
      "down": "",
      "left": "",
      "right": ""
    },
    "host_modifier": "RightCtrl"
  },
  "video_backend": "software"
}"#;

    let result = serde_json::from_str::<Settings>(json_missing_size);
    match result {
        Ok(settings) => {
            assert_eq!(
                settings.window_width, 512,
                "window_width should default to 512"
            );
            assert_eq!(
                settings.window_height, 480,
                "window_height should default to 480"
            );
            println!("Test passed: Missing fields use defaults correctly");
        }
        Err(e) => {
            panic!(
                "Test FAILED: Could not deserialize JSON with missing window size fields: {}",
                e
            );
        }
    }
}

#[test]
fn test_settings_empty_json_deserialize() {
    // Edge case: Completely empty JSON should use all defaults
    let empty_json = "{}";

    let result = serde_json::from_str::<Settings>(empty_json);
    match result {
        Ok(settings) => {
            assert_eq!(
                settings.window_width, 512,
                "window_width should default to 512"
            );
            assert_eq!(
                settings.window_height, 480,
                "window_height should default to 480"
            );
            assert_eq!(
                settings.video_backend, "software",
                "video_backend should default to 'software'"
            );
            assert_eq!(
                settings.input.player1.a, "Z",
                "player1.a should default to 'Z'"
            );
            assert_eq!(
                settings.input.host_modifier, "RightCtrl",
                "host_modifier should default to 'RightCtrl'"
            );
            println!("Test passed: Empty JSON uses all defaults correctly");
        }
        Err(e) => {
            panic!("Test FAILED: Could not deserialize empty JSON: {}", e);
        }
    }
}

#[test]
fn test_settings_roundtrip_with_saved_config() {
    use std::fs;

    // This simulates the real-world scenario from the bug report:
    // 1. App starts without config.json
    // 2. Settings::load() returns defaults
    // 3. settings.save() writes config.json
    // 4. App restarts and loads config.json

    let test_dir = std::env::temp_dir().join("hemulator_roundtrip_test");
    let _ = fs::remove_dir_all(&test_dir); // Clean up from previous runs
    fs::create_dir_all(&test_dir).unwrap();

    // Change to test directory
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(&test_dir).unwrap();

    // Step 1: First launch - no config.json exists
    let settings1 = Settings::load();
    assert_eq!(settings1.window_width, 512);
    assert_eq!(settings1.window_height, 480);

    // Step 2: Save settings (creates config.json)
    settings1.save().unwrap();

    // Verify config.json was created
    assert!(test_dir.join("config.json").exists());

    // Step 3: Second launch - load from config.json
    let settings2 = Settings::load();
    assert_eq!(
        settings2.window_width, 512,
        "Loaded window_width should match saved value"
    );
    assert_eq!(
        settings2.window_height, 480,
        "Loaded window_height should match saved value"
    );
    assert_eq!(settings2.video_backend, "software");
    assert_eq!(settings2.input.player1.a, "Z");

    // Step 4: Simulate an old or partial config.json (manually edit it)
    let config_content = fs::read_to_string(test_dir.join("config.json")).unwrap();
    let mut json: serde_json::Value = serde_json::from_str(&config_content).unwrap();

    // Remove window_width and window_height to simulate old config
    if let serde_json::Value::Object(ref mut obj) = json {
        obj.remove("window_width");
        obj.remove("window_height");
    }

    // Write the modified config back
    fs::write(
        test_dir.join("config.json"),
        serde_json::to_string_pretty(&json).unwrap(),
    )
    .unwrap();

    // Step 5: Third launch - should still work with missing fields
    let settings3 = Settings::load();
    assert_eq!(
        settings3.window_width, 512,
        "Missing window_width should use default"
    );
    assert_eq!(
        settings3.window_height, 480,
        "Missing window_height should use default"
    );

    // Cleanup
    std::env::set_current_dir(&original_dir).unwrap();
    fs::remove_dir_all(&test_dir).unwrap();

    println!("Roundtrip test passed!");
}

#[test]
fn test_actual_problematic_config() {
    // Test the exact config.json from the bug report
    let problematic_json = r#"{
  "input": {
    "player1": {
      "a": "Z",
      "b": "X",
      "x": "",
      "y": "",
      "l": "",
      "r": "",
      "select": "LeftShift",
      "start": "Enter",
      "up": "Up",
      "down": "Down",
      "left": "Left",
      "right": "Right"
    },
    "player2": {
      "a": "U",
      "b": "O",
      "x": "",
      "y": "",
      "l": "",
      "r": "",
      "select": "RightShift",
      "start": "P",
      "up": "I",
      "down": "K",
      "left": "J",
      "right": "L"
    },
    "player3": {
      "a": "",
      "b": "",
      "x": "",
      "y": "",
      "l": "",
      "r": "",
      "select": "",
      "start": "",
      "up": "",
      "down": "",
      "left": "",
      "right": ""
    },
    "player4": {
      "a": "",
      "b": "",
      "x": "",
      "y": "",
      "l": "",
      "r": "",
      "select": "",
      "start": "",
      "up": "",
      "down": "",
      "left": "",
      "right": ""
    },
    "host_modifier": "RightCtrl"
  },
  "window_width": 640,
  "window_height": 480,
  "display_filter": "None",
  "video_backend": "software"
}"#;

    println!("Testing problematic config JSON...");
    let result = serde_json::from_str::<Settings>(problematic_json);
    match result {
        Ok(settings) => {
            println!("Successfully deserialized!");
            println!("window_width: {}", settings.window_width);
            println!("window_height: {}", settings.window_height);
            println!("display_filter: {:?}", settings.display_filter);
            println!("video_backend: {}", settings.video_backend);
            println!("player1.a: {}", settings.input.player1.a);
            println!("player1.x: '{}'", settings.input.player1.x);
        }
        Err(e) => {
            panic!("FAILED to deserialize: {}", e);
        }
    }
}

#[test]
fn test_emulation_speed_default_issue() {
    // The REAL bug: emulation_speed has #[serde(default)] which uses f64::default() = 0.0
    // When it's 0.0, the emulator is paused, causing a black screen!

    let problematic_json = r#"{
  "input": {
    "player1": {
      "a": "Z",
      "b": "X",
      "select": "LeftShift",
      "start": "Enter",
      "up": "Up",
      "down": "Down",
      "left": "Left",
      "right": "Right"
    },
    "player2": {
      "a": "U",
      "b": "O",
      "select": "RightShift",
      "start": "P",
      "up": "I",
      "down": "K",
      "left": "J",
      "right": "L"
    },
    "player3": { "a": "", "b": "", "select": "", "start": "", "up": "", "down": "", "left": "", "right": "" },
    "player4": { "a": "", "b": "", "select": "", "start": "", "up": "", "down": "", "left": "", "right": "" },
    "host_modifier": "RightCtrl"
  },
  "window_width": 640,
  "window_height": 480,
  "display_filter": "None",
  "video_backend": "software"
}"#;

    let result = serde_json::from_str::<Settings>(problematic_json);
    match result {
        Ok(settings) => {
            println!("Deserialized successfully");
            println!("emulation_speed: {}", settings.emulation_speed);

            // This is the bug! emulation_speed defaults to 0.0 instead of 1.0
            // When it's 0.0, the emulator is paused!
            if settings.emulation_speed == 0.0 {
                panic!("BUG FOUND! emulation_speed is 0.0 (paused) instead of 1.0 (normal speed). This causes a black screen!");
            }
        }
        Err(e) => {
            panic!("Failed to deserialize: {}", e);
        }
    }
}

#[test]
fn test_scaling_mode_names() {
    assert_eq!(ScalingMode::Original.name(), "Original");
    assert_eq!(ScalingMode::Fit.name(), "Fit");
    assert_eq!(ScalingMode::Stretch.name(), "Stretch");
}

#[test]
fn test_scaling_mode_default() {
    assert_eq!(ScalingMode::default(), ScalingMode::Fit);
}

#[test]
fn test_scaling_mode_serialization() {
    // Test that scaling modes can be serialized/deserialized
    let modes = vec![
        ScalingMode::Original,
        ScalingMode::Fit,
        ScalingMode::Stretch,
    ];

    for mode in modes {
        let json = serde_json::to_string(&mode).expect("Failed to serialize");
        let deserialized: ScalingMode = serde_json::from_str(&json).expect("Failed to deserialize");
        assert_eq!(mode, deserialized);
    }
}

#[test]
fn test_settings_with_scaling_mode() {
    let mut settings = Settings::default();
    assert_eq!(settings.scaling_mode, ScalingMode::Fit);

    settings.scaling_mode = ScalingMode::Original;
    assert_eq!(settings.scaling_mode, ScalingMode::Original);

    settings.scaling_mode = ScalingMode::Stretch;
    assert_eq!(settings.scaling_mode, ScalingMode::Stretch);
}
