# 65c816 operations referenc

**Highly unofficial, just for cross-referencing the cpu implementation**

### **1. Emulator State Definitions**

**Registers:**

* **Accumulator (A/C):** 16-bit (Access as `A` for low byte, `B` for high byte, or `C` for full 16-bit).
* **Index Registers (X, Y):** 16-bit (can be switched to 8-bit).
* **Stack Pointer (SP):** 16-bit (Unlike 6502 which was fixed to page 1).
* **Program Counter (PC):** 16-bit.
* **Program Bank (K or PBR):** 8-bit (The high byte of the 24-bit PC address).
* **Data Bank (B or DBR):** 8-bit (The high byte for absolute addressing).
* **Direct Page (D):** 16-bit (Replaces 6502 "Zero Page". Can be moved anywhere in the first 64k).

**Processor Status (P) - The Flag Register:**
The meaning of the flags changes depending on the **E (Emulation)** bit.

**Native Mode (E=0):**


* **N, V, D, I, Z, C:** Same as 6502.
* **M (Memory/Accumulator Select):** 1 = 8-bit A, 0 = 16-bit A.
* **X (Index Select):** 1 = 8-bit X/Y, 0 = 16-bit X/Y.

**Emulation Mode (E=1):**
Mimics the 6502 exactly. The M and X bits are lost (forced to 1 effectively) and replaced by the Break flag logic in stack pushes.

---

### **2. New Addressing Modes**

The 65816 introduces **24-bit addressing** (Bank:Address).

| Mode | Syntax | Description |
| --- | --- | --- |
| **Absolute Long** | `LDA $123456` | 24-bit address. Uses valid Bank byte. |
| **Absolute Long Indexed** | `LDA $123456,X` | 24-bit address + X. |
| **Stack Relative** | `LDA $02,S` | Address = SP + $02. |
| **Stack Rel. Indirect Indexed** | `LDA ($02,S),Y` | Indirect address from stack + Y. |
| **Direct Page Indirect Long** | `LDA [$10]` | 24-bit pointer at D+$10. |
| **Block Move** | `MVN $01,$02` | Move memory block (Src Bank, Dest Bank). |

---

### **3. Mode Switching Instructions**

These are the most critical instructions for valid 65816 emulation.

| Mnemonic | Description | Notes |
| --- | --- | --- |
| **REP** `#$xx` | **Reset** Status Bits | Clears bits in P. `REP #$20` sets A to 16-bit (Clears M). |
| **SEP** `#$xx` | **Set** Status Bits | Sets bits in P. `SEP #$30` sets A/X/Y to 8-bit. |
| **XCE** | Exchange Carry & Emulation | **The only way to switch Native/Emulation mode.** |

*Note: Changing M/X flags immediately truncates registers if switching 16->8 bit. The high bytes are preserved in hidden latches or lost depending on the specific register.*

---

### **4. New Data Movement & Arithmetic**

| Mnemonic | Description | Notes |
| --- | --- | --- |
| **MVN** `src,dest` | Block Move Negative | Decrements addresses. Src/Dest are **Banks**. Uses C=Count-1. |
| **MVP** `src,dest` | Block Move Positive | Increments addresses. Src/Dest are **Banks**. |
| **PEA** `addr` | Push Effective Address | Pushes 16-bit constant. |
| **PEI** `(dp)` | Push Effective Indirect | Pushes 16-bit value from Direct Page. |
| **PER** `addr` | Push Effective PC Relative | Pushes 16-bit value (PC + offset). |
| **TXY, TYX** | Transfer X <-> Y |  |
| **TCS, TSC** | Transfer C (Accum) <-> SP | 16-bit transfer. |
| **TCD, TDC** | Transfer C (Accum) <-> Direct Page |  |
| **XYC** | Exchange Y and C | Swaps 16-bit Y with 16-bit Accumulator. |

---

### **5. New Stack & Bank Control**

