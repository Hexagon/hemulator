//! Shared MIPS CPU helpers — standalone chip-support crate.
//!
//! Provides register names, instruction field extraction, and a base MIPS I
//! disassembler.  Used by `hemu-mips-r3000a` and `hemu-mips-r4300i`.
//!
//! See the [README](../README.md) for usage and references.

pub mod disasm_mips;
pub mod fields;
pub mod regs;
