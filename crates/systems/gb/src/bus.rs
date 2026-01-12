//! Game Boy memory bus implementation
//!
//! The memory bus connects all components of the Game Boy system and handles
//! address decoding and routing. It implements the complete Game Boy memory map
//! and provides access to ROM, RAM, VRAM, OAM, and I/O registers.
//!
//! # Memory Map
//!
//! ```text
//! $0000-$00FF  Boot ROM (disabled after boot via $FF50)
//! $0000-$3FFF  ROM Bank 0 (16KB, fixed)
//! $4000-$7FFF  ROM Bank 1-N (16KB, switchable via MBC)
//! $8000-$9FFF  Video RAM (8KB)
//! $A000-$BFFF  External RAM (8KB, switchable via MBC, battery-backed)
//! $C000-$CFFF  Work RAM Bank 0 (4KB)
//! $D000-$DFFF  Work RAM Bank 1 (4KB) [CGB: Banks 1-7 switchable]
//! $E000-$FDFF  Echo RAM (mirror of $C000-$DDFF)
//! $FE00-$FE9F  OAM - Object Attribute Memory (160 bytes, 40 sprites × 4 bytes)
//! $FEA0-$FEFF  Not usable
//! $FF00-$FF7F  I/O Registers
//! $FF80-$FFFE  High RAM (127 bytes, fast RAM)
//! $FFFF        Interrupt Enable Register
//! ```
//!
//! # I/O Registers
//!
//! ## Joypad
//! - `$FF00 (P1)`: Joypad register
//!   - Bit 5: Select button keys (0=select)
//!   - Bit 4: Select direction keys (0=select)
//!   - Bits 3-0: Input (0=pressed, 1=released)
//!
//! ## Serial Transfer
//! - `$FF01 (SB)`: Serial transfer data
//! - `$FF02 (SC)`: Serial transfer control
//!
//! ## Timer
//! - `$FF04 (DIV)`: Divider register
//! - `$FF05 (TIMA)`: Timer counter
//! - `$FF06 (TMA)`: Timer modulo
//! - `$FF07 (TAC)`: Timer control
//!
//! ## Interrupts
//! - `$FF0F (IF)`: Interrupt flag
//! - `$FFFF (IE)`: Interrupt enable
//!
//! ## PPU Registers
//! - `$FF40 (LCDC)`: LCD control
//! - `$FF41 (STAT)`: LCD status
//! - `$FF42 (SCY)`: Scroll Y
//! - `$FF43 (SCX)`: Scroll X
//! - `$FF44 (LY)`: LCD Y coordinate (read-only)
//! - `$FF45 (LYC)`: LY compare
//! - `$FF47 (BGP)`: Background palette
//! - `$FF48 (OBP0)`: Object palette 0
//! - `$FF49 (OBP1)`: Object palette 1
//! - `$FF4A (WY)`: Window Y position
//! - `$FF4B (WX)`: Window X position
//!
//! ## Other
//! - `$FF50`: Boot ROM disable (write 1 to disable)
//!
//! # MBC (Memory Bank Controllers)
//!
//! MBCs allow games to use more than 32KB of ROM by bank switching.
//! Writes to ROM address space trigger MBC commands.
//!
//! ## Implemented
//! - MBC0: No mapper (32KB ROM max)
//! - MBC1: Most common (up to 2MB ROM, 32KB RAM)
//! - MBC3: With RTC support (up to 2MB ROM, 32KB RAM)
//! - MBC5: For larger ROMs (up to 8MB ROM, 128KB RAM)
//!
//! # Current Implementation
//!
//! ## Implemented
//! - ✅ Full memory map with proper mirroring
//! - ✅ VRAM access via PPU (8KB)
//! - ✅ OAM access via PPU (160 bytes)
//! - ✅ Work RAM (32KB with banking in CGB mode)
//! - ✅ High RAM (127 bytes)
//! - ✅ Joypad register with matrix selection
//! - ✅ PPU registers (LCDC, STAT, palettes, scroll, etc.)
//! - ✅ APU registers (sound channels, master controls, wave RAM)
//! - ✅ Timer registers (DIV, TIMA, TMA, TAC)
//! - ✅ Interrupt registers (IF, IE)
//! - ✅ Boot ROM disable register
//! - ✅ Cartridge ROM loading (up to size)
//! - ✅ Cartridge RAM with size detection
//! - ✅ MBC0, MBC1, MBC2, MBC3, MBC5, HuC1 mappers
//! - ✅ OAM DMA transfer (0xFF46)
//! - ✅ CGB-specific registers (VBK, BCPS/BCPD, OCPS/OCPD, KEY1, SVBK)
//! - ✅ Speed switching (KEY1 at 0xFF4D, CGB only)
//! - ✅ WRAM banking (SVBK at 0xFF70, CGB only)
//! - ✅ Serial transfer (0xFF01, 0xFF02, with loopback mode)
//! - ✅ HDMA (0xFF51-0xFF55, CGB only) - General Purpose and HBlank DMA
//! - ✅ Infrared port (RP at 0xFF56, CGB only, register access only)
//!
//! ## Not Implemented
//! - ❌ External link cable hardware emulation
//! - ❌ Actual infrared hardware communication

