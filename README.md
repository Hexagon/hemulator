# Hemulator — Multi-System Console Emulator

A cross-platform, multi-system console emulator written in Rust. **NES emulation is fully working** with ~90% game coverage. Other systems (Atari 2600, Game Boy, SNES, N64, PC/DOS) are in various stages of development.

## Features

- 🎮 **NES Emulation**: 14 mapper implementations supporting most popular games
- 💻 **PC Emulation**: COM/EXE loading, CGA text and graphics modes. MS-DOS 5.0 and FreeDOS support
- 🕹️ **Atari 2600 Emulation**: Support for most cartridge formats (2K-32K) with multiple banking schemes
- 🎲 **Game Boy Emulation**: MBC0/1/2/3/5 and HuC1 support with full APU and timer functionality
- 🏰 **SNES Emulation**: Full CPU and SPC700 APU, PPU modes 0-7 (0-1 complete), graphics working, silent gameplay
- 🎮 **N64 Emulation**: 3D rendering functionality
- 🖱️ **Modern GUI**: Menu bar and status bar with mouse and keyboard support - no more cryptic F-keys!
- 💾 **Save States**: 5 slots per game with instant save/load (Ctrl+1-5 / Ctrl+Shift+1-5)
- ⚙️ **Persistent Settings**: Customizable controls, window scaling, and auto-restore last ROM
- 🖥️ **Cross-Platform GUI**: Built with SDL2 for Windows, Linux, and macOS
- 🎨 **Video Processing**: Modular architecture supporting both software and OpenGL-accelerated rendering
- 🎞️ **CRT Filters**: Hardware-accelerated shader-based CRT effects (scanlines, phosphor, full CRT)
- 🎵 **Audio Support**: Integrated audio playback via rodio (NES and Atari 2600 audio implemented)
- 📁 **ROM Auto-Detection**: Automatically detects NES (iNES), Atari 2600, Game Boy, SNES, N64, and DOS executable formats

## System Implementation Status

| System | Status | CPU | Graphics | Audio | Input | Save States / Persistance | Coverage/Notes |
|--------|--------|-----|----------|-------|-------|-------------|----------------|
| **NES** | ✅ Fully Working | 6502 (Complete) | PPU (Complete) | APU (Complete) | ✅ | ✅ | ~90% of all games via 14 mappers |
| **PC (DOS)** | ⚠️ Experimental | 8086-80386 (16-bit complete, 32-bit in progress) | CGA/EGA/VGA (Text + Graphics) | ❌ Not implemented | ⚠️ Keyboard passthrough | ✅ | COM/EXE loading; multi-mode video |
| **Atari 2600** | 🚧 In Development | 6502/6507 (Complete) | TIA (Partial - rendering issues) | TIA (Complete) | ✅ | ✅ | Most cartridge formats; rendering WIP |
| **CHIP-8** | ✅ Fully Working | CHIP-8 VM (Complete) | Multi-mode (Complete) | Beep timer (Complete) | ✅ | ✅ | CHIP-8/Hires/Super-CHIP/XO-CHIP/Mega-CHIP |
| **Game Boy** | ✅ Fully Functional | LR35902 (Complete) | PPU (Complete) | APU (Complete) | ✅ | ✅ | ~99% of games; MBC0/1/2/3/5, HuC1 supported, WRAM banking |
| **SMS** | ✅ Functional | Z80 (Complete) | VDP (Complete) | PSG (Complete) | ✅ | ✅ | Full hardware emulation; needs game testing |
| **SNES** | 🚧 In Development | 65C816 (Complete) | PPU (Partial) | SPC700 (Complete, no DSP) | ✅ | ✅ | Graphics working; modes 0-1 complete; no audio output |
| **N64** | 🚧 In Development | R4300i (Complete) | RDP/RSP (Partial) | ❌ Not implemented | ⚠️ Ready (not integrated) | ✅ | 3D rendering works; limited game support |

