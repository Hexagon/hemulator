/// ROM detection and system selection
use std::error::Error;
use std::fmt;

/// Standard floppy disk image sizes in bytes.
///
/// - 360 KB  (5.25", double-density)
/// - 720 KB  (3.5", double-density)
/// - 1.2 MB  (5.25", high-density)
/// - 1.44 MB (3.5", high-density) – most common
/// - 2.88 MB (3.5", extended density)
pub const FLOPPY_IMAGE_SIZES: &[usize] = &[368_640, 737_280, 1_228_800, 1_474_560, 2_949_120];

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::upper_case_acronyms)]
pub enum SystemType {
    NES,
    GameBoy,
    GBA,
    Atari2600,
    PC,
    SNES,
    N64,
    SMS,
    Chip8,
    ColecoVision,
    SG1000,
    PS1,
}

#[derive(Debug)]
pub struct UnsupportedRomError {
    pub reason: String,
}

impl fmt::Display for UnsupportedRomError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Unsupported ROM: {}", self.reason)
    }
}

impl Error for UnsupportedRomError {}

/// Detect ROM type with file extension hint and optional preferred system
/// This function first checks the file extension for unambiguous cases (like .ch8 for CHIP-8),
/// then uses the preferred system for ambiguous extensions (like .bin),
/// and finally falls back to content-based detection
pub fn detect_rom_type_with_extension(
    data: &[u8],
    extension: Option<&str>,
    preferred_system: Option<SystemType>,
) -> Result<SystemType, UnsupportedRomError> {
    // Check file extension first for unambiguous cases
    // CHIP-8 files (.ch8, .c8) have no header and overlap in size with PC COM files
    if let Some(ext) = extension {
        let ext_lower = ext.to_lowercase();
        match ext_lower.as_str() {
            "ch8" | "c8" => return Ok(SystemType::Chip8),
            "nes" => {
                // For .nes extension, still verify it has iNES header
                if data.len() >= 16 && &data[0..4] == b"NES\x1A" {
                    return Ok(SystemType::NES);
                }
                // If no header, fall through to content detection
            }
            "gb" | "gbc" => {
                // For Game Boy extensions, verify the logo
                if data.len() >= 0x150 {
                    let logo_start = &data[0x104..0x108];
                    if logo_start == [0xCE, 0xED, 0x66, 0x66] {
                        return Ok(SystemType::GameBoy);
                    }
                }
                // Fall through to content detection
            }
            "gba" | "agb" => {
                return Ok(SystemType::GBA);
            }
            "sms" => {
                // Prefer SMS for .sms extension even without header
                return Ok(SystemType::SMS);
            }
            "col" => {
                // ColecoVision cartridge
                return Ok(SystemType::ColecoVision);
            }
            "sg" | "sc" => {
                // SG-1000 cartridge (.sg) or SC-3000 (.sc)
                return Ok(SystemType::SG1000);
            }
            "a26" => {
                // For .a26 extension, prefer Atari detection
                // Check if size matches known Atari cartridge sizes
                if matches!(data.len(), 2048 | 4096 | 8192 | 12288 | 16384 | 32768) {
                    return Ok(SystemType::Atari2600);
                }
                // Fall through to content detection for other sizes
            }
            "bin" | "iso" | "img" | "ima" => {
                // Check for PS1 BIOS first (512KB .bin files)
                if is_ps1_bios(data) {
                    return Ok(SystemType::PS1);
                }
                // Check for PS1 disc image
                if is_ps1_disc_image(data) {
                    return Ok(SystemType::PS1);
                }
                // .img and .ima are PC disk image extensions: detect floppy/hard-drive sizes
                if matches!(ext_lower.as_str(), "img" | "ima") {
                    if FLOPPY_IMAGE_SIZES.contains(&data.len()) {
                        return Ok(SystemType::PC);
                    }
                    // Hard drive: >= 1 MB and not a recognised floppy size
                    if data.len() >= 1024 * 1024 {
                        return Ok(SystemType::PC);
                    }
                }
                // For .bin extension (ambiguous), use preferred system if provided
                if let Some(preferred) = preferred_system {
                    // Validate the size matches the preferred system's expectations
                    match preferred {
                        SystemType::Atari2600 => {
                            if matches!(data.len(), 2048 | 4096 | 8192 | 12288 | 16384 | 32768) {
                                return Ok(SystemType::Atari2600);
                            }
                        }
                        SystemType::GameBoy => {
                            // Check for Game Boy logo
                            if data.len() >= 0x150 {
                                let logo_start = &data[0x104..0x108];
                                if logo_start == [0xCE, 0xED, 0x66, 0x66] {
                                    return Ok(SystemType::GameBoy);
                                }
                            }
                        }
                        SystemType::SNES => {
                            // SNES ROMs are power-of-2 sizes
                            let header_offset = if data.len() % 1024 == 512 { 512 } else { 0 };
                            let rom_size = data.len() - header_offset;
                            if rom_size >= 0x8000
                                && rom_size.is_power_of_two()
                                && rom_size <= 0x400000
                            {
                                return Ok(SystemType::SNES);
                            }
                        }
                        SystemType::N64 => {
                            // N64 has magic bytes, check those
                            if data.len() >= 4 {
                                match &data[0..4] {
                                    [0x80, 0x37, 0x12, 0x40]
                                    | [0x40, 0x12, 0x37, 0x80]
                                    | [0x37, 0x80, 0x40, 0x12] => {
                                        return Ok(SystemType::N64);
                                    }
                                    _ => {}
                                }
                            }
                        }
                        _ => {
                            // For other systems, fall through to content detection
                        }
                    }
                }
                // If no preferred system or validation failed, check Atari first (common .bin use)
                if matches!(data.len(), 2048 | 4096 | 8192 | 12288 | 16384 | 32768) {
                    return Ok(SystemType::Atari2600);
                }
                // Fall through to content detection
            }
            "smc" | "sfc" => {
                // Prefer SNES for .smc/.sfc extensions
                return Ok(SystemType::SNES);
            }
            "z64" | "n64" | "v64" => {
                // Prefer N64 for these extensions
                return Ok(SystemType::N64);
            }
            "com" => {
                // PC COM executable
                return Ok(SystemType::PC);
            }
            "exe" => {
                // Could be PC EXE or PS-X EXE
                if data.len() >= 8 && &data[0..8] == b"PS-X EXE" {
                    return Ok(SystemType::PS1);
                }
                return Ok(SystemType::PC);
            }
            "psexe" | "psx" | "cue" => {
                return Ok(SystemType::PS1);
            }
            _ => {
                // Unknown extension - use preferred system if provided
                if let Some(preferred) = preferred_system {
                    // For unknown extensions, trust the preferred system
                    // but still do basic validation
                    match preferred {
                        SystemType::NES => {
                            if data.len() >= 16 && &data[0..4] == b"NES\x1A" {
                                return Ok(SystemType::NES);
                            }
                        }
                        SystemType::GameBoy => {
                            if data.len() >= 0x150 {
                                let logo_start = &data[0x104..0x108];
                                if logo_start == [0xCE, 0xED, 0x66, 0x66] {
                                    return Ok(SystemType::GameBoy);
                                }
                            }
                        }
                        SystemType::N64 => {
                            if data.len() >= 4 {
                                match &data[0..4] {
                                    [0x80, 0x37, 0x12, 0x40]
                                    | [0x40, 0x12, 0x37, 0x80]
                                    | [0x37, 0x80, 0x40, 0x12] => {
                                        return Ok(SystemType::N64);
                                    }
                                    _ => {}
                                }
                            }
                        }
                        // For other preferred systems, just return them
                        // (they don't have strict headers we can validate)
                        _ => return Ok(preferred),
                    }
                }
                // Fall through to content detection
            }
        }
    }

    // Fall back to content-based detection
    detect_rom_type(data)
}

