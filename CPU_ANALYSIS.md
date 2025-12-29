# CPU 8086 Instruction Implementation Analysis

**Date:** 2025-12-28  
**Analyzer:** GitHub Copilot AI Agent  
**Target:** `crates/core/src/cpu_8086.rs`  
**Reference:** `REFERENCE.md`  
**Goal:** Identify bugs affecting MS-DOS and Floppinux boot (FreeDOS works correctly)

---

## Executive Summary

This analysis identified **3 critical bugs** related to segment override handling in string instructions, which are highly likely to affect MS-DOS and Floppinux boot sequences. FreeDOS may work despite these bugs if it doesn't rely on segment overrides for REPNE CMPS or XLAT instructions.

---

## Critical Bugs (High Priority - Fix Immediately)

### BUG #1: REPNE CMPSB Missing Segment Override

**Location:** Line 1444 in `cpu_8086.rs`

**Current Code:**
```rust
// CMPSB (REPNE prefix)
0xA6 => {
    while self.cx != 0 {
        let src = self.read(self.ds, self.si);  // ❌ WRONG
        let dst = self.read(self.es, self.di);
        // ... rest of implementation
    }
}
```

**Should Be:**
```rust
// CMPSB (REPNE prefix)
0xA6 => {
    let src_seg = self.get_segment_with_override(self.ds);  // ✓ CORRECT
    while self.cx != 0 {
        let src = self.read(src_seg, self.si);
        let dst = self.read(self.es, self.di);
        // ... rest of implementation
    }
}
```

**Evidence:**
- REPE CMPSB (line 1290) correctly uses `self.get_segment_with_override(self.ds)`
- Non-REP CMPSB (line 6775) correctly uses `self.get_segment_with_override(self.ds)`
- Only REPNE CMPSB is broken

**Impact:**
- Segment override prefixes (ES:, CS:, SS:, DS:) are completely ignored
- Common pattern in DOS code: `ES: REPNE CMPSB` to compare strings in extra segment
- **Severity: HIGH** - String operations with overrides are fundamental to DOS

---

### BUG #2: REPNE CMPSW Missing Segment Override

**Location:** Line 1472 in `cpu_8086.rs`

**Current Code:**
```rust
// CMPSW (REPNE prefix)
0xA7 => {
    while self.cx != 0 {
        let src = self.read_u16(self.ds, self.si);  // ❌ WRONG
        let dst = self.read_u16(self.es, self.di);
        // ... rest of implementation
    }
}
```

**Should Be:**
```rust
// CMPSW (REPNE prefix)
0xA7 => {
    let src_seg = self.get_segment_with_override(self.ds);  // ✓ CORRECT
    while self.cx != 0 {
        let src = self.read_u16(src_seg, self.si);
        let dst = self.read_u16(self.es, self.di);
        // ... rest of implementation
    }
}
```

**Impact:**
- Same as BUG #1 but for word (16-bit) operations
- **Severity: HIGH**

---

### BUG #3: XLAT Missing Segment Override

**Location:** Line 6019 in `cpu_8086.rs`

**Current Code:**
```rust
// XLAT/XLATB (0xD7)
0xD7 => {
    let al = (self.ax & 0xFF) as u8;
    let offset = self.bx.wrapping_add(al as u16);
    let val = self.read(self.ds, offset);  // ❌ WRONG
    self.ax = (self.ax & 0xFF00) | (val as u16);
    self.cycles += 11;
    11
}
```

**Should Be:**
```rust
// XLAT/XLATB (0xD7)
0xD7 => {
    let al = (self.ax & 0xFF) as u8;
    let offset = self.bx.wrapping_add(al as u16);
    let seg = self.get_segment_with_override(self.ds);  // ✓ CORRECT
    let val = self.read(seg, offset);
    self.ax = (self.ax & 0xFF00) | (val as u16);
    self.cycles += 11;
    11
}
```

**Reference Documentation:**
- REFERENCE.md states: "XLAT - Table lookup: `AL = [DS:BX + unsigned AL]`"
- This confirms DS should be overrideable

**Impact:**
- XLAT with segment override prefix is ignored
- Common in translation tables stored in different segments
- **Severity: MEDIUM** - Less commonly used than CMPS, but still important

---

## Design Issues (Lower Priority)

### ISSUE #4: LEA Unnecessarily Consumes Segment Override

