# SNES Hang at $8082 - Investigation Summary

**Date**: January 11, 2026  
**ROM**: Super Mario World (USA).sfc  
**Symptom**: Emulator hangs at PC=$8082 during initialization

---

## Problem Description

The SNES emulator gets stuck in an infinite loop at address $8082, waiting for the SPC700 audio processor to output `$BBAA` (ready signature) in ports $2140/$2141. The SPC700 driver outputs `$00` to all ports instead.

## Code at Hang Location

```asm
8079  08            PHP
807A  C2 30         REP #$30      ; 16-bit A/X/Y
807C  A0 00 00      LDY #$0000
807F  A9 AA BB      LDA #$BBAA
8082  CD 40 21      CMP $2140     ; Compare with APU port 0/1 (16-bit read)
8085  D0 FB         BNE $8082     ; Loop until $2140/$2141 = $BBAA ← STUCK HERE
8087  E2 20         SEP #$20      ; Continue upload...
```

---

## Root Cause Analysis

### Initialization Flow

1. **$8052**: `JSR $80E8` - First upload routine call
2. **$80E8**: Sets up upload parameters (dest=$8000, block=$0E)
3. **$80F8**: `JSR $8079` - Calls the actual upload routine
4. **$8079-$80D9**: Upload routine that:
   - Waits for `$BBAA` in ports (IPL ready signal) ✅ Works first time
   - Sends `$CC` to initiate transfer
   - Uploads bytes one by one with acknowledgment
   - After upload completes, IPL ROM jumps to uploaded code at $0500

### What Works

- SPC700 IPL ROM boots correctly and outputs `$AA/$BB` initially
- First upload handshake succeeds (IPL provides `$BBAA`)
- Data transfer completes (12,000+ bytes uploaded)
- SPC700 jumps from IPL ROM ($FFFB) to driver at $0500
- Driver initializes and enters main loop

### What Fails

After the driver starts running:
1. Main CPU clears all APU ports ($2140-$2143 = $00)
2. Main CPU calls `JSR $80FD` for **second upload** (block $0F)
3. Second upload routine waits for `$BBAA` at $8082
4. **SPC700 driver outputs `$00` to all ports** - never provides `$BBAA`
5. Main CPU loops forever waiting

---

## SPC700 Driver Behavior

### After Jump to $0500

The uploaded SMW driver:
- Initializes memory at $0386-$0389
- Enters main processing loop at $055E, $0566, $056E, $0586
- Calls subroutine at $05A5 which writes `$00` to all output ports ($F4-$F7)
- Loops continuously, never outputs `$AA/$BB`

### Trace Evidence

```
SPC700: Write port $F4 = $00 (apu_out now: $00 $00 $00 $00)
SPC700: Write port $F5 = $00 (apu_out now: $00 $00 $00 $00)
SPC700: Write port $F6 = $00 (apu_out now: $00 $00 $00 $00)
SPC700: Write port $F7 = $00 (apu_out now: $00 $00 $00 $00)
[repeats indefinitely]
```

---

## Key Question

**Does SMW actually need a second upload, or is the code path wrong?**

Looking at the init sequence:
```asm
8052  20 E8 80      JSR $80E8   ; Upload block $0E (driver code)
8055  9C 00 01      STZ $0100
8058  9C 09 01      STZ $0109
805B  20 4E 8A      JSR $8A4E   ; Audio engine init (never reached!)
805E  20 FD 80      JSR $80FD   ; Upload block $0F (never reached!)
```

We never reach $805B because we're stuck in the **first** upload at $8082.

Wait - this contradicts earlier findings. Let me re-examine:

The trace showed:
- $8087 WAS executed (loop exited once)
- But we end up stuck at $8082 again

This means:
1. First wait for `$BBAA` succeeds (IPL provides it)
2. Upload proceeds
3. After upload, **the routine waits for `$BBAA` again** (end-of-transfer acknowledgment?)
4. But the driver is now running and doesn't provide `$BBAA`

---

## Possible Solutions

### Option 1: Driver Should Re-enter IPL Mode

The SMW driver could have code to:
- Monitor port 0 for upload request (e.g., `$FF` in port 1)
- Re-enable IPL ROM (set bit 7 of $F1 control register)
- Allow IPL ROM to handle subsequent uploads

### Option 2: Driver Should Provide Ready Signal

After initialization, the driver should output `$AA/$BB` to indicate readiness for commands. The main CPU could then proceed without expecting an upload.

### Option 3: Main CPU Protocol Issue

The main CPU's upload routine may need modification:
- Check if already uploaded and use different communication protocol
- Skip second upload if driver is already running

### Option 4: Timing Issue

The SPC700 may need more cycles to initialize before the main CPU reads ports. The driver might eventually output `$AA/$BB` but the main CPU reads too early.

---

## Technical Details

### Port Communication Model

| Port | Main CPU Writes | SPC700 Reads | Main CPU Reads | SPC700 Writes |
|------|-----------------|--------------|----------------|---------------|
| $2140/$F4 | → cpuio[0] | cpuio[0] | apu_out[0] ← | apu_out[0] |
| $2141/$F5 | → cpuio[1] | cpuio[1] | apu_out[1] ← | apu_out[1] |
| $2142/$F6 | → cpuio[2] | cpuio[2] | apu_out[2] ← | apu_out[2] |
| $2143/$F7 | → cpuio[3] | cpuio[3] | apu_out[3] ← | apu_out[3] |

### IPL Boot Protocol

1. IPL ROM clears memory, writes `$AA/$BB` to ports
2. Waits for `$CC` in port 0 from main CPU
3. Reads destination address from ports 2-3
4. Uploads bytes, echoing index to port 0 as acknowledgment
5. When index wraps or non-contiguous, jumps to uploaded code

### SMW Uploaded Driver Start

```
$0500: 20 CD CF BD E8 00 C5 86 03 C5 87 03 C5 88 03 C5
       CLRP; MOV X,#$CF; MOV SP,X; MOV A,#$00; MOV !$0386,A; ...
```

---

## Files Involved

- `crates/core/src/apu/spc700.rs` - SPC700 implementation with IPL ROM
- `crates/core/src/cpu_spc700.rs` - SPC700 CPU opcodes
- `crates/systems/snes/src/bus.rs` - SNES bus, APU port mapping ($2140-$2143)

---

## Next Steps

1. **Compare with working emulator**: Run SMW in bsnes/higan and trace the port communication to see expected behavior
2. **Analyze SMW driver**: Disassemble the uploaded code at $0500 to find if/where it should respond with `$AA/$BB`
3. **Check timing**: Verify if SPC700 is running enough cycles relative to main CPU
4. **Test with other ROMs**: See if the issue is SMW-specific or affects all SNES games

---

## Debug Commands Used

```powershell
# Debug dump at specific cycle
cargo run --profile release-quick -- --debug-dump-cycles 800000 "roms/snes/Super Mario World (USA).sfc"

# Trace APU port activity
cargo run --profile release-quick -- --log-apu info --debug-dump-cycles 800000 "roms/snes/Super Mario World (USA).sfc" 2>&1 | Select-String "Write port|Jumped out"

# Trace bus activity
cargo run --profile release-quick -- --log-bus trace --debug-dump-cycles 800000 "roms/snes/Super Mario World (USA).sfc" 2>&1 | Select-String "reads APU port"

# CPU trace
cargo run --profile release-quick -- --log-cpu trace --debug-dump-cycles 200000 "roms/snes/Super Mario World (USA).sfc" 2>&1 | Select-String "8082"
```
