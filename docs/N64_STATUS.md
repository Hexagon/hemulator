# N64 Emulator Development Status

**Last Updated**: January 3, 2026  
**Status**: Matrix transformation fixed, 3D geometry rendering working correctly
**ROM Tested**: Enhanced test ROM (WORKING), 3D Pong test ROM (WORKING - red/blue paddles + green ball), Super Mario 64 (NOT RENDERING)

## Current State

### ✅ Working Components

- **ROM Loading**: Successfully loads 8MB commercial ROMs
- **IPL3 Boot**: Executes boot sequence, jumps to entry point (0x80000400), writes ERET exception handler
- **CPU Execution**: MIPS R4300i core executes instructions correctly
- **ERET Instruction**: Properly clears EXL bit to re-enable interrupts (FIXED Jan 3)
- **GPR Initialization**: Stack pointer, return address, and other registers properly set
- **VI System**: Video Interface generates interrupts at configured scanline
- **Interrupt System**: Proper MI-based interrupt routing, interrupts only set once until acknowledged (FIXED Jan 3)
- **Framebuffer**: Rendering pipeline functional
- **RDP Display Lists**: SET_FILL_COLOR, FILL_RECTANGLE, SYNC_FULL commands working
- **RDP Texture Commands**: LOAD_TLUT, TEXTURE_RECTANGLE with texture sampling
- **Enhanced Test ROM**: Renders red and green rectangles correctly (direct RDP commands)
- **Virtual Address Translation**: KSEG0/KSEG1 to physical address conversion (FIXED Jan 3)
- **RSP Task Structure Reading**: Can read task structure from RDRAM at 0x00200000 (FIXED Jan 3)
- **F3DEX Display List Parsing**: Correctly parses display lists with virtual addresses (FIXED Jan 3)
- **Matrix Transformation**: Column-major matrix support for proper 3D transformation (FIXED Jan 3)
- **3D Pong ROM**: Renders all three objects correctly with proper colors (WORKING - Jan 3)

### 🔧 Partially Working

- **RSP HLE**: Task structure reading works, display list parsing works, 3D rendering works for simple test ROMs
- **RDP Commands**: Basic commands work (fill, rectangles, triangles with shading and Z-buffer)
- **Interrupt Handling**: Core infrastructure working

### ❌ Not Working / Needs Improvement

- **Commercial ROM Graphics**: RSP processing needs more work for complex games (more F3DEX commands, lighting, etc.)
- **Frustum Clipping**: Currently using simple NDC clamping, needs proper clipping
- **PIF Controller Polling**: Controller input not yet tested with running games
- **Texture Operations**: More texture formats and operations needed
- **Viewport Configuration**: Using hardcoded 320x240, needs dynamic viewport from G_MOVEMEM commands

## Recent Changes (January 3, 2026)

### 1. Matrix Transformation Fix - Column-Major Support
**Files**: `crates/systems/n64/src/rsp_hle.rs`

**Problem**: The RSP HLE was treating N64 matrices as row-major, but the N64 hardware and test ROMs use column-major matrix layout. This caused incorrect vertex transformations - all vertices were being mapped to extreme coordinates, resulting in a solid green screen from the ball's triangles covering the entire framebuffer.

**Fix**: 
- Updated `transform_vertex()` to use column-major indexing: `matrix[row + col*4]`
- Updated `multiply_matrix()` to perform column-major matrix multiplication
- Added NDC clamping to prevent coordinate overflow: x/y clamped to [-10, 10], z to [-1, 1]
- Updated tests to reflect the corrected transformation behavior

**Result**: 
- 3D Pong ROM now renders correctly with all three objects visible:
  - Left paddle: 646 red pixels
  - Right paddle: 646 blue pixels  
  - Ball: 133 green pixels
- Proper perspective projection and vertex transformation working
- All 126 tests still pass

---

### 2. Virtual-to-Physical Address Conversion Fix
**Files**: `crates/systems/n64/src/rsp_hle.rs`

**Problem**: F3DEX display list commands contain virtual addresses (0x80xxxxxx for KSEG0), but the code was treating them as physical addresses when indexing into RDRAM. This caused out-of-bounds access and prevented display lists, vertices, and matrices from being loaded correctly.

**Fix**: 
- Added `virt_to_phys()` helper function to convert N64 virtual addresses to physical addresses
- Applied conversion in `parse_f3dex_display_list()` for display list addresses
- Applied conversion in `load_vertex()` for vertex data addresses
- Applied conversion in `load_matrix_from_rdram()` for matrix data addresses

**Result**: 
- RSP can now correctly read task structures from RDRAM
- F3DEX display lists are parsed successfully
- Pong3D ROM now produces visible output (green screen - partial rendering)
- All 126 tests still pass

---

### 2. Enhanced Logging for RSP Task Processing
**Files**: `crates/systems/n64/src/rsp_hle.rs`

**Changes**:
- Added detailed logging for task structure reading from DMEM and RDRAM
- Added logging for display list parsing with virtual and physical addresses
- Added logging for RDP output buffer processing
- Added warnings when no display list is found

