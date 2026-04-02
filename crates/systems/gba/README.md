# Game Boy Advance (GBA)

**Status**: Functional (most games working; CPU + PPU + DMA + Timers + Debugger)

## Overview

The GBA system crate emulates the Game Boy Advance handheld console with an ARM7TDMI CPU,
a full PPU rendering pipeline, DMA controller, hardware timers, cartridge identification, and debug introspection.

## Implemented

- **ARM7TDMI CPU**: Complete ARM and Thumb instruction sets, 7 processor modes, banked registers, hardware interrupts
- **PPU**: Scanline-based 240×160 rendering
  - Background modes 0–5 (text, affine, bitmap)
  - 128 sprites with normal and affine transforms
  - Layer compositing with priority, windowing, color effects
  - Alpha blending, brightness increase/decrease
  - Mosaic effect for backgrounds
- **Cartridge Identification**: Full GBA ROM header parsing
  - Game title, game code, maker code (30+ known publishers)
  - Region detection from game code
  - Nintendo logo validation and header checksum verification
  - Save type auto-detection via SDK library string scanning (EEPROM, SRAM, Flash 64K/128K)
- **Debugger**: Implements the `Debugger` trait for `--debug-dump-*` support
  - ARM and Thumb disassembly via `disasm_arm7tdmi`
  - Full CPU state: R0–R15, CPSR, all condition flags (N/Z/C/V/I/F/T), processor mode
  - 9 memory regions: BIOS, EWRAM, IWRAM, I/O, Palette, VRAM, OAM, ROM (dynamic size), SRAM
- **Hardware Timers**: 4 × 16-bit timers with full functionality
  - Prescaler dividers: F/1, F/64, F/256, F/1024
  - Cascade mode (timer N counts overflows from timer N-1)
  - Timer overflow IRQ generation
  - Counter reload on overflow
  - Proper start/stop behavior with prescaler reset
- **DMA Controller**: 4 DMA channels with full functionality
  - Immediate, VBlank, and HBlank start timing
  - 16-bit and 32-bit transfer modes
  - Source/destination address control (increment, decrement, fixed, increment/reload)
  - Repeat mode for auto-triggered transfers
  - DMA completion IRQ generation
  - Priority-ordered execution (DMA0 > DMA1 > DMA2 > DMA3)
  - Sound FIFO (DMA1/2 special mode) and video capture (DMA3 special mode) timing support

## Known Limitations

- APU audio not implemented
- No save state support
- Serial/link cable not implemented

## References

- [GBATEK](https://problemkaputt.de/gbatek.htm) - Comprehensive GBA hardware documentation
- [Tonc](https://www.coranac.com/tonc/text/) - GBA programming tutorial and reference
- [ARM7TDMI Technical Reference Manual](https://developer.arm.com/documentation/ddi0210/c/) - CPU architecture
