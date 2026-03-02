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

## GBA

### High
- [ ] **Affine ref point writes via DMA**: Verify HBlank DMA correctly updates PPU affine reference points for Mode 7 games (F-Zero, Mario Kart)
- [ ] **Medal of Honor white screen**: HBlank polling loop — verify HBlank timing fix resolves the issue

### Medium
- [ ] **Implement halt/stop modes** (`HALTCNT` at `0x04000301`) — `crates/systems/gba/src/lib.rs`
- [ ] **HBlank OAM access restriction** (`DISPCNT` bit 5) — `crates/systems/gba/src/ppu.rs`

## SNES

### High
- [ ] **JMP absolute indexed indirect (`jmp ($addr,x)`) bug** — cputest-basic fails at test `0x01b7`
  - Instruction: `jmp ($F000,x)` with `X=$6000`, `DBR=$7F`
  - Indirect address should be read from bank 0 (`$007000`), not from DBR
  - Fix: `jmp ($addr,x)` must use program bank (PBR), not data bank register (DBR), for the pointer fetch
  - File: `crates/core/src/cpu_65c816/` (JMP indirect indexed opcode handler)
  - Raise `MIN_PASSING` in `test_cputest_basic_loads_and_runs` once resolved
- [ ] **ADC indirect indexed in emulation mode (`E=1`) bug** — cputest-full fails at test `0x0024`
  - Instruction: `adc ($EF,x)` with `E=1`, `D=$0100`, `X=$0010`, wrapping within stack page
  - Emulation-mode `(direct,X)` indirect addressing does not correctly handle page wrapping when `D` low byte is non-zero
  - Fix: in emulation mode, the high byte of the indirect address must wrap within the direct page (see gilyon/snes-tests README for full undocumented behavior spec)
  - File: `crates/core/src/cpu_65c816/` (emulation-mode ADC indirect-indexed handler)
  - Raise `MIN_PASSING` in `test_cputest_full_loads_and_runs` once resolved
