//! Base MIPS I disassembler — shared by R3000A and R4300i.
//!
//! Decodes all standard MIPS I opcodes (SPECIAL, REGIMM, branches, loads,
//! stores, immediates, COP0 basics).  Variant crates extend this by handling
//! opcodes that return `None` from [`disassemble_mips_i`] — e.g. COP2/GTE
//! for R3000A or 64-bit ops and likely branches for R4300i.

use crate::fields::Fields;
use crate::regs::REG_NAMES;
use hemu_types::DisassembledInstruction;

/// Byte order for reading 32-bit instruction words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Endian {
    /// Little-endian (R3000A / PS1)
    Little,
    /// Big-endian (R4300i / N64)
    Big,
}

/// Read a 32-bit word from `memory` in the given endianness.
pub fn read_word(memory: &[u8], endian: Endian) -> Option<u32> {
    if memory.len() < 4 {
        return None;
    }
    Some(match endian {
        Endian::Little => u32::from_le_bytes([memory[0], memory[1], memory[2], memory[3]]),
        Endian::Big => u32::from_be_bytes([memory[0], memory[1], memory[2], memory[3]]),
    })
}

/// Disassemble a standard MIPS I instruction.
///
/// Returns `None` if the opcode belongs to a variant-specific extension
/// (COP1/COP2/COP3, 64-bit ops, likely branches, etc.) so the caller can
/// provide its own decode.
///
/// # Arguments
/// * `memory` — at least 4 bytes starting at the instruction
/// * `address` — the PC of this instruction (used for branch target display)
/// * `endian` — byte order
pub fn disassemble_mips_i(
    memory: &[u8],
    address: u32,
    endian: Endian,
) -> Option<DisassembledInstruction> {
    let word = read_word(memory, endian)?;
    let bytes = vec![memory[0], memory[1], memory[2], memory[3]];
    let f = Fields::from(word);

    let mnemonic = decode_mips_i(&f, address)?;
    Some(DisassembledInstruction::new(address, bytes, mnemonic))
}

