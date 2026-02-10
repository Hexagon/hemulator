//! GBA hardware timers.
//!
//! The GBA has 4 independent 16-bit timers (Timer 0–3) that count up at
//! configurable rates. Each timer can:
//! - Count at CPU_FREQ/1, CPU_FREQ/64, CPU_FREQ/256, or CPU_FREQ/1024
//! - Cascade from the previous timer (increment when timer N-1 overflows)
//! - Fire an IRQ on overflow
//! - Reload a preset value on overflow
//!
//! ## I/O Registers
//!
//! | Address    | Name     | Description              |
//! |------------|----------|--------------------------|
//! | 0x04000100 | TM0CNT_L | Timer 0 Counter/Reload   |
//! | 0x04000102 | TM0CNT_H | Timer 0 Control          |
//! | 0x04000104 | TM1CNT_L | Timer 1 Counter/Reload   |
//! | 0x04000106 | TM1CNT_H | Timer 1 Control          |
//! | 0x04000108 | TM2CNT_L | Timer 2 Counter/Reload   |
//! | 0x0400010A | TM2CNT_H | Timer 2 Control          |
//! | 0x0400010C | TM3CNT_L | Timer 3 Counter/Reload   |
//! | 0x0400010E | TM3CNT_H | Timer 3 Control          |
//!
//! ## Control Register (TMxCNT_H)
//!
//! | Bit   | Description                           |
//! |-------|---------------------------------------|
//! | 0-1   | Prescaler (0=F/1, 1=F/64, 2=F/256, 3=F/1024) |
//! | 2     | Count-Up Timing (cascade from previous timer) |
//! | 6     | Timer IRQ Enable                      |
//! | 7     | Timer Start/Stop (0=Stop, 1=Start)    |

/// Number of hardware timers
const NUM_TIMERS: usize = 4;

/// Prescaler divider values indexed by bits 0-1 of control register
const PRESCALER_DIVIDERS: [u32; 4] = [1, 64, 256, 1024];

// Control register bit masks
const TIMER_CTRL_PRESCALER: u8 = 0x03;
const TIMER_CTRL_CASCADE: u8 = 1 << 2;
const TIMER_CTRL_IRQ_ENABLE: u8 = 1 << 6;
const TIMER_CTRL_ENABLED: u8 = 1 << 7;

// IRQ bits for each timer (bits 3-6 of the IE/IF registers)
const TIMER_IRQ_BITS: [u16; NUM_TIMERS] = [
    1 << 3, // Timer 0
    1 << 4, // Timer 1
    1 << 5, // Timer 2
    1 << 6, // Timer 3
];

/// State of a single GBA timer
#[derive(Debug, Clone)]
struct Timer {
    /// Current 16-bit counter value
    counter: u16,
    /// Reload value (written to TM*CNT_L)
    reload: u16,
    /// Control register (TM*CNT_H)
    control: u8,
    /// Internal prescaler clock accumulator (sub-tick fractional cycles)
    prescaler_counter: u32,
}

impl Timer {
    fn new() -> Self {
        Self {
            counter: 0,
            reload: 0,
            control: 0,
            prescaler_counter: 0,
        }
    }

    /// Whether this timer is running
    #[inline]
    fn is_enabled(&self) -> bool {
        self.control & TIMER_CTRL_ENABLED != 0
    }

    /// Whether this timer cascades (counts overflows from previous timer)
    #[inline]
    fn is_cascade(&self) -> bool {
        self.control & TIMER_CTRL_CASCADE != 0
    }

    /// Whether this timer fires an IRQ on overflow
    #[inline]
    fn irq_enabled(&self) -> bool {
        self.control & TIMER_CTRL_IRQ_ENABLE != 0
    }

    /// Get the prescaler divider for this timer
    #[inline]
    fn prescaler(&self) -> u32 {
        PRESCALER_DIVIDERS[(self.control & TIMER_CTRL_PRESCALER) as usize]
    }

