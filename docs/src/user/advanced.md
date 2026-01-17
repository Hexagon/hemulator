---
title: "Advanced Features"
parent: "User Manual"
nav_order: 6
---

# Advanced Features & Debugging

### Command-Line Debugging Tools

Hemulator includes powerful debugging and tracing features for developers and advanced users. These tools run in **headless mode** (no GUI) for faster execution and automated testing.

#### Debug Dumps

Generate comprehensive snapshots of emulation state at specific execution points:

```bash
# Dump when PC reaches a specific address
hemu --debug-dump-pc 0x8000 game.nes

# Dump after N CPU cycles
hemu --debug-dump-cycles 10000 game.nes

# Specify custom output file
hemu --debug-dump-pc 0x8000 --debug-dump-file analysis.txt game.nes
```

**Debug dump contents**:
- Complete CPU state (all registers and flags)
- Disassembly (±100 instructions around current PC)
- Full memory hex dumps with ASCII view
- Cycle count and timestamp
- Screenshot of last rendered frame

#### Instruction Tracing

Track execution history with circular buffer tracing:

```bash
# Enable instruction tracing (default: 10,000 instructions)
hemu --trace-instructions game.nes

# Set custom trace buffer size
hemu --trace-instructions --trace-limit 5000 game.nes

# Dump trace to file when breakpoint is hit
hemu --trace-instructions --trace-dump-file trace.txt --breakpoint 0x8100 game.nes

# Multiple breakpoints
hemu --trace-instructions --breakpoint 0x8000 --breakpoint 0x8100 game.nes
```

**Trace features**:
- Circular buffer keeps last N executed instructions
- Breakpoints halt execution and trigger trace dump
- Minimal performance overhead when enabled
- Cross-system support (NES, GB, SMS, SNES, N64, Atari 2600, CHIP-8)

#### Debug Logging

Enable detailed logging for specific subsystems:

```bash
# CPU-level debugging
hemu --log-cpu debug game.nes

# Multiple log categories
hemu --log-cpu debug --log-interrupts info game.nes

# Global log level
hemu --log-level trace game.nes

# Save logs to file
hemu --log-cpu debug --log-file debug.log game.nes
```

**Log categories**:
- `--log-cpu` - CPU execution and instructions
- `--log-bus` - Memory bus operations
- `--log-ppu` - Graphics processing
- `--log-apu` - Audio processing
- `--log-interrupts` - Interrupt handling
- `--log-stubs` - Unimplemented features

**Log levels**: `off`, `error`, `warn`, `info`, `debug`, `trace`

#### Benchmark Mode

Measure raw emulation performance without frame limiting:

```bash
hemu --benchmark game.nes
```

Runs with no frame rate cap to measure maximum performance. Use the Inspector panel to view FPS and performance metrics.

### Use Cases

**Debugging game issues**:
- Use `--debug-dump-pc` to inspect state at specific code locations
- Combine with `--log-cpu trace` for detailed execution flow
- Screenshot captures visual state at breakpoint

**Automated testing**:
- Headless mode enables CI/CD integration
- Generate reference dumps for regression testing
- Deterministic execution for reproducible results

**Performance analysis**:
- Use `--benchmark` to measure optimization impact
- Trace buffer helps identify hot paths
- CPU logging reveals performance bottlenecks

**Development workflow**:
- Set breakpoints at problem addresses
- Trace buffer shows execution history leading to bug
- Memory dumps reveal corruption or incorrect state

For complete debugging documentation, see the [Contributing Guide](../developer/contributing.html#command-line-debug-dumps).

