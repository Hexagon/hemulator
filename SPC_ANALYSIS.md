# SPC700 / SNES Boot Analysis

**Date**: March 4, 2026  
**Test ROM**: `roms/snes/Super Mario World (USA).sfc`  
**Status**: Second SPC700 upload fails — upload handler overwrites its own code

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

## Next Steps

1. **Investigate the block transition timing**: The first 80-byte block (dest=$8000) finishes, and the SPC700 should read the second header (dest=$8100) from ports 2–3. Trace the exact port values at the $1325 MOVW to see if $1300 comes from reading stale/incorrect port data, or from the `INC $15` path incorrectly wrapping.

2. **Check if the issue is a main CPU timing bug**: The main CPU upload routine may not be writing the second block header fast enough. Add tracing to the main CPU's $2142/$2143 writes to see when the new destination address is written relative to when the SPC700 reads it.

3. **Compare with reference emulator**: Check how ares/bsnes handles the timing of the SPC700 relative to the main CPU during uploads. The `sync_spc700()` call in `bus.rs` runs pending SPC700 cycles before port access, but the ratio or accumulation may be off.

4. **Alternative theory — opcode $D0 (BNE) with wrap**: The `D0 FB` at $1308 branches backward by 5 to $1305. Verify the signed offset is being calculated correctly for negative branch offsets in the SPC700 BNE implementation.

5. **Clean up debug infrastructure** once the root cause is resolved (see list above).
