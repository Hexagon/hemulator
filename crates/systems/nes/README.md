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
  - **Cycle-accurate sprite 0 hit detection** with comprehensive edge case handling:
    - **Timing**: Flag set at PPU dot = X + 2 (2-cycle pipeline delay)
    - **Opaque pixels only**: Both sprite and background must be non-transparent (pattern != 0)
    - **X=255 boundary**: Hit CANNOT occur at rightmost pixel position
    - **Left clipping**: Respects PPUMASK bits 1 and 2 (leftmost 8 pixels)
    - **Rendering disabled**: No hit if background or sprites are disabled
    - **8x16 mode**: Works correctly for both top and bottom sprite halves
    - **Scrolling**: Accounts for background scroll position
    - **Odd frame timing**: Pixel-based detection unaffected by odd frame skip (dot 0 of scanline 0)
    - **Flag lifetime**: Cleared at pre-render scanline (dot 1) and VBlank start (compatibility)
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
  - **NTSC**: 262 scanlines per frame (60.1 Hz refresh rate)
    - 240 visible scanlines (0-239)
    - Post-render scanline (240)
    - 20 VBlank scanlines (241-260)
    - Pre-render scanline (261)
  - **PAL**: 312 scanlines per frame (50.0 Hz refresh rate)
    - 240 visible scanlines (0-239)
    - Post-render scanline (240)
    - 70 VBlank scanlines (241-310) - 3.3x longer than NTSC
    - Pre-render scanline (311)
  - **VBlank**: Starts at scanline 241 for both NTSC and PAL
  - **Timing Detection**: Automatic from iNES header (byte 9 for 1.0, byte 12 for 2.0)
  - **Dynamic Switching**: Can change timing mode at runtime via `set_timing()`

- **APU Timing**: Audio timing automatically adjusts to match ROM timing mode
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

### ROM Format Support

The emulator supports both **iNES 1.0** and **iNES 2.0** ROM formats:

**iNES 1.0** (Legacy):
- 8-bit mapper number (0-255)
- Basic timing detection via unofficial flags
- Limited metadata

**iNES 2.0** (Modern):
- 12-bit mapper number (0-4095) - supports extended mapper range
- 4-bit submapper number (0-15) - distinguishes mapper variants
- Official timing flags (NTSC/PAL/Dual/Dendy)
- Enhanced ROM size fields
- VS System and other metadata

The format is auto-detected from the header (byte 7, bits 2-3):
- iNES 1.0: bits 2-3 != `10`
- iNES 2.0: bits 2-3 == `10`

**Submapper Support**: When present in iNES 2.0 ROMs, the submapper number is stored and available for mapper implementations to distinguish between hardware variants (e.g., BNROM vs NINA-001 for mapper 34).

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

- **222+ total tests**:
  - PPU tests (scrolling, loopy registers, sprite handling, rendering)
    - Loopy register behavior (5 tests)
    - Sprite 0 hit comprehensive tests (10 tests):
      - Edge cases (x=255, clipping, transparent pixels, rendering disabled)
      - Timing precision (cycle-accurate, odd frame behavior)
      - Advanced scenarios (8x16 mode, scrolling, background transparency)
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

## ROM Database

The NES emulator includes a ROM database system to override incorrect or missing cartridge header information.

**Location**: `src/rom_db.rs`

### Purpose

Some NES ROM files have corrupted or incorrect headers due to:
- DiskDude! corruption (automated header fixing is already implemented)
- Incorrect mapper assignments
- Incorrect mirroring flags
- Homebrew ROMs not following iNES conventions

The ROM database allows overriding the mapper number and mirroring mode based on the ROM's CRC32 checksum.

### Adding Database Entries

To add a ROM to the database:

1. Calculate the CRC32 of the entire ROM file (including header):
   ```rust
   use emu_nes::rom_db::calculate_crc32;
   let crc32 = calculate_crc32(&rom_data);
   println!("CRC32: 0x{:08X}", crc32);
   ```

2. Determine the correct mapper and mirroring from:
   - BootGod's Database: http://bootgod.dyndns.org:7777/
   - NesCartDB: https://nescartdb.com/
   - NESdev Wiki: https://www.nesdev.org/

