# Game Boy Emulation

This crate implements Game Boy (DMG) and Game Boy Color (CGB) emulation for the Hemulator project.

**For overall architecture**, see [ARCHITECTURE.md](../../../ARCHITECTURE.md)

## Current Status

The Game Boy emulator is **fully working** with ~99% game coverage through MBC0/1/2/3/5 and HuC1 support, plus full CGB color support and WRAM banking.

### What Works

- ✅ **CPU (LR35902)** - Complete Sharp LR35902 CPU from `emu_core::cpu_lr35902`
- ✅ **PPU** - Full DMG and CGB PPU with background, window, sprites
- ✅ **CGB Color** - 15-bit RGB color palettes (8 BG + 8 OBJ)
- ✅ **Palette Initialization** - DMG-compatible defaults for early CGB games (Pokemon Yellow, etc.)
- ✅ **VRAM Banking** - 2 banks of 8KB for CGB mode
- ✅ **WRAM Banking** - 8 banks of 4KB (32KB total) with SVBK register for CGB mode
- ✅ **Tile Attributes** - CGB palette selection, VRAM banking, flipping
- ✅ **APU** - Complete audio with all 4 channels
- ✅ **Mappers** - MBC0, MBC1, MBC2, MBC3, MBC5, HuC1 (~97% coverage)
- ✅ **Speed Switching** - KEY1 register for CGB double-speed mode
- ✅ **Joypad** - Full input support
- ✅ **Timer** - DIV, TIMA, TMA, TAC with interrupts
- ✅ **Interrupts** - VBlank and Timer interrupts
- ✅ **Save States** - Complete state serialization

### Supported Memory Bank Controllers

- **MBC0** (No mapper): 32KB ROMs
- **MBC1**: Most common (~70% of games)
  - Up to 2MB ROM, 32KB RAM
  - ROM/RAM banking modes
- **MBC2**: Built-in RAM mapper (~1% of games)
  - Up to 256KB ROM, 512×4 bits built-in RAM
  - Address bit 8 determines register function
- **MBC3**: Popular for games with saves (~15% of games)
  - Up to 2MB ROM, 32KB RAM
  - **Working RTC (Real-Time Clock)** - time counting fully functional
- **MBC5**: Advanced mapper (~10% of games)
  - Up to 8MB ROM, 128KB RAM
  - 9-bit ROM banking
- **HuC1**: Hudson Soft mapper (<1% of games)
  - Up to 1MB ROM, 32KB RAM
  - IR sensor support (stubbed)
  - Used by: Pocket Bomberman, Tamagotchi games

## Architecture

### Component Structure

```
GbSystem
  └── GbCpu (wraps CpuLr35902<GbBus>)
      └── GbBus (implements MemoryLr35902)
          ├── 32KB Work RAM (WRAM) with banking (CGB)
          │   ├── Bank 0: 0xC000-0xCFFF (fixed)
          │   └── Banks 1-7: 0xD000-0xDFFF (switchable via SVBK)
          ├── 127 bytes High RAM (HRAM)
          ├── GB PPU
          │   ├── 8KB VRAM
          │   ├── 160-byte OAM (40 sprites)
          │   └── Background/Window/Sprite rendering
          ├── GB APU
          │   ├── Pulse 1 (with sweep)
          │   ├── Pulse 2
          │   ├── Wave (custom waveform)
          │   └── Noise
          ├── Joypad (matrix input)
          ├── Timer (DIV, TIMA, TMA, TAC)
          └── Cartridge (ROM + RAM + MBC)
```

### PPU Implementation

**Location**: `src/ppu.rs`, `src/ppu_renderer.rs`

Implements DMG (original Game Boy) and CGB (Game Boy Color) modes with a flexible renderer architecture:

- **Resolution**: 160x144 pixels
- **Tile System**:
  - 8x8 pixel tiles, 2 bits per pixel (4 colors)
  - Two tile data areas (unsigned $8000-$8FFF, signed $8800-$97FF)
  - Two tilemap areas ($9800-$9BFF, $9C00-$9FFF)
  - CGB: VRAM banking (2 banks of 8KB)
- **Layers**:
  - Background with scrolling (SCX, SCY)
  - Window layer (WX, WY)
  - 40 sprites (8x8 or 8x16 modes)
