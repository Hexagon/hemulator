//! Tests for 16-bit ALU operations and data movement
//!
//! This module contains tests for 16-bit operations on AX, BX, CX, DX, SI, DI, BP, SP

use crate::cpu_8086::ArrayMemory;
use crate::cpu_8086::{
    Cpu8086, CpuModel, Memory8086, FLAG_AF, FLAG_CF, FLAG_OF, FLAG_SF, FLAG_ZF,
};

use super::physical_address;

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

#[test]
fn test_sub_r16_rm16_memory() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.ds = 0x1000;
    cpu.si = 0x0200;
    cpu.ax = 0x0050; // AX = 80

    // Write value to memory
    let addr = Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x0200);
    cpu.memory.write(addr, 0x1E); // Low byte = 30
    cpu.memory.write(addr + 1, 0x00); // High byte = 0

    // SUB AX, [SI] (0x2B with ModR/M 0b00_000_100)
    cpu.memory.load_program(0xFFFF0, &[0x2B, 0b00_000_100]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();
    assert_eq!(cpu.ax, 50); // AX should be 80 - 30 = 50
}

#[test]
fn test_mul_16bit() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // MUL CX (0xF7 with ModR/M 0b11_100_001)
    cpu.memory.load_program(0xFFFF0, &[0xF7, 0b11_100_001]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0x1000; // AX = 4096
    cpu.cx = 0x0010; // CX = 16

    cpu.step();
    assert_eq!(cpu.ax, 0x0000); // Low word of 65536
    assert_eq!(cpu.dx, 0x0001); // High word of 65536
    assert!(cpu.get_flag(FLAG_CF)); // DX is non-zero
    assert!(cpu.get_flag(FLAG_OF));
}

#[test]
fn test_div_16bit() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // DIV CX (0xF7 with ModR/M 0b11_110_001)
    cpu.memory.load_program(0xFFFF0, &[0xF7, 0b11_110_001]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.dx = 0x0001; // High word of dividend
    cpu.ax = 0x0000; // Low word: 0x10000 = 65536
    cpu.cx = 100; // Divisor

    cpu.step();
    // 65536 / 100 = 655 remainder 36
    assert_eq!(cpu.ax, 655); // Quotient
    assert_eq!(cpu.dx, 36); // Remainder
}

#[test]
fn test_shl_by_cl() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // SHL AL, CL (0xD2 with ModR/M 0b11_100_000)
    cpu.memory.load_program(0xFFFF0, &[0xD2, 0b11_100_000]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0x0002; // AL = 2
    cpu.cx = 0x0003; // CL = 3

    cpu.step();
    assert_eq!(cpu.ax & 0xFF, 16); // 2 << 3 = 16
}

#[test]
fn test_shl_16bit() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // SHL AX, 1 (0xD1 with ModR/M 0b11_100_000)
    cpu.memory.load_program(0xFFFF0, &[0xD1, 0b11_100_000]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0x1234;

    cpu.step();
    assert_eq!(cpu.ax, 0x2468); // 0x1234 << 1 = 0x2468
}

#[test]
fn test_ror_16bit() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // ROR AX, 1 (0xD1 with ModR/M 0b11_001_000)
    cpu.memory.load_program(0xFFFF0, &[0xD1, 0b11_001_000]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0x8001;

    cpu.step();
    assert_eq!(cpu.ax, 0xC000); // Bit 0 rotates to bit 15
    assert!(cpu.get_flag(FLAG_CF));
}

#[test]
fn test_pusha() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80186);

    cpu.ax = 0x1111;
    cpu.cx = 0x2222;
    cpu.dx = 0x3333;
    cpu.bx = 0x4444;
    cpu.sp = 0x0100;
    cpu.bp = 0x5555;
    cpu.si = 0x6666;
    cpu.di = 0x7777;
    cpu.ss = 0x1000;

    // PUSHA (0x60)
    cpu.memory.load_program(0xFFFF0, &[0x60]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    // SP should be decremented by 16 (8 words)
    assert_eq!(cpu.sp, 0x00F0);

    // Check values on stack
    let base = physical_address(0x1000, 0x00F0);
    assert_eq!(cpu.memory.read_u16(base), 0x7777); // DI
    assert_eq!(cpu.memory.read_u16(base + 2), 0x6666); // SI
    assert_eq!(cpu.memory.read_u16(base + 4), 0x5555); // BP
    assert_eq!(cpu.memory.read_u16(base + 6), 0x0100); // Original SP
    assert_eq!(cpu.memory.read_u16(base + 8), 0x4444); // BX
    assert_eq!(cpu.memory.read_u16(base + 10), 0x3333); // DX
    assert_eq!(cpu.memory.read_u16(base + 12), 0x2222); // CX
    assert_eq!(cpu.memory.read_u16(base + 14), 0x1111); // AX
}

