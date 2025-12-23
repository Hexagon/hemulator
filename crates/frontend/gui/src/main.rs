pub mod crt_filter;
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
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::mpsc::{sync_channel, Receiver};
use std::time::{Duration, Instant};
use window_backend::{string_to_key, Key, Sdl2Backend, WindowBackend};

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

    #[allow(dead_code)]
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
            EmulatorSystem::GameBoy(_) => vec![0; count], // TODO: Implement audio for Game Boy
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
    keep_logs: bool,
    rom_path: Option<String>,
    slot1: Option<String>,                       // BIOS or primary file
    slot2: Option<String>,                       // FloppyA
    slot3: Option<String>,                       // FloppyB
    slot4: Option<String>,                       // HardDrive
    slot5: Option<String>,                       // Reserved for future use
    create_blank_disk: Option<(String, String)>, // (path, format)
}

impl CliArgs {
    /// Parse command-line arguments
    fn parse() -> Self {
        let mut args = CliArgs::default();
        let mut arg_iter = env::args().skip(1);

        while let Some(arg) = arg_iter.next() {
            match arg.as_str() {
                "--keep-logs" => {
                    args.keep_logs = true;
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
        eprintln!("Usage: hemu [OPTIONS] [ROM_FILE]");
        eprintln!();
        eprintln!("Options:");
        eprintln!("  --keep-logs              Preserve debug logging environment variables");
        eprintln!("  --slot1 <file>           Load file into slot 1 (BIOS for PC)");
        eprintln!("  --slot2 <file>           Load file into slot 2 (Floppy A for PC)");
        eprintln!("  --slot3 <file>           Load file into slot 3 (Floppy B for PC)");
        eprintln!("  --slot4 <file>           Load file into slot 4 (Hard Drive for PC)");
        eprintln!("  --slot5 <file>           Load file into slot 5 (reserved)");
        eprintln!("  --create-blank-disk <path> <format>");
        eprintln!("                           Create a blank disk image");
        eprintln!();
        eprintln!("Disk formats:");
        eprintln!("  360k, 720k, 1.2m, 1.44m  Floppy disk formats");
        eprintln!("  10m, 20m, 40m            Hard drive formats");
        eprintln!();
        eprintln!("Examples:");
        eprintln!("  hemu game.nes                                  # Load NES ROM");
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
}

fn main() {
    // Parse command-line arguments
    let cli_args = CliArgs::parse();

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

    // The NES core has some env-var gated debug logging that can produce massive output
    // (and effectively stall the GUI). Disable those by default for the GUI process.
    // Use `--keep-logs` to preserve current env-var behavior.
    if !cli_args.keep_logs {
        env::remove_var("EMU_LOG_PPU_WRITES");
        env::remove_var("EMU_LOG_UNKNOWN_OPS");
    }

    // Load settings
    let mut settings = Settings::load();

    // Save settings immediately to ensure config.json exists
    // (if it didn't exist, Settings::load() created defaults)
    if let Err(e) = settings.save() {
        eprintln!("Warning: Failed to save config.json: {}", e);
    }

    // Initialize system - start with NES by default, no ROM loaded
    let mut sys: EmulatorSystem = EmulatorSystem::NES(Box::default());
    let mut rom_hash: Option<String> = None;
    let mut rom_loaded = false;
    let mut status_message = String::new();

    // Only load ROM if provided via CLI argument (no auto-loading from settings)
    if let Some(ref rom_path) = cli_args.rom_path {
        status_message = "Detecting ROM format...".to_string();
        match std::fs::read(rom_path) {
            Ok(data) => match detect_rom_type(&data) {
                Ok(SystemType::NES) => {
                    status_message = "Detected NES ROM - Initializing...".to_string();
                    rom_hash = Some(GameSaves::rom_hash(&data));
                    let mut nes_sys = emu_nes::NesSystem::default();
                    if let Err(e) = nes_sys.mount("Cartridge", &data) {
                        eprintln!("Failed to load NES ROM: {}", e);
                        status_message = format!("Error: {}", e);
                        rom_hash = None;
                    } else {
                        rom_loaded = true;
                        sys = EmulatorSystem::NES(Box::new(nes_sys));
                        settings.set_mount_point("Cartridge", rom_path.clone());
                        if let Err(e) = settings.save() {
                            eprintln!("Warning: Failed to save settings: {}", e);
                        }
                        status_message = "NES ROM loaded".to_string();
                        println!("Loaded NES ROM: {}", rom_path);
                    }
                }
                Ok(SystemType::Atari2600) => {
                    status_message = "Detected Atari 2600 ROM - Initializing...".to_string();
                    rom_hash = Some(GameSaves::rom_hash(&data));
                    let mut a2600_sys = emu_atari2600::Atari2600System::new();
                    if let Err(e) = a2600_sys.mount("Cartridge", &data) {
                        eprintln!("Failed to load Atari 2600 ROM: {}", e);
                        status_message = format!("Error: {}", e);
                        rom_hash = None;
                    } else {
                        rom_loaded = true;
                        sys = EmulatorSystem::Atari2600(Box::new(a2600_sys));
                        settings.set_mount_point("Cartridge", rom_path.clone());
                        if let Err(e) = settings.save() {
                            eprintln!("Warning: Failed to save settings: {}", e);
                        }
                        status_message = "Atari 2600 ROM loaded".to_string();
                        println!("Loaded Atari 2600 ROM: {}", rom_path);
                    }
                }
                Ok(SystemType::GameBoy) => {
                    status_message = "Detected Game Boy ROM - Initializing...".to_string();
                    rom_hash = Some(GameSaves::rom_hash(&data));
                    let mut gb_sys = emu_gb::GbSystem::new();
                    if let Err(e) = gb_sys.mount("Cartridge", &data) {
                        eprintln!("Failed to load Game Boy ROM: {}", e);
                        status_message = format!("Error: {}", e);
                        rom_hash = None;
                    } else {
                        rom_loaded = true;
                        sys = EmulatorSystem::GameBoy(Box::new(gb_sys));
                        settings.set_mount_point("Cartridge", rom_path.clone());
                        if let Err(e) = settings.save() {
                            eprintln!("Warning: Failed to save settings: {}", e);
                        }
                        status_message = "Game Boy ROM loaded".to_string();
                        println!("Loaded Game Boy ROM: {}", rom_path);
                    }
                }
                Ok(SystemType::PC) => {
                    status_message = "Detected PC executable - Initializing...".to_string();
                    rom_hash = Some(GameSaves::rom_hash(&data));
                    let mut pc_sys = emu_pc::PcSystem::new();
                    if let Err(e) = pc_sys.mount("Executable", &data) {
                        eprintln!("Failed to load PC executable: {}", e);
                        status_message = format!("Error: {}", e);
                        rom_hash = None;
                    } else {
                        rom_loaded = true;
                        sys = EmulatorSystem::PC(Box::new(pc_sys));
                        settings.set_mount_point("Executable", rom_path.clone());
                        if let Err(e) = settings.save() {
                            eprintln!("Warning: Failed to save settings: {}", e);
                        }
                        status_message = "PC executable loaded".to_string();
                        println!("Loaded PC executable: {}", rom_path);
                    }
                }
                Ok(SystemType::SNES) => {
                    status_message = "Detected SNES ROM - Initializing...".to_string();
                    rom_hash = Some(GameSaves::rom_hash(&data));
                    let mut snes_sys = emu_snes::SnesSystem::new();
                    if let Err(e) = snes_sys.mount("Cartridge", &data) {
                        eprintln!("Failed to load SNES ROM: {}", e);
                        status_message = format!("Error: {}", e);
                        rom_hash = None;
                    } else {
                        rom_loaded = true;
                        sys = EmulatorSystem::SNES(Box::new(snes_sys));
                        settings.set_mount_point("Cartridge", rom_path.clone());
                        if let Err(e) = settings.save() {
                            eprintln!("Warning: Failed to save settings: {}", e);
                        }
                        status_message = "SNES ROM loaded".to_string();
                        println!("Loaded SNES ROM: {}", rom_path);
                    }
                }
                Ok(SystemType::N64) => {
                    status_message = "Detected N64 ROM - Initializing...".to_string();
                    rom_hash = Some(GameSaves::rom_hash(&data));
                    let mut n64_sys = emu_n64::N64System::new();
                    if let Err(e) = n64_sys.mount("Cartridge", &data) {
                        eprintln!("Failed to load N64 ROM: {}", e);
                        status_message = format!("Error: {}", e);
                        rom_hash = None;
                    } else {
                        rom_loaded = true;
                        sys = EmulatorSystem::N64(Box::new(n64_sys));
                        settings.set_mount_point("Cartridge", rom_path.clone());
                        if let Err(e) = settings.save() {
                            eprintln!("Warning: Failed to save settings: {}", e);
                        }
                        status_message = "N64 ROM loaded".to_string();
                        println!("Loaded N64 ROM: {}", rom_path);
                    }
                }
                Err(e) => {
                    eprintln!("Unsupported ROM: {}", e);
                    status_message = format!("Unsupported ROM: {}", e);
                }
            },
            Err(e) => {
                eprintln!("Failed to read ROM file: {}", e);
                status_message = format!("Failed to read ROM: {}", e);
            }
        }
    }

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
                        settings.set_mount_point("BIOS", slot1_path.clone());
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
                        settings.set_mount_point("FloppyA", slot2_path.clone());
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
                        settings.set_mount_point("FloppyB", slot3_path.clone());
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
                        settings.set_mount_point("HardDrive", slot4_path.clone());
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
    let (width, height) = sys.resolution();

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

    // Speed selector state
    let mut show_speed_selector = false;

    // System selector state
    let mut show_system_selector = false;

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

    while window.is_open() && !window.is_key_down(Key::Escape) {
        // Poll events at the start of each frame
        window.poll_events();

        // Toggle help overlay (F1)
        if window.is_key_pressed(Key::F1, false) {
            show_help = !show_help;
            show_slot_selector = false; // Close slot selector if open
            show_mount_selector = false; // Close mount selector if open
            show_speed_selector = false; // Close speed selector if open
            show_debug = false; // Close debug if open
        }

        // Toggle speed selector (F2)
        if window.is_key_pressed(Key::F2, false) {
            show_speed_selector = !show_speed_selector;
            show_help = false; // Close help if open
            show_slot_selector = false; // Close slot selector if open
            show_mount_selector = false; // Close mount selector if open
            show_debug = false; // Close debug if open
        }

        // Toggle debug overlay (F10)
        if window.is_key_pressed(Key::F10, false) && rom_loaded {
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

        // Cycle CRT filter (F11)
        if window.is_key_pressed(Key::F11, false) {
            settings.crt_filter = settings.crt_filter.next();
            // Update the backend's filter setting
            if let Some(sdl2_backend) = window.as_any_mut().downcast_mut::<Sdl2Backend>() {
                sdl2_backend.set_filter(settings.crt_filter);
            }
            if let Err(e) = settings.save() {
                eprintln!("Warning: Failed to save CRT filter setting: {}", e);
            }
            println!("CRT Filter: {}", settings.crt_filter.name());
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

        // Handle system selector
        if show_system_selector {
            // Check for system selection (1-6) or cancel (ESC)
            let mut selected_system: Option<u8> = None;

            if window.is_key_pressed(Key::Key1, false) {
                selected_system = Some(1); // NES
            } else if window.is_key_pressed(Key::Key2, false) {
                selected_system = Some(2); // Game Boy
            } else if window.is_key_pressed(Key::Key3, false) {
                selected_system = Some(3); // Atari 2600
            } else if window.is_key_pressed(Key::Key4, false) {
                selected_system = Some(4); // PC
            } else if window.is_key_pressed(Key::Key5, false) {
                selected_system = Some(5); // SNES
            } else if window.is_key_pressed(Key::Key6, false) {
                selected_system = Some(6); // N64
            }

            if let Some(sys_num) = selected_system {
                show_system_selector = false;

                // Clear current ROM state
                rom_loaded = false;
                rom_hash = None;

                // Create new system instance
                match sys_num {
                    1 => {
                        sys = EmulatorSystem::NES(Box::default());
                        status_message = "Switched to NES".to_string();
                        println!("Switched to NES system");
                    }
                    2 => {
                        sys = EmulatorSystem::GameBoy(Box::new(emu_gb::GbSystem::new()));
                        status_message = "Switched to Game Boy".to_string();
                        println!("Switched to Game Boy system");
                    }
                    3 => {
                        sys = EmulatorSystem::Atari2600(Box::new(emu_atari2600::Atari2600System::new()));
                        status_message = "Switched to Atari 2600".to_string();
                        println!("Switched to Atari 2600 system");
                    }
                    4 => {
                        sys = EmulatorSystem::PC(Box::new(emu_pc::PcSystem::new()));
                        status_message = "Switched to PC".to_string();
                        println!("Switched to PC system");
                    }
                    5 => {
                        sys = EmulatorSystem::SNES(Box::new(emu_snes::SnesSystem::new()));
                        status_message = "Switched to SNES".to_string();
                        println!("Switched to SNES system");
                    }
                    6 => {
                        sys = EmulatorSystem::N64(Box::new(emu_n64::N64System::new()));
                        status_message = "Switched to N64".to_string();
                        println!("Switched to N64 system");
                    }
                    _ => {}
                }

                // Update buffer with splash screen
                let (new_width, new_height) = sys.resolution();
                buffer = ui_render::create_splash_screen_with_status(new_width, new_height, &status_message);
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
                                        settings.set_mount_point(&mp_info.id, path_str.clone());
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
                                    }
                                    Err(e) => {
                                        eprintln!(
                                            "Failed to mount media into {}: {}",
                                            mp_info.name, e
                                        );
                                        status_message = format!("Failed to mount: {}", e);
                                        buffer = ui_render::create_splash_screen_with_status(width, height, &status_message);
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
        }

        // Check for reset key (F12)
        if window.is_key_pressed(Key::F12, false) && rom_loaded {
            sys.reset();
            println!("System reset");
        }

        // Check for open ROM/Project dialog (F3)
        if window.is_key_pressed(Key::F3, false) {
            let mount_points = sys.mount_points();

            // For PC system (multi-mount), load .hemu project file
            if matches!(&sys, EmulatorSystem::PC(_)) {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Hemulator Project", &["hemu"])
                    .add_filter("All Files", &["*"])
                    .pick_file()
                {
                    let path_str = path.to_string_lossy().to_string();
                    match HemuProject::load(&path) {
                        Ok(project) => {
                            if project.system != "pc" {
                                eprintln!("Project is for {} system, but PC system is active", project.system);
                                status_message = format!("Wrong system: project is for {}", project.system);
                            } else {
                                // Load all mounts from project
                                let mut any_mounted = false;
                                for (mount_id, mount_path) in &project.mounts {
                                    match std::fs::read(mount_path) {
                                        Ok(data) => {
                                            if let Err(e) = sys.mount(mount_id, &data) {
                                                eprintln!("Failed to mount {}: {}", mount_id, e);
                                            } else {
                                                settings.set_mount_point(mount_id, mount_path.clone());
                                                any_mounted = true;
                                                println!("Mounted {}: {}", mount_id, mount_path);
                                            }
                                        }
                                        Err(e) => {
                                            eprintln!("Failed to read file for {}: {}", mount_id, e);
                                        }
                                    }
                                }

                                if any_mounted {
                                    rom_loaded = true;
                                    status_message = "Project loaded".to_string();
                                    println!("Loaded project from: {}", path_str);
                                    if let Err(e) = settings.save() {
                                        eprintln!("Warning: Failed to save settings: {}", e);
                                    }
                                } else {
                                    status_message = "Failed to mount project files".to_string();
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to load project: {}", e);
                            status_message = format!("Failed to load project: {}", e);
                        }
                    }
                }
            } else if mount_points.len() == 1 {
                // Single-mount systems: load ROM directly
                let mp_info = &mount_points[0];

                if let Some(path) = create_file_dialog(mp_info).pick_file() {
                    let path_str = path.to_string_lossy().to_string();
                    match std::fs::read(&path) {
                        Ok(data) => {
                            match sys.mount(&mp_info.id, &data) {
                                Ok(_) => {
                                    rom_loaded = true;
                                    rom_hash = Some(GameSaves::rom_hash(&data));
                                    settings.set_mount_point(&mp_info.id, path_str.clone());
                                    if let Err(e) = settings.save() {
                                        eprintln!("Warning: Failed to save settings: {}", e);
                                    }
                                    game_saves = if let Some(ref hash) = rom_hash {
                                        GameSaves::load(hash)
                                    } else {
                                        GameSaves::default()
                                    };
                                    status_message = format!("{} loaded", mp_info.name);
                                    println!("Loaded media into {}: {}", mp_info.name, path_str);
                                }
                                Err(e) => {
                                    eprintln!("Failed to mount media into {}: {}", mp_info.name, e);
                                    rom_hash = None;
                                    rom_loaded = false;
                                    status_message = format!("Failed to mount: {}", e);
                                    buffer = ui_render::create_splash_screen_with_status(width, height, &status_message);
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to read file: {}", e);
                            status_message = format!("Failed to read file: {}", e);
                        }
                    }
                }
            } else if mount_points.len() > 1 {
                // Show mount point selector for systems with multiple mount points (shouldn't happen now)
                show_mount_selector = true;
                show_help = false;
            }
        }

        // Check for screenshot key (F4)
        if window.is_key_pressed(Key::F4, false) {
            match save_screenshot(&buffer, width, height, sys.system_name()) {
                Ok(path) => println!("Screenshot saved to: {}", path),
                Err(e) => eprintln!("Failed to save screenshot: {}", e),
            }
        }

        // F5 - Show save state slot selector
        if rom_loaded && window.is_key_pressed(Key::F5, false) {
            if sys.supports_save_states() {
                show_slot_selector = true;
                slot_selector_mode = "SAVE";
                show_help = false;
            } else {
                eprintln!("Save states are not supported for this system");
            }
        }

        // F6 - Show load state slot selector
        if rom_loaded && window.is_key_pressed(Key::F6, false) {
            if sys.supports_save_states() {
                show_slot_selector = true;
                slot_selector_mode = "LOAD";
                show_help = false;
            } else {
                eprintln!("Save states are not supported for this system");
            }
        }

        // F7 - Show system selector
        if window.is_key_pressed(Key::F7, false) {
            show_system_selector = true;
            show_help = false;
            show_slot_selector = false;
            show_mount_selector = false;
            show_speed_selector = false;
            show_debug = false;
        }

        // F8 - Create new project file (for PC system)
        if window.is_key_pressed(Key::F8, false) {
            // Only allow project creation for multi-mount systems (PC)
            if matches!(&sys, EmulatorSystem::PC(_)) {
                // Show file save dialog
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Hemulator Project", &["hemu"])
                    .set_file_name("project.hemu")
                    .save_file()
                {
                    let mut project = HemuProject::new("pc".to_string());
                    
                    // Get current mount points from settings
                    for (mount_id, mount_path) in &settings.mount_points {
                        project.set_mount(mount_id.clone(), mount_path.clone());
                    }
                    
                    match project.save(&path) {
                        Ok(_) => {
                            println!("Project saved to: {}", path.display());
                            status_message = format!("Project saved: {}", path.file_name().unwrap_or_default().to_string_lossy());
                        }
                        Err(e) => {
                            eprintln!("Failed to save project: {}", e);
                            status_message = format!("Failed to save project: {}", e);
                        }
                    }
                }
            } else {
                println!("Project files are only supported for multi-mount systems (PC)");
                status_message = "Project files only for PC system".to_string();
            }
        }

        // Handle controller input / emulation step when ROM is loaded.
        // Debug overlay should NOT pause the game, but selectors should.
        // Speed selector and 0x speed also pause the game.
        if rom_loaded
            && !show_help
            && !show_slot_selector
            && !show_mount_selector
            && !show_speed_selector
            && !show_system_selector
            && settings.emulation_speed > 0.0
        {
            // Handle keyboard input for PC system
            // PC keyboard events are handled via the handle_keyboard method in EmulatorSystem
            // which is called from the event loop based on key press/release events.
            // For other systems, we poll controller state.
            if !matches!(&sys, EmulatorSystem::PC(_)) {
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
                        settings.crt_filter.apply(&mut buffer, width, height);
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
        } else if show_system_selector {
            // Render system selector overlay
            let system_buffer = ui_render::create_system_selector_overlay(width, height);
            if let Err(e) = window.update_with_buffer(&system_buffer, width, height) {
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
