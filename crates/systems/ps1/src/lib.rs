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

pub mod debugger;
pub mod gpu;
pub mod spu;

use emu_core::cpu_mips_r3000a::{CpuR3000A, MemoryR3000A};
use emu_core::debug::Debugger;
use emu_core::logging::{log, LogCategory, LogLevel};
use emu_core::renderer::Renderer;
use emu_core::types::Frame;
use emu_core::{MountPointInfo, System};
use serde_json::Value;
use std::cell::Cell;
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
// CD-ROM Controller
// ============================================================================

/// Convert BCD byte to binary
fn bcd_to_bin(bcd: u8) -> u8 {
    (bcd >> 4) * 10 + (bcd & 0x0F)
}

/// Convert MSF (minute:second:frame in BCD) to LBA (logical block address)
fn msf_to_lba(mm: u8, ss: u8, ff: u8) -> u32 {
    let m = bcd_to_bin(mm) as u32;
    let s = bcd_to_bin(ss) as u32;
    let f = bcd_to_bin(ff) as u32;
    // Standard CD: LBA = (m*60+s)*75 + f - 150
    // The 150 offset accounts for the 2-second pregap
    let raw = (m * 60 + s) * 75 + f;
    raw.saturating_sub(150)
}

/// PS1 CD-ROM controller with FIFO management, two-stage responses, and sector reading.
///
/// The PS1 CD-ROM uses an asynchronous command/response model:
/// - Commands generate INT3 (acknowledge) immediately
/// - Many commands then generate a delayed INT2 (complete) or INT1 (data ready)
/// - The BIOS polls the status register and IRQ flags to detect responses
/// - Response and data FIFOs use `Cell<usize>` for interior mutability during `&self` reads
pub struct CdRom {
    /// Register bank index (0-3), written to 0x1F801800
    pub index: u8,
    /// Parameter FIFO (up to 16 bytes)
    pub params: Vec<u8>,
    /// Response FIFO
    response: Vec<u8>,
    /// Response read position (Cell for interior mutability during &self reads)
    response_pos: Cell<usize>,
    /// Data FIFO (sector data, up to 2048 bytes)
    pub data_fifo: Vec<u8>,
    /// Data FIFO read position
    pub data_fifo_pos: Cell<usize>,
    /// IRQ enable register (5 bits)
    pub irq_enable: u8,
    /// IRQ flag register (5 bits)
    pub irq_flag: u8,
    /// Seek target from Setloc (BCD: MM, SS, FF)
    seek_target: [u8; 3],
    /// Whether we're actively reading sectors
    pub read_active: bool,
    /// Current read position (LBA)
    pub read_lba: u32,
    /// Pending 2nd response data (for two-stage commands like GetID, Init)
    pending_response: Vec<u8>,
    /// Pending 2nd response IRQ type
    pub pending_irq: u8,
    /// Drive mode byte (set by SetMode command 0x0E)
    mode_byte: u8,
    /// Sector buffer (last read sector's user data)
    pub sector_buffer: Vec<u8>,
    /// Delay counter for pending response delivery (in scanlines)
    pub delivery_delay: u32,
    /// Whether the motor is spinning
    motor_on: bool,
    /// Disc image data
    pub disc_data: Vec<u8>,
    /// Detected sector size in disc image (2048 for ISO, 2352 for BIN)
    pub disc_sector_size: usize,
}

impl Default for CdRom {
    fn default() -> Self {
        Self::new()
    }
}

impl CdRom {
    pub fn new() -> Self {
        Self {
            index: 0,
            params: Vec::with_capacity(16),
            response: Vec::new(),
            response_pos: Cell::new(0),
            data_fifo: Vec::new(),
            data_fifo_pos: Cell::new(0),
            irq_enable: 0,
            irq_flag: 0,
            seek_target: [0; 3],
            read_active: false,
            read_lba: 0,
            pending_response: Vec::new(),
            pending_irq: 0,
            mode_byte: 0,
            sector_buffer: Vec::new(),
            delivery_delay: 0,
            motor_on: false,
            disc_data: Vec::new(),
            disc_sector_size: 2352,
        }
    }

    pub fn reset(&mut self) {
        let disc_data = std::mem::take(&mut self.disc_data);
        let disc_sector_size = self.disc_sector_size;
        *self = Self::new();
        self.disc_data = disc_data;
        self.disc_sector_size = disc_sector_size;
    }

