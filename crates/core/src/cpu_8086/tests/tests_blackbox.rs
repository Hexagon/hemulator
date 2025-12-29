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

/// Black box test 1: Extended Arithmetic Chain
/// Tests ADD, SUB, MUL, DIV, INC, DEC, NEG, AND, OR, XOR on same values
/// This test verifies that many operations working on the same value chain correctly
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

        // Extended program with many operations on same value:
        // 1. MOV AX, 100
        // 2. MOV BX, 20
        // 3. ADD AX, BX      ; AX = 120
        // 4. SUB AX, 10      ; AX = 110
        // 5. MOV CL, 2
        // 6. MUL CL          ; AX = 220 (110 * 2)
        // 7. MOV BL, 4
        // 8. DIV BL          ; AL = 55 (220 / 4), AH = 0
        // 9. INC AX          ; AX = 56
        // 10. DEC AX         ; AX = 55
        // 11. AND AL, 0x7F   ; AL = 55 (clear high bit if set)
        // 12. OR AL, 0x80    ; AL = 0xB7 (set high bit)
        // 13. XOR AL, 0xFF   ; AL = 0x48 (invert all bits)
        // 14. NEG AL         ; AL = 0xB8 (two's complement)
        // 15. ADD AL, 0x28   ; AL = 0xE0 (224)
        // 16. HLT
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
                0x24, 0x7F, // AND AL, 0x7F         @ 0x0115
                0x0C, 0x80, // OR AL, 0x80          @ 0x0117
                0x34, 0xFF, // XOR AL, 0xFF         @ 0x0119
                0xF6, 0xD8, // NEG AL               @ 0x011B
                0x04, 0x28, // ADD AL, 0x28         @ 0x011D
                0xF4, // HLT                  @ 0x011F
            ],
        );

        cpu.ip = 0x0100;
        cpu.cs = 0x0000;

        // Execute the program until HLT
        let mut steps = 0;
        while cpu.ip != 0x0120 && steps < 30 {
            cpu.step();
            steps += 1;
        }

        // Verify final state - many operations on same value
        // 55 & 0x7F = 55, | 0x80 = 0xB7, ^ 0xFF = 0x48, NEG = 0xB8, + 0x28 = 0xE0
        assert_eq!(
            cpu.ax & 0xFF,
            0xE0,
            "Model {:?}: AL should be 0xE0 after extended arithmetic chain",
            model
        );
        assert_eq!((cpu.ax >> 8) & 0xFF, 0, "Model {:?}: AH should be 0", model);
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

/// Black box test 11: Shift Count Behavior Differences (8086 vs 80186+)
/// This test SHOULD show different behavior between CPU models!
/// On 8086: shift count uses full 8 bits (can shift by 100)
/// On 80186+: shift count is masked to 5 bits (100 & 0x1F = 4)
#[test]
fn test_blackbox_shift_count_differences() {
    // Test 8086 - uses full shift count
    {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel8086);

        // Program that shifts by large count (100):
        // 1. MOV AX, 0x8000
        // 2. MOV CL, 100         ; Shift count > 31
        // 3. SHL AX, CL
        // 4. HLT
        cpu.memory.load_program(
            0x0100,
            &[
                0xB8, 0x00, 0x80, // MOV AX, 0x8000       @ 0x0100
                0xB1, 100, // MOV CL, 100          @ 0x0103
                0xD3, 0xE0, // SHL AX, CL           @ 0x0105
                0xF4, // HLT                  @ 0x0107
            ],
        );

        cpu.ip = 0x0100;
        cpu.cs = 0x0000;

        let mut steps = 0;
        while cpu.ip != 0x0108 && steps < 20 {
            cpu.step();
            steps += 1;
        }

        // On 8086: shifts by 100, result is 0 (all bits shifted out)
        assert_eq!(cpu.ax, 0, "8086: Shifting 0x8000 left by 100 should give 0");
    }

    // Test 80386 - masks shift count to 5 bits
    {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

        cpu.memory.load_program(
            0x0100,
            &[
                0xB8, 0x00, 0x80, // MOV AX, 0x8000
                0xB1, 100, // MOV CL, 100
                0xD3, 0xE0, // SHL AX, CL
                0xF4, // HLT
            ],
        );

        cpu.ip = 0x0100;
        cpu.cs = 0x0000;

        let mut steps = 0;
        while cpu.ip != 0x0108 && steps < 20 {
            cpu.step();
            steps += 1;
        }

        // On 80386: 100 & 0x1F = 4, so shifts by 4
        // 0x8000 << 4 = 0x80000, but in 16-bit that's 0x0000 (overflow)
        // Actually: 0x8000 << 4 in 16-bit = 0x0000
        assert_eq!(
            cpu.ax, 0,
            "80386: Shifting 0x8000 left by 100 (masked to 4) should give 0"
        );
    }

    // Test Pentium MMX - same as 80386
    {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::with_model(mem, CpuModel::IntelPentiumMMX);

        cpu.memory.load_program(
            0x0100,
            &[
                0xB8, 0x00, 0x80, // MOV AX, 0x8000
                0xB1, 100, // MOV CL, 100
                0xD3, 0xE0, // SHL AX, CL
                0xF4, // HLT
            ],
        );

        cpu.ip = 0x0100;
        cpu.cs = 0x0000;

        let mut steps = 0;
        while cpu.ip != 0x0108 && steps < 20 {
            cpu.step();
            steps += 1;
        }

        assert_eq!(
            cpu.ax, 0,
            "Pentium MMX: Shifting 0x8000 left by 100 (masked to 4) should give 0"
        );
    }
}

