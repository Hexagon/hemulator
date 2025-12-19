//! RP2A03 audio chip (NES NTSC).
//!
//! The RP2A03 is the main chip in NTSC NES/Famicom consoles, containing:
//! - MOS 6502 CPU (implemented separately in core)
//! - 2 Pulse channels
//! - 1 Triangle channel
//! - 1 Noise channel
//! - 1 DMC (Delta Modulation Channel)
//!
//! This module implements just the audio portion.

use super::{
    audio_chip::AudioChip, NoiseChannel, PulseChannel, TimingMode, TriangleChannel, LENGTH_TABLE,
};

/// RP2A03 APU (NTSC variant).
///
/// The audio processing unit in the NTSC NES console.
#[derive(Debug)]
pub struct Rp2a03Apu {
    pub pulse1: PulseChannel,
    pub pulse2: PulseChannel,
    pub triangle: TriangleChannel,
    pub noise: NoiseChannel,
    timing: TimingMode,
}

impl Rp2a03Apu {
    /// Create a new RP2A03 APU (NTSC timing)
    pub fn new() -> Self {
        Self {
            pulse1: PulseChannel::new(),
            pulse2: PulseChannel::new(),
            triangle: TriangleChannel::new(),
            noise: NoiseChannel::new(),
            timing: TimingMode::Ntsc,
        }
    }
}

impl AudioChip for Rp2a03Apu {
    fn write_register(&mut self, addr: u16, val: u8) {
        match addr {
            // Pulse 1 registers ($4000-$4003)
            0x4000 => {
                self.pulse1.duty = (val >> 6) & 3;
                self.pulse1.length_counter_halt = (val & 0x20) != 0;
                self.pulse1.constant_volume = (val & 0x10) != 0;
                self.pulse1.envelope = val & 15;
            }
            0x4001 => {
                // Sweep unit (not yet implemented)
            }
            0x4002 => {
                let low = val as u16;
                let high = (self.pulse1.timer_reload >> 8) & 0x07;
                self.pulse1.set_timer((high << 8) | low);
            }
            0x4003 => {
                let high = (val & 0x07) as u16;
                let low = self.pulse1.timer_reload & 0xFF;
                self.pulse1.set_timer((high << 8) | low);
                self.pulse1.reset_phase();
                let len_index = (val >> 3) & 0x1F;
                if self.pulse1.enabled {
                    self.pulse1.length_counter = LENGTH_TABLE[len_index as usize];
                }
            }

            // Pulse 2 registers ($4004-$4007)
            0x4004 => {
                self.pulse2.duty = (val >> 6) & 3;
                self.pulse2.length_counter_halt = (val & 0x20) != 0;
                self.pulse2.constant_volume = (val & 0x10) != 0;
                self.pulse2.envelope = val & 15;
            }
            0x4005 => {
                // Sweep unit (not yet implemented)
            }
            0x4006 => {
                let low = val as u16;
                let high = (self.pulse2.timer_reload >> 8) & 0x07;
                self.pulse2.set_timer((high << 8) | low);
            }
            0x4007 => {
                let high = (val & 0x07) as u16;
                let low = self.pulse2.timer_reload & 0xFF;
                self.pulse2.set_timer((high << 8) | low);
                self.pulse2.reset_phase();
                let len_index = (val >> 3) & 0x1F;
                if self.pulse2.enabled {
                    self.pulse2.length_counter = LENGTH_TABLE[len_index as usize];
                }
            }

            // Triangle registers ($4008-$400B)
            0x4008 => {
                self.triangle.control_flag = (val & 0x80) != 0;
                self.triangle.linear_counter_reload = val & 0x7F;
            }
            0x4009 => {
                // Unused
            }
            0x400A => {
                let low = val as u16;
                let high = (self.triangle.timer_reload >> 8) & 0x07;
                self.triangle.set_timer((high << 8) | low);
            }
            0x400B => {
                let high = (val & 0x07) as u16;
                let low = self.triangle.timer_reload & 0xFF;
                self.triangle.set_timer((high << 8) | low);
                self.triangle.linear_counter_reload_flag = true;
                let len_index = (val >> 3) & 0x1F;
                if self.triangle.enabled {
                    self.triangle.length_counter = LENGTH_TABLE[len_index as usize];
                }
            }

            // Noise registers ($400C-$400F)
            0x400C => {
                self.noise.length_counter_halt = (val & 0x20) != 0;
                self.noise.constant_volume = (val & 0x10) != 0;
                self.noise.envelope = val & 0x0F;
            }
            0x400D => {
                // Unused
            }
            0x400E => {
                self.noise.mode = (val & 0x80) != 0;
                self.noise.set_period(val & 0x0F);
            }
            0x400F => {
                let len_index = (val >> 3) & 0x1F;
                if self.noise.enabled {
                    self.noise.length_counter = LENGTH_TABLE[len_index as usize];
                }
            }

            // Status register ($4015)
            0x4015 => {
                self.pulse1.enabled = (val & 0x01) != 0;
                self.pulse2.enabled = (val & 0x02) != 0;
                self.triangle.enabled = (val & 0x04) != 0;
                self.noise.enabled = (val & 0x08) != 0;
                // DMC enable at bit 4 (not yet implemented)
            }

            // Frame counter ($4017) - not yet implemented
            0x4017 => {
                // Frame counter control
            }

            _ => {}
        }
    }

    fn clock(&mut self) -> i16 {
        let p1 = self.pulse1.clock();
        let p2 = self.pulse2.clock();
        let tri = self.triangle.clock();
        let noise = self.noise.clock();

        // Simple mixing for now (should use non-linear mixing)
        let mixed = (p1 as i32 + p2 as i32 + tri as i32 + noise as i32) / 4;
        mixed.clamp(-32768, 32767) as i16
    }

    fn timing(&self) -> TimingMode {
        self.timing
    }

    fn reset(&mut self) {
        self.pulse1 = PulseChannel::new();
        self.pulse2 = PulseChannel::new();
        self.triangle = TriangleChannel::new();
        self.noise = NoiseChannel::new();
    }
}

impl Default for Rp2a03Apu {
    fn default() -> Self {
        Self::new()
    }
}
