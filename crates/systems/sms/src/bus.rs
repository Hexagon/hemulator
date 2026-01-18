//! Sega Master System memory bus implementation

use crate::psg::SmsPsg;
use crate::vdp::Vdp;
use emu_core::cpu_z80::MemoryZ80;
use emu_core::logging::{log, LogCategory, LogLevel};
use std::cell::RefCell;
use std::rc::Rc;

/// SMS Memory bus
///
/// Memory Map:
/// - 0x0000-0xBFFF: ROM (up to 48KB direct mapped, or banked)
/// - 0xC000-0xDFFF: RAM (8KB)
/// - 0xE000-0xFFFF: RAM mirror
///
/// BIOS Support:
/// - When bit 3 of memory control (port 0x3E) is 0, BIOS is mapped at 0x0000-0x03FF (1KB)
/// - When bit 3 is set, cartridge ROM is mapped starting at 0x0000
///
/// I/O Ports:
/// - 0x7E/0x7F: PSG
/// - 0xBE: VDP data port
/// - 0xBF: VDP control/status port
/// - 0xDC/0xDD: Controller ports
/// - 0x3E: Memory control (banking, BIOS enable)
pub struct SmsMemory {
    // ROM data
    rom: Vec<u8>,

    // BIOS ROM (1KB or 8KB, optional)
    bios: Vec<u8>,

    // RAM (8KB)
    ram: [u8; 0x2000],

    // Shared VDP reference
    vdp: Rc<RefCell<Vdp>>,

    // Shared PSG reference
    psg: Rc<RefCell<SmsPsg>>,

    // Banking registers (for ROMs > 48KB)
    rom_bank_0: usize, // Maps to 0x0000-0x3FFF
    rom_bank_1: usize, // Maps to 0x4000-0x7FFF
    rom_bank_2: usize, // Maps to 0x8000-0xBFFF
    num_banks: usize,

    // Controller state
    controller_1: u8,
    controller_2: u8,

    // Memory control register
    memory_control: u8,
}

impl SmsMemory {
    /// Create a new SMS memory bus
    pub fn new(rom: Vec<u8>, vdp: Rc<RefCell<Vdp>>, psg: Rc<RefCell<SmsPsg>>) -> Self {
        // Calculate number of 16KB banks
        let num_banks = rom.len().div_ceil(0x4000);

        Self {
            rom,
            bios: Vec::new(), // No BIOS by default
            ram: [0; 0x2000],
            vdp,
            psg,
            rom_bank_0: 0,
            rom_bank_1: 1,
            rom_bank_2: 2,
            num_banks,
            controller_1: 0xFF,
            controller_2: 0xFF,
            memory_control: 0x08, // Bit 3 set by default (BIOS disabled)
        }
    }

    /// Load BIOS ROM
    pub fn load_bios(&mut self, bios: Vec<u8>) {
        self.bios = bios;
        // Enable BIOS when explicitly loaded (user wants to use it)
        if !self.bios.is_empty() {
            self.memory_control &= !0x08; // Clear bit 3 to enable BIOS
        } else {
            self.memory_control |= 0x08; // Set bit 3 to disable BIOS when none is loaded
        }
    }

    /// Load cartridge ROM (preserves BIOS)
    pub fn load_rom(&mut self, rom: Vec<u8>) {
        self.rom = rom;
        // Recalculate number of banks
        self.num_banks = self.rom.len().div_ceil(0x4000);
        // Reset bank registers
        self.rom_bank_0 = 0;
        self.rom_bank_1 = 1;
        self.rom_bank_2 = 2;
    }

    /// Check if BIOS is currently enabled
    pub fn is_bios_enabled(&self) -> bool {
        // Bit 3 of memory control: 0 = BIOS enabled, 1 = BIOS disabled
        (self.memory_control & 0x08) == 0 && !self.bios.is_empty()
    }

    /// Update banking configuration
    fn update_banking(&mut self) {
        // Banking registers are at 0xFFFC, 0xFFFD, 0xFFFE in RAM
        let frame_0 = self.ram[0x1FFC] as usize;
        let frame_1 = self.ram[0x1FFD] as usize;
        let frame_2 = self.ram[0x1FFE] as usize;

        // Map banks with wraparound
        self.rom_bank_0 = frame_0 % self.num_banks.max(1);
        self.rom_bank_1 = frame_1 % self.num_banks.max(1);
        self.rom_bank_2 = frame_2 % self.num_banks.max(1);
    }

