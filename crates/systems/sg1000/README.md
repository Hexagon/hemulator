# SG-1000 Emulator

This crate implements an SG-1000 emulator for the Hemulator multi-system emulator.

## Hardware

The SG-1000 (1983) is Sega's first home video game console.

### CPU
- **Zilog Z80A** @ 3.579545 MHz (NTSC)
- 8-bit microprocessor
- Reuses the Z80 CPU implementation from `emu_core`

### Graphics - TMS9918A VDP
- **Texas Instruments TMS9918A** Video Display Processor
- Same chip as ColecoVision
- 256×192 pixel resolution
- 16-color palette
- 16 KB VRAM
- 4 graphics modes (Graphics I/II, Text, Multicolor)
- 32 hardware sprites with collision detection

### Audio - SN76489 PSG
- **Texas Instruments SN76489** Programmable Sound Generator
- Same chip as ColecoVision and SMS
- 3 square wave tone generators
- 1 noise generator
- Reuses the SN76489 implementation from `emu_core`

### Memory Map

```
0x0000-0xBFFF  Cartridge ROM (up to 48KB)
0xC000-0xC3FF  RAM (1 KB)
0xC400-0xFFFF  RAM Mirror (repeats 0xC000-0xC3FF)
```

### I/O Ports

```
0x7F       SN76489 PSG (write only)
0xBE       VDP Data (read/write)
0xBF       VDP Control/Status (read/write)
0xDC-0xDF  Controller ports
```

## Implementation Details

### Hardware Reuse

The SG-1000 shares all major components with the ColecoVision:
- **Z80 CPU**: Identical processor and clock speed
- **TMS9918A VDP**: Same graphics chip
- **SN76489 PSG**: Same sound chip

The key differences are:
- **No BIOS ROM**: SG-1000 boots directly from cartridge
- **Different memory map**: Cartridge at 0x0000 instead of 0x8000
- **Different I/O ports**: PSG at 0x7F instead of 0xA0

### VDP and PSG

The VDP and PSG modules are copied from the ColecoVision implementation, as the hardware is identical. This maximizes code reuse while maintaining separate system implementations.

### Save States

Full save state support includes:
- CPU state (all Z80 registers)
- VDP state (VRAM, registers, internal state)  
- PSG state (all channel states)
- Memory (RAM contents)

## System Requirements

The SG-1000 only requires a cartridge ROM to run - no BIOS is needed.

## Known Limitations

- No SC-3000 support (keyboard computer variant)
- No cassette tape support
- Controllers limited to standard joystick

## Recent Fixes

### Audio Pipeline Integration (February 2026)
- **PSG Audio Now Functional**: Complete audio pipeline integration with frontend
  - SN76489 PSG now generates and outputs audio samples to the frontend
  - Cycle-accurate audio generation matching SMS implementation
  - PSG properly resets on system reset
  - Audio quality: 44.1 kHz stereo output with exponential volume curve
- **Impact**: Games now have sound! All 3 tone channels and 1 noise channel working correctly

### TMS9918A Display Blank Bit (January 2026)
- **Fixed display blanking**: When bit 6 of register 1 is set (display blanked), the entire display now shows only the backdrop color (register 7, lower 4 bits)
- **Previous behavior**: Only sprites were disabled when blanked, but background tiles were still rendered
- **Correct behavior**: Both tiles and sprites are disabled when blanked, matching TMS9918A hardware behavior
- **Impact**: Fixes games like Bomb Jack that show only a blue screen instead of graphics (game was blanking display during initialization)

### TMS9918A Sprite Rendering (January 2026)
- **Fixed sprite transparency**: Sprites with color code 0 are now properly treated as transparent and not drawn, matching TMS9918A hardware behavior
- **Impact**: Fixes missing sprites in games like 007 where sprites were incorrectly rendered as invisible black rectangles

## References

### Technical Documentation
- [TMS9918A Datasheet](http://www.vdp-tester.com/TMS9918A_and_TMS9928A_Data_Manual.pdf)
- [SG-1000 Tech Specs](http://www.smspower.org/Development/SG1000)
- [Z80 CPU User Manual](http://www.zilog.com/docs/z80/um0080.pdf)
- [SN76489 Datasheet](http://www.smspower.org/maxim/Documents/SN76489)

### Development Resources
- [SG-1000 Programming Guide](http://www.smspower.org/Development/SG1000)
- [TMS9918A Programming](http://map.grauw.nl/resources/video/texasinstruments_tms9918.pdf)
- [SG-1000 Memory Map](http://www.smspower.org/Development/MemoryMap-SG)

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
