//! N64 memory bus implementation

use crate::ai::AudioInterface;
use crate::cartridge::Cartridge;
use crate::mi::MipsInterface;
use crate::pif::Pif;
use crate::rdp::Rdp;
use crate::rsp::Rsp;
use crate::tlb::Tlb;
use crate::vi::VideoInterface;
use crate::N64Error;
use emu_core::cpu_mips_r4300i::MemoryMips;
use emu_core::logging::{log, LogCategory, LogLevel};

/// N64 memory bus
pub struct N64Bus {
    /// 4MB RDRAM
    rdram: Vec<u8>,
    /// PIF (Peripheral Interface - controllers and boot ROM)
    pif: Pif,
    /// Cartridge (optional)
    cartridge: Option<Cartridge>,
    /// Cartridge save storage (32 KB for SRAM, 128 KB for FlashRAM placeholder).
    /// Named `cart_save` because it holds both SRAM and the FlashRAM byte-array
    /// stand-in; the two are physically different chips with different access
    /// protocols, but share the same address space at 0x08000000.
    cart_save: Option<Vec<u8>>,
    /// RDP (Reality Display Processor)
    rdp: Rdp,
    /// RSP (Reality Signal Processor)
    rsp: Rsp,
    /// VI (Video Interface)
    vi: VideoInterface,
    /// MI (MIPS Interface - interrupt controller)
    mi: MipsInterface,
    /// AI (Audio Interface)
    ai: AudioInterface,
    /// TLB (Translation Lookaside Buffer)
    tlb: Tlb,
    /// Entry point from ROM header (set during cartridge load)
    entry_point: Option<u64>,
}

impl N64Bus {
    /// Create a new N64 bus with OpenGL renderer
    /// Requires a GL context for hardware-accelerated rendering
    pub fn new(gl: glow::Context) -> Result<Self, String> {
        let rdp = Rdp::new(gl)?;

        let mut bus = Self {
            rdram: vec![0; 4 * 1024 * 1024], // 4MB
            pif: Pif::new(),
            cartridge: None,
            cart_save: None,
            rdp,
            rsp: Rsp::new(),
            vi: VideoInterface::new(),
            mi: MipsInterface::new(),
            ai: AudioInterface::new(),
            tlb: Tlb::new(),
            entry_point: None,
        };

        // Initialize PIF ROM
        bus.pif.init_rom();

        Ok(bus)
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
        log(LogCategory::Bus, LogLevel::Info, || {
            format!("N64 Bus: Loading cartridge, size={} bytes", data.len())
        });

        // Load the cartridge
        let cart = Cartridge::load(data)?;

        // Auto-detect and configure save type
        let save_type = cart.save_type();
        log(LogCategory::Bus, LogLevel::Info, || {
            format!("N64 Bus: Detected save type: {:?}", save_type)
        });
        self.configure_save_type(save_type);

        // Perform IPL3 boot sequence - copy ROM to RDRAM and get entry point
        let rom_data = cart.read_range(0, cart.size());
        let entry_point = self.pif.perform_ipl3_boot(&mut self.rdram, &rom_data);

        log(LogCategory::Bus, LogLevel::Info, || {
            format!(
                "N64 Bus: IPL3 boot complete, entry point=0x{:016X}",
                entry_point
            )
        });

        // Store the entry point for CPU reset
        self.cartridge = Some(cart);
        self.entry_point = Some(entry_point);

        log(LogCategory::Bus, LogLevel::Info, || {
            "N64 Bus: Cartridge loaded successfully".to_string()
        });
        Ok(())
    }

