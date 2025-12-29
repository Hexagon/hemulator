//! Tests for 8-bit ALU operations and data movement
//!
//! This module contains tests for 8-bit operations on AL, BL, CL, DL, AH, BH, CH, DH

use crate::cpu_8086::ArrayMemory;
use crate::cpu_8086::{
    Cpu8086, CpuModel, Memory8086, FLAG_AF, FLAG_CF, FLAG_DF, FLAG_OF, FLAG_PF, FLAG_SF, FLAG_ZF,
};

// Helper function for tests to calculate physical address
fn physical_address(segment: u16, offset: u16) -> u32 {
    ((segment as u32) << 4) + (offset as u32)
}

#[test]
fn test_test_rm8_r8() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // TEST CL, AL (0x84 with ModR/M 0b11_000_001)
    cpu.memory.load_program(0xFFFF0, &[0x84, 0b11_000_001]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0x00FF; // AL = 0xFF
    cpu.cx = 0x00AA; // CL = 0xAA

    let old_ax = cpu.ax;
    let old_cx = cpu.cx;
    cpu.step();

    // TEST doesn't modify operands
    assert_eq!(cpu.ax, old_ax);
    assert_eq!(cpu.cx, old_cx);

    // Flags should be set based on AL & CL = 0xFF & 0xAA = 0xAA
    assert!(!cpu.get_flag(FLAG_ZF)); // Result is not zero
    assert!(cpu.get_flag(FLAG_SF)); // Result has sign bit set
    assert!(!cpu.get_flag(FLAG_CF)); // CF cleared
    assert!(!cpu.get_flag(FLAG_OF)); // OF cleared
}

#[test]
fn test_test_al_imm8_zero() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // TEST AL, 0x0F (0xA8, 0x0F)
    cpu.memory.load_program(0xFFFF0, &[0xA8, 0x0F]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0x00F0; // AL = 0xF0

    cpu.step();

    // AL & 0x0F = 0xF0 & 0x0F = 0x00
    assert!(cpu.get_flag(FLAG_ZF)); // Result is zero
}

#[test]
fn test_not_r8() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // NOT AL (0xF6 with ModR/M 0b11_010_000)
    cpu.memory.load_program(0xFFFF0, &[0xF6, 0b11_010_000]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0x00AA; // AL = 0xAA

    cpu.step();

    // AL should be ~0xAA = 0x55
    assert_eq!(cpu.ax & 0xFF, 0x55);
}

#[test]
fn test_neg_r8() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // NEG AL (0xF6 with ModR/M 0b11_011_000)
    cpu.memory.load_program(0xFFFF0, &[0xF6, 0b11_011_000]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0x0005; // AL = 5

    cpu.step();

    // AL should be -5 = 0xFB (two's complement)
    assert_eq!(cpu.ax & 0xFF, 0xFB);
    assert!(cpu.get_flag(FLAG_CF)); // CF set when operand is not zero
    assert!(!cpu.get_flag(FLAG_ZF));
    assert!(cpu.get_flag(FLAG_SF));
}

#[test]
fn test_mov_r8_rm8_register() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // MOV AL, CL (0x8A with ModR/M 0b11_000_001)
    // AL = reg field (000), CL = r/m field (001)
    cpu.memory.load_program(0xFFFF0, &[0x8A, 0b11_000_001]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.cx = 0x0042; // CL = 0x42

    cpu.step();
    assert_eq!(cpu.ax & 0xFF, 0x42);
}

#[test]
fn test_mov_r8_rm8_memory() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.ds = 0x1000;
    cpu.bx = 0x0100;

    // Write test value to memory at DS:BX
    let addr = Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x0100);
    cpu.memory.write(addr, 0x55);

    // MOV AL, [BX] (0x8A with ModR/M 0b00_000_111)
    cpu.memory.load_program(0xFFFF0, &[0x8A, 0b00_000_111]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();
    assert_eq!(cpu.ax & 0xFF, 0x55);
}

#[test]
fn test_mov_rm8_r8_register() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // MOV CL, AL (0x88 with ModR/M 0b11_000_001)
    // AL = reg field (000), CL = r/m field (001)
    cpu.memory.load_program(0xFFFF0, &[0x88, 0b11_000_001]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0x0099; // AL = 0x99

    cpu.step();
    assert_eq!(cpu.cx & 0xFF, 0x99);
}

#[test]
fn test_mov_rm8_r8_memory() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.ds = 0x1000;
    cpu.bx = 0x0100;
    cpu.ax = 0x00AA; // AL = 0xAA

    // MOV [BX], AL (0x88 with ModR/M 0b00_000_111)
    cpu.memory.load_program(0xFFFF0, &[0x88, 0b00_000_111]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    // Verify memory was written
    let addr = Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x0100);
    assert_eq!(cpu.memory.read(addr), 0xAA);
}

#[test]
fn test_add_rm8_r8_register() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // ADD CL, AL (0x00 with ModR/M 0b11_000_001)
    // AL = reg (000), CL = r/m (001)
    cpu.memory.load_program(0xFFFF0, &[0x00, 0b11_000_001]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0x0005; // AL = 5
    cpu.cx = 0x0003; // CL = 3

    cpu.step();
    assert_eq!(cpu.cx & 0xFF, 8); // CL should be 3 + 5 = 8
    assert!(!cpu.get_flag(FLAG_ZF));
    assert!(!cpu.get_flag(FLAG_CF));
}

#[test]
fn test_sub_rm8_r8_register() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // SUB CL, AL (0x28 with ModR/M 0b11_000_001)
    cpu.memory.load_program(0xFFFF0, &[0x28, 0b11_000_001]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0x0005; // AL = 5
    cpu.cx = 0x000A; // CL = 10

    cpu.step();
    assert_eq!(cpu.cx & 0xFF, 5); // CL should be 10 - 5 = 5
    assert!(!cpu.get_flag(FLAG_ZF));
    assert!(!cpu.get_flag(FLAG_CF));
}

#[test]
fn test_shl_8bit_with_carry() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // SHL AL, 1
    cpu.memory.load_program(0xFFFF0, &[0xD0, 0b11_100_000]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0x0080; // AL = 0x80 (bit 7 set)

    cpu.step();
    assert_eq!(cpu.ax & 0xFF, 0x00); // 0x80 << 1 = 0x00 (wraps)
    assert!(cpu.get_flag(FLAG_CF)); // Bit 7 was shifted into CF
}

#[test]
fn test_shr_8bit() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // SHR AL, 1 (0xD0 with ModR/M 0b11_101_000)
    cpu.memory.load_program(0xFFFF0, &[0xD0, 0b11_101_000]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0x0042; // AL = 0x42

    cpu.step();
    assert_eq!(cpu.ax & 0xFF, 0x21); // 0x42 >> 1 = 0x21
    assert!(!cpu.get_flag(FLAG_CF));
}
