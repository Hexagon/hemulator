# TODO

## Dev Shell (`dev.ps1`)

- [x] `build` — Build workspace with `release-quick` profile
- [x] `run` — Build and run emulator with a ROM
- [x] `test` — Run all workspace tests
- [x] `clippy` — Clippy with `-D warnings`
- [x] `fmt` — Format all code
- [x] `check` — Full CI pipeline (fmt + clippy + build + test)
- [x] `trace` — Run with instruction tracing
- [x] `dump` — Headless debug dump
- [x] `cpu` — Run with CPU trace/debug logging (`--log-cpu`)
- [x] `gpu` — Run with PPU trace/debug logging (`--log-ppu`)
- [ ] `apu` — Run with APU logging (`--log-apu`)
- [ ] `bus` — Run with bus logging (`--log-bus`)
- [ ] `all-logs` — Run with all log categories enabled

## SMS

### Medium
- [ ] **Line interrupt counter reload timing**: The line counter is reloaded from R10 at scanline 192 (start of VBlank). Verify correct reload behavior for games that change R10 mid-frame. — `crates/systems/sms/src/vdp.rs`

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
- [x] **JMP/JSR absolute indexed indirect (`jmp ($addr,x)`) bank-wrap bug** — fixed: `ptr+X` now wraps within 16 bits (stays in PBR bank); cputest-basic advances from `0x01b7` → `0x025d`
- [x] **`(dp,X)` indirect pointer read page-wrap in emulation mode (`E=1`)** — fixed: all 8 `(dp,X)` instructions (ORA/AND/EOR/ADC/STA/LDA/CMP/SBC) now use `read_word_dp_wrapped` which wraps the hi-byte read within the direct page in emulation mode; cputest-full advances from `0x0024` → `0x0025`
- [ ] **cputest-basic fails at test `0x025d`** — next failing test after bank-wrap fix; needs investigation
  - File: `crates/core/src/cpu_65c816.rs`
  - Raise `MIN_PASSING` in `test_cputest_basic_loads_and_runs` once resolved
- [ ] **cputest-full fails at test `0x0025`** — next failing emulation-mode test after dp-wrap fix; needs investigation
  - File: `crates/core/src/cpu_65c816.rs`
  - Raise `MIN_PASSING` in `test_cputest_full_loads_and_runs` once resolved
