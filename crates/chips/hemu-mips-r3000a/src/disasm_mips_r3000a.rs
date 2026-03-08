//! MIPS R3000A CPU disassembler
//!
//! Full disassembly for the MIPS R3000A CPU used in PlayStation 1.
//! All MIPS I instructions are supported, plus COP0 and GTE (COP2) basics.

use hemu_types::DisassembledInstruction;

/// Standard MIPS register names
const REG_NAMES: [&str; 32] = [
    "zero", "at", "v0", "v1", "a0", "a1", "a2", "a3", "t0", "t1", "t2", "t3", "t4", "t5", "t6",
    "t7", "s0", "s1", "s2", "s3", "s4", "s5", "s6", "s7", "t8", "t9", "k0", "k1", "gp", "sp", "fp",
    "ra",
];

/// GTE command names (COP2 function field)
fn gte_command_name(funct: u32) -> &'static str {
    match funct {
        0x01 => "RTPS",
        0x06 => "NCLIP",
        0x0C => "OP",
        0x10 => "DPCS",
        0x11 => "INTPL",
        0x12 => "MVMVA",
        0x13 => "NCDS",
        0x14 => "CDP",
        0x16 => "NCDT",
        0x1B => "NCCS",
        0x1C => "CC",
        0x1E => "NCS",
        0x20 => "NCT",
        0x28 => "SQR",
        0x29 => "DCPL",
        0x2A => "DPCT",
        0x2D => "AVSZ3",
        0x2E => "AVSZ4",
        0x30 => "RTPT",
        0x3D => "GPF",
        0x3E => "GPL",
        0x3F => "NCCT",
        _ => "GTE_UNK",
    }
}

