# Hemulator — Multi-System Console Emulator

A cross-platform, multi-system console emulator written in Rust, focusing on NES and Game Boy emulation with comprehensive save state management and customizable controls.

## Features

- 🎮 **NES Emulation**: Full support for 86% of NES games via 9 mapper implementations
- 💾 **Save States**: 5 slots per game with instant save/load
- ⚙️ **Persistent Settings**: Customizable controls, window scaling, and auto-restore last ROM
- 🖥️ **Cross-Platform GUI**: Built with minifb for Windows, Linux, and macOS
- 🎵 **Audio Support**: Integrated audio playback via rodio
- 📁 **ROM Auto-Detection**: Automatically detects NES (iNES) and Game Boy ROM formats

## For Users

Download the latest release from the [Releases](https://github.com/Hexagon/hemulator/releases) page. See [MANUAL.md](MANUAL.md) for complete usage instructions.

## For Developers

### Quick Start

```bash
# Clone the repository
git clone https://github.com/Hexagon/hemulator.git
cd hemulator

# Build the project
cargo build --release

# Run the emulator
cargo run --release -p emu_gui
# Or using the built binary (located in target/release/hemu)
./target/release/hemu path/to/your/game.nes
```

## NES Mapper Support

The NES emulator supports 9 mappers covering approximately **86% of all NES games**.

### Supported Mappers
- **Mapper 0 (NROM)** - Basic mapper with no banking
- **Mapper 1 (MMC1/SxROM)** - Tetris, Metroid, The Legend of Zelda
- **Mapper 2 (UxROM)** - Mega Man, Castlevania, Contra
- **Mapper 3 (CNROM)** - Gradius, Paperboy
- **Mapper 4 (MMC3/TxROM)** - Super Mario Bros. 3, Mega Man 3-6
- **Mapper 7 (AxROM)** - Battletoads, Marble Madness
- **Mapper 9 (MMC2/PxROM)** - Mike Tyson's Punch-Out!!
- **Mapper 10 (MMC4/FxROM)** - Fire Emblem (Japanese exclusives)
- **Mapper 11 (Color Dreams)** - Color Dreams and Wisdom Tree games

### Implementation Details
- All mappers handle basic PRG and CHR banking
- MMC1: Serial register writes and mirroring control
- MMC3: IRQ generation for raster effects
- MMC2/MMC4: PPU-triggered CHR latch switching
- CHR-RAM support for games without CHR-ROM
- Comprehensive unit tests (48 tests total)

See [MANUAL.md](MANUAL.md) for user-facing mapper information and game compatibility.

## Supported ROM Formats

- **NES**: iNES format (.nes) - automatically detected via header signature
- **Game Boy**: GB/GBC format (.gb, .gbc) - skeleton implementation (WIP)

## Project Structure

```
hemulator/
├── crates/
│   ├── core/           # Shared traits and types (System, Frame, save-state)
│   ├── systems/
│   │   ├── nes/        # NES emulation (CPU, PPU, APU, mappers)
│   │   └── gb/         # Game Boy emulation (skeleton)
│   └── frontend/
│       └── gui/        # GUI frontend (minifb + rodio) - builds as 'hemu'
├── config.json         # User settings (created on first run)
├── saves/              # Save state directory (per-ROM)
├── MANUAL.md           # User manual (included in releases)
└── AGENTS.md           # Guidelines for automated agents
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

### Testing

```bash
# Run all tests
cargo test --workspace

# Run tests for specific crate
cargo test -p emu_nes
cargo test -p emu_core

# Run tests with output
cargo test --workspace -- --nocapture
```

### Architecture

The project follows a modular architecture:

- **Core (`emu_core`)**: Defines traits and types shared across systems
  - `System` trait for emulator implementations
  - `Frame` for video output
  - Save state serialization support
  
- **Systems**: Individual emulator implementations
  - **NES (`emu_nes`)**: Complete NES emulator with CPU, PPU, APU, and 9 mappers
  - **Game Boy (`emu_gb`)**: Skeleton implementation (WIP)
  
- **Frontend (`emu_gui`)**: GUI application
  - Window management with `minifb`
  - Audio playback with `rodio`
  - Settings and save state management
  - ROM loading and system selection

### Adding New Mappers

To add a new NES mapper:

1. Create a new module in `crates/systems/nes/src/mappers/`
2. Implement the `Mapper` trait
3. Add unit tests for mapper behavior
4. Register the mapper in `create_mapper()` in `mappers/mod.rs`
5. Update documentation

See existing mapper implementations for examples.

## Contributing

Contributions are welcome! Please follow these guidelines:

1. **Code Style**: Run `cargo fmt` before committing
2. **Linting**: Ensure `cargo clippy --workspace --all-targets -- -D warnings` passes
3. **Testing**: Add tests for new features and ensure all tests pass
4. **Documentation**: Update MANUAL.md for user-facing changes, README.md for developer info

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
