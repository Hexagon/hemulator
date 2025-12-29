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

#[test]
fn test_reset() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.ax = 0x1234;
    cpu.bx = 0x5678;
    cpu.flags = 0xFFFF;

    cpu.reset();

    assert_eq!(cpu.ax, 0);
    assert_eq!(cpu.bx, 0);
    assert_eq!(cpu.flags & 0x0002, 0x0002);
}

#[test]
fn test_physical_address() {
    let addr = Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x0234);
    assert_eq!(addr, 0x10234);

    let addr = Cpu8086::<ArrayMemory>::physical_address(0xFFFF, 0xFFFF);
    assert_eq!(addr, 0x10FFEF);
}

#[test]
fn test_nop() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.memory.load_program(0xFFFF0, &[0x90]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    let old_ip = cpu.ip;

    let cycles = cpu.step();
    assert_eq!(cycles, 3);
    assert_eq!(cpu.ip, old_ip + 1);
}

#[test]
fn test_halt() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.memory.load_program(0xFFFF0, &[0xF4]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();
    assert!(cpu.halted);

    // Further steps should do nothing
    let cycles = cpu.step();
    assert_eq!(cycles, 1);
}

#[test]
fn test_int_instruction() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // Setup interrupt vector for INT 0x10 at IVT address 0x0000:0x0040 (0x10 * 4)
    // IVT entry: offset=0x1000, segment=0xF000
    cpu.memory.write(0x0040, 0x00); // IP low
    cpu.memory.write(0x0041, 0x10); // IP high
    cpu.memory.write(0x0042, 0x00); // CS low
    cpu.memory.write(0x0043, 0xF0); // CS high

    // Load INT 0x10 instruction at CS:IP
    cpu.memory.load_program(0xFFFF0, &[0xCD, 0x10]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ss = 0x0000;
    cpu.sp = 0xFFFE;
    cpu.flags = 0x0202; // IF=1

    let old_ip = cpu.ip;
    let old_cs = cpu.cs;
    let old_flags = cpu.flags;

    cpu.step();

    // Check that CS:IP jumped to interrupt handler
    assert_eq!(cpu.cs, 0xF000);
    assert_eq!(cpu.ip, 0x1000);

    // Check that FLAGS, CS, IP were pushed to stack
    assert_eq!(cpu.sp, 0xFFF8); // Stack pointer moved down by 6 bytes

    // Read pushed values from stack (pushed in order: FLAGS, CS, IP)
    // Last pushed (IP) is at SP, first pushed (FLAGS) is at SP+4
    let pushed_ip = cpu.memory.read(0xFFF8) as u16 | ((cpu.memory.read(0xFFF9) as u16) << 8);
    let pushed_cs = cpu.memory.read(0xFFFA) as u16 | ((cpu.memory.read(0xFFFB) as u16) << 8);
    let pushed_flags = cpu.memory.read(0xFFFC) as u16 | ((cpu.memory.read(0xFFFD) as u16) << 8);

    // IP should point to next instruction (after INT)
    assert_eq!(pushed_ip, (old_ip + 2) as u16);
    assert_eq!(pushed_cs, old_cs);
    assert_eq!(pushed_flags, old_flags as u16);

    // Check that IF flag was cleared
    assert!(!cpu.get_flag(FLAG_IF));
}

#[test]
fn test_decode_modrm() {
    // Test ModR/M byte decoding
    let (modbits, reg, rm) = Cpu8086::<ArrayMemory>::decode_modrm(0b11_010_001);
    assert_eq!(modbits, 0b11); // Register mode
    assert_eq!(reg, 0b010); // DX
    assert_eq!(rm, 0b001); // CX

    let (modbits, reg, rm) = Cpu8086::<ArrayMemory>::decode_modrm(0b00_101_110);
    assert_eq!(modbits, 0b00); // Memory mode, no displacement
    assert_eq!(reg, 0b101); // BP
    assert_eq!(rm, 0b110); // Direct address

    let (modbits, reg, rm) = Cpu8086::<ArrayMemory>::decode_modrm(0b01_000_100);
    assert_eq!(modbits, 0b01); // Memory mode, 8-bit displacement
    assert_eq!(reg, 0b000); // AX
    assert_eq!(rm, 0b100); // [SI+disp8]
}

#[test]
fn test_effective_address_register_mode() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.bx = 0x1000;
    cpu.si = 0x0200;

    // Register mode should not calculate addresses
    // We're just testing that the function is callable
    let modbits = 0b11;
    let rm = 0b000;
    let (seg, offset, bytes) = cpu.calc_effective_address(modbits, rm);
    assert_eq!(bytes, 0);
    // In register mode, seg/offset are not used
    let _ = (seg, offset); // Suppress unused warning
}

#[test]
fn test_effective_address_no_displacement() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.ds = 0x1000;
    cpu.bx = 0x0100;
    cpu.si = 0x0020;

    // mod=00, rm=000: [BX+SI]
    let (seg, offset, bytes) = cpu.calc_effective_address(0b00, 0b000);
    assert_eq!(seg, 0x1000);
    assert_eq!(offset, 0x0120); // BX + SI
    assert_eq!(bytes, 0);

    // mod=00, rm=111: [BX]
    let (seg, offset, bytes) = cpu.calc_effective_address(0b00, 0b111);
    assert_eq!(seg, 0x1000);
    assert_eq!(offset, 0x0100); // BX
    assert_eq!(bytes, 0);
}

#[test]
fn test_effective_address_direct() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.ds = 0x1000;
    cpu.memory.load_program(0xFFFF0, &[0x34, 0x12]); // 16-bit displacement: 0x1234
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    // mod=00, rm=110: Direct address (16-bit displacement)
    let (seg, offset, bytes) = cpu.calc_effective_address(0b00, 0b110);
    assert_eq!(seg, 0x1000);
    assert_eq!(offset, 0x1234);
    assert_eq!(bytes, 2);
}

#[test]
fn test_effective_address_8bit_displacement() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.ds = 0x1000;
    cpu.si = 0x0100;
    cpu.memory.load_program(0xFFFF0, &[0x10]); // 8-bit displacement: +16
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    // mod=01, rm=100: [SI+disp8]
    let (seg, offset, bytes) = cpu.calc_effective_address(0b01, 0b100);
    assert_eq!(seg, 0x1000);
    assert_eq!(offset, 0x0110); // SI + 0x10
    assert_eq!(bytes, 1);
}

