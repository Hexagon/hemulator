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
- [ ] **Save type detection for unlisted games**: Only common retail titles are in the save-type database. Unknown games silently receive `SaveType::None`. Consider implementing a checksum-based database fallback (e.g. using the ipl3 CRC or the CIC seed) so that unlisted games still get a sensible default. — `crates/systems/n64/src/cartridge.rs`

### High
- [ ] **RDP Blend/Combine pipeline**: `SET_OTHER_MODES` values are stored in both the RDP (`rdp.rs`) and RSP HLE (`rsp_hle.rs`) but are never consumed by the rendering pipeline. Applying cycle type, texture filtering, and alpha-blending modes would significantly improve visual accuracy. — `crates/systems/n64/src/rdp.rs`, `crates/systems/n64/src/rdp_renderer_opengl.rs`

### Medium
- [ ] **RSP ADPCM decode**: The ADPCM command (0x01) currently writes silence. A proper VADPCM decoder would significantly improve audio quality for games that use ADPCM-compressed audio. — `crates/systems/n64/src/rsp_hle.rs`
- [ ] **RSP Audio RESAMPLE command**: The RESAMPLE command (0x03) is currently a no-op. A proper resample algorithm would improve audio accuracy. — `crates/systems/n64/src/rsp_hle.rs`
- [ ] **RSP F3DEX fog / clipping commands**: Several less-common F3DEX commands still log as stubs: `G_SETPRIMDEPTH`, `G_TEXTURE` (texture coordinate scaling not forwarded to RDP), `G_LOAD_UCODE` (microcode reload). — `crates/systems/n64/src/rsp_hle.rs`
- [ ] **RDP performance counters**: `DPC_CLOCK`, `DPC_BUFBUSY`, `DPC_PIPEBUSY`, and `DPC_TMEM` registers return hardcoded 0. Some games wait for these to reach expected values. — `crates/systems/n64/src/rdp.rs`
- [ ] **CPU overflow traps**: Signed arithmetic instructions (`ADD`, `ADDI`, `SUB`) should raise an overflow exception on overflow, but currently use wrapping arithmetic. Most commercial software does not rely on this, but some may. — `crates/core/src/cpu_mips_r4300i/`

### Low
- [ ] **Memory alignment validation**: Load/store instructions that require alignment (LH/SH = 2 B, LW/SW = 4 B, LD/SD = 8 B) do not raise `AddressError` exceptions on misaligned access. — `crates/core/src/cpu_mips_r4300i/`
- [ ] **Full cache coherency**: The TLB/cache is direct-mapped only; cache invalidation and dirty-line write-back are not emulated. — `crates/systems/n64/src/bus.rs`

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
- [ ] **CD-ROM command handling** — Only GetStat/GetID stubs; no disc data streaming or sector reads. Required for running real game discs. — `crates/systems/ps1/src/lib.rs`

### Medium
- [ ] **Save States** — `save_state()` / `load_state()` are stubs (return empty/no-op). Need full serialization of CPU, GPU, SPU, RAM, and DMA state. — `crates/systems/ps1/src/lib.rs`
- [ ] **MDEC** — Motion Decoder (FMV) stubbed; no macroblock decoding. — `crates/systems/ps1/src/lib.rs`
- [ ] **SPU Reverb** — Reverb processing unit not yet implemented; voices are mixed without reverb. — `crates/systems/ps1/src/spu.rs`
- [ ] **SPU Pitch Modulation** — FM (pitch modulation) between voices not yet implemented. — `crates/systems/ps1/src/spu.rs`
- [ ] **SPU Noise generator** — Noise channel not yet connected (noise_enable register decoded but not used in sample generation). — `crates/systems/ps1/src/spu.rs`
- [ ] **GPU Gouraud shading for lines** — Gouraud (per-vertex color) for line and polyline primitives uses start color only. — `crates/systems/ps1/src/gpu.rs`
- [ ] **Textured primitive semi-transparency** — Semi-transparency for textured primitives uses the STP bit from CLUT entries which is not yet propagated correctly. — `crates/systems/ps1/src/gpu.rs`