#[test]
fn test_popa() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80186);

    cpu.sp = 0x00F0;
    cpu.ss = 0x1000;

    // Set up stack with test values
    let base = physical_address(0x1000, 0x00F0);
    cpu.memory.write_u16(base, 0x7777); // DI
    cpu.memory.write_u16(base + 2, 0x6666); // SI
    cpu.memory.write_u16(base + 4, 0x5555); // BP
    cpu.memory.write_u16(base + 6, 0x9999); // SP (discarded)
    cpu.memory.write_u16(base + 8, 0x4444); // BX
    cpu.memory.write_u16(base + 10, 0x3333); // DX
    cpu.memory.write_u16(base + 12, 0x2222); // CX
    cpu.memory.write_u16(base + 14, 0x1111); // AX

    // POPA (0x61)
    cpu.memory.load_program(0xFFFF0, &[0x61]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    // Check registers
    assert_eq!(cpu.ax, 0x1111);
    assert_eq!(cpu.cx, 0x2222);
    assert_eq!(cpu.dx, 0x3333);
    assert_eq!(cpu.bx, 0x4444);
    assert_eq!(cpu.bp, 0x5555);
    assert_eq!(cpu.si, 0x6666);
    assert_eq!(cpu.di, 0x7777);
    // SP should be incremented by 16
    assert_eq!(cpu.sp, 0x0100);
}

#[test]
fn test_push_immediate_word() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80186);

    cpu.sp = 0x0100;
    cpu.ss = 0x1000;

    // PUSH imm16 (0x68) - Push 0x1234
    cpu.memory.load_program(0xFFFF0, &[0x68, 0x34, 0x12]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    // SP should be decremented by 2
    assert_eq!(cpu.sp, 0x00FE);

    // Check value on stack
    let val = cpu.memory.read_u16(physical_address(0x1000, 0x00FE));
    assert_eq!(val, 0x1234);
}

#[test]
fn test_push_immediate_byte() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80186);

    cpu.sp = 0x0100;
    cpu.ss = 0x1000;

    // PUSH imm8 (0x6A) - Push 0x7F (positive, sign extends to 0x007F)
    cpu.memory.load_program(0xFFFF0, &[0x6A, 0x7F]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    // Check value on stack (should be sign-extended)
    let val = cpu.memory.read_u16(physical_address(0x1000, 0x00FE));
    assert_eq!(val, 0x007F);

    // Test with negative value (0xFF should sign extend to 0xFFFF)
    cpu.sp = 0x0100;
    cpu.memory.load_program(0xFFFF0, &[0x6A, 0xFF]);
    cpu.ip = 0x0000;

    cpu.step();

    let val = cpu.memory.read_u16(physical_address(0x1000, 0x00FE));
    assert_eq!(val, 0xFFFF);
}

#[test]
fn test_imul_immediate_word() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80186);

    cpu.bx = 10;

    // IMUL AX, BX, 20 (0x69 ModRM imm16) - AX = BX * 20
    cpu.memory.load_program(0xFFFF0, &[0x69, 0xC3, 0x14, 0x00]); // ModRM=0xC3 (AX, BX)
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    // AX should be 10 * 20 = 200
    assert_eq!(cpu.ax, 200);
    // No overflow for this multiplication
    assert!(!cpu.get_flag(FLAG_CF));
    assert!(!cpu.get_flag(FLAG_OF));
}

#[test]
fn test_imul_immediate_byte() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80186);

    cpu.dx = 5;

    // IMUL AX, DX, 7 (0x6B ModRM imm8) - AX = DX * 7
    cpu.memory.load_program(0xFFFF0, &[0x6B, 0xC2, 0x07]); // ModRM=0xC2 (AX, DX)
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    // AX should be 5 * 7 = 35
    assert_eq!(cpu.ax, 35);
    // No overflow
    assert!(!cpu.get_flag(FLAG_CF));
    assert!(!cpu.get_flag(FLAG_OF));
}

#[test]
fn test_bound_in_range() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80186);

    cpu.ax = 50; // Index to test
    cpu.ds = 0x1000;

    // Set up bounds in memory at DS:0x0100
    // Lower bound: 10, Upper bound: 100
    let addr = physical_address(0x1000, 0x0100);
    cpu.memory.write_u16(addr, 10); // Lower bound
    cpu.memory.write_u16(addr + 2, 100); // Upper bound

    // BOUND AX, [0x0100] (0x62 ModRM disp16)
    cpu.memory.load_program(0xFFFF0, &[0x62, 0x06, 0x00, 0x01]); // ModRM=0x06 (direct addr)
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    let old_ip = cpu.ip;
    cpu.step();

    // Should not trigger interrupt, IP should advance
    assert_ne!(cpu.ip, old_ip);
}

