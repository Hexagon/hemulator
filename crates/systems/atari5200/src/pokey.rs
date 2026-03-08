//! POKEY (POtentiometer and KEYboard Integrated Circuit)
//!
//! The POKEY handles:
//! - 4-channel audio synthesis
//! - Keyboard/controller input scanning
//! - Serial I/O (not used in 5200)
//! - 4 programmable timers
//! - Random number generation
//! - Pot (paddle) input
//!
//! # Audio Channels
//! Each channel has:
//! - AUDF (frequency divider, 8-bit)
//! - AUDC (control: volume + distortion)
//! - Channels can be linked (1+2 or 3+4) for 16-bit frequency
//!
//! # Registers ($E800-$E80F)
//! Write:
//! - $E800/$E802/$E804/$E806: AUDF1-4 (frequency)
//! - $E801/$E803/$E805/$E807: AUDC1-4 (control)
//! - $E808: AUDCTL (audio control)
//! - $E809: STIMER (start timers)
//! - $E80A: SKRES (serial port reset - not used)
//! - $E80B: POTGO (start pot scan)
//! - $E80D: SEROUT (serial output - not used)
//! - $E80E: IRQEN (IRQ enable)
//! - $E80F: SKCTL (serial port control)
//!
//! Read:
//! - $E800-$E807: POT0-POT7 (pot/paddle values)
//! - $E808: ALLPOT (pot scan completion status)
//! - $E809: KBCODE (keyboard code)
//! - $E80A: RANDOM (random number)
//! - $E80D: SERIN (serial input - not used)
//! - $E80E: IRQST (IRQ status)
//! - $E80F: SKSTAT (serial port status)

use serde::{Deserialize, Serialize};

/// A single POKEY audio channel
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct AudioChannel {
    frequency: u8,
    control: u8,
    counter: u16,
    output: bool,
}

impl AudioChannel {
    fn volume(&self) -> u8 {
        self.control & 0x0F
    }

    fn volume_only(&self) -> bool {
        self.control & 0x10 != 0
    }

    fn distortion(&self) -> u8 {
        (self.control >> 5) & 0x07
    }
}

/// POKEY chip state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pokey {
    channels: [AudioChannel; 4],
    audctl: u8,

    // Pot/paddle values (0-228)
    pot: [u8; 8],
    pot_counter: [u8; 8],
    pot_scanning: bool,
    allpot: u8,

    // Keyboard
    kbcode: u8,

    // IRQ
    irqen: u8,
    irqst: u8,

    // Serial port control
    skctl: u8,

    // Timer counters (used for IRQ timing)
    timer: [u16; 4],
    timer_irq_pending: [bool; 4],

    // Random number generator (LFSR)
    random: u8,
    lfsr: u32,

    // Audio output accumulator
    sample_accumulator: f32,
    sample_count: u32,

    // Internal clock divider
    clock_counter: u32,
}

impl Default for Pokey {
    fn default() -> Self {
        Self::new()
    }
}