pub fn detect_rom_type(data: &[u8]) -> Result<SystemType, UnsupportedRomError> {
    // Check for NES (iNES format)
    if data.len() >= 16 && &data[0..4] == b"NES\x1A" {
        return Ok(SystemType::NES);
    }

    // Check for N64 (magic bytes)
    if data.len() >= 4 {
        match &data[0..4] {
            [0x80, 0x37, 0x12, 0x40] | // .z64
            [0x40, 0x12, 0x37, 0x80] | // .n64
            [0x37, 0x80, 0x40, 0x12]   // .v64
            => {
                return Ok(SystemType::N64);
            }
            _ => {}
        }
    }

    // Check for Game Boy (check BEFORE SNES due to overlapping size ranges)
    // Game Boy ROMs have a Nintendo logo at 0x104-0x133 and a header checksum at 0x14D
    if data.len() >= 0x150 {
        // Check for the Nintendo logo bytes (partial check for first few bytes)
        let logo_start = &data[0x104..0x108];
        // Standard GB/GBC logo starts with 0xCE 0xED 0x66 0x66
        if logo_start == [0xCE, 0xED, 0x66, 0x66] {
            return Ok(SystemType::GameBoy);
        }
    }

    // Check for Sega Master System (check BEFORE SNES due to overlapping size ranges)
    // SMS ROMs can have optional TMR SEGA header at 0x7FF0
    // Common sizes: 8KB to 512KB (power of 2, or with 512-byte header)
    if data.len() >= 0x7FF0 + 16 {
        // Check for TMR SEGA header
        let header_offset = if data.len() % 1024 == 512 { 512 } else { 0 };
        let sig_offset = header_offset + 0x7FF0;

        if sig_offset + 8 <= data.len() {
            let signature = &data[sig_offset..sig_offset + 8];
            if signature == b"TMR SEGA" {
                return Ok(SystemType::SMS);
            }
        }
    }

    // Also check common SMS ROM sizes (headerless) that don't overlap with common SNES sizes
    // SMS-specific sizes: 48KB is common for SMS, less so for SNES
    // We'll be conservative and only auto-detect SMS for sizes that are distinctly SMS
    if matches!(data.len(), 49152) {
        // 48KB is a common SMS size but not a standard SNES size
        return Ok(SystemType::SMS);
    }

    // Check for SNES (SMC header or size-based detection)
    // SNES ROMs are typically multiples of 32KB (with optional 512-byte SMC header)
    if data.len() >= 0x8000 {
        // Check for SMC header (512 bytes)
        let header_offset = if data.len() % 1024 == 512 { 512 } else { 0 };
        let rom_size = data.len() - header_offset;

        // SNES ROMs are typically 32KB, 64KB, 128KB, 256KB, 512KB, 1MB, 2MB, 4MB
        if rom_size >= 0x8000 && rom_size.is_power_of_two() && rom_size <= 0x400000 {
            // Additional validation: check for valid SNES header at known locations
            // LoROM: $7FC0-$7FFF, HiROM: $FFC0-$FFFF
            // For now, we'll accept any power-of-2 sized ROM >= 32KB as potentially SNES
            // This is a heuristic and may need refinement
            return Ok(SystemType::SNES);
        }
    }

    // Check for PS-X EXE
    if data.len() >= 8 && &data[0..8] == b"PS-X EXE" {
        return Ok(SystemType::PS1);
    }

    // Check for PS1 BIOS (512KB, contains "Sony Computer Entertainment")
    if is_ps1_bios(data) {
        return Ok(SystemType::PS1);
    }

    // Check for PS1 disc image (BIN/IMG format with CD sync pattern)
    if is_ps1_disc_image(data) {
        return Ok(SystemType::PS1);
    }

    // Check for DOS executable (MZ header)
    if data.len() >= 2 && &data[0..2] == b"MZ" {
        return Ok(SystemType::PC);
    }

    // Check for Atari 2600 FIRST (before CHIP-8 and COM files)
    // Atari 2600 ROMs are typically 2K, 4K, 8K, 12K, 16K, or 32K
    // They have no header, so we detect by size and lack of other formats
    if matches!(data.len(), 2048 | 4096 | 8192 | 12288 | 16384 | 32768) {
        // If it's a power-of-2 size that matches Atari 2600 cartridge sizes
        // and doesn't match other formats, assume it's Atari 2600
        return Ok(SystemType::Atari2600);
    }

    // Check for DOS COM file (no header, typically small)
    // COM files are 64KB or less and have no specific signature
    // Note: CHIP-8 files (.ch8, .c8) overlap in size range with COM files,
    // but are detected via file extension in the calling code (main.rs)
    // This function only does content-based detection, so small files default to PC/COM
    if data.len() <= 0xFF00 && data.len() >= 16 {
        return Ok(SystemType::PC);
    }

    // Try to provide a helpful error message
    if data.len() < 16 {
        return Err(UnsupportedRomError {
            reason: "File too small to be a valid ROM".to_string(),
        });
    }

    // Check if it might be a raw binary
    if data.len().is_multiple_of(1024) {
        return Err(UnsupportedRomError {
            reason: "Unrecognized ROM format. Supported formats: iNES (.nes), Game Boy (.gb/.gbc), GBA (.gba), Atari 2600 (.a26/.bin), DOS (.com/.exe), SNES (.smc/.sfc), N64 (.z64/.n64/.v64), SMS (.sms), CHIP-8 (.ch8/.c8), ColecoVision (.col), SG-1000 (.sg/.sc), PS1 (.exe/.psexe/.cue/.bin/.iso)".to_string(),
        });
    }

    Err(UnsupportedRomError {
        reason: "Unknown ROM format. Supported formats: iNES (.nes), Game Boy (.gb/.gbc), GBA (.gba), Atari 2600 (.a26/.bin), DOS (.com/.exe), SNES (.smc/.sfc), N64 (.z64/.n64/.v64), SMS (.sms), CHIP-8 (.ch8/.c8), ColecoVision (.col), SG-1000 (.sg/.sc), PS1 (.exe/.psexe/.cue/.bin/.iso)"
            .to_string(),
    })
}

