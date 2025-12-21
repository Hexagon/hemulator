#!/bin/bash
# Build script for Game Boy test ROM

set -e

echo "Building Game Boy test ROM..."

# Assemble
rgbasm -o test.o test.asm

# Link
rgblink -o test.gb test.o

# Fix header checksums
rgbfix -v -p 0 test.gb

# Cleanup
rm -f test.o

echo "Game Boy test ROM built: test.gb"
ls -lh test.gb
