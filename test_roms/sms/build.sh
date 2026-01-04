#!/bin/bash
# Build script for SMS test ROM

set -e

echo "Building SMS test ROM..."

# Run the Python ROM generator
python3 build_rom.py

echo "SMS test ROM built: test.sms"
ls -lh test.sms
