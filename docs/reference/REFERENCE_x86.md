# Intel x86 operations

**Highly unofficial, just for cross-referencing the cpu implementation**

This list is optimized for checking implementation gaps in your source code.

### Legend

* **r/m**: Register or Memory operand.
* **imm**: Immediate value (constant).
* **src/dst**: Source / Destination.
* **st(i)**: FPU Stack Register (0-7). `st(0)` is the Top of Stack.
* **IP**: Modifies Instruction Pointer directly (Jump/Call).
* **Fl**: Modifies CPU Status Flags (ZF, CF, OF, SF, PF).
* **m16/32/64/80**: Memory operand size in bits.

---

# Intel x86 Instruction Set Reference (8086 - Pentium MMX)

## 1. Integer Core: Data Transfer

*Fundamental addressing and data movement. Verify Segment Registers (CS, DS, ES, SS, FS, GS) logic.*

| Mnemonic | Operands | Size | CPU | Emulator Notes |
| --- | --- | --- | --- | --- |
| **MOV** | r/m, r/m/imm | 8/16/32 | 8086 | No flags affected. Handle Sreg moves carefully (protection faults). |
| **PUSH** | r/m/imm/sreg | 16/32 | 8086 | Decs SP/ESP. **186+** allows `PUSH imm`. |
| **POP** | r/m/sreg | 16/32 | 8086 | Incs SP/ESP. `POP CS` is illegal (except early 8088 bugs). |
| **XCHG** | r/m, r | 8/16/32 | 8086 | Atomic (implicit LOCK) if operand is memory. |
| **XLAT** | - | 8 | 8086 | Table lookup: `AL = [DS:BX + unsigned AL]`. |
| **IN / OUT** | port | 8/16/32 | 8086 | I/O address space. Use `DX` for ports > 255. |
| **LEA** | r, m | 16/32 | 8086 | Calc effective address only. **Does not access memory.** |
| **LDS/LES** | r, m | 32/48 | 8086 | Load Far Pointer. |
| **LFS/LGS/LSS** | r, m | 32/48 | 386 | Load Far Pointer (FS/GS/SS). |
| **BSWAP** | r32 | 32 | 486 | Byte Swap (Endianness conversion). |
| **MOVZX/SX** | r, r/m | 16/32 | 386 | Zero-Extend / Sign-Extend. Essential for casting. |
| **CMOVcc** | r, r/m | 16/32 | P6* | *Conditional Move. Technically P6, but supported by late Socket 7 CPUs.* |

## 2. Integer Core: ALU (Arithmetic & Logic)

*Source of most bugs. Pay close attention to Flag updates (Overflow vs Carry).*

| Mnemonic | Operands | Size | CPU | Emulator Notes |
| --- | --- | --- | --- | --- |
| **ADD/ADC** | dst, src | 8/16/32 | 8086 | ADC includes CF. **Fl:** All status flags. |
| **SUB/SBB** | dst, src | 8/16/32 | 8086 | SBB subtracts CF. **Fl:** All status flags. |
| **INC/DEC** | r/m | 8/16/32 | 8086 | **Fl:** **Does NOT affect Carry Flag (CF).** Crucial! |
| **CMP** | r/m, r/m/imm | 8/16/32 | 8086 | Non-destructive SUB. Only updates **Fl**. |
| **NEG** | r/m | 8/16/32 | 8086 | Two's complement (`0 - x`). |
| **MUL/IMUL** | r/m | 8/16/32 | 8086 | Unsigned/Signed. Affects AX/DX/EDX. **186+** adds 3-op `IMUL`. |
| **DIV/IDIV** | r/m | 8/16/32 | 8086 | Divides `(E)AX:(E)DX` by operand. **Trap #DE** if div by 0. |
| **AND/OR/XOR** | r/m, r/m/imm | 8/16/32 | 8086 | **Fl:** Clears CF and OF. Updates ZF, SF, PF. |
| **TEST** | r/m, r/m/imm | 8/16/32 | 8086 | Non-destructive AND. Updates **Fl**. |
| **NOT** | r/m | 8/16/32 | 8086 | 1's complement. Affects **NO** flags. |
| **SHL/SHR** | r/m, cl/imm | 8/16/32 | 8086 | Logical Shift. **186+** allows immediate != 1. |
| **SAL/SAR** | r/m, cl/imm | 8/16/32 | 8086 | Arithmetic Shift (SAR preserves sign bit). |
| **ROL/ROR** | r/m, cl/imm | 8/16/32 | 8086 | Rotate. |
| **RCL/RCR** | r/m, cl/imm | 8/16/32 | 8086 | Rotate through Carry Flag. |
| **SHLD/SHRD** | r/m, r, imm | 16/32 | 386 | Double precision shift (across two registers). |
| **XADD** | r/m, r | 8/16/32 | 486 | Atomic exchange + add. |
| **CMPXCHG** | r/m, r | 8/16/32 | 486 | Compare and Exchange. (Pentium adds `CMPXCHG8B`). |
| **BCD Ops** | DAA, DAS, etc. | 8 | 8086 | *Legacy*. Decimal Adjust. Often implemented incorrectly. |

