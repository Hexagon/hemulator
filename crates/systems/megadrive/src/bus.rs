//! Mega Drive bus / memory map
//!
//! Maps the 68000's 24-bit address space to the various devices:
//!   $000000-$3FFFFF  Cartridge ROM (up to 4 MB)
//!   $A00000-$A0FFFF  Z80 address space (RAM + YM2612 + PSG + bank register)
//!   $A10000-$A1001F  I/O area (controllers)
//!   $A11100          Z80 bus request
//!   $A11200          Z80 reset
//!   $C00000-$C00003  VDP data port
//!   $C00004-$C00007  VDP control port
//!   $C00008-$C0000F  VDP HV counter
//!   $C00011          PSG
//!   $FF0000-$FFFFFF  68K work RAM (64 KB, mirrored)

use crate::m68k::Memory68k;
use crate::psg::Psg;
use crate::vdp::Vdp;
use crate::ym2612::Ym2612;
use std::cell::RefCell;

const WORK_RAM_SIZE: usize = 0x10000; // 64 KB
const Z80_RAM_SIZE: usize = 0x2000; // 8 KB

/// Mega Drive bus connecting the 68000 to all devices
pub struct MdBus {
    pub rom: Vec<u8>,
    pub ram: Vec<u8>,
    pub z80_ram: Vec<u8>,

    pub vdp: RefCell<Vdp>,
    pub ym2612: Ym2612,
    pub psg: Psg,

    // I/O
    pub controller_1: u16,
    pub controller_2: u16,
    io_ctrl_1: u8,
    io_ctrl_2: u8,
    io_ctrl_ext: u8,

    // Z80 control
    pub z80_bus_requested: bool,
    pub z80_reset: bool,
    z80_bank_register: u32,
    z80_bank_shift: u8,

    // Region (PAL/NTSC)
    pub region_pal: bool,

    // SRAM (battery-backed)
    sram: Vec<u8>,
    sram_enabled: bool,
    sram_start: u32,
    sram_end: u32,
}

impl MdBus {
    pub fn new() -> Self {
        Self {
            rom: Vec::new(),
            ram: vec![0; WORK_RAM_SIZE],
            z80_ram: vec![0; Z80_RAM_SIZE],
            vdp: RefCell::new(Vdp::new()),
            ym2612: Ym2612::new(),
            psg: Psg::new(),
            controller_1: 0xFFFF, // All buttons released
            controller_2: 0xFFFF,
            io_ctrl_1: 0x00,
            io_ctrl_2: 0x00,
            io_ctrl_ext: 0x00,
            z80_bus_requested: false,
            z80_reset: true,
            z80_bank_register: 0,
            z80_bank_shift: 0,
            region_pal: false,
            sram: Vec::new(),
            sram_enabled: false,
            sram_start: 0,
            sram_end: 0,
        }
    }

    pub fn load_rom(&mut self, data: &[u8]) {
        self.rom = data.to_vec();

        // Detect region from ROM header at $1F0
        self.region_pal = false;
        if data.len() > 0x1F0 {
            // Region string is at $1F0, up to 16 bytes
            let region_end = (data.len()).min(0x200);
            let region_bytes = &data[0x1F0..region_end];
            let has_europe = region_bytes.iter().any(|&b| b == b'E' || b == b'8');
            let has_ntsc = region_bytes.iter().any(|&b| b == b'J' || b == b'U' || b == b'1' || b == b'4');
            // PAL if Europe-only (no NTSC regions)
            if has_europe && !has_ntsc {
                self.region_pal = true;
            }
        }

        // Pass PAL flag to VDP
        self.vdp.borrow_mut().region_pal = self.region_pal;

        // Check for SRAM info in header at $1B0-$1BF
        if self.rom.len() > 0x1C0 {
            let sram_flag = self.rom.get(0x1B0).copied().unwrap_or(0);
            if sram_flag == b'R' && self.rom.get(0x1B1).copied().unwrap_or(0) == b'A' {
                let sram_type = self.rom.get(0x1B2).copied().unwrap_or(0);
                if sram_type & 0x40 != 0 {
                    // Even addresses
                    self.sram_start = u32::from_be_bytes([
                        self.rom.get(0x1B4).copied().unwrap_or(0),
                        self.rom.get(0x1B5).copied().unwrap_or(0),
                        self.rom.get(0x1B6).copied().unwrap_or(0),
                        self.rom.get(0x1B7).copied().unwrap_or(0),
                    ]);
                    self.sram_end = u32::from_be_bytes([
                        self.rom.get(0x1B8).copied().unwrap_or(0),
                        self.rom.get(0x1B9).copied().unwrap_or(0),
                        self.rom.get(0x1BA).copied().unwrap_or(0),
                        self.rom.get(0x1BB).copied().unwrap_or(0),
                    ]);
                    let sram_size = (self.sram_end.saturating_sub(self.sram_start) + 1) as usize;
                    let sram_size = sram_size.min(0x10000); // Cap at 64KB
                    self.sram = vec![0xFF; sram_size];
                    self.sram_enabled = true;
                }
            }
        }
    }

