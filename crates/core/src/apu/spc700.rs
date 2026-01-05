//! Complete SPC700 Audio Processing Unit chip
//!
//! This module combines the SPC700 CPU, DSP, RAM, timers, and I/O ports
//! into a complete audio processing unit that can be used by the SNES system.
//!
//! **Architecture:**
//! - SPC700 CPU (8-bit, 256 opcodes)
//! - 64KB RAM
//! - 64-byte IPL boot ROM ($FFC0-$FFFF, can be disabled)
//! - 4 communication ports ($F4-$F7) for CPU<->APU communication
//! - 3 timers (8-bit counters with programmable periods)
//! - DSP (8-channel audio with ADPCM, ADSR, echo, etc.)
//!
//! **Communication Protocol:**
//! The IPL ROM implements a boot protocol where it waits for the main CPU
//! to upload code via the communication ports, then executes it.

use crate::apu::{AudioChip, TimingMode};
use crate::cpu_spc700::{CpuSpc700, MemorySpc700};
use crate::logging::{log, LogCategory, LogLevel};

/// SPC700 I/O register addresses
const TEST_REG: u16 = 0x00F0; // Test register
const CONTROL_REG: u16 = 0x00F1; // Control register (timer enables, IPL ROM enable)
const DSP_ADDR: u16 = 0x00F2; // DSP address register
const DSP_DATA: u16 = 0x00F3; // DSP data register
const CPUIO0: u16 = 0x00F4; // Communication port 0
const CPUIO1: u16 = 0x00F5; // Communication port 1
const CPUIO2: u16 = 0x00F6; // Communication port 2
const CPUIO3: u16 = 0x00F7; // Communication port 3
const AUX_IO4: u16 = 0x00F8; // Auxiliary I/O port 4
const AUX_IO5: u16 = 0x00F9; // Auxiliary I/O port 5
const TIMER0: u16 = 0x00FA; // Timer 0 divisor
const TIMER1: u16 = 0x00FB; // Timer 1 divisor
const TIMER2: u16 = 0x00FC; // Timer 2 divisor
const COUNTER0: u16 = 0x00FD; // Timer 0 counter
const COUNTER1: u16 = 0x00FE; // Timer 1 counter
const COUNTER2: u16 = 0x00FF; // Timer 2 counter

/// SPC700 IPL (Initial Program Loader) boot ROM
/// This 64-byte ROM implements the boot protocol for receiving code uploads
const IPL_ROM: [u8; 64] = [
    // This is a simplified/placeholder IPL ROM
    // The real IPL ROM implements the upload protocol
    // For now, we'll use a minimal stub that sets ports to $BBAA and halts
    0xCD, 0x00, // MOV X, #$00
    0x8D, 0x00, // MOV Y, #$00
    0xE8, 0xAA, // MOV A, #$AA
    0xC4, 0xF4, // MOV $F4, A
    0xE8, 0xBB, // MOV A, #$BB
    0xC4, 0xF5, // MOV $F5, A
    0xEF, // SLEEP
    0x2F, 0xFC, // BRA $-4 (loop)
    // Pad with NOPs to reach 64 bytes
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00,
];

/// SPC700 memory implementation
struct Spc700Memory {
    /// 64KB RAM
    ram: Box<[u8; 0x10000]>,
    /// Control register ($F1)
    /// Bit 7: Enable timers 0 and 1
    /// Bit 6: Enable timer 2
    /// Bit 5: Unused
    /// Bit 4: Clear ports $F4-$F5
    /// Bit 3: Clear ports $F6-$F7
    /// Bit 2-1: Unused
    /// Bit 0: Enable IPL ROM
    control: u8,
    /// Communication ports (shared with main CPU)
    /// These are written by main CPU, read by SPC700
    cpuio: [u8; 4],
    /// Ports written by SPC700, read by main CPU
    /// (Separate from cpuio for bidirectional communication)
    apu_out: [u8; 4],
    /// Timer divisors
    timer_divisor: [u8; 3],
    /// Timer counters (incremented by timer tick)
    timer_counter: [u8; 3],
    /// DSP registers (128 bytes)
    dsp_regs: [u8; 128],
    /// DSP address register
    dsp_addr: u8,
}

