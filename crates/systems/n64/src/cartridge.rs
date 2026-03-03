//! N64 cartridge implementation

use crate::N64Error;

/// N64 ROM magic number (big-endian format)
#[allow(dead_code)] // Used in tests
pub const N64_ROM_MAGIC: [u8; 4] = [0x80, 0x37, 0x12, 0x40];

/// Save type used by a cartridge
///
/// N64 games use different non-volatile storage chips for save data:
/// - EEPROM (4Kbit or 16Kbit): Most common; accessed via PIF serial commands
/// - SRAM (256Kbit): Used by games like Super Mario 64; battery-backed RAM at 0x08000000
/// - FlashRAM (1Mbit): Used by games like Pokémon Stadium; larger storage via serial commands
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveType {
    /// No save chip present (game has no saves or uses password system)
    None,
    /// 4Kbit EEPROM (512 bytes) - accessed via PIF serial commands
    Eeprom4K,
    /// 16Kbit EEPROM (2048 bytes) - accessed via PIF serial commands
    Eeprom16K,
    /// 256Kbit SRAM (32768 bytes) - battery-backed RAM at 0x08000000
    Sram,
    /// 1Mbit FlashRAM (131072 bytes) - accessed via serial commands
    FlashRam,
}

impl SaveType {
    /// Return the size in bytes of the save data, or 0 for None
    pub fn size_bytes(self) -> usize {
        match self {
            SaveType::None => 0,
            SaveType::Eeprom4K => 512,
            SaveType::Eeprom16K => 2048,
            SaveType::Sram => 32768,
            SaveType::FlashRam => 131072,
        }
    }
}

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

    /// Get the 4-byte cartridge ID from the ROM header (bytes 0x38–0x3B).
    ///
    /// Layout:
    /// - 0x38: media type ('N' for Nintendo 64 cartridge)
    /// - 0x39–0x3A: 2-char game code (e.g. "SM" for Super Mario 64)
    /// - 0x3B: country/region code ('E' = USA, 'J' = Japan, 'P' = PAL)
    ///
    /// Returns `None` when the ROM is too short to contain the header.
    pub fn game_id(&self) -> Option<[u8; 4]> {
        if self.rom.len() < 0x3C {
            return None;
        }
        Some([
            self.rom[0x38],
            self.rom[0x39],
            self.rom[0x3A],
            self.rom[0x3B],
        ])
    }

    /// Detect the save type for this cartridge based on its game ID.
    ///
    /// N64 games do not store the save type in the ROM header, so a game-ID
    /// lookup table is used instead.  Unknown games default to [`SaveType::None`].
    pub fn save_type(&self) -> SaveType {
        let Some(id) = self.game_id() else {
            return SaveType::None;
        };
        detect_save_type(&id)
    }
}

