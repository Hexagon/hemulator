# Agent Handoff - PC/DOS Boot Investigation

## Current Status: BLOCKED - Investigating Fundamental CPU/Boot Issue

**Date**: December 26, 2025  
**System**: IBM PC/XT Emulator (Work-in-Progress)  
**Issue**: DOS boot sector fails to boot despite correctly reading system files

---

## Problem Statement

All DOS disk images (MS-DOS 6.21, FreeDOS, etc.) fail to boot with error: **"Non-System disk or disk error"**

This is a **known good disk image** (Dos6.21.img) that boots successfully in other emulators (QEMU, VirtualBox, etc.) but fails in our WIP emulator.

### Critical Context from User
- **"Known good image"** - verified to work in other emulators
- **"FreeDOS and other versions of DOS images has the same or similar problems"** - affects ALL DOS versions
- **"BIOS, CPU and the bus, and image reading of this system is WIP. Think outside the box."** - suggests fundamental architectural issue

---

## What Works ✅

1. **INT 13h Disk Services (AH=02h Read Sectors)**
   - Correctly reads sectors from disk image file
   - CHS to LBA conversion working: `LBA = (C×H + H)×SPT + (S-1)`
   - Properly handles 1-based sector numbering
   - Data successfully written to memory at ES:BX
   - Added BIOS spec-compliant validations:
     - Rejects count==0 (illegal parameter)
     - Rejects count>=128 (exceeds BIOS limit)
     - Checks 64KB segment boundary crossing (returns error 0x09)

2. **Disk Image Reading**
   - File: `C:\Users\user\Downloads\Dos6.21.img` (1.44MB FAT12 floppy)
   - Boot sector: LBA 0, signature 0x55AA verified
   - Root directory: LBA 19-32 (14 sectors)
   - FAT tables: LBA 1-9 (FAT1), LBA 10-18 (FAT2)
   - System files confirmed present on disk:
     - **IO.SYS**: cluster 0x0002, size 40,566 bytes, attr 0x07 (hidden+system+read-only)
     - **MSDOS.SYS**: cluster 0x0052, size 38,138 bytes, attr 0x07

3. **Boot Sector Execution**
   - Boot sector loads from LBA 0 to memory 0x7C00
   - Executes successfully up to directory read
   - Reads root directory sectors (LBA 19, 20, 21)
   - Directory data correctly written to memory:
     - Entry 0 at 0x0500: IO.SYS (verified bytes match disk)
     - Entry 1 at 0x0520: MSDOS.SYS (verified bytes match disk)

---

## What Doesn't Work ❌

### Critical Issue: Boot Sector Rejects Valid Files

**Boot sequence observed:**
1. Boot sector reads directory ✅ (LBA 19, 20, 21)
2. Boot sector finds both IO.SYS and MSDOS.SYS entries ✅
3. Boot sector **displays error message WITHOUT attempting to load system files** ❌
4. Boot sector **NEVER reads data clusters** (LBA 33+ never accessed) ❌
5. CPU gets stuck in infinite loop at address **0x7CF7** executing **POP SI** (opcode 0x5E) ❌

### Infinite Loop Details

**Address**: 0x7CF7  
**Instruction**: POP SI (opcode 0x5E)  
**Behavior**: Instruction executes repeatedly without IP advancing  

**Disassembly at 0x7CF7:**
```
5E          POP SI
1F          POP DS  
8F 04       POP [SI]
8F 44 02    POP [SI+2]
CD 19       INT 19h  ; Bootstrap loader
```

This is cleanup code before INT 19h (bootstrap), indicating boot sector decided to reject the disk.

**Trace Output:**
```
[PC] 0x7CF7 opcode=5E
[PC] 0x7CF7 opcode=5E
[PC] 0x7CF7 opcode=5E
... (repeats infinitely)
```

---

## Investigation History

### Attempts Made

1. **INT 13h Specification Compliance** ✅ COMPLETED
   - Added sector count validation (0 and >=128 checks)
   - Added 64KB boundary crossing check
   - Implementation now matches official BIOS specification
   - **Result**: No change in boot behavior

