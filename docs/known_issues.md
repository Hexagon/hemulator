# Known Issues and Future Improvements

## Graphics Issues

### Tetris (Mapper 1/MMC1)
- **Status**: Requires testing with actual ROM to verify
- **Potential Issues**:
  - Character rendering artifacts
  - Color palette issues
  - **Investigation Steps**:
    1. Verify CHR banking is switching correctly for both 4KB and 8KB modes
    2. Check that mirroring changes during gameplay are applied properly
    3. Verify palette writes are being captured correctly
    4. Test with EMU_LOG_PPU_WRITES=1 to see if palette/CHR updates are happening

### Super Mario Bros. 3 (Mapper 4/MMC3)
- **Status**: Requires testing with actual ROM to verify
- **Potential Issues**:
  - Game may not be starting correctly
  - IRQ timing might need refinement
  - **Investigation Steps**:
    1. Verify MMC3 PRG bank initialization (banks 0,1,6,7 setup)
    2. Check IRQ counter behavior - needs accurate scanline counting
    3. Verify CHR banking mode switching
    4. Test with debug logging to see if IRQ fires are happening at expected times

## General PPU Improvements Needed

1. **Scrolling**:
   - Current implementation is simplified
   - Fine X scroll register not fully implemented
   - PPUADDR/PPUSCROLL timing not cycle-accurate

2. **Sprite 0 Hit**:
   - Not implemented
   - Required for split-screen effects in many games

3. **Timing**:
   - Frame timing is approximate
   - PPU cycle-accurate behavior would improve compatibility

4. **Register Behavior**:
   - PPUSTATUS read side effects are minimal
   - PPUDATA read buffering is basic

## Mapper Improvements

### MMC1 (Mapper 1)
- Serial write implementation seems correct
- PRG/CHR banking logic verified with tests
- Potential areas to check:
  - Consecutive write timing (some games may be sensitive)
  - Reset behavior timing

### MMC3 (Mapper 4)
- IRQ counter implementation follows spec
- Potential improvements:
  - A12 edge detection accuracy (currently works via PPU CHR fetches)
  - IRQ reload timing edge cases

## Testing Without ROMs

Since we cannot include ROMs in the repository:
1. Unit tests verify mapper logic independently
2. PPU rendering logic has been reviewed for correctness
3. Integration testing requires users to provide their own legal ROM files

## Configuration Interface (Task 5)

Future work should include:
- Key binding configuration
- Window scale/resolution options
- Audio volume control
- Save state management UI
- Controller mapping

Implementing this would require:
1. Adding a configuration struct
2. Serializing/deserializing config to JSON
3. Adding UI overlay or separate config window
4. Hooking up configuration changes to the runtime