3. Add an entry to `ROM_DATABASE` in `src/rom_db.rs`:
   ```rust
   RomDbEntry::new(
       0x12345678,                      // CRC32 of full ROM file
       Some(4),                          // Override to mapper 4 (MMC3)
       Some(Mirroring::Horizontal),      // Override to horizontal mirroring
       Some("TLSROM"),                   // Board type (optional)
   ),
   ```

### How It Works

When a ROM is loaded:
1. CRC32 is calculated for the entire ROM file
2. The database is checked for a matching CRC32
3. If found, mapper and/or mirroring are overridden
4. Log messages indicate when overrides are applied

Example log output:
```
NES ROM DB: Overriding mapper 0 -> 4 for CRC32 0x12345678 (TLSROM)
NES ROM DB: Overriding mirroring Vertical -> Horizontal for CRC32 0x12345678 (TLSROM)
```

## Known Limitations

See [User Manual](https://hemulator.56k.guru/user/systems.html#nes-nintendo-entertainment-system) for user-facing limitations.

**Technical Limitations**:
- MMC2/MMC4 latch switching happens per-frame, not mid-scanline
- Some games requiring dot-level PPU rendering may not work perfectly

**Cycle-Accurate Features** (implemented):
- ✅ **NMI/VBlank timing**: Cycle-accurate with scanline 241, dot 1 precision
- ✅ **$2002 race condition**: Proper NMI suppression when reading PPUSTATUS at VBlank start
- ✅ **Sprite overflow bug**: Hardware-accurate m/n pointer increment bug with false positives/negatives
- ✅ **Sprite flags**: Cleared at exact cycle (scanline 261, dot 1)
- ✅ **Odd frame skip**: Scanline 0, dot 0 skipped on odd frames when rendering enabled

**Supported Edge Cases**:
- ✅ **Illegal/undocumented 6502 opcodes**: Full support for LAX, SAX, DCP, ISC, SLO, RLA, SRE, RRA with all addressing modes
- ✅ **Sprite 0 hit**: Cycle-accurate detection with comprehensive edge case handling (see detailed section below)
- ✅ **Sprite overflow**: Hardware-accurate with m/n pointer bug emulation
- ✅ **MMC3 IRQ timing**: Uses MMC3B/C behavior (counter decrements to 0 triggers IRQ)
- ✅ **DMC DMA**: Full memory read support with automatic sample playback
- ✅ **Cycle-accurate PPU**: 3:1 PPU-CPU clock ratio, 341 dots per scanline, 262 scanlines per frame

## Sprite 0 Hit - Deep Dive

Sprite 0 hit is a crucial hardware feature for mid-frame effects like split-screen scrolling. This implementation provides cycle-accurate detection with comprehensive edge case handling.

### Hardware Behavior

The sprite 0 hit flag (PPUSTATUS bit 6, `$2002`) is set when:
1. A **non-transparent pixel** of sprite 0 (OAM entry 0) overlaps
2. A **non-transparent pixel** of the background
3. During **visible scanline rendering** (scanlines 0-239, dots 2-257)

### Timing Precision

- **Detection**: Occurs during `render_scanline()` when checking pixel overlap
- **Pipeline Delay**: Flag is actually set 2 PPU cycles later (at dot = X + 2)
  - Example: Hit at pixel X=50 triggers flag at dot 52
  - This models the 2-cycle PPU pipeline delay in real hardware
- **Two-Stage Implementation**: 
  1. `render_scanline()`: Detects overlap, schedules pending hit at (scanline, X position)
  2. `tick()`: Sets flag when PPU dot counter reaches X + 2

### Edge Cases (All Tested)

| Condition | Behavior | Test Coverage |
|-----------|----------|---------------|
| **X=255** | Hit CANNOT occur at rightmost pixel | ✅ `test_sprite_0_hit_edge_cases` |
| **Left clipping** | Respects PPUMASK bits 1 & 2 (leftmost 8 pixels) | ✅ `test_sprite_0_hit_with_clipping` |
| **Transparent sprite** | Pattern value 0 = no hit | ✅ `test_sprite_0_hit_transparent_pixels` |
| **Transparent background** | Pattern value 0 = no hit | ✅ `test_sprite_0_hit_background_transparent` |
| **Background disabled** | PPUMASK bit 3 = 0, no hit | ✅ `test_sprite_0_hit_not_set_when_rendering_disabled` |
| **Sprites disabled** | PPUMASK bit 4 = 0, no hit | ✅ `test_sprite_0_hit_not_set_when_rendering_disabled` |
| **8x16 mode** | Works for both halves | ✅ `test_sprite_0_hit_8x16_mode` |
| **Scrolling** | Accounts for background position | ✅ `test_sprite_0_hit_with_scrolling` |
| **Odd frame** | Unaffected by dot 0 skip | ✅ `test_sprite_0_hit_odd_frame_timing` |
| **Cycle timing** | Triggers at exact dot (X + 2) | ✅ `test_sprite_0_hit_cycle_accurate_timing` |

### Odd Frame Timing

**Key Insight**: Sprite 0 hit is **pixel-based**, not dot-based.

On odd frames (when rendering is enabled):
- The PPU skips dot 0 of scanline 0
- This makes the frame 89341 dots instead of 89342 dots
- Rendering happens at dot 1 instead of dot 0

**However**, sprite 0 hit detection is based on:
- **Sprite X position** (0-255, pixel coordinate)
- **Background scroll** (pixel coordinates via v register)

Since hit detection uses pixel positions (not dot positions), the odd frame skip does **not** affect the sprite 0 hit X position. The hit occurs at the same pixel coordinate on both even and odd frames.

**Test Evidence**: `test_sprite_0_hit_odd_frame_timing` verifies that hit position is identical on even/odd frames.

### Flag Lifetime

- **Set**: During visible scanline rendering (scanlines 0-239)
- **Cleared**: Two places for compatibility
  1. **Pre-render scanline**: Dot 1 of scanline 261 (hardware behavior)
  2. **VBlank start**: Dot 1 of scanline 241 (compatibility fix for games like Battletoads)

The dual clearing ensures both hardware accuracy and game compatibility.

### References

- [NESdev Wiki - PPU rendering](https://www.nesdev.org/wiki/PPU_rendering)
- [NESdev Wiki - PPU OAM](https://www.nesdev.org/wiki/PPU_OAM)
- [NESdev Wiki - PPU frame timing](https://www.nesdev.org/wiki/PPU_frame_timing)

## Performance

- **Target**: 60 FPS (NTSC) / 50 FPS (PAL)
- **Typical**: Runs at full speed on modern CPUs
- **Single-threaded**: Uses one CPU core

### Performance Optimizations

The NES emulator includes cycle-accurate PPU timing with the following rendering optimizations:

**Implemented Optimizations**:
- ✅ **CHR Fetch Optimization**: Fast-path CHR reads during rendering bypass RefCell overhead while maintaining MMC2/MMC4 mapper compatibility
- ✅ **Background Tile Batching**: Processes background in 8-pixel tile chunks instead of per-pixel, reducing divisions and improving cache locality

**Cycle Accuracy**: All optimizations preserve perfect cycle accuracy. The emulator maintains exact timing regardless of display framerate:
- VBlank/NMI timing: Scanline 241, dot 1 (hardware-accurate)
- Sprite evaluation: Dot 192 (hardware-accurate)
- PPU clock ratio: 3× CPU clock (hardware-accurate)
- Low FPS does NOT affect emulation accuracy - time-based model preserves timing

**Future Performance Enhancements** (not yet implemented):
- **Sprite Pre-filtering**: Build per-scanline sprite lists to avoid checking all 64 sprites (5-10% improvement, requires careful timing preservation for sprite overflow flag)
- **PPU Tick Batching**: Batch multiple PPU ticks when no timing events pending (5-10% improvement, requires careful preservation of exact event timing)

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
3. **Documentation**: Update this README and [User Manual](https://hemulator.56k.guru/user/systems.html)
4. **Known Limitations**: Update limitations when features are added

## References

- **Architecture**: [ARCHITECTURE.md](../../../ARCHITECTURE.md)
- **User Manual**: [User Manual](https://hemulator.56k.guru/user/systems.html#nes-nintendo-entertainment-system)
- **Contributing**: [Contributing Guide](https://hemulator.56k.guru/developer/contributing.html)
- **NESDev Wiki**: https://www.nesdev.org/

## License

Same as the parent Hemulator project.
