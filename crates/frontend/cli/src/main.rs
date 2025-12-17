use anyhow::Result;
use clap::Parser;
use std::fs::File;
use std::io::Write;
use emu_core::System;

#[derive(Parser)]
struct Args {
    /// System to run: "nes" or "gb"
    system: String,

    /// Optional path to a ROM file (not used by the skeleton)
    rom: Option<String>,

    /// Dump save-state to this file as JSON
    #[arg(long, default_value = "state.json")]
    save: String,
}

fn main() -> Result<()> {
    env_logger::init();
    let args = Args::parse();

    match args.system.as_str() {
        "nes" => {
            let mut sys = emu_nes::NesSystem::default();
            if let Some(rom) = args.rom.as_ref() {
                sys.load_rom_from_path(rom)?;
            }
            let frame = sys.step_frame()?;
            println!("Produced frame {}x{}", frame.width, frame.height);
            let state = sys.save_state();
            let mut f = File::create(&args.save)?;
            write!(f, "{}", serde_json::to_string_pretty(&state)?)?;
        }
        "gb" => {
            let mut sys = emu_gb::GbSystem::default();
            let frame = sys.step_frame()?;
            println!("Produced frame {}x{}", frame.width, frame.height);
            let state = sys.save_state();
            let mut f = File::create(&args.save)?;
            write!(f, "{}", serde_json::to_string_pretty(&state)?)?;
        }
        other => anyhow::bail!("Unsupported system: {}", other),
    }

    Ok(())
}
