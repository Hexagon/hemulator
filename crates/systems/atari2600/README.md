# Atari 2600 Emulation

This crate implements Atari 2600 emulation for the Hemulator project.

**Documentation**:
- [ARCHITECTURE.md](../../../ARCHITECTURE.md) - Overall emulation architecture
- [TIA Hardware Reference](../../../docs/src/references/tia.md) - TIA register specification

## Current Status

The Atari 2600 emulator is **fully working** with support for most common cartridge formats.

### What Works

- ✅ **CPU (6507)** - Uses `cpu_6502` from `emu_core` with 13-bit address bus
- ✅ **TIA** - Television Interface Adapter for video and audio
- ✅ **RIOT** - 6532 chip with RAM, I/O, and timer
- ✅ **Cartridge Banking** - 2K to 32K ROMs with multiple banking schemes
- ✅ **Controllers** - Joystick input support
- ✅ **Save States** - Complete state serialization

### Supported Cartridge Formats

- **2K ROM** - No banking (Combat)
- **4K ROM** - No banking (Pac-Man)
- **8K (F8)** - 2 banks (Asteroids, Missile Command)
- **12K (FA)** - 3 banks (CBS games)
- **16K (F6)** - 4 banks (Donkey Kong)
- **32K (F4)** - 8 banks (larger games)

## Architecture

### Component Structure

```
Atari2600System
  └── Atari2600Cpu (wraps Cpu6502<Atari2600Bus>)
      └── Atari2600Bus (implements Memory6502)
          ├── TIA (Television Interface Adapter)
          │   ├── Playfield (40-bit bitmap)
          │   ├── 2 Players (8-pixel sprites)
          │   ├── 2 Missiles
          │   ├── 1 Ball
          │   └── 2 Audio channels
          ├── RIOT (6532 chip)
          │   ├── 128 bytes RAM
          │   ├── I/O ports (joystick, console switches)
          │   └── Programmable timer
          └── Cartridge (ROM + banking logic)
```

### TIA Implementation

**Location**: `src/tia.rs`

The TIA handles both video and audio:

- **Video**:
  - 160x192 visible pixels (NTSC)
  - 128-color NTSC palette
  - Playfield: 40-bit bitmap (20 bits × 2 halves)
  - 2 Players: 8-pixel sprites with reflection
  - 2 Missiles: 1-pixel wide
  - 1 Ball: 1-pixel wide
  - Priority ordering configurable
  
- **Audio**:
  - 2 audio channels
  - Polynomial waveform generation (uses `PolynomialCounter` from `emu_core::apu`)
  - 16 waveform types per channel
  - Frequency and volume control

**Timing Model**: Frame-based rendering with scanline state latching

### RIOT Implementation

**Location**: `src/riot.rs`

The 6532 RIOT provides:

- **128 bytes RAM** with proper mirroring
- **Programmable timer** with 4 interval modes (1, 8, 64, 1024 clocks)
- **I/O ports**: 
  - SWCHA: Joystick input
  - SWCHB: Console switches (reset, select, difficulty)
- **Timer interrupt flag** (auto-clears on read)

### Cartridge Banking

**Location**: `src/cartridge.rs`

Supports multiple banking schemes:

- **F8 (8K)**: 2 banks, switch at $1FF8-$1FF9
- **FA (12K)**: 3 banks, switch at $1FF8-$1FFA
- **F6 (16K)**: 4 banks, switch at $1FF6-$1FF9
- **F4 (32K)**: 8 banks, switch at $1FF4-$1FFB

Auto-detection based on ROM size.

## Building

```bash
# Build Atari 2600 crate
cargo build --package emu_atari2600

# Run tests
cargo test --package emu_atari2600

# Run with specific ROM
cargo run --release -p emu_gui -- path/to/game.bin
```

## Testing

The Atari 2600 crate includes comprehensive tests:

- **122 total tests** (all passing):
  - TIA tests (rendering, registers, playfield, ball sizing, delayed graphics, RESMP, **timing validation**)
  - RIOT tests (RAM, timer, I/O)
  - Cartridge tests (banking schemes)
  - System integration tests
  - Bus address mapping tests (including dual-write behavior at 0x40-0x7F)
  - **Timing tests**: Validates accurate scanline/frame timing against hardware specifications

