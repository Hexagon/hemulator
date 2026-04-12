//! MIPS R4300i CPU disassembler
//!
//! Provides full disassembly for the MIPS R4300i CPU used in the Nintendo 64.
//!
//! All MIPS instructions are exactly 4 bytes (32-bit), big-endian. Three encoding
//! formats are used:
//!
//! - **R-type** (opcode 0): `op(6) | rs(5) | rt(5) | rd(5) | sa(5) | func(6)`
//! - **I-type**: `op(6) | rs(5) | rt(5) | imm16(16)`
//! - **J-type**: `op(6) | target26(26)`
//!
//! References:
//! - IDT MIPS R4300i Datasheet
//! - MIPS III ISA Specification

use crate::debug::DisassembledInstruction;

// Register names (ABI names)
const REG_NAMES: [&str; 32] = [
    "$zero", "$at", "$v0", "$v1", "$a0", "$a1", "$a2", "$a3", "$t0", "$t1", "$t2", "$t3", "$t4",
    "$t5", "$t6", "$t7", "$s0", "$s1", "$s2", "$s3", "$s4", "$s5", "$s6", "$s7", "$t8", "$t9",
    "$k0", "$k1", "$gp", "$sp", "$fp", "$ra",
];

// CP0 register names
const CP0_NAMES: [&str; 32] = [
    "Index",
    "Random",
    "EntryLo0",
    "EntryLo1",
    "Context",
    "PageMask",
    "Wired",
    "C7",
    "BadVAddr",
    "Count",
    "EntryHi",
    "Compare",
    "Status",
    "Cause",
    "EPC",
    "PRId",
    "Config",
    "LLAddr",
    "WatchLo",
    "WatchHi",
    "XContext",
    "C21",
    "C22",
    "C23",
    "C24",
    "C25",
    "ParityError",
    "CacheError",
    "TagLo",
    "TagHi",
    "ErrorEPC",
    "C31",
];

#[inline]
fn reg(r: u32) -> &'static str {
    REG_NAMES[(r & 0x1F) as usize]
}

#[inline]
fn cp0reg(r: u32) -> &'static str {
    CP0_NAMES[(r & 0x1F) as usize]
}

/// Sign-extend a 16-bit immediate to i32 for display.
#[inline]
fn imm16_signed(imm: u32) -> i32 {
    (imm as i16) as i32
}

/// Disassemble a single MIPS R4300i instruction from a byte slice.
///
/// Returns `None` if fewer than 4 bytes are available.
pub fn disassemble_mips(memory: &[u8], address: u32) -> Option<DisassembledInstruction> {
    if memory.len() < 4 {
        return None;
    }

    let bytes = vec![memory[0], memory[1], memory[2], memory[3]];
    let instr = u32::from_be_bytes([memory[0], memory[1], memory[2], memory[3]]);

    let mnemonic = decode_instruction(instr, address);
    Some(DisassembledInstruction::new(address, bytes, mnemonic))
}

