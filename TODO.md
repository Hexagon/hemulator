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

### High
- [ ] **Frame interrupt not firing in `SMS_waitForVBlank()`**: CPU gets stuck in the VDP V-counter polling loop at the `SMS_waitForVBlank()` call (PC≈0x757D in SMSTestSuite). The `frame_interrupt_pending` flag is never set during this phase. Root cause: `set_scanline()` is called via cycle-count interpolation in `step_frame`; if VDP registers are written (enabling the frame interrupt) *after* the scanline has already crossed the VBlank boundary in the same frame, the interrupt enable check in `set_scanline` happens before the flag is set in `registers[1]`. This means `SMS_init` sets R1=0x20 (frame-int enable) just after the VBlank crossing has been missed, so the first real interrupt never fires. Fix: track the state of `registers[1]` frame-interrupt-enable bit transitions and fire the interrupt retroactively if already in VBlank, or use a separate `frame_interrupt_enabled` latch. — `crates/systems/sms/src/vdp.rs`, `crates/systems/sms/src/system.rs`
- [ ] **SMSTestSuite main menu not rendered**: Depends on the frame interrupt fix above. Once `SMS_waitForVBlank()` returns correctly, `SMS_init` will complete and the main menu should display. Validate using `test_roms/sms/SMSTestSuite.sms` smoke test (`smoke_test_sms_test_suite` — currently asserts only alpha channel and PC advancement). — `crates/systems/sms/src/system.rs`

### Medium
- [ ] **Line interrupt counter reload timing**: The line counter is reloaded from R10 at scanline 192 (start of VBlank). Verify correct reload behavior for games that change R10 mid-frame. — `crates/systems/sms/src/vdp.rs`
- [ ] **`SMS_waitForVBlank()` V-counter polling**: SMSlib polls port 0x7E until the V-counter wraps from 0xF2/0xDA back to a low value. Ensure the V-counter wraps correctly at the end of each frame (NTSC: wraps at 0xF3→0x00, PAL: 0xF3→0x00 with jump at 0xF2). — `crates/systems/sms/src/vdp.rs`

## GBA

### High
- [ ] **Affine ref point writes via DMA**: Verify HBlank DMA correctly updates PPU affine reference points for Mode 7 games (F-Zero, Mario Kart)
- [ ] **Medal of Honor white screen**: HBlank polling loop — verify HBlank timing fix resolves the issue

### Medium
- [ ] **Implement halt/stop modes** (`HALTCNT` at `0x04000301`) — `crates/systems/gba/src/lib.rs`
- [ ] **HBlank OAM access restriction** (`DISPCNT` bit 5) — `crates/systems/gba/src/ppu.rs`

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
