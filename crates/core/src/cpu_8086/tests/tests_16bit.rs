//! Tests for 16-bit ALU operations and data movement
//!
//! This module contains tests for 16-bit operations on AX, BX, CX, DX, SI, DI, BP, SP

use crate::cpu_8086::ArrayMemory;
use crate::cpu_8086::{
    Cpu8086, CpuModel, Memory8086, FLAG_AF, FLAG_CF, FLAG_DF, FLAG_OF, FLAG_PF, FLAG_SF, FLAG_ZF,
};

// Helper function for tests to calculate physical address
fn physical_address(segment: u16, offset: u16) -> u32 {
    ((segment as u32) << 4) + (offset as u32)
}

#[test]
fn test_mov_immediate_16bit() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // MOV AX, 0x1234
    cpu.memory.load_program(0xFFFF0, &[0xB8, 0x34, 0x12]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    let cycles = cpu.step();
    assert_eq!(cycles, 4);
    assert_eq!(cpu.ax, 0x1234);
}

#[test]
fn test_add_immediate() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // ADD AL, 0x10
    cpu.memory.load_program(0xFFFF0, &[0x04, 0x10]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0x0005;

    cpu.step();
    assert_eq!(cpu.ax & 0xFF, 0x15);
    assert!(!cpu.get_flag(FLAG_ZF));
    assert!(!cpu.get_flag(FLAG_CF));
}

#[test]
fn test_sub_immediate() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // SUB AL, 0x05
    cpu.memory.load_program(0xFFFF0, &[0x2C, 0x05]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0x0010;

    cpu.step();
    assert_eq!(cpu.ax & 0xFF, 0x0B);
    assert!(!cpu.get_flag(FLAG_CF));
}

#[test]
fn test_and_immediate() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // AND AL, 0x0F
    cpu.memory.load_program(0xFFFF0, &[0x24, 0x0F]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0x00FF;

    cpu.step();
    assert_eq!(cpu.ax & 0xFF, 0x0F);
    assert!(!cpu.get_flag(FLAG_CF));
    assert!(!cpu.get_flag(FLAG_OF));
}

#[test]
fn test_or_immediate() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // OR AL, 0xF0
    cpu.memory.load_program(0xFFFF0, &[0x0C, 0xF0]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0x000F;

    cpu.step();
    assert_eq!(cpu.ax & 0xFF, 0xFF);
}

#[test]
fn test_xor_immediate() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // XOR AL, 0xFF
    cpu.memory.load_program(0xFFFF0, &[0x34, 0xFF]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0x00AA;

    cpu.step();
    assert_eq!(cpu.ax & 0xFF, 0x55);
}

#[test]
fn test_inc_dec_register() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // INC AX
    cpu.memory.load_program(0xFFFF0, &[0x40]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0x0010;

    cpu.step();
    assert_eq!(cpu.ax, 0x0011);

    // DEC BX
    cpu.memory.load_program(0xFFFF0, &[0x4B]);
    cpu.ip = 0x0000;
    cpu.bx = 0x0010;

    cpu.step();
    assert_eq!(cpu.bx, 0x000F);
}

#[test]
fn test_push_pop() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);
    cpu.ss = 0x1000;
    cpu.sp = 0x0100;

    // PUSH AX
    cpu.memory.load_program(0xFFFF0, &[0x50]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0x1234;

    cpu.step();
    assert_eq!(cpu.sp, 0x00FE);

    // POP BX
    cpu.memory.load_program(0xFFFF0, &[0x5B]);
    cpu.ip = 0x0000;

    cpu.step();
    assert_eq!(cpu.bx, 0x1234);
    assert_eq!(cpu.sp, 0x0100);
}

#[test]
fn test_test_ax_imm16() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // TEST AX, 0x8000 (0xA9, 0x00, 0x80)
    cpu.memory.load_program(0xFFFF0, &[0xA9, 0x00, 0x80]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0x8080;

    cpu.step();

    // AX & 0x8000 = 0x8080 & 0x8000 = 0x8000
    assert!(!cpu.get_flag(FLAG_ZF));
    assert!(cpu.get_flag(FLAG_SF)); // Sign bit set
}

#[test]
fn test_not_r16() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // NOT AX (0xF7 with ModR/M 0b11_010_000)
    cpu.memory.load_program(0xFFFF0, &[0xF7, 0b11_010_000]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0xAAAA;

    cpu.step();

    // AX should be ~0xAAAA = 0x5555
    assert_eq!(cpu.ax, 0x5555);
}

#[test]
fn test_neg_r16() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // NEG AX (0xF7 with ModR/M 0b11_011_000)
    cpu.memory.load_program(0xFFFF0, &[0xF7, 0b11_011_000]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0x1000;

    cpu.step();

    // AX should be -0x1000 = 0xF000 (two's complement)
    assert_eq!(cpu.ax, 0xF000);
    assert!(cpu.get_flag(FLAG_CF));
    assert!(cpu.get_flag(FLAG_SF));
}