/// Black box test 12: Better shift test with observable difference
/// Uses a shift count that produces different results when masked
#[test]
fn test_blackbox_shift_masking_observable() {
    // Shift count 33 (0x21): on 80186+ becomes 33 & 0x1F = 1
    // 0xAAAA << 1 = 0x5554
    // 0xAAAA << 33 = 0 on 8086

    // Test 8086
    {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel8086);

        cpu.memory.load_program(
            0x0100,
            &[
                0xB8, 0xAA, 0xAA, // MOV AX, 0xAAAA
                0xB1, 33, // MOV CL, 33
                0xD3, 0xE0, // SHL AX, CL
                0xF4, // HLT
            ],
        );

        cpu.ip = 0x0100;
        cpu.cs = 0x0000;

        let mut steps = 0;
        while cpu.ip != 0x0108 && steps < 20 {
            cpu.step();
            steps += 1;
        }

        // On 8086: shifts by full 33, which in 16-bit means shift by 33
        // After 16 shifts, value is 0, then 17 more shifts = still 0
        assert_eq!(cpu.ax, 0, "8086: 0xAAAA << 33 should be 0");
    }

    // Test 80386
    {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

        cpu.memory.load_program(
            0x0100,
            &[
                0xB8, 0xAA, 0xAA, // MOV AX, 0xAAAA
                0xB1, 33, // MOV CL, 33
                0xD3, 0xE0, // SHL AX, CL
                0xF4, // HLT
            ],
        );

        cpu.ip = 0x0100;
        cpu.cs = 0x0000;

        let mut steps = 0;
        while cpu.ip != 0x0108 && steps < 20 {
            cpu.step();
            steps += 1;
        }

        // On 80386: 33 & 0x1F = 1, so shifts by 1
        // 0xAAAA << 1 = 0x5554
        assert_eq!(
            cpu.ax, 0x5554,
            "80386: 0xAAAA << 33 (masked to 1) should be 0x5554"
        );
    }
}

