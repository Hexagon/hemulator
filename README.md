# Hemulator — Multi-System Console Emulator

A cross-platform, multi-system console emulator written in Rust, focusing on NES and Game Boy emulation with comprehensive save state management and customizable controls.

## Features

- 🎮 **NES Emulation**: Full support for 86% of NES games via 9 mapper implementations
- 💾 **Save States**: 5 slots per game with instant save/load
- ⚙️ **Persistent Settings**: Customizable controls, window scaling, and auto-restore last ROM
- 🖥️ **Cross-Platform GUI**: Built with minifb for Windows, Linux, and macOS
- 🎵 **Audio Support**: Integrated audio playback via rodio
- 📁 **ROM Auto-Detection**: Automatically detects NES (iNES) and Game Boy ROM formats

## Quick Start

### Installation

```bash
# Clone the repository
git clone https://github.com/Hexagon/hemulator.git
cd hemulator

# Build the project
cargo build --release

# Run the GUI
cargo run --release -p emu_gui
```

### First Run

1. **Launch the emulator**: The GUI will show a splash screen with instructions
2. **Load a ROM**: Press `F3` to open the file dialog, or provide a path as an argument:
   ```bash
   cargo run --release -p emu_gui -- path/to/your/game.nes
   ```
3. **View controls**: Press `F1` to see the help overlay with all available keys
4. **Play**: Use arrow keys for D-pad, Z for A button, X for B button

The emulator will remember your last ROM and automatically load it next time you start.

## Controls

### Game Controller (Customizable)

| Key | Action | Notes |
|-----|--------|-------|
| Arrow Keys | D-pad | Up/Down/Left/Right |
| Z | A button | Confirm/Jump |
| X | B button | Back/Action |
| Enter | Start | Pause menu |
| Left Shift | Select | Menu navigation |
| Escape | Exit | Close emulator |

*All controller mappings can be customized by editing `config.json`*

### Function Keys

| Key | Action | Description |
|-----|--------|-------------|
| F1 | Help Overlay | Show/hide all controls and key mappings |
| F3 | Open ROM | Browse and load a ROM file |
| F5-F9 | Save State | Save to slot 1-5 |
| Shift+F5-F9 | Load State | Load from slot 1-5 |
| F11 | Cycle Scale | Switch between 1x, 2x, 4x, 8x window size |
| F12 | Reset System | Restart the current game |

## Configuration

### Settings File (`config.json`)

Located in the same directory as the executable, this file stores:

```json
{
  "keyboard": {
    "a": "Z",
    "b": "X",
    "select": "LeftShift",
    "start": "Enter",
    "up": "Up",
    "down": "Down",
    "left": "Left",
    "right": "Right"
  },
  "window_width": 256,
  "window_height": 240,
  "scale": 2,
  "fullscreen": false,
  "last_rom_path": "/path/to/last/rom.nes"
}
```

**Customization**: Edit this file to change key bindings. Changes are automatically saved when you modify settings in-game (e.g., changing window scale with F11).

### Save States

Save states are stored in `saves/<rom_hash>/states.json`:
- Each game gets its own directory based on ROM hash
- 5 slots available per game (F5-F9 to save, Shift+F5-F9 to load)
- States are base64-encoded JSON for portability
- Directory structure is created automatically

Example structure:
```
saves/
  ├── a1b2c3d4.../
  │   └── states.json
  └── e5f6g7h8.../
      └── states.json
```

## NES Mapper Support

The NES emulator supports 9 mappers covering approximately **86% of all NES games**.

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

Common mappers planned for future implementation include:
- **Mapper 5 (MMC5)** - Advanced features used by Castlevania III (rare, ~1% of games)
- **Mapper 19 (Namco 163)** - Namco exclusive titles (~0.8% of games)

## Supported ROM Formats

### NES (Nintendo Entertainment System)
- **Format**: iNES (.nes files)
- **Detection**: Automatic via header signature (`NES\x1A`)
- **Status**: Fully supported with 9 mappers

### Game Boy / Game Boy Color
- **Format**: GB/GBC (.gb, .gbc files)
- **Detection**: Automatic via Nintendo logo at offset 0x104
- **Status**: Skeleton implementation (work in progress)

When you open a ROM file (F3), the emulator automatically detects the format and selects the appropriate system. Unsupported formats will display a clear error message.

## Project Structure

```
hemulator/
├── crates/
│   ├── core/           # Shared traits and types (System, Frame, save-state)
│   ├── systems/
│   │   ├── nes/        # NES emulation (CPU, PPU, APU, mappers)
│   │   └── gb/         # Game Boy emulation (skeleton)
│   └── frontend/
│       ├── gui/        # Main GUI frontend (minifb + rodio)
│       └── cli/        # CLI runner for testing
├── config.json         # User settings (created on first run)
└── saves/              # Save state directory (per-ROM)
```

## Development

### Building

```bash
# Development build
cargo build

# Release build (optimized)
cargo build --release

# Run tests
cargo test --workspace

# Run linting
cargo clippy --workspace --all-targets -- -D warnings

# Format code
cargo fmt --all
```

### CLI Testing

The project includes a CLI frontend for headless testing:

```bash
# Run NES system test
cargo run -p emu_cli -- nes

# Run Game Boy system test
cargo run -p emu_cli -- gb
```

This produces a JSON save state at `state.json` for debugging.

## Troubleshooting

### ROM won't load
- Ensure the ROM is in iNES format (.nes) for NES games
- Check that the file isn't corrupted
- Try a different ROM to verify the emulator works
- Check console output for specific error messages

### Audio issues
- The emulator requires a working audio output device
- On Linux, ensure ALSA is properly configured
- Try running with `--keep-logs` flag for debug information

### Settings not saving
- Verify you have write permissions in the emulator directory
- Check that `config.json` isn't marked as read-only
- Settings save automatically when changed (e.g., F11 for scale, F3 for ROM)

### Save states not working
- Ensure you've loaded a ROM first
- The `saves/` directory should be created automatically
- Check file system permissions

## Contributing

Contributions are welcome! Please follow these guidelines:

1. **Code Style**: Run `cargo fmt` before committing
2. **Linting**: Ensure `cargo clippy --workspace --all-targets -- -D warnings` passes
3. **Testing**: Add tests for new features and ensure all tests pass with `cargo test --workspace`
4. **Documentation**: Update relevant documentation for user-facing changes

### Areas for Contribution
- Additional mapper implementations (MMC5, VRC6, etc.)
- Game Boy emulation completion
- Performance optimizations
- Additional platform support
- UI/UX improvements

## License

See [LICENSE](LICENSE) file for details.

## Acknowledgments

- Built with [minifb](https://github.com/emoon/rust_minifb) for cross-platform windowing
- Audio playback via [rodio](https://github.com/RustAudio/rodio)
- NES mapper references from [NESDev Wiki](https://www.nesdev.org/)

---

**Note**: This emulator is for educational purposes. Users must provide their own legally obtained ROM files. The project does not include or distribute any copyrighted game data.
