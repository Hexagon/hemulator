# IBM PC/XT Test ROMs

This directory contains test ROMs and a custom BIOS for the IBM PC/XT emulator.

## Custom BIOS

### File: `bios.bin` (64KB)

A minimal BIOS ROM for the IBM PC/XT emulator with the following features:

- **Size**: 64KB (standard BIOS size)
- **Entry Point**: 0xFFFF:0x0000 (physical address 0xFFFF0)
- **BIOS Date**: 12/22/24 (at offset 0xFFF5)
- **System Model**: 0xFE (PC XT model byte at offset 0xFFFE)

### Features:

- Basic interrupt vector setup
- INT 13h (Disk Services) stub - returns success for all operations
- Proper segment initialization
- Stack setup at 0x0000:0xFFFE

### Building from Source:

```bash
./build.sh
```

Requirements:
- NASM (Netwide Assembler)

The build script assembles `bios.asm` into `bios.bin` and verifies it's exactly 64KB.

### Usage:

The BIOS is loaded by default when the PC system starts. It can be replaced by mounting
a custom BIOS binary to the "BIOS" mount point:

```rust
let custom_bios = std::fs::read("custom_bios.bin")?;
pc_system.mount("BIOS", &custom_bios)?;
```

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
