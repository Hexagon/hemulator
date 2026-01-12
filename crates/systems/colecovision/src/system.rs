//! ColecoVision main system implementation

use crate::bus::ColecoVisionMemory;
use crate::psg::ColecoVisionPsg;
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

/// ColecoVision emulator errors
#[derive(Debug, Error)]
pub enum ColecoVisionError {
    #[error("Invalid mount point")]
    InvalidMountPoint,
    #[error("BIOS not loaded")]
    BiosNotLoaded,
    #[error("Cartridge not loaded")]
    CartridgeNotLoaded,
}

/// Data for the tile viewer tab (ColecoVision)
#[derive(Clone)]
pub struct TileViewerData {
    /// VRAM data (16KB)
    pub vram: Vec<u8>,
    /// Palette colors as RGB (16 colors)
    pub palette: Vec<u32>,
    /// VDP registers
    pub registers: Vec<u8>,
}

/// ColecoVision emulator
pub struct ColecoVisionSystem {
    // CPU
    pub(crate) cpu: CpuZ80<ColecoVisionMemory>,

    // Shared components
    vdp: Rc<RefCell<Vdp>>,
    psg: Rc<RefCell<ColecoVisionPsg>>,

    // Timing
    cycles: u64,
    cpu_cycles_per_frame: u32,
    scanline_cycles: u32,

    // Audio buffer
    audio_buffer: Vec<i16>,

    // Loaded media
    bios_loaded: bool,
    cartridge_loaded: bool,

    // Debugging
    /// Instruction tracer for debugging
    pub(crate) instruction_tracer: emu_core::instruction_tracer::InstructionTracer,
    /// Breakpoint manager for debugging
    pub(crate) breakpoint_manager: emu_core::breakpoints::BreakpointManager,
}

impl ColecoVisionSystem {
    /// Create a new ColecoVision system
    pub fn new() -> Self {
        // Create shared components
        let vdp = Rc::new(RefCell::new(Vdp::new()));
        let psg = Rc::new(RefCell::new(ColecoVisionPsg::new()));

        // Create empty BIOS and ROM
        let bios = vec![0; 0x2000]; // 8KB BIOS
        let rom = vec![0; 0x8000]; // 32KB default ROM size
        let memory = ColecoVisionMemory::new(bios, rom, Rc::clone(&vdp), Rc::clone(&psg));

        // Create CPU
        let cpu = CpuZ80::new(memory);

        // ColecoVision timing: 3.579545 MHz CPU, 60 Hz (NTSC)
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
            bios_loaded: false,
            cartridge_loaded: false,
            instruction_tracer: emu_core::instruction_tracer::InstructionTracer::new(),
            breakpoint_manager: emu_core::breakpoints::BreakpointManager::new(),
        }
    }

    /// Load BIOS ROM
    pub fn load_bios(&mut self, bios_data: Vec<u8>) {
        log(LogCategory::CPU, LogLevel::Info, || {
            format!("ColecoVision: Loading BIOS ({} bytes)", bios_data.len())
        });

        self.bios_loaded = true;

        // Create new memory with BIOS
        let rom = if self.cartridge_loaded {
            // Keep existing ROM
            self.cpu.memory.rom.clone()
        } else {
            vec![0; 0x8000]
        };

        let memory =
            ColecoVisionMemory::new(bios_data, rom, Rc::clone(&self.vdp), Rc::clone(&self.psg));
        self.cpu = CpuZ80::new(memory);
        self.reset();
    }

    /// Load a cartridge ROM
    pub fn load_cartridge(&mut self, rom_data: Vec<u8>) {
        log(LogCategory::CPU, LogLevel::Info, || {
            format!(
                "ColecoVision: Loading cartridge ({} bytes)",
                rom_data.len()
            )
        });

        self.cartridge_loaded = true;

        // Create new memory with cartridge
        let bios = if self.bios_loaded {
            // Keep existing BIOS
            self.cpu.memory.bios.clone()
        } else {
            vec![0; 0x2000]
        };

        let memory =
            ColecoVisionMemory::new(bios, rom_data, Rc::clone(&self.vdp), Rc::clone(&self.psg));
        self.cpu = CpuZ80::new(memory);
        self.reset();
    }

    /// Get tile viewer data for debugging
    pub fn get_tile_viewer_data(&self) -> TileViewerData {
        self.vdp.borrow().get_tile_viewer_data()
    }

    /// Set controller state
    pub fn set_controller(&mut self, controller: u8, state: u8) {
        match controller {
            1 => self.cpu.memory.set_controller1(state),
            2 => self.cpu.memory.set_controller2(state),
            _ => {}
        }
    }
}

