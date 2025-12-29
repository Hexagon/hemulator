//! Tests for BCD (Binary Coded Decimal) arithmetic instructions
//!
//! This module contains comprehensive tests for DAA, DAS, AAA, AAS, AAM, AAD
//! These instructions are noted in REFERENCE.md as "often implemented incorrectly"

use crate::cpu_8086::ArrayMemory;
use crate::cpu_8086::{Cpu8086, FLAG_AF, FLAG_CF, FLAG_PF, FLAG_SF, FLAG_ZF};

#[test]
fn test_daa_no_adjust_needed() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // ADD 0x05 + 0x03 = 0x08 (valid BCD, no adjustment needed)
    cpu.ax = 0x0005;
    cpu.memory.load_program(0xFFFF0, &[0x04, 0x03]); // ADD AL, 0x03
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.step();

    assert_eq!(cpu.ax & 0xFF, 0x08);

    // DAA
    cpu.memory.load_program(0xFFFF0, &[0x27]);
    cpu.ip = 0x0000;
    cpu.step();

    // Should remain 0x08 (valid BCD)
    assert_eq!(cpu.ax & 0xFF, 0x08);
}

#[test]
fn test_daa_low_nibble_adjust() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // 0x09 + 0x08 = 0x11, low nibble needs adjustment
    cpu.ax = 0x0009;
    cpu.memory.load_program(0xFFFF0, &[0x04, 0x08]); // ADD AL, 0x08
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.step();

    assert_eq!(cpu.ax & 0xFF, 0x11); // Binary result

    // DAA should adjust to BCD
    cpu.memory.load_program(0xFFFF0, &[0x27]);
    cpu.ip = 0x0000;
    cpu.step();

    // 0x11 + 0x06 = 0x17 (BCD for 17)
    assert_eq!(cpu.ax & 0xFF, 0x17);
}

#[test]
fn test_daa_high_nibble_adjust() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // 0x99 + 0x01 = 0x9A, high nibble needs adjustment
    cpu.ax = 0x0099;
    cpu.memory.load_program(0xFFFF0, &[0x04, 0x01]); // ADD AL, 0x01
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.step();

    assert_eq!(cpu.ax & 0xFF, 0x9A); // Binary result

    // DAA
    cpu.memory.load_program(0xFFFF0, &[0x27]);
    cpu.ip = 0x0000;
    cpu.step();

    // 0x9A + 0x60 = 0x00 with carry (BCD for 100)
    assert_eq!(cpu.ax & 0xFF, 0x00);
    assert!(cpu.get_flag(FLAG_CF));
}

#[test]
fn test_daa_both_nibbles_adjust() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // 0x9F (invalid BCD but test it)
    cpu.ax = 0x009F;

    // DAA
    cpu.memory.load_program(0xFFFF0, &[0x27]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.step();

    // 0x9F + 0x66 = 0x05 with carry
    assert_eq!(cpu.ax & 0xFF, 0x05);
    assert!(cpu.get_flag(FLAG_CF));
}

#[test]
fn test_das_no_adjust_needed() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // SUB 0x08 - 0x03 = 0x05 (valid BCD)
    cpu.ax = 0x0008;
    cpu.memory.load_program(0xFFFF0, &[0x2C, 0x03]); // SUB AL, 0x03
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.step();

    assert_eq!(cpu.ax & 0xFF, 0x05);

    // DAS
    cpu.memory.load_program(0xFFFF0, &[0x2F]);
    cpu.ip = 0x0000;
    cpu.step();

    // Should remain 0x05
    assert_eq!(cpu.ax & 0xFF, 0x05);
}

#[test]
fn test_das_low_nibble_adjust() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // 0x20 - 0x07 = 0x19, needs BCD adjustment
    cpu.ax = 0x0020;
    cpu.memory.load_program(0xFFFF0, &[0x2C, 0x07]); // SUB AL, 0x07
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.step();

    assert_eq!(cpu.ax & 0xFF, 0x19); // Binary result

    // DAS
    cpu.memory.load_program(0xFFFF0, &[0x2F]);
    cpu.ip = 0x0000;
    cpu.step();

    // 0x19 - 0x06 = 0x13 (BCD for 13)
    assert_eq!(cpu.ax & 0xFF, 0x13);
}

