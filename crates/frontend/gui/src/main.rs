mod settings;
mod save_state;

use emu_core::System;
use minifb::{Key, Scale, Window, WindowOptions};
use rodio::{OutputStream, Source};
use settings::Settings;
use save_state::GameSaves;
use std::env;
use std::sync::mpsc::{sync_channel, Receiver};
use std::time::{Duration, Instant};

fn key_to_button(key: Key) -> Option<u8> {
    // NES controller bit mapping: 0=A,1=B,2=Select,3=Start,4=Up,5=Down,6=Left,7=Right
    match key {
        Key::Z => Some(0),         // A
        Key::X => Some(1),         // B
        Key::LeftShift => Some(2), // Select
        Key::Enter => Some(3),     // Start
        Key::Up => Some(4),
        Key::Down => Some(5),
        Key::Left => Some(6),
        Key::Right => Some(7),
        _ => None,
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

    // If no ROM path provided via args, try to load from settings
    if rom_path.is_none() {
        rom_path = settings.last_rom_path.clone();
    }

    let mut sys = emu_nes::NesSystem::default();
    let mut _rom_data: Option<Vec<u8>> = None;
    let mut rom_hash: Option<String> = None;

    if let Some(p) = &rom_path {
        match std::fs::read(p) {
            Ok(data) => {
                rom_hash = Some(GameSaves::rom_hash(&data));
                _rom_data = Some(data.clone());
                if let Err(e) = sys.load_rom(&data) {
                    eprintln!("Failed to load ROM: {}", e);
                    rom_hash = None;
                    _rom_data = None;
                } else {
                    // Update settings with last ROM path
                    settings.last_rom_path = Some(p.clone());
                    if let Err(e) = settings.save() {
                        eprintln!("Warning: Failed to save settings: {}", e);
                    }
                }
            }
            Err(e) => {
                eprintln!("Failed to read ROM file: {}", e);
            }
        }
    }

    if rom_hash.is_none() {
        println!("No ROM loaded. Press F3 to open a ROM file.");
    }

    // Create window using settings or NES resolution
    let frame = match sys.step_frame() {
        Ok(f) => f,
        Err(_) => {
            eprintln!("Failed to produce initial frame");
            return;
        }
    };

    let width = frame.width as usize;
    let height = frame.height as usize;

    let scale = match settings.scale {
        1 => Scale::X1,
        2 => Scale::X2,
        4 => Scale::X4,
        8 => Scale::X8,
        _ => Scale::X2,
    };

    let mut window = match Window::new(
        "emu_gui - NES",
        width,
        height,
        WindowOptions {
            scale,
            ..WindowOptions::default()
        },
    ) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("Failed to create window: {}", e);
            return;
        }
    };

    // Initialize audio output with a streaming channel-backed source to avoid underruns.
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
    let (audio_tx, audio_rx) = sync_channel::<i16>(44100 * 2); // ~2 seconds buffer
    if let Err(e) = stream_handle.play_raw(
        StreamSource {
            rx: audio_rx,
            sample_rate: 44100,
        }
        .convert_samples(),
    ) {
        eprintln!(
            "Warning: Failed to start audio playback: {}. Audio will be disabled.",
            e
        );
    }

    // controller state: bitfield per controller
    let mut ctrl0: u8;

    // initial buffer
    let mut buffer = frame.pixels.clone();

    // timing trackers
    let mut last_audio = Instant::now();
    let mut last_frame = Instant::now();

    // Load saves for current ROM if available
    let mut game_saves = if let Some(ref hash) = rom_hash {
        GameSaves::load(hash)
    } else {
        GameSaves::default()
    };

    while window.is_open() && !window.is_key_down(Key::Escape) {
        // Check for reset key (F12)
        if window.is_key_down(Key::F12) {
            sys.reset();
        }

        // Check for open ROM dialog (F3)
        if window.is_key_pressed(Key::F3, minifb::KeyRepeat::No) {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("NES ROM", &["nes"])
                .pick_file()
            {
                let path_str = path.to_string_lossy().to_string();
                match std::fs::read(&path) {
                    Ok(data) => {
                        rom_hash = Some(GameSaves::rom_hash(&data));
                        _rom_data = Some(data.clone());
                        match sys.load_rom(&data) {
                            Ok(_) => {
                                settings.last_rom_path = Some(path_str.clone());
                                if let Err(e) = settings.save() {
                                    eprintln!("Warning: Failed to save settings: {}", e);
                                }
                                // Load saves for new ROM
                                game_saves = if let Some(ref hash) = rom_hash {
                                    GameSaves::load(hash)
                                } else {
                                    GameSaves::default()
                                };
                                println!("Loaded ROM: {}", path_str);
                            }
                            Err(e) => {
                                eprintln!("Failed to load ROM: {}", e);
                                rom_hash = None;
                                _rom_data = None;
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to read ROM file: {}", e);
                    }
                }
            }
        }

        // Check for fullscreen/scale toggle (F11)
        if window.is_key_pressed(Key::F11, minifb::KeyRepeat::No) {
            // Cycle through scales: 1 -> 2 -> 4 -> 8 -> 1
            settings.scale = match settings.scale {
                1 => 2,
                2 => 4,
                4 => 8,
                8 => 1,
                _ => 2,
            };

            let new_scale = match settings.scale {
                1 => Scale::X1,
                2 => Scale::X2,
                4 => Scale::X4,
                8 => Scale::X8,
                _ => Scale::X2,
            };

            // Recreate window with new scale
            window = match Window::new(
                "emu_gui - NES",
                width,
                height,
                WindowOptions {
                    scale: new_scale,
                    ..WindowOptions::default()
                },
            ) {
                Ok(w) => w,
                Err(e) => {
                    eprintln!("Failed to recreate window: {}", e);
                    break;
                }
            };

            if let Err(e) = settings.save() {
                eprintln!("Warning: Failed to save settings: {}", e);
            }
            println!("Scale changed to {}x", settings.scale);
        }

        // Check for save state keys (F5-F9 for save, Shift+F5-F9 for load)
        let shift_pressed = window.is_key_down(Key::LeftShift) || window.is_key_down(Key::RightShift);
        
        for (idx, key) in [Key::F5, Key::F6, Key::F7, Key::F8, Key::F9].iter().enumerate() {
            let slot = (idx + 1) as u8;
            
            if window.is_key_pressed(*key, minifb::KeyRepeat::No) {
                if let Some(ref hash) = rom_hash {
                    if shift_pressed {
                        // Load state
                        match game_saves.load_slot(slot) {
                            Ok(data) => {
                                match serde_json::from_slice::<serde_json::Value>(&data) {
                                    Ok(state) => {
                                        match sys.load_state(&state) {
                                            Ok(_) => println!("Loaded state from slot {}", slot),
                                            Err(e) => eprintln!("Failed to load state: {}", e),
                                        }
                                    }
                                    Err(e) => eprintln!("Failed to parse save state: {}", e),
                                }
                            }
                            Err(e) => eprintln!("Failed to load from slot {}: {}", slot, e),
                        }
                    } else {
                        // Save state
                        let state = sys.save_state();
                        match serde_json::to_vec(&state) {
                            Ok(data) => {
                                match game_saves.save_slot(slot, &data, hash) {
                                    Ok(_) => println!("Saved state to slot {}", slot),
                                    Err(e) => eprintln!("Failed to save to slot {}: {}", slot, e),
                                }
                            }
                            Err(e) => eprintln!("Failed to serialize state: {}", e),
                        }
                    }
                } else {
                    eprintln!("No ROM loaded - cannot save/load state");
                }
            }
        }

        // Get all keys for controller mapping
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

        // Map pressed keys to controller bits (controller 0 only for now)
        ctrl0 = 0;
        for k in keys_to_check.iter() {
            if window.is_key_down(*k) {
                if let Some(bit) = key_mapping_to_button(*k, &settings) {
                    ctrl0 |= 1u8 << bit;
                }
            }
        }
        sys.set_controller(0, ctrl0);

        // Step one frame and display
        match sys.step_frame() {
            Ok(f) => {
                buffer = f.pixels.clone();

                // Audio: generate based on elapsed wall time to avoid gaps when the loop runs slow.
                let elapsed = last_audio.elapsed();
                let mut wanted = (elapsed.as_secs_f64() * 44_100.0).round() as usize;
                // Bound to keep buffers reasonable.
                wanted = wanted.clamp(400, 2000);
                let audio_samples = sys.get_audio_samples(wanted);
                last_audio = Instant::now();
                for s in audio_samples {
                    let _ = audio_tx.try_send(s);
                }
            }
            Err(e) => eprintln!("Frame generation error: {:?}", e),
        }

        if let Err(e) = window.update_with_buffer(&buffer, width, height) {
            eprintln!("Window update error: {}", e);
            break;
        }

        // ~60 FPS if ahead; if behind, skip sleep.
        let frame_dt = last_frame.elapsed();
        if frame_dt < Duration::from_millis(16) {
            std::thread::sleep(Duration::from_millis(16) - frame_dt);
        }
        last_frame = Instant::now();
    }
}
