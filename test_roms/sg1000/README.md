# SG-1000 Test ROMs

This directory contains test ROMs for the Sega SG-1000 emulator.

## Test ROMs

### test.sg - Checkerboard Pattern Test
A minimal test ROM that displays a checkerboard pattern using the TMS9918A VDP in Graphics I mode.

**Purpose:**
- Verify basic Z80 CPU functionality
- Test TMS9918A VDP initialization (Graphics I mode)
- Test pattern table and name table loading
- Demonstrate basic rendering with 2 patterns

**Expected Output:**
- 256×192 pixel resolution
- Checkerboard pattern (alternating 0xAA and 0x55 patterns)
- White foreground on black background
- Approximately 50% white, 50% black pixels

**Building:**
```bash
./build.sh
# or
python3 build_rom.py
```

## Requirements

- Python 3 (for ROM generation script)

## ROM Structure

The test ROM uses the standard SG-1000 format:
- **Format**: Raw binary (no header)
- **Size**: 32 KB
- **Load Address**: 0x0000 (cartridge ROM space)
- **Graphics Mode**: TMS9918A Graphics I mode

## Testing

Smoke tests for this ROM are included in `crates/systems/sg1000/src/lib.rs`:
- `smoke_test_sg1000()`: Tests the checkerboard ROM

Run tests with:
```bash
cargo test --package emu_sg1000
```

## Implementation Notes

### TMS9918A Graphics I Mode
The test ROM uses Graphics I mode, the standard mode for SG-1000 games:
- Character/tile-based rendering
- 1 bit per pixel (2 colors per character)
- 256×192 resolution
- 32×24 character grid
- Pattern table at 0x0800
- Name table at 0x1800
- Color table at 0x2000

### VDP Register Setup
The ROM initializes the following VDP registers:
- **Register 0** (0x00): Graphics I mode, no external video
- **Register 1** (0xE0): 16K VRAM, display enabled, interrupts enabled
- **Register 2** (0x06): Name table at 0x1800
- **Register 3** (0xFF): Color table at 0x2000
- **Register 4** (0x01): Pattern table at 0x0800
- **Register 5** (0x36): Sprite attribute table at 0x1B00
- **Register 6** (0x07): Sprite pattern table at 0x3800
- **Register 7** (0xF4): White (F) on dark blue (4)

### Pattern Format
TMS9918A Graphics I mode uses 1-bit-per-pixel patterns:
- 8 bytes per 8×8 pattern
- Each bit represents a pixel
- Color determined by color table (one byte per 8 patterns)

Example patterns used:
```
Pattern 0: 0xAA 0xAA 0xAA 0xAA 0xAA 0xAA 0xAA 0xAA  # 10101010 pattern
Pattern 1: 0x55 0x55 0x55 0x55 0x55 0x55 0x55 0x55  # 01010101 pattern
```

These create a checkerboard effect when alternated in the name table.

### Color Table
The color table is set to white (F) on black (1) for all patterns, providing clear visual contrast.

## Compatibility

The SG-1000 shares the same core components as the ColecoVision:
- Z80A CPU @ 3.58 MHz
- TMS9918A VDP
- SN76489 PSG
- 1 KB RAM

This test ROM should work in any accurate SG-1000 or ColecoVision emulator when run in SG-1000 mode (no BIOS required).
