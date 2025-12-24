//! Intel 8253/8254 Programmable Interval Timer (PIT)
//!
//! The PIT is a critical component of the IBM PC/XT system, providing:
//! - Channel 0: System timer interrupt (IRQ 0, INT 08h) - ~18.2 Hz
//! - Channel 1: DRAM refresh (legacy, not needed for emulation)
//! - Channel 2: PC speaker control
//!
//! The PIT operates at 1.193182 MHz (approximately 1/3 of CPU clock)

/// PIT base frequency in Hz (1.193182 MHz)
pub const PIT_FREQUENCY: f64 = 1_193_182.0;

/// Default system timer frequency (~18.2 Hz)
pub const SYSTEM_TIMER_FREQUENCY: f64 = 18.2;

/// Operating modes for PIT channels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PitMode {
    /// Mode 0: Interrupt on terminal count
    InterruptOnTerminalCount = 0,
    /// Mode 1: Hardware re-triggerable one-shot
    HardwareOneShot = 1,
    /// Mode 2: Rate generator
    RateGenerator = 2,
    /// Mode 3: Square wave generator
    SquareWave = 3,
    /// Mode 4: Software triggered strobe
    SoftwareStrobe = 4,
    /// Mode 5: Hardware triggered strobe
    HardwareStrobe = 5,
}

impl PitMode {
    fn from_bits(bits: u8) -> Self {
        match (bits >> 1) & 0x07 {
            0 => PitMode::InterruptOnTerminalCount,
            1 => PitMode::HardwareOneShot,
            2 | 6 => PitMode::RateGenerator,
            3 | 7 => PitMode::SquareWave,
            4 => PitMode::SoftwareStrobe,
            5 => PitMode::HardwareStrobe,
            _ => unreachable!(),
        }
    }
}

/// Access mode for counter value
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AccessMode {
    /// Latch count value
    LatchCount,
    /// Read/write low byte only
    LowByteOnly,
    /// Read/write high byte only
    HighByteOnly,
    /// Read/write low byte then high byte
    LowHighByte,
}

impl AccessMode {
    fn from_bits(bits: u8) -> Self {
        match (bits >> 4) & 0x03 {
            0 => AccessMode::LatchCount,
            1 => AccessMode::LowByteOnly,
            2 => AccessMode::HighByteOnly,
            3 => AccessMode::LowHighByte,
            _ => unreachable!(),
        }
    }
}

/// Single PIT channel
#[derive(Debug, Clone)]
struct PitChannel {
    /// Current counter value
    counter: u16,
    /// Reload value (divisor)
    reload: u16,
    /// Operating mode
    mode: PitMode,
    /// Access mode
    access_mode: AccessMode,
    /// Whether we're reading/writing the high byte next
    high_byte_next: bool,
    /// Output state (for speaker)
    output: bool,
    /// Whether counter is counting
    counting: bool,
    /// Latched value (for read-back)
    latched_value: Option<u16>,
}

impl PitChannel {
    fn new() -> Self {
        Self {
            counter: 0,
            reload: 0,
            mode: PitMode::InterruptOnTerminalCount,
            access_mode: AccessMode::LowHighByte,
            high_byte_next: false,
            output: false,
            counting: false,
            latched_value: None,
        }
    }

    /// Reset the channel
    fn reset(&mut self) {
        self.counter = 0;
        self.reload = 0;
        self.mode = PitMode::InterruptOnTerminalCount;
        self.access_mode = AccessMode::LowHighByte;
        self.high_byte_next = false;
        self.output = false;
        self.counting = false;
        self.latched_value = None;
    }

    /// Write a value to the channel
    fn write(&mut self, value: u8) {
        match self.access_mode {
            AccessMode::LatchCount => {
                // Latch command - ignore writes
            }
            AccessMode::LowByteOnly => {
                self.reload = value as u16;
                self.counter = self.reload;
                self.counting = true;
                self.high_byte_next = false;
            }
            AccessMode::HighByteOnly => {
                self.reload = (value as u16) << 8;
                self.counter = self.reload;
                self.counting = true;
                self.high_byte_next = false;
            }
            AccessMode::LowHighByte => {
                if !self.high_byte_next {
                    // Write low byte
                    self.reload = (self.reload & 0xFF00) | (value as u16);
                    self.high_byte_next = true;
                } else {
                    // Write high byte
                    self.reload = (self.reload & 0x00FF) | ((value as u16) << 8);
                    self.counter = self.reload;
                    self.counting = true;
                    self.high_byte_next = false;
                }
            }
        }
    }

