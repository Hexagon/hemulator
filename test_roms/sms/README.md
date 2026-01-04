# SMS Test ROM

This directory contains a minimal test ROM for the Sega Master System.

## Purpose

This test ROM is designed to:
1. Verify basic Z80 CPU functionality
2. Test VDP initialization and register writes
3. Demonstrate tile pattern loading
4. Display a visible checkerboard pattern
5. Provide deterministic output for smoke tests

## Building

```bash
./build.sh
```

Requirements:
- Python 3 (for ROM generation script)

## ROM Structure

- **Format**: SMS ROM with TMR SEGA header
- **Size**: 32 KB
- **Header**: TMR SEGA signature at 0x7FF0

## Test Program

The test ROM performs the following operations:

1. **Initialization**:
   - Disables interrupts (DI)
   - Sets stack pointer to 0xDFF0
   
2. **VDP Setup**:
   - Configures VDP registers (mode, name table, sprite table)
   - Sets display mode 4 with display enabled
   
3. **Tile Data**:
   - Loads tile 0: All white pixels (32 bytes of 0xFF)
   - Loads tile 1: All black pixels (32 bytes of 0x00)
   
4. **Name Table**:
   - Fills 32x24 tile name table at 0x3800
   - Creates checkerboard pattern by alternating tiles 0 and 1
   
5. **Palette**:
   - Sets palette entry 0 to white (0x3F - max brightness)
   - Sets palette entry 1 to black (0x00)
   
6. **Main Loop**:
   - Infinite loop with HALT instruction

## Expected Output

The test ROM should display a checkerboard pattern on screen:
- 256x192 pixel resolution
- Alternating white and black 8x8 tiles
- Approximately 50% white pixels, 50% black pixels

## Smoke Test

The smoke test in `crates/systems/sms/src/system.rs` verifies:
- Frame dimensions (256x192)
- Visible output (non-zero pixel count > 0)
- Basic rendering (white pixel percentage > 1%)

More strict verification (exact checkerboard pattern) requires complete VDP implementation.
