# NES Emulation - Nintendo Entertainment System

This crate implements Nintendo Entertainment System emulation for the Hemulator project.

**For overall architecture**, see [ARCHITECTURE.md](../../../ARCHITECTURE.md)

## Current Status

The NES emulator is **fully working** with ~90%+ game coverage through 14 mapper implementations.

### What Works

- ✅ **CPU (6502)** - Complete instruction set from `emu_core::cpu_6502`
  - All official opcodes
  - 56 illegal/undocumented opcodes (LAX, SAX, DCP, ISC, SLO, RLA, SRE, RRA)
- ✅ **PPU (2C02)** - Full PPU emulation with background, sprites, scrolling
- ✅ **APU (RP2A03)** - Complete audio with all 5 channels
- ✅ **Mappers** - 15 mappers including complex MMC5
- ✅ **Controllers** - Full input support
- ✅ **Save States** - Complete state serialization
- ✅ **PAL/NTSC** - Auto-detection and timing support

### Supported Mappers

The NES emulator supports 15 mappers covering a wide range of NES games:

- **Mapper 0 (NROM)** - Basic mapper (~10% of games)
- **Mapper 1 (MMC1/SxROM)** - Tetris, Metroid, Zelda (~28% of games)
- **Mapper 2 (UxROM)** - Mega Man, Castlevania (~11% of games)
- **Mapper 3 (CNROM)** - Gradius, Paperboy (~6.4% of games)
- **Mapper 4 (MMC3/TxROM)** - Super Mario Bros. 3 (~24% of games)
- **Mapper 5 (MMC5)** - Castlevania 3 (US), Just Breed, Laser Invasion
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

- **Scrolling**:
  - Proper loopy register system (v, t, fine_x)
  - Accurate PPUSCROLL ($2005) behavior with separate coarse/fine components
  - Correct PPUADDR ($2006) interaction with scroll registers
  - Shared write latch between $2005 and $2006
  - Nametable selection via v register bits 10-11 (hardware-accurate loopy registers)
  - v→t synchronization at scanline boundaries for mid-frame scroll changes
  
- **Rendering**:
  - 256x240 resolution (NTSC) / 256x240 (PAL)
  - Background rendering with attribute tables
  - 64 sprites (8x8 or 8x16 modes)
  - Accurate sprite 0 hit detection with all edge cases:
    - Checks for opaque pixel overlap
    - Respects x=255 boundary (no hit)
    - Respects left 8-pixel clipping
  - Sprite overflow detection (>8 sprites per scanline)
  - 8-sprite-per-scanline hardware limit
  - Correct sprite priority (front-to-back buffer fill)
  - **Software Renderer**: CPU-based tile/sprite rendering (default)
  - **OpenGL Renderer**: GPU-accelerated rendering (optional, via `opengl` feature)
  
- **Memory**:
  - 2KB internal VRAM for nametables (4KB for four-screen)
  - 32-byte palette RAM (8 background + 8 sprite palettes)
  - 8KB CHR memory (ROM or RAM)
  - 256-byte OAM (Object Attribute Memory)
  
- **Timing Model**: Frame-based rendering with scanline support
  - Renders complete 256x240 frames on-demand
  - Scanline rendering for mid-frame register changes
  - Suitable for ~90%+ of games

### APU Implementation

**Location**: `src/apu.rs`

Uses reusable components from `emu_core::apu`:

- **Pulse 1** (`PulseChannel` + `SweepUnit`): Square wave with sweep
- **Pulse 2** (`PulseChannel`): Square wave
- **Triangle** (`TriangleChannel`): 32-step triangle wave
- **Noise** (`NoiseChannel`): LFSR-based noise
- **DMC**: Delta modulation channel with full memory read support
  - Sample playback from CPU memory via DMA
  - Automatic memory reads at configurable sample rates
  - IRQ generation on sample completion
  - Loop support for continuous playback

**Frame Sequencer**: 240Hz timing for envelopes, length counters, and sweep

**Audio Output**: 44.1 kHz sample rate with improved non-linear mixing
- Non-linear APU mixer for authentic NES sound
- Gentle dynamic compression curve
- Enhanced mid-range dynamics

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

- **215+ total tests**:
  - PPU tests (scrolling, loopy registers, sprite handling, rendering)
    - Loopy register behavior (5 tests)
    - Sprite 0 hit edge cases (2 tests)
    - Sprite overflow detection (3 tests)
    - Sprite priority and rendering (8 tests)
    - Nametable scrolling (5 tests)
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

See [User Manual](https://hemulator.56k.guru/user/manual.html#nes-nintendo-entertainment-system) for user-facing limitations.

**Technical Limitations**:
- Frame-based timing (not cycle-accurate)
- MMC2/MMC4 latch switching happens per-frame, not mid-scanline
- Some games requiring precise PPU timing may not work perfectly

## Performance

- **Target**: 60 FPS (NTSC) / 50 FPS (PAL)
- **Typical**: Runs at full speed on modern CPUs
- **Single-threaded**: Uses one CPU core

## Future Improvements

- Additional mappers for expansion audio:
  - VRC6 (Mapper 24) - Konami expansion audio
  - FME-7 (Mapper 69) - Sunsoft expansion audio
  - Namcot 106 (Mapper 19) - Namco expansion audio
- High-pass and low-pass audio filters for even better sound quality

## Contributing

When adding NES features:

1. **Mappers**: Add to `src/mappers/`, implement `Mapper` trait
2. **Tests**: Add unit tests for new functionality
3. **Documentation**: Update this README and [User Manual](https://hemulator.56k.guru/user/manual.html)
4. **Known Limitations**: Update limitations when features are added

## References

- **Architecture**: [ARCHITECTURE.md](../../../ARCHITECTURE.md)
- **User Manual**: [User Manual](https://hemulator.56k.guru/user/manual.html#nes-nintendo-entertainment-system)
- **Contributing**: [Contributing Guide](https://hemulator.56k.guru/developer/contributing.html)
- **NESDev Wiki**: https://www.nesdev.org/

## License

Same as the parent Hemulator project.
