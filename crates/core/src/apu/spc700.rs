//! SPC700 Audio Processing Unit chip
//!
//! This module implements the SPC700 CPU, RAM, timers, and I/O ports that form
//! the SNES Audio Processing Unit. The S-DSP (Digital Signal Processor) audio
//! generation is now **partially implemented** with basic voice synthesis.
//!
//! **Architecture:**
//! - SPC700 CPU (8-bit, 256 opcodes) - ✅ **Fully implemented**
//! - 64KB RAM - ✅ **Fully implemented**
//! - 64-byte IPL boot ROM ($FFC0-$FFFF, can be disabled) - ✅ **Fully implemented**
//! - 4 communication ports ($F4-$F7) for CPU<->APU communication - ✅ **Fully implemented**
//! - 3 timers (8-bit counters with programmable periods) - ✅ **Fully implemented**
//! - DSP register interface ($F2/$F3, 128 registers) - ✅ **Fully implemented**
//! - S-DSP audio processing (8-channel ADPCM, ADSR, echo, etc.) - 🚧 **Partially implemented**
//!
//! **Implementation Status:**
//!
//! ✅ **Implemented:**
//! - SPC700 CPU with all 256 opcodes
//! - IPL ROM boot sequence and data upload protocol
//! - Bidirectional communication ports with main CPU
//! - Three timers with correct frequencies (8 kHz for T0/T1, 64 kHz for T2)
//! - Control register for IPL ROM, timers, and port clearing
//! - DSP register read/write interface
//! - DSP voice control (key on/off, volume, pitch)
//! - Basic envelope generation (ADSR and GAIN modes)
//! - Voice mixing to stereo output
//!
//! 🚧 **Partially Implemented:**
//! - BRR (ADPCM) sample decoder (structure in place, needs RAM access)
//! - Sample playback (stub interpolation)
//! - Envelope curves (simplified)
//!
//! ❌ **NOT Yet Implemented:**
//! - BRR sample fetching from RAM
//! - Gaussian interpolation filter
//! - Echo/reverb FIR filter
//! - Noise generator
//! - Pitch modulation
//! - Accurate envelope rates
//!
//! **Result:** Audio drivers execute correctly and DSP accepts register writes.
//! Basic audio output is generated (simplified envelopes and mixing).
//!
//! **Communication Protocol:**
//! The IPL ROM implements a boot protocol where it waits for the main CPU
//! to upload code via the communication ports, then executes it.
//!
//! **References:**
//! - [SPC700 Reference](https://wiki.superfamicom.org/spc700-reference)
//! - [Transferring Data to APU](https://wiki.superfamicom.org/transferring-data-from-rom-to-the-snes-apu)
//! - [Fullsnes Documentation](https://problemkaputt.de/fullsnes.htm#snescpuspc700audiosystemapu)
//! - [S-SMP - SNESdev Wiki](https://snes.nesdev.org/wiki/S-SMP)
//! - [SPC-700 Instruction Set - SNESdev Wiki](https://snes.nesdev.org/wiki/SPC-700_instruction_set)
//! - [S-DSP Registers - SNESdev Wiki](https://snes.nesdev.org/wiki/S-DSP_registers)
//! - [DSP Envelopes - SNESdev Wiki](https://snes.nesdev.org/wiki/DSP_envelopes)

use std::cell::Cell;

use crate::apu::{AudioChip, Dsp, TimingMode};
use crate::cpu_spc700::{CpuSpc700, MemorySpc700};
use crate::logging::{log, LogCategory, LogLevel};

/// SPC700 I/O register addresses
const TEST_REG: u16 = 0x00F0; // Test register
const CONTROL_REG: u16 = 0x00F1; // Control register (timer enables, IPL ROM enable)
const DSP_ADDR: u16 = 0x00F2; // DSP address register
const DSP_DATA: u16 = 0x00F3; // DSP data register
const CPUIO0: u16 = 0x00F4; // Communication port 0
#[allow(dead_code)]
const CPUIO1: u16 = 0x00F5; // Communication port 1
#[allow(dead_code)]
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
/// This is the actual SPC700 IPL ROM from hardware
/// The clear loop intentionally clears ports $F4-$F7, then rewrites $F4/$F5 with $AA/$BB
const IPL_ROM: [u8; 64] = [
    // Real SPC700 IPL ROM (verified from hardware dumps and documentation)
    // Source: Anomie's SPC700 documentation, SnesLab, multiple emulators
    0xCD, 0xEF, // $FFC0: MOV X, #$EF       - Set X to $EF
    0xBD, // $FFC2: MOV SP, X         - Set stack pointer to $EF
    0xE8, 0x00, // $FFC3: MOV A, #$00       - A = 0
    0xC6, // $FFC5: MOV (X), A        - Clear memory from $EF down
    0x1D, // $FFC6: DEC X             - Decrement X
    0xD0, 0xFC, // $FFC7: BNE $FFC5         - Loop until X = 0 (clears $00-$EF)
    0x8F, 0xAA, 0xF4, // $FFC9: MOV $F4, #$AA     - Write $AA to port $F4
    0x8F, 0xBB, 0xF5, // $FFCC: MOV $F5, #$BB     - Write $BB to port $F5 (ready signature)
    0x78, 0xCC, 0xF4, // $FFCF: CMP $F4, #$CC     - Wait for $CC in port $F4
    0xD0, 0xFB, // $FFD2: BNE $FFCF         - Loop until $CC received
    0x2F, 0x19, // $FFD4: BRA $FFEF         - Branch to entry point setup
    0xEB, 0xF4, // $FFD6: MOV Y, $F4        - Upload loop: Y = index from port
    0xD0, 0xFC, // $FFD8: BNE $FFD6         - Loop while Y = 0
    0x7E, 0xF4, // $FFDA: CMP Y, $F4        - Compare Y with port (wait for match)
    0xD0, 0x0B, // $FFDC: BNE $FFE9         - If not equal, go to $FFE9
    0xE4, 0xF5, // $FFDE: MOV A, $F5        - Read data byte from port $F5
    0xCB, 0xF4, // $FFE0: MOV $F4, Y        - Echo index to port $F4 (acknowledge)
    0xD7, 0x00, // $FFE2: MOV ($00)+Y, A    - Store byte at (ZP+Y)
    0xFC, // $FFE4: INC Y             - Increment Y
    0xD0, 0xF3, // $FFE5: BNE $FFDA         - Loop if Y != 0
    0xAB, 0x01, // $FFE7: INC $01           - Increment high byte of address
    0x10, 0xEF, // $FFE9: BPL $FFDA         - Continue if bit 7 clear
    0x7E, 0xF4, // $FFEB: CMP Y, $F4        - Final check
    0x10, 0xEB, // $FFED: BPL $FFDA         - Continue if positive
    0xBA, 0xF6, // $FFEF: MOVW YA, $F6      - Read 16-bit entry point from ports $F6/$F7
    0xDA, 0x00, // $FFF1: MOVW $00, YA      - Store entry point at $0000-$0001
    0xBA, 0xF4, // $FFF3: MOVW YA, $F4      - Read 16-bit value from ports $F4/$F5
    0xC4, 0xF4, // $FFF5: MOV $F4, A        - Echo low byte to port $F4
    0xDD, // $FFF7: MOV A, Y          - A = Y (high byte)
    0x5D, // $FFF8: MOV X, A          - X = A
    0xD0, 0xDB, // $FFF9: BNE $FFD6         - If X != 0, go to upload loop
    0x1F, 0x00, 0x00, // $FFFB: JMP [$0000+X]     - Jump indirect to address at $0000+X
    0xC0, 0xFF, // $FFFE: (Reset vector points to $FFC0)
];

