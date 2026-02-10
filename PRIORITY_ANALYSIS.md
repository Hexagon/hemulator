# Hemulator Priority Analysis
**Date**: 2026-02-10  
**Purpose**: Identify the most important areas to work on based on open issues and TODO items

## Executive Summary

After analyzing 12 open GitHub issues and the comprehensive TODO.md (625 lines), the highest-impact areas for development are:

1. **NES Mapper 1 (MMC1)** - Affects 3 reported issues and blocks major game titles
2. **Atari 2600 Rendering** - Fundamental system issues preventing gameplay
3. **PC/DOS type command bug** - Clear debugging path with trace file available
4. **SNES Graphics** - System mostly complete but specific rendering issues remain

## Detailed Analysis

### GitHub Issues (12 Open)

#### By System Distribution
- **NES**: 7 issues (58%)
- **PC/DOS**: 2 issues (17%)
- **Atari 2600**: 1 issue (8%)
- **SNES**: 1 issue (8%)
- **Game Boy**: 1 issue (8%)

#### By Severity

**Critical Issues** (blocking gameplay):
1. **#307** - NES Mapper 1: games hang, Rad Racer hangs before showing anything
2. **#324** - Mike Tyson Punch-Out: gray screen (likely Mapper 1)
3. **#305** - Atari 2600: horizontal drawing issues, player control broken
4. **#169** - PC: FreeDOS `type` command infinite loop (36MB trace available)

**High Priority** (major rendering issues):
5. **#363** - Rad Racer 2 / Turbo Racing: rendering issues
6. **#526** - SNES SimCity: scrambled graphics
7. **#387** - NES Bee 52: map broken, freezes
8. **#171** - PC: QBasic keyboard non-responsive

**Medium Priority** (game-specific glitches):
9. **#486** - NES California Games: menus weird
10. **#378** - NES Battletoads: plane animation glitches
11. **#365** - NES Worms Armageddon: scrambled land
12. **#476** - Game Boy Wario Land 2: menu weird

### TODO.md Analysis

#### Status Overview
- **Critical**: ✅ All resolved (SNES DMA, Audio DSP, ColecoVision)
- **High**: 2 items (both GBA-related)
- **Medium**: ~40 items across all systems
- **Low**: ~50 items (optimizations, documentation)

#### High Priority TODO Items
1. **GBA Audio + Save States** - Complete system functionality
2. **GBA BIOS SWI Functions** - Many games hang without these

