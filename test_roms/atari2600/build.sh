#!/bin/bash
# Build script for Atari 2600 test ROM

set -e

echo "Building Atari 2600 test ROM..."

# Assemble (output 4K ROM)
dasm test.asm -f3 -otest.bin

echo "Atari 2600 test ROM built: test.bin"
ls -lh test.bin
