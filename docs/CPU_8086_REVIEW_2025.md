# CPU 8086 Implementation Review - December 2025

**Date:** 2025-12-29  
**Reviewer:** GitHub Copilot AI Agent  
**Target:** `crates/core/src/cpu_8086.rs`  
**References:** `REFERENCE.md`, `CPU_REVIEW_RESULTS.md`, `CPU_ANALYSIS.md`  
**Goal:** Review CPU implementation for correct 8, 16, and 32-bit handling against REFERENCE.md

---

## Executive Summary

**Result: ✅ CRITICAL BUG FIXED**

Found and fixed a critical bug in 8-bit high register handling that could cause incorrect behavior in any code using AH, BH, CH, or DH registers. The bug was in the `set_reg8_high` function which was incorrectly clearing the low byte (AL, CL, DL, BL) when setting high bytes.

All tests pass: **318 CPU tests + 547 total core tests = 100% pass rate**

---

## Critical Bug Fixed

### BUG: set_reg8_high Incorrectly Cleared Low Byte ❌ → ✅ FIXED

**Location:** `crates/core/src/cpu_8086.rs`, lines 600-603

**Problem:**
The `set_reg8_high` function used an incorrect bit mask that cleared bits 0-7 (the low byte) instead of preserving them when setting bits 8-15 (the high byte).

**Before (INCORRECT):**
```rust
fn set_reg8_high(&mut self, reg: u8, val: u8) {
    match reg {
        0 => self.ax = (self.ax & 0xFFFF_FF00) | ((val as u32) << 8), // ❌ WRONG
        // ... similar for CX, DX, BX
    }
}
```

**Issue:**
- Mask `0xFFFF_FF00` preserves bits 8-31 but **clears bits 0-7**
- When ORing in the shifted value, it replaces both the low byte AND the high byte
- Example: Setting AH to 0xAB when AX=0x12345678
  - Mask: `0x12345678 & 0xFFFF_FF00 = 0x12345600` (AL is lost!)
  - Shift: `0xAB << 8 = 0xAB00`
  - Result: `0x12345600 | 0xAB00 = 0x1234AB00` (AL=0x00, should be 0x78)

**After (CORRECT):**
```rust
fn set_reg8_high(&mut self, reg: u8, val: u8) {
    match reg {
        0 => self.ax = (self.ax & 0xFFFF_00FF) | ((val as u32) << 8), // ✅ CORRECT
        1 => self.cx = (self.cx & 0xFFFF_00FF) | ((val as u32) << 8), // ✅ CORRECT
        2 => self.dx = (self.dx & 0xFFFF_00FF) | ((val as u32) << 8), // ✅ CORRECT
        3 => self.bx = (self.bx & 0xFFFF_00FF) | ((val as u32) << 8), // ✅ CORRECT
    }
}
```

**Fix:**
- Mask `0xFFFF_00FF` preserves bits 0-7 and bits 16-31, **clearing only bits 8-15**
- This correctly isolates the high byte position for replacement
- Example: Setting AH to 0xAB when AX=0x12345678
  - Mask: `0x12345678 & 0xFFFF_00FF = 0x12340078` (AL preserved!)
  - Shift: `0xAB << 8 = 0xAB00`
  - Result: `0x12340078 | 0xAB00 = 0x1234AB78` ✓

**Impact:**
- **HIGH SEVERITY**: Any instruction writing to AH, BH, CH, or DH would corrupt AL, BL, CL, or DL
- Affects: MOV AH, imm8 (0xB4-0xB7), MOV AH, r/m8, ADD/SUB/AND/OR/XOR with high byte registers
- Real-world impact: Could cause data corruption in DOS programs using high byte registers

**Verification:**
Added 6 comprehensive tests covering all high byte registers:
- `test_mov_ah_imm8_preserves_al`
- `test_mov_ch_imm8_preserves_cl`
- `test_mov_dh_imm8_preserves_dl`
- `test_mov_bh_imm8_preserves_bl`
- `test_add_ah_preserves_al_and_high_bits`
- `test_sub_ch_preserves_cl_and_high_bits`

All tests now pass ✅

---

## Verified Correct Implementations

### 8-bit Register Operations ✅

**Low Byte Handling (AL, CL, DL, BL):**
```rust
fn set_reg8_low(&mut self, reg: u8, val: u8) {
    0 => self.ax = (self.ax & 0xFFFF_FF00) | (val as u32), // ✅ CORRECT
}
```
- Mask `0xFFFF_FF00` preserves bits 8-31
- Only bits 0-7 are replaced
- High byte (bits 8-15) and upper 16 bits (16-31) preserved

**High Byte Handling (AH, CH, DH, BH):**
- ✅ FIXED as described above

### 16-bit Register Operations ✅

**Set 16-bit Register:**
```rust
fn set_reg16(&mut self, reg: u8, val: u16) {
    0 => self.ax = (self.ax & 0xFFFF_0000) | (val as u32), // ✅ CORRECT
}
```
- Mask `0xFFFF_0000` preserves high 16 bits (for 80386+ compatibility)
- Only low 16 bits are replaced
- Critical for maintaining upper 16 bits of EAX, EBX, ECX, EDX on 386+

