//! CIA 6526 Complex Interface Adapter
//!
//! The C64 has two CIAs:
//! - **CIA 1** ($DC00): Keyboard matrix, joystick port 2, IRQ generation
//! - **CIA 2** ($DD00): VIC-II bank select, serial bus, joystick port 1, NMI generation
//!
//! ## Features
//! - Two 8-bit bidirectional I/O ports (PA, PB)
//! - Two 16-bit countdown timers (Timer A, Timer B)
//! - 24-hour Time of Day clock (BCD format, 1/10 second resolution)
//! - 8-bit serial shift register
//! - Interrupt control/status register

/// CIA 6526 chip state
pub struct Cia {
    /// Port A data register (directly written/read values)
    pub port_a: u8,
    /// Port B data register
    pub port_b: u8,
    /// Data Direction Register A (1=output, 0=input)
    pub ddr_a: u8,
    /// Data Direction Register B
    pub ddr_b: u8,

    /// Timer A counter (16-bit, decrements)
    timer_a_counter: u16,
    /// Timer A latch (reload value)
    timer_a_latch: u16,
    /// Timer A running flag
    timer_a_running: bool,
    /// Timer A one-shot mode (true=one-shot, false=continuous)
    timer_a_oneshot: bool,

    /// Timer B counter
    timer_b_counter: u16,
    /// Timer B latch
    timer_b_latch: u16,
    /// Timer B running flag
    timer_b_running: bool,
    /// Timer B one-shot mode
    timer_b_oneshot: bool,
    /// Timer B count mode: false=PHI2 clocks, true=Timer A underflows
    timer_b_count_ta: bool,

    /// Control Register A (raw byte for reads)
    cra: u8,
    /// Control Register B (raw byte for reads)
    crb: u8,

    /// Interrupt Control Register - pending interrupt flags (bits 0-4)
    icr_data: u8,
    /// Interrupt Mask Register - which sources trigger the IRQ pin (bits 0-4)
    icr_mask: u8,

    /// Whether the CIA's IRQ/NMI output pin is active
    pub irq_line: bool,

    /// Time of Day registers (BCD: 1/10s, seconds, minutes, hours)
    tod: [u8; 4],
    /// TOD alarm registers
    tod_alarm: [u8; 4],
    /// TOD running flag
    tod_running: bool,
    /// TOD latch (for reading stable values)
    tod_latched: bool,
    tod_latch: [u8; 4],

    /// Keyboard matrix (8 rows; active-low: 0 = key pressed)
    /// CIA 1 only - indexed by column selection
    pub keyboard_matrix: [u8; 8],

    /// Serial shift register
    sdr: u8,
}

