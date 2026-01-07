---
title: "SPC700 IPL Protocol"
parent: "CPU & Hardware References"
---

# SPC700 IPL ROM Upload Protocol

## Overview
The SPC700 IPL (Initial Program Loader) ROM implements a multi-phase protocol for uploading audio code from the SNES main CPU to the SPC700 audio processor.

## IPL ROM Memory Map
- **$FFC0-$FFFF**: 64-byte IPL ROM (read-only when control bit 7 is set)
- **Control Register $F1**: Bit 7 enables/disables IPL ROM overlay

## Protocol Phases

### Phase 1: Initialization ($FFC0-$FFC8)
```asm
$FFC0: CD EF        MOV X, #$EF       ; Set X = $EF
$FFC2: BD           MOV SP, X         ; Set stack pointer to $EF
$FFC3: E8 00        MOV A, #$00       ; A = 0
$FFC5: C6           MOV (X), A        ; Clear memory from $EF down
$FFC6: 1D           DEC X             ; Decrement X
$FFC7: D0 FC        BNE $FFC5         ; Loop until X = 0 (clears $00-$EF)
```

**Actions:**
- Sets up stack at $EF
- Clears memory $00-$EF (including ports $F4-$F7)
- Takes ~2048 cycles

### Phase 2: Ready Signature ($FFC9-$FFCE)
```asm
$FFC9: 8F AA F4     MOV $F4, #$AA     ; Write $AA to port $F4
$FFCC: 8F BB F5     MOV $F5, #$BB     ; Write $BB to port $F5
```

**Actions:**
- Writes $BBAA signature to ports $F4/$F5 (appears as $AA/$BB at SNES $2140/$2141)
- Signals to main CPU: "SPC700 ready for commands"

### Phase 3: Command Wait ($FFCF-$FFD2)
```asm
$FFCF: 78 CC F4     CMP $F4, #$CC     ; Wait for $CC in port $F4
$FFD2: D0 FB        BNE $FFCF         ; Loop until $CC received
```

**Actions:**
- Waits for main CPU to write $CC to port $2140
- This signals: "Main CPU ready to upload code"

### Phase 4: Branch to Entry Point Setup ($FFD4)
```asm
$FFD4: 2F 19        BRA $FFEF         ; Branch to $FFEF (entry point setup)
```

**Actions:**
- Skips the upload loop at $FFD6 (direct path)
- Goes to $FFEF to read entry point address

### Phase 5: Upload Loop at $FFD6 (ALTERNATE PATH)
The upload loop at $FFD6 is an **alternate entry point** for uploading data blocks.
It's not reached via the normal flow from $FFC0, but can be entered by:
- Jumping directly to $FFD6 after initial setup
- Branching back from $FFE9/$FFED during multi-block uploads

```asm
; Wait for non-zero index in Y
$FFD6: EB F4        MOV Y, $F4        ; Y = port $F4 (index)
$FFD8: D0 FC        BNE $FFD6         ; Loop while Y = 0

; Inner loop: receive bytes
$FFDA: 7E F4        CMP Y, $F4        ; Compare Y with port $F4
$FFDC: D0 0B        BNE $FFE9         ; If not equal, go to $FFE9
$FFDE: E4 F5        MOV A, $F5        ; Read data byte from port $F5
$FFE0: CB F4        MOV $F4, Y        ; Echo index to port $F4
$FFE2: D7 00        MOV ($00)+Y, A    ; Store byte at (ZP+Y)
$FFE4: FC           INC Y             ; Increment Y
$FFE5: D0 F3        BNE $FFDA         ; Loop if Y != 0 (256 bytes max)

$FFE7: AB 01        INC $01           ; Increment high byte of address
$FFE9: 10 EF        BPL $FFDA         ; Continue if bit 7 clear

; End of block
$FFEB: 7E F4        CMP Y, $F4        ; Final check
$FFED: 10 EB        BPL $FFDA         ; Continue if positive
```

**Upload Protocol:**
1. **Wait for index**: Y = port $F4, loop while Y = 0
2. **For each byte**: 
   - Wait for port $F4 to match Y (synchronization)
   - Read data byte from port $F5
   - Echo index Y to port $F4 (acknowledgment)
   - Store byte at (ZP+Y) where ZP = $0000-$0001
   - Increment Y
3. **Block continuation**: When Y wraps to 0, increment high byte ($01)
4. **Block termination**: When high byte bit 7 is set, fall through to $FFEF

### Phase 6: Entry Point Setup and Execution ($FFEF-$FFFF)
```asm
$FFEF: BA F6        MOVW YA, $F6      ; Read 16-bit entry point from ports $F6/$F7
$FFF1: DA 00        MOVW $00, YA      ; Store entry point at $0000-$0001
$FFF3: BA F4        MOVW YA, $F4      ; Read 16-bit mode value from ports $F4/$F5
$FFF5: C4 F4        MOV $F4, A        ; Echo low byte to port $F4
$FFF7: DD           MOV A, Y          ; A = high byte of mode
$FFF8: 5D           MOV X, A          ; X = high byte of mode
$FFF9: D0 DB        BNE $FFD6         ; If X != 0, branch to upload loop
$FFFB: 1F 00 00     JMP [$0000+X]     ; Jump indirect to address at $0000+X
```

