# Hemulator — Multi-System Console Emulator

A cross-platform, multi-system console emulator written in Rust, supporting NES, Atari 2600, Game Boy, SNES, N64, and PC emulation with comprehensive save state management and customizable controls.

## Features

- 🎮 **NES Emulation**: ✅ Fully working - ~90%+ of NES games via 14 mapper implementations
- 🕹️ **Atari 2600 Emulation**: ✅ Fully working - Most cartridge formats (2K-32K) with multiple banking schemes
- 🎲 **Game Boy Emulation**: ⚠️ Functional - Core features work, ~95% game coverage (MBC0/1/3/5), missing audio/timer
- 🏰 **SNES Emulation**: 🚧 Basic infrastructure - CPU working, minimal PPU, no APU/input yet
- 🎮 **N64 Emulation**: 🚧 In development - 3D rendering functional, limited game support
- 💻 **PC Emulation**: 🧪 Experimental - COM/EXE loading, black screen only
- 💾 **Save States**: 5 slots per game with instant save/load
- ⚙️ **Persistent Settings**: Customizable controls, window scaling, and auto-restore last ROM
- 🖥️ **Cross-Platform GUI**: Built with minifb for Windows, Linux, and macOS
- 🎨 **Video Processing**: Modular architecture supporting both software and OpenGL-accelerated rendering
- 🎞️ **CRT Filters**: Hardware-accelerated shader-based CRT effects (scanlines, phosphor, full CRT)
- 🎵 **Audio Support**: Integrated audio playback via rodio (NES and Atari 2600 audio implemented)
- 📁 **ROM Auto-Detection**: Automatically detects NES (iNES), Atari 2600, Game Boy, SNES, N64, and DOS executable formats

## System Implementation Status

| System | Status | CPU | Graphics | Audio | Input | Save States | Coverage/Notes |
|--------|--------|-----|----------|-------|-------|-------------|----------------|
| **NES** | ✅ Fully Working | 6502 (Complete) | PPU (Complete) | APU (Complete) | ✅ | ✅ | ~90% of all games via 14 mappers |
| **Atari 2600** | ✅ Fully Working | 6502/6507 (Complete) | TIA (Functional) | TIA (Complete) | ✅ | ✅ | Most cartridge formats (2K-32K) |
| **Game Boy** | ⚠️ Functional | LR35902 (Complete) | PPU (Complete) | APU (Not integrated) | ✅ | ✅ | ~95% of games; MBC0/1/3/5 supported; no audio/timer |
| **SNES** | 🚧 Basic | 65C816 (Complete) | PPU (Minimal) | ❌ Not implemented | ❌ | ✅ | Infrastructure only; minimal rendering |
| **N64** | 🚧 In Development | R4300i (Complete) | RDP/RSP (Partial) | ❌ Not implemented | ⚠️ Ready (not integrated) | ✅ | 3D rendering works; limited game support |
| **PC (DOS)** | 🧪 Experimental | 8086 (Partial) | VGA (Stub) | ❌ Not implemented | ⚠️ Keyboard passthrough | ❌ | COM/EXE loading; black screen only |

**Legend:**
- ✅ Fully Working - Production ready with comprehensive features
- ⚠️ Functional - Core features work but missing some capabilities
- 🚧 In Development - Active work in progress with partial functionality
- 🧪 Experimental - Proof of concept or early stage
- ❌ Not implemented - Component not yet available

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

| System | Format | Detection Method | Status | Notes |
|--------|--------|------------------|--------|-------|
| **NES** | iNES (.nes) | Header signature | ✅ Fully supported | ~90% game coverage |
| **Atari 2600** | Raw binary (.a26, .bin) | File size | ✅ Fully supported | 2K-32K ROMs |
| **Game Boy** | GB/GBC (.gb, .gbc) | Nintendo logo | ⚠️ Functional | No audio, ~95% compatible |
| **SNES** | SMC/SFC (.smc, .sfc) | Header detection | 🚧 Basic | LoROM only, minimal PPU |
| **N64** | Z64/N64/V64 (.z64, .n64, .v64) | Magic byte + conversion | 🚧 In development | Byte-order auto-detection |
| **PC/DOS** | COM/EXE (.com, .exe) | MZ header or size | 🧪 Experimental | Black screen only |

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
│   │   ├── nes/        # ✅ NES emulation (CPU, PPU, APU, mappers)
│   │   ├── atari2600/  # ✅ Atari 2600 emulation (TIA, RIOT, cartridge banking)
│   │   ├── gb/         # ⚠️ Game Boy emulation (functional, no audio)
│   │   ├── snes/       # 🚧 SNES emulation (basic infrastructure)
│   │   ├── n64/        # 🚧 N64 emulation (in development, 3D rendering)
│   │   └── pc/         # 🧪 IBM PC/XT emulation (experimental)
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
  - **NES (`emu_nes`)**: ✅ Complete NES emulator with CPU, PPU, APU, and 14 mappers
  - **Atari 2600 (`emu_atari2600`)**: ✅ Complete Atari 2600 with TIA, RIOT, and cartridge banking
  - **Game Boy (`emu_gb`)**: ⚠️ Functional Game Boy emulator (MBC0/1/3/5, no audio/timer)
  - **SNES (`emu_snes`)**: 🚧 Basic SNES infrastructure (CPU working, minimal PPU)
  - **N64 (`emu_n64`)**: 🚧 N64 in development (CPU, RDP 3D rendering, RSP HLE)
  - **PC (`emu_pc`)**: 🧪 Experimental IBM PC/XT emulator with 8086 CPU and BIOS stub
  
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
