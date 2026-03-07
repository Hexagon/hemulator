# TODO

## Ignored Tests

All tests below are marked `#[ignore]` because they require an OpenGL context that is
not available in CI or in headless `cargo test` runs.  They can be run manually with
`cargo test --package <pkg> -- --ignored`.

### N64 — Requires OpenGL context

#### Low
- [ ] **Enable N64 system tests** (18 tests): Remove `#[ignore]` once a headless GL context is available in CI — `crates/systems/n64/src/lib.rs`
- [ ] **Enable N64 debugger tests** (5 tests): Remove `#[ignore]` once a headless GL context is available in CI — `crates/systems/n64/src/debugger.rs`
- [ ] **Enable RDP tests** (38 tests): Remove `#[ignore]` once a headless GL context is available in CI — `crates/systems/n64/src/rdp.rs`
- [ ] **Enable RSP HLE tests** (20 tests): Remove `#[ignore]` once a headless GL context is available in CI — `crates/systems/n64/src/rsp_hle.rs`
- [ ] **Enable GUI logging integration test** (1 test, `test_logging_n64_rdp_unknown_command`): Remove `#[ignore]` once a headless GL context is available in CI — `crates/frontend/gui/tests/logging_integration.rs`

## N64

### Critical
- [x] **FlashRAM command protocol** (`Macronix MX29L1100`): Implemented the full Macronix MX29L1100 state machine — erase (chip + sector), write, status, and read modes. Games using FlashRAM (Pokémon Stadium, etc.) now save correctly. — `crates/systems/n64/src/bus.rs`
- [ ] **Save type detection for unlisted games**: Only common retail titles are in the save-type database. Unknown games silently receive `SaveType::None`. Consider implementing a checksum-based database fallback (e.g. using the ipl3 CRC or the CIC seed) so that unlisted games still get a sensible default. — `crates/systems/n64/src/cartridge.rs`

### High
- [ ] **RDP Blend/Combine pipeline**: `SET_OTHER_MODES` values are stored in both the RDP (`rdp.rs`) and RSP HLE (`rsp_hle.rs`) but are never consumed by the rendering pipeline. Applying cycle type, texture filtering, and alpha-blending modes would significantly improve visual accuracy. — `crates/systems/n64/src/rdp.rs`, `crates/systems/n64/src/rdp_renderer_opengl.rs`
- [x] **RSP Audio microcode (ABI1)**: Implemented a basic ABI1 command interpreter (SPNOOP, ADPCM stub, CLEARBUFF, RESAMPLE stub, DMEMMOVE, MIXER, INTERLEAVE, LOADBUFF, SAVEBUFF). Games that use ABI1 now produce audio output via the INTERLEAVE→RDRAM path. ADPCM decode and RESAMPLE remain approximations. — `crates/systems/n64/src/rsp_hle.rs`
- [x] **Controller Pak (memory card)**: Implemented PIF commands 0x02 (read pak) and 0x03 (write pak) with CRC-8 generation, 32 KB per-slot storage, and proper save/load API. Rewrote PIF channel walker to correctly parse variable-length channels instead of fixed offsets. Games requiring a Controller Pak (e.g. Wave Race 64) now save correctly. — `crates/systems/n64/src/pif.rs`

### Medium
- [ ] **RSP ADPCM decode**: The ADPCM command (0x01) currently writes silence. A proper VADPCM decoder would significantly improve audio quality for games that use ADPCM-compressed audio. — `crates/systems/n64/src/rsp_hle.rs`
- [ ] **RSP Audio RESAMPLE command**: The RESAMPLE command (0x03) is currently a no-op. A proper resample algorithm would improve audio accuracy. — `crates/systems/n64/src/rsp_hle.rs`
- [ ] **RSP F3DEX fog / clipping commands**: Several less-common F3DEX commands still log as stubs: `G_SETPRIMDEPTH`, `G_TEXTURE` (texture coordinate scaling not forwarded to RDP), `G_LOAD_UCODE` (microcode reload). — `crates/systems/n64/src/rsp_hle.rs`
- [ ] **RDP performance counters**: `DPC_CLOCK`, `DPC_BUFBUSY`, `DPC_PIPEBUSY`, and `DPC_TMEM` registers return hardcoded 0. Some games wait for these to reach expected values. — `crates/systems/n64/src/rdp.rs`
- [ ] **CPU overflow traps**: Signed arithmetic instructions (`ADD`, `ADDI`, `SUB`) should raise an overflow exception on overflow, but currently use wrapping arithmetic. Most commercial software does not rely on this, but some may. — `crates/core/src/cpu_mips_r4300i/`

