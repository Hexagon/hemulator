#!/bin/bash
# Build script for N64 test ROM
# Requires: mips64-linux-gnu-gcc, mips64-linux-gnu-ld, and n64tool (or manual ROM header creation)

set -e

echo "Building N64 test ROM..."

# For now, we'll create a minimal binary ROM manually since assembling MIPS64
# requires cross-compilation tools that may not be readily available

# Create a simple test ROM that can be loaded
# This is a minimal ROM with proper header and simple code

# Use Python to generate the ROM if available, otherwise skip
if command -v python3 &> /dev/null; then
    python3 << 'EOF'
import struct
import os

# Create output directory if needed
os.makedirs(os.path.dirname(__file__) or '.', exist_ok=True)

# N64 ROM header (64 bytes)
rom = bytearray()

# Magic number for Z64 format (big-endian)
rom.extend(struct.pack('>I', 0x80371240))  # 0x00: Magic

# Clock rate / boot address
rom.extend(struct.pack('>I', 0x0000000F))  # 0x04: Clock rate
rom.extend(struct.pack('>I', 0x80000400))  # 0x08: Boot address
rom.extend(struct.pack('>I', 0x00001444))  # 0x0C: Release

# CRC (not validated by emulator, use dummy values)
rom.extend(struct.pack('>I', 0x00000000))  # 0x10: CRC1
rom.extend(struct.pack('>I', 0x00000000))  # 0x14: CRC2

# Reserved
rom.extend(b'\x00' * 8)                    # 0x18-0x1F

# Game title (20 bytes, padded with spaces)
title = b'TEST ROM            '
rom.extend(title[:20])                     # 0x20-0x33

# Reserved
rom.extend(b'\x00' * 7)                    # 0x34-0x3A

# Game ID
rom.extend(b'N')                           # 0x3B: Manufacturer
rom.extend(b'TE')                          # 0x3C-0x3D: Cartridge ID
rom.extend(b'\x00')                        # 0x3E: Country code

# Version
rom.extend(b'\x00')                        # 0x3F: Version

# Pad header to 0x1000 (IPL3 boot code area)
rom.extend(b'\x00' * (0x1000 - len(rom)))

# Boot code at 0x1000 (physical address 0x00000000, will be mapped to 0x80000400)
# This code writes display list and triggers RDP

# MIPS instructions (big-endian)
def mips_instr(opcode):
    return struct.pack('>I', opcode)

# li $sp, 0x801FFFF0 (lui $sp, 0x801F; ori $sp, $sp, 0xFFF0)
rom.extend(mips_instr(0x3C1D801F))  # lui $sp, 0x801F
rom.extend(mips_instr(0x37BDFFF0))  # ori $sp, $sp, 0xFFF0

# li $t0, 0x00100000 (lui $t0, 0x0010; ori $t0, $t0, 0x0000)
rom.extend(mips_instr(0x3C080010))  # lui $t0, 0x0010
rom.extend(mips_instr(0x35080000))  # ori $t0, $t0, 0x0000

# Command 1: SET_FILL_COLOR - Red
# li $t1, 0x37000000
rom.extend(mips_instr(0x3C093700))  # lui $t1, 0x3700
# sw $t1, 0($t0)
rom.extend(mips_instr(0xAD090000))  # sw $t1, 0($t0)
# li $t1, 0xFFFF0000 (red)
rom.extend(mips_instr(0x3C09FFFF))  # lui $t1, 0xFFFF
# sw $t1, 4($t0)
rom.extend(mips_instr(0xAD090004))  # sw $t1, 4($t0)

# Command 2: FILL_RECTANGLE
# li $t1, 0x36258258
rom.extend(mips_instr(0x3C093625))  # lui $t1, 0x3625
rom.extend(mips_instr(0x35298258))  # ori $t1, $t1, 0x8258
rom.extend(mips_instr(0xAD090008))  # sw $t1, 8($t0)
# li $t1, 0x00C800C8
rom.extend(mips_instr(0x3C0900C8))  # lui $t1, 0x00C8
rom.extend(mips_instr(0x352900C8))  # ori $t1, $t1, 0x00C8
rom.extend(mips_instr(0xAD09000C))  # sw $t1, 12($t0)

# Command 3: SET_FILL_COLOR - Green
rom.extend(mips_instr(0x3C093700))  # lui $t1, 0x3700
rom.extend(mips_instr(0xAD090010))  # sw $t1, 16($t0)
rom.extend(mips_instr(0x3C09FF00))  # lui $t1, 0xFF00
rom.extend(mips_instr(0x3529FF00))  # ori $t1, $t1, 0xFF00
rom.extend(mips_instr(0xAD090014))  # sw $t1, 20($t0)

# Command 4: FILL_RECTANGLE
rom.extend(mips_instr(0x3C093634))  # lui $t1, 0x3634
rom.extend(mips_instr(0x35298230))  # ori $t1, $t1, 0x8230
rom.extend(mips_instr(0xAD090018))  # sw $t1, 24($t0)
rom.extend(mips_instr(0x3C090280))  # lui $t1, 0x0280
rom.extend(mips_instr(0x35290168))  # ori $t1, $t1, 0x0168
rom.extend(mips_instr(0xAD09001C))  # sw $t1, 28($t0)

# Command 5: SYNC_FULL
rom.extend(mips_instr(0x3C092900))  # lui $t1, 0x2900
rom.extend(mips_instr(0xAD090020))  # sw $t1, 32($t0)
rom.extend(mips_instr(0xAD000024))  # sw $zero, 36($t0)

# Trigger RDP
# li $t0, 0x04100000
rom.extend(mips_instr(0x3C080410))  # lui $t0, 0x0410
# li $t1, 0x00100000 (DPC_START)
rom.extend(mips_instr(0x3C090010))  # lui $t1, 0x0010
rom.extend(mips_instr(0xAD090000))  # sw $t1, 0($t0) - DPC_START
# li $t1, 0x00100028 (DPC_END)
rom.extend(mips_instr(0x3C090010))  # lui $t1, 0x0010
rom.extend(mips_instr(0x35290028))  # ori $t1, $t1, 0x0028
rom.extend(mips_instr(0xAD090004))  # sw $t1, 4($t0) - DPC_END

# Infinite loop
# loop: j loop; nop
loop_offset = len(rom) - 0x1000  # Relative to boot code start
rom.extend(mips_instr(0x08000000 | (loop_offset >> 2)))  # j loop
rom.extend(mips_instr(0x00000000))  # nop (delay slot)

# Pad to 1MB minimum
while len(rom) < 1024 * 1024:
    rom.extend(b'\x00' * 1024)

# Write ROM file
with open('test.z64', 'wb') as f:
    f.write(rom)

print(f"Created test.z64 ({len(rom)} bytes)")
print("ROM header: Magic=0x80371240, Entry=0x80000400")
print("Display list commands at RDRAM 0x00100000:")
print("  1. SET_FILL_COLOR: Red (0xFFFF0000)")
print("  2. FILL_RECTANGLE: (50,50) to (150,150)")
print("  3. SET_FILL_COLOR: Green (0xFF00FF00)")
print("  4. FILL_RECTANGLE: (160,90) to (210,140)")
print("  5. SYNC_FULL")
EOF
    echo "Build complete: test.z64"
else
    echo "ERROR: Python 3 not found. Cannot build test ROM."
    echo "Please install Python 3 to build the N64 test ROM."
    exit 1
fi
