# Hemulator User Manual

Welcome to Hemulator, a cross-platform multi-system console emulator supporting NES, SNES, N64, Atari 2600, Game Boy, and PC/DOS emulation.

## Getting Started

### First Run

1. **Launch the emulator**: Double-click `hemu` (or `hemu.exe` on Windows)
2. **The splash screen appears** with instructions
3. **Load a ROM**: Press `F3` to open the file browser
4. **Select your game file**:
   - `.nes` for NES
   - `.smc`/`.sfc` for SNES
   - `.z64`/`.n64`/`.v64` for N64
   - `.a26`/`.bin` for Atari 2600
   - `.gb`/`.gbc` for Game Boy
   - `.com`/`.exe` for PC/DOS
5. **Start playing!** Use the controls listed below

Alternatively, you can provide a ROM path as an argument:
```bash
./hemu path/to/your/game.nes
```

The emulator will remember your last ROM and automatically load it next time you start.

## Controls

### Multi-Player Support

Hemulator supports up to 4 players for systems that support multiple controllers. By default, Player 1 and Player 2 are mapped to the keyboard.

#### Player 1 Controller (Default Mapping)

| Key | Action | Notes |
|-----|--------|-------|
| Arrow Keys | D-pad | Up/Down/Left/Right |
| Z | A button | Confirm/Jump |
| X | B button | Back/Action |
| Enter | Start | Pause menu |
| Left Shift | Select | Menu navigation |

#### Player 2 Controller (Default Mapping)

| Key | Action | Notes |
|-----|--------|-------|
| I/J/K/L | D-pad | I=Up, K=Down, J=Left, L=Right |
| U | A button | Confirm/Jump |
| O | B button | Back/Action |
| P | Start | Pause menu |
| Right Shift | Select | Menu navigation |

*All controller mappings for all players can be customized by editing `config.json`*

**Note**: All Player 1 keys are on the left side of the keyboard, and all Player 2 keys are on the right side for comfortable simultaneous play. Players 3 and 4 are not mapped by default but can be configured in `config.json` for systems that support 4 players (future SNES support, etc.).

### Future Enhancements

**Joystick/Gamepad Support**: Physical USB joysticks and gamepads are planned for future releases. When implemented, joysticks will be automatically mapped to D-pad and button controls, with the ability to customize mappings in `config.json`. Until then, keyboard controls provide full functionality for all supported systems.

### PC/DOS Keyboard Input

When running PC/DOS programs, the emulator provides full keyboard passthrough by default. This means all keyboard keys are sent directly to the emulated PC, allowing you to type and use DOS programs naturally.

#### Host Modifier Key

To access function keys (F1-F12) for emulator controls while running a PC program, hold the **Right Ctrl** key (the host modifier) while pressing the function key. For example:
- **Right Ctrl + F3**: Open ROM/executable file dialog
- **Right Ctrl + F4**: Take screenshot
- **Right Ctrl + F5**: Save state

The host modifier key can be customized in `config.json` by changing the `host_modifier` field (default: `RightCtrl`).

**Without the host modifier**: Function keys are sent to the DOS program
**With the host modifier**: Function keys control the emulator

**Note**: ESC always exits the emulator, even in PC mode.

### Function Keys

| Key | Action | Description |
|-----|--------|-------------|
| F1 | Help Overlay | Show/hide all controls and key mappings |
| F2 | Speed Selector | Open speed selector menu (pause, 0.25x, 0.5x, 1x, 2x, 10x) |
| F3 | Load Media | Open mount point selector (if system has multiple slots) or file browser directly |
| F4 | Screenshot | Save screenshot to `screenshots/<system-name>/YYYYMMDDHHMMSSRRR.png` |
| F5 | Save State | Open slot selector (1-5) to save |
| F6 | Load State | Open slot selector (1-5) to load |
| F10 | Debug Info | Show/hide debug information overlay |
| F11 | CRT Filter | Cycle through CRT display filters |
| F12 | Reset System | Restart the current game |

### Emulation Speed Control (F2)

Press **F2** to open the speed selector menu. The game will pause while the menu is visible.

