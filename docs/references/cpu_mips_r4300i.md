# MIPS R4300i CPU Reference

## Overview

The MIPS R4300i is a 64-bit RISC processor used in the Nintendo 64. It combines 64-bit data processing with a 32-bit address space, includes a floating-point coprocessor, and features a five-stage pipeline. This implementation provides a CPU core for N64 emulation.

**Implementation**: `crates/core/src/cpu_mips_r4300i.rs`

## Architecture

### General Purpose Registers

The R4300i has 32 64-bit general-purpose registers:

- **r0-r31**: 64-bit general-purpose registers
  - **r0** ($zero): Always contains zero (writes are ignored)
  - **r1** ($at): Assembler temporary
  - **r2-r3** ($v0-$v1): Function return values
  - **r4-r7** ($a0-$a3): Function arguments
  - **r8-r15** ($t0-$t7): Temporary registers
  - **r16-r23** ($s0-$s7): Saved registers
  - **r24-r25** ($t8-$t9): More temporaries
  - **r26-r27** ($k0-$k1): Kernel reserved
  - **r28** ($gp): Global pointer
  - **r29** ($sp): Stack pointer
  - **r30** ($fp/$s8): Frame pointer / saved register
  - **r31** ($ra): Return address

### Special Registers

- **PC** (Program Counter): 64-bit instruction pointer
- **HI**: High 64 bits of multiply/divide result
- **LO**: Low 64 bits of multiply/divide result

### Coprocessor 0 (System Control)

CP0 contains system control and configuration registers:

- **Index** (r0): TLB entry index
- **Random** (r1): TLB random index
- **EntryLo0/1** (r2/r3): TLB entry low bits
- **Context** (r4): TLB context
- **PageMask** (r5): TLB page mask
- **Wired** (r6): TLB wired entries
- **BadVAddr** (r8): Bad virtual address
- **Count** (r9): Timer count
- **EntryHi** (r10): TLB entry high bits
- **Compare** (r11): Timer compare
- **Status** (r12): Processor status
- **Cause** (r13): Exception cause
- **EPC** (r14): Exception program counter
- **PRId** (r15): Processor revision identifier
- **Config** (r16): Configuration
- **LLAddr** (r17): Load-linked address
- **WatchLo/Hi** (r18/r19): Watchpoint
- **XContext** (r20): Extended context
- **PErr** (r26): Parity error
- **TagLo/Hi** (r28/r29): Cache tag
- **ErrorEPC** (r30): Error exception PC

### Coprocessor 1 (FPU)

CP1 is the floating-point unit with:

- **f0-f31**: 32 floating-point registers (64-bit each)
- **FCR0**: FPU implementation/revision register
- **FCR31**: FPU control/status register

Registers can be accessed as:
- 32 single-precision (32-bit) registers
- 16 double-precision (64-bit) register pairs

### Status Register (CP0.Status)

```
Bits:
  0: IE  - Interrupt Enable
  1: EXL - Exception Level
  2: ERL - Error Level
3-4: KSU - Kernel/Supervisor/User mode
  5: UX  - User 64-bit enable
  6: SX  - Supervisor 64-bit enable
  7: KX  - Kernel 64-bit enable
8-15: Interrupt Mask
16-17: Diagnostic Status
  18: CH  - Cache Hit
  19: CE  - Cache Error
  22: BEV - Bootstrap Exception Vectors
  25: RE  - Reverse Endian
  26: FR  - FPU Register mode (0=32-bit, 1=64-bit)
  27: RP  - Reduced Power
  28: CU0 - Coprocessor 0 Usable
  29: CU1 - Coprocessor 1 Usable (FPU)
```

## Usage

Systems using the R4300i must implement the `MemoryR4300i` trait:

```rust
pub trait MemoryR4300i {
    fn read_word(&self, addr: u32) -> u32;
    fn read_halfword(&self, addr: u32) -> u16;
    fn read_byte(&self, addr: u32) -> u8;
    fn write_word(&mut self, addr: u32, val: u32);
    fn write_halfword(&mut self, addr: u32, val: u16);
    fn write_byte(&mut self, addr: u32, val: u8);
}
```

### Example

```rust
use emu_core::cpu_mips_r4300i::{CpuMipsR4300i, MemoryR4300i};

struct N64System {
    ram: Vec<u8>,
}

impl MemoryR4300i for N64System {
    fn read_word(&self, addr: u32) -> u32 {
        let addr = addr as usize;
        u32::from_be_bytes([
            self.ram[addr],
            self.ram[addr + 1],
            self.ram[addr + 2],
            self.ram[addr + 3],
        ])
    }
    
    fn write_word(&mut self, addr: u32, val: u32) {
        let addr = addr as usize;
        let bytes = val.to_be_bytes();
        self.ram[addr..addr + 4].copy_from_slice(&bytes);
    }
    
    // Implement other methods...
}

let system = N64System { ram: vec![0; 0x800000] };
let mut cpu = CpuMipsR4300i::new(system);
cpu.reset();
```