#[test]
fn test_ins_outs_byte() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80186);

    cpu.dx = 0x60; // Port
    cpu.es = 0x1000;
    cpu.di = 0x0100;

    // INSB (0x6C) - Input from port DX to ES:DI
    cpu.memory.load_program(0xFFFF0, &[0x6C]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    // DI should be incremented
    assert_eq!(cpu.di, 0x0101);
    // Value should be written (0xFF from stub I/O)
    let val = cpu.memory.read(physical_address(0x1000, 0x0100));
    assert_eq!(val, 0xFF);

    // Test OUTSB (0x6E)
    cpu.ds = 0x1000;
    cpu.si = 0x0200;
    cpu.memory.write(physical_address(0x1000, 0x0200), 0x42);

    cpu.memory.load_program(0xFFFF0, &[0x6E]);
    cpu.ip = 0x0000;

    cpu.step();

    // SI should be incremented
    assert_eq!(cpu.si, 0x0201);
}

#[test]
fn test_ins_outs_word() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80186);

    cpu.dx = 0x60; // Port
    cpu.es = 0x1000;
    cpu.di = 0x0100;

    // INSW (0x6D) - Input word from port DX to ES:DI
    cpu.memory.load_program(0xFFFF0, &[0x6D]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    // DI should be incremented by 2
    assert_eq!(cpu.di, 0x0102);
    // Value should be written (0xFFFF from stub I/O)
    let val = cpu.memory.read_u16(physical_address(0x1000, 0x0100));
    assert_eq!(val, 0xFFFF);

    // Test OUTSW (0x6F)
    cpu.ds = 0x1000;
    cpu.si = 0x0200;
    cpu.memory
        .write_u16(physical_address(0x1000, 0x0200), 0x1234);

    cpu.memory.load_program(0xFFFF0, &[0x6F]);
    cpu.ip = 0x0000;

    cpu.step();

    // SI should be incremented by 2
    assert_eq!(cpu.si, 0x0202);
}

#[test]
fn test_read_write_rm16_register() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // Set AX to 0x1234
    cpu.ax = 0x1234;

    // Read AX using ModR/M (mod=11, rm=000 for AX)
    let val = cpu.read_rm16(0b11, 0b000);
    assert_eq!(val, 0x1234);

    // Write to CX (mod=11, rm=001 for CX)
    cpu.write_rm16(0b11, 0b001, 0x5678);
    assert_eq!(cpu.cx, 0x5678);
}

#[test]
fn test_read_write_rm16_memory() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.ds = 0x1000;
    cpu.bx = 0x0100;

    // Write to memory using ModR/M (mod=00, rm=111 for [BX])
    cpu.write_rm16(0b00, 0b111, 0xAABB);

    // Read it back
    let val = cpu.read_rm16(0b00, 0b111);
    assert_eq!(val, 0xAABB);

    // Verify it's at the right physical address (little-endian)
    let physical_addr = Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x0100);
    assert_eq!(cpu.memory.read(physical_addr), 0xBB); // Low byte
    assert_eq!(cpu.memory.read(physical_addr + 1), 0xAA); // High byte
}

#[test]
fn test_cmpxchg16() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80486);

    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ds = 0x1000;
    cpu.bx = 0x0100;

    // Test equal case: AX == [BX]
    cpu.ax = 0x1234;
    cpu.cx = 0x5678;
    cpu.memory.write_u16(0x10100, 0x1234);

    // CMPXCHG [BX], CX (0x0F 0xB1 with ModR/M 0x0F)
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0xB1, 0x0F]);
    cpu.step();

    assert!(cpu.get_flag(FLAG_ZF), "ZF should be set when equal");
    assert_eq!(
        cpu.memory.read_u16(0x10100),
        0x5678,
        "Memory should be updated with CX"
    );

    // Test not equal case
    cpu.ip = 0x0000;
    cpu.ax = 0x1234;
    cpu.cx = 0x5678;
    cpu.memory.write_u16(0x10100, 0xABCD);

    cpu.memory.load_program(0xFFFF0, &[0x0F, 0xB1, 0x0F]);
    cpu.step();

    assert!(!cpu.get_flag(FLAG_ZF), "ZF should be clear when not equal");
    assert_eq!(cpu.ax, 0xABCD, "AX should be loaded from memory");
}

