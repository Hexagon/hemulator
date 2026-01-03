# Game Boy Sprite Rendering Investigation

**Date**: January 3, 2026  
**Status**: ✅ **RESOLVED**  
**Affected System**: Game Boy (DMG/CGB)  
**Issue**: Sprites not rendering due to OAM DMA address masking bug

---

## Resolution

**Root Cause Found**: Incorrect bit masking in `write_oam()` and `read_oam()` functions.

The functions used `(addr & 0x9F)` to mask OAM addresses, but this is incorrect because:
- OAM is 160 bytes (0xA0), not a power of 2
- The mask 0x9F has bit 5 cleared, causing addresses like 0x20, 0x40, 0x60, 0x80 to incorrectly wrap back to 0x00
- This caused later DMA iterations to overwrite the first few OAM entries with zeros

**Example of the bug**:
```
addr=0x00: writes 0x80 to index 0 ✓
addr=0x20: writes 0x00 to index 0 ✗ (overwrites!)
addr=0x40: writes 0x00 to index 0 ✗ (overwrites!)
addr=0x60: writes 0x00 to index 0 ✗ (overwrites!)
addr=0x80: writes 0x00 to index 0 ✗ (overwrites!)
```

**Fix Applied**: 
- Removed bit masking (`& 0x9F`)
- Added bounds checking (`if addr >= 0xA0`)
- All 104 Game Boy tests now pass
- DMA is now fully functional

**Files Changed**:
- `crates/systems/gb/src/ppu.rs`: Fixed `read_oam()`, `write_oam()`, `read_oam_debug()`
- `crates/systems/gb/src/lib.rs`: Added unit tests, updated documentation

---

## Problem Statement

Game Boy Tetris displays background/window graphics correctly but all "active" sprites (falling tetrominos, next piece preview) are invisible. Background tiles that have landed work fine, but any sprite-based graphics fail to render.

---

## Investigation Summary

### What Works ✅
1. **Background/Window rendering** - Tiles, playfield, UI all display correctly
2. **ROM loading and execution** - Game runs, responds to input
3. **WRAM writes** - Sprite data IS being written to $C000-$C09F
4. **DMA trigger** - Register $FF46 receives correct value ($C0)
5. **DMA source data** - Memory at $C000 contains valid sprite data (Y=0x80, X=0x10, tile=0x58, etc.)

### Critical Bug Found 🔴

**DMA reads correct data but OAM remains empty!**

Debug output shows:
```
[DMA #180] Copying from $C000: 80 10 58 00    <- SOURCE HAS DATA
[DMA #180] After copy, OAM[0-3]: 00 00 00 00  <- DESTINATION STILL ZEROS
```

**The DMA loop executes** (confirmed in code at `crates/systems/gb/src/bus.rs:344-346`):
```rust
for i in 0..0xA0u16 {
    let byte = self.read(source_base + i);  // Reads 0x80, 0x10, 0x58...
    self.ppu.write_oam(i, byte);            // Called but OAM stays 0x00
}
```

**PPU write_oam() function** (`crates/systems/gb/src/ppu.rs:283`):
```rust
pub fn write_oam(&mut self, addr: u16, val: u8) {
    self.oam[(addr & 0x9F) as usize] = val;  // Should write but doesn't
}
```

### Evidence Timeline