| Mnemonic | Description | Notes |
| --- | --- | --- |
| **PHB / PLB** | Push / Pop Data Bank (B) | Changes the default bank for Absolute addressing. |
| **PHK** | Push Program Bank (K) | There is **no PLK**. Change K via `RTL` or `JML`. |
| **PHD / PLD** | Push / Pop Direct Page (D) | Save/Restore the DP register. |
| **PHX / PHY** | Push X / Push Y |  |
| **PLX / PLY** | Pop X / Pop Y |  |

---

### **6. New Branch & Flow Control**

| Mnemonic | Description | Notes |
| --- | --- | --- |
| **BRA** | Branch Always | Unconditional branch (8-bit offset). |
| **BRL** | Branch Long | Unconditional branch (16-bit offset). |
| **JML** | Jump Long | `JMP` to 24-bit address. Changes K register. |
| **JSL** | Jump Subroutine Long | Pushes K, then PC (24-bit return address). |
| **RTL** | Return from Long | Pops PC, then K. |

---

### **7. System Control**

| Mnemonic | Description | Notes |
| --- | --- | --- |
| **WAI** | Wait for Interrupt | Halts CPU. **Low power.** Resumes on IRQ/NMI/Reset. |
| **STP** | Stop Processor | Halts CPU. Clock stops. Requires **Reset** to wake. |
| **COP** | Co-Processor | Acts like `BRK` but uses vector `$FFF4` (Native). |

---

### **8. Critical Implementation Details ("Gotchas")**

#### **A. 8-bit vs 16-bit Handling (M and X Flags)**

You must dynamically adjust instruction behavior based on M and X.

* **If M=0 (16-bit A):** Memory fetches for `LDA`, `ADC`, etc., read **2 bytes** (Low, High). PC increases by extra byte. T-States increase by 1.
* **If X=0 (16-bit X/Y):** `LDX`, `LDY` read **2 bytes**.
* **Immediate Mode:** `LDA #$00` is 2 bytes long if M=1. It is **3 bytes long** if M=0. **This changes the instruction stream parsing.**

#### **B. Bank Wrapping vs. Page Wrapping**

* **Page Wrapping (Old 6502):** If D=$00FF and you read `(D),Y`, it might wrap inside the direct page (Zero Page).
* **Bank Wrapping (65816):**
* **Direct Page:** Does **not** wrap at page boundary. $FFFF + 1 wraps to $0000 (Bank 0).
* **Absolute:** $xxFFFF + 1 wraps to $xx0000 (Stays in same bank).
* **Program Counter (PC):** Wraps within the current Program Bank (K). Executing past $FFFF wraps to $0000 in the *same* bank. You must use `JML`/`JSL`/`RTL` to cross banks.


#### **C. Emulation Mode (E=1) Constraints**

When `E=1`, the CPU **must** behave like a 6502.

* Stack is forced to `$01xx` (High byte of SP is forced to $01).
* Direct Page is forced to `$0000` (High byte of D is forced to $00).
* Interrupt vectors are fetched from Bank 0 `$FFxx`.
* M and X flags are forced to 1 (8-bit mode).

#### **D. Direct Page Register (D)**

D is a 16-bit value added to the operand.

* If `D = $1000`, `LDA $10` reads from address `$1010`.
* Crucially, **cycles are added** if `D` is not page-aligned (i.e., lower byte of D is not $00). This is a common performance penalty in SNES games.

#### **E. MVN and MVP (Block Moves)**

These are interruptible instructions.

* If an IRQ fires during `MVN`, the PC is **not** advanced. The instruction repeats after the IRQ handler returns (decremented counters in A/X/Y allow it to resume where it left off).
* Format: `Opcode | Dest Bank | Src Bank`. (Note order).

#### **F. V-Flag in 16-bit Mode**

The Overflow (V) calculation works the same, but must operate on the MSB (Bit 15) when in 16-bit mode.

```c
// Pseudo-code for dynamic width
width = (P & FLAG_M) ? 8 : 16;
mask = (width == 8) ? 0xFF : 0xFFFF;
sign_bit = (width == 8) ? 0x80 : 0x8000;

```
