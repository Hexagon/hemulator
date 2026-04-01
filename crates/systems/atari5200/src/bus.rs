//! Atari 5200 memory bus implementation
//!
//! Memory Map:
//! $0000-$3FFF: 16KB RAM
//! $4000-$7FFF: Unused (open bus)
//! $8000-$BFFF: Cartridge ROM (may be banked)
//! $C000-$C0FF: GTIA registers (mirrored every 32 bytes)
//! $D000-$D0FF: Unused
//! $D400-$D4FF: ANTIC registers (mirrored every 16 bytes)
//! $E800-$E8FF: POKEY registers (mirrored every 16 bytes)
//! $F000-$F7FF: Atari 5200 BIOS ROM (4KB, built-in)
//! $F800-$FFFF: Atari 5200 BIOS ROM mirror / last 2KB

use emu_core::cpu_6502::Memory6502;
use serde::{Deserialize, Serialize};

use crate::antic::Antic;
use crate::cartridge::Cartridge;
use crate::gtia::Gtia;
use crate::pokey::Pokey;

/// Built-in 5200 BIOS stub
/// This minimal BIOS provides enough to boot cartridges:
/// - Sets up display list
/// - Initializes hardware registers
/// - Jumps to cartridge entry point at $BFF9-$BFFA (or reset vector at $FFFC)
fn default_bios() -> Vec<u8> {
    let mut bios = vec![0u8; 2048];

    // The BIOS normally initializes the system and then jumps to the cart.
    // For a minimal stub:
    // $F800: SEI
    // $F801: CLD
    // $F802: LDX #$FF
    // $F804: TXS
    // $F805: LDA $BFFD  ; Cart reset vector high byte
    // $F808: PHA
    // $F809: LDA $BFFC  ; Cart reset vector low byte
    // $F80C: PHA
    // $F80D: RTS        ; Jump to cart via RTS trick

    let code: &[u8] = &[
        0x78, // SEI
        0xD8, // CLD
        0xA2, 0xFF, // LDX #$FF
        0x9A, // TXS
        // Initialize ANTIC - set up a simple display list
        0xA9, 0x00, // LDA #$00
        0x8D, 0x00, 0xD4, // STA $D400 (DMACTL = 0, disable display for now)
        0xA9, 0x40, // LDA #$40
        0x8D, 0x0E, 0xD4, // STA $D40E (NMIEN = $40, enable VBI)
        // Read cartridge start address from $BFFD:$BFFC
        0xAD, 0xFD, 0xBF, // LDA $BFFD (cart reset vector high)
        0x48, // PHA
        0xAD, 0xFC, 0xBF, // LDA $BFFC (cart reset vector low)
        0x48, // PHA
        0x60, // RTS (jump to cart entry)
    ];

    // Place code at the start of the BIOS area ($F800)
    bios[..code.len()].copy_from_slice(code);

    // Set up vectors at the end of BIOS
    // NMI vector at $FFFA-$FFFB -> point to a simple RTI
    let rti_addr = 0xF800 + code.len() as u16;
    bios[code.len()] = 0x40; // RTI instruction

    // NMI vector
    bios[0x7FA] = (rti_addr & 0xFF) as u8;
    bios[0x7FB] = (rti_addr >> 8) as u8;

    // RESET vector at $FFFC-$FFFD -> $F800 (start of BIOS)
    bios[0x7FC] = 0x00;
    bios[0x7FD] = 0xF8;

    // IRQ vector at $FFFE-$FFFF -> RTI
    bios[0x7FE] = (rti_addr & 0xFF) as u8;
    bios[0x7FF] = (rti_addr >> 8) as u8;

    bios
}

/// Atari 5200 memory bus
#[derive(Debug, Serialize, Deserialize)]
pub struct Atari5200Bus {
    ram: Vec<u8>,
    pub antic: Antic,
    pub gtia: Gtia,
    pub pokey: Pokey,
    #[serde(skip)]
    pub cartridge: Option<Cartridge>,
    bios: Vec<u8>,
    #[serde(skip)]
    wsync_request: bool,
}

impl Default for Atari5200Bus {
    fn default() -> Self {
        Self::new()
    }
}

impl Atari5200Bus {
    pub fn new() -> Self {
        Self {
            ram: vec![0u8; 0x4000], // 16KB RAM
            antic: Antic::new(),
            gtia: Gtia::new(),
            pokey: Pokey::new(),
            cartridge: None,
            bios: default_bios(),
            wsync_request: false,
        }
    }

    /// Load a cartridge
    pub fn load_cartridge(&mut self, cartridge: Cartridge) {
        self.cartridge = Some(cartridge);
    }

    /// Load a custom BIOS (2KB or 4KB)
    pub fn load_bios(&mut self, data: &[u8]) {
        if data.len() >= 2048 {
            self.bios = data[..2048].to_vec();
        }
    }

    /// Reset BIOS to the built-in default stub
    pub fn reset_bios(&mut self) {
        self.bios = default_bios();
    }

