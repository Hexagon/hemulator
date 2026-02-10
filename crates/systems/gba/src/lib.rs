//! Game Boy Advance (GBA) system emulator.
//!
//! The GBA uses an ARM7TDMI CPU running at 16.78 MHz with:
//! - 32KB internal WRAM (on-chip, fast)
//! - 256KB external WRAM (on-board, slower)
//! - 1KB palette RAM
//! - 96KB VRAM
//! - 1KB OAM (Object Attribute Memory)
//! - Up to 32MB cartridge ROM
//! - 64KB cartridge SRAM (battery-backed save)
//!
//! ## Memory Map
//!
//! | Address Range       | Size   | Description                    |
//! |---------------------|--------|--------------------------------|
//! | 0x00000000-0x00003FFF | 16KB  | BIOS ROM                      |
//! | 0x02000000-0x0203FFFF | 256KB | External WRAM (on-board)       |
//! | 0x03000000-0x03007FFF | 32KB  | Internal WRAM (on-chip)        |
//! | 0x04000000-0x040003FE | 1KB   | I/O Registers                  |
//! | 0x05000000-0x050003FF | 1KB   | Palette RAM                    |
//! | 0x06000000-0x06017FFF | 96KB  | VRAM                           |
//! | 0x07000000-0x070003FF | 1KB   | OAM                            |
//! | 0x08000000-0x09FFFFFF | 32MB  | Game Pak ROM (Wait State 0)    |
//! | 0x0A000000-0x0BFFFFFF | 32MB  | Game Pak ROM (Wait State 1)    |
//! | 0x0C000000-0x0DFFFFFF | 32MB  | Game Pak ROM (Wait State 2)    |
//! | 0x0E000000-0x0E00FFFF | 64KB  | Game Pak SRAM                  |

use emu_core::cpu_arm7tdmi::{Arm7Tdmi, MemoryArm7};
use emu_core::debug::Debugger;
use emu_core::{types::Frame, MountPointInfo, System};
use serde_json::Value;
use thiserror::Error;

pub mod cartridge;
pub mod debugger;
pub mod dma;
pub mod ppu;
pub mod timers;

#[derive(Debug, Error)]
pub enum GbaError {
    #[error("Cartridge not loaded")]
    NoCartridge,
    #[error("Invalid mount point: {0}")]
    InvalidMountPoint(String),
}

// =============================================================================
// GBA Memory Bus
// =============================================================================

/// GBA memory bus implementing the ARM7TDMI memory interface.
///
/// Handles the full GBA memory map with mirroring and wait states.
pub struct GbaBus {
    /// BIOS ROM (16KB) - read-only after boot, protected
    bios: Vec<u8>,
    /// External WRAM (256KB) - on-board, 2 wait states
    ewram: Vec<u8>,
    /// Internal WRAM (32KB) - on-chip, 0 wait states
    iwram: Vec<u8>,
    /// I/O registers (1KB)
    io: Vec<u8>,
    /// Palette RAM (1KB) - BG and OBJ palettes
    palette: Vec<u8>,
    /// Video RAM (96KB)
    vram: Vec<u8>,
    /// Object Attribute Memory (1KB)
    oam: Vec<u8>,
    /// Cartridge ROM (up to 32MB)
    rom: Vec<u8>,
    /// Cartridge SRAM (up to 64KB)
    sram: Vec<u8>,
    /// DMA controller (4 channels)
    dma: dma::Dma,
    /// Hardware timers (4 × 16-bit)
    timers: timers::Timers,
    /// Whether an IRQ is currently pending (IE & IF & IME)
    irq_pending: bool,
    /// Interrupt Master Enable (0x04000208)
    ime: bool,
    /// Interrupt Enable (0x04000200)
    ie: u16,
    /// Interrupt Request Flags (0x04000202)
    if_flags: u16,
}

impl std::fmt::Debug for GbaBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GbaBus")
            .field("rom_size", &self.rom.len())
            .field("dma", &self.dma)
            .field("timers", &self.timers)
            .field("ime", &self.ime)
            .field("ie", &self.ie)
            .field("if_flags", &self.if_flags)
            .finish()
    }
}