use crate::apu::GbApu;
use crate::mappers::Mapper;
use crate::ppu::Ppu;
use crate::timer::Timer;
use emu_core::cpu_lr35902::MemoryLr35902;

/// Game Boy memory bus
pub struct GbBus {
    /// Work RAM (32KB for CGB, 8 banks of 4KB each)
    /// Bank 0 is always at 0xC000-0xCFFF
    /// Banks 1-7 are switchable at 0xD000-0xDFFF via SVBK register
    wram: [u8; 0x8000],
    /// SVBK register (0xFF70) - WRAM bank select (CGB only)
    /// Bits 0-2: WRAM bank (0-7, where 0 is mapped to bank 1)
    /// Bits 3-7: Unused (read as 1)
    svbk: u8,
    /// High RAM (127 bytes)
    hram: [u8; 0x7F],
    /// Interrupt Enable register
    ie: u8,
    /// Interrupt Flag register
    if_reg: u8,
    /// Cartridge mapper (handles ROM/RAM banking)
    mapper: Option<Mapper>,
    /// Boot ROM enabled flag
    boot_rom_enabled: bool,
    /// PPU (Picture Processing Unit)
    pub ppu: Ppu,
    /// APU (Audio Processing Unit)
    pub apu: GbApu,
    /// Timer
    pub timer: Timer,
    /// Joypad state register (0xFF00)
    joypad: u8,
    /// Joypad button state
    button_state: u8,
    /// CGB mode flag (true if Game Boy Color features are enabled)
    cgb_mode: bool,
    /// KEY1 register (0xFF4D) - CGB speed switch
    /// Bit 7 (read): Current speed (0=normal 4.19 MHz, 1=double 8.39 MHz)
    /// Bit 0 (write): Speed switch prepare flag
    key1: u8,
    /// Serial transfer data (0xFF01)
    sb: u8,
    /// Serial control (0xFF02)
    /// Bit 7: Transfer start (1=start, 0=no transfer in progress)
    /// Bit 1: Clock speed (CGB only, 0=normal, 1=fast)
    /// Bit 0: Clock source (0=external, 1=internal)
    sc: u8,
    /// Serial transfer bit counter (for internal clock mode)
    serial_bit_counter: u8,
    /// Serial transfer cycle counter
    serial_cycle_counter: u32,
    /// Infrared port register (0xFF56, CGB only)
    /// Bits 6-7: LED control (0=off, 1=on for bits 6 and 7)
    /// Bits 0-1: Signal receive (read-only, stubbed to 0)
    /// Bits 2-5: Unused
    rp: u8,
    /// HDMA Source High (0xFF51, CGB only)
    hdma1: u8,
    /// HDMA Source Low (0xFF52, CGB only)
    hdma2: u8,
    /// HDMA Destination High (0xFF53, CGB only)
    hdma3: u8,
    /// HDMA Destination Low (0xFF54, CGB only)
    hdma4: u8,
    /// HDMA Length/Mode/Start (0xFF55, CGB only)
    /// Bit 7: 0=General Purpose DMA, 1=HBlank DMA
    /// Bits 0-6: Length (in 16-byte blocks - 1)
    hdma5: u8,
    /// HDMA active flag
    hdma_active: bool,
    /// HDMA remaining length (in 16-byte blocks)
    hdma_remaining: u8,
    /// HDMA source address (actual address, updated during transfer)
    hdma_source: u16,
    /// HDMA destination address (actual address, updated during transfer)
    hdma_dest: u16,
}

