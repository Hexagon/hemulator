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

## Build Dependencies

### Linux Development Dependencies

On Ubuntu/Debian, install these packages before building:

```bash
sudo apt-get update
sudo apt-get install -y libasound2-dev pkg-config
```

**Required packages:**
- `libasound2-dev` - Required for audio support (ALSA)
- `pkg-config` - Required for library detection during build

### 32-bit Linux Builds (i686)

For building 32-bit binaries on 64-bit Linux (e.g., for release packaging):

```bash
sudo dpkg --add-architecture i386
sudo apt-get update
sudo apt-get install -y gcc-multilib g++-multilib libasound2-dev:i386
```

**Required packages:**
- `gcc-multilib` - 32-bit C compiler support and libraries
- `g++-multilib` - 32-bit C++ compiler and standard library (required for SDL2 bundled build)
- `libasound2-dev:i386` - 32-bit ALSA audio library

**Note**: The `g++-multilib` package is essential because SDL2's bundled build compiles C++ code from source using CMake. Without the 32-bit C++ standard library, the build will fail with linker errors like `cannot find -lstdc++`.

### Windows and macOS

No additional system dependencies are required beyond the Rust toolchain. All required libraries are either bundled or managed through Cargo.

## Additional Guidelines

- **Documentation**: 
  - Update [MANUAL.md](MANUAL.md) for user-facing changes (controls, features, system limitations)
  - Update [README.md](../README.md) for developer setup info and project overview
  - Update [AGENTS.md](../AGENTS.md) for architecture changes and implementation guidelines
- **Code Quality**: Write clean, well-documented code with meaningful variable names
- **Commit Messages**: Use clear, descriptive commit messages

## Benchmarking

The project uses [Criterion.rs](https://github.com/bheisler/criterion.rs) for performance benchmarking.

### Running Benchmarks

```bash
# Run all benchmarks in a specific crate
cd crates/core
cargo bench

# Run a specific benchmark
cargo bench cpu_6502

# Save baseline for comparison
cargo bench --bench cpu_6502 -- --save-baseline my-baseline

# Compare against baseline
cargo bench --bench cpu_6502 -- --baseline my-baseline
```

### Available Benchmarks

- **`cpu_6502`**: CPU instruction execution performance
  - Single instruction execution
  - Multiple step execution (10, 100, 1000 instructions)
  - Addressing mode performance
  - Reset operation

### Adding New Benchmarks

1. Create a new file in `crates/*/benches/`
2. Use criterion's benchmark harness
3. Focus on hot paths (instruction execution, rendering, memory access)
4. Document baseline expectations in benchmark comments

Example benchmark structure:
```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_something(c: &mut Criterion) {
    c.bench_function("test_name", |b| {
        b.iter(|| {
            // Code to benchmark
            black_box(result);
        });
    });
}

criterion_group!(benches, bench_something);
criterion_main!(benches);
```

## Security

### Dependency Auditing

The project uses `cargo-audit` to check for known security vulnerabilities in dependencies.

```bash
# Install cargo-audit
cargo install cargo-audit

# Run security audit
cargo audit

# Update audit database
cargo audit fetch
```

Security audits run automatically in CI on every push and pull request. If vulnerabilities are found:

1. Check the advisory details with `cargo audit`
2. Update affected dependencies if patches are available
3. If no patch exists, evaluate risk and document the decision
4. Consider using `cargo audit fix` to automatically update vulnerable dependencies

### Reporting Security Issues

If you discover a security vulnerability, please report it privately to the maintainers rather than opening a public issue.

## Areas for Contribution

Contributions are welcome in the following areas:

- **NES**: Additional mapper implementations (MMC5, VRC6, etc.)
- **Atari 2600**: Bug fixes for major gameplay issues, improved compatibility
- **Game Boy**: Bug fixes for major gameplay issues, improved compatibility
- **SNES**: APU (SPC700) implementation, additional PPU modes (2-7), bug fixes
- **N64**: RSP microcode execution, texture mapping improvements
- **PC**: More complete DOS API (INT 21h), PC speaker audio
- **All Systems**: Performance optimizations, UI/UX improvements, bug fixes
- **Cross-platform**: Additional platform support, testing on macOS

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
