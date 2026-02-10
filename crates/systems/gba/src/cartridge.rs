//! GBA cartridge identification and ROM header parsing.
//!
//! The GBA ROM header occupies the first 192 bytes (0x000–0x0BF) of the cartridge.
//! It contains identification information, the entry point, and a header checksum.
//!
//! ## Header Layout
//!
//! | Offset  | Size | Description                         |
//! |---------|------|-------------------------------------|
//! | 0x000   | 4    | ROM entry point (ARM branch)        |
//! | 0x004   | 156  | Nintendo logo (compressed bitmap)    |
//! | 0x0A0   | 12   | Game title (uppercase ASCII)        |
//! | 0x0AC   | 4    | Game code (e.g., "AXVE")            |
//! | 0x0B0   | 2    | Maker code (e.g., "01" = Nintendo)  |
//! | 0x0B2   | 1    | Fixed value (must be 0x96)          |
//! | 0x0B3   | 1    | Main unit code (0x00 for GBA)       |
//! | 0x0B4   | 1    | Device type                         |
//! | 0x0B5   | 7    | Reserved (should be zero)           |
//! | 0x0BC   | 1    | Software version                    |
//! | 0x0BD   | 1    | Complement check (header checksum)  |
//! | 0x0BE   | 2    | Reserved (should be zero)           |

use emu_core::logging::{log, LogCategory, LogLevel};

/// Minimum size for a valid GBA ROM (at least the header)
const GBA_HEADER_SIZE: usize = 0xC0; // 192 bytes

/// The fixed value at offset 0xB2 that identifies a GBA cartridge
const GBA_FIXED_VALUE: u8 = 0x96;

/// GBA Nintendo logo (compressed bitmap, 156 bytes at offset 0x04-0x9F)
/// Used for cartridge validation. The BIOS checks this during boot.
const NINTENDO_LOGO: [u8; 156] = [
    0x24, 0xFF, 0xAE, 0x51, 0x69, 0x9A, 0xA2, 0x21, 0x3D, 0x84, 0x82, 0x0A, 0x84, 0xE4, 0x09,
    0xAD, 0x11, 0x24, 0x8B, 0x98, 0xC0, 0x81, 0x7F, 0x21, 0xA3, 0x52, 0xBE, 0x19, 0x93, 0x09,
    0xCE, 0x20, 0x10, 0x46, 0x4A, 0x4A, 0xF8, 0x27, 0x31, 0xEC, 0x58, 0xC7, 0xE8, 0x33, 0x82,
    0xE3, 0xCE, 0xBF, 0x85, 0xF4, 0xDF, 0x94, 0xCE, 0x4B, 0x09, 0xC1, 0x94, 0x56, 0x8A, 0xC0,
    0x13, 0x72, 0xA7, 0xFC, 0x9F, 0x84, 0x4D, 0x73, 0xA3, 0xCA, 0x9A, 0x61, 0x58, 0x97, 0xA3,
    0x27, 0xFC, 0x03, 0x98, 0x76, 0x23, 0x1D, 0xC7, 0x61, 0x03, 0x04, 0xAE, 0x56, 0xBF, 0x38,
    0x84, 0x00, 0x40, 0xA7, 0x0E, 0xFD, 0xFF, 0x52, 0xFE, 0x03, 0x6F, 0x95, 0x30, 0xF1, 0x97,
    0xFB, 0xC0, 0x85, 0x60, 0xD6, 0x80, 0x25, 0xA9, 0x63, 0xBE, 0x03, 0x01, 0x4E, 0x38, 0xE2,
    0xF9, 0xA2, 0x34, 0xFF, 0xBB, 0x3E, 0x03, 0x44, 0x78, 0x00, 0x90, 0xCB, 0x88, 0x11, 0x3A,
    0x94, 0x65, 0xC0, 0x7C, 0x63, 0x87, 0xF0, 0x3C, 0xAF, 0xD6, 0x25, 0xE4, 0x8B, 0x38, 0x0A,
    0xAC, 0x72, 0x21, 0xD4, 0xF8, 0x07,
];