#[test]
fn test_xadd16() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80486);

    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ds = 0x1000;
    cpu.bx = 0x0100;

    cpu.ax = 0x0100;
    cpu.cx = 0x0020;
    cpu.memory.write_u16(0x10100, 0x1000);

    // XADD [BX], CX (0x0F 0xC1 with ModR/M 0x0F)
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0xC1, 0x0F]);
    cpu.step();

    assert_eq!(
        cpu.memory.read_u16(0x10100),
        0x1020,
        "Memory should be 0x1000 + 0x20"
    );
    assert_eq!(cpu.cx, 0x1000, "CX should be old memory value");
}

#[test]
fn test_shift_count_masking_8086() {
    // On 8086, shift count is NOT masked - full 8-bit count is used
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel8086);

    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0x00FF;
    cpu.cx = 0x0020; // CL = 32 (shift by 32 on 8086 should shift all bits out)

    // SHL AL, CL (0xD2 with ModR/M 0b11_100_000)
    cpu.memory.load_program(0xFFFF0, &[0xD2, 0xE0]);
    cpu.step();

    // On 8086, shifting by 32 should result in 0 (all bits shifted out)
    assert_eq!(
        cpu.ax & 0xFF,
        0,
        "8086 should shift by full count (32 shifts all bits out)"
    );
}

#[test]
fn test_shift_count_masking_80186() {
    // On 80186+, shift count IS masked to 5 bits (0-31)
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80186);

    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0x00FF;
    cpu.cx = 0x0020; // CL = 32, but masked to 0 on 80186+

    // SHL AL, CL (0xD2 with ModR/M 0b11_100_000)
    cpu.memory.load_program(0xFFFF0, &[0xD2, 0xE0]);
    cpu.step();

    // On 80186+, count 32 is masked to 0, so value should be unchanged
    assert_eq!(
        cpu.ax & 0xFF,
        0xFF,
        "80186 should mask count to 5 bits (32 -> 0)"
    );
}

#[test]
fn test_shift_count_masking_80186_with_33() {
    // Test with count 33 which should be masked to 1
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80186);

    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0x00FF;
    cpu.cx = 0x0021; // CL = 33, masked to 1 on 80186+

    // SHL AL, CL (0xD2 with ModR/M 0b11_100_000)
    cpu.memory.load_program(0xFFFF0, &[0xD2, 0xE0]);
    cpu.step();

    // On 80186+, count 33 is masked to 1, so 0xFF << 1 = 0xFE
    assert_eq!(
        cpu.ax & 0xFF,
        0xFE,
        "80186 should mask count to 5 bits (33 -> 1)"
    );
}

#[test]
fn test_shift_immediate_invalid_on_8086() {
    // Test that shift by immediate (0xC0, 0xC1) is invalid on 8086
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel8086);

    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    // SHL AL, imm8 (0xC0 with ModR/M and immediate)
    cpu.memory.load_program(0xFFFF0, &[0xC0, 0xE0, 0x04]); // SHL AL, 4
    let cycles = cpu.step();
    assert_eq!(cycles, 10, "Shift by immediate should be invalid on 8086");

    // SHL AX, imm8 (0xC1 with ModR/M and immediate)
    cpu.ip = 0x0000;
    cpu.memory.load_program(0xFFFF0, &[0xC1, 0xE0, 0x04]); // SHL AX, 4
    let cycles = cpu.step();
    assert_eq!(cycles, 10, "Shift by immediate should be invalid on 8086");
}

#[test]
fn test_shift_immediate_valid_on_80186() {
    // Test that shift by immediate (0xC0, 0xC1) works on 80186
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80186);

    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0x00FF;

    // SHL AL, imm8 (0xC0 with ModR/M and immediate)
    cpu.memory.load_program(0xFFFF0, &[0xC0, 0xE0, 0x04]); // SHL AL, 4
    let cycles = cpu.step();
    assert!(cycles > 10, "Shift by immediate should work on 80186");
    assert_eq!(cpu.ax & 0xFF, 0xF0, "SHL AL, 4 should shift left by 4");
}

#[test]
fn test_mov_ax_preserves_high_16_bits() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // MOV AX, 0x5678 (0xB8, 0x78, 0x56)
    cpu.memory.load_program(0xFFFF0, &[0xB8, 0x78, 0x56]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0x12345678; // Set all 32 bits initially

    cpu.step();

    assert_eq!(cpu.ax & 0xFFFF, 0x5678, "AX should be set to 0x5678");
    assert_eq!(
        cpu.ax & 0xFFFF_0000,
        0x1234_0000,
        "High 16 bits should be preserved"
    );
}