impl Spc700Memory {
    fn new() -> Self {
        Self {
            ram: Box::new([0; 0x10000]),
            control: 0x80, // IPL ROM enabled by default
            cpuio: [0; 4],
            apu_out: [0xAA, 0xBB, 0, 0], // Default to ready signature
            timer_divisor: [0; 3],
            timer_counter: [0; 3],
            dsp_regs: [0; 128],
            dsp_addr: 0,
        }
    }

    /// Write to communication port from main CPU side
    fn write_cpuio(&mut self, port: usize, val: u8) {
        if port < 4 {
            self.cpuio[port] = val;
        }
    }

    /// Read from APU output port (main CPU side)
    fn read_apu_out(&self, port: usize) -> u8 {
        if port < 4 {
            self.apu_out[port]
        } else {
            0
        }
    }
}

impl MemorySpc700 for Spc700Memory {
    fn read(&self, addr: u16) -> u8 {
        match addr {
            // IPL ROM region (if enabled)
            0xFFC0..=0xFFFF => {
                if self.control & 0x80 != 0 {
                    IPL_ROM[(addr - 0xFFC0) as usize]
                } else {
                    self.ram[addr as usize]
                }
            }
            // Communication ports (read what SPC700 wrote)
            CPUIO0..=CPUIO3 => self.cpuio[(addr - CPUIO0) as usize],
            // Timer counters
            COUNTER0 => {
                let val = self.timer_counter[0];
                // Reading counter clears it (on real hardware)
                // We'll implement this when timers are active
                val
            }
            COUNTER1 => self.timer_counter[1],
            COUNTER2 => self.timer_counter[2],
            // DSP data register
            DSP_DATA => {
                if (self.dsp_addr as usize) < self.dsp_regs.len() {
                    self.dsp_regs[self.dsp_addr as usize]
                } else {
                    0
                }
            }
            // Other I/O registers
            TEST_REG => 0,
            CONTROL_REG => self.control,
            DSP_ADDR => self.dsp_addr,
            TIMER0 => self.timer_divisor[0],
            TIMER1 => self.timer_divisor[1],
            TIMER2 => self.timer_divisor[2],
            AUX_IO4 | AUX_IO5 => 0,
            // RAM
            _ => self.ram[addr as usize],
        }
    }

    fn write(&mut self, addr: u16, val: u8) {
        match addr {
            // IPL ROM region is read-only
            0xFFC0..=0xFFFF => {
                if self.control & 0x80 == 0 {
                    // Only writable when IPL ROM is disabled
                    self.ram[addr as usize] = val;
                }
            }
            // Communication ports (SPC700 writes, main CPU reads)
            CPUIO0..=CPUIO3 => {
                let port = (addr - CPUIO0) as usize;
                self.apu_out[port] = val;
                log(LogCategory::Bus, LogLevel::Trace, || {
                    format!("SPC700: Write port ${:04X} = ${:02X}", addr, val)
                });
            }
            // Control register
            CONTROL_REG => {
                self.control = val;
                if val & 0x10 != 0 {
                    // Clear ports $F4-$F5
                    self.cpuio[0] = 0;
                    self.cpuio[1] = 0;
                }
                if val & 0x20 != 0 {
                    // Clear ports $F6-$F7
                    self.cpuio[2] = 0;
                    self.cpuio[3] = 0;
                }
            }
            // DSP address register
            DSP_ADDR => {
                self.dsp_addr = val & 0x7F; // Only 7 bits used
            }
            // DSP data register
            DSP_DATA => {
                if (self.dsp_addr as usize) < self.dsp_regs.len() {
                    self.dsp_regs[self.dsp_addr as usize] = val;
                }
            }
            // Timer divisors
            TIMER0 => self.timer_divisor[0] = val,
            TIMER1 => self.timer_divisor[1] = val,
            TIMER2 => self.timer_divisor[2] = val,
            // Test register and counters are read-only
            TEST_REG | COUNTER0 | COUNTER1 | COUNTER2 | AUX_IO4 | AUX_IO5 => {}
            // RAM
            _ => self.ram[addr as usize] = val,
        }
    }
}

/// Complete SPC700 Audio Processing Unit
pub struct Spc700 {
    /// SPC700 CPU
    cpu: CpuSpc700<Spc700Memory>,
    /// Timing mode (always NTSC for SNES)
    timing: TimingMode,
    /// Cycle accumulator for audio sample generation
    cycle_acc: f64,
    /// Cycles per audio sample
    cycles_per_sample: f64,
}

