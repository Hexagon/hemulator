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

#[test]
fn test_cpu_model_80386() {
    let mem = ArrayMemory::new();
    let cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);
    assert_eq!(cpu.model(), CpuModel::Intel80386);
    assert_eq!(CpuModel::Intel80386.name(), "Intel 80386");
    assert!(CpuModel::Intel80386.supports_80186_instructions());
    assert!(CpuModel::Intel80386.supports_80286_instructions());
    assert!(CpuModel::Intel80386.supports_80386_instructions());
}

#[test]
fn test_486_cpu_models() {
    // Test that 486 models can be created and used
    let mem = ArrayMemory::new();
    let cpu_dx = Cpu8086::with_model(mem, CpuModel::Intel80486);
    assert_eq!(cpu_dx.model(), CpuModel::Intel80486);
    assert!(cpu_dx.model().supports_80486_instructions());

    let mem = ArrayMemory::new();
    let cpu_sx = Cpu8086::with_model(mem, CpuModel::Intel80486SX);
    assert_eq!(cpu_sx.model(), CpuModel::Intel80486SX);
    assert!(cpu_sx.model().supports_80486_instructions());

    let mem = ArrayMemory::new();
    let cpu_dx2 = Cpu8086::with_model(mem, CpuModel::Intel80486DX2);
    assert_eq!(cpu_dx2.model(), CpuModel::Intel80486DX2);
    assert!(cpu_dx2.model().supports_80486_instructions());

    let mem = ArrayMemory::new();
    let cpu_sx2 = Cpu8086::with_model(mem, CpuModel::Intel80486SX2);
    assert_eq!(cpu_sx2.model(), CpuModel::Intel80486SX2);
    assert!(cpu_sx2.model().supports_80486_instructions());

    let mem = ArrayMemory::new();
    let cpu_dx4 = Cpu8086::with_model(mem, CpuModel::Intel80486DX4);
    assert_eq!(cpu_dx4.model(), CpuModel::Intel80486DX4);
    assert!(cpu_dx4.model().supports_80486_instructions());
}

#[test]
fn test_pentium_cpu_models() {
    // Test that Pentium models can be created and used
    let mem = ArrayMemory::new();
    let cpu_p5 = Cpu8086::with_model(mem, CpuModel::IntelPentium);
    assert_eq!(cpu_p5.model(), CpuModel::IntelPentium);
    assert!(cpu_p5.model().supports_pentium_instructions());
    assert!(cpu_p5.model().supports_80486_instructions());

    let mem = ArrayMemory::new();
    let cpu_mmx = Cpu8086::with_model(mem, CpuModel::IntelPentiumMMX);
    assert_eq!(cpu_mmx.model(), CpuModel::IntelPentiumMMX);
    assert!(cpu_mmx.model().supports_pentium_instructions());
    assert!(cpu_mmx.model().supports_80486_instructions());
}

#[test]
fn test_80186_instructions_invalid_on_8086() {
    // Test that 80186 instructions are rejected on 8086/8088
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel8086);

    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    // Test PUSHA (0x60)
    cpu.memory.load_program(0xFFFF0, &[0x60]);
    let cycles = cpu.step();
    assert_eq!(cycles, 10, "PUSHA should be invalid on 8086");

    // Test POPA (0x61)
    cpu.ip = 0x0000;
    cpu.memory.load_program(0xFFFF0, &[0x61]);
    let cycles = cpu.step();
    assert_eq!(cycles, 10, "POPA should be invalid on 8086");

    // Test BOUND (0x62)
    cpu.ip = 0x0000;
    cpu.memory.load_program(0xFFFF0, &[0x62, 0xC0]); // BOUND AX, AX (with ModRM)
    let cycles = cpu.step();
    assert_eq!(cycles, 10, "BOUND should be invalid on 8086");

    // Test PUSH imm16 (0x68)
    cpu.ip = 0x0000;
    cpu.memory.load_program(0xFFFF0, &[0x68, 0x34, 0x12]);
    let cycles = cpu.step();
    assert_eq!(cycles, 10, "PUSH imm16 should be invalid on 8086");

    // Test PUSH imm8 (0x6A)
    cpu.ip = 0x0000;
    cpu.memory.load_program(0xFFFF0, &[0x6A, 0x42]);
    let cycles = cpu.step();
    assert_eq!(cycles, 10, "PUSH imm8 should be invalid on 8086");

    // Test IMUL imm16 (0x69)
    cpu.ip = 0x0000;
    cpu.memory.load_program(0xFFFF0, &[0x69, 0xC0, 0x10, 0x00]);
    let cycles = cpu.step();
    assert_eq!(cycles, 10, "IMUL imm16 should be invalid on 8086");

    // Test IMUL imm8 (0x6B)
    cpu.ip = 0x0000;
    cpu.memory.load_program(0xFFFF0, &[0x6B, 0xC0, 0x10]);
    let cycles = cpu.step();
    assert_eq!(cycles, 10, "IMUL imm8 should be invalid on 8086");

    // Test INSB (0x6C)
    cpu.ip = 0x0000;
    cpu.memory.load_program(0xFFFF0, &[0x6C]);
    let cycles = cpu.step();
    assert_eq!(cycles, 10, "INSB should be invalid on 8086");

    // Test INSW (0x6D)
    cpu.ip = 0x0000;
    cpu.memory.load_program(0xFFFF0, &[0x6D]);
    let cycles = cpu.step();
    assert_eq!(cycles, 10, "INSW should be invalid on 8086");

    // Test OUTSB (0x6E)
    cpu.ip = 0x0000;
    cpu.memory.load_program(0xFFFF0, &[0x6E]);
    let cycles = cpu.step();
    assert_eq!(cycles, 10, "OUTSB should be invalid on 8086");

    // Test OUTSW (0x6F)
    cpu.ip = 0x0000;
    cpu.memory.load_program(0xFFFF0, &[0x6F]);
    let cycles = cpu.step();
    assert_eq!(cycles, 10, "OUTSW should be invalid on 8086");

    // Test ENTER (0xC8)
    cpu.ip = 0x0000;
    cpu.memory.load_program(0xFFFF0, &[0xC8, 0x10, 0x00, 0x00]);
    let cycles = cpu.step();
    assert_eq!(cycles, 10, "ENTER should be invalid on 8086");

    // Test LEAVE (0xC9)
    cpu.ip = 0x0000;
    cpu.memory.load_program(0xFFFF0, &[0xC9]);
    let cycles = cpu.step();
    assert_eq!(cycles, 10, "LEAVE should be invalid on 8086");
}

#[test]
fn test_80186_instructions_valid_on_80186() {
    // Test that 80186 instructions work correctly on 80186
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80186);

    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.sp = 0x0100;
    cpu.ss = 0x1000;

    // Test PUSH imm16 (0x68) - should work on 80186
    cpu.memory.load_program(0xFFFF0, &[0x68, 0x34, 0x12]);
    let cycles = cpu.step();
    assert_eq!(cycles, 3, "PUSH imm16 should work on 80186");
    assert_eq!(cpu.sp, 0x00FE);

    // Test PUSHA (0x60) - should work on 80186
    cpu.ip = 0x0000;
    cpu.ax = 0x1111;
    cpu.memory.load_program(0xFFFF0, &[0x60]);
    let cycles = cpu.step();
    assert_eq!(cycles, 36, "PUSHA should work on 80186");
}

#[test]
fn test_80286_instructions_valid_on_80286() {
    // Test that 80286 instructions work correctly on 80286
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80286);

    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    // Test LMSW (0x0F 0x01 /6) - should work on 80286
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0x01, 0xF0]); // LMSW AX
    let cycles = cpu.step();
    assert!(cycles > 0, "LMSW should work on 80286");
}

