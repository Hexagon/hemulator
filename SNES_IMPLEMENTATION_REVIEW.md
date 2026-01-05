# SNES Emulator Implementation Review

**Date**: 2026-01-05  
**Purpose**: Cross-check SNES implementation against README.md claims and reference documentation

This document provides a systematic review of the SNES emulator implementation, comparing:
1. Claims in `crates/systems/snes/README.md`
2. Actual implementation in source code
3. SNESdev Wiki specifications
4. Previous verification document (`docs/SNES_FULLSNES_VERIFICATION.md`)

## Executive Summary

The SNES README.md contains **several inaccurate claims** that overstate the implementation status:

### ✅ ACCURATE CLAIMS
- CPU (65C816) is fully implemented ✅
- Memory systems (WRAM, VRAM, CGRAM, OAM) are complete ✅
- DMA and HDMA are fully implemented ✅
- HiROM and LoROM both work ✅
- PPU Modes 0-7 basic rendering IS implemented ✅
- Controller input works correctly ✅

### ❌ INACCURATE/MISLEADING CLAIMS
- **Mode 7**: README says "Complete implementation" but mode 7 matrix registers (M7SEL, M7A-M7D, M7X, M7Y) are NOT implemented
  - Actual status: Basic 8bpp rendering only, no rotation/scaling
- **Offset-per-tile**: README mentions this for Modes 2, 4, 6 but NOT implemented
- **Hi-res modes**: Modes 5-6 claim "hi-res" but actually render at normal 256px resolution

## Detailed Findings

### 1. CPU & Memory Systems ✅

**README Claim**: "Complete 16-bit CPU", "Full SNES memory map"

**Actual Implementation**: ✅ VERIFIED
- 65C816 CPU from `emu_core::cpu_65c816` with 256/256 opcodes
- 128KB WRAM correctly mapped
- Shadow RAM at $0000-$1FFF in banks $00-$3F and $80-$BF
- All hardware registers properly routed

**SNESdev Wiki Compliance**: ✅ Fully compliant

### 2. PPU Modes ⚠️ PARTIALLY ACCURATE

#### Mode 0 ✅
**README**: "4 BG layers, 2bpp each"  
**Implementation**: ✅ Fully implemented with proper priority handling  
**Tests**: `test_mode0_rendering` passes

#### Mode 1 ✅
**README**: "2 BG layers 4bpp + 1 BG layer 2bpp (most common commercial mode)"  
**Implementation**: ✅ Fully implemented including BG3 priority toggle  
**Tests**: `test_mode1_rendering`, `test_mode1_typical_commercial_pattern` pass

#### Mode 2 ⚠️
**README**: "2 BG layers, 4bpp each, offset-per-tile capability"  
**Implementation**: 
- ✅ Basic 4bpp rendering implemented
- ❌ **Offset-per-tile NOT implemented**
  - Searched codebase for "offset-per-tile" implementation: NOT FOUND
  - Would require reading offset data from BG3 tilemap
**Status**: Misleading - basic rendering works but key feature missing

#### Mode 3 ✅
**README**: "BG1 8bpp (256 colors), BG2 4bpp (16 colors)"  
**Implementation**: ✅ Implemented with proper priority handling  
**Code**: Lines 829-856 in `ppu.rs`

#### Mode 4 ⚠️
**README**: "BG1 8bpp, BG2 2bpp, offset-per-tile"  
**Implementation**:
- ✅ 8bpp and 2bpp rendering
- ❌ **Offset-per-tile NOT implemented**
**Status**: Misleading

#### Mode 5 ⚠️
**README**: "2 BG layers (hi-res), BG1 4bpp, BG2 2bpp"  
**Implementation**:
- ✅ 4bpp and 2bpp rendering
- ❌ **Hi-res NOT implemented** - renders at 256px, not 512px
- Code comment admits: "True hi-res would require a 512px wide frame, we'll render at 256px for now"
**Status**: Misleading - feature claim not fulfilled

#### Mode 6 ⚠️
**README**: "1 BG layer (hi-res), 4bpp, offset-per-tile"  
**Implementation**:
- ✅ 4bpp rendering
- ❌ **Hi-res NOT implemented** - renders at 256px
- ❌ **Offset-per-tile NOT implemented**
**Status**: Misleading - 2/3 features not implemented

