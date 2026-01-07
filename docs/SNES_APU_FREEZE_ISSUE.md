# SNES APU Freeze Issue - Debugging Handover

**Date**: January 7, 2026  
**Status**: Unresolved - Intermittent freeze  
**Affected System**: SNES (Super Nintendo Entertainment System)  
**Test ROM**: Super Mario World (USA).sfc

---

## Executive Summary

The SNES emulator intermittently freezes with a black screen. The main 65C816 CPU gets stuck in an infinite loop at **$8085** waiting for the SPC700 APU to respond with the `$AA/$BB` boot signature. The issue appears to be a **timing/synchronization problem** between the main CPU and the SPC700 APU that manifests after extended runtime.

---

## Symptoms

1. **Black screen** - PPU register $2100 has bit 7 set (force blank mode)
2. **CPU stuck at $8085** in loop:
   ```asm
   8082  CD 40 21      CMP $2140    ; Compare A with APU port 0
   8085  D0 FB         BNE $8082    ; Loop until match
   ```
3. **CPU waiting for $BBAA signature** - Register A contains `$BBAA`, expecting APU to echo this
4. **SPC700 not responding** - APU output ports read as `$00 $00 $00 $00`
5. **Issue is intermittent** - Works for a while, then freezes (especially on second upload session)

---

## What We Investigated

### 1. SPC700 Cycle Ratio
- **Finding**: SPC700 was severely under-clocked (only 556 cycles after 1M main CPU cycles)
- **Expected**: ~312,500 SPC700 cycles (1M ÷ 3.2 ratio)
- **Status**: ✅ Fixed - SPC700 now runs at correct ratio

### 2. Timer Implementation
- **Finding**: SPC700 timers were not being ticked at all
- **Timer 0/1**: 8 kHz (128 cycle prescaler)
- **Timer 2**: 64 kHz (16 cycle prescaler)
- **Status**: ✅ Fixed - Timers now tick correctly with proper prescaler/divisor/output counter

### 3. Port Communication Direction
- **Architecture**: 
  - Main CPU writes to `$2140-$2143` → SPC700 reads from `$F4-$F7` (CPUIO)
  - SPC700 writes to `$F4-$F7` → Main CPU reads from `$2140-$2143` (apu_out)
- **Status**: ✅ Correct - Two separate port arrays implemented

### 4. IPL ROM Boot Sequence
- **Finding**: SPC700 needs ~5000 cycles to complete boot and write `$AA/$BB`
- **Status**: ✅ Fixed - Added pre-run of 6000 cycles at reset

### 5. Port Latching (REMOVED)
- **Finding**: Incorrect latching mechanism was added during debugging
- **Status**: ✅ Removed - Real SNES has no latching for APU ports

---

## Current State of the Code

### Files Modified
- `crates/core/src/apu/spc700.rs` - Timer implementation, port handling
- `crates/systems/snes/src/bus.rs` - APU port reads/writes
- `crates/systems/snes/src/lib.rs` - SPC700 pre-run at reset

### Key Code Paths

**Main CPU reading APU ports** (`bus.rs`):
```rust
0x2140..=0x2143 => {
    let port = (offset - 0x2140) as u8;
    if let Some(ref spc700) = self.spc700 {
        spc700.read_port(port)  // Returns apu_out[port]
    } else {
        self.apu_ports[port as usize]
    }
}
```

**SPC700 writing to output ports** (`spc700.rs`):
```rust
0xF4..=0xF7 => {
    let port = (addr - 0xF4) as usize;
    self.apu_out[port] = val;  // Main CPU will read this
}
```

**Timer ticking** (`spc700.rs`):
```rust
fn tick_timers(&mut self, cycles: u32) {
    // Prescaler -> Internal counter -> Output counter (4-bit)
    // Timer 0/1: 128 cycle prescaler (8 kHz)
    // Timer 2: 16 cycle prescaler (64 kHz)
}
```

---

## The Remaining Problem