Available speed options:
- **0 - Pause (0x)**: Completely pause emulation (useful for studying frame-by-frame)
- **1 - Slow Motion (0.25x)**: Quarter speed for analyzing difficult sections
- **2 - Half Speed (0.5x)**: Half speed for practicing tricky maneuvers
- **3 - Normal (1x)**: Default speed (typically ~60 FPS for NTSC, ~50 FPS for PAL)
- **4 - Fast Forward (2x)**: Double speed for skipping slow parts
- **5 - Turbo (10x)**: Ultra fast for grinding or replaying sections quickly

The selected speed is automatically saved and restored when you restart the emulator. Press **0-5** to select a speed, or **ESC** to cancel.

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

## Configuration

## Debug Information (F10)

When a ROM is loaded, press **F10** to display the debug information overlay.

**For NES games**, this shows:
- **Mapper**: The cartridge mapper number and name
- **PRG**: Number of PRG ROM banks (16KB each)
- **CHR**: Number of CHR ROM banks (8KB each) or "RAM" if using CHR-RAM
- **Timing**: NTSC or PAL timing mode (auto-detected from ROM header)
- **FPS**: Current frame rate

**For Atari 2600 games**, debug information is currently limited. Future versions will show cartridge banking information.

This information is useful for troubleshooting compatibility issues or understanding ROM specifications.

### Settings File (`config.json`)

Located in the same directory as the executable, this file stores your preferences:

```json
{
  "input": {
    "player1": {
      "a": "Z",
      "b": "X",
      "x": "",
      "y": "",
      "l": "",
      "r": "",
      "select": "LeftShift",
      "start": "Enter",
      "up": "Up",
      "down": "Down",
      "left": "Left",
      "right": "Right"
    },
    "player2": {
      "a": "U",
      "b": "I",
      "x": "",
      "y": "",
      "l": "",
      "r": "",
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
    "host_modifier": "RightCtrl"
  },
  "window_width": 512,
  "window_height": 480,
  "mount_points": {
    "Cartridge": "/path/to/last/rom.nes"
  },
  "crt_filter": "None",
  "emulation_speed": 1.0
}
```

**Customization**: 
- Edit this file to change key bindings for any player
- Empty strings ("") mean that button is unmapped
- The `x`, `y`, `l`, and `r` buttons are for future SNES support and other systems
- The `host_modifier` key (default: "RightCtrl") controls when function keys are passed to the emulator vs the PC system
- The window size is automatically saved when you resize the window
- CRT filter preference is saved automatically when you cycle filters with F11
- Emulation speed is saved automatically when you change it with F2
- Valid `crt_filter` values: "None", "Scanlines", "Phosphor", "CrtMonitor"
- Valid `emulation_speed` values: 0.0 (pause), 0.25, 0.5, 1.0, 2.0, 10.0 (or any positive number)

**Valid Key Names**: 
A-Z, Space, Enter, LeftShift, RightShift, LeftCtrl, RightCtrl, Up, Down, Left, Right, LeftBracket, RightBracket

**Backward Compatibility**: If you have an old `config.json` with a `keyboard` field instead of `input`, it will be automatically migrated to `input.player1` on first load.

**Mount Points**: The emulator now supports multiple media slots per system. Each system defines mount points (e.g., NES has "Cartridge", future systems might have "BIOS", "Floppy1", etc.). When you press F3:
- If the system has only one mount point (like NES), the file browser opens directly
- If the system has multiple mount points, a selector appears first to choose which slot to load media into

### Save States

Save states are stored in `saves/<rom_hash>/states.json`:
- Each game gets its own directory based on ROM hash
- 5 slots available per game
- **F5** opens the save slot selector - press 1-5 to select a slot (only for systems that support save states)
- **F6** opens the load slot selector - press 1-5 to select a slot (shows which slots have saves)
- States are portable and can be backed up or transferred between systems
- **Important**: Save states do NOT include ROM/cartridge data - they only save emulator state
- The emulator verifies that the correct ROM is loaded before allowing state load
- If you try to load a state with a different ROM mounted, you'll get an error

**Save State Support by System**:
- **NES**: Fully supported - save and load states with F5/F6 when a cartridge is loaded
- **Atari 2600**: Fully supported - save and load states with F5/F6
- **Game Boy**: Not yet implemented (skeleton)

Example structure:
```
saves/
  ├── a1b2c3d4.../  (ROM hash)
  │   └── states.json
  └── e5f6g7h8.../
      └── states.json
```

