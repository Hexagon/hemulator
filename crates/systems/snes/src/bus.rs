//! SNES memory bus implementation

use crate::cartridge::Cartridge;
use crate::ppu::Ppu;
use crate::SnesError;
use emu_core::apu::Spc700;
use emu_core::cpu_65c816::Memory65c816;
use emu_core::logging::{log, LogCategory, LogLevel};
use std::cell::{Cell, RefCell};

/// DMA channel configuration (one per channel, 8 total)
#[derive(Clone, Copy)]
struct DmaChannel {
    /// $43x0 - DMA control (direction, increment, mode)
    control: u8,
    /// $43x1 - B-bus address (PPU register)
    b_addr: u8,
    /// $43x2-$43x4 - A-bus address (24-bit)
    a_addr: u32,
    /// $43x5-$43x6 - Transfer size (0 = 65536)
    size: u16,
    /// $43x7 - HDMA indirect address bank (HDMA only)
    hdma_bank: u8,
    /// $43x8-$43x9 - HDMA table address (HDMA only)
    hdma_table: u16,
    /// $43xA - HDMA line counter (HDMA only)
    hdma_line: u8,
}

/// HDMA channel state (runtime state, not registers)
#[derive(Clone, Copy, Default)]
struct HdmaState {
    /// Current table address
    table_addr: u32,
    /// Current line counter
    line_counter: u8,
    /// Repeat mode flag
    repeat: bool,
    /// Channel is active
    active: bool,
}

/// APU state machine for handling multi-session upload protocol
#[derive(Clone, Copy, Debug, PartialEq, Default)]
enum ApuState {
    /// Idle state - waiting for initial boot or new session
    #[default]
    Idle,
    /// Boot ready - just returned $BBAA signature
    BootReady,
    /// Uploading data - echoing bytes and counting
    Uploading,
    /// Ready - ready for next command after processing
    Ready,
}

impl Default for DmaChannel {
    fn default() -> Self {
        Self {
            control: 0xFF,
            b_addr: 0xFF,
            a_addr: 0xFFFFFF,
            size: 0xFFFF,
            hdma_bank: 0xFF,
            hdma_table: 0xFFFF,
            hdma_line: 0xFF,
        }
    }
}

/// SNES memory bus
pub struct SnesBus {
    /// 128KB WRAM (work RAM)
    wram: [u8; 0x20000],
    /// Cartridge (optional)
    cartridge: Option<Cartridge>,
    /// PPU (Picture Processing Unit)
    ppu: Ppu,
    /// Frame counter for VBlank emulation
    frame_counter: u64,
    /// Cycle counter within current frame for VBlank timing
    /// NTSC SNES: ~89,342 cycles/frame, VBlank starts around cycle 75,000
    frame_cycle: u32,
    /// Cached VBlank state (updated when frame_cycle changes)
    cached_in_vblank: Cell<bool>,
    /// Cached HBlank state (updated when frame_cycle changes)
    cached_in_hblank: Cell<bool>,
    /// Last frame_cycle value when cache was updated
    last_cached_cycle: Cell<u32>,

    /// Last main CPU PC (PBR:PC as u32) observed at an instruction boundary.
    /// Used for targeted debug logging inside the bus.
    last_cpu_pc: u32,
    /// Controller state (16 bits per controller)
    /// Button mapping: B Y Select Start Up Down Left Right A X L R 0 0 0 0
    pub controller_state: [u16; 2],
    /// Controller shift registers for serial readout
    controller_shift: [Cell<u16>; 2],
    /// Controller strobe state
    controller_strobe: bool,
    /// Auto-joypad read enable ($4200 bit 0)
    auto_joypad_enable: bool,
    /// DMA channels (8 channels)
    dma_channels: [DmaChannel; 8],
    /// HDMA enable register ($420C)
    hdma_enable: u8,
    /// HDMA State for each channel
    hdma_state: [HdmaState; 8],
    /// APU communication ports ($2140-$2143)
    ///
    /// On real SNES hardware, reads and writes go to separate latches:
    /// - Writes: S-CPU -> SPC700 (input ports)
    /// - Reads:  SPC700 -> S-CPU (output ports)
    ///
    /// The stub needs to model this separation. If writes update the readback values,
    /// games can fail early APU handshake checks (notably the $BBAA signature).
    apu_in_ports: [u8; 4],
    apu_out_ports: [u8; 4],
    /// APU communication state tracker (tracks S-CPU writes into input ports)
    apu_last_written: [u8; 4],
    /// Cycle counter for simulating APU response delay
    apu_response_delay: u32,
    /// Counter for data transfer sequences to detect completion
    apu_transfer_counter: u8,
    /// Current APU state machine state
    apu_state: ApuState,
    /// Session identifier to track different upload sequences
    apu_session_id: u8,
    /// Hardware-accurate SPC700 APU (always enabled for audio output)
    spc700: Option<RefCell<Spc700>>,
    /// Track pending SPC700 cycles for synchronization
    /// This ensures the SPC700 gets enough time to process writes before reads
    spc700_pending_cycles: Cell<u32>,
    /// Fractional accumulator for SPC700 cycle conversion to prevent rounding error drift
    /// Stores the remainder from integer division (in units of 1/3580)
    spc700_cycle_accumulator: Cell<u64>,

    /// Main CPU data bus open-bus value (best-effort).
    /// Used for undefined hardware register reads.
    open_bus: Cell<u8>,

    /// $4201/$4213 - WRIO/RDIO (I/O port) latch
    wrio: u8,

    /// $4202/$4203 -> $4216/$4217 multiplication registers/result
    wrmpya: u8,
    wrmpyb: u8,
    math_4216: u16,

    /// $4204-$4206 -> $4214-$4217 division registers/results
    wrdiv: u16,
    wrdivb: u8,
    div_quotient: u16,

    /// $4207-$420A - H/V timer registers
    htime: u16,
    vtime: u16,

    /// IRQ flag (set when H/V timer matches)
    irq_flag: Cell<bool>,

    /// H/V timer IRQ enable mode from $4200 bits 4-5
    /// 00 = disabled, 01 = H only, 10 = V only, 11 = HV
    hv_irq_mode: u8,

    /// $420D - MEMSEL (FastROM)
    memsel: u8,

    /// $2180-$2183 - WRAM access port (WMDATA/WMADD)
    ///
    /// Allows reading/writing the full 128KB WRAM via a 17-bit address register.
    /// Writes/reads to $2180 auto-increment the address.
    wram_port_addr: Cell<u32>,

    /// Pending DMA cycles that halt the CPU
    /// When non-zero, the CPU should not execute and these cycles should be consumed
    pending_dma_cycles: Cell<u32>,
}

impl SnesBus {
    // Timing constants (approximate CPU cycles, see lib.rs for detailed explanation)
    // Reference: https://wiki.superfamicom.org/timing
    // - Actual hardware: 1364 master cycles per scanline, 262 scanlines per frame
    // - CPU cycles are abstract and depend on operation type (IO vs memory access)
    // - These constants are tuned for the emulator's CPU cycle tracking
    const SCANLINE_CYCLES: u32 = 341; // Approximate CPU cycles per scanline
    const HBLANK_CYCLES: u32 = 40; // Approximate CPU cycles during H-blank (~40-60 depending on HDMA)
    pub fn new() -> Self {
        log(LogCategory::Bus, LogLevel::Info, || {
            "SNES Bus: Initializing with hardware-accurate SPC700 APU".to_string()
        });

        Self {
            wram: [0; 0x20000],
            cartridge: None,
            ppu: Ppu::new(),
            frame_counter: 0,
            frame_cycle: 0,
            cached_in_vblank: Cell::new(false),
            cached_in_hblank: Cell::new(false),
            last_cached_cycle: Cell::new(0),
            last_cpu_pc: 0,
            controller_state: [0; 2],
            controller_shift: [Cell::new(0), Cell::new(0)],
            controller_strobe: false,
            auto_joypad_enable: true, // Default to enabled
            dma_channels: [DmaChannel::default(); 8],
            hdma_enable: 0,
            hdma_state: [HdmaState::default(); 8],
            apu_in_ports: [0x00, 0x00, 0x00, 0x00],
            apu_out_ports: [0xAA, 0xBB, 0x00, 0x00], // Initial values for APU ready state (SPC700 IPL sets ports to $BBAA when read as 16-bit)
            apu_last_written: [0; 4],
            apu_response_delay: 0,
            apu_transfer_counter: 0,
            apu_state: ApuState::BootReady, // Start in BootReady state with $BBAA signature
            apu_session_id: 0,
            spc700: Some(RefCell::new(Spc700::new())), // Use hardware-accurate SPC700 APU by default
            spc700_pending_cycles: Cell::new(0),
            spc700_cycle_accumulator: Cell::new(0),

            open_bus: Cell::new(0),
            wrio: 0,
            wrmpya: 0,
            wrmpyb: 0,
            math_4216: 0,
            wrdiv: 0,
            wrdivb: 0,
            div_quotient: 0,
            htime: 0,
            vtime: 0,
            irq_flag: Cell::new(false),
            hv_irq_mode: 0,
            memsel: 0,

            wram_port_addr: Cell::new(0),
            pending_dma_cycles: Cell::new(0),
        }
    }

    pub fn set_last_cpu_pc(&mut self, pc: u32) {
        self.last_cpu_pc = pc;
    }

    /// Re-initialize the SPC700 APU (creates a new instance)
    /// Note: SPC700 is enabled by default, this can be used to reset it
    #[allow(dead_code)]
    pub fn enable_spc700(&mut self) {
        log(LogCategory::Bus, LogLevel::Info, || {
            "SNES Bus: (Re)initializing SPC700 APU".to_string()
        });
        self.spc700 = Some(RefCell::new(Spc700::new()));
    }

    /// Disable the SPC700 APU and use stub protocol for testing
    #[allow(dead_code)]
    pub fn disable_spc700(&mut self) {
        log(LogCategory::Bus, LogLevel::Info, || {
            "SNES Bus: Disabling SPC700 APU (using stub protocol)".to_string()
        });
        self.spc700 = None;
    }