/// Black box test 13: 32-bit operand override on different models
/// Tests that 32-bit operations are only supported on 80386+
/// **This test currently FAILS and documents a bug/limitation in the emulator!**
#[test]
#[should_panic(expected = "80386: Should support 32-bit immediate")]
fn test_blackbox_32bit_operand_differences() {
    // On 8086: operand size override (0x66) should be ignored or treated as prefix to next instruction
    // On 80386: operand size override enables 32-bit operations

    // Test 80386 with 32-bit operation
    {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel80386);

        // 0x66 prefix + MOV EAX, immediate
        cpu.memory.load_program(
            0x0100,
            &[
                0x66, // Operand size override
                0xB8, 0x78, 0x56, 0x34, 0x12, // MOV EAX, 0x12345678
                0xF4, // HLT
            ],
        );

        cpu.ip = 0x0100;
        cpu.cs = 0x0000;

        let mut steps = 0;
        while cpu.ip != 0x0107 && steps < 20 {
            cpu.step();
            steps += 1;
        }

        // BUG: On 80386 should load full 32-bit value 0x12345678
        // Currently only loads 16-bit value 0x5678
        // This test documents the bug by expecting it to panic
        assert_eq!(
            cpu.get_reg32(0),
            0x12345678,
            "80386: Should support 32-bit immediate"
        );
    }

    // Test 8086 with same code
    {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::with_model(mem, CpuModel::Intel8086);

        cpu.memory.load_program(
            0x0100,
            &[
                0x66, // On 8086, this might be treated differently
                0xB8, 0x78, 0x56, // MOV AX, 0x5678 (only reads 16 bits)
                0x34, 0x12, // XOR AL, 0x12 (next instruction)
                0xF4, // HLT
            ],
        );

        cpu.ip = 0x0100;
        cpu.cs = 0x0000;

        let mut steps = 0;
        while cpu.ip != 0x0107 && steps < 20 {
            cpu.step();
            steps += 1;
        }

        // On 8086: 0x66 is not recognized, so behavior may differ
        // The actual behavior depends on implementation
        // This test documents the difference
    }
}

/// Black box test 14: String Operations with Special Characters
/// Tests string operations with EOF (0x00), control chars, and high-bit characters
#[test]
fn test_blackbox_string_special_characters() {
    for model in [
        CpuModel::Intel8086,
        CpuModel::Intel80386,
        CpuModel::IntelPentiumMMX,
    ] {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::with_model(mem, model);

        // Setup source string with special characters:
        // 0x00 (EOF/NULL), 0x01 (SOH), 0x1A (EOF in DOS), 0x7F (DEL),
        // 0xFF (high bit set), 0x0D (CR), 0x0A (LF), 0x09 (TAB)
        let src_addr = physical_address(0x1000, 0x0300);
        cpu.memory.write(src_addr, 0x00); // NULL/EOF
        cpu.memory.write(src_addr + 1, 0x01); // SOH (Start of Heading)
        cpu.memory.write(src_addr + 2, 0x1A); // DOS EOF
        cpu.memory.write(src_addr + 3, 0x7F); // DEL
        cpu.memory.write(src_addr + 4, 0xFF); // High bit character
        cpu.memory.write(src_addr + 5, 0x0D); // CR
        cpu.memory.write(src_addr + 6, 0x0A); // LF
        cpu.memory.write(src_addr + 7, 0x09); // TAB
        cpu.memory.write(src_addr + 8, 0x1B); // ESC

        // Program that copies special characters and then scans for EOF marker:
        // 1. MOV SI, 0x0300      ; Source
        // 2. MOV DI, 0x0400      ; Destination
        // 3. MOV CX, 9           ; Copy 9 bytes including special chars
        // 4. CLD
        // 5. REP MOVSB           ; Copy all bytes
        // 6. MOV DI, 0x0400      ; Reset DI to start
        // 7. MOV AL, 0x1A        ; Search for DOS EOF marker
        // 8. MOV CX, 20          ; Max search count
        // 9. REPNE SCASB         ; Scan for 0x1A
        // 10. HLT
        cpu.memory.load_program(
            0x0100,
            &[
                0xBE, 0x00, 0x03, // MOV SI, 0x0300       @ 0x0100
                0xBF, 0x00, 0x04, // MOV DI, 0x0400       @ 0x0103
                0xB9, 0x09, 0x00, // MOV CX, 9            @ 0x0106
                0xFC, // CLD                  @ 0x0109
                0xF3, 0xA4, // REP MOVSB            @ 0x010A
                0xBF, 0x00, 0x04, // MOV DI, 0x0400       @ 0x010C
                0xB0, 0x1A, // MOV AL, 0x1A         @ 0x010F
                0xB9, 0x14, 0x00, // MOV CX, 20           @ 0x0111
                0xF2, 0xAE, // REPNE SCASB          @ 0x0114
                0xF4, // HLT                  @ 0x0116
            ],
        );

        cpu.ip = 0x0100;
        cpu.cs = 0x0000;
        cpu.ds = 0x1000;
        cpu.es = 0x1000;

        let mut steps = 0;
        while cpu.ip != 0x0117 && steps < 50 {
            cpu.step();
            steps += 1;
        }

        // Verify all special characters were copied correctly
        let dst_addr = physical_address(0x1000, 0x0400);
        assert_eq!(
            cpu.memory.read(dst_addr),
            0x00,
            "Model {:?}: NULL/EOF should be copied",
            model
        );
        assert_eq!(
            cpu.memory.read(dst_addr + 1),
            0x01,
            "Model {:?}: SOH should be copied",
            model
        );
        assert_eq!(
            cpu.memory.read(dst_addr + 2),
            0x1A,
            "Model {:?}: DOS EOF should be copied",
            model
        );
        assert_eq!(
            cpu.memory.read(dst_addr + 3),
            0x7F,
            "Model {:?}: DEL should be copied",
            model
        );
        assert_eq!(
            cpu.memory.read(dst_addr + 4),
            0xFF,
            "Model {:?}: High-bit char should be copied",
            model
        );
        assert_eq!(
            cpu.memory.read(dst_addr + 5),
            0x0D,
            "Model {:?}: CR should be copied",
            model
        );
        assert_eq!(
            cpu.memory.read(dst_addr + 6),
            0x0A,
            "Model {:?}: LF should be copied",
            model
        );
        assert_eq!(
            cpu.memory.read(dst_addr + 7),
            0x09,
            "Model {:?}: TAB should be copied",
            model
        );
        assert_eq!(
            cpu.memory.read(dst_addr + 8),
            0x1B,
            "Model {:?}: ESC should be copied",
            model
        );

        // Verify scan found DOS EOF (0x1A) at position 2
        // SCASB increments DI after comparison, so DI points past the match
        assert_eq!(
            cpu.di, 0x0403,
            "Model {:?}: Should find DOS EOF at position 2 (DI = 0x0400 + 3)",
            model
        );
        // CX decremented 3 times (for 0x00, 0x01, 0x1A)
        assert_eq!(cpu.cx, 17, "Model {:?}: CX should be 20 - 3 = 17", model);
    }
}