    /// Reset prescaler accumulator (done when timer is started)
    fn reset_prescaler(&mut self) {
        self.prescaler_counter = 0;
    }
}

/// GBA timer subsystem managing all 4 hardware timers
#[derive(Debug, Clone)]
pub struct Timers {
    timers: [Timer; NUM_TIMERS],
}

impl Default for Timers {
    fn default() -> Self {
        Self::new()
    }
}

impl Timers {
    /// Create a new timer subsystem with all timers stopped
    pub fn new() -> Self {
        Self {
            timers: [Timer::new(), Timer::new(), Timer::new(), Timer::new()],
        }
    }

    /// Reset all timers to initial state
    pub fn reset(&mut self) {
        for timer in &mut self.timers {
            *timer = Timer::new();
        }
    }

    /// Tick all timers by the given number of CPU cycles.
    ///
    /// Returns a bitmask of IRQs to fire (bits 3-6 for timers 0-3).
    /// The caller is responsible for calling `request_interrupt()` with these bits.
    pub fn tick(&mut self, cycles: u32) -> u16 {
        let mut irq_flags: u16 = 0;

        // Process each timer. Cascaded timers are handled via overflow propagation.
        // We need to track overflows from the previous timer for cascade logic.
        let mut prev_overflows: u32 = 0;

        for (i, &irq_bit) in TIMER_IRQ_BITS.iter().enumerate() {
            if !self.timers[i].is_enabled() {
                // Timer is stopped - no ticking, no overflow propagation
                prev_overflows = 0;
                continue;
            }

            let overflows = if self.timers[i].is_cascade() && i > 0 {
                // Cascade mode: increment counter by the number of overflows
                // from the previous timer
                self.tick_timer_cascade(i, prev_overflows)
            } else {
                // Normal mode: tick based on prescaler
                self.tick_timer_prescaled(i, cycles)
            };

            // Fire IRQ if enabled and timer overflowed
            if overflows > 0 && self.timers[i].irq_enabled() {
                irq_flags |= irq_bit;
            }

            prev_overflows = overflows;
        }

        irq_flags
    }

    /// Tick a timer using its prescaler divider.
    /// Returns the number of times the timer overflowed.
    fn tick_timer_prescaled(&mut self, index: usize, cycles: u32) -> u32 {
        let timer = &mut self.timers[index];
        let divider = timer.prescaler();

        // Add cycles to prescaler accumulator
        timer.prescaler_counter += cycles;

        // How many actual ticks does this produce?
        let ticks = timer.prescaler_counter / divider;
        timer.prescaler_counter %= divider;

        if ticks == 0 {
            return 0;
        }

        // Calculate overflows
        let remaining_until_overflow = 0x10000u32 - timer.counter as u32;

        if ticks < remaining_until_overflow {
            // No overflow, just increment
            timer.counter = timer.counter.wrapping_add(ticks as u16);
            0
        } else {
            // At least one overflow
            let ticks_after_first_overflow = ticks - remaining_until_overflow;
            let reload = timer.reload as u32;
            let ticks_per_cycle = 0x10000u32 - reload;

            if ticks_per_cycle == 0 {
                // Reload == 0xFFFF+1, overflows every tick after first
                // This is a degenerate case
                let overflows = 1 + ticks_after_first_overflow;
                timer.counter = timer.reload;
                overflows
            } else {
                let additional_overflows = ticks_after_first_overflow / ticks_per_cycle;
                let remaining_ticks = ticks_after_first_overflow % ticks_per_cycle;
                timer.counter = (reload + remaining_ticks) as u16;
                1 + additional_overflows
            }
        }
    }

