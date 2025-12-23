//! Game Boy Timer implementation
//!
//! The Game Boy includes a programmable timer that can generate interrupts
//! at configurable intervals. This is used by many games for timing and
//! synchronization.
//!
//! # Timer Registers
//!
//! - `$FF04 (DIV)`: Divider register - Increments at 16384 Hz, write resets to 0
//! - `$FF05 (TIMA)`: Timer counter - Increments at rate specified by TAC
//! - `$FF06 (TMA)`: Timer modulo - TIMA is loaded with this value on overflow
//! - `$FF07 (TAC)`: Timer control
//!   - Bit 2: Timer enable (0=stop, 1=run)
//!   - Bits 1-0: Clock select
//!     - 00: 4096 Hz (CPU clock / 1024)
//!     - 01: 262144 Hz (CPU clock / 16)
//!     - 10: 65536 Hz (CPU clock / 64)
//!     - 11: 16384 Hz (CPU clock / 256)
//!
//! # Timing
//!
//! The timer is clocked at the CPU speed (4.194304 MHz). The DIV register
//! increments at a fixed rate of 16384 Hz (CPU clock / 256). The TIMA register
//! increments at a configurable rate based on the TAC register.
//!
//! When TIMA overflows (goes from 0xFF to 0x00), it is reloaded with the value
//! in TMA, and a timer interrupt is requested.
//!
//! # Implementation
//!
//! This implementation uses cycle counting to track when registers should
//! increment, avoiding floating-point math and ensuring deterministic behavior.

/// Game Boy Timer
///
/// Implements the DIV, TIMA, TMA, and TAC registers and handles
/// timer interrupts.
pub struct Timer {
    /// Divider register (FF04) - read-only, write resets to 0
    /// Increments at 16384 Hz (every 256 CPU cycles)
    div: u8,

    /// Timer counter (FF05) - increments at rate specified by TAC
    tima: u8,

    /// Timer modulo (FF06) - value loaded into TIMA on overflow
    tma: u8,

    /// Timer control (FF07)
    /// Bit 2: Enable (0=stop, 1=run)
    /// Bits 1-0: Clock select (00=4096Hz, 01=262144Hz, 10=65536Hz, 11=16384Hz)
    tac: u8,

    /// Internal cycle counter for DIV register
    /// DIV increments every 256 cycles
    div_cycles: u32,

    /// Internal cycle counter for TIMA register
    /// Increments based on TAC clock select
    tima_cycles: u32,

    /// Timer interrupt pending flag
    interrupt_pending: bool,
}

impl Timer {
    /// Create a new timer with default values
    pub fn new() -> Self {
        Self {
            div: 0,
            tima: 0,
            tma: 0,
            tac: 0,
            div_cycles: 0,
            tima_cycles: 0,
            interrupt_pending: false,
        }
    }

    /// Reset timer to initial state
    #[allow(dead_code)]
    pub fn reset(&mut self) {
        self.div = 0;
        self.tima = 0;
        self.tma = 0;
        self.tac = 0;
        self.div_cycles = 0;
        self.tima_cycles = 0;
        self.interrupt_pending = false;
    }

    /// Read a timer register
    pub fn read_register(&self, addr: u16) -> u8 {
        match addr {
            0xFF04 => self.div,
            0xFF05 => self.tima,
            0xFF06 => self.tma,
            0xFF07 => self.tac | 0xF8, // Upper 5 bits always read as 1
            _ => 0xFF,
        }
    }

    /// Write to a timer register
    pub fn write_register(&mut self, addr: u16, val: u8) {
        match addr {
            0xFF04 => {
                // Writing any value to DIV resets it to 0
                self.div = 0;
                self.div_cycles = 0;
            }
            0xFF05 => self.tima = val,
            0xFF06 => self.tma = val,
            0xFF07 => self.tac = val & 0x07, // Only lower 3 bits are writable
            _ => {}
        }
    }

    /// Clock the timer by a number of CPU cycles
    ///
    /// Returns true if a timer interrupt should be triggered
    pub fn step(&mut self, cycles: u32) -> bool {
        // Clear interrupt flag
        self.interrupt_pending = false;

        // Update DIV register (increments every 256 cycles)
        self.div_cycles += cycles;
        while self.div_cycles >= 256 {
            self.div = self.div.wrapping_add(1);
            self.div_cycles -= 256;
        }

        // Update TIMA if timer is enabled
        if self.tac & 0x04 != 0 {
            // Get the period based on clock select
            let period = match self.tac & 0x03 {
                0 => 1024, // 4096 Hz
                1 => 16,   // 262144 Hz
                2 => 64,   // 65536 Hz
                3 => 256,  // 16384 Hz
                _ => unreachable!(),
            };

            self.tima_cycles += cycles;
            while self.tima_cycles >= period {
                let (new_tima, overflow) = self.tima.overflowing_add(1);
                self.tima = new_tima;
                self.tima_cycles -= period;

                if overflow {
                    // Timer overflowed - reload with TMA and request interrupt
                    self.tima = self.tma;
                    self.interrupt_pending = true;
                }
            }
        }

        self.interrupt_pending
    }

    /// Check if timer interrupt is pending
    #[allow(dead_code)]
    pub fn interrupt_pending(&self) -> bool {
        self.interrupt_pending
    }
}

