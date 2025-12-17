# emu — Rust multi-system console emulator (starter)

This repository is a minimal, cross-platform starter for a multi-system console emulator focusing initially on NES and Game Boy.

Quickstart

```bash
# Build the workspace
cargo build

# Run CLI demo (produces a JSON save state at state.json)
cargo run -p emu_cli -- nes
cargo run -p emu_cli -- gb
```

Project layout

- `crates/core`: shared traits and types (System, Frame, save-state helpers).
- `crates/systems/nes`: NES system skeleton (CPU/PPU/APU placeholders).
- `crates/systems/gb`: Game Boy system skeleton.
- `crates/frontend/cli`: simple CLI runner for headless testing and state dumping.
- `crates/frontend/gui`: minimal GUI skeleton (future work).

Save-states are JSON by default for easy debugging; consider binary formats later for performance.

Contributing

Follow Rust formatting and lints: `cargo fmt` and `cargo clippy`.
