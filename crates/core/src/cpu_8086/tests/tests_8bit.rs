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

#[test]
fn test_mov_immediate_8bit() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // MOV AL, 0x42
    cpu.memory.load_program(0xFFFF0, &[0xB0, 0x42]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    let cycles = cpu.step();
    assert_eq!(cycles, 4);
    assert_eq!(cpu.ax & 0xFF, 0x42);
    assert_eq!((cpu.ax >> 8) & 0xFF, 0);
}

#[test]
fn test_add_r8_rm8_register() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // ADD AL, CL (0x02 with ModR/M 0b11_000_001)
    cpu.memory.load_program(0xFFFF0, &[0x02, 0b11_000_001]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0x0010; // AL = 16
    cpu.cx = 0x0020; // CL = 32

    cpu.step();
    assert_eq!(cpu.ax & 0xFF, 48); // AL should be 16 + 32 = 48
}

#[test]
fn test_cmp_rm8_r8_sets_zero_flag() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // CMP CL, AL (0x38 with ModR/M 0b11_000_001)
    cpu.memory.load_program(0xFFFF0, &[0x38, 0b11_000_001]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0x0042; // AL = 0x42
    cpu.cx = 0x0042; // CL = 0x42

    let old_cx = cpu.cx;
    cpu.step();
    assert_eq!(cpu.cx, old_cx); // CMP doesn't modify operand
    assert!(cpu.get_flag(FLAG_ZF)); // Should set zero flag when equal
}

#[test]
fn test_not_memory() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.ds = 0x1000;
    cpu.bx = 0x0100;

    // Write value to memory
    let addr = Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x0100);
    cpu.memory.write(addr, 0xF0);

    // NOT byte ptr [BX] (0xF6 with ModR/M 0b00_010_111)
    cpu.memory.load_program(0xFFFF0, &[0xF6, 0b00_010_111]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    // Memory should contain ~0xF0 = 0x0F
    assert_eq!(cpu.memory.read(addr), 0x0F);
}

#[test]
fn test_mul_8bit() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // MUL CL (0xF6 with ModR/M 0b11_100_001)
    cpu.memory.load_program(0xFFFF0, &[0xF6, 0b11_100_001]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0x0005; // AL = 5
    cpu.cx = 0x0006; // CL = 6

    cpu.step();
    assert_eq!(cpu.ax, 30); // 5 * 6 = 30
    assert!(!cpu.get_flag(FLAG_CF)); // High byte is zero
    assert!(!cpu.get_flag(FLAG_OF));
}

#[test]
fn test_mul_8bit_overflow() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // MUL CL
    cpu.memory.load_program(0xFFFF0, &[0xF6, 0b11_100_001]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0x0080; // AL = 128
    cpu.cx = 0x0002; // CL = 2

    cpu.step();
    assert_eq!(cpu.ax, 256); // 128 * 2 = 256 (0x0100)
    assert!(cpu.get_flag(FLAG_CF)); // High byte is non-zero
    assert!(cpu.get_flag(FLAG_OF));
}

#[test]
fn test_div_8bit() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // DIV CL (0xF6 with ModR/M 0b11_110_001)
    cpu.memory.load_program(0xFFFF0, &[0xF6, 0b11_110_001]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 100; // Dividend
    cpu.cx = 7; // CL = divisor

    cpu.step();
    // 100 / 7 = 14 remainder 2
    // AL = quotient, AH = remainder
    assert_eq!(cpu.ax & 0xFF, 14); // AL = quotient
    assert_eq!((cpu.ax >> 8) & 0xFF, 2); // AH = remainder
}

#[test]
fn test_idiv_8bit() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // IDIV CL (0xF6 with ModR/M 0b11_111_001)
    cpu.memory.load_program(0xFFFF0, &[0xF6, 0b11_111_001]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = ((-50i16) as u16) as u32; // -50 as signed dividend
    cpu.cx = 0x0007; // CL = 7

    cpu.step();
    // -50 / 7 = -7 remainder -1
    assert_eq!((cpu.ax & 0xFF) as i8, -7); // AL = quotient
    assert_eq!(((cpu.ax >> 8) & 0xFF) as i8, -1); // AH = remainder
}

#[test]
fn test_imul_8bit() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // IMUL CL (0xF6 with ModR/M 0b11_101_001)
    cpu.memory.load_program(0xFFFF0, &[0xF6, 0b11_101_001]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0x00FB; // AL = -5 (signed)
    cpu.cx = 0x0006; // CL = 6

    cpu.step();
    // -5 * 6 = -30 = 0xFFE2 in 16-bit two's complement
    assert_eq!(cpu.ax & 0xFFFF, 0xFFE2);
}