#[test]
fn test_effective_address_16bit_displacement() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.ds = 0x1000;
    cpu.bx = 0x0200;
    cpu.di = 0x0050;
    cpu.memory.load_program(0xFFFF0, &[0x00, 0x10]); // 16-bit displacement: 0x1000
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    // mod=10, rm=001: [BX+DI+disp16]
    let (seg, offset, bytes) = cpu.calc_effective_address(0b10, 0b001);
    assert_eq!(seg, 0x1000);
    assert_eq!(offset, 0x1250); // BX + DI + 0x1000
    assert_eq!(bytes, 2);
}

#[test]
fn test_cpu_model_default() {
    let mem = ArrayMemory::new();
    let cpu = Cpu8086::new(mem);
    assert_eq!(cpu.model(), CpuModel::Intel8086);
}

#[test]
fn test_cpu_model_with_model() {
    let mem = ArrayMemory::new();
    let cpu = Cpu8086::with_model(mem, CpuModel::Intel80186);
    assert_eq!(cpu.model(), CpuModel::Intel80186);
}

#[test]
fn test_cpu_model_set() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);
    assert_eq!(cpu.model(), CpuModel::Intel8086);

    cpu.set_model(CpuModel::Intel80286);
    assert_eq!(cpu.model(), CpuModel::Intel80286);
}

#[test]
fn test_cpu_model_preserved_on_reset() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80186);

    cpu.ax = 0x1234;
    assert_eq!(cpu.model(), CpuModel::Intel80186);

    cpu.reset();

    assert_eq!(cpu.ax, 0); // Registers reset
    assert_eq!(cpu.model(), CpuModel::Intel80186); // Model preserved
}

#[test]
fn test_cpu_model_feature_flags() {
    // 80186+ instructions support
    assert!(!CpuModel::Intel8086.supports_80186_instructions());
    assert!(!CpuModel::Intel8088.supports_80186_instructions());
    assert!(CpuModel::Intel80186.supports_80186_instructions());
    assert!(CpuModel::Intel80188.supports_80186_instructions());
    assert!(CpuModel::Intel80286.supports_80186_instructions());
    assert!(CpuModel::Intel80386.supports_80186_instructions());
    assert!(CpuModel::Intel80486.supports_80186_instructions());
    assert!(CpuModel::Intel80486SX.supports_80186_instructions());
    assert!(CpuModel::Intel80486DX2.supports_80186_instructions());
    assert!(CpuModel::Intel80486SX2.supports_80186_instructions());
    assert!(CpuModel::Intel80486DX4.supports_80186_instructions());
    assert!(CpuModel::IntelPentium.supports_80186_instructions());
    assert!(CpuModel::IntelPentiumMMX.supports_80186_instructions());

    // 80286+ instructions support
    assert!(!CpuModel::Intel8086.supports_80286_instructions());
    assert!(!CpuModel::Intel80186.supports_80286_instructions());
    assert!(CpuModel::Intel80286.supports_80286_instructions());
    assert!(CpuModel::Intel80386.supports_80286_instructions());
    assert!(CpuModel::Intel80486.supports_80286_instructions());
    assert!(CpuModel::Intel80486SX.supports_80286_instructions());
    assert!(CpuModel::Intel80486DX2.supports_80286_instructions());
    assert!(CpuModel::Intel80486SX2.supports_80286_instructions());
    assert!(CpuModel::Intel80486DX4.supports_80286_instructions());
    assert!(CpuModel::IntelPentium.supports_80286_instructions());
    assert!(CpuModel::IntelPentiumMMX.supports_80286_instructions());

    // 80386+ instructions support
    assert!(!CpuModel::Intel8086.supports_80386_instructions());
    assert!(!CpuModel::Intel80286.supports_80386_instructions());
    assert!(CpuModel::Intel80386.supports_80386_instructions());
    assert!(CpuModel::Intel80486.supports_80386_instructions());
    assert!(CpuModel::Intel80486SX.supports_80386_instructions());
    assert!(CpuModel::Intel80486DX2.supports_80386_instructions());
    assert!(CpuModel::Intel80486SX2.supports_80386_instructions());
    assert!(CpuModel::Intel80486DX4.supports_80386_instructions());
    assert!(CpuModel::IntelPentium.supports_80386_instructions());
    assert!(CpuModel::IntelPentiumMMX.supports_80386_instructions());

    // 80486+ instructions support
    assert!(!CpuModel::Intel8086.supports_80486_instructions());
    assert!(!CpuModel::Intel80286.supports_80486_instructions());
    assert!(!CpuModel::Intel80386.supports_80486_instructions());
    assert!(CpuModel::Intel80486.supports_80486_instructions());
    assert!(CpuModel::Intel80486SX.supports_80486_instructions());
    assert!(CpuModel::Intel80486DX2.supports_80486_instructions());
    assert!(CpuModel::Intel80486SX2.supports_80486_instructions());
    assert!(CpuModel::Intel80486DX4.supports_80486_instructions());
    assert!(CpuModel::IntelPentium.supports_80486_instructions());
    assert!(CpuModel::IntelPentiumMMX.supports_80486_instructions());

    // Pentium+ instructions support
    assert!(!CpuModel::Intel8086.supports_pentium_instructions());
    assert!(!CpuModel::Intel80286.supports_pentium_instructions());
    assert!(!CpuModel::Intel80386.supports_pentium_instructions());
    assert!(!CpuModel::Intel80486.supports_pentium_instructions());
    assert!(!CpuModel::Intel80486SX.supports_pentium_instructions());
    assert!(!CpuModel::Intel80486DX2.supports_pentium_instructions());
    assert!(!CpuModel::Intel80486SX2.supports_pentium_instructions());
    assert!(!CpuModel::Intel80486DX4.supports_pentium_instructions());
    assert!(CpuModel::IntelPentium.supports_pentium_instructions());
    assert!(CpuModel::IntelPentiumMMX.supports_pentium_instructions());
}

