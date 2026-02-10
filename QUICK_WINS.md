# Quick Wins - High Impact, Low Effort Items

This document identifies issues and TODO items that can be fixed relatively quickly but have significant impact on user experience or system functionality.

## Immediate Quick Wins (1-2 hours each)

### 1. PC QBasic Keyboard Input (#171)
**Effort**: Low (1-2 hours)  
**Impact**: Medium  
**Status**: PC keyboard handling exists but QBasic-specific issue

**Why it's quick**:
- Keyboard system already implemented (`crates/systems/pc/src/keyboard.rs`)
- Issue is specific to QBasic (edit command works)
- Likely a key buffer or special key handling issue

**Files to check**:
- `crates/systems/pc/src/keyboard.rs`
- `crates/systems/pc/src/cpu.rs` (INT 16h keyboard BIOS)

**Testing**: MS-DOS 5.0 with QBasic

---

### 2. CHIP-8 Audio Beep (TODO Low)
**Effort**: Low (1-2 hours)  
**Impact**: Low-Medium  
**Status**: Returns silence, needs simple beep tone

**Why it's quick**:
- CHIP-8 only has one audio feature (simple beep)
- Audio infrastructure already exists
- Just needs a tone generator

**Files**:
- `crates/frontend/gui/src/main.rs:602`
- Generate simple square wave or sine tone

**Testing**: Any CHIP-8 ROM with sound timer

---

### 3. HuC1 IR Sensor Documentation (TODO Low)
**Effort**: Very Low (15 minutes)  
**Impact**: Low  
**Status**: Works but undocumented

**Why it's quick**:
- Just needs comment additions
- No code changes needed

**Files**:
- `crates/systems/gb/src/mappers/huc1.rs:99-100,119-122`

---

## Medium Effort, High Impact (1-2 days each)

### 4. MBC3 RTC Persistence (TODO Medium)
**Effort**: Medium (4-6 hours)  
**Impact**: High (Pokemon games)  
**Status**: RTC implemented but resets on restart

**Why it's valuable**:
- Affects popular Pokemon Gold/Silver/Crystal
- RTC logic already exists
- Just needs serialization

**Files**:
- `crates/systems/gb/src/mappers/mbc3.rs`
- Add to save state system

**Testing**: Pokemon Gold/Silver/Crystal time-based events

---

### 5. PC Disk Geometry Detection (TODO Medium)
**Effort**: Medium (3-4 hours)  
**Impact**: Medium  
**Status**: Hardcoded to 1.44MB floppies

**Why it's valuable**:
- Expands PC/DOS compatibility
- BPB parsing is well-documented
- Affects 360KB, 720KB, 1.2MB disk images

**Files**:
- `crates/systems/pc/src/disk.rs:65`

**Testing**: Various DOS disk images

---

### 6. Game Boy Sprite Per-Scanline Limit (TODO Medium)
**Effort**: Medium (4-6 hours)  
**Impact**: Medium-High  
**Status**: Currently relaxed to 40, should be 10

