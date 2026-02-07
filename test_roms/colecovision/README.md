# ColecoVision Test ROM

This directory contains a production-like test ROM for the ColecoVision emulator.

## Test ROM

### test.col - Multi-Color Band Test with Sprites

A comprehensive test ROM that demonstrates production-like VDP usage.

**Purpose:**
- Verify Z80 CPU functionality
- Test TMS9918A VDP Graphics II mode initialization
- Test pattern and color table loading
- Test nametable filling
- Test sprite rendering
- Demonstrate production-like VDP programming patterns

**Expected Output:**
- 256×192 pixel resolution
- 4 horizontal colored bands (each 6 rows of tiles = 48 pixels high):
  - Band 1 (rows 0-7): White text/pattern on black background
  - Band 2 (rows 8-15): Red text/pattern on black background
  - Band 3 (rows 16-23): Light green text/pattern on black background
  - Band 4 (rows 24+): Cyan text/pattern on black background (repeating)
- 2 sprites displayed:
  - Yellow sprite at position (100, 96)
  - Magenta sprite at position (156, 96)
- Black backdrop color

**Building:**
```bash
python3 build_rom.py
# or
./build.sh
```

## Requirements

- Python 3 (for ROM generation script)

## ROM Structure

The ROM uses the standard ColecoVision cartridge format:
- **Format**: Raw binary cartridge ROM
- **Size**: 32 KB
- **Load Address**: $8000 (ColecoVision cartridge space)
- **Mode**: TMS9918A Graphics II mode
- **Region**: NTSC

## Testing

Smoke tests for this ROM are included in `crates/systems/colecovision/src/lib.rs`:
- `smoke_test_colecovision()`: Tests VDP functionality with manual initialization
- `smoke_test_colecovision_with_rom_execution()`: Tests full ROM execution through BIOS

Run tests with:
```bash
cargo test --package emu_colecovision smoke_test
```

## Test BIOS

A minimal test BIOS (`test_bios.rom`) is provided for testing ROM execution. This BIOS:
- Initializes the stack pointer to $73FF
- Sets interrupt mode 1
- Jumps to cartridge ROM at $8000
- Provides proper interrupt handlers:
  - **$0038 (IM 1)**: Reads VDP status port to clear interrupt flag, re-enables interrupts with EI before RETI
  - **$0066 (NMI)**: Returns from NMI with RETN

Build the test BIOS with:
```bash
python3 build_test_bios.py
```

**Note**: This is a minimal BIOS for testing only. Real ColecoVision systems use a proprietary BIOS that cannot be distributed.

## Implementation Notes

### TMS9918A Graphics II Mode

The test ROM uses Graphics II mode, which is the most common mode for ColecoVision games:
- Tile-based background rendering
- 256×192 resolution
- Pattern table at $0000 (2KB for 256 tiles × 8 bytes each)
- Color table at $2000 (2KB, 8 bytes per tile for per-row colors)
- Nametable at $3800 (768 bytes for 32×24 tiles)
- Sprite attribute table at $3B00
- Sprite pattern table at $1800

### Graphics II Color Model

In Graphics II mode, each tile has independent color control per row:
- Each tile pattern row (8 pixels) has a color byte
- Color byte format: `[foreground_color:4][background_color:4]`
- This allows very flexible coloring (different colors per row within a tile)

### VDP Registers

The test ROM initializes these VDP registers:

| Register | Value | Purpose |
|----------|-------|---------|
| 0 | $00 | Mode control (Graphics II mode bit set by register 1) |
| 1 | $E2 | 16K VRAM, display on, frame interrupt, Graphics II mode |
| 2 | $0E | Nametable at $3800 |
| 3 | $7F | Color table at $2000 (Graphics II mode) |
| 4 | $03 | Pattern table at $0000 (Graphics II mode) |
| 5 | $76 | Sprite attribute table at $3B00 |
| 6 | $03 | Sprite pattern table at $1800 |
| 7 | $01 | Backdrop color: black |

### Sprite Format

Sprite attributes (4 bytes per sprite):
1. Y position (offset by +1, $D0 = end marker)
2. X position
3. Pattern number
4. Color code (0-15) and flags

Sprite patterns:
- 8×8 pixels, 8 bytes per sprite
- Each bit represents a pixel (1 = color, 0 = transparent)

### Color Palette (TMS9918A Standard)

| Code | Color | Code | Color |
|------|-------|------|-------|
| 0 | Transparent | 8 | Medium Red |
| 1 | Black | 9 | Light Red |
| 2 | Medium Green | A | Dark Yellow |
| 3 | Light Green | B | Light Yellow |
| 4 | Dark Blue | C | Dark Green |
| 5 | Light Blue | D | Magenta |
| 6 | Dark Red | E | Gray |
| 7 | Cyan | F | White |

## Production-Like Features

This test ROM demonstrates patterns used in real ColecoVision games:

1. **Graphics II Mode**: The most common mode for games (not Graphics I or Text mode)
2. **Per-Row Color Control**: Uses the Graphics II color table for flexible coloring
3. **Sprite Usage**: Shows how to set up and display sprites
4. **Frame Interrupts**: Enables VDP frame interrupts for timing
5. **Proper Initialization**: Follows the correct VDP initialization sequence
6. **Multiple Visual Elements**: Combines tiles and sprites together

## Comparison with Other Systems

Unlike the SMS test ROM which uses Mode 4 (Sega's enhanced VDP), this ColecoVision ROM uses the standard TMS9918A Graphics II mode, which is:
- More limited color palette (16 colors vs SMS's 64)
- Simpler but very flexible color model
- Fewer sprites per line (4 vs SMS's 8)
- Standard for all TMS9918A-based systems
