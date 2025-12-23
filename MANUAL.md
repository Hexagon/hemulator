# Hemulator User Manual

Welcome to Hemulator, a cross-platform multi-system console emulator supporting NES, SNES, N64, Atari 2600, Game Boy, and PC/DOS emulation.

**For Developers**: See [README.md](README.md) for build instructions and [AGENTS.md](AGENTS.md) for architecture details.

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

### Advanced Command-Line Options

**PC/XT Slot-Based Loading**:

For PC/XT emulation, you can specify disk images for specific drive slots:

```bash
# Load PC with a floppy disk in drive A:
./hemu --slot2 bootdisk.img

# Load PC with both floppy and hard drive
./hemu --slot2 boot.img --slot4 harddrive.img

# Load custom BIOS and disks
./hemu --slot1 custom_bios.bin --slot2 floppy.img --slot4 hdd.img
```

**Slot Mapping for PC/XT**:
- `--slot1 <file>`: BIOS ROM (default BIOS used if not specified)
- `--slot2 <file>`: Floppy Drive A:
- `--slot3 <file>`: Floppy Drive B:
- `--slot4 <file>`: Hard Drive C:
- `--slot5 <file>`: Reserved for future use

**Creating Blank Disk Images**:

Create blank floppy or hard drive images for use with PC/XT emulation:

```bash
# Create a 1.44MB floppy disk
./hemu --create-blank-disk mydisk.img 1.44m

# Create a 20MB hard drive
./hemu --create-blank-disk harddrive.img 20m
```

**Supported Disk Formats**:
- Floppy: `360k`, `720k`, `1.2m`, `1.44m`
- Hard Drive: `10m`, `20m`, `40m`

**Other Options**:
- `--keep-logs`: Preserve debug logging environment variables (for development)

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
- **NES**: Fully supported - save and load states with F5-F9 when a cartridge is loaded
- **Atari 2600**: Fully supported - save and load states with F5-F9
- **Game Boy**: Fully supported - save and load states with F5-F9

Example structure:
```
saves/
  ├── a1b2c3d4.../  (ROM hash)
  │   └── states.json
  └── e5f6g7h8.../
      └── states.json
```

## Supported Systems

This emulator supports 6 different retro gaming systems. Here's a quick overview:

| System | Status | What Works | What's Missing | Recommended For |
|--------|--------|------------|----------------|-----------------|
| **NES** | ✅ Fully Working | Everything | - | Playing NES games |
| **Atari 2600** | ✅ Fully Working | Everything | - | Playing Atari games |
| **Game Boy** | ✅ Fully Working | Everything | - | Playing GB games |
| **SNES** | 🚧 Basic | CPU, basic rendering | PPU features, audio, input | Testing only |
| **N64** | 🚧 In Development | 3D rendering, CPU | Full graphics, audio, games | Development/testing |
| **PC/DOS** | 🧪 Experimental | Multi-slot mounts, disk controller, custom BIOS, CGA/EGA/VGA | Full disk I/O, boot | Development/testing |

### NES (Nintendo Entertainment System)

**Status**: ✅ Fully Working  
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

**Status**: ✅ Fully Working  
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
- TIA audio emulation with 2 channels (polynomial waveform synthesis)
- RIOT (6532) chip emulation for RAM, I/O, and timers
- Save states (F5/F6)
- Joystick controls mapped to keyboard (same as NES controls)
- 160x192 resolution

**Known Limitations**:
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

**Status**: ✅ Fully Working  
**Coverage**: ~95%+ of Game Boy games supported (MBC0, MBC1, MBC3, MBC5 implemented)

**ROM Format**: GB/GBC (.gb, .gbc files) - automatically detected

**Features**:
- Full PPU (Picture Processing Unit) rendering: background, window, sprites
- Resolution: 160x144 pixels (DMG mode)
- Sprite support: 40 sprites with 8x8/8x16 modes, flipping, priority
- **MBC (Memory Bank Controller) Support**:
  - MBC0: No mapper (32KB ROMs)
  - MBC1: Most common mapper (~70% of games, up to 2MB ROM, 32KB RAM)
  - MBC3: With battery saves and RTC registers (~15% of games, up to 2MB ROM, 32KB RAM)
  - MBC5: Advanced mapper (~10% of games, up to 8MB ROM, 128KB RAM)
