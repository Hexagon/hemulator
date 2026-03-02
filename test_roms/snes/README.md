# SNES Test ROMs

This directory contains test ROMs for the SNES emulator.

## ⚠️ REBUILD REQUIRED

**The following test ROMs need to be rebuilt** after fixing the PPU bitplane addressing:

- `test.sfc` - Updated source in `test.s` to use correct interleaved 2bpp format
- `test_simple_sprite.sfc` - Needs source update for correct interleaved 4bpp format

**What changed**: The PPU was fixed to use the correct SNES bitplane layout:
- 2bpp: BP0,BP1 interleaved per row (not sequential BP0[0-7], BP1[8-15])
- 4bpp: BP0,BP1 pairs in bytes 0-15, BP2,BP3 pairs in bytes 16-31
- 8bpp: Four pairs of interleaved bitplanes in 64 bytes

**To rebuild after installing cc65**:
```bash
cd test_roms/snes
./build.sh  # or manually: ca65 test.s -o test.o && ld65 -C snes.cfg test.o -o test.sfc
```

**For test_simple_sprite.s**, update the SPRITE_TILE_DATA section to use interleaved 4bpp format:
```asm
; Correct SNES 4bpp format: BP0,BP1 pairs (bytes 0-15), BP2,BP3 pairs (bytes 16-31)
; For color 1 (BP0=1, BP1=0, BP2=0, BP3=0):
SPRITE_TILE_DATA:
    ; Bytes 0-15: BP0,BP1 pairs for rows 0-7
    .byte $FF, $00  ; Row 0: BP0=$FF, BP1=$00
    .byte $FF, $00  ; Row 1
    .byte $FF, $00  ; Row 2
    .byte $FF, $00  ; Row 3
    .byte $FF, $00  ; Row 4
    .byte $FF, $00  ; Row 5
    .byte $FF, $00  ; Row 6
    .byte $FF, $00  ; Row 7
    ; Bytes 16-31: BP2,BP3 pairs for rows 0-7
    .byte $00, $00  ; Row 0: BP2=$00, BP3=$00
    .byte $00, $00  ; Row 1
    .byte $00, $00  ; Row 2
    .byte $00, $00  ; Row 3
    .byte $00, $00  ; Row 4
    .byte $00, $00  ; Row 5
    .byte $00, $00  ; Row 6
    .byte $00, $00  ; Row 7
```

---

## Test ROMs

### cputest-basic.sfc
**Purpose**: Comprehensive 65C816 CPU instruction test (basic subset)