1. **Early frames (DMA #1-134)**: $C000 contains zeros, DMA copies zeros → Expected
2. **DMA #135+**: $C000 contains sprite data (0x80, 0x10, 0x58...) → **DMA copies but OAM stays zero**
3. **Frame 180, 240**: OAM still all zeros despite repeated DMA with valid source data
4. **write_oam() debug logging**: Function not appearing in logs despite being called in DMA loop

---

## Root Cause Hypotheses

### 1. **Ownership/Borrowing Issue** (Most Likely)
- `self.ppu.write_oam()` may be writing to a different PPU instance
- `self.ppu` in DMA handler may not be the same PPU instance used for rendering
- Rust's borrowing rules may be creating a temporary copy

**Investigation Steps**:
- Add debug output INSIDE `write_oam()` to confirm it's actually executing
- Verify `self.ppu` address/identity in both DMA and render paths
- Check if PPU is passed by value anywhere (should be by reference)

### 2. **Memory Corruption/Overwrite**
- OAM being written correctly but then immediately cleared
- Another part of code resetting OAM to zeros
- Rendering happening before DMA completes

**Investigation Steps**:
- Add OAM write timestamps/counters
- Log ALL OAM writes, not just DMA
- Check reset() or clear() functions called between DMA and render

### 3. **Address Masking Bug**
- `addr & 0x9F` masking may be incorrect for certain values
- Off-by-one in OAM array indexing

**Investigation Steps**:
- Test with fixed addresses (write to OAM[0], OAM[1] directly)
- Verify OAM array size is 160 bytes
- Check if masking with 0x9F vs 0xFF matters

### 4. **Compiler Optimization Issue**
- Release mode optimization removing writes
- `self.oam` array access being optimized out
- Need `volatile` or atomic operations

**Investigation Steps**:
- Test in debug mode (`cargo build`)
- Add `#[inline(never)]` to `write_oam()`
- Check assembly output for the DMA loop

---

## Required Tests

### Unit Tests (Add to `crates/systems/gb/src/lib.rs`)

```rust
#[test]
fn test_oam_dma_basic() {
    let mut system = GameBoy::new();
    
    // Write sprite data to WRAM
    system.bus.write(0xC000, 0x80); // Y
    system.bus.write(0xC001, 0x10); // X
    system.bus.write(0xC002, 0x58); // Tile
    system.bus.write(0xC003, 0x00); // Flags
    
    // Trigger DMA
    system.bus.write(0xFF46, 0xC0);
    
    // Verify OAM was updated
    assert_eq!(system.bus.ppu.oam[0], 0x80, "OAM[0] should be 0x80");
    assert_eq!(system.bus.ppu.oam[1], 0x10, "OAM[1] should be 0x10");
    assert_eq!(system.bus.ppu.oam[2], 0x58, "OAM[2] should be 0x58");
    assert_eq!(system.bus.ppu.oam[3], 0x00, "OAM[3] should be 0x00");
}

#[test]
fn test_oam_direct_write() {
    let mut ppu = Ppu::new();
    
    ppu.write_oam(0, 0x80);
    ppu.write_oam(1, 0x10);
    
    assert_eq!(ppu.oam[0], 0x80);
    assert_eq!(ppu.oam[1], 0x10);
}

#[test]
fn test_oam_dma_full_copy() {
    let mut system = GameBoy::new();
    
    // Fill WRAM with test pattern
    for i in 0..160 {
        system.bus.write(0xC000 + i, (i as u8) ^ 0xAA);
    }
    
    // Trigger DMA
    system.bus.write(0xFF46, 0xC0);
    
    // Verify all 160 bytes copied
    for i in 0..160 {
        let expected = (i as u8) ^ 0xAA;
        assert_eq!(
            system.bus.ppu.oam[i as usize], 
            expected,
            "OAM[{}] mismatch", i
        );
    }
}
```

### Integration Tests

1. **Test with Tetris test ROM**: Create minimal test ROM that sets up known sprite data
2. **Test sprite rendering pipeline**: Verify sprite data flows through entire render path
3. **Test DMA timing**: Ensure DMA doesn't interfere with rendering

---

## Deep Dive Areas

### 1. Memory Architecture (`crates/systems/gb/src/bus.rs`)

**Current Structure**:
- Bus owns PPU: `pub ppu: Ppu`
- WRAM: 8KB array `wram: [u8; 0x2000]`
- OAM lives in PPU: `ppu.oam: [u8; 0xA0]`

**Questions**:
- Is PPU moved or cloned anywhere?
- Are there multiple Bus instances?
- Does `read()` during DMA create issues with borrowing?
- Should OAM live in Bus instead of PPU?

**Investigation**:
```rust
// Add to DMA handler
eprintln!("PPU address: {:p}", &self.ppu);
eprintln!("OAM address: {:p}", &self.ppu.oam as *const _);

// Add to render_frame
eprintln!("Rendering with PPU at: {:p}", self as *const _);
```

### 2. Sprite Priority System (`crates/systems/gb/src/ppu.rs:630-850`)

**Current Implementation**:
- BG priority flag (bit 7 of sprite flags)
- CGB vs DMG mode differences
- 10-sprite-per-scanline limit
- X-coordinate sorting

**Potential Issues**:
- Priority logic may be hiding all sprites
- BG color index checks might be wrong
- OBJ-to-BG priority inverted

**Test Cases**:
- Sprite with priority=0 (should appear above BG colors 1-3)
- Sprite with priority=1 (should appear behind BG colors 1-3)
- CGB mode with LCDC.0=0 (sprites always on top)
- Sprite over BG color 0 (transparent - sprite always visible)

### 3. Sprite Coordinate System

**Game Boy Specifics**:
- OAM Y coordinate: sprite_screen_y = oam_y - 16
- OAM X coordinate: sprite_screen_x = oam_x - 8
- Sprites with Y=0 or Y>=160 are off-screen

**Tetris Data**:
- Y=0x80 (128) → screen_y = 128 - 16 = 112 ✅ (visible)
- X=0x10 (16) → screen_x = 16 - 8 = 8 ✅ (visible)

**Verification Needed**:
- Ensure coordinate checks don't have off-by-one errors
- Verify wrapping arithmetic is correct
- Check if LCDC sprite enable bit is being checked

### 4. OAM Access Restrictions

**Real Hardware Behavior**:
- OAM is NOT accessible during PPU modes 2 (OAM scan) and 3 (pixel transfer)
- Reads return 0xFF, writes ignored during restricted modes
- Only accessible during VBlank (mode 1) and HBlank (mode 0)

**Current Implementation**:
- On-demand rendering (no continuous PPU mode tracking)
- OAM always accessible
- May need mode-based access control

**Consideration**:
- Our current architecture renders full frame on-demand
- Real GB renders scanline-by-scanline
- May need to add PPU mode state even with on-demand rendering

---

## Immediate Action Items ✅ COMPLETE

### Priority 1: Fix DMA Write
1. ✅ Add debug output inside `write_oam()` to confirm execution
2. ✅ Verify OAM array is actually being modified
3. ✅ Confirmed `self.ppu` is the correct instance (same pointer throughout)
4. ✅ Added unit test for direct OAM writes (`test_oam_direct_write`)
5. ✅ Added unit test for DMA operation (`test_oam_dma_basic`, `test_oam_dma_full_copy`)

**Result**: All tests pass! DMA is now fully functional.

### Priority 2: Verify Architecture  
1. ✅ Traced PPU ownership - single instance owned by GbBus
2. ✅ Confirmed no cloning/copying of PPU
3. ✅ Verified `&mut self` propagates correctly in DMA path
4. ❌ Not needed - issue was bit masking, not atomic access

**Result**: Architecture is sound. Bug was in address masking logic.

### Priority 3: Sprite Rendering Review
❌ Not needed - sprite rendering code was correct all along. Issue was that OAM never received data due to DMA bug.

---

## Test Results

Added three new unit tests:
1. `test_oam_direct_write` - Verifies PPU write_oam() works correctly
2. `test_oam_dma_basic` - Tests DMA with 4 bytes of sprite data
3. `test_oam_dma_full_copy` - Tests full 160-byte DMA transfer

All tests pass. Total: 104 Game Boy tests passing.

---

## Debug Commands (Historical)

### Check OAM State
```powershell
cargo run --profile release-quick -- ".\roms\gb\Tetris (World).gb" 2>&1 | 
    Select-String "OAM|DMA"
```

### Monitor Memory Writes
```powershell
cargo run --profile release-quick -- ".\roms\gb\Tetris (World).gb" 2>&1 | 
    Select-String "WRAM.*C00[0-9]"
```

### Run Tests
```powershell
cargo test --package emu_gb --lib test_oam
```

---

## Related Files

- **DMA Implementation**: `crates/systems/gb/src/bus.rs:341-347`
- **OAM Write Function**: `crates/systems/gb/src/ppu.rs:283-286`
- **Sprite Rendering**: `crates/systems/gb/src/ppu.rs:630-850`
- **Memory Map**: `crates/systems/gb/src/bus.rs:240-335`
- **Test Suite**: `crates/systems/gb/src/lib.rs` (tests section)

---

## Notes

- All 101 existing Game Boy tests pass
- Background rendering works perfectly
- Issue is SPECIFIC to sprite rendering
- Bug is NOT in sprite rendering logic itself (that's never reached because OAM is empty)
- Bug is in the DMA→OAM data path

**The smoking gun**: DMA reads valid data but OAM write fails silently.
