# N64 Emulation - Nintendo 64 System

This crate implements Nintendo 64 emulation for the Hemulator project.

## Current Status

The N64 emulator is a **basic implementation** with functional RDP graphics processor supporting 3D triangle rendering. The emulator can execute test ROMs and render simple graphics, but full game compatibility requires additional work.

### What Works

- ✅ **MIPS R4300i CPU** - Complete instruction set implementation
- ✅ **Memory Bus** - 4MB RDRAM, PIF boot, SP memory, cartridge ROM
- ✅ **RDP (Reality Display Processor)** - Graphics rendering
  - Pluggable renderer architecture (Software/OpenGL)
  - 3D triangle rasterization (flat, Gouraud shading)
  - Z-buffer for depth testing
  - Display list processing
  - Basic RDP commands (fill, scissor, sync)
- ✅ **Cartridge Loading** - Z64/N64/V64 formats with byte-order conversion
- ✅ **Save States** - Full state serialization

### What's Missing

- ⏳ **RSP (Reality Signal Processor)** - Geometry processing, microcode execution
- ⏳ **Texture Mapping** - TMEM structure in place, sampling not implemented
- ⏳ **Audio** - Audio interface not implemented
- ⏳ **Controller Input** - Input system not implemented
- ⏳ **Memory Management** - No TLB, cache, or accurate timing

## Renderer Architecture

The N64 RDP uses a **pluggable renderer architecture** that allows switching between different rendering backends.

### Software Renderer (Default)

**Location**: `src/rdp_renderer_software.rs`

**Status**: ✅ Complete and production-ready

**Features**:
- CPU-based rasterization using scanline algorithm
- Full triangle rendering (flat, Gouraud, Z-buffered)
- 16-bit Z-buffer using `emu_core::graphics::ZBuffer`
- Color interpolation using `emu_core::graphics::ColorOps`
- Scissor clipping
- 6 comprehensive unit tests

**Performance**: Suitable for most use cases. Optimized with direct pixel access.

### OpenGL Renderer (Stub)

**Location**: `src/rdp_renderer_opengl.rs`

**Status**: ⏸️ Stub implementation (not functional)

**Feature Flag**: Build with `--features opengl` to include

**Blocker**: Requires OpenGL context from frontend
- Current frontend uses `minifb` which doesn't expose GL context
- Full implementation requires either:
  - Headless GL context (EGL/WGL)
  - Frontend migration to SDL2 or winit+glutin
  - Separate rendering window with GL context

**Architecture**: Template in place showing how to implement:
- OpenGL FBO for offscreen rendering
- Vertex buffers for triangles
- Hardware depth testing
- Shader programs for flat/Gouraud shading

See `N64_RENDERER_ARCHITECTURE.md` in the repository root for detailed architecture documentation.

## Building

### Default (Software Renderer)
```bash
cargo build --package emu_n64
```

### With OpenGL Stub
```bash
cargo build --package emu_n64 --features opengl
```

## Testing

```bash
# Run all tests (69 tests)
cargo test --package emu_n64

# Run with OpenGL stub (70 tests - includes OpenGL stub test)
cargo test --package emu_n64 --features opengl
```

### Test ROM

The `test_roms/n64/` directory contains a basic test ROM (`test.z64`) that:
- Draws colored rectangles using RDP fill commands
- Tests display list processing
- Verifies basic RDP functionality

Build the test ROM with:
```bash
cd test_roms/n64
./build.sh
```

## Usage Example

```rust
use emu_n64::N64System;
use emu_core::System;

// Create system
let mut n64 = N64System::new();

// Load ROM
let rom_data = std::fs::read("game.z64")?;
n64.mount("Cartridge", &rom_data)?;

// Run one frame
let frame = n64.step_frame()?;

// Access framebuffer
println!("Frame: {}x{}", frame.width, frame.height);
for pixel in &frame.pixels {
    // Process ARGB pixel data
}
```

## Architecture

### Directory Structure
```
src/
  ├── lib.rs                    - Public API and System trait impl
  ├── bus.rs                    - Memory bus (RDRAM, PIF, cartridge)
  ├── cpu.rs                    - MIPS R4300i wrapper
  ├── rdp.rs                    - RDP state and display list processor
  ├── rdp_renderer.rs           - Renderer trait definition
  ├── rdp_renderer_software.rs  - Software renderer (complete)
  ├── rdp_renderer_opengl.rs    - OpenGL renderer (stub)
  ├── rsp.rs                    - RSP stub (not implemented)
  ├── vi.rs                     - Video Interface registers
  └── cartridge.rs              - ROM loading and format detection
```

### Component Interaction

```
N64System
  └── N64Cpu (MIPS R4300i)
      └── N64Bus
          ├── RDRAM (4MB)
          ├── Cartridge ROM
          ├── RDP ─┬─> SoftwareRdpRenderer (default)
          │        └─> OpenGLRdpRenderer (stub)
          ├── RSP (stub)
          └── VI (registers only)
```

## Performance

**Software Renderer** (default):
- ~60 FPS for simple scenes on modern CPUs
- Scanline-based rasterization
- Single-threaded (CPU core utilization: 1 core)

**Future OpenGL Renderer**:
- Expected: >60 FPS for complex scenes
- GPU-accelerated rasterization
- Hardware depth testing

## Known Limitations

See `MANUAL.md` and `N64_RENDERER_ARCHITECTURE.md` for comprehensive lists.

**Critical limitations**:
1. No RSP - can't run real games (no geometry processing)
2. No texture mapping - only flat/shaded triangles
3. No audio
4. No controller input
5. Frame-based timing (not cycle-accurate)

## Future Development

### Short Term
1. Implement texture sampling (TMEM already structured)
2. Add more RDP display list commands
3. Improve VI integration for proper scanout

### Medium Term
1. RSP microcode execution (essential for games)
2. Audio interface implementation
3. Controller input support

### Long Term
1. Full OpenGL renderer with GL context integration
2. Cycle-accurate timing
3. TLB and cache emulation
4. Game compatibility improvements

## Contributing

When adding features to the N64 emulator:

1. **Follow the renderer pattern**: Keep renderers separate from RDP state
2. **Write tests**: Add unit tests for new functionality
3. **Document limitations**: Update `MANUAL.md` when fixing issues
4. **Preserve accuracy**: Software renderer should be reference implementation

## References

- **N64 Architecture**: See `N64_RENDERER_ARCHITECTURE.md`
- **RDP Commands**: Documented in `rdp.rs`
- **Test ROMs**: See `test_roms/n64/README.md` (if exists)

## License

Same as the parent Hemulator project.
