# SPC700 IPL ROM Upload Protocol

## Overview
The SPC700 IPL (Initial Program Loader) ROM implements a multi-phase protocol for uploading audio code from the SNES main CPU to the SPC700 audio processor.

## IPL ROM Memory Map
- **$FFC0-$FFFF**: 64-byte IPL ROM (read-only when control bit 7 is set)
- **Control Register $F1**: Bit 7 enables/disables IPL ROM overlay

## Protocol Phases

### Phase 1: Initialization ($FFC0-$FFC8)
```assembly
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
```assembly
$FFC9: 8F AA F4     MOV $F4, #$AA     ; Write $AA to port $F4
$FFCC: 8F BB F5     MOV $F5, #$BB     ; Write $BB to port $F5
```

**Actions:**
- Writes $BBAA signature to ports $F4/$F5 (appears as $AA/$BB at SNES $2140/$2141)
- Signals to main CPU: "SPC700 ready for commands"

### Phase 3: Command Wait ($FFCF-$FFD2)
```assembly
$FFCF: 78 CC F4     CMP $F4, #$CC     ; Wait for $CC in port $F4
$FFD2: D0 FB        BNE $FFCF         ; Loop until $CC received
```

**Actions:**
- Waits for main CPU to write $CC to port $2140
- This signals: "Main CPU ready to upload code"

### Phase 4: Branch to Entry Point Setup ($FFD4)
```assembly
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

```assembly
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

### Phase 6: Entry Point Setup ($FFEF-$FFFE)
```assembly
$FFEF: E4 F6        MOV A, $F6        ; Read low byte of entry point
$FFF1: C4 F4        MOV $F4, A        ; Echo to port $F4
$FFF3: E4 F7        MOV A, $F7        ; Read high byte of entry point
$FFF5: C4 F5        MOV $F5, A        ; Echo to port $F5
$FFF7: E4 F6        MOV A, $F6        ; Read entry point low byte again
$FFF9: C4 00        MOV $00, A        ; Store as low byte of return address
$FFFB: E4 F7        MOV A, $F7        ; Read entry point high byte again
$FFFD: C4 $01       MOV $01, A        ; Store as high byte of return address
$FFFF: 6F           RET               ; Jump to address in $0000-$0001
```

**Actions:**
- Reads 16-bit entry point from ports $F6/$F7
- Echoes entry point to ports $F4/$F5 (acknowledgment)
- Stores entry point at $0000-$0001
- RET pops stack (which has junk) and jumps to entry point

## Main CPU Upload Procedure

### Step 1: Wait for Ready Signature
```
1. Reset SPC700 (enable IPL ROM)
2. Wait for port $2140 = $AA
3. Wait for port $2141 = $BB
```

### Step 2: Send Upload Command
```
4. Write $CC to port $2140
5. Write $00 to ports $2141-$2143 (optional, for protocol init)
```

### Step 3: Upload Data Blocks
For multi-block uploads, games may jump to $FFD6 by:
- Setting entry point to $FFD6 initially
- Using the upload loop to upload code that loops back

```
6. Set destination address in ZP ($0000-$0001)
7. For each byte:
   - Write index to port $2140
   - Write data to port $2141
   - Wait for port $2140 to echo index (acknowledgment)
```

### Step 4: Execute Uploaded Code
```
8. Write entry point low byte to port $2142
9. Write entry point high byte to port $2143
10. SPC700 reads from $F6/$F7 and jumps to entry point
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
