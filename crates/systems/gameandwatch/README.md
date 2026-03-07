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
- Without artwork overlays, segments are shown as a raw grid

### Input

- **K port**: 4-bit multiplexed input (buttons depend on S output state)
- **BA/B**: Additional single-bit inputs
- **ACL**: All Clear (reset) button
- Input matrix: S output selects which button row is read on K port

### Audio

- Piezo buzzer controlled by melody generator
- Simple square-wave output when melody is enabled

## Supported ROM Formats

- Raw SM510 program ROM binary (`.gw`, `.gnw` extensions)
- Typical ROM sizes: 1–4 KB

## Controller Mapping

| Button | Keyboard | Function |
|--------|----------|----------|
| Left   | ←        | Direction |
| Right  | →        | Direction |
| Up     | ↑        | Direction |
| Down   | ↓        | Direction |
| Game A | Z        | Start Game A |
| Game B | X        | Start Game B |
| Time   | T        | Time display |
| Alarm  | Enter    | Set alarm |
| ACL    | R        | All Clear (reset game) |

## Implementation Status

- [x] SM510 CPU core (all 49 instruction types)
- [x] RAM / ROM memory system
- [x] LCD segment grid visualization
- [x] Input matrix with multiplexed K port
- [x] Frequency divider chain
- [x] Single-level subroutine stack
- [x] Basic melody/buzzer state
- [x] Debugger interface
- [ ] SVG/PNG artwork overlay support (planned)
- [ ] SM511/SM5A CPU variants (planned)
- [ ] Accurate melody ROM playback (planned)

## References

- Sharp SM510 Technical Manual
- MAME SM510 emulation: `src/devices/cpu/sm510/`
- [Game & Watch technical info](https://www.seanriddle.com/gnw.html)
- [SM510 instruction set reference](https://tama.dev/sm510/)