    pub fn load_cartridge(&mut self, data: &[u8]) -> Result<(), SnesError> {
        log(LogCategory::Bus, LogLevel::Info, || {
            format!("SNES Bus: Loading cartridge ({} bytes)", data.len())
        });
        self.cartridge = Some(Cartridge::load(data)?);
        Ok(())
    }

    pub fn unload_cartridge(&mut self) {
        log(LogCategory::Bus, LogLevel::Info, || {
            "SNES Bus: Unloading cartridge".to_string()
        });
        self.cartridge = None;
    }

    pub fn has_cartridge(&self) -> bool {
        self.cartridge.is_some()
    }

    pub fn ppu(&self) -> &Ppu {
        &self.ppu
    }

    pub fn ppu_mut(&mut self) -> &mut Ppu {
        &mut self.ppu
    }

    /// Trigger H/V timer IRQ (sets IRQ flag)
    pub fn trigger_hv_irq(&self) {
        self.irq_flag.set(true);
    }

    /// Get H/V IRQ mode for debugging/logging
    pub fn get_hv_irq_mode(&self) -> u8 {
        self.hv_irq_mode
    }

    /// Check if there are pending DMA cycles that should halt the CPU
    pub fn has_pending_dma(&self) -> bool {
        self.pending_dma_cycles.get() > 0
    }

    /// Get the number of pending DMA cycles
    #[allow(dead_code)]
    pub fn get_pending_dma_cycles(&self) -> u32 {
        self.pending_dma_cycles.get()
    }

    /// Consume pending DMA cycles
    /// Returns the actual number of cycles consumed
    pub fn consume_dma_cycles(&self, cycles: u32) -> u32 {
        let pending = self.pending_dma_cycles.get();
        let consumed = cycles.min(pending);
        self.pending_dma_cycles.set(pending - consumed);
        consumed
    }