/// Pure mnemonic decode for base MIPS I.
///
/// Returns `None` for opcodes that should be handled by variant extensions.
pub fn decode_mips_i(f: &Fields, address: u32) -> Option<String> {
    let r = REG_NAMES;

    match f.opcode {
        // SPECIAL (R-type) — returns None for MIPS III 64-bit functs
        0x00 => decode_special(f),

        // REGIMM
        0x01 => decode_regimm(f, address),

        // J / JAL
        0x02 => Some(format!("J ${:08X}", f.jump_target(address))),
        0x03 => Some(format!("JAL ${:08X}", f.jump_target(address))),

        // Branches
        0x04 => {
            let tgt = f.branch_target(address);
            if f.rs == 0 && f.rt == 0 {
                Some(format!("B ${:08X}", tgt))
            } else {
                Some(format!("BEQ {}, {}, ${:08X}", r[f.rs], r[f.rt], tgt))
            }
        }
        0x05 => Some(format!(
            "BNE {}, {}, ${:08X}",
            r[f.rs],
            r[f.rt],
            f.branch_target(address)
        )),
        0x06 => Some(format!(
            "BLEZ {}, ${:08X}",
            r[f.rs],
            f.branch_target(address)
        )),
        0x07 => Some(format!(
            "BGTZ {}, ${:08X}",
            r[f.rs],
            f.branch_target(address)
        )),

        // Immediate arithmetic / logic
        0x08 => Some(format!("ADDI {}, {}, {}", r[f.rt], r[f.rs], f.simm)),
        0x09 => {
            if f.rs == 0 {
                Some(format!("LI {}, {}", r[f.rt], f.simm))
            } else {
                Some(format!("ADDIU {}, {}, {}", r[f.rt], r[f.rs], f.simm))
            }
        }
        0x0A => Some(format!("SLTI {}, {}, {}", r[f.rt], r[f.rs], f.simm)),
        0x0B => Some(format!("SLTIU {}, {}, 0x{:04X}", r[f.rt], r[f.rs], f.imm16)),
        0x0C => Some(format!("ANDI {}, {}, 0x{:04X}", r[f.rt], r[f.rs], f.imm16)),
        0x0D => Some(format!("ORI {}, {}, 0x{:04X}", r[f.rt], r[f.rs], f.imm16)),
        0x0E => Some(format!("XORI {}, {}, 0x{:04X}", r[f.rt], r[f.rs], f.imm16)),
        0x0F => Some(format!("LUI {}, 0x{:04X}", r[f.rt], f.imm16)),

        // COP0 (common subset)
        0x10 => decode_cop0(f),

        // COP1, COP2, COP3 — variant-specific, let caller handle
        0x11..=0x13 => None,

        // Load instructions
        0x20 => Some(format!("LB {}, {}({})", r[f.rt], f.simm, r[f.rs])),
        0x21 => Some(format!("LH {}, {}({})", r[f.rt], f.simm, r[f.rs])),
        0x22 => Some(format!("LWL {}, {}({})", r[f.rt], f.simm, r[f.rs])),
        0x23 => Some(format!("LW {}, {}({})", r[f.rt], f.simm, r[f.rs])),
        0x24 => Some(format!("LBU {}, {}({})", r[f.rt], f.simm, r[f.rs])),
        0x25 => Some(format!("LHU {}, {}({})", r[f.rt], f.simm, r[f.rs])),
        0x26 => Some(format!("LWR {}, {}({})", r[f.rt], f.simm, r[f.rs])),

        // Store instructions
        0x28 => Some(format!("SB {}, {}({})", r[f.rt], f.simm, r[f.rs])),
        0x29 => Some(format!("SH {}, {}({})", r[f.rt], f.simm, r[f.rs])),
        0x2A => Some(format!("SWL {}, {}({})", r[f.rt], f.simm, r[f.rs])),
        0x2B => Some(format!("SW {}, {}({})", r[f.rt], f.simm, r[f.rs])),
        0x2E => Some(format!("SWR {}, {}({})", r[f.rt], f.simm, r[f.rs])),

        // COP load/store — let variant override (R3000A uses LWC2/SWC2 for GTE)
        0x31 | 0x32 | 0x33 | 0x35 | 0x36 | 0x39 | 0x3A | 0x3B | 0x3D | 0x3E => None,

        // Cache (MIPS III+) — variant specific
        0x2F => None,

        // 64-bit ops (MIPS III+) — variant specific
        0x14..=0x1F | 0x27 | 0x2C | 0x2D | 0x30 | 0x34 | 0x37 | 0x38 | 0x3C | 0x3F => None,

        _ => Some(format!(".word 0x{:08X}", f.word)),
    }
}

