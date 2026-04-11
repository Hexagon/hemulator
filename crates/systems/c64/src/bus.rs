//! C64 memory bus / address decoder
//!
//! ## Memory Map (depends on 6510 I/O port $0001 bits 0–2)
//!
//! ```text
//! $0000–$0001: 6510 CPU I/O port (DDR + data)
//! $0002–$9FFF: RAM (always)
//! $A000–$BFFF: BASIC ROM (if LORAM=1 && HIRAM=1) or RAM
//! $C000–$CFFF: RAM (always)
//! $D000–$DFFF: I/O area (if CHAREN=1 and HIRAM or LORAM=1)
//!              CHAR ROM (if CHAREN=0 and HIRAM or LORAM=1)
//!              RAM (if both HIRAM=0 and LORAM=0)
//! $E000–$FFFF: KERNAL ROM (if HIRAM=1) or RAM
//! ```
//!
//! ## I/O Area ($D000–$DFFF when visible)
//! - $D000–$D3FF: VIC-II (mirrored every 64 bytes)
//! - $D400–$D7FF: SID (mirrored every 32 bytes)
//! - $D800–$DBFF: Color RAM (1024 nybbles)
//! - $DC00–$DCFF: CIA 1 (mirrored every 16 bytes)
//! - $DD00–$DDFF: CIA 2 (mirrored every 16 bytes)
//! - $DE00–$DFFF: I/O expansion area (no devices, returns $FF)

use crate::cia::Cia;
use crate::sid::Sid;
use crate::vic::Vic;
use emu_core::cpu_6502::Memory6502;
use std::cell::RefCell;
use std::rc::Rc;

/// C64 memory bus implementing the 6510 address space
pub struct C64Bus {
    /// 64KB main RAM
    pub ram: [u8; 0x10000],
    /// 1KB color RAM (4-bit nybbles; upper 4 bits read as noise/0xF)
    pub color_ram: [u8; 0x0400],

    /// 6510 CPU I/O port - Data Direction Register ($0000)
    pub io_port_ddr: u8,
    /// 6510 CPU I/O port - Data Register ($0001)
    /// Bits 0-2 control memory banking:
    ///   Bit 0 (LORAM): 1 = BASIC ROM visible at $A000
    ///   Bit 1 (HIRAM): 1 = KERNAL ROM visible at $E000
    ///   Bit 2 (CHAREN): 1 = I/O visible at $D000, 0 = CHAR ROM at $D000
    pub io_port: u8,

    /// KERNAL ROM (8KB, mapped at $E000–$FFFF)
    pub kernal_rom: Vec<u8>,
    /// BASIC ROM (8KB, mapped at $A000–$BFFF)
    pub basic_rom: Vec<u8>,
    /// Character ROM (4KB, mapped at $D000–$DFFF when CHAREN=0)
    pub char_rom: Vec<u8>,

    /// VIC-II video chip (shared with system)
    pub vic: Rc<RefCell<Vic>>,
    /// SID audio chip (shared with system)
    pub sid: Rc<RefCell<Sid>>,
    /// CIA 1 - keyboard/joystick/IRQ (shared with system)
    pub cia1: Rc<RefCell<Cia>>,
    /// CIA 2 - VIC bank/serial/NMI (shared with system)
    pub cia2: Rc<RefCell<Cia>>,
}

impl C64Bus {
    pub fn new(
        vic: Rc<RefCell<Vic>>,
        sid: Rc<RefCell<Sid>>,
        cia1: Rc<RefCell<Cia>>,
        cia2: Rc<RefCell<Cia>>,
    ) -> Self {
        Self {
            ram: [0u8; 0x10000],
            color_ram: [0u8; 0x0400],
            io_port_ddr: 0x2F, // Default: bits 0-3,5 are output
            io_port: 0x37,     // Default: all ROM/IO visible
            kernal_rom: make_stub_kernal(),
            basic_rom: make_stub_basic(),
            char_rom: make_default_char_rom(),
            vic,
            sid,
            cia1,
            cia2,
        }
    }

    /// Load KERNAL ROM data (8KB)
    pub fn load_kernal(&mut self, data: Vec<u8>) {
        self.kernal_rom = data;
    }

