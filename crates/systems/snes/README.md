# SNES Emulation - Super Nintendo Entertainment System

This crate implements Super Nintendo Entertainment System emulation for the Hemulator project.

**For overall architecture**, see [ARCHITECTURE.md](../../../docs/ARCHITECTURE.md)

## References

This implementation follows specifications from the **SNESdev Wiki**:
- **Main Wiki**: https://snes.nesdev.org/wiki/SNESdev_Wiki
- **65C816 CPU**: https://snes.nesdev.org/wiki/65c816_reference
- **PPU Registers**: https://snes.nesdev.org/wiki/PPU_registers
- **CPU Registers**: https://snes.nesdev.org/wiki/CPU_registers
- **Memory Map**: https://snes.nesdev.org/wiki/Memory_map
- **DMA & HDMA**: https://snes.nesdev.org/wiki/DMA_&_HDMA
- **Timing**: https://snes.nesdev.org/wiki/Timing

## Current Status

The SNES emulator supports basic gameplay with complete CPU, full DMA/HDMA, both LoROM and HiROM cartridge support, SPC700 APU processor, and PPU rendering for modes 0-7 (with limitations). Modes 0 and 1 are fully complete. Modes 2-7 have basic rendering but missing advanced features (offset-per-tile, hi-res, Mode 7 matrix). Audio processor (SPC700) is fully implemented but DSP (sound generation) is not, so games run silently.

### What Works

