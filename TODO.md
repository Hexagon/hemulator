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
