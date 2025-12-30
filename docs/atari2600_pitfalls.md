# Atari 2600 Emulation: Common Pitfalls and Edge Cases

This document outlines common pitfalls, edge cases, and implementation challenges when emulating the Atari 2600, along with our current implementation status.

## Table of Contents
1. [Critical Timing Issues](#critical-timing-issues)
2. [TIA (Video/Audio) Edge Cases](#tia-videoaudio-edge-cases)
3. [RIOT (RAM/IO/Timer) Edge Cases](#riot-ramiotimer-edge-cases)
4. [Cartridge and Banking Issues](#cartridge-and-banking-issues)
5. [Input Handling Pitfalls](#input-handling-pitfalls)
6. [Known Game-Specific Issues](#known-game-specific-issues)

---

## Critical Timing Issues

### 1. WSYNC (Wait for Horizontal Sync)
**Pitfall**: WSYNC must halt the CPU until the **current scanline completes**, not the next scanline.

**Our Implementation**: ✅ Correctly implemented
```rust
// Handle WSYNC - CPU halts until end of current scanline
if bus.take_wsync_request() {
    let extra = bus.tia.cpu_cycles_until_scanline_end();
    bus.clock(extra);
}
```
- Located in: `crates/systems/atari2600/src/lib.rs:304-308`
- CPU correctly waits for remaining cycles in current scanline
- Critical for games that use racing the beam techniques

### 2. Scanline Counting and Frame Timing
**Pitfall**: Atari 2600 has no fixed frame boundary - games control VSYNC manually.

**Our Implementation**: ✅ Correctly implemented
- Runs for exactly 262 scanlines per frame (NTSC standard)
- Tracks scanline wrapping from 261→0
- Safety limit prevents infinite loops (MAX_CPU_STEPS = 50,000)
- Games that don't use standard 262-line frames may have issues

**Edge Case**: Some homebrew games use non-standard scanline counts (e.g., 250 or 280 lines).
- **Status**: ⚠️ May not work correctly - our implementation assumes 262 scanlines
- **Impact**: Low - affects only exotic homebrew ROMs

### 3. VBLANK Timing
**Pitfall**: VBLANK signal doesn't directly control rendering - games must time it correctly.

**Our Implementation**: ✅ Uses dynamic visible window detection
```rust
let visible_start = bus.tia.visible_window_start_scanline();
```
- Located in: `crates/systems/atari2600/src/lib.rs:362`
- Determines visible area based on actual VBLANK timing
- Handles games with non-standard blanking periods

---

## TIA (Video/Audio) Edge Cases

### 4. Horizontal Positioning (RESPx/RESMx/RESBL)
**Pitfall**: Horizontal position is set by **strobing** registers at a specific time, not by writing a value.

**Our Implementation**: ✅ Correctly strobed
```rust
0x10 => self.player0_x = self.current_visible_x(),
0x11 => self.player1_x = self.current_visible_x(),
```
- Located in: `crates/systems/atari2600/src/tia.rs:520-521`
- Position is set based on current beam position when register is written
- This is the correct "racing the beam" technique used by Atari 2600

### 5. Horizontal Motion (HMxx registers)
**Pitfall**: HMxx registers store signed 4-bit values for fine-tuning position after RESP.

**Our Implementation**: ⚠️ Partially implemented
- HMxx registers are stored: `hmp0`, `hmp1`, `hmm0`, `hmm1`, `hmbl`
- Values are written to registers correctly
- **Missing**: Motion is NOT applied during rendering
- **Impact**: Medium - affects games that rely on fine positioning (e.g., precise sprite alignment)
- **Location**: `crates/systems/atari2600/src/tia.rs:185-189`

**TODO**: Apply horizontal motion during rendering:
```rust
// Example fix needed:
fn apply_motion(&self, pos: u8, motion: i8) -> u8 {
    let p = pos as i16;
    let m = motion as i16;
    (p + m).clamp(0, 159) as u8
}
```

### 6. Player/Missile Sizing and Duplication (NUSIZ)
**Pitfall**: NUSIZ controls sprite width (1x, 2x, 4x) and duplication (close, medium, wide copies).

**Our Implementation**: ⚠️ Registers stored but not applied
- NUSIZ registers are written but values ignored during rendering
- All sprites render at default 1x size
- **Impact**: High - many games use sprite sizing and duplication
- **Examples**: Space Invaders (duplicated invaders), Pitfall (multiple objects)

**TODO**: Implement NUSIZ modes:
- **Size**: 1x (8 pixels), 2x (16 pixels), 4x (32 pixels)
- **Duplication**: None, Close (16px apart), Medium (32px), Wide (64px)
- **Missile sizes**: 1px, 2px, 4px, 8px widths

### 7. Playfield Bit Ordering
**Pitfall**: Playfield registers have unusual bit ordering - PF0 is reversed!

**Our Implementation**: ✅ Correctly handled
- PF0 bits are reversed: bit 4 is leftmost, bit 7 is rightmost
- PF1 and PF2 use normal bit ordering
- Located in: `crates/systems/atari2600/src/tia.rs:915-930`

### 8. Playfield Reflection vs. Repeat Mode
**Pitfall**: CTRLPF bit 0 controls whether right half mirrors or repeats left half.

**Our Implementation**: ✅ Correctly implemented
- Reflection mode: right half is mirror of left (bit reversed)
- Repeat mode: right half is exact copy of left
- Located in: `crates/systems/atari2600/src/tia.rs:933-945`

### 9. Playfield Priority Mode
**Pitfall**: CTRLPF bit 2 changes rendering order - playfield in front of sprites.

**Our Implementation**: ✅ Correctly implemented
- Default: Players → Missiles → Ball → Playfield → Background
- Priority: Playfield/Ball → Players → Missiles → Background
- Located in: `crates/systems/atari2600/src/tia.rs:955-989`

### 10. Collision Detection
**Pitfall**: TIA has 15 collision registers (CXM0P, CXM1P, etc.) that must set bits when sprites overlap.

**Our Implementation**: ❌ Not implemented
- Collision registers always return 0
- Games that rely on collision detection won't work correctly
- **Impact**: High - affects many games (Asteroids, Breakout, Combat)
- **Location**: `crates/systems/atari2600/src/tia.rs:603-611`

**TODO**: Implement collision detection:
1. Track pixel-perfect overlap during rendering
2. Set appropriate collision register bits
3. Implement CXCLR to clear all collision registers

### 11. Delayed Graphics Registers (VDELPx)
**Pitfall**: VDELPx delays player graphics update by one scanline for smoother animation.

**Our Implementation**: ❌ Not implemented
- VDELP0/VDELP1 registers not present
- Old/new graphics pattern not tracked
- **Impact**: Medium - affects games using delayed graphics for flicker reduction
- **Examples**: Some multi-sprite games

### 12. Color Clock Precision
**Pitfall**: Each pixel is 4 color clocks wide, not 1. Playfield bits control 4-pixel blocks.

**Our Implementation**: ✅ Correctly implemented
- Playfield bits control 4-pixel-wide blocks
- Located in: `crates/systems/atari2600/src/tia.rs:915-945`
- Test validates 4-pixel scaling: `test_playfield_pixel_scaling()`

---

## RIOT (RAM/IO/Timer) Edge Cases

### 13. Timer Interval Switching
**Pitfall**: Writing to TIM1T, TIM8T, TIM64T, T1024T sets timer AND changes interval.

**Our Implementation**: ✅ Correctly implemented
- Writing to timer register resets value and sets interval
- Intervals: 1, 8, 64, or 1024 CPU cycles per decrement
- Located in: `crates/systems/atari2600/src/riot.rs:215-239`

### 14. Timer Underflow Flag (TIMINT)
**Pitfall**: TIMINT flag auto-clears on read - critical for synchronization loops!

**Our Implementation**: ✅ Correctly implemented
```rust
// Reading TIMINT clears the flag
0x0285 => {
    let val = if self.timer_underflow.get() { 0x80 } else { 0x00 };
    self.timer_underflow.set(false); // Clear on read
    val | (self.timer & 0x7F)
}
```
- Located in: `crates/systems/atari2600/src/riot.rs:261-266`
- Test validates flag clearing: `test_timer_interrupt_flag_behavior()`
- Critical for games using BIT TIMINT wait loops

### 15. Timer Continues After Zero
**Pitfall**: After reaching 0, timer continues decrementing at 1 cycle/decrement (ignoring interval).

**Our Implementation**: ✅ Correctly implemented
- Timer wraps to 0xFF and continues at 1-cycle rate
- Located in: `crates/systems/atari2600/src/riot.rs:289-295`

### 16. RAM Mirroring
**Pitfall**: 128 bytes of RAM are mirrored multiple times in address space.

**Our Implementation**: ✅ Correctly implemented
- RAM accessible at $80-$FF, $00-$7F, $100-$17F
- Mirroring handled by address masking
- Located in: `crates/systems/atari2600/src/bus.rs:86-91`

### 17. Input Port Data Direction (DDR)
**Pitfall**: SWACNT/SWBCNT control which pins are inputs vs. outputs.

**Our Implementation**: ⚠️ Stored but not enforced
- DDR registers are stored but not used to filter reads
- Input always returns joystick state regardless of DDR
- **Impact**: Low - most games set DDR correctly
- **Location**: `crates/systems/atari2600/src/riot.rs:169-177`

### 18. Console Switches (SWCHB)
**Pitfall**: SWCHB uses active-low logic - 0 = pressed/selected.

**Our Implementation**: ✅ Correctly implemented
- Reset, Select: 0 = pressed
- Color/BW: 0 = BW, 1 = Color
- Difficulty switches: 0 = A/Pro, 1 = B/Amateur
- Located in: `crates/systems/atari2600/src/riot.rs:198-200`

### 19. Joystick Direction Conflicts
**Pitfall**: Opposite directions pressed simultaneously (Up+Down or Left+Right).

**Our Implementation**: ✅ Both can be active
- Hardware allows simultaneous opposite directions
- Some games may have undefined behavior
- Real hardware behavior varies by controller type

---

## Cartridge and Banking Issues

### 20. Bank Switching Hotspots
**Pitfall**: Reading from specific addresses (not writing!) triggers bank switches.

**Our Implementation**: ✅ Correctly implemented
- F8 (8K): Switch at $1FF8-$1FF9
- F6 (16K): Switch at $1FF6-$1FF9
- F4 (32K): Switch at $1FF4-$1FFB
- FA (12K): Switch at $1FF8-$1FFA
- Located in: `crates/systems/atari2600/src/cartridge.rs`

### 21. Simultaneous TIA/RAM Write
**Pitfall**: Addresses $40-$7F write to BOTH TIA and RAM simultaneously on real hardware.

**Our Implementation**: ✅ Correctly implemented
```rust
0x0040..=0x007F => {
    self.tia.write((addr & 0x3F) as u8, val);
    self.riot.write(addr, val);
}
```
- Located in: `crates/systems/atari2600/src/bus.rs:134-141`
- Matches real hardware behavior

### 22. Exotic Banking Schemes
**Pitfall**: Some games use advanced banking (DPC, FE, 3F, E0, etc.).

**Our Implementation**: ❌ Not implemented
- Only standard schemes: 2K, 4K, F8, FA, F6, F4
- **Missing**: DPC (Pitfall II), FE (Decathlon), 3F (Espial), E0 (Parker Bros)
- **Impact**: Medium - affects specific commercial games
- **TODO**: Add these banking schemes for better compatibility

---

## Input Handling Pitfalls

### 23. Fire Button Logic (INPT4/INPT5)
**Pitfall**: Fire buttons use active-low bit 7 - 0 = pressed, 1 = released.

**Our Implementation**: ✅ Correctly implemented
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
- Located in: `crates/systems/atari2600/src/tia.rs:377-384`
- Matches hardware behavior

### 24. Joystick Direction Logic (SWCHA)
**Pitfall**: Joystick directions use active-low - 0 = pressed, 1 = released.

**Our Implementation**: ✅ Correctly implemented
- Player 0: bits 0-3 (Up, Down, Left, Right)
- Player 1: bits 4-7 (Up, Down, Left, Right)
- Located in: `crates/systems/atari2600/src/riot.rs:334-346`
- Tests validate active-low logic

### 25. Paddle Controllers
**Pitfall**: INPT0-INPT3 are used for paddle/driving controller analog input.

**Our Implementation**: ❌ Not implemented
- INPT0-INPT3 always return 0
- Paddle timing circuits not emulated
- **Impact**: High for paddle games (Breakout, Kaboom!, Warlords)
- **Location**: `crates/systems/atari2600/src/tia.rs:604-605`

---

## Known Game-Specific Issues

### 26. Racing the Beam
**Issue**: Games that update graphics mid-scanline for effects.

**Our Implementation**: ⚠️ Partial support
- State is latched per-scanline after writes
- May not handle rapid mid-scanline updates perfectly
- **Impact**: Some visual effects may not render correctly

### 27. Kernel Variations
**Issue**: Different games use different display kernels (2-line, asymmetric, etc.).

**Our Implementation**: ✅ Should handle most kernels
- Frame-based rendering adapts to actual register writes
- Dynamic visible window detection handles variations
- **Impact**: Most kernels should work

### 28. Flicker Reduction Techniques
**Issue**: Games use various tricks to display >5 sprites (multi-sprite flicker).

**Our Implementation**: ⚠️ Depends on technique
- Frame-based rendering may not show flicker correctly
- Delayed graphics not implemented (affects some techniques)
- **Impact**: Some multi-sprite scenes may look different

---

## Summary and Recommendations

### Critical Issues (High Priority)
1. ❌ **Collision Detection** - Many games rely on this
2. ⚠️ **Horizontal Motion** - Affects sprite positioning in many games
3. ⚠️ **NUSIZ (Sizing/Duplication)** - Common feature used by many games
4. ❌ **Paddle Controllers** - Required for paddle games

### Medium Priority
5. ❌ **Delayed Graphics** - Affects some multi-sprite games
6. ❌ **Exotic Banking** - Needed for specific commercial games

### Low Priority
7. ⚠️ **Non-standard Frame Timing** - Affects only exotic homebrew
8. ⚠️ **Input DDR Enforcement** - Most games work without it

### Working Correctly ✅
- WSYNC timing
- Horizontal positioning (RESPx)
- Playfield rendering (bit order, reflection, priority)
- RIOT timer (intervals, underflow flag, continuation)
- Bank switching (standard schemes)
- Input handling (active-low logic for joystick and fire)
- RAM mirroring
- Simultaneous TIA/RAM writes

---

## Testing Recommendations

To validate these edge cases, consider:

1. **Test ROMs**: Use Atari 2600 test suite ROMs
2. **Known Games**: Test with games that stress specific features
   - Combat (basic functionality)
   - Space Invaders (NUSIZ duplication)
   - Asteroids (collision detection)
   - Breakout (paddle input, collision)
   - Pitfall II (DPC banking)

3. **Visual Inspection**: Compare output with other emulators (Stella)
4. **Frame-perfect Recording**: Record and compare frame sequences

---

## References

- **Stella Programmer's Guide**: Official Atari 2600 documentation
- **TIA Hardware Notes**: Detailed TIA timing and behavior
- **6532 RIOT Datasheet**: Timer and I/O specifications
- **Atari 2600 Mappers**: Banking scheme documentation

---

*Last Updated*: 2025-12-30
*Implementation Version*: Current hemulator codebase
