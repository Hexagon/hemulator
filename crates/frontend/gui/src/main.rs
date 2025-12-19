mod crt_filter;
mod rom_detect;
mod save_state;
mod settings;
mod ui_render;

use emu_core::System;
use minifb::{Key, Window, WindowOptions};
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
    // Note: We still use ROM type detection at startup to determine which system to instantiate.
    // The mount point system works within a system for managing media slots.
    if let Some(p) = &rom_path {
        match std::fs::read(p) {
            Ok(data) => match detect_rom_type(&data) {
                Ok(SystemType::NES) => {
                    rom_hash = Some(GameSaves::rom_hash(&data));
                    // Use the mount point system to load the cartridge
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
            let a = ((o >> 24) & 0xFF) as u32;
            if a == 0 {
                out.push(b);
                continue;
            }
            if a == 255 {
                out.push(0xFF00_0000 | (o & 0x00FF_FFFF));
                continue;
            }

            let inv = 255 - a;
            let br = ((b >> 16) & 0xFF) as u32;
            let bg = ((b >> 8) & 0xFF) as u32;
            let bb = (b & 0xFF) as u32;

            let or = ((o >> 16) & 0xFF) as u32;
            let og = ((o >> 8) & 0xFF) as u32;
            let ob = (o & 0xFF) as u32;

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
            show_debug = false; // Close debug if open
        }

        // Toggle debug overlay (F10)
        if window.is_key_pressed(Key::F10, minifb::KeyRepeat::No) && rom_loaded {
            show_debug = !show_debug;
            show_slot_selector = false; // Close slot selector if open
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
            let debug_info = sys.get_debug_info();
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
            ));
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
        if rom_loaded && !show_help && !show_slot_selector && !show_mount_selector {
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

                    // Apply CRT filter if not showing overlays
                    if !show_help && !show_slot_selector {
                        settings.crt_filter.apply(&mut buffer, width, height);
                    }

                    // Audio generation: generate samples based on actual frame rate
                    // NTSC: ~60.1 FPS (≈734 samples), PAL: ~50.0 FPS (≈882 samples)
                    let timing = sys.timing();
                    let samples_per_frame = (SAMPLE_RATE as f64 / timing.frame_rate_hz()).round() as usize;
                    let audio_samples = sys.get_audio_samples(samples_per_frame);
                    for s in audio_samples {
                        let _ = audio_tx.try_send(s);
                    }
                }
                Err(e) => eprintln!("Frame generation error: {:?}", e),
            }
        }

        let frame_to_present: &[u32] = if show_slot_selector {
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
            let mount_buffer = ui_render::create_mount_point_selector(
                width,
                height,
                &mount_points,
            );
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

        // Get target frame time from system timing mode
        let target_frame_time = if rom_loaded {
            let timing = sys.timing();
            let frame_rate = timing.frame_rate_hz();
            Duration::from_secs_f64(1.0 / frame_rate)
        } else {
            // Default to NTSC timing when no ROM loaded
            Duration::from_secs_f64(1.0 / 60.0988)
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
