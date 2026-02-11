//! GBA DMA (Direct Memory Access) controller.
//!
//! The GBA has 4 DMA channels (DMA0-DMA3) with hardware priority:
//! - **DMA0**: Highest priority, internal memory only
//! - **DMA1**: Medium-high priority, used for Sound FIFO A
//! - **DMA2**: Medium-low priority, used for Sound FIFO B
//! - **DMA3**: Lowest priority, general purpose, can write to Game Pak
//!
//! ## I/O Registers
//!
//! Each channel has 3 registers (12 bytes per channel):
//!
//! | Offset | Register  | Description                    |
//! |--------|-----------|--------------------------------|
//! | +0x00  | DMAxSAD   | Source Address (32-bit, W)      |
//! | +0x04  | DMAxDAD   | Destination Address (32-bit, W) |
//! | +0x08  | DMAxCNT_L | Word Count (16-bit, W)          |
//! | +0x0A  | DMAxCNT_H | Control (16-bit, R/W)           |
//!
//! Base addresses: DMA0=0x040000B0, DMA1=0x040000BC, DMA2=0x040000C8, DMA3=0x040000D4
//!
//! ## Control Register (DMAxCNT_H) Bits
//!
//! | Bit(s) | Description                                              |
//! |--------|----------------------------------------------------------|
//! | 5-6    | Dest Address Control: 0=Inc, 1=Dec, 2=Fixed, 3=Inc/Reload |
//! | 7-8    | Source Address Control: 0=Inc, 1=Dec, 2=Fixed, 3=Prohibited |
//! | 9      | DMA Repeat                                                |
//! | 10     | DMA Transfer Type: 0=16-bit, 1=32-bit                     |
//! | 12-13  | Start Timing: 0=Immediate, 1=VBlank, 2=HBlank, 3=Special  |
//! | 14     | IRQ on end of word count                                   |
//! | 15     | DMA Enable                                                 |

use emu_core::cpu_arm7tdmi::MemoryArm7;

/// Number of DMA channels
const NUM_CHANNELS: usize = 4;

/// DMA IRQ bits in IE/IF (bits 8-11)
const DMA_IRQ_BITS: [u16; NUM_CHANNELS] = [1 << 8, 1 << 9, 1 << 10, 1 << 11];

/// DMA start timing modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DmaStartTiming {
    /// Start immediately when enabled
    Immediate = 0,
    /// Start at VBlank
    VBlank = 1,
    /// Start at HBlank
    HBlank = 2,
    /// Special: DMA1/2=Sound FIFO, DMA3=Video Capture
    Special = 3,
}

impl DmaStartTiming {
    fn from_bits(bits: u8) -> Self {
        match bits & 3 {
            0 => DmaStartTiming::Immediate,
            1 => DmaStartTiming::VBlank,
            2 => DmaStartTiming::HBlank,
            3 => DmaStartTiming::Special,
            _ => unreachable!(),
        }
    }
}

/// Address control mode for source/destination
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddrControl {
    /// Increment after each transfer
    Increment = 0,
    /// Decrement after each transfer
    Decrement = 1,
    /// Fixed (don't change)
    Fixed = 2,
    /// Increment and reload (dest only; prohibited for source)
    IncrementReload = 3,
}

impl AddrControl {
    fn from_bits(bits: u8) -> Self {
        match bits & 3 {
            0 => AddrControl::Increment,
            1 => AddrControl::Decrement,
            2 => AddrControl::Fixed,
            3 => AddrControl::IncrementReload,
            _ => unreachable!(),
        }
    }
}

// Control register bit masks
const CTRL_DEST_ADDR_SHIFT: u16 = 5;
const CTRL_DEST_ADDR_MASK: u16 = 0x3 << CTRL_DEST_ADDR_SHIFT;
const CTRL_SRC_ADDR_SHIFT: u16 = 7;
const CTRL_SRC_ADDR_MASK: u16 = 0x3 << CTRL_SRC_ADDR_SHIFT;
const CTRL_REPEAT: u16 = 1 << 9;
const CTRL_TRANSFER_32: u16 = 1 << 10;
const CTRL_START_TIMING_SHIFT: u16 = 12;
const CTRL_START_TIMING_MASK: u16 = 0x3 << CTRL_START_TIMING_SHIFT;
const CTRL_IRQ_ENABLE: u16 = 1 << 14;
const CTRL_ENABLE: u16 = 1 << 15;

/// Maximum word counts per channel
/// DMA0-2: 14-bit (0x4000 max), DMA3: 16-bit (0x10000 max)
const MAX_WORD_COUNT: [u32; NUM_CHANNELS] = [0x4000, 0x4000, 0x4000, 0x10000];

/// Source address masks per channel
/// DMA0: 27-bit (internal only), DMA1-3: 28-bit
const SRC_ADDR_MASK: [u32; NUM_CHANNELS] = [0x07FF_FFFF, 0x0FFF_FFFF, 0x0FFF_FFFF, 0x0FFF_FFFF];