impl Default for GbaBus {
    fn default() -> Self {
        Self::new()
    }
}

impl GbaBus {
    pub fn new() -> Self {
        // Stub BIOS with essential handlers for HLE operation
        let mut bios = vec![0u8; 0x4000];

        // 0x00: Reset vector - infinite loop (B 0x00)
        Self::write_arm_word(&mut bios, 0x00, 0xEAFF_FFFE);

        // 0x08: SWI vector - just return (safety net for unhandled SWIs)
        // MOVS PC, LR restores CPSR from SPSR and returns
        Self::write_arm_word(&mut bios, 0x08, 0xE1B0_F00E);

        // 0x18-0x2C: IRQ handler stub
        // This matches the real GBA BIOS IRQ handler behavior:
        // 1. Save context on IRQ stack
        // 2. Load game's IRQ handler from [0x03FFFFFC] (IWRAM mirror of 0x03007FFC)
        // 3. Set return address and call handler
        // 4. Restore context and return from exception
        //
        // 0x18: STMFD SP!, {R0-R3, R12, LR}  - save registers on IRQ stack
        Self::write_arm_word(&mut bios, 0x18, 0xE92D_500F);
        // 0x1C: MOV R0, #0x04000000           - load I/O base address
        Self::write_arm_word(&mut bios, 0x1C, 0xE3A0_0301);
        // 0x20: ADD LR, PC, #0                - set return address to 0x28
        Self::write_arm_word(&mut bios, 0x20, 0xE28F_E000);
        // 0x24: LDR PC, [R0, #-4]             - jump to handler at [0x03FFFFFC]
        Self::write_arm_word(&mut bios, 0x24, 0xE510_F004);
        // 0x28: LDMFD SP!, {R0-R3, R12, LR}  - restore registers
        Self::write_arm_word(&mut bios, 0x28, 0xE8BD_500F);
        // 0x2C: SUBS PC, LR, #4               - return from IRQ (restores CPSR)
        Self::write_arm_word(&mut bios, 0x2C, 0xE25E_F004);

        Self {
            bios,
            ewram: vec![0; 0x40000], // 256KB
            iwram: vec![0; 0x8000],  // 32KB
            io: vec![0; 0x400],      // 1KB
            palette: vec![0; 0x400], // 1KB
            vram: vec![0; 0x18000],  // 96KB
            oam: vec![0; 0x400],     // 1KB
            rom: Vec::new(),
            sram: vec![0; 0x10000], // 64KB
            dma: dma::Dma::new(),
            timers: timers::Timers::new(),
            irq_pending: false,
            ime: false,
            ie: 0,
            if_flags: 0,
        }
    }

    /// Write a 32-bit ARM instruction to the BIOS buffer (little-endian)
    fn write_arm_word(bios: &mut [u8], offset: usize, word: u32) {
        bios[offset] = word as u8;
        bios[offset + 1] = (word >> 8) as u8;
        bios[offset + 2] = (word >> 16) as u8;
        bios[offset + 3] = (word >> 24) as u8;
    }

    /// Load cartridge ROM data
    pub fn load_rom(&mut self, data: &[u8]) {
        self.rom = data.to_vec();
    }

    /// Clear cartridge ROM
    pub fn unload_rom(&mut self) {
        self.rom.clear();
    }

    /// Request an interrupt
    pub fn request_interrupt(&mut self, irq_bit: u16) {
        self.if_flags |= irq_bit;
        self.update_irq_line();
    }

    /// Update the aggregate IRQ pending flag
    fn update_irq_line(&mut self) {
        self.irq_pending = self.ime && (self.ie & self.if_flags) != 0;
    }