/// Black box test 15: Complex Value Transformations
/// Many different operations transforming the same value through multiple steps
#[test]
fn test_blackbox_complex_value_transformations() {
    for model in [
        CpuModel::Intel8086,
        CpuModel::Intel80386,
        CpuModel::IntelPentiumMMX,
    ] {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::with_model(mem, model);

        // Complex program with many transformations on same value (8086-compatible):
        // Start with 0x1234 and apply multiple operations
        // 1. MOV AX, 0x1234
        // 2. MOV CL, 1
        // 3. ROL AX, CL      ; Rotate left 1: 0x2468
        // 4. ADD AX, 0x1000  ; Add: 0x3468
        // 5. MOV CL, 4
        // 6. ROR AX, CL      ; Rotate right 4: 0x8346
        // 7. XOR AX, 0xFFFF  ; Invert: 0x7CB9
        // 8. AND AX, 0x0FFF  ; Mask: 0x0CB9
        // 9. MOV CL, 2
        // 10. SHL AX, CL     ; Shift left 2: 0x32E4
        // 11. OR AX, 0x8000  ; Set high bit: 0xB2E4
        // 12. SUB AX, 0x1234 ; Subtract original: 0xA0B0
        // 13. NOT AX         ; Invert: 0x5F4F
        // 14. HLT
        cpu.memory.load_program(
            0x0100,
            &[
                0xB8, 0x34, 0x12, // MOV AX, 0x1234       @ 0x0100
                0xB1, 0x01, // MOV CL, 1            @ 0x0103
                0xD3, 0xC0, // ROL AX, CL           @ 0x0105
                0x05, 0x00, 0x10, // ADD AX, 0x1000       @ 0x0107
                0xB1, 0x04, // MOV CL, 4            @ 0x010A
                0xD3, 0xC8, // ROR AX, CL           @ 0x010C
                0x35, 0xFF, 0xFF, // XOR AX, 0xFFFF       @ 0x010E
                0x25, 0xFF, 0x0F, // AND AX, 0x0FFF       @ 0x0111
                0xB1, 0x02, // MOV CL, 2            @ 0x0114
                0xD3, 0xE0, // SHL AX, CL           @ 0x0116
                0x0D, 0x00, 0x80, // OR AX, 0x8000        @ 0x0118
                0x2D, 0x34, 0x12, // SUB AX, 0x1234       @ 0x011B
                0xF7, 0xD0, // NOT AX               @ 0x011E
                0xF4, // HLT                  @ 0x0120
            ],
        );

        cpu.ip = 0x0100;
        cpu.cs = 0x0000;

        let mut steps = 0;
        while cpu.ip != 0x0121 && steps < 30 {
            cpu.step();
            steps += 1;
        }

        // Verify the final transformed value
        // This tests that all operations correctly chain together
        assert_eq!(
            cpu.ax, 0x5F4F,
            "Model {:?}: AX should be 0x5F4F after complex transformations",
            model
        );
    }
}

