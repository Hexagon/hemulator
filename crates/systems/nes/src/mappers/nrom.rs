use crate::cartridge::Cartridge;
use emu_core::apu::TimingMode;

/// NROM (Mapper 0) - Basic mapper with no banking
#[derive(Debug)]
pub struct Nrom {
    prg_rom: Vec<u8>,
}

impl Nrom {
    pub fn new(cart: Cartridge) -> Self {
        Self {
            prg_rom: cart.prg_rom,
        }
    }

    pub fn read_prg(&self, addr: u16) -> u8 {
        let prg = &self.prg_rom;
        let len = prg.len();
        if len == 0 {
            return 0;
        }
        let off = if len == 0x4000 {
            (addr as usize - 0x8000) % 0x4000
        } else {
            (addr as usize - 0x8000) % len
        };
        prg[off]
    }

    pub fn prg_rom(&self) -> &[u8] {
        &self.prg_rom
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nrom_16kb_mirroring() {
        use crate::cartridge::Mirroring;

        let cart = Cartridge {
            prg_rom: vec![0x42; 0x4000], // 16KB PRG
            chr_rom: vec![],
            mapper: 0,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Horizontal,
        };
        let nrom = Nrom::new(cart);

        // 16KB ROM should mirror at 0x8000 and 0xC000
        assert_eq!(nrom.read_prg(0x8000), 0x42);
        assert_eq!(nrom.read_prg(0xC000), 0x42);
        assert_eq!(nrom.read_prg(0xFFFF), 0x42);
    }

    #[test]
    fn nrom_32kb_no_mirroring() {
        use crate::cartridge::Mirroring;

        let mut prg = vec![0; 0x8000]; // 32KB PRG
        prg[0] = 0x11;
        prg[0x4000] = 0x22;

        let cart = Cartridge {
            prg_rom: prg,
            chr_rom: vec![],
            mapper: 0,
            timing: TimingMode::Ntsc,
            mirroring: Mirroring::Horizontal,
        };
        let nrom = Nrom::new(cart);

        // 32KB ROM should not mirror
        assert_eq!(nrom.read_prg(0x8000), 0x11);
        assert_eq!(nrom.read_prg(0xC000), 0x22);
    }
}