#[test]
fn test_add_ax_preserves_high_16_bits() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // ADD AX, BX (0x01 with ModR/M 0b11_011_000 = register mode, BX to AX)
    cpu.memory.load_program(0xFFFF0, &[0x01, 0b11_011_000]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0xAABB1234; // AX=0x1234
    cpu.bx = 0xCCDD5678; // BX=0x5678

    cpu.step();

    // AX should be 0x1234 + 0x5678 = 0x68AC
    assert_eq!(cpu.ax & 0xFFFF, 0x68AC, "AX should be 0x1234 + 0x5678");
    assert_eq!(
        cpu.ax & 0xFFFF_0000,
        0xAABB_0000,
        "High 16 bits should be preserved"
    );
}

#[test]
fn test_sub_cx_preserves_high_16_bits() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // SUB CX, DX (0x29 with ModR/M 0b11_010_001 = register mode, DX to CX)
    cpu.memory.load_program(0xFFFF0, &[0x29, 0b11_010_001]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.cx = 0x11229999; // CX=0x9999
    cpu.dx = 0x33441111; // DX=0x1111

    cpu.step();

    // CX should be 0x9999 - 0x1111 = 0x8888
    assert_eq!(cpu.cx & 0xFFFF, 0x8888, "CX should be 0x9999 - 0x1111");
    assert_eq!(
        cpu.cx & 0xFFFF_0000,
        0x1122_0000,
        "High 16 bits should be preserved"
    );
}

#[test]
fn test_add_ax_imm16_preserves_high_bits() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // ADD AX, 0x1234 (0x05, 0x34, 0x12)
    cpu.memory.load_program(0xFFFF0, &[0x05, 0x34, 0x12]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0xAABB5678; // AX=0x5678

    cpu.step();

    // AX should be 0x5678 + 0x1234 = 0x68AC
    assert_eq!(cpu.ax & 0xFFFF, 0x68AC, "AX should be 0x5678 + 0x1234");
    assert_eq!(
        cpu.ax & 0xFFFF_0000,
        0xAABB_0000,
        "High 16 bits should be preserved"
    );
}

#[test]
fn test_sub_ax_imm16_preserves_high_bits() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // SUB AX, 0x1111 (0x2D, 0x11, 0x11)
    cpu.memory.load_program(0xFFFF0, &[0x2D, 0x11, 0x11]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0xCCDD9999; // AX=0x9999

    cpu.step();

    // AX should be 0x9999 - 0x1111 = 0x8888
    assert_eq!(cpu.ax & 0xFFFF, 0x8888, "AX should be 0x9999 - 0x1111");
    assert_eq!(
        cpu.ax & 0xFFFF_0000,
        0xCCDD_0000,
        "High 16 bits should be preserved"
    );
}

#[test]
fn test_and_ax_imm16_preserves_high_bits() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // AND AX, 0x0F0F (0x25, 0x0F, 0x0F)
    cpu.memory.load_program(0xFFFF0, &[0x25, 0x0F, 0x0F]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0x1122FFFF; // AX=0xFFFF

    cpu.step();

    // AX should be 0xFFFF & 0x0F0F = 0x0F0F
    assert_eq!(cpu.ax & 0xFFFF, 0x0F0F, "AX should be 0xFFFF & 0x0F0F");
    assert_eq!(
        cpu.ax & 0xFFFF_0000,
        0x1122_0000,
        "High 16 bits should be preserved"
    );
}

#[test]
fn test_or_ax_imm16_preserves_high_bits() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // OR AX, 0xF0F0 (0x0D, 0xF0, 0xF0)
    cpu.memory.load_program(0xFFFF0, &[0x0D, 0xF0, 0xF0]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0x33440F0F; // AX=0x0F0F

    cpu.step();

    // AX should be 0x0F0F | 0xF0F0 = 0xFFFF
    assert_eq!(cpu.ax & 0xFFFF, 0xFFFF, "AX should be 0x0F0F | 0xF0F0");
    assert_eq!(
        cpu.ax & 0xFFFF_0000,
        0x3344_0000,
        "High 16 bits should be preserved"
    );
}

#[test]
fn test_xor_ax_imm16_preserves_high_bits() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // XOR AX, 0xAAAA (0x35, 0xAA, 0xAA)
    cpu.memory.load_program(0xFFFF0, &[0x35, 0xAA, 0xAA]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0x5566FFFF; // AX=0xFFFF

    cpu.step();

    // AX should be 0xFFFF ^ 0xAAAA = 0x5555
    assert_eq!(cpu.ax & 0xFFFF, 0x5555, "AX should be 0xFFFF ^ 0xAAAA");
    assert_eq!(
        cpu.ax & 0xFFFF_0000,
        0x5566_0000,
        "High 16 bits should be preserved"
    );
}