    /// Read an I/O register
    fn io_read(&self, addr: u32) -> u8 {
        let offset = (addr - 0x04000000) as usize;

        match addr {
            // DISPCNT (Display Control)
            0x04000000..=0x04000001 => self.io.get(offset).copied().unwrap_or(0),

            // DISPSTAT (Display Status) - bits 0-2 are PPU state flags
            0x04000004..=0x04000005 => self.io.get(offset).copied().unwrap_or(0),

            // VCOUNT (Vertical Counter) - current scanline
            0x04000006 => self.io.get(offset).copied().unwrap_or(0),
            0x04000007 => 0, // VCOUNT is only 8 bits

            // DMA registers (0x040000B0-0x040000DF)
            0x040000B0..=0x040000DF => self.dma.read(addr - 0x040000B0),

            // Timer registers (0x04000100-0x0400010F)
            0x04000100..=0x0400010F => self.timers.read(addr - 0x04000100),

            // KEYINPUT (Key Status) - all keys released = 0x03FF
            0x04000130 => 0xFF,
            0x04000131 => 0x03,

            // IE (Interrupt Enable)
            0x04000200 => self.ie as u8,
            0x04000201 => (self.ie >> 8) as u8,

            // IF (Interrupt Request Flags)
            0x04000202 => self.if_flags as u8,
            0x04000203 => (self.if_flags >> 8) as u8,

            // IME (Interrupt Master Enable)
            0x04000208 => self.ime as u8,
            0x04000209 => 0,

            // Other I/O registers
            _ => self.io.get(offset).copied().unwrap_or(0),
        }
    }

    /// Write an I/O register
    fn io_write(&mut self, addr: u32, val: u8) {
        let offset = (addr - 0x04000000) as usize;

        match addr {
            // IE (Interrupt Enable)
            0x04000200 => {
                self.ie = (self.ie & 0xFF00) | val as u16;
                self.update_irq_line();
            }
            0x04000201 => {
                self.ie = (self.ie & 0x00FF) | ((val as u16) << 8);
                self.update_irq_line();
            }

            // IF (Interrupt Flags) - writing 1 acknowledges/clears the flag
            0x04000202 => {
                self.if_flags &= !(val as u16);
                self.update_irq_line();
            }
            0x04000203 => {
                self.if_flags &= !((val as u16) << 8);
                self.update_irq_line();
            }

            // IME (Interrupt Master Enable)
            0x04000208 => {
                self.ime = val & 1 != 0;
                self.update_irq_line();
            }

            // DMA registers (0x040000B0-0x040000DF)
            0x040000B0..=0x040000DF => {
                self.dma.write(addr - 0x040000B0, val);
            }

            // Timer registers (0x04000100-0x0400010F)
            0x04000100..=0x0400010F => {
                self.timers.write(addr - 0x04000100, val);
            }

            // HALTCNT (0x04000301) - Halt/Stop
            0x04000301 => {
                // TODO: Implement halt/stop modes
            }

            // Other I/O registers
            _ => {
                if offset < self.io.len() {
                    self.io[offset] = val;
                }
            }
        }
    }
}

impl MemoryArm7 for GbaBus {
    fn read_byte(&self, addr: u32) -> u8 {
        match addr {
            // BIOS (0x00000000 - 0x00003FFF)
            0x00000000..=0x00003FFF => self.bios.get(addr as usize).copied().unwrap_or(0),

            // External WRAM (0x02000000 - 0x0203FFFF, mirrored)
            0x02000000..=0x02FFFFFF => {
                let offset = (addr & 0x3FFFF) as usize;
                self.ewram[offset]
            }

            // Internal WRAM (0x03000000 - 0x03007FFF, mirrored)
            0x03000000..=0x03FFFFFF => {
                let offset = (addr & 0x7FFF) as usize;
                self.iwram[offset]
            }

            // I/O Registers (0x04000000 - 0x040003FE)
            0x04000000..=0x04FFFFFF => self.io_read(addr),

            // Palette RAM (0x05000000 - 0x050003FF, mirrored)
            0x05000000..=0x05FFFFFF => {
                let offset = (addr & 0x3FF) as usize;
                self.palette[offset]
            }

            // VRAM (0x06000000 - 0x06017FFF, mirrored)
            0x06000000..=0x06FFFFFF => {
                let offset = (addr & 0x1FFFF) as usize;
                // VRAM is 96KB, addresses 0x18000-0x1FFFF mirror 0x10000-0x17FFF
                let offset = if offset >= 0x18000 {
                    offset - 0x8000
                } else {
                    offset
                };
                self.vram.get(offset).copied().unwrap_or(0)
            }

            // OAM (0x07000000 - 0x070003FF, mirrored)
            0x07000000..=0x07FFFFFF => {
                let offset = (addr & 0x3FF) as usize;
                self.oam[offset]
            }

            // Cartridge ROM (0x08000000 - 0x0DFFFFFF, 3 wait state regions)
            0x08000000..=0x0DFFFFFF => {
                let offset = (addr & 0x01FFFFFF) as usize;
                self.rom.get(offset).copied().unwrap_or(0)
            }

            // Cartridge SRAM (0x0E000000 - 0x0E00FFFF)
            0x0E000000..=0x0EFFFFFF => {
                let offset = (addr & 0xFFFF) as usize;
                self.sram.get(offset).copied().unwrap_or(0)
            }

            // Unused / open bus
            _ => 0,
        }
    }