### Low
- [ ] **Memory alignment validation**: Load/store instructions that require alignment (LH/SH = 2 B, LW/SW = 4 B, LD/SD = 8 B) do not raise `AddressError` exceptions on misaligned access. — `crates/core/src/cpu_mips_r4300i/`
- [ ] **Full cache coherency**: The TLB/cache is direct-mapped only; cache invalidation and dirty-line write-back are not emulated. — `crates/systems/n64/src/bus.rs`

## SMS

### Medium
- [ ] **Line interrupt counter reload timing**: The line counter is reloaded from R10 at scanline 192 (start of VBlank). Verify correct reload behavior for games that change R10 mid-frame. — `crates/systems/sms/src/vdp.rs`

## Game & Watch

### Medium
- [ ] **SM511/SM5A CPU variants**: Some .mgw ROMs use SM511 or SM5A CPUs which have different instruction sets and display mappings. Currently only SM510 is supported. — `crates/systems/gameandwatch/src/sm510.rs`
- [ ] **JPEG background support**: .mgw files with FLAG_BACKGROUND_JPEG (bit 5) have the background as a JPEG appended after the compressed data. Currently not parsed. — `crates/systems/gameandwatch/src/mgw.rs`
- [ ] **LZMA compression**: .mgw files compressed with LZMA are not supported (only LZ4 and ZLIB). — `crates/systems/gameandwatch/src/mgw.rs`
- [ ] **Accurate melody ROM playback**: Melody section data from .mgw is parsed but not used for audio generation. Current buzzer is a simple square wave. — `crates/systems/gameandwatch/src/lib.rs`

### Low
- [ ] **LCD deflicker filtering**: .mgw flag bits 6-7 specify deflicker mode for smoother segment transitions. Not implemented. — `crates/systems/gameandwatch/src/lib.rs`
- [ ] **Segment pixel compositing accuracy**: Current rendering uses grayscale as alpha mask. Verify this matches gw-libretro's actual rendering pipeline. — `crates/systems/gameandwatch/src/lib.rs`

## GBA

### High
- [ ] **Affine ref point writes via DMA**: Verify HBlank DMA correctly updates PPU affine reference points for Mode 7 games (F-Zero, Mario Kart)
- [ ] **Medal of Honor white screen**: HBlank polling loop — verify HBlank timing fix resolves the issue

### Medium
- [ ] **Implement halt/stop modes** (`HALTCNT` at `0x04000301`) — `crates/systems/gba/src/lib.rs`
- [ ] **HBlank OAM access restriction** (`DISPCNT` bit 5) — `crates/systems/gba/src/ppu.rs`

## PS1

### High
- [ ] **SPU ADPCM Audio** — 24-voice ADPCM decoding, ADSR envelopes, and reverb not yet implemented. `generate_sample()` currently returns silence. — `crates/systems/ps1/src/spu.rs`
- [ ] **CD-ROM command handling** — Only GetStat/GetID stubs; no disc data streaming or sector reads. Required for running real game discs. — `crates/systems/ps1/src/lib.rs`

### Medium
- [ ] **Save States** — `save_state()` / `load_state()` are stubs (return empty/no-op). Need full serialization of CPU, GPU, SPU, RAM, and DMA state. — `crates/systems/ps1/src/lib.rs`
- [ ] **MDEC** — Motion Decoder (FMV) stubbed; no macroblock decoding. — `crates/systems/ps1/src/lib.rs`

### Low
- [ ] **GPU Polylines** — Polyline (multi-segment line) commands with termination-code detection and Gouraud shading not implemented. — `crates/systems/ps1/src/gpu.rs`
- [ ] **VRAM→CPU read transfer** — GP0 command to initiate VRAM read transfer not fully implemented. — `crates/systems/ps1/src/gpu.rs`
- [ ] **24-bit display mode** — 24-bit pixel readback path not implemented (reads 16-bit pixels instead). — `crates/systems/ps1/src/gpu.rs`

## SNES

### High
- [ ] **cputest-basic fails at test `0x025d`** — next failing test after bank-wrap fix; needs investigation
  - File: `crates/core/src/cpu_65c816.rs`
  - Raise `MIN_PASSING` in `test_cputest_basic_loads_and_runs` once resolved
- [ ] **cputest-full fails at test `0x0025`** — next failing emulation-mode test after dp-wrap fix; needs investigation
  - File: `crates/core/src/cpu_65c816.rs`
  - Raise `MIN_PASSING` in `test_cputest_full_loads_and_runs` once resolved
