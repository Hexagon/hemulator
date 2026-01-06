# N64 Emulation - Nintendo 64 System

This crate implements Nintendo 64 emulation for the Hemulator project.

## Current Status

The N64 emulator is a **basic implementation** with functional RDP graphics processor supporting 3D triangle rendering. The emulator can execute test ROMs and render simple graphics, but full game compatibility requires additional work.

### What Works

- ✅ **MIPS R4300i CPU** - Complete instruction set implementation
- ✅ **Memory Bus** - 4MB RDRAM, PIF boot, SP memory, cartridge ROM
- ✅ **RDP (Reality Display Processor)** - OpenGL hardware-accelerated graphics rendering
  - GPU-accelerated using OpenGL 3.3 Core Profile
  - 3D triangle rasterization (flat, Gouraud shading, textured triangles)
  - Hardware Z-buffer for depth testing
  - Display list processing
  - Basic RDP commands (fill, scissor, sync, texture operations)
- ✅ **Cartridge Loading** - Z64/N64/V64 formats with byte-order conversion
- ✅ **Save States** - Full state serialization

### What's Missing

- ⏳ **RSP (Reality Signal Processor)** - Geometry processing, microcode execution
- ⏳ **Texture Mapping** - TMEM structure in place, sampling not fully implemented
- ⏳ **Audio** - Audio interface not implemented
- ⏳ **Controller Input** - Input system not implemented
- ⏳ **Memory Management** - No TLB, cache, or accurate timing

## Renderer Architecture

The N64 RDP uses **OpenGL 3.3+ hardware-accelerated rendering exclusively**. The software renderer has been removed to simplify the codebase and ensure consistent GPU performance.

For detailed architecture documentation, see the **[Renderer Implementation Guidelines](../../../AGENTS.md#renderer-implementation)** section in AGENTS.md.

### OpenGL Renderer (Required)

**Location**: `src/rdp_renderer_opengl.rs`

**Status**: ✅ Production-ready, GPU-accelerated

**Features**:
- OpenGL 3.3 Core Profile with hardware acceleration
- Full triangle rendering (flat, Gouraud, textured, Z-buffered)
- Hardware depth testing via GPU Z-buffer
- Shader programs for different rendering modes
- Vertex buffers and efficient GPU submission
- Framebuffer readback for display
- TMEM (Texture Memory) support with tile descriptors

**Requirements**: 
- OpenGL 3.3+ compatible graphics hardware
- GL context provided by frontend (SDL2 in current implementation)

**Performance**: Optimized for GPU with minimal CPU overhead

### Architecture Changes (January 2026)

- ❌ **Software renderer removed** - Deleted `rdp_renderer_software.rs` (~850 lines)
- ✅ **OpenGL is mandatory** - Part of default features, always enabled
- ✅ **Simplified initialization** - RDP created directly with OpenGL renderer
- ✅ **GL context required** - `Rdp::new()` and `N64System::new()` require `glow::Context`
- ✅ **No renderer switching** - Renderer is fixed at system creation time

See `rdp.rs` for RDP command implementation details. For overall renderer architecture, see [AGENTS.md](../../../AGENTS.md#renderer-implementation).

## Building

### Default (OpenGL Renderer - Always Enabled)
```bash
cargo build --package emu_n64
```
```bash
cargo build --package emu_n64 --features opengl
```

## Testing

**Note**: N64 tests require an actual OpenGL context. Tests that create `N64System` are marked as `#[ignore]` and skipped in CI environments.

```bash
# Run all tests (non-GL tests only)
cargo test --package emu_n64

# Run all tests including GL-dependent tests (requires OpenGL 3.3+)
cargo test --package emu_n64 -- --ignored
```

**Test Status**: 43 tests pass (non-GL), 77 tests ignored (require OpenGL context)

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

// Create OpenGL context (example using glow)
let gl = unsafe {
    glow::Context::from_loader_function(|s| {
        // Your GL function loader here
        std::ptr::null()
    })
};

// Create system (requires GL context)
let mut n64 = N64System::new(gl)?;

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

**Note**: In production, the GL context is provided by the frontend (SDL2 in the GUI).

## Architecture

### Directory Structure
```
src/
  ├── lib.rs                    - Public API and System trait impl
  ├── bus.rs                    - Memory bus (RDRAM, PIF, cartridge)
  ├── cpu.rs                    - MIPS R4300i wrapper
  ├── rdp.rs                    - RDP state and display list processor
  ├── rdp_renderer.rs           - Renderer trait definition
  ├── rdp_renderer_opengl.rs    - OpenGL hardware renderer (required)
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
          ├── RDP ──> OpenGLRdpRenderer (required)
          ├── RSP (stub)
          └── VI (registers only)
```

## Performance

**OpenGL Renderer** (required):
- GPU-accelerated rasterization
- Hardware depth testing
- Efficient for complex 3D scenes
- Requires OpenGL 3.3+ compatible hardware

## Known Limitations

See [MANUAL.md](../../../docs/MANUAL.md#n64-nintendo-64) for the complete list of user-facing limitations.

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
2. **Write tests**: Add unit tests for new functionality (mark GL-dependent tests with `#[ignore]`)
3. **Document limitations**: Update `docs/MANUAL.md` when fixing issues
4. **GPU optimization**: OpenGL renderer provides hardware-accelerated performance

## References

- **Project Documentation**: See [README.md](../../../README.md), [MANUAL.md](../../../docs/MANUAL.md), and [AGENTS.md](../../../AGENTS.md)
- **RDP Commands**: Documented in `rdp.rs`
- **Test ROMs**: See `../../../test_roms/README.md`

## License

Same as the parent Hemulator project.
