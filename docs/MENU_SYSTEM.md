# Menu System UI Layout

## Overview
The new UI consists of three layers:
1. **Menu Bar** (top) - Always visible, 24px height
2. **Game Display** (middle) - The emulated system's output
3. **Status Bar** (bottom) - Always visible, 20px height

## Menu Bar (Top)

```
┌────────────────────────────────────────────────────────────────┐
│ File   Emulation   State   View   Help                         │
└────────────────────────────────────────────────────────────────┘
```

### File Menu
- **Open ROM...** (Ctrl+O) - Open a ROM file
- **Open Project...** (Ctrl+Shift+O) - Open a .hemu project file
- **Save Project...** (Ctrl+S) - Save current configuration as .hemu project
- **Mount Points...** - Manage disk/cartridge mounts
- **Exit** (Esc) - Exit the emulator

### Emulation Menu
- **Reset** (Ctrl+R) - Reset the emulated system
- **Pause/Resume** (Ctrl+P) - Pause or resume emulation
- **Speed: 25%** - Set emulation speed to 25%
- **Speed: 50%** - Set emulation speed to 50%
- **Speed: 100%** - Set emulation speed to 100%
- **Speed: 200%** - Set emulation speed to 200%
- **Speed: 400%** - Set emulation speed to 400%

### State Menu
- **Save State Slot 1** (Ctrl+1)
- **Save State Slot 2** (Ctrl+2)
- **Save State Slot 3** (Ctrl+3)
- **Save State Slot 4** (Ctrl+4)
- **Save State Slot 5** (Ctrl+5)
- **Load State Slot 1** (Ctrl+Shift+1)
- **Load State Slot 2** (Ctrl+Shift+2)
- **Load State Slot 3** (Ctrl+Shift+3)
- **Load State Slot 4** (Ctrl+Shift+4)
- **Load State Slot 5** (Ctrl+Shift+5)

### View Menu
- **Take Screenshot** (F4) - Capture current frame to PNG
- **Debug Info** (F10) - Toggle debug overlay
- **CRT Filter** (F11) - Cycle through CRT filter options

### Help Menu
- **Help** (F1) - Show help overlay
- **About** - Show about information

## Status Bar (Bottom)

```
┌────────────────────────────────────────────────────────────────┐
│ NES [100%]          Status: System reset              60.0 FPS │
└────────────────────────────────────────────────────────────────┘
```

**Layout:**
- **Left:** System name and state ([PAUSED] or [XX%] speed indicator)
- **Center:** Status messages (e.g., "State saved to slot 1", "ROM loaded")
- **Right:** Current FPS counter

## Colors
- Menu bar background: #2A2A3E (dark gray/purple)
- Menu text: #FFFFFF (white)
- Menu highlight: #16F2B3 (cyan/green)
- Status bar background: #2A2A3E (dark gray/purple)
- Status text: #FFFFFF (white)
- Status messages: #16F2B3 (cyan/green)

## Keyboard Shortcuts Summary

### File Operations
- **Ctrl+O** - Open ROM
- **Ctrl+Shift+O** - Open Project
- **Ctrl+S** - Save Project

### Emulation Control
- **Ctrl+R** - Reset
- **Ctrl+P** - Pause/Resume
- **Esc** - Exit (or close overlay)

### Save States
- **Ctrl+1-5** - Save to slot 1-5
- **Ctrl+Shift+1-5** - Load from slot 1-5

### View
- **F4** - Screenshot
- **F10** - Debug Info
- **F11** - CRT Filter
- **F1** - Help

## Migration from F1-F12

The following F-key shortcuts have been replaced with new shortcuts:

| Old Shortcut | New Shortcut | Action |
|--------------|--------------|--------|
| F2 | Emulation menu | Speed selector |
| F3 | Ctrl+O or File menu | Open ROM |
| F5-F9 | Ctrl+1-5 | Save state slots 1-5 |
| Shift+F5-F9 | Ctrl+Shift+1-5 | Load state slots 1-5 |
| F7 | Ctrl+Shift+O | Open project |
| F8 | Ctrl+S | Save project |
| F12 | Ctrl+R | Reset |

**Retained F-keys (for convenience):**
- **F1** - Help
- **F4** - Screenshot
- **F10** - Debug info
- **F11** - CRT filter

## Notes
- The menu bar is always visible and rendered on top of the game display
- The status bar is always visible and rendered at the bottom
- Menu dropdowns appear when clicking on menu items
- Keyboard shortcuts work immediately and don't require menu interaction
- All overlays (help, debug, slot selector, etc.) are rendered on top of everything
