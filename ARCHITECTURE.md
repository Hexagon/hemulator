# Hemulator Architecture

This document describes the overall architecture of the Hemulator multi-system console emulator.

**Related Documentation**:
- **[README](README.md)**: Developer quick start, build instructions, project overview
- **[User Manual](https://hemulator.56k.guru/user/)**: End-user manual with controls, features, and system-specific information
- **[Contributing](https://hemulator.56k.guru/developer/contributing.html)**: Contribution workflow, pre-commit checks, coding standards
- **[Agent Guidelines](AGENTS.md)**: Implementation guidelines for automated agents and CI

**System-Specific Details**:
- **[NES](https://github.com/Hexagon/hemulator/blob/master/crates/systems/nes/README.md)**: Nintendo Entertainment System implementation
- **[Game Boy](https://github.com/Hexagon/hemulator/blob/master/crates/systems/gb/README.md)**: Game Boy / Game Boy Color implementation
- **[GBA](https://github.com/Hexagon/hemulator/blob/master/crates/systems/gba/README.md)**: Game Boy Advance implementation
- **[Atari 2600](https://github.com/Hexagon/hemulator/blob/master/crates/systems/atari2600/README.md)**: Atari 2600 implementation
- **[CHIP-8](https://github.com/Hexagon/hemulator/blob/master/crates/systems/chip8/README.md)**: CHIP-8 / Super-CHIP / XO-CHIP / Mega-CHIP implementation
- **[SMS](https://github.com/Hexagon/hemulator/blob/master/crates/systems/sms/README.md)**: Sega Master System implementation
- **[ColecoVision](https://github.com/Hexagon/hemulator/blob/master/crates/systems/colecovision/README.md)**: ColecoVision implementation
- **[SG-1000](https://github.com/Hexagon/hemulator/blob/master/crates/systems/sg1000/README.md)**: Sega SG-1000 implementation
- **[SNES](https://github.com/Hexagon/hemulator/blob/master/crates/systems/snes/README.md)**: Super Nintendo Entertainment System implementation
- **[N64](https://github.com/Hexagon/hemulator/blob/master/crates/systems/n64/README.md)**: Nintendo 64 implementation
- **[PS1](https://github.com/Hexagon/hemulator/blob/master/crates/systems/ps1/README.md)**: Sony PlayStation 1 implementation
- **[PC](https://github.com/Hexagon/hemulator/blob/master/crates/systems/pc/README.md)**: IBM PC/XT implementation


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
│  │  • Window Management (SDL2)                              │  │
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
  - Range-based interrupt priority system (hardware, BIOS, OS handlers)
  
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

### Debugger Architecture (`crates/core/src/debug.rs`)

The debugger subsystem provides a unified interface for system introspection, debugging, and analysis across all emulated systems.

#### Core Components

**Debug Trait (`Debugger`)**:
- `disassemble_instruction(address)`: Disassemble a single instruction at a given address
- `disassemble_range(address, count)`: Disassemble multiple instructions (with automatic mode tracking for 65C816)
- `read_memory(address, length)`: Read memory with bounds checking
- `get_memory_regions()`: Return list of memory regions with metadata
- `get_cpu_state()`: Snapshot of all CPU registers and flags
- `get_execution_history()`: Access instruction trace buffer (if enabled)
- `has_execution_history()`: Check if tracing is active

**Data Structures**:
- `MemoryRegion`: Memory region metadata (name, address range, description, permissions)
- `DisassembledInstruction`: Disassembled instruction with address, bytes, mnemonic, optional comment
- `CpuRegister`: Register value with name, value, width (8/16/32 bits)
- `CpuFlags`: Collection of CPU status flags
- `CpuState`: Complete CPU state snapshot (registers + flags + PC)
- `ExecutionTrace`: Instruction + post-execution CPU state

**Instruction Tracer (`crates/core/src/instruction_tracer.rs`)**:
- Circular buffer for execution history (default: 10 million instructions)
- Configurable at runtime (enable/disable, buffer size)
- Dump to file with formatted output
- Zero overhead when disabled
- Integrated with breakpoint system

#### Implementation Pattern

Each system implements the `Debugger` trait to expose its internals:

```rust
impl Debugger for MySystem {
    fn disassemble_instruction(&self, address: u32) -> Option<DisassembledInstruction> {
        let memory = self.read_memory(address, MAX_INSTR_SIZE)?;
        disasm_mycpu::disassemble(&memory, address)
    }

    fn read_memory(&self, address: u32, length: usize) -> Option<Vec<u8>> {
        // Bounds check address space
        // Read from system bus
    }

    fn get_memory_regions(&self) -> Vec<MemoryRegion> {
        vec![
            MemoryRegion::new("RAM", 0x0000, 0x1FFF, "System RAM", true, true),
            MemoryRegion::new("ROM", 0x8000, 0xFFFF, "Program ROM", true, false),
        ]
    }

    fn get_cpu_state(&self) -> CpuState {
        let mut state = CpuState::new(self.cpu.pc as u32);
        state.add_register(CpuRegister::new_16bit("PC", self.cpu.pc));
        state.add_register(CpuRegister::new_8bit("A", self.cpu.a));
        state.add_flag("Z", (self.cpu.status & 0x02) != 0);
        state
    }

    // Automatically implement execution history methods
    emu_core::impl_debugger_execution_history!();
}
```

**Helper Macros**:
- `impl_debugger_execution_history!()`: Auto-implement execution history methods by delegating to `instruction_tracer` field
- `impl_instruction_tracer_methods!()`: Add convenience methods for tracer control (`set_instruction_tracing`, `get_instruction_tracer`)

#### System-Specific Implementations

| System | Status | Special Features |
|--------|--------|------------------|
| **NES** | ✅ Complete | Full memory map, comprehensive tests |
| **Game Boy** | ✅ Complete | VRAM banks, OAM, IME/HALT flags |
| **SNES** | ✅ Complete | M/X flag tracking for accurate disassembly, 24-bit addressing |
| **Atari 2600** | ✅ Complete | TIA registers, 13-bit address space |
| **CHIP-8** | ✅ Complete | Variable memory size, all variants |
| **SMS** | ✅ Complete | Z80 shadow registers, interrupt modes |
| **ColecoVision** | ✅ Complete | BIOS region, Z80 full state |
| **SG-1000** | ✅ Complete | Z80 full state |
| **N64** | ✅ Complete | 32 GPRs, CP0 registers, MIPS disassembly |
| **PC** | ✅ Complete | BDA/EBDA regions, segment addressing, mode-specific regions |

#### GUI Integration

The debugger is exposed through the Inspector dock in the GUI:

- **Debug Tab** (`src/egui_ui/inspector_tabs.rs`):
  - **CPU State Panel**: Live register and flag values
  - **Memory Viewer**: Hex dump with ASCII view, multiple memory regions
  - **Disassembly Panel**: 
    - ±100 instructions around current PC
    - Current instruction highlighted
    - Address, bytes, mnemonic display
    - Auto-scrolls with PC

- **System-Specific Tabs**: Each system can add custom debug views
  - NES: Tiles, Palettes, Nametables, OAM
  - Game Boy: Tiles, Palettes, VRAM banks
  - PC: BDA/EBDA inspector, video memory

#### Command-Line Debug Tools

**Debug Dump** (`--debug-dump-pc`, `--debug-dump-cycles`):
- Headless mode (no GUI, faster execution)
- Generate comprehensive debug dumps:
  - Timestamp and cycle count
  - CPU state (all registers and flags)
  - Disassembly (±100 instructions around PC)
  - Full memory hex dumps for all regions
  - Screenshot of last frame
- Useful for CI/automated testing
- Example: `hemu --debug-dump-pc 0x8000 --debug-dump-file dump.txt game.nes`

**Instruction Tracing** (`--trace-instructions`, `--breakpoint`):
- Record all executed instructions to circular buffer
- Includes full CPU state after each instruction
- Breakpoints trigger automatic trace dump
- Example: `hemu --trace-instructions --breakpoint 0x8100 --trace-dump-file trace.txt game.nes`
- Essential for debugging execution flow issues

#### Testing Strategy

All debugger implementations include comprehensive tests:

1. **Memory Regions Test**: Verify all regions are exposed with correct addresses and permissions
2. **CPU State Test**: Check all registers and flags are present in correct order
3. **Memory Read Test**: Validate address bounds checking and read functionality
4. **Disassembly Test**: Verify instruction disassembly (when ROM can be mounted)

Example test pattern:
```rust
#[test]
fn test_memory_regions() {
    let system = MySystem::new();
    let regions = system.get_memory_regions();
    
    assert!(regions.len() >= 2);
    assert!(regions.iter().any(|r| r.name == "RAM"));
    // Verify each region's properties
}
```

#### Design Principles

1. **Consistency**: Same interface across all systems
2. **Performance**: Zero overhead when debugging is disabled
3. **Completeness**: Expose all system state for inspection
4. **Testability**: All implementations have comprehensive tests
5. **Documentation**: Clear docs for each system's memory map and registers

#### Future Enhancements

- **Watchpoints**: Break on memory read/write
- **Conditional Breakpoints**: Break on register values
- **Trace Analysis**: Statistical analysis of execution traces
- **Symbol Support**: Load symbol files for human-readable debugging
- **Remote Debugging**: GDB protocol support

For implementation examples, see:
- **NES**: `crates/systems/nes/src/debugger.rs` (reference implementation)
- **SNES**: `crates/systems/snes/src/debugger.rs` (advanced M/X flag tracking)
- **N64**: `crates/systems/n64/src/debugger.rs` (32 registers, MIPS example)

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

For the current system status and coverage, see the **[System Status table in README.md](README.md#system-status)**.

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

- **Window Management**: SDL2 for cross-platform windowing and OpenGL context
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

## GUI Frontend

The GUI frontend (`crates/frontend/gui/`) provides a modern cross-platform interface using SDL2 with egui for the UI:

### Architecture

```
┌──────────────────────────────────────────────────────────────┐
│ Window (SDL2 + egui)                                         │
│  ├─ Menu Bar (egui, top)                                     │
│  │   └─ File | Emulation | View | Help                       │
│  ├─ Main Content Area (center)                               │
│  │   ├─ Emulator Tab (game framebuffer, default)             │
│  │   ├─ New Project Tab (closeable)                          │
│  │   ├─ Help Tab (closeable)                                 │
│  │   └─ About Tab (closeable)                                │
│  ├─ Inspector Dock (bottom, resizable, moveable)             │
│  │   ├─ Generic Tabs (always):                               │
│  │   │   ├─ Log (with live message capture)                  │
│  │   │   ├─ Debug (CPU state, memory, disassembly)           │
│  │   │   └─ Memory (generic memory viewer)                   │
│  │   └─ System-Specific Tabs (dynamic):                      │
│  │       ├─ NES: Tiles, Palettes, Nametables                 │
│  │       ├─ GB: Tiles, Palettes                              │
│  │       ├─ SMS: Tiles, Palettes                             │
│  │       ├─ SNES: Tiles, Palettes, Layers                    │
│  │       └─ PC: BDA/EBDA Inspector                           │
│  ├─ Property Pane (right, resizable, moveable)               │
│  │   └─ Machine Metrics, Settings, Mounts, Save States       │
│  └─ Status Bar (bottom, fixed)                               │
│      └─ System | State | Messages | FPS                      │
└──────────────────────────────────────────────────────────────┘
```

### Components

- **Menu Bar** (`src/egui_ui/menu_bar.rs`): egui-based menu system
  - Dropdown menus with keyboard shortcuts
  - Dynamic enable/disable based on emulator state
  - Single "Inspector" toggle (replaces separate Debug/Log/Tiles items)
  
- **Inspector Dock** (`src/egui_ui/inspector_tabs.rs`, `src/egui_ui/dock_layout.rs`): System-aware debugging panel
  - **Generic tabs** (always available):
    - Log: Live log message capture from core logging system with level controls
    - Debug: CPU state, memory viewer, disassembly (comprehensive 3-panel view)
    - Memory: Generic memory inspector
  - **System-specific tabs** (dynamic based on loaded ROM):
    - Tabs automatically update when ROM is loaded/unloaded
    - Each system gets appropriate debugging tools (Tiles, Palettes, etc.)
  - Dockable using egui_dock (resizable, moveable)
  - Hidden by default, toggle with View → Inspector
  - All tabs non-closeable and always visible when dock is open
  
- **Property Pane** (`src/egui_ui/property_pane.rs`): System configuration and state
  - Machine metrics (FPS, CPU frequency, BDA for PC)
  - Project settings (renderer, display filter, emulation speed, input config)
  - Mount points (disk/ROM mounting)
  - Save state management (5 slots per game)
  - Dockable using egui_dock (resizable, moveable)
  
- **Status Bar** (`src/egui_ui/status_bar.rs`): Real-time status display
  - System name, pause/speed state, messages, FPS counter

- **Tab System** (`src/egui_ui/tabs.rs`): Main content area tabs
  - Emulator tab (game display with scaling modes)
  - New Project tab (system selection)
  - Help tab (keyboard controls)
  - About tab (version info)

- **Window Backend** (`src/window_backend/`): SDL2 + egui abstraction
  - Event handling (keyboard, mouse, gamepad)
  - Frame presentation with egui overlay
  - Window management

### Design Decisions

- **egui framework**: Modern immediate-mode GUI for responsive, developer-friendly UI
- **Docking system**: egui_dock enables flexible, customizable layout
- **System-aware Inspector**: Tabs dynamically adapt to loaded system's debugging needs
- **Live logging**: Message capture buffer (1000 messages) for real-time log display
- **Cross-platform**: Works identically on Windows, macOS, and Linux
- **No native dependencies**: Pure Rust + SDL2, no GTK/Win32/Cocoa required

For user-facing controls and features, see the [User Manual](https://hemulator.56k.guru/user/).

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
- **User Guide**: See the [User Manual](https://hemulator.56k.guru/user/) for user-facing features and limitations
- **Contributing**: See the [Contributing Guide](https://hemulator.56k.guru/developer/contributing.html) for development workflow
