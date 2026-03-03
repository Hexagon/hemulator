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

## N64

### Critical
- [ ] **FlashRAM command protocol** (`Macronix MX29L1100`): SRAM is used as a simple byte array stand-in for FlashRAM but the real protocol has erase/write commands that games issue before writing. Games that use FlashRAM (Pokémon Stadium, etc.) will not save correctly until the protocol is implemented. — `crates/systems/n64/src/bus.rs` (`configure_save_type()`)
- [ ] **Save type detection for unlisted games**: Only common retail titles are in the save-type database. Unknown games silently receive `SaveType::None`. Consider implementing a checksum-based database fallback (e.g. using the ipl3 CRC or the CIC seed) so that unlisted games still get a sensible default. — `crates/systems/n64/src/cartridge.rs`

### High
- [ ] **RDP Blend/Combine pipeline**: `SET_OTHER_MODES` values are stored in both the RDP (`rdp.rs`) and RSP HLE (`rsp_hle.rs`) but are never consumed by the rendering pipeline. Applying cycle type, texture filtering, and alpha-blending modes would significantly improve visual accuracy. — `crates/systems/n64/src/rdp.rs`, `crates/systems/n64/src/rdp_renderer_opengl.rs`
- [ ] **RSP Audio microcode (ABI1/ABI2)**: Audio tasks are detected but silently skipped (the stub at `execute_audio_task` logs the task structure then returns without processing). This means N64 games produce no game audio. A basic ABI1 implementation (ADPCM decode, mix, resample) would fix most commercial titles. — `crates/systems/n64/src/rsp_hle.rs`
- [ ] **Controller Pak (memory card)**: PIF commands 0x02 (read pak) and 0x03 (write pak) are not implemented. Games that require a Controller Pak for save data (e.g. Wave Race 64) will not save. — `crates/systems/n64/src/pif.rs`

### Medium
- [ ] **RSP F3DEX fog / clipping commands**: Several less-common F3DEX commands still log as stubs: `G_SETPRIMDEPTH`, `G_TEXTURE` (texture coordinate scaling not forwarded to RDP), `G_LOAD_UCODE` (microcode reload). — `crates/systems/n64/src/rsp_hle.rs`
- [ ] **RDP performance counters**: `DPC_CLOCK`, `DPC_BUFBUSY`, `DPC_PIPEBUSY`, and `DPC_TMEM` registers return hardcoded 0. Some games wait for these to reach expected values. — `crates/systems/n64/src/rdp.rs`
- [ ] **CPU overflow traps**: Signed arithmetic instructions (`ADD`, `ADDI`, `SUB`) should raise an overflow exception on overflow, but currently use wrapping arithmetic. Most commercial software does not rely on this, but some may. — `crates/core/src/cpu_mips_r4300i/`

### Low
- [ ] **RSP semaphore and signal bits**: The SP semaphore register always returns 0; `SIG0–SIG7` bits are not implemented. Some games use these to synchronise CPU and RSP workloads. — `crates/systems/n64/src/rsp.rs`
- [ ] **Memory alignment validation**: Load/store instructions that require alignment (LH/SH = 2 B, LW/SW = 4 B, LD/SD = 8 B) do not raise `AddressError` exceptions on misaligned access. — `crates/core/src/cpu_mips_r4300i/`
- [ ] **Full cache coherency**: The TLB/cache is direct-mapped only; cache invalidation and dirty-line write-back are not emulated. — `crates/systems/n64/src/bus.rs`

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
