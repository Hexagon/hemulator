use std::fs::File;
use std::io::Read;
use std::path::Path;

#[derive(Debug)]
pub struct Cartridge {
    pub prg_rom: Vec<u8>,
    pub chr_rom: Vec<u8>,
    pub mapper: u8,
}

impl Cartridge {
    /// Very small iNES loader supporting mapper 0 (NROM).
    pub fn from_file<P: AsRef<Path>>(p: P) -> std::io::Result<Self> {
        let mut f = File::open(p)?;
        let mut header = [0u8; 16];
        f.read_exact(&mut header)?;
        if &header[0..4] != b"NES\x1A" {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Not iNES file"));
        }
        let prg_size = header[4] as usize * 16 * 1024;
        let chr_size = header[5] as usize * 8 * 1024;
        let mapper = (header[6] >> 4) | (header[7] & 0xF0);

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

        Ok(Self { prg_rom, chr_rom, mapper })
    }
}
