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
3. **Button Polarity**: Buttons are active-HIGH (1=pressed, 0=released) - **opposite of what research shows!**
4. **Auto-Joypad Read**: $4200 bit 0 enables automatic read during VBlank to $4218-$421F

**Current Status**: ✅ Serial reading implemented, ⚠️ Button polarity needs verification

**CRITICAL ISSUE FOUND**: Research indicates SNES controllers use **0=pressed, 1=released** (active-low), 
but our implementation and tests assume 1=pressed (active-high). This needs verification!

**Potential Issues**:
- Button polarity may be inverted
- No auto-joypad read implementation ($4200 bit 0, $4218-$421F registers)
- No support for extended controllers (mouse, multitap)

**Recommendation**:
- **URGENT**: Verify button polarity with hardware documentation
- Implement auto-joypad read for better compatibility
- Document that mouse/multitap are not supported

### 4. VRAM Access Timing

**Issue**: VRAM can only be safely accessed during VBlank or when rendering is disabled

**Current Status**: ⚠️ No VRAM access protection

**Edge Cases**:
1. **Force Blank**: When $2100 bit 7 is set, screen is blanked and VRAM is always accessible
2. **During Rendering**: VRAM writes during active display may be ignored or cause glitches
3. **VRAM Increment Modes**: $2115 controls auto-increment on low/high byte access

**Potential Issues**:
- VRAM writes during active display are not blocked
- May allow impossible writes that hardware would ignore

**Recommendation**:
- Add VRAM access protection (only allow when in VBlank or force blank)
- Log/warn when VRAM access attempted during active display

### 5. Color Math and Windows

**Issue**: Many games use color math (add/subtract colors) and window masking

**Current Status**: ❌ Not implemented

**Edge Cases**:
- Color addition/subtraction between main and sub-screens
- Window clipping regions (up to 2 windows per layer)
- Window logic (AND, OR, XOR, XNOR)

**Games Affected**: Any game using transparency effects, fades, or HUD windows

**Recommendation**:
- Document as not supported in MANUAL.md
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
- Document as not supported
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
- Document as not supported
- Consider as major future enhancement

## Testing Recommendations

### Test ROMs
1. **Current**: Basic checkerboard pattern (tests Mode 0, 2bpp rendering)
2. **Needed**: Mode 1 test with priority bits
3. **Needed**: Sprite overflow test (>32 sprites/scanline)
4. **Needed**: Controller test (verify button polarity)
5. **Needed**: VRAM access timing test

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
- No PAL support (NTSC only)
- Controller polarity uncertainty
- No VRAM access protection
- No sprite-per-scanline limits
- No priority bit handling
- No auto-joypad read

## Action Items

### High Priority
1. ✅ ~~Fix controller input (implemented in recent commits)~~
2. ✅ ~~Fix VBlank/NMI timing (implemented in recent commits)~~
3. **⚠️ VERIFY button polarity** (research suggests active-low, but we use active-high)
4. Add auto-joypad read support ($4200 bit 0, $4218-$421F)
5. Implement priority bit handling in BG rendering

### Medium Priority
6. Add VRAM access protection
7. Implement sprite-per-scanline limits
8. Add BG3 priority toggle for Mode 1
9. Create test ROMs for priority and sprite overflow

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
Last updated: 2025-12-30
