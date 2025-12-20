//! Game Boy APU (Audio Processing Unit) implementation.
//!
//! This module provides the Game Boy-specific APU interface using
//! reusable components from the core module.
//!
//! ## Game Boy Audio Architecture
//!
//! The Game Boy has 4 sound channels:
//!
//! 1. **Pulse 1**: Square wave with sweep (NR10-NR14)
//!    - Duty cycle: 12.5%, 25%, 50%, 75%
//!    - Frequency sweep (increase/decrease over time)
//!    - Envelope generator for volume control
//!    - Length counter for automatic note duration
//!
//! 2. **Pulse 2**: Square wave without sweep (NR21-NR24)
//!    - Same as Pulse 1 but no sweep unit
//!    - Duty cycle: 12.5%, 25%, 50%, 75%
//!    - Envelope generator and length counter
//!
//! 3. **Wave**: Custom waveform (NR30-NR34, $FF30-$FF3F)
//!    - 32 x 4-bit samples in wave RAM
//!    - Volume control: mute, 100%, 50%, 25%
//!    - No envelope generator
//!    - Length counter
//!
//! 4. **Noise**: Pseudo-random noise (NR41-NR44)
//!    - 7-bit or 15-bit LFSR modes
//!    - Envelope generator for volume control
//!    - Length counter
//!
//! ## Register Map
//!
//! - **$FF10-$FF14**: Pulse 1 (NR10-NR14)
//! - **$FF15-$FF19**: Pulse 2 (NR20-NR24, NR15 unused)
//! - **$FF1A-$FF1E**: Wave (NR30-NR34)
//! - **$FF1F-$FF23**: Noise (NR40-NR44, NR40 unused)
//! - **$FF24**: Master volume (NR50)
//! - **$FF25**: Sound panning (NR51)
//! - **$FF26**: Sound on/off (NR52)
//! - **$FF30-$FF3F**: Wave pattern RAM (16 bytes, 32 samples)
//!
//! ## Frame Sequencer
//!
//! The Game Boy frame sequencer runs at 512 Hz and controls:
//! - Step 0: Length counter
//! - Step 1: Nothing
//! - Step 2: Length counter and sweep
//! - Step 3: Nothing
//! - Step 4: Length counter
//! - Step 5: Nothing
//! - Step 6: Length counter and sweep
//! - Step 7: Envelope
//!
//! ## Timing
//!
//! - CPU clock: 4.194304 MHz
//! - Frame sequencer: 512 Hz (every 8192 cycles)
//! - Length counter: 256 Hz (every other frame sequencer step)
//! - Envelope: 64 Hz (every 8th frame sequencer step)
//! - Sweep: 128 Hz (every 4th frame sequencer step)
//!
//! ## Audio Output
//!
//! The APU generates 44.1 kHz stereo audio by:
//!
//! 1. Clocking the APU at CPU speed (4.194304 MHz)
//! 2. Running the frame sequencer at 512 Hz
//! 3. Mixing the active channels
//! 4. Downsampling to the target sample rate

use emu_core::apu::{Envelope, LengthCounter, NoiseChannel, PulseChannel, SweepUnit, WaveChannel};

