# PC BIOS Interrupt Handling Analysis

**Date**: 2025-12-28  
**Purpose**: Comprehensive evaluation of PC emulator interrupt handling against BIOS interrupt reference  
**Status**: ✅ **HIGH priority issues implemented** - Critical functions for HIMEM.SYS and QBasic now working

## Predicted Issues for Linux Kernel and Other Operating Systems

### 🔴 CRITICAL: Linux Kernel Boot Requirements

Based on Linux kernel boot protocol analysis, the following issues are predicted:

#### 1. **INT 15h AX=E820h: Memory Map Query** - 🔴 **CRITICAL**
**Status**: ❌ **Stub only - returns no entries**
**Impact**: Linux kernel 2.6+ **WILL FAIL** to boot
**Reason**: Modern Linux requires E820 memory map to:
- Detect available RAM
- Identify reserved regions (BIOS, ACPI, etc.)
- Set up page tables and memory management

**Current Implementation** (`cpu.rs:3405-3413`):
```rust
fn int15h_query_system_address_map(&mut self) -> u32 {
    self.cpu.bx = 0; // No continuation
    self.set_carry_flag(true); // No more entries
    51
}
```

**Required Implementation**:
- Return memory map entries describing:
  - Conventional memory (0x00000000 - 0x0009FFFF) - Type 1 (available)
  - VGA/BIOS reserved (0x000A0000 - 0x000FFFFF) - Type 2 (reserved)
  - Extended memory (0x00100000 - end of RAM) - Type 1 (available)
- Use continuation value in EBX to iterate through entries
- Each entry: base address (64-bit), length (64-bit), type (32-bit)

**Priority**: 🔴 **CRITICAL** - Linux 2.6+ will not boot without this

#### 2. **INT 15h AX=E801h: Extended Memory Size** - 🟠 **HIGH**
**Status**: ⚠️ **Partial - returns fixed values**
**Impact**: Linux fallback detection may fail
**Reason**: If E820h fails, kernel tries E801h as fallback

**Current Implementation** (`cpu.rs:3383-3403`):
```rust
fn int15h_get_extended_memory_size_e801(&mut self) -> u32 {
    // Returns memory in two ranges:
    // AX/CX = memory between 1MB-16MB in 1KB blocks (max 15MB = 0x3C00)
    // BX/DX = memory above 16MB in 64KB blocks
    let mem_1m_16m = 0x3C00; // 15MB (full)
    let mem_above_16m = 0x0000; // None
    
    self.cpu.ax = mem_1m_16m;
    self.cpu.cx = mem_1m_16m;
    self.cpu.bx = mem_above_16m;
    self.cpu.dx = mem_above_16m;
    self.set_carry_flag(false);
    51
}
```

**Issue**: Hardcoded to 15MB, doesn't reflect actual system memory
**Fix Needed**: Read from bus.get_extended_memory_kb()

#### 3. **INT 15h AH=88h: Get Extended Memory Size** - 🟠 **HIGH**
**Status**: ⚠️ **Returns 0 - no extended memory**
**Impact**: Very old kernels (pre-2.4) and boot loaders may fail

**Current Implementation** (`cpu.rs:3373-3381`):
```rust
fn int15h_get_extended_memory_size(&mut self) -> u32 {
    self.cpu.ax = 0; // No extended memory
    self.set_carry_flag(false);
    51
}
```

**Fix Needed**: Return actual extended memory size in KB

#### 4. **INT 13h AH=42h: LBA Extended Read** - 🟠 **MEDIUM-HIGH**
**Status**: ❌ **Stub - returns "not supported"**
**Impact**: Modern boot loaders (GRUB2, LILO) may fail on large disks

**Reason**: 
- CHS addressing limited to ~8GB disks
- Modern Linux installations use LBA for boot
- GRUB2 requires LBA for disks >504MB

**Priority**: 🟠 **HIGH** for disk images >8GB

#### 5. **APM (Advanced Power Management) - INT 15h AH=53h** - 🟡 **LOW-MEDIUM**
**Status**: ❌ **Not implemented**
**Impact**: Older kernels may log warnings but usually continue

**Functions needed**:
- 53h/00h: APM installation check
- 53h/01h: Connect real mode interface
- 53h/02h: Connect 16-bit protected mode
- 53h/07h: Enable/disable power management

**Priority**: 🟡 **LOW** - Kernel will fall back to other methods

#### 6. **VESA BIOS Extensions (VBE) - INT 10h AH=4Fh** - 🟡 **MEDIUM**
**Status**: ❌ **Not implemented**
**Impact**: Graphical boot (splash screen) will fail, text mode works

**Functions needed**:
- 4F00h: Get VBE controller information
- 4F01h: Get VBE mode information
- 4F02h: Set VBE mode
- 4F03h: Get current VBE mode

**Priority**: 🟡 **MEDIUM** - Required for framebuffer console

### 🟠 Windows Boot Requirements

#### 1. **INT 13h Extended Functions** - 🟠 **HIGH**
**Status**: ❌ **Mostly stubs**
**Impact**: Windows 2000+ may have boot issues

**Required for Windows**:
- AH=42h: Extended read (LBA)
- AH=43h: Extended write (LBA)
- AH=48h: Get drive parameters (extended)

#### 2. **PnP BIOS - INT 15h AH=C1h** - 🟡 **MEDIUM**
**Status**: ⚠️ **Stub - returns "not supported"**
**Impact**: Device detection may be incomplete

### 🔵 FreeBSD/NetBSD Boot Requirements

#### 1. **INT 15h E820h** - 🔴 **CRITICAL**
Same as Linux - modern BSD kernels require memory map

#### 2. **INT 13h LBA support** - 🟠 **HIGH**
Boot loaders expect LBA for modern disks

### Summary of Predicted Boot Failures

| Operating System | Will Boot? | Critical Missing Features |
|-----------------|------------|--------------------------|
| MS-DOS 6.22 | ✅ Yes | ✅ All implemented |
| MS-DOS 5.0 + HIMEM | ✅ Yes | ✅ A20 gate now working |
| Windows 95/98 | 🟡 Maybe | 🟠 INT 13h LBA, 🟡 APM |
| Windows 2000/XP | 🔴 No | 🔴 INT 15h E820h, 🟠 INT 13h LBA |
| Linux 2.4.x | 🔴 No | 🔴 INT 15h E820h |
| Linux 2.6+ | 🔴 No | 🔴 INT 15h E820h (CRITICAL) |
| FreeBSD 8+ | 🔴 No | 🔴 INT 15h E820h |
| NetBSD 6+ | 🔴 No | 🔴 INT 15h E820h |

### Recommended Implementation Priority

1. **🔴 CRITICAL (Blocks Linux/Modern Windows)**:
   - INT 15h AX=E820h: Memory map query (full implementation)
   - INT 15h AH=88h: Extended memory size (from bus)
   - INT 15h AX=E801h: Extended memory (from bus)

2. **🟠 HIGH (Improves compatibility)**:
   - INT 13h AH=42h: LBA read
   - INT 13h AH=43h: LBA write
   - INT 13h AH=48h: Extended drive parameters

3. **🟡 MEDIUM (Nice to have)**:
   - INT 10h AH=4Fh: VESA VBE (for framebuffer)
   - INT 15h AH=53h: APM (for power management)