impl Cia {
    pub fn new() -> Self {
        Self {
            port_a: 0xFF,
            port_b: 0xFF,
            ddr_a: 0,
            ddr_b: 0,
            timer_a_counter: 0xFFFF,
            timer_a_latch: 0xFFFF,
            timer_a_running: false,
            timer_a_oneshot: false,
            timer_b_counter: 0xFFFF,
            timer_b_latch: 0xFFFF,
            timer_b_running: false,
            timer_b_oneshot: false,
            timer_b_count_ta: false,
            cra: 0,
            crb: 0,
            icr_data: 0,
            icr_mask: 0,
            irq_line: false,
            tod: [0; 4],
            tod_alarm: [0; 4],
            tod_running: true,
            tod_latched: false,
            tod_latch: [0; 4],
            keyboard_matrix: [0xFF; 8],
            sdr: 0,
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Read CIA register (0x00–0x0F)
    pub fn read(&mut self, reg: u8) -> u8 {
        match reg & 0x0F {
            0x00 => {
                // Port A: output DDR bits from port_a, input bits from external
                // For CIA1: PA is keyboard column select / joystick 2
                (self.port_a & self.ddr_a) | !self.ddr_a
            }
            0x01 => {
                // Port B: For CIA1, keyboard row read with matrix scan
                let col_select = self.port_a | !self.ddr_a;
                let mut result = 0xFF_u8;
                for col in 0..8 {
                    if col_select & (1 << col) == 0 {
                        // This column is driven low - read corresponding rows
                        result &= self.keyboard_matrix[col];
                    }
                }
                // AND with port_b for output bits
                (result & !self.ddr_b) | (self.port_b & self.ddr_b)
            }
            0x02 => self.ddr_a,
            0x03 => self.ddr_b,
            0x04 => (self.timer_a_counter & 0xFF) as u8,
            0x05 => (self.timer_a_counter >> 8) as u8,
            0x06 => (self.timer_b_counter & 0xFF) as u8,
            0x07 => (self.timer_b_counter >> 8) as u8,
            0x08 => {
                // TOD 1/10 seconds - reading this unlatches
                let val = if self.tod_latched {
                    self.tod_latch[0]
                } else {
                    self.tod[0]
                };
                self.tod_latched = false;
                val
            }
            0x09 => {
                if self.tod_latched {
                    self.tod_latch[1]
                } else {
                    self.tod[1]
                }
            }
            0x0A => {
                if self.tod_latched {
                    self.tod_latch[2]
                } else {
                    self.tod[2]
                }
            }
            0x0B => {
                // TOD hours - reading this latches all TOD regs
                self.tod_latched = true;
                self.tod_latch = self.tod;
                self.tod[3]
            }
            0x0C => self.sdr,
            0x0D => {
                // ICR: read clears all flags and releases IRQ line
                let val = self.icr_data;
                self.icr_data = 0;
                self.irq_line = false;
                val | if val & self.icr_mask != 0 { 0x80 } else { 0 }
            }
            0x0E => self.cra,
            0x0F => self.crb,
            _ => 0,
        }
    }

    /// Write CIA register
    pub fn write(&mut self, reg: u8, val: u8) {
        match reg & 0x0F {
            0x00 => self.port_a = val,
            0x01 => self.port_b = val,
            0x02 => self.ddr_a = val,
            0x03 => self.ddr_b = val,
            0x04 => self.timer_a_latch = (self.timer_a_latch & 0xFF00) | val as u16,
            0x05 => {
                self.timer_a_latch = (self.timer_a_latch & 0x00FF) | ((val as u16) << 8);
                // If timer not running, writing high byte loads counter
                if !self.timer_a_running {
                    self.timer_a_counter = self.timer_a_latch;
                }
            }
            0x06 => self.timer_b_latch = (self.timer_b_latch & 0xFF00) | val as u16,
            0x07 => {
                self.timer_b_latch = (self.timer_b_latch & 0x00FF) | ((val as u16) << 8);
                if !self.timer_b_running {
                    self.timer_b_counter = self.timer_b_latch;
                }
            }
            0x08 => self.tod[0] = val & 0x0F,
            0x09 => self.tod[1] = val & 0x7F,
            0x0A => self.tod[2] = val & 0x7F,
            0x0B => self.tod[3] = val & 0x9F,
            0x0C => self.sdr = val,
            0x0D => {
                // ICR mask: bit 7 = set/clear
                if val & 0x80 != 0 {
                    self.icr_mask |= val & 0x1F;
                } else {
                    self.icr_mask &= !(val & 0x1F);
                }
                // Re-evaluate IRQ
                if self.icr_data & self.icr_mask != 0 {
                    self.irq_line = true;
                }
            }
            0x0E => {
                self.cra = val & 0xEF; // Bit 4 (force load) is strobe
                self.timer_a_running = val & 0x01 != 0;
                self.timer_a_oneshot = val & 0x08 != 0;
                if val & 0x10 != 0 {
                    // Force load
                    self.timer_a_counter = self.timer_a_latch;
                }
            }
            0x0F => {
                self.crb = val & 0xEF;
                self.timer_b_running = val & 0x01 != 0;
                self.timer_b_oneshot = val & 0x08 != 0;
                self.timer_b_count_ta = (val >> 5) & 0x03 == 0x01;
                if val & 0x10 != 0 {
                    self.timer_b_counter = self.timer_b_latch;
                }
            }
            _ => {}
        }
    }

    /// Tick CIA by given number of CPU cycles. Returns true if timer underflow occurred.
    pub fn tick(&mut self, cycles: u32) {
        let mut ta_underflow = false;

        for _ in 0..cycles {
            // Timer A
            if self.timer_a_running {
                if self.timer_a_counter == 0 {
                    self.timer_a_counter = self.timer_a_latch;
                    ta_underflow = true;
                    // Set Timer A underflow flag
                    self.icr_data |= 0x01;
                    if self.icr_mask & 0x01 != 0 {
                        self.irq_line = true;
                    }
                    if self.timer_a_oneshot {
                        self.timer_a_running = false;
                    }
                } else {
                    self.timer_a_counter -= 1;
                }
            }

            // Timer B
            if self.timer_b_running {
                let tick_b = if self.timer_b_count_ta {
                    ta_underflow
                } else {
                    true
                };

                if tick_b {
                    if self.timer_b_counter == 0 {
                        self.timer_b_counter = self.timer_b_latch;
                        self.icr_data |= 0x02;
                        if self.icr_mask & 0x02 != 0 {
                            self.irq_line = true;
                        }
                        if self.timer_b_oneshot {
                            self.timer_b_running = false;
                        }
                    } else {
                        self.timer_b_counter -= 1;
                    }
                }
            }

            ta_underflow = false;
        }
    }

    /// Set a key in the keyboard matrix
    /// row: 0–7, col: 0–7, pressed: true if pressed
    pub fn set_key(&mut self, row: u8, col: u8, pressed: bool) {
        if row < 8 && col < 8 {
            if pressed {
                self.keyboard_matrix[col as usize] &= !(1 << row);
            } else {
                self.keyboard_matrix[col as usize] |= 1 << row;
            }
        }
    }
}

impl Default for Cia {
    fn default() -> Self {
        Self::new()
    }
}
