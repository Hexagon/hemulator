# Hemulator Architecture

This document describes the overall architecture of the Hemulator multi-system console emulator.

**Related Documentation**:
- **[README.md](README.md)**: Developer quick start, build instructions, project overview
- **[MANUAL.md](MANUAL.md)**: End-user manual with controls, features, and system-specific information
- **[CONTRIBUTING.md](CONTRIBUTING.md)**: Contribution workflow, pre-commit checks, coding standards
- **[AGENTS.md](AGENTS.md)**: Implementation guidelines for automated agents and CI

**System-Specific Details**:
- **[NES](crates/systems/nes/README.md)**: Nintendo Entertainment System implementation
- **[Game Boy](crates/systems/gb/README.md)**: Game Boy / Game Boy Color implementation
- **[Atari 2600](crates/systems/atari2600/README.md)**: Atari 2600 implementation
- **[SNES](crates/systems/snes/README.md)**: Super Nintendo Entertainment System implementation
- **[N64](crates/systems/n64/README.md)**: Nintendo 64 implementation
- **[PC](crates/systems/pc/README.md)**: IBM PC/XT implementation

---

## Overview

Hemulator is built on a modular architecture that separates reusable emulation components from system-specific implementations. This design enables:

- **Code Reuse**: Common CPU implementations, audio components, and graphics utilities shared across systems
- **Consistency**: Unified interfaces and patterns across all emulated systems
- **Testability**: Independent testing of core components and system implementations
- **Extensibility**: Easy addition of new systems by composing existing components

## High-Level Architecture

```
┌────────────────────────────────────────────────────────────────┐
│                         Frontend (GUI)                         │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │  • Window Management (minifb)                            │  │
│  │  • Audio Playback (rodio)                                │  │
│  │  • Input Handling (keyboard, future gamepad support)     │  │
│  │  • Settings Management (config.json)                     │  │
│  │  • Video Processing (CRT filters, scaling)               │  │
│  │  • Save State Management                                 │  │
│  └──────────────────────────────────────────────────────────┘  │
└────────────────────┬───────────────────────────────────────────┘
                     │ System Trait
┌────────────────────┴───────────────────────────────────────────┐
│                      System Implementations                    │
│  ┌──────────┬──────────┬──────────┬──────────┬──────────┬─────┤
│  │   NES    │  GB/GBC  │ Atari    │  SNES    │   N64    │ PC  │
│  │          │          │  2600    │          │          │     │
│  └──────────┴──────────┴──────────┴──────────┴──────────┴─────┤
└────────────────────┬───────────────────────────────────────────┘
                     │ Uses
┌────────────────────┴───────────────────────────────────────────┐
│                      Core Components                           │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │  • CPU Implementations (6502, Z80, LR35902, 65C816,      │  │
│  │    MIPS R4300i, 8086, 8080)                              │  │
│  │  • Audio Components (APU channels, envelopes, mixers)    │  │
│  │  • Graphics Utilities (ZBuffer, ColorOps, palettes)      │  │
│  │  • Common Traits (System, Cpu, Renderer, AudioChip)      │  │
│  │  • Data Structures (Frame, AudioSample)                  │  │
│  └──────────────────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────────────────┘
```

## Core Module (`crates/core/`)

The core module provides reusable components that multiple systems can share.

### CPU Implementations

Hemulator implements several CPU architectures as generic components:

- **`cpu_6502`**: MOS 6502 (NES, Atari 2600, Apple II, Commodore 64)
  - Complete instruction set with all addressing modes
  - Hardware interrupt support (NMI, IRQ)
  - Generic `Memory6502` trait for system-specific memory implementations
  
- **`cpu_65c816`**: WDC 65C816 (SNES, Apple IIGS)
  - 16-bit extension of 6502
  - 256/256 opcodes implemented (100% complete)
  - 8/16-bit mode switching, 24-bit address space
  
- **`cpu_lr35902`**: Sharp LR35902 (Game Boy, Game Boy Color)
  - Z80-like CPU with Game Boy-specific modifications
  - 8-bit and 16-bit register operations
  
- **`cpu_z80`**: Zilog Z80 (Sega Master System, Game Gear, ZX Spectrum)
  - Shadow registers and index registers
  - Multiple interrupt modes
  
- **`cpu_mips_r4300i`**: MIPS R4300i (Nintendo 64)
  - 64-bit MIPS III RISC processor
  - Complete instruction set including FPU operations
  
- **`cpu_8086`**: Intel 8086 (IBM PC, PC XT)
  - Segment-based memory addressing
  - Complete instruction set with ModR/M addressing
  
