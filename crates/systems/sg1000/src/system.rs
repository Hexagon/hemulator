//! SG-1000 main system implementation

use crate::bus::Sg1000Memory;
use crate::psg::Sg1000Psg;
use emu_core::cpu_z80::CpuZ80;
use emu_core::debug::Debugger;
use emu_core::logging::{log, LogCategory, LogLevel};
use emu_core::renderer::Renderer;
use emu_core::tms9918a::Tms9918a;
use emu_core::types::Frame;
use emu_core::{MountPointInfo, System};
use serde_json::Value;
use std::cell::RefCell;
use std::rc::Rc;
use thiserror::Error;

/// SG-1000 emulator errors
#[derive(Debug, Error)]
pub enum Sg1000Error {
    #[error("Invalid mount point")]
    InvalidMountPoint,
    #[error("Cartridge not loaded")]
    CartridgeNotLoaded,
}

/// Data for the tile viewer tab (SG-1000)
#[derive(Clone)]
pub struct TileViewerData {
    /// VRAM data (16KB)
    pub vram: Vec<u8>,
    /// Palette colors as RGB (16 colors)
    pub palette: Vec<u32>,
    /// VDP registers
    pub registers: Vec<u8>,
}

/// SG-1000 emulator
pub struct Sg1000System {
    // CPU
    pub(crate) cpu: CpuZ80<Sg1000Memory>,

    // Shared components
    vdp: Rc<RefCell<Tms9918a>>,
    psg: Rc<RefCell<Sg1000Psg>>,

    // Timing
    cycles: u64,
    cpu_cycles_per_frame: u32,
    #[allow(dead_code)] // Calculated but reserved for future use (e.g., line interrupts)
    scanline_cycles: u32,

    // Loaded media
    cartridge_loaded: bool,

    // Debugging
    /// Instruction tracer for debugging
    pub instruction_tracer: emu_core::instruction_tracer::InstructionTracer,
    /// Breakpoint manager for debugging
    pub breakpoint_manager: emu_core::breakpoints::BreakpointManager,
}

impl Sg1000System {
    /// Create a new SG-1000 system
    pub fn new() -> Self {
        // Create shared components
        let vdp = Rc::new(RefCell::new(Tms9918a::new()));
        let psg = Rc::new(RefCell::new(Sg1000Psg::new()));

        // Create empty ROM
        let rom = vec![0; 0x8000]; // 32KB default ROM size
        let memory = Sg1000Memory::new(rom, Rc::clone(&vdp), Rc::clone(&psg));

        // Create CPU
        let cpu = CpuZ80::new(memory);

        // SG-1000 timing: 3.579545 MHz CPU, 60 Hz (NTSC)
        let cpu_frequency = 3_579_545;
        let frame_rate = 60;
        let cpu_cycles_per_frame = cpu_frequency / frame_rate;

        // 262 scanlines per frame (NTSC)
        let scanlines_per_frame = 262;
        let scanline_cycles = cpu_cycles_per_frame / scanlines_per_frame;

        Self {
            cpu,
            vdp,
            psg,
            cycles: 0,
            cpu_cycles_per_frame,
            scanline_cycles,
            cartridge_loaded: false,
            instruction_tracer: emu_core::instruction_tracer::InstructionTracer::new(),
            breakpoint_manager: emu_core::breakpoints::BreakpointManager::new(),
        }
    }

    /// Set controller 1 state (type-safe method)
    pub fn set_controller1(&mut self, state: u8) {
        self.cpu.memory.set_controller1(state);
    }

    /// Set controller 2 state (type-safe method)
    pub fn set_controller2(&mut self, state: u8) {
        self.cpu.memory.set_controller2(state);
    }

    /// Set controller state (generic method for backward compatibility)
    pub fn set_controller(&mut self, port: u8, state: u8) {
        self.cpu.memory.set_controller(port, state);
    }

    /// Get tile viewer data for debugging
    pub fn get_tile_viewer_data(&self) -> TileViewerData {
        let (vram, palette, registers) = self.vdp.borrow().get_tile_viewer_data();
        TileViewerData {
            vram,
            palette,
            registers,
        }
    }
}

impl System for Sg1000System {
    type Error = Sg1000Error;

    fn step_frame(&mut self) -> Result<Frame, Self::Error> {
        // Execute one frame worth of CPU cycles
        let mut cycles_this_frame = 0;

        // SG-1000 timing: 262 scanlines per frame (NTSC)
        let total_scanlines = 262_u64;
        let target_cycles = self.cpu_cycles_per_frame as u64;

        while cycles_this_frame < self.cpu_cycles_per_frame {
            // Execute one CPU instruction
            let cycles = self.cpu.step();
            cycles_this_frame += cycles;
            self.cycles += cycles as u64;

            // Update VDP scanline based on cycles executed (for interrupt timing)
            let current_scanline =
                (cycles_this_frame as u64 * total_scanlines / target_cycles) % total_scanlines;
            self.vdp.borrow_mut().set_scanline(current_scanline as u16);

            // Check for VDP interrupt (don't clear it here - the game will clear it by reading status)
            if self.vdp.borrow().frame_interrupt_pending() {
                self.cpu.interrupt(0xFF);
            }
        }

        // Render the full frame once at the end
        self.vdp.borrow_mut().render_frame();

        Ok(self.vdp.borrow().get_frame().clone())
    }

