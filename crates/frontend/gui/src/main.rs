mod rom_detect;
mod save_state;
mod settings;
mod ui_render;

use emu_core::System;
use minifb::{Key, Scale, ScaleMode, Window, WindowOptions};
use rodio::{OutputStream, Source};
use rom_detect::{detect_rom_type, SystemType};
use save_state::GameSaves;
use settings::Settings;
use std::env;
use std::sync::mpsc::{sync_channel, Receiver};
use std::time::{Duration, Instant};

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

    let mut sys = emu_nes::NesSystem::default();
    let mut rom_hash: Option<String> = None;
    let mut rom_loaded = false;

    // Try to load ROM if path is available
    if let Some(p) = &rom_path {
        match std::fs::read(p) {
            Ok(data) => match detect_rom_type(&data) {
                Ok(SystemType::NES) => {
                    rom_hash = Some(GameSaves::rom_hash(&data));
                    if let Err(e) = sys.mount("Cartridge", &data) {
                        eprintln!("Failed to load NES ROM: {}", e);
                        rom_hash = None;
                    } else {
                        rom_loaded = true;
                        settings.set_mount_point("Cartridge", p.clone());
                        settings.last_rom_path = Some(p.clone()); // Keep for backward compat
                        if let Err(e) = settings.save() {
                            eprintln!("Warning: Failed to save settings: {}", e);
                        }
                        println!("Loaded NES ROM: {}", p);
                    }
                }
                Ok(SystemType::GameBoy) => {
                    eprintln!("Game Boy ROMs are not yet fully implemented");
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

    // Create window using NES resolution (256x240)
    let width = 256;
    let height = 240;

    let scale = match settings.scale {
        1 => Scale::X1,
        2 => Scale::X2,
        4 => Scale::X4,
        8 => Scale::X8,
        _ => Scale::X2,
    };

    let mut window = match Window::new(
        "Hemulator - Multi-System Emulator",
        width,
        height,
        WindowOptions {
            resize: true,
            scale_mode: ScaleMode::AspectRatioStretch,
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

    // Slot selector state
    let mut show_slot_selector = false;
    let mut slot_selector_mode = "SAVE"; // "SAVE" or "LOAD"

    // Timing trackers
    let mut last_frame = Instant::now();

    // Audio: NES runs at ~60 FPS, generate samples to match
    const SAMPLE_RATE: usize = 44100;
    const FRAME_RATE: usize = 60;
    const SAMPLES_PER_FRAME: usize = SAMPLE_RATE / FRAME_RATE; // ~735 samples per frame

    // Load saves for current ROM if available
    let mut game_saves = if let Some(ref hash) = rom_hash {
        GameSaves::load(hash)
    } else {
        GameSaves::default()
    };

    while window.is_open() && !window.is_key_down(Key::Escape) {
        // Toggle help overlay (F1)
        if window.is_key_pressed(Key::F1, minifb::KeyRepeat::No) {
            show_help = !show_help;
            show_slot_selector = false; // Close slot selector if open
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
                        // Save state
                        let state = sys.save_state();
                        match serde_json::to_vec(&state) {
                            Ok(data) => match game_saves.save_slot(slot, &data, hash) {
                                Ok(_) => println!("Saved state to slot {}", slot),
                                Err(e) => eprintln!("Failed to save to slot {}: {}", slot, e),
                            },
                            Err(e) => eprintln!("Failed to serialize state: {}", e),
                        }
                    } else {
                        // Load state
                        match game_saves.load_slot(slot) {
                            Ok(data) => match serde_json::from_slice::<serde_json::Value>(&data) {
                                Ok(state) => match sys.load_state(&state) {
                                    Ok(_) => println!("Loaded state from slot {}", slot),
                                    Err(e) => eprintln!("Failed to load state: {}", e),
                                },
                                Err(e) => eprintln!("Failed to parse save state: {}", e),
                            },
                            Err(e) => eprintln!("Failed to load from slot {}: {}", slot, e),
                        }
                    }
                }
            }

            // Render slot selector
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
            std::thread::sleep(Duration::from_millis(16));
            continue;
        }

        // Prepare help overlay buffer when requested; keep processing input so other
        // keys still work while the overlay is visible.
        let mut help_overlay: Option<Vec<u32>> = None;
        if show_help {
            help_overlay = Some(ui_render::create_help_overlay(width, height, &settings));
        }

        // Check for reset key (F12)
        if window.is_key_pressed(Key::F12, minifb::KeyRepeat::No) && rom_loaded {
            sys.reset();
            println!("System reset");
        }

        // Check for open ROM dialog (F3)
        if window.is_key_pressed(Key::F3, minifb::KeyRepeat::No) {
            // For now, NES only has one mount point ("Cartridge"), so we go directly to file dialog
            // Future enhancement: show mount point selector for systems with multiple mount points
            let mount_point_id = "Cartridge"; // For NES
            
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("ROM Files", &["nes", "gb", "gbc"])
                .pick_file()
            {
                let path_str = path.to_string_lossy().to_string();
                match std::fs::read(&path) {
                    Ok(data) => match detect_rom_type(&data) {
                        Ok(SystemType::NES) => {
                            rom_hash = Some(GameSaves::rom_hash(&data));
                            match sys.mount(mount_point_id, &data) {
                                Ok(_) => {
                                    rom_loaded = true;
                                    settings.set_mount_point(mount_point_id, path_str.clone());
                                    settings.last_rom_path = Some(path_str.clone()); // Keep for backward compat
                                    if let Err(e) = settings.save() {
                                        eprintln!("Warning: Failed to save settings: {}", e);
                                    }
                                    game_saves = if let Some(ref hash) = rom_hash {
                                        GameSaves::load(hash)
                                    } else {
                                        GameSaves::default()
                                    };
                                    println!("Loaded NES ROM into {}: {}", mount_point_id, path_str);
                                }
                                Err(e) => {
                                    eprintln!("Failed to mount ROM into {}: {}", mount_point_id, e);
                                    rom_hash = None;
                                    rom_loaded = false;
                                    buffer = ui_render::create_default_screen(width, height);
                                }
                            }
                        }
                        Ok(SystemType::GameBoy) => {
                            eprintln!("Game Boy ROMs are not yet fully implemented");
                            buffer = ui_render::create_default_screen(width, height);
                        }
                        Err(e) => {
                            eprintln!("Unsupported ROM: {}", e);
                            buffer = ui_render::create_default_screen(width, height);
                        }
                    },
                    Err(e) => {
                        eprintln!("Failed to read ROM file: {}", e);
                    }
                }
            }
        }

        // Check for fullscreen/scale toggle (F11) – fullscreen not available in minifb 0.25,
        // so we cycle through 1x/2x/4x/8x scales instead.
        if window.is_key_pressed(Key::F11, minifb::KeyRepeat::No) {
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

            window = match Window::new(
                "Hemulator - Multi-System Emulator",
                width,
                height,
                WindowOptions {
                    resize: true,
                    scale_mode: ScaleMode::AspectRatioStretch,
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

        // F5 - Show save state slot selector
        if rom_loaded && window.is_key_pressed(Key::F5, minifb::KeyRepeat::No) {
            show_slot_selector = true;
            slot_selector_mode = "SAVE";
            show_help = false;
        }

        // F6 - Show load state slot selector
        if rom_loaded && window.is_key_pressed(Key::F6, minifb::KeyRepeat::No) {
            show_slot_selector = true;
            slot_selector_mode = "LOAD";
            show_help = false;
        }

        // Handle controller input only if ROM is loaded
        if rom_loaded && !show_help {
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

            // Step one frame and display
            match sys.step_frame() {
                Ok(f) => {
                    buffer = f.pixels.clone();

                    // Audio generation: generate a consistent number of samples per frame
                    // to match the ~60 FPS frame rate (44100 Hz / 60 fps ≈ 735 samples)
                    let audio_samples = sys.get_audio_samples(SAMPLES_PER_FRAME);
                    for s in audio_samples {
                        let _ = audio_tx.try_send(s);
                    }
                }
                Err(e) => eprintln!("Frame generation error: {:?}", e),
            }
        }

        let frame_to_present: &[u32] = if let Some(ref overlay) = help_overlay {
            overlay.as_slice()
        } else {
            &buffer
        };

        if let Err(e) = window.update_with_buffer(frame_to_present, width, height) {
            eprintln!("Window update error: {}", e);
            break;
        }

        // ~60 FPS timing
        let frame_dt = last_frame.elapsed();
        if frame_dt < Duration::from_millis(16) {
            std::thread::sleep(Duration::from_millis(16) - frame_dt);
        }
        last_frame = Instant::now();
    }
}
