#!/bin/bash
# Build script for ColecoVision test ROM

set -e

echo "Building ColecoVision test ROM..."

# Run the Python ROM generator
python3 build_rom.py

echo "ColecoVision test ROM built: test.col"
ls -lh test.col
