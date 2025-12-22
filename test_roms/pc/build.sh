#!/bin/bash
# Build script for custom PC BIOS

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

echo "Building custom PC BIOS..."

# Assemble BIOS
nasm -f bin -o bios.bin bios.asm

# Check that the BIOS is exactly 64KB
SIZE=$(stat -c%s bios.bin 2>/dev/null || stat -f%z bios.bin 2>/dev/null)
EXPECTED=65536

if [ "$SIZE" -ne "$EXPECTED" ]; then
    echo "Error: BIOS size is $SIZE bytes, expected $EXPECTED bytes"
    exit 1
fi

echo "BIOS built successfully: bios.bin (64KB)"
echo "BIOS can be loaded into slot 1 (replaceable)"
