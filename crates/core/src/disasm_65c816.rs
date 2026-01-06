//! 65C816 CPU disassembler
//!
//! Provides disassembly for the WDC 65C816 CPU used in SNES.
//! The 65C816 is a 16-bit extension of the 6502 with additional addressing modes
//! and instructions. This implementation assumes native mode (not emulation mode).

use crate::debug::DisassembledInstruction;

/// Disassemble a single 65C816 instruction from memory
///
/// Note: This implementation assumes native mode. The 65C816 has mode-dependent
/// instruction sizes (M and X flags affect immediate operand width), but for
/// debugging purposes we assume 8-bit mode which matches most SNES usage.
pub fn disassemble_65c816(memory: &[u8], address: u32) -> Option<DisassembledInstruction> {
    if memory.is_empty() {
        return None;
    }

    let opcode = memory[0];

    // Decode instruction based on opcode
    let (mnemonic, len) = decode_instruction(opcode, memory);

    let bytes: Vec<u8> = memory.iter().take(len).copied().collect();
    Some(DisassembledInstruction::new(address, bytes, mnemonic))
}

fn decode_instruction(opcode: u8, _memory: &[u8]) -> (String, usize) {
    match opcode {
        // BRK, COP, WDM
        0x00 => ("BRK".to_string(), 2),
        0x02 => ("COP #$%02X".to_string(), 2),
        0x42 => ("WDM #$%02X".to_string(), 2),

        // Stack operations
        0x08 => ("PHP".to_string(), 1),
        0x0B => ("PHD".to_string(), 1),
        0x28 => ("PLP".to_string(), 1),
        0x2B => ("PLD".to_string(), 1),
        0x48 => ("PHA".to_string(), 1),
        0x4B => ("PHK".to_string(), 1),
        0x5B => ("TCD".to_string(), 1),
        0x68 => ("PLA".to_string(), 1),
        0x7B => ("TDC".to_string(), 1),
        0x8B => ("PHB".to_string(), 1),
        0xAB => ("PLB".to_string(), 1),
        0xDA => ("PHX".to_string(), 1),
        0xFA => ("PLX".to_string(), 1),
        0x5A => ("PHY".to_string(), 1),
        0x7A => ("PLY".to_string(), 1),

        // Control flow
        0x18 => ("CLC".to_string(), 1),
        0x38 => ("SEC".to_string(), 1),
        0x58 => ("CLI".to_string(), 1),
        0x78 => ("SEI".to_string(), 1),
        0xD8 => ("CLD".to_string(), 1),
        0xF8 => ("SED".to_string(), 1),
        0xB8 => ("CLV".to_string(), 1),
        0xC2 => ("REP #$%02X".to_string(), 2),
        0xE2 => ("SEP #$%02X".to_string(), 2),

        // Transfer operations
        0x8A => ("TXA".to_string(), 1),
        0x98 => ("TYA".to_string(), 1),
        0x9A => ("TXS".to_string(), 1),
        0x9B => ("TXY".to_string(), 1),
        0xA8 => ("TAY".to_string(), 1),
        0xAA => ("TAX".to_string(), 1),
        0xBA => ("TSX".to_string(), 1),
        0xBB => ("TYX".to_string(), 1),

        // Increment/Decrement
        0x1A => ("INC A".to_string(), 1),
        0x3A => ("DEC A".to_string(), 1),
        0xC8 => ("INY".to_string(), 1),
        0xCA => ("DEX".to_string(), 1),
        0xE8 => ("INX".to_string(), 1),
        0x88 => ("DEY".to_string(), 1),

        // Special
        0x40 => ("RTI".to_string(), 1),
        0x60 => ("RTS".to_string(), 1),
        0x6B => ("RTL".to_string(), 1),
        0xCB => ("WAI".to_string(), 1),
        0xDB => ("STP".to_string(), 1),
        0xEA => ("NOP".to_string(), 1),
        0xEB => ("XBA".to_string(), 1),
        0xFB => ("XCE".to_string(), 1),

        // ORA instructions
        0x09 => ("ORA #$%02X".to_string(), 2),
        0x05 => ("ORA $%02X".to_string(), 2),
        0x15 => ("ORA $%02X,X".to_string(), 2),
        0x0D => ("ORA $%04X".to_string(), 3),
        0x1D => ("ORA $%04X,X".to_string(), 3),
        0x19 => ("ORA $%04X,Y".to_string(), 3),
        0x01 => ("ORA ($%02X,X)".to_string(), 2),
        0x11 => ("ORA ($%02X),Y".to_string(), 2),
        0x07 => ("ORA [$%02X]".to_string(), 2),
        0x17 => ("ORA [$%02X],Y".to_string(), 2),
        0x03 => ("ORA $%02X,S".to_string(), 2),
        0x13 => ("ORA ($%02X,S),Y".to_string(), 2),

        // AND instructions
        0x29 => ("AND #$%02X".to_string(), 2),
        0x25 => ("AND $%02X".to_string(), 2),
        0x35 => ("AND $%02X,X".to_string(), 2),
        0x2D => ("AND $%04X".to_string(), 3),
        0x3D => ("AND $%04X,X".to_string(), 3),
        0x39 => ("AND $%04X,Y".to_string(), 3),
        0x21 => ("AND ($%02X,X)".to_string(), 2),
        0x31 => ("AND ($%02X),Y".to_string(), 2),
        0x27 => ("AND [$%02X]".to_string(), 2),
        0x37 => ("AND [$%02X],Y".to_string(), 2),
        0x23 => ("AND $%02X,S".to_string(), 2),
        0x33 => ("AND ($%02X,S),Y".to_string(), 2),

        // EOR instructions
        0x49 => ("EOR #$%02X".to_string(), 2),
        0x45 => ("EOR $%02X".to_string(), 2),
        0x55 => ("EOR $%02X,X".to_string(), 2),
        0x4D => ("EOR $%04X".to_string(), 3),
        0x5D => ("EOR $%04X,X".to_string(), 3),
        0x59 => ("EOR $%04X,Y".to_string(), 3),
        0x41 => ("EOR ($%02X,X)".to_string(), 2),
        0x51 => ("EOR ($%02X),Y".to_string(), 2),
        0x47 => ("EOR [$%02X]".to_string(), 2),
        0x57 => ("EOR [$%02X],Y".to_string(), 2),
        0x43 => ("EOR $%02X,S".to_string(), 2),
        0x53 => ("EOR ($%02X,S),Y".to_string(), 2),

        // ADC instructions
        0x69 => ("ADC #$%02X".to_string(), 2),
        0x65 => ("ADC $%02X".to_string(), 2),
        0x75 => ("ADC $%02X,X".to_string(), 2),
        0x6D => ("ADC $%04X".to_string(), 3),
        0x7D => ("ADC $%04X,X".to_string(), 3),
        0x79 => ("ADC $%04X,Y".to_string(), 3),
        0x61 => ("ADC ($%02X,X)".to_string(), 2),
        0x71 => ("ADC ($%02X),Y".to_string(), 2),
        0x67 => ("ADC [$%02X]".to_string(), 2),
        0x77 => ("ADC [$%02X],Y".to_string(), 2),
        0x63 => ("ADC $%02X,S".to_string(), 2),
        0x73 => ("ADC ($%02X,S),Y".to_string(), 2),

        // SBC instructions
        0xE9 => ("SBC #$%02X".to_string(), 2),
        0xE5 => ("SBC $%02X".to_string(), 2),
        0xF5 => ("SBC $%02X,X".to_string(), 2),
        0xED => ("SBC $%04X".to_string(), 3),
        0xFD => ("SBC $%04X,X".to_string(), 3),
        0xF9 => ("SBC $%04X,Y".to_string(), 3),
        0xE1 => ("SBC ($%02X,X)".to_string(), 2),
        0xF1 => ("SBC ($%02X),Y".to_string(), 2),
        0xE7 => ("SBC [$%02X]".to_string(), 2),
        0xF7 => ("SBC [$%02X],Y".to_string(), 2),
        0xE3 => ("SBC $%02X,S".to_string(), 2),
        0xF3 => ("SBC ($%02X,S),Y".to_string(), 2),

        // CMP instructions
        0xC9 => ("CMP #$%02X".to_string(), 2),
        0xC5 => ("CMP $%02X".to_string(), 2),
        0xD5 => ("CMP $%02X,X".to_string(), 2),
        0xCD => ("CMP $%04X".to_string(), 3),
        0xDD => ("CMP $%04X,X".to_string(), 3),
        0xD9 => ("CMP $%04X,Y".to_string(), 3),
        0xC1 => ("CMP ($%02X,X)".to_string(), 2),
        0xD1 => ("CMP ($%02X),Y".to_string(), 2),
        0xC7 => ("CMP [$%02X]".to_string(), 2),
        0xD7 => ("CMP [$%02X],Y".to_string(), 2),
        0xC3 => ("CMP $%02X,S".to_string(), 2),
        0xD3 => ("CMP ($%02X,S),Y".to_string(), 2),

        // CPX, CPY
        0xE0 => ("CPX #$%02X".to_string(), 2),
        0xE4 => ("CPX $%02X".to_string(), 2),
        0xEC => ("CPX $%04X".to_string(), 3),
        0xC0 => ("CPY #$%02X".to_string(), 2),
        0xC4 => ("CPY $%02X".to_string(), 2),
        0xCC => ("CPY $%04X".to_string(), 3),

        // LDA instructions
        0xA9 => ("LDA #$%02X".to_string(), 2),
        0xA5 => ("LDA $%02X".to_string(), 2),
        0xB5 => ("LDA $%02X,X".to_string(), 2),
        0xAD => ("LDA $%04X".to_string(), 3),
        0xBD => ("LDA $%04X,X".to_string(), 3),
        0xB9 => ("LDA $%04X,Y".to_string(), 3),
        0xA1 => ("LDA ($%02X,X)".to_string(), 2),
        0xB1 => ("LDA ($%02X),Y".to_string(), 2),
        0xA7 => ("LDA [$%02X]".to_string(), 2),
        0xB7 => ("LDA [$%02X],Y".to_string(), 2),
        0xA3 => ("LDA $%02X,S".to_string(), 2),
        0xB3 => ("LDA ($%02X,S),Y".to_string(), 2),

        // LDX, LDY
        0xA2 => ("LDX #$%02X".to_string(), 2),
        0xA6 => ("LDX $%02X".to_string(), 2),
        0xB6 => ("LDX $%02X,Y".to_string(), 2),
        0xAE => ("LDX $%04X".to_string(), 3),
        0xBE => ("LDX $%04X,Y".to_string(), 3),
        0xA0 => ("LDY #$%02X".to_string(), 2),
        0xA4 => ("LDY $%02X".to_string(), 2),
        0xB4 => ("LDY $%02X,X".to_string(), 2),
        0xAC => ("LDY $%04X".to_string(), 3),
        0xBC => ("LDY $%04X,X".to_string(), 3),

        // STA instructions
        0x85 => ("STA $%02X".to_string(), 2),
        0x95 => ("STA $%02X,X".to_string(), 2),
        0x8D => ("STA $%04X".to_string(), 3),
        0x9D => ("STA $%04X,X".to_string(), 3),
        0x99 => ("STA $%04X,Y".to_string(), 3),
        0x81 => ("STA ($%02X,X)".to_string(), 2),
        0x91 => ("STA ($%02X),Y".to_string(), 2),
        0x87 => ("STA [$%02X]".to_string(), 2),
        0x97 => ("STA [$%02X],Y".to_string(), 2),
        0x83 => ("STA $%02X,S".to_string(), 2),
        0x93 => ("STA ($%02X,S),Y".to_string(), 2),

        // STX, STY, STZ
        0x86 => ("STX $%02X".to_string(), 2),
        0x96 => ("STX $%02X,Y".to_string(), 2),
        0x8E => ("STX $%04X".to_string(), 3),
        0x84 => ("STY $%02X".to_string(), 2),
        0x94 => ("STY $%02X,X".to_string(), 2),
        0x8C => ("STY $%04X".to_string(), 3),
        0x64 => ("STZ $%02X".to_string(), 2),
        0x74 => ("STZ $%02X,X".to_string(), 2),
        0x9C => ("STZ $%04X".to_string(), 3),
        0x9E => ("STZ $%04X,X".to_string(), 3),

        // Shifts and rotates
        0x0A => ("ASL A".to_string(), 1),
        0x06 => ("ASL $%02X".to_string(), 2),
        0x16 => ("ASL $%02X,X".to_string(), 2),
        0x0E => ("ASL $%04X".to_string(), 3),
        0x1E => ("ASL $%04X,X".to_string(), 3),
        0x4A => ("LSR A".to_string(), 1),
        0x46 => ("LSR $%02X".to_string(), 2),
        0x56 => ("LSR $%02X,X".to_string(), 2),
        0x4E => ("LSR $%04X".to_string(), 3),
        0x5E => ("LSR $%04X,X".to_string(), 3),
        0x2A => ("ROL A".to_string(), 1),
        0x26 => ("ROL $%02X".to_string(), 2),
        0x36 => ("ROL $%02X,X".to_string(), 2),
        0x2E => ("ROL $%04X".to_string(), 3),
        0x3E => ("ROL $%04X,X".to_string(), 3),
        0x6A => ("ROR A".to_string(), 1),
        0x66 => ("ROR $%02X".to_string(), 2),
        0x76 => ("ROR $%02X,X".to_string(), 2),
        0x6E => ("ROR $%04X".to_string(), 3),
        0x7E => ("ROR $%04X,X".to_string(), 3),

        // Bit operations
        0x89 => ("BIT #$%02X".to_string(), 2),
        0x24 => ("BIT $%02X".to_string(), 2),
        0x34 => ("BIT $%02X,X".to_string(), 2),
        0x2C => ("BIT $%04X".to_string(), 3),
        0x3C => ("BIT $%04X,X".to_string(), 3),
        0x04 => ("TSB $%02X".to_string(), 2),
        0x0C => ("TSB $%04X".to_string(), 3),
        0x14 => ("TRB $%02X".to_string(), 2),
        0x1C => ("TRB $%04X".to_string(), 3),

        // Branches
        0x10 => ("BPL $%02X".to_string(), 2),
        0x30 => ("BMI $%02X".to_string(), 2),
        0x50 => ("BVC $%02X".to_string(), 2),
        0x70 => ("BVS $%02X".to_string(), 2),
        0x80 => ("BRA $%02X".to_string(), 2),
        0x82 => ("BRL $%04X".to_string(), 3),
        0x90 => ("BCC $%02X".to_string(), 2),
        0xB0 => ("BCS $%02X".to_string(), 2),
        0xD0 => ("BNE $%02X".to_string(), 2),
        0xF0 => ("BEQ $%02X".to_string(), 2),

        // Jumps and calls
        0x4C => ("JMP $%04X".to_string(), 3),
        0x5C => ("JML $%06X".to_string(), 4),
        0x6C => ("JMP ($%04X)".to_string(), 3),
        0x7C => ("JMP ($%04X,X)".to_string(), 3),
        0xDC => ("JML [$%04X]".to_string(), 3),
        0x20 => ("JSR $%04X".to_string(), 3),
        0x22 => ("JSL $%06X".to_string(), 4),
        0xFC => ("JSR ($%04X,X)".to_string(), 3),

        // Block moves
        0x44 => ("MVP $%02X,$%02X".to_string(), 3),
        0x54 => ("MVN $%02X,$%02X".to_string(), 3),

        // PEI, PER, PEA
        0xD4 => ("PEI ($%02X)".to_string(), 2),
        0x62 => ("PER $%04X".to_string(), 3),
        0xF4 => ("PEA $%04X".to_string(), 3),

        // INC, DEC absolute
        0xE6 => ("INC $%02X".to_string(), 2),
        0xF6 => ("INC $%02X,X".to_string(), 2),
        0xEE => ("INC $%04X".to_string(), 3),
        0xFE => ("INC $%04X,X".to_string(), 3),
        0xC6 => ("DEC $%02X".to_string(), 2),
        0xD6 => ("DEC $%02X,X".to_string(), 2),
        0xCE => ("DEC $%04X".to_string(), 3),
        0xDE => ("DEC $%04X,X".to_string(), 3),

        _ => (format!("??? ${:02X}", opcode), 1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disassemble_nop() {
        let memory = [0xEA];
        let instr = disassemble_65c816(&memory, 0x8000).unwrap();
        assert_eq!(instr.address, 0x8000);
        assert_eq!(instr.len(), 1);
        assert_eq!(instr.mnemonic, "NOP");
    }

    #[test]
    fn test_disassemble_lda_immediate() {
        let memory = [0xA9, 0x42];
        let instr = disassemble_65c816(&memory, 0x8000).unwrap();
        assert_eq!(instr.mnemonic, "LDA #$%02X");
        assert_eq!(instr.len(), 2);
    }

    #[test]
    fn test_disassemble_jmp_absolute() {
        let memory = [0x4C, 0x00, 0x80];
        let instr = disassemble_65c816(&memory, 0x8000).unwrap();
        assert_eq!(instr.mnemonic, "JMP $%04X");
        assert_eq!(instr.len(), 3);
    }

    #[test]
    fn test_disassemble_rep() {
        let memory = [0xC2, 0x30];
        let instr = disassemble_65c816(&memory, 0x8000).unwrap();
        assert_eq!(instr.mnemonic, "REP #$%02X");
        assert_eq!(instr.len(), 2);
    }

    #[test]
    fn test_disassemble_jsl() {
        let memory = [0x22, 0x00, 0x80, 0x00];
        let instr = disassemble_65c816(&memory, 0x8000).unwrap();
        assert_eq!(instr.mnemonic, "JSL $%06X");
        assert_eq!(instr.len(), 4);
    }

    #[test]
    fn test_disassemble_empty() {
        let memory = [];
        assert!(disassemble_65c816(&memory, 0x8000).is_none());
    }
}