/// Known GBA maker codes
fn maker_name(code: &str) -> &'static str {
    match code {
        "01" => "Nintendo",
        "08" => "Capcom",
        "13" => "Electronic Arts",
        "18" => "Hudson Soft",
        "20" => "DSI/Destination Software",
        "2N" => "Kemco",
        "34" => "Konami",
        "41" => "Ubi Soft",
        "4F" => "Eidos",
        "4Q" => "Disney Interactive",
        "52" => "Activision",
        "54" => "Rockstar Games",
        "5D" => "Midway",
        "5G" => "Majesco",
        "64" => "LucasArts",
        "69" => "THQ",
        "6S" => "Star-Fish",
        "70" => "Atari/Infogrames",
        "78" => "THQ",
        "7D" => "Vivendi",
        "7J" => "Zodiac",
        "8J" => "Kadokawa Shoten",
        "8P" => "Sega",
        "99" => "AQ Interactive",
        "A4" => "Tecmo/Koei",
        "AF" => "Namco Bandai",
        "B2" => "Bandai",
        "B4" => "Enix",
        "BN" => "Sunrise",
        "C8" => "Koei",
        "EB" => "Atlus",
        "FH" => "Foreign Media",
        _ => "Unknown",
    }
}

/// Game code region prefix (first character of the 4-char game code)
fn region_from_game_code(code: &str) -> &'static str {
    if code.len() < 4 {
        return "Unknown";
    }
    match code.as_bytes()[3] {
        b'J' => "Japan",
        b'E' => "USA/English",
        b'P' => "Europe",
        b'D' => "Germany",
        b'F' => "France",
        b'I' => "Italy",
        b'S' => "Spain",
        b'U' => "Australia",
        _ => "Unknown",
    }
}

/// Parsed GBA cartridge header information.
#[derive(Debug, Clone)]
pub struct GbaCartridgeHeader {
    /// Entry point ARM instruction (branch to start of code)
    pub entry_point: u32,
    /// Game title (up to 12 ASCII characters)
    pub title: String,
    /// Game code (4 characters, e.g., "AXVE" for Pokémon Ruby)
    pub game_code: String,
    /// Maker code (2 characters, e.g., "01" for Nintendo)
    pub maker_code: String,
    /// Fixed value (should be 0x96)
    pub fixed_value: u8,
    /// Main unit code (should be 0x00 for current GBA models)
    pub main_unit_code: u8,
    /// Device type
    pub device_type: u8,
    /// Software version number
    pub software_version: u8,
    /// Header checksum (complement check)
    pub complement_check: u8,
    /// Whether the header checksum is valid
    pub checksum_valid: bool,
    /// Whether the Nintendo logo matches
    pub logo_valid: bool,
    /// Total ROM size in bytes
    pub rom_size: usize,
    /// Detected save type (from ROM scanning)
    pub save_type: SaveType,
}

/// Detected cartridge save type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveType {
    /// No save hardware detected
    None,
    /// EEPROM (512 bytes or 8KB)
    Eeprom,
    /// SRAM (32KB)
    Sram,
    /// Flash RAM (64KB)
    Flash64K,
    /// Flash RAM (128KB)
    Flash128K,
}

impl std::fmt::Display for SaveType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SaveType::None => write!(f, "None"),
            SaveType::Eeprom => write!(f, "EEPROM"),
            SaveType::Sram => write!(f, "SRAM (32KB)"),
            SaveType::Flash64K => write!(f, "Flash (64KB)"),
            SaveType::Flash128K => write!(f, "Flash (128KB)"),
        }
    }
}

impl GbaCartridgeHeader {
    /// Parse a GBA ROM header from raw ROM data.
    ///
    /// Returns `None` if the data is too small to contain a valid header.
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < GBA_HEADER_SIZE {
            log(LogCategory::Bus, LogLevel::Warn, || {
                format!(
                    "GBA: ROM too small for header ({} bytes, need {})",
                    data.len(),
                    GBA_HEADER_SIZE
                )
            });
            return None;
        }

