//! Tests for 32-bit operations (80386+ instructions)
//!
//! This module contains tests for 32-bit operations on EAX, EBX, ECX, EDX, ESI, EDI, EBP, ESP

use crate::cpu_8086::ArrayMemory;
use crate::cpu_8086::{
    Cpu8086, CpuModel, Memory8086, FLAG_AF, FLAG_CF, FLAG_DF, FLAG_OF, FLAG_PF, FLAG_SF, FLAG_ZF,
};

// Helper function for tests to calculate physical address
fn physical_address(segment: u16, offset: u16) -> u32 {
    ((segment as u32) << 4) + (offset as u32)
}

#[test]
fn test_operand_size_override_prefix() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    // Setup: 0x66 prefix followed by NOP at 0x0000:0x0100
    // 0x66 = Operand-size override prefix
    // 0x90 = NOP
    cpu.memory.load_program(0x0100, &[0x66, 0x90]);

    cpu.ip = 0x0100;
    cpu.cs = 0x0000;

    // Execute 0x66 NOP
    cpu.step();

    // The operand_size_override flag should be cleared after instruction
    assert!(
        !cpu.operand_size_override,
        "Operand size override should be cleared after instruction"
    );
}

#[test]
fn test_operand_size_override_mov_imm32() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    // Setup: 0x66 0xC7 0xC0 (MOV EAX, imm32) at 0x0000:0x0100
    // 0x66 = Operand-size override
    // 0xC7 = MOV r/m, imm
    // ModR/M: 0xC0 (mod=11, op=0, r/m=AX)
    // Immediate: 0x78563412 (little-endian: 12 34 56 78)
    cpu.memory
        .load_program(0x0100, &[0x66, 0xC7, 0xC0, 0x12, 0x34, 0x56, 0x78]);

    cpu.ip = 0x0100;
    cpu.cs = 0x0000;
    cpu.ax = 0x0000;

    // Execute 0x66 MOV EAX, imm32
    cpu.step();

    // With full 32-bit support, we now store all 32 bits
    assert_eq!(
        cpu.get_reg32(0),
        0x78563412,
        "EAX should contain full 32-bit immediate"
    );
    // Verify IP advanced correctly (consumed all 7 bytes)
    assert_eq!(cpu.ip, 0x0107, "IP should advance by 7 bytes");
}

#[test]
fn test_operand_size_override_mov_imm32_memory() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    // Setup: 0x66 0xC7 0x06 (MOV [addr], imm32) at 0x0000:0x0100
    // 0x66 = Operand-size override
    // 0xC7 = MOV r/m, imm
    // ModR/M: 0x06 (mod=00, op=0, r/m=110 = direct address)
    // Address: 0x0200
    // Immediate: 0xDEADBEEF (little-endian: EF BE AD DE)
    cpu.memory.load_program(
        0x0100,
        &[0x66, 0xC7, 0x06, 0x00, 0x02, 0xEF, 0xBE, 0xAD, 0xDE],
    );

    cpu.ip = 0x0100;
    cpu.cs = 0x0000;
    cpu.ds = 0x1000;

    // Execute 0x66 MOV [0x0200], imm32
    cpu.step();

    // Verify 32-bit value was written to memory
    let val_32 = cpu.read_u32(0x1000, 0x0200);
    assert_eq!(val_32, 0xDEADBEEF, "Full 32-bit value should be written");
    // Verify IP advanced correctly (consumed all 9 bytes)
    assert_eq!(cpu.ip, 0x0109, "IP should advance by 9 bytes");
}

#[test]
fn test_mov_rm32_imm32() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    // MOV EAX, 0xCAFEBABE: opcode 0xC7, ModR/M = 0xC0 (mod=11, op=0, rm=000)
    cpu.cs = 0;
    cpu.ip = 0x1000;
    let addr = Cpu8086::<ArrayMemory>::physical_address(cpu.cs, 0x1000);
    cpu.memory.write(addr, 0x66); // Operand size override
    cpu.memory.write(addr + 1, 0xC7); // MOV opcode
    cpu.memory.write(addr + 2, 0xC0); // ModR/M
                                      // Immediate value 0xCAFEBABE (little-endian)
    cpu.memory.write(addr + 3, 0xBE);
    cpu.memory.write(addr + 4, 0xBA);
    cpu.memory.write(addr + 5, 0xFE);
    cpu.memory.write(addr + 6, 0xCA);

    // Execute the instruction
    cpu.step();

    // Verify EAX was set to immediate value
    assert_eq!(
        cpu.get_reg32(0),
        0xCAFEBABE,
        "EAX should contain immediate value"
    );
}