**Estimated Implementation Effort**:
- INT 15h E820h: ~100 lines (memory map table + iteration logic)
- INT 13h LBA functions: ~80 lines (42h + 43h)
- Total critical path: ~180 lines to enable Linux boot

**Status**: ✅ **HIGH priority issues implemented** - Critical functions for HIMEM.SYS and QBasic now working

## Implementation Status (2025-12-28)

### ✅ Completed HIGH Priority Fixes

1. **BIOS Architecture/Model Byte Consistency** ✅ FIXED
   - Made `generate_minimal_bios()` accept `cpu_model` parameter
   - CPU model now determines system architecture:
     - 8086/8088/186/188 → 0xFE (PC/XT), feature byte 0x00
     - 286 → 0xFC (AT), feature byte 0x70 (RTC, 2nd PIC, keyboard intercept)
     - 386+ → 0xF8 (PS/2), feature byte 0x70
   - Both BIOS model byte (F000:FFFE) and system config table (F000:E002) now consistent

2. **INT 15h AH=24h: A20 Gate Control** ✅ IMPLEMENTED
   - AL=00h: Disable A20 (acknowledged, always enabled in emulator)
   - AL=01h: Enable A20 (acknowledged, always enabled in emulator)
   - AL=02h: Get A20 status (returns enabled)
   - AL=03h: Get A20 support (returns supported)
   - **Impact**: HIMEM.SYS can now load successfully in MS-DOS 5.0+

3. **INT 10h AH=0Bh: Set Color Palette** ✅ IMPLEMENTED
   - BH=00h: Set background/border color
   - BH=01h: Set CGA palette ID
   - **Impact**: QBasic and other DOS applications can control colors

4. **INT 10h AH=1Bh: Get Video State** ✅ IMPLEMENTED
   - Returns video state table pointer at ES:DI
   - **Impact**: QBasic can detect video capabilities

5. **INT 10h AH=EFh, FAh: Undocumented VGA Functions** ✅ IMPLEMENTED
   - Stub handlers prevent errors
   - **Impact**: QBasic no longer crashes on these calls

### 🟡 Partial Implementation

1. **INT 08h → INT 1Ch Chaining** 🟡 DOCUMENTED
   - Tick counter properly maintained at 0040:006C
   - Midnight rollover implemented
   - **Limitation**: Direct INT 1Ch call not yet implemented (requires CPU core changes)
   - **Workaround**: Programs should hook INT 08h directly

## Executive Summary

This document analyzes the current PC emulator's interrupt handling implementation against the complete BIOS interrupt specification. The emulator implements a **selective BIOS-only approach**, correctly handling only the interrupts that should be provided by the BIOS while leaving DOS and OS interrupts to the guest operating system.

**Overall Assessment**: ✅ **Architecture is correct** - The emulator correctly distinguishes between BIOS responsibilities and OS responsibilities.

**Key Findings**:
- ✅ Main BIOS services (INT 10h, 13h, 16h) are well-implemented
- ✅ **FIXED**: INT 15h AH=24h (A20 gate control) now implemented
- ✅ **FIXED**: INT 10h AH=0Bh (palette control) now implemented
- ✅ **FIXED**: INT 10h AH=1Bh (video state) now implemented
- ✅ **FIXED**: BIOS model bytes now consistent and adapt to CPU model
- ⚠️ CPU exceptions (INT 00h-10h) are minimally implemented
- ⚠️ Hardware IRQ handlers (INT 08h-77h) are mostly stubs
- ✅ OS interrupts (INT 20h-31h) are correctly NOT intercepted (DOS handles them)
- ⚠️ Extended BIOS services need expansion
- ⚠️ **Keyboard Issue**: Hardcoded to XT scan code set 1 (AT/PS2 should support set 2)

## Real-World Testing Findings (MS-DOS 5.0 Boot)

Testing with MS-DOS 5.0 and QBasic revealed the following critical missing interrupts:

### Boot Failures
```
HIMEM: DOS XMS Driver, Version 2.78 - 09/19/91
NOTICE: Stub interrupt handler called: INT 0x15, AH=0x24 (Extended Services) at 024B:073B
ERROR: Unable to control A20 line!
XMS Driver not installed.
HMA not available : Loading DOS low
```

**Issue**: INT 15h AH=24h (A20 gate control) is not implemented, preventing HIMEM.SYS from loading.

### QBasic Failures
```
A:\>qbasic
NOTICE: Stub interrupt handler called: INT 0x10, AH=0x1B (Video BIOS) at 23F5:7E1F
NOTICE: Stub interrupt handler called: INT 0x10, AH=0xEF (Video BIOS) at 23F5:7E1F
NOTICE: Stub interrupt handler called: INT 0x10, AH=0xFA (Video BIOS) at 47C1:0AED
```

**Issue**: Multiple INT 10h video functions missing (AH=0Bh, 1Bh, EFh, FAh), causing display issues in QBasic.

### Priority Upgrade

Based on real-world testing, these functions are upgraded to **HIGH** priority:
- INT 15h AH=24h: A20 gate control (breaks HIMEM.SYS)
- INT 10h AH=0Bh: Set color palette (used by many DOS apps)
- INT 10h AH=1Bh: Get video state (used by QBasic)

---

## Architecture and System Model Handling

### Current Implementation Issues

The emulator has **inconsistent architecture reporting** that doesn't adapt to the selected CPU model:

#### 1. **BIOS Model Byte Mismatch**

**Location**: `bios.rs:271` and `bios.rs:124`

- **System Configuration Table** (INT 15h AH=C0h): Reports **0xFC** (AT system)
- **BIOS Model Byte** (F000:FFFE): Reports **0xFE** (PC/XT)
- **Issue**: These should match and adapt based on CPU model

**Standard PC Architecture Models**:
```
0xFF = Original PC (8088)
0xFE = PC/XT (8088)
0xFD = PCjr
0xFC = PC/AT (80286+)
0xFB = PC/XT Model 286
0xFA = PS/2 Model 25/30 (8086)
0xF9 = PC Convertible
0xF8 = PS/2 Model 80 (80386)
```

**Expected Mapping**:
- Intel 8086/8088 → 0xFE (PC/XT) or 0xFF (PC)
- Intel 80186/80188 → 0xFE (XT-compatible)
- Intel 80286 → 0xFC (AT)
- Intel 80386+ → 0xF8 (PS/2 Model 80) or 0xFC (AT-compatible)

#### 2. **Keyboard Scan Code Set**

**Location**: `keyboard.rs`

- **Current**: Uses PC/XT scan code set 1 (hardcoded)
- **Issue**: AT and PS/2 systems should support scan code set 2
- **Impact**: Some DOS software may check keyboard type via INT 16h or port 60h

**Scan Code Set Evolution**:
- **PC/XT**: Scan code set 1 only
- **AT**: Scan code set 2 (default), can switch to set 1
- **PS/2**: Scan code set 2 or 3

#### 3. **Feature Byte Inconsistency**

**Location**: `bios.rs:127-135`

Current feature byte 1 (`0x70`):
```
bit 6: 2nd 8259 installed (1) ← AT/PS2 feature
bit 5: Real-time clock (1)    ← AT/PS2 feature
bit 4: INT 15h/AH=4Fh (1)     ← AT/PS2 feature
```

**Issue**: Features indicate AT system, but model byte says XT

#### 4. **Temperature Sensors**

**Status**: ❌ **Not implemented**