- Joypad input with matrix selection
- Timer registers (DIV, TIMA, TMA, TAC) with interrupt support
- VBlank and Timer interrupts
- **Audio**: Full APU with 4 sound channels (Pulse 1/2, Wave, Noise)
- Audio integrated with frontend (44.1 kHz stereo output)
- Save states (F5-F9)
- Frame-based timing (~59.73 Hz)

**Known Limitations**:
- **MBC2**: Not implemented (~1% of games) - rare mapper with built-in 512×4 bits RAM
- **Game Boy Color**: DMG (original Game Boy) mode only - no CGB color palettes or features
- **RTC**: MBC3 RTC registers are accessible but clock doesn't actually count time
- **Timing Model**: Frame-based rendering (not cycle-accurate) - suitable for most games
- **Other**: No serial transfer (link cable), OAM DMA, or sprite-per-scanline limit

**Controls**: Game Boy buttons are mapped to the same keyboard layout as NES:
- Arrow keys = D-pad
- Z = A button
- X = B button
- Enter = Start
- Left Shift = Select

### SNES (Super Nintendo Entertainment System)

**Status**: 🚧 Basic (Mode 0 rendering functional - limited game support)  
**Coverage**: Very limited - CPU works, Mode 0 PPU functional, simple games may display

**ROM Format**: SMC/SFC (.smc, .sfc files) - automatically detected

**Features**:
- 65C816 CPU core with 16-bit extensions (100% complete)
- Basic memory bus (128KB WRAM + cartridge mapping)
- LoROM cartridge mapping
- SMC header detection and removal
- **PPU with Mode 0 support**:
  - 4 background layers with 2bpp tiles (4 colors per tile)
  - 8 palettes per layer (32 colors total per layer)
  - Horizontal and vertical tile flipping
  - Layer enable/disable control
  - Proper tile attribute handling
  - 256x224 resolution
- Save states (F5/F6)

**Known Limitations**:
- **Graphics**: Mode 0 only - no sprites, no other PPU modes (1-7)
  - No scrolling (BG offset registers not implemented)
  - No windows, masks, or special effects
  - No HDMA, mosaic, or color math
  - Only 32x32 tilemap size (other sizes not implemented)
- **Audio**: APU not implemented - silent gameplay
- **Input**: Controller support not implemented
- **Cartridge**: Only basic LoROM mapping - no HiROM, ExHiROM, or enhancement chips (SuperFX, DSP, etc.)
- **Timing**: Frame-based - not cycle-accurate
- **Status**: Can display simple Mode 0 graphics (like test ROMs), but most commercial games won't work due to missing features

### N64 (Nintendo 64)

**Status**: 🚧 In Development (3D rendering works, limited game support)  
**Coverage**: Very limited - Core components functional, working towards game compatibility

**ROM Format**: Z64/N64/V64 (.z64, .n64, .v64 files) - automatically detected with byte-order conversion

**Features**:
- MIPS R4300i CPU core with complete instruction set
- Memory bus (4MB RDRAM + PIF + SP memory + RDP/VI registers)
- **PIF (Peripheral Interface)** - Controller support
  - 4 controller ports with full button mapping
  - All 14 buttons supported: A, B, Z, Start, D-pad (4), L, R, C-buttons (4)
  - Analog stick with full range (-128 to 127 on X/Y axes)
  - Controller command protocol for game communication
- **RSP (Reality Signal Processor)** - High-Level Emulation
  - Microcode detection (F3DEX/F3DEX2/Audio)
  - Vertex buffer management (32 vertices)
  - **Full matrix transformation pipeline**:
    - 4x4 projection and modelview matrices
    - Matrix loading from RDRAM (16.16 fixed-point format)
    - Matrix multiplication (LOAD and MUL modes)
    - Complete vertex transformation: modelview → projection → perspective divide → viewport
  - **F3DEX display list commands**:
    - G_VTX (0x01) - Load vertices
    - G_TRI1 (0x05), G_TRI2 (0x06), G_QUAD (0x07) - Triangle/quad rendering
    - G_MTX (0xDA) - Load transformation matrices
    - G_GEOMETRYMODE (0xD9) - Set rendering flags
    - G_DL (0xDE) - Display list branching (nested display lists)
    - G_ENDDL (0xDF) - End display list
    - RDP passthrough (0xE0-0xFF) - Embedded RDP commands
  - Task execution framework for graphics microcode
