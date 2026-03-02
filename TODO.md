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