- **Features**:
  - Sprite flipping (horizontal/vertical)
  - Sprite priority (BG priority flag)
  - **Hardware-accurate sprite selection and priority**:
    - **Selection** (which 10 sprites per scanline): OAM order for both DMG and CGB
    - **Rendering priority** (which sprite appears on top):
      - DMG: Lower X coordinate wins, OAM order as tiebreaker
      - CGB: OAM order only (X coordinate irrelevant)
  - **Hardware-accurate scrolling**:
    - SCX/SCY with modulo 256 wrapping
    - Seamless tilemap boundary wrapping
  - 10 sprites per scanline limit (hardware accurate)
  - DMG: Monochrome palettes (BGP, OBP0, OBP1)
  - CGB: 15-bit RGB color palettes (8 BG, 8 OBJ)
  - CGB: Tile attributes (palette, VRAM bank, flip)
  - CGB: Sprite attributes (palette, VRAM bank)
  - Automatic mode detection (DMG vs CGB)
  - **Palette Initialization**:
    - CGB compatibility mode (flag 0x80): DMG-compatible greenish palette (White, Light Green, Dark Green, Black)
    - CGB-only mode (flag 0xC0): White default palette (game sets its own)
    - Ensures early CGB games (Pokemon Yellow, etc.) display correctly without boot ROM
- **Rendering**:
  - **Software Renderer**: CPU-based tile/sprite rendering (default)
  - **Hardware Renderer**: GPU-accelerated rendering (future work)
  - Follows `emu_core::renderer::Renderer` trait pattern
- **Timing**: Frame-based rendering (~59.73 Hz)

### APU Implementation

**Location**: `src/apu.rs`

Uses reusable components from `emu_core::apu`:

- **Pulse 1** (`PulseChannel` + `SweepUnit`): Square wave with sweep
- **Pulse 2** (`PulseChannel`): Square wave
- **Wave** (`WaveChannel`): 32×4-bit programmable waveform
- **Noise** (`NoiseChannel`): 7-bit or 15-bit LFSR modes

**Frame Sequencer**: 512 Hz timing controller

**Audio Output**: 44.1 kHz sample rate with panning and volume control

## Building

```bash
# Build Game Boy crate
cargo build --package emu_gb

# Run tests
cargo test --package emu_gb

# Run with specific ROM
cargo run --release -p emu_gui -- path/to/game.gb
```

## Testing

The Game Boy crate includes comprehensive tests:

- **137 total tests**:
  - PPU tests (rendering, registers, scrolling, sprite priority, sprite selection)
  - APU tests (all channels, registers)
  - System tests (reset, state management, controller input, joypad integration)
  - Mapper tests (MBC0/1/2/3/5, HuC1)
  - Timer tests (DIV, TIMA overflow, interrupts)
  - Renderer tests (software renderer)
  - WRAM banking tests (CGB mode, all banks, echo RAM, DMG mode, boundaries)

- **Smoke Tests**: Uses `test_roms/gb/test.gb` and `test_roms/gbc/test.gbc` to verify basic functionality

## Usage Example

```rust
use emu_gb::GbSystem;
use emu_core::System;

// Create system
let mut gb = GbSystem::new();

// Load ROM
let rom_data = std::fs::read("game.gb")?;
gb.mount("Cartridge", &rom_data)?;

// Run one frame
let frame = gb.step_frame()?;
```

## Known Limitations

See [User Manual](https://hemulator.56k.guru/user/manual.html#game-boy--game-boy-color) for user-facing limitations.

**Technical Limitations**:
- **Frame-based timing**: Not cycle-accurate - renders full frames at once rather than scanline-by-scanline. Suitable for ~99% of games.
- **Mid-scanline effects**: Register changes within a single scanline are not supported. However, scanline split effects (changing registers between scanlines using STAT interrupts) work correctly.

**Link Cable Limitations**:
- Serial transfer registers (0xFF01, 0xFF02) are implemented with loopback mode
- External link cable connections not supported
- Multiplayer and trading features require actual link cable hardware emulation

**Infrared Port Limitations**:
- RP register (0xFF56) implemented with read/write support
- LED control bits (6-7) accessible
- Actual IR communication hardware not emulated
- IR-based features (Pokemon Mystery Gift, etc.) won't communicate with external devices

**Unimplemented Mappers** (rare, <3% of games combined):
- MBC6 (Game Boy Camera only)
- MBC7 (tilt sensor games: Kirby Tilt 'n' Tumble, Command Master)
- HuC3 (Robopon games)
- MMM01 (multi-game cartridges)
- TAMA5 (Tamagotchi 3)

## Performance

- **Target**: ~59.73 FPS
- **Typical**: Runs at full speed on modern CPUs
- **Single-threaded**: Uses one CPU core

## Future Improvements

- Cycle-accurate timing
- Link cable emulation
- Boot ROM support

## Contributing

When adding Game Boy features:

1. **Mappers**: Add to `src/mappers/`, implement `Mapper` trait
2. **Tests**: Add unit tests for new functionality
3. **Documentation**: Update this README and [User Manual](https://hemulator.56k.guru/user/manual.html)

## References

- **Architecture**: [ARCHITECTURE.md](../../../ARCHITECTURE.md)
- **User Manual**: [User Manual](https://hemulator.56k.guru/user/manual.html#game-boy--game-boy-color)
- **Contributing**: [Contributing Guide](https://hemulator.56k.guru/developer/contributing.html)
- **Pan Docs**: https://gbdev.io/pandocs/

## License

Same as the parent Hemulator project.
