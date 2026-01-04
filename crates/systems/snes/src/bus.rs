//! SNES memory bus implementation

use crate::cartridge::Cartridge;
use crate::ppu::Ppu;
use crate::SnesError;
use emu_core::cpu_65c816::Memory65c816;
use emu_core::logging::{log, LogCategory, LogLevel};
use std::cell::Cell;

/// DMA channel configuration (one per channel, 8 total)
#[derive(Clone, Copy)]
struct DmaChannel {
    /// $43x0 - DMA control (direction, increment, mode)
    control: u8,
    /// $43x1 - B-bus address (PPU register)
    b_addr: u8,
    /// $43x2-$43x4 - A-bus address (24-bit)
    a_addr: u32,
    /// $43x5-$43x6 - Transfer size (0 = 65536)
    size: u16,
    /// $43x7 - HDMA indirect address bank (HDMA only)
    hdma_bank: u8,
    /// $43x8-$43x9 - HDMA table address (HDMA only)
    hdma_table: u16,
    /// $43xA - HDMA line counter (HDMA only)
    hdma_line: u8,
}

/// HDMA channel state (runtime state, not registers)
#[derive(Clone, Copy, Default)]
struct HdmaState {
    /// Current table address
    table_addr: u32,
    /// Current line counter
    line_counter: u8,
    /// Repeat mode flag
    repeat: bool,
    /// Channel is active
    active: bool,
}

