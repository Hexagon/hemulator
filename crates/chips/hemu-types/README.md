# hemu-types

Shared types used across all `hemu-*` chip crates.

## Contents

### `DisassembledInstruction`

A portable representation of a single disassembled machine instruction, returned by every chip crate's disassembler.

```rust
use hemu_types::DisassembledInstruction;

let instr = DisassembledInstruction::new(0x8000, vec![0xA9, 0x42], "LDA #$42");
println!("{:04X}  {}", instr.address, instr.mnemonic);
```

## Design

`hemu-types` has **zero dependencies** and intentionally contains only the types needed to wire chip crate disassemblers together with the host system's debugger. It is the glue between the independently-publishable chip crates and the consuming application.