#### Notable Medium Priority Items
- **NES Mapper State Inspection** - Would help debug mapper issues
- **PC INT 21h DOS API** - File I/O compatibility
- **Atari 2600** - (No specific TODO items, but GitHub issue #305 is critical)
- **SNES FastROM Timing** - Performance optimization

## Priority Ranking with Justification

### Priority 1: NES Mapper 1 (MMC1) Issues

**Why it's #1**:
- Affects **3 reported GitHub issues** (#307, #324, #363)
- Mapper 1 is used by **major game titles**: 
  - The Legend of Zelda
  - Mega Man series
  - Final Fantasy
  - Metroid
  - Rad Racer (specifically reported)
  - Mike Tyson's Punch-Out (specifically reported)
- Implementation exists but has bugs
- High user impact (58% of issues are NES-related)

**Technical Details**:
- File: `crates/systems/nes/src/mappers/mmc1.rs`
- Has unit tests (`mmc1_serial_write`)
- Likely issues: serial register handling, bank switching, or timing

**Expected Impact**: Fixing this could resolve 25% of all open issues

---

### Priority 2: Atari 2600 Rendering (#305)

**Why it's #2**:
- **Fundamental system issues** blocking all gameplay
- Multiple interconnected problems:
  - Background rendering compressed/duplicated horizontally
  - Player sprite not moveable sideways
  - Falling balls move wrong direction with position jumps
- System status: "🚧 In Development - rendering WIP"

**Technical Details**:
- Likely TIA (Television Interface Adapter) rendering issues
- Horizontal positioning/HMOVE timing problems
- Input handling may be affected

**Expected Impact**: Would make Atari 2600 system functional

---

### Priority 3: PC/DOS FreeDOS `type` Command (#169)

**Why it's #3**:
- **Clear debugging path**: 36MB CPU trace file available in typebug branch
- Infinite loop indicates specific memory/register handling bug
- Could reveal broader PC system issues
- Would improve PC/DOS compatibility

**Technical Details**:
- File: typebug branch - `typebug.cputrace`
- Likely issues: memory read/write handling, register state, or I/O
- May be related to INT 21h file operations

**Expected Impact**: Better PC/DOS system stability and compatibility

---

### Priority 4: SNES Graphics Issues (#526)

**Why it's #4**:
- System mostly complete (audio working, modes 0-1 done)
- Specific game issue (SimCity) suggests edge case bug
- Lower priority than broader issues

**Technical Details**:
- PPU implementation in `crates/systems/snes/src/ppu.rs`
- Modes 0-1 complete but specific rendering bugs remain
- Numbers glitch between specific values

**Expected Impact**: Improved SNES game compatibility

---

### Priority 5: GBA System Completion

**Why it's #5**:
- No user-reported issues (yet)
- System functionality gaps rather than bugs
- Would enable new system support

**Technical Details**:
- Missing APU audio
- Missing save state serialization  
- Many BIOS SWI functions incomplete
- CPU/PPU/DMA/timers already implemented

**Expected Impact**: Complete GBA system support

---

### Priority 6: Individual NES Game Issues

**Why it's #6**:
- Game-specific rather than systemic
- May be resolved by fixing Mapper 1 issues
- Lower user impact per issue

**Affected Games**:
- California Games (#486)
- Bee 52 (#387)
- Battletoads (#378)
- Worms Armageddon (#365)

---

### Priority 7: PC QBasic Keyboard (#171)

**Why it's #7**:
- Specific to one program
- PC system is "⚠️ Experimental"
- Lower than systemic bugs

---

### Priority 8: Game Boy Wario Land 2 (#476)

**Why it's #8**:
- Single game issue
- Game Boy system is "✅ Fully Functional"
- Likely edge case or MBC-specific bug

## Recommended Action Plan

### Phase 1: Maximum Impact (Weeks 1-2)
1. **Debug and fix NES Mapper 1 (MMC1)**
   - Could resolve 3 issues
   - Test with Zelda, Mega Man, Rad Racer, Punch-Out
   
2. **Fix Atari 2600 rendering fundamentals**
   - Focus on horizontal positioning
   - Fix player movement and sprite rendering

### Phase 2: System Stability (Weeks 3-4)
3. **Analyze and fix PC FreeDOS type bug**
   - Use provided trace file
   - May reveal other PC issues
   
4. **Debug SNES SimCity graphics**
   - Review PPU mode handling
   - Check number rendering specifically

### Phase 3: Feature Completion (Weeks 5-6)
5. **Complete GBA implementation**
   - Implement APU audio
   - Add save state support
   - Complete critical BIOS SWI functions

6. **Address remaining NES game-specific issues**
   - Test and fix individual games
   - May require PPU or mapper refinements

## Success Metrics

- **Issue Resolution**: Target 6-8 of 12 issues closed (50-67%)
- **System Status**: Move Atari 2600 from "🚧 In Development" to "✅ Fully Working"
- **Game Coverage**: Increase NES compatibility from ~90% to ~95%
- **User Impact**: Address 75% of reported issues by user count

## Notes

- NES has the most user engagement (7 issues) - prioritize accordingly
- Atari 2600 and PC systems need foundational work before feature additions
- SNES and GBA have good foundations, need polish and completion
- Game Boy is largely complete, issues are edge cases

## References

- [GitHub Issues](https://github.com/Hexagon/hemulator/issues)
- [TODO.md](/home/runner/work/hemulator/hemulator/TODO.md)
- [README.md](/home/runner/work/hemulator/hemulator/README.md)
- [ARCHITECTURE.md](/home/runner/work/hemulator/hemulator/ARCHITECTURE.md)
