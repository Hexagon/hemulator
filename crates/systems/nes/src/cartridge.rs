use std::fs::File;
use std::io::Read;
use std::path::Path;
use emu_core::apu::TimingMode;

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
        let header = &data[0..16];
        if &header[0..4] != b"NES\x1A" {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Not iNES file",
            ));
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
