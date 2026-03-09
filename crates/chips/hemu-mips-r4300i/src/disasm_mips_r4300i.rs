//! MIPS R4300i CPU disassembler
//!
//! Full disassembly for the MIPS R4300i CPU used in Nintendo 64.
//! Extends the base MIPS I decoder from `hemu-mips-common` with
//! MIPS III 64-bit instructions, likely branches, COP1 (FPU), and cache ops.

use hemu_mips_common::disasm_mips::{self, Endian};
use hemu_mips_common::fields::Fields;
use hemu_mips_common::regs::REG_NAMES;
use hemu_types::DisassembledInstruction;

/// FPU register names
const FPR_NAMES: [&str; 32] = [
    "f0", "f1", "f2", "f3", "f4", "f5", "f6", "f7", "f8", "f9", "f10", "f11", "f12", "f13", "f14",
    "f15", "f16", "f17", "f18", "f19", "f20", "f21", "f22", "f23", "f24", "f25", "f26", "f27",
    "f28", "f29", "f30", "f31",
];

/// Disassemble a single R4300i instruction (big-endian, MIPS III).
pub fn disassemble_mips(memory: &[u8], address: u32) -> Option<DisassembledInstruction> {
    // Try the common MIPS I decoder first
    if let Some(instr) = disasm_mips::disassemble_mips_i(memory, address, Endian::Big) {
        return Some(instr);
    }

    // Fall through to R4300i / MIPS III specific opcodes
    let word = disasm_mips::read_word(memory, Endian::Big)?;
    let bytes = vec![memory[0], memory[1], memory[2], memory[3]];
    let f = Fields::from(word);

    let mnemonic = decode_mips_iii(&f, address);
    Some(DisassembledInstruction::new(address, bytes, mnemonic))
}

/// Decode MIPS III extended opcodes (64-bit, likely branches, FPU, cache).
fn decode_mips_iii(f: &Fields, address: u32) -> String {
    let r = REG_NAMES;

    match f.opcode {
        // Likely branches (MIPS II/III)
        0x14 => format!(
            "BEQL {}, {}, ${:08X}",
            r[f.rs],
            r[f.rt],
            f.branch_target(address)
        ),
        0x15 => format!(
            "BNEL {}, {}, ${:08X}",
            r[f.rs],
            r[f.rt],
            f.branch_target(address)
        ),
        0x16 => format!("BLEZL {}, ${:08X}", r[f.rs], f.branch_target(address)),
        0x17 => format!("BGTZL {}, ${:08X}", r[f.rs], f.branch_target(address)),

        // MIPS III 64-bit immediate ops
        0x18 => format!("DADDI {}, {}, {}", r[f.rt], r[f.rs], f.simm),
        0x19 => format!("DADDIU {}, {}, {}", r[f.rt], r[f.rs], f.simm),
        0x1A => format!("LDL {}, {}({})", r[f.rt], f.simm, r[f.rs]),
        0x1B => format!("LDR {}, {}({})", r[f.rt], f.simm, r[f.rs]),

        // 64-bit loads/stores
        0x27 => format!("LWU {}, {}({})", r[f.rt], f.simm, r[f.rs]),
        0x2C => format!("SDL {}, {}({})", r[f.rt], f.simm, r[f.rs]),
        0x2D => format!("SDR {}, {}({})", r[f.rt], f.simm, r[f.rs]),

        // Cache
        0x2F => format!("CACHE 0x{:02X}, {}({})", f.rt, f.simm, r[f.rs]),

        // Linked loads/stores
        0x30 => format!("LL {}, {}({})", r[f.rt], f.simm, r[f.rs]),
        0x34 => format!("LLD {}, {}({})", r[f.rt], f.simm, r[f.rs]),
        0x37 => format!("LD {}, {}({})", r[f.rt], f.simm, r[f.rs]),
        0x38 => format!("SC {}, {}({})", r[f.rt], f.simm, r[f.rs]),
        0x3C => format!("SCD {}, {}({})", r[f.rt], f.simm, r[f.rs]),
        0x3F => format!("SD {}, {}({})", r[f.rt], f.simm, r[f.rs]),

        // COP1 (FPU)
        0x11 => decode_cop1(f),

        // COP1 loads/stores
        0x31 => format!("LWC1 {}, {}({})", FPR_NAMES[f.rt], f.simm, r[f.rs]),
        0x35 => format!("LDC1 {}, {}({})", FPR_NAMES[f.rt], f.simm, r[f.rs]),
        0x39 => format!("SWC1 {}, {}({})", FPR_NAMES[f.rt], f.simm, r[f.rs]),
        0x3D => format!("SDC1 {}, {}({})", FPR_NAMES[f.rt], f.simm, r[f.rs]),

        _ => format!(".word 0x{:08X}", f.word),
    }
}

