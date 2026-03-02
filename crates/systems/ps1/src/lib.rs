//! PlayStation 1 (PS1/PSX) Emulator
//!
//! ## Hardware Overview
//! - **CPU**: MIPS R3000A @ 33.8688 MHz (MIPS I ISA, 32-bit)
//! - **GPU**: Custom 2D/3D graphics processor, 1 MB VRAM
//! - **SPU**: 24-channel ADPCM audio, 512 KB sound RAM
//! - **RAM**: 2 MB main RAM
//! - **BIOS**: 512 KB ROM
//! - **CD-ROM**: 2x speed CD drive
//! - **GTE**: Geometry Transform Engine (COP2) — hardware 3D math
//! - **MDEC**: Motion Decoder (for FMV playback)
//!
//! ## Memory Map (Physical)
//! | Address Range       | Size   | Description          |
//! |---------------------|--------|----------------------|
//! | 0x00000000-0x001FFFFF | 2 MB  | Main RAM             |
//! | 0x1F000000-0x1F7FFFFF | 8 MB  | Expansion Region 1  |
//! | 0x1F800000-0x1F8003FF | 1 KB  | Scratchpad (D-Cache) |
//! | 0x1F801000-0x1F801FFF | 4 KB  | I/O Ports            |
//! | 0x1F802000-0x1F802FFF | 4 KB  | Expansion Region 2  |
//! | 0x1FC00000-0x1FC7FFFF | 512 KB | BIOS ROM            |
//!
//! ## References
//! - nocash PSX specifications: https://problemkaputt.de/psx-spx.htm
//! - Avocado PS1 emulator
//! - Rustation PS1 emulator

pub mod gpu;
pub mod spu;

use emu_core::cpu_mips_r3000a::{CpuR3000A, MemoryR3000A};
use emu_core::renderer::Renderer;
use emu_core::types::Frame;
use emu_core::{MountPointInfo, System};
use serde_json::Value;
use thiserror::Error;

use gpu::Gpu;
use spu::Spu;

// ============================================================================
// Constants
// ============================================================================

/// CPU clock frequency: 33.8688 MHz
#[allow(dead_code)]
const CPU_CLOCK_HZ: u64 = 33_868_800;

/// GPU dot clock (NTSC): ~53.693 MHz (not directly used, but useful reference)
#[allow(dead_code)]
const GPU_DOT_CLOCK_HZ: u64 = 53_693_175;

/// Scanlines per frame (NTSC)
const SCANLINES_NTSC: u32 = 263;
/// Scanlines per frame (PAL)
#[allow(dead_code)]
const SCANLINES_PAL: u32 = 314;

/// CPU cycles per scanline (~128.67 for NTSC 60fps)
/// 33868800 / 60 / 263 ≈ 2148
const CYCLES_PER_SCANLINE: u32 = 2148;

/// Main RAM size: 2 MB
const RAM_SIZE: usize = 2 * 1024 * 1024;
/// BIOS ROM size: 512 KB
const BIOS_SIZE: usize = 512 * 1024;
/// Scratchpad size: 1 KB
const SCRATCHPAD_SIZE: usize = 1024;
/// Expansion 1 size
#[allow(dead_code)]
const EXP1_SIZE: usize = 8 * 1024 * 1024;

// ============================================================================
// Error type
// ============================================================================

#[derive(Debug, Error)]
pub enum Ps1Error {
    #[error("No BIOS loaded")]
    NoBios,
    #[error("Invalid BIOS size: {0} bytes (expected {BIOS_SIZE})")]
    InvalidBiosSize(usize),
    #[error("Invalid ROM/disc image")]
    InvalidImage,
    #[error("I/O error: {0}")]
    Io(String),
}

// ============================================================================
// DMA — Direct Memory Access
// ============================================================================

/// DMA channel state
#[derive(Debug, Clone, Default)]
struct DmaChannel {
    /// Base address
    base: u32,
    /// Block control
    block_control: u32,
    /// Channel control
    control: u32,
}

impl DmaChannel {
    fn active(&self) -> bool {
        let enable = self.control & (1 << 24) != 0;
        let trigger = self.control & (1 << 28) != 0;
        let sync_mode = (self.control >> 9) & 3;

        enable && (trigger || sync_mode != 0)
    }

    fn direction_to_ram(&self) -> bool {
        self.control & 1 == 0
    }

    fn step_backward(&self) -> bool {
        self.control & (1 << 1) != 0
    }

    fn sync_mode(&self) -> u32 {
        (self.control >> 9) & 3
    }
}

/// DMA controller with 7 channels
struct Dma {
    /// Control register (DPCR)
    control: u32,
    /// Interrupt register (DICR)
    interrupt: u32,
    /// Channels 0-6:
    /// 0: MDEC in, 1: MDEC out, 2: GPU, 3: CD-ROM, 4: SPU, 5: PIO, 6: OTC (reverse clear)
    channels: [DmaChannel; 7],
}

