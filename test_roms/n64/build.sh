#!/bin/bash
# Build script for N64 test ROM

set -e

echo "Building N64 test ROM..."

# Generate ROM with Python (simpler than MIPS assembly for minimal test)
python3 << 'EOF'
#!/usr/bin/env python3
# Generate a minimal N64 test ROM with checkerboard pattern

import struct

rom_size = 1024 * 1024  # 1MB ROM (minimum)
rom = bytearray([0x00] * rom_size)

# N64 ROM header (first 64 bytes)
# Magic bytes (big-endian)
rom[0:4] = b'\x80\x37\x12\x40'  # PI BSD Domain 1 register value

# Clock rate and boot address
rom[4:8] = struct.pack('>I', 0x0000000F)  # Clock rate
rom[8:12] = struct.pack('>I', 0x10000000)  # Boot address (entry point in ROM)
rom[12:16] = struct.pack('>I', 0x00001444)  # Release

# CRC (checksum - not validated by emulator for now)
rom[16:20] = b'\x00\x00\x00\x00'  # CRC1
rom[20:24] = b'\x00\x00\x00\x00'  # CRC2

# Unknown/unused
rom[24:32] = b'\x00' * 8

# Game title (20 bytes, padded with spaces)
rom[32:52] = b'N64 TEST ROM        '

# Unknown/unused
rom[52:64] = b'\x00' * 12

# Simple MIPS code at boot address (0x1000 in ROM = 0x10001000 in address space)
# The code writes a checkerboard pattern to RDRAM at 0x00000000
code_offset = 0x1000

code = [
    # Initialize: write pattern to RDRAM
    # lui t0, 0x0000      # t0 = base address (RDRAM at 0x00000000)
    0x3C, 0x08, 0x00, 0x00,
    
    # li t1, 0xAA        # t1 = pattern byte 1 (0xAA)
    0x34, 0x09, 0x00, 0xAA,
    
    # li t2, 0x55        # t2 = pattern byte 2 (0x55)
    0x34, 0x0A, 0x00, 0x55,
    
    # li t3, 0x2000      # t3 = counter (8KB)
    0x34, 0x0B, 0x20, 0x00,
    
    # loop:
    # sb t1, 0(t0)       # Store byte 1
    0xA1, 0x09, 0x00, 0x00,
    
    # addiu t0, t0, 1    # Increment address
    0x25, 0x08, 0x00, 0x01,
    
    # sb t2, 0(t0)       # Store byte 2
    0xA1, 0x0A, 0x00, 0x00,
    
    # addiu t0, t0, 1    # Increment address
    0x25, 0x08, 0x00, 0x01,
    
    # addiu t3, t3, -2   # Decrement counter by 2
    0x25, 0x6B, 0xFF, 0xFE,
    
    # bne t3, $0, loop   # Branch if not zero
    0x15, 0x60, 0xFF, 0xF8,
    
    # nop (delay slot)
    0x00, 0x00, 0x00, 0x00,
    
    # infinite_loop:
    # j infinite_loop    # Jump to self
    0x08, 0x04, 0x00, 0x11,
    
    # nop (delay slot)
    0x00, 0x00, 0x00, 0x00,
]

# Write code to ROM
rom[code_offset:code_offset + len(code)] = bytes(code)

# Write ROM file
with open('test.z64', 'wb') as f:
    f.write(rom)

print(f"Generated N64 test ROM: test.z64 ({len(rom)} bytes)")
EOF

echo "N64 test ROM built: test.z64"
ls -lh test.z64