fn decode_instruction(instr: u32, pc: u32) -> String {
    let opcode = (instr >> 26) & 0x3F;
    let rs = (instr >> 21) & 0x1F;
    let rt = (instr >> 16) & 0x1F;
    let rd = (instr >> 11) & 0x1F;
    let sa = (instr >> 6) & 0x1F;
    let func = instr & 0x3F;
    let imm = instr & 0xFFFF;
    let target = instr & 0x03FF_FFFF;

    match opcode {
        0x00 => decode_special(rs, rt, rd, sa, func),
        0x01 => decode_regimm(rs, rt, imm, pc),
        0x02 => {
            let addr = ((pc.wrapping_add(4)) & 0xF000_0000) | (target << 2);
            format!("J        0x{:08X}", addr)
        }
        0x03 => {
            let addr = ((pc.wrapping_add(4)) & 0xF000_0000) | (target << 2);
            format!("JAL      0x{:08X}", addr)
        }
        0x04 => format!(
            "BEQ      {}, {}, {}",
            reg(rs),
            reg(rt),
            branch_target(pc, imm)
        ),
        0x05 => format!(
            "BNE      {}, {}, {}",
            reg(rs),
            reg(rt),
            branch_target(pc, imm)
        ),
        0x06 => format!("BLEZ     {}, {}", reg(rs), branch_target(pc, imm)),
        0x07 => format!("BGTZ     {}, {}", reg(rs), branch_target(pc, imm)),
        0x08 => format!("ADDI     {}, {}, {}", reg(rt), reg(rs), imm16_signed(imm)),
        0x09 => format!("ADDIU    {}, {}, {}", reg(rt), reg(rs), imm16_signed(imm)),
        0x0A => format!("SLTI     {}, {}, {}", reg(rt), reg(rs), imm16_signed(imm)),
        0x0B => format!("SLTIU    {}, {}, {}", reg(rt), reg(rs), imm16_signed(imm)),
        0x0C => format!("ANDI     {}, {}, 0x{:04X}", reg(rt), reg(rs), imm),
        0x0D => format!("ORI      {}, {}, 0x{:04X}", reg(rt), reg(rs), imm),
        0x0E => format!("XORI     {}, {}, 0x{:04X}", reg(rt), reg(rs), imm),
        0x0F => format!("LUI      {}, 0x{:04X}", reg(rt), imm),
        0x10 => decode_cop0(rs, rt, rd, func, instr),
        0x11 => decode_cop1(rs, rt, rd, sa, func, instr, pc),
        0x14 => format!(
            "BEQL     {}, {}, {}",
            reg(rs),
            reg(rt),
            branch_target(pc, imm)
        ),
        0x15 => format!(
            "BNEL     {}, {}, {}",
            reg(rs),
            reg(rt),
            branch_target(pc, imm)
        ),
        0x16 => format!("BLEZL    {}, {}", reg(rs), branch_target(pc, imm)),
        0x17 => format!("BGTZL    {}, {}", reg(rs), branch_target(pc, imm)),
        0x18 => format!("DADDI    {}, {}, {}", reg(rt), reg(rs), imm16_signed(imm)),
        0x19 => format!("DADDIU   {}, {}, {}", reg(rt), reg(rs), imm16_signed(imm)),
        0x1A => format!("LDL      {}, {}({})", reg(rt), imm16_signed(imm), reg(rs)),
        0x1B => format!("LDR      {}, {}({})", reg(rt), imm16_signed(imm), reg(rs)),
        0x20 => format!("LB       {}, {}({})", reg(rt), imm16_signed(imm), reg(rs)),
        0x21 => format!("LH       {}, {}({})", reg(rt), imm16_signed(imm), reg(rs)),
        0x22 => format!("LWL      {}, {}({})", reg(rt), imm16_signed(imm), reg(rs)),
        0x23 => format!("LW       {}, {}({})", reg(rt), imm16_signed(imm), reg(rs)),
        0x24 => format!("LBU      {}, {}({})", reg(rt), imm16_signed(imm), reg(rs)),
        0x25 => format!("LHU      {}, {}({})", reg(rt), imm16_signed(imm), reg(rs)),
        0x26 => format!("LWR      {}, {}({})", reg(rt), imm16_signed(imm), reg(rs)),
        0x27 => format!("LWU      {}, {}({})", reg(rt), imm16_signed(imm), reg(rs)),
        0x28 => format!("SB       {}, {}({})", reg(rt), imm16_signed(imm), reg(rs)),
        0x29 => format!("SH       {}, {}({})", reg(rt), imm16_signed(imm), reg(rs)),
        0x2A => format!("SWL      {}, {}({})", reg(rt), imm16_signed(imm), reg(rs)),
        0x2B => format!("SW       {}, {}({})", reg(rt), imm16_signed(imm), reg(rs)),
        0x2C => format!("SDL      {}, {}({})", reg(rt), imm16_signed(imm), reg(rs)),
        0x2D => format!("SDR      {}, {}({})", reg(rt), imm16_signed(imm), reg(rs)),
        0x2E => format!("SWR      {}, {}({})", reg(rt), imm16_signed(imm), reg(rs)),
        0x2F => format!("CACHE    0x{:02X}, {}({})", rt, imm16_signed(imm), reg(rs)),
        0x30 => format!("LL       {}, {}({})", reg(rt), imm16_signed(imm), reg(rs)),
        0x31 => format!("LWC1     $f{}, {}({})", rt, imm16_signed(imm), reg(rs)),
        0x35 => format!("LDC1     $f{}, {}({})", rt, imm16_signed(imm), reg(rs)),
        0x37 => format!("LD       {}, {}({})", reg(rt), imm16_signed(imm), reg(rs)),
        0x38 => format!("SC       {}, {}({})", reg(rt), imm16_signed(imm), reg(rs)),
        0x39 => format!("SWC1     $f{}, {}({})", rt, imm16_signed(imm), reg(rs)),
        0x3D => format!("SDC1     $f{}, {}({})", rt, imm16_signed(imm), reg(rs)),
        0x3F => format!("SD       {}, {}({})", reg(rt), imm16_signed(imm), reg(rs)),
        _ => format!(".word    0x{:08X}", instr),
    }
}

