/// .hemu project file format for multi-mount systems
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Represents a .hemu project file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HemuProject {
    /// Project format version
    pub version: u32,
    /// Target system (e.g., "pc", "nes", "gb")
    pub system: String,
    /// Mount points and their file paths
    /// Key: mount point ID (e.g., "BIOS", "FloppyA", "Cartridge")
    /// Value: file path (relative or absolute)
    pub mounts: HashMap<String, String>,
    /// Boot priority for PC systems (optional, defaults to FloppyFirst)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub boot_priority: Option<String>,
}

impl HemuProject {
    /// Create a new project for a given system
    pub fn new(system: String) -> Self {
        Self {
            version: 1,
            system,
            mounts: HashMap::new(),
            boot_priority: None,
        }
    }

    /// Load a project from a .hemu file
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let contents = fs::read_to_string(path)?;
        let project: HemuProject = serde_json::from_str(&contents)?;
        Ok(project)
    }

    /// Save the project to a .hemu file
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), Box<dyn std::error::Error>> {
        let contents = serde_json::to_string_pretty(self)?;
        fs::write(path, contents)?;
        Ok(())
    }

    /// Set a mount point
    pub fn set_mount(&mut self, mount_id: String, file_path: String) {
        self.mounts.insert(mount_id, file_path);
    }

    /// Get a mount point path
    pub fn get_mount(&self, mount_id: &str) -> Option<&String> {
        self.mounts.get(mount_id)
    }

    /// Set boot priority (for PC systems)
    pub fn set_boot_priority(&mut self, priority: String) {
        self.boot_priority = Some(priority);
    }

    /// Get boot priority
    pub fn get_boot_priority(&self) -> Option<&String> {
        self.boot_priority.as_ref()
    }

    /// Check if system has multiple mount points (requires .hemu file)
    pub fn is_multi_mount_system(system: &str) -> bool {
        matches!(system, "pc")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_create_project() {
        let project = HemuProject::new("pc".to_string());
        assert_eq!(project.system, "pc");
        assert_eq!(project.version, 1);
        assert!(project.mounts.is_empty());
    }

    #[test]
    fn test_set_mount() {
        let mut project = HemuProject::new("pc".to_string());
        project.set_mount("BIOS".to_string(), "bios.rom".to_string());
        assert_eq!(project.get_mount("BIOS"), Some(&"bios.rom".to_string()));
    }

    #[test]
    fn test_save_load_roundtrip() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_project.hemu");

        let mut project = HemuProject::new("pc".to_string());
        project.set_mount("BIOS".to_string(), "bios.rom".to_string());
        project.set_mount("FloppyA".to_string(), "disk.img".to_string());
        project.set_boot_priority("HardDriveFirst".to_string());

        // Save
        project.save(&test_file).expect("Failed to save");

        // Load
        let loaded = HemuProject::load(&test_file).expect("Failed to load");
        assert_eq!(loaded.system, "pc");
        assert_eq!(loaded.get_mount("BIOS"), Some(&"bios.rom".to_string()));
        assert_eq!(
            loaded.get_mount("FloppyA"),
            Some(&"disk.img".to_string())
        );
        assert_eq!(loaded.get_boot_priority(), Some(&"HardDriveFirst".to_string()));

        // Cleanup
        fs::remove_file(test_file).ok();
    }

    #[test]
    fn test_boot_priority() {
        let mut project = HemuProject::new("pc".to_string());
        assert_eq!(project.get_boot_priority(), None);

        project.set_boot_priority("FloppyFirst".to_string());
        assert_eq!(project.get_boot_priority(), Some(&"FloppyFirst".to_string()));
    }

    #[test]
    fn test_multi_mount_detection() {
        assert!(HemuProject::is_multi_mount_system("pc"));
        assert!(!HemuProject::is_multi_mount_system("nes"));
        assert!(!HemuProject::is_multi_mount_system("gb"));
        assert!(!HemuProject::is_multi_mount_system("atari2600"));
    }
}