        // Entry point (ARM branch instruction at offset 0x000)
        let entry_point = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);

        // Nintendo logo check (offset 0x004-0x09F, 156 bytes)
        let logo_valid = data[0x04..0xA0] == NINTENDO_LOGO;

        // Game title (offset 0x0A0-0x0AB, 12 bytes, uppercase ASCII, null-padded)
        let title = std::str::from_utf8(&data[0xA0..0xAC])
            .unwrap_or("")
            .trim_end_matches('\0')
            .to_string();

        // Game code (offset 0x0AC-0x0AF, 4 bytes)
        let game_code = std::str::from_utf8(&data[0xAC..0xB0])
            .unwrap_or("")
            .trim_end_matches('\0')
            .to_string();

        // Maker code (offset 0x0B0-0x0B1, 2 bytes)
        let maker_code = std::str::from_utf8(&data[0xB0..0xB2])
            .unwrap_or("")
            .trim_end_matches('\0')
            .to_string();

        // Fixed value (offset 0x0B2, must be 0x96)
        let fixed_value = data[0xB2];

        // Main unit code (offset 0x0B3)
        let main_unit_code = data[0xB3];

        // Device type (offset 0x0B4)
        let device_type = data[0xB4];

        // Software version (offset 0x0BC)
        let software_version = data[0xBC];

        // Complement check (offset 0x0BD)
        // Checksum = -(sum of bytes 0xA0..0xBC + 0x19) & 0xFF
        let complement_check = data[0xBD];
        let computed_checksum = {
            let mut sum: u8 = 0;
            for &byte in &data[0xA0..0xBD] {
                sum = sum.wrapping_add(byte);
            }
            (-(sum as i8).wrapping_add(0x19)) as u8
        };
        let checksum_valid = complement_check == computed_checksum;

        // Detect save type by scanning ROM for identification strings
        let save_type = detect_save_type(data);

        let rom_size = data.len();

        let header = Self {
            entry_point,
            title,
            game_code,
            maker_code,
            fixed_value,
            main_unit_code,
            device_type,
            software_version,
            complement_check,
            checksum_valid,
            logo_valid,
            rom_size,
            save_type,
        };

        // Log cartridge info
        let maker = maker_name(&header.maker_code);
        let region = region_from_game_code(&header.game_code);

        log(LogCategory::Bus, LogLevel::Info, || {
            format!(
                "GBA: Loaded cartridge - \"{}\" [{}] ({}, {}, {} KB, v{}, save: {}{}{})",
                header.title,
                header.game_code,
                maker,
                region,
                header.rom_size / 1024,
                header.software_version,
                header.save_type,
                if !header.checksum_valid {
                    ", BAD CHECKSUM"
                } else {
                    ""
                },
                if !header.logo_valid {
                    ", BAD LOGO"
                } else {
                    ""
                },
            )
        });

        if fixed_value != GBA_FIXED_VALUE {
            log(LogCategory::Bus, LogLevel::Warn, || {
                format!(
                    "GBA: Fixed value mismatch (expected 0x{:02X}, got 0x{:02X})",
                    GBA_FIXED_VALUE, fixed_value
                )
            });
        }

        Some(header)
    }

    /// Get a human-readable summary of the cartridge
    pub fn summary(&self) -> String {
        let maker = maker_name(&self.maker_code);
        let region = region_from_game_code(&self.game_code);
        format!(
            "{} [{}] by {} | {} | {} KB | Save: {} | v{}",
            self.title,
            self.game_code,
            maker,
            region,
            self.rom_size / 1024,
            self.save_type,
            self.software_version,
        )
    }
}

/// Detect save type by scanning the ROM for identification strings.
///
/// GBA games embed library identification strings that indicate the save hardware:
/// - "EEPROM_V" → EEPROM
/// - "SRAM_V" → SRAM
/// - "FLASH_V" or "FLASH512_V" → Flash 64KB
/// - "FLASH1M_V" → Flash 128KB
fn detect_save_type(data: &[u8]) -> SaveType {
    // Search for save type identification strings in the ROM
    // These are embedded by the SDK libraries
    let rom_str = data
        .windows(10)
        .find_map(|window| {
            if window.starts_with(b"FLASH1M_V") {
                Some(SaveType::Flash128K)
            } else if window.starts_with(b"FLASH512_") || window.starts_with(b"FLASH_V") {
                Some(SaveType::Flash64K)
            } else if window.starts_with(b"SRAM_V") {
                Some(SaveType::Sram)
            } else if window.starts_with(b"EEPROM_V") {
                Some(SaveType::Eeprom)
            } else {
                None
            }
        });

    rom_str.unwrap_or(SaveType::None)
}