**Source**: [gilyon/snes-tests](https://github.com/gilyon/snes-tests) — MIT license

**Features tested** (~1107 tests):
- All 65C816 opcodes (except STP and WAI)
- Each instruction in all supported addressing modes
- Edge cases and correct flag results
- Does **not** include emulation-mode (6502 compatibility) wrapping edge cases

**Result detection**:
- ROM writes "Success" or "Failed" to VRAM word address `$32` (byte offset `$64`)
  - `b'S'` (0x53) = all tests passed
  - `b'F'` (0x46) = a test failed; the failing test number is in WRAM at `$0010`
- Current test number stored as a little-endian `u16` at WRAM address `$0010`

**Why this is important**: Verifies that the 65C816 CPU core executes every instruction
correctly, catching subtle bugs in addressing modes, flag computation, and mode switching.

---

### cputest-full.sfc
**Purpose**: Comprehensive 65C816 CPU instruction test (full suite)

**Source**: [gilyon/snes-tests](https://github.com/gilyon/snes-tests) — MIT license

**Features tested** (~1610 tests):
- Everything in `cputest-basic.sfc`, plus
- Emulation-mode (E=1) wrapping behavior
- Undocumented addressing-mode quirks documented in the README

---

### test.sfc
**Purpose**: Basic smoke test for Mode 0 rendering

**Features tested**:
- Mode 0 (4-layer 2bpp)
- Basic tilemap setup
- Simple palette configuration
- Checkerboard pattern rendering
- VRAM/CGRAM writes

**Expected output**: A checkerboard pattern with alternating blue and red tiles

---

### test_enhanced.sfc
**Purpose**: Comprehensive test for Mode 1 and features used by commercial games

**Features tested**:
- Mode 1 (most common in commercial games)
  - BG1: 4bpp (16 colors)
  - BG2: 4bpp (16 colors)
  - BG3: 2bpp (4 colors)
- Sprite rendering (8x8 sprites at specific positions)
- NMI handling and interrupt system
- Auto-joypad read enable ($4200)
- Multiple BG layers with different tile sets
- Scrolling (BG1 scrolls horizontally each frame)
- Force blank during initialization
- Typical commercial ROM initialization sequence

**Expected output**:
- BG1: Horizontal color stripes (white, red, blue)
- BG2: Vertical stripes (alternating colors)
- BG3: Solid light blue background
- Sprites: Two sprites visible at (64, 64) and (128, 64)
- BG1 scrolls slowly to the left

**Why this is important**: This ROM mimics the initialization and features that real commercial SNES games use, making it a better test for compatibility.

## Building

### Requirements
- `cc65` toolchain (includes `ca65` assembler and `ld65` linker)
- Python 3 (for the cputest ROM generator)

On Ubuntu/Debian:
```bash
sudo apt-get install cc65
```

### Build all test ROMs
```bash
./build.sh
```

This builds all ROMs including `cputest-basic.sfc` and `cputest-full.sfc`.

### Build individual ROMs
```bash
# Build test.sfc only
ca65 -t none --cpu 65816 test.s -o test.o
ld65 -C snes.cfg test.o -o test.sfc

# Build test_enhanced.sfc only
ca65 -t none --cpu 65816 test_enhanced.s -o test_enhanced.o
ld65 -C snes.cfg test_enhanced.o -o test_enhanced.sfc

# Build cputest-basic.sfc (basic 65C816 tests)
# Note: cputest_font.bin is excluded from git (*.bin rule). Download it from
# https://github.com/gilyon/snes-tests/blob/main/cputest/font.bin first.
# Note: font.bin is excluded from git (*.bin rule). Download it from
# https://github.com/gilyon/snes-tests/blob/main/cputest/font.bin and save it as
# font.bin in this directory (test_roms/snes/) before building.
python3 make_cpu_tests.py --basic
ca65 -D basic cputest_main.asm -o cputest-basic.o
ld65 -C cputest_lorom.cfg -o cputest-basic.sfc cputest-basic.o

# Build cputest-full.sfc (full 65C816 tests including emulation mode)
python3 make_cpu_tests.py
ca65 cputest_main.asm -o cputest-full.o
ld65 -C cputest_lorom.cfg -o cputest-full.sfc cputest-full.o
```

## Running Tests

The test ROMs are automatically included in the unit tests:

```bash
# Run all SNES tests (includes smoke tests for all ROMs)
cargo test --package emu_snes

# Run specific smoke test
cargo test --package emu_snes test_snes_smoke_test_rom
cargo test --package emu_snes test_enhanced_rom
cargo test --package emu_snes test_priority_rom
cargo test --package emu_snes test_sprite_overflow_rom

# Run 65C816 CPU instruction tests (gilyon/snes-tests)
cargo test --package emu_snes test_cputest_basic_loads_and_runs
cargo test --package emu_snes test_cputest_full_loads_and_runs
```

## ROM Format

Both ROMs use:
- **Format**: LoROM
- **Size**: 32KB
- **Header**: Internal header at $FFB0-$FFDF
- **Vectors**: At $FFE0-$FFFF (native mode) and $FFF0-$FFFF (emulation mode)

## Technical Details

### Addressing
- VRAM uses word addressing (multiply by 2 for byte address)
- CHR base addresses use bits shifted by 13 (multiply by 8192)
- Tilemap base addresses use bits shifted by 11 (multiply by 2048)

### Palette Format
- 15-bit BGR format: `0bbbbbgg gggrrrrr`
- Color 0 in each palette is transparent
- Sprite palettes start at color 128

### Tile Format
- 2bpp: 16 bytes per tile (2 bitplanes × 8 rows)
- 4bpp: 32 bytes per tile (4 bitplanes × 8 rows)
- Bitplanes are interleaved in memory

## Troubleshooting

**Q: Test ROM doesn't build**
- Ensure `cc65` is installed: `which ca65`
- Check for syntax errors in the .s files
- Make sure you're running from the test_roms/snes directory

**Q: Test fails but ROM builds successfully**
- Check if the expected output matches what's being rendered
- Look at frame dimensions (should be 256x224)
- Verify non-black pixel count
- Enable SNES logging: `--log-ppu debug --log-cpu debug`

---

### test_priority.sfc
**Purpose**: Test BG tile priority bit handling

**Features tested**:
- Mode 0 rendering
- Priority bit in tile attributes (bit 13)
- Correct priority ordering (high-priority tiles render in front of low-priority tiles)
- Alternating low/high priority tiles across screen

**Expected output**:
- Checkerboard pattern with alternating red (low priority) and blue (high priority) tiles
- All tiles should be visible (priority system working correctly)

**Why this is important**: Commercial games rely heavily on priority bits for layering effects, HUDs, and text overlays.

---

### test_sprite_overflow.sfc
**Purpose**: Test sprite-per-scanline limits

**Features tested**:
- Sprite rendering with overflow conditions
- 32 sprite per scanline hardware limit
- Sprite culling when limit exceeded
- All 128 sprites positioned on same scanline (Y=100)

**Expected output**:
- Horizontal row of sprites at Y=100
- Only first 32 sprites should be visible (rest culled)
- Emulator should not crash or freeze

**Why this is important**: SNES hardware has strict limits that commercial games depend on. Without proper overflow handling, sprite glitches and crashes can occur.

---

## Future Test ROMs

Potential additions:
- ~~Priority bit test ROM (test BG tile priority bits)~~ ✅ Done
- ~~Sprite overflow test (>32 sprites per scanline)~~ ✅ Done
- ~~65C816 CPU instruction tests~~ ✅ Done (cputest-basic/full from gilyon/snes-tests)
- VRAM access timing test (access during/outside VBlank)
- Controller serial I/O test
- Mode 2-7 test ROMs (when implemented)
