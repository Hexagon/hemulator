//! Sega Master System memory bus implementation

use crate::vdp::Vdp;
use emu_core::apu::Sn76489Psg;
use emu_core::cpu_z80::MemoryZ80;
use std::cell::RefCell;
use std::rc::Rc;

/// SMS Memory bus
///
/// Memory Map:
/// - 0x0000-0xBFFF: ROM (up to 48KB direct mapped, or banked)
/// - 0xC000-0xDFFF: RAM (8KB)
/// - 0xE000-0xFFFF: RAM mirror
///
/// I/O Ports:
/// - 0x7E/0x7F: PSG
/// - 0xBE: VDP data port
/// - 0xBF: VDP control/status port
/// - 0xDC/0xDD: Controller ports
/// - 0x3E: Memory control (banking)
pub struct SmsMemory {
    // ROM data
    rom: Vec<u8>,

    // RAM (8KB)
    ram: [u8; 0x2000],

    // Shared VDP reference
    vdp: Rc<RefCell<Vdp>>,

    // Shared PSG reference
    psg: Rc<RefCell<Sn76489Psg>>,

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
    pub fn new(
        rom: Vec<u8>,
        vdp: Rc<RefCell<Vdp>>,
        psg: Rc<RefCell<Sn76489Psg>>,
    ) -> Self {
        // Calculate number of 16KB banks
        let num_banks = (rom.len() + 0x3FFF) / 0x4000;

        Self {
            rom,
            ram: [0; 0x2000],
            vdp,
            psg,
            rom_bank_0: 0,
            rom_bank_1: 1,
            rom_bank_2: 2,
            num_banks,
            controller_1: 0xFF,
            controller_2: 0xFF,
            memory_control: 0,
        }
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
}

impl MemoryZ80 for SmsMemory {
    fn read(&self, addr: u16) -> u8 {
        match addr {
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
                if matches!(ram_addr, 0x1FFC | 0x1FFD | 0x1FFE) {
                    self.update_banking();
                }
            }
            _ => {
                // ROM area - ignore writes
            }
        }
    }

    fn io_read(&mut self, port: u8) -> u8 {
        match port {
            0x7E | 0x7F => {
                // V-counter (0x7E) / H-counter (0x7F) - both read VDP vcounter for now
                self.vdp.borrow().read_vcounter()
            }
            0xBE => {
                // VDP data port
                self.vdp.borrow_mut().read_data()
            }
            0xBF => {
                // VDP control/status port
                self.vdp.borrow_mut().read_status()
            }
            0xDC => {
                // Controller port 1
                self.controller_1
            }
            0xDD => {
                // Controller port 2
                self.controller_2
            }
            _ => 0xFF,
        }
    }

    fn io_write(&mut self, port: u8, val: u8) {
        match port {
            0x7E | 0x7F => {
                // PSG write
                self.psg.borrow_mut().write(val);
            }
            0xBE => {
                // VDP data port
                self.vdp.borrow_mut().write_data(val);
            }
            0xBF => {
                // VDP control port
                self.vdp.borrow_mut().write_control(val);
            }
            0x3E => {
                // Memory control register
                self.memory_control = val;
                // TODO: Implement memory control features (cartridge slot control, etc.)
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
    use emu_core::apu::TimingMode;

    #[test]
    fn test_ram_read_write() {
        let vdp = Rc::new(RefCell::new(Vdp::new()));
        let psg = Rc::new(RefCell::new(Sn76489Psg::new(TimingMode::Ntsc)));
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
        let psg = Rc::new(RefCell::new(Sn76489Psg::new(TimingMode::Ntsc)));
        let mut rom = vec![0; 0x8000];
        rom[0x100] = 0xAB;

        let mem = SmsMemory::new(rom, vdp, psg);

        assert_eq!(mem.read(0x100), 0xAB);
    }

    #[test]
    fn test_banking() {
        let vdp = Rc::new(RefCell::new(Vdp::new()));
        let psg = Rc::new(RefCell::new(Sn76489Psg::new(TimingMode::Ntsc)));
        
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