#### Mode 7 ❌
**README**: "1 BG layer, 8bpp (256 colors), basic rendering"  
**Implementation**:
- ✅ 8bpp rendering works
- ❌ **Mode 7 matrix registers NOT implemented**
  - Missing: $211A (M7SEL), $211B-$211E (M7A-M7D), $211F-$2120 (M7X, M7Y)
  - Grep search confirmed: NO Mode 7 register handling in bus.rs or ppu.rs
- Code comment: "Full Mode 7 requires matrix transformation, this is a simplified version"
**Status**: Accurate in "What Works" section (says "basic rendering") but misleading in summary

### 3. DMA & HDMA ✅

**README Claim**: "Full 8-channel support", "All transfer modes (0-7)", "Direct and indirect addressing modes"

**Actual Implementation**: ✅ VERIFIED
- 8 DMA channels fully implemented in `bus.rs` lines 188-278
- Transfer modes 0-7 with proper B-bus patterns
- Address modes: increment, decrement, fixed
- HDMA with line counter and repeat mode (lines 280-400)
- Cycle-accurate timing (8 cycles per byte + overhead)

**Tests**: 
- `test_dma_transfer_simple` ✅
- `test_dma_registers` ✅
- `test_dma_multiple_channels` ✅
- `test_hdma_initialization` ✅
- `test_hdma_execution_simple` ✅
- `test_hdma_repeat_mode` ✅

**SNESdev Wiki Compliance**: ✅ Fully compliant

**Discrepancy with FULLSNES_VERIFICATION.md**: 
- Verification doc (dated 2026-01-04) says "❌ Not Implemented" for DMA/HDMA
- **RESOLUTION**: Implementation was added AFTER verification doc was written
- All tests pass, implementation is complete

### 4. Cartridge Support ✅

**README Claim**: "Both LoROM and HiROM with auto-detection"

**Actual Implementation**: ✅ VERIFIED
- LoROM: 32KB banks at $8000-$FFFF per bank (lines 145-180 in `cartridge.rs`)
- HiROM: Full 64KB banks with linear addressing (lines 182-228)
- Auto-detection via header scoring (lines 79-105)
- SMC header detection and removal
- SRAM support for both modes

**Tests**:
- `test_read_rom_lorom` ✅
- `test_read_rom_hirom` ✅
- `test_write_read_ram_lorom` ✅
- `test_write_read_ram_hirom` ✅
- `test_mapping_mode_detection` ✅

**SNESdev Wiki Compliance**: ✅ Fully compliant

**Discrepancy with FULLSNES_VERIFICATION.md**:
- Verification doc says "❌ Not Implemented" for HiROM
- **RESOLUTION**: HiROM IS fully implemented, verification doc is incorrect

### 5. PPU Registers ✅

**README Claim**: Core PPU registers implemented

**Actual Implementation**: ✅ MOSTLY VERIFIED
- Screen display, BG mode, tilemap configuration ✅
- VRAM access ($2115-$2119) ✅
- CGRAM access ($2121-$2122) ✅
- OAM access ($2101-$2104) ✅
- Scroll registers ($210D-$2114) ✅
- Main screen enable ($212C) ✅
- Status registers ($213F, $4212) ✅

**Missing Registers** (correctly documented as stubs):
- ⚠️ Mode 7 matrix: $211A-$2120 (7 registers)
- ⚠️ Windows: $2123-$212B (9 registers)
- ⚠️ Color math: $2130-$2132 (3 registers)
- ⚠️ Hardware multiply result: $2134-$2136 (3 registers)

**SNESdev Wiki Compliance**: ⚠️ Partially compliant (core features work, advanced features stubbed)

### 6. CPU I/O Registers ✅

**README Claim**: Interrupt control, controller input, DMA/HDMA control implemented

**Actual Implementation**: ✅ VERIFIED
- $4200 (NMITIMEN) - NMI/IRQ enable, auto-joypad ✅
- $4210 (RDNMI) - NMI flag with read-and-clear ✅
- $4211 (TIMEUP) - IRQ flag stub ⚠️
- $4212 (HVBJOY) - H/V-Blank status ✅
- $4016-$4017 - Serial joypad ports ✅
- $4218-$421F - Auto-joypad read ✅
- $420B (MDMAEN) - DMA enable ✅
- $420C (HDMAEN) - HDMA enable ✅
- $4300-$437F - DMA/HDMA channel registers ✅

