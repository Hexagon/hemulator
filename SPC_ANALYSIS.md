# SPC700 / SNES Boot Analysis

**Date**: March 4, 2026 (initial), resolved March 4, 2026 (second session)  
**Test ROM**: `roms/snes/Super Mario World (USA).sfc`  
**Status**: ✅ RESOLVED — Second SPC700 upload fixed (sync timing + dp addressing)

---

## Background

Super Mario World was not booting. Investigation revealed multiple bugs in the SPC700 CPU and APU I/O subsystem.

## Bugs Found & Fixed

### SPC700 I/O Bugs (3) — `crates/core/src/apu/spc700.rs`

1. **Control register port clear direction** (CRITICAL): Bits 4–5 of control register ($F1) were clearing `apu_out` (SPC→CPU output) instead of `cpuio` (CPU→SPC input). Fixed to clear `cpuio[0..1]` / `cpuio[2..3]`.
2. **IPL ROM region writes**: Writes to $FFC0–$FFFF were blocked when IPL ROM was enabled. Fixed: writes always go to RAM regardless of ROM enable bit.
3. **I/O register RAM passthrough**: Writes to I/O registers ($F0–$FF) were not mirrored to RAM. Fixed: all I/O writes also update `self.ram[addr]`.

### SPC700 CPU Instruction Bugs (~18) — `crates/core/src/cpu_spc700.rs`

| Opcode | Was | Fixed To | Impact |
|--------|-----|----------|--------|
| `$DB` | 16-bit word read into X,Y | `MOV dp+X, Y` (store Y to dp+X) | **Critical** — destroyed X, broke port echo |
| `$D4` | `MOV dp, X` | `MOV dp+X, A` (store A to dp+X, 5 cycles) | Wrong register & addressing |
| `$D8` | Indirect Y store | `MOV dp, X` (simple dp store, 4 cycles) | Completely wrong operation |
| `$D9` | Stored A instead of X | `MOV dp+Y, X` | Wrong source register |
| `$C9` | `CMP X, dp` | `MOV !abs, X` (absolute store, 3-byte) | Compare vs store confusion |
| `$CC` | Stored X instead of Y | `MOV !abs, Y` | Wrong source register |
| `$C7` | MOV1 logic | `MOV [dp+X], A` (indexed indirect store) | Completely wrong operation |
| `$CA` | Byte-copy logic | `MOV1 mem.bit, C` (carry to memory bit) | Completely wrong operation |
| `$C6` | Missing `direct_page()` | `MOV (X), A` with correct DP offset | Address calculation error |
| `$D7` | Missing `direct_page()` on ptr | `MOV [dp]+Y, A` with correct DP offset | Address calculation error |
| `$E7` | Missing X index + `direct_page()` | `MOV A, [dp+X]` | Address calculation error |
| `$E9` | `EOR dp,dp` | `MOV X, !abs` | Completely wrong operation |
| `$F9` | Nonexistent `MOV Y,X` | `MOV X, dp+Y` | Wrong instruction entirely |
| `$F7` | Missing `direct_page()` | `MOV A, [dp]+Y` with correct DP | Address calculation error |
| `$E6` | Missing `direct_page()` | `MOV A, (X)` with correct DP | Address calculation error |
| `$FA` | Missing `direct_page()` on both | `MOV dp, dp` with correct DP offsets | Address calculation error |
| `$BA` | Z flag only checked Y | `MOVW YA, dp` — Z flag checks full 16-bit YA | Flag calculation error |
| `$F4` | Missing `direct_page()` | `MOV A, dp+X` with correct DP | Address calculation error |
| `$FB` | Missing `direct_page()` | `MOV Y, dp+X` with correct DP | Address calculation error |

## Current Boot Progress

After all fixes, the SNES boot sequence progresses as follows:

### ✅ Phase 1 — IPL ROM Handshake
- SPC700 boots from IPL ROM at $FFC0
- Writes $AA to port 0, $BB to port 1 (the `$BBAA` signature)
- Main CPU detects $BBAA at $2140

### ✅ Phase 2 — First Upload (3 blocks)
Main CPU uploads code from ROM bank $0E:
| Block | Size | Destination | Content |
|-------|------|-------------|---------|
| 0 | $0E3E bytes | $0500 | N-SPC audio engine (main code) |
| 1 | $0A6B bytes | $5570 | N-SPC audio engine (data/samples) |
| 2 | $161D bytes | $1360 | N-SPC audio engine (extended routines) |
Then jumps SPC700 to $0500.

### ✅ Phase 3 — N-SPC Engine Init
- SPC700 enters N-SPC engine at $0500
- Init routine at $0536 writes $F0 to ctrl (clear ports, enable IPL), then $01 (enable Timer 0)
- Engine enters main loop at $0549 (timer-driven)