#[test]
fn test_shl_8bit_by_1() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // SHL AL, 1 (0xD0 with ModR/M 0b11_100_000)
    cpu.memory.load_program(0xFFFF0, &[0xD0, 0b11_100_000]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0x0042; // AL = 0x42

    cpu.step();
    assert_eq!(cpu.ax & 0xFF, 0x84); // 0x42 << 1 = 0x84
    assert!(!cpu.get_flag(FLAG_CF)); // No bit shifted out
}

#[test]
fn test_sar_8bit() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // SAR AL, 1 (0xD0 with ModR/M 0b11_111_000)
    cpu.memory.load_program(0xFFFF0, &[0xD0, 0b11_111_000]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0x0084; // AL = 0x84 (negative in signed)

    cpu.step();
    assert_eq!(cpu.ax & 0xFF, 0xC2); // Sign bit preserved: 0x84 >> 1 = 0xC2
    assert!(!cpu.get_flag(FLAG_CF));
}

#[test]
fn test_rol_8bit() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // ROL AL, 1 (0xD0 with ModR/M 0b11_000_000)
    cpu.memory.load_program(0xFFFF0, &[0xD0, 0b11_000_000]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0x0081; // AL = 0x81

    cpu.step();
    assert_eq!(cpu.ax & 0xFF, 0x03); // 0x81 rotated left = 0x03
    assert!(cpu.get_flag(FLAG_CF)); // Bit 7 rotated into CF
}

#[test]
fn test_ror_8bit() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // ROR AL, 1 (0xD0 with ModR/M 0b11_001_000)
    cpu.memory.load_program(0xFFFF0, &[0xD0, 0b11_001_000]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0x0081; // AL = 0x81

    cpu.step();
    assert_eq!(cpu.ax & 0xFF, 0xC0); // 0x81 rotated right = 0xC0
    assert!(cpu.get_flag(FLAG_CF)); // Bit 0 rotated into CF
}

#[test]
fn test_rcl_8bit() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // RCL AL, 1 (0xD0 with ModR/M 0b11_010_000)
    cpu.memory.load_program(0xFFFF0, &[0xD0, 0b11_010_000]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0x0081; // AL = 0x81
    cpu.set_flag(FLAG_CF, true); // CF = 1

    cpu.step();
    assert_eq!(cpu.ax & 0xFF, 0x03); // 0x81 << 1 with CF=1 becomes 0x03
    assert!(cpu.get_flag(FLAG_CF)); // Old bit 7 moved to CF
}

#[test]
fn test_rcr_8bit() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // RCR AL, 1 (0xD0 with ModR/M 0b11_011_000)
    cpu.memory.load_program(0xFFFF0, &[0xD0, 0b11_011_000]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0x0081; // AL = 0x81
    cpu.set_flag(FLAG_CF, true); // CF = 1

    cpu.step();
    assert_eq!(cpu.ax & 0xFF, 0xC0); // 0x81 >> 1 with CF=1 becomes 0xC0
    assert!(cpu.get_flag(FLAG_CF)); // Old bit 0 moved to CF
}

#[test]
fn test_read_write_rm8_register() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // Set AL to 0x42
    cpu.ax = 0x0042;

    // Read AL using ModR/M (mod=11, rm=000 for AL)
    let val = cpu.read_rm8(0b11, 0b000);
    assert_eq!(val, 0x42);

    // Write to CL (mod=11, rm=001 for CL)
    cpu.write_rm8(0b11, 0b001, 0x55);
    assert_eq!(cpu.cx & 0xFF, 0x55);
}

#[test]
fn test_read_write_rm8_memory() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.ds = 0x1000;
    cpu.bx = 0x0100;

    // Write to memory using ModR/M (mod=00, rm=111 for [BX])
    cpu.write_rm8(0b00, 0b111, 0xAA);

    // Read it back
    let val = cpu.read_rm8(0b00, 0b111);
    assert_eq!(val, 0xAA);

    // Verify it's at the right physical address
    let physical_addr = Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x0100);
    assert_eq!(cpu.memory.read(physical_addr), 0xAA);
}

