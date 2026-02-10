# Technical Root Cause Analysis

This document analyzes the likely root causes of reported issues based on system architecture and common emulation pitfalls.

## NES Issues (7 total)

### Category 1: Mapper 1 (MMC1) Issues
**Affected Issues**: #307, #324, #363  
**Symptoms**: Hangs, gray screens, games won't start

#### Likely Root Causes

1. **Serial Register Handling**
   - MMC1 uses a 5-bit serial shift register
   - Writes must accumulate 5 bits before applying to target register
   - Reset if bit 7 is set on any write
   - **Common bugs**: 
     - Not resetting shift register after 5 writes
     - Not handling reset bit correctly
     - Timing of register application

2. **Bank Switching Logic**
   - Control register bits determine PRG/CHR bank modes
   - Different games use different bank switching modes
   - **Common bugs**:
     - Incorrect bank size calculations (16KB vs 32KB)
     - Fixed bank selection (first vs last)
     - CHR bank switching interfering with PRG

3. **Mirroring Control**
   - MMC1 can switch between horizontal, vertical, and one-screen mirroring
   - **Common bugs**:
     - Not updating PPU mirroring mode
     - Incorrect mirroring bit extraction

**Debug Strategy**:
```rust
// Add extensive logging in mmc1.rs
- Log every serial write (bit value, shift count)
- Log register applications (which register, value)
- Log bank switching (old banks -> new banks)
- Compare with known-good emulator (Mesen, FCEUX)
```

**Files to Investigate**:
- `crates/systems/nes/src/mappers/mmc1.rs`
- Test ROM: create simple MMC1 bank switching test

---

### Category 2: PPU/Rendering Issues
**Affected Issues**: #486, #387, #378, #365  
**Symptoms**: Scrambled graphics, missing sprites, animation glitches

#### Likely Root Causes

