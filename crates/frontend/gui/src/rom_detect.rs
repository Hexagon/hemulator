/// ROM detection and system selection
use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::upper_case_acronyms)]
pub enum SystemType {
    NES,
    GameBoy,
    Atari2600,
    PC,
    SNES,
    N64,
    SMS,
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

    // Check for DOS executable (MZ header)
    if data.len() >= 2 && &data[0..2] == b"MZ" {
        return Ok(SystemType::PC);
    }

    // Check for DOS COM file (no header, typically small)
    // COM files are 64KB or less and have no specific signature
    // We'll detect them by exclusion and reasonable size
    if data.len() <= 0xFF00 && data.len() >= 16 {
        // Could be a COM file - check if it's not another format first
        // If we get here and it's a reasonable size, tentatively classify as PC
        // but continue checking other formats
    }

    // Check for Atari 2600
    // Atari 2600 ROMs are typically 2K, 4K, 8K, 12K, 16K, or 32K
    // They have no header, so we detect by size and lack of other formats
    if matches!(data.len(), 2048 | 4096 | 8192 | 12288 | 16384 | 32768) {
        // If it's a power-of-2 size that matches Atari 2600 cartridge sizes
        // and doesn't match other formats, assume it's Atari 2600
        return Ok(SystemType::Atari2600);
    }

    // Check for Sega Master System
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
    
    // Also check common SMS ROM sizes (headerless)
    if matches!(
        data.len(),
        8192 | 16384 | 32768 | 49152 | 65536 | 131072 | 262144 | 524288
    ) && data.len() != 8192 && data.len() != 32768 {
        // Exclude sizes that conflict with Atari 2600
        // This is a heuristic - could be SMS
        // TODO: Improve detection with checksum validation
        return Ok(SystemType::SMS);
    }

    // If it's small enough and not another format, assume COM file
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
            reason: "Unrecognized ROM format. Supported formats: iNES (.nes), Game Boy (.gb/.gbc), Atari 2600 (.a26/.bin), DOS (.com/.exe), SNES (.smc/.sfc), N64 (.z64/.n64/.v64), SMS (.sms)".to_string(),
        });
    }

    Err(UnsupportedRomError {
        reason: "Unknown ROM format. Supported formats: iNES (.nes), Game Boy (.gb/.gbc), Atari 2600 (.a26/.bin), DOS (.com/.exe), SNES (.smc/.sfc), N64 (.z64/.n64/.v64), SMS (.sms)"
            .to_string(),
    })
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
        // 128KB SMS ROM (common size)
        let data = vec![0u8; 131072];
        assert_eq!(detect_rom_type(&data).unwrap(), SystemType::SMS);

        // 256KB SMS ROM
        let data = vec![0u8; 262144];
        assert_eq!(detect_rom_type(&data).unwrap(), SystemType::SMS);
    }
}
