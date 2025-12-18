use emu_core::System;
use minifb::{Key, Scale, Window, WindowOptions};
use rodio::{OutputStream, Source};
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

    let mut sys = emu_nes::NesSystem::default();
    if let Some(p) = rom_path {
        if let Err(e) = sys.load_rom_from_path(p) {
            eprintln!("Failed to load ROM: {}", e);
            return;
        }
    } else {
        println!("No ROM provided; running with empty state. To run a ROM pass its path as the first argument.");
    }

    // Create window using NES resolution
    let frame = match sys.step_frame() {
        Ok(f) => f,
        Err(_) => {
            eprintln!("Failed to produce initial frame");
            return;
        }
    };

    let width = frame.width as usize;
    let height = frame.height as usize;

    let mut window = match Window::new(
        "emu_gui - NES",
        width,
        height,
        WindowOptions {
            scale: Scale::X2,
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

    while window.is_open() && !window.is_key_down(Key::Escape) {
        // Map pressed keys to controller bits (controller 0 only for now)
        ctrl0 = 0;
        for k in [
            Key::Z,
            Key::X,
            Key::LeftShift,
            Key::Enter,
            Key::Up,
            Key::Down,
            Key::Left,
            Key::Right,
        ]
        .iter()
        {
            if window.is_key_down(*k) {
                if let Some(bit) = key_to_button(*k) {
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