1. **Sprite 0 Hit Timing** (Battletoads #378)
   - Used for mid-scanline effects (parallax scrolling)
   - TODO item already noted in TODO.md
   - **Common bugs**:
     - Hit detection at wrong X position
     - Odd frame timing issues
     - Transparent pixel handling

2. **PPU A12 Edge Detection** (Bee 52 #387)
   - Used by mappers for scanline IRQ counting
   - TODO items: "PPU A12 Edge Timing" and "Synthetic A12 Edge Generation"
   - **Common bugs**:
     - IRQ firing too early/late
     - Missing edges during rendering
     - Incorrect suppression during DRAM refresh

3. **Attribute Table Rendering** (California Games #486)
   - Color palettes for background tiles
   - **Common bugs**:
     - Incorrect attribute byte addressing
     - Wrong quadrant selection
     - Nametable mirroring issues

4. **Sprite Overflow Flag** (Worms Armageddon #365)
   - Set when more than 8 sprites on a scanline
   - **Common bugs**:
     - Not setting overflow correctly
     - Clearing at wrong time
     - Affecting sprite evaluation

**Debug Strategy**:
```
- Enable PPU logging for affected games
- Compare with known-good emulator frame-by-frame
- Use sprite viewer in debugger
- Check nametable/attribute table contents
```

**Files to Investigate**:
- `crates/systems/nes/src/ppu.rs` - Lines 980, 1103-1106, 1230-1314
- `crates/systems/nes/src/bus.rs` - Lines 109-114

---

## Atari 2600 Issue (#305)

### Symptoms
1. Background compressed and written twice horizontally
2. Player sprite not moveable sideways
3. Falling balls move wrong way, then jump

#### Root Cause Analysis

**1. Horizontal Positioning (HMOVE)**
- TIA uses HMOVE strobe for fine horizontal positioning
- **Likely bug**: HMOVE timing incorrect
- Should occur during horizontal blank
- **Effect**: Sprites/playfield appear in wrong positions

**2. Playfield Rendering**
- Playfield is drawn using 20-bit pattern (PF0, PF1, PF2)
- Pattern is mirrored or repeated based on CTRLPF
- **Likely bug**: 
  - Mirroring vs repeat mode wrong
  - Not accounting for playfield asymmetry
  - Pixel doubling issues

**3. Player Movement**
- Player position set by HMOVE and RESP0/RESP1
- **Likely bug**:
  - Not processing HMOVE correctly
  - HMP0/HMP1 registers not applied
  - Position counters not incrementing

**4. Missile/Ball Position**
- Similar to player positioning
- **Likely bug**: Same HMOVE issues affecting all objects

**Debug Strategy**:
```
- Trace TIA register writes (especially HMOVE, RESPx, HMPx)
- Compare horizontal position calculations with spec
- Check CTRLPF playfield control bits
- Verify playfield reflection vs repeat
```

**Files to Investigate**:
- `crates/systems/atari2600/src/tia.rs` - TIA rendering
- ATARI_2600_REVIEW_SUMMARY.md - Previous review notes

**References**:
- Stella Programmer's Guide
- TIA Hardware Manual
- Andrew Towers' TIA documentation

---

## PC/DOS Issues

### Issue #169: FreeDOS `type` Command Infinite Loop

#### Symptoms
- `type fdauto.bat` outputs same file repeatedly
- 36MB CPU trace available in typebug branch

#### Root Cause Hypotheses

**1. File Handle Not Closing**
- INT 21h AH=3Eh (Close File) not working
- File position not resetting
- **Effect**: Read continues from start instead of stopping at EOF

**2. EOF Detection Failure**
- INT 21h AH=3Fh (Read File) not returning 0 at EOF
- Carry flag not set on error
- **Effect**: Program thinks there's always more data

**3. File Position Tracking**
- INT 21h AH=42h (LSEEK) not updating position correctly
- Read pointer wrapping around
- **Effect**: Reads same data repeatedly

**4. Buffer Management**
- DOS buffer not flushing correctly
- Old data remaining in buffer
- **Effect**: Same content output multiple times

**Debug Strategy**:
```bash
# Analyze the trace file
grep "INT 21" typebug.cputrace | tail -1000 > last_1000_int21.txt
# Look for patterns:
# - AH=3F (read) calls and CX (bytes read) values
# - AH=3E (close) ever called?
# - AH=42 (lseek) position changes
# - Repeated sequences indicating loop
```

**Files to Investigate**:
- `crates/systems/pc/src/cpu.rs` - INT 21h implementation
- `crates/systems/pc/src/disk.rs` - File I/O operations

**Known Issues from TODO.md**:
- INT 21h file operations are stubs
- May be related to incomplete implementation

---

### Issue #171: QBasic Keyboard Non-Responsive

#### Symptoms
- QBasic interface shows up
- Keypresses do nothing
- DOS `edit` command works fine

#### Root Cause Hypotheses

**1. INT 16h Function Differences**
- QBasic may use different INT 16h functions than edit
- AH=00h (read key) vs AH=01h (check key) vs AH=10h/11h (extended)
- **Likely bug**: Extended keyboard functions not implemented

**2. Keyboard Buffer Handling**
- Edit may be more forgiving of buffer issues
- QBasic may check buffer in different way
- **Likely bug**: Buffer state not maintained correctly

**3. Special Key Handling**
- QBasic uses function keys, cursor keys heavily
- Extended scan codes required
- **Likely bug**: Scan codes for special keys wrong or missing

**4. Keyboard IRQ (INT 09h)**
- TODO.md notes INT 09h is "partial stub"
- QBasic may rely on IRQ for input
- **Likely bug**: IRQ not populating buffer correctly

**Debug Strategy**:
```
- Add logging to all INT 16h calls
- Compare QBasic vs edit keyboard access patterns
- Check scan code mappings for special keys
- Verify keyboard buffer BIOS data area (0x40:0x1A-0x1E)
```

**Files to Investigate**:
- `crates/systems/pc/src/cpu.rs` - INT 16h implementation
- `crates/systems/pc/src/keyboard.rs` - Keyboard handling
- TODO.md line 209: "INT 09h (Keyboard IRQ) - partial stub"

---

## SNES Issue (#526): SimCity Graphics Scrambled

### Symptoms
- Graphics scrambled
- Numbers glitch between values

#### Root Cause Hypotheses

**1. PPU Mode Issues**
- SimCity uses Mode 1 (4-color BG0/BG1 + 16-color BG2)
- **Likely bug**: Mode 1 implementation incomplete
- May be confusing BG priority or color depths

**2. Tile Addressing**
- 16x16 tile support required
- **Likely bug**: 16x16 tile calculation wrong
- Note: TODO.md shows unused 16x16 helper was removed

**3. VRAM Increment Mode**
- SimCity may use auto-increment on VRAM access
- Register $2115 controls increment
- **Likely bug**: Not incrementing at right time or by right amount

**4. Color Math / Transparency**
- Color math (add/subtract) on backgrounds
- **Likely bug**: Transparency not handled correctly
- Fixed color addition/subtraction wrong

**5. Window Masking**
- SimCity may use window effects
- **Likely bug**: Window clip logic incorrect

**Debug Strategy**:
```
- Log VRAM writes and reads
- Check which PPU mode is set
- Dump VRAM contents and compare with good emulator
- Check BG tile maps and character data
- Verify color palette setup
```

**Files to Investigate**:
- `crates/systems/snes/src/ppu.rs` - PPU implementation
- Focus on Mode 1 rendering (lines around mode checking)
- VRAM increment logic

---

## Game Boy Issue (#476): Wario Land 2 Menu Weird

### Symptoms
- Menu displays incorrectly

#### Root Cause Hypotheses

**1. MBC Type**
- Wario Land 2 uses MBC2 (unusual 512x4-bit RAM)
- **Likely bug**: MBC2 half-nibble RAM not implemented correctly
- Upper 4 bits should read as 1s

**2. Sprite Rendering**
- Issue may be sprite-related if menu uses sprites
- TODO.md notes sprite limit is relaxed (40 instead of 10)
- **Likely bug**: Incorrect sprite priority or OAM handling

**3. Window Layer**
- Window feature used for menus
- **Likely bug**: Window position or enable logic wrong

**4. Game Boy Color Features**
- Wario Land 2 is GBC-enhanced
- May use GBC-specific features
- **Likely bug**: CGB palette or attribute handling

**Debug Strategy**:
```
- Check which MBC is detected
- If MBC2, verify RAM read/write (only uses lower 4 bits)
- Check sprite count on menu screen
- Verify window register values (WY, WX)
- Compare with known-good emulator
```

**Files to Investigate**:
- `crates/systems/gb/src/mappers/mbc2.rs` - If it exists
- `crates/systems/gb/src/ppu.rs` - Window and sprite rendering
- TODO.md line 97-100: Sprite per-scanline limit

---

## Common Patterns Across Issues

### Timing Issues
- PPU A12 edges (NES)
- HMOVE timing (Atari 2600)
- Sprite 0 hit (NES)
- **Solution**: Cycle-accurate emulation or better timing approximations

### Register State Management
- MMC1 shift register (NES)
- Keyboard buffer (PC)
- VRAM increment (SNES)
- **Solution**: State machine verification and logging

### Edge Cases
- EOF detection (PC)
- MBC2 nibble RAM (Game Boy)
- Extended keyboard (PC)
- **Solution**: Comprehensive testing and hardware verification

### Rendering Bugs
- Horizontal positioning (Atari 2600)
- Tile addressing (SNES)
- Sprite limits (Game Boy, NES)
- **Solution**: Frame-by-frame comparison with accurate emulators

---

## Testing Recommendations

### Regression Testing
After any fix, test:
1. **NES**: Known working games (Super Mario Bros, Zelda, Mega Man 2)
2. **Atari 2600**: Pitfall, Adventure, Combat
3. **PC**: Boot FreeDOS, run COMMAND.COM commands
4. **SNES**: Known working games (if any)
5. **Game Boy**: Tetris, Pokemon Red, Link's Awakening

### Test ROM Development
Create simple test ROMs for:
1. **MMC1**: Bank switching test
2. **Atari 2600**: HMOVE and playfield test
3. **PC**: File I/O test (open, read, close, verify EOF)

### Debugging Tools Needed
1. **Mapper state inspector** (TODO.md line 196-200)
2. **Memory watchpoints** (TODO.md line 123-127)
3. **JSON debug dumps** (TODO.md line 309-313)

---

## Priority Ordering by Root Cause Confidence

**High Confidence (likely to fix issue)**:
1. NES Mapper 1 - Serial register or bank switching
2. PC type command - EOF or file handle management
3. Atari 2600 - HMOVE timing

**Medium Confidence (probable root cause)**:
4. PC QBasic - Extended keyboard functions
5. SNES SimCity - VRAM increment or Mode 1 rendering
6. Game Boy Wario Land 2 - Sprite limit or MBC2

**Low Confidence (needs investigation)**:
7. NES game-specific issues - Multiple possible causes each

---

## Resources for Investigation

### NES
- NESDev Wiki: https://www.nesdev.org/wiki/
- MMC1: https://www.nesdev.org/wiki/MMC1
- PPU: https://www.nesdev.org/wiki/PPU

### Atari 2600
- Stella Programmer's Guide
- TIA documentation by Andrew Towers

### PC/DOS
- Ralf Brown's Interrupt List
- IBM PC Technical Reference

### SNES
- SNESdev Wiki: https://snes.nesdev.org/
- Fullsnes documentation
- bsnes source code

### Game Boy
- Pan Docs: https://gbdev.io/pandocs/
- Game Boy CPU Manual
- MBC documentation