impl Default for Timer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timer_creation() {
        let timer = Timer::new();
        assert_eq!(timer.div, 0);
        assert_eq!(timer.tima, 0);
        assert_eq!(timer.tma, 0);
        assert_eq!(timer.tac, 0);
    }

    #[test]
    fn test_div_increment() {
        let mut timer = Timer::new();

        // DIV should increment every 256 cycles
        timer.step(255);
        assert_eq!(timer.div, 0);

        timer.step(1);
        assert_eq!(timer.div, 1);

        timer.step(256);
        assert_eq!(timer.div, 2);
    }

    #[test]
    fn test_div_write_resets() {
        let mut timer = Timer::new();

        // Increment DIV
        timer.step(512);
        assert_eq!(timer.div, 2);

        // Writing to DIV resets it to 0
        timer.write_register(0xFF04, 0xFF);
        assert_eq!(timer.div, 0);

        // Cycle counter should also reset
        timer.step(255);
        assert_eq!(timer.div, 0);
    }

    #[test]
    fn test_tima_disabled() {
        let mut timer = Timer::new();

        // Set TIMA to 0, enable bit not set
        timer.write_register(0xFF05, 0);
        timer.write_register(0xFF07, 0x00); // TAC = 0 (disabled)

        // Clock for many cycles
        timer.step(10000);

        // TIMA should not increment
        assert_eq!(timer.tima, 0);
    }

    #[test]
    fn test_tima_4096hz() {
        let mut timer = Timer::new();

        // Enable timer at 4096 Hz (period = 1024 cycles)
        timer.write_register(0xFF05, 0);
        timer.write_register(0xFF07, 0x04); // Enable, 4096 Hz

        // Clock 1023 cycles
        timer.step(1023);
        assert_eq!(timer.tima, 0);

        // One more cycle should increment
        timer.step(1);
        assert_eq!(timer.tima, 1);

        // Another 1024 cycles
        timer.step(1024);
        assert_eq!(timer.tima, 2);
    }

    #[test]
    fn test_tima_262144hz() {
        let mut timer = Timer::new();

        // Enable timer at 262144 Hz (period = 16 cycles)
        timer.write_register(0xFF05, 0);
        timer.write_register(0xFF07, 0x05); // Enable, 262144 Hz

        timer.step(15);
        assert_eq!(timer.tima, 0);

        timer.step(1);
        assert_eq!(timer.tima, 1);

        timer.step(16);
        assert_eq!(timer.tima, 2);
    }

    #[test]
    fn test_tima_65536hz() {
        let mut timer = Timer::new();

        // Enable timer at 65536 Hz (period = 64 cycles)
        timer.write_register(0xFF05, 0);
        timer.write_register(0xFF07, 0x06); // Enable, 65536 Hz

        timer.step(63);
        assert_eq!(timer.tima, 0);

        timer.step(1);
        assert_eq!(timer.tima, 1);

        timer.step(64);
        assert_eq!(timer.tima, 2);
    }

    #[test]
    fn test_tima_16384hz() {
        let mut timer = Timer::new();

        // Enable timer at 16384 Hz (period = 256 cycles)
        timer.write_register(0xFF05, 0);
        timer.write_register(0xFF07, 0x07); // Enable, 16384 Hz

        timer.step(255);
        assert_eq!(timer.tima, 0);

        timer.step(1);
        assert_eq!(timer.tima, 1);

        timer.step(256);
        assert_eq!(timer.tima, 2);
    }

    #[test]
    fn test_tima_overflow_and_interrupt() {
        let mut timer = Timer::new();

        // Set TIMA to 0xFF, TMA to 0x10
        timer.write_register(0xFF05, 0xFF);
        timer.write_register(0xFF06, 0x10);
        timer.write_register(0xFF07, 0x05); // Enable, 262144 Hz (16 cycles)

        // Clock 16 cycles - should overflow
        let interrupt = timer.step(16);

        // TIMA should be loaded with TMA
        assert_eq!(timer.tima, 0x10);

        // Interrupt should be pending
        assert!(interrupt);
        assert!(timer.interrupt_pending());
    }

    #[test]
    fn test_tima_modulo_reload() {
        let mut timer = Timer::new();

        // Set TIMA to 0xFE, TMA to 0x05
        timer.write_register(0xFF05, 0xFE);
        timer.write_register(0xFF06, 0x05);
        timer.write_register(0xFF07, 0x04); // Enable, 4096 Hz

        // Clock until overflow (2 increments)
        timer.step(1024); // TIMA = 0xFF
        assert_eq!(timer.tima, 0xFF);

        timer.step(1024); // TIMA overflows, loads TMA
        assert_eq!(timer.tima, 0x05);
    }

    #[test]
    fn test_timer_reset() {
        let mut timer = Timer::new();

        // Set some values
        timer.write_register(0xFF05, 0xAA);
        timer.write_register(0xFF06, 0xBB);
        timer.write_register(0xFF07, 0x07);
        timer.step(1000);

        // Reset
        timer.reset();

        // All values should be 0
        assert_eq!(timer.div, 0);
        assert_eq!(timer.tima, 0);
        assert_eq!(timer.tma, 0);
        assert_eq!(timer.tac, 0);
        assert!(!timer.interrupt_pending());
    }
}
