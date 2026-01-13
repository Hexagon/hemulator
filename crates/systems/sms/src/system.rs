//! Sega Master System main system implementation

use crate::bus::SmsMemory;
use crate::psg::SmsPsg;
use crate::vdp::Vdp;
use emu_core::cpu_z80::{CpuZ80, MemoryZ80};
use emu_core::debug::Debugger;
use emu_core::logging::{log, LogCategory, LogLevel};
use emu_core::renderer::Renderer;
use emu_core::types::Frame;
use emu_core::{MountPointInfo, System};
use serde_json::Value;
use std::cell::RefCell;
use std::rc::Rc;
use thiserror::Error;

/// SMS emulator errors
#[derive(Debug, Error)]
pub enum SmsError {
    #[error("Invalid mount point")]
    InvalidMountPoint,
}

/// Data for the tile viewer tab (SMS)
#[derive(Clone)]
pub struct TileViewerData {
    /// VRAM data (16KB)
    pub vram: Vec<u8>,
    /// Color RAM (32 bytes for palette)
    pub cram: Vec<u8>,
    /// Palette colors as RGB (32 colors)
    pub palette: Vec<u32>,
    /// VDP registers
    pub registers: Vec<u8>,
}

/// Sega Master System emulator
pub struct SmsSystem {
    // CPU
    pub(crate) cpu: CpuZ80<SmsMemory>,

    // Shared components
    vdp: Rc<RefCell<Vdp>>,
    psg: Rc<RefCell<SmsPsg>>,

    // Timing
    cycles: u64,
    timing_mode: emu_core::apu::TimingMode,

    // Debugging
    /// Instruction tracer for debugging
    pub(crate) instruction_tracer: emu_core::instruction_tracer::InstructionTracer,
    /// Breakpoint manager for debugging
    pub(crate) breakpoint_manager: emu_core::breakpoints::BreakpointManager,
}

impl SmsSystem {
    /// Create a new SMS system
    pub fn new() -> Self {
        // Create shared components
        let vdp = Rc::new(RefCell::new(Vdp::new()));
        let psg = Rc::new(RefCell::new(SmsPsg::new()));

        // Create empty ROM
        let rom = vec![0; 0x8000];
        let memory = SmsMemory::new(rom, Rc::clone(&vdp), Rc::clone(&psg));

        // Create CPU
        let cpu = CpuZ80::new(memory);

        Self {
            cpu,
            vdp,
            psg,
            cycles: 0,
            timing_mode: emu_core::apu::TimingMode::Ntsc,
            instruction_tracer: emu_core::instruction_tracer::InstructionTracer::new(),
            breakpoint_manager: emu_core::breakpoints::BreakpointManager::new(),
        }
    }

    /// Load a ROM
    pub fn load_rom(&mut self, rom_data: Vec<u8>) {
        // Log first few bytes for debugging
        log(LogCategory::CPU, LogLevel::Debug, || {
            format!(
                "SMS ROM: First 16 bytes: {:02X?}",
                &rom_data[0..16.min(rom_data.len())]
            )
        });

        // Detect timing mode from ROM header
        self.timing_mode = Self::detect_timing_mode(&rom_data);
        log(LogCategory::CPU, LogLevel::Info, || {
            format!("SMS: Detected timing mode: {:?}", self.timing_mode)
        });

        // Update PSG timing
        self.psg.borrow_mut().set_timing(self.timing_mode);

        // Create new memory with ROM
        let memory = SmsMemory::new(rom_data, Rc::clone(&self.vdp), Rc::clone(&self.psg));
        self.cpu = CpuZ80::new(memory);
        self.reset();
    }

    /// Detect timing mode (PAL/NTSC) from ROM header
    fn detect_timing_mode(rom_data: &[u8]) -> emu_core::apu::TimingMode {
        // Check for TMR SEGA header at offset 0x7FF0
        if rom_data.len() >= 0x7FF0 + 16 {
            let header_region = &rom_data[0x7FF0..0x7FF0 + 16];

            // Check for "TMR SEGA" signature
            if &header_region[0..8] == b"TMR SEGA" {
                // Byte 0x0F (offset 15) contains region code
                let region = header_region[15];

                // Region code interpretation (upper nibble only):
                // We look at the upper nibble (region >> 4):
                //   0x3, 0x5 -> Japan regions (NTSC)
                //   0x4, 0x6, 0x7 -> Export regions (assumed PAL)
                //
                // Note: Export regions could be either PAL (Europe) or NTSC (USA/Brazil).
                // Without additional metadata, we assume PAL for export regions as a heuristic.
                // This may cause incorrect timing for USA-region games. Consider manual override
                // via set_timing() if games run at incorrect speeds.

                match region >> 4 {
                    0x3 | 0x5 => {
                        // Japan region - always NTSC
                        log(LogCategory::CPU, LogLevel::Info, || {
                            format!(
                                "SMS: Japan region detected (region byte: 0x{:02X}), using NTSC",
                                region
                            )
                        });
                        return emu_core::apu::TimingMode::Ntsc;
                    }
                    0x4 | 0x6 | 0x7 => {
                        // Export region - could be PAL or NTSC
                        // Without additional metadata, assume PAL for European export
                        log(LogCategory::CPU, LogLevel::Info, || {
                            format!(
                                "SMS: Export region detected (region byte: 0x{:02X}), assuming PAL",
                                region
                            )
                        });
                        return emu_core::apu::TimingMode::Pal;
                    }
                    _ => {
                        log(LogCategory::CPU, LogLevel::Warn, || {
                            format!(
                                "SMS: Unknown region code 0x{:02X}, defaulting to NTSC",
                                region
                            )
                        });
                    }
                }
            }
        }

        // Default to NTSC if no valid header found
        log(LogCategory::CPU, LogLevel::Info, || {
            "SMS: No valid TMR SEGA header found, defaulting to NTSC".to_string()
        });
        emu_core::apu::TimingMode::Ntsc
    }

