// Quick test to see what the generated BIOS looks like

fn main() {
    // We need to import from emu_pc, but we can't easily do that from a standalone file
    // Let's just manually create the BIOS using the same logic
    
    let mut bios = vec![0x00; 0x10000]; // 64KB
    
    // Copy the init code
    let init_code: Vec<u8> = vec![
        0xFA, // CLI - disable interrupts
        0xB8, 0x00, 0x00, // MOV AX, 0x0000
        0x8E, 0xD8, // MOV DS, AX
        0x8E, 0xC0, // MOV ES, AX
        0x8E, 0xD0, // MOV SS, AX
        0xBC, 0xFE, 0xFF, // MOV SP, 0xFFFE
        // Set up interrupt vectors
        // INT 0x00 (Divide Error) at 0x0000
        0xB8, 0x50, 0x00, // MOV AX, 0x0050
        0xA3, 0x00, 0x00, // MOV [0x0000], AX
        0xB8, 0x00, 0xF0, // MOV AX, 0xF000
        0xA3, 0x02, 0x00, // MOV [0x0002], AX
        // INT 0x10 (Video Services) at 0x0040
        0xB8, 0x00, 0x01, // MOV AX, 0x0100
        0xA3, 0x40, 0x00, // MOV [0x0040], AX
        0xB8, 0x00, 0xF0, // MOV AX, 0xF000
        0xA3, 0x42, 0x00, // MOV [0x0042], AX
        // INT 0x13 (Disk Services) at 0x004C
        0xB8, 0x00, 0x02, // MOV AX, 0x0200
        0xA3, 0x4C, 0x00, // MOV [0x004C], AX
        0xB8, 0x00, 0xF0, // MOV AX, 0xF000
        0xA3, 0x4E, 0x00, // MOV [0x004E], AX
        // INT 0x16 (Keyboard Services) at 0x0058
        0xB8, 0x00, 0x03, // MOV AX, 0x0300
        0xA3, 0x58, 0x00, // MOV [0x0058], AX
        0xB8, 0x00, 0xF0, // MOV AX, 0xF000
        0xA3, 0x5A, 0x00, // MOV [0x005A], AX
        // INT 0x21 (DOS Services) at 0x0084
        0xB8, 0x00, 0x04, // MOV AX, 0x0400
        0xA3, 0x84, 0x00, // MOV [0x0084], AX
        0xB8, 0x00, 0xF0, // MOV AX, 0xF000
        0xA3, 0x86, 0x00, // MOV [0x0086], AX
        0xFB, // STI - enable interrupts
        // Check if boot sector is loaded
        0xBE, 0xFE, 0x7D, // MOV SI, 0x7DFE
        0x8A, 0x04, // MOV AL, [SI]
        0x3C, 0x55, // CMP AL, 0x55
        0x75, 0x0C, // JNZ skip_boot
        0x8A, 0x44, 0x01, // MOV AL, [SI+1]
        0x3C, 0xAA, // CMP AL, 0xAA
        0x75, 0x09, // JNZ skip_boot
        // Boot signature valid
        0xB2, 0x00, // MOV DL, 0x00
        0xB6, 0x00, // MOV DH, 0x00
        0xEA, 0x00, 0x7C, 0x00, 0x00, // JMP FAR 0x0000:0x7C00
        // skip_boot: infinite loop
        0xEB, 0xFE, // JMP -2 (infinite loop)
    ];
    
    println!("Init code length: {} bytes (0x{:02X})", init_code.len(), init_code.len());
    println!("\nByte at offset 0x63 (99): 0x{:02X}", init_code[0x63]);
    println!("Bytes at 0x60-0x68:");
    for i in 0x60..=0x68 {
        print!("  0x{:02X}: 0x{:02X}", i, init_code[i]);
        // Try to decode instruction
        match init_code[i] {
            0xA3 => println!(" (MOV [addr], AX)"),
            0xB8 => println!(" (MOV AX, imm16)"),
            0xEB => println!(" (JMP short)"),
            0xFB => println!(" (STI)"),
            0xBE => println!(" (MOV SI, imm16)"),
            _ => println!(),
        }
    }
    
    // Find where the infinite loop is
    for i in 0..init_code.len()-1 {
        if init_code[i] == 0xEB && init_code[i+1] == 0xFE {
            println!("\nInfinite loop (EB FE) at offset: 0x{:02X}", i);
        }
    }
}