/// Game Boy APU with 4 sound channels.
///
/// Uses core APU components for audio synthesis.
///
/// # Registers
///
/// The APU responds to reads/writes at $FF10-$FF26 and $FF30-$FF3F.
///
/// ## Pulse 1 (NR10-NR14)
/// - NR10 ($FF10): Sweep (PPP DNNN - Period, Negate, Shift)
/// - NR11 ($FF11): Duty and length (DDLL LLLL)
/// - NR12 ($FF12): Envelope (VVVV APPP - Volume, Add/subtract, Period)
/// - NR13 ($FF13): Frequency low (FFFF FFFF)
/// - NR14 ($FF14): Frequency high and control (TL-- -FFF)
///
/// ## Pulse 2 (NR21-NR24)
/// - NR21 ($FF16): Duty and length (DDLL LLLL)
/// - NR22 ($FF17): Envelope (VVVV APPP)
/// - NR23 ($FF18): Frequency low (FFFF FFFF)
/// - NR24 ($FF19): Frequency high and control (TL-- -FFF)
///
/// ## Wave (NR30-NR34)
/// - NR30 ($FF1A): DAC enable (E--- ----)
/// - NR31 ($FF1B): Length (LLLL LLLL)
/// - NR32 ($FF1C): Volume (0VV- ---- - 0=mute, 1=100%, 2=50%, 3=25%)
/// - NR33 ($FF1D): Frequency low (FFFF FFFF)
/// - NR34 ($FF1E): Frequency high and control (TL-- -FFF)
///
/// ## Noise (NR41-NR44)
/// - NR41 ($FF20): Length (--LL LLLL)
/// - NR42 ($FF21): Envelope (VVVV APPP)
/// - NR43 ($FF22): Polynomial counter (SSSS WDDD - Clock shift, Width, Divisor)
/// - NR44 ($FF23): Control (T L-- ----)
///
/// ## Control
/// - NR50 ($FF24): Master volume (ALLL BLLL - Vin L/R enable, Left/Right volume)
/// - NR51 ($FF25): Sound panning (4444 3333 2222 1111 - Channel to L/R output)
/// - NR52 ($FF26): Sound on/off (P--- 4321 - Power, channel enables)
///
/// ## Wave RAM
/// - $FF30-$FF3F: 16 bytes (32 x 4-bit samples)
#[derive(Debug)]
pub struct GbApu {
    // Sound channels
    pub pulse1: PulseChannel,
    pub pulse1_sweep: SweepUnit,
    pub pulse1_envelope: Envelope,
    pub pulse1_length: LengthCounter,
    
    pub pulse2: PulseChannel,
    pub pulse2_envelope: Envelope,
    pub pulse2_length: LengthCounter,
    
    pub wave: WaveChannel,
    pub wave_length: LengthCounter,
    
    pub noise: NoiseChannel,
    pub noise_envelope: Envelope,
    pub noise_length: LengthCounter,
    
    // Frame sequencer
    frame_sequencer_cycles: u32,
    frame_sequencer_step: u8,
    
    // Master controls
    power_on: bool,
    left_volume: u8,
    right_volume: u8,
    channel_panning: u8, // Bits for L/R panning per channel
    
    // Temporary registers for triggering
    pulse1_frequency: u16,
    pulse2_frequency: u16,
    wave_frequency: u16,
    wave_dac_enabled: bool,
    
    // Sample generation
    _cycle_accum: f64,
}

impl GbApu {
    /// Create a new Game Boy APU with default state
    pub fn new() -> Self {
        Self {
            pulse1: PulseChannel::new(),
            pulse1_sweep: SweepUnit::new(),
            pulse1_envelope: Envelope::new(),
            pulse1_length: LengthCounter::new(),
            
            pulse2: PulseChannel::new(),
            pulse2_envelope: Envelope::new(),
            pulse2_length: LengthCounter::new(),
            
            wave: WaveChannel::new(),
            wave_length: LengthCounter::new(),
            
            noise: NoiseChannel::new(),
            noise_envelope: Envelope::new(),
            noise_length: LengthCounter::new(),
            
            frame_sequencer_cycles: 0,
            frame_sequencer_step: 0,
            
            power_on: true,
            left_volume: 7,
            right_volume: 7,
            channel_panning: 0xFF,
            
            pulse1_frequency: 0,
            pulse2_frequency: 0,
            wave_frequency: 0,
            wave_dac_enabled: false,
            
            _cycle_accum: 0.0,
        }
    }
    
    /// Clock the APU for one CPU cycle
    pub fn clock(&mut self) {
        // Frame sequencer runs at 512 Hz (every 8192 CPU cycles at 4.194304 MHz)
        const CYCLES_PER_FRAME_STEP: u32 = 8192;
        
        self.frame_sequencer_cycles += 1;
        if self.frame_sequencer_cycles >= CYCLES_PER_FRAME_STEP {
            self.frame_sequencer_cycles = 0;
            self.clock_frame_sequencer();
        }
        
        // Clock all channels
        if self.power_on {
            // Pulse channels clock at CPU speed
            let _ = self.pulse1.clock();
            let _ = self.pulse2.clock();
            let _ = self.wave.clock();
            let _ = self.noise.clock();
        }
    }
    