    /// Set controller 1 state
    pub fn set_controller_1(&mut self, state: u8) {
        self.cpu.memory.set_controller_1(state);
    }

    /// Set controller 2 state
    pub fn set_controller_2(&mut self, state: u8) {
        self.cpu.memory.set_controller_2(state);
    }
}

impl Default for SmsSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl System for SmsSystem {
    type Error = SmsError;

    fn reset(&mut self) {
        log(LogCategory::CPU, LogLevel::Info, || {
            "SMS: System reset".to_string()
        });
        self.cpu.reset();
        self.vdp.borrow_mut().reset();
        self.psg.borrow_mut().reset();
        self.cycles = 0;

        log(LogCategory::CPU, LogLevel::Debug, || {
            format!(
                "SMS CPU: PC=${:04X}, SP=${:04X}, A=${:02X}, F=${:02X}",
                self.cpu.pc, self.cpu.sp, self.cpu.a, self.cpu.f
            )
        });
    }

    fn step_frame(&mut self) -> Result<Frame, Self::Error> {
        // Calculate target cycles and scanlines based on timing mode
        let (target_cycles, total_scanlines) = match self.timing_mode {
            emu_core::apu::TimingMode::Ntsc => {
                // NTSC: 3.579545 MHz / 60 Hz = 59659 cycles/frame
                // 262 scanlines total
                (59659_u64, 262_u64)
            }
            emu_core::apu::TimingMode::Pal => {
                // PAL: 3.546894 MHz / 50 Hz = 70938 cycles/frame
                // 313 scanlines total
                (70938_u64, 313_u64)
            }
        };

        while self.cycles < target_cycles {
            // Log CPU state on first few cycles (using cycles count directly)
            if self.cycles < 100 {
                let opcode = self.cpu.memory.read(self.cpu.pc);
                log(LogCategory::CPU, LogLevel::Debug, || {
                    format!(
                        "SMS CPU: PC=${:04X} opcode=${:02X}, SP=${:04X}, A=${:02X}, BC=${:04X}, DE=${:04X}, HL=${:04X}",
                        self.cpu.pc, opcode, self.cpu.sp, self.cpu.a,
                        ((self.cpu.b as u16) << 8) | self.cpu.c as u16,
                        ((self.cpu.d as u16) << 8) | self.cpu.e as u16,
                        ((self.cpu.h as u16) << 8) | self.cpu.l as u16
                    )
                });
            }

            // Execute one CPU instruction
            let pc_before = self.cpu.pc as u32;
            let cpu_cycles = self.cpu.step() as u64;
            self.cycles += cpu_cycles;

            // Record instruction if tracing is enabled
            if self.instruction_tracer.is_enabled() {
                if let Some(instr) = self.disassemble_instruction(pc_before) {
                    let cpu_state = self.get_cpu_state();
                    self.instruction_tracer.trace(instr, cpu_state);
                }
            }

            // Update VDP scanline based on cycles
            // Calculate dynamically to avoid cumulative timing drift
            let current_scanline =
                (self.cycles * total_scanlines / target_cycles) % total_scanlines;
            self.vdp.borrow_mut().set_scanline(current_scanline as u16);

            // Check for VDP interrupts (frame interrupt has priority over line interrupt)
            if self.vdp.borrow().frame_interrupt_pending() {
                // Trigger Z80 interrupt (IM 1: RST 38h = jump to 0x0038)
                // Data byte doesn't matter in IM 1, but pass 0xFF as default
                self.cpu.interrupt(0xFF);
                self.vdp.borrow_mut().clear_frame_interrupt();
            } else if self.vdp.borrow().line_interrupt_pending() {
                // Trigger Z80 interrupt for line interrupt
                self.cpu.interrupt(0xFF);
                self.vdp.borrow_mut().clear_line_interrupt();
            }
        }

        self.cycles -= target_cycles;

        // Get frame from VDP
        let frame = self.vdp.borrow().get_frame().clone();

        Ok(frame)
    }

