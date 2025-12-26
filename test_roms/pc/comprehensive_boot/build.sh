#!/bin/bash
# Build script for comprehensive PC boot test ROM

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# Check if NASM is installed
if ! command -v nasm &> /dev/null; then
    echo "Error: NASM assembler is not installed"
    echo "Install with: sudo apt-get install nasm"
    exit 1
fi

echo "Building comprehensive PC boot test..."

# Assemble the boot sector
echo "Assembling comprehensive_boot.asm..."
nasm -f bin comprehensive_boot.asm -o comprehensive_boot.bin

if [ $? -eq 0 ]; then
    SIZE=$(stat -c%s comprehensive_boot.bin 2>/dev/null || stat -f%z comprehensive_boot.bin 2>/dev/null)
    echo "Successfully created comprehensive_boot.bin ($SIZE bytes)"
    
    # Verify boot signature
    if hexdump -C comprehensive_boot.bin | tail -1 | grep -q "55 aa"; then
        echo "Boot signature verified: 0xAA55"
    else
        echo "Warning: Boot signature not found!"
    fi
    
    # Create a bootable 1.44MB floppy image
    echo "Creating bootable floppy image: comprehensive_boot.img..."
    
    # Create blank 1.44MB image (2880 sectors * 512 bytes)
    dd if=/dev/zero of=comprehensive_boot.img bs=512 count=2880 status=none
    
    # Write boot sector
    dd if=comprehensive_boot.bin of=comprehensive_boot.img bs=512 count=1 conv=notrunc status=none
    
    # Fill sectors 2-20 with test data (simulating files on disk)
    # This allows the disk read tests to succeed
    echo -n "TEST_DATA_SECTOR_02" | dd of=comprehensive_boot.img bs=512 seek=1 conv=notrunc status=none 2>/dev/null
    echo -n "TEST_DATA_SECTOR_03" | dd of=comprehensive_boot.img bs=512 seek=2 conv=notrunc status=none 2>/dev/null
    echo -n "TEST_DATA_SECTOR_04" | dd of=comprehensive_boot.img bs=512 seek=3 conv=notrunc status=none 2>/dev/null
    echo -n "TEST_DATA_SECTOR_05" | dd of=comprehensive_boot.img bs=512 seek=4 conv=notrunc status=none 2>/dev/null
    
    # Add data to additional sectors for multi-sector read test
    for i in {5..20}; do
        echo -n "TEST_DATA_SECTOR_$(printf '%02d' $i)" | dd of=comprehensive_boot.img bs=512 seek=$((i-1)) conv=notrunc status=none 2>/dev/null
    done
    
    echo "Successfully created comprehensive_boot.img (1.44MB floppy image)"
    echo ""
    echo "Usage:"
    echo "  1. Load comprehensive_boot.img in the PC emulator"
    echo "  2. System will boot and run all tests automatically"
    echo "  3. Tests include: CPU, Memory, Disk I/O, Program Loading"
    echo "  4. If all tests pass, you'll see 'BOOT>' prompt"
    echo "  5. Type 'q' or 'Q' to quit"
else
    echo "Error: Assembly failed"
    exit 1
fi