**Actions:**
- Reads 16-bit entry point from ports $F6/$F7 (A=low, Y=high)
- Stores entry point at zero page $0000-$0001
- Reads 16-bit mode value from ports $F4/$F5
- Echoes low byte to port $F4 (acknowledgment)
- Uses high byte of mode to determine action:
  - If high byte != 0: Branch to upload loop at $FFD6
  - If high byte == 0: Execute JMP [$0000+X] to jump to entry point

**Key Insight:**
The JMP [$0000+X] instruction reads the 16-bit address stored at $0000+X and jumps to it.
Since X=0 (because we only execute this when high byte of mode is 0), it jumps to the
address at $0000-$0001, which is the entry point we just stored!

## Main CPU Upload Procedure

### Method 1: Direct Execution (No Upload)
For games that have pre-loaded audio code or don't need to upload:

```
1. Wait for port $2140 = $AA
2. Wait for port $2141 = $BB
3. Write $CC to port $2140 (trigger ready state)
4. Write entry point low byte to port $2142 ($F6)
5. Write entry point high byte to port $2143 ($F7)
6. Write $00 to port $2140 ($F4) - mode low byte
7. Write $00 to port $2141 ($F5) - mode high byte = 0 means execute
8. SPC700 echoes $00 to port $2140
9. SPC700 jumps to entry point
```

### Method 2: Upload with IPL ROM Upload Loop
For games that need to upload audio driver code:

**Step 1: Initial Setup**
```
1. Wait for port $2140 = $AA
2. Wait for port $2141 = $BB
3. Write $CC to port $2140 (trigger ready state)
```

**Step 2: Enter Upload Mode**
```
4. Write entry point low byte to port $2142 ($F6) - typically $D6
5. Write entry point high byte to port $2143 ($F7) - typically $FF
6. Write $00 to port $2140 ($F4) - mode low byte
7. Write $01 to port $2141 ($F5) - mode high byte = non-zero triggers upload
8. SPC700 branches to upload loop at $FFD6
```

**Step 3: Upload Data Blocks**
Set destination address in ports (SPC700 will store at $0000-$0001):
```
For each block:
  For each byte:
    - Write index to port $2140 ($F4)
    - Write data byte to port $2141 ($F5)
    - Wait for port $2140 to echo index back
  When 256 bytes sent (index wraps to 0):
    - High byte at $0001 increments automatically
  To end upload:
    - Ensure high byte ($0001) >= $80 (bit 7 set)
```

**Step 4: Execute Uploaded Code**
```
9. Write final entry point low byte to port $2142 ($F6)
10. Write final entry point high byte to port $2143 ($F7)
11. Write $00 to port $2140 ($F4)
12. Write $00 to port $2141 ($F5) - mode = 0 to execute
13. SPC700 jumps to uploaded code at final entry point
```

## Common Usage Patterns

### Pattern 1: Simple Upload (Skip $FFD6)
Most games use a simplified protocol:
1. Wait for $BBAA
2. Send $CC
3. Send entry point in $F6/$F7
4. SPC700 jumps directly to entry point

This is the **direct path**: $FFC0 → $FFCF → $FFD4 → $FFEF → RET

### Pattern 2: Block Upload (Use $FFD6)
Some games upload large audio drivers:
1. Wait for $BBAA
2. Send $CC
3. Set entry point to $FFD6 (upload loop)
4. Upload data blocks using index/data protocol
5. When done, set entry point to start of uploaded code

## Implementation Notes

### Port Behavior
- **SNES writes to $2140-$2143** → SPC700 reads from $F4-$F7
- **SPC700 writes to $F4-$F7** → SNES reads from $2140-$2143
- Ports are bidirectional but have separate read/write paths

### Timing
- IPL ROM initialization: ~2048 cycles
- Each byte upload: ~20-30 cycles (including echo wait)
- Total for 64KB upload: ~1-2 million cycles (~1-2ms at 1.024MHz)

### Synchronization
The echo protocol ensures data integrity:
- Main CPU writes index → waits for echo
- SPC700 reads index → writes echo → reads data
- Prevents race conditions and lost data

## References
- [SPC700/IPL ROM - SnesLab](https://sneslab.net/wiki/SPC700/IPL_ROM)
- [Anomie's SPC700 Documentation](https://github.com/gilligan/snesdev/blob/master/docs/spc700.txt)
- [Super NES Programming/Loading SPC700 programs](https://en.wikibooks.org/wiki/Super_NES_Programming/Loading_SPC700_programs)
