#!/bin/bash
# Build script for file I/O test bootloader

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

echo "Building file I/O test bootloader..."

# Assemble the bootloader
nasm -f bin -o fileio_test.bin fileio_test.asm

# Check that it's exactly 512 bytes
SIZE=$(stat -c%s fileio_test.bin 2>/dev/null || stat -f%z fileio_test.bin 2>/dev/null)
EXPECTED=512

if [ "$SIZE" -ne "$EXPECTED" ]; then
    echo "Error: Boot sector size is $SIZE bytes, expected $EXPECTED bytes"
    exit 1
fi

# Create a blank 1.44MB floppy image
dd if=/dev/zero of=fileio_test.img bs=512 count=2880 status=none

# Write the boot sector to the image
dd if=fileio_test.bin of=fileio_test.img bs=512 count=1 conv=notrunc status=none

# Create sample files on the disk (we'll need to create these manually for now)
# For testing, we can create a simple FAT12 filesystem with sample files

echo "File I/O test built successfully:"
echo "  - fileio_test.bin (512 bytes boot sector)"
echo "  - fileio_test.img (1.44MB floppy with boot sector)"
echo ""
echo "To test: Load fileio_test.img into FloppyA slot"
