//! Comprehensive test for file reading loop patterns
//! This test simulates the kind of loop that FreeDOS uses when reading files

use crate::cpu_8086::{ArrayMemory, Cpu8086, Memory8086, FLAG_ZF};

#[test]
fn test_file_read_loop_pattern() {
    // Simulate a typical DOS file reading loop:
    // 1. Call INT 21h to read data
    // 2. Check if AX (bytes read) is zero
    // 3. If zero, exit (EOF)
    // 4. Process data
    // 5. Loop back to step 1
    
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);
    
    // Simulate the loop structure:
    // read_loop:
    //     mov ah, 0x3F        ; Read from file
    //     int 0x21            ; (simulated below)
    //     test ax, ax         ; Check if bytes read == 0
    //     jz done             ; Jump if zero (EOF)
    //     ; process data here
    //     jmp read_loop       ; Continue reading
    // done:
    //     ret
    
    cpu.memory.load_program(
        0x1000,
        &[
            // read_loop:
            0xB4, 0x3F,             // MOV AH, 0x3F         @ 0x1000
            0x90,                    // NOP (simulate INT)    @ 0x1002
            0x85, 0xC0,             // TEST AX, AX          @ 0x1003
            0x74, 0x06,             // JZ +6 (to 0x100D)    @ 0x1005
            0x90,                    // NOP (process data)    @ 0x1007
            0xEB, 0xF6,             // JMP -10 (to 0x1000)  @ 0x1008
            // done:
            0xF4,                    // HLT                  @ 0x100A
        ],
    );
    
    cpu.ip = 0x1000;
    cpu.cs = 0x0000;
    
    // First iteration: simulate reading 10 bytes
    cpu.step(); // MOV AH, 0x3F
    cpu.step(); // NOP
    cpu.ax = 10; // Simulate INT 21h returning 10 bytes read
    cpu.step(); // TEST AX, AX
    assert!(!cpu.get_flag(FLAG_ZF), "ZF should be clear when AX != 0");
    cpu.step(); // JZ (should not jump)
    assert_eq!(cpu.ip, 0x1007, "Should not jump when ZF is clear");
    cpu.step(); // NOP (process)
    cpu.step(); // JMP (should jump back)
    assert_eq!(cpu.ip, 0x1000, "Should jump back to start of loop");
    
    // Second iteration: simulate reading 0 bytes (EOF)
    cpu.step(); // MOV AH, 0x3F
    cpu.step(); // NOP
    cpu.ax = 0; // Simulate INT 21h returning 0 (EOF)
    cpu.step(); // TEST AX, AX
    assert!(cpu.get_flag(FLAG_ZF), "ZF should be set when AX == 0");
    cpu.step(); // JZ (should jump to done)
    assert_eq!(cpu.ip, 0x100D, "Should jump to done when AX == 0");
}

#[test]
fn test_loop_with_cmp_and_conditional_jump() {
    // Test a loop that uses CMP and conditional jumps
    // This simulates checking for a specific value (like EOF marker)
    
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);
    
    // Loop that reads until it finds a zero byte:
    // read_loop:
    //     mov al, [bx]        ; Read byte
    //     inc bx              ; Advance pointer
    //     cmp al, 0           ; Check if zero
    //     jz done             ; Exit if zero
    //     loop read_loop      ; Continue if CX != 0
    // done:
    
    cpu.memory.load_program(
        0x1000,
        &[
            // read_loop:
            0x8A, 0x07,             // MOV AL, [BX]         @ 0x1000
            0x43,                    // INC BX               @ 0x1002
            0x3C, 0x00,             // CMP AL, 0            @ 0x1003
            0x74, 0x02,             // JZ +2 (to 0x1009)    @ 0x1005
            0xE2, 0xF7,             // LOOP -9 (to 0x1000)  @ 0x1007
            // done:
            0xF4,                    // HLT                  @ 0x1009
        ],
    );
    
    // Setup test data: "ABC" followed by zero
    cpu.memory.write(0x2000, b'A');
    cpu.memory.write(0x2001, b'B');
    cpu.memory.write(0x2002, b'C');
    cpu.memory.write(0x2003, 0x00);
    
    cpu.ip = 0x1000;
    cpu.cs = 0x0000;
    cpu.bx = 0x2000;
    cpu.cx = 10; // Max iterations
    
    // Read 'A'
    cpu.step(); // MOV AL, [BX]
    assert_eq!(cpu.ax & 0xFF, b'A' as u32);
    cpu.step(); // INC BX
    assert_eq!(cpu.bx, 0x2001);
    cpu.step(); // CMP AL, 0
    assert!(!cpu.get_flag(FLAG_ZF), "ZF should be clear for 'A'");
    cpu.step(); // JZ (should not jump)
    assert_eq!(cpu.ip, 0x1007);
    cpu.step(); // LOOP (should loop)
    assert_eq!(cpu.ip, 0x1000, "Should loop back");
    assert_eq!(cpu.cx, 9);
    
    // Read 'B'
    cpu.step(); // MOV AL, [BX]
    assert_eq!(cpu.ax & 0xFF, b'B' as u32);
    cpu.step(); // INC BX
    cpu.step(); // CMP AL, 0
    cpu.step(); // JZ (should not jump)
    cpu.step(); // LOOP (should loop)
    assert_eq!(cpu.cx, 8);
    
    // Read 'C'
    cpu.step(); // MOV AL, [BX]
    assert_eq!(cpu.ax & 0xFF, b'C' as u32);
    cpu.step(); // INC BX
    cpu.step(); // CMP AL, 0
    cpu.step(); // JZ (should not jump)
    cpu.step(); // LOOP (should loop)
    assert_eq!(cpu.cx, 7);
    
    // Read zero byte
    cpu.step(); // MOV AL, [BX]
    assert_eq!(cpu.ax & 0xFF, 0);
    cpu.step(); // INC BX
    cpu.step(); // CMP AL, 0
    assert!(cpu.get_flag(FLAG_ZF), "ZF should be set for zero");
    cpu.step(); // JZ (should jump to done)
    assert_eq!(cpu.ip, 0x1009, "Should jump to done");
}