- **PC/XT/AT**: No temperature sensor support in BIOS
- **Modern PS/2+**: Some models have thermal monitoring
- **Recommendation**: Not needed for DOS compatibility
- **Note**: Temperature sensors are not reported through standard BIOS interrupts in PC/AT era systems

### Recommendations

#### 🔴 **HIGH Priority**: Fix Architecture Consistency

1. **Make BIOS generation dynamic** based on CPU model:
   ```rust
   pub fn generate_minimal_bios(cpu_model: CpuModel) -> Vec<u8> {
       let model_byte = match cpu_model {
           CpuModel::Intel8086 | CpuModel::Intel8088 => 0xFE, // PC/XT
           CpuModel::Intel80186 | CpuModel::Intel80188 => 0xFE, // XT-compatible
           CpuModel::Intel80286 => 0xFC, // AT
           _ => 0xF8, // PS/2 Model 80 (386+)
       };
       // ... use model_byte in both locations
   }
   ```

2. **Match feature bytes to model**:
   - XT (0xFE): Feature byte 1 = `0x00` (no RTC, no 2nd PIC)
   - AT (0xFC): Feature byte 1 = `0x70` (RTC, 2nd PIC, keyboard intercept)
   - PS/2 (0xF8): Feature byte 1 = `0x70`, additional features in bytes 2-5

3. **Update INT 15h AH=C0h handler** to match model byte

#### 🟠 **MEDIUM Priority**: Keyboard Scan Code Set Support

- Implement scan code set 2 for AT/PS/2 models
- Add keyboard controller command to switch sets (port 60h/64h)
- Current set 1 implementation is acceptable for XT mode

#### 🟡 **LOW Priority**: Extended System Information

- Add submodel byte based on specific 286/386/486 variant
- Temperature sensors: Not needed for DOS compatibility

### Code Impact

- **BIOS generation**: ~30 lines (add cpu_model parameter, switch logic)
- **Update callers**: ~10 lines (pass cpu_model to generate_minimal_bios)
- **Feature byte logic**: ~20 lines (conditional feature byte generation)
- **Total**: ~60 lines

**Risk**: 🟢 **LOW** - Changes are localized to BIOS generation

---

## 1. CPU Exceptions (INT 00h-10h) - BIOS Responsibility

These are CPU-generated exceptions that the BIOS should handle in real mode.

### Currently Implemented

| INT | Description | Status | Location | Notes |
|-----|-------------|--------|----------|-------|
| 00h | Divide by zero | ✅ Stub | `bios.rs:54` + `cpu.rs` | BIOS ROM has handler at 0x50, returns via IRET |
| 01h-04h | Single step, NMI, Breakpoint, Overflow | ❌ Missing | - | Not implemented |
| 05h | BOUND range exceeded (186+) | ✅ Stub | `cpu.rs:237` | Returns via handle_int05h() |
| 06h-0Eh | Invalid opcode, Coprocessor, etc. | ❌ Missing | - | Not implemented |
| 0Fh | Reserved | ❌ Missing | - | Not implemented |
| 10h | Coprocessor error | ❌ Missing | - | Not implemented |

### Analysis

**Current State**: Only INT 00h and INT 05h have basic handlers. Other CPU exceptions are not intercepted.

**Recommendation**: 
- **Priority: LOW** - Most DOS programs don't rely on CPU exception handlers
- **Action**: Add stub handlers that log and IRET for INT 01h-04h, 06h-10h
- **Rationale**: CPU exceptions are rare in typical DOS programs; logging would help debugging

**Code Impact**: Minimal - add ~10 lines per exception in cpu.rs

---

## 2. Hardware IRQ Handlers (INT 08h-77h) - BIOS Responsibility

Hardware interrupts are triggered by devices but initialized by BIOS. The BIOS must provide default handlers.

### IRQ 0-7 (Master PIC: INT 08h-0Fh)

| INT | IRQ | Device | Status | Implementation | Notes |
|-----|-----|--------|--------|----------------|-------|
| 08h | 0 | Timer tick (~18.2 Hz) | ✅ Implemented | `cpu.rs:1465` | Increments tick counter at 0040:006C, handles midnight rollover |
| 09h | 1 | Keyboard | ✅ Stub | `cpu.rs:239` | Returns via handle_int09h(), keyboard handled via INT 16h |
| 0Ah | 2 | Cascade (AT+) | ❌ Missing | - | IRQ2 cascades to slave PIC |
| 0Bh | 3 | Serial COM2/4 | ✅ Stub | `bios.rs:46` | Generic stub handler (IRET) |
| 0Ch | 4 | Serial COM1/3 | ✅ Stub | `bios.rs:46` | Generic stub handler (IRET) |
| 0Dh | 5 | LPT2/Fixed disk | ✅ Stub | `bios.rs:46` | Generic stub handler (IRET) |
| 0Eh | 6 | Floppy disk | ✅ Stub | `bios.rs:46` | Generic stub handler (IRET) |
| 0Fh | 7 | LPT1 | ✅ Stub | `bios.rs:46` | Generic stub handler (IRET) |

### IRQ 8-15 (Slave PIC: INT 70h-77h) - AT+ Systems

| INT | IRQ | Device | Status | Implementation | Notes |
|-----|-----|--------|--------|----------------|-------|
| 70h | 8 | Real-time clock | ❌ Missing | - | AT+ systems only |
| 71h | 9 | Redirected IRQ2 | ❌ Missing | - | AT+ systems only |
| 72h | 10 | Reserved | ❌ Missing | - | AT+ systems only |
| 73h | 11 | Reserved | ❌ Missing | - | AT+ systems only |
| 74h | 12 | Mouse (PS/2+) | ❌ Missing | - | PS/2 systems only |
| 75h | 13 | Math coprocessor | ❌ Missing | - | AT+ systems only |
| 76h | 14 | Fixed disk controller | ❌ Missing | - | AT+ systems only |
| 77h | 15 | Reserved | ❌ Missing | - | AT+ systems only |

### Analysis

**Current State**: 
- Primary IRQs (08h-09h) have functional stubs
- Secondary IRQs (0Ah, 0Bh-0Fh) have generic IRET handlers in BIOS ROM
- AT+ IRQs (70h-77h) are not implemented

**Recommendation**:
- **Priority: MEDIUM** - Timer tick (INT 08h) should increment BIOS tick counter
- **Action for INT 08h**: Implement proper tick counter at 0040:006C (DWORD)
- **Action for INT 70h-77h**: Add stub handlers for AT compatibility
- **Rationale**: Many DOS programs rely on timer tick count for timing

**Code Impact**: 
- INT 08h: ~15 lines to read/write tick counter at 0040:006C
- INT 70h-77h: ~5 lines each for stub handlers

---

## 3. Main BIOS Services (INT 05h, 10h-1Ah) - BIOS Responsibility

These are the primary BIOS software interrupts that provide hardware abstraction.

### INT 05h - Print Screen

| Function | Description | Status | Implementation |
|----------|-------------|--------|----------------|
| - | Print screen | ✅ Stub | `cpu.rs:237` handle_int05h() |

**Analysis**: Stub is sufficient for emulation purposes.

### INT 10h - Video Services

**Overall Status**: ✅ **Well-implemented** (18 functions)