#### CPU & Memory
- ✅ **CPU (65C816)** - Complete 16-bit CPU from `emu_core::cpu_65c816`
  - 256/256 opcodes implemented (100% complete)
  - 8/16-bit mode switching (M and X flags)
  - 24-bit address space (16MB addressable)
  - Native and emulation modes
  - Reference: [65c816 Reference](https://snes.nesdev.org/wiki/65c816_reference)

- ✅ **Memory Bus** - Full SNES memory map implementation
  - 128KB WRAM ($7E0000-$7FFFFF)
  - Shadow RAM at $0000-$1FFF in banks $00-$3F and $80-$BF
  - Hardware registers ($2100-$21FF, $4000-$43FF)
  - Reference: [Memory Map](https://snes.nesdev.org/wiki/Memory_map)

- ✅ **Cartridge Loading** - Both LoROM and HiROM with auto-detection
  - LoROM: 32KB banks at $8000-$FFFF per bank
  - HiROM: Full 64KB banks with linear addressing
  - SMC header detection and removal
  - SRAM support for both modes
  - Reference: [ROM File Formats](https://snes.nesdev.org/wiki/ROM_file_formats)

#### PPU (Picture Processing Unit)
- ⚠️ **Background Modes** - Basic rendering for all modes, some features missing
  - **Mode 0**: ✅ Complete - 4 BG layers, 2bpp each (4 colors per tile)
  - **Mode 1**: ✅ Complete - 2 BG layers 4bpp + 1 BG layer 2bpp (most common commercial mode)
  - **Mode 2**: ⚠️ Partial - 2 BG layers, 4bpp each (offset-per-tile NOT implemented)
  - **Mode 3**: ✅ Complete - BG1 8bpp (256 colors), BG2 4bpp (16 colors)
  - **Mode 4**: ⚠️ Partial - BG1 8bpp, BG2 2bpp (offset-per-tile NOT implemented)
  - **Mode 5**: ⚠️ Partial - BG1 4bpp, BG2 2bpp (renders at 256px, NOT true 512px hi-res)
  - **Mode 6**: ⚠️ Partial - BG1 4bpp (renders at 256px, offset-per-tile NOT implemented)
  - **Mode 7**: ⚠️ Partial - 8bpp rendering only (matrix transformation NOT implemented)
  - Reference: [PPU Overview](https://snes.nesdev.org/wiki/PPU_registers)

- ✅ **Sprites (OAM)** - Complete sprite system
  - 128 sprites with 4bpp (16 colors per sprite)
  - Multiple size modes (8x8, 16x16, 32x32, 64x64)
  - Priority levels (0-3)
  - Hardware-accurate 32 sprites/scanline limit
  - 34 tile slots/scanline limit
  - Reference: [Sprites](https://snes.nesdev.org/wiki/PPU_OAM)

- ✅ **Scrolling** - Full background scrolling support
  - Horizontal and vertical scrolling on all BG layers
  - Per-layer scroll registers ($210D-$2114)
  - Reference: [Scrolling](https://snes.nesdev.org/wiki/PPU_registers#Background_Scrolling)

- ✅ **VRAM/CGRAM/OAM Access**
  - 64KB VRAM for tiles and tilemaps
  - 512-byte CGRAM for 256 colors (15-bit BGR)
  - 544-byte OAM (512 bytes main + 32 bytes high table)
  - VRAM access protection during active display
  - Reference: [VRAM](https://snes.nesdev.org/wiki/PPU_registers#VRAM)

#### DMA & HDMA
- ✅ **General-Purpose DMA** - Full 8-channel support
  - Channels configured via $4300-$437F
  - Enable register $420B (MDMAEN)
  - All transfer modes (0-7) with proper B-bus patterns
  - Address modes: increment, decrement, fixed
  - Direction: A-bus ↔ B-bus (both directions)
  - Cycle-accurate timing (8 cycles per byte + overhead)
  - Reference: [DMA](https://snes.nesdev.org/wiki/DMA_&_HDMA#DMA)

- ✅ **HDMA (H-blank DMA)** - Per-scanline updates
  - 8-channel HDMA support (shared channels with DMA)
  - Enable register $420C (HDMAEN)
  - Direct and indirect addressing modes
  - Per-scanline register updates
  - Line counter and repeat mode
  - Automatic table processing
  - Reference: [HDMA](https://snes.nesdev.org/wiki/DMA_&_HDMA#HDMA)

#### CPU I/O Registers
- ✅ **Interrupt Control**
  - $4200 (NMITIMEN) - NMI/IRQ enable and auto-joypad
  - $4210 (RDNMI) - NMI flag with read-and-clear ⭐
  - $4211 (TIMEUP) - IRQ flag (stub)
  - $4212 (HVBJOY) - H/V-Blank and joypad status
  - Reference: [CPU Registers](https://snes.nesdev.org/wiki/CPU_registers)

- ✅ **Controller Input**
  - $4016-$4017 - Serial joypad ports (JOYSER0/1)
  - $4218-$421F - Auto-joypad read registers (JOY1L-JOY4H)
  - Full SNES controller support (12 buttons)
  - Auto-joypad read during VBlank
  - Reference: [Controllers](https://snes.nesdev.org/wiki/Input_devices)

#### APU (Audio Processing Unit)
- ✅ **SPC700 CPU** - Complete audio processor implementation
  - Full SPC700 instruction set from `emu_core::apu::Spc700`
  - 64KB audio RAM (ARAM)
  - IPL boot ROM with upload protocol
  - $2140-$2143 (APUIO0-3) - CPU ↔ SPC700 communication ports
  - Bidirectional port communication working
  - Games can upload audio drivers and communicate with APU
  - Reference: [APU](https://snes.nesdev.org/wiki/APU), [SPC700](https://snes.nesdev.org/wiki/SPC700)

- ❌ **DSP (Digital Signal Processor)** - Not implemented
  - No audio sample generation
  - No 8-voice synthesis
  - Silent gameplay (no sound output)
  - Reference: [DSP](https://snes.nesdev.org/wiki/DSP)

#### Timing
- ✅ **Frame Timing** - NTSC timing implementation
  - 89,342 master cycles per frame (~3.58 MHz / 60 Hz)
  - 341 cycles per scanline
  - 262 scanlines per frame (224 visible + 38 VBlank)
  - Reference: [Timing](https://snes.nesdev.org/wiki/Timing)

- ✅ **VBlank/NMI**
  - VBlank starts at scanline 225
  - NMI triggers if enabled ($4200 bit 7)
  - Proper NMI flag handling ($4210 read-and-clear)
  - Reference: [NMI](https://snes.nesdev.org/wiki/NMI)

#### Other Features
- ✅ **Save States** - Full system state serialization
- ✅ **Logging** - Comprehensive debug logging for CPU, PPU, DMA, interrupts

### What's Missing

#### PPU Advanced Features
- ❌ **Windows** - No window masking ($2123-$212B)
  - Reference: [Windows](https://snes.nesdev.org/wiki/PPU_registers#Windows)
- ❌ **Color Math** - No color addition/subtraction ($2130-$2132)
  - Reference: [Color Math](https://snes.nesdev.org/wiki/PPU_registers#Color_addition)
- ❌ **Mosaic** - No mosaic effect ($2106)
- ❌ **Sub-screen** - No sub-screen support ($212D)
- ❌ **Mode 7 Transform** - Matrix registers not implemented ($211A-$2120)
  - Basic 8bpp tile rendering works, but no rotation/scaling
  - Missing: M7SEL, M7A-M7D (matrix), M7X/M7Y (center point)
  - Reference: [Mode 7](https://snes.nesdev.org/wiki/Mode_7)
- ❌ **Hi-res (512px)** - Modes 5-6 render at normal 256px resolution
- ❌ **Offset-per-tile** - Not implemented for Modes 2, 4, 6
  - Would require reading offset data from BG3 tilemap
  - Reference: [Offset-per-tile](https://snes.nesdev.org/wiki/PPU_registers#BG_Scroll)

#### Audio
- ❌ **DSP (Digital Signal Processor)** - No sound generation
  - SPC700 CPU is fully implemented and functional
  - DSP registers can be accessed but produce no audio
  - No 8-voice synthesis, ADPCM playback, or echo effects
  - Reference: [DSP](https://snes.nesdev.org/wiki/DSP)

#### Enhancement Chips
- ❌ **No enhancement chip support**
  - No SuperFX, SA-1, DSP-1/2/3/4, S-DD1, Cx4, etc.
  - Games requiring these chips will not work
  - Reference: [Enhancement Chips](https://snes.nesdev.org/wiki/Enhancement_chips)

#### Other Missing Features
- ❌ **IRQ** - H/V timer interrupts not implemented
- ❌ **PAL** - NTSC timing only
- ❌ **Interlace** - No interlace mode support
- ❌ **Hardware Multiply/Divide** - Registers stubbed

## Register Implementation Status

### PPU Registers ($2100-$213F)
Core PPU registers implemented:
- ✅ $2100 (INIDISP) - Screen display, force blank, brightness
- ✅ $2105 (BGMODE) - BG mode and character size
- ✅ $2107-$210A - BG tilemap address and size
- ✅ $210B-$210C - BG character data address
- ✅ $210D-$2114 - Background scrolling
- ✅ $2115-$2119 - VRAM access
- ✅ $2121-$2122 - CGRAM access
- ✅ $2101-$2104 - OAM access
- ✅ $212C (TM) - Main screen layer enable
- ✅ $213F (STAT78) - PPU status and NMI flag
- ⚠️ $2123-$212B - Windows (stubbed)
- ⚠️ $2130-$2132 - Color math (stubbed)

Reference: [PPU Registers](https://snes.nesdev.org/wiki/PPU_registers)

### CPU I/O Registers ($4000-$43FF)
- ✅ $4200 (NMITIMEN) - Interrupt enable
- ✅ $4210 (RDNMI) - NMI flag ⭐ Critical for proper NMI handling
- ✅ $4211 (TIMEUP) - IRQ flag (stub)
- ✅ $4212 (HVBJOY) - H/V-Blank and joypad status
- ✅ $4016-$4017 - Controller serial ports
- ✅ $4218-$421F - Auto-joypad read
- ✅ $420B (MDMAEN) - DMA enable
- ✅ $420C (HDMAEN) - HDMA enable
- ✅ $4300-$437F - DMA/HDMA channel registers
- ⚠️ $4202-$4206 - Multiply/Divide (stubbed)

Reference: [CPU Registers](https://snes.nesdev.org/wiki/CPU_registers)

## Architecture

### Component Structure

```
SnesSystem
  └── SnesCpu (wraps Cpu65c816<SnesBus>)
      └── SnesBus (implements Memory65c816)
          ├── 128KB WRAM
          ├── SPC700 APU (Full implementation)
          │   ├── SPC700 CPU core
          │   ├── 64KB Audio RAM (ARAM)
          │   ├── IPL boot ROM
          │   ├── Communication ports ($2140-$2143)
          │   └── DSP registers (stub - no audio output)
          ├── DMA Controller (8 channels)
          │   ├── General-purpose DMA
          │   ├── HDMA (H-blank DMA)
          │   └── Transfer modes 0-7
          ├── SNES PPU (All Modes 0-7)
          │   ├── 64KB VRAM
          │   ├── 256-color CGRAM (palette)
          │   ├── 4 BG layers (modes 0-1)
          │   ├── 2 BG layers (modes 2-5)
          │   ├── 1 BG layer (modes 6-7)
          │   └── 2bpp/4bpp/8bpp tile support
          └── Cartridge (LoROM/HiROM auto-detect)
              ├── ROM banks (LoROM: 32KB chunks, HiROM: 64KB linear)
              └── 32KB SRAM
```

### Key Files
- `src/lib.rs` - System initialization and frame execution
- `src/cpu.rs` - CPU wrapper using core 65C816
- `src/bus.rs` - Memory bus with all hardware registers
- `src/ppu.rs` - Complete PPU implementation (modes 0-7)
- `src/ppu_renderer.rs` - Rendering backend
- `src/cartridge.rs` - ROM loading and mapping

## Testing

### Test ROMs
Located in `test_roms/snes/`:
- `test.sfc` - Basic Mode 0 checkerboard pattern
- `test_priority.sfc` - Priority bit handling test
- `test_enhanced.sfc` - Enhanced rendering features
- `test_sprite_overflow.sfc` - Sprite limits test

### Unit Tests
```bash
cargo test --package emu_snes
```
- 61+ unit tests covering bus, PPU, DMA, HDMA, controllers
- All tests passing

### Commercial Game Testing
Games known to work (with limitations):
- ✅ **Super Mario World** - Works with Mode 1, sprites, scrolling (no audio)
- ⚠️ **Donkey Kong Country** - Graphics work
- ❌ **F-Zero** - Requires Mode 7 rotation
- ❌ **Super Mario RPG** - Requires SA-1 chip
- ❌ **Star Fox** - Requires SuperFX chip

## Development

### Building
```bash
cargo build --profile release-quick
```

### Debugging
Enable logging for different subsystems:
```bash
# CPU execution trace
cargo run -- game.sfc --log-cpu trace

# Interrupt debugging
cargo run -- game.sfc --log-interrupts info

# PPU register access
cargo run -- game.sfc --log-ppu info

# DMA operations
cargo run -- game.sfc --log-bus debug
```

## Known Issues

1. **Audio Output** - SPC700 CPU implemented but no sound
   - SPC700 processor fully functional
   - Games can upload audio drivers
   - DSP not implemented, so no audio generation
   - Silent gameplay

2. **Timing** - Frame-based, not cycle-accurate
   - Good enough for most games
   - Some timing-sensitive effects may not work

## Additional Documentation

- `SNES_REGISTER_FIXES.md` - Details on NMI register implementation
- `SNES_WAI_INVESTIGATION.md` - WAI instruction debugging notes

## References & Further Reading

### Primary References
- **SNESdev Wiki**: https://snes.nesdev.org/wiki/SNESdev_Wiki
- **Anomie's Register Doc**: https://snes.nesdev.org/wiki/Anomie%27s_Doc
- **fullsnes**: https://problemkaputt.de/fullsnes.htm

### Specific Topics
- **65C816 CPU**: https://snes.nesdev.org/wiki/65c816_reference
- **PPU Registers**: https://snes.nesdev.org/wiki/PPU_registers
- **CPU Registers**: https://snes.nesdev.org/wiki/CPU_registers
- **DMA/HDMA**: https://snes.nesdev.org/wiki/DMA_&_HDMA
- **Memory Map**: https://snes.nesdev.org/wiki/Memory_map
- **Timing**: https://snes.nesdev.org/wiki/Timing
- **Controllers**: https://snes.nesdev.org/wiki/Input_devices
- **APU/SPC700**: https://snes.nesdev.org/wiki/SPC700
- **Enhancement Chips**: https://snes.nesdev.org/wiki/Enhancement_chips