    pub fn reset(&mut self) {
        self.ram = vec![0; WORK_RAM_SIZE];
        self.z80_ram = vec![0; Z80_RAM_SIZE];
        self.vdp.borrow_mut().reset();
        self.ym2612.reset();
        self.psg.reset();
        self.controller_1 = 0xFFFF;
        self.controller_2 = 0xFFFF;
        self.io_ctrl_1 = 0x00;
        self.io_ctrl_2 = 0x00;
        self.io_ctrl_ext = 0x00;
        self.z80_bus_requested = false;
        self.z80_reset = true;
        self.z80_bank_register = 0;
        self.z80_bank_shift = 0;
    }

    /// Read controller data port (3-button pad)
    /// Bits: _CBRLDU (active low)
    fn read_controller(&self, ctrl: u16, io_ctrl: u8) -> u8 {
        // TH line controlled by bit 6 of io_ctrl
        let th_out = io_ctrl & 0x40 != 0;
        if th_out {
            // TH=1: returns _C_BRLDU
            let up = if ctrl & 0x0001 == 0 { 0 } else { 0x01 };
            let down = if ctrl & 0x0002 == 0 { 0 } else { 0x02 };
            let left = if ctrl & 0x0004 == 0 { 0 } else { 0x04 };
            let right = if ctrl & 0x0008 == 0 { 0 } else { 0x08 };
            let b = if ctrl & 0x0010 == 0 { 0 } else { 0x10 };
            let c = if ctrl & 0x0020 == 0 { 0 } else { 0x20 };
            0x40 | c | b | right | left | down | up
        } else {
            // TH=0: returns __SA__DU
            let up = if ctrl & 0x0001 == 0 { 0 } else { 0x01 };
            let down = if ctrl & 0x0002 == 0 { 0 } else { 0x02 };
            let a = if ctrl & 0x0040 == 0 { 0 } else { 0x10 };
            let start = if ctrl & 0x0080 == 0 { 0 } else { 0x20 };
            start | a | down | up
        }
    }

    /// Execute pending 68K-to-VRAM/CRAM/VSRAM DMA transfer
    fn execute_pending_dma(&self) {
        if self.vdp.borrow().dma_pending() {
            let rom = &self.rom;
            let ram = &self.ram;
            self.vdp.borrow_mut().do_dma_68k(&|addr| {
                let addr = addr & 0xFFFFFF;
                match addr {
                    0x000000..=0x3FFFFF => {
                        let a = addr as usize;
                        if a + 1 < rom.len() {
                            ((rom[a] as u16) << 8) | rom[a + 1] as u16
                        } else {
                            0xFFFF
                        }
                    }
                    0xE00000..=0xFFFFFF => {
                        let a = (addr & 0xFFFF) as usize;
                        if a + 1 < ram.len() {
                            ((ram[a] as u16) << 8) | ram[a + 1] as u16
                        } else {
                            ((ram[a % ram.len()] as u16) << 8) | ram[(a + 1) % ram.len()] as u16
                        }
                    }
                    _ => 0xFFFF,
                }
            });
        }
    }
}