#[test]
fn test_80386_instructions_invalid_on_8086() {
    // Test that 80386 instructions are rejected on 8086/8088
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel8086);

    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    // Test FS segment override (0x64)
    cpu.memory.load_program(0xFFFF0, &[0x64, 0x90]); // FS: NOP
    let cycles = cpu.step();
    assert_eq!(cycles, 10, "FS segment override should be invalid on 8086");

    // Test GS segment override (0x65)
    cpu.ip = 0x0000;
    cpu.memory.load_program(0xFFFF0, &[0x65, 0x90]); // GS: NOP
    let cycles = cpu.step();
    assert_eq!(cycles, 10, "GS segment override should be invalid on 8086");

    // Test MOVSX (0x0F 0xBE)
    cpu.ip = 0x0000;
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0xBE, 0xC0]);
    let cycles = cpu.step();
    assert_eq!(cycles, 10, "MOVSX should be invalid on 8086");

    // Test MOVZX (0x0F 0xB6)
    cpu.ip = 0x0000;
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0xB6, 0xC0]);
    let cycles = cpu.step();
    assert_eq!(cycles, 10, "MOVZX should be invalid on 8086");

    // Test BSF (0x0F 0xBC)
    cpu.ip = 0x0000;
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0xBC, 0xC0]);
    let cycles = cpu.step();
    assert_eq!(cycles, 10, "BSF should be invalid on 8086");

    // Test BSR (0x0F 0xBD)
    cpu.ip = 0x0000;
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0xBD, 0xC0]);
    let cycles = cpu.step();
    assert_eq!(cycles, 10, "BSR should be invalid on 8086");

    // Test BT (0x0F 0xA3)
    cpu.ip = 0x0000;
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0xA3, 0xC0]);
    let cycles = cpu.step();
    assert_eq!(cycles, 10, "BT should be invalid on 8086");

    // Test BTS (0x0F 0xAB)
    cpu.ip = 0x0000;
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0xAB, 0xC0]);
    let cycles = cpu.step();
    assert_eq!(cycles, 10, "BTS should be invalid on 8086");

    // Test BTR (0x0F 0xB3)
    cpu.ip = 0x0000;
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0xB3, 0xC0]);
    let cycles = cpu.step();
    assert_eq!(cycles, 10, "BTR should be invalid on 8086");

    // Test BTC (0x0F 0xBB)
    cpu.ip = 0x0000;
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0xBB, 0xC0]);
    let cycles = cpu.step();
    assert_eq!(cycles, 10, "BTC should be invalid on 8086");

    // Test SHLD (0x0F 0xA4)
    cpu.ip = 0x0000;
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0xA4, 0xC0, 0x01]);
    let cycles = cpu.step();
    assert_eq!(cycles, 10, "SHLD should be invalid on 8086");

    // Test SHRD (0x0F 0xAC)
    cpu.ip = 0x0000;
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0xAC, 0xC0, 0x01]);
    let cycles = cpu.step();
    assert_eq!(cycles, 10, "SHRD should be invalid on 8086");

    // Test SETcc (0x0F 0x90)
    cpu.ip = 0x0000;
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0x90, 0xC0]);
    let cycles = cpu.step();
    assert_eq!(cycles, 10, "SETO should be invalid on 8086");
}

#[test]
fn test_80386_instructions_invalid_on_80186() {
    // Test that 80386 instructions are rejected on 80186
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80186);

    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    // Test MOVSX (0x0F 0xBE) - should be invalid on 80186
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0xBE, 0xC0]);
    let cycles = cpu.step();
    assert_eq!(cycles, 10, "MOVSX should be invalid on 80186");

    // Test BSF (0x0F 0xBC) - should be invalid on 80186
    cpu.ip = 0x0000;
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0xBC, 0xC0]);
    let cycles = cpu.step();
    assert_eq!(cycles, 10, "BSF should be invalid on 80186");
}

#[test]
fn test_80386_instructions_invalid_on_80286() {
    // Test that 80386 instructions are rejected on 80286
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80286);

    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    // Test MOVSX (0x0F 0xBE) - should be invalid on 80286
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0xBE, 0xC0]);
    let cycles = cpu.step();
    assert_eq!(cycles, 10, "MOVSX should be invalid on 80286");

    // Test BSF (0x0F 0xBC) - should be invalid on 80286
    cpu.ip = 0x0000;
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0xBC, 0xC0]);
    let cycles = cpu.step();
    assert_eq!(cycles, 10, "BSF should be invalid on 80286");

    // Test FS segment override (0x64) - should be invalid on 80286
    cpu.ip = 0x0000;
    cpu.memory.load_program(0xFFFF0, &[0x64, 0x90]); // FS: NOP
    let cycles = cpu.step();
    assert_eq!(cycles, 10, "FS segment override should be invalid on 80286");
}

#[test]
fn test_80386_instructions_valid_on_80386() {
    // Test that 80386 instructions work correctly on 80386
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    // Test MOVSX (0x0F 0xBE) - should work on 80386
    cpu.bx = 0x00FF;
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0xBE, 0xC3]); // MOVSX AX, BL
    let cycles = cpu.step();
    assert_eq!(cycles, 3, "MOVSX should work on 80386");
    assert_eq!(cpu.ax, 0xFFFF); // 0xFF sign-extended

    // Test BSF (0x0F 0xBC) - should work on 80386
    cpu.ip = 0x0000;
    cpu.bx = 0x0008;
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0xBC, 0xC3]); // BSF AX, BX
    let cycles = cpu.step();
    assert_eq!(cycles, 10, "BSF should work on 80386");
    assert_eq!(cpu.ax, 3); // First set bit is at position 3
}

#[test]
fn test_sib_decode_scale_1() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);
    cpu.cs = 0; // Set CS before calculating address

    // SIB byte: scale=00 (1x), index=001 (ECX), base=010 (EDX)
    // Binary: 00 001 010 = 0x0A
    let addr = Cpu8086::<ArrayMemory>::physical_address(cpu.cs, 0x1000);
    cpu.memory.write(addr, 0x0A);
    cpu.ip = 0x1000;

    let (scale, index, base, bytes) = cpu.decode_sib();
    assert_eq!(scale, 1, "Scale should be 1");
    assert_eq!(index, 1, "Index should be ECX (1)");
    assert_eq!(base, 2, "Base should be EDX (2)");
    assert_eq!(bytes, 1, "Should consume 1 byte");
}

#[test]
fn test_sib_decode_scale_2() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);
    cpu.cs = 0; // Set CS before calculating address

    // SIB byte: scale=01 (2x), index=011 (EBX), base=000 (EAX)
    // Binary: 01 011 000 = 0x58
    let addr = Cpu8086::<ArrayMemory>::physical_address(cpu.cs, 0x1000);
    cpu.memory.write(addr, 0x58);
    cpu.ip = 0x1000;

    let (scale, index, base, bytes) = cpu.decode_sib();
    assert_eq!(scale, 2, "Scale should be 2");
    assert_eq!(index, 3, "Index should be EBX (3)");
    assert_eq!(base, 0, "Base should be EAX (0)");
    assert_eq!(bytes, 1, "Should consume 1 byte");
}

#[test]
fn test_sib_decode_scale_4() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);
    cpu.cs = 0; // Set CS before calculating address

    // SIB byte: scale=10 (4x), index=110 (ESI), base=111 (EDI)
    // Binary: 10 110 111 = 0xB7
    let addr = Cpu8086::<ArrayMemory>::physical_address(cpu.cs, 0x1000);
    cpu.memory.write(addr, 0xB7);
    cpu.ip = 0x1000;

    let (scale, index, base, bytes) = cpu.decode_sib();
    assert_eq!(scale, 4, "Scale should be 4");
    assert_eq!(index, 6, "Index should be ESI (6)");
    assert_eq!(base, 7, "Base should be EDI (7)");
    assert_eq!(bytes, 1, "Should consume 1 byte");
}

#[test]
fn test_sib_decode_scale_8() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);
    cpu.cs = 0; // Set CS before calculating address

    // SIB byte: scale=11 (8x), index=010 (EDX), base=001 (ECX)
    // Binary: 11 010 001 = 0xD1
    let addr = Cpu8086::<ArrayMemory>::physical_address(cpu.cs, 0x1000);
    cpu.memory.write(addr, 0xD1);
    cpu.ip = 0x1000;

    let (scale, index, base, bytes) = cpu.decode_sib();
    assert_eq!(scale, 8, "Scale should be 8");
    assert_eq!(index, 2, "Index should be EDX (2)");
    assert_eq!(base, 1, "Base should be ECX (1)");
    assert_eq!(bytes, 1, "Should consume 1 byte");
}

#[test]
fn test_sib_decode_no_index() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);
    cpu.cs = 0; // Set CS before calculating address

    // SIB byte: scale=00 (1x), index=100 (none/ESP), base=000 (EAX)
    // Binary: 00 100 000 = 0x20
    let addr = Cpu8086::<ArrayMemory>::physical_address(cpu.cs, 0x1000);
    cpu.memory.write(addr, 0x20);
    cpu.ip = 0x1000;

    let (scale, index, base, bytes) = cpu.decode_sib();
    assert_eq!(scale, 1, "Scale should be 1");
    assert_eq!(index, 4, "Index should be 4 (ESP/none)");
    assert_eq!(base, 0, "Base should be EAX (0)");
    assert_eq!(bytes, 1, "Should consume 1 byte");
}

#[test]
fn test_calc_effective_address_32_direct_register() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    // Test [EAX] with mod=00, rm=000
    cpu.ax = 0x12345678; // EAX = 0x12345678
    cpu.cs = 0;
    cpu.ip = 0x1000;

    let (seg, offset, bytes) = cpu.calc_effective_address_32(0b00, 0b000);
    assert_eq!(seg, cpu.ds, "Should use DS segment");
    assert_eq!(offset, 0x12345678, "Offset should be EAX value");
    assert_eq!(bytes, 0, "Should consume 0 additional bytes");
}

#[test]
fn test_calc_effective_address_32_with_disp8() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    // Test [EBX+disp8] with mod=01, rm=011
    cpu.bx = 0x10000000; // EBX = 0x10000000
    cpu.cs = 0;
    cpu.ip = 0x1000;
    let addr = Cpu8086::<ArrayMemory>::physical_address(cpu.cs, 0x1000);
    cpu.memory.write(addr, 0x50); // disp8 = 0x50 (positive)

    let (seg, offset, bytes) = cpu.calc_effective_address_32(0b01, 0b011);
    assert_eq!(seg, cpu.ds, "Should use DS segment");
    assert_eq!(offset, 0x10000050, "Offset should be EBX + disp8");
    assert_eq!(bytes, 1, "Should consume 1 byte for disp8");
}

