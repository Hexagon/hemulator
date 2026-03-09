//! MIPS R3000A CPU disassembler
//!
//! Full disassembly for the MIPS R3000A CPU used in PlayStation 1.
//! Extends the base MIPS I decoder from `hemu-mips-common` with
//! COP2/GTE instructions and LWC2/SWC2.

use hemu_mips_common::disasm_mips::{self, Endian};
use hemu_mips_common::fields::Fields;
use hemu_mips_common::regs::REG_NAMES;
use hemu_types::DisassembledInstruction;

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

/// Disassemble a single R3000A instruction (little-endian, MIPS I + GTE).
pub fn disassemble_r3000a(memory: &[u8], address: u32) -> Option<DisassembledInstruction> {
    // Try the common MIPS I decoder first
    if let Some(instr) = disasm_mips::disassemble_mips_i(memory, address, Endian::Little) {
        return Some(instr);
    }

    // Fall through to R3000A-specific opcodes
    let word = disasm_mips::read_word(memory, Endian::Little)?;
    let bytes = vec![memory[0], memory[1], memory[2], memory[3]];
    let f = Fields::from(word);
    let r = REG_NAMES;

    let mnemonic = match f.opcode {
        // COP2 (GTE)
        0x12 => {
            if f.rs & 0x10 != 0 {
                // GTE command
                let cmd = f.word & 0x1FFFFFF;
                let name = gte_command_name(cmd & 0x3F);
                format!("{} (0x{:07X})", name, cmd)
            } else {
                match f.rs {
                    0x00 => format!("MFC2 {}, ${}", r[f.rt], f.rd),
                    0x02 => format!("CFC2 {}, ${}", r[f.rt], f.rd),
                    0x04 => format!("MTC2 {}, ${}", r[f.rt], f.rd),
                    0x06 => format!("CTC2 {}, ${}", r[f.rt], f.rd),
                    _ => format!("COP2 ${:08X}", f.word),
                }
            }
        }

        // GTE loads/stores
        0x32 => format!("LWC2 ${}, {}({})", f.rt, f.simm, r[f.rs]),
        0x3A => format!("SWC2 ${}, {}({})", f.rt, f.simm, r[f.rs]),

        _ => format!(".word 0x{:08X}", f.word),
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
        let word = (0x0Fu32 << 26) | (8 << 16) | 0x8000;
        let mem = encode_le(word);
        let instr = disassemble_r3000a(&mem, 0x80000000).unwrap();
        assert_eq!(instr.mnemonic, "LUI t0, 0x8000");
    }

    #[test]
    fn test_addiu() {
        let word = (9u32 << 26) | (8 << 21) | (9 << 16) | 42;
        let mem = encode_le(word);
        let instr = disassemble_r3000a(&mem, 0x80000000).unwrap();
        assert_eq!(instr.mnemonic, "ADDIU t1, t0, 42");
    }

    #[test]
    fn test_j() {
        let word = (2u32 << 26) | ((0x80001000u32 >> 2) & 0x03FF_FFFF);
        let mem = encode_le(word);
        let instr = disassemble_r3000a(&mem, 0x80000000).unwrap();
        assert_eq!(instr.mnemonic, "J $80001000");
    }

    #[test]
    fn test_lw() {
        let word = (0x23u32 << 26) | (29 << 21) | (8 << 16) | 0x0010;
        let mem = encode_le(word);
        let instr = disassemble_r3000a(&mem, 0x80000000).unwrap();
        assert_eq!(instr.mnemonic, "LW t0, 16(sp)");
    }

    #[test]
    fn test_sw() {
        let word = (0x2Bu32 << 26) | (29 << 21) | (31 << 16) | 0xFFFC;
        let mem = encode_le(word);
        let instr = disassemble_r3000a(&mem, 0x80000000).unwrap();
        assert_eq!(instr.mnemonic, "SW ra, -4(sp)");
    }

    #[test]
    fn test_beq() {
        let word = (4u32 << 26) | (0 << 21) | (0 << 16) | 0x0002;
        let mem = encode_le(word);
        let instr = disassemble_r3000a(&mem, 0x80000000).unwrap();
        assert!(instr.mnemonic.starts_with("B "));
    }

    #[test]
    fn test_jr_ra() {
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
        let word = (0x10u32 << 26) | (0 << 21) | (8 << 16) | (12 << 11);
        let mem = encode_le(word);
        let instr = disassemble_r3000a(&mem, 0x80000000).unwrap();
        assert_eq!(instr.mnemonic, "MFC0 t0, $12");
    }

    #[test]
    fn test_rfe() {
        let word = (0x10u32 << 26) | (0x10 << 21) | 0x10;
        let mem = encode_le(word);
        let instr = disassemble_r3000a(&mem, 0x80000000).unwrap();
        assert_eq!(instr.mnemonic, "RFE");
    }

    #[test]
    fn test_gte_rtps() {
        let word = (0x12u32 << 26) | (1 << 25) | 0x01;
        let mem = encode_le(word);
        let instr = disassemble_r3000a(&mem, 0x80000000).unwrap();
        assert!(instr.mnemonic.starts_with("RTPS"));
    }

    #[test]
    fn test_mfc2() {
        let word = (0x12u32 << 26) | (0 << 21) | (8 << 16) | (5 << 11);
        let mem = encode_le(word);
        let instr = disassemble_r3000a(&mem, 0x80000000).unwrap();
        assert_eq!(instr.mnemonic, "MFC2 t0, $5");
    }

    #[test]
    fn test_lwc2() {
        let word = (0x32u32 << 26) | (29 << 21) | (5 << 16) | 0x0010;
        let mem = encode_le(word);
        let instr = disassemble_r3000a(&mem, 0x80000000).unwrap();
        assert_eq!(instr.mnemonic, "LWC2 $5, 16(sp)");
    }

    #[test]
    fn test_short_memory() {
        let mem = [0x00, 0x00];
        assert!(disassemble_r3000a(&mem, 0x80000000).is_none());
    }

    #[test]
    fn test_li_pseudo() {
        let word = (9u32 << 26) | (0 << 21) | (8 << 16) | 42;
        let mem = encode_le(word);
        let instr = disassemble_r3000a(&mem, 0x80000000).unwrap();
        assert_eq!(instr.mnemonic, "LI t0, 42");
    }

    #[test]
    fn test_move_pseudo() {
        let word = (0u32 << 26) | (9 << 21) | (0 << 16) | (8 << 11) | 0x21;
        let mem = encode_le(word);
        let instr = disassemble_r3000a(&mem, 0x80000000).unwrap();
        assert_eq!(instr.mnemonic, "MOVE t0, t1");
    }
}
