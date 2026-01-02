# N64 Emulator Development Status

**Last Updated**: January 2, 2026  
**Status**: Interrupts enabled, enhanced test ROM working, ready for RSP/RDP expansion  
**ROM Tested**: Enhanced test ROM with interrupt handling, Super Mario 64 (8MB)

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
- **Test ROM**: Enhanced test ROM with interrupt handling passes all tests

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

### 1. Interrupt System Enable
**Files**: `crates/systems/n64/src/cpu.rs`, `crates/systems/n64/src/pif.rs`

**Changes**:
- Set IE bit in CP0_STATUS_COMMERCIAL_BOOT (0x34000000 → 0x34000801)
- Enabled IM3 (bit 11) for VI interrupt routing
- Changed exception handler from infinite loop (`j 0x80000180`) to ERET (`0x42000018`)

**Result**: Interrupts now properly delivered to CPU, exception handler returns gracefully

---

### 2. Enhanced Test ROM
**File**: `test_roms/n64/enhanced_test.py`, `test_roms/n64/test_enhanced.z64`

**Features**:
- Configures MI_INTR_MASK to enable VI interrupts
- Sets VI_INTR to trigger on scanline 100
- Main loop polls MI_INTR for interrupt status
- Triggers RDP to render red and green rectangles
- Behaves like commercial ROM boot sequence

**Test Coverage**: New test `test_enhanced_rom_interrupts` validates interrupt setup and rendering

---

### 3. Framebuffer Initialization
**File**: `crates/systems/n64/src/rdp.rs`

**Changes**: Reverted dark blue initialization (0xFF000040) back to black (0x00000000) for test compatibility

**Result**: All 125 tests pass without false failures

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

All tests passing (125/125):
```
cargo test --package emu_n64 --lib
```

Test categories:
- ✅ System creation and reset (5 tests)
- ✅ ROM loading and boot sequence (3 tests)
- ✅ RDP rendering (fill, triangles, Z-buffer) (35+ tests)
- ✅ Interrupt flow (VI, MI, CPU integration) (6 tests)
- ✅ Controller input (PIF, multi-player) (5 tests)
- ✅ Enhanced ROM with interrupts (1 test)
- ✅ Component tests (MI, VI, RSP, cartridge) (70+ tests)

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

1. **Expand RSP HLE**: Implement more F3DEX commands (G_VTX, G_TRI1, G_MATRIX, etc.)
2. **Microcode Detection**: Parse RSP IMEM to identify F3DEX vs F3DEX2
3. **Display List Processing**: Process task structures and generate RDP commands
4. **Texture Loading**: Implement LOAD_BLOCK and LOAD_TILE properly
5. **Test with Commercial ROM**: Verify improvements with Super Mario 64

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
