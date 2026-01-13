//! NES ROM Database
//!
//! This module provides a database of known NES ROMs with incorrect or missing header information.
//! The database allows overriding the mapper number and mirroring mode based on the ROM's CRC32 checksum.
//!
//! ## Use Cases
//!
//! - ROMs with corrupted iNES headers (e.g., DiskDude! corruption)
//! - ROMs with incorrect mapper assignments in the header
//! - ROMs with incorrect mirroring flags in the header
//! - Homebrew ROMs that don't follow iNES conventions
//!
//! ## Database Format
//!
//! Each entry in the database contains:
//! - CRC32 checksum of the ROM file (including header)
//! - Optional mapper number override
//! - Optional mirroring mode override
//! - Optional board name for documentation purposes
//!
//! ## Adding Entries
//!
//! To add a new ROM to the database:
//! 1. Calculate the CRC32 of the entire ROM file (including header)
//! 2. Determine the correct mapper and mirroring from hardware documentation
//! 3. Add an entry to the `ROM_DATABASE` array
//!
//! ## References
//!
//! - NESdev Wiki: https://www.nesdev.org/wiki/NES_2.0
//! - BootGod's Database: http://bootgod.dyndns.org:7777/
//! - NesCartDB: https://nescartdb.com/

use crate::cartridge::Mirroring;

/// ROM database entry containing override information for a specific ROM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RomDbEntry {
    /// CRC32 checksum of the entire ROM file (including iNES header)
    pub crc32: u32,
    /// Optional mapper number override (if None, use header value)
    pub mapper: Option<u8>,
    /// Optional mirroring mode override (if None, use header value)
    pub mirroring: Option<Mirroring>,
    /// Optional board name for documentation purposes
    pub board: Option<&'static str>,
}

impl RomDbEntry {
    /// Create a new database entry with all fields
    #[allow(dead_code)]
    pub const fn new(
        crc32: u32,
        mapper: Option<u8>,
        mirroring: Option<Mirroring>,
        board: Option<&'static str>,
    ) -> Self {
        Self {
            crc32,
            mapper,
            mirroring,
            board,
        }
    }
}

/// ROM database containing known ROMs with override information.
///
/// This is a static array to avoid heap allocations and enable compile-time verification.
/// Entries are sorted by CRC32 for potential binary search optimization in the future.
static ROM_DATABASE: &[RomDbEntry] = &[
    // Example entries - these should be replaced with actual ROM data
    // CRC32 values are placeholders and should be calculated from real ROMs

    // Note: Add real entries here as they are discovered
    // Format:
    // RomDbEntry::new(
    //     0x12345678, // CRC32
    //     Some(4),    // Mapper override (MMC3)
    //     Some(Mirroring::Horizontal), // Mirroring override
    //     Some("TLSROM"), // Board name
    // ),
];

/// Calculate CRC32 checksum of ROM data.
///
/// This includes the entire ROM file, including the iNES header.
///
/// # Arguments
///
/// * `data` - The complete ROM file data (header + PRG + CHR)
///
/// # Returns
///
/// The CRC32 checksum as a 32-bit unsigned integer.
pub fn calculate_crc32(data: &[u8]) -> u32 {
    let mut hasher = crc32fast::Hasher::new();
    hasher.update(data);
    hasher.finalize()
}

/// Look up a ROM in the database by its CRC32 checksum.
///
/// # Arguments
///
/// * `crc32` - The CRC32 checksum of the ROM file
///
/// # Returns
///
/// An `Option` containing a reference to the database entry if found, or `None` if not found.
pub fn lookup_rom(crc32: u32) -> Option<&'static RomDbEntry> {
    ROM_DATABASE.iter().find(|entry| entry.crc32 == crc32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crc32_calculation() {
        // Test with a known CRC32 value
        // "NES\x1A" is the iNES header magic
        let test_data = b"NES\x1A";
        let crc = calculate_crc32(test_data);

        // Verify the CRC is calculated (non-zero for this input)
        assert_ne!(crc, 0);

        // Verify consistency - same input should give same output
        let crc2 = calculate_crc32(test_data);
        assert_eq!(crc, crc2);
    }

    #[test]
    fn test_crc32_different_data() {
        // Different data should give different CRC32 values
        let data1 = b"NES\x1A\x01\x01\x00\x00";
        let data2 = b"NES\x1A\x02\x01\x00\x00";

        let crc1 = calculate_crc32(data1);
        let crc2 = calculate_crc32(data2);

        assert_ne!(crc1, crc2);
    }

    #[test]
    fn test_lookup_rom_not_found() {
        // Looking up a non-existent CRC32 should return None
        let result = lookup_rom(0xDEADBEEF);
        assert!(result.is_none());
    }

    #[test]
    fn test_lookup_rom_found() {
        // If we add an entry to the database, it should be found
        // This test is a placeholder - it will pass as long as the database is empty
        // Once real entries are added, update this test

        // For now, verify the database is accessible
        assert_eq!(ROM_DATABASE.len(), 0);
    }

    #[test]
    fn test_rom_db_entry_creation() {
        // Test creating a database entry
        let entry = RomDbEntry::new(
            0x12345678,
            Some(4),
            Some(Mirroring::Horizontal),
            Some("TEST"),
        );

        assert_eq!(entry.crc32, 0x12345678);
        assert_eq!(entry.mapper, Some(4));
        assert_eq!(entry.mirroring, Some(Mirroring::Horizontal));
        assert_eq!(entry.board, Some("TEST"));
    }

    #[test]
    fn test_rom_db_entry_optional_fields() {
        // Test creating an entry with only CRC32 (no overrides)
        let entry = RomDbEntry::new(0xABCDEF00, None, None, None);

        assert_eq!(entry.crc32, 0xABCDEF00);
        assert_eq!(entry.mapper, None);
        assert_eq!(entry.mirroring, None);
        assert_eq!(entry.board, None);
    }
}