impl Default for DmaChannel {
    fn default() -> Self {
        Self {
            control: 0xFF,
            b_addr: 0xFF,
            a_addr: 0xFFFFFF,
            size: 0xFFFF,
            hdma_bank: 0xFF,
            hdma_table: 0xFFFF,
            hdma_line: 0xFF,
        }
    }
}

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
    /// Auto-joypad read enable ($4200 bit 0)
    auto_joypad_enable: bool,
    /// DMA channels (8 channels)
    dma_channels: [DmaChannel; 8],
    /// HDMA enable register ($420C)
    hdma_enable: u8,
    /// HDMA state for each channel
    hdma_state: [HdmaState; 8],
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
            auto_joypad_enable: true, // Default to enabled
            dma_channels: [DmaChannel::default(); 8],
            hdma_enable: 0,
            hdma_state: [HdmaState::default(); 8],
        }
    }

    pub fn load_cartridge(&mut self, data: &[u8]) -> Result<(), SnesError> {
        log(LogCategory::Bus, LogLevel::Info, || {
            format!("SNES Bus: Loading cartridge ({} bytes)", data.len())
        });
        self.cartridge = Some(Cartridge::load(data)?);
        Ok(())
    }

    pub fn unload_cartridge(&mut self) {
        log(LogCategory::Bus, LogLevel::Info, || {
            "SNES Bus: Unloading cartridge".to_string()
        });
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
            log(LogCategory::Bus, LogLevel::Debug, || {
                format!(
                    "SNES Bus: Controller {} state set to 0x{:04X}",
                    idx + 1,
                    state
                )
            });
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

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn is_hirom(&self) -> bool {
        if let Some(ref cart) = self.cartridge {
            cart.is_hirom()
        } else {
            false
        }
    }

    /// Perform DMA transfer for specified channels
    /// Returns number of cycles consumed
    pub fn do_dma(&mut self, channels: u8) -> u32 {
        let mut cycles = 0u32;

        // Process each enabled channel
        for ch in 0..8 {
            if (channels & (1 << ch)) == 0 {
                continue;
            }

            let dma = self.dma_channels[ch];
            let direction = (dma.control & 0x80) != 0; // 0 = A->B, 1 = B->A
            let increment_mode = (dma.control >> 3) & 0x03; // 00=inc, 01=fixed, 10/11=dec
            let transfer_mode = dma.control & 0x07;

            let mut size = if dma.size == 0 {
                0x10000
            } else {
                dma.size as usize
            };
            let mut a_addr = dma.a_addr;

            log(LogCategory::Bus, LogLevel::Debug, || {
                format!(
                    "DMA Channel {}: {} {} bytes from ${:06X} to ${:02X}, mode={}, inc={}",
                    ch,
                    if direction { "B->A" } else { "A->B" },
                    size,
                    a_addr,
                    dma.b_addr,
                    transfer_mode,
                    increment_mode
                )
            });

            // 8 cycles overhead per channel
            cycles += 8;

            // Transfer loop
            while size > 0 {
                let bytes_this_transfer = match transfer_mode {
                    0 | 2 | 6 => 1, // 1 byte per transfer
                    1 | 5 => 2,     // 2 bytes per transfer (alternate between two B-bus addresses)
                    3 | 7 => 4,     // 4 bytes per transfer
                    4 => 4,         // 4 bytes per transfer
                    _ => 1,
                };

                for i in 0..bytes_this_transfer.min(size) {
                    // Calculate B-bus register address
                    let b_reg = match transfer_mode {
                        0 | 4 => 0x2100 | (dma.b_addr as u16),
                        1 | 5 => {
                            // Alternate: b_addr, b_addr+1
                            0x2100 | ((dma.b_addr as u16) + (i as u16 & 1))
                        }
                        2 | 6 => 0x2100 | (dma.b_addr as u16), // Fixed register
                        3 | 7 => {
                            // Pattern: b_addr, b_addr, b_addr+1, b_addr+1
                            0x2100 | ((dma.b_addr as u16) + ((i as u16 >> 1) & 1))
                        }
                        _ => 0x2100 | (dma.b_addr as u16),
                    };

                    if direction {
                        // B-bus to A-bus (rare, mostly for reading from PPU)
                        let val = self.read(b_reg as u32);
                        self.write(a_addr, val);
                    } else {
                        // A-bus to B-bus (common, writing to VRAM/CGRAM/OAM)
                        let val = self.read(a_addr);
                        self.write(b_reg as u32, val);
                    }

                    // Update A-bus address based on increment mode
                    match increment_mode {
                        0 => a_addr += 1,     // Increment
                        1 => {}               // Fixed
                        2 | 3 => a_addr -= 1, // Decrement
                        _ => {}
                    }

                    size -= 1;
                    cycles += 8; // 8 master cycles per byte
                }
            }
        }

        cycles
    }

    /// Initialize HDMA channels at start of frame
    pub fn init_hdma(&mut self) {
        for ch in 0..8 {
            if (self.hdma_enable & (1 << ch)) != 0 {
                let dma = self.dma_channels[ch];

                // Initialize HDMA state from registers
                self.hdma_state[ch].table_addr =
                    (dma.hdma_table as u32) | ((dma.hdma_bank as u32) << 16);
                self.hdma_state[ch].line_counter = 0;
                self.hdma_state[ch].repeat = false;
                self.hdma_state[ch].active = true;

                log(LogCategory::Bus, LogLevel::Debug, || {
                    format!(
                        "HDMA Channel {} initialized: table=${:06X}",
                        ch, self.hdma_state[ch].table_addr
                    )
                });
            } else {
                self.hdma_state[ch].active = false;
            }
        }
    }

    /// Execute HDMA for all active channels (called during H-blank of each scanline)
    pub fn do_hdma(&mut self) -> u32 {
        let mut cycles = 0u32;

        for ch in 0..8 {
            if !self.hdma_state[ch].active {
                continue;
            }

            let dma = self.dma_channels[ch];

            // Check if we need to fetch a new line count
            if self.hdma_state[ch].line_counter == 0 {
                // Read line count byte from table
                let line_byte = self.read(self.hdma_state[ch].table_addr);
                self.hdma_state[ch].table_addr += 1;

                // Check for termination (line count = 0)
                if line_byte == 0 {
                    self.hdma_state[ch].active = false;
                    log(LogCategory::Bus, LogLevel::Debug, || {
                        format!("HDMA Channel {} terminated", ch)
                    });
                    continue;
                }

                // Extract repeat flag and line count
                self.hdma_state[ch].repeat = (line_byte & 0x80) != 0;
                self.hdma_state[ch].line_counter = line_byte & 0x7F;

                // For indirect mode, read the indirect address
                let indirect = (dma.control & 0x40) != 0;
                if indirect {
                    // Read 2-byte indirect address
                    let addr_low = self.read(self.hdma_state[ch].table_addr) as u32;
                    let addr_high = self.read(self.hdma_state[ch].table_addr + 1) as u32;
                    self.hdma_state[ch].table_addr += 2;

                    // Store indirect address (will be used for data fetch)
                    // For now, we'll use the A-bus address field temporarily
                    self.dma_channels[ch].a_addr =
                        addr_low | (addr_high << 8) | ((dma.hdma_bank as u32) << 16);
                }
            }

            // Perform the transfer
            let transfer_mode = dma.control & 0x07;
            let bytes_to_transfer = match transfer_mode {
                0 | 2 | 6 => 1, // 1 byte
                1 | 5 => 2,     // 2 bytes
                3 | 7 => 4,     // 4 bytes
                4 => 4,         // 4 bytes
                _ => 1,
            };

            let indirect = (dma.control & 0x40) != 0;
            let source_addr = if indirect {
                // Use indirect address
                dma.a_addr
            } else {
                // Use table address directly
                self.hdma_state[ch].table_addr
            };

            // Transfer the bytes
            for i in 0..bytes_to_transfer {
                let b_reg = match transfer_mode {
                    0 | 4 => 0x2100 | (dma.b_addr as u16),
                    1 | 5 => 0x2100 | ((dma.b_addr as u16) + (i as u16 & 1)),
                    2 | 6 => 0x2100 | (dma.b_addr as u16),
                    3 | 7 => 0x2100 | ((dma.b_addr as u16) + ((i as u16 >> 1) & 1)),
                    _ => 0x2100 | (dma.b_addr as u16),
                };

                let val = self.read(source_addr + i as u32);
                self.write(b_reg as u32, val);
                cycles += 8; // 8 cycles per byte
            }

            // Update addresses for next scanline
            if indirect {
                // Increment indirect address
                self.dma_channels[ch].a_addr += bytes_to_transfer as u32;
            } else if !self.hdma_state[ch].repeat {
                // Increment table address (only if not in repeat mode)
                self.hdma_state[ch].table_addr += bytes_to_transfer as u32;
            }

            // Decrement line counter
            self.hdma_state[ch].line_counter -= 1;
        }

        cycles
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
                        if self.ppu.nmi_enable {
                            0x80
                        } else {
                            0x00
                        }
                    }
                    // $420C - HDMAEN - HDMA Enable (read)
                    0x420C => self.hdma_enable,
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
                    // $4218-$421F - JOYxL/JOYxH - Auto-joypad read (only valid when auto-read enabled)
                    0x4218 => {
                        if self.auto_joypad_enable {
                            (self.controller_state[0] & 0xFF) as u8 // JOY1L
                        } else {
                            0 // Return 0 when auto-read disabled
                        }
                    }
                    0x4219 => {
                        if self.auto_joypad_enable {
                            ((self.controller_state[0] >> 8) & 0xFF) as u8 // JOY1H
                        } else {
                            0
                        }
                    }
                    0x421A => {
                        if self.auto_joypad_enable {
                            (self.controller_state[1] & 0xFF) as u8 // JOY2L
                        } else {
                            0
                        }
                    }
                    0x421B => {
                        if self.auto_joypad_enable {
                            ((self.controller_state[1] >> 8) & 0xFF) as u8 // JOY2H
                        } else {
                            0
                        }
                    }
                    0x421C => 0, // JOY3L (not implemented)
                    0x421D => 0, // JOY3H (not implemented)
                    0x421E => 0, // JOY4L (not implemented)
                    0x421F => 0, // JOY4H (not implemented)
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
                    // $43x0-$43xA - DMA channel registers (read)
                    0x4300..=0x437F => {
                        let ch = ((offset - 0x4300) >> 4) as usize & 7;
                        let reg = (offset & 0x0F) as usize;
                        match reg {
                            0x0 => self.dma_channels[ch].control,
                            0x1 => self.dma_channels[ch].b_addr,
                            0x2 => (self.dma_channels[ch].a_addr & 0xFF) as u8,
                            0x3 => ((self.dma_channels[ch].a_addr >> 8) & 0xFF) as u8,
                            0x4 => ((self.dma_channels[ch].a_addr >> 16) & 0xFF) as u8,
                            0x5 => (self.dma_channels[ch].size & 0xFF) as u8,
                            0x6 => ((self.dma_channels[ch].size >> 8) & 0xFF) as u8,
                            0x7 => self.dma_channels[ch].hdma_bank,
                            0x8 => (self.dma_channels[ch].hdma_table & 0xFF) as u8,
                            0x9 => ((self.dma_channels[ch].hdma_table >> 8) & 0xFF) as u8,
                            0xA => self.dma_channels[ch].hdma_line,
                            _ => 0xFF, // Open bus for unused registers
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
                        // Bit 0: Joypad auto-read enable
                        // Other bits: H/V timer interrupt enable (not implemented)
                        let old_nmi_enable = self.ppu.nmi_enable;
                        self.ppu.nmi_enable = (val & 0x80) != 0;
                        if old_nmi_enable != self.ppu.nmi_enable {
                            log(LogCategory::Interrupts, LogLevel::Debug, || {
                                format!(
                                    "SNES Bus: NMI {}",
                                    if self.ppu.nmi_enable {
                                        "enabled"
                                    } else {
                                        "disabled"
                                    }
                                )
                            });
                        }

                        // Bit 0: Auto-joypad read enable
                        let old_auto_joypad = self.auto_joypad_enable;
                        self.auto_joypad_enable = (val & 0x01) != 0;
                        if old_auto_joypad != self.auto_joypad_enable {
                            log(LogCategory::Bus, LogLevel::Debug, || {
                                format!(
                                    "SNES Bus: Auto-joypad read {}",
                                    if self.auto_joypad_enable {
                                        "enabled"
                                    } else {
                                        "disabled"
                                    }
                                )
                            });
                        }
                    }
                    // $4016 - JOYWR - Controller Strobe
                    0x4016 => {
                        // Bit 0: Controller strobe (1 = latch, 0 = shift)
                        let old_strobe = self.controller_strobe;
                        self.controller_strobe = (val & 1) != 0;

                        // On falling edge (1 -> 0), latch the controller state
                        if old_strobe && !self.controller_strobe {
                            log(LogCategory::Bus, LogLevel::Trace, || {
                                format!(
                                    "SNES Bus: Controller latch - P1: 0x{:04X}, P2: 0x{:04X}",
                                    self.controller_state[0], self.controller_state[1]
                                )
                            });
                            self.controller_shift[0].set(self.controller_state[0]);
                            self.controller_shift[1].set(self.controller_state[1]);
                        }
                    }
                    // $420B - MDMAEN - DMA Enable
                    0x420B => {
                        // Each bit enables a DMA channel
                        if val != 0 {
                            log(LogCategory::Bus, LogLevel::Info, || {
                                format!("SNES Bus: Starting DMA on channels 0b{:08b}", val)
                            });
                            // Note: In a real implementation, this would halt the CPU
                            // and perform the DMA transfer. We'll handle this in the CPU step.
                            // For now, we just trigger it immediately.
                            let _cycles = self.do_dma(val);
                        }
                    }
                    // $420C - HDMAEN - HDMA Enable
                    0x420C => {
                        self.hdma_enable = val;
                        if val != 0 {
                            log(LogCategory::Bus, LogLevel::Info, || {
                                format!("SNES Bus: HDMA enabled for channels 0b{:08b}", val)
                            });
                        }
                    }
                    // $43x0-$43xA - DMA channel registers (write)
                    0x4300..=0x437F => {
                        let ch = ((offset - 0x4300) >> 4) as usize & 7;
                        let reg = (offset & 0x0F) as usize;
                        match reg {
                            0x0 => self.dma_channels[ch].control = val,
                            0x1 => self.dma_channels[ch].b_addr = val,
                            0x2 => {
                                self.dma_channels[ch].a_addr =
                                    (self.dma_channels[ch].a_addr & 0xFFFF00) | (val as u32);
                            }
                            0x3 => {
                                self.dma_channels[ch].a_addr =
                                    (self.dma_channels[ch].a_addr & 0xFF00FF) | ((val as u32) << 8);
                            }
                            0x4 => {
                                self.dma_channels[ch].a_addr = (self.dma_channels[ch].a_addr
                                    & 0x00FFFF)
                                    | ((val as u32) << 16);
                            }
                            0x5 => {
                                self.dma_channels[ch].size =
                                    (self.dma_channels[ch].size & 0xFF00) | (val as u16);
                            }
                            0x6 => {
                                self.dma_channels[ch].size =
                                    (self.dma_channels[ch].size & 0x00FF) | ((val as u16) << 8);
                            }
                            0x7 => self.dma_channels[ch].hdma_bank = val,
                            0x8 => {
                                self.dma_channels[ch].hdma_table =
                                    (self.dma_channels[ch].hdma_table & 0xFF00) | (val as u16);
                            }
                            0x9 => {
                                self.dma_channels[ch].hdma_table =
                                    (self.dma_channels[ch].hdma_table & 0x00FF)
                                        | ((val as u16) << 8);
                            }
                            0xA => self.dma_channels[ch].hdma_line = val,
                            _ => {} // Unused registers, ignore writes
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

    #[test]
    fn test_dma_registers() {
        let mut bus = SnesBus::new();

        // Write to DMA channel 0 registers
        bus.write(0x4300, 0x01); // Control
        bus.write(0x4301, 0x18); // B-bus address ($2118 = VRAM data low)
        bus.write(0x4302, 0x00); // A-bus address low
        bus.write(0x4303, 0x80); // A-bus address mid
        bus.write(0x4304, 0x7E); // A-bus address high (bank)
        bus.write(0x4305, 0x00); // Size low (256 bytes)
        bus.write(0x4306, 0x01); // Size high

        // Read back registers
        assert_eq!(bus.read(0x4300), 0x01);
        assert_eq!(bus.read(0x4301), 0x18);
        assert_eq!(bus.read(0x4302), 0x00);
        assert_eq!(bus.read(0x4303), 0x80);
        assert_eq!(bus.read(0x4304), 0x7E);
        assert_eq!(bus.read(0x4305), 0x00);
        assert_eq!(bus.read(0x4306), 0x01);
    }

    #[test]
    fn test_dma_transfer_simple() {
        let mut bus = SnesBus::new();

        // Set up WRAM with test data
        for i in 0..16 {
            bus.wram[i] = (i as u8) * 0x11;
        }

        // Configure DMA channel 0: WRAM -> VRAM
        bus.write(0x4300, 0x01); // Mode 1: 2 registers write once
        bus.write(0x4301, 0x18); // B-bus: $2118 (VMDATAL)
        bus.write(0x4302, 0x00); // A-bus: $7E0000 (WRAM start)
        bus.write(0x4303, 0x00);
        bus.write(0x4304, 0x7E);
        bus.write(0x4305, 0x10); // Size: 16 bytes
        bus.write(0x4306, 0x00);

        // Trigger DMA
        bus.write(0x420B, 0x01); // Enable channel 0

        // Verify data was transferred to VRAM (through PPU)
        // The DMA should have written to VMDATAL, which updates VRAM
        // Note: This is a basic test - actual VRAM verification would require
        // checking the PPU's internal state
    }

    #[test]
    fn test_dma_multiple_channels() {
        let mut bus = SnesBus::new();

        // Configure two channels
        bus.write(0x4300, 0x00); // Channel 0: mode 0
        bus.write(0x4301, 0x18); // B-bus: VRAM
        bus.write(0x4302, 0x00); // A-bus: $7E0000
        bus.write(0x4303, 0x00);
        bus.write(0x4304, 0x7E);
        bus.write(0x4305, 0x08); // 8 bytes
        bus.write(0x4306, 0x00);

        bus.write(0x4310, 0x00); // Channel 1: mode 0
        bus.write(0x4311, 0x22); // B-bus: CGRAM
        bus.write(0x4312, 0x10); // A-bus: $7E0010
        bus.write(0x4313, 0x00);
        bus.write(0x4314, 0x7E);
        bus.write(0x4315, 0x08); // 8 bytes
        bus.write(0x4316, 0x00);

        // Trigger both channels
        bus.write(0x420B, 0x03); // Enable channels 0 and 1

        // Both channels should complete
    }

    #[test]
    fn test_hdma_enable_register() {
        let mut bus = SnesBus::new();

        // Write to HDMA enable register
        bus.write(0x420C, 0x05); // Enable channels 0 and 2

        // Read back
        assert_eq!(bus.read(0x420C), 0x05);
        assert_eq!(bus.hdma_enable, 0x05);
    }

    #[test]
    fn test_hdma_initialization() {
        let mut bus = SnesBus::new();

        // Configure HDMA channel 0
        bus.write(0x4300, 0x00); // Mode 0, direct
        bus.write(0x4301, 0x00); // B-bus: $2100 (INIDISP - brightness)
        bus.write(0x4307, 0x7E); // HDMA bank
        bus.write(0x4308, 0x00); // HDMA table address low
        bus.write(0x4309, 0x10); // HDMA table address high ($7E1000)

        // Set up a simple HDMA table in WRAM
        // Format: [line_count, data, line_count, data, ..., 0]
        bus.wram[0x1000] = 0x01; // 1 scanline
        bus.wram[0x1001] = 0x0F; // Full brightness
        bus.wram[0x1002] = 0x00; // Terminate

        // Enable HDMA
        bus.write(0x420C, 0x01); // Enable channel 0

        // Initialize HDMA
        bus.init_hdma();

        // Verify state was initialized
        assert!(bus.hdma_state[0].active);
        assert_eq!(bus.hdma_state[0].table_addr, 0x7E1000);
    }

    #[test]
    fn test_hdma_execution_simple() {
        let mut bus = SnesBus::new();

        // Configure HDMA channel 0 for brightness control
        bus.write(0x4300, 0x00); // Mode 0: 1 byte transfer, direct
        bus.write(0x4301, 0x00); // B-bus: $2100 (INIDISP)
        bus.write(0x4307, 0x7E); // HDMA bank
        bus.write(0x4308, 0x00); // HDMA table low
        bus.write(0x4309, 0x20); // HDMA table high ($7E2000)

        // Set up HDMA table: 2 scanlines of 0x0F brightness, then terminate
        bus.wram[0x2000] = 0x02; // 2 scanlines
        bus.wram[0x2001] = 0x0F; // Brightness value
        bus.wram[0x2002] = 0x00; // Terminate

        // Enable and initialize HDMA
        bus.write(0x420C, 0x01);
        bus.init_hdma();

        // Execute HDMA for first scanline
        let _cycles = bus.do_hdma();

        // Verify the HDMA executed (line counter should be decremented)
        assert_eq!(bus.hdma_state[0].line_counter, 1);

        // Execute HDMA for second scanline
        let _cycles = bus.do_hdma();

        // Line counter should be 0, ready to fetch next entry
        assert_eq!(bus.hdma_state[0].line_counter, 0);

        // Execute HDMA again - should terminate
        let _cycles = bus.do_hdma();

        // Channel should be inactive now
        assert!(!bus.hdma_state[0].active);
    }

    #[test]
    fn test_hdma_repeat_mode() {
        let mut bus = SnesBus::new();

        // Configure HDMA
        bus.write(0x4300, 0x00); // Mode 0
        bus.write(0x4301, 0x00); // $2100
        bus.write(0x4307, 0x7E);
        bus.write(0x4308, 0x00);
        bus.write(0x4309, 0x30);

        // HDMA table with repeat flag set (0x80 | line_count)
        bus.wram[0x3000] = 0x83; // Repeat for 3 scanlines
        bus.wram[0x3001] = 0x07; // Value
        bus.wram[0x3002] = 0x00; // Terminate

        bus.write(0x420C, 0x01);
        bus.init_hdma();

        // Execute HDMA 3 times
        for _ in 0..3 {
            let _cycles = bus.do_hdma();
        }

        // Should still be at same table position (repeat mode)
        // and line counter should be 0
        assert_eq!(bus.hdma_state[0].line_counter, 0);
    }
}