2. **Directory Entry Logging** ✅ COMPLETED, THEN CLEANED UP
   - Enhanced logging to show both IO.SYS and MSDOS.SYS entries
   - Verified both files are correctly read from disk
   - Verified data correctly written to memory
   - **Result**: Confirmed data transfer is working - removed debug code

3. **Disk Image File Reading Verification** ✅ VERIFIED
   - Used PowerShell to verify files exist in disk image
   - Confirmed IO.SYS at cluster 2, MSDOS.SYS at cluster 82
   - **Result**: Disk image is valid and correctly formatted

4. **Stack Simulation for INT Handling** ❌ FAILED, REVERTED
   - Attempted to simulate full INT/IRET behavior
   - Pushed FLAGS/CS/IP on INT entry, popped on exit
   - **Result**: Made no difference - boot still fails identically
   - **Reverted**: Back to simple INT interception approach

### Current INT 13h Interception Strategy

**Location**: `crates/systems/pc/src/cpu.rs`, function `handle_int13h()`

**Approach**: Intercept INT 0x13 **BEFORE** CPU executes it
1. Check if next instruction is INT 0x13 (opcodes 0xCD 0x13)
2. If yes: advance IP by 2 bytes
3. Call appropriate handler (read_sectors, get_params, etc.)
4. Return directly without executing INT instruction

**Why this approach?**
- Avoids need for BIOS code in memory
- Bypasses interrupt vector table complexity
- Directly provides disk services

**Why it might be wrong?**
- User hinted "think outside the box" and noted "BIOS/CPU/bus are WIP"
- Boot sector might expect standard INT/IRET stack manipulation
- Other BIOS interrupts (INT 10h, 16h, 21h) also intercepted this way

---

## Technical Details

### File Locations

**Modified Files:**
- `crates/systems/pc/src/cpu.rs` - PcCpu wrapper with INT handlers (~1700 lines)
  - `handle_int13h()` at line ~1082 - INT 13h interception
  - `int13h_read_sectors()` at line ~1188 - Sector read implementation
  - `int13h_get_drive_params()` at line ~1292 - Geometry query

**Core CPU:**
- `crates/core/src/cpu_8086.rs` - Core 8086 emulation
  - Line 4890-4896: POP r16 implementation (suspected issue)
  - Line ~5000+: step() function (instruction execution)

**Disk Controller:**
- `crates/systems/pc/src/disk.rs` - CHS/LBA conversion and file I/O
  - Line 58-95: read_sectors() implementation
  - Working correctly (verified through logging)

### Boot Disk Details

**Image**: Dos6.21.img (C:\Users\user\Downloads\Dos6.21.img)  
**Format**: FAT12, 1.44MB floppy  
**Geometry**: 
- 80 cylinders
- 2 heads  
- 18 sectors/track
- 512 bytes/sector

**Layout:**
- LBA 0: Boot sector (MSDOS5.0 OEM, signature 0x55AA)
- LBA 1-9: FAT1 (File Allocation Table)
- LBA 10-18: FAT2 (backup FAT)
- LBA 19-32: Root directory (14 sectors, 224 entries max)
- LBA 33+: Data area (clusters start at 2)

**System Files:**
- IO.SYS: Cluster 2, size 40,566 bytes, attributes 0x07
- MSDOS.SYS: Cluster 82 (0x52), size 38,138 bytes, attributes 0x07

---

## Test Resources

### Available Test Images

1. **Primary Test Image**: `Dos6.21.img`
   - Location: `C:\Users\user\Downloads\Dos6.21.img`
   - Status: Known good, boots in QEMU/VirtualBox

2. **Alternative Test Image**: `x86BOOT.img`
   - Location: `test_roms/pc/x86BOOT.img` (in repository)
   - Status: Available for testing different boot scenarios

### Debug Environment Variables

Enable verbose logging with PowerShell:
```powershell
$env:EMU_TRACE_PC=1          # Log PC hotspots every 60 frames
$env:EMU_LOG_BRK=1           # Log BRK instruction execution
$env:EMU_LOG_UNKNOWN_OPS=1   # Log unknown opcodes
```

Run test:
```powershell
cargo run --release -- .\virtual_machine.hemu
```

