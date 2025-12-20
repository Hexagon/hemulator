# agents.md

Purpose: guidance for automated agents and maintainers about CI, formatting, and safety.

- **Keep track of the work**: Keep a todo in TODO.md
- **Project structure**: workspace with `crates/core`, `crates/systems/*`, and `crates/frontend/gui`.
  - **Binary**: The GUI crate builds as `hemu` (not `emu_gui`)
  - **CLI removed**: There is no CLI frontend, only the GUI
  - **Core architecture**: Reusable CPU implementations in `crates/core/` (e.g., `cpu_6502`)
- **Agent tasks**:
  - Run `cargo fmt` and `cargo clippy` on PRs.
  - Build the workspace (`cargo build --workspace`).
  - Run unit/integration tests (`cargo test`).
  - Optionally run benchmarks in a separate job.
- **Implementation philosophy**:
  - **Always prefer full, tested implementations** of each module/component, even if all parts aren't immediately used
  - **Especially important** when other not-yet-implemented systems will use the features
  - Example: Implement complete APU with all channels (pulse, triangle, noise, DMC) even if only pulse is currently used, because future systems will need the other channels
  - Incomplete implementations create technical debt and require refactoring later
  - Full implementations with comprehensive tests ensure robustness and reusability
- **Permissions & safety**:
  - Agents must not add or distribute ROMs or other copyrighted game data.
  - Agents may run tests that do not require ROMs; for ROM-based tests, maintainers must provide legal test ROMs off-repo.
- **Cross-platform notes**:
  - Frontend uses `minifb` and `rodio` which are cross-platform; CI should include at least Linux and Windows runners.
  - For macOS specifics, `rodio` may require additional CI setup; document platform checks in CI config.
- **When to notify maintainers**:
  - Failing build or tests, or lint errors.
  - Long-running benchmark jobs exceeding expected time.

## Architecture

### Core Module (`crates/core/`)

Contains reusable CPU implementations and common traits:

- **`cpu_6502`**: Complete MOS 6502 CPU implementation
  - Generic `Memory6502` trait for memory access
  - Full instruction set with all addressing modes
  - Comprehensive test coverage (12 unit tests)
  - Can be used by any system: NES, Atari 2600, Apple II, Commodore 64, etc.
  - Implementation includes:
    - All official 6502 opcodes
    - Accurate cycle counting
    - Hardware interrupt support (NMI, IRQ)
    - Page-wrap bug emulation (JMP indirect)
    - Stack operations
    - Status flags (N, V, B, D, I, Z, C)
  - `ArrayMemory` helper for testing and simple use cases

- **`cpu_8080`**: Intel 8080 CPU implementation
  - Generic `Memory8080` trait for memory access
  - Foundation for Z80 and Game Boy CPUs
  - Stub implementation with basic opcodes (LXI, MOV, XCHG, etc.)
  - I/O port support (IN/OUT instructions)
  - Flag register (S, Z, AC, P, C)
  - Can be used for: Space Invaders, CP/M systems, early arcade games

- **`cpu_z80`**: Zilog Z80 CPU implementation
  - Generic `MemoryZ80` trait for memory access
  - Extends 8080 with shadow registers and index registers
  - Stub implementation with Z80-specific features:
    - Shadow register set (AF', BC', DE', HL')
    - Index registers (IX, IY)
    - Interrupt vector (I) and memory refresh (R) registers
    - Multiple interrupt modes (IM 0, 1, 2)
  - Can be used for: Sega Master System, Game Gear, ZX Spectrum, MSX

- **`cpu_lr35902`**: Sharp LR35902 CPU implementation (Game Boy)
  - Generic `MemoryLr35902` trait for memory access
  - Z80-like CPU with Game Boy-specific modifications
  - Stub implementation with GB-specific features:
    - 8-bit registers: A, F, B, C, D, E, H, L (no shadow registers)
    - 16-bit registers: SP, PC
    - Flags: Z (Zero), N (Subtract), H (Half Carry), C (Carry)
    - IME (Interrupt Master Enable) flag
    - HALT and STOP instructions
    - Starts at PC=0x0100 (after boot ROM)
  - Used by: Game Boy, Game Boy Color, Game Boy Advance (in GB mode)
