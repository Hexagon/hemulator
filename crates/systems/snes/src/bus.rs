//! SNES memory bus implementation

use crate::cartridge::Cartridge;
use crate::ppu::Ppu;
use crate::SnesError;
use emu_core::cpu_65c816::Memory65c816;
use emu_core::logging::{log, LogCategory, LogLevel};
use std::cell::Cell;

/// SNES memory bus
pub struct SnesBus {
    /// 128KB WRAM (work RAM)
    wram: [u8; 0x20000],
    /// Cartridge (optional)
    cartridge: Option<Cartridge>,
    /// PPU (Picture Processing Unit)
    ppu: Ppu,
    /// Frame counter for VBlank emulation
    frame_counter: u64,
    /// Cycle counter within current frame for VBlank timing
    /// NTSC SNES: ~89,342 cycles/frame, VBlank starts around cycle 75,000
    frame_cycle: u32,
    /// Controller state (16 bits per controller)
    /// Button mapping: B Y Select Start Up Down Left Right A X L R 0 0 0 0
    pub controller_state: [u16; 2],
    /// Controller shift registers for serial readout
    controller_shift: [Cell<u16>; 2],
    /// Controller strobe state
    controller_strobe: bool,
}

impl SnesBus {
    pub fn new() -> Self {
        Self {
            wram: [0; 0x20000],
            cartridge: None,
            ppu: Ppu::new(),
            frame_counter: 0,
            frame_cycle: 0,
            controller_state: [0; 2],
            controller_shift: [Cell::new(0), Cell::new(0)],
            controller_strobe: false,
        }
    }

    pub fn load_cartridge(&mut self, data: &[u8]) -> Result<(), SnesError> {
        self.cartridge = Some(Cartridge::load(data)?);
        Ok(())
    }

    pub fn unload_cartridge(&mut self) {
        self.cartridge = None;
    }

    pub fn has_cartridge(&self) -> bool {
        self.cartridge.is_some()
    }

    pub fn ppu(&self) -> &Ppu {
        &self.ppu
    }

    pub fn ppu_mut(&mut self) -> &mut Ppu {
        &mut self.ppu
    }

    pub fn tick_frame(&mut self) {
        self.frame_counter += 1;
        self.frame_cycle = 0; // Reset cycle counter at frame start
    }

    /// Update cycle counter within frame (called after each CPU step)
    pub fn tick_cycles(&mut self, cycles: u32) {
        self.frame_cycle += cycles;
    }

    /// Check if currently in VBlank period
    /// VBlank occurs during the last ~15% of the frame
    /// NTSC: ~89,342 cycles/frame, VBlank starts around cycle 75,000
    fn is_in_vblank(&self) -> bool {
        // VBlank starts after visible scanlines complete
        // Roughly 224/262 scanlines = ~85.5% of frame
        // So VBlank starts at cycle ~76,400 out of 89,342
        self.frame_cycle >= 76400
    }

    /// Set controller state (16 buttons) for controller `idx` (0 or 1).
    /// Button layout: B Y Select Start Up Down Left Right A X L R 0 0 0 0
    pub fn set_controller(&mut self, idx: usize, state: u16) {
        if idx < 2 {
            self.controller_state[idx] = state;
        }
    }

    pub fn get_rom_size(&self) -> usize {
        if let Some(ref cart) = self.cartridge {
            cart.rom_size()
        } else {
            0
        }
    }

    pub fn has_smc_header(&self) -> bool {
        if let Some(ref cart) = self.cartridge {
            cart.has_smc_header()
        } else {
            false
        }
    }
}

impl Default for SnesBus {
    fn default() -> Self {
        Self::new()
    }
}