#[test]
fn test_mul_16bit_preserves_high_bits() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // MUL BX (0xF7 with ModR/M 0b11_100_011 = MUL BX)
    cpu.memory.load_program(0xFFFF0, &[0xF7, 0b11_100_011]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0xAABB1000; // AX=0x1000
    cpu.bx = 0xCCDD2000; // BX=0x2000
    cpu.dx = 0x11223344; // Set high 16 bits to check preservation

    cpu.step();

    // Result: 0x1000 * 0x2000 = 0x2000000 = DX:AX = 0x0200:0x0000
    assert_eq!(cpu.ax & 0xFFFF, 0x0000, "AX low 16 bits = 0x0000");
    assert_eq!(cpu.dx & 0xFFFF, 0x0200, "DX low 16 bits = 0x0200");
    assert_eq!(
        cpu.ax & 0xFFFF_0000,
        0xAABB_0000,
        "AX high 16 bits should be preserved"
    );
    assert_eq!(
        cpu.dx & 0xFFFF_0000,
        0x1122_0000,
        "DX high 16 bits should be preserved"
    );
}

#[test]
fn test_imul_16bit_preserves_high_bits() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // IMUL CX (0xF7 with ModR/M 0b11_101_001 = IMUL CX)
    cpu.memory.load_program(0xFFFF0, &[0xF7, 0b11_101_001]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0x55660010; // AX=0x0010 (16)
    cpu.cx = 0x77880020; // CX=0x0020 (32)
    cpu.dx = 0x99AABBCC; // Set high 16 bits

    cpu.step();

    // Result: 16 * 32 = 512 = 0x0200 = DX:AX = 0x0000:0x0200
    assert_eq!(cpu.ax & 0xFFFF, 0x0200, "AX low 16 bits = 0x0200");
    assert_eq!(cpu.dx & 0xFFFF, 0x0000, "DX low 16 bits = 0x0000");
    assert_eq!(
        cpu.ax & 0xFFFF_0000,
        0x5566_0000,
        "AX high 16 bits should be preserved"
    );
    assert_eq!(
        cpu.dx & 0xFFFF_0000,
        0x99AA_0000,
        "DX high 16 bits should be preserved"
    );
}

#[test]
fn test_leave_preserves_high_bits() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80186);

    // LEAVE (0xC9)
    cpu.memory.load_program(0xFFFF0, &[0xC9]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ss = 0x1000;
    cpu.sp = 0x1000;
    cpu.bp = 0xAABB2000; // BP=0x2000, high bits set

    // Write value to stack at BP location [SS:BP] = [0x1000:0x2000] = 0x12000
    cpu.memory.write(0x12000, 0x34); // Low byte
    cpu.memory.write(0x12001, 0x12); // High byte (BP will be set to 0x1234)

    cpu.step();

    // SP should be set to BP (0x2000) then incremented by 2
    assert_eq!(cpu.sp & 0xFFFF, 0x2002, "SP should be old BP + 2");
    assert_eq!(cpu.bp & 0xFFFF, 0x1234, "BP should be popped value");
    assert_eq!(
        cpu.bp & 0xFFFF_0000,
        0xAABB_0000,
        "BP high 16 bits should be preserved"
    );
}

