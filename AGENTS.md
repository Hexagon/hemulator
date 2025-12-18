# agents.md

Purpose: guidance for automated agents and maintainers about CI, formatting, and safety.

- **Keep track of the work**: Keep a todo in TODO.md
- **Project structure**: workspace with `crates/core`, `crates/systems/*`, and `crates/frontend/gui`.
  - **Binary**: The GUI crate builds as `hemu` (not `emu_gui`)
  - **CLI removed**: There is no CLI frontend, only the GUI
- **Agent tasks**:
  - Run `cargo fmt` and `cargo clippy` on PRs.
  - Build the workspace (`cargo build --workspace`).
  - Run unit/integration tests (`cargo test`).
  - Optionally run benchmarks in a separate job.
- **Permissions & safety**:
  - Agents must not add or distribute ROMs or other copyrighted game data.
  - Agents may run tests that do not require ROMs; for ROM-based tests, maintainers must provide legal test ROMs off-repo.
- **Cross-platform notes**:
  - Frontend uses `minifb` and `rodio` which are cross-platform; CI should include at least Linux and Windows runners.
  - For macOS specifics, `rodio` may require additional CI setup; document platform checks in CI config.
- **When to notify maintainers**:
  - Failing build or tests, or lint errors.
  - Long-running benchmark jobs exceeding expected time.

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
- **Window scale**: 1x, 2x, 4x, or 8x window scaling (default: 2x)
- **Last ROM path**: Automatically remembered for quick restarts
- **Location**: `./config.json` (relative to executable, not working directory)

### ROM Loading

ROMs are auto-detected based on their format:
- **NES**: iNES format (header starts with `NES\x1A`)
- **Game Boy**: GB/GBC format (Nintendo logo at offset 0x104)
- Unsupported formats show clear error messages

ROM loading workflow:
1. User opens ROM via F3 key or command-line argument
2. System detects ROM format automatically
3. Appropriate emulator core is selected (NES fully implemented, GB is skeleton)
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
- **F11**: Cycle window scale (1x → 2x → 4x → 8x → 1x)
- **F12**: Reset system
- **ESC**: Exit emulator

### Default Screen

When no ROM is loaded or ROM fails to load, a default splash screen is displayed:
- Shows "HEMULATOR" logo
- Instructions: "Press F3 to open a ROM" and "Press F1 for help"
- Clean dark blue background with cyan/white text

Local reproduction: run the same commands the agent runs (build, clippy, test) from the workspace root.