    /// Get mutable reference to SPC700 APU if enabled
    pub fn spc700_mut(&mut self) -> Option<std::cell::RefMut<'_, Spc700>> {
        self.spc700.as_ref().map(|rc| rc.borrow_mut())
    }

    pub fn tick_frame(&mut self) {
        self.frame_counter += 1;
        self.frame_cycle = 0; // Reset cycle counter at frame start
    }

    /// Update cycle counter within frame (called after each CPU step)
    pub fn tick_cycles(&mut self, cycles: u32) {
        self.frame_cycle += cycles;

        // Tick cartridge enhancement chip (e.g., SuperFX) with master cycles
        // SuperFX runs at the master clock frequency (21.48 MHz)
        // Main CPU cycles are abstract units, but we approximate master cycles as CPU cycles * 6
        // (since most CPU operations take 6 master cycles)
        if let Some(ref mut cart) = self.cartridge {
            let master_cycles = cycles * 6;
            cart.tick_chip(master_cycles);
        }

        // Convert main CPU cycles to SPC700 cycles using proper clock ratio
        // Main CPU: ~3.58 MHz (NTSC)
        // SPC700: ~1.024 MHz
        // Ratio: 1.024 / 3.58 ≈ 0.286 (SPC700 runs at about 28.6% of main CPU speed)
        // Using integer math with fractional accumulator to prevent rounding error drift:
        // SPC700 cycles = CPU cycles * 1024 / 3580

        // Calculate SPC700 cycles with fractional tracking
        let numerator = cycles as u64 * 1024;
        let accumulator = self.spc700_cycle_accumulator.get();
        let total = numerator + accumulator;
        let spc700_cycles = total / 3580;
        let remainder = total % 3580;

        // Store remainder for next calculation to prevent drift
        self.spc700_cycle_accumulator.set(remainder);

        // Accumulate cycles for SPC700 instead of running immediately
        // This allows us to synchronize before port access
        // Use saturating_add to prevent overflow
        let current = self.spc700_pending_cycles.get() as u64;
        let total_pending = current.saturating_add(spc700_cycles);
        let clamped = u32::try_from(total_pending).unwrap_or(u32::MAX);
        self.spc700_pending_cycles.set(clamped);

        // Decrement APU response delay for simulating processing time
        if self.apu_response_delay > 0 {
            self.apu_response_delay = self.apu_response_delay.saturating_sub(cycles);
        }
    }

    /// Synchronize SPC700 to current cycle count
    /// Called before APU port reads/writes to ensure proper timing
    #[inline]
    fn sync_spc700(&self) {
        if let Some(ref spc700_cell) = self.spc700 {
            let pending = self.spc700_pending_cycles.get();
            if pending > 0 {
                spc700_cell.borrow_mut().run_cycles(pending);
                self.spc700_pending_cycles.set(0);
            }
        }
    }

    /// Check if currently in VBlank period
    ///
    /// Hardware behavior (https://wiki.superfamicom.org/timing):
    /// - VBlank starts at scanline $E1 (225) or $F0 (240) depending on $2133 bit 2
    /// - VBlank ends at scanline 0 (V=0 H=0)
    /// - Total of 262 scanlines per frame (NTSC, non-interlace)
    ///
    /// Implementation:
    /// - We use scanline 225 as VBlank start (standard configuration)
    /// - Scanline 224 is the last fully visible scanline
    /// - This gives us 262 - 225 = 37 VBlank scanlines
    /// - Cached to avoid expensive division in hot path
    fn is_in_vblank(&self) -> bool {
        // Update cache if frame_cycle changed
        if self.last_cached_cycle.get() != self.frame_cycle {
            self.update_blanking_cache();
        }
        self.cached_in_vblank.get()
    }

    /// Check if currently in HBlank period (approximate).
    ///
    /// Hardware behavior (https://wiki.superfamicom.org/timing):
    /// - H-Blank begins at H=274 of every scanline
    /// - H-Blank ends at H=1 (next scanline)
    /// - This gives roughly 66-67 dots (264-268 master cycles) for H-Blank
    /// - HDMA transfers occur during H-Blank starting at dot 278
    ///
    /// Implementation:
    /// - We model H-Blank as the last ~40 CPU cycles of each scanline
    /// - This is an approximation since CPU cycle timing varies by operation
    /// - Cached to avoid expensive modulo in hot path
    ///
    /// We model HBlank as the last ~40 cycles of each scanline.
    fn is_in_hblank(&self) -> bool {
        // Update cache if frame_cycle changed
        if self.last_cached_cycle.get() != self.frame_cycle {
            self.update_blanking_cache();
        }
        self.cached_in_hblank.get()
    }

    /// Update the cached VBlank/HBlank state based on current frame_cycle.
    /// Called only when frame_cycle changes, avoiding expensive division/modulo in hot path.
    #[inline]
    fn update_blanking_cache(&self) {
        let current_scanline = self.frame_cycle / Self::SCANLINE_CYCLES;
        let cycle_in_scanline = self.frame_cycle % Self::SCANLINE_CYCLES;

        // VBlank is active during scanlines 225-261 (NTSC has 262 scanlines total)
        self.cached_in_vblank.set(current_scanline >= 225);

        // HBlank is the last ~40 cycles of each scanline
        self.cached_in_hblank
            .set(cycle_in_scanline >= (Self::SCANLINE_CYCLES - Self::HBLANK_CYCLES));

        // Update the cached cycle value
        self.last_cached_cycle.set(self.frame_cycle);
    }

    /// Check if H/V timer IRQ should trigger
    ///
    /// Hardware behavior (https://sneslab.net/wiki/H/V_Count_Timer):
    /// - Mode 00 (hv_irq_mode=0): Timer off, never triggers
    /// - Mode 01 (hv_irq_mode=1): H-IRQ only - triggers every scanline at H = HTIME + ~3.5 cycles
    /// - Mode 10 (hv_irq_mode=2): V-IRQ only - triggers at V = VTIME, H ≈ 2.5
    /// - Mode 11 (hv_irq_mode=3): HV-IRQ - triggers at V=VTIME and H=HTIME + ~3.5 cycles
    ///
    /// Returns true if IRQ should be triggered
    pub fn check_hv_timer_irq(&self, scanline: u32, h_pos: u32) -> bool {
        match self.hv_irq_mode {
            0 => false, // Timer off
            1 => {
                // H-timer only: trigger every scanline when H matches
                // Approximate HTIME comparison (real hardware has +3.5 cycle offset)
                h_pos as u16 == self.htime
            }
            2 => {
                // V-timer only: trigger on specific scanline
                // Trigger at beginning of scanline (H ≈ 2.5)
                scanline as u16 == self.vtime && h_pos < 10
            }
            3 => {
                // HV-timer: trigger on specific scanline AND H-position
                scanline as u16 == self.vtime && h_pos as u16 == self.htime
            }
            _ => false,
        }
    }

    /// Set controller state (16 buttons) for controller `idx` (0 or 1).
    /// Button layout: B Y Select Start Up Down Left Right A X L R 0 0 0 0
    pub fn set_controller(&mut self, idx: usize, state: u16) {
        if idx < 2 {
            log(LogCategory::Bus, LogLevel::Debug, || {
                format!(
                    "SNES Bus: Controller {} state set to 0x{:04X}",
                    idx + 1,
                    state
                )
            });
            self.controller_state[idx] = state;
        }
    }

    pub fn get_rom_size(&self) -> usize {
        if let Some(ref cart) = self.cartridge {
            cart.rom_size()
        } else {
            0
        }
    }

    pub fn has_smc_header(&self) -> bool {
        if let Some(ref cart) = self.cartridge {
            cart.has_smc_header()
        } else {
            false
        }
    }

    pub fn get_mapping_mode(&self) -> String {
        if let Some(ref cart) = self.cartridge {
            if cart.is_exhirom() {
                "ExHiROM".to_string()
            } else if cart.is_hirom() {
                "HiROM".to_string()
            } else {
                "LoROM".to_string()
            }
        } else {
            "Unknown".to_string()
        }
    }

    pub fn get_chip_type(&self) -> String {
        if let Some(ref cart) = self.cartridge {
            cart.chip_type().name().to_string()
        } else {
            "Unknown".to_string()
        }
    }

    /// Perform DMA transfer for specified channels
    /// Returns number of cycles consumed
    ///
    /// # DMA Transfer Modes (bits 0-2 of $43x0)
    ///
    /// According to https://wiki.superfamicom.org/dma-and-hdma:
    ///
    /// - Mode 0 (000): 1 byte to 1 register (write once)
    ///   - Pattern: b_addr
    ///   - Used for: Single register updates
    ///
    /// - Mode 1 (001): 2 bytes to 2 registers (write once)
    ///   - Pattern: b_addr, b_addr+1
    ///   - Used for: Paired register updates (e.g., VMDATAL/VMDATAH)
    ///
    /// - Mode 2 (010): 2 bytes to 1 register (write twice)
    ///   - Pattern: b_addr, b_addr
    ///   - Used for: Writing 16-bit values to 8-bit registers
    ///
    /// - Mode 3 (011): 4 bytes to 2 registers (write twice each)
    ///   - Pattern: b_addr, b_addr, b_addr+1, b_addr+1
    ///   - Used for: Two paired register updates
    ///
    /// - Mode 4 (100): 4 bytes to 4 registers (write once)
    ///   - Pattern: b_addr, b_addr+1, b_addr+2, b_addr+3
    ///   - Used for: Consecutive register updates
    ///
    /// Modes 5-7 are mirrors/combinations of the above modes.
    ///
    /// # Timing
    ///
    /// - 8 master cycles per byte transferred
    /// - 8 master cycles overhead per channel
    /// - 12-24 cycles overhead for the whole transfer
    ///
    /// # Direction and Addressing
    ///
    /// - Bit 7 of $43x0: 0 = A-bus → B-bus, 1 = B-bus → A-bus
    /// - Bits 3-4 of $43x0: Address adjustment mode
    ///   - 00: Increment A-bus address after each byte
    ///   - 01: Fixed (no change)
    ///   - 10/11: Decrement A-bus address after each byte
    pub fn do_dma(&mut self, channels: u8) -> u32 {
        let mut cycles = 0u32;

        // Process each enabled channel
        for ch in 0..8 {
            if (channels & (1 << ch)) == 0 {
                continue;
            }

            let dma = self.dma_channels[ch];
            let direction = (dma.control & 0x80) != 0; // 0 = A->B, 1 = B->A
            let increment_mode = (dma.control >> 3) & 0x03; // 00=inc, 01=fixed, 10/11=dec
            let transfer_mode = dma.control & 0x07;

            let mut size = if dma.size == 0 {
                0x10000
            } else {
                dma.size as usize
            };
            let mut a_addr = dma.a_addr;

            log(LogCategory::Bus, LogLevel::Debug, || {
                format!(
                    "DMA Channel {}: {} {} bytes from ${:06X} to ${:02X}, mode={}, inc={}",
                    ch,
                    if direction { "B->A" } else { "A->B" },
                    size,
                    a_addr,
                    dma.b_addr,
                    transfer_mode,
                    increment_mode
                )
            });

            // 8 cycles overhead per channel
            cycles += 8;

            // Pre-compute transfer parameters outside the loop for performance
            let bytes_per_transfer = match transfer_mode {
                0 => 1,     // Mode 0: 1 byte to 1 register
                1 | 5 => 2, // Mode 1/5: 2 bytes to 2 registers
                2 | 6 => 2, // Mode 2/6: 2 bytes to 1 register (write twice)
                3 | 7 => 4, // Mode 3/7: 4 bytes to 2 registers (write twice each)
                4 => 4,     // Mode 4: 4 bytes to 4 registers
                _ => 1,
            };

            // Transfer loop - optimized by hoisting bytes_per_transfer out of the inner loop
            while size > 0 {
                let count = bytes_per_transfer.min(size);

                for i in 0..count {
                    // Calculate B-bus register address based on transfer mode for each byte.
                    // Note: bytes_per_transfer is precomputed, but b_reg is still computed per byte.
                    let b_reg = match transfer_mode {
                        0 => 0x2100 | (dma.b_addr as u16),
                        1 | 5 => {
                            // Alternate: b_addr, b_addr+1
                            0x2100 | ((dma.b_addr as u16) + (i as u16 & 1))
                        }
                        2 | 6 => {
                            // Write twice to same register: b_addr, b_addr
                            0x2100 | (dma.b_addr as u16)
                        }
                        3 | 7 => {
                            // Pattern: b_addr, b_addr, b_addr+1, b_addr+1
                            0x2100 | ((dma.b_addr as u16) + ((i as u16 >> 1) & 1))
                        }
                        4 => {
                            // Four consecutive registers: b_addr, b_addr+1, b_addr+2, b_addr+3
                            // e.g., if b_addr=0x18, accesses $2118, $2119, $211A, $211B
                            0x2100 | ((dma.b_addr as u16) + (i as u16 & 3))
                        }
                        _ => 0x2100 | (dma.b_addr as u16),
                    };

                    if direction {
                        // B-bus to A-bus (rare, mostly for reading from PPU)
                        let val = self.read(b_reg as u32);
                        self.write(a_addr, val);
                    } else {
                        // A-bus to B-bus (common, writing to VRAM/CGRAM/OAM)
                        let val = self.read(a_addr);
                        self.write(b_reg as u32, val);
                    }

                    // Update A-bus address based on increment mode
                    match increment_mode {
                        0 => a_addr += 1,     // Increment
                        1 => {}               // Fixed
                        2 | 3 => a_addr -= 1, // Decrement
                        _ => {}
                    }

                    size -= 1;
                    cycles += 8; // 8 master cycles per byte
                }
            }
        }

        cycles
    }

    /// Initialize HDMA channels at start of frame (V=0, H≈6)
    ///
    /// According to https://wiki.superfamicom.org/dma-and-hdma:
    ///
    /// 1. Copy AAddress ($43x2-4) into internal Address register
    /// 2. Load $43xA (Line Counter and Repeat flag) from the table
    /// 3. Load Indirect Address if using indirect mode
    /// 4. Set Do Transfer flag to true
    ///
    /// # Timing
    ///
    /// - ~18 master cycles overhead
    /// - 8 master cycles per channel (direct mode)
    /// - 24 master cycles per channel (indirect mode)
    pub fn init_hdma(&mut self) {
        for ch in 0..8 {
            if (self.hdma_enable & (1 << ch)) != 0 {
                let dma = self.dma_channels[ch];

                // Initialize HDMA state from registers
                self.hdma_state[ch].table_addr =
                    (dma.hdma_table as u32) | ((dma.hdma_bank as u32) << 16);
                self.hdma_state[ch].line_counter = 0;
                self.hdma_state[ch].repeat = false;
                self.hdma_state[ch].active = true;

                log(LogCategory::Bus, LogLevel::Debug, || {
                    format!(
                        "HDMA Channel {} initialized: table=${:06X}",
                        ch, self.hdma_state[ch].table_addr
                    )
                });
            } else {
                self.hdma_state[ch].active = false;
            }
        }
    }

    /// Execute HDMA for all active channels (called during H-blank of each scanline)
    ///
    /// According to https://wiki.superfamicom.org/dma-and-hdma:
    ///
    /// Per-scanline process (V=0 to V=$E0, H≈$116):
    ///
    /// 1. If Do Transfer is false, skip to step 3
    /// 2. Transfer the appropriate number of bytes for the transfer mode
    /// 3. Decrement Line Counter
    /// 4. Set Do Transfer to the Repeat flag value
    /// 5. If Line Counter is zero:
    ///    - Read next byte from Address into Line Counter and Repeat
    ///    - If indirect mode, read 2-byte indirect address
    ///    - If new Line Counter is 0, terminate channel for this frame
    ///    - Set Do Transfer to true
    ///
    /// # Timing
    ///
    /// - ~18 master cycles overhead per scanline (if any channel active)
    /// - 8 master cycles per active channel
    /// - 16 master cycles for indirect address load (when needed)
    /// - 8 master cycles per byte transferred
    ///
    /// Maximum: 466 master cycles per scanline (all 8 channels active with indirect addressing)
    pub fn do_hdma(&mut self) -> u32 {
        let mut cycles = 0u32;

        for ch in 0..8 {
            if !self.hdma_state[ch].active {
                continue;
            }

            let dma = self.dma_channels[ch];

            // Check if we need to fetch a new line count
            if self.hdma_state[ch].line_counter == 0 {
                // Read line count byte from table
                let line_byte = self.read(self.hdma_state[ch].table_addr);
                self.hdma_state[ch].table_addr += 1;

                // Check for termination (line count = 0)
                if line_byte == 0 {
                    self.hdma_state[ch].active = false;
                    log(LogCategory::Bus, LogLevel::Debug, || {
                        format!("HDMA Channel {} terminated", ch)
                    });
                    continue;
                }

                // Extract repeat flag and line count
                self.hdma_state[ch].repeat = (line_byte & 0x80) != 0;
                self.hdma_state[ch].line_counter = line_byte & 0x7F;

                // For indirect mode, read the indirect address
                let indirect = (dma.control & 0x40) != 0;
                if indirect {
                    // Read 2-byte indirect address
                    let addr_low = self.read(self.hdma_state[ch].table_addr) as u32;
                    let addr_high = self.read(self.hdma_state[ch].table_addr + 1) as u32;
                    self.hdma_state[ch].table_addr += 2;

                    // Store indirect address (will be used for data fetch)
                    // For now, we'll use the A-bus address field temporarily
                    self.dma_channels[ch].a_addr =
                        addr_low | (addr_high << 8) | ((dma.hdma_bank as u32) << 16);
                }
            }

            // Perform the transfer
            let transfer_mode = dma.control & 0x07;
            let bytes_to_transfer = match transfer_mode {
                0 => 1,     // Mode 0: 1 byte
                1 | 5 => 2, // Mode 1/5: 2 bytes
                2 | 6 => 2, // Mode 2/6: 2 bytes
                3 | 7 => 4, // Mode 3/7: 4 bytes
                4 => 4,     // Mode 4: 4 bytes
                _ => 1,
            };

            let indirect = (dma.control & 0x40) != 0;
            let source_addr = if indirect {
                // Use indirect address
                dma.a_addr
            } else {
                // Use table address directly
                self.hdma_state[ch].table_addr
            };

            // Transfer the bytes
            for i in 0..bytes_to_transfer {
                let b_reg = match transfer_mode {
                    0 => 0x2100 | (dma.b_addr as u16),
                    1 | 5 => 0x2100 | ((dma.b_addr as u16) + (i as u16 & 1)),
                    2 | 6 => 0x2100 | (dma.b_addr as u16),
                    3 | 7 => 0x2100 | ((dma.b_addr as u16) + ((i as u16 >> 1) & 1)),
                    4 => {
                        // Four consecutive registers: b_addr, b_addr+1, b_addr+2, b_addr+3
                        0x2100 | ((dma.b_addr as u16) + (i as u16 & 3))
                    }
                    _ => 0x2100 | (dma.b_addr as u16),
                };

                let val = self.read(source_addr + i as u32);
                self.write(b_reg as u32, val);
                cycles += 8; // 8 cycles per byte
            }

            // Update addresses for next scanline
            if indirect {
                // Increment indirect address
                self.dma_channels[ch].a_addr += bytes_to_transfer as u32;
            } else if !self.hdma_state[ch].repeat {
                // Increment table address (only if not in repeat mode)
                self.hdma_state[ch].table_addr += bytes_to_transfer as u32;
            }

            // Decrement line counter
            self.hdma_state[ch].line_counter -= 1;
        }

        cycles
    }
}

