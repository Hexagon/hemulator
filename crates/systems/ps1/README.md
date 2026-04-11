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
  - Semi-transparency blending (4 modes); texel STP bit controls per-pixel transparency
  - Sprite (rectangle) rendering
  - Dithering (4×4 Bayer matrix when GP0(0xE1) dithering bit is set)
  - Display resolutions up to 640×480 with correct 5→8-bit color expansion
  - GP0 rendering commands and GP1 display-control commands
  - VRAM-to-VRAM copies with mask-bit support and CPU→VRAM transfers
  - Large-primitive culling (> 1023px wide or > 511px tall)
  - Interlace odd/even field tracking in GPUSTAT
  - VBlank interrupt generation
- ✅ **DMA Controller** — 7 channels (MDEC-in/out, GPU, CD-ROM, SPU, PIO, OTC)
  - Block-transfer and linked-list modes
  - DMA interrupt controller with channel-level enable/mask
- ✅ **Timers** — 3 hardware timers with target/overflow interrupts
- ✅ **Interrupt Controller** — I_STAT / I_MASK registers, IRQ routing to CPU COP0
- ✅ **Memory Bus** — 2 MB main RAM, 1 KB scratchpad, 512 KB BIOS ROM, I/O ports
- ✅ **BIOS Loading** — Required 512 KB BIOS ROM (`bin`/`rom`)
- ✅ **PS-X EXE Loading** — Direct execution of PlayStation executable files
- ✅ **GTE (COP2)** — Geometry Transform Engine; all commands implemented:
  - RTPS/RTPT (perspective transform), NCLIP (back-face culling), AVSZ3/4 (Z-sort)
  - MVMVA (general matrix×vector), OP (outer product), SQR (square)
  - NCS/NCT, NCDS/NCDT, NCCS/NCCT (normal-color lighting pipelines)
  - CC, CDP, DCPL (color operations), DPCS/DPCT (depth cue), INTPL, GPF, GPL
  - Full FLAG register overflow/saturation tracking
- ✅ **SPU (stub)** — 24-voice register file; audio output not yet implemented
- ✅ **Debugger** — Full `Debugger` trait implementation via `crates/systems/ps1/src/debugger.rs`:
  - CPU state: all 32 GPRs, PC, HI, LO, COP0 SR/Cause/EPC, status-register flags
  - Memory regions: Main RAM, Scratchpad, I/O, BIOS, KSEG0/KSEG1 mirrors
  - Physical address translation (KSEG0/KSEG1 → physical) for memory reads
  - MIPS R3000A disassembly via `disasm_mips_r3000a` (all MIPS I + GTE COP2 ops)
- ✅ **Instruction Tracing** — Circular-buffer instruction trace with CPU state snapshots;
  enabled with `--trace-instructions` CLI flag; configurable history depth via `--trace-limit`
- ✅ **Breakpoints** — Execute, read, and write breakpoints via `BreakpointManager`;
  set via `--breakpoint` CLI flag or the GUI Debug Inspector; hit-logging routed to the CPU log category
- ✅ **GPU Inspector Tab** — Live GPU state viewer in the Inspector dock:
  - GPUSTAT register decoded (draw/display mode, color depth, interlace, DMA direction)
  - Drawing area (left/top/right/bottom) and drawing offset (X/Y)
  - Texture page (X/Y, bit depth, semi-transparency mode), texture-window mask/offset
  - Display area (VRAM X/Y, horizontal/vertical ranges), display enable/disable
  - Timing info (current scanline, VBlank flag, IRQ flag)

### What's Missing

- ⏳ **CD-ROM** — Command handling is minimal (GetStat, GetID stubs only); no disc data streaming
- ⏳ **SPU Audio** — Voice ADPCM decoding, ADSR envelopes, and reverb not implemented
- ⏳ **Save States** — Serialization stubs present but not implemented
- ⏳ **MDEC** — Motion Decoder (FMV) stubbed
- ⏳ **GPU Lines** — Polylines and Gouraud-shaded lines incomplete
- ⏳ **Game Compatibility** — Real game discs require CD-ROM support

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
│   ├── lib.rs       # System trait impl, bus, DMA, timers, IRQ, CD-ROM stubs
│   ├── debugger.rs  # Debugger trait: CPU state, memory regions, disassembly, breakpoints, tracing
│   ├── gpu.rs       # GPU state machine (GP0/GP1), VRAM, renderer, inspector data
│   └── spu.rs       # SPU register file (24 voices, ADPCM decode, ADSR)
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
# Direct CLI launch with BIOS + PS-X EXE
hemu --bios SCPH1001.BIN game.exe
```

## Debugging

### Instruction Tracing

```bash
# Trace all executed MIPS instructions (last 10,000 by default)
hemu --trace-instructions --bios SCPH1001.BIN game.exe

# Limit trace buffer size
hemu --trace-instructions --trace-limit 5000 --bios SCPH1001.BIN game.exe

# Dump trace to file when a breakpoint is hit
hemu --trace-instructions --trace-dump-file trace.txt --breakpoint 0x80030000 --bios SCPH1001.BIN game.exe
```

Each trace entry records the disassembled instruction and a full CPU state snapshot
(all 32 GPRs, PC, HI, LO, COP0 SR/Cause/EPC).

### Breakpoints

```bash
# Break execution at a specific address
hemu --breakpoint 0x80030000 --bios SCPH1001.BIN game.exe

# Multiple breakpoints
hemu --breakpoint 0x80030000 --breakpoint 0x80031000 --bios SCPH1001.BIN game.exe
```

Breakpoints can also be added and removed at runtime from the **Debug** inspector tab in the GUI.
Execute, read, and write breakpoints are all supported.

### Debug Dump

```bash
# Dump CPU state + disassembly + memory map at a specific PC
hemu --debug-dump-pc 0x80030000 --bios SCPH1001.BIN game.exe

# Dump after N cycles
hemu --debug-dump-cycles 1000000 --bios SCPH1001.BIN game.exe
```

### GPU Inspector Tab

Open the **Inspector** dock (View → Inspector) when a PS1 game is loaded to see the live
**🎮 GPU** tab.  It shows:
- **GPUSTAT** register: draw mode, color depth, interlace, DMA direction, display enable
- **Drawing area** and **drawing offset**
- **Texture page**: X/Y base, bit-depth, semi-transparency mode, texture disable flag
- **Texture window** mask and offset
- **Display area**: VRAM X/Y origin, horizontal/vertical display ranges, 15-bit vs 24-bit color
- **Timing**: current scanline, VBlank status, IRQ flag

## Known Limitations

- **No CD-ROM disc support** — Real game discs cannot be run yet
- **No audio** — SPU register file is present but ADPCM/reverb output is not implemented
- **No save states** — State serialization is stubbed
- **No MDEC** — Full-motion video sequences will not play

See the [User Manual](https://hemulator.56k.guru/user/systems.html#ps1-sony-playstation) for user-facing notes.

## References

- **nocash PSX-SPX Specifications**: <https://psx-spx.consoledev.net/graphicsprocessingunitgpu/> — comprehensive PS1 hardware reference (GPU chapter used for all rendering fixes)
- **nocash PSX-SPX (mirror)**: <https://problemkaputt.de/psx-spx.htm>
- **Avocado PS1 emulator** — reference implementation
- **Rustation PS1 emulator** — reference implementation

## License

Same as the parent Hemulator project.
