# hemu-8080

Reusable **Intel 8080** CPU emulation core — the 8-bit ancestor of the Zilog Z80 and the Sharp LR35902 (Game Boy).

## Features

- Complete Intel 8080 instruction set
- Generic memory interface via `Memory8080` trait — plug in any bus implementation
- Shared 8080-family helper utilities (`cpu_8080_common`) used by the Z80 and LR35902
- No `std` dependency beyond normal Rust idioms (no heap allocations inside step)
- Accurate cycle counts per instruction

## Usage

```rust
use hemu_8080::cpu_8080::{Cpu8080, Memory8080};

struct MyBus { ram: [u8; 65536] }

impl Memory8080 for MyBus {
    fn read(&self, addr: u16) -> u8 { self.ram[addr as usize] }
    fn write(&mut self, addr: u16, val: u8) { self.ram[addr as usize] = val; }
    fn io_read(&mut self, _port: u8) -> u8 { 0xFF }
    fn io_write(&mut self, _port: u8, _val: u8) {}
}

let mut cpu = Cpu8080::new(MyBus { ram: [0; 65536] });
cpu.reset();
let cycles = cpu.step(); // execute one instruction
```

## Crate Layout

| Module | Description |
|--------|-------------|
| `cpu_8080` | Intel 8080 CPU state machine and instruction execution |
| `cpu_8080_common` | Shared 8080-family helpers (stack push/pop, register-pair accessors) |

## Systems Using This Chip

- CP/M systems
- Space Invaders arcade board
- Altair 8800

The shared helpers in `cpu_8080_common` are also used by [`hemu-z80`] and [`hemu-lr35902`].

## References

- Intel 8080 Assembly Language Programming Manual (1975)
- Intel 8080 Microcomputer Systems User's Manual (1975)