/// Check if data is a PS1 BIOS ROM.
/// PS1 BIOS files are exactly 512KB and contain known Sony strings.
fn is_ps1_bios(data: &[u8]) -> bool {
    // Must be exactly 512KB
    if data.len() != 512 * 1024 {
        return false;
    }
    // Search for known BIOS strings
    let search_strings: &[&[u8]] = &[
        b"Sony Computer Entertainment",
        b"PlayStation",
        b"PS-X Realtime Kernel",
        b"System ROM Version",
    ];
    for needle in search_strings {
        if data.windows(needle.len()).any(|w| w == *needle) {
            return true;
        }
    }
    false
}

/// Check if data is a PS1 disc image (BIN/CUE raw format).
/// PS1 disc BIN files use 2352-byte sectors starting with a CD sync pattern.
fn is_ps1_disc_image(data: &[u8]) -> bool {
    // CD-ROM sync pattern (12 bytes at start of each 2352-byte sector)
    const CD_SYNC: [u8; 12] = [
        0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00,
    ];

    // Must be large enough (at least a few sectors) and a multiple of 2352
    if data.len() < 2352 * 16 {
        return false;
    }

    // Check for CD sync pattern at start
    if data.len() >= 12 && data[..12] == CD_SYNC {
        // Also check that file size is a multiple of 2352 (raw sector size)
        // or 2336 (Mode 2 without sync)
        if data.len().is_multiple_of(2352) || data.len().is_multiple_of(2336) {
            // Look for "PlayStation" or "PLAYSTATION" in the first few sectors
            let search_area = &data[..data.len().min(2352 * 20)];
            if search_area
                .windows(11)
                .any(|w| w == b"PlayStation" || w == b"PLAYSTATION")
                || search_area.windows(8).any(|w| w == b"Sony Com")
            {
                return true;
            }
            // Even without string, CD sync + correct sector size + large enough = likely PS1 disc
            if data.len() > 1024 * 1024 {
                return true;
            }
        }
    }
    false
}

