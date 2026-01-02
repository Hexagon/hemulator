# N64 Emulator Development Status

**Last Updated**: January 2, 2026  
**Status**: Blue screen displays, game stuck in initialization loop  
**ROM Tested**: Super Mario 64 (8MB)

## Current State

### ✅ Working Components

- **ROM Loading**: Successfully loads 8MB commercial ROMs
- **IPL3 Boot**: Executes boot sequence, jumps to entry point (0x80246000)
- **CPU Execution**: MIPS R4300i core executes instructions correctly
- **GPR Initialization**: Stack pointer, return address, and other registers properly set
- **VI System**: Video Interface generates interrupts at scanline 256 every frame
- **Framebuffer**: Dark blue (0xFF000040) background visible on screen
- **Rendering Loop**: Confirmed executing via frame counter logs (60 fps)

### ❌ Not Working

- **RSP Activity**: No DMA operations, no microcode loading detected
- **RDP Activity**: No graphics commands being processed
- **PIF Access**: No controller polling or communication
- **VI Configuration**: Game never writes to VI registers
- **Game Progression**: Stuck in tight polling loop reading VI_LEAP register

## Session Changes

### 1. Framebuffer Visibility Fix
**File**: `crates/systems/n64/src/rdp.rs` (lines 185-240)  
**File**: `crates/systems/n64/src/vi.rs`

**Problem**: Framebuffer initialized to transparent black (0x00000000), invisible on screen.

**Fix**: Changed initialization to dark blue (0xFF000040) with full alpha:
```rust
// In RDP::with_resolution() and reset()
renderer.clear(0xFF000040); // Dark blue, fully opaque
```

**Result**: Blue screen now visible, confirms rendering pipeline works.

---

### 2. GPR Initialization
**File**: `crates/systems/n64/src/cpu.rs` (lines 47-62)

**Problem**: Game jumped to address 0x00000000 (null pointer) due to uninitialized registers.

**Fix**: Added proper register initialization matching real N64 IPL3 boot:
```rust
// Initialize GPRs that are expected by commercial ROMs
self.cpu.gpr[11] = 0xFFFFFFFFA4000040; // $t3 = cart domain 1 config address
self.cpu.gpr[20] = 0x0000000000000001; // $s4 = 1
self.cpu.gpr[22] = 0x000000000000003F; // $s6 = 0x3F  
self.cpu.gpr[29] = 0xFFFFFFFFA4001FF0; // $sp = stack pointer
self.cpu.gpr[31] = 0xFFFFFFFFA4001550; // $ra = return address
```

**Result**: Game executes past initial crash, reaches polling loop.

---

### 3. VI Interrupt System Fix
**File**: `crates/systems/n64/src/vi.rs` (line 82)

**Problem**: VI_INTR default was 0x3FF (scanline 511), but NTSC only has 262 scanlines. Interrupt never fired.

**Fix**: Changed default to valid scanline:
```rust
intr: 0x200, // Default to scanline 256 (0x200 >> 1 = 0x100 = 256)
```

**Result**: VI interrupts now fire every frame at scanline 256.

---

### 4. Logging Infrastructure
**Files**: Multiple (cpu.rs, vi.rs, rsp.rs, bus.rs, pif.rs, lib.rs)

**Added**:
- Frame counter logging (every 60 frames)
- RSP DMA operation logging
- VI register write logging
- PIF controller access logging
- Microcode detection logging

**Result**: Can track execution and identify where game is stuck.

## Root Cause Analysis

### Primary Issue: Interrupts Disabled

**Current CP0_STATUS value**: `0x34000000`

**Binary breakdown**:
```
0x34000000 = 0011 0100 0000 0000 0000 0000 0000 0000
                  ^^ = CU1, CU0 (coprocessors enabled)
                                                     ^ = IE (0 = DISABLED)
```

**Key bits**:
- Bit 0 (IE - Interrupt Enable): **0** ← Problem!
- Bit 1 (EXL - Exception Level): 0
- Bit 2 (ERL - Error Level): 0
- Bits 28-29 (CU0, CU1): 1 (Coprocessor 0 and 1 usable)

**Why this matters**:
1. VI interrupt fires and sets IP bits in CP0_CAUSE
2. CPU checks: `(STATUS.IE == 1) && (STATUS.EXL == 0) && (STATUS.ERL == 0) && (CAUSE.IP & STATUS.IM)`
3. Since IE=0, interrupts never actually execute
4. Game waits for interrupt handler to run
5. Gets stuck in polling loop

### Evidence from Execution

**CPU trace**:
```
N64 CPU: Jump/Branch from 0xA4400020 to 0xA0543054  (repeated infinitely)
```

**Interpretation**:
- `0xA4400020` = VI register VI_LEAP (offset 0x20 from 0x04400000)
- Game reads this constant value (0x0C150C15)
- Uses it to compute jump to code checking interrupt flags
- Flags never set because handler never runs
- Loops forever

**Logs show**:
```
N64: VI interrupt triggered at scanline 256  (every frame)
N64: Frame 60 complete                       (every second)
```
But **zero** PIF access, RSP DMA, or VI writes → game stuck before initialization completes.

## Solution Path

### Option 1: Enable Interrupts in Boot (RECOMMENDED)

**Change**: `crates/systems/n64/src/cpu.rs` line 9

**Current**:
```rust
pub const CP0_STATUS_COMMERCIAL_BOOT: u64 = 0x34000000;
```

**Proposed**:
```rust
pub const CP0_STATUS_COMMERCIAL_BOOT: u64 = 0x34000001; // Enable IE bit
```

