---
title: "System Documentation"
nav_order: 2
---

# Emulated Systems

Hemulator supports multiple classic gaming systems. Each system has its own implementation with detailed technical documentation.

## System Status Overview

| System | Status | CPU | Graphics | Audio | Coverage |
|--------|--------|-----|----------|-------|----------|
| [NES](#nes) | ✅ Fully Working | 6502 | PPU | APU | ~90% of games |
| [Game Boy](#game-boy) | ✅ Fully Functional | LR35902 | PPU | APU | ~99% of games |
| [CHIP-8](#chip-8) | ✅ Fully Working | VM | Multi-mode | Beep | Complete |
| [SMS](#sega-master-system) | ✅ Functional | Z80 | VDP | PSG | Testing needed |
| [Atari 2600](#atari-2600) | 🚧 In Development | 6502 | TIA | TIA | Rendering WIP |
| [PC/DOS](#pcdos) | ⚠️ Experimental | 8086-80386 | CGA/EGA/VGA | ❌ | Basic support |
| [SNES](#snes) | 🚧 In Development | 65C816 | PPU | SPC700 | No audio output |
| [N64](#nintendo-64) | 🚧 In Development | R4300i | RDP/RSP | ❌ | Limited support |

## System Details

### NES

**Nintendo Entertainment System** - Fully working with ~90% game coverage through 14 mapper implementations.

- **Architecture**: [NES README](https://github.com/Hexagon/hemulator/blob/master/crates/systems/nes/README.md)
- **Status**: Production ready
- **Supported Formats**: iNES (.nes)

**Key Features**:
- Complete 6502 CPU from `emu_core::cpu_6502`
- Full PPU emulation with background, sprites, scrolling
- Complete APU with all 5 channels
- 14 mappers covering ~90%+ of games
- PAL/NTSC auto-detection

### Game Boy

**Game Boy / Game Boy Color** - Fully functional with ~99% game coverage.

- **Architecture**: [Game Boy README](https://github.com/Hexagon/hemulator/blob/master/crates/systems/gb/README.md)
- **Verification**: [Verification Guide](https://github.com/Hexagon/hemulator/blob/master/crates/systems/gb/VERIFICATION.md)
- **Status**: Production ready
- **Supported Formats**: .gb, .gbc

**Key Features**:
- Complete LR35902 CPU
- Full PPU with DMG and CGB modes
- Complete APU with all 4 channels
- MBC support: MBC0, MBC1, MBC2, MBC3, MBC5, HuC1
- WRAM banking for GBC
- Cartridge battery saves

### CHIP-8

**CHIP-8 / Super-CHIP / XO-CHIP / Mega-CHIP** - Fully working with complete compatibility.

- **Architecture**: [CHIP-8 README](https://github.com/Hexagon/hemulator/blob/master/crates/systems/chip8/README.md)
- **Status**: Production ready
- **Supported Formats**: .ch8, .sc8, .xo8, .mc8

**Key Features**:
- Complete CHIP-8 VM implementation
- Multiple display modes (64x32, 64x64, 128x64, 256x192)
- XO-CHIP extended instruction set
- Mega-CHIP high-resolution mode
- Beep timer for audio

### Sega Master System

**Sega Master System** - Functional with full hardware emulation, needs game testing.

- **Architecture**: [SMS README](https://github.com/Hexagon/hemulator/blob/master/crates/systems/sms/README.md)
- **Status**: Functional
- **Supported Formats**: .sms

**Key Features**:
- Complete Z80 CPU
- Full VDP implementation
- Complete PSG (SN76489) audio
- Cartridge banking support
- Save state support

### Atari 2600

**Atari 2600** - In development with rendering issues.

- **Architecture**: [Atari 2600 README](https://github.com/Hexagon/hemulator/blob/master/crates/systems/atari2600/README.md)
- **Compliance**: [Spec Compliance](https://github.com/Hexagon/hemulator/blob/master/crates/systems/atari2600/SPEC_COMPLIANCE.md)
- **Timing**: [Timing Analysis](https://github.com/Hexagon/hemulator/blob/master/crates/systems/atari2600/TIMING_ANALYSIS.md)
- **Status**: In development
- **Supported Formats**: .a26, .bin

**Key Features**:
- Complete 6502/6507 CPU
- TIA graphics (partial - rendering issues)
- TIA audio (complete)
- Multiple cartridge formats (2K-32K)
- Banking schemes: F8, F6, F4, FE, E0, 3F

### PC/DOS

**IBM PC/XT** - Experimental with CGA/EGA/VGA support.

- **Architecture**: [PC README](https://github.com/Hexagon/hemulator/blob/master/crates/systems/pc/README.md)
- **Status**: Experimental
- **Supported Formats**: .com, .exe

**Key Features**:
- 8086/80186/80286/80386 CPU (16-bit complete, 32-bit in progress)
- CGA, EGA, and VGA video adapters
- Text and graphics modes
- BIOS interrupt support
- Disk image support (.img files)
- .hemu configuration files for multi-disk setups

### SNES

**Super Nintendo Entertainment System** - In development with graphics working but no audio output.

- **Architecture**: [SNES README](https://github.com/Hexagon/hemulator/blob/master/crates/systems/snes/README.md)
- **Status**: In development
- **Supported Formats**: .smc, .sfc

**Key Features**:
- Complete 65C816 CPU
- PPU modes 0-7 (modes 0-1 complete)
- Complete SPC700 APU (no DSP)
- LoROM support
- Graphics working, silent gameplay

### Nintendo 64

**Nintendo 64** - In development with 3D rendering functionality.

- **Architecture**: [N64 README](https://github.com/Hexagon/hemulator/blob/master/crates/systems/n64/README.md)
- **Status**: [N64 Status Report](../developer/n64-status.md)
- **Supported Formats**: .z64, .n64, .v64

**Key Features**:
- Complete MIPS R4300i CPU
- RDP 3D graphics processor
- RSP (partial)
- Basic 3D rendering
- Byte-order auto-detection

## Planning Documents

For developers interested in future system implementations:

- [Next Emulator Recommendation](../developer/next-emulator.md) - Analysis of which system to implement next
- [SMS Implementation Guide](../developer/sms-guide.md) - Practical guide for SMS implementation

## Related Documentation

- [User Manual](../user/manual.md) - Controls and usage information
- [Architecture](../developer/architecture.md) - Overall system architecture
- [CPU References](../references/) - Technical CPU documentation