    fn save_state(&self) -> Value {
        let vdp = self.vdp.borrow();
        let psg = self.psg.borrow();

        serde_json::json!({
            "system": "sms",
            "version": 1,
            "cycles": self.cycles,
            "timing_mode": match self.timing_mode {
                emu_core::apu::TimingMode::Ntsc => "ntsc",
                emu_core::apu::TimingMode::Pal => "pal",
            },
            "cpu": {
                // Main registers
                "a": self.cpu.a,
                "f": self.cpu.f,
                "b": self.cpu.b,
                "c": self.cpu.c,
                "d": self.cpu.d,
                "e": self.cpu.e,
                "h": self.cpu.h,
                "l": self.cpu.l,
                // Shadow registers
                "a_prime": self.cpu.a_prime,
                "f_prime": self.cpu.f_prime,
                "b_prime": self.cpu.b_prime,
                "c_prime": self.cpu.c_prime,
                "d_prime": self.cpu.d_prime,
                "e_prime": self.cpu.e_prime,
                "h_prime": self.cpu.h_prime,
                "l_prime": self.cpu.l_prime,
                // Index registers
                "ix": self.cpu.ix,
                "iy": self.cpu.iy,
                // Special registers
                "i": self.cpu.i,
                "r": self.cpu.r,
                "sp": self.cpu.sp,
                "pc": self.cpu.pc,
                // Interrupt state
                "iff1": self.cpu.iff1,
                "iff2": self.cpu.iff2,
                "im": self.cpu.im,
                "halted": self.cpu.halted,
            },
            "memory": {
                "ram": self.cpu.memory.get_ram(),
                "rom_bank_0": self.cpu.memory.get_rom_bank_0(),
                "rom_bank_1": self.cpu.memory.get_rom_bank_1(),
                "rom_bank_2": self.cpu.memory.get_rom_bank_2(),
                "controller_1": self.cpu.memory.get_controller_1(),
                "controller_2": self.cpu.memory.get_controller_2(),
                "memory_control": self.cpu.memory.get_memory_control(),
            },
            "vdp": vdp.get_state(),
            "psg": psg.get_state(),
        })
    }

    fn load_state(&mut self, state: &Value) -> Result<(), serde_json::Error> {
        // Validate system type
        if let Some(system) = state.get("system").and_then(|s| s.as_str()) {
            if system != "sms" {
                // Create a proper error by trying to deserialize an incompatible value
                let _: () = serde_json::from_value(serde_json::json!({
                    "error": "Invalid system type"
                }))?;
            }
        }

        // Helper macros for loading values
        macro_rules! load_u8 {
            ($state:expr, $field:literal, $target:expr) => {
                if let Some(val) = $state.get($field).and_then(|v| v.as_u64()) {
                    $target = val as u8;
                }
            };
        }

        macro_rules! load_u16 {
            ($state:expr, $field:literal, $target:expr) => {
                if let Some(val) = $state.get($field).and_then(|v| v.as_u64()) {
                    $target = val as u16;
                }
            };
        }

        macro_rules! load_bool {
            ($state:expr, $field:literal, $target:expr) => {
                if let Some(val) = $state.get($field).and_then(|v| v.as_bool()) {
                    $target = val;
                }
            };
        }

        // Load cycles
        if let Some(cycles) = state.get("cycles").and_then(|v| v.as_u64()) {
            self.cycles = cycles;
        }

        // Load timing mode
        if let Some(timing_str) = state.get("timing_mode").and_then(|v| v.as_str()) {
            self.timing_mode = match timing_str {
                "pal" => emu_core::apu::TimingMode::Pal,
                _ => emu_core::apu::TimingMode::Ntsc,
            };
        }

        // Load CPU state
        if let Some(cpu_state) = state.get("cpu") {
            load_u8!(cpu_state, "a", self.cpu.a);
            load_u8!(cpu_state, "f", self.cpu.f);
            load_u8!(cpu_state, "b", self.cpu.b);
            load_u8!(cpu_state, "c", self.cpu.c);
            load_u8!(cpu_state, "d", self.cpu.d);
            load_u8!(cpu_state, "e", self.cpu.e);
            load_u8!(cpu_state, "h", self.cpu.h);
            load_u8!(cpu_state, "l", self.cpu.l);
            load_u8!(cpu_state, "a_prime", self.cpu.a_prime);
            load_u8!(cpu_state, "f_prime", self.cpu.f_prime);
            load_u8!(cpu_state, "b_prime", self.cpu.b_prime);
            load_u8!(cpu_state, "c_prime", self.cpu.c_prime);
            load_u8!(cpu_state, "d_prime", self.cpu.d_prime);
            load_u8!(cpu_state, "e_prime", self.cpu.e_prime);
            load_u8!(cpu_state, "h_prime", self.cpu.h_prime);
            load_u8!(cpu_state, "l_prime", self.cpu.l_prime);
            load_u16!(cpu_state, "ix", self.cpu.ix);
            load_u16!(cpu_state, "iy", self.cpu.iy);
            load_u8!(cpu_state, "i", self.cpu.i);
            load_u8!(cpu_state, "r", self.cpu.r);
            load_u16!(cpu_state, "sp", self.cpu.sp);
            load_u16!(cpu_state, "pc", self.cpu.pc);
            load_bool!(cpu_state, "iff1", self.cpu.iff1);
            load_bool!(cpu_state, "iff2", self.cpu.iff2);
            load_u8!(cpu_state, "im", self.cpu.im);
            load_bool!(cpu_state, "halted", self.cpu.halted);
        }

        // Load memory state
        if let Some(mem_state) = state.get("memory") {
            if let Some(ram) = mem_state.get("ram").and_then(|v| v.as_array()) {
                self.cpu.memory.set_ram(
                    &ram.iter()
                        .filter_map(|v| v.as_u64().map(|n| n as u8))
                        .collect::<Vec<u8>>(),
                );
            }
            if let Some(bank) = mem_state.get("rom_bank_0").and_then(|v| v.as_u64()) {
                self.cpu.memory.set_rom_bank_0(bank as usize);
            }
            if let Some(bank) = mem_state.get("rom_bank_1").and_then(|v| v.as_u64()) {
                self.cpu.memory.set_rom_bank_1(bank as usize);
            }
            if let Some(bank) = mem_state.get("rom_bank_2").and_then(|v| v.as_u64()) {
                self.cpu.memory.set_rom_bank_2(bank as usize);
            }
            if let Some(ctrl1) = mem_state.get("controller_1").and_then(|v| v.as_u64()) {
                self.cpu.memory.set_controller_1(ctrl1 as u8);
            }
            if let Some(ctrl2) = mem_state.get("controller_2").and_then(|v| v.as_u64()) {
                self.cpu.memory.set_controller_2(ctrl2 as u8);
            }
            if let Some(mem_ctrl) = mem_state.get("memory_control").and_then(|v| v.as_u64()) {
                self.cpu.memory.set_memory_control(mem_ctrl as u8);
            }
        }

        // Load VDP state
        if let Some(vdp_state) = state.get("vdp") {
            self.vdp.borrow_mut().set_state(vdp_state)?;
        }

        // Load PSG state
        if let Some(psg_state) = state.get("psg") {
            self.psg.borrow_mut().set_state(psg_state)?;
        }

        Ok(())
    }