impl Default for ColecoVisionSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl System for ColecoVisionSystem {
    type Error = ColecoVisionError;

    fn reset(&mut self) {
        self.cpu.reset();
        self.vdp.borrow_mut().reset();
        self.cycles = 0;
        self.audio_buffer.clear();
    }

    fn step_frame(&mut self) -> Result<Frame, Self::Error> {
        let target_cycles = self.cycles + self.cpu_cycles_per_frame as u64;

        while self.cycles < target_cycles {
            // Step CPU
            let cpu_cycles = self.cpu.step();
            self.cycles += cpu_cycles as u64;

            // Update VDP scanline based on cycles
            let scanline = ((self.cycles % self.cpu_cycles_per_frame as u64) / self.scanline_cycles as u64) as u16;
            self.vdp.borrow_mut().set_scanline(scanline);

            // Check for VDP interrupt
            if self.vdp.borrow().frame_interrupt_pending() {
                self.cpu.trigger_interrupt();
                self.vdp.borrow_mut().clear_frame_interrupt();
            }

            // Step PSG and collect audio samples
            let samples = self.psg.borrow_mut().step(cpu_cycles);
            self.audio_buffer.extend_from_slice(&samples);
        }

        Ok(self.vdp.borrow().get_frame())
    }

    fn save_state(&self) -> Value {
        serde_json::json!({
            "cpu": self.cpu.get_state(),
            "vdp": self.vdp.borrow().get_state(),
            "psg": self.psg.borrow().get_state(),
            "memory": self.cpu.memory.get_state(),
            "cycles": self.cycles,
        })
    }

    fn load_state(&mut self, state: &Value) -> Result<(), serde_json::Error> {
        if let Some(cpu_state) = state.get("cpu") {
            self.cpu.set_state(cpu_state)?;
        }

        if let Some(vdp_state) = state.get("vdp") {
            self.vdp.borrow_mut().set_state(vdp_state)?;
        }

        if let Some(psg_state) = state.get("psg") {
            self.psg.borrow_mut().set_state(psg_state)?;
        }

        if let Some(memory_state) = state.get("memory") {
            self.cpu.memory.set_state(memory_state)?;
        }

        if let Some(cycles) = state.get("cycles").and_then(|v| v.as_u64()) {
            self.cycles = cycles;
        }

        Ok(())
    }

    fn supports_save_states(&self) -> bool {
        true
    }

    fn mount_points(&self) -> Vec<MountPointInfo> {
        vec![
            MountPointInfo {
                id: "BIOS".to_string(),
                name: "BIOS ROM".to_string(),
                extensions: vec!["rom".to_string(), "bin".to_string()],
                required: true,
            },
            MountPointInfo {
                id: "Cartridge".to_string(),
                name: "Cartridge".to_string(),
                extensions: vec!["col".to_string(), "bin".to_string()],
                required: true,
            },
        ]
    }

    fn mount(&mut self, mount_point_id: &str, data: &[u8]) -> Result<(), Self::Error> {
        match mount_point_id {
            "BIOS" => {
                self.load_bios(data.to_vec());
                Ok(())
            }
            "Cartridge" => {
                self.load_cartridge(data.to_vec());
                Ok(())
            }
            _ => Err(ColecoVisionError::InvalidMountPoint),
        }
    }

    fn unmount(&mut self, mount_point_id: &str) -> Result<(), Self::Error> {
        match mount_point_id {
            "BIOS" => {
                self.bios_loaded = false;
                Ok(())
            }
            "Cartridge" => {
                self.cartridge_loaded = false;
                Ok(())
            }
            _ => Err(ColecoVisionError::InvalidMountPoint),
        }
    }

    fn is_mounted(&self, mount_point_id: &str) -> bool {
        match mount_point_id {
            "BIOS" => self.bios_loaded,
            "Cartridge" => self.cartridge_loaded,
            _ => false,
        }
    }

    fn debugger(&self) -> Option<&dyn Debugger> {
        Some(self)
    }

    fn get_total_cycles(&self) -> u64 {
        self.cycles
    }
}
