//! Delta Modulation Channel (DMC) for NES APU.
//!
//! The DMC channel plays 1-bit delta-encoded samples from memory.
//!
//! ## Features
//!
//! - 7-bit output level (0-127)
//! - Sample playback from CPU memory via DMA
//! - 16 different sample rates
//! - Loop support
//! - IRQ generation on sample completion
//!
//! ## Register Interface
//!
//! - **$4010**: Flags and rate (IRQ enable, loop, rate index)
//! - **$4011**: Direct load (7-bit output level)
//! - **$4012**: Sample address ($C000 + address * 64)
//! - **$4013**: Sample length (length * 16 + 1 bytes)

/// NES DMC (Delta Modulation Channel).
///
/// Plays 1-bit delta-encoded samples from CPU memory.
#[derive(Debug, Clone)]
pub struct DmcChannel {
    /// IRQ enable flag
    pub irq_enabled: bool,
    /// Loop flag - restart sample when complete
    pub loop_enabled: bool,
    /// Sample rate index (0-15)
    pub rate_index: u8,
    /// Current output level (7-bit, 0-127)
    pub output_level: u8,
    /// Sample address ($C000 + address * 64)
    pub sample_address: u16,
    /// Sample length in bytes (length * 16 + 1)
    pub sample_length: u16,

    // Internal state
    /// Current address being read
    current_address: u16,
    /// Bytes remaining in current sample
    bytes_remaining: u16,
    /// Sample buffer (8 bits)
    sample_buffer: u8,
    /// Bits remaining in sample buffer
    bits_remaining: u8,
    /// Timer for sample rate
    timer: u16,
    /// Timer period (based on rate_index)
    timer_period: u16,
    /// Silence flag (no sample loaded)
    silence: bool,
    /// IRQ pending flag
    pub irq_pending: bool,
    /// Channel enabled flag
    pub enabled: bool,
}

/// NTSC DMC rate table (CPU cycles between output changes)
/// Reference: NESdev wiki - APU DMC
const DMC_RATE_TABLE_NTSC: [u16; 16] = [
    428, 380, 340, 320, 286, 254, 226, 214, 190, 160, 142, 128, 106, 84, 72, 54,
];

/// PAL DMC rate table (CPU cycles between output changes)
const DMC_RATE_TABLE_PAL: [u16; 16] = [
    398, 354, 316, 298, 276, 236, 210, 198, 176, 148, 132, 118, 98, 78, 66, 50,
];

impl DmcChannel {
    /// Create a new DMC channel.
    pub fn new() -> Self {
        Self {
            irq_enabled: false,
            loop_enabled: false,
            rate_index: 0,
            output_level: 0,
            sample_address: 0xC000,
            sample_length: 1,
            current_address: 0xC000,
            bytes_remaining: 0,
            sample_buffer: 0,
            bits_remaining: 0,
            timer: 0,
            timer_period: DMC_RATE_TABLE_NTSC[0],
            silence: true,
            irq_pending: false,
            enabled: false,
        }
    }

    /// Set the rate index and update timer period.
    pub fn set_rate(&mut self, rate_index: u8, use_pal: bool) {
        self.rate_index = rate_index & 0x0F;
        let table = if use_pal {
            &DMC_RATE_TABLE_PAL
        } else {
            &DMC_RATE_TABLE_NTSC
        };
        self.timer_period = table[self.rate_index as usize];
    }

    /// Write to $4010 - flags and rate
    pub fn write_flags_rate(&mut self, val: u8, use_pal: bool) {
        self.irq_enabled = (val & 0x80) != 0;
        self.loop_enabled = (val & 0x40) != 0;
        self.set_rate(val & 0x0F, use_pal);

        // Clear IRQ flag if IRQ is disabled
        if !self.irq_enabled {
            self.irq_pending = false;
        }
    }

    /// Write to $4011 - direct load
    pub fn write_direct_load(&mut self, val: u8) {
        self.output_level = val & 0x7F;
    }

    /// Write to $4012 - sample address
    pub fn write_sample_address(&mut self, val: u8) {
        self.sample_address = 0xC000 + (val as u16) * 64;
    }

    /// Write to $4013 - sample length
    pub fn write_sample_length(&mut self, val: u8) {
        self.sample_length = (val as u16) * 16 + 1;
    }

    /// Start playing the sample (called when enabled via $4015).
    pub fn start_sample(&mut self) {
        if self.bytes_remaining == 0 {
            self.current_address = self.sample_address;
            self.bytes_remaining = self.sample_length;
            self.silence = false;
        }
    }