| AH | Function | Status | Implementation | Notes |
|----|----------|--------|----------------|-------|
| 00h | Set video mode | ✅ Implemented | `cpu.rs:332` int10h_set_video_mode() | Acknowledges mode change |
| 01h | Set cursor shape | ✅ Implemented | `cpu.rs:342` int10h_set_cursor_shape() | Acknowledges shape change |
| 02h | Set cursor position | ✅ Implemented | `cpu.rs:350` int10h_set_cursor_position() | Stores in BIOS data area 0040:0050 |
| 03h | Get cursor position | ✅ Implemented | `cpu.rs:366` int10h_get_cursor_position() | Reads from BIOS data area |
| 05h | Set active page | ✅ Implemented | `cpu.rs:683` int10h_select_active_page() | Stores page at 0040:0062 |
| 06h | Scroll up | ✅ Implemented | `cpu.rs:383` int10h_scroll_up() | Full implementation with clear |
| 07h | Scroll down | ✅ Implemented | `cpu.rs:431` int10h_scroll_down() | Full implementation |
| 08h | Read char/attr | ✅ Implemented | `cpu.rs:479` int10h_read_char_attr() | Reads from video memory |
| 09h | Write char/attr | ✅ Implemented | `cpu.rs:503` int10h_write_char_attr() | Writes to video memory |
| 0Ah | Write char only | ✅ Implemented | `cpu.rs:693` int10h_write_char_only() | Preserves attributes |
| 0Ch | Write pixel | ✅ Implemented | `cpu.rs:724` int10h_write_pixel() | Graphics mode support |
| 0Dh | Read pixel | ✅ Implemented | `cpu.rs:746` int10h_read_pixel() | Graphics mode support |
| 0Eh | Teletype output | ✅ Implemented | `cpu.rs:559` int10h_teletype_output() | Full implementation with scrolling |
| 0Fh | Get video mode | ✅ Implemented | `cpu.rs:629` int10h_get_video_mode() | Returns mode 3 (80x25) |
| 10h | Palette functions | ✅ Implemented | `cpu.rs:769` int10h_palette_functions() | Partial (subfunction 03h) |
| 11h | Character generator | ✅ Stub | `cpu.rs:795` int10h_character_generator() | Acknowledges only |
| 12h | Video subsystem config | ✅ Stub | `cpu.rs:807` int10h_video_subsystem_config() | Acknowledges only |
| 13h | Write string | ✅ Implemented | `cpu.rs:639` int10h_write_string() | Full implementation |
| 1Ah | Display combination | ✅ Implemented | `cpu.rs:822` int10h_display_combination() | Returns VGA (00h/08h) |

**Additional INT 10h functions NOT implemented**:
- 04h: Read light pen position (rarely used)
- **0Bh: Set color palette (CGA-specific) - 🔴 HIGH PRIORITY - used by QBasic and many DOS apps**
- 14h-19h: Various LCD/video functions (uncommon)
- **1Bh: Get video state (VGA BIOS extension) - 🔴 HIGH PRIORITY - used by QBasic**
- 1Ch: Save/restore video state (VGA BIOS extension)
- **EFh, FAh: Undocumented VGA functions - used by QBasic**

**Analysis**: 
- ✅ **Excellent coverage** of core functions
- ✅ All commonly-used functions are implemented
- 🔴 **CRITICAL**: Missing INT 10h AH=0Bh (set palette) breaks color display in many DOS apps
- 🔴 **CRITICAL**: Missing INT 10h AH=1Bh (get video state) causes QBasic display issues
- ⚠️ Missing undocumented functions EFh, FAh used by some applications

**Recommendation**:
- **Priority: HIGH** - Implement INT 10h AH=0Bh (set color palette) - **REQUIRED for proper color support**
- **Priority: HIGH** - Implement INT 10h AH=1Bh (get video state) - **REQUIRED for QBasic**
- **Priority: LOW** - Add stubs for undocumented functions (EFh, FAh) to prevent errors
- **Rationale**: QBasic and many DOS applications rely on these functions for proper display

**Code Impact**: 
- AH=0Bh: ~15 lines (set palette register or overscan color)
- AH=1Bh: ~30 lines (return video state structure)
- AH=EFh, FAh: ~5 lines each (stub handlers)

### INT 11h - Equipment Determination

| Function | Description | Status | Implementation |
|----------|-------------|--------|----------------|
| - | Get equipment list | ✅ Implemented | `cpu.rs:1666` handle_int11h() |

**Analysis**: 
- ✅ Returns equipment flags based on system configuration
- ✅ Reflects floppy drives, video adapter type
- ✅ Comprehensive implementation

### INT 12h - Memory Size

| Function | Description | Status | Implementation |
|----------|-------------|--------|----------------|
| - | Get memory size | ✅ Implemented | `cpu.rs:1701` handle_int12h() |

**Analysis**:
- ✅ Returns conventional memory size in KB (from bus)
- ✅ Correct implementation

### INT 13h - Disk Services

**Overall Status**: ✅ **Well-implemented** (11 functions)

| AH | Function | Status | Implementation | Notes |
|----|----------|--------|----------------|-------|
| 00h | Reset disk | ✅ Implemented | `cpu.rs:1729` int13h_reset_disk() | Returns success |
| 01h | Get status | ✅ Implemented | `cpu.rs:1746` int13h_get_status() | Returns last status |
| 02h | Read sectors | ✅ Implemented | `cpu.rs:1763` int13h_read_sectors() | Full CHS support, floppy + HDD |
| 03h | Write sectors | ✅ Implemented | `cpu.rs:1922` int13h_write_sectors() | Full CHS support, floppy + HDD |
| 04h | Verify sectors | ✅ Implemented | `cpu.rs:2087` int13h_verify_sectors() | Returns success |
| 05h | Format track | ✅ Stub | `cpu.rs:2098` int13h_format_track() | Returns success |
| 08h | Get drive params | ✅ Implemented | `cpu.rs:2106` int13h_get_drive_params() | Returns geometry for floppy/HDD |
| 15h | Get disk type | ✅ Implemented | `cpu.rs:2271` int13h_get_disk_type() | Returns type + sector count |
| 16h | Disk change status | ✅ Implemented | `cpu.rs:2331` int13h_disk_change() | Returns "no change" |
| 41h | Check extensions | ✅ Implemented | `cpu.rs:2351` int13h_check_extensions() | Returns "not supported" |
| 42h | Extended read | ✅ Stub | `cpu.rs:2360` int13h_extended_read() | Returns "not supported" |

**Additional INT 13h functions NOT implemented**:
- 06h-07h: Format track (advanced, rarely used)
- 09h-0Dh: Initialize, read long, write long (uncommon)
| 0Eh-14h: Controller diagnostics (uncommon)
- 17h-1Ah: Set media type, park heads (uncommon)
- 43h-48h: Extended write, verify, seek (LBA extensions)

**Analysis**:
- ✅ **Comprehensive coverage** of standard CHS operations
- ✅ Proper CHS-to-LBA conversion for both floppy and HDD
- ✅ Geometry detection for 1.44MB floppy and variable HDD sizes
- ⚠️ LBA extensions (42h-48h) not implemented
- ✅ Correctly handles zero-sector reads/writes (DOS 6.21 compatibility)
- ✅ Does NOT modify ES:BX (correct BIOS behavior)

**Recommendation**:
- **Priority: MEDIUM** - Add INT 13h AH=42h (extended read) for large disk support
- **Action**: Implement basic LBA read/write (42h, 43h)
- **Rationale**: Some modern boot loaders expect LBA support for disks >8GB

