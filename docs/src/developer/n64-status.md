---
title: "N64 Status"
parent: "Developer Manual"
nav_order: 3
---
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
- ✅ **OpenGL Hardware Renderer** - GPU-accelerated rendering (REQUIRED)
- ✅ **3D Triangle Rendering** - Flat shading, Gouraud shading, Z-buffer depth testing
- ✅ **Display List Processing** - RDP command execution
- ✅ **Basic RDP Commands** - Fill, scissor, sync operations
- ✅ **Test ROM Rendering** - Colored rectangles and simple 3D scenes work correctly

### ⏳ Partially Working / In Development

**RSP (Reality Signal Processor)**:
- ⏳ **High-Level Emulation (HLE)** - Partial F3DEX/F3DEX2 support implemented
  - ✅ Microcode detection and vertex transformation pipeline
  - ⚠️ Many F3DEX commands are stubs (G_MOVEWORD, G_MOVEMEM, G_SETOTHERMODE_L/H)
  - ❌ Audio microcode tasks not implemented
  - ❌ Semaphore and signal bits not implemented
- ❌ **Instruction-Level Execution** - Not implemented (HLE only, no LLE)
- ❌ **Geometry Processing** - Limited to basic vertex transformation

**Graphics (RDP)**:
- ⏳ **Display List Processing** - Partial command support
  - ✅ Basic commands (FILL_RECTANGLE, SET_FILL_COLOR, SYNC, SET_SCISSOR)
  - ✅ Triangle rendering (all modes 0x08-0x0F)
  - ⚠️ SET_OTHER_MODES stubbed (rendering mode configuration ignored)
  - ⚠️ Performance counters return hardcoded zeros
- ⏳ **Texture Mapping** - TMEM structure in place, some formats missing
  - ✅ RGBA16, RGBA32 formats working
  - ❌ Advanced formats (CI, IA, I) render as white
  - ❌ Perspective-correct mapping not implemented

**Audio**:
- ⏳ **Audio Interface (AI)** - Hardware implemented but no microcode support
  - ✅ AI registers and DMA transfers working
  - ✅ Integration with frontend audio output
  - ❌ RSP audio microcode tasks not implemented - no sound output

**Save System**:
- ❌ **EEPROM** - Not implemented (4Kbit/16Kbit variants)
- ❌ **Flash RAM** - Not implemented
- ❌ **Controller Pak** - Not implemented (memory card support)

**CPU**:
- ⏳ **Edge Cases** - Some features not implemented
  - ❌ Overflow traps (uses wrapping arithmetic instead)
  - ❌ Memory alignment validation
  - ⚠️ Cache is direct-mapped only (no full coherency)

### ❌ Not Implemented

- ❌ **Controller Input** - Input system not connected to frontend (infrastructure complete, mapping needed)
- ❌ **Memory Management** - No full TLB coherency or cache simulation (basic TLB/MMU implemented)
- ❌ **Cycle-Accurate Timing** - Frame-based timing (50,000 cycles/frame vs hardware's 1,562,500)

## Renderer Architecture

The N64 RDP now uses **OpenGL 3.3+ hardware-accelerated rendering exclusively**. Software renderer has been removed to simplify the codebase and ensure consistent GPU performance.

### OpenGL Renderer (Required)

- ✅ **GPU-accelerated rendering** using OpenGL 3.3 Core Profile
- ✅ **Hardware depth testing** - Leverages GPU Z-buffer
- ✅ **Full triangle rendering** - Flat shading, Gouraud shading, textured triangles
- ✅ **Feature parity** - All RDP rendering capabilities
- ✅ **Default feature** - OpenGL is now part of default features (always enabled)
- **Requirements**: OpenGL 3.3+ compatible graphics hardware

### Architecture Changes (January 2026)

- ❌ **Software renderer removed** - Deleted ~850 lines of CPU-based rasterization code
- ✅ **Simplified initialization** - RDP created directly with OpenGL renderer
- ✅ **Mandatory GL context** - `N64System::new()` requires `glow::Context` parameter
- ✅ **No renderer switching** - Renderer is fixed at system creation time

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

**Note**: N64 tests require an actual OpenGL context and are marked as `#[ignore]` for CI.

```bash
# Run all N64 tests (non-GL tests only)
cargo test --package emu_n64

# Run all tests including GL-dependent tests (requires OpenGL 3.3+)
cargo test --package emu_n64 -- --ignored
```

## Known Limitations

See [MANUAL.md](MANUAL.md#n64-nintendo-64) for complete user-facing limitations.

**Critical blockers for commercial games**:
1. **Incomplete RSP HLE** - Many F3DEX commands stubbed (G_MOVEWORD, G_MOVEMEM, G_SETOTHERMODE_*); audio microcode not implemented
2. **No Save System** - EEPROM/Flash/Controller Pak not implemented; games cannot save progress
3. **RDP Commands Incomplete** - SET_OTHER_MODES ignored; some texture formats missing (render as white); performance counters return zeros
4. **No Audio Microcode** - RSP audio tasks not implemented; games cannot produce sound
5. **Frame-based timing** - Uses 50,000 cycles/frame (vs hardware's 1,562,500); not cycle-accurate
6. **CPU Edge Cases** - Overflow traps not implemented; memory alignment not validated; cache is direct-mapped only

**System Requirements**:
- **OpenGL 3.3+ required** - No software fallback available
- Compatible graphics hardware with OpenGL 3.3+ support

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
1. Cycle-accurate timing
2. TLB and cache emulation
3. Commercial game compatibility

## Building and Debugging

### Build Commands
```bash
# Default (OpenGL renderer - always enabled)
cargo build --package emu_n64 --profile release-quick

# Frontend with N64 support
cargo build --profile release-quick
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
- **January 2026**: OpenGL renderer made mandatory, software renderer removed (~850 lines)
- **January 2026**: Basic RDP rendering, test ROMs working
- **December 2025**: Initial MIPS R4300i CPU implementation
- **November 2025**: Project structure and core components

---

**Document Status**: This is a high-level status overview. For implementation details, always refer to the [N64 System README](../crates/systems/n64/README.md).
