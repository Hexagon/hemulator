//! Intel 8080 disassembler
//!
//! Provides comprehensive instruction disassembly for the Intel 8080 CPU,
//! covering all 256 opcodes including undocumented NOP/JMP/RET/CALL aliases.

use hemu_types::DisassembledInstruction;

// ── helpers ──────────────────────────────────────────────────────────────────

#[inline]
fn make(address: u32, bytes: &[u8], mnemonic: String) -> DisassembledInstruction {
    DisassembledInstruction::new(address, bytes.to_vec(), mnemonic)
}

/// Intel 8080 8-bit register name (0=B … 7=A).
///
/// The memory-reference pseudo-register is called `M` on the 8080
/// (Z80 spells the same thing `(HL)`).
#[inline]
fn reg(r: u8) -> &'static str {
    match r & 7 {
        0 => "B",
        1 => "C",
        2 => "D",
        3 => "E",
        4 => "H",
        5 => "L",
        6 => "M",
        _ => "A",
    }
}

// ── public entry point ────────────────────────────────────────────────────────

/// Disassemble a single Intel 8080 instruction.
///
/// `memory` must start at the first byte of the instruction; `address` is the
/// program-counter value of that byte.  Returns `None` if `memory` is too short
/// to decode the instruction.
pub fn disassemble_8080(memory: &[u8], address: u32) -> Option<DisassembledInstruction> {
    if memory.is_empty() {
        return None;
    }

    let opcode = memory[0];

    // ── 0x40–0x7F  MOV dst,src  (and HLT at 0x76) ────────────────────────────
    if (0x40..=0x7F).contains(&opcode) {
        if opcode == 0x76 {
            return Some(make(address, &memory[..1], "HLT".into()));
        }
        let dst = (opcode >> 3) & 7;
        let src = opcode & 7;
        return Some(make(
            address,
            &memory[..1],
            format!("MOV {},{}", reg(dst), reg(src)),
        ));
    }

    // ── 0x80–0xBF  ALU r ─────────────────────────────────────────────────────
    if (0x80..=0xBF).contains(&opcode) {
        let operand = reg(opcode & 7);
        let mnemonic = match (opcode >> 3) & 7 {
            0 => format!("ADD {}", operand),
            1 => format!("ADC {}", operand),
            2 => format!("SUB {}", operand),
            3 => format!("SBB {}", operand),
            4 => format!("ANA {}", operand),
            5 => format!("XRA {}", operand),
            6 => format!("ORA {}", operand),
            _ => format!("CMP {}", operand),
        };
        return Some(make(address, &memory[..1], mnemonic));
    }

    // ── main opcode table (0x00–0x3F and 0xC0–0xFF) ───────────────────────────
    macro_rules! need {
        ($n:expr) => {
            if memory.len() < $n {
                return None;
            }
        };
    }
    macro_rules! nn {
        () => {{
            need!(3);
            u16::from_le_bytes([memory[1], memory[2]])
        }};
    }
    macro_rules! n {
        () => {{
            need!(2);
            memory[1]
        }};
    }

    let (mnemonic, len): (String, usize) = match opcode {
        // ── NOP + undocumented NOP aliases ───────────────────────────────────
        0x00 | 0x08 | 0x10 | 0x18 | 0x20 | 0x28 | 0x30 | 0x38 => ("NOP".into(), 1),

        // ── LXI rp,d16 ───────────────────────────────────────────────────────
        0x01 => (format!("LXI B,${:04X}", nn!()), 3),
        0x11 => (format!("LXI D,${:04X}", nn!()), 3),
        0x21 => (format!("LXI H,${:04X}", nn!()), 3),
        0x31 => (format!("LXI SP,${:04X}", nn!()), 3),

        // ── STAX rp ──────────────────────────────────────────────────────────
        0x02 => ("STAX B".into(), 1),
        0x12 => ("STAX D".into(), 1),

        // ── INX rp ───────────────────────────────────────────────────────────
        0x03 => ("INX B".into(), 1),
        0x13 => ("INX D".into(), 1),
        0x23 => ("INX H".into(), 1),
        0x33 => ("INX SP".into(), 1),

        // ── INR r ────────────────────────────────────────────────────────────
        0x04 => ("INR B".into(), 1),
        0x0C => ("INR C".into(), 1),
        0x14 => ("INR D".into(), 1),
        0x1C => ("INR E".into(), 1),
        0x24 => ("INR H".into(), 1),
        0x2C => ("INR L".into(), 1),
        0x34 => ("INR M".into(), 1),
        0x3C => ("INR A".into(), 1),

        // ── DCR r ────────────────────────────────────────────────────────────
        0x05 => ("DCR B".into(), 1),
        0x0D => ("DCR C".into(), 1),
        0x15 => ("DCR D".into(), 1),
        0x1D => ("DCR E".into(), 1),
        0x25 => ("DCR H".into(), 1),
        0x2D => ("DCR L".into(), 1),
        0x35 => ("DCR M".into(), 1),
        0x3D => ("DCR A".into(), 1),

        // ── MVI r,d8 ─────────────────────────────────────────────────────────
        0x06 => (format!("MVI B,${:02X}", n!()), 2),
        0x0E => (format!("MVI C,${:02X}", n!()), 2),
        0x16 => (format!("MVI D,${:02X}", n!()), 2),
        0x1E => (format!("MVI E,${:02X}", n!()), 2),
        0x26 => (format!("MVI H,${:02X}", n!()), 2),
        0x2E => (format!("MVI L,${:02X}", n!()), 2),
        0x36 => (format!("MVI M,${:02X}", n!()), 2),
        0x3E => (format!("MVI A,${:02X}", n!()), 2),

        // ── Accumulator rotates ───────────────────────────────────────────────
        0x07 => ("RLC".into(), 1),
        0x0F => ("RRC".into(), 1),
        0x17 => ("RAL".into(), 1),
        0x1F => ("RAR".into(), 1),

        // ── DAD rp  (HL += rp) ────────────────────────────────────────────────
        0x09 => ("DAD B".into(), 1),
        0x19 => ("DAD D".into(), 1),
        0x29 => ("DAD H".into(), 1),
        0x39 => ("DAD SP".into(), 1),

        // ── LDAX rp ──────────────────────────────────────────────────────────
        0x0A => ("LDAX B".into(), 1),
        0x1A => ("LDAX D".into(), 1),

        // ── DCX rp ───────────────────────────────────────────────────────────
        0x0B => ("DCX B".into(), 1),
        0x1B => ("DCX D".into(), 1),
        0x2B => ("DCX H".into(), 1),
        0x3B => ("DCX SP".into(), 1),

        // ── Direct memory access ─────────────────────────────────────────────
        0x22 => (format!("SHLD ${:04X}", nn!()), 3),
        0x2A => (format!("LHLD ${:04X}", nn!()), 3),
        0x32 => (format!("STA ${:04X}", nn!()), 3),
        0x3A => (format!("LDA ${:04X}", nn!()), 3),

        // ── Miscellaneous accumulator / flag ops ─────────────────────────────
        0x27 => ("DAA".into(), 1),
        0x2F => ("CMA".into(), 1),
        0x37 => ("STC".into(), 1),
        0x3F => ("CMC".into(), 1),

        // ── 0x40–0xBF handled above ──────────────────────────────────────────

        // ── Conditional returns ───────────────────────────────────────────────
        0xC0 => ("RNZ".into(), 1),
        0xC8 => ("RZ".into(), 1),
        0xD0 => ("RNC".into(), 1),
        0xD8 => ("RC".into(), 1),
        0xE0 => ("RPO".into(), 1),
        0xE8 => ("RPE".into(), 1),
        0xF0 => ("RP".into(), 1),
        0xF8 => ("RM".into(), 1),

        // ── Unconditional return (0xD9 is undocumented alias) ─────────────────
        0xC9 | 0xD9 => ("RET".into(), 1),

        // ── POP rp ───────────────────────────────────────────────────────────
        0xC1 => ("POP B".into(), 1),
        0xD1 => ("POP D".into(), 1),
        0xE1 => ("POP H".into(), 1),
        0xF1 => ("POP PSW".into(), 1),

        // ── Conditional jumps ─────────────────────────────────────────────────
        0xC2 => (format!("JNZ ${:04X}", nn!()), 3),
        0xCA => (format!("JZ ${:04X}", nn!()), 3),
        0xD2 => (format!("JNC ${:04X}", nn!()), 3),
        0xDA => (format!("JC ${:04X}", nn!()), 3),
        0xE2 => (format!("JPO ${:04X}", nn!()), 3),
        0xEA => (format!("JPE ${:04X}", nn!()), 3),
        0xF2 => (format!("JP ${:04X}", nn!()), 3),
        0xFA => (format!("JM ${:04X}", nn!()), 3),

        // ── Unconditional jump (0xCB is undocumented alias) ───────────────────
        0xC3 | 0xCB => (format!("JMP ${:04X}", nn!()), 3),

        // ── Conditional calls ─────────────────────────────────────────────────
        0xC4 => (format!("CNZ ${:04X}", nn!()), 3),
        0xCC => (format!("CZ ${:04X}", nn!()), 3),
        0xD4 => (format!("CNC ${:04X}", nn!()), 3),
        0xDC => (format!("CC ${:04X}", nn!()), 3),
        0xE4 => (format!("CPO ${:04X}", nn!()), 3),
        0xEC => (format!("CPE ${:04X}", nn!()), 3),
        0xF4 => (format!("CP ${:04X}", nn!()), 3),
        0xFC => (format!("CM ${:04X}", nn!()), 3),

        // ── Unconditional call (0xDD/0xED/0xFD are undocumented aliases) ──────
        0xCD | 0xDD | 0xED | 0xFD => (format!("CALL ${:04X}", nn!()), 3),

        // ── PUSH rp ──────────────────────────────────────────────────────────
        0xC5 => ("PUSH B".into(), 1),
        0xD5 => ("PUSH D".into(), 1),
        0xE5 => ("PUSH H".into(), 1),
        0xF5 => ("PUSH PSW".into(), 1),

        // ── ALU immediate ─────────────────────────────────────────────────────
        0xC6 => (format!("ADI ${:02X}", n!()), 2),
        0xCE => (format!("ACI ${:02X}", n!()), 2),
        0xD6 => (format!("SUI ${:02X}", n!()), 2),
        0xDE => (format!("SBI ${:02X}", n!()), 2),
        0xE6 => (format!("ANI ${:02X}", n!()), 2),
        0xEE => (format!("XRI ${:02X}", n!()), 2),
        0xF6 => (format!("ORI ${:02X}", n!()), 2),
        0xFE => (format!("CPI ${:02X}", n!()), 2),

        // ── RST n ─────────────────────────────────────────────────────────────
        0xC7 => ("RST 0".into(), 1),
        0xCF => ("RST 1".into(), 1),
        0xD7 => ("RST 2".into(), 1),
        0xDF => ("RST 3".into(), 1),
        0xE7 => ("RST 4".into(), 1),
        0xEF => ("RST 5".into(), 1),
        0xF7 => ("RST 6".into(), 1),
        0xFF => ("RST 7".into(), 1),

        // ── I/O ───────────────────────────────────────────────────────────────
        0xD3 => (format!("OUT ${:02X}", n!()), 2),
        0xDB => (format!("IN ${:02X}", n!()), 2),

        // ── Miscellaneous ─────────────────────────────────────────────────────
        0xE3 => ("XTHL".into(), 1),
        0xE9 => ("PCHL".into(), 1),
        0xEB => ("XCHG".into(), 1),
        0xF3 => ("DI".into(), 1),
        0xF9 => ("SPHL".into(), 1),
        0xFB => ("EI".into(), 1),

        _ => (format!("DB ${:02X}", opcode), 1),
    };

    Some(make(address, &memory[..len], mnemonic))
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn dis(bytes: &[u8]) -> String {
        disassemble_8080(bytes, 0x0000).unwrap().mnemonic
    }

    fn dis_at(bytes: &[u8], addr: u32) -> DisassembledInstruction {
        disassemble_8080(bytes, addr).unwrap()
    }

    // ── basic coverage ────────────────────────────────────────────────────────

    #[test]
    fn test_nop() {
        assert_eq!(dis(&[0x00]), "NOP");
    }

    #[test]
    fn test_nop_undocumented_aliases() {
        for &op in &[0x08u8, 0x10, 0x18, 0x20, 0x28, 0x30, 0x38] {
            assert_eq!(dis(&[op]), "NOP", "opcode ${:02X} should be NOP", op);
        }
    }

    #[test]
    fn test_hlt() {
        assert_eq!(dis(&[0x76]), "HLT");
    }

    // ── loads ─────────────────────────────────────────────────────────────────

    #[test]
    fn test_lxi() {
        assert_eq!(dis(&[0x01, 0x34, 0x12]), "LXI B,$1234");
        assert_eq!(dis(&[0x11, 0x78, 0x56]), "LXI D,$5678");
        assert_eq!(dis(&[0x21, 0xCD, 0xAB]), "LXI H,$ABCD");
        assert_eq!(dis(&[0x31, 0xFF, 0x01]), "LXI SP,$01FF");
    }

    #[test]
    fn test_mvi() {
        assert_eq!(dis(&[0x06, 0x42]), "MVI B,$42");
        assert_eq!(dis(&[0x3E, 0xFF]), "MVI A,$FF");
        assert_eq!(dis(&[0x36, 0x00]), "MVI M,$00");
    }

    #[test]
    fn test_ldax_stax() {
        assert_eq!(dis(&[0x0A]), "LDAX B");
        assert_eq!(dis(&[0x1A]), "LDAX D");
        assert_eq!(dis(&[0x02]), "STAX B");
        assert_eq!(dis(&[0x12]), "STAX D");
    }

    #[test]
    fn test_lda_sta() {
        assert_eq!(dis(&[0x3A, 0x00, 0x20]), "LDA $2000");
        assert_eq!(dis(&[0x32, 0x00, 0x20]), "STA $2000");
    }

    #[test]
    fn test_lhld_shld() {
        assert_eq!(dis(&[0x2A, 0x00, 0x40]), "LHLD $4000");
        assert_eq!(dis(&[0x22, 0x00, 0x40]), "SHLD $4000");
    }

    // ── MOV ──────────────────────────────────────────────────────────────────

    #[test]
    fn test_mov_register_to_register() {
        assert_eq!(dis(&[0x41]), "MOV B,C");
        assert_eq!(dis(&[0x57]), "MOV D,A");
        assert_eq!(dis(&[0x7F]), "MOV A,A");
    }

    #[test]
    fn test_mov_memory() {
        assert_eq!(dis(&[0x46]), "MOV B,M");
        assert_eq!(dis(&[0x77]), "MOV M,A");
    }

    // ── arithmetic ────────────────────────────────────────────────────────────

    #[test]
    fn test_alu_register() {
        assert_eq!(dis(&[0x80]), "ADD B");
        assert_eq!(dis(&[0x88]), "ADC B");
        assert_eq!(dis(&[0x90]), "SUB B");
        assert_eq!(dis(&[0x98]), "SBB B");
        assert_eq!(dis(&[0xA0]), "ANA B");
        assert_eq!(dis(&[0xA8]), "XRA B");
        assert_eq!(dis(&[0xB0]), "ORA B");
        assert_eq!(dis(&[0xB8]), "CMP B");
    }

    #[test]
    fn test_alu_memory() {
        assert_eq!(dis(&[0x86]), "ADD M");
        assert_eq!(dis(&[0xBE]), "CMP M");
    }

    #[test]
    fn test_alu_immediate() {
        assert_eq!(dis(&[0xC6, 0x01]), "ADI $01");
        assert_eq!(dis(&[0xCE, 0x02]), "ACI $02");
        assert_eq!(dis(&[0xD6, 0x03]), "SUI $03");
        assert_eq!(dis(&[0xDE, 0x04]), "SBI $04");
        assert_eq!(dis(&[0xE6, 0x0F]), "ANI $0F");
        assert_eq!(dis(&[0xEE, 0xAA]), "XRI $AA");
        assert_eq!(dis(&[0xF6, 0x80]), "ORI $80");
        assert_eq!(dis(&[0xFE, 0xFF]), "CPI $FF");
    }

    #[test]
    fn test_inr_dcr() {
        assert_eq!(dis(&[0x04]), "INR B");
        assert_eq!(dis(&[0x3C]), "INR A");
        assert_eq!(dis(&[0x05]), "DCR B");
        assert_eq!(dis(&[0x3D]), "DCR A");
    }

    #[test]
    fn test_inx_dcx_dad() {
        assert_eq!(dis(&[0x03]), "INX B");
        assert_eq!(dis(&[0x23]), "INX H");
        assert_eq!(dis(&[0x0B]), "DCX B");
        assert_eq!(dis(&[0x3B]), "DCX SP");
        assert_eq!(dis(&[0x09]), "DAD B");
        assert_eq!(dis(&[0x39]), "DAD SP");
    }

    // ── rotates ───────────────────────────────────────────────────────────────

    #[test]
    fn test_rotates() {
        assert_eq!(dis(&[0x07]), "RLC");
        assert_eq!(dis(&[0x0F]), "RRC");
        assert_eq!(dis(&[0x17]), "RAL");
        assert_eq!(dis(&[0x1F]), "RAR");
    }

    // ── jumps / calls / returns ───────────────────────────────────────────────

    #[test]
    fn test_jmp() {
        assert_eq!(dis(&[0xC3, 0x00, 0x80]), "JMP $8000");
    }

    #[test]
    fn test_jmp_undocumented() {
        assert_eq!(dis(&[0xCB, 0x00, 0x80]), "JMP $8000");
    }

    #[test]
    fn test_conditional_jumps() {
        assert_eq!(dis(&[0xC2, 0x00, 0x80]), "JNZ $8000");
        assert_eq!(dis(&[0xCA, 0x00, 0x80]), "JZ $8000");
        assert_eq!(dis(&[0xD2, 0x00, 0x80]), "JNC $8000");
        assert_eq!(dis(&[0xDA, 0x00, 0x80]), "JC $8000");
        assert_eq!(dis(&[0xE2, 0x00, 0x80]), "JPO $8000");
        assert_eq!(dis(&[0xEA, 0x00, 0x80]), "JPE $8000");
        assert_eq!(dis(&[0xF2, 0x00, 0x80]), "JP $8000");
        assert_eq!(dis(&[0xFA, 0x00, 0x80]), "JM $8000");
    }

    #[test]
    fn test_call_ret() {
        assert_eq!(dis(&[0xCD, 0x00, 0x10]), "CALL $1000");
        assert_eq!(dis(&[0xC9]), "RET");
    }

    #[test]
    fn test_call_undocumented_aliases() {
        for &op in &[0xDDu8, 0xED, 0xFD] {
            assert_eq!(
                dis(&[op, 0x00, 0x10]),
                "CALL $1000",
                "opcode ${:02X} should be CALL",
                op
            );
        }
    }

    #[test]
    fn test_ret_undocumented() {
        assert_eq!(dis(&[0xD9]), "RET");
    }

    #[test]
    fn test_conditional_calls_and_returns() {
        assert_eq!(dis(&[0xC4, 0x00, 0x20]), "CNZ $2000");
        assert_eq!(dis(&[0xCC, 0x00, 0x20]), "CZ $2000");
        assert_eq!(dis(&[0xC0]), "RNZ");
        assert_eq!(dis(&[0xC8]), "RZ");
        assert_eq!(dis(&[0xD0]), "RNC");
        assert_eq!(dis(&[0xD8]), "RC");
        assert_eq!(dis(&[0xE0]), "RPO");
        assert_eq!(dis(&[0xE8]), "RPE");
        assert_eq!(dis(&[0xF0]), "RP");
        assert_eq!(dis(&[0xF8]), "RM");
    }

    // ── stack ─────────────────────────────────────────────────────────────────

    #[test]
    fn test_push_pop() {
        assert_eq!(dis(&[0xC5]), "PUSH B");
        assert_eq!(dis(&[0xD5]), "PUSH D");
        assert_eq!(dis(&[0xE5]), "PUSH H");
        assert_eq!(dis(&[0xF5]), "PUSH PSW");
        assert_eq!(dis(&[0xC1]), "POP B");
        assert_eq!(dis(&[0xD1]), "POP D");
        assert_eq!(dis(&[0xE1]), "POP H");
        assert_eq!(dis(&[0xF1]), "POP PSW");
    }

    // ── RST ───────────────────────────────────────────────────────────────────

    #[test]
    fn test_rst() {
        assert_eq!(dis(&[0xC7]), "RST 0");
        assert_eq!(dis(&[0xCF]), "RST 1");
        assert_eq!(dis(&[0xD7]), "RST 2");
        assert_eq!(dis(&[0xDF]), "RST 3");
        assert_eq!(dis(&[0xE7]), "RST 4");
        assert_eq!(dis(&[0xEF]), "RST 5");
        assert_eq!(dis(&[0xF7]), "RST 6");
        assert_eq!(dis(&[0xFF]), "RST 7");
    }

    // ── I/O ───────────────────────────────────────────────────────────────────

    #[test]
    fn test_in_out() {
        assert_eq!(dis(&[0xD3, 0x10]), "OUT $10");
        assert_eq!(dis(&[0xDB, 0x20]), "IN $20");
    }

    // ── misc single-byte ops ─────────────────────────────────────────────────

    #[test]
    fn test_misc() {
        assert_eq!(dis(&[0x27]), "DAA");
        assert_eq!(dis(&[0x2F]), "CMA");
        assert_eq!(dis(&[0x37]), "STC");
        assert_eq!(dis(&[0x3F]), "CMC");
        assert_eq!(dis(&[0xE3]), "XTHL");
        assert_eq!(dis(&[0xE9]), "PCHL");
        assert_eq!(dis(&[0xEB]), "XCHG");
        assert_eq!(dis(&[0xF3]), "DI");
        assert_eq!(dis(&[0xF9]), "SPHL");
        assert_eq!(dis(&[0xFB]), "EI");
    }

    // ── address and byte fields ───────────────────────────────────────────────

    #[test]
    fn test_instruction_address_preserved() {
        let insn = dis_at(&[0xC9], 0x1234);
        assert_eq!(insn.address, 0x1234);
    }

    #[test]
    fn test_instruction_bytes_1() {
        let insn = dis_at(&[0x00], 0x0000);
        assert_eq!(insn.bytes, vec![0x00]);
        assert_eq!(insn.len(), 1);
    }

    #[test]
    fn test_instruction_bytes_2() {
        let insn = dis_at(&[0x3E, 0x42], 0x0000);
        assert_eq!(insn.bytes, vec![0x3E, 0x42]);
        assert_eq!(insn.len(), 2);
    }

    #[test]
    fn test_instruction_bytes_3() {
        let insn = dis_at(&[0xC3, 0x00, 0x80], 0x0000);
        assert_eq!(insn.bytes, vec![0xC3, 0x00, 0x80]);
        assert_eq!(insn.len(), 3);
    }

    // ── truncated input ───────────────────────────────────────────────────────

    #[test]
    fn test_empty_returns_none() {
        assert!(disassemble_8080(&[], 0).is_none());
    }

    #[test]
    fn test_truncated_2byte_returns_none() {
        // MVI A needs 2 bytes
        assert!(disassemble_8080(&[0x3E], 0).is_none());
    }

    #[test]
    fn test_truncated_3byte_returns_none() {
        // JMP needs 3 bytes
        assert!(disassemble_8080(&[0xC3, 0x00], 0).is_none());
    }

    // ── extra context bytes are ignored ──────────────────────────────────────

    #[test]
    fn test_extra_bytes_ignored() {
        // 1-byte NOP with extra context — should still decode fine
        let insn = dis_at(&[0x00, 0xFF, 0xFF], 0x0000);
        assert_eq!(insn.mnemonic, "NOP");
        assert_eq!(insn.len(), 1);
    }
}