    /// Compute the status register byte (0x1F801800 read)
    pub fn read_status(&self) -> u8 {
        let index = self.index & 3;
        let prmempt = if self.params.is_empty() { 1 << 3 } else { 0 };
        let prmwrdy = if self.params.len() < 16 { 1 << 4 } else { 0 };
        let rslrrdy = if self.response_pos.get() < self.response.len() {
            1 << 5
        } else {
            0
        };
        let drqsts = if self.data_fifo_pos.get() < self.data_fifo.len() {
            1 << 6
        } else {
            0
        };
        index | prmempt | prmwrdy | rslrrdy | drqsts
    }

    /// Read and pop a byte from the response FIFO (0x1F801801 read)
    pub fn read_response(&self) -> u8 {
        let pos = self.response_pos.get();
        if pos < self.response.len() {
            self.response_pos.set(pos + 1);
            self.response[pos]
        } else {
            0
        }
    }

    /// Read and pop a byte from the data FIFO (0x1F801802 read)
    pub fn read_data(&self) -> u8 {
        let pos = self.data_fifo_pos.get();
        if pos < self.data_fifo.len() {
            self.data_fifo_pos.set(pos + 1);
            self.data_fifo[pos]
        } else {
            0
        }
    }

    /// Read 4 bytes from the data FIFO as a little-endian word (for DMA channel 3)
    pub fn read_data_word(&self) -> u32 {
        let mut val = 0u32;
        for i in 0..4 {
            val |= (self.read_data() as u32) << (i * 8);
        }
        val
    }

    /// Get the drive status byte used in command responses
    fn stat_byte(&self) -> u8 {
        let mut stat = 0u8;
        if self.motor_on {
            stat |= 0x02;
        }
        if self.read_active {
            stat |= 0x20;
        }
        stat
    }

    /// Set the first response (clears previous, sets IRQ flag)
    fn set_response(&mut self, data: &[u8], irq: u8) {
        self.response.clear();
        self.response_pos.set(0);
        self.response.extend_from_slice(data);
        self.irq_flag = irq;
    }

    /// Queue a second response for delivery after IRQ acknowledge
    fn queue_pending(&mut self, data: &[u8], irq: u8) {
        self.pending_response.clear();
        self.pending_response.extend_from_slice(data);
        self.pending_irq = irq;
        self.delivery_delay = 50; // ~50 scanlines delay
    }

    /// Deliver pending 2nd response
    pub fn deliver_pending(&mut self) {
        if self.pending_irq != 0 {
            self.response.clear();
            self.response_pos.set(0);
            let pending = std::mem::take(&mut self.pending_response);
            self.response.extend_from_slice(&pending);
            self.irq_flag = self.pending_irq;
            self.pending_irq = 0;
            self.delivery_delay = 0;
        }
    }

    /// Load sector buffer into data FIFO (triggered by Request Register bit 7)
    pub fn load_data_fifo(&mut self) {
        if !self.sector_buffer.is_empty() {
            self.data_fifo.clear();
            self.data_fifo_pos.set(0);
            self.data_fifo.extend_from_slice(&self.sector_buffer);
        }
    }

    /// Read a sector's user data from the disc image (pure function, no &self)
    fn read_sector_data(lba: u32, disc_data: &[u8], sector_size: usize) -> Vec<u8> {
        let offset = lba as usize * sector_size;
        if offset >= disc_data.len() {
            return vec![0; 2048];
        }

        if sector_size == 2352 {
            // Raw sector: extract user data based on mode byte
            let mode = if offset + 15 < disc_data.len() {
                disc_data[offset + 15]
            } else {
                1
            };
            let data_offset = match mode {
                2 => 24, // Mode 2 Form 1: sync(12)+header(4)+subheader(8)+data
                _ => 16, // Mode 1: sync(12)+header(4)+data
            };
            let start = offset + data_offset;
            let end = (start + 2048).min(disc_data.len());
            if start < disc_data.len() {
                disc_data[start..end].to_vec()
            } else {
                vec![0; 2048]
            }
        } else {
            // ISO format: pure user data
            let end = (offset + 2048).min(disc_data.len());
            disc_data[offset..end].to_vec()
        }
    }

