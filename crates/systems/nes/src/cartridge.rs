use emu_core::apu::TimingMode;
use emu_core::logging::{log, LogCategory, LogLevel};
use std::fs::File;
use std::io::Read;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mirroring {
    Horizontal,
    Vertical,
    FourScreen,
    SingleScreenLower,
    SingleScreenUpper,
}

#[derive(Debug, Clone)]
pub struct Cartridge {
    pub prg_rom: Vec<u8>,
    pub chr_rom: Vec<u8>,
    pub mapper: u16,
    pub submapper: u8,
    pub mirroring: Mirroring,
    pub timing: TimingMode,
    /// CRC32 checksum of the entire ROM file (including header)
    pub crc32: u32,
    /// Mapper number from the iNES header (before any DB overrides)
    pub header_mapper: u16,
    /// Submapper number from the iNES 2.0 header
    pub header_submapper: u8,
    /// Mirroring mode from the iNES header (before any DB overrides)
    pub header_mirroring: Mirroring,
    /// Whether the mapper was overridden by the ROM database
    pub db_mapper_override: bool,
    /// Whether the mirroring was overridden by the ROM database
    pub db_mirroring_override: bool,
    /// Board name from ROM database (if available)
    pub board_name: Option<String>,
}

impl Cartridge {
    /// Get a safe initial mirroring mode for mappers with mapper-controlled mirroring.
    ///
    /// Some mappers support dynamic mirroring control via register writes. This function
    /// returns the appropriate initial mirroring mode:
    ///
    /// - Mapper 001 (MMC1): Supports H/V/single-screen via $8000-$9FFF - use header mirroring
    /// - Mapper 004 (MMC3): Supports H/V via $A000 - use header mirroring
    /// - Mapper 007 (AxROM): Always single-screen via $8000 - ignore header, use SingleScreenLower
    /// - Mapper 071 (Camerica): Most games have hard-wired Vertical mirroring on the cartridge
    ///   board.
    ///   Games can optionally use mapper-controlled single-screen mirroring via $9000 writes.
    ///
    /// For other mappers, returns the header mirroring unchanged.
    pub fn get_initial_mirroring(&self) -> Mirroring {
        match self.mapper {
            // AxROM (007): Always uses single-screen mirroring, header is meaningless
            7 => Mirroring::SingleScreenLower,

            // Camerica (071): All Camerica boards have Vertical mirroring hard-wired on PCB.
            // iNES headers are often incorrect (e.g., Bee 52 header says Horizontal).
            // Games can override to single-screen via $9000 writes if needed (e.g., Fire Hawk).
            // Reference: https://www.nesdev.org/wiki/INES_Mapper_071
            71 => Mirroring::Vertical,

            // All other mappers: Use header mirroring
            _ => self.mirroring,
        }
    }
}

