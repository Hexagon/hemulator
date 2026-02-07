#!/usr/bin/env python3
"""
Enhanced SMS test ROM generator (simplified)
Creates a test with multiple colors
"""

def write_word(rom, addr, value):
    """Write a 16-bit word to ROM (little-endian)"""
    rom[addr] = value & 0xFF
    rom[addr + 1] = (value >> 8) & 0xFF

def assemble_z80(rom):
    """Assemble Z80 code for enhanced test"""
    pc = 0
    
    # DI
    rom[pc] = 0xF3
    pc += 1
    
    # LD SP, 0xDFF0
    rom[pc] = 0x31
    pc += 1
    write_word(rom, pc, 0xDFF0)
    pc += 2
    
    # === VDP REGISTER SETUP ===
    # Helper to write register
    regs = [
        (0, 0x04),  # Mode 4
        (1, 0x28),  # Display on (bit 6=0), sprites on (bit 3=1), frame interrupt (bit 5=1)
        (2, 0x0E),  # Name table at 0x3800
        (5, 0x7E),  # Sprite table at 0x3F00
        (7, 0x0F),  # Backdrop color entry 15
    ]
    
    for reg, val in regs:
        rom[pc] = 0x3E  # LD A, val
        pc += 1
        rom[pc] = val
        pc += 1
        rom[pc] = 0xD3  # OUT (0xBF), A
        pc += 1
        rom[pc] = 0xBF
        pc += 1
        rom[pc] = 0x3E  # LD A, 0x80|reg
        pc += 1
        rom[pc] = 0x80 | reg
        pc += 1
        rom[pc] = 0xD3  # OUT (0xBF), A
        pc += 1
        rom[pc] = 0xBF
        pc += 1
    
    # === LOAD PALETTE (CRAM) ===
    # SMS color format: 6-bit RGB encoded as --BBGGRR (2 bits per channel)
    # Color values: 0b00 (dark), 0b01 (medium), 0b10 (bright), 0b11 (max)
    # Example: 0x30 = 0b110000 = blue at max brightness
    colors = [
        0x00,  # 0: Black (0b000000)
        0x3F,  # 1: White (0b111111)
        0x03,  # 2: Red (0b000011)
        0x0C,  # 3: Green (0b001100)
        0x30,  # 4: Blue (0b110000)
        0x15,  # 5: Orange
        0x33,  # 6: Magenta
        0x3C,  # 7: Cyan
        0x2A,  # 8: Purple
        0x1F,  # 9: Bright Red
        0x3C,  # 10: Bright Green
        0x30,  # 11: Bright Blue
        0x15,  # 12: Dark gray
        0x2A,  # 13: Mid gray
        0x3F,  # 14: Bright white
        0x15,  # 15: Brown
        0x30,  # 16: Backdrop (blue 0b110000)
    ]
    
    for i, color in enumerate(colors):
        rom[pc] = 0x3E  # LD A, i
        pc += 1
        rom[pc] = i
        pc += 1
        rom[pc] = 0xD3  # OUT (0xBF), A
        pc += 1
        rom[pc] = 0xBF
        pc += 1
        rom[pc] = 0x3E  # LD A, 0xC0
        pc += 1
        rom[pc] = 0xC0
        pc += 1
        rom[pc] = 0xD3  # OUT (0xBF), A
        pc += 1
        rom[pc] = 0xBF
        pc += 1
        rom[pc] = 0x3E  # LD A, color
        pc += 1
        rom[pc] = color
        pc += 1
        rom[pc] = 0xD3  # OUT (0xBE), A
        pc += 1
        rom[pc] = 0xBE
        pc += 1
    
    # === LOAD TILE PATTERNS ===
    # Set VRAM address to 0x0000
    rom[pc] = 0x3E  # LD A, 0x00
    pc += 1
    rom[pc] = 0x00
    pc += 1
    rom[pc] = 0xD3  # OUT (0xBF), A
    pc += 1
    rom[pc] = 0xBF
    pc += 1
    rom[pc] = 0x3E  # LD A, 0x40
    pc += 1
    rom[pc] = 0x40
    pc += 1
    rom[pc] = 0xD3  # OUT (0xBF), A
    pc += 1
    rom[pc] = 0xBF
    pc += 1
    
    # Write 4 solid color tiles (32 bytes each)
    tiles = [
        (0xFF, 0x00, 0x00, 0x00),  # Tile 0: Pixel value 1 (white)
        (0x00, 0xFF, 0x00, 0x00),  # Tile 1: Pixel value 2 (red)
        (0xFF, 0xFF, 0x00, 0x00),  # Tile 2: Pixel value 3 (green)
        (0x00, 0x00, 0xFF, 0x00),  # Tile 3: Pixel value 4 (blue)
    ]
    
    for tile in tiles:
        for row in range(8):
            for byte_val in tile:
                rom[pc] = 0x3E  # LD A, byte_val
                pc += 1
                rom[pc] = byte_val
                pc += 1
                rom[pc] = 0xD3  # OUT (0xBE), A
                pc += 1
                rom[pc] = 0xBE
                pc += 1
    
    # === FILL NAME TABLE ===
    # Set VRAM address to 0x3800
    rom[pc] = 0x3E  # LD A, 0x00
    pc += 1
    rom[pc] = 0x00
    pc += 1
    rom[pc] = 0xD3  # OUT (0xBF), A
    pc += 1
    rom[pc] = 0xBF
    pc += 1
    rom[pc] = 0x3E  # LD A, 0x78
    pc += 1
    rom[pc] = 0x78
    pc += 1
    rom[pc] = 0xD3  # OUT (0xBF), A
    pc += 1
    rom[pc] = 0xBF
    pc += 1
    
    # Fill name table: 24 rows x 32 cols
    for row in range(24):
        tile_idx = 0 if row < 6 else (1 if row < 12 else (2 if row < 18 else 3))
        for col in range(32):
            rom[pc] = 0x3E  # LD A, tile_idx
            pc += 1
            rom[pc] = tile_idx
            pc += 1
            rom[pc] = 0xD3  # OUT (0xBE), A
            pc += 1
            rom[pc] = 0xBE
            pc += 1
            rom[pc] = 0x3E  # LD A, 0x00
            pc += 1
            rom[pc] = 0x00
            pc += 1
            rom[pc] = 0xD3  # OUT (0xBE), A
            pc += 1
            rom[pc] = 0xBE
            pc += 1
    
    # === LOOP ===
    rom[pc] = 0xFB  # EI
    pc += 1
    loop_addr = pc
    rom[pc] = 0x76  # HALT
    pc += 1
    rom[pc] = 0x18  # JR loop_addr
    pc += 1
    rom[pc] = (loop_addr - (pc + 1)) & 0xFF
    pc += 1
    
    return pc

def add_sms_header(rom):
    """Add TMR SEGA header"""
    header_offset = 0x7FF0
    rom[header_offset:header_offset + 8] = b"TMR SEGA"
    rom[header_offset + 8:header_offset + 10] = bytes([0x00, 0x00])
    checksum = sum(rom[:0x7FF0]) & 0xFFFF
    rom[header_offset + 10] = checksum & 0xFF
    rom[header_offset + 11] = (checksum >> 8) & 0xFF
    rom[header_offset + 12:header_offset + 15] = bytes([0x00, 0x00, 0x01])
    rom[header_offset + 15] = 0x4C

def main():
    rom = bytearray(32 * 1024)
    code_size = assemble_z80(rom)
    print(f"Code size: {code_size} bytes")
    add_sms_header(rom)
    with open("test_enhanced.sms", "wb") as f:
        f.write(rom)
    print("Created test_enhanced.sms (32768 bytes)")
    print("Expected: Blue backdrop with 4 colored bands")

if __name__ == "__main__":
    main()