impl Memory65c816 for SnesBus {
    fn read(&self, addr: u32) -> u8 {
        let bank = (addr >> 16) as u8;
        let offset = (addr & 0xFFFF) as u16;

        match bank {
            // Banks $00-$3F and $80-$BF: System area
            0x00..=0x3F | 0x80..=0xBF => {
                match offset {
                    // WRAM (shadow at $0000-$1FFF)
                    0x0000..=0x1FFF => self.wram[offset as usize],
                    // Hardware registers (PPU: $2100-$213F)
                    0x2100..=0x213F => self.ppu.read_register(offset),
                    // $4200 - NMITIMEN - Interrupt Enable and Joypad Request
                    0x4200 => {
                        // Bit 7: NMI enable
                        // Other bits: H/V timer interrupt enable, auto-joypad read enable
                        if self.ppu.nmi_enable { 0x80 } else { 0x00 }
                    }
                    // $4016 - JOYSER0 - Controller 1 Serial Data
                    0x4016 => {
                        // Bit 0: Serial data for controller 1
                        // Bits 1-7: Open bus (typically 0)
                        if self.controller_strobe {
                            // While strobed, return bit 0 of the current state
                            (self.controller_state[0] & 1) as u8
                        } else {
                            // Shift out the latched state
                            let cur = self.controller_shift[0].get();
                            let bit = (cur & 1) as u8;
                            self.controller_shift[0].set(cur >> 1);
                            bit
                        }
                    }
                    // $4017 - JOYSER1 - Controller 2 Serial Data
                    0x4017 => {
                        // Bit 0: Serial data for controller 2
                        // Bits 1-4: Not used (0x1E if nothing connected)
                        if self.controller_strobe {
                            (self.controller_state[1] & 1) as u8
                        } else {
                            let cur = self.controller_shift[1].get();
                            let bit = (cur & 1) as u8;
                            self.controller_shift[1].set(cur >> 1);
                            bit
                        }
                    }
                    // $4218-$421F - JOYxL/JOYxH - Auto-joypad read (set during VBlank)
                    0x4218 => (self.controller_state[0] & 0xFF) as u8, // JOY1L
                    0x4219 => ((self.controller_state[0] >> 8) & 0xFF) as u8, // JOY1H
                    0x421A => (self.controller_state[1] & 0xFF) as u8, // JOY2L
                    0x421B => ((self.controller_state[1] >> 8) & 0xFF) as u8, // JOY2H
                    0x421C => 0,                                       // JOY3L (not implemented)
                    0x421D => 0,                                       // JOY3H (not implemented)
                    0x421E => 0,                                       // JOY4L (not implemented)
                    0x421F => 0,                                       // JOY4H (not implemented)
                    // $4212 - HVBJOY - H/V Blank and Joypad Status
                    0x4212 => {
                        // Bit 7: VBlank flag (set during VBlank period)
                        // Bit 6: HBlank flag (not implemented)
                        // Bit 0: Auto-joypad read in progress (0 = finished)
                        if self.is_in_vblank() {
                            0x80 // VBlank
                        } else {
                            0x00 // Not in VBlank
                        }
                    }
                    // Other hardware registers
                    0x2000..=0x5FFF => {
                        log(LogCategory::Bus, LogLevel::Debug, || {
                            format!("SNES: Read from stubbed hardware register 0x{:04X} (bank 0x{:02X})", addr, bank)
                        });
                        0 // Stub
                    }
                    // WRAM (full at $6000-$7FFF in banks $00-$3F)
                    0x6000..=0x7FFF if bank < 0x40 => self.wram[(offset - 0x6000) as usize],
                    // Cartridge ROM
                    0x8000..=0xFFFF => {
                        if let Some(ref cart) = self.cartridge {
                            cart.read(addr)
                        } else {
                            0
                        }
                    }
                    _ => 0,
                }
            }
            // Banks $7E-$7F: Full WRAM mirror
            0x7E..=0x7F => {
                let wram_addr = ((bank as usize - 0x7E) << 16) | offset as usize;
                self.wram[wram_addr]
            }
            // Banks $40-$6F and $C0-$FF: Cartridge ROM
            _ => {
                if let Some(ref cart) = self.cartridge {
                    cart.read(addr)
                } else {
                    0
                }
            }
        }
    }

