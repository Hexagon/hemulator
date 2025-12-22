//! N64 memory bus implementation

use crate::cartridge::Cartridge;
use crate::mi::MipsInterface;
use crate::pif::Pif;
use crate::rdp::Rdp;
use crate::rsp::Rsp;
use crate::vi::VideoInterface;
use crate::N64Error;
use emu_core::cpu_mips_r4300i::MemoryMips;

/// N64 memory bus
pub struct N64Bus {
    /// 4MB RDRAM
    rdram: Vec<u8>,
    /// PIF (Peripheral Interface - controllers and boot ROM)
    pif: Pif,
    /// Cartridge (optional)
    cartridge: Option<Cartridge>,
    /// RDP (Reality Display Processor)
    rdp: Rdp,
    /// RSP (Reality Signal Processor)
    rsp: Rsp,
    /// VI (Video Interface)
    vi: VideoInterface,
    /// MI (MIPS Interface - interrupt controller)
    mi: MipsInterface,
}

impl N64Bus {
    pub fn new() -> Self {
        let mut bus = Self {
            rdram: vec![0; 4 * 1024 * 1024], // 4MB
            pif: Pif::new(),
            cartridge: None,
            rdp: Rdp::new(),
            rsp: Rsp::new(),
            vi: VideoInterface::new(),
            mi: MipsInterface::new(),
        };

        // Initialize PIF ROM
        bus.pif.init_rom();

        bus
    }

    /// Update controller state (for input handling)
    pub fn set_controller1(&mut self, state: crate::pif::ControllerState) {
        self.pif.set_controller1(state);
    }

    pub fn set_controller2(&mut self, state: crate::pif::ControllerState) {
        self.pif.set_controller2(state);
    }

    pub fn set_controller3(&mut self, state: crate::pif::ControllerState) {
        self.pif.set_controller3(state);
    }

    pub fn set_controller4(&mut self, state: crate::pif::ControllerState) {
        self.pif.set_controller4(state);
    }

    pub fn load_cartridge(&mut self, data: &[u8]) -> Result<(), N64Error> {
        self.cartridge = Some(Cartridge::load(data)?);
        Ok(())
    }

    pub fn unload_cartridge(&mut self) {
        self.cartridge = None;
    }

    pub fn has_cartridge(&self) -> bool {
        self.cartridge.is_some()
    }

    pub fn cartridge(&self) -> Option<&Cartridge> {
        self.cartridge.as_ref()
    }

    pub fn rdp(&self) -> &Rdp {
        &self.rdp
    }

    pub fn rdp_mut(&mut self) -> &mut Rdp {
        &mut self.rdp
    }

    pub fn rsp(&self) -> &Rsp {
        &self.rsp
    }

    #[allow(dead_code)] // Reserved for future use
    pub fn rsp_mut(&mut self) -> &mut Rsp {
        &mut self.rsp
    }

    #[allow(dead_code)] // Reserved for future use when VI is integrated with frame rendering
    pub fn vi(&self) -> &VideoInterface {
        &self.vi
    }

    #[allow(dead_code)] // Reserved for future use when VI is integrated with frame rendering
    pub fn vi_mut(&mut self) -> &mut VideoInterface {
        &mut self.vi
    }

    pub fn mi(&self) -> &MipsInterface {
        &self.mi
    }

    pub fn mi_mut(&mut self) -> &mut MipsInterface {
        &mut self.mi
    }

    /// Execute pending RSP task if RSP is not halted
    pub fn process_rsp_task(&mut self) {
        // Clone RDRAM reference to avoid borrow checker issues
        let rdram_clone = self.rdram.clone();
        self.rsp.execute_task(&rdram_clone, &mut self.rdp);
    }

    /// Process pending RDP display list if needed
    pub fn process_rdp_display_list(&mut self) {
        if self.rdp.needs_processing() {
            self.rdp.process_display_list(&self.rdram);
        }
    }

    fn translate_address(&self, addr: u32) -> u32 {
        // Simple address translation (unmapped addresses)
        addr & 0x1FFFFFFF
    }
}

impl Default for N64Bus {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryMips for N64Bus {
    fn read_byte(&self, addr: u32) -> u8 {
        let phys_addr = self.translate_address(addr);

        match phys_addr {
            // RDRAM (0x00000000 - 0x003FFFFF)
            0x0000_0000..=0x003F_FFFF => self.rdram[(phys_addr & 0x003FFFFF) as usize],
            // SP DMEM (0x04000000 - 0x04000FFF)
            0x0400_0000..=0x0400_0FFF => {
                let offset = phys_addr & 0xFFF;
                self.rsp.read_dmem(offset)
            }
            // SP IMEM (0x04001000 - 0x04001FFF)
            0x0400_1000..=0x0400_1FFF => {
                let offset = phys_addr & 0xFFF;
                self.rsp.read_imem(offset)
            }
            // PIF RAM (0x1FC00000 - 0x1FC007FF)
            0x1FC0_0000..=0x1FC0_07FF => {
                let offset = phys_addr & 0x7FF;
                self.pif.read_ram(offset)
            }
            // Cartridge ROM (0x10000000 - 0x1FBFFFFF)
            0x1000_0000..=0x1FBF_FFFF => {
                if let Some(ref cart) = self.cartridge {
                    cart.read(phys_addr - 0x1000_0000)
                } else {
                    0
                }
            }
            _ => 0,
        }
    }

    fn read_halfword(&self, addr: u32) -> u16 {
        let b0 = self.read_byte(addr);
        let b1 = self.read_byte(addr + 1);
        u16::from_be_bytes([b0, b1])
    }

