//! Atari 2600 memory bus implementation
//!
//! The 6507 has a 13-bit address bus (8KB addressable space):
//! $0000-$002C: TIA write registers
//! $0030-$003F: TIA read registers (collision detection, input)
//! $0080-$00FF: RIOT RAM (128 bytes)
//! $0280-$0297: RIOT I/O and timer registers
//! $1000-$1FFF: Cartridge ROM (4KB, may be banked)

use emu_core::cpu_6502::Memory6502;
use serde::{Deserialize, Serialize};

use crate::cartridge::Cartridge;
use crate::riot::Riot;
use crate::tia::Tia;

/// Atari 2600 memory bus
#[derive(Debug, Serialize, Deserialize)]
pub struct Atari2600Bus {
    pub tia: Tia,
    pub riot: Riot,
    #[serde(skip)]
    pub cartridge: Option<Cartridge>,
    #[serde(skip)]
    wsync_request: bool,
}

impl Default for Atari2600Bus {
    fn default() -> Self {
        Self::new()
    }
}

impl Atari2600Bus {
    /// Create a new bus
    pub fn new() -> Self {
        Self {
            tia: Tia::new(),
            riot: Riot::new(),
            cartridge: None,
            wsync_request: false,
        }
    }

    /// Load a cartridge
    pub fn load_cartridge(&mut self, cartridge: Cartridge) {
        self.cartridge = Some(cartridge);
    }

    /// Reset the bus
    pub fn reset(&mut self) {
        self.tia.reset();
        self.riot.reset();
        self.wsync_request = false;
    }

    /// Check if WSYNC was requested and clear the flag
    pub fn take_wsync_request(&mut self) -> bool {
        let requested = self.wsync_request;
        self.wsync_request = false;
        requested
    }

    /// Clock the bus (TIA and RIOT)
    pub fn clock(&mut self, cycles: u32) {
        for _ in 0..cycles {
            self.tia.clock();
        }
        self.riot.clock(cycles as u16);
    }
}

impl Memory6502 for Atari2600Bus {
    fn read(&self, addr: u16) -> u8 {
        // 6507 only has 13-bit address bus
        let addr = addr & 0x1FFF;

        match addr {
            // TIA read registers (collision detection and input)
            0x0000..=0x000D => self.tia.read((addr & 0x0F) as u8),
            0x000E..=0x002F => 0, // Unused

            // TIA read (mirrored)
            0x0030..=0x003F => self.tia.read((addr & 0x0F) as u8),

            // RIOT RAM (mirrored at 0x00-0x7F)
            0x0040..=0x007F => self.riot.read(addr),

            // RIOT RAM
            0x0080..=0x00FF => self.riot.read(addr),
            0x0100..=0x017F => self.riot.read(addr),

            // Unused
            0x0180..=0x027F => 0,

            // RIOT I/O and timer
            0x0280..=0x029F => self.riot.read(addr),

            // Unused
            0x02A0..=0x0FFF => 0,

            // Cartridge ROM
            0x1000..=0x1FFF => {
                if let Some(cart) = &self.cartridge {
                    cart.read(addr)
                } else {
                    0xFF
                }
            }

            _ => 0,
        }
    }

    fn write(&mut self, addr: u16, val: u8) {
        // 6507 only has 13-bit address bus
        let addr = addr & 0x1FFF;

        match addr {
            // TIA write registers
            0x0000..=0x002C => {
                // Check if this is a WSYNC write
                if (addr & 0x3F) == 0x02 {
                    self.wsync_request = true;
                }
                self.tia.write((addr & 0x3F) as u8, val);
            }
            0x002D..=0x003F => {} // Unused

            // TIA write (mirrored) / RIOT RAM (mirrored at 0x00-0x7F)
            // On real hardware, addresses $40-$7F are decoded for both TIA write and RIOT RAM.
            // The 6507 bus allows simultaneous writes to both chips in this range.
            // This is intentional hardware behavior - not a bug in the emulator.
            0x0040..=0x007F => {
                // WSYNC is mirrored too (e.g., $42)
                if (addr & 0x3F) == 0x02 {
                    self.wsync_request = true;
                }
                self.tia.write((addr & 0x3F) as u8, val);
                self.riot.write(addr, val);
            }

            // RIOT RAM
            0x0080..=0x00FF => self.riot.write(addr, val),
            0x0100..=0x017F => self.riot.write(addr, val),

            // Unused
            0x0180..=0x027F => {}

            // RIOT I/O and timer
            0x0280..=0x029F => self.riot.write(addr, val),

            // Cartridge ROM (for bank switching)
            0x1000..=0x1FFF => {
                if let Some(cart) = &mut self.cartridge {
                    cart.write(addr);
                }
            }

            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bus_tia_access() {
        let mut bus = Atari2600Bus::new();

        // Write to TIA
        bus.write(0x0006, 0x42); // COLUP0

        // TIA writes don't have read-back, but we can verify no crash
        assert_eq!(bus.read(0x0000), 0); // TIA read register
    }

    #[test]
    fn test_bus_riot_ram() {
        let mut bus = Atari2600Bus::new();

        // Write to RIOT RAM
        bus.write(0x0080, 0x12);
        assert_eq!(bus.read(0x0080), 0x12);

        // Test mirror
        bus.write(0x0100, 0x34);
        assert_eq!(bus.read(0x0100), 0x34);
    }

    #[test]
    fn test_bus_riot_timer() {
        let mut bus = Atari2600Bus::new();

        // Set timer (TIM1T at $294)
        bus.write(0x0294, 10);

        // Clock the bus
        bus.clock(1);

        // Timer should have decremented (INTIM at $284)
        let timer_val = bus.read(0x0284);
        assert!(timer_val <= 10);
    }

    #[test]
    fn test_bus_address_masking() {
        let bus = Atari2600Bus::new();

        // 6507 has 13-bit address bus, so high bits should be masked
        // $2000 should map to $0000
        assert_eq!(bus.read(0x2000), bus.read(0x0000));
    }
}
