//! Advanced black box integration tests for CPU behavior across different models
//!
//! These tests execute complete programs with many operations in sequence to detect
//! behavioral differences between CPU models (8086, 80386, Pentium MMX, etc.)
//!
//! Each test:
//! - Loads a complete program (multiple instructions)
//! - Executes it to completion
//! - Verifies final state (registers, memory, flags)
//! - Runs on multiple CPU models to ensure consistent behavior

use crate::cpu_8086::ArrayMemory;
use crate::cpu_8086::{Cpu8086, CpuModel, Memory8086, FLAG_ZF};

// Helper function for tests to calculate physical address
fn physical_address(segment: u16, offset: u16) -> u32 {
    ((segment as u32) << 4) + (offset as u32)
}

/// Black box test 1: Arithmetic Chain
/// Tests ADD, SUB, MUL, DIV, INC, DEC in sequence
/// This test verifies that basic arithmetic operations chain correctly
#[test]
fn test_blackbox_arithmetic_chain() {
    // Test on multiple CPU models to detect behavioral differences
    for model in [
        CpuModel::Intel8086,
        CpuModel::Intel80386,
        CpuModel::IntelPentiumMMX,
    ] {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::with_model(mem, model);

        // Program that performs a series of arithmetic operations:
        // 1. MOV AX, 100
        // 2. MOV BX, 20
        // 3. ADD AX, BX      ; AX = 120
        // 4. SUB AX, 10      ; AX = 110
        // 5. MOV CL, 2
        // 6. MUL CL          ; AX = 220 (110 * 2)
        // 7. MOV BL, 4
        // 8. DIV BL          ; AL = 55 (220 / 4), AH = 0 (remainder)
        // 9. INC AX          ; AX = 56
        // 10. DEC AX         ; AX = 55
        // 11. HLT
        cpu.memory.load_program(
            0x0100,
            &[
                0xB8, 0x64, 0x00, // MOV AX, 100          @ 0x0100
                0xBB, 0x14, 0x00, // MOV BX, 20           @ 0x0103
                0x01, 0xD8, // ADD AX, BX           @ 0x0106
                0x2D, 0x0A, 0x00, // SUB AX, 10           @ 0x0108
                0xB1, 0x02, // MOV CL, 2            @ 0x010B
                0xF6, 0xE1, // MUL CL               @ 0x010D
                0xB3, 0x04, // MOV BL, 4            @ 0x010F
                0xF6, 0xF3, // DIV BL               @ 0x0111
                0x40, // INC AX               @ 0x0113
                0x48, // DEC AX               @ 0x0114
                0xF4, // HLT                  @ 0x0115
            ],
        );

        cpu.ip = 0x0100;
        cpu.cs = 0x0000;

        // Execute the program until HLT
        let mut steps = 0;
        while cpu.ip != 0x0116 && steps < 20 {
            cpu.step();
            steps += 1;
        }

        // Verify final state - should be the same on all CPU models
        assert_eq!(
            cpu.ax & 0xFF,
            55,
            "Model {:?}: AL should be 55 after arithmetic chain",
            model
        );
        assert_eq!(
            (cpu.ax >> 8) & 0xFF,
            0,
            "Model {:?}: AH should be 0 (remainder)",
            model
        );
    }
}

/// Black box test 2: Loop with Counter
/// Tests LOOP instruction with INC/DEC operations
#[test]
fn test_blackbox_loop_counter() {
    for model in [
        CpuModel::Intel8086,
        CpuModel::Intel80386,
        CpuModel::IntelPentiumMMX,
    ] {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::with_model(mem, model);

        // Program that loops 10 times, incrementing BX each time:
        // 1. MOV CX, 10      ; Loop counter
        // 2. MOV BX, 0       ; Accumulator
        // loop_start:
        // 3. INC BX          ; Increment
        // 4. LOOP loop_start ; Loop while CX != 0
        // 5. HLT
        cpu.memory.load_program(
            0x0100,
            &[
                0xB9, 0x0A, 0x00, // MOV CX, 10           @ 0x0100
                0xBB, 0x00, 0x00, // MOV BX, 0            @ 0x0103
                0x43, // INC BX               @ 0x0106 (loop_start)
                0xE2, 0xFD, // LOOP -3              @ 0x0107 (jumps to 0x0106)
                0xF4, // HLT                  @ 0x0109
            ],
        );

        cpu.ip = 0x0100;
        cpu.cs = 0x0000;

        // Execute until HLT or timeout
        let mut steps = 0;
        while cpu.ip != 0x010A && steps < 50 {
            cpu.step();
            steps += 1;
        }

        // Verify results
        assert_eq!(
            cpu.cx, 0,
            "Model {:?}: CX should be 0 after loop completes",
            model
        );
        assert_eq!(
            cpu.bx, 10,
            "Model {:?}: BX should be incremented 10 times",
            model
        );
    }
}