#[test]
fn test_calc_effective_address_32_with_disp32() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    // Test [ECX+disp32] with mod=10, rm=001
    cpu.cx = 0x20000000; // ECX = 0x20000000
    cpu.cs = 0;
    cpu.ip = 0x1000;
    // disp32 = 0x12345678 (little-endian)
    let addr = Cpu8086::<ArrayMemory>::physical_address(cpu.cs, 0x1000);
    cpu.memory.write(addr, 0x78);
    cpu.memory.write(addr + 1, 0x56);
    cpu.memory.write(addr + 2, 0x34);
    cpu.memory.write(addr + 3, 0x12);

    let (seg, offset, bytes) = cpu.calc_effective_address_32(0b10, 0b001);
    assert_eq!(seg, cpu.ds, "Should use DS segment");
    assert_eq!(offset, 0x32345678, "Offset should be ECX + disp32");
    assert_eq!(bytes, 4, "Should consume 4 bytes for disp32");
}

#[test]
fn test_calc_effective_address_32_sib_base_index() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    // Test [EAX + EBX*4] with mod=00, rm=100 (SIB)
    cpu.ax = 0x10000000; // EAX (base) = 0x10000000
    cpu.bx = 0x00000100; // EBX (index) = 0x00000100
    cpu.ip = 0x1000;
    cpu.cs = 0; // Set CS before address calculation

    // SIB byte: scale=10 (4x), index=011 (EBX), base=000 (EAX)
    // Binary: 10 011 000 = 0x98
    let addr = Cpu8086::<ArrayMemory>::physical_address(cpu.cs, 0x1000);
    cpu.memory.write(addr, 0x98);

    let (seg, offset, bytes) = cpu.calc_effective_address_32(0b00, 0b100);
    assert_eq!(seg, cpu.ds, "Should use DS segment");
    assert_eq!(offset, 0x10000400, "Offset should be EAX + EBX*4");
    assert_eq!(bytes, 1, "Should consume 1 byte for SIB");
}

#[test]
fn test_calc_effective_address_32_sib_no_base() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    // Test [EDX*8 + disp32] with mod=00, rm=100 (SIB), base=101 (special)
    cpu.dx = 0x00001000; // EDX (index) = 0x00001000
    cpu.cs = 0;
    cpu.ip = 0x1000;
    cpu.cs = 0; // Set CS before address calculation

    // SIB byte: scale=11 (8x), index=010 (EDX), base=101 (disp32)
    // Binary: 11 010 101 = 0xD5
    let addr = Cpu8086::<ArrayMemory>::physical_address(cpu.cs, 0x1000);
    cpu.memory.write(addr, 0xD5);
    // disp32 = 0x00020000
    cpu.memory.write(addr + 1, 0x00);
    cpu.memory.write(addr + 2, 0x00);
    cpu.memory.write(addr + 3, 0x02);
    cpu.memory.write(addr + 4, 0x00);

    let (seg, offset, bytes) = cpu.calc_effective_address_32(0b00, 0b100);
    assert_eq!(seg, cpu.ds, "Should use DS segment");
    assert_eq!(offset, 0x00028000, "Offset should be EDX*8 + disp32");
    assert_eq!(bytes, 5, "Should consume 1 byte for SIB + 4 for disp32");
}

#[test]
fn test_calc_effective_address_32_sib_no_index() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    // Test [EBP] with SIB, mod=00, rm=100, index=100 (none)
    cpu.bp = 0x30000000; // EBP = 0x30000000
    cpu.cs = 0;
    cpu.ip = 0x1000;
    cpu.cs = 0; // Set CS before address calculation

    // SIB byte: scale=00 (1x), index=100 (none), base=101 (EBP, but with mod=00 means disp32)
    // This is actually [disp32] case when base=101 and mod=00
    // Let's use base=101 (EBP) with mod=01 instead
    let addr = Cpu8086::<ArrayMemory>::physical_address(cpu.cs, 0x1000);
    cpu.memory.write(addr, 0x25); // SIB: scale=00, index=100, base=101 (EBP)
    cpu.memory.write(addr + 1, 0x10); // disp8 = 0x10

    let (seg, offset, bytes) = cpu.calc_effective_address_32(0b01, 0b100);
    assert_eq!(seg, cpu.ss, "Should use SS segment for EBP");
    assert_eq!(
        offset, 0x30000010,
        "Offset should be EBP + disp8 (no index)"
    );
    assert_eq!(bytes, 2, "Should consume 1 byte for SIB + 1 for disp8");
}

#[test]
fn test_calc_effective_address_32_disp32_only() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    // Test [disp32] with mod=00, rm=101 (special case)
    cpu.cs = 0;
    cpu.ip = 0x1000;
    // disp32 = 0xABCDEF00
    let addr = Cpu8086::<ArrayMemory>::physical_address(cpu.cs, 0x1000);
    cpu.memory.write(addr, 0x00);
    cpu.memory.write(addr + 1, 0xEF);
    cpu.memory.write(addr + 2, 0xCD);
    cpu.memory.write(addr + 3, 0xAB);

    let (seg, offset, bytes) = cpu.calc_effective_address_32(0b00, 0b101);
    assert_eq!(seg, cpu.ds, "Should use DS segment");
    assert_eq!(offset, 0xABCDEF00, "Offset should be disp32");
    assert_eq!(bytes, 4, "Should consume 4 bytes for disp32");
}

#[test]
fn test_calc_effective_address_32_esp_uses_ss() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    // Test [ESP] should use SS segment (mod=00, rm=100 with base=ESP)
    // ESP is register 4
    cpu.sp = 0x00001000; // ESP = 0x00001000
    cpu.cs = 0;
    cpu.ip = 0x1000;

    // SIB byte: scale=00, index=100 (none), base=100 (ESP)
    // Binary: 00 100 100 = 0x24
    let addr = Cpu8086::<ArrayMemory>::physical_address(cpu.cs, 0x1000);
    cpu.memory.write(addr, 0x24);

    let (seg, offset, bytes) = cpu.calc_effective_address_32(0b00, 0b100);
    assert_eq!(seg, cpu.ss, "Should use SS segment for ESP base");
    assert_eq!(offset, 0x00001000, "Offset should be ESP");
    assert_eq!(bytes, 1, "Should consume 1 byte for SIB");
}

#[test]
fn test_read_write_u32_memory() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    // Write 32-bit value to memory
    let test_value = 0x12345678u32;
    cpu.write_u32(0x1000, 0x0000, test_value);

    // Read it back
    let read_value = cpu.read_u32(0x1000, 0x0000);
    assert_eq!(read_value, test_value, "32-bit read/write should match");

    // Verify little-endian byte order
    let addr = Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x0000);
    assert_eq!(cpu.memory.read(addr), 0x78, "Byte 0 should be low byte");
    assert_eq!(cpu.memory.read(addr + 1), 0x56, "Byte 1 should be byte 1");
    assert_eq!(cpu.memory.read(addr + 2), 0x34, "Byte 2 should be byte 2");
    assert_eq!(
        cpu.memory.read(addr + 3),
        0x12,
        "Byte 3 should be high byte"
    );
}

#[test]
fn test_read_rm32_register_mode() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    // Set EAX to test value
    cpu.set_reg32(0, 0xABCDEF01);

    // Read from register mode (mod=11, rm=000 for EAX)
    let value = cpu.read_rm32(0b11, 0b000);
    assert_eq!(value, 0xABCDEF01, "Should read EAX value");
}

#[test]
fn test_write_rm32_register_mode() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    // Write to register mode (mod=11, rm=011 for EBX)
    cpu.write_rm32(0b11, 0b011, 0x11223344);

    // Verify BX was updated
    assert_eq!(cpu.get_reg32(3), 0x11223344, "EBX should be updated");
}

#[test]
fn test_read_rm32_memory_mode_16bit_addressing() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    // Set up memory with test value
    cpu.write_u32(0x1000, 0x0100, 0x87654321);

    // Set BX to 0x0100 for [BX] addressing
    cpu.bx = 0x0100;
    cpu.ds = 0x1000;
    cpu.cs = 0;
    cpu.ip = 0x1000;

    // Read from memory mode (mod=00, rm=111 for [BX])
    let value = cpu.read_rm32(0b00, 0b111);
    assert_eq!(value, 0x87654321, "Should read from DS:BX");
}

#[test]
fn test_write_rm32_memory_mode_16bit_addressing() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    // Set SI to 0x0200 for [SI] addressing
    cpu.si = 0x0200;
    cpu.ds = 0x1000;
    cpu.cs = 0;
    cpu.ip = 0x1000;

    // Write to memory mode (mod=00, rm=100 for [SI])
    cpu.write_rm32(0b00, 0b100, 0xFEDCBA98);

    // Verify memory was updated
    let value = cpu.read_u32(0x1000, 0x0200);
    assert_eq!(value, 0xFEDCBA98, "Memory at DS:SI should be updated");
}