impl Dma {
    fn new() -> Self {
        Self {
            control: 0x0765_4321, // Default: all channels with priorities
            interrupt: 0,
            channels: Default::default(),
        }
    }

    fn reset(&mut self) {
        *self = Self::new();
    }

    fn channel_enabled(&self, ch: usize) -> bool {
        self.control & (1 << (ch * 4 + 3)) != 0
    }

    fn irq_active(&self) -> bool {
        // Force IRQ bit 15
        let force = self.interrupt & (1 << 15) != 0;
        // Master enable bit 23
        let master = self.interrupt & (1 << 23) != 0;
        // Channel IRQ flags (bits 24-30) & enables (bits 16-22)
        let flags = (self.interrupt >> 24) & 0x7F;
        let enables = (self.interrupt >> 16) & 0x7F;
        force || (master && (flags & enables) != 0)
    }
}

// ============================================================================
// Interrupt controller
// ============================================================================

struct InterruptController {
    /// Interrupt status (I_STAT)
    status: u16,
    /// Interrupt mask (I_MASK)
    mask: u16,
}

impl InterruptController {
    fn new() -> Self {
        Self { status: 0, mask: 0 }
    }

    fn reset(&mut self) {
        self.status = 0;
        self.mask = 0;
    }

    /// Raise an interrupt bit
    fn raise(&mut self, bit: u16) {
        self.status |= bit;
    }

    /// Check if any enabled interrupt is pending
    fn pending(&self) -> bool {
        self.status & self.mask != 0
    }
}

// Interrupt bits
const IRQ_VBLANK: u16 = 1 << 0;
#[allow(dead_code)]
const IRQ_GPU: u16 = 1 << 1;
#[allow(dead_code)]
const IRQ_CDROM: u16 = 1 << 2;
const IRQ_DMA: u16 = 1 << 3;
const IRQ_TIMER0: u16 = 1 << 4;
#[allow(dead_code)]
const IRQ_TIMER1: u16 = 1 << 5;
#[allow(dead_code)]
const IRQ_TIMER2: u16 = 1 << 6;
#[allow(dead_code)]
const IRQ_PAD: u16 = 1 << 7;
#[allow(dead_code)]
const IRQ_SIO: u16 = 1 << 8;
#[allow(dead_code)]
const IRQ_SPU: u16 = 1 << 9;

// ============================================================================
// Timers
// ============================================================================

#[derive(Debug, Clone, Default)]
struct Timer {
    counter: u16,
    target: u16,
    mode: u16,
}

impl Timer {
    fn step(&mut self, ticks: u32) -> bool {
        let mut irq = false;
        for _ in 0..ticks {
            self.counter = self.counter.wrapping_add(1);
            if self.counter == self.target {
                // Target reached
                if self.mode & (1 << 4) != 0 {
                    // Reset on target
                    self.counter = 0;
                }
                if self.mode & (1 << 3) != 0 {
                    // IRQ on target
                    irq = true;
                }
            }
            if self.counter == 0xFFFF && self.mode & (1 << 5) != 0 {
                // IRQ on overflow
                irq = true;
            }
        }
        irq
    }
}

// ============================================================================
// PS1 Memory Bus
// ============================================================================

/// PS1 memory bus — connects CPU to RAM, BIOS, GPU, SPU, DMA, etc.
pub struct Ps1Bus {
    /// Main RAM (2 MB)
    ram: Vec<u8>,
    /// BIOS ROM (512 KB)
    bios: Vec<u8>,
    /// Scratchpad / D-Cache (1 KB)
    scratchpad: Vec<u8>,
    /// GPU
    pub gpu: Gpu,
    /// SPU
    pub spu: Spu,
    /// DMA controller
    dma: Dma,
    /// Interrupt controller
    irq: InterruptController,
    /// Timers (3)
    timers: [Timer; 3],
    /// BIOS loaded flag
    bios_loaded: bool,
    /// CD-ROM sector buffer (simplified)
    cdrom_data: Vec<u8>,
    /// CD-ROM data read position
    cdrom_data_pos: usize,
    /// CD-ROM status register
    cdrom_status: u8,
    /// CD-ROM parameter FIFO
    cdrom_params: Vec<u8>,
    /// CD-ROM response FIFO
    cdrom_response: Vec<u8>,
    /// CD-ROM interrupt flag
    cdrom_irq: u8,
    /// CD-ROM interrupt enable
    cdrom_irq_enable: u8,
    /// Disc image data
    disc_data: Vec<u8>,
    /// Joypad state
    joy_data: u16,
    /// Joypad status
    joy_stat: u32,
    /// Joypad control
    joy_ctrl: u16,
    /// Memory control registers
    mem_ctrl: [u32; 9],
    /// RAM size register
    ram_size_reg: u32,
    /// Cache control register
    cache_ctrl: u32,
}

impl Default for Ps1Bus {
    fn default() -> Self {
        Self::new()
    }
}

