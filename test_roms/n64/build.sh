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
title = b'TEST ROM RSP        '
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
# This code sets up an RSP task and triggers the RSP

# MIPS instructions (big-endian)
def mips_instr(opcode):
    return struct.pack('>I', opcode)

# li $sp, 0x801FFFF0 (lui $sp, 0x801F; ori $sp, $sp, 0xFFF0)
rom.extend(mips_instr(0x3C1D801F))  # lui $sp, 0x801F
rom.extend(mips_instr(0x37BDFFF0))  # ori $sp, $sp, 0xFFF0

# Step 1: Build F3DEX display list in RDRAM at 0x00100000
# li $t0, 0x80100000 (KSEG0 cached RDRAM + 0x100000)
rom.extend(mips_instr(0x3C088010))  # lui $t0, 0x8010
rom.extend(mips_instr(0x35080000))  # ori $t0, $t0, 0x0000

# G_ENDDL (0xDF) - End display list command
# Command: 0xDF000000 00000000
rom.extend(mips_instr(0x3C09DF00))  # lui $t1, 0xDF00
rom.extend(mips_instr(0xAD090000))  # sw $t1, 0($t0)
rom.extend(mips_instr(0xAD000004))  # sw $zero, 4($t0)

# Step 2: Build RDP display list in RDRAM at 0x00101000
# li $t0, 0x80101000
rom.extend(mips_instr(0x3C088010))  # lui $t0, 0x8010
rom.extend(mips_instr(0x35081000))  # ori $t0, $t0, 0x1000

# SET_FILL_COLOR - Red
rom.extend(mips_instr(0x3C093700))  # lui $t1, 0x3700
rom.extend(mips_instr(0xAD090000))  # sw $t1, 0($t0)
rom.extend(mips_instr(0x3C09FFFF))  # lui $t1, 0xFFFF
rom.extend(mips_instr(0xAD090004))  # sw $t1, 4($t0)

# FILL_RECTANGLE 1
rom.extend(mips_instr(0x3C093625))  # lui $t1, 0x3625
rom.extend(mips_instr(0x35298258))  # ori $t1, $t1, 0x8258
rom.extend(mips_instr(0xAD090008))  # sw $t1, 8($t0)
rom.extend(mips_instr(0x3C09000C))  # lui $t1, 0x000C
rom.extend(mips_instr(0x352980C8))  # ori $t1, $t1, 0x80C8
rom.extend(mips_instr(0xAD09000C))  # sw $t1, 12($t0)

# SET_FILL_COLOR - Green
rom.extend(mips_instr(0x3C093700))  # lui $t1, 0x3700
rom.extend(mips_instr(0xAD090010))  # sw $t1, 16($t0)
rom.extend(mips_instr(0x3C09FF00))  # lui $t1, 0xFF00
rom.extend(mips_instr(0x3529FF00))  # ori $t1, $t1, 0xFF00
rom.extend(mips_instr(0xAD090014))  # sw $t1, 20($t0)

# FILL_RECTANGLE 2
rom.extend(mips_instr(0x3C093634))  # lui $t1, 0x3634
rom.extend(mips_instr(0x35298230))  # ori $t1, $t1, 0x8230
rom.extend(mips_instr(0xAD090018))  # sw $t1, 24($t0)
rom.extend(mips_instr(0x3C090028))  # lui $t1, 0x0028
rom.extend(mips_instr(0x35290168))  # ori $t1, $t1, 0x0168
rom.extend(mips_instr(0xAD09001C))  # sw $t1, 28($t0)

# SYNC_FULL
rom.extend(mips_instr(0x3C092900))  # lui $t1, 0x2900
rom.extend(mips_instr(0xAD090020))  # sw $t1, 32($t0)
rom.extend(mips_instr(0xAD000024))  # sw $zero, 36($t0)

# Step 3: Build task structure in RDRAM at 0x00102000
# li $t0, 0x80102000
rom.extend(mips_instr(0x3C088010))  # lui $t0, 0x8010
rom.extend(mips_instr(0x35082000))  # ori $t0, $t0, 0x2000

# Task structure (64 bytes):
# 0x00: task_type (1 = graphics)
rom.extend(mips_instr(0x34090001))  # ori $t1, $zero, 1
rom.extend(mips_instr(0xAD090000))  # sw $t1, 0($t0)
# 0x04-0x24: other fields (set to 0)
for i in range(1, 10):
    rom.extend(mips_instr(0xAD000000 + (i * 4)))  # sw $zero, offset($t0)
