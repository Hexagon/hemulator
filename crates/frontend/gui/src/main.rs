mod crt_filter;
mod rom_detect;
mod save_state;
mod settings;
mod ui_render;

use emu_core::{types::Frame, System};
use minifb::{Key, Window, WindowOptions};
use rodio::{OutputStream, Source};
use rom_detect::{detect_rom_type, SystemType};
use save_state::GameSaves;
use settings::Settings;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::mpsc::{sync_channel, Receiver};
use std::time::{Duration, Instant};

// System wrapper enum to support multiple emulated systems
enum EmulatorSystem {
    NES(emu_nes::NesSystem),
    GameBoy(emu_gb::GbSystem),
    Atari2600(emu_atari2600::Atari2600System),
    PC(emu_pc::PcSystem),
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
        }
    }

    fn reset(&mut self) {
        match self {
            EmulatorSystem::NES(sys) => sys.reset(),
            EmulatorSystem::GameBoy(sys) => sys.reset(),
            EmulatorSystem::Atari2600(sys) => sys.reset(),
            EmulatorSystem::PC(sys) => sys.reset(),
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
        }
    }

    fn mount_points(&self) -> Vec<emu_core::MountPointInfo> {
        match self {
            EmulatorSystem::NES(sys) => sys.mount_points(),
            EmulatorSystem::GameBoy(sys) => sys.mount_points(),
            EmulatorSystem::Atari2600(sys) => sys.mount_points(),
            EmulatorSystem::PC(sys) => sys.mount_points(),
        }
    }

    fn supports_save_states(&self) -> bool {
        match self {
            EmulatorSystem::NES(sys) => sys.supports_save_states(),
            EmulatorSystem::GameBoy(sys) => sys.supports_save_states(),
            EmulatorSystem::Atari2600(sys) => sys.supports_save_states(),
            EmulatorSystem::PC(sys) => sys.supports_save_states(),
        }
    }

    fn save_state(&self) -> serde_json::Value {
        match self {
            EmulatorSystem::NES(sys) => sys.save_state(),
            EmulatorSystem::GameBoy(sys) => sys.save_state(),
            EmulatorSystem::Atari2600(sys) => sys.save_state(),
            EmulatorSystem::PC(sys) => sys.save_state(),
        }
    }

    fn load_state(&mut self, state: &serde_json::Value) -> Result<(), serde_json::Error> {
        match self {
            EmulatorSystem::NES(sys) => sys.load_state(state),
            EmulatorSystem::GameBoy(sys) => sys.load_state(state),
            EmulatorSystem::Atari2600(sys) => sys.load_state(state),
            EmulatorSystem::PC(sys) => sys.load_state(state),
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
        }
    }

    fn handle_keyboard(&mut self, key: minifb::Key, pressed: bool) {
        match self {
            EmulatorSystem::PC(sys) => {
                // Map minifb keys to PC scancodes
                let scancode = match key {
                    minifb::Key::A => Some(emu_pc::SCANCODE_A),
                    minifb::Key::B => Some(emu_pc::SCANCODE_B),
                    minifb::Key::C => Some(emu_pc::SCANCODE_C),
                    minifb::Key::D => Some(emu_pc::SCANCODE_D),
                    minifb::Key::E => Some(emu_pc::SCANCODE_E),
                    minifb::Key::F => Some(emu_pc::SCANCODE_F),
                    minifb::Key::G => Some(emu_pc::SCANCODE_G),
                    minifb::Key::H => Some(emu_pc::SCANCODE_H),
                    minifb::Key::I => Some(emu_pc::SCANCODE_I),
                    minifb::Key::J => Some(emu_pc::SCANCODE_J),
                    minifb::Key::K => Some(emu_pc::SCANCODE_K),
                    minifb::Key::L => Some(emu_pc::SCANCODE_L),
                    minifb::Key::M => Some(emu_pc::SCANCODE_M),
                    minifb::Key::N => Some(emu_pc::SCANCODE_N),
                    minifb::Key::O => Some(emu_pc::SCANCODE_O),
                    minifb::Key::P => Some(emu_pc::SCANCODE_P),
                    minifb::Key::Q => Some(emu_pc::SCANCODE_Q),
                    minifb::Key::R => Some(emu_pc::SCANCODE_R),
                    minifb::Key::S => Some(emu_pc::SCANCODE_S),
                    minifb::Key::T => Some(emu_pc::SCANCODE_T),
                    minifb::Key::U => Some(emu_pc::SCANCODE_U),
                    minifb::Key::V => Some(emu_pc::SCANCODE_V),
                    minifb::Key::W => Some(emu_pc::SCANCODE_W),
                    minifb::Key::X => Some(emu_pc::SCANCODE_X),
                    minifb::Key::Y => Some(emu_pc::SCANCODE_Y),
                    minifb::Key::Z => Some(emu_pc::SCANCODE_Z),
                    minifb::Key::Key0 => Some(emu_pc::SCANCODE_0),
                    minifb::Key::Key1 => Some(emu_pc::SCANCODE_1),
                    minifb::Key::Key2 => Some(emu_pc::SCANCODE_2),
                    minifb::Key::Key3 => Some(emu_pc::SCANCODE_3),
                    minifb::Key::Key4 => Some(emu_pc::SCANCODE_4),
                    minifb::Key::Key5 => Some(emu_pc::SCANCODE_5),
                    minifb::Key::Key6 => Some(emu_pc::SCANCODE_6),
                    minifb::Key::Key7 => Some(emu_pc::SCANCODE_7),
                    minifb::Key::Key8 => Some(emu_pc::SCANCODE_8),
                    minifb::Key::Key9 => Some(emu_pc::SCANCODE_9),
                    minifb::Key::Space => Some(emu_pc::SCANCODE_SPACE),
                    minifb::Key::Enter => Some(emu_pc::SCANCODE_ENTER),
                    minifb::Key::Backspace => Some(emu_pc::SCANCODE_BACKSPACE),
                    minifb::Key::Tab => Some(emu_pc::SCANCODE_TAB),
                    minifb::Key::LeftShift | minifb::Key::RightShift => {
                        Some(emu_pc::SCANCODE_LEFT_SHIFT)
                    }
                    minifb::Key::LeftCtrl | minifb::Key::RightCtrl => {
                        Some(emu_pc::SCANCODE_LEFT_CTRL)
                    }
                    minifb::Key::LeftAlt | minifb::Key::RightAlt => Some(emu_pc::SCANCODE_LEFT_ALT),
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
            _ => {} // Other systems don't use keyboard input
        }
    }

    fn get_debug_info(&self) -> Option<emu_nes::DebugInfo> {
        match self {
            EmulatorSystem::NES(sys) => Some(sys.get_debug_info()),
            EmulatorSystem::GameBoy(_) => None,
            EmulatorSystem::Atari2600(_) => None,
            EmulatorSystem::PC(_) => None,
        }
    }

    fn get_runtime_stats(&self) -> emu_nes::RuntimeStats {
        match self {
            EmulatorSystem::NES(sys) => sys.get_runtime_stats(),
            EmulatorSystem::GameBoy(_) => emu_nes::RuntimeStats::default(),
            EmulatorSystem::Atari2600(_) => emu_nes::RuntimeStats::default(),
            EmulatorSystem::PC(_) => emu_nes::RuntimeStats::default(),
        }
    }

    fn timing(&self) -> emu_core::apu::TimingMode {
        match self {
            EmulatorSystem::NES(sys) => sys.timing(),
            EmulatorSystem::GameBoy(_) => emu_core::apu::TimingMode::Ntsc,
            EmulatorSystem::Atari2600(_) => emu_core::apu::TimingMode::Ntsc,
            EmulatorSystem::PC(_) => emu_core::apu::TimingMode::Ntsc,
        }
    }

    fn get_audio_samples(&mut self, count: usize) -> Vec<i16> {
        match self {
            EmulatorSystem::NES(sys) => sys.get_audio_samples(count),
            EmulatorSystem::GameBoy(_) => vec![0; count], // TODO: Implement audio for Game Boy
            EmulatorSystem::Atari2600(_) => vec![0; count], // TODO: Implement audio for Atari 2600
            EmulatorSystem::PC(_) => vec![0; count],      // TODO: Implement audio for PC
        }
    }

    fn resolution(&self) -> (usize, usize) {
        match self {
            EmulatorSystem::NES(_) => (256, 240),
            EmulatorSystem::GameBoy(_) => (160, 144),
            EmulatorSystem::Atari2600(_) => (160, 192),
            EmulatorSystem::PC(_) => (640, 400),
        }
    }

    fn system_name(&self) -> &str {
        match self {
            EmulatorSystem::NES(_) => "nes",
            EmulatorSystem::GameBoy(_) => "gameboy",
            EmulatorSystem::Atari2600(_) => "atari2600",
            EmulatorSystem::PC(_) => "pc",
        }
    }
}

fn string_to_key(s: &str) -> Option<Key> {
    match s {
        "Z" => Some(Key::Z),
        "X" => Some(Key::X),
        "A" => Some(Key::A),
        "B" => Some(Key::B),
        "C" => Some(Key::C),
        "D" => Some(Key::D),
        "E" => Some(Key::E),
        "F" => Some(Key::F),
        "G" => Some(Key::G),
        "H" => Some(Key::H),
        "I" => Some(Key::I),
        "J" => Some(Key::J),
        "K" => Some(Key::K),
        "L" => Some(Key::L),
        "M" => Some(Key::M),
        "N" => Some(Key::N),
        "O" => Some(Key::O),
        "P" => Some(Key::P),
        "Q" => Some(Key::Q),
        "R" => Some(Key::R),
        "S" => Some(Key::S),
        "T" => Some(Key::T),
        "U" => Some(Key::U),
        "V" => Some(Key::V),
        "W" => Some(Key::W),
        "Y" => Some(Key::Y),
        "LeftShift" => Some(Key::LeftShift),
        "RightShift" => Some(Key::RightShift),
        "Enter" => Some(Key::Enter),
        "Space" => Some(Key::Space),
        "Up" => Some(Key::Up),
        "Down" => Some(Key::Down),
        "Left" => Some(Key::Left),
        "Right" => Some(Key::Right),
        _ => None,
    }
}

fn key_mapping_to_button(key: Key, settings: &Settings) -> Option<u8> {
    // Map key to button based on settings
    if Some(key) == string_to_key(&settings.keyboard.a) {
        Some(0)
    } else if Some(key) == string_to_key(&settings.keyboard.b) {
        Some(1)
    } else if Some(key) == string_to_key(&settings.keyboard.select) {
        Some(2)
    } else if Some(key) == string_to_key(&settings.keyboard.start) {
        Some(3)
    } else if Some(key) == string_to_key(&settings.keyboard.up) {
        Some(4)
    } else if Some(key) == string_to_key(&settings.keyboard.down) {
        Some(5)
    } else if Some(key) == string_to_key(&settings.keyboard.left) {
        Some(6)
    } else if Some(key) == string_to_key(&settings.keyboard.right) {
        Some(7)
    } else {
        None
    }
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
    let filename = format!(
        "{}{:03}.png",
        now.format("%Y%m%d%H%M%S"),
        random
    );
    
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

fn main() {
    // The NES core has some env-var gated debug logging that can produce massive output
    // (and effectively stall the GUI). Disable those by default for the GUI process.
    // Use `--keep-logs` to preserve current env-var behavior.
    let mut args = env::args().skip(1);
    let mut keep_logs = false;
    let mut rom_path: Option<String> = None;
    for a in args.by_ref() {
        if a == "--keep-logs" {
            keep_logs = true;
            continue;
        }
        if rom_path.is_none() {
            rom_path = Some(a);
        }
    }

    if !keep_logs {
        env::remove_var("EMU_LOG_PPU_WRITES");
        env::remove_var("EMU_LOG_UNKNOWN_OPS");
    }

    // Load settings
    let mut settings = Settings::load();

    // If no ROM path provided via args, try to load from settings (backward compatibility)
    if rom_path.is_none() {
        // Try new mount_points system first
        if let Some(cartridge_path) = settings.get_mount_point("Cartridge") {
            rom_path = Some(cartridge_path.clone());
        } else if let Some(ref path) = settings.last_rom_path {
            // Fall back to old last_rom_path for backward compatibility
            rom_path = Some(path.clone());
        }
    }

    let mut sys: EmulatorSystem = EmulatorSystem::NES(emu_nes::NesSystem::default());
    let mut rom_hash: Option<String> = None;
    let mut rom_loaded = false;

    // Try to load ROM if path is available
    // Note: We still use ROM type detection at startup to determine which system to instantiate.
    // The mount point system works within a system for managing media slots.
    if let Some(p) = &rom_path {
        match std::fs::read(p) {
            Ok(data) => match detect_rom_type(&data) {
                Ok(SystemType::NES) => {
                    rom_hash = Some(GameSaves::rom_hash(&data));
                    let mut nes_sys = emu_nes::NesSystem::default();
                    // Use the mount point system to load the cartridge
                    if let Err(e) = nes_sys.mount("Cartridge", &data) {
                        eprintln!("Failed to load NES ROM: {}", e);
                        rom_hash = None;
                    } else {
                        rom_loaded = true;
                        sys = EmulatorSystem::NES(nes_sys);
                        settings.set_mount_point("Cartridge", p.clone());
                        settings.last_rom_path = Some(p.clone()); // Keep for backward compat
                        if let Err(e) = settings.save() {
                            eprintln!("Warning: Failed to save settings: {}", e);
                        }
                        println!("Loaded NES ROM: {}", p);
                    }
                }
                Ok(SystemType::Atari2600) => {
                    rom_hash = Some(GameSaves::rom_hash(&data));
                    let mut a2600_sys = emu_atari2600::Atari2600System::new();
                    if let Err(e) = a2600_sys.mount("Cartridge", &data) {
                        eprintln!("Failed to load Atari 2600 ROM: {}", e);
                        rom_hash = None;
                    } else {
                        rom_loaded = true;
                        sys = EmulatorSystem::Atari2600(a2600_sys);
                        settings.set_mount_point("Cartridge", p.clone());
                        settings.last_rom_path = Some(p.clone());
                        if let Err(e) = settings.save() {
                            eprintln!("Warning: Failed to save settings: {}", e);
                        }
                        println!("Loaded Atari 2600 ROM: {}", p);
                    }
                }
                Ok(SystemType::GameBoy) => {
                    rom_hash = Some(GameSaves::rom_hash(&data));
                    let mut gb_sys = emu_gb::GbSystem::new();
                    if let Err(e) = gb_sys.mount("Cartridge", &data) {
                        eprintln!("Failed to load Game Boy ROM: {}", e);
                        rom_hash = None;
                    } else {
                        rom_loaded = true;
                        sys = EmulatorSystem::GameBoy(gb_sys);
                        settings.set_mount_point("Cartridge", p.clone());
                        settings.last_rom_path = Some(p.clone());
                        if let Err(e) = settings.save() {
                            eprintln!("Warning: Failed to save settings: {}", e);
                        }
                        println!("Loaded Game Boy ROM: {}", p);
                    }
                }
                Ok(SystemType::PC) => {
                    rom_hash = Some(GameSaves::rom_hash(&data));
                    let mut pc_sys = emu_pc::PcSystem::new();
                    if let Err(e) = pc_sys.mount("Executable", &data) {
                        eprintln!("Failed to load PC executable: {}", e);
                        rom_hash = None;
                    } else {
                        rom_loaded = true;
                        sys = EmulatorSystem::PC(pc_sys);
                        settings.set_mount_point("Executable", p.clone());
                        settings.last_rom_path = Some(p.clone());
                        if let Err(e) = settings.save() {
                            eprintln!("Warning: Failed to save settings: {}", e);
                        }
                        println!("Loaded PC executable: {}", p);
                    }
                }
                Err(e) => {
                    eprintln!("Unsupported ROM: {}", e);
                }
            },
            Err(e) => {
                eprintln!("Failed to read ROM file: {}", e);
            }
        }
    }

    // Get resolution from the system
    let (width, height) = sys.resolution();

    // Window size is user-resizable and persisted; buffer size stays at native resolution.
    let window_width = settings.window_width.max(width);
    let window_height = settings.window_height.max(height);

    let mut window = match Window::new(
        "Hemulator - Multi-System Emulator",
        window_width,
        window_height,
        WindowOptions {
            resize: true,
            ..WindowOptions::default()
        },
    ) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("Failed to create window: {}", e);
            return;
        }
    };

    // Note: minifb 0.25 doesn't provide a way to programmatically set window size
    // The window uses default sizing, but users can resize it freely via OS controls
    // The size is tracked and saved in settings (could be used with future minifb versions)

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
        ui_render::create_default_screen(width, height)
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
        // Toggle help overlay (F1)
        if window.is_key_pressed(Key::F1, minifb::KeyRepeat::No) {
            show_help = !show_help;
            show_slot_selector = false; // Close slot selector if open
            show_mount_selector = false; // Close mount selector if open
            show_speed_selector = false; // Close speed selector if open
            show_debug = false; // Close debug if open
        }

        // Toggle speed selector (F2)
        if window.is_key_pressed(Key::F2, minifb::KeyRepeat::No) {
            show_speed_selector = !show_speed_selector;
            show_help = false; // Close help if open
            show_slot_selector = false; // Close slot selector if open
            show_mount_selector = false; // Close mount selector if open
            show_debug = false; // Close debug if open
        }

        // Toggle debug overlay (F10)
        if window.is_key_pressed(Key::F10, minifb::KeyRepeat::No) && rom_loaded {
            show_debug = !show_debug;
            show_slot_selector = false; // Close slot selector if open
            show_speed_selector = false; // Close speed selector if open
            show_help = false; // Close help if open
        }

        // Cycle CRT filter (F11)
        if window.is_key_pressed(Key::F11, minifb::KeyRepeat::No) {
            settings.crt_filter = settings.crt_filter.next();
            if let Err(e) = settings.save() {
                eprintln!("Warning: Failed to save CRT filter setting: {}", e);
            }
            println!("CRT Filter: {}", settings.crt_filter.name());
        }

        // Handle speed selector
        if show_speed_selector {
            // Check for speed selection (0-5) or cancel (ESC)
            let mut selected_speed: Option<f64> = None;

            if window.is_key_pressed(Key::Key0, minifb::KeyRepeat::No) {
                selected_speed = Some(0.0); // Pause
            } else if window.is_key_pressed(Key::Key1, minifb::KeyRepeat::No) {
                selected_speed = Some(0.25);
            } else if window.is_key_pressed(Key::Key2, minifb::KeyRepeat::No) {
                selected_speed = Some(0.5);
            } else if window.is_key_pressed(Key::Key3, minifb::KeyRepeat::No) {
                selected_speed = Some(1.0);
            } else if window.is_key_pressed(Key::Key4, minifb::KeyRepeat::No) {
                selected_speed = Some(2.0);
            } else if window.is_key_pressed(Key::Key5, minifb::KeyRepeat::No) {
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

            if window.is_key_pressed(Key::Key1, minifb::KeyRepeat::No) {
                selected_slot = Some(1);
            } else if window.is_key_pressed(Key::Key2, minifb::KeyRepeat::No) {
                selected_slot = Some(2);
            } else if window.is_key_pressed(Key::Key3, minifb::KeyRepeat::No) {
                selected_slot = Some(3);
            } else if window.is_key_pressed(Key::Key4, minifb::KeyRepeat::No) {
                selected_slot = Some(4);
            } else if window.is_key_pressed(Key::Key5, minifb::KeyRepeat::No) {
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

            if window.is_key_pressed(Key::Key1, minifb::KeyRepeat::No) {
                selected_index = Some(0);
            } else if window.is_key_pressed(Key::Key2, minifb::KeyRepeat::No) {
                selected_index = Some(1);
            } else if window.is_key_pressed(Key::Key3, minifb::KeyRepeat::No) {
                selected_index = Some(2);
            } else if window.is_key_pressed(Key::Key4, minifb::KeyRepeat::No) {
                selected_index = Some(3);
            } else if window.is_key_pressed(Key::Key5, minifb::KeyRepeat::No) {
                selected_index = Some(4);
            } else if window.is_key_pressed(Key::Key6, minifb::KeyRepeat::No) {
                selected_index = Some(5);
            } else if window.is_key_pressed(Key::Key7, minifb::KeyRepeat::No) {
                selected_index = Some(6);
            } else if window.is_key_pressed(Key::Key8, minifb::KeyRepeat::No) {
                selected_index = Some(7);
            } else if window.is_key_pressed(Key::Key9, minifb::KeyRepeat::No) {
                selected_index = Some(8);
            }

            if let Some(idx) = selected_index {
                if idx < mount_points.len() {
                    show_mount_selector = false;

                    // Now show file dialog for the selected mount point
                    let mp_info = &mount_points[idx];
                    let extensions: Vec<&str> =
                        mp_info.extensions.iter().map(|s| s.as_str()).collect();

                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("ROM/Media Files", &extensions)
                        .pick_file()
                    {
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
                                        buffer = ui_render::create_default_screen(width, height);
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
            if let Some(debug_info) = sys.get_debug_info() {
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
                ));
            }
        }

        // Check for reset key (F12)
        if window.is_key_pressed(Key::F12, minifb::KeyRepeat::No) && rom_loaded {
            sys.reset();
            println!("System reset");
        }

        // Check for open ROM dialog (F3)
        if window.is_key_pressed(Key::F3, minifb::KeyRepeat::No) {
            let mount_points = sys.mount_points();

            // If system has only one mount point, go directly to file dialog
            // Otherwise, show mount point selector
            if mount_points.len() == 1 {
                let mp_info = &mount_points[0];
                let extensions: Vec<&str> = mp_info.extensions.iter().map(|s| s.as_str()).collect();

                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("ROM/Media Files", &extensions)
                    .pick_file()
                {
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
                                    println!("Loaded media into {}: {}", mp_info.name, path_str);
                                }
                                Err(e) => {
                                    eprintln!("Failed to mount media into {}: {}", mp_info.name, e);
                                    rom_hash = None;
                                    rom_loaded = false;
                                    buffer = ui_render::create_default_screen(width, height);
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to read file: {}", e);
                        }
                    }
                }
            } else if mount_points.len() > 1 {
                // Show mount point selector for systems with multiple mount points
                show_mount_selector = true;
                show_help = false;
            }
        }

        // Check for screenshot key (F4)
        if window.is_key_pressed(Key::F4, minifb::KeyRepeat::No) {
            match save_screenshot(&buffer, width, height, sys.system_name()) {
                Ok(path) => println!("Screenshot saved to: {}", path),
                Err(e) => eprintln!("Failed to save screenshot: {}", e),
            }
        }

        // F5 - Show save state slot selector
        if rom_loaded && window.is_key_pressed(Key::F5, minifb::KeyRepeat::No) {
            if sys.supports_save_states() {
                show_slot_selector = true;
                slot_selector_mode = "SAVE";
                show_help = false;
            } else {
                eprintln!("Save states are not supported for this system");
            }
        }

        // F6 - Show load state slot selector
        if rom_loaded && window.is_key_pressed(Key::F6, minifb::KeyRepeat::No) {
            if sys.supports_save_states() {
                show_slot_selector = true;
                slot_selector_mode = "LOAD";
                show_help = false;
            } else {
                eprintln!("Save states are not supported for this system");
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
            && settings.emulation_speed > 0.0
        {
            let keys_to_check: Vec<Key> = vec![
                string_to_key(&settings.keyboard.a),
                string_to_key(&settings.keyboard.b),
                string_to_key(&settings.keyboard.select),
                string_to_key(&settings.keyboard.start),
                string_to_key(&settings.keyboard.up),
                string_to_key(&settings.keyboard.down),
                string_to_key(&settings.keyboard.left),
                string_to_key(&settings.keyboard.right),
            ]
            .into_iter()
            .flatten()
            .collect();

            let mut ctrl0: u8 = 0;
            for k in keys_to_check.iter() {
                if window.is_key_down(*k) {
                    if let Some(bit) = key_mapping_to_button(*k, &settings) {
                        ctrl0 |= 1u8 << bit;
                    }
                }
            }
            sys.set_controller(0, ctrl0);

            // Handle keyboard input for PC system
            if let EmulatorSystem::PC(_) = &sys {
                // Get all keys and send to PC system
                let keys = window.get_keys_pressed(minifb::KeyRepeat::Yes);
                for key in keys {
                    sys.handle_keyboard(key, true);
                }
                // Note: Key releases are not easily tracked with minifb's API
                // The keyboard buffer in PC system handles this with timeouts
            }

            // Step one frame and display
            match sys.step_frame() {
                Ok(f) => {
                    buffer = f.pixels.clone();

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