/// Disassemble a single MIPS R3000A instruction from memory (little-endian)
pub fn disassemble_r3000a(memory: &[u8], address: u32) -> Option<DisassembledInstruction> {
    if memory.len() < 4 {
        return None;
    }

    let bytes = vec![memory[0], memory[1], memory[2], memory[3]];
    let word = u32::from_le_bytes([memory[0], memory[1], memory[2], memory[3]]);

    let opcode = (word >> 26) & 0x3F;
    let rs = ((word >> 21) & 0x1F) as usize;
    let rt = ((word >> 16) & 0x1F) as usize;
    let rd = ((word >> 11) & 0x1F) as usize;
    let sa = (word >> 6) & 0x1F;
    let funct = word & 0x3F;
    let imm16 = (word & 0xFFFF) as u16;
    let simm = imm16 as i16;
    let target = word & 0x03FF_FFFF;

    let mnemonic = match opcode {
        // SPECIAL (R-type)
        0x00 => match funct {
            0x00 if word == 0 => "NOP".to_string(),
            0x00 => format!("SLL {}, {}, {}", REG_NAMES[rd], REG_NAMES[rt], sa),
            0x02 => format!("SRL {}, {}, {}", REG_NAMES[rd], REG_NAMES[rt], sa),
            0x03 => format!("SRA {}, {}, {}", REG_NAMES[rd], REG_NAMES[rt], sa),
            0x04 => format!(
                "SLLV {}, {}, {}",
                REG_NAMES[rd], REG_NAMES[rt], REG_NAMES[rs]
            ),
            0x06 => format!(
                "SRLV {}, {}, {}",
                REG_NAMES[rd], REG_NAMES[rt], REG_NAMES[rs]
            ),
            0x07 => format!(
                "SRAV {}, {}, {}",
                REG_NAMES[rd], REG_NAMES[rt], REG_NAMES[rs]
            ),
            0x08 => format!("JR {}", REG_NAMES[rs]),
            0x09 => {
                if rd == 31 {
                    format!("JALR {}", REG_NAMES[rs])
                } else {
                    format!("JALR {}, {}", REG_NAMES[rd], REG_NAMES[rs])
                }
            }
            0x0C => "SYSCALL".to_string(),
            0x0D => "BREAK".to_string(),
            0x10 => format!("MFHI {}", REG_NAMES[rd]),
            0x11 => format!("MTHI {}", REG_NAMES[rs]),
            0x12 => format!("MFLO {}", REG_NAMES[rd]),
            0x13 => format!("MTLO {}", REG_NAMES[rs]),
            0x18 => format!("MULT {}, {}", REG_NAMES[rs], REG_NAMES[rt]),
            0x19 => format!("MULTU {}, {}", REG_NAMES[rs], REG_NAMES[rt]),
            0x1A => format!("DIV {}, {}", REG_NAMES[rs], REG_NAMES[rt]),
            0x1B => format!("DIVU {}, {}", REG_NAMES[rs], REG_NAMES[rt]),
            0x20 => format!(
                "ADD {}, {}, {}",
                REG_NAMES[rd], REG_NAMES[rs], REG_NAMES[rt]
            ),
            0x21 => {
                if rt == 0 {
                    format!("MOVE {}, {}", REG_NAMES[rd], REG_NAMES[rs])
                } else {
                    format!(
                        "ADDU {}, {}, {}",
                        REG_NAMES[rd], REG_NAMES[rs], REG_NAMES[rt]
                    )
                }
            }
            0x22 => format!(
                "SUB {}, {}, {}",
                REG_NAMES[rd], REG_NAMES[rs], REG_NAMES[rt]
            ),
            0x23 => format!(
                "SUBU {}, {}, {}",
                REG_NAMES[rd], REG_NAMES[rs], REG_NAMES[rt]
            ),
            0x24 => format!(
                "AND {}, {}, {}",
                REG_NAMES[rd], REG_NAMES[rs], REG_NAMES[rt]
            ),
            0x25 => format!("OR {}, {}, {}", REG_NAMES[rd], REG_NAMES[rs], REG_NAMES[rt]),
            0x26 => format!(
                "XOR {}, {}, {}",
                REG_NAMES[rd], REG_NAMES[rs], REG_NAMES[rt]
            ),
            0x27 => format!(
                "NOR {}, {}, {}",
                REG_NAMES[rd], REG_NAMES[rs], REG_NAMES[rt]
            ),
            0x2A => format!(
                "SLT {}, {}, {}",
                REG_NAMES[rd], REG_NAMES[rs], REG_NAMES[rt]
            ),
            0x2B => format!(
                "SLTU {}, {}, {}",
                REG_NAMES[rd], REG_NAMES[rs], REG_NAMES[rt]
            ),
            _ => format!("SPECIAL ${:02X}", funct),
        },

        // BCOND (BLTZ, BGEZ, BLTZAL, BGEZAL)
        0x01 => {
            let branch_target = address
                .wrapping_add(4)
                .wrapping_add((simm as i32 as u32) << 2);
            match rt {
                0x00 => format!("BLTZ {}, ${:08X}", REG_NAMES[rs], branch_target),
                0x01 => format!("BGEZ {}, ${:08X}", REG_NAMES[rs], branch_target),
                0x10 => format!("BLTZAL {}, ${:08X}", REG_NAMES[rs], branch_target),
                0x11 => format!("BGEZAL {}, ${:08X}", REG_NAMES[rs], branch_target),
                _ => format!("BCOND rt={} {}, ${:08X}", rt, REG_NAMES[rs], branch_target),
            }
        }

        // J / JAL
        0x02 => {
            let jump_target = (address & 0xF000_0000) | (target << 2);
            format!("J ${:08X}", jump_target)
        }
        0x03 => {
            let jump_target = (address & 0xF000_0000) | (target << 2);
            format!("JAL ${:08X}", jump_target)
        }

        // Branches
        0x04 => {
            let branch_target = address
                .wrapping_add(4)
                .wrapping_add((simm as i32 as u32) << 2);
            if rs == 0 && rt == 0 {
                format!("B ${:08X}", branch_target)
            } else {
                format!(
                    "BEQ {}, {}, ${:08X}",
                    REG_NAMES[rs], REG_NAMES[rt], branch_target
                )
            }
        }
        0x05 => {
            let branch_target = address
                .wrapping_add(4)
                .wrapping_add((simm as i32 as u32) << 2);
            format!(
                "BNE {}, {}, ${:08X}",
                REG_NAMES[rs], REG_NAMES[rt], branch_target
            )
        }
        0x06 => {
            let branch_target = address
                .wrapping_add(4)
                .wrapping_add((simm as i32 as u32) << 2);
            format!("BLEZ {}, ${:08X}", REG_NAMES[rs], branch_target)
        }
        0x07 => {
            let branch_target = address
                .wrapping_add(4)
                .wrapping_add((simm as i32 as u32) << 2);
            format!("BGTZ {}, ${:08X}", REG_NAMES[rs], branch_target)
        }

        // Immediate arithmetic/logic
        0x08 => format!("ADDI {}, {}, {}", REG_NAMES[rt], REG_NAMES[rs], simm),
        0x09 => {
            if rs == 0 {
                format!("LI {}, {}", REG_NAMES[rt], simm)
            } else {
                format!("ADDIU {}, {}, {}", REG_NAMES[rt], REG_NAMES[rs], simm)
            }
        }
        0x0A => format!("SLTI {}, {}, {}", REG_NAMES[rt], REG_NAMES[rs], simm),
        0x0B => format!(
            "SLTIU {}, {}, 0x{:04X}",
            REG_NAMES[rt], REG_NAMES[rs], imm16
        ),
        0x0C => format!("ANDI {}, {}, 0x{:04X}", REG_NAMES[rt], REG_NAMES[rs], imm16),
        0x0D => format!("ORI {}, {}, 0x{:04X}", REG_NAMES[rt], REG_NAMES[rs], imm16),
        0x0E => format!("XORI {}, {}, 0x{:04X}", REG_NAMES[rt], REG_NAMES[rs], imm16),
        0x0F => format!("LUI {}, 0x{:04X}", REG_NAMES[rt], imm16),

        // COP0
        0x10 => match rs {
            0x00 => format!("MFC0 {}, ${}", REG_NAMES[rt], rd),
            0x04 => format!("MTC0 {}, ${}", REG_NAMES[rt], rd),
            0x10 => match funct {
                0x10 => "RFE".to_string(),
                _ => format!("COP0 ${:08X}", word),
            },
            _ => format!("COP0 ${:08X}", word),
        },

        // COP2 (GTE)
        0x12 => {
            if rs & 0x10 != 0 {
                // GTE command
                let cmd = word & 0x1FFFFFF;
                let name = gte_command_name(cmd & 0x3F);
                format!("{} (0x{:07X})", name, cmd)
            } else {
                match rs {
                    0x00 => format!("MFC2 {}, ${}", REG_NAMES[rt], rd),
                    0x02 => format!("CFC2 {}, ${}", REG_NAMES[rt], rd),
                    0x04 => format!("MTC2 {}, ${}", REG_NAMES[rt], rd),
                    0x06 => format!("CTC2 {}, ${}", REG_NAMES[rt], rd),
                    _ => format!("COP2 ${:08X}", word),
                }
            }
        }

        // Load instructions
        0x20 => format!("LB {}, {}({})", REG_NAMES[rt], simm, REG_NAMES[rs]),
        0x21 => format!("LH {}, {}({})", REG_NAMES[rt], simm, REG_NAMES[rs]),
        0x22 => format!("LWL {}, {}({})", REG_NAMES[rt], simm, REG_NAMES[rs]),
        0x23 => format!("LW {}, {}({})", REG_NAMES[rt], simm, REG_NAMES[rs]),
        0x24 => format!("LBU {}, {}({})", REG_NAMES[rt], simm, REG_NAMES[rs]),
        0x25 => format!("LHU {}, {}({})", REG_NAMES[rt], simm, REG_NAMES[rs]),
        0x26 => format!("LWR {}, {}({})", REG_NAMES[rt], simm, REG_NAMES[rs]),

        // Store instructions
        0x28 => format!("SB {}, {}({})", REG_NAMES[rt], simm, REG_NAMES[rs]),
        0x29 => format!("SH {}, {}({})", REG_NAMES[rt], simm, REG_NAMES[rs]),
        0x2A => format!("SWL {}, {}({})", REG_NAMES[rt], simm, REG_NAMES[rs]),
        0x2B => format!("SW {}, {}({})", REG_NAMES[rt], simm, REG_NAMES[rs]),
        0x2E => format!("SWR {}, {}({})", REG_NAMES[rt], simm, REG_NAMES[rs]),

        // COP2 loads/stores (GTE)
        0x32 => format!("LWC2 ${}, {}({})", rt, simm, REG_NAMES[rs]),
        0x3A => format!("SWC2 ${}, {}({})", rt, simm, REG_NAMES[rs]),

        _ => format!(".word 0x{:08X}", word),
    };

    Some(DisassembledInstruction::new(address, bytes, mnemonic))
}