#[test]
fn test_backward_jump_boundary_cases() {
    // Test backwards jumps at various IP values to ensure wrapping works correctly
    
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);
    
    // Test jumping backwards from near the start of a segment
    cpu.memory.load_program(
        0x0005,
        &[
            0x90,                    // NOP                  @ 0x0005
            0xEB, 0xFD,             // JMP -3 (to 0x0005)   @ 0x0006
        ],
    );
    
    cpu.ip = 0x0006;
    cpu.cs = 0x0000;
    cpu.step(); // JMP
    assert_eq!(cpu.ip, 0x0005, "Should jump to 0x0005");
    
    // Test LOOP with backwards jump
    cpu.memory.load_program(
        0x0010,
        &[
            0x90,                    // NOP                  @ 0x0010
            0xE2, 0xFD,             // LOOP -3 (to 0x0010)  @ 0x0011
        ],
    );
    
    cpu.ip = 0x0011;
    cpu.cx = 1;
    cpu.step(); // LOOP
    assert_eq!(cpu.cx, 0);
    assert_eq!(cpu.ip, 0x0013, "Should exit loop when CX becomes 0");
    
    // Test with CX > 1
    cpu.ip = 0x0011;
    cpu.cx = 2;
    cpu.step(); // LOOP
    assert_eq!(cpu.cx, 1);
    assert_eq!(cpu.ip, 0x0010, "Should loop back when CX != 0");
}

#[test]
fn test_dos_file_read_simulation() {
    // Simulate a typical DOS file reading pattern more accurately
    // This mimics what FreeDOS TYPE command might do
    
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);
    
    // Simulate DOS INT 21h AH=3Fh (read file) loop:
    // loop:
    //     mov ah, 0x3F        ; Function 3Fh = read
    //     mov bx, handle      ; File handle
    //     mov cx, buffer_size ; Bytes to read
    //     mov dx, buffer      ; Buffer offset
    //     int 0x21            ; Call DOS (simulated)
    //     or ax, ax           ; Check if bytes read == 0
    //     jz done             ; Exit if EOF
    //     ; process buffer
    //     jmp loop            ; Read more
    // done:
    
    cpu.memory.load_program(
        0x1000,
        &[
            // loop:
            0xB4, 0x3F,             // MOV AH, 0x3F         @ 0x1000
            0xBB, 0x05, 0x00,       // MOV BX, 5 (handle)   @ 0x1002
            0xB9, 0x00, 0x01,       // MOV CX, 0x0100       @ 0x1005
            0xBA, 0x00, 0x20,       // MOV DX, 0x2000       @ 0x1008
            0x90,                    // NOP (simulate INT)    @ 0x100B
            0x0B, 0xC0,             // OR AX, AX            @ 0x100C
            0x74, 0x02,             // JZ +2 (to 0x1012)    @ 0x100E
            0xEB, 0xEE,             // JMP -18 (to 0x1000) @ 0x1010
            // done:
            0xF4,                    // HLT                  @ 0x1012
        ],
    );
    
    cpu.ip = 0x1000;
    cpu.cs = 0x0000;
    
    // First read: return 256 bytes
    for _ in 0..5 { cpu.step(); } // Execute MOV instructions + NOP
    cpu.ax = 256; // Simulate INT 21h returning 256 bytes
    cpu.step(); // OR AX, AX
    assert!(!cpu.get_flag(FLAG_ZF), "ZF should be clear when AX != 0");
    cpu.step(); // JZ (should not jump)
    assert_eq!(cpu.ip, 0x1010, "Should not jump to done");
    cpu.step(); // JMP (should loop back)
    assert_eq!(cpu.ip, 0x1000, "Should jump back to loop start");
    
    // Second read: return 0 bytes (EOF)
    for _ in 0..5 { cpu.step(); } // Execute MOV instructions + NOP
    cpu.ax = 0; // Simulate INT 21h returning 0 (EOF)
    cpu.step(); // OR AX, AX
    assert!(cpu.get_flag(FLAG_ZF), "ZF should be set when AX == 0");
    cpu.step(); // JZ (should jump to done)
    assert_eq!(cpu.ip, 0x1012, "Should jump to done on EOF");
}