#[test]
fn test_popa_preserves_high_bits() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80186);

    // POPA (0x61)
    cpu.memory.load_program(0xFFFF0, &[0x61]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ss = 0x1000;
    cpu.sp = 0x1000;

    // Set high bits on all registers
    cpu.di = 0x11110001;
    cpu.si = 0x22220002;
    cpu.bp = 0x33330003;
    cpu.bx = 0x44440004;
    cpu.dx = 0x55550005;
    cpu.cx = 0x66660006;
    cpu.ax = 0x77770007;

    // Push values to stack (in POPA order: DI, SI, BP, SP, BX, DX, CX, AX)
    cpu.memory.write_u16(0x11000, 0xAAAA); // DI
    cpu.memory.write_u16(0x11002, 0xBBBB); // SI
    cpu.memory.write_u16(0x11004, 0xCCCC); // BP
    cpu.memory.write_u16(0x11006, 0xDDDD); // SP (ignored)
    cpu.memory.write_u16(0x11008, 0xEEEE); // BX
    cpu.memory.write_u16(0x1100A, 0xFFFF); // DX
    cpu.memory.write_u16(0x1100C, 0x1111); // CX
    cpu.memory.write_u16(0x1100E, 0x2222); // AX

    cpu.step();

    assert_eq!(cpu.di & 0xFFFF, 0xAAAA, "DI low 16 bits");
    assert_eq!(cpu.si & 0xFFFF, 0xBBBB, "SI low 16 bits");
    assert_eq!(cpu.bp & 0xFFFF, 0xCCCC, "BP low 16 bits");
    assert_eq!(cpu.bx & 0xFFFF, 0xEEEE, "BX low 16 bits");
    assert_eq!(cpu.dx & 0xFFFF, 0xFFFF, "DX low 16 bits");
    assert_eq!(cpu.cx & 0xFFFF, 0x1111, "CX low 16 bits");
    assert_eq!(cpu.ax & 0xFFFF, 0x2222, "AX low 16 bits");

    // Verify high 16 bits preserved
    assert_eq!(
        cpu.di & 0xFFFF_0000,
        0x1111_0000,
        "DI high 16 bits preserved"
    );
    assert_eq!(
        cpu.si & 0xFFFF_0000,
        0x2222_0000,
        "SI high 16 bits preserved"
    );
    assert_eq!(
        cpu.bp & 0xFFFF_0000,
        0x3333_0000,
        "BP high 16 bits preserved"
    );
    assert_eq!(
        cpu.bx & 0xFFFF_0000,
        0x4444_0000,
        "BX high 16 bits preserved"
    );
    assert_eq!(
        cpu.dx & 0xFFFF_0000,
        0x5555_0000,
        "DX high 16 bits preserved"
    );
    assert_eq!(
        cpu.cx & 0xFFFF_0000,
        0x6666_0000,
        "CX high 16 bits preserved"
    );
    assert_eq!(
        cpu.ax & 0xFFFF_0000,
        0x7777_0000,
        "AX high 16 bits preserved"
    );
}

#[test]
fn test_lodsw_preserves_high_bits() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // LODSW (0xAD)
    cpu.memory.load_program(0xFFFF0, &[0xAD]);
    cpu.memory.write(0x10100, 0x78); // Low byte
    cpu.memory.write(0x10101, 0x56); // High byte
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ds = 0x1010;
    cpu.si = 0x0000;
    cpu.ax = 0x11223344; // Set high 16 bits

    cpu.step();

    // AX should be loaded with 0x5678
    assert_eq!(cpu.ax & 0xFFFF, 0x5678, "AX should be loaded with 0x5678");
    assert_eq!(
        cpu.ax & 0xFFFF_0000,
        0x1122_0000,
        "High 16 bits should be preserved"
    );
    assert_eq!(cpu.si & 0xFFFF, 0x0002, "SI should be incremented by 2");
}

// Tests for AF flag in REP-prefixed 16-bit string operations
// These tests validate the fix for missing AF calculations similar to PR #222

#[test]
fn test_repe_cmpsw_af_with_borrow() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // Test case: src=0x0010, dst=0x0001
    // Subtraction: 0x0010 - 0x0001 = 0x000F
    // AF should be set (borrow from bit 4: (0x10 & 0xF) = 0, (0x01 & 0xF) = 1, 0 < 1)
    cpu.ds = 0x1000;
    cpu.es = 0x2000;
    cpu.si = 0x0100;
    cpu.di = 0x0200;
    cpu.cx = 0x0001; // Compare one word

    // Write test values to memory
    cpu.memory.write_u16(physical_address(0x1000, 0x0100), 0x0010); // DS:SI
    cpu.memory.write_u16(physical_address(0x2000, 0x0200), 0x0001); // ES:DI

    // REPE CMPSW (0xF3 0xA7)
    cpu.memory.load_program(0xFFFF0, &[0xF3, 0xA7]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    // AF should be set when there's a borrow from bit 4 to bit 3
    assert!(
        cpu.get_flag(FLAG_AF),
        "AF should be set when (src & 0xF) < (dst & 0xF)"
    );
}

#[test]
fn test_repe_cmpsw_af_no_borrow() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // Test case: src=0x0012, dst=0x0001
    // Subtraction: 0x0012 - 0x0001 = 0x0011
    // AF should NOT be set (no borrow: (0x12 & 0xF) = 2, (0x01 & 0xF) = 1, 2 >= 1)
    cpu.ds = 0x1000;
    cpu.es = 0x2000;
    cpu.si = 0x0100;
    cpu.di = 0x0200;
    cpu.cx = 0x0001;

    cpu.memory.write_u16(physical_address(0x1000, 0x0100), 0x0012);
    cpu.memory.write_u16(physical_address(0x2000, 0x0200), 0x0001);

    cpu.memory.load_program(0xFFFF0, &[0xF3, 0xA7]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    assert!(
        !cpu.get_flag(FLAG_AF),
        "AF should not be set when (src & 0xF) >= (dst & 0xF)"
    );
}

