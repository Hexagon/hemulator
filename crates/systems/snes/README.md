# SNES Emulation - Super Nintendo Entertainment System

This crate implements Super Nintendo Entertainment System emulation for the Hemulator project.

**For overall architecture**, see [ARCHITECTURE.md](../../../ARCHITECTURE.md)

## Current Status

The SNES emulator is **functional** with CPU, PPU Modes 0 & 1, sprites, scrolling, and controller support.

### What Works

- ✅ **CPU (65C816)** - Complete 16-bit CPU from `emu_core::cpu_65c816`
  - 256/256 opcodes implemented (100% complete)
  - 8/16-bit mode switching
  - 24-bit address space
- ✅ **Memory Bus** - 128KB WRAM, cartridge mapping
- ✅ **Cartridge Loading** - LoROM mapping with SMC header detection
- ✅ **PPU Mode 0** - 4-layer 2bpp rendering (4 colors per tile)
- ✅ **PPU Mode 1** - 2-layer 4bpp + 1-layer 2bpp rendering (most common mode)
- ✅ **Sprites (OAM)** - 128 sprites with 4bpp, multiple size modes
- ✅ **Scrolling** - Full horizontal and vertical scrolling on all BG layers
- ✅ **Controllers** - Full SNES controller support (A, B, X, Y, L, R, Start, Select, D-pad)
- ✅ **Save States** - CPU state serialization

### What's Missing

- ⏳ **PPU**: Modes 2-7 not implemented
  - No windows, masks, or effects
  - No HDMA
  - No mosaic or color math
- ⏳ **APU (SPC700)**: Not implemented - no audio
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
- 8 palettes per layer (32 colors total)
- Tile attributes (flip, palette selection)
- Layer priority rendering (BG4 → BG3 → BG2 → BG1)
- Transparent pixel handling
- Full scrolling support on all layers

**Mode 1 Support** (2 BG layers 4bpp, 1 BG layer 2bpp):

- 256x224 resolution
- BG1/BG2: 8x8 tiles with 16 colors per tile (4bpp)
- BG3: 8x8 tiles with 4 colors per tile (2bpp)
- 8 palettes per layer
- Tile attributes (flip, palette selection)
- Layer priority rendering (BG3 → BG2 → BG1)
- Full scrolling support on all layers
- **Most common mode in commercial games**

**Sprite Support** (OAM):

- 128 sprites total
- 4bpp (16 colors per sprite)
- 8 sprite palettes (CGRAM 128-255)
- Multiple size modes (8x8/16x16, 8x8/32x32, etc.)
- Horizontal and vertical flipping
- Priority-based rendering (sprite 127 → sprite 0)
- Configurable VRAM base address

**NOT Implemented**:
- Modes 2-7
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

The SNES crate includes comprehensive tests:

- **34 total tests**:
  - Cartridge tests (loading, SMC header)
  - PPU tests (Modes 0 & 1, scrolling, sprites, OAM registers)
  - Controller tests (serial I/O, auto-read, button mapping)
  - System tests (state management)

- **Smoke Test**: Uses `test_roms/snes/test.sfc` to verify basic functionality

## Usage Example

```rust
use emu_snes::{SnesSystem, controller};
use emu_core::System;

// Create system
let mut snes = SnesSystem::new();

// Load ROM
let rom_data = std::fs::read("game.sfc")?;
snes.mount("Cartridge", &rom_data)?;

// Set controller input
snes.set_controller(0, controller::A | controller::START);

// Run one frame
let frame = snes.step_frame()?;
```

### Controller Button Constants

```rust
use emu_snes::controller;

// Face buttons
controller::A       // 0x0080
controller::B       // 0x8000
controller::X       // 0x0040
controller::Y       // 0x4000

// Shoulder buttons
controller::L       // 0x0020
controller::R       // 0x0010

// System buttons
controller::START   // 0x1000
controller::SELECT  // 0x2000

// D-pad
controller::UP      // 0x0800
controller::DOWN    // 0x0400
controller::LEFT    // 0x0200
controller::RIGHT   // 0x0100
```

## Known Limitations

See [MANUAL.md](../../../MANUAL.md#snes-super-nintendo-entertainment-system) for user-facing limitations.

**Status**: Functional - can run games using Mode 0 or Mode 1 with sprites and controllers. Missing only audio and advanced PPU modes.

## Performance

- **Target**: ~60 FPS (NTSC)
- **Current**: CPU executes at correct speed
- **Single-threaded**: Uses one CPU core

## Future Improvements

**Short Term**:
- PPU Mode 2-7 support
- APU (SPC700 CPU + DSP)
- APU (SPC700 CPU + DSP)

**Medium Term**:
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