# 0x28: output_buff (RDP display list at 0x00101000)
rom.extend(mips_instr(0x3C090010))  # lui $t1, 0x0010
rom.extend(mips_instr(0x35291000))  # ori $t1, $t1, 0x1000
rom.extend(mips_instr(0xAD090028))  # sw $t1, 0x28($t0)
# 0x2C: output_buff_size (40 bytes = 5 RDP commands)
rom.extend(mips_instr(0x34090028))  # ori $t1, $zero, 0x28
rom.extend(mips_instr(0xAD09002C))  # sw $t1, 0x2C($t0)
# 0x30: data_ptr (F3DEX display list at 0x00100000)
rom.extend(mips_instr(0x3C090010))  # lui $t1, 0x0010
rom.extend(mips_instr(0xAD090030))  # sw $t1, 0x30($t0)
# 0x34: data_size (8 bytes = 1 F3DEX command)
rom.extend(mips_instr(0x34090008))  # ori $t1, $zero, 8
rom.extend(mips_instr(0xAD090034))  # sw $t1, 0x34($t0)
# 0x38-0x3C: yield data (set to 0)
rom.extend(mips_instr(0xAD000038))  # sw $zero, 0x38($t0)
rom.extend(mips_instr(0xAD00003C))  # sw $zero, 0x3C($t0)

# Step 4: Write dummy microcode to RDRAM at 0x00103000
# li $t0, 0x80103000
rom.extend(mips_instr(0x3C088010))  # lui $t0, 0x8010
rom.extend(mips_instr(0x35083000))  # ori $t0, $t0, 0x3000
# Write non-zero values to trigger F3DEX detection
rom.extend(mips_instr(0x34090001))  # ori $t1, $zero, 1
rom.extend(mips_instr(0xAD090000))  # sw $t1, 0($t0)
rom.extend(mips_instr(0xAD090004))  # sw $t1, 4($t0)

# Step 5: DMA dummy microcode from RDRAM to RSP IMEM
# li $t0, 0xA4040000 (SP registers base - KSEG1 unmapped)
rom.extend(mips_instr(0x3C08A404))  # lui $t0, 0xA404

# SP_MEM_ADDR = 0x1000 (IMEM offset 0)
rom.extend(mips_instr(0x34091000))  # ori $t1, $zero, 0x1000
rom.extend(mips_instr(0xAD090000))  # sw $t1, 0($t0)

# SP_DRAM_ADDR = 0x00103000 (microcode in RDRAM)
rom.extend(mips_instr(0x3C090010))  # lui $t1, 0x0010
rom.extend(mips_instr(0x35293000))  # ori $t1, $t1, 0x3000
rom.extend(mips_instr(0xAD090004))  # sw $t1, 4($t0)

# SP_RD_LEN = 7 (8 bytes - 1)
rom.extend(mips_instr(0x34090007))  # ori $t1, $zero, 7
rom.extend(mips_instr(0xAD090008))  # sw $t1, 8($t0)

# Step 6: DMA task structure from RDRAM to RSP DMEM
# SP_MEM_ADDR = 0 (DMEM offset 0)
rom.extend(mips_instr(0xAD000000))  # sw $zero, 0($t0)

# SP_DRAM_ADDR = 0x00102000 (task structure in RDRAM)
rom.extend(mips_instr(0x3C090010))  # lui $t1, 0x0010
rom.extend(mips_instr(0x35292000))  # ori $t1, $t1, 0x2000
rom.extend(mips_instr(0xAD090004))  # sw $t1, 4($t0)

# SP_RD_LEN = 63 (64 bytes - 1)
rom.extend(mips_instr(0x3409003F))  # ori $t1, $zero, 0x3F
rom.extend(mips_instr(0xAD090008))  # sw $t1, 8($t0)

# Step 7: Trigger RSP by clearing halt bit in SP_STATUS
# SP_STATUS = 1 (clear halt)
rom.extend(mips_instr(0x34090001))  # ori $t1, $zero, 1
rom.extend(mips_instr(0xAD090010))  # sw $t1, 0x10($t0)

# Infinite loop
loop_pc = 0x90001000 + (len(rom) - 0x1000)  # Absolute address in KSEG0
loop_target = loop_pc >> 2  # Divide by 4 for jump target
rom.extend(mips_instr(0x08000000 | (loop_target & 0x3FFFFFF)))  # j loop
rom.extend(mips_instr(0x00000000))  # nop (delay slot)

# Pad to 1MB minimum
while len(rom) < 1024 * 1024:
    rom.extend(b'\x00' * 1024)

# Write ROM file
with open('test.z64', 'wb') as f:
    f.write(rom)

print(f"Created test.z64 ({len(rom)} bytes)")
print("ROM header: Magic=0x80371240, Entry=0x80000400")
print("Test ROM now uses RSP path:")
print("  1. F3DEX display list at RDRAM 0x00100000")
print("  2. RDP display list at RDRAM 0x00101000")
print("  3. RSP task structure at RDRAM 0x00102000")
print("  4. Triggers RSP which processes task and forwards to RDP")
print("  5. RDP renders: Red rectangle (50,50)-(150,150), Green rectangle (160,90)-(210,140)")
EOF
    echo "Build complete: test.z64"
else
    echo "ERROR: Python 3 not found. Cannot build test ROM."
    echo "Please install Python 3 to build the N64 test ROM."
    exit 1
fi
