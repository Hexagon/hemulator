//! MBC3 (Memory Bank Controller 3)
//!
//! Used by many Game Boy games, especially those with battery-backed saves.
//! Some cartridges include a Real-Time Clock (RTC).
//!
//! Supports up to 2MB ROM and 32KB RAM.
//!
//! # Register Map
//!
//! - 0x0000-0x1FFF: RAM and Timer Enable (write 0x0A to enable)
//! - 0x2000-0x3FFF: ROM Bank Number (7 bits, 0-127)
//! - 0x4000-0x5FFF: RAM Bank Number (2 bits, 0-3) or RTC Register Select (0x08-0x0C)
//! - 0x6000-0x7FFF: Latch Clock Data (write 0x00 then 0x01 to latch)
//!
//! # RTC Registers
//!
//! - 0x08: RTC Seconds (0-59)
//! - 0x09: RTC Minutes (0-59)
//! - 0x0A: RTC Hours (0-23)
//! - 0x0B: RTC Days (lower 8 bits)
//! - 0x0C: RTC Days (upper 1 bit) + Halt + Day Carry flags

/// MBC3 mapper
#[derive(Debug)]
pub struct Mbc3 {
    rom: Vec<u8>,
    ram: Vec<u8>,
    ram_rtc_enabled: bool,
    rom_bank: u8,
    ram_bank: u8,
    // RTC registers (stubbed for now)
    rtc_s: u8,     // Seconds
    rtc_m: u8,     // Minutes
    rtc_h: u8,     // Hours
    rtc_dl: u8,    // Days lower
    rtc_dh: u8,    // Days upper + flags
    rtc_latch: u8, // For latching RTC (0x00 -> 0x01 sequence)
}

impl Mbc3 {
    pub fn new(rom: Vec<u8>, ram: Vec<u8>) -> Self {
        Self {
            rom,
            ram,
            ram_rtc_enabled: false,
            rom_bank: 1,
            ram_bank: 0,
            rtc_s: 0,
            rtc_m: 0,
            rtc_h: 0,
            rtc_dl: 0,
            rtc_dh: 0,
            rtc_latch: 0xFF,
        }
    }

    fn rom_bank_count(&self) -> usize {
        self.rom.len().div_ceil(0x4000)
    }

    fn ram_bank_count(&self) -> usize {
        if self.ram.is_empty() {
            1
        } else {
            self.ram.len().div_ceil(0x2000)
        }
    }

    pub fn read_rom(&self, addr: u16) -> u8 {
        let bank = if addr < 0x4000 {
            0
        } else {
            let bank = if self.rom_bank == 0 { 1 } else { self.rom_bank };
            (bank as usize) % self.rom_bank_count()
        };

        let offset = (bank * 0x4000) + ((addr & 0x3FFF) as usize);
        if offset < self.rom.len() {
            self.rom[offset]
        } else {
            0xFF
        }
    }

    pub fn write_rom(&mut self, addr: u16, val: u8) {
        match addr {
            0x0000..=0x1FFF => {
                // RAM and Timer Enable
                self.ram_rtc_enabled = (val & 0x0F) == 0x0A;
            }
            0x2000..=0x3FFF => {
                // ROM Bank Number (7 bits)
                self.rom_bank = val & 0x7F;
            }
            0x4000..=0x5FFF => {
                // RAM Bank Number or RTC Register Select
                self.ram_bank = val & 0x0F;
            }
            0x6000..=0x7FFF => {
                // Latch Clock Data (0x00 -> 0x01 latches RTC)
                if self.rtc_latch == 0x00 && val == 0x01 {
                    // RTC latch operation (stub - clock doesn't actually run)
                }
                self.rtc_latch = val;
            }
            _ => {}
        }
    }

    pub fn read_ram(&self, addr: u16) -> u8 {
        if !self.ram_rtc_enabled {
            return 0xFF;
        }

        match self.ram_bank {
            0x00..=0x03 => {
                // RAM bank
                if self.ram.is_empty() {
                    return 0xFF;
                }
                let bank = (self.ram_bank as usize) % self.ram_bank_count();
                let offset = (bank * 0x2000) + ((addr - 0xA000) as usize);
                if offset < self.ram.len() {
                    self.ram[offset]
                } else {
                    0xFF
                }
            }
            0x08 => self.rtc_s,  // RTC Seconds
            0x09 => self.rtc_m,  // RTC Minutes
            0x0A => self.rtc_h,  // RTC Hours
            0x0B => self.rtc_dl, // RTC Days lower
            0x0C => self.rtc_dh, // RTC Days upper
            _ => 0xFF,
        }
    }