## Supported Systems

### NES (Nintendo Entertainment System)

**Coverage**: ~90% of all NES games (14 mappers supported)

The emulator supports the following NES mappers:
- **Mapper 0 (NROM)** - Simple games (~10% of games)
- **Mapper 1 (MMC1/SxROM)** - Tetris, Metroid, The Legend of Zelda (~28% of games)
- **Mapper 2 (UxROM)** - Mega Man, Castlevania, Contra (~11% of games)
- **Mapper 3 (CNROM)** - Gradius, Paperboy (~6.4% of games)
- **Mapper 4 (MMC3/TxROM)** - Super Mario Bros. 3, Mega Man 3-6 (~24% of games)
- **Mapper 7 (AxROM)** - Battletoads, Marble Madness (~3.1% of games)
- **Mapper 9 (MMC2/PxROM)** - Mike Tyson's Punch-Out!!
- **Mapper 10 (MMC4/FxROM)** - Fire Emblem (Japan)
- **Mapper 11 (Color Dreams)** - Color Dreams and Wisdom Tree games (~1.3% of games)
- **Mapper 34 (BNROM)** - Deadly Towers, homebrew titles
- **Mapper 66 (GxROM)** - SMB + Duck Hunt, Doraemon (~1.2% of games)
- **Mapper 71 (Camerica)** - Fire Hawk, Micro Machines (~0.6% of games)
- **Mapper 79 (NINA-03/06)** - AVE games like Dudes with Attitude, Pyramid
- **Mapper 206 (Namco 118)** - Dragon Spirit, Famista (~1.8% of games)

**ROM Format**: iNES (.nes files) - automatically detected

**Features**:
- Full PPU (video) and APU (audio) emulation
- Save states (F5/F6)
- NTSC and PAL timing modes (auto-detected)
- Controller support with customizable key mappings

### Atari 2600

**Coverage**: Most common cartridge formats (2K, 4K, 8K, 12K, 16K, 32K)

The emulator supports the following cartridge banking schemes:
- **2K ROM** - No banking, simple games like Combat
- **4K ROM** - No banking, common format for early games like Pac-Man
- **8K (F8)** - 2x 4KB banks, games like Asteroids, Missile Command
- **12K (FA)** - 3x 4KB banks, used by some CBS games
- **16K (F6)** - 4x 4KB banks, games like Donkey Kong, Crystal Castles
- **32K (F4)** - 8x 4KB banks, later and larger games

**ROM Format**: Raw binary (.a26, .bin files) - automatically detected by size

**Features**:
- TIA (Television Interface Adapter) video emulation with playfield rendering
- RIOT (6532) chip emulation for RAM, I/O, and timers
- Save states (F5/F6)
- Joystick controls mapped to keyboard (same as NES controls)
- 160x192 resolution

**Known Limitations**:
- **Audio**: Registers stored but waveform synthesis not yet implemented (silent gameplay)
- **Player/Missile Sizing**: NUSIZ registers stored but size/duplication modes not applied
- **Horizontal Motion**: HMxx registers stored but fine positioning not applied
- **Collision Detection**: Registers exist but always return 0
- **Timing Model**: Frame-based rendering (not cycle-accurate) - suitable for most games
- **Banking**: Most common schemes supported; some exotic formats (DPC, FE, 3F, E0) not yet implemented

**Controls**: The Atari 2600 joystick is mapped to the same keyboard layout as NES:
- Arrow keys = Joystick directions
- Z = Fire button
- Enter = Game Reset (console switch)
- Left Shift = Game Select (console switch)

### Game Boy / Game Boy Color

**Status**: Functional implementation with PPU, joypad, and rendering

**Coverage**: Homebrew ROMs and simple games (32KB ROMs, MBC0 only)

**ROM Format**: GB/GBC (.gb, .gbc files) - automatically detected

**Features**:
- Full PPU (Picture Processing Unit) rendering: background, window, sprites
- Resolution: 160x144 pixels (DMG mode)
- Sprite support: 40 sprites with 8x8/8x16 modes, flipping, priority
- Joypad input with matrix selection
- Save states (F5/F6)
- Frame-based timing (~59.73 Hz)

