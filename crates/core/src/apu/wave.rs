//! Wave channel with programmable waveform.
//!
//! This module implements a wave channel that can play custom waveforms from a sample table.
//! Used in the Game Boy APU and potentially reusable in other systems with programmable waveform channels.

/// Wave channel that generates samples from a programmable waveform table.
///
/// The wave channel supports:
/// - 32 x 4-bit samples in wave RAM
/// - Timer-based frequency control
/// - Volume shift control (0%, 25%, 50%, 100%)
/// - No envelope generator (fixed volume)
#[derive(Debug, Clone)]
pub struct WaveChannel {
    /// Wave RAM: 32 samples, each 4 bits (0-15)
    pub wave_ram: [u8; 32],
    /// Timer reload value (up to 12 bits) used by GB/GBA wave frequency calculations
    pub timer_reload: u16,
    /// Timer counter
    timer: u16,
    /// Current position in wave table (0-31)
    position: u8,
    /// Volume shift (0=mute, 1=100%, 2=50%, 3=25%, 4=25%)
    pub volume_shift: u8,
    /// Whether the channel is enabled
    pub enabled: bool,
}

impl WaveChannel {
    /// Create a new wave channel with default state
    pub fn new() -> Self {
        Self {
            wave_ram: [0; 32],
            timer_reload: 0,
            timer: 0,
            position: 0,
            volume_shift: 0,
            enabled: false,
        }
    }

    /// Clock the wave channel for one CPU cycle
    pub fn clock(&mut self) -> i16 {
        // Get current sample from wave RAM
        let sample_4bit = self.wave_ram[self.position as usize] & 0x0F;

        // Apply volume shift
        let sample = if self.enabled && self.volume_shift > 0 {
            let shifted = match self.volume_shift {
                1 => sample_4bit,          // 100% (no shift)
                2 => sample_4bit >> 1,     // 50% (shift right 1)
                3 | 4 => sample_4bit >> 2, // 25% (shift right 2)
                _ => 0,                    // Mute
            };
            // Convert 4-bit value to signed 16-bit (scale up without centering)
            (shifted as i16) << 10
        } else {
            0
        };

        // Timer countdown
        if self.timer > 0 {
            self.timer -= 1;
        } else {
            // Reload timer and advance position
            self.timer = self.timer_reload;
            self.position = (self.position + 1) & 31;
        }

        sample
    }

    /// Set timer reload value
    ///
    /// Accepts values up to 12 bits (0–4095) to support Game Boy / GBA
    /// frequency calculations, which can produce timer-reload values up to
    /// 4095.  GB/GBA-family channels use this full range for low-frequency
    /// notes.
    pub fn set_timer(&mut self, t: u16) {
        self.timer_reload = t & 0x0FFF;
    }

    /// Reset the timer to reload value (used on channel trigger)
    pub fn reset_timer(&mut self) {
        self.timer = self.timer_reload;
    }

    /// Write a byte to wave RAM (Game Boy format: 2 samples per byte)
    pub fn write_wave_ram_byte(&mut self, offset: usize, value: u8) {
        if offset < 16 {
            // Each byte contains 2 samples (upper and lower nibbles)
            self.wave_ram[offset * 2] = (value >> 4) & 0x0F;
            self.wave_ram[offset * 2 + 1] = value & 0x0F;
        }
    }

    /// Read a byte from wave RAM (Game Boy format: 2 samples per byte)
    pub fn read_wave_ram_byte(&self, offset: usize) -> u8 {
        if offset < 16 {
            (self.wave_ram[offset * 2] << 4) | (self.wave_ram[offset * 2 + 1] & 0x0F)
        } else {
            0
        }
    }

    /// Reset the position to start of wave table
    pub fn reset_position(&mut self) {
        self.position = 0;
    }
}

impl Default for WaveChannel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wave_position_advances() {
        let mut wave = WaveChannel::new();
        wave.enabled = true;
        wave.volume_shift = 1; // 100% volume
        wave.set_timer(0); // Fastest timer