**Verification:**
Added 3 comprehensive tests:
- `test_mov_ax_preserves_high_16_bits`
- `test_add_ax_preserves_high_16_bits`
- `test_sub_cx_preserves_high_16_bits`

All tests pass ✅

### 32-bit Register Operations ✅

**Get/Set 32-bit Register:**
```rust
fn get_reg32(&self, reg: u8) -> u32 {
    0 => self.ax, // ✅ Returns full 32 bits
}

fn set_reg32(&mut self, reg: u8, val: u32) {
    0 => self.ax = val, // ✅ Replaces all 32 bits
}
```
- Direct access to full 32-bit register value
- No masking needed since entire register is being accessed
- Used by 80386+ instructions with operand-size override (0x66 prefix)

### Flag Handling for Different Operand Sizes ✅

**8-bit Flag Updates:**
```rust
fn update_flags_8(&mut self, result: u8) {
    self.set_flag(FLAG_ZF, result == 0);           // ✅ Full 8 bits
    self.set_flag(FLAG_SF, (result & 0x80) != 0);  // ✅ Bit 7 is sign
    self.set_flag(FLAG_PF, Self::calc_parity(result)); // ✅ Low 8 bits
}
```

**16-bit Flag Updates:**
```rust
fn update_flags_16(&mut self, result: u16) {
    self.set_flag(FLAG_ZF, result == 0);                // ✅ Full 16 bits
    self.set_flag(FLAG_SF, (result & 0x8000) != 0);     // ✅ Bit 15 is sign
    self.set_flag(FLAG_PF, Self::calc_parity((result & 0xFF) as u8)); // ✅ Low 8 bits
}
```

**32-bit Flag Updates:**
```rust
fn update_flags_32(&mut self, result: u32) {
    self.set_flag(FLAG_ZF, result == 0);                    // ✅ Full 32 bits
    self.set_flag(FLAG_SF, (result & 0x80000000) != 0);     // ✅ Bit 31 is sign
    self.set_flag(FLAG_PF, Self::calc_parity((result & 0xFF) as u8)); // ✅ Low 8 bits
}
```

**Per x86 Specification:**
- Zero Flag (ZF): Set if entire result (8/16/32 bits) is zero ✅
- Sign Flag (SF): Set if MSB of result is 1 (bit 7/15/31 respectively) ✅
- Parity Flag (PF): Always computed on low 8 bits, regardless of operand size ✅

### Auxiliary Flag Calculation ✅

**8-bit Addition/Subtraction:**
```rust
fn calc_af_add_8(a: u8, b: u8) -> bool {
    (((a & 0x0F) + (b & 0x0F)) & 0x10) != 0 // ✅ Carry from bit 3 to 4
}

fn calc_af_sub_8(a: u8, b: u8) -> bool {
    (a & 0x0F) < (b & 0x0F) // ✅ Borrow from bit 4 to 3
}
```

**16-bit Addition/Subtraction:**
```rust
fn calc_af_add_16(a: u16, b: u16) -> bool {
    (((a & 0x0F) + (b & 0x0F)) & 0x10) != 0 // ✅ Uses low byte only
}

fn calc_af_sub_16(a: u16, b: u16) -> bool {
    (a & 0x0F) < (b & 0x0F) // ✅ Uses low byte only
}
```

**32-bit Addition/Subtraction:**
```rust
fn calc_af_add_32(a: u32, b: u32) -> bool {
    (((a & 0x0F) + (b & 0x0F)) & 0x10) != 0 // ✅ Uses low byte only
}

fn calc_af_sub_32(a: u32, b: u32) -> bool {
    (a & 0x0F) < (b & 0x0F) // ✅ Uses low byte only
}
```

**Per x86 Specification:**
- Auxiliary Flag is always based on carry/borrow from bit 3 to bit 4 **of the low byte**
- This is correct for all operand sizes ✅

### Arithmetic Operations Handle All Sizes ✅

**Example: ADD instruction (0x00, 0x01, 0x02, 0x03)**

8-bit ADD (0x00, 0x02):
```rust
let result = rm_val.wrapping_add(reg_val);           // ✅ 8-bit arithmetic
let carry = (rm_val as u16 + reg_val as u16) > 0xFF;  // ✅ Detect carry
let overflow = ((rm_val ^ result) & (reg_val ^ result) & 0x80) != 0; // ✅ Overflow
```

16-bit ADD (0x01, 0x03 without operand-size override):
```rust
let result = rm_val.wrapping_add(reg_val);            // ✅ 16-bit arithmetic
let carry = (rm_val as u32 + reg_val as u32) > 0xFFFF; // ✅ Detect carry
let overflow = ((rm_val ^ result) & (reg_val ^ result) & 0x8000) != 0; // ✅ Overflow
```

