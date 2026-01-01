# Status Bar Changes Summary

## Overview

This PR addresses all three issues mentioned in the problem statement:

1. ✅ Status bar now shows if software or hardware rendering is used
2. ✅ Status bar shows current CPU frequency target (+ infrastructure for actual CPU frequency)
3. ✅ IP is now properly updated for all systems

## Visual Changes

### Before
```
Left: [⏸ PAUSED]        Center: [Status Message]        Right: [60fps · $0000 · 128K]
```

### After
```
Left: [⏸ PAUSED]        Center: [Status Message]        Right: [Software · 1.8MHz · 60fps · $C000 · 128K]
```

## Status Bar Layout (Right Side)

The status bar now displays the following information on the right side (in order):

1. **Rendering Backend** - "Software" or "OpenGL"
2. **CPU Frequency** - Target frequency in MHz (e.g., "1.8MHz", "4.8MHz", "93.8MHz")
3. **FPS** - Frames per second (e.g., "60fps")
4. **Instruction Pointer** - Current program counter/IP in hex (e.g., "$C000", "$F0A123", "$80000000")
5. **Cycle Count** - CPU cycles (where available, e.g., "128K", "2M")

## System-Specific CPU Frequencies

Each system displays its historically accurate CPU frequency:

- **NES**: 1.8 MHz (1.789773 MHz NTSC)
- **Game Boy**: 4.2 MHz (4.194304 MHz)
- **Atari 2600**: 1.2 MHz (1.19 MHz)
- **PC**: Variable based on CPU model
  - 8086/8088: 4.8 MHz
  - 80286: 12.0 MHz
  - 80386: 20.0 MHz
  - 80486: 25.0-100.0 MHz
  - Pentium: 60.0-166.0 MHz
- **SNES**: 3.6 MHz (3.58 MHz)
- **N64**: 93.8 MHz (93.75 MHz)

## Instruction Pointer Formatting

The IP is now properly formatted based on address size:

- **16-bit addresses** (NES, GB): 4 hex digits (e.g., `$C000`)
- **20-bit addresses** (PC 8086): 5 hex digits (e.g., `$F0000`)
- **24-bit addresses** (SNES): 6 hex digits (e.g., `$F0A123`)
- **32-bit addresses** (N64, PC 386+): 8 hex digits (e.g., `$80000000`)

## Implementation Details

### Files Changed

1. **`crates/frontend/gui/src/status_bar.rs`**
   - Added `rendering_backend`, `cpu_freq_target`, and `cpu_freq_actual` fields
   - Updated rendering logic to display new fields
   - Improved IP formatting to handle different address sizes
   - Added comprehensive unit tests

2. **`crates/frontend/gui/src/main.rs`**
   - Added `get_instruction_pointer()` method to EmulatorSystem
   - Added `get_cpu_freq_target()` method to EmulatorSystem
   - Added `get_cpu_freq_actual()` method stub (infrastructure for future implementation)
   - Updated status bar update logic to use new methods

### Bug Fixes

The main bug was that IP was only being read from `RuntimeStats`, which only NES implements. Other systems return default (zero) values. The fix was to:

1. Create a new `get_instruction_pointer()` method that gets the IP from each system's debug info
2. Use this method to update the status bar IP instead of relying on RuntimeStats

## Testing

- ✅ All existing tests pass
- ✅ New unit tests added for status bar functionality
- ✅ Code builds successfully (debug and release)
- ✅ Clippy passes with no warnings
- ✅ Code is properly formatted

Manual testing guide available in `TESTING_STATUS_BAR.md`.

## Future Enhancements

The infrastructure is now in place to add actual CPU frequency tracking:

```rust
fn get_cpu_freq_actual(&self) -> Option<f64> {
    // Track cycles over time to calculate actual frequency
    // This would require adding cycle tracking in the main loop
    None
}
```

When implemented, the status bar will show both target and actual frequencies if they differ:
- `4.8/4.5MHz` - Running at 4.5 MHz instead of target 4.8 MHz
- `4.8MHz` - Running at target speed (or close enough)
