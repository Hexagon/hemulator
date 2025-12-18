# Hemulator User Manual

Welcome to Hemulator, a cross-platform multi-system console emulator focusing on NES and Game Boy emulation.

## Getting Started

### First Run

1. **Launch the emulator**: Double-click `hemu` (or `hemu.exe` on Windows)
2. **The splash screen appears** with instructions
3. **Load a ROM**: Press `F3` to open the file browser
4. **Select your game file** (`.nes` for NES, `.gb`/`.gbc` for Game Boy)
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
| F3 | Open ROM | Browse and load a ROM file |
| F5 | Save State | Open slot selector (1-5) to save |
| F6 | Load State | Open slot selector (1-5) to load |
| F10 | Debug Info | Show/hide debug information overlay |
| F12 | Reset System | Restart the current game |

### Window Management

The emulator window can be resized freely by dragging the window edges or maximizing the window. The window maintains the correct aspect ratio while stretching to fill the available space. The window size is automatically remembered between sessions.

## Debug Information (F10)

When a ROM is loaded, press **F10** to display the debug information overlay. This shows:

- **Mapper**: The cartridge mapper number and name
- **PRG**: Number of PRG ROM banks (16KB each)
- **CHR**: Number of CHR ROM banks (8KB each) or "RAM" if using CHR-RAM
- **Timing**: NTSC or PAL timing mode (auto-detected from ROM header)
- **FPS**: Current frame rate

This information is useful for troubleshooting compatibility issues or understanding ROM specifications.

## Configuration

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
  "last_rom_path": "/path/to/last/rom.nes"
}
```

**Customization**: Edit this file to change key bindings. The window size is automatically saved when you resize the window.

### Save States

Save states are stored in `saves/<rom_hash>/states.json`:
- Each game gets its own directory based on ROM hash
- 5 slots available per game
- **F5** opens the save slot selector - press 1-5 to select a slot
- **F6** opens the load slot selector - press 1-5 to select a slot (shows which slots have saves)
- States are portable and can be backed up or transferred between systems

Example structure:
```
saves/
  ├── a1b2c3d4.../
  │   └── states.json
  └── e5f6g7h8.../
      └── states.json
```

## Supported Systems

### NES (Nintendo Entertainment System)

**Coverage**: 86% of all NES games (9 mappers supported)

The emulator supports the following NES mappers:
- **Mapper 0 (NROM)** - Simple games
- **Mapper 1 (MMC1/SxROM)** - Tetris, Metroid, The Legend of Zelda
- **Mapper 2 (UxROM)** - Mega Man, Castlevania, Contra
- **Mapper 3 (CNROM)** - Gradius, Paperboy
- **Mapper 4 (MMC3/TxROM)** - Super Mario Bros. 3, Mega Man 3-6
- **Mapper 7 (AxROM)** - Battletoads, Marble Madness
- **Mapper 9 (MMC2/PxROM)** - Mike Tyson's Punch-Out!!
- **Mapper 10 (MMC4/FxROM)** - Fire Emblem (Japan)
- **Mapper 11 (Color Dreams)** - Color Dreams and Wisdom Tree games

**ROM Format**: iNES (.nes files) - automatically detected

### Game Boy / Game Boy Color

**Status**: Work in progress (skeleton implementation)

**ROM Format**: GB/GBC (.gb, .gbc files) - automatically detected

## Troubleshooting

### ROM won't load
- Ensure the ROM is in iNES format (.nes) for NES games
- Check that the file isn't corrupted
- Try a different ROM to verify the emulator works
- Check the console output (if running from terminal) for specific error messages

### Audio issues
- The emulator requires a working audio output device
- On Linux, ensure ALSA is properly configured
- Check your system's audio settings

### Settings not saving
- Verify you have write permissions in the emulator directory
- Check that `config.json` isn't marked as read-only
- Settings save automatically when changed (e.g., F11 for scale, F3 for ROM)

### Save states not working
- Ensure you've loaded a ROM first
- The `saves/` directory should be created automatically
- Check file system permissions in the emulator directory

### Performance issues
- Try using a lower window scale (F11 to cycle through options)
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
