//! Tests for jump, call, return, and loop instructions
//!
//! This module contains tests for control flow instructions

use crate::cpu_8086::ArrayMemory;
use crate::cpu_8086::{
    Cpu8086, CpuModel, Memory8086, FLAG_AF, FLAG_CF, FLAG_DF, FLAG_OF, FLAG_PF, FLAG_SF, FLAG_ZF,
};

// Helper function for tests to calculate physical address
fn physical_address(segment: u16, offset: u16) -> u32 {
    ((segment as u32) << 4) + (offset as u32)
}

#[test]
fn test_jump_short() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // JMP short +5
    cpu.memory.load_program(0xFFFF0, &[0xEB, 0x05]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();
    assert_eq!(cpu.ip, 0x0007); // 2 (instruction size) + 5 (offset)
}

#[test]
fn test_conditional_jump_taken() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // JZ +3 (should jump when ZF is set)
    cpu.memory.load_program(0xFFFF0, &[0x74, 0x03]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.set_flag(FLAG_ZF, true);

    let cycles = cpu.step();
    assert_eq!(cycles, 16);
    assert_eq!(cpu.ip, 0x0005); // 2 + 3
}

#[test]
fn test_conditional_jump_not_taken() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // JZ +3 (should not jump when ZF is clear)
    cpu.memory.load_program(0xFFFF0, &[0x74, 0x03]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.set_flag(FLAG_ZF, false);

    let cycles = cpu.step();
    assert_eq!(cycles, 4);
    assert_eq!(cpu.ip, 0x0002); // Just past instruction
}

#[test]
fn test_iret_instruction() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // Setup stack with return values
    cpu.ss = 0x0000;
    cpu.sp = 0xFFF8;

    // IRET pops in order: IP, CS, FLAGS
    // So stack layout from SP upwards is: IP, CS, FLAGS
    cpu.memory.write(0xFFF8, 0x78); // IP low
    cpu.memory.write(0xFFF9, 0x56); // IP high
    cpu.memory.write(0xFFFA, 0x34); // CS low
    cpu.memory.write(0xFFFB, 0x12); // CS high
    cpu.memory.write(0xFFFC, 0x02); // FLAGS low
    cpu.memory.write(0xFFFD, 0x02); // FLAGS high

    // Load IRET instruction
    cpu.memory.load_program(0xF0000, &[0xCF]);
    cpu.ip = 0x0000;
    cpu.cs = 0xF000;

    cpu.step();

    // Check that IP, CS, FLAGS were popped
    assert_eq!(cpu.ip, 0x5678);
    assert_eq!(cpu.cs, 0x1234);
    assert_eq!(cpu.flags, 0x0202);
    assert_eq!(cpu.sp, 0xFFFE); // Stack pointer restored
}

#[test]
fn test_call_near() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.ss = 0x1000;
    cpu.sp = 0x0100;
    cpu.cs = 0x2000;
    cpu.ip = 0x0010;

    // CALL near with offset +0x0050 (0xE8, 0x50, 0x00)
    cpu.memory.load_program(0x20010, &[0xE8, 0x50, 0x00]);

    let old_sp = cpu.sp;
    cpu.step();

    // IP should be at offset location (0x0010 + 3 (instruction size) + 0x0050)
    assert_eq!(cpu.ip, 0x0063);

    // Stack should contain return address (0x0013 - after the CALL instruction)
    assert_eq!(cpu.sp, old_sp - 2);
    let return_addr = cpu.read_u16(cpu.ss, cpu.sp as u16);
    assert_eq!(return_addr, 0x0013);
}

#[test]
fn test_ret_near() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.ss = 0x1000;
    cpu.sp = 0x00FE;
    cpu.cs = 0x2000;

    // Push return address onto stack
    cpu.memory.write(
        Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x00FE),
        0x34,
    );
    cpu.memory.write(
        Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x00FF),
        0x12,
    );

    // RET (0xC3)
    cpu.memory.load_program(0x20000, &[0xC3]);
    cpu.ip = 0x0000;

    let old_sp = cpu.sp;
    cpu.step();

    // IP should be restored to return address
    assert_eq!(cpu.ip, 0x1234);
    // Stack pointer should be restored
    assert_eq!(cpu.sp, old_sp + 2);
}