impl Ps1Bus {
    pub fn new() -> Self {
        Self {
            ram: vec![0; RAM_SIZE],
            bios: vec![0; BIOS_SIZE],
            scratchpad: vec![0; SCRATCHPAD_SIZE],
            gpu: Gpu::new(),
            spu: Spu::new(),
            dma: Dma::new(),
            irq: InterruptController::new(),
            timers: Default::default(),
            bios_loaded: false,
            cdrom_data: Vec::new(),
            cdrom_data_pos: 0,
            cdrom_status: 0x18, // Not busy, parameter fifo empty, can receive
            cdrom_params: Vec::new(),
            cdrom_response: Vec::new(),
            cdrom_irq: 0,
            cdrom_irq_enable: 0,
            disc_data: Vec::new(),
            joy_data: 0xFFFF,
            joy_stat: 0,
            joy_ctrl: 0,
            mem_ctrl: [0; 9],
            ram_size_reg: 0,
            cache_ctrl: 0,
        }
    }

    pub fn reset(&mut self) {
        self.ram.fill(0);
        self.scratchpad.fill(0);
        self.gpu.reset();
        self.spu.reset();
        self.dma.reset();
        self.irq.reset();
        self.timers = Default::default();
        self.cdrom_status = 0x18;
        self.cdrom_params.clear();
        self.cdrom_response.clear();
        self.cdrom_irq = 0;
        self.cdrom_data_pos = 0;
        // Don't clear bios or disc_data
    }

    pub fn load_bios(&mut self, data: &[u8]) -> Result<(), Ps1Error> {
        if data.len() != BIOS_SIZE {
            return Err(Ps1Error::InvalidBiosSize(data.len()));
        }
        self.bios[..BIOS_SIZE].copy_from_slice(data);
        self.bios_loaded = true;
        Ok(())
    }

    pub fn load_exe(&mut self, data: &[u8]) -> Result<(), Ps1Error> {
        // PS-X EXE format: 0x800 byte header + code
        if data.len() < 0x800 || &data[0..8] != b"PS-X EXE" {
            return Err(Ps1Error::InvalidImage);
        }

        let pc = u32::from_le_bytes([data[0x10], data[0x11], data[0x12], data[0x13]]);
        let _gp = u32::from_le_bytes([data[0x14], data[0x15], data[0x16], data[0x17]]);
        let dest = u32::from_le_bytes([data[0x18], data[0x19], data[0x1A], data[0x1B]]);
        let size = u32::from_le_bytes([data[0x1C], data[0x1D], data[0x1E], data[0x1F]]);
        let _sp = u32::from_le_bytes([data[0x30], data[0x31], data[0x32], data[0x33]]);

        // Copy code to RAM
        let code = &data[0x800..];
        let dest_phys = dest & 0x1FFF_FFFF;
        let end = (dest_phys as usize) + (size as usize).min(code.len());
        if end <= self.ram.len() {
            self.ram[dest_phys as usize..end].copy_from_slice(&code[..(end - dest_phys as usize)]);
        }

        // Store initial PC for the CPU to use after BIOS boot
        // (In practice, this is called after BIOS has initialized, or we patch the BIOS)
        let _ = pc; // The system will set PC after BIOS boot

        Ok(())
    }

    // ========================================================================
    // I/O read dispatch
    // ========================================================================

    fn io_read_byte(&self, addr: u32) -> u8 {
        match addr {
            // CD-ROM registers (0x1F801800-0x1F801803)
            0x1F80_1800 => self.cdrom_status,
            0x1F80_1801 => {
                // Response FIFO
                if !self.cdrom_response.is_empty() {
                    self.cdrom_response[0]
                } else {
                    0
                }
            }
            0x1F80_1802 => {
                // Data FIFO
                if self.cdrom_data_pos < self.cdrom_data.len() {
                    self.cdrom_data[self.cdrom_data_pos]
                } else {
                    0
                }
            }
            0x1F80_1803 => self.cdrom_irq_enable | 0xE0,
            _ => 0,
        }
    }

    fn io_read_halfword(&self, addr: u32) -> u16 {
        match addr {
            // Interrupt controller
            0x1F80_1070 => self.irq.status,
            0x1F80_1074 => self.irq.mask,

            // SPU registers (0x1F801C00-0x1F801FFF)
            0x1F80_1C00..=0x1F80_1FFF => {
                let offset = addr - 0x1F80_1C00;
                self.spu.read_register(offset)
            }

            // Timer registers
            0x1F80_1100 => self.timers[0].counter,
            0x1F80_1104 => self.timers[0].mode,
            0x1F80_1108 => self.timers[0].target,
            0x1F80_1110 => self.timers[1].counter,
            0x1F80_1114 => self.timers[1].mode,
            0x1F80_1118 => self.timers[1].target,
            0x1F80_1120 => self.timers[2].counter,
            0x1F80_1124 => self.timers[2].mode,
            0x1F80_1128 => self.timers[2].target,

            // Joypad
            0x1F80_1040 => self.joy_data,
            0x1F80_1044 => self.joy_stat as u16,
            0x1F80_104A => self.joy_ctrl,

            _ => 0,
        }
    }