- **Test ROMs**: Multiple test ROMs in `test_roms/atari2600/`:
  - `test.bin`: Basic playfield pattern
  - `checkerboard.bin`: Alternating playfield validation
  - `test_timer.bin`: RIOT timer and color cycling
  - `game_test.bin`: Game-like test ROM with sprite movement and color bars

## Usage Example

```rust
use emu_atari2600::Atari2600System;
use emu_core::System;

// Create system
let mut atari = Atari2600System::new();

// Load ROM
let rom_data = std::fs::read("game.bin")?;
atari.mount("Cartridge", &rom_data)?;

// Run one frame
let frame = atari.step_frame()?;
```

## Implementation Details

### TIA Edge Cases and Special Behaviors

This section documents important implementation details and edge cases in the TIA emulation.

#### Horizontal Positioning (RESPx/RESMx/RESBL)

✅ **Correctly Implemented**

Horizontal position is set by **strobing** registers at a specific time, not by writing a value. When RESP0/RESP1/RESM0/RESM1/RESBL is written, the position is set based on the current beam position.

```rust
// Position is set based on current beam position when register is written
0x10 => self.player0_x = self.current_visible_x(),
0x11 => self.player1_x = self.current_visible_x(),
```

This is the correct "racing the beam" technique used by Atari 2600 games.

#### Horizontal Motion (HMxx Registers)

✅ **Fully Implemented**

HMxx registers store signed 4-bit values for fine-tuning position after RESP. The implementation:
- Stores HMxx values when written to registers 0x20-0x24
- Applies motion when HMOVE (0x2A) is triggered
- Clears motion values when HMCLR (0x2B) is written

```rust
// From TIA implementation (src/tia.rs)
// Apply horizontal motion when HMOVE (0x2A) is written
0x2A => {
    self.player0_x = self.apply_motion(self.player0_x, self.hmp0);
    self.player1_x = self.apply_motion(self.player1_x, self.hmp1);
    self.missile0_x = self.apply_motion(self.missile0_x, self.hmm0);
    self.missile1_x = self.apply_motion(self.missile1_x, self.hmm1);
    self.ball_x = self.apply_motion(self.ball_x, self.hmbl);
}

// Helper function in TIA implementation
fn apply_motion(&self, pos: u8, motion: i8) -> u8 {
    let p = pos as i16;
    let m = motion as i16;
    (p + m).clamp(0, 159) as u8
}
```

#### Playfield Bit Ordering

✅ **Correctly Implemented**

Playfield registers have unusual bit ordering - PF0 is reversed!
- PF0 bits are reversed: bit 4 is leftmost, bit 7 is rightmost
- PF1 and PF2 use normal bit ordering

#### Playfield Reflection vs. Repeat Mode

✅ **Correctly Implemented**

CTRLPF bit 0 controls whether right half mirrors or repeats left half:
- **Reflection mode** (bit 0 = 1): Right half is mirror of left (bit reversed)
- **Repeat mode** (bit 0 = 0): Right half is exact copy of left

#### Playfield Priority Mode

✅ **Correctly Implemented**

CTRLPF bit 2 changes rendering order:
- **Default priority**: Players → Missiles → Ball → Playfield → Background
- **Playfield priority**: Playfield/Ball → Players → Missiles → Background

#### WSYNC (Wait for Horizontal Sync)

✅ **Correctly Implemented**

WSYNC must halt the CPU until the **current scanline completes**, not the next scanline. The implementation correctly waits for remaining cycles in current scanline:

```rust
if bus.take_wsync_request() {
    let extra = bus.tia.cpu_cycles_until_scanline_end();
    bus.clock(extra);
}
```

This is critical for games that use racing the beam techniques.

#### Color Clock Precision

✅ **Correctly Implemented**

Each pixel is 4 color clocks wide, not 1. Playfield bits control 4-pixel blocks. Tests validate correct 4-pixel scaling.

