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
//! Example:
//! ```rust,ignore
//! RomDbEntry::new(
//!     0x12345678,                      // CRC32 of full ROM file
//!     Some(4),                          // Override to mapper 4 (MMC3)
//!     Some(Mirroring::Horizontal),      // Override to horizontal mirroring
//!     Some("TLSROM"),                   // Board type (optional, for documentation)
//! ),
//! ```
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
    pub mapper: Option<u16>,
    /// Optional mirroring mode override (if None, use header value)
    pub mirroring: Option<Mirroring>,
    /// Optional board name for documentation purposes
    pub board: Option<&'static str>,
}

impl RomDbEntry {
    /// Create a new database entry with all fields
    ///
    /// This method is used when populating the ROM database with known ROMs.
    /// It's currently unused because the database is empty, but will be needed
    /// when adding actual ROM entries.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// RomDbEntry::new(
    ///     0x12345678,                      // CRC32 of full ROM file
    ///     Some(4),                          // Override to mapper 4 (MMC3)
    ///     Some(Mirroring::Horizontal),      // Override to horizontal mirroring
    ///     Some("TLSROM"),                   // Board type (optional)
    /// )
    /// ```
    pub const fn new(
        crc32: u32,
        mapper: Option<u16>,
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
    // Bad Dudes (USA) - MMC3/TLROM board with horizontal mirroring
    // Some dumps have corrupted headers that may misidentify mapper or mirroring
    RomDbEntry::new(
        0x161D717B,                  // CRC32 of full ROM file
        Some(4),                     // Override to mapper 4 (MMC3)
        Some(Mirroring::Horizontal), // TLROM uses horizontal mirroring
        Some("TLROM"),               // Board type
    ),
    // Dragon Ninja (Japan) - Same game as Bad Dudes, MMC3/TLROM
    RomDbEntry::new(
        0x2A7D3ADF,                  // CRC32 of full ROM file
        Some(4),                     // Override to mapper 4 (MMC3)
        Some(Mirroring::Horizontal), // TLROM uses horizontal mirroring
        Some("TLROM"),               // Board type
    ),
    // Dragon Ninja (Japan, Rev A) - Revised version, MMC3/TLROM
    RomDbEntry::new(
        0x2AE535CA,                  // CRC32 of full ROM file
        Some(4),                     // Override to mapper 4 (MMC3)
        Some(Mirroring::Horizontal), // TLROM uses horizontal mirroring
        Some("TLROM"),               // Board type
    ),
    // Bad Dudes vs Dragon Ninja (Europe) - MMC3/TLROM
    RomDbEntry::new(
        0x8C252AC4,                  // CRC32 of full ROM file
        Some(4),                     // Override to mapper 4 (MMC3)
        Some(Mirroring::Horizontal), // TLROM uses horizontal mirroring
        Some("TLROM"),               // Board type
    ),
    // Bee 52 (USA) (Unl) - Header incorrectly specifies horizontal mirroring,
    // but the game requires vertical mirroring for correct scrolling
    RomDbEntry::new(
        0xE19C2722,                // CRC32 of full ROM file
        None,                      // Use header mapper (71 - Camerica)
        Some(Mirroring::Vertical), // Override to vertical mirroring
        None,                      // No specific board name
    ),
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
        // The database has entries - verify a known entry is found
        assert!(!ROM_DATABASE.is_empty());

        // Dynamically select a known entry from the ROM database
        let known = &ROM_DATABASE[0];

        // Look up the entry by its CRC32 and verify all fields match
        let looked_up = lookup_rom(known.crc32);
        assert!(looked_up.is_some());
        let entry = looked_up.unwrap();
        assert_eq!(*entry, *known);
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