impl GbBus {
    pub fn new() -> Self {
        Self {
            wram: [0; 0x8000],
            svbk: 0,
            hram: [0; 0x7F],
            ie: 0,
            if_reg: 0,
            mapper: None,
            boot_rom_enabled: true,
            ppu: Ppu::new(),
            apu: GbApu::new(),
            timer: Timer::new(),
            joypad: 0xFF,
            button_state: 0xFF,
            cgb_mode: false,
            key1: 0, // Start in normal speed mode
            sb: 0,
            sc: 0,
            serial_bit_counter: 0,
            serial_cycle_counter: 0,
            rp: 0,
            hdma1: 0,
            hdma2: 0,
            hdma3: 0,
            hdma4: 0,
            hdma5: 0xFF, // All bits set when inactive
            hdma_active: false,
            hdma_remaining: 0,
            hdma_source: 0,
            hdma_dest: 0,
        }
    }

    /// Set joypad button state
    /// Bits: 0=Right, 1=Left, 2=Up, 3=Down, 4=A, 5=B, 6=Select, 7=Start
    pub fn set_buttons(&mut self, state: u8) {
        self.button_state = state;
    }

    /// Request an interrupt
    /// Bit 0: VBlank
    /// Bit 1: LCD STAT
    /// Bit 2: Timer
    /// Bit 3: Serial
    /// Bit 4: Joypad
    pub fn request_interrupt(&mut self, interrupt_bit: u8) {
        self.if_reg |= interrupt_bit;
    }

    /// Tick the mapper (e.g., for MBC3 RTC)
    /// Should be called once per frame
    pub fn tick_mapper(&mut self) {
        if let Some(mapper) = &mut self.mapper {
            mapper.tick();
        }
    }

    /// Step the serial transfer
    /// Returns true if a serial interrupt should be generated
    /// Should be called with CPU cycles
    pub fn step_serial(&mut self, cycles: u32) -> bool {
        // Only process if transfer is active and using internal clock
        if (self.sc & 0x80) == 0 || (self.sc & 0x01) == 0 {
            return false;
        }

        self.serial_cycle_counter += cycles;

        // Serial transfer takes 512 cycles per bit in normal speed
        // (8192 Hz clock rate, which is CPU clock / 512)
        // In CGB fast mode (bit 1 set), it takes 16 cycles per bit
        let cycles_per_bit = if (self.sc & 0x02) != 0 { 16 } else { 512 };

        if self.serial_cycle_counter >= cycles_per_bit {
            self.serial_cycle_counter -= cycles_per_bit;

            if self.serial_bit_counter > 0 {
                // Shift out one bit from SB, shift in 0xFF (no device connected)
                // In loopback mode, we just shift in what we shift out
                let _out_bit = (self.sb >> 7) & 0x01;
                self.sb = (self.sb << 1) | 0x01; // Shift in 1 (disconnected = all high)

                self.serial_bit_counter -= 1;

                // When all 8 bits are transferred, clear transfer flag and request interrupt
                if self.serial_bit_counter == 0 {
                    self.sc &= !0x80; // Clear transfer start bit
                    return true; // Request serial interrupt
                }
            }
        }

        false
    }

    /// Write to HDMA5 register - triggers DMA transfer
    fn write_hdma5(&mut self, val: u8) {
        // If bit 7 is 0, this is a General Purpose DMA (immediate transfer)
        // If bit 7 is 1, this is an HBlank DMA (transfer during HBlank)

        let is_hblank_mode = (val & 0x80) != 0;
        let length = ((val & 0x7F) + 1) as u16; // Length in 16-byte blocks

        // If HDMA is active and we're writing 0 to bit 7, stop the transfer
        if self.hdma_active && !is_hblank_mode {
            self.hdma_active = false;
            self.hdma5 = 0xFF;
            return;
        }

        // Calculate source and destination addresses
        // Source: HDMA1:HDMA2, but lower 4 bits of HDMA2 are ignored
        let source = ((self.hdma1 as u16) << 8) | ((self.hdma2 as u16) & 0xF0);

        // Destination: HDMA3:HDMA4 + 0x8000, lower 4 bits of HDMA4 are ignored
        // Destination is always in VRAM (0x8000-0x9FFF)
        let dest = 0x8000 | (((self.hdma3 as u16) << 8) | ((self.hdma4 as u16) & 0xF0)) & 0x1FFF;

        self.hdma_source = source;
        self.hdma_dest = dest;
        self.hdma_remaining = length as u8;
        self.hdma5 = val & 0x7F; // Store the length part

        if is_hblank_mode {
            // HBlank DMA - will be performed during HBlank periods
            self.hdma_active = true;
        } else {
            // General Purpose DMA - perform immediately
            self.perform_gdma();
            self.hdma5 = 0xFF; // Transfer complete
        }
    }

