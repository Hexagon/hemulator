use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyMapping {
    pub a: String,
    pub b: String,
    pub select: String,
    pub start: String,
    pub up: String,
    pub down: String,
    pub left: String,
    pub right: String,
}

impl Default for KeyMapping {
    fn default() -> Self {
        Self {
            a: "Z".to_string(),
            b: "X".to_string(),
            select: "LeftShift".to_string(),
            start: "Enter".to_string(),
            up: "Up".to_string(),
            down: "Down".to_string(),
            left: "Left".to_string(),
            right: "Right".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub keyboard: KeyMapping,
    pub window_width: usize,
    pub window_height: usize,
    pub scale: u8,
    pub fullscreen: bool,
    pub last_rom_path: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            keyboard: KeyMapping::default(),
            window_width: 256,
            window_height: 240,
            scale: 2,
            fullscreen: false,
            last_rom_path: None,
        }
    }
}

impl Settings {
    /// Get the config file path relative to the executable
    pub fn config_path() -> PathBuf {
        let mut path = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."));
        path.push("config.json");
        path
    }

    /// Load settings from config.json, falling back to defaults on error
    pub fn load() -> Self {
        let path = Self::config_path();
        match fs::read_to_string(&path) {
            Ok(contents) => match serde_json::from_str(&contents) {
                Ok(settings) => settings,
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
        assert_eq!(settings.keyboard.a, "Z");
        assert_eq!(settings.keyboard.b, "X");
        assert_eq!(settings.scale, 2);
        assert!(!settings.fullscreen);
        assert_eq!(settings.last_rom_path, None);
    }

    #[test]
    fn test_settings_serialization() {
        let settings = Settings::default();
        let json = serde_json::to_string(&settings).expect("Failed to serialize");
        let deserialized: Settings = serde_json::from_str(&json).expect("Failed to deserialize");
        assert_eq!(deserialized.keyboard.a, settings.keyboard.a);
        assert_eq!(deserialized.scale, settings.scale);
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
            scale: 4,
            ..Default::default()
        };

        // Manually save to test path
        let contents = serde_json::to_string_pretty(&settings).unwrap();
        fs::write(&test_config, contents).unwrap();

        // Manually load from test path
        let loaded_contents = fs::read_to_string(&test_config).unwrap();
        let loaded: Settings = serde_json::from_str(&loaded_contents).unwrap();

        assert_eq!(
            loaded.last_rom_path,
            Some("/test/path/game.nes".to_string())
        );
        assert_eq!(loaded.scale, 4);

        // Clean up
        fs::remove_dir_all(&test_dir).unwrap();
    }
}
