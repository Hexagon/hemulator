# Atari 2600 Emulation

This crate implements Atari 2600 emulation for the Hemulator project.

**For overall architecture**, see [ARCHITECTURE.md](../../../ARCHITECTURE.md)

## Current Status

The Atari 2600 emulator is **fully working** with support for most common cartridge formats.

### What Works

- ✅ **CPU (6507)** - Uses `cpu_6502` from `emu_core` with 13-bit address bus
- ✅ **TIA** - Television Interface Adapter for video and audio
- ✅ **RIOT** - 6532 chip with RAM, I/O, and timer
- ✅ **Cartridge Banking** - 2K to 32K ROMs with multiple banking schemes
- ✅ **Controllers** - Joystick input support
- ✅ **Save States** - Complete state serialization

### Supported Cartridge Formats

- **2K ROM** - No banking (Combat)
- **4K ROM** - No banking (Pac-Man)
- **8K (F8)** - 2 banks (Asteroids, Missile Command)
- **12K (FA)** - 3 banks (CBS games)
- **16K (F6)** - 4 banks (Donkey Kong)
- **32K (F4)** - 8 banks (larger games)

## Architecture

### Component Structure

```
Atari2600System
  └── Atari2600Cpu (wraps Cpu6502<Atari2600Bus>)
      └── Atari2600Bus (implements Memory6502)
          ├── TIA (Television Interface Adapter)
          │   ├── Playfield (40-bit bitmap)
          │   ├── 2 Players (8-pixel sprites)
          │   ├── 2 Missiles
          │   ├── 1 Ball
          │   └── 2 Audio channels
          ├── RIOT (6532 chip)
          │   ├── 128 bytes RAM
          │   ├── I/O ports (joystick, console switches)
          │   └── Programmable timer
          └── Cartridge (ROM + banking logic)
```

### TIA Implementation

**Location**: `src/tia.rs`

The TIA handles both video and audio:

- **Video**:
  - 160x192 visible pixels (NTSC)
  - 128-color NTSC palette
  - Playfield: 40-bit bitmap (20 bits × 2 halves)
  - 2 Players: 8-pixel sprites with reflection
  - 2 Missiles: 1-pixel wide
  - 1 Ball: 1-pixel wide
  - Priority ordering configurable
  
- **Audio**:
  - 2 audio channels
  - Polynomial waveform generation (uses `PolynomialCounter` from `emu_core::apu`)
  - 16 waveform types per channel
  - Frequency and volume control

**Timing Model**: Frame-based rendering with scanline state latching

### RIOT Implementation

**Location**: `src/riot.rs`

The 6532 RIOT provides:

- **128 bytes RAM** with proper mirroring
- **Programmable timer** with 4 interval modes (1, 8, 64, 1024 clocks)
- **I/O ports**: 
  - SWCHA: Joystick input
  - SWCHB: Console switches (reset, select, difficulty)
- **Timer interrupt flag** (auto-clears on read)

### Cartridge Banking

**Location**: `src/cartridge.rs`

Supports multiple banking schemes:

- **F8 (8K)**: 2 banks, switch at $1FF8-$1FF9
- **FA (12K)**: 3 banks, switch at $1FF8-$1FFA
- **F6 (16K)**: 4 banks, switch at $1FF6-$1FF9
- **F4 (32K)**: 8 banks, switch at $1FF4-$1FFB

Auto-detection based on ROM size.

## Building

```bash
# Build Atari 2600 crate
cargo build --package emu_atari2600

# Run tests
cargo test --package emu_atari2600

# Run with specific ROM
cargo run --release -p emu_gui -- path/to/game.bin
```

## Testing

The Atari 2600 crate includes comprehensive tests:

- **45 total tests**:
  - TIA tests (rendering, registers, playfield)
  - RIOT tests (RAM, timer, I/O)
  - Cartridge tests (banking schemes)
  - System integration tests

- **Test ROMs**: Multiple test ROMs in `test_roms/atari2600/`:
  - `test.bin`: Basic playfield pattern
  - `checkerboard.bin`: Alternating playfield validation
  - `test_timer.bin`: RIOT timer and color cycling

## Usage Example

```rust
use emu_atari2600::Atari2600System;
use emu_core::System;

// Create system
let mut atari = Atari2600System::new();

// Load ROM
let rom_data = std::fs::read("game.bin")?;
atari.mount("Cartridge", &rom_data)?;

// Run one frame
let frame = atari.step_frame()?;
```

## Known Limitations

See [MANUAL.md](../../../MANUAL.md#atari-2600) for user-facing limitations.

**Technical Limitations**:
- Player/missile sizing (NUSIZ) stored but not applied
- Horizontal motion (HMxx) stored but not applied
- Collision detection registers return 0
- Frame-based rendering (not cycle-accurate)
- Some exotic banking schemes not implemented (DPC, FE, 3F, E0)

## Performance

- **Target**: ~60 FPS (NTSC)
- **Typical**: Runs at full speed on modern CPUs
- **Single-threaded**: Uses one CPU core

## Future Improvements

- Player/missile sizing and horizontal motion
- Collision detection
- Additional banking schemes (DPC, FE, 3F, E0)
- Cycle-accurate TIA rendering

## Contributing

When adding Atari 2600 features:

1. **Banking Schemes**: Add to `src/cartridge.rs`
2. **Tests**: Add unit tests for new functionality
3. **Documentation**: Update this README and [MANUAL.md](../../../MANUAL.md)

## References

- **Architecture**: [ARCHITECTURE.md](../../../ARCHITECTURE.md)
- **User Manual**: [MANUAL.md](../../../MANUAL.md#atari-2600)
- **Contributing**: [CONTRIBUTING.md](../../../CONTRIBUTING.md)
- **Stella Programmer's Guide**: Classic Atari 2600 documentation

## License

Same as the parent Hemulator project.