**Location:** Line 5163 in `cpu_8086.rs`

**Current Code:**
```rust
// LEA - Load Effective Address (0x8D)
0x8D => {
    let modrm = self.fetch_u8();
    let (modbits, reg, rm) = Self::decode_modrm(modrm);
    // LEA only works with memory operands (not register mode)
    if modbits != 0b11 {
        let (_, offset_ea, _) = self.calc_effective_address(modbits, rm);  // ❌ Consumes override
        self.set_reg16(reg, offset_ea);
    }
    self.cycles += 2;
    2
}
```

**Problem:**
- `calc_effective_address()` calls `get_segment_with_override()` at line 1034
- This **consumes** the segment override even though LEA doesn't access memory
- The segment value is discarded (only offset is used)

**Should Be:**
- `calc_effective_address()` should have a parameter to control whether to consume override
- OR have a separate `calc_effective_offset()` function for LEA

**Impact:**
- Segment override is consumed without being used
- May cause issues if programmer expects override to carry to next instruction
- **Severity: LOW** - Unusual to use segment override before LEA

**Why This Matters:**
- x86 Reference: "LEA - Calc effective address only. **Does not access memory.**"
- If it doesn't access memory, it shouldn't consume the segment override
- This is a subtle difference in behavior from real x86

---

## Minor Issues (Optional Fixes)

### ISSUE #5: Auxiliary Flag (AF) Not Set by Arithmetic

**Location:** Lines 264-265, multiple arithmetic operations

**Current Status:**
```rust
const FLAG_AF: u16 = 0x0010; // Auxiliary Carry Flag
#[allow(dead_code)]  // ❌ Marked as unused
```

**Problem:**
- AF is defined but not set by ADD, SUB, ADC, SBB, etc.
- Should be set when there's a carry from bit 3 to bit 4
- DAA/DAS/AAA/AAS instructions do read and write AF (lines 2167, 2200, 4455, 4478)

**Impact:**
- BCD operations via DAA/DAS still work (they maintain their own AF logic)
- AF state may be incorrect for non-BCD code
- **Severity: VERY LOW** - Rarely affects real-world code outside BCD operations

**Why Not Critical:**
- Most DOS code doesn't rely on AF except for BCD operations
- The BCD adjust instructions maintain AF correctly
- Only affects programs that directly check AF after arithmetic

---

## Verified Correct Implementations

The following were checked against REFERENCE.md and found to be correctly implemented:

### Flag Handling
✅ **INC/DEC do NOT affect Carry Flag (CF)**
- Reference: "**Fl:** **Does NOT affect Carry Flag (CF).** Crucial!"
- Implementation: Lines 6207-6223, 6250-6278, 6355-6383 correctly omit CF updates

✅ **AND/OR/XOR clear CF and OF**
- Reference: "**Fl:** Clears CF and OF. Updates ZF, SF, PF."
- Implementation: Lines 2074-2075, 2094-2095, 2234-2235, 4340-4341, etc.

✅ **NOT affects NO flags**
- Reference: "Affects **NO** flags."
- Implementation: Lines 5324-5335, 5475-5486 correctly do not update flags

### Instruction Features
✅ **PUSH immediate gated to 80186+**
- Reference: "**186+** allows `PUSH imm`."
- Implementation: Lines 6538-6548 (0x68), 6581-6592 (0x6A) check `supports_80186_instructions()`

✅ **IMUL 3-operand forms gated to 80186+**
- Reference: "**186+** adds 3-op `IMUL`."
- Implementation: Lines 6551-6578 (0x69), 6594-6619 (0x6B) check `supports_80186_instructions()`

✅ **Shift count masking**
- Reference: Implicit in 80186+ behavior
- Implementation: Lines 732-736, 850-854 correctly mask to 5 bits on 80186+, full 8 bits on 8086

### Exception Handling
✅ **DIV/IDIV trigger INT 0 on error**
- Reference: "**Trap #DE** if div by 0."
- Implementation: Lines 5393-5394, 5418-5419, 5548-5549, 5574-5575 all call `trigger_interrupt(0, true)`

✅ **Exception vs Software INT handling**
- CPU exceptions save IP of faulting instruction (line 472-476)
- Software INT saves IP of next instruction (line 476)
- Test verification at lines 11337-11461

