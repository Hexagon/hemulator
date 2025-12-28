# AGENTS.md

**Purpose**: Guidance for automated agents and maintainers about CI, formatting, and implementation guidelines.

**Related Documentation**:
- **[README.md](README.md)**: Developer quick start, build instructions, project overview
- **[ARCHITECTURE.md](ARCHITECTURE.md)**: Overall emulation system architecture and design patterns
- **[MANUAL.md](MANUAL.md)**: End-user manual with controls, features, and system-specific information
- **[CONTRIBUTING.md](CONTRIBUTING.md)**: Contribution workflow, pre-commit checks, coding standards

**System-Specific Implementation Details**:
- **[NES](crates/systems/nes/README.md)**: Nintendo Entertainment System
- **[Game Boy](crates/systems/gb/README.md)**: Game Boy / Game Boy Color
- **[Atari 2600](crates/systems/atari2600/README.md)**: Atari 2600
- **[SNES](crates/systems/snes/README.md)**: Super Nintendo Entertainment System
- **[N64](crates/systems/n64/README.md)**: Nintendo 64
- **[PC](crates/systems/pc/README.md)**: IBM PC/XT

---

## Agent Guidelines

- **Keep track of known limitations**: Document known limitations and missing features in MANUAL.md under each system's "Known Limitations" section. When making changes related to a system, review and update its limitations list if any are fixed.

- **Project structure**: workspace with `crates/core`, `crates/systems/*`, and `crates/frontend/gui`.
  - **Binary**: The GUI crate builds as `hemu` (not `emu_gui`)
  - **CLI removed**: There is no CLI frontend, only the GUI
  - **Core architecture**: Reusable CPU implementations in `crates/core/` (e.g., `cpu_6502`)

- **Agent tasks**:
  - Run `cargo fmt` and `cargo clippy` on PRs.
  - Build the workspace (`cargo build --workspace`).
  - Run unit/integration tests (`cargo test`).
  - Optionally run benchmarks in a separate job.

- **Pre-commit checks** (REQUIRED before committing any code):
  1. **Formatting**: `cargo fmt --all -- --check` - Must pass with no diff
  2. **Clippy**: `cargo clippy --workspace --all-targets -- -D warnings` - Must pass with no warnings
  3. **Build**: `cargo build --workspace` - Must compile successfully
  4. **Tests**: `cargo test --workspace` - All tests must pass
  - Run these checks in order and fix any issues before committing
  - If any check fails, fix the issues and re-run all checks
  - These same checks run in CI, so ensuring they pass locally prevents CI failures

- **Implementation philosophy**:
  - **Always prefer full, tested implementations** of each module/component, even if all parts aren't immediately used
  - **Especially important** when other not-yet-implemented systems will use the features
  - Example: Implement complete APU with all channels (pulse, triangle, noise, DMC) even if only pulse is currently used, because future systems will need the other channels
  - Incomplete implementations create technical debt and require refactoring later
  - Full implementations with comprehensive tests ensure robustness and reusability

- **Permissions & safety**:
  - Agents must not add or distribute ROMs or other copyrighted game data.
  - Agents may run tests that do not require ROMs; for ROM-based tests, maintainers must provide legal test ROMs off-repo.
  - **Exception**: Simple test ROMs created from scratch for smoke testing are allowed and required.

## Test ROM Requirements

- **Every system MUST have a basic test ROM** in `test_roms/<system>/` for smoke testing.
- Test ROMs must be minimal, created from scratch (not copyrighted), and built from assembly source.
- Each test ROM directory must include:
  - Assembly source code (`.s`, `.asm`)
  - Build script (`build.sh`)
  - Built ROM file for CI/testing
- Test ROMs should produce deterministic, verifiable output (e.g., known pixel pattern).
- If implementing a new system, create a test ROM before adding smoke tests.

**Building test ROMs**:
- NES: Use `cc65` (ca65 assembler, ld65 linker)
- Game Boy: Use `rgbds` (rgbasm assembler, rgblink linker, rgbfix for header)
- Atari 2600: Use `dasm` assembler
- Install on Ubuntu: `sudo apt-get install cc65 dasm libpng-dev && git clone https://github.com/gbdev/rgbds.git && cd rgbds && make && sudo make install`
- See `test_roms/README.md` for detailed instructions and specifications.

**Smoke tests**:
- Each system crate must include a smoke test using its test ROM.
- Smoke tests verify basic functionality: ROM loading, execution, and frame rendering.
- Tests should check frame dimensions and pixel data for expected patterns.
- See existing smoke tests in `crates/systems/*/src/lib.rs` for examples.

## Cross-Platform Notes

- Frontend uses SDL2 and `rodio` which are cross-platform; CI should include at least Linux and Windows runners.
- For macOS specifics, `rodio` may require additional CI setup; document platform checks in CI config.

## When to Notify Maintainers

- Failing build or tests, or lint errors.
- Long-running benchmark jobs exceeding expected time.

## Architecture Quick Reference

For comprehensive architecture documentation, see **[ARCHITECTURE.md](ARCHITECTURE.md)**.

