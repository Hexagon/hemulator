//! CHIP-8/Super-CHIP/XO-CHIP disassembler
//!
//! Provides instruction disassembly for CHIP-8 VM and its variants.

use crate::debug::DisassembledInstruction;

/// Disassemble a single CHIP-8 instruction from memory
///
/// # Arguments
/// * `memory` - Slice of memory containing the instruction (at least 2 bytes)
/// * `address` - Address of the instruction
///
/// # Returns
/// A `DisassembledInstruction` if the instruction could be disassembled, or `None` if memory is too short
pub fn disassemble_chip8(memory: &[u8], address: u32) -> Option<DisassembledInstruction> {
    if memory.len() < 2 {
        return None;
    }

    // CHIP-8 instructions are 2 bytes, big-endian
    let opcode = u16::from_be_bytes([memory[0], memory[1]]);
    let bytes = vec![memory[0], memory[1]];

    // Extract commonly used fields
    let nnn = opcode & 0x0FFF; // 12-bit address
    let nn = (opcode & 0x00FF) as u8; // 8-bit constant
    let n = (opcode & 0x000F) as u8; // 4-bit constant
    let x = ((opcode & 0x0F00) >> 8) as u8; // 4-bit register index
    let y = ((opcode & 0x00F0) >> 4) as u8; // 4-bit register index

    let mnemonic = match opcode & 0xF000 {
        0x0000 => match opcode {
            0x00E0 => "CLS".to_string(),
            0x00EE => "RET".to_string(),
            0x00FB => "SCR".to_string(),  // Super-CHIP scroll right
            0x00FC => "SCL".to_string(),  // Super-CHIP scroll left
            0x00FD => "EXIT".to_string(), // Super-CHIP exit
            0x00FE => "LOW".to_string(),  // Super-CHIP low-res
            0x00FF => "HIGH".to_string(), // Super-CHIP high-res
            _ => {
                if (opcode & 0x00F0) == 0x00C0 {
                    format!("SCD {}", n) // Super-CHIP scroll down
                } else if (opcode & 0x00F0) == 0x00D0 {
                    format!("SCU {}", n) // XO-CHIP scroll up
                } else {
                    format!("SYS {:03X}", nnn) // Machine code routine (ignored)
                }
            }
        },
        0x1000 => format!("JP {:03X}", nnn),
        0x2000 => format!("CALL {:03X}", nnn),
        0x3000 => format!("SE V{:X}, {:02X}", x, nn),
        0x4000 => format!("SNE V{:X}, {:02X}", x, nn),
        0x5000 => match n {
            0x0 => format!("SE V{:X}, V{:X}", x, y),
            0x2 => format!("SAVE V{:X}, V{:X}", x, y), // XO-CHIP save range
            0x3 => format!("LOAD V{:X}, V{:X}", x, y), // XO-CHIP load range
            _ => format!("??? {:04X}", opcode),
        },
        0x6000 => format!("LD V{:X}, {:02X}", x, nn),
        0x7000 => format!("ADD V{:X}, {:02X}", x, nn),
        0x8000 => match n {
            0x0 => format!("LD V{:X}, V{:X}", x, y),
            0x1 => format!("OR V{:X}, V{:X}", x, y),
            0x2 => format!("AND V{:X}, V{:X}", x, y),
            0x3 => format!("XOR V{:X}, V{:X}", x, y),
            0x4 => format!("ADD V{:X}, V{:X}", x, y),
            0x5 => format!("SUB V{:X}, V{:X}", x, y),
            0x6 => format!("SHR V{:X}", x),
            0x7 => format!("SUBN V{:X}, V{:X}", x, y),
            0xE => format!("SHL V{:X}", x),
            _ => format!("??? {:04X}", opcode),
        },
        0x9000 => format!("SNE V{:X}, V{:X}", x, y),
        0xA000 => format!("LD I, {:03X}", nnn),
        0xB000 => format!("JP V0, {:03X}", nnn),
        0xC000 => format!("RND V{:X}, {:02X}", x, nn),
        0xD000 => {
            if n == 0 {
                format!("DRW V{:X}, V{:X}, 0", x, y) // Super-CHIP 16x16 sprite
            } else {
                format!("DRW V{:X}, V{:X}, {}", x, y, n)
            }
        }
        0xE000 => match nn {
            0x9E => format!("SKP V{:X}", x),
            0xA1 => format!("SKNP V{:X}", x),
            _ => format!("??? {:04X}", opcode),
        },
        0xF000 => match nn {
            0x00 => "LD I, LONG".to_string(),  // XO-CHIP load long address
            0x01 => format!("PLANE {}", x), // XO-CHIP select plane
            0x02 => "AUDIO".to_string(),    // XO-CHIP load audio pattern
            0x07 => format!("LD V{:X}, DT", x),
            0x0A => format!("LD V{:X}, K", x),
            0x15 => format!("LD DT, V{:X}", x),
            0x18 => format!("LD ST, V{:X}", x),
            0x1E => format!("ADD I, V{:X}", x),
            0x29 => format!("LD F, V{:X}", x),
            0x30 => format!("LD HF, V{:X}", x), // Super-CHIP 10-byte font
            0x33 => format!("LD B, V{:X}", x),
            0x3A => format!("LD PITCH, V{:X}", x), // XO-CHIP set pitch
            0x55 => format!("LD [I], V{:X}", x),
            0x65 => format!("LD V{:X}, [I]", x),
            0x75 => format!("LD R, V{:X}", x), // Super-CHIP save flags
            0x85 => format!("LD V{:X}, R", x), // Super-CHIP load flags
            _ => format!("??? {:04X}", opcode),
        },
        _ => format!("??? {:04X}", opcode),
    };

    Some(DisassembledInstruction::new(address, bytes, mnemonic))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disassemble_cls() {
        let memory = [0x00, 0xE0];
        let instr = disassemble_chip8(&memory, 0x200).unwrap();
        assert_eq!(instr.address, 0x200);
        assert_eq!(instr.bytes, vec![0x00, 0xE0]);
        assert_eq!(instr.mnemonic, "CLS");
    }

    #[test]
    fn test_disassemble_ret() {
        let memory = [0x00, 0xEE];
        let instr = disassemble_chip8(&memory, 0x202).unwrap();
        assert_eq!(instr.mnemonic, "RET");
    }

    #[test]
    fn test_disassemble_jp() {
        let memory = [0x12, 0x34];
        let instr = disassemble_chip8(&memory, 0x204).unwrap();
        assert_eq!(instr.mnemonic, "JP 234");
    }

    #[test]
    fn test_disassemble_call() {
        let memory = [0x23, 0x45];
        let instr = disassemble_chip8(&memory, 0x206).unwrap();
        assert_eq!(instr.mnemonic, "CALL 345");
    }

    #[test]
    fn test_disassemble_se() {
        let memory = [0x34, 0x56];
        let instr = disassemble_chip8(&memory, 0x208).unwrap();
        assert_eq!(instr.mnemonic, "SE V4, 56");
    }

    #[test]
    fn test_disassemble_ld() {
        let memory = [0x64, 0x12];
        let instr = disassemble_chip8(&memory, 0x20A).unwrap();
        assert_eq!(instr.mnemonic, "LD V4, 12");
    }

    #[test]
    fn test_disassemble_add() {
        let memory = [0x74, 0x05];
        let instr = disassemble_chip8(&memory, 0x20C).unwrap();
        assert_eq!(instr.mnemonic, "ADD V4, 05");
    }

    #[test]
    fn test_disassemble_ld_i() {
        let memory = [0xA2, 0x34];
        let instr = disassemble_chip8(&memory, 0x20E).unwrap();
        assert_eq!(instr.mnemonic, "LD I, 234");
    }

    #[test]
    fn test_disassemble_drw() {
        let memory = [0xD1, 0x25];
        let instr = disassemble_chip8(&memory, 0x210).unwrap();
        assert_eq!(instr.mnemonic, "DRW V1, V2, 5");
    }

    #[test]
    fn test_disassemble_drw_16x16() {
        let memory = [0xD1, 0x20];
        let instr = disassemble_chip8(&memory, 0x212).unwrap();
        assert_eq!(instr.mnemonic, "DRW V1, V2, 0");
    }

    #[test]
    fn test_disassemble_short_memory() {
        let memory = [0x00];
        let result = disassemble_chip8(&memory, 0x200);
        assert!(result.is_none());
    }

    #[test]
    fn test_disassemble_alu_ops() {
        // Test various 8XYn instructions
        let tests = vec![
            ([0x81, 0x20], "LD V1, V2"),
            ([0x81, 0x21], "OR V1, V2"),
            ([0x81, 0x22], "AND V1, V2"),
            ([0x81, 0x23], "XOR V1, V2"),
            ([0x81, 0x24], "ADD V1, V2"),
            ([0x81, 0x25], "SUB V1, V2"),
            ([0x81, 0x26], "SHR V1"),
            ([0x81, 0x27], "SUBN V1, V2"),
            ([0x81, 0x2E], "SHL V1"),
        ];

        for (memory, expected) in tests {
            let instr = disassemble_chip8(&memory, 0x200).unwrap();
            assert_eq!(instr.mnemonic, expected);
        }
    }
}
