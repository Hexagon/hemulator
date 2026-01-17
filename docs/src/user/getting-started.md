---
title: "Getting Started"
parent: "User Manual"
nav_order: 1
---

# Getting Started

## First Run

1. **Launch the emulator**: Double-click `hemu` (or `hemu.exe` on Windows)
2. **The splash screen appears** with instructions
3. **Load a ROM**: Click **File > Open ROM** or press **Ctrl+O** to open the file browser
4. **Select your game file**:
   - `.nes` for NES
   - `.smc`/`.sfc` for SNES
   - `.z64`/`.n64`/`.v64` for N64
   - `.sms` for Sega Master System
   - `.a26`/`.bin` for Atari 2600
   - `.gb`/`.gbc` for Game Boy
   - `.ch8`/`.c8` for CHIP-8/Super-CHIP/XO-CHIP
   - `.col` for ColecoVision
   - `.sg`/`.sc` for SG-1000
   - `.com`/`.exe`/`.img` for PC/DOS
5. **Start playing!** Use the controls listed in the [Controls](controls.md) section

Alternatively, you can provide a ROM path as an argument:
```bash
./hemu path/to/your/game.nes
```

The emulator will remember your last ROM and automatically load it next time you start.

## Advanced Command-Line Options

### PC/XT Slot-Based Loading

For PC/XT emulation, you can specify disk images for specific drive slots:

```bash
# Load PC with a floppy disk in drive A:
./hemu --slot2 bootdisk.img

# Load PC with both floppy and hard drive
./hemu --slot2 boot.img --slot4 harddrive.img

# Optional: Load custom BIOS ROM (built-in BIOS used if not specified)
./hemu --slot1 custom_bios.bin --slot2 floppy.img --slot4 hdd.img
```

**Slot Mapping for PC/XT**:
- `--slot1 <file>`: BIOS ROM (optional - built-in BIOS used if not specified)
- `--slot2 <file>`: Floppy Drive A:
- `--slot3 <file>`: Floppy Drive B:
- `--slot4 <file>`: Hard Drive C:
- `--slot5 <file>`: Reserved for future use

### Creating Blank Disk Images

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

### Other Options

- `--keep-logs`: Preserve debug logging environment variables (for development)
