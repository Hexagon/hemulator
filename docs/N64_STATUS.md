# N64 Emulator Development Status

**Last Updated**: January 2, 2026  
**Status**: RSP microcode detection improved, texture commands added, 3D Pong test ROM created
**ROM Tested**: Enhanced test ROM with interrupt handling, 3D Pong test ROM, Super Mario 64 (8MB)

## Current State

### ✅ Working Components

- **ROM Loading**: Successfully loads 8MB commercial ROMs
- **IPL3 Boot**: Executes boot sequence, jumps to entry point (0x80000400)
- **CPU Execution**: MIPS R4300i core executes instructions correctly
- **GPR Initialization**: Stack pointer, return address, and other registers properly set
- **VI System**: Video Interface generates interrupts at scanline 256 every frame
- **Interrupt System**: IE bit enabled, IM3 configured for VI interrupts, exception handler uses ERET
- **Framebuffer**: Rendering pipeline functional (black background by default)
- **RDP Display Lists**: SET_FILL_COLOR and FILL_RECTANGLE commands working
- **RDP Texture Commands**: LOAD_TLUT, TEXTURE_RECTANGLE with texture sampling
- **RSP Microcode Detection**: CRC32-based signature matching for F3DEX/F3DEX2/Audio
- **Test ROMs**: Enhanced test ROM and 3D Pong test ROM both working

### 🔧 Partially Working

- **RSP Activity**: Basic HLE support exists, but microcode processing needs expansion
- **RDP Commands**: Basic commands work (fill, rectangles, triangles), textures need more work
- **Interrupt Handling**: Infrastructure in place, commercial ROMs may need tuning

### ❌ Not Working / Needs Improvement

- **RSP Microcode**: F3DEX/F3DEX2 detection and processing incomplete
- **PIF Controller Polling**: Commercial games may not poll controllers correctly
- **VI Configuration**: Commercial games may not fully configure VI registers
- **Texture Operations**: More texture formats and operations needed
- **Commercial ROM Progress**: Games still stuck in initialization (need RSP microcode)

## Recent Changes (January 2, 2026)

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

1. **Expand RSP Task DMA**: Implement proper task structure parsing from DMEM
2. **Add More RDP Commands**: SET_COMBINE_MODE usage, SET_Z_IMAGE, more texture formats
3. **Improve Texture Sampling**: Add filtering, mipmapping, texture wrapping modes
4. **Test with Commercial ROMs**: Verify improvements with Super Mario 64, other games
5. **Matrix Stack Management**: Improve G_POPMTX handling for nested display lists
6. **Display List Branching**: Full support for G_DL with proper call stack

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