fn decode_special(rs: u32, rt: u32, rd: u32, sa: u32, func: u32) -> String {
    match func {
        0x00 => {
            if rs == 0 && rt == 0 && rd == 0 && sa == 0 {
                "NOP".to_string()
            } else {
                format!("SLL      {}, {}, {}", reg(rd), reg(rt), sa)
            }
        }
        0x02 => format!("SRL      {}, {}, {}", reg(rd), reg(rt), sa),
        0x03 => format!("SRA      {}, {}, {}", reg(rd), reg(rt), sa),
        0x04 => format!("SLLV     {}, {}, {}", reg(rd), reg(rt), reg(rs)),
        0x06 => format!("SRLV     {}, {}, {}", reg(rd), reg(rt), reg(rs)),
        0x07 => format!("SRAV     {}, {}, {}", reg(rd), reg(rt), reg(rs)),
        0x08 => format!("JR       {}", reg(rs)),
        0x09 => {
            if rd == 31 {
                format!("JALR     {}", reg(rs))
            } else {
                format!("JALR     {}, {}", reg(rd), reg(rs))
            }
        }
        0x0C => "SYSCALL".to_string(),
        0x0D => "BREAK".to_string(),
        0x0F => "SYNC".to_string(),
        0x10 => format!("MFHI     {}", reg(rd)),
        0x11 => format!("MTHI     {}", reg(rs)),
        0x12 => format!("MFLO     {}", reg(rd)),
        0x13 => format!("MTLO     {}", reg(rs)),
        0x14 => format!("DSLLV    {}, {}, {}", reg(rd), reg(rt), reg(rs)),
        0x16 => format!("DSRLV    {}, {}, {}", reg(rd), reg(rt), reg(rs)),
        0x17 => format!("DSRAV    {}, {}, {}", reg(rd), reg(rt), reg(rs)),
        0x18 => format!("MULT     {}, {}", reg(rs), reg(rt)),
        0x19 => format!("MULTU    {}, {}", reg(rs), reg(rt)),
        0x1A => format!("DIV      {}, {}", reg(rs), reg(rt)),
        0x1B => format!("DIVU     {}, {}", reg(rs), reg(rt)),
        0x1C => format!("DMULT    {}, {}", reg(rs), reg(rt)),
        0x1D => format!("DMULTU   {}, {}", reg(rs), reg(rt)),
        0x1E => format!("DDIV     {}, {}", reg(rs), reg(rt)),
        0x1F => format!("DDIVU    {}, {}", reg(rs), reg(rt)),
        0x20 => format!("ADD      {}, {}, {}", reg(rd), reg(rs), reg(rt)),
        0x21 => format!("ADDU     {}, {}, {}", reg(rd), reg(rs), reg(rt)),
        0x22 => format!("SUB      {}, {}, {}", reg(rd), reg(rs), reg(rt)),
        0x23 => format!("SUBU     {}, {}, {}", reg(rd), reg(rs), reg(rt)),
        0x24 => format!("AND      {}, {}, {}", reg(rd), reg(rs), reg(rt)),
        0x25 => format!("OR       {}, {}, {}", reg(rd), reg(rs), reg(rt)),
        0x26 => format!("XOR      {}, {}, {}", reg(rd), reg(rs), reg(rt)),
        0x27 => format!("NOR      {}, {}, {}", reg(rd), reg(rs), reg(rt)),
        0x2A => format!("SLT      {}, {}, {}", reg(rd), reg(rs), reg(rt)),
        0x2B => format!("SLTU     {}, {}, {}", reg(rd), reg(rs), reg(rt)),
        0x2C => format!("DADD     {}, {}, {}", reg(rd), reg(rs), reg(rt)),
        0x2D => format!("DADDU    {}, {}, {}", reg(rd), reg(rs), reg(rt)),
        0x2E => format!("DSUB     {}, {}, {}", reg(rd), reg(rs), reg(rt)),
        0x2F => format!("DSUBU    {}, {}, {}", reg(rd), reg(rs), reg(rt)),
        0x30 => format!("TGE      {}, {}", reg(rs), reg(rt)),
        0x31 => format!("TGEU     {}, {}", reg(rs), reg(rt)),
        0x32 => format!("TLT      {}, {}", reg(rs), reg(rt)),
        0x33 => format!("TLTU     {}, {}", reg(rs), reg(rt)),
        0x34 => format!("TEQ      {}, {}", reg(rs), reg(rt)),
        0x36 => format!("TNE      {}, {}", reg(rs), reg(rt)),
        0x38 => format!("DSLL     {}, {}, {}", reg(rd), reg(rt), sa),
        0x3A => format!("DSRL     {}, {}, {}", reg(rd), reg(rt), sa),
        0x3B => format!("DSRA     {}, {}, {}", reg(rd), reg(rt), sa),
        0x3C => format!("DSLL32   {}, {}, {}", reg(rd), reg(rt), sa),
        0x3E => format!("DSRL32   {}, {}, {}", reg(rd), reg(rt), sa),
        0x3F => format!("DSRA32   {}, {}, {}", reg(rd), reg(rt), sa),
        _ => format!("SPECIAL  func=0x{:02X}", func),
    }
}

