//! Shared types for hemu-* chip crates.
//!
//! This crate provides zero-dependency types shared between chip implementations
//! and the host application's debugger infrastructure.

/// A single disassembled machine instruction.
///
/// Returned by every chip crate's disassembler and consumed by the host
/// system's [`Debugger`] trait implementation.
///
/// [`Debugger`]: https://docs.rs/emu_core/latest/emu_core/debug/trait.Debugger.html
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisassembledInstruction {
    /// Program counter / address of instruction
    pub address: u32,
    /// Raw bytes of the instruction
    pub bytes: Vec<u8>,
    /// Disassembled mnemonic (e.g., "LDA #$10", "MOV AX, BX")
    pub mnemonic: String,
    /// Optional comment or annotation
    pub comment: Option<String>,
}

impl DisassembledInstruction {
    /// Create a new disassembled instruction
    pub fn new(address: u32, bytes: Vec<u8>, mnemonic: impl Into<String>) -> Self {
        Self {
            address,
            bytes,
            mnemonic: mnemonic.into(),
            comment: None,
        }
    }

    /// Add a comment to the instruction
    pub fn with_comment(mut self, comment: impl Into<String>) -> Self {
        self.comment = Some(comment.into());
        self
    }

    /// Get the length of this instruction in bytes
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Check if this instruction has zero length (should never happen)
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}