#[test]
fn test_cmpxchg8() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80486);

    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ds = 0x1000;
    cpu.bx = 0x0100;

    // Test equal case: AL == [BX]
    cpu.ax = 0x0042; // AL = 0x42
    cpu.cx = 0x0099; // CL = 0x99
    cpu.memory.write(0x10100, 0x42); // Memory = 0x42

    // CMPXCHG [BX], CL (0x0F 0xB0 with ModR/M 0x0F for [BX], CL)
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0xB0, 0x0F]);
    cpu.step();

    assert!(cpu.get_flag(FLAG_ZF), "ZF should be set when equal");
    assert_eq!(
        cpu.memory.read(0x10100),
        0x99,
        "Memory should be updated with CL"
    );

    // Test not equal case: AL != [BX]
    cpu.ip = 0x0000;
    cpu.ax = 0x0042; // AL = 0x42
    cpu.cx = 0x0099; // CL = 0x99
    cpu.memory.write(0x10100, 0x55); // Memory = 0x55

    cpu.memory.load_program(0xFFFF0, &[0x0F, 0xB0, 0x0F]);
    cpu.step();

    assert!(!cpu.get_flag(FLAG_ZF), "ZF should be clear when not equal");
    assert_eq!(cpu.ax & 0xFF, 0x55, "AL should be loaded from memory");
    assert_eq!(cpu.memory.read(0x10100), 0x55, "Memory should not change");
}

#[test]
fn test_xadd8() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80486);

    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ds = 0x1000;
    cpu.bx = 0x0100;

    cpu.ax = 0x0005; // AL = 5
    cpu.cx = 0x0003; // CL = 3
    cpu.memory.write(0x10100, 0x0A); // Memory = 10

    // XADD [BX], CL (0x0F 0xC0 with ModR/M 0x0F)
    cpu.memory.load_program(0xFFFF0, &[0x0F, 0xC0, 0x0F]);
    cpu.step();

    assert_eq!(
        cpu.memory.read(0x10100),
        0x0D,
        "Memory should be 10 + 3 = 13"
    );
    assert_eq!(cpu.cx & 0xFF, 0x0A, "CL should be old memory value (10)");
}

#[test]
fn test_mov_ah_imm8_preserves_al() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // MOV AH, 0xAB (0xB4, 0xAB)
    cpu.memory.load_program(0xFFFF0, &[0xB4, 0xAB]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0x12345678; // Start with a known value

    cpu.step();

    // Expected: AX should be 0x1234AB78 (AH=0xAB, AL=0x78 preserved)
    assert_eq!(cpu.ax & 0xFF, 0x78, "AL should be preserved");
    assert_eq!((cpu.ax >> 8) & 0xFF, 0xAB, "AH should be set to 0xAB");
    assert_eq!(
        cpu.ax & 0xFFFF_0000,
        0x1234_0000,
        "High 16 bits should be preserved"
    );
}

#[test]
fn test_mov_ch_imm8_preserves_cl() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // MOV CH, 0xCD (0xB5, 0xCD)
    cpu.memory.load_program(0xFFFF0, &[0xB5, 0xCD]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.cx = 0x11223344;

    cpu.step();

    assert_eq!(cpu.cx & 0xFF, 0x44, "CL should be preserved");
    assert_eq!((cpu.cx >> 8) & 0xFF, 0xCD, "CH should be set to 0xCD");
    assert_eq!(
        cpu.cx & 0xFFFF_0000,
        0x1122_0000,
        "High 16 bits should be preserved"
    );
}

#[test]
fn test_mov_dh_imm8_preserves_dl() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // MOV DH, 0xEF (0xB6, 0xEF)
    cpu.memory.load_program(0xFFFF0, &[0xB6, 0xEF]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.dx = 0xAABBCCDD;

    cpu.step();

    assert_eq!(cpu.dx & 0xFF, 0xDD, "DL should be preserved");
    assert_eq!((cpu.dx >> 8) & 0xFF, 0xEF, "DH should be set to 0xEF");
    assert_eq!(
        cpu.dx & 0xFFFF_0000,
        0xAABB_0000,
        "High 16 bits should be preserved"
    );
}

#[test]
fn test_mov_bh_imm8_preserves_bl() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // MOV BH, 0x99 (0xB7, 0x99)
    cpu.memory.load_program(0xFFFF0, &[0xB7, 0x99]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.bx = 0x11223344;

    cpu.step();

    assert_eq!(cpu.bx & 0xFF, 0x44, "BL should be preserved");
    assert_eq!((cpu.bx >> 8) & 0xFF, 0x99, "BH should be set to 0x99");
    assert_eq!(
        cpu.bx & 0xFFFF_0000,
        0x1122_0000,
        "High 16 bits should be preserved"
    );
}
