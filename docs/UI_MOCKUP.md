# UI Visual Mockup

This document shows a visual representation of the new hemulator UI with menu bar and status bar.

## Full Window Layout (512x480 pixels default)

```
┌──────────────────────────────────────────────────────────────────┐
│ File   Emulation   State   View   Help                           │ ← Menu Bar (24px)
├──────────────────────────────────────────────────────────────────┤
│                                                                   │
│                                                                   │
│                                                                   │
│                                                                   │
│                                                                   │
│                                                                   │
│                    EMULATED SYSTEM DISPLAY                        │
│                      (Game/System Output)                         │ ← Game Display
│                         256x240 native                            │   (Variable height)
│                    Scaled to fit window                           │
│                                                                   │
│                                                                   │
│                                                                   │
│                                                                   │
│                                                                   │
│                                                                   │
│                                                                   │
├──────────────────────────────────────────────────────────────────┤
│ NES                                                       60.1 FPS│ ← Status Bar (20px)
└──────────────────────────────────────────────────────────────────┘
```

## Menu Bar Expanded (File Menu Open)

```
┌──────────────────────────────────────────────────────────────────┐
│ File   Emulation   State   View   Help                           │
├────────────────────┐                                              │
│ Open ROM...        │ (Ctrl+O)                                     │
│ Open Project...    │ (Ctrl+Shift+O)                               │
│ Save Project...    │ (Ctrl+S)                                     │
│ ─────────────────  │                                              │
│ Mount Points...    │                                              │
│ ─────────────────  │                                              │
│ Exit               │ (Esc)                                        │
└────────────────────┘                                              │
│                                                                   │
│                    EMULATED SYSTEM DISPLAY                        │
│                                                                   │
│                                                                   │
├──────────────────────────────────────────────────────────────────┤
│ NES                                                       60.1 FPS│
└──────────────────────────────────────────────────────────────────┘
```

## Status Bar States

### Normal State
```
┌──────────────────────────────────────────────────────────────────┐
│ NES                                                       60.1 FPS│
└──────────────────────────────────────────────────────────────────┘
```

### With Message
```
┌──────────────────────────────────────────────────────────────────┐
│ NES           State saved to slot 1                       60.1 FPS│
└──────────────────────────────────────────────────────────────────┘
```

### Paused State
```
┌──────────────────────────────────────────────────────────────────┐
│ NES [PAUSED]                                               0.0 FPS│
└──────────────────────────────────────────────────────────────────┘
```

### Speed Modified
```
┌──────────────────────────────────────────────────────────────────┐
│ NES [200%]                                               120.2 FPS│
└──────────────────────────────────────────────────────────────────┘
```

## Menu Structure Detail

### File Menu
```
Open ROM...          Ctrl+O
Open Project...      Ctrl+Shift+O
Save Project...      Ctrl+S
─────────────────
Mount Points...
─────────────────
Exit                 Esc
```

### Emulation Menu
```
Reset                Ctrl+R
Pause/Resume         Ctrl+P
─────────────────
Speed: 25%
Speed: 50%
Speed: 100%
Speed: 200%
Speed: 400%
```

### State Menu
```
Save State Slot 1    Ctrl+1
Save State Slot 2    Ctrl+2
Save State Slot 3    Ctrl+3
Save State Slot 4    Ctrl+4
Save State Slot 5    Ctrl+5
Load State Slot 1    Ctrl+Shift+1
Load State Slot 2    Ctrl+Shift+2
Load State Slot 3    Ctrl+Shift+3
Load State Slot 4    Ctrl+Shift+4
Load State Slot 5    Ctrl+Shift+5
```

### View Menu
```
Take Screenshot      F4
Debug Info           F10
CRT Filter           F11
```

### Help Menu
```
Help                 F1
About
```

## Color Scheme

- **Menu Bar Background**: #2A2A3E (Dark gray/purple)
- **Menu Text**: #FFFFFF (White)
- **Menu Highlighted Item**: #16F2B3 (Cyan/green accent)
- **Menu Dropdown Background**: #1A1A2E (Darker purple)
- **Shortcut Text**: #888888 (Gray)
- **Status Bar Background**: #2A2A3E (Dark gray/purple)
- **Status Bar Text**: #FFFFFF (White)
- **Status Bar Messages**: #16F2B3 (Cyan/green accent)

## Interactive Elements

### Menu Bar Interaction
1. Click on menu item to open dropdown
2. Click again to close
3. Click on menu item in dropdown to execute action
4. Keyboard shortcuts work at any time (without opening menu)

### Status Bar
- Read-only display
- Updates in real-time:
  - FPS counter refreshes every frame
  - Messages appear when actions occur
  - System name shows current emulated system
  - Pause/Speed indicators update when changed

## Responsive Behavior

- Menu bar stays at top (fixed height: 24px)
- Status bar stays at bottom (fixed height: 20px)
- Game display area adjusts to window size
- All elements scale proportionally with window resize
- Minimum window size enforces readable text

## Accessibility

- Clear contrast ratios for text readability
- Keyboard shortcuts for all menu actions
- Consistent layout and positioning
- Visual feedback for state changes
- Help screen accessible via F1

## Future Enhancements

- [ ] Checkmarks for toggleable options (CRT filter, pause state)
- [ ] Submenu indicators (arrows for nested menus)
- [ ] Theme customization options
