---
title: "Download & Install"
description: "Download Hemulator - Multi-system console emulator for Windows and Linux. Get the latest release and start playing retro games."
keywords: "download hemulator, install emulator, hemulator releases, windows emulator, linux emulator"
nav_order: 2
---

# Download & Install

Get the latest version of Hemulator for your platform.

---

## Latest Release

**[📦 Download Latest Release](https://github.com/Hexagon/hemulator/releases/latest)**

Visit the [GitHub Releases page](https://github.com/Hexagon/hemulator/releases) to download the latest version of Hemulator.

## Platform-Specific Downloads

### Windows

**64-bit (Recommended)**:
- Download `hemu-{version}-windows-x86_64.zip`
- Extract the archive
- Run `hemu.exe`

**32-bit**:
- Download `hemu-{version}-windows-i686.zip`
- Extract the archive
- Run `hemu.exe`

**Included Files**:
- `hemu.exe` - Main emulator executable
- `SDL2.dll` - Required SDL2 library
- `LICENSE` - License information
- `README.md` - Project overview and quick start

### Linux

**64-bit Binary (Recommended)**:
- Download `hemu-{version}-linux-x86_64.tar.gz`
- Extract: `tar -xzf hemu-{version}-linux-x86_64.tar.gz`
- Run: `./hemu`

**32-bit Binary**:
- Download `hemu-{version}-linux-i686.tar.gz`
- Extract: `tar -xzf hemu-{version}-linux-i686.tar.gz`
- Run: `./hemu`

**Debian Package (64-bit)**:
- Download `hemu_{version}_amd64.deb`
- Install: `sudo dpkg -i hemu_{version}_amd64.deb`
- Run: `hemu`

**Debian Package (32-bit)**:
- Download `hemu_{version}_i386.deb`
- Install: `sudo dpkg -i hemu_{version}_i386.deb`
- Run: `hemu`

## System Requirements

### Minimum Requirements
- **OS**: Windows 7+ or Linux (kernel 3.2+)
- **CPU**: Dual-core processor (2 GHz+)
- **RAM**: 512 MB
- **Graphics**: OpenGL 2.1+ support
- **Storage**: 50 MB free space

### Recommended Requirements
- **OS**: Windows 10/11 or modern Linux distribution
- **CPU**: Quad-core processor (3 GHz+)
- **RAM**: 2 GB
- **Graphics**: OpenGL 3.3+ or modern GPU
- **Storage**: 100 MB free space

## Building from Source

For developers who want to build from source, see the [README](https://github.com/Hexagon/hemulator/blob/main/README.md) for build instructions.

### Quick Build Steps

```bash
# Clone the repository
git clone https://github.com/Hexagon/hemulator.git
cd hemulator

# Build with optimized profile
cargo build --profile release-quick

# Run
cargo run --profile release-quick -- path/to/rom.nes
```

For detailed build instructions, dependencies, and development setup, see:
- **[README.md](https://github.com/Hexagon/hemulator/blob/main/README.md)** - Build instructions
- **[Contributing Guide](developer/contributing.html)** - Development workflow

## Getting Started

After installation:

1. **Launch Hemulator** - Double-click the executable
2. **Load a ROM** - Press `Ctrl+O` or use **File > Open ROM**
3. **Start Playing** - Use keyboard controls (see [User Manual](user/index.html))

## Need Help?

- **📖 [User Manual](user/index.html)** - Complete usage guide
- **🐛 [Report Issues](https://github.com/Hexagon/hemulator/issues)** - Bug reports and feature requests
- **💬 [Discussions](https://github.com/Hexagon/hemulator/discussions)** - Community support

## License

Hemulator is free and open source software. See the [LICENSE](https://github.com/Hexagon/hemulator/blob/main/LICENSE) for details.

**Educational Purpose**: This emulator is for educational purposes. Users must provide their own legally obtained ROM files.
