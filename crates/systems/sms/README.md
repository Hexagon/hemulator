# Sega Master System Implementation

This document describes the SMS (Sega Master System) implementation in Hemulator.

## Current Status

**✅ Implemented:**
- Z80 CPU with full instruction set and interrupt support (IM 0, IM 1, IM 2, NMI)
- SN76489 PSG audio chip with CPU-cycle-accurate emulation:
  - 3 square wave tone generators (10-bit frequency control)
  - 1 noise generator (16-bit LFSR for Sega variant)
  - 4-bit volume control per channel (0=max, 15=mute)
  - Exponential volume curve (~-2dB per step)
  - Proper downsampling from SMS CPU speed (3.579545 MHz NTSC / 3.546894 MHz PAL) to 44.1 kHz
  - NTSC and PAL timing support
- VDP (Video Display Processor) with:
  - Tilemap and sprite rendering
  - Frame interrupts
  - Line interrupts
  - Sprite overflow detection
  - Sprite collision detection
- Memory bus with ROM banking support
- BIOS support with optional BIOS mount point:
  - Games run without BIOS by default
  - Optional BIOS ROM loading for games that require it
  - BIOS can be enabled/disabled via memory control register (port 0x3E, bit 3)
  - Minimal default BIOS available for testing
- PAL/NTSC timing detection from ROM header (TMR SEGA)
  - Automatic detection based on region code
  - Proper frame timing (60Hz NTSC, 50Hz PAL)
  - Correct scanline counts (262 NTSC, 313 PAL)
- Save state serialization:
  - Complete CPU state (all registers, flags, interrupt state)
  - VDP state (VRAM, CRAM, registers, internal state)
  - PSG state (all audio channels and parameters)
  - Memory state (RAM, ROM banking)
- System trait implementation
- Frontend integration (ROM detection, controller input, audio)
- Test ROM and smoke tests
- All unit tests passing (50/50)

**❌ Not Yet Implemented:**
- Game Gear support (planned)
- FM sound unit support (Master System only, optional accessory)

## Architecture

### Hardware Components

| Component | Specification |
|-----------|--------------|
| CPU | Zilog Z80A @ 3.58 MHz (NTSC) / 3.55 MHz (PAL) |
| RAM | 8 KB main RAM |
| VRAM | 16 KB video RAM |
| VDP | Sega 315-5124 (SMS 1), 315-5246 (SMS 2) |
| PSG | Texas Instruments SN76489 (Sega variant SN76496) |
| Resolution | 256×192 pixels @ 60Hz (NTSC) / 50Hz (PAL) |
| Colors | 64 colors (6-bit RGB), 32 simultaneous |
| Sprites | 64 total, 8 per scanline |

### Module Structure

```
crates/systems/sms/
├── src/
│   ├── lib.rs          # Public API and module declarations
│   ├── system.rs       # SmsSystem implementing System trait
│   ├── psg.rs          # SMS-specific PSG wrapper with cycle-accurate audio
│   ├── vdp.rs          # Video Display Processor
│   └── bus.rs          # Memory bus (SmsMemory)
└── Cargo.toml

crates/core/src/apu/
└── sn76489.rs          # Core SN76489 PSG implementation
```

## Implementation Details

### VDP (Video Display Processor)

The VDP implements the `Renderer` trait and provides:
- 256×192 resolution framebuffer
- Tilemap-based background rendering
- 64 hardware sprites with 8 per scanline limit
- 6-bit color palette (64 colors total, 32 on-screen)
- Scrolling support
- Frame and line interrupts

Register interface:
- Port 0xBE: Data port (read/write VRAM/CRAM)
- Port 0xBF: Control/status port

### SN76489 PSG (Programmable Sound Generator)

The PSG uses a two-layer architecture similar to the NES APU:

**System-Specific Layer (`psg.rs`):**
- CPU-cycle-accurate audio generation with proper downsampling
- Cycle accumulation for precise 44.1 kHz output
- Timing mode support (NTSC @ 3.579545 MHz / PAL @ 3.546894 MHz)
- Integration with SMS system timing

**Core Implementation (`emu_core::apu::sn76489`):**
- 3 square wave tone generators with 10-bit frequency control
- 1 noise generator with 16-bit LFSR (Sega variant)
- 4-bit volume control per channel (0=max, 15=mute)
- Exponential volume curve (~-2dB per step)
- Proper channel mixing and audio sample generation

**Audio Generation Process:**
1. Calculate CPU cycles per audio sample (CPU_HZ / 44,100)
2. Clock PSG at CPU speed for each audio sample
3. Average output over CPU cycles to generate final sample
4. Use cycle accumulation to handle fractional cycles

