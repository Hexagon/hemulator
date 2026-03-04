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

### Critical
- [ ] **SPC700 second upload self-corruption**: N-SPC upload handler at $1315 (`MOV [$14]+Y, A`) writes uploaded data to $1300 (itself) because ZP $14-$15 gets set to $1300 instead of $8100. Root cause is likely a port read timing / synchronization issue between main CPU and SPC700 during the block header read at $1325. — `crates/core/src/apu/spc700.rs`, `crates/systems/snes/src/bus.rs`
  - See `SPC_ANALYSIS.md` for full disassembly and trace evidence
  - Investigate block transition timing (stale port values at $1325 MOVW)
  - Compare SPC700/CPU clock sync with reference emulator (ares/bsnes)

### High
- [ ] **Clean up SPC700 debug infrastructure**: Remove all temporary tracing after the upload bug is resolved — `crates/core/src/apu/spc700.rs`, `crates/core/src/cpu_spc700.rs`, `crates/systems/snes/src/bus.rs`
  - Remove `trace_ports`, `last_pc`, `port_trace_count` fields from `Spc700Memory`
  - Remove PORT-READ/PORT-WRITE/CTRL-WRITE file tracing to `spc700_diag.txt`
  - Remove DIAG snapshots every 5000 cycles
  - Remove SPC700-TRACE for N-SPC addresses
  - Remove watchpoint on $1308–$130C
  - Remove jump detection RAM dump
  - Remove timer counter `eprintln!` in COUNTER0 read handler
  - Remove per-instruction logging ($0100–$0FFF) in `cpu_spc700.rs` step()
  - Remove port read/write tracing at $2140–$2143 in `bus.rs`
  - Restore `config.json` `log_rate_limit` from 10000000 to 20
