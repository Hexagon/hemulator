# Contributing to Hemulator

Contributions are welcome! Please follow these guidelines.

**For Users**: See [MANUAL.md](MANUAL.md) for usage instructions.

**For Architecture Details**: See [AGENTS.md](AGENTS.md) for implementation guidelines and system architecture.

## Pre-Commit Checks (REQUIRED)

Before committing any code, run these checks in order and ensure they all pass:

1. **Formatting**: `cargo fmt --all -- --check`
   - Must pass with no diff
   - If it fails, run `cargo fmt --all` to auto-format the code

2. **Clippy**: `cargo clippy --workspace --all-targets -- -D warnings`
   - Must pass with no warnings
   - Fix all clippy warnings before committing

3. **Build**: `cargo build --workspace`
   - Must compile successfully
   - Fix any compilation errors before committing

4. **Tests**: `cargo test --workspace`
   - All tests must pass
   - Add tests for new features and fix any failing tests

**Important**: These same checks run in CI, so ensuring they pass locally prevents CI failures and speeds up the review process.

## Additional Guidelines

- **Documentation**: 
  - Update [MANUAL.md](MANUAL.md) for user-facing changes (controls, features, system limitations)
  - Update [README.md](README.md) for developer setup info and project overview
  - Update [AGENTS.md](AGENTS.md) for architecture changes and implementation guidelines
- **Code Quality**: Write clean, well-documented code with meaningful variable names
- **Commit Messages**: Use clear, descriptive commit messages

## Areas for Contribution
- Additional mapper implementations (MMC5, VRC6, etc.)
- Game Boy emulation completion
- Performance optimizations
- Additional platform support
- UI/UX improvements

## Debug Environment Variables

The emulator supports several environment variables for debugging. These can be enabled by setting them to `1`, `true`, or `TRUE`, and disabled by setting them to `0` or any other value (or by not setting them at all).

**For comprehensive debug variable documentation**, see [AGENTS.md](AGENTS.md#debug-environment-variables).

### Core (6502 CPU)
- **`EMU_LOG_UNKNOWN_OPS`**: Log unknown/unimplemented 6502 opcodes to stderr
  - Useful for finding missing CPU instruction implementations
  - Applies to: NES, Atari 2600, and any other 6502-based systems

- **`EMU_LOG_BRK`**: Log BRK instruction execution with PC and status register
  - Shows when BRK is executed and where it jumps to (IRQ vector)
  - Helpful for debugging unexpected BRK loops or interrupt issues
  - Applies to: NES, Atari 2600, and any other 6502-based systems

### NES-Specific
- **`EMU_LOG_PPU_WRITES`**: Log all PPU register writes
  - Shows when games write to PPU registers ($2000-$2007)
  - Useful for debugging graphics/rendering issues
  
- **`EMU_LOG_IRQ`**: Log when IRQ interrupts are fired
  - Shows when mapper or APU IRQs are pending and triggered
  - Useful for debugging IRQ timing issues (e.g., MMC3 scanline counter)

- **`EMU_TRACE_PC`**: Log program counter hotspots every 60 frames
  - Shows the top 3 most frequently executed addresses
  - Useful for performance profiling and finding infinite loops

- **`EMU_TRACE_NES`**: Comprehensive NES system trace every 60 frames
  - Logs frame index, PC, CPU steps/cycles, IRQ/NMI counts, MMC3 A12 edges, PPU registers, and interrupt vectors
  - Useful for debugging complex system-level issues
  - High-level overview of NES state over time

### Usage Examples

**PowerShell (Windows):**
```powershell
# Enable a single log type
$env:EMU_LOG_BRK=1; cargo run --release -- roms/nes/game.nes

# Enable multiple log types
$env:EMU_LOG_BRK=1; $env:EMU_LOG_IRQ=1; cargo run --release -- roms/nes/game.nes

# Disable a log type
$env:EMU_LOG_BRK=0; cargo run --release -- roms/nes/game.nes
```

**Bash (Linux/macOS):**
```bash
# Enable a single log type
EMU_LOG_BRK=1 cargo run --release -- roms/nes/game.nes

# Enable multiple log types
EMU_LOG_BRK=1 EMU_LOG_IRQ=1 cargo run --release -- roms/nes/game.nes

# Disable a log type (or just don't set it)
EMU_LOG_BRK=0 cargo run --release -- roms/nes/game.nes
```

**Note**: Setting a variable to `0` or any value other than `1`, `true`, or `TRUE` will disable that log type.
