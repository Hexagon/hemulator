//! Tests for PR #192 bug fixes
//!
//! This module contains tests that specifically target the bugs fixed in PR #192:
//! 1. CX register upper 16 bits preservation in REP string operations and LOOP instructions
//! 2. Operand size override for PUSH/POP operations
//! 3. LOOP/LOOPZ/LOOPNZ with 32-bit ECX register

use crate::cpu_8086::ArrayMemory;
use crate::cpu_8086::{Cpu8086, CpuModel, Memory8086, FLAG_ZF};

/// Test that REP MOVSB preserves upper 16 bits of ECX
#[test]
fn test_rep_movsb_preserves_ecx_upper_bits() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    cpu.ds = 0x1000;
    cpu.es = 0x2000;
    cpu.si = 0x0100;
    cpu.di = 0x0200;
    // Set ECX with upper bits set: 0xDEAD0003 means 3 iterations
    cpu.cx = 0xDEAD0003;

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

    // CRITICAL: Check that upper 16 bits of ECX were preserved
    assert_eq!(cpu.cx & 0xFFFF, 0, "CX should be 0 after REP");
    assert_eq!(
        cpu.cx >> 16,
        0xDEAD,
        "Upper 16 bits of ECX should be preserved"
    );
    assert_eq!(cpu.cx, 0xDEAD0000, "ECX should be 0xDEAD0000");
}

/// Test that REP STOSB preserves upper 16 bits of ECX
#[test]
fn test_rep_stosb_preserves_ecx_upper_bits() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    cpu.es = 0x2000;
    cpu.di = 0x0100;
    cpu.ax = 0x00AA; // AL = 0xAA
                     // Set ECX with upper bits set: 0x12340005 means 5 iterations
    cpu.cx = 0x12340005;

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

    // CRITICAL: Check that upper 16 bits of ECX were preserved
    assert_eq!(cpu.cx & 0xFFFF, 0, "CX should be 0 after REP");
    assert_eq!(
        cpu.cx >> 16,
        0x1234,
        "Upper 16 bits of ECX should be preserved"
    );
    assert_eq!(cpu.cx, 0x12340000, "ECX should be 0x12340000");
}

/// Test that REP STOSW preserves upper 16 bits of ECX
#[test]
fn test_rep_stosw_preserves_ecx_upper_bits() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    cpu.es = 0x2000;
    cpu.di = 0x0100;
    cpu.ax = 0xBEEF;
    // Set ECX with upper bits set: 0xABCD0003 means 3 iterations
    cpu.cx = 0xABCD0003;

    // REP STOSW (0xF3 0xAB)
    cpu.memory.load_program(0xFFFF0, &[0xF3, 0xAB]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    // Verify 3 words written
    let addr = Cpu8086::<ArrayMemory>::physical_address(0x2000, 0x0100);
    for i in 0..3 {
        assert_eq!(cpu.memory.read_u16(addr + i * 2), 0xBEEF);
    }

    // CRITICAL: Check that upper 16 bits of ECX were preserved
    assert_eq!(cpu.cx & 0xFFFF, 0, "CX should be 0 after REP");
    assert_eq!(
        cpu.cx >> 16,
        0xABCD,
        "Upper 16 bits of ECX should be preserved"
    );
    assert_eq!(cpu.cx, 0xABCD0000, "ECX should be 0xABCD0000");
}

/// Test that LOOP preserves upper 16 bits of ECX
#[test]
fn test_loop_preserves_ecx_upper_bits() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    // Set ECX with upper bits set: 0x5678000A means 10 iterations
    cpu.cx = 0x5678000A;
    cpu.bx = 0;

    // Program that loops 10 times, incrementing BX each time:
    // loop_start:
    // INC BX          @ 0x0100
    // LOOP loop_start @ 0x0101 (offset -3)
    // HLT             @ 0x0103
    cpu.memory.load_program(
        0x0100,
        &[
            0x43, // INC BX
            0xE2, 0xFD, // LOOP -3 (back to 0x0100)
            0xF4, // HLT
        ],
    );

    cpu.ip = 0x0100;
    cpu.cs = 0x0000;

    // Execute until HLT or timeout
    let mut steps = 0;
    while cpu.ip != 0x0103 && steps < 50 {
        cpu.step();
        steps += 1;
    }

    // Verify results
    assert_eq!(cpu.cx & 0xFFFF, 0, "CX should be 0 after loop completes");
    assert_eq!(
        cpu.cx >> 16,
        0x5678,
        "Upper 16 bits of ECX should be preserved"
    );
    assert_eq!(cpu.cx, 0x56780000, "ECX should be 0x56780000");
    assert_eq!(cpu.bx, 10, "BX should be incremented 10 times");
}