#[test]
fn test_ret_near_with_immediate() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.ss = 0x1000;
    cpu.sp = 0x00F8;
    cpu.cs = 0x2000;

    // Push return address onto stack
    cpu.memory.write(
        Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x00F8),
        0x78,
    );
    cpu.memory.write(
        Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x00F9),
        0x56,
    );

    // RET 0x0004 (0xC2, 0x04, 0x00) - pops return address and adds 4 to SP
    cpu.memory.load_program(0x20000, &[0xC2, 0x04, 0x00]);
    cpu.ip = 0x0000;

    cpu.step();

    // IP should be restored to return address
    assert_eq!(cpu.ip, 0x5678);
    // Stack pointer should be restored plus the immediate value
    assert_eq!(cpu.sp, 0x00F8 + 2 + 4); // Original SP + 2 (pop) + 4 (immediate)
}

#[test]
fn test_call_ret_roundtrip() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.ss = 0x1000;
    cpu.sp = 0x0100;
    cpu.cs = 0x2000;
    cpu.ip = 0x0010;

    // CALL near with offset +0x0020
    cpu.memory.load_program(0x20010, &[0xE8, 0x20, 0x00]);
    cpu.step();
    assert_eq!(cpu.ip, 0x0033); // 0x0010 + 3 + 0x0020

    // RET
    cpu.memory.load_program(0x20033, &[0xC3]);
    cpu.step();
    assert_eq!(cpu.ip, 0x0013); // Return to address after CALL
    assert_eq!(cpu.sp, 0x0100); // Stack pointer restored
}

#[test]
fn test_call_far() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.ss = 0x1000;
    cpu.sp = 0x0100;
    cpu.cs = 0x2000;
    cpu.ip = 0x0010;

    // CALL far to 0x3000:0x0050 (0x9A, 0x50, 0x00, 0x00, 0x30)
    cpu.memory
        .load_program(0x20010, &[0x9A, 0x50, 0x00, 0x00, 0x30]);

    let old_sp = cpu.sp;
    cpu.step();

    // CS:IP should be at far address
    assert_eq!(cpu.cs, 0x3000);
    assert_eq!(cpu.ip, 0x0050);

    // Stack should contain old CS and IP
    assert_eq!(cpu.sp, old_sp - 4);
    let return_ip = cpu.read_u16(cpu.ss, (old_sp - 4) as u16); // IP is pushed last
    let return_cs = cpu.read_u16(cpu.ss, (old_sp - 2) as u16); // CS is pushed first
    assert_eq!(return_ip, 0x0015); // After CALL instruction
    assert_eq!(return_cs, 0x2000);
}

#[test]
fn test_ret_far() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.ss = 0x1000;
    cpu.sp = 0x00FC;
    cpu.cs = 0x3000;

    // Push return CS and IP onto stack (IP first, then CS)
    cpu.memory.write(
        Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x00FC),
        0x34,
    ); // IP low
    cpu.memory.write(
        Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x00FD),
        0x12,
    ); // IP high
    cpu.memory.write(
        Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x00FE),
        0x00,
    ); // CS low
    cpu.memory.write(
        Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x00FF),
        0x20,
    ); // CS high

    // RET far (0xCB)
    cpu.memory.load_program(0x30000, &[0xCB]);
    cpu.ip = 0x0000;

    cpu.step();

    // CS:IP should be restored
    assert_eq!(cpu.ip, 0x1234);
    assert_eq!(cpu.cs, 0x2000);
    assert_eq!(cpu.sp, 0x0100); // SP restored
}