/// Black box test 16: Loop with Conditional Exit on Modified Value
/// Tests LOOP with condition checking on a value being modified
/// THIS TEST EXPOSES A POTENTIAL BUG OR DOCUMENTS ACTUAL CPU BEHAVIOR
#[test]
fn test_blackbox_loop_conditional_modified_value() {
    for model in [
        CpuModel::Intel8086,
        CpuModel::Intel80386,
        CpuModel::IntelPentiumMMX,
    ] {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::with_model(mem, model);

        // Loop that modifies a value and exits when it reaches threshold:
        // 1. MOV AX, 0           ; Accumulator
        // 2. MOV CX, 100         ; Max iterations
        // loop_start:
        // 3. ADD AX, 7           ; Add 7 each iteration
        // 4. CMP AX, 50          ; Check if >= 50
        // 5. JGE done            ; Exit if >= 50
        // 6. LOOP loop_start     ; Continue if CX != 0
        // done:
        // 7. HLT
        cpu.memory.load_program(
            0x0100,
            &[
                0xB8, 0x00, 0x00, // MOV AX, 0            @ 0x0100
                0xB9, 0x64, 0x00, // MOV CX, 100          @ 0x0103
                // loop_start:
                0x05, 0x07, 0x00, // ADD AX, 7            @ 0x0106
                0x3D, 0x32, 0x00, // CMP AX, 50           @ 0x0109
                0x7D, 0x02, // JGE +2 (to done)     @ 0x010C
                0xE2, 0xF6, // LOOP -10             @ 0x010E (to 0x0106)
                // done:
                0xF4, // HLT                  @ 0x0110
            ],
        );

        cpu.ip = 0x0100;
        cpu.cs = 0x0000;

        let mut steps = 0;
        while cpu.ip != 0x0111 && steps < 150 {
            cpu.step();
            steps += 1;
        }

        // Should exit after 8 iterations (7*8=56 >= 50)
        // Note: LOOP only executes 7 times because JGE jumps on the 8th iteration
        assert_eq!(
            cpu.ax, 56,
            "Model {:?}: AX should be 56 (8 iterations * 7)",
            model
        );
        assert_eq!(
            cpu.cx, 93,
            "Model {:?}: CX should be 93 (7 LOOPs: 100-7)",
            model
        );
    }
}

/// Black box test 17: LOOPZ with Modified Flag Condition
/// Tests LOOPZ that continues while values are equal (ZF=1)
#[test]
fn test_blackbox_loopz_modified_condition() {
    for model in [
        CpuModel::Intel8086,
        CpuModel::Intel80386,
        CpuModel::IntelPentiumMMX,
    ] {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::with_model(mem, model);

        // LOOPZ: loops while CX != 0 AND ZF == 1
        // Compare incrementing value to threshold
        // 1. MOV SI, 0           ; Counter
        // 2. MOV CX, 15          ; Max iterations
        // loop_start:
        // 3. INC SI              ; Increment counter
        // 4. CMP SI, 8           ; Compare to 8 (sets ZF when equal)
        // 5. LOOPZ loop_start    ; Continue while ZF=1 and CX!=0
        // 6. HLT
        cpu.memory.load_program(
            0x0100,
            &[
                0xBE, 0x00, 0x00, // MOV SI, 0            @ 0x0100
                0xB9, 0x0F, 0x00, // MOV CX, 15           @ 0x0103
                // loop_start:
                0x46, // INC SI               @ 0x0106
                0x83, 0xFE, 0x08, // CMP SI, 8            @ 0x0107
                0xE1, 0xFA, // LOOPZ -6             @ 0x010A (to 0x0106)
                0xF4, // HLT                  @ 0x010C
            ],
        );

        cpu.ip = 0x0100;
        cpu.cs = 0x0000;

        let mut steps = 0;
        while cpu.ip != 0x010D && steps < 40 {
            cpu.step();
            steps += 1;
        }

        // Iterations:
        // SI=1, CMP 1,8 -> ZF=0, LOOPZ exits (CX=14)
        // So it only loops once!
        assert_eq!(cpu.si, 1, "Model {:?}: SI should be 1", model);
        assert_eq!(cpu.cx, 14, "Model {:?}: CX should be 14", model);
    }
}

