//! Tests for CPU flags and flag manipulation
//!
//! This module contains tests for flag operations and condition testing

use crate::cpu_8086::ArrayMemory;
use crate::cpu_8086::{Cpu8086, CpuModel, Memory8086, FLAG_CF, FLAG_OF, FLAG_PF, FLAG_SF, FLAG_ZF};

#[test]
fn test_add_with_carry() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // ADD AL, 0xFF (0xFF + 0xFF = 0x1FE, should set carry)
    cpu.memory.load_program(0xFFFF0, &[0x04, 0xFF]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0x00FF;

    cpu.step();
    assert_eq!(cpu.ax & 0xFF, 0xFE);
    assert!(cpu.get_flag(FLAG_CF));
}

#[test]
fn test_sub_with_borrow() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // SUB AL, 0x10 (0x05 - 0x10, should set carry/borrow)
    cpu.memory.load_program(0xFFFF0, &[0x2C, 0x10]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0x0005;

    cpu.step();
    assert_eq!(cpu.ax & 0xFF, 0xF5);
    assert!(cpu.get_flag(FLAG_CF));
}

#[test]
fn test_cmp_sets_flags() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // CMP AL, 0x42 (should set zero flag when equal)
    cpu.memory.load_program(0xFFFF0, &[0x3C, 0x42]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0x0042;

    cpu.step();
    assert_eq!(cpu.ax & 0xFF, 0x42); // CMP doesn't modify register
    assert!(cpu.get_flag(FLAG_ZF));
}

#[test]
fn test_flag_instructions() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // CLC
    cpu.memory.load_program(0xFFFF0, &[0xF8]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.set_flag(FLAG_CF, true);

    cpu.step();
    assert!(!cpu.get_flag(FLAG_CF));

    // STC
    cpu.memory.load_program(0xFFFF0, &[0xF9]);
    cpu.ip = 0x0000;

    cpu.step();
    assert!(cpu.get_flag(FLAG_CF));
}

#[test]
fn test_parity_flag() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // AND AL, 0x03 (result = 0x03, has 2 ones = even parity)
    cpu.memory.load_program(0xFFFF0, &[0x24, 0x03]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0x00FF;

    cpu.step();
    assert!(cpu.get_flag(FLAG_PF));

    // AND AL, 0x01 (result = 0x01, has 1 one = odd parity)
    cpu.memory.load_program(0xFFFF0, &[0x24, 0x01]);
    cpu.ip = 0x0000;
    cpu.ax = 0x00FF;

    cpu.step();
    assert!(!cpu.get_flag(FLAG_PF));
}

#[test]
fn test_neg_zero() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // NEG AL with AL=0 (0xF6 with ModR/M 0b11_011_000)
    cpu.memory.load_program(0xFFFF0, &[0xF6, 0b11_011_000]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0x0000; // AL = 0

    cpu.step();

    // AL should remain 0
    assert_eq!(cpu.ax & 0xFF, 0);
    assert!(!cpu.get_flag(FLAG_CF)); // CF cleared when operand is zero
    assert!(cpu.get_flag(FLAG_ZF));
}

#[test]
fn test_neg_overflow() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // NEG AL with AL=0x80 (0xF6 with ModR/M 0b11_011_000)
    cpu.memory.load_program(0xFFFF0, &[0xF6, 0b11_011_000]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0x0080; // AL = -128

    cpu.step();

    // AL should become 0x80 (overflow: -(-128) cannot be represented in 8-bit signed)
    assert_eq!(cpu.ax & 0xFF, 0x80);
    assert!(cpu.get_flag(FLAG_OF)); // OF set for overflow
    assert!(cpu.get_flag(FLAG_CF)); // CF set when operand is not zero
}

#[test]
fn test_cmp_r16_rm16_sets_carry_flag() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // CMP AX, CX (0x3B with ModR/M 0b11_000_001)
    cpu.memory.load_program(0xFFFF0, &[0x3B, 0b11_000_001]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0x0010; // AX = 16
    cpu.cx = 0x0020; // CX = 32

    cpu.step();
    assert_eq!(cpu.ax, 0x0010); // CMP doesn't modify operand
    assert!(cpu.get_flag(FLAG_CF)); // Should set carry when AX < CX
}

