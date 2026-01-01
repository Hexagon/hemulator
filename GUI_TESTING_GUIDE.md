# GUI Improvements - Visual Testing Guide

This guide helps reviewers verify all the GUI improvements made in this PR.

## How to Test

### Prerequisites
Build the project:
```bash
cargo build --release
```

### Test Scenarios

#### 1. Color Scheme & Font Contrast

**Test:** Launch the emulator (any ROM or without ROM)
```bash
./target/release/hemu
```

**Expected Results:**
- Menu bar at top has dark gray background (RGB 37, 37, 38)
- Status bar at bottom has dark gray background
- Property pane on right has dark gray background
- All text is clearly readable with light gray/white colors
- Text contrast is significantly better than before
- Overall appearance similar to VS Code dark theme

**Check:**
- [ ] Menu bar has good contrast
- [ ] Status bar has good contrast
- [ ] Property pane has good contrast
- [ ] All text is easily readable

---

#### 2. Emulation Speed Selector

**Test:** Load any ROM and adjust speed
```bash
./target/release/hemu test_roms/nes/test.nes
```

**Steps:**
1. Open Property Pane (right side)
2. Expand "Project Settings" section
3. Click different speed buttons: 25%, 50%, 100%, 200%, 400%
4. Observe emulation speed changes

**Expected Results:**
- No slider present (only buttons)
- Current speed displayed as "Emulation Speed: X%"
- Clicking 25% makes emulation run at 1/4 speed
- Clicking 400% makes emulation run at 4x speed
- Speed changes are immediate and visible

**Check:**
- [ ] Slider is removed
- [ ] Speed buttons work
- [ ] Speed changes are visible
- [ ] Current speed is displayed

---

#### 3. Display Filter Selector

**Test:** Load a ROM and select different filters
```bash
./target/release/hemu test_roms/nes/test.nes
```

**Steps:**
1. Open Property Pane
2. Expand "Project Settings" section
3. Click "Display Filter" dropdown
4. Select each filter one by one

**Expected Results:**
- Dropdown shows 6 filters:
  - None (default, no effect)
  - Sony Trinitron (RGB stripes, scanlines, bloom)
  - IBM 5151 (green monochrome, phosphor glow)
  - Commodore 1702 (shadow mask, moderate scanlines)
  - Sharp LCD (grayscale, blur, pixel grid)
  - RCA Victor (B&W, heavy scanlines, vignette)
- Selecting a filter immediately applies it to the display
- Filter effect is visible on the emulator screen

**Check:**
- [ ] Dropdown works and shows all 6 filters
- [ ] "None" shows raw pixels
- [ ] Other filters apply visible effects
- [ ] Filter changes are immediate

---

#### 4. Mount Points Display

**Test with NES ROM:**
```bash
./target/release/hemu test_roms/nes/test.nes
```

**Expected Results:**
- Property Pane → "Mount Points" section shows:
  - Cartridge: test.nes (or filename of loaded ROM)

**Test with PC system (if .hemu file available):**
```bash
./target/release/hemu workbench/workbench.hemu
```

**Expected Results:**
- Property Pane → "Mount Points" section shows:
  - BIOS: (filename if mounted)
  - Floppy A: (filename if mounted)
  - Floppy B: (filename if mounted)
  - Hard Drive: (filename if mounted)

**Check:**
- [ ] Mount points show when ROM is loaded
- [ ] Mount points show "No mount points available" when nothing loaded
- [ ] Filenames are displayed (not full paths)
- [ ] Mount status is accurate

---

#### 5. PC Config Tab (DBA)

**Test:** Load a PC system
```bash
./target/release/hemu workbench/workbench.hemu
# OR any PC disk image
```

**Steps:**
1. After loading PC system, check tab bar
2. Click "PC Config" tab

**Expected Results:**
- "PC Config" tab appears automatically when PC system loads
- Tab shows:
  - **System Configuration:**
    - CPU Model (e.g., "Intel 8086")
    - Memory (e.g., "640 KB")
    - Video Adapter (e.g., "CGA Software Renderer")
    - Boot Priority (e.g., "Floppy First")
  - **Mounted Devices:**
    - BIOS: ✓ Mounted or ✗ Not mounted
    - Floppy A: ✓ Mounted or ✗ Not mounted
    - Floppy B: ✓ Mounted or ✗ Not mounted
    - Hard Drive: ✓ Mounted or ✗ Not mounted
- Tab has close button (✖) that's clearly visible

**Check:**
- [ ] PC Config tab appears for PC systems only
- [ ] CPU model is correct
- [ ] Memory size is correct
- [ ] Video adapter name is correct
- [ ] Boot priority is correct
- [ ] Mount status matches actual mounts
- [ ] Close button (✖) is visible and works

---

#### 6. Tab Close Button Visibility

**Test:** Open Help and Debug tabs
```bash
./target/release/hemu test_roms/nes/test.nes
```

**Steps:**
1. Press F1 to toggle Debug tab
2. Open menu → Help → Controls & Help

**Expected Results:**
- Help tab appears with "✖" button
- Debug tab appears with "✖" button
- PC Config tab (if PC loaded) has "✖" button
- All ✖ buttons are clearly visible with light gray text (RGB 220, 220, 220)
- Hovering shows tooltip: "Close [Tab Name] tab"
- Clicking closes the tab

**Check:**
- [ ] Help tab close button is visible
- [ ] Debug tab close button is visible
- [ ] PC Config tab close button is visible (when applicable)
- [ ] Close buttons work correctly

---

#### 7. Inactive Tab Text Color

**Test:** Have multiple tabs open
```bash
./target/release/hemu test_roms/nes/test.nes
```

**Steps:**
1. Open Help tab
2. Open Debug tab (F1)
3. Click between tabs

**Expected Results:**
- Active tab has bright white text (RGB 255, 255, 255)
- Inactive tabs have lighter gray text (RGB 180, 180, 180)
- Inactive tabs are clearly readable (not too dark)
- Clear visual distinction between active and inactive tabs

**Check:**
- [ ] Active tab text is bright
- [ ] Inactive tab text is clearly visible
- [ ] Easy to distinguish active from inactive
- [ ] No tabs have illegible text

---

## Quick Verification Checklist

All features:
- [ ] VS Code-like color scheme applied
- [ ] All text has good contrast
- [ ] Emulation speed buttons work (no slider)
- [ ] Display filter dropdown works with all 6 filters
- [ ] Mount points display correctly
- [ ] PC Config tab appears for PC systems
- [ ] Tab close buttons are visible
- [ ] Inactive tabs are readable

## Known Working Configurations

These test files are known to work:
- NES: `test_roms/nes/test.nes`
- Game Boy: `test_roms/gb/test.gb`
- Atari 2600: `test_roms/atari2600/test.bin`
- PC: `workbench/workbench.hemu` (if available)

## Troubleshooting

**If emulator doesn't start:**
- Check SDL2 libraries are installed
- Try building in debug mode: `cargo build`
- Check console output for errors

**If no display:**
- Ensure graphics drivers are working
- Try a different ROM file
- Check that ROM format is supported

**If changes aren't visible:**
- Ensure you built the release version
- Clear any cached settings: `rm config.json`
- Restart the emulator