    /// Set controller 1 state
    pub fn set_controller_1(&mut self, state: u8) {
        self.controller_1 = state;
    }

    /// Set controller 2 state
    pub fn set_controller_2(&mut self, state: u8) {
        self.controller_2 = state;
    }

    // Save state support methods
    /// Get RAM contents for save state
    pub fn get_ram(&self) -> Vec<u8> {
        self.ram.to_vec()
    }

    /// Set RAM contents from save state
    pub fn set_ram(&mut self, data: &[u8]) {
        let len = data.len().min(self.ram.len());
        self.ram[..len].copy_from_slice(&data[..len]);
    }

    /// Get ROM bank 0 index
    pub fn get_rom_bank_0(&self) -> usize {
        self.rom_bank_0
    }

    /// Get ROM bank 1 index
    pub fn get_rom_bank_1(&self) -> usize {
        self.rom_bank_1
    }

    /// Get ROM bank 2 index
    pub fn get_rom_bank_2(&self) -> usize {
        self.rom_bank_2
    }

    /// Get controller 1 state
    pub fn get_controller_1(&self) -> u8 {
        self.controller_1
    }

    /// Get controller 2 state
    pub fn get_controller_2(&self) -> u8 {
        self.controller_2
    }

    /// Get memory control register
    pub fn get_memory_control(&self) -> u8 {
        self.memory_control
    }

    /// Set ROM bank 0 index for save state
    pub fn set_rom_bank_0(&mut self, bank: usize) {
        self.rom_bank_0 = bank % self.num_banks.max(1);
    }

    /// Set ROM bank 1 index for save state
    pub fn set_rom_bank_1(&mut self, bank: usize) {
        self.rom_bank_1 = bank % self.num_banks.max(1);
    }

    /// Set ROM bank 2 index for save state
    pub fn set_rom_bank_2(&mut self, bank: usize) {
        self.rom_bank_2 = bank % self.num_banks.max(1);
    }

    /// Set memory control register for save state
    pub fn set_memory_control(&mut self, value: u8) {
        self.memory_control = value;
    }
}

