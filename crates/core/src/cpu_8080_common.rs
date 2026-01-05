//! Common 8080-family CPU helper functions
//!
//! This module contains helper functions that are shared across the 8080, Z80, and LR35902 CPUs.
//! These functions are identical across all three implementations and are extracted here to reduce
//! code duplication.
//!
//! The 8080, Z80, and LR35902 (Game Boy CPU) all share:
//! - Similar register organization (A, B, C, D, E, H, L, SP, PC)
//! - Little-endian 16-bit operations
//! - Stack operations
//! - Register pair accessors

/// Read a 16-bit value from PC and advance PC by 2
///
/// This is a common pattern across all 8080-family CPUs:
/// - Read low byte from PC, increment PC
/// - Read high byte from PC, increment PC
/// - Combine into little-endian 16-bit value
#[inline]
pub fn read_pc_u16_le<F>(mut read_pc: F) -> u16
where
    F: FnMut() -> u8,
{
    let lo = read_pc() as u16;
    let hi = read_pc() as u16;
    (hi << 8) | lo
}

/// Push a 16-bit value onto the stack (little-endian)
///
/// Stack grows downward in memory:
/// - Decrement SP, write high byte
/// - Decrement SP, write low byte
#[inline]
pub fn push_u16_le<F>(sp: &mut u16, val: u16, mut write: F)
where
    F: FnMut(u16, u8),
{
    *sp = sp.wrapping_sub(1);
    write(*sp, (val >> 8) as u8);
    *sp = sp.wrapping_sub(1);
    write(*sp, val as u8);
}

/// Pop a 16-bit value from the stack (little-endian)
///
/// Stack grows downward in memory:
/// - Read low byte from SP, increment SP
/// - Read high byte from SP, increment SP
/// - Combine into little-endian 16-bit value
#[inline]
pub fn pop_u16_le<F>(sp: &mut u16, mut read: F) -> u16
where
    F: FnMut(u16) -> u8,
{
    let lo = read(*sp) as u16;
    *sp = sp.wrapping_add(1);
    let hi = read(*sp) as u16;
    *sp = sp.wrapping_add(1);
    (hi << 8) | lo
}

/// Get 16-bit register pair from two 8-bit registers (big-endian)
///
/// Used for BC, DE, HL register pairs where the first register is the high byte
#[inline]
pub fn get_reg_pair(high: u8, low: u8) -> u16 {
    ((high as u16) << 8) | (low as u16)
}

/// Set 16-bit register pair from a 16-bit value (big-endian)
///
/// Used for BC, DE, HL register pairs where the first register is the high byte
/// Returns (high_byte, low_byte)
#[inline]
pub fn set_reg_pair(val: u16) -> (u8, u8) {
    ((val >> 8) as u8, val as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_pc_u16_le() {
        let data = [0x34, 0x12]; // Little-endian 0x1234
        let mut idx = 0;
        let result = read_pc_u16_le(|| {
            let val = data[idx];
            idx += 1;
            val
        });
        assert_eq!(result, 0x1234);
    }

    #[test]
    fn test_push_pop_u16_le() {
        let mut memory = vec![0u8; 256];
        let mut sp = 0x100u16;

        // Push 0x1234
        push_u16_le(&mut sp, 0x1234, |addr, val| {
            memory[addr as usize] = val;
        });

        assert_eq!(sp, 0xFE); // SP decremented by 2
        assert_eq!(memory[0xFE], 0x34); // Low byte
        assert_eq!(memory[0xFF], 0x12); // High byte

        // Pop it back
        let result = pop_u16_le(&mut sp, |addr| memory[addr as usize]);

        assert_eq!(result, 0x1234);
        assert_eq!(sp, 0x100); // SP back to original
    }

    #[test]
    fn test_get_set_reg_pair() {
        // Test get
        assert_eq!(get_reg_pair(0x12, 0x34), 0x1234);
        assert_eq!(get_reg_pair(0xFF, 0x00), 0xFF00);
        assert_eq!(get_reg_pair(0x00, 0xFF), 0x00FF);

        // Test set
        assert_eq!(set_reg_pair(0x1234), (0x12, 0x34));
        assert_eq!(set_reg_pair(0xFF00), (0xFF, 0x00));
        assert_eq!(set_reg_pair(0x00FF), (0x00, 0xFF));

        // Test roundtrip
        let (h, l) = set_reg_pair(0xABCD);
        assert_eq!(get_reg_pair(h, l), 0xABCD);
    }

    #[test]
    fn test_push_pop_wrapping() {
        let mut memory = vec![0u8; 0x10000];
        let mut sp = 0x0001u16; // Near bottom of stack

        // Push should wrap around
        push_u16_le(&mut sp, 0xABCD, |addr, val| {
            memory[addr as usize] = val;
        });

        assert_eq!(sp, 0xFFFF); // Wrapped around
        assert_eq!(memory[0xFFFF], 0xCD); // Low byte
        assert_eq!(memory[0x0000], 0xAB); // High byte (wrapped)

        // Pop should wrap around too
        let result = pop_u16_le(&mut sp, |addr| memory[addr as usize]);
        assert_eq!(result, 0xABCD);
        assert_eq!(sp, 0x0001); // Back to original
    }
}