    pub fn write_ram(&mut self, addr: u16, val: u8) {
        if !self.ram_rtc_enabled {
            return;
        }

        match self.ram_bank {
            0x00..=0x03 => {
                // RAM bank
                if self.ram.is_empty() {
                    return;
                }
                let bank = (self.ram_bank as usize) % self.ram_bank_count();
                let offset = (bank * 0x2000) + ((addr - 0xA000) as usize);
                if offset < self.ram.len() {
                    self.ram[offset] = val;
                }
            }
            0x08 => self.rtc_s = val,  // RTC Seconds
            0x09 => self.rtc_m = val,  // RTC Minutes
            0x0A => self.rtc_h = val,  // RTC Hours
            0x0B => self.rtc_dl = val, // RTC Days lower
            0x0C => self.rtc_dh = val, // RTC Days upper
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mbc3_default_banks() {
        let rom = vec![0xFF; 0x80000]; // 512KB
        let mbc = Mbc3::new(rom, vec![]);

        assert_eq!(mbc.rom_bank, 1);
        assert_eq!(mbc.ram_bank, 0);
    }

    #[test]
    fn test_mbc3_rom_banking() {
        let mut rom = vec![0; 0x100000]; // 1MB (64 banks)
        for bank in 0..64 {
            rom[bank * 0x4000] = bank as u8;
        }

        let mut mbc = Mbc3::new(rom, vec![]);

        // Bank 0 at lower, bank 1 at upper
        assert_eq!(mbc.read_rom(0x0000), 0);
        assert_eq!(mbc.read_rom(0x4000), 1);

        // Switch to bank 5
        mbc.write_rom(0x2000, 5);
        assert_eq!(mbc.read_rom(0x4000), 5);

        // Switch to bank 63
        mbc.write_rom(0x2000, 63);
        assert_eq!(mbc.read_rom(0x4000), 63);

        // Bank 0 at 0x4000 should map to bank 1
        mbc.write_rom(0x2000, 0);
        assert_eq!(mbc.read_rom(0x4000), 1);
    }

    #[test]
    fn test_mbc3_ram_enable() {
        let mbc = Mbc3::new(vec![0; 0x8000], vec![0; 0x2000]);
        assert!(!mbc.ram_rtc_enabled);

        let mut mbc = mbc;
        mbc.write_rom(0x0000, 0x0A);
        assert!(mbc.ram_rtc_enabled);

        mbc.write_rom(0x0000, 0x00);
        assert!(!mbc.ram_rtc_enabled);
    }

    #[test]
    fn test_mbc3_ram_read_write() {
        let mut mbc = Mbc3::new(vec![0; 0x8000], vec![0; 0x2000]);

        // RAM disabled
        assert_eq!(mbc.read_ram(0xA000), 0xFF);

        // Enable RAM
        mbc.write_rom(0x0000, 0x0A);

        // Write and read
        mbc.write_ram(0xA000, 0x42);
        assert_eq!(mbc.read_ram(0xA000), 0x42);
    }

    #[test]
    fn test_mbc3_ram_banking() {
        let mut ram = vec![0; 0x8000]; // 32KB (4 banks)
        for bank in 0..4 {
            ram[bank * 0x2000] = bank as u8;
        }

        let mut mbc = Mbc3::new(vec![0; 0x8000], ram);
        mbc.write_rom(0x0000, 0x0A); // Enable RAM

        // Bank 0
        mbc.write_rom(0x4000, 0);
        assert_eq!(mbc.read_ram(0xA000), 0);

        // Bank 1
        mbc.write_rom(0x4000, 1);
        assert_eq!(mbc.read_ram(0xA000), 1);

        // Bank 2
        mbc.write_rom(0x4000, 2);
        assert_eq!(mbc.read_ram(0xA000), 2);

        // Bank 3
        mbc.write_rom(0x4000, 3);
        assert_eq!(mbc.read_ram(0xA000), 3);
    }

    #[test]
    fn test_mbc3_rtc_registers() {
        let mut mbc = Mbc3::new(vec![0; 0x8000], vec![]);

        // Enable RTC
        mbc.write_rom(0x0000, 0x0A);

        // Write to RTC seconds
        mbc.write_rom(0x4000, 0x08);
        mbc.write_ram(0xA000, 0x2A);
        assert_eq!(mbc.read_ram(0xA000), 0x2A);

        // Write to RTC minutes
        mbc.write_rom(0x4000, 0x09);
        mbc.write_ram(0xA000, 0x1F);
        assert_eq!(mbc.read_ram(0xA000), 0x1F);

        // Write to RTC hours
        mbc.write_rom(0x4000, 0x0A);
        mbc.write_ram(0xA000, 0x17);
        assert_eq!(mbc.read_ram(0xA000), 0x17);

        // Write to RTC days lower
        mbc.write_rom(0x4000, 0x0B);
        mbc.write_ram(0xA000, 0xFF);
        assert_eq!(mbc.read_ram(0xA000), 0xFF);

        // Write to RTC days upper
        mbc.write_rom(0x4000, 0x0C);
        mbc.write_ram(0xA000, 0x01);
        assert_eq!(mbc.read_ram(0xA000), 0x01);
    }

    #[test]
    fn test_mbc3_rtc_latch() {
        let mut mbc = Mbc3::new(vec![0; 0x8000], vec![]);

        // Latch sequence: write 0x00 then 0x01
        mbc.write_rom(0x6000, 0x00);
        assert_eq!(mbc.rtc_latch, 0x00);

        mbc.write_rom(0x6000, 0x01);
        assert_eq!(mbc.rtc_latch, 0x01);
        // RTC values are latched (though our stub doesn't actually update them)
    }

    #[test]
    fn test_mbc3_no_ram() {
        let mut mbc = Mbc3::new(vec![0; 0x8000], vec![]);

        mbc.write_rom(0x0000, 0x0A); // Enable RAM
        mbc.write_rom(0x4000, 0x00); // Select RAM bank 0

        assert_eq!(mbc.read_ram(0xA000), 0xFF);
        mbc.write_ram(0xA000, 0x42);
        assert_eq!(mbc.read_ram(0xA000), 0xFF);
    }
}