/// Test that LOOPZ preserves upper 16 bits of ECX
#[test]
fn test_loopz_preserves_ecx_upper_bits() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    // Set ECX with upper bits set: 0xFFFF0003 means 3 iterations max
    cpu.cx = 0xFFFF0003;

    // Simple program: just LOOPZ in a tight loop with ZF set
    // We'll set ZF manually before each iteration
    cpu.memory.load_program(
        0x0100,
        &[
            0xE1, 0xFE, // LOOPZ -2 (loop to self, infinite if ZF && CX)
            0xF4, // HLT
        ],
    );

    cpu.ip = 0x0100;
    cpu.cs = 0x0000;
    cpu.flags = FLAG_ZF; // Set ZF so loop continues

    // Execute 3 iterations manually
    cpu.step(); // CX: 3 -> 2, ZF=1, continues
    assert_eq!(cpu.cx & 0xFFFF, 2);
    assert_eq!(cpu.cx >> 16, 0xFFFF);

    cpu.flags = FLAG_ZF; // Keep ZF set
    cpu.step(); // CX: 2 -> 1, ZF=1, continues
    assert_eq!(cpu.cx & 0xFFFF, 1);
    assert_eq!(cpu.cx >> 16, 0xFFFF);

    cpu.flags = FLAG_ZF; // Keep ZF set
    cpu.step(); // CX: 1 -> 0, ZF=1, exits (CX=0)

    // Verify results - should exit because CX reached 0
    assert_eq!(cpu.cx & 0xFFFF, 0, "CX should be 0 when loop exits");
    assert_eq!(
        cpu.cx >> 16,
        0xFFFF,
        "Upper 16 bits of ECX should be preserved"
    );
    assert_eq!(cpu.cx, 0xFFFF0000, "ECX should be 0xFFFF0000");
    assert_eq!(cpu.ip, 0x0102, "Should exit loop and be at HLT");
}

/// Test that LOOPNZ preserves upper 16 bits of ECX
#[test]
fn test_loopnz_preserves_ecx_upper_bits() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    // Set ECX with upper bits set: 0x98760003 means 3 iterations max
    cpu.cx = 0x98760003;
    cpu.bx = 0; // Counter

    // Program that loops while ZF is clear
    // INC BX           @ 0x0100
    // CMP BX, BX       @ 0x0101 (always sets ZF=1)
    // LOOPNZ loop      @ 0x0103 (back to 0x0100 while !ZF && CX)
    // HLT              @ 0x0105
    cpu.memory.load_program(
        0x0100,
        &[
            0x43, // INC BX
            0x39, 0xDB, // CMP BX, BX (always ZF=1)
            0xE0, 0xFA, // LOOPNZ -6 (back to 0x0100)
            0xF4, // HLT
        ],
    );

    cpu.ip = 0x0100;
    cpu.cs = 0x0000;

    // Execute until HLT or timeout
    let mut steps = 0;
    while cpu.ip != 0x0105 && steps < 50 {
        cpu.step();
        steps += 1;
    }

    // LOOPNZ should stop on first iteration when ZF is set
    // 1st iteration: INC BX (BX=1), CMP BX,BX (ZF=1), LOOPNZ (CX=2, ZF=1, exit)
    assert_eq!(cpu.cx & 0xFFFF, 2, "CX should be 2 when ZF sets");
    assert_eq!(
        cpu.cx >> 16,
        0x9876,
        "Upper 16 bits of ECX should be preserved"
    );
    assert_eq!(cpu.cx, 0x98760002, "ECX should be 0x98760002");
    assert_eq!(cpu.bx, 1, "BX should be 1");
}

