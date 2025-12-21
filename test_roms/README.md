# Test ROMs

This directory contains minimal test ROMs for smoke testing each emulated system.

## Purpose

These test ROMs are designed to:
1. Verify basic system functionality (CPU, memory, video output)
2. Provide deterministic output for automated testing
3. Serve as reference implementations for new systems

## Current Systems

- **NES** - Full implementation with smoke test
- **Game Boy** - Implementation with smoke test and visible output validation
- **Game Boy Color** - DMG-compatible mode with smoke test
- **Atari 2600** - Full implementation with smoke test
- **SNES** - Implementation with smoke test (65C816 CPU with real execution)
- **N64** - Implementation with smoke test (MIPS R4300i CPU with real execution)

## Future Systems

When implementing new systems (SNES, GBC, etc.), follow this pattern:
1. Create a subdirectory: `test_roms/<system>/`
2. Write minimal assembly code that produces visible output
3. Add build script and built ROM
4. Add smoke test to system crate
5. Update this README and AGENTS.md

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

### Game Boy Color (test.gbc)
- Format: GBC ROM (CGB flag = 0x80, DMG compatible)
- Size: 32KB
- Cartridge type: ROM only (no MBC)
- Behavior: Same as GB test ROM, runs in DMG mode
- Expected output: Visible checkerboard pattern on screen

### Atari 2600 (test.bin)
- Format: Raw binary
- Size: 4KB
- Behavior: Sets playfield to alternating pattern ($AA)
- Expected output: Visible playfield pattern on screen

### SNES (test.sfc)
- Format: LoROM (32KB with SMC header support)
- Size: 32KB
- CPU: WDC 65C816 (16-bit processor)
- Behavior: Writes alternating 0xAA and 0x55 bytes to WRAM at $7E:0000
- Expected output: Checkerboard pattern with 50/50 color distribution
- Test ROM generates code via Python (see build.sh)

### N64 (test.z64)
- Format: Z64 (big-endian)
- Size: 1MB  
- CPU: MIPS R4300i (64-bit RISC processor)
- Behavior: Writes alternating 0xAA and 0x55 bytes to RDRAM at 0x00000000
- Expected output: Checkerboard pattern with 50/50 color distribution
- Test ROM generates code via Python (see build.sh)

## Integration with Tests

These ROMs are included in the smoke tests for each system crate:
- `crates/systems/nes/src/lib.rs` - NES smoke test
- `crates/systems/gb/src/lib.rs` - Game Boy and Game Boy Color smoke tests
- `crates/systems/atari2600/src/lib.rs` - Atari 2600 smoke test
- `crates/systems/snes/src/lib.rs` - SNES smoke test
- `crates/systems/n64/src/lib.rs` - N64 smoke test

The tests load each ROM, run it for a few frames, and verify that the output frame contains expected non-zero pixel data, confirming that the emulator is functioning correctly.