### RIOT Edge Cases and Special Behaviors

#### Timer Interval Switching

✅ **Correctly Implemented**

Writing to TIM1T, TIM8T, TIM64T, T1024T sets timer AND changes interval. Intervals: 1, 8, 64, or 1024 CPU cycles per decrement.

#### Timer Underflow Flag (TIMINT)

✅ **Correctly Implemented**

TIMINT flag auto-clears on read - critical for synchronization loops:

```rust
// From RIOT implementation (src/riot.rs) - reading TIMINT clears the flag
0x0285 => {
    let val = if self.timer_underflow.get() { 0x80 } else { 0x00 };
    self.timer_underflow.set(false); // Clear on read
    val | (self.timer & 0x7F)
}
```

#### Timer Continues After Zero

✅ **Correctly Implemented**

After reaching 0, timer continues decrementing at 1 cycle/decrement (ignoring interval), wrapping to 0xFF.

#### RAM Mirroring

✅ **Correctly Implemented**

128 bytes of RAM are mirrored multiple times in address space:
- RAM accessible at $80-$FF, $00-$7F, $100-$17F
- Mirroring handled by address masking

#### Console Switches (SWCHB)

✅ **Correctly Implemented**

SWCHB uses active-low logic:
- Reset, Select: 0 = pressed
- Color/BW: 0 = BW, 1 = Color
- Difficulty switches: 0 = A/Pro, 1 = B/Amateur

### Input Handling

#### Fire Button Logic (INPT4/INPT5)

✅ **Correctly Implemented**

Fire buttons use active-low bit 7 - 0 = pressed, 1 = released.

```rust
pub fn set_fire_button(&mut self, player: u8, pressed: bool) {
    let value = if pressed { 0x00 } else { 0x80 };
    match player {
        0 => self.inpt4 = value,
        1 => self.inpt5 = value,
        _ => {}
    }
}
```

#### Joystick Direction Logic (SWCHA)

✅ **Correctly Implemented**

Joystick directions use active-low - 0 = pressed, 1 = released.
- Player 0: bits 0-3 (Up, Down, Left, Right)
- Player 1: bits 4-7 (Up, Down, Left, Right)

Tests validate active-low logic.

#### Joystick Direction Conflicts

✅ **Hardware-Accurate**

Opposite directions pressed simultaneously (Up+Down or Left+Right) are allowed - both can be active. Hardware allows simultaneous opposite directions, though some games may have undefined behavior.

### Cartridge and Banking

#### Bank Switching Hotspots

✅ **Correctly Implemented**

Reading from specific addresses (not writing!) triggers bank switches:
- F8 (8K): Switch at $1FF8-$1FF9
- F6 (16K): Switch at $1FF6-$1FF9
- F4 (32K): Switch at $1FF4-$1FFB
- FA (12K): Switch at $1FF8-$1FFA

#### Simultaneous TIA/RAM Write

✅ **Correctly Implemented**

Addresses $40-$7F write to BOTH TIA and RAM simultaneously on real hardware:

```rust
0x0040..=0x007F => {
    self.tia.write((addr & 0x3F) as u8, val);
    self.riot.write(addr, val);
}
```

## Known Limitations

