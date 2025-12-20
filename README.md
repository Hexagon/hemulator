# Hemulator — Multi-System Console Emulator

A cross-platform, multi-system console emulator written in Rust, supporting NES, Atari 2600, Game Boy, and PC emulation with comprehensive save state management and customizable controls.

## Features

- 🎮 **NES Emulation**: Full support for ~90%+ of NES games via 14 mapper implementations
- 🕹️ **Atari 2600 Emulation**: Support for most cartridge formats (2K-32K) with multiple banking schemes
- 🎲 **Game Boy Emulation**: Work-in-progress support for Game Boy/Game Boy Color ROMs
- 💻 **PC Emulation**: Basic IBM PC/XT emulation with 8086 CPU (experimental)
- 💾 **Save States**: 5 slots per game with instant save/load
- ⚙️ **Persistent Settings**: Customizable controls, window scaling, and auto-restore last ROM
- 🖥️ **Cross-Platform GUI**: Built with minifb for Windows, Linux, and macOS
- 🎵 **Audio Support**: Integrated audio playback via rodio (NES audio implemented)
- 📁 **ROM Auto-Detection**: Automatically detects NES (iNES), Atari 2600, Game Boy, and DOS executable formats

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

The NES emulator supports 14 mappers covering approximately **90%+ of all NES games** (based on nescartdb statistics).

### Supported Mappers
- **Mapper 0 (NROM)** - Basic mapper with no banking (~10% of games)
- **Mapper 1 (MMC1/SxROM)** - Tetris, Metroid, The Legend of Zelda (~28% of games)
- **Mapper 2 (UxROM)** - Mega Man, Castlevania, Contra (~11% of games)
- **Mapper 3 (CNROM)** - Gradius, Paperboy (~6.4% of games)
- **Mapper 4 (MMC3/TxROM)** - Super Mario Bros. 3, Mega Man 3-6 (~24% of games)
- **Mapper 7 (AxROM)** - Battletoads, Marble Madness (~3.1% of games)
- **Mapper 9 (MMC2/PxROM)** - Mike Tyson's Punch-Out!!
- **Mapper 10 (MMC4/FxROM)** - Fire Emblem (Japanese exclusives)
- **Mapper 11 (Color Dreams)** - Color Dreams and Wisdom Tree games (~1.3% of games)
- **Mapper 34 (BNROM)** - Deadly Towers, homebrew titles
- **Mapper 66 (GxROM)** - SMB + Duck Hunt, Doraemon (~1.2% of games)
- **Mapper 71 (Camerica/Codemasters)** - Fire Hawk, Micro Machines (~0.6% of games)
- **Mapper 79 (NINA-03/06)** - AVE games like Dudes with Attitude, Pyramid
- **Mapper 206 (Namco 118)** - Dragon Spirit, Famista (~1.8% of games)

### Implementation Details
- All mappers handle basic PRG and CHR banking
- MMC1: Serial register writes and mirroring control
- MMC3: IRQ generation for raster effects
- MMC2/MMC4: PPU-triggered CHR latch switching
- Namco 118: MMC3-like banking without IRQ support
- GxROM: Dual PRG/CHR bank switching
- BNROM: Simple 32KB PRG bank switching with CHR-RAM
- Camerica: UxROM variant with bus conflict prevention
- NINA-03/06: AVE discrete logic mapper with unusual register range
- CHR-RAM support for games without CHR-ROM
- Comprehensive unit tests (61 tests total)

See [MANUAL.md](MANUAL.md) for user-facing mapper information and game compatibility.

## Supported ROM Formats

- **NES**: iNES format (.nes) - automatically detected via header signature
- **Atari 2600**: Raw binary (.a26, .bin) - detected by size (2K, 4K, 8K, 12K, 16K, 32K)
- **Game Boy**: GB/GBC format (.gb, .gbc) - skeleton implementation (WIP)
- **PC/DOS**: COM/EXE executables (.com, .exe) - experimental 8086 emulation

## Project Structure

```
hemulator/
├── crates/
│   ├── core/           # Shared traits and types (System, Frame, save-state)
│   ├── systems/
│   │   ├── nes/        # NES emulation (CPU, PPU, APU, mappers)
│   │   ├── atari2600/  # Atari 2600 emulation (TIA, RIOT, cartridge banking)
│   │   ├── gb/         # Game Boy emulation (WIP)
│   │   └── pc/         # IBM PC/XT emulation (8086 CPU, experimental)
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
  - `cpu_6502` module: Reusable 6502 CPU (used by NES and Atari 2600)
  - `cpu_8086` module: Reusable 8086 CPU with core instruction set (used by PC emulation)
  - `cpu_8080` module: Intel 8080 CPU (foundation for Z80 and Game Boy CPUs)
  - `cpu_z80` module: Zilog Z80 CPU (stub implementation)
  - `cpu_lr35902` module: Game Boy CPU (stub implementation)
  - `apu` module: Reusable audio components (currently used by NES)
  - `ppu` module: Reusable video primitives
  - Save state serialization support
  
- **Systems**: Individual emulator implementations
  - **NES (`emu_nes`)**: Complete NES emulator with CPU, PPU, APU, and 14 mappers
  - **Atari 2600 (`emu_atari2600`)**: Atari 2600 with TIA, RIOT, and cartridge banking
  - **Game Boy (`emu_gb`)**: Work-in-progress Game Boy emulator
  - **PC (`emu_pc`)**: Experimental IBM PC/XT emulator with 8086 CPU, BIOS stub, and DOS executable support
  
- **Frontend (`emu_gui`)**: GUI application
  - Window management with `minifb`
  - Audio playback with `rodio`
  - Settings and save state management
  - ROM loading and system selection
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

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines on how to contribute.

## License

See [LICENSE](LICENSE) file for details.

## Acknowledgments

- Built with [minifb](https://github.com/emoon/rust_minifb) for cross-platform windowing
- Audio playback via [rodio](https://github.com/RustAudio/rodio)
- NES mapper references from [NESDev Wiki](https://www.nesdev.org/)

---

**Note**: This emulator is for educational purposes. Users must provide their own legally obtained ROM files. The project does not include or distribute any copyrighted game data.