    fn read_word(&self, addr: u32) -> u32 {
        let phys_addr = self.translate_address(addr);

        match phys_addr {
            // RDRAM
            0x0000_0000..=0x003F_FFFF => {
                let offset = (phys_addr & 0x003FFFFF) as usize;
                u32::from_be_bytes([
                    self.rdram[offset],
                    self.rdram[offset + 1],
                    self.rdram[offset + 2],
                    self.rdram[offset + 3],
                ])
            }
            // RSP registers (0x04040000 - 0x0404001F)
            0x0404_0000..=0x0404_001F => {
                let offset = phys_addr & 0x1F;
                self.rsp.read_register(offset)
            }
            // RDP Command registers (0x04100000 - 0x0410001F)
            0x0410_0000..=0x0410_001F => {
                let offset = phys_addr & 0x1F;
                self.rdp.read_register(offset)
            }
            // MI registers (0x04300000 - 0x0430000F)
            0x0430_0000..=0x0430_000F => {
                let offset = phys_addr & 0x0F;
                self.mi.read_register(offset)
            }
            // VI registers (0x04400000 - 0x04400037)
            0x0440_0000..=0x0440_0037 => {
                let offset = phys_addr & 0x3F;
                self.vi.read_register(offset)
            }
            // Cartridge ROM
            0x1000_0000..=0x1FBF_FFFF => {
                if let Some(ref cart) = self.cartridge {
                    let offset = phys_addr - 0x1000_0000;
                    u32::from_be_bytes([
                        cart.read(offset),
                        cart.read(offset + 1),
                        cart.read(offset + 2),
                        cart.read(offset + 3),
                    ])
                } else {
                    0
                }
            }
            _ => {
                let b0 = self.read_byte(addr);
                let b1 = self.read_byte(addr + 1);
                let b2 = self.read_byte(addr + 2);
                let b3 = self.read_byte(addr + 3);
                u32::from_be_bytes([b0, b1, b2, b3])
            }
        }
    }

    fn read_doubleword(&self, addr: u32) -> u64 {
        let hi = self.read_word(addr) as u64;
        let lo = self.read_word(addr + 4) as u64;
        (hi << 32) | lo
    }

    fn write_byte(&mut self, addr: u32, val: u8) {
        let phys_addr = self.translate_address(addr);

        match phys_addr {
            // RDRAM
            0x0000_0000..=0x003F_FFFF => {
                self.rdram[(phys_addr & 0x003FFFFF) as usize] = val;
            }
            // SP DMEM (0x04000000 - 0x04000FFF)
            0x0400_0000..=0x0400_0FFF => {
                let offset = phys_addr & 0xFFF;
                self.rsp.write_dmem(offset, val);
            }
            // SP IMEM (0x04001000 - 0x04001FFF)
            0x0400_1000..=0x0400_1FFF => {
                let offset = phys_addr & 0xFFF;
                self.rsp.write_imem(offset, val);
            }
            // PIF RAM
            0x1FC0_0000..=0x1FC0_07FF => {
                let offset = phys_addr & 0x7FF;
                self.pif.write_ram(offset, val);
            }
            _ => {}
        }
    }

    fn write_halfword(&mut self, addr: u32, val: u16) {
        let bytes = val.to_be_bytes();
        self.write_byte(addr, bytes[0]);
        self.write_byte(addr + 1, bytes[1]);
    }

    fn write_word(&mut self, addr: u32, val: u32) {
        let phys_addr = self.translate_address(addr);

        match phys_addr {
            // RDRAM
            0x0000_0000..=0x003F_FFFF => {
                let offset = (phys_addr & 0x003FFFFF) as usize;
                let bytes = val.to_be_bytes();
                self.rdram[offset] = bytes[0];
                self.rdram[offset + 1] = bytes[1];
                self.rdram[offset + 2] = bytes[2];
                self.rdram[offset + 3] = bytes[3];
            }
            // RSP registers (0x04040000 - 0x0404001F)
            0x0404_0000..=0x0404_001F => {
                let offset = phys_addr & 0x1F;
                self.rsp.write_register(offset, val, &mut self.rdram);

                // If SP_STATUS was written (offset 0x10), check if RSP was un-halted
                // and execute pending task
                if offset == 0x10 {
                    self.process_rsp_task();
                }
            }
            // RDP Command registers (0x04100000 - 0x0410001F)
            0x0410_0000..=0x0410_001F => {
                let offset = phys_addr & 0x1F;
                self.rdp.write_register(offset, val);

                // If DPC_END was written (offset 0x04), process the display list
                if offset == 0x04 {
                    self.process_rdp_display_list();
                }
            }
            // MI registers (0x04300000 - 0x0430000F)
            0x0430_0000..=0x0430_000F => {
                let offset = phys_addr & 0x0F;
                self.mi.write_register(offset, val);
            }
            // VI registers (0x04400000 - 0x04400037)
            0x0440_0000..=0x0440_0037 => {
                let offset = phys_addr & 0x3F;
                self.vi.write_register(offset, val);
            }
            _ => {
                let bytes = val.to_be_bytes();
                self.write_byte(addr, bytes[0]);
                self.write_byte(addr + 1, bytes[1]);
                self.write_byte(addr + 2, bytes[2]);
                self.write_byte(addr + 3, bytes[3]);
            }
        }
    }

    fn write_doubleword(&mut self, addr: u32, val: u64) {
        let hi = (val >> 32) as u32;
        let lo = val as u32;
        self.write_word(addr, hi);
        self.write_word(addr + 4, lo);
    }
}
