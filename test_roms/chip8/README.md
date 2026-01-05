# CHIP-8 Test ROM

This directory contains a minimal CHIP-8 test program used for smoke testing.

## Files

- `test.asm` - Assembly source code (hex opcodes with comments)
- `assemble.py` - Simple Python assembler
- `build.sh` - Build script
- `test.ch8` - Built ROM (binary output)

## Building

```bash
./build.sh
```

Or manually:
```bash
python3 assemble.py test.asm test.ch8
```

## Test Program

The test program performs the following actions:

1. Clears the screen (00E0)
2. Draws the digit '0' at position (10, 10) using the built-in font
3. Draws the digit '8' at position (20, 10) using the built-in font
4. Enters an infinite loop

## Expected Output

The program should display:
- Two digits on a black background
- '0' character on the left
- '8' character on the right
- Both characters should be white (pixels on)

This verifies that:
- The CHIP-8 interpreter can load and execute programs
- The display system works correctly
- Sprite drawing with XOR mode functions properly
- The built-in font is loaded correctly in memory
- Basic opcodes (clear screen, set register, set I, draw sprite, jump) work

## ROM Specifications

- Size: 20 bytes
- Start address: 0x200 (standard CHIP-8 program start)
- No external dependencies
- Uses only built-in font sprites