impl Default for SnesBus {
    fn default() -> Self {
        Self::new()
    }
}

impl Memory65c816 for SnesBus {
    fn read(&self, addr: u32) -> u8 {
        let bank = (addr >> 16) as u8;
        let offset = (addr & 0xFFFF) as u16;

        match bank {
            // Banks $00-$3F and $80-$BF: System area
            0x00..=0x3F | 0x80..=0xBF => {
                match offset {
                    // WRAM (shadow at $0000-$1FFF)
                    0x0000..=0x1FFF => self.wram[offset as usize],

                    // $2180-$2183 - WRAM access port
                    0x2180 => {
                        let cur = self.wram_port_addr.get();
                        let a = (cur as usize) & 0x1FFFF;
                        let v = self.wram[a];
                        self.open_bus.set(v);
                        // Auto-increment (wrap at 128KB)
                        let next = (cur + 1) & 0x1FFFF;
                        self.wram_port_addr.set(next);
                        v
                    }
                    0x2181 => {
                        let v = (self.wram_port_addr.get() & 0xFF) as u8;
                        self.open_bus.set(v);
                        v
                    }
                    0x2182 => {
                        let v = ((self.wram_port_addr.get() >> 8) & 0xFF) as u8;
                        self.open_bus.set(v);
                        v
                    }
                    0x2183 => {
                        let v = ((self.wram_port_addr.get() >> 16) & 0x01) as u8;
                        self.open_bus.set(v);
                        v
                    }

                    // $2140-$2143 - APUIO0-3 - APU Communication Ports
                    // Main CPU reads what SPC700 has written (apu_out ports)
                    0x2140..=0x2143 => {
                        let port = (offset - 0x2140) as u8;

                        // Synchronize SPC700 before reading to ensure it has processed any pending writes
                        // This matches Mesen2's approach of calling Run() before port access
                        self.sync_spc700();

                        // Simply read the current port value - no latching needed
                        // Real SNES hardware has no latching for APU port reads
                        let val = if let Some(ref spc700_cell) = self.spc700 {
                            spc700_cell.borrow().read_port(port)
                        } else {
                            let port_idx = port as usize;
                            // Defensive bounds check: the match range 0x2140..=0x2143 guarantees port is 0-3,
                            // but we verify explicitly before array indexing for clarity and safety
                            debug_assert!(
                                port_idx < 4,
                                "APU port index must be 0-3, got {}",
                                port_idx
                            );
                            self.apu_out_ports[port_idx]
                        };

                        // Hot path optimization: Remove debug logging from APU port reads
                        // APU ports are accessed frequently and logging here is a major performance bottleneck
                        val
                    }
                    // Hardware registers (PPU: $2100-$213F, excluding APU ports above)
                    // Note: $2140-$2143 are handled by the APU case above
                    0x2100..=0x213F => self.ppu.read_register(offset),
                    // $4200 - NMITIMEN - Interrupt Enable and Joypad Request
                    0x4200 => {
                        // Bit 7: NMI enable
                        // Other bits: H/V timer interrupt enable, auto-joypad read enable
                        let v = (if self.ppu.nmi_enable { 0x80 } else { 0x00 })
                            | (if self.auto_joypad_enable { 0x01 } else { 0x00 });
                        self.open_bus.set(v);
                        v
                    }
                    // $4201 - WRIO (readback)
                    0x4201 => {
                        let v = self.wrio;
                        self.open_bus.set(v);
                        v
                    }
                    // $4202/$4203 - WRMPYA/WRMPYB (readback)
                    0x4202 => {
                        let v = self.wrmpya;
                        self.open_bus.set(v);
                        v
                    }
                    0x4203 => {
                        let v = self.wrmpyb;
                        self.open_bus.set(v);
                        v
                    }
                    // $4204-$4206 - WRDIVL/WRDIVH/WRDIVB (readback)
                    0x4204 => {
                        let v = (self.wrdiv & 0xFF) as u8;
                        self.open_bus.set(v);
                        v
                    }
                    0x4205 => {
                        let v = ((self.wrdiv >> 8) & 0xFF) as u8;
                        self.open_bus.set(v);
                        v
                    }
                    0x4206 => {
                        let v = self.wrdivb;
                        self.open_bus.set(v);
                        v
                    }
                    // $4207-$420A - HTIMEL/HTIMEH/VTIMEL/VTIMEH
                    0x4207 => {
                        let v = (self.htime & 0xFF) as u8;
                        self.open_bus.set(v);
                        v
                    }
                    0x4208 => {
                        let v = ((self.htime >> 8) & 0xFF) as u8;
                        self.open_bus.set(v);
                        v
                    }
                    0x4209 => {
                        let v = (self.vtime & 0xFF) as u8;
                        self.open_bus.set(v);
                        v
                    }
                    0x420A => {
                        let v = ((self.vtime >> 8) & 0xFF) as u8;
                        self.open_bus.set(v);
                        v
                    }
                    // $420D - MEMSEL
                    0x420D => {
                        let v = self.memsel;
                        self.open_bus.set(v);
                        v
                    }
                    // $4210 - RDNMI - NMI Flag (read and clear)
                    0x4210 => {
                        // Bit 7: NMI flag (set at start of VBlank if NMI enabled)
                        // Bits 0-3: CPU version (return 2 for 65C816)
                        // Reading this register clears the NMI flag
                        let nmi_flag = if self.ppu.nmi_flag.get() { 0x80 } else { 0x00 };
                        self.ppu.clear_nmi_flag();
                        // Hot path optimization: Remove trace logging from RDNMI reads
                        // This register is read frequently during NMI handling
                        nmi_flag | 0x02 // CPU version 2
                    }
                    // $4211 - TIMEUP - IRQ Flag (read and clear)
                    0x4211 => {
                        // Bit 7: IRQ flag
                        // Reading this register clears the IRQ flag
                        let irq = if self.irq_flag.get() { 0x80 } else { 0x00 };
                        self.irq_flag.set(false); // Clear IRQ flag on read
                        log(LogCategory::Interrupts, LogLevel::Debug, || {
                            format!("SNES Bus: Read $4211 TIMEUP = ${:02X}", irq)
                        });
                        irq
                    }
                    // $4212 - HVBJOY - H/V Blank and Joypad Status
                    0x4212 => {
                        // Bit 7: VBlank flag (set during VBlank period)
                        // Bit 6: HBlank flag
                        // Bit 0: Auto-joypad read in progress (0 = finished)
                        let mut val = 0u8;
                        if self.is_in_vblank() {
                            val |= 0x80;
                        }
                        if self.is_in_hblank() {
                            val |= 0x40;
                        }

                        // Hot path optimization: Remove trace logging from HVBJOY reads
                        // This register is polled frequently (every frame) to detect VBlank

                        val
                    }
                    // $4213 - RDIO - Programmable I/O port (read)
                    0x4213 => {
                        let v = self.wrio;
                        self.open_bus.set(v);
                        v
                    }
                    // $4214-$4215 - RDDIVL/RDDIVH - Division quotient (read)
                    0x4214 => {
                        let v = (self.div_quotient & 0xFF) as u8;
                        self.open_bus.set(v);
                        v
                    }
                    0x4215 => {
                        let v = ((self.div_quotient >> 8) & 0xFF) as u8;
                        self.open_bus.set(v);
                        v
                    }
                    // $4216-$4217 - RDMPYL/RDMPYH (product) / RDMPY (remainder) low/high
                    0x4216 => {
                        let v = (self.math_4216 & 0xFF) as u8;
                        self.open_bus.set(v);
                        v
                    }
                    0x4217 => {
                        let v = ((self.math_4216 >> 8) & 0xFF) as u8;
                        self.open_bus.set(v);
                        v
                    }
                    // $4016 - JOYSER0 - Controller 1 Serial Data
                    0x4016 => {
                        // Bit 0: Serial data for controller 1
                        // Bits 1-7: Open bus (typically 0)
                        if self.controller_strobe {
                            // While strobed, return bit 0 of the current state
                            (self.controller_state[0] & 1) as u8
                        } else {
                            // Shift out the latched state
                            let cur = self.controller_shift[0].get();
                            let bit = (cur & 1) as u8;
                            self.controller_shift[0].set(cur >> 1);
                            bit
                        }
                    }
                    // $4017 - JOYSER1 - Controller 2 Serial Data
                    0x4017 => {
                        // Bit 0: Serial data for controller 2
                        // Bits 1-4: Not used (0x1E if nothing connected)
                        if self.controller_strobe {
                            (self.controller_state[1] & 1) as u8
                        } else {
                            let cur = self.controller_shift[1].get();
                            let bit = (cur & 1) as u8;
                            self.controller_shift[1].set(cur >> 1);
                            bit
                        }
                    }
                    // $4218-$421F - JOYxL/JOYxH - Auto-joypad read (only valid when auto-read enabled)
                    0x4218 => {
                        if self.auto_joypad_enable {
                            (self.controller_state[0] & 0xFF) as u8 // JOY1L
                        } else {
                            0 // Return 0 when auto-read disabled
                        }
                    }
                    0x4219 => {
                        if self.auto_joypad_enable {
                            ((self.controller_state[0] >> 8) & 0xFF) as u8 // JOY1H
                        } else {
                            0
                        }
                    }
                    0x421A => {
                        if self.auto_joypad_enable {
                            (self.controller_state[1] & 0xFF) as u8 // JOY2L
                        } else {
                            0
                        }
                    }
                    0x421B => {
                        if self.auto_joypad_enable {
                            ((self.controller_state[1] >> 8) & 0xFF) as u8 // JOY2H
                        } else {
                            0
                        }
                    }
                    // JOY3/JOY4 - Controllers 3 and 4 (multitap support not implemented)
                    // Low priority: Most games only use 2 controllers
                    // Would require full multitap implementation for 3-4 player games
                    0x421C => 0, // JOY3L
                    0x421D => 0, // JOY3H
                    0x421E => 0, // JOY4L
                    0x421F => 0, // JOY4H
                    // $420C - HDMAEN - HDMA Enable (read)
                    0x420C => {
                        let v = self.hdma_enable;
                        self.open_bus.set(v);
                        v
                    }
                    // $43x0-$43xA - DMA channel registers (read)
                    0x4300..=0x437F => {
                        let ch = ((offset - 0x4300) >> 4) as usize & 7;
                        let reg = (offset & 0x0F) as usize;
                        match reg {
                            0x0 => self.dma_channels[ch].control,
                            0x1 => self.dma_channels[ch].b_addr,
                            0x2 => (self.dma_channels[ch].a_addr & 0xFF) as u8,
                            0x3 => ((self.dma_channels[ch].a_addr >> 8) & 0xFF) as u8,
                            0x4 => ((self.dma_channels[ch].a_addr >> 16) & 0xFF) as u8,
                            0x5 => (self.dma_channels[ch].size & 0xFF) as u8,
                            0x6 => ((self.dma_channels[ch].size >> 8) & 0xFF) as u8,
                            0x7 => self.dma_channels[ch].hdma_bank,
                            0x8 => (self.dma_channels[ch].hdma_table & 0xFF) as u8,
                            0x9 => ((self.dma_channels[ch].hdma_table >> 8) & 0xFF) as u8,
                            0xA => self.dma_channels[ch].hdma_line,
                            _ => 0xFF, // Open bus for unused registers
                        }
                    }
                    // Other hardware registers
                    0x2000..=0x5FFF => {
                        log(LogCategory::Bus, LogLevel::Trace, || {
                            format!("SNES: Read from stubbed hardware register 0x{:04X} (bank 0x{:02X})", addr, bank)
                        });
                        // Best-effort open bus behavior: return last data bus value.
                        self.open_bus.get()
                    }
                    // $6000-$7FFF: Expansion / cartridge-mapped region (SRAM on HiROM)
                    0x6000..=0x7FFF => {
                        if let Some(ref cart) = self.cartridge {
                            cart.read(addr)
                        } else {
                            self.open_bus.get()
                        }
                    }
                    // Cartridge ROM
                    0x8000..=0xFFFF => {
                        if let Some(ref cart) = self.cartridge {
                            cart.read(addr)
                        } else {
                            0
                        }
                    }
                }
            }
            // Banks $7E-$7F: Full WRAM mirror
            0x7E..=0x7F => {
                let wram_addr = ((bank as usize - 0x7E) << 16) | offset as usize;
                self.wram[wram_addr]
            }
            // Banks $40-$6F and $C0-$FF: Cartridge ROM
            _ => {
                if let Some(ref cart) = self.cartridge {
                    cart.read(addr)
                } else {
                    0
                }
            }
        }
    }

