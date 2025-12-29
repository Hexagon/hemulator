# 32-bit Addressing Implementation Plan for 80386+ Support

## Overview

This document outlines the phased, backward-compatible approach to implementing full 32-bit addressing and operand support for 80386+ CPUs in the hemulator PC emulator.

## Status: Planning Phase

**Current State:** 
- ✅ CPU supports 8086 through Pentium MMX instruction sets
- ✅ 16-bit registers (AX, BX, CX, DX, SI, DI, BP, SP)
- ✅ Segment registers (CS, DS, ES, SS, FS, GS)
- ✅ Prefix support: segment override, operand-size (0x66), address-size (0x67)
- ❌ Prefixes set flags but 32-bit modes not fully implemented

**Target State:**
- Full 80386+ 32-bit addressing with SIB byte support
- 32-bit register extensions (EAX, EBX, ECX, EDX, ESI, EDI, EBP, ESP)
- Backward compatible with all existing 8086/80186/80286 code

## Phased Implementation

### Phase 1: Register Architecture Extension ⏳

**Goal:** Extend CPU registers to support 32-bit operations while maintaining backward compatibility.

**Changes:**
- [ ] Extend general-purpose registers from `u16` to `u32`
  - `ax` → `eax` (with `ax` as low 16 bits)
  - `bx` → `ebx` (with `bx` as low 16 bits)
  - `cx` → `ecx` (with `cx` as low 16 bits)
  - `dx` → `edx` (with `dx` as low 16 bits)
- [ ] Extend index/pointer registers
  - `si` → `esi` (with `si` as low 16 bits)
  - `di` → `edi` (with `di` as low 16 bits)
  - `bp` → `ebp` (with `bp` as low 16 bits)
  - `sp` → `esp` (with `sp` as low 16 bits)
- [ ] Extend instruction pointer: `ip` → `eip`
- [ ] Extend flags register: `flags` → `eflags` (32-bit)
- [ ] Add helper methods:
  - `get_reg32(reg)` / `set_reg32(reg, val)` for 32-bit access
  - Keep existing `get_reg16(reg)` / `set_reg16(reg, val)` for 16-bit access
  - Keep existing `get_reg8_low/high()` for 8-bit access
- [ ] Update CPU initialization and reset logic
- [ ] Verify all existing tests still pass

**Backward Compatibility:**
- All existing 16-bit operations continue to work on low 16 bits
- High 16 bits of registers default to zero for 8086/80186/80286 modes
- No changes to existing instruction implementations in Phase 1

**Testing:**
- [ ] All 224 existing CPU tests must pass
- [ ] Add tests for 32-bit register access
- [ ] Verify 8086 mode still works correctly

**Estimated Impact:** ~200 lines changed in `cpu_8086.rs`

---

### Phase 2: 32-bit Addressing Mode Infrastructure ⏳

**Goal:** Implement core infrastructure for 32-bit addressing modes and SIB byte decoding.

**Changes:**
- [ ] Implement SIB (Scale-Index-Base) byte decoder
  - `decode_sib(sib_byte) -> (scale, index, base)`
  - Scale values: 1, 2, 4, 8
- [ ] Create `calc_effective_address_32()` function
  - Support all 32-bit addressing modes
  - Handle SIB byte when mod != 11 and r/m == 100
  - Support displacement32 (disp32)
  - Handle special cases (e.g., [EBP] requires disp8/disp32)
- [ ] Extend `calc_effective_address()` to check `address_size_override`
  - If set on 386+: use 32-bit addressing
  - If not set: use existing 16-bit addressing
- [ ] Implement address size calculation helpers
  - `effective_address_size()` -> returns 16 or 32 based on mode and override
- [ ] Update ModR/M decoding to handle 32-bit modes

**Backward Compatibility:**
- Existing 16-bit addressing unchanged when `address_size_override` is false
- 32-bit addressing only active when prefix 0x67 is present on 386+ CPU

**Testing:**
- [ ] Add tests for SIB byte decoding
- [ ] Add tests for 32-bit effective address calculation
- [ ] Test all addressing modes: [EAX], [EAX+disp8], [EAX+disp32], [EAX+EBX*4], etc.
- [ ] Verify backward compatibility with 16-bit addressing

**Estimated Impact:** ~300 lines added to `cpu_8086.rs`

---

### Phase 3: 32-bit Operand Support ⏳

**Goal:** Enable 32-bit operands for arithmetic and data movement instructions.

**Changes:**
- [ ] Implement 32-bit register access in ModR/M operations
  - `read_rm32()` / `write_rm32()` for 32-bit register/memory operands
  - `read_rmw32()` / `write_rmw32()` for read-modify-write operations
- [ ] Update core instruction groups to support 32-bit operands:
  - [ ] MOV (all variants)
  - [ ] ADD, SUB, ADC, SBB (all variants)
  - [ ] AND, OR, XOR, TEST (all variants)
  - [ ] INC, DEC
  - [ ] PUSH, POP
  - [ ] CMP
- [ ] Implement operand-size override (0x66) handling
  - In 16-bit mode: 0x66 makes operations 32-bit
  - In 32-bit mode (for future): 0x66 makes operations 16-bit
- [ ] Add 32-bit flag update functions
  - `update_flags_32(result)` for ZF, SF, PF
  - Update overflow/carry calculations for 32-bit
- [ ] Update immediate fetch functions
  - `fetch_u32()` for 32-bit immediates

