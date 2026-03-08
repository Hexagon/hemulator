//! 6502 CPU disassembler
//!
//! Provides instruction disassembly for the MOS 6502 CPU used in NES, Atari 2600, and other systems.

use hemu_types::DisassembledInstruction;

/// Addressing mode for 6502 instructions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AddressingMode {
    Implied,
    Accumulator,
    Immediate,
    ZeroPage,
    ZeroPageX,
    ZeroPageY,
    Absolute,
    AbsoluteX,
    AbsoluteY,
    Indirect,
    IndirectX,
    IndirectY,
    Relative,
}

/// Instruction definition
struct Instruction {
    mnemonic: &'static str,
    mode: AddressingMode,
    bytes: u8,
}

/// Get instruction info for a given opcode
fn get_instruction(opcode: u8) -> Instruction {
    use AddressingMode::*;

    match opcode {
        // ADC
        0x69 => Instruction {
            mnemonic: "ADC",
            mode: Immediate,
            bytes: 2,
        },
        0x65 => Instruction {
            mnemonic: "ADC",
            mode: ZeroPage,
            bytes: 2,
        },
        0x75 => Instruction {
            mnemonic: "ADC",
            mode: ZeroPageX,
            bytes: 2,
        },
        0x6D => Instruction {
            mnemonic: "ADC",
            mode: Absolute,
            bytes: 3,
        },
        0x7D => Instruction {
            mnemonic: "ADC",
            mode: AbsoluteX,
            bytes: 3,
        },
        0x79 => Instruction {
            mnemonic: "ADC",
            mode: AbsoluteY,
            bytes: 3,
        },
        0x61 => Instruction {
            mnemonic: "ADC",
            mode: IndirectX,
            bytes: 2,
        },
        0x71 => Instruction {
            mnemonic: "ADC",
            mode: IndirectY,
            bytes: 2,
        },

        // AND
        0x29 => Instruction {
            mnemonic: "AND",
            mode: Immediate,
            bytes: 2,
        },
        0x25 => Instruction {
            mnemonic: "AND",
            mode: ZeroPage,
            bytes: 2,
        },
        0x35 => Instruction {
            mnemonic: "AND",
            mode: ZeroPageX,
            bytes: 2,
        },
        0x2D => Instruction {
            mnemonic: "AND",
            mode: Absolute,
            bytes: 3,
        },
        0x3D => Instruction {
            mnemonic: "AND",
            mode: AbsoluteX,
            bytes: 3,
        },
        0x39 => Instruction {
            mnemonic: "AND",
            mode: AbsoluteY,
            bytes: 3,
        },
        0x21 => Instruction {
            mnemonic: "AND",
            mode: IndirectX,
            bytes: 2,
        },
        0x31 => Instruction {
            mnemonic: "AND",
            mode: IndirectY,
            bytes: 2,
        },

        // ASL
        0x0A => Instruction {
            mnemonic: "ASL",
            mode: Accumulator,
            bytes: 1,
        },
        0x06 => Instruction {
            mnemonic: "ASL",
            mode: ZeroPage,
            bytes: 2,
        },
        0x16 => Instruction {
            mnemonic: "ASL",
            mode: ZeroPageX,
            bytes: 2,
        },
        0x0E => Instruction {
            mnemonic: "ASL",
            mode: Absolute,
            bytes: 3,
        },
        0x1E => Instruction {
            mnemonic: "ASL",
            mode: AbsoluteX,
            bytes: 3,
        },

        // BCC, BCS, BEQ, BMI, BNE, BPL, BVC, BVS (branches)
        0x90 => Instruction {
            mnemonic: "BCC",
            mode: Relative,
            bytes: 2,
        },
        0xB0 => Instruction {
            mnemonic: "BCS",
            mode: Relative,
            bytes: 2,
        },
        0xF0 => Instruction {
            mnemonic: "BEQ",
            mode: Relative,
            bytes: 2,
        },
        0x30 => Instruction {
            mnemonic: "BMI",
            mode: Relative,
            bytes: 2,
        },
        0xD0 => Instruction {
            mnemonic: "BNE",
            mode: Relative,
            bytes: 2,
        },
        0x10 => Instruction {
            mnemonic: "BPL",
            mode: Relative,
            bytes: 2,
        },
        0x50 => Instruction {
            mnemonic: "BVC",
            mode: Relative,
            bytes: 2,
        },
        0x70 => Instruction {
            mnemonic: "BVS",
            mode: Relative,
            bytes: 2,
        },

        // BIT
        0x24 => Instruction {
            mnemonic: "BIT",
            mode: ZeroPage,
            bytes: 2,
        },
        0x2C => Instruction {
            mnemonic: "BIT",
            mode: Absolute,
            bytes: 3,
        },

        // BRK
        0x00 => Instruction {
            mnemonic: "BRK",
            mode: Implied,
            bytes: 1,
        },

        // CLC, CLD, CLI, CLV
        0x18 => Instruction {
            mnemonic: "CLC",
            mode: Implied,
            bytes: 1,
        },
        0xD8 => Instruction {
            mnemonic: "CLD",
            mode: Implied,
            bytes: 1,
        },
        0x58 => Instruction {
            mnemonic: "CLI",
            mode: Implied,
            bytes: 1,
        },
        0xB8 => Instruction {
            mnemonic: "CLV",
            mode: Implied,
            bytes: 1,
        },

        // CMP
        0xC9 => Instruction {
            mnemonic: "CMP",
            mode: Immediate,
            bytes: 2,
        },
        0xC5 => Instruction {
            mnemonic: "CMP",
            mode: ZeroPage,
            bytes: 2,
        },
        0xD5 => Instruction {
            mnemonic: "CMP",
            mode: ZeroPageX,
            bytes: 2,
        },
        0xCD => Instruction {
            mnemonic: "CMP",
            mode: Absolute,
            bytes: 3,
        },
        0xDD => Instruction {
            mnemonic: "CMP",
            mode: AbsoluteX,
            bytes: 3,
        },
        0xD9 => Instruction {
            mnemonic: "CMP",
            mode: AbsoluteY,
            bytes: 3,
        },
        0xC1 => Instruction {
            mnemonic: "CMP",
            mode: IndirectX,
            bytes: 2,
        },
        0xD1 => Instruction {
            mnemonic: "CMP",
            mode: IndirectY,
            bytes: 2,
        },

        // CPX
        0xE0 => Instruction {
            mnemonic: "CPX",
            mode: Immediate,
            bytes: 2,
        },
        0xE4 => Instruction {
            mnemonic: "CPX",
            mode: ZeroPage,
            bytes: 2,
        },
        0xEC => Instruction {
            mnemonic: "CPX",
            mode: Absolute,
            bytes: 3,
        },

        // CPY
        0xC0 => Instruction {
            mnemonic: "CPY",
            mode: Immediate,
            bytes: 2,
        },
        0xC4 => Instruction {
            mnemonic: "CPY",
            mode: ZeroPage,
            bytes: 2,
        },
        0xCC => Instruction {
            mnemonic: "CPY",
            mode: Absolute,
            bytes: 3,
        },

        // DEC
        0xC6 => Instruction {
            mnemonic: "DEC",
            mode: ZeroPage,
            bytes: 2,
        },
        0xD6 => Instruction {
            mnemonic: "DEC",
            mode: ZeroPageX,
            bytes: 2,
        },
        0xCE => Instruction {
            mnemonic: "DEC",
            mode: Absolute,
            bytes: 3,
        },
        0xDE => Instruction {
            mnemonic: "DEC",
            mode: AbsoluteX,
            bytes: 3,
        },

        // DEX, DEY
        0xCA => Instruction {
            mnemonic: "DEX",
            mode: Implied,
            bytes: 1,
        },
        0x88 => Instruction {
            mnemonic: "DEY",
            mode: Implied,
            bytes: 1,
        },

        // EOR
        0x49 => Instruction {
            mnemonic: "EOR",
            mode: Immediate,
            bytes: 2,
        },
        0x45 => Instruction {
            mnemonic: "EOR",
            mode: ZeroPage,
            bytes: 2,
        },
        0x55 => Instruction {
            mnemonic: "EOR",
            mode: ZeroPageX,
            bytes: 2,
        },
        0x4D => Instruction {
            mnemonic: "EOR",
            mode: Absolute,
            bytes: 3,
        },
        0x5D => Instruction {
            mnemonic: "EOR",
            mode: AbsoluteX,
            bytes: 3,
        },
        0x59 => Instruction {
            mnemonic: "EOR",
            mode: AbsoluteY,
            bytes: 3,
        },
        0x41 => Instruction {
            mnemonic: "EOR",
            mode: IndirectX,
            bytes: 2,
        },
        0x51 => Instruction {
            mnemonic: "EOR",
            mode: IndirectY,
            bytes: 2,
        },

        // INC
        0xE6 => Instruction {
            mnemonic: "INC",
            mode: ZeroPage,
            bytes: 2,
        },
        0xF6 => Instruction {
            mnemonic: "INC",
            mode: ZeroPageX,
            bytes: 2,
        },
        0xEE => Instruction {
            mnemonic: "INC",
            mode: Absolute,
            bytes: 3,
        },
        0xFE => Instruction {
            mnemonic: "INC",
            mode: AbsoluteX,
            bytes: 3,
        },

        // INX, INY
        0xE8 => Instruction {
            mnemonic: "INX",
            mode: Implied,
            bytes: 1,
        },
        0xC8 => Instruction {
            mnemonic: "INY",
            mode: Implied,
            bytes: 1,
        },

        // JMP
        0x4C => Instruction {
            mnemonic: "JMP",
            mode: Absolute,
            bytes: 3,
        },
        0x6C => Instruction {
            mnemonic: "JMP",
            mode: Indirect,
            bytes: 3,
        },

        // JSR
        0x20 => Instruction {
            mnemonic: "JSR",
            mode: Absolute,
            bytes: 3,
        },

        // LDA
        0xA9 => Instruction {
            mnemonic: "LDA",
            mode: Immediate,
            bytes: 2,
        },
        0xA5 => Instruction {
            mnemonic: "LDA",
            mode: ZeroPage,
            bytes: 2,
        },
        0xB5 => Instruction {
            mnemonic: "LDA",
            mode: ZeroPageX,
            bytes: 2,
        },
        0xAD => Instruction {
            mnemonic: "LDA",
            mode: Absolute,
            bytes: 3,
        },
        0xBD => Instruction {
            mnemonic: "LDA",
            mode: AbsoluteX,
            bytes: 3,
        },
        0xB9 => Instruction {
            mnemonic: "LDA",
            mode: AbsoluteY,
            bytes: 3,
        },
        0xA1 => Instruction {
            mnemonic: "LDA",
            mode: IndirectX,
            bytes: 2,
        },
        0xB1 => Instruction {
            mnemonic: "LDA",
            mode: IndirectY,
            bytes: 2,
        },

        // LDX
        0xA2 => Instruction {
            mnemonic: "LDX",
            mode: Immediate,
            bytes: 2,
        },
        0xA6 => Instruction {
            mnemonic: "LDX",
            mode: ZeroPage,
            bytes: 2,
        },
        0xB6 => Instruction {
            mnemonic: "LDX",
            mode: ZeroPageY,
            bytes: 2,
        },
        0xAE => Instruction {
            mnemonic: "LDX",
            mode: Absolute,
            bytes: 3,
        },
        0xBE => Instruction {
            mnemonic: "LDX",
            mode: AbsoluteY,
            bytes: 3,
        },

        // LDY
        0xA0 => Instruction {
            mnemonic: "LDY",
            mode: Immediate,
            bytes: 2,
        },
        0xA4 => Instruction {
            mnemonic: "LDY",
            mode: ZeroPage,
            bytes: 2,
        },
        0xB4 => Instruction {
            mnemonic: "LDY",
            mode: ZeroPageX,
            bytes: 2,
        },
        0xAC => Instruction {
            mnemonic: "LDY",
            mode: Absolute,
            bytes: 3,
        },
        0xBC => Instruction {
            mnemonic: "LDY",
            mode: AbsoluteX,
            bytes: 3,
        },

        // LSR
        0x4A => Instruction {
            mnemonic: "LSR",
            mode: Accumulator,
            bytes: 1,
        },
        0x46 => Instruction {
            mnemonic: "LSR",
            mode: ZeroPage,
            bytes: 2,
        },
        0x56 => Instruction {
            mnemonic: "LSR",
            mode: ZeroPageX,
            bytes: 2,
        },
        0x4E => Instruction {
            mnemonic: "LSR",
            mode: Absolute,
            bytes: 3,
        },
        0x5E => Instruction {
            mnemonic: "LSR",
            mode: AbsoluteX,
            bytes: 3,
        },

        // NOP
        0xEA => Instruction {
            mnemonic: "NOP",
            mode: Implied,
            bytes: 1,
        },

        // ORA
        0x09 => Instruction {
            mnemonic: "ORA",
            mode: Immediate,
            bytes: 2,
        },
        0x05 => Instruction {
            mnemonic: "ORA",
            mode: ZeroPage,
            bytes: 2,
        },
        0x15 => Instruction {
            mnemonic: "ORA",
            mode: ZeroPageX,
            bytes: 2,
        },
        0x0D => Instruction {
            mnemonic: "ORA",
            mode: Absolute,
            bytes: 3,
        },
        0x1D => Instruction {
            mnemonic: "ORA",
            mode: AbsoluteX,
            bytes: 3,
        },
        0x19 => Instruction {
            mnemonic: "ORA",
            mode: AbsoluteY,
            bytes: 3,
        },
        0x01 => Instruction {
            mnemonic: "ORA",
            mode: IndirectX,
            bytes: 2,
        },
        0x11 => Instruction {
            mnemonic: "ORA",
            mode: IndirectY,
            bytes: 2,
        },

        // PHA, PHP, PLA, PLP
        0x48 => Instruction {
            mnemonic: "PHA",
            mode: Implied,
            bytes: 1,
        },
        0x08 => Instruction {
            mnemonic: "PHP",
            mode: Implied,
            bytes: 1,
        },
        0x68 => Instruction {
            mnemonic: "PLA",
            mode: Implied,
            bytes: 1,
        },
        0x28 => Instruction {
            mnemonic: "PLP",
            mode: Implied,
            bytes: 1,
        },

        // ROL
        0x2A => Instruction {
            mnemonic: "ROL",
            mode: Accumulator,
            bytes: 1,
        },
        0x26 => Instruction {
            mnemonic: "ROL",
            mode: ZeroPage,
            bytes: 2,
        },
        0x36 => Instruction {
            mnemonic: "ROL",
            mode: ZeroPageX,
            bytes: 2,
        },
        0x2E => Instruction {
            mnemonic: "ROL",
            mode: Absolute,
            bytes: 3,
        },
        0x3E => Instruction {
            mnemonic: "ROL",
            mode: AbsoluteX,
            bytes: 3,
        },

        // ROR
        0x6A => Instruction {
            mnemonic: "ROR",
            mode: Accumulator,
            bytes: 1,
        },
        0x66 => Instruction {
            mnemonic: "ROR",
            mode: ZeroPage,
            bytes: 2,
        },
        0x76 => Instruction {
            mnemonic: "ROR",
            mode: ZeroPageX,
            bytes: 2,
        },
        0x6E => Instruction {
            mnemonic: "ROR",
            mode: Absolute,
            bytes: 3,
        },
        0x7E => Instruction {
            mnemonic: "ROR",
            mode: AbsoluteX,
            bytes: 3,
        },

        // RTI, RTS
        0x40 => Instruction {
            mnemonic: "RTI",
            mode: Implied,
            bytes: 1,
        },
        0x60 => Instruction {
            mnemonic: "RTS",
            mode: Implied,
            bytes: 1,
        },

        // SBC
        0xE9 => Instruction {
            mnemonic: "SBC",
            mode: Immediate,
            bytes: 2,
        },
        0xE5 => Instruction {
            mnemonic: "SBC",
            mode: ZeroPage,
            bytes: 2,
        },
        0xF5 => Instruction {
            mnemonic: "SBC",
            mode: ZeroPageX,
            bytes: 2,
        },
        0xED => Instruction {
            mnemonic: "SBC",
            mode: Absolute,
            bytes: 3,
        },
        0xFD => Instruction {
            mnemonic: "SBC",
            mode: AbsoluteX,
            bytes: 3,
        },
        0xF9 => Instruction {
            mnemonic: "SBC",
            mode: AbsoluteY,
            bytes: 3,
        },
        0xE1 => Instruction {
            mnemonic: "SBC",
            mode: IndirectX,
            bytes: 2,
        },
        0xF1 => Instruction {
            mnemonic: "SBC",
            mode: IndirectY,
            bytes: 2,
        },

        // SEC, SED, SEI
        0x38 => Instruction {
            mnemonic: "SEC",
            mode: Implied,
            bytes: 1,
        },
        0xF8 => Instruction {
            mnemonic: "SED",
            mode: Implied,
            bytes: 1,
        },
        0x78 => Instruction {
            mnemonic: "SEI",
            mode: Implied,
            bytes: 1,
        },

        // STA
        0x85 => Instruction {
            mnemonic: "STA",
            mode: ZeroPage,
            bytes: 2,
        },
        0x95 => Instruction {
            mnemonic: "STA",
            mode: ZeroPageX,
            bytes: 2,
        },
        0x8D => Instruction {
            mnemonic: "STA",
            mode: Absolute,
            bytes: 3,
        },
        0x9D => Instruction {
            mnemonic: "STA",
            mode: AbsoluteX,
            bytes: 3,
        },
        0x99 => Instruction {
            mnemonic: "STA",
            mode: AbsoluteY,
            bytes: 3,
        },
        0x81 => Instruction {
            mnemonic: "STA",
            mode: IndirectX,
            bytes: 2,
        },
        0x91 => Instruction {
            mnemonic: "STA",
            mode: IndirectY,
            bytes: 2,
        },

        // STX
        0x86 => Instruction {
            mnemonic: "STX",
            mode: ZeroPage,
            bytes: 2,
        },
        0x96 => Instruction {
            mnemonic: "STX",
            mode: ZeroPageY,
            bytes: 2,
        },
        0x8E => Instruction {
            mnemonic: "STX",
            mode: Absolute,
            bytes: 3,
        },

        // STY
        0x84 => Instruction {
            mnemonic: "STY",
            mode: ZeroPage,
            bytes: 2,
        },
        0x94 => Instruction {
            mnemonic: "STY",
            mode: ZeroPageX,
            bytes: 2,
        },
        0x8C => Instruction {
            mnemonic: "STY",
            mode: Absolute,
            bytes: 3,
        },

        // TAX, TAY, TSX, TXA, TXS, TYA
        0xAA => Instruction {
            mnemonic: "TAX",
            mode: Implied,
            bytes: 1,
        },
        0xA8 => Instruction {
            mnemonic: "TAY",
            mode: Implied,
            bytes: 1,
        },
        0xBA => Instruction {
            mnemonic: "TSX",
            mode: Implied,
            bytes: 1,
        },
        0x8A => Instruction {
            mnemonic: "TXA",
            mode: Implied,
            bytes: 1,
        },
        0x9A => Instruction {
            mnemonic: "TXS",
            mode: Implied,
            bytes: 1,
        },
        0x98 => Instruction {
            mnemonic: "TYA",
            mode: Implied,
            bytes: 1,
        },

        // Unknown/illegal opcodes - treat as single-byte NOP
        _ => Instruction {
            mnemonic: "???",
            mode: Implied,
            bytes: 1,
        },
    }
}