fn decode_regimm(rs: u32, rt: u32, imm: u32, pc: u32) -> String {
    match rt {
        0x00 => format!("BLTZ     {}, {}", reg(rs), branch_target(pc, imm)),
        0x01 => format!("BGEZ     {}, {}", reg(rs), branch_target(pc, imm)),
        0x02 => format!("BLTZL    {}, {}", reg(rs), branch_target(pc, imm)),
        0x03 => format!("BGEZL    {}, {}", reg(rs), branch_target(pc, imm)),
        0x08 => format!("TGEI     {}, {}", reg(rs), imm16_signed(imm)),
        0x09 => format!("TGEIU    {}, {}", reg(rs), imm16_signed(imm)),
        0x0A => format!("TLTI     {}, {}", reg(rs), imm16_signed(imm)),
        0x0B => format!("TLTIU    {}, {}", reg(rs), imm16_signed(imm)),
        0x0C => format!("TEQI     {}, {}", reg(rs), imm16_signed(imm)),
        0x0E => format!("TNEI     {}, {}", reg(rs), imm16_signed(imm)),
        0x10 => format!("BLTZAL   {}, {}", reg(rs), branch_target(pc, imm)),
        0x11 => format!("BGEZAL   {}, {}", reg(rs), branch_target(pc, imm)),
        0x12 => format!("BLTZALL  {}, {}", reg(rs), branch_target(pc, imm)),
        0x13 => format!("BGEZALL  {}, {}", reg(rs), branch_target(pc, imm)),
        _ => format!("REGIMM   rt=0x{:02X}", rt),
    }
}

