#!/usr/bin/env python3
"""
SMS test ROM generator
Creates a minimal Sega Master System ROM that displays a checkerboard pattern
"""

import struct

def write_byte(rom, addr, value):
    """Write a byte to ROM at address"""
    rom[addr] = value & 0xFF

def write_word(rom, addr, value):
    """Write a 16-bit word to ROM (little-endian)"""
    rom[addr] = value & 0xFF
    rom[addr + 1] = (value >> 8) & 0xFF

def assemble_z80(rom):
    """
    Assemble basic Z80 code for SMS test
    
    This creates a minimal program that:
    1. Disables interrupts
    2. Sets up stack pointer
    3. Initializes VDP registers
    4. Loads checkerboard tile data into VRAM
    5. Fills tilemap with alternating tiles
    6. Enables display
    7. Loops forever
    """
    
    pc = 0x0000  # Start at address 0
    
    # Disable interrupts
    rom[pc] = 0xF3  # DI
    pc += 1
    
    # Set stack pointer to $DFF0
    rom[pc] = 0x31  # LD SP, nn
    pc += 1
    write_word(rom, pc, 0xDFF0)
    pc += 2
    
    # Initialize VDP registers
    # Register 0: Mode Control 1 - $04 (no interrupts, mode 4)
    rom[pc] = 0x3E  # LD A, n
    pc += 1
    rom[pc] = 0x04  # Mode control value
    pc += 1
    rom[pc] = 0xD3  # OUT (n), A
    pc += 1
    rom[pc] = 0xBF  # VDP control port
    pc += 1
    rom[pc] = 0x3E  # LD A, n
    pc += 1
    rom[pc] = 0x80  # Register write command (register 0)
    pc += 1
    rom[pc] = 0xD3  # OUT (n), A
    pc += 1
    rom[pc] = 0xBF  # VDP control port
    pc += 1
    
    # Register 1: Mode Control 2 - $20 (display on, mode 4)
    rom[pc] = 0x3E  # LD A, n
    pc += 1
    rom[pc] = 0x20  # Mode control value (bit 6 = display enable)
    pc += 1
    rom[pc] = 0xD3  # OUT (n), A
    pc += 1
    rom[pc] = 0xBF  # VDP control port
    pc += 1
    rom[pc] = 0x3E  # LD A, n
    pc += 1
    rom[pc] = 0x81  # Register write command (register 1)
    pc += 1
    rom[pc] = 0xD3  # OUT (n), A
    pc += 1
    rom[pc] = 0xBF  # VDP control port
    pc += 1
    
    # Register 2: Name Table Base Address - $FF (nametable at $3800)
    rom[pc] = 0x3E  # LD A, n
    pc += 1
    rom[pc] = 0x0E  # Name table at $3800 ($0E << 10)
    pc += 1
    rom[pc] = 0xD3  # OUT (n), A
    pc += 1
    rom[pc] = 0xBF  # VDP control port
    pc += 1
    rom[pc] = 0x3E  # LD A, n
    pc += 1
    rom[pc] = 0x82  # Register write command (register 2)
    pc += 1
    rom[pc] = 0xD3  # OUT (n), A
    pc += 1
    rom[pc] = 0xBF  # VDP control port
    pc += 1
    
    # Register 5: Sprite Attribute Table - $FF (at $3F00)
    rom[pc] = 0x3E  # LD A, n
    pc += 1
    rom[pc] = 0x7E  # Sprite table at $3F00
    pc += 1
    rom[pc] = 0xD3  # OUT (n), A
    pc += 1
    rom[pc] = 0xBF  # VDP control port
    pc += 1
    rom[pc] = 0x3E  # LD A, n
    pc += 1
    rom[pc] = 0x85  # Register write command (register 5)
    pc += 1
    rom[pc] = 0xD3  # OUT (n), A
    pc += 1
    rom[pc] = 0xBF  # VDP control port
    pc += 1
    
    # Register 6: Sprite Pattern Base - $FF (at $2000)
    rom[pc] = 0x3E  # LD A, n
    pc += 1
    rom[pc] = 0x04  # Sprite patterns at $2000
    pc += 1
    rom[pc] = 0xD3  # OUT (n), A
    pc += 1
    rom[pc] = 0xBF  # VDP control port
    pc += 1
    rom[pc] = 0x3E  # LD A, n
    pc += 1
    rom[pc] = 0x86  # Register write command (register 6)
    pc += 1
    rom[pc] = 0xD3  # OUT (n), A
    pc += 1
    rom[pc] = 0xBF  # VDP control port
    pc += 1
    
    # Register 7: Backdrop color - $00 (black)
    rom[pc] = 0x3E  # LD A, n
    pc += 1
    rom[pc] = 0x00  # Black backdrop
    pc += 1
    rom[pc] = 0xD3  # OUT (n), A
    pc += 1
    rom[pc] = 0xBF  # VDP control port
    pc += 1
    rom[pc] = 0x3E  # LD A, n
    pc += 1
    rom[pc] = 0x87  # Register write command (register 7)
    pc += 1
    rom[pc] = 0xD3  # OUT (n), A
    pc += 1
    rom[pc] = 0xBF  # VDP control port
    pc += 1
    
    # Set VRAM write address to $0000 (tile patterns start)
    rom[pc] = 0x3E  # LD A, n
    pc += 1
    rom[pc] = 0x00  # Low byte of address
    pc += 1
    rom[pc] = 0xD3  # OUT (n), A
    pc += 1
    rom[pc] = 0xBF  # VDP control port
    pc += 1
    rom[pc] = 0x3E  # LD A, n
    pc += 1
    rom[pc] = 0x40  # High byte with VRAM write bit (0x40)
    pc += 1
    rom[pc] = 0xD3  # OUT (n), A
    pc += 1
    rom[pc] = 0xBF  # VDP control port
    pc += 1
    
    # Write tile 0: All white pixels (32 bytes of $FF)
    # B = counter
    rom[pc] = 0x06  # LD B, n
    pc += 1
    rom[pc] = 32  # 32 bytes
    pc += 1
    rom[pc] = 0x3E  # LD A, n
    pc += 1
    rom[pc] = 0xFF  # White pixel
    pc += 1
    # Loop start at current pc
    tile0_loop = pc
    rom[pc] = 0xD3  # OUT (n), A
    pc += 1
    rom[pc] = 0xBE  # VDP data port
    pc += 1
    rom[pc] = 0x10  # DJNZ (relative jump)
    pc += 1
    offset = tile0_loop - (pc + 1)
    rom[pc] = offset & 0xFF  # Relative offset (signed byte)
    pc += 1
    
    # Write tile 1: All black pixels (32 bytes of $00)
    rom[pc] = 0x06  # LD B, n
    pc += 1
    rom[pc] = 32  # 32 bytes
    pc += 1
    rom[pc] = 0x3E  # LD A, n
    pc += 1
    rom[pc] = 0x00  # Black pixel
    pc += 1
    # Loop start
    tile1_loop = pc
    rom[pc] = 0xD3  # OUT (n), A
    pc += 1
    rom[pc] = 0xBE  # VDP data port
    pc += 1
    rom[pc] = 0x10  # DJNZ
    pc += 1
    offset = tile1_loop - (pc + 1)
    rom[pc] = offset & 0xFF  # Relative offset (signed byte)
    pc += 1
    
    # Set VRAM write address to $3800 (name table)
    rom[pc] = 0x3E  # LD A, n
    pc += 1
    rom[pc] = 0x00  # Low byte
    pc += 1
    rom[pc] = 0xD3  # OUT (n), A
    pc += 1
    rom[pc] = 0xBF  # VDP control port
    pc += 1
    rom[pc] = 0x3E  # LD A, n
    pc += 1
    rom[pc] = 0x78  # High byte (0x3800 with VRAM write bit = 0x78)
    pc += 1
    rom[pc] = 0xD3  # OUT (n), A
    pc += 1
    rom[pc] = 0xBF  # VDP control port
    pc += 1
    
    # Fill name table with checkerboard pattern (32x24 tiles)
    # Outer loop: 24 rows
    rom[pc] = 0x16  # LD D, n
    pc += 1
    rom[pc] = 24  # 24 rows
    pc += 1
    
    row_loop = pc
    # Inner loop: 32 columns
    rom[pc] = 0x0E  # LD C, n
    pc += 1
    rom[pc] = 32  # 32 columns
    pc += 1
    
    # Get pattern: alternating based on (row + col) & 1
    rom[pc] = 0x3E  # LD A, n
    pc += 1
    rom[pc] = 0x00  # Start with tile 0
    pc += 1
    
    col_loop = pc
    rom[pc] = 0xD3  # OUT (n), A
    pc += 1
    rom[pc] = 0xBE  # VDP data port
    pc += 1
    rom[pc] = 0xEE  # XOR n
    pc += 1
    rom[pc] = 0x01  # Toggle between 0 and 1
    pc += 1
    rom[pc] = 0x0D  # DEC C
    pc += 1
    rom[pc] = 0x20  # JR NZ, offset
    pc += 1
    offset = col_loop - (pc + 1)
    rom[pc] = offset & 0xFF  # Relative offset (signed byte)
    pc += 1
    
    # Next row
    rom[pc] = 0x15  # DEC D
    pc += 1
    rom[pc] = 0x20  # JR NZ, offset
    pc += 1
    offset = row_loop - (pc + 1)
    rom[pc] = offset & 0xFF  # Relative offset (signed byte)
    pc += 1
    
    # Set palette: White = $3F (max brightness), Black = $00
    # CRAM write address $0000
    rom[pc] = 0x3E  # LD A, n
    pc += 1
    rom[pc] = 0x00  # Low byte
    pc += 1
    rom[pc] = 0xD3  # OUT (n), A
    pc += 1
    rom[pc] = 0xBF  # VDP control port
    pc += 1
    rom[pc] = 0x3E  # LD A, n
    pc += 1
    rom[pc] = 0xC0  # CRAM write (0xC0)
    pc += 1
    rom[pc] = 0xD3  # OUT (n), A
    pc += 1
    rom[pc] = 0xBF  # VDP control port
    pc += 1
    
    # Write palette entries
    # Entry 0: White (RGB: 3,3,3)
    rom[pc] = 0x3E  # LD A, n
    pc += 1
    rom[pc] = 0x3F  # White
    pc += 1
    rom[pc] = 0xD3  # OUT (n), A
    pc += 1
    rom[pc] = 0xBE  # VDP data port
    pc += 1
    
    # Entry 1: Black (RGB: 0,0,0)
    rom[pc] = 0x3E  # LD A, n
    pc += 1
    rom[pc] = 0x00  # Black
    pc += 1
    rom[pc] = 0xD3  # OUT (n), A
    pc += 1
    rom[pc] = 0xBE  # VDP data port
    pc += 1
    
    # Infinite loop
    loop_addr = pc
    rom[pc] = 0x76  # HALT
    pc += 1
    rom[pc] = 0x18  # JR offset
    pc += 1
    offset = loop_addr - (pc + 1)
    rom[pc] = offset & 0xFF  # Jump back to HALT (signed byte)
    pc += 1
    
    return pc