- **`cpu_8080`**: Intel 8080 (Space Invaders, CP/M systems)
  - Foundation for Z80
  - I/O port support

Each CPU implementation follows the same pattern:
1. Generic memory trait (e.g., `Memory6502`, `MemoryMips`)
2. CPU struct with registers and state
3. Instruction execution with cycle-accurate timing
4. Comprehensive unit tests

For implementation details, see `crates/core/src/cpu_*.rs`

### Audio Components (`crates/core/src/apu/`)

Reusable audio building blocks:

- **Waveform Generators**:
  - `PulseChannel`: Square wave with duty cycle control
  - `TriangleChannel`: Triangle wave (NES-style)
  - `WaveChannel`: Programmable waveform playback
  - `NoiseChannel`: Pseudo-random noise (LFSR-based)
  - `PolynomialCounter`: TIA-style waveform generation

- **Modulation Components**:
  - `Envelope`: Volume envelope with decay
  - `LengthCounter`: Automatic note duration
  - `SweepUnit`: Frequency sweep/modulation
  - `FrameCounter`: Timing controller

- **Audio Chip Implementations**:
  - `Rp2a03Apu`: NES NTSC audio (1.789773 MHz)
  - `Rp2a07Apu`: NES PAL audio (1.662607 MHz)

- **AudioChip Trait**: Common interface for pluggable audio systems

### Graphics Components

- **`graphics`** (`crates/core/src/graphics/`):
  - `ZBuffer`: 16-bit depth buffer for 3D rendering
  - `ColorOps`: Color manipulation utilities (ARGB8888)
  
- **`ppu`** (`crates/core/src/ppu/`):
  - `IndexedPalette`: Generic palette trait
  - `TileDecoder`: Tile format decoders (NES 2bpp, Game Boy 2bpp)
  - `RamPalette`: Simple palette storage

- **`renderer`** (`crates/core/src/renderer.rs`):
  - `Renderer` trait: Unified rendering interface
  - Pattern: System (state) → Renderer trait → {Software, Hardware} implementations

### Common Traits

- **`System` trait**: High-level emulator interface
  - `step_frame()`: Execute one frame of emulation
  - `reset()`: Reset system to initial state
  - Mount/unmount media (cartridges, disks)
  - Save state serialization

- **`Cpu` trait**: Generic CPU interface
  - `step()`: Execute one instruction
  - `reset()`: Reset CPU state
  - Register access methods

- **`Renderer` trait**: Graphics rendering interface
  - `get_frame()`: Get current framebuffer
  - `clear()`, `reset()`, `resize()`: Renderer operations
  - Optional hardware acceleration

## System Implementations (`crates/systems/`)

Each system crate combines core components with system-specific logic.

### Current Systems

| System | Status | Description |
|--------|--------|-------------|
| **[NES](crates/systems/nes/README.md)** | ✅ Fully Working | ~90% game coverage, 14 mappers |
| **[Game Boy](crates/systems/gb/README.md)** | ✅ Fully Working | DMG mode, MBC0/1/3/5 support |
| **[Atari 2600](crates/systems/atari2600/README.md)** | ✅ Fully Working | Complete TIA/RIOT emulation |
| **[SNES](crates/systems/snes/README.md)** | 🚧 Basic | CPU complete, minimal PPU |
| **[N64](crates/systems/n64/README.md)** | 🚧 In Development | 3D rendering functional |
| **[PC](crates/systems/pc/README.md)** | 🧪 Experimental | CGA/EGA/VGA modes, basic BIOS |

For detailed implementation information, see each system's README.md file.

### System Architecture Pattern

Each system follows a consistent architecture:

```
SystemStruct
  ├── CPU (from emu_core)
  │   └── SystemBus (implements Memory trait)
  │       ├── RAM/ROM
  │       ├── Video Hardware (PPU, TIA, RDP, VideoAdapter)
  │       ├── Audio Hardware (APU, TIA audio)
  │       ├── Input/Output
  │       └── System-specific components
  └── Implements System trait
```

Example (NES):
```
NesSystem
  └── NesCpu (wraps Cpu6502<NesMemory>)
      └── NesMemory (implements Memory6502)
          ├── 2KB RAM
          ├── NES PPU (2C02)
          ├── NES APU (RP2A03)
          ├── Controllers
          └── Mapper (cartridge banking)
```

## Frontend (`crates/frontend/gui`)

The GUI frontend provides a unified interface to all systems.

### Key Components

- **Window Management**: minifb for cross-platform windowing
- **Audio Playback**: rodio for cross-platform audio
- **Input**: Keyboard (with configurable mappings)
- **Settings**: Persistent configuration (config.json)
- **Save States**: Per-ROM state management
- **Video Processing**: CRT filters and scaling

