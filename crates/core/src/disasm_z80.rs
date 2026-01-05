//! Z80 CPU disassembler
//!
//! Provides instruction disassembly for the Zilog Z80 CPU used in SMS and other systems.

use crate::debug::DisassembledInstruction;

/// Disassemble a single Z80 instruction from memory
pub fn disassemble_z80(memory: &[u8], address: u32) -> Option<DisassembledInstruction> {
    if memory.is_empty() {
        return None;
    }

    let opcode = memory[0];

    // Handle extended instructions
    match opcode {
        0xCB => return disassemble_cb_instruction(memory, address),
        0xDD => return disassemble_dd_instruction(memory, address), // IX prefix
        0xED => return disassemble_ed_instruction(memory, address),
        0xFD => return disassemble_fd_instruction(memory, address), // IY prefix
        _ => {}
    }

    // Main opcode table (simplified - covers common opcodes)
    let (mnemonic, len) = match opcode {
        // 8-bit loads
        0x06 => ("LD B, n".to_string(), 2),
        0x0E => ("LD C, n".to_string(), 2),
        0x16 => ("LD D, n".to_string(), 2),
        0x1E => ("LD E, n".to_string(), 2),
        0x26 => ("LD H, n".to_string(), 2),
        0x2E => ("LD L, n".to_string(), 2),
        0x3E => ("LD A, n".to_string(), 2),

        // 16-bit loads
        0x01 => ("LD BC, nn".to_string(), 3),
        0x11 => ("LD DE, nn".to_string(), 3),
        0x21 => ("LD HL, nn".to_string(), 3),
        0x31 => ("LD SP, nn".to_string(), 3),

        // Stack operations
        0xC5 => ("PUSH BC".to_string(), 1),
        0xD5 => ("PUSH DE".to_string(), 1),
        0xE5 => ("PUSH HL".to_string(), 1),
        0xF5 => ("PUSH AF".to_string(), 1),
        0xC1 => ("POP BC".to_string(), 1),
        0xD1 => ("POP DE".to_string(), 1),
        0xE1 => ("POP HL".to_string(), 1),
        0xF1 => ("POP AF".to_string(), 1),

        // Arithmetic
        0x04 => ("INC B".to_string(), 1),
        0x0C => ("INC C".to_string(), 1),
        0x14 => ("INC D".to_string(), 1),
        0x1C => ("INC E".to_string(), 1),
        0x24 => ("INC H".to_string(), 1),
        0x2C => ("INC L".to_string(), 1),
        0x34 => ("INC (HL)".to_string(), 1),
        0x3C => ("INC A".to_string(), 1),

        0x05 => ("DEC B".to_string(), 1),
        0x0D => ("DEC C".to_string(), 1),
        0x15 => ("DEC D".to_string(), 1),
        0x1D => ("DEC E".to_string(), 1),
        0x25 => ("DEC H".to_string(), 1),
        0x2D => ("DEC L".to_string(), 1),
        0x35 => ("DEC (HL)".to_string(), 1),
        0x3D => ("DEC A".to_string(), 1),

        // Control flow
        0x00 => ("NOP".to_string(), 1),
        0x76 => ("HALT".to_string(), 1),
        0xC3 => ("JP nn".to_string(), 3),
        0xCD => ("CALL nn".to_string(), 3),
        0xC9 => ("RET".to_string(), 1),

        0xC0 => ("RET NZ".to_string(), 1),
        0xC8 => ("RET Z".to_string(), 1),
        0xD0 => ("RET NC".to_string(), 1),
        0xD8 => ("RET C".to_string(), 1),
        0xE0 => ("RET PO".to_string(), 1),
        0xE8 => ("RET PE".to_string(), 1),
        0xF0 => ("RET P".to_string(), 1),
        0xF8 => ("RET M".to_string(), 1),

        0xC2 => ("JP NZ, nn".to_string(), 3),
        0xCA => ("JP Z, nn".to_string(), 3),
        0xD2 => ("JP NC, nn".to_string(), 3),
        0xDA => ("JP C, nn".to_string(), 3),

        0x18 => ("JR e".to_string(), 2),
        0x20 => ("JR NZ, e".to_string(), 2),
        0x28 => ("JR Z, e".to_string(), 2),
        0x30 => ("JR NC, e".to_string(), 2),
        0x38 => ("JR C, e".to_string(), 2),

        // I/O
        0xD3 => ("OUT (n), A".to_string(), 2),
        0xDB => ("IN A, (n)".to_string(), 2),

        // Misc
        0x27 => ("DAA".to_string(), 1),
        0x2F => ("CPL".to_string(), 1),
        0x37 => ("SCF".to_string(), 1),
        0x3F => ("CCF".to_string(), 1),
        0xF3 => ("DI".to_string(), 1),
        0xFB => ("EI".to_string(), 1),

        // RST instructions
        0xC7 => ("RST 00H".to_string(), 1),
        0xCF => ("RST 08H".to_string(), 1),
        0xD7 => ("RST 10H".to_string(), 1),
        0xDF => ("RST 18H".to_string(), 1),
        0xE7 => ("RST 20H".to_string(), 1),
        0xEF => ("RST 28H".to_string(), 1),
        0xF7 => ("RST 30H".to_string(), 1),
        0xFF => ("RST 38H".to_string(), 1),

        _ => (format!("DB ${:02X}", opcode), 1),
    };

    let bytes: Vec<u8> = memory.iter().take(len).copied().collect();
    Some(DisassembledInstruction::new(address, bytes, mnemonic))
}

