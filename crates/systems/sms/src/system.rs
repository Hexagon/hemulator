//! Sega Master System main system implementation

use crate::bus::SmsMemory;
use crate::vdp::Vdp;
use emu_core::apu::{AudioChip, Sn76489Psg, TimingMode};
use emu_core::cpu_z80::CpuZ80;
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

    /// Sega Master System emulator
pub struct SmsSystem {
    // CPU
    cpu: CpuZ80<SmsMemory>,

    // Shared components
    vdp: Rc<RefCell<Vdp>>,
    psg: Rc<RefCell<Sn76489Psg>>,

    // Timing
    cycles: u64,
}

impl SmsSystem {
    /// Create a new SMS system
    pub fn new() -> Self {
        // Create shared components
        let vdp = Rc::new(RefCell::new(Vdp::new()));
        let psg = Rc::new(RefCell::new(Sn76489Psg::new(TimingMode::Ntsc)));

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
        }
    }

    /// Load a ROM
    pub fn load_rom(&mut self, rom_data: Vec<u8>) {
        // Create new memory with ROM
        let memory = SmsMemory::new(rom_data, Rc::clone(&self.vdp), Rc::clone(&self.psg));
        self.cpu = CpuZ80::new(memory);
        self.reset();
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
        self.cpu.reset();
        self.vdp.borrow_mut().reset();
        self.psg.borrow_mut().reset();
        self.cycles = 0;
    }

    fn step_frame(&mut self) -> Result<Frame, Self::Error> {
        let target_cycles = 59659; // ~3.58 MHz / 60 Hz

        while self.cycles < target_cycles {
            // Execute one CPU instruction
            let cpu_cycles = self.cpu.step() as u64;
            self.cycles += cpu_cycles;

            // Clock VDP (simplified - step by scanline instead of pixel)
            // VDP runs at ~3.58 MHz, renders 262 scanlines per frame
            // Each scanline takes roughly 228 cycles
            let scanlines_to_render = cpu_cycles / 228;
            for _ in 0..scanlines_to_render {
                self.vdp.borrow_mut().step_scanline();
            }

            // Check for VDP interrupts
            // TODO: Implement Z80 interrupt handling once CPU is complete
            if self.vdp.borrow().frame_interrupt_pending() {
                // Would trigger interrupt (IM 1: jump to 0x0038)
                // self.cpu.interrupt();
            }
        }

        self.cycles -= target_cycles;

        // Get frame from VDP
        let frame = self.vdp.borrow().get_frame().clone();

        Ok(frame)
    }

    fn save_state(&self) -> Value {
        // TODO: Implement state serialization
        serde_json::json!({
            "cycles": self.cycles,
        })
    }

    fn load_state(&mut self, _state: &Value) -> Result<(), serde_json::Error> {
        // TODO: Implement state deserialization
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
}
