#!/bin/bash
# Build script for SNES test ROMs

set -e

# Build original test ROM
echo "Building test.sfc..."
ca65 -t none --cpu 65816 test.s -o test.o
ld65 -C snes.cfg test.o -o test.sfc
echo "Built test.sfc"
ls -lh test.sfc

# Build enhanced test ROM
echo ""
echo "Building test_enhanced.sfc..."
ca65 -t none --cpu 65816 test_enhanced.s -o test_enhanced.o
ld65 -C snes.cfg test_enhanced.o -o test_enhanced.sfc
echo "Built test_enhanced.sfc"
ls -lh test_enhanced.sfc

# Build priority test ROM
echo ""
echo "Building test_priority.sfc..."
ca65 -t none --cpu 65816 test_priority.s -o test_priority.o
ld65 -C snes.cfg test_priority.o -o test_priority.sfc
echo "Built test_priority.sfc"
ls -lh test_priority.sfc

# Build sprite overflow test ROM
echo ""
echo "Building test_sprite_overflow.sfc..."
ca65 -t none --cpu 65816 test_sprite_overflow.s -o test_sprite_overflow.o
ld65 -C snes.cfg test_sprite_overflow.o -o test_sprite_overflow.sfc
echo "Built test_sprite_overflow.sfc"
ls -lh test_sprite_overflow.sfc

echo ""
echo "All test ROMs built successfully!"