fn decode_cop0(rs: u32, rt: u32, rd: u32, _func: u32, instr: u32) -> String {
    match rs {
        0x00 => format!("MFC0     {}, {}", reg(rt), cp0reg(rd)),
        0x01 => format!("DMFC0    {}, {}", reg(rt), cp0reg(rd)),
        0x04 => format!("MTC0     {}, {}", reg(rt), cp0reg(rd)),
        0x05 => format!("DMTC0    {}, {}", reg(rt), cp0reg(rd)),
        0x10 => {
            // CO instructions
            let co_func = instr & 0x3F;
            match co_func {
                0x01 => "TLBR".to_string(),
                0x02 => "TLBWI".to_string(),
                0x06 => "TLBWR".to_string(),
                0x08 => "TLBP".to_string(),
                0x18 => "ERET".to_string(),
                _ => format!("COP0.CO  func=0x{:02X}", co_func),
            }
        }
        _ => format!("COP0     rs=0x{:02X}", rs),
    }
}

fn decode_cop1(rs: u32, rt: u32, rd: u32, sa: u32, func: u32, instr: u32, pc: u32) -> String {
    let fmt = rs;
    let ft = rt;
    let fs = rd;
    let fd = sa;

    match fmt {
        0x00 => format!("MFC1     {}, $f{}", reg(rt), rd),
        0x01 => format!("DMFC1    {}, $f{}", reg(rt), rd),
        0x02 => format!("CFC1     {}, $f{}", reg(rt), rd),
        0x04 => format!("MTC1     {}, $f{}", reg(rt), rd),
        0x05 => format!("DMTC1    {}, $f{}", reg(rt), rd),
        0x06 => format!("CTC1     {}, $f{}", reg(rt), rd),
        0x08 => {
            // BC1
            let cc = (instr >> 18) & 0x07;
            let nd = (instr >> 17) & 0x01;
            let tf = (instr >> 16) & 0x01;
            let imm = instr & 0xFFFF;
            match (nd, tf) {
                (0, 0) => format!("BC1F     cc={}, {}", cc, branch_target(pc, imm)),
                (0, 1) => format!("BC1T     cc={}, {}", cc, branch_target(pc, imm)),
                (1, 0) => format!("BC1FL    cc={}, {}", cc, branch_target(pc, imm)),
                (1, 1) => format!("BC1TL    cc={}, {}", cc, branch_target(pc, imm)),
                _ => "BC1?".to_string(),
            }
        }
        0x10 => decode_cop1_fmt("S", fs, ft, fd, func),
        0x11 => decode_cop1_fmt("D", fs, ft, fd, func),
        0x14 => decode_cop1_fmt("W", fs, ft, fd, func),
        0x15 => decode_cop1_fmt("L", fs, ft, fd, func),
        _ => format!("COP1     fmt=0x{:02X}", fmt),
    }
}

