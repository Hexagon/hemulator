---
title: TIA (Television Interface Adapter)
description: Atari 2600 TIA chip register reference and specifications
parent: "CPU & Hardware References"
---

# TIA (Television Interface Adapter)

The Television Interface Adapter (TIA) is the custom chip in the Atari 2600 that handles all video and audio generation. Unlike modern systems with framebuffers, the TIA generates video signals in real-time, scanline by scanline—a technique known as "racing the beam."

**Related Documentation**:
- [Atari 2600 System README](../../../crates/systems/atari2600/README.md)
- [CPU 6502 Reference](6502.md) (the 6507 is a pin-compatible subset)

## Overview

### Video Specifications

| Specification | NTSC | PAL |
|---------------|------|-----|
| **Visible Resolution** | 160×192 | 160×228 |
| **Total Scanlines** | 262 | 312 |
| **Color Clocks/Scanline** | 228 | 228 |
| **Visible Color Clocks** | 160 | 160 |
| **HBLANK Clocks** | 68 | 68 |
| **Frame Rate** | ~60 Hz | ~50 Hz |
| **Color Clock Frequency** | 3.579545 MHz | 3.546894 MHz |
| **CPU Clock** | ~1.19 MHz | ~1.19 MHz |
| **CPU Cycles/Scanline** | 76 | 76 |

### Graphics Objects

The TIA can render several graphics objects simultaneously:

| Object | Width | Color Source | Count |
|--------|-------|--------------|-------|
| **Playfield** | 40 bits (160 pixels) | COLUPF | 1 |
| **Player 0** | 8 pixels (scalable) | COLUP0 | 1 |
| **Player 1** | 8 pixels (scalable) | COLUP1 | 1 |
| **Missile 0** | 1-8 pixels | COLUP0 | 1 |
| **Missile 1** | 1-8 pixels | COLUP1 | 1 |
| **Ball** | 1-8 pixels | COLUPF | 1 |

### Color Palette

The TIA uses a 128-color palette (NTSC):
- **Upper 4 bits**: Hue (0-15, representing different colors)
- **Lower 3 bits**: Luminance (0-7, controlling brightness)
- **Bit 0**: Unused in color registers

## Register Map

### Write Registers ($00-$2C)

#### Sync and Timing

| Address | Name | Bits | Description |
|---------|------|------|-------------|
| $00 | VSYNC | D1 | Vertical sync (set D1 for 3 scanlines) |
| $01 | VBLANK | D1,D6,D7 | Vertical blank: D1=enable, D6=latch inputs, D7=dump paddles |
| $02 | WSYNC | - | Wait for horizontal sync (halts CPU until scanline end) |
| $03 | RSYNC | - | Reset horizontal sync counter (not typically used) |

#### Player Size and Copies (NUSIZ)

| Address | Name | Bits | Description |
|---------|------|------|-------------|
| $04 | NUSIZ0 | D0-D2, D4-D5 | Player 0 / Missile 0 size and copies |
| $05 | NUSIZ1 | D0-D2, D4-D5 | Player 1 / Missile 1 size and copies |

**NUSIZ Encoding (D0-D2)**:
| Value | Player Mode | Description |
|-------|-------------|-------------|
| 0 | One copy | Normal single sprite |
| 1 | Two copies close | 8-pixel gap |
| 2 | Two copies medium | 24-pixel gap |
| 3 | Three copies close | 8-pixel gaps |
| 4 | Two copies wide | 56-pixel gap |
| 5 | Double-size player | 16 pixels wide |
| 6 | Three copies medium | 24-pixel gaps |
| 7 | Quad-size player | 32 pixels wide |

**Missile Size (D4-D5)**:
| Value | Missile Width |
|-------|---------------|
| 0 | 1 pixel |
| 1 | 2 pixels |
| 2 | 4 pixels |
| 3 | 8 pixels |

#### Color Registers

| Address | Name | Bits | Description |
|---------|------|------|-------------|
| $06 | COLUP0 | D1-D7 | Player 0 and Missile 0 color |
| $07 | COLUP1 | D1-D7 | Player 1 and Missile 1 color |
| $08 | COLUPF | D1-D7 | Playfield and Ball color |
| $09 | COLUBK | D1-D7 | Background color |

#### Playfield Control