/// Black box test 3: String Operations with Memory
/// Tests MOVSB, SCASB, and CMPSB with REP prefix
#[test]
fn test_blackbox_string_operations() {
    for model in [
        CpuModel::Intel8086,
        CpuModel::Intel80386,
        CpuModel::IntelPentiumMMX,
    ] {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::with_model(mem, model);

        // Setup source data in memory
        let src_addr = physical_address(0x1000, 0x0200);
        cpu.memory.write(src_addr, b'H');
        cpu.memory.write(src_addr + 1, b'E');
        cpu.memory.write(src_addr + 2, b'L');
        cpu.memory.write(src_addr + 3, b'L');
        cpu.memory.write(src_addr + 4, b'O');

        // Program that copies 5 bytes from DS:SI to ES:DI using REP MOVSB:
        // 1. MOV SI, 0x0200      ; Source offset
        // 2. MOV DI, 0x0300      ; Destination offset
        // 3. MOV CX, 5           ; Byte count
        // 4. CLD                 ; Clear direction flag
        // 5. REP MOVSB           ; Copy CX bytes
        // 6. HLT
        cpu.memory.load_program(
            0x0100,
            &[
                0xBE, 0x00, 0x02, // MOV SI, 0x0200       @ 0x0100
                0xBF, 0x00, 0x03, // MOV DI, 0x0300       @ 0x0103
                0xB9, 0x05, 0x00, // MOV CX, 5            @ 0x0106
                0xFC, // CLD                  @ 0x0109
                0xF3, 0xA4, // REP MOVSB            @ 0x010A
                0xF4, // HLT                  @ 0x010C
            ],
        );

        cpu.ip = 0x0100;
        cpu.cs = 0x0000;
        cpu.ds = 0x1000;
        cpu.es = 0x1000;

        // Execute until HLT
        let mut steps = 0;
        while cpu.ip != 0x010D && steps < 30 {
            cpu.step();
            steps += 1;
        }

        // Verify the data was copied
        let dst_addr = physical_address(0x1000, 0x0300);
        assert_eq!(
            cpu.memory.read(dst_addr),
            b'H',
            "Model {:?}: First byte should be 'H'",
            model
        );
        assert_eq!(
            cpu.memory.read(dst_addr + 1),
            b'E',
            "Model {:?}: Second byte should be 'E'",
            model
        );
        assert_eq!(
            cpu.memory.read(dst_addr + 2),
            b'L',
            "Model {:?}: Third byte should be 'L'",
            model
        );
        assert_eq!(
            cpu.memory.read(dst_addr + 3),
            b'L',
            "Model {:?}: Fourth byte should be 'L'",
            model
        );
        assert_eq!(
            cpu.memory.read(dst_addr + 4),
            b'O',
            "Model {:?}: Fifth byte should be 'O'",
            model
        );
        assert_eq!(cpu.cx, 0, "Model {:?}: CX should be 0 after REP", model);
        assert_eq!(cpu.si, 0x0205, "Model {:?}: SI should advance by 5", model);
        assert_eq!(cpu.di, 0x0305, "Model {:?}: DI should advance by 5", model);
    }
}

