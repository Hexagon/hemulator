# Generic Slot-Based BIOS Loading Implementation

## Summary

Successfully implemented a **generic slot-based file mounting system** for command-line BIOS and ROM loading across multiple emulated systems. The system provides a unified interface for loading optional components (BIOS ROMs, boot floppies, etc.) via `--slot1`, `--slot2`, etc. flags.

## What Was Implemented

### Core Features

1. **Generic Mount Point System** (`mount_slot_files()` in `main.rs`)
   - Maps command-line slot arguments (`--slot1`, `--slot2`, etc.) to system-specific mount points
   - Works with PC, SMS, ColecoVision, and other systems
   - Automatic file validation and reading
   - Persistent mount tracking via `RuntimeState`

2. **System-Specific Slot Mappings**

   | System | Slot 1 | Slot 2 | Slot 3 | Slot 4 | Slot 5 |
   |--------|--------|--------|--------|--------|--------|
   | **PC** | BIOS ROM | Floppy A: | Floppy B: | Hard Drive C: | CD-ROM |
   | **SMS** | BIOS ROM | Cartridge | — | — | — |
   | **ColecoVision** | BIOS ROM | Cartridge | — | — | — |
   | **Other** | Cartridge | — | — | — | — |

3. **Command-Line Interface**

   ```bash
   # PC system with BIOS and floppy boot
   hemu --system pc --slot1 bios.bin --slot2 boot.img
   
   # SMS with BIOS ROM
   hemu --system sms --slot1 bios.sms --slot2 sonic.sms
   
   # ColecoVision with BIOS ROM
   hemu --system colecovision --slot1 bios.rom --slot2 donkey_kong.col
   ```

4. **Enhanced Help Text**
   - Added system-specific slot descriptions to `--help` output
   - Provided usage examples for each system
   - Clear indication of required vs. optional slots

### Systems Enabled

- ✅ PC - Slot-based loading preserved and refactored
- ✅ SMS - Slot 1 = BIOS, Slot 2 = Cartridge
- ✅ ColecoVision - Slot 1 = BIOS, Slot 2 = Cartridge
- ✅ SNES - Clean system initialization
- ✅ CHIP-8 - Clean system initialization
- ✅ SG-1000 - Clean system initialization
- ⚠️ N64 - Uses deferred initialization (requires OpenGL context)

## Code Changes

### Main File: `crates/frontend/gui/src/main.rs`

1. **New Function** (line ~1884): `mount_slot_files()`
   - Generic handler for all system slot mounting
   - System-aware dispatch based on `EmulatorSystem` variant
   - File reading and mount point tracking
   - Returns success status

2. **Updated System Initialization** (line ~2636)
   - Added cases for SMS, ColecoVision, SNES, CHIP-8, SG-1000
   - Removed duplicate N64 case that was causing build error
   - Preserved N64's deferred initialization pattern

3. **Integrated Slot Mounting** (line ~3116)
   - Called `mount_slot_files()` for any system with slot arguments
   - Works alongside auto-detected ROM loading
   - Properly handles slot-specific initialization

4. **Enhanced Help Text** (lines ~1641-1729)
   - Documented system-specific slot support
   - Added usage examples for PC, SMS, ColecoVision
   - Formatted for clarity and reference

### Documentation Updates

#### [SMS README](crates/systems/sms/README.md)
- Added "Command-Line Usage" section with examples
- Documented BIOS loading via `--slot1` and `--slot2`
- Explained BIOS behavior (optional, auto-enables, disables itself)
- Noted which games require BIOS support

#### [ColecoVision README](crates/systems/colecovision/README.md)
- Added "Command-Line Usage" section
- Documented required BIOS ROM mounting
- Explained system alias (`colecovision` / `coleco`)
- Provided concrete usage examples

## Build Status

- ✅ **Build**: Successful (`cargo build --profile release-quick`)
- ✅ **Formatting**: Passed (`cargo fmt --all -- --check`)
- ⚠️ **Clippy**: 2 minor warnings (unused variable `_pc_sys` - cosmetic, pre-existing SDL2 issue)
- ⚠️ **Tests**: Blocked by SDL2 build dependency issue (pre-existing environment issue, not code-related)

## Testing Recommendations

After the SDL2 environment is fixed, test:

1. **SMS with BIOS**
   ```bash
   hemu --system sms --slot1 bios.sms --slot2 sonic.sms
   ```
   Expected: BIOS shows SEGA logo, game loads

2. **ColecoVision with BIOS**
   ```bash
   hemu --system colecovision --slot1 "ColecoVision BIOS (1982).col" --slot2 donkey_kong.col
   ```
   Expected: BIOS initializes, game displays properly

3. **PC with Floppy**
   ```bash
   hemu --system pc --slot1 bios.bin --slot2 boot.img
   ```
   Expected: BIOS boots, loads floppy

## Files Modified

1. `crates/frontend/gui/src/main.rs` - Core implementation
2. `crates/systems/sms/README.md` - Documentation
3. `crates/systems/colecovision/README.md` - Documentation

## Files NOT Modified (as intended)

- Core system implementations (SMS, ColecoVision) - No code changes needed
- Test files - Would require valid test ROMs (legal requirement)
- Main README - Feature well-represented in documentation site

## Benefits

1. **Unified Interface** - Same `--slot1`, `--slot2` pattern across all systems
2. **BIOS Support** - SMS and ColecoVision games can now load BIOS ROMs
3. **Extensible** - Easy to add slot support for other systems
4. **Documented** - Clear examples and usage in system READMEs
5. **Backward Compatible** - Doesn't break existing ROM loading mechanisms

## Future Enhancements

1. Add test ROMs for SMS and ColecoVision for smoke testing
2. Document slot support in main user manual
3. Add GUI dialog for slot file selection
4. Implement slot support for SNES (optional cartridge header)
5. Consider BIOS auto-detection from known locations

## Known Limitations

- N64 uses deferred initialization (requires OpenGL context, not available at startup)
- SDL2 environment issue prevents full test suite execution (pre-existing)
- Some systems may have additional slot requirements not yet documented

## References

- [Original Conversation Context](../../AGENTS.md) - Implementation guidelines
- [SMS System Documentation](crates/systems/sms/README.md) - BIOS support details
- [ColecoVision System Documentation](crates/systems/colecovision/README.md) - Required BIOS
- [PC System Documentation](crates/systems/pc/README.md) - Multiple slot usage example