/// Destination address masks per channel
/// DMA0-2: 27-bit (internal only), DMA3: 28-bit
const DST_ADDR_MASK: [u32; NUM_CHANNELS] = [0x07FF_FFFF, 0x07FF_FFFF, 0x07FF_FFFF, 0x0FFF_FFFF];

/// A single DMA channel's state
#[derive(Debug, Clone)]
pub struct DmaChannel {
    /// Latched source address (internal, changes during transfer)
    src_addr: u32,
    /// Latched destination address (internal, changes during transfer)
    dst_addr: u32,
    /// Written source address register (reload value)
    src_addr_reg: u32,
    /// Written destination address register (reload value)
    dst_addr_reg: u32,
    /// Written word count register (reload value)
    word_count_reg: u16,
    /// Control register
    control: u16,
    /// Latched word count (internal)
    word_count: u32,
    /// Whether this channel is actively scheduled (enabled + correct timing)
    active: bool,
}

impl DmaChannel {
    fn new() -> Self {
        Self {
            src_addr: 0,
            dst_addr: 0,
            src_addr_reg: 0,
            dst_addr_reg: 0,
            word_count_reg: 0,
            control: 0,
            word_count: 0,
            active: false,
        }
    }

    fn reset(&mut self) {
        *self = Self::new();
    }

    /// Whether the channel is enabled
    fn enabled(&self) -> bool {
        self.control & CTRL_ENABLE != 0
    }

    /// Get the start timing mode
    fn start_timing(&self) -> DmaStartTiming {
        DmaStartTiming::from_bits(
            ((self.control & CTRL_START_TIMING_MASK) >> CTRL_START_TIMING_SHIFT) as u8,
        )
    }

    /// Whether transfers are 32-bit (true) or 16-bit (false)
    fn transfer_32(&self) -> bool {
        self.control & CTRL_TRANSFER_32 != 0
    }

    /// Whether repeat mode is enabled
    fn repeat(&self) -> bool {
        self.control & CTRL_REPEAT != 0
    }

    /// Whether to fire an IRQ on completion
    fn irq_on_end(&self) -> bool {
        self.control & CTRL_IRQ_ENABLE != 0
    }

    /// Get destination address control mode
    fn dest_control(&self) -> AddrControl {
        AddrControl::from_bits(((self.control & CTRL_DEST_ADDR_MASK) >> CTRL_DEST_ADDR_SHIFT) as u8)
    }

    /// Get source address control mode
    fn src_control(&self) -> AddrControl {
        let bits = ((self.control & CTRL_SRC_ADDR_MASK) >> CTRL_SRC_ADDR_SHIFT) as u8;
        // Source cannot use IncrementReload (mode 3 is prohibited)
        let ctrl = AddrControl::from_bits(bits);
        if ctrl == AddrControl::IncrementReload {
            AddrControl::Increment // Treat prohibited as increment
        } else {
            ctrl
        }
    }
}

/// DMA controller managing all 4 channels.
#[derive(Debug, Clone)]
pub struct Dma {
    channels: [DmaChannel; NUM_CHANNELS],
}

impl Dma {
    /// Create a new DMA controller with all channels disabled
    pub fn new() -> Self {
        Self {
            channels: [
                DmaChannel::new(),
                DmaChannel::new(),
                DmaChannel::new(),
                DmaChannel::new(),
            ],
        }
    }

    /// Reset all DMA channels
    pub fn reset(&mut self) {
        for ch in &mut self.channels {
            ch.reset();
        }
    }

    /// Read a DMA I/O register byte.
    ///
    /// `offset` is relative to 0x040000B0.
    /// Only DMAxCNT_H (control) is readable. Other registers return 0.
    pub fn read(&self, offset: u32) -> u8 {
        let channel_idx = (offset / 12) as usize;
        let reg_offset = offset % 12;

        if channel_idx >= NUM_CHANNELS {
            return 0;
        }

        let ch = &self.channels[channel_idx];

        match reg_offset {
            // DMAxSAD (write-only)
            0..=3 => 0,
            // DMAxDAD (write-only)
            4..=7 => 0,
            // DMAxCNT_L (write-only)
            8..=9 => 0,
            // DMAxCNT_H (readable)
            10 => ch.control as u8,
            11 => (ch.control >> 8) as u8,
            _ => 0,
        }
    }

