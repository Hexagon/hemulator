use anyhow::Result;
use clap::Parser;
use emu_core::System;
use std::env;
use std::fs::File;
use std::io::Write;

#[derive(Parser)]
struct Args {
    /// System to run: "nes" or "gb"
    system: String,

    /// Optional path to a ROM file (not used by the skeleton)
    rom: Option<String>,

    /// Dump save-state to this file as JSON
    #[arg(long, default_value = "state.json")]
    save: String,

    /// Number of frames to run (NES only)
    #[arg(long, default_value_t = 5)]
    frames: u32,

    /// Print per-frame pixels + debug_state (NES only)
    #[arg(long, default_value_t = false)]
    debug: bool,

    /// Suppress all per-frame output (still writes --save)
    #[arg(long, default_value_t = false)]
    quiet: bool,

    /// Preserve env-var gated core logs (e.g. EMU_LOG_PPU_WRITES)
    #[arg(long, default_value_t = false)]
    keep_logs: bool,
}

fn main() -> Result<()> {
    env_logger::init();
    let args = Args::parse();

    // Avoid "why is it only debug spam" surprises: core logging can be extremely noisy.
    // CLI keeps core logs off by default; opt-in with --keep-logs.
    if !args.keep_logs {
        env::remove_var("EMU_LOG_PPU_WRITES");
        env::remove_var("EMU_LOG_UNKNOWN_OPS");
    }

    match args.system.as_str() {
        "nes" => {
            let mut sys = emu_nes::NesSystem::default();
            if let Some(rom) = args.rom.as_ref() {
                sys.load_rom_from_path(rom)?;
            }

            // Run frames. By default, stay quiet unless --debug is requested.
            for fnum in 1..=args.frames {
                let frame = sys.step_frame()?;
                if args.quiet {
                    continue;
                }

                if args.debug {
                    println!("Frame {}: {}x{}", fnum, frame.width, frame.height);
                    let dump_len = std::cmp::min(16, frame.pixels.len());
                    let mut out = String::new();
                    for i in 0..dump_len {
                        out.push_str(&format!("{:08X} ", frame.pixels[i]));
                    }
                    println!("First {} pixels: {}", dump_len, out);
                    let dbg = sys.debug_state();
                    println!(
                        "DEBUG STATE (frame {}):\n{}",
                        fnum,
                        serde_json::to_string_pretty(&dbg)?
                    );
                }
            }
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