impl Memory68k for MdBus {
    fn read_byte(&self, addr: u32) -> u8 {
        let addr = addr & 0xFFFFFF;
        match addr {
            // ROM
            0x000000..=0x3FFFFF => {
                // Check SRAM overlap
                if self.sram_enabled && addr >= self.sram_start && addr <= self.sram_end {
                    let sram_addr = (addr - self.sram_start) as usize;
                    return self.sram.get(sram_addr).copied().unwrap_or(0xFF);
                }
                self.rom.get(addr as usize).copied().unwrap_or(0xFF)
            }

            // Z80 space (accessible when bus is requested)
            0xA00000..=0xA0FFFF => {
                let z80_addr = (addr & 0xFFFF) as usize;
                match z80_addr {
                    0x0000..=0x1FFF => self.z80_ram.get(z80_addr).copied().unwrap_or(0),
                    0x4000..=0x4003 => {
                        // YM2612 read (status)
                        self.ym2612.read_status()
                    }
                    _ => 0xFF,
                }
            }

            // I/O registers (byte-wide, mirrored to even addresses)
            0xA10000 | 0xA10001 => {
                // Version register: bit 7 = overseas, bit 6 = PAL, bits 3-0 = revision
                if self.region_pal { 0xE0 } else { 0xA0 }
            }
            0xA10002 | 0xA10003 => self.read_controller(self.controller_1, self.io_ctrl_1),
            0xA10004 | 0xA10005 => self.read_controller(self.controller_2, self.io_ctrl_2),
            0xA10006 | 0xA10007 => 0xFF, // EXT port
            0xA10008 | 0xA10009 => self.io_ctrl_1,
            0xA1000A | 0xA1000B => self.io_ctrl_2,
            0xA1000C | 0xA1000D => self.io_ctrl_ext,

            // Z80 bus request status — always granted (no Z80 emulated)
            0xA11100 | 0xA11101 => 0x00,

            // VDP ($C00000-$DFFFFF, mirrored every 32 bytes)
            0xC00000..=0xDFFFFF => {
                let vdp_reg = addr & 0x1F;
                match vdp_reg {
                    0x00..=0x03 => {
                        let word = self.vdp.borrow_mut().read_data();
                        if addr & 1 == 0 {
                            (word >> 8) as u8
                        } else {
                            word as u8
                        }
                    }
                    0x04..=0x07 => {
                        let word = self.vdp.borrow_mut().read_control();
                        if addr & 1 == 0 {
                            (word >> 8) as u8
                        } else {
                            word as u8
                        }
                    }
                    0x08..=0x0F => {
                        let word = self.vdp.borrow().read_hv_counter();
                        if addr & 1 == 0 {
                            (word >> 8) as u8
                        } else {
                            word as u8
                        }
                    }
                    _ => 0xFF,
                }
            }

            // Work RAM ($E00000-$FFFFFF, mirrored)
            0xE00000..=0xFFFFFF => {
                let ram_addr = (addr & 0xFFFF) as usize;
                self.ram[ram_addr]
            }

            _ => 0xFF,
        }
    }

    fn read_word(&self, addr: u32) -> u16 {
        let addr = addr & 0xFFFFFE; // Word-aligned
        match addr {
            // VDP ($C00000-$DFFFFF, mirrored every 32 bytes)
            0xC00000..=0xDFFFFF => {
                let vdp_reg = addr & 0x1E; // word-aligned offset within 32-byte window
                match vdp_reg {
                    0x00 | 0x02 => self.vdp.borrow_mut().read_data(),
                    0x04 | 0x06 => self.vdp.borrow_mut().read_control(),
                    0x08..=0x0E => self.vdp.borrow().read_hv_counter(),
                    _ => 0xFFFF,
                }
            }

            _ => {
                let hi = self.read_byte(addr) as u16;
                let lo = self.read_byte(addr | 1) as u16;
                (hi << 8) | lo
            }
        }
    }

