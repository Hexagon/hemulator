# Zilog Z80 CPU Reference

## Overview

The Zilog Z80 is an 8-bit microprocessor designed as an enhanced and compatible successor to the Intel 8080. It was widely used in home computers, arcade games, and embedded systems throughout the 1980s and beyond. This implementation provides a reusable Z80 CPU core.

**Implementation**: `crates/core/src/cpu_z80.rs`

## Architecture

### Registers

The Z80 has two complete sets of general-purpose registers that can be swapped:

#### Main Register Set
- **A** (Accumulator): Primary register for arithmetic and logic operations
- **F** (Flags): Processor status flags
- **B, C**: 8-bit registers (can be paired as BC)
- **D, E**: 8-bit registers (can be paired as DE)
- **H, L**: 8-bit registers (can be paired as HL for memory addressing)

#### Shadow Register Set
- **A', F'**: Shadow accumulator and flags
- **B', C'**: Shadow BC pair
- **D', E'**: Shadow DE pair
- **H', L'**: Shadow HL pair

#### Index Registers
- **IX**: 16-bit index register (can access IXH, IXL as 8-bit)
- **IY**: 16-bit index register (can access IYH, IYL as 8-bit)

#### Special Purpose Registers
- **SP** (Stack Pointer): 16-bit pointer to stack
- **PC** (Program Counter): 16-bit pointer to current instruction
- **I** (Interrupt Vector): 8-bit interrupt page address
- **R** (Memory Refresh): 7-bit refresh counter
- **IFF1, IFF2**: Interrupt enable flip-flops

### Register Pairs

Main registers can be accessed as 16-bit pairs:
- **AF**: A (high byte) and F (low byte)
- **BC**: B (high byte) and C (low byte)
- **DE**: D (high byte) and E (low byte)
- **HL**: H (high byte) and L (low byte)

### Flags Register (F)

```
SZ5H3PNC
││││││││
││││││││└─ Carry flag (C)
│││││││└── Add/Subtract flag (N) - for BCD
││││││└─── Parity/Overflow flag (P/V)
│││││└──── Undocumented flag (bit 3)
││││└───── Half Carry flag (H)
│││└────── Undocumented flag (bit 5)
││└─────── Zero flag (Z)
│└──────── Sign flag (S)
```

## Usage

Systems using the Z80 must implement the `MemoryZ80` trait:

```rust
pub trait MemoryZ80 {
    fn read(&self, addr: u16) -> u8;
    fn write(&mut self, addr: u16, val: u8);
    fn port_in(&mut self, port: u8) -> u8;
    fn port_out(&mut self, port: u8, val: u8);
}
```

### Example

```rust
use emu_core::cpu_z80::{CpuZ80, MemoryZ80};

struct MySystem {
    ram: [u8; 65536],
}

impl MemoryZ80 for MySystem {
    fn read(&self, addr: u16) -> u8 {
        self.ram[addr as usize]
    }
    
    fn write(&mut self, addr: u16, val: u8) {
        self.ram[addr as usize] = val;
    }
    
    fn port_in(&mut self, port: u8) -> u8 {
        0 // System-specific I/O
    }
    
    fn port_out(&mut self, port: u8, val: u8) {
        // System-specific I/O
    }
}

let system = MySystem { ram: [0; 65536] };
let mut cpu = CpuZ80::new(system);
cpu.reset();
```

## Instruction Set

The Z80 includes all 8080 instructions plus many enhancements.

### 8080-Compatible Instructions

All 8080 instructions are supported (see [cpu_8080.md](cpu_8080.md) for details).

### Z80 Enhancements to 8080 Instructions

Many 8080 instructions have Z80 variants:
- **LD**: Replaces MOV, MVI, etc. (more consistent syntax)
- Enhanced versions of arithmetic and logical operations
- Additional register combinations

### New Z80 Instructions

#### Register Exchange
- **EXX**: Exchange main and shadow register sets (BC, DE, HL)
- **EX AF,AF'**: Exchange AF with shadow AF'
- **EX DE,HL**: Exchange DE and HL
- **EX (SP),HL**: Exchange top of stack with HL
- **EX (SP),IX**: Exchange top of stack with IX
- **EX (SP),IY**: Exchange top of stack with IY

#### Block Operations
- **LDI**: Load and increment (HL) → (DE), HL++, DE++, BC--
- **LDIR**: Load, increment and repeat until BC=0
- **LDD**: Load and decrement
- **LDDR**: Load, decrement and repeat
- **CPI**: Compare and increment
- **CPIR**: Compare, increment and repeat (search)
- **CPD**: Compare and decrement
- **CPDR**: Compare, decrement and repeat

#### Block I/O
- **INI**: Input and increment
- **INIR**: Input, increment and repeat
- **IND**: Input and decrement
- **INDR**: Input, decrement and repeat
- **OUTI**: Output and increment
- **OTIR**: Output, increment and repeat
- **OUTD**: Output and decrement
- **OTDR**: Output, decrement and repeat

#### Bit Operations
- **BIT n,r**: Test bit n of register r
- **SET n,r**: Set bit n of register r
- **RES n,r**: Reset (clear) bit n of register r

#### Relative Jumps
- **JR**: Unconditional relative jump
- **JR cc**: Conditional relative jump (NZ, Z, NC, C)
- **DJNZ**: Decrement B and jump if not zero

#### Index Register Operations
- IX and IY can be used in place of HL for most instructions
- **LD IX,nn**: Load IX immediate
- **LD IY,nn**: Load IY immediate
- **ADD IX,rr**: Add register pair to IX
- **INC/DEC IX/IY**: Increment/decrement index registers
- Indexed addressing: `LD A,(IX+d)`, `LD (IY+d),n`

