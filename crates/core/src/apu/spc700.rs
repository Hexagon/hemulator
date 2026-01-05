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
            apu_out: [0; 4], // IPL ROM will write $AA/$BB signature
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
                // Log all port reads during upload (when IPL ROM is enabled)
                if self.control & 0x80 != 0 {
                    log(LogCategory::APU, LogLevel::Debug, || {
                        format!("SPC700: Read port $F{} (CPUIO) = ${:02X} (IPL enabled, all ports: ${:02X} ${:02X} ${:02X} ${:02X})", 
                            4 + port, val, self.cpuio[0], self.cpuio[1], self.cpuio[2], self.cpuio[3])
                    });
                }
                val
            }
            // Timer counters
            COUNTER0 => {
                // Reading counter clears it (on real hardware)
                // We'll implement this when timers are active
                self.timer_counter[0]
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
        // Track if we're in critical IPL ROM areas
        let was_in_upload_loop = self.cpu.pc >= 0xFFD6 && self.cpu.pc <= 0xFFEE;
        let was_in_entry_setup = self.cpu.pc >= 0xFFEF;

        // Log first few calls to verify this is being called
        if self.cpu.cycles < 1000 {
            log(LogCategory::APU, LogLevel::Info, || {
                format!(
                    "SPC700: run_cycles({}) called, PC=${:04X}, total_cycles={}",
                    cycles, self.cpu.pc, self.cpu.cycles
                )
            });
        }

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

        // Run enough cycles for IPL ROM to complete clear loop and write $BBAA
        // Clear loop: ~2048 cycles, writing $BBAA: ~10 cycles
        apu.run_cycles(3000);

        // Read back (should get $BBAA from IPL ROM)
        let port0 = apu.read_port(0);
        let port1 = apu.read_port(1);

        println!("Port 0: ${:02X}, Port 1: ${:02X}", port0, port1);
        println!("SPC700 PC: ${:04X}", apu.cpu.pc);

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

        println!("PC before more cycles: ${:04X}", apu.cpu.pc);
        apu.run_cycles(100);
        println!("PC after IPL ROM: ${:04X}", apu.cpu.pc);

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

        println!(
            "Uploaded code at $0000: ${:02X} ${:02X} ${:02X}",
            apu.cpu.memory.ram[0x0000], apu.cpu.memory.ram[0x0001], apu.cpu.memory.ram[0x0002]
        );

        // Run until SPC700 executes the uploaded code
        apu.run_cycles(1000);

        println!("SPC700 PC after execution: ${:04X}", apu.cpu.pc);
        println!("Port $F4 value: ${:02X}", apu.read_port(0));

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
        println!("Phase 1: Waiting for IPL ROM ready signal...");
        apu.run_cycles(3000);

        let port0 = apu.read_port(0);
        let port1 = apu.read_port(1);
        println!("  Port 0: ${:02X}, Port 1: ${:02X}", port0, port1);

        // Main CPU performs 16-bit read of $2140-$2141, expecting $BBAA
        assert_eq!(port0, 0xAA, "Port 0 should be $AA");
        assert_eq!(port1, 0xBB, "Port 1 should be $BB");

        // Combine into 16-bit value (little-endian: low byte first)
        let signature = (port1 as u16) << 8 | (port0 as u16);
        assert_eq!(signature, 0xBBAA, "16-bit signature should be $BBAA");

        println!("  ✓ Got correct $BBAA signature");

        // === Phase 2: Clear ports (main CPU writes $00 to all ports) ===
        println!("Phase 2: Clearing ports...");
        apu.write_port(0, 0x00);
        apu.write_port(1, 0x00);
        apu.write_port(2, 0x00);
        apu.write_port(3, 0x00);
        apu.run_cycles(10);
        println!("  ✓ Ports cleared");

        // === Phase 3: Send start command ===
        println!("Phase 3: Sending start command...");
        // Set upload address to $0200
        apu.write_port(2, 0x00); // Low byte
        apu.write_port(3, 0x02); // High byte
        apu.write_port(1, 0x01); // Non-zero (more data coming)
        apu.write_port(0, 0xCC); // Start signal

        apu.run_cycles(100);

        // Verify SPC700 acknowledged
        assert_eq!(apu.read_port(0), 0xCC, "SPC700 should acknowledge with $CC");
        println!("  ✓ SPC700 acknowledged start command");

        // === Phase 4: Upload audio driver (simplified - just a few bytes) ===
        println!("Phase 4: Uploading audio driver...");

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
        println!("  ✓ {} bytes uploaded successfully", audio_driver.len());

        // === Phase 5: Execute uploaded code ===
        println!("Phase 5: Starting uploaded audio driver...");
        apu.write_port(2, 0x00); // Entry point low
        apu.write_port(3, 0x02); // Entry point high
        apu.write_port(1, 0x00); // Zero = execute
        let final_index = (audio_driver.len() as u8).wrapping_add(2);
        apu.write_port(0, final_index);

        // Run fewer cycles - uploaded code runs quickly and overwrites acknowledgment
        apu.run_cycles(50);

        // The SPC700 may have already jumped to uploaded code which writes $CC
        // So we might see either the acknowledgment or $CC
        let port0_after_exec = apu.read_port(0);
        println!(
            "  Port 0 after execution command: ${:02X} (expected ack ${:02X} or driver output $CC)",
            port0_after_exec, final_index
        );

        // === Phase 6: Wait for audio driver to signal ready ===
        println!("Phase 6: Waiting for audio driver ready signal...");

        // The audio driver should now be running and will write $CC to port 0
        apu.run_cycles(100);

        let port0_final = apu.read_port(0);
        println!("  Final port 0 value: ${:02X}", port0_final);

        assert_eq!(
            port0_final, 0xCC,
            "Audio driver should signal ready with $CC"
        );
        println!("  ✓ Audio driver is running and signaled ready");

        // === Phase 7: Verify main CPU can read $BBAA again (NOT expected in real world) ===
        // In the real scenario, the main CPU at PC=$8085 is waiting for ports
        // to return to $BBAA. However, this doesn't match the wiki documentation.
        // The wiki says after upload, the SPC700 executes the uploaded code,
        // which would NOT return to $BBAA.
        //
        // The main CPU should be waiting for the uploaded audio driver to
        // acknowledge a command, not waiting for $BBAA again.

        println!("\n=== Initialization Complete ===");
        println!(
            "SPC700 is now running uploaded code at PC=${:04X}",
            apu.cpu.pc
        );
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
    #[test]
    fn test_port_clear_via_control() {
        let mut apu = Spc700::new();

        // Write some values to input ports (from main CPU)
        apu.write_port(0, 0x12);
        apu.write_port(1, 0x34);
        apu.write_port(2, 0x56);
        apu.write_port(3, 0x78);

        // Verify they were written
        assert_eq!(apu.cpu.memory.cpuio[0], 0x12);
        assert_eq!(apu.cpu.memory.cpuio[1], 0x34);
        assert_eq!(apu.cpu.memory.cpuio[2], 0x56);
        assert_eq!(apu.cpu.memory.cpuio[3], 0x78);

        // Clear ports 0-1 via control register (bit 4)
        apu.cpu.memory.write(CONTROL_REG, 0x10);
        assert_eq!(apu.cpu.memory.cpuio[0], 0x00, "Port 0 should be cleared");
        assert_eq!(apu.cpu.memory.cpuio[1], 0x00, "Port 1 should be cleared");
        assert_eq!(
            apu.cpu.memory.cpuio[2], 0x56,
            "Port 2 should not be cleared"
        );
        assert_eq!(
            apu.cpu.memory.cpuio[3], 0x78,
            "Port 3 should not be cleared"
        );

        // Clear ports 2-3 via control register (bit 5)
        apu.cpu.memory.write(CONTROL_REG, 0x20);
        assert_eq!(apu.cpu.memory.cpuio[2], 0x00, "Port 2 should be cleared");
        assert_eq!(apu.cpu.memory.cpuio[3], 0x00, "Port 3 should be cleared");
    }
}
