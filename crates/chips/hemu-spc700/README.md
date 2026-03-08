# hemu-spc700

Reusable **Sony SPC700** CPU emulation core — the 8-bit processor inside the SNES Audio Processing Unit (APU/S-SMP).

## Features

- Complete SPC700 instruction set (256 opcodes)
- All addressing modes (direct page, indexed, indirect, etc.)
- 3 hardware timers (2×8 kHz, 1×64 Hz)
- Communication ports for interfacing with the SNES main CPU (65C816)
- IPL ROM (64-byte boot ROM, can be disabled)
- DSP register access interface
- Generic memory interface via `MemorySpc700` trait
- Optional `log` crate integration for trace-level debugging

## Usage

```rust
use hemu_spc700::cpu_spc700::{CpuSpc700, MemorySpc700};

struct MyApu { ram: [u8; 65536] }

impl MemorySpc700 for MyApu {
    fn read(&self, addr: u16) -> u8 { self.ram[addr as usize] }
    fn write(&mut self, addr: u16, val: u8) { self.ram[addr as usize] = val; }
}

let mut cpu = CpuSpc700::new(MyApu { ram: [0; 65536] });
cpu.reset();
let cycles = cpu.step();
```

## Systems Using This Chip

| System | Notes |
|--------|-------|
| Super Nintendo Entertainment System (SNES) | 1.024 MHz |

## References

- Fullsnes — SPC700 Documentation (https://problemkaputt.de/fullsnes.htm#snescpuspc700audiosystemapu)
- Super Famicom Wiki — SPC700 Reference (https://wiki.superfamicom.org/spc700-reference)
- SNES Development Manual (Nintendo, 1993)
