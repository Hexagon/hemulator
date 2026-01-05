# CHIP-8 System Implementation

This module provides a complete emulation of the CHIP-8 interpreted programming language.

## Overview

CHIP-8 is an interpreted programming language designed for 8-bit microcomputers in the mid-1970s by Joseph Weisbecker. It was initially used on the COSMAC VIP and Telmac 1800 computers in the mid-1970s. CHIP-8 programs are run on a CHIP-8 virtual machine, making them highly portable.

## Architecture

### CPU/Interpreter

- 16 8-bit general-purpose registers (V0-VF, where VF is used as a flag register)
- 16-bit index register (I)
- 16-bit program counter (PC)
- 8-bit stack pointer (SP)
- 16 levels of stack for subroutine calls
- Two timers (delay timer and sound timer) that count down at 60Hz

### Memory

- **Total**: 4KB (4096 bytes) of RAM
- **0x000-0x1FF**: Reserved for interpreter (includes built-in font data)
- **0x200-0xFFF**: Program ROM and work RAM
- Programs start at address 0x200

### Display

- **Resolution**: 64x32 pixels, monochrome
- **Sprites**: 8 pixels wide, 1-15 pixels tall
- **Drawing mode**: XOR (toggle pixels)
- **Collision detection**: VF register set to 1 if any pixels are turned off during draw
- **Built-in font**: Hexadecimal digits 0-F (5 bytes each, stored at 0x000-0x04F)

### Input

- **Keypad**: 16-key hexadecimal (0x0-0xF)
- **Original layout**:
  ```
  1 2 3 C
  4 5 6 D
  7 8 9 E
  A 0 B F
  ```
- Commonly mapped to QWERTY keyboard in modern emulators

### Audio

- Single tone beep
- Sounds when sound timer is non-zero
- Simple monotone output

### Instruction Set

- 35 opcodes, all 2 bytes long
- Big-endian format
- Executed at variable speed (traditionally ~700 instructions per second)
- Full instruction set implemented including:
  - Display operations (clear, draw sprite)
  - Flow control (jump, call, return, skip)
  - Arithmetic and logic operations
  - Memory operations
  - Timer operations
  - Input handling

## Implementation Details

### Timing

- **Execution speed**: ~10 instructions per frame at 60 FPS (~600 IPS)
- **Timer frequency**: 60 Hz
- Frame-based execution model

### Rendering

- Software renderer using XOR pixel mode
- Immediate display updates
- 64x32 framebuffer converted to RGBA for display

### Save States

Full save state support implemented:
- CPU registers and state
- All RAM (4KB)
- Stack state
- Timer values
- Base64 encoding for memory in JSON

### Input Mapping

- Controller state uses 16-bit value (one bit per key)
- Supports `set_controller` method for standard integration
- Individual key control via `set_key` method

## Testing

The implementation includes comprehensive tests:

- Unit tests for individual instructions
- Memory and register tests
- Save/load state tests
- Smoke test with bundled test ROM

Test ROM verifies:
- Program loading and execution
- Display rendering
- Built-in font sprites
- Basic opcodes (CLS, LD, DRAW, JP)

## Usage

```rust
use emu_chip8::{Chip8System, Chip8Mode};
use emu_core::System;

// Original CHIP-8
let mut system = Chip8System::new();
let rom_data = std::fs::read("program.ch8")?;
system.mount("Program", &rom_data)?;
let frame = system.step_frame()?;
// frame.pixels contains 64x32 RGBA pixels

// Super-CHIP with high resolution
let mut system = Chip8System::new_with_mode(Chip8Mode::SuperChip);
system.mount("Program", &rom_data)?;
let frame = system.step_frame()?;
// frame.pixels contains 128x64 RGBA pixels

// XO-CHIP with 4-color support
let mut system = Chip8System::new_with_mode(Chip8Mode::XoChip);
system.mount("Program", &rom_data)?;
let frame = system.step_frame()?;
// frame.pixels contains 128x64 RGBA pixels with 4 colors
```

## Implemented Extensions

### Super-CHIP (SCHIP)
- ✅ High-resolution 128x64 display mode (00FE/00FF opcodes)
- ✅ Scrolling in all 4 directions (00CN, 00DN, 00FB, 00FC)
- ✅ Large 16x16 font sprites for digits 0-9
- ✅ 16x16 sprite drawing in high-res mode (DXY0)
- ✅ RPL flag register save/load (FX75/FX85)
- ✅ Large font pointer (FX30)

### XO-CHIP
- ✅ 4-color display using dual bit planes
- ✅ Extended 64KB memory addressing
- ✅ Plane selection (FN01)
- ✅ Audio pattern buffer (F002)
- ✅ Pitch control (FX3A)
- ✅ Extended I register loading (F000 NNNN)
- ✅ Range save/load opcodes (5XY2/5XY3)
- ✅ Scroll up opcode (00DN)

## Known Limitations

1. **Random Number Generator**: Uses simple LCG instead of true random
2. **Audio**: Audio pattern buffer and pitch are tracked but no actual sound synthesis in this module
3. **Timing**: Fixed instruction count per frame rather than cycle-accurate timing
4. **Original CHIP-8 Compatibility**: All 35 original opcodes fully implemented
5. **Super-CHIP Compatibility**: All Super-CHIP extensions fully implemented
6. **XO-CHIP Compatibility**: All XO-CHIP extensions fully implemented

## References

This implementation is based on the following specifications and resources:

1. **Cowgod's CHIP-8 Technical Reference**  
   http://devernay.free.fr/hacks/chip8/C8TECH10.HTM  
   The definitive technical reference for CHIP-8, documenting all 35 opcodes and system architecture.

2. **Columbia University CHIP-8 Design Specification**  
   https://www.cs.columbia.edu/~sedwards/classes/2016/4840-spring/designs/Chip8.pdf  
   Academic documentation of CHIP-8 architecture and design.

3. **Guide to making a CHIP-8 emulator by Tobias V. Langhoff**  
   https://tobiasvl.github.io/blog/write-a-chip-8-emulator/  
   Comprehensive guide to CHIP-8 emulator development with implementation details.

4. **CHIP-8 Wikipedia Article**  
   https://en.wikipedia.org/wiki/CHIP-8  
   Historical context and overview of the CHIP-8 system and its variants.

5. **Chip-8 on the COSMAC VIP**  
   RCA COSMAC VIP Instruction Manual (1978)  
   Original documentation for the first CHIP-8 implementation.

## Future Enhancements

Potential improvements for future versions:

- **Super-CHIP**: Extended 128x64 display mode
- **XO-CHIP**: Color support and additional opcodes
- **Sound synthesis**: Actual beep tone generation
- **Configurable timing**: Adjustable instructions per second
- **Keyboard remapping**: Customizable key mappings per program

## See Also

- [Test ROM README](../../../test_roms/chip8/README.md): Information about the bundled test program
- [ARCHITECTURE.md](../../../docs/ARCHITECTURE.md): Overall emulator architecture
- [MANUAL.md](../../../docs/MANUAL.md): End-user manual including CHIP-8 controls