    /// Execute a CD-ROM command
    pub fn execute_command(&mut self, cmd: u8) {
        let stat = self.stat_byte();

        match cmd {
            0x01 => {
                // GetStat — returns drive status byte
                self.motor_on = true;
                let s = self.stat_byte();
                self.set_response(&[s], 3);
            }
            0x02 => {
                // Setloc(mm, ss, ff) — set seek target
                if self.params.len() >= 3 {
                    self.seek_target = [self.params[0], self.params[1], self.params[2]];
                }
                self.set_response(&[stat], 3);
            }
            0x03 => {
                // Play — start audio playback (stub)
                self.set_response(&[stat], 3);
            }
            0x06 => {
                // ReadN — read data sectors with retry
                self.motor_on = true;
                let lba = msf_to_lba(
                    self.seek_target[0],
                    self.seek_target[1],
                    self.seek_target[2],
                );
                self.read_lba = lba;
                self.read_active = true;
                let s = self.stat_byte();
                self.set_response(&[s], 3); // INT3: acknowledge
                                            // Load first sector and set delivery delay for INT1
                self.sector_buffer =
                    Self::read_sector_data(lba, &self.disc_data, self.disc_sector_size);
                self.delivery_delay = 100;
            }
            0x07 => {
                // MotorOn
                self.motor_on = true;
                let s = self.stat_byte();
                self.set_response(&[s], 3);
                let s2 = self.stat_byte();
                self.queue_pending(&[s2], 2);
            }
            0x08 => {
                // Stop
                self.motor_on = false;
                self.read_active = false;
                self.set_response(&[stat], 3);
                let s = self.stat_byte();
                self.queue_pending(&[s], 2);
            }
            0x09 => {
                // Pause
                self.read_active = false;
                self.set_response(&[stat], 3);
                let s = self.stat_byte();
                self.queue_pending(&[s], 2);
            }
            0x0A => {
                // Init — initialize controller
                self.motor_on = true;
                self.mode_byte = 0x20; // Default mode
                let s = self.stat_byte();
                self.set_response(&[s], 3);
                let s2 = self.stat_byte();
                self.queue_pending(&[s2], 2);
            }
            0x0B => {
                // Mute
                self.set_response(&[stat], 3);
            }
            0x0C => {
                // Demute
                self.set_response(&[stat], 3);
            }
            0x0D => {
                // SetFilter (file, channel from params)
                self.set_response(&[stat], 3);
            }
            0x0E => {
                // SetMode — set drive mode
                if !self.params.is_empty() {
                    self.mode_byte = self.params[0];
                }
                self.set_response(&[stat], 3);
            }
            0x0F => {
                // Getparam — returns mode, file, channel, etc.
                self.set_response(&[stat, self.mode_byte, 0x00, 0x00, 0x00], 3);
            }
            0x10 => {
                // GetlocL — get logical position (data sector header)
                self.set_response(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00], 3);
            }
            0x11 => {
                // GetlocP — get physical position (subQ data)
                self.set_response(&[0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00], 3);
            }
            0x13 => {
                // GetTN — get first/last track numbers
                self.set_response(&[stat, 0x01, 0x01], 3);
            }
            0x14 => {
                // GetTD — get track start position
                self.set_response(&[stat, 0x00, 0x02], 3);
            }
            0x15 => {
                // SeekL — seek to data sector
                self.motor_on = true;
                let s = self.stat_byte();
                self.set_response(&[s], 3);
                let s2 = self.stat_byte();
                self.queue_pending(&[s2], 2);
            }
            0x16 => {
                // SeekP — seek to audio position
                self.motor_on = true;
                let s = self.stat_byte();
                self.set_response(&[s], 3);
                let s2 = self.stat_byte();
                self.queue_pending(&[s2], 2);
            }
            0x19 => {
                // Test — sub-function in param
                let sub = self.params.first().copied().unwrap_or(0);
                match sub {
                    0x20 => {
                        // Get BIOS date/version: 97/01/10 Rev C3
                        self.set_response(&[0x98, 0x06, 0x10, 0xC3], 3);
                    }
                    _ => {
                        self.set_response(&[0], 3);
                    }
                }
            }
            0x1A => {
                // GetID — identify disc
                if self.disc_data.is_empty() {
                    // No disc
                    self.set_response(&[0x11, 0x80], 5); // INT5: error
                } else {
                    // Game disc present: two-stage response
                    self.motor_on = true;
                    let s = self.stat_byte();
                    self.set_response(&[s], 3); // INT3: acknowledge
                                                // Queue INT2: stat, flags=0(licensed), type=0x20(mode2), atip=0, "SCEA"
                    self.queue_pending(&[0x02, 0x00, 0x20, 0x00, b'S', b'C', b'E', b'A'], 2);
                }
            }
            0x1B => {
                // ReadS — read data sectors without retry (same as ReadN)
                self.motor_on = true;
                let lba = msf_to_lba(
                    self.seek_target[0],
                    self.seek_target[1],
                    self.seek_target[2],
                );
                self.read_lba = lba;
                self.read_active = true;
                let s = self.stat_byte();
                self.set_response(&[s], 3);
                self.sector_buffer =
                    Self::read_sector_data(lba, &self.disc_data, self.disc_sector_size);
                self.delivery_delay = 100;
            }
            0x1E => {
                // ReadTOC
                self.set_response(&[stat], 3);
                let s = self.stat_byte();
                self.queue_pending(&[s], 2);
            }
            _ => {
                // Unknown command — just acknowledge
                self.set_response(&[stat], 3);
            }
        }
        self.params.clear();
    }

    /// Step the CD-ROM controller (called per scanline).
    /// Returns true if a CD-ROM IRQ should be raised.
    pub fn step(&mut self) -> bool {
        if self.delivery_delay > 0 {
            self.delivery_delay -= 1;
            if self.delivery_delay == 0 {
                // Sector data delivery (read active, no other pending response)
                if self.read_active && self.pending_irq == 0 {
                    if self.irq_flag == 0 {
                        self.response.clear();
                        self.response_pos.set(0);
                        self.response.push(self.stat_byte());
                        self.irq_flag = 1; // INT1: data ready
                        return true;
                    } else {
                        // Previous IRQ not yet acknowledged, retry next scanline
                        self.delivery_delay = 1;
                    }
                }
                // Pending second response delivery
                if self.pending_irq != 0 {
                    if self.irq_flag == 0 {
                        self.deliver_pending();
                        return true;
                    } else {
                        // Previous IRQ not yet acknowledged, retry next scanline
                        self.delivery_delay = 1;
                    }
                }
            }
        }

        false
    }

    /// Advance to next sector after INT1 acknowledge (called from io_write_byte)
    pub fn advance_read(&mut self) {
        if self.read_active {
            self.read_lba += 1;
            self.sector_buffer =
                Self::read_sector_data(self.read_lba, &self.disc_data, self.disc_sector_size);
            self.delivery_delay = 100;
        }
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
    /// CD-ROM controller
    cdrom: CdRom,
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
            cdrom: CdRom::new(),
            joy_data: 0xFFFF,
            joy_stat: 0x05, // TX Ready 1 + TX Ready 2
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
        self.cdrom.reset();
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
            0x1F80_1800 => self.cdrom.read_status(),
            0x1F80_1801 => self.cdrom.read_response(),
            0x1F80_1802 => self.cdrom.read_data(),
            0x1F80_1803 => {
                let index = self.cdrom.index;
                match index & 1 {
                    0 => self.cdrom.irq_enable | 0xE0,
                    1 => self.cdrom.irq_flag | 0xE0,
                    _ => unreachable!(),
                }
            }
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
            0x1F80_1044 => (self.joy_stat | 0x05) as u16, // Always report TX Ready
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
            0x1F80_1044 => self.joy_stat | 0x05, // Always report TX Ready
            0x1F80_104E => 0,                    // JOY_BAUD

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
                // Index register (bits 0-1)
                self.cdrom.index = val & 3;
            }
            0x1F80_1801 => {
                let index = self.cdrom.index;
                match index {
                    0 => {
                        // Command register
                        self.cdrom.execute_command(val);
                        // Raise CD-ROM IRQ immediately if flag matches enable
                        if (self.cdrom.irq_flag & self.cdrom.irq_enable) != 0 {
                            self.irq.raise(IRQ_CDROM);
                        }
                    }
                    1 => {
                        // Sound Map Data Out (stub)
                    }
                    2 => {
                        // Sound Map Coding Info (stub)
                    }
                    3 => {
                        // Audio Volume Apply (stub)
                    }
                    _ => {}
                }
            }
            0x1F80_1802 => {
                let index = self.cdrom.index;
                match index {
                    0 => self.cdrom.params.push(val), // Parameter FIFO
                    1 => self.cdrom.irq_enable = val & 0x1F,
                    2 => {
                        // Audio Volume Left->Left (stub)
                    }
                    3 => {
                        // Audio Volume Right->Left (stub)
                    }
                    _ => {}
                }
            }
            0x1F80_1803 => {
                let index = self.cdrom.index;
                match index {
                    0 => {
                        // Request register
                        if val & 0x80 != 0 {
                            // Want data — load sector into data FIFO if available
                            self.cdrom.load_data_fifo();
                        } else {
                            // Clear data FIFO
                            self.cdrom.data_fifo.clear();
                            self.cdrom.data_fifo_pos.set(0);
                        }
                    }
                    1 => {
                        // Interrupt flag acknowledge
                        let was_data_irq = self.cdrom.irq_flag == 1 && self.cdrom.read_active;
                        self.cdrom.irq_flag &= !(val & 0x1F);
                        if val & 0x40 != 0 {
                            self.cdrom.params.clear();
                        }
                        // If IRQ now clear and we have a pending 2nd response, deliver it
                        if self.cdrom.irq_flag == 0 && self.cdrom.pending_irq != 0 {
                            self.cdrom.deliver_pending();
                            // Raise CD-ROM IRQ for newly delivered response
                            if (self.cdrom.irq_flag & self.cdrom.irq_enable) != 0 {
                                self.irq.raise(IRQ_CDROM);
                            }
                        }
                        // If we just acknowledged a sector data INT1, advance to next sector
                        if was_data_irq && self.cdrom.irq_flag == 0 {
                            self.cdrom.advance_read();
                        }
                    }
                    2 => {
                        // Audio Volume Left->Right (stub)
                    }
                    3 => {
                        // Audio Volume Apply Changes (stub)
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

        // OTC (channel 6): Ordering Table Clear — writes reverse linked-list into RAM
        if ch == 6 {
            for i in 0..word_count {
                let phys = (addr & 0x1F_FFFF) as usize;
                let data = if i == word_count - 1 {
                    0x00FF_FFFFu32 // End-of-list marker
                } else {
                    (addr.wrapping_sub(4)) & 0x1F_FFFF // Pointer to previous entry
                };
                if phys + 3 < self.ram.len() {
                    self.ram[phys] = data as u8;
                    self.ram[phys + 1] = (data >> 8) as u8;
                    self.ram[phys + 2] = (data >> 16) as u8;
                    self.ram[phys + 3] = (data >> 24) as u8;
                }
                addr = addr.wrapping_sub(4); // OTC always steps backward
            }
            return;
        }

        if channel.direction_to_ram() {
            // Device → RAM
            for _ in 0..word_count {
                let data = match ch {
                    2 => self.gpu.gpuread(),          // GPU read
                    3 => self.cdrom.read_data_word(), // CD-ROM
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
}

impl MemoryR3000A for Ps1Bus {
    fn read_byte(&self, addr: u32) -> u8 {
        match addr {
            // Main RAM (2 MB, mirrored 4× across first 8 MB)
            0x0000_0000..=0x007F_FFFF => self.ram[(addr & 0x1F_FFFF) as usize],

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
            0x0000_0000..=0x007F_FFFF => {
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
            0x1F00_0000..=0x1F7F_FFFF => 0xFFFF,
            _ => 0,
        }
    }

    fn read_word(&self, addr: u32) -> u32 {
        match addr {
            0x0000_0000..=0x007F_FFFF => {
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
            0x1F00_0000..=0x1F7F_FFFF => 0xFFFF_FFFF,
            _ => 0,
        }
    }

    fn write_byte(&mut self, addr: u32, val: u8) {
        match addr {
            0x0000_0000..=0x007F_FFFF => self.ram[(addr & 0x1F_FFFF) as usize] = val,
            0x1F80_0000..=0x1F80_03FF => self.scratchpad[(addr & 0x3FF) as usize] = val,
            0x1F80_1000..=0x1F80_2FFF => self.io_write_byte(addr, val),
            _ => {}
        }
    }

    fn write_halfword(&mut self, addr: u32, val: u16) {
        match addr {
            0x0000_0000..=0x007F_FFFF => {
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
            0x0000_0000..=0x007F_FFFF => {
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
    /// Frame counter (incremented each step_frame call)
    frame_index: u64,
    /// Instruction tracer for debugging
    instruction_tracer: emu_core::instruction_tracer::InstructionTracer,
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
            frame_index: 0,
            instruction_tracer: emu_core::instruction_tracer::InstructionTracer::new(),
        }
    }

    emu_core::impl_instruction_tracer_methods!();

    /// Get GPU inspector data for the debug UI.
    pub fn get_gpu_inspector_data(&self) -> gpu::Ps1GpuInspectorData {
        self.bus().gpu.get_inspector_data()
    }

    /// Get audio samples (stereo interleaved, i16) generated by the SPU.
    /// Returns `count` stereo pairs (2×count i16 values).
    pub fn get_audio_samples(&mut self, count: usize) -> Vec<i16> {
        self.bus_mut().spu.get_audio_samples(count)
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
        let mut cpu_steps: u32 = 0;
        let mut timer_irqs: u32 = 0;

        for _scanline in 0..total_scanlines {
            // Run CPU for one scanline worth of cycles
            let mut cycles_this_line = 0u32;
            while cycles_this_line < CYCLES_PER_SCANLINE {
                // Capture PC before step for instruction tracing
                let pc_before = self.cpu.pc;
                let c = self.cpu.step();
                cycles_this_line += c;
                self.total_cycles += c as u64;
                cpu_steps += 1;

                // Record instruction if tracing is enabled
                if self.instruction_tracer.is_enabled() {
                    if let Some(instr) = self.disassemble_instruction(pc_before) {
                        let cpu_state = self.get_cpu_state();
                        self.instruction_tracer.trace(instr, cpu_state);
                    }
                }
            }

            // Step timers
            let timer0_irq = self.cpu.memory.timers[0].step(cycles_this_line);
            let timer1_irq = self.cpu.memory.timers[1].step(cycles_this_line);
            let timer2_irq = self.cpu.memory.timers[2].step(cycles_this_line);

            if timer0_irq {
                self.cpu.memory.irq.raise(IRQ_TIMER0);
                timer_irqs += 1;
                log(LogCategory::Interrupts, LogLevel::Debug, || {
                    "PS1: Timer0 IRQ".to_string()
                });
            }
            if timer1_irq {
                self.cpu.memory.irq.raise(IRQ_TIMER1);
                timer_irqs += 1;
                log(LogCategory::Interrupts, LogLevel::Debug, || {
                    "PS1: Timer1 IRQ".to_string()
                });
            }
            if timer2_irq {
                self.cpu.memory.irq.raise(IRQ_TIMER2);
                timer_irqs += 1;
                log(LogCategory::Interrupts, LogLevel::Debug, || {
                    "PS1: Timer2 IRQ".to_string()
                });
            }

            // Step GPU scanline
            self.cpu.memory.gpu.step_scanline();

            // Step CD-ROM controller
            self.cpu.memory.cdrom.step();
            // Level-triggered: always assert IRQ while CD-ROM has active interrupt
            if (self.cpu.memory.cdrom.irq_flag & self.cpu.memory.cdrom.irq_enable) != 0 {
                self.cpu.memory.irq.raise(IRQ_CDROM);
            }

            // VBlank interrupt
            if self.cpu.memory.gpu.in_vblank && _scanline == 240 {
                self.cpu.memory.irq.raise(IRQ_VBLANK);
                log(LogCategory::Interrupts, LogLevel::Debug, || {
                    format!("PS1: VBlank IRQ (frame={})", self.frame_index)
                });
            }
        }

        // Generate SPU audio samples for this frame
        // At 44100 Hz and ~60 fps (NTSC), each frame needs ~735 stereo samples
        const SPU_SAMPLES_PER_FRAME: usize = 735;
        self.cpu.memory.spu.tick_samples(SPU_SAMPLES_PER_FRAME);

        // Log frame statistics at trace level (every 60 frames)
        if self.frame_index.is_multiple_of(60) {
            log(LogCategory::CPU, LogLevel::Trace, || {
                format!(
                    "PS1 TRACE: frame={} pc=0x{:08X} steps={} cycles={} timer_irqs={}",
                    self.frame_index, self.cpu.pc, cpu_steps, self.total_cycles, timer_irqs,
                )
            });
        }

        self.frame_index += 1;

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
                    // Assume raw disc image — auto-detect sector size
                    let sector_size = if data.len().is_multiple_of(2352) {
                        2352
                    } else {
                        2048
                    };
                    self.bus_mut().cdrom.disc_data = data.to_vec();
                    self.bus_mut().cdrom.disc_sector_size = sector_size;
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
                self.bus_mut().cdrom.disc_data.clear();
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
            "disc" => !self.bus().cdrom.disc_data.is_empty(),
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: encode a MIPS I-type instruction
    fn encode_i(opcode: u32, rs: u32, rt: u32, imm: u16) -> u32 {
        (opcode << 26) | (rs << 21) | (rt << 16) | (imm as u32)
    }

    /// Helper: encode a MIPS R-type instruction
    #[allow(dead_code)]
    fn encode_r(opcode: u32, rs: u32, rt: u32, rd: u32, _sa: u32, funct: u32) -> u32 {
        (opcode << 26) | (rs << 21) | (rt << 16) | (rd << 11) | (_sa << 6) | funct
    }

    /// Helper: encode a MIPS J-type instruction
    fn encode_j(opcode: u32, target: u32) -> u32 {
        (opcode << 26) | (target & 0x03FF_FFFF)
    }

    /// Write a word in LE to a byte slice at an offset
    fn write_le32(buf: &mut [u8], offset: usize, val: u32) {
        buf[offset] = val as u8;
        buf[offset + 1] = (val >> 8) as u8;
        buf[offset + 2] = (val >> 16) as u8;
        buf[offset + 3] = (val >> 24) as u8;
    }

    #[test]
    fn test_ps1_bios_boot_nop_loop() {
        let mut bios = vec![0u8; 512 * 1024];
        write_le32(&mut bios, 0, encode_j(2, 0xBFC00000 >> 2));

        let mut sys = Ps1System::new();
        sys.bus_mut().load_bios(&bios).unwrap();

        let result = sys.step_frame();
        assert!(result.is_ok(), "step_frame should succeed with BIOS loaded");

        let frame = result.unwrap();
        assert_eq!(frame.width, 320);
        assert_eq!(frame.height, 240);
    }

    #[test]
    fn test_ps1_bios_io_access() {
        let mut bios = vec![0u8; 512 * 1024];
        let mut off = 0usize;
        // LUI $t0, 0xBF80; ORI $t0, 0x1814; LUI $t1, 0; SW $t1, 0($t0) → GP1 reset
        write_le32(&mut bios, off, encode_i(0x0F, 0, 8, 0xBF80));
        off += 4;
        write_le32(&mut bios, off, encode_i(0x0D, 8, 8, 0x1814));
        off += 4;
        write_le32(&mut bios, off, encode_i(0x0F, 0, 9, 0x0000));
        off += 4;
        write_le32(&mut bios, off, encode_i(0x2B, 8, 9, 0x0000));
        off += 4;
        // Read IRQ status
        write_le32(&mut bios, off, encode_i(0x0F, 0, 8, 0xBF80));
        off += 4;
        write_le32(&mut bios, off, encode_i(0x0D, 8, 8, 0x1070));
        off += 4;
        write_le32(&mut bios, off, encode_i(0x23, 8, 9, 0x0000));
        off += 4;
        // Write DMA control
        write_le32(&mut bios, off, encode_i(0x0F, 0, 8, 0xBF80));
        off += 4;
        write_le32(&mut bios, off, encode_i(0x0D, 8, 8, 0x10F0));
        off += 4;
        write_le32(&mut bios, off, encode_i(0x2B, 8, 0, 0x0000));
        off += 4;
        // Write RAM size register
        write_le32(&mut bios, off, encode_i(0x0F, 0, 8, 0xBF80));
        off += 4;
        write_le32(&mut bios, off, encode_i(0x0D, 8, 8, 0x1060));
        off += 4;
        write_le32(&mut bios, off, encode_i(0x0F, 0, 9, 0x0B88));
        off += 4;
        write_le32(&mut bios, off, encode_i(0x2B, 8, 9, 0x0000));
        off += 4;
        // Write to RAM via KSEG0
        write_le32(&mut bios, off, encode_i(0x0F, 0, 8, 0x8000));
        off += 4;
        write_le32(&mut bios, off, encode_i(0x2B, 8, 0, 0x0000));
        off += 4;
        // Loop
        write_le32(&mut bios, off, encode_j(2, (0xBFC00000 + off as u32) >> 2));
        off += 4;
        write_le32(&mut bios, off, 0);

        let mut sys = Ps1System::new();
        sys.bus_mut().load_bios(&bios).unwrap();
        let result = sys.step_frame();
        assert!(
            result.is_ok(),
            "step_frame with I/O access should not panic"
        );
    }

    #[test]
    fn test_ps1_expansion_and_unmapped_access() {
        let mut bios = vec![0u8; 512 * 1024];
        write_le32(&mut bios, 0, encode_i(0x0F, 0, 8, 0xBF00));
        write_le32(&mut bios, 4, encode_i(0x20, 8, 9, 0x0000));
        write_le32(&mut bios, 8, encode_i(0x0F, 0, 8, 0xBF80));
        write_le32(&mut bios, 0xC, encode_i(0x0D, 8, 8, 0x2000));
        write_le32(&mut bios, 0x10, encode_i(0x20, 8, 9, 0x0000));
        write_le32(&mut bios, 0x14, encode_j(2, 0xBFC00014 >> 2));
        write_le32(&mut bios, 0x18, 0);

        let mut sys = Ps1System::new();
        sys.bus_mut().load_bios(&bios).unwrap();
        let result = sys.step_frame();
        assert!(result.is_ok(), "unmapped access should not panic");
    }

    #[test]
    fn test_ps1_cdrom_probe() {
        let mut bios = vec![0u8; 512 * 1024];
        let mut off = 0usize;
        // Write to CD-ROM
        write_le32(&mut bios, off, encode_i(0x0F, 0, 8, 0xBF80));
        off += 4;
        write_le32(&mut bios, off, encode_i(0x0D, 8, 8, 0x1800));
        off += 4;
        write_le32(&mut bios, off, encode_i(0x28, 8, 0, 0x0000));
        off += 4; // SB index=0
        write_le32(&mut bios, off, encode_i(0x0D, 0, 9, 0x0001));
        off += 4; // LI $t1, 1
        write_le32(&mut bios, off, encode_i(0x28, 8, 9, 0x0001));
        off += 4; // SB cmd=1
        write_le32(&mut bios, off, encode_i(0x20, 8, 10, 0x0001));
        off += 4; // LB resp
        write_le32(&mut bios, off, encode_j(2, (0xBFC00000 + off as u32) >> 2));
        off += 4;
        write_le32(&mut bios, off, 0);

        let mut sys = Ps1System::new();
        sys.bus_mut().load_bios(&bios).unwrap();
        let result = sys.step_frame();
        assert!(result.is_ok(), "CD-ROM probe should not panic");
    }

    #[test]
    fn test_ps1_no_bios_returns_error() {
        let mut sys = Ps1System::new();
        let result = sys.step_frame();
        assert!(result.is_err(), "step_frame without BIOS should error");
    }

    #[test]
    fn test_ps1_ram_mirrors() {
        let mut sys = Ps1System::new();
        let bus = sys.bus_mut();
        // Write to base RAM address
        bus.ram[0x1000] = 0xAB;
        bus.ram[0x1001] = 0xCD;
        bus.ram[0x1002] = 0xEF;
        bus.ram[0x1003] = 0x12;

        // Read from mirror at +2MB (0x00200000)
        assert_eq!(bus.read_byte(0x0020_1000), 0xAB);
        assert_eq!(bus.read_byte(0x0020_1001), 0xCD);
        // Read from mirror at +4MB (0x00400000)
        assert_eq!(bus.read_byte(0x0040_1000), 0xAB);
        // Read from mirror at +6MB (0x00600000)
        assert_eq!(bus.read_byte(0x0060_1000), 0xAB);

        // Halfword reads through mirror
        let hw = bus.read_halfword(0x0020_1000);
        assert_eq!(hw, 0xCDAB);

        // Word reads through mirror
        let w = bus.read_word(0x0020_1000);
        assert_eq!(w, 0x12EFCDAB);

        // Write through mirror and read from base
        bus.write_byte(0x0040_2000, 0x42);
        assert_eq!(bus.ram[0x2000], 0x42);
    }

    #[test]
    fn test_ps1_otc_dma() {
        let mut sys = Ps1System::new();
        let bus = sys.bus_mut();

        // Set up OTC DMA (channel 6) to clear an ordering table of 4 entries
        // starting at address 0x1000 (going backward)
        let base_addr: u32 = 0x100C; // Start at highest entry
        let word_count: u32 = 4;

        bus.dma.channels[6].base = base_addr;
        bus.dma.channels[6].block_control = word_count;
        bus.dma.channels[6].control = (1 << 24) | (1 << 28) | (1 << 1); // Enable, trigger, direction=to_ram

        bus.execute_dma(6);

        // Read back the OT entries
        let read_word = |bus: &Ps1Bus, addr: u32| -> u32 {
            let a = addr as usize;
            bus.ram[a] as u32
                | (bus.ram[a + 1] as u32) << 8
                | (bus.ram[a + 2] as u32) << 16
                | (bus.ram[a + 3] as u32) << 24
        };

        // First entry (highest addr 0x100C) points to 0x1008
        assert_eq!(read_word(bus, 0x100C), 0x0000_1008);
        // Second entry (0x1008) points to 0x1004
        assert_eq!(read_word(bus, 0x1008), 0x0000_1004);
        // Third entry (0x1004) points to 0x1000
        assert_eq!(read_word(bus, 0x1004), 0x0000_1000);
        // Last entry (0x1000) has end-of-list marker
        assert_eq!(read_word(bus, 0x1000), 0x00FF_FFFF);
    }

    #[test]
    fn test_ps1_audio_samples_generated() {
        let mut bios = vec![0u8; 512 * 1024];
        write_le32(&mut bios, 0, encode_j(2, 0xBFC00000 >> 2));

        let mut sys = Ps1System::new();
        sys.bus_mut().load_bios(&bios).unwrap();

        // Run one frame — generates SPU audio
        sys.step_frame().unwrap();

        // Get 735 stereo samples (standard for 44100 Hz at 60 fps)
        let samples = sys.get_audio_samples(735);
        assert_eq!(
            samples.len(),
            1470,
            "get_audio_samples(735) should return 1470 values (stereo)"
        );
    }
}
