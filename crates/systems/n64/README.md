# N64 Emulation - Nintendo 64 System

This crate implements Nintendo 64 emulation for the Hemulator project.

## Current Status

The N64 emulator is a **basic implementation** with functional RDP graphics processor supporting 3D triangle rendering. The emulator can execute test ROMs and render simple graphics, but full game compatibility requires additional work.

### What Works

- ✅ **MIPS R4300i CPU** - Complete instruction set implementation
  - All base instructions (load/store, arithmetic, branch, etc.)
  - CP0 coprocessor with TLB support
  - **TLB/MMU** - Full TLB implementation with CP0 integration
    - TLBWI (Write Indexed), TLBWR (Write Random)
    - TLBR (Read), TLBP (Probe)
    - 32-entry TLB with ASID support
    - Page sizes from 4KB to 16MB
- ✅ **Memory Bus** - 4MB RDRAM, PIF boot, SP memory, cartridge ROM
- ✅ **Audio Output** - AI (Audio Interface) with frontend integration
  - DMA transfer from RDRAM to audio buffer
  - 16-bit stereo PCM output at configurable sample rate
  - AI interrupt generation on DMA completion
  - Integrated with rodio audio backend
  - Sample rate control via AI_DACRATE register
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

- ⏳ **Cycle Accuracy** - Uses reduced cycle count (50,000 cycles/frame instead of hardware-accurate 1,562,500) for performance; frame-based timing, not cycle-accurate (configurable via `set_frame_cycles()`)
- ⏳ **RDP Commands** - Many commands stubbed or simplified:
  - `SET_OTHER_MODES` (0x2F) - Ignored (rendering mode configuration not applied)
  - Some texture formats not implemented (returns white for unknown formats)
  - Advanced blend/combine modes missing
  - Performance counters (DPC_CLOCK, DPC_BUFBUSY, DPC_PIPEBUSY, DPC_TMEM) return hardcoded zeros
- ⏳ **RSP Implementation** - High-Level Emulation only (no Low-Level Emulation):
  - Only F3DEX/F3DEX2 graphics commands implemented
  - Audio microcode tasks explicitly not implemented
  - Many F3DEX commands are stubs: G_MOVEWORD, G_MOVEMEM, G_SETOTHERMODE_L/H
  - Semaphore register always returns 0 (stub)
  - Signal bits (SIG0-SIG7) not implemented
  - No instruction-level execution (scalar/vector units not emulated)
- ⏳ **Save System** - No EEPROM, Flash, or Memory Card support:
  - Cartridge saves not implemented
  - Controller Pak (memory card) not supported
  - No persistent storage for game progress
- ⏳ **CPU Accuracy** - Some edge cases not fully implemented:
  - Overflow traps not implemented (uses wrapping arithmetic instead)
  - Memory alignment not validated (assumes properly aligned access)
  - Cache is direct-mapped only (no full coherency)

### Recent Improvements (January 2026)

- ✅ **Viewport Y-Axis Transformation Fix** (January 15, 2026) - Corrected viewport transformation in RSP HLE
  - Fixed incorrect Y-axis calculation in `clip_to_screen` function
  - Changed from `vp_y + (1.0 - ndc_y) * scale_y` to `vp_y + (ndc_y + 1.0) * scale_y`
  - Aligns with N64 standard viewport transformation: `screen = vtrans + ndc * vscale`
  - Fixes vertical positioning and orientation of rendered 3D triangles
  - Added comprehensive viewport transformation tests
- ✅ **TLB/MMU Integration** (January 13, 2026) - CP0 TLB instructions fully implemented and operational
  - **TLBWI** (TLB Write Indexed) - Write TLB entries at specific index
  - **TLBWR** (TLB Write Random) - Write TLB entries at random index  
  - **TLBR** (TLB Read) - Read TLB entries into CP0 registers
  - **TLBP** (TLB Probe) - Search TLB for matching entries
  - Full CP0 register integration (EntryHi, EntryLo0, EntryLo1, PageMask, Index)
  - Comprehensive logging for TLB operations
- ✅ **Audio Output Integration** (January 13, 2026) - AI module now connected to frontend audio backend
  - Audio samples stream from AI buffer to rodio audio output
  - 16-bit stereo PCM at configurable sample rate (typically 44.1kHz)
  - DMA-based audio transfer from RDRAM fully functional
  - AI interrupts integrated with MI (MIPS Interface)
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

