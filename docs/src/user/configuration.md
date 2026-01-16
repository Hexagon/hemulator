---
title: "Configuration"
parent: "User Manual"
nav_order: 3
---

# Configuration

## Inspector Panel

The Inspector is a dockable panel at the bottom of the window that provides debugging tools and system information. Toggle it with **View → Inspector** from the menu.

The Inspector provides several tabs with debugging information:

**Generic Tabs** (available for all systems):
- **Log**: Live message capture with level controls (error, warn, info, debug, trace)
- **Debug**: CPU state, memory viewer, disassembly (3-panel comprehensive view)
- **Memory**: Memory regions and hex dump viewer

**System-Specific Tabs** (automatically shown based on loaded ROM):
- **NES**: Tiles, Palettes, Nametables
- **PC**: BDA/EBDA (BIOS Data Area)
- Additional tabs appear for other systems as appropriate

The Inspector helps with troubleshooting compatibility issues, understanding ROM specifications, and debugging emulation behavior.

### Settings File (`config.json`)

Located in the same directory as the executable, this file stores your preferences:

```json
{
  "input": {
    "player1": {
      "a": "Z",
      "b": "X",
      "x": "A",
      "y": "S",
      "l": "Q",
      "r": "W",
      "select": "LeftShift",
      "start": "Enter",
      "up": "Up",
      "down": "Down",
      "left": "Left",
      "right": "Right"
    },
    "player2": {
      "a": "U",
      "b": "O",
      "x": "Y",
      "y": "H",
      "l": "T",
      "r": "R",
      "select": "RightShift",
      "start": "P",
      "up": "I",
      "down": "K",
      "left": "J",
      "right": "L"
    },
    "player3": {
      "a": "",
      "b": "",
      ...
    },
    "player4": {
      "a": "",
      "b": "",
      ...
    },
    "host_modifier": "RightCtrl",
    "mouse_enabled": false,
    "mouse_sensitivity": 1.0
  },
  "window_width": 512,
  "window_height": 480,
  "display_filter": "None",
  "video_backend": "software"
}
```

**Customization**: 
- Edit this file to change key bindings for any player
- Set `mouse_enabled` to `true` to enable mouse input (for compatible systems)
- Adjust `mouse_sensitivity` to control mouse movement speed (default: 1.0)
- Change `video_backend` to `"opengl"` for hardware-accelerated rendering
- Set `display_filter` to `"Scanlines"`, `"Phosphor"`, or `"CRTMonitor"` for visual effects

**Default Button Mappings**:

**Player 1** (designed for right-handed play with arrow keys):
- **Action buttons**: Z (A), X (B), A (X), S (Y)
- **Shoulder buttons**: Q (L), W (R)
- **D-pad**: Arrow keys
- **System buttons**: Left Shift (Select), Enter (Start)

**Player 2** (designed for two-player games):
- **Action buttons**: U (A), O (B), Y (X), H (Y)
- **Shoulder buttons**: T (L), R (R)
- **D-pad**: I (Up), K (Down), J (Left), L (Right)
- **System buttons**: Right Shift (Select), P (Start)

These mappings work for all systems:
- **NES/Game Boy**: Uses A, B, Select, Start, and D-pad
- **SNES**: Uses all buttons including X, Y, L, R
- **Other systems**: May use subsets of these buttons

**Advanced Input Configuration with Controller Profiles**:

For advanced users, you can define reusable controller profiles that map physical gamepad/joystick inputs to virtual buttons. This allows sharing button mappings between systems and per-game customization.

Example with gamepad profile:

```json
{
  "input": {
    "player1": { /* keyboard mappings as above */ },
    "player2": { /* keyboard mappings as above */ },
    "player3": { "a": "", "b": "", ... },
    "player4": { "a": "", "b": "", ... },
    "host_modifier": "RightCtrl",
    "mouse_enabled": false,
    "mouse_sensitivity": 1.0,
    "profiles": [
      {
        "name": "My Xbox Controller",
        "device_type": "Gamepad",
        "mappings": {
          "A": { "GamepadButton": 0 },
          "B": { "GamepadButton": 1 },
          "X": { "GamepadButton": 2 },
          "Y": { "GamepadButton": 3 },
          "L": { "GamepadButton": 4 },
          "R": { "GamepadButton": 5 },
          "Select": { "GamepadButton": 6 },
          "Start": { "GamepadButton": 7 },
          "Up": { "GamepadAxis": { "axis": 1, "direction": -1 } },
          "Down": { "GamepadAxis": { "axis": 1, "direction": 1 } },
          "Left": { "GamepadAxis": { "axis": 0, "direction": -1 } },
          "Right": { "GamepadAxis": { "axis": 0, "direction": 1 } }
        }
      }
    ]
  },
  "window_width": 512,
  "window_height": 480,
  "display_filter": "None",
  "video_backend": "software"
}
```

