# DMA & HDMA Implementation Review

## Purpose

This document reviews the SNES DMA and HDMA implementation against the authoritative reference at https://wiki.superfamicom.org/dma-and-hdma

## Reference Documentation

Primary reference: **https://wiki.superfamicom.org/dma-and-hdma**

This reference has been added to the SNES README.md as the primary source for DMA/HDMA implementation.

## Issues Found and Fixed

### Issue 1: Mode 2 Transfer Pattern (FIXED)

**Problem**: Mode 2 was transferring 1 byte instead of 2 bytes.

**Reference Specification**: 
- Mode 2 (010): 2 bytes to 1 register (write twice)
- Pattern: b_addr, b_addr

**Original Implementation**:
```rust
let bytes_this_transfer = match transfer_mode {
    0 | 2 | 6 => 1, // WRONG: Mode 2 should be 2 bytes
    ...
};
```

**Fixed Implementation**:
```rust
let bytes_this_transfer = match transfer_mode {
    0 => 1,         // Mode 0: 1 byte to 1 register
    2 | 6 => 2,     // Mode 2/6: 2 bytes to 1 register (write twice)
    ...
};
```

**Impact**: Games using Mode 2 DMA (writing 16-bit values to 8-bit registers) would only transfer half the data.

### Issue 2: Mode 4 B-bus Address Pattern (FIXED)

**Problem**: Mode 4 was writing all 4 bytes to the same register instead of incrementing through 4 registers.

**Reference Specification**:
- Mode 4 (100): 4 bytes to 4 registers (write once)
- Pattern: b_addr, b_addr+1, b_addr+2, b_addr+3

**Original Implementation**:
```rust
let b_reg = match transfer_mode {
    0 | 4 => 0x2100 | (dma.b_addr as u16), // WRONG: Mode 4 should increment
    ...
};
```

**Fixed Implementation**:
```rust
let b_reg = match transfer_mode {
    0 => 0x2100 | (dma.b_addr as u16),
    4 => 0x2100 | ((dma.b_addr as u16) + (i as u16 & 3)), // Correct: increment through 4 registers
    ...
};
```

**Impact**: Games using Mode 4 DMA (for consecutive register updates) would write all data to a single register, likely causing visual glitches or incorrect behavior.

## Transfer Mode Summary

All transfer modes now correctly implement the wiki.superfamicom.org specification:

| Mode | Description | Bytes | Pattern | Fixed |
|------|-------------|-------|---------|-------|
| 0 | 1 register write once | 1 | b_addr | ✅ Already correct |
| 1 | 2 registers write once | 2 | b_addr, b_addr+1 | ✅ Already correct |
| 2 | 1 register write twice | 2 | b_addr, b_addr | ✅ Fixed |
| 3 | 2 registers write twice each | 4 | b_addr, b_addr, b_addr+1, b_addr+1 | ✅ Already correct |
| 4 | 4 registers write once | 4 | b_addr, b_addr+1, b_addr+2, b_addr+3 | ✅ Fixed |
| 5 | (mirror of mode 1) | 2 | b_addr, b_addr+1 | ✅ Already correct |
| 6 | (mirror of mode 2) | 2 | b_addr, b_addr | ✅ Fixed |
| 7 | (mirror of mode 3) | 4 | b_addr, b_addr, b_addr+1, b_addr+1 | ✅ Already correct |

## Implementation Verification

### DMA Implementation

The DMA implementation in `src/bus.rs::do_dma()` now correctly:

1. ✅ Implements all 8 transfer modes with correct byte counts and B-bus patterns
2. ✅ Handles direction bit (A→B or B→A)
3. ✅ Handles address increment modes (increment, fixed, decrement)
4. ✅ Implements 8 cycles per byte timing
5. ✅ Implements 8 cycles per channel overhead
6. ⚠️ Missing: 12-24 cycle whole-transfer overhead (minor timing inaccuracy)

### HDMA Implementation

The HDMA implementation in `src/bus.rs::init_hdma()` and `do_hdma()` now correctly:

1. ✅ Implements all 8 transfer modes (same as DMA)
2. ✅ Handles direct and indirect addressing modes (bit 6)
3. ✅ Loads line counter and repeat flag from table
4. ✅ Handles indirect address loading
5. ✅ Terminates on line count = 0
6. ✅ Implements 8 cycles per byte timing
7. ⚠️ Missing: ~18 cycle scanline overhead (minor timing inaccuracy)
8. ⚠️ Missing: 8 cycle per-channel overhead (minor timing inaccuracy)
9. ⚠️ Missing: 16 cycle indirect address load overhead (minor timing inaccuracy)

Note: The missing overhead cycles are documented but not critical for most games. They can be added in the future if cycle-accuracy becomes important.

## Test Coverage

New tests added to verify the fixes:

### DMA Tests
- `test_dma_mode_2_write_twice` - Verifies mode 2 transfers 2 bytes
- `test_dma_mode_4_four_registers` - Verifies mode 4 transfers to 4 registers

### HDMA Tests
- `test_hdma_mode_2` - Verifies HDMA mode 2 transfers 2 bytes
- `test_hdma_mode_4` - Verifies HDMA mode 4 transfers to 4 registers

All tests pass successfully.

## Remaining Work (Optional Improvements)

While the core DMA/HDMA functionality is now correct, the following timing improvements could be made:

1. **DMA Whole-Transfer Overhead**: Add 12-24 cycle overhead for entire DMA operation
2. **HDMA Scanline Overhead**: Add ~18 cycle overhead per scanline with active HDMA
3. **HDMA Channel Overhead**: Add 8 cycle overhead per active channel per scanline
4. **HDMA Indirect Load Overhead**: Add 16 cycle overhead when loading indirect addresses

These are timing refinements that would improve cycle accuracy but are not critical for game compatibility.

## Documentation Updates

1. ✅ Added wiki.superfamicom.org as primary DMA/HDMA reference in README.md
2. ✅ Updated all DMA/HDMA reference links to point to wiki.superfamicom.org
3. ✅ Added comprehensive documentation to `do_dma()` function
4. ✅ Added comprehensive documentation to `init_hdma()` function
5. ✅ Added comprehensive documentation to `do_hdma()` function
6. ✅ Documented all transfer modes with patterns and use cases

## Conclusion

The DMA and HDMA implementation has been thoroughly reviewed against the wiki.superfamicom.org reference. Two critical bugs in transfer modes 2 and 4 were found and fixed. The implementation now correctly handles all transfer modes and addressing patterns as specified in the reference documentation.

The remaining work items are optional timing refinements that would improve cycle accuracy but are not critical for game compatibility.