#[test]
fn test_cpu_model_names() {
    assert_eq!(CpuModel::Intel8086.name(), "Intel 8086");
    assert_eq!(CpuModel::Intel8088.name(), "Intel 8088");
    assert_eq!(CpuModel::Intel80186.name(), "Intel 80186");
    assert_eq!(CpuModel::Intel80188.name(), "Intel 80188");
    assert_eq!(CpuModel::Intel80286.name(), "Intel 80286");
    assert_eq!(CpuModel::Intel80386.name(), "Intel 80386");
    assert_eq!(CpuModel::Intel80486.name(), "Intel 80486");
    assert_eq!(CpuModel::Intel80486SX.name(), "Intel 80486 SX");
    assert_eq!(CpuModel::Intel80486DX2.name(), "Intel 80486 DX2");
    assert_eq!(CpuModel::Intel80486SX2.name(), "Intel 80486 SX2");
    assert_eq!(CpuModel::Intel80486DX4.name(), "Intel 80486 DX4");
    assert_eq!(CpuModel::IntelPentium.name(), "Intel Pentium");
    assert_eq!(CpuModel::IntelPentiumMMX.name(), "Intel Pentium MMX");
}

#[test]
fn test_mov_seg_to_reg() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // MOV AX, DS (0x8C with ModR/M 0b11_011_000)
    // seg=3 (DS), rm=0 (AX)
    cpu.memory.load_program(0xFFFF0, &[0x8C, 0b11_011_000]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ds = 0x1234;

    cpu.step();
    assert_eq!(cpu.ax, 0x1234); // AX should now contain DS value
}

#[test]
fn test_mov_reg_to_seg() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // MOV DS, AX (0x8E with ModR/M 0b11_011_000)
    // seg=3 (DS), rm=0 (AX)
    cpu.memory.load_program(0xFFFF0, &[0x8E, 0b11_011_000]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;
    cpu.ax = 0x5678;

    cpu.step();
    assert_eq!(cpu.ds, 0x5678); // DS should now contain AX value
}

#[test]
fn test_mov_seg_to_memory() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.ds = 0x1000;
    cpu.bx = 0x0100;
    cpu.es = 0x2345; // ES value to store

    // MOV [BX], ES (0x8C with ModR/M 0b00_000_111)
    // seg=0 (ES), rm=7 ([BX])
    cpu.memory.load_program(0xFFFF0, &[0x8C, 0b00_000_111]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    // Verify ES was written to memory
    let addr = Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x0100);
    let value = cpu.memory.read(addr) as u16 | ((cpu.memory.read(addr + 1) as u16) << 8);
    assert_eq!(value, 0x2345);
}

#[test]
fn test_mov_memory_to_seg() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.ds = 0x1000;
    cpu.bx = 0x0200;

    // Write test value to memory
    let addr = Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x0200);
    cpu.memory.write(addr, 0xCD); // Low byte
    cpu.memory.write(addr + 1, 0xAB); // High byte

    // MOV SS, [BX] (0x8E with ModR/M 0b00_010_111)
    // seg=2 (SS), rm=7 ([BX])
    cpu.memory.load_program(0xFFFF0, &[0x8E, 0b00_010_111]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();
    assert_eq!(cpu.ss, 0xABCD); // SS should contain value from memory
}

#[test]
fn test_movsb() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.ds = 0x1000;
    cpu.es = 0x2000;
    cpu.si = 0x0100;
    cpu.di = 0x0200;

    // Write source data
    let src_addr = Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x0100);
    cpu.memory.write(src_addr, 0x42);

    // MOVSB (0xA4)
    cpu.memory.load_program(0xFFFF0, &[0xA4]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    // Verify data copied
    let dst_addr = Cpu8086::<ArrayMemory>::physical_address(0x2000, 0x0200);
    assert_eq!(cpu.memory.read(dst_addr), 0x42);

    // Verify SI and DI incremented (DF=0)
    assert_eq!(cpu.si, 0x0101);
    assert_eq!(cpu.di, 0x0201);
}

#[test]
fn test_movsw() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.ds = 0x1000;
    cpu.es = 0x2000;
    cpu.si = 0x0100;
    cpu.di = 0x0200;

    // Write source word
    let src_addr = Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x0100);
    cpu.memory.write(src_addr, 0x34);
    cpu.memory.write(src_addr + 1, 0x12);

    // MOVSW (0xA5)
    cpu.memory.load_program(0xFFFF0, &[0xA5]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    // Verify word copied
    let dst_addr = Cpu8086::<ArrayMemory>::physical_address(0x2000, 0x0200);
    assert_eq!(cpu.memory.read(dst_addr), 0x34);
    assert_eq!(cpu.memory.read(dst_addr + 1), 0x12);

    // Verify SI and DI incremented by 2
    assert_eq!(cpu.si, 0x0102);
    assert_eq!(cpu.di, 0x0202);
}

#[test]
fn test_movsb_with_df() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.ds = 0x1000;
    cpu.es = 0x2000;
    cpu.si = 0x0100;
    cpu.di = 0x0200;
    cpu.set_flag(FLAG_DF, true); // Set direction flag

    // Write source data
    let src_addr = Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x0100);
    cpu.memory.write(src_addr, 0xAB);

    // MOVSB
    cpu.memory.load_program(0xFFFF0, &[0xA4]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    // Verify SI and DI decremented (DF=1)
    assert_eq!(cpu.si, 0x00FF);
    assert_eq!(cpu.di, 0x01FF);
}

#[test]
fn test_stosb() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.es = 0x2000;
    cpu.di = 0x0100;
    cpu.ax = 0x00FF; // AL = 0xFF

    // STOSB (0xAA)
    cpu.memory.load_program(0xFFFF0, &[0xAA]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    // Verify AL stored to ES:DI
    let addr = Cpu8086::<ArrayMemory>::physical_address(0x2000, 0x0100);
    assert_eq!(cpu.memory.read(addr), 0xFF);
    assert_eq!(cpu.di, 0x0101);
}

#[test]
fn test_stosw() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.es = 0x2000;
    cpu.di = 0x0100;
    cpu.ax = 0xABCD;

    // STOSW (0xAB)
    cpu.memory.load_program(0xFFFF0, &[0xAB]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    // Verify AX stored to ES:DI
    let addr = Cpu8086::<ArrayMemory>::physical_address(0x2000, 0x0100);
    assert_eq!(cpu.memory.read(addr), 0xCD);
    assert_eq!(cpu.memory.read(addr + 1), 0xAB);
    assert_eq!(cpu.di, 0x0102);
}