    /// Configure the save storage for the given save type.
    ///
    /// Calling this resets both cart save storage and EEPROM to their blank (all-0xFF) state.
    pub fn configure_save_type(&mut self, save_type: crate::cartridge::SaveType) {
        use crate::cartridge::SaveType;
        use crate::pif::EepromType;

        // Reset any existing save storage
        self.cart_save = None;
        self.pif.set_eeprom_type(EepromType::None);

        match save_type {
            SaveType::None => {}
            SaveType::Eeprom4K => {
                self.pif.set_eeprom_type(EepromType::Eeprom4K);
            }
            SaveType::Eeprom16K => {
                self.pif.set_eeprom_type(EepromType::Eeprom16K);
            }
            SaveType::Sram => {
                // 32 KB SRAM, initialised to all-0xFF (blank)
                self.cart_save = Some(vec![0xFF; 32768]);
            }
            SaveType::FlashRam => {
                // 128 KB FlashRAM, initialised to all-0xFF (blank).
                // The Macronix MX29L1100 command protocol (erase/write commands) is not yet
                // implemented — games will not save correctly until it is.
                // See TODO.md N64 section: "FlashRAM command protocol"
                self.cart_save = Some(vec![0xFF; 131072]);
            }
        }
    }

    /// Export save data for persistence.
    ///
    /// Returns `None` when no save storage is configured for the current cartridge.
    pub fn get_save_data(&self) -> Option<Vec<u8>> {
        if let Some(ref buf) = self.cart_save {
            return Some(buf.clone());
        }
        self.pif.save_eeprom()
    }

    /// Import previously persisted save data.
    ///
    /// Returns `Err` if the data length does not match the configured save storage,
    /// if no save storage is configured for the current cartridge, or if another
    /// underlying error occurs (e.g. EEPROM type not set).
    pub fn set_save_data(&mut self, data: Vec<u8>) -> Result<(), String> {
        // SRAM / FlashRAM
        if let Some(ref mut buf) = self.cart_save {
            if data.len() != buf.len() {
                return Err(format!(
                    "Save data size mismatch: expected {} bytes, got {}",
                    buf.len(),
                    data.len()
                ));
            }
            buf.copy_from_slice(&data);
            return Ok(());
        }
        // EEPROM — or no save storage configured at all
        if self.pif.save_eeprom().is_none() {
            return Err("No save storage configured for this cartridge".to_string());
        }
        self.pif.load_eeprom(data)
    }

    /// Get the entry point from the loaded cartridge (for CPU initialization)
    pub fn get_entry_point(&self) -> Option<u64> {
        self.entry_point
    }

