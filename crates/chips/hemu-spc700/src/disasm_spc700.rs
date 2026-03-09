//! Sony SPC700 CPU disassembler
//!
//! Provides instruction disassembly for the Sony SPC700 CPU used in the SNES APU.

use hemu_types::DisassembledInstruction;

/// Disassemble a SPC700 instruction from memory.
///
/// # Arguments
/// * `memory` - Byte slice beginning at the instruction to disassemble
/// * `address` - Address of the instruction (used for branch target comments)
///
/// # Returns
/// `Some(DisassembledInstruction)` on success, `None` if `memory` is empty or
/// too short to contain the full instruction.
pub fn disassemble_spc700(memory: &[u8], address: u32) -> Option<DisassembledInstruction> {
    if memory.is_empty() {
        return None;
    }

    let opcode = memory[0];
    let ops = memory.get(1..).unwrap_or(&[]);
    let (byte_count, mnemonic, comment) = decode_instruction(opcode, ops, address);

    if memory.len() < byte_count {
        return None;
    }

    let bytes = memory[..byte_count].to_vec();
    let mut result = DisassembledInstruction::new(address, bytes, mnemonic);
    if let Some(c) = comment {
        result = result.with_comment(c);
    }
    Some(result)
}

/// Returns `(byte_count, mnemonic_string, optional_comment)` for the given opcode.
fn decode_instruction(opcode: u8, ops: &[u8], address: u32) -> (usize, String, Option<String>) {
    let b0 = ops.first().copied().unwrap_or(0);
    let b1 = ops.get(1).copied().unwrap_or(0);

    // Compute branch target for 2-byte branch (opcode + 1 rel byte).
    // PC after fetch = address + 2; target = that + signed offset.
    let branch2 = |rel: u8| -> (String, String) {
        let offset = rel as i8;
        let target = ((address as i32) + 2 + (offset as i32)) as u16;
        (format!("${:04X}", target), format!("-> ${:04X}", target))
    };

    // Compute branch target for 3-byte branch (opcode + 1 dp byte + 1 rel byte).
    let branch3 = |rel: u8| -> (String, String) {
        let offset = rel as i8;
        let target = ((address as i32) + 3 + (offset as i32)) as u16;
        (format!("${:04X}", target), format!("-> ${:04X}", target))
    };

    // Decode the 13-bit address and 3-bit bit-number packed in a 16-bit word.
    // Bits 15:13 = bit index; bits 12:0 = memory address.
    let membit = |lo: u8, hi: u8| -> String {
        let word = u16::from_le_bytes([lo, hi]);
        let bit = (word >> 13) as u8;
        let addr = word & 0x1FFF;
        format!("${:04X}.{}", addr, bit)
    };

    let abs16 = |lo: u8, hi: u8| -> String { format!("${:04X}", u16::from_le_bytes([lo, hi])) };

    match opcode {
        // ── 0x00 ────────────────────────────────────────────────────────────────
        0x00 => (1, "NOP".into(), None),
        0x01 => (1, "TCALL 0".into(), None),
        0x02 => (2, format!("SET1 ${:02X}.0", b0), None),
        0x03 => {
            let (tgt, cmt) = branch3(b1);
            (3, format!("BBS ${:02X}.0, {}", b0, tgt), Some(cmt))
        }
        0x04 => (2, format!("OR A, ${:02X}", b0), None),
        0x05 => (3, format!("OR A, {}", abs16(b0, b1)), None),
        0x06 => (1, "OR A, (X)".into(), None),
        0x07 => (2, format!("OR A, [${:02X}+X]", b0), None),
        0x08 => (2, format!("OR A, #${:02X}", b0), None),
        0x09 => (3, format!("OR ${:02X}, ${:02X}", b0, b1), None),
        0x0A => (3, format!("OR1 C, {}", membit(b0, b1)), None),
        0x0B => (2, format!("ASL ${:02X}", b0), None),
        0x0C => (3, format!("ASL {}", abs16(b0, b1)), None),
        0x0D => (1, "PUSH PSW".into(), None),
        0x0E => (3, format!("TSET1 {}", abs16(b0, b1)), None),
        0x0F => (1, "BRK".into(), None),

        // ── 0x10 ────────────────────────────────────────────────────────────────
        0x10 => {
            let (tgt, cmt) = branch2(b0);
            (2, format!("BPL {}", tgt), Some(cmt))
        }
        0x11 => (1, "TCALL 1".into(), None),
        0x12 => (2, format!("CLR1 ${:02X}.0", b0), None),
        0x13 => {
            let (tgt, cmt) = branch3(b1);
            (3, format!("BBC ${:02X}.0, {}", b0, tgt), Some(cmt))
        }
        0x14 => (2, format!("OR A, ${:02X}+X", b0), None),
        0x15 => (3, format!("OR A, {}+X", abs16(b0, b1)), None),
        0x16 => (3, format!("OR A, {}+Y", abs16(b0, b1)), None),
        0x17 => (2, format!("OR A, [${:02X}]+Y", b0), None),
        0x18 => (3, format!("OR ${:02X}, #${:02X}", b1, b0), None),
        0x19 => (1, "OR (X), (Y)".into(), None),
        0x1A => (2, format!("DECW ${:02X}", b0), None),
        0x1B => (2, format!("ASL ${:02X}+X", b0), None),
        0x1C => (1, "ASL A".into(), None),
        0x1D => (1, "DEC X".into(), None),
        0x1E => (3, format!("CMP X, {}", abs16(b0, b1)), None),
        0x1F => (3, format!("JMP [{}+X]", abs16(b0, b1)), None),

        // ── 0x20 ────────────────────────────────────────────────────────────────
        0x20 => (1, "CLRP".into(), None),
        0x21 => (1, "TCALL 2".into(), None),
        0x22 => (2, format!("SET1 ${:02X}.1", b0), None),
        0x23 => {
            let (tgt, cmt) = branch3(b1);
            (3, format!("BBS ${:02X}.1, {}", b0, tgt), Some(cmt))
        }
        0x24 => (2, format!("AND A, ${:02X}", b0), None),
        0x25 => (3, format!("AND A, {}", abs16(b0, b1)), None),
        0x26 => (1, "AND A, (X)".into(), None),
        0x27 => (2, format!("AND A, [${:02X}+X]", b0), None),
        0x28 => (2, format!("AND A, #${:02X}", b0), None),
        0x29 => (3, format!("AND ${:02X}, ${:02X}", b0, b1), None),
        0x2A => (3, format!("OR1 C, /{}", membit(b0, b1)), None),
        0x2B => (2, format!("ROL ${:02X}", b0), None),
        0x2C => (3, format!("ROL {}", abs16(b0, b1)), None),
        0x2D => (1, "PUSH A".into(), None),
        0x2E => {
            let (tgt, cmt) = branch3(b1);
            (3, format!("CBNE ${:02X}, {}", b0, tgt), Some(cmt))
        }
        0x2F => {
            let (tgt, cmt) = branch2(b0);
            (2, format!("BRA {}", tgt), Some(cmt))
        }

        // ── 0x30 ────────────────────────────────────────────────────────────────
        0x30 => {
            let (tgt, cmt) = branch2(b0);
            (2, format!("BMI {}", tgt), Some(cmt))
        }
        0x31 => (1, "TCALL 3".into(), None),
        0x32 => (2, format!("CLR1 ${:02X}.1", b0), None),
        0x33 => {
            let (tgt, cmt) = branch3(b1);
            (3, format!("BBC ${:02X}.1, {}", b0, tgt), Some(cmt))
        }
        0x34 => (2, format!("AND A, ${:02X}+X", b0), None),
        0x35 => (3, format!("AND A, {}+X", abs16(b0, b1)), None),
        0x36 => (3, format!("AND A, {}+Y", abs16(b0, b1)), None),
        0x37 => (2, format!("AND A, [${:02X}]+Y", b0), None),
        0x38 => (3, format!("AND ${:02X}, #${:02X}", b1, b0), None),
        0x39 => (1, "AND (X), (Y)".into(), None),
        0x3A => (2, format!("INCW ${:02X}", b0), None),
        0x3B => (2, format!("ROL ${:02X}+X", b0), None),
        0x3C => (1, "ROL A".into(), None),
        0x3D => (1, "INC X".into(), None),
        0x3E => (2, format!("CMP X, ${:02X}", b0), None),
        0x3F => (3, format!("CALL {}", abs16(b0, b1)), None),

        // ── 0x40 ────────────────────────────────────────────────────────────────
        0x40 => (1, "SETP".into(), None),
        0x41 => (1, "TCALL 4".into(), None),
        0x42 => (2, format!("SET1 ${:02X}.2", b0), None),
        0x43 => {
            let (tgt, cmt) = branch3(b1);
            (3, format!("BBS ${:02X}.2, {}", b0, tgt), Some(cmt))
        }
        0x44 => (2, format!("EOR A, ${:02X}", b0), None),
        0x45 => (3, format!("EOR A, {}", abs16(b0, b1)), None),
        0x46 => (1, "EOR A, (X)".into(), None),
        0x47 => (2, format!("EOR A, [${:02X}+X]", b0), None),
        0x48 => (2, format!("EOR A, #${:02X}", b0), None),
        0x49 => (3, format!("EOR ${:02X}, ${:02X}", b0, b1), None),
        0x4A => (3, format!("AND1 C, {}", membit(b0, b1)), None),
        0x4B => (2, format!("LSR ${:02X}", b0), None),
        0x4C => (3, format!("LSR {}", abs16(b0, b1)), None),
        0x4D => (1, "PUSH X".into(), None),
        0x4E => (3, format!("TCLR1 {}", abs16(b0, b1)), None),
        0x4F => (2, format!("PCALL #${:02X}", b0), None),

        // ── 0x50 ────────────────────────────────────────────────────────────────
        0x50 => {
            let (tgt, cmt) = branch2(b0);
            (2, format!("BVC {}", tgt), Some(cmt))
        }
        0x51 => (1, "TCALL 5".into(), None),
        0x52 => (2, format!("CLR1 ${:02X}.2", b0), None),
        0x53 => {
            let (tgt, cmt) = branch3(b1);
            (3, format!("BBC ${:02X}.2, {}", b0, tgt), Some(cmt))
        }
        0x54 => (2, format!("EOR A, ${:02X}+X", b0), None),
        0x55 => (3, format!("EOR A, {}+X", abs16(b0, b1)), None),
        0x56 => (3, format!("EOR A, {}+Y", abs16(b0, b1)), None),
        0x57 => (2, format!("EOR A, [${:02X}]+Y", b0), None),
        0x58 => (3, format!("EOR ${:02X}, #${:02X}", b1, b0), None),
        0x59 => (1, "EOR (X), (Y)".into(), None),
        0x5A => (2, format!("CMPW YA, ${:02X}", b0), None),
        0x5B => (2, format!("LSR ${:02X}+X", b0), None),
        0x5C => (1, "LSR A".into(), None),
        0x5D => (1, "MOV X, A".into(), None),
        0x5E => (3, format!("CMP Y, {}", abs16(b0, b1)), None),
        0x5F => (3, format!("JMP {}", abs16(b0, b1)), None),

        // ── 0x60 ────────────────────────────────────────────────────────────────
        0x60 => (1, "CLRC".into(), None),
        0x61 => (1, "TCALL 6".into(), None),
        0x62 => (2, format!("SET1 ${:02X}.3", b0), None),
        0x63 => {
            let (tgt, cmt) = branch3(b1);
            (3, format!("BBS ${:02X}.3, {}", b0, tgt), Some(cmt))
        }
        0x64 => (2, format!("CMP A, ${:02X}", b0), None),
        0x65 => (3, format!("CMP A, {}", abs16(b0, b1)), None),
        0x66 => (1, "CMP A, (X)".into(), None),
        0x67 => (2, format!("CMP A, [${:02X}+X]", b0), None),
        0x68 => (2, format!("CMP A, #${:02X}", b0), None),
        0x69 => (3, format!("CMP ${:02X}, ${:02X}", b0, b1), None),
        0x6A => (3, format!("AND1 C, /{}", membit(b0, b1)), None),
        0x6B => (2, format!("ROR ${:02X}", b0), None),
        0x6C => (3, format!("ROR {}", abs16(b0, b1)), None),
        0x6D => (1, "PUSH Y".into(), None),
        0x6E => {
            let (tgt, cmt) = branch3(b1);
            (3, format!("DBNZ ${:02X}, {}", b0, tgt), Some(cmt))
        }
        0x6F => (1, "RET".into(), None),

        // ── 0x70 ────────────────────────────────────────────────────────────────
        0x70 => {
            let (tgt, cmt) = branch2(b0);
            (2, format!("BVS {}", tgt), Some(cmt))
        }
        0x71 => (1, "TCALL 7".into(), None),
        0x72 => (2, format!("CLR1 ${:02X}.3", b0), None),
        0x73 => {
            let (tgt, cmt) = branch3(b1);
            (3, format!("BBC ${:02X}.3, {}", b0, tgt), Some(cmt))
        }
        0x74 => (2, format!("CMP A, ${:02X}+X", b0), None),
        0x75 => (3, format!("CMP A, {}+X", abs16(b0, b1)), None),
        0x76 => (3, format!("CMP A, {}+Y", abs16(b0, b1)), None),
        0x77 => (2, format!("CMP A, [${:02X}]+Y", b0), None),
        0x78 => (3, format!("CMP ${:02X}, #${:02X}", b1, b0), None),
        0x79 => (1, "CMP (X), (Y)".into(), None),
        0x7A => (2, format!("ADDW YA, ${:02X}", b0), None),
        0x7B => (2, format!("ROR ${:02X}+X", b0), None),
        0x7C => (1, "ROR A".into(), None),
        0x7D => (1, "MOV A, X".into(), None),
        0x7E => (2, format!("CMP Y, ${:02X}", b0), None),
        0x7F => (1, "RETI".into(), None),

        // ── 0x80 ────────────────────────────────────────────────────────────────
        0x80 => (1, "SETC".into(), None),
        0x81 => (1, "TCALL 8".into(), None),
        0x82 => (2, format!("SET1 ${:02X}.4", b0), None),
        0x83 => {
            let (tgt, cmt) = branch3(b1);
            (3, format!("BBS ${:02X}.4, {}", b0, tgt), Some(cmt))
        }
        0x84 => (2, format!("ADC A, ${:02X}", b0), None),
        0x85 => (3, format!("ADC A, {}", abs16(b0, b1)), None),
        0x86 => (1, "ADC A, (X)".into(), None),
        0x87 => (2, format!("ADC A, [${:02X}+X]", b0), None),
        0x88 => (2, format!("ADC A, #${:02X}", b0), None),
        0x89 => (3, format!("ADC ${:02X}, ${:02X}", b0, b1), None),
        0x8A => (3, format!("EOR1 C, {}", membit(b0, b1)), None),
        0x8B => (2, format!("DEC ${:02X}", b0), None),
        0x8C => (3, format!("DEC {}", abs16(b0, b1)), None),
        0x8D => (2, format!("MOV Y, #${:02X}", b0), None),
        0x8E => (1, "POP PSW".into(), None),
        0x8F => (3, format!("MOV ${:02X}, #${:02X}", b1, b0), None),

        // ── 0x90 ────────────────────────────────────────────────────────────────
        0x90 => {
            let (tgt, cmt) = branch2(b0);
            (2, format!("BCC {}", tgt), Some(cmt))
        }
        0x91 => (1, "TCALL 9".into(), None),
        0x92 => (2, format!("CLR1 ${:02X}.4", b0), None),
        0x93 => {
            let (tgt, cmt) = branch3(b1);
            (3, format!("BBC ${:02X}.4, {}", b0, tgt), Some(cmt))
        }
        0x94 => (2, format!("ADC A, ${:02X}+X", b0), None),
        0x95 => (3, format!("ADC A, {}+X", abs16(b0, b1)), None),
        0x96 => (3, format!("ADC A, {}+Y", abs16(b0, b1)), None),
        0x97 => (2, format!("ADC A, [${:02X}]+Y", b0), None),
        0x98 => (3, format!("ADC ${:02X}, #${:02X}", b1, b0), None),
        0x99 => (1, "ADC (X), (Y)".into(), None),
        0x9A => (2, format!("SUBW YA, ${:02X}", b0), None),
        0x9B => (2, format!("DEC ${:02X}+X", b0), None),
        0x9C => (1, "DEC A".into(), None),
        0x9D => (1, "MOV X, SP".into(), None),
        0x9E => (1, "DIV YA, X".into(), None),
        0x9F => (1, "XCN A".into(), None),

        // ── 0xA0 ────────────────────────────────────────────────────────────────
        0xA0 => (1, "EI".into(), None),
        0xA1 => (1, "TCALL 10".into(), None),
        0xA2 => (2, format!("SET1 ${:02X}.5", b0), None),
        0xA3 => {
            let (tgt, cmt) = branch3(b1);
            (3, format!("BBS ${:02X}.5, {}", b0, tgt), Some(cmt))
        }
        0xA4 => (2, format!("SBC A, ${:02X}", b0), None),
        0xA5 => (3, format!("SBC A, {}", abs16(b0, b1)), None),
        0xA6 => (1, "SBC A, (X)".into(), None),
        0xA7 => (2, format!("SBC A, [${:02X}+X]", b0), None),
        0xA8 => (2, format!("SBC A, #${:02X}", b0), None),
        0xA9 => (3, format!("SBC ${:02X}, ${:02X}", b0, b1), None),
        0xAA => (3, format!("MOV1 C, {}", membit(b0, b1)), None),
        0xAB => (2, format!("INC ${:02X}", b0), None),
        0xAC => (3, format!("INC {}", abs16(b0, b1)), None),
        0xAD => (2, format!("CMP Y, #${:02X}", b0), None),
        0xAE => (1, "POP A".into(), None),
        0xAF => (1, "MOV (X)+, A".into(), None),

        // ── 0xB0 ────────────────────────────────────────────────────────────────
        0xB0 => {
            let (tgt, cmt) = branch2(b0);
            (2, format!("BCS {}", tgt), Some(cmt))
        }
        0xB1 => (1, "TCALL 11".into(), None),
        0xB2 => (2, format!("CLR1 ${:02X}.5", b0), None),
        0xB3 => {
            let (tgt, cmt) = branch3(b1);
            (3, format!("BBC ${:02X}.5, {}", b0, tgt), Some(cmt))
        }
        0xB4 => (2, format!("SBC A, ${:02X}+X", b0), None),
        0xB5 => (3, format!("SBC A, {}+X", abs16(b0, b1)), None),
        0xB6 => (3, format!("SBC A, {}+Y", abs16(b0, b1)), None),
        0xB7 => (2, format!("SBC A, [${:02X}]+Y", b0), None),
        0xB8 => (3, format!("SBC ${:02X}, #${:02X}", b1, b0), None),
        0xB9 => (1, "SBC (X), (Y)".into(), None),
        0xBA => (2, format!("MOVW YA, ${:02X}", b0), None),
        0xBB => (2, format!("INC ${:02X}+X", b0), None),
        0xBC => (1, "INC A".into(), None),
        0xBD => (1, "MOV SP, X".into(), None),
        0xBE => (1, "DAS A".into(), None),
        0xBF => (1, "MOV A, (X)+".into(), None),

        // ── 0xC0 ────────────────────────────────────────────────────────────────
        0xC0 => (1, "DI".into(), None),
        0xC1 => (1, "TCALL 12".into(), None),
        0xC2 => (2, format!("SET1 ${:02X}.6", b0), None),
        0xC3 => {
            let (tgt, cmt) = branch3(b1);
            (3, format!("BBS ${:02X}.6, {}", b0, tgt), Some(cmt))
        }
        0xC4 => (2, format!("MOV ${:02X}, A", b0), None),
        0xC5 => (3, format!("MOV {}, A", abs16(b0, b1)), None),
        0xC6 => (1, "MOV (X), A".into(), None),
        0xC7 => (2, format!("MOV [${:02X}+X], A", b0), None),
        0xC8 => (2, format!("CMP X, #${:02X}", b0), None),
        0xC9 => (3, format!("MOV {}, X", abs16(b0, b1)), None),
        0xCA => (3, format!("MOV1 {}, C", membit(b0, b1)), None),
        0xCB => (2, format!("MOV ${:02X}, Y", b0), None),
        0xCC => (3, format!("MOV {}, Y", abs16(b0, b1)), None),
        0xCD => (2, format!("MOV X, #${:02X}", b0), None),
        0xCE => (1, "POP X".into(), None),
        0xCF => (1, "MUL YA".into(), None),

        // ── 0xD0 ────────────────────────────────────────────────────────────────
        0xD0 => {
            let (tgt, cmt) = branch2(b0);
            (2, format!("BNE {}", tgt), Some(cmt))
        }
        0xD1 => (1, "TCALL 13".into(), None),
        0xD2 => (2, format!("CLR1 ${:02X}.6", b0), None),
        0xD3 => {
            let (tgt, cmt) = branch3(b1);
            (3, format!("BBC ${:02X}.6, {}", b0, tgt), Some(cmt))
        }
        0xD4 => (2, format!("MOV ${:02X}+X, A", b0), None),
        0xD5 => (3, format!("MOV {}+X, A", abs16(b0, b1)), None),
        0xD6 => (3, format!("MOV {}+Y, A", abs16(b0, b1)), None),
        0xD7 => (2, format!("MOV [${:02X}]+Y, A", b0), None),
        0xD8 => (2, format!("MOV ${:02X}, X", b0), None),
        0xD9 => (2, format!("MOV ${:02X}+Y, X", b0), None),
        0xDA => (2, format!("MOVW ${:02X}, YA", b0), None),
        0xDB => (2, format!("MOV ${:02X}+X, Y", b0), None),
        0xDC => (1, "DEC Y".into(), None),
        0xDD => (1, "MOV A, Y".into(), None),
        0xDE => {
            let (tgt, cmt) = branch3(b1);
            (3, format!("CBNE ${:02X}+X, {}", b0, tgt), Some(cmt))
        }
        0xDF => (1, "DAA A".into(), None),

        // ── 0xE0 ────────────────────────────────────────────────────────────────
        0xE0 => (1, "CLRV".into(), None),
        0xE1 => (1, "TCALL 14".into(), None),
        0xE2 => (2, format!("SET1 ${:02X}.7", b0), None),
        0xE3 => {
            let (tgt, cmt) = branch3(b1);
            (3, format!("BBS ${:02X}.7, {}", b0, tgt), Some(cmt))
        }
        0xE4 => (2, format!("MOV A, ${:02X}", b0), None),
        0xE5 => (3, format!("MOV A, {}", abs16(b0, b1)), None),
        0xE6 => (1, "MOV A, (X)".into(), None),
        0xE7 => (2, format!("MOV A, [${:02X}+X]", b0), None),
        0xE8 => (2, format!("MOV A, #${:02X}", b0), None),
        0xE9 => (3, format!("MOV X, {}", abs16(b0, b1)), None),
        0xEA => (3, format!("NOT1 {}", membit(b0, b1)), None),
        0xEB => (2, format!("MOV Y, ${:02X}", b0), None),
        0xEC => (3, format!("MOV Y, {}", abs16(b0, b1)), None),
        0xED => (1, "NOTC".into(), None),
        0xEE => (1, "POP Y".into(), None),
        0xEF => (1, "SLEEP".into(), None),

        // ── 0xF0 ────────────────────────────────────────────────────────────────
        0xF0 => {
            let (tgt, cmt) = branch2(b0);
            (2, format!("BEQ {}", tgt), Some(cmt))
        }
        0xF1 => (1, "TCALL 15".into(), None),
        0xF2 => (2, format!("CLR1 ${:02X}.7", b0), None),
        0xF3 => {
            let (tgt, cmt) = branch3(b1);
            (3, format!("BBC ${:02X}.7, {}", b0, tgt), Some(cmt))
        }
        0xF4 => (2, format!("MOV A, ${:02X}+X", b0), None),
        0xF5 => (3, format!("MOV A, {}+X", abs16(b0, b1)), None),
        0xF6 => (3, format!("MOV A, {}+Y", abs16(b0, b1)), None),
        0xF7 => (2, format!("MOV A, [${:02X}]+Y", b0), None),
        0xF8 => (2, format!("MOV X, ${:02X}", b0), None),
        0xF9 => (2, format!("MOV X, ${:02X}+Y", b0), None),
        0xFA => (3, format!("MOV ${:02X}, ${:02X}", b1, b0), None),
        0xFB => (2, format!("MOV Y, ${:02X}+X", b0), None),
        0xFC => (1, "INC Y".into(), None),
        0xFD => (1, "MOV Y, A".into(), None),
        0xFE => {
            let (tgt, cmt) = branch2(b0);
            (2, format!("DBNZ Y, {}", tgt), Some(cmt))
        }
        0xFF => (1, "STOP".into(), None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── basic implied instructions ────────────────────────────────────────────

    #[test]
    fn test_nop() {
        let mem = [0x00u8];
        let instr = disassemble_spc700(&mem, 0x0200).unwrap();
        assert_eq!(instr.address, 0x0200);
        assert_eq!(instr.bytes, vec![0x00]);
        assert_eq!(instr.mnemonic, "NOP");
        assert_eq!(instr.len(), 1);
        assert!(instr.comment.is_none());
    }

    #[test]
    fn test_brk() {
        let mem = [0x0Fu8];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "BRK");
        assert_eq!(instr.len(), 1);
    }

    #[test]
    fn test_ret() {
        let mem = [0x6Fu8];
        let instr = disassemble_spc700(&mem, 0x1000).unwrap();
        assert_eq!(instr.mnemonic, "RET");
        assert_eq!(instr.len(), 1);
    }

    #[test]
    fn test_reti() {
        let mem = [0x7Fu8];
        let instr = disassemble_spc700(&mem, 0x1000).unwrap();
        assert_eq!(instr.mnemonic, "RETI");
        assert_eq!(instr.len(), 1);
    }

    #[test]
    fn test_stop() {
        let mem = [0xFFu8];
        let instr = disassemble_spc700(&mem, 0xFFFF).unwrap();
        assert_eq!(instr.mnemonic, "STOP");
        assert_eq!(instr.len(), 1);
    }

    #[test]
    fn test_sleep() {
        let mem = [0xEFu8];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "SLEEP");
        assert_eq!(instr.len(), 1);
    }

    // ── TCALL ────────────────────────────────────────────────────────────────

    #[test]
    fn test_tcall_0() {
        let mem = [0x01u8];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "TCALL 0");
        assert_eq!(instr.len(), 1);
    }

    #[test]
    fn test_tcall_15() {
        let mem = [0xF1u8];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "TCALL 15");
        assert_eq!(instr.len(), 1);
    }

    // ── immediate / direct-page loads ────────────────────────────────────────

    #[test]
    fn test_mov_a_imm() {
        let mem = [0xE8u8, 0x42];
        let instr = disassemble_spc700(&mem, 0x0300).unwrap();
        assert_eq!(instr.mnemonic, "MOV A, #$42");
        assert_eq!(instr.bytes, vec![0xE8, 0x42]);
        assert_eq!(instr.len(), 2);
    }

    #[test]
    fn test_mov_a_dp() {
        let mem = [0xE4u8, 0x10];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "MOV A, $10");
        assert_eq!(instr.len(), 2);
    }

    #[test]
    fn test_mov_a_abs() {
        let mem = [0xE5u8, 0x34, 0x12];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "MOV A, $1234");
        assert_eq!(instr.len(), 3);
    }

    #[test]
    fn test_mov_x_imm() {
        let mem = [0xCDu8, 0xFF];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "MOV X, #$FF");
        assert_eq!(instr.len(), 2);
    }

    #[test]
    fn test_mov_y_imm() {
        let mem = [0x8Du8, 0x00];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "MOV Y, #$00");
        assert_eq!(instr.len(), 2);
    }

    // ── stores ───────────────────────────────────────────────────────────────

    #[test]
    fn test_mov_dp_a() {
        let mem = [0xC4u8, 0x20];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "MOV $20, A");
        assert_eq!(instr.len(), 2);
    }

    #[test]
    fn test_mov_abs_a() {
        let mem = [0xC5u8, 0x00, 0xF0];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "MOV $F000, A");
        assert_eq!(instr.len(), 3);
    }

    #[test]
    fn test_mov_dp_imm() {
        // 0x8F: MOV dp, #imm — bytes: [0x8F][imm][dp]
        let mem = [0x8Fu8, 0xAB, 0x10];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "MOV $10, #$AB");
        assert_eq!(instr.len(), 3);
    }

    // ── ALU operations ───────────────────────────────────────────────────────

    #[test]
    fn test_or_a_imm() {
        let mem = [0x08u8, 0x0F];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "OR A, #$0F");
        assert_eq!(instr.len(), 2);
    }

    #[test]
    fn test_and_a_dp() {
        let mem = [0x24u8, 0x55];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "AND A, $55");
        assert_eq!(instr.len(), 2);
    }

    #[test]
    fn test_eor_dp_dp() {
        let mem = [0x49u8, 0x10, 0x20];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "EOR $10, $20");
        assert_eq!(instr.len(), 3);
    }

    #[test]
    fn test_adc_a_imm() {
        let mem = [0x88u8, 0x01];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "ADC A, #$01");
        assert_eq!(instr.len(), 2);
    }

    #[test]
    fn test_sbc_dp_imm() {
        // 0xB8: SBC dp, imm — bytes [0xB8][imm][dp]
        let mem = [0xB8u8, 0x01, 0x30];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "SBC $30, #$01");
        assert_eq!(instr.len(), 3);
    }

    // ── shifts/rotates ───────────────────────────────────────────────────────

    #[test]
    fn test_asl_a() {
        let mem = [0x1Cu8];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "ASL A");
        assert_eq!(instr.len(), 1);
    }

    #[test]
    fn test_lsr_dp() {
        let mem = [0x4Bu8, 0x08];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "LSR $08");
        assert_eq!(instr.len(), 2);
    }

    #[test]
    fn test_rol_abs() {
        let mem = [0x2Cu8, 0x00, 0x01];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "ROL $0100");
        assert_eq!(instr.len(), 3);
    }

    #[test]
    fn test_ror_dp_x() {
        let mem = [0x7Bu8, 0x04];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "ROR $04+X");
        assert_eq!(instr.len(), 2);
    }

    // ── inc/dec ───────────────────────────────────────────────────────────────

    #[test]
    fn test_inc_a() {
        let mem = [0xBCu8];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "INC A");
        assert_eq!(instr.len(), 1);
    }

    #[test]
    fn test_dec_x() {
        let mem = [0x1Du8];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "DEC X");
        assert_eq!(instr.len(), 1);
    }

    #[test]
    fn test_incw_dp() {
        let mem = [0x3Au8, 0x12];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "INCW $12");
        assert_eq!(instr.len(), 2);
    }

    #[test]
    fn test_decw_dp() {
        let mem = [0x1Au8, 0xF0];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "DECW $F0");
        assert_eq!(instr.len(), 2);
    }

    // ── branches (with target address comment) ───────────────────────────────

    #[test]
    fn test_bra_forward() {
        // BRA +$10: PC=0x1000, next PC=0x1002, target=0x1012
        let mem = [0x2Fu8, 0x10];
        let instr = disassemble_spc700(&mem, 0x1000).unwrap();
        assert_eq!(instr.mnemonic, "BRA $1012");
        assert_eq!(instr.comment.as_deref(), Some("-> $1012"));
        assert_eq!(instr.len(), 2);
    }

    #[test]
    fn test_bra_backward() {
        // BRA -2 (0xFE as i8 = -2): PC=0x1000, next PC=0x1002, target=0x1000
        let mem = [0x2Fu8, 0xFE];
        let instr = disassemble_spc700(&mem, 0x1000).unwrap();
        assert_eq!(instr.mnemonic, "BRA $1000");
        assert_eq!(instr.comment.as_deref(), Some("-> $1000"));
    }

    #[test]
    fn test_bne() {
        // BNE $05: PC=0x0300, target=0x0307
        let mem = [0xD0u8, 0x05];
        let instr = disassemble_spc700(&mem, 0x0300).unwrap();
        assert_eq!(instr.mnemonic, "BNE $0307");
        assert_eq!(instr.comment.as_deref(), Some("-> $0307"));
        assert_eq!(instr.len(), 2);
    }

    #[test]
    fn test_beq() {
        let mem = [0xF0u8, 0x00];
        let instr = disassemble_spc700(&mem, 0x0100).unwrap();
        assert_eq!(instr.mnemonic, "BEQ $0102");
        assert_eq!(instr.len(), 2);
    }

    #[test]
    fn test_bpl() {
        let mem = [0x10u8, 0x08];
        let instr = disassemble_spc700(&mem, 0x2000).unwrap();
        assert_eq!(instr.mnemonic, "BPL $200A");
        assert_eq!(instr.len(), 2);
    }

    #[test]
    fn test_bmi() {
        let mem = [0x30u8, 0xFC]; // -4: target = addr+2-4 = addr-2
        let instr = disassemble_spc700(&mem, 0x0010).unwrap();
        assert_eq!(instr.mnemonic, "BMI $000E");
        assert_eq!(instr.len(), 2);
    }

    // ── BBS / BBC (3-byte, with branch comment) ───────────────────────────────

    #[test]
    fn test_bbs_dp0() {
        // BBS $10.0, target: PC=0x0200, next=0x0203, target=0x020A (rel=0x07)
        let mem = [0x03u8, 0x10, 0x07];
        let instr = disassemble_spc700(&mem, 0x0200).unwrap();
        assert_eq!(instr.mnemonic, "BBS $10.0, $020A");
        assert_eq!(instr.comment.as_deref(), Some("-> $020A"));
        assert_eq!(instr.len(), 3);
    }

    #[test]
    fn test_bbc_dp7() {
        let mem = [0xF3u8, 0x20, 0xFA]; // rel = -6: 0x0300+3-6 = 0x02FD
        let instr = disassemble_spc700(&mem, 0x0300).unwrap();
        assert_eq!(instr.mnemonic, "BBC $20.7, $02FD");
        assert_eq!(instr.len(), 3);
    }

    // ── SET1 / CLR1 ───────────────────────────────────────────────────────────

    #[test]
    fn test_set1_bit0() {
        let mem = [0x02u8, 0x30];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "SET1 $30.0");
        assert_eq!(instr.len(), 2);
    }

    #[test]
    fn test_set1_bit7() {
        let mem = [0xE2u8, 0x10];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "SET1 $10.7");
        assert_eq!(instr.len(), 2);
    }

    #[test]
    fn test_clr1_bit3() {
        let mem = [0x72u8, 0x44];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "CLR1 $44.3");
        assert_eq!(instr.len(), 2);
    }

    // ── mem.bit instructions ─────────────────────────────────────────────────

    #[test]
    fn test_or1_c_membit() {
        // 0x0A: OR1 C, mem.bit
        // word = 0x4001 → bit = 0x4001 >> 13 = 2, addr = 0x4001 & 0x1FFF = 0x0001
        let mem = [0x0Au8, 0x01, 0x40];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "OR1 C, $0001.2");
        assert_eq!(instr.len(), 3);
    }

    #[test]
    fn test_and1_c_neg_membit() {
        // 0x6A: AND1 C, /mem.bit
        // word = 0x2008 → bit = 1, addr = 0x0008
        let mem = [0x6Au8, 0x08, 0x20];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "AND1 C, /$0008.1");
        assert_eq!(instr.len(), 3);
    }

    #[test]
    fn test_mov1_c_membit() {
        let mem = [0xAAu8, 0x00, 0x00];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "MOV1 C, $0000.0");
        assert_eq!(instr.len(), 3);
    }

    #[test]
    fn test_not1_membit() {
        let mem = [0xEAu8, 0xFF, 0xFF];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        // word=0xFFFF → bit=7, addr=0x1FFF
        assert_eq!(instr.mnemonic, "NOT1 $1FFF.7");
        assert_eq!(instr.len(), 3);
    }

    // ── CBNE / DBNZ ──────────────────────────────────────────────────────────

    #[test]
    fn test_cbne_dp_rel() {
        // 0x2E: CBNE dp, rel — bytes [op][dp][rel]
        // PC=0x0500, target=0x0500+3+0x02=0x0505
        let mem = [0x2Eu8, 0x10, 0x02];
        let instr = disassemble_spc700(&mem, 0x0500).unwrap();
        assert_eq!(instr.mnemonic, "CBNE $10, $0505");
        assert_eq!(instr.comment.as_deref(), Some("-> $0505"));
        assert_eq!(instr.len(), 3);
    }

    #[test]
    fn test_cbne_dp_x_rel() {
        // 0xDE: CBNE dp+X, rel
        let mem = [0xDEu8, 0x20, 0xFD]; // rel=-3: 0x1000+3-3=0x1000
        let instr = disassemble_spc700(&mem, 0x1000).unwrap();
        assert_eq!(instr.mnemonic, "CBNE $20+X, $1000");
        assert_eq!(instr.len(), 3);
    }

    #[test]
    fn test_dbnz_dp_rel() {
        // 0x6E: DBNZ dp, rel
        let mem = [0x6Eu8, 0x08, 0xFE]; // rel=-2: 0x0200+3-2=0x0201
        let instr = disassemble_spc700(&mem, 0x0200).unwrap();
        assert_eq!(instr.mnemonic, "DBNZ $08, $0201");
        assert_eq!(instr.len(), 3);
    }

    #[test]
    fn test_dbnz_y_rel() {
        // 0xFE: DBNZ Y, rel
        let mem = [0xFEu8, 0xF8]; // rel=-8: 0x0010+2-8=0x000A
        let instr = disassemble_spc700(&mem, 0x0010).unwrap();
        assert_eq!(instr.mnemonic, "DBNZ Y, $000A");
        assert_eq!(instr.len(), 2);
    }

    // ── CALL / JMP ───────────────────────────────────────────────────────────

    #[test]
    fn test_call_abs() {
        let mem = [0x3Fu8, 0x00, 0x80];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "CALL $8000");
        assert_eq!(instr.len(), 3);
    }

    #[test]
    fn test_jmp_abs() {
        let mem = [0x5Fu8, 0x34, 0x12];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "JMP $1234");
        assert_eq!(instr.len(), 3);
    }

    #[test]
    fn test_jmp_abs_ind_x() {
        let mem = [0x1Fu8, 0x00, 0x01];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "JMP [$0100+X]");
        assert_eq!(instr.len(), 3);
    }

    #[test]
    fn test_pcall() {
        let mem = [0x4Fu8, 0x7A];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "PCALL #$7A");
        assert_eq!(instr.len(), 2);
    }

    // ── push/pop ─────────────────────────────────────────────────────────────

    #[test]
    fn test_push_psw() {
        let mem = [0x0Du8];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "PUSH PSW");
    }

    #[test]
    fn test_pop_a() {
        let mem = [0xAEu8];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "POP A");
    }

    // ── 16-bit word operations ────────────────────────────────────────────────

    #[test]
    fn test_movw_ya_dp() {
        let mem = [0xBAu8, 0x40];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "MOVW YA, $40");
        assert_eq!(instr.len(), 2);
    }

    #[test]
    fn test_movw_dp_ya() {
        let mem = [0xDAu8, 0x40];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "MOVW $40, YA");
        assert_eq!(instr.len(), 2);
    }

    #[test]
    fn test_addw_ya_dp() {
        let mem = [0x7Au8, 0x10];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "ADDW YA, $10");
        assert_eq!(instr.len(), 2);
    }

    #[test]
    fn test_subw_ya_dp() {
        let mem = [0x9Au8, 0x10];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "SUBW YA, $10");
        assert_eq!(instr.len(), 2);
    }

    #[test]
    fn test_cmpw_ya_dp() {
        let mem = [0x5Au8, 0x20];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "CMPW YA, $20");
        assert_eq!(instr.len(), 2);
    }

    // ── multiply / divide ─────────────────────────────────────────────────────

    #[test]
    fn test_mul_ya() {
        let mem = [0xCFu8];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "MUL YA");
        assert_eq!(instr.len(), 1);
    }

    #[test]
    fn test_div_ya_x() {
        let mem = [0x9Eu8];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "DIV YA, X");
        assert_eq!(instr.len(), 1);
    }

    // ── flag / misc instructions ──────────────────────────────────────────────

    #[test]
    fn test_clrc() {
        let mem = [0x60u8];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "CLRC");
    }

    #[test]
    fn test_setc() {
        let mem = [0x80u8];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "SETC");
    }

    #[test]
    fn test_notc() {
        let mem = [0xEDu8];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "NOTC");
    }

    #[test]
    fn test_ei() {
        let mem = [0xA0u8];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "EI");
    }

    #[test]
    fn test_di() {
        let mem = [0xC0u8];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "DI");
    }

    #[test]
    fn test_clrp_setp() {
        let clrp = disassemble_spc700(&[0x20u8], 0x0000).unwrap();
        let setp = disassemble_spc700(&[0x40u8], 0x0000).unwrap();
        assert_eq!(clrp.mnemonic, "CLRP");
        assert_eq!(setp.mnemonic, "SETP");
    }

    #[test]
    fn test_clrv() {
        let mem = [0xE0u8];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "CLRV");
    }

    #[test]
    fn test_xcn_a() {
        let mem = [0x9Fu8];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "XCN A");
    }

    #[test]
    fn test_daa_das() {
        let daa = disassemble_spc700(&[0xDFu8], 0x0000).unwrap();
        let das = disassemble_spc700(&[0xBEu8], 0x0000).unwrap();
        assert_eq!(daa.mnemonic, "DAA A");
        assert_eq!(das.mnemonic, "DAS A");
    }

    // ── indexed addressing ────────────────────────────────────────────────────

    #[test]
    fn test_mov_a_dp_x() {
        let mem = [0xF4u8, 0x10];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "MOV A, $10+X");
        assert_eq!(instr.len(), 2);
    }

    #[test]
    fn test_mov_a_abs_y() {
        let mem = [0xF6u8, 0x00, 0x02];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "MOV A, $0200+Y");
        assert_eq!(instr.len(), 3);
    }

    #[test]
    fn test_mov_a_indirect_x() {
        let mem = [0xE6u8];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "MOV A, (X)");
        assert_eq!(instr.len(), 1);
    }

    #[test]
    fn test_mov_a_ind_dp_x() {
        let mem = [0xE7u8, 0x40];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "MOV A, [$40+X]");
        assert_eq!(instr.len(), 2);
    }

    #[test]
    fn test_mov_a_ind_dp_y() {
        let mem = [0xF7u8, 0x40];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "MOV A, [$40]+Y");
        assert_eq!(instr.len(), 2);
    }

    #[test]
    fn test_or_x_y_indirect() {
        let mem = [0x19u8];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "OR (X), (Y)");
        assert_eq!(instr.len(), 1);
    }

    // ── MOV (X)+, A  /  MOV A, (X)+ ─────────────────────────────────────────

    #[test]
    fn test_mov_ind_x_inc_a() {
        let mem = [0xAFu8];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "MOV (X)+, A");
    }

    #[test]
    fn test_mov_a_ind_x_inc() {
        let mem = [0xBFu8];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "MOV A, (X)+");
    }

    // ── register-to-register transfers ───────────────────────────────────────

    #[test]
    fn test_mov_x_a() {
        let mem = [0x5Du8];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "MOV X, A");
    }

    #[test]
    fn test_mov_a_x() {
        let mem = [0x7Du8];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "MOV A, X");
    }

    #[test]
    fn test_mov_a_y() {
        let mem = [0xDDu8];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "MOV A, Y");
    }

    #[test]
    fn test_mov_y_a() {
        let mem = [0xFDu8];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "MOV Y, A");
    }

    #[test]
    fn test_mov_x_sp() {
        let mem = [0x9Du8];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "MOV X, SP");
    }

    #[test]
    fn test_mov_sp_x() {
        let mem = [0xBDu8];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "MOV SP, X");
    }

    // ── MOV dp, dp ────────────────────────────────────────────────────────────

    #[test]
    fn test_mov_dp_dp() {
        // 0xFA: MOV dp, dp — [0xFA][src_dp][dst_dp]
        let mem = [0xFAu8, 0x10, 0x20];
        let instr = disassemble_spc700(&mem, 0x0000).unwrap();
        assert_eq!(instr.mnemonic, "MOV $20, $10");
        assert_eq!(instr.len(), 3);
    }

    // ── None on empty / short buffer ─────────────────────────────────────────

    #[test]
    fn test_none_on_empty() {
        assert!(disassemble_spc700(&[], 0x0000).is_none());
    }

    #[test]
    fn test_none_on_short_buffer() {
        // 0xE5 needs 3 bytes total; supply only 2
        let mem = [0xE5u8, 0x00];
        assert!(disassemble_spc700(&mem, 0x0000).is_none());
    }

    #[test]
    fn test_none_on_short_branch() {
        // BNE needs 2 bytes; supply only 1
        let mem = [0xD0u8];
        assert!(disassemble_spc700(&mem, 0x0000).is_none());
    }

    #[test]
    fn test_none_on_short_bbs() {
        // BBS dp.0,rel needs 3 bytes; supply only 2
        let mem = [0x03u8, 0x10];
        assert!(disassemble_spc700(&mem, 0x0000).is_none());
    }
}
