# hemu-65c816

Reusable **WDC 65C816** CPU emulation core — the 16-bit successor to the MOS 6502.

## Features

- Full 65C816 instruction set (16-bit native mode and 6502 emulation mode)
- Switchable accumulator and index register widths (8-bit / 16-bit) via `REP`/`SEP`
- 24-bit address space (16 MB) with bank registers (`DBR`, `PBR`)
- Direct page addressing with configurable base (`D` register)
- All addressing modes including stack-relative and `[...]` indirect long
- Emulation mode (`e` flag) for 6502 backward compatibility
- `WAI` (wait for interrupt) and `STP` (stop the clock) instructions
- Accurate cycle counts per instruction
- Optional `log` crate integration for trace-level debugging

## Usage

```rust
use hemu_65c816::cpu_65c816::{Cpu65c816, Memory65c816};

struct MyBus { rom: [u8; 0x100_0000] }

impl Memory65c816 for MyBus {
    fn read(&self, addr: u32) -> u8 { self.rom[addr as usize] }
    fn write(&mut self, addr: u32, val: u8) { self.rom[addr as usize] = val; }
}

let mut cpu = Cpu65c816::new(MyBus { rom: [0; 0x100_0000] });
cpu.reset();
let cycles = cpu.step();
```

## Systems Using This Chip

| System | Notes |
|--------|-------|
| Super Nintendo Entertainment System (SNES) | 3.58 MHz |
| Apple IIGS | 1–2.8 MHz |
| Acorn Communicator | — |

## References

- WDC W65C816S Data Sheet (2018, Western Design Center)
- WDC W65C816 Programming Manual
- SNES Development Manual (Nintendo, 1993)
- Appendix G — 65C816 Instruction Set (http://6502.org/tutorials/65c816opcodes.html)
