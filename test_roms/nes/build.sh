#!/bin/bash
# Build script for NES test ROM

set -e

echo "Building NES test ROM..."

# Assemble
ca65 test.s -o test.o

# Link
ld65 -C nes.cfg test.o -o test.nes

# Cleanup
rm -f test.o

echo "NES test ROM built: test.nes"
ls -lh test.nes