### Observed Behavior
1. **First upload session**: Works correctly
   - Main CPU sees `$AA/$BB` signature
   - Upload proceeds with byte counter incrementing: `$00, $01, $02...`
   - Upload completes successfully

2. **After first upload**: 
   - Uploaded code executes at `$054A`
   - Code enters timer wait loop (reading `$FD` for timer 0 counter)
   - Eventually, main CPU expects another upload session
   - Main CPU waits for `$AA/$BB` again but SPC700 never writes it

### Theories

#### Theory A: Uploaded Code Never Returns to IPL
The uploaded driver code is supposed to either:
1. Write `$AA/$BB` to signal ready for next session, OR
2. Jump back to IPL ROM to restart boot sequence

If the uploaded code is stuck in its own loop (e.g., timer wait), it will never write the signature.

#### Theory B: Timer Counter Not Incrementing Fast Enough
The uploaded code at `$054A-$054D` reads timer counter in a tight loop:
```asm
054A  EC xx FD      MOV Y, $FDxx  ; Read timer 0 counter
054D  F0 FB         BEQ $054A     ; Loop if zero
```

If the timer isn't incrementing (divisor set to high value, or timer disabled), the loop never exits.

#### Theory C: Cycle Synchronization Drift
Over time, the ratio between main CPU cycles and SPC700 cycles may drift. If SPC700 falls behind, it can't respond fast enough.

#### Theory D: Multiple SPC700 Instances or State Reset
There might be a situation where the SPC700 state is reset or a different instance is used, losing the apu_out values.

---

## Debugging Suggestions

### 1. Add Persistent Logging
Log every APU output port write with timestamp/cycle count to a file:
```rust
log(LogCategory::APU, LogLevel::Info, || {
    format!("SPC700 @ cycle {}: Write port {} = ${:02X}", 
        total_cycles, port, val)
});
```

### 2. Trace Uploaded Code Behavior
The uploaded code starting at `$054A` needs to be analyzed:
- What does it expect from timers?
- When does it write to output ports?
- Does it ever return to IPL or restart boot sequence?

### 3. Compare with Reference Emulator
Run the same ROM in bsnes/higan with logging enabled and compare:
- APU port values at each frame
- When `$AA/$BB` gets written
- Timer behavior

### 4. Add Watchpoint on apu_out
Break/log whenever `apu_out[0]` or `apu_out[1]` changes to track all signature writes.

### 5. Check Control Register
The control register `$F1` enables timers (bits 0-2) and clears counters (bits 4-5). Log writes to this register.

---

## Test Cases Added

Located in `crates/core/src/apu/spc700.rs`:
- `test_timer_output_counter_increments` - Verifies timer counter works
- `test_timer_counter_clear_on_read` - Verifies 4-bit counter clears on read
- `test_timer_prescaler_accuracy` - Verifies 128/16 cycle prescaler
- `test_ipl_boot_writes_signature` - Verifies IPL ROM writes `$AA/$BB`

---

## Key References

- **SPC700 Reference**: `docs/references/cpu_spc700.md`
- **IPL Protocol**: `docs/references/spc700_ipl_protocol.md`
- **SNES Dev Wiki**: https://snes.nesdev.org/wiki/S-SMP

---

## Quick Reproduction

```powershell
# Run with APU logging to see the freeze
cargo run --profile release-quick -- --log-apu debug "roms/snes/Super Mario World (USA).sfc"

# The freeze manifests as:
# 1. Game starts (you may see brief visuals)
# 2. Screen goes black
# 3. Logs show repeated "Main CPU reads port X = $00"
```

---

## Files to Focus On

1. **`crates/core/src/apu/spc700.rs`** - SPC700 CPU, memory, timers, ports
2. **`crates/systems/snes/src/bus.rs`** - Main CPU bus, APU port mapping
3. **`crates/systems/snes/src/lib.rs`** - System integration, SPC700 cycling

---

## Contact

This document was created during a debugging session. The issue remains open and requires further investigation into the interaction between the uploaded APU driver code and the timer/port communication system.