#[test]
fn test_repe_scasw_af_with_borrow() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // Test case: AX=0x0010, [ES:DI]=0x0001
    // Subtraction: 0x0010 - 0x0001 = 0x000F
    // AF should be set
    cpu.es = 0x2000;
    cpu.di = 0x0200;
    cpu.cx = 0x0001;
    cpu.ax = 0x0010;

    cpu.memory.write_u16(physical_address(0x2000, 0x0200), 0x0001);

    // REPE SCASW (0xF3 0xAF)
    cpu.memory.load_program(0xFFFF0, &[0xF3, 0xAF]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    assert!(
        cpu.get_flag(FLAG_AF),
        "AF should be set when (AX & 0xF) < (val & 0xF)"
    );
}

#[test]
fn test_repe_scasw_af_no_borrow() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // Test case: AX=0x0012, [ES:DI]=0x0001
    // Subtraction: 0x0012 - 0x0001 = 0x0011
    // AF should NOT be set
    cpu.es = 0x2000;
    cpu.di = 0x0200;
    cpu.cx = 0x0001;
    cpu.ax = 0x0012;

    cpu.memory.write_u16(physical_address(0x2000, 0x0200), 0x0001);

    cpu.memory.load_program(0xFFFF0, &[0xF3, 0xAF]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    assert!(
        !cpu.get_flag(FLAG_AF),
        "AF should not be set when (AX & 0xF) >= (val & 0xF)"
    );
}

#[test]
fn test_repne_cmpsw_af_with_borrow() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // Test case: src=0x0010, dst=0x0001
    // AF should be set
    cpu.ds = 0x1000;
    cpu.es = 0x2000;
    cpu.si = 0x0100;
    cpu.di = 0x0200;
    cpu.cx = 0x0001;

    cpu.memory.write_u16(physical_address(0x1000, 0x0100), 0x0010);
    cpu.memory.write_u16(physical_address(0x2000, 0x0200), 0x0001);

    // REPNE CMPSW (0xF2 0xA7)
    cpu.memory.load_program(0xFFFF0, &[0xF2, 0xA7]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    assert!(
        cpu.get_flag(FLAG_AF),
        "AF should be set when (src & 0xF) < (dst & 0xF)"
    );
}

#[test]
fn test_repne_cmpsw_af_no_borrow() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // Test case: src=0x0012, dst=0x0001
    // AF should NOT be set
    cpu.ds = 0x1000;
    cpu.es = 0x2000;
    cpu.si = 0x0100;
    cpu.di = 0x0200;
    cpu.cx = 0x0001;

    cpu.memory.write_u16(physical_address(0x1000, 0x0100), 0x0012);
    cpu.memory.write_u16(physical_address(0x2000, 0x0200), 0x0001);

    cpu.memory.load_program(0xFFFF0, &[0xF2, 0xA7]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    assert!(
        !cpu.get_flag(FLAG_AF),
        "AF should not be set when (src & 0xF) >= (dst & 0xF)"
    );
}

#[test]
fn test_repne_scasw_af_with_borrow() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // Test case: AX=0x0010, [ES:DI]=0x0001
    // AF should be set
    cpu.es = 0x2000;
    cpu.di = 0x0200;
    cpu.cx = 0x0001;
    cpu.ax = 0x0010;

    cpu.memory.write_u16(physical_address(0x2000, 0x0200), 0x0001);

    // REPNE SCASW (0xF2 0xAF)
    cpu.memory.load_program(0xFFFF0, &[0xF2, 0xAF]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    assert!(
        cpu.get_flag(FLAG_AF),
        "AF should be set when (AX & 0xF) < (val & 0xF)"
    );
}

#[test]
fn test_repne_scasw_af_no_borrow() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // Test case: AX=0x0012, [ES:DI]=0x0001
    // AF should NOT be set
    cpu.es = 0x2000;
    cpu.di = 0x0200;
    cpu.cx = 0x0001;
    cpu.ax = 0x0012;

    cpu.memory.write_u16(physical_address(0x2000, 0x0200), 0x0001);

    cpu.memory.load_program(0xFFFF0, &[0xF2, 0xAF]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    assert!(
        !cpu.get_flag(FLAG_AF),
        "AF should not be set when (AX & 0xF) >= (val & 0xF)"
    );
}