    /// Load BASIC ROM data (8KB)
    pub fn load_basic(&mut self, data: Vec<u8>) {
        self.basic_rom = data;
    }

    /// Load character ROM data (4KB)
    pub fn load_char_rom(&mut self, data: Vec<u8>) {
        self.char_rom = data;
    }

    /// Load a .PRG file: first 2 bytes are load address (little-endian), rest is data
    pub fn load_prg(&mut self, data: &[u8]) {
        if data.len() < 2 {
            return;
        }
        let load_addr = (data[0] as u16) | ((data[1] as u16) << 8);
        let payload = &data[2..];
        for (i, &byte) in payload.iter().enumerate() {
            let addr = load_addr.wrapping_add(i as u16);
            self.ram[addr as usize] = byte;
        }
    }

    pub(crate) fn loram(&self) -> bool {
        self.io_port & 0x01 != 0
    }
    pub(crate) fn hiram(&self) -> bool {
        self.io_port & 0x02 != 0
    }
    pub(crate) fn charen(&self) -> bool {
        self.io_port & 0x04 != 0
    }

    /// Read a byte for the debugger without side effects.
    ///
    /// Identical to [`Memory6502::read`] for most addresses.  For the I/O area
    /// ($D000–$DFFF) the underlying RAM byte is returned instead of routing to
    /// the live chip registers.  This avoids clearing latched registers such as
    /// the VIC-II sprite-collision latches ($D01E/$D01F) or CIA ICR status
    /// simply because the user opened the memory viewer.
    pub fn peek(&self, addr: u16) -> u8 {
        use emu_core::cpu_6502::Memory6502;
        match addr {
            // 6510 I/O port — no side effects, same as normal read
            0x0000 => self.io_port_ddr,
            0x0001 => {
                let output_bits = self.io_port & self.io_port_ddr;
                let input_bits = !self.io_port_ddr & 0x37;
                output_bits | input_bits
            }
            // I/O / Char ROM area: return RAM to avoid chip side effects
            0xD000..=0xDFFF => {
                if self.charen() && (self.hiram() || self.loram()) {
                    // I/O visible — read from RAM instead of live I/O bus
                    self.ram[addr as usize]
                } else if !self.charen() && (self.hiram() || self.loram()) {
                    // Char ROM visible
                    let off = (addr - 0xD000) as usize;
                    if off < self.char_rom.len() {
                        self.char_rom[off]
                    } else {
                        self.ram[addr as usize]
                    }
                } else {
                    self.ram[addr as usize]
                }
            }
            // All other addresses: delegate to the standard (side-effect-free) read
            _ => self.read(addr),
        }
    }

    /// Read from I/O area ($D000–$DFFF)
    fn io_read(&self, addr: u16) -> u8 {
        match addr {
            0xD000..=0xD3FF => {
                // VIC-II (mirrored every 64 bytes)
                let reg = ((addr - 0xD000) & 0x3F) as u8;
                self.vic.borrow_mut().read_reg(reg)
            }
            0xD400..=0xD7FF => {
                // SID (mirrored every 32 bytes)
                let reg = ((addr - 0xD400) & 0x1F) as u8;
                self.sid.borrow().read_reg(reg)
            }
            0xD800..=0xDBFF => {
                // Color RAM (only lower 4 bits valid, upper return random-ish)
                let idx = (addr - 0xD800) as usize;
                if idx < self.color_ram.len() {
                    (self.color_ram[idx] & 0x0F) | 0xF0
                } else {
                    0xFF
                }
            }
            0xDC00..=0xDCFF => {
                // CIA 1 (mirrored every 16 bytes)
                let reg = ((addr - 0xDC00) & 0x0F) as u8;
                self.cia1.borrow_mut().read(reg)
            }
            0xDD00..=0xDDFF => {
                // CIA 2 (mirrored every 16 bytes)
                let reg = ((addr - 0xDD00) & 0x0F) as u8;
                self.cia2.borrow_mut().read(reg)
            }
            0xDE00..=0xDFFF => {
                // I/O expansion area (no device present)
                0xFF
            }
            _ => 0xFF,
        }
    }

