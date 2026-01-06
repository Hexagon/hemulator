---
title: "Hemulator Documentation"
nav_order: 1
---

<p align="center">
<img src="https://raw.githubusercontent.com/Hexagon/hemulator/master/assets/icon.png" alt="Hemulator" width="150" height="150"><br>
A cross-platform multi-system console emulator written in Rust
</p>

## Welcome

Hemulator is a modern, modular emulator supporting multiple classic gaming systems including NES, Game Boy, Atari 2600, SNES, N64, Sega Master System, CHIP-8, and PC/DOS.

## Documentation Sections

### For Users

- **[User Manual](user/manual.md)** - Getting started, controls, features, and system-specific information
- **[System Guides](systems/)** - Detailed information about each emulated system

**Get Help & Contribute**:
- **[🐛 Report a Bug](https://github.com/Hexagon/hemulator/issues/new?template=bug_report.yml)** - Found a problem? Let us know!
- **[🚀 Request a Feature](https://github.com/Hexagon/hemulator/issues/new?template=feature_request.yml)** - Have an idea for a new feature?
- **[💬 Discussions](https://github.com/Hexagon/hemulator/discussions)** - Ask questions and share ideas

### For Developers

- **[Architecture Overview](developer/architecture.md)** - High-level system architecture and design patterns
  - Full details: [ARCHITECTURE.md](https://github.com/Hexagon/hemulator/blob/master/ARCHITECTURE.md) in repository
- **[Contributing](developer/contributing.md)** - Development workflow and contribution guidelines
- **[CPU References](references/)** - Technical reference documentation for CPU implementations

**Development Support**:
- **[🔧 Technical Issue](https://github.com/Hexagon/hemulator/issues/new?template=developer_issue.yml)** - Report build issues, implementation problems, or ask technical questions

## Quick Links

- [GitHub Repository](https://github.com/Hexagon/hemulator)
- [Latest Releases](https://github.com/Hexagon/hemulator/releases)
- [Issue Tracker](https://github.com/Hexagon/hemulator/issues)

## Features

- 🎮 **Multiple Systems**: NES, Game Boy, Atari 2600, SNES, N64, SMS, CHIP-8, PC/DOS
- 💾 **Save States**: 5 slots per game with instant save/load
- ⚙️ **Persistent Settings**: Customizable controls and window scaling
- 🖥️ **Cross-Platform**: Windows, Linux, and macOS support
- 🎨 **CRT Filters**: Hardware-accelerated shader-based effects
- 🎵 **Audio Support**: Integrated audio playback for supported systems

## System Status

| System | Status | CPU | Graphics | Audio | Coverage |
|--------|--------|-----|----------|-------|----------|
| **NES** | ✅ Fully Working | 6502 | PPU | APU | ~90% of games |
| **Game Boy** | ✅ Fully Functional | LR35902 | PPU | APU | ~99% of games |
| **CHIP-8** | ✅ Fully Working | VM | Multi-mode | Beep | Complete |
| **SMS** | ✅ Functional | Z80 | VDP | PSG | Testing needed |
| **Atari 2600** | 🚧 In Development | 6502 | TIA | TIA | Rendering WIP |
| **PC/DOS** | ⚠️ Experimental | 8086-80386 | CGA/EGA/VGA | ❌ | Basic support |
| **SNES** | 🚧 In Development | 65C816 | PPU | SPC700 | No audio output |
| **N64** | 🚧 In Development | R4300i | RDP/RSP | ❌ | Limited support |

**Legend:**
- ✅ Production ready - Comprehensive features and game coverage
- ⚠️ Functional - Core features work, missing some capabilities
- 🚧 In Development - Partial functionality, active work
- ❌ Not implemented

## License

Hemulator is open source software. See the [LICENSE](https://github.com/Hexagon/hemulator/blob/master/LICENSE) file for details.

## Educational Purpose

This emulator is for educational purposes. Users must provide their own legally obtained ROM files. The project does not include or distribute any copyrighted game data.