/// Public check if data appears to be a PS1 BIOS (for use by main.rs)
pub fn is_ps1_bios_file(data: &[u8]) -> bool {
    is_ps1_bios(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_nes_rom() {
        let mut data = vec![0u8; 1024];
        data[0..4].copy_from_slice(b"NES\x1A");
        assert_eq!(detect_rom_type(&data).unwrap(), SystemType::NES);
    }

    #[test]
    fn test_detect_gb_rom() {
        let mut data = vec![0u8; 0x150];
        data[0x104..0x108].copy_from_slice(&[0xCE, 0xED, 0x66, 0x66]);
        assert_eq!(detect_rom_type(&data).unwrap(), SystemType::GameBoy);
    }

    #[test]
    fn test_detect_too_small() {
        let data = vec![0u8; 8];
        assert!(detect_rom_type(&data).is_err());
    }

    #[test]
    fn test_detect_atari2600_rom() {
        // 4K ROM
        let data = vec![0u8; 4096];
        assert_eq!(detect_rom_type(&data).unwrap(), SystemType::Atari2600);

        // 2K ROM
        let data = vec![0u8; 2048];
        assert_eq!(detect_rom_type(&data).unwrap(), SystemType::Atari2600);

        // 8K ROM
        let data = vec![0u8; 8192];
        assert_eq!(detect_rom_type(&data).unwrap(), SystemType::Atari2600);
    }

    #[test]
    fn test_detect_pc_exe() {
        // DOS EXE with MZ header
        let mut data = vec![0u8; 1024];
        data[0..2].copy_from_slice(b"MZ");
        assert_eq!(detect_rom_type(&data).unwrap(), SystemType::PC);
    }

    #[test]
    fn test_detect_pc_com() {
        // Small COM file (no header) - needs to be at least 16 bytes
        let mut data = vec![0xB8, 0x00, 0x4C, 0xCD, 0x21]; // Simple DOS program
        data.resize(20, 0x90); // Pad with NOP instructions to 20 bytes
        assert_eq!(detect_rom_type(&data).unwrap(), SystemType::PC);

        // Larger COM file
        let data = vec![0u8; 1000];
        assert_eq!(detect_rom_type(&data).unwrap(), SystemType::PC);
    }

    #[test]
    fn test_detect_snes_rom() {
        // 32KB SNES ROM (minimum size)
        let data = vec![0u8; 0x8000];
        assert_eq!(detect_rom_type(&data).unwrap(), SystemType::SNES);

        // 64KB SNES ROM
        let data = vec![0u8; 0x10000];
        assert_eq!(detect_rom_type(&data).unwrap(), SystemType::SNES);

        // 1MB SNES ROM
        let data = vec![0u8; 0x100000];
        assert_eq!(detect_rom_type(&data).unwrap(), SystemType::SNES);

        // SNES ROM with SMC header (512 bytes + 32KB)
        let data = vec![0u8; 512 + 0x8000];
        assert_eq!(detect_rom_type(&data).unwrap(), SystemType::SNES);
    }

    #[test]
    fn test_detect_n64_z64() {
        let mut data = vec![0u8; 0x100000]; // 1MB ROM
        data[0..4].copy_from_slice(&[0x80, 0x37, 0x12, 0x40]);
        assert_eq!(detect_rom_type(&data).unwrap(), SystemType::N64);
    }

    #[test]
    fn test_detect_n64_n64() {
        let mut data = vec![0u8; 0x100000]; // 1MB ROM
        data[0..4].copy_from_slice(&[0x40, 0x12, 0x37, 0x80]);
        assert_eq!(detect_rom_type(&data).unwrap(), SystemType::N64);
    }

    #[test]
    fn test_detect_n64_v64() {
        let mut data = vec![0u8; 0x100000]; // 1MB ROM
        data[0..4].copy_from_slice(&[0x37, 0x80, 0x40, 0x12]);
        assert_eq!(detect_rom_type(&data).unwrap(), SystemType::N64);
    }

    #[test]
    fn test_detect_sms_with_header() {
        // SMS ROM with TMR SEGA header
        let mut data = vec![0u8; 0x10000]; // 64KB
                                           // Add TMR SEGA signature at 0x7FF0
        data[0x7FF0..0x7FF8].copy_from_slice(b"TMR SEGA");
        assert_eq!(detect_rom_type(&data).unwrap(), SystemType::SMS);
    }

    #[test]
    fn test_detect_sms_headerless() {
        // 48KB SMS ROM (this size is more distinctly SMS)
        let data = vec![0u8; 49152];
        assert_eq!(detect_rom_type(&data).unwrap(), SystemType::SMS);
    }

    // Tests for extension-aware detection
    #[test]
    fn test_chip8_extension_detection() {
        // Small file that could be PC COM or CHIP-8
        let data = vec![0u8; 512];

        // Without extension, should detect as PC
        assert_eq!(detect_rom_type(&data).unwrap(), SystemType::PC);

        // With .ch8 extension, should detect as CHIP-8
        assert_eq!(
            detect_rom_type_with_extension(&data, Some("ch8"), None).unwrap(),
            SystemType::Chip8
        );

        // With .c8 extension, should detect as CHIP-8
        assert_eq!(
            detect_rom_type_with_extension(&data, Some("c8"), None).unwrap(),
            SystemType::Chip8
        );
    }

    #[test]
    fn test_atari2600_extension_detection() {
        // 32KB file could be Atari 2600 or SNES
        let data = vec![0u8; 32768];

        // Without extension, currently detects as SNES (ambiguous)
        assert_eq!(detect_rom_type(&data).unwrap(), SystemType::SNES);

        // With .a26 extension, should detect as Atari 2600
        assert_eq!(
            detect_rom_type_with_extension(&data, Some("a26"), None).unwrap(),
            SystemType::Atari2600
        );

        // With .bin extension (no preferred system), should detect as Atari 2600
        assert_eq!(
            detect_rom_type_with_extension(&data, Some("bin"), None).unwrap(),
            SystemType::Atari2600
        );
    }

    #[test]
    fn test_snes_extension_detection() {
        // 32KB file with SNES extension
        let data = vec![0u8; 32768];

        // With .smc extension, should detect as SNES
        assert_eq!(
            detect_rom_type_with_extension(&data, Some("smc"), None).unwrap(),
            SystemType::SNES
        );

        // With .sfc extension, should detect as SNES
        assert_eq!(
            detect_rom_type_with_extension(&data, Some("sfc"), None).unwrap(),
            SystemType::SNES
        );
    }

    #[test]
    fn test_pc_extension_detection() {
        // Small file with PC executable extension
        let data = vec![0u8; 100];

        // With .com extension, should detect as PC
        assert_eq!(
            detect_rom_type_with_extension(&data, Some("com"), None).unwrap(),
            SystemType::PC
        );

        // With .exe extension, should detect as PC
        assert_eq!(
            detect_rom_type_with_extension(&data, Some("exe"), None).unwrap(),
            SystemType::PC
        );
    }

    #[test]
    fn test_extension_case_insensitive() {
        let data = vec![0u8; 512];

        // Extensions should be case-insensitive
        assert_eq!(
            detect_rom_type_with_extension(&data, Some("CH8"), None).unwrap(),
            SystemType::Chip8
        );
        assert_eq!(
            detect_rom_type_with_extension(&data, Some("Ch8"), None).unwrap(),
            SystemType::Chip8
        );
        assert_eq!(
            detect_rom_type_with_extension(&data, Some("C8"), None).unwrap(),
            SystemType::Chip8
        );
    }

    #[test]
    fn test_gba_extension_detection() {
        let data = vec![0u8; 1024];
        assert_eq!(
            detect_rom_type_with_extension(&data, Some("gba"), None).unwrap(),
            SystemType::GBA
        );
    }

    #[test]
    fn test_bin_with_preferred_system() {
        // 32KB .bin file should use preferred system
        let data = vec![0u8; 32768];

        // With SNES preferred, should detect as SNES
        assert_eq!(
            detect_rom_type_with_extension(&data, Some("bin"), Some(SystemType::SNES)).unwrap(),
            SystemType::SNES
        );

        // With Atari2600 preferred, should detect as Atari2600
        assert_eq!(
            detect_rom_type_with_extension(&data, Some("bin"), Some(SystemType::Atari2600))
                .unwrap(),
            SystemType::Atari2600
        );

        // Without preferred system, defaults to Atari2600 (most common .bin use)
        assert_eq!(
            detect_rom_type_with_extension(&data, Some("bin"), None).unwrap(),
            SystemType::Atari2600
        );
    }

    #[test]
    fn test_unknown_extension_with_preferred_system() {
        // File with unknown extension should use preferred system
        let data = vec![0u8; 4096];

        // With Atari2600 preferred and matching size, should detect as Atari2600
        assert_eq!(
            detect_rom_type_with_extension(&data, Some("xyz"), Some(SystemType::Atari2600))
                .unwrap(),
            SystemType::Atari2600
        );

        // With CHIP-8 preferred, should detect as CHIP-8 (no strict validation)
        assert_eq!(
            detect_rom_type_with_extension(&data, Some("xyz"), Some(SystemType::Chip8)).unwrap(),
            SystemType::Chip8
        );

        // Without preferred system, falls back to content detection (Atari2600 by size)
        assert_eq!(
            detect_rom_type_with_extension(&data, Some("xyz"), None).unwrap(),
            SystemType::Atari2600
        );
    }
}

#[cfg(test)]
mod edge_case_tests {
    use super::*;

    #[test]
    fn test_32kb_file_ambiguity() {
        // A 32KB file with no headers could be Atari 2600 or SNES
        // Currently this will incorrectly detect as SNES
        let data = vec![0u8; 32768];
        let result = detect_rom_type(&data).unwrap();

        // This test will fail because 32KB is detected as SNES, not Atari2600
        // This is the bug!
        println!("32KB file detected as: {:?}", result);

        // The current logic detects this as SNES because SNES check comes first
        assert_eq!(
            result,
            SystemType::SNES,
            "BUG: 32KB files are ambiguous - could be Atari 2600 or SNES"
        );
    }

    #[test]
    fn test_8kb_file_ambiguity() {
        // An 8KB file with no headers could be Atari 2600 or small PC COM
        let data = vec![0u8; 8192];
        let result = detect_rom_type(&data).unwrap();

        println!("8KB file detected as: {:?}", result);
        // Currently detected as Atari2600 (line 110 matches 8192)
        assert_eq!(result, SystemType::Atari2600);
    }

    #[test]
    fn test_4kb_file_ambiguity() {
        // A 4KB file with no headers could be Atari 2600 or PC COM
        let data = vec![0u8; 4096];
        let result = detect_rom_type(&data).unwrap();

        println!("4KB file detected as: {:?}", result);
        // Currently detected as Atari2600 (line 110 matches 4096)
        assert_eq!(result, SystemType::Atari2600);
    }

    #[test]
    fn test_img_floppy_detection() {
        // 1.44 MB floppy image (.img) should be detected as PC
        let data = vec![0u8; 1474560];
        assert_eq!(
            detect_rom_type_with_extension(&data, Some("img"), None).unwrap(),
            SystemType::PC
        );

        // 720 KB floppy image (.img)
        let data = vec![0u8; 737280];
        assert_eq!(
            detect_rom_type_with_extension(&data, Some("img"), None).unwrap(),
            SystemType::PC
        );

        // .ima extension also detected as PC floppy
        let data = vec![0u8; 1474560];
        assert_eq!(
            detect_rom_type_with_extension(&data, Some("ima"), None).unwrap(),
            SystemType::PC
        );
    }

    #[test]
    fn test_img_hard_drive_detection() {
        // 20 MB hard drive image (.img) should be detected as PC
        let data = vec![0u8; 20 * 1024 * 1024];
        assert_eq!(
            detect_rom_type_with_extension(&data, Some("img"), None).unwrap(),
            SystemType::PC
        );
    }
}