### ✅ Phase 4 — $FF Command Detection
- Main CPU writes $FF to port 1 ($2141)
- N-SPC port echo at $05A5 detects change via CBNE
- Command dispatch at $09E5 reads ZP $01 = $FF
- $FF handler at $099C: keys off all voices, calls $12F2

### ✅ Phase 5 — $BBAA Re-Signal
- $12F2 writes $AA to port $F4, $BB to port $F5
- Main CPU sees $BBAA and begins second upload

### ❌ Phase 6 — Second Upload (CURRENT FAILURE)
Main CPU uploads from ROM bank $0F:
| Block | Size | Destination | Content |
|-------|------|-------------|---------|
| 0 | $0050 bytes | $8000 | Unknown |
| 1 | $6F20 bytes | $8100 | Unknown |
Then should jump SPC700 to $0500.

**The upload never completes.**

## Root Cause Analysis

### The N-SPC Upload Handler at $1300

After the $BBAA re-signal, the SPC700 does NOT re-enter the IPL ROM. Instead, the N-SPC engine has its own upload handler at $12F2–$133D. After writing $BBAA, this code falls through to a custom upload loop at $1300:

```
$12F0:  1F 00          JMP ($0000+X)  ; (part of earlier code, entry is $12F2)
$12F2:  E8 AA          MOV A, #$AA
$12F4:  C5 F4 00       MOV $00F4, A   ; Write $AA to port 0
$12F7:  E8 BB          MOV A, #$BB
$12F9:  C5 F5 00       MOV $00F5, A   ; Write $BB to port 1
$12FC:  E5 F4 00       MOV A, $00F4   ; Read port 0 (from CPU)
$12FF:  68 CC          CMP A, #$CC    ; Wait for $CC handshake
$1301:  D0 F9          BNE $12FC      ; Loop until $CC received

; --- Upload loop entry ---
$1303:  2F 20          BRA $1325      ; Jump to block header read

; --- Byte upload loop ---
$1305:  EC F4 00       MOV Y, $00F4   ; Y = port 0 (counter from CPU)
$1308:  D0 FB          BNE -5         ; Wait until counter changes? (loops on Y≠0)
$130A:  5E F4 00       CMP Y, $00F4   ; Compare Y with current port 0 value
$130D:  D0 0F          BNE $131E      ; If changed, handle block end
$130F:  E5 F5 00       MOV A, $00F5   ; Read data byte from port 1
$1312:  CC F4 00       MOV $00F4, Y   ; Echo counter to port 0 (acknowledge)
$1315:  D7 14          MOV [$14]+Y, A ; Store byte at [ZP$14]+Y  ← THE PROBLEM
$1317:  FC             INC Y
$1318:  D0 F0          BNE $130A      ; Loop until Y wraps (256 bytes)
$131A:  AB 15          INC $15        ; Increment high byte of dest pointer
$131C:  2F EC          BRA $130A      ; Continue uploading

; --- Block end / new block ---
$131E:  10 EA          BPL $130A      ; If port0 >= 0, continue upload loop
$1320:  5E F4 00       CMP Y, $00F4   ; Re-check port 0
$1323:  10 E5          BPL $130A      ; Continue if still positive

; --- Read block header ---
$1325:  E5 F6 00       MOV A, $00F6   ; Read dest addr low from port 2
$1328:  EC F7 00       MOV Y, $00F7   ; Read dest addr high from port 3
$132B:  DA 14          MOVW $14, YA   ; Store 16-bit dest addr to ZP $14-$15
$132D:  EC F4 00       MOV Y, $00F4   ; Read counter from port 0
$1330:  E5 F5 00       MOV A, $00F5   ; Read first data byte from port 1
$1333:  CC F4 00       MOV $00F4, Y   ; Acknowledge (echo counter)
$1336:  D0 CD          BNE $1305      ; If data != 0, enter byte loop (more data)

; --- Upload complete, jump ---
$1338:  CD 31          MOV X, #$31    ; Set SP? (X = $31)
$133A:  C9 F1 00       MOV $00F1, X   ; Write to control register ($F1)
$133D:  6F             RET            ; Return (to N-SPC main loop → re-enters $0500)
```

### The Bug: ZP $14-$15 = $1300 (self-corruption)

**Watchpoint evidence:**
```
WATCHPOINT: Write $00 to $1308 (was $D0) from PC=$1315 ZP14-15=[00,13] ptr=$1300
WATCHPOINT: Write $00 to $1309 (was $FB) from PC=$1315 ZP14-15=[00,13] ptr=$1300
WATCHPOINT: Write $00 to $130A (was $5E) from PC=$1315 ZP14-15=[00,13] ptr=$1300
WATCHPOINT: Write $00 to $130B (was $F4) from PC=$1315 ZP14-15=[00,13] ptr=$1300
```

