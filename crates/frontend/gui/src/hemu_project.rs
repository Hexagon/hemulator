/// .hemu project file format for all systems
use crate::display_filter::DisplayFilter;
use crate::settings::InputConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Display settings (window size and CRT filter)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplaySettings {
    /// Window width in pixels
    pub window_width: usize,
    /// Window height in pixels
    pub window_height: usize,
    /// CRT display filter
    #[serde(default)]
    pub display_filter: DisplayFilter,
}

impl Default for DisplaySettings {
    fn default() -> Self {
        Self {
            window_width: 512,
            window_height: 480,
            display_filter: DisplayFilter::default(),
        }
    }
}

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
    /// Display settings (window size and filter)
    #[serde(default)]
    pub display: DisplaySettings,
    /// Optional input config override (overrides global config.json settings)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<InputConfig>,
    /// Boot priority for PC systems (optional, defaults to FloppyFirst)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub boot_priority: Option<String>,
    /// CPU model for PC systems (optional, defaults to Intel8086)
    /// Valid values: "Intel8086", "Intel8088", "Intel80186", "Intel80188", "Intel80286", "Intel80386"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu_model: Option<String>,
    /// Memory size in KB for PC systems (optional, defaults to 640)
    /// Common values: 256, 512, 640 (maximum conventional memory)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_kb: Option<u32>,
    /// Video mode for PC systems (optional, defaults to "CGA")
    /// Valid values: "CGA", "EGA", "VGA"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video_mode: Option<String>,
}

impl HemuProject {
    /// Create a new project for a given system
    pub fn new(system: String) -> Self {
        Self {
            version: 1,
            system,
            mounts: HashMap::new(),
            display: DisplaySettings::default(),
            input: None,
            boot_priority: None,
            cpu_model: None,
            memory_kb: None,
            video_mode: None,
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
    #[allow(dead_code)]
    pub fn get_mount(&self, mount_id: &str) -> Option<&String> {
        self.mounts.get(mount_id)
    }

    /// Set boot priority (for PC systems)
    #[allow(dead_code)]
    pub fn set_boot_priority(&mut self, priority: String) {
        self.boot_priority = Some(priority);
    }

    /// Get boot priority
    pub fn get_boot_priority(&self) -> Option<&String> {
        self.boot_priority.as_ref()
    }

    /// Set CPU model (for PC systems)
    #[allow(dead_code)]
    pub fn set_cpu_model(&mut self, model: String) {
        self.cpu_model = Some(model);
    }

    /// Get CPU model
    pub fn get_cpu_model(&self) -> Option<&String> {
        self.cpu_model.as_ref()
    }

    /// Set memory size in KB (for PC systems)
    #[allow(dead_code)]
    pub fn set_memory_kb(&mut self, kb: u32) {
        self.memory_kb = Some(kb);
    }

    /// Get memory size in KB
    pub fn get_memory_kb(&self) -> Option<u32> {
        self.memory_kb
    }

    /// Set video mode (for PC systems)
    #[allow(dead_code)]
    pub fn set_video_mode(&mut self, mode: String) {
        self.video_mode = Some(mode);
    }

    /// Get video mode
    pub fn get_video_mode(&self) -> Option<&String> {
        self.video_mode.as_ref()
    }

    /// Set display settings
    pub fn set_display_settings(&mut self, width: usize, height: usize, filter: DisplayFilter) {
        self.display.window_width = width;
        self.display.window_height = height;
        self.display.display_filter = filter;
    }

    /// Get display settings
    pub fn get_display_settings(&self) -> &DisplaySettings {
        &self.display
    }

    /// Set input config override
    pub fn set_input_override(&mut self, input: InputConfig) {
        self.input = Some(input);
    }

    /// Get input config override
    pub fn get_input_override(&self) -> Option<&InputConfig> {
        self.input.as_ref()
    }

    /// Get the list of mount point IDs that are relevant for this system
    pub fn relevant_mount_points(&self) -> Vec<&str> {
        match self.system.as_str() {
            "pc" => vec!["BIOS", "FloppyA", "FloppyB", "HardDrive"],
            "nes" | "gb" | "gameboy" | "atari2600" | "snes" | "n64" => vec!["Cartridge"],
            _ => vec![],
        }
    }

    /// Check if system has multiple mount points (requires .hemu file)
    #[allow(dead_code)]
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
        assert_eq!(loaded.get_mount("FloppyA"), Some(&"disk.img".to_string()));
        assert_eq!(
            loaded.get_boot_priority(),
            Some(&"HardDriveFirst".to_string())
        );

        // Cleanup
        fs::remove_file(test_file).ok();
    }

    #[test]
    fn test_boot_priority() {
        let mut project = HemuProject::new("pc".to_string());
        assert_eq!(project.get_boot_priority(), None);

        project.set_boot_priority("FloppyFirst".to_string());
        assert_eq!(
            project.get_boot_priority(),
            Some(&"FloppyFirst".to_string())
        );
    }

    #[test]
    fn test_multi_mount_detection() {
        assert!(HemuProject::is_multi_mount_system("pc"));
        assert!(!HemuProject::is_multi_mount_system("nes"));
        assert!(!HemuProject::is_multi_mount_system("gb"));
        assert!(!HemuProject::is_multi_mount_system("atari2600"));
    }

    #[test]
    fn test_cpu_model() {
        let mut project = HemuProject::new("pc".to_string());
        assert_eq!(project.get_cpu_model(), None);

        project.set_cpu_model("Intel80286".to_string());
        assert_eq!(project.get_cpu_model(), Some(&"Intel80286".to_string()));
    }

    #[test]
    fn test_memory_kb() {
        let mut project = HemuProject::new("pc".to_string());
        assert_eq!(project.get_memory_kb(), None);

        project.set_memory_kb(512);
        assert_eq!(project.get_memory_kb(), Some(512));
    }

    #[test]
    fn test_video_mode() {
        let mut project = HemuProject::new("pc".to_string());
        assert_eq!(project.get_video_mode(), None);

        project.set_video_mode("VGA".to_string());
        assert_eq!(project.get_video_mode(), Some(&"VGA".to_string()));
    }

    #[test]
    fn test_save_load_with_all_pc_options() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_project_full.hemu");

        let mut project = HemuProject::new("pc".to_string());
        project.set_mount("BIOS".to_string(), "bios.rom".to_string());
        project.set_mount("FloppyA".to_string(), "disk.img".to_string());
        project.set_boot_priority("HardDriveFirst".to_string());
        project.set_cpu_model("Intel80286".to_string());
        project.set_memory_kb(512);
        project.set_video_mode("EGA".to_string());

        // Save
        project.save(&test_file).expect("Failed to save");

        // Load
        let loaded = HemuProject::load(&test_file).expect("Failed to load");
        assert_eq!(loaded.system, "pc");
        assert_eq!(loaded.get_mount("BIOS"), Some(&"bios.rom".to_string()));
        assert_eq!(loaded.get_mount("FloppyA"), Some(&"disk.img".to_string()));
        assert_eq!(
            loaded.get_boot_priority(),
            Some(&"HardDriveFirst".to_string())
        );
        assert_eq!(loaded.get_cpu_model(), Some(&"Intel80286".to_string()));
        assert_eq!(loaded.get_memory_kb(), Some(512));
        assert_eq!(loaded.get_video_mode(), Some(&"EGA".to_string()));

        // Cleanup
        fs::remove_file(test_file).ok();
    }
}