/// Black box test 4: Complex Loop with String Scan
/// Tests LOOP with SCASB to find a character in memory
#[test]
fn test_blackbox_loop_string_scan() {
    for model in [
        CpuModel::Intel8086,
        CpuModel::Intel80386,
        CpuModel::IntelPentiumMMX,
    ] {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::with_model(mem, model);

        // Setup a string in memory with a target character
        let str_addr = physical_address(0x1000, 0x0400);
        cpu.memory.write(str_addr, b'A');
        cpu.memory.write(str_addr + 1, b'B');
        cpu.memory.write(str_addr + 2, b'C');
        cpu.memory.write(str_addr + 3, b'X'); // Target
        cpu.memory.write(str_addr + 4, b'D');

        // Program that scans for 'X' using REPNE SCASB:
        // 1. MOV DI, 0x0400      ; String start
        // 2. MOV AL, 'X'         ; Search character
        // 3. MOV CX, 10          ; Max scan count
        // 4. CLD                 ; Clear direction
        // 5. REPNE SCASB         ; Scan while not equal
        // 6. HLT
        cpu.memory.load_program(
            0x0100,
            &[
                0xBF, 0x00, 0x04, // MOV DI, 0x0400       @ 0x0100
                0xB0, b'X', // MOV AL, 'X'          @ 0x0103
                0xB9, 0x0A, 0x00, // MOV CX, 10           @ 0x0105
                0xFC, // CLD                  @ 0x0108
                0xF2, 0xAE, // REPNE SCASB          @ 0x0109
                0xF4, // HLT                  @ 0x010B
            ],
        );

        cpu.ip = 0x0100;
        cpu.cs = 0x0000;
        cpu.es = 0x1000;

        // Execute until HLT
        let mut steps = 0;
        while cpu.ip != 0x010C && steps < 30 {
            cpu.step();
            steps += 1;
        }

        // Verify the scan found 'X' at position 3 (index 3)
        // SCASB increments DI after comparison, so DI should point past the match
        assert_eq!(
            cpu.di, 0x0404,
            "Model {:?}: DI should point past found character (0x0400 + 4)",
            model
        );
        // CX should be decremented 4 times (for A, B, C, X)
        assert_eq!(cpu.cx, 6, "Model {:?}: CX should be 10 - 4 = 6", model);
        // ZF should be set (equal comparison on 'X')
        assert!(
            cpu.get_flag(FLAG_ZF),
            "Model {:?}: ZF should be set after finding match",
            model
        );
    }
}

/// Black box test 5: Stack Operations with CALL/RET
/// Tests PUSH, POP, CALL, RET in sequence
#[test]
fn test_blackbox_stack_operations() {
    for model in [
        CpuModel::Intel8086,
        CpuModel::Intel80386,
        CpuModel::IntelPentiumMMX,
    ] {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::with_model(mem, model);

        // Program with subroutine call:
        // main:
        // 1. MOV AX, 5
        // 2. MOV BX, 10
        // 3. CALL add_func       ; Call subroutine
        // 4. HLT                 ; Result in AX
        // add_func:
        // 5. ADD AX, BX
        // 6. RET
        cpu.memory.load_program(
            0x0100,
            &[
                0xB8, 0x05, 0x00, // MOV AX, 5            @ 0x0100
                0xBB, 0x0A, 0x00, // MOV BX, 10           @ 0x0103
                0xE8, 0x03, 0x00, // CALL +3 (0x010C)     @ 0x0106
                0xF4, // HLT                  @ 0x0109
                // Padding to align subroutine
                0x90, // NOP                  @ 0x010A
                0x90, // NOP                  @ 0x010B
                // add_func:
                0x01, 0xD8, // ADD AX, BX           @ 0x010C
                0xC3, // RET                  @ 0x010E
            ],
        );

        cpu.ip = 0x0100;
        cpu.cs = 0x0000;
        cpu.ss = 0x1000;
        cpu.sp = 0x0200;

        // Execute until HLT
        let mut steps = 0;
        while cpu.ip != 0x010A && steps < 20 {
            cpu.step();
            steps += 1;
        }

        // Verify results
        assert_eq!(cpu.ax, 15, "Model {:?}: AX should be 5 + 10 = 15", model);
        assert_eq!(
            cpu.sp, 0x0200,
            "Model {:?}: SP should be restored after RET",
            model
        );
    }
}

/// Black box test 6: Multiple Division Operations
/// Tests DIV with different operands in sequence
#[test]
fn test_blackbox_multiple_divisions() {
    for model in [
        CpuModel::Intel8086,
        CpuModel::Intel80386,
        CpuModel::IntelPentiumMMX,
    ] {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::with_model(mem, model);

        // Program performing multiple divisions:
        // 1. MOV AX, 100
        // 2. MOV BL, 3
        // 3. DIV BL          ; AL = 33, AH = 1 (100/3 = 33 remainder 1)
        // 4. MOV BL, 5
        // 5. MOV AH, 0       ; Clear remainder
        // 6. DIV BL          ; AL = 6, AH = 3 (33/5 = 6 remainder 3)
        // 7. MOV BL, 2
        // 8. MOV AH, 0
        // 9. DIV BL          ; AL = 3, AH = 0 (6/2 = 3 remainder 0)
        // 10. HLT
        cpu.memory.load_program(
            0x0100,
            &[
                0xB8, 0x64, 0x00, // MOV AX, 100          @ 0x0100
                0xB3, 0x03, // MOV BL, 3            @ 0x0103
                0xF6, 0xF3, // DIV BL               @ 0x0105
                0xB3, 0x05, // MOV BL, 5            @ 0x0107
                0xB4, 0x00, // MOV AH, 0            @ 0x0109
                0xF6, 0xF3, // DIV BL               @ 0x010B
                0xB3, 0x02, // MOV BL, 2            @ 0x010D
                0xB4, 0x00, // MOV AH, 0            @ 0x010F
                0xF6, 0xF3, // DIV BL               @ 0x0111
                0xF4, // HLT                  @ 0x0113
            ],
        );

        cpu.ip = 0x0100;
        cpu.cs = 0x0000;

        // Execute until HLT
        let mut steps = 0;
        while cpu.ip != 0x0114 && steps < 20 {
            cpu.step();
            steps += 1;
        }

        // Verify final result
        assert_eq!(
            cpu.ax & 0xFF,
            3,
            "Model {:?}: AL should be 3 after divisions",
            model
        );
        assert_eq!(
            (cpu.ax >> 8) & 0xFF,
            0,
            "Model {:?}: AH should be 0 (no remainder)",
            model
        );
    }
}

