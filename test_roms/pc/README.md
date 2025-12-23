# IBM PC/XT Test ROMs

This directory contains test ROMs and BIOS for the IBM PC/XT emulator.

## Boot Sector Test (boot.bin)

### File: `boot.bin` (512 bytes)

A minimal bootable boot sector that writes "BOOT OK" to the screen in green text.

**Building:**
```bash
./build_boot.sh
```

**Requirements:**
- NASM assembler (`sudo apt-get install nasm`)

**Testing:**
The boot sector is used in the smoke test `test_boot_sector_smoke_test` which:
1. Creates a 1.44MB floppy image with the boot sector
2. Boots from the floppy
3. Verifies that "BOOT OK" is written to video memory

**Usage:**
- Create a bootable floppy image: 
  ```bash
  dd if=boot.bin of=test_floppy.img bs=512 count=1 && dd if=/dev/zero bs=512 count=2879 >> test_floppy.img
  ```
- Load the floppy image in the emulator to test boot functionality

### Creating Custom Boot Sectors

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

## Custom BIOS (bios.bin)

### File: `bios.bin` (64KB)

A minimal BIOS ROM for the IBM PC/XT emulator.

**Building:**
```bash
./build.sh
```

**Features:**
- Entry point at 0xFFFF:0x0000 (physical 0xFFFF0)
- Segment and stack initialization
- Boot sector loading from floppy/hard drive
- Boot signature validation (0xAA55)
- Jump to loaded boot sector at 0x0000:0x7C00

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
    "BIOS": "custom_bios.bin",
    "FloppyA": "dos622_boot.img",
    "HardDrive": "freedos.img"
  },
  "boot_priority": "FloppyFirst"
}
```

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
   - Required: No (has default)
   - Default: Custom BIOS (this file)

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

## Future Enhancements

- Full INT 13h implementation for actual disk I/O
- Boot sector loading and execution
- INT 10h (Video Services)
- INT 16h (Keyboard Services)
- INT 19h (Bootstrap Loader)
