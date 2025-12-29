//! Tests for addressing modes and memory access edge cases
//!
//! This module tests various addressing mode combinations and memory access patterns

use crate::cpu_8086::ArrayMemory;
use crate::cpu_8086::{Cpu8086, CpuModel, Memory8086, FLAG_ZF};

#[test]
fn test_xchg_ax_ax_is_nop() {
    // XCHG AX, AX is encoded as NOP (0x90)
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.ax = 0x1234;
    cpu.memory.load_program(0xFFFF0, &[0x90]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    assert_eq!(cpu.ax, 0x1234, "XCHG AX, AX (NOP) should not change AX");
}

#[test]
fn test_xchg_registers() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.ax = 0x1234;
    cpu.bx = 0x5678;

    // XCHG AX, BX (0x93)
    cpu.memory.load_program(0xFFFF0, &[0x93]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    assert_eq!(cpu.ax, 0x5678);
    assert_eq!(cpu.bx, 0x1234);
}

#[test]
fn test_xchg_memory_register() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.ds = 0x1000;
    cpu.bx = 0x0100;
    cpu.ax = 0xAAAA;

    // Write value to memory
    let addr = Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x0100);
    cpu.memory.write(addr, 0x34);
    cpu.memory.write(addr + 1, 0x12);

    // XCHG AX, [BX] (0x87 with ModR/M 0x07)
    cpu.memory.load_program(0xFFFF0, &[0x87, 0x07]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    // AX should have memory value
    assert_eq!(cpu.ax, 0x1234);
    // Memory should have old AX value
    let mem_val = cpu.memory.read(addr) as u16 | ((cpu.memory.read(addr + 1) as u16) << 8);
    assert_eq!(mem_val, 0xAAAA);
}

#[test]
fn test_lea_various_modes() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.ds = 0x1000;
    cpu.bx = 0x0100;
    cpu.si = 0x0020;

    // LEA AX, [BX+SI+disp] (0x8D with ModR/M and displacement)
    // ModR/M: 0b10_000_000 (mod=10 for disp16, reg=AX, rm=[BX+SI])
    cpu.memory.load_program(0xFFFF0, &[0x8D, 0x80, 0x50, 0x00]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    // AX should contain effective address: BX + SI + 0x0050
    assert_eq!(cpu.ax, 0x0100 + 0x0020 + 0x0050);
}

#[test]
fn test_addressing_mode_bp_uses_ss() {
    // BP-based addressing should use SS segment by default
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.ss = 0x2000; // Stack segment
    cpu.bp = 0x0100;

    // Write test value to SS:BP
    let addr = Cpu8086::<ArrayMemory>::physical_address(0x2000, 0x0100);
    cpu.memory.write(addr, 0x42);

    // MOV AL, [BP] (0x8A with ModR/M 0x46 0x00)
    // ModR/M: 0b01_000_110 (mod=01, reg=AL, rm=BP+disp8)
    cpu.memory.load_program(0xFFFF0, &[0x8A, 0x46, 0x00]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    assert_eq!(cpu.ax & 0xFF, 0x42, "BP addressing should use SS segment");
}

#[test]
fn test_addressing_mode_bp_with_segment_override() {
    // BP with segment override should use overridden segment
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.ss = 0x2000;
    cpu.ds = 0x1000;
    cpu.bp = 0x0100;

    // Write test value to DS:BP (not SS:BP)
    let addr = Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x0100);
    cpu.memory.write(addr, 0x99);

    // DS: override + MOV AL, [BP]
    cpu.memory.load_program(0xFFFF0, &[0x3E, 0x8A, 0x46, 0x00]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    assert_eq!(cpu.ax & 0xFF, 0x99, "DS: override should work with BP");
}

#[test]
fn test_mov_immediate_to_memory() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.ds = 0x1000;

    // MOV WORD PTR [0x0200], 0xABCD (0xC7 0x06 addr imm)
    cpu.memory.load_program(
        0xFFFF0,
        &[
            0xC7, 0x06, 0x00, 0x02, // MOV [0x0200], ...
            0xCD, 0xAB, // ... 0xABCD
        ],
    );
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    // Verify memory was written
    let addr = Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x0200);
    let val = cpu.memory.read(addr) as u16 | ((cpu.memory.read(addr + 1) as u16) << 8);
    assert_eq!(val, 0xABCD);
}

