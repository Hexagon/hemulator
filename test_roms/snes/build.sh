#!/bin/bash
# Build script for SNES test ROM

set -e

# Assemble
ca65 -t none --cpu 65816 test.s -o test.o

# Link
ld65 -C snes.cfg test.o -o test.sfc

echo "Built test.sfc"
ls -lh test.sfc
