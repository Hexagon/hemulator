//! Tests for shift and rotate instructions
//!
//! This module contains tests for shift/rotate edge cases
//! 
//! NOTE: SHLD/SHRD are currently not fully implemented (marked as stubs in cpu_8086.rs)
//! Tests for these instructions are omitted until implementation is complete.

use crate::cpu_8086::ArrayMemory;
use crate::cpu_8086::{Cpu8086, CpuModel, FLAG_CF, FLAG_OF, FLAG_SF, FLAG_ZF};

#[test]
fn test_inc_preserves_carry_flag() {
    // INC/DEC do NOT affect the carry flag - this is critical!
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // Set carry flag
    cpu.set_flag(FLAG_CF, true);
    cpu.ax = 0x0005;

    // INC AX (0x40)
    cpu.memory.load_program(0xFFFF0, &[0x40]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.step();

    assert_eq!(cpu.ax, 0x0006);
    assert!(cpu.get_flag(FLAG_CF), "INC must NOT affect carry flag");
}

#[test]
fn test_dec_preserves_carry_flag() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // Set carry flag
    cpu.set_flag(FLAG_CF, true);
    cpu.ax = 0x0005;

    // DEC AX (0x48)
    cpu.memory.load_program(0xFFFF0, &[0x48]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.step();

    assert_eq!(cpu.ax, 0x0004);
    assert!(cpu.get_flag(FLAG_CF), "DEC must NOT affect carry flag");
}

#[test]
fn test_inc_zero_to_one() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.ax = 0x0000;

    // INC AX
    cpu.memory.load_program(0xFFFF0, &[0x40]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.step();

    assert_eq!(cpu.ax, 0x0001);
    assert!(!cpu.get_flag(FLAG_ZF));
    assert!(!cpu.get_flag(FLAG_SF));
}

#[test]
fn test_dec_one_to_zero() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.ax = 0x0001;

    // DEC AX
    cpu.memory.load_program(0xFFFF0, &[0x48]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.step();

    assert_eq!(cpu.ax, 0x0000);
    assert!(cpu.get_flag(FLAG_ZF));
    assert!(!cpu.get_flag(FLAG_SF));
}

#[test]
fn test_dec_zero_wraps() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.ax = 0x0000;

    // DEC AX
    cpu.memory.load_program(0xFFFF0, &[0x48]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.step();

    assert_eq!(cpu.ax, 0xFFFF);
    assert!(!cpu.get_flag(FLAG_ZF));
    assert!(cpu.get_flag(FLAG_SF));
}

#[test]
fn test_inc_overflow_edge_case() {
    // Test INC at the overflow boundary
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.ax = 0x7FFF; // Maximum positive signed 16-bit value

    // INC AX
    cpu.memory.load_program(0xFFFF0, &[0x40]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.step();

    assert_eq!(cpu.ax, 0x8000);
    assert!(cpu.get_flag(FLAG_OF), "Overflow flag should be set");
    assert!(cpu.get_flag(FLAG_SF), "Sign flag should be set");
}

#[test]
fn test_shl_multiple_by_cl_edge_cases() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // Test shifting by 0
    cpu.ax = 0x1234;
    cpu.cx = 0x0000; // CL = 0

    // SHL AX, CL (0xD3 with ModR/M 0xE0)
    cpu.memory.load_program(0xFFFF0, &[0xD3, 0xE0]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.step();

    assert_eq!(cpu.ax, 0x1234, "SHL by 0 should not change value");
}

#[test]
fn test_sar_sign_extension() {
    // SAR (arithmetic shift right) must preserve the sign bit
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.ax = 0x8000; // Negative number in signed representation

    // SAR AX, 1 (0xD1 with ModR/M 0xF8)
    cpu.memory.load_program(0xFFFF0, &[0xD1, 0xF8]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.step();

    // Sign bit should be preserved: 0x8000 >> 1 = 0xC000 (not 0x4000)
    assert_eq!(cpu.ax, 0xC000);
    assert!(cpu.get_flag(FLAG_SF), "Sign flag should remain set");
}

#[test]
fn test_shr_no_sign_extension() {
    // SHR (logical shift right) fills with zeros
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.ax = 0x8000;

    // SHR AX, 1 (0xD1 with ModR/M 0xE8)
    cpu.memory.load_program(0xFFFF0, &[0xD1, 0xE8]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.step();

    // Logical shift: 0x8000 >> 1 = 0x4000
    assert_eq!(cpu.ax, 0x4000);
    assert!(!cpu.get_flag(FLAG_SF), "Sign flag should be clear");
}

#[test]
fn test_rcl_through_carry() {
    // RCL rotates through carry flag
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.ax = 0x8000;
    cpu.set_flag(FLAG_CF, false);

    // RCL AX, 1 (0xD1 with ModR/M 0xD0)
    cpu.memory.load_program(0xFFFF0, &[0xD1, 0xD0]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.step();

    // 0x8000 rotated left through carry:
    // Bit 15 (1) goes to CF, CF (0) goes to bit 0
    // Result: 0x0000
    assert_eq!(cpu.ax, 0x0000);
    assert!(cpu.get_flag(FLAG_CF), "Bit 15 should be in carry");
}

#[test]
fn test_rcr_through_carry() {
    // RCR rotates through carry flag
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.ax = 0x0001;
    cpu.set_flag(FLAG_CF, false);

    // RCR AX, 1 (0xD1 with ModR/M 0xD8)
    cpu.memory.load_program(0xFFFF0, &[0xD1, 0xD8]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.step();

    // 0x0001 rotated right through carry:
    // Bit 0 (1) goes to CF, CF (0) goes to bit 15
    // Result: 0x0000
    assert_eq!(cpu.ax, 0x0000);
    assert!(cpu.get_flag(FLAG_CF), "Bit 0 should be in carry");
}
