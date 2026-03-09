//! Standard MIPS register ABI names.
//!
//! The 32-register naming convention is shared across all MIPS variants
//! (R3000A, R4300i, R5900, …).

/// Standard MIPS register names (o32 ABI).
pub const REG_NAMES: [&str; 32] = [
    "zero", "at", "v0", "v1", "a0", "a1", "a2", "a3", "t0", "t1", "t2", "t3", "t4", "t5", "t6",
    "t7", "s0", "s1", "s2", "s3", "s4", "s5", "s6", "s7", "t8", "t9", "k0", "k1", "gp", "sp", "fp",
    "ra",
];

/// COP0 register names common to most MIPS implementations.
///
/// Specific variants may override entries (e.g. R4300i has `Config` at 16).
pub const COP0_NAMES: [&str; 32] = [
    "Index", "Random", "EntryLo0", "EntryLo1", "Context", "PageMask", "Wired", "cp0_7", "BadVAddr",
    "Count", "EntryHi", "Compare", "SR", "Cause", "EPC", "PRId", "Config", "LLAddr", "WatchLo",
    "WatchHi", "XContext", "cp0_21", "cp0_22", "cp0_23", "cp0_24", "cp0_25", "PErr", "CacheErr",
    "TagLo", "TagHi", "ErrorEPC", "cp0_31",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reg_name_count() {
        assert_eq!(REG_NAMES.len(), 32);
    }

    #[test]
    fn zero_register() {
        assert_eq!(REG_NAMES[0], "zero");
    }

    #[test]
    fn ra_register() {
        assert_eq!(REG_NAMES[31], "ra");
    }

    #[test]
    fn cop0_name_count() {
        assert_eq!(COP0_NAMES.len(), 32);
    }
}