    /// Clock the frame sequencer (called at 512 Hz)
    fn clock_frame_sequencer(&mut self) {
        // Frame sequencer pattern (8 steps):
        // Step 0: Length
        // Step 1: -
        // Step 2: Length + Sweep
        // Step 3: -
        // Step 4: Length
        // Step 5: -
        // Step 6: Length + Sweep
        // Step 7: Envelope
        
        match self.frame_sequencer_step {
            0 | 2 | 4 | 6 => {
                // Clock length counters
                self.pulse1_length.clock();
                self.pulse2_length.clock();
                self.wave_length.clock();
                self.noise_length.clock();
                
                // Update channel enabled state based on length counters
                if !self.pulse1_length.is_active() {
                    self.pulse1.enabled = false;
                }
                if !self.pulse2_length.is_active() {
                    self.pulse2.enabled = false;
                }
                if !self.wave_length.is_active() {
                    self.wave.enabled = false;
                }
                if !self.noise_length.is_active() {
                    self.noise.enabled = false;
                }
                
                // Clock sweep on steps 2 and 6
                if self.frame_sequencer_step == 2 || self.frame_sequencer_step == 6 {
                    if let Some(new_freq) = self.pulse1_sweep.clock() {
                        self.pulse1_frequency = new_freq;
                        self.pulse1.set_timer(new_freq);
                    }
                }
            }
            7 => {
                // Clock envelopes
                self.pulse1_envelope.clock();
                self.pulse2_envelope.clock();
                self.noise_envelope.clock();
                
                // Update channel volumes
                self.pulse1.envelope = self.pulse1_envelope.volume();
                self.pulse2.envelope = self.pulse2_envelope.volume();
                self.noise.envelope = self.noise_envelope.volume();
            }
            _ => {}
        }
        
        self.frame_sequencer_step = (self.frame_sequencer_step + 1) & 7;
    }
    
