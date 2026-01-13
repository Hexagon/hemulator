# ColecoVision Emulator

This crate implements a ColecoVision emulator for the Hemulator multi-system emulator.

## Hardware

The ColecoVision (1982) is a second-generation home video game console.

### CPU
- **Zilog Z80A** @ 3.579545 MHz (NTSC)
- 8-bit microprocessor
- Reuses the Z80 CPU implementation from `emu_core`

### Graphics - TMS9918A VDP
- **Texas Instruments TMS9918A** Video Display Processor
- 256×192 pixel resolution
- 16-color palette
- 16 KB VRAM
- 4 graphics modes:
  - **Graphics I**: 256×192, 8×8 tiles, 2 colors per 8 patterns
  - **Graphics II**: 256×192, 8×8 tiles, 2 colors per pattern row (most common for games)
  - **Text**: 40×24 characters, 6×8 font
  - **Multicolor**: 64×48 blocks, 4×4 pixels per block
- 32 hardware sprites:
  - 8×8 or 16×16 pixels
  - 1× or 2× magnification
  - 4 sprites per scanline limit
  - Sprite collision detection
  - Early clock shift (-32 pixels)

### Audio - SN76489 PSG
- **Texas Instruments SN76489** Programmable Sound Generator
- 3 square wave tone generators (10-bit frequency)
- 1 noise generator (white/periodic noise)
- 4-bit volume control per channel
- Reuses the SN76489 implementation from `emu_core`

### Memory Map

```
0x0000-0x1FFF  BIOS ROM (8 KB)
0x2000-0x5FFF  Expansion/Unused
0x6000-0x63FF  RAM (1 KB, mirrored to 0x73FF)
0x7400-0x7FFF  Expansion/Unused
0x8000-0xFFFF  Cartridge ROM (up to 32 KB)
```

### I/O Ports

```
0xA0-0xA1  SN76489 PSG (write only)
0xBE       VDP Data (read/write)
0xBF       VDP Control/Status (read/write)
0xE0-0xFF  Controller ports
```

## Implementation Details

### VDP Rendering

The VDP implements all 4 graphics modes with full sprite support:

- **Graphics I/II**: Most commonly used for games, renders tiled backgrounds with per-tile or per-row color control
- **Text Mode**: 40-column text display with 6-pixel wide characters
- **Multicolor Mode**: Low-resolution color block mode for simple graphics
- **Sprites**: Rendered on top of background with collision detection

The VDP generates frame interrupts at 60 Hz (NTSC) to drive game logic.

### Audio Generation

The PSG runs at the CPU clock frequency (3.579545 MHz) and generates audio at 44.1 kHz sample rate using cycle-accurate timing and downsampling.

### Save States

Full save state support includes:
- CPU state (all Z80 registers)
- VDP state (VRAM, registers, internal state)
- PSG state (all channel states)
- Memory (RAM contents)

## System Requirements

The ColecoVision requires a BIOS ROM to boot, which must be provided separately via the mount points system:

- **BIOS**: 8 KB system ROM (required)
- **Cartridge**: Game ROM up to 32 KB (required)

## Known Limitations

- No expansion module support (e.g., Super Game Module)
- No tape/disk drive support
- Controllers limited to standard joystick (no Super Action Controllers, spinners, etc.)
- Audio output currently stubbed (PSG implemented but not connected to audio pipeline)

## Recent Improvements

### VDP Edge Cases (January 2026)
- **Sprite Collision Detection**: Now uses dedicated sprite buffer instead of color comparison, correctly detecting sprite-to-sprite overlap
- **Sprite Overflow Handling**: Flags now properly persist across scanlines within a frame, matching TMS9918A hardware behavior
- **Sprite Y Position**: Correctly implements Y offset (-1) for proper sprite positioning
- **Array Bounds Checking**: Added validation for sprite pattern and attribute table accesses to prevent buffer overruns
- **PSG Reset**: PSG state now properly resets on system reset
- **Sprite Rendering**: Corrected sprite enable logic - sprites render whenever display is enabled (no separate enable bit)

### Testing
Unit tests now cover:
- System creation and reset
- VDP sprite collision detection
- VDP sprite overflow detection (5th sprite flag)
- VDP address register wrapping
- Save state roundtrip

## References

### Technical Documentation
- [TMS9918A Datasheet](http://www.vdp-tester.com/TMS9918A_and_TMS9928A_Data_Manual.pdf)
- [ColecoVision Tech Specs](http://www.atarihq.com/danb/files/CV-Tech.txt)
- [Z80 CPU User Manual](http://www.zilog.com/docs/z80/um0080.pdf)
- [SN76489 Datasheet](http://www.smspower.org/maxim/Documents/SN76489)

### Development Resources
- [ColecoVision Programming Guide](http://www.atarihq.com/danb/files/CV-Programming.txt)
- [TMS9918A Programming](http://map.grauw.nl/resources/video/texasinstruments_tms9918.pdf)
- [ColecoVision Memory Map](http://www.atarihq.com/danb/files/CV-MemMap.txt)

## Testing

Currently no test ROM or smoke tests are implemented. Test ROMs can be created using:
- z80asm assembler
- SDCC (Small Device C Compiler) for Z80

Example test ROM structure would verify:
- VDP initialization and mode switching
- Pattern/color table loading
- Sprite rendering
- Controller input
- Audio output