### Low
- [ ] **Joypad/Controller input** — Controller polling via SIO is stubbed; digital pad button state not connected to the emulator input system. — `crates/systems/ps1/src/lib.rs`
- [ ] **Memory Card** — Memory card SIO protocol not implemented. — `crates/systems/ps1/src/lib.rs`

## NES

### Medium
- [ ] **OAMDATA ($2004) reads during active rendering**: During visible scanlines (0–239) with rendering enabled, $2004 should return `$FF` on dots 0–64 (secondary OAM clear phase) and dots 257–340 (sprite tile fetch), and the primary OAM Y-byte of the sprite currently being evaluated (`oam[sprite_index * 4]`, where `sprite_index = (dot - 65) / 2`) on dots 65–256. Currently always returns `OAM[OAMADDR]` regardless of rendering state. Games like Bee 52 read $2004 to synchronise with the PPU and time HUD scroll splits; wrong values cause timing loops to fail. Reference: NESdev wiki PPU sprite evaluation, Mesen2 `NesPpu.cpp ReadRam()` — `crates/systems/nes/src/ppu.rs`
- [ ] **Mid-scanline PPUMASK re-enable re-render**: When PPUMASK transitions from rendering-disabled to rendering-enabled during a visible scanline (0–239), that scanline has already been drawn as backdrop at dot 0 and must be re-rendered with the current scroll and mask values. Needed for HUD scroll splits in Bee 52 and other Codemasters games that disable rendering to change scroll, then re-enable mid-scanline. Proposed approach from PR #632: add a `rerender_scanline: Cell<Option<u16>>` field set in `write_register(1)` when the transition is detected, then re-render in the main loop after each CPU step. Reference: PR #632 investigation, NESdev wiki mid-frame updates — `crates/systems/nes/src/ppu.rs`, `crates/systems/nes/src/lib.rs`

## SNES

### High
- [ ] **DSP echo/reverb FIR filter not implemented**: The SPC700 DSP echo buffer and 8-tap FIR reverb filter are not emulated. Many games rely on reverb for audio quality (e.g., Donkey Kong Country series). — `crates/systems/snes/src/`
- [ ] **SuperFX coprocessor incomplete**: Only ~20% of SuperFX opcodes implemented. Star Fox, Yoshi's Island, Doom etc. will not work. — `crates/systems/snes/src/coprocessors/superfx.rs`

### Medium
- [ ] **Multiply/divide hardware timing**: Math unit computes results immediately. Hardware takes 8 cycles (multiply) / 16 cycles (divide). May affect timing-sensitive code. — `crates/systems/snes/src/bus.rs`

## Mega Drive / Genesis

### Medium
- [ ] **Z80 sound CPU**: Z80 sub-CPU is not emulated. FM/PSG playback relies on 68K writing to YM2612/PSG registers directly, but games that use the Z80 for sound driver won't have audio. — `crates/systems/megadrive/src/bus.rs`
- [ ] **YM2612 FM synthesis accuracy**: Timer and envelope generator are simplified. LFO, SSG-EG, and DAC mixing not fully accurate. — `crates/systems/megadrive/src/ym2612.rs`
- [ ] **H-interrupt timing**: HInt counter and timing may not be cycle-accurate for raster effect games. — `crates/systems/megadrive/src/vdp.rs`
- [ ] **Window plane clipping**: Window plane rendering may have edge cases with complex window configurations. — `crates/systems/megadrive/src/vdp.rs`
- [ ] **DMA timing**: DMA transfers are instant; real hardware transfers data across multiple scanlines, affecting CPU access timing. — `crates/systems/megadrive/src/vdp.rs`

### Low
- [ ] **Interlace modes**: Interlaced display modes (reg $0C bits 1-2) not implemented. — `crates/systems/megadrive/src/vdp.rs`
- [ ] **SRAM save/load persistence**: SRAM is detected but not persisted to disk between sessions. — `crates/systems/megadrive/src/bus.rs`
- [ ] **Controller input 6-button support**: Only 3-button controller protocol emulated. — `crates/systems/megadrive/src/bus.rs`
