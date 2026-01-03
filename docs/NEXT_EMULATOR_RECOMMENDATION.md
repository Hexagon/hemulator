# Next Emulator Implementation Recommendation

**Date**: January 2026  
**Status**: Proposal  
**Recommended System**: **Sega Master System / Game Gear**

## Executive Summary

After comprehensive analysis of well-documented retro gaming systems and evaluation of Hemulator's existing architecture, **Sega Master System (SMS) and Game Gear** are recommended as the next emulator implementations. These systems offer:

1. **Optimal difficulty level**: Simpler than modern systems (GBA, SNES) but more feature-complete than already-implemented 8-bit systems
2. **Excellent documentation**: Extensive technical references available from Sega, SMS Power!, and the emulation community
3. **Code reuse**: Can leverage existing Z80 CPU core and audio components
4. **Dual implementation**: Master System and Game Gear share ~95% of their architecture, providing two systems for minimal extra effort

## Current Implementation Status

### Completed Systems (Production Ready)
- ✅ **NES**: ~90% game coverage, 14 mappers, complete CPU/PPU/APU
- ✅ **Atari 2600**: Complete TIA, RIOT, cartridge banking
- ✅ **Game Boy**: ~95% game coverage, MBC0/1/2/3/5 support

### In-Development Systems
- 🚧 **SNES**: Basic infrastructure, minimal PPU, no APU/input
- 🚧 **N64**: 3D rendering functional, limited game support
- 🧪 **PC**: Experimental, CGA/EGA/VGA modes

## Why Sega Master System?

### 1. Well-Documented Hardware

The SMS has exceptional technical documentation:

- **Official Documentation**: [SEGA Mk3 Hardware Reference Manual (1986)](https://archive.org/details/SEGAMk3HardwareReferenceManual)
- **Community Resources**: 
  - [SMS Power! Development Documents](https://www.smspower.org/Development/Documents)
  - [Charles MacDonald's VDP Documentation](https://github.com/franckverrot/EmulationResources/blob/master/consoles/sms-gg/Sega%20Master%20System%20VDP%20documentation.txt)
  - [Copetti's Architecture Analysis](https://www.copetti.org/writings/consoles/master-system/)
- **Active Community**: Robust open-source emulators (Gearsystem, Kega Fusion) for reference

### 2. Appropriate Complexity

SMS strikes the perfect balance:

**Simpler than:**
- GBA (ARM CPU, complex PPU modes, DMA)
- SNES (65C816, Mode 7, DSP)
- PlayStation (MIPS + GPU + complex 3D pipeline)

**More feature-rich than:**
- Already-completed NES and Atari 2600
- Offers new challenges without overwhelming complexity

### 3. Excellent Code Reuse Opportunities

Hemulator's existing architecture is well-suited for SMS:

#### CPU: Z80 (Already Implemented)
- **Location**: `crates/core/src/cpu_z80.rs`
- **Current State**: Stub implementation with registers and state management
- **Required Work**: Complete instruction set implementation (~200-250 opcodes)
- **Reference**: Comprehensive Z80 documentation already exists in `docs/references/cpu_z80.md`

#### Audio: SN76489 PSG
- **Channels**: 3 square wave + 1 noise (similar to NES APU structure)
- **Reusable Components**: 
  - `PulseChannel` (for square waves)
  - `NoiseChannel` (for noise)
  - `PolynomialCounter` (for LFSR-based noise)
- **Location**: `crates/core/src/apu/` has all needed building blocks
- **New Work**: System-specific wrapper for SN76489/SN76496 variant

#### Graphics: VDP
- **Based on**: TMS9918A (well-documented, simpler than NES PPU)
- **Features**: Tilemap-based, sprite system, 16KB VRAM
- **Resolution**: 256×192 (similar complexity to NES 256×240)
- **Reusable**: Existing tile decoder utilities from `crates/core/src/ppu/`

### 4. Two Systems for One Effort

**Master System and Game Gear share ~95% of their code:**

| Component | Master System | Game Gear | Notes |
|-----------|--------------|-----------|-------|
| CPU | Z80 @ 3.58 MHz | Z80 @ 3.58 MHz | Identical |
| VDP Chip | 315-5124/5246 | 315-5378 | Minor differences |
| PSG | SN76496 (mono) | SN76496 (stereo) | Same chip, different output |
| Resolution | 256×192 | 160×144 | Different display window |
| Colors | 64 palette (32 on-screen) | 4096 palette (32 on-screen) | Extended color space |
| Input | Gamepad | Gamepad + Start | Nearly identical |

**Implementation Strategy:**
1. Build Master System first
2. Add Game Gear as variant with:
   - Different resolution/viewport
   - Extended color palette (12-bit vs 6-bit)
   - Stereo audio panning

### 5. Mature Ecosystem for Testing

**Test Resources:**
- [Homebrew test ROMs available](https://www.smspower.org/Homebrew/)
- Well-known game library for compatibility testing
- Existing emulators for validation (Gearsystem is open-source)

**ROM Detection:**
- Simple header detection (similar to existing NES/GB loaders)
- Common formats: .sms, .gg (raw binary with optional headers)

## Technical Implementation Roadmap

### Phase 1: Z80 CPU Completion (1-2 weeks)
- [ ] Complete Z80 instruction set (~200-250 opcodes)
- [ ] Implement all addressing modes
- [ ] Add interrupt support (IM 1 primarily, IM 2 for completeness)
- [ ] Shadow registers and index registers (IX/IY)
- [ ] Comprehensive unit tests
- [ ] Cycle-accurate timing

**Estimated LOC**: ~1,500-2,000 (similar to existing cpu_6502.rs)

### Phase 2: SN76489 PSG Audio (1 week)
- [ ] Create `Sn76489Psg` struct using existing components
- [ ] 3 square wave channels (reuse `PulseChannel`)
- [ ] 1 noise channel (reuse `NoiseChannel` + `PolynomialCounter`)
- [ ] Register interface (frequency, volume, control)
- [ ] Implement Sega variant differences (16-bit LFSR)
- [ ] Unit tests for each channel

**Estimated LOC**: ~400-600

### Phase 3: VDP Graphics (2-3 weeks)
- [ ] VDP state machine and registers
- [ ] 16KB VRAM management
- [ ] Tilemap rendering (background)
- [ ] Sprite engine (64 sprites, size modes)
- [ ] Color palette (64 colors, 32 simultaneous)
- [ ] Scrolling support
- [ ] Line interrupt timing
- [ ] Implement as `Renderer` trait

**Estimated LOC**: ~1,000-1,500

### Phase 4: Master System Integration (1 week)
- [ ] Memory map implementation (ROM, RAM, I/O ports)
- [ ] Input handling (controller ports)
- [ ] System struct implementing `System` trait
- [ ] ROM loading and cartridge banking
- [ ] Save state support
- [ ] Integration tests

**Estimated LOC**: ~800-1,000

### Phase 5: Game Gear Extension (3-5 days)
- [ ] Extended color palette (4096 colors)
- [ ] Resolution/viewport adjustment (160×144)
- [ ] Stereo audio panning
- [ ] Game Gear-specific input (Start button)
- [ ] ROM format detection

**Estimated LOC**: ~200-300 (mostly configuration differences)

### Phase 6: Testing and Refinement (1 week)
- [ ] Test ROM implementation for smoke tests
- [ ] Game compatibility testing
- [ ] Timing refinement
- [ ] Bug fixes and edge cases

**Total Estimated Time**: 6-8 weeks  
**Total Estimated LOC**: ~4,000-5,500 (comparable to existing system implementations)

## Comparison with Alternatives

### Game Boy Advance
**Pros:**
- Popular system with large game library
- Well-documented

**Cons:**
- ARM7TDMI CPU (new architecture, no existing implementation)
- Complex PPU with multiple modes, rotoscale, affine transforms
- DMA and advanced hardware features
- Estimated 8-12 weeks implementation time

### Sega Genesis/Mega Drive
**Pros:**
- Z80 + 68000 CPUs (could reuse Z80 for sound)
- Popular system

**Cons:**
- Requires new 68000 CPU implementation
- Complex VDP with multiple modes
- FM synthesis audio (YM2612) is more complex
- Estimated 10-14 weeks implementation time

### Neo Geo Pocket (Color)
**Pros:**
- Well-documented
- Simple architecture

**Cons:**
- Less popular/smaller library
- TLCS-900H CPU (new architecture)
- Less code reuse opportunity
- Estimated 6-8 weeks but with less benefit

## Alignment with Project Goals

### Hemulator's Implementation Philosophy
From `AGENTS.md`:
> "Always prefer full, tested implementations of each module/component, even if all parts aren't immediately used"

SMS/Game Gear fit this perfectly:
- **Full Z80 implementation** will be reusable for future systems (ZX Spectrum, MSX, CP/M)
- **SN76489 PSG** is used in many systems (ColecoVision, TI-99/4A, BBC Micro)
- **VDP techniques** applicable to similar tilemap-based systems

### Code Reuse Benefits
- Z80 can later be used for: ZX Spectrum, MSX, ColecoVision, Amstrad CPC
- SN76489 PSG for: ColecoVision, TI-99/4A, Tandy 1000, BBC Micro
- Tilemap rendering patterns for: Various 8/16-bit systems

## Success Criteria

**Minimum Viable Implementation:**
- [ ] Z80 CPU with full instruction set
- [ ] SN76489 PSG with all 4 channels
- [ ] VDP with background and sprite rendering
- [ ] Master System ROM loading and execution
- [ ] 80%+ compatibility with commercial games

**Stretch Goals:**
- [ ] Game Gear support
- [ ] FM sound unit (optional, Master System only)
- [ ] Light gun support
- [ ] 90%+ game compatibility
- [ ] Comprehensive test suite

## Risk Assessment

**Low Risk:**
- ✅ Well-documented hardware
- ✅ Existing CPU stub to build upon
- ✅ Proven audio component architecture
- ✅ Similar complexity to completed systems

**Medium Risk:**
- ⚠️ VDP timing accuracy (requires careful testing)
- ⚠️ Cartridge mapper variety (though less than NES)

**Mitigation:**
- Use existing emulators (Gearsystem) for validation
- Implement test ROM suite early
- Leverage SMS Power! community resources

## Conclusion

**Sega Master System and Game Gear are the optimal next implementation for Hemulator** due to:

1. **Perfect difficulty balance**: Challenging enough to be rewarding, simple enough to be achievable
2. **Excellent documentation**: Multiple high-quality technical references
3. **Maximum code reuse**: Leverages existing Z80 CPU and audio components
4. **Two systems for one**: Game Gear comes almost free with Master System
5. **Future-proof**: Z80 and SN76489 implementations will benefit future systems
6. **Active community**: Resources and testing support readily available

**Estimated implementation time**: 6-8 weeks for complete Master System + Game Gear support

**Recommendation**: Proceed with Sega Master System implementation as the next major system addition to Hemulator.

---

## References

1. [Sega Master System Architecture - Rodrigo Copetti](https://www.copetti.org/writings/consoles/master-system/)
2. [SEGA Mk3 Hardware Reference Manual - Archive.org](https://archive.org/details/SEGAMk3HardwareReferenceManual)
3. [SMS Power! Development Documents](https://www.smspower.org/Development/Documents)
4. [Charles MacDonald's VDP Documentation](https://github.com/franckverrot/EmulationResources/blob/master/consoles/sms-gg/Sega%20Master%20System%20VDP%20documentation.txt)
5. [SN76489 PSG Documentation - Wikipedia](https://en.wikipedia.org/wiki/Texas_Instruments_SN76489)
6. [VGMPF SN76489 Technical Reference](https://www.vgmpf.com/Wiki/index.php?title=SN76489)
7. [Z80 CPU Documentation - Hemulator](docs/references/cpu_z80.md)