**Code Impact**: ~40 lines for AH=42h, ~40 lines for AH=43h

### INT 14h - Serial Communications

| Function | Description | Status | Implementation |
|----------|-------------|--------|----------------|
| All | Serial port functions | ✅ Stub | `cpu.rs:2381` handle_int14h() |

**Analysis**: 
- ✅ Stub is sufficient (serial ports not emulated)
- ⚠️ Could log unsupported function calls for debugging

**Recommendation**: **Priority: LOW** - Add logging for diagnostic purposes

### INT 15h - System Services

**Overall Status**: ⚠️ **Partially implemented** (4 functions)

| AH | Function | Status | Implementation | Notes |
|----|----------|--------|----------------|-------|
| 41h | Wait on external event | ✅ Implemented | `cpu.rs:2413` int15h_wait_on_external_event() | Returns "not supported" |
| 4Fh | Keyboard intercept | ✅ Stub | `cpu.rs:2424` int15h_keyboard_intercept() | Returns AL unchanged |
| 86h | Wait | ✅ Stub | `cpu.rs:2433` int15h_wait() | Returns immediately |
| C0h | Get system config | ✅ Implemented | `cpu.rs:2444` int15h_get_system_config() | Returns table at 9000:E000 |
| C1h | Get extended BIOS data | ✅ Stub | `cpu.rs:2476` int15h_get_extended_bios_data() | Returns "not supported" |
| E8h | Extended memory size | ✅ Stub | `cpu.rs:2485` int15h_get_extended_memory() | Returns 0 (no extended memory) |

**Additional INT 15h functions NOT implemented**:
- **24h: Set A20 gate (PS/2+ protected mode) - 🔴 HIGH PRIORITY**
- 87h: Move extended memory block
- 88h: Get extended memory size (older method)
- 89h: Switch to protected mode (AT+)
- E820h: Get memory map (modern systems)

**Analysis**:
- ✅ System configuration table properly implemented
- ⚠️ Wait function (86h) should delay, not return immediately
- 🔴 **CRITICAL**: Missing A20 gate (24h) breaks HIMEM.SYS and prevents DOS from using extended memory
- ⚠️ Missing extended memory functions
- ⚠️ Missing E820h (memory map) for modern boot loaders

**Recommendation**:
- **Priority: HIGH** - Implement INT 15h AH=24h (A20 gate control) - **REQUIRED for HIMEM.SYS**
- **Priority: MEDIUM** - Implement INT 15h AH=86h (wait) properly
- **Priority: LOW** - Add INT 15h AH=88h (extended memory size)
- **Priority: LOW** - Add INT 15h AH=E820h (memory map) for modern loaders
- **Rationale**: A20 gate is critical for MS-DOS 5.0+ and Windows 3.x; wait function affects timing-sensitive code

**Code Impact**: 
- AH=24h: ~20 lines (A20 gate enable/disable/status)
- AH=86h: ~10 lines (simple delay loop or timestamp check)
- AH=88h: ~5 lines (return extended memory size)
- AH=E820h: ~30 lines (memory map structure)

### INT 16h - Keyboard Services

**Overall Status**: ✅ **Well-implemented** (3 functions)

| AH | Function | Status | Implementation | Notes |
|----|----------|--------|----------------|-------|
| 00h | Read keystroke | ✅ Implemented | `cpu.rs:868` int16h_read_keystroke() | Blocking read, halts CPU |
| 01h | Check keystroke | ✅ Implemented | `cpu.rs:910` int16h_check_keystroke() | Non-blocking check, sets ZF |
| 02h | Get shift flags | ✅ Implemented | `cpu.rs:934` int16h_get_shift_flags() | Returns modifier state |

**Additional INT 16h functions NOT implemented**:
- 03h: Set typematic rate
- 05h: Push keystroke
- 10h-12h: Extended keyboard functions (101/102-key)

**Analysis**:
- ✅ **Excellent implementation** of core functions
- ✅ Proper keyboard buffer management (peek vs. read)
- ✅ Shift flag tracking (left/right shift, Ctrl, Alt, etc.)
- ✅ Scancode-to-ASCII conversion with shift/AltGr support
- ⚠️ Missing extended keyboard functions (10h-12h)

**Recommendation**:
- **Priority: LOW** - Add INT 16h AH=10h-12h for 101-key keyboard
- **Rationale**: Most DOS programs use 00h-02h; extended functions are optional

**Code Impact**: ~15 lines per function (10h, 11h, 12h)

### INT 17h - Parallel Printer

| Function | Description | Status | Implementation |
|----------|-------------|--------|----------------|
| All | Printer functions | ✅ Stub | `cpu.rs:2510` handle_int17h() |

**Analysis**: ✅ Stub is sufficient (printer not emulated)

### INT 18h - Cassette BASIC / Boot Failure

| Function | Description | Status | Implementation |
|----------|-------------|--------|----------------|
| - | Boot failure | ✅ Implemented | `cpu.rs:2525` handle_int18h() |

**Analysis**: 
- ✅ Displays "No bootable disk" message
- ✅ Halts CPU (correct behavior)

### INT 19h - Bootstrap Loader

| Function | Description | Status | Implementation |
|----------|-------------|--------|----------------|
| - | Reboot system | ✅ Implemented | `cpu.rs:2548` handle_int19h() |

**Analysis**: 
- ✅ Resets CPU registers
- ✅ Jumps to BIOS entry point (F000:FFF0)
- ✅ Proper reboot implementation

### INT 1Ah - Time of Day

**Overall Status**: ✅ **Implemented** (2 functions)

| AH | Function | Status | Implementation | Notes |
|----|----------|--------|----------------|-------|
| 00h | Get tick count | ✅ Implemented | `cpu.rs:2568` int1ah_get_tick_count() | Returns ticks since midnight |
| 01h | Set tick count | ✅ Implemented | `cpu.rs:2586` int1ah_set_tick_count() | Sets tick counter |

**Additional INT 1Ah functions NOT implemented**:
- 02h-07h: Read/set RTC time/date (AT+)
- 09h-0Bh: RTC alarm functions (AT+)

**Analysis**:
- ✅ Tick count functions work correctly
- ⚠️ Missing RTC functions (AT+ systems)

**Recommendation**:
- **Priority: LOW** - Add INT 1Ah AH=02h-07h for RTC support
- **Rationale**: Many DOS programs use tick count; RTC is less common

**Code Impact**: ~10 lines per function (02h-07h)

---

## 4. BIOS Service Interrupts (INT 1Bh-1Fh, 33h, 40h-50h)

### INT 1Bh - Ctrl-Break Handler

| Function | Description | Status | Implementation |
|----------|-------------|--------|----------------|
| - | User-defined handler | ❌ Not intercepted | - |

**Analysis**: 
- ✅ **Correct** - This is meant to be hooked by DOS/programs, not provided by BIOS
- ✅ BIOS sets up vector in `bios.rs:229`, programs hook it

### INT 1Ch - Timer Tick Handler

| Function | Description | Status | Implementation |
|----------|-------------|--------|----------------|
| - | User-defined handler | ❌ Not intercepted | - |

**Analysis**:
- ✅ **Correct** - This is meant to be hooked by programs, not provided by BIOS
- ✅ BIOS sets up vector in `bios.rs:234`, programs hook it
- ✅ INT 08h should CALL INT 1Ch, then handle tick counter