#[test]
fn test_or_test_pattern_for_zero_check() {
    // Test OR reg, reg and TEST reg, reg patterns for checking zero
    // (more efficient than CMP reg, 0)

    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // Pattern: OR AX, AX to test if AX is zero
    cpu.memory.load_program(
        0x0100,
        &[
            0x0B, 0xC0, // OR AX, AX        @ 0x0100
            0x74, 0x02, // JZ +2            @ 0x0102 (jumps if AX=0)
            0x43, // INC BX           @ 0x0104
            0xF4, // HLT              @ 0x0105
        ],
    );

    cpu.ip = 0x0100;
    cpu.cs = 0x0000;
    cpu.ax = 0;
    cpu.bx = 0;
    cpu.step(); // OR AX, AX
    assert!(cpu.get_flag(0x0040), "ZF should be set when AX=0");
    cpu.step(); // JZ
    assert_eq!(cpu.ip, 0x0106, "Should jump when AX=0");
    assert_eq!(cpu.bx, 0, "Should skip INC BX");

    // Pattern: TEST AL, AL
    cpu.ip = 0x0100;
    cpu.memory.write(0x0100, 0x84); // Change to TEST
    cpu.memory.write(0x0101, 0xC0); // AL, AL
    cpu.ax = 5;
    cpu.step(); // TEST AL, AL
    assert!(!cpu.get_flag(0x0040), "ZF should be clear when AL!=0");
    cpu.step(); // JZ
    assert_eq!(cpu.ip, 0x0104, "Should not jump when AL!=0");
}

#[test]
fn test_sub_and_compare_zero_pattern() {
    // Test SUB followed by zero check - like: bytes_left -= bytes_read; if (bytes_left == 0) break;

    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // Simulate file of 10 bytes, read 3 bytes at a time
    let total_size = 10u16;
    let bytes_remaining = total_size;

    // Store bytes_remaining at memory location
    cpu.memory.write(0x0200, (bytes_remaining & 0xFF) as u8);
    cpu.memory
        .write(0x0201, ((bytes_remaining >> 8) & 0xFF) as u8);

    // Store read sizes
    cpu.memory.write(0x0210, 3); // First read: 3 bytes
    cpu.memory.write(0x0211, 3); // Second read: 3 bytes
    cpu.memory.write(0x0212, 3); // Third read: 3 bytes
    cpu.memory.write(0x0213, 1); // Fourth read: 1 byte (reaches EOF)

    cpu.bx = 0x0210; // Pointer to read sizes
    cpu.cx = 0; // Iteration counter

    // Program:
    // loop_start:
    //   MOV AL, [BX]           ; Get bytes read this iteration
    //   INC BX
    //   MOV DX, [0x0200]       ; Load bytes_remaining
    //   SUB DL, AL             ; Subtract bytes read (8-bit for simplicity)
    //   MOV [0x0200], DX       ; Store updated bytes_remaining
    //   INC CX                 ; Count iteration
    //   CMP DL, 0              ; Check if bytes_remaining == 0
    //   JNZ loop_start         ; Continue if not zero
    //   HLT

    cpu.memory.load_program(
        0x0100,
        &[
            0x8A, 0x07, // MOV AL, [BX]         @ 0x0100
            0x43, // INC BX               @ 0x0102
            0x8B, 0x16, 0x00, 0x02, // MOV DX, [0x0200]     @ 0x0103
            0x28, 0xC2, // SUB DL, AL           @ 0x0107
            0x89, 0x16, 0x00, 0x02, // MOV [0x0200], DX     @ 0x0109
            0x41, // INC CX               @ 0x010D
            0x80, 0xFA, 0x00, // CMP DL, 0            @ 0x010E
            0x75, 0xED, // JNZ -19              @ 0x0111 (jumps to 0x0113-19=0x0100)
            0xF4, // HLT                  @ 0x0113
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
            let bytes_left = cpu.memory.read(0x0200);
            panic!(
                "Infinite loop in SUB pattern! CX={}, iterations={}, bytes_remaining={}",
                cpu.cx, iterations, bytes_left
            );
        }
    }

    let final_bytes = cpu.memory.read(0x0200);
    assert_eq!(final_bytes, 0, "Bytes remaining should be 0");
    assert_eq!(cpu.cx, 4, "Should have 4 iterations (3+3+3+1=10)");
}

#[test]
fn test_update_flags_32_zero() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    cpu.update_flags_32(0);
    assert!(cpu.get_flag(FLAG_ZF), "ZF should be set for zero");
    assert!(!cpu.get_flag(FLAG_SF), "SF should not be set for zero");
}

#[test]
fn test_update_flags_32_negative() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    cpu.update_flags_32(0x80000000); // MSB set = negative in signed interpretation
    assert!(!cpu.get_flag(FLAG_ZF), "ZF should not be set");
    assert!(cpu.get_flag(FLAG_SF), "SF should be set for negative");
}

#[test]
fn test_update_flags_32_parity() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    // Value with even parity in low byte (0x03 = 2 bits set)
    cpu.update_flags_32(0x12345603);
    assert!(cpu.get_flag(FLAG_PF), "PF should be set for even parity");

    // Value with odd parity in low byte (0x07 = 3 bits set)
    cpu.update_flags_32(0x12345607);
    assert!(
        !cpu.get_flag(FLAG_PF),
        "PF should not be set for odd parity"
    );
}

#[test]
fn test_update_flags_32_positive() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    cpu.update_flags_32(0x7FFFFFFF); // MSB not set = positive
    assert!(!cpu.get_flag(FLAG_ZF), "ZF should not be set");
    assert!(!cpu.get_flag(FLAG_SF), "SF should not be set for positive");
}
