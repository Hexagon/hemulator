# SNES Register Implementation Fixes (SNESdev Wiki Review)

**Date**: January 4, 2026  
**Status**: Critical fixes implemented

## Problem

Games showing black screens despite working PPU implementation. Investigation revealed missing critical registers required for proper NMI handling according to SNESdev Wiki specifications.

## Critical Missing Register: $4210 (RDNMI)

### SNESdev Wiki Specification
- **Address**: $4210
- **Name**: RDNMI (Read NMI Flag)
- **Function**: 
  - Bit 7: NMI flag (1 = NMI occurred during VBlank)
  - Bits 0-3: CPU version number
  - **Critical**: Reading this register clears the NMI flag

### Why This Caused Black Screens

1. Games wait for VBlank by executing WAI instruction
2. VBlank occurs, NMI flag is set, NMI triggers
3. NMI handler runs and completes
4. **Without $4210**: NMI flag stays set forever
5. Next VBlank: NMI tries to trigger but flag still set → NMI doesn't trigger
6. Game hangs waiting for next NMI
7. Graphics initialization never completes → black screen

### Implementation Details

Added $4210 register with proper behavior:
- Returns NMI flag in bit 7
- Returns CPU version 2 in bits 0-3 (for 65C816)
- **Clears NMI flag on read** (critical for proper operation)
- Uses Cell<bool> for interior mutability (read is &self but must mutate flag)
- Added trace-level logging for debugging

## Additional Fixes

### 1. Added $4211 (TIMEUP) - IRQ Flag
- Stub implementation (returns 0)
- Not used by most games but required for completeness
- Will be needed if IRQ/timer functionality is added later

### 2. Fixed $4212 (HVBJOY) Duplicate
- **Bug Found**: $4212 was implemented twice with conflicting code
- First instance at line 465 had wrong implementation (controller code)
- Second instance at line 537 had correct implementation (HVBJOY)
- **Fix**: Removed duplicate, kept correct HVBJOY implementation
- Moved to proper location after $4211

### 3. Added Missing $420C Read Support
- HDMA Enable register was write-only
- Added read support returning current hdma_enable value
- Fixed failing test_hdma_enable_register

### 4. NMI Flag Interior Mutability
- Changed `nmi_flag: bool` to `nmi_flag: Cell<bool>`
- Allows clearing flag in immutable read method
- Critical for $4210 and $213F register behavior
- Updated all references to use `.get()` and `.set()`

## Code Changes Summary

### crates/systems/snes/src/bus.rs
- ✅ Added $4210 (RDNMI) register with NMI flag read-and-clear
- ✅ Added $4211 (TIMEUP) register stub
- ✅ Fixed $4212 (HVBJOY) duplicate issue
- ✅ Added $420C read support for HDMA enable
- ✅ Added trace logging for NMI flag operations

### crates/systems/snes/src/ppu.rs
- ✅ Changed nmi_flag to Cell<bool> for interior mutability
- ✅ Updated $213F to clear NMI flag on read (was commented as TODO)
- ✅ Updated all nmi_flag references to use Cell methods
- ✅ Updated comments to reflect $4210 also clears flag

## Testing

- ✅ All 61 SNES unit tests pass
- ✅ test_hdma_enable_register now passes
- ✅ Pre-commit checks pass (fmt, clippy, build)

## Expected Impact

With proper $4210 implementation:
1. Games can properly acknowledge NMI by reading $4210
2. NMI flag clears after each VBlank, allowing next NMI
3. WAI instruction will properly release on each VBlank
4. Games can proceed past initialization
5. Graphics setup should complete
6. Black screen issue should be resolved

## References

- SNESdev Wiki: https://snes.nesdev.org/wiki/
- $4210 specification: https://snes.nesdev.org/wiki/RDNMI
- $4212 specification: https://snes.nesdev.org/wiki/HVBJOY
- NMI handling: https://snes.nesdev.org/wiki/NMI

## Next Steps

1. Test with Super Mario World to verify black screen is fixed
2. Test with Pokemon Y and Tetris DX (user-reported issues)
3. Monitor for proper NMI handling with `--log-interrupts trace`
4. Verify graphics initialization completes with `--log-ppu info`