impl std::fmt::Display for GbaCartridgeHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "GBA Cartridge Header:")?;
        writeln!(
            f,
            "  Title:      {}",
            if self.title.is_empty() {
                "(none)"
            } else {
                &self.title
            }
        )?;
        writeln!(
            f,
            "  Game Code:  {}",
            if self.game_code.is_empty() {
                "(none)"
            } else {
                &self.game_code
            }
        )?;
        writeln!(
            f,
            "  Maker:      {} ({})",
            self.maker_code,
            maker_name(&self.maker_code)
        )?;
        writeln!(
            f,
            "  Region:     {}",
            region_from_game_code(&self.game_code)
        )?;
        writeln!(f, "  ROM Size:   {} KB", self.rom_size / 1024)?;
        writeln!(f, "  Version:    {}", self.software_version)?;
        writeln!(f, "  Save Type:  {}", self.save_type)?;
        writeln!(f, "  Entry:      ${:08X}", self.entry_point)?;
        writeln!(
            f,
            "  Checksum:   0x{:02X} ({})",
            self.complement_check,
            if self.checksum_valid { "OK" } else { "BAD" }
        )?;
        writeln!(
            f,
            "  Logo:       {}",
            if self.logo_valid { "Valid" } else { "Invalid" }
        )?;
        write!(
            f,
            "  Fixed:      0x{:02X} ({})",
            self.fixed_value,
            if self.fixed_value == GBA_FIXED_VALUE {
                "OK"
            } else {
                "BAD"
            }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a minimal valid GBA ROM header for testing
    fn create_test_rom(title: &str, game_code: &str, maker_code: &str) -> Vec<u8> {
        let mut rom = vec![0u8; 0x100]; // Minimum size with some padding

        // Entry point: B 0x08000000 (ARM branch)
        rom[0] = 0x00;
        rom[1] = 0x00;
        rom[2] = 0x00;
        rom[3] = 0xEA;

        // Nintendo logo
        rom[0x04..0xA0].copy_from_slice(&NINTENDO_LOGO);

        // Title (12 bytes, null-padded)
        let title_bytes = title.as_bytes();
        let len = title_bytes.len().min(12);
        rom[0xA0..0xA0 + len].copy_from_slice(&title_bytes[..len]);

        // Game code (4 bytes)
        let code_bytes = game_code.as_bytes();
        let len = code_bytes.len().min(4);
        rom[0xAC..0xAC + len].copy_from_slice(&code_bytes[..len]);

        // Maker code (2 bytes)
        let maker_bytes = maker_code.as_bytes();
        let len = maker_bytes.len().min(2);
        rom[0xB0..0xB0 + len].copy_from_slice(&maker_bytes[..len]);

        // Fixed value
        rom[0xB2] = GBA_FIXED_VALUE;

        // Compute complement check
        let mut sum: u8 = 0;
        for &byte in &rom[0xA0..0xBD] {
            sum = sum.wrapping_add(byte);
        }
        rom[0xBD] = (-(sum as i8).wrapping_add(0x19)) as u8;

        rom
    }

    #[test]
    fn test_parse_header_basic() {
        let rom = create_test_rom("TESTGAME", "ATSE", "01");
        let header = GbaCartridgeHeader::from_bytes(&rom).unwrap();

        assert_eq!(header.title, "TESTGAME");
        assert_eq!(header.game_code, "ATSE");
        assert_eq!(header.maker_code, "01");
        assert_eq!(header.fixed_value, GBA_FIXED_VALUE);
        assert!(header.checksum_valid);
        assert!(header.logo_valid);
    }

    #[test]
    fn test_parse_header_empty_title() {
        let rom = create_test_rom("", "AXVE", "01");
        let header = GbaCartridgeHeader::from_bytes(&rom).unwrap();
        assert_eq!(header.title, "");
    }

    #[test]
    fn test_parse_header_full_title() {
        let rom = create_test_rom("POKEMON RUBY", "AXVE", "01");
        let header = GbaCartridgeHeader::from_bytes(&rom).unwrap();
        assert_eq!(header.title, "POKEMON RUBY");
    }

    #[test]
    fn test_header_too_small() {
        let small_rom = vec![0u8; 32];
        assert!(GbaCartridgeHeader::from_bytes(&small_rom).is_none());
    }

    #[test]
    fn test_bad_checksum() {
        let mut rom = create_test_rom("BADCSUM", "TEST", "01");
        rom[0xBD] = 0xFF; // Corrupt checksum
        let header = GbaCartridgeHeader::from_bytes(&rom).unwrap();
        assert!(!header.checksum_valid);
    }

    #[test]
    fn test_bad_logo() {
        let mut rom = create_test_rom("BADLOGO", "TEST", "01");
        rom[0x04] = 0x00; // Corrupt logo
        let header = GbaCartridgeHeader::from_bytes(&rom).unwrap();
        assert!(!header.logo_valid);
    }

    #[test]
    fn test_bad_fixed_value() {
        let mut rom = create_test_rom("BADFIXED", "TEST", "01");
        rom[0xB2] = 0x00; // Wrong fixed value
        // Recompute checksum since we changed header data
        let mut sum: u8 = 0;
        for &byte in &rom[0xA0..0xBD] {
            sum = sum.wrapping_add(byte);
        }
        rom[0xBD] = (-(sum as i8).wrapping_add(0x19)) as u8;

        let header = GbaCartridgeHeader::from_bytes(&rom).unwrap();
        assert_ne!(header.fixed_value, GBA_FIXED_VALUE);
    }

    #[test]
    fn test_save_type_detection_sram() {
        let mut rom = create_test_rom("SRAMGAME", "TEST", "01");
        rom.resize(0x1000, 0);
        // Inject SRAM identification string
        let sram_id = b"SRAM_V113";
        rom[0x200..0x200 + sram_id.len()].copy_from_slice(sram_id);

        let header = GbaCartridgeHeader::from_bytes(&rom).unwrap();
        assert_eq!(header.save_type, SaveType::Sram);
    }

    #[test]
    fn test_save_type_detection_flash() {
        let mut rom = create_test_rom("FLASHGAME", "TEST", "01");
        rom.resize(0x1000, 0);
        let flash_id = b"FLASH1M_V103";
        rom[0x200..0x200 + flash_id.len()].copy_from_slice(flash_id);

        let header = GbaCartridgeHeader::from_bytes(&rom).unwrap();
        assert_eq!(header.save_type, SaveType::Flash128K);
    }

    #[test]
    fn test_save_type_detection_eeprom() {
        let mut rom = create_test_rom("EEPROMGAM", "TEST", "01");
        rom.resize(0x1000, 0);
        let eeprom_id = b"EEPROM_V124";
        rom[0x200..0x200 + eeprom_id.len()].copy_from_slice(eeprom_id);

        let header = GbaCartridgeHeader::from_bytes(&rom).unwrap();
        assert_eq!(header.save_type, SaveType::Eeprom);
    }

    #[test]
    fn test_save_type_none() {
        let rom = create_test_rom("NOSAVE", "TEST", "01");
        let header = GbaCartridgeHeader::from_bytes(&rom).unwrap();
        assert_eq!(header.save_type, SaveType::None);
    }

    #[test]
    fn test_maker_name_lookup() {
        assert_eq!(maker_name("01"), "Nintendo");
        assert_eq!(maker_name("08"), "Capcom");
        assert_eq!(maker_name("8P"), "Sega");
        assert_eq!(maker_name("XX"), "Unknown");
    }

    #[test]
    fn test_region_detection() {
        assert_eq!(region_from_game_code("AXVJ"), "Japan");
        assert_eq!(region_from_game_code("AXVE"), "USA/English");
        assert_eq!(region_from_game_code("AXVP"), "Europe");
        assert_eq!(region_from_game_code("AX"), "Unknown");
    }

    #[test]
    fn test_header_display() {
        let rom = create_test_rom("TESTGAME", "ATSE", "01");
        let header = GbaCartridgeHeader::from_bytes(&rom).unwrap();
        let display = format!("{}", header);
        assert!(display.contains("TESTGAME"));
        assert!(display.contains("ATSE"));
        assert!(display.contains("Nintendo"));
    }

    #[test]
    fn test_header_summary() {
        let rom = create_test_rom("TESTGAME", "ATSE", "01");
        let header = GbaCartridgeHeader::from_bytes(&rom).unwrap();
        let summary = header.summary();
        assert!(summary.contains("TESTGAME"));
        assert!(summary.contains("ATSE"));
        assert!(summary.contains("Nintendo"));
    }

    #[test]
    fn test_rom_size_tracking() {
        let mut rom = create_test_rom("SIZETEST", "TEST", "01");
        rom.resize(4 * 1024 * 1024, 0); // 4MB ROM
        let header = GbaCartridgeHeader::from_bytes(&rom).unwrap();
        assert_eq!(header.rom_size, 4 * 1024 * 1024);
    }
}