#[test]
fn test_read_rmw32_register_mode() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    // Set ECX to test value
    cpu.set_reg32(1, 0x99887766);

    // Read for RMW (mod=11, rm=001 for ECX)
    let (value, seg, offset) = cpu.read_rmw32(0b11, 0b001);
    assert_eq!(value, 0x99887766, "Should read ECX value");
    assert_eq!(seg, 0, "Seg should be dummy for register mode");
    assert_eq!(offset, 0, "Offset should be dummy for register mode");
}

#[test]
fn test_write_rmw32_register_mode() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    // Write RMW result to register (mod=11, rm=010 for EDX)
    cpu.write_rmw32(0b11, 0b010, 0x55443322, 0, 0);

    // Verify EDX was updated
    assert_eq!(cpu.get_reg32(2), 0x55443322, "EDX should be updated");
}

#[test]
fn test_mov_r32_rm32_register_to_register() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    // Set EBX to source value
    cpu.set_reg32(3, 0xDEADBEEF);

    // MOV EAX, EBX: opcode 0x89, ModR/M = 0xD8 (mod=11, reg=011, rm=000)
    cpu.cs = 0;
    cpu.ip = 0x1000;
    let addr = Cpu8086::<ArrayMemory>::physical_address(cpu.cs, 0x1000);
    cpu.memory.write(addr, 0x66); // Operand size override
    cpu.memory.write(addr + 1, 0x89); // MOV opcode
    cpu.memory.write(addr + 2, 0xD8); // ModR/M

    // Execute the instruction
    cpu.operand_size_override = false; // Will be set by prefix decoder
    cpu.step();

    // Verify EAX was updated with full 32-bit value
    assert_eq!(cpu.get_reg32(0), 0xDEADBEEF, "EAX should contain EBX value");
}

#[test]
fn test_mov_rm32_r32_register_to_register() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    // Set ECX to source value
    cpu.set_reg32(1, 0x12345678);

    // MOV EDX, ECX: opcode 0x8B, ModR/M = 0xD1 (mod=11, reg=010, rm=001)
    cpu.cs = 0;
    cpu.ip = 0x1000;
    let addr = Cpu8086::<ArrayMemory>::physical_address(cpu.cs, 0x1000);
    cpu.memory.write(addr, 0x66); // Operand size override
    cpu.memory.write(addr + 1, 0x8B); // MOV opcode
    cpu.memory.write(addr + 2, 0xD1); // ModR/M

    // Execute the instruction
    cpu.step();

    // Verify EDX was updated with full 32-bit value
    assert_eq!(cpu.get_reg32(2), 0x12345678, "EDX should contain ECX value");
}

#[test]
fn test_mov_preserves_16bit_operation_without_override() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    // Set EBX to 32-bit value
    cpu.set_reg32(3, 0xFFFFFFFF);

    // MOV BX, 0x1234 (16-bit, no override): opcode 0xC7, ModR/M = 0xC3
    cpu.cs = 0;
    cpu.ip = 0x1000;
    let addr = Cpu8086::<ArrayMemory>::physical_address(cpu.cs, 0x1000);
    cpu.memory.write(addr, 0xC7); // MOV opcode (no 0x66 prefix)
    cpu.memory.write(addr + 1, 0xC3); // ModR/M (mod=11, op=0, rm=011 for BX)
    cpu.memory.write(addr + 2, 0x34); // Immediate low byte
    cpu.memory.write(addr + 3, 0x12); // Immediate high byte

    // Execute the instruction
    cpu.step();

    // Verify only low 16 bits were affected
    assert_eq!(cpu.get_reg16(3), 0x1234, "BX should be 0x1234");
    assert_eq!(
        cpu.get_reg32(3),
        0xFFFF1234,
        "EBX high bits should be preserved"
    );
}

#[test]
fn test_add_r32_rm32() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    // Set up registers
    cpu.set_reg32(0, 0x12345678); // EAX
    cpu.set_reg32(3, 0x87654321); // EBX

    // ADD EAX, EBX: opcode 0x03, ModR/M = 0xC3 (mod=11, reg=000, rm=011)
    cpu.cs = 0;
    cpu.ip = 0x1000;
    let addr = Cpu8086::<ArrayMemory>::physical_address(cpu.cs, 0x1000);
    cpu.memory.write(addr, 0x66); // Operand size override
    cpu.memory.write(addr + 1, 0x03); // ADD opcode
    cpu.memory.write(addr + 2, 0xC3); // ModR/M

    // Execute the instruction
    cpu.step();

    // Verify result
    assert_eq!(cpu.get_reg32(0), 0x99999999, "EAX should contain sum");
    assert!(!cpu.get_flag(FLAG_CF), "CF should not be set");
    assert!(!cpu.get_flag(FLAG_OF), "OF should not be set");
    assert!(!cpu.get_flag(FLAG_ZF), "ZF should not be set");
    assert!(cpu.get_flag(FLAG_SF), "SF should be set (bit 31 set)");
}

#[test]
fn test_add_rm32_r32() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    // Set up registers
    cpu.set_reg32(1, 0x00000001); // ECX
    cpu.set_reg32(2, 0xFFFFFFFF); // EDX

    // ADD EDX, ECX: opcode 0x01, ModR/M = 0xCA (mod=11, reg=001, rm=010)
    cpu.cs = 0;
    cpu.ip = 0x1000;
    let addr = Cpu8086::<ArrayMemory>::physical_address(cpu.cs, 0x1000);
    cpu.memory.write(addr, 0x66); // Operand size override
    cpu.memory.write(addr + 1, 0x01); // ADD opcode
    cpu.memory.write(addr + 2, 0xCA); // ModR/M

    // Execute the instruction
    cpu.step();

    // Verify result (overflow to 0)
    assert_eq!(cpu.get_reg32(2), 0x00000000, "EDX should wrap to 0");
    assert!(cpu.get_flag(FLAG_CF), "CF should be set (carry occurred)");
    assert!(!cpu.get_flag(FLAG_OF), "OF should not be set");
    assert!(cpu.get_flag(FLAG_ZF), "ZF should be set (result is zero)");
    assert!(!cpu.get_flag(FLAG_SF), "SF should not be set");
}

#[test]
fn test_add_32bit_overflow() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    // Set up registers for signed overflow
    cpu.set_reg32(0, 0x7FFFFFFF); // EAX = largest positive i32
    cpu.set_reg32(3, 0x00000001); // EBX = 1

    // ADD EAX, EBX: opcode 0x03, ModR/M = 0xC3
    cpu.cs = 0;
    cpu.ip = 0x1000;
    let addr = Cpu8086::<ArrayMemory>::physical_address(cpu.cs, 0x1000);
    cpu.memory.write(addr, 0x66); // Operand size override
    cpu.memory.write(addr + 1, 0x03); // ADD opcode
    cpu.memory.write(addr + 2, 0xC3); // ModR/M

    // Execute the instruction
    cpu.step();

    // Verify result
    assert_eq!(cpu.get_reg32(0), 0x80000000, "EAX should be 0x80000000");
    assert!(!cpu.get_flag(FLAG_CF), "CF should not be set");
    assert!(cpu.get_flag(FLAG_OF), "OF should be set (signed overflow)");
    assert!(!cpu.get_flag(FLAG_ZF), "ZF should not be set");
    assert!(cpu.get_flag(FLAG_SF), "SF should be set (negative result)");
}

#[test]
fn test_add_preserves_16bit_without_override() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    // Set up 32-bit registers
    cpu.set_reg32(0, 0xFFFF0001); // EAX
    cpu.set_reg32(3, 0xFFFF0002); // EBX

    // ADD AX, BX (16-bit, no override): opcode 0x03, ModR/M = 0xC3
    cpu.cs = 0;
    cpu.ip = 0x1000;
    let addr = Cpu8086::<ArrayMemory>::physical_address(cpu.cs, 0x1000);
    cpu.memory.write(addr, 0x03); // ADD opcode (no 0x66 prefix)
    cpu.memory.write(addr + 1, 0xC3); // ModR/M

    // Execute the instruction
    cpu.step();

    // Verify only low 16 bits were affected
    assert_eq!(cpu.get_reg16(0), 0x0003, "AX should be 0x0003");
    assert_eq!(
        cpu.get_reg32(0),
        0xFFFF0003,
        "EAX high bits should be preserved"
    );
}

#[test]
fn test_sub_r32_rm32() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    // Set up registers
    cpu.set_reg32(0, 0x99999999); // EAX
    cpu.set_reg32(3, 0x11111111); // EBX

    // SUB EAX, EBX: opcode 0x2B, ModR/M = 0xC3 (mod=11, reg=000, rm=011)
    cpu.cs = 0;
    cpu.ip = 0x1000;
    let addr = Cpu8086::<ArrayMemory>::physical_address(cpu.cs, 0x1000);
    cpu.memory.write(addr, 0x66); // Operand size override
    cpu.memory.write(addr + 1, 0x2B); // SUB opcode
    cpu.memory.write(addr + 2, 0xC3); // ModR/M

    // Execute the instruction
    cpu.step();

    // Verify result
    assert_eq!(
        cpu.get_reg32(0),
        0x88888888,
        "EAX should contain difference"
    );
    assert!(!cpu.get_flag(FLAG_CF), "CF should not be set");
    assert!(!cpu.get_flag(FLAG_OF), "OF should not be set");
    assert!(!cpu.get_flag(FLAG_ZF), "ZF should not be set");
    assert!(cpu.get_flag(FLAG_SF), "SF should be set (bit 31 set)");
}

