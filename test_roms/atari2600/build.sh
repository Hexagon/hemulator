#!/bin/bash
# Build script for Atari 2600 test ROMs

set -e

echo "Building Atari 2600 test ROMs..."

# Build the basic test ROM
echo "  Building test.bin..."
dasm test.asm -f3 -otest.bin

# Build the checkerboard test ROM
echo "  Building checkerboard.bin..."
dasm checkerboard.asm -f3 -ocheckerboard.bin

# Build the timer test ROM (if it doesn't already exist as .bin)
if [ -f "test_timer.asm" ]; then
    echo "  Building test_timer.bin..."
    dasm test_timer.asm -f3 -otest_timer.bin
fi

echo "Atari 2600 test ROMs built successfully:"
ls -lh *.bin