/// Test that REP SCASB preserves upper 16 bits of ECX
#[test]
fn test_rep_scasb_preserves_ecx_upper_bits() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    cpu.es = 0x2000;
    cpu.di = 0x0100;
    cpu.ax = 0x00FF; // AL = 0xFF (search for this)
                     // Set ECX with upper bits set: 0x11110005 means search 5 bytes max
    cpu.cx = 0x11110005;

    // Write data to search - match on 3rd byte
    let addr = Cpu8086::<ArrayMemory>::physical_address(0x2000, 0x0100);
    cpu.memory.write(addr, 0xAA); // Different
    cpu.memory.write(addr + 1, 0xBB); // Different
    cpu.memory.write(addr + 2, 0xFF); // Match! (REPNE stops here)
    cpu.memory.write(addr + 3, 0xCC); // Won't be scanned
    cpu.memory.write(addr + 4, 0xDD); // Won't be scanned

    // REPNE SCASB (0xF2 0xAE) - stops when match found (ZF=1)
    cpu.memory.load_program(0xFFFF0, &[0xF2, 0xAE]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    // Should scan 3 bytes, then stop on match
    // CX starts at 5, decrements to 2 when match is found
    assert_eq!(
        cpu.cx & 0xFFFF,
        2,
        "CX should be 2 when match found on 3rd byte"
    );
    assert_eq!(
        cpu.cx >> 16,
        0x1111,
        "Upper 16 bits of ECX should be preserved"
    );
    assert_eq!(cpu.cx, 0x11110002, "ECX should be 0x11110002");
}

/// Test that REP CMPSB preserves upper 16 bits of ECX
#[test]
fn test_rep_cmpsb_preserves_ecx_upper_bits() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    cpu.ds = 0x1000;
    cpu.es = 0x2000;
    cpu.si = 0x0100;
    cpu.di = 0x0200;
    // Set ECX with upper bits set: 0xBEEF0004 means compare 4 bytes max
    cpu.cx = 0xBEEF0004;
    cpu.flags = FLAG_ZF; // REPE continues while ZF=1

    // Write matching data to both locations
    let src_addr = Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x0100);
    let dst_addr = Cpu8086::<ArrayMemory>::physical_address(0x2000, 0x0200);
    for i in 0..4 {
        cpu.memory.write(src_addr + i, 0x42);
        cpu.memory.write(dst_addr + i, 0x42);
    }

    // REPE CMPSB (0xF3 0xA6)
    cpu.memory.load_program(0xFFFF0, &[0xF3, 0xA6]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    // Verify CX is decremented but upper bits preserved
    assert_eq!(cpu.cx & 0xFFFF, 0, "CX should be 0 after REPE CMPSB");
    assert_eq!(
        cpu.cx >> 16,
        0xBEEF,
        "Upper 16 bits of ECX should be preserved"
    );
    assert_eq!(cpu.cx, 0xBEEF0000, "ECX should be 0xBEEF0000");
}

/// Test 32-bit PUSH with operand size override
#[test]
fn test_push_32bit_with_override() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    cpu.ss = 0x1000;
    cpu.sp = 0x0100;
    cpu.ax = 0xDEADBEEF; // Full 32-bit value

    // 0x66 0x50 = PUSH EAX (with operand size override)
    cpu.memory.load_program(0xFFFF0, &[0x66, 0x50]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    // Verify SP decremented by 4
    assert_eq!(cpu.sp, 0x00FC, "SP should be decremented by 4");

    // Verify 32-bit value was pushed to stack (little-endian)
    let addr = Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x00FC);
    assert_eq!(cpu.memory.read(addr), 0xEF, "Byte 0 should be 0xEF");
    assert_eq!(cpu.memory.read(addr + 1), 0xBE, "Byte 1 should be 0xBE");
    assert_eq!(cpu.memory.read(addr + 2), 0xAD, "Byte 2 should be 0xAD");
    assert_eq!(cpu.memory.read(addr + 3), 0xDE, "Byte 3 should be 0xDE");
}