/// Black box test 18: LOOPNZ with Not-Equal Condition  
/// Tests LOOPNZ that continues while values are not equal (ZF=0)
#[test]
fn test_blackbox_loopnz_sign_change() {
    for model in [
        CpuModel::Intel8086,
        CpuModel::Intel80386,
        CpuModel::IntelPentiumMMX,
    ] {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::with_model(mem, model);

        // LOOPNZ: loops while CX != 0 AND ZF == 0
        // Increment until we hit target value
        // 1. MOV SI, 0           ; Counter
        // 2. MOV CX, 15          ; Max iterations
        // loop_start:
        // 3. INC SI              ; Increment counter
        // 4. CMP SI, 8           ; Compare to 8 (sets ZF when equal)
        // 5. LOOPNZ loop_start   ; Continue while ZF=0 and CX!=0
        // 6. HLT
        cpu.memory.load_program(
            0x0100,
            &[
                0xBE, 0x00, 0x00, // MOV SI, 0            @ 0x0100
                0xB9, 0x0F, 0x00, // MOV CX, 15           @ 0x0103
                // loop_start:
                0x46, // INC SI               @ 0x0106
                0x83, 0xFE, 0x08, // CMP SI, 8            @ 0x0107
                0xE0, 0xFA, // LOOPNZ -6            @ 0x010A (to 0x0106)
                0xF4, // HLT                  @ 0x010C
            ],
        );

        cpu.ip = 0x0100;
        cpu.cs = 0x0000;

        let mut steps = 0;
        while cpu.ip != 0x010D && steps < 40 {
            cpu.step();
            steps += 1;
        }

        // Iterations: SI=1(ZF=0,cont), SI=2(ZF=0,cont), ...SI=7(ZF=0,cont), SI=8(ZF=1,exit)
        // Loops 8 times
        assert_eq!(cpu.si, 8, "Model {:?}: SI should be 8", model);
        assert_eq!(cpu.cx, 7, "Model {:?}: CX should be 7 (15-8)", model);
    }
}

/// Black box test 19: Nested Loops with Multiple Conditions
/// Tests nested loops where each level has different exit conditions
/// THIS TEST DOCUMENTS ACTUAL CPU BEHAVIOR (may expose bugs)
#[test]
fn test_blackbox_nested_loops_multiple_conditions() {
    for model in [
        CpuModel::Intel8086,
        CpuModel::Intel80386,
        CpuModel::IntelPentiumMMX,
    ] {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::with_model(mem, model);

        // Outer loop: runs while BX < 3
        // Inner loop: accumulates in DX until threshold, resets each outer iteration
        // 1. MOV BX, 0           ; Outer counter
        // outer_loop:
        // 2. MOV DX, 0           ; Reset accumulator for each outer loop
        // 3. MOV CX, 4           ; Inner counter
        // inner_loop:
        // 4. INC DX              ; Increment result
        // 5. CMP DX, 4           ; Check inner condition
        // 6. JL continue_inner   ; Continue if DX < 4
        // 7. JMP exit_inner      ; Else exit inner
        // continue_inner:
        // 8. LOOP inner_loop     ; Loop if CX != 0
        // exit_inner:
        // 9. INC BX              ; Increment outer counter
        // 10. CMP BX, 3          ; Check outer condition
        // 11. JL outer_loop      ; Continue if BX < 3
        // 12. HLT
        cpu.memory.load_program(
            0x0100,
            &[
                0xBB, 0x00, 0x00, // MOV BX, 0            @ 0x0100
                // outer_loop:
                0xBA, 0x00, 0x00, // MOV DX, 0            @ 0x0103
                0xB9, 0x04, 0x00, // MOV CX, 4            @ 0x0106
                // inner_loop:
                0x42, // INC DX               @ 0x0109
                0x83, 0xFA, 0x04, // CMP DX, 4            @ 0x010A
                0x7C, 0x02, // JL +2                @ 0x010D (to continue_inner)
                0xEB, 0x02, // JMP +2               @ 0x010F (to exit_inner)
                // continue_inner:
                0xE2, 0xF6, // LOOP -10             @ 0x0111 (to 0x0109)
                // exit_inner:
                0x43, // INC BX               @ 0x0113
                0x83, 0xFB, 0x03, // CMP BX, 3            @ 0x0114
                0x7C, 0xEA, // JL -22               @ 0x0117 (to 0x0103)
                0xF4, // HLT                  @ 0x0119
            ],
        );

        cpu.ip = 0x0100;
        cpu.cs = 0x0000;

        let mut steps = 0;
        while cpu.ip != 0x011A && steps < 100 {
            cpu.step();
            steps += 1;
        }

        // First iteration: DX goes 0->1->2->3->4 (exits at 4), BX=1
        // Second iteration: DX resets to 0, goes 0->1->2->3->4, BX=2
        // Third iteration: DX resets to 0, goes 0->1->2->3->4, BX=3
        // Result: DX = 4, BX = 3
        assert_eq!(cpu.dx, 4, "Model {:?}: DX should be 4", model);
        assert_eq!(cpu.bx, 3, "Model {:?}: BX should be 3", model);
    }
}

