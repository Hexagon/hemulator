# hemu-arm7tdmi

Reusable **ARM7TDMI** CPU emulation core — the 32-bit RISC processor used in the Game Boy Advance.

## Features

- Full **ARM** (32-bit) and **Thumb** (16-bit) instruction sets
- All 7 processor modes: User, FIQ, IRQ, Supervisor, Abort, Undefined, System
- Banked registers per privilege mode
- Barrel shifter (LSL, LSR, ASR, ROR) integrated into data-processing instructions
- Block data transfer (`LDM`/`STM`) with writeback
- 32×32 multiply (`MUL`, `MLA`) and 64-bit multiply (`UMULL`, `SMULL`, etc.)
- Single data swap (`SWP`)
- Software interrupt (`SWI`) with optional BIOS handler
- Halfword and signed byte load/store extensions
- Accurate cycle counts
- Optional `log` crate integration for trace-level debugging

## Usage

```rust
use hemu_arm7tdmi::cpu_arm7tdmi::{Arm7Tdmi, MemoryArm7};

struct MyBus { ram: Vec<u8> }

impl MemoryArm7 for MyBus {
    fn read_byte(&self, addr: u32) -> u8 { self.ram[addr as usize] }
    fn write_byte(&mut self, addr: u32, val: u8) { self.ram[addr as usize] = val; }
    fn read_halfword(&self, addr: u32) -> u16 {
        let a = addr as usize & !1;
        u16::from_le_bytes([self.ram[a], self.ram[a+1]])
    }
    fn write_halfword(&mut self, addr: u32, val: u16) {
        let a = addr as usize & !1;
        let b = val.to_le_bytes();
        self.ram[a] = b[0]; self.ram[a+1] = b[1];
    }
    fn read_word(&self, addr: u32) -> u32 {
        let a = addr as usize & !3;
        u32::from_le_bytes(self.ram[a..a+4].try_into().unwrap())
    }
    fn write_word(&mut self, addr: u32, val: u32) {
        let a = addr as usize & !3;
        let b = val.to_le_bytes();
        self.ram[a..a+4].copy_from_slice(&b);
    }
}

let mut cpu = Arm7Tdmi::new(MyBus { ram: vec![0; 0x1000_0000] });
cpu.reset();
let cycles = cpu.step();
```

## Systems Using This Chip

| System | Clock |
|--------|-------|
| Game Boy Advance (GBA) | 16.78 MHz |
| Nintendo DS (ARM7 co-processor) | 33.51 MHz |
| Various embedded systems | — |

## References

- ARM7TDMI Technical Reference Manual (ARM DDI 0029G)
- ARM Architecture Reference Manual (ARM DDI 0100E)
- GBATEK — GBA/NDS Technical Data (https://problemkaputt.de/gbatek.htm)
