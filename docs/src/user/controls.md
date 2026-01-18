---
title: "Controls"
parent: "User Manual"
nav_order: 2
---

# Controls

### Multi-Player Support

Hemulator supports up to 4 players for systems that support multiple controllers. By default, Player 1 and Player 2 are mapped to the keyboard.

#### Player 1 Controller (Default Mapping)

| Key | Action | Notes |
|-----|--------|-------|
| Arrow Keys | D-pad | Up/Down/Left/Right |
| Z | A button | Confirm/Jump |
| X | B button | Back/Action |
| A | X button | SNES X button |
| S | Y button | SNES Y button |
| Q | L button | SNES L shoulder |
| W | R button | SNES R shoulder |
| Enter | Start | Pause menu |
| Left Shift | Select | Menu navigation |

#### Player 2 Controller (Default Mapping)

| Key | Action | Notes |
|-----|--------|-------|
| I/J/K/L | D-pad | I=Up, K=Down, J=Left, L=Right |
| U | A button | Confirm/Jump |
| O | B button | Back/Action |
| Y | X button | SNES X button |
| H | Y button | SNES Y button |
| T | L button | SNES L shoulder |
| R | R button | SNES R shoulder |
| P | Start | Pause menu |
| Right Shift | Select | Menu navigation |

*All controller mappings for all players can be customized by editing `config.json`*

**Note**: All Player 1 keys are on the left side of the keyboard, and all Player 2 keys are on the right side for comfortable simultaneous play. Players 3 and 4 are not mapped by default but can be configured in `config.json` for systems that support 4 players (future SNES support, etc.).

### Gamepad and Joystick Support

**✅ Now Available!** Physical USB gamepads and joysticks are automatically detected and can be used to control games. The emulator supports:
- **Game Controllers**: Xbox, PlayStation, and other controllers using the SDL2 GameController API
- **Generic Joysticks**: Any USB joystick with buttons, axes, and hat switches
- **Automatic Detection**: Controllers are detected when plugged in
- **Customizable Mappings**: Button mappings can be customized in `config.json` using controller profiles

To use a gamepad:
1. Plug in your USB controller before starting the emulator
2. The emulator will automatically detect it and print a message like "Opened game controller: Xbox Controller"
3. Use the default button mappings or customize them in `config.json` (see Configuration section below)

**Default Gamepad Mapping**:
- **A/Cross** → A button
- **B/Circle** → B button  
- **X/Square** → X button (SNES)
- **Y/Triangle** → Y button (SNES)
- **L1/LB** → L shoulder (SNES)
- **R1/RB** → R shoulder (SNES)
- **Back/Select** → Select
- **Start** → Start
- **Left Stick/D-Pad** → Directional controls

**Mouse Support**: Mouse input is available for systems that support it. Enable mouse input in `config.json` with `"mouse_enabled": true` and adjust sensitivity with `"mouse_sensitivity": 1.0` (default).

### PC/DOS Keyboard Input

When running PC/DOS programs, the emulator provides full keyboard passthrough by default. This means all keyboard keys are sent directly to the emulated PC, allowing you to type and use DOS programs naturally.

#### Host Modifier Key (PC System Only)

To access emulator controls while running a PC program, hold the **Right Ctrl** key (the host modifier) while pressing function keys or using shortcuts. For example:
- **Right Ctrl + F4**: Take screenshot
- **Right Ctrl + Ctrl+P**: Pause/Resume
- **Right Ctrl + Esc**: Exit the emulator

You can also click on the menu bar to access all emulator functions without using the host key.

The host modifier key can be customized in `config.json` by changing the `host_modifier` field (default: `RightCtrl`).

**Without the host modifier**: Function keys are sent to the DOS program
**With the host modifier**: Function keys control the emulator

**Note**: ESC requires the host modifier (Right Ctrl + ESC) to exit the emulator in PC mode.

**Other Systems**: NES, Game Boy, Atari 2600, SNES, and N64 do NOT require the host modifier key for function keys. Press function keys directly to control the emulator.

