//! MIPS instruction field extraction.
//!
//! All MIPS variants share the same 32-bit instruction encoding with identical
//! field positions.  This module provides a single extraction struct that works
//! for MIPS I through MIPS IV.

/// Pre-extracted fields from a 32-bit MIPS instruction word.
#[derive(Debug, Clone, Copy)]
pub struct Fields {
    /// Raw 32-bit instruction word
    pub word: u32,
    /// Primary opcode (bits 31–26)
    pub opcode: u32,
    /// Source register index (bits 25–21)
    pub rs: usize,
    /// Target register index (bits 20–16)
    pub rt: usize,
    /// Destination register index (bits 15–11)
    pub rd: usize,
    /// Shift amount (bits 10–6)
    pub sa: u32,
    /// Function code (bits 5–0)
    pub funct: u32,
    /// 16-bit unsigned immediate (bits 15–0)
    pub imm16: u16,
    /// 16-bit signed immediate
    pub simm: i16,
    /// 26-bit jump target (bits 25–0)
    pub target: u32,
}

impl From<u32> for Fields {
    fn from(word: u32) -> Self {
        let imm16 = (word & 0xFFFF) as u16;
        Self {
            word,
            opcode: (word >> 26) & 0x3F,
            rs: ((word >> 21) & 0x1F) as usize,
            rt: ((word >> 16) & 0x1F) as usize,
            rd: ((word >> 11) & 0x1F) as usize,
            sa: (word >> 6) & 0x1F,
            funct: word & 0x3F,
            imm16,
            simm: imm16 as i16,
            target: word & 0x03FF_FFFF,
        }
    }
}

impl Fields {
    /// Compute a branch target address.
    ///
    /// `pc` is the address of the branch instruction itself.
    pub fn branch_target(&self, pc: u32) -> u32 {
        pc.wrapping_add(4)
            .wrapping_add((self.simm as i32 as u32) << 2)
    }

    /// Compute a jump target address in the current 256 MB region.
    pub fn jump_target(&self, pc: u32) -> u32 {
        (pc & 0xF000_0000) | (self.target << 2)
    }
}

#[cfg(test)]
#[allow(clippy::identity_op)]
mod tests {
    use super::*;

    #[test]
    fn test_nop() {
        let f = Fields::from(0u32);
        assert_eq!(f.opcode, 0);
        assert_eq!(f.rs, 0);
        assert_eq!(f.rt, 0);
        assert_eq!(f.rd, 0);
        assert_eq!(f.sa, 0);
        assert_eq!(f.funct, 0);
    }

    #[test]
    fn test_lui() {
        // LUI $t0, 0x8000 => opcode=0x0F, rt=8, imm=0x8000
        let word = (0x0Fu32 << 26) | (8 << 16) | 0x8000;
        let f = Fields::from(word);
        assert_eq!(f.opcode, 0x0F);
        assert_eq!(f.rt, 8);
        assert_eq!(f.imm16, 0x8000);
        assert_eq!(f.simm, -32768);
    }

    #[test]
    fn test_r_type() {
        // ADD $t2, $t0, $t1 => opcode=0, rs=8, rt=9, rd=10, funct=0x20
        let word = (8u32 << 21) | (9 << 16) | (10 << 11) | 0x20;
        let f = Fields::from(word);
        assert_eq!(f.opcode, 0);
        assert_eq!(f.rs, 8);
        assert_eq!(f.rt, 9);
        assert_eq!(f.rd, 10);
        assert_eq!(f.funct, 0x20);
    }

    #[test]
    fn test_branch_target() {
        // BEQ offset = +2 words from PC 0x80000000
        let word = (4u32 << 26) | 0x0002;
        let f = Fields::from(word);
        assert_eq!(f.branch_target(0x80000000), 0x8000000C);
    }

    #[test]
    fn test_jump_target() {
        let word = (2u32 << 26) | ((0x80001000u32 >> 2) & 0x03FF_FFFF);
        let f = Fields::from(word);
        assert_eq!(f.jump_target(0x80000000), 0x80001000);
    }

    #[test]
    fn test_negative_immediate() {
        // ADDIU $t0, $zero, -1
        let word = (9u32 << 26) | (0 << 21) | (8 << 16) | 0xFFFF;
        let f = Fields::from(word);
        assert_eq!(f.simm, -1);
        assert_eq!(f.imm16, 0xFFFF);
    }
}