    fn write_byte(&mut self, addr: u32, val: u8) {
        let addr = addr & 0xFFFFFF;
        match addr {
            // ROM area — check SRAM
            0x000000..=0x3FFFFF => {
                if self.sram_enabled && addr >= self.sram_start && addr <= self.sram_end {
                    let sram_addr = (addr - self.sram_start) as usize;
                    if let Some(cell) = self.sram.get_mut(sram_addr) {
                        *cell = val;
                    }
                }
                // Otherwise ignore writes to ROM
            }

            // Z80 space
            0xA00000..=0xA0FFFF => {
                let z80_addr = (addr & 0xFFFF) as usize;
                match z80_addr {
                    0x0000..=0x1FFF => {
                        if let Some(cell) = self.z80_ram.get_mut(z80_addr) {
                            *cell = val;
                        }
                    }
                    0x4000 => self.ym2612.write_address(0, val),
                    0x4001 => self.ym2612.write_data(0, val),
                    0x4002 => self.ym2612.write_address(1, val),
                    0x4003 => self.ym2612.write_data(1, val),
                    0x6000..=0x60FF => {
                        // Z80 bank register
                        self.z80_bank_register =
                            (self.z80_bank_register >> 1) | ((val as u32 & 1) << 23);
                        self.z80_bank_shift += 1;
                        if self.z80_bank_shift >= 9 {
                            self.z80_bank_shift = 0;
                        }
                    }
                    _ => {}
                }
            }

            // I/O control (even and odd addresses mirror)
            0xA10002 | 0xA10003 => self.io_ctrl_1 = (self.io_ctrl_1 & !0x40) | (val & 0x40),
            0xA10004 | 0xA10005 => self.io_ctrl_2 = (self.io_ctrl_2 & !0x40) | (val & 0x40),
            0xA10008 | 0xA10009 => self.io_ctrl_1 = val,
            0xA1000A | 0xA1000B => self.io_ctrl_2 = val,
            0xA1000C | 0xA1000D => self.io_ctrl_ext = val,

            // Z80 bus request
            0xA11100 | 0xA11101 => {
                self.z80_bus_requested = val & 0x01 != 0;
            }
            // Z80 reset
            0xA11200 | 0xA11201 => {
                let was_reset = self.z80_reset;
                self.z80_reset = val & 0x01 == 0;
                if was_reset && !self.z80_reset {
                    // Z80 coming out of reset — clear bank register
                    self.z80_bank_register = 0;
                    self.z80_bank_shift = 0;
                }
            }

            // TMSS ($A14000-$A14003) — ignore writes (no lock implemented)
            0xA14000..=0xA14003 => {}

            // VDP ($C00000-$DFFFFF, mirrored every 32 bytes)
            0xC00000..=0xDFFFFF => {
                let vdp_reg = addr & 0x1F;
                match vdp_reg {
                    0x00..=0x03 => {
                        self.vdp
                            .borrow_mut()
                            .write_data(((val as u16) << 8) | val as u16);
                    }
                    0x04..=0x07 => {
                        self.vdp
                            .borrow_mut()
                            .write_control(((val as u16) << 8) | val as u16);
                        self.execute_pending_dma();
                    }
                    0x11 | 0x13 | 0x15 | 0x17 => {
                        self.psg.write(val);
                    }
                    _ => {}
                }
            }

            // Work RAM ($E00000-$FFFFFF, mirrored)
            0xE00000..=0xFFFFFF => {
                let ram_addr = (addr & 0xFFFF) as usize;
                self.ram[ram_addr] = val;
            }

            _ => {} // Ignore unmapped writes
        }
    }

    fn write_word(&mut self, addr: u32, val: u16) {
        let addr = addr & 0xFFFFFE;
        match addr {
            // VDP ($C00000-$DFFFFF, mirrored every 32 bytes)
            0xC00000..=0xDFFFFF => {
                let vdp_reg = addr & 0x1E; // word-aligned offset within 32-byte window
                match vdp_reg {
                    0x00 | 0x02 => {
                        self.vdp.borrow_mut().write_data(val);
                    }
                    0x04 | 0x06 => {
                        self.vdp.borrow_mut().write_control(val);
                        self.execute_pending_dma();
                    }
                    0x10..=0x16 => {
                        self.psg.write(val as u8);
                    }
                    _ => {}
                }
            }

            _ => {
                self.write_byte(addr, (val >> 8) as u8);
                self.write_byte(addr | 1, val as u8);
            }
        }
    }
}