/// Decode COP1 (FPU) instructions.
fn decode_cop1(f: &Fields) -> String {
    let r = REG_NAMES;
    let fp = FPR_NAMES;
    let fd = (f.word >> 6) & 0x1F;
    let fs = f.rd;
    let ft = f.rt;

    match f.rs {
        0x00 => format!("MFC1 {}, {}", r[ft], fp[fs]),
        0x01 => format!("DMFC1 {}, {}", r[ft], fp[fs]),
        0x02 => format!("CFC1 {}, ${}", r[ft], fs),
        0x04 => format!("MTC1 {}, {}", r[ft], fp[fs]),
        0x05 => format!("DMTC1 {}, {}", r[ft], fp[fs]),
        0x06 => format!("CTC1 {}, ${}", r[ft], fs),
        // BC1F / BC1T
        0x08 => {
            let tgt = f.branch_target(f.word & !0xFFFF | (f.imm16 as u32));
            match ft {
                0x00 => format!("BC1F ${:08X}", tgt),
                0x01 => format!("BC1T ${:08X}", tgt),
                0x02 => format!("BC1FL ${:08X}", tgt),
                0x03 => format!("BC1TL ${:08X}", tgt),
                _ => format!("COP1 ${:08X}", f.word),
            }
        }
        // Single-precision (fmt=0x10)
        0x10 => decode_cop1_fmt(f, "S", fd as usize, fs, ft),
        // Double-precision (fmt=0x11)
        0x11 => decode_cop1_fmt(f, "D", fd as usize, fs, ft),
        // Word (fmt=0x14) — conversion from integer
        0x14 => match f.funct {
            0x20 => format!("CVT.S.W {}, {}", fp[fd as usize], fp[fs]),
            0x21 => format!("CVT.D.W {}, {}", fp[fd as usize], fp[fs]),
            _ => format!("COP1 ${:08X}", f.word),
        },
        // Long (fmt=0x15) — conversion from 64-bit integer
        0x15 => match f.funct {
            0x20 => format!("CVT.S.L {}, {}", fp[fd as usize], fp[fs]),
            0x21 => format!("CVT.D.L {}, {}", fp[fd as usize], fp[fs]),
            _ => format!("COP1 ${:08X}", f.word),
        },
        _ => format!("COP1 ${:08X}", f.word),
    }
}