    fn io_read_word(&self, addr: u32) -> u32 {
        match addr {
            // Interrupt controller
            0x1F80_1070 => self.irq.status as u32,
            0x1F80_1074 => self.irq.mask as u32,

            // DMA registers
            0x1F80_1080..=0x1F80_10EF => {
                let ch = ((addr - 0x1F80_1080) >> 4) as usize;
                let reg = (addr & 0xF) as usize;
                if ch < 7 {
                    match reg {
                        0 => self.dma.channels[ch].base,
                        4 => self.dma.channels[ch].block_control,
                        8 => self.dma.channels[ch].control,
                        _ => 0,
                    }
                } else {
                    0
                }
            }
            0x1F80_10F0 => self.dma.control,
            0x1F80_10F4 => self.dma.interrupt,

            // GPU
            0x1F80_1810 => self.gpu.gpuread(),
            0x1F80_1814 => self.gpu.gpustat(),

            // MDEC (Motion Decoder) — stub
            0x1F80_1820 => 0,           // MDEC data/response
            0x1F80_1824 => 0x8000_0000, // MDEC status: ready, no DMA

            // Timer registers
            0x1F80_1100 => self.timers[0].counter as u32,
            0x1F80_1104 => self.timers[0].mode as u32,
            0x1F80_1108 => self.timers[0].target as u32,
            0x1F80_1110 => self.timers[1].counter as u32,
            0x1F80_1114 => self.timers[1].mode as u32,
            0x1F80_1118 => self.timers[1].target as u32,
            0x1F80_1120 => self.timers[2].counter as u32,
            0x1F80_1124 => self.timers[2].mode as u32,
            0x1F80_1128 => self.timers[2].target as u32,

            // Joypad
            0x1F80_1040 => self.joy_data as u32,
            0x1F80_1044 => self.joy_stat,
            0x1F80_104E => 0, // JOY_BAUD

            // Memory control
            0x1F80_1000..=0x1F80_1020 => {
                let idx = ((addr - 0x1F80_1000) / 4) as usize;
                self.mem_ctrl.get(idx).copied().unwrap_or(0)
            }
            0x1F80_1060 => self.ram_size_reg,

            // SPU
            0x1F80_1C00..=0x1F80_1FFF => {
                let offset = addr - 0x1F80_1C00;
                let lo = self.spu.read_register(offset) as u32;
                let hi = self.spu.read_register(offset + 2) as u32;
                lo | (hi << 16)
            }

            _ => 0,
        }
    }

    // ========================================================================
    // I/O write dispatch
    // ========================================================================

