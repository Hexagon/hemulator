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
- **Mapper 3 (CNROM)** - Simple CHR bank switching. Used by games like Gradius and Paperboy.
- **Mapper 4 (MMC3/TxROM)** - Advanced mapper with PRG/CHR banking and scanline IRQ counter. Used by games like Super Mario Bros. 3, Mega Man 3-6, and many others.
- **Mapper 7 (AxROM)** - 32KB PRG bank switching with single-screen mirroring. Used by games like Battletoads and Marble Madness.
- **Mapper 9 (MMC2/PxROM)** - PPU-triggered CHR bank switching with latch. Used exclusively by Mike Tyson's Punch-Out!!
- **Mapper 10 (MMC4/FxROM)** - Similar to MMC2 with different latch addresses. Used by Fire Emblem and other Japanese exclusive games.
- **Mapper 11 (Color Dreams)** - Simple PRG and CHR bank switching. Used in unlicensed Color Dreams and Wisdom Tree games.

### Implementation Status
- All supported mappers handle basic PRG and CHR banking
- MMC1 implements serial register writes and mirroring control
- MMC3 implements IRQ generation for raster effects
- MMC2 and MMC4 implement PPU-triggered CHR latch switching for advanced graphics effects
- CNROM and Color Dreams implement CHR-ROM bank switching
- AxROM implements single-screen mirroring control
- CHR-RAM is supported for games without CHR-ROM
- Comprehensive unit tests verify mapper behavior (48 tests total)

### Known Limitations
- Mapper implementations focus on common use cases
- Some advanced mapper features may not be fully implemented
- Four-screen mirroring is treated as vertical mirroring (2KB VRAM limitation)
- PPU implementation is simplified and may not perfectly replicate all NES hardware behaviors
- MMC2/MMC4 latch switching requires PPU integration for full accuracy
- Sprite rendering and background rendering are functional but may have minor visual artifacts in some games

### Unsupported Mappers
With the currently supported mappers (0, 1, 2, 3, 4, 7, 9, 10, 11), approximately 86% of all NES games are compatible. Common mappers planned for future implementation include:
- Mapper 5 (MMC5) - Advanced features used by Castlevania III (rare, ~1% of games)
- Mapper 19 (Namco 163) - Namco exclusive titles (~0.8% of games)

## GUI Controls

### Controller (Customizable via config.json)
- **Arrow Keys** - D-pad (default)
- **Z** - A button (default)
- **X** - B button (default)
- **Enter** - Start (default)
- **Left Shift** - Select (default)

### Function Keys
- **F1** - Toggle help overlay
- **F3** - Open ROM file dialog
- **F5-F9** - Save state to slot 1-5
- **Shift+F5-F9** - Load state from slot 1-5
- **F11** - Cycle window scale (1x → 2x → 4x → 8x)
- **F12** - Reset system
- **Escape** - Exit emulator

### Settings and Save States

The emulator includes a comprehensive settings system:
- **Settings**: Stored in `config.json` in the executable directory
  - Keyboard mappings (customizable)
  - Window scale preference
  - Last ROM path (auto-loads on restart)
- **Save States**: Stored in `saves/<rom_hash>/states.json`
  - 5 slots per game
  - Organized by ROM hash
  - Automatically persisted to disk

ROMs are auto-detected (NES iNES format and Game Boy format supported).

## Contributing

Follow Rust formatting and lints: `cargo fmt` and `cargo clippy`.