#[test]
fn test_lodsb() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.ds = 0x1000;
    cpu.si = 0x0100;

    // Write test data
    let addr = Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x0100);
    cpu.memory.write(addr, 0x55);

    // LODSB (0xAC)
    cpu.memory.load_program(0xFFFF0, &[0xAC]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    // Verify data loaded into AL
    assert_eq!(cpu.ax & 0xFF, 0x55);
    assert_eq!(cpu.si, 0x0101);
}

#[test]
fn test_lodsw() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.ds = 0x1000;
    cpu.si = 0x0100;

    // Write test word
    let addr = Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x0100);
    cpu.memory.write(addr, 0x78);
    cpu.memory.write(addr + 1, 0x56);

    // LODSW (0xAD)
    cpu.memory.load_program(0xFFFF0, &[0xAD]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    // Verify word loaded into AX
    assert_eq!(cpu.ax, 0x5678);
    assert_eq!(cpu.si, 0x0102);
}

#[test]
fn test_scasb() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.es = 0x2000;
    cpu.di = 0x0100;
    cpu.ax = 0x0042; // AL = 0x42

    // Write test data
    let addr = Cpu8086::<ArrayMemory>::physical_address(0x2000, 0x0100);
    cpu.memory.write(addr, 0x42);

    // SCASB (0xAE)
    cpu.memory.load_program(0xFFFF0, &[0xAE]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    // Verify ZF set (AL == [ES:DI])
    assert!(cpu.get_flag(FLAG_ZF));
    assert_eq!(cpu.di, 0x0101);
}

#[test]
fn test_scasw() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.es = 0x2000;
    cpu.di = 0x0100;
    cpu.ax = 0x1234;

    // Write different word
    let addr = Cpu8086::<ArrayMemory>::physical_address(0x2000, 0x0100);
    cpu.memory.write(addr, 0x56);
    cpu.memory.write(addr + 1, 0x78);

    // SCASW (0xAF)
    cpu.memory.load_program(0xFFFF0, &[0xAF]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    // Verify ZF clear (AX != [ES:DI])
    assert!(!cpu.get_flag(FLAG_ZF));
    assert_eq!(cpu.di, 0x0102);
}

#[test]
fn test_cmpsb() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.ds = 0x1000;
    cpu.es = 0x2000;
    cpu.si = 0x0100;
    cpu.di = 0x0200;

    // Write matching bytes
    let src_addr = Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x0100);
    let dst_addr = Cpu8086::<ArrayMemory>::physical_address(0x2000, 0x0200);
    cpu.memory.write(src_addr, 0x55);
    cpu.memory.write(dst_addr, 0x55);

    // CMPSB (0xA6)
    cpu.memory.load_program(0xFFFF0, &[0xA6]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    // Verify ZF set (bytes equal)
    assert!(cpu.get_flag(FLAG_ZF));
    assert_eq!(cpu.si, 0x0101);
    assert_eq!(cpu.di, 0x0201);
}

#[test]
fn test_cmpsw() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.ds = 0x1000;
    cpu.es = 0x2000;
    cpu.si = 0x0100;
    cpu.di = 0x0200;

    // Write different words
    let src_addr = Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x0100);
    let dst_addr = Cpu8086::<ArrayMemory>::physical_address(0x2000, 0x0200);
    cpu.memory.write(src_addr, 0x34);
    cpu.memory.write(src_addr + 1, 0x12);
    cpu.memory.write(dst_addr, 0x78);
    cpu.memory.write(dst_addr + 1, 0x56);

    // CMPSW (0xA7)
    cpu.memory.load_program(0xFFFF0, &[0xA7]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    // Verify ZF clear (words not equal)
    assert!(!cpu.get_flag(FLAG_ZF));
    assert_eq!(cpu.si, 0x0102);
    assert_eq!(cpu.di, 0x0202);
}

#[test]
fn test_rep_stosb() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.es = 0x2000;
    cpu.di = 0x0100;
    cpu.ax = 0x00AA; // AL = 0xAA
    cpu.cx = 5; // Repeat 5 times

    // REP STOSB (0xF3 0xAA)
    cpu.memory.load_program(0xFFFF0, &[0xF3, 0xAA]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    // Verify 5 bytes written
    let addr = Cpu8086::<ArrayMemory>::physical_address(0x2000, 0x0100);
    for i in 0..5 {
        assert_eq!(cpu.memory.read(addr + i), 0xAA);
    }
    assert_eq!(cpu.di, 0x0105);
    assert_eq!(cpu.cx, 0); // CX should be 0
}

#[test]
fn test_rep_movsb() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.ds = 0x1000;
    cpu.es = 0x2000;
    cpu.si = 0x0100;
    cpu.di = 0x0200;
    cpu.cx = 3;

    // Write source data
    let src_addr = Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x0100);
    cpu.memory.write(src_addr, 0x11);
    cpu.memory.write(src_addr + 1, 0x22);
    cpu.memory.write(src_addr + 2, 0x33);

    // REP MOVSB (0xF3 0xA4)
    cpu.memory.load_program(0xFFFF0, &[0xF3, 0xA4]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    // Verify all bytes copied
    let dst_addr = Cpu8086::<ArrayMemory>::physical_address(0x2000, 0x0200);
    assert_eq!(cpu.memory.read(dst_addr), 0x11);
    assert_eq!(cpu.memory.read(dst_addr + 1), 0x22);
    assert_eq!(cpu.memory.read(dst_addr + 2), 0x33);
    assert_eq!(cpu.cx, 0);
}

#[test]
fn test_repe_scasb_match() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.es = 0x2000;
    cpu.di = 0x0100;
    cpu.ax = 0x00FF; // AL = 0xFF
    cpu.cx = 5;

    // Fill memory with 0xFF
    let addr = Cpu8086::<ArrayMemory>::physical_address(0x2000, 0x0100);
    for i in 0..5 {
        cpu.memory.write(addr + i, 0xFF);
    }

    // REPE SCASB (0xF3 0xAE) - scan while equal
    cpu.memory.load_program(0xFFFF0, &[0xF3, 0xAE]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    // Should scan all 5 bytes and stop when CX=0
    assert_eq!(cpu.cx, 0);
    assert_eq!(cpu.di, 0x0105);
    assert!(cpu.get_flag(FLAG_ZF)); // Last comparison was equal
}