**Known Limitation**: Some host key + key combinations may be intercepted by your operating system before reaching the emulator. For example, on Windows, Left Ctrl + ESC opens the Start menu and cannot be captured. If you experience issues with your chosen host modifier key:
- Try using `RightCtrl` instead of `LeftCtrl` (default setting)
- Or choose a different modifier key in `config.json` (e.g., `RightAlt`)
- Some OS-level keyboard shortcuts cannot be overridden by applications

### ColecoVision Controller Input

The ColecoVision has a unique controller with:
- **Joystick**: 4 directions (up, down, left, right)
- **Two fire buttons**: Side buttons (A and B)
- **12-key numeric keypad**: Numbers 0-9 plus * and # keys

The emulator maps these controls to the keyboard for both Player 1 and Player 2.

#### Player 1 ColecoVision Controller

| Key | Action | Notes |
|-----|--------|-------|
| Arrow Keys | Joystick | Up/Down/Left/Right |
| Left Shift | Fire Button A | Left side fire button |
| Enter | Fire Button B | Right side fire button |
| **Numeric Keypad** | | |
| 1 | Key 1 | Top row |
| 2 | Key 2 | Top row |
| 3 | Key 3 | Top row |
| Q | Key 4 | Second row |
| W | Key 5 | Second row |
| E | Key 6 | Second row |
| A | Key 7 | Third row |
| S | Key 8 | Third row |
| D | Key 9 | Third row |
| Z | Key * | Bottom row |
| X | Key 0 | Bottom row |
| C | Key # | Bottom row |

#### Player 2 ColecoVision Controller

| Key | Action | Notes |
|-----|--------|-------|
| I/J/K/L | Joystick | I=Up, K=Down, J=Left, L=Right |
| Right Shift | Fire Button A | Left side fire button |
| P | Fire Button B | Right side fire button |
| **Numeric Keypad** | | |
| 7 | Key 1 | Top row |
| 8 | Key 2 | Top row |
| 9 | Key 3 | Top row |
| U | Key 4 | Second row |
| I | Key 5 | Second row (same as Up) |
| O | Key 6 | Second row |
| H | Key 7 | Third row |
| J | Key 8 | Third row (same as Left) |
| K | Key 9 | Third row (same as Down) |
| N | Key * | Bottom row |
| M | Key 0 | Bottom row |
| Comma (,) | Key # | Bottom row |

**Note**: The ColecoVision keypad mappings are designed to mirror the physical layout of the ColecoVision controller's numeric keypad, with keys arranged in a 3x4 grid. Some Player 2 keys overlap with joystick directions (I, J, K) - games typically don't use keypad and joystick simultaneously.

**Technical Note**: The ColecoVision hardware reads the joystick/fire buttons and the numeric keypad separately. The current implementation focuses on the joystick and fire buttons. Full keypad support for all 12 keys requires additional implementation in the emulation core.

### User Interface

The emulator features a menu bar at the top and a status bar at the bottom of the window:

- **Menu Bar** (top, 24px): Access all emulator functions via clickable menus (File, Emulation, State, View, Help)
- **Status Bar** (bottom, 20px): Displays system name, pause/speed state, status messages, FPS, instruction pointer (IP), and CPU cycles

#### Menus

**File Menu:**
- Open ROM... (Ctrl+O) - Load a ROM file
- Open Project... (Ctrl+Shift+O) - Load a .hemu project file
- Save Project... (Ctrl+S) - Save current configuration as .hemu project
- Mount Points... - Manage disk/cartridge mounts (PC system)
- Exit (Esc) - Exit the emulator

**Emulation Menu:**
- Reset (Ctrl+R) - Reset the emulated system
- Pause (Ctrl+P) - Pause emulation
- Resume (Ctrl+P) - Resume emulation
- Speed options: 25%, 50%, 100%, 200%, 400%

**State Menu:**
- Save State Slot 1-5 (Ctrl+1-5) - Save state to slots
- Load State Slot 1-5 (Ctrl+Shift+1-5) - Load state from slots

