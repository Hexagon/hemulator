# IBM PC/XT Test ROMs

This directory contains test ROMs and BIOS for the IBM PC/XT emulator.

## Directory Structure

Each test ROM has its own subdirectory with source, build script, and binaries:

- **basic_boot/** - Simple boot sector test (writes "BOOT OK" to screen)
- **menu/** - Interactive menu test (keyboard, video, basic operations)
- **fileio/** - File I/O test (demonstrates INT 21h file operations)
- **comprehensive_boot/** - Comprehensive boot test (CPU, memory, disk I/O, program loading)

## Test ROMs

### 1. Basic Boot Test (`basic_boot/`)

**File:** `boot.bin` (512 bytes)

A minimal bootable boot sector that writes "BOOT OK" to the screen in green text.

**Building:**
```bash
cd basic_boot
./build.sh
```

**Testing:**
The boot sector is used in the smoke test `test_boot_sector_smoke_test` which:
1. Creates a 1.44MB floppy image with the boot sector
2. Boots from the floppy
3. Verifies that "BOOT OK" is written to video memory

**Usage:**
- Load `test_floppy.img` in the emulator to see basic boot functionality

### 2. Interactive Menu Test (`menu/`)

**File:** `menu.bin` (512 bytes)

An interactive bootable boot sector with a menu for manual testing of various features.

**Building:**
```bash
cd menu
./build.sh
```

**Features:**
1. Prints "BOOT OK" on startup
2. Runs memory test and prints "MEM OK" (or "MEM FAIL" if failed)
   - Tests writing and reading a pattern (0xAA55) to memory at 0x1000
3. Runs CPU test and prints "CPU OK" (or "CPU FAIL" if failed)
   - Tests basic addition (2+2=4)
   - Tests XOR operation
4. Displays an interactive menu:
   - Test user input (keyboard echo)
   - Calculate 2+2 (arithmetic test)
   - Test file I/O (read/write simulation)
   - Quit option
5. Uses INT 10h (video services) for display
6. Uses INT 16h (keyboard services) for input

**Usage:**
- Load `menu_floppy.img` in the emulator
- Press F3 in the emulator and select the image file
- Select menu options by pressing 1, 2, 3, or Q

**Testing Features:**
- **Option 1 (User Input)**: Type any text and see it echoed on screen. Press ESC to return to menu.
- **Option 2 (Calculate 2+2)**: Demonstrates basic arithmetic (adds 2+2 and displays result: 4)
- **Option 3 (File I/O)**: Simulates file read/write operations with status messages
- **Option Q (Quit)**: Halts the system with a goodbye message

### 3. File I/O Test (`fileio/`)

**File:** `fileio_test.asm`

A bootloader that demonstrates INT 21h file operations (DOS API).

**Building:**
```bash
cd fileio
./build.sh
```

**Features:**
- Attempts to open files (IO.SYS, MSDOS.SYS)
- Demonstrates file reading
- Demonstrates file creation and writing
- Shows error codes for failed operations

**Note:** This test requires a DOS filesystem on the disk. Without DOS, it will show error codes but demonstrates the API usage.

### 4. Comprehensive Boot Test (`comprehensive_boot/`)

**File:** `comprehensive_boot.bin` (512 bytes)

A thorough boot sector test that replicates the DOS boot process and helps diagnose boot-related issues.

**Building:**
```bash
cd comprehensive_boot
./build.sh
```

**Features:**

**CPU Tests:**
- Basic arithmetic (ADD, SUB)
- Logical operations (AND, OR, XOR)
- Shift operations (SHL, SHR)

**Memory Tests:**
- Read/write at various addresses
- Pattern fill and verify (0x5AA5)
- Sequential pattern testing

**Disk I/O Tests:**
- Disk reset (INT 13h, AH=00h)
- Read multiple sectors (sectors 2-5)
- Multi-track reads (head 0 and 1)
- Simulates DOS boot sector reads

**Program Loading Test:**
- Multi-sector consecutive reads (5 sectors)
- Simulates loading IO.SYS/MSDOS.SYS from disk
- Tests sector advancement and buffer management

**Interactive Prompt:**
- If all tests pass, displays "BOOT>" prompt
- Accepts keyboard input
- Type 'q' or 'Q' to quit

**Usage:**
- Load `comprehensive_boot.img` in the emulator
- Tests run automatically on boot
- Each test displays "OK" or "FAIL"
- Successful boot reaches the "BOOT>" prompt

**Purpose:**
This test is designed to help diagnose the FreeDOS/MS-DOS freeze issue by:
1. Replicating the DOS boot process (disk reads, multi-sector loading)
2. Testing all CPU operations used during boot
3. Verifying memory operations
4. Simulating the IO.SYS/MSDOS.SYS loading sequence

**Known Issue:**
Both FreeDOS and MS-DOS currently freeze during boot in the emulator. The comprehensive boot test helps isolate where the freeze occurs by testing each component independently.

## Boot Process

The PC emulator boot process:

1. CPU starts at 0xFFFF:0x0000 (BIOS entry point)
2. BIOS initializes segments (DS, ES, SS) to 0x0000
3. BIOS sets stack pointer (SP) to 0xFFFE
4. Emulator loads boot sector (sector 0, 512 bytes) from disk to 0x0000:0x7C00
5. Emulator validates boot signature (0xAA55) at offset 510-511
6. BIOS jumps to 0x0000:0x7C00
7. Boot sector code executes

## Boot Priority

The emulator supports configurable boot priority:
- **FloppyFirst** (default): Try floppy A, then hard drive C
- **HardDriveFirst**: Try hard drive C, then floppy A
- **FloppyOnly**: Only try floppy A
- **HardDriveOnly**: Only try hard drive C

Set boot priority in .hemu project files or via the API.

## .hemu Project Files

For PC systems with multiple disk images, you can create a `.hemu` project file to configure all mount points and boot priority. Example:

```json
{
  "version": 1,
  "system": "pc",
  "mounts": {
    "FloppyA": "dos622_boot.img",
    "HardDrive": "freedos.img"
  },
  "boot_priority": "FloppyFirst"
}
```

Note: BIOS mount is optional. If not specified, the built-in generated BIOS is used. Custom BIOS ROMs can be loaded with `"BIOS": "custom_bios.bin"` if needed.

**Boot Priority Options:**
- `FloppyFirst` - Boot from floppy A first, then hard drive C (default)
- `HardDriveFirst` - Boot from hard drive C first, then floppy A
- `FloppyOnly` - Only boot from floppy A
- `HardDriveOnly` - Only boot from hard drive C

**Loading a Project:**
1. Press F3 in the emulator
2. Select your `.hemu` file
3. All disks will be mounted and boot priority will be set
4. System will reset and boot from the configured disk

See `example.hemu` for a template.

## Mount Points

The PC emulator supports the following mount points:

1. **BIOS** (Slot 1)
   - Extensions: `.bin`, `.rom`
   - Required: No (has built-in BIOS)
   - Default: Generated minimal BIOS (see `crates/systems/pc/src/bios.rs`)
   - Note: Custom BIOS ROMs can be loaded if needed

2. **FloppyA** (Floppy Drive A:)
   - Extensions: `.img`, `.ima`
   - Required: No
   - Format: Raw disk image (360KB, 720KB, 1.44MB, or 2.88MB)

3. **FloppyB** (Floppy Drive B:)
   - Extensions: `.img`, `.ima`
   - Required: No
   - Format: Raw disk image

4. **HardDrive** (Hard Drive C:)
   - Extensions: `.img`, `.vhd`
   - Required: No
   - Format: Raw disk image or VHD

## Creating Custom Boot Sectors

To create your own boot sector:

1. Write 16-bit x86 assembly code
2. Assemble to a flat binary: `nasm -f bin yourboot.asm -o yourboot.bin`
3. Ensure the file is exactly 512 bytes
4. Ensure bytes 510-511 contain the boot signature `0x55 0xAA`

Example minimal structure:
```asm
BITS 16
ORG 0x7C00
start:
    ; Your boot code here
    cli
    hlt
times 510-($-$$) db 0
dw 0xAA55    ; Boot signature
```

## Known Issues

### FreeDOS/MS-DOS Boot Freeze

Both FreeDOS and MS-DOS currently freeze during boot in an infinite loop.

**Root Cause:**
The FreeDOS boot code gets stuck in an infinite loop at address 12CE:000F-0017. Detailed tracing shows:

```
000F: A4          MOVSB       (move byte from DS:SI to ES:DI)
0010: 00 FF       ADD [BX+DI], BH
0012: 75 03       JNZ +3      (jump to 0017 if not zero)
0017: 72 F6       JC -10      (jump to 000F if carry)
```

The loop condition: `000F → 0010 → 0012 → 0017 → 000F` repeats indefinitely because:
1. The carry flag (CF) remains set
2. The ADD instruction at 0010 doesn't produce a zero result
3. Both branch conditions remain true, creating an infinite loop

**This is NOT a division by zero issue** - the loop involves string operations (MOVSB) and arithmetic (ADD), not division. The original diagnosis was incorrect.

**Status:** Under investigation. The issue exists in the FreeDOS boot code itself and is not caused by the emulator's division error handling. Real x86 hardware would likely exhibit similar behavior with this boot sector.

**Debug Output:**
When running with trace logging, you'll see the CPU stuck looping through addresses 12CE:000F-0017 repeatedly.

**Workaround:**
Use the custom boot sectors in this directory instead of FreeDOS/MS-DOS:
- `basic_boot/` - Simple boot test
- `menu/` - Interactive menu
- `comprehensive_boot/` - Full diagnostic boot test

The comprehensive boot test was created to help isolate boot issues by testing:
- CPU operations that DOS would use
- Memory operations
- Disk I/O patterns similar to DOS
- Multi-sector reads like IO.SYS loading

## Testing

Each test ROM can be built and run independently:

```bash
# Build and test basic boot
cd basic_boot
./build.sh
# Load test_floppy.img in emulator

# Build and test menu
cd ../menu
./build.sh
# Load menu_floppy.img in emulator

# Build and test comprehensive boot
cd ../comprehensive_boot
./build.sh
# Load comprehensive_boot.img in emulator
```

The emulator includes automated tests for the basic boot sector in `crates/systems/pc/src/lib.rs`:
- `test_boot_sector_smoke_test` - Verifies basic boot functionality

## Requirements

- NASM assembler (`sudo apt-get install nasm`)
- Tools: `dd`, `hexdump` (standard on Linux/Unix systems)

## Future Enhancements

- Additional test ROMs for specific DOS functions
- Tests for protected mode operations
- Tests for video mode switching
- Tests for INT 21h DOS API functions
- Automated test suite for comprehensive boot test