**Also verify** interrupt mask (IM field, bits 8-15 of STATUS) allows VI interrupts:
```rust
pub const CP0_STATUS_COMMERCIAL_BOOT: u64 = 0x34000401; 
// Bit 0: IE = 1 (enable interrupts)
// Bit 10: IM2 = 1 (allow interrupt line 2, typically VI)
```

**Risk**: If exception handler at 0x80000180 isn't ready, may crash. Current handler is infinite loop placeholder.

---

### Option 2: Monitor Game's Interrupt Enable

**Add logging** to track when/if game writes to CP0_STATUS:
```rust
// In cpu_mips_r4300i.rs, in MTC0 instruction handler
if rd == 12 { // CP0_STATUS
    log(LogCategory::CPU, LogLevel::Info, || {
        format!("CPU: Writing CP0_STATUS = 0x{:08X}", rt_val)
    });
}
```

**Purpose**: Determine if game expects to enable interrupts itself.

---

### Option 3: Improve Exception Handler

**Current state** (set by IPL3 boot in `pif.rs`):
```rust
const EXCEPTION_VECTOR_CODE: [u8; 8] = [
    0x08, 0x00, 0x00, 0x60, // j 0x80000180 (jump to self - infinite loop)
    0x00, 0x00, 0x00, 0x00, // nop (delay slot)
];
```

**Better placeholder**:
```rust
const EXCEPTION_VECTOR_CODE: [u8; 8] = [
    0x40, 0x1A, 0x68, 0x00, // eret (exception return)
    0x00, 0x00, 0x00, 0x00, // nop (delay slot)
];
```

**Purpose**: Allow interrupts to be acknowledged without hanging. Game may set up real handler later.

## Expected Behavior After Fix

Once interrupts work properly:

1. **VI interrupt fires** at scanline 256
2. **CPU jumps** to exception handler (0x80000180)
3. **Game's handler executes** (or ERET placeholder returns)
4. **Game exits polling loop**, continues initialization
5. **RSP initialization** - Game loads microcode via DMA
6. **Microcode detection** - F3DEX/F3DEX2 identified
7. **RSP tasks execute** - Display lists processed
8. **RDP receives commands** - Triangles queued for rendering
9. **Framebuffer updates** - Blue screen → actual graphics!

## Testing Strategy

1. **Try IE=1 first** - Single bit change, minimal risk
2. **Run with interrupt logging** - Confirm handlers execute:
   ```
   cargo run --profile release-quick -- .\roms\n64\mario.z64 --log-interrupts info --log-ppu info --log-file n64_test.log
   ```
3. **Watch for RSP activity** - Should see DMA operations within 1-2 seconds
4. **Check for PIF access** - Controller polling should start
5. **Monitor VI writes** - Game should configure display
6. **Verify rendering** - Triangles should appear on screen

## Debug Commands

### Build and run with logging:
```powershell
cargo build --profile release-quick
.\target\release-quick\hemu.exe .\roms\n64\mario.z64 --log-interrupts info --log-ppu info --log-file n64.log
```

### Check log after ~5 seconds:
```powershell
Get-Content n64.log | Select-Object -First 100
```

### Look for key events:
```powershell
Get-Content n64.log | Select-String "interrupt|RSP|DMA|Microcode"
```

## Files Modified This Session

| File | Lines Changed | Purpose |
|------|--------------|---------|
| `crates/systems/n64/src/cpu.rs` | 47-62 | GPR initialization |
| `crates/systems/n64/src/vi.rs` | 82, 125-147 | VI_INTR default, register logging |
| `crates/systems/n64/src/rdp.rs` | 185-240 | Framebuffer color initialization |
| `crates/systems/n64/src/rsp.rs` | 211-219, 275-282 | DMA logging, microcode detection logging |
| `crates/systems/n64/src/bus.rs` | 159-174 | RSP task processing logging |
| `crates/systems/n64/src/pif.rs` | 270-282 | Controller command logging |
| `crates/systems/n64/src/lib.rs` | 180-193 | Frame counter logging |

## Known Issues

### PowerShell Logging Quirk
When piping output (`2>&1 | Select-Object`), GUI window closes immediately. Use `--log-file` instead:
```powershell
# ❌ Doesn't work:
cargo run --release -- rom.z64 --log-ppu info 2>&1 | Select-Object -First 50

# ✅ Works:
cargo run --release -- rom.z64 --log-ppu info --log-file output.log
```

### Log File Buffering
Log files may appear empty if emulator is terminated mid-write. Let it run for a few seconds before checking.

## Next Session Priorities

1. **Enable IE bit** in CP0_STATUS (1-line change, high impact)
2. **Test interrupt execution** - Verify handlers run
3. **Monitor RSP DMA** - Should happen once interrupts work
4. **Track microcode loading** - Confirm F3DEX/F3DEX2 detected
5. **Watch for first triangle** - Validates full rendering pipeline

## Reference Documentation

- **ARCHITECTURE.md** - System overview, renderer patterns
- **AGENTS.md** - Build instructions, pre-commit checks
- **N64 README** - N64-specific implementation details
- **CPU Reference** - `docs/references/cpu_mips_r4300i.md`

## Contact Points

- Framebuffer init: `crates/systems/n64/src/rdp.rs:185`
- GPR init: `crates/systems/n64/src/cpu.rs:47`
- Interrupt enable: `crates/systems/n64/src/cpu.rs:9` (CP0_STATUS_COMMERCIAL_BOOT)
- VI interrupt: `crates/systems/n64/src/lib.rs:218` (update_scanline)
- Exception handler: `crates/systems/n64/src/pif.rs:50` (EXCEPTION_VECTOR_CODE)