The `MOV [$14]+Y, A` at $1315 is the store instruction in the upload loop. ZP $14-$15 holds the destination pointer, which was set to `$1300` — meaning the upload handler is **overwriting itself** with uploaded data (which happens to be $00 NOP bytes at that offset).

### Why ZP $14-$15 = $1300

Looking at the trace:
```
PC=$132B A=00 X=00 Y=80 → MOVW $14, YA   ; YA = Y:A = $80:$00 = $8000 ✓ (first block dest)
PC=$1336 A=01 X=00 Y=CC → BNE $1305      ; A=01 (non-zero), enters byte loop
```

The first block header read correctly gets dest=$8000. The upload then proceeds (port 0 increments from $CC upward). But at some point the `INC $15` at $131A or the block-end logic produces a pointer of $1300.

**Most likely scenario**: After uploading 256 bytes to $80xx, `INC $15` at $131A bumps the high byte ($80→$81, etc.), and eventually the second block header at $1325 reads dest=$8100 correctly. But there appears to be a **synchronization failure** between CPU and SPC700 where the CPU sends a new block header but the SPC700 misinterprets the port values, resulting in ZP $14-$15 being set to $1300 instead of the intended $8100.

### Suspect: Port read timing / synchronization

The last trace before the block header read:
```
PC=$1305 A=01 X=00 Y=CC cpuio=[00,00,00,80]
```

Note `cpuio=[00,00,00,80]` — port 2 = $00, port 3 = $80. But this is the **first** block's header ($8000). The second block should have ports 2–3 = $00,$81 ($8100). The SPC700 may be reading stale port values because:

1. **The main CPU hasn't written the new header yet** (timing issue — SPC700 runs ahead)
2. **A port read bug** causes the SPC700 to see wrong values
3. **The $1305 wait loop at bytes $1308–$1309** (`D0 FB` = BNE $1305) may be misinterpreted after the first 256-byte block completes

## Key Diagnostic Infrastructure (Currently Present)

All of this is **debug code that should be removed** once the issue is resolved:

### `crates/core/src/apu/spc700.rs`
- `trace_ports: bool` field — enables file-based tracing
- `last_pc: u16` field — tracks SPC700 PC for watchpoint
- `port_trace_count: Cell<u32>` — limits to 2000 port trace messages
- PORT-READ/PORT-WRITE file tracing to `spc700_diag.txt`
- CTRL-WRITE tracing for $F1 writes
- DIAG snapshots every 5000 `run_cycles` calls
- SPC700-TRACE for key N-SPC addresses ($099C, $12F2, $09E5, $1303, $1305, $1325, $132B, $1336, etc.)
- Jump detection (IPL ROM → user code) with 2KB RAM dump
- Watchpoint on $1308–$130C with PC + ZP dump
- Timer 0 counter read `eprintln!` for non-zero values

### `crates/core/src/cpu_spc700.rs`
- Per-instruction logging for addresses $0100–$0FFF at Info level

### `crates/systems/snes/src/bus.rs`
- Port read/write tracing at $2140–$2143

### Other
- `config.json`: `log_rate_limit` set to 10000000 (should be 20)

## Code Review Findings (March 4, 2026 session)

### Finding 1: `INC dp` (opcode $AB) missing `direct_page()` call

In `crates/core/src/cpu_spc700.rs` line ~605, opcode $AB (`INC dp`) does:
```rust
0xAB => {
    let addr = self.fetch_byte() as u16;  // ← BUG: no direct_page()!
    let val = self.memory.read(addr);
    ...
}
```
It should be `let addr = self.direct_page() | (self.fetch_byte() as u16);`. This means when the PSW P flag is set (direct page = $0100), `INC $15` would write to $0015 instead of $0115.

**Impact on current bug**: Likely **not** the root cause since the N-SPC engine probably runs with P=0 (direct page = $0000), but it IS a real bug that needs fixing. May affect other games.

### Finding 2: Sync timing creates split-read race condition (LIKELY ROOT CAUSE)

The `sync_spc700()` mechanism in `bus.rs` (line ~397) only runs the SPC700 when the main CPU accesses APU ports ($2140–$2143). Between port accesses, cycles accumulate in `spc700_pending_cycles`. When the main CPU writes a multi-byte value across ports (e.g., destination address to ports 2+3), the sequence is:

1. Main CPU writes low byte to $2142 → `sync_spc700()` flushes pending cycles, THEN writes port 2
2. Main CPU writes high byte to $2143 → `sync_spc700()` flushes pending cycles, THEN writes port 3