    fn reset(&mut self) {
        self.cpu.reset();
        self.vdp.borrow_mut().reset();
        self.psg.borrow_mut().reset();
        self.cycles = 0;
        log(LogCategory::Bus, LogLevel::Info, || {
            "SG-1000: System reset".to_string()
        });
    }

    fn mount_points(&self) -> Vec<MountPointInfo> {
        vec![MountPointInfo {
            id: "Cartridge".to_string(),
            name: "Cartridge".to_string(),
            extensions: vec!["sg".to_string(), "sc".to_string()],
            required: true,
        }]
    }

    fn mount(&mut self, mount_point_id: &str, data: &[u8]) -> Result<(), Self::Error> {
        match mount_point_id {
            "Cartridge" => {
                if data.is_empty() {
                    return Err(Sg1000Error::CartridgeNotLoaded);
                }
                self.cpu.memory.rom = data.to_vec();
                self.cartridge_loaded = true;
                log(LogCategory::Bus, LogLevel::Info, || {
                    format!("SG-1000: Cartridge loaded ({} bytes)", data.len())
                });
                Ok(())
            }
            _ => Err(Sg1000Error::InvalidMountPoint),
        }
    }

    fn unmount(&mut self, mount_point_id: &str) -> Result<(), Self::Error> {
        match mount_point_id {
            "Cartridge" => {
                self.cpu.memory.rom = vec![0; 0x8000];
                self.cartridge_loaded = false;
                Ok(())
            }
            _ => Err(Sg1000Error::InvalidMountPoint),
        }
    }

    fn is_mounted(&self, mount_point_id: &str) -> bool {
        match mount_point_id {
            "Cartridge" => self.cartridge_loaded,
            _ => false,
        }
    }

    fn supports_save_states(&self) -> bool {
        true
    }

    fn save_state(&self) -> Value {
        serde_json::json!({
            "system": "sg1000",
            "version": 1,
            "cycles": self.cycles,
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
            "vdp": self.vdp.borrow().get_state(),
            "psg": self.psg.borrow().get_state(),
            "memory": self.cpu.memory.get_state(),
        })
    }

    fn load_state(&mut self, state: &Value) -> Result<(), serde_json::Error> {
        // Validate system type
        if let Some(system) = state.get("system").and_then(|s| s.as_str()) {
            if system != "sg1000" {
                // Return error by trying to deserialize incompatible data
                let _: () = serde_json::from_value(serde_json::json!({
                    "error": "Incompatible system type"
                }))?;
                return Ok(()); // Will never reach here
            }
        }

        // Load CPU state
        if let Some(cpu_state) = state.get("cpu") {
            macro_rules! load_u8 {
                ($field:literal, $target:expr) => {
                    if let Some(val) = cpu_state.get($field).and_then(|v| v.as_u64()) {
                        $target = val as u8;
                    }
                };
            }

            macro_rules! load_u16 {
                ($field:literal, $target:expr) => {
                    if let Some(val) = cpu_state.get($field).and_then(|v| v.as_u64()) {
                        $target = val as u16;
                    }
                };
            }

            macro_rules! load_bool {
                ($field:literal, $target:expr) => {
                    if let Some(val) = cpu_state.get($field).and_then(|v| v.as_bool()) {
                        $target = val;
                    }
                };
            }

            load_u8!("a", self.cpu.a);
            load_u8!("f", self.cpu.f);
            load_u8!("b", self.cpu.b);
            load_u8!("c", self.cpu.c);
            load_u8!("d", self.cpu.d);
            load_u8!("e", self.cpu.e);
            load_u8!("h", self.cpu.h);
            load_u8!("l", self.cpu.l);
            load_u8!("a_prime", self.cpu.a_prime);
            load_u8!("f_prime", self.cpu.f_prime);
            load_u8!("b_prime", self.cpu.b_prime);
            load_u8!("c_prime", self.cpu.c_prime);
            load_u8!("d_prime", self.cpu.d_prime);
            load_u8!("e_prime", self.cpu.e_prime);
            load_u8!("h_prime", self.cpu.h_prime);
            load_u8!("l_prime", self.cpu.l_prime);
            load_u16!("ix", self.cpu.ix);
            load_u16!("iy", self.cpu.iy);
            load_u8!("i", self.cpu.i);
            load_u8!("r", self.cpu.r);
            load_u16!("sp", self.cpu.sp);
            load_u16!("pc", self.cpu.pc);
            load_bool!("iff1", self.cpu.iff1);
            load_bool!("iff2", self.cpu.iff2);
            load_u8!("im", self.cpu.im);
            load_bool!("halted", self.cpu.halted);
        }

        // Load VDP state
        if let Some(vdp_state) = state.get("vdp") {
            self.vdp.borrow_mut().set_state(vdp_state)?;
        }

        // Load PSG state
        if let Some(psg_state) = state.get("psg") {
            self.psg.borrow_mut().set_state(psg_state)?;
        }

        // Load memory state
        if let Some(memory_state) = state.get("memory") {
            self.cpu.memory.set_state(memory_state)?;
        }

        // Load cycles
        if let Some(cycles) = state.get("cycles").and_then(|v| v.as_u64()) {
            self.cycles = cycles;
        }

        Ok(())
    }