**Legend:**
- ✅ Fully Working - Production ready with comprehensive features
- ⚠️ Functional - Core features work but missing some capabilities
- 🚧 In Development - Active work in progress with partial functionality
- 🧪 Experimental - Early development stage, not recommended for general use
- ❌ Not implemented - Component not yet available

## For Users

Download the latest release from the [Releases](https://github.com/Hexagon/hemulator/releases) page.

**Documentation**:
- 📚 **[Complete Documentation Site](https://hexagon.56k.guru)** - Full documentation with user guides and developer references
- 📖 **[User Manual](docs/MANUAL.md)** - Complete usage instructions, controls, and system-specific information
- 🎮 **[System Documentation](https://hexagon.56k.guru/systems/)** - Detailed information about each emulated system

## For Developers

**Core Documentation**:
- 🏗️ **[Architecture Guide](docs/ARCHITECTURE.md)** - Overall emulation system architecture and design patterns
- 🤝 **[Contributing Guide](docs/CONTRIBUTING.md)** - Development workflow and contribution guidelines
- 🤖 **[Agent Guidelines](AGENTS.md)** - Implementation guidelines and CI requirements
- 🌐 **[Full Documentation Site](https://hexagon.56k.guru)** - Complete developer and API documentation

**Planning and Reference Documents**:
- [N64_STATUS.md](docs/N64_STATUS.md) - Active development status for Nintendo 64 emulation
- [NEXT_EMULATOR_RECOMMENDATION.md](docs/NEXT_EMULATOR_RECOMMENDATION.md) - Recommendation for next emulator to implement
- [SMS_IMPLEMENTATION_GUIDE.md](docs/SMS_IMPLEMENTATION_GUIDE.md) - Practical guide for implementing Sega Master System
- [references/](docs/references/) - CPU and hardware technical references

**System-Specific Documentation**:
- [NES](crates/systems/nes/README.md) - PPU, APU, mappers
- [Game Boy](crates/systems/gb/README.md) - PPU, APU, MBCs
- [Atari 2600](crates/systems/atari2600/README.md) - TIA, RIOT, cartridges
- [CHIP-8](crates/systems/chip8/README.md) - VM architecture, display modes
- [SMS](crates/systems/sms/README.md) - Z80 CPU, VDP, PSG
- [SNES](crates/systems/snes/README.md) - PPU modes, memory map
- [N64](crates/systems/n64/README.md) - RDP renderer, RSP
- [PC](crates/systems/pc/README.md) - Video adapters, BIOS

### Quick Start

**Linux Development Dependencies:**

On Ubuntu/Debian, install these packages before building:
```bash
sudo apt-get install libasound2-dev cmake pkg-config
```

- `libasound2-dev` - Required for audio support (ALSA)
- `cmake` - Required by some Rust dependencies
- `pkg-config` - Required for library detection during build

**Building and Running:**

```bash
# Clone the repository
git clone https://github.com/Hexagon/hemulator.git
cd hemulator

# Build the project (optimized for distribution)
cargo build --release

# For faster iterative development, use release-quick profile
# ~18x faster incremental builds with near-release performance
cargo build --profile release-quick

# Run the emulator
cargo run --release -p emu_gui
# Or using the built binary (located in target/release/hemu)
./target/release/hemu path/to/your/game.nes
```

## Architecture

Hemulator uses a modular architecture that separates reusable emulation components from system-specific implementations. For detailed architecture documentation, see **[ARCHITECTURE.md](docs/ARCHITECTURE.md)**.

**Core Components** (`crates/core/`):
- CPUs: 6502, 65C816, LR35902, Z80, 8080, MIPS R4300i, 8086/80186/80286/80386
- Audio: APU channels, envelopes, mixers
- Graphics: ZBuffer, ColorOps, palette/tile utilities
- Traits: System, Cpu, Renderer, AudioChip

**System Implementations** (`crates/systems/`):
- Each system combines core components with system-specific logic
- See individual [system READMEs](#for-developers) for implementation details

### Renderer Architecture

The project uses a modular renderer architecture across multiple systems for consistency and future GPU acceleration support. See [ARCHITECTURE.md](docs/ARCHITECTURE.md#renderer-architecture) for implementation details.

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

See [MANUAL.md](docs/MANUAL.md) for user-facing mapper information and game compatibility.

## Supported ROM Formats

| System | Format | Detection Method | Status | Notes |
|--------|--------|------------------|--------|-------|
| **NES** | iNES (.nes) | Header signature | ✅ Fully supported | ~90% game coverage |
| **Atari 2600** | Raw binary (.a26, .bin) | File size | ✅ Fully supported | 2K-32K ROMs |
| **Game Boy** | GB/GBC (.gb, .gbc) | Nintendo logo | ✅ Fully Functional | MBC0/1/2/3/5, HuC1; ~97% compatible |
| **SNES** | SMC/SFC (.smc, .sfc) | Header detection | 🚧 Basic | LoROM only, minimal PPU |
| **N64** | Z64/N64/V64 (.z64, .n64, .v64) | Magic byte + conversion | 🚧 In development | Byte-order auto-detection |
| **PC/DOS** | COM/EXE (.com, .exe) | MZ header or size | 🧪 Experimental | CGA/EGA/VGA modes |

## Video Processing System

Hemulator features a modular video processing architecture that supports multiple rendering backends:

### Software Renderer (Default)
- **CPU-based rendering**: Uses traditional software rendering for maximum compatibility
- **CRT Filters**: CPU-based implementation of CRT effects
- **Cross-platform**: Works on all systems without GPU requirements
- **Default backend**: Automatically selected if OpenGL is unavailable

### OpenGL Renderer (Optional)
- **Hardware-accelerated**: Utilizes GPU for faster rendering and effects
- **Shader-based filters**: CRT effects implemented as GLSL shaders for superior performance
- **Available effects**:
  - **None**: Direct pixel output
  - **Scanlines**: Horizontal scan lines effect (darkens every other line)
  - **Phosphor**: Horizontal color bleeding/glow between pixels
  - **CRT Monitor**: Combines scanlines and phosphor for authentic CRT appearance
- **Dynamic switching**: Shaders are compiled and switched on-the-fly based on selected filter

### Building with OpenGL Support

```bash
# Build with OpenGL support enabled
cargo build --release --features opengl

# Or run directly with OpenGL
cargo run --release --features opengl -p emu_gui
```

### Architecture
The video processing system uses the `VideoProcessor` trait to abstract rendering backends:
- `SoftwareProcessor`: CPU-based rendering using the existing CRT filter code
- `OpenGLProcessor`: GPU-accelerated rendering using GLSL shaders
- Both backends implement the same interface, allowing seamless switching
- Configuration stored in `config.json` (`video_backend`: "software" or "opengl")

### Shader Files
GLSL shaders are located in `crates/frontend/gui/src/shaders/`:
- `vertex.glsl`: Vertex shader (fullscreen quad)
- `fragment_none.glsl`: Passthrough fragment shader (no filter)
- `fragment_scanlines.glsl`: Scanlines effect
- `fragment_phosphor.glsl`: Phosphor glow effect
- `fragment_crt.glsl`: Combined CRT effect (scanlines + phosphor)

## Project Structure

```
hemulator/
├── crates/
│   ├── core/           # Shared traits and types (System, Frame, save-state)
│   ├── systems/
│   │   ├── nes/        # NES emulation (CPU, PPU, APU, mappers)
│   │   ├── atari2600/  # Atari 2600 emulation (TIA, RIOT, cartridge banking)
│   │   ├── gb/         # Game Boy emulation (CPU, PPU, APU, MBC0/1/2/3/5, HuC1)
│   │   ├── snes/       # SNES emulation (basic infrastructure)
│   │   ├── n64/        # N64 emulation (in development, 3D rendering)
│   │   └── pc/         # IBM PC/XT emulation (experimental)
│   └── frontend/
│       └── gui/        # GUI frontend (SDL2 + rodio) - builds as 'hemu'
├── config.json         # User settings (created on first run)
├── saves/              # Save state directory (per-ROM)
└── AGENTS.md           # Guidelines for automated agents
```

## Development

### Prerequisites

**Linux:**
```bash
sudo apt-get install libasound2-dev cmake pkg-config
```

**Windows/macOS:** No additional dependencies required beyond Rust toolchain.

### Building

```bash
# Development build (fast compile, slower runtime)
cargo build

# Release-quick build (fast compile, good runtime performance)
# Best for iterative development - ~18x faster incremental builds than release
cargo build --profile release-quick

# Release build (slow compile, best runtime performance)
# Use for final testing and distribution
cargo build --release

# Run tests
cargo test --workspace

# Run linting
cargo clippy --workspace --all-targets -- -D warnings

# Format code
cargo fmt --all

# Run benchmarks
cd crates/core
cargo bench
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

### Benchmarking

Performance benchmarks are available using [Criterion.rs](https://github.com/bheisler/criterion.rs):

```bash
# Run all benchmarks in core crate
cd crates/core
cargo bench

# Run specific benchmark
cargo bench cpu_6502

# Save baseline and compare
cargo bench -- --save-baseline my-baseline
cargo bench -- --baseline my-baseline
```

See [CONTRIBUTING.md](docs/CONTRIBUTING.md#benchmarking) for detailed benchmarking guidelines.

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
  - **Atari 2600 (`emu_atari2600`)**: Complete Atari 2600 with TIA, RIOT, and cartridge banking
  - **Game Boy (`emu_gb`)**: Fully functional Game Boy emulator (LR35902 CPU, PPU, APU, MBC0/1/2/3/5, HuC1)
  - **SNES (`emu_snes`)**: Basic SNES infrastructure (CPU working, minimal PPU)
  - **N64 (`emu_n64`)**: N64 in development (CPU, RDP 3D rendering, RSP HLE)
  - **PC (`emu_pc`)**: Experimental IBM PC/XT emulator with 8086 CPU and BIOS stub
  
- **Frontend (`emu_gui`)**: GUI application
  - Window management with SDL2
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

Contributions are welcome! Please see **[CONTRIBUTING.md](docs/CONTRIBUTING.md)** for:
- Pre-commit check requirements (formatting, linting, building, testing)
- Development workflow and coding standards
- Debug environment variables
- Areas where contributions are needed

For architecture details and implementation guidelines, see **[AGENTS.md](AGENTS.md)**.

## License

Hemulator is open source software licensed under the MIT License. See the [LICENSE](LICENSE) file for details.

## Project Maintainers

Hemulator is founded and maintained by:
- **[@Hexagon](https://github.com/Hexagon)**
- **[@Oliodh](https://github.com/Oliodh)**

This is a free and open source community project. Contributions from the community are crucial and invaluable to the project's success.

## Acknowledgments

- Built with [SDL2](https://www.libsdl.org/) for cross-platform windowing and rendering
- Audio playback via [rodio](https://github.com/RustAudio/rodio)
- NES mapper references from [NESDev Wiki](https://www.nesdev.org/)

---

**Note**: This emulator is for educational purposes. Users must provide their own legally obtained ROM files. The project does not include or distribute any copyrighted game data.

### Debug Logging

The emulator supports comprehensive debug logging via command-line flags. See [CONTRIBUTING.md](docs/CONTRIBUTING.md#debug-logging) for detailed usage.

Quick reference:
```bash
# Enable CPU debug logging
cargo run --release -- --log-cpu debug game.nes

# Enable multiple categories
cargo run --release -- --log-cpu debug --log-interrupts info game.nes

# Set global log level
cargo run --release -- --log-level trace game.nes
```

Available log categories: `--log-cpu`, `--log-bus`, `--log-ppu`, `--log-apu`, `--log-interrupts`, `--log-stubs`

Log levels: `off`, `error`, `warn`, `info`, `debug`, `trace`