#[test]
fn test_x86_jump_offset_calculation() {
    // Verify that jump offsets are calculated correctly per x86 spec:
    // Offset is relative to IP AFTER the instruction (IP points to next instruction)

    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // Test forward jump
    // JMP at 0x0100, offset +5 should land at 0x0107
    cpu.memory.load_program(
        0x0100,
        &[
            0xEB, 0x05, // JMP +5           @ 0x0100 (jumps to 0x0102+5=0x0107)
            0x90, 0x90, 0x90, // NOPs (skipped)   @ 0x0102-0x0104
            0x90, 0x90, // NOPs (skipped)   @ 0x0105-0x0106
            0xF4, // HLT              @ 0x0107
        ],
    );

    cpu.ip = 0x0100;
    cpu.cs = 0x0000;
    cpu.step(); // Execute JMP
    assert_eq!(cpu.ip, 0x0107, "Forward JMP should land at correct address");

    // Test backward jump
    // JMP at 0x0105, offset -5 should land at 0x0102
    cpu.memory.load_program(
        0x0100,
        &[
            0xEB, 0x03, // JMP +3           @ 0x0100 (jumps to 0x0105)
            0xF4, // HLT              @ 0x0102 (target of backward jump)
            0x90, 0x90, // NOPs             @ 0x0103-0x0104
            0xEB, 0xFB, // JMP -5           @ 0x0105 (jumps to 0x0107-5=0x0102)
        ],
    );

    cpu.ip = 0x0100;
    cpu.step(); // Execute first JMP (forward to 0x0105)
    assert_eq!(cpu.ip & 0xFFFF, 0x0105, "Should jump forward to 0x0105");
    cpu.step(); // Execute second JMP (backward to 0x0102)
    assert_eq!(
        cpu.ip & 0xFFFF,
        0x0102,
        "Backward JMP should land at 0x0102"
    );
}

#[test]
fn test_loop_instruction_variants() {
    // Test LOOP, LOOPZ/LOOPE, LOOPNZ/LOOPNE

    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // Test LOOP (0xE2) - decrements CX and jumps if CX != 0
    cpu.cx = 3;
    cpu.memory.load_program(
        0x0100,
        &[
            0x43, // INC BX           @ 0x0100
            0xE2, 0xFD, // LOOP -3          @ 0x0101 (jumps to 0x0103-3=0x0100)
            0xF4, // HLT              @ 0x0103
        ],
    );

    cpu.ip = 0x0100;
    cpu.cs = 0x0000;
    cpu.bx = 0;

    // Should loop 3 times
    for i in 1..=3 {
        cpu.step(); // INC BX
        cpu.step(); // LOOP
        assert_eq!(cpu.cx, 3 - i, "CX should decrement");
    }
    assert_eq!(cpu.bx, 3, "Should have looped 3 times");
    assert_eq!(cpu.ip, 0x0103, "Should exit loop when CX=0");
}

