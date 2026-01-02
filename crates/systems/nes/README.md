# NES Emulation - Nintendo Entertainment System

This crate implements Nintendo Entertainment System emulation for the Hemulator project.

**For overall architecture**, see [ARCHITECTURE.md](../../../docs/ARCHITECTURE.md)

## Current Status

The NES emulator is **fully working** with ~90%+ game coverage through 14 mapper implementations.

### What Works

- ✅ **CPU (6502)** - Complete instruction set from `emu_core::cpu_6502`
- ✅ **PPU (2C02)** - Full PPU emulation with background, sprites, scrolling
- ✅ **APU (RP2A03)** - Complete audio with all 5 channels
- ✅ **Mappers** - 14 mappers covering ~90%+ of games
- ✅ **Controllers** - Full input support
- ✅ **Save States** - Complete state serialization
- ✅ **PAL/NTSC** - Auto-detection and timing support

### Supported Mappers

The NES emulator supports 14 mappers covering approximately **90%+ of all NES games**:

- **Mapper 0 (NROM)** - Basic mapper (~10% of games)
- **Mapper 1 (MMC1/SxROM)** - Tetris, Metroid, Zelda (~28% of games)
- **Mapper 2 (UxROM)** - Mega Man, Castlevania (~11% of games)
- **Mapper 3 (CNROM)** - Gradius, Paperboy (~6.4% of games)
- **Mapper 4 (MMC3/TxROM)** - Super Mario Bros. 3 (~24% of games)
- **Mapper 7 (AxROM)** - Battletoads (~3.1% of games)
- **Mapper 9 (MMC2)** - Punch-Out!!
- **Mapper 10 (MMC4)** - Fire Emblem (Japan)
- **Mapper 11 (Color Dreams)** - Color Dreams games (~1.3% of games)
- **Mapper 34 (BNROM)** - Deadly Towers
- **Mapper 66 (GxROM)** - SMB + Duck Hunt (~1.2% of games)
- **Mapper 71 (Camerica)** - Fire Hawk (~0.6% of games)
- **Mapper 79 (NINA-03/06)** - AVE games
- **Mapper 206 (Namco 118)** - Dragon Spirit (~1.8% of games)

## Architecture

### Component Structure

```
NesSystem
  └── NesCpu (wraps Cpu6502<NesMemory>)
      └── NesMemory (implements Memory6502)
          ├── 2KB CPU RAM
          ├── NES PPU (2C02)
          │   ├── 2KB VRAM (nametables)
          │   ├── 32-byte palette RAM
          │   ├── 8KB CHR memory
          │   └── 256-byte OAM (sprites)
          ├── NES APU (RP2A03)
          │   ├── Pulse 1 (with sweep)
          │   ├── Pulse 2
          │   ├── Triangle
          │   ├── Noise
          │   └── DMC
          ├── Controllers (2x)
          └── Mapper (cartridge banking)
```

### PPU Implementation

**Location**: `src/ppu.rs`, `src/ppu_renderer.rs`, `src/ppu_renderer_opengl.rs`

The 2C02 PPU implements:

- **Rendering**:
  - 256x240 resolution (NTSC) / 256x240 (PAL)
  - Background rendering with attribute tables
  - 64 sprites (8x8 or 8x16 modes)
  - Sprite 0 hit detection
  - Sprite overflow detection
  - Horizontal and vertical scrolling
  - **Software Renderer**: CPU-based tile/sprite rendering (default)
  - **OpenGL Renderer**: GPU-accelerated rendering (optional, via `opengl` feature)
  
- **Memory**:
  - 2KB internal VRAM for nametables
  - 32-byte palette RAM (8 background + 8 sprite palettes)
  - 8KB CHR memory (ROM or RAM)
  - 256-byte OAM (Object Attribute Memory)
  
- **Timing Model**: Frame-based rendering
  - Renders complete 256x240 frames on-demand
  - Scanline rendering for mapper CHR switching
  - Suitable for most games

### APU Implementation

**Location**: `src/apu.rs`

Uses reusable components from `emu_core::apu`:

- **Pulse 1** (`PulseChannel` + `SweepUnit`): Square wave with sweep
- **Pulse 2** (`PulseChannel`): Square wave
- **Triangle** (`TriangleChannel`): 32-step triangle wave
- **Noise** (`NoiseChannel`): LFSR-based noise
- **DMC**: Delta modulation channel (basic implementation)

**Frame Sequencer**: 240Hz timing for envelopes, length counters, and sweep

**Audio Output**: 44.1 kHz sample rate, mixed to stereo

### Mapper System

**Location**: `src/mappers/`

Each mapper handles:
- PRG ROM banking (program code)
- CHR ROM/RAM banking (graphics)
- Mirroring control (horizontal, vertical, single-screen)
- IRQ generation (MMC3, MMC5)
- CHR latch switching (MMC2, MMC4)

**Mapper Selection**: Auto-detected from iNES header

## Building

```bash
# Build NES crate (software renderer only)
cargo build --package emu_nes

# Build NES crate with OpenGL renderer support
cargo build --package emu_nes --features opengl

# Run tests
cargo test --package emu_nes

# Run tests with OpenGL feature
cargo test --package emu_nes --features opengl

# Run with specific ROM
cargo run --release -p emu_gui -- path/to/game.nes
```

## Testing

The NES crate includes comprehensive tests:

- **130 total tests**:
  - APU tests (pulse, triangle, noise, sweep, frame counter)
  - Mapper tests (all 14 mappers)
  - PPU tests (rendering, registers, scrolling)
  - System integration tests

- **Smoke Test**: Uses `test_roms/nes/test.nes` to verify:
  - ROM loading
  - CPU execution
  - PPU rendering
  - Checkerboard pattern output

## Usage Example

```rust
use emu_nes::NesSystem;
use emu_core::System;

// Create system
let mut nes = NesSystem::new();

// Load ROM
let rom_data = std::fs::read("game.nes")?;
nes.mount("Cartridge", &rom_data)?;

// Run one frame
let frame = nes.step_frame()?;

// Access audio samples
let samples = nes.audio_samples();
```

## Known Limitations

See [MANUAL.md](../../../docs/MANUAL.md#nes-nintendo-entertainment-system) for user-facing limitations.

**Technical Limitations**:
- Frame-based timing (not cycle-accurate)
- MMC2/MMC4 latch switching happens per-frame, not mid-scanline
- Some games requiring precise PPU timing may not work perfectly

## Performance

- **Target**: 60 FPS (NTSC) / 50 FPS (PAL)
- **Typical**: Runs at full speed on modern CPUs
- **Single-threaded**: Uses one CPU core

## Future Improvements

- Cycle-accurate PPU rendering
- Additional mappers (MMC5, VRC6, etc.)
- Accurate sprite evaluation timing
- Enhanced audio (DMC improvements, better filtering)

## Contributing

When adding NES features:

1. **Mappers**: Add to `src/mappers/`, implement `Mapper` trait
2. **Tests**: Add unit tests for new functionality
3. **Documentation**: Update this README and [MANUAL.md](../../../docs/MANUAL.md)
4. **Known Limitations**: Update limitations when features are added

## References

- **Architecture**: [ARCHITECTURE.md](../../../docs/ARCHITECTURE.md)
- **User Manual**: [MANUAL.md](../../../docs/MANUAL.md#nes-nintendo-entertainment-system)
- **Contributing**: [CONTRIBUTING.md](../../../docs/CONTRIBUTING.md)
- **NESDev Wiki**: https://www.nesdev.org/

## License

Same as the parent Hemulator project.