    /// Tick a cascaded timer by a number of overflow events from the previous timer.
    /// Returns the number of times this timer overflowed.
    fn tick_timer_cascade(&mut self, index: usize, overflows_from_prev: u32) -> u32 {
        if overflows_from_prev == 0 {
            return 0;
        }

        let timer = &mut self.timers[index];
        let remaining_until_overflow = 0x10000u32 - timer.counter as u32;

        if overflows_from_prev < remaining_until_overflow {
            timer.counter = timer.counter.wrapping_add(overflows_from_prev as u16);
            0
        } else {
            let ticks_after_first = overflows_from_prev - remaining_until_overflow;
            let reload = timer.reload as u32;
            let ticks_per_cycle = 0x10000u32 - reload;

            if ticks_per_cycle == 0 {
                let total_overflows = 1 + ticks_after_first;
                timer.counter = timer.reload;
                total_overflows
            } else {
                let additional_overflows = ticks_after_first / ticks_per_cycle;
                let remaining = ticks_after_first % ticks_per_cycle;
                timer.counter = (reload + remaining) as u16;
                1 + additional_overflows
            }
        }
    }

    /// Read a timer I/O register byte.
    ///
    /// Addresses are relative offsets from 0x04000100:
    /// - 0x00-0x01: TM0CNT_L (counter, read-only)
    /// - 0x02-0x03: TM0CNT_H (control)
    /// - 0x04-0x07: TM1 registers
    /// - 0x08-0x0B: TM2 registers
    /// - 0x0C-0x0F: TM3 registers
    pub fn read(&self, offset: u32) -> u8 {
        let timer_index = (offset / 4) as usize;
        let reg_offset = offset & 3;

        if timer_index >= NUM_TIMERS {
            return 0;
        }

        let timer = &self.timers[timer_index];

        match reg_offset {
            // TMxCNT_L: read returns current counter value (not reload)
            0 => timer.counter as u8,
            1 => (timer.counter >> 8) as u8,
            // TMxCNT_H: control register
            2 => timer.control,
            3 => 0, // Upper byte of control is unused
            _ => 0,
        }
    }

