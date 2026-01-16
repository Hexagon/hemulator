---
title: "Introduction"
description: "Complete documentation for Hemulator - a cross-platform multi-system console emulator written in Rust. Supports NES, Game Boy, SNES, N64, Atari 2600, SMS, CHIP-8, and PC/DOS emulation."
keywords: "hemulator, emulator, NES, Game Boy, SNES, N64, Atari 2600, retro gaming, Rust, cross-platform, documentation"
nav_order: 1
---

<p align="center">
<img src="https://raw.githubusercontent.com/Hexagon/hemulator/refs/heads/main/assets/icon_256.png" alt="Hemulator" width="150" height="150">
</p>

Hemulator is a modern, modular emulator supporting multiple classic gaming systems including NES, Game Boy, Atari 2600, SNES, N64, Sega Master System, CHIP-8, and PC/DOS.

**[📦 Download Latest Release](https://github.com/Hexagon/hemulator/releases/latest)** | **[📖 Installation Guide](download.md)**

## Documentation Sections

### For Users

- **[Download & Install](download.md)** - Get the latest release for Windows or Linux
- **[User Manual](user/index.md)** - Getting started, controls, features, and system-specific information
- **[System Guides](systems/index.md)** - Detailed information about each emulated system

**Get Help & Contribute**:
- **[🐛 Report a Bug](https://github.com/Hexagon/hemulator/issues/new?template=bug_report.yml)** - Found a problem? Let us know!
- **[🚀 Request a Feature](https://github.com/Hexagon/hemulator/issues/new?template=feature_request.yml)** - Have an idea for a new feature?
- **[💬 Discussions](https://github.com/Hexagon/hemulator/discussions)** - Ask questions and share ideas

### For Developers

- **[Architecture Overview](developer/architecture.md)** - High-level system architecture and design patterns
  - Full details: [ARCHITECTURE.md](https://github.com/Hexagon/hemulator/blob/master/ARCHITECTURE.md) in repository
- **[Contributing](developer/contributing.md)** - Development workflow and contribution guidelines
- **[CPU References](references/index.md)** - Technical reference documentation for CPU implementations

**Development Support**:
- **[🔧 Technical Issue](https://github.com/Hexagon/hemulator/issues/new?template=developer_issue.yml)** - Report build issues, implementation problems, or ask technical questions

## Quick Links

- [GitHub Repository](https://github.com/Hexagon/hemulator)
- [Latest Releases](https://github.com/Hexagon/hemulator/releases)
- [Issue Tracker](https://github.com/Hexagon/hemulator/issues)

## Features

- 🎮 **Multiple Systems**: NES, Game Boy, Atari 2600, SNES, N64, SMS, ColecoVision, SG-1000, CHIP-8, PC/DOS - [See all systems](systems/index.md)
- 💾 **Save States**: 5 slots per game with instant save/load
- ⚙️ **Persistent Settings**: Customizable controls and window scaling
- 🖥️ **Cross-Platform**: Windows, Linux, and macOS support
- 🎨 **CRT Filters**: Hardware-accelerated shader-based effects
- 🎵 **Audio Support**: Integrated audio playback for supported systems

## Open Source Community Project

Hemulator is a **free and open source** community effort founded and maintained by GitHub users **[@Hexagon](https://github.com/Hexagon)** and **[@Oliodh](https://github.com/Oliodh)**. This project exists thanks to the passionate developers, contributors, and retro gaming enthusiasts who volunteer their time and expertise.

**🤝 Contributions are Crucial and Invaluable**

Every contribution, no matter how small, helps make Hemulator better:
- Bug reports help us identify and fix issues
- Feature requests guide development priorities
- Code contributions add new features and improve quality
- Documentation improvements help users and developers
- Testing and feedback ensure compatibility and usability

**We deeply value every contribution and contributor** - from first-time contributors to long-time maintainers. If you're interested in contributing, see our [Contributing Guide](developer/contributing.md).

**💝 Support the Project**

If you find Hemulator useful and want to support its development, consider sponsoring the maintainers:
- **[Sponsor @Hexagon on GitHub](https://github.com/sponsors/Hexagon)** - Your support helps sustain development and maintenance

All donations go directly to supporting the developers who volunteer their time to make Hemulator better.

## Development Resources

Hemulator's development relies on excellent documentation and resources from the emulation community:

**Technical Documentation**:
- [NESDev Wiki](https://www.nesdev.org/) - Comprehensive NES hardware documentation
- [Pan Docs](https://gbdev.io/pandocs/) - Game Boy technical reference
- [SMS Power!](https://www.smspower.org/) - Sega Master System documentation
- [SNESdev Wiki](https://snes.nesdev.org/) - SNES hardware documentation
- [N64brew](https://n64brew.dev/) - Nintendo 64 development resources
- [OSDev Wiki](https://wiki.osdev.org/) - PC/x86 hardware documentation

**Datasheets & Specifications**:
- CPU datasheets (6502, Z80, 65C816, MIPS, x86)
- Video and audio chip specifications
- Original system manuals and developer documentation

**Community Resources**:
- Emulation development forums and Discord servers
- Test ROMs and validation suites
- Open source emulator projects for reference

We are grateful to all these communities and resources that make emulation development possible. See our [CPU & Hardware References](references/) section for detailed source attribution.

## License

Hemulator is open source software. See the [LICENSE](https://github.com/Hexagon/hemulator/blob/master/LICENSE) file for details.

## Educational Purpose

This emulator is for educational purposes. Users must provide their own legally obtained ROM files. The project does not include or distribute any copyrighted game data.