#[cfg(test)]
#[allow(clippy::identity_op)]
mod tests {
    use super::*;

    fn encode_le(word: u32) -> [u8; 4] {
        word.to_le_bytes()
    }

    #[test]
    fn test_nop() {
        let mem = encode_le(0x0000_0000);
        let instr = disassemble_r3000a(&mem, 0x80000000).unwrap();
        assert_eq!(instr.mnemonic, "NOP");
        assert_eq!(instr.len(), 4);
    }

    #[test]
    fn test_lui() {
        // LUI $t0, 0x8000 => opcode=0x0F, rt=8, imm=0x8000
        let word = (0x0Fu32 << 26) | (8 << 16) | 0x8000;
        let mem = encode_le(word);
        let instr = disassemble_r3000a(&mem, 0x80000000).unwrap();
        assert_eq!(instr.mnemonic, "LUI t0, 0x8000");
    }

    #[test]
    fn test_addiu() {
        // ADDIU $t1, $t0, 42 => opcode=9, rs=8, rt=9, imm=42
        let word = (9u32 << 26) | (8 << 21) | (9 << 16) | 42;
        let mem = encode_le(word);
        let instr = disassemble_r3000a(&mem, 0x80000000).unwrap();
        assert_eq!(instr.mnemonic, "ADDIU t1, t0, 42");
    }