/// Format an operand based on addressing mode
fn format_operand(mode: AddressingMode, operand_bytes: &[u8]) -> String {
    use AddressingMode::*;

    match mode {
        Implied => String::new(),
        Accumulator => "A".to_string(),
        Immediate => {
            if !operand_bytes.is_empty() {
                format!("#${:02X}", operand_bytes[0])
            } else {
                "#$??".to_string()
            }
        }
        ZeroPage => {
            if !operand_bytes.is_empty() {
                format!("${:02X}", operand_bytes[0])
            } else {
                "$??".to_string()
            }
        }
        ZeroPageX => {
            if !operand_bytes.is_empty() {
                format!("${:02X},X", operand_bytes[0])
            } else {
                "$??,X".to_string()
            }
        }
        ZeroPageY => {
            if !operand_bytes.is_empty() {
                format!("${:02X},Y", operand_bytes[0])
            } else {
                "$??,Y".to_string()
            }
        }
        Absolute => {
            if operand_bytes.len() >= 2 {
                let addr = u16::from_le_bytes([operand_bytes[0], operand_bytes[1]]);
                format!("${:04X}", addr)
            } else {
                "$????".to_string()
            }
        }
        AbsoluteX => {
            if operand_bytes.len() >= 2 {
                let addr = u16::from_le_bytes([operand_bytes[0], operand_bytes[1]]);
                format!("${:04X},X", addr)
            } else {
                "$????,X".to_string()
            }
        }
        AbsoluteY => {
            if operand_bytes.len() >= 2 {
                let addr = u16::from_le_bytes([operand_bytes[0], operand_bytes[1]]);
                format!("${:04X},Y", addr)
            } else {
                "$????,Y".to_string()
            }
        }
        Indirect => {
            if operand_bytes.len() >= 2 {
                let addr = u16::from_le_bytes([operand_bytes[0], operand_bytes[1]]);
                format!("(${:04X})", addr)
            } else {
                "($????)".to_string()
            }
        }
        IndirectX => {
            if !operand_bytes.is_empty() {
                format!("(${:02X},X)", operand_bytes[0])
            } else {
                "($??,X)".to_string()
            }
        }
        IndirectY => {
            if !operand_bytes.is_empty() {
                format!("(${:02X}),Y", operand_bytes[0])
            } else {
                "($??),Y".to_string()
            }
        }
        Relative => {
            if !operand_bytes.is_empty() {
                let offset = operand_bytes[0] as i8;
                if offset >= 0 {
                    format!("+${:02X}", offset)
                } else {
                    format!("-${:02X}", -offset)
                }
            } else {
                "+$??".to_string()
            }
        }
    }
}