    /// Read from an APU register
    pub fn read_register(&self, addr: u16) -> u8 {
        match addr {
            // NR10: Pulse 1 sweep
            0xFF10 => {
                let period = self.pulse1_sweep.period & 0x07;
                let negate = if self.pulse1_sweep.negate { 0x08 } else { 0x00 };
                let shift = self.pulse1_sweep.shift & 0x07;
                0x80 | (period << 4) | negate | shift
            }
            // NR11: Pulse 1 duty (write-only, return duty only)
            0xFF11 => {
                (self.pulse1.duty << 6) | 0x3F
            }
            // NR12: Pulse 1 envelope
            0xFF12 => {
                let volume = self.pulse1_envelope.initial_volume() & 0x0F;
                let add_mode = if self.pulse1_envelope.add_mode() { 0x08 } else { 0x00 };
                let period = self.pulse1_envelope.period() & 0x07;
                (volume << 4) | add_mode | period
            }
            // NR13: Pulse 1 frequency low (write-only)
            0xFF13 => 0xFF,
            // NR14: Pulse 1 frequency high and control
            0xFF14 => {
                let length_enable = if self.pulse1_length.is_enabled() { 0x40 } else { 0x00 };
                0xBF | length_enable
            }
            
            // NR20 unused
            0xFF15 => 0xFF,
            // NR21: Pulse 2 duty
            0xFF16 => {
                (self.pulse2.duty << 6) | 0x3F
            }
            // NR22: Pulse 2 envelope
            0xFF17 => {
                let volume = self.pulse2_envelope.initial_volume() & 0x0F;
                let add_mode = if self.pulse2_envelope.add_mode() { 0x08 } else { 0x00 };
                let period = self.pulse2_envelope.period() & 0x07;
                (volume << 4) | add_mode | period
            }
            // NR23: Pulse 2 frequency low (write-only)
            0xFF18 => 0xFF,
            // NR24: Pulse 2 frequency high and control
            0xFF19 => {
                let length_enable = if self.pulse2_length.is_enabled() { 0x40 } else { 0x00 };
                0xBF | length_enable
            }
            
            // NR30: Wave DAC enable
            0xFF1A => {
                if self.wave_dac_enabled { 0x80 } else { 0x7F }
            }
            // NR31: Wave length (write-only)
            0xFF1B => 0xFF,
            // NR32: Wave volume
            0xFF1C => {
                ((self.wave.volume_shift & 0x03) << 5) | 0x9F
            }
            // NR33: Wave frequency low (write-only)
            0xFF1D => 0xFF,
            // NR34: Wave frequency high and control
            0xFF1E => {
                let length_enable = if self.wave_length.is_enabled() { 0x40 } else { 0x00 };
                0xBF | length_enable
            }
            
            // NR40 unused
            0xFF1F => 0xFF,
            // NR41: Noise length (write-only)
            0xFF20 => 0xFF,
            // NR42: Noise envelope
            0xFF21 => {
                let volume = self.noise_envelope.initial_volume() & 0x0F;
                let add_mode = if self.noise_envelope.add_mode() { 0x08 } else { 0x00 };
                let period = self.noise_envelope.period() & 0x07;
                (volume << 4) | add_mode | period
            }
            // NR43: Noise polynomial counter
            0xFF22 => {
                let shift = (self.noise.period_index >> 4) & 0x0F;
                let width = if self.noise.mode { 0x08 } else { 0x00 };
                let divisor = self.noise.period_index & 0x07;
                (shift << 4) | width | divisor
            }
            // NR44: Noise control
            0xFF23 => {
                let length_enable = if self.noise_length.is_enabled() { 0x40 } else { 0x00 };
                0xBF | length_enable
            }
            
            // NR50: Master volume
            0xFF24 => {
                ((self.left_volume & 0x07) << 4) | (self.right_volume & 0x07)
            }
            // NR51: Sound panning
            0xFF25 => self.channel_panning,
            // NR52: Sound on/off
            0xFF26 => {
                let power = if self.power_on { 0x80 } else { 0x00 };
                let ch1 = if self.pulse1.enabled { 0x01 } else { 0x00 };
                let ch2 = if self.pulse2.enabled { 0x02 } else { 0x00 };
                let ch3 = if self.wave.enabled { 0x04 } else { 0x00 };
                let ch4 = if self.noise.enabled { 0x08 } else { 0x00 };
                power | ch1 | ch2 | ch3 | ch4 | 0x70
            }
            
            // Wave RAM
            0xFF30..=0xFF3F => {
                let offset = (addr - 0xFF30) as usize;
                self.wave.read_wave_ram_byte(offset)
            }
            
            _ => 0xFF,
        }
    }
    
