use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Maximum number of save slots per game
pub const MAX_SAVE_SLOTS: u8 = 5;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveSlot {
    pub data: String, // Base64 encoded save state data
    pub timestamp: u64,
    #[serde(default)]
    pub rom_hash: Option<String>, // Hash of the ROM this state was saved with
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GameSaves {
    pub slots: HashMap<u8, SaveSlot>, // Slots 1-5
}

impl GameSaves {
    /// Calculate SHA256 hash of ROM data
    pub fn rom_hash(rom_data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(rom_data);
        format!("{:x}", hasher.finalize())
    }

    /// Get the saves directory path
    pub fn saves_dir() -> PathBuf {
        let mut path = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."));
        path.push("saves");
        path
    }

    /// Get the path to a game's save file
    pub fn game_save_path(rom_hash: &str) -> PathBuf {
        let mut path = Self::saves_dir();
        path.push(rom_hash);
        path.push("states.json");
        path
    }

    /// Load saves for a specific game
    pub fn load(rom_hash: &str) -> Self {
        let path = Self::game_save_path(rom_hash);
        match fs::read_to_string(&path) {
            Ok(contents) => match serde_json::from_str(&contents) {
                Ok(saves) => saves,
                Err(e) => {
                    eprintln!(
                        "Warning: Failed to parse save file: {}. Using empty saves.",
                        e
                    );
                    Self::default()
                }
            },
            Err(_) => {
                // File doesn't exist or can't be read
                Self::default()
            }
        }
    }

    /// Save the game saves to disk
    pub fn save(&self, rom_hash: &str) -> Result<(), Box<dyn std::error::Error>> {
        let path = Self::game_save_path(rom_hash);

        // Create parent directories if they don't exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let contents = serde_json::to_string_pretty(self)?;
        fs::write(&path, contents)?;
        Ok(())
    }

    /// Save state data to a specific slot (1-MAX_SAVE_SLOTS)
    pub fn save_slot(
        &mut self,
        slot: u8,
        data: &[u8],
        rom_hash: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if !(1..=MAX_SAVE_SLOTS).contains(&slot) {
            return Err(format!("Slot must be between 1 and {}", MAX_SAVE_SLOTS).into());
        }

        let encoded = BASE64.encode(data);
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();

        self.slots.insert(
            slot,
            SaveSlot {
                data: encoded,
                timestamp,
                rom_hash: Some(rom_hash.to_string()), // Store ROM hash for verification
            },
        );

        self.save(rom_hash)?;
        Ok(())
    }

    /// Load state data from a specific slot (1-MAX_SAVE_SLOTS)
    /// Verifies that the ROM hash matches if present in the save slot
    pub fn load_slot(
        &self,
        slot: u8,
        current_rom_hash: &str,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        if !(1..=MAX_SAVE_SLOTS).contains(&slot) {
            return Err(format!("Slot must be between 1 and {}", MAX_SAVE_SLOTS).into());
        }

        match self.slots.get(&slot) {
            Some(save_slot) => {
                // Verify ROM hash if present in save slot
                if let Some(ref saved_hash) = save_slot.rom_hash {
                    if saved_hash != current_rom_hash {
                        return Err(
                            "ROM hash mismatch: save state was created with a different ROM"
                                .to_string()
                                .into(),
                        );
                    }
                }

                let decoded = BASE64.decode(&save_slot.data)?;
                Ok(decoded)
            }
            None => Err(format!("No save data in slot {}", slot).into()),
        }
    }

    /// Check if a slot has data
    #[cfg(test)]
    pub fn has_slot(&self, slot: u8) -> bool {
        self.slots.contains_key(&slot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rom_hash() {
        let rom_data = b"test rom data";
        let hash = GameSaves::rom_hash(rom_data);
        assert_eq!(hash.len(), 64); // SHA256 produces 64 hex characters
                                    // Hash should be consistent
        assert_eq!(hash, GameSaves::rom_hash(rom_data));
    }

    #[test]
    fn test_save_load_slot() {
        let mut saves = GameSaves::default();
        let test_data = b"test save state data";
        let rom_hash = "test_hash_12345";

        // Create temporary test directory
        let test_dir = std::env::temp_dir().join("hemulator_test_saves");
        let _test_path = test_dir.join(rom_hash).join("states.json");

        // Manually save
        saves
            .save_slot(1, test_data, rom_hash)
            .expect("Failed to save");

        // Load from the same hash
        let loaded = GameSaves::load(rom_hash);
        let decoded = loaded.load_slot(1, rom_hash).expect("Failed to load slot");

        assert_eq!(decoded, test_data);
        assert!(loaded.has_slot(1));
        assert!(!loaded.has_slot(2));

        // Clean up
        if test_dir.exists() {
            fs::remove_dir_all(&test_dir).unwrap();
        }
    }

    #[test]
    fn test_slot_validation() {
        let saves = GameSaves::default();
        let rom_hash = "test_hash";

        // Test invalid slots
        assert!(saves.load_slot(0, rom_hash).is_err());
        assert!(saves.load_slot(6, rom_hash).is_err());

        // Test valid slot that's empty
        assert!(saves.load_slot(3, rom_hash).is_err());
    }

    #[test]
    fn test_base64_encoding() {
        let mut saves = GameSaves::default();
        let test_data = b"\x00\x01\x02\xFF\xFE\xFD"; // Binary data
        let rom_hash = "test_binary_hash";

        saves.save_slot(2, test_data, rom_hash).unwrap();
        let loaded = GameSaves::load(rom_hash);
        let decoded = loaded.load_slot(2, rom_hash).unwrap();

        assert_eq!(decoded, test_data);

        // Clean up
        let test_dir = std::env::temp_dir().join("hemulator_test_saves");
        if test_dir.exists() {
            fs::remove_dir_all(&test_dir).unwrap();
        }
    }

    #[test]
    fn test_rom_hash_verification() {
        let mut saves = GameSaves::default();
        let test_data = b"test state data";
        let rom_hash1 = "original_rom_hash";
        let rom_hash2 = "different_rom_hash";

        // Save with one ROM hash
        saves.save_slot(1, test_data, rom_hash1).unwrap();

        // Try to load with different ROM hash - should fail
        let loaded = GameSaves::load(rom_hash1);
        let result = loaded.load_slot(1, rom_hash2);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("ROM hash mismatch"));

        // Load with correct ROM hash - should succeed
        let result = loaded.load_slot(1, rom_hash1);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), test_data);

        // Clean up
        let test_dir = std::env::temp_dir().join("hemulator_test_saves");
        if test_dir.exists() {
            fs::remove_dir_all(&test_dir).unwrap();
        }
    }
}