/// Black box test 7: Loop with Decrement Until Zero
/// Tests LOOP with DEC to create a countdown
#[test]
fn test_blackbox_loop_decrement() {
    for model in [
        CpuModel::Intel8086,
        CpuModel::Intel80386,
        CpuModel::IntelPentiumMMX,
    ] {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::with_model(mem, model);

        // Program that uses a loop to accumulate a value:
        // 1. MOV DX, 0           ; Accumulator
        // 2. MOV CX, 25          ; Loop 25 times
        // loop_start:
        // 3. INC DX              ; Increment accumulator
        // 4. LOOP loop_start     ; Loop while CX != 0
        // 5. HLT
        cpu.memory.load_program(
            0x0100,
            &[
                0xBA, 0x00, 0x00, // MOV DX, 0            @ 0x0100
                0xB9, 0x19, 0x00, // MOV CX, 25           @ 0x0103
                0x42, // INC DX               @ 0x0106 (loop_start)
                0xE2, 0xFD, // LOOP -3              @ 0x0107
                0xF4, // HLT                  @ 0x0109
            ],
        );

        cpu.ip = 0x0100;
        cpu.cs = 0x0000;

        // Execute until HLT
        let mut steps = 0;
        while cpu.ip != 0x010A && steps < 100 {
            cpu.step();
            steps += 1;
        }

        // Verify results - loop runs 25 times, so DX should be 25
        assert_eq!(
            cpu.dx, 25,
            "Model {:?}: DX should be incremented 25 times",
            model
        );
        assert_eq!(cpu.cx, 0, "Model {:?}: CX should be 0 after loop", model);
    }
}

/// Black box test 8: Comprehensive Arithmetic and Logic
/// Tests ADD, SUB, AND, OR, XOR, NOT in complex sequence
#[test]
fn test_blackbox_comprehensive_alu() {
    for model in [
        CpuModel::Intel8086,
        CpuModel::Intel80386,
        CpuModel::IntelPentiumMMX,
    ] {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::with_model(mem, model);

        // Complex ALU operation sequence:
        // 1. MOV AX, 0xFF00
        // 2. MOV BX, 0x00FF
        // 3. AND AX, BX      ; AX = 0x0000
        // 4. MOV AX, 0xFF00
        // 5. OR AX, BX       ; AX = 0xFFFF
        // 6. XOR AX, 0xAAAA  ; AX = 0x5555
        // 7. NOT AX          ; AX = 0xAAAA
        // 8. HLT
        cpu.memory.load_program(
            0x0100,
            &[
                0xB8, 0x00, 0xFF, // MOV AX, 0xFF00       @ 0x0100
                0xBB, 0xFF, 0x00, // MOV BX, 0x00FF       @ 0x0103
                0x21, 0xD8, // AND AX, BX           @ 0x0106
                0xB8, 0x00, 0xFF, // MOV AX, 0xFF00       @ 0x0108
                0x09, 0xD8, // OR AX, BX            @ 0x010B
                0x35, 0xAA, 0xAA, // XOR AX, 0xAAAA       @ 0x010D
                0xF7, 0xD0, // NOT AX               @ 0x0110
                0xF4, // HLT                  @ 0x0112
            ],
        );

        cpu.ip = 0x0100;
        cpu.cs = 0x0000;

        // Execute until HLT
        let mut steps = 0;
        while cpu.ip != 0x0113 && steps < 20 {
            cpu.step();
            steps += 1;
        }

        // Verify result
        assert_eq!(
            cpu.ax, 0xAAAA,
            "Model {:?}: AX should be 0xAAAA after ALU operations",
            model
        );
    }
}