#[test]
fn test_loopz_loopnz_instructions() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // Test LOOPZ (0xE1) - loop while zero flag is set and CX != 0
    cpu.cx = 5;
    cpu.set_flag(0x0040, true); // Set ZF
    cpu.memory.load_program(
        0x0100,
        &[
            0x40, // INC AX           @ 0x0100 (clears ZF when AX becomes non-zero)
            0xE1, 0xFD, // LOOPZ -3         @ 0x0101 (jumps to 0x0103-3=0x0100 if ZF && CX!=0)
            0xF4, // HLT              @ 0x0103
        ],
    );

    cpu.ip = 0x0100;
    cpu.cs = 0x0000;
    cpu.ax = 0xFFFF;

    cpu.step(); // INC AX (wraps to 0, sets ZF)
    assert!(cpu.get_flag(0x0040), "ZF should be set when AX=0");
    cpu.step(); // LOOPZ should jump because ZF=1
    assert_eq!(cpu.ip & 0xFFFF, 0x0100, "Should loop back");
    assert_eq!(cpu.cx & 0xFFFF, 4, "CX should decrement");

    cpu.step(); // INC AX (AX=1, clears ZF)
    assert!(!cpu.get_flag(0x0040), "ZF should be clear when AX!=0");
    cpu.step(); // LOOPZ should NOT jump because ZF=0
    assert_eq!(cpu.ip, 0x0103, "Should exit loop when ZF=0");
    assert_eq!(cpu.cx, 3, "CX should still decrement");

    // Test LOOPNZ (0xE0) - loop while zero flag is clear and CX != 0
    let mem2 = ArrayMemory::new();
    let mut cpu2 = Cpu8086::new(mem2);

    cpu2.cx = 5;
    cpu2.set_flag(0x0040, false); // Clear ZF
    cpu2.memory.load_program(
        0x0100,
        &[
            0x48, // DEC AX           @ 0x0100 (sets ZF when AX becomes 0)
            0xE0, 0xFD, // LOOPNZ -3        @ 0x0101 (jumps if !ZF && CX!=0)
            0xF4, // HLT              @ 0x0103
        ],
    );

    cpu2.ip = 0x0100;
    cpu2.cs = 0x0000;
    cpu2.ax = 3;

    // Should loop while AX != 0
    for _ in 0..3 {
        cpu2.step(); // DEC AX
        if cpu2.ax > 0 {
            cpu2.step(); // LOOPNZ should jump
            assert_eq!(cpu2.ip & 0xFFFF, 0x0100, "Should loop back while AX!=0");
        }
    }
    assert_eq!(cpu2.ax & 0xFFFF, 0, "AX should be 0");
    assert!(cpu2.get_flag(0x0040), "ZF should be set");
}

#[test]
fn test_jcxz_instruction() {
    // Test JCXZ (0xE3) - Jump if CX is zero

    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.memory.load_program(
        0x0100,
        &[
            0xE3, 0x04, // JCXZ +4          @ 0x0100 (jumps to 0x0102+4=0x0106 if CX=0)
            0x43, // INC BX           @ 0x0102
            0x43, // INC BX           @ 0x0103
            0xEB, 0x02, // JMP +2           @ 0x0104 (skip to HLT)
            0x41, // INC CX           @ 0x0106 (reached via JCXZ)
            0x41, // INC CX           @ 0x0107
            0xF4, // HLT              @ 0x0108
        ],
    );

    cpu.ip = 0x0100;
    cpu.cs = 0x0000;

    // Test with CX=0 (should jump)
    cpu.cx = 0;
    cpu.bx = 0;
    cpu.step(); // JCXZ
    assert_eq!(cpu.ip, 0x0106, "Should jump when CX=0");
    assert_eq!(cpu.bx, 0, "Should have skipped INC BX");

    // Test with CX!=0 (should not jump)
    cpu.ip = 0x0100;
    cpu.cx = 5;
    cpu.bx = 0;
    cpu.step(); // JCXZ
    assert_eq!(cpu.ip, 0x0102, "Should not jump when CX!=0");
    cpu.step(); // INC BX
    assert_eq!(cpu.bx, 1, "Should execute INC BX");
}

#[test]
fn test_signed_conditional_jumps() {
    // Test JL, JGE, JLE, JG (signed comparisons)

    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // Test JL (0x7C) - Jump if Less (SF != OF)
    cpu.memory.load_program(
        0x0100,
        &[
            0x3C, 0x05, // CMP AL, 5        @ 0x0100
            0x7C, 0x02, // JL +2            @ 0x0102 (jumps if AL < 5)
            0x43, // INC BX           @ 0x0104
            0xF4, // HLT              @ 0x0105
        ],
    );

    cpu.ip = 0x0100;
    cpu.cs = 0x0000;
    cpu.ax = 3; // 3 < 5, should jump
    cpu.bx = 0;
    cpu.step(); // CMP
    cpu.step(); // JL
    assert_eq!(cpu.ip, 0x0106, "Should jump when 3 < 5");
    assert_eq!(cpu.bx, 0, "Should skip INC BX");

    // Test JGE (0x7D) - Jump if Greater or Equal (SF == OF)
    cpu.ip = 0x0100;
    cpu.memory.write(0x0102, 0x7D); // Change to JGE
    cpu.ax = 7; // 7 >= 5, should jump
    cpu.step(); // CMP
    cpu.step(); // JGE
    assert_eq!(cpu.ip, 0x0106, "Should jump when 7 >= 5");

    // Test with negative numbers
    cpu.ip = 0x0100;
    cpu.ax = 0xFFFE; // -2 in signed 8-bit
    cpu.memory.write(0x0102, 0x7C); // JL
    cpu.step(); // CMP AL, 5 (-2 < 5)
    cpu.step(); // JL
    assert_eq!(cpu.ip, 0x0106, "Should jump when -2 < 5");
}