    #[test]
    fn test_j() {
        // J 0x80001000 => opcode=2, target=(0x80001000>>2) & 0x03FFFFFF = 0x0000_0400
        let word = (2u32 << 26) | ((0x80001000u32 >> 2) & 0x03FF_FFFF);
        let mem = encode_le(word);
        let instr = disassemble_r3000a(&mem, 0x80000000).unwrap();
        assert_eq!(instr.mnemonic, "J $80001000");
    }

    #[test]
    fn test_lw() {
        // LW $t0, 0x10($sp) => opcode=0x23, rs=29(sp), rt=8(t0), imm=0x10
        let word = (0x23u32 << 26) | (29 << 21) | (8 << 16) | 0x0010;
        let mem = encode_le(word);
        let instr = disassemble_r3000a(&mem, 0x80000000).unwrap();
        assert_eq!(instr.mnemonic, "LW t0, 16(sp)");
    }

    #[test]
    fn test_sw() {
        // SW $ra, -4($sp) => opcode=0x2B, rs=29, rt=31, imm=0xFFFC (-4)
        let word = (0x2Bu32 << 26) | (29 << 21) | (31 << 16) | 0xFFFC;
        let mem = encode_le(word);
        let instr = disassemble_r3000a(&mem, 0x80000000).unwrap();
        assert_eq!(instr.mnemonic, "SW ra, -4(sp)");
    }

    #[test]
    fn test_beq() {
        // BEQ $zero, $zero, +8 => unconditional branch = B
        let word = (4u32 << 26) | (0 << 21) | (0 << 16) | 0x0002; // offset +2 words = +8
        let mem = encode_le(word);
        let instr = disassemble_r3000a(&mem, 0x80000000).unwrap();
        assert!(instr.mnemonic.starts_with("B "));
    }

    #[test]
    fn test_jr_ra() {
        // JR $ra => SPECIAL, rs=31, funct=0x08
        let word = (31u32 << 21) | 0x08;
        let mem = encode_le(word);
        let instr = disassemble_r3000a(&mem, 0x80000000).unwrap();
        assert_eq!(instr.mnemonic, "JR ra");
    }

    #[test]
    fn test_syscall() {
        let word = 0x0C;
        let mem = encode_le(word);
        let instr = disassemble_r3000a(&mem, 0x80000000).unwrap();
        assert_eq!(instr.mnemonic, "SYSCALL");
    }

    #[test]
    fn test_mfc0() {
        // MFC0 $t0, $12 (SR) => COP0, rs=0, rt=8, rd=12
        let word = (0x10u32 << 26) | (0 << 21) | (8 << 16) | (12 << 11);
        let mem = encode_le(word);
        let instr = disassemble_r3000a(&mem, 0x80000000).unwrap();
        assert_eq!(instr.mnemonic, "MFC0 t0, $12");
    }

    #[test]
    fn test_rfe() {
        // RFE => COP0, rs=0x10, funct=0x10
        let word = (0x10u32 << 26) | (0x10 << 21) | 0x10;
        let mem = encode_le(word);
        let instr = disassemble_r3000a(&mem, 0x80000000).unwrap();
        assert_eq!(instr.mnemonic, "RFE");
    }

    #[test]
    fn test_gte_rtps() {
        // COP2 command RTPS => opcode=0x12, bit25=1, funct=0x01
        let word = (0x12u32 << 26) | (1 << 25) | 0x01;
        let mem = encode_le(word);
        let instr = disassemble_r3000a(&mem, 0x80000000).unwrap();
        assert!(instr.mnemonic.starts_with("RTPS"));
    }

    #[test]
    fn test_short_memory() {
        let mem = [0x00, 0x00];
        assert!(disassemble_r3000a(&mem, 0x80000000).is_none());
    }

    #[test]
    fn test_li_pseudo() {
        // ADDIU $t0, $zero, 42 => LI $t0, 42
        let word = (9u32 << 26) | (0 << 21) | (8 << 16) | 42;
        let mem = encode_le(word);
        let instr = disassemble_r3000a(&mem, 0x80000000).unwrap();
        assert_eq!(instr.mnemonic, "LI t0, 42");
    }

    #[test]
    fn test_move_pseudo() {
        // ADDU $t0, $t1, $zero => MOVE $t0, $t1
        let word = (0u32 << 26) | (9 << 21) | (0 << 16) | (8 << 11) | 0x21;
        let mem = encode_le(word);
        let instr = disassemble_r3000a(&mem, 0x80000000).unwrap();
        assert_eq!(instr.mnemonic, "MOVE t0, t1");
    }
}