    /// Write a DMA I/O register byte.
    ///
    /// `offset` is relative to 0x040000B0.
    /// Returns an IRQ bitmask if an immediate DMA transfer completed with IRQ enabled.
    pub fn write(&mut self, offset: u32, val: u8) -> u16 {
        let channel_idx = (offset / 12) as usize;
        let reg_offset = offset % 12;

        if channel_idx >= NUM_CHANNELS {
            return 0;
        }

        let ch = &mut self.channels[channel_idx];

        match reg_offset {
            // DMAxSAD (source address, 4 bytes)
            0 => ch.src_addr_reg = (ch.src_addr_reg & 0xFFFF_FF00) | val as u32,
            1 => ch.src_addr_reg = (ch.src_addr_reg & 0xFFFF_00FF) | ((val as u32) << 8),
            2 => ch.src_addr_reg = (ch.src_addr_reg & 0xFF00_FFFF) | ((val as u32) << 16),
            3 => {
                ch.src_addr_reg = (ch.src_addr_reg & 0x00FF_FFFF) | ((val as u32) << 24);
                ch.src_addr_reg &= SRC_ADDR_MASK[channel_idx];
            }

            // DMAxDAD (destination address, 4 bytes)
            4 => ch.dst_addr_reg = (ch.dst_addr_reg & 0xFFFF_FF00) | val as u32,
            5 => ch.dst_addr_reg = (ch.dst_addr_reg & 0xFFFF_00FF) | ((val as u32) << 8),
            6 => ch.dst_addr_reg = (ch.dst_addr_reg & 0xFF00_FFFF) | ((val as u32) << 16),
            7 => {
                ch.dst_addr_reg = (ch.dst_addr_reg & 0x00FF_FFFF) | ((val as u32) << 24);
                ch.dst_addr_reg &= DST_ADDR_MASK[channel_idx];
            }

            // DMAxCNT_L (word count, 2 bytes)
            8 => ch.word_count_reg = (ch.word_count_reg & 0xFF00) | val as u16,
            9 => ch.word_count_reg = (ch.word_count_reg & 0x00FF) | ((val as u16) << 8),

            // DMAxCNT_H (control, 2 bytes)
            10 => {
                ch.control = (ch.control & 0xFF00) | val as u16;
            }
            11 => {
                let old_control = ch.control;
                ch.control = (ch.control & 0x00FF) | ((val as u16) << 8);

                let was_enabled = old_control & CTRL_ENABLE != 0;
                let now_enabled = ch.control & CTRL_ENABLE != 0;

                if !was_enabled && now_enabled {
                    // Rising edge of enable bit: latch registers
                    ch.src_addr = ch.src_addr_reg;
                    ch.dst_addr = ch.dst_addr_reg;
                    let wc = ch.word_count_reg as u32;
                    ch.word_count = if wc == 0 {
                        MAX_WORD_COUNT[channel_idx]
                    } else {
                        wc & (MAX_WORD_COUNT[channel_idx] - 1)
                    };

                    // Immediate DMA handled by caller after write completes
                    if ch.start_timing() == DmaStartTiming::Immediate {
                        ch.active = true;
                    }
                } else if !now_enabled {
                    ch.active = false;
                }
            }
            _ => {}
        }

        0 // IRQs from immediate DMA are handled during execute_pending
    }

    /// Check if any DMA channel is pending execution and return which channels
    /// need to run. Called after writes to DMA control registers to trigger
    /// immediate transfers.
    pub fn has_pending_immediate(&self) -> bool {
        self.channels
            .iter()
            .any(|ch| ch.active && ch.enabled() && ch.start_timing() == DmaStartTiming::Immediate)
    }

    /// Trigger DMA channels for a specific timing event.
    ///
    /// Called by the system at appropriate times (VBlank, HBlank).
    /// Returns channels that should be activated.
    pub fn notify_timing(&mut self, timing: DmaStartTiming) {
        for (ch_idx, ch) in self.channels.iter_mut().enumerate() {
            if ch.enabled() && ch.start_timing() == timing {
                ch.active = true;

                // For repeat mode, reload word count (and dest addr if IncrementReload)
                if ch.repeat() {
                    let wc = ch.word_count_reg as u32;
                    ch.word_count = if wc == 0 {
                        MAX_WORD_COUNT[ch_idx]
                    } else {
                        wc & (MAX_WORD_COUNT[ch_idx] - 1)
                    };

                    if ch.dest_control() == AddrControl::IncrementReload {
                        ch.dst_addr = ch.dst_addr_reg;
                    }
                }
            }
        }
    }