**View Menu:**
- Take Screenshot (F4) - Capture current frame to PNG
- Inspector - Toggle Inspector panel with debugging tools
- CRT Filter (F11) - Cycle through CRT filter options
- Start/Stop Logging - Enable/disable debug logging to log.txt

**Help Menu:**
- Help (F1) - Show help overlay
- About - Display version info

#### Keyboard Shortcuts

Modern keyboard shortcuts for common actions:

| Shortcut | Action |
|----------|--------|
| Ctrl+O | Open ROM |
| Ctrl+Shift+O | Open Project |
| Ctrl+S | Save Project |
| Ctrl+R | Reset System |
| Ctrl+P | Pause/Resume (toggle) |
| Ctrl+1-5 | Save State (slots 1-5) |
| Ctrl+Shift+1-5 | Load State (slots 1-5) |
| F1 | Help Overlay |
| F4 | Screenshot |
| F11 | CRT Filter |
| Esc | Exit/Close |

**Note on Host Key (PC System Only)**: When running PC/DOS programs, you must hold **Right Ctrl** (or your configured host modifier key) while pressing function keys. This allows function keys to pass through to the DOS program when the host key is not held. Other systems (NES, Game Boy, etc.) do not require the host key for function keys.

### Emulation Speed Control

Access speed control through the **Emulation** menu or press **Ctrl+P** to pause/resume.

Available speed options in the Emulation menu:
- **25%**: Quarter speed for analyzing difficult sections
- **50%**: Half speed for practicing tricky maneuvers
- **100%**: Default speed (typically ~60 FPS for NTSC, ~50 FPS for PAL)
- **200%**: Double speed for skipping slow parts
- **400%**: Ultra fast for grinding or replaying sections quickly

The selected speed is automatically saved and restored when you restart the emulator.

### CRT Filters (F11)

Press **F11** to cycle through different CRT (Cathode Ray Tube) display filters that simulate the appearance of classic CRT monitors and TVs. These filters add visual effects to make the emulator output look more authentic to the original hardware experience.

#### Available Filters

**1. None (Default)**
- **Description**: Raw pixel output with no processing
- **Use Case**: When you want sharp, unfiltered pixels
- **Performance**: No overhead

**2. Scanlines**
- **Description**: Simulates the horizontal raster scan lines visible on CRT displays
- **Implementation**: 
  - Darkens every other horizontal line (odd rows)
  - Reduces brightness to 60% on affected rows
  - Even rows remain at full brightness
- **Visual Effect**: Creates horizontal dark lines across the screen
- **Use Case**: For a classic CRT TV look
- **Example**:
  ```
  Row 0: ████████████████ (Full brightness)
  Row 1: ▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓ (60% brightness - scanline)
  Row 2: ████████████████ (Full brightness)
  Row 3: ▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓ (60% brightness - scanline)
  ```

**3. Phosphor**
- **Description**: Simulates the phosphor glow and color bleeding of CRT screens
- **Implementation**:
  - Blends each pixel with its horizontal neighbors
  - 15% blend ratio with left neighbor (if exists)
  - 15% blend ratio with right neighbor (if exists)
  - Creates soft horizontal glow
- **Visual Effect**: Softens edges and creates a subtle glow between pixels
- **Use Case**: For a softer, more authentic CRT appearance without harsh scanlines
- **Example**: Sharp edges become blurred horizontally, colors bleed slightly into adjacent pixels

**4. CRT Monitor (Full Effect)**
- **Description**: Combines multiple CRT characteristics for the most authentic look
- **Implementation**:
  1. First applies phosphor effect (horizontal color bleeding)
  2. Then applies scanlines with 70% darkness (less aggressive than scanlines-only)
  3. Boosts brightness on non-scanline rows by 5% for contrast
- **Visual Effect**: 
  - Horizontal color bleeding from phosphor
  - Visible but not harsh scanlines
  - Enhanced contrast between scanlines and active rows
- **Use Case**: For the most authentic CRT monitor simulation
- **Performance**: Most intensive filter (processes buffer twice)

