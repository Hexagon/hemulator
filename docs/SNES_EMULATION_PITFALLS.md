# SNES Emulation Pitfalls and Edge Cases

This document outlines common pitfalls, edge cases, and timing issues in SNES emulation based on research and community documentation.

## Current Implementation Status

Our SNES emulator implements:
- ✅ Mode 0 and Mode 1 PPU modes
- ✅ Sprite rendering with priority
- ✅ 16-bit controller input (serial and auto-joypad)
- ✅ VBlank/NMI infrastructure
- ✅ Basic timing (NTSC frame cycle counts)

## Critical Pitfalls and Edge Cases

### 1. Timing and Scanline Issues

**Issue**: SNES has region-specific scanline timing with special cases:
- NTSC: 262 scanlines/frame, ~1364 master cycles/scanline
- PAL: 312 scanlines/frame with different timing
- Special cases: "short scanlines" (NTSC, interlace off, field=1, V=240)
- Special cases: "long scanlines" (PAL, interlace on, field=1, V=311)

**Current Status**: ✅ We use NTSC timing constants (SNES_FRAME_CYCLES = 89,342)

**Potential Issues**:
- No PAL support yet
- No interlace mode handling
- Games using mid-scanline effects may not work correctly

**Games Affected**: Air Strike Patrol (uses precise raster timing)

**Recommendation**: 
- Add PAL timing mode support
- Document that mid-scanline effects are not supported in current frame-based renderer

### 2. PPU Priority and Rendering

**Issue**: Background and sprite priority system is complex:
- Each BG tile has a priority bit (high/low priority)
- Sprites have priority levels (0-3)
- Priority stack: High-priority sprites > High-priority BG > Low-priority sprites > Low-priority BG
- In Mode 1, BG3 has a global priority toggle bit in $2105

**Current Status**: ⚠️ Basic rendering implemented, but priority not fully handled

**Edge Cases**:
1. **Sprite-BG Priority Ties**: When sprite and BG have equal priority, sprite with lower OAM index wins
2. **BG3 Global Priority (Mode 1)**: BG3 can be set to render above all sprites (common for HUDs)
3. **Sprite Overlap**: With 128 sprites but only 32/scanline limit, need proper overflow handling
4. **Transparency Handling**: Palette index 0 is transparent for both BG and sprites

**Potential Issues**:
- Priority bit handling not implemented in tile rendering
- No sprite-per-scanline limit (32 sprites, 34 8x8 slots)
- BG3 priority toggle bit ($2105 bit 3) not respected in Mode 1

**Recommendation**:
- Add priority bit handling to BG rendering
- Implement sprite-per-scanline limits with proper overflow behavior
- Add BG3 priority toggle support in Mode 1

### 3. Controller Input Edge Cases

**Issue**: SNES controller serial reading has subtle timing requirements:

**Serial Read Process**:
1. Write 1 to $4016 to latch controller state
2. Write 0 to $4016 to enter serial mode
3. Read $4016/$4017 16 times (one bit per read)

**Edge Cases**:
1. **First Bit Quirk**: B button data available immediately after latch, other bits on clock edges
2. **Unused Bits**: Bits 13-16 return '1' (no expansion hardware)
3. **Button Polarity**: Hardware uses active-low (0=pressed, 1=released), but our emulator uses active-high (1=pressed, 0=released) as an internal representation. This is a valid design choice as long as the emulation is functionally correct for games.
4. **Auto-Joypad Read**: $4200 bit 0 enables automatic read during VBlank to $4218-$421F

**Current Status**: ✅ Serial reading implemented, ✅ Auto-joypad read implemented with enable/disable control

**Implementation**:
- $4200 bit 0 controls auto-joypad read enable/disable
- When disabled, $4218-$421F registers return 0
- When enabled, registers provide controller state during VBlank

**Potential Issues**:
- No support for extended controllers (mouse, multitap)

**Recommendation**:
- Document that mouse/multitap are not supported

### 4. VRAM Access Timing

**Issue**: VRAM can only be safely accessed during VBlank or when rendering is disabled

**Current Status**: ✅ VRAM access protection implemented

**Edge Cases**:
1. **Force Blank**: When $2100 bit 7 is set, screen is blanked and VRAM is always accessible
2. **During Rendering**: VRAM writes during active display are ignored (matches hardware)
3. **VRAM Increment Modes**: $2115 controls auto-increment on low/high byte access

**Implementation**:
- VRAM writes are only allowed during VBlank or force blank
- Writes during active display are ignored and logged as warnings
- Matches SNES hardware behavior for better accuracy

### 5. Color Math and Windows