    /// Perform General Purpose DMA (immediate transfer)
    fn perform_gdma(&mut self) {
        // Transfer all blocks immediately
        let blocks = self.hdma_remaining;

        for _ in 0..blocks {
            // Transfer one 16-byte block
            for i in 0..16 {
                let byte = self.read(self.hdma_source + i);
                self.ppu.write_vram(self.hdma_dest - 0x8000 + i, byte);
            }

            self.hdma_source += 16;
            self.hdma_dest += 16;
        }

        self.hdma_remaining = 0;
        self.hdma_active = false;
    }

    /// Perform one block of HBlank DMA
    /// Should be called during HBlank period
    /// Returns true if transfer is complete
    pub fn step_hdma(&mut self) -> bool {
        if !self.hdma_active || self.hdma_remaining == 0 {
            return false;
        }

        // Transfer one 16-byte block during HBlank
        for i in 0..16 {
            let byte = self.read(self.hdma_source + i);
            self.ppu.write_vram(self.hdma_dest - 0x8000 + i, byte);
        }

        self.hdma_source += 16;
        self.hdma_dest += 16;
        self.hdma_remaining -= 1;

        // Update HDMA5 register
        if self.hdma_remaining == 0 {
            // Transfer complete
            self.hdma_active = false;
            self.hdma5 = 0xFF;
            true
        } else {
            self.hdma5 = self.hdma_remaining - 1;
            false
        }
    }

    /// Check if CGB mode is enabled
    #[allow(dead_code)] // Will be used when CGB features are fully implemented
    pub fn is_cgb_mode(&self) -> bool {
        self.cgb_mode
    }

    /// Get the KEY1 register (0xFF4D) value
    /// Bit 7 (read): Current speed (0=normal, 1=double)
    /// Bit 0 (read): Speed switch armed flag
    pub fn read_key1(&self) -> u8 {
        // Bit 7: current speed, Bit 0: prepare switch flag
        // Bits 1-6 always read as 1
        (self.key1 & 0x81) | 0x7E
    }

    /// Write to KEY1 register (0xFF4D)
    /// Only bit 0 is writable - it arms the speed switch
    pub fn write_key1(&mut self, val: u8) {
        // Only bit 0 is writable (prepare speed switch)
        self.key1 = (self.key1 & 0x80) | (val & 0x01);
    }

    /// Perform CGB speed switch
    /// Called by STOP instruction when KEY1 bit 0 is set
    /// Returns true if speed switch was performed
    fn do_speed_switch(&mut self) -> bool {
        // Only perform switch if bit 0 is set (switch armed) and in CGB mode
        if !self.cgb_mode || (self.key1 & 0x01) == 0 {
            return false;
        }

        // Toggle speed (bit 7)
        self.key1 ^= 0x80;

        // Clear prepare flag (bit 0)
        self.key1 &= 0xFE;

        true
    }

    /// Read SVBK register (0xFF70) - WRAM bank select (CGB only)
    /// Bits 0-2: WRAM bank (0-7, where 0 is mapped to bank 1)
    /// Bits 3-7: Unused (always read as 1)
    fn read_svbk(&self) -> u8 {
        // Return current bank selection with unused bits set to 1
        (self.svbk & 0x07) | 0xF8
    }

    /// Write SVBK register (0xFF70) - WRAM bank select (CGB only)
    /// Only bits 0-2 are writable
    fn write_svbk(&mut self, val: u8) {
        // Only bits 0-2 are writable
        self.svbk = val & 0x07;
    }

    /// Get the actual WRAM bank for address range 0xD000-0xDFFF
    /// Bank 0 in SVBK is mapped to bank 1
    fn get_wram_bank(&self) -> usize {
        if self.cgb_mode {
            // In CGB mode, SVBK selects bank (0 maps to 1, 1-7 map directly)
            let bank = self.svbk & 0x07;
            if bank == 0 {
                1 // Bank 0 maps to bank 1
            } else {
                bank as usize
            }
        } else {
            // In DMG mode, always use bank 1
            1
        }
    }