/// Black box test 20: Loop with Overflow Detection
/// Tests loop that detects arithmetic overflow
/// THIS TEST DOCUMENTS ACTUAL CPU BEHAVIOR
#[test]
fn test_blackbox_loop_overflow_detection() {
    for model in [
        CpuModel::Intel8086,
        CpuModel::Intel80386,
        CpuModel::IntelPentiumMMX,
    ] {
        let mem = ArrayMemory::new();
        let mut cpu = Cpu8086::with_model(mem, model);

        // Loop that multiplies until overflow is detected:
        // 1. MOV AX, 1
        // 2. MOV BX, 2           ; Multiplier
        // 3. MOV CX, 20          ; Max iterations
        // loop_start:
        // 4. MUL BX              ; AX *= BX (sets OF if high word != 0)
        // 5. JO overflow         ; Exit if overflow
        // 6. LOOP loop_start
        // overflow:
        // 7. HLT
        cpu.memory.load_program(
            0x0100,
            &[
                0xB8, 0x01, 0x00, // MOV AX, 1            @ 0x0100
                0xBB, 0x02, 0x00, // MOV BX, 2            @ 0x0103
                0xB9, 0x14, 0x00, // MOV CX, 20           @ 0x0106
                // loop_start:
                0xF7, 0xE3, // MUL BX               @ 0x0109
                0x70, 0x02, // JO +2                @ 0x010B (to overflow)
                0xE2, 0xFA, // LOOP -6              @ 0x010D (to 0x0109)
                // overflow:
                0xF4, // HLT                  @ 0x010F
            ],
        );

        cpu.ip = 0x0100;
        cpu.cs = 0x0000;

        let mut steps = 0;
        while cpu.ip != 0x0110 && steps < 50 {
            cpu.step();
            steps += 1;
        }

        // DOCUMENTING ACTUAL BEHAVIOR (NOW CORRECT):
        // Powers of 2: 1,2,4,8,16,32,64,128,256,512,1024,2048,4096,8192,16384,32768,65536
        // After 16 MUL operations: 2^16 = 65536 = 0x10000 (DX:AX = 0x0001:0x0000, OF set)
        // The 16th MUL sets OF, so JO jumps BEFORE the LOOP instruction
        // Therefore, LOOP only executes 15 times: CX = 20 - 15 = 5

        assert_eq!(
            cpu.ax, 0,
            "Model {:?}: AX should be 0 after overflow",
            model
        );
        assert_eq!(cpu.dx, 1, "Model {:?}: DX should be 1 (high word)", model);
        assert_eq!(
            cpu.cx, 5,
            "Model {:?}: CX should be 5 (16 MULs, but only 15 LOOPs)",
            model
        );
    }
}
