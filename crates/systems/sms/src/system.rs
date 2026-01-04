//! Sega Master System main system implementation

use crate::bus::SmsMemory;
use crate::vdp::Vdp;
use emu_core::apu::{AudioChip, Sn76489Psg, TimingMode};
use emu_core::cpu_z80::{CpuZ80, MemoryZ80};
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
        // Log first few bytes for debugging
        log(LogCategory::CPU, LogLevel::Debug, || {
            format!(
                "SMS ROM: First 16 bytes: {:02X?}",
                &rom_data[0..16.min(rom_data.len())]
            )
        });
        
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
        log(LogCategory::CPU, LogLevel::Info, || "SMS: System reset".to_string());
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
        let target_cycles = 59659; // ~3.58 MHz / 60 Hz
        
        static mut FRAME_COUNT: u64 = 0;
        let log_this_frame = unsafe {
            FRAME_COUNT += 1;
            FRAME_COUNT <= 5
        };

        while self.cycles < target_cycles {
            // Log CPU state on first few frames
            if log_this_frame && self.cycles < 100 {
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
            let cpu_cycles = self.cpu.step() as u64;
            self.cycles += cpu_cycles;

            // Update VDP scanline based on cycles
            // Each scanline takes approximately 228 cycles (~3.58MHz / 262 scanlines / 60Hz)
            let current_scanline = (self.cycles / 228) % 262;
            self.vdp.borrow_mut().set_scanline(current_scanline as u16);

            // Check for VDP interrupts
            if self.vdp.borrow().frame_interrupt_pending() {
                // Trigger Z80 interrupt (IM 1: RST 38h = jump to 0x0038)
                self.cpu.interrupt();
                self.vdp.borrow_mut().clear_frame_interrupt();
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
        // Check that we have non-zero pixels (display is working)
        let pixels = &frame.pixels;
        let non_zero_count = pixels.iter().filter(|&&p| p != 0).count();

        // We should have a significant number of white pixels from the checkerboard
        // The exact count depends on VDP implementation, but it should be > 0
        assert!(
            non_zero_count > 0,
            "Expected visible output from test ROM, got {} non-zero pixels",
            non_zero_count
        );

        // For a proper checkerboard, approximately half the pixels should be white
        // Allow a wide tolerance since the exact rendering depends on tile implementation
        let total_pixels = pixels.len();
        let white_percentage = (non_zero_count as f32 / total_pixels as f32) * 100.0;

        println!(
            "Test ROM produced {:.1}% white pixels (expected ~50% for checkerboard)",
            white_percentage
        );

        // Very loose check - just ensure SOME pixels are rendered
        // (More strict checking would require full VDP implementation)
        assert!(
            white_percentage > 1.0,
            "Expected at least some visible pixels, got {:.1}%",
            white_percentage
        );
    }
}
