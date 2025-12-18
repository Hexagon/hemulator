/// ROM detection and system selection
use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::upper_case_acronyms)]
pub enum SystemType {
    NES,
    GameBoy,
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

    // Check for Game Boy
    // Game Boy ROMs have a Nintendo logo at 0x104-0x133 and a header checksum at 0x14D
    if data.len() >= 0x150 {
        // Check for the Nintendo logo bytes (partial check for first few bytes)
        let logo_start = &data[0x104..0x108];
        // Standard GB/GBC logo starts with 0xCE 0xED 0x66 0x66
        if logo_start == [0xCE, 0xED, 0x66, 0x66] {
            return Ok(SystemType::GameBoy);
        }
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
            reason: "Unrecognized ROM format. Only iNES (.nes) and Game Boy (.gb/.gbc) formats are supported".to_string(),
        });
    }

    Err(UnsupportedRomError {
        reason: "Unknown ROM format. Supported formats: iNES (.nes), Game Boy (.gb/.gbc)"
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
    fn test_detect_unknown() {
        let data = vec![0u8; 1024];
        assert!(detect_rom_type(&data).is_err());
    }
}