    fn write(&mut self, addr: u32, val: u8) {
        let bank = (addr >> 16) as u8;
        let offset = (addr & 0xFFFF) as u16;

        match bank {
            // Banks $00-$3F and $80-$BF: System area
            0x00..=0x3F | 0x80..=0xBF => {
                match offset {
                    // WRAM (shadow at $0000-$1FFF)
                    0x0000..=0x1FFF => self.wram[offset as usize] = val,
                    // $2100-$213F - PPU registers
                    0x2100..=0x213F => self.ppu.write_register(offset, val),
                    // $4200 - NMITIMEN - Interrupt Enable and Joypad Request
                    0x4200 => {
                        // Bit 7: NMI enable
                        // Bit 4: Joypad auto-read enable (not implemented)
                        // Other bits: H/V timer interrupt enable (not implemented)
                        self.ppu.nmi_enable = (val & 0x80) != 0;
                    }
                    // $4016 - JOYWR - Controller Strobe
                    0x4016 => {
                        // Bit 0: Controller strobe (1 = latch, 0 = shift)
                        let old_strobe = self.controller_strobe;
                        self.controller_strobe = (val & 1) != 0;

                        // On falling edge (1 -> 0), latch the controller state
                        if old_strobe && !self.controller_strobe {
                            self.controller_shift[0].set(self.controller_state[0]);
                            self.controller_shift[1].set(self.controller_state[1]);
                        }
                    }
                    // Other hardware registers
                    0x2000..=0x5FFF => {} // Stub - ignore writes
                    // WRAM (full at $6000-$7FFF in banks $00-$3F)
                    0x6000..=0x7FFF if bank < 0x40 => {
                        self.wram[(offset - 0x6000) as usize] = val;
                    }
                    // Cartridge ROM/RAM
                    0x8000..=0xFFFF => {
                        if let Some(ref mut cart) = self.cartridge {
                            cart.write(addr, val);
                        }
                    }
                    _ => {}
                }
            }
            // Banks $7E-$7F: Full WRAM mirror
            0x7E..=0x7F => {
                let wram_addr = ((bank as usize - 0x7E) << 16) | offset as usize;
                self.wram[wram_addr] = val;
            }
            // Banks $40-$6F and $C0-$FF: Cartridge ROM/RAM
            _ => {
                if let Some(ref mut cart) = self.cartridge {
                    cart.write(addr, val);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_controller_registers() {
        let mut bus = SnesBus::new();

        // Set controller 1 state: B button (bit 15)
        bus.set_controller(0, 0x8000);

        // Read auto-joypad registers
        let joy1l = bus.read(0x4218);
        let joy1h = bus.read(0x4219);
        assert_eq!(joy1l, 0x00);
        assert_eq!(joy1h, 0x80); // B button
    }

    #[test]
    fn test_controller_serial_read() {
        let mut bus = SnesBus::new();

        // Set controller state: A button (bit 7)
        bus.set_controller(0, 0x0080);

        // Latch state
        bus.write(0x4016, 1);
        bus.write(0x4016, 0);

        // Read bits serially (SNES sends LSB first)
        let mut bits_read = 0u16;
        for i in 0..16 {
            let bit = bus.read(0x4016) & 1;
            bits_read |= (bit as u16) << i;
        }

        assert_eq!(bits_read, 0x0080); // Should match the A button state
    }

    #[test]
    fn test_controller_strobe() {
        let mut bus = SnesBus::new();

        // Set controller state
        bus.set_controller(0, 0x1234);

        // Strobe on - should read current bit 0
        bus.write(0x4016, 1);
        let bit_strobed = bus.read(0x4016) & 1;
        assert_eq!(bit_strobed, 0); // bit 0 of 0x1234 is 0

        // Strobe off - latch and shift
        bus.write(0x4016, 0);

        // Read first bit
        let bit0 = bus.read(0x4016) & 1;
        assert_eq!(bit0, 0); // LSB of 0x1234
    }

    #[test]
    fn test_dual_controllers() {
        let mut bus = SnesBus::new();

        // Set different states for both controllers
        bus.set_controller(0, 0xAAAA);
        bus.set_controller(1, 0x5555);

        // Read auto-joypad registers
        assert_eq!(bus.read(0x4218), 0xAA); // JOY1L
        assert_eq!(bus.read(0x4219), 0xAA); // JOY1H
        assert_eq!(bus.read(0x421A), 0x55); // JOY2L
        assert_eq!(bus.read(0x421B), 0x55); // JOY2H

        // Latch both controllers
        bus.write(0x4016, 1);
        bus.write(0x4016, 0);

        // Read first bits from both controllers
        let bit1_0 = bus.read(0x4016) & 1;
        let bit2_0 = bus.read(0x4017) & 1;

        assert_eq!(bit1_0, 0); // LSB of 0xAAAA
        assert_eq!(bit2_0, 1); // LSB of 0x5555
    }
}
