# Contributing to Hemulator

Contributions are welcome! Please follow these guidelines.

**For Users**: See [MANUAL.md](MANUAL.md) for usage instructions.

**For Architecture Details**: See [ARCHITECTURE.md](ARCHITECTURE.md) for overall emulation system architecture.

**For Implementation Guidelines**: See [AGENTS.md](../AGENTS.md) for detailed implementation patterns and CI requirements.

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
  - Update [README.md](../README.md) for developer setup info and project overview
  - Update [AGENTS.md](../AGENTS.md) for architecture changes and implementation guidelines
- **Code Quality**: Write clean, well-documented code with meaningful variable names
- **Commit Messages**: Use clear, descriptive commit messages

## Areas for Contribution
- Additional mapper implementations (MMC5, VRC6, etc.)
- Game Boy emulation completion
- Performance optimizations
- Additional platform support
- UI/UX improvements

## Performance Optimization

### Build Profiles

The project uses optimized Cargo profiles for different build scenarios:

**Release Builds** (`cargo build --release`):
- `opt-level = 3`: Maximum optimization level
- `lto = "fat"`: Full Link Time Optimization for cross-crate inlining
- `codegen-units = 1`: Single codegen unit for maximum optimization (slower compile, faster runtime)
- `strip = true`: Strip debug symbols to reduce binary size
- `panic = "abort"`: Abort on panic for smaller binaries

Expected performance: **300+ FPS** on modern hardware (significantly faster than the 60Hz target)

**Debug Builds** (`cargo build`):
- `opt-level = 1`: Basic optimization to make debug builds usable
- Debug symbols enabled for debugging

Expected performance: **~60 FPS** (usable for development and testing)

### Performance Testing

When working on performance-sensitive code (CPU emulation, PPU rendering, mapper logic):

1. Always test with `--release` for accurate performance measurements
2. Use the debug overlay (F10) to monitor FPS in real-time
3. Profile with `cargo flamegraph` or `perf` if investigating specific hotspots
4. Ensure changes don't significantly impact frame times

### Performance Tips

- Use `#[inline]` on hot-path functions (CPU instruction handlers, memory read/write)
- Avoid allocations in per-frame or per-instruction code
- Use const arrays instead of HashMap lookups when possible
- Profile before and after optimization changes

## Debug Logging

The emulator supports debug logging through command-line arguments.

**For comprehensive logging documentation and implementation guidelines**, see [AGENTS.md](../AGENTS.md#logging-system).

### Command-Line Logging Options

Use these flags when running the emulator to enable debug logging:

- **`--log-level <LEVEL>`**: Set global log level for all categories
- **`--log-cpu <LEVEL>`**: Log CPU execution (instruction execution, PC tracing, BRK)
- **`--log-bus <LEVEL>`**: Log bus/memory access (disk I/O, INT13 calls)
- **`--log-ppu <LEVEL>`**: Log PPU/graphics (register writes, rendering, TIA)
- **`--log-apu <LEVEL>`**: Log APU/audio operations
- **`--log-interrupts <LEVEL>`**: Log interrupts (IRQ, NMI)
- **`--log-stubs <LEVEL>`**: Log unimplemented features/opcodes

**Log Levels** (in increasing verbosity):
- `off` - No logging (default)
- `error` - Critical errors only
- `warn` - Warnings and errors
- `info` - Informational messages
- `debug` - Detailed debugging information
- `trace` - Very verbose tracing (performance impact)

### Usage Examples

```bash
# Enable CPU debug logging
cargo run --release -- --log-cpu debug game.nes

# Enable multiple categories
cargo run --release -- --log-cpu debug --log-interrupts info game.nes

# Set global level (applies to all categories)
cargo run --release -- --log-level trace game.nes

# Mix global and specific levels (specific overrides global)
cargo run --release -- --log-level info --log-cpu trace game.nes
```