This approach ensures cycle-accurate audio emulation similar to the NES,
producing high-quality audio output that accurately represents the original
hardware behavior.

**Register Interface:**
- Port 0x7E/0x7F: PSG write
- Latch/Data byte format: `1cctdddd` (c=channel, t=type, d=data)
- Data byte format: `0ddddddd` (d=data)

**Frequency Calculation:**
```
Frequency (Hz) = CPU_Clock / (32 × register_value)
For 440 Hz (A4): register = 3579545 / (32 × 440) ≈ 254
```

### Memory Map

| Address Range | Description |
|--------------|-------------|
| 0x0000-0x03FF | BIOS ROM (1KB, when enabled via bit 3 of port 0x3E) |
| 0x0000-0x3FFF | ROM Bank 0 (16KB, when BIOS disabled) |
| 0x4000-0x7FFF | ROM Bank 1 (16KB) |
| 0x8000-0xBFFF | ROM Bank 2 (16KB) |
| 0xC000-0xDFFF | RAM (8KB) |
| 0xE000-0xFFFF | RAM Mirror |

Banking registers at 0xFFFC, 0xFFFD, 0xFFFE (in RAM) control which 16KB banks are mapped.

### BIOS Support

The SMS has optional BIOS ROM support:

**Memory Control Register (Port 0x3E):**
- Bit 3: BIOS enable/disable
  - 0 = BIOS enabled (BIOS ROM mapped at 0x0000-0x03FF)
  - 1 = BIOS disabled (Cartridge ROM mapped from 0x0000)

**Default Behavior:**
- Most games work without BIOS
- BIOS is not loaded by default
- Games start directly from cartridge ROM at 0x0000

**Loading BIOS:**
- BIOS can be mounted via the "bios" mount point
- Supports .sms, .bin, .rom file extensions
- When loaded, BIOS is automatically enabled
- BIOS typically boots, initializes hardware, then disables itself

**Use Cases:**
- Some Japanese games require BIOS
- BIOS shows SEGA logo on boot
- BIOS provides utility functions for games

### I/O Ports

| Port | Description |
|------|-------------|
| 0x7E/0x7F | PSG write |
| 0xBE | VDP data port |
| 0xBF | VDP control/status port |
| 0xDC | Controller port 1 |
| 0xDD | Controller port 2 |
| 0x3E | Memory control |

## Testing

Current test coverage:
- ✅ PSG: audio generation, volume control, tone/noise channels, timing modes, cycle accuracy (9 tests)
- ✅ VDP: register writes, VRAM access, color decoding, interrupts, sprite flags (8 tests)
- ✅ Memory bus: RAM/ROM access, banking (3 tests)
- ✅ System: creation, reset, ROM loading, frame stepping, interrupts (10 tests)
- ✅ BIOS: mounting/unmounting, enable/disable, memory reads (5 tests)
- ✅ Save states: CPU, VDP, PSG, memory serialization (5 tests)
- ✅ Timing: PAL/NTSC detection, frame cycle calculations (3 tests)
- ✅ Total: 50 tests passing

Run tests with:
```bash
cargo test --package emu_sms
```

## Next Steps

1. **Game Gear Support**
   - Extended resolution (160×144)
   - LCD palette
   - Link cable support

2. **Enhanced Testing**
   - Test with more commercial SMS ROMs
   - Performance profiling
   - Accuracy testing against hardware

4. **Documentation**
   - Update MANUAL.md with detailed SMS controls and features
   - Document mapper support

## ROM Format

SMS ROMs are typically headerless binary files:
- Common sizes: 8KB, 16KB, 32KB, 48KB, 64KB, 128KB, 256KB, 512KB
- Optional TMR SEGA header at offset 0x7FF0
- Detection: Check for "TMR SEGA" signature or common ROM sizes

## References

- [SMS Power! Development Documents](https://www.smspower.org/Development/)
- [Charles MacDonald's VDP Documentation](https://github.com/franckverrot/EmulationResources/blob/master/consoles/sms-gg/Sega%20Master%20System%20VDP%20documentation.txt)
- [Rodrigo Copetti's SMS Architecture](https://www.copetti.org/writings/consoles/master-system/)
- [SN76489 PSG Documentation](https://www.vgmpf.com/Wiki/index.php?title=SN76489)
- [Z80 User Manual](http://www.zilog.com/docs/z80/um0080.pdf)

## Known Limitations

- No Game Gear support yet (planned)
- No FM sound unit support (Master System only, optional accessory)