impl Pokey {
    pub fn new() -> Self {
        Self {
            channels: Default::default(),
            audctl: 0,
            pot: [228; 8], // Center position
            pot_counter: [0; 8],
            pot_scanning: false,
            allpot: 0xFF, // All pots "not done"
            kbcode: 0,
            irqen: 0,
            irqst: 0xFF, // All IRQ bits clear (active low)
            skctl: 0,
            timer: [0; 4],
            timer_irq_pending: [false; 4],
            random: 0xFF,
            lfsr: 0x1FFFF, // 17-bit LFSR all ones
            sample_accumulator: 0.0,
            sample_count: 0,
            clock_counter: 0,
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Read a POKEY register
    pub fn read(&self, addr: u16) -> u8 {
        match addr & 0x0F {
            0x00..=0x07 => self.pot[(addr & 0x07) as usize],
            0x08 => self.allpot,
            0x09 => self.kbcode,
            0x0A => self.random,
            0x0D => 0xFF, // SERIN (not used)
            0x0E => self.irqst,
            0x0F => {
                // SKSTAT
                let mut status = 0xFF;
                // Bit 2: keyboard overrun (clear)
                status &= !0x04;
                // Bit 3: serial frame error (clear)
                status &= !0x08;
                status
            }
            _ => 0xFF,
        }
    }

    /// Write a POKEY register
    pub fn write(&mut self, addr: u16, val: u8) {
        match addr & 0x0F {
            0x00 => self.channels[0].frequency = val,
            0x01 => self.channels[0].control = val,
            0x02 => self.channels[1].frequency = val,
            0x03 => self.channels[1].control = val,
            0x04 => self.channels[2].frequency = val,
            0x05 => self.channels[2].control = val,
            0x06 => self.channels[3].frequency = val,
            0x07 => self.channels[3].control = val,
            0x08 => self.audctl = val,
            0x09 => {
                // STIMER - reset all audio counters
                for ch in &mut self.channels {
                    ch.counter = 0;
                }
            }
            0x0A => { /* SKRES - serial reset (not used) */ }
            0x0B => {
                // POTGO - start pot scan
                self.pot_scanning = true;
                self.allpot = 0xFF;
                for counter in &mut self.pot_counter {
                    *counter = 0;
                }
            }
            0x0D => { /* SEROUT - serial output (not used) */ }
            0x0E => {
                self.irqen = val;
                // Update IRQ status based on new enable mask
                self.irqst |= !val; // Clear disabled IRQ bits
            }
            0x0F => self.skctl = val,
            _ => {}
        }
    }

    /// Clock POKEY for one CPU cycle (~1.79 MHz)
    pub fn clock(&mut self) {
        self.clock_counter += 1;

        // Update LFSR (random number generator)
        if self.clock_counter & 0x03 == 0 {
            let bit = ((self.lfsr >> 16) ^ (self.lfsr >> 11)) & 1;
            self.lfsr = ((self.lfsr << 1) | bit) & 0x1FFFF;
            self.random = (self.lfsr & 0xFF) as u8;
        }

        // Update pot scanning
        if self.pot_scanning {
            let mut all_done = true;
            for i in 0..8 {
                if self.allpot & (1 << i) != 0 {
                    self.pot_counter[i] = self.pot_counter[i].wrapping_add(1);
                    if self.pot_counter[i] >= self.pot[i] {
                        self.allpot &= !(1 << i); // Mark as done
                    } else {
                        all_done = false;
                    }
                }
            }
            if all_done {
                self.pot_scanning = false;
            }
        }

        // Audio channel clocking
        // Base clock divider: 64kHz (divide by 28) or 15kHz (divide by 114)
        let use_15khz = self.audctl & 0x01 != 0;
        let divider = if use_15khz { 114 } else { 28 };

        if self.clock_counter.is_multiple_of(divider) {
            self.clock_channels();
        }

        // Accumulate audio sample
        let sample = self.mix_channels();
        self.sample_accumulator += sample;
        self.sample_count += 1;
    }

    /// Clock all audio channels
    fn clock_channels(&mut self) {
        // Check for channel linking
        let ch12_linked = self.audctl & 0x10 != 0;
        let ch34_linked = self.audctl & 0x08 != 0;

        if ch12_linked {
            // Channels 1+2 linked as 16-bit
            let freq = (self.channels[1].frequency as u16) << 8 | self.channels[0].frequency as u16;
            let counter = self.channels[0].counter.wrapping_add(1);
            if counter > freq {
                self.channels[0].counter = 0;
                self.channels[0].output = !self.channels[0].output;
                self.channels[1].output = self.channels[0].output;
            } else {
                self.channels[0].counter = counter;
            }
        } else {
            // Independent channels
            for i in 0..2 {
                let counter = self.channels[i].counter.wrapping_add(1);
                if counter > self.channels[i].frequency as u16 {
                    self.channels[i].counter = 0;
                    self.channels[i].output = !self.channels[i].output;
                } else {
                    self.channels[i].counter = counter;
                }
            }
        }

        if ch34_linked {
            let freq = (self.channels[3].frequency as u16) << 8 | self.channels[2].frequency as u16;
            let counter = self.channels[2].counter.wrapping_add(1);
            if counter > freq {
                self.channels[2].counter = 0;
                self.channels[2].output = !self.channels[2].output;
                self.channels[3].output = self.channels[2].output;
            } else {
                self.channels[2].counter = counter;
            }
        } else {
            for i in 2..4 {
                let counter = self.channels[i].counter.wrapping_add(1);
                if counter > self.channels[i].frequency as u16 {
                    self.channels[i].counter = 0;
                    self.channels[i].output = !self.channels[i].output;
                } else {
                    self.channels[i].counter = counter;
                }
            }
        }
    }

    /// Mix all channels to a single sample
    fn mix_channels(&self) -> f32 {
        let mut sum = 0.0f32;
        for ch in &self.channels {
            let vol = ch.volume() as f32 / 15.0;
            if ch.volume_only() {
                // Volume-only mode - DC output at volume level
                sum += vol;
            } else {
                // Square/noise wave based on distortion
                let output = match ch.distortion() {
                    0 => {
                        // 5+17 bit poly (noise)
                        if ch.output {
                            vol
                        } else {
                            -vol
                        }
                    }
                    2 | 6 => {
                        // 5 bit poly (harsh noise)
                        if ch.output {
                            vol
                        } else {
                            -vol
                        }
                    }
                    4 => {
                        // 17 bit poly (white noise)
                        if ch.output {
                            vol
                        } else {
                            -vol
                        }
                    }
                    // Pure tone (all other distortion values with bit 0 set)
                    _ => {
                        if ch.output {
                            vol
                        } else {
                            -vol
                        }
                    }
                };
                sum += output;
            }
        }
        sum / 4.0 // Normalize by number of channels
    }

    /// Generate audio samples for output
    pub fn generate_audio_samples(&mut self, count: usize) -> Vec<i16> {
        let mut samples = Vec::with_capacity(count);
        let cpu_cycles_per_sample = 1789773.0 / 44100.0; // ~40.6 cycles per sample

        for _ in 0..count {
            // Average the accumulated samples
            let sample = if self.sample_count > 0 {
                self.sample_accumulator / self.sample_count as f32
            } else {
                0.0
            };

            // Convert to i16 range
            let output = (sample * 8000.0).clamp(-32767.0, 32767.0) as i16;
            samples.push(output);

            self.sample_accumulator = 0.0;
            self.sample_count = 0;

            // If we haven't clocked enough, clock the POKEY to catch up
            let target_clocks = cpu_cycles_per_sample as u32;
            if self.sample_count == 0 {
                for _ in 0..target_clocks {
                    self.clock();
                }
            }
        }

        samples
    }

    /// Set pot/paddle value (0-228)
    pub fn set_pot(&mut self, index: usize, value: u8) {
        if index < 8 {
            self.pot[index] = value.min(228);
        }
    }

    /// Set keyboard code
    pub fn set_kbcode(&mut self, code: u8) {
        self.kbcode = code;
        // Set keyboard IRQ if enabled
        if self.irqen & 0x40 != 0 {
            self.irqst &= !0x40; // Active low
        }
    }

    /// Check if any IRQ is pending
    pub fn irq_pending(&self) -> bool {
        // IRQ is active when IRQST bit is low AND IRQEN bit is high
        (self.irqst & self.irqen) != self.irqen
    }

    /// Get IRQST
    pub fn irqst(&self) -> u8 {
        self.irqst
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_registers() {
        let mut pokey = Pokey::new();
        pokey.write(0xE800, 100); // AUDF1
        pokey.write(0xE801, 0xBF); // AUDC1
        assert_eq!(pokey.channels[0].frequency, 100);
        assert_eq!(pokey.channels[0].control, 0xBF);
        assert_eq!(pokey.channels[0].volume(), 0x0F);
        assert!(pokey.channels[0].volume_only()); // Bit 4 set
    }

    #[test]
    fn test_random_number() {
        let mut pokey = Pokey::new();
        let initial = pokey.read(0x0A);
        // Clock a bunch of times
        for _ in 0..1000 {
            pokey.clock();
        }
        let after = pokey.read(0x0A);
        // Random should have changed
        assert_ne!(initial, after);
    }

    #[test]
    fn test_pot_scanning() {
        let mut pokey = Pokey::new();
        pokey.set_pot(0, 100);
        pokey.write(0x0B, 0); // POTGO
        assert!(pokey.pot_scanning);
        assert_eq!(pokey.allpot, 0xFF);
    }

    #[test]
    fn test_irq() {
        let mut pokey = Pokey::new();
        pokey.write(0x0E, 0x40); // Enable keyboard IRQ
        assert!(!pokey.irq_pending());
        pokey.set_kbcode(0x42);
        assert!(pokey.irq_pending());
    }
}