See [User Manual](https://hemulator.56k.guru/user/systems.html#n64-nintendo-64) for the complete list of user-facing limitations.

**Main limitations preventing full game compatibility**:
1. **Cycle Accuracy** - Uses 50,000 cycles/frame (vs hardware's 1,562,500) for performance; may cause issues with precise timing-dependent games (configurable via `set_frame_cycles()`)
2. **RDP Incomplete** - Many commands stubbed or simplified (SET_OTHER_MODES ignored, some texture formats missing, performance counters return zeros)
3. **RSP HLE Only** - No instruction-level execution; only F3DEX graphics commands partially implemented; audio microcode not supported; many commands are stubs
4. **No Save System** - EEPROM, Flash, and Memory Card (Controller Pak) not implemented; games cannot save progress
5. **CPU Edge Cases** - Overflow traps not implemented (uses wrapping arithmetic); memory alignment not validated; cache is direct-mapped only

## Future Development

### Critical for Commercial Games
1. **RSP Microcode Expansion** - Implement missing F3DEX commands (G_MOVEWORD, G_MOVEMEM, G_SETOTHERMODE_*) and add audio microcode support
2. **Save System** - Implement EEPROM/Flash cartridge saves and Controller Pak (memory card) support
3. **RDP Command Completion** - Implement SET_OTHER_MODES and missing texture formats for proper rendering
4. **Cycle-Accurate Timing** - Improve timing precision for games that depend on it (optional enhancement - configurable system already in place)

### Nice to Have
1. RDP performance counters (DPC_CLOCK, DPC_BUFBUSY, DPC_PIPEBUSY, DPC_TMEM)
2. RSP semaphore and signal bits implementation
3. CPU overflow trap exceptions for signed arithmetic
4. Memory alignment validation
5. Full cache coherency (currently direct-mapped only)
6. Extended controller features (rumble, accessory support)

### Long Term
1. Low-Level RSP Emulation (instruction-level execution)
2. Complete RDP blend/combine pipeline
3. Full cache emulation
4. Game compatibility improvements

## Contributing

When adding features to the N64 emulator:

1. **Follow the renderer pattern**: Keep renderers separate from RDP state
2. **Write tests**: Add unit tests for new functionality (mark GL-dependent tests with `#[ignore]`)
3. **Document limitations**: Update the User Manual when fixing issues
4. **GPU optimization**: OpenGL renderer provides hardware-accelerated performance

### Common Pitfalls and Edge Cases

When working on the N64 emulator, be aware of these potential issues:

#### Sign Extension (CRITICAL)
The R4300i is a 64-bit CPU with many 32-bit operations. **Proper sign extension is essential**:

```rust
// ✅ Correct: Sign-extend 32-bit to 64-bit
let result = value as i32 as u64;  // First cast to i32 (sign extends), then to u64

// ❌ Incorrect: Zero-extends instead
let result = value as u64;  // Only for unsigned operations (LWU, LBU, etc.)
```

**Key locations**:
- Load Word (LW): Must sign-extend loaded value
- 32-bit arithmetic (ADD, SUB, MULT): Results must be sign-extended
- LUI instruction: Shifts into upper 16 bits, then sign-extends

#### Division by Zero
All division instructions **already handle division by zero correctly**:
- DIV, DIVU, DDIV, DDIVU check `divisor != 0`
- If divisor is 0, operation is skipped (HI/LO unchanged)
- Matches MIPS behavior (no trap, unpredictable result)

#### Overflow Traps (NOT IMPLEMENTED)
Signed arithmetic instructions (ADD, ADDI, SUB, etc.) **should trap on overflow** per MIPS spec:
- **Current behavior**: Uses `wrapping_add`/`wrapping_sub` (no trap)
- **Rationale**: Most N64 software doesn't rely on overflow traps
- **Unsigned variants** (ADDU, ADDIU, SUBU): Never trap (correct)

#### Memory Alignment
Load/store instructions have alignment requirements:
- **LH/SH**: 2-byte aligned
- **LW/SW**: 4-byte aligned
- **LD/SD**: 8-byte aligned
- **Current**: Not validated (most code is properly aligned)
- **Unaligned access**: Use LWL/LWR/LDL/LDR instructions

#### TLB Translation Edge Cases
The TLB implementation includes robust edge case handling:
- **Page mask overflow**: Limited to valid range (0x000-0xFFF)
- **Physical address overflow**: Ensures result fits in 32 bits
- **Invalid pages**: V=0 pages are skipped (TLB miss)
- **ASID matching**: Properly handles global vs. per-process entries

#### Register 0 Immutability
GPR[0] is hardwired to zero and **automatically enforced**:
- After every instruction: `self.gpr[0] = 0`
- No special handling needed in individual instructions

#### Shift Operations
All shift operations are **safe against overflow**:
- Uses Rust's `wrapping_shl`/`wrapping_shr`
- 32-bit shifts: Masked to 5 bits (0-31)
- 64-bit shifts: Masked to 6 bits (0-63)

### Testing Edge Cases

When adding new CPU instructions or memory operations:

1. **Test sign extension**: Verify negative values extend correctly
2. **Test division by zero**: Ensure safe handling
3. **Test alignment**: Consider both aligned and unaligned access
4. **Test TLB**: Verify page boundaries and ASID matching
5. **Test Register 0**: Verify it stays zero after all operations

Example test pattern:
```rust
#[test]
fn test_sign_extension_lw() {
    // Test that LW sign-extends negative values
    let val: u32 = 0x80000000; // Negative in signed representation
    let extended = val as i32 as u64;
    assert_eq!(extended, 0xFFFFFFFF80000000); // Should be sign-extended
}
```

## References

- **Project Documentation**: See [README.md](../../../README.md), [User Manual](https://hemulator.56k.guru/user/systems.html), and [AGENTS.md](../../../AGENTS.md)
- **RDP Commands**: Documented in `rdp.rs`
- **Test ROMs**: See `../../../test_roms/README.md`

## License

Same as the parent Hemulator project.