**Issue**: Many games use color math (add/subtract colors) and window masking

**Current Status**: ❌ Not implemented

**Edge Cases**:
- Color addition/subtraction between main and sub-screens
- Window clipping regions (up to 2 windows per layer)
- Window logic (AND, OR, XOR, XNOR)

**Games Affected**: Any game using transparency effects, fades, or HUD windows

**Recommendation**:
- ✅ Already documented as not supported in MANUAL.md
- Consider as future enhancement

### 6. Enhancement Chips

**Issue**: Many SNES games use cartridge enhancement chips

**Common Chips**:
- SuperFX (Star Fox, Yoshi's Island)
- SA1 (Super Mario RPG, Kirby Super Star)
- DSP-1 through DSP-4 (Pilotwings, Super Mario Kart)
- S-DD1 (Street Fighter Alpha 2, Star Ocean)
- Cx4 (Mega Man X2/X3)

**Current Status**: ❌ Not implemented

**Recommendation**:
- ✅ Already documented as not supported
- Games requiring enhancement chips will not work

### 7. Audio (SPC700/DSP)

**Issue**: SNES has a separate SPC700 CPU for audio with its own memory and DSP

**Current Status**: ❌ Not implemented (stub only)

**Edge Cases**:
- SPC700 runs independently at ~1.024 MHz
- 64KB audio RAM
- 8 voice DSP with ADPCM sample playback
- Echo effects, pitch modulation, noise generation

**Recommendation**:
- ✅ Already documented as not supported
- Consider as major future enhancement

## Testing Recommendations

### Test ROMs
1. **Current**: Basic checkerboard pattern (tests Mode 0, 2bpp rendering) ✅
2. **Needed**: Mode 1 test with priority bits
3. **Needed**: Sprite overflow test (>32 sprites/scanline)
4. **Needed**: Controller test ROM to verify serial I/O matches SNES behavior
5. **Needed**: VRAM access timing test

**Note**: Controller functionality has been tested with the auto-joypad registers ($4218-$421F) and serial reads ($4016-$4017) showing correct behavior in unit tests.

### Commercial Games for Testing
- **Super Mario World**: Tests Mode 1, sprite rendering, scrolling
- **F-Zero**: Tests Mode 7 (not supported yet)
- **Donkey Kong Country**: Tests priority, transparency
- **Super Metroid**: Tests windows, color math (not supported yet)

## Known Limitations

### Documented in MANUAL.md
- Mode 0 and Mode 1 only (no Mode 2-7)
- No enhancement chip support
- No audio (SPC700/DSP)
- No windows or color math
- No HDMA
- Frame-based rendering (not cycle-accurate)

### Should Be Added to MANUAL.md
- ✅ All key limitations are already documented in MANUAL.md (PAL support, VRAM access protection, sprite-per-scanline limits, priority bit handling, mouse/multitap)

## Action Items

### High Priority
1. ✅ ~~Fix controller input (implemented in recent commits)~~
2. ✅ ~~Fix VBlank/NMI timing (implemented in recent commits)~~
3. ✅ ~~Implement auto-joypad read support ($4218-$421F registers)~~
4. ✅ ~~Implement priority bit handling in BG rendering~~
5. ✅ ~~Add sprite-per-scanline limits~~

### Medium Priority
6. ✅ ~~Add VRAM access protection~~
7. ✅ ~~Add BG3 priority toggle for Mode 1~~
8. Create test ROMs for priority and sprite overflow
9. ✅ ~~Implement $4200 auto-read enable/disable control~~

### Low Priority
10. Add PAL timing support
11. Document all limitations in MANUAL.md
12. Implement windows and color math
13. Add enhancement chip support

## References

- [SNESdev Wiki - Timing](https://snes.nesdev.org/wiki/Timing)
- [Super Famicom Development Wiki - Backgrounds](https://wiki.superfamicom.org/backgrounds)
- [Super Famicom Development Wiki - Registers](https://wiki.superfamicom.org/registers)
- [GameSX - SNES Controller Data](https://gamesx.com/controldata/snesdat.htm)
- [bsnes Documentation](https://bsnes.org/articles/edge-of-emulation/)
- [Fabien Sanglard - SNES PPU Rendering](https://fabiensanglard.net/snes_ppus_why/)

## Notes

This document should be updated as features are implemented and new edge cases are discovered.

**About Button Polarity**: While SNES hardware uses active-low polarity (0=pressed), this emulator uses active-high (1=pressed) as an internal representation. This is a valid design choice that doesn't affect game compatibility as long as the serial protocol emulation is functionally correct. The internal representation is independent of the hardware protocol.

Last updated: 2026-01-02