/// Disassemble a 6502 instruction from memory
///
/// # Arguments
/// * `memory` - Byte slice containing the instruction and operands
/// * `address` - Address where the instruction is located (for display)
///
/// # Returns
/// A `DisassembledInstruction` with the disassembled instruction, or None if memory is empty
pub fn disassemble_6502(memory: &[u8], address: u32) -> Option<DisassembledInstruction> {
    if memory.is_empty() {
        return None;
    }

    let opcode = memory[0];
    let instr = get_instruction(opcode);

    // Extract operand bytes
    let operand_count = (instr.bytes as usize).saturating_sub(1);
    let operand_bytes: Vec<u8> = memory.get(1..=operand_count).unwrap_or(&[]).to_vec();

    // Get all bytes for this instruction
    let mut all_bytes = vec![opcode];
    all_bytes.extend_from_slice(&operand_bytes);

    // Format the mnemonic with operand
    let operand_str = format_operand(instr.mode, &operand_bytes);
    let mnemonic = if operand_str.is_empty() {
        instr.mnemonic.to_string()
    } else {
        format!("{} {}", instr.mnemonic, operand_str)
    };

    let mut result = DisassembledInstruction::new(address, all_bytes, mnemonic);

    // For relative branches, add target address as a comment
    if instr.mode == AddressingMode::Relative && !operand_bytes.is_empty() {
        let offset = operand_bytes[0] as i8;
        // Target address = (PC after instruction) + offset
        // PC after instruction = current address + instruction length (2 bytes for branches)
        let target = (address as i32 + 2 + offset as i32) as u32;
        result = result.with_comment(format!("-> ${:04X}", target));
    }

    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disassemble_lda_immediate() {
        let memory = [0xA9, 0x42];
        let instr = disassemble_6502(&memory, 0x8000).unwrap();
        assert_eq!(instr.address, 0x8000);
        assert_eq!(instr.bytes, vec![0xA9, 0x42]);
        assert_eq!(instr.mnemonic, "LDA #$42");
        assert_eq!(instr.len(), 2);
    }

    #[test]
    fn test_disassemble_jmp_absolute() {
        let memory = [0x4C, 0x00, 0x80];
        let instr = disassemble_6502(&memory, 0xC000).unwrap();
        assert_eq!(instr.address, 0xC000);
        assert_eq!(instr.bytes, vec![0x4C, 0x00, 0x80]);
        assert_eq!(instr.mnemonic, "JMP $8000");
        assert_eq!(instr.len(), 3);
    }

    #[test]
    fn test_disassemble_implied() {
        let memory = [0xEA]; // NOP
        let instr = disassemble_6502(&memory, 0x1000).unwrap();
        assert_eq!(instr.mnemonic, "NOP");
        assert_eq!(instr.len(), 1);
    }

    #[test]
    fn test_disassemble_indexed() {
        let memory = [0xBD, 0x00, 0x02]; // LDA $0200,X
        let instr = disassemble_6502(&memory, 0x8000).unwrap();
        assert_eq!(instr.mnemonic, "LDA $0200,X");
        assert_eq!(instr.len(), 3);
    }

    #[test]
    fn test_disassemble_relative() {
        let memory = [0x90, 0x10]; // BCC +$10
        let instr = disassemble_6502(&memory, 0x8000).unwrap();
        assert_eq!(instr.mnemonic, "BCC +$10");
        assert_eq!(instr.len(), 2);
        // Target = 0x8000 + 2 + 0x10 = 0x8012
        assert_eq!(instr.comment, Some("-> $8012".to_string()));

        let memory_neg = [0x90, 0xF0]; // BCC -$10
        let instr_neg = disassemble_6502(&memory_neg, 0x8000).unwrap();
        assert_eq!(instr_neg.mnemonic, "BCC -$10");
        // Target = 0x8000 + 2 + (-16) = 0x7FF2
        assert_eq!(instr_neg.comment, Some("-> $7FF2".to_string()));
    }

    #[test]
    fn test_disassemble_empty() {
        let memory = [];
        let instr = disassemble_6502(&memory, 0x8000);
        assert!(instr.is_none());
    }

    #[test]
    fn test_disassemble_incomplete() {
        // Only 2 bytes provided for a 3-byte instruction
        let memory = [0x4C, 0x00]; // JMP with missing byte
        let instr = disassemble_6502(&memory, 0x8000).unwrap();
        // Should still disassemble but with placeholder
        assert_eq!(instr.mnemonic, "JMP $????");
        // We only have the bytes that were provided
        assert!(instr.len() <= 3);
    }
}