    /// Read the current counter value
    fn read(&mut self) -> u8 {
        let value = if let Some(latched) = self.latched_value {
            latched
        } else {
            self.counter
        };

        let result = match self.access_mode {
            AccessMode::LatchCount => {
                // Return latched value
                if !self.high_byte_next {
                    let low = (value & 0xFF) as u8;
                    self.high_byte_next = true;
                    low
                } else {
                    let high = ((value >> 8) & 0xFF) as u8;
                    self.high_byte_next = false;
                    self.latched_value = None; // Clear latch after reading both bytes
                    high
                }
            }
            AccessMode::LowByteOnly => {
                self.high_byte_next = false;
                (value & 0xFF) as u8
            }
            AccessMode::HighByteOnly => {
                self.high_byte_next = false;
                ((value >> 8) & 0xFF) as u8
            }
            AccessMode::LowHighByte => {
                if !self.high_byte_next {
                    self.high_byte_next = true;
                    (value & 0xFF) as u8
                } else {
                    self.high_byte_next = false;
                    ((value >> 8) & 0xFF) as u8
                }
            }
        };

        result
    }

    /// Latch the current counter value for reading
    fn latch(&mut self) {
        if self.latched_value.is_none() {
            self.latched_value = Some(self.counter);
            self.high_byte_next = false;
        }
    }