/// Decode COP1 arithmetic format (S/D).
fn decode_cop1_fmt(f: &Fields, fmt: &str, fd: usize, fs: usize, ft: usize) -> String {
    let fp = FPR_NAMES;
    match f.funct {
        0x00 => format!("ADD.{} {}, {}, {}", fmt, fp[fd], fp[fs], fp[ft]),
        0x01 => format!("SUB.{} {}, {}, {}", fmt, fp[fd], fp[fs], fp[ft]),
        0x02 => format!("MUL.{} {}, {}, {}", fmt, fp[fd], fp[fs], fp[ft]),
        0x03 => format!("DIV.{} {}, {}, {}", fmt, fp[fd], fp[fs], fp[ft]),
        0x04 => format!("SQRT.{} {}, {}", fmt, fp[fd], fp[fs]),
        0x05 => format!("ABS.{} {}, {}", fmt, fp[fd], fp[fs]),
        0x06 => format!("MOV.{} {}, {}", fmt, fp[fd], fp[fs]),
        0x07 => format!("NEG.{} {}, {}", fmt, fp[fd], fp[fs]),
        0x08 => format!("ROUND.L.{} {}, {}", fmt, fp[fd], fp[fs]),
        0x09 => format!("TRUNC.L.{} {}, {}", fmt, fp[fd], fp[fs]),
        0x0A => format!("CEIL.L.{} {}, {}", fmt, fp[fd], fp[fs]),
        0x0B => format!("FLOOR.L.{} {}, {}", fmt, fp[fd], fp[fs]),
        0x0C => format!("ROUND.W.{} {}, {}", fmt, fp[fd], fp[fs]),
        0x0D => format!("TRUNC.W.{} {}, {}", fmt, fp[fd], fp[fs]),
        0x0E => format!("CEIL.W.{} {}, {}", fmt, fp[fd], fp[fs]),
        0x0F => format!("FLOOR.W.{} {}, {}", fmt, fp[fd], fp[fs]),
        0x20 => format!("CVT.S.{} {}, {}", fmt, fp[fd], fp[fs]),
        0x21 => format!("CVT.D.{} {}, {}", fmt, fp[fd], fp[fs]),
        0x24 => format!("CVT.W.{} {}, {}", fmt, fp[fd], fp[fs]),
        0x25 => format!("CVT.L.{} {}, {}", fmt, fp[fd], fp[fs]),
        c @ 0x30..=0x3F => {
            let cond_name = match c & 0x0F {
                0x00 => "F",
                0x01 => "UN",
                0x02 => "EQ",
                0x03 => "UEQ",
                0x04 => "OLT",
                0x05 => "ULT",
                0x06 => "OLE",
                0x07 => "ULE",
                0x08 => "SF",
                0x09 => "NGLE",
                0x0A => "SEQ",
                0x0B => "NGL",
                0x0C => "LT",
                0x0D => "NGE",
                0x0E => "LE",
                0x0F => "NGT",
                _ => "???",
            };
            format!("C.{}.{} {}, {}", cond_name, fmt, fp[fs], fp[ft])
        }
        _ => format!("COP1.{} ${:02X}", fmt, f.funct),
    }
}

#[cfg(test)]
#[allow(clippy::identity_op)]
mod tests {
    use super::*;

    fn encode_be(word: u32) -> [u8; 4] {
        word.to_be_bytes()
    }

    fn dis(word: u32) -> String {
        let mem = encode_be(word);
        disassemble_mips(&mem, 0x80000000).unwrap().mnemonic
    }

    // --- Base MIPS I (delegated to common) ---

    #[test]
    fn test_nop() {
        assert_eq!(dis(0x0000_0000), "NOP");
    }

    #[test]
    fn test_lui() {
        let word = (0x0Fu32 << 26) | (8 << 16) | 0x1234;
        assert_eq!(dis(word), "LUI t0, 0x1234");
    }

    #[test]
    fn test_j() {
        let word = (2u32 << 26) | ((0x80001000u32 >> 2) & 0x03FF_FFFF);
        assert_eq!(dis(word), "J $80001000");
    }

    #[test]
    fn test_lw() {
        let word = (0x23u32 << 26) | (29 << 21) | (8 << 16) | 0x0010;
        assert_eq!(dis(word), "LW t0, 16(sp)");
    }

    // --- MIPS III 64-bit ---

    #[test]
    fn test_daddiu() {
        let word = (0x19u32 << 26) | (8 << 21) | (9 << 16) | 100;
        assert_eq!(dis(word), "DADDIU t1, t0, 100");
    }

    #[test]
    fn test_ld() {
        let word = (0x37u32 << 26) | (29 << 21) | (8 << 16) | 0x0008;
        assert_eq!(dis(word), "LD t0, 8(sp)");
    }