### Video Processing Pipeline

```
System Renderer → Frame → VideoProcessor → Post-Processed Frame → Display
```

The frontend supports two video processing backends:

- **SoftwareProcessor**: CPU-based CRT filters (default)
- **OpenGLProcessor**: GPU-accelerated shader-based filters (optional)

## Renderer Architecture

All graphics-capable systems follow a unified renderer pattern:

```
System (state management) → Renderer trait → {Software, Hardware} implementations
```

### Benefits

- **Consistency**: Same interface across all systems
- **Flexibility**: Easy to add new rendering backends (Vulkan, Metal, DirectX)
- **Performance**: Optional GPU acceleration without modifying core emulation
- **Testability**: Renderers can be tested independently

### Current Implementations

- **N64**: `RdpRenderer` trait (3D triangle rasterization)
  - `SoftwareRdpRenderer`: CPU-based (complete)
  - `OpenGLRdpRenderer`: GPU-accelerated (stub)
  
- **PC**: `VideoAdapter` trait (text/graphics modes)
  - `SoftwareCgaAdapter`: CGA text mode
  - `CgaGraphicsAdapter`: CGA graphics modes
  - `SoftwareEgaAdapter`: EGA modes
  - `SoftwareVgaAdapter`: VGA modes
  - Hardware adapters: OpenGL stubs
  
- **Frontend**: `VideoProcessor` trait (post-processing)
  - `SoftwareProcessor`: CPU-based filters
  - `OpenGLProcessor`: GPU-accelerated shaders

## Data Flow

### Frame Execution

```
1. Frontend calls system.step_frame()
2. System executes CPU instructions until frame complete
3. CPU reads/writes trigger:
   - Memory bus operations
   - Video hardware updates (PPU, TIA, RDP)
   - Audio hardware updates (APU)
   - Input polling
4. System generates Frame (video) and AudioSamples (audio)
5. Frontend applies video processing (CRT filters)
6. Frontend displays frame and plays audio
```

### Save States

```
1. User presses F5 (save) or F6 (load)
2. Frontend calculates ROM hash
3. System serializes/deserializes state
4. State saved to saves/<rom_hash>/states.json
5. 5 slots available per game
```

## Memory Management

Each system implements its own memory bus with the appropriate Memory trait:

- **NES**: `Memory6502` trait
  - CPU RAM, PPU registers, APU registers, controllers, mapper
  
- **Game Boy**: `MemoryLr35902` trait
  - WRAM, HRAM, I/O registers, PPU, cartridge ROM/RAM
  
- **Atari 2600**: `Memory6502` trait
  - 128 bytes RIOT RAM, TIA registers, cartridge
  
- **N64**: `MemoryMips` trait
  - 4MB RDRAM, PIF, SP memory, RDP registers, cartridge ROM

## Testing Strategy

- **Unit Tests**: Core components (CPUs, audio, graphics utilities)
- **Integration Tests**: System-level functionality
- **Smoke Tests**: Basic ROM loading and execution for each system
- **Test ROMs**: Custom-built minimal ROMs for automated testing

Test ROMs are located in `test_roms/<system>/` and built from assembly source.

## Build System

- **Workspace**: Cargo workspace with multiple crates
- **Binary**: GUI crate builds as `hemu` (not `emu_gui`)
- **Features**: Optional features for OpenGL support (`--features opengl`)
- **Pre-commit Checks**: fmt, clippy, build, test (required before commits)

## Design Principles

1. **Modularity**: Reusable components over monolithic implementations
2. **Accuracy**: Cycle-accurate where feasible, frame-based where practical
3. **Testability**: Comprehensive test coverage for all components
4. **Documentation**: Clear documentation for architecture and implementation
5. **Code Reuse**: Share components across systems when possible
6. **Separation of Concerns**: Clean boundaries between state and rendering

## Future Architecture Improvements

- **Hardware Acceleration**: Complete OpenGL renderer integration
- **Pluggable Renderers**: Adopt renderer pattern for PPU-based systems (NES, GB, SNES)
- **Audio Mixing**: Unified audio mixing architecture
- **Input Abstraction**: Generic input system with gamepad support
- **Network**: Link cable emulation, netplay support

## Related Documentation

- **Implementation Guidelines**: See [AGENTS.md](AGENTS.md) for detailed implementation patterns
- **System Details**: See individual system README files for implementation specifics
- **User Guide**: See [MANUAL.md](MANUAL.md) for user-facing features and limitations
- **Contributing**: See [CONTRIBUTING.md](CONTRIBUTING.md) for development workflow