    /// Write to I/O area ($D000–$DFFF)
    fn io_write(&mut self, addr: u16, val: u8) {
        match addr {
            0xD000..=0xD3FF => {
                let reg = ((addr - 0xD000) & 0x3F) as u8;
                self.vic.borrow_mut().write_reg(reg, val);
            }
            0xD400..=0xD7FF => {
                let reg = ((addr - 0xD400) & 0x1F) as u8;
                self.sid.borrow_mut().write_reg(reg, val);
            }
            0xD800..=0xDBFF => {
                let idx = (addr - 0xD800) as usize;
                if idx < self.color_ram.len() {
                    self.color_ram[idx] = val & 0x0F;
                }
            }
            0xDC00..=0xDCFF => {
                let reg = ((addr - 0xDC00) & 0x0F) as u8;
                self.cia1.borrow_mut().write(reg, val);
            }
            0xDD00..=0xDDFF => {
                let reg = ((addr - 0xDD00) & 0x0F) as u8;
                self.cia2.borrow_mut().write(reg, val);
            }
            _ => {} // I/O expansion: writes ignored
        }
    }
}

impl Memory6502 for C64Bus {
    fn read(&self, addr: u16) -> u8 {
        match addr {
            // 6510 I/O port
            0x0000 => self.io_port_ddr,
            0x0001 => {
                // Bits where DDR=0 are input (float high on C64 with no datasette)
                let output_bits = self.io_port & self.io_port_ddr;
                let input_bits = !self.io_port_ddr & 0x37; // Input bits read as pulled-up
                output_bits | input_bits
            }

            // Always RAM
            0x0002..=0x9FFF => self.ram[addr as usize],

            // BASIC ROM or RAM
            0xA000..=0xBFFF => {
                if self.loram() && self.hiram() {
                    let off = (addr - 0xA000) as usize;
                    if off < self.basic_rom.len() {
                        self.basic_rom[off]
                    } else {
                        self.ram[addr as usize]
                    }
                } else {
                    self.ram[addr as usize]
                }
            }

            // Always RAM
            0xC000..=0xCFFF => self.ram[addr as usize],

            // I/O, CHAR ROM, or RAM
            0xD000..=0xDFFF => {
                if self.charen() && (self.hiram() || self.loram()) {
                    // I/O visible
                    self.io_read(addr)
                } else if !self.charen() && (self.hiram() || self.loram()) {
                    // Character ROM visible
                    let off = (addr - 0xD000) as usize;
                    if off < self.char_rom.len() {
                        self.char_rom[off]
                    } else {
                        self.ram[addr as usize]
                    }
                } else {
                    // All ROM/IO disabled - see through to RAM
                    self.ram[addr as usize]
                }
            }

            // KERNAL ROM or RAM
            0xE000..=0xFFFF => {
                if self.hiram() {
                    let off = (addr - 0xE000) as usize;
                    if off < self.kernal_rom.len() {
                        self.kernal_rom[off]
                    } else {
                        self.ram[addr as usize]
                    }
                } else {
                    self.ram[addr as usize]
                }
            }
        }
    }

    fn write(&mut self, addr: u16, val: u8) {
        match addr {
            // 6510 I/O port
            0x0000 => self.io_port_ddr = val,
            0x0001 => {
                // Only bits where DDR=1 can be written
                self.io_port = (val & self.io_port_ddr) | (self.io_port & !self.io_port_ddr);
            }

            // I/O area: writes always go to I/O when I/O is visible
            0xD000..=0xDFFF => {
                if self.charen() && (self.hiram() || self.loram()) {
                    self.io_write(addr, val);
                }
                // Writes always go to RAM underneath as well
                self.ram[addr as usize] = val;
            }

            // All other writes always go to RAM
            // (ROM areas are write-through to underlying RAM)
            _ => {
                self.ram[addr as usize] = val;
            }
        }
    }
}