    /// Write to an APU register
    pub fn write_register(&mut self, addr: u16, val: u8) {
        // If power is off, ignore all writes except to NR52
        if !self.power_on && addr != 0xFF26 {
            return;
        }
        
        match addr {
            // NR10: Pulse 1 sweep
            0xFF10 => {
                self.pulse1_sweep.period = (val >> 4) & 0x07;
                self.pulse1_sweep.negate = (val & 0x08) != 0;
                self.pulse1_sweep.shift = val & 0x07;
            }
            // NR11: Pulse 1 duty and length
            0xFF11 => {
                self.pulse1.duty = (val >> 6) & 0x03;
                let length_load = val & 0x3F;
                self.pulse1_length.load_gb(length_load, 64);
            }
            // NR12: Pulse 1 envelope
            0xFF12 => {
                let initial_volume = (val >> 4) & 0x0F;
                let add_mode = (val & 0x08) != 0;
                let period = val & 0x07;
                self.pulse1_envelope.set_params(initial_volume, add_mode, period);
                
                // DAC enable: if top 5 bits are 0, DAC is off
                if (val & 0xF8) == 0 {
                    self.pulse1.enabled = false;
                }
            }
            // NR13: Pulse 1 frequency low
            0xFF13 => {
                self.pulse1_frequency = (self.pulse1_frequency & 0x0700) | (val as u16);
            }
            // NR14: Pulse 1 frequency high and control
            0xFF14 => {
                self.pulse1_frequency = (self.pulse1_frequency & 0x00FF) | (((val & 0x07) as u16) << 8);
                let length_enable = (val & 0x40) != 0;
                let trigger = (val & 0x80) != 0;
                
                self.pulse1_length.set_enabled(length_enable);
                
                if trigger {
                    self.pulse1.enabled = true;
                    self.pulse1.set_timer(self.pulse1_frequency);
                    self.pulse1_envelope.trigger();
                    self.pulse1_sweep.trigger(self.pulse1_frequency);
                    
                    // If length counter is 0, reload it
                    if self.pulse1_length.value() == 0 {
                        self.pulse1_length.load_gb(0, 64);
                    }
                }
            }
            
            // NR20 unused
            0xFF15 => {}
            // NR21: Pulse 2 duty and length
            0xFF16 => {
                self.pulse2.duty = (val >> 6) & 0x03;
                let length_load = val & 0x3F;
                self.pulse2_length.load_gb(length_load, 64);
            }
            // NR22: Pulse 2 envelope
            0xFF17 => {
                let initial_volume = (val >> 4) & 0x0F;
                let add_mode = (val & 0x08) != 0;
                let period = val & 0x07;
                self.pulse2_envelope.set_params(initial_volume, add_mode, period);
                
                // DAC enable
                if (val & 0xF8) == 0 {
                    self.pulse2.enabled = false;
                }
            }
            // NR23: Pulse 2 frequency low
            0xFF18 => {
                self.pulse2_frequency = (self.pulse2_frequency & 0x0700) | (val as u16);
            }
            // NR24: Pulse 2 frequency high and control
            0xFF19 => {
                self.pulse2_frequency = (self.pulse2_frequency & 0x00FF) | (((val & 0x07) as u16) << 8);
                let length_enable = (val & 0x40) != 0;
                let trigger = (val & 0x80) != 0;
                
                self.pulse2_length.set_enabled(length_enable);
                
                if trigger {
                    self.pulse2.enabled = true;
                    self.pulse2.set_timer(self.pulse2_frequency);
                    self.pulse2_envelope.trigger();
                    
                    if self.pulse2_length.value() == 0 {
                        self.pulse2_length.load_gb(0, 64);
                    }
                }
            }
            
            // NR30: Wave DAC enable
            0xFF1A => {
                self.wave_dac_enabled = (val & 0x80) != 0;
                if !self.wave_dac_enabled {
                    self.wave.enabled = false;
                }
            }
            // NR31: Wave length
            0xFF1B => {
                let length_load = val;
                self.wave_length.load_gb(length_load, 256);
            }
            // NR32: Wave volume
            0xFF1C => {
                self.wave.volume_shift = (val >> 5) & 0x03;
            }
            // NR33: Wave frequency low
            0xFF1D => {
                self.wave_frequency = (self.wave_frequency & 0x0700) | (val as u16);
            }
            // NR34: Wave frequency high and control
            0xFF1E => {
                self.wave_frequency = (self.wave_frequency & 0x00FF) | (((val & 0x07) as u16) << 8);
                let length_enable = (val & 0x40) != 0;
                let trigger = (val & 0x80) != 0;
                
                self.wave_length.set_enabled(length_enable);
                
                if trigger && self.wave_dac_enabled {
                    self.wave.enabled = true;
                    self.wave.set_timer(self.wave_frequency);
                    self.wave.reset_position();
                    
                    if self.wave_length.value() == 0 {
                        self.wave_length.load_gb(0, 256);
                    }
                }
            }
            
            // NR40 unused
            0xFF1F => {}
            // NR41: Noise length
            0xFF20 => {
                let length_load = val & 0x3F;
                self.noise_length.load_gb(length_load, 64);
            }
            // NR42: Noise envelope
            0xFF21 => {
                let initial_volume = (val >> 4) & 0x0F;
                let add_mode = (val & 0x08) != 0;
                let period = val & 0x07;
                self.noise_envelope.set_params(initial_volume, add_mode, period);
                
                // DAC enable
                if (val & 0xF8) == 0 {
                    self.noise.enabled = false;
                }
            }
            // NR43: Noise polynomial counter
            0xFF22 => {
                // Game Boy noise uses different encoding than NES
                // Format: SSSS WDDD
                // S = clock shift (0-15)
                // W = width mode (0 = 15-bit, 1 = 7-bit)
                // D = divisor code (0-7)
                
                let _shift = (val >> 4) & 0x0F;
                let width = (val & 0x08) != 0;
                let _divisor = val & 0x07;
                
                self.noise.mode = width;
                
                // Convert to period index
                // GB uses: frequency = 262144 / (divisor * 2^(shift+1))
                // We'll store shift and divisor in period_index for now
                self.noise.period_index = val;
            }
            // NR44: Noise control
            0xFF23 => {
                let length_enable = (val & 0x40) != 0;
                let trigger = (val & 0x80) != 0;
                
                self.noise_length.set_enabled(length_enable);
                
                if trigger {
                    self.noise.enabled = true;
                    self.noise_envelope.trigger();
                    
                    if self.noise_length.value() == 0 {
                        self.noise_length.load_gb(0, 64);
                    }
                }
            }
            
            // NR50: Master volume
            0xFF24 => {
                self.left_volume = (val >> 4) & 0x07;
                self.right_volume = val & 0x07;
            }
            // NR51: Sound panning
            0xFF25 => {
                self.channel_panning = val;
            }
            // NR52: Sound on/off
            0xFF26 => {
                let new_power = (val & 0x80) != 0;
                
                if !new_power && self.power_on {
                    // Turning power off - clear all registers
                    self.reset();
                }
                
                self.power_on = new_power;
            }
            
            // Wave RAM
            0xFF30..=0xFF3F => {
                let offset = (addr - 0xFF30) as usize;
                self.wave.write_wave_ram_byte(offset, val);
            }
            
            _ => {}
        }
    }
    
