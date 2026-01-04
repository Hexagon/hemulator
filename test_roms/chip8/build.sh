#!/bin/bash
# Build script for CHIP-8 test ROM

python3 assemble.py test.asm test.ch8
echo "Built test.ch8"
