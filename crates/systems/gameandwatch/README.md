# Game & Watch Emulation

Nintendo's Game & Watch handheld series (1980–1991), based on the Sharp SM510 4-bit microcontroller family.

## Architecture

### SM510 CPU

The SM510 is a 4-bit microcontroller running at 32.768 kHz (watch crystal):

- **ACC**: 4-bit accumulator
- **C**: 1-bit carry flag
- **PC**: 12-bit program counter (page:offset, 6-bit pages of 64 bytes)
- **Stack**: Single-level return address stack
- **BL**: 4-bit RAM address low register
- **BM**: 2-bit RAM address high register (extended by SBM flag)
- **ROM**: Up to 4 KB program memory
- **RAM**: 128 × 4-bit nibbles (512 bits total)
- **Divider**: 15-bit frequency divider chain from 32.768 kHz crystal

### LCD Display

- Segment-based LCD driven from display RAM
- Up to 16 segment lines × 4 common lines = 64 segments per group
- 8 RAM groups × 16 columns × 4 bits = 512 individual segments
- **With .mgw ROMs**: 320×240 artwork with per-segment composited overlays
- **Without artwork** (raw ROMs): segments shown as a diagnostic grid

### Input

- **K port**: 4-bit multiplexed input (buttons depend on S output state)
- **BA/B**: Additional single-bit inputs
- **ACL**: All Clear (reset) button
- Input matrix: S output selects which button row is read on K port
- **With .mgw ROMs**: Per-game keyboard mapping extracted from ROM container

### Audio

- Piezo buzzer controlled by melody generator
- Simple square-wave output when melody is enabled

## Supported ROM Formats

### .mgw (preferred)

Compressed container format from LCD-Game-Shrinker / gw-libretro. Contains:
- CPU program ROM (1–4 KB)
- Background image (320×240 RGB565)
- LCD segment artwork (grayscale pixel masks, up to 256 segments)
- Keyboard input mapping (per-game S/K matrix wiring)
- Melody data

Supports LZ4, ZLIB, and uncompressed containers. Typical file sizes: 50 KB – 2.3 MB.

### Raw ROM (.gw, .gnw, .bin)

Raw SM510 program ROM binary files. Typical sizes: 1–4 KB.
Without artwork, segments are shown as a diagnostic grid. Input uses a hardcoded
button-to-matrix mapping.

## Controller Mapping

| Button | Keyboard | Function |
|--------|----------|----------|
| Left   | ←        | Direction |
| Right  | →        | Direction |
| Up     | ↑        | Direction |
| Down   | ↓        | Direction |
| A      | Z        | Game A / Primary action |
| B      | X        | Game B / Secondary action |
| Time   | T        | Time display |
| Game   | G        | Game select / Alarm |

## Implementation Status

- [x] SM510 CPU core (all 49 instruction types)
- [x] RAM / ROM memory system
- [x] .mgw container parsing (LZ4/ZLIB decompression)
- [x] LCD artwork rendering with segment compositing
- [x] Per-game keyboard mapping from .mgw ROMs
- [x] Fallback LCD segment grid visualization (raw ROMs)
- [x] Input matrix with multiplexed K port
- [x] Frequency divider chain
- [x] Single-level subroutine stack
- [x] Basic melody/buzzer state
- [x] Debugger interface
- [ ] SM511/SM5A CPU variants (planned)
- [ ] Accurate melody ROM playback (planned)
- [ ] JPEG background support in .mgw (planned)
- [ ] LCD deflicker filtering (planned)

## References

- Sharp SM510 Technical Manual
- MAME SM510 emulation: `src/devices/cpu/sm510/`
- [Game & Watch technical info](https://www.seanriddle.com/gnw.html)
- [SM510 instruction set reference](https://tama.dev/sm510/)
- [LCD-Game-Shrinker](https://github.com/bzhxx/LCD-Game-Shrinker) — .mgw ROM builder
- [LCD-Game-Emulator (gw-libretro)](https://github.com/bzhxx/LCD-Game-Emulator) — Reference implementation