**Recommendation**:
- **Priority: MEDIUM** - Modify INT 08h to call INT 1Ch before incrementing tick count
- **Rationale**: Standard BIOS behavior; programs expect this chain

**Code Impact**: ~5 lines in handle_int08h()

### INT 1Dh-1Fh - Table Pointers

| INT | Description | Status | Implementation |
|-----|-------------|--------|----------------|
| 1Dh | Video parameter table | ❌ Missing | - |
| 1Eh | Diskette parameter table | ✅ Set | `bios.rs:100` (DPT at F000:0250) |
| 1Fh | Graphics character table | ❌ Missing | - |

**Analysis**:
- ✅ Diskette parameter table exists in BIOS ROM
- ⚠️ Table vectors not set in interrupt vector table
- ⚠️ INT 1Dh and 1Fh not initialized

**Recommendation**:
- **Priority: LOW** - Set INT 1Dh and 1Fh vectors to point to tables
- **Action**: Initialize vectors during BIOS startup
- **Rationale**: Some programs query these vectors

**Code Impact**: ~10 lines in bios.rs init code

### INT 33h - Mouse Services

| Function | Description | Status | Implementation |
|----------|-------------|--------|----------------|
| All | Mouse functions | ❌ Not intercepted | - |

**Analysis**:
- ✅ **Correct** - Mouse driver (MOUSE.COM) provides this, not BIOS
- ⚠️ Could add basic stub for driver detection

