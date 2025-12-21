#!/bin/bash
# Build script for Game Boy Color test ROM

set -e

# Check if rgbasm is installed
if ! command -v rgbasm &> /dev/null; then
    echo "Error: rgbasm not found. Please install rgbds:"
    echo "  Ubuntu/Debian: build from source (see test_roms/README.md)"
    echo "  macOS: brew install rgbds"
    exit 1
fi

echo "Building GBC test ROM..."

# Assemble
rgbasm -o test.o test.asm

# Link
rgblink -o test.gb test.o

# Fix header checksums
rgbfix -v -p 0xFF test.gb

# Cleanup
rm test.o

echo "Build complete: test.gb"
