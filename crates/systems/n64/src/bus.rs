//! N64 memory bus implementation

use crate::cartridge::Cartridge;
use crate::rdp::Rdp;
use crate::N64Error;
use emu_core::cpu_mips_r4300i::MemoryMips;

/// N64 memory bus
pub struct N64Bus {
    /// 4MB RDRAM
    rdram: Vec<u8>,
    /// 64KB PIF RAM/ROM (boot)
    pif_ram: [u8; 0x800],
    /// 64KB SP DMEM/IMEM (RSP memory)
    sp_mem: [u8; 0x2000],
    /// Cartridge (optional)
    cartridge: Option<Cartridge>,
    /// RDP (Reality Display Processor)
    rdp: Rdp,
}

impl N64Bus {
    pub fn new() -> Self {
        let mut bus = Self {
            rdram: vec![0; 4 * 1024 * 1024], // 4MB
            pif_ram: [0; 0x800],
            sp_mem: [0; 0x2000],
            cartridge: None,
            rdp: Rdp::new(),
        };

        // Initialize minimal PIF ROM stub for booting
        // This stub copies cartridge boot code to RDRAM and jumps to it
        bus.init_pif_rom();

        bus
    }

    /// Initialize minimal PIF ROM stub
    /// Real N64 PIF ROM is complex, but we only need basic boot functionality
    fn init_pif_rom(&mut self) {
        // PIF ROM starts at 0xBFC00000 (physical 0x1FC00000)
        // We'll put a simple boot loader that:
        // 1. Copies 1MB from cartridge ROM (0x10000000) to RDRAM (0x00000000)
        // 2. Jumps to entry point at 0x80000400 (RDRAM + 0x400, cached)

        let pif_rom: Vec<u32> = vec![
            // Copy cartridge to RDRAM (simplified - just set up registers and jump)
            // In reality, PIF ROM does CRC checks, copies specific regions, etc.

            // li $t0, 0x10000000  # Source: ROM base
            0x3C081000, // lui $t0, 0x1000
            // li $t1, 0x00000000  # Dest: RDRAM base
            0x34090000, // ori $t1, $zero, 0x0000
            // li $t2, 0x00100000  # Size: 1MB
            0x3C0A0010, // lui $t2, 0x0010
            // copy_loop: lw $t3, 0($t0)
            0x8D0B0000, // lw $t3, 0($t0)
            // sw $t3, 0($t1)
            0xAD2B0000, // sw $t3, 0($t1)
            // addiu $t0, $t0, 4
            0x25080004, // addiu $t0, $t0, 4
            // addiu $t1, $t1, 4
            0x25290004, // addiu $t1, $t1, 4
            // addiu $t2, $t2, -4
            0x254AFFFC, // addiu $t2, $t2, -4
            // bne $t2, $zero, copy_loop
            0x1540FFF8, // bne $t2, $zero, -8 instructions
            // nop (delay slot)
            0x00000000,
            // Jump to entry point at 0x80000400
            0x3C088000, // lui $t0, 0x8000
            0x35080400, // ori $t0, $t0, 0x0400
            0x01000008, // jr $t0
            0x00000000, // nop (delay slot)
        ];

        // Write PIF ROM to PIF RAM area (we're using PIF RAM as ROM for simplicity)
        for (i, &instr) in pif_rom.iter().enumerate() {
            let offset = i * 4;
            if offset + 3 < self.pif_ram.len() {
                let bytes = instr.to_be_bytes();
                self.pif_ram[offset] = bytes[0];
                self.pif_ram[offset + 1] = bytes[1];
                self.pif_ram[offset + 2] = bytes[2];
                self.pif_ram[offset + 3] = bytes[3];
            }
        }
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

    pub fn rdp(&self) -> &Rdp {
        &self.rdp
    }

    pub fn rdp_mut(&mut self) -> &mut Rdp {
        &mut self.rdp
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
            // SP DMEM/IMEM (0x04000000 - 0x04001FFF)
            0x0400_0000..=0x0400_1FFF => self.sp_mem[(phys_addr & 0x1FFF) as usize],
            // PIF RAM (0x1FC00000 - 0x1FC007FF)
            0x1FC0_0000..=0x1FC0_07FF => self.pif_ram[(phys_addr & 0x7FF) as usize],
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
            // RDP Command registers (0x04100000 - 0x0410001F)
            0x0410_0000..=0x0410_001F => {
                let offset = phys_addr & 0x1F;
                self.rdp.read_register(offset)
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
            // SP DMEM/IMEM
            0x0400_0000..=0x0400_1FFF => {
                self.sp_mem[(phys_addr & 0x1FFF) as usize] = val;
            }
            // PIF RAM
            0x1FC0_0000..=0x1FC0_07FF => {
                self.pif_ram[(phys_addr & 0x7FF) as usize] = val;
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
            // RDP Command registers (0x04100000 - 0x0410001F)
            0x0410_0000..=0x0410_001F => {
                let offset = phys_addr & 0x1F;
                self.rdp.write_register(offset, val);

                // If DPC_END was written (offset 0x04), process the display list
                if offset == 0x04 {
                    self.process_rdp_display_list();
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
}
