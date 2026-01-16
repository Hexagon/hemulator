---
title: "Troubleshooting"
parent: "User Manual"
nav_order: 7
---

# Troubleshooting

### ROM won't load
- Ensure the ROM is in the correct format:
  - NES: iNES format (.nes files)
  - SNES: SMC/SFC format (.smc, .sfc files) - supports 512-byte SMC headers
  - N64: Z64/N64/V64 format (.z64, .n64, .v64 files) - all byte orders supported
  - Atari 2600: Raw binary (.a26 or .bin files) - must be 2K, 4K, 8K, 12K, 16K, or 32K in size
  - Game Boy: GB/GBC format (.gb, .gbc files)
  - SMS: SMS format (.sms files)
  - ColecoVision: Cartridge format (.col files)
  - SG-1000: Cartridge format (.sg, .sc files)
  - PC/DOS: COM/EXE format (.com, .exe files)
  - CHIP-8: CH8/C8 format (.ch8, .c8 files)
- Check that the file isn't corrupted
- Try a different ROM to verify the emulator works
- Check the console output (if running from terminal) for specific error messages
- For Atari 2600: Some ROM dumps may have headers that need to be removed - use headerless ROMs
- For ColecoVision: BIOS is required for full compatibility (some games may work without it)
- For SG-1000: Similar to ColecoVision as both use the TMS9918A VDP

### Audio issues
- The emulator requires a working audio output device
- On Linux, ensure ALSA is properly configured
- Check your system's audio settings
- Game Boy audio is not yet connected to the frontend

### Settings not saving
- Verify you have write permissions in the emulator directory
- Check that `config.json` isn't marked as read-only
- Settings save automatically when changed (e.g., View → CRT Filter, File → Open ROM)

### Save states not working
- Ensure you've loaded a ROM first
- The `saves/` directory should be created automatically
- Check file system permissions in the emulator directory

### Performance issues
- Try disabling CRT filters (View → CRT Filter to cycle to "None")
- Close other resource-intensive applications
- Ensure your graphics drivers are up to date

For system requirements, see the [Download & Install](../download.html#system-requirements) page.

