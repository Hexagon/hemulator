#!/usr/bin/env python3
"""
SG-1000 test ROM generator
Creates a minimal Sega SG-1000 ROM that displays a checkerboard pattern using TMS9918A VDP
"""

def write_byte(rom, addr, value):
    """Write a byte to ROM at address"""
    rom[addr] = value & 0xFF

def write_word(rom, addr, value):
    """Write a 16-bit word to ROM (little-endian)"""
    rom[addr] = value & 0xFF
    rom[addr + 1] = (value >> 8) & 0xFF

def assemble_z80(rom):
    """
    Assemble basic Z80 code for SG-1000 test
    
    This creates a minimal program that:
    1. Disables interrupts
    2. Sets up stack pointer
    3. Initializes VDP registers (Graphics I mode)
    4. Loads checkerboard tile data into VRAM
    5. Fills tilemap with alternating tiles
    6. Sets colors
    7. Loops forever
    """
    
    pc = 0x0000  # Start at address 0
    
    # Disable interrupts
    rom[pc] = 0xF3  # DI
    pc += 1
    
    # Set stack pointer to $C3F0 (in RAM area)
    rom[pc] = 0x31  # LD SP, nn
    pc += 1
    write_word(rom, pc, 0xC3F0)
    pc += 2
    
    # Initialize VDP registers for Graphics I mode
    # Register 0: Mode Control 1 - $00 (Graphics I mode, no external video)
    rom[pc] = 0x3E  # LD A, n
    pc += 1
    rom[pc] = 0x00  # Mode control value
    pc += 1
    rom[pc] = 0xD3  # OUT (n), A
    pc += 1
    rom[pc] = 0xBF  # VDP control port (odd port in 0x80-0xFF range)
    pc += 1
    rom[pc] = 0x3E  # LD A, n
    pc += 1
    rom[pc] = 0x80  # Register write command (register 0)
    pc += 1
    rom[pc] = 0xD3  # OUT (n), A
    pc += 1
    rom[pc] = 0xBF  # VDP control port
    pc += 1
    
    # Register 1: Mode Control 2 - $E0 (16K, enable display, enable VDP interrupt generation, Graphics I)
    rom[pc] = 0x3E  # LD A, n
    pc += 1
    rom[pc] = 0xE0  # Mode control value (bit 6 = 1 for display on; CPU interrupts remain disabled due to DI)
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
    
    # Register 2: Name Table Base Address - $06 (at $1800)
    rom[pc] = 0x3E  # LD A, n
    pc += 1
    rom[pc] = 0x06  # Name table at $1800 (0x06 << 10)
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
    
    # Register 3: Color Table Base - $FF (at $2000 for Graphics I)
    rom[pc] = 0x3E  # LD A, n
    pc += 1
    rom[pc] = 0xFF  # Color table at $2000
    pc += 1
    rom[pc] = 0xD3  # OUT (n), A
    pc += 1
    rom[pc] = 0xBF  # VDP control port
    pc += 1
    rom[pc] = 0x3E  # LD A, n
    pc += 1
    rom[pc] = 0x83  # Register write command (register 3)
    pc += 1
    rom[pc] = 0xD3  # OUT (n), A
    pc += 1
    rom[pc] = 0xBF  # VDP control port
    pc += 1
    
    # Register 4: Pattern Table Base - $01 (at $800)
    rom[pc] = 0x3E  # LD A, n
    pc += 1
    rom[pc] = 0x01  # Pattern table at $800
    pc += 1
    rom[pc] = 0xD3  # OUT (n), A
    pc += 1
    rom[pc] = 0xBF  # VDP control port
    pc += 1
    rom[pc] = 0x3E  # LD A, n
    pc += 1
    rom[pc] = 0x84  # Register write command (register 4)
    pc += 1
    rom[pc] = 0xD3  # OUT (n), A
    pc += 1
    rom[pc] = 0xBF  # VDP control port
    pc += 1
    
    # Register 5: Sprite Attribute Table - $36 (at $1B00)
    rom[pc] = 0x3E  # LD A, n
    pc += 1
    rom[pc] = 0x36  # Sprite attribute at $1B00
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
    
    # Register 6: Sprite Pattern Table - $07 (at $3800)
    rom[pc] = 0x3E  # LD A, n
    pc += 1
    rom[pc] = 0x07  # Sprite pattern at $3800
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
    
    # Register 7: Foreground/Background Color - $F4 (white on dark blue)
    rom[pc] = 0x3E  # LD A, n
    pc += 1
    rom[pc] = 0xF4  # FG=white(F), BG=dark blue(4)
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
    
    # Set VRAM write address to $0800 (pattern table start)
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
    rom[pc] = 0x48  # High byte with VRAM write bit (0x0800 -> 0x48)
    pc += 1
    rom[pc] = 0xD3  # OUT (n), A
    pc += 1
    rom[pc] = 0xBF  # VDP control port
    pc += 1
    
    # Write pattern 0: Checkerboard pattern (alternating pixels)
    # Pattern: 0xAA (10101010) for each row
    rom[pc] = 0x06  # LD B, n
    pc += 1
    rom[pc] = 8  # 8 rows
    pc += 1
    rom[pc] = 0x3E  # LD A, n
    pc += 1
    rom[pc] = 0xAA  # Checkerboard pattern
    pc += 1
    # Loop start
    pattern0_loop = pc
    rom[pc] = 0xD3  # OUT (n), A
    pc += 1
    rom[pc] = 0xBE  # VDP data port (even port in 0x80-0xFF)
    pc += 1
    rom[pc] = 0x10  # DJNZ (decrement B and loop if not zero)
    pc += 1
    offset = pattern0_loop - (pc + 1)
    rom[pc] = offset & 0xFF  # Relative offset (signed byte)
    pc += 1
    
    # Write pattern 1: Inverse checkerboard (0x55)
    rom[pc] = 0x06  # LD B, n
    pc += 1
    rom[pc] = 8  # 8 rows
    pc += 1
    rom[pc] = 0x3E  # LD A, n
    pc += 1
    rom[pc] = 0x55  # Inverse checkerboard
    pc += 1
    # Loop start
    pattern1_loop = pc
    rom[pc] = 0xD3  # OUT (n), A
    pc += 1
    rom[pc] = 0xBE  # VDP data port
    pc += 1
    rom[pc] = 0x10  # DJNZ
    pc += 1
    offset = pattern1_loop - (pc + 1)
    rom[pc] = offset & 0xFF  # Relative offset (signed byte)
    pc += 1
    
    # Set VRAM write address to $2000 (color table)
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
    rom[pc] = 0x60  # High byte (0x2000 with VRAM write bit = 0x60)
    pc += 1
    rom[pc] = 0xD3  # OUT (n), A
    pc += 1
    rom[pc] = 0xBF  # VDP control port
    pc += 1
    
    # Write color table: white on black for both patterns
    rom[pc] = 0x06  # LD B, n
    pc += 1
    rom[pc] = 32  # 32 bytes (covers all pattern positions)
    pc += 1
    rom[pc] = 0x3E  # LD A, n
    pc += 1
    rom[pc] = 0xF1  # White (F) on black (1)
    pc += 1
    # Loop start
    color_loop = pc
    rom[pc] = 0xD3  # OUT (n), A
    pc += 1
    rom[pc] = 0xBE  # VDP data port
    pc += 1
    rom[pc] = 0x10  # DJNZ
    pc += 1
    offset = color_loop - (pc + 1)
    rom[pc] = offset & 0xFF  # Relative offset (signed byte)
    pc += 1
    
    # Set VRAM write address to $1800 (name table)
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
    rom[pc] = 0x58  # High byte (0x1800 with VRAM write bit = 0x58)
    pc += 1
    rom[pc] = 0xD3  # OUT (n), A
    pc += 1
    rom[pc] = 0xBF  # VDP control port
    pc += 1
    
    # Fill name table with checkerboard pattern (32x24 characters)
    # Use HL as counter for total characters (768 = 0x0300)
    rom[pc] = 0x21  # LD HL, nn
    pc += 1
    write_word(rom, pc, 768)  # 32 x 24 chars
    pc += 2
    
    # Use B as toggle state (start with 0)
    rom[pc] = 0x06  # LD B, n
    pc += 1
    rom[pc] = 0x00  # Start with pattern 0
    pc += 1
    
    name_loop = pc
    # Write character index (B register)
    rom[pc] = 0x78  # LD A, B
    pc += 1
    rom[pc] = 0xD3  # OUT (n), A
    pc += 1
    rom[pc] = 0xBE  # VDP data port
    pc += 1
    
    # Toggle B between 0 and 1
    rom[pc] = 0x78  # LD A, B
    pc += 1
    rom[pc] = 0xEE  # XOR n
    pc += 1
    rom[pc] = 0x01  # XOR with 1
    pc += 1
    rom[pc] = 0x47  # LD B, A (save back to B)
    pc += 1
    
    # Decrement counter HL
    rom[pc] = 0x2B  # DEC HL
    pc += 1
    
    # Check if HL == 0
    rom[pc] = 0x7C  # LD A, H
    pc += 1
    rom[pc] = 0xB5  # OR L
    pc += 1
    rom[pc] = 0x20  # JR NZ, offset
    pc += 1
    offset = name_loop - (pc + 1)
    rom[pc] = offset & 0xFF  # Relative offset (signed byte)
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

def main():
    # Create a 32KB ROM (standard size for SG-1000)
    rom_size = 32 * 1024
    rom = bytearray(rom_size)
    
    # Fill with NOP instructions by default (0x00)
    for i in range(rom_size):
        rom[i] = 0x00
    
    # Assemble the code
    code_size = assemble_z80(rom)
    print(f"Code size: {code_size} bytes")
    
    # Write to file
    with open("test.sg", "wb") as f:
        f.write(rom)
    
    print(f"Created test.sg ({rom_size} bytes)")

if __name__ == "__main__":
    main()
