# hemu-8086

Reusable **Intel 8086 / 80186 / 80286 / 80386** CPU emulation core.

## Features

- Complete 8086 instruction set (all official opcodes)
- 80186 additional instructions (`PUSHA`/`POPA`, `IMUL imm`, `INS`/`OUTS`, shift with count, etc.)
- 80286 protected mode (descriptor tables, privilege levels, task switching)
- 80386 32-bit registers and addressing modes
- Accurate segmented memory model (`CS:IP`, `DS:`, `ES:`, `SS:`)
- Configurable CPU model via `CpuModel` enum (8086 through Pentium MMX family)
- Generic memory + I/O interface via `Memory8086` trait
- Comprehensive test suite (12 test modules covering all instruction groups)
- Optional `log` crate integration for trace-level debugging

## Usage

```rust
use hemu_8086::cpu_8086::{Cpu8086, Memory8086};

struct MyBus { ram: Vec<u8> }

impl Memory8086 for MyBus {
    fn read_byte(&self, addr: u32) -> u8 { self.ram[addr as usize] }
    fn write_byte(&mut self, addr: u32, val: u8) { self.ram[addr as usize] = val; }
    fn io_read_byte(&mut self, _port: u16) -> u8 { 0xFF }
    fn io_write_byte(&mut self, _port: u16, _val: u8) {}
}

let mut cpu = Cpu8086::new(MyBus { ram: vec![0; 1 << 20] });
cpu.reset();
let cycles = cpu.step();
```

## Crate Layout

| Module | Description |
|--------|-------------|
| `cpu_8086` | Main CPU state machine and instruction execution |
| `cpu_8086_protected` | 80286 protected mode (GDT, IDT, LDT, TSS, descriptor tables) |

## Systems Using This Chip

| System | CPU Model |
|--------|-----------|
| IBM PC / PC XT | Intel 8086 / 8088 |
| IBM PC AT | Intel 80286 |
| IBM PS/2 (some models) | Intel 80386 |
| Tandy 1000 | Intel 8088 |

## References

- Intel 8086/8088 User's Manual (1979, Intel)
- Intel 80286 Programmer's Reference Manual (1985, Intel)
- Intel 80386 Programmer's Reference Manual (1986, Intel)
- Ralf Brown's Interrupt List (http://www.ctyme.com/rbrown.htm)
- x86 Opcode and Instruction Reference (http://ref.x86asm.net/coder32.html)
