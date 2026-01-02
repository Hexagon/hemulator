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

echo ""
echo "All test ROMs built successfully!"
