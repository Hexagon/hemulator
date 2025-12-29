# Intel 8080 operations

**Highly unofficial, just for cross-referencing the cpu implementation**

### **1. Emulator State Definitions**

Before implementing instructions, ensure your CPU state definitions match the hardware reality.

**Registers:**

* **8-bit:** `A` (Accumulator), `B`, `C`, `D`, `E`, `H`, `L`.
* **16-bit:** `PC` (Program Counter), `SP` (Stack Pointer).
* **Virtual Register M:** `M` represents the memory byte at address `(H << 8) | L`.

**Flags (The F Register):**
The 8080 Status Word (PSW) is an 8-bit register. The bits are defined as:


* **Bit 7 (S):** Sign (1 if bit 7 of result is 1).
* **Bit 6 (Z):** Zero (1 if result is 0).
* **Bit 5:** Not used (Always 0).
* **Bit 4 (AC):** Auxiliary Carry (Carry out of bit 3). Used for `DAA`.
* **Bit 3:** Not used (Always 0).
* **Bit 2 (P):** Parity (1 if result has even parity/even number of 1s).
* **Bit 1:** Not used (Always 1).
* **Bit 0 (C):** Carry (1 if operation produced carry/borrow).

---

### **2. Legend for Tables**

* **r, r1, r2:** 8-bit register (A, B, C, D, E, H, L, M).
* **rp:** 16-bit register pair (BC, DE, HL, SP).
* **d8:** Immediate 8-bit data.
* **d16:** Immediate 16-bit data (Little Endian in memory: Low byte first).
* **addr:** 16-bit memory address.
* **Flags:** `Z` (Zero), `S` (Sign), `P` (Parity), `CY` (Carry), `AC` (Aux Carry).
* **T-States:** Number of clock cycles required.

---

### **3. Data Transfer Group**

*Note: Data transfer instructions generally **DO NOT** affect flags.*

| Mnemonic | Operands | Description | Flags | T-States | Notes |
| --- | --- | --- | --- | --- | --- |
| **MOV** | `r1`, `r2` | Move `r2` to `r1` | None | 5 | If `r` is `M`, T=7. |
| **MVI** | `r`, `d8` | Move immediate `d8` to `r` | None | 7 | If `r` is `M`, T=10. |
| **LXI** | `rp`, `d16` | Load `rp` with `d16` | None | 10 |  |
| **LDA** | `addr` | Load A from `addr` | None | 13 |  |
| **STA** | `addr` | Store A to `addr` | None | 13 |  |
| **LHLD** | `addr` | Load L from `addr`, H from `addr+1` | None | 16 |  |
| **SHLD** | `addr` | Store L to `addr`, H to `addr+1` | None | 16 |  |
| **LDAX** | `rp` | Load A from address in `BC` or `DE` | None | 7 | Only BC or DE allowed. |
| **STAX** | `rp` | Store A to address in `BC` or `DE` | None | 7 | Only BC or DE allowed. |
| **XCHG** | - | Exchange `DE` and `HL` pairs | None | 4 |  |

---

### **4. Arithmetic Group**

*Note: Unless noted, these affect all flags (Z, S, P, CY, AC).* 

| Mnemonic | Operands | Description | Flags | T-States | Notes |
| --- | --- | --- | --- | --- | --- |
| **ADD** | `r` | A = A + `r` | All | 4 | If `r` is `M`, T=7. |
| **ADI** | `d8` | A = A + `d8` | All | 7 |  |
| **ADC** | `r` | A = A + `r` + CY | All | 4 | If `r` is `M`, T=7. |
| **ACI** | `d8` | A = A + `d8` + CY | All | 7 |  |
| **SUB** | `r` | A = A - `r` | All | 4 | If `r` is `M`, T=7. |
| **SUI** | `d8` | A = A - `d8` | All | 7 |  |
| **SBB** | `r` | A = A - `r` - CY | All | 4 | If `r` is `M`, T=7. |
| **SBI** | `d8` | A = A - `d8` - CY | All | 7 |  |
| **INR** | `r` | `r` = `r` + 1 | Z, S, P, AC | 5 | **CY not affected**. If `M`, T=10. |
| **DCR** | `r` | `r` = `r` - 1 | Z, S, P, AC | 5 | **CY not affected**. If `M`, T=10. |
| **INX** | `rp` | `rp` = `rp` + 1 | **None** | 5 | 16-bit operation. No flags. |
| **DCX** | `rp` | `rp` = `rp` - 1 | **None** | 5 | 16-bit operation. No flags. |
| **DAD** | `rp` | HL = HL + `rp` | **CY Only** | 10 | 16-bit add. S, Z, P, AC not changed. |
| **DAA** | - | Decimal Adjust Accumulator | All | 4 | See specific section below. |

---

### **5. Logical Group**