    /// Write a timer I/O register byte.
    ///
    /// Addresses are relative offsets from 0x04000100.
    pub fn write(&mut self, offset: u32, val: u8) {
        let timer_index = (offset / 4) as usize;
        let reg_offset = offset & 3;

        if timer_index >= NUM_TIMERS {
            return;
        }

        match reg_offset {
            // TMxCNT_L: write sets reload value (not counter directly)
            0 => {
                self.timers[timer_index].reload =
                    (self.timers[timer_index].reload & 0xFF00) | val as u16;
            }
            1 => {
                self.timers[timer_index].reload =
                    (self.timers[timer_index].reload & 0x00FF) | ((val as u16) << 8);
            }
            // TMxCNT_H: control register
            2 => {
                let old_enabled = self.timers[timer_index].is_enabled();
                self.timers[timer_index].control = val;
                let new_enabled = self.timers[timer_index].is_enabled();

                // When timer transitions from stopped to started:
                // - Counter is reloaded from reload value
                // - Prescaler counter is reset
                if !old_enabled && new_enabled {
                    self.timers[timer_index].counter = self.timers[timer_index].reload;
                    self.timers[timer_index].reset_prescaler();
                }
            }
            3 => {} // Upper byte of control is unused
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timer_new() {
        let timers = Timers::new();
        for i in 0..4 {
            assert_eq!(timers.read(i * 4), 0); // counter lo
            assert_eq!(timers.read(i * 4 + 1), 0); // counter hi
            assert_eq!(timers.read(i * 4 + 2), 0); // control
        }
    }

    #[test]
    fn test_timer_reload_on_start() {
        let mut timers = Timers::new();

        // Set reload value to 0x8000
        timers.write(0, 0x00); // TM0CNT_L low
        timers.write(1, 0x80); // TM0CNT_L high

        // Counter should still be 0 (reload doesn't affect counter directly)
        assert_eq!(timers.read(0), 0x00);
        assert_eq!(timers.read(1), 0x00);

        // Start timer (bit 7 = enable)
        timers.write(2, TIMER_CTRL_ENABLED);

        // Counter should now be loaded with reload value
        assert_eq!(timers.read(0), 0x00);
        assert_eq!(timers.read(1), 0x80);
    }

    #[test]
    fn test_timer_prescaler_1() {
        let mut timers = Timers::new();

        // Set reload to 0, prescaler = F/1, enable
        timers.write(0, 0x00);
        timers.write(1, 0x00);
        timers.write(2, TIMER_CTRL_ENABLED); // prescaler=0 (F/1)

        // Tick 100 cycles
        timers.tick(100);

        // Counter should be 100
        let counter = timers.read(0) as u16 | ((timers.read(1) as u16) << 8);
        assert_eq!(counter, 100);
    }

    #[test]
    fn test_timer_prescaler_64() {
        let mut timers = Timers::new();

        // prescaler = F/64
        timers.write(0, 0x00);
        timers.write(1, 0x00);
        timers.write(2, TIMER_CTRL_ENABLED | 0x01); // prescaler=1 (F/64)

        // Tick 128 cycles = 2 timer ticks at F/64
        timers.tick(128);

        let counter = timers.read(0) as u16 | ((timers.read(1) as u16) << 8);
        assert_eq!(counter, 2);
    }

    #[test]
    fn test_timer_prescaler_256() {
        let mut timers = Timers::new();

        // prescaler = F/256
        timers.write(0, 0x00);
        timers.write(1, 0x00);
        timers.write(2, TIMER_CTRL_ENABLED | 0x02); // prescaler=2 (F/256)

        // Tick 512 cycles = 2 timer ticks at F/256
        timers.tick(512);

        let counter = timers.read(0) as u16 | ((timers.read(1) as u16) << 8);
        assert_eq!(counter, 2);
    }

    #[test]
    fn test_timer_prescaler_1024() {
        let mut timers = Timers::new();

        // prescaler = F/1024
        timers.write(0, 0x00);
        timers.write(1, 0x00);
        timers.write(2, TIMER_CTRL_ENABLED | 0x03); // prescaler=3 (F/1024)

        // Tick 2048 cycles = 2 timer ticks at F/1024
        timers.tick(2048);

        let counter = timers.read(0) as u16 | ((timers.read(1) as u16) << 8);
        assert_eq!(counter, 2);
    }

    #[test]
    fn test_timer_overflow_and_reload() {
        let mut timers = Timers::new();

        // Set reload to 0xFFF0 (will overflow after 16 ticks)
        timers.write(0, 0xF0);
        timers.write(1, 0xFF);
        timers.write(2, TIMER_CTRL_ENABLED); // F/1

        // Tick 20 cycles: 16 to overflow + 4 more from reload
        let irqs = timers.tick(20);

        // No IRQ (not enabled)
        assert_eq!(irqs, 0);

        // Counter should be reload + 4 = 0xFFF0 + 4 = 0xFFF4
        let counter = timers.read(0) as u16 | ((timers.read(1) as u16) << 8);
        assert_eq!(counter, 0xFFF4);
    }

    #[test]
    fn test_timer_irq_on_overflow() {
        let mut timers = Timers::new();

        // Set reload to 0xFFFF (overflow after 1 tick)
        timers.write(0, 0xFF);
        timers.write(1, 0xFF);
        // Enable timer + IRQ
        timers.write(2, TIMER_CTRL_ENABLED | TIMER_CTRL_IRQ_ENABLE);

        // Tick 1 to overflow
        let irqs = timers.tick(1);

        // Should have Timer 0 IRQ (bit 3)
        assert_eq!(irqs, TIMER_IRQ_BITS[0]);
    }

    #[test]
    fn test_timer_no_irq_when_disabled() {
        let mut timers = Timers::new();

        // Set reload to 0xFFFF (overflow after 1 tick)
        timers.write(0, 0xFF);
        timers.write(1, 0xFF);
        // Enable timer but NOT IRQ
        timers.write(2, TIMER_CTRL_ENABLED);

        let irqs = timers.tick(1);
        assert_eq!(irqs, 0);
    }

    #[test]
    fn test_timer_stopped_does_not_tick() {
        let mut timers = Timers::new();

        // Don't enable timer
        timers.write(0, 0x00);
        timers.write(1, 0x00);
        // Control = 0 (stopped)

        timers.tick(1000);

        let counter = timers.read(0) as u16 | ((timers.read(1) as u16) << 8);
        assert_eq!(counter, 0);
    }

    #[test]
    fn test_timer_cascade() {
        let mut timers = Timers::new();

        // Timer 0: reload=0xFFFF, F/1, enabled (overflows every tick)
        timers.write(0, 0xFF); // TM0 reload lo
        timers.write(1, 0xFF); // TM0 reload hi
        timers.write(2, TIMER_CTRL_ENABLED); // TM0 control

        // Timer 1: reload=0x0000, cascade from TM0, enabled
        timers.write(4, 0x00); // TM1 reload lo
        timers.write(5, 0x00); // TM1 reload hi
        timers.write(6, TIMER_CTRL_ENABLED | TIMER_CTRL_CASCADE); // TM1 control

        // Tick 10 cycles: TM0 overflows 10 times, TM1 should increment by 10
        timers.tick(10);

        let tm1_counter = timers.read(4) as u16 | ((timers.read(5) as u16) << 8);
        assert_eq!(tm1_counter, 10);
    }

    #[test]
    fn test_timer_cascade_chain() {
        let mut timers = Timers::new();

        // Timer 0: reload=0xFFFF, F/1 (overflows every tick)
        timers.write(0, 0xFF);
        timers.write(1, 0xFF);
        timers.write(2, TIMER_CTRL_ENABLED);

        // Timer 1: reload=0xFFFE, cascade (overflows after 2 increments from TM0)
        timers.write(4, 0xFE);
        timers.write(5, 0xFF);
        timers.write(6, TIMER_CTRL_ENABLED | TIMER_CTRL_CASCADE);

        // Timer 2: reload=0x0000, cascade (counts TM1 overflows)
        timers.write(8, 0x00);
        timers.write(9, 0x00);
        timers.write(10, TIMER_CTRL_ENABLED | TIMER_CTRL_CASCADE);

        // TM0 overflows on every tick.
        // TM1 reload=0xFFFE, so it takes 2 TM0 overflows to overflow TM1.
        // Tick 4: TM0 overflows 4x, TM1 overflows 2x, TM2 increments by 2.
        timers.tick(4);

        let tm2_counter = timers.read(8) as u16 | ((timers.read(9) as u16) << 8);
        assert_eq!(tm2_counter, 2);
    }

    #[test]
    fn test_timer_cascade_timer0_cannot_cascade() {
        let mut timers = Timers::new();

        // Timer 0 with cascade bit set should still use prescaler (no previous timer)
        timers.write(0, 0x00);
        timers.write(1, 0x00);
        timers.write(2, TIMER_CTRL_ENABLED | TIMER_CTRL_CASCADE);

        // Tick 100 cycles - should tick since cascade bit is ignored on TM0
        timers.tick(100);

        let counter = timers.read(0) as u16 | ((timers.read(1) as u16) << 8);
        assert_eq!(counter, 100);
    }

    #[test]
    fn test_timer_read_write_registers() {
        let mut timers = Timers::new();

        // Write reload value
        timers.write(0, 0x34);
        timers.write(1, 0x12);

        // Reading counter should return 0 (timer not started, reload not applied)
        assert_eq!(timers.read(0), 0x00);
        assert_eq!(timers.read(1), 0x00);

        // Start timer
        timers.write(2, TIMER_CTRL_ENABLED);

        // Counter should now show reload value
        assert_eq!(timers.read(0), 0x34);
        assert_eq!(timers.read(1), 0x12);

        // Read control register
        assert_eq!(timers.read(2), TIMER_CTRL_ENABLED);

        // Upper control byte is unused
        assert_eq!(timers.read(3), 0);
    }

    #[test]
    fn test_timer_reset() {
        let mut timers = Timers::new();

        // Set up and run a timer
        timers.write(0, 0xFF);
        timers.write(1, 0xFF);
        timers.write(2, TIMER_CTRL_ENABLED);
        timers.tick(100);

        // Reset
        timers.reset();

        // All should be zero
        for i in 0..4 {
            assert_eq!(timers.read(i * 4), 0);
            assert_eq!(timers.read(i * 4 + 1), 0);
            assert_eq!(timers.read(i * 4 + 2), 0);
        }
    }

    #[test]
    fn test_timer_multiple_overflows() {
        let mut timers = Timers::new();

        // Reload=0xFFF0 (overflows every 16 ticks)
        timers.write(0, 0xF0);
        timers.write(1, 0xFF);
        timers.write(2, TIMER_CTRL_ENABLED | TIMER_CTRL_IRQ_ENABLE);

        // Tick 50 cycles: 16 to first overflow, then 34 remaining
        // 34 / 16 = 2 more overflows + 2 remaining
        let irqs = timers.tick(50);

        // IRQ should fire (at least one overflow)
        assert_eq!(irqs, TIMER_IRQ_BITS[0]);

        // Counter should be 0xFFF0 + 2 = 0xFFF2
        let counter = timers.read(0) as u16 | ((timers.read(1) as u16) << 8);
        assert_eq!(counter, 0xFFF2);
    }

    #[test]
    fn test_timer_prescaler_accumulation() {
        let mut timers = Timers::new();

        // F/64 prescaler
        timers.write(0, 0x00);
        timers.write(1, 0x00);
        timers.write(2, TIMER_CTRL_ENABLED | 0x01);

        // Tick 32 cycles (half a prescaler period)
        timers.tick(32);
        let c1 = timers.read(0) as u16 | ((timers.read(1) as u16) << 8);
        assert_eq!(c1, 0); // Not enough for a tick

        // Tick 32 more (total 64 = one prescaler period)
        timers.tick(32);
        let c2 = timers.read(0) as u16 | ((timers.read(1) as u16) << 8);
        assert_eq!(c2, 1); // Now we have one tick
    }

    #[test]
    fn test_timer_irq_bits_correct() {
        // Verify IRQ bits match GBA spec
        assert_eq!(TIMER_IRQ_BITS[0], 0x0008); // Bit 3
        assert_eq!(TIMER_IRQ_BITS[1], 0x0010); // Bit 4
        assert_eq!(TIMER_IRQ_BITS[2], 0x0020); // Bit 5
        assert_eq!(TIMER_IRQ_BITS[3], 0x0040); // Bit 6
    }

    #[test]
    fn test_all_timers_independent() {
        let mut timers = Timers::new();

        // Start all 4 timers with different prescalers
        for i in 0..4u32 {
            timers.write(i * 4, 0x00); // reload lo
            timers.write(i * 4 + 1, 0x00); // reload hi
            timers.write(i * 4 + 2, TIMER_CTRL_ENABLED | (i as u8 & 0x03)); // prescaler = i
        }

        timers.tick(1024);

        // TM0 (F/1): counter = 1024
        let c0 = timers.read(0) as u16 | ((timers.read(1) as u16) << 8);
        assert_eq!(c0, 1024);

        // TM1 (F/64): counter = 1024/64 = 16
        let c1 = timers.read(4) as u16 | ((timers.read(5) as u16) << 8);
        assert_eq!(c1, 16);

        // TM2 (F/256): counter = 1024/256 = 4
        let c2 = timers.read(8) as u16 | ((timers.read(9) as u16) << 8);
        assert_eq!(c2, 4);

        // TM3 (F/1024): counter = 1024/1024 = 1
        let c3 = timers.read(12) as u16 | ((timers.read(13) as u16) << 8);
        assert_eq!(c3, 1);
    }
}