/// Decode SPECIAL (opcode 0x00) R-type instructions.
///
/// Returns `None` for MIPS III 64-bit funct codes (DSLL, DSRL, DMULT, DADD,
/// etc.) so variant crates can provide their own decode.
fn decode_special(f: &Fields) -> Option<String> {
    let r = REG_NAMES;
    match f.funct {
        0x00 if f.word == 0 => Some("NOP".to_string()),
        0x00 => Some(format!("SLL {}, {}, {}", r[f.rd], r[f.rt], f.sa)),
        0x02 => Some(format!("SRL {}, {}, {}", r[f.rd], r[f.rt], f.sa)),
        0x03 => Some(format!("SRA {}, {}, {}", r[f.rd], r[f.rt], f.sa)),
        0x04 => Some(format!("SLLV {}, {}, {}", r[f.rd], r[f.rt], r[f.rs])),
        0x06 => Some(format!("SRLV {}, {}, {}", r[f.rd], r[f.rt], r[f.rs])),
        0x07 => Some(format!("SRAV {}, {}, {}", r[f.rd], r[f.rt], r[f.rs])),
        0x08 => Some(format!("JR {}", r[f.rs])),
        0x09 => {
            if f.rd == 31 {
                Some(format!("JALR {}", r[f.rs]))
            } else {
                Some(format!("JALR {}, {}", r[f.rd], r[f.rs]))
            }
        }
        0x0C => Some("SYSCALL".to_string()),
        0x0D => Some("BREAK".to_string()),
        0x0F => Some("SYNC".to_string()),
        0x10 => Some(format!("MFHI {}", r[f.rd])),
        0x11 => Some(format!("MTHI {}", r[f.rs])),
        0x12 => Some(format!("MFLO {}", r[f.rd])),
        0x13 => Some(format!("MTLO {}", r[f.rs])),
        0x18 => Some(format!("MULT {}, {}", r[f.rs], r[f.rt])),
        0x19 => Some(format!("MULTU {}, {}", r[f.rs], r[f.rt])),
        0x1A => Some(format!("DIV {}, {}", r[f.rs], r[f.rt])),
        0x1B => Some(format!("DIVU {}, {}", r[f.rs], r[f.rt])),
        0x20 => Some(format!("ADD {}, {}, {}", r[f.rd], r[f.rs], r[f.rt])),
        0x21 => {
            if f.rt == 0 {
                Some(format!("MOVE {}, {}", r[f.rd], r[f.rs]))
            } else {
                Some(format!("ADDU {}, {}, {}", r[f.rd], r[f.rs], r[f.rt]))
            }
        }
        0x22 => Some(format!("SUB {}, {}, {}", r[f.rd], r[f.rs], r[f.rt])),
        0x23 => Some(format!("SUBU {}, {}, {}", r[f.rd], r[f.rs], r[f.rt])),
        0x24 => Some(format!("AND {}, {}, {}", r[f.rd], r[f.rs], r[f.rt])),
        0x25 => Some(format!("OR {}, {}, {}", r[f.rd], r[f.rs], r[f.rt])),
        0x26 => Some(format!("XOR {}, {}, {}", r[f.rd], r[f.rs], r[f.rt])),
        0x27 => Some(format!("NOR {}, {}, {}", r[f.rd], r[f.rs], r[f.rt])),
        0x2A => Some(format!("SLT {}, {}, {}", r[f.rd], r[f.rs], r[f.rt])),
        0x2B => Some(format!("SLTU {}, {}, {}", r[f.rd], r[f.rs], r[f.rt])),
        // MIPS III 64-bit funct codes — let variant handle
        _ => None,
    }
}

/// Decode REGIMM (opcode 0x01) branch instructions.
///
/// Returns `None` for MIPS II/III likely branches — variant crates handle those.
fn decode_regimm(f: &Fields, address: u32) -> Option<String> {
    let r = REG_NAMES;
    let tgt = f.branch_target(address);
    match f.rt {
        0x00 => Some(format!("BLTZ {}, ${:08X}", r[f.rs], tgt)),
        0x01 => Some(format!("BGEZ {}, ${:08X}", r[f.rs], tgt)),
        0x10 => Some(format!("BLTZAL {}, ${:08X}", r[f.rs], tgt)),
        0x11 => Some(format!("BGEZAL {}, ${:08X}", r[f.rs], tgt)),
        // MIPS II/III likely branches — variant-specific
        _ => None,
    }
}

/// Decode COP0 instructions (MIPS I common subset).
///
/// Returns `None` for MIPS II/III instructions (DMFC0, DMTC0, ERET) — variant crates handle those.
fn decode_cop0(f: &Fields) -> Option<String> {
    let r = REG_NAMES;
    match f.rs {
        0x00 => Some(format!("MFC0 {}, ${}", r[f.rt], f.rd)),
        0x04 => Some(format!("MTC0 {}, ${}", r[f.rt], f.rd)),
        0x10..=0x1F => match f.funct {
            0x01 => Some("TLBR".to_string()),
            0x02 => Some("TLBWI".to_string()),
            0x06 => Some("TLBWR".to_string()),
            0x08 => Some("TLBP".to_string()),
            0x10 => Some("RFE".to_string()),
            // DMFC0, DMTC0, ERET are MIPS II/III — let variant handle
            _ => None,
        },
        // DMFC0 (0x01), DMTC0 (0x05) are MIPS III — let variant handle
        _ => None,
    }
}