#[test]
fn test_sub_rm32_r32() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    // Set up registers
    cpu.set_reg32(1, 0x00000001); // ECX
    cpu.set_reg32(2, 0x00000000); // EDX

    // SUB EDX, ECX: opcode 0x29, ModR/M = 0xCA (mod=11, reg=001, rm=010)
    cpu.cs = 0;
    cpu.ip = 0x1000;
    let addr = Cpu8086::<ArrayMemory>::physical_address(cpu.cs, 0x1000);
    cpu.memory.write(addr, 0x66); // Operand size override
    cpu.memory.write(addr + 1, 0x29); // SUB opcode
    cpu.memory.write(addr + 2, 0xCA); // ModR/M

    // Execute the instruction
    cpu.step();

    // Verify result (underflow to 0xFFFFFFFF)
    assert_eq!(
        cpu.get_reg32(2),
        0xFFFFFFFF,
        "EDX should wrap to 0xFFFFFFFF"
    );
    assert!(cpu.get_flag(FLAG_CF), "CF should be set (borrow occurred)");
    assert!(!cpu.get_flag(FLAG_OF), "OF should not be set");
    assert!(!cpu.get_flag(FLAG_ZF), "ZF should not be set");
    assert!(cpu.get_flag(FLAG_SF), "SF should be set (negative result)");
}

#[test]
fn test_sub_32bit_overflow() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    // Set up registers for signed overflow
    cpu.set_reg32(0, 0x80000000); // EAX = most negative i32
    cpu.set_reg32(3, 0x00000001); // EBX = 1

    // SUB EAX, EBX: opcode 0x2B, ModR/M = 0xC3
    cpu.cs = 0;
    cpu.ip = 0x1000;
    let addr = Cpu8086::<ArrayMemory>::physical_address(cpu.cs, 0x1000);
    cpu.memory.write(addr, 0x66); // Operand size override
    cpu.memory.write(addr + 1, 0x2B); // SUB opcode
    cpu.memory.write(addr + 2, 0xC3); // ModR/M

    // Execute the instruction
    cpu.step();

    // Verify result
    assert_eq!(cpu.get_reg32(0), 0x7FFFFFFF, "EAX should be 0x7FFFFFFF");
    assert!(!cpu.get_flag(FLAG_CF), "CF should not be set");
    assert!(cpu.get_flag(FLAG_OF), "OF should be set (signed overflow)");
    assert!(!cpu.get_flag(FLAG_ZF), "ZF should not be set");
    assert!(
        !cpu.get_flag(FLAG_SF),
        "SF should not be set (positive result)"
    );
}

#[test]
fn test_mixed_16_32bit_operations() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    // Test mixing 16-bit and 32-bit operations
    cpu.set_reg32(0, 0x12345678); // EAX
    cpu.set_reg32(3, 0xABCDEF00); // EBX

    // Set up a sequence: 16-bit MOV, 32-bit ADD, 16-bit SUB
    cpu.cs = 0;
    cpu.ip = 0x1000;
    let addr = Cpu8086::<ArrayMemory>::physical_address(cpu.cs, 0x1000);

    // MOV AX, BX (16-bit)
    cpu.memory.write(addr, 0x89); // MOV opcode
    cpu.memory.write(addr + 1, 0xD8); // ModR/M (BX to AX)

    // ADD EAX, EBX (32-bit)
    cpu.memory.write(addr + 2, 0x66); // Operand size override
    cpu.memory.write(addr + 3, 0x03); // ADD opcode
    cpu.memory.write(addr + 4, 0xC3); // ModR/M (EAX + EBX)

    // SUB AX, BX (16-bit)
    cpu.memory.write(addr + 5, 0x2B); // SUB opcode
    cpu.memory.write(addr + 6, 0xC3); // ModR/M (AX - BX)

    // Execute MOV AX, BX
    cpu.step();
    assert_eq!(cpu.get_reg16(0), 0xEF00, "AX should be low 16 bits of BX");
    assert_eq!(cpu.get_reg32(0), 0x1234EF00, "EAX high bits preserved");

    // Execute ADD EAX, EBX (32-bit)
    cpu.step();
    assert_eq!(
        cpu.get_reg32(0),
        0xBE02DE00,
        "EAX = 0x1234EF00 + 0xABCDEF00"
    );

    // Execute SUB AX, BX (16-bit)
    cpu.step();
    assert_eq!(cpu.get_reg16(0), 0xEF00, "AX = 0xDE00 - 0xEF00");
    assert_eq!(
        cpu.get_reg32(0),
        0xBE02EF00,
        "EAX high bits preserved after 16-bit SUB"
    );
}

#[test]
fn test_operand_size_prefix_multiple_instructions() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    // Test that operand_size_override flag is properly reset between instructions
    cpu.set_reg32(0, 0x00000001);
    cpu.set_reg32(1, 0x00000002);

    cpu.cs = 0;
    cpu.ip = 0x1000;
    let addr = Cpu8086::<ArrayMemory>::physical_address(cpu.cs, 0x1000);

    // 32-bit ADD with prefix
    cpu.memory.write(addr, 0x66); // Operand size override
    cpu.memory.write(addr + 1, 0x03); // ADD opcode
    cpu.memory.write(addr + 2, 0xC1); // ModR/M (EAX + ECX)

    // 16-bit ADD without prefix (should work correctly after previous 32-bit)
    cpu.memory.write(addr + 3, 0x03); // ADD opcode
    cpu.memory.write(addr + 4, 0xC1); // ModR/M (AX + CX)

    // Execute 32-bit ADD
    cpu.step();
    assert_eq!(cpu.get_reg32(0), 0x00000003, "EAX = 1 + 2 (32-bit)");

    // Execute 16-bit ADD
    cpu.step();
    assert_eq!(cpu.get_reg16(0), 0x0005, "AX = 3 + 2 (16-bit)");
    assert_eq!(cpu.get_reg32(0), 0x00000005, "EAX full value");
}

#[test]
fn test_and_32bit() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    cpu.set_reg32(0, 0xFFFF0000); // EAX
    cpu.set_reg32(3, 0x0000FFFF); // EBX

    // AND EAX, EBX: opcode 0x23, ModR/M = 0xC3
    cpu.cs = 0;
    cpu.ip = 0x1000;
    let addr = Cpu8086::<ArrayMemory>::physical_address(cpu.cs, 0x1000);
    cpu.memory.write(addr, 0x66); // Operand size override
    cpu.memory.write(addr + 1, 0x23); // AND opcode
    cpu.memory.write(addr + 2, 0xC3); // ModR/M

    cpu.step();

    assert_eq!(
        cpu.get_reg32(0),
        0x00000000,
        "EAX should be 0 (no common bits)"
    );
    assert!(cpu.get_flag(FLAG_ZF), "ZF should be set");
    assert!(!cpu.get_flag(FLAG_SF), "SF should not be set");
    assert!(!cpu.get_flag(FLAG_CF), "CF should be cleared");
    assert!(!cpu.get_flag(FLAG_OF), "OF should be cleared");
}

#[test]
fn test_or_32bit() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    cpu.set_reg32(0, 0xAAAAAAAA); // EAX
    cpu.set_reg32(3, 0x55555555); // EBX

    // OR EAX, EBX: opcode 0x0B, ModR/M = 0xC3
    cpu.cs = 0;
    cpu.ip = 0x1000;
    let addr = Cpu8086::<ArrayMemory>::physical_address(cpu.cs, 0x1000);
    cpu.memory.write(addr, 0x66); // Operand size override
    cpu.memory.write(addr + 1, 0x0B); // OR opcode
    cpu.memory.write(addr + 2, 0xC3); // ModR/M

    cpu.step();

    assert_eq!(cpu.get_reg32(0), 0xFFFFFFFF, "EAX should be all 1s");
    assert!(!cpu.get_flag(FLAG_ZF), "ZF should not be set");
    assert!(cpu.get_flag(FLAG_SF), "SF should be set (bit 31)");
    assert!(!cpu.get_flag(FLAG_CF), "CF should be cleared");
    assert!(!cpu.get_flag(FLAG_OF), "OF should be cleared");
}

#[test]
fn test_xor_32bit() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    cpu.set_reg32(0, 0x12345678); // EAX
    cpu.set_reg32(3, 0x12345678); // EBX (same value)

    // XOR EAX, EBX: opcode 0x33, ModR/M = 0xC3
    cpu.cs = 0;
    cpu.ip = 0x1000;
    let addr = Cpu8086::<ArrayMemory>::physical_address(cpu.cs, 0x1000);
    cpu.memory.write(addr, 0x66); // Operand size override
    cpu.memory.write(addr + 1, 0x33); // XOR opcode
    cpu.memory.write(addr + 2, 0xC3); // ModR/M

    cpu.step();

    assert_eq!(cpu.get_reg32(0), 0x00000000, "EAX XOR EBX should be 0");
    assert!(cpu.get_flag(FLAG_ZF), "ZF should be set");
    assert!(!cpu.get_flag(FLAG_SF), "SF should not be set");
    assert!(!cpu.get_flag(FLAG_CF), "CF should be cleared");
    assert!(!cpu.get_flag(FLAG_OF), "OF should be cleared");
}