/// SPC700 memory implementation
struct Spc700Memory {
    /// 64KB RAM
    ram: Box<[u8; 0x10000]>,
    /// Control register ($F1)
    /// Reference: https://wiki.superfamicom.org/spc700-reference
    /// Bit 7: IPL ROM enable (1 = enabled, maps $FFC0-$FFFF to IPL ROM)
    /// Bit 5: Clear OUTPUT ports $F6-$F7 (write-only, auto-clears, clears what main CPU reads)
    /// Bit 4: Clear OUTPUT ports $F4-$F5 (write-only, auto-clears, clears what main CPU reads)
    /// Bit 2: Timer 2 enable
    /// Bit 1: Timer 1 enable
    /// Bit 0: Timer 0 enable
    control: u8,
    /// Communication ports (shared with main CPU)
    /// These are written by main CPU, read by SPC700
    cpuio: [u8; 4],
    /// Ports written by SPC700, read by main CPU
    /// (Separate from cpuio for bidirectional communication)
    apu_out: [u8; 4],
    /// Timer divisors (written to $FA-$FC)
    /// Value 0 means 256
    timer_divisor: [u8; 3],
    /// Timer output counters (read from $FD-$FF, 4-bit, cleared on read)
    /// Uses Cell for interior mutability since reads clear the counter
    timer_counter: [Cell<u8>; 3],
    /// Timer internal prescalers (count cycles before timer tick)
    /// Timer 0 & 1: tick every 128 cycles (8 kHz at 1.024 MHz)
    /// Timer 2: tick every 16 cycles (64 kHz at 1.024 MHz)
    timer_prescaler: [u16; 3],
    /// Timer internal counters (count ticks up to divisor)
    timer_internal: [u8; 3],
    /// DSP (Digital Signal Processor) for audio generation
    dsp: Dsp,
    /// DSP address register
    dsp_addr: u8,
}

impl Spc700Memory {
    fn new() -> Self {
        let mut mem = Self {
            ram: Box::new([0; 0x10000]),
            control: 0x80, // IPL ROM enabled by default
            cpuio: [0; 4],
            apu_out: [0; 4], // IPL ROM will write $AA/$BB signature
            timer_divisor: [0; 3],
            timer_counter: [Cell::new(0), Cell::new(0), Cell::new(0)],
            timer_prescaler: [0; 3],
            timer_internal: [0; 3],
            dsp: Dsp::new(),
            dsp_addr: 0,
        };
        // Give DSP access to RAM for BRR sample fetching
        mem.dsp.set_ram(&*mem.ram as *const [u8; 0x10000]);
        mem
    }

    /// Update timers based on cycles elapsed
    /// Called from run_cycles for each CPU step
    fn tick_timers(&mut self, cycles: u32) {
        // Timer 0 & 1: prescaler period is 128 cycles (8 kHz)
        // Timer 2: prescaler period is 16 cycles (64 kHz)
        const PRESCALER_0_1: u16 = 128;
        const PRESCALER_2: u16 = 16;

        for timer in 0..3 {
            // Check if timer is enabled (bits 0-2 of control register)
            if self.control & (1 << timer) == 0 {
                continue;
            }

            let prescaler_period = if timer == 2 {
                PRESCALER_2
            } else {
                PRESCALER_0_1
            };

            // Add cycles to prescaler
            self.timer_prescaler[timer] += cycles as u16;

            // Process prescaler overflows (timer ticks)
            while self.timer_prescaler[timer] >= prescaler_period {
                self.timer_prescaler[timer] -= prescaler_period;

                // Increment internal 8-bit counter (modulo divisor)
                // When divisor register is 0, it acts as 256
                let divisor = if self.timer_divisor[timer] == 0 {
                    256u16
                } else {
                    self.timer_divisor[timer] as u16
                };

                self.timer_internal[timer] = self.timer_internal[timer].wrapping_add(1);

                // When internal counter reaches divisor, reset it and increment output counter
                // Reference: https://wiki.superfamicom.org/spc700-reference (Timers section)
                if self.timer_internal[timer] as u16 >= divisor {
                    self.timer_internal[timer] = 0;
                    // Increment 4-bit output counter (wraps from 15 to 0)
                    let old = self.timer_counter[timer].get();
                    let new_val = (old + 1) & 0x0F;
                    self.timer_counter[timer].set(new_val);
                    log(LogCategory::APU, LogLevel::Debug, || {
                        format!(
                            "SPC700: Timer {} counter incremented: {} -> {}",
                            timer, old, new_val
                        )
                    });
                }
            }
        }
    }

