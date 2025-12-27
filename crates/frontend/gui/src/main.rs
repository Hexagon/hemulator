pub mod display_filter;
mod hemu_project;
mod rom_detect;
mod save_state;
mod settings;
mod ui_render;
pub mod video_processor;
pub mod window_backend;

use emu_core::{types::Frame, System};
use hemu_project::HemuProject;
use rodio::{OutputStream, Source};
use rom_detect::{detect_rom_type, SystemType};
use save_state::GameSaves;
use settings::Settings;
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::mpsc::{sync_channel, Receiver};
use std::time::{Duration, Instant};
use window_backend::{string_to_key, Key, Sdl2Backend, WindowBackend};

/// Runtime state for tracking currently loaded project and mounts
/// This replaces the mount_points field in Settings which has been deprecated
struct RuntimeState {
    /// Currently loaded .hemu project file path (if any)
    current_project_path: Option<PathBuf>,
    /// Current mount points (mount_id -> file_path)
    /// This is runtime-only and not persisted to config.json
    current_mounts: HashMap<String, String>,
}

impl RuntimeState {
    fn new() -> Self {
        Self {
            current_project_path: None,
            current_mounts: HashMap::new(),
        }
    }

    fn set_mount(&mut self, mount_id: String, path: String) {
        self.current_mounts.insert(mount_id, path);
    }

    fn get_mount(&self, mount_id: &str) -> Option<&String> {
        self.current_mounts.get(mount_id)
    }

    fn clear_mounts(&mut self) {
        self.current_mounts.clear();
    }

    fn set_project_path(&mut self, path: PathBuf) {
        self.current_project_path = Some(path);
    }

    fn clear_project_path(&mut self) {
        self.current_project_path = None;
    }
}

// System wrapper enum to support multiple emulated systems
// Box large variants to prevent stack overflow
#[allow(clippy::upper_case_acronyms)]
enum EmulatorSystem {
    NES(Box<emu_nes::NesSystem>),
    GameBoy(Box<emu_gb::GbSystem>),
    Atari2600(Box<emu_atari2600::Atari2600System>),
    PC(Box<emu_pc::PcSystem>),
    SNES(Box<emu_snes::SnesSystem>),
    N64(Box<emu_n64::N64System>),
}

impl EmulatorSystem {
    fn step_frame(&mut self) -> Result<Frame, Box<dyn std::error::Error>> {
        match self {
            EmulatorSystem::NES(sys) => sys
                .step_frame()
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
            EmulatorSystem::GameBoy(sys) => sys
                .step_frame()
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
            EmulatorSystem::Atari2600(sys) => sys
                .step_frame()
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
            EmulatorSystem::PC(sys) => sys
                .step_frame()
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
            EmulatorSystem::SNES(sys) => sys
                .step_frame()
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
            EmulatorSystem::N64(sys) => sys
                .step_frame()
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
        }
    }

    fn reset(&mut self) {
        match self {
            EmulatorSystem::NES(sys) => sys.reset(),
            EmulatorSystem::GameBoy(sys) => sys.reset(),
            EmulatorSystem::Atari2600(sys) => sys.reset(),
            EmulatorSystem::PC(sys) => sys.reset(),
            EmulatorSystem::SNES(sys) => sys.reset(),
            EmulatorSystem::N64(sys) => sys.reset(),
        }
    }

    fn mount(
        &mut self,
        mount_point_id: &str,
        data: &[u8],
    ) -> Result<(), Box<dyn std::error::Error>> {
        match self {
            EmulatorSystem::NES(sys) => sys
                .mount(mount_point_id, data)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
            EmulatorSystem::GameBoy(sys) => sys
                .mount(mount_point_id, data)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
            EmulatorSystem::Atari2600(sys) => sys
                .mount(mount_point_id, data)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
            EmulatorSystem::PC(sys) => sys
                .mount(mount_point_id, data)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
            EmulatorSystem::SNES(sys) => sys
                .mount(mount_point_id, data)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
            EmulatorSystem::N64(sys) => sys
                .mount(mount_point_id, data)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
        }
    }

    fn mount_points(&self) -> Vec<emu_core::MountPointInfo> {
        match self {
            EmulatorSystem::NES(sys) => sys.mount_points(),
            EmulatorSystem::GameBoy(sys) => sys.mount_points(),
            EmulatorSystem::Atari2600(sys) => sys.mount_points(),
            EmulatorSystem::PC(sys) => sys.mount_points(),
            EmulatorSystem::SNES(sys) => sys.mount_points(),
            EmulatorSystem::N64(sys) => sys.mount_points(),
        }
    }

    fn supports_save_states(&self) -> bool {
        match self {
            EmulatorSystem::NES(sys) => sys.supports_save_states(),
            EmulatorSystem::GameBoy(sys) => sys.supports_save_states(),
            EmulatorSystem::Atari2600(sys) => sys.supports_save_states(),
            EmulatorSystem::PC(sys) => sys.supports_save_states(),
            EmulatorSystem::SNES(sys) => sys.supports_save_states(),
            EmulatorSystem::N64(sys) => sys.supports_save_states(),
        }
    }

    fn save_state(&self) -> serde_json::Value {
        match self {
            EmulatorSystem::NES(sys) => sys.save_state(),
            EmulatorSystem::GameBoy(sys) => sys.save_state(),
            EmulatorSystem::Atari2600(sys) => sys.save_state(),
            EmulatorSystem::PC(sys) => sys.save_state(),
            EmulatorSystem::SNES(sys) => sys.save_state(),
            EmulatorSystem::N64(sys) => sys.save_state(),
        }
    }

    fn load_state(&mut self, state: &serde_json::Value) -> Result<(), serde_json::Error> {
        match self {
            EmulatorSystem::NES(sys) => sys.load_state(state),
            EmulatorSystem::GameBoy(sys) => sys.load_state(state),
            EmulatorSystem::Atari2600(sys) => sys.load_state(state),
            EmulatorSystem::PC(sys) => sys.load_state(state),
            EmulatorSystem::SNES(sys) => sys.load_state(state),
            EmulatorSystem::N64(sys) => sys.load_state(state),
        }
    }

    // System-specific methods
    fn set_controller(&mut self, port: usize, state: u8) {
        match self {
            EmulatorSystem::NES(sys) => sys.set_controller(port, state),
            EmulatorSystem::GameBoy(sys) => {
                // Game Boy only has one controller (port)
                // We'll map the standard button IDs to Game Boy buttons
                // Game Boy buttons: Right, Left, Up, Down, A, B, Select, Start (bits 0-7)
                if port == 0 {
                    // Convert from standard mapping (A, B, Select, Start, Up, Down, Left, Right)
                    // to Game Boy mapping (Right, Left, Up, Down, A, B, Select, Start)
                    let gb_state = ((state & 0x80) >> 7)  // Right (bit 7 -> bit 0)
                        | ((state & 0x40) >> 5)           // Left (bit 6 -> bit 1)
                        | ((state & 0x10) >> 2)           // Up (bit 4 -> bit 2)
                        | ((state & 0x20) >> 2)           // Down (bit 5 -> bit 3)
                        | ((state & 0x01) << 4)           // A (bit 0 -> bit 4)
                        | ((state & 0x02) << 4)           // B (bit 1 -> bit 5)
                        | ((state & 0x04) << 4)           // Select (bit 2 -> bit 6)
                        | ((state & 0x08) << 4); // Start (bit 3 -> bit 7)
                    sys.set_controller(gb_state);
                }
            }
            EmulatorSystem::Atari2600(_) => {}
            EmulatorSystem::PC(_) => {} // PC doesn't use controller input
            EmulatorSystem::SNES(_) => {} // SNES controller support stub
            EmulatorSystem::N64(_) => {} // N64 controller support stub
        }
    }

    fn handle_keyboard(&mut self, key: Key, pressed: bool) {
        if let EmulatorSystem::PC(sys) = self {
            // Map keys to PC scancodes
            let scancode = match key {
                Key::A => Some(emu_pc::SCANCODE_A),
                Key::B => Some(emu_pc::SCANCODE_B),
                Key::C => Some(emu_pc::SCANCODE_C),
                Key::D => Some(emu_pc::SCANCODE_D),
                Key::E => Some(emu_pc::SCANCODE_E),
                Key::F => Some(emu_pc::SCANCODE_F),
                Key::G => Some(emu_pc::SCANCODE_G),
                Key::H => Some(emu_pc::SCANCODE_H),
                Key::I => Some(emu_pc::SCANCODE_I),
                Key::J => Some(emu_pc::SCANCODE_J),
                Key::K => Some(emu_pc::SCANCODE_K),
                Key::L => Some(emu_pc::SCANCODE_L),
                Key::M => Some(emu_pc::SCANCODE_M),
                Key::N => Some(emu_pc::SCANCODE_N),
                Key::O => Some(emu_pc::SCANCODE_O),
                Key::P => Some(emu_pc::SCANCODE_P),
                Key::Q => Some(emu_pc::SCANCODE_Q),
                Key::R => Some(emu_pc::SCANCODE_R),
                Key::S => Some(emu_pc::SCANCODE_S),
                Key::T => Some(emu_pc::SCANCODE_T),
                Key::U => Some(emu_pc::SCANCODE_U),
                Key::V => Some(emu_pc::SCANCODE_V),
                Key::W => Some(emu_pc::SCANCODE_W),
                Key::X => Some(emu_pc::SCANCODE_X),
                Key::Y => Some(emu_pc::SCANCODE_Y),
                Key::Z => Some(emu_pc::SCANCODE_Z),
                Key::Key0 => Some(emu_pc::SCANCODE_0),
                Key::Key1 => Some(emu_pc::SCANCODE_1),
                Key::Key2 => Some(emu_pc::SCANCODE_2),
                Key::Key3 => Some(emu_pc::SCANCODE_3),
                Key::Key4 => Some(emu_pc::SCANCODE_4),
                Key::Key5 => Some(emu_pc::SCANCODE_5),
                Key::Key6 => Some(emu_pc::SCANCODE_6),
                Key::Key7 => Some(emu_pc::SCANCODE_7),
                Key::Key8 => Some(emu_pc::SCANCODE_8),
                Key::Key9 => Some(emu_pc::SCANCODE_9),
                Key::Space => Some(emu_pc::SCANCODE_SPACE),
                Key::Enter => Some(emu_pc::SCANCODE_ENTER),
                Key::Backspace => Some(emu_pc::SCANCODE_BACKSPACE),
                Key::Tab => Some(emu_pc::SCANCODE_TAB),
                Key::Escape => Some(emu_pc::SCANCODE_ESC),
                Key::LeftShift | Key::RightShift => Some(emu_pc::SCANCODE_LEFT_SHIFT),
                Key::LeftCtrl | Key::RightCtrl => Some(emu_pc::SCANCODE_LEFT_CTRL),
                Key::LeftAlt | Key::RightAlt => Some(emu_pc::SCANCODE_LEFT_ALT),
                _ => None,
            };

            if let Some(sc) = scancode {
                if pressed {
                    sys.key_press(sc);
                } else {
                    sys.key_release(sc);
                }
            }
        }
        // Other systems don't use keyboard input
    }

    fn get_debug_info_nes(&self) -> Option<emu_nes::DebugInfo> {
        match self {
            EmulatorSystem::NES(sys) => Some(sys.get_debug_info()),
            _ => None,
        }
    }

