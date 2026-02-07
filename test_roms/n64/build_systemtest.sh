#!/bin/bash
# Build script for n64-systemtest test ROM
# This builds the comprehensive N64 test suite from @lemmy-64/n64-systemtest

set -euo pipefail

echo "Building n64-systemtest ROM..."

# Check if rust is installed
if ! command -v cargo &> /dev/null; then
    echo "ERROR: Rust/Cargo not found. Please install Rust from https://rustup.rs/"
    exit 1
fi

# Check if nust64 is installed
if ! command -v nust64 &> /dev/null; then
    echo "Installing nust64 (N64 ROM builder)..."
    cargo +stable install nust64
fi

# Check if submodule has been initialized
if [ ! -d "n64-systemtest" ]; then
    echo "ERROR: 'n64-systemtest' directory not found."
    echo "If this repository uses git submodules, try running:"
    echo "  git submodule update --init --recursive"
    exit 1
fi

# Navigate to n64-systemtest directory
cd n64-systemtest

# Build the test ROM (default feature set)
echo "Building n64-systemtest with default feature set..."
cargo run --release

# Check if ROM was created
if [ ! -f "target/mips-nintendo64-none/release/n64-systemtest.n64" ]; then
    echo "ERROR: n64-systemtest ROM build failed"
    exit 1
fi

# Copy ROM to parent directory for easy access
cp target/mips-nintendo64-none/release/n64-systemtest.n64 ../n64-systemtest.n64

echo "Build complete: n64-systemtest.n64"
echo "This ROM contains comprehensive N64 hardware tests"
echo "Expected output: 'Done! Tests: XXX. Failed: 0' (if emulator is perfect)"
echo "For more information, see: https://github.com/lemmy-64/n64-systemtest"