/// Look up the save type for a 4-byte cartridge ID.
///
/// The database covers the most common retail N64 titles.  Games not listed
/// default to [`SaveType::None`].
fn detect_save_type(id: &[u8; 4]) -> SaveType {
    // Convenience: compare against a string literal
    let id_str = std::str::from_utf8(id).unwrap_or("");

    match id_str {
        // --- SRAM (256Kbit = 32 KB) ---
        // Super Mario 64 (all regions)
        "NSME" | "NSMJ" | "NSMP" | "NSMF" => SaveType::Sram,
        // Banjo-Kazooie (all regions)
        "NBKE" | "NBKJ" | "NBKP" => SaveType::Sram,
        // Banjo-Tooie (all regions)
        "NB8E" | "NB8J" | "NB8P" => SaveType::Sram,
        // Conker's Bad Fur Day
        "NFUE" | "NFUJ" | "NFUP" => SaveType::Sram,
        // Diddy Kong Racing (all regions)
        "NDYE" | "NDYJ" | "NDYP" => SaveType::Sram,
        // Yoshi's Story (all regions)
        "NYSE" | "NYSJ" | "NYSP" => SaveType::Sram,
        // Kirby 64: The Crystal Shards
        "NK4E" | "NK4J" | "NK4P" => SaveType::Sram,
        // Paper Mario
        "NMQE" | "NMQJ" | "NMQP" => SaveType::Sram,
        // Star Wars: Shadows of the Empire
        "NSWE" | "NSWJ" | "NSWP" => SaveType::Sram,
        // Goldeneye 007
        "NGEE" | "NGEJ" | "NGEP" => SaveType::Sram,
        // Perfect Dark
        "NPDE" | "NPDJ" | "NPDP" => SaveType::Sram,
        // Doom 64 — uses SRAM (note: NDME is NOT Dr. Mario 64; that game is NM7E)
        "NDME" | "NDMJ" | "NDMP" => SaveType::Sram,
        // Quake 64
        "NQKE" => SaveType::Sram,
        // Duke Nukem 64
        "NDNE" => SaveType::Sram,
        // Turok: Dinosaur Hunter
        "NTUE" | "NTUJ" | "NTUP" => SaveType::Sram,
        // Turok 2: Seeds of Evil
        "NT2E" | "NT2J" | "NT2P" => SaveType::Sram,

        // --- EEPROM 16 Kbit (2 KB) ---
        // The Legend of Zelda: Ocarina of Time (all regions)
        "NZLE" | "NZLJ" | "NZLP" => SaveType::Eeprom16K,
        // The Legend of Zelda: Majora's Mask (all regions)
        "NZSE" | "NZSJ" | "NZSP" => SaveType::Eeprom16K,
        // Donkey Kong 64
        "NDOE" | "NDOJ" | "NDOP" => SaveType::Eeprom16K,
        // F-Zero X
        "NFZE" | "NFZJ" | "NFZP" => SaveType::Eeprom16K,
        // Pokemon Snap
        "NPPE" | "NPPJ" | "NPPP" => SaveType::Eeprom16K,
        // Pokemon Puzzle League
        "NPUE" => SaveType::Eeprom16K,
        // Harvest Moon 64
        "NHME" => SaveType::Eeprom16K,
        // Ogre Battle 64
        "NOBE" => SaveType::Eeprom16K,

        // --- EEPROM 4 Kbit (512 bytes) ---
        // Mario Kart 64
        "NM8E" | "NM8J" | "NM8P" => SaveType::Eeprom4K,
        // Star Fox 64 / Lylat Wars
        "NFXE" | "NFXJ" | "NFXP" => SaveType::Eeprom4K,
        // Wave Race 64
        "NWRE" | "NWRJ" | "NWRP" => SaveType::Eeprom4K,
        // 1080° Snowboarding
        "NTWE" | "NTWJ" | "NTWP" => SaveType::Eeprom4K,
        // Pilotwings 64
        "NPWE" | "NPWJ" | "NPWP" => SaveType::Eeprom4K,
        // Super Smash Bros.
        "NALE" | "NALJ" | "NALP" => SaveType::Eeprom4K,
        // Mario Golf
        "NMGE" | "NMGJ" | "NMGP" => SaveType::Eeprom4K,
        // Mario Tennis
        "NMTE" | "NMTJ" | "NMTP" => SaveType::Eeprom4K,
        // Mario Party
        "NMPE" | "NMPJ" | "NMPP" => SaveType::Eeprom4K,
        // Mario Party 2
        "NM2E" | "NM2J" | "NM2P" => SaveType::Eeprom4K,
        // Mario Party 3
        "NM3E" | "NM3J" | "NM3P" => SaveType::Eeprom4K,
        // Excitebike 64
        "NEXE" => SaveType::Eeprom4K,
        // Dr. Mario 64 (NDME is Doom 64; Dr. Mario 64's actual ID is NM7E)
        "NM7E" | "NM7J" | "NM7P" => SaveType::Eeprom4K,
        // Tetrisphere
        "NTSE" => SaveType::Eeprom4K,
        // Tetris 64
        "NTTE" | "NTTJ" => SaveType::Eeprom4K,
        // NHL Breakaway 98
        "NNHE" => SaveType::Eeprom4K,

        // --- FlashRAM (1 Mbit = 128 KB) ---
        // Pokémon Stadium (all regions)
        "NPSE" | "NPSJ" | "NPSP" => SaveType::FlashRam,
        // Pokémon Stadium 2 (Gold and Silver)
        "NPGE" | "NPGJ" | "NPGP" => SaveType::FlashRam,
        // Resident Evil 2
        "NBID" => SaveType::FlashRam,

        // Unknown game — assume no save
        _ => SaveType::None,
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

    #[test]
    fn test_game_id_short_rom() {
        // ROM that is big enough to load (0x1000) but shorter than header field
        let mut data = vec![0u8; 0x1000];
        data[0..4].copy_from_slice(&N64_ROM_MAGIC);
        let cart = Cartridge::load(&data).unwrap();
        // ROM is 0x1000 bytes which is >= 0x3C, so game_id should succeed with zero bytes
        let id = cart.game_id().unwrap();
        assert_eq!(id, [0, 0, 0, 0]);
    }

    #[test]
    fn test_game_id_known_game() {
        let mut data = vec![0u8; 0x1000];
        data[0..4].copy_from_slice(&N64_ROM_MAGIC);
        // Write Super Mario 64 US game ID at 0x38
        data[0x38] = b'N';
        data[0x39] = b'S';
        data[0x3A] = b'M';
        data[0x3B] = b'E';
        let cart = Cartridge::load(&data).unwrap();
        assert_eq!(cart.game_id(), Some([b'N', b'S', b'M', b'E']));
        assert_eq!(cart.save_type(), SaveType::Sram);
    }

    #[test]
    fn test_save_type_eeprom16k() {
        let mut data = vec![0u8; 0x1000];
        data[0..4].copy_from_slice(&N64_ROM_MAGIC);
        // Zelda OOT US
        data[0x38] = b'N';
        data[0x39] = b'Z';
        data[0x3A] = b'L';
        data[0x3B] = b'E';
        let cart = Cartridge::load(&data).unwrap();
        assert_eq!(cart.save_type(), SaveType::Eeprom16K);
    }

    #[test]
    fn test_save_type_eeprom4k() {
        let mut data = vec![0u8; 0x1000];
        data[0..4].copy_from_slice(&N64_ROM_MAGIC);
        // Mario Kart 64 US
        data[0x38] = b'N';
        data[0x39] = b'M';
        data[0x3A] = b'8';
        data[0x3B] = b'E';
        let cart = Cartridge::load(&data).unwrap();
        assert_eq!(cart.save_type(), SaveType::Eeprom4K);
    }

    #[test]
    fn test_save_type_none_unknown() {
        let mut data = vec![0u8; 0x1000];
        data[0..4].copy_from_slice(&N64_ROM_MAGIC);
        // Unknown game ID
        data[0x38] = b'N';
        data[0x39] = b'X';
        data[0x3A] = b'X';
        data[0x3B] = b'E';
        let cart = Cartridge::load(&data).unwrap();
        assert_eq!(cart.save_type(), SaveType::None);
    }

    #[test]
    fn test_save_type_size_bytes() {
        assert_eq!(SaveType::None.size_bytes(), 0);
        assert_eq!(SaveType::Eeprom4K.size_bytes(), 512);
        assert_eq!(SaveType::Eeprom16K.size_bytes(), 2048);
        assert_eq!(SaveType::Sram.size_bytes(), 32768);
        assert_eq!(SaveType::FlashRam.size_bytes(), 131072);
    }
}
