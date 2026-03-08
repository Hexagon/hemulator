# hemu-mips-r3000a

Reusable **MIPS R3000A** CPU emulation core — the 32-bit MIPS I processor used in the PlayStation 1.

## Features

- Complete MIPS I instruction set (all 64 opcodes)
- COP0 system control coprocessor (exception handling, status/cause registers)
- COP2 GTE (Geometry Transform Engine) interface
- Branch delay slots
- Precise exception handling (SYSCALL, BREAK, address errors, overflow)
- Generic memory interface via `MemoryR3000A` trait
- MIPS I disassembler via `disasm_mips_r3000a` module

## Usage

```rust
use hemu_mips_r3000a::cpu_mips_r3000a::{CpuR3000A, MemoryR3000A};

struct MyBus { ram: Vec<u8> }

impl MemoryR3000A for MyBus {
    fn read_byte(&self, addr: u32) -> u8 { self.ram[addr as usize & 0x1FFFFF] }
    fn write_byte(&mut self, addr: u32, val: u8) { self.ram[addr as usize & 0x1FFFFF] = val; }
    // ... other required methods
}

let mut cpu = CpuR3000A::new(MyBus { ram: vec![0; 0x200000] });
cpu.reset();
let cycles = cpu.step();
```

## Disassembler

```rust
use hemu_mips_r3000a::disasm_mips_r3000a::disassemble_r3000a;

if let Some(instr) = disassemble_r3000a(&memory[pc..], pc as u32) {
    println!("{:08X}  {}", instr.address, instr.mnemonic);
}
```

## Systems Using This Chip

| System | Clock |
|--------|-------|
| Sony PlayStation 1 (PSX) | 33.8688 MHz |

## References

- MIPS R3000 CPU Reference Manual (IDT, 1994)
- Nocash PSX Specifications (https://problemkaputt.de/psx-spx.htm)
- PlayStation 1 No-Intro Disc (BIOS/ROM documentation)
