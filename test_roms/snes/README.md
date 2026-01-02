# SNES Test ROMs

This directory contains test ROMs for the SNES emulator.

## Test ROMs

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

On Ubuntu/Debian:
```bash
sudo apt-get install cc65
```

### Build all test ROMs
```bash
./build.sh
```

This will build both `test.sfc` and `test_enhanced.sfc`.

### Build individual ROMs
```bash
# Build test.sfc only
ca65 -t none --cpu 65816 test.s -o test.o
ld65 -C snes.cfg test.o -o test.sfc

# Build test_enhanced.sfc only
ca65 -t none --cpu 65816 test_enhanced.s -o test_enhanced.o
ld65 -C snes.cfg test_enhanced.o -o test_enhanced.sfc
```

## Running Tests

The test ROMs are automatically included in the unit tests:

```bash
# Run all SNES tests (includes smoke tests for both ROMs)
cargo test --package emu_snes

# Run specific smoke test
cargo test --package emu_snes test_snes_smoke_test_rom
cargo test --package emu_snes test_enhanced_rom
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

## Future Test ROMs

Potential additions:
- Priority bit test ROM (test BG tile priority bits)
- Sprite overflow test (>32 sprites per scanline)
- VRAM access timing test (access during/outside VBlank)
- Controller serial I/O test
- Mode 2-7 test ROMs (when implemented)
