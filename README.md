# Hemulator — Multi-System Console Emulator

A cross-platform, multi-system console emulator written in Rust. **NES emulation is fully working** with ~90% game coverage. **GBA emulation is functional** with most games working. 16 systems are supported in various stages of development — see [System Status](#system-status) below.

## Features

- 🎮 **Multiple Systems**: 16 emulated platforms — see [System Status](#system-status) for details
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
| **GBA** | ✅ Functional | Most games working; ARM7TDMI CPU, PPU, Flash/EEPROM/SRAM saves |
| **CHIP-8** | ✅ Fully Working | CHIP-8/Hires/Super-CHIP/XO-CHIP/Mega-CHIP |
| **SMS** | ✅ Fully Working | Z80 CPU, VDP, PSG, ROM banking; ~90% game compatibility |
| **Game Gear** | ✅ Fully Working | SMS-compatible; 160×144 viewport, 12-bit CRAM, GG I/O ports |
| **Game & Watch** | 🚧 In Development | SM510 CPU, LCD artwork rendering, .mgw container support |
| **ColecoVision** | 🚧 In Development | Not producing image; full hardware emulation |
| **SG-1000** | ⚠️ Experimental | Full hardware emulation |
| **Atari 2600** | 🚧 In Development | Most cartridge formats; rendering WIP |
| **Atari 5200** | 🚧 In Development | 6502C CPU, ANTIC, GTIA, POKEY; early development |
| **Mega Drive** | 🚧 In Development | M68000 CPU, VDP, YM2612 FM audio, SN76489 PSG; early development |
| **SNES** | ✅ Functional | All PPU modes 0-7, sprites, DMA/HDMA, SPC700+DSP audio |
| **N64** | 🚧 In Development | 3D rendering works; limited game support |
| **PS1** | 🚧 In Development | MIPS R3000A CPU, GPU (2D/3D), DMA, timers; Audio/CD-ROM WIP |
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

### ARM / Raspberry Pi 5

The emulator supports ARM64 (aarch64) platforms including Raspberry Pi 5. 

**Pre-built ARM packages** are available from GitHub Actions or can be built locally:

```bash
# Option 1: Build natively on ARM device (Raspberry Pi 5, etc.)
cargo build --profile release-quick

# Option 2: Cross-compile from x86_64 Linux using 'cross'
cargo install cross --git https://github.com/cross-rs/cross
cross build --profile release-quick --target aarch64-unknown-linux-gnu

# Option 3: Use GitHub Actions workflow
# ARM64 packages (.tar.gz and .deb) are automatically built on tagged releases
```

**Dependencies on Raspberry Pi OS:**
```bash
sudo apt-get install libasound2-dev cmake pkg-config libsdl2-dev
```

The ARM build produces both a standalone binary (`hemu`) and a Debian package (`hemu_*_arm64.deb`) for easy installation.

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
│   │   ├── atari5200/     # Atari 5200
│   │   ├── gameandwatch/  # Game & Watch (SM510)
│   │   ├── megadrive/     # Sega Mega Drive / Genesis
│   │   ├── snes/          # Super Nintendo
│   │   ├── n64/           # Nintendo 64
│   │   ├── ps1/           # Sony PlayStation 1
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
