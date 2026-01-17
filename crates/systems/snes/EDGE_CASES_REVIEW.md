# SNES Emulator Edge Case Review

## Purpose

This document reviews the SNES emulator implementation for accuracy against known hardware edge cases and quirks. The goal is to ensure the emulator handles uncommon but important hardware behaviors correctly.

## References

- **Primary Hardware Reference**: https://wiki.superfamicom.org
- **Anomie's Doc**: https://snes.nesdev.org/wiki/Anomie%27s_Doc
- **fullsnes**: https://problemkaputt.de/fullsnes.htm

## Edge Cases by Category

### 1. OAM (Object Attribute Memory) Edge Cases ✅

#### 1.1 OAM Address Auto-Increment
**Hardware Behavior**: Writing to $2104 (OAMDATA) auto-increments OAM address.
**Expected**: Address wraps at 544 bytes (512 main + 32 high table).
**Implementation Status**: ✅ CORRECT
- File: `ppu.rs` lines 564-566
- Uses wrapping add: `(oam_addr + 1) % 544`
- Test: Basic functionality tested

**Edge Case**: What happens when OAM address is set to 543 and a write occurs?
- Expected: Wraps to 0
- Status: ✅ Handled by modulo operation

#### 1.2 Sprite Priority Rotation
**Hardware Behavior**: Bit 7 of $2103 enables sprite priority rotation.
**Expected**: When enabled, sprite at `(OAMAddr & 0xFE) >> 1` gets highest priority.
**Implementation Status**: ✅ CORRECT
- File: `ppu.rs` lines 3316-3323
- Correctly calculates first sprite: `((self.oam_addr & 0x1FE) >> 1)`
- Test: `test_sprite_priority_rotation()` validates enable/disable

**Edge Case**: Mid-frame priority rotation changes
- Hardware: Changes take effect immediately but rendering order is determined at scanline start
- Status: ⚠️ NOT TESTED - Current implementation renders full frame at once
- Recommendation: Document that frame-based rendering doesn't support mid-frame changes

#### 1.3 Sprite Overflow Limits
**Hardware Behavior**:
- Maximum 32 sprites per scanline (Range Over flag - $213E bit 6)
- Maximum 34 8x8 tile slots per scanline (Time Over flag - $213E bit 7)
**Implementation Status**: ✅ CORRECT
- File: `ppu.rs` lines 3418-3446
- Correctly tracks sprites/tiles per scanline
- Sets overflow flags appropriately
- Test: `test_sprite_overflow.sfc` test ROM exists

**Edge Case**: Sprite partially on scanline
- Hardware: Counts against scanline limit for all rows it touches
- Status: ✅ CORRECT - Loops through all scanlines sprite occupies

**Edge Case**: What if sprite would cause overflow mid-sprite?
- Hardware: Entire sprite is skipped if any scanline would overflow
- Status: ✅ CORRECT - Checks all scanlines before rendering

### 2. VRAM Access Edge Cases ✅

#### 2.1 VRAM Address Increment Modes
**Hardware Behavior**: $2115 (VMAIN) controls increment behavior
- Bits 0-1: Increment amount (0=1, 1=32, 2-3=128 words)
- Bit 7: When to increment (0=after low byte, 1=after high byte)
**Implementation Status**: ✅ CORRECT
- File: `ppu.rs` lines 750-829, 1789-1796
- Test: `test_vram_read_registers()` validates increment modes

**Edge Case**: Writing only low byte then switching address
- Hardware: No increment occurs if corresponding write didn't happen
- Status: ✅ CORRECT - Only increments on specified byte write

#### 2.2 VRAM Read Prefetch Buffer
**Hardware Behavior**: Reading VRAM returns previously buffered value, then prefetches next.
**Expected**: Setting VRAM address causes immediate prefetch.
**Implementation Status**: ✅ CORRECT
- File: `ppu.rs` lines 758-770
- Prefetches on address write
- Returns buffered value on read
- Test: `test_vram_read_buffer_prefetch()` validates behavior

**Edge Case**: Read immediately after address set
- Hardware: Returns old buffer contents from previous address
- Status: ✅ CORRECT - Buffer updated on address set, not cleared

**Edge Case**: VRAM address wraparound
- Hardware: Address wraps at 64KB (32K words)
- Status: ✅ CORRECT - Uses `% (VRAM_SIZE / 2)` for word addressing