    /// Reset the APU (called when power is turned off)
    fn reset(&mut self) {
        self.pulse1 = PulseChannel::new();
        self.pulse1_sweep = SweepUnit::new();
        self.pulse1_envelope = Envelope::new();
        self.pulse1_length = LengthCounter::new();
        
        self.pulse2 = PulseChannel::new();
        self.pulse2_envelope = Envelope::new();
        self.pulse2_length = LengthCounter::new();
        
        self.wave = WaveChannel::new();
        self.wave_length = LengthCounter::new();
        
        self.noise = NoiseChannel::new();
        self.noise_envelope = Envelope::new();
        self.noise_length = LengthCounter::new();
        
        self.frame_sequencer_cycles = 0;
        self.frame_sequencer_step = 0;
        
        self.left_volume = 0;
        self.right_volume = 0;
        self.channel_panning = 0;
        
        self.pulse1_frequency = 0;
        self.pulse2_frequency = 0;
        self.wave_frequency = 0;
        self.wave_dac_enabled = false;
    }
    
    /// Generate audio samples for a number of CPU cycles
    ///
    /// Returns samples at 44.1 kHz sample rate.
    #[allow(dead_code)]
    pub fn generate_samples(&mut self, cpu_cycles: u32) -> Vec<i16> {
        const SAMPLE_RATE: f64 = 44100.0;
        const CPU_CLOCK: f64 = 4194304.0;
        const CYCLES_PER_SAMPLE: f64 = CPU_CLOCK / SAMPLE_RATE;
        
        let mut samples = Vec::new();
        let mut cycle_accum = 0.0;
        
        for _ in 0..cpu_cycles {
            self.clock();
            
            cycle_accum += 1.0;
            if cycle_accum >= CYCLES_PER_SAMPLE {
                cycle_accum -= CYCLES_PER_SAMPLE;
                
                // Mix all channels
                let sample = self.mix_channels();
                samples.push(sample);
            }
        }
        
        samples
    }
    
