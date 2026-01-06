//! 8086 CPU disassembler (minimal implementation)
//!
//! Provides basic disassembly for the Intel 8086 CPU family used in PC.
//! This is a simplified implementation that shows instruction bytes.

use crate::debug::DisassembledInstruction;

/// Disassemble a single 8086 instruction from memory
///
/// Note: This is a minimal implementation. Full 8086 disassembly is complex
/// due to variable-length instructions, prefixes, and ModR/M bytes.
pub fn disassemble_8086(memory: &[u8], address: u32) -> Option<DisassembledInstruction> {
    if memory.is_empty() {
        return None;
    }

    let opcode = memory[0];

    // Very basic length estimation (8086 instructions are 1-6 bytes typically)
    let len = match opcode {
        // 1-byte instructions
        0x90
        | 0x9B..=0x9F
        | 0xA4..=0xA7
        | 0xAA..=0xAF
        | 0xC3
        | 0xC9
        | 0xCB
        | 0xCC
        | 0xCE..=0xCF
        | 0xD0..=0xD7
        | 0xE4..=0xE7
        | 0xEC..=0xEF
        | 0xF0..=0xF3
        | 0xF4
        | 0xF5
        | 0xF8..=0xFD => 1,

        // Common 2-byte instructions
        0xB0..=0xBF | 0xCD => 2,

        // Assume 3 bytes for most other instructions (good enough for basic debugging)
        _ => 3,
    };

    let bytes: Vec<u8> = memory.iter().take(len.min(memory.len())).copied().collect();
    let mnemonic = format!(
        "DB ${}",
        bytes
            .iter()
            .map(|b| format!("{:02X}", b))
            .collect::<Vec<_>>()
            .join(",")
    );

    Some(DisassembledInstruction::new(address, bytes, mnemonic))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disassemble_nop() {
        let memory = [0x90]; // NOP
        let instr = disassemble_8086(&memory, 0x0000).unwrap();
        assert_eq!(instr.address, 0x0000);
        assert_eq!(instr.len(), 1);
    }

    #[test]
    fn test_disassemble_empty() {
        let memory = [];
        assert!(disassemble_8086(&memory, 0x0000).is_none());
    }
}
