//! Tests for miscellaneous operations
//!
//! This module contains tests for system instructions, I/O, segment operations,
//! string operations, BCD arithmetic, and other special instructions

use crate::cpu_8086::ArrayMemory;
use crate::cpu_8086::{
    Cpu8086, CpuModel, Memory8086, FLAG_AF, FLAG_CF, FLAG_DF, FLAG_IF, FLAG_OF, FLAG_PF, FLAG_SF,
    FLAG_ZF,
};

// Helper function for tests to calculate physical address
fn physical_address(segment: u16, offset: u16) -> u32 {
    ((segment as u32) << 4) + (offset as u32)
}

#[test]
fn test_cpu_initialization() {
    let mem = ArrayMemory::new();
    let cpu = Cpu8086::new(mem);

    assert_eq!(cpu.ax, 0);
    assert_eq!(cpu.bx, 0);
    assert_eq!(cpu.cx, 0);
    assert_eq!(cpu.dx, 0);
    assert_eq!(cpu.cs, 0xFFFF);
    assert_eq!(cpu.ds, 0);
    assert_eq!(cpu.flags & 0x0002, 0x0002); // Reserved bit
}

#[test]
fn test_enter_leave() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80186);

    cpu.sp = 0x0100;
    cpu.bp = 0x5555;
    cpu.ss = 0x1000;

    // ENTER 16, 0 (0xC8 size_low size_high nesting)
    cpu.memory.load_program(0xFFFF0, &[0xC8, 0x10, 0x00, 0x00]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    // BP should be saved and set to old SP - 2
    let expected_bp = 0x00FE;
    assert_eq!(cpu.bp, expected_bp);
    // SP should be decremented by 2 (push BP) + 16 (local space)
    assert_eq!(cpu.sp, 0x00EE);

    // Now test LEAVE (0xC9)
    cpu.memory.load_program(0xFFFF0, &[0xC9]);
    cpu.ip = 0x0000;

    cpu.step();

    // SP should be restored to BP + 2 (after popping BP)
    assert_eq!(cpu.sp, 0x0100);
    // BP should be popped (restored to 0x5555)
    assert_eq!(cpu.bp, 0x5555);
}
