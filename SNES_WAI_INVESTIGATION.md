# SNES Emulator - WAI Instruction Investigation

**Date**: January 4, 2026  
**Status**: In Progress - WAI implementation added but not yet verified

## Problem Statement

Super Mario World (and other commercial SNES ROMs) display only a black screen and fail to initialize properly. The test ROM works correctly, proving the rendering pipeline is functional.

### Symptoms
- **Test ROM** (`test.sfc`): ✅ Works perfectly - renders 57344 pixels with blue color (0xFF0000F8)
- **Super Mario World**: ❌ Renders 0 pixels every frame, never sets up graphics
- CPU stuck at PC=$00:8085 in an infinite loop
- Screen is blanked (forced blank mode enabled at startup)
- No VRAM/CGRAM writes occur
- NMI is never enabled

## Root Cause Discovery

Through CPU state logging, discovered:
```
SNES CPU: PC=$00:8085, A=$BBAA, X=$FFFD, Y=$0000, S=$01FA, P=$85, E=false
```

This repeats indefinitely. The game executes its initialization code, then:
1. Writes to $2100 to blank the screen (confirmed via logging)
2. Executes WAI instruction at $00:8085
3. Expected: CPU halts, waits for VBlank NMI, jumps to NMI handler to continue initialization
4. **Actual**: CPU was re-executing WAI infinitely because WAI wasn't properly implemented

## Implementation Changes Made

### 1. WAI (Wait for Interrupt) Instruction - 65C816 CPU

**File**: `crates/core/src/cpu_65c816.rs`

#### Added CPU State Field
```rust
/// WAI (Wait for Interrupt) state - CPU is halted until interrupt occurs
waiting_for_interrupt: bool,
```

#### Modified CPU Step Function
```rust
pub fn step(&mut self) -> u32 {
    let start_cycles = self.cycles;

    // If CPU is waiting for interrupt (WAI instruction), consume cycles without executing
    if self.waiting_for_interrupt {
        self.cycles += 1;
        return (self.cycles - start_cycles) as u32;
    }

    let opcode = self.fetch_byte();
    // ... rest of execution
}
```

#### Updated WAI Instruction (Opcode 0xCB)
```rust
0xCB => {
    // WAI - Wait for Interrupt
    // Halt CPU until an interrupt (IRQ or NMI) occurs
    // PC has already advanced past WAI, so when interrupt occurs, execution continues at next instruction
    self.waiting_for_interrupt = true;
    // Log when entering WAI state (for debugging stuck games)
    eprintln!("65C816: Entering WAI at PC=${:02X}:{:04X}", self.pbr, self.pc.wrapping_sub(1));
    self.cycles += 3;
}
```

#### Modified NMI Trigger
```rust
pub fn trigger_nmi(&mut self) {
    // Avoid nested NMIs
    if self.in_nmi {
        return;
    }

    // WAI instruction is released by any interrupt
    if self.waiting_for_interrupt {
        eprintln!("65C816: NMI triggered, releasing WAI");
    }
    self.waiting_for_interrupt = false;
    
    // ... rest of NMI handling
}
```

#### Reset Handling
Ensured `waiting_for_interrupt` is set to `false` during CPU reset.

### 2. Enhanced PPU Register Logging

**File**: `crates/systems/snes/src/ppu.rs`

Added comprehensive logging for key PPU registers:
- **$2100 (INIDISP)**: Screen Display - logs BLANKED/ENABLED state and brightness
- **$2105 (BGMODE)**: BG Mode - logs when mode is set
- **$2107 (BG1SC)**: BG1 Tilemap Address - logs base address and size
- **$212C (TM)**: Main Screen Layer Enable - logs which layers are enabled (BG1-4, OBJ)

### 3. CPU State Logging

**File**: `crates/systems/snes/src/lib.rs`

Added periodic CPU state logging on first scanline of early frames:
```rust
// Log CPU state on first scanline of first few frames for debugging
if scanline == 0 && self.current_cycles < 10000 {
    log(LogCategory::CPU, LogLevel::Debug, || {
        format!(
            "SNES CPU: PC=${:02X}:{:04X}, A=${:04X}, X=${:04X}, Y=${:04X}, S=${:04X}, P=${:02X}, E={}",
            self.cpu.cpu.pbr, self.cpu.cpu.pc, self.cpu.cpu.c, self.cpu.cpu.x,
            self.cpu.cpu.y, self.cpu.cpu.s, self.cpu.cpu.status, self.cpu.cpu.emulation
        )
    });
}
```

## Current State