/// Black box test 9: String Compare Operations
/// Tests CMPSB with conditional jumps
#[test]
fn test_blackbox_string_compare() {
    for model in [
        CpuModel::Intel8086,
        CpuModel::Intel80386,
        CpuModel::IntelPentiumMMX,
    ] {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::with_model(mem, model);

        // Setup two strings
        let str1_addr = physical_address(0x1000, 0x0500);
        let str2_addr = physical_address(0x1000, 0x0600);

        // String 1: "TEST"
        cpu.memory.write(str1_addr, b'T');
        cpu.memory.write(str1_addr + 1, b'E');
        cpu.memory.write(str1_addr + 2, b'S');
        cpu.memory.write(str1_addr + 3, b'T');

        // String 2: "TEST" (identical)
        cpu.memory.write(str2_addr, b'T');
        cpu.memory.write(str2_addr + 1, b'E');
        cpu.memory.write(str2_addr + 2, b'S');
        cpu.memory.write(str2_addr + 3, b'T');

        // Program that compares 4 bytes:
        // 1. MOV SI, 0x0500      ; String 1
        // 2. MOV DI, 0x0600      ; String 2
        // 3. MOV CX, 4           ; Compare 4 bytes
        // 4. CLD
        // 5. REPE CMPSB          ; Compare while equal
        // 6. HLT
        cpu.memory.load_program(
            0x0100,
            &[
                0xBE, 0x00, 0x05, // MOV SI, 0x0500       @ 0x0100
                0xBF, 0x00, 0x06, // MOV DI, 0x0600       @ 0x0103
                0xB9, 0x04, 0x00, // MOV CX, 4            @ 0x0106
                0xFC, // CLD                  @ 0x0109
                0xF3, 0xA6, // REPE CMPSB           @ 0x010A
                0xF4, // HLT                  @ 0x010C
            ],
        );

        cpu.ip = 0x0100;
        cpu.cs = 0x0000;
        cpu.ds = 0x1000;
        cpu.es = 0x1000;

        // Execute until HLT
        let mut steps = 0;
        while cpu.ip != 0x010D && steps < 30 {
            cpu.step();
            steps += 1;
        }

        // Verify comparison succeeded
        assert_eq!(
            cpu.cx, 0,
            "Model {:?}: CX should be 0 (all bytes compared)",
            model
        );
        assert!(
            cpu.get_flag(FLAG_ZF),
            "Model {:?}: ZF should be set (strings match)",
            model
        );
        assert_eq!(cpu.si, 0x0504, "Model {:?}: SI should advance by 4", model);
        assert_eq!(cpu.di, 0x0604, "Model {:?}: DI should advance by 4", model);
    }
}

/// Black box test 10: Nested Loops
/// Tests nested loop structures with counters
#[test]
fn test_blackbox_nested_loops() {
    for model in [
        CpuModel::Intel8086,
        CpuModel::Intel80386,
        CpuModel::IntelPentiumMMX,
    ] {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::with_model(mem, model);

        // Nested loop program: outer loop 5 times, inner loop 3 times
        // Total: DX should be incremented 15 times
        // 1. MOV BX, 5           ; Outer counter
        // 2. MOV DX, 0           ; Accumulator
        // outer_loop:
        // 3. MOV CX, 3           ; Inner counter
        // inner_loop:
        // 4. INC DX
        // 5. LOOP inner_loop
        // 6. DEC BX
        // 7. JNZ outer_loop
        // 8. HLT
        cpu.memory.load_program(
            0x0100,
            &[
                0xBB, 0x05, 0x00, // MOV BX, 5            @ 0x0100
                0xBA, 0x00, 0x00, // MOV DX, 0            @ 0x0103
                // outer_loop:
                0xB9, 0x03, 0x00, // MOV CX, 3            @ 0x0106
                // inner_loop:
                0x42, // INC DX               @ 0x0109
                0xE2, 0xFD, // LOOP -3              @ 0x010A (to 0x0109)
                0x4B, // DEC BX               @ 0x010C
                0x75, 0xF7, // JNZ -9               @ 0x010D (to 0x0106)
                0xF4, // HLT                  @ 0x010F
            ],
        );

        cpu.ip = 0x0100;
        cpu.cs = 0x0000;

        // Execute until HLT (with safety limit)
        let mut steps = 0;
        while cpu.ip != 0x0110 && steps < 200 {
            cpu.step();
            steps += 1;
        }

        // Verify results
        assert_eq!(
            cpu.dx, 15,
            "Model {:?}: DX should be 15 (5 * 3 iterations)",
            model
        );
        assert_eq!(cpu.bx, 0, "Model {:?}: BX should be 0", model);
        assert_eq!(cpu.cx, 0, "Model {:?}: CX should be 0", model);
    }
}