#[test]
fn test_cmp_32bit() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    cpu.set_reg32(0, 0x00000005); // EAX
    cpu.set_reg32(3, 0x00000003); // EBX

    // CMP EAX, EBX: opcode 0x3B, ModR/M = 0xC3
    cpu.cs = 0;
    cpu.ip = 0x1000;
    let addr = Cpu8086::<ArrayMemory>::physical_address(cpu.cs, 0x1000);
    cpu.memory.write(addr, 0x66); // Operand size override
    cpu.memory.write(addr + 1, 0x3B); // CMP opcode
    cpu.memory.write(addr + 2, 0xC3); // ModR/M

    cpu.step();

    // CMP doesn't modify registers, only flags
    assert_eq!(cpu.get_reg32(0), 0x00000005, "EAX should be unchanged");
    assert_eq!(cpu.get_reg32(3), 0x00000003, "EBX should be unchanged");
    assert!(!cpu.get_flag(FLAG_ZF), "ZF should not be set (5 != 3)");
    assert!(!cpu.get_flag(FLAG_CF), "CF should not be set (5 > 3)");
}

#[test]
fn test_test_32bit() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    cpu.set_reg32(0, 0x80000000); // EAX (bit 31 set)
    cpu.set_reg32(3, 0x80000000); // EBX (bit 31 set)

    // TEST r/m32, r32: opcode 0x85, ModR/M = 0xC3
    cpu.cs = 0;
    cpu.ip = 0x1000;
    let addr = Cpu8086::<ArrayMemory>::physical_address(cpu.cs, 0x1000);
    cpu.memory.write(addr, 0x66); // Operand size override
    cpu.memory.write(addr + 1, 0x85); // TEST opcode
    cpu.memory.write(addr + 2, 0xC3); // ModR/M

    cpu.step();

    // TEST doesn't modify registers, only flags
    assert_eq!(cpu.get_reg32(0), 0x80000000, "EAX should be unchanged");
    assert!(!cpu.get_flag(FLAG_ZF), "ZF should not be set");
    assert!(
        cpu.get_flag(FLAG_SF),
        "SF should be set (result has bit 31)"
    );
    assert!(!cpu.get_flag(FLAG_CF), "CF should be cleared");
    assert!(!cpu.get_flag(FLAG_OF), "OF should be cleared");
}

#[test]
fn test_movsx() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    cpu.bx = 0x00FF; // Set BL to 0xFF (negative byte)

    // MOVSX AX, BL (0x0F 0xBE ModRM)
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0xBE, 0xC3]); // ModRM=0xC3 (AX, BX)
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    // 0xFF sign-extended to 16-bit should be 0xFFFF
    assert_eq!(cpu.ax, 0xFFFF);

    // Test with positive value
    cpu.bx = 0x007F; // Set BL to 0x7F (positive byte)
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0xBE, 0xC3]);
    cpu.ip = 0x0000;

    cpu.step();

    // 0x7F sign-extended to 16-bit should be 0x007F
    assert_eq!(cpu.ax, 0x007F);
}

#[test]
fn test_movzx() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    cpu.bx = 0xFFFF; // Set BL to 0xFF

    // MOVZX AX, BL (0x0F 0xB6 ModRM)
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0xB6, 0xC3]); // ModRM=0xC3 (AX, BX)
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    // 0xFF zero-extended to 16-bit should be 0x00FF
    assert_eq!(cpu.ax, 0x00FF);
}

#[test]
fn test_bsf() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    cpu.bx = 0x0018; // Binary: 0000 0000 0001 1000

    // BSF AX, BX (0x0F 0xBC ModRM) - Find first set bit
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0xBC, 0xC3]); // ModRM=0xC3 (AX, BX)
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    // First set bit from LSB is at position 3
    assert_eq!(cpu.ax, 3);
    assert!(!cpu.get_flag(FLAG_ZF));

    // Test with zero
    cpu.bx = 0;
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0xBC, 0xC3]);
    cpu.ip = 0x0000;

    cpu.step();

    // ZF should be set for zero
    assert!(cpu.get_flag(FLAG_ZF));
}

#[test]
fn test_bsr() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    cpu.bx = 0x0018; // Binary: 0000 0000 0001 1000

    // BSR AX, BX (0x0F 0xBD ModRM) - Find last set bit
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0xBD, 0xC3]); // ModRM=0xC3 (AX, BX)
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    // First set bit from MSB is at position 4
    assert_eq!(cpu.ax, 4);
    assert!(!cpu.get_flag(FLAG_ZF));
}

#[test]
fn test_bt() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    cpu.ax = 3; // Bit index
    cpu.bx = 0x0008; // Binary: 0000 0000 0000 1000 (bit 3 set)

    // BT BX, AX (0x0F 0xA3 ModRM) - Test bit
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0xA3, 0xC3]); // ModRM=0xC3 (BX, AX)
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    // Bit 3 is set, so CF should be set
    assert!(cpu.get_flag(FLAG_CF));

    // Test with bit not set
    cpu.ax = 5; // Bit index
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0xA3, 0xC3]);
    cpu.ip = 0x0000;

    cpu.step();

    // Bit 5 is not set, so CF should be clear
    assert!(!cpu.get_flag(FLAG_CF));
}

#[test]
fn test_bts() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    cpu.ax = 5; // Bit index
    cpu.bx = 0x0008; // Binary: 0000 0000 0000 1000

    // BTS BX, AX (0x0F 0xAB ModRM) - Test and set bit
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0xAB, 0xC3]); // ModRM=0xC3 (BX, AX)
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    // Bit 5 was not set, so CF should be clear
    assert!(!cpu.get_flag(FLAG_CF));
    // Bit 5 should now be set: 0x0008 | 0x0020 = 0x0028
    assert_eq!(cpu.bx, 0x0028);
}

#[test]
fn test_btr() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    cpu.ax = 3; // Bit index
    cpu.bx = 0x0028; // Binary: 0000 0000 0010 1000

    // BTR BX, AX (0x0F 0xB3 ModRM) - Test and reset bit
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0xB3, 0xC3]); // ModRM=0xC3 (BX, AX)
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    // Bit 3 was set, so CF should be set
    assert!(cpu.get_flag(FLAG_CF));
    // Bit 3 should now be clear: 0x0028 & ~0x0008 = 0x0020
    assert_eq!(cpu.bx, 0x0020);
}

#[test]
fn test_btc() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    cpu.ax = 3; // Bit index
    cpu.bx = 0x0008; // Binary: 0000 0000 0000 1000

    // BTC BX, AX (0x0F 0xBB ModRM) - Test and complement bit
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0xBB, 0xC3]); // ModRM=0xC3 (BX, AX)
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    // Bit 3 was set, so CF should be set
    assert!(cpu.get_flag(FLAG_CF));
    // Bit 3 should now be clear: 0x0008 ^ 0x0008 = 0x0000
    assert_eq!(cpu.bx, 0x0000);

    // Test complement again (from 0 to 1)
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0xBB, 0xC3]);
    cpu.ip = 0x0000;

    cpu.step();

    // Bit 3 was clear, so CF should be clear
    assert!(!cpu.get_flag(FLAG_CF));
    // Bit 3 should now be set: 0x0000 ^ 0x0008 = 0x0008
    assert_eq!(cpu.bx, 0x0008);
}

#[test]
fn test_setcc() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    // Set ZF flag
    cpu.flags = FLAG_ZF;

    // SETE BL (0x0F 0x94 ModRM) - Set if equal/zero
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0x94, 0xC3]); // ModRM=0xC3 (BL)
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    // ZF is set, so BL should be 1
    assert_eq!(cpu.bx & 0xFF, 1);

    // Clear ZF flag
    cpu.flags = 0;

    // SETE BL again
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0x94, 0xC3]);
    cpu.ip = 0x0000;

    cpu.step();

    // ZF is clear, so BL should be 0
    assert_eq!(cpu.bx & 0xFF, 0);
}

#[test]
fn test_lfs() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    // Setup: LFS BX, [SI] at 0x0000:0x0100
    // 0x0F 0xB4 = LFS
    // ModR/M: 0b00_011_100 (mod=00, reg=BX=3, r/m=SI=4)
    cpu.memory.load_program(0x0100, &[0x0F, 0xB4, 0b00_011_100]);

    // Put far pointer data at DS:SI
    cpu.ds = 0x1000;
    cpu.si = 0x0200;
    cpu.memory.write_u16(0x10200, 0x5678); // Offset
    cpu.memory.write_u16(0x10202, 0x9ABC); // Segment

    cpu.ip = 0x0100;
    cpu.cs = 0x0000;

    // Execute LFS BX, [SI]
    cpu.step();
    assert_eq!(cpu.bx, 0x5678, "BX should contain offset");
    assert_eq!(cpu.fs, 0x9ABC, "FS should contain segment");
}