### String Operations
✅ **SCAS uses ES:DI (no override)**
- Reference: String operations - destination is always ES:DI
- Implementation: Lines 1374, 1399 correctly use `self.es` directly

✅ **Non-REP CMPS uses segment override**
- Implementation: Lines 6775, 6801 correctly use `get_segment_with_override(self.ds)`

### Control Flow
✅ **LOOP/LOOPE/LOOPNE/JCXZ**
- Reference: "Decs (E)CX. Jumps if (E)CX!=0 (and Z-flag check for LOOPE/NE)."
- Implementation: Lines 6040-6093 correctly implement all variants

✅ **LEA does not access memory**
- Reference: "**Does not access memory.**"
- Implementation: Line 5163 correctly only calls `calc_effective_address()` for offset calculation
- (Though it has the segment override consumption issue noted above)

### Other
✅ **POP CS not implemented**
- Reference: "`POP CS` is illegal (except early 8088 bugs)."
- Implementation: 0x0F is two-byte opcode prefix (line 2434), not POP CS

✅ **ModR/M decoding**
- Lines 970-975 correctly decode mod (bits 7-6), reg (bits 5-3), r/m (bits 2-0)

✅ **Segment override prefixes**
- Lines 2156-2158 (ES:), 2188-2190 (CS:), 4446-4449 (SS:), 4469-4472 (DS:)
- Lines 6514-6517 (FS:), 6522-6525 (GS:) for 80386+
- All correctly set `segment_override` field

---

## Root Cause Analysis

### Why FreeDOS Works But MS-DOS/Floppinux Don't

**Hypothesis:**
1. FreeDOS may not use segment overrides with REPNE CMPS instructions
2. FreeDOS may not use XLAT with segment overrides
3. MS-DOS and Floppinux use more sophisticated memory management requiring segment overrides

**Testing Strategy:**
1. Fix BUG #1, #2, #3 (segment override issues)
2. Test MS-DOS and Floppinux boot
3. If still failing, enable detailed CPU logging and trace segment override usage

---

## Recommendations

### Immediate Action (Critical Bugs)
1. ✅ **Fix REPNE CMPSB segment override** (BUG #1)
2. ✅ **Fix REPNE CMPSW segment override** (BUG #2)  
3. ✅ **Fix XLAT segment override** (BUG #3)

### Short Term (Design Issues)
4. ⚠️ **Fix LEA segment override consumption** (ISSUE #4)
   - Create `calc_effective_offset()` that doesn't consume override
   - OR add `consume_override: bool` parameter to `calc_effective_address()`

### Long Term (Nice to Have)
5. ℹ️ **Implement AF setting in arithmetic operations** (ISSUE #5)
   - Add AF calculation to ADD, SUB, ADC, SBB
   - Low priority - unlikely to affect DOS boot

---

## Testing Checklist

After fixes:
- [ ] Compile: `cargo build --workspace`
- [ ] Run tests: `cargo test --workspace`  
- [ ] Test FreeDOS boot (should still work)
- [ ] Test MS-DOS boot (should now work)
- [ ] Test Floppinux boot (should now work)
- [ ] Enable CPU logging and verify segment override behavior
- [ ] Run full test suite

---

## Appendix: Code Locations Reference

### Critical Bug Locations
- REPNE CMPSB: Line 1442-1465 (inside 0xF2 REPNE prefix handler)
- REPNE CMPSW: Line 1470-1495 (inside 0xF2 REPNE prefix handler)
- XLAT: Line 6015-6023 (opcode 0xD7)

### Comparison: Correct Implementations
- REPE CMPSB: Line 1286-1336 (inside 0xF3 REPE prefix handler) ✓
- REPE CMPSW: Line 1341-1368 (inside 0xF3 REPE prefix handler) ✓
- Non-REP CMPSB: Line 6772-6795 (opcode 0xA6) ✓
- Non-REP CMPSW: Line 6798-6821 (opcode 0xA7) ✓

### Helper Function
- `get_segment_with_override()`: Line 378-388 (consumes and returns segment)

---

## References

1. REFERENCE.md - Unofficial x86 instruction reference for emulator development
2. Intel 80186/80188 User's Manual
3. Intel 8086 Family User's Manual
4. x86 Instruction Set Reference (multiple sources)

---

*This analysis was performed by comparing the implementation against the provided REFERENCE.md and internal knowledge of x86 architecture. While comprehensive, real hardware testing is recommended to validate fixes.*
