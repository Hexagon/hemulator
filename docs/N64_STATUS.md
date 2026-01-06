# N64 Emulator Development Status

**Last Updated**: January 6, 2026  
**Status**: Basic implementation with functional RDP graphics processor  
**ROM Tested**: Test ROMs (WORKING - rectangles, triangles), Commercial ROMs (NOT PLAYABLE - require RSP)

## Overview

For detailed implementation information, see:
- **[N64 System README](../crates/systems/n64/README.md)** - Complete implementation details
- **[Architecture Documentation](ARCHITECTURE.md)** - Overall emulator architecture
- **[User Manual](MANUAL.md)** - End-user features and limitations

## Current Implementation Status

### ✅ Working Components

**Core System**:
- ✅ **MIPS R4300i CPU** - Complete instruction set implementation from `emu_core::cpu_mips_r4300i`
- ✅ **Memory Bus** - 4MB RDRAM, PIF boot, SP memory, cartridge ROM
- ✅ **Cartridge Loading** - Z64/N64/V64 formats with byte-order conversion
- ✅ **Save States** - Full system state serialization

**Graphics (RDP)**:
- ✅ **Software Renderer** - CPU-based rasterization (default, production-ready)
- ✅ **3D Triangle Rendering** - Flat shading, Gouraud shading, Z-buffer depth testing
- ✅ **Display List Processing** - RDP command execution
- ✅ **Basic RDP Commands** - Fill, scissor, sync operations
- ✅ **Test ROM Rendering** - Colored rectangles and simple 3D scenes work correctly

### ⏳ Partially Working / In Development

**RSP (Reality Signal Processor)**:
- ⏳ **Basic Infrastructure** - Stub implementation in place
- ❌ **Microcode Execution** - Not implemented (required for commercial games)
- ❌ **Geometry Processing** - Not implemented

**Graphics**:
- ⏳ **Texture Mapping** - TMEM structure in place, sampling not fully implemented
- ⏳ **Advanced RDP Commands** - Many commands stubbed or missing

### ❌ Not Implemented

- ❌ **Audio** - Audio interface not implemented
- ❌ **Controller Input** - Input system not implemented
- ❌ **Memory Management** - No TLB, cache, or accurate timing
- ❌ **Commercial Game Support** - RSP microcode execution required

## Renderer Architecture

The N64 RDP uses a **pluggable renderer architecture**. See the [N64 README](../crates/systems/n64/README.md#renderer-architecture) for details.

### Software Renderer (Default)
- ✅ Complete and production-ready
- CPU-based scanline rasterization
- Full triangle rendering with depth testing
- Suitable for most use cases

### OpenGL Renderer
- ⏸️ Stub implementation (not functional)
- Build with `--features opengl` to include
- Hardware-accelerated rendering (when implemented)

## Testing

### Test ROMs
The `test_roms/n64/` directory contains basic test ROMs that verify:
- RDP fill command rendering
- Display list processing
- Triangle rasterization
- Z-buffer depth testing

Build test ROMs with:
```bash
cd test_roms/n64
./build.sh
```

### Running Tests
```bash
# Run all N64 tests
cargo test --package emu_n64

# With OpenGL stub
cargo test --package emu_n64 --features opengl
```

## Known Limitations

See [MANUAL.md](MANUAL.md#n64-nintendo-64) for complete user-facing limitations.

**Critical blockers for commercial games**:
1. **No RSP microcode execution** - Cannot process geometry from games
2. **No texture mapping** - Only flat/shaded triangles (TMEM structure exists but sampling incomplete)
3. **No audio** - Audio interface not implemented
4. **No controller input** - Input system not implemented
5. **Frame-based timing** - Not cycle-accurate

## Development Priorities

For detailed development roadmap, see the [N64 README](../crates/systems/n64/README.md#future-development).

### Short Term
1. Complete texture sampling implementation
2. Expand RDP command coverage
3. Improve VI integration

### Medium Term
1. **RSP microcode execution** (essential for commercial games)
2. Audio interface implementation
3. Controller input support

### Long Term
1. OpenGL renderer with GL context integration
2. Cycle-accurate timing
3. TLB and cache emulation
4. Commercial game compatibility

## Building and Debugging

### Build Commands
```bash
# Default (software renderer)
cargo build --package emu_n64 --profile release-quick

# With OpenGL stub
cargo build --package emu_n64 --features opengl --profile release-quick
```

### Debug Logging
```bash
# CPU execution trace
cargo run --profile release-quick -- rom.z64 --log-cpu trace

# Interrupt debugging
cargo run --profile release-quick -- rom.z64 --log-interrupts info

# PPU/graphics debugging
cargo run --profile release-quick -- rom.z64 --log-ppu info
```

## References

### Documentation
- **[N64 System README](../crates/systems/n64/README.md)** - Detailed implementation docs
- **[ARCHITECTURE.md](ARCHITECTURE.md)** - Overall emulator architecture
- **[MANUAL.md](MANUAL.md)** - User manual and features
- **[AGENTS.md](../AGENTS.md)** - Implementation guidelines

### Hardware Reference
- **[MIPS R4300i CPU Reference](references/cpu_mips_r4300i.md)** - CPU documentation
- See N64 README for additional hardware references

## Change History

This document tracks high-level status. For detailed change history:
- See git commit log for implementation changes
- See N64 README for architecture and component details
- Historical detailed session logs have been archived

**Major Milestones**:
- **January 2026**: Basic RDP rendering, test ROMs working
- **December 2025**: Initial MIPS R4300i CPU implementation
- **November 2025**: Project structure and core components

---

**Document Status**: This is a high-level status overview. For implementation details, always refer to the [N64 System README](../crates/systems/n64/README.md).
