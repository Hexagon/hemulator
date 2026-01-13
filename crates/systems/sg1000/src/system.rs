//! SG-1000 main system implementation

use crate::bus::Sg1000Memory;
use crate::psg::Sg1000Psg;
use crate::vdp::Vdp;
use emu_core::cpu_z80::CpuZ80;
use emu_core::debug::Debugger;
use emu_core::logging::{log, LogCategory, LogLevel};
use emu_core::renderer::Renderer;
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
    vdp: Rc<RefCell<Vdp>>,
    psg: Rc<RefCell<Sg1000Psg>>,

    // Timing
    cycles: u64,
    cpu_cycles_per_frame: u32,
    scanline_cycles: u32,

    // Audio buffer
    audio_buffer: Vec<i16>,

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
        let vdp = Rc::new(RefCell::new(Vdp::new()));
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
            audio_buffer: Vec::new(),
            cartridge_loaded: false,
            instruction_tracer: emu_core::instruction_tracer::InstructionTracer::new(),
            breakpoint_manager: emu_core::breakpoints::BreakpointManager::new(),
        }
    }

    /// Set controller state
    pub fn set_controller(&mut self, port: u8, state: u8) {
        self.cpu.memory.set_controller(port, state);
    }

    /// Get tile viewer data for debugging
    pub fn get_tile_viewer_data(&self) -> TileViewerData {
        self.vdp.borrow().get_tile_viewer_data()
    }
}

impl System for Sg1000System {
    type Error = Sg1000Error;

    fn step_frame(&mut self) -> Result<Frame, Self::Error> {
        // Execute one frame worth of CPU cycles
        let mut cycles_this_frame = 0;

        while cycles_this_frame < self.cpu_cycles_per_frame {
            // Execute one CPU instruction
            let cycles = self.cpu.step();
            cycles_this_frame += cycles as u32;
            self.cycles += cycles as u64;

            // Check for VDP interrupt
            if self.vdp.borrow().frame_interrupt_pending() {
                self.cpu.interrupt(0xFF);
                self.vdp.borrow_mut().clear_frame_interrupt();
            }
        }

        // Render happens during step_frame execution, no separate render_frame call needed

        Ok(self.vdp.borrow().get_frame().clone())
    }

    fn reset(&mut self) {
        self.cpu.reset();
        self.vdp.borrow_mut().reset();
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
