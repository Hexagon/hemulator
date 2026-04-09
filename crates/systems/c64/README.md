# Commodore 64 Implementation

The Commodore 64 (1982) is one of the best-selling home computers of all time, with an estimated
17 million units sold. Its custom chips — the VIC-II for graphics, SID for audio, and dual CIAs
for I/O — made it a powerhouse for games and demos.

## Hardware Overview

| Component | Details |
|-----------|---------|
| **CPU** | MOS 6510 (6502 + built-in I/O port) @ 0.985 MHz (PAL) / 1.023 MHz (NTSC) |
| **Video** | MOS 6569 VIC-II (PAL) / MOS 6567 (NTSC) — 320×200, 16 colors |
| **Audio** | MOS 6581 SID — 3 voices, ADSR, multimode filter |
| **I/O** | 2× MOS 6526 CIA — Timers, keyboard, joystick, serial |
| **RAM** | 64KB main + 1KB color RAM |
| **ROM** | 8KB KERNAL + 8KB BASIC + 4KB character generator |

## Memory Map

```
$0000-$0001  6510 CPU I/O port (DDR + data, controls ROM/IO banking)
$0002-$9FFF  RAM
$A000-$BFFF  BASIC ROM / RAM (controlled by LORAM bit)
$C000-$CFFF  RAM
$D000-$DFFF  I/O / Character ROM / RAM (controlled by CHAREN/HIRAM/LORAM)
$E000-$FFFF  KERNAL ROM / RAM (controlled by HIRAM bit)
```

### I/O Area ($D000–$DFFF when visible)

```
$D000-$D3FF  VIC-II registers (mirrored every 64 bytes)
$D400-$D7FF  SID registers (mirrored every 32 bytes)
$D800-$DBFF  Color RAM (1024 nybbles)
$DC00-$DCFF  CIA 1 (keyboard matrix, joystick port 2, IRQ)
$DD00-$DDFF  CIA 2 (VIC bank select, serial bus, joystick port 1, NMI)
$DE00-$DFFF  I/O expansion area
```

### PLA Banking

The 6510 CPU's built-in I/O port at $0001 (bits 0–2) controls which ROM/IO regions are visible:

| HIRAM | LORAM | CHAREN | $A000 | $D000 | $E000 |
|-------|-------|--------|-------|-------|-------|
| 1 | 1 | 1 | BASIC ROM | I/O | KERNAL ROM |
| 1 | 1 | 0 | BASIC ROM | Char ROM | KERNAL ROM |
| 1 | 0 | x | RAM | I/O* | KERNAL ROM |
| 0 | x | x | RAM | RAM | RAM |

## Implementation Status

### ✅ Implemented

- **CPU**: MOS 6510 via `emu_core::cpu_6502` with I/O port banking
- **VIC-II (MOS 6569 PAL)**:
  - Standard character mode (40×25)
  - Multicolor character mode
  - Standard bitmap mode (320×200)
  - Multicolor bitmap mode (160×200)
  - Extended color mode (ECM)
  - 8 hardware sprites with X/Y expansion and multicolor
  - Raster interrupt generation
  - Bad line detection and cycle stealing
  - VIC bank selection via CIA2 PA
  - Character ROM substitution in banks 0/2
- **SID (MOS 6581)**:
  - 3 independent voices with 16-bit frequency
  - Waveforms: triangle, sawtooth, pulse (variable duty), noise
  - Full ADSR envelope generator per voice
  - Ring modulation (voice N with voice N-1)
  - Hard synchronization
  - 23-bit Galois LFSR noise generator
  - Master volume control
  - Voice 3 oscillator/envelope readback registers
  - 44.1 kHz stereo audio output
- **CIA 6526 × 2**:
  - 16-bit countdown timers A and B (one-shot / continuous)
  - Timer B counting Timer A underflows
  - 8×8 keyboard matrix scanning
  - Joystick port reading
  - Interrupt control/status register
  - CIA1 → IRQ, CIA2 → NMI (edge-triggered)
  - Time-of-Day clock registers
  - Serial shift register
- **Memory bus**:
  - Full PLA banking via $0001 (LORAM/HIRAM/CHAREN)
  - Write-through to underlying RAM
  - Color RAM (4-bit nybbles at $D800)
- **System features**:
  - PRG file loading (2-byte load address header)
  - Save states (CPU + I/O port state)
  - Instruction tracing and breakpoints
  - Stub KERNAL/BASIC/CHAR ROMs for testing without real ROMs

### 🚧 Not Yet Implemented

- **SID filter**: Registers are stored but multimode filter (LP/BP/HP) is not applied to audio output
- **Sprite collision detection**: Collision registers are readable but not populated during rendering
- **Smooth scrolling**: X/Y scroll registers ($D011/$D016) are not applied to display offset
- **Border rendering**: 38/40 column and 24/25 row border modes not fully implemented
- **Sprite priority**: Sprite-to-background priority bit not enforced
- **Light pen**: Light pen registers return fixed values
- **Datasette**: Cassette port not emulated
- **Disk drive**: 1541 drive emulation not implemented (.d64 files)
- **Cartridge formats**: .crt cartridge bank switching not implemented
- **Real ROM loading**: Requires stub ROMs; real KERNAL/BASIC/CHAR ROMs can be loaded via mount points
- **Debugger trait**: Full `Debugger` implementation for GUI Inspector
- **NTSC mode**: Only PAL timing implemented

## Architecture

The C64 uses the reusable 6502 CPU from `emu_core` and implements system-specific chips:

```
emu_core::cpu_6502::Cpu6502<C64Bus>
     │
     └─→ C64Bus (Memory6502 trait)
           ├─→ Vic (VIC-II, Rc<RefCell<>>)
           ├─→ Sid (SID, Rc<RefCell<>>)
           ├─→ Cia (CIA 1, Rc<RefCell<>>) → IRQ
           ├─→ Cia (CIA 2, Rc<RefCell<>>) → NMI
           ├─→ RAM [64KB]
           ├─→ Color RAM [1KB]
           └─→ ROM (KERNAL/BASIC/CHAR)
```

The SID, VIC-II, and CIA chips are C64-specific (not shared with other systems in this emulator)
and are implemented directly in the system crate.

## ROM Formats

| Extension | Format | Description |
|-----------|--------|-------------|
| `.prg` | PRG | 2-byte load address + data |
| `.crt` | CRT | Cartridge image (bank switching not yet supported) |

## References

- [C64 Wiki — Memory Map](https://www.c64-wiki.com/wiki/Memory_Map)
- [C64 Programmer's Reference Guide (Commodore)](https://www.c64-wiki.com/wiki/Commodore_64_Programmer%27s_Reference_Guide)
- [MOS 6569 VIC-II datasheet](http://www.6502.org/documents/datasheets/mos/mos_6569_vic-ii.pdf)
- [MOS 6581 SID datasheet](http://www.6502.org/documents/datasheets/mos/mos_6581_sid.pdf)
- [MOS 6526 CIA datasheet](http://www.6502.org/documents/datasheets/mos/mos_6526_cia.pdf)
- [The MOS 6567/6569 video controller (VIC-II) and its application in the Commodore 64 (Christian Bauer)](http://www.zimmers.net/cbmpics/cbm/c64/vic-ii.txt)
- [The Commodore 64 PLA (Thomas 'skoe' Giesel)](https://skoe.de/docs/c64-dissected/pla/c64_pla_active_active.html)