def add_sms_header(rom):
    """Add TMR SEGA header at 0x7FF0"""
    header_offset = 0x7FF0
    
    # TMR SEGA signature
    rom[header_offset:header_offset + 8] = b"TMR SEGA"
    
    # Reserved bytes
    rom[header_offset + 8] = 0x00
    rom[header_offset + 9] = 0x00
    
    # Checksum (we'll just put 0x0000 for now)
    rom[header_offset + 10] = 0x00
    rom[header_offset + 11] = 0x00
    
    # Product code and version
    rom[header_offset + 12] = 0x00
    rom[header_offset + 13] = 0x00
    rom[header_offset + 14] = 0x00
    
    # Region code and ROM size
    rom[header_offset + 15] = 0x4C  # Export, 32KB

def main():
    # Create a 32KB ROM (minimum SMS size)
    rom_size = 32 * 1024
    rom = bytearray(rom_size)
    
    # Fill with NOP instructions by default (0x00)
    for i in range(rom_size):
        rom[i] = 0x00
    
    # Assemble the code
    code_size = assemble_z80(rom)
    print(f"Code size: {code_size} bytes")
    
    # Add SMS header
    add_sms_header(rom)
    
    # Write to file
    with open("test.sms", "wb") as f:
        f.write(rom)
    
    print(f"Created test.sms ({rom_size} bytes)")

if __name__ == "__main__":
    main()
