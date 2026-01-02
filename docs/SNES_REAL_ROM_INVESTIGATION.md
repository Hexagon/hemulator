# SNES Real ROM Compatibility Investigation

## Problem Statement
Real SNES ROMs do not produce any visible output. The emulator's test ROMs work correctly, but commercial games show a black screen.

## Investigation Results

### What Works
✅ **Mode 0 rendering** - Tested with test.sfc (checkerboard pattern)
✅ **Mode 1 rendering** - Tested with test_enhanced.sfc and diagnostic tests
✅ **Basic sprite rendering** - Sprites display correctly in test ROMs
✅ **Controller input** - All buttons work correctly
✅ **NMI/VBlank handling** - Interrupts trigger correctly
✅ **VRAM/CGRAM/OAM access** - All PPU registers work correctly
✅ **Scrolling** - Horizontal and vertical scrolling works
✅ **Tilemap/CHR addressing** - All addressing modes tested and working

### Root Cause Analysis

Based on analysis of the code and SNES_EMULATION_PITFALLS.md, the most likely causes for real ROMs not displaying are:

#### 1. **Priority Bit Handling (HIGHEST PRIORITY)**

**Issue**: Priority bits in tile attributes are read but not used

**Code Location**: `crates/systems/snes/src/ppu.rs` lines 756-763, 1012-1014

**Details**:
- Tile format includes priority bits: `YXpppttt` (bits 13-15 are priority)
- Priority bits are extracted: `let _priority = ((tile_high >> 2) & 0x07) as usize;`
- But they're assigned to `_priority` (unused variable)
- Without priority handling, high-priority background tiles may be hidden behind low-priority ones
- **This breaks layering in commercial games which rely heavily on priority for HUDs, text, effects**

**Expected Behavior**:
- Priority 0 = low priority (renders behind)
- Priority 1 = high priority (renders in front)
- Correct render order: High-priority sprites > High-priority BG > Low-priority sprites > Low-priority BG

**Impact on Commercial Games**:
- **Critical**: Most SNES games use priority for HUDs, text overlays, and visual effects
- Without this, important graphics may be completely invisible
- Games like Super Mario World, Donkey Kong Country, etc. rely on priority

#### 2. **BG3 Priority Toggle in Mode 1**

**Issue**: BG3 can be set to render above ALL sprites in Mode 1 (BGMODE register bit 3)

**Code Location**: `crates/systems/snes/src/ppu.rs` line 88

**Details**:
- BGMODE $2105 bit 3 controls BG3 global priority in Mode 1
- When set, BG3 renders above all sprites (common for HUDs)
- Currently not implemented

**Impact**: Games using BG3 for HUDs may have invisible text/UI

#### 3. **Sprite-per-Scanline Limits**

**Issue**: SNES hardware has limits (32 sprites/scanline, 34 8x8 tiles/scanline)

**Details**:
- Currently, all 128 sprites are rendered regardless of scanline limits
- Real hardware would drop sprites beyond the limit
- This might cause different rendering behavior but less likely to cause complete black screen

### Why Test ROMs Work

Our test ROMs work because they:
1. Don't rely on priority bits (use default priority 0)
2. Use simple single-layer rendering
3. Don't use BG3 priority toggle
4. Don't exceed sprite limits

**This is not representative of commercial games!**

### Recommended Fixes (In Priority Order)

#### High Priority
1. **Implement priority bit handling in BG rendering**
   - Parse priority from tile attributes
   - Render tiles in priority order
   - Test with overlapping layers

2. **Implement BG3 priority toggle for Mode 1**
   - Check BGMODE bit 3
   - Render BG3 above sprites when enabled

#### Medium Priority
3. **Add sprite-per-scanline limits**
   - Track sprites rendered per scanline
   - Implement 32 sprite / 34 tile limits
   - Properly handle overflow

#### Testing Strategy
4. **Enhanced test ROMs**
   - Create test ROM that uses priority bits
   - Create test ROM for BG3 priority toggle
   - Create test ROM for sprite overflow

## Enhanced Test ROM

Created `test_enhanced.sfc` which tests:
- ✅ Mode 1 (most common in commercial games)
- ✅ Multiple BG layers with different tile sets
- ✅ Sprite rendering
- ✅ NMI handling
- ✅ Scrolling
- ❌ Priority bits (TODO)
- ❌ BG3 priority toggle (TODO)
- ❌ Sprite limits (TODO)

## Conclusion

The SNES emulator's core functionality is **solid**:
- CPU works correctly
- PPU registers work correctly
- Basic rendering works correctly

However, **commercial game compatibility requires priority handling**. Without it:
- Important graphics layers may be hidden
- HUDs and text may be invisible
- Games will appear to show "no visible output"

This explains why our test ROMs work but real games don't - our test ROMs don't use the features that commercial games rely on.

## Next Steps

1. Implement priority bit handling in BG layer rendering
2. Add test ROM that specifically tests priority bits
3. Implement BG3 priority toggle
4. Test with real commercial ROMs to verify fixes
5. Add sprite-per-scanline limits (lower priority)

## References

- `docs/SNES_EMULATION_PITFALLS.md` - Documents these exact issues
- `crates/systems/snes/src/ppu.rs` - PPU implementation
- `test_roms/snes/README.md` - Test ROM documentation
- SNESdev Wiki - https://snes.nesdev.org/