## 3. Integer Core: Control Flow

*Directly modifies IP/EIP.*

| Mnemonic | Operands | CPU | Emulator Notes |
| --- | --- | --- | --- |
| **JMP** | rel/r/m | 8086 | **IP update.** Short/Near/Far types. Verify absolute vs relative logic. |
| **CALL** | rel/r/m | 8086 | Pushes return address (IP or CS:IP), then Jumps. |
| **RET / RETF** | imm? | 8086 | Pops IP (and CS if Far). Optional imm added to SP (stdcall). |
| **Jcc** | rel | 8086 | Conditional Jump (JE, JNE, JG, etc). Checks **Fl**. **386+** adds near conditional. |
| **LOOP/x** | rel8 | 8086 | Decs (E)CX. Jumps if (E)CX!=0 (and Z-flag check for LOOPE/NE). |
| **INT n** | imm8 | 8086 | Software Interrupt. Pushes Flags, CS, IP. Vectors via IDT/IVT. |
| **IRET** | - | 8086 | Return from Interrupt. Pops IP, CS, Flags. Handles Task Switch in Protected Mode. |

## 4. Integer Core: String Operations

*Usually prefixed with `REP`, `REPE`, `REPNE`. Checks `DF` (Direction Flag).*

| Mnemonic | Operation | CPU | Emulator Notes |
| --- | --- | --- | --- |
| **MOVS** | `[ES:DI] = [DS:SI]` | 8086 | Inc/Dec SI/DI based on operand size. |
| **CMPS** | `CMP [DS:SI], [ES:DI]` | 8086 | Compare memory. |
| **SCAS** | `CMP Acc, [ES:DI]` | 8086 | Compare Accumulator (AL/AX/EAX) with memory. |
| **LODS** | `Acc = [DS:SI]` | 8086 | Load memory to Accumulator. |
| **STOS** | `[ES:DI] = Acc` | 8086 | Store Accumulator to memory. |

## 5. System & Protected Mode

*Essential for OS booting.*

| Mnemonic | Description | CPU | Emulator Notes |
| --- | --- | --- | --- |
| **LGDT/LIDT** | Load GDT/IDT Register | 286 | Reads 6 bytes (limit + base) from memory. |
| **LLDT/LTR** | Load LDT/Task Register | 286 | Selector loads. |
| **MOV CRn, r** | Move to/from Control Reg | 386 | CR0 (PE, PG), CR3 (Page Dir), CR4. Triggers mode switches. |
| **LMSW** | Load Machine Status Word | 286 | Precursor to CR0 modification. |
| **CLTS** | Clear Task Switched Flag | 286 | Used in FPU context switching logic. |
| **CPUID** | Processor ID | Pent | Returns features in EAX/EBX/ECX/EDX. |
| **RDTSC** | Read Time Stamp | Pent | 64-bit cycle count to EDX:EAX. |

---

## 6. FPU (x87 Floating Point)

Operates on 80-bit stack `ST(0)`...`ST(7)`. Distinct from Integer registers.

**Critical:** FPU Status Word (SW) contains condition codes (C0-C3).

### Data Transfer

| Mnemonic | Operands | Notes |
| --- | --- | --- |
| **FLD** | m32/64/80 / st(i) | Push Value to Stack. (Converts int/float to 80-bit ext). |
| **FST / FSTP** | m32/64/80 / st(i) | Store (Copy) / Store & Pop. |
| **FILD** | m16/32/64 | Load Integer (convert to float) & Push. |
| **FIST / P** | m16/32/64 | Store Integer (convert float to int). |
| **FXCH** | st(i) | Swap ST(0) with ST(i). |
| **FBLD / FBSTP** | m80 | Load/Store BCD (Decimal) 80-bit. |

### Arithmetic