    fn read_halfword(&self, addr: u32) -> u16 {
        let addr = addr & !1; // Force alignment
        let lo = self.read_byte(addr) as u16;
        let hi = self.read_byte(addr + 1) as u16;
        lo | (hi << 8)
    }

    fn read_word(&self, addr: u32) -> u32 {
        let addr = addr & !3; // Force alignment
        let b0 = self.read_byte(addr) as u32;
        let b1 = self.read_byte(addr + 1) as u32;
        let b2 = self.read_byte(addr + 2) as u32;
        let b3 = self.read_byte(addr + 3) as u32;
        b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
    }

    fn write_byte(&mut self, addr: u32, val: u8) {
        match addr {
            // BIOS is read-only
            0x00000000..=0x00003FFF => {}

            // External WRAM
            0x02000000..=0x02FFFFFF => {
                let offset = (addr & 0x3FFFF) as usize;
                self.ewram[offset] = val;
            }

            // Internal WRAM
            0x03000000..=0x03FFFFFF => {
                let offset = (addr & 0x7FFF) as usize;
                self.iwram[offset] = val;
            }

            // I/O Registers
            0x04000000..=0x04FFFFFF => self.io_write(addr, val),

            // Palette RAM (byte writes are special: write to both bytes of halfword)
            0x05000000..=0x05FFFFFF => {
                let offset = (addr & 0x3FE) as usize; // Force halfword alignment
                self.palette[offset] = val;
                self.palette[offset + 1] = val;
            }

            // VRAM (byte writes: write to both bytes of halfword, OBJ region ignores)
            0x06000000..=0x06FFFFFF => {
                let offset = (addr & 0x1FFFF) as usize;
                let offset = if offset >= 0x18000 {
                    offset - 0x8000
                } else {
                    offset
                };
                // Only BG VRAM (< 0x10000 in bitmap modes, < 0x14000 otherwise)
                if offset < 0x10000 {
                    let aligned = offset & !1;
                    if aligned + 1 < self.vram.len() {
                        self.vram[aligned] = val;
                        self.vram[aligned + 1] = val;
                    }
                }
            }

            // OAM (byte writes are ignored)
            0x07000000..=0x07FFFFFF => {}

            // Cartridge ROM is read-only
            0x08000000..=0x0DFFFFFF => {}

            // Cartridge SRAM
            0x0E000000..=0x0EFFFFFF => {
                let offset = (addr & 0xFFFF) as usize;
                if offset < self.sram.len() {
                    self.sram[offset] = val;
                }
            }

            _ => {}
        }
    }

    fn write_halfword(&mut self, addr: u32, val: u16) {
        let addr = addr & !1;
        self.write_byte(addr, val as u8);
        self.write_byte(addr + 1, (val >> 8) as u8);
    }

    fn write_word(&mut self, addr: u32, val: u32) {
        let addr = addr & !3;
        self.write_byte(addr, val as u8);
        self.write_byte(addr + 1, (val >> 8) as u8);
        self.write_byte(addr + 2, (val >> 16) as u8);
        self.write_byte(addr + 3, (val >> 24) as u8);
    }

