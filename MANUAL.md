# Hemulator User Manual

Welcome to Hemulator, a cross-platform multi-system console emulator supporting NES, Atari 2600, and Game Boy (WIP) emulation.

## Getting Started

### First Run

1. **Launch the emulator**: Double-click `hemu` (or `hemu.exe` on Windows)
2. **The splash screen appears** with instructions
3. **Load a ROM**: Press `F3` to open the file browser
4. **Select your game file** (`.nes` for NES, `.a26`/`.bin` for Atari 2600, `.gb`/`.gbc` for Game Boy)
5. **Start playing!** Use the controls listed below

Alternatively, you can provide a ROM path as an argument:
```bash
./hemu path/to/your/game.nes
```

The emulator will remember your last ROM and automatically load it next time you start.

## Controls

### Game Controller (Customizable)

| Key | Action | Notes |
|-----|--------|-------|
| Arrow Keys | D-pad | Up/Down/Left/Right |
| Z | A button | Confirm/Jump |
| X | B button | Back/Action |
| Enter | Start | Pause menu |
| Left Shift | Select | Menu navigation |
| Escape | Exit | Close emulator |

*All controller mappings can be customized by editing `config.json`*

### Function Keys

| Key | Action | Description |
|-----|--------|-------------|
| F1 | Help Overlay | Show/hide all controls and key mappings |
| F2 | Speed Selector | Open speed selector menu (pause, 0.25x, 0.5x, 1x, 2x, 10x) |
| F3 | Load Media | Open mount point selector (if system has multiple slots) or file browser directly |
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

Available filters:
- **None**: Raw pixel output with no filtering
- **Scanlines**: Adds horizontal dark lines simulating CRT raster scan lines
- **Phosphor**: Creates a soft glow and horizontal color bleeding between pixels, mimicking CRT phosphor behavior
- **CRT Monitor**: Full CRT effect combining scanlines, phosphor glow, and brightness adjustments for an authentic CRT appearance

The selected filter is automatically saved and restored when you restart the emulator.

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
  "keyboard": {
    "a": "Z",
    "b": "X",
    "select": "LeftShift",
    "start": "Enter",
    "up": "Up",
    "down": "Down",
    "left": "Left",
    "right": "Right"
  },
  "window_width": 512,
  "window_height": 480,
  "last_rom_path": "/path/to/last/rom.nes",
    "mount_points": {
    "Cartridge": "/path/to/last/rom.nes"
  }
  "crt_filter": "None",
  "emulation_speed": 1.0
}
```

**Customization**: 
- Edit this file to change key bindings
- The window size is automatically saved when you resize the window
- CRT filter preference is saved automatically when you cycle filters with F11
- Emulation speed is saved automatically when you change it with F2
- Valid `crt_filter` values: "None", "Scanlines", "Phosphor", "CrtMonitor"
- Valid `emulation_speed` values: 0.0 (pause), 0.25, 0.5, 1.0, 2.0, 10.0 (or any positive number)

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
- Audio not yet implemented (sound registers present but silent)
- Sprite/player graphics not yet rendered (playfield only)
- Some advanced cartridge formats not supported

**Controls**: The Atari 2600 joystick is mapped to the same keyboard layout as NES:
- Arrow keys = Joystick directions
- Z = Fire button
- Enter = Game Reset (console switch)
- Left Shift = Game Select (console switch)

### Game Boy / Game Boy Color

**Status**: Work in progress (skeleton implementation)

**ROM Format**: GB/GBC (.gb, .gbc files) - automatically detected

## Troubleshooting

### ROM won't load
- Ensure the ROM is in the correct format:
  - NES: iNES format (.nes files)
  - Atari 2600: Raw binary (.a26 or .bin files) - must be 2K, 4K, 8K, 12K, 16K, or 32K in size
  - Game Boy: GB/GBC format (.gb, .gbc files)
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