**Recommendation**: **Priority: LOW** - No action needed (driver's responsibility)

### INT 40h - Relocated Disk Services

| Function | Description | Status | Implementation |
|----------|-------------|--------|----------------|
| - | Floppy disk services | ❌ Missing | - |

**Analysis**: 
- ⚠️ Some BIOSes relocate original INT 13h to INT 40h
- ⚠️ Allows hard disk drivers to hook INT 13h
- ❌ Not implemented in current emulator

**Recommendation**:
- **Priority: LOW** - Add INT 40h as copy of original INT 13h
- **Rationale**: Rarely used; most programs use INT 13h directly

**Code Impact**: ~5 lines (point INT 40h to INT 13h handler)

### INT 41h, 46h - Disk Parameter Table Pointers

| INT | Description | Status | Implementation |
|-----|-------------|--------|----------------|
| 41h | Fixed disk 0 params | ❌ Missing | - |
| 46h | Fixed disk 1 params | ❌ Missing | - |

**Analysis**:
- ⚠️ These should point to hard disk parameter tables
- ❌ Not initialized in current BIOS

**Recommendation**:
- **Priority: MEDIUM** - Initialize INT 41h to point to HDD parameter table
- **Rationale**: Some hard disk utilities query this vector

**Code Impact**: ~20 lines (create HDD parameter table + set vector)

### INT 4Ah - User Alarm

| Function | Description | Status | Implementation |
|----------|-------------|--------|----------------|
| - | RTC alarm (AT+) | ✅ Stub | `cpu.rs:2604` handle_int4ah() |

**Analysis**: ✅ Stub is sufficient (RTC not fully emulated)

### INT 50h - Periodic Alarm

| Function | Description | Status | Implementation |
|----------|-------------|--------|----------------|
| - | Periodic alarm (PS/2) | ❌ Missing | - |

**Analysis**: 
- ⚠️ PS/2-specific, rarely used
- ❌ Not implemented

**Recommendation**: **Priority: LOW** - Add stub if needed

---

## 5. DOS/OS Interrupts (INT 20h-31h) - NOT BIOS Responsibility

These interrupts are provided by DOS or other operating systems. **The emulator correctly does NOT intercept these** (except as fallback for standalone programs).

### INT 20h - Program Termination

| Function | Description | Status | Implementation |
|----------|-------------|--------|----------------|
| - | Terminate program | ⚠️ Fallback only | `cpu.rs:946` (commented out) |

**Analysis**:
- ✅ **Correct** - DOS provides this
- ✅ Fallback handler exists for standalone programs
- ✅ Check for DOS handler before using fallback (line 259-273)

### INT 21h - DOS API

**Overall Status**: ⚠️ **Fallback implementation for standalone programs**

**Analysis**:
- ✅ **Correct architecture** - Checks if DOS has installed handler (line 259-273)
- ✅ If DOS handler exists, CPU executes it normally
- ✅ If no DOS handler, fallback provides basic functions
- ✅ Extensive fallback implementation (30+ functions for standalone programs)

**Implemented Fallback Functions** (only when DOS not present):

| AH | Function | Status | Notes |
|----|----------|--------|-------|
| 00h | Terminate | ✅ Implemented | Fallback for standalone |
| 01h | Read char stdin | ✅ Implemented | Uses INT 16h |
| 02h | Write char stdout | ✅ Implemented | Uses INT 10h |
| 06h | Direct console I/O | ✅ Implemented | Uses INT 16h/10h |
| 07h | Direct stdin | ✅ Implemented | Uses INT 16h |
| 08h | Stdin no echo | ✅ Implemented | Uses INT 16h |
| 09h | Write string | ✅ Implemented | Uses INT 10h |
| 0Ah | Buffered input | ✅ Stub | Returns empty |
| 0Bh | Check stdin | ✅ Implemented | Uses INT 16h |
| 25h | Set interrupt vector | ✅ Stub | Acknowledged |
| 35h | Get interrupt vector | ✅ Stub | Returns 0000:0000 |
| 3Ch | Create file | ✅ Stub | Returns error |
| 3Dh | Open file | ✅ Implemented | Device support (CON, NUL, PRN, COM, LPT, CLOCK$) |
| 3Eh | Close file | ✅ Implemented | Standard handles |
| 3Fh | Read file | ✅ Implemented | Stdin (handle 0), other handles error |
| 40h | Write file | ✅ Implemented | Stdout/stderr (1-2), other handles error |
| 4Ch | Terminate with code | ✅ Implemented | Fallback for standalone |

**Analysis**:
- ✅ **Excellent architecture** - Correctly distinguishes DOS from BIOS
- ✅ Fallback allows standalone COM/EXE files to run without DOS
- ✅ Device name support (CON, NUL, etc.) is well-implemented
- ✅ Standard file handles (0-4) work correctly
- ⚠️ File I/O (open/create/read/write actual files) returns errors (correct for BIOS)

**Recommendation**:
- **Priority: LOW** - Current design is correct
- **Action**: None needed - DOS provides full INT 21h when loaded
- **Rationale**: BIOS should not implement file I/O; that's DOS's job

### INT 22h-24h - DOS Internal

| INT | Description | Status |
|-----|-------------|--------|
| 22h | Terminate address | ✅ Not intercepted (correct) |
| 23h | Ctrl-Break address | ✅ Not intercepted (correct) |
| 24h | Critical error handler | ✅ Not intercepted (correct) |

**Analysis**: ✅ **Correct** - These are DOS-internal, BIOS should not touch them

### INT 25h-27h - DOS Disk Services

| INT | Description | Status |
|-----|-------------|--------|
| 25h | Absolute disk read | ✅ Not intercepted (correct) |
| 26h | Absolute disk write | ✅ Not intercepted (correct) |
| 27h | TSR | ✅ Not intercepted (correct) |

**Analysis**: ✅ **Correct** - DOS provides these, not BIOS

### INT 28h - DOS Idle Loop

| Function | Description | Status | Implementation |
|----------|-------------|--------|----------------|
| - | DOS idle callout | ✅ Stub | `cpu.rs:274` handle_int28h() |

**Analysis**:
- ⚠️ This is a DOS-internal interrupt
- ✅ Stub returns immediately (correct behavior)

### INT 29h - Fast Console Output

| Function | Description | Status | Implementation |
|----------|-------------|--------|----------------|
| - | Fast console I/O | ✅ Implemented | `cpu.rs:2615` handle_int29h() |

**Analysis**:
- ⚠️ This is typically provided by DOS
- ✅ Implementation uses INT 10h teletype (correct fallback)
- ✅ Allows standalone programs to output quickly

### INT 2Ah - Network Installation API

| Function | Description | Status | Implementation |
|----------|-------------|--------|----------------|
| All | Network API | ✅ Stub | `cpu.rs:276` handle_int2ah() |

**Analysis**: 
- ✅ Stub is correct (network redirector provides this)
- ✅ Returns AL=0xFF (not installed)

### INT 2Fh - Multiplex

| Function | Description | Status | Implementation |
|----------|-------------|--------|----------------|
| All | Multiplex | ❌ Not intercepted | - |

**Analysis**:
- ✅ **Correct** - DOS and TSRs provide this
- ⚠️ Could add stub for XMS/HIMEM detection (AH=43h)

**Recommendation**:
- **Priority: LOW** - Add INT 2Fh stub for XMS detection
- **Rationale**: Some programs check for XMS before loading
- **Code Impact**: ~10 lines

### INT 31h - DPMI

| Function | Description | Status | Implementation |
|----------|-------------|--------|----------------|
| All | DPMI services | ❌ Not intercepted | - |

**Analysis**: ✅ **Correct** - DPMI host provides this, not BIOS

---

## 6. Windows Interrupts (INT 2Fh extensions, 30h, 31h) - NOT BIOS Responsibility

**Analysis**: ✅ **Correct** - Windows provides these, emulator correctly does not intercept them

---

## 7. Summary of Findings

### ✅ **Correctly Implemented** (Architecture is Sound)

1. **DOS/OS Separation**: Emulator correctly distinguishes between BIOS and DOS responsibilities
2. **INT 10h (Video)**: Comprehensive implementation with 18 functions
3. **INT 13h (Disk)**: Excellent CHS support with proper geometry handling
4. **INT 16h (Keyboard)**: Full keyboard services with shift/AltGr support
5. **INT 11h/12h**: Equipment and memory detection work correctly
6. **INT 21h Fallback**: Smart fallback for standalone programs, defers to DOS when present

### ⚠️ **Needs Improvement**

1. **INT 1Ch Chain**: INT 08h should call INT 1Ch for user timer hook (documented but not yet chained)
2. **INT 15h (System)**: Missing wait function, extended memory, and A20 gate
3. **INT 41h/46h**: Missing hard disk parameter table pointers
4. **INT 70h-77h**: Missing AT+ IRQ handlers (stubs needed)
5. **CPU Exceptions**: Missing handlers for INT 01h-04h, 06h-10h (low priority)

### ❌ **Missing but Low Priority**

1. **INT 10h Extensions**: AH=1Bh (video state), advanced VGA functions
2. **INT 13h LBA**: AH=42h-48h (extended read/write for large disks)
3. **INT 1Ah RTC**: AH=02h-07h (read/set RTC time/date)
4. **INT 2Fh Stub**: XMS detection (AH=43h)
5. **INT 1Dh/1Fh**: Video and graphics character table pointers

---

## 8. Recommendations by Priority

### 🔴 **HIGH Priority** (Breaks Real-World Software)

**Based on MS-DOS 5.0 and QBasic testing:**

1. **INT 15h AH=24h**: Implement A20 gate control
   - **Impact**: **CRITICAL** - HIMEM.SYS fails to load without this, preventing extended memory access
   - **Effort**: ~20 lines of code
   - **Files**: `cpu.rs` (add int15h_a20_gate function)
   - **Functions needed**: 
     - AL=00h: Disable A20 (return success)
     - AL=01h: Enable A20 (return success)
     - AL=02h: Get A20 status (return enabled)
     - AL=03h: Get A20 gate support (return supported)

2. **INT 10h AH=0Bh**: Implement set color palette
   - **Impact**: **CRITICAL** - Many DOS applications including QBasic rely on palette control
   - **Effort**: ~15 lines of code
   - **Files**: `cpu.rs` (add int10h_set_color_palette function)
   - **Functions needed**:
     - BH=00h: Set background/border color (BL=color)
     - BH=01h: Set palette (BL=palette ID)

3. **INT 10h AH=1Bh**: Implement get video state
   - **Impact**: **CRITICAL** - QBasic and other applications query video capabilities
   - **Effort**: ~30 lines of code
   - **Files**: `cpu.rs` (add int10h_get_video_state function)
   - **Returns**: ES:DI = pointer to video state structure

4. **INT 10h AH=EFh, FAh**: Add stub handlers for undocumented functions
   - **Impact**: Prevents "unsupported subfunction" errors in QBasic
   - **Effort**: ~5 lines each
   - **Files**: `cpu.rs` (add to handle_int10h match statement)
   - **Action**: Return immediately (stub/no-op)

5. **Fix BIOS Architecture/Model Byte Inconsistency**
   - **Impact**: **CRITICAL** - Ensures proper system identification for DOS and applications
   - **Effort**: ~60 lines of code
   - **Files**: 
     - `bios.rs:42` (make generate_minimal_bios accept cpu_model parameter)
     - `bios.rs:124` (system config table model byte - match CPU)
     - `bios.rs:271` (BIOS model byte at FFFE - match CPU)
     - `lib.rs:105` (pass cpu_model to generate_minimal_bios)
   - **Functions needed**:
     - Map CPU model to appropriate PC architecture (PC/XT/AT/PS2)
     - Set model byte: 0xFE for 8086/8088, 0xFC for 286+, 0xF8 for 386+
     - Set feature bytes to match architecture (RTC, 2nd PIC for AT+)
   - **Rationale**: Software checks BIOS model to determine available features

### 🟠 **MEDIUM Priority** (Improves Compatibility)

1. **INT 08h Enhancement**: Chain to INT 1Ch for user timer hook
   - **Impact**: Programs hooking INT 1Ch will receive timer ticks
   - **Effort**: ~5 lines of code
   - **Files**: `cpu.rs:1503` (handle_int08h) - Add INT 1Ch call
   - **Note**: Tick counter already properly implemented at 0040:006C

2. **INT 15h AH=86h**: Implement wait function properly
   - **Impact**: Programs using delay will work correctly
   - **Effort**: ~10 lines
   - **Files**: `cpu.rs:2433` (int15h_wait)

3. **INT 41h/46h**: Add hard disk parameter table pointers
   - **Impact**: Hard disk utilities will detect drive correctly
   - **Effort**: ~20 lines
   - **Files**: `bios.rs` (add table), `bios.rs` (set vectors)

4. **INT 13h AH=42h**: Implement basic LBA read
   - **Impact**: Support for large disks (>8GB)
   - **Effort**: ~40 lines
   - **Files**: `cpu.rs:2360` (int13h_extended_read)

5. **Keyboard Scan Code Set Support (AT/PS2 systems)**
   - **Impact**: Improves AT/PS2 compatibility, some software checks keyboard type
   - **Effort**: ~50 lines
   - **Files**: `keyboard.rs` (add scan code set 2 translation)
   - **Rationale**: AT and PS/2 systems default to scan code set 2
   - **Note**: Current set 1 works for most DOS software

### 🟡 **LOW Priority** (Nice to Have)

1. **INT 70h-77h**: Add AT+ IRQ stubs
2. **INT 16h AH=10h-12h**: Add extended keyboard functions
3. **INT 1Ah AH=02h-07h**: Add RTC functions
4. **INT 2Fh**: Add XMS detection stub
5. **CPU Exceptions**: Add logging for INT 01h-04h, 06h-10h
6. **INT 10h AH=04h**: Read light pen position (rarely used)

---

## 9. Code Quality Assessment

### Strengths

1. ✅ **Clean Architecture**: Clear separation of BIOS vs. DOS responsibilities
2. ✅ **Comprehensive Testing**: 136 unit tests cover interrupt functions
3. ✅ **Good Documentation**: Function comments explain BIOS behavior
4. ✅ **Proper Register Handling**: Correct preservation/modification of registers
5. ✅ **Error Handling**: Carry flag and error codes set correctly

### Areas for Improvement

1. ⚠️ **Consistency**: Some functions are stubs, others fully implemented
2. ⚠️ **Logging**: Stub interrupt calls could log AH function code for debugging
3. ⚠️ **BIOS Data Area**: Some functions don't update BIOS data area (e.g., tick count)

---

## 10. Conclusion

**Overall Assessment**: ✅ **Architecture is fundamentally correct**

The PC emulator's interrupt handling follows best practices by:
- Implementing BIOS-level services (INT 10h, 13h, 16h)
- Leaving OS-level services to DOS (INT 21h, 2Fh)
- Providing smart fallback for standalone programs
- Correctly handling hardware interrupts (INT 08h, 09h)

**Most Important Improvements** (Based on Real-World Testing):
1. **INT 15h AH=24h**: A20 gate control (HIGH priority) - **REQUIRED for HIMEM.SYS**
2. **INT 10h AH=0Bh**: Set color palette (HIGH priority) - **REQUIRED for QBasic and many DOS apps**
3. **INT 10h AH=1Bh**: Get video state (HIGH priority) - **REQUIRED for QBasic**
4. **INT 10h AH=EFh, FAh**: Undocumented VGA stubs (HIGH priority) - **REQUIRED for QBasic**
5. **BIOS Architecture Consistency**: Fix model byte mismatch (HIGH priority) - **REQUIRED for proper system identification**
6. INT 08h: Chain to INT 1Ch (MEDIUM priority) - tick counter already works
7. INT 15h AH=86h: Wait function (MEDIUM priority)
8. INT 41h: Hard disk parameter table (MEDIUM priority)
9. Keyboard scan code set 2 (MEDIUM priority) - for AT/PS2 compatibility

**Estimated Total Effort**: 
- HIGH priority (critical for DOS 5.0/QBasic and system identification): ~135 lines of code
  - Interrupt functions: ~75 lines
  - Architecture/model byte fixes: ~60 lines
- MEDIUM priority improvements: ~130 lines of code
  - Interrupt enhancements: ~80 lines
  - Keyboard scan code set 2: ~50 lines
- **Total**: ~265 lines

**Risk Assessment**: 🟢 **LOW** - Changes are isolated and well-understood

**Testing Notes**: Analysis updated based on real-world testing with:
- MS-DOS 5.0 boot sequence (HIMEM.SYS failure)
- QBasic application (video function failures)
- Architecture verification (model byte inconsistency found)

---

## Appendix A: Interrupt Vector Table Layout

Standard PC BIOS interrupt vector table (first 256 interrupts):

```
0000:0000  INT 00h  Divide by zero exception
0000:0004  INT 01h  Single step exception
0000:0008  INT 02h  NMI
0000:000C  INT 03h  Breakpoint
0000:0010  INT 04h  Overflow exception
0000:0014  INT 05h  BOUND exception / Print screen
...
0000:0020  INT 08h  IRQ0 Timer tick
0000:0024  INT 09h  IRQ1 Keyboard
0000:0028  INT 0Ah  IRQ2 Cascade
0000:002C  INT 0Bh  IRQ3 Serial COM2/4
0000:0030  INT 0Ch  IRQ4 Serial COM1/3
0000:0034  INT 0Dh  IRQ5 LPT2/HDD
0000:0038  INT 0Eh  IRQ6 Floppy
0000:003C  INT 0Fh  IRQ7 LPT1
0000:0040  INT 10h  Video BIOS
0000:0044  INT 11h  Equipment list
0000:0048  INT 12h  Memory size
0000:004C  INT 13h  Disk BIOS
0000:0050  INT 14h  Serial port
0000:0054  INT 15h  System services
0000:0058  INT 16h  Keyboard BIOS
0000:005C  INT 17h  Printer
0000:0060  INT 18h  Boot failure
0000:0064  INT 19h  Bootstrap
0000:0068  INT 1Ah  Time of day
0000:006C  INT 1Bh  Ctrl-Break
0000:0070  INT 1Ch  Timer tick user
0000:0074  INT 1Dh  Video params pointer
0000:0078  INT 1Eh  Disk params pointer
0000:007C  INT 1Fh  Graphics chars pointer
0000:0080  INT 20h  DOS terminate
0000:0084  INT 21h  DOS API
...
0000:01C0  INT 70h  IRQ8 RTC (AT+)
0000:01C4  INT 71h  IRQ9 Redirect
0000:01C8  INT 72h  IRQ10 Reserved
0000:01CC  INT 73h  IRQ11 Reserved
0000:01D0  INT 74h  IRQ12 Mouse
0000:01D4  INT 75h  IRQ13 Math coproc
0000:01D8  INT 76h  IRQ14 HDD controller
0000:01DC  INT 77h  IRQ15 Reserved
```

## Appendix B: BIOS Data Area Layout

Important BIOS data area locations (segment 0040h):

```
0040:0000  COM port addresses (8 bytes)
0040:0008  LPT port addresses (8 bytes)
0040:0010  Equipment flags (2 bytes)
0040:0013  Memory size in KB (2 bytes)
0040:001A  Keyboard buffer head pointer
0040:001C  Keyboard buffer tail pointer
0040:001E  Keyboard buffer (32 bytes)
0040:003E  Floppy drive calibration
0040:0040  Floppy motor status
0040:0049  Video mode (1 byte)
0040:004A  Columns (2 bytes)
0040:004E  Video page offset (2 bytes)
0040:0050  Cursor positions (16 bytes, 8 pages)
0040:0060  Cursor shape (2 bytes)
0040:0062  Active video page (1 byte)
0040:0063  CRT controller base address (2 bytes)
0040:006C  Timer tick count (4 bytes)
0040:0070  Timer overflow flag (1 byte)
0040:0071  Break flag (1 byte)
0040:0074  Disk status (1 byte)
0040:0075  Number of hard disks (1 byte)
0040:0078  LPT timeout (4 bytes)
0040:007C  COM timeout (4 bytes)
0040:0080  Keyboard buffer start offset (2 bytes)
0040:0082  Keyboard buffer end offset (2 bytes)
```

---

**End of Analysis**