    fn get_debug_info_n64(&self) -> Option<emu_n64::DebugInfo> {
        match self {
            EmulatorSystem::N64(sys) => Some(sys.get_debug_info()),
            _ => None,
        }
    }

    fn get_debug_info_atari2600(&self) -> Option<emu_atari2600::DebugInfo> {
        match self {
            EmulatorSystem::Atari2600(sys) => sys.debug_info(),
            _ => None,
        }
    }

    fn get_debug_info_snes(&self) -> Option<emu_snes::DebugInfo> {
        match self {
            EmulatorSystem::SNES(sys) => Some(sys.get_debug_info()),
            _ => None,
        }
    }

    fn get_debug_info_pc(&self) -> Option<emu_pc::DebugInfo> {
        match self {
            EmulatorSystem::PC(sys) => Some(sys.debug_info()),
            _ => None,
        }
    }

    fn get_runtime_stats(&self) -> emu_nes::RuntimeStats {
        match self {
            EmulatorSystem::NES(sys) => sys.get_runtime_stats(),
            EmulatorSystem::GameBoy(_) => emu_nes::RuntimeStats::default(),
            EmulatorSystem::Atari2600(_) => emu_nes::RuntimeStats::default(),
            EmulatorSystem::PC(_) => emu_nes::RuntimeStats::default(),
            EmulatorSystem::SNES(_) => emu_nes::RuntimeStats::default(),
            EmulatorSystem::N64(_) => emu_nes::RuntimeStats::default(),
        }
    }

    fn timing(&self) -> emu_core::apu::TimingMode {
        match self {
            EmulatorSystem::NES(sys) => sys.timing(),
            EmulatorSystem::GameBoy(_) => emu_core::apu::TimingMode::Ntsc,
            EmulatorSystem::Atari2600(_) => emu_core::apu::TimingMode::Ntsc,
            EmulatorSystem::PC(_) => emu_core::apu::TimingMode::Ntsc,
            EmulatorSystem::SNES(_) => emu_core::apu::TimingMode::Ntsc,
            EmulatorSystem::N64(_) => emu_core::apu::TimingMode::Ntsc,
        }
    }

    fn get_audio_samples(&mut self, count: usize) -> Vec<i16> {
        match self {
            EmulatorSystem::NES(sys) => sys.get_audio_samples(count),
            EmulatorSystem::GameBoy(sys) => sys.get_audio_samples(count),
            EmulatorSystem::Atari2600(sys) => sys.get_audio_samples(count),
            EmulatorSystem::PC(_) => vec![0; count], // TODO: Implement audio for PC
            EmulatorSystem::SNES(_) => vec![0; count], // TODO: Implement audio for SNES
            EmulatorSystem::N64(_) => vec![0; count], // TODO: Implement audio for N64
        }
    }

    fn resolution(&self) -> (usize, usize) {
        match self {
            EmulatorSystem::NES(_) => (256, 240),
            EmulatorSystem::GameBoy(_) => (160, 144),
            EmulatorSystem::Atari2600(_) => (160, 192),
            EmulatorSystem::PC(_) => (640, 400),
            EmulatorSystem::SNES(_) => (256, 224),
            EmulatorSystem::N64(_) => (320, 240),
        }
    }

    fn system_name(&self) -> &str {
        match self {
            EmulatorSystem::NES(_) => "nes",
            EmulatorSystem::GameBoy(_) => "gameboy",
            EmulatorSystem::Atari2600(_) => "atari2600",
            EmulatorSystem::PC(_) => "pc",
            EmulatorSystem::SNES(_) => "snes",
            EmulatorSystem::N64(_) => "n64",
        }
    }

    /// Update POST screen for PC system
    fn update_post_screen(&mut self) {
        if let EmulatorSystem::PC(sys) = self {
            sys.update_post_screen();
        }
    }

    /// Check if this system requires the host key to be held for function keys
    /// Only PC system requires this to allow ESC and function keys to pass through to the emulated system
    fn requires_host_key_for_function_keys(&self) -> bool {
        matches!(self, EmulatorSystem::PC(_))
    }
}

fn key_mapping_to_button(key: Key, mapping: &settings::KeyMapping) -> Option<u8> {
    // Map key to button based on mapping
    if Some(key) == string_to_key(&mapping.a) {
        Some(0)
    } else if Some(key) == string_to_key(&mapping.b) {
        Some(1)
    } else if Some(key) == string_to_key(&mapping.select) {
        Some(2)
    } else if Some(key) == string_to_key(&mapping.start) {
        Some(3)
    } else if Some(key) == string_to_key(&mapping.up) {
        Some(4)
    } else if Some(key) == string_to_key(&mapping.down) {
        Some(5)
    } else if Some(key) == string_to_key(&mapping.left) {
        Some(6)
    } else if Some(key) == string_to_key(&mapping.right) {
        Some(7)
    } else {
        None
    }
}

/// Get controller state for a player from current keyboard state
fn get_controller_state(window: &dyn WindowBackend, mapping: &settings::KeyMapping) -> u8 {
    let keys_to_check: Vec<Key> = vec![
        string_to_key(&mapping.a),
        string_to_key(&mapping.b),
        string_to_key(&mapping.select),
        string_to_key(&mapping.start),
        string_to_key(&mapping.up),
        string_to_key(&mapping.down),
        string_to_key(&mapping.left),
        string_to_key(&mapping.right),
    ]
    .into_iter()
    .flatten()
    .collect();

    let mut state: u8 = 0;
    for k in keys_to_check.iter() {
        if window.is_key_down(*k) {
            if let Some(bit) = key_mapping_to_button(*k, mapping) {
                state |= 1u8 << bit;
            }
        }
    }
    state
}

/// Streaming audio source backed by a channel. When there's no data, it outputs silence to avoid
/// underruns.
struct StreamSource {
    rx: Receiver<i16>,
    sample_rate: u32,
}

impl Iterator for StreamSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let s = self.rx.try_recv().unwrap_or(0);
        Some(s as f32 / 32768.0)
    }
}

impl Source for StreamSource {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> u16 {
        1
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<std::time::Duration> {
        None
    }
}

/// Save current emulation state to a .hemu project file
/// Works for all systems, not just PC
fn save_project(
    sys: &EmulatorSystem,
    runtime_state: &RuntimeState,
    settings: &Settings,
    status_message: &mut String,
) {
    // Show file save dialog
    let default_name = format!("{}_project.hemu", sys.system_name());
    if let Some(path) = rfd::FileDialog::new()
        .add_filter("Hemulator Project", &["hemu"])
        .set_file_name(&default_name)
        .save_file()
    {
        let mut project = HemuProject::new(sys.system_name().to_string());

        // Copy current mount points from runtime state
        // Filter to only include mounts relevant to this system
        // Get system name first to avoid borrowing issue
        let system_name = sys.system_name();
        let relevant_mounts: Vec<&str> = match system_name {
            "pc" => vec!["BIOS", "FloppyA", "FloppyB", "HardDrive"],
            "nes" | "gameboy" | "atari2600" | "snes" | "n64" => vec!["Cartridge"],
            _ => vec![],
        };

        for (mount_id, mount_path) in &runtime_state.current_mounts {
            if relevant_mounts.contains(&mount_id.as_str()) {
                project.set_mount(mount_id.clone(), mount_path.clone());
            }
        }

        // Set display settings from current window state
        project.set_display_settings(
            settings.window_width,
            settings.window_height,
            settings.display_filter,
        );

        // For PC system, also save PC-specific configuration
        if let EmulatorSystem::PC(pc_sys) = sys {
            // Get boot priority from PC system
            let priority = pc_sys.boot_priority();
            let priority_str = match priority {
                emu_pc::BootPriority::FloppyFirst => "FloppyFirst",
                emu_pc::BootPriority::HardDriveFirst => "HardDriveFirst",
                emu_pc::BootPriority::FloppyOnly => "FloppyOnly",
                emu_pc::BootPriority::HardDriveOnly => "HardDriveOnly",
            };
            project.set_boot_priority(priority_str.to_string());

            // Get CPU model from PC system
            let cpu_model = pc_sys.cpu_model();
            let cpu_str = match cpu_model {
                emu_core::cpu_8086::CpuModel::Intel8086 => "Intel8086",
                emu_core::cpu_8086::CpuModel::Intel8088 => "Intel8088",
                emu_core::cpu_8086::CpuModel::Intel80186 => "Intel80186",
                emu_core::cpu_8086::CpuModel::Intel80188 => "Intel80188",
                emu_core::cpu_8086::CpuModel::Intel80286 => "Intel80286",
                emu_core::cpu_8086::CpuModel::Intel80386 => "Intel80386",
                emu_core::cpu_8086::CpuModel::Intel80486 => "Intel80486",
                emu_core::cpu_8086::CpuModel::Intel80486SX => "Intel80486SX",
                emu_core::cpu_8086::CpuModel::Intel80486DX2 => "Intel80486DX2",
                emu_core::cpu_8086::CpuModel::Intel80486SX2 => "Intel80486SX2",
                emu_core::cpu_8086::CpuModel::Intel80486DX4 => "Intel80486DX4",
                emu_core::cpu_8086::CpuModel::IntelPentium => "IntelPentium",
                emu_core::cpu_8086::CpuModel::IntelPentiumMMX => "IntelPentiumMMX",
            };
            project.set_cpu_model(cpu_str.to_string());

            // Get memory size from PC system
            let memory_kb = pc_sys.memory_kb();
            project.set_memory_kb(memory_kb);

            // Get video mode from PC system
            let video_name = pc_sys.video_adapter_name();
            let video_mode = if video_name.contains("VGA") {
                "VGA"
            } else if video_name.contains("EGA") {
                "EGA"
            } else {
                "CGA"
            };
            project.set_video_mode(video_mode.to_string());
        }

        match project.save(&path) {
            Ok(_) => {
                println!("Project saved to: {}", path.display());
                *status_message = format!(
                    "Project saved: {}",
                    path.file_name().unwrap_or_default().to_string_lossy()
                );
            }
            Err(e) => {
                eprintln!("Failed to save project: {}", e);
                *status_message = format!("Failed to save project: {}", e);
            }
        }
    }
}

/// Save a screenshot to the screenshots directory
/// Format: screenshots/<system-name>/YYYYMMDDHHMMSSRRR.png
/// where RRR is a random number between 000 and 999
fn save_screenshot(
    buffer: &[u32],
    width: usize,
    height: usize,
    system_name: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    use chrono::Local;
    use png::Encoder;
    use rand::Rng;

    // Get current local time
    let now = Local::now();

    // Generate random number 000-999
    let random = rand::thread_rng().gen_range(0..1000);

    // Create filename: YYYYMMDDHHMMSSRRR.png
    let filename = format!("{}{:03}.png", now.format("%Y%m%d%H%M%S"), random);

    // Create screenshots directory structure
    let screenshots_dir = PathBuf::from("screenshots").join(system_name);
    fs::create_dir_all(&screenshots_dir)?;

    let filepath = screenshots_dir.join(&filename);

    // Convert RGBA buffer to RGB
    let mut rgb_data = Vec::with_capacity(width * height * 3);
    for pixel in buffer {
        let r = ((pixel >> 16) & 0xFF) as u8;
        let g = ((pixel >> 8) & 0xFF) as u8;
        let b = (pixel & 0xFF) as u8;
        rgb_data.push(r);
        rgb_data.push(g);
        rgb_data.push(b);
    }

    // Write PNG file
    let file = fs::File::create(&filepath)?;
    let mut encoder = Encoder::new(file, width as u32, height as u32);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);