    /// Stop playing (called when disabled via $4015).
    pub fn stop(&mut self) {
        self.bytes_remaining = 0;
        self.silence = true;
    }

    /// Check if sample has bytes remaining.
    pub fn has_bytes_remaining(&self) -> bool {
        self.bytes_remaining > 0
    }

    /// Get a byte from memory (to be provided by caller via DMA).
    /// This should be called by the system when the DMC needs a new byte.
    pub fn load_sample_byte(&mut self, byte: u8) {
        self.sample_buffer = byte;
        self.bits_remaining = 8;
        self.silence = false;

        // Advance to next byte
        self.current_address = self.current_address.wrapping_add(1);
        if self.current_address == 0 {
            self.current_address = 0x8000; // Wrap to start of ROM
        }

        if self.bytes_remaining > 0 {
            self.bytes_remaining -= 1;
        }

        // Check if sample is complete
        if self.bytes_remaining == 0 {
            if self.loop_enabled {
                // Restart sample
                self.current_address = self.sample_address;
                self.bytes_remaining = self.sample_length;
            } else {
                // Sample complete
                if self.irq_enabled {
                    self.irq_pending = true;
                }
            }
        }
    }

    /// Clock the DMC channel.
    /// Returns the current address to read from if a byte is needed.
    pub fn clock(&mut self) -> Option<u16> {
        if !self.enabled {
            return None;
        }

        // Decrement timer
        if self.timer > 0 {
            self.timer -= 1;
            return None;
        }

        // Timer expired, reload it
        self.timer = self.timer_period;

        // Process one bit
        if !self.silence {
            if self.bits_remaining > 0 {
                // Get the next bit from the sample buffer
                let bit = (self.sample_buffer & 1) != 0;
                self.sample_buffer >>= 1;
                self.bits_remaining -= 1;

                // Update output level based on bit
                if bit {
                    if self.output_level <= 125 {
                        self.output_level += 2;
                    }
                } else if self.output_level >= 2 {
                    self.output_level -= 2;
                }
            } else {
                // Need a new byte
                self.silence = true;
                if self.bytes_remaining > 0 {
                    return Some(self.current_address);
                }
            }
        }

        None
    }

    /// Get the current output level (0-127).
    pub fn output(&self) -> u8 {
        self.output_level
    }

    /// Clear and return IRQ pending flag.
    pub fn take_irq_pending(&mut self) -> bool {
        let was = self.irq_pending;
        self.irq_pending = false;
        was
    }
}

impl Default for DmcChannel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dmc_rate_setting() {
        let mut dmc = DmcChannel::new();
        dmc.write_flags_rate(0x0F, false); // Max rate on NTSC
        assert_eq!(dmc.timer_period, 54);

        dmc.write_flags_rate(0x00, false); // Min rate on NTSC
        assert_eq!(dmc.timer_period, 428);
    }

    #[test]
    fn test_dmc_direct_load() {
        let mut dmc = DmcChannel::new();
        dmc.write_direct_load(0xFF);
        assert_eq!(dmc.output_level, 0x7F); // Only 7 bits used

        dmc.write_direct_load(0x40);
        assert_eq!(dmc.output_level, 0x40);
    }

    #[test]
    fn test_dmc_sample_address() {
        let mut dmc = DmcChannel::new();
        dmc.write_sample_address(0x00);
        assert_eq!(dmc.sample_address, 0xC000);

        dmc.write_sample_address(0x80);
        assert_eq!(dmc.sample_address, 0xC000 + 0x80 * 64);
    }

    #[test]
    fn test_dmc_sample_length() {
        let mut dmc = DmcChannel::new();
        dmc.write_sample_length(0x00);
        assert_eq!(dmc.sample_length, 1);

        dmc.write_sample_length(0x10);
        assert_eq!(dmc.sample_length, 16 * 16 + 1);
    }

    #[test]
    fn test_dmc_output_changes() {
        let mut dmc = DmcChannel::new();
        dmc.enabled = true;
        dmc.output_level = 64;

        // Load a byte with bit pattern 0b10101010
        dmc.load_sample_byte(0b10101010);

        // Process bits - should alternate increment/decrement
        for _ in 0..8 {
            dmc.timer = 0; // Force timer expiry
            dmc.clock();
        }

        // After 4 increments (+2 each) and 4 decrements (-2 each), should be back at 64
        assert_eq!(dmc.output_level, 64);
    }
}