*Most instructions have a `P` variant (e.g., `FADDP`) which pops the stack after op.*
| Mnemonic | Description | Notes |
| :--- | :--- | :--- |
| **FADD / FSUB** | Add / Subtract | `st(0) += src`. Watch for NaNs and Infinities. |
| **FMUL / FDIV** | Multiply / Divide | `st(0) *= src`. Handle #Z (Divide by Zero). |
| **FPREM / 1** | Partial Remainder | IEEE 754 remainder. Important for trig reduction. |
| **FABS** | `st(0) = abs(st(0))` | Clears sign bit. |
| **FCHS** | `st(0) = -st(0)` | Inverts sign bit. |
| **FRNDINT** | Round to Integer | Uses RC (Rounding Control) field in Control Word. |
| **FSCALE** | `st(0) * 2^st(1)` | Fast multiplication by power of 2. |
| **FSQRT** | Square Root | |

### Comparison & Control

| Mnemonic | Description | Emulator Notes |
| --- | --- | --- |
| **FCOM / P / PP** | Compare | Sets C0, C2, C3 in Status Word. **Does not set CPU Flags.** |
| **FCOMI / P** | Compare | Sets **CPU Flags** (ZF, PF, CF) directly (Pentium Pro+). |
| **FSTSW** | Store Status Word | Usually `FSTSW AX`. Used to move FPU conditions to CPU flags (via `SAHF`). |
| **FINIT** | Initialize FPU | Reset Control/Status words, tags to Empty. |
| **FLDCW / FSTCW** | Load/Store Control Word | Sets Rounding Mode, Precision, Exception Masks. |
| **FWAIT** | Wait | Checks for pending FPU exceptions. |

### Transcendental (Trig)

*Hard to emulate perfectly bit-exact due to internal microcode variations.*
| Mnemonic | Description |
| :--- | :--- |
| **FSIN / FCOS** | Sine / Cosine of ST(0). |
| **FSINCOS** | Computes both. Pushes Cos, then Sin. |
| **FPTAN** | Partial Tangent. |
| **FPATAN** | Partial Arctangent. |
| **FYL2X** | `st(1) * log2(st(0))` |
| **FYL2XP1** | `st(1) * log2(st(0) + 1)` |

### Constants

| Mnemonic | Value Pushed |
| --- | --- |
| **FLDZ / FLD1** | 0.0 / 1.0 |
| **FLDPI** |  |
| **FLDL2T / FLDL2E** |  /  |
| **FLDLG2 / FLDLN2** |  /  |

---

## 7. MMX (Pentium MMX)

SIMD operations. Registers **MM0-MM7** are aliased to FPU **ST0-ST7** (lower 64 bits).

**Warning:** Mixing MMX and FPU instructions without `EMMS` corrupts data.

| Mnemonic | Operands | Operation |
| --- | --- | --- |
| **EMMS** | - | **Empty MMX State.** Sets FPU tags to empty. Must be called before returning to Float code. |
| **MOVD** | mm, r/m32 | Move 32-bit (Doubleword). |
| **MOVQ** | mm, mm/m64 | Move 64-bit (Quadword). |
| **PACKSS/US** | mm, mm/m64 | Pack with Signed/Unsigned Saturation (e.g., 16-bit -> 8-bit). |
| **PUNPCKH/L** | mm, mm/m64 | Unpack (Interleave) High/Low data. |
| **PADD/SUB** | mm, mm/m64 | Parallel Add/Sub. Wraps around on overflow. |
| **PADDS/US** | mm, mm/m64 | Parallel Add/Sub with **Saturation** (clamps to min/max). |
| **PMULL/H** | mm, mm/m64 | Parallel Multiply (stores Low or High bits of result). |
| **PMADDWD** | mm, mm/m64 | Multiply-Add (Dot product backbone). |
| **PAND/OR/XOR** | mm, mm/m64 | Bitwise logical operations. |
| **PCMPGT/EQ** | mm, mm/m64 | Parallel Compare. Result is mask of 1s (True) or 0s (False). |
| **PSLL/SRL/SRA** | mm, imm/x | Shift Packed Data (Logical Left/Right, Arithmetic Right). |

---

### Emulator Implementation Tip: The `ModR/M` Byte

For most integer and MMX instructions, immediately following the opcode is the **ModR/M byte**. You must parse this correctly to identify the operands.

* **Format:** `[Mod (2 bits)] [Reg/Opcode (3 bits)] [R/M (3 bits)]`
* **Mod 00:** `[reg]` (Memory)
* **Mod 01:** `[reg + disp8]` (Memory)
* **Mod 10:** `[reg + disp32]` (Memory)
* **Mod 11:** `reg` (Direct Register)

*Note: The `SIB` (Scale Index Base) byte follows ModR/M if `R/M == 100` in 32-bit mode.*