#### 2.3 VRAM Accessibility
**Hardware Behavior**: VRAM only accessible during VBlank, HBlank, or force blank
**Implementation Status**: ✅ IMPLEMENTED with logging
- File: `ppu.rs` lines 1894-1902
- Logs warnings for invalid access
- Some games rely on HBlank writes (documented)

**Edge Case**: What happens to writes during active display?
- Hardware: Writes are ignored
- Status: ✅ CORRECT - Writes are ignored with warning log

### 3. CGRAM (Color Generator RAM) Edge Cases ⚠️

#### 3.1 CGRAM Write Latch
**Hardware Behavior**: $2122 writes toggle between low/high bytes
**Implementation Status**: ✅ CORRECT
- File: `ppu.rs` lines 838-874
- Latch toggles on each write
- Address increments after high byte
- Test: `test_cgram_address_increment()` validates behavior

**Edge Case**: What if $2121 is written mid-color?
- Hardware: Resets write latch to low byte
- Status: ✅ CORRECT - Line 834 resets latch on address write

**Edge Case**: Partial color write then jump to different address
- Hardware: Color is left half-written
- Status: ⚠️ NOT TESTED but implementation appears correct

#### 3.2 CGRAM Read Behavior
**Hardware Behavior**: Reading $213B returns CGRAM byte
**Implementation Status**: ✅ IMPLEMENTED
- File: `ppu.rs` lines 1129-1137
- Respects write latch state for address calculation

**Edge Case**: Does CGRAM read toggle the latch?
- Hardware: Read does NOT toggle latch (only writes do)
- Status: ⚠️ NEEDS VERIFICATION - Current implementation doesn't toggle on read
- Recommendation: Add test to verify latch state unchanged after read

**Edge Case**: Does CGRAM read auto-increment?
- Hardware: Read does NOT auto-increment address
- Status: ✅ CORRECT - No increment in read_register()

### 4. Mode 7 Edge Cases ⚠️

#### 4.1 Screen Over Modes
**Hardware Behavior**: $211A bits 6-7 control behavior outside 1024x1024 map
- 00: Wrap (default)
- 01: Transparent outside bounds
- 10/11: Tile 0 outside bounds
**Implementation Status**: ✅ CORRECT
- File: `ppu.rs` lines 2875-3011
- All three modes implemented

**Edge Case**: What happens at exact boundary (1023, 1023)?
- Hardware: Wrapping uses bitwise AND with 0x3FF
- Status: ✅ CORRECT - Uses `& 0x3FF` for wrapping

#### 4.2 Fixed-Point Overflow
**Hardware Behavior**: Matrix values are 8.8 signed fixed-point
**Expected**: Values from -128.0 to +127.996
**Implementation Status**: ✅ IMPLEMENTED
- File: `ppu.rs` lines 2942-2943
- Uses signed 16-bit with proper bit shifting

**Edge Case**: What if transformed coordinates overflow?
- Hardware: Coordinates wrap/clip depending on screen over mode
- Status: ⚠️ NOT TESTED
- Recommendation: Add tests for extreme matrix values causing overflow

**Edge Case**: What if M7A-M7D are all zero?
- Hardware: Produces black screen (all pixels map to 0,0)
- Status: ⚠️ NOT TESTED
- Recommendation: Add test for zero matrix

#### 4.3 Center Point Sign Extension
**Hardware Behavior**: M7X and M7Y are 13-bit signed values
**Expected**: Sign extension from bit 12
**Implementation Status**: ✅ CORRECT
- File: `ppu.rs` lines 2894-2908
- Uses i16 for storage, proper sign handling

**Edge Case**: Maximum/minimum center point values
- Hardware: Range is -4096 to +4095
- Status: ⚠️ NOT TESTED
- Recommendation: Add tests for extreme center values

### 5. Scroll Register Edge Cases ✅

#### 5.1 Double-Write Latch
**Hardware Behavior**: Scroll registers ($210D-$2114) require two writes
**Expected**: First write sets low bits, second write sets high bits
**Implementation Status**: ✅ CORRECT
- File: `ppu.rs` lines 618-703
- All scroll registers use double-write pattern
- Shares common `scroll_latch` state

**Edge Case**: Writing same register three times
- Hardware: Third write becomes new low byte, latch resets
- Status: ✅ CORRECT - Latch alternates properly

**Edge Case**: Writing different scroll registers interleaved
- Hardware: Each has independent latch, shares previous value
- Status: ⚠️ POTENTIAL ISSUE - All scroll registers share one `scroll_latch`
  - This means writing BG1H then BG2H uses BG1H's low byte for BG2H's low byte
  - NEEDS VERIFICATION: Is this hardware-accurate or a bug?
  - Reference: https://wiki.superfamicom.org/backgrounds says "previous value" is shared

