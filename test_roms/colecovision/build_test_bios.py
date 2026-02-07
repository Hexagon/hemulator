#!/usr/bin/env python3
"""
ColecoVision minimal test BIOS generator

This creates a minimal BIOS ROM that initializes the system and jumps to the cartridge.
This is ONLY for testing purposes - real ColecoVision systems use proprietary BIOS.

The BIOS:
1. Sets up the stack pointer
2. Initializes interrupt mode 1
3. Jumps to cartridge ROM at 0x8000

Size: 8KB (0x2000 bytes)
"""

def create_test_bios():
    """Create a minimal test BIOS that just jumps to cartridge"""
    # Create 8KB BIOS filled with zeros
    bios = bytearray(0x2000)
    
    pc = 0x0000
    
    # --- Reset vector at 0x0000 ---
    # DI - Disable interrupts
    bios[pc] = 0xF3
    pc += 1
    
    # LD SP, $73FF - Set stack pointer to top of RAM
    bios[pc] = 0x31  # LD SP, nn
    pc += 1
    bios[pc] = 0xFF  # Low byte of 0x73FF
    pc += 1
    bios[pc] = 0x73  # High byte of 0x73FF
    pc += 1
    
    # IM 1 - Set interrupt mode 1
    bios[pc] = 0xED
    pc += 1
    bios[pc] = 0x56
    pc += 1
    
    # EI - Enable interrupts
    bios[pc] = 0xFB
    pc += 1
    
    # JP $8000 - Jump to cartridge ROM
    bios[pc] = 0xC3  # JP nn
    pc += 1
    bios[pc] = 0x00  # Low byte of 0x8000
    pc += 1
    bios[pc] = 0x80  # High byte of 0x8000
    pc += 1
    
    # --- Interrupt vector at 0x0038 (for IM 1) ---
    # This is where VDP interrupts go in IM 1
    pc = 0x0038
    
    # RETI - Return from interrupt
    bios[pc] = 0xED
    pc += 1
    bios[pc] = 0x4D
    pc += 1
    
    # --- NMI vector at 0x0066 ---
    pc = 0x0066
    
    # RETN - Return from NMI
    bios[pc] = 0xED
    pc += 1
    bios[pc] = 0x45
    pc += 1
    
    return bytes(bios)

def main():
    print("Creating minimal test BIOS for ColecoVision...")
    bios = create_test_bios()
    
    # Write to file
    with open('test_bios.rom', 'wb') as f:
        f.write(bios)
    
    print(f"Created test_bios.rom ({len(bios)} bytes)")
    print("This BIOS:")
    print("  - Initializes stack pointer to $73FF")
    print("  - Sets interrupt mode 1")
    print("  - Jumps to cartridge ROM at $8000")
    print("  - Provides interrupt handlers at $0038 (IM 1) and $0066 (NMI)")

if __name__ == '__main__':
    main()
