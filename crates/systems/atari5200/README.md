# Atari 5200 Implementation

The Atari 5200 SuperSystem (1982) is a home video game console based on the Atari 400/800
computer architecture.

## Hardware Overview

### CPU — MOS 6502C

A CMOS variant of the 6502 running at 1.79 MHz (NTSC).  Uses the reusable `cpu_6502` from
`emu_core`.

### ANTIC — Alphanumeric Television Interface Controller

A DMA-driven display list processor that generates the playfield display.

- Reads a "display list" program from memory
- Multiple character and bitmap modes
- Supports horizontal/vertical scrolling
- Generates DLI/VBI interrupts
- Resolution: up to 320×192 (hi-res) or 160×192 (standard)

### GTIA — George's Television Interface Adapter

Handles color generation, player-missile graphics, and collision detection.

- 9 color registers (4 player, 4 playfield, 1 background)
- 4 player sprites (8 pixels wide, full-screen tall)
- 4 missiles (2 pixels wide)
- 128-color NTSC palette (hue × luminance)
- Hardware collision detection

### POKEY — POtentiometer and KEYboard IC

Handles sound, input, timers, and serial I/O.

- 4-channel audio (square waves with distortion)
- Paddle/joystick analog input
- Random number generation
- Programmable timers

## Memory Map

```
$0000-$3FFF: 16KB RAM
$4000-$7FFF: Unused (open bus)
$8000-$BFFF: Cartridge ROM (8KB, 16KB, or 32KB banked)
$C000-$C0FF: GTIA registers
$D400-$D4FF: ANTIC registers
$E800-$E8FF: POKEY registers
$F800-$FFFF: Built-in BIOS ROM (2KB)
```

## Cartridge Support

| Size | Banking | Description |
|------|---------|-------------|
| 8KB  | None    | ROM at $8000-$9FFF, mirrored |
| 16KB | None    | ROM at $8000-$BFFF |
| 32KB | F8-type | Two 16KB banks, switched via $BFE0-$BFEF |

## Implementation Status

**Implemented:**
- 6502C CPU via reusable `cpu_6502` core
- ANTIC display list processor (multiple graphics modes)
- GTIA player-missile graphics, color registers, and collision detection
- POKEY audio (4 channels) and input handling
- 8KB / 16KB / 32KB cartridge loading with F8-type banking

**Not Yet Implemented:**
- Video output not yet fully functional
- Full game compatibility pending
- Paddle/analog controller support
- BIOS ROM loading

## References

- [Atari 5200 Technical Reference Manual](https://archive.org/details/Atari_5200_Technical_Reference_Notes)
- [ANTIC and GTIA technical documentation (AtariAge)](https://www.atarimax.com/jindroush.atari.org/atanttim.html)
- [POKEY datasheet (Atari)](https://archive.org/details/pokey-doc)