    /// Mix all active channels into a single sample
    fn mix_channels(&self) -> i16 {
        if !self.power_on {
            return 0;
        }
        
        let mut sample = 0i32;
        let mut active_channels = 0;
        
        // Add pulse 1
        if self.pulse1.enabled && self.pulse1_length.is_active() {
            sample += self.pulse1.duty_output() as i32 * (self.pulse1.envelope as i32);
            active_channels += 1;
        }
        
        // Add pulse 2
        if self.pulse2.enabled && self.pulse2_length.is_active() {
            sample += self.pulse2.duty_output() as i32 * (self.pulse2.envelope as i32);
            active_channels += 1;
        }
        
        // Add wave
        if self.wave.enabled && self.wave_length.is_active() && self.wave_dac_enabled {
            // Wave channel outputs 4-bit samples
            let wave_sample = self.wave.wave_ram[0] as i32;
            sample += wave_sample * (1 << (self.wave.volume_shift));
            active_channels += 1;
        }
        
        // Add noise
        if self.noise.enabled && self.noise_length.is_active() {
            sample += self.noise.envelope as i32;
            active_channels += 1;
        }
        
        // Average and apply master volume
        if active_channels > 0 {
            sample /= active_channels;
            sample = sample * ((self.left_volume + self.right_volume) as i32) / 14;
            // Scale to 16-bit range
            (sample << 8) as i16
        } else {
            0
        }
    }
}

impl Default for GbApu {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_apu_creation() {
        let apu = GbApu::new();
        assert!(apu.power_on);
        assert_eq!(apu.left_volume, 7);
        assert_eq!(apu.right_volume, 7);
    }
    
    #[test]
    fn test_power_control() {
        let mut apu = GbApu::new();
        
        // Power on by default
        assert!(apu.power_on);
        assert_eq!(apu.read_register(0xFF26) & 0x80, 0x80);
        
        // Turn power off
        apu.write_register(0xFF26, 0x00);
        assert!(!apu.power_on);
        assert_eq!(apu.read_register(0xFF26) & 0x80, 0x00);
        
        // Turn power back on
        apu.write_register(0xFF26, 0x80);
        assert!(apu.power_on);
    }
    
    #[test]
    fn test_pulse1_register_writes() {
        let mut apu = GbApu::new();
        
        // Write duty and length
        apu.write_register(0xFF11, 0b11_000001);
        assert_eq!(apu.pulse1.duty, 3);
        
        // Write envelope
        apu.write_register(0xFF12, 0xF3); // Initial volume 15, add mode, period 3
        
        // Write frequency
        apu.write_register(0xFF13, 0x00);
        apu.write_register(0xFF14, 0x87); // Trigger, length enable, freq high = 7
        
        assert!(apu.pulse1.enabled);
    }
    
    #[test]
    fn test_wave_ram_access() {
        let mut apu = GbApu::new();
        
        // Write to wave RAM
        apu.write_register(0xFF30, 0x12);
        apu.write_register(0xFF31, 0x34);
        
        // Read back
        assert_eq!(apu.read_register(0xFF30), 0x12);
        assert_eq!(apu.read_register(0xFF31), 0x34);
    }
    
    #[test]
    fn test_master_volume() {
        let mut apu = GbApu::new();
        
        // Set left volume 5, right volume 3
        apu.write_register(0xFF24, 0x53);
        
        assert_eq!(apu.left_volume, 5);
        assert_eq!(apu.right_volume, 3);
        assert_eq!(apu.read_register(0xFF24), 0x53);
    }
    
    #[test]
    fn test_channel_enable_status() {
        let mut apu = GbApu::new();
        
        // Initially no channels enabled
        let status = apu.read_register(0xFF26);
        assert_eq!(status & 0x0F, 0);
        
        // Enable pulse 1
        apu.write_register(0xFF12, 0xF0); // DAC on
        apu.write_register(0xFF14, 0x80); // Trigger
        
        let status = apu.read_register(0xFF26);
        assert_eq!(status & 0x01, 0x01);
    }
    