    /// Execute all pending DMA transfers in priority order.
    ///
    /// This performs the actual memory transfers through the provided bus.
    /// DMA channels are processed in priority order (0 = highest).
    ///
    /// Returns (cycles_consumed, irq_bits)
    pub fn execute_with_bus(&mut self, bus: &mut impl MemoryArm7) -> (u64, u16) {
        let mut total_cycles: u64 = 0;
        let mut irq_bits: u16 = 0;

        // Process channels in priority order (0 = highest)
        for ch_idx in 0..NUM_CHANNELS {
            if !self.channels[ch_idx].active {
                continue;
            }

            let ch = &mut self.channels[ch_idx];
            ch.active = false;

            if !ch.enabled() {
                continue;
            }

            let transfer_32 = ch.transfer_32();
            let step = if transfer_32 { 4u32 } else { 2u32 };
            let src_ctrl = ch.src_control();
            let dst_ctrl = ch.dest_control();
            let word_count = ch.word_count;

            // Calculate address adjustments
            let src_step: i32 = match src_ctrl {
                AddrControl::Increment => step as i32,
                AddrControl::Decrement => -(step as i32),
                AddrControl::Fixed => 0,
                AddrControl::IncrementReload => step as i32, // Should not happen for src
            };
            let dst_step: i32 = match dst_ctrl {
                AddrControl::Increment | AddrControl::IncrementReload => step as i32,
                AddrControl::Decrement => -(step as i32),
                AddrControl::Fixed => 0,
            };

            // Perform the transfer
            for _ in 0..word_count {
                if transfer_32 {
                    let val = bus.read_word(ch.src_addr);
                    bus.write_word(ch.dst_addr, val);
                } else {
                    let val = bus.read_halfword(ch.src_addr);
                    bus.write_halfword(ch.dst_addr, val);
                }

                ch.src_addr = (ch.src_addr as i64 + src_step as i64) as u32;
                ch.dst_addr = (ch.dst_addr as i64 + dst_step as i64) as u32;
            }

            // Each unit transferred takes ~2 cycles (access time varies by memory)
            total_cycles += word_count as u64 * 2;

            // Fire IRQ if enabled
            if ch.irq_on_end() {
                irq_bits |= DMA_IRQ_BITS[ch_idx];
            }

            // Handle repeat mode
            if ch.repeat() && ch.start_timing() != DmaStartTiming::Immediate {
                // Reload word count
                let wc = ch.word_count_reg as u32;
                ch.word_count = if wc == 0 {
                    MAX_WORD_COUNT[ch_idx]
                } else {
                    wc & (MAX_WORD_COUNT[ch_idx] - 1)
                };

                // Reload destination for IncrementReload mode
                if dst_ctrl == AddrControl::IncrementReload {
                    ch.dst_addr = ch.dst_addr_reg;
                }
                // Channel stays enabled for next trigger
            } else {
                // Disable channel after transfer
                ch.control &= !CTRL_ENABLE;
            }
        }

        (total_cycles, irq_bits)
    }

    /// Execute all pending DMA transfers using closures for memory access.
    ///
    /// This is primarily used for testing where a full bus isn't available.
    ///
    /// Returns (cycles_consumed, irq_bits)
    #[cfg(test)]
    pub fn execute_pending(
        &mut self,
        mut read16: impl FnMut(u32) -> u16,
        mut read32: impl FnMut(u32) -> u32,
        mut write16: impl FnMut(u32, u16),
        mut write32: impl FnMut(u32, u32),
    ) -> (u64, u16) {
        let mut total_cycles: u64 = 0;
        let mut irq_bits: u16 = 0;

        // Process channels in priority order (0 = highest)
        for ch_idx in 0..NUM_CHANNELS {
            if !self.channels[ch_idx].active {
                continue;
            }

            let ch = &mut self.channels[ch_idx];
            ch.active = false;

            if !ch.enabled() {
                continue;
            }

            let transfer_32 = ch.transfer_32();
            let step = if transfer_32 { 4u32 } else { 2u32 };
            let src_ctrl = ch.src_control();
            let dst_ctrl = ch.dest_control();
            let word_count = ch.word_count;

            // Calculate address adjustments
            let src_step: i32 = match src_ctrl {
                AddrControl::Increment => step as i32,
                AddrControl::Decrement => -(step as i32),
                AddrControl::Fixed => 0,
                AddrControl::IncrementReload => step as i32, // Should not happen for src
            };
            let dst_step: i32 = match dst_ctrl {
                AddrControl::Increment | AddrControl::IncrementReload => step as i32,
                AddrControl::Decrement => -(step as i32),
                AddrControl::Fixed => 0,
            };

            // Perform the transfer
            for _ in 0..word_count {
                if transfer_32 {
                    let val = read32(ch.src_addr);
                    write32(ch.dst_addr, val);
                } else {
                    let val = read16(ch.src_addr);
                    write16(ch.dst_addr, val);
                }

                ch.src_addr = (ch.src_addr as i64 + src_step as i64) as u32;
                ch.dst_addr = (ch.dst_addr as i64 + dst_step as i64) as u32;
            }

            // Each unit transferred takes ~2 cycles (access time varies by memory)
            total_cycles += word_count as u64 * 2;

            // Fire IRQ if enabled
            if ch.irq_on_end() {
                irq_bits |= DMA_IRQ_BITS[ch_idx];
            }

            // Handle repeat mode
            if ch.repeat() && ch.start_timing() != DmaStartTiming::Immediate {
                // Reload word count
                let wc = ch.word_count_reg as u32;
                ch.word_count = if wc == 0 {
                    MAX_WORD_COUNT[ch_idx]
                } else {
                    wc & (MAX_WORD_COUNT[ch_idx] - 1)
                };

                // Reload destination for IncrementReload mode
                if dst_ctrl == AddrControl::IncrementReload {
                    ch.dst_addr = ch.dst_addr_reg;
                }
                // Channel stays enabled for next trigger
            } else {
                // Disable channel after transfer
                ch.control &= !CTRL_ENABLE;
            }
        }

        (total_cycles, irq_bits)
    }