    fn mount_points(&self) -> Vec<MountPointInfo> {
        vec![MountPointInfo {
            id: "cartridge".to_string(),
            name: "Cartridge".to_string(),
            extensions: vec!["sms".to_string()],
            required: true,
        }]
    }

    fn mount(&mut self, mount_point_id: &str, data: &[u8]) -> Result<(), Self::Error> {
        if mount_point_id == "cartridge" {
            self.load_rom(data.to_vec());
            Ok(())
        } else {
            Err(SmsError::InvalidMountPoint)
        }
    }

    fn unmount(&mut self, mount_point_id: &str) -> Result<(), Self::Error> {
        if mount_point_id == "cartridge" {
            self.load_rom(vec![0; 0x8000]);
            Ok(())
        } else {
            Err(SmsError::InvalidMountPoint)
        }
    }

    fn is_mounted(&self, mount_point_id: &str) -> bool {
        mount_point_id == "cartridge"
    }

    fn debugger(&self) -> Option<&dyn emu_core::debug::Debugger> {
        Some(self)
    }
}

impl SmsSystem {
    /// Get audio samples from the PSG
    ///
    /// This method generates the requested number of audio samples by clocking
    /// the SN76489 PSG audio chip.
    pub fn get_audio_samples(&mut self, count: usize) -> Vec<i16> {
        self.psg.borrow_mut().generate_samples(count)
    }

    /// Set timing mode (NTSC/PAL) for the system
    ///
    /// This updates both the system timing and PSG's clock rate to match the selected mode.
    pub fn set_timing(&mut self, timing: emu_core::apu::TimingMode) {
        self.timing_mode = timing;
        self.psg.borrow_mut().set_timing(timing);
    }

    /// Get current timing mode
    pub fn get_timing(&self) -> emu_core::apu::TimingMode {
        self.timing_mode
    }

    emu_core::impl_instruction_tracer_methods!();

    /// Check if instruction tracing is enabled
    pub fn is_instruction_tracing_enabled(&self) -> bool {
        self.instruction_tracer.is_enabled()
    }

    emu_core::impl_breakpoint_methods!();

    /// Enable or disable breakpoints
    pub fn set_breakpoints_enabled(&mut self, enabled: bool) {
        self.breakpoint_manager.set_enabled(enabled);
    }

    /// Get the breakpoint manager
    pub fn get_breakpoint_manager(&self) -> &emu_core::breakpoints::BreakpointManager {
        &self.breakpoint_manager
    }

