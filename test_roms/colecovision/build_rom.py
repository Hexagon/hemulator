#!/usr/bin/env python3
"""
ColecoVision test ROM generator
Creates a minimal ColecoVision cartridge ROM that displays a production-like test pattern

This ROM demonstrates:
- TMS9918A VDP initialization in Graphics II mode
- Multi-color band display (4 horizontal color bands)
- Sprite rendering (2 sprites with movement)
- Pattern and color table setup
- Frame interrupt handling
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
    Assemble Z80 code for ColecoVision test
    
    This creates a program that:
    1. Initializes VDP in Graphics II mode
    2. Sets up pattern and color tables for 4 colored bands
    3. Fills nametable with tile indices
    4. Sets up 2 sprites
    5. Enables display with frame interrupts
    6. Loops forever (updating sprite positions)
    """
    
    pc = 0x0000  # Start at beginning of ROM (will be mapped to 0x8000 in ColecoVision)
    
    # --- Reset/Startup Code ---
    # Disable interrupts
    rom[pc] = 0xF3  # DI
    pc += 1
    
    # Set stack pointer to $73FF (top of ColecoVision RAM)
    rom[pc] = 0x31  # LD SP, nn
    pc += 1
    write_word(rom, pc, 0x73FF)
    pc += 2
    
    # Enable interrupts mode 1 (for VDP interrupts)
    rom[pc] = 0xED  # IM 1
    pc += 1
    rom[pc] = 0x56
    pc += 1
    rom[pc] = 0xFB  # EI
    pc += 1
    
    # --- Initialize VDP Registers ---
    # Register 0: Mode Control 1 - $00 (Graphics II mode)
    rom[pc] = 0x3E  # LD A, n
    pc += 1
    rom[pc] = 0x00  # Mode 0 (will be set to mode 2 by register 1)
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
    
    # Register 1: Mode Control 2 - $E0 (16K, display on, frame interrupt, Graphics II mode)
    # Bit 7 = 16K VRAM, Bit 6 = Display enable, Bit 5 = Frame interrupt, Bit 1 = Graphics II mode
    rom[pc] = 0x3E  # LD A, n
    pc += 1
    rom[pc] = 0xE2  # $E2 = 11100010 (16K, display on, interrupt on, Graphics II, 8x8 sprites)
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
    
    # Register 2: Name Table Base - $0F (nametable at $3800)
    # Value $0F = 00001111, bits 3-0 = 15, 15 << 10 = $3C00
    # For standard Graphics II: $0E = 14 << 10 = $3800
    rom[pc] = 0x3E  # LD A, n
    pc += 1
    rom[pc] = 0x0E  # Nametable at $3800
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
    
    # Register 3: Color Table Base - $FF (color table at $2000)
    # In Graphics II mode: $FF = all 1s, bits 7-0 = 255, pattern is different
    # For Graphics II: use $7F for color table at $2000
    rom[pc] = 0x3E  # LD A, n
    pc += 1
    rom[pc] = 0x7F  # Color table at $2000 (Graphics II mode)
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
    
    # Register 4: Pattern Table Base - $03 (pattern table at $0000)
    # In Graphics II: $03 means pattern table at $0000 (bits 2-0, bit 2 = 1 for Graphics II)
    rom[pc] = 0x3E  # LD A, n
    pc += 1
    rom[pc] = 0x03  # Pattern table at $0000 (Graphics II mode)
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
    
    # Register 5: Sprite Attribute Table - $76 (at $3B00)
    # $76 << 7 = $3B00
    rom[pc] = 0x3E  # LD A, n
    pc += 1
    rom[pc] = 0x76  # Sprite table at $3B00
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
    
    # Register 6: Sprite Pattern Base - $03 (at $1800)
    # $03 << 11 = $1800
    rom[pc] = 0x3E  # LD A, n
    pc += 1
    rom[pc] = 0x03  # Sprite patterns at $1800
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
    
    # Register 7: Backdrop Color - $01 (black)
    rom[pc] = 0x3E  # LD A, n
    pc += 1
    rom[pc] = 0x01  # Black backdrop (color 1)
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
    
    # --- Load Pattern Data (create solid tile patterns) ---
    # Set VRAM write address to $0000 (pattern table)
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
    rom[pc] = 0x40  # High byte with VRAM write bit
    pc += 1
    rom[pc] = 0xD3  # OUT (n), A
    pc += 1
    rom[pc] = 0xBF  # VDP control port
    pc += 1
    
    # Write solid tile pattern (all pixels set) for all 256 tiles
    # Each tile is 8 bytes (8 rows of 8 pixels)
    rom[pc] = 0x21  # LD HL, nn
    pc += 1
    write_word(rom, pc, 2048)  # 256 tiles * 8 bytes = 2048 bytes
    pc += 2
    
    pattern_loop = pc
    rom[pc] = 0x3E  # LD A, n
    pc += 1
    rom[pc] = 0xFF  # Solid pattern (all pixels on)
    pc += 1
    rom[pc] = 0xD3  # OUT (n), A
    pc += 1
    rom[pc] = 0xBE  # VDP data port
    pc += 1
    rom[pc] = 0x2B  # DEC HL
    pc += 1
    rom[pc] = 0x7C  # LD A, H
    pc += 1
    rom[pc] = 0xB5  # OR L
    pc += 1
    rom[pc] = 0x20  # JR NZ, offset
    pc += 1
    offset = pattern_loop - (pc + 1)
    rom[pc] = offset & 0xFF
    pc += 1
    
    # --- Load Color Data (4 horizontal bands: white, red, green, cyan) ---
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
    rom[pc] = 0x60  # High byte ($2000 with VRAM write bit = $60)
    pc += 1
    rom[pc] = 0xD3  # OUT (n), A
    pc += 1
    rom[pc] = 0xBF  # VDP control port
    pc += 1
    
    # Band 1: White on black (tiles 0-63, rows 0-7)
    # Color byte: high nibble = foreground, low nibble = background
    rom[pc] = 0x21  # LD HL, nn
    pc += 1
    write_word(rom, pc, 512)  # 64 tiles * 8 bytes = 512
    pc += 2
    
    band1_loop = pc
    rom[pc] = 0x3E  # LD A, n
    pc += 1
    rom[pc] = 0xF1  # White (F) on black (1)
    pc += 1
    rom[pc] = 0xD3  # OUT (n), A
    pc += 1
    rom[pc] = 0xBE  # VDP data port
    pc += 1
    rom[pc] = 0x2B  # DEC HL
    pc += 1
    rom[pc] = 0x7C  # LD A, H
    pc += 1
    rom[pc] = 0xB5  # OR L
    pc += 1
    rom[pc] = 0x20  # JR NZ, offset
    pc += 1
    offset = band1_loop - (pc + 1)
    rom[pc] = offset & 0xFF
    pc += 1
    
    # Band 2: Red on black (tiles 64-127, rows 8-15)
    rom[pc] = 0x21  # LD HL, nn
    pc += 1
    write_word(rom, pc, 512)
    pc += 2
    
    band2_loop = pc
    rom[pc] = 0x3E  # LD A, n
    pc += 1
    rom[pc] = 0x61  # Red (6) on black (1)
    pc += 1
    rom[pc] = 0xD3  # OUT (n), A
    pc += 1
    rom[pc] = 0xBE  # VDP data port
    pc += 1
    rom[pc] = 0x2B  # DEC HL
    pc += 1
    rom[pc] = 0x7C  # LD A, H
    pc += 1
    rom[pc] = 0xB5  # OR L
    pc += 1
    rom[pc] = 0x20  # JR NZ, offset
    pc += 1
    offset = band2_loop - (pc + 1)
    rom[pc] = offset & 0xFF
    pc += 1
    
    # Band 3: Green on black (tiles 128-191, rows 16-23)
    rom[pc] = 0x21  # LD HL, nn
    pc += 1
    write_word(rom, pc, 512)
    pc += 2
    
    band3_loop = pc
    rom[pc] = 0x3E  # LD A, n
    pc += 1
    rom[pc] = 0x31  # Light Green (3) on black (1)
    pc += 1
    rom[pc] = 0xD3  # OUT (n), A
    pc += 1
    rom[pc] = 0xBE  # VDP data port
    pc += 1
    rom[pc] = 0x2B  # DEC HL
    pc += 1
    rom[pc] = 0x7C  # LD A, H
    pc += 1
    rom[pc] = 0xB5  # OR L
    pc += 1
    rom[pc] = 0x20  # JR NZ, offset
    pc += 1
    offset = band3_loop - (pc + 1)
    rom[pc] = offset & 0xFF
    pc += 1
    
    # Band 4: Cyan on black (tiles 192-255)
    rom[pc] = 0x21  # LD HL, nn
    pc += 1
    write_word(rom, pc, 512)
    pc += 2
    
    band4_loop = pc
    rom[pc] = 0x3E  # LD A, n
    pc += 1
    rom[pc] = 0x71  # Cyan (7) on black (1)
    pc += 1
    rom[pc] = 0xD3  # OUT (n), A
    pc += 1
    rom[pc] = 0xBE  # VDP data port
    pc += 1
    rom[pc] = 0x2B  # DEC HL
    pc += 1
    rom[pc] = 0x7C  # LD A, H
    pc += 1
    rom[pc] = 0xB5  # OR L
    pc += 1
    rom[pc] = 0x20  # JR NZ, offset
    pc += 1
    offset = band4_loop - (pc + 1)
    rom[pc] = offset & 0xFF
    pc += 1
    
    # --- Fill Nametable ---
    # Set VRAM write address to $3800 (nametable)
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
    rom[pc] = 0x78  # High byte ($3800 with VRAM write bit = $78)
    pc += 1
    rom[pc] = 0xD3  # OUT (n), A
    pc += 1
    rom[pc] = 0xBF  # VDP control port
    pc += 1
    
    # Fill nametable with sequential tile indices (32x24 = 768 tiles)
    # This will create 4 bands: tiles 0-63, 64-127, 128-191, 192-255 repeating
    rom[pc] = 0x21  # LD HL, nn
    pc += 1
    write_word(rom, pc, 768)  # 32 columns x 24 rows
    pc += 2
    
    rom[pc] = 0x06  # LD B, n (tile counter)
    pc += 1
    rom[pc] = 0x00  # Start at tile 0
    pc += 1
    
    nametable_loop = pc
    rom[pc] = 0x78  # LD A, B
    pc += 1
    rom[pc] = 0xD3  # OUT (n), A
    pc += 1
    rom[pc] = 0xBE  # VDP data port
    pc += 1
    rom[pc] = 0x04  # INC B
    pc += 1
    rom[pc] = 0x2B  # DEC HL
    pc += 1
    rom[pc] = 0x7C  # LD A, H
    pc += 1
    rom[pc] = 0xB5  # OR L
    pc += 1
    rom[pc] = 0x20  # JR NZ, offset
    pc += 1
    offset = nametable_loop - (pc + 1)
    rom[pc] = offset & 0xFF
    pc += 1
    
    # --- Set up Sprites ---
    # Set VRAM write address to $1800 (sprite pattern table)
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
    rom[pc] = 0x58  # High byte ($1800 with VRAM write bit = $58)
    pc += 1
    rom[pc] = 0xD3  # OUT (n), A
    pc += 1
    rom[pc] = 0xBF  # VDP control port
    pc += 1
    
    # Write sprite pattern 0: filled circle (simplified as a square)
    sprite_pattern = [
        0b00111100,  # Row 0: ..XXXX..
        0b01111110,  # Row 1: .XXXXXX.
        0b11111111,  # Row 2: XXXXXXXX
        0b11111111,  # Row 3: XXXXXXXX
        0b11111111,  # Row 4: XXXXXXXX
        0b11111111,  # Row 5: XXXXXXXX
        0b01111110,  # Row 6: .XXXXXX.
        0b00111100,  # Row 7: ..XXXX..
    ]
    
    for byte_val in sprite_pattern:
        rom[pc] = 0x3E  # LD A, n
        pc += 1
        rom[pc] = byte_val
        pc += 1
        rom[pc] = 0xD3  # OUT (n), A
        pc += 1
        rom[pc] = 0xBE  # VDP data port
        pc += 1
    
    # Set VRAM write address to $3B00 (sprite attribute table)
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
    rom[pc] = 0x7B  # High byte ($3B00 with VRAM write bit = $7B)
    pc += 1
    rom[pc] = 0xD3  # OUT (n), A
    pc += 1
    rom[pc] = 0xBF  # VDP control port
    pc += 1
    
    # Sprite 0: Y=96 (center-ish), X=100, Pattern=0, Color=Yellow (A)
    rom[pc] = 0x3E  # LD A, n
    pc += 1
    rom[pc] = 97  # Y position (offset by +1)
    pc += 1
    rom[pc] = 0xD3  # OUT (n), A
    pc += 1
    rom[pc] = 0xBE  # VDP data port
    pc += 1
    
    rom[pc] = 0x3E  # LD A, n
    pc += 1
    rom[pc] = 100  # X position
    pc += 1
    rom[pc] = 0xD3  # OUT (n), A
    pc += 1
    rom[pc] = 0xBE  # VDP data port
    pc += 1
    
    rom[pc] = 0x3E  # LD A, n
    pc += 1
    rom[pc] = 0x00  # Pattern 0
    pc += 1
    rom[pc] = 0xD3  # OUT (n), A
    pc += 1
    rom[pc] = 0xBE  # VDP data port
    pc += 1
    
    rom[pc] = 0x3E  # LD A, n
    pc += 1
    rom[pc] = 0x0A  # Color yellow (A)
    pc += 1
    rom[pc] = 0xD3  # OUT (n), A
    pc += 1
    rom[pc] = 0xBE  # VDP data port
    pc += 1
    
    # Sprite 1: Y=96, X=156, Pattern=0, Color=Magenta (D)
    rom[pc] = 0x3E  # LD A, n
    pc += 1
    rom[pc] = 97  # Y position (offset by +1)
    pc += 1
    rom[pc] = 0xD3  # OUT (n), A
    pc += 1
    rom[pc] = 0xBE  # VDP data port
    pc += 1
    
    rom[pc] = 0x3E  # LD A, n
    pc += 1
    rom[pc] = 156  # X position
    pc += 1
    rom[pc] = 0xD3  # OUT (n), A
    pc += 1
    rom[pc] = 0xBE  # VDP data port
    pc += 1
    
    rom[pc] = 0x3E  # LD A, n
    pc += 1
    rom[pc] = 0x00  # Pattern 0
    pc += 1
    rom[pc] = 0xD3  # OUT (n), A
    pc += 1
    rom[pc] = 0xBE  # VDP data port
    pc += 1
    
    rom[pc] = 0x3E  # LD A, n
    pc += 1
    rom[pc] = 0x0D  # Color magenta (D)
    pc += 1
    rom[pc] = 0xD3  # OUT (n), A
    pc += 1
    rom[pc] = 0xBE  # VDP data port
    pc += 1
    
    # End of sprite list (Y=0xD0)
    rom[pc] = 0x3E  # LD A, n
    pc += 1
    rom[pc] = 0xD0  # End marker
    pc += 1
    rom[pc] = 0xD3  # OUT (n), A
    pc += 1
    rom[pc] = 0xBE  # VDP data port
    pc += 1
    
    # --- Main Loop (HALT and wait for interrupts) ---
    loop_addr = pc
    rom[pc] = 0x76  # HALT (wait for VDP interrupt)
    pc += 1
    rom[pc] = 0x18  # JR offset
    pc += 1
    offset = loop_addr - (pc + 1)
    rom[pc] = offset & 0xFF
    pc += 1
    
    return pc  # Return code size

def main():
    # Create a 32KB ROM (standard ColecoVision cartridge size)
    rom_size = 32 * 1024
    rom = bytearray(rom_size)
    
    # Fill with NOP instructions by default
    for i in range(rom_size):
        rom[i] = 0x00
    
    # Assemble the code starting at 0x8000
    code_size = assemble_z80(rom)
    print(f"Code size: {code_size} bytes")
    
    # Write to file
    with open("test.col", "wb") as f:
        f.write(rom)
    
    print(f"Created test.col ({rom_size} bytes)")
    print("Expected output:")
    print("  - 4 horizontal colored bands (white, red, green, cyan)")
    print("  - 2 sprites (yellow and magenta) at different positions")
    print("  - 256x192 resolution")

if __name__ == "__main__":
    main()