- **`cpu_8086`**: Intel 8086 CPU implementation with core instruction set
  - Generic `Memory8086` trait for memory access
  - Segment-based memory addressing (CS, DS, ES, SS)
  - Comprehensive test coverage (22 unit tests)
  - Can be used by any system: IBM PC, PC XT, etc.
  - Implementation includes:
    - All general-purpose registers (AX, BX, CX, DX, SI, DI, BP, SP)
    - Segment registers (CS, DS, ES, SS)
    - Instruction pointer (IP) and FLAGS register
    - Core instructions: MOV (immediate), arithmetic (ADD, SUB, CMP, INC, DEC), logical (AND, OR, XOR)
    - Control flow: JMP (short), conditional jumps (JZ, JNZ, JC, JNC)
    - Stack operations (PUSH, POP)
    - Flag manipulation (CLC, STC, CLI, STI, CLD, STD)
    - Accurate cycle counting
    - Parity, zero, sign, carry, and overflow flags
  - Ready for extension with additional instructions (ModR/M, multiply/divide, shifts, string operations, etc.)
  - `ArrayMemory` helper for testing and simple use cases

- **`apu`**: Reusable audio processing unit components
  - **Core Components** (building blocks for various systems):
    - `PulseChannel`: Square wave generator with duty cycle control
    - `TriangleChannel`: Triangle wave generator (32-step)
    - `NoiseChannel`: Pseudo-random noise with LFSR
    - `Envelope`: Volume envelope generator with decay
    - `LengthCounter`: Automatic note duration control
    - `FrameCounter`: Timing controller for envelope/length/sweep units
  - **Audio Chip Implementations**:
    - `Rp2a03Apu`: NES NTSC audio chip (1.789773 MHz)
    - `Rp2a07Apu`: NES PAL audio chip (1.662607 MHz)
  - **AudioChip trait**: Common interface for pluggable audio systems
    - Allows different chips to be swapped (C64 SID, Atari 2600 TIA, ColecoVision SN76489, etc.)
    - Provides standard methods: `write_register`, `read_register`, `clock`, `reset`, `timing`
  - **Timing Support**:
    - `TimingMode` enum for NTSC/PAL configuration
    - CPU clock frequencies: NTSC 1.789773 MHz, PAL 1.662607 MHz
    - Frame rates: NTSC ~60.1 Hz, PAL ~50.0 Hz
    - Frame counter rates: NTSC 240 Hz, PAL 200 Hz
  - Comprehensive unit tests (40+ tests)

- **`ppu`**: Reusable video/graphics processing components
  - **Core Components** (building blocks for tile-based systems):
    - `IndexedPalette`: Generic indexed palette trait for color lookup systems
    - `RamPalette`: Simple RAM-based palette storage
    - `TileDecoder`: Trait for decoding tile/pattern data into pixel indices
    - `Nes2BppDecoder`: NES/Famicom 2bpp planar tile format
    - `GameBoy2BppDecoder`: Game Boy 2bpp interleaved tile format
    - `TileFormat`: Enum for different tile encoding formats
  - **Design Philosophy**:
    - Provides reusable primitives, not complete PPU implementations
    - Each system has unique register layouts, memory maps, and rendering pipelines
    - Systems like NES, Game Boy, SNES, Genesis share common concepts (tiles, palettes, sprites)
    - Core components reduce code duplication while allowing system-specific customization
  - **Future Formats**: SNES 4/8bpp, Genesis 4bpp linear (currently unimplemented)
  - Comprehensive unit tests (10+ tests)

- **`types`**: Common data structures (Frame, AudioSample)
- **`Cpu` trait**: Generic CPU interface
- **`System` trait**: High-level system interface

### System Modules (`crates/systems/`)

System-specific implementations that use core components:

- **NES (`emu_nes`)**: 
  - Uses `cpu_6502` from core with NES-specific bus implementation
  - `NesCpu` wraps `Cpu6502<NesMemory>` to provide NES-specific interface
  - `NesMemory` enum implements `Memory6502` trait for both simple array and full NES bus
  - NES bus includes: PPU, APU, controllers, mappers, RAM, WRAM
  - **PPU (Picture Processing Unit)**:
    - System-specific implementation in `crates/systems/nes/ppu.rs`
    - 2C02 PPU with 64-color master palette (NES-specific RGB values)
    - 32-byte palette RAM with NES-specific mirroring rules
    - 8KB CHR/pattern memory (ROM or RAM depending on cartridge)
    - 2KB internal VRAM for nametables with cartridge mirroring support
    - 256-byte OAM (Object Attribute Memory) for sprites
    - Background rendering with attribute table palette selection
    - Sprite rendering with 8x8 and 8x16 modes, priority, and flipping
    - Scrolling support with nametable switching
    - **Timing Model**: Frame-based rendering (not cycle-accurate)
      - Renders complete 256x240 frames on-demand via `render_frame()`
      - Does not track individual scanlines or PPU cycles
      - VBlank flag management for NMI generation
      - Suitable for most games; some games requiring precise PPU timing may not work perfectly
    - Could potentially use core `ppu` components (palette, tile decoder) in future refactoring
  - **PAL/NTSC Support**:
    - Auto-detection from iNES/NES 2.0 ROM headers
    - Timing-aware CPU cycles per frame (NTSC: ~29780, PAL: ~33247)
    - Timing-aware VBlank cycles (NTSC: 2500, PAL: 2798)
    - APU configured to match ROM timing mode
    - **PPU Timing Differences** (informational - not implemented in current frame-based model):
      - NTSC: 262 scanlines/frame, 341 PPU cycles/scanline, ~60.1 Hz
      - PAL: 312 scanlines/frame, 341 PPU cycles/scanline, ~50.0 Hz
      - Current implementation abstracts these differences at system level
  - **Mappers**:
    - System-specific implementations in `crates/systems/nes/mappers/`
    - Supported mappers: NROM (0), MMC1 (1), UxROM (2), CNROM (3), MMC3 (4), AxROM (7), MMC2 (9), MMC4 (10), ColorDreams (11), BNROM (34), GxROM (66), Camerica (71), NINA-03/06 (79), Namco 118 (206)
    - **Coverage**: ~90%+ of all NES games (based on nescartdb statistics)
    - **MMC2/MMC4 Latch Switching**: Fully implemented via CHR read callbacks
      - Hardware switches CHR banks when PPU reads from specific addresses ($0FD8, $0FE8, $1FD8-$1FDF, $1FE8-$1FEF)
      - PPU notifies mapper of CHR reads during frame rendering
      - Mapper tracks latch state changes and applies CHR bank updates after each frame
      - Frame-based rendering means updates happen per-frame, not mid-scanline
      - Games like Punch-Out!! and Fire Emblem should work correctly with per-frame latch switching
  - All existing tests pass (61 mapper and PPU tests)

- **Game Boy (`emu_gb`)**: Working implementation with CPU integration
  - Uses `cpu_lr35902` from core with GB-specific memory bus
  - `GbSystem` integrates CPU with `GbBus` memory implementation
  - **Memory Bus** (`GbBus`):
    - 8KB Work RAM (WRAM) at $C000-$DFFF
    - 127 bytes High RAM (HRAM) at $FF80-$FFFE
    - Cartridge ROM support (32KB+ with banking)
    - Cartridge RAM support (size auto-detected from ROM header)
    - I/O registers (stub)
    - Interrupt Enable ($FFFF) and Interrupt Flag ($FF0F) registers
    - Boot ROM disable support ($FF50)
  - **Cartridge Support**:
    - ROM loading with header parsing
    - RAM size detection (0KB, 8KB, 32KB, 64KB, 128KB)
    - MBC0 (no mapper) support
    - Future: MBC1, MBC3, MBC5 implementations
  - **Timing**:
    - 4.194304 MHz CPU clock
    - ~59.73 Hz frame rate
    - ~70224 cycles per frame
  - **Features**:
    - Save state support (CPU registers)
    - Cartridge mount/unmount
    - System reset
  - All tests pass (7 GB system tests)