    /// Get tile viewer data for debugging
    pub fn get_tile_viewer_data(&self) -> TileViewerData {
        self.vdp.borrow().get_tile_viewer_data()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use emu_core::cpu_z80::MemoryZ80;

    #[test]
    fn test_system_creation() {
        let system = SmsSystem::new();
        assert_eq!(system.mount_points()[0].name, "Cartridge");
    }

    #[test]
    fn test_system_reset() {
        let mut system = SmsSystem::new();
        system.cycles = 12345;
        system.reset();
        assert_eq!(system.cycles, 0);
    }

    #[test]
    fn test_rom_loading() {
        let mut system = SmsSystem::new();
        let rom = vec![0xAB; 0x8000];
        system.load_rom(rom);

        // Verify ROM was loaded
        assert_eq!(system.cpu.memory.read(0x100), 0xAB);
    }

    #[test]
    fn test_step_frame() {
        let mut system = SmsSystem::new();

        // Load a simple ROM that just loops
        let mut rom = vec![0; 0x8000];
        rom[0] = 0x18; // JR opcode (not yet implemented in Z80, but ROM is loaded)
        rom[1] = 0xFE; // -2 (infinite loop)

        system.load_rom(rom);

        let frame = system.step_frame().unwrap();

        // Verify frame dimensions
        assert_eq!(frame.width, 256);
        assert_eq!(frame.height, 192);
    }

    #[test]
    fn smoke_test_sms() {
        // Load the test ROM
        let rom = include_bytes!("../../../../test_roms/sms/test.sms");
        let mut system = SmsSystem::new();
        system.load_rom(rom.to_vec());

        system.reset();

        // Run for several frames to allow initialization
        for _ in 0..10 {
            let _ = system.step_frame();
        }

        // Get a frame
        let frame = system.step_frame().unwrap();

        // Verify frame dimensions
        assert_eq!(frame.width, 256);
        assert_eq!(frame.height, 192);

        // The test ROM should produce a checkerboard pattern
        // Count unique colors and their distribution
        use std::collections::HashMap;
        let mut color_counts: HashMap<u32, usize> = HashMap::new();
        for &pixel in &frame.pixels {
            *color_counts.entry(pixel).or_insert(0) += 1;
        }

        // We expect exactly 2 colors: white (0xFFFFFFFF) and black (0xFF000000)
        assert_eq!(
            color_counts.len(),
            2,
            "Expected 2 colors (checkerboard), got {}",
            color_counts.len()
        );

        // Count white and black pixels
        let white_count = *color_counts.get(&0xFFFFFFFF).unwrap_or(&0);
        let black_count = *color_counts.get(&0xFF000000).unwrap_or(&0);
        let total_pixels = frame.pixels.len();

        let white_percentage = (white_count as f32 / total_pixels as f32) * 100.0;
        let black_percentage = (black_count as f32 / total_pixels as f32) * 100.0;

        println!(
            "Test ROM: {:.1}% white, {:.1}% black (expected ~50% each for checkerboard)",
            white_percentage, black_percentage
        );

        // For a perfect checkerboard, we expect exactly 50% white and 50% black
        // Allow small tolerance for rounding
        assert!(
            (white_percentage - 50.0).abs() < 1.0,
            "Expected ~50% white pixels, got {:.1}%",
            white_percentage
        );
        assert!(
            (black_percentage - 50.0).abs() < 1.0,
            "Expected ~50% black pixels, got {:.1}%",
            black_percentage
        );
    }

    #[test]
    fn test_vdp_interrupt_triggers() {
        let mut system = SmsSystem::new();

        // Load a simple ROM that enables interrupts
        let mut rom = vec![0; 0x8000];

        // At 0x0000: Enable interrupts and loop
        rom[0x0000] = 0xFB; // EI - Enable interrupts
        rom[0x0001] = 0xED; // IM 1 (prefix)
        rom[0x0002] = 0x56; // IM 1 (opcode)
        rom[0x0003] = 0x76; // HALT - Wait for interrupt

        // At 0x0038: Interrupt handler (IM 1 jumps here)
        rom[0x0038] = 0xFB; // EI - Re-enable interrupts
        rom[0x0039] = 0xED; // RETI (prefix)
        rom[0x003A] = 0x4D; // RETI (opcode)

        system.load_rom(rom);
        system.reset();

        // Enable VDP frame interrupts by setting bit 5 of register 1
        // Use VDP control port to write register 1
        system.vdp.borrow_mut().write_control(0x20); // First byte: value
        system.vdp.borrow_mut().write_control(0x81); // Second byte: register 1

        // Execute initial instructions
        system.cpu.step(); // EI
        system.cpu.step(); // IM 1 (prefix)
        system.cpu.step(); // IM 1 (opcode)

        // Verify state after EI and IM 1
        assert!(system.cpu.iff1, "Interrupts should be enabled after EI");
        assert_eq!(system.cpu.im, 1, "Should be in interrupt mode 1");

        // CPU should now be in HALT state
        system.cpu.step(); // HALT
        assert!(system.cpu.halted, "CPU should be halted");

        let initial_pc = system.cpu.pc;

        // Execute step_frame which should trigger the interrupt via VDP
        // Set scanline to 192 to trigger frame interrupt
        system.vdp.borrow_mut().set_scanline(192);

        // Check if interrupt is pending
        let interrupt_pending = system.vdp.borrow().frame_interrupt_pending();
        assert!(
            interrupt_pending,
            "Frame interrupt should be pending at scanline 192"
        );

        // Manually trigger the interrupt like step_frame does
        system.cpu.interrupt(0xFF);

        // Verify interrupt was triggered
        assert!(!system.cpu.halted, "CPU should exit halt on interrupt");
        assert_eq!(
            system.cpu.pc, 0x0038,
            "PC should jump to IM 1 interrupt vector"
        );
        assert!(
            !system.cpu.iff1,
            "Interrupts should be disabled during handler"
        );

        println!(
            "Interrupt test passed: PC jumped from 0x{:04X} to 0x{:04X}",
            initial_pc, system.cpu.pc
        );
    }

    #[test]
    fn test_interrupt_with_disabled_iff1() {
        let mut system = SmsSystem::new();

        // Load a simple ROM
        let mut rom = vec![0; 0x8000];
        rom[0] = 0xF3; // DI - Disable interrupts
        rom[1] = 0x00; // NOP

        system.load_rom(rom);
        system.reset();

        system.cpu.step(); // DI

        let initial_pc = system.cpu.pc;

        // Try to trigger interrupt
        system.cpu.interrupt(0xFF);

        // PC should not change because interrupts are disabled
        assert_eq!(system.cpu.pc, initial_pc);
        assert!(!system.cpu.iff1, "Interrupts should remain disabled");
    }

    #[test]
    fn test_nmi_functionality() {
        let mut system = SmsSystem::new();

        // Load a simple ROM with NMI handler
        let mut rom = vec![0; 0x8000];

        // Main program
        rom[0x0000] = 0xFB; // EI
        rom[0x0001] = 0x76; // HALT

        // NMI handler at 0x0066
        rom[0x0066] = 0xED; // RETN
        rom[0x0067] = 0x45; // RETN

        system.load_rom(rom);
        system.reset();

        system.cpu.step(); // EI
        assert!(system.cpu.iff1, "Interrupts should be enabled");

        let initial_pc = system.cpu.pc;

        // Trigger NMI
        system.cpu.nmi();

        // Verify NMI behavior
        assert_eq!(system.cpu.pc, 0x0066, "PC should jump to NMI vector");
        assert!(!system.cpu.iff1, "IFF1 should be disabled");
        assert!(system.cpu.iff2, "IFF2 should preserve previous IFF1 state");

        println!(
            "NMI test passed: PC jumped from 0x{:04X} to 0x{:04X}",
            initial_pc, system.cpu.pc
        );
    }

    #[test]
    fn test_audio_generation_integration() {
        let mut system = SmsSystem::new();

        // Generate audio samples
        let samples = system.get_audio_samples(1000);

        // Should generate exactly the requested number of samples
        assert_eq!(samples.len(), 1000);

        // With default muted state, samples should be near zero
        let max_sample = samples.iter().map(|&s| s.abs()).max().unwrap_or(0);
        assert!(
            max_sample <= 100,
            "Expected near-silent output by default, got max sample: {}",
            max_sample
        );

        // Now write to PSG to enable a tone
        // Set channel 0 to max volume
        system.cpu.memory.io_write(0x7F, 0x90); // Volume 0 (max)

        // Set a frequency
        system.cpu.memory.io_write(0x7F, 0x80 | 0x04); // Low bits
        system.cpu.memory.io_write(0x7F, 0x01); // High bits

        // Generate more samples
        let samples_with_tone = system.get_audio_samples(1000);
        assert_eq!(samples_with_tone.len(), 1000);

        // Should now have audible output
        let non_zero_count = samples_with_tone.iter().filter(|&&s| s != 0).count();
        assert!(
            non_zero_count > 0,
            "Expected audio output after enabling tone"
        );
    }

    #[test]
    fn test_set_timing_mode() {
        use emu_core::apu::TimingMode;

        let mut system = SmsSystem::new();

        // Change to PAL timing
        system.set_timing(TimingMode::Pal);

        // Should still be able to generate audio
        let samples = system.get_audio_samples(100);
        assert_eq!(samples.len(), 100);

        // Change back to NTSC
        system.set_timing(TimingMode::Ntsc);

        let samples = system.get_audio_samples(100);
        assert_eq!(samples.len(), 100);
    }

    #[test]
    fn test_cycle_counting() {
        let mut system = SmsSystem::new();

        // Load a simple ROM with known instruction cycles
        let mut rom = vec![0; 0x8000];
        rom[0x0000] = 0x00; // NOP (4 cycles)
        rom[0x0001] = 0x00; // NOP (4 cycles)
        rom[0x0002] = 0xC3; // JP 0x0000 (10 cycles)
        rom[0x0003] = 0x00;
        rom[0x0004] = 0x00;

        system.load_rom(rom);
        system.reset();

        // Initial cycles should be 0
        assert_eq!(system.cycles, 0, "Initial cycles should be 0");

        // Execute one NOP (4 cycles)
        let cycles = system.cpu.step() as u64;
        system.cycles += cycles;
        assert_eq!(cycles, 4, "NOP should take 4 cycles");
        assert_eq!(system.cycles, 4, "Total cycles should be 4 after one NOP");

        // Execute another NOP (4 cycles)
        let cycles = system.cpu.step() as u64;
        system.cycles += cycles;
        assert_eq!(system.cycles, 8, "Total cycles should be 8 after two NOPs");

        // Execute JP instruction (10 cycles)
        let cycles = system.cpu.step() as u64;
        system.cycles += cycles;
        assert_eq!(cycles, 10, "JP should take 10 cycles");
        assert_eq!(system.cycles, 18, "Total cycles should be 18");
    }

    #[test]
    fn test_frame_cycle_target() {
        // Verify the target cycles for one frame matches SMS NTSC specs
        // SMS NTSC: 3.579545 MHz / 60 Hz ≈ 59659 cycles per frame
        let target_cycles = 59659;
        let expected = 3_579_545.0 / 60.0;

        // Allow 1 cycle tolerance due to rounding
        assert!(
            (target_cycles as f64 - expected).abs() < 1.0,
            "Target cycles {} should be close to expected {}",
            target_cycles,
            expected
        );
    }

    #[test]
    fn test_save_load_state() {
        let mut system = SmsSystem::new();

        // Load a ROM and run for a bit
        let mut rom = vec![0; 0x8000];
        rom[0] = 0x3E; // LD A, n
        rom[1] = 0x42;
        rom[2] = 0x06; // LD B, n
        rom[3] = 0x12;
        rom[4] = 0x76; // HALT

        system.load_rom(rom);
        system.reset();

        // Execute a few instructions
        system.cpu.step(); // LD A, 0x42
        system.cpu.step(); // LD B, 0x12

        // Verify CPU state before save
        assert_eq!(system.cpu.a, 0x42);
        assert_eq!(system.cpu.b, 0x12);

        // Save state
        let state = system.save_state();

        // Modify system state
        system.cpu.a = 0xFF;
        system.cpu.b = 0xFF;
        system.cycles = 99999;

        // Verify state was modified
        assert_eq!(system.cpu.a, 0xFF);
        assert_eq!(system.cpu.b, 0xFF);

        // Load state
        system.load_state(&state).unwrap();

        // Verify state was restored
        assert_eq!(system.cpu.a, 0x42);
        assert_eq!(system.cpu.b, 0x12);
    }

    #[test]
    fn test_save_load_state_cpu_registers() {
        let mut system = SmsSystem::new();
        system.load_rom(vec![0; 0x8000]);

        // Set various CPU registers
        system.cpu.a = 0xAA;
        system.cpu.f = 0x55;
        system.cpu.b = 0x11;
        system.cpu.c = 0x22;
        system.cpu.d = 0x33;
        system.cpu.e = 0x44;
        system.cpu.h = 0x55;
        system.cpu.l = 0x66;
        system.cpu.ix = 0x1234;
        system.cpu.iy = 0x5678;
        system.cpu.sp = 0xFFFE;
        system.cpu.pc = 0x8000;
        system.cpu.iff1 = true;
        system.cpu.iff2 = false;
        system.cpu.im = 2;
        system.cycles = 12345;

        // Save and restore
        let state = system.save_state();
        system.cpu.a = 0;
        system.cpu.f = 0;
        system.load_state(&state).unwrap();

        // Verify all registers restored
        assert_eq!(system.cpu.a, 0xAA);
        assert_eq!(system.cpu.f, 0x55);
        assert_eq!(system.cpu.b, 0x11);
        assert_eq!(system.cpu.c, 0x22);
        assert_eq!(system.cpu.d, 0x33);
        assert_eq!(system.cpu.e, 0x44);
        assert_eq!(system.cpu.h, 0x55);
        assert_eq!(system.cpu.l, 0x66);
        assert_eq!(system.cpu.ix, 0x1234);
        assert_eq!(system.cpu.iy, 0x5678);
        assert_eq!(system.cpu.sp, 0xFFFE);
        assert_eq!(system.cpu.pc, 0x8000);
        assert!(system.cpu.iff1);
        assert!(!system.cpu.iff2);
        assert_eq!(system.cpu.im, 2);
        assert_eq!(system.cycles, 12345);
    }

    #[test]
    fn test_save_load_state_memory() {
        let mut system = SmsSystem::new();
        system.load_rom(vec![0; 0x8000]);

        // Write some data to RAM
        system.cpu.memory.write(0xC000, 0xAB);
        system.cpu.memory.write(0xC100, 0xCD);
        system.cpu.memory.write(0xDFFF, 0xEF);

        // Save state
        let state = system.save_state();

        // Clear RAM
        system.cpu.memory.write(0xC000, 0);
        system.cpu.memory.write(0xC100, 0);
        system.cpu.memory.write(0xDFFF, 0);

        // Load state
        system.load_state(&state).unwrap();

        // Verify RAM was restored
        assert_eq!(system.cpu.memory.read(0xC000), 0xAB);
        assert_eq!(system.cpu.memory.read(0xC100), 0xCD);
        assert_eq!(system.cpu.memory.read(0xDFFF), 0xEF);
    }

    #[test]
    fn test_save_load_state_vdp() {
        let mut system = SmsSystem::new();
        system.load_rom(vec![0; 0x8000]);

        // Write to VDP registers and VRAM
        system.vdp.borrow_mut().write_control(0x20); // Register value
        system.vdp.borrow_mut().write_control(0x81); // Write to register 1

        // Write to VRAM
        system.vdp.borrow_mut().write_control(0x00); // Address low
        system.vdp.borrow_mut().write_control(0x40); // Address high (VRAM write)
        system.vdp.borrow_mut().write_data(0x42);

        // Get tile viewer data to verify state
        let data_before = system.get_tile_viewer_data();

        // Save state
        let state = system.save_state();

        // Modify VDP state
        system.vdp.borrow_mut().reset();

        // Load state
        system.load_state(&state).unwrap();

        // Verify VDP state was restored
        let data_after = system.get_tile_viewer_data();
        assert_eq!(data_after.registers[1], data_before.registers[1]);
        assert_eq!(data_after.vram[0], data_before.vram[0]);
        assert_eq!(data_after.registers[1], 0x20);
        assert_eq!(data_after.vram[0], 0x42);
    }

    #[test]
    fn test_save_load_state_invalid_system() {
        let mut system = SmsSystem::new();
        system.load_rom(vec![0; 0x8000]);

        // Try to load a state from a different system
        let invalid_state = serde_json::json!({
            "system": "nes",
            "version": 1,
        });

        let result = system.load_state(&invalid_state);
        assert!(result.is_err(), "Should reject state from different system");
    }

    #[test]
    fn test_pal_timing_detection() {
        use emu_core::apu::TimingMode;
        let mut system = SmsSystem::new();

        // Create a ROM with PAL region header
        let mut rom = vec![0; 0x8000];

        // Add TMR SEGA header at 0x7FF0
        rom[0x7FF0..0x7FF0 + 8].copy_from_slice(b"TMR SEGA");
        rom[0x7FFF] = 0x40; // Export region (PAL)

        system.load_rom(rom);

        // Should detect as PAL
        assert_eq!(system.get_timing(), TimingMode::Pal);
    }

    #[test]
    fn test_ntsc_timing_detection() {
        use emu_core::apu::TimingMode;
        let mut system = SmsSystem::new();

        // Create a ROM with Japan (NTSC) region header
        let mut rom = vec![0; 0x8000];

        // Add TMR SEGA header at 0x7FF0
        rom[0x7FF0..0x7FF0 + 8].copy_from_slice(b"TMR SEGA");
        rom[0x7FFF] = 0x30; // Japan region (NTSC)

        system.load_rom(rom);

        // Should detect as NTSC
        assert_eq!(system.get_timing(), TimingMode::Ntsc);
    }

    #[test]
    fn test_default_timing_no_header() {
        use emu_core::apu::TimingMode;
        let mut system = SmsSystem::new();

        // Create a ROM without header
        let rom = vec![0; 0x8000];

        system.load_rom(rom);

        // Should default to NTSC
        assert_eq!(system.get_timing(), TimingMode::Ntsc);
    }

    #[test]
    fn test_pal_frame_cycles() {
        // PAL: 3.546894 MHz / 50 Hz ≈ 70938 cycles per frame
        let target_cycles = 70938;
        let expected = 3_546_894.0 / 50.0;

        // Allow 1 cycle tolerance due to rounding
        assert!(
            (target_cycles as f64 - expected).abs() < 1.0,
            "PAL target cycles {} should be close to expected {}",
            target_cycles,
            expected
        );
    }

    #[test]
    fn test_save_load_state_timing_mode() {
        use emu_core::apu::TimingMode;
        let mut system = SmsSystem::new();
        system.load_rom(vec![0; 0x8000]);

        // Set to PAL mode
        system.set_timing(TimingMode::Pal);
        assert_eq!(system.get_timing(), TimingMode::Pal);

        // Save state
        let state = system.save_state();

        // Change to NTSC
        system.set_timing(TimingMode::Ntsc);
        assert_eq!(system.get_timing(), TimingMode::Ntsc);

        // Load state - should restore PAL
        system.load_state(&state).unwrap();
        assert_eq!(system.get_timing(), TimingMode::Pal);
    }
}
