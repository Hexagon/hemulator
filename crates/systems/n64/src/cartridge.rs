//! N64 cartridge implementation

use crate::N64Error;

/// N64 ROM magic number (big-endian format)
#[allow(dead_code)] // Used in tests
pub const N64_ROM_MAGIC: [u8; 4] = [0x80, 0x37, 0x12, 0x40];

/// N64 ROM byte order formats
#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(clippy::enum_variant_names)]
enum ByteOrder {
    /// Big-endian (native N64 format, .z64)
    BigEndian,
    /// Little-endian (byte-swapped, .n64)
    LittleEndian,
    /// Middle-endian (word-swapped, .v64)
    MiddleEndian,
}

/// N64 cartridge
pub struct Cartridge {
    /// ROM data (converted to big-endian)
    rom: Vec<u8>,
}

impl Cartridge {
    pub fn load(data: &[u8]) -> Result<Self, N64Error> {
        if data.len() < 0x1000 {
            return Err(N64Error::InvalidRom(
                "ROM too small (minimum 4KB)".to_string(),
            ));
        }

        // Detect byte order from header
        let byte_order = Self::detect_byte_order(data)?;

        // Convert to big-endian if necessary
        let rom = match byte_order {
            ByteOrder::BigEndian => data.to_vec(),
            ByteOrder::LittleEndian => Self::convert_little_endian(data),
            ByteOrder::MiddleEndian => Self::convert_middle_endian(data),
        };

        Ok(Self { rom })
    }

    fn detect_byte_order(data: &[u8]) -> Result<ByteOrder, N64Error> {
        if data.len() < 4 {
            return Err(N64Error::InvalidRom("ROM too small".to_string()));
        }

        // Check first 4 bytes for magic value
        match &data[0..4] {
            [0x80, 0x37, 0x12, 0x40] => Ok(ByteOrder::BigEndian), // .z64
            [0x40, 0x12, 0x37, 0x80] => Ok(ByteOrder::LittleEndian), // .n64
            [0x37, 0x80, 0x40, 0x12] => Ok(ByteOrder::MiddleEndian), // .v64
            _ => Err(N64Error::InvalidRom(
                "Unrecognized N64 ROM format (bad magic)".to_string(),
            )),
        }
    }

    fn convert_little_endian(data: &[u8]) -> Vec<u8> {
        let mut result = data.to_vec();
        for chunk in result.chunks_exact_mut(4) {
            chunk.swap(0, 3);
            chunk.swap(1, 2);
        }
        result
    }

    fn convert_middle_endian(data: &[u8]) -> Vec<u8> {
        let mut result = data.to_vec();
        for chunk in result.chunks_exact_mut(4) {
            chunk.swap(0, 1);
            chunk.swap(2, 3);
        }
        result
    }

    pub fn read(&self, offset: u32) -> u8 {
        *self.rom.get(offset as usize).unwrap_or(&0)
    }

    /// Read a range of bytes from ROM
    pub fn read_range(&self, offset: u32, len: usize) -> Vec<u8> {
        let start = offset as usize;
        let end = (start + len).min(self.rom.len());
        self.rom.get(start..end).unwrap_or(&[]).to_vec()
    }

    /// Get ROM size in bytes
    pub fn size(&self) -> usize {
        self.rom.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_too_small() {
        let data = vec![0; 1024];
        assert!(Cartridge::load(&data).is_err());
    }

    #[test]
    fn test_detect_big_endian() {
        let mut data = vec![0; 0x1000];
        data[0..4].copy_from_slice(&N64_ROM_MAGIC);

        let cart = Cartridge::load(&data).unwrap();
        assert_eq!(cart.rom[0..4], N64_ROM_MAGIC);
    }

    #[test]
    fn test_detect_little_endian() {
        let mut data = vec![0; 0x1000];
        data[0..4].copy_from_slice(&[0x40, 0x12, 0x37, 0x80]);

        let cart = Cartridge::load(&data).unwrap();
        // Should be converted to big-endian
        assert_eq!(cart.rom[0..4], N64_ROM_MAGIC);
    }

    #[test]
    fn test_detect_middle_endian() {
        let mut data = vec![0; 0x1000];
        data[0..4].copy_from_slice(&[0x37, 0x80, 0x40, 0x12]);

        let cart = Cartridge::load(&data).unwrap();
        // Should be converted to big-endian
        assert_eq!(cart.rom[0..4], N64_ROM_MAGIC);
    }

    #[test]
    fn test_read_rom() {
        let mut data = vec![0; 0x1000];
        data[0..4].copy_from_slice(&N64_ROM_MAGIC);
        data[4] = 0x42;

        let cart = Cartridge::load(&data).unwrap();
        assert_eq!(cart.read(4), 0x42);
    }

    #[test]
    fn test_read_out_of_bounds() {
        let mut data = vec![0; 0x1000];
        data[0..4].copy_from_slice(&N64_ROM_MAGIC);

        let cart = Cartridge::load(&data).unwrap();
        assert_eq!(cart.read(0x10000), 0);
    }
}
