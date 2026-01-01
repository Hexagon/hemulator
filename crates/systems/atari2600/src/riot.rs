//! RIOT (6532) - RAM, I/O, and Timer chip for Atari 2600
//!
//! The RIOT chip provides RAM, I/O ports, and timing functions for the Atari 2600.
//!
//! # Components
//!
//! ## RAM
//! - **Size**: 128 bytes (all the RAM the Atari 2600 has!)
//! - **Address Range**: $80-$FF in RIOT address space
//! - **Mirroring**: Also accessible at $00-$7F and $100-$17F in the 6507's address space
//!
//! This is the **only** read/write memory in the system. Games must be extremely frugal with
//! RAM usage, often reusing variables and packing multiple flags into single bytes.
//!
//! ## I/O Ports
//!
//! The RIOT provides two 8-bit I/O ports with programmable direction registers:
//!
//! ### Port A (SWCHA / SWACNT)
//! Used for **joystick and paddle controllers**:
//!
//! **SWCHA bits** (active low - 0 = pressed):
//! - Bit 0: Player 0 Up
//! - Bit 1: Player 0 Down  
//! - Bit 2: Player 0 Left
//! - Bit 3: Player 0 Right
//! - Bit 4: Player 1 Up
//! - Bit 5: Player 1 Down
//! - Bit 6: Player 1 Left
//! - Bit 7: Player 1 Right
//!
//! **SWACNT**: Data direction register (0 = input, 1 = output)
//!
//! ### Port B (SWCHB / SWBCNT)
//! Used for **console switches**:
//!
//! **SWCHB bits** (active low - 0 = pressed/selected):
//! - Bit 0: Reset button
//! - Bit 1: Select button
//! - Bit 3: Color/BW switch (0 = BW, 1 = Color)
//! - Bit 6: Left difficulty (0 = A/Pro, 1 = B/Amateur)
//! - Bit 7: Right difficulty (0 = A/Pro, 1 = B/Amateur)
//!
//! **SWBCNT**: Data direction register
//!
//! ## Programmable Interval Timer
//!
//! The timer is a countdown timer that can trigger interrupts:
//!
//! **Features**:
//! - 8-bit counter that counts down to 0
//! - Four clock intervals: 1, 8, 64, or 1024 CPU clocks per decrement
//! - After reaching 0, continues counting down at 1 clock/decrement
//! - Sets underflow flag (TIMINT) when reaching 0
//!
//! **Registers**:
//! - **TIM1T** ($294): Set timer with 1 clock interval
//! - **TIM8T** ($295): Set timer with 8 clock interval
//! - **TIM64T** ($296): Set timer with 64 clock interval
//! - **T1024T** ($297): Set timer with 1024 clock interval
//! - **INTIM** ($284): Read current timer value
//! - **TIMINT** ($285): Read timer underflow flag (bit 7), clears flag on read
//!
//! **Important**: Reading TIMINT/INSTAT clears the underflow flag as a hardware side effect.
//! This allows games to detect when the timer has expired between checks.
//!
//! **Usage**: Games use the timer for frame synchronization. A common pattern is:
//! 1. Set timer at start of frame (e.g., `STA TIM64T`)
//! 2. Do frame processing
//! 3. Wait for timer to reach 0 (`BIT TIMINT` loop)
//! 4. Start next frame
//!
//! This ensures consistent frame timing regardless of how much work the CPU does each frame.
//!
//! # Memory Map (within RIOT)
//!
//! ```text
//! $00-$7F:   RAM (128 bytes, mirrored)
//! $80-$FF:   RAM (128 bytes)
//! $100-$17F: RAM (128 bytes, mirrored)
//! $280:      SWCHA (Port A data)
//! $281:      SWACNT (Port A direction)
//! $282:      SWCHB (Port B data)
//! $283:      SWBCNT (Port B direction)
//! $284:      INTIM (Read timer)
//! $285:      TIMINT (Read timer status)
//! $294:      TIM1T (Set timer, 1 clock interval)
//! $295:      TIM8T (Set timer, 8 clock interval)
//! $296:      TIM64T (Set timer, 64 clock interval)
//! $297:      T1024T (Set timer, 1024 clock interval)
//! ```
//!
//! # Implementation Notes
//!
//! This implementation provides full RIOT functionality including:
//! - ✅ Complete 128-byte RAM with proper mirroring
//! - ✅ Programmable interval timer with all 4 clock rates
//! - ✅ Timer underflow detection
//! - ✅ I/O port registers (with helper methods for setting controller state)
//! - ✅ Data direction registers (stored but not enforced)
//!
//! Controller input is managed through public API methods (`set_joystick`, `set_console_switch`)
//! rather than directly manipulating the I/O port registers.

