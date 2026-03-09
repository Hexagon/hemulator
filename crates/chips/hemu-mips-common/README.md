# hemu-mips-common

Shared helpers for MIPS CPU emulation — register names, instruction field
extraction, and a base **MIPS I** disassembler that can be extended by
variant-specific crates like `hemu-mips-r3000a` (MIPS I + GTE) and
`hemu-mips-r4300i` (MIPS III + TLB).

## What's included

| Module | Description |
|--------|-------------|
| `fields` | Instruction field extraction (opcode, rs, rt, rd, sa, funct, imm, target) |
| `regs` | Standard 32-register ABI names (`zero`, `at`, `v0`–`v1`, `a0`–`a3`, etc.) |
| `disasm_mips` | Base MIPS I disassembler — SPECIAL, REGIMM, branches, loads/stores, immediates |

## Usage

```rust
use hemu_mips_common::fields;
use hemu_mips_common::regs::REG_NAMES;
use hemu_mips_common::disasm_mips;

// Extract fields from a MIPS instruction word
let word: u32 = 0x2108002A; // ADDI $t0, $t0, 42
let f = fields::Fields::from(word);
println!("opcode={} rs={} rt={} imm={}", f.opcode, f.rs, f.rt, f.simm);

// Disassemble with the base MIPS I decoder
let mem = word.to_le_bytes();
if let Some(instr) = disasm_mips::disassemble_mips_i(&mem, 0x80000000, disasm_mips::Endian::Little) {
    println!("{}", instr.mnemonic);
}
```

## Extending for a specific variant

Variant crates (R3000A, R4300i) call `disassemble_mips_i` first and only
override opcodes that differ — e.g. COP2/GTE for R3000A, 64-bit ops and
likely-branches for R4300i.

## Hardware references

- MIPS I instruction set: *MIPS IV Instruction Set* (Revision 3.2), SGI 1995
- IDT R30xx Family Software Reference Manual
- MIPS R4300i CPU Manual (NEC VR4300)