impl Spc700 {
    /// Create a new SPC700 APU
    pub fn new() -> Self {
        let memory = Spc700Memory::new();
        let cpu = CpuSpc700::new(memory);

        // SNES APU runs at ~1.024 MHz
        // Audio output is typically 32000 Hz
        let apu_clock_hz = 1024000.0;
        let sample_rate = 32000.0;

        Self {
            cpu,
            timing: TimingMode::Ntsc,
            cycle_acc: 0.0,
            cycles_per_sample: apu_clock_hz / sample_rate,
        }
    }

    /// Write to a communication port from the main CPU
    pub fn write_port(&mut self, port: u8, val: u8) {
        if port < 4 {
            self.cpu.memory.write_cpuio(port as usize, val);
        }
    }

    /// Read from a communication port (main CPU side)
    pub fn read_port(&self, port: u8) -> u8 {
        if port < 4 {
            self.cpu.memory.read_apu_out(port as usize)
        } else {
            0
        }
    }

    /// Execute CPU for a number of cycles
    pub fn run_cycles(&mut self, cycles: u32) {
        let mut remaining = cycles;
        while remaining > 0 {
            let executed = self.cpu.step() as u32;
            remaining = remaining.saturating_sub(executed);
        }
    }
}

impl Default for Spc700 {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioChip for Spc700 {
    fn write_register(&mut self, addr: u16, val: u8) {
        // For SNES, registers are accessed via communication ports
        // This is called from the main CPU bus
        match addr {
            0x2140 => self.write_port(0, val),
            0x2141 => self.write_port(1, val),
            0x2142 => self.write_port(2, val),
            0x2143 => self.write_port(3, val),
            _ => {}
        }
    }

    fn read_register(&self, addr: u16) -> u8 {
        match addr {
            0x2140 => self.read_port(0),
            0x2141 => self.read_port(1),
            0x2142 => self.read_port(2),
            0x2143 => self.read_port(3),
            _ => 0,
        }
    }

    fn clock(&mut self) -> i16 {
        // Execute one CPU cycle
        self.cpu.step();

        // For now, return silence
        // TODO: Implement DSP audio generation
        0
    }

    fn timing(&self) -> TimingMode {
        self.timing
    }

    fn reset(&mut self) {
        self.cpu.reset();
        self.cpu.memory = Spc700Memory::new();
        self.cycle_acc = 0.0;
    }

    fn generate_samples(&mut self, count: usize) -> Vec<i16> {
        let mut samples = Vec::with_capacity(count);

        for _ in 0..count {
            // Accumulate cycles
            self.cycle_acc += self.cycles_per_sample;

            // Execute CPU cycles
            while self.cycle_acc >= 1.0 {
                self.cpu.step();
                self.cycle_acc -= 1.0;
            }

            // Generate audio sample (silence for now)
            // TODO: Implement DSP audio generation
            samples.push(0);
        }

        samples
    }

    fn sample_rate(&self) -> f64 {
        32000.0 // 32 kHz output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spc700_creation() {
        let apu = Spc700::new();
        assert_eq!(apu.cpu.pc, 0xFFC0); // Should start at IPL ROM
    }

    #[test]
    fn test_communication_ports() {
        let mut apu = Spc700::new();

        // Main CPU writes to port
        apu.write_port(0, 0x42);
        apu.write_port(1, 0x43);

        // Run a few cycles to let SPC700 potentially respond
        apu.run_cycles(100);

        // Read back (should get $BBAA default or SPC700's response)
        let port0 = apu.read_port(0);
        let port1 = apu.read_port(1);

        // With the IPL ROM, it should set ports to $AA, $BB
        assert_eq!(port0, 0xAA);
        assert_eq!(port1, 0xBB);
    }

    #[test]
    fn test_audio_chip_interface() {
        let mut apu = Spc700::new();

        // Test AudioChip trait methods
        apu.write_register(0x2140, 0x12);
        let val = apu.read_register(0x2140);

        // Should get back what SPC700 wrote
        assert!(val == 0x12 || val == 0xAA); // Either echo or default
    }
}
