# hemu-mips-r4300i

Reusable **MIPS R4300i** CPU emulation core — the 64-bit MIPS III processor used in the Nintendo 64.

## Features

- Complete 64-bit MIPS III instruction set
- 32 general-purpose 64-bit registers
- 32 64-bit FPU registers (COP1)
- TLB (Translation Lookaside Buffer) for virtual memory
- COP0 system control coprocessor
- Branch delay slots
- 32-bit and 64-bit operation modes
- Generic memory + TLB interface via `MemoryMips` trait
- Full MIPS III disassembler via `disasm_mips_r4300i` module (extends `hemu-mips-common` base + 64-bit/FPU/cache overlay)
- Optional `log` crate integration for unaligned access warnings

## Usage

```rust
use hemu_mips_r4300i::cpu_mips_r4300i::{CpuMips, MemoryMips};

struct MyBus { rdram: Vec<u8> }

impl MemoryMips for MyBus {
    fn read_u8(&mut self, addr: u64) -> u8 { /* ... */ }
    fn write_u8(&mut self, addr: u64, val: u8) { /* ... */ }
    // ... other required methods
}

let mut cpu = CpuMips::new(MyBus { rdram: vec![0; 0x800000] });
cpu.reset();
let cycles = cpu.step();
```

## Systems Using This Chip

| System | Clock |
|--------|-------|
| Nintendo 64 | 93.75 MHz |

## References

- MIPS R4000 Microprocessor User's Manual (MIPS Technologies, 1994)
- N64 Technical Reference Manual (Nintendo, 1996)
- n64.readthedocs.io — N64 programming reference