**Why it's valuable**:
- Fixes sprite flicker hacks
- Improves hardware accuracy
- May fix Wario Land 2 issue (#476)

**Files**:
- `crates/systems/gb/src/ppu.rs`

**Testing**: Games with sprite-heavy scenes

---

## Optimizations (Variable effort, measurable impact)

### 7. Game Boy Per-Scanline Sprite Allocation (TODO Low)
**Effort**: Low (1-2 hours)  
**Impact**: Low (5-10% performance)  
**Status**: Vec allocation 144 times per frame

**Why it's quick**:
- Simple array replacement
- Clear performance win
- Low risk

**Files**:
- `crates/systems/gb/src/ppu.rs:887`

---

### 8. NES Sprite Evaluation Early Exit (TODO Low)
**Effort**: Low (30 minutes)  
**Impact**: Low (minor performance)  
**Status**: Iterates all 64 sprites even when only 8 needed

**Why it's quick**:
- Simple loop optimization
- Just move break statement
- Preserve overflow detection

**Files**:
- `crates/systems/nes/src/ppu.rs:1230-1314`

---

## Documentation Quick Wins (15-30 minutes each)

### 9. PC Magic Number Documentation (TODO Low)
**Effort**: Very Low  
**Impact**: Low (code maintainability)

**Files**:
- `crates/systems/pc/src/cpu.rs`
- `crates/systems/pc/src/bus.rs`

**What to add**:
- 0xB8000 (video memory) comments
- 0x400 (BIOS Data Area) references
- Port address ranges
- Boot sector signature 0xAA55

---

### 10. INT 10h Documentation (TODO Low)
**Effort**: Low  
**Impact**: Low (code maintainability)

**Files**:
- `crates/systems/pc/src/cpu.rs:412+`

**What to add**:
- Module-level overview
- Summary of supported video BIOS functions
- Mode support matrix

---

## Strategic Quick Wins (May unlock multiple fixes)

### 11. NES Mapper State Inspection (TODO Medium)
**Effort**: Medium (4-6 hours)  
**Impact**: High (debugging capability)

**Why it's strategic**:
- Would help debug all mapper issues
- Could accelerate fixing #307, #324, #363
- Improves debugger capabilities

**Files**:
- `crates/systems/nes/src/debugger.rs`
- Each mapper file

**What to expose**:
- Bank select registers
- IRQ counter state
- CHR latch state

---

### 12. Instruction Trace JSON Export (TODO Low)
**Effort**: Low-Medium (2-4 hours)  
**Impact**: Medium (debugging capability)

**Why it's strategic**:
- Would help debug PC type command issue (#169)
- Enables automated testing
- Machine-parseable output

**Files**:
- `crates/frontend/gui/src/main.rs:1993-2179`

**Add**: `--debug-dump-format json` flag

---

## Prioritized Quick Win Order

**If you have 1 day**:
1. CHIP-8 Audio Beep (1-2 hours)
2. PC QBasic Keyboard (#171) (1-2 hours)
3. Game Boy Sprite Allocation Optimization (1-2 hours)
4. NES Sprite Evaluation Early Exit (30 min)
5. Documentation items (1-2 hours total)

**If you have 1 week**:
1. All 1-day items above
2. MBC3 RTC Persistence (4-6 hours)
3. Game Boy Sprite Per-Scanline Limit (4-6 hours)
4. PC Disk Geometry Detection (3-4 hours)
5. NES Mapper State Inspection (4-6 hours)

**If you have 2 weeks**:
- All above items
- Plus begin work on Priority 1-3 from PRIORITY_ANALYSIS.md
- Focus on NES Mapper 1 issues

## Return on Investment (ROI) Analysis

| Item | Effort | Impact | ROI | Issues Fixed |
|------|--------|--------|-----|--------------|
| CHIP-8 Audio | Low | Medium | High | 0 (enhancement) |
| PC QBasic Keyboard | Low | Medium | High | 1 (#171) |
| GB Sprite Limit | Medium | High | High | 1? (#476) |
| MBC3 RTC | Medium | High | High | 0 (enhancement) |
| PC Disk Geometry | Medium | Medium | Medium | 0 (enhancement) |
| NES Mapper Inspector | Medium | High | Very High | 0 (tool for fixing 3+) |
| Sprite Allocation Opt | Low | Low | Medium | 0 (perf) |
| Documentation | Very Low | Low | High | 0 (maintainability) |

## Notes

- **Quick wins build momentum** - Early successes make harder problems easier
- **Debugging tools unlock fixes** - Mapper inspector and JSON export enable faster debugging
- **Performance wins are satisfying** - Users notice faster emulation
- **Documentation prevents future issues** - Small time investment, long-term benefit

## Testing Strategy for Quick Wins

After each fix:
1. Run existing tests: `cargo test --workspace`
2. Manual testing with specific ROMs
3. Check for regressions in other games
4. Update TODO.md to check off completed items
5. Report progress and commit
