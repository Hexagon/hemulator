# hemu-z80

Reusable **Zilog Z80** CPU emulation core.

## Features

- Complete Z80 instruction set including all prefixed opcodes (`CB`, `DD`, `ED`, `FD`, `DDCB`, `FDCB`)
- Documented undocumented instructions (`IXH`/`IXL`/`IYH`/`IYL`, `ED` mirrors, etc.)
- Shadow register set (`AF'`, `BC'`, `DE'`, `HL'`)
- Interrupt modes 0, 1, and 2
- Generic memory + I/O interface via `MemoryZ80` trait
- Accurate cycle counts per instruction

## Usage

```rust
use hemu_z80::cpu_z80::{CpuZ80, MemoryZ80};

struct MyBus { ram: [u8; 65536] }

impl MemoryZ80 for MyBus {
    fn read(&self, addr: u16) -> u8 { self.ram[addr as usize] }
    fn write(&mut self, addr: u16, val: u8) { self.ram[addr as usize] = val; }
    fn io_read(&mut self, _port: u8) -> u8 { 0xFF }
    fn io_write(&mut self, _port: u8, _val: u8) {}
}

let mut cpu = CpuZ80::new(MyBus { ram: [0; 65536] });
cpu.reset();
let cycles = cpu.step();
```

## Systems Using This Chip

| System | Notes |
|--------|-------|
| Sega Master System | 3.58 MHz NTSC / 3.55 MHz PAL |
| Sega SG-1000 | 3.58 MHz |
| ColecoVision | 3.58 MHz |
| Sega Game Gear | 3.58 MHz |
| Amstrad CPC | 4 MHz |
| ZX Spectrum | 3.5 MHz |

## References

- Zilog Z80 CPU User Manual (UM0080)
- Z80 CPU Technical Manual (Zilog 1977)
- Sean Young's *Z80 Undocumented Documented* (http://www.z80.info/z80undoc.htm)
