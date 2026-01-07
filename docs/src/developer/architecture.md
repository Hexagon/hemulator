---
title: "Architecture Overview"
description: "High-level architecture overview of Hemulator's modular emulation system - CPU implementations, audio components, graphics utilities, and system designs."
keywords: "hemulator architecture, emulator design, CPU implementation, modular architecture, system design"
parent: "Developer"
nav_order: 1
---

# Hemulator Architecture Overview

> **📄 For complete architecture documentation, see [ARCHITECTURE.md](https://github.com/Hexagon/hemulator/blob/master/ARCHITECTURE.md) in the repository root**

This page provides a high-level overview of Hemulator's architecture. For detailed implementation information, system design patterns, and technical specifics, please refer to the complete [ARCHITECTURE.md](https://github.com/Hexagon/hemulator/blob/master/ARCHITECTURE.md) in the repository.

## System Architecture

Hemulator is built on a modular architecture that separates reusable emulation components from system-specific implementations:

```
┌────────────────────────────────────────────────────────────────┐
│                         Frontend (GUI)                         │
│  • Window Management (SDL2)                                    │
│  • Audio Playback (rodio)                                      │
│  • Input Handling                                              │
│  • Settings Management                                         │
└────────────────────┬───────────────────────────────────────────┘
                     │ System Trait
┌────────────────────┴───────────────────────────────────────────┐
│                      System Implementations                    │
│  NES │ GB/GBC │ Atari 2600 │ SNES │ N64 │ PC │ SMS │ CHIP-8   │
└────────────────────┬───────────────────────────────────────────┘
                     │ Uses
┌────────────────────┴───────────────────────────────────────────┐
│                      Core Components                           │
│  • CPU Implementations                                         │
│  • Audio Components                                            │
│  • Graphics Utilities                                          │
│  • Common Traits                                               │
└────────────────────────────────────────────────────────────────┘
```

## Core Components

**CPU Implementations**: Reusable CPU cores shared across systems
- 6502 (NES, Atari 2600)
- 65C816 (SNES)
- LR35902 (Game Boy)
- Z80 (SMS)
- MIPS R4300i (N64)
- 8086/80386 (PC)

**Audio Components**: Shared audio processing
- APU channels (pulse, triangle, noise, DMC)
- Envelopes and mixers
- AudioChip trait

**Graphics**: Common graphics utilities
- Renderer trait for software/hardware rendering
- ZBuffer and color operations
- Palette and tile utilities

## System Implementations

Each system (`crates/systems/*/`) combines:
- Core CPU implementation
- System-specific memory bus
- Graphics processor (PPU/VDP/TIA/etc.)
- Audio processor (APU/PSG/etc.)
- Input handling
- Cartridge/ROM management

**System Documentation**:
- [System Overview](../systems/) - Status and features for each system
- Individual system READMEs in `crates/systems/*/README.md`

## Key Design Patterns

### Modular Architecture
- **Separation of concerns**: Core components vs system-specific logic
- **Code reuse**: Share CPU implementations across systems
- **Testability**: Independent testing of components

### Trait-Based Design
- `System` trait for emulator implementations
- `Cpu` trait for processor cores
- `Renderer` trait for graphics backends
- `AudioChip` trait for audio processors

### Memory Management
- System-specific memory bus implementations
- Generic memory traits for CPU cores
- Efficient memory mapping and banking

## Further Reading

For complete details including:
- Detailed component architecture
- Implementation guidelines
- Audio and renderer patterns
- Memory management specifics
- System-specific architectures

Please refer to the **[complete ARCHITECTURE.md](https://github.com/Hexagon/hemulator/blob/master/ARCHITECTURE.md)** in the repository.

**Related Documentation**:
- [Contributing Guide](contributing.md) - Development workflow
- [System Documentation](../systems/) - Individual system details
- [CPU References](../references/) - CPU implementation references