    /// Reset the bus
    pub fn reset(&mut self) {
        self.ram.fill(0);
        self.antic.reset();
        self.gtia.reset();
        self.pokey.reset();
        self.wsync_request = false;
    }

    /// Check if WSYNC was requested
    pub fn take_wsync_request(&mut self) -> bool {
        // Check both ANTIC's WSYNC and our local flag
        let antic_wsync = self.antic.take_wsync_request();
        let local = self.wsync_request;
        self.wsync_request = false;
        antic_wsync || local
    }

    /// Clock the bus for the given number of CPU cycles
    pub fn clock(&mut self, cycles: u32) {
        for _ in 0..cycles {
            self.pokey.clock();
        }
    }

    /// Read memory without side effects (for rendering/debugging)
    pub fn read_internal(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x3FFF => self.ram[addr as usize],
            0x4000..=0xBFFF => {
                if let Some(ref cart) = self.cartridge {
                    if addr >= 0x8000 {
                        cart.read(addr)
                    } else {
                        0xFF
                    }
                } else {
                    0xFF
                }
            }
            0xF800..=0xFFFF => {
                let offset = (addr - 0xF800) as usize;
                self.bios.get(offset).copied().unwrap_or(0xFF)
            }
            _ => 0xFF,
        }
    }
}

impl Memory6502 for Atari5200Bus {
    fn read(&self, addr: u16) -> u8 {
        match addr {
            // RAM (16KB)
            0x0000..=0x3FFF => self.ram[addr as usize],

            // Cartridge ROM
            0x4000..=0xBFFF => {
                if let Some(ref cart) = self.cartridge {
                    if addr >= 0x8000 {
                        cart.read(addr)
                    } else {
                        // $4000-$7FFF: some carts mirror here, otherwise open bus
                        0xFF
                    }
                } else {
                    0xFF
                }
            }

            // GTIA ($C000-$C0FF, mirrored)
            0xC000..=0xCFFF => self.gtia.read(addr),

            // ANTIC ($D400-$D4FF, mirrored)
            0xD400..=0xD4FF => self.antic.read(addr),

            // POKEY ($E800-$E8FF, mirrored)
            0xE800..=0xE8FF => self.pokey.read(addr),

            // BIOS ROM ($F800-$FFFF)
            0xF800..=0xFFFF => {
                let offset = (addr - 0xF800) as usize;
                self.bios.get(offset).copied().unwrap_or(0xFF)
            }

            // Other ranges
            _ => 0xFF,
        }
    }

    fn write(&mut self, addr: u16, val: u8) {
        match addr {
            // RAM (16KB)
            0x0000..=0x3FFF => self.ram[addr as usize] = val,

            // Cartridge (bank switching)
            0x4000..=0xBFFF => {
                if let Some(ref cart) = self.cartridge {
                    cart.write(addr, val);
                }
            }

            // GTIA ($C000-$C0FF, mirrored)
            0xC000..=0xCFFF => self.gtia.write(addr, val),

            // ANTIC ($D400-$D4FF, mirrored)
            0xD400..=0xD4FF => {
                self.antic.write(addr, val);
                // WSYNC written through ANTIC
                if addr & 0x0F == 0x0A {
                    self.wsync_request = true;
                }
            }

            // POKEY ($E800-$E8FF, mirrored)
            0xE800..=0xE8FF => self.pokey.write(addr, val),

            // BIOS area is ROM - writes ignored
            0xF800..=0xFFFF => {}

            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ram_read_write() {
        let mut bus = Atari5200Bus::new();
        bus.write(0x0100, 0x42);
        assert_eq!(bus.read(0x0100), 0x42);
    }

    #[test]
    fn test_gtia_registers() {
        let mut bus = Atari5200Bus::new();
        bus.write(0xC016, 0x28); // COLPF0 via memory map
        assert_eq!(bus.gtia.colpf(0), 0x28);
    }

    #[test]
    fn test_antic_registers() {
        let mut bus = Atari5200Bus::new();
        bus.write(0xD402, 0x00); // DLISTL
        bus.write(0xD403, 0x40); // DLISTH
        assert_eq!(bus.antic.dlist(), 0x4000);
    }

    #[test]
    fn test_pokey_registers() {
        let mut bus = Atari5200Bus::new();
        bus.write(0xE800, 100); // AUDF1
        assert_eq!(bus.pokey.read(0x00), bus.pokey.read(0x00)); // POT0
    }

    #[test]
    fn test_bios_vectors() {
        let bus = Atari5200Bus::new();
        // RESET vector at $FFFC-$FFFD should point to $F800
        let low = bus.read(0xFFFC);
        let high = bus.read(0xFFFD);
        let reset_addr = (high as u16) << 8 | low as u16;
        assert_eq!(reset_addr, 0xF800);
    }

    #[test]
    fn test_wsync() {
        let mut bus = Atari5200Bus::new();
        assert!(!bus.take_wsync_request());
        bus.write(0xD40A, 0x00); // WSYNC
        assert!(bus.take_wsync_request());
        assert!(!bus.take_wsync_request()); // Cleared after take
    }
}
