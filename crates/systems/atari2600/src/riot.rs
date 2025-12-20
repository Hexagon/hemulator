//! RIOT (6532) - RAM, I/O, and Timer chip for Atari 2600
//!
//! The RIOT chip provides:
//! - 128 bytes of RAM
//! - Two 8-bit I/O ports (for joystick/paddle controllers and console switches)
//! - Programmable interval timer

use serde::{Deserialize, Serialize};

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

    /// Timer interrupt flag
    timer_underflow: bool,

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
            timer_underflow: false,
            swcha_ddr: 0,
            swcha: 0xFF, // Joysticks unpressed
            swchb_ddr: 0,
            swchb: 0x0B, // Console switches (default: no switches pressed)
        }
    }

    /// Reset RIOT to power-on state
    pub fn reset(&mut self) {
        self.ram = [0; 128];
        self.timer = 0;
        self.timer_interval = 1;
        self.timer_cycles = 0;
        self.timer_underflow = false;
        self.swcha_ddr = 0;
        self.swcha = 0xFF;
        self.swchb_ddr = 0;
        self.swchb = 0x0B;
    }

    /// Read from RIOT  address space
    /// Addr is the Atari 2600 system address (not masked)
    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            // RAM ($80-$FF and mirrors at $00-$7F, $100-$17F)
            0x0000..=0x007F => self.ram[addr as usize],
            0x0080..=0x00FF => self.ram[(addr & 0x7F) as usize],
            0x0100..=0x017F => self.ram[(addr & 0x7F) as usize],

            // I/O and timer ($280-$297, mirrored every 32 bytes)
            0x0280..=0x029F => {
                match addr & 0x0F {
                    0x00 => self.swcha,
                    0x01 => self.swcha_ddr,
                    0x02 => self.swchb,
                    0x03 => self.swchb_ddr,
                    0x04 | 0x06 => self.timer, // INTIM
                    0x05 | 0x07 => {
                        // TIMINT/INSTAT
                        if self.timer_underflow {
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
                        self.timer_underflow = false;
                    }
                    0x15 => {
                        // TIM8T
                        self.timer = val;
                        self.timer_interval = 8;
                        self.timer_cycles = 0;
                        self.timer_underflow = false;
                    }
                    0x16 => {
                        // TIM64T
                        self.timer = val;
                        self.timer_interval = 64;
                        self.timer_cycles = 0;
                        self.timer_underflow = false;
                    }
                    0x17 => {
                        // T1024T
                        self.timer = val;
                        self.timer_interval = 1024;
                        self.timer_cycles = 0;
                        self.timer_underflow = false;
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
                    self.timer_underflow = true;
                    self.timer_interval = 1; // After underflow, decrement every cycle
                    self.timer = 0xFF;
                } else {
                    self.timer = self.timer.wrapping_sub(1);
                    // Check if we just hit 0
                    if self.timer == 0 {
                        self.timer_underflow = true;
                        self.timer_interval = 1;
                        self.timer = 0xFF;
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

        // Clock until underflow
        riot.clock(9);
        assert_eq!(riot.read(0x0284), 0xFF); // Should wrap to 0xFF
        assert_eq!(riot.read(0x0285) & 0x80, 0x80); // Underflow flag set at $285
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