        let initial_pos = wave.position;
        wave.clock();
        assert_eq!(wave.position, (initial_pos + 1) & 31);
    }

    #[test]
    fn wave_position_wraps_at_32() {
        let mut wave = WaveChannel::new();
        wave.enabled = true;
        wave.volume_shift = 1;
        wave.set_timer(0);
        wave.position = 31;

        wave.clock();
        assert_eq!(wave.position, 0);
    }

    #[test]
    fn wave_volume_shift_controls_output() {
        let mut wave = WaveChannel::new();
        wave.enabled = true;
        wave.wave_ram[0] = 15; // Maximum sample value
        wave.set_timer(10); // Slow enough to sample same position

        // 100% volume
        wave.volume_shift = 1;
        let sample_100 = wave.clock().abs();

        // 50% volume
        wave.position = 0;
        wave.volume_shift = 2;
        let sample_50 = wave.clock().abs();

        // 25% volume
        wave.position = 0;
        wave.volume_shift = 3;
        let sample_25 = wave.clock().abs();

        // Volume should decrease
        assert!(sample_100 > sample_50);
        assert!(sample_50 > sample_25);
    }

    #[test]
    fn wave_disabled_mutes() {
        let mut wave = WaveChannel::new();
        wave.enabled = false;
        wave.volume_shift = 1;
        wave.wave_ram[0] = 15;

        let sample = wave.clock();
        assert_eq!(sample, 0);
    }

    #[test]
    fn wave_volume_shift_zero_mutes() {
        let mut wave = WaveChannel::new();
        wave.enabled = true;
        wave.volume_shift = 0;
        wave.wave_ram[0] = 15;

        let sample = wave.clock();
        assert_eq!(sample, 0);
    }

    #[test]
    fn wave_ram_write_read() {
        let mut wave = WaveChannel::new();

        // Write a byte containing two samples
        wave.write_wave_ram_byte(0, 0xAB);
        assert_eq!(wave.wave_ram[0], 0x0A);
        assert_eq!(wave.wave_ram[1], 0x0B);

        // Read it back
        assert_eq!(wave.read_wave_ram_byte(0), 0xAB);
    }

    #[test]
    fn wave_ram_byte_format() {
        let mut wave = WaveChannel::new();

        // Write multiple bytes
        wave.write_wave_ram_byte(0, 0x12);
        wave.write_wave_ram_byte(1, 0x34);
        wave.write_wave_ram_byte(2, 0x56);

        // Check samples are stored correctly
        assert_eq!(wave.wave_ram[0], 0x01);
        assert_eq!(wave.wave_ram[1], 0x02);
        assert_eq!(wave.wave_ram[2], 0x03);
        assert_eq!(wave.wave_ram[3], 0x04);
        assert_eq!(wave.wave_ram[4], 0x05);
        assert_eq!(wave.wave_ram[5], 0x06);
    }

    /// Verify that set_timer preserves 12-bit values (needed for GB/GBA low-frequency notes).
    ///
    /// The GB/GBA wave-timer reload formula `(2048 - freq) * 2 - 1` can yield
    /// values up to 4095 (0x0FFF).  The old 11-bit mask would have corrupted
    /// these, making bass notes play at wrong (typically very high) frequencies.
    #[test]
    fn wave_set_timer_accepts_12_bit_value() {
        let mut wave = WaveChannel::new();

        // Maximum 12-bit value
        wave.set_timer(0x0FFF);
        assert_eq!(
            wave.timer_reload, 0x0FFF,
            "timer_reload should preserve 12-bit value 0x0FFF"
        );

        // A value that would have been truncated by the old 11-bit mask:
        // 0x0800 → old mask gave 0, new mask gives 0x0800.
        wave.set_timer(0x0800);
        assert_eq!(
            wave.timer_reload, 0x0800,
            "timer_reload should preserve bit 11 (0x0800)"
        );

        // Values in the 11-bit range must be preserved unchanged.
        wave.set_timer(0x07FF);
        assert_eq!(wave.timer_reload, 0x07FF);

        // Values above 12 bits must be masked.
        wave.set_timer(0x1234);
        assert_eq!(wave.timer_reload, 0x0234);
    }

    /// Verify that a 12-bit reload produces the correct position-advance period.
    ///
    /// `WaveChannel::set_timer(N)` stores N in `timer_reload` without touching the
    /// live `timer` counter.  Since `timer` is initialised to 0, the very first
    /// `clock()` call advances position and reloads `timer` to `timer_reload`.
    /// After that reload, position advances again every `timer_reload + 1` clocks:
    /// the timer counts down from `timer_reload` to 0 (= `timer_reload` decrements,
    /// each without an advance) and the advance fires on the next clock when
    /// `timer == 0`.  With `timer_reload = 0x0FFF = 4095` the period is 4096 clocks.
    #[test]
    fn wave_12_bit_timer_period() {
        let mut wave = WaveChannel::new();
        wave.enabled = true;
        wave.volume_shift = 1;
        wave.wave_ram.fill(7); // non-zero so position increments are visible
        wave.set_timer(0x0FFF); // timer_reload = 4095

        // First clock: timer == 0 → position advances (to 1) and timer reloads to 4095.
        wave.clock();
        let baseline_pos = wave.position;

        // Next 4095 clocks: timer decrements from 4095 down to 0 on the final
        // step, but no position advance occurs during a decrement.
        for _ in 0..4095 {
            wave.clock();
        }
        assert_eq!(
            wave.position, baseline_pos,
            "position must not advance while timer is counting down (4095 clocks)"
        );

        // One more clock: timer is 0 at the start → position advances, timer reloads.
        wave.clock();
        assert_eq!(
            wave.position,
            (baseline_pos + 1) & 31,
            "position must advance after 4096 clocks from the last reload"
        );
    }
}