## Instruction Set

The R4300i uses the MIPS III instruction set architecture.

### Instruction Format

MIPS uses three instruction formats:

#### R-Type (Register)
```
31    26 25  21 20  16 15  11 10   6 5     0
| opcode |  rs  |  rt  |  rd  | shamt | funct |
```

#### I-Type (Immediate)
```
31    26 25  21 20  16 15                   0
| opcode |  rs  |  rt  |     immediate      |
```

#### J-Type (Jump)
```
31    26 25                                 0
| opcode |          target address          |
```

### Instruction Categories

#### Load/Store
- **LB/LH/LW/LD**: Load byte/halfword/word/doubleword
- **LBU/LHU/LWU**: Load unsigned
- **SB/SH/SW/SD**: Store byte/halfword/word/doubleword
- **LWL/LWR**: Load word left/right (unaligned)
- **SWL/SWR**: Store word left/right
- **LDL/LDR**: Load doubleword left/right
- **SDL/SDR**: Store doubleword left/right
- **LL/SC**: Load-linked/store-conditional (atomic)
- **LLD/SCD**: Doubleword versions

#### Arithmetic (Integer)
- **ADD/ADDU**: Add (with/without overflow trap)
- **ADDI/ADDIU**: Add immediate
- **DADD/DADDU**: Doubleword add
- **DADDI/DADDIU**: Doubleword add immediate
- **SUB/SUBU**: Subtract
- **DSUB/DSUBU**: Doubleword subtract
- **MULT/MULTU**: Multiply (32-bit)
- **DMULT/DMULTU**: Doubleword multiply
- **DIV/DIVU**: Divide (32-bit)
- **DDIV/DDIVU**: Doubleword divide
- **MFHI/MFLO**: Move from HI/LO
- **MTHI/MTLO**: Move to HI/LO

#### Logical
- **AND/OR/XOR/NOR**: Bitwise operations
- **ANDI/ORI/XORI**: Bitwise with immediate
- **SLL/SRL/SRA**: Shift left/right logical/arithmetic
- **SLLV/SRLV/SRAV**: Variable shift
- **DSLL/DSRL/DSRA**: Doubleword shifts
- **DSLL32/DSRL32/DSRA32**: Doubleword shifts by 32+

#### Comparison
- **SLT/SLTU**: Set on less than (signed/unsigned)
- **SLTI/SLTIU**: Set on less than immediate

#### Branch
- **BEQ/BNE**: Branch on equal/not equal
- **BGTZ/BLEZ**: Branch on greater/less than or equal to zero
- **BLTZ/BGEZ**: Branch on less/greater than or equal to zero
- **BLTZAL/BGEZAL**: Branch and link
- **BC1F/BC1T**: FPU branch on false/true

#### Jump
- **J/JAL**: Jump (and link)
- **JR/JALR**: Jump register (and link)

#### Special
- **SYSCALL**: System call
- **BREAK**: Breakpoint
- **SYNC**: Synchronize shared memory
- **CACHE**: Cache operation

#### Coprocessor
- **MFC0/MTC0**: Move from/to coprocessor 0
- **DMFC0/DMTC0**: Doubleword move from/to CP0
- **MFC1/MTC1**: Move from/to FPU
- **DMFC1/DMTC1**: Doubleword move from/to FPU
- **CFC1/CTC1**: Control from/to FPU
- **LWC1/SWC1**: Load/store word to FPU
- **LDC1/SDC1**: Load/store doubleword to FPU

#### Floating-Point (CP1)
- **ADD.S/ADD.D**: Single/double add
- **SUB.S/SUB.D**: Single/double subtract
- **MUL.S/MUL.D**: Single/double multiply
- **DIV.S/DIV.D**: Single/double divide
- **SQRT.S/SQRT.D**: Square root
- **ABS.S/ABS.D**: Absolute value
- **NEG.S/NEG.D**: Negate
- **MOV.S/MOV.D**: Move
- **CVT**: Convert between formats
- **C.cond.S/D**: Compare and set FPU condition

#### TLB (Memory Management)
- **TLBR**: Read TLB entry
- **TLBWI**: Write TLB entry indexed
- **TLBWR**: Write TLB entry random
- **TLBP**: Probe TLB for matching entry

## Addressing Modes

MIPS uses simple addressing modes:

1. **Register Direct**: Operands in registers
2. **Immediate**: 16-bit immediate value (sign-extended)
3. **Base + Offset**: Register + 16-bit offset for loads/stores
4. **PC-Relative**: 16-bit offset for branches (target = PC + 4 + offset << 2)
5. **Absolute**: 26-bit address for jumps (target = (PC & 0xF0000000) | (target << 2))

## Pipeline

The R4300i has a 5-stage pipeline:

1. **IF** (Instruction Fetch): Fetch instruction from memory
2. **ID** (Instruction Decode): Decode and read registers
3. **EX** (Execute): ALU operation or address calculation
4. **MEM** (Memory Access): Load/store memory access
5. **WB** (Write Back): Write result to register

### Branch Delay Slot

MIPS has a branch delay slot - the instruction after a branch/jump always executes before the branch is taken:

```assembly
BEQ r1, r2, target
ADD r3, r4, r5      # Always executes (delay slot)
target:
```

## Exceptions and Interrupts

### Exception Types

- **Interrupt**: External hardware interrupt
- **TLB Miss**: TLB refill exception
- **TLB Invalid**: Invalid TLB entry
- **TLB Modified**: Write to read-only page
- **Address Error**: Misaligned access
- **Bus Error**: Memory bus error
- **Syscall**: SYSCALL instruction
- **Breakpoint**: BREAK instruction
- **Reserved Instruction**: Illegal opcode
- **Coprocessor Unusable**: CP not enabled
- **Overflow**: Arithmetic overflow
- **Trap**: Trap instruction
- **FPE**: Floating-point exception

### Exception Vectors

- **Reset/NMI**: 0xBFC00000 (ROM)
- **TLB Refill**: 0x80000000
- **General Exception**: 0x80000180
- **Interrupt**: 0x80000200 (depends on BEV)

When BEV (Bootstrap Exception Vector) is set, vectors are in uncached ROM space (0xBFC00xxx).

## Memory Map (N64 Context)

- **0x00000000-0x03FFFFFF**: RDRAM (4MB expandable to 8MB)
- **0x04000000-0x04000FFF**: RSP DMEM (4KB)
- **0x04001000-0x04001FFF**: RSP IMEM (4KB)
- **0x04040000-0x040FFFFF**: SP registers
- **0x04100000-0x041FFFFF**: DP (RDP) registers
- **0x04300000-0x043FFFFF**: MI (MIPS Interface) registers
- **0x04400000-0x044FFFFF**: VI (Video Interface) registers
- **0x04500000-0x045FFFFF**: AI (Audio Interface) registers
- **0x04600000-0x046FFFFF**: PI (Peripheral Interface) registers
- **0x04700000-0x047FFFFF**: RI (RDRAM Interface) registers
- **0x04800000-0x048FFFFF**: SI (Serial Interface) registers
- **0x10000000-0x1FBFFFFF**: Cartridge ROM
- **0x1FC00000-0x1FC007BF**: PIF ROM (Boot code)

## Cache

The R4300i has:
- **Instruction Cache**: 16KB, 2-way set associative
- **Data Cache**: 8KB, 2-way set associative

Cache control via CACHE instruction and CP0 registers.

## Endianness

The R4300i supports both big-endian and little-endian modes:
- **Big-Endian**: Default for N64
- **Little-Endian**: Via RE bit in Status register

## Implementation Notes

### 64-bit Operations

While the CPU is 64-bit, the N64 primarily uses 32-bit operations. 64-bit operations are available but less common.

### Delay Slots

All branches and jumps have a delay slot. Emulators must execute the delay slot instruction before taking the branch.

### TLB

The Translation Lookaside Buffer (TLB) has 32 entries. Each entry can map two pages with variable size (4KB to 16MB).

### FPU

The FPU supports:
- Single precision (32-bit)
- Double precision (64-bit)
- IEEE 754 compliance with exceptions

## Systems Using R4300i

This CPU core is used by:

- **N64** (Nintendo 64) - `crates/systems/n64/`

## Performance Considerations

### Pipeline Stalls

Stalls occur on:
- Load-use hazards (loading data then immediately using it)
- Branch mispredictions
- Cache misses
- Coprocessor operations

### Optimization

For accurate emulation:
- Track pipeline state
- Model cache behavior
- Implement accurate timing for loads/stores
- Handle exceptions correctly

## References

- [MIPS R4300i Datasheet](https://n64brew.dev/wiki/R4300i) - N64 developer wiki
- [MIPS III ISA](https://www.cs.cornell.edu/courses/cs3410/2008fa/MIPS_Vol2.pdf) - Official MIPS instruction set
- [N64 Programming Manual](https://n64.dev/) - N64 development resources
- [Ultra64 Documentation](https://ultra64.ca/resources/documentation/) - Official Nintendo documentation