    /// Check if any DMA channel is actively transferring.
    /// Used to determine if CPU should be halted during DMA.
    pub fn is_transferring(&self) -> bool {
        self.channels.iter().any(|ch| ch.active)
    }

    /// Get a reference to a specific channel (for debugging)
    pub fn channel(&self, idx: usize) -> Option<&DmaChannel> {
        self.channels.get(idx)
    }

    /// Get DMA3's word count and destination address if it's active.
    ///
    /// Used for EEPROM size detection — the DMA word count indicates
    /// whether 6-bit or 14-bit addressing is being used.
    pub fn dma3_transfer_info(&self) -> Option<(u32, u32)> {
        let ch = &self.channels[3];
        if ch.active && ch.enabled() {
            Some((ch.word_count, ch.dst_addr))
        } else {
            None
        }
    }
}

impl Default for Dma {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let dma = Dma::new();
        for i in 0..NUM_CHANNELS {
            let ch = &dma.channels[i];
            assert_eq!(ch.src_addr, 0);
            assert_eq!(ch.dst_addr, 0);
            assert_eq!(ch.control, 0);
            assert!(!ch.enabled());
            assert!(!ch.active);
        }
    }

    #[test]
    fn test_reset() {
        let mut dma = Dma::new();
        // Set some state
        dma.channels[0].control = CTRL_ENABLE;
        dma.channels[0].src_addr = 0x1234;
        dma.reset();
        assert_eq!(dma.channels[0].control, 0);
        assert_eq!(dma.channels[0].src_addr, 0);
    }

    #[test]
    fn test_write_source_addr() {
        let mut dma = Dma::new();
        // DMA0 source = 0x02001234
        dma.write(0, 0x34); // byte 0 of SAD
        dma.write(1, 0x12); // byte 1 of SAD
        dma.write(2, 0x00); // byte 2 of SAD
        dma.write(3, 0x02); // byte 3 of SAD
        assert_eq!(dma.channels[0].src_addr_reg, 0x02001234);
    }

    #[test]
    fn test_write_dest_addr() {
        let mut dma = Dma::new();
        // DMA0 dest = 0x03005678
        dma.write(4, 0x78); // byte 0 of DAD
        dma.write(5, 0x56); // byte 1 of DAD
        dma.write(6, 0x00); // byte 2 of DAD
        dma.write(7, 0x03); // byte 3 of DAD
        assert_eq!(dma.channels[0].dst_addr_reg, 0x03005678);
    }

    #[test]
    fn test_write_word_count() {
        let mut dma = Dma::new();
        // DMA0 word count = 0x0100
        dma.write(8, 0x00); // low byte
        dma.write(9, 0x01); // high byte
        assert_eq!(dma.channels[0].word_count_reg, 0x0100);
    }

    #[test]
    fn test_control_read_write() {
        let mut dma = Dma::new();
        // Write control: enable, immediate, 16-bit, increment
        let ctrl = CTRL_ENABLE; // 0x8000
        dma.write(10, ctrl as u8);
        dma.write(11, (ctrl >> 8) as u8);

        assert_eq!(dma.read(10), ctrl as u8);
        assert_eq!(dma.read(11), (ctrl >> 8) as u8);
    }

    #[test]
    fn test_source_dest_write_only() {
        let mut dma = Dma::new();
        dma.channels[0].src_addr_reg = 0x12345678;
        dma.channels[0].dst_addr_reg = 0xABCD0000;
        dma.channels[0].word_count_reg = 0x100;

        // SAD is write-only
        assert_eq!(dma.read(0), 0);
        assert_eq!(dma.read(1), 0);
        assert_eq!(dma.read(2), 0);
        assert_eq!(dma.read(3), 0);

        // DAD is write-only
        assert_eq!(dma.read(4), 0);
        assert_eq!(dma.read(5), 0);
        assert_eq!(dma.read(6), 0);
        assert_eq!(dma.read(7), 0);

        // CNT_L is write-only
        assert_eq!(dma.read(8), 0);
        assert_eq!(dma.read(9), 0);
    }

    #[test]
    fn test_dma1_registers() {
        let mut dma = Dma::new();
        // DMA1 starts at offset 12
        dma.write(12, 0x00); // DMA1 SAD low
        dma.write(13, 0x00);
        dma.write(14, 0x00);
        dma.write(15, 0x08); // 0x08000000
        assert_eq!(dma.channels[1].src_addr_reg, 0x08000000);

        dma.write(16, 0x00); // DMA1 DAD low
        dma.write(17, 0x00);
        dma.write(18, 0x00);
        dma.write(19, 0x04); // 0x04000000
        assert_eq!(dma.channels[1].dst_addr_reg, 0x04000000);
    }

    #[test]
    fn test_enable_latches_registers() {
        let mut dma = Dma::new();
        // Set source, dest, word count for DMA0
        dma.write(0, 0x00);
        dma.write(1, 0x00);
        dma.write(2, 0x00);
        dma.write(3, 0x02); // src = 0x02000000

        dma.write(4, 0x00);
        dma.write(5, 0x00);
        dma.write(6, 0x00);
        dma.write(7, 0x03); // dst = 0x03000000

        dma.write(8, 0x04);
        dma.write(9, 0x00); // word count = 4

        // Enable with immediate start
        dma.write(10, 0x00);
        dma.write(11, 0x80); // CTRL_ENABLE

        assert!(dma.channels[0].enabled());
        assert!(dma.channels[0].active);
        assert_eq!(dma.channels[0].src_addr, 0x02000000);
        assert_eq!(dma.channels[0].dst_addr, 0x03000000);
        assert_eq!(dma.channels[0].word_count, 4);
    }

    #[test]
    fn test_zero_word_count_uses_max() {
        let mut dma = Dma::new();
        // Word count = 0
        dma.write(8, 0x00);
        dma.write(9, 0x00);

        // Enable
        dma.write(10, 0x00);
        dma.write(11, 0x80);

        // DMA0 max word count is 0x4000
        assert_eq!(dma.channels[0].word_count, 0x4000);
    }

    #[test]
    fn test_dma3_max_word_count() {
        let mut dma = Dma::new();
        // DMA3 at offset 36
        // Word count = 0
        dma.write(36 + 8, 0x00);
        dma.write(36 + 9, 0x00);

        // Enable
        dma.write(36 + 10, 0x00);
        dma.write(36 + 11, 0x80);

        // DMA3 max word count is 0x10000
        assert_eq!(dma.channels[3].word_count, 0x10000);
    }

    #[test]
    fn test_immediate_transfer_16bit() {
        let mut dma = Dma::new();
        // Source: some memory with test data
        let src_data: Vec<u16> = vec![0x1234, 0x5678, 0xABCD, 0xEF01];
        let mut dst_data: Vec<u16> = vec![0; 4];

        // Setup DMA0: src=0x100, dst=0x200, count=4, 16-bit, immediate
        dma.channels[0].src_addr_reg = 0x100;
        dma.channels[0].dst_addr_reg = 0x200;
        dma.channels[0].word_count_reg = 4;
        // Enable immediately
        dma.channels[0].control = CTRL_ENABLE;
        dma.channels[0].src_addr = 0x100;
        dma.channels[0].dst_addr = 0x200;
        dma.channels[0].word_count = 4;
        dma.channels[0].active = true;

        let (cycles, irq) = dma.execute_pending(
            |addr| {
                let idx = ((addr - 0x100) / 2) as usize;
                src_data[idx]
            },
            |_| 0,
            |addr, val| {
                let idx = ((addr - 0x200) / 2) as usize;
                dst_data[idx] = val;
            },
            |_, _| {},
        );

        assert_eq!(dst_data, vec![0x1234, 0x5678, 0xABCD, 0xEF01]);
        assert_eq!(cycles, 8); // 4 words * 2 cycles
        assert_eq!(irq, 0); // No IRQ enabled

        // Channel should be disabled after immediate transfer
        assert!(!dma.channels[0].enabled());
    }

    #[test]
    fn test_immediate_transfer_32bit() {
        let mut dma = Dma::new();
        let src_data: Vec<u32> = vec![0xDEADBEEF, 0xCAFEBABE];
        let mut dst_data: Vec<u32> = vec![0; 2];

        // Setup DMA0: 32-bit transfer
        dma.channels[0].src_addr = 0x100;
        dma.channels[0].dst_addr = 0x200;
        dma.channels[0].word_count = 2;
        dma.channels[0].control = CTRL_ENABLE | CTRL_TRANSFER_32;
        dma.channels[0].active = true;

        let (cycles, _) = dma.execute_pending(
            |_| 0,
            |addr| {
                let idx = ((addr - 0x100) / 4) as usize;
                src_data[idx]
            },
            |_, _| {},
            |addr, val| {
                let idx = ((addr - 0x200) / 4) as usize;
                dst_data[idx] = val;
            },
        );

        assert_eq!(dst_data, vec![0xDEADBEEF, 0xCAFEBABE]);
        assert_eq!(cycles, 4); // 2 words * 2 cycles
    }

    #[test]
    fn test_irq_on_completion() {
        let mut dma = Dma::new();
        // Setup DMA0 with IRQ enabled
        dma.channels[0].src_addr = 0x100;
        dma.channels[0].dst_addr = 0x200;
        dma.channels[0].word_count = 1;
        dma.channels[0].control = CTRL_ENABLE | CTRL_IRQ_ENABLE;
        dma.channels[0].active = true;

        let (_, irq) = dma.execute_pending(|_| 0, |_| 0, |_, _| {}, |_, _| {});

        assert_eq!(irq, DMA_IRQ_BITS[0]); // DMA0 IRQ bit 8
    }

    #[test]
    fn test_irq_bits_correct_per_channel() {
        // Verify IRQ bit assignments
        assert_eq!(DMA_IRQ_BITS[0], 1 << 8);
        assert_eq!(DMA_IRQ_BITS[1], 1 << 9);
        assert_eq!(DMA_IRQ_BITS[2], 1 << 10);
        assert_eq!(DMA_IRQ_BITS[3], 1 << 11);
    }

    #[test]
    fn test_source_decrement() {
        let mut dma = Dma::new();
        let mut reads: Vec<u32> = Vec::new();

        dma.channels[0].src_addr = 0x106; // Start at end
        dma.channels[0].dst_addr = 0x200;
        dma.channels[0].word_count = 4;
        // Src decrement, 16-bit
        dma.channels[0].control = CTRL_ENABLE | (1 << CTRL_SRC_ADDR_SHIFT); // src = decrement
        dma.channels[0].active = true;

        dma.execute_pending(
            |addr| {
                reads.push(addr);
                0
            },
            |_| 0,
            |_, _| {},
            |_, _| {},
        );

        assert_eq!(reads, vec![0x106, 0x104, 0x102, 0x100]);
    }

    #[test]
    fn test_dest_fixed() {
        let mut dma = Dma::new();
        let mut writes: Vec<u32> = Vec::new();

        dma.channels[0].src_addr = 0x100;
        dma.channels[0].dst_addr = 0x200;
        dma.channels[0].word_count = 3;
        // Dst fixed (mode 2), 16-bit
        dma.channels[0].control = CTRL_ENABLE | (2 << CTRL_DEST_ADDR_SHIFT); // dest = fixed
        dma.channels[0].active = true;

        dma.execute_pending(
            |_| 0x42,
            |_| 0,
            |addr, _| {
                writes.push(addr);
            },
            |_, _| {},
        );

        // All writes to same address
        assert_eq!(writes, vec![0x200, 0x200, 0x200]);
    }

    #[test]
    fn test_repeat_non_immediate() {
        let mut dma = Dma::new();

        // Setup DMA0 with repeat at VBlank
        dma.channels[0].src_addr = 0x100;
        dma.channels[0].dst_addr = 0x200;
        dma.channels[0].word_count = 2;
        dma.channels[0].word_count_reg = 2;
        dma.channels[0].control = CTRL_ENABLE | CTRL_REPEAT | (1 << CTRL_START_TIMING_SHIFT); // VBlank timing
        dma.channels[0].active = true;

        let (_, _) = dma.execute_pending(|_| 0, |_| 0, |_, _| {}, |_, _| {});

        // Channel should still be enabled (repeat)
        assert!(dma.channels[0].enabled());
        // Word count is reloaded
        assert_eq!(dma.channels[0].word_count, 2);
    }

    #[test]
    fn test_notify_vblank() {
        let mut dma = Dma::new();

        // Setup DMA0 with VBlank timing
        dma.channels[0].control = CTRL_ENABLE | (1 << CTRL_START_TIMING_SHIFT); // VBlank
        dma.channels[0].word_count_reg = 4;
        dma.channels[0].src_addr = 0x100;
        dma.channels[0].dst_addr = 0x200;
        dma.channels[0].word_count = 4;

        assert!(!dma.channels[0].active);
        dma.notify_timing(DmaStartTiming::VBlank);
        assert!(dma.channels[0].active);
    }

    #[test]
    fn test_notify_hblank() {
        let mut dma = Dma::new();

        // Setup DMA2 with HBlank timing
        dma.channels[2].control = CTRL_ENABLE | (2 << CTRL_START_TIMING_SHIFT); // HBlank
        dma.channels[2].word_count = 4;

        dma.notify_timing(DmaStartTiming::HBlank);
        assert!(dma.channels[2].active);
    }

    #[test]
    fn test_priority_order() {
        let mut dma = Dma::new();
        let mut order: Vec<usize> = Vec::new();

        // Activate DMA3 and DMA0 simultaneously
        for i in [3, 0] {
            dma.channels[i].src_addr = 0x100 + i as u32 * 0x100;
            dma.channels[i].dst_addr = 0x1000 + i as u32 * 0x100;
            dma.channels[i].word_count = 1;
            dma.channels[i].control = CTRL_ENABLE;
            dma.channels[i].active = true;
        }

        dma.execute_pending(
            |addr| {
                let ch = ((addr - 0x100) / 0x100) as usize;
                order.push(ch);
                0
            },
            |_| 0,
            |_, _| {},
            |_, _| {},
        );

        // DMA0 should execute before DMA3
        assert_eq!(order, vec![0, 3]);
    }

    #[test]
    fn test_disable_clears_active() {
        let mut dma = Dma::new();

        // Enable channel
        dma.channels[0].control = CTRL_ENABLE;
        dma.channels[0].active = true;

        // Disable via control write
        dma.write(10, 0x00);
        dma.write(11, 0x00); // Clear enable bit

        assert!(!dma.channels[0].active);
        assert!(!dma.channels[0].enabled());
    }

    #[test]
    fn test_addr_masks() {
        let mut dma = Dma::new();

        // DMA0 source is 27-bit (internal only)
        dma.write(0, 0xFF);
        dma.write(1, 0xFF);
        dma.write(2, 0xFF);
        dma.write(3, 0xFF); // Write all 1s
        assert_eq!(dma.channels[0].src_addr_reg, 0x07FF_FFFF);

        // DMA3 source is 28-bit
        dma.write(36, 0xFF);
        dma.write(36 + 1, 0xFF);
        dma.write(36 + 2, 0xFF);
        dma.write(36 + 3, 0xFF);
        assert_eq!(dma.channels[3].src_addr_reg, 0x0FFF_FFFF);

        // DMA0 dest is 27-bit
        dma.write(4, 0xFF);
        dma.write(5, 0xFF);
        dma.write(6, 0xFF);
        dma.write(7, 0xFF);
        assert_eq!(dma.channels[0].dst_addr_reg, 0x07FF_FFFF);

        // DMA3 dest is 28-bit
        dma.write(36 + 4, 0xFF);
        dma.write(36 + 5, 0xFF);
        dma.write(36 + 6, 0xFF);
        dma.write(36 + 7, 0xFF);
        assert_eq!(dma.channels[3].dst_addr_reg, 0x0FFF_FFFF);
    }

    #[test]
    fn test_start_timing_modes() {
        assert_eq!(DmaStartTiming::from_bits(0), DmaStartTiming::Immediate);
        assert_eq!(DmaStartTiming::from_bits(1), DmaStartTiming::VBlank);
        assert_eq!(DmaStartTiming::from_bits(2), DmaStartTiming::HBlank);
        assert_eq!(DmaStartTiming::from_bits(3), DmaStartTiming::Special);
    }

    #[test]
    fn test_addr_control_modes() {
        assert_eq!(AddrControl::from_bits(0), AddrControl::Increment);
        assert_eq!(AddrControl::from_bits(1), AddrControl::Decrement);
        assert_eq!(AddrControl::from_bits(2), AddrControl::Fixed);
        assert_eq!(AddrControl::from_bits(3), AddrControl::IncrementReload);
    }

    #[test]
    fn test_src_prohibits_increment_reload() {
        let mut ch = DmaChannel::new();
        // Set source control to mode 3 (prohibited)
        ch.control = 3 << CTRL_SRC_ADDR_SHIFT;
        // Should fall back to Increment
        assert_eq!(ch.src_control(), AddrControl::Increment);
    }

    #[test]
    fn test_dest_increment_reload() {
        let mut dma = Dma::new();
        let mut write_addrs: Vec<u32> = Vec::new();

        // DMA0 with IncrementReload dest, repeat, VBlank timing
        dma.channels[0].src_addr = 0x100;
        dma.channels[0].dst_addr = 0x200;
        dma.channels[0].dst_addr_reg = 0x200;
        dma.channels[0].word_count = 2;
        dma.channels[0].word_count_reg = 2;
        dma.channels[0].control = CTRL_ENABLE | CTRL_REPEAT
            | (3 << CTRL_DEST_ADDR_SHIFT) // IncrementReload
            | (1 << CTRL_START_TIMING_SHIFT); // VBlank
        dma.channels[0].active = true;

        // First transfer
        dma.execute_pending(|_| 0, |_| 0, |addr, _| write_addrs.push(addr), |_, _| {});

        // Dest should have incremented
        assert_eq!(write_addrs, vec![0x200, 0x202]);

        // After repeat, dest should be reloaded to original
        assert_eq!(dma.channels[0].dst_addr, 0x200);
    }

    #[test]
    fn test_has_pending_immediate() {
        let mut dma = Dma::new();
        assert!(!dma.has_pending_immediate());

        // Setup immediate DMA
        dma.channels[0].control = CTRL_ENABLE;
        dma.channels[0].active = true;
        assert!(dma.has_pending_immediate());

        // VBlank DMA is not "immediate"
        dma.channels[0].control = CTRL_ENABLE | (1 << CTRL_START_TIMING_SHIFT);
        assert!(!dma.has_pending_immediate());
    }
}