use serde::{Deserialize, Serialize};
use std::cell::Cell;

mod serde_arrays {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(arr: &[u8; 128], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        arr.as_slice().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<[u8; 128], D::Error>
    where
        D: Deserializer<'de>,
    {
        let vec = Vec::<u8>::deserialize(deserializer)?;
        let mut arr = [0u8; 128];
        arr.copy_from_slice(&vec);
        Ok(arr)
    }
}

mod serde_cell_bool {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::cell::Cell;

    pub fn serialize<S>(cell: &Cell<bool>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        cell.get().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Cell<bool>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let val = bool::deserialize(deserializer)?;
        Ok(Cell::new(val))
    }
}

/// RIOT chip state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Riot {
    /// 128 bytes of RAM (0x80-0xFF in RIOT address space)
    #[serde(with = "serde_arrays")]
    ram: [u8; 128],

    /// Timer value (counts down to 0)
    timer: u8,

    /// Timer interval (1, 8, 64, or 1024)
    timer_interval: u16,

    /// Cycles until next timer decrement
    timer_cycles: u16,

    /// Timer interrupt flag (cleared when TIMINT/INSTAT is read)
    #[serde(with = "serde_cell_bool")]
    timer_underflow: Cell<bool>,

    /// Port A data direction register (0 = input, 1 = output)
    swcha_ddr: u8,

    /// Port A data (joystick inputs)
    swcha: u8,

    /// Port B data direction register
    swchb_ddr: u8,

    /// Port B data (console switches)
    swchb: u8,
}

impl Default for Riot {
    fn default() -> Self {
        Self::new()
    }
}

impl Riot {
    /// Create a new RIOT chip
    pub fn new() -> Self {
        Self {
            ram: [0; 128],
            timer: 0,
            timer_interval: 1,
            timer_cycles: 0,
            timer_underflow: Cell::new(false),
            swcha_ddr: 0,
            swcha: 0xFF, // Joysticks unpressed
            swchb_ddr: 0,
            swchb: 0xFF, // Console switches unpressed/high (active low)
        }
    }

    /// Reset RIOT to power-on state
    pub fn reset(&mut self) {
        self.ram = [0; 128];
        self.timer = 0;
        self.timer_interval = 1;
        self.timer_cycles = 0;
        self.timer_underflow.set(false);
        self.swcha_ddr = 0;
        self.swcha = 0xFF;
        self.swchb_ddr = 0;
        self.swchb = 0xFF;
    }

    /// Read from RIOT  address space
    /// Addr is the Atari 2600 system address (not masked)
    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            // RAM ($80-$FF and mirrors at $00-$7F, $100-$17F)
            0x0000..=0x007F => self.ram[addr as usize],
            0x0080..=0x00FF => self.ram[(addr & 0x7F) as usize],
            0x0100..=0x017F => self.ram[(addr & 0x7F) as usize],
            // Stack mirror ($180-$1FF)
            0x0180..=0x01FF => self.ram[(addr & 0x7F) as usize],

            // I/O and timer ($280-$297, mirrored every 32 bytes)
            0x0280..=0x029F => {
                match addr & 0x0F {
                    0x00 => self.swcha,
                    0x01 => self.swcha_ddr,
                    0x02 => self.swchb,
                    0x03 => self.swchb_ddr,
                    0x04 | 0x06 => self.timer, // INTIM
                    0x05 | 0x07 => {
                        // TIMINT/INSTAT - reading clears the underflow flag
                        let flag = self.timer_underflow.get();
                        self.timer_underflow.set(false);
                        if flag {
                            0x80
                        } else {
                            0x00
                        }
                    }
                    _ => 0,
                }
            }