**Known Limitations**:
- **MBC Support**: Only MBC0 (no mapper) - works with 32KB ROMs only. MBC1/MBC3/MBC5 needed for 95%+ of commercial games
- **Game Boy Color**: DMG (original Game Boy) mode only - no CGB color palettes or features
- **Audio**: APU implementation exists but not integrated with frontend (silent gameplay)
- **Timer**: Timer registers not implemented - games relying on timer interrupts won't work
- **Interrupts**: Registers exist but interrupt handling not fully wired
- **Timing Model**: Frame-based rendering (not cycle-accurate) - suitable for homebrew and simple games
- **Other**: No serial transfer (link cable), OAM DMA, or sprite-per-scanline limit

**Controls**: Game Boy buttons are mapped to the same keyboard layout as NES:
- Arrow keys = D-pad
- Z = A button
- X = B button
- Enter = Start
- Left Shift = Select

### SNES (Super Nintendo Entertainment System)

**Status**: Basic implementation (stub)

**Coverage**: Limited - skeleton implementation for testing

**ROM Format**: SMC/SFC (.smc, .sfc files) - automatically detected

**Features**:
- 65C816 CPU core with 16-bit extensions
- Basic memory bus (128KB WRAM + cartridge mapping)
- LoROM cartridge mapping
- SMC header detection and removal
- Save states (F5/F6)
- Resolution: 256x224 pixels

**Known Limitations**:
- **Graphics**: PPU not implemented - displays black screen only
- **Audio**: APU not implemented - silent gameplay
- **Input**: Controller support not implemented
- **Cartridge**: Only basic LoROM mapping - no HiROM, ExHiROM, or enhancement chips (SuperFX, DSP, etc.)
- **Timing**: Stub implementation - not cycle-accurate
- **Status**: This is a skeleton implementation for infrastructure testing. Full SNES emulation requires significant PPU and APU work.

### N64 (Nintendo 64)

**Status**: Basic implementation with enhanced RDP graphics processor including 3D triangle rendering

**Coverage**: Limited - CPU and RDP with 3D rendering implemented, no game rendering yet

**ROM Format**: Z64/N64/V64 (.z64, .n64, .v64 files) - automatically detected with byte-order conversion

**Features**:
- MIPS R4300i CPU core with complete instruction set
- Memory bus (4MB RDRAM + PIF + SP memory + RDP/VI registers)
- RDP (Reality Display Processor) with enhanced framebuffer support
  - **3D triangle rasterization** with flat and Gouraud shading
  - **Z-buffer (depth buffer)** for hidden surface removal
  - **Scissor clipping** for efficient rendering
  - Scanline-based triangle rasterization
- VI (Video Interface) with display configuration registers
- ROM loading with automatic byte-order detection and conversion
- Save states (F5/F6)
- Resolution: 320x240 pixels (configurable)

**3D Rendering Capabilities**:
- **Triangle Rendering**:
  - Flat-shaded triangles (solid color)
  - Gouraud-shaded triangles (per-vertex color interpolation)
  - Z-buffered triangles (depth testing for proper occlusion)
  - Combined shading + Z-buffer rendering
- **Z-Buffer**:
  - 16-bit depth buffer (0 = near, 0xFFFF = far)
  - Per-pixel depth testing
  - Automatic depth buffer updates
  - Can be enabled/disabled per triangle
- **Rasterization Features**:
  - Scanline-based edge walking
  - Barycentric coordinate interpolation
  - Per-pixel color and depth interpolation
  - Scissor rectangle clipping