**The problem**: Between steps 1 and 2, `sync_spc700()` runs all accumulated SPC700 cycles. During that batch, if the SPC700 reaches the block header read at $1325–$132B:
- `MOV A, $00F6` at $1325 reads port 2 = **new low byte** (just written)
- `MOV Y, $00F7` at $1328 reads port 3 = **old high byte** (not yet written!)
- `MOVW $14, YA` at $132B stores a **corrupt hybrid address**

This is a classic **torn read** / split-write race. The SPC700 sees half-old, half-new header data.

**How this produces $1300**: If port 2 = $00 (new, correct for $8100 low byte) and port 3 = $13 (some stale/intermediate value), dest becomes $1300. OR if the race hits at a different point in the upload sequence where ports contain residual values.

### Finding 3: BNE branch offset is correct

Opcode $D0 (BNE) at `cpu_spc700.rs` line ~265 correctly handles signed offsets:
```rust
let offset = self.fetch_byte() as i8;
self.pc = self.pc.wrapping_add(offset as u16);
```
`$FB` as `i8` = -5, so `$130A + (-5) = $1305` ✓. This is NOT a bug.

### Finding 4: `MOV [dp]+Y, A` ($D7) looks correct

Opcode $D7 at line ~593 correctly reads the 16-bit pointer from direct page with wrapping, then adds Y. The implementation looks correct — the bug is in the **pointer value** (ZP $14-$15), not in the instruction itself.

### Finding 5: Cycle ratio calculation

`bus.rs` line ~374: `SPC700 cycles = CPU cycles * 1024 / 3580`

This gives ratio ~0.286, which matches the real hardware ratio (1.024 MHz / 3.58 MHz). The fractional accumulator prevents drift. The ratio itself appears correct, but the **batch execution model** is the issue — cycles are accumulated and run in one big burst only on port access.

## Resolution (March 4, 2026 — second session)

All issues identified in the Code Review Findings have been fixed:

### Fix 1: All dp-addressing opcodes — `direct_page()` added (~45 instructions)

Opcode `$AB` (`INC dp`) and ~44 other instructions were missing `direct_page()` in their address calculations. Every SPC700 instruction that uses any direct-page addressing mode now correctly applies the `direct_page()` base offset (0x0000 when P=0, 0x0100 when P=1). The affected addressing modes were:
- Simple dp: `let addr = self.fetch_byte() as u16;` → `self.direct_page() | (fetch_byte() as u16)`
- dp+X: `dp.wrapping_add(x) as u16` → `self.direct_page() | (dp.wrapping_add(x) as u16)`
- [dp+X] indexed indirect: pointer fetch now uses `direct_page()`
- [dp]+Y indirect indexed: pointer fetch now uses `direct_page()`
- dp,dp two-operand: both operands now use `direct_page()`
- dp,#imm: dp operand now uses `direct_page()`

### Fix 2: Sync timing (Option B implemented) — `crates/systems/snes/src/bus.rs`

The `sync_spc700()` method and `spc700_pending_cycles` accumulator have been removed. The SPC700 now runs **inline** in `tick_cycles()` for its proportional share of cycles after each main CPU instruction:

```rust
// In tick_cycles():
if spc700_cycles > 0 {
    if let Some(ref spc700_cell) = self.spc700 {
        spc700_cell.borrow_mut().run_cycles(spc700_cycles as u32);
    }
}
```

This prevents large cycle batches from building up. Between any two consecutive main CPU port writes (e.g., `$2142` low byte and `$2143` high byte), the SPC700 can execute at most 1–2 instructions rather than hundreds. The torn-read race is effectively eliminated.

### Fix 3: Debug infrastructure removed

All temporary diagnostic code has been removed from:
- `crates/core/src/apu/spc700.rs`: trace_ports, last_pc, port_trace_count, file tracing, watchpoints, DIAG snapshots, SPC700-TRACE, jump detection RAM dump, timer eprintln
- `crates/core/src/cpu_spc700.rs`: per-instruction logging for $0100–$0FFF
- `crates/systems/snes/src/bus.rs`: port read/write tracing at $2140–$2143, sync_spc700() calls

### Fix 4: Test updated — `test_port_clear_via_control`

The test was asserting the OLD (wrong) behavior (that control register bits 4–5 clear `apu_out`). Updated to correctly test the hardware behavior: bits 4–5 clear the INPUT latches (`cpuio`), as documented in fullsnes and the S-SMP wiki.

## Summary

The second upload self-corruption bug was caused by a race condition in the SPC700 synchronization model: large batches of SPC700 cycles were flushed on each port access, allowing the SPC700 to execute hundreds of instructions between two consecutive writes to the upload destination-address ports ($2142 and $2143). This produced a torn 16-bit address read at the N-SPC $1325 block header. The fix (Option B) runs the SPC700 incrementally — keeping both CPUs in tight lockstep — so the SPC700 can no longer race ahead between adjacent port writes.