#[test]
fn test_repe_scasb_mismatch() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.es = 0x2000;
    cpu.di = 0x0100;
    cpu.ax = 0x00FF; // AL = 0xFF
    cpu.cx = 5;

    // Fill first 2 with 0xFF, then different value
    let addr = Cpu8086::<ArrayMemory>::physical_address(0x2000, 0x0100);
    cpu.memory.write(addr, 0xFF);
    cpu.memory.write(addr + 1, 0xFF);
    cpu.memory.write(addr + 2, 0xAA); // Different

    // REPE SCASB - should stop at mismatch
    cpu.memory.load_program(0xFFFF0, &[0xF3, 0xAE]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    // Should stop after 3 comparisons (2 matches + 1 mismatch)
    assert_eq!(cpu.cx, 2); // 5 - 3 = 2 remaining
    assert_eq!(cpu.di, 0x0103);
    assert!(!cpu.get_flag(FLAG_ZF)); // Last comparison was not equal
}

#[test]
fn test_repne_scasb() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.es = 0x2000;
    cpu.di = 0x0100;
    cpu.ax = 0x0000; // AL = 0x00 (looking for null)
    cpu.cx = 10;

    // Fill with non-zero, then zero at position 5
    let addr = Cpu8086::<ArrayMemory>::physical_address(0x2000, 0x0100);
    for i in 0..5 {
        cpu.memory.write(addr + i, 0xFF);
    }
    cpu.memory.write(addr + 5, 0x00); // Match at position 5

    // REPNE SCASB (0xF2 0xAE) - scan while not equal
    cpu.memory.load_program(0xFFFF0, &[0xF2, 0xAE]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    // Should stop when it finds 0x00 at position 5
    assert_eq!(cpu.cx, 4); // 10 - 6 = 4 remaining
    assert_eq!(cpu.di, 0x0106);
    assert!(cpu.get_flag(FLAG_ZF)); // Found match
}

#[test]
fn test_div_by_zero_exception_saves_faulting_ip() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // Setup: INT 0 vector points to a simple IRET at 0x1000:0x0000
    cpu.memory.write_u16(0x0000, 0x0000); // IP = 0x0000
    cpu.memory.write_u16(0x0002, 0x1000); // CS = 0x1000
    cpu.memory.load_program(0x10000, &[0xCF]); // IRET at 0x1000:0x0000

    // Setup: DIV by zero instruction at 0x2000:0x0100
    // DIV BL (0xF6 with ModR/M 0b11_110_011)
    cpu.memory.load_program(0x20100, &[0xF6, 0b11_110_011]);

    cpu.ip = 0x0100;
    cpu.cs = 0x2000;
    cpu.ss = 0x3000;
    cpu.sp = 0xFFFE;
    cpu.ax = 100; // Dividend
    cpu.bx = 0x0000; // BL = 0 (divisor)

    // Execute DIV instruction (should trigger INT 0)
    cpu.step();

    // After INT 0, we should be at the INT 0 handler (0x1000:0x0000)
    assert_eq!(cpu.cs, 0x1000, "CS should point to INT 0 handler segment");
    assert_eq!(cpu.ip, 0x0000, "IP should point to INT 0 handler offset");

    // Stack should contain: FLAGS, CS=0x2000, IP=0x0100 (start of DIV instruction)
    // SP was 0xFFFE, after 3 pushes it's 0xFFFE - 6 = 0xFFF8
    assert_eq!(cpu.sp, 0xFFF8, "Stack pointer should have 3 words pushed");

    // Pop the values to verify
    let saved_ip = cpu.pop();
    let saved_cs = cpu.pop();
    let _saved_flags = cpu.pop();

    // The saved IP should point to the START of the DIV instruction (0x0100)
    // NOT to the byte after it (0x0102)
    assert_eq!(
        saved_ip, 0x0100,
        "Saved IP should point to the faulting DIV instruction"
    );
    assert_eq!(
        saved_cs, 0x2000,
        "Saved CS should be the original code segment"
    );
}

#[test]
fn test_div_overflow_exception_saves_faulting_ip() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // Setup: INT 0 vector points to a simple IRET at 0x1000:0x0000
    cpu.memory.write_u16(0x0000, 0x0000); // IP = 0x0000
    cpu.memory.write_u16(0x0002, 0x1000); // CS = 0x1000
    cpu.memory.load_program(0x10000, &[0xCF]); // IRET at 0x1000:0x0000

    // Setup: DIV with overflow at 0x2000:0x0200
    // DIV BL (0xF6 with ModR/M 0b11_110_011)
    cpu.memory.load_program(0x20200, &[0xF6, 0b11_110_011]);

    cpu.ip = 0x0200;
    cpu.cs = 0x2000;
    cpu.ss = 0x3000;
    cpu.sp = 0xFFFE;
    cpu.ax = 0xFFFF; // Dividend = 65535
    cpu.bx = 0x0001; // BL = 1 (divisor)
                     // 65535 / 1 = 65535, which doesn't fit in AL (max 255) -> overflow

    // Execute DIV instruction (should trigger INT 0 due to overflow)
    cpu.step();

    // After INT 0, we should be at the INT 0 handler
    assert_eq!(cpu.cs, 0x1000);
    assert_eq!(cpu.ip, 0x0000);

    // Verify saved IP points to the faulting instruction
    assert_eq!(cpu.sp, 0xFFF8);
    let saved_ip = cpu.pop();

    assert_eq!(
        saved_ip, 0x0200,
        "Saved IP should point to the faulting DIV instruction on overflow"
    );
}

