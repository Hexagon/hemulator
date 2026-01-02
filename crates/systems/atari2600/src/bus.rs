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
            // Note: 0x00-0x2F are TIA write-only. On real hardware, reading them returns
            // the last value on the data bus (open bus). For now, we return 0 to avoid
            // executing them as code if the CPU jumps there.
            0x0000..=0x002F => 0,

            0x0030..=0x003F => self.tia.read((addr & 0x0F) as u8),

            // RIOT RAM (mirrored at 0x00-0x7F)
            0x0040..=0x007F => self.riot.read(addr),

            // RIOT RAM
            0x0080..=0x00FF => self.riot.read(addr),

            // TIA mirrors (0x0100-0x012F) - write-only TIA registers mirror
            0x0100..=0x012F => 0, // TIA write mirrors (read=0)
            
            // TIA read mirrors (0x0130-0x013F) - collision detection registers
            0x0130..=0x013F => self.tia.read((addr & 0x0F) as u8),
            
            // TIA + RAM mirrors (0x0140-0x017F) - mirrors the dual read/write region at 0x40-0x7F
            0x0140..=0x017F => self.riot.read(addr),

            // RIOT RAM mirrors (0x0180-0x01FF) - A7=1
            // This is CRITICAL for the stack (SP=0xFF -> 0x01FF)
            0x0180..=0x01FF => self.riot.read(addr),

            // Unused / TIA mirrors
            0x0200..=0x027F => 0,

            // RIOT I/O and timer
            0x0280..=0x029F => self.riot.read(addr),

            // Everything else maps to cartridge ROM
            _ => {
                if let Some(cart) = &self.cartridge {
                    let val = cart.read(addr);
                    // Debug logging for vector reads to diagnose boot issues
                    if addr >= 0x1FF0 {
                        // emu_core::logging::log(emu_core::logging::LogCategory::Bus, emu_core::logging::LogLevel::Trace, || format!("Bus: Read ROM {:04X} -> {:02X}", addr, val));
                    }
                    val
                } else {
                    0xFF
                }
            }
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

            // TIA write (mirrored) AND RIOT RAM simultaneously
            // On real hardware, addresses $40-$7F write to BOTH TIA and RAM
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

            // TIA mirrors (0x0100-0x013F) - TIA only
            0x0100..=0x013F => {
                if (addr & 0x3F) == 0x02 {
                    self.wsync_request = true;
                }
                self.tia.write((addr & 0x3F) as u8, val);
            }

            // TIA + RAM mirrors (0x0140-0x017F) - mirrors the dual-write behavior of 0x40-0x7F
            0x0140..=0x017F => {
                if (addr & 0x3F) == 0x02 {
                    self.wsync_request = true;
                }
                self.tia.write((addr & 0x3F) as u8, val);
                self.riot.write(addr, val);
            }

            // RIOT RAM mirrors (0x0180-0x01FF)
            // CRITICAL for stack
            0x0180..=0x01FF => self.riot.write(addr, val),

            // Unused / TIA mirrors
            0x0200..=0x027F => {}

            // RIOT I/O and timer
            0x0280..=0x029F => self.riot.write(addr, val),

            // Everything else maps to cartridge ROM (for bank switching)
            _ => {
                if let Some(cart) = &mut self.cartridge {
                    cart.write(addr);
                }
            }
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

        // Reading from TIA write-only addresses returns 0 (open bus emulation placeholder)
        assert_eq!(bus.read(0x0000), 0);

        // Reading from TIA read registers works
        assert_eq!(bus.read(0x0030), 0); // CXM0P - collision register (returns 0)
    }

    #[test]
    fn test_bus_riot_ram() {
        let mut bus = Atari2600Bus::new();

        // Write to RIOT RAM
        bus.write(0x0080, 0x12);
        assert_eq!(bus.read(0x0080), 0x12);

        // Test mirror at $0180 (A7=1)
        bus.write(0x0180, 0x34);
        assert_eq!(bus.read(0x0180), 0x34);
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

    #[test]
    fn test_bus_tia_ram_simultaneous_write() {
        let mut bus = Atari2600Bus::new();

        // Addresses $40-$7F should write to BOTH TIA and RAM on real hardware
        // Write to $40 (first byte of this dual-write range)
        bus.write(0x0040, 0xAB);

        // Verify the value was written to RAM (readable at $40)
        assert_eq!(bus.read(0x0040), 0xAB);

        // Write to another address in the range
        bus.write(0x007F, 0xCD);
        assert_eq!(bus.read(0x007F), 0xCD);

        // Verify we can still access normal RIOT RAM at $80+
        bus.write(0x0080, 0x12);
        assert_eq!(bus.read(0x0080), 0x12);

        // Test that the mirror at 0x140-0x17F also works (mirrors 0x40-0x7F behavior)
        bus.write(0x0140, 0xEF);
        assert_eq!(bus.read(0x0140), 0xEF);

        // Verify it's the same RAM location (address masking)
        assert_eq!(bus.read(0x0040), 0xEF);
    }
}
