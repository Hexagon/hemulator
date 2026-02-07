# N64 Test ROMs

This directory contains the test ROM for N64 emulation testing.

## n64-systemtest - Comprehensive N64 Hardware Test Suite

**Source**: https://github.com/lemmy-64/n64-systemtest

A comprehensive test suite that validates a wide variety of N64 features:
- CPU instructions (MFC0/DMFC0/MTC0/DMTC0, LLD/LD/SC/SCD)
- Exceptions (overflow, unaligned memory access, TRAP, BREAK, SYSCALL)
- TLB (Translation Lookaside Buffer)
- Memory access (8, 16, 32, 64 bit) to RAM, ROM, SPMEM, PIF
- RSP (Reality Signal Processor)

**Key Features**:
- Self-validating: ROM reports test results directly (no image comparison needed)
- Fast execution: Runs quickly for regression testing
- Detailed error messages: Clear indication of what's broken
- Works in known-good emulators (Project64, Mupen64Plus, etc.)

## Building the Test ROM

**Prerequisites**:
1. Install Rust: https://www.rust-lang.org/tools/install
2. Install nust64: `cargo +stable install nust64`

**Build**:
```bash
# Build n64-systemtest ROM
./build_systemtest.sh
```

This creates `n64-systemtest.n64` with the default test suite.

**Advanced builds** (from n64-systemtest directory):
```bash
cd n64-systemtest

# Build with timing tests
cargo run --release --features timing

# Build with cycle-accurate tests
cargo run --release --features cycle,timing

# Build stress tests only
cargo run --release --no-default-features --features vmulf_stress_test,vmulu_stress_test
```

See https://github.com/lemmy-64/n64-systemtest for complete documentation.

## Running the Test ROM

### In the Hemulator GUI
```bash
cargo run --profile release-quick -- test_roms/n64/n64-systemtest.n64
```

### Expected Output

Look for output on screen:
- Success: "Done! Tests: XXX. Failed: 0"
- Partial success: "Done! Tests: XXX. Failed: Y" (with error messages)
- Failure: Empty screen (emulator crashed or didn't boot)

## Integration with Smoke Tests

**Note**: N64 smoke tests have been removed as the simple test ROMs don't work in known-good emulators. For N64 testing, use n64-systemtest manually as described above.

## Troubleshooting

### n64-systemtest shows empty screen

The emulator may be missing features required for boot. See the n64-systemtest README for troubleshooting:
- https://github.com/lemmy-64/n64-systemtest#troubleshooting

Key recommendations:
1. Implement LL/SC instructions (even stub implementations help)
2. Implement DMFC0/DMTC0 (can be simplified to MFC0/MTC0 for initial testing)
3. Support ISViewer output (memory-mapped console at 0xB3FF0020-0xB3FF0220)



## References

- n64-systemtest repository: https://github.com/lemmy-64/n64-systemtest
- N64 system documentation: `../../crates/systems/n64/README.md`
- Test ROM guidelines: `../README.md`
- AGENTS.md test requirements: `../../AGENTS.md#test-rom-requirements`