#[test]
fn test_aaa_no_adjust() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // AL = 0x05 (valid digit, no adjust needed)
    cpu.ax = 0x0005;

    // AAA
    cpu.memory.load_program(0xFFFF0, &[0x37]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.step();

    // AL should remain 0x05, AH should remain 0
    assert_eq!(cpu.ax & 0xFF, 0x05);
    assert_eq!((cpu.ax >> 8) & 0xFF, 0x00);
    assert!(!cpu.get_flag(FLAG_CF));
    assert!(!cpu.get_flag(FLAG_AF));
}

#[test]
fn test_aaa_adjust_needed() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // 0x09 + 0x08 = 0x11 needs ASCII adjust
    cpu.ax = 0x0009;
    cpu.memory.load_program(0xFFFF0, &[0x04, 0x08]); // ADD AL, 0x08
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.step();

    assert_eq!(cpu.ax & 0xFF, 0x11);

    // AAA
    cpu.memory.load_program(0xFFFF0, &[0x37]);
    cpu.ip = 0x0000;
    cpu.step();

    // AL should be 0x07 (low nibble + 6, then & 0x0F)
    // AH should be incremented
    assert_eq!(cpu.ax & 0x0F, 0x07);
    assert_eq!((cpu.ax >> 8) & 0xFF, 0x01);
    assert!(cpu.get_flag(FLAG_CF));
    assert!(cpu.get_flag(FLAG_AF));
}

#[test]
fn test_aas_no_adjust() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // AL = 0x08, no adjust needed
    cpu.ax = 0x0008;

    // AAS
    cpu.memory.load_program(0xFFFF0, &[0x3F]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.step();

    // Should remain unchanged
    assert_eq!(cpu.ax & 0xFF, 0x08);
    assert_eq!((cpu.ax >> 8) & 0xFF, 0x00);
    assert!(!cpu.get_flag(FLAG_CF));
}

#[test]
fn test_aas_adjust_needed() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // 0x00 - 0x01 = 0xFF (needs adjustment)
    cpu.ax = 0x0500; // AH = 5, AL = 0
    cpu.memory.load_program(0xFFFF0, &[0x2C, 0x01]); // SUB AL, 0x01
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.step();

    assert_eq!(cpu.ax & 0xFF, 0xFF);

    // AAS
    cpu.memory.load_program(0xFFFF0, &[0x3F]);
    cpu.ip = 0x0000;
    cpu.step();

    // AL should be adjusted
    assert_eq!(cpu.ax & 0x0F, 0x09); // Low nibble should be 9
    assert_eq!((cpu.ax >> 8) & 0xFF, 0x04); // AH decremented
    assert!(cpu.get_flag(FLAG_CF));
}

#[test]
fn test_aam_basic() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // AL = 0x3F (63 decimal, result of multiplying two single digits)
    cpu.ax = 0x003F;

    // AAM (0xD4 0x0A) - divide by 10
    cpu.memory.load_program(0xFFFF0, &[0xD4, 0x0A]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.step();

    // 63 / 10 = 6 remainder 3
    // AH should be 6 (quotient), AL should be 3 (remainder)
    assert_eq!((cpu.ax >> 8) & 0xFF, 0x06);
    assert_eq!(cpu.ax & 0xFF, 0x03);
}

#[test]
fn test_aam_with_different_base() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // AL = 0x20 (32 decimal)
    cpu.ax = 0x0020;

    // AAM with base 5 (0xD4 0x05)
    cpu.memory.load_program(0xFFFF0, &[0xD4, 0x05]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.step();

    // 32 / 5 = 6 remainder 2
    assert_eq!((cpu.ax >> 8) & 0xFF, 0x06);
    assert_eq!(cpu.ax & 0xFF, 0x02);
}

