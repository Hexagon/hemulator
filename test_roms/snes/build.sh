#!/bin/bash
# Build script for SNES test ROM

set -e

echo "Building SNES test ROM..."

# Generate ROM with Python (simpler than assembly for minimal test)
python3 << 'EOF'
#!/usr/bin/env python3
# Generate a minimal SNES test ROM with checkerboard pattern in WRAM

rom_size = 32768  # 32KB ROM (minimum for LoROM)
rom = bytearray([0x00] * rom_size)

# Simple 6502/65816 code at $8000 (start of ROM)
code = [
    0x78,        # SEI - disable interrupts
    0x18,        # CLC - clear carry
    0xFB,        # XCE - switch to native mode (65816)
    0xC2, 0x38,  # REP #$38 - 16-bit A, 16-bit X/Y
    0xA9, 0xFF, 0x1F,  # LDA #$1FFF - set up stack
    0x1B,        # TCS - transfer to stack
    0xA9, 0x00, 0x00,  # LDA #$0000 - clear A
    0x5B,        # TCD - clear direct page
    0x48,        # PHA - push for data bank
    0xAB,        # PLB - pull data bank (set to 0)
    0xE2, 0x20,  # SEP #$20 - 8-bit accumulator
    0xA2, 0x00, 0x00,  # LDX #$0000 - start offset
    # write_loop:
    0xA9, 0xAA,  # LDA #$AA - pattern 1
    0x9F, 0x00, 0x00, 0x7E,  # STA $7E0000,X - write to WRAM
    0xE8,        # INX
    0xA9, 0x55,  # LDA #$55 - pattern 2
    0x9F, 0x00, 0x00, 0x7E,  # STA $7E0000,X
    0xE8,        # INX
    0xE0, 0x00, 0x20,  # CPX #$2000 - compare to 8KB
    0xD0, 0xEC,  # BNE write_loop - branch back (-20 bytes)
    # forever:
    0xCB,        # WAI - wait for interrupt
    0x80, 0xFD,  # BRA forever - branch to self (-3 bytes)
]

# Write code to $8000 (offset 0 in ROM)
rom[0:len(code)] = code

# NMI handler (just RTI)
nmi_code = [0x40]  # RTI
rom[0x0100:0x0101] = nmi_code

# IRQ handler (just RTI) 
irq_code = [0x40]  # RTI
rom[0x0200:0x0201] = irq_code

# LoROM header at $7FB0 (offset in 32KB bank)
header_offset = 0x7FB0
rom[header_offset:header_offset+21] = b"SNES TEST ROM    "  # Title (21 bytes)
rom[header_offset+21] = 0x20  # ROM makeup (LoROM)
rom[header_offset+22] = 0x00  # ROM type
rom[header_offset+23] = 0x07  # ROM size (128KB)
rom[header_offset+24] = 0x00  # SRAM size
rom[header_offset+25] = 0x01  # Country (USA)
rom[header_offset+26] = 0x33  # License
rom[header_offset+27] = 0x00  # Version
rom[header_offset+28:header_offset+30] = b'\x00\x00'  # Checksum complement
rom[header_offset+30:header_offset+32] = b'\x00\x00'  # Checksum

# Native mode vectors at $7FE4
vectors_offset = 0x7FE4
rom[vectors_offset:vectors_offset+2] = b'\x00\x00'      # COP
rom[vectors_offset+2:vectors_offset+4] = b'\x00\x00'    # BRK
rom[vectors_offset+4:vectors_offset+6] = b'\x00\x00'    # ABORT
rom[vectors_offset+6:vectors_offset+8] = b'\x00\x81'    # NMI ($8100)
rom[vectors_offset+8:vectors_offset+10] = b'\x00\x00'   # unused
rom[vectors_offset+10:vectors_offset+12] = b'\x00\x82'  # IRQ ($8200)

# Emulation mode vectors at $7FF4
emu_vectors_offset = 0x7FF4
rom[emu_vectors_offset:emu_vectors_offset+2] = b'\x00\x00'    # COP
rom[emu_vectors_offset+2:emu_vectors_offset+4] = b'\x00\x00'  # unused
rom[emu_vectors_offset+4:emu_vectors_offset+6] = b'\x00\x00'  # ABORT
rom[emu_vectors_offset+6:emu_vectors_offset+8] = b'\x00\x81'  # NMI ($8100)
rom[emu_vectors_offset+8:emu_vectors_offset+10] = b'\x00\x80' # RESET ($8000)
rom[emu_vectors_offset+10:emu_vectors_offset+12] = b'\x00\x82' # IRQ ($8200)

# Write ROM file
with open('test.sfc', 'wb') as f:
    f.write(rom)

print(f"Generated SNES test ROM: test.sfc ({len(rom)} bytes)")
EOF

echo "SNES test ROM built: test.sfc"
ls -lh test.sfc