fn disassemble_cb_instruction(memory: &[u8], address: u32) -> Option<DisassembledInstruction> {
    if memory.len() < 2 {
        return None;
    }

    let cb_opcode = memory[1];
    let bit = (cb_opcode >> 3) & 0x07;
    let reg = cb_opcode & 0x07;

    let reg_name = match reg {
        0 => "B",
        1 => "C",
        2 => "D",
        3 => "E",
        4 => "H",
        5 => "L",
        6 => "(HL)",
        7 => "A",
        _ => "?",
    };

    let mnemonic = match cb_opcode & 0xC0 {
        0x00 => {
            // Rotates and shifts
            match (cb_opcode >> 3) & 0x07 {
                0 => format!("RLC {}", reg_name),
                1 => format!("RRC {}", reg_name),
                2 => format!("RL {}", reg_name),
                3 => format!("RR {}", reg_name),
                4 => format!("SLA {}", reg_name),
                5 => format!("SRA {}", reg_name),
                6 => format!("SLL {}", reg_name), // Undocumented
                7 => format!("SRL {}", reg_name),
                _ => format!("CB {:02X}", cb_opcode),
            }
        }
        0x40 => format!("BIT {}, {}", bit, reg_name),
        0x80 => format!("RES {}, {}", bit, reg_name),
        0xC0 => format!("SET {}, {}", bit, reg_name),
        _ => format!("CB {:02X}", cb_opcode),
    };

    Some(DisassembledInstruction::new(
        address,
        vec![0xCB, cb_opcode],
        mnemonic,
    ))
}

fn disassemble_dd_instruction(memory: &[u8], address: u32) -> Option<DisassembledInstruction> {
    if memory.len() < 2 {
        return None;
    }
    // Simplified IX instructions
    Some(DisassembledInstruction::new(
        address,
        vec![memory[0], memory[1]],
        format!("IX-prefix {:02X}", memory[1]),
    ))
}

fn disassemble_ed_instruction(memory: &[u8], address: u32) -> Option<DisassembledInstruction> {
    if memory.len() < 2 {
        return None;
    }
    let ed_opcode = memory[1];
    let (mnemonic, len) = match ed_opcode {
        0x44 => ("NEG".to_string(), 2),
        0x46 => ("IM 0".to_string(), 2),
        0x56 => ("IM 1".to_string(), 2),
        0x5E => ("IM 2".to_string(), 2),
        0xA0 => ("LDI".to_string(), 2),
        0xA1 => ("CPI".to_string(), 2),
        0xA2 => ("INI".to_string(), 2),
        0xA3 => ("OUTI".to_string(), 2),
        0xB0 => ("LDIR".to_string(), 2),
        0xB1 => ("CPIR".to_string(), 2),
        _ => (format!("ED {:02X}", ed_opcode), 2),
    };

    let bytes: Vec<u8> = memory.iter().take(len).copied().collect();
    Some(DisassembledInstruction::new(address, bytes, mnemonic))
}

fn disassemble_fd_instruction(memory: &[u8], address: u32) -> Option<DisassembledInstruction> {
    if memory.len() < 2 {
        return None;
    }
    // Simplified IY instructions
    Some(DisassembledInstruction::new(
        address,
        vec![memory[0], memory[1]],
        format!("IY-prefix {:02X}", memory[1]),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disassemble_nop() {
        let memory = [0x00];
        let instr = disassemble_z80(&memory, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "NOP");
    }

    #[test]
    fn test_disassemble_ld() {
        let memory = [0x3E, 0x42];
        let instr = disassemble_z80(&memory, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "LD A, n");
        assert_eq!(instr.len(), 2);
    }

    #[test]
    fn test_disassemble_jp() {
        let memory = [0xC3, 0x00, 0x10];
        let instr = disassemble_z80(&memory, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "JP nn");
        assert_eq!(instr.len(), 3);
    }

    #[test]
    fn test_disassemble_cb_bit() {
        let memory = [0xCB, 0x47];
        let instr = disassemble_z80(&memory, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "BIT 0, A");
    }

    #[test]
    fn test_disassemble_ed() {
        let memory = [0xED, 0x44];
        let instr = disassemble_z80(&memory, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "NEG");
    }
}