#[test]
fn test_aam_zero_result() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // AL = 0x00
    cpu.ax = 0x0000;

    // AAM
    cpu.memory.load_program(0xFFFF0, &[0xD4, 0x0A]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.step();

    // Both should be zero
    assert_eq!(cpu.ax, 0x0000);
    assert!(cpu.get_flag(FLAG_ZF));
}

#[test]
fn test_aad_basic() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // AH = 4, AL = 2 represents 42 in unpacked BCD
    cpu.ax = 0x0402;

    // AAD (0xD5 0x0A) - multiply by 10 and add
    cpu.memory.load_program(0xFFFF0, &[0xD5, 0x0A]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.step();

    // Result: 4 * 10 + 2 = 42 (0x2A)
    // AH should be 0, AL should be 42
    assert_eq!((cpu.ax >> 8) & 0xFF, 0x00);
    assert_eq!(cpu.ax & 0xFF, 0x2A);
}

#[test]
fn test_aad_with_different_base() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // AH = 3, AL = 2 in base 5 represents 17 decimal (3*5 + 2)
    cpu.ax = 0x0302;

    // AAD with base 5
    cpu.memory.load_program(0xFFFF0, &[0xD5, 0x05]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.step();

    // Result: 3 * 5 + 2 = 17 (0x11)
    assert_eq!((cpu.ax >> 8) & 0xFF, 0x00);
    assert_eq!(cpu.ax & 0xFF, 0x11);
}

#[test]
fn test_aad_zero() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.ax = 0x0000;

    // AAD
    cpu.memory.load_program(0xFFFF0, &[0xD5, 0x0A]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.step();

    assert_eq!(cpu.ax, 0x0000);
    assert!(cpu.get_flag(FLAG_ZF));
}

#[test]
fn test_bcd_sequence_add() {
    // Test a complete BCD addition sequence
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // Add 58 + 46 in BCD
    // 58 = 0x58 in BCD, 46 = 0x46 in BCD
    cpu.ax = 0x0058;
    cpu.memory.load_program(
        0xFFFF0,
        &[
            0x04, 0x46, // ADD AL, 0x46
            0x27, // DAA
        ],
    );
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step(); // ADD
    cpu.step(); // DAA

    // 58 + 46 = 104 in decimal
    // In BCD with carry: 0x04 in AL, CF set
    assert_eq!(cpu.ax & 0xFF, 0x04);
    assert!(cpu.get_flag(FLAG_CF)); // Indicates the hundreds digit
}

#[test]
fn test_bcd_sequence_subtract() {
    // Test a complete BCD subtraction sequence
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // Subtract 25 - 17 in BCD
    // 25 = 0x25, 17 = 0x17
    cpu.ax = 0x0025;
    cpu.memory.load_program(
        0xFFFF0,
        &[
            0x2C, 0x17, // SUB AL, 0x17
            0x2F, // DAS
        ],
    );
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step(); // SUB
    cpu.step(); // DAS

    // 25 - 17 = 8
    assert_eq!(cpu.ax & 0xFF, 0x08);
    assert!(!cpu.get_flag(FLAG_CF));
}

#[test]
fn test_ascii_to_bcd_conversion() {
    // Test converting ASCII digits to BCD and performing arithmetic
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // ASCII '5' = 0x35, '3' = 0x33
    // Mask to get digits: AND AL, 0x0F
    cpu.ax = 0x0035; // ASCII '5'
    cpu.memory.load_program(
        0xFFFF0,
        &[
            0x24, 0x0F, // AND AL, 0x0F  -> AL = 0x05
            0x04, 0x03, // ADD AL, 0x03  -> AL = 0x08
            0x0C, 0x30, // OR AL, 0x30   -> AL = 0x38 (ASCII '8')
        ],
    );
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step(); // AND
    assert_eq!(cpu.ax & 0xFF, 0x05);

    cpu.step(); // ADD
    assert_eq!(cpu.ax & 0xFF, 0x08);

    cpu.step(); // OR
    assert_eq!(cpu.ax & 0xFF, 0x38); // ASCII '8'
}