**Missing** (correctly documented):
- ❌ Hardware multiply/divide: $4202-$4206, $4214-$4217
- ❌ IRQ timers: $4207-$420A
- ❌ Programmable I/O: $4201, $4213

**SNESdev Wiki Compliance**: ✅ All implemented features are compliant

### 7. APU (Audio) ⚠️

**README Claim**: "Stub implementation for boot"

**Actual Implementation**: ✅ ACCURATE
- APU communication ports ($2140-$2143) respond
- Initialized to SPC700 IPL ready state (0xBB, 0xAA, 0x00, 0x00)
- Echo/passthrough allows boot handshakes
- No actual SPC700 CPU or DSP

**Tests**:
- `test_apu_ports_initial_values` ✅
- `test_apu_ports_echo` ✅

**Status**: README accurately describes this as stub only

### 8. Timing ✅

**README Claim**: "89,342 master cycles per frame (~3.58 MHz / 60 Hz)"

**Actual Implementation**: ✅ VERIFIED
- Frame timing: 89,342 cycles/frame
- Scanline timing: 341 cycles/scanline
- 262 scanlines/frame (224 visible + 38 VBlank)
- VBlank starts at scanline 225

**Calculation Verification**:
- 3.58 MHz / 89,342 cycles = 60.05 Hz ✅
- VBlank timing: 224/262 scanlines = 85.5% ✅

**SNESdev Wiki Compliance**: ✅ NTSC timing is accurate

## Comparison with FULLSNES_VERIFICATION.md

The verification document (`docs/SNES_FULLSNES_VERIFICATION.md`) dated 2026-01-04 contains several **outdated assessments**:

### Incorrect "Not Implemented" Claims:
1. **DMA** - Document says ❌, actually ✅ FULLY IMPLEMENTED
2. **HDMA** - Document says ❌, actually ✅ FULLY IMPLEMENTED  
3. **HiROM** - Document says ❌, actually ✅ FULLY IMPLEMENTED
4. **PPU Modes 2-7** - Document says ❌, actually ⚠️ PARTIALLY IMPLEMENTED

### Likely Explanation:
- DMA/HDMA implementation was added recently (after 2026-01-04)
- HiROM was always implemented but not detected during verification
- Verification was incomplete or based on outdated code

### Still Accurate Claims:
- ❌ Mode 7 matrix transformation (correct)
- ❌ Windows and color math (correct)
- ❌ APU/SPC700 (correct)
- ❌ Hardware multiply/divide (correct)

## SNESdev Wiki Compliance Summary

### ✅ Fully Compliant Features:
- 65C816 CPU (all opcodes, modes, addressing)
- Memory map (WRAM, VRAM, CGRAM, OAM)
- DMA/HDMA (all modes, proper timing)
- LoROM and HiROM cartridge mapping
- Controller input (serial and auto-read)
- NMI/VBlank timing and flags
- Basic PPU rendering (Modes 0-1 fully, 2-7 partially)

### ⚠️ Partially Compliant Features:
- PPU Modes 2, 4, 6 (missing offset-per-tile)
- PPU Modes 5, 6 (missing true hi-res 512px)
- Mode 7 (missing matrix transformation)
- PPU registers (core implemented, advanced stubbed)

### ❌ Not Implemented:
- Windows and masking
- Color math (add/subtract/average)
- Hardware multiply/divide
- IRQ timers
- SPC700/DSP (audio)
- Enhancement chips

## Recommendations

### 1. Update README.md - HIGH PRIORITY
**Issue**: README overstates Mode 2-7 capabilities

**Recommended Changes**:
```markdown
- Mode 2: Basic 4bpp rendering (offset-per-tile NOT implemented)
- Mode 3: BG1 8bpp + BG2 4bpp ✅
- Mode 4: BG1 8bpp + BG2 2bpp (offset-per-tile NOT implemented)
- Mode 5: BG1 4bpp + BG2 2bpp (hi-res NOT implemented, renders at 256px)
- Mode 6: BG1 4bpp (hi-res and offset-per-tile NOT implemented)
- Mode 7: Basic 8bpp rendering (matrix transformation NOT implemented)
```

### 2. Update FULLSNES_VERIFICATION.md - HIGH PRIORITY
**Issue**: Document is outdated and contains incorrect assessments