#[test]
fn test_unsigned_conditional_jumps() {
    // Test JB, JAE, JBE, JA (unsigned comparisons)

    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // Test JB (0x72) - Jump if Below (CF=1)
    cpu.memory.load_program(
        0x0100,
        &[
            0x3C, 0x80, // CMP AL, 0x80     @ 0x0100
            0x72, 0x02, // JB +2            @ 0x0102 (jumps if AL < 0x80 unsigned)
            0x43, // INC BX           @ 0x0104
            0xF4, // HLT              @ 0x0105
        ],
    );

    cpu.ip = 0x0100;
    cpu.cs = 0x0000;
    cpu.ax = 0x50; // 0x50 < 0x80, should jump
    cpu.bx = 0;
    cpu.step(); // CMP
    cpu.step(); // JB
    assert_eq!(cpu.ip, 0x0106, "Should jump when 0x50 < 0x80");

    // Test JAE (0x73) - Jump if Above or Equal (CF=0)
    cpu.ip = 0x0100;
    cpu.memory.write(0x0102, 0x73); // Change to JAE
    cpu.ax = 0xFF; // 0xFF >= 0x80, should jump
    cpu.step(); // CMP
    cpu.step(); // JAE
    assert_eq!(cpu.ip, 0x0106, "Should jump when 0xFF >= 0x80");

    // Test JBE (0x76) - Jump if Below or Equal (CF=1 or ZF=1)
    cpu.ip = 0x0100;
    cpu.memory.write(0x0102, 0x76); // Change to JBE
    cpu.ax = 0x80; // 0x80 == 0x80, should jump (ZF=1)
    cpu.step(); // CMP
    cpu.step(); // JBE
    assert_eq!(cpu.ip, 0x0106, "Should jump when 0x80 == 0x80");
}

#[test]
fn test_memory_based_loop_counter() {
    // Test pattern where loop counter is in memory (common in C code)

    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // Store loop counter at 0x0200
    cpu.memory.write(0x0200, 5);

    cpu.memory.load_program(
        0x0100,
        &[
            0x43, // INC BX               @ 0x0100
            0xFE, 0x0E, 0x00, 0x02, // DEC BYTE [0x0200]    @ 0x0101
            0x75, 0xF9, // JNZ -7               @ 0x0105 (jumps to 0x0107-7=0x0100)
            0xF4, // HLT                  @ 0x0107
        ],
    );

    cpu.ip = 0x0100;
    cpu.cs = 0x0000;
    cpu.bx = 0;

    // Run until HLT
    let mut iterations = 0;
    loop {
        cpu.step();
        iterations += 1;

        let opcode = cpu.memory.read(((cpu.cs as u32) << 4) + cpu.ip);
        if opcode == 0xF4 {
            break;
        }

        if iterations > 50 {
            panic!("Infinite loop detected in memory counter test");
        }
    }

    assert_eq!(cpu.bx, 5, "Should have looped 5 times");
    assert_eq!(cpu.memory.read(0x0200), 0, "Memory counter should be 0");
}

