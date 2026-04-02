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

## SMS

### High
- [ ] **TMS9918A mode sprite rendering missing**: Sprites are not rendered in TMS9918A compatibility modes (0–3); only Mode 4 (SMS native) supports sprites. Affects backward-compatible SMS software that uses TMS modes. — `crates/systems/sms/src/vdp.rs`

### Medium
- [ ] **Line interrupt counter reload timing**: The line counter is reloaded from R10 at scanline 192 (start of VBlank). Verify correct reload behavior for games that change R10 mid-frame. — `crates/systems/sms/src/vdp.rs`

### Low
- [ ] **Game Gear not supported**: The Game Gear is a handheld derived from the SMS with a 160×144 display viewport, 64-byte CRAM, and different I/O port layout. — `crates/systems/sms/`

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
- [ ] **Sprite 0 hit timing**: Fix trigger to fire at `hit_x + 1` (not `hit_x + 2`), and restrict the detection window to dots 1–256 (not 2–257). Per hardware, dot 1 = pixel x=0, so the hit fires at dot = x + 1. The current one-dot offset causes incorrect sprite 0 hit timing for games that use it to split the screen (e.g. Bee 52 HUD). Reference: Mesen2 `NesPpu.cpp GetPixelColor()`, NESdev wiki PPU sprite evaluation — `crates/systems/nes/src/ppu.rs`
- [ ] **Odd-frame cycle skip NTSC-only gate**: The skip at pre-render scanline dot 339 must only apply when `TimingMode::Ntsc`. The PAL PPU (2C07) never performs this skip; every PAL frame is exactly 312 × 341 dots. Without the guard, PAL games lose one dot per frame, causing cumulative CPU/PPU drift. Reference: NESdev wiki PPU frame timing, NESdev wiki PAL video — `crates/systems/nes/src/ppu.rs`
- [ ] **OAMDATA ($2004) reads during active rendering**: During visible scanlines (0–239) with rendering enabled, $2004 should return `$FF` on dots 0–64 (secondary OAM clear phase) and dots 257–340 (sprite tile fetch), and the primary OAM Y-byte of the sprite currently being evaluated (`oam[sprite_index * 4]`, where `sprite_index = (dot - 65) / 2`) on dots 65–256. Currently always returns `OAM[OAMADDR]` regardless of rendering state. Games like Bee 52 read $2004 to synchronise with the PPU and time HUD scroll splits; wrong values cause timing loops to fail. Reference: NESdev wiki PPU sprite evaluation, Mesen2 `NesPpu.cpp ReadRam()` — `crates/systems/nes/src/ppu.rs`
- [ ] **Mid-scanline PPUMASK re-enable re-render**: When PPUMASK transitions from rendering-disabled to rendering-enabled during a visible scanline (0–239), that scanline has already been drawn as backdrop at dot 0 and must be re-rendered with the current scroll and mask values. Needed for HUD scroll splits in Bee 52 and other Codemasters games that disable rendering to change scroll, then re-enable mid-scanline. Proposed approach from PR #632: add a `rerender_scanline: Cell<Option<u16>>` field set in `write_register(1)` when the transition is detected, then re-render in the main loop after each CPU step. Reference: PR #632 investigation, NESdev wiki mid-frame updates — `crates/systems/nes/src/ppu.rs`, `crates/systems/nes/src/lib.rs`

### Low
- [ ] **ROM DB lookup test — use dynamic entry**: `test_lookup_rom_found` in `rom_db.rs` hardcodes Bee 52's CRC32 (`0xE19C2722`). Replacing the lookup with `ROM_DATABASE[0]` (and asserting all fields match) makes the test database-agnostic and prevents breakage if the Bee 52 entry is ever removed or reordered. — `crates/systems/nes/src/rom_db.rs`

## SNES

### High
- [ ] **DSP echo/reverb FIR filter not implemented**: The SPC700 DSP echo buffer and 8-tap FIR reverb filter are not emulated. Many games rely on reverb for audio quality (e.g., Donkey Kong Country series). — `crates/systems/snes/src/`
- [ ] **cputest-basic fails at test `0x025d`** — next failing test after bank-wrap fix; needs investigation
  - File: `crates/core/src/cpu_65c816.rs`
  - Raise `MIN_PASSING` in `test_cputest_basic_loads_and_runs` once resolved
- [ ] **cputest-full fails at test `0x0025`** — next failing emulation-mode test after dp-wrap fix; needs investigation
  - File: `crates/core/src/cpu_65c816.rs`
  - Raise `MIN_PASSING` in `test_cputest_full_loads_and_runs` once resolved

