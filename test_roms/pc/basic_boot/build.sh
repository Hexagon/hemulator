#!/bin/bash
# Build script for PC boot sector test ROM

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# Check if NASM is installed
if ! command -v nasm &> /dev/null; then
    echo "Error: NASM assembler is not installed"
    echo "Install with: sudo apt-get install nasm"
    exit 1
fi

# Assemble the boot sector
echo "Assembling boot.asm..."
nasm -f bin boot.asm -o boot.bin

if [ $? -eq 0 ]; then
    SIZE=$(stat -c%s boot.bin 2>/dev/null || stat -f%z boot.bin 2>/dev/null)
    echo "Successfully created boot.bin ($SIZE bytes)"
    
    # Verify boot signature
    if hexdump -C boot.bin | tail -1 | grep -q "55 aa"; then
        echo "Boot signature verified: 0xAA55"
    else
        echo "Warning: Boot signature not found!"
    fi
    
    # Create a simple floppy image for testing
    echo "Creating test_floppy.img..."
    dd if=boot.bin of=test_floppy.img bs=512 count=1 2>/dev/null
    dd if=/dev/zero bs=512 count=2879 >> test_floppy.img 2>/dev/null
    echo "Created test_floppy.img (1.44MB)"
else
    echo "Error: Assembly failed"
    exit 1
fi
