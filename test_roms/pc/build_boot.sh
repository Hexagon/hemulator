#!/bin/bash
# Build script for PC boot sector test ROM

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
    echo "Successfully created boot.bin ($(stat -c%s boot.bin) bytes)"
    
    # Verify boot signature
    if hexdump -C boot.bin | tail -1 | grep -q "55 aa"; then
        echo "Boot signature verified: 0xAA55"
    else
        echo "Warning: Boot signature not found!"
    fi
else
    echo "Error: Assembly failed"
    exit 1
fi