**Input Source Types**:
- `KeyboardKey`: String name of a keyboard key (e.g., `"Z"`, `"Enter"`, `"LeftShift"`)
- `MouseButton`: Mouse button number (0 = left, 1 = middle, 2 = right)
- `GamepadButton`: SDL2 GameController button ID (0-15)
- `GamepadAxis`: Gamepad axis with direction (`"axis": 0-5`, `"direction": -1 or 1`)
- `JoystickButton`: Joystick button ID
- `JoystickAxis`: Joystick axis with direction
- `JoystickHat`: Joystick hat switch with direction (1=up, 2=right, 4=down, 8=left)

**When profiles are defined**, they only affect inputs for their own device type (e.g., Gamepad vs Keyboard). For any logical button where both a simple mapping and a profile mapping exist for the same device type, the profile mapping is used and the simple mapping is ignored. Keyboard mappings remain active even when you add a gamepad (or other) profile; profiles add support for additional devices rather than disabling keyboard controls.
- Empty strings ("") mean that button is unmapped
- The `x`, `y`, `l`, and `r` buttons are used for SNES controllers and may also be reused by other systems
- The `host_modifier` key (default: "RightCtrl") controls when function keys are passed to the emulator vs the PC system
- The window size is automatically saved when you resize the window
- CRT filter preference is saved automatically when you cycle filters with F11
- Emulation speed is saved automatically when you change it with F2
- Valid `crt_filter` values: "None", "Scanlines", "Phosphor", "CrtMonitor"
- Valid `emulation_speed` values: 0.0 (pause), 0.25, 0.5, 1.0, 2.0, 10.0 (or any positive number)

**Valid Key Names**: 
A-Z, Space, Enter, LeftShift, RightShift, LeftCtrl, RightCtrl, Up, Down, Left, Right, LeftBracket, RightBracket

**Backward Compatibility**: If you have an old `config.json` with a `keyboard` field instead of `input`, it will be automatically migrated to `input.player1` on first load.

### Mount Points and Project Files

**Mount Points**: The emulator supports multiple media slots per system. Each system defines mount points (e.g., NES has "Cartridge", PC has "BIOS", "FloppyA", "FloppyB", "HardDrive"). 

When you press **F3** (Select Mount Points):
- A mount point selector always appears showing all available slots for the current system
- Select a slot (1-9) to open a file browser for that mount point
- Even single-mount systems (NES, Game Boy) show the selector for consistency

**Project Files (.hemu)**: Project files save your complete setup including mounts, display settings, and input configuration:
- **F7** (Load Project): Load a `.hemu` project file
  - Restores all mount points
  - Applies display settings (window size, CRT filter)
  - Can override input key mappings per-project
  - Works for all systems (NES, PC, Game Boy, etc.)
- **F8** (Save Project): Save current configuration to `.hemu` file
  - Saves only relevant mount points for the system
  - Saves current window size and CRT filter settings
  - Can be used by all systems, not just PC
  - File paths in project are relative to the `.hemu` file location

**Configuration Files**:
- `config.json`: Global settings (window size, input mappings, video backend) - mount points no longer saved here
- `.hemu` files: Per-project settings (mounts, display overrides, system-specific config)
- Runtime settings (emulation speed) are not persisted to any file

### Save States

Save states are stored in `saves/<rom_hash>/states.json`:
- Each game gets its own directory based on ROM hash
- 5 slots available per game
- **F5** opens the save slot selector - press 1-5 to select a slot (only for console systems)
- **F6** opens the load slot selector - press 1-5 to select a slot (shows which slots have saves)
- States are portable and can be backed up or transferred between systems
- **Important**: Save states do NOT include ROM/cartridge data - they only save emulator state
- The emulator verifies that the correct ROM is loaded before allowing state load
- If you try to load a state with a different ROM mounted, you'll get an error

**Save State Support by System**:
- **NES**: Fully supported - save and load states with F5-F6 when a cartridge is loaded
- **Atari 2600**: Fully supported - save and load states with F5-F6
- **Game Boy**: Fully supported - save and load states with F5-F6
- **PC/DOS**: Not supported - PC systems use **Project files** (.hemu) instead
  - **F8** saves the current VM configuration to a `.hemu` project file
  - **F7** loads a `.hemu` project file to restore all settings
  - VM files include all mounted disk images, BIOS, and boot priority settings
  - Disk state is preserved in the disk image files themselves (as in a real PC)
  - This approach matches how real PCs work - state persists on disks, not in memory snapshots

Example structure:
```
saves/
  ├── a1b2c3d4.../  (ROM hash)
  │   └── states.json
  └── e5f6g7h8.../
      └── states.json
```

