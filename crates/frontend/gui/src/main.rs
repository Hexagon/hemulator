use minifb::{Key, Window, WindowOptions};
use emu_core::System;

fn main() {
    // Start a NES system and display one frame in a window.
    let mut sys = emu_nes::NesSystem::default();
    let frame = match sys.step_frame() {
        Ok(f) => f,
        Err(_) => {
            println!("Failed to produce frame");
            return;
        }
    };

    let width = frame.width as usize;
    let height = frame.height as usize;

    let mut window = match Window::new("emu_gui - NES frame", width, height, WindowOptions::default()) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("Failed to create window: {}", e);
            return;
        }
    };

    let mut buffer = frame.pixels.clone();

    while window.is_open() && !window.is_key_down(Key::Escape) {
        if let Err(e) = window.update_with_buffer(&buffer, width, height) {
            eprintln!("Window update error: {}", e);
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(16));
    }
}
