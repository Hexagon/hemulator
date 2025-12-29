# 6502 operations reference

**Highly unofficial, just for cross-referencing the cpu implementation**

### **1. Emulator State Definitions**

**Registers:**

* **8-bit:** `A` (Accumulator), `X` (Index), `Y` (Index), `S` (Stack Pointer).
* **16-bit:** `PC` (Program Counter).
* **Note:** The Stack is hardcoded to memory page 1 (`$0100` - `$01FF`). The `S` register represents the low byte offset only.

**Flags (The P Register):**
The Processor Status Register is 8 bits.


* **Bit 7 (N):** Negative (Set if bit 7 of result is 1).
* **Bit 6 (V):** Overflow (Signed 2's complement overflow).
* **Bit 5:** Unused (Always read as 1).
* **Bit 4 (B):** Break (See "Gotchas" below – this is complex).
* **Bit 3 (D):** Decimal Mode (1 = BCD math for ADC/SBC).
* **Bit 2 (I):** Interrupt Disable (1 = Disable IRQ).
* **Bit 1 (Z):** Zero (Set if result is 0).
* **Bit 0 (C):** Carry (1 = Carry out / No Borrow).

---

### **2. Addressing Modes & Cycle Counts**

Cycles vary by addressing mode. This table serves as the baseline for the instruction tables.

| Mode | Syntax | Bytes | Base Cycles | Cycle Penalty (+1 if...) |
| --- | --- | --- | --- | --- |
| **Implicit** | `INX` | 1 | 2 | None |
| **Accumulator** | `ROR A` | 1 | 2 | None |
| **Immediate** | `LDA #$10` | 2 | 2 | None |
| **Zero Page** | `LDA $00` | 2 | 3 | None |
| **Zero Page,X** | `LDA $00,X` | 2 | 4 | None |
| **Zero Page,Y** | `LDX $00,Y` | 2 | 4 | None |
| **Relative** | `BNE $10` | 2 | 2 | +1 if branch taken, +2 if page crossed |
| **Absolute** | `LDA $1234` | 3 | 4 | None |
| **Absolute,X** | `LDA $1234,X` | 3 | 4 | +1 if page boundary crossed |
| **Absolute,Y** | `LDA $1234,Y` | 3 | 4 | +1 if page boundary crossed |
| **Indirect** | `JMP ($1234)` | 3 | 5 | None |
| **Indirect X** | `LDA ($00,X)` | 2 | 6 | (Pre-indexed indirect) |
| **Indirect Y** | `LDA ($00),Y` | 2 | 5 | +1 if page boundary crossed (Post-indexed) |

---

### **3. Load/Store Transfer Group**

*Flags affected: N, Z (unless noted)*

| Mnemonic | Description | Modes Available |
| --- | --- | --- |
| **LDA** | Load Accumulator | Imm, ZP, ZP,X, Abs, Abs,X, Abs,Y, (Ind,X), (Ind),Y |
| **LDX** | Load X Register | Imm, ZP, ZP,Y, Abs, Abs,Y |
| **LDY** | Load Y Register | Imm, ZP, ZP,X, Abs, Abs,X |
| **STA** | Store Accumulator | ZP, ZP,X, Abs, Abs,X, Abs,Y, (Ind,X), (Ind),Y |
| **STX** | Store X Register | ZP, ZP,Y, Abs |
| **STY** | Store Y Register | ZP, ZP,X, Abs |
| **TAX** | Transfer A to X | Implicit |
| **TAY** | Transfer A to Y | Implicit |
| **TXA** | Transfer X to A | Implicit |
| **TYA** | Transfer Y to A | Implicit |
| **TSX** | Transfer Stack Ptr to X | Implicit |
| **TXS** | Transfer X to Stack Ptr | Implicit (**No Flags Affected**) |

---

### **4. Arithmetic Group**

*Flags affected: N, Z, C, V (ADC/SBC only)*

| Mnemonic | Description | Modes Available | Notes |
| --- | --- | --- | --- |
| **ADC** | Add with Carry | Imm, ZP, ZP,X, Abs, Abs,X, Abs,Y, (Ind,X), (Ind),Y | Uses D flag. |
| **SBC** | Subtract with Carry | Imm, ZP, ZP,X, Abs, Abs,X, Abs,Y, (Ind,X), (Ind),Y | Uses D flag. |
| **INC** | Increment Memory | ZP, ZP,X, Abs, Abs,X | **Does not touch C** |
| **INX** | Increment X | Implicit | **Does not touch C** |
| **INY** | Increment Y | Implicit | **Does not touch C** |
| **DEC** | Decrement Memory | ZP, ZP,X, Abs, Abs,X | **Does not touch C** |
| **DEX** | Decrement X | Implicit | **Does not touch C** |
| **DEY** | Decrement Y | Implicit | **Does not touch C** |

---

### **5. Shift & Rotate Group**

*Flags affected: N, Z, C*

| Mnemonic | Description | Modes Available | Notes |
| --- | --- | --- | --- |
| **ASL** | Arithmetic Shift Left | Acc, ZP, ZP,X, Abs, Abs,X | Bit 7 -> C, 0 -> Bit 0 |
| **LSR** | Logical Shift Right | Acc, ZP, ZP,X, Abs, Abs,X | 0 -> Bit 7, Bit 0 -> C |
| **ROL** | Rotate Left | Acc, ZP, ZP,X, Abs, Abs,X | C -> Bit 0, Bit 7 -> C |
| **ROR** | Rotate Right | Acc, ZP, ZP,X, Abs, Abs,X | C -> Bit 7, Bit 0 -> C |

---

### **6. Logical Group**

*Flags affected: N, Z*

| Mnemonic | Description | Modes Available | Notes |
| --- | --- | --- | --- |
| **AND** | Logical AND | Imm, ZP, ZP,X, Abs, Abs,X, Abs,Y, (Ind,X), (Ind),Y |  |
| **ORA** | Logical OR | Imm, ZP, ZP,X, Abs, Abs,X, Abs,Y, (Ind,X), (Ind),Y |  |
| **EOR** | Exclusive OR | Imm, ZP, ZP,X, Abs, Abs,X, Abs,Y, (Ind,X), (Ind),Y |  |
| **BIT** | Bit Test | ZP, Abs | **Special Flags:** M7->N, M6->V, Z set if (A & M)==0 |

---

### **7. Branch & Control Group**

| Mnemonic | Description | Condition | Cycles |
| --- | --- | --- | --- |
| **JMP** | Jump | - | 3 (Abs), 5 (Ind) |
| **BCC** | Branch if Carry Clear | C = 0 | 2 (+1 taken, +2 page cross) |
| **BCS** | Branch if Carry Set | C = 1 | " |
| **BEQ** | Branch if Equal | Z = 1 | " |
| **BNE** | Branch if Not Equal | Z = 0 | " |
| **BMI** | Branch if Minus | N = 1 | " |
| **BPL** | Branch if Plus | N = 0 | " |
| **BVC** | Branch if Overflow Clear | V = 0 | " |
| **BVS** | Branch if Overflow Set | V = 1 | " |

---

### **8. Stack & System Group**

| Mnemonic | Description | Modes | Cycles | Notes |
| --- | --- | --- | --- | --- |
| **PHA** | Push Accumulator | Imp | 3 |  |
| **PHP** | Push Processor Status | Imp | 3 | Sets B flag on stack. |
| **PLA** | Pop Accumulator | Imp | 4 | Sets N, Z. |
| **PLP** | Pop Processor Status | Imp | 4 | Loads all flags. |
| **JSR** | Jump to Subroutine | Abs | 6 | Pushes PC+2. |
| **RTS** | Return from Subroutine | Imp | 6 | Pops PC, adds 1. |
| **RTI** | Return from Interrupt | Imp | 6 | Pops P, then PC. |
| **BRK** | Force Interrupt | Imp | 7 | Pushes PC+2, P (with B set). Sets I. |
| **NOP** | No Operation | Imp | 2 |  |
| **CLC/SEC** | Clear/Set Carry | Imp | 2 |  |
| **CLD/SED** | Clear/Set Decimal | Imp | 2 |  |
| **CLI/SEI** | Clear/Set Interrupt | Imp | 2 |  |
| **CLV** | Clear Overflow | Imp | 2 | No "SEV" exists. |

---

### **9. Critical Implementation Details (The "Gotchas")**

The 6502 is famous for quirky hardware behaviors that must be emulated exactly.

#### **A. The "B" Flag (Break)**

The `B` flag **does not exist** in the P register hardware.

* It only exists on the byte pushed to the **Stack**.
* When `PHP` or `BRK` pushes P to the stack, bit 4 is written as **1**.
* When an IRQ/NMI pushes P to the stack, bit 4 is written as **0**.
* The `PLP` and `RTI` instructions ignore bit 4 when popping.

#### **B. ADC and SBC Logic**

Unlike the 8080/Z80, the 6502 `SBC` requires the Carry flag to be **Set** for a standard subtraction (No borrow).

* `ADC`: Result = A + M + C
* `SBC`: Result = A - M - (1 - C)  *(Logic: A + ~M + C)*

**Overflow (V) Calculation:**
For both ADC and SBC, V is set if the sign of the result is incorrect (i.e., Positive + Positive = Negative).

```c
// ADC Example
V = (~(A ^ M) & (A ^ Result) & 0x80) ? 1 : 0;

```

#### **C. Decimal Mode (The D Flag)**

If `D=1`, `ADC` and `SBC` treat values as Binary Coded Decimal.

* *Note:* The NES (Ricoh 2A03) disabled this circuit. If emulating NES, ignore D. For Commodore/Apple, you must implement it.
* **ADC in Decimal:** Adjusts lower nibble if >9, then upper nibble if >9. Affects C and N/Z flags differently depending on exact CPU revision (CMOS vs NMOS), but usually N/Z are calculated on the *binary* result before BCD correction on NMOS 6502s.

#### **D. The Indirect Jump Bug**

The instruction `JMP ($xxFF)` (Indirect jump) has a hardware bug.
It fetches the Low Byte (LSB) from `$xxFF`.
It fetches the High Byte (MSB) from `$xx00`, **NOT** `$xx00 + 1`.

* *Example:* `JMP ($02FF)` reads LSB from `$02FF` and MSB from `$0200`.

#### **E. Page Crossing Penalty**

For Branch instructions (`BNE`, `BEQ`, etc.):

* +1 cycle if the branch is taken.
* +1 **additional** cycle if the branch destination is on a different memory page (High byte of PC changes).
* *Total:* 4 cycles max.


For Indexed instructions (e.g., `LDA $1230,X`):

* +1 cycle if the addition of X/Y causes a carry to the high byte (page cross).