#[test]
fn test_mov_r16_rm16_register() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // MOV AX, CX (0x8B with ModR/M 0b11_000_001)
    cpu.memory.load_program(0xFFFF0, &[0x8B, 0b11_000_001]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.cx = 0x1234;

    cpu.step();
    assert_eq!(cpu.ax, 0x1234);
}

#[test]
fn test_mov_r16_rm16_memory() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.ds = 0x1000;
    cpu.si = 0x0200;

    // Write test value to memory at DS:SI
    let addr = Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x0200);
    cpu.memory.write(addr, 0x78); // Low byte
    cpu.memory.write(addr + 1, 0x56); // High byte

    // MOV AX, [SI] (0x8B with ModR/M 0b00_000_100)
    cpu.memory.load_program(0xFFFF0, &[0x8B, 0b00_000_100]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();
    assert_eq!(cpu.ax, 0x5678);
}

#[test]
fn test_mov_rm16_r16_register() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // MOV CX, AX (0x89 with ModR/M 0b11_000_001)
    cpu.memory.load_program(0xFFFF0, &[0x89, 0b11_000_001]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0xABCD;

    cpu.step();
    assert_eq!(cpu.cx, 0xABCD);
}

#[test]
fn test_mov_rm16_r16_memory() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.ds = 0x1000;
    cpu.di = 0x0300;
    cpu.ax = 0x9876;

    // MOV [DI], AX (0x89 with ModR/M 0b00_000_101)
    cpu.memory.load_program(0xFFFF0, &[0x89, 0b00_000_101]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    // Verify memory was written (little-endian)
    let addr = Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x0300);
    assert_eq!(cpu.memory.read(addr), 0x76); // Low byte
    assert_eq!(cpu.memory.read(addr + 1), 0x98); // High byte
}

#[test]
fn test_add_rm16_r16_memory() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.ds = 0x1000;
    cpu.bx = 0x0100;
    cpu.ax = 0x0020; // AX = 32

    // Write initial value to memory
    let addr = Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x0100);
    cpu.memory.write(addr, 0x10); // Low byte = 16
    cpu.memory.write(addr + 1, 0x00); // High byte = 0

    // ADD [BX], AX (0x01 with ModR/M 0b00_000_111)
    cpu.memory.load_program(0xFFFF0, &[0x01, 0b00_000_111]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    // Memory should now contain 16 + 32 = 48
    let result = cpu.memory.read(addr) as u16 | ((cpu.memory.read(addr + 1) as u16) << 8);
    assert_eq!(result, 48);
}

#[test]
fn test_push_pop_fs() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    // Setup: PUSH FS, POP FS at 0x0000:0x0100
    // 0x0F 0xA0 = PUSH FS
    // 0x0F 0xA1 = POP FS
    cpu.memory.load_program(0x0100, &[0x0F, 0xA0, 0x0F, 0xA1]);

    cpu.ip = 0x0100;
    cpu.cs = 0x0000;
    cpu.ss = 0x1000;
    cpu.sp = 0xFFFE;
    cpu.fs = 0x1234;

    // Execute PUSH FS
    cpu.step();
    assert_eq!(cpu.sp, 0xFFFC, "SP should decrease by 2");
    assert_eq!(
        cpu.read_u16(cpu.ss, cpu.sp as u16),
        0x1234,
        "FS value should be on stack"
    );

    // Modify FS
    cpu.fs = 0x5678;

    // Execute POP FS
    cpu.step();
    assert_eq!(cpu.sp, 0xFFFE, "SP should be restored");
    assert_eq!(cpu.fs, 0x1234, "FS should be restored from stack");
}

#[test]
fn test_push_pop_gs() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    // Setup: PUSH GS, POP GS at 0x0000:0x0100
    // 0x0F 0xA8 = PUSH GS
    // 0x0F 0xA9 = POP GS
    cpu.memory.load_program(0x0100, &[0x0F, 0xA8, 0x0F, 0xA9]);

    cpu.ip = 0x0100;
    cpu.cs = 0x0000;
    cpu.ss = 0x1000;
    cpu.sp = 0xFFFE;
    cpu.gs = 0xABCD;

    // Execute PUSH GS
    cpu.step();
    assert_eq!(cpu.sp, 0xFFFC, "SP should decrease by 2");
    assert_eq!(
        cpu.read_u16(cpu.ss, cpu.sp as u16),
        0xABCD,
        "GS value should be on stack"
    );

    // Modify GS
    cpu.gs = 0xEF01;

    // Execute POP GS
    cpu.step();
    assert_eq!(cpu.sp, 0xFFFE, "SP should be restored");
    assert_eq!(cpu.gs, 0xABCD, "GS should be restored from stack");
}