- **Atari 2600 (`emu_atari2600`)**: 
  - Uses `cpu_6502` from core with Atari 2600-specific bus implementation (6507 variant)
  - `Atari2600Cpu` wraps `Cpu6502<Atari2600Bus>` to provide system-specific interface
  - Atari 2600 bus includes: TIA (video/audio), RIOT (RAM/I/O/timer), cartridge
  - **TIA (Television Interface Adapter)**:
    - Simplified scanline-based rendering (160x192 resolution)
    - Playfield rendering with reflection/score modes
    - Color registers for background, playfield, and sprites
    - Audio registers (simplified - full audio not yet implemented)
  - **RIOT (6532 chip)**:
    - 128 bytes of RAM with mirroring
    - Programmable interval timer (1, 8, 64, 1024 clock intervals)
    - I/O ports for joystick and console switches
  - **Cartridge Banking**:
    - Supports 2K, 4K (no banking), 8K (F8), 12K (FA), 16K (F6), 32K (F4)
    - Bank switching via memory writes to specific addresses
  - All existing tests pass (33 tests total)

- **PC (`emu_pc`)**: Experimental IBM PC/XT emulation
  - Uses `cpu_8086` from core with PC-specific bus implementation
  - `PcCpu` wraps `Cpu8086<PcBus>` to provide PC-specific interface
  - PC bus includes: 640KB RAM, 128KB VRAM, 256KB ROM area
  - **Memory Map**:
    - 0x00000-0x9FFFF: Conventional memory (640KB)
    - 0xA0000-0xBFFFF: Video memory (128KB)
    - 0xC0000-0xFFFFF: ROM area (256KB, includes BIOS)
    - 0xF0000-0xFFFFF: BIOS ROM (64KB)
  - **BIOS**:
    - Minimal BIOS stub for booting DOS executables
    - Entry point at 0xFFFF:0x0000 (physical 0xFFFF0)
    - Initializes segments and stack
    - Jumps to loaded program at 0x0000:0x0100 (COM file convention)
  - **Executable Support**:
    - COM files: Loaded at 0x0100, limited to 64KB - 256 bytes
    - EXE files: MZ header detected but full parsing not yet implemented
  - **Timing**:
    - 4.77 MHz CPU clock (IBM PC standard)
    - ~79,500 cycles per frame at 60 Hz
  - **Display**:
    - 640x400 frame buffer (text mode 80x25 equivalent)
    - Currently renders black screen (video hardware not implemented)
  - All tests pass (22 tests total)

### Frontend (`crates/frontend/gui`)

GUI frontend using minifb and rodio.

## Documentation Structure

- **README.md**: Developer-focused documentation (building, architecture, contributing)
- **MANUAL.md**: End-user manual with usage instructions, controls, troubleshooting
  - Included in all release packages
  - Keep separate from README to focus on user needs
  - Update when adding user-facing features or changing controls
- **CONTRIBUTING.md**: Contribution guidelines for developers
- **AGENTS.md**: This file - guidance for automated agents and CI
- **TODO.md**: Work tracking and future plans

## Release Packaging

When building release artifacts:
- **Include**: Executable (`hemu` or `hemu.exe`), `LICENSE`, `MANUAL.md`
- **Exclude**: All other files (source code, build artifacts, config files, saves)
- **Platforms**: Windows (.exe), Linux (binary + .deb package)
- **Architectures**: Both 64-bit (x86_64/amd64) and 32-bit (i686/i386)
- **Naming**: 
  - Windows 64-bit: `hemu-{version}-windows-x86_64.zip` containing `hemu.exe`, `LICENSE`, `MANUAL.md`
  - Windows 32-bit: `hemu-{version}-windows-i686.zip` containing `hemu.exe`, `LICENSE`, `MANUAL.md`
  - Linux 64-bit binary: `hemu-{version}-linux-x86_64.tar.gz` containing `hemu`, `LICENSE`, `MANUAL.md`
  - Linux 32-bit binary: `hemu-{version}-linux-i686.tar.gz` containing `hemu`, `LICENSE`, `MANUAL.md`
  - Debian package 64-bit: `hemu_{version}_amd64.deb` with proper packaging structure
  - Debian package 32-bit: `hemu_{version}_i386.deb` with proper packaging structure

## Settings System

The GUI frontend includes a comprehensive settings system stored in `config.json` in the executable directory.

### Settings Structure
- **Keyboard mappings**: Customizable button mappings for emulated controllers
  - Default: Z (A), X (B), LeftShift (Select), Enter (Start), Arrow keys (D-pad)
  - Settings automatically persist to disk on any change
- **Window size**: Actual window dimensions (width and height in pixels)
  - Automatically saved when window is resized
  - Default: 512x480 (2x scale of native 256x240 resolution)