- RDP (Reality Display Processor) with enhanced framebuffer support
  - **Pluggable renderer architecture**: Software (CPU) and OpenGL (GPU) backends
  - **Software renderer** (default): Fully functional, high accuracy
  - **OpenGL renderer** (stub): Architecture in place for future GPU acceleration
  - **3D triangle rasterization** with flat, Gouraud shading, and texture mapping
  - **Z-buffer (depth buffer)** for hidden surface removal
  - **Scissor clipping** for efficient rendering
  - **Texture mapping** with UV coordinate interpolation
  - Scanline-based triangle rasterization
- VI (Video Interface) with display configuration registers
- ROM loading with automatic byte-order detection and conversion
- Save states (F5/F6)
- Resolution: 320x240 pixels (configurable)

**3D Rendering Capabilities**:
- **Triangle Rendering**:
  - Flat-shaded triangles (solid color)
  - Gouraud-shaded triangles (per-vertex color interpolation)
  - **Textured triangles** (with UV coordinate interpolation)
  - Z-buffered triangles (depth testing for proper occlusion)
  - Combined shading + Z-buffer rendering
  - Combined texture + Z-buffer rendering
- **Z-Buffer**:
  - 16-bit depth buffer (0 = near, 0xFFFF = far)
  - Per-pixel depth testing
  - Automatic depth buffer updates
  - Can be enabled/disabled per triangle
- **Rasterization Features**:
  - Scanline-based edge walking
  - Barycentric coordinate interpolation
  - Per-pixel color and depth interpolation
  - **Per-pixel texture coordinate interpolation**
  - Scissor rectangle clipping
- **Texture Mapping**:
  - 4KB TMEM (Texture Memory) for texture storage
  - 8 tile descriptors for texture configuration
  - RGBA16 (5-5-5-1) and RGBA32 (8-8-8-8) format support
  - Texture wrapping and clamping modes
  - LOAD_BLOCK and LOAD_TILE commands for texture loading

**Controller Mapping**:
For N64 games, the standard controller mappings apply with these button equivalents:
- **A** = Z key (Player 1) / U key (Player 2)
- **B** = X key (Player 1) / O key (Player 2)
- **Start** = Enter (Player 1) / P (Player 2)
- **D-pad** = Arrow keys (Player 1) / I/J/K/L (Player 2)
- **L/R** = (Not yet mapped - will be added in future update)
- **C-buttons** = (Not yet mapped - will be added in future update)
- **Analog stick** = (Not yet mapped - will be added in future update)

*Note: Controller mappings can be customized in `config.json`. Full analog stick and shoulder button support coming soon.*