impl MemoryZ80 for SmsMemory {
    fn read(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x03FF if self.is_bios_enabled() => {
                // BIOS area (1KB) when BIOS is enabled
                self.bios.get(addr as usize).copied().unwrap_or(0xFF)
            }
            0x0000..=0x3FFF => {
                // Bank 0
                let offset = self.rom_bank_0 * 0x4000 + (addr as usize);
                self.rom.get(offset).copied().unwrap_or(0xFF)
            }
            0x4000..=0x7FFF => {
                // Bank 1
                let offset = self.rom_bank_1 * 0x4000 + ((addr & 0x3FFF) as usize);
                self.rom.get(offset).copied().unwrap_or(0xFF)
            }
            0x8000..=0xBFFF => {
                // Bank 2
                let offset = self.rom_bank_2 * 0x4000 + ((addr & 0x3FFF) as usize);
                self.rom.get(offset).copied().unwrap_or(0xFF)
            }
            0xC000..=0xFFFF => {
                // RAM (8KB, mirrored)
                self.ram[(addr & 0x1FFF) as usize]
            }
        }
    }

    fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0xC000..=0xFFFF => {
                // RAM write
                let ram_addr = (addr & 0x1FFF) as usize;
                self.ram[ram_addr] = val;

                // Check if banking registers were updated
                if matches!(ram_addr, 0x1FFC..=0x1FFE) {
                    self.update_banking();
                }
            }
            _ => {
                // ROM area - ignore writes
            }
        }
    }

    fn io_read(&mut self, port: u8) -> u8 {
        // SMS I/O port decoding (partial decoding based on bit patterns):
        // - 0x00-0x3F: Memory control, I/O control, etc.
        // - 0x40-0x7F: V-counter (even) / H-counter (odd)
        // - 0x80-0xBF: VDP data (even) / VDP status (odd)
        // - 0xC0-0xFF: Controller ports (even = port A, odd = port B)
        let value = match port {
            // 0x40-0x7F: V-counter (even ports) / H-counter (odd ports)
            p if (0x40..=0x7F).contains(&p) => {
                // V-counter on even ports, H-counter on odd ports
                // Both currently read vcounter for simplicity
                self.vdp.borrow().read_vcounter()
            }
            // 0x80-0xBF: VDP ports (bit 0 determines data vs control)
            p if (0x80..=0xBF).contains(&p) => {
                if p & 0x01 == 0 {
                    // Even port: VDP data
                    self.vdp.borrow_mut().read_data()
                } else {
                    // Odd port: VDP status
                    self.vdp.borrow_mut().read_status()
                }
            }
            // 0xC0-0xFF: Controller ports
            p if (0xC0..=0xFF).contains(&p) => {
                if p & 0x01 == 0 {
                    // Even port: Controller port 1
                    self.controller_1
                } else {
                    // Odd port: Controller port 2
                    self.controller_2
                }
            }
            _ => 0xFF,
        };

        // Log I/O reads for debugging (only when debug logging is enabled)
        log(LogCategory::Bus, LogLevel::Debug, || {
            format!("SMS I/O: Read port ${:02X} = ${:02X}", port, value)
        });

        value
    }

    fn io_write(&mut self, port: u8, val: u8) {
        // SMS I/O port decoding (partial decoding based on bit patterns):
        // - 0x00-0x3F: Memory control, I/O control, etc.
        // - 0x40-0x7F: PSG write (directly connected to SN76489)
        // - 0x80-0xBF: VDP data (even) / VDP control (odd)
        // - 0xC0-0xFF: Controller ports (directly readable, no writes typically)
        match port {
            // 0x00-0x3F: Memory control registers
            0x3E => {
                // Memory control register
                self.memory_control = val;
            }
            0x3F => {
                // I/O port control (nationalization adapter)
                // Controls TH pin direction for controller ports
                // Not implemented yet
            }
            // 0x40-0x7F: PSG write
            p if (0x40..=0x7F).contains(&p) => {
                self.psg.borrow_mut().write(val);
            }
            // 0x80-0xBF: VDP ports (bit 0 determines data vs control)
            p if (0x80..=0xBF).contains(&p) => {
                if p & 0x01 == 0 {
                    // Even port: VDP data
                    self.vdp.borrow_mut().write_data(val);
                } else {
                    // Odd port: VDP control
                    self.vdp.borrow_mut().write_control(val);
                }
            }
            _ => {
                // Ignore writes to other ports
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::psg::SmsPsg;

    #[test]
    fn test_ram_read_write() {
        let vdp = Rc::new(RefCell::new(Vdp::new()));
        let psg = Rc::new(RefCell::new(SmsPsg::new()));
        let rom = vec![0; 0x8000];
        let mut mem = SmsMemory::new(rom, vdp, psg);

        // Write to RAM
        mem.write(0xC000, 0x42);
        assert_eq!(mem.read(0xC000), 0x42);

        // Check RAM mirror
        assert_eq!(mem.read(0xE000), 0x42);
    }

    #[test]
    fn test_rom_read() {
        let vdp = Rc::new(RefCell::new(Vdp::new()));
        let psg = Rc::new(RefCell::new(SmsPsg::new()));
        let mut rom = vec![0; 0x8000];
        rom[0x100] = 0xAB;

        let mem = SmsMemory::new(rom, vdp, psg);

        assert_eq!(mem.read(0x100), 0xAB);
    }

    #[test]
    fn test_banking() {
        let vdp = Rc::new(RefCell::new(Vdp::new()));
        let psg = Rc::new(RefCell::new(SmsPsg::new()));

        // Create 128KB ROM (8 banks of 16KB)
        let mut rom = vec![0; 0x20000];
        // Mark each bank with its number
        for i in 0..8 {
            rom[i * 0x4000] = i as u8;
        }

        let mut mem = SmsMemory::new(rom, vdp, psg);

        // Initially bank 0, 1, 2 should be mapped
        assert_eq!(mem.read(0x0000), 0);
        assert_eq!(mem.read(0x4000), 1);
        assert_eq!(mem.read(0x8000), 2);

        // Switch bank 2 to bank 5
        mem.write(0xFFFE, 5);
        assert_eq!(mem.read(0x8000), 5);
    }
}