            _ => 0,
        }
    }

    /// Write to RIOT address space
    /// Addr is the Atari 2600 system address (not masked)
    pub fn write(&mut self, addr: u16, val: u8) {
        match addr {
            // RAM ($80-$FF and mirrors at $00-$7F, $100-$17F)
            0x0000..=0x007F => self.ram[addr as usize] = val,
            0x0080..=0x00FF => self.ram[(addr & 0x7F) as usize] = val,
            0x0100..=0x017F => self.ram[(addr & 0x7F) as usize] = val,
            // Stack mirror ($180-$1FF)
            0x0180..=0x01FF => self.ram[(addr & 0x7F) as usize] = val,

            // I/O and timer ($280-$29F, mirrored every 32 bytes)
            0x0280..=0x029F => {
                match addr & 0x1F {
                    0x00 => self.swcha = val,
                    0x01 => self.swcha_ddr = val,
                    0x02 => self.swchb = val,
                    0x03 => self.swchb_ddr = val,
                    0x14 => {
                        // TIM1T
                        self.timer = val;
                        self.timer_interval = 1;
                        self.timer_cycles = 0;
                        self.timer_underflow.set(false);
                    }
                    0x15 => {
                        // TIM8T
                        self.timer = val;
                        self.timer_interval = 8;
                        self.timer_cycles = 0;
                        self.timer_underflow.set(false);
                    }
                    0x16 => {
                        // TIM64T
                        // eprintln!("RIOT: TIM64T write val={} interval=64", val);
                        self.timer = val;
                        self.timer_interval = 64;
                        self.timer_cycles = 0;
                        self.timer_underflow.set(false);
                    }
                    0x17 => {
                        // T1024T
                        self.timer = val;
                        self.timer_interval = 1024;
                        self.timer_cycles = 0;
                        self.timer_underflow.set(false);
                    }
                    _ => {}
                }
            }

            _ => {}
        }
    }

    /// Clock the timer
    pub fn clock(&mut self, cycles: u16) {
        for _ in 0..cycles {
            self.timer_cycles += 1;
            if self.timer_cycles >= self.timer_interval {
                self.timer_cycles = 0;

                // Decrement timer
                if self.timer == 0 {
                    // Timer at 0, wrap around
                    // eprintln!("RIOT: Timer underflow! interval=1");
                    self.timer_underflow.set(true);
                    self.timer_interval = 1; // After underflow, decrement every cycle
                    self.timer = 0xFF;
                } else {
                    self.timer = self.timer.wrapping_sub(1);
                    // Check if we just hit 0
                    if self.timer == 0 {
                        // eprintln!("RIOT: Timer hit 0! interval=1");
                        self.timer_underflow.set(true);
                        self.timer_interval = 1;
                        // self.timer = 0xFF; // REMOVED: Don't wrap immediately, stay at 0 for one interval
                    }
                }
            }
        }
    }

    /// Set joystick state (Port A)
    /// Bits: P0 Right, P0 Left, P0 Down, P0 Up, P1 Right, P1 Left, P1 Down, P1 Up
    /// 0 = pressed, 1 = not pressed (active low)
    #[allow(dead_code)]
    pub fn set_joystick(&mut self, player: u8, direction: u8, pressed: bool) {
        let bit = if player == 0 {
            direction // Player 0: bits 0-3
        } else {
            direction + 4 // Player 1: bits 4-7
        };

        if pressed {
            self.swcha &= !(1 << bit);
        } else {
            self.swcha |= 1 << bit;
        }
    }

    /// Set console switch state (Port B)
    /// Bit 0: Reset (0 = pressed)
    /// Bit 1: Select (0 = pressed)
    /// Bit 3: BW/Color (0 = BW, 1 = Color)
    /// Bit 6: Left difficulty (0 = A/Pro, 1 = B/Amateur)
    /// Bit 7: Right difficulty (0 = A/Pro, 1 = B/Amateur)
    #[allow(dead_code)]
    pub fn set_console_switch(&mut self, bit: u8, pressed: bool) {
        if pressed {
            self.swchb &= !(1 << bit);
        } else {
            self.swchb |= 1 << bit;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_riot_ram() {
        let mut riot = Riot::new();

        // Test RAM write/read at $80
        riot.write(0x0080, 0x42);
        assert_eq!(riot.read(0x0080), 0x42);

        // Test RAM mirroring at $00
        riot.write(0x0000, 0x12);
        assert_eq!(riot.read(0x0000), 0x12);
    }

    #[test]
    fn test_riot_timer() {
        let mut riot = Riot::new();

        // Set timer to 10 with 1 clock interval (TIM1T is at $294)
        riot.write(0x0294, 10);
        assert_eq!(riot.read(0x0284), 10); // INTIM is at $284

        // Clock once
        riot.clock(1);
        assert_eq!(riot.read(0x0284), 9);

        // Clock until 0
        riot.clock(9);
        assert_eq!(riot.read(0x0284), 0); // Should be 0
        assert_eq!(riot.read(0x0285) & 0x80, 0x80); // Underflow flag set at $285

        // Clock one more time
        riot.clock(1);
        assert_eq!(riot.read(0x0284), 0xFF); // Should wrap to 0xFF
    }

    #[test]
    fn test_riot_timer_interrupt_flag_clears_on_read() {
        let mut riot = Riot::new();

        // Set timer to 2 with 1 clock interval
        riot.write(0x0294, 2);

        // Initially, underflow flag should be clear
        assert_eq!(riot.read(0x0285) & 0x80, 0x00);

        // Clock until timer expires
        riot.clock(2);

        // Underflow flag should now be set
        assert_eq!(riot.read(0x0285) & 0x80, 0x80);

        // Reading TIMINT should clear the flag
        // Second read should return flag cleared
        assert_eq!(riot.read(0x0285) & 0x80, 0x00);

        // Verify flag stays cleared on subsequent reads
        assert_eq!(riot.read(0x0285) & 0x80, 0x00);

        // Also test with INSTAT mirror at $287
        riot.write(0x0294, 1);
        riot.clock(1);
        assert_eq!(riot.read(0x0287) & 0x80, 0x80); // Flag set
        assert_eq!(riot.read(0x0287) & 0x80, 0x00); // Flag cleared by read
    }

    #[test]
    fn test_riot_timer_intervals() {
        let mut riot = Riot::new();

        // Test TIM8T at $295
        riot.write(0x0295, 5);
        riot.clock(8);
        assert_eq!(riot.read(0x0284), 4);

        // Test TIM64T at $296
        riot.write(0x0296, 5);
        riot.clock(64);
        assert_eq!(riot.read(0x0284), 4);

        // Test T1024T at $297
        riot.write(0x0297, 5);
        riot.clock(1024);
        assert_eq!(riot.read(0x0284), 4);
    }

    #[test]
    fn test_riot_joystick() {
        let mut riot = Riot::new();

        // Initially all joysticks unpressed (all bits high)
        assert_eq!(riot.read(0x0280), 0xFF); // SWCHA at $280

        // Press Player 0 Up (bit 0)
        riot.set_joystick(0, 0, true);
        assert_eq!(riot.read(0x0280) & 0x01, 0x00);

        // Press Player 1 Down (bit 6)
        riot.set_joystick(1, 2, true);
        assert_eq!(riot.read(0x0280) & 0x40, 0x00);
    }

    #[test]
    fn test_riot_console_switches() {
        let mut riot = Riot::new();

        // Press reset switch (bit 0) - SWCHB at $282
        riot.set_console_switch(0, true);
        assert_eq!(riot.read(0x0282) & 0x01, 0x00);

        // Press select switch (bit 1)
        riot.set_console_switch(1, true);
        assert_eq!(riot.read(0x0282) & 0x02, 0x00);
    }

    #[test]
    fn test_riot_reset() {
        let mut riot = Riot::new();

        riot.write(0x0080, 0x42);
        riot.write(0x0294, 10);

        riot.reset();

        assert_eq!(riot.read(0x0080), 0x00);
        assert_eq!(riot.read(0x0284), 0x00);
    }
}