#### Interrupt Mode
- **IM 0**: 8080-compatible mode
- **IM 1**: Jump to $0038
- **IM 2**: Vectored interrupts using I register

#### Misc New Instructions
- **NEG**: Negate accumulator (two's complement)
- **RLD**: Rotate left decimal (nibble rotation)
- **RRD**: Rotate right decimal
- **LD A,I**: Load A from I register
- **LD A,R**: Load A from R register
- **LD I,A**: Load I from A
- **LD R,A**: Load R from A
- **RETI**: Return from interrupt (for daisy chain)
- **RETN**: Return from NMI

## Instruction Prefixes

Z80 uses prefixes to extend the instruction set:

- **$CB**: Bit operations prefix
- **$DD**: IX index register prefix
- **$ED**: Extended instruction prefix
- **$FD**: IY index register prefix

Prefixes can be combined: `$DD $CB displacement opcode` for bit operations on (IX+d).

## Addressing Modes

The Z80 supports all 8080 addressing modes plus:

6. **Indexed**: `LD A,(IX+d)`, `LD (IY+d),n`
7. **Relative**: `JR offset`, `DJNZ offset`
8. **Bit Addressing**: `BIT n,(HL)`, `SET n,(IX+d)`

## Interrupts

The Z80 has a sophisticated interrupt system:

### Interrupt Modes

#### Mode 0 (IM 0)
- 8080-compatible
- Interrupting device provides instruction (usually RST)

#### Mode 1 (IM 1)
- Simple mode
- All interrupts jump to $0038
- Most commonly used

#### Mode 2 (IM 2)
- Vectored interrupts
- I register provides high byte of vector table
- Interrupting device provides low byte
- Allows 128 different interrupt vectors

### Non-Maskable Interrupt (NMI)

- Cannot be disabled
- Always jumps to $0066
- IFF1 copied to IFF2 for later restoration
- Use RETN to return

### Interrupt Enable

- **IFF1**: Main interrupt enable
- **IFF2**: Temporary storage (used during NMI)
- **EI**: Enable interrupts (sets IFF1 and IFF2)
- **DI**: Disable interrupts (clears IFF1 and IFF2)

## Timing

Z80 instructions use M-cycles (machine cycles) and T-states (clock cycles):
- 1 M-cycle = 3-6 T-states (typically 4)
- Instructions: 1-6 M-cycles (4-23 T-states typical)

Common timings:
- Simple register operations: 4 T-states
- Memory access: 7-11 T-states
- I/O operations: 11-16 T-states
- Block operations: Variable (depend on BC)

## Special Features

### Shadow Registers

Fast context switching using EXX and EX AF,AF':
```assembly
EXX           ; Swap BC, DE, HL with shadows
EX AF,AF'     ; Swap AF with shadow
; Now using shadow set
EXX           ; Restore main registers
EX AF,AF'
```

### Block Operations

Efficient memory operations:
```assembly
LD HL,source
LD DE,dest
LD BC,length
LDIR          ; Copy BC bytes from (HL) to (DE)
```

### Index Registers

Alternative to HL for data structures:
```assembly
LD IX,table
LD A,(IX+5)   ; Access table[5]
INC (IX+10)   ; Increment table[10]
```

### Decimal Arithmetic

BCD support via DAA and RLD/RRD:
- **DAA**: Decimal adjust after addition/subtraction
- **RLD/RRD**: Rotate BCD digits

## Differences from 8080

**Enhancements:**
- IX/IY index registers
- Shadow register set
- Block move/search operations
- Relative jumps
- Bit manipulation instructions
- Three interrupt modes
- Improved instruction mnemonics

**Timing:**
- Different cycle counts for some instructions
- More consistent timing

**Compatibility:**
- All 8080 code runs on Z80
- Z80 code won't run on 8080

## Undocumented Features

The Z80 has several undocumented features:

### Undocumented Flags
- Bit 3 and 5 of F are affected by some operations
- Various instructions set these flags in specific ways

### Undocumented Instructions
- IX and IY high/low byte access (IXH, IXL, IYH, IYL)
- Various combinations of prefixes
- Some illegal opcodes have defined behavior

### WZ Register
- Internal 16-bit register
- Affects undocumented flags
- Used for address calculations

## Systems That Could Use Z80

While not currently used directly in Hemulator, this CPU core could be used for:
- ZX Spectrum
- MSX computers
- Sega Master System
- Game Boy (uses LR35902, a Z80 derivative)
- CP/M systems
- Many arcade games

## Implementation Notes

### Refresh Register (R)

The R register is incremented after each instruction fetch. It was originally used for dynamic RAM refresh but is often used as a pseudo-random number generator.

### Interrupt Response

In IM 2, the interrupt vector is formed by:
```
Vector = (I << 8) | (device_byte & 0xFE)
```

The least significant bit is masked to 0, so vectors must be at even addresses.

### Block Instruction Timing

Block instructions (LDIR, CPIR, etc.) repeat until a condition is met or BC reaches zero. Total cycles depend on the number of iterations.

## References

- [Z80 User Manual](http://www.zilog.com/docs/z80/um0080.pdf) - Official Zilog documentation
- [The Undocumented Z80 Documented](http://www.myquest.nl/z80undocumented/) - Comprehensive undocumented features guide
- [Z80 Instruction Set](http://www.z80.info/z80oplist.txt) - Complete opcode listing
- [Z80 Timing](http://www.z80.info/z80time.txt) - Detailed timing information