| Address | Name | Bits | Description |
|---------|------|------|-------------|
| $0A | CTRLPF | D0-D2, D4-D5 | Playfield control and ball size |
| $0B | REFP0 | D3 | Player 0 reflect (mirror horizontally) |
| $0C | REFP1 | D3 | Player 1 reflect (mirror horizontally) |
| $0D | PF0 | D4-D7 | Playfield register 0 (reversed bit order) |
| $0E | PF1 | D0-D7 | Playfield register 1 (normal order) |
| $0F | PF2 | D0-D7 | Playfield register 2 (reversed bit order) |

**CTRLPF Encoding**:
| Bit | Name | Description |
|-----|------|-------------|
| D0 | REF | Playfield reflect (mirror right half) |
| D1 | SCORE | Score mode (PF uses player colors) |
| D2 | PFP | Playfield priority (PF/Ball in front of players) |
| D4-D5 | BALL | Ball size (same encoding as missile) |

#### Position Reset (Strobe Registers)

| Address | Name | Description |
|---------|------|-------------|
| $10 | RESP0 | Reset Player 0 position to current beam |
| $11 | RESP1 | Reset Player 1 position to current beam |
| $12 | RESM0 | Reset Missile 0 position to current beam |
| $13 | RESM1 | Reset Missile 1 position to current beam |
| $14 | RESBL | Reset Ball position to current beam |

These are **strobe registers**: writing any value triggers the reset. Position is set based on the current horizontal beam position when the write occurs.

#### Audio Registers

| Address | Name | Bits | Description |
|---------|------|------|-------------|
| $15 | AUDC0 | D0-D3 | Audio control 0 (waveform select) |
| $16 | AUDC1 | D0-D3 | Audio control 1 (waveform select) |
| $17 | AUDF0 | D0-D4 | Audio frequency 0 (5-bit divider) |
| $18 | AUDF1 | D0-D4 | Audio frequency 1 (5-bit divider) |
| $19 | AUDV0 | D0-D3 | Audio volume 0 |
| $1A | AUDV1 | D0-D3 | Audio volume 1 |

**Audio Control (AUDC) Waveforms**:
| Value | Waveform Type |
|-------|---------------|
| 0, 11 | Set to 1 (DC) |
| 1 | 4-bit polynomial |
| 2 | Division by 2 |
| 3 | 4-bit AND 5-bit poly |
| 4, 5 | Pure tone |
| 6, 10 | Division by 31 |
| 7, 9 | 5-bit polynomial |
| 8 | 5-bit polynomial (noise) |
| 12, 13 | Pure tone with 4-bit poly |
| 14 | 4-bit polynomial |
| 15 | 4-bit XOR 5-bit poly |

#### Graphics Registers

| Address | Name | Bits | Description |
|---------|------|------|-------------|
| $1B | GRP0 | D0-D7 | Player 0 graphics (8 pixels) |
| $1C | GRP1 | D0-D7 | Player 1 graphics (8 pixels) |
| $1D | ENAM0 | D1 | Enable Missile 0 |
| $1E | ENAM1 | D1 | Enable Missile 1 |
| $1F | ENABL | D1 | Enable Ball |

#### Horizontal Motion

| Address | Name | Bits | Description |
|---------|------|------|-------------|
| $20 | HMP0 | D4-D7 | Player 0 horizontal motion (signed 4-bit) |
| $21 | HMP1 | D4-D7 | Player 1 horizontal motion (signed 4-bit) |
| $22 | HMM0 | D4-D7 | Missile 0 horizontal motion |
| $23 | HMM1 | D4-D7 | Missile 1 horizontal motion |
| $24 | HMBL | D4-D7 | Ball horizontal motion |

**Motion Value Encoding** (signed 4-bit in upper nibble):
- Values +1 to +7: Move LEFT by 1-7 pixels
- Value 0: No movement
- Values -1 to -8: Move RIGHT by 1-8 pixels

#### Vertical Delay

| Address | Name | Bits | Description |
|---------|------|------|-------------|
| $25 | VDELP0 | D0 | Delay Player 0 graphics by 1 scanline |
| $26 | VDELP1 | D0 | Delay Player 1 graphics by 1 scanline |
| $27 | VDELBL | D0 | Delay Ball enable by 1 scanline |

#### Missile-to-Player Reset

| Address | Name | Bits | Description |
|---------|------|------|-------------|
| $28 | RESMP0 | D1 | Reset Missile 0 to Player 0 center |
| $29 | RESMP1 | D1 | Reset Missile 1 to Player 1 center |