#[test]
fn test_file_read_loop_pattern() {
    // Test the exact pattern that FreeDOS type.c uses:
    // while((len = dos_read(fd, buf, sizeof(buf))) >= 0) {
    //     if (len == 0) break;
    // }
    //
    // This simulates:
    // - Reading a return value into AX
    // - Testing if AX >= 0 (signed comparison)
    // - Testing if AX == 0
    // - Looping back or exiting

    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // Simulate multiple "reads" returning decreasing values, then 0
    // Memory at 0x0200 contains the simulated return values
    cpu.memory.write(0x0200, 10); // First read returns 10 bytes
    cpu.memory.write(0x0201, 5); // Second read returns 5 bytes
    cpu.memory.write(0x0202, 2); // Third read returns 2 bytes
    cpu.memory.write(0x0203, 0); // Fourth read returns 0 (EOF)

    // BX will point to current read result
    cpu.bx = 0x0200;

    // CX will count iterations (for safety - should be 4)
    cpu.cx = 0;

    // Program that simulates the read loop:
    // loop_start:
    //   MOV AL, [BX]      ; Read simulated return value
    //   INC BX            ; Move to next return value
    //   TEST AL, AL       ; Check if AL == 0
    //   JZ loop_end       ; Exit if zero
    //   INC CX            ; Count iterations
    //   JMP loop_start    ; Continue loop
    // loop_end:
    //   HLT

    cpu.memory.load_program(
        0x0100,
        &[
            0x8A, 0x07, // MOV AL, [BX]  @ 0x0100
            0x43, // INC BX        @ 0x0102
            0x84, 0xC0, // TEST AL, AL   @ 0x0103
            0x74, 0x03, // JZ +3         @ 0x0105 (jumps to 0x0107+3=0x010A if ZF)
            0x41, // INC CX        @ 0x0107
            0xEB, 0xF6, // JMP -10       @ 0x0108 (jumps to 0x010A-10=0x0100)
            0xF4, // HLT           @ 0x010A
        ],
    );

    cpu.ip = 0x0100;
    cpu.cs = 0x0000;

    // Run the loop (max 100 iterations for safety)
    let mut iterations = 0;
    loop {
        let _ip_before = cpu.ip;
        let opcode = cpu.memory.read(((cpu.cs as u32) << 4) + cpu.ip);

        // Debug: print state before execution
        if !(20..=95).contains(&iterations) {
            eprintln!(
                "Iter {}: IP={:04X} Opcode={:02X} CX={} BX={:04X} AX={:04X} Flags={:04X}",
                iterations, cpu.ip, opcode, cpu.cx, cpu.bx, cpu.ax, cpu.flags
            );
        }

        cpu.step();
        iterations += 1;

        // Check if we hit HLT (opcode 0xF4)
        let current_opcode = cpu.memory.read(((cpu.cs as u32) << 4) + cpu.ip);
        if current_opcode == 0xF4 {
            break;
        }

        if iterations > 100 {
            eprintln!("\n=== INFINITE LOOP DETECTED ===");
            eprintln!(
                "Final state: CX={}, BX={:04X}, AX={:04X}, IP={:04X}",
                cpu.cx, cpu.bx, cpu.ax, cpu.ip
            );
            eprintln!(
                "Flags: ZF={} SF={} CF={} OF={}",
                cpu.get_flag(0x0040),
                cpu.get_flag(0x0080),
                cpu.get_flag(0x0001),
                cpu.get_flag(0x0800)
            );
            panic!("Loop ran for more than 100 iterations - infinite loop detected! CX={}, BX={:04X}, AX={:04X}", 
               cpu.cx, cpu.bx, cpu.ax);
        }
    }

    // Should have done exactly 3 iterations (for values 10, 5, 2), then stopped at 0
    assert_eq!(cpu.cx, 3, "Should have 3 iterations before hitting EOF");
    assert_eq!(cpu.bx, 0x0204, "BX should point past the last value");
    assert_eq!(cpu.ax & 0xFF, 0, "AL should be 0 (EOF value)");
}