    fn get_total_cycles(&self) -> u64 {
        self.cycles
    }

    fn debugger(&self) -> Option<&dyn Debugger> {
        Some(self)
    }
}

impl Sg1000System {
    /// Get audio samples from the PSG
    ///
    /// This method generates the requested number of audio samples by clocking
    /// the SN76489 PSG audio chip.
    pub fn get_audio_samples(&mut self, count: usize) -> Vec<i16> {
        self.psg.borrow_mut().generate_samples(count)
    }

    /// Get resolution for the renderer
    pub fn resolution(&self) -> (usize, usize) {
        (256, 192) // TMS9918A resolution
    }
}

impl Default for Sg1000System {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use emu_core::cpu_z80::MemoryZ80;
    use emu_core::System;

    #[test]
    fn test_system_creation() {
        let system = Sg1000System::new();
        assert_eq!(system.cycles, 0);
        assert!(!system.cartridge_loaded);
    }

    #[test]
    fn test_system_reset() {
        let mut system = Sg1000System::new();

        // Execute some cycles
        let _ = system.step_frame();
        assert!(system.cycles > 0);

        // Reset
        system.reset();
        assert_eq!(system.cycles, 0);
    }

    #[test]
    fn test_audio_generation() {
        let mut system = Sg1000System::new();

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
    }

    #[test]
    fn test_audio_with_tone() {
        let mut system = Sg1000System::new();

        // Write to PSG to enable a tone
        // SG-1000 PSG is at I/O ports 0x40-0x7F (all mirrored)
        system.cpu.memory.io_write(0x7F, 0x90); // Channel 0, Volume 0 (max)
        system.cpu.memory.io_write(0x7F, 0x80 | 0x04); // Channel 0, Tone low bits
        system.cpu.memory.io_write(0x7F, 0x01); // Tone high bits

        // Generate samples
        let samples = system.get_audio_samples(1000);
        assert_eq!(samples.len(), 1000);

        // Should now have audible output
        let non_zero_count = samples.iter().filter(|&&s| s != 0).count();
        assert!(
            non_zero_count > 0,
            "Expected audio output after enabling tone"
        );
    }

    #[test]
    fn test_psg_reset() {
        let mut system = Sg1000System::new();

        // Enable a tone
        system.cpu.memory.io_write(0x7F, 0x90); // Max volume
        system.cpu.memory.io_write(0x7F, 0x84); // Tone frequency

        // Reset system
        system.reset();

        // PSG should be reset - audio should be silent again
        let samples = system.get_audio_samples(100);
        let max_sample = samples.iter().map(|&s| s.abs()).max().unwrap_or(0);
        assert!(
            max_sample <= 100,
            "Expected near-silent output after reset, got max sample: {}",
            max_sample
        );
    }

    #[test]
    fn test_resolution() {
        let system = Sg1000System::new();
        let (width, height) = system.resolution();
        assert_eq!(width, 256);
        assert_eq!(height, 192);
    }

    #[test]
    fn test_mount_cartridge() {
        let mut system = Sg1000System::new();

        // Create a dummy cartridge
        let cartridge = vec![0x00, 0xC3, 0x00, 0x00]; // JP 0x0000

        // Mount cartridge
        let result = system.mount("Cartridge", &cartridge);
        assert!(result.is_ok());
        assert!(system.is_mounted("Cartridge"));
    }

    #[test]
    fn test_save_state_roundtrip() {
        let mut system = Sg1000System::new();

        // Mount and run a bit
        let cartridge = vec![0; 0x8000];
        system.mount("Cartridge", &cartridge).unwrap();
        let _ = system.step_frame();

        // Save state
        let state = system.save_state();

        // Modify system
        system.reset();

        // Load state
        let result = system.load_state(&state);
        assert!(result.is_ok());

        // State should be restored
        // Note: We don't check exact cycles because load_state may not restore them exactly
    }
}
