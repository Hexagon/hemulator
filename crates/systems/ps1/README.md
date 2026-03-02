# PlayStation 1 (PS1/PSX) Implementation

This document describes the PS1 (Sony PlayStation) implementation in Hemulator.

## Current Status

**🚧 In Development** — Basic hardware emulation is functional. BIOS + PS-X EXE loading works;
full disc (CD-ROM) game compatibility is not yet implemented.

### What Works

- ✅ **MIPS R3000A CPU** — Full MIPS I instruction set via the `cpu_mips_r3000a` core crate
- ✅ **GPU** — 2D/3D graphics rendering
  - 1 MB VRAM (1024×512, 16-bit pixels)
  - Flat-shaded and Gouraud-shaded polygons (triangles and quads)
  - Textured primitives with 4-bit, 8-bit, and 15-bit texture modes
  - Semi-transparency blending
  - Sprite (rectangle) rendering
  - Display resolutions up to 640×480
  - GP0 rendering commands and GP1 display-control commands
  - VRAM-to-VRAM copies and CPU→VRAM transfers
  - VBlank interrupt generation
- ✅ **DMA Controller** — 7 channels (MDEC-in/out, GPU, CD-ROM, SPU, PIO, OTC)
  - Block-transfer and linked-list modes
  - DMA interrupt controller with channel-level enable/mask
- ✅ **Timers** — 3 hardware timers with target/overflow interrupts
- ✅ **Interrupt Controller** — I_STAT / I_MASK registers, IRQ routing to CPU COP0
- ✅ **Memory Bus** — 2 MB main RAM, 1 KB scratchpad, 512 KB BIOS ROM, I/O ports
- ✅ **BIOS Loading** — Required 512 KB BIOS ROM (`bin`/`rom`)
- ✅ **PS-X EXE Loading** — Direct execution of PlayStation executable files
- ✅ **SPU (stub)** — 24-voice register file; audio output not yet implemented

### What's Missing

- ⏳ **CD-ROM** — Command handling is minimal (GetStat, GetID stubs only); no disc data streaming
- ⏳ **SPU Audio** — Voice ADPCM decoding, ADSR envelopes, and reverb not implemented
- ⏳ **Save States** — Serialization stubs present but not implemented
- ⏳ **MDEC** — Motion Decoder (FMV) stubbed
- ⏳ **GTE (COP2)** — Geometry Transform Engine not yet implemented
- ⏳ **GPU Lines** — Polylines and Gouraud-shaded lines incomplete
- ⏳ **24-bit Display Mode** — 24-bit pixel read path not implemented
- ⏳ **Game Compatibility** — Real game discs require CD-ROM and GTE support

## Hardware Overview

| Component | Specification |
|-----------|---------------|
| CPU | MIPS R3000A @ 33.8688 MHz |
| RAM | 2 MB main RAM + 1 KB scratchpad (D-Cache) |
| GPU | Custom 2D/3D, 1 MB VRAM |
| SPU | 24-channel ADPCM, 512 KB sound RAM |
| CD-ROM | 2× speed drive |
| BIOS | 512 KB ROM |
| GTE | Geometry Transform Engine (COP2) |
| MDEC | Motion Decoder (FMV) |

## Memory Map (Physical)

| Address Range | Size | Description |
|---|---|---|
| `0x00000000–0x001FFFFF` | 2 MB | Main RAM |
| `0x1F000000–0x1F7FFFFF` | 8 MB | Expansion Region 1 |
| `0x1F800000–0x1F8003FF` | 1 KB | Scratchpad (D-Cache) |
| `0x1F801000–0x1F801FFF` | 4 KB | I/O Ports |
| `0x1F802000–0x1F802FFF` | 4 KB | Expansion Region 2 |
| `0x1FC00000–0x1FC7FFFF` | 512 KB | BIOS ROM |

## Module Structure

```
crates/systems/ps1/
├── src/
│   ├── lib.rs      # System trait impl, bus, DMA, timers, IRQ, CD-ROM stubs
│   ├── gpu.rs      # GPU state machine (GP0/GP1), VRAM, renderer
│   └── spu.rs      # SPU register file (24 voices, audio stubs)
└── Cargo.toml
```

## Usage

```rust
use emu_ps1::Ps1System;
use emu_core::System;

let mut ps1 = Ps1System::new();

// Mount required BIOS (512 KB PlayStation BIOS ROM)
let bios = std::fs::read("SCPH1001.BIN")?;
ps1.mount("bios", &bios)?;

// Optionally mount a PS-X EXE for direct execution
let exe = std::fs::read("game.exe")?;
ps1.mount("disc", &exe)?;

// Run one frame
let frame = ps1.step_frame()?;
println!("{}×{}", frame.width, frame.height);
```

**GUI / command-line:**
```bash
# Load via project file
hemu --bios SCPH1001.BIN game.exe
```

## Known Limitations

- **No CD-ROM disc support** — Real game discs cannot be run yet
- **No audio** — SPU register file is present but ADPCM/reverb output is not implemented
- **No save states** — State serialization is stubbed
- **No GTE** — 3D math acceleration (COP2) is absent; 3D games will not compute geometry correctly
- **No MDEC** — Full-motion video sequences will not play

See the [User Manual](https://hemulator.56k.guru/user/systems.html#ps1-sony-playstation) for user-facing notes.

## References

- **nocash PSX-SPX Specifications**: <https://problemkaputt.de/psx-spx.htm> — comprehensive PS1 hardware reference
- **Avocado PS1 emulator** — reference implementation
- **Rustation PS1 emulator** — reference implementation

## License

Same as the parent Hemulator project.