#[test]
fn test_signed_comparison_loop_pattern() {
    // Test the signed comparison pattern: while(len >= 0)
    // This uses JGE (Jump if Greater or Equal, signed)

    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // Simulate return values: positive, then 0, then should stop
    cpu.memory.write(0x0200, 5); // Positive value
    cpu.memory.write(0x0201, 0); // Zero (EOF)
    cpu.memory.write(0x0202, 0xFF); // -1 (error) - should not reach

    cpu.bx = 0x0200;
    cpu.cx = 0; // Iteration counter

    // Program:
    // loop_start:
    //   MOV AL, [BX]      ; Read value
    //   INC BX            ; Next value
    //   TEST AL, AL       ; Set flags
    //   JS loop_end       ; Exit if negative (SF set)
    //   INC CX            ; Count iteration
    //   CMP AL, 0         ; Check if zero
    //   JNZ loop_start    ; Continue if not zero
    // loop_end:
    //   HLT

    cpu.memory.load_program(
        0x0100,
        &[
            0x8A, 0x07, // MOV AL, [BX]         @ 0x0100
            0x43, // INC BX               @ 0x0102
            0x84, 0xC0, // TEST AL, AL          @ 0x0103
            0x78, 0x05, // JS +5                @ 0x0105 (jumps to 0x0107+5=0x010C if SF)
            0x41, // INC CX               @ 0x0107
            0x3C, 0x00, // CMP AL, 0            @ 0x0108
            0x75, 0xF4, // JNZ -12              @ 0x010A (jumps to 0x010C-12=0x0100)
            0xF4, // HLT                  @ 0x010C
        ],
    );

    cpu.ip = 0x0100;
    cpu.cs = 0x0000;

    let mut iterations = 0;
    loop {
        let opcode = cpu.memory.read(((cpu.cs as u32) << 4) + cpu.ip);

        if iterations < 20 {
            eprintln!(
                "Iter {}: IP={:04X} Opcode={:02X} CX={} BX={:04X} AX={:04X} Flags=ZF:{} SF:{}",
                iterations,
                cpu.ip,
                opcode,
                cpu.cx,
                cpu.bx,
                cpu.ax,
                cpu.get_flag(0x0040),
                cpu.get_flag(0x0080)
            );
        }

        cpu.step();
        iterations += 1;

        let current_opcode = cpu.memory.read(((cpu.cs as u32) << 4) + cpu.ip);
        if current_opcode == 0xF4 {
            break;
        }

        if iterations > 100 {
            panic!(
                "Infinite loop detected! CX={}, BX={:04X}, AX={:04X}",
                cpu.cx, cpu.bx, cpu.ax
            );
        }
    }

    // Should process 5 and 0, then stop (2 iterations)
    assert_eq!(cpu.cx, 2, "Should have 2 iterations");
    assert_eq!(cpu.bx, 0x0202, "BX should point past the zero");
}

#[test]
fn test_dec_and_loop_pattern() {
    // Test DEC with loop - common pattern for counting down bytes

    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // CX = bytes remaining counter
    cpu.cx = 5;
    cpu.bx = 0; // Sum accumulator

    // Program:
    // loop_start:
    //   ADD BX, 1         ; Accumulate
    //   DEC CX            ; Decrement counter
    //   JNZ loop_start    ; Loop if not zero
    //   HLT

    cpu.memory.load_program(
        0x0100,
        &[
            0x83, 0xC3, 0x01, // ADD BX, 1            @ 0x0100
            0x49, // DEC CX               @ 0x0103
            0x75, 0xFA, // JNZ -6               @ 0x0104 (jumps to 0x0106-6=0x0100)
            0xF4, // HLT                  @ 0x0106
        ],
    );

    cpu.ip = 0x0100;
    cpu.cs = 0x0000;

    let mut iterations = 0;
    loop {
        cpu.step();
        iterations += 1;

        let current_opcode = cpu.memory.read(((cpu.cs as u32) << 4) + cpu.ip);
        if current_opcode == 0xF4 {
            break;
        }

        if iterations > 100 {
            panic!(
                "Infinite loop in DEC pattern! CX={}, BX={}, iterations={}",
                cpu.cx, cpu.bx, iterations
            );
        }
    }

    assert_eq!(cpu.cx, 0, "CX should be 0 after loop");
    assert_eq!(cpu.bx, 5, "Should have accumulated 5");
}