#[cfg(test)]
#[allow(clippy::identity_op)]
mod tests {
    use super::*;

    fn dis_le(word: u32, pc: u32) -> String {
        let mem = word.to_le_bytes();
        disassemble_mips_i(&mem, pc, Endian::Little)
            .unwrap()
            .mnemonic
    }

    fn dis_be(word: u32, pc: u32) -> String {
        let mem = word.to_be_bytes();
        disassemble_mips_i(&mem, pc, Endian::Big).unwrap().mnemonic
    }

    #[test]
    fn test_nop() {
        assert_eq!(dis_le(0x0000_0000, 0x80000000), "NOP");
        assert_eq!(dis_be(0x0000_0000, 0x80000000), "NOP");
    }

    #[test]
    fn test_lui() {
        let word = (0x0Fu32 << 26) | (8 << 16) | 0x8000;
        assert_eq!(dis_le(word, 0x80000000), "LUI t0, 0x8000");
    }

    #[test]
    fn test_addiu() {
        let word = (9u32 << 26) | (8 << 21) | (9 << 16) | 42;
        assert_eq!(dis_le(word, 0x80000000), "ADDIU t1, t0, 42");
    }

    #[test]
    fn test_li_pseudo() {
        let word = (9u32 << 26) | (0 << 21) | (8 << 16) | 42;
        assert_eq!(dis_le(word, 0x80000000), "LI t0, 42");
    }

    #[test]
    fn test_j() {
        let word = (2u32 << 26) | ((0x80001000u32 >> 2) & 0x03FF_FFFF);
        assert_eq!(dis_le(word, 0x80000000), "J $80001000");
    }

    #[test]
    fn test_jr_ra() {
        let word = (31u32 << 21) | 0x08;
        assert_eq!(dis_le(word, 0x80000000), "JR ra");
    }

    #[test]
    fn test_lw() {
        let word = (0x23u32 << 26) | (29 << 21) | (8 << 16) | 0x0010;
        assert_eq!(dis_le(word, 0x80000000), "LW t0, 16(sp)");
    }

    #[test]
    fn test_sw() {
        let word = (0x2Bu32 << 26) | (29 << 21) | (31 << 16) | 0xFFFC;
        assert_eq!(dis_le(word, 0x80000000), "SW ra, -4(sp)");
    }

    #[test]
    fn test_beq_branch() {
        let word = (4u32 << 26) | (0 << 21) | (0 << 16) | 0x0002;
        assert!(dis_le(word, 0x80000000).starts_with("B "));
    }

    #[test]
    fn test_syscall() {
        let word = 0x0Cu32;
        assert_eq!(dis_le(word, 0x80000000), "SYSCALL");
    }

    #[test]
    fn test_move_pseudo() {
        let word = (0u32 << 26) | (9 << 21) | (0 << 16) | (8 << 11) | 0x21;
        assert_eq!(dis_le(word, 0x80000000), "MOVE t0, t1");
    }

    #[test]
    fn test_mfc0() {
        let word = (0x10u32 << 26) | (0 << 21) | (8 << 16) | (12 << 11);
        assert_eq!(dis_le(word, 0x80000000), "MFC0 t0, $12");
    }

    #[test]
    fn test_rfe() {
        let word = (0x10u32 << 26) | (0x10 << 21) | 0x10;
        assert_eq!(dis_le(word, 0x80000000), "RFE");
    }

    #[test]
    fn test_big_endian() {
        // NOP in big-endian
        let word = 0x0000_0000u32;
        assert_eq!(dis_be(word, 0x80000000), "NOP");
        // LUI in big-endian
        let word = (0x0Fu32 << 26) | (8 << 16) | 0x1234;
        assert_eq!(dis_be(word, 0x80000000), "LUI t0, 0x1234");
    }