### Code Status
- ✅ WAI instruction implementation added to 65C816 CPU
- ✅ `waiting_for_interrupt` flag added to CPU state
- ✅ NMI trigger clears `waiting_for_interrupt` flag
- ✅ CPU step function skips execution when waiting for interrupt
- ✅ Debug logging added to WAI entry and NMI release
- ✅ PPU register logging enhanced
- ✅ Code compiles successfully

### Verification Status
- ❌ **NOT VERIFIED**: WAI eprintln messages not appearing in output
- ❌ **NOT VERIFIED**: Game progression past WAI not confirmed
- ❌ **NOT VERIFIED**: NMI trigger and release not observed

### Test Results
```bash
# Last test run - still showing stuck at $00:8085
cargo run --profile release-quick -- "roms\snes\Super Mario World (USA).sfc" --log-cpu debug

Output:
SNES CPU: PC=$00:8085, A=$BBAA, X=$FFFD, Y=$0000, S=$01FA, P=$85, E=false
SNES CPU: PC=$00:8085, A=$BBAA, X=$FFFD, Y=$0000, S=$01FA, P=$85, E=false
[... repeats infinitely ...]
```

**Expected WAI message not appearing**: `"65C816: Entering WAI at PC=$00:8085"`

## Potential Issues to Investigate

1. **WAI eprintln not executing**: Either:
   - WAI instruction not being reached (PC logging happens before execution)
   - eprintln output being buffered/suppressed
   - Instruction at $00:8085 is not actually 0xCB (WAI)

2. **NMI not triggering**: Game might not be enabling NMI via $4200 register
   - Check if NMI enable bit is set
   - Verify VBlank detection is working
   - Confirm `ppu.take_nmi_pending()` returns true during VBlank

3. **Timing issue**: WAI executed once at startup, but logging condition `self.current_cycles < 10000` prevents seeing subsequent frames

## Next Steps

### Immediate Actions
1. **Verify instruction at $00:8085**: Read ROM byte to confirm it's 0xCB (WAI)
2. **Remove cycle condition from logging**: Change `if scanline == 0 && self.current_cycles < 10000` to log all frames
3. **Check NMI enable register**: Log writes to $4200 to see if game enables NMI
4. **Verify VBlank timing**: Confirm VBlank flag is being set and NMI is triggered

### Debug Commands
```bash
# Check for WAI/NMI messages
cargo run --profile release-quick -- "roms\snes\Super Mario World (USA).sfc" 2>&1 | Select-String "WAI|NMI"

# Full CPU logging
cargo run --profile release-quick -- "roms\snes\Super Mario World (USA).sfc" --log-cpu debug --log-interrupts debug

# PPU register logging
cargo run --profile release-quick -- "roms\snes\Super Mario World (USA).sfc" --log-ppu info
```

### Code to Add
1. **Instruction logging**: Add trace-level logging to show actual opcode being executed
2. **NMI trigger logging**: Add log in `step_frame()` when NMI is triggered
3. **$4200 write logging**: Already exists but verify it's being called

## Background Context

### SNES Initialization Pattern
Typical SNES game initialization:
1. Reset: Jump to reset vector, start in emulation mode
2. Switch to native mode (CLC; XCE)
3. Initialize stack, registers
4. Blank screen (write $80 to $2100)
5. **Execute WAI** - wait for VBlank
6. NMI handler: Set up PPU (VRAM, CGRAM, registers)
7. Enable screen, start game loop

Super Mario World follows this pattern - it reaches step 5 but hangs because WAI wasn't working.

### Files Modified
- `crates/core/src/cpu_65c816.rs` - WAI implementation
- `crates/systems/snes/src/ppu.rs` - Enhanced logging
- `crates/systems/snes/src/lib.rs` - CPU state logging

### Related Documentation
- 65C816 Reference: `docs/references/cpu_65c816.md`
- SNES Architecture: `docs/ARCHITECTURE.md`
- Previous investigation: Conversation history about tm register fix (0x1F)

## Success Criteria

The implementation will be considered successful when:
1. ✅ WAI eprintln message appears: `"65C816: Entering WAI at PC=$00:8085"`
2. ✅ NMI release message appears: `"65C816: NMI triggered, releasing WAI"`
3. ✅ PC advances past $00:8085 to NMI handler
4. ✅ PPU register writes occur (VRAM, CGRAM, layer config)
5. ✅ Screen is enabled (write to $2100 with bit 7 clear)
6. ✅ Pixels are rendered (non-zero pixel count in frame summary)
7. ✅ Super Mario World displays title screen

## Notes

- Test ROM continues to work, confirming rendering pipeline is correct
- The tm register fix (0x1F) is already applied and necessary
- All pre-commit checks pass (fmt, clippy, build, tests)
- This is a critical emulation feature - many SNES games use WAI for synchronization
