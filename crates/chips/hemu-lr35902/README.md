# hemu-lr35902

Reusable **Sharp LR35902** CPU emulation core — the custom Z80-derivative processor used in the original Game Boy and Game Boy Color.

## Features

- Complete LR35902 instruction set (Z80 subset with Game Boy–specific additions)
- Full `CB`-prefixed instruction set (bit operations, rotates, shifts)
- Interrupt Master Enable (`IME`) with proper `EI` delay emulation
- HALT instruction with hardware HALT-bug emulation
- STOP instruction with CGB double-speed support
- Generic memory interface via `MemoryLr35902` trait
- Accurate cycle counts per instruction (M-cycles)

## Usage

```rust
use hemu_lr35902::cpu_lr35902::{CpuLr35902, MemoryLr35902};

struct MyBus { ram: [u8; 65536] }

impl MemoryLr35902 for MyBus {
    fn read(&self, addr: u16) -> u8 { self.ram[addr as usize] }
    fn write(&mut self, addr: u16, val: u8) { self.ram[addr as usize] = val; }
}

let mut cpu = CpuLr35902::new(MyBus { ram: [0; 65536] });
cpu.reset();
let cycles = cpu.step();
```

## Systems Using This Chip

| System | Notes |
|--------|-------|
| Game Boy (DMG) | 4.19 MHz |
| Game Boy Pocket (MGB) | 4.19 MHz |
| Game Boy Color (CGB) | 4.19 / 8.38 MHz (double-speed) |
| Super Game Boy | 4.295 MHz |

## References

- Pan Docs — Game Boy Technical Reference (https://gbdev.io/pandocs/)
- The Cycle-Accurate Game Boy Docs (https://github.com/AntonioND/giibiiadvance/blob/master/docs/TCAGBD.pdf)
- Game Boy CPU Manual (http://marc.rawer.de/Gameboy/Docs/GBCPUman.pdf)