    pub fn load_cart(&mut self, data: &[u8]) {
        // Parse cart header
        if data.len() < 0x150 {
            // Too small to be a valid cart, but load it anyway
            self.mapper = Some(Mapper::from_cart(data.to_vec(), vec![], 0x00));
            self.boot_rom_enabled = false;
            self.cgb_mode = false;
            return;
        }

        let cart_type = data[0x147];
        let ram_size_code = data[0x149];

        // Check CGB flag at 0x143
        // 0x80 = CGB-compatible (works on both DMG and CGB, uses CGB features on CGB)
        // 0xC0 = CGB-only game (won't work on DMG hardware)
        // Other values = DMG only game
        let cgb_flag = data[0x143];

        // Enable CGB mode for both CGB-compatible (0x80) and CGB-only (0xC0) games
        self.cgb_mode = cgb_flag == 0x80 || cgb_flag == 0xC0;

        // Enable CGB mode in PPU if CGB ROM
        if self.cgb_mode {
            // For 0x80 (CGB-compatible), enable compatibility mode with default DMG palette
            // For 0xC0 (CGB-only), disable compatibility mode (game will set its own palettes)
            let compatibility_mode = cgb_flag == 0x80;
            self.ppu.enable_cgb_mode(compatibility_mode);
        }

        let ram_size = match ram_size_code {
            0x00 => 0,
            0x01 => 0, // Unused
            0x02 => 8 * 1024,
            0x03 => 32 * 1024,
            0x04 => 128 * 1024,
            0x05 => 64 * 1024,
            _ => 0,
        };

        let ram = if ram_size > 0 {
            vec![0; ram_size]
        } else {
            vec![]
        };

        self.mapper = Some(Mapper::from_cart(data.to_vec(), ram, cart_type));
        self.boot_rom_enabled = false; // Skip boot ROM for now
    }
}

