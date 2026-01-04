# Atari 2600 Timing and Positioning Analysis

This document analyzes the current implementation against problemkaputt.de/2k6specs.htm for timing accuracy, particularly focusing on video output, horizontal positioning, and vertical synchronization.

## Timing Specifications (NTSC)

### Horizontal Timing (Per Scanline)
Per problemkaputt.de spec:
- **Total color clocks**: 228 per scanline
- **HBLANK**: ~68 color clocks (invisible)
- **Visible pixels**: 160 color clocks (68-227)
- **TIA frequency**: 3.579545 MHz (color clock)
- **CPU frequency**: 1.19318 MHz (≈3 color clocks per CPU cycle)

**Current Implementation** (tia.rs):
```rust
const HBLANK_COLOR_CLOCKS: i16 = 68;  ✅ CORRECT

pub fn clock(&mut self) {
    self.pixel += 3; // 3 color clocks per CPU cycle ✅ CORRECT
    
    if self.pixel >= 228 { // ✅ CORRECT
        self.pixel -= 228; // Proper wraparound ✅ CORRECT
        self.latch_scanline_state(old_scanline);
        self.scanline += 1;
        if self.scanline >= 262 {
            self.scanline = 0;
        }
    }
}
```

**Status**: ✅ Horizontal timing appears spec-compliant

### Vertical Timing (Per Frame)
Per problemkaputt.de spec:
- **Total scanlines**: 262 (NTSC)
- **VSYNC period**: ~3 scanlines (vertical sync pulse)
- **VBLANK period**: ~37-40 scanlines (overscan + VSYNC + top blanking)
- **Visible scanlines**: ~192 scanlines
- **Frame rate**: ~60 Hz

**Current Implementation**:
```rust
if self.scanline >= 262 {
    self.scanline = 0; // ✅ CORRECT wraparound
}
```

**Status**: ✅ Vertical timing wraparound is correct

## Horizontal Positioning (RESP0/RESP1)

### Specification Behavior
Per Atari 2600 programming guides:
1. Sprites are positioned by **strobing** RESP0/RESP1 at the desired time
2. Writing to RESP sets position based on **current color clock**
3. Position = (current_pixel - 68) for visible area
4. Writing during HBLANK (pixel < 68) positions sprite offscreen left

### Current Implementation
```rust
fn current_visible_x(&self) -> u8 {
    let x = (self.pixel as i16) - Self::HBLANK_COLOR_CLOCKS;
    x.clamp(0, 159) as u8 // ⚠️ POTENTIAL ISSUE
}

// When RESP0 is written:
0x10 => self.player0_x = self.current_visible_x(),
```

### Potential Issue: HBLANK Positioning
**Problem**: Clamping negative values to 0 may not accurately represent hardware behavior

**Hardware behavior** (per spec):
- Writing RESP at pixel 0-67 (HBLANK): sprite is offscreen/invisible
- Writing RESP at pixel 68: sprite at position 0
- Writing RESP at pixel 227: sprite at position 159

**Current behavior**:
- Writing RESP at pixel 0-67: sprite at position 0 (clamped)
- Writing RESP at pixel 68: sprite at position 0
- Writing RESP at pixel 227: sprite at position 159

**Question**: Should sprites positioned during HBLANK:
1. Be clamped to position 0? (current)
2. Be marked as invisible/offscreen? (may be more accurate)
3. Wrap around to right side? (unlikely based on research)

### Testing Recommendation
Need to test with actual games that use specific positioning techniques to determine correct behavior.

## Vertical Scroll Issues

### Visible Window Detection
The implementation caches the first detected visible window start to prevent vertical jumping:

```rust
pub fn visible_window_start_scanline(&mut self) -> u16 {
    if let Some(cached) = self.cached_visible_start {
        return cached; // ✅ Prevents vertical jumping
    }
    
    // Detect VBLANK false transition
    for i in 1..262 {
        let prev = self.scanline_states.get(i - 1).copied().unwrap_or_default();
        let cur = self.scanline_states.get(i).copied().unwrap_or_default();
        
        if prev.vblank && !cur.vblank {
            self.cached_visible_start = Some(i as u16);
            return i as u16;
        }
    }
    
    self.cached_visible_start = Some(40); // Fallback
    40
}
```

**Potential Issues**:
1. **One-time caching**: Once cached, window never updates even if game changes VBLANK timing
2. **Frame detection**: Relies on VBLANK transitions which may vary between frames
3. **Fallback value**: Hardcoded to 40 may not match all games

### Recommendation
If vertical scroll issues occur:
1. Add option to reset `cached_visible_start` on game state changes
2. Consider averaging visible_start over multiple frames
3. Add logging to track visible_start stability

## Memory Handling

### Address Bus (13-bit)
```rust
fn read(&self, addr: u16) -> u8 {
    let addr = addr & 0x1FFF; // ✅ CORRECT: 13-bit masking
    // ...
}
```

### Mirroring
- **TIA mirrors**: ✅ Correctly implemented at multiple address ranges
- **RAM mirrors**: ✅ Correctly implemented at $00-$7F, $80-$FF, $180-$1FF
- **Dual-write behavior**: ✅ Correctly implements $40-$7F writes to both TIA and RAM

**Status**: ✅ Memory handling appears spec-compliant

## WSYNC (Wait for Horizontal Sync)

```rust
pub fn cpu_cycles_until_scanline_end(&self) -> u32 {
    let pixel = self.pixel.min(227) as u32;
    let remaining_color_clocks = 228u32.saturating_sub(pixel);
    let extra = remaining_color_clocks.div_ceil(3);
    extra.max(1)
}
```

**Status**: ✅ WSYNC calculation appears correct

## Recommendations for Further Investigation

### If Horizontal Position Issues Occur:
1. **Test with known games**: Combat, Space Invaders, Pac-Man use different positioning techniques
2. **Check HMOVE artifacts**: HMOVE should cause "HMOVE comb" if not handled correctly
3. **Verify fine positioning**: HMxx registers provide -8 to +7 fine adjustment
4. **Log positioning writes**: Add debug logging for RESP0/1 writes and resulting positions

### If Vertical Scroll Issues Occur:
1. **Check VSYNC detection**: Verify VSYNC edges are detected correctly
2. **Monitor visible_start**: Log visible_start per frame to detect instability
3. **Test VBLANK transitions**: Ensure VBLANK false transition is detected consistently
4. **Frame boundary detection**: Verify frame boundaries align with VSYNC pulses

### Test ROM Recommendations:
1. Create test ROM that writes RESP at various times (HBLANK vs visible)
2. Create test ROM with varying VBLANK timing
3. Test with commercial ROMs known to use precise timing (racing games, etc.)

## Summary

**Spec Compliance**: Implementation appears highly spec-compliant for:
- ✅ Horizontal timing (228 color clocks/scanline)
- ✅ Vertical timing (262 scanlines/frame)
- ✅ Memory addressing and mirroring
- ✅ Clock ratios (3 color clocks per CPU cycle)

**Potential Issues to Investigate**:
- ⚠️ HBLANK positioning behavior (clamping vs offscreen)
- ⚠️ Visible window caching (one-time vs adaptive)
- ⚠️ Need specific test cases to identify exact timing issues

**Next Steps**:
- Await specific details on observed issues
- Create targeted test ROMs
- Add debug logging for timing-sensitive operations