    let mut writer = encoder.write_header()?;
    writer.write_image_data(&rgb_data)?;

    Ok(filepath.to_string_lossy().to_string())
}

/// Create a file dialog with individual filters for each file type plus an "All Files" option
/// This improves the user experience by allowing them to filter by specific file types
fn create_file_dialog(mount_point: &emu_core::MountPointInfo) -> rfd::FileDialog {
    let mut dialog = rfd::FileDialog::new();

    // Add individual filters for each extension
    for ext in &mount_point.extensions {
        // Create a user-friendly name for the filter
        let filter_name = match ext.as_str() {
            "nes" => "NES ROM (*.nes)".to_string(),
            "unf" => "UNIF ROM (*.unf)".to_string(),
            "gb" => "Game Boy ROM (*.gb)".to_string(),
            "gbc" => "Game Boy Color ROM (*.gbc)".to_string(),
            "a26" => "Atari 2600 ROM (*.a26)".to_string(),
            "bin" => "Binary ROM (*.bin)".to_string(),
            "com" => "DOS COM Executable (*.com)".to_string(),
            "exe" => "DOS EXE Executable (*.exe)".to_string(),
            _ => {
                // For unknown extensions, create a generic filter
                format!("{} File (*.{})", ext.to_uppercase(), ext)
            }
        };

        dialog = dialog.add_filter(&filter_name, &[ext.as_str()]);
    }

    // Add "All supported files" filter with all extensions
    let extensions: Vec<&str> = mount_point.extensions.iter().map(|s| s.as_str()).collect();
    dialog = dialog.add_filter("All Supported Files", &extensions);

    // Add "All Files" filter
    dialog = dialog.add_filter("All Files (*.*)", &["*"]);

    dialog
}

/// Command-line arguments for the emulator
#[derive(Debug, Default)]
struct CliArgs {
    rom_path: Option<String>,
    system: Option<String>, // System to start (pc, nes, gb, atari2600, snes, n64)
    slot1: Option<String>,  // BIOS or primary file
    slot2: Option<String>,  // FloppyA
    slot3: Option<String>,  // FloppyB
    slot4: Option<String>,  // HardDrive
    slot5: Option<String>,  // Reserved for future use
    create_blank_disk: Option<(String, String)>, // (path, format)
    show_help: bool,        // Show help message
    show_version: bool,     // Show version
    // Logging configuration
    log_level: Option<String>,      // Global log level
    log_cpu: Option<String>,        // CPU log level
    log_bus: Option<String>,        // Bus log level
    log_ppu: Option<String>,        // PPU log level
    log_apu: Option<String>,        // APU log level
    log_interrupts: Option<String>, // Interrupt log level
    log_stubs: Option<String>,      // Stub/unimplemented log level
    log_file: Option<String>,       // Log file path
}

impl CliArgs {
    /// Parse command-line arguments
    fn parse() -> Self {
        let mut args = CliArgs::default();
        let mut arg_iter = env::args().skip(1);

        while let Some(arg) = arg_iter.next() {
            match arg.as_str() {
                "--help" | "-h" => {
                    args.show_help = true;
                }
                "--version" | "-v" => {
                    args.show_version = true;
                }
                "--system" | "-S" => {
                    if let Some(system) = arg_iter.next() {
                        args.system = Some(system);
                    } else {
                        eprintln!(
                            "Error: --system requires a value (pc, nes, gb, atari2600, snes, n64)."
                        );
                        std::process::exit(1);
                    }
                }
                "--slot1" => {
                    args.slot1 = arg_iter.next();
                }
                "--slot2" => {
                    args.slot2 = arg_iter.next();
                }
                "--slot3" => {
                    args.slot3 = arg_iter.next();
                }
                "--slot4" => {
                    args.slot4 = arg_iter.next();
                }
                "--slot5" => {
                    args.slot5 = arg_iter.next();
                }
                "--create-blank-disk" => {
                    if let Some(path) = arg_iter.next() {
                        if let Some(format) = arg_iter.next() {
                            args.create_blank_disk = Some((path, format));
                        }
                    }
                }
                // Logging configuration
                "--log-level" => {
                    if let Some(level) = arg_iter.next() {
                        args.log_level = Some(level);
                    } else {
                        eprintln!("Error: --log-level requires a value (e.g., 'debug').");
                        std::process::exit(1);
                    }
                }
                "--log-cpu" => {
                    if let Some(level) = arg_iter.next() {
                        args.log_cpu = Some(level);
                    } else {
                        eprintln!("Error: --log-cpu requires a value (e.g., 'debug').");
                        std::process::exit(1);
                    }
                }
                "--log-bus" => {
                    if let Some(level) = arg_iter.next() {
                        args.log_bus = Some(level);
                    } else {
                        eprintln!("Error: --log-bus requires a value (e.g., 'debug').");
                        std::process::exit(1);
                    }
                }
                "--log-ppu" => {
                    if let Some(level) = arg_iter.next() {
                        args.log_ppu = Some(level);
                    } else {
                        eprintln!("Error: --log-ppu requires a value (e.g., 'debug').");
                        std::process::exit(1);
                    }
                }
                "--log-apu" => {
                    if let Some(level) = arg_iter.next() {
                        args.log_apu = Some(level);
                    } else {
                        eprintln!("Error: --log-apu requires a value (e.g., 'debug').");
                        std::process::exit(1);
                    }
                }
                "--log-interrupts" => {
                    if let Some(level) = arg_iter.next() {
                        args.log_interrupts = Some(level);
                    } else {
                        eprintln!("Error: --log-interrupts requires a value (e.g., 'debug').");
                        std::process::exit(1);
                    }
                }
                "--log-stubs" => {
                    if let Some(level) = arg_iter.next() {
                        args.log_stubs = Some(level);
                    } else {
                        eprintln!("Error: --log-stubs requires a value (e.g., 'debug').");
                        std::process::exit(1);
                    }
                }
                "--log-file" => {
                    if let Some(path) = arg_iter.next() {
                        args.log_file = Some(path);
                    } else {
                        eprintln!(
                            "Error: --log-file requires a file path (e.g., 'debug_trace.log')."
                        );
                        std::process::exit(1);
                    }
                }
                _ => {
                    // First non-flag argument is treated as ROM path for backward compatibility
                    if args.rom_path.is_none() && !arg.starts_with("--") {
                        args.rom_path = Some(arg);
                    }
                }
            }
        }

        args
    }

    /// Print usage information
    fn print_usage() {
        eprintln!(
            "Hemulator - Multi-System Emulator v{}",
            env!("CARGO_PKG_VERSION")
        );
        eprintln!();
        eprintln!("Usage: hemu [OPTIONS] [FILE]");
        eprintln!();
        eprintln!("Arguments:");
        eprintln!("  [FILE]                   ROM file or .hemu project file to load");
        eprintln!();
        eprintln!("Options:");
        eprintln!("  -h, --help               Show this help message");
        eprintln!("  -v, --version            Show version information");
        eprintln!(
            "  -S, --system <SYSTEM>    Start clean system (pc, nes, gb, atari2600, snes, n64)"
        );
        eprintln!("  --slot1 <file>           Load file into slot 1 (BIOS for PC)");
        eprintln!("  --slot2 <file>           Load file into slot 2 (Floppy A for PC)");
        eprintln!("  --slot3 <file>           Load file into slot 3 (Floppy B for PC)");
        eprintln!("  --slot4 <file>           Load file into slot 4 (Hard Drive for PC)");
        eprintln!("  --slot5 <file>           Load file into slot 5 (reserved)");
        eprintln!("  --create-blank-disk <path> <format>");
        eprintln!("                           Create a blank disk image");
        eprintln!();
        eprintln!("Logging Options:");
        eprintln!("  --log-level <LEVEL>      Set global log level (off, error, warn, info, debug, trace)");
        eprintln!("  --log-cpu <LEVEL>        Set CPU log level");
        eprintln!("  --log-bus <LEVEL>        Set bus/memory log level");
        eprintln!("  --log-ppu <LEVEL>        Set PPU/graphics log level");
        eprintln!("  --log-apu <LEVEL>        Set APU/audio log level");
        eprintln!("  --log-interrupts <LEVEL> Set interrupt log level");
        eprintln!("  --log-stubs <LEVEL>      Set unimplemented feature log level");
        eprintln!("  --log-file <PATH>        Write logs to file instead of stderr");
        eprintln!();
        eprintln!("Disk formats:");
        eprintln!("  360k, 720k, 1.2m, 1.44m  Floppy disk formats");
        eprintln!("  10m, 20m, 40m            Hard drive formats");
        eprintln!();
        eprintln!("Examples:");
        eprintln!("  hemu game.nes                                  # Load NES ROM");
        eprintln!("  hemu project.hemu                              # Load project file");
        eprintln!("  hemu --system pc                               # Start clean PC system");
        eprintln!("  hemu --system nes                              # Start clean NES system");
        eprintln!("  hemu --log-cpu debug game.nes                  # Load with CPU debug logging");
        eprintln!(
            "  hemu --log-level info game.nes                 # Load with global info logging"
        );
        eprintln!("  hemu --log-cpu trace --log-file trace.log game.nes # Log CPU trace to file");
        eprintln!(
            "  hemu --slot2 disk.img                          # Load PC with floppy in drive A"
        );
        eprintln!(
            "  hemu --slot2 boot.img --slot4 hdd.img         # Load PC with floppy and hard drive"
        );
        eprintln!("  hemu --create-blank-disk floppy.img 1.44m      # Create 1.44MB floppy image");
        eprintln!(
            "  hemu --create-blank-disk hdd.img 20m           # Create 20MB hard drive image"
        );
    }

    /// Print version information
    fn print_version() {
        println!("Hemulator v{}", env!("CARGO_PKG_VERSION"));
        println!("Multi-System Emulator");
        println!("Supported systems: NES, Game Boy, Atari 2600, PC/DOS, SNES, N64");
    }
}