**Known Limitations**:
- **Renderer Architecture**:
  - Software renderer is fully functional (default)
  - OpenGL renderer is a stub (requires GL context from frontend)
  - To enable OpenGL stub: build with `--features opengl`
  - Future OpenGL implementation blocked by minifb's lack of GL context exposure
  - See `N64_RENDERER_ARCHITECTURE.md` for details on renderer design
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
    - 0x08: Non-shaded triangle (fully implemented)
    - 0x09: Non-shaded triangle with Z-buffer (fully implemented)
    - 0x0A: Textured triangle (fully implemented)
    - 0x0B: Textured triangle with Z-buffer (fully implemented)
    - 0x0C: Shaded triangle (fully implemented)
    - 0x0D: Shaded triangle with Z-buffer (fully implemented)
  - **Stub implementations** (accept but don't fully process):
    - TEXTURE_RECTANGLE - currently renders as solid rectangle (needs advanced sampling)
    - SET_OTHER_MODES - rendering modes configuration
  - **TMEM (Texture Memory)**: ✅ Fully implemented
    - 4KB TMEM buffer with texture loading via LOAD_BLOCK and LOAD_TILE
    - Tile descriptors (8 tiles) fully configured via SET_TILE
    - Texture image address tracking via SET_TEXTURE_IMAGE
    - Texture sampling for RGBA16 and RGBA32 formats
    - **Textured triangle rendering fully integrated**
  - **Not implemented**: 
    - Advanced texture formats (CI, IA, I)
    - Anti-aliasing and blending
    - Perspective-correct texture mapping
    - Most advanced rendering commands
  - Can render 3D textured graphics with depth testing
  - Full game graphics require perspective-correct mapping, additional RDP features, and more complete RSP emulation
- **VI (Video Interface)**: Registers implemented but not fully integrated
  - All VI registers accessible (STATUS, ORIGIN, WIDTH, timing, scaling)
  - Not yet used for actual display output (uses RDP internal framebuffer)
  - Scanline tracking and interrupt support in place but not active
- **RSP**: High-Level Emulation with F3DEX display list processing
  - ✅ **Implemented**:
    - Microcode detection (F3DEX, F3DEX2, Audio)
    - Vertex buffer management (32 vertices)
    - Full matrix transformation pipeline (projection, modelview, viewport)
    - **10-level matrix stack** with push/pop operations (G_MTX, G_POPMTX)
    - Display list parsing with command execution
    - Matrix loading (G_MTX) with LOAD/MUL/PUSH modes
    - Geometry mode control (G_GEOMETRYMODE)
    - Triangle rendering commands (G_TRI1, G_TRI2, G_QUAD)
    - Display list branching (G_DL) for nested lists
    - **Conditional branching (G_BRANCH_Z)** for Z-buffer-based culling
    - RDP command passthrough (0xE0-0xFF range)
    - Vertex transformation with perspective projection
    - RDP triangle command generation
  - ⚠️ **Limitations**:
    - No lighting calculations
    - No texture coordinate generation
    - Some advanced F3DEX2 commands missing
- **Audio**: Audio interface not implemented - silent gameplay
- **Input**: Controller infrastructure complete, needs frontend integration
  - All 14 buttons defined and working (A, B, Z, Start, D-pad, L, R, C-buttons)
  - Analog stick support implemented (-128 to 127 range)
  - PIF command protocol functional
  - Frontend keyboard/gamepad mapping not yet connected
- **Memory**: Basic memory map only - no TLB, cache, or accurate timing
- **Timing**: Frame-based implementation - not cycle-accurate
- **Status**: Core infrastructure in place (CPU, RDP, RSP HLE with F3DEX support, PIF). RSP supports full matrix stack operations and conditional branching. **Textured triangle rendering fully implemented** with TMEM texture loading and sampling. Next steps: perspective-correct mapping, lighting, frontend controller integration. Test ROMs can run and render transformed 3D graphics with textures.

### PC/DOS (IBM PC/XT)

**Status**: 🧪 Experimental (Modular architecture with disk support)  
**Coverage**: Basic emulation - Custom BIOS, multi-slot mount system

**File Formats**: 
- Executable: COM/EXE (.com, .exe files)
- BIOS: Binary ROM (.bin, .rom files)
- Floppy disks: Disk images (.img, .ima files)
- Hard drives: Disk images (.img, .vhd files)

**Features**:
- **8086 CPU core** with complete instruction set
  - All MOV, arithmetic (ADD, SUB, CMP, INC, DEC), and logical operations (AND, OR, XOR)
  - Control flow (JMP, conditional jumps, CALL, RET)
  - Stack operations (PUSH, POP)
  - Flag manipulation (CLC, STC, CLI, STI, etc.)
  - See `AGENTS.md` for full instruction set details
- **Memory bus** (640KB RAM, 128KB VRAM, 256KB ROM)
- **Custom BIOS** built from assembly source
  - 64KB BIOS ROM with INT 13h disk services
  - Source: `test_roms/pc/bios.asm`
  - Build script: `test_roms/pc/build.sh` (requires NASM)
  - Replaceable via BIOS mount point
- **Modular mount point system**:
  1. **BIOS** (Slot 1) - Custom or replacement BIOS ROM (`.bin`, `.rom`)
  2. **Floppy A** - Floppy disk drive A: (`.img`, `.ima`)
  3. **Floppy B** - Floppy disk drive B: (`.img`, `.ima`)
  4. **Hard Drive C** - Hard disk drive C: (`.img`, `.vhd`)
- **Disk controller** with INT 13h support
  - Floppy geometry: 1.44MB format (80 cylinders, 18 sectors, 2 heads)
  - Hard drive geometry: 10MB format (306 cylinders, 17 sectors, 4 heads)
  - LBA (Logical Block Address) calculation
  - Read/write operations to disk images
- **CGA video** (640x400 text mode)
- **Keyboard input** with full passthrough
- Save states (F5/F6)

**Mount Point Usage**:

There are two ways to mount disk images and BIOS:

1. **GUI Method** (F3 key):
   - Press F3 to open mount point selector
   - Select the desired slot (BIOS, FloppyA, FloppyB, or HardDrive)
   - Choose the file to mount

2. **Command-Line Method** (Recommended for quick loading):
   - Use `--slot1` through `--slot4` to load files directly
   - See "Advanced Command-Line Options" section for examples
   - Example: `./hemu --slot2 boot.img --slot4 hdd.img`

**Creating Disk Images**:
- Use `--create-blank-disk <path> <format>` to create blank disks
- See "Advanced Command-Line Options" section for supported formats
- Example: `./hemu --create-blank-disk floppy.img 1.44m`

**Keyboard Input**:
- All keyboard keys are passed through to the emulated PC
- Use **Right Ctrl** as host modifier to access emulator function keys (F1-F12)
- See "PC/DOS Keyboard Input" section for details

**Known Limitations**:
- **BIOS**: Minimal implementation - INT 13h disk services are stubs
  - Disk operations return success but don't actually read/write disk data yet
  - Future: Full INT 13h implementation with actual disk I/O
- **Display**: CGA, EGA, and VGA adapters implemented with multiple modes
  - **CGA Support** (Color Graphics Adapter):
    - Text mode: 80x25 characters (640x400 pixels)
    - Graphics modes: 320x200 4-color, 640x200 2-color
    - 16-color fixed palette
    - Software rendering (CPU-based)
    - Hardware rendering stub (OpenGL, for future use)
  - **EGA Support** (Enhanced Graphics Adapter):
    - Text mode: 80x25 characters (640x350 pixels, 8x14 font)
    - Graphics modes: 640x350 16-color, 320x200 16-color
    - 64-color palette (6-bit RGB), 16 active colors
    - Planar memory organization (4 bit planes)
    - Software rendering (CPU-based)
    - Hardware rendering stub (OpenGL, for future use)
  - **VGA Support** (Video Graphics Array):
    - Text mode: 80x25 characters (720x400 pixels, 9x16 font)
    - Graphics modes: 320x200 256-color (Mode 13h), 640x480 16-color
    - 256-color palette (18-bit RGB: 6 bits per channel)
    - Mode 13h uses linear addressing (1 byte per pixel)
    - 640x480x16 uses planar memory (4 bit planes)
    - Software rendering (CPU-based)
    - Hardware rendering stub (OpenGL, for future use)
  - Future: Additional palettes, more VGA modes
- **Input**: Keyboard passthrough works, but:
  - No mouse support
  - No serial/parallel port emulation
- **Disk I/O**: Infrastructure in place but not yet connected to BIOS
  - Disk controller implemented with read/write support
  - BIOS INT 13h handlers exist but return stubs
  - Next step: Wire disk controller to BIOS interrupts
- **No audio**: PC speaker not implemented
- **No timer**: PIT (Programmable Interval Timer) not implemented
- **Timing**: Frame-based execution - not cycle-accurate
- **Status**: Modular architecture complete with mount points for BIOS and disks. Custom BIOS built from source. Disk controller ready for integration. CGA, EGA, and VGA graphics adapters fully implemented with software and hardware (OpenGL stub) backends. Next steps: Connect disk controller to BIOS INT 13h, implement boot sector loading, add additional VGA modes and palettes.

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
- Game Boy audio is not yet connected to the frontend

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