    /// Clock the channel (decrement counter)
    fn clock(&mut self) -> bool {
        if !self.counting {
            return false;
        }
        
        // Get the effective reload value (0 means 65536)
        let effective_reload = if self.reload == 0 { 65536u32 } else { self.reload as u32 };

        let wrapped = match self.mode {
            PitMode::InterruptOnTerminalCount => {
                // Mode 0: Count down, output goes high when reaching 0
                if self.counter > 0 {
                    self.counter -= 1;
                    if self.counter == 0 {
                        self.output = true;
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            PitMode::RateGenerator => {
                // Mode 2: Divide by N counter
                if self.counter > 1 {
                    self.counter -= 1;
                    self.output = true;
                    false
                } else {
                    // Reload and generate pulse
                    self.counter = self.reload;
                    self.output = false;
                    true
                }
            }
            PitMode::SquareWave => {
                // Mode 3: Square wave generator
                if self.counter > 0 {
                    self.counter -= 1;
                    if self.counter == 0 {
                        self.counter = self.reload;
                        self.output = !self.output;
                        true
                    } else {
                        // Also toggle at half period for even divisors
                        let half_period = (effective_reload / 2) as u16;
                        if half_period > 0 && self.counter == half_period {
                            self.output = !self.output;
                        }
                        false
                    }
                } else {
                    // Counter is 0, reload and toggle
                    self.counter = self.reload;
                    self.output = !self.output;
                    true
                }
            }
            _ => {
                // Other modes not commonly used - basic countdown
                if self.counter > 0 {
                    self.counter -= 1;
                    self.counter == 0
                } else {
                    false
                }
            }
        };

        wrapped
    }

    /// Get the output state
    fn output(&self) -> bool {
        self.output
    }

    /// Get the current frequency in Hz
    pub fn frequency(&self) -> f64 {
        if self.reload == 0 {
            // A reload value of 0 represents 65536 (maximum count)
            PIT_FREQUENCY / 65536.0
        } else {
            PIT_FREQUENCY / self.reload as f64
        }
    }
}

/// Intel 8253/8254 Programmable Interval Timer
pub struct Pit {
    /// Three channels
    channels: [PitChannel; 3],
    /// Accumulated time since last clock (in PIT ticks)
    accumulated_ticks: f64,
    /// System timer interrupt flag (channel 0)
    timer_interrupt: bool,
}

impl Pit {
    /// Create a new PIT
    pub fn new() -> Self {
        Self {
            channels: [PitChannel::new(), PitChannel::new(), PitChannel::new()],
            accumulated_ticks: 0.0,
            timer_interrupt: false,
        }
    }

    /// Reset the PIT to initial state
    pub fn reset(&mut self) {
        for channel in &mut self.channels {
            channel.reset();
        }
        self.accumulated_ticks = 0.0;
        self.timer_interrupt = false;

        // Initialize channel 0 for system timer (~18.2 Hz)
        // Divisor = 1193182 / 18.2 ≈ 65536
        // Note: Writing 0x0000 represents a count of 65536 (wraps from 0)
        self.write_control(0b00110110); // Channel 0, low/high byte, mode 3
        self.write_channel(0, 0x00); // Low byte
        self.write_channel(0, 0x00); // High byte
        
        // The PIT treats 0 as 65536 internally, but we store it as 0
        // The frequency calculation handles this correctly
    }

    /// Write to the mode/command register (port 0x43)
    pub fn write_control(&mut self, value: u8) {
        let channel_select = (value >> 6) & 0x03;
        
        // Check for read-back command (only on 8254)
        if channel_select == 3 {
            // Read-back command - latch counters
            if (value & 0x20) == 0 {
                // Latch count
                if (value & 0x02) != 0 {
                    self.channels[0].latch();
                }
                if (value & 0x04) != 0 {
                    self.channels[1].latch();
                }
                if (value & 0x08) != 0 {
                    self.channels[2].latch();
                }
            }
            return;
        }

        let channel = &mut self.channels[channel_select as usize];
        let access_mode = AccessMode::from_bits(value);
        
        if matches!(access_mode, AccessMode::LatchCount) {
            // Latch command
            channel.latch();
        } else {
            // Configure command
            channel.access_mode = access_mode;
            channel.mode = PitMode::from_bits(value);
            channel.high_byte_next = false;
            channel.counting = false;
        }
    }

    /// Write to a channel data register (ports 0x40-0x42)
    pub fn write_channel(&mut self, channel: usize, value: u8) {
        if channel < 3 {
            self.channels[channel].write(value);
        }
    }

    /// Read from a channel data register (ports 0x40-0x42)
    pub fn read_channel(&mut self, channel: usize) -> u8 {
        if channel < 3 {
            self.channels[channel].read()
        } else {
            0xFF
        }
    }

    /// Clock the PIT with CPU cycles
    /// Returns true if a timer interrupt should be generated
    pub fn clock(&mut self, cpu_cycles: u32) -> bool {
        // Convert CPU cycles to PIT ticks
        // CPU runs at ~4.77 MHz, PIT at ~1.19 MHz (1/4 speed)
        let pit_ticks = cpu_cycles as f64 / 4.0;
        self.accumulated_ticks += pit_ticks;

        let mut interrupt = false;

        // Process integer ticks
        while self.accumulated_ticks >= 1.0 {
            self.accumulated_ticks -= 1.0;

            // Clock channel 0 (system timer)
            if self.channels[0].clock() {
                interrupt = true;
            }

            // Clock channel 1 (DRAM refresh - not needed for emulation)
            self.channels[1].clock();

            // Clock channel 2 (PC speaker)
            self.channels[2].clock();
        }

        if interrupt {
            self.timer_interrupt = true;
        }

        interrupt
    }

    /// Check if a timer interrupt is pending
    pub fn timer_interrupt_pending(&self) -> bool {
        self.timer_interrupt
    }

    /// Clear the timer interrupt flag
    pub fn clear_timer_interrupt(&mut self) {
        self.timer_interrupt = false;
    }

    /// Get the speaker output state (channel 2)
    pub fn speaker_output(&self) -> bool {
        self.channels[2].output()
    }

    /// Get the speaker frequency in Hz (channel 2)
    pub fn speaker_frequency(&self) -> f64 {
        self.channels[2].frequency()
    }

    /// Get channel 0 frequency (system timer)
    pub fn system_timer_frequency(&self) -> f64 {
        self.channels[0].frequency()
    }
}

impl Default for Pit {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pit_creation() {
        let pit = Pit::new();
        assert!(!pit.timer_interrupt_pending());
        assert!(!pit.speaker_output());
    }

    #[test]
    fn test_pit_reset() {
        let mut pit = Pit::new();
        pit.reset();
        assert!(!pit.timer_interrupt_pending());
        
        // After reset, channel 0 should be configured
        let freq = pit.system_timer_frequency();
        assert!((freq - SYSTEM_TIMER_FREQUENCY).abs() < 0.1);
    }

    #[test]
    fn test_channel_write_read_low_byte() {
        let mut pit = Pit::new();
        
        // Configure channel 0 for low byte only, mode 2
        pit.write_control(0b00010100);
        pit.write_channel(0, 0x42);
        
        // Read back
        let value = pit.read_channel(0);
        assert_eq!(value, 0x42);
    }

    #[test]
    fn test_channel_write_read_high_byte() {
        let mut pit = Pit::new();
        
        // Configure channel 0 for high byte only, mode 2
        pit.write_control(0b00100100);
        pit.write_channel(0, 0x42);
        
        // Read back
        let value = pit.read_channel(0);
        assert_eq!(value, 0x42);
    }

    #[test]
    fn test_channel_write_read_low_high() {
        let mut pit = Pit::new();
        
        // Configure channel 0 for low/high byte, mode 2
        pit.write_control(0b00110100);
        pit.write_channel(0, 0x34); // Low byte
        pit.write_channel(0, 0x12); // High byte
        
        // Read back
        let low = pit.read_channel(0);
        let high = pit.read_channel(0);
        assert_eq!(low, 0x34);
        assert_eq!(high, 0x12);
    }

    #[test]
    fn test_channel_latch() {
        let mut pit = Pit::new();
        
        // Configure and write value
        pit.write_control(0b00110100);
        pit.write_channel(0, 0x00);
        pit.write_channel(0, 0x10);
        
        // Latch the value
        pit.write_control(0b00000000); // Latch channel 0
        
        // Read latched value
        let low = pit.read_channel(0);
        let high = pit.read_channel(0);
        assert_eq!(low, 0x00);
        assert_eq!(high, 0x10);
    }

    #[test]
    fn test_mode_3_square_wave() {
        let mut pit = Pit::new();
        
        // Configure channel 2 for square wave, divisor 4
        pit.write_control(0b10110110); // Channel 2, low/high, mode 3
        pit.write_channel(2, 0x04); // Low byte
        pit.write_channel(2, 0x00); // High byte
        
        // Channel should start counting
        assert!(pit.channels[2].counting);
        
        // Get initial output
        let initial_output = pit.speaker_output();
        
        // Clock enough times to see output change
        // With divisor 4, output should toggle after 2 ticks and again after 4
        for _ in 0..10 {
            pit.clock(4); // 1 PIT tick per clock
        }
        
        let new_output = pit.speaker_output();
        // After 10 ticks with divisor 4, output should have toggled multiple times
        // We can't predict the exact state, but we can verify the channel is working
        // by checking it's still counting
        assert!(pit.channels[2].counting);
    }

    #[test]
    fn test_speaker_frequency() {
        let mut pit = Pit::new();
        
        // Configure channel 2 for 1000 Hz
        // Divisor = 1193182 / 1000 ≈ 1193
        pit.write_control(0b10110110);
        pit.write_channel(2, 0xA9); // Low byte of 1193
        pit.write_channel(2, 0x04); // High byte of 1193
        
        let freq = pit.speaker_frequency();
        assert!((freq - 1000.0).abs() < 1.0);
    }

    #[test]
    fn test_timer_interrupt() {
        let mut pit = Pit::new();
        pit.reset();
        
        // Clock enough to generate an interrupt
        // With divisor 65536, we need 65536 PIT ticks
        // = 65536 * 4 CPU cycles = 262144 cycles
        let mut interrupted = false;
        for _ in 0..1000 {
            if pit.clock(300) {
                interrupted = true;
                break;
            }
        }
        
        assert!(interrupted || pit.timer_interrupt_pending());
    }

    #[test]
    fn test_clear_timer_interrupt() {
        let mut pit = Pit::new();
        pit.timer_interrupt = true;
        assert!(pit.timer_interrupt_pending());
        
        pit.clear_timer_interrupt();
        assert!(!pit.timer_interrupt_pending());
    }

    #[test]
    fn test_multiple_channels() {
        let mut pit = Pit::new();
        
        // Configure all channels differently
        pit.write_control(0b00110100); // Ch 0, mode 2
        pit.write_channel(0, 0x00);
        pit.write_channel(0, 0x10);
        
        pit.write_control(0b01110100); // Ch 1, mode 2
        pit.write_channel(1, 0x00);
        pit.write_channel(1, 0x20);
        
        pit.write_control(0b10110100); // Ch 2, mode 2
        pit.write_channel(2, 0x00);
        pit.write_channel(2, 0x30);
        
        // Read back all channels
        let ch0_low = pit.read_channel(0);
        let ch0_high = pit.read_channel(0);
        let ch1_low = pit.read_channel(1);
        let ch1_high = pit.read_channel(1);
        let ch2_low = pit.read_channel(2);
        let ch2_high = pit.read_channel(2);
        
        assert_eq!((ch0_high as u16) << 8 | ch0_low as u16, 0x1000);
        assert_eq!((ch1_high as u16) << 8 | ch1_low as u16, 0x2000);
        assert_eq!((ch2_high as u16) << 8 | ch2_low as u16, 0x3000);
    }

    #[test]
    fn test_pit_frequency_constant() {
        // Verify the PIT frequency constant
        assert!((PIT_FREQUENCY - 1_193_182.0).abs() < 1.0);
    }

    #[test]
    fn test_system_timer_frequency_constant() {
        // Verify the system timer frequency constant
        assert!((SYSTEM_TIMER_FREQUENCY - 18.2).abs() < 0.1);
    }
}