#[test]
fn test_software_int_saves_next_ip() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // Setup: INT 0x10 vector points to a simple IRET at 0x1000:0x0000
    cpu.memory.write_u16(0x0010 * 4, 0x0000); // IP = 0x0000
    cpu.memory.write_u16(0x0010 * 4 + 2, 0x1000); // CS = 0x1000
    cpu.memory.load_program(0x10000, &[0xCF]); // IRET at 0x1000:0x0000

    // Setup: INT 10h instruction at 0x2000:0x0300
    // INT 10h is 0xCD 0x10 (2 bytes)
    cpu.memory.load_program(0x20300, &[0xCD, 0x10, 0x90]); // INT 10h, NOP

    cpu.ip = 0x0300;
    cpu.cs = 0x2000;
    cpu.ss = 0x3000;
    cpu.sp = 0xFFFE;

    // Execute INT 10h instruction
    cpu.step();

    // After INT, we should be at the INT 10h handler
    assert_eq!(cpu.cs, 0x1000);
    assert_eq!(cpu.ip, 0x0000);

    // Verify saved IP points AFTER the INT instruction
    assert_eq!(cpu.sp, 0xFFF8);
    let saved_ip = cpu.pop();

    // Software INT should save IP pointing to the next instruction (0x0302)
    // NOT to the INT instruction itself (0x0300)
    assert_eq!(
        saved_ip, 0x0302,
        "Saved IP should point AFTER the INT instruction for software interrupts"
    );
}

#[test]
fn test_sub_with_memory_operand() {
    // Test SUB with memory operand (common in file position tracking)

    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // Store bytes_remaining at 0x0200
    cpu.memory.write(0x0200, 100);
    cpu.memory.write(0x0201, 0);

    cpu.memory.load_program(
        0x0100,
        &[
            0xB0, 0x0A, // MOV AL, 10           @ 0x0100
            0x28, 0x06, 0x00, 0x02, // SUB [0x0200], AL     @ 0x0102
            0x75, 0xF8, // JNZ -8               @ 0x0106 (loop back)
            0xF4, // HLT                  @ 0x0108
        ],
    );

    cpu.ip = 0x0100;
    cpu.cs = 0x0000;

    // Run until HLT or infinite loop
    let mut iterations = 0;
    loop {
        cpu.step();
        iterations += 1;

        let opcode = cpu.memory.read(((cpu.cs as u32) << 4) + cpu.ip);
        if opcode == 0xF4 {
            break;
        }

        if iterations > 50 {
            let remaining = cpu.memory.read(0x0200);
            panic!(
                "Infinite loop! iterations={}, remaining={}",
                iterations, remaining
            );
        }
    }

    assert_eq!(cpu.memory.read(0x0200), 0, "Should count down to 0");
    assert_eq!(
        iterations, 30,
        "Should take 30 instructions (10 loops * 3 instructions, HLT not counted)"
    );
}

#[test]
fn test_repne_cmpsb_with_segment_override() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // Set up segments - ES override should apply to source (DS:SI)
    cpu.ds = 0x1000;
    cpu.es = 0x3000; // Destination segment (always ES:DI, never overridden)
    cpu.si = 0x0100;
    cpu.di = 0x0200;
    cpu.cx = 5;

    // Write test data at ES:0x0100 (the overridden source segment)
    let src_addr = Cpu8086::<ArrayMemory>::physical_address(0x3000, 0x0100);
    cpu.memory.write(src_addr, 0xAA);
    cpu.memory.write(src_addr + 1, 0xBB);
    cpu.memory.write(src_addr + 2, 0xCC);
    cpu.memory.write(src_addr + 3, 0xDD); // This one matches
    cpu.memory.write(src_addr + 4, 0xEE);

    // Write data at ES:DI (destination) - first 3 don't match, 4th matches
    let dst_addr = Cpu8086::<ArrayMemory>::physical_address(0x3000, 0x0200);
    cpu.memory.write(dst_addr, 0x11); // Does NOT match 0xAA
    cpu.memory.write(dst_addr + 1, 0x22); // Does NOT match 0xBB
    cpu.memory.write(dst_addr + 2, 0x33); // Does NOT match 0xCC
    cpu.memory.write(dst_addr + 3, 0xDD); // MATCHES 0xDD - REPNE should stop here
    cpu.memory.write(dst_addr + 4, 0xEE);

    // ES: prefix (0x26) + REPNE (0xF2) + CMPSB (0xA6)
    cpu.memory.load_program(0xFFFF0, &[0x26, 0xF2, 0xA6]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    // REPNE should have compared 4 bytes (AA!=11, BB!=22, CC!=33, DD==DD) and stopped on 4th
    assert_eq!(
        cpu.cx, 1,
        "Should have 1 iteration remaining (5 - 4 comparisons)"
    );
    assert_eq!(cpu.si, 0x0104, "SI should have advanced 4 bytes");
    assert_eq!(cpu.di, 0x0204, "DI should have advanced 4 bytes");
    assert!(
        cpu.get_flag(FLAG_ZF),
        "ZF should be set (bytes matched on exit)"
    );
}

#[test]
fn test_repne_cmpsw_with_segment_override() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // Set up segments - SS override should apply to source (DS:SI)
    cpu.ds = 0x1000;
    cpu.ss = 0x2000; // Use SS instead of CS to avoid confusion
    cpu.es = 0x3000; // Destination segment (always ES:DI)
    cpu.si = 0x0100;
    cpu.di = 0x0200;
    cpu.cx = 3;

    // Write test data at SS:0x0100 (the overridden source segment)
    let src_addr = Cpu8086::<ArrayMemory>::physical_address(0x2000, 0x0100);
    cpu.memory.write(src_addr, 0x11);
    cpu.memory.write(src_addr + 1, 0x22);
    cpu.memory.write(src_addr + 2, 0x33);
    cpu.memory.write(src_addr + 3, 0x44);
    cpu.memory.write(src_addr + 4, 0x55);
    cpu.memory.write(src_addr + 5, 0x66);

    // Write data at ES:DI (destination) - first doesn't match, second matches
    let dst_addr = Cpu8086::<ArrayMemory>::physical_address(0x3000, 0x0200);
    cpu.memory.write(dst_addr, 0x99);
    cpu.memory.write(dst_addr + 1, 0x88); // First word: 8899 != 2211
    cpu.memory.write(dst_addr + 2, 0x33);
    cpu.memory.write(dst_addr + 3, 0x44); // Second word: 4433 == 4433 - REPNE should stop here
    cpu.memory.write(dst_addr + 4, 0x55);
    cpu.memory.write(dst_addr + 5, 0x66);

    // SS: prefix (0x36) + REPNE (0xF2) + CMPSW (0xA7)
    cpu.memory.load_program(0xFFFF0, &[0x36, 0xF2, 0xA7]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    // REPNE should have compared 2 words (2211!=9988, 4433==4433) and stopped
    assert_eq!(
        cpu.cx, 1,
        "Should have 1 iteration remaining (3 - 2 comparisons)"
    );
    assert_eq!(cpu.si, 0x0104, "SI should have advanced 4 bytes (2 words)");
    assert_eq!(cpu.di, 0x0204, "DI should have advanced 4 bytes (2 words)");
    assert!(
        cpu.get_flag(FLAG_ZF),
        "ZF should be set (words matched on exit)"
    );
}