    /// Write to communication port from main CPU side
    fn write_cpuio(&mut self, port: usize, val: u8) {
        if port < 4 {
            self.cpuio[port] = val;
            log(LogCategory::APU, LogLevel::Debug, || {
                format!("SPC700 Memory: Main CPU wrote ${:02X} to port ${} (CPUIO now: ${:02X} ${:02X} ${:02X} ${:02X})",
                    val, port, self.cpuio[0], self.cpuio[1], self.cpuio[2], self.cpuio[3])
            });
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
            // Communication ports (read what main CPU wrote)
            CPUIO0..=CPUIO3 => {
                let port = (addr - CPUIO0) as usize;
                let val = self.cpuio[port];
                // Log port reads for debugging (reduced to Debug level to avoid spam)
                log(LogCategory::APU, LogLevel::Debug, || {
                    format!("SPC700: Read port $F{} (CPUIO) = ${:02X} (all ports: ${:02X} ${:02X} ${:02X} ${:02X})", 
                        4 + port, val, self.cpuio[0], self.cpuio[1], self.cpuio[2], self.cpuio[3])
                });
                val
            }
            // Timer counters - reading clears the counter
            COUNTER0 => {
                let val = self.timer_counter[0].get();
                self.timer_counter[0].set(0);
                val
            }
            COUNTER1 => {
                let val = self.timer_counter[1].get();
                self.timer_counter[1].set(0);
                val
            }
            COUNTER2 => {
                let val = self.timer_counter[2].get();
                self.timer_counter[2].set(0);
                val
            }
            // DSP data register
            DSP_DATA => self.dsp.read_register(self.dsp_addr & 0x7F),
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
                // Log port writes for debugging (reduced to Debug level to avoid spam)
                log(LogCategory::APU, LogLevel::Debug, || {
                    format!("SPC700: Write port $F{} = ${:02X} (apu_out now: ${:02X} ${:02X} ${:02X} ${:02X})", 
                        4 + port, val, self.apu_out[0], self.apu_out[1], self.apu_out[2], self.apu_out[3])
                });
            }
            // Control register
            CONTROL_REG => {
                let old_control = self.control;
                self.control = val;

                // Log IPL ROM enable/disable
                if (old_control & 0x80) != (val & 0x80) {
                    log(LogCategory::APU, LogLevel::Info, || {
                        format!(
                            "SPC700: IPL ROM {} (control=${:02X})",
                            if val & 0x80 != 0 {
                                "ENABLED"
                            } else {
                                "DISABLED"
                            },
                            val
                        )
                    });
                }

                // When a timer is enabled (bit transitions from 0 to 1),
                // reset its internal counter and output counter
                for timer in 0..3 {
                    let mask = 1 << timer;
                    if (old_control & mask) == 0 && (val & mask) != 0 {
                        self.timer_prescaler[timer] = 0;
                        self.timer_internal[timer] = 0;
                        self.timer_counter[timer].set(0);
                        log(LogCategory::APU, LogLevel::Debug, || {
                            format!("SPC700: Timer {} enabled, counters reset", timer)
                        });
                    }
                }

                if val & 0x10 != 0 {
                    // Clear ports $F4-$F5 (SPC700 output ports, main CPU input)
                    // Reference: https://wiki.superfamicom.org/spc700-reference
                    // Bits 4-5 clear the OUTPUT ports (what main CPU reads)
                    self.apu_out[0] = 0;
                    self.apu_out[1] = 0;
                }
                if val & 0x20 != 0 {
                    // Clear ports $F6-$F7 (SPC700 output ports, main CPU input)
                    self.apu_out[2] = 0;
                    self.apu_out[3] = 0;
                }
            }
            // DSP address register
            DSP_ADDR => {
                self.dsp_addr = val & 0x7F; // Only 7 bits used
            }
            // DSP data register
            DSP_DATA => {
                self.dsp.write_register(self.dsp_addr & 0x7F, val);
            }
            // Timer divisors
            TIMER0 => {
                self.timer_divisor[0] = val;
                log(LogCategory::APU, LogLevel::Info, || {
                    format!(
                        "SPC700: Timer 0 divisor set to ${:02X} ({})",
                        val,
                        if val == 0 { 256 } else { val as u16 }
                    )
                });
            }
            TIMER1 => {
                self.timer_divisor[1] = val;
                log(LogCategory::APU, LogLevel::Info, || {
                    format!(
                        "SPC700: Timer 1 divisor set to ${:02X} ({})",
                        val,
                        if val == 0 { 256 } else { val as u16 }
                    )
                });
            }
            TIMER2 => {
                self.timer_divisor[2] = val;
                log(LogCategory::APU, LogLevel::Info, || {
                    format!(
                        "SPC700: Timer 2 divisor set to ${:02X} ({})",
                        val,
                        if val == 0 { 256 } else { val as u16 }
                    )
                });
            }
            // Test register and counters are read-only
            TEST_REG | COUNTER0 | COUNTER1 | COUNTER2 | AUX_IO4 | AUX_IO5 => {}
            // RAM
            _ => {
                // Log writes to zero page during upload (especially $00-$01 for indirect addressing)
                if self.control & 0x80 != 0 && addr <= 0x01 {
                    log(LogCategory::APU, LogLevel::Info, || {
                        format!(
                            "SPC700: ZP write RAM[${:04X}] = ${:02X} (base address for upload)",
                            addr, val
                        )
                    });
                }
                // Log all other writes to low RAM during upload
                else if self.control & 0x80 != 0 && addr < 0x0100 {
                    log(LogCategory::APU, LogLevel::Debug, || {
                        format!("SPC700: Upload write to RAM[${:04X}] = ${:02X}", addr, val)
                    });
                }
                // Log writes to uploaded code area (likely $0200-$XXXX)
                else if self.control & 0x80 != 0 && (0x0200..0x1000).contains(&addr) {
                    log(LogCategory::APU, LogLevel::Info, || {
                        format!(
                            "SPC700: Upload data to RAM[${:04X}] = ${:02X} (uploaded code area)",
                            addr, val
                        )
                    });
                }
                self.ram[addr as usize] = val;
            }
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
    /// DSP cycle accumulator (DSP runs at 32kHz, CPU at 1.024MHz)
    /// DSP clocks every 32 CPU cycles (1024000/32000 = 32)
    dsp_cycle_acc: u32,
    /// Total cycles requested from run_cycles (for debugging)
    total_cycles_requested: u64,
    /// Number of run_cycles calls (for debugging)
    run_cycles_call_count: u64,
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
            dsp_cycle_acc: 0,
            total_cycles_requested: 0,
            run_cycles_call_count: 0,
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
            let val = self.cpu.memory.read_apu_out(port as usize);
            log(LogCategory::APU, LogLevel::Debug, || {
                format!("SPC700: Main CPU reads port {} = ${:02X}", port, val)
            });
            val
        } else {
            0
        }
    }

    /// Execute CPU for a number of cycles
    pub fn run_cycles(&mut self, cycles: u32) {
        // Track statistics
        self.run_cycles_call_count += 1;
        self.total_cycles_requested += cycles as u64;

        // Log summary every 10000 calls to avoid spam
        if self.run_cycles_call_count.is_multiple_of(10000) {
            log(LogCategory::APU, LogLevel::Info, || {
                format!(
                    "SPC700: run_cycles called {} times, total_requested={}, SPC700_cycles={}, PC=${:04X}",
                    self.run_cycles_call_count, self.total_cycles_requested, self.cpu.cycles, self.cpu.pc
                )
            });
        }

        // Track if we're in critical IPL ROM areas
        let was_in_upload_loop = self.cpu.pc >= 0xFFD6 && self.cpu.pc <= 0xFFEE;
        let was_in_entry_setup = self.cpu.pc >= 0xFFEF;

        // Log calls to verify this is being called (debug level)
        log(LogCategory::APU, LogLevel::Debug, || {
            format!(
                "SPC700: run_cycles({}) called, PC=${:04X}, total_cycles={}",
                cycles, self.cpu.pc, self.cpu.cycles
            )
        });

        let mut remaining = cycles;
        while remaining > 0 {
            let old_pc = self.cpu.pc;
            let executed = self.cpu.step() as u32;

            // Log when entering critical IPL ROM sections
            if self.cpu.memory.control & 0x80 != 0 {
                if !was_in_upload_loop && self.cpu.pc >= 0xFFD6 && self.cpu.pc <= 0xFFEE {
                    log(LogCategory::APU, LogLevel::Info, || {
                        format!(
                            "SPC700: Entered upload loop at PC=${:04X} (from PC=${:04X})",
                            self.cpu.pc, old_pc
                        )
                    });
                }
                if !was_in_entry_setup && self.cpu.pc >= 0xFFEF {
                    log(LogCategory::APU, LogLevel::Info, || {
                        format!("SPC700: Entered entry point setup at PC=${:04X} (from PC=${:04X}, ZP=$00={:02X}, ZP=$01={:02X})",
                            self.cpu.pc, old_pc,
                            self.cpu.memory.ram[0x00], self.cpu.memory.ram[0x01])
                    });
                }
                // Log when IPL ROM is disabled or jumped out of
                if old_pc >= 0xFFC0 && self.cpu.pc < 0xFFC0 {
                    log(LogCategory::APU, LogLevel::Info, || {
                        format!("SPC700: Jumped out of IPL ROM from PC=${:04X} to PC=${:04X} (uploaded code start)", 
                            old_pc, self.cpu.pc)
                    });
                    // Dump first 64 bytes of uploaded code area for diagnosis
                    let mut dump = String::from("SPC700: First 64 bytes at jump target:\n");
                    for i in 0..4 {
                        dump.push_str(&format!("${:04X}: ", self.cpu.pc + i * 16));
                        for j in 0..16 {
                            let addr = (self.cpu.pc + i * 16 + j) as usize;
                            if addr < self.cpu.memory.ram.len() {
                                dump.push_str(&format!("{:02X} ", self.cpu.memory.ram[addr]));
                            }
                        }
                        dump.push('\n');
                    }
                    log(LogCategory::APU, LogLevel::Info, || dump);
                }
            }

            // Update timers based on executed cycles
            self.cpu.memory.tick_timers(executed);

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

        // DSP runs at 32kHz (every 32 CPU cycles at 1.024 MHz)
        self.dsp_cycle_acc += 1;
        if self.dsp_cycle_acc >= 32 {
            self.dsp_cycle_acc -= 32;
            // Clock the DSP and return stereo sample (mix to mono for now)
            let (left, right) = self.cpu.memory.dsp.clock();
            // Simple mono mix
            return ((left as i32 + right as i32) / 2) as i16;
        }

        // Return last sample if DSP didn't clock (simple hold)
        // TODO: Implement proper sample interpolation
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
            let mut dsp_sample = (0i16, 0i16);
            while self.cycle_acc >= 1.0 {
                self.cpu.step();
                self.cycle_acc -= 1.0;

                // Clock DSP at 32kHz (every 32 CPU cycles)
                self.dsp_cycle_acc += 1;
                if self.dsp_cycle_acc >= 32 {
                    self.dsp_cycle_acc -= 32;
                    dsp_sample = self.cpu.memory.dsp.clock();
                }
            }

            // Mix stereo to mono for now
            let mono = ((dsp_sample.0 as i32 + dsp_sample.1 as i32) / 2) as i16;
            samples.push(mono);
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

        // Run enough cycles for IPL ROM to complete clear loop and write $BBAA
        // Clear loop: ~2048 cycles, writing $BBAA: ~10 cycles
        apu.run_cycles(3000);

        // Read back (should get $BBAA from IPL ROM)
        let port0 = apu.read_port(0);
        let port1 = apu.read_port(1);

        // With the IPL ROM, it should set ports to $AA, $BB
        assert_eq!(port0, 0xAA, "Port 0 should be $AA from IPL ROM");
        assert_eq!(port1, 0xBB, "Port 1 should be $BB from IPL ROM");
    }

    #[test]
    fn test_audio_chip_interface() {
        let mut apu = Spc700::new();

        // Run enough cycles for IPL ROM to write $BBAA signature
        apu.run_cycles(3000);

        // Test AudioChip trait methods
        apu.write_register(0x2140, 0x12);

        // Run a few cycles to let SPC700 process
        apu.run_cycles(10);

        let val = apu.read_register(0x2140);

        // With correct IPL ROM, after writing signature, SPC700 is waiting for $CC
        // So it won't echo our $12, but we should still see $AA from the signature
        assert!(
            val == 0x12 || val == 0xAA,
            "Got ${:02X}, expected $12 or $AA",
            val
        );
    }

    #[test]
    fn test_spc700_ipl_upload_protocol() {
        let mut apu = Spc700::new();

        // Run until IPL ROM writes $BBAA (about 3000 cycles)
        apu.run_cycles(3000);

        // Verify $BBAA signature
        assert_eq!(apu.read_port(0), 0xAA);
        assert_eq!(apu.read_port(1), 0xBB);

        // Verify SPC700 is waiting for $CC at $FFCF-$FFD2
        assert!(apu.cpu.pc == 0xFFCF || apu.cpu.pc == 0xFFD2);

        // Send $CC to signal ready
        apu.write_port(0, 0xCC);
        apu.run_cycles(100);

        // SPC700 should now be at $FFEF (reading entry point)
        // Let's send entry point address $0200
        apu.write_port(2, 0x00); // Low byte of address
        apu.write_port(3, 0x02); // High byte of address

        apu.run_cycles(100);

        // After RET, SPC700 should jump to $0200
        // But since stack is cleared, it will actually jump to $0000
        // Let's write a simple test program to $0000: write $CC to port $F4
        // We can't upload it through IPL ROM (that path is not implemented)
        // So let's manually put it in RAM for this test
        apu.cpu.memory.ram[0x0000] = 0x8F; // MOV $F4, #imm
        apu.cpu.memory.ram[0x0001] = 0xCC; // Immediate value $CC
        apu.cpu.memory.ram[0x0002] = 0xF4; // Port address $F4
        apu.cpu.memory.ram[0x0003] = 0x2F; // BRA (infinite loop)
        apu.cpu.memory.ram[0x0004] = 0xFD; // Branch to self

        // Run until SPC700 executes the uploaded code
        apu.run_cycles(1000);

        // Verify SPC700 wrote $CC to port $F4
        assert_eq!(
            apu.read_port(0),
            0xCC,
            "SPC700 should have written $CC to port $F4"
        );
    }

    /// Test the complete IPL ROM boot sequence as described in the wiki:
    /// https://snes.nesdev.org/wiki/Booting_the_SPC700
    #[test]
    fn test_ipl_boot_sequence_complete() {
        let mut apu = Spc700::new();

        // Verify initial state
        assert_eq!(
            apu.cpu.pc, 0xFFC0,
            "SPC700 should start at IPL ROM entry point"
        );

        // Step 1: Reset - Stack pointer = $EF, Zero-page from $00-$EF is set to $00
        // Run the clear loop (takes about 2000+ cycles)
        apu.run_cycles(2500);

        // Verify stack pointer was set
        assert_eq!(apu.cpu.sp, 0xEF, "Stack pointer should be set to $EF");

        // Verify zero page was cleared (check a few spots)
        for addr in [0x00, 0x10, 0x50, 0xEF] {
            assert_eq!(
                apu.cpu.memory.ram[addr], 0x00,
                "Zero page ${:02X} should be cleared",
                addr
            );
        }

        // Step 2: Signal ready - Port 0 = $AA, Port 1 = $BB
        assert_eq!(
            apu.read_port(0),
            0xAA,
            "Port 0 should be $AA (ready signal)"
        );
        assert_eq!(
            apu.read_port(1),
            0xBB,
            "Port 1 should be $BB (ready signal)"
        );

        // Step 3: Wait for signal - Loop until $CC is read from port 0
        // SPC700 should be waiting at $FFCF or $FFD2 (the wait loop)
        assert!(
            apu.cpu.pc == 0xFFCF || apu.cpu.pc == 0xFFD2,
            "SPC700 should be in wait loop at $FFCF-$FFD2, got ${:04X}",
            apu.cpu.pc
        );

        // Verify it stays in the loop when we haven't sent $CC
        apu.run_cycles(100);
        assert!(
            apu.cpu.pc == 0xFFCF || apu.cpu.pc == 0xFFD2,
            "SPC700 should still be waiting for $CC"
        );

        // IMPORTANT: Before sending $CC, we must set up ports $F6/$F7 with an entry point
        // Otherwise SPC700 will read garbage and jump to a random address
        // Set entry point to $0200 (typical for SPC700 programs)
        apu.write_port(2, 0x00); // Low byte of entry point
        apu.write_port(3, 0x02); // High byte of entry point

        // Also set port 1 to non-zero to indicate we'll upload data
        apu.write_port(1, 0x01);

        // Now send the $CC signal
        apu.write_port(0, 0xCC);
        apu.run_cycles(100);

        // SPC700 should have progressed to $FFEF area (entry point setup)
        // and then looped back to upload loop at $FFD6 (because port 1 was non-zero)
        assert!(
            apu.cpu.pc >= 0xFFD6,
            "SPC700 should have progressed to upload loop after receiving $CC, PC=${:04X}",
            apu.cpu.pc
        );
    }

    /// Test the full upload protocol simulation as described in the wiki
    #[test]
    fn test_complete_upload_protocol() {
        let mut apu = Spc700::new();

        // Wait for IPL ROM to signal ready ($BBAA)
        apu.run_cycles(3000);
        assert_eq!(apu.read_port(0), 0xAA, "Should see $AA ready signal");
        assert_eq!(apu.read_port(1), 0xBB, "Should see $BB ready signal");

        // Step 1: Set starting address to $0200 (typical for SPC700 programs)
        apu.write_port(2, 0x00); // Low byte
        apu.write_port(3, 0x02); // High byte
        apu.write_port(1, 0x01); // Non-zero value (indicates more data coming)
        apu.write_port(0, 0xCC); // Send $CC to start

        // Step 2: Wait for acknowledgment
        apu.run_cycles(100);
        assert_eq!(
            apu.read_port(0),
            0xCC,
            "SPC700 should echo back $CC as acknowledgment"
        );

        // Step 3: Send data bytes (let's send a simple 4-byte program)
        // Program: MOV $F4, #$42; BRA $-2 (write $42 to port, then loop)
        let test_program = [
            0x8F, 0x42, 0xF4, // MOV $F4, #$42
            0x2F, 0xFD, // BRA $-3 (loop forever)
        ];

        for (i, &byte) in test_program.iter().enumerate() {
            // Write data byte to port 1
            apu.write_port(1, byte);

            // Write index to port 0 (low byte of counter)
            let index = i as u8;
            apu.write_port(0, index);

            // Run cycles to let SPC700 process
            apu.run_cycles(50);

            // Verify acknowledgment (SPC700 echoes index back)
            assert_eq!(
                apu.read_port(0),
                index,
                "SPC700 should echo index ${:02X} for byte {}",
                index,
                i
            );
        }

        // Step 4: Tell SPC700 to execute the uploaded code
        // Write entry point to ports 2-3 again
        apu.write_port(2, 0x00);
        apu.write_port(3, 0x02);
        // Write 0 to port 1 (signals execution)
        apu.write_port(1, 0x00);
        // Increment counter by 2 (from last index + 2)
        let final_index = (test_program.len() as u8).wrapping_add(2);
        apu.write_port(0, final_index);

        // Run more cycles to ensure SPC700 processes the execution command
        // The SPC700 needs to:
        // 1. Read the changed port 0 value (was 4, now 7)
        // 2. Notice it jumped by more than 1
        // 3. Echo it back
        // 4. Jump to uploaded code
        apu.run_cycles(100);

        // Read port 0 - should be either the acknowledgment OR the uploaded code output
        // The IPL ROM echoes the index, but then immediately jumps to uploaded code
        // which writes $42 to port 0. Timing determines which we see.
        let port0_value = apu.read_port(0);
        assert!(
            port0_value == final_index || port0_value == 0x42 || port0_value == 4,
            "Port 0 should be last index $04, ack ${:02X}, or program output $42, got ${:02X}",
            final_index,
            port0_value
        );

        // SPC700 should now be executing the uploaded code
        // NOTE: The IPL ROM may still be enabled - uploaded code must disable it explicitly
        // by writing to control register $F1. The IPL ROM doesn't auto-disable.

        // Our simple test program doesn't disable IPL ROM, so we can't assert it's disabled
        // In real audio drivers, they typically disable IPL ROM early in initialization

        // Run the uploaded program (it writes $42 to port 0)
        apu.run_cycles(100);

        // Verify the program executed - should definitely be $42 now
        assert_eq!(
            apu.read_port(0),
            0x42,
            "Uploaded program should have written $42 to port 0"
        );
    }

    /// Test that simulates the exact sequence from Super Mario World
    /// This is the real-world scenario that was failing
    #[test]
    fn test_super_mario_world_initialization_sequence() {
        let mut apu = Spc700::new();

        // === Phase 1: Wait for IPL ROM ready signal ===
        apu.run_cycles(3000);

        let port0 = apu.read_port(0);
        let port1 = apu.read_port(1);

        // Main CPU performs 16-bit read of $2140-$2141, expecting $BBAA
        assert_eq!(port0, 0xAA, "Port 0 should be $AA");
        assert_eq!(port1, 0xBB, "Port 1 should be $BB");

        // Combine into 16-bit value (little-endian: low byte first)
        let signature = (port1 as u16) << 8 | (port0 as u16);
        assert_eq!(signature, 0xBBAA, "16-bit signature should be $BBAA");

        // === Phase 2: Clear ports (main CPU writes $00 to all ports) ===
        apu.write_port(0, 0x00);
        apu.write_port(1, 0x00);
        apu.write_port(2, 0x00);
        apu.write_port(3, 0x00);
        apu.run_cycles(10);

        // === Phase 3: Send start command ===
        // Set upload address to $0200
        apu.write_port(2, 0x00); // Low byte
        apu.write_port(3, 0x02); // High byte
        apu.write_port(1, 0x01); // Non-zero (more data coming)
        apu.write_port(0, 0xCC); // Start signal

        apu.run_cycles(100);

        // Verify SPC700 acknowledged
        assert_eq!(apu.read_port(0), 0xCC, "SPC700 should acknowledge with $CC");

        // === Phase 4: Upload audio driver (simplified - just a few bytes) ===

        // Real audio driver is typically 1-2KB, but we'll upload a minimal stub
        // that writes $CC back to port 0 to signal completion
        let audio_driver = [
            0x8F, 0xCC, 0xF4, // MOV $F4, #$CC (write $CC to port 0)
            0x2F, 0xFD, // BRA $-3 (loop forever)
        ];

        for (i, &byte) in audio_driver.iter().enumerate() {
            apu.write_port(1, byte);
            apu.write_port(0, i as u8);
            apu.run_cycles(50);

            let echoed = apu.read_port(0);
            assert_eq!(
                echoed, i as u8,
                "Index {} should be echoed, got ${:02X}",
                i, echoed
            );
        }

        // === Phase 5: Execute uploaded code ===
        apu.write_port(2, 0x00); // Entry point low
        apu.write_port(3, 0x02); // Entry point high
        apu.write_port(1, 0x00); // Zero = execute
        let final_index = (audio_driver.len() as u8).wrapping_add(2);
        apu.write_port(0, final_index);

        // Run fewer cycles - uploaded code runs quickly and overwrites acknowledgment
        apu.run_cycles(50);

        // === Phase 6: Wait for audio driver to signal ready ===

        // The audio driver should now be running and will write $CC to port 0
        apu.run_cycles(100);

        let port0_final = apu.read_port(0);

        assert_eq!(
            port0_final, 0xCC,
            "Audio driver should signal ready with $CC"
        );

        // === Phase 7: Verify main CPU can read $BBAA again (NOT expected in real world) ===
        // In the real scenario, the main CPU at PC=$8085 is waiting for ports
        // to return to $BBAA. However, this doesn't match the wiki documentation.
        // The wiki says after upload, the SPC700 executes the uploaded code,
        // which would NOT return to $BBAA.
        //
        // The main CPU should be waiting for the uploaded audio driver to
        // acknowledge a command, not waiting for $BBAA again.
    }

    /// Test atomic 16-bit port reads (simulating the issue at PC=$8085)
    #[test]
    fn test_atomic_16bit_port_read() {
        let mut apu = Spc700::new();

        // Let IPL ROM write $BBAA
        apu.run_cycles(3000);

        // Simulate main CPU doing a 16-bit read of ports $2140-$2141
        // This should read both ports atomically, even if SPC700 is running

        // First read (establishes latch)
        let port0 = apu.read_port(0);

        // Run SPC700 for some cycles (it might update ports)
        apu.run_cycles(10);

        // Second read (should still get same value if latched properly)
        let port1 = apu.read_port(1);

        // Combine into 16-bit value
        let value = (port1 as u16) << 8 | (port0 as u16);

        // Should be $BBAA regardless of SPC700 activity between reads
        assert_eq!(
            value, 0xBBAA,
            "16-bit read should be atomic, got ${:04X}",
            value
        );
    }

    /// Test the control register IPL ROM enable/disable functionality
    #[test]
    fn test_ipl_rom_control() {
        let mut apu = Spc700::new();

        // IPL ROM should be enabled initially
        assert_ne!(
            apu.cpu.memory.control & 0x80,
            0,
            "IPL ROM should be enabled"
        );

        // Read from IPL ROM region
        let ipl_byte = apu.cpu.memory.read(0xFFC0);
        assert_eq!(ipl_byte, 0xCD, "Should read IPL ROM opcode at $FFC0");

        // Disable IPL ROM by clearing bit 7 of control register
        apu.cpu.memory.write(CONTROL_REG, 0x00);
        assert_eq!(
            apu.cpu.memory.control & 0x80,
            0,
            "IPL ROM should be disabled"
        );

        // Now reading same address should get RAM (which is 0)
        let ram_byte = apu.cpu.memory.read(0xFFC0);
        assert_eq!(
            ram_byte, 0x00,
            "Should read RAM (0) at $FFC0 when IPL disabled"
        );

        // Re-enable IPL ROM
        apu.cpu.memory.write(CONTROL_REG, 0x80);
        assert_ne!(
            apu.cpu.memory.control & 0x80,
            0,
            "IPL ROM should be re-enabled"
        );

        // Should read IPL ROM again
        let ipl_byte2 = apu.cpu.memory.read(0xFFC0);
        assert_eq!(ipl_byte2, 0xCD, "Should read IPL ROM again at $FFC0");
    }

    /// Test port clearing functionality via control register
    /// Reference: https://wiki.superfamicom.org/spc700-reference
    /// Bits 4-5 of control register ($F1) clear OUTPUT ports (what main CPU reads)
    #[test]
    fn test_port_clear_via_control() {
        let mut apu = Spc700::new();

        // SPC700 writes some values to output ports (for main CPU to read)
        apu.cpu.memory.write(CPUIO0, 0x12);
        apu.cpu.memory.write(CPUIO1, 0x34);
        apu.cpu.memory.write(CPUIO2, 0x56);
        apu.cpu.memory.write(CPUIO3, 0x78);

        // Verify they were written to output ports
        assert_eq!(apu.cpu.memory.apu_out[0], 0x12);
        assert_eq!(apu.cpu.memory.apu_out[1], 0x34);
        assert_eq!(apu.cpu.memory.apu_out[2], 0x56);
        assert_eq!(apu.cpu.memory.apu_out[3], 0x78);

        // Main CPU should be able to read these values
        assert_eq!(apu.read_port(0), 0x12);
        assert_eq!(apu.read_port(1), 0x34);
        assert_eq!(apu.read_port(2), 0x56);
        assert_eq!(apu.read_port(3), 0x78);

        // Clear OUTPUT ports 0-1 via control register (bit 4)
        // This is what SPC700 would do to reset its output state
        apu.cpu.memory.write(CONTROL_REG, 0x10);
        assert_eq!(
            apu.cpu.memory.apu_out[0], 0x00,
            "Output port 0 should be cleared"
        );
        assert_eq!(
            apu.cpu.memory.apu_out[1], 0x00,
            "Output port 1 should be cleared"
        );
        assert_eq!(
            apu.cpu.memory.apu_out[2], 0x56,
            "Output port 2 should not be cleared"
        );
        assert_eq!(
            apu.cpu.memory.apu_out[3], 0x78,
            "Output port 3 should not be cleared"
        );

        // Main CPU reads should now see cleared ports
        assert_eq!(apu.read_port(0), 0x00, "Main CPU should read 0 from port 0");
        assert_eq!(apu.read_port(1), 0x00, "Main CPU should read 0 from port 1");
        assert_eq!(
            apu.read_port(2),
            0x56,
            "Main CPU should still read $56 from port 2"
        );
        assert_eq!(
            apu.read_port(3),
            0x78,
            "Main CPU should still read $78 from port 3"
        );

        // Clear OUTPUT ports 2-3 via control register (bit 5)
        apu.cpu.memory.write(CONTROL_REG, 0x20);
        assert_eq!(
            apu.cpu.memory.apu_out[2], 0x00,
            "Output port 2 should be cleared"
        );
        assert_eq!(
            apu.cpu.memory.apu_out[3], 0x00,
            "Output port 3 should be cleared"
        );

        // Main CPU reads should see all ports cleared
        assert_eq!(apu.read_port(2), 0x00, "Main CPU should read 0 from port 2");
        assert_eq!(apu.read_port(3), 0x00, "Main CPU should read 0 from port 3");
    }

    /// Test that reproduces the multi-session upload issue from Super Mario World.
    ///
    /// **The Issue:**
    /// After the first upload session completes and the uploaded code executes,
    /// the main CPU expects the SPC700 to write $AA/$BB to signal readiness for
    /// a second upload session. However, the uploaded audio driver code often
    /// waits for a timer before responding, causing the main CPU to be stuck
    /// in a loop waiting for $AA at $2140.
    ///
    /// **Expected Behavior:**
    /// When the uploaded code uses timers, the timers must tick properly so the
    /// code can progress and eventually respond to the main CPU.
    ///
    /// This test simulates:
    /// 1. First upload session (IPL ROM -> uploaded driver)
    /// 2. Uploaded driver uses Timer 0 to wait before responding
    /// 3. Main CPU waits for response (would be stuck if timers don't work)
    #[test]
    fn test_multi_session_upload_with_timer_wait() {
        let mut apu = Spc700::new();

        // === Phase 1: Wait for IPL ROM ready signal ===
        apu.run_cycles(3000);
        assert_eq!(apu.read_port(0), 0xAA, "IPL should signal $AA");
        assert_eq!(apu.read_port(1), 0xBB, "IPL should signal $BB");

        // === Phase 2: Start first upload session ===
        apu.write_port(2, 0x00); // Low byte of address
        apu.write_port(3, 0x02); // High byte: $0200
        apu.write_port(1, 0x01); // Non-zero = more data coming
        apu.write_port(0, 0xCC); // Start signal
        apu.run_cycles(100);
        assert_eq!(apu.read_port(0), 0xCC, "SPC700 should acknowledge with $CC");

        // === Phase 3: Upload a driver that uses Timer 0 to delay response ===
        // This simulates real audio drivers that wait for a timer before responding.
        //
        // Uploaded code at $0200:
        //   MOV $FA, #$FF     ; Set Timer 0 divisor to 255 (slowest)
        //   MOV $F1, #$01     ; Enable Timer 0
        //   wait_loop:
        //     MOV A, $FD      ; Read Timer 0 counter
        //     BEQ wait_loop   ; Loop while counter is 0
        //   MOV $F4, #$AA     ; Write $AA to port 0 (ready for second session)
        //   MOV $F5, #$BB     ; Write $BB to port 1
        //   loop_forever:
        //     BRA loop_forever
        let timer_driver = [
            // MOV $FA, #$FF (Set Timer 0 divisor)
            0x8F, 0xFF, 0xFA, // MOV $F1, #$81 (Enable Timer 0 + keep IPL ROM enabled)
            0x8F, 0x81, 0xF1, // wait_loop: MOV A, $FD (Read Timer 0 counter)
            0xE4, 0xFD, // BEQ wait_loop (Loop while A == 0) - branch offset -4
            0xF0, 0xFC, // MOV $F4, #$AA (Write $AA to port 0)
            0x8F, 0xAA, 0xF4, // MOV $F5, #$BB (Write $BB to port 1)
            0x8F, 0xBB, 0xF5, // BRA loop_forever (infinite loop) - branch offset -2
            0x2F, 0xFE,
        ];

        for (i, &byte) in timer_driver.iter().enumerate() {
            apu.write_port(1, byte);
            apu.write_port(0, i as u8);
            apu.run_cycles(50);
            assert_eq!(apu.read_port(0), i as u8, "Index {} should be echoed", i);
        }

        // === Phase 4: Execute uploaded code ===
        apu.write_port(2, 0x00); // Entry point low
        apu.write_port(3, 0x02); // Entry point high
        apu.write_port(1, 0x00); // Zero = execute
        let final_index = (timer_driver.len() as u8).wrapping_add(2);
        apu.write_port(0, final_index);
        apu.run_cycles(50);

        // Verify SPC700 is now executing at uploaded code location
        // (It may be in the timer wait loop)

        // === Phase 5: This is where the bug manifests ===
        // The uploaded driver is now waiting for Timer 0 to tick.
        // If timers don't work, the SPC700 will be stuck forever.
        //
        // Main CPU would be polling port 0, expecting $AA.
        // Let's run enough cycles for the timer to tick and the driver to respond.

        // Timer 0 ticks at 8 kHz (every 128 SPC700 cycles)
        // With divisor $FF (255), output counter increments every 255 * 128 = 32,640 cycles
        // We need at least one timer tick for the BEQ to fail

        // But the counter just needs to go from 0 to 1, which happens after divisor+1 prescaler ticks
        // Divisor = $FF = 255, so internal counter overflows after 255 * 128 = 32640 cycles

        // Run for ~35,000 cycles to ensure timer ticks
        apu.run_cycles(35000);

        let port0_after = apu.read_port(0);
        let port1_after = apu.read_port(1);

        // The driver should have written $AA/$BB by now
        assert_eq!(
            port0_after, 0xAA,
            "After timer ticks, driver should write $AA to port 0. \
             If this fails, timers are not ticking properly!"
        );
        assert_eq!(
            port1_after, 0xBB,
            "After timer ticks, driver should write $BB to port 1"
        );
    }

    /// Test that Timer 0 actually ticks and increments the counter
    #[test]
    fn test_timer0_ticks() {
        let mut apu = Spc700::new();

        // Disable IPL ROM and set up for direct testing
        apu.cpu.memory.write(CONTROL_REG, 0x00); // Disable IPL ROM

        // Set Timer 0 divisor to 1 (fastest: output increments every 128 cycles)
        apu.cpu.memory.write(TIMER0, 0x01);

        // Enable Timer 0 (bit 0 of control register)
        apu.cpu.memory.write(CONTROL_REG, 0x01);

        // Read initial counter value (should be 0)
        let initial = apu.cpu.memory.timer_counter[0].get();
        assert_eq!(initial, 0, "Timer 0 counter should start at 0");

        // Run for 128 cycles (one prescaler period) - counter should increment once
        apu.run_cycles(128);

        // Read counter (this clears it!)
        let after_128 = apu.cpu.memory.timer_counter[0].get();

        // With divisor=1, after 128 cycles, internal counter overflows and output increments
        assert!(
            after_128 >= 1,
            "Timer 0 counter should be >= 1 after 128 cycles with divisor=1, got {}",
            after_128
        );
    }

    /// Test timer counter clear-on-read behavior
    #[test]
    fn test_timer_counter_clear_on_read() {
        let mut apu = Spc700::new();

        // Disable IPL ROM
        apu.cpu.memory.write(CONTROL_REG, 0x00);

        // Set Timer 0 divisor to 1 and enable
        apu.cpu.memory.write(TIMER0, 0x01);
        apu.cpu.memory.write(CONTROL_REG, 0x01);

        // Run for 256 cycles (should get 2 counter increments)
        apu.run_cycles(256);

        // First read should return count
        let first_read = apu.cpu.memory.read(COUNTER0);
        assert!(first_read >= 1, "Timer should have ticked");

        // Second read should return 0 (counter was cleared)
        let second_read = apu.cpu.memory.read(COUNTER0);
        assert_eq!(second_read, 0, "Counter should be cleared after read");
    }
}