    #[test]
    fn test_cop2_returns_none() {
        // COP2 instructions should return None — variant-specific
        let word = (0x12u32 << 26) | (1 << 25) | 0x01; // GTE RTPS
        let mem = word.to_le_bytes();
        assert!(disassemble_mips_i(&mem, 0x80000000, Endian::Little).is_none());
    }

    #[test]
    fn test_short_memory() {
        let mem = [0x00, 0x00];
        assert!(disassemble_mips_i(&mem, 0x80000000, Endian::Little).is_none());
    }

    #[test]
    fn test_bltz() {
        let word = (1u32 << 26) | (8 << 21) | (0 << 16) | 0x0004;
        assert!(dis_le(word, 0x80000000).starts_with("BLTZ t0"));
    }

    #[test]
    fn test_andi() {
        let word = (0x0Cu32 << 26) | (8 << 21) | (9 << 16) | 0x00FF;
        assert_eq!(dis_le(word, 0x80000000), "ANDI t1, t0, 0x00FF");
    }

    #[test]
    fn test_ori() {
        let word = (0x0Du32 << 26) | (8 << 21) | (9 << 16) | 0x1234;
        assert_eq!(dis_le(word, 0x80000000), "ORI t1, t0, 0x1234");
    }

    #[test]
    fn test_mult() {
        let word = (8u32 << 21) | (9 << 16) | 0x18;
        assert_eq!(dis_le(word, 0x80000000), "MULT t0, t1");
    }

    #[test]
    fn test_slt() {
        let word = (8u32 << 21) | (9 << 16) | (10 << 11) | 0x2A;
        assert_eq!(dis_le(word, 0x80000000), "SLT t2, t0, t1");
    }

    #[test]
    fn test_eret_returns_none() {
        // ERET is MIPS II — base decoder returns None for variant-specific handling
        let word = (0x10u32 << 26) | (0x10 << 21) | 0x18;
        let mem = word.to_le_bytes();
        assert!(disassemble_mips_i(&mem, 0x80000000, Endian::Little).is_none());
    }

    #[test]
    fn test_dmfc0_returns_none() {
        // DMFC0 is MIPS III — base decoder returns None
        let word = (0x10u32 << 26) | (0x01 << 21) | (8 << 16) | (12 << 11);
        let mem = word.to_le_bytes();
        assert!(disassemble_mips_i(&mem, 0x80000000, Endian::Little).is_none());
    }

    #[test]
    fn test_regimm_likely_returns_none() {
        // BLTZL is MIPS II — base decoder returns None for variant handling
        let word = (1u32 << 26) | (8 << 21) | (2 << 16) | 0x0004;
        let mem = word.to_le_bytes();
        assert!(disassemble_mips_i(&mem, 0x80000000, Endian::Little).is_none());
    }

    #[test]
    fn test_special_mips_iii_returns_none() {
        // DSLL (funct=0x38) is MIPS III — base decoder returns None
        let word = (0u32 << 26) | (8 << 16) | (10 << 11) | (4 << 6) | 0x38;
        let mem = word.to_le_bytes();
        assert!(disassemble_mips_i(&mem, 0x80000000, Endian::Little).is_none());

        // DMULT (funct=0x1C) is MIPS III — base decoder returns None
        let word2 = (8u32 << 21) | (9 << 16) | 0x1C;
        let mem2 = word2.to_le_bytes();
        assert!(disassemble_mips_i(&mem2, 0x80000000, Endian::Little).is_none());

        // DADDU (funct=0x2D) is MIPS III — base decoder returns None
        let word3 = (8u32 << 21) | (9 << 16) | (10 << 11) | 0x2D;
        let mem3 = word3.to_le_bytes();
        assert!(disassemble_mips_i(&mem3, 0x80000000, Endian::Little).is_none());
    }
}