#[test]
fn test_segment_wrap_around() {
    // Test that address calculation wraps at segment boundaries
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.ds = 0xFFFF;
    cpu.si = 0xFFFF;

    // Calculate physical address for DS:SI
    let addr = Cpu8086::<ArrayMemory>::physical_address(0xFFFF, 0xFFFF);

    // This should wrap: (0xFFFF << 4) + 0xFFFF = 0xFFFF0 + 0xFFFF = 0x10FFEF
    assert_eq!(addr, 0x10FFEF);
}

#[test]
fn test_zero_displacement_addressing() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.ds = 0x1000;
    cpu.bx = 0x0200;

    // Write test value
    let addr = Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x0200);
    cpu.memory.write(addr, 0x77);

    // MOV AL, [BX+0] (with explicit 0 displacement)
    // ModR/M: 0b01_000_111 (mod=01 for disp8, reg=AL, rm=BX)
    cpu.memory.load_program(0xFFFF0, &[0x8A, 0x47, 0x00]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    assert_eq!(cpu.ax & 0xFF, 0x77);
}

#[test]
fn test_negative_displacement() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.ds = 0x1000;
    cpu.bp = 0x0100;

    // Write test value at BP-4
    let addr = Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x00FC); // 0x0100 - 4
    cpu.memory.write(addr, 0x88);

    // MOV AL, [BP-4] (with -4 displacement = 0xFC in two's complement)
    // ModR/M: 0b01_000_110 (mod=01 for disp8, reg=AL, rm=BP)
    cpu.memory.load_program(0xFFFF0, &[0x8A, 0x46, 0xFC]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ss = 0x1000; // BP uses SS by default

    cpu.step();

    assert_eq!(cpu.ax & 0xFF, 0x88);
}

#[test]
fn test_string_operations_with_address_wrap() {
    // Test that string operations handle 16-bit address wrap correctly
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.ds = 0x1000;
    cpu.es = 0x2000;
    cpu.si = 0xFFFF; // Will wrap to 0x0000 after increment (in 16-bit mode)
    cpu.di = 0x0100;

    // Write source data
    let src_addr = Cpu8086::<ArrayMemory>::physical_address(0x1000, 0xFFFF);
    cpu.memory.write(src_addr, 0xAA);

    // MOVSB
    cpu.memory.load_program(0xFFFF0, &[0xA4]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    // SI internally increments to 0x10000 but only the lower 16 bits are used
    // So effectively it wraps to 0x0000 in 16-bit addressing mode
    assert_eq!(cpu.si & 0xFFFF, 0x0000, "SI should wrap at 16-bit boundary");
    assert_eq!(cpu.di & 0xFFFF, 0x0101);

    // Verify data copied
    let dst_addr = Cpu8086::<ArrayMemory>::physical_address(0x2000, 0x0100);
    assert_eq!(cpu.memory.read(dst_addr), 0xAA);
}

#[test]
fn test_cmpxchg_memory_operand() {
    // Test CMPXCHG with memory operand (80486+)
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80486);

    cpu.ds = 0x1000;
    cpu.bx = 0x0100;
    cpu.ax = 0x1234;
    cpu.cx = 0x5678;

    // Write test value to memory
    let addr = Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x0100);
    cpu.memory.write(addr, 0x34);
    cpu.memory.write(addr + 1, 0x12);

    // CMPXCHG [BX], CX (0x0F 0xB1 with ModR/M)
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0xB1, 0x0F]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    // Memory should be updated with CX (since AX == [BX])
    let mem_val = cpu.memory.read(addr) as u16 | ((cpu.memory.read(addr + 1) as u16) << 8);
    assert_eq!(mem_val, 0x5678);
    assert!(cpu.get_flag(FLAG_ZF));
}

#[test]
fn test_bswap_all_registers() {
    // Test BSWAP on different registers (80486+)
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80486);

    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    // Test each register
    for reg in 0..8 {
        cpu.set_reg32(reg, 0x12345678);

        // BSWAP reg (0x0F 0xC8+reg)
        cpu.memory.load_program(0xFFFF0, &[0x0F, 0xC8 + reg as u8]);
        cpu.ip = 0x0000;
        cpu.step();

        assert_eq!(cpu.get_reg32(reg), 0x78563412, "BSWAP on register {}", reg);
    }
}