    fn irq_pending(&self) -> bool {
        self.irq_pending
    }
}

// =============================================================================
// GBA System
// =============================================================================

/// GBA clock speed: 16.78 MHz (2^24 Hz = 16,777,216 Hz)
#[allow(dead_code)] // Used for reference timing calculations
const CPU_FREQ: u64 = 16_777_216;

/// Cycles per scanline: 1232 (280896 cycles/frame ÷ 228 scanlines)
const CYCLES_PER_SCANLINE: u64 = 1232;

/// Visible scanlines: 160
const VISIBLE_SCANLINES: u32 = 160;

/// Total scanlines per frame: 228 (160 visible + 68 VBlank)
const TOTAL_SCANLINES: u32 = 228;

/// Cycles per frame: 280896
const CYCLES_PER_FRAME: u64 = CYCLES_PER_SCANLINE * TOTAL_SCANLINES as u64;

// I/O register offsets
const REG_DISPSTAT: usize = 0x004;
const REG_VCOUNT: usize = 0x006;

// DISPSTAT bits
const DISPSTAT_VBLANK: u8 = 1 << 0;
#[allow(dead_code)] // Will be used when HBlank timing is fully implemented
const DISPSTAT_HBLANK: u8 = 1 << 1;
const DISPSTAT_VCOUNT_MATCH: u8 = 1 << 2;
const DISPSTAT_VBLANK_IRQ: u8 = 1 << 3;
const DISPSTAT_HBLANK_IRQ: u8 = 1 << 4;
const DISPSTAT_VCOUNT_IRQ: u8 = 1 << 5;

// IRQ bits
const IRQ_VBLANK: u16 = 1 << 0;
const IRQ_HBLANK: u16 = 1 << 1;
const IRQ_VCOUNT: u16 = 1 << 2;

pub struct GbaSystem {
    cpu: Arm7Tdmi<GbaBus>,
    ppu: ppu::Ppu,
    total_cycles: u64,
    scanline: u32,
    scanline_cycles: u64,
    /// Parsed cartridge header (set on mount)
    header: Option<cartridge::GbaCartridgeHeader>,
    /// Instruction tracer for debugging
    instruction_tracer: emu_core::instruction_tracer::InstructionTracer,
}

impl std::fmt::Debug for GbaSystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GbaSystem")
            .field("total_cycles", &self.total_cycles)
            .field("scanline", &self.scanline)
            .field("ppu", &self.ppu)
            .field("header", &self.header)
            .finish()
    }
}

impl Default for GbaSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl GbaSystem {
    pub fn new() -> Self {
        let bus = GbaBus::new();
        let cpu = Arm7Tdmi::new(bus);

        Self {
            cpu,
            ppu: ppu::Ppu::new(),
            total_cycles: 0,
            scanline: 0,
            scanline_cycles: 0,
            header: None,
            instruction_tracer: emu_core::instruction_tracer::InstructionTracer::new(),
        }
    }

    emu_core::impl_instruction_tracer_methods!();

    /// Check if a ROM is loaded
    fn has_rom(&self) -> bool {
        !self.cpu.memory.rom.is_empty()
    }

    /// Get the parsed cartridge header (if a ROM is loaded)
    pub fn cartridge_header(&self) -> Option<&cartridge::GbaCartridgeHeader> {
        self.header.as_ref()
    }

    /// Execute any pending DMA transfers.
    ///
    /// DMA transfers read/write through the memory bus, bypassing I/O register
    /// side effects for DMA source/dest registers themselves.
    /// Returns the number of CPU cycles consumed by the DMA.
    fn execute_dma(&mut self) -> u64 {
        if !self.cpu.memory.dma.is_transferring() {
            return 0;
        }

        // Take DMA out to avoid borrow conflicts between DMA and bus
        let mut dma = std::mem::take(&mut self.cpu.memory.dma);

        let (cycles, irq_bits) = dma.execute_with_bus(&mut self.cpu.memory);

        // Put DMA back
        self.cpu.memory.dma = dma;

        if irq_bits != 0 {
            self.cpu.memory.request_interrupt(irq_bits);
        }

        cycles
    }