impl MemoryLr35902 for GbBus {
    fn read(&self, addr: u16) -> u8 {
        match addr {
            // ROM Bank 0 and Bank 1-N (switchable)
            0x0000..=0x7FFF => {
                if addr < 0x0100 && self.boot_rom_enabled {
                    // Boot ROM would go here
                    0xFF
                } else if let Some(mapper) = &self.mapper {
                    mapper.read_rom(addr)
                } else {
                    0xFF
                }
            }
            // VRAM (8KB) - delegate to PPU
            0x8000..=0x9FFF => self.ppu.read_vram(addr - 0x8000),
            // External RAM (switchable)
            0xA000..=0xBFFF => {
                if let Some(mapper) = &self.mapper {
                    mapper.read_ram(addr)
                } else {
                    0xFF
                }
            }
            // Work RAM (8KB with banking in CGB mode)
            // 0xC000-0xCFFF: Bank 0 (fixed)
            // 0xD000-0xDFFF: Bank 1-7 (switchable via SVBK in CGB mode)
            0xC000..=0xCFFF => {
                // Bank 0 is always at 0xC000-0xCFFF
                self.wram[(addr - 0xC000) as usize]
            }
            0xD000..=0xDFFF => {
                // Switchable bank area (banks 1-7 in CGB mode, bank 1 in DMG mode)
                let bank = self.get_wram_bank();
                let offset = (bank * 0x1000) + (addr - 0xD000) as usize;
                self.wram[offset]
            }
            // Echo RAM (mirror of 0xC000-0xDDFF)
            0xE000..=0xEFFF => {
                // Mirror of bank 0
                self.wram[(addr - 0xE000) as usize]
            }
            0xF000..=0xFDFF => {
                // Mirror of switchable bank area
                let bank = self.get_wram_bank();
                let offset = (bank * 0x1000) + (addr - 0xF000) as usize;
                self.wram[offset]
            }
            // OAM (Object Attribute Memory) - delegate to PPU
            0xFE00..=0xFE9F => self.ppu.read_oam(addr - 0xFE00),
            // Not usable
            0xFEA0..=0xFEFF => 0xFF,
            // I/O Registers
            0xFF00..=0xFF7F => match addr {
                0xFF00 => {
                    // Joypad register
                    // Return button state based on selected mode
                    let select_buttons = (self.joypad & 0x20) == 0;
                    let select_dpad = (self.joypad & 0x10) == 0;

                    // Bits 6-7 are unused and always read as 1
                    let mut result = (self.joypad & 0x30) | 0xC0;
                    if select_buttons {
                        result |= (self.button_state >> 4) & 0x0F;
                    } else if select_dpad {
                        result |= self.button_state & 0x0F;
                    } else {
                        result |= 0x0F;
                    }
                    result
                }
                // Serial transfer registers
                0xFF01 => self.sb,
                0xFF02 => self.sc | 0x7C, // Bits 2-6 unused, always read as 1
                // Timer registers
                0xFF04..=0xFF07 => self.timer.read_register(addr),
                0xFF0F => self.if_reg | 0xE0, // Bits 5-7 unused, always read as 1
                // APU registers
                0xFF10..=0xFF26 => self.apu.read_register(addr),
                0xFF30..=0xFF3F => self.apu.read_register(addr),
                // PPU registers
                0xFF40 => self.ppu.lcdc,
                0xFF41 => self.ppu.stat | 0x80, // Bit 7 unused, always reads as 1
                0xFF42 => self.ppu.scy,
                0xFF43 => self.ppu.scx,
                0xFF44 => self.ppu.ly,
                0xFF45 => self.ppu.lyc,
                0xFF47 => self.ppu.bgp,
                0xFF48 => self.ppu.obp0,
                0xFF49 => self.ppu.obp1,
                0xFF4A => self.ppu.wy,
                0xFF4B => self.ppu.wx,
                0xFF4D => self.read_key1(), // KEY1 - Speed switch (CGB only)
                // CGB registers
                0xFF4F => self.ppu.get_vram_bank(), // VBK - VRAM bank
                // HDMA registers (CGB only)
                0xFF51 => self.hdma1,
                0xFF52 => self.hdma2,
                0xFF53 => self.hdma3,
                0xFF54 => self.hdma4,
                0xFF55 => {
                    // Reading HDMA5 returns remaining length or 0xFF if inactive
                    if self.hdma_active {
                        (self.hdma_remaining.saturating_sub(1)) & 0x7F // Bit 7 is 0 during HBlank DMA, value is (remaining - 1)
                    } else {
                        0xFF
                    }
                }
                0xFF56 => self.rp,              // RP - Infrared port (CGB only)
                0xFF68 => self.ppu.read_bgpi(), // BCPS/BGPI - BG palette index
                0xFF69 => self.ppu.read_bgpd(), // BCPD/BGPD - BG palette data
                0xFF6A => self.ppu.read_obpi(), // OCPS/OBPI - OBJ palette index
                0xFF6B => self.ppu.read_obpd(), // OCPD/OBPD - OBJ palette data
                0xFF70 => self.read_svbk(),     // SVBK - WRAM bank (CGB only)
                _ => 0xFF,
            },
            // High RAM
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize],
            // Interrupt Enable
            0xFFFF => self.ie | 0xE0, // Bits 5-7 unused, always read as 1
        }
    }

    fn write(&mut self, addr: u16, val: u8) {
        match addr {
            // ROM (read-only, but may trigger MBC commands)
            0x0000..=0x7FFF => {
                if let Some(mapper) = &mut self.mapper {
                    mapper.write_rom(addr, val);
                }
            }
            // VRAM - delegate to PPU
            0x8000..=0x9FFF => self.ppu.write_vram(addr - 0x8000, val),
            // External RAM
            0xA000..=0xBFFF => {
                if let Some(mapper) = &mut self.mapper {
                    mapper.write_ram(addr, val);
                }
            }
            // Work RAM (8KB with banking in CGB mode)
            // 0xC000-0xCFFF: Bank 0 (fixed)
            // 0xD000-0xDFFF: Bank 1-7 (switchable via SVBK in CGB mode)
            0xC000..=0xCFFF => {
                // Bank 0 is always at 0xC000-0xCFFF
                self.wram[(addr - 0xC000) as usize] = val;
            }
            0xD000..=0xDFFF => {
                // Switchable bank area (banks 1-7 in CGB mode, bank 1 in DMG mode)
                let bank = self.get_wram_bank();
                let offset = (bank * 0x1000) + (addr - 0xD000) as usize;
                self.wram[offset] = val;
            }
            // Echo RAM (mirror of 0xC000-0xDDFF)
            0xE000..=0xEFFF => {
                // Mirror of bank 0
                self.wram[(addr - 0xE000) as usize] = val;
            }
            0xF000..=0xFDFF => {
                // Mirror of switchable bank area
                let bank = self.get_wram_bank();
                let offset = (bank * 0x1000) + (addr - 0xF000) as usize;
                self.wram[offset] = val;
            }
            // OAM - delegate to PPU
            0xFE00..=0xFE9F => self.ppu.write_oam(addr - 0xFE00, val),
            // Not usable
            0xFEA0..=0xFEFF => {}
            // I/O Registers
            0xFF00..=0xFF7F => {
                match addr {
                    0xFF00 => self.joypad = val & 0x30, // Only bits 4-5 are writable
                    // Serial transfer registers
                    0xFF01 => self.sb = val,
                    0xFF02 => {
                        self.sc = val & 0x83; // Only bits 0, 1, and 7 are writable

                        // If transfer start bit is set and using internal clock
                        if (val & 0x80) != 0 && (val & 0x01) != 0 {
                            // Start serial transfer
                            self.serial_bit_counter = 8;
                            self.serial_cycle_counter = 0;
                        }
                    }
                    // Timer registers
                    0xFF04..=0xFF07 => self.timer.write_register(addr, val),
                    0xFF0F => self.if_reg = val & 0x1F, // Only bits 0-4 are writable
                    // APU registers
                    0xFF10..=0xFF26 => self.apu.write_register(addr, val),
                    0xFF30..=0xFF3F => self.apu.write_register(addr, val),
                    // PPU registers
                    0xFF40 => self.ppu.lcdc = val,
                    0xFF41 => self.ppu.stat = val,
                    0xFF42 => self.ppu.scy = val,
                    0xFF43 => self.ppu.scx = val,
                    0xFF44 => {} // LY is read-only
                    0xFF45 => self.ppu.lyc = val,
                    0xFF46 => {
                        // OAM DMA: Copy 160 bytes from XX00-XX9F to OAM
                        let source_base = (val as u16) << 8;

                        for i in 0..0xA0u16 {
                            let byte = self.read(source_base + i);
                            self.ppu.write_oam(i, byte);
                        }
                    }
                    0xFF47 => self.ppu.bgp = val,
                    0xFF48 => self.ppu.obp0 = val,
                    0xFF49 => self.ppu.obp1 = val,
                    0xFF4A => self.ppu.wy = val,
                    0xFF4B => self.ppu.wx = val,
                    0xFF4D => self.write_key1(val), // KEY1 - Speed switch (CGB only)
                    // CGB registers
                    0xFF4F => self.ppu.set_vram_bank(val), // VBK - VRAM bank
                    // HDMA registers (CGB only)
                    0xFF51 => self.hdma1 = val,
                    0xFF52 => self.hdma2 = val & 0xF0, // Lower 4 bits are ignored
                    0xFF53 => self.hdma3 = val & 0x1F, // Only bits 0-4 are used (VRAM range 0x8000-0x9FFF)
                    0xFF54 => self.hdma4 = val & 0xF0, // Lower 4 bits are ignored
                    0xFF55 => self.write_hdma5(val),   // HDMA Length/Mode/Start
                    0xFF56 => self.rp = val & 0xC1, // RP - Infrared port (CGB only, only bits 0, 6, 7)
                    0xFF68 => self.ppu.write_bgpi(val), // BCPS/BGPI
                    0xFF69 => self.ppu.write_bgpd(val), // BCPD/BGPD
                    0xFF6A => self.ppu.write_obpi(val), // OCPS/OBPI
                    0xFF6B => self.ppu.write_obpd(val), // OCPD/OBPD
                    0xFF70 => self.write_svbk(val), // SVBK - WRAM bank (CGB only)
                    0xFF50 => self.boot_rom_enabled = false, // Disable boot ROM
                    _ => {}
                }
            }
            // High RAM
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize] = val,
            // Interrupt Enable
            0xFFFF => self.ie = val & 0x1F, // Only bits 0-4 are writable
        }
    }

    fn is_cgb_mode(&self) -> bool {
        self.cgb_mode
    }

    fn perform_speed_switch(&mut self) -> bool {
        self.do_speed_switch()
    }
}