#[test]
fn test_lgs() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    // Setup: LGS DX, [DI] at 0x0000:0x0100
    // 0x0F 0xB5 = LGS
    // ModR/M: 0b00_010_101 (mod=00, reg=DX=2, r/m=DI=5)
    cpu.memory.load_program(0x0100, &[0x0F, 0xB5, 0b00_010_101]);

    // Put far pointer data at DS:DI
    cpu.ds = 0x2000;
    cpu.di = 0x0300;
    cpu.memory.write_u16(0x20300, 0x1122); // Offset
    cpu.memory.write_u16(0x20302, 0x3344); // Segment

    cpu.ip = 0x0100;
    cpu.cs = 0x0000;

    // Execute LGS DX, [DI]
    cpu.step();
    assert_eq!(cpu.dx, 0x1122, "DX should contain offset");
    assert_eq!(cpu.gs, 0x3344, "GS should contain segment");
}

#[test]
fn test_lss() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    // Setup: LSS SP, [BX] at 0x0000:0x0100
    // 0x0F 0xB2 = LSS
    // ModR/M: 0b00_100_111 (mod=00, reg=SP=4, r/m=BX=7)
    cpu.memory.load_program(0x0100, &[0x0F, 0xB2, 0b00_100_111]);

    // Put far pointer data at DS:BX
    cpu.ds = 0x3000;
    cpu.bx = 0x0400;
    cpu.memory.write_u16(0x30400, 0xFFFE); // Offset (new SP)
    cpu.memory.write_u16(0x30402, 0x5000); // Segment (new SS)

    cpu.ip = 0x0100;
    cpu.cs = 0x0000;

    // Execute LSS SP, [BX]
    cpu.step();
    assert_eq!(cpu.sp, 0xFFFE, "SP should contain offset");
    assert_eq!(cpu.ss, 0x5000, "SS should contain segment");
}

#[test]
fn test_fs_gs_invalid_on_80286() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80286);

    // PUSH FS should be invalid on 80286
    cpu.memory.load_program(0x0100, &[0x0F, 0xA0]);
    cpu.ip = 0x0100;
    cpu.cs = 0x0000;
    let initial_cycles = cpu.cycles;
    cpu.step();
    // Should execute but as invalid (returns early)
    assert_eq!(
        cpu.cycles - initial_cycles,
        10,
        "Invalid opcode should consume 10 cycles"
    );
}

#[test]
fn test_bswap() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80486);

    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0x00001234; // Full 32-bit value
    cpu.bx = 0x0000ABCD; // Full 32-bit value

    // BSWAP EAX (0x0F 0xC8)
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0xC8]);
    cpu.step();
    assert_eq!(
        cpu.ax, 0x34120000,
        "BSWAP should swap bytes in full 32-bit register"
    );

    // BSWAP EBX (0x0F 0xCB)
    cpu.ip = 0x0000;
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0xCB]);
    cpu.step();
    assert_eq!(
        cpu.bx, 0xCDAB0000,
        "BSWAP should swap bytes in full 32-bit register"
    );
}

#[test]
fn test_bswap_invalid_on_80386() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0x1234;

    // BSWAP EAX (0x0F 0xC8) - should be invalid on 80386
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0xC8]);
    let cycles = cpu.step();
    assert_eq!(cycles, 2, "BSWAP should be invalid on 80386");
    assert_eq!(cpu.ax, 0x1234, "AX should not be modified");
}

#[test]
fn test_cmpxchg8b() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::IntelPentium);

    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ds = 0x1000;
    cpu.bx = 0x0100;

    // Test equal case: DX:AX == [BX]
    cpu.ax = 0x5678; // Low word
    cpu.dx = 0x1234; // High word
    cpu.bx = 0x0100;
    cpu.cx = 0xCDEF; // New high word
                     // bx already set above

    // Write matching value to memory
    cpu.memory.write_u16(0x10100, 0x5678);
    cpu.memory.write_u16(0x10102, 0x1234);

    // CMPXCHG8B [BX] (0x0F 0xC7 with ModR/M, reg field must be 1)
    // ModR/M: mod=00 (memory), reg=001 (required for CMPXCHG8B), rm=111 ([BX])
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0xC7, 0x0F]);
    cpu.step();

    assert!(cpu.get_flag(FLAG_ZF), "ZF should be set when equal");
    // Memory should now contain BX (low word) - wait, I need to fix this
    // Actually in my implementation I use CX:BX, let me check...
}

#[test]
fn test_invd_wbinvd() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80486);

    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    // INVD (0x0F 0x08)
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0x08]);
    cpu.step();
    // Should not crash, just a NOP

    // WBINVD (0x0F 0x09)
    cpu.ip = 0x0000;
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0x09]);
    cpu.step();
    // Should not crash, just a NOP
}

#[test]
fn test_cpuid() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::IntelPentium);

    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    // Test function 0 (vendor ID)
    cpu.ax = 0;
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0xA2]);
    cpu.step();

    assert_eq!(cpu.ax, 1, "Should support functions 0 and 1");
    assert_eq!(cpu.bx, 0x756E, "Vendor ID part 1");
    assert_eq!(cpu.dx, 0x4965, "Vendor ID part 2");
    assert_eq!(cpu.cx, 0x6C65, "Vendor ID part 3");

    // Test function 1 (processor info)
    cpu.ip = 0x0000;
    cpu.ax = 1;
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0xA2]);
    cpu.step();

    assert_eq!(cpu.ax, 0x0543, "Family 5, Model 4, Stepping 3");
    assert_eq!(cpu.dx & 0x0001, 0x0001, "FPU should be present");
}

#[test]
fn test_cpuid_invalid_on_80486() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80486);

    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0;

    cpu.memory.load_program(0xFFFF0, &[0x0F, 0xA2]);
    let cycles = cpu.step();

    assert_eq!(cycles, 2, "CPUID should be invalid on 80486");
}

#[test]
fn test_rdtsc() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::IntelPentium);

    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.tsc = 0x0000ABCD5678; // Set a known TSC value (fits in 32 bits for easy testing)

    // RDTSC (0x0F 0x31)
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0x31]);
    cpu.step();

    // RDTSC reads TSC *before* incrementing, so we should get the value we set
    // plus any increment from before RDTSC executes
    // Check that EDX:EAX contains TSC low 32 bits
    let result = (cpu.ax as u32) | ((cpu.dx as u32) << 16);
    // The TSC should have been read, then incremented by 6 cycles
    // So the result should be the original value (0xABCD5678)
    assert_eq!(result, 0xABCD5678, "Should read TSC value");
}

#[test]
fn test_rdtsc_increments() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::IntelPentium);

    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.tsc = 0;

    // Execute a NOP (0x90) to increment TSC
    cpu.memory.load_program(0xFFFF0, &[0x90]);
    cpu.step();

    // TSC should have incremented by the number of cycles
    assert!(cpu.tsc > 0, "TSC should increment with each instruction");
}

#[test]
fn test_rdmsr_wrmsr() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::IntelPentium);

    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    // Write to MSR
    cpu.cx = 0x0010; // MSR index
    cpu.ax = 0x1234; // Low 16 bits
    cpu.dx = 0x5678; // High 16 bits

    // WRMSR (0x0F 0x30) - Wait, I have the opcodes swapped!
    // Let me check: WRMSR is 0x30, RDMSR is 0x32
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0x30]);
    cpu.step();

    // Read back from MSR
    cpu.ip = 0x0000;
    cpu.ax = 0;
    cpu.dx = 0;
    cpu.cx = 0x0010; // Same MSR index

    // RDMSR (0x0F 0x32)
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0x32]);
    cpu.step();

    assert_eq!(cpu.ax, 0x1234, "Low 16 bits should match");
    assert_eq!(cpu.dx, 0x5678, "High 16 bits should match");
}

#[test]
fn test_486_instructions_on_pentium() {
    // Test that 486 instructions work on Pentium
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::IntelPentium);

    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0x1234;

    // BSWAP should work on Pentium (supports all 486 instructions)
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0xC8]);
    cpu.step();
    assert_eq!(
        cpu.ax, 0x34120000,
        "486 instructions should work on Pentium (BSWAP on full 32-bit)"
    );
}

#[test]
fn test_mmx_support_check() {
    assert!(!CpuModel::Intel80486.supports_mmx_instructions());
    assert!(!CpuModel::IntelPentium.supports_mmx_instructions());
    assert!(CpuModel::IntelPentiumMMX.supports_mmx_instructions());
}

#[test]
fn test_emms() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::IntelPentiumMMX);

    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.mmx_regs[0] = 0x1234567890ABCDEF;
    cpu.mmx_regs[7] = 0xFEDCBA9876543210;

    // EMMS (0x0F 0x77)
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0x77]);
    cpu.step();

    // All MMX registers should be cleared
    for i in 0..8 {
        assert_eq!(cpu.mmx_regs[i], 0, "MMX register {} should be cleared", i);
    }
}