See [User Manual](https://hemulator.56k.guru/user/manual.html#atari-2600) for user-facing limitations.

### Implemented Features

#### Player/Missile Sizing (NUSIZ)

✅ **Implemented**

NUSIZ registers (0x04, 0x05) control sprite width and duplication:
- **Size modes**: 1x (8 pixels), 2x (16 pixels), 4x (32 pixels) ✅
- **Duplication modes**: None, Close (16px apart), Medium (32px), Wide (64px) ✅  
- **Missile sizes**: 1px, 2px, 4px, 8px widths ✅
- **Impact**: High - many games use sprite sizing and duplication (e.g., Space Invaders for duplicated invaders)

#### Collision Detection

✅ **Implemented**

TIA has 8 collision registers (CXM0P, CXM1P, CXM0FB, etc.) that set bits when sprites overlap:
- **Collision registers**: All 8 collision registers implemented ✅
- **CXCLR**: Clear collision registers supported ✅
- **Impact**: High - enables proper gameplay for many games (Asteroids, Breakout, Combat)

#### Ball Sizing

✅ **Implemented**

CTRLPF bits 4-5 control ball size (1, 2, 4, or 8 pixels):
- **Size modes**: 1px, 2px, 4px, 8px ✅
- **Impact**: Medium - improves visual accuracy for ball-based games (Pong, Breakout)
- **Implementation**: Hardware-accurate sizing based on CTRLPF register bits 4-5

#### Delayed Ball Graphics (VDELBL)

✅ **Implemented**

VDELBL delays ball enable/disable by one scanline, similar to VDELP for players:
- **VDELBL register**: Implemented at 0x27 ✅
- **Impact**: Low-Medium - Used by some games for flicker reduction
- **Implementation**: Ball uses previous ENABL state when VDELBL is enabled

#### Reset Missile to Player (RESMP0/RESMP1)

✅ **Implemented**

RESMP0/RESMP1 lock missiles to their associated player positions:
- **RESMP0/RESMP1 registers**: Implemented at 0x28/0x29 ✅
- **Positioning**: Missile locked to center of player (player_x + 4) ✅
- **Impact**: Medium - Used by some games for synchronized player/missile graphics
- **Implementation**: Missiles automatically follow players when RESMP is enabled

#### Delayed Graphics Registers (VDELPx)

✅ **Implemented**

VDELP0/VDELP1 delay player graphics update by one scanline for smoother animation:
- **VDELP0/VDELP1**: Both registers implemented ✅
- **Impact**: Medium - improves rendering for games using delayed graphics for flicker reduction
- **Use case**: Multi-sprite games rely on this for smooth animation

### Not Implemented Features

These features are not yet implemented but would improve game compatibility:

#### Paddle Controllers

❌ **Not Implemented**

INPT0-INPT3 are used for paddle/driving controller analog input but always return 0. Paddle timing circuits not emulated.

- **Impact**: High for paddle games (Breakout, Kaboom!, Warlords)
- **Games affected**: All paddle-based games are unplayable

#### Exotic Banking Schemes

❌ **Not Implemented**

Only standard schemes supported: 2K, 4K, F8, FA, F6, F4. Missing formats:
- **DPC** (Pitfall II) - Display Processor Chip with additional graphics capabilities
- **FE** (Decathlon) - Write-based bank switching
- **3F** (Espial) - RAM-based banking with 2K banks
- **E0** (Parker Bros) - Multiple simultaneous banks

- **Impact**: Medium - affects specific commercial games
- **Games affected**: Pitfall II, Decathlon, Espial, Parker Bros titles

### Hardware-Accurate Behaviors (Not Bugs)

These behaviors are authentic to the original hardware and affect all emulators:

#### Sprite Flicker (e.g., Pac-Man Ghosts)

✅ **Hardware-Accurate** ⚠️ **Appears as Bug on Modern Displays**

**Background**: The Atari 2600 TIA only supports 2 player sprites (Player 0 and Player 1) simultaneously. Games needing more sprites use "sprite multiplexing" - rapidly cycling through sprites on alternate frames.

**Example - Pac-Man**:
- Game needs 5 sprites: Pac-Man + 4 ghosts
- Hardware limitation: Only 2 sprite registers available
- Solution: Pac-Man uses Player 0, ghosts cycle through Player 1 across frames
- Result: Ghosts appear to flash/flicker

**On Original Hardware (CRT TV)**:
- CRT phosphor persistence naturally blends flickering sprites
- Fast phosphor decay creates a "ghosting" effect that makes all sprites appear semi-solid
- Flicker is less noticeable due to scanline interlacing and analog display characteristics

**On Modern Displays and Emulators**:
- LCD/LED displays have instant pixel response (no phosphor persistence)
- Digital rendering shows sharp frame transitions
- Flicker becomes very pronounced and distracting

**Solution Used by Other Emulators**:
- **Stella**: Offers "TV Effects" → "Phosphor" mode with adjustable frame blending (0-100%)
- **z26**: Basic frame blending options via configuration
- **Effect**: Blends current frame with previous frame(s) to simulate CRT phosphor decay
- **Trade-off**: Reduces flicker but can blur fast-moving objects

**Status in This Emulator**:
- ✅ Sprite rendering is cycle-accurate and hardware-correct
- ⚠️ No frame blending/phosphor simulation currently implemented
- 💡 **Workaround**: Use display filters (View menu) for CRT simulation effects
  - Filters apply scanlines and phosphor patterns but not temporal blending
  - Temporal (inter-frame) blending would require storing previous frame buffers

**Other Affected Games**:
- **Space Invaders**: Flicker on invader formations
- **Asteroids**: Flicker on asteroid sprites
- **Centipede**: Flicker on centipede segments
- Any game displaying more than 2 moving objects simultaneously

**Technical Details**:
For those interested, this is a well-documented limitation of the TIA chip. See the "References" section for hardware documentation explaining the 2-sprite limit and common multiplexing techniques.

### Timing and Rendering

#### Cycle-Accurate Rendering

The emulator uses **cycle-accurate TIA emulation**:

- Renders each pixel as it's generated (228 color clocks per scanline)
- Mid-scanline register updates affect remaining pixels on that line
- HMOVE effects happen at exact color clock boundaries
- "Racing the beam" techniques work as on hardware
- **Performance**: Runs at full speed on modern hardware
- **Compatibility**: Maximum - handles all timing-sensitive games correctly

**Timing Accuracy:** Hardware-accurate timing validated against official specifications:
- ✅ 228 color clocks per scanline
- ✅ 76 CPU cycles per scanline (3 color clocks per CPU cycle)
- ✅ 262 scanlines per frame (NTSC), 312 scanlines (PAL)
- ✅ 19,912 CPU cycles per frame (NTSC), 23,712 (PAL)
- ✅ See [ATARI_2600_TIMING_REVIEW.md](../../../ATARI_2600_TIMING_REVIEW.md) for complete timing analysis

#### Visible Window Detection

The emulator caches the first detected visible window start position to prevent frame-to-frame variation:
- VBLANK timing variations are handled gracefully
- Visible area remains stable across frames

#### Known Timing Limitations

⚠️ **Non-Standard Frame Timing**: Some homebrew games use non-standard scanline counts (e.g., 250 or 280 lines instead of 262). The emulator handles these gracefully via VSYNC detection, but visual results may vary.

⚠️ **Input DDR Enforcement**: SWACNT/SWBCNT Data Direction Registers are stored but not used to filter reads. Most games work correctly.

## Performance

- **Target**: ~60 FPS (NTSC)
- **Typical**: Runs at full speed on modern CPUs
- **Single-threaded**: Uses one CPU core
  - Frame-Based: Fastest, optimized for speed

## Future Improvements

Priority improvements for better game compatibility:

1. **Paddle Controller Support** - Essential for paddle games (Breakout, Kaboom!, Warlords)
2. **Additional Banking Schemes** (DPC, FE, 3F, E0) - Needed for specific commercial games

## Contributing

When adding Atari 2600 features:

1. **Banking Schemes**: Add to `src/cartridge.rs`
2. **Tests**: Add unit tests for new functionality
3. **Documentation**: Update this README and [User Manual](https://hemulator.56k.guru/user/manual.html)

## References

### Project Documentation
- [ARCHITECTURE.md](../../../ARCHITECTURE.md) - Overall emulation architecture
- [User Manual](https://hemulator.56k.guru/user/manual.html#atari-2600) - User manual
- [Contributing Guide](https://hemulator.56k.guru/developer/contributing.html) - Contribution guidelines
- [TIA Hardware Reference](../../../docs/src/references/tia.md) - TIA register specification

### External References
- [problemkaputt.de 2k6specs](https://problemkaputt.de/2k6specs.htm) - Comprehensive hardware specification
- [Stella Programmer's Guide](https://alienbill.com/2600/101/docs/stella.html) - Classic programming reference
- [AtariAge Programming Resources](https://www.atariage.com/2600/programming/index.html) - Community documentation

## License

Same as the parent Hemulator project.
