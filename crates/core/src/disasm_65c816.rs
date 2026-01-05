//! 65C816 CPU disassembler (minimal implementation)
//!
//! Provides basic disassembly for the WDC 65C816 CPU used in SNES.
//! This is a simplified implementation that shows instruction bytes.

use crate::debug::DisassembledInstruction;

/// Disassemble a single 65C816 instruction from memory
///
/// Note: This is a minimal implementation. Full 65C816 disassembly is complex
/// due to variable-width instructions and emulation/native mode differences.
pub fn disassemble_65c816(memory: &[u8], address: u32) -> Option<DisassembledInstruction> {
    if memory.is_empty() {
        return None;
    }

    let opcode = memory[0];
    
    // Basic length estimation (most instructions are 1-3 bytes)
    let len = match opcode {
        // 1-byte instructions
        0x00 | 0x08 | 0x0B | 0x18 | 0x1A | 0x1B | 0x28 | 0x2B | 0x38 | 0x3A | 0x3B |
        0x40 | 0x48 | 0x4B | 0x58 | 0x5A | 0x5B | 0x60 | 0x68 | 0x6B | 0x78 | 0x7A | 0x7B |
        0x88 | 0x8A | 0x8B | 0x98 | 0x9A | 0x9B | 0xA8 | 0xAA | 0xAB | 0xB8 | 0xBA | 0xBB |
        0xC8 | 0xCA | 0xCB | 0xD8 | 0xDA | 0xDB | 0xE8 | 0xEA | 0xEB | 0xF8 | 0xFA | 0xFB => 1,
        
        // 3-byte instructions (absolute addressing, etc.)
        0x0C | 0x0D | 0x0E | 0x0F | 0x1C | 0x1D | 0x1E | 0x1F |
        0x2C | 0x2D | 0x2E | 0x2F | 0x3C | 0x3D | 0x3E | 0x3F |
        0x4C | 0x4D | 0x4E | 0x4F | 0x5C | 0x5D | 0x5E | 0x5F |
        0x6C | 0x6D | 0x6E | 0x6F | 0x7C | 0x7D | 0x7E | 0x7F |
        0x8C | 0x8D | 0x8E | 0x8F | 0x9C | 0x9D | 0x9E | 0x9F |
        0xAC | 0xAD | 0xAE | 0xAF | 0xBC | 0xBD | 0xBE | 0xBF |
        0xCC | 0xCD | 0xCE | 0xCF | 0xDC | 0xDD | 0xDE | 0xDF |
        0xEC | 0xED | 0xEE | 0xEF | 0xFC | 0xFD | 0xFE | 0xFF => 3,
        
        // Default to 2 bytes for most immediate and zero-page modes
        _ => 2,
    };

    let bytes: Vec<u8> = memory.iter().take(len).copied().collect();
    let mnemonic = format!("DB ${}", bytes.iter().map(|b| format!("{:02X}", b)).collect::<Vec<_>>().join(","));
    
    Some(DisassembledInstruction::new(address, bytes, mnemonic))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disassemble_basic() {
        let memory = [0xEA]; // NOP
        let instr = disassemble_65c816(&memory, 0x8000).unwrap();
        assert_eq!(instr.address, 0x8000);
        assert_eq!(instr.len(), 1);
    }

    #[test]
    fn test_disassemble_empty() {
        let memory = [];
        assert!(disassemble_65c816(&memory, 0x8000).is_none());
    }
}
