use crate::crt_filter::CrtFilter;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
    #[serde(default)]
    pub last_rom_path: Option<String>, // Kept for backward compatibility
    #[serde(default)]
    pub mount_points: HashMap<String, String>, // mount_point_id -> file_path
    #[serde(default)]
    pub crt_filter: CrtFilter,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            keyboard: KeyMapping::default(),
            window_width: 512,  // 256 * 2 (default 2x scale)
            window_height: 480, // 240 * 2 (default 2x scale)
            last_rom_path: None,
            mount_points: HashMap::new(),
            crt_filter: CrtFilter::default(),
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

    /// Set a mount point path
    pub fn set_mount_point(&mut self, mount_point_id: &str, path: String) {
        self.mount_points.insert(mount_point_id.to_string(), path);
    }

    /// Get a mount point path
    pub fn get_mount_point(&self, mount_point_id: &str) -> Option<&String> {
        self.mount_points.get(mount_point_id)
    }

    /// Clear a mount point path
    pub fn clear_mount_point(&mut self, mount_point_id: &str) {
        self.mount_points.remove(mount_point_id);
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
        assert_eq!(settings.window_width, 512);
        assert_eq!(settings.window_height, 480);
        assert_eq!(settings.last_rom_path, None);
    }

    #[test]
    fn test_settings_serialization() {
        let settings = Settings::default();
        let json = serde_json::to_string(&settings).expect("Failed to serialize");
        let deserialized: Settings = serde_json::from_str(&json).expect("Failed to deserialize");
        assert_eq!(deserialized.keyboard.a, settings.keyboard.a);
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

        assert_eq!(
            loaded.last_rom_path,
            Some("/test/path/game.nes".to_string())
        );
        assert_eq!(loaded.window_width, 1024);
        assert_eq!(loaded.window_height, 960);

        // Clean up
        fs::remove_dir_all(&test_dir).unwrap();
    }
}
