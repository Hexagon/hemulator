# emu — Rust multi-system console emulator (starter)

This repository is a minimal, cross-platform starter for a multi-system console emulator focusing initially on NES and Game Boy.

## Quickstart

```bash
# Build the workspace
cargo build

# Run GUI with a ROM
cargo run -p emu_gui -- path/to/rom.nes

# Run CLI demo (produces a JSON save state at state.json)
cargo run -p emu_cli -- nes
cargo run -p emu_cli -- gb
```

## Project layout

- `crates/core`: shared traits and types (System, Frame, save-state helpers).
- `crates/systems/nes`: NES system skeleton (CPU/PPU/APU placeholders).
- `crates/systems/gb`: Game Boy system skeleton.
- `crates/frontend/cli`: simple CLI runner for headless testing and state dumping.
- `crates/frontend/gui`: GUI frontend using minifb and rodio.

Save-states are JSON by default for easy debugging; consider binary formats later for performance.

## NES Mapper Support

The NES emulator currently supports the following iNES mappers:

### Supported Mappers
- **Mapper 0 (NROM)** - Basic mapper with no banking. Used by simple games.
- **Mapper 1 (MMC1/SxROM)** - Switchable PRG and CHR banks with configurable mirroring. Used by games like Tetris, Metroid, and The Legend of Zelda.
- **Mapper 2 (UxROM)** - Switchable 16KB PRG banks with fixed last bank. Used by games like Mega Man, Castlevania, and Contra.
- **Mapper 4 (MMC3/TxROM)** - Advanced mapper with PRG/CHR banking and scanline IRQ counter. Used by games like Super Mario Bros. 3, Mega Man 3-6, and many others.

### Implementation Status
- All supported mappers handle basic PRG and CHR banking
- MMC1 implements serial register writes and mirroring control
- MMC3 implements IRQ generation for raster effects
- CHR-RAM is supported for games without CHR-ROM

### Known Limitations
- Mapper implementations focus on common use cases
- Some advanced mapper features may not be fully implemented
- Four-screen mirroring is treated as vertical mirroring (2KB VRAM limitation)

### Unsupported Mappers
Games using other mappers (3, 5, 7, 9, 10, 11, etc.) will not work correctly. Common mappers planned for future implementation include:
- Mapper 3 (CNROM) - Simple CHR banking
- Mapper 7 (AxROM) - 32KB PRG switching with single-screen mirroring
- Mapper 9/10 (MMC2/MMC4) - Used by Punch-Out!!

## GUI Controls

- **Arrow Keys** - D-pad
- **Z** - A button
- **X** - B button
- **Enter** - Start
- **Left Shift** - Select
- **Escape** - Exit emulator
- **F12** - Reset (if implemented)

## Contributing

Follow Rust formatting and lints: `cargo fmt` and `cargo clippy`.