Clean up:
```powershell
Remove-Item Env:EMU_TRACE_PC -ErrorAction SilentlyContinue
```

---

## Hypotheses for Next Agent

### Primary Hypothesis: CPU Instruction Execution Bug

**Evidence:**
- Infinite loop at POP SI suggests IP not advancing
- Core issue is in `crates/core/src/cpu_8086.rs`
- All POP instructions may be affected

**Investigation Steps:**
1. Check POP r16 implementation (line 4890-4896)
2. Verify step() function returns correct cycle count
3. Test if IP register is properly incremented after POP
4. Create unit test for POP SI specifically

### Secondary Hypothesis: Flag Handling in Conditional Jumps

**Evidence:**
- Boot sector likely uses JC/JNC after INT 13h to check carry flag
- If flags aren't set correctly, boot sector takes wrong path
- Never attempts to load system files (suggests wrong branch taken)

**Investigation Steps:**
1. Trace boot sector execution to find decision point
2. Log carry flag state after INT 13h returns
3. Check if boot sector uses carry flag to determine success/failure
4. Verify CPU sets/clears carry flag correctly in INT handlers

### Tertiary Hypothesis: INT Interception Architecture

**Evidence:**
- User emphasized "think outside the box"
- "BIOS, CPU, and bus are WIP"
- Current approach intercepts INT before execution
- Standard approach would use BIOS code at interrupt vectors

**Investigation Steps:**
1. Consider implementing actual BIOS code in memory
2. Set up interrupt vector table (IVT) at 0x0000-0x03FF
3. Place BIOS INT handlers in high memory (0xF0000+)
4. Let CPU execute INT instruction normally
5. BIOS code calls disk controller and returns via IRET

---

## Code Quality Notes

### What Was Cleaned Up
- ✅ Removed excessive directory entry logging
- ✅ Removed memory dump debug code  
- ✅ Removed verbose error messages
- ✅ Reverted stack simulation attempts

### What Was Kept
- ✅ INT 13h parameter validation (sector count, boundary checks)
- ✅ Basic error logging (status codes, CHS parameters)
- ✅ Working INT 13h implementation

### Code is Now Clean and Ready
- No debug cruft
- Minimal logging for troubleshooting
- Clear separation between working and broken functionality
- Ready for next agent to investigate CPU/boot issue

---

## Recommended Next Steps

1. **Immediate**: Fix infinite loop at POP SI (0x5E)
   - File: `crates/core/src/cpu_8086.rs`
   - Focus: Verify IP advancement in step() function
   - Test: Create unit test for POP instruction

2. **Short-term**: Trace boot sector execution path
   - Identify where boot sector decides files are invalid
   - Log flag states (especially carry flag) after INT 13h
   - Determine if wrong branch is being taken

3. **Long-term**: Consider BIOS architecture overhaul
   - User hint "think outside the box" suggests fundamental issue
   - May need to implement proper BIOS ROM instead of interception
   - Would align with how real hardware works

---

## Success Criteria

Boot is considered **working** when:
1. Boot sector reads directory ✅ (already working)
2. Boot sector finds system files ✅ (already working)  
3. Boot sector **loads IO.SYS from data clusters** ❌ (not happening)
4. Boot sector **transfers control to IO.SYS** ❌ (not happening)
5. IO.SYS displays "Starting MS-DOS..." or similar ❌ (never reached)

---

## Additional Context

- All DOS versions fail similarly (MS-DOS, FreeDOS, PC-DOS)
- Problem is NOT in disk reading (verified extensively)
- Problem is NOT in data transfer (directory entries correct in memory)
- Problem IS in CPU execution or boot sector decision logic
- Boot sector never even TRIES to load system files - suggests it thinks files are invalid
- Infinite loop at cleanup code suggests CPU execution issue, not logic issue

---

## Final Notes

This is a **work-in-progress emulator**. The CPU, BIOS, and bus implementations are not complete. The issue is likely a fundamental architectural problem rather than a simple bug in INT 13h.

The user's hint to "think outside the box" and emphasis on "WIP" status suggests the solution may require rethinking the BIOS interception approach or fixing core CPU instruction execution.

**Good luck!** 🚀