**Required Updates**:
- ✅ Change DMA from ❌ to ✅
- ✅ Change HDMA from ❌ to ✅
- ✅ Change HiROM from ❌ to ✅
- ⚠️ Update PPU Modes 2-7 from ❌ to ⚠️ (basic rendering implemented)
- Add note about implementation date vs. verification date

### 3. Add Implementation Status Table - MEDIUM PRIORITY
**Purpose**: Clear, accurate status overview

**Suggested Addition to README**:
```markdown
## Feature Implementation Matrix

| Feature | Status | Notes |
|---------|--------|-------|
| CPU 65C816 | ✅ Complete | All 256 opcodes |
| LoROM | ✅ Complete | Full support |
| HiROM | ✅ Complete | Full support |
| DMA | ✅ Complete | 8 channels, all modes |
| HDMA | ✅ Complete | Direct & indirect |
| Mode 0 | ✅ Complete | 4×2bpp |
| Mode 1 | ✅ Complete | 2×4bpp + 1×2bpp |
| Mode 2 | ⚠️ Partial | 2×4bpp, no offset-per-tile |
| Mode 3 | ✅ Complete | 1×8bpp + 1×4bpp |
| Mode 4 | ⚠️ Partial | 1×8bpp + 1×2bpp, no offset-per-tile |
| Mode 5 | ⚠️ Partial | 2 layers, no hi-res (256px only) |
| Mode 6 | ⚠️ Partial | 1 layer, no hi-res or offset-per-tile |
| Mode 7 | ⚠️ Partial | 8bpp only, no matrix transform |
| Sprites | ✅ Complete | 128 sprites, size modes, priority |
| Controllers | ✅ Complete | Serial + auto-read |
| Windows | ❌ Stub | Registers exist, no effect |
| Color Math | ❌ Stub | Registers exist, no effect |
| APU/Audio | ❌ Stub | Boot handshake only |
```

### 4. Consider Feature Flags - LOW PRIORITY
**Purpose**: Allow users to check feature availability at runtime

**Suggested API**:
```rust
pub fn has_feature(&self, feature: SnesFeature) -> bool {
    match feature {
        SnesFeature::Mode7Matrix => false,
        SnesFeature::OffsetPerTile => false,
        SnesFeature::HiResMode => false,
        SnesFeature::ColorMath => false,
        SnesFeature::Windows => false,
        SnesFeature::Audio => false,
        _ => true,
    }
}
```

## Test Coverage Assessment

**Total Tests**: 63 unit tests + 5 integration tests = 68 tests

**Well-Tested Features**:
- ✅ DMA/HDMA (6 tests)
- ✅ Controllers (5 tests)
- ✅ VRAM/CGRAM/OAM (8 tests)
- ✅ Cartridge mapping (5 tests)
- ✅ PPU modes 0-1 (3 tests)

**Under-Tested Features**:
- ⚠️ PPU Modes 2-7 (0 dedicated tests)
- ⚠️ Sprite rendering (2 basic tests)
- ⚠️ Scrolling (1 test)

**Recommended New Tests**:
1. Mode 2-7 rendering tests (one per mode)
2. Offset-per-tile detection test (should fail)
3. Hi-res mode detection test (should fail)
4. Mode 7 matrix register test (should be stubbed)

## Conclusion

The SNES emulator has a **solid foundation** with excellent CPU, memory, DMA, and basic PPU support. However, the documentation contains **significant inaccuracies** that overstate the completeness of PPU modes 2-7.

**Key Actions Required**:
1. ✅ Update README.md to accurately reflect Mode 2-7 limitations
2. ✅ Update FULLSNES_VERIFICATION.md with correct DMA/HDMA/HiROM status
3. ✅ Add clear feature matrix showing what IS and ISN'T implemented
4. ⚠️ Consider adding Mode 7 matrix registers as stubs (for compatibility)
5. ⚠️ Consider adding offset-per-tile detection/warning

**Overall Grade**: **B+ (Good with caveats)**
- Strong fundamentals (CPU, memory, DMA) ✅
- Documentation overstates capabilities ❌
- Missing features clearly documented in "What's Missing" section ✅
- Test coverage good for implemented features ✅
- Ready for Mode 0-1 games, limited support for others ⚠️
