//! LR35902 (Game Boy CPU) disassembler
//!
//! Provides instruction disassembly for the Sharp LR35902 CPU used in Game Boy.
//! This is a Z80-like CPU with some instructions removed and modified.

use hemu_types::DisassembledInstruction;

/// Disassemble a single LR35902 instruction from memory
pub fn disassemble_lr35902(memory: &[u8], address: u32) -> Option<DisassembledInstruction> {
    if memory.is_empty() {
        return None;
    }

    let opcode = memory[0];

    // Handle CB-prefixed instructions (bit operations)
    if opcode == 0xCB {
        if memory.len() < 2 {
            return None;
        }
        return disassemble_cb_instruction(memory, address);
    }

    let (mnemonic, len) = match opcode {
        // 8-bit loads
        0x06 => ("LD B, d8".to_string(), 2),
        0x0E => ("LD C, d8".to_string(), 2),
        0x16 => ("LD D, d8".to_string(), 2),
        0x1E => ("LD E, d8".to_string(), 2),
        0x26 => ("LD H, d8".to_string(), 2),
        0x2E => ("LD L, d8".to_string(), 2),
        0x3E => ("LD A, d8".to_string(), 2),

        // 16-bit loads
        0x01 => ("LD BC, d16".to_string(), 3),
        0x11 => ("LD DE, d16".to_string(), 3),
        0x21 => ("LD HL, d16".to_string(), 3),
        0x31 => ("LD SP, d16".to_string(), 3),

        // Memory operations
        0x02 => ("LD (BC), A".to_string(), 1),
        0x12 => ("LD (DE), A".to_string(), 1),
        0x22 => ("LD (HL+), A".to_string(), 1),
        0x32 => ("LD (HL-), A".to_string(), 1),
        0x0A => ("LD A, (BC)".to_string(), 1),
        0x1A => ("LD A, (DE)".to_string(), 1),
        0x2A => ("LD A, (HL+)".to_string(), 1),
        0x3A => ("LD A, (HL-)".to_string(), 1),

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
        0x10 => ("STOP".to_string(), 2),

        0xC3 => ("JP a16".to_string(), 3),
        0xCD => ("CALL a16".to_string(), 3),
        0xC9 => ("RET".to_string(), 1),

        0xC0 => ("RET NZ".to_string(), 1),
        0xC8 => ("RET Z".to_string(), 1),
        0xD0 => ("RET NC".to_string(), 1),
        0xD8 => ("RET C".to_string(), 1),

        0xC2 => ("JP NZ, a16".to_string(), 3),
        0xCA => ("JP Z, a16".to_string(), 3),
        0xD2 => ("JP NC, a16".to_string(), 3),
        0xDA => ("JP C, a16".to_string(), 3),

        0x20 => ("JR NZ, r8".to_string(), 2),
        0x28 => ("JR Z, r8".to_string(), 2),
        0x30 => ("JR NC, r8".to_string(), 2),
        0x38 => ("JR C, r8".to_string(), 2),
        0x18 => ("JR r8".to_string(), 2),

        // Stack operations
        0xC5 => ("PUSH BC".to_string(), 1),
        0xD5 => ("PUSH DE".to_string(), 1),
        0xE5 => ("PUSH HL".to_string(), 1),
        0xF5 => ("PUSH AF".to_string(), 1),

        0xC1 => ("POP BC".to_string(), 1),
        0xD1 => ("POP DE".to_string(), 1),
        0xE1 => ("POP HL".to_string(), 1),
        0xF1 => ("POP AF".to_string(), 1),

        // I/O
        0xE0 => ("LDH (a8), A".to_string(), 2),
        0xF0 => ("LDH A, (a8)".to_string(), 2),
        0xE2 => ("LD (C), A".to_string(), 1),
        0xF2 => ("LD A, (C)".to_string(), 1),

        0xEA => ("LD (a16), A".to_string(), 3),
        0xFA => ("LD A, (a16)".to_string(), 3),

        // Misc
        0x27 => ("DAA".to_string(), 1),
        0x2F => ("CPL".to_string(), 1),
        0x37 => ("SCF".to_string(), 1),
        0x3F => ("CCF".to_string(), 1),
        0xF3 => ("DI".to_string(), 1),
        0xFB => ("EI".to_string(), 1),

        // ADD/ADC/SUB/SBC with immediate
        0xC6 => ("ADD A, d8".to_string(), 2),
        0xCE => ("ADC A, d8".to_string(), 2),
        0xD6 => ("SUB d8".to_string(), 2),
        0xDE => ("SBC A, d8".to_string(), 2),
        0xE6 => ("AND d8".to_string(), 2),
        0xEE => ("XOR d8".to_string(), 2),
        0xF6 => ("OR d8".to_string(), 2),
        0xFE => ("CP d8".to_string(), 2),

        // RST instructions
        0xC7 => ("RST 00H".to_string(), 1),
        0xCF => ("RST 08H".to_string(), 1),
        0xD7 => ("RST 10H".to_string(), 1),
        0xDF => ("RST 18H".to_string(), 1),
        0xE7 => ("RST 20H".to_string(), 1),
        0xEF => ("RST 28H".to_string(), 1),
        0xF7 => ("RST 30H".to_string(), 1),
        0xFF => ("RST 38H".to_string(), 1),

        // For opcodes not explicitly listed, use a simple format
        _ => (format!("DB ${:02X}", opcode), 1),
    };

    let bytes: Vec<u8> = memory.iter().take(len).copied().collect();
    Some(DisassembledInstruction::new(address, bytes, mnemonic))
}

fn disassemble_cb_instruction(memory: &[u8], address: u32) -> Option<DisassembledInstruction> {
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
                6 => format!("SWAP {}", reg_name),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disassemble_nop() {
        let memory = [0x00];
        let instr = disassemble_lr35902(&memory, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "NOP");
        assert_eq!(instr.len(), 1);
    }

    #[test]
    fn test_disassemble_ld_immediate() {
        let memory = [0x3E, 0x42];
        let instr = disassemble_lr35902(&memory, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "LD A, d8");
        assert_eq!(instr.len(), 2);
    }

    #[test]
    fn test_disassemble_jp() {
        let memory = [0xC3, 0x00, 0x01];
        let instr = disassemble_lr35902(&memory, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "JP a16");
        assert_eq!(instr.len(), 3);
    }

    #[test]
    fn test_disassemble_cb_bit() {
        let memory = [0xCB, 0x47]; // BIT 0, A
        let instr = disassemble_lr35902(&memory, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "BIT 0, A");
        assert_eq!(instr.len(), 2);
    }

    #[test]
    fn test_disassemble_cb_set() {
        let memory = [0xCB, 0xCF]; // SET 1, A
        let instr = disassemble_lr35902(&memory, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "SET 1, A");
        assert_eq!(instr.len(), 2);
    }

    #[test]
    fn test_disassemble_short_memory() {
        let memory = [];
        assert!(disassemble_lr35902(&memory, 0x0000).is_none());
    }

    #[test]
    fn test_disassemble_cb_short_memory() {
        let memory = [0xCB]; // CB prefix but no second byte
        assert!(disassemble_lr35902(&memory, 0x0000).is_none());
    }
}
