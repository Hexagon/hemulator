#!/usr/bin/env python3
"""
Simple CHIP-8 assembler for test ROM
Converts hex opcodes to binary format
"""

import sys
import re

def assemble_chip8(asm_file, output_file):
    """Assemble CHIP-8 asm file to binary."""
    opcodes = []
    
    with open(asm_file, 'r') as f:
        for line in f:
            # Remove comments and whitespace
            line = line.split(';')[0].strip()
            if not line:
                continue
            
            # Extract hex codes (4-digit hex numbers)
            hex_codes = re.findall(r'[0-9A-Fa-f]{4}', line)
            for hex_code in hex_codes:
                # Convert to 2 bytes (big-endian)
                value = int(hex_code, 16)
                high_byte = (value >> 8) & 0xFF
                low_byte = value & 0xFF
                opcodes.append(high_byte)
                opcodes.append(low_byte)
    
    # Write binary file
    with open(output_file, 'wb') as f:
        f.write(bytes(opcodes))
    
    print(f"Assembled {len(opcodes)} bytes from {asm_file} to {output_file}")
    print(f"Opcodes: {' '.join(f'{b:02X}' for b in opcodes)}")

if __name__ == '__main__':
    if len(sys.argv) != 3:
        print("Usage: assemble.py <input.asm> <output.ch8>")
        sys.exit(1)
    
    assemble_chip8(sys.argv[1], sys.argv[2])