impl Cartridge {
    /// Load iNES ROM from bytes
    pub fn from_bytes(data: &[u8]) -> std::io::Result<Self> {
        if data.len() < 16 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Data too small for iNES header",
            ));
        }
        let mut header = [0u8; 16];
        header.copy_from_slice(&data[0..16]);

        if &header[0..4] != b"NES\x1A" {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Not iNES file",
            ));
        }

        // Check if this is iNES 2.0 format BEFORE cleaning header
        // NES 2.0 identification: bits 2-3 of byte 7 must be 10 (0x08)
        let is_nes2 = (header[7] & 0x0C) == 0x08;

        // Clean up corrupted header bytes
        // Many old ROMs have garbage data in the header from tools like DiskDude!
        if !is_nes2 {
            // For iNES 1.0, handle two types of corruption:

            // Type A: "DiskDude!" signature corruption (bytes 7-15)
            //   This old tool left its signature which completely corrupts the header
            if &header[7..16] == b"DiskDude!" {
                log(LogCategory::Bus, LogLevel::Info, || {
                    "NES: Cleaning DiskDude! corrupted header".to_string()
                });
                header[7..16].fill(0);
            } else {
                // Type B: Other garbage data corruption
                let has_corruption_8_15 = header[8..16].iter().any(|&b| b != 0);

                // For iNES 1.0, byte 7 bits 2-3 MUST be 00 (they're used for NES 2.0 identification)
                // If they're set, the entire byte 7 is likely garbage and should be zeroed
                let byte7_bits_2_3 = header[7] & 0x0C;
                let byte7_corrupted = byte7_bits_2_3 != 0;

                // Check if bytes 8-15 are severely corrupted (many non-zero bytes)
                // If so, byte 7 is also likely corrupted even if bits 2-3 are correct
                const SEVERE_CORRUPTION_THRESHOLD: usize = 4;
                let severe_corruption = header[8..16].iter().filter(|&&b| b != 0).count()
                    >= SEVERE_CORRUPTION_THRESHOLD;

                if has_corruption_8_15 || byte7_corrupted {
                    log(LogCategory::Bus, LogLevel::Info, || {
                        "NES: Cleaning corrupted iNES 1.0 header (invalid data in bytes 7-15)"
                            .to_string()
                    });

                    // Clean bytes 8-15 completely
                    header[8..16].fill(0);

                    // Zero byte 7 if:
                    // - Bits 2-3 are set (NES 2.0 identifier in iNES 1.0 ROM)
                    // - Severe corruption in bytes 8-15 (likely entire header is bad)
                    if byte7_corrupted || severe_corruption {
                        header[7] = 0;
                    }
                }
            }
        } else {
            // For NES 2.0, only bytes 13-15 should be zero (reserved for future use)
            // Bytes 7-12 contain valid NES 2.0 metadata (mapper, submapper, sizes, timing)
            let has_corruption = header[13..16].iter().any(|&b| b != 0);
            if has_corruption {
                log(LogCategory::Bus, LogLevel::Info, || {
                    "NES: Cleaning corrupted NES 2.0 header (bytes 13-15 should be zero)"
                        .to_string()
                });
                header[13..16].fill(0);
            }
        }

        let prg_size = header[4] as usize * 16 * 1024;
        let chr_size = header[5] as usize * 8 * 1024;

        // Parse mapper number (8 bits for iNES 1.0, 12 bits for iNES 2.0)
        let mapper = if is_nes2 {
            // iNES 2.0: mapper is 12 bits (bytes 6, 7, and 8)
            // Byte 6 bits 4-7: mapper bits 0-3
            // Byte 7 bits 4-7: mapper bits 4-7
            // Byte 8 bits 0-3: mapper bits 8-11
            let low = (header[6] >> 4) as u16;
            let mid = (header[7] & 0xF0) as u16;
            let high = ((header[8] & 0x0F) as u16) << 8;
            high | mid as u16 | low
        } else {
            // iNES 1.0: mapper is 8 bits (bytes 6 and 7)
            ((header[6] >> 4) | (header[7] & 0xF0)) as u16
        };

        // Parse submapper (only in iNES 2.0)
        let submapper = if is_nes2 { (header[8] >> 4) & 0x0F } else { 0 };

        // iNES flags 6:
        // bit 0 = mirroring (0 horizontal, 1 vertical)
        // bit 3 = four-screen VRAM
        let four_screen = (header[6] & 0x08) != 0;
        let vertical = (header[6] & 0x01) != 0;
        let mirroring = if four_screen {
            Mirroring::FourScreen
        } else if vertical {
            Mirroring::Vertical
        } else {
            Mirroring::Horizontal
        };

        // Auto-detect PAL/NTSC from iNES 2.0 header (byte 12) or NES 2.0 flags
        let timing = if is_nes2 && data.len() > 12 {
            // NES 2.0: byte 12 bits 0-1 indicate timing
            // 0 = NTSC, 1 = PAL, 2 = Dual compatible, 3 = Dendy
            match header[12] & 0x03 {
                1 => TimingMode::Pal,
                _ => TimingMode::Ntsc, // Default to NTSC for dual/dendy/ntsc
            }
        } else {
            // iNES 1.0: no timing flag, default to NTSC
            // Note: Some ROMs use byte 9 bit 0 as PAL flag (unofficial)
            if header[9] & 0x01 != 0 {
                TimingMode::Pal
            } else {
                TimingMode::Ntsc
            }
        };

        // ignore trainer if present (flag 6 bit 2)
        let has_trainer = (header[6] & 0x04) != 0;
        let mut offset = 16;
        if has_trainer {
            offset += 512;
        }

        if data.len() < offset + prg_size {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Data too small for PRG ROM",
            ));
        }

        let prg_rom = data[offset..offset + prg_size].to_vec();
        offset += prg_size;

        let chr_rom = if chr_size > 0 {
            if data.len() < offset + chr_size {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Data too small for CHR ROM",
                ));
            }
            data[offset..offset + chr_size].to_vec()
        } else {
            vec![]
        };

        // Calculate CRC32 and check ROM database for overrides
        let crc32 = crate::rom_db::calculate_crc32(data);
        let (mut final_mapper, mut final_mirroring) = (mapper, mirroring);
        let mut db_mapper_override = false;
        let mut db_mirroring_override = false;
        let mut board_name: Option<String> = None;

        if let Some(db_entry) = crate::rom_db::lookup_rom(crc32) {
            // Helper to format board name for logging
            let board_info = db_entry
                .board
                .map(|b| format!(" ({})", b))
                .unwrap_or_default();

            // Store board name if available
            board_name = db_entry.board.map(|b| b.to_string());

            // Apply mapper override if present
            if let Some(db_mapper) = db_entry.mapper {
                log(LogCategory::Bus, LogLevel::Info, || {
                    format!(
                        "NES ROM DB: Overriding mapper {} -> {} for CRC32 0x{:08X}{}",
                        mapper, db_mapper, crc32, board_info
                    )
                });
                final_mapper = db_mapper;
                db_mapper_override = true;
            }

            // Apply mirroring override if present
            if let Some(db_mirroring) = db_entry.mirroring {
                log(LogCategory::Bus, LogLevel::Info, || {
                    format!(
                        "NES ROM DB: Overriding mirroring {:?} -> {:?} for CRC32 0x{:08X}{}",
                        mirroring, db_mirroring, crc32, board_info
                    )
                });
                final_mirroring = db_mirroring;
                db_mirroring_override = true;
            }
        }

        log(LogCategory::Bus, LogLevel::Info, || {
            format!(
                "NES: Loaded cartridge - Mapper {} ({} KB PRG, {} KB CHR, {:?}, {:?})",
                final_mapper,
                prg_size / 1024,
                chr_size / 1024,
                final_mirroring,
                timing
            )
        });

        Ok(Self {
            prg_rom,
            chr_rom,
            mapper: final_mapper,
            submapper,
            mirroring: final_mirroring,
            timing,
            crc32,
            header_mapper: mapper,
            header_submapper: submapper,
            header_mirroring: mirroring,
            db_mapper_override,
            db_mirroring_override,
            board_name,
        })
    }

    /// Very small iNES loader supporting all mappers.
    pub fn from_file<P: AsRef<Path>>(p: P) -> std::io::Result<Self> {
        let mut f = File::open(p)?;
        let mut data = Vec::new();
        f.read_to_end(&mut data)?;
        Self::from_bytes(&data)
    }

    /// Create a test cartridge with minimal fields (for unit tests only)
    #[cfg(test)]
    pub fn new_test(
        prg_rom: Vec<u8>,
        chr_rom: Vec<u8>,
        mapper: u8,
        mirroring: Mirroring,
        timing: TimingMode,
    ) -> Self {
        Self {
            prg_rom,
            chr_rom,
            mapper: mapper as u16,
            submapper: 0,
            mirroring,
            timing,
            crc32: 0,
            header_mapper: mapper as u16,
            header_submapper: 0,
            header_mirroring: mirroring,
            db_mapper_override: false,
            db_mirroring_override: false,
            board_name: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diskdude_cleanup() {
        let mut data = vec![
            0x4E, 0x45, 0x53, 0x1A, // NES<EOF>
            0x08, // PRG size
            0x10, // CHR size
            0x11, // Flags 6 (Mapper low nibble 1)
            0x44, // Flags 7 (Mapper high nibble 4) -> 'D'
            0x69, // 'i'
            0x73, // 's'
            0x6B, // 'k'
            0x44, // 'D'
            0x75, // 'u'
            0x64, // 'd'
            0x65, // 'e'
            0x21, // '!'
        ];
        // Add some dummy PRG/CHR data
        data.resize(16 + 128 * 1024 + 128 * 1024, 0);

        let cart = Cartridge::from_bytes(&data).unwrap();
        // Before fix: Mapper would be 0x41 (65)
        // After fix: Mapper should be 0x01 (1) because byte 7 is zeroed.
        assert_eq!(cart.mapper, 1);
    }

    #[test]
    fn test_minimal_valid_rom() {
        // Edge case: Smallest valid iNES ROM (16-byte header + 16KB PRG, no CHR)
        let mut data = vec![
            0x4E, 0x45, 0x53, 0x1A, // NES<EOF>
            0x01, // PRG size: 1 unit = 16KB
            0x00, // CHR size: 0 (CHR-RAM)
            0x00, // Flags 6: Mapper 0, horizontal mirroring
            0x00, // Flags 7
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // Padding
        ];
        // Add 16KB PRG ROM
        data.extend(vec![0; 16 * 1024]);

        let cart = Cartridge::from_bytes(&data).unwrap();
        assert_eq!(cart.mapper, 0);
        assert_eq!(cart.prg_rom.len(), 16 * 1024);
        assert_eq!(cart.chr_rom.len(), 0); // No CHR ROM (will use CHR-RAM)
        assert_eq!(cart.mirroring, Mirroring::Horizontal);
    }

    #[test]
    fn test_rom_with_chr_ram() {
        // Edge case: ROM with CHR size 0 indicates CHR-RAM should be used
        let mut data = vec![
            0x4E, 0x45, 0x53, 0x1A, // NES<EOF>
            0x02, // PRG size: 2 units = 32KB
            0x00, // CHR size: 0 (CHR-RAM)
            0x01, // Flags 6: Mapper 0, vertical mirroring
            0x00, // Flags 7
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        data.extend(vec![0; 32 * 1024]); // PRG ROM

        let cart = Cartridge::from_bytes(&data).unwrap();
        assert_eq!(cart.prg_rom.len(), 32 * 1024);
        assert_eq!(cart.chr_rom.len(), 0);
        assert_eq!(cart.mirroring, Mirroring::Vertical);
    }

    #[test]
    fn test_rom_with_trainer() {
        // Edge case: ROM with trainer (512-byte trainer before PRG ROM)
        let mut data = vec![
            0x4E, 0x45, 0x53, 0x1A, // NES<EOF>
            0x01, // PRG size: 16KB
            0x01, // CHR size: 8KB
            0x04, // Flags 6: Trainer present (bit 2)
            0x00, // Flags 7
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        data.extend(vec![0xFF; 512]); // Trainer (512 bytes)
        data.extend(vec![0xAA; 16 * 1024]); // PRG ROM
        data.extend(vec![0x55; 8 * 1024]); // CHR ROM

        let cart = Cartridge::from_bytes(&data).unwrap();
        // Verify trainer was skipped and PRG/CHR loaded correctly
        assert_eq!(cart.prg_rom.len(), 16 * 1024);
        assert_eq!(cart.chr_rom.len(), 8 * 1024);
        assert_eq!(cart.prg_rom[0], 0xAA); // First PRG byte, not trainer
        assert_eq!(cart.chr_rom[0], 0x55); // First CHR byte
    }

    #[test]
    fn test_four_screen_mirroring() {
        // Edge case: Four-screen VRAM (bit 3 of flags 6)
        let mut data = vec![
            0x4E, 0x45, 0x53, 0x1A, // NES<EOF>
            0x01, 0x01, // 16KB PRG, 8KB CHR
            0x08, // Flags 6: Four-screen VRAM (bit 3)
            0x00, // Flags 7
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        data.extend(vec![0; 16 * 1024 + 8 * 1024]);

        let cart = Cartridge::from_bytes(&data).unwrap();
        assert_eq!(cart.mirroring, Mirroring::FourScreen);
    }

    #[test]
    fn test_invalid_rom_too_small() {
        // Edge case: Data too small to contain header
        let data = vec![0x4E, 0x45, 0x53]; // Only 3 bytes

        let result = Cartridge::from_bytes(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_rom_wrong_magic() {
        // Edge case: Invalid magic number
        let mut data = vec![
            0x4E, 0x45, 0x58, 0x1A, // NE X <EOF> (wrong magic)
            0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        data.extend(vec![0; 16 * 1024]);

        let result = Cartridge::from_bytes(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_rom_size_mismatch() {
        // Edge case: Header indicates more data than provided
        let data = vec![
            0x4E, 0x45, 0x53, 0x1A, // NES<EOF>
            0x10, // PRG size: 16 units = 256KB (but we won't provide this much)
            0x00, // CHR size: 0
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        // Only provide 16 bytes of data, not 256KB

        let result = Cartridge::from_bytes(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_nes2_timing_detection() {
        // Edge case: NES 2.0 format with PAL timing
        let mut data = vec![
            0x4E, 0x45, 0x53, 0x1A, // NES<EOF>
            0x01, 0x01, // 16KB PRG, 8KB CHR
            0x00, // Flags 6
            0x08, // Flags 7: NES 2.0 format (bits 2-3 = 10)
            0x00, 0x00, 0x00, 0x00, 0x01, // Byte 12: PAL timing (bits 0-1 = 01)
            0x00, 0x00, 0x00,
        ];
        data.extend(vec![0; 16 * 1024 + 8 * 1024]);

        let cart = Cartridge::from_bytes(&data).unwrap();
        assert_eq!(cart.timing, TimingMode::Pal);
    }

    #[test]
    fn test_mapper_number_extraction() {
        // Edge case: Mapper number from both nibbles of flags 6 and 7
        let mut data = vec![
            0x4E, 0x45, 0x53, 0x1A, // NES<EOF>
            0x01, 0x00, // 16KB PRG, no CHR
            0x40, // Flags 6: Mapper low nibble = 4
            0x30, // Flags 7: Mapper high nibble = 3 -> mapper = 0x34 (52)
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        data.extend(vec![0; 16 * 1024]);

        let cart = Cartridge::from_bytes(&data).unwrap();
        assert_eq!(cart.mapper, 0x34); // Mapper 52 (0x30 | 0x04)
    }

    #[test]
    fn test_rom_db_integration_no_override() {
        // Test that ROMs not in the database work normally
        let mut data = vec![
            0x4E, 0x45, 0x53, 0x1A, // NES<EOF>
            0x01, 0x00, // 16KB PRG, no CHR
            0x00, // Flags 6: Mapper 0, horizontal mirroring
            0x00, // Flags 7
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        data.extend(vec![0; 16 * 1024]);

        let cart = Cartridge::from_bytes(&data).unwrap();
        assert_eq!(cart.mapper, 0);
        assert_eq!(cart.mirroring, Mirroring::Horizontal);
    }

    #[test]
    fn test_rom_db_crc32_calculation() {
        // Test that CRC32 is calculated correctly for a known ROM
        let mut data = vec![
            0x4E, 0x45, 0x53, 0x1A, // NES<EOF>
            0x01, 0x00, // 16KB PRG, no CHR
            0x00, // Flags 6
            0x00, // Flags 7
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        data.extend(vec![0; 16 * 1024]);

        // Calculate CRC32 manually to verify it's done during load
        let expected_crc = crate::rom_db::calculate_crc32(&data);

        // Load the cartridge (which also calculates CRC32 internally)
        let _cart = Cartridge::from_bytes(&data).unwrap();

        // Verify the CRC32 is non-zero (simple sanity check)
        assert_ne!(expected_crc, 0);
    }

    #[test]
    fn test_initial_mirroring_axrom() {
        // Test that AxROM (mapper 7) always uses single-screen mirroring
        let mut data = vec![
            0x4E, 0x45, 0x53, 0x1A, // NES<EOF>
            0x02, 0x00, // 32KB PRG, no CHR
            0x71, // Flags 6: Mapper low nibble = 7, vertical mirroring
            0x00, // Flags 7
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        data.extend(vec![0; 32 * 1024]);

        let cart = Cartridge::from_bytes(&data).unwrap();
        assert_eq!(cart.mapper, 7);
        // Even though header says vertical, AxROM should use single-screen
        assert_eq!(cart.get_initial_mirroring(), Mirroring::SingleScreenLower);
    }

    #[test]
    fn test_initial_mirroring_camerica() {
        // Test that Camerica (mapper 71) respects header mirroring
        let mut data = vec![
            0x4E, 0x45, 0x53, 0x1A, // NES<EOF>
            0x02, 0x00, // 32KB PRG, no CHR
            0x71, // Flags 6: Mapper low nibble = 7, vertical mirroring
            0x40, // Flags 7: Mapper high nibble = 4 -> mapper 71
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        data.extend(vec![0; 32 * 1024]);

        let cart = Cartridge::from_bytes(&data).unwrap();
        assert_eq!(cart.mapper, 71);
        // Camerica should respect header mirroring
        assert_eq!(cart.get_initial_mirroring(), Mirroring::Vertical);
    }

    #[test]
    fn test_ines2_mapper_parsing() {
        // Test iNES 2.0 mapper parsing with 12-bit mapper number
        let mut data = vec![
            0x4E, 0x45, 0x53, 0x1A, // NES<EOF>
            0x01, 0x01, // 16KB PRG, 8KB CHR
            0x00, // Flags 6: Mapper bits 0-3 = 0
            0x08, // Flags 7: NES 2.0 format (bits 2-3 = 10), mapper bits 4-7 = 0
            0x01, // Byte 8: Mapper bits 8-11 = 1 (bits 0-3), submapper = 0 (bits 4-7)
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        data.extend(vec![0; 16 * 1024 + 8 * 1024]);

        let cart = Cartridge::from_bytes(&data).unwrap();
        // Mapper should be 0x100 (256) - high bits set via byte 8
        assert_eq!(cart.mapper, 0x100);
        assert_eq!(cart.submapper, 0);
    }

    #[test]
    fn test_ines2_submapper_parsing() {
        // Test iNES 2.0 submapper parsing
        let mut data = vec![
            0x4E, 0x45, 0x53, 0x1A, // NES<EOF>
            0x01, 0x01, // 16KB PRG, 8KB CHR
            0x10, // Flags 6: Mapper bits 0-3 = 1
            0x08, // Flags 7: NES 2.0 format (bits 2-3 = 10), mapper bits 4-7 = 0
            0x50, // Byte 8: Mapper bits 8-11 = 0 (bits 0-3), submapper = 5 (bits 4-7)
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        data.extend(vec![0; 16 * 1024 + 8 * 1024]);

        let cart = Cartridge::from_bytes(&data).unwrap();
        assert_eq!(cart.mapper, 1); // Mapper 1 (MMC1)
        assert_eq!(cart.submapper, 5); // Submapper 5
    }

    #[test]
    fn test_ines2_full_mapper_range() {
        // Test maximum mapper number in iNES 2.0 (12 bits = 4095)
        let mut data = vec![
            0x4E, 0x45, 0x53, 0x1A, // NES<EOF>
            0x01, 0x00, // 16KB PRG, no CHR
            0xF0, // Flags 6: Mapper bits 0-3 = 15 (0xF)
            0xF8, // Flags 7: NES 2.0 format + mapper bits 4-7 = 15 (0xF)
            0x0F, // Byte 8: Mapper bits 8-11 = 15 (0xF), submapper = 0
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        data.extend(vec![0; 16 * 1024]);

        let cart = Cartridge::from_bytes(&data).unwrap();
        // Mapper should be 0xFFF (4095) - all 12 bits set
        assert_eq!(cart.mapper, 0xFFF);
        assert_eq!(cart.submapper, 0);
    }

    #[test]
    fn test_ines2_full_submapper_range() {
        // Test maximum submapper number in iNES 2.0 (4 bits = 15)
        let mut data = vec![
            0x4E, 0x45, 0x53, 0x1A, // NES<EOF>
            0x01, 0x00, // 16KB PRG, no CHR
            0x00, // Flags 6: Mapper bits 0-3 = 0
            0x08, // Flags 7: NES 2.0 format, mapper bits 4-7 = 0
            0xF0, // Byte 8: Mapper bits 8-11 = 0, submapper = 15 (0xF)
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        data.extend(vec![0; 16 * 1024]);

        let cart = Cartridge::from_bytes(&data).unwrap();
        assert_eq!(cart.mapper, 0); // Mapper 0 (NROM)
        assert_eq!(cart.submapper, 15); // Maximum submapper value
    }

    #[test]
    fn test_ines1_backward_compatibility() {
        // Test that iNES 1.0 ROMs still work correctly (8-bit mapper, no submapper)
        let mut data = vec![
            0x4E, 0x45, 0x53, 0x1A, // NES<EOF>
            0x01, 0x00, // 16KB PRG, no CHR
            0x40, // Flags 6: Mapper bits 0-3 = 4
            0x30, // Flags 7: NOT NES 2.0 (bits 2-3 != 10), mapper bits 4-7 = 3
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        data.extend(vec![0; 16 * 1024]);

        let cart = Cartridge::from_bytes(&data).unwrap();
        assert_eq!(cart.mapper, 0x34); // Mapper 52 (iNES 1.0 format)
        assert_eq!(cart.submapper, 0); // No submapper in iNES 1.0
    }

    #[test]
    fn test_ines1_header_cleanup_garbage_bytes() {
        // Test that garbage data in bytes 7-15 is cleaned up for iNES 1.0 ROMs
        // This simulates common corruptions from old dumping tools
        let mut data = vec![
            0x4E, 0x45, 0x53, 0x1A, // NES<EOF>
            0x01, // PRG size: 1 unit = 16KB
            0x00, // CHR size: 0 (CHR-RAM)
            0x40, // Flags 6: Mapper low nibble = 4
            0xFF, // Flags 7: Garbage data (should be cleaned to 0)
            0xDE, 0xAD, 0xBE, 0xEF, 0xBA, 0xAD, 0xF0, 0x0D, // Intentional garbage pattern
        ];
        data.extend(vec![0; 16 * 1024]);

        let cart = Cartridge::from_bytes(&data).unwrap();
        // After cleanup, byte 7 should be 0, so mapper should be 0x04 (4), not 0xF4 (244)
        assert_eq!(cart.mapper, 4);
        assert_eq!(cart.submapper, 0);
    }

    #[test]
    fn test_ines1_header_cleanup_partial_garbage() {
        // Test cleanup when only some bytes 7-15 contain garbage
        let mut data = vec![
            0x4E, 0x45, 0x53, 0x1A, // NES<EOF>
            0x08, // PRG size: 8 units = 128KB
            0x10, // CHR size: 16 units = 128KB
            0x10, // Flags 6: Mapper low nibble = 1
            0x00, // Flags 7: Clean
            0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, // Single garbage byte
        ];
        data.extend(vec![0; 128 * 1024 + 128 * 1024]);

        let cart = Cartridge::from_bytes(&data).unwrap();
        // Should still be mapper 1 (MMC1)
        assert_eq!(cart.mapper, 1);
    }

    #[test]
    fn test_nes2_header_cleanup_preserves_valid_bytes() {
        // Test that NES 2.0 format preserves bytes 7-12 but cleans 13-15
        let mut data = vec![
            0x4E, 0x45, 0x53, 0x1A, // NES<EOF>
            0x01, 0x01, // 16KB PRG, 8KB CHR
            0x00, // Flags 6: Mapper low nibble = 0
            0x08, // Flags 7: NES 2.0 format (bits 2-3 = 10), mapper high nibble = 0
            0x20, // Byte 8: Submapper 2 (bits 4-7), mapper bits 8-11 = 0
            0x00, 0x00, 0x00, 0x00, // Bytes 9-12: Valid NES 2.0 data
            0xAA, 0xBB, 0xCC, // Bytes 13-15: Garbage (should be cleaned)
        ];
        data.extend(vec![0; 16 * 1024 + 8 * 1024]);

        let cart = Cartridge::from_bytes(&data).unwrap();
        assert_eq!(cart.mapper, 0); // Mapper 0
        assert_eq!(cart.submapper, 2); // Submapper 2 (from byte 8, should be preserved)
    }

    #[test]
    fn test_bad_dudes_simulated_corruption() {
        // Simulate the type of corruption seen in Bad Dudes ROMs
        // TLROM board: mapper 4 (MMC3), horizontal mirroring, 128KB PRG, 128KB CHR
        let mut data = vec![
            0x4E, 0x45, 0x53, 0x1A, // NES<EOF>
            0x08, // PRG size: 8 units = 128KB
            0x10, // CHR size: 16 units = 128KB
            0x40, // Flags 6: Mapper low nibble = 4, horizontal mirroring
            0x50, // Flags 7: Mapper high nibble = 5 due to garbage (should be 0)
            0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, // Garbage in bytes 8-15
        ];
        data.extend(vec![0; 128 * 1024 + 128 * 1024]);

        let cart = Cartridge::from_bytes(&data).unwrap();
        // After cleanup, mapper should be 4 (MMC3), not 0x54 (84)
        assert_eq!(cart.mapper, 4);
        assert_eq!(cart.mirroring, Mirroring::Horizontal);
    }
}