/// Create a minimal stub KERNAL ROM for testing without real ROMs.
/// Provides reset/NMI/IRQ vectors and a simple infinite loop.
pub fn make_stub_kernal() -> Vec<u8> {
    let mut rom = vec![0xEA_u8; 0x2000]; // NOP sled

    // Reset entry point at $FCE2 (standard C64 KERNAL reset vector target)
    let reset_addr: u16 = 0xFCE2;
    let offset = (reset_addr - 0xE000) as usize;

    // Simple initialization sequence:
    // SEI          ; Disable interrupts
    // LDX #$FF     ; Init stack pointer
    // TXS
    // CLD          ; Clear decimal mode
    // LDA #$37     ; Default memory config
    // STA $01
    // JMP $FCE2    ; Loop (or JMP to self for simplicity)
    rom[offset] = 0x78; // SEI
    rom[offset + 1] = 0xA2; // LDX #$FF
    rom[offset + 2] = 0xFF;
    rom[offset + 3] = 0x9A; // TXS
    rom[offset + 4] = 0xD8; // CLD
    rom[offset + 5] = 0xA9; // LDA #$37
    rom[offset + 6] = 0x37;
    rom[offset + 7] = 0x85; // STA $01
    rom[offset + 8] = 0x01;
    // Jump to infinite loop
    let loop_addr = reset_addr + 9;
    rom[offset + 9] = 0x4C; // JMP loop_addr
    rom[offset + 10] = (loop_addr & 0xFF) as u8;
    rom[offset + 11] = (loop_addr >> 8) as u8;

    // IRQ/BRK handler: simple RTI
    let irq_addr: u16 = 0xFF48;
    let irq_offset = (irq_addr - 0xE000) as usize;
    rom[irq_offset] = 0x40; // RTI

    // NMI handler: simple RTI
    let nmi_addr: u16 = 0xFE47;
    let nmi_offset = (nmi_addr - 0xE000) as usize;
    rom[nmi_offset] = 0x40; // RTI

    // Set vectors
    let reset_vec = (0xFFFC - 0xE000) as usize;
    rom[reset_vec] = (reset_addr & 0xFF) as u8;
    rom[reset_vec + 1] = (reset_addr >> 8) as u8;

    let nmi_vec = (0xFFFA - 0xE000) as usize;
    rom[nmi_vec] = (nmi_addr & 0xFF) as u8;
    rom[nmi_vec + 1] = (nmi_addr >> 8) as u8;

    let irq_vec = (0xFFFE - 0xE000) as usize;
    rom[irq_vec] = (irq_addr & 0xFF) as u8;
    rom[irq_vec + 1] = (irq_addr >> 8) as u8;

    rom
}

/// Create a minimal stub BASIC ROM
pub fn make_stub_basic() -> Vec<u8> {
    vec![0xEA_u8; 0x2000]
}

/// Create a default character ROM with basic ASCII characters
/// This generates a minimal 4KB character ROM for testing
pub fn make_default_char_rom() -> Vec<u8> {
    let mut rom = vec![0u8; 0x1000];

    // Define basic font patterns for printable ASCII characters
    // Each character is 8 bytes (8x8 pixels)
    // We define a minimal set for letters/digits
    let font_data: &[(u8, [u8; 8])] = &[
        // Space (0x20)
        (0x20, [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]),
        // 'A' (0x01 in PETSCII screen codes)
        (0x01, [0x3C, 0x66, 0x66, 0x7E, 0x66, 0x66, 0x66, 0x00]),
        // 'B'
        (0x02, [0x7C, 0x66, 0x66, 0x7C, 0x66, 0x66, 0x7C, 0x00]),
        // 'C'
        (0x03, [0x3C, 0x66, 0x60, 0x60, 0x60, 0x66, 0x3C, 0x00]),
        // 'D'
        (0x04, [0x78, 0x6C, 0x66, 0x66, 0x66, 0x6C, 0x78, 0x00]),
        // 'E'
        (0x05, [0x7E, 0x60, 0x60, 0x78, 0x60, 0x60, 0x7E, 0x00]),
        // '@' (0x00 in PETSCII)
        (0x00, [0x3C, 0x66, 0x6E, 0x6A, 0x6E, 0x60, 0x3C, 0x00]),
    ];

    for &(code, ref pattern) in font_data {
        let offset = code as usize * 8;
        if offset + 8 <= rom.len() {
            rom[offset..offset + 8].copy_from_slice(pattern);
        }
    }

    rom
}