**Edge Case**: Reading scroll register
- Hardware: Write-only registers, reads return open bus
- Status: ⚠️ NOT IMPLEMENTED - Would need bus state tracking

### 6. Mode 7 Matrix Register Edge Cases ⚠️

#### 6.1 Double-Write Behavior
**Hardware Behavior**: M7A-M7D, M7X, M7Y require two writes (low then high)
**Implementation Status**: ✅ IMPLEMENTED
- File: `ppu.rs` lines 713-747
- All use shared `m7_prev` for low byte

**Edge Case**: Do Mode 7 registers share latch with scroll?
- Hardware: Mode 7 has separate latch (m7_prev vs scroll_prev)
- Status: ✅ CORRECT - Separate latch variables

**Edge Case**: Writing M7A then M7B uses M7A's low byte?
- Hardware: YES - all Mode 7 registers share the "previous value"
- Status: ✅ CORRECT - Shares `m7_prev` across all M7 registers

### 7. DMA/HDMA Edge Cases ✅

**Note**: DMA/HDMA was thoroughly reviewed in DMA_HDMA_REVIEW.md
- Transfer mode patterns: ✅ All 8 modes correct
- Timing: ⚠️ Missing minor overhead cycles (documented, not critical)

### 8. Interrupt Edge Cases ✅

#### 8.1 NMI Flag Clearing
**Hardware Behavior**: $4210 bit 7 is read-and-clear
**Implementation Status**: ✅ CORRECT
- Documented in README (line 424)
- Handled in bus.rs

**Edge Case**: Reading $213F also clears NMI flag
- Hardware: $213F bit 7 mirrors NMI flag and clears on read
- Status: ⚠️ NEEDS VERIFICATION
- File: `ppu.rs` line 1835 suggests it's handled

## Summary of Findings

### Fully Correct (Hardware Accurate)
1. ✅ OAM address increment and wrapping
2. ✅ Sprite priority rotation
3. ✅ Sprite overflow limits (32 sprites, 34 tiles per scanline)
4. ✅ VRAM address increment modes
5. ✅ VRAM prefetch buffer
6. ✅ CGRAM write latch behavior
7. ✅ Mode 7 screen over modes
8. ✅ DMA/HDMA transfer patterns

### Needs Testing
1. ⚠️ Mode 7 fixed-point overflow scenarios
2. ⚠️ Mode 7 zero matrix behavior
3. ⚠️ Mode 7 extreme center point values
4. ⚠️ CGRAM read latch behavior
5. ⚠️ Scroll register shared latch behavior
6. ⚠️ OAM rotation mid-frame changes (documented limitation)

### Recommendations

#### High Priority (Testing Needed)
1. Add test for scroll register shared latch behavior
2. Add test for CGRAM read latch state (verify no toggle)
3. Add tests for Mode 7 edge cases:
   - Zero matrix
   - Extreme center points
   - Overflow scenarios

#### Medium Priority (Documentation)
1. Document that frame-based rendering doesn't support mid-frame OAM rotation
2. Document scroll register latch sharing behavior
3. Document Mode 7 fixed-point range

#### Low Priority (Nice to Have)
1. Add open bus emulation for read-only registers
2. Cycle-accurate VRAM/CGRAM access timing
3. Implement mosaic effects ($2106)
4. Complete direct color mode implementation

## Testing Plan

### New Tests to Add
1. `test_mode7_zero_matrix` - Verify zero matrix produces (0,0) coordinates
2. `test_mode7_extreme_center` - Test maximum/minimum center point values
3. `test_mode7_overflow` - Test coordinate overflow with extreme matrix values
4. `test_cgram_read_latch_unchanged` - Verify reads don't toggle latch
5. `test_scroll_register_shared_latch` - Verify cross-register latch behavior
6. `test_vram_address_wraparound` - Test address wrapping at 32K words

### Edge Case Test ROM Ideas
1. Mode 7 stress test ROM - extreme transformations
2. Sprite overflow test ROM - edge cases for 32/34 limits
3. VRAM timing test ROM - HBlank access patterns

## Conclusion

The SNES emulator implementation is **highly accurate** for most hardware edge cases. The vast majority of quirks and special behaviors are correctly implemented. The main gaps are in testing rather than implementation - the code appears correct but lacks comprehensive edge case tests to verify behavior under extreme conditions.

The identified testing gaps should be addressed to ensure long-term correctness and prevent regressions.
