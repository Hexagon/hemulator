# Sega Master System Implementation

This document describes the SMS (Sega Master System) implementation in Hemulator.

## Current Status

**✅ Implemented:**
- Z80 CPU with full instruction set and interrupt support (IM 0, IM 1, IM 2, NMI)
- SN76489 PSG audio chip (3 tone channels + 1 noise channel)
- VDP (Video Display Processor) with:
  - Tilemap and sprite rendering
  - Frame interrupts
  - Line interrupts
  - Sprite overflow detection
  - Sprite collision detection
- Memory bus with ROM banking support
- System trait implementation
- Frontend integration (ROM detection, controller input, audio)
- Test ROM and smoke tests
- All unit tests passing (19/19)

**⚠️ Partially Implemented:**
- Save state serialization (trait implemented, serialization logic pending)

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
│   ├── vdp.rs          # Video Display Processor
│   └── bus.rs          # Memory bus (SmsMemory)
└── Cargo.toml

crates/core/src/apu/
└── sn76489.rs          # PSG audio chip
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

### SN76489 PSG

The PSG implements the `AudioChip` trait and provides:
- 3 square wave tone generators (10-bit frequency control)
- 1 noise generator (16-bit LFSR for Sega variant)
- 4-bit volume control per channel (0=max, 15=mute)
- Exponential volume curve (~-2dB per step)

Register interface:
- Port 0x7E/0x7F: PSG write

### Memory Map

| Address Range | Description |
|--------------|-------------|
| 0x0000-0x3FFF | ROM Bank 0 (16KB) |
| 0x4000-0x7FFF | ROM Bank 1 (16KB) |
| 0x8000-0xBFFF | ROM Bank 2 (16KB) |
| 0xC000-0xDFFF | RAM (8KB) |
| 0xE000-0xFFFF | RAM Mirror |

Banking registers at 0xFFFC, 0xFFFD, 0xFFFE (in RAM) control which 16KB banks are mapped.

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
- ✅ VDP: register writes, VRAM access, color decoding, interrupts, sprite flags (8 tests)
- ✅ Memory bus: RAM/ROM access, banking (3 tests)
- ✅ System: creation, reset, ROM loading, frame stepping, interrupts (8 tests)
- ✅ Total: 19 tests passing

Run tests with:
```bash
cargo test --package emu_sms
```

## Next Steps

1. **Save State Serialization**
   - Implement state serialization for CPU, VDP, PSG, and memory
   - Add tests for save/load functionality

2. **Game Gear Support**
   - Extended resolution (160×144)
   - LCD palette
   - Link cable support

3. **Enhanced Testing**
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

- Save states not fully implemented (serialization logic pending)
- No Game Gear support yet (planned)
- No FM sound unit support (Master System only, optional accessory)