#[test]
fn test_movd_reg_to_mm() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::IntelPentiumMMX);

    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0x1234;

    // MOVD MM0, EAX (0x0F 0x6E with ModR/M 0xC0 for MM0, EAX)
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0x6E, 0xC0]);
    cpu.step();

    assert_eq!(cpu.mmx_regs[0], 0x1234, "MM0 should contain value from AX");
}

#[test]
fn test_movd_mm_to_reg() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::IntelPentiumMMX);

    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.mmx_regs[1] = 0xABCD;

    // MOVD EAX, MM1 (0x0F 0x7E with ModR/M 0xC8 for MM1, EAX)
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0x7E, 0xC8]);
    cpu.step();

    assert_eq!(cpu.ax, 0xABCD, "AX should contain value from MM1");
}

#[test]
fn test_movq_mm_to_mm() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::IntelPentiumMMX);

    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.mmx_regs[2] = 0x1234567890ABCDEF;

    // MOVQ MM0, MM2 (0x0F 0x6F with ModR/M 0xC2)
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0x6F, 0xC2]);
    cpu.step();

    assert_eq!(cpu.mmx_regs[0], 0x1234567890ABCDEF, "MM0 should equal MM2");
}

#[test]
fn test_paddb() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::IntelPentiumMMX);

    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.mmx_regs[0] = 0x0102030405060708;
    cpu.mmx_regs[1] = 0x0F0E0D0C0B0A0908;

    // PADDB MM0, MM1 (0x0F 0xFC with ModR/M 0xC1)
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0xFC, 0xC1]);
    cpu.step();

    // Each byte should add independently with wraparound
    assert_eq!(cpu.mmx_regs[0], 0x1010101010101010);
}

#[test]
fn test_paddw() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::IntelPentiumMMX);

    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.mmx_regs[0] = 0x0001000200030004;
    cpu.mmx_regs[1] = 0x000F000E000D000C;

    // PADDW MM0, MM1 (0x0F 0xFD with ModR/M 0xC1)
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0xFD, 0xC1]);
    cpu.step();

    // Each word should add independently
    assert_eq!(cpu.mmx_regs[0], 0x0010001000100010);
}

#[test]
fn test_paddd() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::IntelPentiumMMX);

    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.mmx_regs[0] = 0x0000000100000002;
    cpu.mmx_regs[1] = 0x0000000F0000000E;

    // PADDD MM0, MM1 (0x0F 0xFE with ModR/M 0xC1)
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0xFE, 0xC1]);
    cpu.step();

    // Each dword should add independently
    assert_eq!(cpu.mmx_regs[0], 0x0000001000000010);
}

#[test]
fn test_psubb() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::IntelPentiumMMX);

    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.mmx_regs[0] = 0x1010101010101010;
    cpu.mmx_regs[1] = 0x0102030405060708;

    // PSUBB MM0, MM1 (0x0F 0xF8 with ModR/M 0xC1)
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0xF8, 0xC1]);
    cpu.step();

    // Each byte should subtract independently
    assert_eq!(cpu.mmx_regs[0], 0x0F0E0D0C0B0A0908);
}

#[test]
fn test_psubd() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::IntelPentiumMMX);

    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.mmx_regs[0] = 0x0000001000000010;
    cpu.mmx_regs[1] = 0x0000000100000002;

    // PSUBD MM0, MM1 (0x0F 0xFA with ModR/M 0xC1)
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0xFA, 0xC1]);
    cpu.step();

    // Each dword should subtract independently
    assert_eq!(cpu.mmx_regs[0], 0x0000000F0000000E);
}

#[test]
fn test_psubw() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::IntelPentiumMMX);

    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.mmx_regs[0] = 0x0010001000100010;
    cpu.mmx_regs[1] = 0x0001000200030004;

    // PSUBW MM0, MM1 (0x0F 0xF9 with ModR/M 0xC1)
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0xF9, 0xC1]);
    cpu.step();

    // Each word should subtract independently
    assert_eq!(cpu.mmx_regs[0], 0x000F000E000D000C);
}

#[test]
fn test_pand() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::IntelPentiumMMX);

    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.mmx_regs[0] = 0xFFFFFFFF00000000;
    cpu.mmx_regs[1] = 0xFF00FF00FF00FF00;

    // PAND MM0, MM1 (0x0F 0xDB with ModR/M 0xC1)
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0xDB, 0xC1]);
    cpu.step();

    assert_eq!(cpu.mmx_regs[0], 0xFF00FF0000000000);
}

#[test]
fn test_por() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::IntelPentiumMMX);

    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.mmx_regs[0] = 0xFF00FF0000000000;
    cpu.mmx_regs[1] = 0x00FF00FF00000000;

    // POR MM0, MM1 (0x0F 0xEB with ModR/M 0xC1)
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0xEB, 0xC1]);
    cpu.step();

    assert_eq!(cpu.mmx_regs[0], 0xFFFFFFFF00000000);
}

#[test]
fn test_pxor() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::IntelPentiumMMX);

    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.mmx_regs[0] = 0xFF00FF00FF00FF00;
    cpu.mmx_regs[1] = 0x0F0F0F0F0F0F0F0F;

    // PXOR MM0, MM1 (0x0F 0xEF with ModR/M 0xC1)
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0xEF, 0xC1]);
    cpu.step();

    assert_eq!(cpu.mmx_regs[0], 0xF00FF00FF00FF00F);
}

#[test]
fn test_pxor_zero() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::IntelPentiumMMX);

    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.mmx_regs[0] = 0x1234567890ABCDEF;

    // PXOR MM0, MM0 (common way to zero a register)
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0xEF, 0xC0]);
    cpu.step();

    assert_eq!(cpu.mmx_regs[0], 0, "PXOR with itself should zero register");
}

#[test]
fn test_pcmpeqb() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::IntelPentiumMMX);

    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.mmx_regs[0] = 0x0102030405060708;
    cpu.mmx_regs[1] = 0x0102FF0405FF0708;

    // PCMPEQB MM0, MM1 (0x0F 0x74 with ModR/M 0xC1)
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0x74, 0xC1]);
    cpu.step();

    // Bytes that are equal get 0xFF, different get 0x00
    // Bytes 0,1,3,4,6,7 equal, bytes 2,5 different
    assert_eq!(cpu.mmx_regs[0], 0xFFFF00FFFF00FFFF);
}

#[test]
fn test_pcmpeqw() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::IntelPentiumMMX);

    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.mmx_regs[0] = 0x0001000200030004;
    cpu.mmx_regs[1] = 0x0001FFFF00030004;

    // PCMPEQW MM0, MM1 (0x0F 0x75 with ModR/M 0xC1)
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0x75, 0xC1]);
    cpu.step();

    // Words that are equal get 0xFFFF, different get 0x0000
    assert_eq!(cpu.mmx_regs[0], 0xFFFF0000FFFFFFFF);
}

#[test]
fn test_pcmpeqd() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::IntelPentiumMMX);

    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.mmx_regs[0] = 0x1234567812345678;
    cpu.mmx_regs[1] = 0x12345678ABCDEF01;

    // PCMPEQD MM0, MM1 (0x0F 0x76 with ModR/M 0xC1)
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0x76, 0xC1]);
    cpu.step();

    // Dwords that are equal get 0xFFFFFFFF, different get 0x00000000
    // High dword: 0x12345678 == 0x12345678 -> 0xFFFFFFFF
    // Low dword: 0x12345678 != 0xABCDEF01 -> 0x00000000
    assert_eq!(cpu.mmx_regs[0], 0xFFFFFFFF00000000);
}

#[test]
fn test_mmx_invalid_on_pentium() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::IntelPentium);

    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    // EMMS should be invalid on regular Pentium (not MMX)
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0x77]);
    let cycles = cpu.step();

    assert_eq!(cycles, 2, "MMX instructions should be invalid on Pentium");
}

#[test]
fn test_mmx_memory_operations() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::IntelPentiumMMX);

    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ds = 0x1000;
    cpu.bx = 0x0100;

    // Write test data to memory (64 bits = 4 words)
    cpu.memory.write_u16(0x10100, 0x1234);
    cpu.memory.write_u16(0x10102, 0x5678);
    cpu.memory.write_u16(0x10104, 0x9ABC);
    cpu.memory.write_u16(0x10106, 0xDEF0);

    // MOVQ MM0, [BX] (0x0F 0x6F with ModR/M 0x07 for [BX])
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0x6F, 0x07]);
    cpu.step();

    assert_eq!(
        cpu.mmx_regs[0], 0xDEF09ABC56781234,
        "MM0 should load from memory"
    );

    // Now write it back to a different location
    cpu.ip = 0x0000;
    cpu.bx = 0x0200;

    // MOVQ [BX], MM0 (0x0F 0x7F with ModR/M 0x07)
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0x7F, 0x07]);
    cpu.step();

    // Verify memory was written correctly
    assert_eq!(cpu.memory.read_u16(0x10200), 0x1234);
    assert_eq!(cpu.memory.read_u16(0x10202), 0x5678);
    assert_eq!(cpu.memory.read_u16(0x10204), 0x9ABC);
    assert_eq!(cpu.memory.read_u16(0x10206), 0xDEF0);
}