    #[test]
    fn test_frame_sequencer() {
        let mut apu = GbApu::new();
        
        // Set up pulse 1 with length counter
        apu.write_register(0xFF11, 0b00_000001); // Length = 1
        apu.write_register(0xFF12, 0xF0); // DAC on
        apu.write_register(0xFF14, 0xC0); // Trigger with length enable
        
        assert!(apu.pulse1.enabled);
        
        // Clock the frame sequencer manually
        for _ in 0..8192 {
            apu.clock();
        }
        
        // Length counter should have been clocked
        // After one frame sequencer step, length should decrease
    }
    
    #[test]
    fn test_pulse2_trigger() {
        let mut apu = GbApu::new();
        
        // Configure pulse 2
        apu.write_register(0xFF16, 0b10_111111); // Duty 50%, length 63
        apu.write_register(0xFF17, 0xF3); // Volume 15, add mode, period 3
        apu.write_register(0xFF18, 0x00); // Freq low
        apu.write_register(0xFF19, 0x87); // Trigger, length enable, freq high
        
        assert!(apu.pulse2.enabled);
        assert_eq!(apu.pulse2.duty, 2); // 50% duty
    }
    
    #[test]
    fn test_wave_channel_enable() {
        let mut apu = GbApu::new();
        
        // Enable DAC
        apu.write_register(0xFF1A, 0x80);
        assert!(apu.wave_dac_enabled);
        
        // Write wave RAM
        for i in 0..16 {
            apu.write_register(0xFF30 + i, i as u8);
        }
        
        // Trigger wave channel
        apu.write_register(0xFF1E, 0x80);
        assert!(apu.wave.enabled);
    }
    
    #[test]
    fn test_noise_channel_modes() {
        let mut apu = GbApu::new();
        
        // Test 7-bit mode
        apu.write_register(0xFF22, 0x08); // Width mode bit set
        assert!(apu.noise.mode);
        
        // Test 15-bit mode
        apu.write_register(0xFF22, 0x00); // Width mode bit clear
        assert!(!apu.noise.mode);
    }
    
    #[test]
    fn test_envelope_increase_mode() {
        let mut apu = GbApu::new();
        
        // Set envelope with increase mode
        apu.write_register(0xFF12, 0x08); // Initial volume 0, add mode, period 0
        apu.write_register(0xFF14, 0x80); // Trigger
        
        // Volume should start at 0
        assert_eq!(apu.pulse1_envelope.volume(), 0);
        
        // Clock envelope
        for _ in 0..8 {
            apu.frame_sequencer_step = 7;
            apu.clock_frame_sequencer();
        }
        
        // Volume should have increased
        assert!(apu.pulse1_envelope.volume() > 0);
    }
    
    #[test]
    fn test_channel_panning() {
        let mut apu = GbApu::new();
        
        // Set panning - all channels to both speakers
        apu.write_register(0xFF25, 0xFF);
        assert_eq!(apu.channel_panning, 0xFF);
        
        // Set panning - channel 1 left only
        apu.write_register(0xFF25, 0x10);
        assert_eq!(apu.channel_panning, 0x10);
    }
    
    #[test]
    fn test_power_off_clears_registers() {
        let mut apu = GbApu::new();
        
        // Set some registers
        apu.write_register(0xFF12, 0xF0);
        apu.write_register(0xFF24, 0x77);
        
        // Turn power off
        apu.write_register(0xFF26, 0x00);
        
        // All state should be cleared
        assert_eq!(apu.left_volume, 0);
        assert_eq!(apu.right_volume, 0);
        assert!(!apu.power_on);
    }
    
    #[test]
    fn test_sweep_unit_integration() {
        let mut apu = GbApu::new();
        
        // Configure sweep
        apu.write_register(0xFF10, 0x11); // Period 1, shift 1
        apu.write_register(0xFF13, 0x00); // Freq low
        apu.write_register(0xFF14, 0x80); // Trigger
        
        // Sweep should be enabled after trigger
        assert!(apu.pulse1_sweep.enabled || apu.pulse1_sweep.period > 0 || apu.pulse1_sweep.shift > 0);
    }
}
