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
    pub mapper: u8,
    pub mirroring: Mirroring,
    pub timing: TimingMode,
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

        // Fix DiskDude! corruption
        if &header[7..16] == b"DiskDude!" {
            header[7..16].fill(0);
        }

        let prg_size = header[4] as usize * 16 * 1024;
        let chr_size = header[5] as usize * 8 * 1024;
        let mapper = (header[6] >> 4) | (header[7] & 0xF0);

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
        // If byte 7 & 0x0C == 0x08, it's NES 2.0 format
        let is_nes2 = (header[7] & 0x0C) == 0x08;
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

        log(LogCategory::Bus, LogLevel::Info, || {
            format!(
                "NES: Loaded cartridge - Mapper {} ({} KB PRG, {} KB CHR, {:?}, {:?})",
                mapper,
                prg_size / 1024,
                chr_size / 1024,
                mirroring,
                timing
            )
        });

        Ok(Self {
            prg_rom,
            chr_rom,
            mapper,
            mirroring,
            timing,
        })
    }

    /// Very small iNES loader supporting all mappers.
    pub fn from_file<P: AsRef<Path>>(p: P) -> std::io::Result<Self> {
        let mut f = File::open(p)?;
        let mut header = [0u8; 16];
        f.read_exact(&mut header)?;
        if &header[0..4] != b"NES\x1A" {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Not iNES file",
            ));
        }

        // Fix DiskDude! corruption
        if &header[7..16] == b"DiskDude!" {
            header[7..16].fill(0);
        }

        let prg_size = header[4] as usize * 16 * 1024;
        let chr_size = header[5] as usize * 8 * 1024;
        let mapper = (header[6] >> 4) | (header[7] & 0xF0);

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
        let is_nes2 = (header[7] & 0x0C) == 0x08;
        let timing = if is_nes2 {
            // NES 2.0: byte 12 bits 0-1 indicate timing (always present in 16-byte header)
            match header[12] & 0x03 {
                1 => TimingMode::Pal,
                _ => TimingMode::Ntsc,
            }
        } else {
            // iNES 1.0: check unofficial PAL flag in byte 9
            if header[9] & 0x01 != 0 {
                TimingMode::Pal
            } else {
                TimingMode::Ntsc
            }
        };

        // ignore trainer if present (flag 6 bit 2)
        let has_trainer = (header[6] & 0x04) != 0;
        if has_trainer {
            let mut _trainer = vec![0u8; 512];
            f.read_exact(&mut _trainer)?;
        }

        let mut prg_rom = vec![0u8; prg_size];
        f.read_exact(&mut prg_rom)?;
        let mut chr_rom = vec![];
        if chr_size > 0 {
            chr_rom = vec![0u8; chr_size];
            f.read_exact(&mut chr_rom)?;
        }

        Ok(Self {
            prg_rom,
            chr_rom,
            mapper,
            mirroring,
            timing,
        })
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
}
