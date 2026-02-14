# SMS Test ROMs

This directory contains test ROMs for the Sega Master System emulator.

## Test ROMs

### test.sms - Basic Checkerboard Test
A minimal test ROM that displays a simple checkerboard pattern.

**Purpose:**
- Verify basic Z80 CPU functionality
- Test VDP Mode 4 initialization
- Test simple tile pattern loading
- Demonstrate basic rendering with 2 colors

**Expected Output:**
- 256×192 pixel resolution
- Checkerboard pattern (50% white, 50% black)
- Uses tiles 0 (white) and 1 (transparent/black backdrop)

**Building:**
```bash
python3 build_rom.py
```

### test_enhanced.sms - Multi-Color Band Test
An enhanced test ROM that demonstrates multi-color rendering.

**Purpose:**
- Test Mode 4 with full register initialization
- Verify CRAM (Color RAM) palette loading
- Test multiple tile patterns with different colors
- Demonstrate production-like VDP usage

**Expected Output:**
- 256×192 pixel resolution
- Blue backdrop color
- Four horizontal colored bands:
  - Rows 0-5: White (25% of screen)
  - Rows 6-11: Red (25% of screen)
  - Rows 12-17: Green (25% of screen)
  - Rows 18-23: Blue (25% of screen)

**Building:**
```bash
python3 build_enhanced.py
```

## Requirements

- Python 3 (for ROM generation scripts)

## ROM Structure

Both ROMs use the standard SMS format:
- **Format**: SMS ROM with TMR SEGA header
- **Size**: 32 KB
- **Header**: TMR SEGA signature at 0x7FF0
- **Mode**: Mode 4 (SMS native graphics mode)
- **Region**: Export (NTSC)

## Testing

Smoke tests for these ROMs are included in `crates/systems/sms/src/system.rs`:
- `smoke_test_sms()`: Tests the basic checkerboard ROM
- `test_enhanced_rom()`: Tests the multi-color band ROM

Run tests with:
```bash
cargo test --package emu_sms
```

## Implementation Notes

### VDP Mode 4
Both test ROMs use Mode 4, the native SMS graphics mode:
- Tile-based background rendering
- 4 bits per pixel (16 colors per tile)
- 256×192 resolution
- Name table at 0x3800 (register 2 = 0x0E)
- Sprite table at 0x3F00 (register 5 = 0x7E)

### Color Palette (CRAM)
The enhanced test ROM sets up a 17-entry palette:
- Entries 0-15: Tile colors (black, white, red, green, blue, etc.)
- Entry 16: Backdrop color (blue)

### Tile Format
SMS Mode 4 uses 4-bit-per-pixel tiles:
- 32 bytes per 8×8 tile
- 4 bytes per row (one byte per bit plane)
- Pixel value = bit0 | (bit1<<1) | (bit2<<2) | (bit3<<3)

Example for a solid white tile (pixel value 1):
```
Row 0: 0xFF, 0x00, 0x00, 0x00  # All pixels = 0b0001 = 1
Row 1: 0xFF, 0x00, 0x00, 0x00
... (8 rows total)
```

### Display Enable Logic
The test ROMs demonstrate the correct display enable behavior:
- Register 1, Bit 6 (BL): Display enable/blank control
  - Bit 6 = 1: Display enabled (screen shows rendered graphics)
  - Bit 6 = 0: Display blanked (screen shows solid backdrop color)
- This applies to both Mode 4 (SMS) and TMS modes (same polarity)
- Real SMS hardware powers up with bit 6 = 1 (display enabled)
- Most commercial games explicitly set this bit to ensure display is on
