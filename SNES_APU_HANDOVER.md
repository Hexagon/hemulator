# SNES APU Communication Issue - Handover Document

**Date**: January 5, 2026  
**Status**: Partial Implementation - APU stub allows initial progress but games still hang
**Affects**: All SNES commercial games (Super Mario World tested)

## Problem Statement

SNES games do not produce a visible image. The screen remains completely black with no graphics rendered.

### Root Cause

Commercial SNES games require communication with the SPC700 audio processor (APU) during initialization. Without a functional APU implementation, games get stuck in infinite polling loops waiting for APU responses, preventing them from:
- Completing initialization
- Enabling NMI interrupts
- Unblanking the screen
- Enabling background layers
- Setting up graphics

## Technical Details

### APU Communication Protocol

The SPC700 has a dedicated boot ROM (IPL) that implements a handshake protocol via communication ports $2140-$2143:

1. **Initial Boot Handshake**:
   - CPU clears all ports to $00
   - APU boot ROM responds with $BBAA in ports 0-1 (16-bit value when read)
   - This signals the APU is ready to receive data

2. **Data Upload Protocol**:
   - CPU writes incrementing counter to port 0 (0x01, 0x02, 0x03...)
   - CPU writes data bytes to port 1
   - APU echoes the counter back in port 0 to acknowledge receipt
   - CPU waits for echo before sending next byte

3. **Transfer Completion**:
   - After all data is uploaded, APU returns $BBAA signature
   - This signals data is processed and APU is ready

### Observed Behavior

**Super Mario World** execution trace:

1. **First APU Check (PC $00:8085)**:
   ```
   $8082: CMP $2140    ; Compare A with [$2140]
   $8085: BNE $8082    ; Loop if not equal
   ```
   - Game clears APU ports to $00
   - Waits for $BBAA response
   - ✅ **Fixed**: Stub now returns $BBAA

2. **First Data Upload (PC $00:8085 second occurrence)**:
   ```
   ; CPU writes: 0xCC, then 0x01, 0x02, 0x03... to port 0
   ; Expects APU to echo each value back
   ```
   - ✅ **Fixed**: Stub echoes values in port 0
   - ✅ **Fixed**: After 25 bytes, returns $BBAA completion

3. **Second Data Upload (PC $00:809A)** - **CURRENT HANG POINT**:
   ```
   $809A: CMP $2140    ; Compare A with [$2140]
   $809D: BNE $809A    ; Loop if not equal
   ```
   - CPU state: A=$0018, expecting port 0 != $AA
   - Port 0 contains: $AA (from previous completion signature)
   - ❌ **Issue**: Game expects different value or echo continuation
   - Game stuck in infinite loop here

### APU Port Activity Log

**First handshake** (successful):
```
Write $2140-$2143: 0x00, 0x00, 0x00, 0x00
Response: $2140-$2141 = 0xAA, 0xBB
```

**First upload** (successful):
```
Write $2140: 0xCC → Echo: 0xCC
Write $2140: 0x01 → Echo: 0x01
Write $2140: 0x02 → Echo: 0x02
...
Write $2140: 0x18 → Echo: 0x18
After 25 bytes: Return 0xAA, 0xBB
```

**Second upload** (FAILS):
```
Write $2142: 0x00
Write $2143: 0x05
Write $2141: 0x01
Write $2140: 0xCC → Echo: 0xCC
Write $2140: 0x00
Write $2141: 0x20
Read $2140: 0xAA (stuck - expects different value)
```

## Current Implementation

### Location
`crates/systems/snes/src/bus.rs`

### APU State Fields
```rust
apu_ports: [u8; 4],              // The communication ports
apu_last_written: [u8; 4],       // Tracks writes for protocol detection
apu_response_delay: u32,         // Simulates APU processing time
apu_transfer_counter: u8,        // Counts data bytes in upload session
```

### Protocol Handler Logic

**Pattern 1**: Boot handshake (all ports = $00)
- Returns: ports 0-1 = $AA, $BB
- Resets transfer counter

**Pattern 2**: Data upload (port 0 written with non-zero)
- Increments transfer counter
- If counter < 25: Echo value in port 0
- If counter >= 25: Return $AA, $BB completion signature
- Reset counter to 0

**Pattern 3**: General write
- Store value in port

### Known Limitations

1. **Single-session logic**: Transfer counter resets after completion but doesn't properly handle new upload sessions that start differently

2. **No state machine**: Should track upload state (IDLE, UPLOADING, COMPLETE) to handle multiple rounds

3. **Missing protocol variants**:
   - Some games write to ports in different orders
   - Some games use ports 2-3 for addresses
   - Some games expect different completion signals

4. **No actual code execution**: Real APU would execute uploaded code which might write back to ports

## What Works

✅ Initial boot handshake ($00 → $BBAA)  
✅ First data upload echo (25+ bytes)  
✅ First completion signature ($BBAA)  
✅ Game progresses past first APU check ($8085)  
✅ Second handshake initiated (writes to ports 2-3, 1, 0)  

## What Doesn't Work

❌ Second upload acknowledgment (game expects port 0 != $AA but gets $AA)  
❌ Multi-round upload sessions  
❌ Screen remains blanked  
❌ NMI never enabled  
❌ No visible graphics  

## Attempted Solutions

### 1. Simple Echo (Initial)
- **Result**: Game stuck at first $00 check
- **Issue**: Didn't return $BBAA ready signature

### 2. Boot Signature Only
- **Result**: Game stuck after initial handshake
- **Issue**: Didn't echo data uploads

