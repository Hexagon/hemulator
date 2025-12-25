#!/bin/bash
# Build script for PC menu test ROM

# Check if NASM is installed
if ! command -v nasm &> /dev/null; then
    echo "Error: NASM assembler is not installed"
    echo "Install with: sudo apt-get install nasm"
    exit 1
fi

# Assemble the menu boot sector
echo "Assembling menu.asm..."
nasm -f bin menu.asm -o menu.bin

if [ $? -eq 0 ]; then
    echo "Successfully created menu.bin ($(stat -c%s menu.bin 2>/dev/null || stat -f%z menu.bin 2>/dev/null) bytes)"
    
    # Verify boot signature
    if hexdump -C menu.bin | tail -1 | grep -q "55 aa"; then
        echo "Boot signature verified: 0xAA55"
    else
        echo "Warning: Boot signature not found!"
    fi
    
    # Create a bootable floppy image with the menu boot sector
    echo "Creating bootable floppy image: menu_floppy.img..."
    dd if=menu.bin of=menu_floppy.img bs=512 count=1 2>/dev/null
    dd if=/dev/zero bs=512 count=2879 >> menu_floppy.img 2>/dev/null
    
    echo "Successfully created menu_floppy.img (1.44MB floppy image)"
    echo "You can load this in the emulator to test the interactive menu"
else
    echo "Error: Assembly failed"
    exit 1
fi
