# Sega Mega Drive / Genesis Implementation

The Sega Mega Drive (1988, sold as "Genesis" in North America) is a 16-bit home video game
console and one of the defining platforms of its era.

## Hardware Overview

| Component | Details |
|-----------|---------|
| **Main CPU** | Motorola 68000 @ 7.67 MHz (NTSC) / 7.60 MHz (PAL) |
| **Sound CPU** | Zilog Z80 @ 3.58 MHz (NTSC) / 3.55 MHz (PAL) |
| **VDP** | Yamaha YM7101 (315-5313) — 64KB VRAM, 128B CRAM, 80B VSRAM |
| **FM Synth** | Yamaha YM2612 (6 FM channels) |
| **PSG** | Texas Instruments SN76489 (4 channels, integrated in VDP) |
| **Main RAM** | 64KB (68K) |
| **Sound RAM** | 8KB (Z80) |

## Memory Map (68000)

```
$000000-$3FFFFF: Cartridge ROM (up to 4MB)
$A00000-$A0FFFF: Z80 address space (banked)
$A10000-$A1001F: I/O ports (controllers, etc.)
$A11100-$A11101: Z80 bus request
$A11200-$A11201: Z80 reset
$C00000-$C0001F: VDP ports
$E00000-$FFFFFF: 64KB main RAM (mirrored)
```

## Implementation Status

**Implemented:**
- Motorola 68000 CPU execution
- Basic VDP with tilemap and sprite rendering
- YM2612 FM synthesizer (6 channels)
- SN76489 PSG (4-channel audio)
- ROM loading and cartridge detection

**Not Yet Implemented:**
- Full game compatibility
- Z80 sub-CPU integration and banked bus access
- Complete audio mixing (FM + PSG)
- Save RAM (SRAM/EEPROM) support

## References

- [Sega Mega Drive / Genesis Technical Reference (Charles MacDonald)](https://segaretro.org/Mega_Drive_Technical_Reference_Manual)
- [68000 Programmer's Reference Manual (Motorola M68000PM/AD)](https://www.nxp.com/docs/en/reference-manual/M68000PM.pdf)
- [YM2612 Application Manual (Yamaha)](https://www.smspower.org/maxim/Documents/YM2612)
- [Genesis/Mega Drive VDP documentation (Charles MacDonald)](http://md.railgun.works/index.php?title=VDP)