**Result**: Better debugging visibility into RSP task processing

---

### 3. Critical ERET Bug Fix
**Files**: `crates/core/src/cpu_mips_r4300i.rs`

**Problem**: ERET instruction was not clearing the EXL (Exception Level) bit in the Status register, keeping the CPU permanently in exception mode and preventing interrupts from being delivered.

**Fix**: Modified ERET to clear EXL bit (bit 1) when returning from exception:
```rust
// Clear EXL bit to re-enable interrupts
self.cp0[CP0_STATUS] &= !0x02;
```

**Result**: CPU can now properly return from exception handlers and re-enable interrupts.

---

### 2. VI Interrupt Loop Fix
**Files**: `crates/systems/n64/src/lib.rs`, `crates/systems/n64/src/mi.rs`

**Problem**: VI interrupt was being set directly in CPU Cause register every frame, bypassing MI interrupt controller. Once set, it was never cleared, causing infinite exception loops after ERET.

**Fix**: 
- Changed to only set interrupt in MI (not CPU directly)
- Only set interrupt once per occurrence (check if already pending)
- Let MI masking logic handle delivery to CPU
- Added `get_interrupt_status()` helper method to MI

**Result**: Interrupts now work correctly - set once, handled once, cleared properly.

---

### 3. Enhanced Test ROM Rendering Success
**Status**: ✅ WORKING

The enhanced test ROM (test_enhanced.z64) now renders correctly:
- Red rectangle at (50,50) to (150,150)
- Green rectangle at (160,90) to (210,140)
- Confirms RDP command processing works
- Confirms interrupt handling works
- ROM writes RDP commands directly to RDRAM, bypassing RSP

---

### 4. RSP HLE Task Structure Issue (IN PROGRESS)
**Files**: `crates/systems/n64/src/rsp_hle.rs`

**Problem**: Pong3D and commercial ROMs don't produce output. RSP HLE reads task structure from DMEM, but test ROMs write it to RDRAM (0x00200000) without DMA transfer.

**Investigation**:
- Pong3D ROM sets up task structure at physical address 0x00200000
- HLE tries to read from DMEM (expecting DMA to have copied it there)
- Added fallback to read directly from RDRAM at 0x00200000
- Added logging to debug what's actually in memory
- Need to verify task structure is actually being written and read correctly

**Next Steps**:
- Verify ROM actually executes task structure setup code
- Check if data_ptr values are correct
- Ensure F3DEX display list processing triggers correctly
- May need to add automatic microcode detection when valid display list found

---

## Previous Changes (January 2, 2026)

### 1. RSP Microcode Detection Enhancement
**Files**: `crates/systems/n64/src/rsp_hle.rs`

**Changes**:
- Implemented CRC32-based microcode signature matching
- Added known CRC32 signatures for F3DEX, F3DEX2, and Audio microcodes
- Fallback to heuristic pattern matching for unknown microcodes
- Improved logging of microcode detection results

**Result**: RSP can now accurately detect different microcode variants used by commercial games

---

### 2. Texture Commands Implementation
**Files**: `crates/systems/n64/src/rdp.rs`

**Changes**:
- Added LOAD_TLUT (0x30) command for loading color palettes for CI textures
- Implemented proper TEXTURE_RECTANGLE (0x24) with texture sampling
- Enhanced texture sampling to check for valid TMEM data before rendering
- Fixed texture coordinate mapping for rectangle rendering

**Result**: Textured rectangles now render correctly, palette-based textures supported

---

### 3. 3D Pong Test ROM
**Files**: `test_roms/n64/pong3d_test.py`, `test_roms/n64/test_pong3d.z64`

**Features**:
- Complete 3D game ROM using F3DEX display lists
- Three game objects: left paddle (red), right paddle (blue), ball (green)
- Perspective projection matrix (60° FOV, 4:3 aspect ratio)
- Camera translation matrix (moved back 300 units)
- Gouraud shading with vertex colors
- F3DEX commands: G_VTX, G_TRI2, G_MTX, G_ENDDL
- Comprehensive test validates RSP integration and rendering pipeline

**Result**: First fully 3D test ROM demonstrating the complete RSP/RDP pipeline

---

### 4. Interrupt System Enable
**Files**: `crates/systems/n64/src/cpu.rs`, `crates/systems/n64/src/pif.rs`

**Changes**:
- Set IE bit in CP0_STATUS_COMMERCIAL_BOOT (0x34000000 → 0x34000801)
- Enabled IM3 (bit 11) for VI interrupt routing
- Changed exception handler from infinite loop (`j 0x80000180`) to ERET (`0x42000018`)

**Result**: Interrupts now properly delivered to CPU, exception handler returns gracefully

---

### 5. Enhanced Test ROM
**File**: `test_roms/n64/enhanced_test.py`, `test_roms/n64/test_enhanced.z64`