| Mnemonic | Operands | Description | Flags | T-States | Notes |
| --- | --- | --- | --- | --- | --- |
| **ANA** | `r` | A = A & `r` | Z, S, P, AC, CY | 4 | CY=0. AC is set on some 8080s, cleared on others. Intel manual implies AC mirrors logical OR of bit 3. If `M`, T=7. |
| **ANI** | `d8` | A = A & `d8` | Z, S, P, AC, CY | 7 | CY=0. AC behavior same as ANA. |
| **ORA** | `r` | A = A | `r` | Z, S, P, AC, CY | 4 | CY=0, AC=0. If `M`, T=7. |
| **ORI** | `d8` | A = A | `d8` | Z, S, P, AC, CY | 7 | CY=0, AC=0. |
| **XRA** | `r` | A = A ^ `r` | Z, S, P, AC, CY | 4 | CY=0, AC=0. If `M`, T=7. |
| **XRI** | `d8` | A = A ^ `d8` | Z, S, P, AC, CY | 7 | CY=0, AC=0. |
| **CMP** | `r` | Compare A and `r` (A - `r`) | All | 4 | A is not modified. Only flags set. If `M`, T=7. |
| **CPI** | `d8` | Compare A and `d8` | All | 7 |  |
| **RLC** | - | Rotate A Left | CY Only | 4 | Bit 7 goes to CY and Bit 0. |
| **RRC** | - | Rotate A Right | CY Only | 4 | Bit 0 goes to CY and Bit 7. |
| **RAL** | - | Rotate A Left thru Carry | CY Only | 4 | Bit 7->CY, old CY->Bit 0. |
| **RAR** | - | Rotate A Right thru Carry | CY Only | 4 | Bit 0->CY, old CY->Bit 7. |
| **CMA** | - | Complement A (NOT) | None | 4 | No flags affected. |
| **CMC** | - | Complement Carry Flag | CY Only | 4 |  |
| **STC** | - | Set Carry Flag | CY Only | 4 | CY = 1. |

---

### **6. Branch Control Group**

*Note: Conditional branches check the status of the flags. If the condition is met, PC is updated.*

**Conditions (cc):**

* `NZ` (Not Zero), `Z` (Zero)
* `NC` (No Carry), `C` (Carry)
* `PO` (Parity Odd), `PE` (Parity Even)
* `P` (Plus/Positive, S=0), `M` (Minus/Negative, S=1)

| Mnemonic | Operands | Description | Flags | T-States | Notes |
| --- | --- | --- | --- | --- | --- |
| **JMP** | `addr` | Jump Unconditional | None | 10 |  |
| **Jcc** | `addr` | Jump Conditional | None | 10 |  |
| **CALL** | `addr` | Call Unconditional | None | 17 | Pushes PC to Stack. |
| **Ccc** | `addr` | Call Conditional | None | 11/17 | 11 if false, 17 if true. |
| **RET** | - | Return | None | 10 | Pops PC from Stack. |
| **Rcc** | - | Return Conditional | None | 5/11 | 5 if false, 11 if true. |
| **RST** | `n` | Restart (Call to `n*8`) | None | 11 | `n` is 0-7. Pushes PC. |
| **PCHL** | - | Move HL to PC | None | 5 | Effective jump to address in HL. |

---

### **7. Stack, I/O, and Machine Control**

| Mnemonic | Operands | Description | Flags | T-States | Notes |
| --- | --- | --- | --- | --- | --- |
| **PUSH** | `rp` | Push `rp` onto Stack | None | 11 | `SP` decreases by 2. |
| **PUSH PSW** | - | Push A and Flags | None | 11 | See Flag Register definition. |
| **POP** | `rp` | Pop Stack into `rp` | None | 10 | `SP` increases by 2. |
| **POP PSW** | - | Pop Flags and A | All | 10 | Overwrites flags from stack byte. |
| **XTHL** | - | Exchange Stack Top with HL | None | 18 | `L` <-> `(SP)`, `H` <-> `(SP+1)` |
| **SPHL** | - | Move HL to SP | None | 5 |  |
| **IN** | `port` | Input from port to A | None | 10 |  |
| **OUT** | `port` | Output A to port | None | 10 |  |
| **EI** | - | Enable Interrupts | None | 4 | Takes effect after *next* instruction. |
| **DI** | - | Disable Interrupts | None | 4 |  |
| **HLT** | - | Halt Processor | None | 7 | Stops until interrupt/reset. |
| **NOP** | - | No Operation | None | 4 |  |

---

### **8. Critical Implementation Details (The "Gotchas")**

To pass standard test suites (like `CPUTEST.COM`), implement these exactly:

#### **A. Parity Flag (P)**

The 8080 Parity flag is set if the result has **Even Parity** (an even number of 1s).

* `00000011` -> 2 ones (Even) -> P = 1
* `00000001` -> 1 one (Odd) -> P = 0

#### **B. Auxiliary Carry (AC)**

This is the carry from bit 3 to bit 4. It is essential for `DAA`.


In C code, you can calculate it as:
`AC = ((operand1 & 0x0F) + (operand2 & 0x0F)) > 0x0F`

#### **C. Decimal Adjust Accumulator (DAA)**

This is the most difficult instruction to emulate correctly. It adjusts the accumulator to valid Binary Coded Decimal (BCD) after an addition.

**Algorithm:**

1. If `(A & 0x0F) > 9` OR `AC == 1`:
* `A = A + 6`
* `AC = 1` (Update AC based on this addition)


2. If `(A > 0x9F)` OR `CY == 1`:
* `A = A + 0x60`
* `CY = 1`


3. Update `Z`, `S`, `P` flags based on the final `A`.

*Note: DAA is only valid after ADD/ADC instructions on the 8080. It behaves unpredictably after SUB/SBB on real hardware, though standard emulation usually ignores this nuance.*

#### **D. SUB / CMP Carry Flag**

On the 8080, the Carry flag acts as a **Borrow** during subtraction.

* If `A < Operand`, `CY = 1`.
* If `A >= Operand`, `CY = 0`.
* *Note: This is the inverse of the 6502 processor.*