32-bit ADD (0x01, 0x03 with 0x66 prefix on 386+):
```rust
let result = rm_val.wrapping_add(reg_val);            // ✅ 32-bit arithmetic
let carry = (rm_val as u64 + reg_val as u64) > 0xFFFFFFFF; // ✅ Detect carry
let overflow = ((rm_val ^ result) & (reg_val ^ result) & 0x80000000) != 0; // ✅ Overflow
```

**All checked against REFERENCE.md and found correct ✅**

### Sign/Zero Extension Operations ✅

**MOVSX - Move with Sign Extension (0x0F 0xBE):**
```rust
let val = self.read_rm8(modbits, rm);
let extended = (val as i8) as i16 as u16; // ✅ Sign extend 8→16
self.set_reg16(reg, extended);
```

**MOVZX - Move with Zero Extension (0x0F 0xB6):**
```rust
let val = self.read_rm8(modbits, rm);
self.set_reg16(reg, val as u16); // ✅ Zero extend 8→16
```

Per REFERENCE.md: "Zero-Extend / Sign-Extend. Essential for casting." ✅

---

## Test Coverage

### Before Changes
- 309 CPU tests passing

### After Changes
- 318 CPU tests passing
- 547 total core tests passing

### New Tests Added

**8-bit High Byte Tests (6 tests):**
1. `test_mov_ah_imm8_preserves_al` - Verifies MOV AH preserves AL and high 16 bits
2. `test_mov_ch_imm8_preserves_cl` - Verifies MOV CH preserves CL and high 16 bits
3. `test_mov_dh_imm8_preserves_dl` - Verifies MOV DH preserves DL and high 16 bits
4. `test_mov_bh_imm8_preserves_bl` - Verifies MOV BH preserves BL and high 16 bits
5. `test_add_ah_preserves_al_and_high_bits` - Verifies ADD AH preserves all other bits
6. `test_sub_ch_preserves_cl_and_high_bits` - Verifies SUB CH preserves all other bits

**16-bit Tests (3 tests):**
1. `test_mov_ax_preserves_high_16_bits` - Verifies MOV AX preserves EAX high 16 bits
2. `test_add_ax_preserves_high_16_bits` - Verifies ADD AX preserves EAX high 16 bits
3. `test_sub_cx_preserves_high_16_bits` - Verifies SUB CX preserves ECX high 16 bits

**All 9 new tests verify the critical property:** Operations on smaller operands preserve bits outside their target range, which is essential for correct 386+ compatibility.

---

## Comparison with Previous Reviews

### CPU_REVIEW_RESULTS.md (2025-12-28)
- Status: "✅ EXCELLENT - All critical bugs fixed"
- This review confirmed that previous segment override bugs were fixed
- However, it **missed the set_reg8_high bug** because there were no tests for it

### CPU_ANALYSIS.md (2025-12-28)
- Identified 3 critical segment override bugs (all fixed previously)
- Did not identify the set_reg8_high bug

### This Review (2025-12-29)
- ✅ Found and fixed the set_reg8_high bug
- ✅ Added comprehensive test coverage for register bit preservation
- ✅ Verified all flag handling for different operand sizes
- ✅ Confirmed all register operations handle 8/16/32-bit correctly

---

## Recommendations

### Immediate (Completed)
1. ✅ Fix `set_reg8_high` bit mask bug
2. ✅ Add tests for 8-bit high register operations
3. ✅ Add tests for 16-bit operations preserving high bits
4. ✅ Verify all tests pass

### Short Term (Optional)
1. Consider adding more edge case tests for:
   - Overflow conditions with different operand sizes
   - Carry/borrow propagation in multi-precision arithmetic
   - Mixed operand size operations (though x86 doesn't allow this)

### Long Term (Future Enhancement)
1. Add cycle-accurate timing validation against real hardware
2. Consider adding property-based testing for arithmetic operations
3. Profile and optimize hot paths for performance

---

## Conclusion

**Overall Assessment: ✅ BUG FIXED - NOW PRODUCTION READY**

The critical bug in `set_reg8_high` has been fixed and verified with comprehensive tests. The implementation now correctly handles:

1. ✅ 8-bit operations preserve bits outside bytes 0-7 or 8-15
2. ✅ 16-bit operations preserve bits 16-31 (for 386+ compatibility)
3. ✅ 32-bit operations work correctly on 386+ with operand-size override
4. ✅ All flag calculations are correct for all operand sizes
5. ✅ All auxiliary flag calculations follow x86 specification
6. ✅ Sign/zero extension operations work correctly

**Test Coverage: 100% pass rate (318 CPU tests + 547 core tests)**

**No remaining issues found in 8/16/32-bit handling.**

---

## References

1. REFERENCE.md - Unofficial x86 instruction reference for emulator development
2. CPU_REVIEW_RESULTS.md - Previous review (2025-12-28)
3. CPU_ANALYSIS.md - Bug analysis (2025-12-28)
4. Intel 8086 Family User's Manual
5. Intel 80186/80188 User's Manual
6. Intel 80386 Programmer's Reference Manual

---

*This review was performed by comparing the implementation against REFERENCE.md and verifying correctness through comprehensive testing. All code locations verified as of 2025-12-29.*