    pub fn unload_cartridge(&mut self) {
        self.cartridge = None;
        self.cart_save = None;
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

    #[allow(dead_code)] // Reserved for audio output integration
    pub fn ai(&self) -> &AudioInterface {
        &self.ai
    }

    /// Get mutable reference to Audio Interface for audio output
    pub fn ai_mut(&mut self) -> &mut AudioInterface {
        &mut self.ai
    }

    #[allow(dead_code)] // Reserved for future use
    pub fn mi(&self) -> &MipsInterface {
        &self.mi
    }

    pub fn mi_mut(&mut self) -> &mut MipsInterface {
        &mut self.mi
    }

    /// Execute pending RSP task if RSP is not halted
    /// Returns true if an SP interrupt should be triggered
    pub fn process_rsp_task(&mut self) -> bool {
        use emu_core::logging::{log, LogCategory, LogLevel};
        log(LogCategory::PPU, LogLevel::Info, || {
            "N64 Bus: process_rsp_task() called".to_string()
        });

        // Clone RDRAM reference to avoid borrow checker issues
        let rdram_clone = self.rdram.clone();
        let (_cycles, should_interrupt) = self.rsp.execute_task(&rdram_clone, &mut self.rdp);

        if should_interrupt {
            log(LogCategory::PPU, LogLevel::Info, || {
                "N64 Bus: RSP task complete, triggering SP interrupt".to_string()
            });
            // Set SP interrupt in MI
            self.mi.set_interrupt(super::mi::MI_INTR_SP);
        }

        should_interrupt
    }

    /// Process pending RDP display list if needed
    pub fn process_rdp_display_list(&mut self) {
        if self.rdp.needs_processing() {
            self.rdp.process_display_list(&self.rdram);
        }
    }

    fn translate_address(&self, addr: u32) -> u32 {
        // Use TLB for address translation
        // Convert 32-bit address to 64-bit for TLB lookup
        let virt_addr = addr as u64;

        match self.tlb.translate(virt_addr) {
            Some((phys_addr, _is_cached)) => phys_addr,
            None => {
                // TLB miss - fallback to simple unmapped translation
                // This handles KSEG0/KSEG1 which should already be handled by TLB,
                // but provides a safety net
                addr & 0x1FFFFFFF
            }
        }
    }

    /// Get mutable reference to TLB for CP0 TLB instructions
    #[allow(dead_code)] // Reserved for future CP0 TLB instruction implementation
    pub fn tlb_mut(&mut self) -> &mut Tlb {
        &mut self.tlb
    }
}

impl MemoryMips for N64Bus {
    fn read_byte(&self, addr: u32) -> u8 {
        let phys_addr = self.translate_address(addr);

        log(LogCategory::Bus, LogLevel::Trace, || {
            format!(
                "N64 Bus: Read byte from 0x{:08X} (phys: 0x{:08X})",
                addr, phys_addr
            )
        });

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
            // Cartridge SRAM / FlashRAM (0x08000000 - 0x0FFFFFFF, cartridge domain 2)
            0x0800_0000..=0x0FFF_FFFF => {
                if let Some(ref sram) = self.cart_save {
                    let offset = (phys_addr - 0x0800_0000) as usize;
                    *sram.get(offset).unwrap_or(&0xFF)
                } else {
                    0xFF
                }
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

        log(LogCategory::Bus, LogLevel::Trace, || {
            format!(
                "N64 Bus: Read word from 0x{:08X} (phys: 0x{:08X})",
                addr, phys_addr
            )
        });

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
            // AI registers (0x04500000 - 0x04500017)
            0x0450_0000..=0x0450_0017 => {
                let offset = phys_addr & 0x1F;
                self.ai.read_register(offset)
            }
            // Cartridge SRAM / FlashRAM (0x08000000 - 0x0FFFFFFF, cartridge domain 2)
            0x0800_0000..=0x0FFF_FFFF => {
                if let Some(ref sram) = self.cart_save {
                    let offset = (phys_addr - 0x0800_0000) as usize;
                    u32::from_be_bytes([
                        *sram.get(offset).unwrap_or(&0xFF),
                        *sram.get(offset + 1).unwrap_or(&0xFF),
                        *sram.get(offset + 2).unwrap_or(&0xFF),
                        *sram.get(offset + 3).unwrap_or(&0xFF),
                    ])
                } else {
                    0xFFFF_FFFF
                }
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

        log(LogCategory::Bus, LogLevel::Trace, || {
            format!(
                "N64 Bus: Write byte 0x{:02X} to 0x{:08X} (phys: 0x{:08X})",
                val, addr, phys_addr
            )
        });

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
            // Cartridge SRAM / FlashRAM (0x08000000 - 0x0FFFFFFF)
            0x0800_0000..=0x0FFF_FFFF => {
                if let Some(ref mut sram) = self.cart_save {
                    let offset = (phys_addr - 0x0800_0000) as usize;
                    if offset < sram.len() {
                        sram[offset] = val;
                    }
                }
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

        log(LogCategory::Bus, LogLevel::Trace, || {
            format!(
                "N64 Bus: Write word 0x{:08X} to 0x{:08X} (phys: 0x{:08X})",
                val, addr, phys_addr
            )
        });

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
            // AI registers (0x04500000 - 0x04500017)
            0x0450_0000..=0x0450_0017 => {
                let offset = phys_addr & 0x1F;
                self.ai.write_register(offset, val, &self.rdram);

                // Check if AI interrupt is pending
                if self.ai.is_interrupt_pending() {
                    self.mi.set_interrupt(crate::mi::MI_INTR_AI);
                    log(LogCategory::Interrupts, LogLevel::Info, || {
                        "N64 Bus: AI interrupt triggered".to_string()
                    });
                }
            }
            // Cartridge SRAM / FlashRAM (0x08000000 - 0x0FFFFFFF)
            0x0800_0000..=0x0FFF_FFFF => {
                if let Some(ref mut sram) = self.cart_save {
                    let offset = (phys_addr - 0x0800_0000) as usize;
                    if offset + 3 < sram.len() {
                        let bytes = val.to_be_bytes();
                        sram[offset] = bytes[0];
                        sram[offset + 1] = bytes[1];
                        sram[offset + 2] = bytes[2];
                        sram[offset + 3] = bytes[3];
                    }
                }
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

    // TLB operations for CP0 instructions

    fn tlb_write_indexed(&mut self, index: usize, entry: emu_core::cpu_mips_r4300i::TlbEntryData) {
        // Convert TlbEntryData to TlbEntry
        let tlb_entry = crate::tlb::TlbEntry {
            vpn2: entry.vpn2,
            asid: entry.asid,
            global: entry.global,
            page_mask: entry.page_mask,
            pfn0: entry.pfn0,
            c0: entry.c0,
            d0: entry.d0,
            v0: entry.v0,
            pfn1: entry.pfn1,
            c1: entry.c1,
            d1: entry.d1,
            v1: entry.v1,
        };

        self.tlb.write_entry(index, tlb_entry);

        log(LogCategory::Bus, LogLevel::Debug, || {
            format!(
                "N64 TLB: Write entry at index {} - VPN2=0x{:07X}, ASID=0x{:02X}",
                index, entry.vpn2, entry.asid
            )
        });
    }

    fn tlb_write_random(&mut self, index: usize, entry: emu_core::cpu_mips_r4300i::TlbEntryData) {
        // Use the index from CP0 Random register (passed from CPU)
        // This ensures deterministic behavior for save states and debugging
        self.tlb_write_indexed(index, entry);

        log(LogCategory::Bus, LogLevel::Debug, || {
            format!(
                "N64 TLB: Write entry at random index {} - VPN2=0x{:07X}, ASID=0x{:02X}",
                index, entry.vpn2, entry.asid
            )
        });
    }

    fn tlb_read_indexed(&self, index: usize) -> Option<emu_core::cpu_mips_r4300i::TlbEntryData> {
        self.tlb
            .read_entry(index)
            .map(|entry| emu_core::cpu_mips_r4300i::TlbEntryData {
                vpn2: entry.vpn2,
                asid: entry.asid,
                global: entry.global,
                page_mask: entry.page_mask,
                pfn0: entry.pfn0,
                c0: entry.c0,
                d0: entry.d0,
                v0: entry.v0,
                pfn1: entry.pfn1,
                c1: entry.c1,
                d1: entry.d1,
                v1: entry.v1,
            })
    }

    fn tlb_probe(&self, vpn2: u64, asid: u8) -> Option<usize> {
        // Probe TLB for matching entry
        // VPN2 is bits 39-13 of the virtual address
        // We need to check each entry manually since we can't modify self
        for (i, entry) in self.tlb.entries.iter().enumerate() {
            let mask = (entry.page_mask as u64) << 12;
            let vpn_mask = !mask;
            if (entry.vpn2 & vpn_mask) == (vpn2 & vpn_mask) && (entry.global || entry.asid == asid)
            {
                log(LogCategory::Bus, LogLevel::Debug, || {
                    format!(
                        "N64 TLB: Probe found match at index {} for VPN2=0x{:07X}, ASID=0x{:02X}",
                        i, vpn2, asid
                    )
                });
                return Some(i);
            }
        }

        log(LogCategory::Bus, LogLevel::Debug, || {
            format!(
                "N64 TLB: Probe found no match for VPN2=0x{:07X}, ASID=0x{:02X}",
                vpn2, asid
            )
        });

        None
    }
}