#[test]
fn test_xlat_with_segment_override() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // Set up segments
    cpu.ds = 0x1000;
    cpu.es = 0x2000;
    cpu.bx = 0x0100;
    cpu.ax = 0x0005; // AL = 5

    // Write translation table at ES:0x0100 (with ES override)
    let table_addr = Cpu8086::<ArrayMemory>::physical_address(0x2000, 0x0100);
    cpu.memory.write(table_addr, 0xAA);
    cpu.memory.write(table_addr + 1, 0xBB);
    cpu.memory.write(table_addr + 2, 0xCC);
    cpu.memory.write(table_addr + 3, 0xDD);
    cpu.memory.write(table_addr + 4, 0xEE);
    cpu.memory.write(table_addr + 5, 0xFF); // Index 5

    // ES: prefix (0x26) + XLAT (0xD7)
    cpu.memory.load_program(0xFFFF0, &[0x26, 0xD7]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    // AL should contain the value from ES:BX+AL = ES:0x0105 = 0xFF
    assert_eq!(
        cpu.ax & 0xFF,
        0xFF,
        "AL should be 0xFF from the translation table"
    );
}

#[test]
fn test_xlat_without_segment_override() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // Set up segments
    cpu.ds = 0x1000;
    cpu.bx = 0x0100;
    cpu.ax = 0x0003; // AL = 3

    // Write translation table at DS:0x0100 (default segment)
    let table_addr = Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x0100);
    cpu.memory.write(table_addr, 0x10);
    cpu.memory.write(table_addr + 1, 0x20);
    cpu.memory.write(table_addr + 2, 0x30);
    cpu.memory.write(table_addr + 3, 0x40); // Index 3

    // XLAT (0xD7) without prefix
    cpu.memory.load_program(0xFFFF0, &[0xD7]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    // AL should contain the value from DS:BX+AL = DS:0x0103 = 0x40
    assert_eq!(
        cpu.ax & 0xFF,
        0x40,
        "AL should be 0x40 from the translation table"
    );
}

#[test]
fn test_lea_does_not_consume_segment_override() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.ds = 0x1000;
    cpu.es = 0x2000;
    cpu.bx = 0x0100;
    cpu.si = 0x0050;

    // ES: prefix (0x26) + LEA AX, [BX+SI] (0x8D 0x00)
    // ModR/M: mod=00, reg=000 (AX), r/m=000 ([BX+SI])
    // LEA should calculate offset only and NOT consume the ES: override
    // The next instruction should still see the ES: override
    cpu.memory
        .load_program(0xFFFF0, &[0x26, 0x8D, 0x00, 0xA0, 0x00, 0x00]);
    // After LEA: MOV AL, [0x0000] which should use ES: override from before
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    // Write test value at ES:0000
    let es_addr = Cpu8086::<ArrayMemory>::physical_address(0x2000, 0x0000);
    cpu.memory.write(es_addr, 0x99);

    // Write different value at DS:0000
    let ds_addr = Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x0000);
    cpu.memory.write(ds_addr, 0x88);

    // Execute LEA
    cpu.step();

    // LEA should have calculated the offset [BX+SI] = 0x0150
    assert_eq!(cpu.ax, 0x0150, "AX should contain offset 0x0150");

    // Now execute the MOV instruction
    // If LEA consumed the override, this will read from DS:0000 (0x88)
    // If LEA did NOT consume the override, this will read from ES:0000 (0x99)
    cpu.step();

    // This test verifies the fix: LEA should NOT consume the segment override
    // So the MOV should use ES: and read 0x99
    assert_eq!(
        cpu.ax & 0xFF,
        0x99,
        "AL should be 0x99 from ES:0000, proving LEA didn't consume ES: override"
    );
}

#[test]
fn test_rmw_displacement_not_fetched_twice_add() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    // Set up: BP=0x7C00, SP=0x7B00, value at [BP-0x10]=0x1234
    cpu.bp = 0x7C00;
    cpu.sp = 0x7B00;
    cpu.ss = 0x0000;
    cpu.ds = 0x0000;
    cpu.ax = 0x0100; // Value to add

    // Write test value at BP-0x10 = 0x7BF0
    cpu.memory.write(0x7BF0, 0x34);
    cpu.memory.write(0x7BF1, 0x12);

    // Instruction: ADD [BP-0x10], AX at 0x0000:0x0100
    // Encoding: 01 86 F0 FF
    // - 0x01: ADD r/m16, r16
    // - 0x86: ModR/M byte (mod=10, reg=000 (AX), rm=110 (BP+disp16))
    // - 0xF0 0xFF: Displacement -0x10 (two's complement of 16)
    cpu.cs = 0x0000;
    cpu.ip = 0x0100;
    cpu.memory.write(0x0100, 0x01); // ADD r/m16, r16
    cpu.memory.write(0x0101, 0x86); // ModR/M: mod=10, reg=000, rm=110
    cpu.memory.write(0x0102, 0xF0); // disp16 low byte
    cpu.memory.write(0x0103, 0xFF); // disp16 high byte

    // Execute the instruction
    cpu.step();

    // IP should advance by exactly 4 bytes (opcode + modrm + disp16)
    assert_eq!(cpu.ip, 0x0104, "IP should advance by 4 bytes, not more");

    // Memory at BP-0x10 should be 0x1234 + 0x0100 = 0x1334
    let result_lo = cpu.memory.read(0x7BF0);
    let result_hi = cpu.memory.read(0x7BF1);
    let result = (result_hi as u16) << 8 | result_lo as u16;
    assert_eq!(result, 0x1334, "ADD result should be correct");
}

