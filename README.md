# Hemulator — Multi-System Console Emulator

A cross-platform, multi-system console emulator written in Rust. **NES emulation is fully working** with ~90% game coverage. Other systems (Game Boy, GBA, CHIP-8, SMS, ColecoVision, SG-1000, Atari 2600, SNES, N64, PC/DOS) are in various stages of development.

## Features

- 🎮 **Multiple Systems**: NES, Game Boy, GBA, CHIP-8, SMS, ColecoVision, SG-1000, Atari 2600, SNES, N64, PC/DOS
- 💾 **Save States**: 5 slots per game with menu-based save/load (Ctrl+1-5 / Ctrl+Shift+1-5)
- ⚙️ **Persistent Settings**: Customizable controls, window scaling, and project files
- 🖥️ **Cross-Platform GUI**: Built with SDL2 for Windows, Linux, and macOS
- 🎨 **CRT Filters**: Software and OpenGL-accelerated shader-based effects (scanlines, phosphor, full CRT)
- 🎵 **Audio Support**: Integrated audio playback via rodio
- 📁 **ROM Auto-Detection**: Automatically detects ROM formats and system types
- 🖱️ **Modern Interface**: Menu bar, status bar, Inspector panel for debugging

## System Status

| System | Status | Coverage/Notes |
|--------|--------|----------------|
| **NES** | ✅ Fully Working | ~90% of games via 14 mappers |
| **Game Boy** | ✅ Fully Functional | ~99% of games; MBC0/1/2/3/5, HuC1 |
| **GBA** | 🚧 In Development | Simple games working; ARM7TDMI CPU, PPU, Flash/EEPROM/SRAM saves |
| **CHIP-8** | ✅ Fully Working | CHIP-8/Hires/Super-CHIP/XO-CHIP/Mega-CHIP |
| **SMS** | 🚧 In Development | VDP initialization issue; test ROMs work, real games don't display |
| **ColecoVision** | 🚧 In Development | Not producing image; full hardware emulation |
| **SG-1000** | ⚠️ Experimental | Full hardware emulation |
| **Atari 2600** | 🚧 In Development | Most cartridge formats; rendering WIP |
| **SNES** | 🚧 In Development | Graphics working; modes 0-1 complete; no audio |
| **N64** | 🚧 In Development | 3D rendering works; limited game support |
| **PC/DOS** | ⚠️ Experimental | COM/EXE loading; CGA/EGA/VGA modes |

**Legend:**
- ✅ Fully Working - Production ready
- 🚧 In Development - Partial functionality
- ⚠️ Experimental - Early development

## Documentation

- 📚 **[Complete Documentation Site](https://hemulator.56k.guru)** - User guides, developer references, system details
- 📖 **[User Manual](https://hemulator.56k.guru/user/)** - Controls, configuration, and usage
- 🏗️ **[Architecture Guide](ARCHITECTURE.md)** - System architecture and design patterns
- 🤝 **[Contributing Guide](https://hemulator.56k.guru/developer/contributing.html)** - Development workflow
- 🤖 **[Agent Guidelines](AGENTS.md)** - CI requirements and implementation guidelines

## Quick Start

### For Users

Download the latest release from the [Releases](https://github.com/Hexagon/hemulator/releases) page.

### For Developers

**Linux Dependencies:**
```bash
sudo apt-get install libasound2-dev cmake pkg-config
```

**Build and Run:**
```bash
# Clone the repository
git clone https://github.com/Hexagon/hemulator.git
cd hemulator

# Build (optimized for iterative development)
cargo build --profile release-quick

# Run the emulator
cargo run --profile release-quick -p emu_gui

# Or build for distribution
cargo build --release
./target/release/hemu path/to/game.nes
```

**Pre-commit Checks** (required before committing):
```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo build --profile release-quick
cargo test --workspace
```

## Project Structure

```
hemulator/
├── crates/
│   ├── core/              # Shared CPU implementations, audio/graphics utilities, traits
│   ├── systems/           # System implementations (NES, GB, SNES, N64, PC, etc.)
│   │   ├── nes/           # Nintendo Entertainment System
│   │   ├── gb/            # Game Boy / Game Boy Color
│   │   ├── gba/           # Game Boy Advance
│   │   ├── chip8/         # CHIP-8 variants
│   │   ├── sms/           # Sega Master System
│   │   ├── colecovision/  # ColecoVision
│   │   ├── sg1000/        # Sega SG-1000
│   │   ├── atari2600/     # Atari 2600
│   │   ├── snes/          # Super Nintendo
│   │   ├── n64/           # Nintendo 64
│   │   └── pc/            # IBM PC/XT
│   └── frontend/gui/      # GUI frontend (SDL2 + egui) - builds as 'hemu'
├── docs/                  # Documentation site source (Lumocs)
├── ARCHITECTURE.md        # Architecture documentation
└── AGENTS.md              # Agent/CI guidelines
```

See [ARCHITECTURE.md](ARCHITECTURE.md) for detailed architecture information and the [documentation site](https://hemulator.56k.guru) for system-specific implementation details.

## Contributing

Contributions are welcome! Please see:
- **[Contributing Guide](https://hemulator.56k.guru/developer/contributing.html)** - Development workflow, coding standards, debug tools
- **[AGENTS.md](AGENTS.md)** - CI requirements and agent guidelines
- **[System READMEs](crates/systems/)** - Implementation details for each system

## License

MIT License - see the [LICENSE](LICENSE) file for details.

## Project Maintainers

Founded and maintained by **[@Hexagon](https://github.com/Hexagon)** and **[@Oliodh](https://github.com/Oliodh)**.

This is a free and open source community project. Contributions from the community are crucial and invaluable.

---

**Note**: This emulator is for educational purposes. Users must provide their own legally obtained ROM files. The project does not include or distribute any copyrighted game data.
