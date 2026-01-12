# SNES SMW Boot Investigation (APU Handshake + Blank Video)

**Date**: January 11–12, 2026  
**ROM**: Super Mario World (USA).sfc  
**Goal**: Get SMW to reach visible video output (unblank + render) in the SNES core.

This started as a hang at PC=$8082 waiting for the APU `$BBAA` ready signature, and is now tracking the next blocker: SMW progresses further, but keeps the screen force-blanked and never enables NMI.

---

## Current Status (Latest)

### What’s fixed / improved

- **APU handshake at $8082 is unblocked** when using the stub APU protocol.
  - Root cause was incorrect port semantics: S-CPU writes were incorrectly visible on S-CPU reads.
  - Fix was to split port latches into **CPU→APU input ports** vs **APU→CPU output ports** and only update output ports when the stub is explicitly “responding”.

### What’s still broken

- **Display remains blank**:
  - `$2100 (INIDISP)` remains `0x80` (force blank, brightness 0).
  - No observed writes of `$2100` to a non-blank value in long runs.
- **NMI remains disabled**:
  - `$4200 (NMITIMEN)` writes observed so far are `0x00` only.
  - No observed writes enabling NMI (bit 7) within at least ~25M cycles.
- SMW *does* run code (PC advances into $B8xx/$B9xx area), so we’re not “hard hung”, but it appears to be stuck in a longer initialization/decompression path or gated on some missing hardware behavior.

---

## Evidence / Artifacts

### Key logs

- Headless run @ ~2M cycles with instrumentation:
  - `smw_2m_instrumented.log`
  - `smw_2m_instrumented_dump.txt`
  - Notable observations:
    - `$4200` writes only `0x00` (e.g. PC=$008001 and PC=$00937D)
    - `$2100` written as `0x80` (e.g. PC=$008018 and PC=$009385)
    - At PC=$00B916: reads `$4210` (returns 0x02; NMI flag not set) and `$4212` (returns 0x80; VBlank)

- Longer headless run @ ~25M cycles:
  - `smw_25m.log`
  - `smw_25m_dump.txt`
  - Still only `$4200=0x00`, `$2100=0x80`.

### Screenshots

- Dumps include a screenshot path (e.g. `screenshots\snes\...png`). These remain blank/backdrop-only as expected given INIDISP state.

---

## Original Problem: Hang at $8082 Waiting for `$BBAA`

SMW waits for the SPC700 IPL ready signature via a 16-bit read of `$2140/$2141`.

```asm
8079  08            PHP
807A  C2 30         REP #$30      ; 16-bit A/X/Y
807C  A0 00 00      LDY #$0000
807F  A9 AA BB      LDA #$BBAA
8082  CD 40 21      CMP $2140     ; Compare with APU port 0/1 (16-bit read)
8085  D0 FB         BNE $8082     ; Loop until $2140/$2141 = $BBAA
```

### Root cause

The stub APU originally behaved like a single shared port array, so CPU writes could corrupt what the CPU later read back, breaking the `$BBAA` check (and/or later handshake steps).

### Fix implemented

- Stub APU ports were split into:
  - `apu_in_ports`: S-CPU writes (what the SPC700 would read)
  - `apu_out_ports`: S-CPU reads (what the SPC700 would write)
- Acknowledgements/signatures are now written only to `apu_out_ports`.

---

## Investigation Focus: Why SMW Never Unblanks / Enables NMI

We now have hard evidence that SMW is executing and is reading VBlank/NMI-related registers, but:

- It does not appear to write `$4200` with bit7 set.
- It does not appear to write `$2100` with force-blank cleared and brightness > 0.

This suggests at least one of:

1. **SMW hasn’t reached the part of init that enables NMI/unblanks** (still doing long work).
2. **SMW is gated on missing/incorrect I/O register behavior** (it may be probing hardware and taking a fallback path).
3. **Addressing/mapping bug causes wrong code/data** during the later init path.

---

## Changes Made During This Investigation (for reference)

### Targeted register/PC instrumentation

To identify what code is interacting with what registers:

- The bus now tracks the most recent CPU instruction address and logs it on key reads/writes.

### Implemented additional CPU I/O + open-bus behavior

Some commercial init code probes/uses CPU math/timer registers.

- Added basic read/write behavior for:
  - `$4201` WRIO / `$4213` RDIO
  - `$4202/$4203` multiply → `$4216/$4217`
  - `$4204-$4206` divide → `$4214-$4217`
  - `$4207-$420A` H/V timer latches (IRQ not implemented)
  - `$420D` MEMSEL (FastROM; stored for readback)
- Unhandled $2000-$5FFF reads now return a best-effort **open-bus** value instead of hard `0`.

---

## Next Steps

1. **Check why `$4200` remains 0**
   - Identify the code path that *should* enable NMI in SMW and confirm we ever reach it.
2. **Reduce “unknown/??? opcode” noise in disassembly**
   - Confirm the disassembler is using correct addressing/mapping for the current PC region.
3. **Implement remaining frequently-probed registers (as needed)**
   - If SMW reads specific registers we still return as open-bus, implement minimal correct semantics.
4. **Consider swapping to real SPC700 path** (after basic sync correctness)
   - Once the stub gets SMW further, compare behavior with the real SPC700 implementation.

---

## Update 2026-01-12: Disassembler Fix

### Fixed "unknown opcode" issue

Added all missing long addressing mode opcodes to the 65C816 disassembler:
- **Opcodes**: 0x0F, 0x1F, 0x2F, 0x3F, 0x4F, 0x5F, 0x6F, 0x7F, 0x8F, 0x9F, 0xAF, 0xBF, 0xCF, 0xDF, 0xEF, 0xFF
- **Instructions**: ORA/AND/EOR/ADC/SBC/CMP/LDA/STA with absolute,long and absolute,long,X addressing
- **Impact**: These use 24-bit addresses and are critical for SNES code accessing ROM across banks
- **Result**: Should eliminate "??? opcode" noise in disassembly and instruction traces

This fixes step #2 from the "Next Steps" section above.

### Remaining work

The main blocker is still that SMW never writes $4200 with NMI enabled. Possible causes:
1. **Stuck in decompression loop**: SMW might be decompressing graphics/data and taking longer than expected
2. **Waiting for hardware state**: Missing or incorrect register behavior could gate progression
3. **CPU execution issue**: Despite the disassembler fix, there could be CPU opcode implementation bugs
4. **DMA timing**: Incorrect DMA cycle counting or behavior could affect init timing

**Next diagnostic step**: Use instruction tracing (already integrated in SnesSystem) to capture execution flow around the $B8xx/$B9xx area and identify:
- Are we in a loop? (repeated PC values)
- What code comes after the current execution point?
- Are there any branches that should lead to NMI enable but don't get taken?
