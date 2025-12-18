//! Minimal NES APU with 2 pulse channels for square wave synthesis.

/// Minimal pulse channel that generates square wave samples.
#[derive(Debug, Clone)]
pub struct Pulse {
    /// Duty cycle (0-3): 12.5%, 25%, 50%, 75%
    pub duty: u8,
    /// 11-bit timer reload value from registers
    pub timer_reload: u16,
    /// Timer counter (counts down to 0, then resets to (reload+1)*2)
    timer: u16,
    /// Current phase of the duty cycle (0-7)
    phase: u8,
    /// Length counter (decrements each frame, mutes when 0)
    pub length_counter: u8,
    /// Envelope volume (4-bit)
    pub envelope: u8,
    /// Whether the channel is enabled
    pub enabled: bool,
}

impl Pulse {
    pub fn new() -> Self {
        Self {
            duty: 0,
            timer_reload: 0,
            timer: 0,
            phase: 0,
            length_counter: 0,
            envelope: 15,
            enabled: false,
        }
    }

    /// Clock the pulse channel for one CPU cycle, returning a single sample (-32768..32767)
    pub fn clock(&mut self) -> i16 {
        // Generate current sample based on duty and phase
        let sample = if self.enabled && self.length_counter > 0 {
            let output = self.duty_output();
            if output {
                (self.envelope as i16) << 10
            } else {
                -((self.envelope as i16) << 10)
            }
        } else {
            0
        };

        // Decrement timer
        if self.timer > 0 {
            self.timer -= 1;
        } else {
            // Reset timer and advance phase
            self.timer = self.timer_reload.wrapping_add(1).saturating_mul(2);
            self.phase = (self.phase + 1) & 7;
        }

        sample
    }

    /// Determine if the current phase should output 1 based on duty cycle
    pub fn duty_output(&self) -> bool {
        // NES duty patterns indexed by (duty, phase)
        // 0: 0 1 0 0 0 0 0 0 (12.5%)
        // 1: 0 1 1 0 0 0 0 0 (25%)
        // 2: 0 1 1 1 1 0 0 0 (50%)
        // 3: 1 0 0 1 1 1 1 1 (75%)
        const TABLE: [[bool; 8]; 4] = [
            [false, true, false, false, false, false, false, false],
            [false, true, true, false, false, false, false, false],
            [false, true, true, true, true, false, false, false],
            [true, false, false, true, true, true, true, true],
        ];
        TABLE[(self.duty & 3) as usize][(self.phase & 7) as usize]
    }

    /// Set timer reload and reload counter using NES formula: period = (t + 1) * 2
    pub fn set_timer(&mut self, t: u16) {
        self.timer_reload = t & 0x07FF;
        self.timer = self.timer_reload.wrapping_add(1).saturating_mul(2);
    }
}

impl Default for Pulse {
    fn default() -> Self {
        Self::new()
    }
}

/// Minimal NES APU with 2 pulse channels
#[derive(Debug)]
pub struct APU {
    pub pulse1: Pulse,
    pub pulse2: Pulse,
    cycle_accum: f64,
}

impl APU {
    pub fn new() -> Self {
        Self {
            pulse1: Pulse::new(),
            pulse2: Pulse::new(),
            cycle_accum: 0.0,
        }
    }

    /// Process APU register writes
    pub fn write_register(&mut self, addr: u16, val: u8) {
        match addr {
            // Pulse 1 registers
            0x4000 => {
                // PPUCTRL: duty, loop, disable_length_counter, envelope
                self.pulse1.duty = (val >> 6) & 3;
                self.pulse1.envelope = val & 15;
            }
            0x4001 => {
                // Sweep (ignore for now)
            }
            0x4002 => {
                // Frequency low byte
                let low = val as u16;
                let high = (self.pulse1.timer_reload >> 8) & 0x07;
                self.pulse1.set_timer((high << 8) | low);
            }
            0x4003 => {
                // Frequency high byte + length counter index
                let high = (val & 0x07) as u16;
                let low = self.pulse1.timer_reload & 0xFF;
                self.pulse1.set_timer((high << 8) | low);
                // Trigger: reset phase and enable
                self.pulse1.phase = 0;
                self.pulse1.enabled = true;
                // Length counter index in upper 5 bits
                let len_index = (val >> 3) & 0x1F;
                self.pulse1.length_counter = LENGTH_TABLE[len_index as usize];
            }

            // Pulse 2 registers
            0x4004 => {
                self.pulse2.duty = (val >> 6) & 3;
                self.pulse2.envelope = val & 15;
            }
            0x4005 => {
                // Sweep (ignore for now)
            }
            0x4006 => {
                let low = val as u16;
                let high = (self.pulse2.timer_reload >> 8) & 0x07;
                self.pulse2.set_timer((high << 8) | low);
            }
            0x4007 => {
                let freq_high = (val & 0x07) as u16;
                let freq_low = (self.pulse2.timer_reload & 0xFF) as u16;
                self.pulse2.set_timer((freq_high << 8) | freq_low);
                self.pulse2.phase = 0;
                self.pulse2.enabled = true;
                let len_index = (val >> 3) & 0x1F;
                self.pulse2.length_counter = LENGTH_TABLE[len_index as usize];
            }

            // APU Enable register
            0x4015 => {
                self.pulse1.enabled = (val & 1) != 0;
                self.pulse2.enabled = (val & 2) != 0;
            }

            _ => {}
        }
    }

    /// Generate audio samples for a given count, stepping APU in CPU-cycle time
    /// using NTSC CPU clock 1.789773 MHz and sample rate 44.1 kHz.
    pub fn generate_samples(&mut self, sample_count: usize) -> Vec<i16> {
        const CPU_HZ: f64 = 1_789_773.0;
        const SAMPLE_HZ: f64 = 44_100.0;
        let cycles_per_sample = CPU_HZ / SAMPLE_HZ; // ~40.58

        let mut out = Vec::with_capacity(sample_count);
        for _ in 0..sample_count {
            self.cycle_accum += cycles_per_sample;
            let mut cycles = self.cycle_accum as u32;
            if cycles == 0 {
                cycles = 1; // ensure we advance state even if timing slips
            }
            self.cycle_accum -= cycles as f64;

            let mut acc = 0i32;
            for _ in 0..cycles {
                let s1 = self.pulse1.clock() as i32;
                let s2 = self.pulse2.clock() as i32;
                acc += s1 + s2;
            }

            let avg = acc / cycles as i32;
            let mixed = avg / 2; // simple pulse mix average
            out.push(mixed.clamp(-32768, 32767) as i16);
        }

        out
    }
}

impl Default for APU {
    fn default() -> Self {
        Self::new()
    }
}

// NES length counter lookup table
const LENGTH_TABLE: [u8; 32] = [
    10, 254, 20, 2, 40, 4, 80, 6, 160, 8, 60, 10, 14, 12, 26, 14, 12, 16, 24, 18, 48, 20, 96, 22,
    192, 24, 72, 26, 16, 28, 32, 30,
];