    fn write(&mut self, addr: u32, val: u8) {
        let bank = (addr >> 16) as u8;
        let offset = (addr & 0xFFFF) as u16;

        match bank {
            // Banks $00-$3F and $80-$BF: System area
            0x00..=0x3F | 0x80..=0xBF => {
                match offset {
                    // WRAM (shadow at $0000-$1FFF)
                    0x0000..=0x1FFF => self.wram[offset as usize] = val,

                    // $2180-$2183 - WRAM access port
                    0x2180 => {
                        self.open_bus.set(val);
                        let cur = self.wram_port_addr.get();
                        let a = (cur as usize) & 0x1FFFF;
                        self.wram[a] = val;
                        self.wram_port_addr.set((cur + 1) & 0x1FFFF);
                    }
                    0x2181 => {
                        self.open_bus.set(val);
                        let cur = self.wram_port_addr.get();
                        self.wram_port_addr.set((cur & !0xFF) | (val as u32));
                    }
                    0x2182 => {
                        self.open_bus.set(val);
                        let cur = self.wram_port_addr.get();
                        self.wram_port_addr
                            .set((cur & !(0xFF << 8)) | ((val as u32) << 8));
                    }
                    0x2183 => {
                        self.open_bus.set(val);
                        let cur = self.wram_port_addr.get();
                        self.wram_port_addr
                            .set((cur & !(1 << 16)) | (((val as u32) & 1) << 16));
                    }

                    // $2140-$2143 - APUIO0-3 - APU Communication Ports
                    0x2140..=0x2143 => {
                        let port = (offset - 0x2140) as u8;

                        // Synchronize SPC700 before writing to ensure proper timing
                        self.sync_spc700();

                        // Use real SPC700 if available
                        if let Some(ref spc700_cell) = self.spc700 {
                            // Hot path optimization: Remove trace logging from APU port writes
                            spc700_cell.borrow_mut().write_port(port, val);
                        } else {
                            // Use stub protocol
                            let port = port as usize;

                            // Defensive bounds check: the match range 0x2140..=0x2143 guarantees port is 0-3,
                            // but we verify explicitly before array indexing for clarity and safety
                            debug_assert!(port < 4, "APU port index must be 0-3, got {}", port);

                            // Hot path optimization: Remove trace logging from APU port writes

                            // Enhanced APU communication protocol stub with state machine
                            // The SPC700 boot ROM implements a multi-round handshake protocol
                            // We simulate this with a proper state machine to handle multiple upload sessions

                            // Record S-CPU write into input port latch
                            self.apu_in_ports[port] = val;

                            // Check for boot handshake pattern (all ports cleared to $00)
                            // Must check BEFORE updating apu_last_written
                            let boot_handshake_pattern = port == 3 && {
                                let mut temp = self.apu_last_written;
                                temp[port] = val;
                                temp == [0x00, 0x00, 0x00, 0x00]
                            };

                            // Track what was written for protocol detection (after pattern check)
                            self.apu_last_written[port] = val;

                            // Handle boot handshake
                            if boot_handshake_pattern {
                                log(LogCategory::Bus, LogLevel::Debug, || {
                                    "SNES Bus: APU boot handshake - setting ready signature"
                                        .to_string()
                                });
                                // SPC700 IPL ready signature
                                self.apu_out_ports[0] = 0xAA; // Low byte of $BBAA
                                self.apu_out_ports[1] = 0xBB; // High byte of $BBAA
                                self.apu_out_ports[2] = 0x00;
                                self.apu_out_ports[3] = 0x00;
                                self.apu_response_delay = 10;
                                self.apu_transfer_counter = 0;
                                self.apu_state = ApuState::BootReady;
                                return;
                            }

                            // Detect new session starts based on write patterns
                            // Writes to ports 2-3 (address/control) when in Ready state indicate new session
                            if (port == 2 || port == 3) && self.apu_state == ApuState::Ready {
                                log(LogCategory::Bus, LogLevel::Debug, || {
                                    format!(
                                        "SNES Bus: APU new session detected (port {} write)",
                                        port
                                    )
                                });
                                self.apu_session_id = self.apu_session_id.wrapping_add(1);
                                self.apu_state = ApuState::Idle;
                                // Some games read back ports 2/3; mirror them on the output side.
                                self.apu_out_ports[port] = val;
                                return;
                            }

                            log(LogCategory::Bus, LogLevel::Debug, || {
                                format!(
                                    "SNES Bus: APU write handler - state: {:?}, port: {}, val: 0x{:02X}, current port 0: 0x{:02X}",
                                    self.apu_state, port, val, self.apu_out_ports[0]
                                )
                            });

                            match self.apu_state {
                                ApuState::Idle | ApuState::BootReady | ApuState::Ready => {
                                    // Check for upload command byte (port 0 with non-zero, non-AA value)
                                    // This typically starts a data upload sequence
                                    if port == 0 && val != 0x00 && val != 0xAA {
                                        log(LogCategory::Bus, LogLevel::Debug, || {
                                            format!(
                                            "SNES Bus: APU starting upload session {} with command 0x{:02X}",
                                            self.apu_session_id, val
                                        )
                                        });
                                        // Echo the command byte to acknowledge
                                        self.apu_out_ports[0] = val;
                                        self.apu_transfer_counter = 1;
                                        self.apu_state = ApuState::Uploading;
                                        self.apu_response_delay = 5;
                                    }
                                    // Port 0 write with 0x00 or 0xAA - acknowledge directly (helps early boot code)
                                    else if port == 0 {
                                        self.apu_out_ports[0] = val;
                                    } else {
                                        // Ports 2/3 are often treated as address/control; mirror them to output.
                                        // Do not mirror port 1 (data) to avoid corrupting 16-bit reads.
                                        if port == 2 || port == 3 {
                                            self.apu_out_ports[port] = val;
                                        }
                                    }
                                }

                                ApuState::Uploading => {
                                    // In upload mode, games typically:
                                    // - write an index/counter to port 0
                                    // - write a data byte to port 1
                                    // - poll port 0 until it matches the index
                                    //
                                    // Model that by acknowledging the last port-0 value *after* a port-1 write.
                                    if port == 0 {
                                        // Many commercial boot loaders poll $2140 immediately after writing it.
                                        // Acknowledge port-0 writes right away to avoid deadlocks.
                                        self.apu_out_ports[0] = val;
                                        self.apu_response_delay = 1;
                                    } else if port == 1 {
                                        self.apu_transfer_counter =
                                            self.apu_transfer_counter.wrapping_add(1);

                                        // Hot path optimization: Remove trace logging from APU upload loop
                                        // This is called for every byte uploaded to APU (potentially thousands)

                                        // Acknowledge index/counter
                                        self.apu_out_ports[0] = self.apu_in_ports[0];
                                        self.apu_response_delay = 5;
                                    } else {
                                        // Mirror ports 2/3 on output side for robustness.
                                        if port == 2 || port == 3 {
                                            self.apu_out_ports[port] = val;
                                        }
                                    }
                                }
                            }
                        } // End of stub else block
                    }
                    // $2100-$213F - PPU registers (excluding APU ports above)
                    // Note: $2140-$2143 are handled by the APU case above
                    0x2100 => {
                        log(LogCategory::PPU, LogLevel::Info, || {
                            format!(
                                "SNES Bus: [PC=${:06X}] write $2100 (INIDISP) = 0x{:02X}",
                                self.last_cpu_pc, val
                            )
                        });
                        self.ppu.write_register(offset, val)
                    }
                    0x2101..=0x213F => self.ppu.write_register(offset, val),
                    // $4200 - NMITIMEN - Interrupt Enable and Joypad Request
                    0x4200 => {
                        self.open_bus.set(val);
                        log(LogCategory::Interrupts, LogLevel::Trace, || {
                            format!(
                                "SNES Bus: [PC=${:06X}] write $4200 (NMITIMEN) = 0x{:02X}",
                                self.last_cpu_pc, val
                            )
                        });
                        // Bit 7: NMI enable
                        // Bits 5-4: H/V timer IRQ enable (00=off, 01=H, 10=V, 11=HV)
                        // Bit 0: Joypad auto-read enable
                        let old_nmi_enable = self.ppu.nmi_enable;
                        self.ppu.nmi_enable = (val & 0x80) != 0;

                        // H/V timer IRQ mode (bits 5-4)
                        let old_hv_irq_mode = self.hv_irq_mode;
                        self.hv_irq_mode = (val >> 4) & 0x03;
                        if old_hv_irq_mode != self.hv_irq_mode {
                            log(LogCategory::Interrupts, LogLevel::Debug, || {
                                let mode_str = match self.hv_irq_mode {
                                    0 => "disabled",
                                    1 => "H-timer only",
                                    2 => "V-timer only",
                                    3 => "H+V timer",
                                    _ => "unknown",
                                };
                                format!("SNES Bus: H/V timer IRQ mode = {}", mode_str)
                            });
                        }

                        if old_nmi_enable != self.ppu.nmi_enable {
                            log(LogCategory::Interrupts, LogLevel::Debug, || {
                                format!(
                                    "SNES Bus: NMI {}",
                                    if self.ppu.nmi_enable {
                                        "enabled"
                                    } else {
                                        "disabled"
                                    }
                                )
                            });
                        }

                        // Bit 0: Auto-joypad read enable
                        let old_auto_joypad = self.auto_joypad_enable;
                        self.auto_joypad_enable = (val & 0x01) != 0;
                        if old_auto_joypad != self.auto_joypad_enable {
                            log(LogCategory::Bus, LogLevel::Debug, || {
                                format!(
                                    "SNES Bus: Auto-joypad read {}",
                                    if self.auto_joypad_enable {
                                        "enabled"
                                    } else {
                                        "disabled"
                                    }
                                )
                            });
                        }
                    }
                    // $4201 - WRIO - Programmable I/O port
                    0x4201 => {
                        self.open_bus.set(val);
                        self.wrio = val;
                    }
                    // $4202 - WRMPYA - Multiplicand A
                    0x4202 => {
                        self.open_bus.set(val);
                        self.wrmpya = val;
                    }
                    // $4203 - WRMPYB - Multiplicand B (write triggers multiplication)
                    0x4203 => {
                        self.open_bus.set(val);
                        self.wrmpyb = val;
                        self.math_4216 = (self.wrmpya as u16).wrapping_mul(self.wrmpyb as u16);
                    }
                    // $4204-$4205 - WRDIVL/WRDIVH - Dividend
                    0x4204 => {
                        self.open_bus.set(val);
                        self.wrdiv = (self.wrdiv & 0xFF00) | (val as u16);
                    }
                    0x4205 => {
                        self.open_bus.set(val);
                        self.wrdiv = (self.wrdiv & 0x00FF) | ((val as u16) << 8);
                    }
                    // $4206 - WRDIVB - Divisor (write triggers division)
                    0x4206 => {
                        self.open_bus.set(val);
                        self.wrdivb = val;

                        if self.wrdivb == 0 {
                            self.div_quotient = 0xFFFF;
                            self.math_4216 = self.wrdiv;
                        } else {
                            let divisor = self.wrdivb as u16;
                            self.div_quotient = self.wrdiv / divisor;
                            self.math_4216 = self.wrdiv % divisor;
                        }
                    }
                    // $4207-$420A - H/V timer registers
                    0x4207 => {
                        self.open_bus.set(val);
                        self.htime = (self.htime & 0xFF00) | (val as u16);
                    }
                    0x4208 => {
                        self.open_bus.set(val);
                        self.htime = (self.htime & 0x00FF) | ((val as u16) << 8);
                    }
                    0x4209 => {
                        self.open_bus.set(val);
                        self.vtime = (self.vtime & 0xFF00) | (val as u16);
                    }
                    0x420A => {
                        self.open_bus.set(val);
                        self.vtime = (self.vtime & 0x00FF) | ((val as u16) << 8);
                    }
                    // $420D - MEMSEL
                    0x420D => {
                        self.open_bus.set(val);
                        self.memsel = val;
                    }
                    // $4016 - JOYWR - Controller Strobe
                    0x4016 => {
                        self.open_bus.set(val);
                        // Bit 0: Controller strobe (1 = latch, 0 = shift)
                        let old_strobe = self.controller_strobe;
                        self.controller_strobe = (val & 1) != 0;

                        // On falling edge (1 -> 0), latch the controller state
                        if old_strobe && !self.controller_strobe {
                            log(LogCategory::Bus, LogLevel::Trace, || {
                                format!(
                                    "SNES Bus: Controller latch - P1: 0x{:04X}, P2: 0x{:04X}",
                                    self.controller_state[0], self.controller_state[1]
                                )
                            });
                            self.controller_shift[0].set(self.controller_state[0]);
                            self.controller_shift[1].set(self.controller_state[1]);
                        }
                    }
                    // $420B - MDMAEN - DMA Enable
                    0x420B => {
                        self.open_bus.set(val);
                        // Each bit enables a DMA channel
                        if val != 0 {
                            log(LogCategory::Bus, LogLevel::Info, || {
                                format!("SNES Bus: Starting DMA on channels 0b{:08b}", val)
                            });
                            // Execute DMA transfer and set pending cycles to halt the CPU
                            let cycles = self.do_dma(val);
                            self.pending_dma_cycles.set(cycles);
                            log(LogCategory::Bus, LogLevel::Debug, || {
                                format!("SNES Bus: DMA halting CPU for {} cycles", cycles)
                            });
                        }
                    }
                    // $420C - HDMAEN - HDMA Enable
                    0x420C => {
                        self.open_bus.set(val);
                        self.hdma_enable = val;
                        if val != 0 {
                            log(LogCategory::Bus, LogLevel::Info, || {
                                format!("SNES Bus: HDMA enabled for channels 0b{:08b}", val)
                            });
                        }
                    }
                    // $43x0-$43xA - DMA channel registers (write)
                    0x4300..=0x437F => {
                        let ch = ((offset - 0x4300) >> 4) as usize & 7;
                        let reg = (offset & 0x0F) as usize;
                        match reg {
                            0x0 => self.dma_channels[ch].control = val,
                            0x1 => self.dma_channels[ch].b_addr = val,
                            0x2 => {
                                self.dma_channels[ch].a_addr =
                                    (self.dma_channels[ch].a_addr & 0xFFFF00) | (val as u32);
                            }
                            0x3 => {
                                self.dma_channels[ch].a_addr =
                                    (self.dma_channels[ch].a_addr & 0xFF00FF) | ((val as u32) << 8);
                            }
                            0x4 => {
                                self.dma_channels[ch].a_addr = (self.dma_channels[ch].a_addr
                                    & 0x00FFFF)
                                    | ((val as u32) << 16);
                            }
                            0x5 => {
                                self.dma_channels[ch].size =
                                    (self.dma_channels[ch].size & 0xFF00) | (val as u16);
                            }
                            0x6 => {
                                self.dma_channels[ch].size =
                                    (self.dma_channels[ch].size & 0x00FF) | ((val as u16) << 8);
                            }
                            0x7 => self.dma_channels[ch].hdma_bank = val,
                            0x8 => {
                                self.dma_channels[ch].hdma_table =
                                    (self.dma_channels[ch].hdma_table & 0xFF00) | (val as u16);
                            }
                            0x9 => {
                                self.dma_channels[ch].hdma_table =
                                    (self.dma_channels[ch].hdma_table & 0x00FF)
                                        | ((val as u16) << 8);
                            }
                            0xA => self.dma_channels[ch].hdma_line = val,
                            _ => {} // Unused registers, ignore writes
                        }
                    }
                    // Other hardware registers
                    0x2000..=0x5FFF => {} // Stub - ignore writes
                    // $6000-$7FFF: Expansion / cartridge-mapped region (SRAM on HiROM)
                    0x6000..=0x7FFF => {
                        if let Some(ref mut cart) = self.cartridge {
                            cart.write(addr, val);
                        }
                    }
                    // Cartridge ROM/RAM
                    0x8000..=0xFFFF => {
                        if let Some(ref mut cart) = self.cartridge {
                            cart.write(addr, val);
                        }
                    }
                }
            }
            // Banks $7E-$7F: Full WRAM mirror
            0x7E..=0x7F => {
                let wram_addr = ((bank as usize - 0x7E) << 16) | offset as usize;
                self.wram[wram_addr] = val;
            }
            // Banks $40-$6F and $C0-$FF: Cartridge ROM/RAM
            _ => {
                if let Some(ref mut cart) = self.cartridge {
                    cart.write(addr, val);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_controller_registers() {
        let mut bus = SnesBus::new();

        // Set controller 1 state: B button (bit 15)
        bus.set_controller(0, 0x8000);

        // Read auto-joypad registers
        let joy1l = bus.read(0x4218);
        let joy1h = bus.read(0x4219);
        assert_eq!(joy1l, 0x00);
        assert_eq!(joy1h, 0x80); // B button
    }

    #[test]
    fn test_controller_serial_read() {
        let mut bus = SnesBus::new();

        // Set controller state: A button (bit 7)
        bus.set_controller(0, 0x0080);

        // Latch state
        bus.write(0x4016, 1);
        bus.write(0x4016, 0);

        // Read bits serially (SNES sends LSB first)
        let mut bits_read = 0u16;
        for i in 0..16 {
            let bit = bus.read(0x4016) & 1;
            bits_read |= (bit as u16) << i;
        }

        assert_eq!(bits_read, 0x0080); // Should match the A button state
    }

    #[test]
    fn test_controller_strobe() {
        let mut bus = SnesBus::new();

        // Set controller state
        bus.set_controller(0, 0x1234);

        // Strobe on - should read current bit 0
        bus.write(0x4016, 1);
        let bit_strobed = bus.read(0x4016) & 1;
        assert_eq!(bit_strobed, 0); // bit 0 of 0x1234 is 0

        // Strobe off - latch and shift
        bus.write(0x4016, 0);

        // Read first bit
        let bit0 = bus.read(0x4016) & 1;
        assert_eq!(bit0, 0); // LSB of 0x1234
    }

    #[test]
    fn test_dual_controllers() {
        let mut bus = SnesBus::new();

        // Set different states for both controllers
        bus.set_controller(0, 0xAAAA);
        bus.set_controller(1, 0x5555);

        // Read auto-joypad registers
        assert_eq!(bus.read(0x4218), 0xAA); // JOY1L
        assert_eq!(bus.read(0x4219), 0xAA); // JOY1H
        assert_eq!(bus.read(0x421A), 0x55); // JOY2L
        assert_eq!(bus.read(0x421B), 0x55); // JOY2H

        // Latch both controllers
        bus.write(0x4016, 1);
        bus.write(0x4016, 0);

        // Read first bits from both controllers
        let bit1_0 = bus.read(0x4016) & 1;
        let bit2_0 = bus.read(0x4017) & 1;

        assert_eq!(bit1_0, 0); // LSB of 0xAAAA
        assert_eq!(bit2_0, 1); // LSB of 0x5555
    }

    #[test]
    fn test_dma_registers() {
        let mut bus = SnesBus::new();

        // Write to DMA channel 0 registers
        bus.write(0x4300, 0x01); // Control
        bus.write(0x4301, 0x18); // B-bus address ($2118 = VRAM data low)
        bus.write(0x4302, 0x00); // A-bus address low
        bus.write(0x4303, 0x80); // A-bus address mid
        bus.write(0x4304, 0x7E); // A-bus address high (bank)
        bus.write(0x4305, 0x00); // Size low (256 bytes)
        bus.write(0x4306, 0x01); // Size high

        // Read back registers
        assert_eq!(bus.read(0x4300), 0x01);
        assert_eq!(bus.read(0x4301), 0x18);
        assert_eq!(bus.read(0x4302), 0x00);
        assert_eq!(bus.read(0x4303), 0x80);
        assert_eq!(bus.read(0x4304), 0x7E);
        assert_eq!(bus.read(0x4305), 0x00);
        assert_eq!(bus.read(0x4306), 0x01);
    }

    #[test]
    #[ignore] // TODO: Same issue as upload protocol test - SPC700 not echoing indices
    fn test_apu_ports_echo() {
        let mut bus = SnesBus::new();

        // This test targets the real SPC700 + IPL ROM behavior.
        bus.enable_spc700();

        // With real SPC700, we need to wait for it to boot and write $BBAA signature
        // With proper clock ratio (SPC700 ~28.6% of main CPU), we need more cycles
        bus.tick_cycles(15000);

        // Verify SPC700 is ready (wrote $BBAA)
        assert_eq!(
            bus.read(0x2140),
            0xAA,
            "SPC700 should signal ready with $AA"
        );
        assert_eq!(
            bus.read(0x2141),
            0xBB,
            "SPC700 should signal ready with $BB"
        );

        // Send start command with entry point
        bus.write(0x2142, 0x00); // Entry point low byte
        bus.write(0x2143, 0x02); // Entry point high byte
        bus.write(0x2141, 0x01); // Non-zero (upload mode)
        bus.write(0x2140, 0xCC); // Start signal

        bus.tick_cycles(500);

        // SPC700 should echo $CC back
        assert_eq!(bus.read(0x2140), 0xCC, "SPC700 should acknowledge with $CC");

        // Now upload a byte (index 1, data $DE)
        // Note: IPL ROM waits for NON-ZERO index at $FFD6-$FFD8
        bus.write(0x2141, 0xDE); // Data
        bus.write(0x2140, 0x01); // Index 1 (not 0!)

        bus.tick_cycles(500);

        // SPC700 should echo index 1
        assert_eq!(bus.read(0x2140), 0x01, "SPC700 should echo index 1");
    }

    #[test]
    fn test_apu_ports_initial_values() {
        let mut bus = SnesBus::new();

        // This test verifies the real SPC700 IPL signature behavior.
        bus.enable_spc700();

        // With real SPC700, we need to run it for enough cycles to complete boot
        // and write the $BBAA ready signature
        // The IPL ROM clears memory first, then writes ports
        // With proper clock ratio (SPC700 ~28.6% of main CPU), we need more cycles
        bus.tick_cycles(15000);

        // APU ports should now have ready values from SPC700 IPL ROM
        // SPC700 IPL sets ports to $BBAA when read as 16-bit little-endian value
        // This means: $2140 (port 0) = 0xAA (low byte), $2141 (port 1) = 0xBB (high byte)
        assert_eq!(bus.read(0x2140), 0xAA, "Port 0 should be $AA after boot");
        assert_eq!(bus.read(0x2141), 0xBB, "Port 1 should be $BB after boot");
        assert_eq!(bus.read(0x2142), 0x00, "Port 2 should be $00");
        assert_eq!(bus.read(0x2143), 0x00, "Port 3 should be $00");
    }

    #[test]
    fn test_dma_transfer_simple() {
        let mut bus = SnesBus::new();

        // Set up WRAM with test data
        for i in 0..16 {
            bus.wram[i] = (i as u8) * 0x11;
        }

        // Configure DMA channel 0: WRAM -> VRAM
        bus.write(0x4300, 0x01); // Mode 1: 2 registers write once
        bus.write(0x4301, 0x18); // B-bus: $2118 (VMDATAL)
        bus.write(0x4302, 0x00); // A-bus: $7E0000 (WRAM start)
        bus.write(0x4303, 0x00);
        bus.write(0x4304, 0x7E);
        bus.write(0x4305, 0x10); // Size: 16 bytes
        bus.write(0x4306, 0x00);

        // Trigger DMA
        bus.write(0x420B, 0x01); // Enable channel 0

        // Verify data was transferred to VRAM (through PPU)
        // The DMA should have written to VMDATAL, which updates VRAM
        // Note: This is a basic test - actual VRAM verification would require
        // checking the PPU's internal state
    }

    #[test]
    fn test_dma_multiple_channels() {
        let mut bus = SnesBus::new();

        // Configure two channels
        bus.write(0x4300, 0x00); // Channel 0: mode 0
        bus.write(0x4301, 0x18); // B-bus: VRAM
        bus.write(0x4302, 0x00); // A-bus: $7E0000
        bus.write(0x4303, 0x00);
        bus.write(0x4304, 0x7E);
        bus.write(0x4305, 0x08); // 8 bytes
        bus.write(0x4306, 0x00);

        bus.write(0x4310, 0x00); // Channel 1: mode 0
        bus.write(0x4311, 0x22); // B-bus: CGRAM
        bus.write(0x4312, 0x10); // A-bus: $7E0010
        bus.write(0x4313, 0x00);
        bus.write(0x4314, 0x7E);
        bus.write(0x4315, 0x08); // 8 bytes
        bus.write(0x4316, 0x00);

        // Trigger both channels
        bus.write(0x420B, 0x03); // Enable channels 0 and 1

        // Both channels should complete
    }

    #[test]
    fn test_hdma_enable_register() {
        let mut bus = SnesBus::new();

        // Write to HDMA enable register
        bus.write(0x420C, 0x05); // Enable channels 0 and 2

        // Read back
        assert_eq!(bus.read(0x420C), 0x05);
        assert_eq!(bus.hdma_enable, 0x05);
    }

    #[test]
    fn test_hdma_initialization() {
        let mut bus = SnesBus::new();

        // Configure HDMA channel 0
        bus.write(0x4300, 0x00); // Mode 0, direct
        bus.write(0x4301, 0x00); // B-bus: $2100 (INIDISP - brightness)
        bus.write(0x4307, 0x7E); // HDMA bank
        bus.write(0x4308, 0x00); // HDMA table address low
        bus.write(0x4309, 0x10); // HDMA table address high ($7E1000)

        // Set up a simple HDMA table in WRAM
        // Format: [line_count, data, line_count, data, ..., 0]
        bus.wram[0x1000] = 0x01; // 1 scanline
        bus.wram[0x1001] = 0x0F; // Full brightness
        bus.wram[0x1002] = 0x00; // Terminate

        // Enable HDMA
        bus.write(0x420C, 0x01); // Enable channel 0

        // Initialize HDMA
        bus.init_hdma();

        // Verify state was initialized
        assert!(bus.hdma_state[0].active);
        assert_eq!(bus.hdma_state[0].table_addr, 0x7E1000);
    }

    #[test]
    fn test_hdma_execution_simple() {
        let mut bus = SnesBus::new();

        // Configure HDMA channel 0 for brightness control
        bus.write(0x4300, 0x00); // Mode 0: 1 byte transfer, direct
        bus.write(0x4301, 0x00); // B-bus: $2100 (INIDISP)
        bus.write(0x4307, 0x7E); // HDMA bank
        bus.write(0x4308, 0x00); // HDMA table low
        bus.write(0x4309, 0x20); // HDMA table high ($7E2000)

        // Set up HDMA table: 2 scanlines of 0x0F brightness, then terminate
        bus.wram[0x2000] = 0x02; // 2 scanlines
        bus.wram[0x2001] = 0x0F; // Brightness value
        bus.wram[0x2002] = 0x00; // Terminate

        // Enable and initialize HDMA
        bus.write(0x420C, 0x01);
        bus.init_hdma();

        // Execute HDMA for first scanline
        let _cycles = bus.do_hdma();

        // Verify the HDMA executed (line counter should be decremented)
        assert_eq!(bus.hdma_state[0].line_counter, 1);

        // Execute HDMA for second scanline
        let _cycles = bus.do_hdma();

        // Line counter should be 0, ready to fetch next entry
        assert_eq!(bus.hdma_state[0].line_counter, 0);

        // Execute HDMA again - should terminate
        let _cycles = bus.do_hdma();

        // Channel should be inactive now
        assert!(!bus.hdma_state[0].active);
    }

    #[test]
    fn test_hdma_repeat_mode() {
        let mut bus = SnesBus::new();

        // Configure HDMA
        bus.write(0x4300, 0x00); // Mode 0
        bus.write(0x4301, 0x00); // $2100
        bus.write(0x4307, 0x7E);
        bus.write(0x4308, 0x00);
        bus.write(0x4309, 0x30);

        // HDMA table with repeat flag set (0x80 | line_count)
        bus.wram[0x3000] = 0x83; // Repeat for 3 scanlines
        bus.wram[0x3001] = 0x07; // Value
        bus.wram[0x3002] = 0x00; // Terminate

        bus.write(0x420C, 0x01);
        bus.init_hdma();

        // Execute HDMA 3 times
        for _ in 0..3 {
            let _cycles = bus.do_hdma();
        }

        // Should still be at same table position (repeat mode)
        // and line counter should be 0
        assert_eq!(bus.hdma_state[0].line_counter, 0);
    }

    #[test]
    fn test_dma_mode_2_write_twice() {
        let mut bus = SnesBus::new();

        // Set up WRAM with test data
        bus.wram[0] = 0xAA;
        bus.wram[1] = 0xBB;

        // Configure DMA channel 0: Mode 2 (2 bytes to 1 register, write twice)
        bus.write(0x4300, 0x02); // Mode 2
        bus.write(0x4301, 0x18); // B-bus: $2118 (VMDATAL)
        bus.write(0x4302, 0x00); // A-bus: $7E0000 (WRAM start)
        bus.write(0x4303, 0x00);
        bus.write(0x4304, 0x7E);
        bus.write(0x4305, 0x02); // Size: 2 bytes
        bus.write(0x4306, 0x00);

        // Trigger DMA
        let cycles = bus.do_dma(0x01);

        // Verify timing: 8 cycles overhead + 2 bytes * 8 cycles = 24 cycles
        assert_eq!(cycles, 24, "Mode 2 should transfer 2 bytes");
    }

    #[test]
    fn test_dma_mode_4_four_registers() {
        let mut bus = SnesBus::new();

        // Set up WRAM with test data
        for i in 0..8 {
            bus.wram[i] = i as u8;
        }

        // Configure DMA channel 0: Mode 4 (4 bytes to 4 registers)
        bus.write(0x4300, 0x04); // Mode 4
        bus.write(0x4301, 0x18); // B-bus: $2118-$211B
        bus.write(0x4302, 0x00); // A-bus: $7E0000 (WRAM start)
        bus.write(0x4303, 0x00);
        bus.write(0x4304, 0x7E);
        bus.write(0x4305, 0x08); // Size: 8 bytes (2 complete cycles)
        bus.write(0x4306, 0x00);

        // Trigger DMA
        let cycles = bus.do_dma(0x01);

        // Verify timing: 8 cycles overhead + 8 bytes * 8 cycles = 72 cycles
        assert_eq!(cycles, 72, "Mode 4 should transfer 8 bytes to 4 registers");
    }

    #[test]
    fn test_hdma_mode_2() {
        let mut bus = SnesBus::new();

        // Configure HDMA: Mode 2 (2 bytes to 1 register)
        bus.write(0x4300, 0x02); // Mode 2, direct
        bus.write(0x4301, 0x18); // B-bus: $2118
        bus.write(0x4307, 0x7E); // HDMA bank
        bus.write(0x4308, 0x00); // HDMA table address
        bus.write(0x4309, 0x30);

        // HDMA table: 1 scanline, 2 bytes
        bus.wram[0x3000] = 0x01; // 1 scanline
        bus.wram[0x3001] = 0xAA; // Byte 1
        bus.wram[0x3002] = 0xBB; // Byte 2
        bus.wram[0x3003] = 0x00; // Terminate

        bus.write(0x420C, 0x01); // Enable channel 0
        bus.init_hdma();

        // Execute HDMA
        let cycles = bus.do_hdma();

        // Verify timing: 2 bytes * 8 cycles = 16 cycles
        assert_eq!(cycles, 16, "Mode 2 HDMA should transfer 2 bytes");
    }

    #[test]
    fn test_hdma_mode_4() {
        let mut bus = SnesBus::new();

        // Configure HDMA: Mode 4 (4 bytes to 4 registers)
        bus.write(0x4300, 0x04); // Mode 4, direct
        bus.write(0x4301, 0x18); // B-bus: $2118-$211B
        bus.write(0x4307, 0x7E); // HDMA bank
        bus.write(0x4308, 0x00); // HDMA table address
        bus.write(0x4309, 0x30);

        // HDMA table: 1 scanline, 4 bytes
        bus.wram[0x3000] = 0x01; // 1 scanline
        bus.wram[0x3001] = 0xAA; // Byte 1 -> $2118
        bus.wram[0x3002] = 0xBB; // Byte 2 -> $2119
        bus.wram[0x3003] = 0xCC; // Byte 3 -> $211A
        bus.wram[0x3004] = 0xDD; // Byte 4 -> $211B
        bus.wram[0x3005] = 0x00; // Terminate

        bus.write(0x420C, 0x01); // Enable channel 0
        bus.init_hdma();

        // Execute HDMA
        let cycles = bus.do_hdma();

        // Verify timing: 4 bytes * 8 cycles = 32 cycles
        assert_eq!(
            cycles, 32,
            "Mode 4 HDMA should transfer 4 bytes to 4 registers"
        );
    }

    #[test]
    fn test_hardware_multiply() {
        let mut bus = SnesBus::new();

        // Test basic multiplication: 10 * 20 = 200
        bus.write(0x4202, 10); // WRMPYA
        bus.write(0x4203, 20); // WRMPYB (triggers multiplication)

        // Read result from $4216-$4217
        let result_low = bus.read(0x4216);
        let result_high = bus.read(0x4217);
        let result = (result_high as u16) << 8 | result_low as u16;
        assert_eq!(result, 200, "10 * 20 should equal 200");

        // Test maximum values: 255 * 255 = 65025
        bus.write(0x4202, 255);
        bus.write(0x4203, 255);
        let result_low = bus.read(0x4216);
        let result_high = bus.read(0x4217);
        let result = (result_high as u16) << 8 | result_low as u16;
        assert_eq!(result, 65025, "255 * 255 should equal 65025");

        // Test zero multiplication
        bus.write(0x4202, 100);
        bus.write(0x4203, 0);
        let result_low = bus.read(0x4216);
        let result_high = bus.read(0x4217);
        let result = (result_high as u16) << 8 | result_low as u16;
        assert_eq!(result, 0, "100 * 0 should equal 0");
    }

    #[test]
    fn test_hardware_divide() {
        let mut bus = SnesBus::new();

        // Test basic division: 100 / 5 = 20 remainder 0
        bus.write(0x4204, 100); // WRDIVL
        bus.write(0x4205, 0); // WRDIVH
        bus.write(0x4206, 5); // WRDIVB (triggers division)

        // Read quotient from $4214-$4215
        let quotient_low = bus.read(0x4214);
        let quotient_high = bus.read(0x4215);
        let quotient = (quotient_high as u16) << 8 | quotient_low as u16;
        assert_eq!(quotient, 20, "100 / 5 should equal 20");

        // Read remainder from $4216-$4217
        let remainder_low = bus.read(0x4216);
        let remainder_high = bus.read(0x4217);
        let remainder = (remainder_high as u16) << 8 | remainder_low as u16;
        assert_eq!(remainder, 0, "100 % 5 should equal 0");

        // Test division with remainder: 107 / 10 = 10 remainder 7
        bus.write(0x4204, 107);
        bus.write(0x4205, 0);
        bus.write(0x4206, 10);
        let quotient_low = bus.read(0x4214);
        let quotient_high = bus.read(0x4215);
        let quotient = (quotient_high as u16) << 8 | quotient_low as u16;
        let remainder_low = bus.read(0x4216);
        let remainder_high = bus.read(0x4217);
        let remainder = (remainder_high as u16) << 8 | remainder_low as u16;
        assert_eq!(quotient, 10, "107 / 10 should equal 10");
        assert_eq!(remainder, 7, "107 % 10 should equal 7");

        // Test division by zero: should return 0xFFFF quotient and dividend as remainder
        bus.write(0x4204, 123);
        bus.write(0x4205, 0);
        bus.write(0x4206, 0); // Divide by zero
        let quotient_low = bus.read(0x4214);
        let quotient_high = bus.read(0x4215);
        let quotient = (quotient_high as u16) << 8 | quotient_low as u16;
        let remainder_low = bus.read(0x4216);
        let remainder_high = bus.read(0x4217);
        let remainder = (remainder_high as u16) << 8 | remainder_low as u16;
        assert_eq!(quotient, 0xFFFF, "Division by zero should return 0xFFFF");
        assert_eq!(
            remainder, 123,
            "Division by zero should return dividend as remainder"
        );

        // Test 16-bit dividend: 50000 / 100 = 500
        bus.write(0x4204, 0x50); // Low byte (50000 = 0xC350)
        bus.write(0x4205, 0xC3); // High byte
        bus.write(0x4206, 100);
        let quotient_low = bus.read(0x4214);
        let quotient_high = bus.read(0x4215);
        let quotient = (quotient_high as u16) << 8 | quotient_low as u16;
        assert_eq!(quotient, 500, "50000 / 100 should equal 500");
    }
}