**Features**:
- Configures MI_INTR_MASK to enable VI interrupts
- Sets VI_INTR to trigger on scanline 100
- Main loop polls MI_INTR for interrupt status
- Triggers RDP to render red and green rectangles
- Behaves like commercial ROM boot sequence

**Test Coverage**: New test `test_enhanced_rom_interrupts` validates interrupt setup and rendering

---

### 6. Framebuffer Initialization
**File**: `crates/systems/n64/src/rdp.rs`

**Changes**: Reverted dark blue initialization (0xFF000040) back to black (0x00000000) for test compatibility

**Result**: All 126 tests pass without false failures

## Solution Path Progress

### ✅ Completed: Enable Interrupts
- IE bit enabled in CP0_STATUS
- IM3 configured for VI interrupt line 3
- Exception handler uses ERET instead of infinite loop
- Enhanced test ROM validates interrupt flow

### 🚧 In Progress: RSP & RDP Expansion
**Next priorities**:
1. Expand RSP HLE microcode processing
2. Implement F3DEX/F3DEX2 microcode detection and execution
3. Add missing texture formats (CI, IA, I variants)
4. Improve RDP command coverage

## Testing Strategy

All tests passing (126/126):
```
cargo test --package emu_n64 --lib
```

Test categories:
- ✅ System creation and reset (5 tests)
- ✅ ROM loading and boot sequence (3 tests)
- ✅ RDP rendering (fill, triangles, textures, Z-buffer) (36+ tests)
- ✅ Interrupt flow (VI, MI, CPU integration) (6 tests)
- ✅ Controller input (PIF, multi-player) (5 tests)
- ✅ Enhanced ROM with interrupts (1 test)
- ✅ 3D Pong ROM with F3DEX display lists (1 test)
- ✅ Component tests (MI, VI, RSP, cartridge) (69+ tests)

## Known Limitations

### Commercial ROM Boot
Commercial ROMs (e.g., Super Mario 64) still don't progress past initialization because:
1. **RSP Microcode**: Games load F3DEX/F3DEX2 microcode and expect it to process geometry
2. **Display List Processing**: RSP needs to convert high-level commands to RDP primitives
3. **DMA Operations**: RSP DMA from RDRAM to IMEM/DMEM needs verification
4. **Task Processing**: RSP task structures need proper parsing and execution

### What's Needed for Commercial ROMs
1. **RSP Microcode Interpreter**: Detect F3DEX/F3DEX2 and execute display list commands
2. **Geometry Processing**: Transform vertices, apply matrices, output triangles
3. **Texture Handling**: Load textures to TMEM, sample during rendering
4. **More RDP Commands**: SET_COMBINE_MODE, SET_Z_IMAGE, SET_SCISSOR, etc. (partially done)

## Files Modified This Session

| File | Purpose |
|------|---------|
| `crates/systems/n64/src/cpu.rs` | Enable IE bit and IM3 in CP0_STATUS |
| `crates/systems/n64/src/pif.rs` | Change exception handler to ERET |
| `crates/systems/n64/src/rdp.rs` | Revert framebuffer init to black |
| `crates/systems/n64/src/lib.rs` | Update exception vector test, add enhanced ROM test |
| `test_roms/n64/enhanced_test.py` | Build script for enhanced test ROM |
| `test_roms/n64/test_enhanced.z64` | Enhanced test ROM binary |

## Next Session Priorities

1. **Improve Frustum Clipping**: Replace simple NDC clamping with proper view frustum clipping
   - Clip triangles against near/far planes before rasterization
   - Prevent artifacts from vertices behind the camera
   - Generate new vertices at clip plane intersections
2. **Add More F3DEX Commands**: Implement missing display list commands for commercial ROM support
   - G_RDPHALF_2, G_RDPHALF_CONT for split RDP commands
   - G_LOAD_UCODE for dynamic microcode switching
   - G_CLEARGEOMETRYMODE, G_SETGEOMETRYMODE improvements
3. **Implement Lighting**: Add support for N64's lighting system
   - G_MOVEMEM for loading light data
   - Ambient, directional, and point lights
   - Per-vertex lighting calculations
4. **Add More RDP Commands**: SET_COMBINE_MODE for blending, SET_Z_IMAGE, more texture formats
5. **Improve Texture Sampling**: Add filtering, mipmapping, texture wrapping modes
6. **Test with Commercial ROMs**: Verify improvements with Super Mario 64, other games
7. **Display List Branching**: Full support for G_DL with proper call stack

## Reference Documentation

- **ARCHITECTURE.md** - System overview, renderer patterns
- **AGENTS.md** - Build instructions, pre-commit checks
- **N64 README** - N64-specific implementation details
- **CPU Reference** - `docs/references/cpu_mips_r4300i.md`

## Debug Commands

### Build and test:
```bash
cargo build --profile release-quick
cargo test --package emu_n64 --lib
```

### Run with logging:
```bash
cargo run --profile release-quick -- rom.z64 --log-interrupts info --log-ppu info --log-file n64.log
```

### Check for key events:
```bash
grep -i "interrupt\|RSP\|DMA\|Microcode" n64.log
```
