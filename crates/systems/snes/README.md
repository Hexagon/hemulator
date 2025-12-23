# SNES Emulation - Super Nintendo Entertainment System

This crate implements Super Nintendo Entertainment System emulation for the Hemulator project.

**For overall architecture**, see [ARCHITECTURE.md](../../../ARCHITECTURE.md)

## Current Status

The SNES emulator is **in development** with basic CPU and minimal PPU Mode 0 support.

### What Works

- ✅ **CPU (65C816)** - Complete 16-bit CPU from `emu_core::cpu_65c816`
  - 256/256 opcodes implemented (100% complete)
  - 8/16-bit mode switching
  - 24-bit address space
- ✅ **Memory Bus** - 128KB WRAM, cartridge mapping
- ✅ **Cartridge Loading** - LoROM mapping with SMC header detection
- ✅ **PPU Mode 0** - Basic 4-layer 2bpp rendering
- ✅ **Save States** - CPU state serialization

### What's Missing

- ⏳ **PPU**: Only Mode 0 implemented
  - No sprites (OAM)
  - No scrolling
  - No windows, masks, or effects
  - No Modes 1-7
- ⏳ **APU (SPC700)**: Not implemented - no audio
- ⏳ **Controllers**: Input system not implemented
- ⏳ **HiROM**: Only LoROM mapping supported
- ⏳ **Enhancement Chips**: No SuperFX, DSP, SA-1, etc.

## Architecture

### Component Structure

```
SnesSystem
  └── SnesCpu (wraps Cpu65c816<SnesBus>)
      └── SnesBus (implements Memory65c816)
          ├── 128KB WRAM
          ├── SNES PPU (Mode 0 only)
          │   ├── 64KB VRAM
          │   ├── 256-color CGRAM (palette)
          │   └── 4 BG layers (2bpp)
          └── Cartridge (LoROM mapping)
              ├── ROM banks
              └── 32KB SRAM
```

### PPU Implementation

**Location**: `src/ppu.rs`

**Mode 0 Support** (4 BG layers, 2bpp each):

- 256x224 resolution
- 8x8 tiles with 4 colors per tile
- 8 palettes per layer
- Tile attributes (flip, palette selection)
- Layer priority rendering (BG4 → BG3 → BG2 → BG1)
- Transparent pixel handling

**NOT Implemented**:
- Modes 1-7
- Sprites
- Scrolling
- Windows/masks
- HDMA, mosaic, color math

### Memory Map

- **$00-$3F, $80-$BF**: WRAM mirrors, I/O, ROM
- **$7E-$7F**: Full 128KB WRAM
- **$8000-$FFFF**: Cartridge ROM (LoROM)
- **$2000-$5FFF**: Hardware registers (PPU, APU)

## Building

```bash
# Build SNES crate
cargo build --package emu_snes

# Run tests
cargo test --package emu_snes

# Run with specific ROM
cargo run --release -p emu_gui -- path/to/game.sfc
```

## Testing

The SNES crate includes basic tests:

- **17 total tests**:
  - Cartridge tests (loading, SMC header)
  - PPU tests (Mode 0 rendering, registers)
  - System tests (state management)

- **Smoke Test**: Uses `test_roms/snes/test.sfc` to verify basic functionality

## Usage Example

```rust
use emu_snes::SnesSystem;
use emu_core::System;

// Create system
let mut snes = SnesSystem::new();

// Load ROM
let rom_data = std::fs::read("game.sfc")?;
snes.mount("Cartridge", &rom_data)?;

// Run one frame
let frame = snes.step_frame()?;
```

## Known Limitations

See [MANUAL.md](../../../MANUAL.md#snes-super-nintendo-entertainment-system) for user-facing limitations.

**Status**: Very limited - can display simple Mode 0 graphics but most commercial games won't work due to missing features.

## Performance

- **Target**: ~60 FPS (NTSC)
- **Current**: CPU executes at correct speed
- **Single-threaded**: Uses one CPU core

## Future Improvements

**Short Term**:
- PPU Mode 1-7 support
- Sprite rendering (OAM)
- Scrolling implementation
- APU (SPC700 CPU + DSP)

**Medium Term**:
- Controller input
- HiROM mapping
- Save RAM support
- Additional PPU features (windows, HDMA)

**Long Term**:
- Enhancement chips (SuperFX, DSP, SA-1)
- Accurate timing
- Full compatibility

## Contributing

When adding SNES features:

1. **PPU Modes**: Add to `src/ppu.rs`
2. **APU**: Create `src/apu.rs` with SPC700 CPU
3. **Tests**: Add unit tests for new functionality
4. **Documentation**: Update this README and [MANUAL.md](../../../MANUAL.md)

## References

- **Architecture**: [ARCHITECTURE.md](../../../ARCHITECTURE.md)
- **User Manual**: [MANUAL.md](../../../MANUAL.md#snes-super-nintendo-entertainment-system)
- **Contributing**: [CONTRIBUTING.md](../../../CONTRIBUTING.md)
- **SNESdev Wiki**: https://www.snesdev.org/

## License

Same as the parent Hemulator project.
