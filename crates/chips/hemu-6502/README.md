# hemu-6502

Reusable **MOS 6502** CPU emulation core.

## Features

- Complete MOS 6502 instruction set with accurate cycle counts
- All official opcodes plus common unofficial/undocumented opcodes
- Decimal mode (BCD) support
- NMI, IRQ, and RESET vector handling
- Generic memory interface via `Memory6502` trait — plug in any bus
- Optional `log` crate integration for trace-level debugging

## Usage

```rust
use hemu_6502::cpu_6502::{Cpu6502, Memory6502};

struct MyBus { ram: [u8; 65536] }

impl Memory6502 for MyBus {
    fn read(&self, addr: u16) -> u8 { self.ram[addr as usize] }
    fn write(&mut self, addr: u16, val: u8) { self.ram[addr as usize] = val; }
}

let mut cpu = Cpu6502::new(MyBus { ram: [0; 65536] });
cpu.reset();
let cycles = cpu.step();
```

## Systems Using This Chip

| System | Clock |
|--------|-------|
| Nintendo Entertainment System (NES) | 1.79 MHz NTSC / 1.66 MHz PAL |
| Atari 2600 | 1.19 MHz |
| Apple II | 1 MHz |
| Commodore 64 (6510) | 1 MHz |
| Atari 5200 | 1.79 MHz |

## References

- MOS Technology MCS6500 Microcomputer Family Programming Manual (1975)
- 6502.org — CPU reference (http://www.6502.org/tutorials/6502opcodes.html)
- Visual 6502 Project (http://www.visual6502.org/)
- Nesdev Wiki — 6502 reference (https://www.nesdev.org/wiki/CPU)
