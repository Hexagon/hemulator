//! MIPS R4300i CPU disassembler (minimal implementation)
//!
//! Provides basic disassembly for the MIPS R4300i CPU used in N64.
//! This is a simplified implementation that shows instruction bytes.

use crate::debug::DisassembledInstruction;

/// Disassemble a single MIPS instruction from memory
///
/// Note: This is a minimal implementation. MIPS instructions are always 4 bytes
/// (32-bit), which makes disassembly simpler than variable-length ISAs.
pub fn disassemble_mips(memory: &[u8], address: u32) -> Option<DisassembledInstruction> {
    if memory.len() < 4 {
        return None;
    }

    // MIPS instructions are 4 bytes, big-endian
    let bytes = vec![memory[0], memory[1], memory[2], memory[3]];
    let instruction = u32::from_be_bytes([memory[0], memory[1], memory[2], memory[3]]);
    
    let mnemonic = format!("MIPS ${:08X}", instruction);
    
    Some(DisassembledInstruction::new(address, bytes, mnemonic))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disassemble_basic() {
        let memory = [0x00, 0x00, 0x00, 0x00]; // NOP (all zeros)
        let instr = disassemble_mips(&memory, 0x80000000).unwrap();
        assert_eq!(instr.address, 0x80000000);
        assert_eq!(instr.len(), 4);
    }

    #[test]
    fn test_disassemble_short_memory() {
        let memory = [0x00, 0x00];
        assert!(disassemble_mips(&memory, 0x80000000).is_none());
    }
}