fn main() {
    // Parse command-line arguments
    let cli_args = CliArgs::parse();

    // Handle --help
    if cli_args.show_help {
        CliArgs::print_usage();
        std::process::exit(0);
    }

    // Handle --version
    if cli_args.show_version {
        CliArgs::print_version();
        std::process::exit(0);
    }

    // Handle --create-blank-disk command
    if let Some((path, format_str)) = &cli_args.create_blank_disk {
        match format_str.to_lowercase().as_str() {
            "360k" => {
                let disk = emu_pc::create_blank_floppy(emu_pc::FloppyFormat::Floppy360K);
                if let Err(e) = fs::write(path, disk) {
                    eprintln!("Error creating disk image: {}", e);
                    std::process::exit(1);
                }
                println!("Created 360KB floppy disk: {}", path);
                std::process::exit(0);
            }
            "720k" => {
                let disk = emu_pc::create_blank_floppy(emu_pc::FloppyFormat::Floppy720K);
                if let Err(e) = fs::write(path, disk) {
                    eprintln!("Error creating disk image: {}", e);
                    std::process::exit(1);
                }
                println!("Created 720KB floppy disk: {}", path);
                std::process::exit(0);
            }
            "1.2m" => {
                let disk = emu_pc::create_blank_floppy(emu_pc::FloppyFormat::Floppy1_2M);
                if let Err(e) = fs::write(path, disk) {
                    eprintln!("Error creating disk image: {}", e);
                    std::process::exit(1);
                }
                println!("Created 1.2MB floppy disk: {}", path);
                std::process::exit(0);
            }
            "1.44m" => {
                let disk = emu_pc::create_blank_floppy(emu_pc::FloppyFormat::Floppy1_44M);
                if let Err(e) = fs::write(path, disk) {
                    eprintln!("Error creating disk image: {}", e);
                    std::process::exit(1);
                }
                println!("Created 1.44MB floppy disk: {}", path);
                std::process::exit(0);
            }
            "10m" => {
                let disk = emu_pc::create_blank_hard_drive(emu_pc::HardDriveFormat::HardDrive10M);
                if let Err(e) = fs::write(path, disk) {
                    eprintln!("Error creating disk image: {}", e);
                    std::process::exit(1);
                }
                println!("Created 10MB hard drive image: {}", path);
                std::process::exit(0);
            }
            "20m" => {
                let disk = emu_pc::create_blank_hard_drive(emu_pc::HardDriveFormat::HardDrive20M);
                if let Err(e) = fs::write(path, disk) {
                    eprintln!("Error creating disk image: {}", e);
                    std::process::exit(1);
                }
                println!("Created 20MB hard drive image: {}", path);
                std::process::exit(0);
            }
            "40m" => {
                let disk = emu_pc::create_blank_hard_drive(emu_pc::HardDriveFormat::HardDrive40M);
                if let Err(e) = fs::write(path, disk) {
                    eprintln!("Error creating disk image: {}", e);
                    std::process::exit(1);
                }
                println!("Created 40MB hard drive image: {}", path);
                std::process::exit(0);
            }
            _ => {
                eprintln!("Error: Unknown disk format '{}'", format_str);
                eprintln!();
                CliArgs::print_usage();
                std::process::exit(1);
            }
        }
    }

    // Initialize the new logging system from command-line arguments
    let log_config = emu_core::logging::LogConfig::global();

    // Parse and set log levels from CLI args
    if let Some(ref level_str) = cli_args.log_level {
        if let Some(level) = emu_core::logging::LogLevel::from_str(level_str) {
            log_config.set_global_level(level);
            eprintln!("Global log level: {:?}", level);
        } else {
            eprintln!("Warning: Invalid log level '{}', using 'off'", level_str);
        }
    }

    // Configure category-specific log levels
    for (opt_level_str, category, name) in [
        (
            &cli_args.log_cpu,
            emu_core::logging::LogCategory::CPU,
            "CPU",
        ),
        (
            &cli_args.log_bus,
            emu_core::logging::LogCategory::Bus,
            "Bus",
        ),
        (
            &cli_args.log_ppu,
            emu_core::logging::LogCategory::PPU,
            "PPU",
        ),
        (
            &cli_args.log_apu,
            emu_core::logging::LogCategory::APU,
            "APU",
        ),
        (
            &cli_args.log_interrupts,
            emu_core::logging::LogCategory::Interrupts,
            "Interrupts",
        ),
        (
            &cli_args.log_stubs,
            emu_core::logging::LogCategory::Stubs,
            "Stubs",
        ),
    ] {
        if let Some(ref level_str) = opt_level_str {
            if let Some(level) = emu_core::logging::LogLevel::from_str(level_str) {
                log_config.set_level(category, level);
                eprintln!("{} log level: {:?}", name, level);
            } else {
                eprintln!(
                    "Warning: Invalid {} log level '{}', using 'off'",
                    name, level_str
                );
            }
        }
    }

    // Configure log file if specified
    if let Some(ref log_file_path) = cli_args.log_file {
        use std::path::PathBuf;
        let path = PathBuf::from(log_file_path);
        match log_config.set_log_file(path) {
            Ok(()) => {
                eprintln!("Logging to file: {}", log_file_path);
            }
            Err(e) => {
                eprintln!("Error: Failed to open log file '{}': {}", log_file_path, e);
                std::process::exit(1);
            }
        }
    }

    // Load settings
    let mut settings = Settings::load();

    // Save settings immediately to ensure config.json exists
    // (if it didn't exist, Settings::load() created defaults)
    if let Err(e) = settings.save() {
        eprintln!("Warning: Failed to save config.json: {}", e);
    }

    // Create runtime state for tracking current project and mounts
    let mut runtime_state = RuntimeState::new();

    // Determine what to load based on CLI args
    let rom_path = cli_args.rom_path.or_else(|| settings.last_rom_path.clone());

    // Validate that we have something to load or a system to start
    if cli_args.system.is_none() && rom_path.is_none() {
        eprintln!("Error: Must specify either a system (--system) or a file to load.");
        eprintln!();
        CliArgs::print_usage();
        std::process::exit(1);
    }

    let mut sys: EmulatorSystem;
    let mut rom_hash: Option<String> = None;
    let mut rom_loaded = false;
    let mut status_message = String::new();

    // Initialize system based on --system parameter if specified
    if let Some(ref system_name) = cli_args.system {
        match system_name.to_lowercase().as_str() {
            "nes" => {
                sys = EmulatorSystem::NES(Box::default());
                status_message = "Clean NES system started".to_string();
                println!("Started clean NES system");
            }
            "gb" | "gameboy" => {
                sys = EmulatorSystem::GameBoy(Box::new(emu_gb::GbSystem::new()));
                status_message = "Clean Game Boy system started".to_string();
                println!("Started clean Game Boy system");
            }
            "atari2600" | "atari" => {
                sys = EmulatorSystem::Atari2600(Box::new(emu_atari2600::Atari2600System::new()));
                status_message = "Clean Atari 2600 system started".to_string();
                println!("Started clean Atari 2600 system");
            }
            "pc" => {
                sys = EmulatorSystem::PC(Box::new(emu_pc::PcSystem::new()));
                status_message = "Clean PC system started".to_string();
                println!("Started clean PC system");
            }
            "snes" => {
                sys = EmulatorSystem::SNES(Box::new(emu_snes::SnesSystem::new()));
                status_message = "Clean SNES system started".to_string();
                println!("Started clean SNES system");
            }
            "n64" => {
                sys = EmulatorSystem::N64(Box::new(emu_n64::N64System::new()));
                status_message = "Clean N64 system started".to_string();
                println!("Started clean N64 system");
            }
            _ => {
                eprintln!("Error: Unknown system '{}'", system_name);
                eprintln!("Valid systems: pc, nes, gb, atari2600, snes, n64");
                std::process::exit(1);
            }
        }
    } else {
        // No --system specified, default to NES for now (will be replaced by file loading below)
        sys = EmulatorSystem::NES(Box::default());
    }

    // Try to load ROM/project file if path is available
    // Check if it's a .hemu project file first (before reading as ROM)
    if let Some(p) = &rom_path {
        if p.to_lowercase().ends_with(".hemu") {
            println!("Detected .hemu project file: {}", p);
            match HemuProject::load(p) {
                Ok(project) => {
                    if project.system != "pc" {
                        eprintln!("Currently only PC system .hemu projects are supported");
                        eprintln!("Project is for: {}", project.system);
                    } else {
                        // Parse configuration from project
                        let cpu_model = if let Some(cpu_str) = project.get_cpu_model() {
                            match cpu_str.as_str() {
                                "Intel8086" => emu_core::cpu_8086::CpuModel::Intel8086,
                                "Intel8088" => emu_core::cpu_8086::CpuModel::Intel8088,
                                "Intel80186" => emu_core::cpu_8086::CpuModel::Intel80186,
                                "Intel80188" => emu_core::cpu_8086::CpuModel::Intel80188,
                                "Intel80286" => emu_core::cpu_8086::CpuModel::Intel80286,
                                "Intel80386" => emu_core::cpu_8086::CpuModel::Intel80386,
                                "Intel80486" => emu_core::cpu_8086::CpuModel::Intel80486,
                                "Intel80486SX" => emu_core::cpu_8086::CpuModel::Intel80486SX,
                                "Intel80486DX2" => emu_core::cpu_8086::CpuModel::Intel80486DX2,
                                "Intel80486SX2" => emu_core::cpu_8086::CpuModel::Intel80486SX2,
                                "Intel80486DX4" => emu_core::cpu_8086::CpuModel::Intel80486DX4,
                                "IntelPentium" => emu_core::cpu_8086::CpuModel::IntelPentium,
                                "IntelPentiumMMX" => emu_core::cpu_8086::CpuModel::IntelPentiumMMX,
                                _ => {
                                    eprintln!(
                                        "Unknown CPU model: {}, using default Intel8086",
                                        cpu_str
                                    );
                                    emu_core::cpu_8086::CpuModel::Intel8086
                                }
                            }
                        } else {
                            emu_core::cpu_8086::CpuModel::Intel8086
                        };
                        println!("CPU model: {:?}", cpu_model);

                        let memory_kb = project.get_memory_kb().unwrap_or(640);
                        println!("Memory: {}KB", memory_kb);

                        // Create video adapter based on project configuration
                        let video_adapter: Box<dyn emu_pc::VideoAdapter> =
                            if let Some(video_str) = project.get_video_mode() {
                                match video_str.as_str() {
                                    "EGA" => {
                                        println!("Video mode: EGA");
                                        Box::new(emu_pc::SoftwareEgaAdapter::new())
                                    }
                                    "VGA" => {
                                        println!("Video mode: VGA");
                                        Box::new(emu_pc::SoftwareVgaAdapter::new())
                                    }
                                    "CGA" => {
                                        println!("Video mode: CGA");
                                        Box::new(emu_pc::SoftwareCgaAdapter::new())
                                    }
                                    _ => {
                                        println!("Video mode: CGA (unknown mode, defaulting)");
                                        Box::new(emu_pc::SoftwareCgaAdapter::new())
                                    }
                                }
                            } else {
                                println!("Video mode: CGA (default)");
                                Box::new(emu_pc::SoftwareCgaAdapter::new())
                            };

                        // Create PC system with configuration
                        let mut pc_sys =
                            emu_pc::PcSystem::with_config(cpu_model, memory_kb, video_adapter);

                        // Load boot priority if specified
                        if let Some(priority_str) = project.boot_priority.as_ref() {
                            let priority = match priority_str.as_str() {
                                "FloppyFirst" => emu_pc::BootPriority::FloppyFirst,
                                "HardDriveFirst" => emu_pc::BootPriority::HardDriveFirst,
                                "FloppyOnly" => emu_pc::BootPriority::FloppyOnly,
                                "HardDriveOnly" => emu_pc::BootPriority::HardDriveOnly,
                                _ => emu_pc::BootPriority::FloppyFirst,
                            };
                            pc_sys.set_boot_priority(priority);
                            println!("Set boot priority: {:?}", priority);
                        }

                        // Mount all files from the project
                        let project_dir = std::path::Path::new(p)
                            .parent()
                            .unwrap_or(std::path::Path::new("."));
                        for (mount_id, relative_path) in &project.mounts {
                            let full_path = project_dir.join(relative_path);
                            match fs::read(&full_path) {
                                Ok(data) => {
                                    if let Err(e) = pc_sys.mount(mount_id, &data) {
                                        eprintln!("Failed to mount {}: {}", mount_id, e);
                                    } else {
                                        runtime_state.set_mount(
                                            mount_id.clone(),
                                            full_path.to_string_lossy().to_string(),
                                        );
                                        println!("Mounted {}: {}", mount_id, relative_path);
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Failed to read {}: {}", relative_path, e);
                                }
                            }
                        }

                        // Update POST screen with mount status
                        pc_sys.update_post_screen();

                        sys = EmulatorSystem::PC(Box::new(pc_sys));
                        status_message = "PC virtual machine loaded".to_string();
                        println!("Switched to PC system");
                        // Note: rom_loaded stays false for PC unless we actually load an executable
                        // This allows POST screen to be displayed

                        if let Err(e) = settings.save() {
                            eprintln!("Warning: Failed to save settings: {}", e);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Failed to load .hemu project: {}", e);
                }
            }
        } else {
            // Regular ROM file detection (not a .hemu file)
            match std::fs::read(p) {
                Ok(data) => match detect_rom_type(&data) {
                    Ok(SystemType::NES) => {
                        rom_hash = Some(GameSaves::rom_hash(&data));
                        let mut nes_sys = emu_nes::NesSystem::default();
                        // Use the mount point system to load the cartridge
                        if let Err(e) = nes_sys.mount("Cartridge", &data) {
                            eprintln!("Failed to load NES ROM: {}", e);
                            status_message = format!("Error: {}", e);
                            rom_hash = None;
                        } else {
                            rom_loaded = true;
                            sys = EmulatorSystem::NES(Box::new(nes_sys));
                            runtime_state.set_mount("Cartridge".to_string(), p.clone());
                            settings.last_rom_path = Some(p.clone()); // Keep for backward compat
                            if let Err(e) = settings.save() {
                                eprintln!("Warning: Failed to save settings: {}", e);
                            }
                            status_message = "NES ROM loaded".to_string();
                            println!("Loaded NES ROM: {}", p);
                        }
                    }
                    Ok(SystemType::Atari2600) => {
                        rom_hash = Some(GameSaves::rom_hash(&data));
                        let mut a2600_sys = emu_atari2600::Atari2600System::new();
                        if let Err(e) = a2600_sys.mount("Cartridge", &data) {
                            eprintln!("Failed to load Atari 2600 ROM: {}", e);
                            status_message = format!("Error: {}", e);
                            rom_hash = None;
                        } else {
                            rom_loaded = true;
                            sys = EmulatorSystem::Atari2600(Box::new(a2600_sys));
                            runtime_state.set_mount("Cartridge".to_string(), p.clone());
                            settings.last_rom_path = Some(p.clone());
                            if let Err(e) = settings.save() {
                                eprintln!("Warning: Failed to save settings: {}", e);
                            }
                            status_message = "Atari 2600 ROM loaded".to_string();
                            println!("Loaded Atari 2600 ROM: {}", p);
                        }
                    }
                    Ok(SystemType::GameBoy) => {
                        rom_hash = Some(GameSaves::rom_hash(&data));
                        let mut gb_sys = emu_gb::GbSystem::new();
                        if let Err(e) = gb_sys.mount("Cartridge", &data) {
                            eprintln!("Failed to load Game Boy ROM: {}", e);
                            status_message = format!("Error: {}", e);
                            rom_hash = None;
                        } else {
                            rom_loaded = true;
                            sys = EmulatorSystem::GameBoy(Box::new(gb_sys));
                            runtime_state.set_mount("Cartridge".to_string(), p.clone());
                            settings.last_rom_path = Some(p.clone());
                            if let Err(e) = settings.save() {
                                eprintln!("Warning: Failed to save settings: {}", e);
                            }
                            status_message = "Game Boy ROM loaded".to_string();
                            println!("Loaded Game Boy ROM: {}", p);
                        }
                    }
                    Ok(SystemType::PC) => {
                        // PC executables should be on disk images, not loaded directly
                        // Create a new PC system and let user mount disk images via F3
                        status_message =
                            "PC system detected. Use F3 to mount disk images.".to_string();
                        rom_hash = None; // PC systems don't use ROM hash
                        let pc_sys = emu_pc::PcSystem::new();
                        sys = EmulatorSystem::PC(Box::new(pc_sys));
                        // Don't save mount points for PC since they use disk images
                        eprintln!("PC executable detected. Please mount disk images using F3.");
                        println!("Initialized PC system. Mount disk images to proceed.");
                    }
                    Ok(SystemType::SNES) => {
                        rom_hash = Some(GameSaves::rom_hash(&data));
                        let mut snes_sys = emu_snes::SnesSystem::new();
                        if let Err(e) = snes_sys.mount("Cartridge", &data) {
                            eprintln!("Failed to load SNES ROM: {}", e);
                            status_message = format!("Error: {}", e);
                            rom_hash = None;
                        } else {
                            rom_loaded = true;
                            sys = EmulatorSystem::SNES(Box::new(snes_sys));
                            runtime_state.set_mount("Cartridge".to_string(), p.clone());
                            settings.last_rom_path = Some(p.clone());
                            if let Err(e) = settings.save() {
                                eprintln!("Warning: Failed to save settings: {}", e);
                            }
                            status_message = "SNES ROM loaded".to_string();
                            println!("Loaded SNES ROM: {}", p);
                        }
                    }
                    Ok(SystemType::N64) => {
                        rom_hash = Some(GameSaves::rom_hash(&data));
                        let mut n64_sys = emu_n64::N64System::new();
                        if let Err(e) = n64_sys.mount("Cartridge", &data) {
                            eprintln!("Failed to load N64 ROM: {}", e);
                            status_message = format!("Error: {}", e);
                            rom_hash = None;
                        } else {
                            rom_loaded = true;
                            sys = EmulatorSystem::N64(Box::new(n64_sys));
                            runtime_state.set_mount("Cartridge".to_string(), p.clone());
                            settings.last_rom_path = Some(p.clone());
                            if let Err(e) = settings.save() {
                                eprintln!("Warning: Failed to save settings: {}", e);
                            }
                            status_message = "N64 ROM loaded".to_string();
                            println!("Loaded N64 ROM: {}", p);
                        }
                    }
                    Err(e) => {
                        eprintln!("Unsupported ROM: {}", e);
                        status_message = format!("Unsupported ROM: {}", e);
                    }
                }, // closes inner match detect_rom_type
                Err(e) => {
                    eprintln!("Failed to read ROM file: {}", e);
                }
            }
        } // closes else block for non-.hemu files
    } // closes if let Some(p) = &rom_path

    // Handle slot-based loading (primarily for PC system)
    // If any slot arguments are provided, auto-select PC mode if no ROM was loaded
    let has_slot_args = cli_args.slot1.is_some()
        || cli_args.slot2.is_some()
        || cli_args.slot3.is_some()
        || cli_args.slot4.is_some()
        || cli_args.slot5.is_some();

    if has_slot_args && !rom_loaded {
        // Auto-select PC mode when slot files are provided
        let pc_sys = emu_pc::PcSystem::new();
        sys = EmulatorSystem::PC(Box::new(pc_sys));
        rom_loaded = true;
        println!("Auto-selected PC mode for slot-based loading");
    }

    // Load slot files for PC system
    if let EmulatorSystem::PC(ref mut pc_sys) = sys {
        // Slot 1: BIOS
        if let Some(ref slot1_path) = cli_args.slot1 {
            match fs::read(slot1_path) {
                Ok(data) => {
                    if let Err(e) = pc_sys.mount("BIOS", &data) {
                        eprintln!("Failed to mount BIOS from slot 1: {}", e);
                    } else {
                        runtime_state.set_mount("BIOS".to_string(), slot1_path.clone());
                        println!("Loaded BIOS from slot 1: {}", slot1_path);
                    }
                }
                Err(e) => eprintln!("Failed to read slot 1 file: {}", e),
            }
        }

        // Slot 2: Floppy A
        if let Some(ref slot2_path) = cli_args.slot2 {
            match fs::read(slot2_path) {
                Ok(data) => {
                    if let Err(e) = pc_sys.mount("FloppyA", &data) {
                        eprintln!("Failed to mount Floppy A from slot 2: {}", e);
                    } else {
                        runtime_state.set_mount("FloppyA".to_string(), slot2_path.clone());
                        println!("Loaded Floppy A from slot 2: {}", slot2_path);
                    }
                }
                Err(e) => eprintln!("Failed to read slot 2 file: {}", e),
            }
        }

        // Slot 3: Floppy B
        if let Some(ref slot3_path) = cli_args.slot3 {
            match fs::read(slot3_path) {
                Ok(data) => {
                    if let Err(e) = pc_sys.mount("FloppyB", &data) {
                        eprintln!("Failed to mount Floppy B from slot 3: {}", e);
                    } else {
                        runtime_state.set_mount("FloppyB".to_string(), slot3_path.clone());
                        println!("Loaded Floppy B from slot 3: {}", slot3_path);
                    }
                }
                Err(e) => eprintln!("Failed to read slot 3 file: {}", e),
            }
        }

        // Slot 4: Hard Drive
        if let Some(ref slot4_path) = cli_args.slot4 {
            match fs::read(slot4_path) {
                Ok(data) => {
                    if let Err(e) = pc_sys.mount("HardDrive", &data) {
                        eprintln!("Failed to mount Hard Drive from slot 4: {}", e);
                    } else {
                        runtime_state.set_mount("HardDrive".to_string(), slot4_path.clone());
                        println!("Loaded Hard Drive from slot 4: {}", slot4_path);
                    }
                }
                Err(e) => eprintln!("Failed to read slot 4 file: {}", e),
            }
        }

        // Slot 5: Reserved for future use
        if cli_args.slot5.is_some() {
            eprintln!("Warning: Slot 5 is reserved for future use and will be ignored");
        }

        // Save settings if any slot was loaded
        if has_slot_args {
            if let Err(e) = settings.save() {
                eprintln!("Warning: Failed to save settings: {}", e);
            }
        }
    }

    // Get resolution from the system
    let (mut width, mut height) = sys.resolution();

    // Window size is user-resizable and persisted; buffer size stays at native resolution.
    let window_width = settings.window_width.max(width);
    let window_height = settings.window_height.max(height);

    // Determine whether to use OpenGL or software rendering
    let use_opengl = settings.video_backend == "opengl";

    let mut window: Box<dyn WindowBackend> = match Sdl2Backend::new(
        "Hemulator - Multi-System Emulator",
        window_width,
        window_height,
        use_opengl,
    ) {
        Ok(w) => Box::new(w),
        Err(e) => {
            eprintln!("Failed to create window: {}", e);
            return;
        }
    };

    // Initialize audio output
    let (_stream, stream_handle) = match OutputStream::try_default() {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "Warning: Failed to initialize audio: {}. Audio will be disabled.",
                e
            );
            return;
        }
    };
    let (audio_tx, audio_rx) = sync_channel::<i16>(44100 * 2);
    if let Err(e) = stream_handle.play_raw(
        StreamSource {
            rx: audio_rx,
            sample_rate: 44100,
        }
        .convert_samples(),
    ) {
        eprintln!("Warning: Failed to start audio playback: {}", e);
    }

    // Buffer for rendering
    let mut buffer = if rom_loaded {
        vec![0; width * height]
    } else {
        ui_render::create_splash_screen_with_status(width, height, &status_message)
    };

    // Help overlay state
    let mut show_help = false;

    // Debug overlay state
    let mut show_debug = false;

    // Slot selector state
    let mut show_slot_selector = false;
    let mut slot_selector_mode = "SAVE"; // "SAVE" or "LOAD"

    // Mount point selector state
    let mut show_mount_selector = false;

    // PC keyboard state tracking
    let mut prev_keys: HashSet<Key> = HashSet::new();

    // Speed selector state
    let mut show_speed_selector = false;

    // Timing trackers
    let mut last_frame = Instant::now();

    // FPS tracking (used by the debug overlay)
    let mut frame_times: Vec<Duration> = Vec::with_capacity(60);
    let mut current_fps = 60.0;

    // Audio sample rate
    const SAMPLE_RATE: usize = 44100;

    // Load saves for current ROM if available
    let mut game_saves = if let Some(ref hash) = rom_hash {
        GameSaves::load(hash)
    } else {
        GameSaves::default()
    };

    fn blend_over(base: &[u32], overlay: &[u32]) -> Vec<u32> {
        debug_assert_eq!(base.len(), overlay.len());
        let mut out = Vec::with_capacity(base.len());
        for (b, o) in base.iter().copied().zip(overlay.iter().copied()) {
            let a = (o >> 24) & 0xFF;
            if a == 0 {
                out.push(b);
                continue;
            }
            if a == 255 {
                out.push(0xFF00_0000 | (o & 0x00FF_FFFF));
                continue;
            }

            let inv = 255 - a;
            let br = (b >> 16) & 0xFF;
            let bg = (b >> 8) & 0xFF;
            let bb = b & 0xFF;

            let or = (o >> 16) & 0xFF;
            let og = (o >> 8) & 0xFF;
            let ob = o & 0xFF;

            let r = (or * a + br * inv) / 255;
            let g = (og * a + bg * inv) / 255;
            let b = (ob * a + bb * inv) / 255;

            out.push(0xFF00_0000 | (r << 16) | (g << 8) | b);
        }
        out
    }

    // Main event loop
    // Host key (Right Alt) must be held to use emulator controls (F1-F12, ESC to exit)
    // Without host key, all keys are passed to the emulated system
    while window.is_open() {
        // Poll events at the start of each frame
        window.poll_events();

        // Check if host key (from settings) is held
        let host_modifier_key =
            window_backend::string_to_key(&settings.input.host_modifier).unwrap_or(Key::RightCtrl); // fallback to RightCtrl if invalid
        let host_key_held = window.is_key_down(host_modifier_key);

        // Check if this system requires host key for function keys
        let needs_host_key = sys.requires_host_key_for_function_keys();

        // Only exit on ESC if host key is held (when required by system), else allow ESC always
        if (needs_host_key && host_key_held && window.is_key_down(Key::Escape))
            || (!needs_host_key && window.is_key_down(Key::Escape))
        {
            break;
        }

        // Toggle help overlay (F1)
        if (needs_host_key && host_key_held && window.is_key_pressed(Key::F1, false))
            || (!needs_host_key && window.is_key_pressed(Key::F1, false))
        {
            show_help = !show_help;
            show_slot_selector = false; // Close slot selector if open
            show_mount_selector = false; // Close mount selector if open
            show_speed_selector = false; // Close speed selector if open
            show_debug = false; // Close debug if open
        }

        // Toggle speed selector (F2)
        if (needs_host_key && host_key_held && window.is_key_pressed(Key::F2, false))
            || (!needs_host_key && window.is_key_pressed(Key::F2, false))
        {
            show_speed_selector = !show_speed_selector;
            show_help = false; // Close help if open
            show_slot_selector = false; // Close slot selector if open
            show_mount_selector = false; // Close mount selector if open
            show_debug = false; // Close debug if open
        }

        // Toggle debug overlay (F10)
        let can_debug = rom_loaded || needs_host_key; // Allow debug for PC even without ROM
        if ((needs_host_key && host_key_held && window.is_key_pressed(Key::F10, false))
            || (!needs_host_key && window.is_key_pressed(Key::F10, false)))
            && can_debug
        {
            show_debug = !show_debug;
            show_slot_selector = false; // Close slot selector if open
            show_speed_selector = false; // Close speed selector if open
            show_help = false; // Close help if open

            // Dump debug info to console when opening debug overlay
            if show_debug {
                println!("\n=== Debug Info Dump ===");

                // Try NES debug info first
                if let Some(debug_info) = sys.get_debug_info_nes() {
                    let timing_str = match debug_info.timing_mode {
                        emu_core::apu::TimingMode::Ntsc => "NTSC",
                        emu_core::apu::TimingMode::Pal => "PAL",
                    };
                    println!("System: NES");
                    println!(
                        "Mapper: {} (#{:03})",
                        debug_info.mapper_name, debug_info.mapper_number
                    );
                    println!("Timing: {}", timing_str);
                    println!(
                        "PRG Banks: {} ({}KB total)",
                        debug_info.prg_banks,
                        debug_info.prg_banks * 16
                    );
                    println!(
                        "CHR Banks: {} ({}KB total)",
                        debug_info.chr_banks,
                        if debug_info.chr_banks == 0 {
                            "RAM".to_string()
                        } else {
                            format!("{}", debug_info.chr_banks * 8)
                        }
                    );
                    let stats = sys.get_runtime_stats();
                    println!("Frame: {}", stats.frame_index);
                    println!("PC: 0x{:04X}", stats.pc);
                }
                // Try Atari 2600 debug info
                else if let Some(debug_info) = sys.get_debug_info_atari2600() {
                    println!("System: Atari 2600");
                    println!("ROM Size: {} bytes", debug_info.rom_size);
                    println!("Banking: {}", debug_info.banking_scheme);
                    println!("Current Bank: {}", debug_info.current_bank);
                    println!("Scanline: {}", debug_info.scanline);
                }
                // Try N64 debug info
                else if let Some(debug_info) = sys.get_debug_info_n64() {
                    println!("System: Nintendo 64");
                    println!("ROM: {}", debug_info.rom_name);
                    println!("ROM Size: {:.2} MB", debug_info.rom_size_mb);
                    println!("PC: 0x{:016X}", debug_info.pc);
                    println!("RSP Microcode: {}", debug_info.rsp_microcode);
                    println!("RSP Vertices: {}", debug_info.rsp_vertex_count);
                    println!("RDP Status: 0x{:08X}", debug_info.rdp_status);
                    println!("Framebuffer: {}", debug_info.framebuffer_resolution);
                }
                // Try SNES debug info
                else if let Some(debug_info) = sys.get_debug_info_snes() {
                    println!("System: SNES");
                    println!("ROM Size: {} KB", debug_info.rom_size / 1024);
                    println!(
                        "Header: {}",
                        if debug_info.has_smc_header {
                            "SMC (512 bytes)"
                        } else {
                            "None"
                        }
                    );
                    println!("PC: 0x{:02X}:{:04X}", debug_info.pbr, debug_info.pc);
                    println!(
                        "Mode: {}",
                        if debug_info.emulation_mode {
                            "Emulation (6502)"
                        } else {
                            "Native (65C816)"
                        }
                    );
                }

                println!("FPS: {:.1}", current_fps);
                println!("Video Backend: {}", settings.video_backend);
                println!("======================\n");
            }
        }

        // Cycle CRT filter (F11) - only when host key is held
        if (needs_host_key && host_key_held && window.is_key_pressed(Key::F11, false))
            || (!needs_host_key && window.is_key_pressed(Key::F11, false))
        {
            settings.display_filter = settings.display_filter.next();
            // Update the backend's filter setting
            if let Some(sdl2_backend) = window.as_any_mut().downcast_mut::<Sdl2Backend>() {
                sdl2_backend.set_filter(settings.display_filter);
            }
            if let Err(e) = settings.save() {
                eprintln!("Warning: Failed to save CRT filter setting: {}", e);
            }
            println!("CRT Filter: {}", settings.display_filter.name());
        }

        // Handle speed selector
        if show_speed_selector {
            // Check for speed selection (0-5) or cancel (ESC)
            let mut selected_speed: Option<f64> = None;

            if window.is_key_pressed(Key::Key0, false) {
                selected_speed = Some(0.0); // Pause
            } else if window.is_key_pressed(Key::Key1, false) {
                selected_speed = Some(0.25);
            } else if window.is_key_pressed(Key::Key2, false) {
                selected_speed = Some(0.5);
            } else if window.is_key_pressed(Key::Key3, false) {
                selected_speed = Some(1.0);
            } else if window.is_key_pressed(Key::Key4, false) {
                selected_speed = Some(2.0);
            } else if window.is_key_pressed(Key::Key5, false) {
                selected_speed = Some(10.0);
            }

            if let Some(speed) = selected_speed {
                show_speed_selector = false;
                settings.emulation_speed = speed;
                if let Err(e) = settings.save() {
                    eprintln!("Warning: Failed to save speed setting: {}", e);
                }
                println!("Emulation speed: {}x", speed);
            }
        }

        // Handle slot selector
        if show_slot_selector {
            // Check for slot selection (1-5) or cancel (ESC)
            let mut selected_slot: Option<u8> = None;

            if window.is_key_pressed(Key::Key1, false) {
                selected_slot = Some(1);
            } else if window.is_key_pressed(Key::Key2, false) {
                selected_slot = Some(2);
            } else if window.is_key_pressed(Key::Key3, false) {
                selected_slot = Some(3);
            } else if window.is_key_pressed(Key::Key4, false) {
                selected_slot = Some(4);
            } else if window.is_key_pressed(Key::Key5, false) {
                selected_slot = Some(5);
            }

            if let Some(slot) = selected_slot {
                show_slot_selector = false;

                if let Some(ref hash) = rom_hash {
                    if slot_selector_mode == "SAVE" {
                        // Check if system supports save states
                        if !sys.supports_save_states() {
                            eprintln!("Save states are not supported for this system");
                        } else {
                            // Save state
                            let state = sys.save_state();
                            match serde_json::to_vec(&state) {
                                Ok(data) => match game_saves.save_slot(slot, &data, hash) {
                                    Ok(_) => println!("Saved state to slot {}", slot),
                                    Err(e) => eprintln!("Failed to save to slot {}: {}", slot, e),
                                },
                                Err(e) => eprintln!("Failed to serialize state: {}", e),
                            }
                        }
                    } else {
                        // Load state
                        if !sys.supports_save_states() {
                            eprintln!("Save states are not supported for this system");
                        } else {
                            match game_saves.load_slot(slot, hash) {
                                Ok(data) => {
                                    match serde_json::from_slice::<serde_json::Value>(&data) {
                                        Ok(state) => match sys.load_state(&state) {
                                            Ok(_) => println!("Loaded state from slot {}", slot),
                                            Err(e) => eprintln!("Failed to load state: {}", e),
                                        },
                                        Err(e) => eprintln!("Failed to parse save state: {}", e),
                                    }
                                }
                                Err(e) => eprintln!("Failed to load from slot {}: {}", slot, e),
                            }
                        }
                    }
                }
            }
        }

        // Handle mount point selector
        if show_mount_selector {
            let mount_points = sys.mount_points();

            // Check for mount point selection
            let mut selected_index: Option<usize> = None;

            if window.is_key_pressed(Key::Key1, false) {
                selected_index = Some(0);
            } else if window.is_key_pressed(Key::Key2, false) {
                selected_index = Some(1);
            } else if window.is_key_pressed(Key::Key3, false) {
                selected_index = Some(2);
            } else if window.is_key_pressed(Key::Key4, false) {
                selected_index = Some(3);
            } else if window.is_key_pressed(Key::Key5, false) {
                selected_index = Some(4);
            } else if window.is_key_pressed(Key::Key6, false) {
                selected_index = Some(5);
            } else if window.is_key_pressed(Key::Key7, false) {
                selected_index = Some(6);
            } else if window.is_key_pressed(Key::Key8, false) {
                selected_index = Some(7);
            } else if window.is_key_pressed(Key::Key9, false) {
                selected_index = Some(8);
            }

            if let Some(idx) = selected_index {
                if idx < mount_points.len() {
                    show_mount_selector = false;

                    // Now show file dialog for the selected mount point
                    let mp_info = &mount_points[idx];

                    if let Some(path) = create_file_dialog(mp_info).pick_file() {
                        let path_str = path.to_string_lossy().to_string();
                        match std::fs::read(&path) {
                            Ok(data) => {
                                match sys.mount(&mp_info.id, &data) {
                                    Ok(_) => {
                                        rom_loaded = true;
                                        rom_hash = Some(GameSaves::rom_hash(&data));
                                        runtime_state
                                            .set_mount(mp_info.id.clone(), path_str.clone());
                                        settings.last_rom_path = Some(path_str.clone()); // Keep for backward compat
                                        if let Err(e) = settings.save() {
                                            eprintln!("Warning: Failed to save settings: {}", e);
                                        }
                                        game_saves = if let Some(ref hash) = rom_hash {
                                            GameSaves::load(hash)
                                        } else {
                                            GameSaves::default()
                                        };
                                        println!(
                                            "Loaded media into {}: {}",
                                            mp_info.name, path_str
                                        );
                                        // Update POST screen for PC system
                                        sys.update_post_screen();
                                    }
                                    Err(e) => {
                                        eprintln!(
                                            "Failed to mount media into {}: {}",
                                            mp_info.name, e
                                        );
                                        status_message = format!("Failed to mount: {}", e);
                                        buffer = ui_render::create_splash_screen_with_status(
                                            width,
                                            height,
                                            &status_message,
                                        );
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!("Failed to read file: {}", e);
                            }
                        }
                    }
                }
            }

            // Render mount point selector
            let mount_buffer = ui_render::create_mount_point_selector(width, height, &mount_points);
            if let Err(e) = window.update_with_buffer(&mount_buffer, width, height) {
                eprintln!("Window update error: {}", e);
                break;
            }
            std::thread::sleep(Duration::from_millis(16));
            continue;
        }

        // Prepare help overlay buffer when requested; keep processing input so other
        // keys still work while the overlay is visible.
        let mut help_overlay: Option<Vec<u32>> = None;
        if show_help {
            help_overlay = Some(ui_render::create_help_overlay(width, height, &settings));
        }

        // Prepare debug overlay buffer when requested
        let mut debug_overlay: Option<Vec<u32>> = None;
        if show_debug && rom_loaded {
            // Try NES debug info first
            if let Some(debug_info) = sys.get_debug_info_nes() {
                let timing_str = match debug_info.timing_mode {
                    emu_core::apu::TimingMode::Ntsc => "NTSC",
                    emu_core::apu::TimingMode::Pal => "PAL",
                };
                debug_overlay = Some(ui_render::create_debug_overlay(
                    width,
                    height,
                    &debug_info.mapper_name,
                    debug_info.mapper_number,
                    timing_str,
                    debug_info.prg_banks,
                    debug_info.chr_banks,
                    current_fps,
                    sys.get_runtime_stats(),
                    &settings.video_backend,
                ));
            }
            // Try Atari 2600 debug info
            else if let Some(debug_info) = sys.get_debug_info_atari2600() {
                debug_overlay = Some(ui_render::create_atari2600_debug_overlay(
                    width,
                    height,
                    debug_info.rom_size,
                    &debug_info.banking_scheme,
                    debug_info.current_bank,
                    debug_info.scanline,
                    current_fps,
                    &settings.video_backend,
                ));
            }
            // Try N64 debug info if NES didn't match
            else if let Some(debug_info) = sys.get_debug_info_n64() {
                debug_overlay = Some(ui_render::create_n64_debug_overlay(
                    width,
                    height,
                    &debug_info.rom_name,
                    debug_info.rom_size_mb,
                    debug_info.pc,
                    &debug_info.rsp_microcode,
                    debug_info.rsp_vertex_count,
                    debug_info.rdp_status,
                    &debug_info.framebuffer_resolution,
                    current_fps,
                    &settings.video_backend,
                ));
            }
            // Try SNES debug info
            else if let Some(debug_info) = sys.get_debug_info_snes() {
                debug_overlay = Some(ui_render::create_snes_debug_overlay(
                    width,
                    height,
                    debug_info.rom_size,
                    debug_info.has_smc_header,
                    debug_info.pc,
                    debug_info.pbr,
                    debug_info.emulation_mode,
                    current_fps,
                    &settings.video_backend,
                ));
            }
            // Try PC debug info
            else if let Some(debug_info) = sys.get_debug_info_pc() {
                debug_overlay = Some(ui_render::create_pc_debug_overlay(
                    width,
                    height,
                    debug_info.cs,
                    debug_info.ip,
                    debug_info.ax,
                    debug_info.bx,
                    debug_info.cx,
                    debug_info.dx,
                    debug_info.sp,
                    debug_info.bp,
                    debug_info.si,
                    debug_info.di,
                    debug_info.flags,
                    debug_info.cycles,
                    current_fps,
                    &settings.video_backend,
                ));
            }
        }

        // Check for reset key (F12) - only when host key is held
        // For PC systems, allow reset even without ROM to trigger boot
        let can_reset = rom_loaded || matches!(&sys, EmulatorSystem::PC(_));
        if (needs_host_key && host_key_held && window.is_key_pressed(Key::F12, false))
            || (!needs_host_key && window.is_key_pressed(Key::F12, false)) && can_reset
        {
            sys.reset();
            println!("System reset");
        }

        // F3 - Show mount point selector - always show submenu, no .hemu loading
        if (needs_host_key && host_key_held && window.is_key_pressed(Key::F3, false))
            || (!needs_host_key && window.is_key_pressed(Key::F3, false))
        {
            // Always show mount point selector, even for single-mount systems
            show_mount_selector = true;
            show_help = false;
            show_slot_selector = false;
            show_speed_selector = false;
            show_debug = false;
        }

        // Check for screenshot key (F4) - only when host key is held
        if (needs_host_key && host_key_held && window.is_key_pressed(Key::F4, false))
            || (!needs_host_key && window.is_key_pressed(Key::F4, false))
        {
            match save_screenshot(&buffer, width, height, sys.system_name()) {
                Ok(path) => println!("Screenshot saved to: {}", path),
                Err(e) => eprintln!("Failed to save screenshot: {}", e),
            }
        }

        // F5 - Save state slot selector - only when host key is held
        if host_key_held && rom_loaded && window.is_key_pressed(Key::F5, false) {
            if sys.supports_save_states() {
                show_slot_selector = true;
                slot_selector_mode = "SAVE";
                show_help = false;
            } else {
                eprintln!("Save states are not supported for this system");
            }
        }

        // F6 - Show load state slot selector - only when host key is held
        if host_key_held && rom_loaded && window.is_key_pressed(Key::F6, false) {
            if sys.supports_save_states() {
                show_slot_selector = true;
                slot_selector_mode = "LOAD";
                show_help = false;
            } else {
                eprintln!("Save states are not supported for this system");
            }
        }

        // F7 - Load project file (.hemu) - no backward compatibility for system selector
        if (needs_host_key && host_key_held && window.is_key_pressed(Key::F7, false))
            || (!needs_host_key && window.is_key_pressed(Key::F7, false))
        {
            // Show file open dialog for .hemu files
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Hemulator Project", &["hemu"])
                .add_filter("All Files", &["*"])
                .pick_file()
            {
                let path_str = path.to_string_lossy().to_string();
                match HemuProject::load(&path) {
                    Ok(project) => {
                        // Apply display settings from project
                        settings.window_width = project.display.window_width;
                        settings.window_height = project.display.window_height;
                        settings.display_filter = project.display.display_filter;

                        // Update CRT filter on backend
                        if let Some(sdl2_backend) =
                            window.as_any_mut().downcast_mut::<Sdl2Backend>()
                        {
                            sdl2_backend.set_filter(settings.display_filter);
                        }

                        // Apply input config override if present
                        if let Some(ref project_input) = project.input {
                            settings.input = project_input.clone();
                        }

                        // Handle system-specific loading based on project.system
                        match project.system.as_str() {
                            "pc" => {
                                // Load PC system with configuration from project
                                let cpu_model = if let Some(cpu_str) = project.get_cpu_model() {
                                    match cpu_str.as_str() {
                                        "Intel8086" => emu_core::cpu_8086::CpuModel::Intel8086,
                                        "Intel8088" => emu_core::cpu_8086::CpuModel::Intel8088,
                                        "Intel80186" => emu_core::cpu_8086::CpuModel::Intel80186,
                                        "Intel80188" => emu_core::cpu_8086::CpuModel::Intel80188,
                                        "Intel80286" => emu_core::cpu_8086::CpuModel::Intel80286,
                                        "Intel80386" => emu_core::cpu_8086::CpuModel::Intel80386,
                                        "Intel80486" => emu_core::cpu_8086::CpuModel::Intel80486,
                                        "Intel80486SX" => {
                                            emu_core::cpu_8086::CpuModel::Intel80486SX
                                        }
                                        "Intel80486DX2" => {
                                            emu_core::cpu_8086::CpuModel::Intel80486DX2
                                        }
                                        "Intel80486SX2" => {
                                            emu_core::cpu_8086::CpuModel::Intel80486SX2
                                        }
                                        "Intel80486DX4" => {
                                            emu_core::cpu_8086::CpuModel::Intel80486DX4
                                        }
                                        "IntelPentium" => {
                                            emu_core::cpu_8086::CpuModel::IntelPentium
                                        }
                                        "IntelPentiumMMX" => {
                                            emu_core::cpu_8086::CpuModel::IntelPentiumMMX
                                        }
                                        _ => {
                                            eprintln!(
                                                "Unknown CPU model: {}, using default Intel8086",
                                                cpu_str
                                            );
                                            emu_core::cpu_8086::CpuModel::Intel8086
                                        }
                                    }
                                } else {
                                    emu_core::cpu_8086::CpuModel::Intel8086
                                };

                                let memory_kb = project.get_memory_kb().unwrap_or(640);

                                let video_adapter: Box<dyn emu_pc::VideoAdapter> =
                                    if let Some(video_str) = project.get_video_mode() {
                                        match video_str.as_str() {
                                            "EGA" => Box::new(emu_pc::SoftwareEgaAdapter::new()),
                                            "VGA" => Box::new(emu_pc::SoftwareVgaAdapter::new()),
                                            "CGA" => Box::new(emu_pc::SoftwareCgaAdapter::new()),
                                            _ => {
                                                eprintln!(
                                                    "Unknown video mode: {}, using default CGA",
                                                    video_str
                                                );
                                                Box::new(emu_pc::SoftwareCgaAdapter::new())
                                            }
                                        }
                                    } else {
                                        Box::new(emu_pc::SoftwareCgaAdapter::new())
                                    };

                                let mut pc_sys = emu_pc::PcSystem::with_config(
                                    cpu_model,
                                    memory_kb,
                                    video_adapter,
                                );

                                if let Some(priority_str) = project.get_boot_priority() {
                                    let priority = match priority_str.as_str() {
                                        "FloppyFirst" => emu_pc::BootPriority::FloppyFirst,
                                        "HardDriveFirst" => emu_pc::BootPriority::HardDriveFirst,
                                        "FloppyOnly" => emu_pc::BootPriority::FloppyOnly,
                                        "HardDriveOnly" => emu_pc::BootPriority::HardDriveOnly,
                                        _ => emu_pc::BootPriority::FloppyFirst,
                                    };
                                    pc_sys.set_boot_priority(priority);
                                }

                                // Load all mounts from project
                                runtime_state.clear_mounts();
                                let project_dir =
                                    path.parent().unwrap_or(std::path::Path::new("."));
                                for (mount_id, mount_path) in &project.mounts {
                                    let full_path = project_dir.join(mount_path);
                                    match std::fs::read(&full_path) {
                                        Ok(data) => {
                                            if let Err(e) = pc_sys.mount(mount_id, &data) {
                                                eprintln!("Failed to mount {}: {}", mount_id, e);
                                            } else {
                                                runtime_state.set_mount(
                                                    mount_id.clone(),
                                                    full_path.to_string_lossy().to_string(),
                                                );
                                            }
                                        }
                                        Err(e) => {
                                            eprintln!(
                                                "Failed to read file for {}: {}",
                                                mount_id, e
                                            );
                                        }
                                    }
                                }

                                pc_sys.update_post_screen();
                                sys = EmulatorSystem::PC(Box::new(pc_sys));
                                rom_loaded = true;
                                status_message = "PC project loaded".to_string();
                            }
                            // Handle other systems (NES, GB, etc.)
                            "nes" => {
                                runtime_state.clear_mounts();
                                let project_dir =
                                    path.parent().unwrap_or(std::path::Path::new("."));

                                if let Some(cart_path) = project.mounts.get("Cartridge") {
                                    let full_path = project_dir.join(cart_path);
                                    match std::fs::read(&full_path) {
                                        Ok(data) => {
                                            let mut nes_sys = emu_nes::NesSystem::default();
                                            if let Err(e) = nes_sys.mount("Cartridge", &data) {
                                                status_message =
                                                    format!("Failed to load ROM: {}", e);
                                            } else {
                                                runtime_state.set_mount(
                                                    "Cartridge".to_string(),
                                                    full_path.to_string_lossy().to_string(),
                                                );
                                                rom_hash = Some(GameSaves::rom_hash(&data));
                                                sys = EmulatorSystem::NES(Box::new(nes_sys));
                                                rom_loaded = true;
                                                status_message = "NES project loaded".to_string();
                                            }
                                        }
                                        Err(e) => {
                                            status_message = format!("Failed to read ROM: {}", e);
                                        }
                                    }
                                }
                            }
                            _ => {
                                status_message = format!("Unsupported system: {}", project.system);
                            }
                        }

                        runtime_state.set_project_path(path.clone());

                        // Update resolution
                        let (new_width, new_height) = sys.resolution();
                        width = new_width;
                        height = new_height;
                        buffer = vec![0; width * height];

                        // Save settings (window size, display filter, input if overridden)
                        if let Err(e) = settings.save() {
                            eprintln!("Warning: Failed to save settings: {}", e);
                        }

                        println!("Loaded project from: {}", path_str);
                    }
                    Err(e) => {
                        eprintln!("Failed to load project: {}", e);
                        status_message = format!("Failed to load project: {}", e);
                    }
                }
            }
        }

        // F8 - Save project for any system
        if (needs_host_key && host_key_held && window.is_key_pressed(Key::F8, false))
            || (!needs_host_key && window.is_key_pressed(Key::F8, false))
        {
            // F8 saves project for all systems
            save_project(&sys, &runtime_state, &settings, &mut status_message);
        }

        // Handle controller input / emulation step when ROM is loaded.
        // Debug overlay should NOT pause the game, but selectors should.
        // Speed selector and 0x speed also pause the game.
        // For PC systems, always render frames (to show POST screen even when no disk is loaded)
        let should_step = (rom_loaded || matches!(&sys, EmulatorSystem::PC(_)))
            && !show_help
            && !show_slot_selector
            && !show_mount_selector
            && !show_speed_selector
            && settings.emulation_speed > 0.0;

        if should_step {
            // Handle keyboard input for PC system
            if matches!(&sys, EmulatorSystem::PC(_)) {
                // PC system: Poll all keys and detect press/release edges
                // Only pass keys to the client if host key is NOT held
                let all_keys = [
                    Key::A,
                    Key::B,
                    Key::C,
                    Key::D,
                    Key::E,
                    Key::F,
                    Key::G,
                    Key::H,
                    Key::I,
                    Key::J,
                    Key::K,
                    Key::L,
                    Key::M,
                    Key::N,
                    Key::O,
                    Key::P,
                    Key::Q,
                    Key::R,
                    Key::S,
                    Key::T,
                    Key::U,
                    Key::V,
                    Key::W,
                    Key::X,
                    Key::Y,
                    Key::Z,
                    Key::Key0,
                    Key::Key1,
                    Key::Key2,
                    Key::Key3,
                    Key::Key4,
                    Key::Key5,
                    Key::Key6,
                    Key::Key7,
                    Key::Key8,
                    Key::Key9,
                    Key::Space,
                    Key::Enter,
                    Key::Backspace,
                    Key::Tab,
                    Key::Escape,
                    Key::LeftShift,
                    Key::RightShift,
                    Key::LeftCtrl,
                    Key::RightCtrl,
                    Key::LeftAlt,
                ];

                for &key in &all_keys {
                    let is_down = window.is_key_down(key);
                    let was_down = prev_keys.contains(&key);

                    // Only pass key events to client if host key is NOT held
                    // This allows ESC and function keys to work in the client
                    if !host_key_held {
                        if is_down && !was_down {
                            // Key pressed
                            sys.handle_keyboard(key, true);
                            prev_keys.insert(key);
                        } else if !is_down && was_down {
                            // Key released
                            sys.handle_keyboard(key, false);
                            prev_keys.remove(&key);
                        }
                    } else {
                        // Host key is held - update tracking but don't send to client
                        if is_down {
                            prev_keys.insert(key);
                        } else {
                            prev_keys.remove(&key);
                        }
                    }
                }
            } else {
                // Controller-based systems (NES, GB, Atari, etc.)
                // Get controller state for each player
                let ctrl0 = get_controller_state(window.as_ref(), &settings.input.player1);
                let ctrl1 = get_controller_state(window.as_ref(), &settings.input.player2);
                // Note: Player 3 and 4 would be ctrl2 and ctrl3 for systems that support them

                sys.set_controller(0, ctrl0);
                sys.set_controller(1, ctrl1);
            }

            // Step one frame and display
            match sys.step_frame() {
                Ok(f) => {
                    buffer = f.pixels; // Move instead of clone

                    // Apply CRT filter if not showing overlays
                    if !show_help && !show_slot_selector {
                        settings.display_filter.apply(&mut buffer, width, height);
                    }

                    // Audio generation: generate samples based on actual frame rate
                    // NTSC: ~60.1 FPS (≈734 samples), PAL: ~50.0 FPS (≈882 samples)
                    let timing = sys.timing();
                    let samples_per_frame =
                        (SAMPLE_RATE as f64 / timing.frame_rate_hz()).round() as usize;
                    let audio_samples = sys.get_audio_samples(samples_per_frame);
                    for s in audio_samples {
                        let _ = audio_tx.try_send(s);
                    }
                }
                Err(e) => eprintln!("Frame generation error: {:?}", e),
            }
        }

        let frame_to_present: &[u32] = if show_speed_selector {
            // Render speed selector overlay
            let speed_buffer =
                ui_render::create_speed_selector_overlay(width, height, settings.emulation_speed);
            if let Err(e) = window.update_with_buffer(&speed_buffer, width, height) {
                eprintln!("Window update error: {}", e);
                break;
            }
            &[]
        } else if show_slot_selector {
            // Render slot selector overlay
            let has_saves = [
                game_saves.slots.contains_key(&1),
                game_saves.slots.contains_key(&2),
                game_saves.slots.contains_key(&3),
                game_saves.slots.contains_key(&4),
                game_saves.slots.contains_key(&5),
            ];
            let slot_buffer = ui_render::create_slot_selector_overlay(
                width,
                height,
                slot_selector_mode,
                &has_saves,
            );
            if let Err(e) = window.update_with_buffer(&slot_buffer, width, height) {
                eprintln!("Window update error: {}", e);
                break;
            }
            &[]
        } else if show_mount_selector {
            // Render mount point selector overlay
            let mount_points = sys.mount_points();
            let mount_buffer = ui_render::create_mount_point_selector(width, height, &mount_points);
            if let Err(e) = window.update_with_buffer(&mount_buffer, width, height) {
                eprintln!("Window update error: {}", e);
                break;
            }
            &[]
        } else if let Some(ref overlay) = debug_overlay {
            let composed = blend_over(&buffer, overlay);
            // Keep the composed buffer alive for the duration of update_with_buffer.
            if let Err(e) = window.update_with_buffer(composed.as_slice(), width, height) {
                eprintln!("Window update error: {}", e);
                break;
            }
            // Timing and window-size persistence still run below.
            // Skip the normal update path since we've already presented.
            // NOTE: This keeps debug overlay transparent without pausing emulation.
            //
            // Return a dummy slice; it won't be used.
            &[]
        } else if let Some(ref overlay) = help_overlay {
            overlay.as_slice()
        } else {
            &buffer
        };

        if !frame_to_present.is_empty() {
            if let Err(e) = window.update_with_buffer(frame_to_present, width, height) {
                eprintln!("Window update error: {}", e);
                break;
            }
        }

        // Dynamic frame pacing based on timing mode (NTSC ~60.1 FPS, PAL ~50.0 FPS)
        let frame_dt = last_frame.elapsed();

        frame_times.push(frame_dt);
        if frame_times.len() > 60 {
            frame_times.remove(0);
        }

        if !frame_times.is_empty() {
            let total_time: Duration = frame_times.iter().sum();
            let avg_frame_time = total_time.as_secs_f64() / frame_times.len() as f64;
            if avg_frame_time > 0.0 {
                current_fps = 1.0 / avg_frame_time;
            }
        }

        // Get target frame time from system timing mode, adjusted by emulation speed
        let target_frame_time = if rom_loaded && settings.emulation_speed > 0.0 {
            let timing = sys.timing();
            let frame_rate = timing.frame_rate_hz();
            Duration::from_secs_f64(1.0 / (frame_rate * settings.emulation_speed))
        } else if settings.emulation_speed > 0.0 {
            // Default to NTSC timing when no ROM loaded
            Duration::from_secs_f64(
                1.0 / (emu_core::apu::TimingMode::Ntsc.frame_rate_hz() * settings.emulation_speed),
            )
        } else {
            // When paused (0x speed), use a longer sleep time
            Duration::from_millis(100)
        };

        if frame_dt < target_frame_time {
            std::thread::sleep(target_frame_time - frame_dt);
        }
        last_frame = Instant::now();

        // Save window size if it changed
        let (new_width, new_height) = window.get_size();
        if new_width != settings.window_width || new_height != settings.window_height {
            settings.window_width = new_width;
            settings.window_height = new_height;
            if let Err(e) = settings.save() {
                eprintln!("Warning: Failed to save window size: {}", e);
            }
        }
    }
}