**Known Limitations**:
- **Graphics**: RDP implementation supports basic display list commands
  - **Working commands**:
    - FILL_RECTANGLE - solid color rectangles
    - SET_FILL_COLOR - set fill color for rectangles
    - SET_SCISSOR - clipping rectangle support (fully working)
    - SET_TILE - configure tile descriptors for textures (fully implemented)
    - SET_TEXTURE_IMAGE - set texture source address (fully implemented)
    - SYNC commands (SYNC_FULL, SYNC_PIPE, SYNC_TILE, SYNC_LOAD)
    - SET_COLOR_IMAGE - accepted but uses internal framebuffer
  - **Triangle commands** (opcodes 0x08-0x0F):
    - Command placeholders in place for future full implementation
    - Direct triangle drawing functions available via API
  - **Stub implementations** (accept but don't fully process):
    - TEXTURE_RECTANGLE - currently renders as solid rectangle (needs texture sampling)
    - LOAD_BLOCK, LOAD_TILE - texture loading (needs RDRAM callback integration)
    - SET_OTHER_MODES - rendering modes configuration
  - **TMEM (Texture Memory)**:
    - 4KB TMEM buffer allocated
    - Tile descriptors (8 tiles) fully configured via SET_TILE
    - Texture image address tracking via SET_TEXTURE_IMAGE
    - Ready for texture sampling implementation
  - **Not implemented**: 
    - Actual texture sampling and filtering
    - Textured triangle rasterization (only flat/shaded triangles)
    - Anti-aliasing and blending
    - Perspective-correct texture mapping
    - Display list triangle commands (API exists but not wired to command processor)
    - Most advanced rendering commands
  - Can render 3D wireframe and flat-shaded graphics
  - Full game graphics require texture sampling, RSP, and additional RDP features
- **VI (Video Interface)**: Registers implemented but not fully integrated
  - All VI registers accessible (STATUS, ORIGIN, WIDTH, timing, scaling)
  - Not yet used for actual display output (uses RDP internal framebuffer)
  - Scanline tracking and interrupt support in place but not active
- **RSP**: Reality Signal Processor not implemented - no geometry processing or microcode execution
  - Display lists must be pre-formatted RDP commands
  - No vertex transformation, lighting, or display list generation
- **Audio**: Audio interface not implemented - silent gameplay
- **Input**: Controller support not implemented
- **Memory**: Basic memory map only - no TLB, cache, or accurate timing
- **Timing**: Frame-based implementation - not cycle-accurate
- **Status**: Basic RDP display list processing is functional with TMEM support. Games can render simple 2D graphics. Full N64 emulation requires RSP implementation, texture sampling, complete RDP command set, and Z-buffer support.

## Troubleshooting

### ROM won't load
- Ensure the ROM is in the correct format:
  - NES: iNES format (.nes files)
  - SNES: SMC/SFC format (.smc, .sfc files) - supports 512-byte SMC headers
  - N64: Z64/N64/V64 format (.z64, .n64, .v64 files) - all byte orders supported
  - Atari 2600: Raw binary (.a26 or .bin files) - must be 2K, 4K, 8K, 12K, 16K, or 32K in size
  - Game Boy: GB/GBC format (.gb, .gbc files)
  - PC/DOS: COM/EXE format (.com, .exe files)
- Check that the file isn't corrupted
- Try a different ROM to verify the emulator works
- Check the console output (if running from terminal) for specific error messages
- For Atari 2600: Some ROM dumps may have headers that need to be removed - use headerless ROMs

### Audio issues
- The emulator requires a working audio output device
- On Linux, ensure ALSA is properly configured
- Check your system's audio settings
- **Note**: Atari 2600 audio is not yet implemented - games will be silent

### Settings not saving
- Verify you have write permissions in the emulator directory
- Check that `config.json` isn't marked as read-only
- Settings save automatically when changed (e.g., F11 for CRT filter, F3 for ROM)

### Save states not working
- Ensure you've loaded a ROM first
- The `saves/` directory should be created automatically
- Check file system permissions in the emulator directory

### Performance issues
- Try disabling CRT filters (F11 to cycle to "None")
- Close other resource-intensive applications
- Ensure your graphics drivers are up to date

## System Requirements

### Minimum Requirements
- **OS**: Windows 10+, Linux (Ubuntu 20.04+), macOS 10.15+
- **RAM**: 256 MB
- **Storage**: 50 MB free space
- **Audio**: Any audio output device

### Recommended Requirements
- **OS**: Windows 11, Linux (Ubuntu 22.04+), macOS 12+
- **RAM**: 512 MB
- **Storage**: 100 MB free space (plus space for save states)

## Legal Notice

This emulator is for educational purposes. Users must provide their own legally obtained ROM files. The project does not include or distribute any copyrighted game data.

## Getting Help

If you encounter issues:
1. Check this manual for troubleshooting steps
2. Visit the project repository: https://github.com/Hexagon/hemulator
3. Report bugs via GitHub Issues with detailed information about your system and the issue

---

**Version**: 0.1.0  
**Last Updated**: December 2024
