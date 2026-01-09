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
  - Display list processing with comprehensive command support:
    - FILL_RECTANGLE (0x36) - Solid color rectangles
    - SET_FILL_COLOR (0x37) - Set fill color
    - SYNC_FULL (0x29) - Pipeline synchronization
    - SET_SCISSOR (0x2D) - Clipping rectangle
    - Triangle commands (0x08-0x0F) - Various triangle rendering modes
    - Texture commands (SET_TILE, SET_TEXTURE_IMAGE, LOAD_BLOCK, LOAD_TILE)
  - Proper RDRAM display list reading and execution
  - OpenGL framebuffer readback for display
- ✅ **Cartridge Loading** - Z64/N64/V64 formats with byte-order conversion
- ✅ **Save States** - Full state serialization
- ✅ **Controller Input** - Full 4-controller support via PIF
  - All 14 buttons (A, B, Z, Start, D-pad, L, R, C-buttons)
  - Analog stick with full range (-128 to 127 on X/Y axes)
  - Controller command protocol (read state, info)
  - Fully integrated with GUI keyboard mapping
- ✅ **RSP High-Level Emulation** - F3DEX/F3DEX2 graphics microcode
  - Complete vertex transformation pipeline
  - Matrix operations (modelview, projection)
  - Viewport mapping and frustum clipping
  - Triangle rendering with shading and Z-buffer
  - Display list parsing (20+ F3DEX commands implemented)
  - Lighting calculations (up to 8 lights)
- ✅ **Texture Mapping** - TMEM and tile descriptor support
  - 4KB texture memory
  - Multiple texture formats (RGBA16, RGBA32, CI4/8, IA, I)
  - Texture loading (LOAD_BLOCK, LOAD_TILE)
  - Texture sampling with wrapping/clamping
- ✅ **Debug Logging** - Comprehensive logging for RDP/RSP operations
- ✅ **Test ROMs** - Multiple test ROMs for validation

### What's Missing for Full Compatibility

- ⏳ **Audio Output Integration** - AI hardware module implemented (DMA transfer, sample rate control, interrupts, 16-bit stereo PCM), but connection to the frontend audio backend is still pending
- ⏳ **Memory Management** - TLB implemented (32-entry, ASID-aware), but CPU cache is still direct-mapped and CP0 TLB instructions/MMU behavior are not fully integrated
- ⏳ **Cycle Accuracy** - Uses reduced cycle count (50,000 cycles/frame instead of hardware-accurate 1,562,500) for performance; frame-based timing, not cycle-accurate
- ⏳ **Some RDP Commands** - Missing some advanced blend/combine modes
- ⏳ **RSP Microcode** - Only common F3DEX/F3DEX2 commands implemented (some games may use less common commands)

### Recent Improvements (January 2026)

- ✅ **Performance Optimization** (January 9, 2026) - Reduced frame cycles from 1,562,500 to 50,000 for ~30x better performance (~1fps → ~30-60fps)
  - **Additional optimizations**: Moved interrupt checking from per-instruction to per-scanline (~190x fewer checks)
  - **Configurable cycles**: Added `set_frame_cycles()` method for runtime performance tuning
  - **System-specific settings**: Frame cycles can be configured via `config.json` using `"n64_frame_cycles"` key
- ✅ **Enhanced RDP Logging** - Added INFO-level logging for debugging:
  - Display list processing shows command count and byte size
  - FILL_RECTANGLE operations log coordinates, dimensions, and color
  - SET_FILL_COLOR operations log color values
  - SYNC_FULL operations confirm pipeline synchronization
- ✅ **Better Debugging Visibility** - Critical operations moved to INFO level for runtime monitoring

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

The N64 emulator uses a **reduced cycle count** for practical performance:
- Hardware: 93.75 MHz (1,562,500 cycles/frame at 60Hz)
- Emulator: **50,000 cycles/frame** by default for ~30x better performance
- Maintains proper timing for interrupts and frame rendering
- Trade-off: Not cycle-accurate, may have timing issues with some games

### Performance Tuning

The frame cycles can be adjusted for different performance/accuracy trade-offs:

**Via Code**:
```rust
let mut n64 = N64System::new(gl)?;
n64.set_frame_cycles(100000); // Increase for better accuracy
```

**Via Configuration File** (`config.json`):
```json
{
  "n64_frame_cycles": 100000
}
```

**Recommended Values**:
- **50,000** (default): Best performance, good for most games
- **100,000**: Better accuracy, still good performance
- **200,000**: Higher accuracy, moderate performance impact
- **1,562,500**: Hardware-accurate, very slow (~1fps)

### Additional Optimizations

- **Interrupt checking**: Optimized to once per scanline (262 times/frame) instead of per instruction (~50,000 times/frame)
- **This allows higher frame_cycles values** with less performance impact

**OpenGL Renderer** (required):
- GPU-accelerated rasterization
- Hardware depth testing
- Efficient for complex 3D scenes
- Requires OpenGL 3.3+ compatible hardware

## Known Limitations

See [MANUAL.md](../../../docs/MANUAL.md#n64-nintendo-64) for the complete list of user-facing limitations.

**Main limitations preventing full game compatibility**:
1. **Audio Output** - AI hardware implemented but frontend audio output integration pending
2. **Memory Management** - TLB implemented but cache is direct-mapped and CP0 TLB/MMU integration incomplete
3. **Cycle Accuracy** - Uses 50,000 cycles/frame (vs hardware's 1,562,500) for performance; may cause issues with precise timing-dependent games
4. **Missing RDP Commands** - Some advanced blend/combine modes not implemented
5. **RSP Coverage** - HLE works for common F3DEX commands but may not cover all microcode variants

## Future Development

### Critical for Commercial Games
1. **Audio Output Integration** - Connect AI module to frontend audio backend (SDL2/rodio)
2. **CP0 TLB/MMU Integration** - Wire TLB instructions (TLBWI, TLBWR, TLBR, TLBP) to CPU
3. **Cycle-Accurate Timing** - Improve timing precision for games that depend on it

### Nice to Have
1. Additional RDP blend/combine modes for advanced graphics effects
2. Expand RSP HLE coverage for less common microcode variants
3. Memory card support in PIF
4. EEPROM save data support
5. Extended controller features (e.g., rumble, accessory support)

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