For system-specific implementation details, see each system's README:
- **[NES](crates/systems/nes/README.md)** - PPU, APU, mappers
- **[Game Boy](crates/systems/gb/README.md)** - PPU, APU, MBCs
- **[Atari 2600](crates/systems/atari2600/README.md)** - TIA, RIOT, cartridges
- **[SNES](crates/systems/snes/README.md)** - PPU modes, memory map
- **[N64](crates/systems/n64/README.md)** - RDP renderer, RSP
- **[PC](crates/systems/pc/README.md)** - Video adapters, BIOS

**Core Components** (`crates/core/`):
- CPUs: 6502, 65C816, LR35902, Z80, 8080, MIPS R4300i, 8086/80186/80286/80386
- Audio: APU channels, envelopes, mixers
- Graphics: ZBuffer, ColorOps, palette/tile utilities
- Traits: System, Cpu, Renderer, AudioChip

**System Modules** (`crates/systems/`):
- ✅ NES (~90% game coverage), Atari 2600 (complete), Game Boy (complete)
- 🚧 SNES (basic), N64 (in development)
- 🧪 PC (experimental)

## Implementation Guidelines

When implementing new features for systems, follow these patterns:

### Audio Implementation

For detailed audio implementation patterns, see **[ARCHITECTURE.md](ARCHITECTURE.md#audio-components)**.

**Quick Pattern**:
1. Identify the audio hardware and select reusable components from `crates/core/src/apu/`:
   - `PulseChannel`, `TriangleChannel`, `WaveChannel`, `NoiseChannel`, `PolynomialCounter`
   - `Envelope`, `LengthCounter`, `SweepUnit`, `FrameCounter`
2. Create system-specific wrapper implementing the `AudioChip` trait
3. Map hardware registers to component parameters
4. Mix channels and generate audio samples
5. Write comprehensive tests for each register and channel

### Renderer Implementation

For detailed renderer patterns, see **[ARCHITECTURE.md](ARCHITECTURE.md#renderer-architecture)**.

**Quick Pattern**:
All systems with graphics follow this pattern:
```
System (state management) -> Renderer trait -> {Software, Hardware} implementations
```

1. Follow the `emu_core::renderer::Renderer` trait pattern
2. Implement core methods: `get_frame()`, `clear()`, `reset()`, `resize()`, `name()`
3. Add system-specific extensions as needed (e.g., `draw_triangle()` for 3D systems)
4. Always provide a software renderer first, hardware renderer is optional
5. See `crates/systems/n64/src/rdp_renderer.rs` (3D) or `crates/systems/pc/src/video_adapter.rs` (multi-mode) for examples

### System-Specific Components

For implementing system-specific components (PPU, mappers, etc.), see the corresponding system's README:
- **[NES README](crates/systems/nes/README.md)**: Mapper implementation patterns
- **[Game Boy README](crates/systems/gb/README.md)**: MBC implementation patterns
- **[PC README](crates/systems/pc/README.md)**: Video adapter implementation patterns

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
3. Appropriate emulator core is selected
4. ROM hash is calculated for save state management
5. Last ROM path is saved to settings for auto-load on next start

### Save States

Save states are stored in `/saves/<rom_hash>/states.json` relative to the executable:
- **5 slots per game**: F5-F9 to save, Shift+F5-F9 to load
- **ROM hash-based organization**: Each ROM's states are in a separate directory
- **Base64 encoding**: State data is base64-encoded JSON
- **Automatic directory creation**: Save directories are created as needed
- **Instant persistence**: States are written immediately to disk

## Logging System

The emulator uses a centralized logging system with command-line configuration.

For comprehensive logging documentation, see **[CONTRIBUTING.md](CONTRIBUTING.md#debug-logging)**.

**Quick Reference**:
- Use `--log-level <LEVEL>` to set global log level
- Use category-specific flags: `--log-cpu`, `--log-bus`, `--log-ppu`, `--log-apu`, `--log-interrupts`, `--log-stubs`
- Log levels: `off`, `error`, `warn`, `info`, `debug`, `trace`
- Example: `cargo run --release -- --log-cpu debug game.nes`

**For agents**: When adding logging to new code, use appropriate categories and levels. See CONTRIBUTING.md for implementation details.
## PC/DOS Testing Workbench

For rapid iteration when developing or debugging PC system code, use the workbench environment:

**Location**: `workbench/`

**Purpose**: Streamlined workflow for testing x86/DOS assembly code without manually editing disk images.

**Setup**:
```
workbench/
├── workbench.hemu      # Config: A: = FreeDOS, B: = test disk
├── source.asm          # Your test code
├── build.ps1           # Assembles and injects into B:
└── images/
    ├── x86boot.img     # FreeDOS boot disk (A:)
    └── temp.img        # Auto-created test disk (B:)
```

**Workflow**:
1. Edit `workbench/source.asm`
2. Run `.\workbench\build.ps1` (assembles to TEST.COM, injects into B: drive)
3. Run `cargo run --release -- workbench\workbench.hemu`
4. In FreeDOS: `B:\TEST.COM`

**Benefits**:
- **Fast iteration**: No manual disk image manipulation
- **Clean separation**: Boot OS (A:) vs test code (B:)
- **Automated**: Build script handles assembly and injection
- **Reusable**: FreeDOS stays on A:, only B: changes per test

**Use cases**:
- Testing INT 21h file I/O implementations
- Debugging DOS system calls
- Reproducing FreeDOS command behavior
- Isolating emulator bugs from DOS environment

See `workbench/README.md` for detailed instructions and examples.