#### Technical Details

**Color Processing**:
- All filters work in RGB color space (0xRRGGBB format)
- Filters use floating-point math for blending, then convert back to u8
- Uses `saturating_add` for brightness adjustments to prevent overflow

**Performance Characteristics**:
- **None**: Zero overhead (no processing)
- **Scanlines**: O(n) single pass, simple arithmetic
- **Phosphor**: O(n) single pass, with neighbor lookups and blending
- **CRT Monitor**: O(2n) two passes (phosphor + enhanced scanlines)

Where n = width × height (typically 256 × 240 = 61,440 pixels for NES)

**Filter Application**:
- Filters are applied after frame rendering but before display
- Filters do NOT affect overlays (help, debug, slot selector)
- Filters modify the buffer in-place for efficiency
- Selected filter persists across sessions via config.json

The selected filter is automatically saved and restored when you restart the emulator.

### Video Processing Backends

Hemulator supports two video processing backends that can be selected based on your system capabilities:

#### Software Renderer (Default)
- **Description**: CPU-based rendering using traditional software algorithms
- **Availability**: Always available, no GPU required
- **CRT Filters**: Implemented in software, applied to frame buffer in memory
- **Performance**: Suitable for all systems, including those without GPU acceleration
- **Compatibility**: Maximum compatibility across all platforms
- **Configuration**: Set `"video_backend": "software"` in `config.json`

#### OpenGL Renderer (Optional)
- **Description**: GPU-accelerated rendering using OpenGL and GLSL shaders
- **Availability**: Only available in builds compiled with `--features opengl`
- **CRT Filters**: Implemented as GLSL shaders, executed on GPU
- **Performance**: Better performance on systems with capable GPUs, especially for high-resolution displays
- **Shader Effects**:
  - All CRT filters (None, Scanlines, Phosphor, CRT Monitor) implemented as fragment shaders
  - Dynamic shader compilation based on selected filter
  - Real-time switching without restart
- **Configuration**: Set `"video_backend": "opengl"` in `config.json`
- **Requirements**: 
  - OpenGL 3.3+ compatible GPU
  - Proper graphics drivers installed
  - Build compiled with OpenGL support

#### Choosing a Backend

**Use Software Renderer if:**
- You want maximum compatibility
- Your system doesn't have a GPU or has limited GPU support
- You're running on older hardware
- You're experiencing issues with OpenGL drivers

**Use OpenGL Renderer if:**
- Your system has a capable GPU
- You want better performance, especially at higher resolutions
- You're interested in future shader-based enhancements
- Your build includes OpenGL support

**Switching Backends:**
1. Exit the emulator
2. Open `config.json` in a text editor
3. Change `"video_backend"` to either `"software"` or `"opengl"`
4. Save the file
5. Restart the emulator

The video backend setting is independent of the CRT filter selection - all filters work with both backends, though the OpenGL backend implements them as shaders for better performance.

### Screenshots (F4)

Press **F4** at any time to capture the current frame and save it as a PNG image.

Screenshots are automatically saved to:
```
screenshots/<system-name>/YYYYMMDDHHMMSSRRR.png
```

Where:
- `<system-name>` is the emulated system (e.g., `nes`, `atari2600`, `gameboy`, `pc`)
- `YYYYMMDDHHMMSS` is the current date and time (Year, Month, Day, Hour, Minute, Second)
- `RRR` is a random 3-digit number (000-999) to prevent filename collisions

**Examples:**
- `screenshots/nes/20231215143022456.png` - NES screenshot from Dec 15, 2023 at 2:30:22 PM
- `screenshots/atari2600/20231215143025789.png` - Atari 2600 screenshot from Dec 15, 2023 at 2:30:25 PM

The `screenshots` directory will be created automatically in the same folder as the emulator executable.

### Window Management

The emulator window can be resized freely by dragging the window edges or maximizing the window. The window maintains the correct aspect ratio while stretching to fill the available space. The window size is automatically remembered between sessions.