/// Test 32-bit POP with operand size override
#[test]
fn test_pop_32bit_with_override() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    cpu.ss = 0x1000;
    cpu.sp = 0x00FC;
    cpu.ax = 0x00000000; // Clear EAX

    // Write 32-bit value to stack (little-endian)
    let addr = Cpu8086::<ArrayMemory>::physical_address(0x1000, 0x00FC);
    cpu.memory.write(addr, 0xEF);
    cpu.memory.write(addr + 1, 0xBE);
    cpu.memory.write(addr + 2, 0xAD);
    cpu.memory.write(addr + 3, 0xDE);

    // 0x66 0x58 = POP EAX (with operand size override)
    cpu.memory.load_program(0xFFFF0, &[0x66, 0x58]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    // Verify SP incremented by 4
    assert_eq!(cpu.sp, 0x0100, "SP should be incremented by 4");

    // Verify 32-bit value was popped from stack
    assert_eq!(cpu.ax, 0xDEADBEEF, "EAX should contain full 32-bit value");
}

/// Test 32-bit PUSHF with operand size override
#[test]
fn test_pushf_32bit_with_override() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    cpu.ss = 0x1000;
    cpu.sp = 0x0100;
    cpu.flags = 0x12345678; // Set 32-bit flags

    // 0x66 0x9C = PUSHFD (with operand size override)
    cpu.memory.load_program(0xFFFF0, &[0x66, 0x9C]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    // Verify SP decremented by 4
    assert_eq!(cpu.sp, 0x00FC, "SP should be decremented by 4");

    // Verify 32-bit flags value was pushed to stack
    let flags_val = cpu.read_u32(0x1000, 0x00FC);
    assert_eq!(flags_val, 0x12345678, "Full 32-bit EFLAGS should be pushed");
}

/// Test 32-bit POPF with operand size override
#[test]
fn test_popf_32bit_with_override() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    cpu.ss = 0x1000;
    cpu.sp = 0x00FC;
    cpu.flags = 0x00000000; // Clear flags

    // Write 32-bit flags to stack
    cpu.write_u32(0x1000, 0x00FC, 0x12345678);

    // 0x66 0x9D = POPFD (with operand size override)
    cpu.memory.load_program(0xFFFF0, &[0x66, 0x9D]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    // Verify SP incremented by 4
    assert_eq!(cpu.sp, 0x0100, "SP should be incremented by 4");

    // Verify 32-bit flags value was popped from stack
    assert_eq!(cpu.flags, 0x12345678, "Full 32-bit EFLAGS should be popped");
}

/// Test 32-bit CALL with operand size override
#[test]
fn test_call_32bit_with_override() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    cpu.ss = 0x1000;
    cpu.sp = 0x0100;
    cpu.ip = 0x0200;
    cpu.cs = 0x0000;

    // 0x66 0xE8 offset32 = CALL near with 32-bit offset
    // offset = 0x12345678 (little-endian: 78 56 34 12)
    cpu.memory
        .load_program(0x0200, &[0x66, 0xE8, 0x78, 0x56, 0x34, 0x12]);

    cpu.step();

    // Verify return address (IP after instruction) was pushed as 32-bit
    let ret_addr = cpu.read_u32(0x1000, 0x00FC);
    assert_eq!(
        ret_addr, 0x00000206,
        "32-bit return address should be pushed"
    );
    assert_eq!(cpu.sp, 0x00FC, "SP should be decremented by 4");

    // Verify IP was updated
    let target = 0x0206u32.wrapping_add(0x12345678);
    assert_eq!(cpu.ip, target, "IP should be updated with 32-bit offset");
}

/// Test 32-bit RET with operand size override
#[test]
fn test_ret_32bit_with_override() {
    let mem = ArrayMemory::new();
    let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

    cpu.ss = 0x1000;
    cpu.sp = 0x00FC;
    cpu.cs = 0x0000;

    // Push a 32-bit return address to stack
    cpu.write_u32(0x1000, 0x00FC, 0x00001234);

    // 0x66 0xC3 = RET near (32-bit)
    cpu.memory.load_program(0xFFFF0, &[0x66, 0xC3]);
    cpu.ip = 0x0000;
    cpu.cs = 0xFFFF;

    cpu.step();

    // Verify SP incremented by 4
    assert_eq!(cpu.sp, 0x0100, "SP should be incremented by 4");

    // Verify IP was loaded from stack
    assert_eq!(
        cpu.ip, 0x00001234,
        "IP should be loaded from stack as 32-bit"
    );
}