### 3. Echo + Transfer Counter (Current)
- **Result**: Game progresses through first upload, stuck at second
- **Issue**: Doesn't handle multi-round protocol properly

## Recommended Solutions

### Short-term: Enhanced Stub
Implement a proper state machine:

```rust
enum ApuState {
    Idle,           // Waiting for command
    BootReady,      // Just returned $BBAA
    Uploading,      // Echoing data bytes
    Processing,     // Simulating code execution
    Ready,          // Ready for next command
}
```

Track upload sessions separately and detect when new session starts (write to ports 2-3 often indicates new session).

### Mid-term: Pattern Recognition
Analyze common Nintendo SDK patterns:
- Detect address writes (ports 2-3)
- Detect length writes  (port 1)
- Detect data stream (port 0)
- Return appropriate completion signals per session type

### Long-term: Minimal SPC700 Emulator
Implement basic SPC700 CPU that can:
- Execute uploaded IPL boot ROM
- Run simple uploaded code
- Write to communication ports autonomously
- This is the only true solution for all games

## Testing Methodology

### Quick Test Command
```powershell
cargo run --profile release-quick -- --log-level off --log-bus debug --log-cpu debug "roms\snes\Super Mario World (USA).sfc" 2>&1 | Select-Object -First 100
```

### Detailed APU Protocol Trace
```powershell
cargo run --profile release-quick -- --log-level off --log-bus trace "roms\snes\Super Mario World (USA).sfc" 2>&1 | Select-String -Pattern "APU"
```

### CPU Execution Trace
```powershell
cargo run --profile release-quick -- --log-level off --log-cpu trace "roms\snes\Super Mario World (USA).sfc" 2>&1 | Select-String -Pattern "at .00:80"
```

### Check Where CPU Hangs
```powershell
cargo run --profile release-quick -- --log-level off --log-cpu debug "roms\snes\Super Mario World (USA).sfc" 2>&1 | Select-Object -Skip 10 -First 50
```

## Code Locations

### APU Communication
- **File**: `crates/systems/snes/src/bus.rs`
- **Read handler**: Lines ~419-428 (0x2140-0x2143 read case)
- **Write handler**: Lines ~620-660 (0x2140-0x2143 write case)
- **State fields**: Lines ~87-91
- **Initialization**: Lines ~106-109

### CPU Polling Loops
- **File**: ROM offset, not source code
- **First check**: $00:8082-8085 (CMP $2140 / BNE loop)
- **Second check**: $00:809A-809D (CMP $2140 / BNE loop)

### Related Systems
- **65C816 CPU**: `crates/core/src/cpu_65c816.rs`
- **SNES System**: `crates/systems/snes/src/lib.rs`
- **PPU**: `crates/systems/snes/src/ppu.rs` (functional but no data to render)

## Performance Impact

Current APU stub has minimal performance impact:
- Simple pattern matching on writes
- No complex state updates
- Cycle delay simulation is just a counter decrement

A full SPC700 emulator would add:
- ~30-40% CPU overhead (rough estimate)
- Separate audio processing thread recommended
- DSP simulation for actual audio output

## Related Issues

1. **No audio output**: Without APU, games produce no sound
2. **Timing issues**: Some games may rely on APU timing for synchronization
3. **Save state compatibility**: APU state must be included in save states

## References

### Technical Documentation
- **SPC700 Reference**: [fullsnes.txt - APU section](https://problemkaputt.de/fullsnes.htm#snesapuioports)
- **APU Boot ROM**: Disassembly available in various SNES dev docs
- **Nintendo SDK**: Commercial games use Nintendo's audio upload routines

### Similar Projects
- **bsnes**: Full SPC700 emulation with cycle-accurate timing
- **Snes9x**: Optimized SPC700 with good compatibility
- **no$sns**: Detailed APU protocol documentation

## Next Steps

1. **Immediate**: Analyze second upload sequence in detail
   - What ports are being written and in what order?
   - What's the expected response pattern?
   - Can we detect session boundaries?

2. **Short-term**: Implement state machine APU stub
   - Track upload sessions properly
   - Detect new session starts
   - Handle multi-round protocols

3. **Medium-term**: Consider minimal SPC700
   - Evaluate complexity vs. benefit
   - Research existing SPC700 cores (could we integrate one?)
   - Decide if audio is worth the implementation cost

4. **Alternative**: APU bypass mode
   - Some homebrew games don't use APU
   - Could add "no APU" mode for testing other systems
   - Would never work for commercial games

## Additional Notes

### Why Not Skip APU Entirely?

Some might ask: "Can't we just bypass the APU and let games run without it?"

**Answer: No, for commercial games.**

- All commercial SNES games initialize the APU during boot
- Games will hang waiting for APU acknowledgment (as demonstrated)
- APU is deeply integrated into game initialization sequences
- Even non-audio functionality sometimes depends on APU timing

### Test ROM Approach

A custom test ROM that doesn't use APU would work perfectly for testing other components (PPU, DMA, controllers, etc.). This would be valuable for:
- Verifying PPU rendering works
- Testing sprite systems
- Validating background layers
- Checking scrolling functionality

Creating such a ROM is recommended for development and testing purposes.

## Summary

The SNES emulator is functionally complete in all systems EXCEPT the APU. The current APU stub successfully handles:
- Initial boot handshake
- First data upload session
- Basic echo protocol

But fails on:
- Multi-round upload sessions
- Complex protocol state transitions
- Games that need actual APU code execution

**Impact**: All commercial SNES games remain unplayable due to initialization hangs. A full SPC700 implementation or significantly enhanced stub is required for compatibility.