When enabled, the missile is locked to the center of its associated player.

#### Motion and Collision Control

| Address | Name | Description |
|---------|------|-------------|
| $2A | HMOVE | Apply horizontal motion to all objects |
| $2B | HMCLR | Clear all horizontal motion registers |
| $2C | CXCLR | Clear all collision latches |

### Read Registers ($30-$3D)

#### Collision Registers

| Address | Name | D7 | D6 |
|---------|------|----|----|
| $30 | CXM0P | M0-P1 | M0-P0 |
| $31 | CXM1P | M1-P0 | M1-P1 |
| $32 | CXP0FB | P0-PF | P0-BL |
| $33 | CXP1FB | P1-PF | P1-BL |
| $34 | CXM0FB | M0-PF | M0-BL |
| $35 | CXM1FB | M1-PF | M1-BL |
| $36 | CXBLPF | BL-PF | (unused) |
| $37 | CXPPMM | P0-P1 | M0-M1 |

Collision bits are **latched**: once set, they remain set until cleared with CXCLR ($2C).

#### Input Registers

| Address | Name | Bits | Description |
|---------|------|------|-------------|
| $38 | INPT0 | D7 | Paddle 0 capacitor (active-low when charged) |
| $39 | INPT1 | D7 | Paddle 1 capacitor |
| $3A | INPT2 | D7 | Paddle 2 capacitor |
| $3B | INPT3 | D7 | Paddle 3 capacitor |
| $3C | INPT4 | D7 | Player 0 fire button (0=pressed, 1=released) |
| $3D | INPT5 | D7 | Player 1 fire button (0=pressed, 1=released) |

## Timing Details

### Horizontal Timing

Each scanline consists of 228 color clocks:
- **Color clocks 0-67**: Horizontal blank (68 clocks)
- **Color clocks 68-227**: Visible pixels (160 clocks)

The CPU executes ~76 cycles per scanline (228 ÷ 3).

### Vertical Timing (NTSC Standard)

A standard NTSC frame consists of 262 scanlines:
- **Scanlines 0-2**: VSYNC (3 scanlines with VSYNC bit set)
- **Scanlines 3-39**: VBLANK (~37 scanlines)
- **Scanlines 40-231**: Visible area (192 scanlines)
- **Scanlines 232-261**: Overscan (30 scanlines)

Note: Games may use non-standard timing. The emulator uses VSYNC transitions for frame detection.

### WSYNC Behavior

Writing to WSYNC ($02) halts the CPU until the end of the current scanline. This is critical for "racing the beam" techniques where code must execute at precise horizontal positions.

### HMOVE Timing

HMOVE ($2A) applies motion values to all objects. On real hardware:
- Takes 6 color clocks to complete
- Creates visible "HMOVE comb" artifacts if triggered outside HBLANK
- Best triggered during HBLANK (first 22 CPU cycles of scanline)

## Priority and Rendering

### Default Priority (Playfield Priority = 0)

From front to back:
1. Player 0 / Missile 0
2. Player 1 / Missile 1  
3. Ball
4. Playfield
5. Background

### Playfield Priority (CTRLPF D2 = 1)

From front to back:
1. Playfield / Ball
2. Player 0 / Missile 0
3. Player 1 / Missile 1
4. Background

### Score Mode (CTRLPF D1 = 1)

- Left half of playfield uses COLUP0 (Player 0 color)
- Right half of playfield uses COLUP1 (Player 1 color)

## Playfield Encoding

The playfield is 40 bits wide (160 visible pixels, 4 pixels per bit):

### Left Half (Bits 0-19)
```
PF0: D4 D5 D6 D7  (4 bits, reversed order)
PF1: D7 D6 D5 D4 D3 D2 D1 D0  (8 bits, MSB first)
PF2: D0 D1 D2 D3 D4 D5 D6 D7  (8 bits, LSB first)
```

### Right Half (Bits 20-39)
- **Repeat mode** (REF=0): Same as left half
- **Reflect mode** (REF=1): Mirror of left half

## References

- [problemkaputt.de 2k6specs](https://problemkaputt.de/2k6specs.htm) - Comprehensive TIA specification
- [Stella Programmer's Guide](https://alienbill.com/2600/101/docs/stella.html) - Classic programming reference
- [AtariAge TIA Hardware Notes](https://www.atariage.com/2600/programming/index.html) - Community documentation