#[test]
fn test_rmw_displacement_not_fetched_twice_or() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.bp = 0x1000;
    cpu.ds = 0x0000;
    cpu.cs = 0x0000;
    cpu.ax = 0x00FF;

    cpu.memory.write(0x0FF0, 0xF0); // Value at BP-0x10
    cpu.memory.write(0x0FF1, 0x0F);

    // OR [BP-0x10], AX
    cpu.ip = 0x0200;
    cpu.memory.write(0x0200, 0x09); // OR r/m16, r16
    cpu.memory.write(0x0201, 0x86); // ModR/M
    cpu.memory.write(0x0202, 0xF0); // disp16 low
    cpu.memory.write(0x0203, 0xFF); // disp16 high

    cpu.step();

    assert_eq!(cpu.ip, 0x0204, "IP should advance by exactly 4 bytes");

    let result = (cpu.memory.read(0x0FF1) as u16) << 8 | cpu.memory.read(0x0FF0) as u16;
    assert_eq!(result, 0x0FFF, "OR result should be correct");
}

#[test]
fn test_rmw_displacement_not_fetched_twice_and() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.bp = 0x2000;
    cpu.ds = 0x0000;
    cpu.cs = 0x0000;
    cpu.ax = 0xFF00;

    cpu.memory.write(0x1FE0, 0xFF); // Value at BP-0x20
    cpu.memory.write(0x1FE1, 0x0F);

    // AND [BP-0x20], AX
    cpu.ip = 0x0300;
    cpu.memory.write(0x0300, 0x21); // AND r/m16, r16
    cpu.memory.write(0x0301, 0x86); // ModR/M
    cpu.memory.write(0x0302, 0xE0); // disp16 low
    cpu.memory.write(0x0303, 0xFF); // disp16 high

    cpu.step();

    assert_eq!(cpu.ip, 0x0304, "IP should advance by exactly 4 bytes");

    let result = (cpu.memory.read(0x1FE1) as u16) << 8 | cpu.memory.read(0x1FE0) as u16;
    assert_eq!(result, 0x0F00, "AND result should be correct");
}

#[test]
fn test_rmw_displacement_not_fetched_twice_sub() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.bp = 0x3000;
    cpu.ds = 0x0000;
    cpu.cs = 0x0000;
    cpu.ax = 0x0001;

    cpu.memory.write(0x2FF0, 0x00); // Value at BP-0x10 = 0x1000
    cpu.memory.write(0x2FF1, 0x10);

    // SUB [BP-0x10], AX
    cpu.ip = 0x0400;
    cpu.memory.write(0x0400, 0x29); // SUB r/m16, r16
    cpu.memory.write(0x0401, 0x86); // ModR/M
    cpu.memory.write(0x0402, 0xF0); // disp16 low
    cpu.memory.write(0x0403, 0xFF); // disp16 high

    cpu.step();

    assert_eq!(cpu.ip, 0x0404, "IP should advance by exactly 4 bytes");

    let result = (cpu.memory.read(0x2FF1) as u16) << 8 | cpu.memory.read(0x2FF0) as u16;
    assert_eq!(result, 0x0FFF, "SUB result should be correct");
}

#[test]
fn test_rmw_displacement_not_fetched_twice_xor() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.bp = 0x4000;
    cpu.ds = 0x0000;
    cpu.cs = 0x0000;
    cpu.ax = 0x5555;

    cpu.memory.write(0x3FE0, 0xAA); // Value at BP-0x20 = 0xAAAA
    cpu.memory.write(0x3FE1, 0xAA);

    // XOR [BP-0x20], AX
    cpu.ip = 0x0500;
    cpu.memory.write(0x0500, 0x31); // XOR r/m16, r16
    cpu.memory.write(0x0501, 0x86); // ModR/M
    cpu.memory.write(0x0502, 0xE0); // disp16 low
    cpu.memory.write(0x0503, 0xFF); // disp16 high

    cpu.step();

    assert_eq!(cpu.ip, 0x0504, "IP should advance by exactly 4 bytes");

    let result = (cpu.memory.read(0x3FE1) as u16) << 8 | cpu.memory.read(0x3FE0) as u16;
    assert_eq!(result, 0xFFFF, "XOR result should be correct");
}

#[test]
fn test_rmw_displacement_not_fetched_twice_adc() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.bp = 0x5000;
    cpu.ds = 0x0000;
    cpu.cs = 0x0000;
    cpu.ax = 0x0001;
    cpu.set_flag(FLAG_CF, true); // Set carry flag

    cpu.memory.write(0x4FF0, 0xFF); // Value at BP-0x10 = 0x00FF
    cpu.memory.write(0x4FF1, 0x00);

    // ADC [BP-0x10], AX
    cpu.ip = 0x0600;
    cpu.memory.write(0x0600, 0x11); // ADC r/m16, r16
    cpu.memory.write(0x0601, 0x86); // ModR/M
    cpu.memory.write(0x0602, 0xF0); // disp16 low
    cpu.memory.write(0x0603, 0xFF); // disp16 high

    cpu.step();

    assert_eq!(cpu.ip, 0x0604, "IP should advance by exactly 4 bytes");

    let result = (cpu.memory.read(0x4FF1) as u16) << 8 | cpu.memory.read(0x4FF0) as u16;
    assert_eq!(result, 0x0101, "ADC result should include carry");
}

#[test]
fn test_rmw_displacement_not_fetched_twice_sbb() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::new(mem);

    cpu.bp = 0x6000;
    cpu.ds = 0x0000;
    cpu.cs = 0x0000;
    cpu.ax = 0x0001;
    cpu.set_flag(FLAG_CF, true); // Set borrow flag

    cpu.memory.write(0x5FF0, 0x00); // Value at BP-0x10 = 0x0100
    cpu.memory.write(0x5FF1, 0x01);

    // SBB [BP-0x10], AX
    cpu.ip = 0x0700;
    cpu.memory.write(0x0700, 0x19); // SBB r/m16, r16
    cpu.memory.write(0x0701, 0x86); // ModR/M
    cpu.memory.write(0x0702, 0xF0); // disp16 low
    cpu.memory.write(0x0703, 0xFF); // disp16 high

    cpu.step();

    assert_eq!(cpu.ip, 0x0704, "IP should advance by exactly 4 bytes");

    let result = (cpu.memory.read(0x5FF1) as u16) << 8 | cpu.memory.read(0x5FF0) as u16;
    assert_eq!(result, 0x00FE, "SBB result should include borrow");
}