fn decode_cop1_fmt(fmt: &str, fs: u32, ft: u32, fd: u32, func: u32) -> String {
    match func {
        0x00 => format!("ADD.{}    $f{}, $f{}, $f{}", fmt, fd, fs, ft),
        0x01 => format!("SUB.{}    $f{}, $f{}, $f{}", fmt, fd, fs, ft),
        0x02 => format!("MUL.{}    $f{}, $f{}, $f{}", fmt, fd, fs, ft),
        0x03 => format!("DIV.{}    $f{}, $f{}, $f{}", fmt, fd, fs, ft),
        0x04 => format!("SQRT.{}   $f{}, $f{}", fmt, fd, fs),
        0x05 => format!("ABS.{}    $f{}, $f{}", fmt, fd, fs),
        0x06 => format!("MOV.{}    $f{}, $f{}", fmt, fd, fs),
        0x07 => format!("NEG.{}    $f{}, $f{}", fmt, fd, fs),
        0x08 => format!("ROUND.L.{} $f{}, $f{}", fmt, fd, fs),
        0x09 => format!("TRUNC.L.{} $f{}, $f{}", fmt, fd, fs),
        0x0A => format!("CEIL.L.{} $f{}, $f{}", fmt, fd, fs),
        0x0B => format!("FLOOR.L.{} $f{}, $f{}", fmt, fd, fs),
        0x0C => format!("ROUND.W.{} $f{}, $f{}", fmt, fd, fs),
        0x0D => format!("TRUNC.W.{} $f{}, $f{}", fmt, fd, fs),
        0x0E => format!("CEIL.W.{} $f{}, $f{}", fmt, fd, fs),
        0x0F => format!("FLOOR.W.{} $f{}, $f{}", fmt, fd, fs),
        0x20 => format!("CVT.S.{} $f{}, $f{}", fmt, fd, fs),
        0x21 => format!("CVT.D.{} $f{}, $f{}", fmt, fd, fs),
        0x24 => format!("CVT.W.{} $f{}, $f{}", fmt, fd, fs),
        0x25 => format!("CVT.L.{} $f{}, $f{}", fmt, fd, fs),
        0x30 => format!("C.F.{}   $f{}, $f{}", fmt, fs, ft),
        0x31 => format!("C.UN.{}  $f{}, $f{}", fmt, fs, ft),
        0x32 => format!("C.EQ.{}  $f{}, $f{}", fmt, fs, ft),
        0x33 => format!("C.UEQ.{} $f{}, $f{}", fmt, fs, ft),
        0x34 => format!("C.OLT.{} $f{}, $f{}", fmt, fs, ft),
        0x35 => format!("C.ULT.{} $f{}, $f{}", fmt, fs, ft),
        0x36 => format!("C.OLE.{} $f{}, $f{}", fmt, fs, ft),
        0x37 => format!("C.ULE.{} $f{}, $f{}", fmt, fs, ft),
        0x38 => format!("C.SF.{}  $f{}, $f{}", fmt, fs, ft),
        0x39 => format!("C.NGLE.{} $f{}, $f{}", fmt, fs, ft),
        0x3A => format!("C.SEQ.{} $f{}, $f{}", fmt, fs, ft),
        0x3B => format!("C.NGL.{} $f{}, $f{}", fmt, fs, ft),
        0x3C => format!("C.LT.{}  $f{}, $f{}", fmt, fs, ft),
        0x3D => format!("C.NGE.{} $f{}, $f{}", fmt, fs, ft),
        0x3E => format!("C.LE.{}  $f{}, $f{}", fmt, fs, ft),
        0x3F => format!("C.NGT.{} $f{}, $f{}", fmt, fs, ft),
        _ => format!("FPU.{}   func=0x{:02X}", fmt, func),
    }
}