    #[test]
    fn test_sd() {
        let word = (0x3Fu32 << 26) | (29 << 21) | (31 << 16) | 0xFFF8;
        assert_eq!(dis(word), "SD ra, -8(sp)");
    }

    #[test]
    fn test_lwu() {
        let word = (0x27u32 << 26) | (29 << 21) | (8 << 16) | 0x0004;
        assert_eq!(dis(word), "LWU t0, 4(sp)");
    }

    // --- Likely branches ---

    #[test]
    fn test_beql() {
        let word = (0x14u32 << 26) | (8 << 21) | (9 << 16) | 0x0004;
        let m = dis(word);
        assert!(m.starts_with("BEQL t0, t1"));
    }

    #[test]
    fn test_bnel() {
        let word = (0x15u32 << 26) | (8 << 21) | (0 << 16) | 0x0002;
        let m = dis(word);
        assert!(m.starts_with("BNEL t0, zero"));
    }

    // --- COP1 (FPU) ---

    #[test]
    fn test_add_s() {
        // ADD.S f4, f2, f0 => COP1 fmt=S(0x10), funct=0x00
        let word = (0x11u32 << 26) | (0x10 << 21) | (0 << 16) | (2 << 11) | (4 << 6) | 0x00;
        assert_eq!(dis(word), "ADD.S f4, f2, f0");
    }

    #[test]
    fn test_mul_d() {
        // MUL.D f6, f4, f2 => COP1 fmt=D(0x11), funct=0x02
        let word = (0x11u32 << 26) | (0x11 << 21) | (2 << 16) | (4 << 11) | (6 << 6) | 0x02;
        assert_eq!(dis(word), "MUL.D f6, f4, f2");
    }

    #[test]
    fn test_mfc1() {
        let word = (0x11u32 << 26) | (0x00 << 21) | (8 << 16) | (4 << 11);
        assert_eq!(dis(word), "MFC1 t0, f4");
    }

    #[test]
    fn test_mtc1() {
        let word = (0x11u32 << 26) | (0x04 << 21) | (8 << 16) | (4 << 11);
        assert_eq!(dis(word), "MTC1 t0, f4");
    }

    #[test]
    fn test_cvt_s_w() {
        let word = (0x11u32 << 26) | (0x14 << 21) | (0 << 16) | (2 << 11) | (4 << 6) | 0x20;
        assert_eq!(dis(word), "CVT.S.W f4, f2");
    }

    #[test]
    fn test_c_lt_s() {
        // C.LT.S f2, f4
        let word = (0x11u32 << 26) | (0x10 << 21) | (4 << 16) | (2 << 11) | (0 << 6) | 0x3C;
        assert_eq!(dis(word), "C.LT.S f2, f4");
    }

    #[test]
    fn test_lwc1() {
        let word = (0x31u32 << 26) | (29 << 21) | (4 << 16) | 0x0010;
        assert_eq!(dis(word), "LWC1 f4, 16(sp)");
    }

    #[test]
    fn test_swc1() {
        let word = (0x39u32 << 26) | (29 << 21) | (4 << 16) | 0x0010;
        assert_eq!(dis(word), "SWC1 f4, 16(sp)");
    }

    // --- Cache ---

    #[test]
    fn test_cache() {
        let word = (0x2Fu32 << 26) | (29 << 21) | (0x10 << 16) | 0x0010;
        assert_eq!(dis(word), "CACHE 0x10, 16(sp)");
    }

    // --- Edge cases ---

    #[test]
    fn test_short_memory() {
        let mem = [0x00, 0x00];
        assert!(disassemble_mips(&mem, 0x80000000).is_none());
    }

    #[test]
    fn test_instruction_length() {
        let mem = encode_be(0x0000_0000);
        let instr = disassemble_mips(&mem, 0x80000000).unwrap();
        assert_eq!(instr.len(), 4);
        assert_eq!(instr.address, 0x80000000);
    }
}