    fn io_write_byte(&mut self, addr: u32, val: u8) {
        match addr {
            // CD-ROM registers
            0x1F80_1800 => {
                // Index register
                self.cdrom_status = (self.cdrom_status & !3) | (val & 3);
            }
            0x1F80_1801 => {
                // Command register (index 0)
                self.cdrom_execute_command(val);
            }
            0x1F80_1802 => {
                let index = self.cdrom_status & 3;
                match index {
                    0 => self.cdrom_params.push(val), // Parameter FIFO
                    1 => self.cdrom_irq_enable = val & 0x1F,
                    _ => {}
                }
            }
            0x1F80_1803 => {
                let index = self.cdrom_status & 3;
                match index {
                    0 => {} // Request register
                    1 => {
                        // Interrupt flag reset
                        self.cdrom_irq &= !(val & 0x1F);
                        if val & 0x40 != 0 {
                            self.cdrom_params.clear();
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn io_write_halfword(&mut self, addr: u32, val: u16) {
        match addr {
            // Interrupt controller
            0x1F80_1070 => self.irq.status &= val, // Write 0 to acknowledge
            0x1F80_1074 => self.irq.mask = val,

            // SPU registers
            0x1F80_1C00..=0x1F80_1FFF => {
                let offset = addr - 0x1F80_1C00;
                self.spu.write_register(offset, val);
            }

            // Timer registers
            0x1F80_1100 => self.timers[0].counter = val,
            0x1F80_1104 => self.timers[0].mode = val,
            0x1F80_1108 => self.timers[0].target = val,
            0x1F80_1110 => self.timers[1].counter = val,
            0x1F80_1114 => self.timers[1].mode = val,
            0x1F80_1118 => self.timers[1].target = val,
            0x1F80_1120 => self.timers[2].counter = val,
            0x1F80_1124 => self.timers[2].mode = val,
            0x1F80_1128 => self.timers[2].target = val,

            // Joypad
            0x1F80_104A => self.joy_ctrl = val,

            _ => {}
        }
    }

    fn io_write_word(&mut self, addr: u32, val: u32) {
        match addr {
            // Interrupt controller
            0x1F80_1070 => self.irq.status &= val as u16,
            0x1F80_1074 => self.irq.mask = val as u16,

            // DMA registers
            0x1F80_1080..=0x1F80_10EF => {
                let ch = ((addr - 0x1F80_1080) >> 4) as usize;
                let reg = (addr & 0xF) as usize;
                if ch < 7 {
                    match reg {
                        0 => self.dma.channels[ch].base = val & 0x00FF_FFFC,
                        4 => self.dma.channels[ch].block_control = val,
                        8 => {
                            self.dma.channels[ch].control = val;
                            if self.dma.channels[ch].active() && self.dma.channel_enabled(ch) {
                                self.execute_dma(ch);
                            }
                        }
                        _ => {}
                    }
                }
            }
            0x1F80_10F0 => self.dma.control = val,
            0x1F80_10F4 => {
                // DICR: bits 24-30 are acknowledged by writing 1
                let ack = (val >> 24) & 0x7F;
                let prev = self.dma.interrupt;
                self.dma.interrupt = (val & 0x00FF_803F) | ((prev & !(ack << 24)) & 0x7F00_0000);
            }

            // GPU
            0x1F80_1810 => self.gpu.gp0_write(val),
            0x1F80_1814 => self.gpu.gp1_write(val),

            // MDEC (Motion Decoder) — stub
            0x1F80_1820 => {} // MDEC command/parameter
            0x1F80_1824 => {} // MDEC control/reset

            // Timer registers (word access)
            0x1F80_1100 => self.timers[0].counter = val as u16,
            0x1F80_1104 => self.timers[0].mode = val as u16,
            0x1F80_1108 => self.timers[0].target = val as u16,
            0x1F80_1110 => self.timers[1].counter = val as u16,
            0x1F80_1114 => self.timers[1].mode = val as u16,
            0x1F80_1118 => self.timers[1].target = val as u16,
            0x1F80_1120 => self.timers[2].counter = val as u16,
            0x1F80_1124 => self.timers[2].mode = val as u16,
            0x1F80_1128 => self.timers[2].target = val as u16,

            // Joypad
            0x1F80_1040 => self.joy_data = val as u16,
            0x1F80_1044 => {} // Read-only status
            0x1F80_104A => self.joy_ctrl = val as u16,

            // Memory control
            0x1F80_1000..=0x1F80_1020 => {
                let idx = ((addr - 0x1F80_1000) / 4) as usize;
                if idx < self.mem_ctrl.len() {
                    self.mem_ctrl[idx] = val;
                }
            }
            0x1F80_1060 => self.ram_size_reg = val,

            // SPU
            0x1F80_1C00..=0x1F80_1FFF => {
                let offset = addr - 0x1F80_1C00;
                self.spu.write_register(offset, val as u16);
                self.spu.write_register(offset + 2, (val >> 16) as u16);
            }

            // Cache control (FFFE0130)
            0x1FFE_0130 => self.cache_ctrl = val,

            _ => {}
        }
    }

    // ========================================================================
    // DMA execution
    // ========================================================================

    fn execute_dma(&mut self, ch: usize) {
        let channel = self.dma.channels[ch].clone();
        match channel.sync_mode() {
            0 => self.dma_block_transfer(ch, &channel),
            1 => self.dma_block_transfer(ch, &channel),
            2 => self.dma_linked_list(ch, &channel),
            _ => {}
        }

        // Mark transfer complete
        self.dma.channels[ch].control &= !(1 << 24); // Clear enable bit
        self.dma.channels[ch].control &= !(1 << 28); // Clear trigger

        // Set channel interrupt flag
        self.dma.interrupt |= 1 << (24 + ch);
        if self.dma.irq_active() {
            self.irq.raise(IRQ_DMA);
        }
    }

    fn dma_block_transfer(&mut self, ch: usize, channel: &DmaChannel) {
        let mut addr = channel.base & 0x1F_FFFC;
        let word_count = match channel.sync_mode() {
            0 => {
                let n = channel.block_control & 0xFFFF;
                if n == 0 {
                    0x10000
                } else {
                    n
                }
            }
            1 => {
                let block_size = channel.block_control & 0xFFFF;
                let block_count = (channel.block_control >> 16) & 0xFFFF;
                block_size * block_count
            }
            _ => return,
        };

        let step: i32 = if channel.step_backward() { -4 } else { 4 };

        if channel.direction_to_ram() {
            // Device → RAM
            for _ in 0..word_count {
                let data = match ch {
                    2 => self.gpu.gpuread(),     // GPU read
                    3 => self.cdrom_read_word(), // CD-ROM
                    _ => 0,
                };
                let phys = (addr & 0x1F_FFFF) as usize;
                if phys + 3 < self.ram.len() {
                    self.ram[phys] = data as u8;
                    self.ram[phys + 1] = (data >> 8) as u8;
                    self.ram[phys + 2] = (data >> 16) as u8;
                    self.ram[phys + 3] = (data >> 24) as u8;
                }
                addr = (addr as i32 + step) as u32;
            }
        } else {
            // RAM → Device
            for _ in 0..word_count {
                let phys = (addr & 0x1F_FFFF) as usize;
                let data = if phys + 3 < self.ram.len() {
                    self.ram[phys] as u32
                        | (self.ram[phys + 1] as u32) << 8
                        | (self.ram[phys + 2] as u32) << 16
                        | (self.ram[phys + 3] as u32) << 24
                } else {
                    0
                };
                match ch {
                    2 => self.gpu.gp0_write(data), // GPU write
                    4 => {
                        // SPU DMA write
                        self.spu.write_register(0x1A8, data as u16);
                        self.spu.write_register(0x1A8, (data >> 16) as u16);
                    }
                    _ => {}
                }
                addr = (addr as i32 + step) as u32;
            }
        }
    }

    fn dma_linked_list(&mut self, ch: usize, channel: &DmaChannel) {
        // Only GPU (channel 2) uses linked list mode
        if ch != 2 {
            return;
        }
        if channel.direction_to_ram() {
            return; // Linked list is RAM → GPU only
        }

        let mut addr = channel.base & 0x1F_FFFC;
        let mut safety = 0u32;

        loop {
            let phys = (addr & 0x1F_FFFF) as usize;
            let header = if phys + 3 < self.ram.len() {
                self.ram[phys] as u32
                    | (self.ram[phys + 1] as u32) << 8
                    | (self.ram[phys + 2] as u32) << 16
                    | (self.ram[phys + 3] as u32) << 24
            } else {
                0x00FF_FFFF // Terminate
            };

            let word_count = header >> 24;
            for i in 1..=word_count {
                let data_addr = ((addr + i * 4) & 0x1F_FFFF) as usize;
                let data = if data_addr + 3 < self.ram.len() {
                    self.ram[data_addr] as u32
                        | (self.ram[data_addr + 1] as u32) << 8
                        | (self.ram[data_addr + 2] as u32) << 16
                        | (self.ram[data_addr + 3] as u32) << 24
                } else {
                    0
                };
                self.gpu.gp0_write(data);
            }

            // Next node
            if header & 0x00FF_FFFF == 0x00FF_FFFF {
                break; // End marker
            }
            addr = header & 0x1F_FFFC;

            safety += 1;
            if safety > 0x10_0000 {
                break; // Safety limit
            }
        }
    }

    // ========================================================================
    // CD-ROM (simplified)
    // ========================================================================

    fn cdrom_execute_command(&mut self, cmd: u8) {
        self.cdrom_response.clear();
        match cmd {
            0x01 => {
                // GetStat
                self.cdrom_response.push(0x02); // Motor on
                self.cdrom_irq = 3; // INT3 (acknowledge)
            }
            0x19 => {
                // Test (sub-function in param)
                let sub = self.cdrom_params.first().copied().unwrap_or(0);
                match sub {
                    0x20 => {
                        // Get BIOS date/version
                        self.cdrom_response
                            .extend_from_slice(&[0x98, 0x06, 0x10, 0xC3]);
                        self.cdrom_irq = 3;
                    }
                    _ => {
                        self.cdrom_response.push(0);
                        self.cdrom_irq = 3;
                    }
                }
            }
            0x1A => {
                // GetID
                // No disc response (simplified)
                self.cdrom_response.extend_from_slice(&[0x11, 0x80]);
                self.cdrom_irq = 5; // INT5 (error)
            }
            0x0E => {
                // SetMode
                self.cdrom_response.push(0x02);
                self.cdrom_irq = 3;
            }
            _ => {
                // Unknown command — acknowledge
                self.cdrom_response.push(0x02);
                self.cdrom_irq = 3;
            }
        }
        self.cdrom_params.clear();
    }

    fn cdrom_read_word(&mut self) -> u32 {
        let mut val = 0u32;
        for i in 0..4 {
            let byte = if self.cdrom_data_pos < self.cdrom_data.len() {
                let b = self.cdrom_data[self.cdrom_data_pos];
                self.cdrom_data_pos += 1;
                b
            } else {
                0
            };
            val |= (byte as u32) << (i * 8);
        }
        val
    }
}

impl MemoryR3000A for Ps1Bus {
    fn read_byte(&self, addr: u32) -> u8 {
        match addr {
            // Main RAM (2 MB, mirrored)
            0x0000_0000..=0x001F_FFFF => self.ram[(addr & 0x1F_FFFF) as usize],

            // Scratchpad (1 KB)
            0x1F80_0000..=0x1F80_03FF => self.scratchpad[(addr & 0x3FF) as usize],

            // I/O ports
            0x1F80_1000..=0x1F80_2FFF => self.io_read_byte(addr),

            // BIOS ROM (512 KB)
            0x1FC0_0000..=0x1FC7_FFFF => self.bios[(addr & 0x7_FFFF) as usize],

            // Expansion Region 1 (return 0xFF for unmapped)
            0x1F00_0000..=0x1F7F_FFFF => 0xFF,

            // Cache control
            0x1FFE_0000..=0x1FFE_FFFF => 0,

            _ => 0,
        }
    }

    fn read_halfword(&self, addr: u32) -> u16 {
        match addr {
            0x0000_0000..=0x001F_FFFF => {
                let a = (addr & 0x1F_FFFF) as usize;
                self.ram[a] as u16 | (self.ram[a + 1] as u16) << 8
            }
            0x1F80_0000..=0x1F80_03FF => {
                let a = (addr & 0x3FF) as usize;
                self.scratchpad[a] as u16 | (self.scratchpad[a + 1] as u16) << 8
            }
            0x1F80_1000..=0x1F80_2FFF => self.io_read_halfword(addr),
            0x1FC0_0000..=0x1FC7_FFFF => {
                let a = (addr & 0x7_FFFF) as usize;
                self.bios[a] as u16 | (self.bios[a + 1] as u16) << 8
            }
            _ => 0,
        }
    }

    fn read_word(&self, addr: u32) -> u32 {
        match addr {
            0x0000_0000..=0x001F_FFFF => {
                let a = (addr & 0x1F_FFFF) as usize;
                self.ram[a] as u32
                    | (self.ram[a + 1] as u32) << 8
                    | (self.ram[a + 2] as u32) << 16
                    | (self.ram[a + 3] as u32) << 24
            }
            0x1F80_0000..=0x1F80_03FF => {
                let a = (addr & 0x3FF) as usize;
                self.scratchpad[a] as u32
                    | (self.scratchpad[a + 1] as u32) << 8
                    | (self.scratchpad[a + 2] as u32) << 16
                    | (self.scratchpad[a + 3] as u32) << 24
            }
            0x1F80_1000..=0x1F80_2FFF => self.io_read_word(addr),
            0x1FC0_0000..=0x1FC7_FFFF => {
                let a = (addr & 0x7_FFFF) as usize;
                self.bios[a] as u32
                    | (self.bios[a + 1] as u32) << 8
                    | (self.bios[a + 2] as u32) << 16
                    | (self.bios[a + 3] as u32) << 24
            }
            0x1FFE_0130 => self.cache_ctrl,
            _ => 0,
        }
    }

    fn write_byte(&mut self, addr: u32, val: u8) {
        match addr {
            0x0000_0000..=0x001F_FFFF => self.ram[(addr & 0x1F_FFFF) as usize] = val,
            0x1F80_0000..=0x1F80_03FF => self.scratchpad[(addr & 0x3FF) as usize] = val,
            0x1F80_1000..=0x1F80_2FFF => self.io_write_byte(addr, val),
            _ => {}
        }
    }

    fn write_halfword(&mut self, addr: u32, val: u16) {
        match addr {
            0x0000_0000..=0x001F_FFFF => {
                let a = (addr & 0x1F_FFFF) as usize;
                self.ram[a] = val as u8;
                self.ram[a + 1] = (val >> 8) as u8;
            }
            0x1F80_0000..=0x1F80_03FF => {
                let a = (addr & 0x3FF) as usize;
                self.scratchpad[a] = val as u8;
                self.scratchpad[a + 1] = (val >> 8) as u8;
            }
            0x1F80_1000..=0x1F80_2FFF => self.io_write_halfword(addr, val),
            _ => {}
        }
    }

    fn write_word(&mut self, addr: u32, val: u32) {
        match addr {
            0x0000_0000..=0x001F_FFFF => {
                let a = (addr & 0x1F_FFFF) as usize;
                self.ram[a] = val as u8;
                self.ram[a + 1] = (val >> 8) as u8;
                self.ram[a + 2] = (val >> 16) as u8;
                self.ram[a + 3] = (val >> 24) as u8;
            }
            0x1F80_0000..=0x1F80_03FF => {
                let a = (addr & 0x3FF) as usize;
                self.scratchpad[a] = val as u8;
                self.scratchpad[a + 1] = (val >> 8) as u8;
                self.scratchpad[a + 2] = (val >> 16) as u8;
                self.scratchpad[a + 3] = (val >> 24) as u8;
            }
            0x1F80_1000..=0x1F80_2FFF => self.io_write_word(addr, val),
            0x1FFE_0130 => self.cache_ctrl = val,
            _ => {}
        }
    }

    fn irq_pending(&self) -> bool {
        self.irq.pending()
    }
}

// ============================================================================
// PS1 System
// ============================================================================

/// PlayStation 1 emulator system.
pub struct Ps1System {
    cpu: CpuR3000A<Ps1Bus>,
    total_cycles: u64,
}

impl Default for Ps1System {
    fn default() -> Self {
        Self::new()
    }
}

impl Ps1System {
    pub fn new() -> Self {
        let bus = Ps1Bus::new();
        Self {
            cpu: CpuR3000A::new(bus),
            total_cycles: 0,
        }
    }

    fn bus(&self) -> &Ps1Bus {
        &self.cpu.memory
    }

    fn bus_mut(&mut self) -> &mut Ps1Bus {
        &mut self.cpu.memory
    }
}

impl System for Ps1System {
    type Error = Ps1Error;

    fn reset(&mut self) {
        self.cpu.reset();
        self.cpu.memory.reset();
        self.total_cycles = 0;
    }

    fn step_frame(&mut self) -> Result<Frame, Ps1Error> {
        if !self.cpu.memory.bios_loaded {
            return Err(Ps1Error::NoBios);
        }

        let total_scanlines = SCANLINES_NTSC;

        for _scanline in 0..total_scanlines {
            // Run CPU for one scanline worth of cycles
            let mut cycles_this_line = 0u32;
            while cycles_this_line < CYCLES_PER_SCANLINE {
                let c = self.cpu.step();
                cycles_this_line += c;
                self.total_cycles += c as u64;
            }

            // Step timers
            let timer0_irq = self.cpu.memory.timers[0].step(cycles_this_line);
            let timer1_irq = self.cpu.memory.timers[1].step(cycles_this_line);
            let timer2_irq = self.cpu.memory.timers[2].step(cycles_this_line);

            if timer0_irq {
                self.cpu.memory.irq.raise(IRQ_TIMER0);
            }
            if timer1_irq {
                self.cpu.memory.irq.raise(IRQ_TIMER0 << 1);
            }
            if timer2_irq {
                self.cpu.memory.irq.raise(IRQ_TIMER0 << 2);
            }

            // Step GPU scanline
            self.cpu.memory.gpu.step_scanline();

            // VBlank interrupt
            if self.cpu.memory.gpu.in_vblank && _scanline == 240 {
                self.cpu.memory.irq.raise(IRQ_VBLANK);
            }
        }

        // Render frame from VRAM
        self.cpu.memory.gpu.render_frame();
        let frame = self.cpu.memory.gpu.get_frame().clone();
        Ok(frame)
    }

    fn save_state(&self) -> Value {
        // TODO: Implement full save state
        serde_json::json!({})
    }

    fn load_state(&mut self, _state: &Value) -> Result<(), serde_json::Error> {
        // TODO: Implement full state restore
        Ok(())
    }

    fn supports_save_states(&self) -> bool {
        false
    }

    fn mount_points(&self) -> Vec<MountPointInfo> {
        vec![
            MountPointInfo {
                id: "bios".to_string(),
                name: "BIOS".to_string(),
                extensions: vec!["bin".to_string(), "rom".to_string()],
                required: true,
            },
            MountPointInfo {
                id: "disc".to_string(),
                name: "Disc Image".to_string(),
                extensions: vec![
                    "bin".to_string(),
                    "iso".to_string(),
                    "exe".to_string(),
                    "psexe".to_string(),
                ],
                required: false,
            },
        ]
    }

    fn mount(&mut self, mount_point_id: &str, data: &[u8]) -> Result<(), Ps1Error> {
        match mount_point_id {
            "bios" => self.bus_mut().load_bios(data),
            "disc" => {
                if data.len() >= 0x800 && &data[0..8] == b"PS-X EXE" {
                    self.bus_mut().load_exe(data)
                } else {
                    // Assume raw disc image
                    self.bus_mut().disc_data = data.to_vec();
                    Ok(())
                }
            }
            _ => Err(Ps1Error::Io(format!(
                "Unknown mount point: {}",
                mount_point_id
            ))),
        }
    }

    fn unmount(&mut self, mount_point_id: &str) -> Result<(), Ps1Error> {
        match mount_point_id {
            "bios" => {
                self.bus_mut().bios.fill(0);
                self.bus_mut().bios_loaded = false;
                Ok(())
            }
            "disc" => {
                self.bus_mut().disc_data.clear();
                Ok(())
            }
            _ => Err(Ps1Error::Io(format!(
                "Unknown mount point: {}",
                mount_point_id
            ))),
        }
    }

    fn is_mounted(&self, mount_point_id: &str) -> bool {
        match mount_point_id {
            "bios" => self.bus().bios_loaded,
            "disc" => !self.bus().disc_data.is_empty(),
            _ => false,
        }
    }

    fn get_total_cycles(&self) -> u64 {
        self.total_cycles
    }
}

impl emu_core::renderer::Renderer for Ps1System {
    fn get_frame(&self) -> &Frame {
        self.bus().gpu.get_frame()
    }

    fn clear(&mut self, color: u32) {
        self.bus_mut().gpu.clear(color);
    }

    fn reset(&mut self) {
        System::reset(self);
    }

    fn name(&self) -> &str {
        "PS1"
    }

    fn resize(&mut self, _width: u32, _height: u32) {}
}