/// Compute the branch target address from the current PC and 16-bit offset.
///
/// Branch offset is a signed 16-bit value counted in instructions (× 4),
/// relative to the instruction following the branch (PC + 4).
fn branch_target(pc: u32, imm: u32) -> String {
    let offset = (imm as i16 as i32) * 4;
    let target = (pc as i64 + 4 + offset as i64) as u32;
    format!("0x{:08X}", target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nop() {
        // SLL $zero, $zero, 0 = NOP (all-zero instruction)
        let mem = [0x00u8, 0x00, 0x00, 0x00];
        let instr = disassemble_mips(&mem, 0x8000_0000).unwrap();
        assert_eq!(instr.address, 0x8000_0000);
        assert_eq!(instr.len(), 4);
        assert_eq!(instr.mnemonic, "NOP");
    }

    #[test]
    fn test_addiu() {
        // ADDIU $sp, $sp, -8 → 0x27BDFFF8
        let mem = [0x27u8, 0xBD, 0xFF, 0xF8];
        let instr = disassemble_mips(&mem, 0x8000_0000).unwrap();
        assert!(
            instr.mnemonic.contains("ADDIU"),
            "expected ADDIU, got: {}",
            instr.mnemonic
        );
        assert!(
            instr.mnemonic.contains("$sp"),
            "expected $sp, got: {}",
            instr.mnemonic
        );
        assert!(
            instr.mnemonic.contains("-8"),
            "expected -8, got: {}",
            instr.mnemonic
        );
    }

    #[test]
    fn test_lw() {
        // LW $t0, 4($sp) → 0x8FA80004
        let mem = [0x8Fu8, 0xA8, 0x00, 0x04];
        let instr = disassemble_mips(&mem, 0x8000_0000).unwrap();
        assert!(
            instr.mnemonic.starts_with("LW"),
            "expected LW, got: {}",
            instr.mnemonic
        );
    }

    #[test]
    fn test_jal() {
        // JAL 0x80000100 → target bits = (0x80000100 & 0x0FFFFFFF) >> 2 = 0x40
        // At PC=0x80000000: 0x0C000040
        let mem = [0x0Cu8, 0x00, 0x00, 0x40];
        let instr = disassemble_mips(&mem, 0x8000_0000).unwrap();
        assert!(
            instr.mnemonic.starts_with("JAL"),
            "expected JAL, got: {}",
            instr.mnemonic
        );
    }

    #[test]
    fn test_jr() {
        // JR $ra → 0x03E00008
        let mem = [0x03u8, 0xE0, 0x00, 0x08];
        let instr = disassemble_mips(&mem, 0x8000_0000).unwrap();
        assert_eq!(instr.mnemonic, "JR       $ra");
    }

    #[test]
    fn test_addu() {
        // ADDU $v0, $a0, $a1 → 0x00851021
        let mem = [0x00u8, 0x85, 0x10, 0x21];
        let instr = disassemble_mips(&mem, 0x8000_0000).unwrap();
        assert!(
            instr.mnemonic.starts_with("ADDU"),
            "expected ADDU, got: {}",
            instr.mnemonic
        );
    }

    #[test]
    fn test_mfc0() {
        // MFC0 $k0, Status (CP0 reg 12) → 0x401A6000
        let mem = [0x40u8, 0x1A, 0x60, 0x00];
        let instr = disassemble_mips(&mem, 0x8000_0000).unwrap();
        assert!(
            instr.mnemonic.contains("MFC0"),
            "expected MFC0, got: {}",
            instr.mnemonic
        );
    }

    #[test]
    fn test_short_memory() {
        let mem = [0x00u8, 0x00];
        assert!(disassemble_mips(&mem, 0x8000_0000).is_none());
    }

    #[test]
    fn test_branch_offset() {
        // BEQ $zero, $zero, +4 (offset=1, target = PC+4+(1*4) = PC+8)
        // At PC=0x80000000: target = 0x80000008
        // BEQ $0,$0,1 → 0x10000001
        let mem = [0x10u8, 0x00, 0x00, 0x01];
        let instr = disassemble_mips(&mem, 0x8000_0000).unwrap();
        assert!(
            instr.mnemonic.starts_with("BEQ"),
            "expected BEQ, got: {}",
            instr.mnemonic
        );
        assert!(
            instr.mnemonic.contains("0x80000008"),
            "expected target 0x80000008, got: {}",
            instr.mnemonic
        );
    }

    #[test]
    fn test_lui() {
        // LUI $v0, 0x8000 → 0x3C028000
        let mem = [0x3Cu8, 0x02, 0x80, 0x00];
        let instr = disassemble_mips(&mem, 0x8000_0000).unwrap();
        assert!(
            instr.mnemonic.starts_with("LUI"),
            "expected LUI, got: {}",
            instr.mnemonic
        );
        assert!(
            instr.mnemonic.contains("$v0"),
            "expected $v0, got: {}",
            instr.mnemonic
        );
    }

    #[test]
    fn test_ori() {
        // ORI $a0, $zero, 0x1234 → 0x34040000 | 0x1234 = 0x34041234
        // Actually: ORI rt=4, rs=0, imm=0x1234 → opcode=0x0D, rs=0, rt=4
        // 0x0D<<26 | 0<<21 | 4<<16 | 0x1234 = 0x34041234
        let mem = [0x34u8, 0x04, 0x12, 0x34];
        let instr = disassemble_mips(&mem, 0x8000_0000).unwrap();
        assert!(
            instr.mnemonic.starts_with("ORI"),
            "expected ORI, got: {}",
            instr.mnemonic
        );
    }

    #[test]
    fn test_sw() {
        // SW $ra, 4($sp) → 0xAFBF0004
        let mem = [0xAFu8, 0xBF, 0x00, 0x04];
        let instr = disassemble_mips(&mem, 0x8000_0000).unwrap();
        assert!(
            instr.mnemonic.starts_with("SW"),
            "expected SW, got: {}",
            instr.mnemonic
        );
        assert!(
            instr.mnemonic.contains("$ra"),
            "expected $ra, got: {}",
            instr.mnemonic
        );
    }

    #[test]
    fn test_sll() {
        // SLL $v0, $a0, 2 → 0x00041080
        let mem = [0x00u8, 0x04, 0x10, 0x80];
        let instr = disassemble_mips(&mem, 0x8000_0000).unwrap();
        assert!(
            instr.mnemonic.starts_with("SLL"),
            "expected SLL, got: {}",
            instr.mnemonic
        );
    }

    #[test]
    fn test_bc1f_uses_real_pc() {
        // BC1F cc=0, offset=+1 (target = PC+4 + 1*4 = PC+8)
        // BC1 encoding: opcode=0x11 (COP1), fmt=0x08 (BC1), rt=0 (nd=0, tf=0), imm=1
        // Bits: [31:26]=0x11, [25:21]=0x08, [20:16]=0x00, [15:0]=0x0001
        // = 0x4500_0001
        let pc = 0x8000_0200u32;
        let mem = [0x45u8, 0x00, 0x00, 0x01];
        let instr = disassemble_mips(&mem, pc).unwrap();
        assert!(
            instr.mnemonic.starts_with("BC1F"),
            "expected BC1F, got: {}",
            instr.mnemonic
        );
        // Target should be pc+4 + 1*4 = 0x80000200 + 4 + 4 = 0x80000208
        assert!(
            instr.mnemonic.contains("0x80000208"),
            "expected target 0x80000208, got: {}",
            instr.mnemonic
        );
    }

    #[test]
    fn test_bc1t_uses_real_pc() {
        // BC1T cc=0, offset=-2 (backward branch)
        // rt bits: nd=0, tf=1 → rt = 0b00001 = 1
        // imm = 0xFFFE (-2 in signed 16-bit)
        // Bits: [31:26]=0x11, [25:21]=0x08, [20:16]=0x01, [15:0]=0xFFFE
        // = 0x4501_FFFE
        let pc = 0x8000_0100u32;
        let mem = [0x45u8, 0x01, 0xFF, 0xFE];
        let instr = disassemble_mips(&mem, pc).unwrap();
        assert!(
            instr.mnemonic.starts_with("BC1T"),
            "expected BC1T, got: {}",
            instr.mnemonic
        );
        // Target = 0x80000100 + 4 + (-2)*4 = 0x80000100 + 4 - 8 = 0x800000FC
        assert!(
            instr.mnemonic.contains("0x800000FC"),
            "expected target 0x800000FC, got: {}",
            instr.mnemonic
        );
    }
}
