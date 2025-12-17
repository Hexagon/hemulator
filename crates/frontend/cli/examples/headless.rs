use std::env;
use emu_core::System;

fn main() {
    let args: Vec<String> = env::args().collect();
    let system = args.get(1).map(|s| s.as_str()).unwrap_or("nes");

    match system {
        "nes" => {
            let mut sys = emu_nes::NesSystem::default();
            let frame = sys.step_frame().unwrap();
            println!("Headless NES frame: {}x{}", frame.width, frame.height);
            println!("Save-state: {}", serde_json::to_string_pretty(&sys.save_state()).unwrap());
        }
        "gb" => {
            let mut sys = emu_gb::GbSystem::default();
            let frame = sys.step_frame().unwrap();
            println!("Headless GB frame: {}x{}", frame.width, frame.height);
            println!("Save-state: {}", serde_json::to_string_pretty(&sys.save_state()).unwrap());
        }
        other => eprintln!("Unknown system: {}", other),
    }
}