- **Last ROM path**: Automatically remembered for quick restarts
- **Location**: `./config.json` (relative to executable, not working directory)

### ROM Loading

ROMs are auto-detected based on their format:
- **NES**: iNES format (header starts with `NES\x1A`)
- **Atari 2600**: Raw binary format, detected by size (2048, 4096, 8192, 12288, 16384, or 32768 bytes)
- **Game Boy**: GB/GBC format (Nintendo logo at offset 0x104)
- **PC/DOS**: MZ header for EXE files, or small binary files (16-65280 bytes) for COM files
- Unsupported formats show clear error messages

ROM loading workflow:
1. User opens ROM via F3 key or command-line argument
2. System detects ROM format automatically
3. Appropriate emulator core is selected (NES fully implemented, Atari 2600 core functional, GB is skeleton)
4. ROM hash is calculated for save state management
5. Last ROM path is saved to settings for auto-load on next start

### Save States

Save states are stored in `/saves/<rom_hash>/states.json` relative to the executable:
- **5 slots per game**: F5-F9 to save, Shift+F5-F9 to load
- **ROM hash-based organization**: Each ROM's states are in a separate directory
- **Base64 encoding**: State data is base64-encoded JSON
- **Automatic directory creation**: Save directories are created as needed
- **Instant persistence**: States are written immediately to disk

### Function Keys

- **F1**: Toggle help overlay (shows all controls)
- **F3**: Open ROM file dialog
- **F5-F9**: Save to slot 1-5
- **Shift+F5-F9**: Load from slot 1-5
- **F10**: Toggle debug overlay (shows ROM info, mapper, PAL/NTSC, FPS)
- **F11**: Cycle CRT filters (None → Scanlines → Phosphor → CRT Monitor → None)
- **F12**: Reset system
- **ESC**: Exit emulator

### CRT Filters

The GUI includes software-based CRT filters that can be toggled with F11:
- **None**: Raw pixel output
- **Scanlines**: Horizontal dark lines on odd rows (60% brightness)
- **Phosphor**: Horizontal color bleeding (15% blend with neighbors)
- **CRT Monitor**: Combined scanlines (70% darkness) + phosphor + brightness boost on even rows

Filters are applied to the frame buffer before display and do not affect overlays (help, debug, slot selector).
The selected filter persists in config.json.

### Default Screen

When no ROM is loaded or ROM fails to load, a default splash screen is displayed:
- Shows "HEMULATOR" logo
- Instructions: "Press F3 to open a ROM" and "Press F1 for help"
- Clean dark blue background with cyan/white text

## Debug Environment Variables

The emulator supports several environment variables for debugging:

- **`EMU_LOG_UNKNOWN_OPS=1`**: Log unknown/unimplemented 6502 opcodes to stderr
  - Useful for finding missing CPU instruction implementations
  - Example: `$env:EMU_LOG_UNKNOWN_OPS=1; cargo run --release -- roms/nes/game.nes`

- **`EMU_LOG_BRK=1`**: Log BRK instruction execution with PC and status register
  - Shows when BRK is executed and where it jumps to (IRQ vector)
  - Helpful for debugging unexpected BRK loops or interrupt issues
  - Example: `$env:EMU_LOG_BRK=1; cargo run --release -- roms/nes/game.nes`

- **`EMU_LOG_IRQ=1`**: Log when IRQ interrupts are fired
  - Shows when mapper or APU IRQs are pending and triggered
  - Useful for debugging IRQ timing issues
  - Example: `$env:EMU_LOG_IRQ=1; cargo run --release -- roms/nes/game.nes`

- **`EMU_TRACE_PC=1`**: Trace program counter (PC) execution
  - Logs every PC address executed (high-volume output)
  - Use with caution - generates massive log files
  - Example: `$env:EMU_TRACE_PC=1; cargo run --release -- roms/nes/game.nes > trace.log`

**PowerShell usage** (Windows):
```powershell
$env:EMU_LOG_BRK=1; $env:EMU_LOG_IRQ=1; cargo run --release -- roms/nes/excitebike.nes
```

**Bash usage** (Linux/macOS):
```bash
EMU_LOG_BRK=1 EMU_LOG_IRQ=1 cargo run --release -- roms/nes/excitebike.nes
```