    /// Update DISPSTAT and VCOUNT I/O registers for current scanline
    fn update_display_status(&mut self) {
        // VCOUNT register
        self.cpu.memory.io[REG_VCOUNT] = self.scanline as u8;

        // DISPSTAT register
        let dispstat = self.cpu.memory.io[REG_DISPSTAT];
        let vcount_target = self.cpu.memory.io[REG_DISPSTAT + 1];

        let in_vblank = self.scanline >= VISIBLE_SCANLINES;
        let vcount_match = self.scanline as u8 == vcount_target;

        let mut new_dispstat = dispstat & !(DISPSTAT_VBLANK | DISPSTAT_VCOUNT_MATCH);
        if in_vblank {
            new_dispstat |= DISPSTAT_VBLANK;
        }
        if vcount_match {
            new_dispstat |= DISPSTAT_VCOUNT_MATCH;
        }
        self.cpu.memory.io[REG_DISPSTAT] = new_dispstat;

        // Fire interrupts
        if in_vblank && self.scanline == VISIBLE_SCANLINES && dispstat & DISPSTAT_VBLANK_IRQ != 0 {
            self.cpu.memory.request_interrupt(IRQ_VBLANK);
        }
        if vcount_match && dispstat & DISPSTAT_VCOUNT_IRQ != 0 {
            self.cpu.memory.request_interrupt(IRQ_VCOUNT);
        }
    }
}

impl System for GbaSystem {
    type Error = GbaError;

    fn reset(&mut self) {
        self.cpu.reset();
        self.ppu.reset();
        self.cpu.memory.dma.reset();
        self.cpu.memory.timers.reset();
        self.total_cycles = 0;
        self.scanline = 0;
        self.scanline_cycles = 0;
        // header is preserved across resets (only cleared on unmount)
    }

    fn step_frame(&mut self) -> Result<Frame, Self::Error> {
        if !self.has_rom() {
            return Err(GbaError::NoCartridge);
        }

        // Run CPU for one frame's worth of cycles
        let frame_end = self.total_cycles + CYCLES_PER_FRAME;

        while self.total_cycles < frame_end {
            // Execute one CPU instruction
            let pc_before = self.cpu.pc();
            let cycles = self.cpu.step() as u64;
            self.total_cycles += cycles;
            self.scanline_cycles += cycles;

            // Record instruction if tracing is enabled
            if self.instruction_tracer.is_enabled() {
                if let Some(instr) = self.disassemble_instruction(pc_before) {
                    let cpu_state = self.get_cpu_state();
                    self.instruction_tracer.trace(instr, cpu_state);
                }

                if self.instruction_tracer.history_size()
                    >= self.instruction_tracer.get_max_history()
                {
                    // Prevent long-running traces from stalling the UI.
                    self.instruction_tracer.set_enabled(false);
                }
            }

            // Tick hardware timers
            let timer_irqs = self.cpu.memory.timers.tick(cycles as u32);
            if timer_irqs != 0 {
                self.cpu.memory.request_interrupt(timer_irqs);
            }

            // Execute any pending immediate DMA transfers
            if self.cpu.memory.dma.has_pending_immediate() {
                let dma_cycles = self.execute_dma();
                self.total_cycles += dma_cycles;
                self.scanline_cycles += dma_cycles;
            }

            // Check if we've completed a scanline
            if self.scanline_cycles >= CYCLES_PER_SCANLINE {
                self.scanline_cycles -= CYCLES_PER_SCANLINE;

                // HBlank IRQ at end of each visible scanline
                if self.scanline < VISIBLE_SCANLINES {
                    let dispstat = self.cpu.memory.io[REG_DISPSTAT];
                    if dispstat & DISPSTAT_HBLANK_IRQ != 0 {
                        self.cpu.memory.request_interrupt(IRQ_HBLANK);
                    }

                    // Trigger HBlank DMA
                    self.cpu
                        .memory
                        .dma
                        .notify_timing(dma::DmaStartTiming::HBlank);
                    let dma_cycles = self.execute_dma();
                    self.total_cycles += dma_cycles;
                    self.scanline_cycles += dma_cycles;

                    // Render this scanline via PPU
                    self.ppu.render_scanline(
                        self.scanline,
                        &self.cpu.memory.io,
                        &self.cpu.memory.palette,
                        &self.cpu.memory.vram,
                        &self.cpu.memory.oam,
                    );
                }

                // Advance to next scanline
                self.scanline += 1;

                // VBlank start
                if self.scanline == VISIBLE_SCANLINES {
                    self.ppu.on_vblank(&self.cpu.memory.io);

                    // Trigger VBlank DMA
                    self.cpu
                        .memory
                        .dma
                        .notify_timing(dma::DmaStartTiming::VBlank);
                    let dma_cycles = self.execute_dma();
                    self.total_cycles += dma_cycles;
                    self.scanline_cycles += dma_cycles;
                }

                if self.scanline >= TOTAL_SCANLINES {
                    self.scanline = 0;
                    // Re-latch affine registers at frame start
                    self.ppu.latch_affine_registers(&self.cpu.memory.io);
                }

                // Update display registers
                self.update_display_status();
            }
        }

        Ok(self.ppu.clone_frame())
    }