### Medium
- [ ] **DSP noise generator and pitch modulation missing**: Noise generation and FM-style pitch modulation between DSP voices are not implemented; affects some sound effects in games. — `crates/systems/snes/src/`
- [ ] **Interlace and overscan modes not rendered**: SETINI register bits for interlace (bit 0) and overscan (bit 2) are tracked but not applied; games using these modes may display at incorrect resolution or crop incorrectly. — `crates/systems/snes/src/ppu.rs`

## Game Boy

### Medium
- [ ] **MBC3 RTC accuracy**: RTC ticks at ~60 Hz (frame rate) instead of hardware-accurate 1 Hz, causing real-time drift. RTC state is not persisted across emulator sessions. — `crates/systems/gb/src/mappers/mbc3.rs`
- [ ] **Link cable not emulated**: Serial transfer uses loopback only; multiplayer and Pokémon trading features require actual link cable protocol emulation. — `crates/systems/gb/src/bus.rs`

### Low
- [ ] **Unimplemented mappers** (MBC6, MBC7, HuC3, MMM01, TAMA5): Affects <3% of games combined. MBC7 required for Kirby Tilt 'n' Tumble (tilt sensor), HuC3 for Robopon series. — `crates/systems/gb/src/mappers/`

## Atari 2600

### Medium
- [ ] **Paddle controller GUI integration**: TIA analog input (INPT0–INPT3) with capacitor timing simulation is fully implemented in hardware, but no frontend input mapping exists. Required for paddle games (Breakout, Kaboom!, Warlords). — `crates/frontend/gui/src/`

## Atari 5200

### Critical
- [ ] **Video output not functional**: ANTIC display list processor and GTIA are partially implemented but the rendering pipeline is incomplete; most games cannot be played. — `crates/systems/atari5200/src/`

### High
- [ ] **BIOS behavior/integration incomplete**: Custom BIOS images can be loaded via the BIOS mount point, but the built-in BIOS stub at $F800–$FFFF is incomplete and BIOS compatibility is not yet accurate. Games that rely on specific BIOS routines will malfunction. — `crates/systems/atari5200/src/`
- [ ] **Paddle/analog input not connected**: POKEY analog input is not wired to the frontend input system; affects all 5200 games using the console's non-centering analog joystick. — `crates/systems/atari5200/src/`

## CHIP-8

### Low
- [ ] **No audio synthesis**: Sound timer and XO-CHIP audio pattern buffer are tracked correctly but no actual tone is generated. All CHIP-8 programs are silent. — `crates/systems/chip8/src/lib.rs`

## ColecoVision

### High
- [ ] **Numeric keypad not implemented**: Only joystick and fire buttons are connected in the controller input system; the 12-key numeric keypad is not functional. Affects games that require keypad input for difficulty selection or gameplay. — `crates/systems/colecovision/src/lib.rs`

## Mega Drive

### Critical
- [ ] **Z80 sub-CPU integration incomplete**: Z80 bus access and banking are not fully integrated. Most commercial games use the Z80 to drive the YM2612 audio driver; without it, sound is missing or incorrect in the majority of titles. — `crates/systems/megadrive/src/`

### High
- [ ] **Audio output accuracy needs refinement**: YM2612 FM and SN76489 PSG samples are already mixed into the stereo output, but overall audio behavior still likely needs accuracy work in areas such as relative levels, panning, filtering, and timing/synchronization. — `crates/systems/megadrive/src/`
- [ ] **SRAM not included in save states**: Battery-backed SRAM is detected from the ROM header and memory-mapped for reads/writes, but SRAM contents are not serialized in `save_state()`/`load_state()`. Game progress stored in SRAM is lost when the emulator exits. — `crates/systems/megadrive/src/system.rs`

## PC

### High
- [ ] **INT 21h (DOS API) mostly stubbed**: Real file I/O, program management, process control, and many DOS services beyond basic console output are not implemented. Most real-world DOS programs will fail. — `crates/systems/pc/src/cpu.rs`
- [ ] **No 32-bit (80386+) support**: 32-bit register extensions (EAX, EBX, ECX, EDX, ESI, EDI, EBP, ESP) and 32-bit addressing modes are not implemented. Software requiring 386+ features will not run. — `crates/core/src/cpu_8086/`

### Medium
- [ ] **PC speaker audio not connected**: PIT channel 2 frequency and gate state are tracked internally but audio output is not generated. Programs using PC speaker sound will be silent. — `crates/systems/pc/src/lib.rs`