**Backward Compatibility:**
- 32-bit operations only active when operand_size_override is set on 386+
- All existing 16-bit operations unchanged

**Testing:**
- [ ] Test 32-bit MOV operations
- [ ] Test 32-bit arithmetic (ADD, SUB, etc.)
- [ ] Test 32-bit logical operations
- [ ] Test flag behavior with 32-bit operands
- [ ] Verify all existing tests pass

**Estimated Impact:** ~500 lines changed/added to `cpu_8086.rs`

---

### Phase 4: Extended Instruction Set ⏳

**Goal:** Implement remaining 386+ instructions with 32-bit support.

**Changes:**
- [ ] Shift/rotate instructions (SHL, SHR, SAL, SAR, ROL, ROR, RCL, RCR)
- [ ] Multiply/divide (IMUL, MUL, IDIV, DIV) 32-bit variants
- [ ] String operations (MOVS, STOS, LODS, SCAS, CMPS) 32-bit
- [ ] Bit manipulation (BT, BTC, BTR, BTS, BSF, BSR)
- [ ] Conditional moves (CMOVcc) - Pentium Pro+
- [ ] LOOP/JECXZ with ECX
- [ ] Far jumps/calls with 32-bit offsets
- [ ] ENTER/LEAVE with 32-bit stack operations

**Backward Compatibility:**
- All new instructions gated by CPU model checks
- Existing 16-bit variants unchanged

**Testing:**
- [ ] Add comprehensive tests for each instruction group
- [ ] Test edge cases (overflow, underflow, zero, etc.)
- [ ] Verify instruction timing/cycles where applicable

**Estimated Impact:** ~800 lines changed/added to `cpu_8086.rs`

---

### Phase 5: Complete Integration & Validation ⏳

**Goal:** Ensure all components work together correctly and validate against real software.

**Changes:**
- [ ] Add comprehensive integration tests
  - Mix of 16-bit and 32-bit operations
  - Mode switching scenarios
  - Prefix combination testing
- [ ] Create test ROMs for 386+ features
  - Assembly test programs in `test_roms/pc/386/`
  - Cover all addressing modes
  - Cover all new instructions
- [ ] Performance optimization
  - Profile critical paths
  - Optimize hot instruction implementations
- [ ] Documentation updates
  - Update MANUAL.md with 386+ support status
  - Update README.md
  - Document any known limitations

**Testing:**
- [ ] Run full test suite (all systems)
- [ ] Test with real 386+ DOS software if available
- [ ] Stress testing with complex addressing modes
- [ ] Performance benchmarking

**Estimated Impact:** ~200 lines added (tests and documentation)

---

## Implementation Guidelines

### Backward Compatibility Rules

1. **Never break existing functionality**
   - All 224 existing CPU tests must pass after every phase
   - Smoke tests for all systems must pass
   
2. **Feature gating**
   - Check `self.model.supports_80386_instructions()` before using 386+ features
   - Return appropriate error/invalid opcode for unsupported CPUs

3. **Default behavior**
   - Default to 16-bit mode for 8086/80186/80286
   - 32-bit features only activate with proper CPU model and prefixes

### Testing Strategy

1. **Unit tests** for each phase
   - Test new functions in isolation
   - Test edge cases

2. **Integration tests** after each phase
   - Ensure new code works with existing code
   - Test prefix combinations

3. **Regression tests**
   - Run full test suite after each commit
   - Ensure no existing tests break

4. **Validation tests**
   - Create assembly test programs
   - Validate against known behavior

### Code Quality

1. **Follow existing patterns**
   - Match style of existing instruction implementations
   - Use similar helper function patterns

2. **Documentation**
   - Comment complex addressing mode calculations
   - Document SIB byte handling
   - Add examples for tricky cases

3. **Performance**
   - Keep hot paths efficient
   - Minimize overhead in 16-bit mode
   - Use inline where appropriate

---

## Dependencies & Risks

### Dependencies
- None external - all changes are in `crates/core/src/cpu_8086.rs`

### Risks & Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Breaking existing tests | High | Run tests after each change; revert if failures |
| Performance regression | Medium | Profile before/after; optimize hot paths |
| Increased complexity | Medium | Phase changes; thorough documentation |
| Incomplete coverage | Medium | Comprehensive test suite; real software validation |

---

## Timeline Estimate

Based on ~1900 lines of changes across all phases:

- **Phase 1:** 2-3 days (register extension)
- **Phase 2:** 3-4 days (addressing infrastructure)
- **Phase 3:** 5-6 days (operand support)
- **Phase 4:** 6-8 days (extended instructions)
- **Phase 5:** 3-4 days (integration & validation)

**Total:** ~3-4 weeks of focused development

---

## Success Criteria

- ✅ All existing 224 CPU tests pass
- ✅ All new 32-bit addressing modes work correctly
- ✅ All new 32-bit instructions implemented
- ✅ Backward compatible with 8086/80186/80286 code
- ✅ Performance within 5% of current implementation for 16-bit code
- ✅ Validated with real 386+ software (if available)
- ✅ Comprehensive test coverage (>90% for new code)
- ✅ Documentation complete and accurate

---

## Notes

- This plan focuses on 32-bit addressing and operands for 386+
- Protected mode extensions are out of scope for this plan
- Paging is out of scope for this plan
- Focus is on real mode 32-bit operations
- Can be extended later for full protected mode support

---

**Document Version:** 1.0  
**Created:** 2025-12-28  
**Status:** Planning - Awaiting approval to begin Phase 1
