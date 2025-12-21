# Test ROMs

This directory contains minimal test ROMs for smoke testing each emulated system.

## Purpose

These test ROMs are designed to:
1. Verify basic system functionality (CPU, memory, video output)
2. Provide deterministic output for automated testing
3. Serve as reference implementations for new systems

## Building

Each system has a `build.sh` script that assembles the test ROM from source:

```bash
# NES
cd nes && ./build.sh

# Game Boy
cd gb && ./build.sh

# Atari 2600
cd atari2600 && ./build.sh
```

## Requirements

- **NES**: cc65 (6502 assembler/linker)
- **Game Boy**: rgbds (GB assembler/linker)
- **Atari 2600**: dasm (6502 assembler)

On Ubuntu/Debian:
```bash
sudo apt-get install cc65 dasm

# For rgbds, build from source:
git clone https://github.com/gbdev/rgbds.git
cd rgbds && make && sudo make install
```

## Test ROM Specifications

### NES (test.nes)
- Format: iNES (16-byte header)
- Mapper: 0 (NROM)
- PRG-ROM: 1 x 16KB
- CHR-ROM: 1 x 8KB
- Behavior: Fills screen with tile $55 (checkerboard pattern)
- Expected output: Visible checkerboard pattern on screen

### Game Boy (test.gb)
- Format: GB ROM (with Nintendo logo)
- Size: 32KB
- Cartridge type: ROM only (no MBC)
- Behavior: Fills screen with tile $00 (checkerboard pattern)
- Expected output: Visible checkerboard pattern on screen

### Atari 2600 (test.bin)
- Format: Raw binary
- Size: 4KB
- Behavior: Sets playfield to alternating pattern ($AA)
- Expected output: Visible playfield pattern on screen

## Integration with Tests

These ROMs are included in the smoke tests for each system crate:
- `crates/systems/nes/src/lib.rs` - NES smoke test
- `crates/systems/gb/src/lib.rs` - Game Boy smoke test
- `crates/systems/atari2600/src/lib.rs` - Atari 2600 smoke test

The tests load each ROM, run it for a few frames, and verify that the output frame contains expected non-zero pixel data, confirming that the emulator is functioning correctly.