    fn save_state(&self) -> Value {
        // TODO: Full state serialization (CPU regs, all memory, I/O, PPU state)
        serde_json::json!({
            "system": "gba",
            "version": 1,
            "total_cycles": self.total_cycles,
            "scanline": self.scanline,
        })
    }

    fn load_state(&mut self, _v: &Value) -> Result<(), serde_json::Error> {
        // TODO: Full state deserialization
        Ok(())
    }

    fn mount_points(&self) -> Vec<MountPointInfo> {
        vec![MountPointInfo {
            id: "Cartridge".to_string(),
            name: "Cartridge Slot".to_string(),
            extensions: vec!["gba".to_string(), "agb".to_string()],
            required: true,
        }]
    }

    fn mount(&mut self, mount_point_id: &str, data: &[u8]) -> Result<(), Self::Error> {
        match mount_point_id {
            "Cartridge" => {
                // Parse cartridge header before loading
                self.header = cartridge::GbaCartridgeHeader::from_bytes(data);
                self.cpu.memory.load_rom(data);
                self.reset();

                // After reset, set PC to ROM entry point
                // GBA ROMs start at 0x08000000 and have an ARM branch instruction
                // at the cartridge header. The BIOS normally jumps here after boot.
                // For now, skip BIOS and jump directly to ROM.
                self.cpu.gpr[15] = 0x08000000;
                // Set initial SP values (as BIOS would)
                self.cpu.gpr[13] = 0x03007F00; // SP_usr/sys
                                               // Switch to System mode (post-BIOS state)
                self.cpu.cpsr = 0x1F; // System mode, ARM, IRQ+FIQ enabled

                // Initialize banked stack pointers (as the real BIOS does)
                // SP_irq = 0x03007FA0
                self.cpu
                    .set_banked_sp(emu_core::cpu_arm7tdmi::ProcessorMode::Irq, 0x03007FA0);
                // SP_svc = 0x03007FE0
                self.cpu.set_banked_sp(
                    emu_core::cpu_arm7tdmi::ProcessorMode::Supervisor,
                    0x03007FE0,
                );

                Ok(())
            }
            _ => Err(GbaError::InvalidMountPoint(mount_point_id.to_string())),
        }
    }

    fn unmount(&mut self, mount_point_id: &str) -> Result<(), Self::Error> {
        match mount_point_id {
            "Cartridge" => {
                self.cpu.memory.unload_rom();
                self.header = None;
                Ok(())
            }
            _ => Err(GbaError::InvalidMountPoint(mount_point_id.to_string())),
        }
    }

    fn is_mounted(&self, mount_point_id: &str) -> bool {
        match mount_point_id {
            "Cartridge" => !self.cpu.memory.rom.is_empty(),
            _ => false,
        }
    }

    fn get_total_cycles(&self) -> u64 {
        self.total_cycles
    }

    fn debugger(&self) -> Option<&dyn emu_core::debug::Debugger> {
        Some(self)
    }
}
